//! An in-process stand-in for connect.lodestone.cc, over plain HTTP on 127.0.0.1: the
//! same protocol (v1 with stable links, and v2 for friends) as `services/connect`,
//! without its limits.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use tokio_tungstenite::{
    tungstenite::{
        self,
        protocol::{frame::coding::CloseCode, CloseFrame, Role},
        Message,
    },
    WebSocketStream,
};

#[derive(Default)]
struct Invite {
    secret: String,
    host: Vec<String>,
    joins: Vec<Vec<String>>,
    live: bool,
    kept: bool,
}

#[derive(Default)]
struct Pair {
    access_hash: String,
    card: String,
    secret: String,
    replies: Vec<String>,
}

#[derive(Default)]
struct Session {
    secret: String,
    records: HashMap<String, String>,
    live: bool,
}

#[derive(Default)]
pub struct State {
    invites: HashMap<String, Invite>,
    next: u32,
    pairs: HashMap<String, Pair>,
    /// slot id -> (SHA-256 of the write proof, sealed state)
    slots: HashMap<String, (String, String)>,
    sessions: HashMap<String, Session>,
    /// lookup id -> session
    lookups: HashMap<String, String>,
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Clone)]
pub struct FakeService {
    pub addr: std::net::SocketAddr,
    state: Arc<Mutex<State>>,
    /// (code, what): "join" for a new join, "ended" or "renewed".
    news: broadcast::Sender<(String, &'static str)>,
    /// (session, message) for meeting sessions' sockets; a null message ends it.
    knocks: broadcast::Sender<(String, Value)>,
}

impl FakeService {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let service = FakeService {
            addr: listener.local_addr().unwrap(),
            state: Arc::default(),
            news: broadcast::channel(64).0,
            knocks: broadcast::channel(64).0,
        };
        let serving = service.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let service = serving.clone();
                tokio::spawn(async move {
                    let _ = service.handle(stream).await;
                });
            }
        });
        service
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Ends an invite as an idle host's would end (the code stays kept).
    pub fn end_quietly(&self, code: &str) {
        if let Some(invite) = self.state.lock().unwrap().invites.get_mut(code) {
            invite.live = false;
        }
        let _ = self.news.send((code.to_string(), "ended"));
    }

    /// Ends a meeting session, as one whose core went quiet.
    pub fn end_session(&self, session: &str) {
        if let Some(open) = self.state.lock().unwrap().sessions.get_mut(session) {
            open.live = false;
        }
        let _ = self.knocks.send((session.to_string(), Value::Null));
    }

    /// The live meeting sessions.
    pub fn sessions(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        state
            .sessions
            .iter()
            .filter(|(_, s)| s.live)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn joins(&self, code: &str) -> usize {
        self.state
            .lock()
            .unwrap()
            .invites
            .get(code)
            .map_or(0, |i| i.joins.len())
    }

    async fn handle(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        let head_end = loop {
            let mut chunk = [0u8; 4096];
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Ok(());
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break at;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let mut lines = head.lines();
        let mut first = lines.next().unwrap_or_default().split_whitespace();
        let method = first.next().unwrap_or_default().to_string();
        let target = first.next().unwrap_or_default().to_string();
        let headers: HashMap<String, String> = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();
        let length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut body = buf[head_end + 4..].to_vec();
        while body.len() < length {
            let mut chunk = [0u8; 4096];
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        let (path, query) = target.split_once('?').unwrap_or((&target, ""));
        let after: usize = query
            .strip_prefix("after=")
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        let parts: Vec<String> = path
            .trim_matches('/')
            .split('/')
            .map(str::to_string)
            .collect();
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_string);
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let candidates: Vec<String> = body["candidates"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|a| Some(a.as_str()?.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();

        if headers
            .get("upgrade")
            .is_some_and(|u| u.eq_ignore_ascii_case("websocket"))
        {
            if let ["v2", "meet", session, "knocks"] = parts.as_slice() {
                return self.meet_socket(stream, session, bearer, &headers).await;
            }
            let ["v1", "invites", code, "joins"] = parts.as_slice() else {
                return respond(&mut stream, 404, json!({"error": "unknown"})).await;
            };
            return self.socket(stream, code, bearer, after, &headers).await;
        }

        let (status, answer) = {
            let mut state = self.state.lock().unwrap();
            match (method.as_str(), parts.as_slice()) {
                ("GET", ["v1", "health"]) => (200, json!({"ok": true, "version": "1"})),
                ("POST", ["v1", "invites"]) => {
                    state.next += 1;
                    let code = format!("TEST{:06}", state.next);
                    let secret = format!("{:032x}", state.next as u128 * 7919);
                    let kept = body["keep"] == json!(true);
                    state.invites.insert(
                        code.clone(),
                        Invite {
                            secret: secret.clone(),
                            host: candidates,
                            live: true,
                            kept,
                            ..Invite::default()
                        },
                    );
                    let mut answer = json!({"code": code, "secret": secret, "expires_at": null});
                    if kept {
                        answer["kept_until"] = json!("later");
                    }
                    (201, answer)
                }
                ("PUT", ["v1", "invites", code]) => match state.invites.get_mut(*code) {
                    Some(invite) if bearer.as_deref() == Some(invite.secret.as_str()) => {
                        if !invite.live && !invite.kept {
                            (404, json!({"error": "not_found"}))
                        } else {
                            invite.live = true;
                            invite.host = candidates;
                            invite.joins.clear();
                            let _ = self.news.send((code.to_string(), "renewed"));
                            (
                                200,
                                json!({"code": code, "expires_at": null, "kept_until": "later"}),
                            )
                        }
                    }
                    Some(_) => (403, json!({"error": "forbidden"})),
                    None => (404, json!({"error": "not_found"})),
                },
                ("POST", ["v1", "invites", code, "joins"]) => match state.invites.get_mut(*code) {
                    Some(invite) if invite.live => {
                        invite.joins.push(candidates);
                        let _ = self.news.send((code.to_string(), "join"));
                        (200, json!({"host": invite.host}))
                    }
                    Some(_) => (
                        404,
                        json!({"error": "host_offline", "message": "The host isn't sharing right now."}),
                    ),
                    None => (404, json!({"error": "not_found"})),
                },
                ("DELETE", ["v1", "invites", code]) => match state.invites.get(*code) {
                    Some(invite) if bearer.as_deref() == Some(invite.secret.as_str()) => {
                        state.invites.remove(*code);
                        let _ = self.news.send((code.to_string(), "ended"));
                        (200, json!({"ok": true}))
                    }
                    Some(_) => (403, json!({"error": "forbidden"})),
                    None => (404, json!({"error": "not_found"})),
                },
                _ => self.v2(&mut state, &method, &parts, bearer.as_deref(), &body),
            }
        };
        respond(&mut stream, status, answer).await
    }

    fn v2(
        &self,
        state: &mut State,
        method: &str,
        parts: &[&str],
        bearer: Option<&str>,
        body: &Value,
    ) -> (u16, Value) {
        let text = |name: &str| body[name].as_str().unwrap_or_default().to_string();
        let forbidden = (403, json!({"error": "forbidden"}));
        let not_found = (404, json!({"error": "not_found"}));
        match (method, parts) {
            ("PUT", ["v2", "pairs", code]) => {
                if state.pairs.contains_key(*code) {
                    return (409, json!({"error": "taken"}));
                }
                state.next += 1;
                let secret = format!("{:032x}", state.next as u128 * 104729);
                state.pairs.insert(
                    code.to_string(),
                    Pair {
                        access_hash: text("access"),
                        card: text("card"),
                        secret: secret.clone(),
                        replies: Vec::new(),
                    },
                );
                (201, json!({"secret": secret, "expires_at": null}))
            }
            ("GET", ["v2", "pairs", code]) => match state.pairs.get(*code) {
                Some(pair) if bearer.map(sha256_hex) == Some(pair.access_hash.clone()) => {
                    (200, json!({"card": pair.card}))
                }
                Some(_) => forbidden,
                None => not_found,
            },
            ("POST", ["v2", "pairs", code, "replies"]) => match state.pairs.get_mut(*code) {
                Some(pair) if bearer.map(sha256_hex) == Some(pair.access_hash.clone()) => {
                    if pair.replies.len() >= 4 {
                        return (429, json!({"error": "full"}));
                    }
                    pair.replies.push(text("reply"));
                    (200, json!({"count": pair.replies.len()}))
                }
                Some(_) => forbidden,
                None => not_found,
            },
            ("GET", ["v2", "pairs", code, "replies"]) => match state.pairs.get(*code) {
                Some(pair) if bearer == Some(pair.secret.as_str()) => {
                    (200, json!({"replies": pair.replies}))
                }
                Some(_) => forbidden,
                None => not_found,
            },
            ("DELETE", ["v2", "pairs", code]) => match state.pairs.get(*code) {
                Some(pair) if bearer == Some(pair.secret.as_str()) => {
                    state.pairs.remove(*code);
                    (200, json!({"ok": true}))
                }
                Some(_) => forbidden,
                None => not_found,
            },
            ("PUT", ["v2", "slots", id]) => {
                let Some(proof) = bearer else {
                    return (401, json!({"error": "unauthorized"}));
                };
                let hash = sha256_hex(proof);
                match state.slots.get(*id) {
                    Some((owner, _)) if owner != &hash => forbidden,
                    _ => {
                        state.slots.insert(id.to_string(), (hash, text("sealed")));
                        (200, json!({"ok": true}))
                    }
                }
            }
            ("GET", ["v2", "slots", id]) => match state.slots.get(*id) {
                Some((_, sealed)) => (200, json!({"sealed": sealed})),
                None => not_found,
            },
            ("PUT", ["v2", "meet"]) if records_of(body).len() > 256 => (
                400,
                json!({"error": "bad_body", "message": "At most 256 records."}),
            ),
            ("PUT", ["v2", "meet"]) => {
                state.next += 1;
                let session = format!("{:032x}", state.next as u128 * 15485863);
                let secret = format!("{:032x}", state.next as u128 * 32452843);
                let records = records_of(body);
                for lookup in records.keys() {
                    state.lookups.insert(lookup.clone(), session.clone());
                }
                state.sessions.insert(
                    session.clone(),
                    Session {
                        secret: secret.clone(),
                        records,
                        live: true,
                    },
                );
                (
                    201,
                    json!({"session": session, "secret": secret, "expires_at": null}),
                )
            }
            ("PUT", ["v2", "meet", session]) => {
                let records = records_of(body);
                match state.sessions.get_mut(*session) {
                    Some(open) if open.live && bearer == Some(open.secret.as_str()) => {
                        open.records = records.clone();
                        state.lookups.retain(|_, owner| owner != session);
                        for lookup in records.keys() {
                            state.lookups.insert(lookup.clone(), session.to_string());
                        }
                        (200, json!({"expires_at": null}))
                    }
                    Some(open) if open.live => forbidden,
                    _ => not_found,
                }
            }
            ("DELETE", ["v2", "meet", session]) => match state.sessions.get_mut(*session) {
                Some(open) if bearer == Some(open.secret.as_str()) => {
                    open.live = false;
                    let _ = self.knocks.send((session.to_string(), Value::Null));
                    (200, json!({"ok": true}))
                }
                Some(_) => forbidden,
                None => not_found,
            },
            ("POST", ["v2", "lookups", lookup, "knocks"]) => {
                let Some(session) = state.lookups.get(*lookup).cloned() else {
                    return not_found;
                };
                match state.sessions.get(&session) {
                    Some(open) if open.live => {
                        let record = open.records.get(*lookup).cloned().unwrap_or_default();
                        let _ = self.knocks.send((
                            session,
                            json!({"knocks": [{"lookup": lookup, "sealed": text("sealed"), "seen": "127.0.0.1"}]}),
                        ));
                        (200, json!({"sealed": record}))
                    }
                    _ => not_found,
                }
            }
            ("HEAD", ["v2", "lookups", lookup]) => {
                let live = state
                    .lookups
                    .get(*lookup)
                    .and_then(|session| state.sessions.get(session))
                    .is_some_and(|open| open.live);
                if live {
                    (200, Value::Null)
                } else {
                    not_found
                }
            }
            _ => (404, json!({"error": "unknown"})),
        }
    }

    async fn meet_socket(
        &self,
        mut stream: TcpStream,
        session: &str,
        bearer: Option<String>,
        headers: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut knocks = self.knocks.subscribe();
        let allowed = {
            let state = self.state.lock().unwrap();
            match state.sessions.get(session) {
                Some(open) if open.live && bearer.as_deref() == Some(open.secret.as_str()) => {
                    Ok(())
                }
                Some(open) if open.live => Err(403),
                _ => Err(404),
            }
        };
        match allowed {
            Ok(()) => {}
            Err(403) => return respond(&mut stream, 403, json!({"error": "forbidden"})).await,
            Err(_) => return respond(&mut stream, 404, json!({"error": "not_found"})).await,
        }
        let key = headers.get("sec-websocket-key").ok_or("no key")?;
        let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
        let crlf = "\r\n";
        stream
            .write_all(
                format!("HTTP/1.1 101 Switching Protocols{crlf}Upgrade: websocket{crlf}Connection: Upgrade{crlf}Sec-WebSocket-Accept: {accept}{crlf}{crlf}")
                    .as_bytes(),
            )
            .await?;
        let mut ws = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
        loop {
            tokio::select! {
                message = ws.next() => match message {
                    Some(Ok(Message::Text(text))) if text.as_str() == "ping" => ws.send(Message::text("pong")).await?,
                    Some(Ok(_)) => {}
                    _ => return Ok(()),
                },
                knock = knocks.recv() => {
                    let Ok((about, message)) = knock else { continue };
                    if about != session {
                        continue;
                    }
                    if message.is_null() {
                        let _ = ws.send(Message::Close(Some(CloseFrame { code: CloseCode::from(4000), reason: "".into() }))).await;
                        return Ok(());
                    }
                    ws.send(Message::text(message.to_string())).await?;
                }
            }
        }
    }

    async fn socket(
        &self,
        mut stream: TcpStream,
        code: &str,
        bearer: Option<String>,
        after: usize,
        headers: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut news = self.news.subscribe();
        let first = {
            let state = self.state.lock().unwrap();
            match state.invites.get(code) {
                Some(invite)
                    if invite.live && bearer.as_deref() == Some(invite.secret.as_str()) =>
                {
                    Ok(joins_after(invite, after))
                }
                Some(invite) if invite.live => Err(403),
                _ => Err(404),
            }
        };
        let first = match first {
            Ok(first) => first,
            Err(403) => return respond(&mut stream, 403, json!({"error": "forbidden"})).await,
            Err(_) => return respond(&mut stream, 404, json!({"error": "not_found"})).await,
        };
        let key = headers.get("sec-websocket-key").ok_or("no key")?;
        let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
        stream
            .write_all(
                format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut ws = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
        let mut sent = after + first["joins"].as_array().map_or(0, Vec::len);
        ws.send(Message::text(first.to_string())).await?;
        loop {
            tokio::select! {
                message = ws.next() => match message {
                    Some(Ok(Message::Text(text))) if text.as_str() == "ping" => ws.send(Message::text("pong")).await?,
                    Some(Ok(_)) => {}
                    _ => return Ok(()),
                },
                news = news.recv() => {
                    let Ok((about, what)) = news else { continue };
                    if about != code {
                        continue;
                    }
                    let close = |code: u16| Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: "".into() }));
                    match what {
                        "ended" => { let _ = ws.send(close(4000)).await; return Ok(()); }
                        "renewed" => { let _ = ws.send(close(4001)).await; return Ok(()); }
                        _ => {
                            let message = {
                                let state = self.state.lock().unwrap();
                                let Some(invite) = state.invites.get(code) else { continue };
                                let message = joins_after(invite, sent);
                                sent = invite.joins.len();
                                message
                            };
                            ws.send(Message::text(message.to_string())).await?;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {}
            }
        }
    }
}

fn records_of(body: &Value) -> HashMap<String, String> {
    body["records"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    Some((
                        r["lookup"].as_str()?.to_string(),
                        r["sealed"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn joins_after(invite: &Invite, after: usize) -> Value {
    let joins: Vec<Value> = invite.joins[after.min(invite.joins.len())..]
        .iter()
        .enumerate()
        .map(|(i, join)| json!({"id": after + i, "candidates": join}))
        .collect();
    json!({ "joins": joins })
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    body: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = body.to_string();
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}
