//! Lodestone Connect: sharing an instance with friends through an invite link.
//!
//! A shared instance has a [`Host`] from the `lodestone_connect` crate: it keeps an
//! invite open at connect.lodestone.cc and a direct, encrypted tunnel to each friend who
//! opens the link in Lodestone Launcher. The tunnel carries two kinds of streams:
//!
//! - game streams, passed to the instance's game port on this computer;
//! - info streams: HTTP for [`info_router`], a few read-only routes about this instance
//!   only (its name, version, loader and client modpack). Nothing else of the API is
//!   reachable through an invite.
//!
//! Each shared instance's identity (certificate, key, token, and the invite's code and
//! secret) is saved in `stores/connect/<uuid>.json`, readable only by the core's user,
//! so the same link keeps working after a restart: sharing resumes at startup. Stopping
//! ends the link for good and deletes the file. The file API never reaches `stores/`.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{Path as UrlPath, State},
    http::HeaderMap,
    response::Response,
    routing::get,
    Json, Router,
};
use color_eyre::eyre::eyre;
use lodestone_connect::{
    host::{FriendId, Host, HostConfig, HostIdentity, HostState, HostStatus, StreamHandler},
    ConnectionKind, GatherOptions, Service, SharedServer, TunnelStream,
};
use serde::Serialize;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tracing::{info, warn};
use ts_rs::TS;

pub mod friends;

use crate::{
    error::{Error, ErrorKind},
    handlers::{
        public_instances::{pack_response, public_instance},
        util::get_instance,
    },
    public_listing::{PublicInstance, PublicInstanceList, PublicScope},
    traits::{
        t_configurable::{Game, TConfigurable},
        t_server::{State as ServerState, TServer},
    },
    types::InstanceUuid,
    AppState,
};

/// The folder in `stores/` with the shared instances' identities.
pub const CONNECT_DIR: &str = "connect";
/// Overrides the introduction service (for development: `http://127.0.0.1:8787` with
/// `npm run dev` in services/connect).
pub const SERVICE_ENV: &str = "LODESTONE_CONNECT_SERVICE";
/// How often resuming a share is tried again after it failed (offline at startup).
const RESUME_RETRY: Duration = Duration::from_secs(60);

/// What friends see of a shared instance on its info streams: no address (they play
/// through the tunnel), its status and client pack.
const SHARED_SCOPE: PublicScope = PublicScope {
    listed: true,
    status: true,
    address: false,
    client_pack: true,
};

// ---------------------------------------------------------------------------
// Status, as the API reports it

/// `GET /instance/{uuid}/share`: whether the instance is shared with friends, and who is
/// connected.
#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct InstanceShare {
    pub sharing: bool,
    /// `null` while not sharing.
    pub state: Option<ShareState>,
    /// The invite link to send to friends: `https://connect.lodestone.cc/j/...`.
    pub link: Option<String>,
    /// The link keeps working after the core restarts.
    pub stable_link: bool,
    /// How many friends joined through the link since sharing (re)started.
    pub joins: u32,
    /// The friends connected now.
    pub friends: Vec<SharedFriend>,
    /// The last problem, such as the introduction service being unreachable.
    pub problem: Option<String>,
    /// This network's router makes direct connections unlikely to work ("symmetric
    /// NAT"): friends may not be able to join.
    pub hard_network: bool,
}

impl InstanceShare {
    fn off() -> Self {
        Self {
            sharing: false,
            state: None,
            link: None,
            stable_link: false,
            joins: 0,
            friends: Vec::new(),
            problem: None,
            hard_network: false,
        }
    }

    fn starting() -> Self {
        Self {
            sharing: true,
            state: Some(ShareState::Starting),
            ..Self::off()
        }
    }

    fn of(status: HostStatus) -> Self {
        Self {
            sharing: status.state != HostState::Stopped,
            state: Some(match status.state {
                HostState::Sharing => ShareState::Sharing,
                _ => ShareState::Reconnecting,
            }),
            link: Some(status.link).filter(|link| !link.is_empty()),
            stable_link: status.stable_link,
            joins: u32::try_from(status.joins).unwrap_or(u32::MAX),
            friends: status
                .friends
                .into_iter()
                .map(|friend| SharedFriend {
                    id: u32::try_from(friend.id).unwrap_or(u32::MAX),
                    address: friend.address.ip().to_string(),
                    connection: match friend.kind {
                        ConnectionKind::SameComputer => ShareConnection::SameComputer,
                        ConnectionKind::LocalNetwork => ShareConnection::LocalNetwork,
                        ConnectionKind::Ipv6 => ShareConnection::Ipv6,
                        ConnectionKind::Internet => ShareConnection::Internet,
                    },
                    connected_at: i64::try_from(friend.connected_at).unwrap_or(0),
                    rtt_ms: u32::try_from(friend.rtt_ms).unwrap_or(u32::MAX),
                })
                .collect(),
            problem: status.problem,
            hard_network: status.symmetric_nat,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ShareState {
    /// Finding this computer's addresses and opening the invite (a few seconds).
    Starting,
    /// Friends can join.
    Sharing,
    /// The introduction service can't be reached: friends already connected keep
    /// playing, new ones can't join until it's back.
    Reconnecting,
}

/// A friend connected to a shared instance.
#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct SharedFriend {
    /// Numbered from 1 in the order friends connected.
    pub id: u32,
    /// The friend's IP address as the tunnel sees it.
    pub address: String,
    pub connection: ShareConnection,
    /// When they connected, in seconds since 1970.
    #[ts(type = "number")]
    pub connected_at: i64,
    /// The round trip time in milliseconds.
    pub rtt_ms: u32,
}

/// How a friend is connected: always directly, over one of these.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ShareConnection {
    SameComputer,
    LocalNetwork,
    Ipv6,
    Internet,
}

// ---------------------------------------------------------------------------
// The shares

enum Slot {
    /// `Host::start` is running. The ticket tells this start apart from a later one of
    /// the same instance: stopping and sharing again while it runs leaves the later
    /// start's slot here, which this one must not take over.
    Starting(Arc<StartTicket>),
    Running {
        host: Box<Host>,
        tasks: Vec<JoinHandle<()>>,
    },
    /// Was starting when the core began shutting down ([`ConnectShares::stop_all`]): the
    /// start stops its host once it has one, keeping the link. The start's ticket, as
    /// in [`Slot::Starting`].
    Abandoned(Arc<StartTicket>),
}

/// A share being started, held by the start and by its [`Slot::Starting`].
#[derive(Default)]
struct StartTicket {
    /// Set by [`ConnectShares::stop`] as it stops this start: whether it ended the
    /// link. The start, once it has its host, ends the link then, as the stop couldn't
    /// without a host.
    end_link: AtomicBool,
}

impl StartTicket {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

/// What a start does with its host once `Host::start` returned.
#[derive(Debug, PartialEq, Eq)]
enum Finish {
    /// It's still this start's slot: save the identity and share.
    Install,
    /// The core began shutting down meanwhile: stop, keeping the link.
    ShuttingDown,
    /// The owner stopped sharing meanwhile, maybe sharing again since (the slot is
    /// then another start's or share's, and the saved identity theirs): stop the host,
    /// ending the link if the stop did, and touch nothing else.
    Stopped { end_link: bool },
}

impl Finish {
    /// Decides, under the slots' lock, what the start with `ticket` does.
    fn of(slot: Option<&Slot>, ticket: &Arc<StartTicket>) -> Self {
        match slot {
            Some(Slot::Starting(current)) if Arc::ptr_eq(current, ticket) => Finish::Install,
            Some(Slot::Abandoned(current)) if Arc::ptr_eq(current, ticket) => Finish::ShuttingDown,
            _ => Finish::Stopped {
                end_link: ticket.end_link.load(Ordering::SeqCst),
            },
        }
    }

    /// Whether the slot is still this start's (Starting or Abandoned), for it to remove.
    fn owns_slot(&self) -> bool {
        !matches!(self, Finish::Stopped { .. })
    }
}

/// The instances shared with friends.
pub struct ConnectShares {
    /// Sharing with friends from the friend list (no links).
    pub friends: Arc<friends::FriendShares>,
    dir: PathBuf,
    service: Service,
    gather: GatherOptions,
    slots: Mutex<HashMap<InstanceUuid, Slot>>,
    /// Set by [`Self::stop_all`] as the core shuts down: nothing starts sharing after it.
    stopping: AtomicBool,
}

fn external(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::External,
        source: eyre!("{}", message.into()),
    }
}

fn internal(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::Internal,
        source: eyre!("{}", message.into()),
    }
}

impl ConnectShares {
    /// Shares kept in `dir` (`stores/connect`), through `service`.
    pub fn new(dir: PathBuf, service: Service, gather: GatherOptions) -> Self {
        Self {
            friends: Arc::new(friends::FriendShares::new(
                dir.join(friends::FRIENDS_DIR),
                service.clone(),
                gather,
            )),
            dir,
            service,
            gather,
            slots: Mutex::new(HashMap::new()),
            stopping: AtomicBool::new(false),
        }
    }

    fn shutting_down() -> Error {
        Error {
            kind: ErrorKind::Conflict,
            source: eyre!("Lodestone Core is shutting down"),
        }
    }

    /// The service from [`SERVICE_ENV`], else connect.lodestone.cc. Lodestone Host
    /// sets it from its developer setting.
    pub fn service_from_env() -> Service {
        service_from(std::env::var(SERVICE_ENV).ok().as_deref())
    }

    fn file(&self, uuid: &InstanceUuid) -> PathBuf {
        self.dir.join(format!("{uuid}.json"))
    }

    /// The saved identity of `uuid`, if it was shared.
    async fn load(&self, uuid: &InstanceUuid) -> Option<HostIdentity> {
        let text = tokio::fs::read_to_string(self.file(uuid)).await.ok()?;
        match HostIdentity::from_json(&text) {
            Ok(identity) => Some(identity),
            Err(e) => {
                warn!("Couldn't read the sharing identity of {uuid}, making a new link: {e}");
                None
            }
        }
    }

    /// Saves `identity` owner-only (the folder 0700, the file 0600 on Unix).
    async fn save(&self, uuid: &InstanceUuid, identity: &HostIdentity) -> Result<(), Error> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || crate::local_claim::ensure_private_dir(&dir))
            .await
            .map_err(|e| internal(format!("Failed to create the sharing folder: {e}")))??;
        crate::persist::write_atomic(self.file(uuid), identity.to_json(), Some(0o600)).await
    }

    async fn forget(&self, uuid: &InstanceUuid) {
        match tokio::fs::remove_file(self.file(uuid)).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("Failed to delete the sharing identity of {uuid}: {e}"),
        }
    }

    /// How sharing `uuid` is going.
    pub async fn status(&self, uuid: &InstanceUuid) -> InstanceShare {
        match self.slots.lock().await.get(uuid) {
            None | Some(Slot::Abandoned(_)) => InstanceShare::off(),
            Some(Slot::Starting(_)) => InstanceShare::starting(),
            Some(Slot::Running { host, .. }) => InstanceShare::of(host.status()),
        }
    }

    /// Starts sharing `uuid` (a Minecraft Java instance), with its saved link if it has
    /// one. Returns once the link is known. Sharing an instance already shared changes
    /// nothing.
    pub async fn start(
        self: &Arc<Self>,
        state: &AppState,
        uuid: &InstanceUuid,
    ) -> Result<InstanceShare, Error> {
        let instance = get_instance(state, uuid)?;
        if !matches!(instance.game_type().await, Game::MinecraftJava { .. }) {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Only Minecraft Java Edition servers can be shared with friends"),
            });
        }
        let ticket = StartTicket::new();
        {
            let mut slots = self.slots.lock().await;
            // under the lock that `stop_all` drains the slots with, so a share either
            // starts before it (and is stopped by it) or not at all
            if self.stopping.load(Ordering::SeqCst) {
                return Err(Self::shutting_down());
            }
            if slots.contains_key(uuid) {
                drop(slots);
                return Ok(self.status(uuid).await);
            }
            slots.insert(uuid.clone(), Slot::Starting(ticket.clone()));
        }
        let started = self.start_host(state, uuid).await;
        let mut slots = self.slots.lock().await;
        let (host, tasks) = match started {
            Ok(started) => started,
            Err(e) => {
                // only this start's slot: a later start may have taken it meanwhile
                if Finish::of(slots.get(uuid), &ticket).owns_slot() {
                    slots.remove(uuid);
                }
                if self.stopping.load(Ordering::SeqCst) {
                    return Err(Self::shutting_down());
                }
                return Err(e);
            }
        };
        let finish = Finish::of(slots.get(uuid), &ticket);
        // The identity is saved under the lock, and only while the slot is still this
        // start's: a stop deletes the file under the same lock, so a start it stopped
        // can't write it back, nor a later start load this one's link.
        let saved = match finish {
            Finish::Install | Finish::ShuttingDown => self.save(uuid, &host.identity()).await,
            Finish::Stopped { .. } => Ok(()),
        };
        if finish == Finish::Install && saved.is_ok() {
            let status = InstanceShare::of(host.status());
            slots.insert(
                uuid.clone(),
                Slot::Running {
                    host: Box::new(host),
                    tasks,
                },
            );
            info!("Sharing {} with friends", instance.name().await);
            return Ok(status);
        }
        if finish.owns_slot() {
            slots.remove(uuid);
        }
        drop(slots);
        for task in tasks {
            task.abort();
        }
        match (finish, saved) {
            (_, Err(e)) => {
                // a link nobody could open again after a restart
                host.stop(true).await;
                Err(e)
            }
            (Finish::ShuttingDown, Ok(())) => {
                // keep the link, as `stop_all` does, to resume with it at the next start
                host.stop(false).await;
                Err(Self::shutting_down())
            }
            (Finish::Stopped { end_link }, Ok(())) => {
                // The stop found no host, so it's this start that ends the link it
                // renewed. The saved identity is the stop's or a later start's to keep
                // or delete.
                host.stop(end_link).await;
                Ok(InstanceShare::off())
            }
            (Finish::Install, Ok(())) => unreachable!("installed above"),
        }
    }

    async fn start_host(
        self: &Arc<Self>,
        state: &AppState,
        uuid: &InstanceUuid,
    ) -> Result<(Host, Vec<JoinHandle<()>>), Error> {
        let identity = match self.load(uuid).await {
            Some(identity) => identity,
            None => HostIdentity::generate().map_err(|e| internal(e.to_string()))?,
        };
        let (info_tx, info_rx) = mpsc::channel(16);
        let handler = CoreStreams {
            state: state.clone(),
            uuid: uuid.clone(),
            info: info_tx,
        };
        let config = HostConfig {
            service: self.service.clone(),
            gather: self.gather,
            keep: true,
        };
        let host = Host::start(identity, config, handler)
            .await
            .map_err(|e| external(format!("Couldn't start sharing: {e}")))?;
        let tasks = vec![
            tokio::spawn(serve_info(state.clone(), uuid.clone(), info_rx)),
            tokio::spawn(self.clone().keep_saved(uuid.clone(), host.subscribe())),
        ];
        Ok((host, tasks))
    }

    /// Saves the identity whenever it changes (a renewed or new invite).
    async fn keep_saved(
        self: Arc<Self>,
        uuid: InstanceUuid,
        mut changed: tokio::sync::watch::Receiver<u64>,
    ) {
        let mut saved = None;
        while changed.changed().await.is_ok() {
            // Saved under the lock a stop deletes the file with, so a share stopped
            // meanwhile can't write its identity back.
            let slots = self.slots.lock().await;
            let identity = match slots.get(&uuid) {
                Some(Slot::Running { host, .. }) => host.identity(),
                _ => continue,
            };
            let json = identity.to_json();
            if saved.as_ref() == Some(&json) {
                continue;
            }
            let result = self.save(&uuid, &identity).await;
            drop(slots);
            match result {
                Ok(()) => saved = Some(json),
                Err(e) => warn!("Failed to save the sharing identity of {uuid}: {e}"),
            }
        }
    }

    /// Stops sharing `uuid`. With `end_link`, the link stops working for good and the
    /// saved identity is deleted (the owner stopped sharing, or the instance is
    /// deleted); without it the link works again once sharing resumes.
    ///
    /// A share still starting is told to end its link (or keep it) once it has its
    /// host, as there's none to stop yet.
    pub async fn stop(&self, uuid: &InstanceUuid, end_link: bool) {
        let slot = {
            let mut slots = self.slots.lock().await;
            let slot = slots.remove(uuid);
            if let Some(Slot::Starting(ticket)) = &slot {
                ticket.end_link.store(end_link, Ordering::SeqCst);
            }
            // Under the lock a start inserts its slot with, so a share started after
            // this stop never loads the identity being deleted.
            if end_link {
                self.forget(uuid).await;
            }
            slot
        };
        if let Some(Slot::Running { host, tasks }) = slot {
            for task in tasks {
                task.abort();
            }
            (*host).stop(end_link).await;
            info!("Stopped sharing {uuid} with friends");
        }
    }

    /// Stops sharing every instance as the core shuts down, keeping the links: sharing
    /// resumes at the next start. Closing the connections tells connected friends at
    /// once, instead of after their connection times out.
    pub async fn stop_all(&self) {
        // none starts after this
        self.stopping.store(true, Ordering::SeqCst);
        self.friends.stop().await;
        let slots: Vec<_> = {
            let mut slots = self.slots.lock().await;
            let drained: Vec<_> = slots.drain().collect();
            // a share still starting stops once it has its host, keeping its link
            for (uuid, slot) in &drained {
                if let Slot::Starting(ticket) = slot {
                    slots.insert(uuid.clone(), Slot::Abandoned(ticket.clone()));
                }
            }
            drained
        };
        for (uuid, slot) in slots {
            if let Slot::Running { host, tasks } = slot {
                for task in tasks {
                    task.abort();
                }
                (*host).stop(false).await;
                info!("Stopped sharing {uuid} with friends (shutting down)");
            }
        }
    }

    /// Resumes sharing the instances that were shared when the core last stopped,
    /// retrying in the background while this computer is offline. Saved identities of
    /// instances that no longer exist are deleted.
    pub async fn resume_all(self: &Arc<Self>, state: &AppState) {
        {
            let friends = self.friends.clone();
            let state = state.clone();
            tokio::spawn(async move { friends.resume(&state).await });
        }
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(uuid) = name.strip_suffix(".json") else {
                continue;
            };
            let uuid = InstanceUuid::from(uuid.to_string());
            if !state.instances.contains_key(&uuid) {
                self.forget(&uuid).await;
                continue;
            }
            let shares = self.clone();
            let state = state.clone();
            tokio::spawn(async move {
                loop {
                    match shares.start(&state, &uuid).await {
                        Ok(_) => return,
                        // the core is shutting down
                        Err(_) if shares.stopping.load(Ordering::SeqCst) => return,
                        Err(e) => warn!("Couldn't resume sharing {uuid}, trying again soon: {e}"),
                    }
                    tokio::time::sleep(RESUME_RETRY).await;
                    // stopped, deleted or shutting down meanwhile
                    if !shares.file(&uuid).exists()
                        || !state.instances.contains_key(&uuid)
                        || shares.stopping.load(Ordering::SeqCst)
                    {
                        return;
                    }
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Streams

/// Passes a shared instance's game streams to its game port, and its info streams to
/// [`serve_info`].
struct CoreStreams {
    state: AppState,
    uuid: InstanceUuid,
    info: mpsc::Sender<TunnelStream>,
}

impl StreamHandler for CoreStreams {
    fn game(&self, stream: TunnelStream, _friend: FriendId) {
        let state = self.state.clone();
        let uuid = self.uuid.clone();
        tokio::spawn(async move {
            let Ok(instance) = get_instance(&state, &uuid) else {
                return;
            };
            if !game_port_open(instance.state().await) {
                return;
            }
            let Ok(port) = u16::try_from(instance.port().await) else {
                return;
            };
            lodestone_connect::host::pipe(stream, SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
                .await;
        });
    }

    fn info(&self, stream: TunnelStream, _friend: FriendId) {
        // dropped (closed) when too many are waiting
        let _ = self.info.try_send(stream);
    }
}

/// The service `value` (from [`SERVICE_ENV`]) names, checked as Lodestone Launcher and
/// Host check their setting ([`Service::from_setting`]): HTTPS, or plain HTTP to this
/// computer only. Anything else is warned about and connect.lodestone.cc used.
pub(crate) fn service_from(value: Option<&str>) -> Service {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Service::default();
    };
    match Service::from_setting(text) {
        Ok(service) => {
            if service != Service::default() {
                info!("Sharing through the Lodestone Connect service at {text}");
            }
            service
        }
        Err(e) => {
            warn!("{SERVICE_ENV} ({text}) isn't a service URL: {e}. Using connect.lodestone.cc");
            Service::default()
        }
    }
}

/// Lets debug builds pass game streams to a stopped instance's port, so the tunnel can
/// be tested against a plain TCP listener without starting a Minecraft server.
pub const ANY_STATE_ENV: &str = "LODESTONE_CONNECT_ANY_STATE";

/// Whether friends' game streams reach the instance's game port. Only while the server
/// runs: then the port is the Minecraft server's. A stopped instance's port could be
/// set to another service on this computer (SSH, the core itself), which a link must
/// never reach.
fn game_port_open(state: ServerState) -> bool {
    state == ServerState::Running
        || (cfg!(debug_assertions) && std::env::var_os(ANY_STATE_ENV).is_some())
}

/// Info streams, as connections for axum.
struct StreamListener<Io> {
    streams: mpsc::Receiver<Io>,
}

impl<Io> axum::serve::Listener for StreamListener<Io>
where
    Io: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Io = Io;
    type Addr = ();

    async fn accept(&mut self) -> (Io, ()) {
        match self.streams.recv().await {
            Some(stream) => (stream, ()),
            // the share stopped: nothing more comes
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Answers HTTP on each stream from `streams` with [`info_router`].
pub(crate) async fn serve_info<Io>(state: AppState, uuid: InstanceUuid, streams: mpsc::Receiver<Io>)
where
    Io: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let _ = axum::serve(StreamListener { streams }, info_router(state, uuid)).await;
}

#[derive(Clone)]
struct Shared {
    state: AppState,
    uuid: InstanceUuid,
}

/// The routes friends reach through info streams, for the shared instance only:
///
/// - `GET /api/v1/connect/info`: [`SharedServer`];
/// - `GET /api/v1/public/instances`: the instance alone, as the public list shows it;
/// - `GET /api/v1/public/instances/{uuid}/client-pack`: its client modpack.
///
/// Everything else is 404.
pub(crate) fn info_router(state: AppState, uuid: InstanceUuid) -> Router {
    Router::new()
        .route("/api/v1/connect/info", get(get_shared_server))
        .route("/api/v1/public/instances", get(get_shared_list))
        .route(
            "/api/v1/public/instances/{uuid}/client-pack",
            get(get_shared_pack),
        )
        .with_state(Shared { state, uuid })
}

fn not_found() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    }
}

async fn shared_instance(shared: &Shared) -> Result<PublicInstance, Error> {
    let instance = get_instance(&shared.state, &shared.uuid)?;
    public_instance(&instance, SHARED_SCOPE)
        .await
        .ok_or_else(not_found)
}

fn as_text<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        _ => String::new(),
    }
}

async fn get_shared_server(State(shared): State<Shared>) -> Result<Json<SharedServer>, Error> {
    let instance = shared_instance(&shared).await?;
    Ok(Json(shared_server_of(&shared.state, instance).await))
}

/// What friends see of a shared instance.
pub(crate) async fn shared_server_of(state: &AppState, instance: PublicInstance) -> SharedServer {
    SharedServer {
        core_name: state.global_settings.lock().await.core_name(),
        core_uuid: state.uuid.clone(),
        instance_uuid: instance.uuid.to_string(),
        name: instance.name,
        game_version: instance.game_version,
        loader: as_text(&instance.loader),
        loader_version: instance.loader_version,
        modpack: instance.has_client_pack.unwrap_or(false),
        state: instance.state.as_ref().map(as_text),
        player_count: instance.player_count.flatten(),
        max_player_count: instance.max_player_count.flatten(),
        online_mode: None,
    }
}

async fn get_shared_list(State(shared): State<Shared>) -> Result<Json<PublicInstanceList>, Error> {
    let instance = shared_instance(&shared).await?;
    Ok(Json(PublicInstanceList {
        core_name: shared.state.global_settings.lock().await.core_name(),
        core_uuid: shared.state.uuid.clone(),
        listing_enabled: true,
        instances: vec![instance],
    }))
}

async fn get_shared_pack(
    State(shared): State<Shared>,
    UrlPath(uuid): UrlPath<InstanceUuid>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    if uuid != shared.uuid {
        return Err(not_found());
    }
    get_instance(&shared.state, &uuid)?;
    pack_response(&shared.state, &uuid, &headers).await
}

/// The path of the folder with the shared instances' identities, in `stores`.
pub fn connect_dir(stores: &Path) -> PathBuf {
    stores.join(CONNECT_DIR)
}

#[cfg(test)]
mod tests;
