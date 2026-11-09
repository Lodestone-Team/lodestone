//! The introduction service: it passes candidate addresses between a host and the
//! friends who open its invite, and nothing else. No game traffic, no keys: the invite's
//! certificate fingerprint and token stay after the `#`, which is never sent here.
//!
//! The real one is `services/connect` (connect.lodestone.cc, on Cloudflare Workers),
//! whose README describes protocol v1. `holepunch rendezvous` serves the same protocol
//! over plain HTTP, for tests on a network of your own:
//!
//! - `POST /v1/invites` `{"candidates": [...]}` → `201 {"code", "secret", "expires_at"}`.
//! - `POST /v1/invites/<code>/joins` `{"candidates": [...]}` → `{"host": [...]}`.
//! - `GET /v1/invites/<code>/joins?after=<n>` with `Authorization: Bearer <secret>`: as a
//!   WebSocket, `{"joins": [{"id", "candidates"}]}` at once and then one per join; the
//!   host sends `ping` (answered `pong`) to stay. Plain, the joins at once.
//! - `DELETE /v1/invites/<code>` with `Authorization: Bearer <secret>`.
//! - Errors are `{"error": "<code>", "message": "<text>"}`. A socket on an invite that
//!   ends is closed with code 4000.

use std::{
    collections::HashMap,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        self, Message,
        client::IntoClientRequest as _,
        http::HeaderValue,
        protocol::{CloseFrame, Role, frame::coding::CloseCode},
    },
};

use crate::Error;

const MAX_INVITES: usize = 2000;
const MAX_JOINS: usize = 16;
const MAX_CANDIDATES: usize = 8;
const MAX_REQUEST: usize = 16 * 1024;
const IDLE: Duration = Duration::from_secs(120);
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// How often a waiting host says it's still there.
const PING_EVERY: Duration = Duration::from_secs(30);
/// The close code for "the invite has ended".
const CLOSE_ENDED: u16 = 4000;
/// Codes are short enough to read out: Crockford base32 without I, L, O, U.
const CODE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 10;

struct Invite {
    secret: String,
    host: Vec<SocketAddr>,
    joins: Vec<Vec<SocketAddr>>,
    created: Instant,
    last_seen: Instant,
    changed: Arc<Notify>,
}

type Invites = Arc<Mutex<HashMap<String, Invite>>>;

pub async fn serve(listen: SocketAddr) -> Result<(), Error> {
    let listener = TcpListener::bind(listen).await?;
    println!("Introduction service on http://{listen}");
    let invites: Invites = Arc::default();
    {
        let invites = invites.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let mut invites = invites.lock().await;
                let before = invites.len();
                invites.retain(|_, invite| {
                    let keep =
                        invite.last_seen.elapsed() < IDLE && invite.created.elapsed() < MAX_AGE;
                    if !keep {
                        invite.changed.notify_waiters();
                    }
                    keep
                });
                if invites.len() != before {
                    println!("  ended {} quiet invite(s)", before - invites.len());
                }
            }
        });
    }
    loop {
        let (stream, from) = listener.accept().await?;
        let invites = invites.clone();
        tokio::spawn(async move {
            if let Err(err) = handle(stream, from, invites).await {
                println!("  {from}: {err}");
            }
        });
    }
}

async fn handle(mut stream: TcpStream, from: SocketAddr, invites: Invites) -> Result<(), Error> {
    let request = tokio::time::timeout(Duration::from_secs(10), read_request(&mut stream))
        .await
        .map_err(|_| "request timed out")??;
    if request
        .header("upgrade")
        .is_some_and(|u| u.eq_ignore_ascii_case("websocket"))
    {
        return host_socket(stream, request, invites).await;
    }
    let (status, body) = route(&request, from, &invites).await;
    respond(&mut stream, status, &body).await
}

async fn respond(stream: &mut TcpStream, status: &str, body: &Value) -> Result<(), Error> {
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    /// Header names in lower case.
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// The secret from `Authorization: Bearer <secret>`.
    fn bearer(&self) -> Option<&str> {
        self.header("authorization")?.strip_prefix("Bearer ")
    }

    fn after(&self) -> usize {
        self.query
            .get("after")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }
}

async fn read_request(stream: &mut TcpStream) -> Result<Request, Error> {
    let mut buf = Vec::with_capacity(1024);
    let header_end = loop {
        let mut chunk = [0u8; 2048];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err("connection closed".into());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        if buf.len() > MAX_REQUEST {
            return Err("request too large".into());
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default().to_string();
    let target = first.next().unwrap_or_default().to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if length > MAX_REQUEST {
        return Err("body too large".into());
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < length {
        let mut chunk = [0u8; 2048];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Ok(Request {
        method,
        path: path.to_string(),
        query,
        headers,
        body,
    })
}

fn error(status: &'static str, code: &str, message: &str) -> (&'static str, Value) {
    (status, json!({ "error": code, "message": message }))
}

fn not_found() -> (&'static str, Value) {
    error(
        "404 Not Found",
        "not_found",
        "No such invite. It may have ended.",
    )
}

fn bad_candidates() -> (&'static str, Value) {
    error(
        "400 Bad Request",
        "bad_candidates",
        "Send {\"candidates\": [...]}: 1 to 8 reachable addresses.",
    )
}

fn candidates(body: &[u8]) -> Option<Vec<SocketAddr>> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let list = value.get("candidates")?.as_array()?;
    if list.is_empty() || list.len() > MAX_CANDIDATES {
        return None;
    }
    list.iter()
        .map(|addr| addr.as_str()?.parse().ok())
        .collect()
}

fn random(alphabet: &[u8], len: usize) -> String {
    let mut bytes = vec![0u8; len];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
        .expect("randomness");
    bytes
        .iter()
        .map(|b| alphabet[*b as usize % alphabet.len()] as char)
        .collect()
}

fn joins_after(invite: &Invite, after: usize) -> Value {
    let joins: Vec<Value> = invite.joins[after.min(invite.joins.len())..]
        .iter()
        .enumerate()
        .map(|(i, join)| {
            let list: Vec<String> = join.iter().map(ToString::to_string).collect();
            json!({ "id": after + i, "candidates": list })
        })
        .collect();
    json!({ "joins": joins })
}

async fn route(request: &Request, from: SocketAddr, invites: &Invites) -> (&'static str, Value) {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("GET", ["v1", "health"]) => ("200 OK", json!({ "ok": true, "version": "1" })),
        ("POST", ["v1", "invites"]) => {
            let Some(host) = candidates(&request.body) else {
                return bad_candidates();
            };
            let mut invites = invites.lock().await;
            if invites.len() >= MAX_INVITES {
                return error(
                    "429 Too Many Requests",
                    "rate_limited",
                    "Too many invites. Try again later.",
                );
            }
            let code = random(CODE_ALPHABET, CODE_LEN);
            let secret = random(b"0123456789abcdef", 32);
            let now = Instant::now();
            invites.insert(
                code.clone(),
                Invite {
                    secret: secret.clone(),
                    host,
                    joins: Vec::new(),
                    created: now,
                    last_seen: now,
                    changed: Arc::new(Notify::new()),
                },
            );
            println!("  {from}: new invite {code}");
            // A local service keeps no clock to report; the real one gives a date.
            (
                "201 Created",
                json!({ "code": code, "secret": secret, "expires_at": null }),
            )
        }
        ("POST", ["v1", "invites", code, "joins"]) => {
            let Some(join) = candidates(&request.body) else {
                return bad_candidates();
            };
            let mut invites = invites.lock().await;
            let Some(invite) = invites.get_mut(*code) else {
                return not_found();
            };
            if invite.joins.len() >= MAX_JOINS {
                return error(
                    "429 Too Many Requests",
                    "invite_full",
                    "This invite has had too many joins.",
                );
            }
            invite.joins.push(join);
            invite.changed.notify_waiters();
            println!("  {from}: joined {code}");
            let host: Vec<String> = invite.host.iter().map(ToString::to_string).collect();
            ("200 OK", json!({ "host": host }))
        }
        ("GET", ["v1", "invites", code, "joins"]) => {
            let mut invites = invites.lock().await;
            let Some(invite) = invites.get_mut(*code) else {
                return not_found();
            };
            if let Err(refused) = check_secret(request, &invite.secret) {
                return refused;
            }
            invite.last_seen = Instant::now();
            ("200 OK", joins_after(invite, request.after()))
        }
        ("DELETE", ["v1", "invites", code]) => {
            let mut invites = invites.lock().await;
            let Some(invite) = invites.get(*code) else {
                return not_found();
            };
            if let Err(refused) = check_secret(request, &invite.secret) {
                return refused;
            }
            if let Some(invite) = invites.remove(*code) {
                invite.changed.notify_waiters();
            }
            ("200 OK", json!({ "ok": true }))
        }
        _ => error("404 Not Found", "unknown", "Unknown request."),
    }
}

fn check_secret(request: &Request, secret: &str) -> Result<(), (&'static str, Value)> {
    let Some(given) = request.bearer() else {
        return Err(error(
            "401 Unauthorized",
            "unauthorized",
            "Send the invite secret as \"Authorization: Bearer <secret>\".",
        ));
    };
    if bool::from(subtle::ConstantTimeEq::ct_eq(
        given.as_bytes(),
        secret.as_bytes(),
    )) {
        Ok(())
    } else {
        Err(error("403 Forbidden", "forbidden", "Wrong secret."))
    }
}

/// A host waiting on its invite over a WebSocket.
async fn host_socket(
    mut stream: TcpStream,
    request: Request,
    invites: Invites,
) -> Result<(), Error> {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    let ["v1", "invites", code, "joins"] = parts.as_slice() else {
        let (status, body) = error("404 Not Found", "unknown", "Unknown request.");
        return respond(&mut stream, status, &body).await;
    };
    let code = code.to_string();
    let first = {
        let mut invites = invites.lock().await;
        match invites.get_mut(&code) {
            None => Err(not_found()),
            Some(invite) => match check_secret(&request, &invite.secret) {
                Err(refused) => Err(refused),
                Ok(()) => {
                    invite.last_seen = Instant::now();
                    Ok(joins_after(invite, request.after()))
                }
            },
        }
    };
    let first = match first {
        Ok(first) => first,
        Err((status, body)) => return respond(&mut stream, status, &body).await,
    };
    let key = request
        .header("sec-websocket-key")
        .ok_or("a WebSocket request without a key")?;
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
    stream
        .write_all(
            format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            )
            .as_bytes(),
        )
        .await?;
    let mut ws = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
    ws.send(Message::text(first.to_string())).await?;
    let mut sent = first["joins"]
        .as_array()
        .map_or(request.after(), |joins| request.after() + joins.len());

    loop {
        // What to wait for: a join, the invite ending, or a message from the host.
        let changed = match invites.lock().await.get(&code) {
            Some(invite) => invite.changed.clone(),
            None => return end_socket(ws).await,
        };
        tokio::select! {
            _ = changed.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            message = ws.next() => match message {
                Some(Ok(Message::Text(text))) if text.as_str() == "ping" => {
                    if let Some(invite) = invites.lock().await.get_mut(&code) {
                        invite.last_seen = Instant::now();
                    }
                    ws.send(Message::text("pong")).await?;
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return Ok(()),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(_)) => {
                    let _ = ws.close(Some(CloseFrame { code: CloseCode::Unsupported, reason: "only \"ping\" is expected".into() })).await;
                    return Ok(());
                }
            },
        }
        let news = match invites.lock().await.get(&code) {
            Some(invite) if invite.joins.len() > sent => {
                let news = joins_after(invite, sent);
                sent = invite.joins.len();
                news
            }
            Some(_) => continue,
            None => return end_socket(ws).await,
        };
        ws.send(Message::text(news.to_string())).await?;
    }
}

async fn end_socket(mut ws: WebSocketStream<TcpStream>) -> Result<(), Error> {
    let _ = ws
        .close(Some(CloseFrame {
            code: CloseCode::from(CLOSE_ENDED),
            reason: "ended".into(),
        }))
        .await;
    Ok(())
}

// ---------------------------------------------------------------------------
// The client side

/// Where the introduction service is: `ip:port` over plain HTTP (a test service), or a
/// domain over HTTPS (connect.lodestone.cc).
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Service {
    /// `http://1.2.3.4:8080`, `https://name.example`, `1.2.3.4:8080` or `name.example`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_end_matches('/');
        if let Some(rest) = text.strip_prefix("https://") {
            return Self::host_port(rest, 443, true);
        }
        if let Some(rest) = text.strip_prefix("http://") {
            return Self::host_port(rest, 80, false);
        }
        Self::from_link(text)
    }

    /// The service as an invite link carries it: an `ip:port` is plain HTTP, a name is HTTPS.
    pub fn from_link(text: &str) -> Option<Self> {
        match text.parse::<SocketAddr>() {
            Ok(addr) => Some(Self {
                host: addr.ip().to_string(),
                port: addr.port(),
                tls: false,
            }),
            Err(_) => Self::host_port(text, 443, true),
        }
    }

    fn host_port(text: &str, default_port: u16, tls: bool) -> Option<Self> {
        if text.is_empty() || text.contains('/') || text.contains('@') {
            return None;
        }
        let (host, port) = match text.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') || host.ends_with(']') => {
                (host.to_string(), port.parse().ok()?)
            }
            _ => (text.to_string(), default_port),
        };
        Some(Self {
            host: host.trim_matches(['[', ']']).to_string(),
            port,
            tls,
        })
    }

    /// How an invite link writes it.
    pub fn link(&self) -> String {
        match (self.tls, self.port) {
            (true, 443) => self.bracketed(),
            _ => format!("{}:{}", self.bracketed(), self.port),
        }
    }

    /// The Host header's value.
    fn authority(&self) -> String {
        match (self.tls, self.port) {
            (true, 443) | (false, 80) => self.bracketed(),
            _ => format!("{}:{}", self.bracketed(), self.port),
        }
    }

    fn bracketed(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        }
    }
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// A connection to the service: TLS for a named service, plain TCP for a test one.
async fn connect(service: &Service) -> Result<Pin<Box<dyn Io>>, Error> {
    let tcp = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect((service.host.as_str(), service.port)),
    )
    .await
    .map_err(|_| "the introduction service didn't answer")??;
    if !service.tls {
        return Ok(Box::pin(tcp));
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(crate::provider())
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(service.host.clone())?;
    let tls = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp),
    )
    .await
    .map_err(|_| "the introduction service didn't answer")??;
    Ok(Box::pin(tls))
}

/// One request to the service, with the invite's secret if given; returns the JSON body
/// of a 2xx answer.
pub async fn call(
    service: &Service,
    method: &str,
    path: &str,
    secret: Option<&str>,
    body: Option<Value>,
) -> Result<Value, Error> {
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let authorization = secret
        .map(|secret| format!("Authorization: Bearer {secret}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\n{authorization}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        service.authority(),
        body.len()
    );
    let mut stream = connect(service).await?;
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    // Some servers end TLS without a close_notify; what arrived before still counts.
    let _ = tokio::time::timeout(Duration::from_secs(15), stream.read_to_end(&mut response)).await;
    let text = String::from_utf8_lossy(&response);
    let (head, rest) = text
        .split_once("\r\n\r\n")
        .ok_or("no answer from the introduction service")?;
    let chunked = head.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    });
    let body = if chunked {
        dechunk(rest)
    } else {
        rest.to_string()
    };
    let status = head.lines().next().unwrap_or_default();
    let value: Value = serde_json::from_str(body.trim()).unwrap_or(Value::Null);
    if !status.contains(" 2") {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(status);
        let code = value.get("error").and_then(Value::as_str).unwrap_or("?");
        return Err(format!("the introduction service said: {message} ({code})").into());
    }
    Ok(value)
}

fn dechunk(mut body: &str) -> String {
    let mut out = String::new();
    while let Some((size, rest)) = body.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 || rest.len() < size {
            break;
        }
        out.push_str(&rest[..size]);
        body = rest[size..].trim_start_matches("\r\n");
    }
    out
}

pub fn addrs_of(value: &Value) -> Vec<SocketAddr> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| a.as_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// What a waiting host hears.
#[derive(Debug, PartialEq)]
pub enum Event {
    /// Friends who joined: their join number and candidate addresses.
    Joins(Vec<(usize, Vec<SocketAddr>)>),
    /// The invite has ended (deleted, quiet too long, or a day old).
    Ended,
}

/// A host's WebSocket, waiting on its invite.
pub struct Watch {
    ws: WebSocketStream<Pin<Box<dyn Io>>>,
    ping: tokio::time::Interval,
}

/// Waits on an invite for joins after the `after`th. `None` if the invite has ended.
pub async fn watch(
    service: &Service,
    code: &str,
    secret: &str,
    after: usize,
) -> Result<Option<Watch>, Error> {
    let scheme = if service.tls { "wss" } else { "ws" };
    let url = format!(
        "{scheme}://{}/v1/invites/{code}/joins?after={after}",
        service.authority()
    );
    let mut request = url.into_client_request()?;
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {secret}"))?,
    );
    let stream = connect(service).await?;
    let ws = match tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::client_async(request, stream),
    )
    .await
    .map_err(|_| "the introduction service didn't answer")?
    {
        Ok((ws, _)) => ws,
        Err(tungstenite::Error::Http(response)) if response.status() == 404 => return Ok(None),
        Err(tungstenite::Error::Http(response)) => {
            return Err(format!("the introduction service refused: {}", response.status()).into());
        }
        Err(err) => return Err(err.into()),
    };
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.reset();
    Ok(Some(Watch { ws, ping }))
}

impl Watch {
    /// The next joins, or the end of the invite. Keeps the socket alive meanwhile.
    pub async fn next(&mut self) -> Result<Event, Error> {
        loop {
            tokio::select! {
                _ = self.ping.tick() => self.ws.send(Message::text("ping")).await?,
                message = self.ws.next() => match message.ok_or("the service closed the connection")?? {
                    Message::Text(text) => {
                        if text.as_str() == "pong" {
                            continue;
                        }
                        let value: Value = serde_json::from_str(text.as_str())?;
                        let joins = value["joins"]
                            .as_array()
                            .ok_or("a garbled message from the service")?
                            .iter()
                            .filter_map(|join| Some((usize::try_from(join["id"].as_u64()?).ok()?, addrs_of(&join["candidates"]))))
                            .collect();
                        return Ok(Event::Joins(joins));
                    }
                    Message::Close(Some(frame)) if u16::from(frame.code) == CLOSE_ENDED => {
                        return Ok(Event::Ended);
                    }
                    Message::Close(frame) => {
                        return Err(format!("the service closed the connection ({frame:?})").into());
                    }
                    _ => {}
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn local_service() -> Service {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        tokio::spawn(serve(addr));
        tokio::time::sleep(Duration::from_millis(100)).await;
        Service::from_link(&addr.to_string()).unwrap()
    }

    #[tokio::test]
    async fn a_waiting_host_hears_about_a_join() {
        let service = &local_service().await;
        let health = call(service, "GET", "/v1/health", None, None)
            .await
            .unwrap();
        assert_eq!(health["version"], "1");

        let created = call(
            service,
            "POST",
            "/v1/invites",
            None,
            Some(json!({"candidates": ["203.0.113.5:4000"]})),
        )
        .await
        .unwrap();
        let code = created["code"].as_str().unwrap().to_string();
        let secret = created["secret"].as_str().unwrap().to_string();
        assert_eq!(code.len(), CODE_LEN);

        let mut watch = watch(service, &code, &secret, 0).await.unwrap().unwrap();
        assert_eq!(watch.next().await.unwrap(), Event::Joins(vec![]));
        let joined = call(
            service,
            "POST",
            &format!("/v1/invites/{code}/joins"),
            None,
            Some(json!({"candidates": ["198.51.100.7:5000"]})),
        )
        .await
        .unwrap();
        assert_eq!(
            addrs_of(&joined["host"]),
            vec!["203.0.113.5:4000".parse().unwrap()]
        );
        assert_eq!(
            watch.next().await.unwrap(),
            Event::Joins(vec![(0, vec!["198.51.100.7:5000".parse().unwrap()])])
        );

        // The secret goes in the header, and is checked.
        let path = format!("/v1/invites/{code}/joins");
        assert!(call(service, "GET", &path, None, None).await.is_err());
        assert!(
            call(service, "GET", &path, Some("nope"), None)
                .await
                .is_err()
        );
        let listed = call(service, "GET", &path, Some(&secret), None)
            .await
            .unwrap();
        assert_eq!(listed["joins"].as_array().unwrap().len(), 1);
        assert!(super::watch(service, &code, "nope", 0).await.is_err());

        // Ending the invite tells the waiting host.
        call(
            service,
            "DELETE",
            &format!("/v1/invites/{code}"),
            Some(&secret),
            None,
        )
        .await
        .unwrap();
        assert_eq!(watch.next().await.unwrap(), Event::Ended);
        assert!(
            super::watch(service, &code, &secret, 0)
                .await
                .unwrap()
                .is_none()
        );
        let missing = call(
            service,
            "POST",
            &format!("/v1/invites/{code}/joins"),
            None,
            Some(json!({"candidates": ["1.2.3.4:5"]})),
        )
        .await
        .unwrap_err();
        assert!(missing.to_string().contains("not_found"), "{missing}");
    }

    #[test]
    fn services_parse_and_print() {
        let http = Service::parse("http://16.146.26.94:8080").unwrap();
        assert_eq!(
            (http.tls, http.port, http.link()),
            (false, 8080, "16.146.26.94:8080".to_string())
        );
        let worker = Service::parse("https://connect.lodestone.cc/").unwrap();
        assert_eq!(
            (worker.tls, worker.port, worker.link()),
            (true, 443, "connect.lodestone.cc".to_string())
        );
        assert_eq!(worker.authority(), "connect.lodestone.cc");
        assert_eq!(Service::from_link(&worker.link()), Some(worker));
        assert_eq!(Service::from_link("16.146.26.94:8080"), Some(http));
        assert!(Service::parse("https://evil.example@x").is_none());
    }
}
