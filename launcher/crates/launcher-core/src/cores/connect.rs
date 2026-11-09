//! Lodestone Connect, the friend's side: a tunnel to a server a friend shares with an
//! invite link (see the `lodestone_connect` crate).
//!
//! [`open`] connects to the host of an invite on a thread of its own, with a small
//! Tokio runtime, and forwards two local ports into the tunnel:
//!
//! - the game port, which Minecraft joins as `127.0.0.1:<port>`;
//! - the info port, where a [`CoreClient`](super::CoreClient) at `http://127.0.0.1:<port>`
//!   reads the shared server's details and its client pack, as from a core whose
//!   servers are listed publicly.
//!
//! The [`Tunnel`] keeps both open until it is dropped, or until the host goes away.

use std::{
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
/// details and opens the two local ports. `progress` follows along.
pub async fn open(invite: Invite, progress: Progress) -> Result<Tunnel, JoinError> {
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
async fn run(
    invite: Invite,
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
        let game = joined.forward(StreamKind::Game, local).await?;
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
        playit_address: None,
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

    #[test]
    fn progress_starts_at_connecting() {
        let progress = Progress::default();
        assert_eq!(progress.stage(), Stage::Connecting);
        progress.set(Stage::ReadingInfo);
        assert_eq!(progress.stage(), Stage::ReadingInfo);
    }
}
