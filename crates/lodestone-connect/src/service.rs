//! The client for the introduction service (connect.lodestone.cc, `services/connect` in
//! this repository). It passes candidate addresses between a host and the friends who
//! open its invite, and nothing else. Its README describes the protocol.

use std::{net::SocketAddr, pin::Pin, sync::Arc, time::Duration};

use serde_json::{json, Value};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::TcpStream,
};

use crate::{Error, Result};

/// The introduction service Lodestone uses.
pub const DEFAULT_SERVICE: &str = "https://connect.lodestone.cc";
/// The most of an answer read.
const MAX_ANSWER: u64 = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(15);

/// Where the introduction service is: a domain over HTTPS (connect.lodestone.cc), or
/// `ip:port` over plain HTTP (a test service).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Service {
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl Default for Service {
    /// connect.lodestone.cc.
    fn default() -> Self {
        Self::parse(DEFAULT_SERVICE).expect("valid")
    }
}

impl Service {
    /// `https://name.example`, `http://1.2.3.4:8787`, `1.2.3.4:8787` or `name.example`.
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

    /// The service as an `lhp2://` link carries it: an `ip:port` is plain HTTP, a name
    /// is HTTPS.
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

    pub(crate) fn host_port(text: &str, default_port: u16, tls: bool) -> Option<Self> {
        if text.is_empty()
            || text.contains(['/', '@', '?', '#', ' '])
            || !text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
        {
            return None;
        }
        let (host, port) = match text.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') || host.ends_with(']') => {
                (host.to_string(), port.parse().ok()?)
            }
            _ => (text.to_string(), default_port),
        };
        let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
        if host.is_empty() || port == 0 {
            return None;
        }
        Some(Self { host, port, tls })
    }

    /// `https://connect.lodestone.cc`, or `http://127.0.0.1:8787` for a test service.
    pub fn base_url(&self) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{}", self.authority())
    }

    /// How an `lhp2://` link writes it.
    pub fn link(&self) -> String {
        match (self.tls, self.port) {
            (true, 443) => self.bracketed(),
            _ => format!("{}:{}", self.bracketed(), self.port),
        }
    }

    /// The Host header's value.
    pub(crate) fn authority(&self) -> String {
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

    /// `GET /v1/health`.
    pub async fn health(&self) -> Result<Value> {
        call(self, "GET", "/v1/health", None, None).await
    }

    /// Joins invite `code` with this side's candidates; returns the host's.
    pub async fn join(&self, code: &str, candidates: &[SocketAddr]) -> Result<Vec<SocketAddr>> {
        let answer = call(
            self,
            "POST",
            &format!("/v1/invites/{code}/joins"),
            None,
            Some(json!({ "candidates": strings(candidates) })),
        )
        .await?;
        let host = addrs_of(&answer["host"]);
        if host.is_empty() {
            return Err(Error::Unreachable(
                "the service named no host address".into(),
            ));
        }
        Ok(host)
    }
}

fn strings(addrs: &[SocketAddr]) -> Vec<String> {
    addrs.iter().map(ToString::to_string).collect()
}

pub(crate) fn addrs_of(value: &Value) -> Vec<SocketAddr> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| a.as_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// A connection to the service: TLS for a named service, plain TCP for a test one.
pub(crate) async fn connect(service: &Service) -> Result<Pin<Box<dyn Io>>> {
    let unreachable = |what: String| Error::Unreachable(what);
    let tcp = tokio::time::timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect((service.host.as_str(), service.port)),
    )
    .await
    .map_err(|_| unreachable("no answer".into()))?
    .map_err(|err| unreachable(err.to_string()))?;
    if !service.tls {
        return Ok(Box::pin(tcp));
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(crate::tunnel::provider())
        .with_safe_default_protocol_versions()
        .map_err(|err| unreachable(err.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(service.host.clone())
        .map_err(|err| unreachable(err.to_string()))?;
    let tls = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp),
    )
    .await
    .map_err(|_| unreachable("no answer".into()))?
    .map_err(|err| unreachable(err.to_string()))?;
    Ok(Box::pin(tls))
}

/// One request to the service, with the invite's secret if given; returns the JSON body
/// of a 2xx answer, or [`Error::Service`] with the service's error code.
pub(crate) async fn call(
    service: &Service,
    method: &str,
    path: &str,
    secret: Option<&str>,
    body: Option<Value>,
) -> Result<Value> {
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let authorization = secret
        .map(|secret| format!("Authorization: Bearer {secret}\r\n"))
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\n{authorization}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nUser-Agent: lodestone-connect\r\nX-Lodestone-Client: 1\r\n\r\n{body}",
        service.authority(),
        body.len()
    );
    let mut stream = connect(service).await?;
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|err| Error::Unreachable(err.to_string()))?;
    let mut response = Vec::new();
    // Some servers end TLS without a close_notify; what arrived before still counts.
    let _ = tokio::time::timeout(
        ANSWER_TIMEOUT,
        (&mut stream).take(MAX_ANSWER).read_to_end(&mut response),
    )
    .await;
    let (status, body) = crate::info::parse_response(&response)
        .ok_or_else(|| Error::Unreachable("no answer".into()))?;
    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("no reason given")
            .to_string();
        let code = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        return Err(Error::Service {
            status,
            code,
            message,
        });
    }
    Ok(value)
}

#[cfg(feature = "host")]
pub use host::*;

#[cfg(feature = "host")]
mod host {
    use futures_util::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::{
        tungstenite::{self, client::IntoClientRequest as _, http::HeaderValue, Message},
        WebSocketStream,
    };

    use super::*;

    /// How often a waiting host says it's still there.
    pub(crate) const PING_EVERY: Duration = Duration::from_secs(30);
    /// The close code for "the invite has ended".
    pub const CLOSE_ENDED: u16 = 4000;
    /// The close code for "the invite was renewed": wait on it again from join 0.
    pub const CLOSE_RENEWED: u16 = 4001;
    /// The largest WebSocket message or frame taken from the service. Its messages (a
    /// knock, a few joins) are a few KiB at most; tungstenite's own limit is 64 MiB.
    pub(crate) const MAX_SERVICE_MESSAGE: usize = 64 * 1024;

    /// An invite the service holds for this host.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Registered {
        pub code: String,
        /// Proves to the service that this is the invite's host. Never shared.
        pub secret: String,
        /// The service keeps the code for this host after the invite ends, so the same
        /// link works again once the host renews it. False on a service without stable
        /// links (the host then gets a new link when an invite ends).
        pub kept: bool,
    }

    impl Service {
        /// Registers an invite. With `keep`, asks the service to keep its code for this
        /// host after the invite ends (a stable link).
        pub async fn create(&self, candidates: &[SocketAddr], keep: bool) -> Result<Registered> {
            let mut body = json!({ "candidates": strings(candidates) });
            if keep {
                body["keep"] = json!(true);
            }
            let answer = call(self, "POST", "/v1/invites", None, Some(body)).await?;
            registered(&answer, None)
        }

        /// Renews a kept invite with fresh candidates, or opens it again after it ended:
        /// the same code and secret, a new day, no joins yet. A 404 error means the
        /// service no longer keeps the code (or doesn't know renewing): create a new one.
        pub async fn renew(
            &self,
            code: &str,
            secret: &str,
            candidates: &[SocketAddr],
        ) -> Result<Registered> {
            let answer = call(
                self,
                "PUT",
                &format!("/v1/invites/{code}"),
                Some(secret),
                Some(json!({ "candidates": strings(candidates) })),
            )
            .await?;
            registered(&answer, Some((code, secret)))
        }

        /// Ends the invite, and stops the service keeping its code.
        pub async fn end(&self, code: &str, secret: &str) -> Result<()> {
            call(
                self,
                "DELETE",
                &format!("/v1/invites/{code}"),
                Some(secret),
                None,
            )
            .await
            .map(|_| ())
        }

        /// Waits on an invite for the joins after the `after`th, over a WebSocket.
        /// `None` if the invite has ended.
        pub async fn watch(&self, code: &str, secret: &str, after: u64) -> Result<Option<Watch>> {
            let path = format!("/v1/invites/{code}/joins?after={after}");
            Ok(self.websocket(&path, secret).await?.map(|ws| {
                let mut ping = tokio::time::interval(PING_EVERY);
                ping.reset();
                Watch { ws, ping }
            }))
        }

        /// Opens a WebSocket to `path` with `secret` as its Bearer. `None` on a 404.
        pub(crate) async fn websocket(
            &self,
            path: &str,
            secret: &str,
        ) -> Result<Option<WebSocketStream<Pin<Box<dyn Io>>>>> {
            let scheme = if self.tls { "wss" } else { "ws" };
            let url = format!("{scheme}://{}{path}", self.authority());
            let unreachable = |err: String| Error::Unreachable(err);
            let mut request = url
                .into_client_request()
                .map_err(|err| unreachable(err.to_string()))?;
            request.headers_mut().insert(
                "authorization",
                HeaderValue::from_str(&format!("Bearer {secret}"))
                    .map_err(|err| unreachable(err.to_string()))?,
            );
            let stream = connect(self).await?;
            match tokio::time::timeout(
                ANSWER_TIMEOUT,
                tokio_tungstenite::client_async_with_config(
                    request,
                    stream,
                    Some(
                        tungstenite::protocol::WebSocketConfig::default()
                            .max_message_size(Some(MAX_SERVICE_MESSAGE))
                            .max_frame_size(Some(MAX_SERVICE_MESSAGE)),
                    ),
                ),
            )
            .await
            .map_err(|_| unreachable("no answer".into()))?
            {
                Ok((ws, _)) => Ok(Some(ws)),
                Err(tungstenite::Error::Http(response)) if response.status() == 404 => Ok(None),
                Err(tungstenite::Error::Http(response)) => {
                    let status = response.status().as_u16();
                    let body: Value = response
                        .body()
                        .as_deref()
                        .and_then(|body| serde_json::from_slice(body).ok())
                        .unwrap_or(Value::Null);
                    Err(Error::Service {
                        status,
                        code: body["error"].as_str().unwrap_or("unknown").to_string(),
                        message: body["message"]
                            .as_str()
                            .unwrap_or("no reason given")
                            .to_string(),
                    })
                }
                Err(err) => Err(unreachable(err.to_string())),
            }
        }
    }

    fn registered(answer: &Value, known: Option<(&str, &str)>) -> Result<Registered> {
        let field = |name: &str, known: Option<&str>| {
            answer[name]
                .as_str()
                .or(known)
                .map(str::to_string)
                .ok_or_else(|| Error::Unreachable(format!("the service sent no {name}")))
        };
        Ok(Registered {
            code: field("code", known.map(|k| k.0))?,
            secret: field("secret", known.map(|k| k.1))?,
            kept: answer["kept_until"].is_string(),
        })
    }

    /// What a waiting host hears.
    #[derive(Debug, PartialEq)]
    pub enum Event {
        /// Friends who joined: their join number and candidate addresses.
        Joins(Vec<(u64, Vec<SocketAddr>)>),
        /// The invite has ended (deleted, quiet too long, or a day old).
        Ended,
        /// The invite was renewed: wait on it again from join 0.
        Renewed,
    }

    /// A host's WebSocket, waiting on its invite.
    pub struct Watch {
        ws: WebSocketStream<Pin<Box<dyn Io>>>,
        ping: tokio::time::Interval,
    }

    impl Watch {
        /// The next joins, or the end of the invite. Keeps the socket alive meanwhile.
        pub async fn next(&mut self) -> Result<Event> {
            let lost = |err: String| Error::Unreachable(err);
            loop {
                tokio::select! {
                    _ = self.ping.tick() => self.ws.send(Message::text("ping")).await.map_err(|err| lost(err.to_string()))?,
                    message = self.ws.next() => match message.ok_or_else(|| lost("the service closed the connection".into()))?.map_err(|err| lost(err.to_string()))? {
                        Message::Text(text) => {
                            if text.as_str() == "pong" {
                                continue;
                            }
                            let value: Value = serde_json::from_str(text.as_str()).map_err(|err| lost(err.to_string()))?;
                            let joins = value["joins"]
                                .as_array()
                                .ok_or_else(|| lost("a garbled message from the service".into()))?
                                .iter()
                                .filter_map(|join| Some((join["id"].as_u64()?, addrs_of(&join["candidates"]))))
                                .collect();
                            return Ok(Event::Joins(joins));
                        }
                        Message::Close(Some(frame)) if u16::from(frame.code) == CLOSE_ENDED => {
                            return Ok(Event::Ended);
                        }
                        Message::Close(Some(frame)) if u16::from(frame.code) == CLOSE_RENEWED => {
                            return Ok(Event::Renewed);
                        }
                        Message::Close(frame) => {
                            return Err(lost(format!("the service closed the connection ({frame:?})")));
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn services_parse_and_print() {
        let http = Service::parse("http://16.146.26.94:8080").unwrap();
        assert_eq!(
            (http.tls, http.port, http.link(), http.base_url()),
            (
                false,
                8080,
                "16.146.26.94:8080".to_string(),
                "http://16.146.26.94:8080".to_string()
            )
        );
        let worker = Service::parse("https://connect.lodestone.cc/").unwrap();
        assert_eq!(worker, Service::default());
        assert_eq!(
            (worker.tls, worker.port, worker.link(), worker.base_url()),
            (
                true,
                443,
                "connect.lodestone.cc".to_string(),
                "https://connect.lodestone.cc".to_string()
            )
        );
        assert_eq!(worker.authority(), "connect.lodestone.cc");
        assert_eq!(Service::from_link(&worker.link()), Some(worker));
        assert_eq!(Service::from_link("16.146.26.94:8080"), Some(http));
        assert!(Service::parse("https://evil.example@x").is_none());
        assert!(Service::parse("https://a b").is_none());
        let v6 = Service::parse("http://[::1]:8787").unwrap();
        assert_eq!(v6.base_url(), "http://[::1]:8787");
    }
}
