//! The introduction service: it passes candidate addresses between a host and the
//! friends who open its invite, and nothing else. No game traffic, no keys: the invite's
//! certificate fingerprint and token stay after the `#`, which is never sent here.
//!
//! Plain HTTP/1.1 with JSON bodies, one request per connection:
//!
//! - `POST /v1/sessions` `{"candidates": [...]}` → `{"code", "secret"}`: a host registers.
//! - `POST /v1/sessions/<code>/joins` `{"candidates": [...]}` → `{"host": [...]}`: a friend
//!   joins and gets the host's addresses.
//! - `GET /v1/sessions/<code>/joins?secret=<s>&after=<n>` → `{"joins": [{"id", "candidates"}]}`:
//!   the host waits (up to 25 s) for joins after the `n`th. Polling keeps the session alive.
//! - `DELETE /v1/sessions/<code>?secret=<s>`: the host ends it.
//!
//! A session no host has polled for two minutes is dropped. There are caps on sessions,
//! joins per session, addresses per message and request sizes.
//!
//! The prototype serves plain HTTP; a real deployment would sit behind HTTPS so nobody
//! on the path can read the addresses or the host's secret.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Notify},
};

use crate::Error;

const MAX_SESSIONS: usize = 2000;
const MAX_JOINS: usize = 16;
const MAX_CANDIDATES: usize = 8;
const MAX_REQUEST: usize = 16 * 1024;
const IDLE: Duration = Duration::from_secs(120);
const LONG_POLL: Duration = Duration::from_secs(25);
/// Codes are short enough to read out: Crockford base32 without I, L, O, U.
const CODE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LEN: usize = 10;

struct Session {
    secret: String,
    host: Vec<SocketAddr>,
    joins: Vec<Vec<SocketAddr>>,
    last_seen: Instant,
    changed: Arc<Notify>,
}

type Sessions = Arc<Mutex<HashMap<String, Session>>>;

pub async fn serve(listen: SocketAddr) -> Result<(), Error> {
    let listener = TcpListener::bind(listen).await?;
    println!("Introduction service on http://{listen}");
    let sessions: Sessions = Arc::default();
    {
        let sessions = sessions.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let mut sessions = sessions.lock().await;
                let before = sessions.len();
                sessions.retain(|_, session| session.last_seen.elapsed() < IDLE);
                if sessions.len() != before {
                    println!("  dropped {} idle session(s)", before - sessions.len());
                }
            }
        });
    }
    loop {
        let (stream, from) = listener.accept().await?;
        let sessions = sessions.clone();
        tokio::spawn(async move {
            if let Err(err) = handle(stream, from, sessions).await {
                println!("  {from}: {err}");
            }
        });
    }
}

async fn handle(mut stream: TcpStream, from: SocketAddr, sessions: Sessions) -> Result<(), Error> {
    let request = tokio::time::timeout(Duration::from_secs(10), read_request(&mut stream))
        .await
        .map_err(|_| "request timed out")??;
    let (status, body) = route(&request, from, &sessions).await;
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
    body: Vec<u8>,
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
    let length: usize = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
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
        body,
    })
}

fn error(status: &'static str, message: &str) -> (&'static str, Value) {
    (status, json!({ "error": message }))
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

async fn route(request: &Request, from: SocketAddr, sessions: &Sessions) -> (&'static str, Value) {
    let parts: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    match (request.method.as_str(), parts.as_slice()) {
        ("POST", ["v1", "sessions"]) => {
            let Some(host) = candidates(&request.body) else {
                return error(
                    "400 Bad Request",
                    "send {\"candidates\": [...]}, 1 to 8 addresses",
                );
            };
            let mut sessions = sessions.lock().await;
            if sessions.len() >= MAX_SESSIONS {
                return error(
                    "503 Service Unavailable",
                    "too many sessions; try again later",
                );
            }
            let code = random(CODE_ALPHABET, CODE_LEN);
            let secret = random(b"0123456789abcdef", 32);
            sessions.insert(
                code.clone(),
                Session {
                    secret: secret.clone(),
                    host,
                    joins: Vec::new(),
                    last_seen: Instant::now(),
                    changed: Arc::new(Notify::new()),
                },
            );
            println!("  {from}: new session {code}");
            ("201 Created", json!({ "code": code, "secret": secret }))
        }
        ("POST", ["v1", "sessions", code, "joins"]) => {
            let Some(join) = candidates(&request.body) else {
                return error(
                    "400 Bad Request",
                    "send {\"candidates\": [...]}, 1 to 8 addresses",
                );
            };
            let mut sessions = sessions.lock().await;
            let Some(session) = sessions.get_mut(*code) else {
                return error("404 Not Found", "no such invite (it may have ended)");
            };
            if session.joins.len() >= MAX_JOINS {
                return error(
                    "429 Too Many Requests",
                    "this invite has had too many joins",
                );
            }
            session.joins.push(join);
            session.changed.notify_waiters();
            println!("  {from}: joined {code}");
            let host: Vec<String> = session.host.iter().map(ToString::to_string).collect();
            ("200 OK", json!({ "host": host }))
        }
        ("GET", ["v1", "sessions", code, "joins"]) => {
            let after: usize = request
                .query
                .get("after")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let deadline = Instant::now() + LONG_POLL;
            loop {
                let notified = {
                    let mut sessions = sessions.lock().await;
                    let Some(session) = sessions.get_mut(*code) else {
                        return error("404 Not Found", "no such session");
                    };
                    if !secret_matches(request.query.get("secret"), &session.secret) {
                        return error("403 Forbidden", "wrong secret");
                    }
                    session.last_seen = Instant::now();
                    if session.joins.len() > after || Instant::now() >= deadline {
                        let joins: Vec<Value> = session.joins[after.min(session.joins.len())..]
                            .iter()
                            .enumerate()
                            .map(|(i, join)| {
                                let list: Vec<String> =
                                    join.iter().map(ToString::to_string).collect();
                                json!({ "id": after + i, "candidates": list })
                            })
                            .collect();
                        return ("200 OK", json!({ "joins": joins }));
                    }
                    session.changed.clone()
                };
                let _ = tokio::time::timeout(
                    deadline.saturating_duration_since(Instant::now()),
                    notified.notified(),
                )
                .await;
            }
        }
        ("DELETE", ["v1", "sessions", code]) => {
            let mut sessions = sessions.lock().await;
            match sessions.get(*code) {
                Some(session) if secret_matches(request.query.get("secret"), &session.secret) => {
                    sessions.remove(*code);
                    ("200 OK", json!({ "ok": true }))
                }
                Some(_) => error("403 Forbidden", "wrong secret"),
                None => error("404 Not Found", "no such session"),
            }
        }
        _ => error("404 Not Found", "unknown request"),
    }
}

fn secret_matches(given: Option<&String>, secret: &str) -> bool {
    given.is_some_and(|given| {
        bool::from(subtle::ConstantTimeEq::ct_eq(
            given.as_bytes(),
            secret.as_bytes(),
        ))
    })
}

// ---------------------------------------------------------------------------
// The client side

/// One plain-HTTP request to the service; returns the JSON body of a 2xx answer.
pub async fn call(
    service: SocketAddr,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, Error> {
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(service))
        .await
        .map_err(|_| "the introduction service didn't answer")??;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {service}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    tokio::time::timeout(
        LONG_POLL + Duration::from_secs(10),
        stream.read_to_end(&mut response),
    )
    .await
    .map_err(|_| "the introduction service took too long")??;
    let text = String::from_utf8_lossy(&response);
    let (head, body) = text.split_once("\r\n\r\n").ok_or("a garbled answer")?;
    let status = head.lines().next().unwrap_or_default();
    let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if !status.contains(" 2") {
        let message = value.get("error").and_then(Value::as_str).unwrap_or(status);
        return Err(format!("the introduction service said: {message}").into());
    }
    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_host_hears_about_a_join() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let service = listener.local_addr().unwrap();
        drop(listener);
        tokio::spawn(serve(service));
        tokio::time::sleep(Duration::from_millis(100)).await;

        let created = call(
            service,
            "POST",
            "/v1/sessions",
            Some(json!({"candidates": ["203.0.113.5:4000"]})),
        )
        .await
        .unwrap();
        let code = created["code"].as_str().unwrap().to_string();
        let secret = created["secret"].as_str().unwrap().to_string();
        assert_eq!(code.len(), CODE_LEN);

        let poll = {
            let path = format!("/v1/sessions/{code}/joins?secret={secret}&after=0");
            tokio::spawn(async move { call(service, "GET", &path, None).await })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        let joined = call(
            service,
            "POST",
            &format!("/v1/sessions/{code}/joins"),
            Some(json!({"candidates": ["198.51.100.7:5000"]})),
        )
        .await
        .unwrap();
        assert_eq!(
            addrs_of(&joined["host"]),
            vec!["203.0.113.5:4000".parse().unwrap()]
        );
        let joins = poll.await.unwrap().unwrap();
        assert_eq!(
            addrs_of(&joins["joins"][0]["candidates"]),
            vec!["198.51.100.7:5000".parse().unwrap()]
        );

        let wrong = call(
            service,
            "GET",
            &format!("/v1/sessions/{code}/joins?secret=nope&after=0"),
            None,
        )
        .await;
        assert!(wrong.is_err());
        let missing = call(
            service,
            "POST",
            "/v1/sessions/NOPE/joins",
            Some(json!({"candidates": ["1.2.3.4:5"]})),
        )
        .await;
        assert!(missing.is_err());
    }
}
