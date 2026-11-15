//! A server's image as its players reach it (`docs/instance-images.md`, "Server to
//! player"): `GET …/image?side=client&have=…&rules=…`, the client view of the server's
//! current content image or a delta to it, and `GET …/objects/{sha256}`, the bytes of a
//! file only the server has.
//!
//! Both routes are reached every way the Launcher reaches a server: signed in, through
//! `/launcher/instances/{uuid}/` with this device's key (or a claimed core's owner key),
//! and without, through `/public/instances/{uuid}/` on the core's own port (the public
//! list), a Connect invite's info port or a friend's tunnel (whose [`CoreClient`] is
//! built on the forwarded port). Every body is read as a stream, through the core's
//! pinned certificate like any other request, with no deadline for the whole request: a
//! view of a big pack passes 256 KiB and an object is a whole file. Instead each wait
//! (for the answer's headers, and between two reads of its body) has its own limit
//! ([`Limits`]), generous for objects, since a download on the server pauses while its
//! store's garbage collection runs.
//!
//! Nothing here trusts the server: bodies are bounded (a view or delta by
//! [`MAX_MANIFEST_BYTES`], an object by the size its view gives), every `Retry-After` is
//! clamped and every wait counted against a limit, and an object's SHA-256 is checked
//! before it is used. What a view or delta *says* is checked by
//! [`super::image_sync::received`].

use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures::{AsyncReadExt as _, StreamExt as _, TryStreamExt as _, future::BoxFuture};
use http_client::{AsyncBody, HttpRequestExt as _, Method, RedirectPolicy, Request};
use lodestone_image::{
    Delta, Image, ManifestError, RULES, hex::is_sha256, store::MAX_MANIFEST_BYTES,
    store::chunklist::CHUNKED_ABOVE,
};

use super::{
    CoreClient, CoreError,
    client::{Auth, RawResponse, check, segment},
};
use crate::{
    Secret, USER_AGENT,
    game::download::{CancelToken, Progress, Sleep},
};

/// Says whether a 200 from `…/image` is the whole view or a delta.
pub const ANSWER_HEADER: &str = "x-lodestone-answer";
/// The current client view's id, on every 200 and 204 from `…/image`.
pub const VIEW_HEADER: &str = "x-lodestone-view";
/// The core's view rules, on every answer of `…/image` past the access checks.
pub const RULES_HEADER: &str = "x-lodestone-rules";
/// At most this many view ids in `have` (the core refuses more).
pub const MAX_HAVE: usize = 16;
/// Objects are fetched this many at a time (the spec asks players for at most 4).
pub const MAX_PARALLEL_OBJECTS: usize = 4;
/// A file is given up on after this many failed fetches: a body cut short, one that
/// doesn't hash to its name, a stall, a server or transport error.
pub const MAX_FAILURES: u32 = 3;
/// The longest a 503's `Retry-After` is waited, whatever the server says.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);
/// An error body is read only up to this.
const MAX_ERROR_BYTES: u64 = 64 * 1024;
/// The wait after a failed fetch of a file, before the next.
const RETRY_PAUSE: Duration = Duration::from_secs(1);

/// How long each wait may take, and how large a view may be. The defaults are the
/// spec's; tests shorten them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// For `…/image`'s headers, and between two reads of its body.
    pub image_idle: Duration,
    /// For an object's headers, and between two reads of its body: minutes, since a
    /// download pauses while the server's store is collected.
    pub object_idle: Duration,
    /// `…/image` asks again through `Conflict` answers (the server records its first
    /// content image, which for a big server takes minutes) until its waits add up to
    /// this.
    pub preparing: Duration,
    /// An object asks again through `Conflict` answers (the server stores a file it lost
    /// again) until its waits add up to this; then the file is given up on.
    pub conflict: Duration,
    /// An object asks again through `TooManyRequests` answers until its waits add up to
    /// this.
    pub busy: Duration,
    /// A view or delta larger than this is refused: [`MAX_MANIFEST_BYTES`].
    pub max_answer: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            image_idle: Duration::from_secs(120),
            object_idle: Duration::from_secs(300),
            preparing: Duration::from_secs(600),
            conflict: Duration::from_secs(120),
            busy: Duration::from_secs(600),
            max_answer: MAX_MANIFEST_BYTES,
        }
    }
}

/// The answer to `GET …/image`, read but not yet checked
/// ([`super::image_sync::received`] does that).
#[derive(Clone, Debug)]
pub enum ImageAnswer {
    /// 204: the view `view` (one of `have`) is current.
    Current { view: String },
    /// 200 with the whole view; `view` is what `X-Lodestone-View` named.
    View { image: Box<Image>, view: String },
    /// 200 with a delta from the `have` view its `from` names to the view `view`.
    Delta { delta: Box<Delta>, view: String },
}

impl ImageAnswer {
    /// The current view's id, as the server named it.
    pub fn view(&self) -> &str {
        match self {
            ImageAnswer::Current { view }
            | ImageAnswer::View { view, .. }
            | ImageAnswer::Delta { view, .. } => view,
        }
    }
}

/// Why a server's image couldn't be had. The messages are for people.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// 404 on `…/image`: a core older than milestone 3, which serves only the `.mrpack`
    /// client pack (or a server this core doesn't serve, which the `.mrpack` route says).
    #[error("The host runs an older Lodestone, which doesn't serve server images.")]
    OlderCore,
    /// The server makes views this Launcher can't check: its rules or schema are newer.
    #[error("This server runs a newer Lodestone. Update {app} to join it.", app = crate::wording::app_name())]
    UpdateLodestone,
    /// 503 `Internal`: the server couldn't record its content; asking again before
    /// `retry_after` seconds gets the same answer.
    #[error("The server couldn't prepare its files. Try again in a few minutes.")]
    PrepareFailed { retry_after: u64 },
    /// 503 `Conflict` for longer than [`Limits::preparing`].
    #[error("The server is still preparing its files. Try again in a few minutes.")]
    StillPreparing,
    /// A file couldn't be had from the server after every try.
    #[error("The server couldn't send {path}. Try again later.")]
    FileFailed { path: String },
    /// An object the view named isn't served any more: the server's view changed. Ask for
    /// the image again.
    #[error("The server's files changed while they downloaded. Try again.")]
    ViewChanged,
    #[error(transparent)]
    Core(#[from] CoreError),
    /// The server sent something no Lodestone would: refused.
    #[error("The server sent files {app} can't use: {0}", app = crate::wording::app_name())]
    Invalid(String),
    #[error("{0}")]
    Io(String),
    #[error("cancelled")]
    Cancelled,
}

/// A server's image as one player reaches it: the core (or a friend's forwarded port),
/// the key to sign in with if any, and the server. Cheap to clone.
#[derive(Clone)]
pub struct ServerImage {
    client: CoreClient,
    key: Option<Secret>,
    server: String,
    sleep: Sleep,
    limits: Limits,
}

/// Waits with Tokio's timer, inside the HTTP client's runtime.
pub fn tokio_sleep() -> Sleep {
    Arc::new(|duration| {
        Box::pin(crate::http::in_http_runtime(async move {
            tokio::time::sleep(duration).await
        }))
    })
}

impl ServerImage {
    /// The image of the server `server` (its uuid) on `client`'s core: signed in with
    /// `key` (`/launcher/…`), or through `/public/…` without one (the public list, an
    /// invite's info port, a friend's tunnel).
    pub fn new(client: CoreClient, key: Option<Secret>, server: &str) -> Result<Self, ImageError> {
        Ok(Self {
            client,
            key,
            server: segment(server)?.to_string(),
            sleep: tokio_sleep(),
            limits: Limits::default(),
        })
    }

    /// Waits with `sleep` (tests record the waits and return at once).
    pub fn with_sleep(mut self, sleep: Sleep) -> Self {
        self.sleep = sleep;
        self
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    fn auth(&self) -> Auth<'_> {
        match &self.key {
            Some(key) => Auth::Bearer(key),
            None => Auth::None,
        }
    }

    fn path(&self, rest: &str) -> String {
        let prefix = match self.key {
            Some(_) => "launcher",
            None => "public",
        };
        format!("{prefix}/instances/{}/{rest}", self.server)
    }

    /// Sends a GET with no deadline for the whole request, waiting at most `idle` for the
    /// answer's headers. The body is read by the caller, each read under `idle` too.
    async fn get(
        &self,
        path: &str,
        accept: &str,
        range: Option<u64>,
        idle: Duration,
    ) -> Result<http_client::Response<AsyncBody>, CoreError> {
        let url = self.client.url(path);
        let auth = self.auth();
        let request = || {
            let mut builder = Request::builder()
                .method(Method::GET)
                .uri(url.as_str())
                .header("User-Agent", USER_AGENT)
                .header("Accept", accept)
                .follow_redirects(RedirectPolicy::NoFollow);
            if let Auth::Bearer(key) = auth {
                builder = builder.header("Authorization", format!("Bearer {}", key.expose()));
            }
            if let Some(from) = range {
                builder = builder.header("Range", format!("bytes={from}-"));
            }
            builder
                .body(AsyncBody::empty())
                .map_err(|err| CoreError::InvalidAddress(err.to_string()))
        };
        let started = std::time::Instant::now();
        // the query (the view ids a player has) isn't logged
        let traced = path.split('?').next().unwrap_or_default().to_string();
        let sent = crate::http::in_http_runtime(tokio::time::timeout(
            idle,
            self.client.send_checked(request),
        ))
        .await;
        let result = match sent {
            Ok(result) => result,
            Err(_) => Err(CoreError::Unreachable(format!(
                "no answer within {} seconds",
                idle.as_secs()
            ))),
        };
        let ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(response) => crate::trace::emit("core_http.response", || {
                serde_json::json!({
                    "method": "GET", "path": traced,
                    "status": response.status().as_u16(), "ms": ms,
                })
            }),
            Err(err) => crate::trace::emit("core_http.error", || {
                serde_json::json!({
                    "method": "GET", "path": traced, "error": err.to_string(), "ms": ms,
                })
            }),
        }
        result
    }

    /// `GET …/image?side=client&have=…&rules=…`: whether one of the views the player has
    /// (`have`, applied view first, at most [`MAX_HAVE`]) is current, or a delta from one,
    /// or the whole view. Asks again through `Conflict` while the server prepares its
    /// files (calling `on_preparing` each time), within [`Limits::preparing`].
    pub async fn image(
        &self,
        have: &[String],
        on_preparing: &(dyn Fn() + Send + Sync),
        cancel: &CancelToken,
    ) -> Result<ImageAnswer, ImageError> {
        let have: Vec<&String> = have
            .iter()
            .filter(|id| is_sha256(id))
            .take(MAX_HAVE)
            .collect();
        let mut path = format!("image?side=client&rules={RULES}");
        if !have.is_empty() {
            let ids: Vec<&str> = have.iter().map(|id| id.as_str()).collect();
            path.push_str(&format!("&have={}", ids.join(",")));
        }
        let path = self.path(&path);
        let mut waited = Duration::ZERO;
        loop {
            if cancel.is_cancelled() {
                return Err(ImageError::Cancelled);
            }
            let idle = self.limits.image_idle;
            let response = self.get(&path, "application/json", None, idle).await?;
            let headers = Headers::of(&response);
            if headers.rules_newer() {
                return Err(ImageError::UpdateLodestone);
            }
            let status = response.status().as_u16();
            let mut body = response.into_body();
            match status {
                204 => {
                    let view = headers.view()?;
                    if !have.iter().any(|id| **id == view) {
                        return Err(ImageError::Invalid(
                            "it says a view is current that this Launcher doesn't have".into(),
                        ));
                    }
                    return Ok(ImageAnswer::Current { view });
                }
                200 => {
                    let view = headers.view()?;
                    if !headers.is_json() {
                        return Err(ImageError::Invalid("its answer isn't JSON".into()));
                    }
                    let max = self.limits.max_answer;
                    if headers.length.is_some_and(|length| length > max) {
                        return Err(too_large(max));
                    }
                    let bytes = read_bounded(&mut body, max, idle, cancel).await?;
                    return match headers.answer.as_deref() {
                        Some("view") => match Image::from_json(&bytes) {
                            Ok(image) => Ok(ImageAnswer::View {
                                image: Box::new(image),
                                view,
                            }),
                            Err(ManifestError::UnsupportedSchema { .. }) => {
                                Err(ImageError::UpdateLodestone)
                            }
                            Err(err) => Err(ImageError::Invalid(err.to_string())),
                        },
                        Some("delta") => match Delta::from_json(&bytes) {
                            Ok(delta) => Ok(ImageAnswer::Delta {
                                delta: Box::new(delta),
                                view,
                            }),
                            Err(err) => Err(ImageError::Invalid(err.to_string())),
                        },
                        _ => Err(ImageError::Invalid(
                            "it doesn't say whether it sent a view or a delta".into(),
                        )),
                    };
                }
                503 => {
                    let unavailable = read_unavailable(&headers, &mut body, idle).await;
                    match unavailable.kind.as_deref() {
                        Some("Internal") => {
                            return Err(ImageError::PrepareFailed {
                                retry_after: unavailable.retry_after.as_secs(),
                            });
                        }
                        _ => {
                            // Conflict: the first content image is being recorded
                            on_preparing();
                            let wait = unavailable.retry_after;
                            waited += wait;
                            if waited > self.limits.preparing {
                                return Err(ImageError::StillPreparing);
                            }
                            (self.sleep)(wait).await;
                        }
                    }
                }
                404 => return Err(ImageError::OlderCore),
                _ => return Err(error_status(status, &mut body, idle, self.auth()).await),
            }
        }
    }

    /// `GET …/objects/{sha256}` into `dest` (a file in a staging folder, which the caller
    /// throws away if anything fails): the file whose SHA-256 is `sha256` and whose size
    /// the view gives as `size`, checked. A partial file at `dest` of a large file (over
    /// 1 MiB) is resumed with `Range`. `path` is the file's path in the game folder, for
    /// messages.
    pub async fn object(
        &self,
        sha256: &str,
        size: u64,
        dest: &Path,
        path: &str,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<(), ImageError> {
        if !is_sha256(sha256) {
            return Err(ImageError::Invalid(format!("{path} has no SHA-256")));
        }
        let url_path = self.path(&format!("objects/{sha256}"));
        let idle = self.limits.object_idle;
        let failed = || ImageError::FileFailed {
            path: lodestone_image::display_text(path).into_owned(),
        };
        let mut failures = 0u32;
        let mut conflict_waited = Duration::ZERO;
        let mut busy_waited = Duration::ZERO;
        // bytes of this file counted in `progress`, taken back if it fails
        let mut counted = 0u64;
        let result = loop {
            if cancel.is_cancelled() {
                break Err(ImageError::Cancelled);
            }
            let outcome = self
                .fetch_once(
                    &url_path,
                    sha256,
                    size,
                    dest,
                    idle,
                    progress,
                    &mut counted,
                    cancel,
                )
                .await;
            let failure = match outcome {
                Fetched::Done => break Ok(()),
                Fetched::Fatal(err) => break Err(err),
                Fetched::Busy(wait) => {
                    busy_waited += wait;
                    if busy_waited > self.limits.busy {
                        break Err(failed());
                    }
                    (self.sleep)(wait).await;
                    continue;
                }
                Fetched::Conflict(wait) => {
                    conflict_waited += wait;
                    if conflict_waited > self.limits.conflict {
                        break Err(failed());
                    }
                    (self.sleep)(wait).await;
                    continue;
                }
                Fetched::Failed(wait) => wait,
            };
            failures += 1;
            if failures >= MAX_FAILURES {
                break Err(failed());
            }
            (self.sleep)(failure.unwrap_or(RETRY_PAUSE)).await;
        };
        match &result {
            Ok(()) => {
                progress.sub_bytes(counted);
                progress.add_bytes(size);
                progress.file_done();
            }
            Err(_) => {
                progress.sub_bytes(counted);
                let _ = fs::remove_file(dest);
            }
        }
        result
    }

    /// One fetch of an object: what it came to.
    #[allow(clippy::too_many_arguments)]
    async fn fetch_once(
        &self,
        url_path: &str,
        sha256: &str,
        size: u64,
        dest: &Path,
        idle: Duration,
        progress: &Progress,
        counted: &mut u64,
        cancel: &CancelToken,
    ) -> Fetched {
        // Only a large file is resumed: the core sends a small one whole anyway.
        let have = match fs::symlink_metadata(dest) {
            Ok(meta) if meta.is_file() && size > CHUNKED_ABOVE && meta.len() < size => meta.len(),
            Ok(_) => {
                let _ = fs::remove_file(dest);
                0
            }
            Err(_) => 0,
        };
        let range = (have > 0).then_some(have);
        let response = match self
            .get(url_path, "application/octet-stream", range, idle)
            .await
        {
            Ok(response) => response,
            Err(err) if err.is_offline() => return Fetched::Failed(None),
            Err(err) => return Fetched::Fatal(err.into()),
        };
        let headers = Headers::of(&response);
        let status = response.status().as_u16();
        let mut body = response.into_body();
        let append = match status {
            200 => {
                if headers.length.is_some_and(|length| length != size) {
                    return Fetched::Failed(None);
                }
                false
            }
            206 if have > 0 => {
                let expected = format!("bytes {have}-{}/{size}", size - 1);
                if headers.content_range.as_deref() != Some(expected.as_str()) {
                    let _ = fs::remove_file(dest);
                    return Fetched::Failed(None);
                }
                true
            }
            416 => {
                let _ = fs::remove_file(dest);
                return Fetched::Failed(None);
            }
            503 => {
                let unavailable = read_unavailable(&headers, &mut body, idle).await;
                return match unavailable.kind.as_deref() {
                    Some("TooManyRequests") => Fetched::Busy(unavailable.retry_after),
                    Some("Conflict") => Fetched::Conflict(unavailable.retry_after),
                    // Internal: counted like any failure, so three in a row give up
                    _ => Fetched::Failed(Some(unavailable.retry_after)),
                };
            }
            404 => return Fetched::Fatal(ImageError::ViewChanged),
            400 => {
                return Fetched::Fatal(ImageError::Invalid(
                    "the server refused to send a file its view names".into(),
                ));
            }
            500..=599 => return Fetched::Failed(None),
            other => {
                return Fetched::Fatal(error_status(other, &mut body, idle, self.auth()).await);
            }
        };
        let file = if append {
            OpenOptions::new().append(true).open(dest)
        } else {
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            File::create(dest)
        };
        let mut file = match file {
            Ok(file) => file,
            Err(err) => return Fetched::Fatal(io_error(dest, err)),
        };
        let mut written = if append { have } else { 0 };
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            if cancel.is_cancelled() {
                return Fetched::Fatal(ImageError::Cancelled);
            }
            let read =
                crate::http::in_http_runtime(tokio::time::timeout(idle, body.read(&mut buffer)))
                    .await;
            let read = match read {
                Ok(Ok(read)) => read,
                // a stall or a connection cut: what came is kept, to resume
                Ok(Err(_)) | Err(_) => return Fetched::Failed(None),
            };
            if read == 0 {
                break;
            }
            if written + read as u64 > size {
                // more than the file: not the file
                drop(file);
                let _ = fs::remove_file(dest);
                return Fetched::Failed(None);
            }
            if let Err(err) = file.write_all(&buffer[..read]) {
                return Fetched::Fatal(io_error(dest, err));
            }
            written += read as u64;
            *counted += read as u64;
            progress.add_bytes(read as u64);
        }
        if written < size {
            // cut short: the next fetch resumes a large file
            return Fetched::Failed(None);
        }
        if let Err(err) = file.sync_all() {
            return Fetched::Fatal(io_error(dest, err));
        }
        drop(file);
        match sha256_of_file(dest) {
            Ok(actual) if actual == sha256 => Fetched::Done,
            Ok(_) => {
                let _ = fs::remove_file(dest);
                Fetched::Failed(None)
            }
            Err(err) => Fetched::Fatal(io_error(dest, err)),
        }
    }

    /// Fetches every object in `items` (SHA-256, size, destination, path in the game
    /// folder), [`MAX_PARALLEL_OBJECTS`] at a time; the first that fails ends the rest.
    pub async fn objects(
        &self,
        items: &[ObjectItem],
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<(), ImageError> {
        let fetches: Vec<BoxFuture<'_, Result<(), ImageError>>> = items
            .iter()
            .map(|item| {
                Box::pin(self.object(
                    &item.sha256,
                    item.size,
                    &item.dest,
                    &item.path,
                    progress,
                    cancel,
                )) as BoxFuture<'_, _>
            })
            .collect();
        futures::stream::iter(fetches)
            .buffer_unordered(MAX_PARALLEL_OBJECTS)
            .try_collect::<Vec<()>>()
            .await?;
        Ok(())
    }
}

/// One object to fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectItem {
    pub sha256: String,
    pub size: u64,
    pub dest: PathBuf,
    /// Its path in the game folder, for messages.
    pub path: String,
}

/// What one fetch of an object came to.
enum Fetched {
    Done,
    /// Counts against [`MAX_FAILURES`]; waits the given time (else a moment) first.
    Failed(Option<Duration>),
    /// 503 `TooManyRequests`: wait, uncounted.
    Busy(Duration),
    /// 503 `Conflict`: wait, against [`Limits::conflict`].
    Conflict(Duration),
    Fatal(ImageError),
}

/// The headers of an answer this reads.
struct Headers {
    status: u16,
    rules: Option<String>,
    view: Option<String>,
    answer: Option<String>,
    content_type: Option<String>,
    length: Option<u64>,
    content_range: Option<String>,
    retry_after: Option<String>,
}

impl Headers {
    fn of(response: &http_client::Response<AsyncBody>) -> Self {
        let get = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.trim().to_string())
        };
        Self {
            status: response.status().as_u16(),
            rules: get(RULES_HEADER),
            view: get(VIEW_HEADER),
            answer: get(ANSWER_HEADER),
            content_type: get("content-type"),
            length: get("content-length").and_then(|v| v.parse().ok()),
            content_range: get("content-range"),
            retry_after: get("retry-after"),
        }
    }

    /// Whether the core's rules are newer than this Launcher's. Digits only; a number too
    /// large for 32 bits is newer than any. Anything else isn't the header the core sends,
    /// and says nothing.
    fn rules_newer(&self) -> bool {
        rules_newer(self.rules.as_deref())
    }

    fn view(&self) -> Result<String, ImageError> {
        match self.view.as_deref() {
            Some(view) if is_sha256(view) => Ok(view.to_string()),
            _ => Err(ImageError::Invalid(format!(
                "its answer ({}) doesn't name the current view",
                self.status
            ))),
        }
    }

    fn is_json(&self) -> bool {
        self.content_type.as_deref().is_some_and(|t| {
            t.split(';')
                .next()
                .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
        })
    }

    /// `Retry-After` in seconds, clamped to [`MAX_RETRY_AFTER`]; a missing or odd one is
    /// the spec's usual 5 seconds.
    fn retry_after(&self) -> Duration {
        let seconds = self
            .retry_after
            .as_deref()
            .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
            .map(|v| v.parse::<u64>().unwrap_or(u64::MAX))
            .unwrap_or(5);
        Duration::from_secs(seconds).min(MAX_RETRY_AFTER)
    }
}

/// Whether a `X-Lodestone-Rules` value names rules newer than [`RULES`].
pub fn rules_newer(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return false;
    };
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    value.parse::<u32>().map_or(true, |rules| rules > RULES)
}

/// A 503's kind and wait.
struct Unavailable {
    kind: Option<String>,
    retry_after: Duration,
}

/// Reads a 503's JSON body (only when it says it is JSON, and only so much of it) for its
/// `kind`.
async fn read_unavailable(headers: &Headers, body: &mut AsyncBody, idle: Duration) -> Unavailable {
    let mut kind = None;
    if headers.is_json() {
        if let Ok(bytes) = read_bounded(body, MAX_ERROR_BYTES, idle, &CancelToken::new()).await {
            kind = serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string));
        }
    }
    Unavailable {
        kind,
        retry_after: headers.retry_after(),
    }
}

/// An answer that is an error: the core's error, from its JSON body.
async fn error_status(
    status: u16,
    body: &mut AsyncBody,
    idle: Duration,
    auth: Auth<'_>,
) -> ImageError {
    let text = read_bounded(body, MAX_ERROR_BYTES, idle, &CancelToken::new())
        .await
        .unwrap_or_default();
    match check(
        &RawResponse {
            status,
            etag: None,
            body: text,
        },
        auth,
    ) {
        Err(err) => ImageError::Core(err),
        Ok(()) => ImageError::Invalid(format!("it answered {status}")),
    }
}

fn too_large(limit: u64) -> ImageError {
    ImageError::Invalid(format!("its answer is larger than {limit} bytes"))
}

/// Reads `body` whole, refusing it past `limit` bytes, each read within `idle`.
async fn read_bounded(
    body: &mut AsyncBody,
    limit: u64,
    idle: Duration,
    cancel: &CancelToken,
) -> Result<Vec<u8>, ImageError> {
    let mut bytes = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        if cancel.is_cancelled() {
            return Err(ImageError::Cancelled);
        }
        let read =
            crate::http::in_http_runtime(tokio::time::timeout(idle, body.read(&mut buffer))).await;
        let read = match read {
            Ok(Ok(read)) => read,
            Ok(Err(err)) => return Err(CoreError::Unreachable(err.to_string()).into()),
            Err(_) => {
                return Err(CoreError::Unreachable(format!(
                    "the server sent nothing for {} seconds",
                    idle.as_secs()
                ))
                .into());
            }
        };
        if read == 0 {
            return Ok(bytes);
        }
        if bytes.len() as u64 + read as u64 > limit {
            return Err(too_large(limit));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

fn io_error(path: &Path, err: std::io::Error) -> ImageError {
    ImageError::Io(format!("{}: {err}", path.display()))
}

/// The SHA-256 of a file, with ring through the shared crate.
pub(crate) fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = lodestone_image::hex::Sha256Hasher::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finish_hex());
        }
        hasher.update(&buffer[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_rules_are_told_apart() {
        assert!(!rules_newer(None));
        assert!(!rules_newer(Some("1")));
        assert!(!rules_newer(Some("0")));
        assert!(rules_newer(Some("2")));
        assert!(rules_newer(Some("99999999999999999999")));
        // not the core's header: says nothing
        assert!(!rules_newer(Some("+2")));
        assert!(!rules_newer(Some("two")));
        assert!(!rules_newer(Some("")));
    }
}
