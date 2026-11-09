//! An in-process stand-in for connect.lodestone.cc, over plain HTTP on 127.0.0.1: the
//! same protocol (v1 with stable links) as `services/connect`, without its limits.

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
pub struct State {
    invites: HashMap<String, Invite>,
    next: u32,
}

#[derive(Clone)]
pub struct FakeService {
    pub addr: std::net::SocketAddr,
    state: Arc<Mutex<State>>,
    /// (code, what): "join" for a new join, "ended" or "renewed".
    news: broadcast::Sender<(String, &'static str)>,
}

impl FakeService {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let service = FakeService {
            addr: listener.local_addr().unwrap(),
            state: Arc::default(),
            news: broadcast::channel(64).0,
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
                _ => (404, json!({"error": "unknown"})),
            }
        };
        respond(&mut stream, status, answer).await
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
