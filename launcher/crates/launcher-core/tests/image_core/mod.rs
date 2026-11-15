//! A fake core that serves a server's image to players as milestone 3's core does
//! (`core/src/handlers/player_image.rs`, `core/src/images/serve.rs`): `…/image` answers
//! computed with `lodestone_image`'s own `view` and `diff` from a server content image,
//! and `…/objects/{sha256}` from the bytes of its `object` entries, with the core's
//! headers. Scripted faults stand in for everything a real or hostile server may do.
//! Each test starts its own, on a loopback port, answering each request on a thread of
//! its own (so stalled answers and parallel downloads behave as they would).

#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use lodestone_image::{
    CanonicalPath, Entry, Game, Image, Kind, Loader, RULES, Side, Source, diff, hex::sha256_hex,
    view,
};

pub const KEY: &str = "lsk_imagecore_KEY-SECRET";
pub const SERVER: &str = "inst-1";

/// What the next answer of a route does instead of the usual.
#[derive(Clone, Debug)]
pub enum Fault {
    /// `…/image`: a 503 of this kind (`Conflict`, `Internal`, `TooManyRequests`) with
    /// this `Retry-After`.
    ImageUnavailable(&'static str, u64),
    /// `…/image`: 404, as a core before milestone 3.
    ImageNotFound,
    /// `…/image`: 400 `BadRequest` with `X-Lodestone-Rules: 2`, as a newer core.
    NewerRules,
    /// `…/image`: 200 whose body never ends.
    Endless,
    /// `…/image`: 200 with a body that says it is this large.
    DeclaredLength(u64),
    /// `…/image`: the usual answer as plain text.
    NotJson,
    /// `…/image`: the usual answer naming another view as current.
    WrongView(String),
    /// Any route: exactly this.
    Raw {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    /// `…/objects`: a 503 of this kind with this `Retry-After`.
    ObjectUnavailable(&'static str, u64),
    /// `…/objects`: only the first this many bytes, then the body ends.
    Short(usize),
    /// `…/objects`: the right length of the wrong bytes.
    WrongBytes,
    /// `…/objects`: half the file, then nothing for this long.
    Stall(Duration),
    /// `…/objects`: this status, no body.
    ObjectStatus(u16),
    /// `…/objects`: the file, ignoring a `Range`.
    IgnoreRange,
}

impl Fault {
    fn for_image(&self) -> bool {
        matches!(
            self,
            Fault::ImageUnavailable(..)
                | Fault::ImageNotFound
                | Fault::NewerRules
                | Fault::Endless
                | Fault::DeclaredLength(_)
                | Fault::NotJson
                | Fault::WrongView(_)
        )
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub path: String,
    pub authorization: Option<String>,
    pub range: Option<String>,
}

#[derive(Default)]
pub struct State {
    /// The server's content images, oldest first; the last is current.
    pub images: Vec<Image>,
    /// The bytes of every object entry, by SHA-256.
    pub objects: HashMap<String, Vec<u8>>,
    /// Faults for the next requests, in order: each is taken by the first request of its
    /// route (`Raw` by whichever comes first).
    pub faults: VecDeque<Fault>,
    pub requests: Vec<Request>,
    /// How long each object answer takes, to see downloads overlap.
    pub object_delay: Duration,
    /// The core's `.mrpack` client pack, for the fallback (`None`: 400 "no mods").
    pub pack: Option<Vec<u8>>,
}

pub struct ImageCore {
    pub base: url::Url,
    pub state: Arc<Mutex<State>>,
    /// Object downloads running now, and the most at once.
    pub running: Arc<AtomicUsize>,
    pub most: Arc<AtomicUsize>,
    server: Arc<tiny_http::Server>,
}

impl Drop for ImageCore {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

impl ImageCore {
    pub fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let running = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        {
            let (server, state, running, most) =
                (server.clone(), state.clone(), running.clone(), most.clone());
            std::thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    let (state, running, most) = (state.clone(), running.clone(), most.clone());
                    std::thread::spawn(move || answer(request, &state, &running, &most));
                }
            });
        }
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}")).unwrap(),
            state,
            running,
            most,
            server,
        }
    }

    pub fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Makes `image` the server's current content image (sealed here), keeping the earlier
    /// ones, and serves the bytes of its `object` entries from `files`.
    pub fn publish(&self, image: &Image, files: &[(&str, &[u8])]) {
        let mut image = image.clone();
        image.seal();
        let mut state = self.state();
        for (_, bytes) in files {
            state.objects.insert(sha256_hex(bytes), bytes.to_vec());
        }
        state.images.push(image);
    }

    /// The client view of the current content image.
    pub fn current_view(&self) -> Image {
        let state = self.state();
        view(state.images.last().unwrap(), Side::Client, RULES).unwrap()
    }

    pub fn fault(&self, fault: Fault) {
        self.state().faults.push_back(fault);
    }

    pub fn requests(&self) -> Vec<Request> {
        self.state().requests.clone()
    }

    /// The paths asked for, without queries.
    pub fn paths(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|r| r.path.split('?').next().unwrap_or_default().to_string())
            .collect()
    }
}

/// A content image of a Fabric server, for tests to fill.
pub fn content_image(minecraft: &str, loader: Loader, loader_version: Option<&str>) -> Image {
    Image::new(
        Kind::Content,
        Game {
            minecraft: minecraft.into(),
            loader,
            loader_version: loader_version.map(str::to_string),
        },
    )
}

/// An `object` entry for `bytes` at `path`.
pub fn object_entry(path: &str, bytes: &[u8]) -> Entry {
    Entry::new(
        CanonicalPath::new(path).unwrap(),
        bytes.len() as u64,
        sha256_hex(bytes),
        Source::Object {},
    )
}

type Answer = (u16, Vec<(String, String)>, Body);

enum Body {
    Bytes(Vec<u8>),
    /// A reader with the length to declare, if any.
    Reader(Box<dyn Read + Send>, Option<usize>),
}

fn header(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.to_string())
}

fn answer(
    request: tiny_http::Request,
    state: &Mutex<State>,
    running: &AtomicUsize,
    most: &AtomicUsize,
) {
    let recorded = Request {
        path: request.url().to_string(),
        authorization: header(&request, "Authorization"),
        range: header(&request, "Range"),
    };
    let is_object = recorded.path.contains("/objects/");
    let _counted = is_object.then(|| Running::start(running, most));
    let (status, headers, body) = {
        let mut state = state.lock().unwrap();
        state.requests.push(recorded.clone());
        route(&mut state, &recorded)
    };
    if is_object {
        let delay = state.lock().unwrap().object_delay;
        std::thread::sleep(delay);
    }
    let headers: Vec<tiny_http::Header> = headers
        .into_iter()
        .map(|(n, v)| tiny_http::Header::from_str(&format!("{n}: {v}")).unwrap())
        .collect();
    let status = tiny_http::StatusCode(status);
    let _ = match body {
        Body::Bytes(bytes) => {
            let length = bytes.len();
            request.respond(tiny_http::Response::new(
                status,
                headers,
                std::io::Cursor::new(bytes),
                Some(length),
                None,
            ))
        }
        Body::Reader(reader, length) => request.respond(tiny_http::Response::new(
            status, headers, reader, length, None,
        )),
    };
}

/// Counts a running download while it lives.
struct Running<'a>(&'a AtomicUsize);

impl<'a> Running<'a> {
    fn start(running: &'a AtomicUsize, most: &AtomicUsize) -> Self {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        most.fetch_max(now, Ordering::SeqCst);
        Running(running)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn json_error(status: u16, kind: &str, message: &str) -> (u16, Vec<(String, String)>, Body) {
    (
        status,
        vec![("Content-Type".into(), "application/json".into())],
        Body::Bytes(
            serde_json::json!({ "kind": kind, "causes": [message] })
                .to_string()
                .into_bytes(),
        ),
    )
}

fn route(state: &mut State, request: &Request) -> Answer {
    let (path, query) = request
        .path
        .split_once('?')
        .unwrap_or((request.path.as_str(), ""));
    let rest = if let Some(rest) = path.strip_prefix("/api/v1/launcher/instances/") {
        if request.authorization.as_deref() != Some(&format!("Bearer {KEY}")) {
            return json_error(401, "Unauthorized", "Unauthorized");
        }
        rest
    } else if let Some(rest) = path.strip_prefix("/api/v1/public/instances/") {
        rest
    } else {
        return json_error(404, "NotFound", "Not found");
    };
    let Some(rest) = rest.strip_prefix(&format!("{SERVER}/")) else {
        return json_error(404, "NotFound", "Not found");
    };
    if rest == "client-pack" {
        return match &state.pack {
            Some(pack) => (
                200,
                vec![("ETag".into(), format!("\"{}\"", sha256_hex(pack)))],
                Body::Bytes(pack.clone()),
            ),
            None => json_error(
                400,
                "BadRequest",
                "Players don't need any mods to join this server",
            ),
        };
    }
    let is_image = rest == "image";
    let fault = match state.faults.front() {
        Some(Fault::Raw { .. }) => state.faults.pop_front(),
        Some(fault) if fault.for_image() == is_image => state.faults.pop_front(),
        _ => None,
    };
    if let Some(Fault::Raw {
        status,
        headers,
        body,
    }) = fault
    {
        return (status, headers, Body::Bytes(body));
    }
    if is_image {
        image(state, query, fault)
    } else if let Some(sha256) = rest.strip_prefix("objects/") {
        object(state, sha256, request.range.as_deref(), fault)
    } else {
        json_error(404, "NotFound", "Not found")
    }
}

fn rules_header() -> (String, String) {
    ("X-Lodestone-Rules".into(), RULES.to_string())
}

fn image(state: &State, query: &str, fault: Option<Fault>) -> Answer {
    let with_rules = |(status, mut headers, body): Answer| {
        headers.push(rules_header());
        (status, headers, body)
    };
    match &fault {
        Some(Fault::ImageNotFound) => return json_error(404, "NotFound", "Not found"),
        Some(Fault::NewerRules) => {
            let (status, mut headers, body) = json_error(
                400,
                "BadRequest",
                "This server runs a newer Lodestone: update Lodestone to join it",
            );
            headers.push(("X-Lodestone-Rules".into(), (RULES + 1).to_string()));
            return (status, headers, body);
        }
        Some(Fault::ImageUnavailable(kind, retry)) => {
            let (status, mut headers, body) = json_error(503, kind, "try again");
            headers.push(("Retry-After".into(), retry.to_string()));
            return with_rules((status, headers, body));
        }
        _ => {}
    }
    let mut have: Vec<String> = Vec::new();
    for pair in query.split('&') {
        if let Some(ids) = pair.strip_prefix("have=") {
            have = ids.split(',').map(str::to_string).collect();
        }
        if let Some(side) = pair.strip_prefix("side=")
            && side != "client"
        {
            return with_rules(json_error(400, "BadRequest", "Only side=client is served"));
        }
    }
    let Some(current) = state.images.last() else {
        let (status, mut headers, body) =
            json_error(503, "Conflict", "The server's content is being recorded");
        headers.push(("Retry-After".into(), "5".into()));
        return with_rules((status, headers, body));
    };
    let current = view(current, Side::Client, RULES).unwrap();
    let views: Vec<Image> = state
        .images
        .iter()
        .map(|image| view(image, Side::Client, RULES).unwrap())
        .collect();
    let mut named = current.id.clone();
    if let Some(Fault::WrongView(other)) = &fault {
        named = other.clone();
    }
    let mut headers = vec![
        ("X-Lodestone-View".into(), named.clone()),
        ("ETag".into(), format!("\"{named}\"")),
        ("Cache-Control".into(), "no-cache".into()),
        rules_header(),
    ];
    let content_type = match fault {
        Some(Fault::NotJson) => "text/plain",
        _ => "application/json",
    };
    // the first id the core knows decides
    for id in &have {
        if *id == current.id {
            return (204, headers, Body::Bytes(Vec::new()));
        }
        if let Some(from) = views.iter().find(|v| v.id == *id) {
            let delta = serde_json::to_vec(&diff(from, &current)).unwrap();
            let whole = current_json(&current);
            headers.push(("Content-Type".into(), content_type.into()));
            if delta.len() < whole.len() {
                headers.push(("X-Lodestone-Answer".into(), "delta".into()));
                return (200, headers, Body::Bytes(delta));
            }
            headers.push(("X-Lodestone-Answer".into(), "view".into()));
            return (200, headers, Body::Bytes(whole));
        }
    }
    headers.push(("Content-Type".into(), content_type.into()));
    headers.push(("X-Lodestone-Answer".into(), "view".into()));
    match fault {
        Some(Fault::Endless) => (
            200,
            headers,
            Body::Reader(Box::new(std::io::repeat(b' ')), None),
        ),
        Some(Fault::DeclaredLength(length)) => (
            200,
            headers,
            Body::Reader(
                Box::new(std::io::Cursor::new(b"{}".to_vec())),
                Some(length as usize),
            ),
        ),
        _ => (200, headers, Body::Bytes(current_json(&current))),
    }
}

fn current_json(view: &Image) -> Vec<u8> {
    serde_json::to_vec(view).unwrap()
}

fn object(state: &State, sha256: &str, range: Option<&str>, fault: Option<Fault>) -> Answer {
    match &fault {
        Some(Fault::ObjectUnavailable(kind, retry)) => {
            let (status, mut headers, body) = json_error(503, kind, "try again");
            headers.push(("Retry-After".into(), retry.to_string()));
            return (status, headers, body);
        }
        Some(Fault::ObjectStatus(status)) => return (*status, vec![], Body::Bytes(Vec::new())),
        _ => {}
    }
    let Some(current) = state.images.last() else {
        return json_error(503, "Conflict", "recording");
    };
    let current = view(current, Side::Client, RULES).unwrap();
    let served = current
        .entries
        .iter()
        .any(|e| e.sha256 == sha256 && matches!(e.source, Source::Object {}));
    let Some(bytes) = state.objects.get(sha256).filter(|_| served) else {
        return json_error(404, "NotFound", "Not found");
    };
    let size = bytes.len();
    let mut headers = vec![
        ("Content-Type".into(), "application/octet-stream".into()),
        ("ETag".into(), format!("\"{sha256}\"")),
        ("Accept-Ranges".into(), "bytes".into()),
    ];
    let start = match (range, &fault) {
        (_, Some(Fault::IgnoreRange)) => None,
        (Some(range), _) if size as u64 > 1 << 20 => range
            .strip_prefix("bytes=")
            .and_then(|r| r.strip_suffix('-'))
            .and_then(|r| r.parse::<usize>().ok()),
        _ => None,
    };
    if let Some(start) = start
        && start >= size
    {
        headers.push(("Content-Range".into(), format!("bytes */{size}")));
        return (416, headers, Body::Bytes(Vec::new()));
    }
    let (status, part) = match start {
        Some(start) => {
            headers.push((
                "Content-Range".into(),
                format!("bytes {start}-{}/{size}", size - 1),
            ));
            (206, bytes[start..].to_vec())
        }
        None => (200, bytes.clone()),
    };
    match fault {
        Some(Fault::Short(n)) => {
            let cut = part[..n.min(part.len())].to_vec();
            // no length: the body ends cleanly, short
            (
                status,
                headers,
                Body::Reader(Box::new(std::io::Cursor::new(cut)), None),
            )
        }
        Some(Fault::WrongBytes) => (status, headers, Body::Bytes(vec![b'x'; part.len()])),
        Some(Fault::Stall(wait)) => {
            let half = part[..part.len() / 2].to_vec();
            let reader = std::io::Cursor::new(half).chain(Stall(Some(wait)));
            (status, headers, Body::Reader(Box::new(reader), None))
        }
        _ => (status, headers, Body::Bytes(part)),
    }
}

/// Sends nothing for a while, then ends.
struct Stall(Option<Duration>);

impl Read for Stall {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        if let Some(wait) = self.0.take() {
            std::thread::sleep(wait);
        }
        Ok(0)
    }
}
