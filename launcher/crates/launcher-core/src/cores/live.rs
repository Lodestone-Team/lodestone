//! A server's live view: its state and players from the core's event stream, its
//! console from the console stream and buffer, sending chat and commands, and linking
//! the player's Minecraft account to their Lodestone user.
//!
//! The streams are the core's WebSocket routes, authenticated with this device's key
//! as the dashboard authenticates them: `/events/all/stream?filter={"bearer_token":…}`
//! and `/instance/{uuid}/console/stream?token=Bearer …`. Each runs on its own thread
//! over a blocking socket (TLS through rustls, with the platform verifier as for the
//! HTTP client), reconnects with backoff when it drops, and stops for good when the
//! core refuses the key. The key is never logged, and neither is any URL that holds it.
//!
//! What the key may do with a server comes from `GET /launcher/instances/{uuid}/access`:
//! a launcher key reads and writes the console only if its user opted in at sign-in,
//! and only where the user may use the console.

use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use futures::channel::mpsc::UnboundedSender;
use http_client::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::{
    HandshakeError, Message, WebSocket, client::IntoClientRequest as _, handshake::client::Response,
};
use url::Url;

use super::{
    CoreClient, CoreError, ServerState,
    client::{REQUEST_TIMEOUT, segment},
    tls::{CertificatePin, PinVerifier},
};
use crate::{Secret, USER_AGENT};

/// The console access to ask for when signing in. Both are off unless the user turns
/// them on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LauncherScope {
    pub console_read: bool,
    pub console_write: bool,
}

impl LauncherScope {
    /// The `scope` of `POST /user/api_keys`. Without opt-ins it is exactly the scope
    /// older launchers send.
    pub fn to_json(self) -> Value {
        let mut scope = json!({ "type": "launcher" });
        if self.console_read {
            scope["console_read"] = json!(true);
        }
        if self.console_write {
            scope["console_write"] = json!(true);
        }
        scope
    }
}

/// A key's scope as the core reports it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyScope {
    Full,
    Custom {},
    Launcher {
        #[serde(default)]
        console_read: bool,
        #[serde(default)]
        console_write: bool,
    },
    #[serde(other)]
    Other,
}

/// What this device's key may do with one server, from
/// `GET /launcher/instances/{uuid}/access`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct LauncherAccess {
    #[serde(default)]
    pub view: bool,
    #[serde(default)]
    pub start: bool,
    #[serde(default)]
    pub stop: bool,
    #[serde(default)]
    pub console_read: bool,
    #[serde(default)]
    pub console_write: bool,
}

/// A player on a server.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Player {
    pub name: String,
    #[serde(default)]
    pub uuid: Option<String>,
}

/// One console line, as the core sent it (with its colour codes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    /// The event's id: increasing, and the same in the buffer and the stream.
    pub snowflake: i64,
    pub message: String,
}

/// A Minecraft account linked to the key's Lodestone user.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct MinecraftLink {
    pub minecraft_uuid: String,
    pub minecraft_name: String,
    #[serde(default)]
    pub verified: bool,
}

/// The most console lines kept, as in the dashboard.
pub const MAX_CONSOLE_LINES: usize = 5000;
/// The longest chat message or command sent.
pub const MAX_COMMAND_CHARS: usize = 1000;

/// The command for what was typed in the chat box: `/command` runs the command, other
/// text is said to everyone with `say`. `None` for nothing to send. Line breaks and
/// other control characters never reach the server's console.
pub fn chat_command(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = cleaned.trim();
    let command = match text.strip_prefix('/') {
        Some(command) => command.trim().to_string(),
        None if text.is_empty() => return None,
        None => format!("say {text}"),
    };
    if command.is_empty() {
        return None;
    }
    Some(command.chars().take(MAX_COMMAND_CHARS).collect())
}

/// Adds `new` lines to `log`, in snowflake order without repeats, keeping the newest
/// [`MAX_CONSOLE_LINES`].
pub fn merge_console(log: &mut Vec<ConsoleLine>, new: impl IntoIterator<Item = ConsoleLine>) {
    for line in new {
        match log.last() {
            Some(last) if last.snowflake < line.snowflake => log.push(line),
            None => log.push(line),
            _ => {
                if let Err(at) = log.binary_search_by_key(&line.snowflake, |l| l.snowflake) {
                    log.insert(at, line);
                }
            }
        }
    }
    if log.len() > MAX_CONSOLE_LINES {
        log.drain(..log.len() - MAX_CONSOLE_LINES);
    }
}

// ---- What the core sends ----

#[derive(Deserialize)]
struct WireEvent {
    event_inner: WireInner,
    #[serde(default)]
    snowflake: Value,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WireInner {
    InstanceEvent(WireInstanceEvent),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct WireInstanceEvent {
    instance_uuid: String,
    instance_event_inner: WireInstanceInner,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WireInstanceInner {
    StateTransition {
        to: ServerState,
    },
    PlayerChange {
        #[serde(default)]
        player_list: Vec<Player>,
    },
    InstanceOutput {
        message: String,
    },
    #[serde(other)]
    Other,
}

fn snowflake_of(value: &Value) -> Option<i64> {
    match value {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

/// What a frame of a live stream means for `instance`; `None` for anything else.
pub fn parse_frame(text: &str, instance: &str) -> Option<LiveMessage> {
    let event: WireEvent = serde_json::from_str(text).ok()?;
    let WireInner::InstanceEvent(inner) = event.event_inner else {
        return None;
    };
    if inner.instance_uuid != instance {
        return None;
    }
    Some(match inner.instance_event_inner {
        WireInstanceInner::StateTransition { to } => LiveMessage::State(to),
        WireInstanceInner::PlayerChange { mut player_list } => {
            player_list.sort_by_key(|p| p.name.to_lowercase());
            LiveMessage::Players(player_list)
        }
        WireInstanceInner::InstanceOutput { message } => LiveMessage::Output(ConsoleLine {
            snowflake: snowflake_of(&event.snowflake)?,
            message,
        }),
        WireInstanceInner::Other => return None,
    })
}

/// The console lines in a `GET /instance/{uuid}/console/buffer` reply.
pub(crate) fn parse_buffer(events: Vec<Value>, instance: &str) -> Vec<ConsoleLine> {
    let mut lines: Vec<ConsoleLine> = events
        .into_iter()
        .filter_map(|event| match parse_frame(&event.to_string(), instance) {
            Some(LiveMessage::Output(line)) => Some(line),
            _ => None,
        })
        .collect();
    lines.sort_by_key(|l| l.snowflake);
    lines.dedup_by_key(|l| l.snowflake);
    lines
}

// ---- REST ----

impl CoreClient {
    /// What `key` may do with `server`. `None` from a core without the route (or
    /// without the server): treat that as the list's view, start and stop only.
    pub async fn access(
        &self,
        key: &Secret,
        server: &str,
    ) -> Result<Option<LauncherAccess>, CoreError> {
        let path = format!("launcher/instances/{}/access", segment(server)?);
        match self.json_with_key(Method::GET, &path, key, None).await {
            Ok(access) => Ok(Some(access)),
            Err(CoreError::NotFound(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// The console lines the core keeps for `server`, oldest first.
    pub async fn console_buffer(
        &self,
        key: &Secret,
        server: &str,
    ) -> Result<Vec<ConsoleLine>, CoreError> {
        let path = format!("instance/{}/console/buffer", segment(server)?);
        let events: Vec<Value> = self.json_with_key(Method::GET, &path, key, None).await?;
        Ok(parse_buffer(events, server))
    }

    /// Runs `command` (without a leading `/`) on `server`'s console.
    pub async fn send_command(
        &self,
        key: &Secret,
        server: &str,
        command: &str,
    ) -> Result<(), CoreError> {
        let path = format!("instance/{}/console", segment(server)?);
        let body = serde_json::to_vec(&command).map_err(|e| CoreError::Invalid(e.to_string()))?;
        let _: Value = self
            .json_with_key(Method::POST, &path, key, Some(body))
            .await?;
        Ok(())
    }

    /// The Minecraft accounts linked to the key's user.
    pub async fn minecraft_links(&self, key: &Secret) -> Result<Vec<MinecraftLink>, CoreError> {
        self.json_with_key(Method::GET, "user/minecraft_links", key, None)
            .await
            .map_err(|err| match err {
                CoreError::NotFound(_) => CoreError::TooOld,
                err => err,
            })
    }

    /// Links the Minecraft account `uuid`/`name` to the key's user. `access_token` is
    /// the account's Minecraft token: the core sends it once to Minecraft's services to
    /// check that it is this account's, and never keeps it.
    pub async fn link_minecraft_account(
        &self,
        key: &Secret,
        uuid: &str,
        name: &str,
        access_token: &Secret,
    ) -> Result<MinecraftLink, CoreError> {
        let body = json!({
            "minecraft_uuid": uuid,
            "minecraft_name": name,
            "access_token": access_token.expose(),
        });
        self.json_with_key(
            Method::POST,
            "user/minecraft_links",
            key,
            Some(body.to_string().into_bytes()),
        )
        .await
        .map_err(|err| match err {
            CoreError::NotFound(_) => CoreError::TooOld,
            err => err,
        })
    }

    /// Removes the link of the Minecraft account `uuid` from the key's user.
    pub async fn unlink_minecraft_account(
        &self,
        key: &Secret,
        uuid: &str,
    ) -> Result<(), CoreError> {
        let path = format!("user/minecraft_links/{}", segment(uuid)?);
        let _: Value = self.json_with_key(Method::DELETE, &path, key, None).await?;
        Ok(())
    }
}

// ---- Streams ----

/// Which stream to follow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamKind {
    /// State changes and the player list of one server.
    Events,
    /// One server's console output.
    Console,
    /// Every event the key may see, from every server and the core itself, as raw
    /// frames ([`LiveMessage::Frame`]): Lodestone Host's whole view of its core. The
    /// `server` given to [`LiveStream::open`] is not used.
    All,
    /// One server's CPU and memory, a report a second (`/monitor/{uuid}`), as raw
    /// frames ([`LiveMessage::Frame`]). The core replays the last minute on connecting.
    Monitor,
}

/// What a stream reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveMessage {
    /// Connected, or connected again: refetch whatever may have been missed.
    Connected,
    /// The connection dropped; it is tried again. The reason is for people.
    Disconnected(String),
    State(ServerState),
    Players(Vec<Player>),
    Output(ConsoleLine),
    /// A frame of a [`StreamKind::All`] stream, as the core sent it (a `ClientEvent`).
    Frame(String),
    /// The core refused the key (signed out, or no longer allowed). Not retried.
    Ended(CoreError),
}

/// How long a read waits before the stream checks whether it should stop, once
/// connected. The handshake reads with [`REQUEST_TIMEOUT`] instead (see [`open_tcp`]).
const READ_TICK: Duration = Duration::from_millis(250);
/// After this long without a frame, the stream sends a ping, and one write failing
/// ends the connection.
const PING_AFTER: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// A stream following a server, on its own thread. Dropping it stops the thread.
pub struct LiveStream {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LiveStream {
    /// Follows `kind` for `server` on the core at `base`, sending what happens to `tx`.
    /// It stops when dropped, when `tx`'s receiver is gone, or when the core refuses
    /// the key.
    pub fn open(
        client: &CoreClient,
        key: Secret,
        server: &str,
        kind: StreamKind,
        tx: UnboundedSender<LiveMessage>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            let target = Target {
                base: client.base().clone(),
                pin: client.pin().cloned(),
                strict: client.is_strict(),
            };
            let server = server.to_string();
            std::thread::Builder::new()
                .name("lodestone-core-stream".into())
                .spawn(move || run(&target, &key, &server, &kind, &tx, &stop))
                .ok()
        };
        Self { stop, thread }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for LiveStream {
    fn drop(&mut self) {
        self.stop();
        // the thread notices within a read tick; don't wait for it on the UI thread
        drop(self.thread.take());
    }
}

/// The stream's path and query below `/api/v1/`. Holds the key: never log it.
fn stream_path(key: &Secret, server: &str, kind: &StreamKind) -> String {
    let encode = |value: &str| -> String {
        url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
    };
    match kind {
        StreamKind::Events => {
            let filter = json!({
                "bearer_token": key.expose(),
                "event_instance_ids": [server],
                "instance_event_types": ["StateTransition", "PlayerChange"],
            });
            format!("events/all/stream?filter={}", encode(&filter.to_string()))
        }
        StreamKind::Console => format!(
            "instance/{server}/console/stream?token={}",
            encode(&format!("Bearer {}", key.expose()))
        ),
        StreamKind::All => {
            let filter = json!({ "bearer_token": key.expose() });
            format!("events/all/stream?filter={}", encode(&filter.to_string()))
        }
        StreamKind::Monitor => format!(
            "monitor/{server}?token={}",
            encode(&format!("Bearer {}", key.expose()))
        ),
    }
}

/// `ws://` or `wss://` for a core address, with `path` below `/api/v1/`.
pub fn ws_url(base: &Url, path: &str) -> Result<Url, CoreError> {
    let scheme = match base.scheme() {
        "http" => "ws",
        "https" => "wss",
        other => {
            return Err(CoreError::InvalidAddress(format!(
                "{other}:// isn't a core address"
            )));
        }
    };
    let raw = format!(
        "{}/api/v1/{}",
        base.as_str().trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let mut url = Url::parse(&raw)
        .map_err(|_| CoreError::InvalidAddress("That isn't a core address".into()))?;
    url.set_scheme(scheme)
        .map_err(|()| CoreError::InvalidAddress("That isn't a core address".into()))?;
    Ok(url)
}

/// Where a stream connects: the core's address, and the certificate the user trusted
/// for it, if any.
struct Target {
    base: Url,
    pin: Option<CertificatePin>,
    /// Only the pinned certificate (see [`CoreClient::strict`]).
    strict: bool,
}

fn run(
    target: &Target,
    key: &Secret,
    server: &str,
    kind: &StreamKind,
    tx: &UnboundedSender<LiveMessage>,
    stop: &AtomicBool,
) {
    let stopped = || stop.load(Ordering::SeqCst) || tx.is_closed();
    let mut backoff = Duration::from_secs(1);
    while !stopped() {
        let reason = match connect(target, key, server, kind) {
            Ok(socket) => {
                backoff = Duration::from_secs(1);
                if tx.unbounded_send(LiveMessage::Connected).is_err() {
                    return;
                }
                follow(socket, server, kind, tx, stop)
            }
            Err(
                err @ (CoreError::SignedOut
                | CoreError::Forbidden(_)
                | CoreError::InvalidAddress(_)
                | CoreError::Untrusted { .. }
                | CoreError::CertificateChanged { .. }),
            ) => {
                let _ = tx.unbounded_send(LiveMessage::Ended(err));
                return;
            }
            Err(err) => err.to_string(),
        };
        if stopped() {
            return;
        }
        if tx
            .unbounded_send(LiveMessage::Disconnected(reason))
            .is_err()
        {
            return;
        }
        let until = Instant::now() + backoff;
        while Instant::now() < until {
            if stopped() {
                return;
            }
            std::thread::sleep(READ_TICK.min(until.saturating_duration_since(Instant::now())));
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// Reads frames until the connection ends; returns why, for people.
fn follow(
    mut socket: WebSocket<Stream>,
    server: &str,
    kind: &StreamKind,
    tx: &UnboundedSender<LiveMessage>,
    stop: &AtomicBool,
) -> String {
    let mut last_frame = Instant::now();
    let mut pinged = false;
    loop {
        if stop.load(Ordering::SeqCst) || tx.is_closed() {
            let _ = socket.close(None);
            let _ = socket.flush();
            return "Stopped".into();
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                last_frame = Instant::now();
                pinged = false;
                if matches!(kind, StreamKind::All | StreamKind::Monitor) {
                    if tx
                        .unbounded_send(LiveMessage::Frame(text.as_str().to_string()))
                        .is_err()
                    {
                        return "Stopped".into();
                    }
                    continue;
                }
                let Some(message) = parse_frame(text.as_str(), server) else {
                    continue;
                };
                let wanted = matches!(
                    (&message, kind),
                    (LiveMessage::Output(_), StreamKind::Console)
                        | (
                            LiveMessage::State(_) | LiveMessage::Players(_),
                            StreamKind::Events
                        )
                );
                if wanted && tx.unbounded_send(message).is_err() {
                    return "Stopped".into();
                }
            }
            Ok(Message::Close(_)) => return "The core closed the connection.".into(),
            Ok(_) => {
                last_frame = Instant::now();
                pinged = false;
            }
            Err(tungstenite::Error::Io(err))
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if last_frame.elapsed() > PING_AFTER {
                    if pinged {
                        return "The core stopped answering.".into();
                    }
                    pinged = true;
                    last_frame = Instant::now();
                    if socket.send(Message::Ping(Vec::new().into())).is_err() {
                        return "The connection dropped.".into();
                    }
                }
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return "The core closed the connection.".into();
            }
            Err(_) => return "The connection dropped.".into(),
        }
    }
}

/// A plain or TLS socket.
pub enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => s.get_ref(),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn is_certificate_error(err: &io::Error) -> bool {
    let lower = err.to_string().to_ascii_lowercase();
    lower.contains("certificate") || lower.contains("unknownissuer")
}

fn io_error(err: &io::Error, host: &str) -> CoreError {
    if is_certificate_error(err) {
        CoreError::Untrusted {
            host: host.to_string(),
            fingerprint: None,
        }
    } else {
        CoreError::Unreachable(err.to_string())
    }
}

/// The core's refusal of a handshake, from its status and JSON error body.
fn refusal(response: &Response) -> CoreError {
    let status = response.status().as_u16();
    let message = response
        .body()
        .as_deref()
        .and_then(|body| serde_json::from_slice::<Value>(body).ok())
        .and_then(|body| body["causes"][0].as_str().map(str::to_string))
        .unwrap_or_default();
    match status {
        401 => CoreError::SignedOut,
        403 => CoreError::Forbidden(if message.is_empty() {
            "The core refused: you don't have permission to do that.".into()
        } else {
            message
        }),
        404 => CoreError::NotFound(if message.is_empty() {
            "Not found".into()
        } else {
            message
        }),
        _ => CoreError::Server { status, message },
    }
}

/// The TLS settings to try, in order: the pinned certificate first, as the HTTP client
/// does, then the platform verifier (a certificate macOS trusts now still works). A
/// strict target tries only the pinned certificate.
fn tls_configs(pin: Option<&CertificatePin>, strict: bool) -> Vec<Arc<rustls::ClientConfig>> {
    let mut configs = Vec::new();
    if let Some(pin) = pin {
        // the provider Zed's own TLS configuration uses, as `PinnedHttp` does
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let verifier = Arc::new(PinVerifier::new(Some(pin.clone()), provider.clone()));
        if let Ok(builder) = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
        {
            configs.push(Arc::new(
                builder
                    .dangerous()
                    .with_custom_certificate_verifier(verifier)
                    .with_no_client_auth(),
            ));
        }
    }
    if !strict {
        configs.push(Arc::new(http_client_tls::tls_config()));
    }
    configs
}

fn open_tcp(base: &Url, host: &str) -> Result<TcpStream, CoreError> {
    let addrs = base
        .socket_addrs(|| None)
        .map_err(|err| CoreError::Unreachable(err.to_string()))?;
    let mut last_error = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(tcp) => {
                let _ = tcp.set_nodelay(true);
                // The TLS and WebSocket handshakes wait as long as any request.
                // Not the short read tick yet: Windows reports a timed-out read as
                // `TimedOut`, which tungstenite and rustls treat as a failure, not
                // as "try again", so a core more than a tick away could never be
                // reached. `handshake` sets the tick once connected.
                tcp.set_read_timeout(Some(REQUEST_TIMEOUT))
                    .map_err(|err| io_error(&err, host))?;
                tcp.set_write_timeout(Some(REQUEST_TIMEOUT))
                    .map_err(|err| io_error(&err, host))?;
                return Ok(tcp);
            }
            Err(err) => last_error = Some(err),
        }
    }
    Err(match last_error {
        Some(err) => io_error(&err, host),
        None => CoreError::Unreachable("The address didn't resolve".into()),
    })
}

fn server_name(
    host: &url::Host<&str>,
) -> Result<rustls::pki_types::ServerName<'static>, CoreError> {
    Ok(match host {
        url::Host::Domain(domain) => rustls::pki_types::ServerName::try_from(domain.to_string())
            .map_err(|_| CoreError::InvalidAddress("That isn't a host name".into()))?,
        url::Host::Ipv4(ip) => {
            rustls::pki_types::ServerName::IpAddress(std::net::IpAddr::V4(*ip).into())
        }
        url::Host::Ipv6(ip) => {
            rustls::pki_types::ServerName::IpAddress(std::net::IpAddr::V6(*ip).into())
        }
    })
}

fn connect(
    target: &Target,
    key: &Secret,
    server: &str,
    kind: &StreamKind,
) -> Result<WebSocket<Stream>, CoreError> {
    segment(server)?;
    let base = &target.base;
    if target.strict && (base.scheme() != "https" || target.pin.is_none()) {
        // the key goes only to the pinned certificate
        return Err(CoreError::Untrusted {
            host: base.host_str().unwrap_or_default().to_string(),
            fingerprint: None,
        });
    }
    let url = ws_url(base, &stream_path(key, server, kind))?;
    let host = base
        .host()
        .ok_or_else(|| CoreError::InvalidAddress("The core address has no host".into()))?;
    let host_label = base.host_str().unwrap_or_default().to_string();
    let configs: Vec<Option<Arc<rustls::ClientConfig>>> = if base.scheme() == "https" {
        tls_configs(target.pin.as_ref(), target.strict)
            .into_iter()
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    let mut result = Err(CoreError::Unreachable("No way to connect".into()));
    for config in configs {
        let tcp = open_tcp(base, &host_label)?;
        let stream = match config {
            Some(config) => {
                let connection = rustls::ClientConnection::new(config, server_name(&host)?)
                    .map_err(|err| CoreError::Unreachable(err.to_string()))?;
                Stream::Tls(Box::new(rustls::StreamOwned::new(connection, tcp)))
            }
            None => Stream::Plain(tcp),
        };
        result = handshake(&url, stream, &host_label);
        // the next TLS settings may accept the certificate
        if !matches!(result, Err(CoreError::Untrusted { .. })) {
            return result;
        }
    }
    result
}

fn handshake(url: &Url, stream: Stream, host: &str) -> Result<WebSocket<Stream>, CoreError> {
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|_| CoreError::InvalidAddress("That isn't a core address".into()))?;
    if let Ok(agent) = USER_AGENT.parse() {
        request.headers_mut().insert("User-Agent", agent);
    }
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut attempt = tungstenite::client::client(request, stream);
    loop {
        match attempt {
            Ok((socket, _)) => {
                // From here on reads wake up every tick, so the stream can stop
                // and send pings; `follow` takes `TimedOut` as a tick too.
                socket
                    .get_ref()
                    .tcp()
                    .set_read_timeout(Some(READ_TICK))
                    .map_err(|err| io_error(&err, host))?;
                return Ok(socket);
            }
            Err(HandshakeError::Interrupted(mid)) => {
                if Instant::now() > deadline {
                    return Err(CoreError::Unreachable(
                        "The core didn't answer in time".into(),
                    ));
                }
                attempt = mid.handshake();
            }
            Err(HandshakeError::Failure(tungstenite::Error::Http(response))) => {
                return Err(refusal(&response));
            }
            Err(HandshakeError::Failure(tungstenite::Error::Io(err))) => {
                return Err(io_error(&err, host));
            }
            // never the error's text: it can quote the request, whose URL holds the key
            Err(HandshakeError::Failure(_)) => {
                return Err(CoreError::Invalid("the WebSocket handshake failed".into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handshake_waits_longer_than_a_read_tick_then_reads_in_ticks() {
        use std::{io::BufRead as _, net::TcpListener};

        // A core that answers the upgrade only after more than a read tick, as a
        // distant or busy one does.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let core = std::thread::spawn(move || {
            let (tcp, _) = listener.accept().unwrap();
            let mut reader = io::BufReader::new(tcp.try_clone().unwrap());
            let mut key = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("sec-websocket-key")
                {
                    key = value.trim().to_string();
                }
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            std::thread::sleep(READ_TICK + Duration::from_millis(300));
            let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
            let mut tcp = tcp;
            write!(
                tcp,
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\
                 Connection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            )
            .unwrap();
            // keep the connection open until the client has read the reply
            std::thread::sleep(Duration::from_millis(200));
        });

        let base = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let tcp = open_tcp(&base, "127.0.0.1").unwrap();
        // Windows fails a read that times out instead of retrying it, so the
        // handshake must not read with the short tick.
        assert_eq!(tcp.read_timeout().unwrap(), Some(REQUEST_TIMEOUT));
        let url = ws_url(&base, "events/all").unwrap();
        let socket = handshake(&url, Stream::Plain(tcp), "127.0.0.1").unwrap();
        assert_eq!(socket.get_ref().tcp().read_timeout().unwrap(), Some(READ_TICK));
        core.join().unwrap();
    }

    #[test]
    fn chat_says_text_and_runs_slash_commands() {
        assert_eq!(chat_command("hello all").as_deref(), Some("say hello all"));
        assert_eq!(
            chat_command("  /time set day ").as_deref(),
            Some("time set day")
        );
        assert_eq!(chat_command("/"), None);
        assert_eq!(chat_command("   "), None);
        // a line break can't start a second command
        assert_eq!(
            chat_command("hi\nop someone").as_deref(),
            Some("say hi op someone")
        );
        assert_eq!(
            chat_command(&"x".repeat(5000)).map(|c| c.chars().count()),
            Some(MAX_COMMAND_CHARS)
        );
    }

    #[test]
    fn the_scope_asks_only_for_what_was_chosen() {
        assert_eq!(
            LauncherScope::default().to_json(),
            json!({ "type": "launcher" })
        );
        assert_eq!(
            LauncherScope {
                console_read: true,
                console_write: false
            }
            .to_json(),
            json!({ "type": "launcher", "console_read": true })
        );
        let parse = |v: Value| serde_json::from_value::<KeyScope>(v).unwrap();
        assert_eq!(
            parse(json!({ "type": "launcher" })),
            KeyScope::Launcher {
                console_read: false,
                console_write: false
            }
        );
        assert_eq!(
            parse(json!({ "type": "custom", "permissions": {} })),
            KeyScope::Custom {}
        );
        assert_eq!(parse(json!({ "type": "future" })), KeyScope::Other);
    }

    #[test]
    fn frames_for_the_server() {
        let frame = |inner: Value| {
            json!({
                "event_inner": { "type": "InstanceEvent", "instance_uuid": "i1", "instance_name": "SMP",
                                 "instance_event_inner": inner },
                "details": "", "snowflake": "42", "level": "Info", "caused_by": { "type": "System" }
            })
            .to_string()
        };
        assert_eq!(
            parse_frame(
                &frame(json!({ "type": "StateTransition", "to": "Running" })),
                "i1"
            ),
            Some(LiveMessage::State(ServerState::Running))
        );
        assert_eq!(
            parse_frame(
                &frame(
                    json!({ "type": "PlayerChange", "players_joined": [], "players_left": [],
                               "player_list": [{ "type": "MinecraftPlayer", "name": "zed", "uuid": null },
                                               { "type": "MinecraftPlayer", "name": "Alex", "uuid": "u" }] })
                ),
                "i1"
            ),
            Some(LiveMessage::Players(vec![
                Player {
                    name: "Alex".into(),
                    uuid: Some("u".into())
                },
                Player {
                    name: "zed".into(),
                    uuid: None
                },
            ]))
        );
        assert_eq!(
            parse_frame(
                &frame(json!({ "type": "InstanceOutput", "message": "§aHi" })),
                "i1"
            ),
            Some(LiveMessage::Output(ConsoleLine {
                snowflake: 42,
                message: "§aHi".into()
            }))
        );
        // another server, other kinds of events, and garbage are left out
        assert_eq!(
            parse_frame(
                &frame(json!({ "type": "InstanceOutput", "message": "x" })),
                "i2"
            ),
            None
        );
        assert_eq!(
            parse_frame(
                &frame(json!({ "type": "InstanceInput", "message": "x" })),
                "i1"
            ),
            None
        );
        assert_eq!(
            parse_frame(
                r#"{"event_inner":{"type":"UserEvent"},"snowflake":"1"}"#,
                "i1"
            ),
            None
        );
        assert_eq!(parse_frame("not json", "i1"), None);
    }

    #[test]
    fn console_lines_merge_in_order_without_repeats() {
        let line = |n: i64| ConsoleLine {
            snowflake: n,
            message: n.to_string(),
        };
        let mut log = vec![line(1), line(3)];
        merge_console(&mut log, [line(4), line(2), line(3), line(5)]);
        assert_eq!(
            log.iter().map(|l| l.snowflake).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        merge_console(&mut log, (6..6 + MAX_CONSOLE_LINES as i64).map(line));
        assert_eq!(log.len(), MAX_CONSOLE_LINES);
        assert_eq!(log[0].snowflake, 6);
    }

    #[test]
    fn stream_urls() {
        let base = Url::parse("https://core.example.com:16662/lodestone").unwrap();
        assert_eq!(
            ws_url(&base, "instance/i1/console/stream")
                .unwrap()
                .as_str(),
            "wss://core.example.com:16662/lodestone/api/v1/instance/i1/console/stream"
        );
        let local = Url::parse("http://127.0.0.1:16662/").unwrap();
        let key = Secret::new("lsk_abc_def");
        let url = ws_url(&local, &stream_path(&key, "i1", &StreamKind::Console)).unwrap();
        assert_eq!(url.scheme(), "ws");
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "token").unwrap().1,
            "Bearer lsk_abc_def"
        );
        let url = ws_url(&local, &stream_path(&key, "i1", &StreamKind::Events)).unwrap();
        let filter: Value =
            serde_json::from_str(&url.query_pairs().find(|(k, _)| k == "filter").unwrap().1)
                .unwrap();
        assert_eq!(filter["bearer_token"], "lsk_abc_def");
        assert_eq!(filter["event_instance_ids"], json!(["i1"]));
    }
}
