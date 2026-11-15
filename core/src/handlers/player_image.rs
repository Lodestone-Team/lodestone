//! What a server serves its players over HTTP (`docs/instance-images.md`, "Server to
//! player"): the client view of its current content image or a delta to it
//! (`GET …/image`), and the bytes only the server has (`GET …/objects/{sha256}`).
//!
//! The same answers are reachable three ways, each router checking access first exactly
//! as its client-pack route does and then calling these:
//!
//! - `/launcher/instances/{uuid}/…` (a signed-in user or launcher key with `ViewInstance`)
//!   and `/public/instances/{uuid}/…` (no sign-in; the instance listed with the client-pack
//!   scope), in [`super::public_instances`];
//! - `/api/v1/public/instances/{uuid}/…` through a Connect invite's info port
//!   (`crate::connect::info_router`) and a friend tunnel (`crate::connect::friends`), for the
//!   shared instance only.

use axum::{
    body::Body,
    http::{
        header::{
            ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, RANGE,
            RETRY_AFTER,
        },
        HeaderMap, HeaderValue, Method, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use color_eyre::eyre::{eyre, Context};
use lodestone_image::hex::is_sha256;
use serde::Deserialize;

use crate::error::{Error, ErrorKind};
use lodestone_image::store::chunklist::CHUNKED_ABOVE;

use crate::images::objects::{
    object_stream, object_there, ByteRange, ClientKey, There, IDLE_TIMEOUT,
};
use crate::images::serve::{ClientAnswer, FirstRecord, MAX_HAVE};
use crate::types::InstanceUuid;
use crate::AppState;

/// Says whether a 200 from `…/image` is the whole view or a delta.
pub const ANSWER_HEADER: &str = "x-lodestone-answer";
/// The current client view's id, on every 200 and 204 from `…/image`.
pub const VIEW_HEADER: &str = "x-lodestone-view";
/// How long a player waits before asking again while the first content image is recorded.
const RECORDING_RETRY_SECS: u64 = 5;
/// How long a player waits before fetching again while too many downloads run.
const BUSY_RETRY_SECS: u64 = 1;
/// How long a player waits before fetching an object again that the store lacks while a
/// record stores it again.
const MISSING_RETRY_SECS: u64 = 5;
/// How long a player waits before fetching again while the store's garbage collection runs.
const COLLECTING_RETRY_SECS: u64 = 5;

/// The query of `…/image`.
#[derive(Debug, Deserialize)]
pub(crate) struct ImageQuery {
    /// Must be `client`: a server view carries the server's configs and secrets.
    #[serde(default)]
    side: Option<String>,
    /// Client view ids the player has, comma-separated, newest first.
    #[serde(default)]
    have: Option<String>,
    /// The newest view rules the player knows (`lodestone_image::RULES`), so a core with
    /// newer rules can tell it to update rather than send a view it can't check.
    #[serde(default)]
    rules: Option<String>,
}

fn bad_request(message: &str) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("{message}"),
    }
}

fn not_found() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Not found"),
    }
}

fn check_side(query: &ImageQuery) -> Result<(), Error> {
    match query.side.as_deref() {
        Some("client") => Ok(()),
        _ => Err(bad_request("Only side=client is served")),
    }
}

/// Checks `rules`: a player that knows only older rules than the core's can't check the
/// views this core makes (`lodestone_image::view` refuses rules newer than its own), so it
/// is told to update instead. Newer rules than the core's are fine: views are made under
/// the core's own, which every later Lodestone still reads.
fn check_rules(text: Option<&str>) -> Result<(), Error> {
    let Some(text) = text else {
        return Ok(());
    };
    let rules: u32 = text
        .parse()
        .ok()
        .filter(|rules| *rules >= 1)
        .ok_or_else(|| bad_request("rules must be a whole number from 1"))?;
    if rules < lodestone_image::RULES {
        return Err(bad_request(
            "This server runs a newer Lodestone: update Lodestone to join it",
        ));
    }
    Ok(())
}

/// The ids in `have`: at most [`MAX_HAVE`], each a SHA-256 in lowercase hex.
fn parse_have(text: Option<&str>) -> Result<Vec<String>, Error> {
    let Some(text) = text.filter(|text| !text.is_empty()) else {
        return Ok(Vec::new());
    };
    let mut ids = Vec::new();
    for id in text.split(',') {
        if ids.len() == MAX_HAVE {
            return Err(bad_request("Too many ids in have"));
        }
        if !is_sha256(id) {
            return Err(bad_request("have must be image ids"));
        }
        ids.push(id.to_string());
    }
    Ok(ids)
}

/// A 503 with the error body and `Retry-After`. Not logged: players poll these every few
/// seconds (the record behind them logs what goes wrong, once).
fn unavailable(kind: ErrorKind, message: &str, retry_after: u64) -> Response {
    let body = Json(Error {
        kind,
        source: eyre!("{message}"),
    });
    let mut response = (StatusCode::SERVICE_UNAVAILABLE, body).into_response();
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(retry_after));
    response
}

/// `Retry-After` for a background record that must wait `retry_after`: at least the usual
/// wait.
fn retry_secs(retry_after: std::time::Duration) -> u64 {
    retry_after
        .as_secs_f64()
        .ceil()
        .max(RECORDING_RETRY_SECS as f64) as u64
}

/// The answer while the instance has no content image: its first is recorded in the
/// background, and the player asks again shortly; or, when the last try failed, later.
async fn recording(state: &AppState, uuid: &InstanceUuid) -> Response {
    let first = match crate::images::context_of(state, uuid).await {
        Some(ctx) => state.images().record_first(ctx),
        None => FirstRecord::Recording,
    };
    match first {
        FirstRecord::Recording => unavailable(
            ErrorKind::Conflict,
            "The server's content is being recorded; try again in a few seconds",
            RECORDING_RETRY_SECS,
        ),
        FirstRecord::Failed { retry_after } => unavailable(
            ErrorKind::Internal,
            "The server's content couldn't be recorded; try again later",
            retry_secs(retry_after),
        ),
    }
}

/// The answer for an object the current view names that the store lacks (gone, found
/// damaged and deleted, or a large file whose chunk list a download took away when it found
/// a chunk lost): a record stores the server's file again in the background, and
/// the player fetches again after it.
async fn missing(state: &AppState, uuid: &InstanceUuid) -> Response {
    let again = match crate::images::context_of(state, uuid).await {
        Some(ctx) => state.images().record_again(ctx),
        None => FirstRecord::Recording,
    };
    match again {
        FirstRecord::Recording => unavailable(
            ErrorKind::Conflict,
            "The server is storing this file again; try again in a few seconds",
            MISSING_RETRY_SECS,
        ),
        FirstRecord::Failed { retry_after } => unavailable(
            ErrorKind::Internal,
            "The server doesn't have this file right now; try again later",
            retry_secs(retry_after),
        ),
    }
}

fn answer_response(answer: ClientAnswer) -> Result<Response, Error> {
    let view = answer.current().clone();
    let builder = Response::builder()
        .header(VIEW_HEADER, view.id())
        .header(ETAG, format!("\"{}\"", view.id()))
        .header(CACHE_CONTROL, "no-cache");
    let response = match answer {
        ClientAnswer::Current(_) => builder.status(StatusCode::NO_CONTENT).body(Body::empty()),
        ClientAnswer::View(view) => builder
            .header(ANSWER_HEADER, "view")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(view.json.clone())),
        ClientAnswer::Delta { json, .. } => builder
            .header(ANSWER_HEADER, "delta")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(json)),
    };
    Ok(response.context("Failed to build the response")?)
}

/// `GET …/image?side=client&have=<id>,<id>`: the current client view, a delta to it from
/// the first `have` the core knows, or 204 when that one is current.
pub(crate) async fn image_response(
    state: &AppState,
    uuid: &InstanceUuid,
    query: ImageQuery,
) -> Result<Response, Error> {
    check_side(&query)?;
    check_rules(query.rules.as_deref())?;
    let have = parse_have(query.have.as_deref())?;
    match state.images().client_answer(uuid, &have).await? {
        Some(answer) => answer_response(answer),
        None => Ok(recording(state, uuid).await),
    }
}

/// What a `Range` header asks of a file of `size` bytes. A file of at most 1 MiB is kept
/// whole and has to be read and checked whole to send any of it, so its ranges are
/// ignored (HTTP allows that): a range of it would cost the core as much as the file.
#[derive(Debug, PartialEq, Eq)]
enum RangeAsk {
    /// No range, or one this doesn't serve (several ranges, another unit, bad syntax): the
    /// whole file, as HTTP allows.
    Whole,
    Part(ByteRange),
    /// A range that starts past the end.
    Unsatisfiable,
}

/// Reads one `bytes=a-b`, `bytes=a-` or `bytes=-n` range (RFC 9110).
fn parse_range(headers: &HeaderMap, size: u64) -> RangeAsk {
    if size <= CHUNKED_ABOVE {
        return RangeAsk::Whole;
    }
    let Some(text) = headers.get(RANGE).and_then(|v| v.to_str().ok()) else {
        return RangeAsk::Whole;
    };
    let Some(spec) = text.trim().strip_prefix("bytes=") else {
        return RangeAsk::Whole;
    };
    if spec.contains(',') {
        return RangeAsk::Whole;
    }
    let Some((first, last)) = spec.trim().split_once('-') else {
        return RangeAsk::Whole;
    };
    let number = |text: &str| -> Option<u64> {
        (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    match (first.trim(), last.trim()) {
        ("", suffix) => match number(suffix) {
            Some(0) => RangeAsk::Unsatisfiable,
            // `size` is over 1 MiB here, never 0
            Some(n) => RangeAsk::Part(ByteRange {
                start: size.saturating_sub(n),
                end: size - 1,
            }),
            None => RangeAsk::Whole,
        },
        (start, end) => {
            let Some(start) = number(start) else {
                return RangeAsk::Whole;
            };
            let end = if end.is_empty() {
                None
            } else {
                match number(end) {
                    Some(end) if end >= start => Some(end),
                    _ => return RangeAsk::Whole,
                }
            };
            if start >= size {
                return RangeAsk::Unsatisfiable;
            }
            RangeAsk::Part(ByteRange {
                start,
                end: end.unwrap_or(u64::MAX).min(size - 1),
            })
        }
    }
}

/// `GET`/`HEAD …/objects/{sha256}`: the bytes of a file the current client view names with
/// source `object` (or the client settings' icon or background), whole or one range.
/// Nothing else in the store is served: it holds worlds, backups and server-only files too.
/// While the instance has no readable current image, the answer is the image route's 503
/// that records one; while a collection runs, a 503 to try again.
/// `client` is who asks, when the route can tell ([`ClientKey`]), for the per-client limit.
pub(crate) async fn object_response(
    state: &AppState,
    uuid: &InstanceUuid,
    sha256: &str,
    method: &Method,
    headers: &HeaderMap,
    client: Option<ClientKey>,
) -> Result<Response, Error> {
    if !is_sha256(sha256) {
        return Err(bad_request("Not a SHA-256"));
    }
    // No readable current image (none yet, or its manifest is damaged) is the image
    // route's 503 that records one, not a 404: a player that holds the view keeps fetching.
    let Some(view) = state.images().client_view(uuid).await? else {
        return Ok(recording(state, uuid).await);
    };
    let Some(size) = view.object_size(sha256) else {
        return Err(not_found());
    };
    // Taken before the store is looked at, so the requests looking at once are bounded by
    // the download limits too (a HEAD gives its place back once it has looked).
    let Some(permit) = state.images().downloads.try_start(uuid, client.as_ref()) else {
        return Ok(unavailable(
            ErrorKind::TooManyRequests,
            "Too many downloads are running; try again in a moment",
            BUSY_RETRY_SECS,
        ));
    };
    let store = state.images().store();
    let sha = sha256.to_string();
    let there = crate::images::blocking(move || {
        object_there(&store, &sha, size)
            .map_err(color_eyre::Report::new)
            .context("Failed to look for the file in the store")
            .map_err(Error::from)
    })
    .await?;
    match there {
        There::Yes => {}
        There::No => {
            drop(permit);
            return Ok(missing(state, uuid).await);
        }
        There::Collecting => {
            return Ok(unavailable(
                ErrorKind::TooManyRequests,
                "The server is tidying its files; try again in a few seconds",
                COLLECTING_RETRY_SECS,
            ))
        }
    }
    let builder = Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(ETAG, format!("\"{sha256}\""))
        .header(ACCEPT_RANGES, "bytes")
        .header(CACHE_CONTROL, "private, max-age=31536000, immutable");
    let (builder, range, length) = match parse_range(headers, size) {
        RangeAsk::Whole => (builder.status(StatusCode::OK), None, size),
        RangeAsk::Part(range) => (
            builder.status(StatusCode::PARTIAL_CONTENT).header(
                CONTENT_RANGE,
                format!("bytes {}-{}/{size}", range.start, range.end),
            ),
            Some(range),
            range.end - range.start + 1,
        ),
        RangeAsk::Unsatisfiable => {
            return Ok(builder
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(CONTENT_RANGE, format!("bytes */{size}"))
                .body(Body::empty())
                .context("Failed to build the response")?)
        }
    };
    let builder = builder.header(CONTENT_LENGTH, length);
    if method == Method::HEAD {
        return Ok(builder
            .body(Body::empty())
            .context("Failed to build the response")?);
    }
    let stream = object_stream(
        state.images().store(),
        sha256.to_string(),
        size,
        range,
        permit,
        IDLE_TIMEOUT,
    );
    Ok(builder
        .body(Body::from_stream(stream))
        .context("Failed to build the response")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(text: &str, size: u64) -> RangeAsk {
        let mut headers = HeaderMap::new();
        headers.insert(RANGE, HeaderValue::from_str(text).unwrap());
        parse_range(&headers, size)
    }

    fn part(start: u64, end: u64) -> RangeAsk {
        RangeAsk::Part(ByteRange { start, end })
    }

    #[test]
    fn ranges() {
        // a small file is sent whole, whatever the range
        assert_eq!(range("bytes=0-4", 10), RangeAsk::Whole);
        assert_eq!(range("bytes=-1", CHUNKED_ABOVE), RangeAsk::Whole);
        assert_eq!(range("bytes=-1", 0), RangeAsk::Whole);
        // past that, one range is served
        let n = CHUNKED_ABOVE + 10;
        let last = n - 1;
        assert_eq!(parse_range(&HeaderMap::new(), n), RangeAsk::Whole);
        assert_eq!(range("bytes=0-4", n), part(0, 4));
        assert_eq!(range("bytes=5-", n), part(5, last));
        assert_eq!(range(&format!("bytes=5-{}", n + 90), n), part(5, last));
        assert_eq!(range("bytes=-3", n), part(n - 3, last));
        assert_eq!(range(&format!("bytes=-{}", n + 20), n), part(0, last));
        assert_eq!(range(&format!("bytes={last}-{last}"), n), part(last, last));
        assert_eq!(range(&format!("bytes={n}-"), n), RangeAsk::Unsatisfiable);
        assert_eq!(range("bytes=-0", n), RangeAsk::Unsatisfiable);
        // what isn't served is ignored: the whole file
        assert_eq!(range("bytes=0-1,3-4", n), RangeAsk::Whole);
        assert_eq!(range("items=0-1", n), RangeAsk::Whole);
        assert_eq!(range("bytes=4-2", n), RangeAsk::Whole);
        assert_eq!(range("bytes=a-2", n), RangeAsk::Whole);
        assert_eq!(range("bytes=+1-2", n), RangeAsk::Whole);
        assert_eq!(range("bytes=-", n), RangeAsk::Whole);
        assert_eq!(range("bytes=1", n), RangeAsk::Whole);
        assert_eq!(range("bytes=99999999999999999999999-", n), RangeAsk::Whole);
    }

    #[test]
    fn rules_are_checked() {
        assert!(check_rules(None).is_ok());
        assert!(check_rules(Some(&lodestone_image::RULES.to_string())).is_ok());
        assert!(check_rules(Some(&(lodestone_image::RULES + 1).to_string())).is_ok());
        for bad in ["0", "", "-1", "x", "1.0", "99999999999"] {
            assert!(check_rules(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn have_lists() {
        let id = "a".repeat(64);
        assert_eq!(parse_have(None).unwrap(), Vec::<String>::new());
        assert_eq!(parse_have(Some("")).unwrap(), Vec::<String>::new());
        assert_eq!(parse_have(Some(&id)).unwrap(), vec![id.clone()]);
        let many = vec![id.as_str(); MAX_HAVE].join(",");
        assert_eq!(parse_have(Some(&many)).unwrap().len(), MAX_HAVE);
        let too_many = vec![id.as_str(); MAX_HAVE + 1].join(",");
        assert!(parse_have(Some(&too_many)).is_err());
        for bad in [
            "x".repeat(64),
            "A".repeat(64),
            "a".repeat(63),
            format!("{id},"),
            format!(",{id}"),
            format!("{id}, {id}"),
            "../etc".to_string(),
        ] {
            assert!(parse_have(Some(&bad)).is_err(), "{bad}");
        }
    }
}
