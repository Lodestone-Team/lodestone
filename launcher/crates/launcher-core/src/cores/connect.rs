//! Lodestone Connect, the friend's side: a tunnel to a server a friend shares with an
//! invite link (see the `lodestone_connect` crate).
//!
//! [`open`] connects to the host of an invite on a thread of its own, with a small
//! Tokio runtime, and forwards two local ports into the tunnel:
//!
//! - the game port, which Minecraft joins as `127.0.0.1:<port>`. A friend's server keeps
//!   the same one from session to session ([`GamePort`]), so an entry in Minecraft's
//!   server list keeps working;
//! - the info port, where a [`CoreClient`](super::CoreClient) at `http://127.0.0.1:<port>`
//!   reads the shared server's details and its client pack, as from a core whose
//!   servers are listed publicly. It is a new free port each time.
//!
//! Both listen on 127.0.0.1 only.
//!
//! The [`Tunnel`] keeps both open until it is dropped, or until the host goes away.

use std::{
    future::Future,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
    },
    time::Duration,
};

use futures::{
    FutureExt as _,
    channel::oneshot,
    future::{self, Either},
};
pub use lodestone_connect::{ConnectionKind, Invite, SharedServer};
use lodestone_connect::{
    StreamKind,
    join::{self, JoinOptions},
};
use url::Url;

use super::models::{ServerInstance, ServerLoader, ServerState};
use crate::game::launch::JoinServer;

/// How far [`open`] is, for the words the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    /// Collecting addresses and finding a way through both networks.
    Connecting = 0,
    /// The tunnel is open: reading the server's details.
    ReadingInfo = 1,
}

/// Where [`open`] is, shared with the window.
#[derive(Clone, Default)]
pub struct Progress(Arc<AtomicU8>);

impl Progress {
    pub fn stage(&self) -> Stage {
        match self.0.load(Ordering::Relaxed) {
            1 => Stage::ReadingInfo,
            _ => Stage::Connecting,
        }
    }

    fn set(&self, stage: Stage) {
        self.0.store(stage as u8, Ordering::Relaxed);
    }

    /// Shows the second stage, for the offscreen screenshots.
    pub fn preview_reading(&self) {
        self.set(Stage::ReadingInfo);
    }
}

/// Which local port the game port is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GamePort {
    /// Any free port: for "Check", which closes the tunnel at once.
    #[default]
    Any,
    /// The port saved for this server. When something else has it, a free port this
    /// time.
    Saved(u16),
    /// A first join: the first of these that is free, else any free port.
    Choose(Vec<u16>),
}

impl GamePort {
    /// The ports to try, in order, before any free port.
    pub fn ports(&self) -> &[u16] {
        match self {
            GamePort::Any => &[],
            GamePort::Saved(port) => std::slice::from_ref(port),
            GamePort::Choose(ports) => ports,
        }
    }
}

/// How long [`answers`] waits for a connection on this computer.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// Whether a program listens on `127.0.0.1:port`: another program, or this server's
/// tunnel open in another launcher. Binding 127.0.0.1 can succeed next to a program
/// that listens on all addresses, so binding alone doesn't tell.
async fn answers(port: u16) -> bool {
    let connect = probe(SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
    // no answer in time counts as taken, to be safe
    !matches!(
        tokio::time::timeout(PROBE_TIMEOUT, connect).await,
        Ok(Err(_))
    )
}

/// Connects to `at` on this computer, failing at once where nothing listens. Windows
/// sends a refused connection's SYN again for about two seconds before it fails,
/// longer than [`PROBE_TIMEOUT`], so every free port looked taken there; this socket
/// sends it once.
async fn probe(at: SocketAddr) -> std::io::Result<tokio::net::TcpStream> {
    let socket = tokio::net::TcpSocket::new_v4()?;
    #[cfg(windows)]
    send_syn_once(&socket);
    socket.connect(at).await
}

/// No SYN retransmissions on `socket` (`SIO_TCP_INITIAL_RTO`, Windows 8 and later).
/// Best effort: if Windows refuses it, the probe only takes longer.
#[cfg(windows)]
fn send_syn_once(socket: &tokio::net::TcpSocket) {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        SIO_TCP_INITIAL_RTO, SOCKET, TCP_INITIAL_RTO_NO_SYN_RETRANSMISSIONS,
        TCP_INITIAL_RTO_PARAMETERS, WSAIoctl,
    };
    let parameters = TCP_INITIAL_RTO_PARAMETERS {
        // TCP_INITIAL_RTO_UNSPECIFIED_RTT: the system's own
        Rtt: u16::MAX,
        // The header's ((UCHAR)-2), which windows-sys gives as a u16.
        MaxSynRetransmissions: TCP_INITIAL_RTO_NO_SYN_RETRANSMISSIONS as u8,
    };
    let mut returned = 0u32;
    // SAFETY: the socket is open for the call, and the input is a live
    // TCP_INITIAL_RTO_PARAMETERS of the size given; there is no output buffer and no
    // overlapped I/O.
    unsafe {
        WSAIoctl(
            socket.as_raw_socket() as SOCKET,
            SIO_TCP_INITIAL_RTO,
            std::ptr::from_ref(&parameters).cast(),
            std::mem::size_of::<TCP_INITIAL_RTO_PARAMETERS>() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
            None,
        );
    }
}

/// Binds the game port with `bind`: the first port of `wanted` nothing listens on and
/// that binds, else any free port (port 0).
async fn bind_game<T, E, F, Fut>(wanted: &GamePort, mut bind: F) -> Result<T, E>
where
    F: FnMut(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    for &port in wanted.ports() {
        if port == 0 || answers(port).await {
            continue;
        }
        if let Ok(bound) = bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await {
            return Ok(bound);
        }
    }
    bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await
}

/// Why joining failed, as the window explains it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JoinError {
    /// The link ended: the friend stopped sharing (or made a new link), or it never
    /// existed.
    #[error("Your friend stopped sharing this server. Ask them for a new link.")]
    Ended,
    /// The host isn't sharing right now (its computer or core is off).
    #[error("Your friend's server isn't being shared right now.")]
    HostOffline,
    /// No probe got through: these two networks need a relay.
    #[error("Your network and your friend's can't connect directly.")]
    NoPath,
    /// The host's certificate doesn't match the link.
    #[error("The computer that answered isn't the one in the invite link.")]
    WrongHost,
    /// This computer seems to be offline.
    #[error("This computer seems to be offline.")]
    Offline,
    /// connect.lodestone.cc couldn't be reached.
    #[error("Couldn't reach the Lodestone Connect service: {0}")]
    Unreachable(String),
    /// Anything else, as a sentence.
    #[error("{0}")]
    Failed(String),
}

impl From<lodestone_connect::Error> for JoinError {
    fn from(err: lodestone_connect::Error) -> Self {
        use lodestone_connect::Error;
        match &err {
            Error::Service { code, .. } if code == "host_offline" => JoinError::HostOffline,
            Error::Service { code, status, .. } if code == "not_found" || *status == 404 => {
                JoinError::Ended
            }
            Error::NoPath(_) => JoinError::NoPath,
            Error::WrongHost => JoinError::WrongHost,
            Error::NoAddress => JoinError::Offline,
            Error::Unreachable(detail) => JoinError::Unreachable(detail.clone()),
            _ => JoinError::Failed(err.to_string()),
        }
    }
}

/// An open tunnel to a friend's server. Dropping it closes the tunnel and its ports.
pub struct Tunnel {
    /// The shared server, as the host said when the tunnel opened.
    pub server: SharedServer,
    /// How this computer reaches the host.
    pub kind: ConnectionKind,
    game: SocketAddr,
    info: SocketAddr,
    rtt_ms: Arc<AtomicU32>,
    closed: Arc<AtomicBool>,
    stop: Option<oneshot::Sender<()>>,
    ended: Option<oneshot::Receiver<String>>,
}

impl std::fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunnel")
            .field("server", &self.server.name)
            .field("kind", &self.kind)
            .field("game", &self.game)
            .finish_non_exhaustive()
    }
}

/// What the tunnel's thread hands back once it is open.
struct Opened {
    server: SharedServer,
    kind: ConnectionKind,
    game: SocketAddr,
    info: SocketAddr,
}

/// How long closing may take before the thread gives up on a clean goodbye.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(3);

/// Connects to the host of `invite` (2 to 10 seconds), reads the shared server's
/// details and opens the two local ports, the game port as `game_port` says.
/// `progress` follows along.
pub async fn open(
    invite: Invite,
    game_port: GamePort,
    progress: Progress,
) -> Result<Tunnel, JoinError> {
    let (ready_tx, ready_rx) = oneshot::channel::<Result<Opened, JoinError>>();
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let (ended_tx, ended_rx) = oneshot::channel::<String>();
    let rtt_ms = Arc::new(AtomicU32::new(0));
    let closed = Arc::new(AtomicBool::new(false));
    let thread_rtt = rtt_ms.clone();
    let thread_closed = closed.clone();
    std::thread::Builder::new()
        .name("lodestone-connect".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = ready_tx.send(Err(JoinError::Failed(err.to_string())));
                    return;
                }
            };
            runtime.block_on(run(
                invite,
                game_port,
                progress,
                ready_tx,
                stop_rx,
                ended_tx,
                thread_rtt,
                thread_closed,
            ));
        })
        .map_err(|err| JoinError::Failed(err.to_string()))?;
    let opened = ready_rx
        .await
        .map_err(|_| JoinError::Failed("the tunnel's thread stopped".into()))??;
    Ok(Tunnel {
        server: opened.server,
        kind: opened.kind,
        game: opened.game,
        info: opened.info,
        rtt_ms,
        closed,
        stop: Some(stop_tx),
        ended: Some(ended_rx),
    })
}

/// The tunnel's thread: joins, forwards the ports, then waits for the end.
#[allow(clippy::too_many_arguments)]
async fn run(
    invite: Invite,
    game_port: GamePort,
    progress: Progress,
    ready: oneshot::Sender<Result<Opened, JoinError>>,
    stop: oneshot::Receiver<()>,
    ended: oneshot::Sender<String>,
    rtt_ms: Arc<AtomicU32>,
    closed: Arc<AtomicBool>,
) {
    progress.set(Stage::Connecting);
    let joined = match join::join(&invite, JoinOptions::default()).await {
        Ok(joined) => joined,
        Err(err) => {
            let _ = ready.send(Err(err.into()));
            return;
        }
    };
    progress.set(Stage::ReadingInfo);
    let local = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let opened = async {
        let server = joined.server_info().await?;
        let game = bind_game(&game_port, |at| joined.forward(StreamKind::Game, at)).await?;
        let info = joined.forward(StreamKind::Info, local).await?;
        Ok::<_, lodestone_connect::Error>((server, game, info))
    }
    .await;
    let (server, game, info) = match opened {
        Ok(opened) => opened,
        Err(err) => {
            let _ = ready.send(Err(err.into()));
            joined.close().await;
            return;
        }
    };
    rtt_ms.store(joined.rtt().as_millis() as u32, Ordering::Relaxed);
    let sent = ready.send(Ok(Opened {
        server,
        kind: joined.kind(),
        game: game.local_addr(),
        info: info.local_addr(),
    }));
    if sent.is_err() {
        // nobody waits any more (the join was cancelled)
        joined.close().await;
        return;
    }
    // the round trip time, for the window, until the tunnel closes or is stopped
    let measure = async {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            rtt_ms.store(joined.rtt().as_millis() as u32, Ordering::Relaxed);
        }
    };
    let why = {
        let host_closed = joined.closed().boxed_local();
        let stopped = stop.boxed_local();
        let either = future::select(host_closed, stopped);
        match future::select(either, measure.boxed_local()).await {
            Either::Left((Either::Left((why, _)), _)) => Some(why),
            // stopped, or the Tunnel was dropped
            _ => None,
        }
    };
    closed.store(true, Ordering::Relaxed);
    drop((game, info));
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, joined.close()).await;
    if let Some(why) = why {
        let _ = ended.send(why);
    }
}

impl Tunnel {
    /// Where Minecraft joins the server: `127.0.0.1:<the game port>`.
    pub fn join_target(&self) -> JoinServer {
        JoinServer {
            host: self.game.ip().to_string(),
            port: self.game.port(),
        }
    }

    /// The game port this tunnel listens on.
    pub fn game_port(&self) -> u16 {
        self.game.port()
    }

    /// The address to point a [`CoreClient`](super::CoreClient) at, for the shared
    /// server's details and client pack.
    pub fn info_url(&self) -> Url {
        Url::parse(&format!("http://{}", self.info)).expect("an IP address and port is a URL")
    }

    /// The round trip time to the host, last measured.
    pub fn rtt(&self) -> Duration {
        Duration::from_millis(self.rtt_ms.load(Ordering::Relaxed).into())
    }

    /// Whether the tunnel has closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Resolves with why when the host ends the tunnel (not when it is dropped).
    /// Once only.
    pub fn take_ended(&mut self) -> Option<oneshot::Receiver<String>> {
        self.ended.take()
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

/// The shared server as a server on a core, for the launcher's matching instance, client
/// pack and mod sync. Its address is the tunnel's, so none is given here.
pub fn server_instance(server: &SharedServer) -> ServerInstance {
    ServerInstance {
        uuid: server.instance_uuid.clone(),
        name: server.name.clone(),
        game_version: server.game_version.clone(),
        loader: loader(&server.loader),
        loader_version: server.loader_version.clone().filter(|v| !v.is_empty()),
        port: None,
        state: server.state.as_deref().map_or(ServerState::Unknown, state),
        player_count: server.player_count,
        max_player_count: server.max_player_count,
        has_client_pack: Some(server.modpack),
        listed: false,
    }
}

/// A loader's name as the host sends it.
pub fn loader(name: &str) -> ServerLoader {
    serde_json::from_value(serde_json::Value::String(name.to_ascii_lowercase()))
        .unwrap_or(ServerLoader::Other)
}

/// A state's name as the host sends it.
fn state(name: &str) -> ServerState {
    serde_json::from_value(serde_json::Value::String(name.to_string()))
        .unwrap_or(ServerState::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared(loader: &str, version: Option<&str>, state: Option<&str>) -> SharedServer {
        SharedServer {
            core_name: "Alex's core".into(),
            core_uuid: "c".into(),
            instance_uuid: "i-1".into(),
            name: "SMP".into(),
            game_version: "1.21.4".into(),
            loader: loader.into(),
            loader_version: version.map(str::to_string),
            modpack: loader == "fabric",
            state: state.map(str::to_string),
            player_count: Some(2),
            max_player_count: Some(20),
        }
    }

    #[test]
    fn a_shared_server_reads_as_a_server_on_a_core() {
        let server = server_instance(&shared("fabric", Some("0.16.14"), Some("Running")));
        assert_eq!(server.uuid, "i-1");
        assert_eq!(server.loader, ServerLoader::Fabric);
        assert_eq!(server.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(server.state, ServerState::Running);
        assert!(server.has_pack());
        assert_eq!(server.port, None);
        assert_eq!(server.players_label().as_deref(), Some("2 / 20 players"));

        let plain = server_instance(&shared("paper", Some(""), None));
        assert_eq!(plain.loader, ServerLoader::Paper);
        assert_eq!(plain.loader_version, None);
        assert_eq!(plain.state, ServerState::Unknown);
        assert!(!plain.has_pack());
        assert!(!plain.mods_hidden());

        assert_eq!(loader("Forge"), ServerLoader::Forge);
        assert_eq!(loader("something"), ServerLoader::Other);
        assert_eq!(state("Stopped"), ServerState::Stopped);
        assert_eq!(state("nonsense"), ServerState::Unknown);
    }

    #[test]
    fn errors_people_can_act_on() {
        use lodestone_connect::Error;
        let service = |status: u16, code: &str| Error::Service {
            status,
            code: code.into(),
            message: "m".into(),
        };
        assert_eq!(JoinError::from(service(404, "not_found")), JoinError::Ended);
        assert_eq!(
            JoinError::from(service(404, "host_offline")),
            JoinError::HostOffline
        );
        assert_eq!(
            JoinError::from(Error::NoPath("x".into())),
            JoinError::NoPath
        );
        assert_eq!(JoinError::from(Error::WrongHost), JoinError::WrongHost);
        assert_eq!(JoinError::from(Error::NoAddress), JoinError::Offline);
        assert!(matches!(
            JoinError::from(Error::Unreachable("dns".into())),
            JoinError::Unreachable(_)
        ));
        assert!(matches!(
            JoinError::from(service(500, "internal")),
            JoinError::Failed(_)
        ));
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A port nothing listens on now.
    fn free_port() -> u16 {
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// Binds as the tunnel's forward does.
    async fn listen(at: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
        tokio::net::TcpListener::bind(at).await
    }

    fn port_of(listener: &tokio::net::TcpListener) -> u16 {
        listener.local_addr().unwrap().port()
    }

    #[test]
    fn the_game_port_binds_the_saved_port_or_falls_back() {
        block_on(async {
            // the saved port, when it is free
            let saved = free_port();
            let bound = bind_game(&GamePort::Saved(saved), listen).await.unwrap();
            assert_eq!(port_of(&bound), saved);
            assert!(bound.local_addr().unwrap().ip().is_loopback());

            // taken (this server's tunnel open elsewhere): another port, this time
            let again = bind_game(&GamePort::Saved(saved), listen).await.unwrap();
            assert_ne!(port_of(&again), saved);
            assert_ne!(port_of(&again), 0);
            drop(bound);

            // a program listening on every address also counts as taken, though
            // 127.0.0.1 alone might bind next to it
            let everywhere = std::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let busy = everywhere.local_addr().unwrap().port();
            let bound = bind_game(&GamePort::Saved(busy), listen).await.unwrap();
            assert_ne!(port_of(&bound), busy);

            // a first join takes the first free choice
            let free = free_port();
            let bound = bind_game(&GamePort::Choose(vec![busy, free]), listen)
                .await
                .unwrap();
            assert_eq!(port_of(&bound), free);
            // and any free port when none is
            let none = bind_game(&GamePort::Choose(vec![busy, free]), listen)
                .await
                .unwrap();
            assert!(![busy, free, 0].contains(&port_of(&none)));
            // Check takes any free port
            let any = bind_game(&GamePort::Any, listen).await.unwrap();
            assert!(![busy, free, 0].contains(&port_of(&any)));
        });
    }

    /// A free port is found free well within the probe's time, on Windows too.
    #[test]
    fn nothing_listening_answers_at_once() {
        block_on(async {
            let free = free_port();
            let started = std::time::Instant::now();
            assert!(!answers(free).await);
            assert!(
                started.elapsed() < PROBE_TIMEOUT,
                "{:?} for a refused connection",
                started.elapsed()
            );
            let listening = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            assert!(answers(listening.local_addr().unwrap().port()).await);
        });
    }

    #[test]
    fn progress_starts_at_connecting() {
        let progress = Progress::default();
        assert_eq!(progress.stage(), Stage::Connecting);
        progress.set(Stage::ReadingInfo);
        assert_eq!(progress.stage(), Stage::ReadingInfo);
    }
}
