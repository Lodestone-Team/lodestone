//! The Discord app's local RPC connection: a named pipe on Windows
//! (`\\.\pipe\discord-ipc-N`) and a Unix socket elsewhere (`$XDG_RUNTIME_DIR` or the
//! temporary folder, `discord-ipc-N`), N from 0 to 9.
//!
//! Each frame is an opcode and a length, both little-endian `u32`, then that many
//! bytes of JSON. The client says hello (`HANDSHAKE` with its application id), Discord
//! answers `READY`, and from then on every command gets one answer with the same
//! `nonce` (not always in order). Closing the connection clears the status.
//!
//! The commands used: `SET_ACTIVITY` (the status, with a party and a join secret for
//! Discord's game invites), `SUBSCRIBE` (to `ACTIVITY_JOIN`, sent when the person
//! pressed Join on an invite, and `ACTIVITY_JOIN_REQUEST`, when someone asks to join
//! theirs) and `SEND_ACTIVITY_JOIN_INVITE` (lets someone who asked join). None needs
//! the person to authorize the application.
//!
//! Other Discord apps can serve the same pipes: Legcord's and Vesktop's arRPC shows
//! statuses but never answers `SUBSCRIBE`, so nothing waits for that answer without
//! a time limit, and [`Connection::open_subscribed`] prefers the app that answers.
//!
//! Everything blocks: the app calls this from its own worker thread.

use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Value, json};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

/// Discord's answers are small; anything bigger isn't Discord.
const MAX_FRAME: usize = 64 * 1024;

/// How long a read or write waits before the connection counts as lost. Windows'
/// pipes, opened as files, have no timeout; Discord answers at once.
#[cfg(unix)]
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The longest text Discord takes in an activity's fields, in characters. Also the
/// longest join secret and party id.
pub const MAX_TEXT: usize = 128;
/// The shortest: a field with less is refused, so it is left out instead.
const MIN_TEXT: usize = 2;

/// What the status shows. Discord draws `details` and `state` as two lines under the
/// application's name, the time since `start`, and the images with their tooltips.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Activity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    #[serde(skip_serializing_if = "Assets::is_empty")]
    pub assets: Assets,
    /// Discord refuses buttons together with [`Self::secrets`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<ActivityButton>,
    /// With [`Self::secrets`], what makes the status joinable: Discord shows
    /// "Ask to Join" on the profile and lets the person send invites in chat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party: Option<Party>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Secrets>,
}

/// The group the status's person is in. Discord shows the size after the state
/// line ("(2 of 20)").
///
/// Never add `privacy`: a public party sets the activity's PARTY_PRIVACY_FRIENDS or
/// PARTY_PRIVACY_VOICE_CHANNEL flag, with which Discord gives the join secret (the
/// invite, key included) to any friend who asks for it, without the host letting
/// them in. Without it Discord marks the status JOIN only (`flags: 2`): people get
/// "Ask to Join", and the secret only on an accepted request or an invite card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Party {
    /// 2 to 128 characters; only the application sees it.
    pub id: String,
    /// Current and most. Discord takes 0 people.
    pub size: [u32; 2],
}

impl Party {
    /// `None` for an id Discord refuses.
    pub fn new(id: impl Into<String>, current: u32, max: u32) -> Option<Self> {
        let id = id.into();
        let len = id.chars().count();
        (MIN_TEXT..=MAX_TEXT).contains(&len).then(|| Party {
            id,
            size: [current, max.max(current).max(1)],
        })
    }
}

/// What Discord hands to the people who join. Only `join`: any other field, even
/// empty, makes Discord's invites arrive blank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Secrets {
    pub join: String,
}

impl Secrets {
    /// `None` for a secret longer than Discord takes (or empty).
    pub fn join(secret: impl Into<String>) -> Option<Self> {
        let join = secret.into();
        (!join.is_empty() && join.chars().count() <= MAX_TEXT).then_some(Secrets { join })
    }
}

/// Something Discord sent without being asked, from a subscription.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The person pressed Join on an invite (or was let in after asking): the host's
    /// join secret.
    Join { secret: String },
    /// Someone asks to join the person's status. Discord forgets the request after
    /// about 30 seconds.
    JoinRequest(JoinRequest),
    /// Discord refused a subscription.
    SubscribeRefused(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinRequest {
    pub user_id: String,
    /// Their display name, else their username.
    pub name: String,
}

impl Event {
    /// The event in a frame, if it is one.
    fn from_frame(body: &Value) -> Option<Event> {
        if body["cmd"] == "SUBSCRIBE" && body["evt"] == "ERROR" {
            return Some(Event::SubscribeRefused(message(body)));
        }
        if body["cmd"] != "DISPATCH" {
            return None;
        }
        let data = &body["data"];
        match body["evt"].as_str()? {
            "ACTIVITY_JOIN" => Some(Event::Join {
                secret: data["secret"].as_str()?.to_owned(),
            }),
            "ACTIVITY_JOIN_REQUEST" => {
                let user = &data["user"];
                let user_id = user["id"].as_str()?.to_owned();
                let name = [&user["global_name"], &user["username"]]
                    .into_iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .find(|name| !name.is_empty())
                    .unwrap_or(&user_id)
                    .to_owned();
                Some(Event::JoinRequest(JoinRequest { user_id, name }))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Timestamps {
    /// Unix milliseconds: Discord shows the time since.
    pub start: u64,
}

/// The images: an art asset's key in the Discord application, or an `https://` URL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Assets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

impl Assets {
    fn is_empty(&self) -> bool {
        self.large_image.is_none() && self.small_image.is_none()
    }
}

/// A link under the status; Discord takes two at most.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActivityButton {
    pub label: String,
    pub url: String,
}

/// `text` as Discord takes it: cut to [`MAX_TEXT`] characters (with an ellipsis), or
/// `None` when shorter than Discord allows.
pub fn field(text: impl AsRef<str>) -> Option<String> {
    let text = text.as_ref().trim();
    if text.chars().count() < MIN_TEXT {
        return None;
    }
    if text.chars().count() <= MAX_TEXT {
        return Some(text.to_owned());
    }
    let mut cut: String = text.chars().take(MAX_TEXT - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    Some(cut)
}

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// The Discord app isn't running (or doesn't take RPC connections).
    #[error("Discord isn't running")]
    NotRunning,
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Discord closed the connection or refused a command.
    #[error("Discord: {0}")]
    Refused(String),
}

#[cfg(windows)]
type Stream = std::fs::File;
#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;

/// What a connection runs over: Discord's pipe or socket (or a test's socket).
pub trait Pipe: Read + Write {
    /// Whether a frame has started arriving, without waiting. An error once the
    /// other side closed.
    fn has_data(&self) -> io::Result<bool>;
    /// Reads wait for as long as it takes, for a connection that only waits for
    /// events.
    fn wait_without_limit(&self) -> io::Result<()>;
}

#[cfg(windows)]
impl Pipe for std::fs::File {
    fn has_data(&self) -> io::Result<bool> {
        use std::os::windows::io::AsRawHandle;
        let mut available = 0u32;
        // SAFETY: the handle is this open pipe's; only `available` is written.
        let ok = unsafe {
            windows_sys::Win32::System::Pipes::PeekNamedPipe(
                self.as_raw_handle() as _,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(available > 0)
    }

    fn wait_without_limit(&self) -> io::Result<()> {
        // A pipe opened as a file has no read timeout.
        Ok(())
    }
}

/// Whether a socket has bytes waiting, without waiting.
#[cfg(any(unix, test))]
fn socket_has_data(
    set_nonblocking: impl Fn(bool) -> io::Result<()>,
    peek: impl FnOnce(&mut [u8]) -> io::Result<usize>,
) -> io::Result<bool> {
    set_nonblocking(true)?;
    let peeked = peek(&mut [0u8; 1]);
    set_nonblocking(false)?;
    match peeked {
        Ok(0) => Err(io::ErrorKind::UnexpectedEof.into()),
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
impl Pipe for std::os::unix::net::UnixStream {
    fn has_data(&self) -> io::Result<bool> {
        socket_has_data(|on| self.set_nonblocking(on), |buf| self.peek(buf))
    }

    fn wait_without_limit(&self) -> io::Result<()> {
        self.set_read_timeout(None)
    }
}

/// The tests' fake Discord listens on a local TCP port, on every platform.
#[cfg(test)]
impl Pipe for std::net::TcpStream {
    fn has_data(&self) -> io::Result<bool> {
        socket_has_data(|on| self.set_nonblocking(on), |buf| self.peek(buf))
    }

    fn wait_without_limit(&self) -> io::Result<()> {
        self.set_read_timeout(None)
    }
}

/// How often [`Connection::open_subscribed`] looks for Discord's answer.
const ANSWER_POLL: Duration = Duration::from_millis(25);

/// A connection to the Discord app.
pub struct Connection<S: Pipe = Stream> {
    stream: S,
    nonce: u64,
    /// Events that arrived while waiting for an answer, oldest first.
    events: VecDeque<Event>,
}

impl Connection {
    /// Connects to the first Discord app that answers, and says hello as `client_id`.
    pub fn open(client_id: &str) -> Result<Self, IpcError> {
        for path in candidates() {
            let Some(stream) = open_stream(&path) else {
                continue;
            };
            let mut connection = Connection::new(stream);
            connection.handshake(client_id)?;
            return Ok(connection);
        }
        Err(IpcError::NotRunning)
    }

    /// Connects as `client_id` and subscribes to `events`, preferring the Discord app
    /// that confirms the first subscription within `wait` (the official app does;
    /// arRPC never answers). Also returns whether it did: if none did, this is the
    /// first app that said hello, which shows statuses but may never send events.
    pub fn open_subscribed(
        client_id: &str,
        events: &[&str],
        wait: Duration,
    ) -> Result<(Self, bool), IpcError> {
        let mut fallback = None;
        let mut last_error = IpcError::NotRunning;
        for path in candidates() {
            let Some(stream) = open_stream(&path) else {
                continue;
            };
            let mut connection = Connection::new(stream);
            let confirmed = connection
                .handshake(client_id)
                .and_then(|()| connection.subscribe_confirmed(events, wait));
            match confirmed {
                Ok(true) => return Ok((connection, true)),
                Ok(false) => {
                    fallback.get_or_insert(connection);
                }
                Err(err) => last_error = err,
            }
        }
        fallback.map(|c| (c, false)).ok_or(last_error)
    }
}

impl<S: Pipe> Connection<S> {
    fn new(stream: S) -> Self {
        Connection {
            stream,
            nonce: 0,
            events: VecDeque::new(),
        }
    }

    /// Subscribes to `events`, and waits up to `wait` for Discord to confirm the
    /// first. Whether it did.
    fn subscribe_confirmed(&mut self, events: &[&str], wait: Duration) -> Result<bool, IpcError> {
        let mut first = None;
        for event in events {
            let nonce = self.subscribe(event)?;
            first.get_or_insert(nonce);
        }
        let Some(first) = first else {
            return Ok(true);
        };
        let deadline = Instant::now() + wait;
        loop {
            while self.stream.has_data()? {
                let (op, body) = self.read()?;
                match op {
                    OP_FRAME if body["nonce"] == first.as_str() => {
                        return if body["evt"] == "ERROR" {
                            Err(IpcError::Refused(message(&body)))
                        } else {
                            Ok(true)
                        };
                    }
                    OP_FRAME => self.events.extend(Event::from_frame(&body)),
                    OP_PING => self.write(OP_PONG, &body)?,
                    OP_CLOSE => return Err(IpcError::Refused(message(&body))),
                    _ => {}
                }
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(ANSWER_POLL);
        }
    }

    fn handshake(&mut self, client_id: &str) -> Result<(), IpcError> {
        self.write(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))?;
        loop {
            let (op, body) = self.read()?;
            match op {
                OP_FRAME if body["evt"] == "READY" => return Ok(()),
                OP_PING => self.write(OP_PONG, &body)?,
                OP_CLOSE => return Err(IpcError::Refused(message(&body))),
                _ if body["evt"] == "ERROR" => return Err(IpcError::Refused(message(&body))),
                _ => {}
            }
        }
    }

    /// Shows `activity`, or clears the status with `None`. `pid` is the process the
    /// status belongs to: Discord clears it when that process exits.
    pub fn set_activity(&mut self, pid: u32, activity: Option<&Activity>) -> Result<(), IpcError> {
        self.command("SET_ACTIVITY", json!({ "pid": pid, "activity": activity }))
            .map(drop)
    }

    /// Lets someone who asked to join ([`Event::JoinRequest`]) in: Discord hands them
    /// the join secret.
    pub fn send_join_invite(&mut self, user_id: &str) -> Result<(), IpcError> {
        self.command("SEND_ACTIVITY_JOIN_INVITE", json!({ "user_id": user_id }))
            .map(drop)
    }

    /// Asks for `event` (`ACTIVITY_JOIN`, `ACTIVITY_JOIN_REQUEST`) without waiting for
    /// the answer, which some apps never send. Returns the command's nonce.
    pub fn subscribe(&mut self, event: &str) -> Result<String, IpcError> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        self.write(
            OP_FRAME,
            &json!({ "cmd": "SUBSCRIBE", "evt": event, "args": {}, "nonce": nonce }),
        )?;
        Ok(nonce)
    }

    /// Waits for the next event, for as long as it takes (see
    /// [`Self::wait_without_limit`]).
    pub fn next_event(&mut self) -> Result<Event, IpcError> {
        loop {
            if let Some(event) = self.events.pop_front() {
                return Ok(event);
            }
            self.read_one()?;
        }
    }

    /// The next event if one has arrived, without waiting.
    pub fn poll_event(&mut self) -> Result<Option<Event>, IpcError> {
        loop {
            if let Some(event) = self.events.pop_front() {
                return Ok(Some(event));
            }
            if !self.stream.has_data()? {
                return Ok(None);
            }
            self.read_one()?;
        }
    }

    /// Reads from now on wait without a time limit (Unix sockets have one): for a
    /// connection that only waits for events.
    pub fn wait_without_limit(&mut self) -> Result<(), IpcError> {
        Ok(self.stream.wait_without_limit()?)
    }

    /// Reads one frame, keeping the event in it.
    fn read_one(&mut self) -> Result<(), IpcError> {
        let (op, body) = self.read()?;
        match op {
            OP_FRAME => self.events.extend(Event::from_frame(&body)),
            OP_PING => self.write(OP_PONG, &body)?,
            OP_CLOSE => return Err(IpcError::Refused(message(&body))),
            _ => {}
        }
        Ok(())
    }

    /// Sends a command and waits for its answer's data, keeping the events that
    /// arrive meanwhile.
    fn command(&mut self, cmd: &str, args: Value) -> Result<Value, IpcError> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        self.write(
            OP_FRAME,
            &json!({ "cmd": cmd, "args": args, "nonce": nonce }),
        )?;
        loop {
            let (op, body) = self.read()?;
            match op {
                OP_FRAME if body["nonce"] == nonce.as_str() => {
                    return if body["evt"] == "ERROR" {
                        Err(IpcError::Refused(message(&body)))
                    } else {
                        Ok(body["data"].clone())
                    };
                }
                // Events, and answers to earlier commands.
                OP_FRAME => self.events.extend(Event::from_frame(&body)),
                OP_PING => self.write(OP_PONG, &body)?,
                OP_CLOSE => return Err(IpcError::Refused(message(&body))),
                _ => {}
            }
        }
    }

    fn write(&mut self, op: u32, body: &Value) -> io::Result<()> {
        write_frame(&mut self.stream, op, body)
    }

    fn read(&mut self) -> Result<(u32, Value), IpcError> {
        read_frame(&mut self.stream)
    }
}

fn write_frame(stream: &mut impl Write, op: u32, body: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(body)?;
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    stream.write_all(&frame)?;
    stream.flush()
}

fn read_frame(stream: &mut impl Read) -> Result<(u32, Value), IpcError> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if len > MAX_FRAME {
        return Err(IpcError::Refused(format!("a {len}-byte answer")));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;
    let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    Ok((op, body))
}

/// A fake Discord app for tests, over local TCP so the tests run everywhere: it
/// says hello like Discord and then does what the test tells it.
#[cfg(test)]
pub(crate) mod fake {
    use std::net::{TcpListener, TcpStream};

    use super::*;

    pub struct FakeDiscord {
        pub stream: TcpStream,
    }

    impl FakeDiscord {
        pub fn read(&mut self) -> (u32, Value) {
            read_frame(&mut self.stream).expect("a frame from the client")
        }

        pub fn send(&mut self, body: Value) {
            write_frame(&mut self.stream, OP_FRAME, &body).expect("the client is there");
        }

        /// Reads the next command, skipping its `SUBSCRIBE`s (returned apart).
        pub fn command(&mut self) -> Value {
            loop {
                let (op, body) = self.read();
                assert_eq!(op, OP_FRAME, "{body}");
                if body["cmd"] != "SUBSCRIBE" {
                    return body;
                }
            }
        }

        /// Answers `command` with `data`.
        pub fn answer(&mut self, command: &Value, data: Value) {
            self.send(json!({ "cmd": command["cmd"], "nonce": command["nonce"], "data": data }));
        }
    }

    /// A listener for fake Discords, and a way to connect to it as `client_id`.
    pub struct Listener(TcpListener);

    impl Listener {
        pub fn new() -> Self {
            Listener(TcpListener::bind("127.0.0.1:0").expect("a local port"))
        }

        /// Connects and says hello (Discord's side answers in `accept`).
        pub fn connect(&self, client_id: &str) -> Result<Connection<TcpStream>, IpcError> {
            let stream = TcpStream::connect(self.0.local_addr()?)?;
            let mut connection = Connection::new(stream);
            connection.handshake(client_id)?;
            Ok(connection)
        }

        /// Takes the next connection and answers its hello.
        pub fn accept(&self) -> FakeDiscord {
            let (stream, _) = self.0.accept().expect("a client");
            let mut fake = FakeDiscord { stream };
            let (op, hello) = fake.read();
            assert_eq!(op, OP_HANDSHAKE);
            assert!(hello["client_id"].is_string());
            fake.send(json!({ "evt": "READY", "cmd": "DISPATCH" }));
            fake
        }
    }

    /// A connected pair: the client's connection and the fake Discord.
    pub fn pair() -> (Connection<TcpStream>, FakeDiscord) {
        let listener = Listener::new();
        std::thread::scope(|scope| {
            let accept = scope.spawn(|| listener.accept());
            let client = listener.connect("42").expect("the handshake");
            (client, accept.join().expect("the fake"))
        })
    }
}

impl<S: Pipe> Drop for Connection<S> {
    fn drop(&mut self) {
        // Says goodbye; closing the connection clears the status either way.
        let _ = self.write(OP_CLOSE, &json!({}));
    }
}

/// The message of an `ERROR` event or a `CLOSE` frame.
fn message(body: &Value) -> String {
    body["data"]["message"]
        .as_str()
        .or_else(|| body["message"].as_str())
        .unwrap_or("closed the connection")
        .to_owned()
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    (0..10)
        .map(|n| PathBuf::from(format!(r"\\.\pipe\discord-ipc-{n}")))
        .collect()
}

#[cfg(windows)]
fn open_stream(path: &std::path::Path) -> Option<Stream> {
    // A pipe that no Discord serves fails at once, without touching a disk.
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()
}

#[cfg(unix)]
fn candidates() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    roots.push(PathBuf::from("/tmp"));
    roots.dedup();
    // Flatpak and Snap builds of Discord, and Vesktop, keep the socket a folder down.
    let subfolders = [
        "",
        "app/com.discordapp.Discord",
        "app/com.discordapp.DiscordCanary",
        "snap.discord",
        "snap.discord-canary",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
    ];
    let mut paths = Vec::new();
    for root in &roots {
        for sub in subfolders {
            let dir = if sub.is_empty() {
                root.clone()
            } else {
                root.join(sub)
            };
            for n in 0..10 {
                paths.push(dir.join(format!("discord-ipc-{n}")));
            }
        }
    }
    paths
}

#[cfg(unix)]
fn open_stream(path: &std::path::Path) -> Option<Stream> {
    let stream = std::os::unix::net::UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    Some(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_cut_to_what_discord_takes() {
        assert_eq!(field("a"), None);
        assert_eq!(field("  "), None);
        assert_eq!(field("ab"), Some("ab".into()));
        let long = "é".repeat(200);
        let cut = field(&long).unwrap();
        assert_eq!(cut.chars().count(), MAX_TEXT);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn an_activity_leaves_out_what_it_lacks() {
        let activity = Activity {
            details: Some("All the Mods 9".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&activity).unwrap(),
            json!({ "details": "All the Mods 9" })
        );
        let activity = Activity {
            state: Some("Overworld".into()),
            timestamps: Some(Timestamps { start: 5 }),
            assets: Assets {
                large_image: Some("lodestone".into()),
                ..Default::default()
            },
            buttons: vec![ActivityButton {
                label: "View modpack".into(),
                url: "https://modrinth.com/modpack/x".into(),
            }],
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&activity).unwrap(),
            json!({
                "state": "Overworld",
                "timestamps": { "start": 5 },
                "assets": { "large_image": "lodestone" },
                "buttons": [{ "label": "View modpack", "url": "https://modrinth.com/modpack/x" }],
            })
        );
    }

    /// A fake Discord on a Unix socket, as Discord's is: answers the handshake and one
    /// command.
    #[cfg(unix)]
    #[test]
    fn talks_to_discord_on_a_unix_socket() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let (op, hello) = read_frame(&mut stream).unwrap();
            assert_eq!(op, OP_HANDSHAKE);
            assert_eq!(hello["client_id"], "42");
            write_frame(
                &mut stream,
                OP_FRAME,
                &json!({ "evt": "READY", "cmd": "DISPATCH" }),
            )
            .unwrap();
            let (_, command) = read_frame(&mut stream).unwrap();
            assert_eq!(command["cmd"], "SET_ACTIVITY");
            assert_eq!(command["args"]["activity"]["details"], "Hello");
            let answer = json!({ "cmd": "SET_ACTIVITY", "nonce": command["nonce"], "data": {} });
            write_frame(&mut stream, OP_FRAME, &answer).unwrap();
        });
        let stream = open_stream(&path).unwrap();
        let mut connection = Connection::new(stream);
        connection.handshake("42").unwrap();
        let activity = Activity {
            details: Some("Hello".into()),
            ..Default::default()
        };
        connection.set_activity(1, Some(&activity)).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn a_joinable_activity_has_a_party_and_only_the_join_secret() {
        let link = "https://connect.lodestone.cc/j/ABCDEFGH23#".to_owned() + &"k".repeat(43);
        let activity = Activity {
            details: Some("Hosting Survival".into()),
            party: Party::new("instance-1", 0, 20),
            secrets: Secrets::join(link.clone()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&activity).unwrap(),
            json!({
                "details": "Hosting Survival",
                "party": { "id": "instance-1", "size": [0, 20] },
                "secrets": { "join": link },
            })
        );
        // only `id` and `size`: no `privacy` (see `Party`)
        let party = serde_json::to_value(Party::new("instance-1", 3, 20)).unwrap();
        let mut fields: Vec<&String> = party.as_object().unwrap().keys().collect();
        fields.sort();
        assert_eq!(fields, ["id", "size"]);
        // what Discord refuses (checked against the Discord app)
        assert_eq!(Party::new("p", 1, 2), None);
        assert_eq!(Secrets::join("j".repeat(129)), None);
        assert!(Secrets::join("j".repeat(128)).is_some());
        assert_eq!(Secrets::join(""), None);
        // a full or unknown size still says how many are in
        assert_eq!(Party::new("id", 25, 20).unwrap().size, [25, 25]);
        assert_eq!(Party::new("id", 0, 0).unwrap().size, [0, 1]);
    }

    #[test]
    fn events_are_read_from_dispatches() {
        let join = json!({
            "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "s3cret" },
        });
        assert_eq!(
            Event::from_frame(&join),
            Some(Event::Join {
                secret: "s3cret".into()
            })
        );
        let request = json!({
            "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST",
            "data": { "user": { "id": "7", "username": "steve", "global_name": "Steve" } },
        });
        assert_eq!(
            Event::from_frame(&request),
            Some(Event::JoinRequest(JoinRequest {
                user_id: "7".into(),
                name: "Steve".into()
            }))
        );
        let no_display_name = json!({
            "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST",
            "data": { "user": { "id": "7", "username": "steve", "global_name": null } },
        });
        assert!(matches!(
            Event::from_frame(&no_display_name),
            Some(Event::JoinRequest(JoinRequest { name, .. })) if name == "steve"
        ));
        // Discord's answer to a made-up event
        let refused = json!({
            "cmd": "SUBSCRIBE", "evt": "ERROR", "nonce": "2",
            "data": { "code": 4004, "message": "Invalid event: NOT_AN_EVENT" },
        });
        assert_eq!(
            Event::from_frame(&refused),
            Some(Event::SubscribeRefused(
                "Invalid event: NOT_AN_EVENT".into()
            ))
        );
        let answer =
            json!({ "cmd": "SUBSCRIBE", "data": { "evt": "ACTIVITY_JOIN" }, "nonce": "1" });
        assert_eq!(Event::from_frame(&answer), None);
    }

    /// Answers come back out of order, and events arrive between them: both kept.
    #[test]
    fn events_that_come_with_answers_are_kept() {
        let (mut client, mut discord) = fake::pair();
        let server = std::thread::spawn(move || {
            let command = discord.command();
            assert_eq!(command["cmd"], "SEND_ACTIVITY_JOIN_INVITE");
            assert_eq!(command["args"]["user_id"], "7");
            discord.send(json!({ "cmd": "SET_ACTIVITY", "nonce": "0", "data": null }));
            discord.send(json!({
                "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "a" },
            }));
            discord.answer(&command, Value::Null);
            discord
        });
        client.send_join_invite("7").unwrap();
        let _discord = server.join().unwrap();
        assert_eq!(
            client.poll_event().unwrap(),
            Some(Event::Join { secret: "a".into() })
        );
        assert_eq!(client.poll_event().unwrap(), None);
    }

    #[test]
    fn a_refused_command_is_an_error() {
        let (mut client, mut discord) = fake::pair();
        let server = std::thread::spawn(move || {
            let command = discord.command();
            discord.send(json!({
                "cmd": command["cmd"], "nonce": command["nonce"], "evt": "ERROR",
                "data": { "code": 5005, "message": "secrets cannot currently be sent with buttons" },
            }));
            discord
        });
        let err = client
            .set_activity(1, Some(&Activity::default()))
            .unwrap_err();
        assert!(err.to_string().contains("buttons"), "{err}");
        drop(server.join());
    }

    #[test]
    fn subscriptions_are_confirmed_or_not() {
        // the official app confirms
        let (mut client, mut discord) = fake::pair();
        let server = std::thread::spawn(move || {
            let (_, subscribe) = discord.read();
            assert_eq!(subscribe["cmd"], "SUBSCRIBE");
            assert_eq!(subscribe["evt"], "ACTIVITY_JOIN");
            discord.answer(&subscribe, json!({ "evt": "ACTIVITY_JOIN" }));
            discord
        });
        let confirmed = client.subscribe_confirmed(&["ACTIVITY_JOIN"], Duration::from_secs(5));
        assert!(confirmed.unwrap());
        drop(server.join());
        // arRPC never answers: not confirmed, after the wait
        let (mut client, discord) = fake::pair();
        let confirmed = client.subscribe_confirmed(&["ACTIVITY_JOIN"], Duration::from_millis(50));
        assert!(!confirmed.unwrap());
        drop(discord);
        // and a closed connection is an error
        assert!(client.poll_event().is_err());
    }
}
