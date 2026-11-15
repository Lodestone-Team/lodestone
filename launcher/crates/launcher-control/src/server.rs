//! The launcher's side of the socket: accepts connections from this user only, reads
//! request lines and hands them to a [`Backend`], and writes what the backend sends.
//!
//! Everything here runs on its own threads and only reads and writes the socket. The
//! launcher's backend forwards each request to the GPUI main thread, which does the
//! work with the app's entities and answers through the connection's [`Outbox`].

use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

use serde_json::Value;

use crate::{
    peer::{self, UserId},
    protocol::{ErrorObject, MAX_LINE_BYTES, Message, Request, codes},
    transport::{self, Listener, Stream},
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

    /// A client opened `connection` (it passed the peer check).
    fn connected(&self, _connection: u64) {}

    /// The client closed `connection`: end its subscriptions.
    fn disconnected(&self, _connection: u64) {}
}

/// The open connections' streams, so the server can close them.
type Streams = Arc<Mutex<HashMap<u64, Stream>>>;

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
    listener: Listener,
    path: PathBuf,
    allowed_uid: UserId,
}

impl Server {
    /// Creates the socket at `path`, with mode 0600 on Unix (on Windows it inherits the
    /// data folder's permissions, which are the user's). A socket file left by a
    /// launcher that quit is replaced; one that a running launcher answers on is not.
    pub fn bind(path: &Path) -> Result<Server, BindError> {
        let dir = path
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no folder"))?;
        #[cfg(unix)]
        if dir.starts_with(crate::paths::fallback_dir()) {
            private_folder(dir)?;
        } else {
            fs::create_dir_all(dir)?;
        }
        // the user's own folders (the data folder or %TEMP%), private already
        #[cfg(windows)]
        fs::create_dir_all(dir)?;
        match fs::symlink_metadata(path) {
            Ok(_) => {
                if Stream::connect(path).is_ok() {
                    return Err(BindError::InUse(path.to_path_buf()));
                }
                if !transport::is_socket(path) {
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
        let listener = listen(dir, path)?;
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
    pub fn with_allowed_uid(mut self, uid: UserId) -> Self {
        self.allowed_uid = uid;
        self
    }

    /// Serves until the handle is shut down or dropped: each connection on a thread of
    /// its own. On Unix a thread waits for connections; on Windows the system's thread
    /// pool does ([`pool_accept`]), so no thread waits in `accept` all day.
    pub fn spawn(self, backend: Arc<dyn Backend>) -> io::Result<ServerHandle> {
        let Server {
            listener,
            path,
            allowed_uid,
        } = self;
        let stop = Arc::new(AtomicBool::new(false));
        let streams = Streams::default();
        let acceptor = Acceptor {
            backend,
            stop: stop.clone(),
            streams: streams.clone(),
            allowed_uid,
            connections: Arc::new(AtomicUsize::new(0)),
            refusing: Arc::new(AtomicUsize::new(0)),
            next_id: AtomicU64::new(1),
        };
        #[cfg(not(windows))]
        thread::Builder::new()
            .name("control-accept".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if acceptor.stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else {
                        continue;
                    };
                    if !acceptor.accepted(stream) {
                        break;
                    }
                }
            })?;
        #[cfg(windows)]
        let waiting = Some(pool_accept::start(listener, acceptor)?);
        Ok(ServerHandle {
            path,
            stop,
            stopped: false,
            streams,
            #[cfg(windows)]
            waiting,
        })
    }
}

/// Refused connections held open at once while their clients finish sending.
const MAX_REFUSING: usize = 8;

/// Reads and drops what a refused client still sends, until it hangs up or for at most
/// two seconds and 64 KiB.
fn drain(stream: &mut Stream) {
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

/// Binds under a temporary name, makes it private, then moves it into place, so the
/// socket is never reachable with looser permissions.
#[cfg(unix)]
fn listen(dir: &Path, path: &Path) -> io::Result<Listener> {
    use std::os::unix::fs::PermissionsExt;
    let temp = dir.join(format!(".c{:x}", std::process::id()));
    let _ = fs::remove_file(&temp);
    let listener = Listener::bind(&temp)?;
    let placed = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
        .and_then(|()| fs::rename(&temp, path));
    if let Err(err) = placed {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    Ok(listener)
}

/// Binds in place: the socket file inherits the folder's permissions, and every
/// connection's user is checked anyway.
#[cfg(windows)]
fn listen(_dir: &Path, path: &Path) -> io::Result<Listener> {
    Listener::bind(path)
}

/// Creates `dir` with mode 0700, or checks that it is this user's and private.
#[cfg(unix)]
fn private_folder(dir: &Path) -> Result<(), BindError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
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
    streams: Streams,
    /// The thread pool's wait for connections, and the listening socket.
    #[cfg(windows)]
    waiting: Option<pool_accept::Waiting>,
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
        #[cfg(not(windows))]
        let _ = Stream::connect(&self.path);
        // stop waiting for connections (a connection being let in finishes first), and
        // close the listening socket
        #[cfg(windows)]
        drop(self.waiting.take());
        let _ = fs::remove_file(&self.path);
        let streams = std::mem::take(&mut *lock(&self.streams));
        for stream in streams.values() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Closes `connection`. Its client sees the socket close; the backend hears
    /// `disconnected`. False when no such connection is open.
    pub fn disconnect(&self, connection: u64) -> bool {
        match lock(&self.streams).remove(&connection) {
            Some(stream) => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                true
            }
            None => false,
        }
    }

    /// How many connections are open.
    pub fn connections(&self) -> usize {
        lock(&self.streams).len()
    }
}

fn lock(streams: &Streams) -> std::sync::MutexGuard<'_, HashMap<u64, Stream>> {
    streams
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// What letting connections in needs, for the server's life.
struct Acceptor {
    backend: Arc<dyn Backend>,
    stop: Arc<AtomicBool>,
    streams: Streams,
    allowed_uid: UserId,
    /// Connections being served.
    connections: Arc<AtomicUsize>,
    /// Refused connections being drained.
    refusing: Arc<AtomicUsize>,
    next_id: AtomicU64,
}

impl Acceptor {
    /// Checks a new connection's user and serves it on a thread of its own, or refuses
    /// it. False once the server is stopping.
    fn accepted(&self, mut stream: Stream) -> bool {
        match peer::peer_uid(&stream) {
            Ok(uid) if uid == self.allowed_uid => {}
            _ => {
                self.refuse(
                    &mut stream,
                    codes::PERMISSION_DENIED,
                    "Only the user running Lodestone Launcher may control it.",
                );
                return true;
            }
        }
        if self.connections.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            self.refuse(
                &mut stream,
                codes::BUSY,
                "Too many connections to the launcher.",
            );
            return true;
        }
        let Ok(held) = stream.try_clone() else {
            return true;
        };
        self.connections.fetch_add(1, Ordering::SeqCst);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        lock(&self.streams).insert(id, held);
        if self.stop.load(Ordering::SeqCst) {
            // shut down while this one was being let in
            lock(&self.streams).remove(&id);
            self.connections.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        let backend = self.backend.clone();
        let open = self.connections.clone();
        let thread_streams = self.streams.clone();
        let spawned = thread::Builder::new()
            .name(format!("control-conn-{id}"))
            .spawn(move || {
                backend.connected(id);
                serve_connection(id, stream, &*backend);
                lock(&thread_streams).remove(&id);
                open.fetch_sub(1, Ordering::SeqCst);
            });
        if spawned.is_err() {
            // the closure (and its counter) was dropped without running
            lock(&self.streams).remove(&id);
            self.connections.fetch_sub(1, Ordering::SeqCst);
        }
        true
    }

    fn refuse(&self, stream: &mut Stream, code: &str, message: &str) {
        let line = Message::result(Value::Null, Err(ErrorObject::new(code, message))).to_line();
        let _ = stream.write_all(line.as_bytes());
        // Closing at once can lose the refusal: the client's request then meets a
        // closed socket (broken pipe) and its buffered answer is dropped. End the
        // writing side and read until the client hangs up, for a short while, on a
        // thread of its own so letting others in never waits; at most a few at once.
        let _ = stream.shutdown(std::net::Shutdown::Write);
        if self.refusing.fetch_add(1, Ordering::SeqCst) < MAX_REFUSING
            && let Ok(mut held) = stream.try_clone()
        {
            let count = self.refusing.clone();
            let spawned = thread::Builder::new()
                .name("control-refuse".into())
                .spawn(move || {
                    drain(&mut held);
                    count.fetch_sub(1, Ordering::SeqCst);
                });
            if spawned.is_err() {
                self.refusing.fetch_sub(1, Ordering::SeqCst);
            }
        } else {
            self.refusing.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Windows: connections are let in by a wait on the system's thread pool (the one
/// GPUI's background executor runs on) rather than by a thread waiting in `accept` for
/// as long as the launcher runs. The listening socket signals an event when a
/// connection arrives (`WSAEventSelect`, which also makes it non-blocking); the pool
/// runs [`on_connection`] then, which accepts every connection waiting and waits again.
#[cfg(windows)]
mod pool_accept {
    use std::{ffi::c_void, io, ptr, sync::atomic::Ordering};

    use windows_sys::Win32::{
        Networking::WinSock::{
            FD_ACCEPT, WSACloseEvent, WSACreateEvent, WSAEnumNetworkEvents, WSAEventSelect,
            WSAGetLastError, WSANETWORKEVENTS,
        },
        System::Threading::{
            CloseThreadpoolWait, CreateThreadpoolWait, PTP_CALLBACK_INSTANCE, PTP_WAIT,
            SetThreadpoolWait, WaitForThreadpoolWaitCallbacks,
        },
    };

    use super::Acceptor;
    use crate::transport::Listener;

    /// Accept failures in a row (clients that gave up while waiting, say) after which
    /// a callback leaves the rest to the next signal.
    const MAX_FAILED_ACCEPTS: usize = 16;

    /// What the pool's callback works with. Freed only after the wait is closed and no
    /// callback runs.
    struct Context {
        listener: Listener,
        /// Manual-reset; signalled while a connection waits to be accepted.
        event: windows_sys::Win32::Networking::WinSock::WSAEVENT,
        acceptor: Acceptor,
    }

    /// The wait for connections. Dropping it stops waiting (letting a callback that
    /// already runs finish) and closes the listening socket.
    pub(super) struct Waiting {
        wait: PTP_WAIT,
        context: *mut Context,
    }

    // SAFETY: `context` is only read by the pool's callback, through a shared
    // reference, and freed by `Drop` after the last callback has returned. What it
    // holds (a socket, an event handle, the acceptor's `Arc`s and atomics) may be
    // used from any thread.
    unsafe impl Send for Waiting {}
    // SAFETY: as for `Send`; `Waiting` has no methods that take `&self`.
    unsafe impl Sync for Waiting {}

    fn wsa_error() -> io::Error {
        // SAFETY: reads this thread's last Winsock error.
        io::Error::from_raw_os_error(unsafe { WSAGetLastError() })
    }

    pub(super) fn start(listener: Listener, acceptor: Acceptor) -> io::Result<Waiting> {
        // SAFETY: no arguments; returns an event handle or null.
        let event = unsafe { WSACreateEvent() };
        if event == 0 {
            return Err(wsa_error());
        }
        // SAFETY: a listening socket and an event this function owns.
        if unsafe { WSAEventSelect(listener.raw(), event, FD_ACCEPT as i32) } != 0 {
            let err = wsa_error();
            // SAFETY: the event created above, not used elsewhere.
            unsafe { WSACloseEvent(event) };
            return Err(err);
        }
        let context = Box::into_raw(Box::new(Context {
            listener,
            event,
            acceptor,
        }));
        // SAFETY: `context` stays valid until `Waiting` is dropped, which closes the
        // wait first; the default pool and callback environment.
        let wait =
            unsafe { CreateThreadpoolWait(Some(on_connection), context.cast(), ptr::null()) };
        if wait == 0 {
            let err = io::Error::last_os_error();
            // SAFETY: no wait was created, so nothing else has the context.
            let context = unsafe { Box::from_raw(context) };
            // SAFETY: the event created above, no longer waited on.
            unsafe { WSACloseEvent(context.event) };
            return Err(err);
        }
        // SAFETY: a wait created above and an event that outlives it.
        unsafe { SetThreadpoolWait(wait, event as *mut c_void, ptr::null()) };
        Ok(Waiting { wait, context })
    }

    /// The pool's callback, run when a connection waits: accepts every connection
    /// waiting, then waits again, unless the server is stopping.
    unsafe extern "system" fn on_connection(
        _instance: PTP_CALLBACK_INSTANCE,
        context: *mut c_void,
        wait: PTP_WAIT,
        _result: u32,
    ) {
        // SAFETY: `Waiting::drop` frees the context only after the wait is closed and
        // this callback has returned.
        let context = unsafe { &*context.cast::<Context>() };
        let acceptor = &context.acceptor;
        if acceptor.stop.load(Ordering::SeqCst) {
            return;
        }
        // Resets the event before accepting, so a connection that arrives after the
        // last accept below signals it again.
        let mut events = WSANETWORKEVENTS::default();
        // SAFETY: the listening socket and its event, both owned by the context.
        unsafe { WSAEnumNetworkEvents(context.listener.raw(), context.event, &mut events) };
        let mut failed = 0;
        loop {
            match context.listener.accept() {
                Ok(stream) => {
                    failed = 0;
                    // It inherited the listener's event selection, and so isn't
                    // blocking: the connection's threads read and write blocking.
                    if stream.clear_event_select().is_err() {
                        continue;
                    }
                    if !acceptor.accepted(stream) {
                        return;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    failed += 1;
                    if failed >= MAX_FAILED_ACCEPTS {
                        break;
                    }
                }
            }
        }
        if !acceptor.stop.load(Ordering::SeqCst) {
            // SAFETY: this callback's own wait, and the context's event.
            unsafe { SetThreadpoolWait(wait, context.event as *mut c_void, ptr::null()) };
        }
    }

    impl Drop for Waiting {
        fn drop(&mut self) {
            // SAFETY: `wait` and `context` are the ones `start` made; the context is
            // freed once, after nothing can call back with it.
            unsafe {
                // A callback that already checked the flag may still wait again: the
                // wait is stopped, callbacks are waited for (and queued ones
                // cancelled), and then once more, after which none can start.
                (*self.context).acceptor.stop.store(true, Ordering::SeqCst);
                for _ in 0..2 {
                    SetThreadpoolWait(self.wait, ptr::null_mut(), ptr::null());
                    WaitForThreadpoolWaitCallbacks(self.wait, 1);
                }
                CloseThreadpoolWait(self.wait);
                let context = Box::from_raw(self.context);
                WSACloseEvent(context.event);
                // closes the listening socket
                drop(context);
            }
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

fn serve_connection(id: u64, stream: Stream, backend: &dyn Backend) {
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
