//! The launcher's side of the socket: accepts connections from this user only, reads
//! request lines and hands them to a [`Backend`], and writes what the backend sends.
//!
//! Everything here runs on its own threads and only reads and writes the socket. The
//! launcher's backend forwards each request to the GPUI main thread, which does the
//! work with the app's entities and answers through the connection's [`Outbox`].

use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

use serde_json::Value;

use crate::{
    peer,
    protocol::{ErrorObject, MAX_LINE_BYTES, Message, Request, codes},
};

/// More connections than this at once are refused.
pub const MAX_CONNECTIONS: usize = 64;

/// Events (not responses) are dropped while a connection has this many lines waiting,
/// so a client that doesn't read can't make the launcher buffer without end.
pub const MAX_QUEUED_EVENTS: usize = 10_000;

/// What the server hands requests to.
pub trait Backend: Send + Sync + 'static {
    /// A request from `connection`. Answer it, now or later, through `outbox`.
    fn request(&self, connection: u64, request: Request, outbox: Outbox);

    /// The client closed `connection`: end its subscriptions.
    fn disconnected(&self, _connection: u64) {}
}

/// Why the socket couldn't be created.
#[derive(Debug)]
pub enum BindError {
    /// Another launcher with the same data directory answers on it.
    InUse(PathBuf),
    /// The fallback folder belongs to another user, or others may write to it.
    UnsafeFolder(PathBuf),
    Io(io::Error),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindError::InUse(path) => write!(
                f,
                "another Lodestone Launcher already answers on {}",
                path.display()
            ),
            BindError::UnsafeFolder(path) => write!(
                f,
                "{} belongs to another user or others may write to it",
                path.display()
            ),
            BindError::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for BindError {}

impl From<io::Error> for BindError {
    fn from(err: io::Error) -> Self {
        BindError::Io(err)
    }
}

/// The bound socket, not yet serving.
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
    allowed_uid: u32,
}

impl Server {
    /// Creates the socket at `path` with mode 0600. A socket file left by a launcher
    /// that quit is replaced; one that a running launcher answers on is not.
    pub fn bind(path: &Path) -> Result<Server, BindError> {
        let dir = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no folder"))?;
        if dir.starts_with(crate::paths::fallback_dir()) {
            private_folder(dir)?;
        } else {
            fs::create_dir_all(dir)?;
        }
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                if UnixStream::connect(path).is_ok() {
                    return Err(BindError::InUse(path.to_path_buf()));
                }
                if !meta.file_type().is_socket() {
                    return Err(BindError::Io(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("{} exists and isn't a socket", path.display()),
                    )));
                }
                fs::remove_file(path)?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        // Bind under a temporary name, make it private, then move it into place, so the
        // socket is never reachable with looser permissions.
        let temp = dir.join(format!(".c{:x}", std::process::id()));
        let _ = fs::remove_file(&temp);
        let listener = UnixListener::bind(&temp)?;
        let placed = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
            .and_then(|()| fs::rename(&temp, path));
        if let Err(err) = placed {
            let _ = fs::remove_file(&temp);
            return Err(err.into());
        }
        Ok(Server {
            listener,
            path: path.to_path_buf(),
            allowed_uid: peer::current_uid(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Accepts only connections from `uid` (by default this process's user). For tests.
    pub fn with_allowed_uid(mut self, uid: u32) -> Self {
        self.allowed_uid = uid;
        self
    }

    /// Serves on a thread of its own until the handle is shut down or dropped.
    pub fn spawn(self, backend: Arc<dyn Backend>) -> io::Result<ServerHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let path = self.path.clone();
        let thread_stop = stop.clone();
        thread::Builder::new()
            .name("control-accept".into())
            .spawn(move || accept_loop(self, backend, thread_stop))?;
        Ok(ServerHandle {
            path,
            stop,
            stopped: false,
        })
    }
}

/// Refused connections held open at once while their clients finish sending.
const MAX_REFUSING: usize = 8;

/// Reads and drops what a refused client still sends, until it hangs up or for at most
/// two seconds and 64 KiB.
fn drain(stream: &mut UnixStream) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(250)));
    let started = std::time::Instant::now();
    let mut buf = [0u8; 4096];
    let mut total = 0usize;
    while started.elapsed() < std::time::Duration::from_secs(2) && total < 64 * 1024 {
        match std::io::Read::read(stream, &mut buf) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
}

/// Creates `dir` with mode 0700, or checks that it is this user's and private.
fn private_folder(dir: &Path) -> Result<(), BindError> {
    match fs::create_dir(dir) {
        Ok(()) => fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?,
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(err.into()),
    }
    let meta = fs::symlink_metadata(dir)?;
    if !meta.is_dir() || meta.uid() != peer::current_uid() || meta.mode() & 0o077 != 0 {
        return Err(BindError::UnsafeFolder(dir.to_path_buf()));
    }
    Ok(())
}

/// Stops the server and removes the socket file when shut down or dropped.
pub struct ServerHandle {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    stopped: bool,
}

impl ServerHandle {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.stop.store(true, Ordering::SeqCst);
        // wake the accept loop so it sees the flag
        let _ = UnixStream::connect(&self.path);
        let _ = fs::remove_file(&self.path);
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn accept_loop(server: Server, backend: Arc<dyn Backend>, stop: Arc<AtomicBool>) {
    let connections = Arc::new(AtomicUsize::new(0));
    let refusing = Arc::new(AtomicUsize::new(0));
    let next_id = AtomicU64::new(1);
    for stream in server.listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else {
            continue;
        };
        let refuse = |stream: &mut UnixStream, code: &str, message: &str| {
            let line = Message::result(Value::Null, Err(ErrorObject::new(code, message))).to_line();
            let _ = stream.write_all(line.as_bytes());
            // Closing at once can lose the refusal: the client's request then meets a
            // closed socket (broken pipe) and its buffered answer is dropped. End the
            // writing side and read until the client hangs up, for a short while, on a
            // thread of its own so the accept loop never waits; at most a few at once.
            let _ = stream.shutdown(std::net::Shutdown::Write);
            if refusing.fetch_add(1, Ordering::SeqCst) < MAX_REFUSING
                && let Ok(mut held) = stream.try_clone()
            {
                let count = refusing.clone();
                let spawned =
                    thread::Builder::new()
                        .name("control-refuse".into())
                        .spawn(move || {
                            drain(&mut held);
                            count.fetch_sub(1, Ordering::SeqCst);
                        });
                if spawned.is_err() {
                    refusing.fetch_sub(1, Ordering::SeqCst);
                }
            } else {
                refusing.fetch_sub(1, Ordering::SeqCst);
            }
        };
        let mut stream = stream;
        match peer::peer_uid(&stream) {
            Ok(uid) if uid == server.allowed_uid => {}
            _ => {
                refuse(
                    &mut stream,
                    codes::PERMISSION_DENIED,
                    "Only the user running Lodestone Launcher may control it.",
                );
                continue;
            }
        }
        if connections.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            refuse(
                &mut stream,
                codes::BUSY,
                "Too many connections to the launcher.",
            );
            continue;
        }
        connections.fetch_add(1, Ordering::SeqCst);
        let id = next_id.fetch_add(1, Ordering::SeqCst);
        let backend = backend.clone();
        let connections = connections.clone();
        let spawned = thread::Builder::new()
            .name(format!("control-conn-{id}"))
            .spawn(move || {
                serve_connection(id, stream, &*backend);
                connections.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            // the closure (and its counter) was dropped without running
            continue;
        }
    }
}

/// Where a connection's lines go. Cheap to clone; usable from any thread.
#[derive(Clone)]
pub struct Outbox {
    tx: mpsc::Sender<String>,
    queued: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl Outbox {
    /// A response, progress line or event. Responses are always queued. Returns
    /// false once the connection is gone.
    pub fn send(&self, message: &Message) -> bool {
        if matches!(message, Message::Event { .. })
            && self.queued.load(Ordering::SeqCst) >= MAX_QUEUED_EVENTS
        {
            self.dropped.fetch_add(1, Ordering::SeqCst);
            return !self.is_closed();
        }
        self.queued.fetch_add(1, Ordering::SeqCst);
        if self.tx.send(message.to_line()).is_err() {
            self.queued.fetch_sub(1, Ordering::SeqCst);
            self.closed.store(true, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// Answers request `id`.
    pub fn respond(&self, id: Value, result: Result<Value, ErrorObject>) -> bool {
        self.send(&Message::result(id, result))
    }

    /// The client closed the connection (or it broke).
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Events dropped because the client read too slowly, since the last call.
    pub fn take_dropped(&self) -> u64 {
        self.dropped.swap(0, Ordering::SeqCst)
    }

    /// An outbox that writes to `tx`, for backends' tests.
    pub fn for_test(tx: mpsc::Sender<String>) -> Self {
        Self {
            tx,
            queued: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }
}

fn serve_connection(id: u64, stream: UnixStream, backend: &dyn Backend) {
    let Ok(write_half) = stream.try_clone() else {
        return;
    };
    let (tx, rx) = mpsc::channel::<String>();
    let outbox = Outbox::for_test(tx);
    let writer_outbox = outbox.clone();
    let writer = thread::Builder::new()
        .name(format!("control-write-{id}"))
        .spawn(move || {
            let mut out = write_half;
            for line in rx {
                writer_outbox.queued.fetch_sub(1, Ordering::SeqCst);
                if out.write_all(line.as_bytes()).is_err() {
                    writer_outbox.closed.store(true, Ordering::SeqCst);
                    break;
                }
            }
        });
    if writer.is_err() {
        return;
    }

    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = (&mut reader)
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line);
        match read {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.len() > MAX_LINE_BYTES {
            outbox.respond(
                Value::Null,
                Err(ErrorObject::new(
                    codes::INVALID_REQUEST,
                    format!("A request line may be at most {MAX_LINE_BYTES} bytes."),
                )),
            );
            break;
        }
        let Ok(text) = std::str::from_utf8(&line) else {
            outbox.respond(
                Value::Null,
                Err(ErrorObject::new(
                    codes::PARSE_ERROR,
                    "The line isn't UTF-8.",
                )),
            );
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        match Request::parse(text) {
            Ok(request) => backend.request(id, request, outbox.clone()),
            Err((id, error)) => {
                outbox.respond(id, Err(error));
            }
        }
    }
    outbox.closed.store(true, Ordering::SeqCst);
    backend.disconnected(id);
}
