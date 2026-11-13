#![allow(clippy::comparison_chain, clippy::type_complexity)]

use crate::error::ErrorKind;
use crate::event_broadcaster::EventBroadcaster;
use crate::migration::migrate;
use crate::prelude::{
    init_app_state, lodestone_path, path_to_backups, path_to_global_settings, path_to_stores,
    path_to_tmp, path_to_users, try_init_paths, VERSION,
};
use crate::traits::t_configurable::GameType;
use crate::traits::t_server::State;
use crate::{
    db::write::write_event_to_db_task,
    global_settings::GlobalSettingsData,
    handlers::{
        api_keys::get_api_key_routes, checks::get_checks_routes, core_info::get_core_info_routes,
        events::get_events_routes, gateway::get_gateway_routes, global_fs::get_global_fs_routes,
        global_settings::get_global_settings_routes, instance::*,
        instance_config::get_instance_config_routes, instance_fs::get_instance_fs_routes,
        instance_import::get_instance_import_routes, instance_mods::get_instance_mods_routes,
        instance_players::get_instance_players_routes, instance_server::get_instance_server_routes,
        instance_setup_configs::get_instance_setup_config_routes, modpack::get_modpack_routes,
        modrinth::get_modrinth_routes, monitor::get_monitor_routes, setup::get_setup_route,
        system::get_system_routes, users::get_user_routes,
    },
    util::rand_alphanumeric,
};

use auth::user::UsersManager;
use axum::{
    http::{header, Method},
    Router,
};

use clap::Parser;
use color_eyre::eyre::Context;
use color_eyre::Report;
use dashmap::DashMap;
use error::Error;
use events::{CausedBy, Event};
use futures::Future;
use global_settings::GlobalSettings;
use implementations::minecraft;
use port_manager::PortManager;
use prelude::GameInstance;
use ringbuffer::{AllocRingBuffer, RingBuffer};

use semver::Version;
use std::sync::atomic::AtomicBool;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    select,
    sync::{broadcast::error::RecvError, Mutex, RwLock},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{debug, error, info, warn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter};
use traits::{t_configurable::TConfigurable, t_server::MonitorReport, t_server::TServer};
use types::{DotLodestoneConfig, InstanceUuid};

/// `#[test]`s that export the TypeScript bindings of types from `lodestone_minecraft` into
/// `bindings/`, like the tests `#[ts(export)]` adds for the core's own types.
#[cfg(test)]
macro_rules! export_shared_bindings {
    ($($test:ident => $ty:ty),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                let cfg = ts_rs::Config::from_env();
                <$ty as ts_rs::TS>::export_all(&cfg).expect("could not export type");
            }
        )*
    };
}
#[cfg(test)]
pub(crate) use export_shared_bindings;

pub mod auth;
pub mod backups;
pub mod capabilities;
mod command_console;
pub mod connect;
mod core_uuid;
pub mod db;
mod docker_bridge;
pub mod error;
mod event_broadcaster;
mod events;
pub mod global_settings;
mod handlers;
pub mod implementations;
mod listen;
pub mod local_claim;
mod migration;
mod minecraft_links;
mod modpack;
mod modrinth;
mod mods;
mod output_types;
mod persist;
mod port_manager;
pub mod prelude;
pub mod product;
mod public_listing;
pub mod remote_access;
mod shutdown;
pub mod tauri_export;
mod tls;
pub mod trace;
mod traits;
pub mod types;
pub mod util;
use handlers::global_fs::DownloadableFile;

#[cfg(test)]
pub(crate) mod test_util {
    /// Installs the same rustls crypto provider as [`crate::run`]. Call it at the start of
    /// any test that uses TLS, which panics without a process default.
    pub(crate) fn install_crypto_provider() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    /// Polls `condition` every few milliseconds until it holds, and panics if it still does
    /// not hold after 30 seconds. Use it instead of a fixed sleep to wait for work done by
    /// another task or thread.
    pub(crate) async fn wait_until<F, Fut>(what: &str, mut condition: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        while !condition().await {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting until {what}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// An [`AppState`](crate::AppState) whose files all live in `dir`, with no instances,
    /// no Docker daemon, a fake Modrinth and an in-memory database. Nothing in it touches
    /// the network or the global Lodestone paths. `settings` are the global settings to
    /// start with.
    pub(crate) async fn test_app_state(
        dir: &std::path::Path,
        settings: crate::global_settings::GlobalSettingsData,
    ) -> crate::AppState {
        use std::sync::Arc;
        use tokio::sync::{Mutex, RwLock};

        let (tx, _rx) = crate::event_broadcaster::EventBroadcaster::new(512);
        let mut users_manager = crate::auth::user::UsersManager::new(
            tx.clone(),
            Default::default(),
            dir.join("users.json"),
        );
        users_manager.load_users().await.unwrap();
        let global_settings = crate::global_settings::GlobalSettings::new(
            dir.join("global_settings.json"),
            tx.clone(),
            settings,
        );
        let code_execution = global_settings.code_execution_switch();
        let remote_access = global_settings.remote_access();
        crate::AppState {
            remote_access,
            // each test core its own store, so parallel tests never share one
            backups: Arc::new(crate::backups::BackupService::new(
                dir.join("backups"),
                dir.join("instances"),
            )),
            instances: Arc::new(dashmap::DashMap::new()),
            users_manager: Arc::new(RwLock::new(users_manager)),
            events_buffer: Arc::new(Mutex::new(ringbuffer::AllocRingBuffer::new(512))),
            console_out_buffer: Default::default(),
            monitor_buffer: Default::default(),
            event_broadcaster: tx.clone(),
            uuid: uuid::Uuid::new_v4().to_string(),
            up_since: chrono::Utc::now().timestamp(),
            port_manager: Arc::new(Mutex::new(crate::port_manager::PortManager::new(
                Default::default(),
            ))),
            first_time_setup_key: Default::default(),
            system: Arc::new(Mutex::new(sysinfo::System::new())),
            download_urls: Default::default(),
            safe_mode: global_settings.safe_mode_flag(),
            code_execution,
            login_limiter: Default::default(),
            tls_info: Arc::new(crate::tls::TlsInfo {
                mode: crate::tls::TlsMode::None,
                fingerprint_sha256: None,
            }),
            mods: Arc::new(crate::mods::ModsState::new(
                Arc::new(crate::modrinth::fake::FakeModrinth::default()),
                crate::modpack::store::PackStore::new(dir.join("modpacks")),
            )),
            public_listing: Arc::new(
                crate::public_listing::PublicListing::load(
                    dir.join("public_listing.json"),
                    dir.join("client-packs"),
                )
                .await,
            ),
            // never Minecraft's real services: tests that link accounts use a mock
            minecraft_links: Arc::new(
                crate::minecraft_links::MinecraftLinks::load(
                    dir.join("minecraft_links.json"),
                    "http://127.0.0.1:9/minecraft/profile",
                )
                .await,
            ),
            global_settings: Arc::new(Mutex::new(global_settings)),
            sqlite_pool: sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap(),
            docker_bridge: crate::docker_bridge::DockerBridge::without_docker(
                tx,
                dir.join("docker_bridge.json"),
            )
            .await,
            local_claims: Arc::new(crate::local_claim::LocalClaims::new(dir)),
            // an address nothing answers on: tests never reach connect.lodestone.cc
            connect: Arc::new(crate::connect::ConnectShares::new(
                dir.join("connect"),
                lodestone_connect::Service::parse("http://127.0.0.1:9").unwrap(),
                lodestone_connect::GatherOptions {
                    portmap: false,
                    loopback_only: true,
                },
            )),
            product: crate::product::CoreProduct::Core,
            launched_by: crate::product::LaunchedBy::Direct,
        }
    }

    /// Serves every `/api/v1` route of `state` on a free loopback port and returns the
    /// base URL, such as `http://127.0.0.1:1234/api/v1`. The server stops with the test's
    /// runtime.
    pub(crate) async fn serve_api(state: crate::AppState) -> String {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().nest("/api/v1", crate::api_router(state));
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
        });
        format!("http://{addr}/api/v1")
    }

    /// [`serve_api`], but every request looks as if it came from `peer`, such as another
    /// device on the network: the address is replaced before any of the API's layers.
    pub(crate) async fn serve_api_from(state: crate::AppState, peer: std::net::IpAddr) -> String {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .nest("/api/v1", crate::api_router(state))
            .layer(axum::middleware::from_fn(
                move |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
                    request
                        .extensions_mut()
                        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::new(
                            peer, 50000,
                        )));
                    next.run(request).await
                },
            ));
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await
        });
        format!("http://{addr}/api/v1")
    }
}

#[derive(Clone)]
pub struct AppState {
    instances: Arc<DashMap<InstanceUuid, GameInstance>>,
    users_manager: Arc<RwLock<UsersManager>>,
    events_buffer: Arc<Mutex<AllocRingBuffer<Event>>>,
    console_out_buffer: Arc<Mutex<HashMap<InstanceUuid, AllocRingBuffer<Event>>>>,
    monitor_buffer: Arc<Mutex<HashMap<InstanceUuid, AllocRingBuffer<MonitorReport>>>>,
    event_broadcaster: EventBroadcaster,
    uuid: String,
    up_since: i64,
    global_settings: Arc<Mutex<GlobalSettings>>,
    system: Arc<Mutex<sysinfo::System>>,
    port_manager: Arc<Mutex<PortManager>>,
    first_time_setup_key: Arc<Mutex<Option<String>>>,
    download_urls: Arc<Mutex<HashMap<String, DownloadableFile>>>,
    sqlite_pool: sqlx::SqlitePool,
    docker_bridge: docker_bridge::DockerBridge,
    /// The current safe mode setting, kept in sync by [`GlobalSettings`].
    safe_mode: Arc<AtomicBool>,
    /// The code execution kill switch, kept in sync by [`GlobalSettings`].
    code_execution: global_settings::CodeExecutionSwitch,
    /// Failed logins per username, see [`auth::login_limiter::LoginRateLimiter`].
    login_limiter: Arc<auth::login_limiter::LoginRateLimiter>,
    /// The web server's certificate, reported by `/info`. Fixed at startup.
    tls_info: Arc<tls::TlsInfo>,
    /// The Modrinth client, stored modpacks and mod change tracking.
    mods: Arc<mods::ModsState>,
    /// The instances listed for Lodestone Launcher, and the caches of the public routes.
    public_listing: Arc<public_listing::PublicListing>,
    /// Lodestone users' Minecraft accounts.
    minecraft_links: Arc<minecraft_links::MinecraftLinks>,
    /// Owner access claimed through the data folder, see [`local_claim`].
    local_claims: Arc<local_claim::LocalClaims>,
    /// The instances shared with friends through Lodestone Connect.
    connect: Arc<connect::ConnectShares>,
    /// Lodestone Host or the standalone core, reported in `/info`. Fixed at startup.
    product: product::CoreProduct,
    /// What started the core, reported in `/info`. Fixed at startup.
    launched_by: product::LaunchedBy,
    /// Whether other devices may use the core, kept in sync by [`GlobalSettings`]; see
    /// [`remote_access`].
    remote_access: Arc<remote_access::RemoteAccess>,
    /// Server backups: the store, its locks and the running backups and restores.
    backups: Arc<backups::BackupService>,
}

impl AppState {
    /// Whether safe mode is on. Unlike `global_settings.lock().await.safe_mode()`, this
    /// takes no lock.
    pub fn safe_mode(&self) -> bool {
        self.safe_mode.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Whether code execution (custom Java commands) is allowed. Takes no lock.
    pub fn code_execution_enabled(&self) -> bool {
        self.code_execution.is_enabled()
    }
}

/// Every `/api/v1` route.
fn api_router(shared_state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::ORIGIN, header::CONTENT_TYPE, header::AUTHORIZATION]) // Note I can't find X-Auth-Token but it was in the original rocket version, hope it's fine
        // sent with a 429 from the login route; a dashboard on another origin
        // can only read it if it is exposed
        .expose_headers([header::RETRY_AFTER])
        .allow_origin(Any);

    let trace = TraceLayer::new_for_http();

    Router::new()
        .merge(get_events_routes(shared_state.clone()))
        .merge(get_instance_setup_config_routes(shared_state.clone()))
        .merge(get_instance_server_routes(shared_state.clone()))
        .merge(get_instance_config_routes(shared_state.clone()))
        .merge(get_instance_players_routes(shared_state.clone()))
        .merge(get_instance_routes(shared_state.clone()))
        .merge(get_instance_import_routes(shared_state.clone()))
        .merge(get_system_routes(shared_state.clone()))
        .merge(get_checks_routes(shared_state.clone()))
        .merge(handlers::connect::get_connect_routes(shared_state.clone()))
        .merge(get_user_routes(shared_state.clone()))
        .merge(get_api_key_routes(shared_state.clone()))
        .merge(get_core_info_routes(shared_state.clone()))
        .merge(get_setup_route(shared_state.clone()))
        .merge(local_claim::get_local_claim_routes(shared_state.clone()))
        .merge(get_monitor_routes(shared_state.clone()))
        .merge(get_instance_fs_routes(shared_state.clone()))
        .merge(get_global_fs_routes(shared_state.clone()))
        .merge(get_global_settings_routes(shared_state.clone()))
        .merge(get_gateway_routes(shared_state.clone()))
        .merge(get_modrinth_routes(shared_state.clone()))
        .merge(get_instance_mods_routes(shared_state.clone()))
        .merge(get_modpack_routes(shared_state.clone()))
        .merge(handlers::server_content::get_server_content_routes(
            shared_state.clone(),
        ))
        .merge(handlers::public_instances::get_public_instance_routes(
            shared_state.clone(),
        ))
        .merge(handlers::launcher_live::get_launcher_live_routes(
            shared_state.clone(),
        ))
        .merge(handlers::minecraft_links::get_minecraft_link_routes(
            shared_state.clone(),
        ))
        .merge(handlers::trace::get_trace_routes(shared_state.clone()))
        .merge(handlers::backups::get_backup_routes(shared_state.clone()))
        .layer(axum::middleware::from_fn(crate::trace::http_middleware))
        // inside CORS, so a dashboard on another origin can read why it was refused (and
        // preflights are answered); before every route, WebSocket upgrades included
        .layer(axum::middleware::from_fn_with_state(
            shared_state.remote_access.clone(),
            remote_access::guard,
        ))
        .layer(cors)
        .layer(trace)
}

/// Binds the core's port on every interface: `port` exactly, or 16662, where a debug
/// build tries the next ports while one is taken.
fn bind_port(port: Option<u16>) -> Result<listen::Listeners, Error> {
    let exact = port.is_some() || cfg!(not(debug_assertions));
    let mut port = port.unwrap_or(16_662_u16);
    loop {
        match listen::bind_all_interfaces(port) {
            Ok(listeners) => return Ok(listeners),
            Err(e) if listen::port_taken(&e) && !exact && port < u16::MAX => {
                // before logging is set up, and only in debug builds
                eprintln!("Port {port} is already in use, trying next port");
                port += 1;
            }
            Err(e) if listen::port_taken(&e) => {
                let message = if e.kind() == std::io::ErrorKind::PermissionDenied {
                    // Windows: a port in a range the system reserved (Hyper-V and WSL
                    // reserve some), which no program can listen on
                    format!(
                        "Port {port} is reserved by Windows or held by another program \
                         (access denied: {e})"
                    )
                } else {
                    format!("Port {port} is already in use. Is another Lodestone Core running?")
                };
                return Err(Error {
                    kind: ErrorKind::Internal,
                    source: Report::msg(message),
                });
            }
            Err(e) => {
                return Err(Error {
                    kind: ErrorKind::Internal,
                    source: Report::msg(format!("Failed to listen on port {port}: {e}")),
                })
            }
        }
    }
}

/// Why the core was asked to stop, once it was: see [`StopRequests`].
type StopSignal = tokio::sync::watch::Receiver<Option<&'static str>>;

/// How long a forced quit (a second Ctrl+C or SIGTERM) waits for the servers to be
/// killed before the core exits anyway.
const FORCE_QUIT_KILL_TIMEOUT: Duration = Duration::from_secs(5);

/// Where a request to stop the core came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopSource {
    /// Ctrl+C, or SIGTERM on Unix.
    Signal,
    /// Stdin closed (`RunOptions::stop_with_stdin`): Lodestone Host quit.
    Stdin,
    /// The caller of [`run`] sent its shutdown signal (the Tauri app quitting).
    Caller,
    /// Part of the core ended on its own, such as the web server.
    Core,
}

/// What had already asked the core to stop when a request came.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EarlierStop {
    /// Nothing: this request starts the shutdown.
    Nothing,
    /// Something, but never a signal.
    NoSignal,
    /// A signal, among others maybe.
    Signal,
}

/// The requests to stop the core, wherever they come from (see [`StopSource`]). The first
/// one starts the shutdown, and from then on no server starts (see [`shutdown`]). A signal
/// after an earlier signal quits at once; a signal after only other requests (stdin closing
/// as Lodestone Host quits, then the session manager sending SIGTERM to every process at
/// logout, say) is taken as the first signal, and the shutdown carries on.
#[derive(Clone)]
struct StopRequests {
    reason: tokio::sync::watch::Sender<Option<&'static str>>,
    /// Whether a signal asked the core to stop.
    signalled: Arc<AtomicBool>,
    /// Marks the core as shutting down, on the first request: [`shutdown::begin`], except
    /// in tests, which must never mark their process.
    begin_shutdown: fn(),
    /// The servers a forced quit kills, once the core has loaded them.
    instances: Arc<std::sync::OnceLock<Arc<DashMap<InstanceUuid, GameInstance>>>>,
}

impl StopRequests {
    fn new() -> (Self, StopSignal) {
        Self::with_shutdown(shutdown::begin)
    }

    fn with_shutdown(begin_shutdown: fn()) -> (Self, StopSignal) {
        let (reason, signal) = tokio::sync::watch::channel(None);
        let requests = Self {
            reason,
            signalled: Arc::default(),
            begin_shutdown,
            instances: Arc::default(),
        };
        (requests, signal)
    }

    /// Records that the core was asked to stop, by what and why, and returns what had asked
    /// before. The first request marks the core as shutting down at once, so no server
    /// starts from then on, even while the core is still starting.
    fn request(&self, source: StopSource, reason: &'static str) -> EarlierStop {
        let earlier_signal = if source == StopSource::Signal {
            self.signalled
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        } else {
            self.signalled.load(std::sync::atomic::Ordering::SeqCst)
        };
        let first = self.reason.send_if_modified(|current| {
            if current.is_some() {
                return false;
            }
            *current = Some(reason);
            true
        });
        if first {
            (self.begin_shutdown)();
            EarlierStop::Nothing
        } else if earlier_signal {
            EarlierStop::Signal
        } else {
            EarlierStop::NoSignal
        }
    }

    /// The servers a forced quit kills.
    fn set_instances(&self, instances: Arc<DashMap<InstanceUuid, GameInstance>>) {
        let _ = self.instances.set(instances);
    }

    /// A signal arrived: the first request starts the shutdown, and the first signal after
    /// other requests lets it carry on. A signal after an earlier one (the user pressing
    /// Ctrl+C again while the core starts or stops its servers) kills the servers and
    /// exits with `exit_code` at once.
    async fn on_signal(&self, reason: &'static str, exit_code: i32) {
        match self.request(StopSource::Signal, reason) {
            EarlierStop::Nothing => return,
            EarlierStop::NoSignal => {
                info!("{reason} while already stopping: still stopping the servers");
                return;
            }
            EarlierStop::Signal => {}
        }
        // stderr too: the log may not be written before the exit
        eprintln!("{reason} while already stopping: killing the servers and quitting");
        warn!("{reason} while already stopping: killing the servers and quitting");
        if let Some(instances) = self.instances.get() {
            let all = [
                State::Starting,
                State::Running,
                State::Stopping,
                State::Error,
            ];
            let _ =
                tokio::time::timeout(FORCE_QUIT_KILL_TIMEOUT, kill_servers(instances, &all)).await;
        }
        std::process::exit(exit_code);
    }
}

/// Listens for Ctrl+C and, on Unix, SIGTERM (as `docker stop`, systemd and `kill` send),
/// so the core stops its servers before it exits, and returns where the requests go.
///
/// Called first thing in [`run_with`]: the handlers are installed before this returns,
/// so a stop asked for while the core is still starting is kept, instead of killing the
/// core on the spot (the systems' default) or going unnoticed. The core then starts no
/// server and shuts down as soon as it has started. The handlers stay installed: a second
/// signal (after an earlier signal, not after stdin closing or another kind of stop),
/// while the core starts or shuts down, kills the servers and exits at once (with 130
/// for Ctrl+C and 143 for SIGTERM, as shells report those signals).
fn listen_for_stop_signals() -> (StopRequests, StopSignal) {
    let (requests, signal) = StopRequests::new();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let listen = |kind: SignalKind, name: &str| {
            signal(kind)
                .map_err(|e| eprintln!("Cannot listen for {name}: {e}"))
                .ok()
        };
        let mut interrupt = listen(SignalKind::interrupt(), "Ctrl+C");
        let mut terminate = listen(SignalKind::terminate(), "SIGTERM");
        let requests = requests.clone();
        tokio::spawn(async move {
            // a signal that cannot be listened for never arrives
            async fn received(signal: &mut Option<tokio::signal::unix::Signal>) {
                match signal {
                    Some(signal) => {
                        // `None` once the runtime shuts down: never again
                        if signal.recv().await.is_none() {
                            std::future::pending::<()>().await;
                        }
                    }
                    None => std::future::pending().await,
                }
            }
            loop {
                let (reason, exit_code) = select! {
                    _ = received(&mut interrupt) => ("Ctrl+C received", 130),
                    _ = received(&mut terminate) => ("SIGTERM received", 143),
                };
                requests.on_signal(reason, exit_code).await;
            }
        });
    }
    #[cfg(windows)]
    {
        // Windows has no SIGTERM
        match tokio::signal::windows::ctrl_c() {
            Ok(mut ctrl_c) => {
                let requests = requests.clone();
                tokio::spawn(async move {
                    while ctrl_c.recv().await.is_some() {
                        requests.on_signal("Ctrl+C received", 130).await;
                    }
                });
            }
            Err(e) => eprintln!("Cannot listen for Ctrl+C: {e}"),
        }
    }
    (requests, signal)
}

/// Whether the core has been asked to stop.
fn stop_requested(stop_signal: &StopSignal) -> bool {
    stop_signal.borrow().is_some()
}

/// Kills the servers whose state is one of `states`, all at once, without asking them to
/// stop first.
async fn kill_servers(instances: &DashMap<InstanceUuid, GameInstance>, states: &[State]) {
    let kills = snapshot_instances(instances)
        .into_iter()
        .map(|(uuid, instance)| async move {
            let state = instance.state().await;
            if !states.contains(&state) {
                return;
            }
            warn!("Killing instance {uuid} ({state:?})");
            if let Err(e) = instance.kill(CausedBy::System).await {
                debug!("Killing instance {uuid} : {e}");
            }
        });
    futures::future::join_all(kills).await;
}

/// How long the core waits on shutdown for its servers to stop, all at once, before it
/// kills the ones still running. Stopping saves a server's world, which can take a while;
/// whatever stops the core must wait longer than this: Lodestone Host and the Tauri app
/// wait 60 s (`STOP_TIMEOUT` in lodestone-host's core_process.rs, `CORE_SHUTDOWN_TIMEOUT`
/// in dashboard/src-tauri), and core/docker-compose.yml gives Docker a minute.
const SHUTDOWN_STOP_TIMEOUT: Duration = Duration::from_secs(45);

/// Stops every server for the core's shutdown, all at once, and returns when they have
/// stopped. A running server is asked to stop, which saves its world, and is killed if it
/// has not stopped `timeout` from now; a server still starting is killed at once.
async fn stop_servers_for_shutdown(
    instances: &DashMap<InstanceUuid, GameInstance>,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    let stops = snapshot_instances(instances)
        .into_iter()
        // each on a task of its own, so a panic while stopping one cannot end the shutdown
        .map(|(uuid, instance)| tokio::spawn(stop_server_for_shutdown(uuid, instance, deadline)));
    futures::future::join_all(stops).await;
}

async fn stop_server_for_shutdown(
    uuid: InstanceUuid,
    instance: GameInstance,
    deadline: tokio::time::Instant,
) {
    let stopped = match instance.state().await {
        State::Stopped | State::Error => return,
        State::Starting => {
            info!("Killing instance that is starting : {uuid}");
            false
        }
        State::Running => {
            match tokio::time::timeout_at(deadline, instance.stop(CausedBy::System, true)).await {
                Ok(Ok(())) => true,
                Ok(Err(e)) => {
                    error!("Failed to stop instance {uuid} : {e}");
                    false
                }
                Err(_) => {
                    warn!("Instance {uuid} did not stop in time, killing it");
                    false
                }
            }
        }
        // asked to stop just before the shutdown
        State::Stopping => wait_until_stopped(&instance, deadline).await,
    };
    if !stopped && instance.state().await != State::Stopped {
        match instance.kill(CausedBy::System).await {
            // The process has exited; the state follows once its output task sees that.
            // Wait for it, so the sweep after the shutdown doesn't take the server for one
            // that started meanwhile.
            Ok(()) => {
                let soon = tokio::time::Instant::now() + Duration::from_secs(2);
                wait_until_stopped(&instance, soon).await;
            }
            Err(e) => {
                error!("Failed to kill instance {uuid} : {e}. Instance may need manual cleanup")
            }
        }
    }
}

/// Waits until `instance` has stopped, or until `deadline`. Returns whether it stopped.
async fn wait_until_stopped(instance: &GameInstance, deadline: tokio::time::Instant) -> bool {
    loop {
        if instance.state().await == State::Stopped {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// How often the API keys' last-used times are saved, at most.
const API_KEY_FLUSH_INTERVAL: Duration = Duration::from_secs(60);

/// Saves the API keys' last-used times once a minute, if any changed.
async fn flush_api_keys_periodically(users_manager: Arc<RwLock<UsersManager>>) {
    let mut interval = tokio::time::interval(API_KEY_FLUSH_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // the first tick completes at once
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(e) = users_manager.read().await.flush_api_keys().await {
            error!("Failed to save API key last-used times: {e}");
        }
    }
}

/// How long the monitor task waits for one instance's report.
const MONITOR_TIMEOUT: Duration = Duration::from_secs(5);
/// The console buffers are checked for deleted instances every this many monitor ticks.
const BUFFER_PRUNE_INTERVAL_TICKS: u64 = 60;
/// How many monitor reports (one a second) are kept per instance, for `/monitor`
/// streams to start with.
const MONITOR_HISTORY: usize = 64;

/// Whether some instance's server process may be running: the monitor has nothing to
/// report on otherwise.
async fn any_server_running(instances: &DashMap<InstanceUuid, GameInstance>) -> bool {
    for (_, instance) in snapshot_instances(instances) {
        if matches!(
            instance.state().await,
            State::Starting | State::Running | State::Stopping
        ) {
            return true;
        }
    }
    false
}

/// Removes console buffers that belong to no instance: deleted instances, or UUIDs a macro
/// made up. Docker containers are not in `instances`, so watched containers are kept.
async fn prune_console_buffers(
    console_out_buffer: &Mutex<HashMap<InstanceUuid, AllocRingBuffer<Event>>>,
    instances: &DashMap<InstanceUuid, GameInstance>,
    docker_bridge: &docker_bridge::DockerBridge,
) {
    let keys: Vec<InstanceUuid> = console_out_buffer.lock().await.keys().cloned().collect();
    let mut stale = Vec::new();
    for uuid in keys {
        if !instances.contains_key(&uuid) && !docker_bridge.is_watched(&uuid).await {
            stale.push(uuid);
        }
    }
    if !stale.is_empty() {
        let mut console_out_buffer = console_out_buffer.lock().await;
        for uuid in stale {
            console_out_buffer.remove(&uuid);
        }
    }
}

/// Clones every instance out of the map.
///
/// DashMap iterators hold a shard lock for as long as an entry is alive. Awaiting while
/// holding one blocks every `insert`/`remove` on that shard (on a runtime worker thread),
/// so callers that await on instances must work on this snapshot instead.
fn snapshot_instances(
    instances: &DashMap<InstanceUuid, GameInstance>,
) -> Vec<(InstanceUuid, GameInstance)> {
    instances
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect()
}

async fn restore_instances(
    instances_path: &Path,
    event_broadcaster: EventBroadcaster,
    code_execution: global_settings::CodeExecutionSwitch,
) -> Result<DashMap<InstanceUuid, GameInstance>, Error> {
    let ret: DashMap<InstanceUuid, GameInstance> = DashMap::new();

    for entry in instances_path
        .read_dir()
        .context("Failed to read instances directory")?
    {
        let path = match entry {
            Ok(v) => v.path(),
            Err(e) => {
                error!("Error while restoring instance, failed to read instance directory : {e}");
                continue;
            }
        };
        let dot_lodestone_config_file = match std::fs::File::open(path.join(".lodestone_config")) {
            Ok(v) => v,
            Err(e) => {
                error!("Error while restoring instance {}, failed to read .lodestone_config file : {e}", path.display());
                continue;
            }
        };
        let dot_lodestone_config: DotLodestoneConfig = match serde_json::from_reader(
            dot_lodestone_config_file,
        ) {
            Ok(v) => v,
            Err(e) => {
                error!("Error while restoring instance {}, failed to parse .lodestone_config file : {e}", path.display());
                continue;
            }
        };

        debug!("restoring instance: {}", path.display());
        let uuid_instance: (InstanceUuid, GameInstance) = match dot_lodestone_config.game_type() {
            GameType::MinecraftJava => {
                let instance = match minecraft::MinecraftInstance::restore(
                    path.to_owned(),
                    dot_lodestone_config.clone(),
                    event_broadcaster.clone(),
                    code_execution.clone(),
                )
                .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        error!(
                            "Error while restoring Minecraft Java instance {} : {e}",
                            path.display()
                        );
                        continue;
                    }
                };
                debug!("Restored Minecraft Java instance successfully");
                (dot_lodestone_config.uuid().to_owned(), instance.into())
            }
            // Atom instances ran TypeScript on Deno, which the core no longer has. Their
            // files are left on disk untouched.
            GameType::Generic => {
                warn!(
                    "Not restoring atom instance {}: Lodestone Core no longer supports atom instances. Its files are kept",
                    path.display()
                );
                continue;
            }
            GameType::MinecraftBedrock => {
                error!(
                    "Skipping instance {}: Minecraft Bedrock instances are not supported",
                    path.display()
                );
                continue;
            }
        };
        let uuid = uuid_instance.0;
        let instance = uuid_instance.1;
        if ret.contains_key(&uuid) {
            warn!("UUID {} is repeated.", uuid.to_string());
        }
        ret.insert(uuid, instance);
    }
    Ok(ret)
}

/// The most log lines waiting for the log file's writer thread.
const LOG_QUEUE_LINES: usize = 1024;

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender =
        tracing_appender::rolling::hourly(lodestone_path().join("log"), "lodestone_core.log");
    // The default queue holds 128,000 lines, and its channel allocates and touches every
    // slot up front: about 4 MB, more than the rest of an idle core. 1,024 lines cover
    // any normal burst; past that, lines are dropped rather than blocking the core.
    let (non_blocking, _guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(LOG_QUEUE_LINES)
        .thread_name("lodestone-log")
        .finish(file_appender);

    // set up a subscriber that logs formatted tracing events to stdout without colors without setting it as the default

    #[cfg(debug_assertions)]
    {
        let fmt_layer_stdout = tracing_subscriber::fmt::layer()
            // Use a more compact, abbreviated log format
            .compact()
            // Display source code file paths
            .with_file(true)
            // Display source code line numbers
            .with_line_number(true)
            // Display the thread ID an event was recorded on
            .with_thread_ids(false)
            // Don't display the event's target (module path)
            .with_target(true)
            .with_writer(std::io::stdout);
        let fmt_layer_file = tracing_subscriber::fmt::layer()
            // Use a more compact, abbreviated log format
            .compact()
            // Display source code file paths
            .with_file(true)
            // Display source code line numbers
            .with_line_number(true)
            // Display the thread ID an event was recorded on
            .with_thread_ids(false)
            // Don't display the event's target (module path)
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking);

        tracing_subscriber::registry()
            .with(fmt_layer_stdout)
            .with(fmt_layer_file)
            .with(crate::trace::TraceLayer.with_filter(crate::trace::layer_filter()))
            .with(EnvFilter::from("lodestone_core=debug"))
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        let fmt_layer_stdout = tracing_subscriber::fmt::layer()
            // Use a more compact, abbreviated log format
            .compact()
            // Display source code file paths
            .with_file(false)
            // Display source code line numbers
            .with_line_number(false)
            // Display the thread ID an event was recorded on
            .with_thread_ids(false)
            // Don't display the event's target (module path)
            .with_target(false)
            .with_writer(std::io::stdout)
            .with_filter(EnvFilter::from("lodestone_core=info"));

        let fmt_layer_file = tracing_subscriber::fmt::layer()
            // Use a more compact, abbreviated log format
            .compact()
            // Display source code file paths
            .with_file(true)
            // Display source code line numbers
            .with_line_number(true)
            // Display the thread ID an event was recorded on
            .with_thread_ids(false)
            // Don't display the event's target (module path)
            .with_target(true)
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_filter(EnvFilter::from("lodestone_core=debug"));

        tracing_subscriber::registry()
            // .with(ErrorLayer::default())
            .with(fmt_layer_stdout)
            .with(fmt_layer_file)
            .with(crate::trace::TraceLayer.with_filter(crate::trace::layer_filter()))
            .init();
    }

    _guard
}

/// What the core prints about the first-time setup key, which lets whoever has it
/// create the owner account.
///
/// It goes to the console only (`println!`), never through the logger, so the key is
/// not written to the log files. Under Lodestone Host (`is_host`) nothing is printed:
/// the Host makes the owner account through the local claim and doesn't need the key,
/// and it keeps the core's console output in a log file of its own.
fn setup_key_notice(key: &str, is_host: bool) -> Option<String> {
    (!is_host).then(|| {
        format!(
            "First time setup key: {key}\n\
             This is a one-time, in-memory randomly generated key that allows you to create the owner account.\n\
             DO NOT SHARE THIS KEY WITH ANYONE!"
        )
    })
}

fn output_sys_info() {
    info!("lodestone_core version {}", VERSION.with(|v| v.clone()));
    // output system info
    info!("System info:");
    info!("  OS: {}", std::env::consts::OS);
    info!("  Arch: {}", std::env::consts::ARCH);
    // CPU and RAM info
    let sys = crate::util::cpu_and_memory_system();
    let cpu_name = sys
        .cpus()
        .first()
        .map_or_else(|| "Unknown CPU", |v| v.brand());
    let ram = sys.total_memory();
    info!("  CPU: {cpu_name}");
    info!(
        "  RAM: {ram} bytes ({ram_gb} GB)",
        ram = ram,
        ram_gb = ram / 1024 / 1024 / 1024
    );
}

/// How long after starting the core looks for a newer version: never while it starts, so
/// the request (DNS, TLS and its certificate store) stays off the startup path.
const UPDATE_CHECK_DELAY: Duration = Duration::from_secs(10 * 60);

/// A release on GitHub, as listed by its API.
#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
}

/// The newest core version among `releases`. The repository also holds Lodestone
/// Launcher's and Lodestone Host's releases (`launcher-v1.0.0`, `host-v0.6.0`,
/// developer builds), so only `vX.Y.Z` tags are the core's. Pre-releases count only for
/// a core that is itself a pre-release.
fn newest_core_release(releases: &[GithubRelease], include_prereleases: bool) -> Option<Version> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| Version::parse(release.tag_name.strip_prefix('v')?).ok())
        .filter(|version| include_prereleases || version.pre.is_empty())
        .max()
}

/// Logs whether a newer core is released. Only logs: it never updates anything.
async fn check_for_core_update() {
    async fn get_releases() -> Result<Vec<GithubRelease>, Report> {
        // The newest 100 releases, published ones only (drafts need a token). Not
        // `releases/latest`, which may be a Launcher or Host release.
        let releases_url =
            "https://api.github.com/repos/Lodestone-Team/lodestone/releases?per_page=100";
        // the shared client sends Lodestone's user agent, which GitHub's API requires
        let response = crate::util::http_client()
            .get(releases_url)
            .timeout(Duration::from_secs(15))
            .send()
            .await?;
        response.error_for_status_ref()?;
        Ok(response.json().await?)
    }

    let current_version = VERSION.with(|v| v.clone());
    let releases = match get_releases().await {
        Ok(releases) => releases,
        Err(e) => {
            warn!("Failed to look for a newer Lodestone Core: {}", e);
            return;
        }
    };
    let Some(latest_version) = newest_core_release(&releases, !current_version.pre.is_empty())
    else {
        return;
    };

    if current_version >= latest_version {
        info!("lodestone_core is up to date");
        return;
    }

    if latest_version.pre.is_empty() {
        info!(
            "A new version of lodestone_core is available: {}",
            latest_version
        );
    } else {
        info!(
            "A new pre-release version of lodestone_core is available: {}",
            latest_version
        );
    }
    info!(
        "Read how to update here: {url}",
        url = "https://github.com/Lodestone-Team/lodestone/wiki/Updating"
    );
}

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "false")]
    pub is_cli: bool,
    /// Set by Lodestone Host, which runs the core inside its own process.
    #[arg(long, alias = "is-desktop", default_value = "false")]
    pub is_host: bool,
    #[arg(short, long)]
    pub lodestone_path: Option<PathBuf>,
    /// Turn off custom Java commands, and stop them from being turned on through the API. Setting the
    /// LODESTONE_DISABLE_CODE_EXECUTION environment variable to 1, true or yes does the same.
    #[arg(long, default_value = "false")]
    pub disable_code_execution: bool,
}

/// How a core run by another program is started, beyond [`Args`]. The core's own
/// binary sets these from its command line (`--port`, `--stop-with-stdin`); Lodestone
/// Host's GPUI app passes them when it runs the core as a child process.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Serve on exactly this port, and fail when it is taken. Without it the core uses
    /// 16662: a release build fails when it is taken, a debug build tries the next ports.
    pub port: Option<u16>,
    /// Shut down (stopping the servers, as on Ctrl+C) when stdin closes. A parent
    /// process that holds the core's stdin then stops it by closing the pipe, and the
    /// core also stops when the parent dies, on every system.
    pub stop_with_stdin: bool,
}

pub async fn run(
    args: Args,
) -> Result<
    (
        impl Future<Output = ()>,
        AppState,
        tracing_appender::non_blocking::WorkerGuard,
        tokio::sync::oneshot::Sender<()>,
    ),
    Error,
> {
    run_with(args, RunOptions::default()).await
}

/// [`run`], with [`RunOptions`].
pub async fn run_with(
    args: Args,
    options: RunOptions,
) -> Result<
    (
        impl Future<Output = ()>,
        AppState,
        tracing_appender::non_blocking::WorkerGuard,
        tokio::sync::oneshot::Sender<()>,
    ),
    Error,
> {
    // first, so a stop asked for while the core starts is not lost
    let (stop_requests, mut stop_signal) = listen_for_stop_signals();
    // Stdin closing asks the core to stop like a signal does: reading it from the start
    // notices that it closed (Lodestone Host quitting while the core starts, say) before
    // the core decides whether to start servers. Commands typed meanwhile wait for the
    // core's state.
    let console = command_console::start(options.stop_with_stdin.then(|| {
        let stop_requests = stop_requests.clone();
        Box::new(move || {
            stop_requests.request(StopSource::Stdin, "Stdin closed, shutting down");
        }) as Box<dyn FnOnce() + Send>
    }));
    // Before the slow part of starting, so that clients can connect at once: the system
    // queues their connections until the web server is ready to take them, instead of
    // refusing them. A port conflict is reported to the caller of `run` instead of exiting
    // the process from inside the library.
    let listeners = bind_port(options.port)?;
    // Crates in the dependency graph can enable different rustls crypto providers, so
    // rustls may not pick a process-wide default on its own and `ClientConfig::builder()` /
    // `ServerConfig::builder()` would panic (the TLS listener, HTTPS requests). Install
    // aws-lc-rs explicitly before anything touches TLS. Errors only if one is already set.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let product = product::CoreProduct::from_is_host(args.is_host);
    let in_docker = product::in_docker();
    let launched_by = product::LaunchedBy::from_flags(args.is_host, args.is_cli, in_docker);
    let _ = color_eyre::install().map_err(|e| {
        error!("Failed to install color_eyre: {}", e);
    });
    let lodestone_path = if let Some(path) = args.lodestone_path {
        path
    } else {
        PathBuf::from(match std::env::var("LODESTONE_PATH") {
            Ok(v) => v,
            Err(_) => {
                let base = match std::env::home_dir() {
                    Some(home) => home,
                    None => std::env::current_dir().map_err(|e| Error {
                        kind: ErrorKind::Internal,
                        source: Report::msg(format!(
                            "Failed to find a home or current directory: {e}"
                        )),
                    })?,
                };
                match base.join(".lodestone").to_str() {
                    Some(p) => p.to_string(),
                    None => {
                        return Err(Error {
                            kind: ErrorKind::Internal,
                            source: Report::msg("Failed to get home dir"),
                        })
                    }
                }
            }
        })
    };
    try_init_paths(lodestone_path.clone()).map_err(|e| Error {
        kind: ErrorKind::Internal,
        source: Report::msg(format!(
            "Failed to create the Lodestone directories in {}: {e}",
            lodestone_path.display()
        )),
    })?;
    info!("Lodestone path: {}", lodestone_path.display());
    std::env::set_current_dir(&lodestone_path).map_err(|_| Error {
        kind: ErrorKind::Internal,
        source: Report::msg("Failed to set current dir"),
    })?;
    let guard = setup_tracing();
    if args.is_host {
        info!("Lodestone Core running in Lodestone Host");
    }
    if !args.is_cli && !args.is_host && !in_docker {
        warn!("Lodestone Core is not meant to be run as a standalone program. Please use Lodestone CLI instead.");
        warn!("Download it here: https://github.com/Lodestone-Team/lodestone_cli")
    }
    // Lodestone Host updates itself and its core, and the Docker image is updated by
    // pulling it, so only a core run on its own looks for a newer version. It only logs,
    // so it waits until the core has long started.
    if !args.is_host && !in_docker {
        tokio::spawn(async {
            tokio::time::sleep(UPDATE_CHECK_DELAY).await;
            check_for_core_update().await;
        });
    }
    output_sys_info();

    let lockfile_path = lodestone_path.join("lodestone.lock");
    let lock_file = std::fs::File::create(lockfile_path.as_path()).map_err(|e| Error {
        kind: ErrorKind::Internal,
        source: Report::msg(format!(
            "Failed to create lockfile {}: {e}",
            lockfile_path.display()
        )),
    })?;
    if lock_file.try_lock().is_err() {
        return Err(Error {
            kind: ErrorKind::Internal,
            source: Report::msg("Another instance of lodestone is running"),
        });
    }

    let _ = migrate(&lodestone_path).map_err(|e| {
        error!("Error while migrating lodestone: {}. Lodestone will still start, but one or more instance may be in an erroneous state", e);
    });

    let path_to_instances = lodestone_path.join("instances");

    let (tx, _rx) = EventBroadcaster::new(512);

    let mut users_manager = UsersManager::new(tx.clone(), HashMap::new(), path_to_users().clone());

    users_manager.load_users().await?;

    let mut global_settings = GlobalSettings::new(
        path_to_global_settings().clone(),
        tx.clone(),
        GlobalSettingsData::default(),
    );

    // Read the lock before anything can run code, in particular before instances are
    // restored below.
    if global_settings::code_execution_lock_from_env(args.disable_code_execution) {
        global_settings.lock_code_execution();
    }
    global_settings.load_from_file().await?;
    let has_owner = users_manager.as_ref().values().any(|user| user.is_owner);
    if args.is_host && !has_owner && global_settings.allow_remote_access() {
        // Lodestone Host makes its owner without a password: until the owner sets one to
        // share the core, only apps on this computer may use it (`remote_access`)
        global_settings.set_allow_remote_access(false).await?;
        info!("New Lodestone Host core: only apps on this computer can use it until its owner shares it");
    }
    let remote_access = global_settings.remote_access();
    if !remote_access.allowed() {
        info!("Only apps on this computer can use this core (access from other devices is off)");
    }
    let code_execution = global_settings.code_execution_switch();
    if !code_execution.is_enabled() {
        warn!("Code execution is disabled: custom Java commands will not run");
    }

    // Before anything is served: the certificate decides how the web server listens.
    let tls_setup = tls::setup(&lodestone_path.join("tls")).await;
    let allow_lan_http = global_settings.allow_lan_http_flag();
    if global_settings.allow_lan_http() && tls_setup.config.is_some() {
        warn!(
            "Plain HTTP from other devices is allowed, so passwords and login tokens can cross your network unencrypted. \
             Turn off \"Allow LAN access over plain HTTP\" in Core Settings and connect with https:// instead"
        );
    }

    let first_time_setup_key = if !users_manager.as_ref().iter().any(|(_, user)| user.is_owner) {
        let key = rand_alphanumeric(16);
        info!("No owner account yet");
        // The key goes to the console only: the log files keep no secret.
        if let Some(notice) = setup_key_notice(&key, args.is_host) {
            println!("{notice}");
        }
        Some(key)
    } else {
        None
    };

    // Interrupted restores are undone before the servers are read, so each is read as it
    // was before the restore.
    let backups = Arc::new(backups::BackupService::new(
        path_to_backups(),
        path_to_instances.clone(),
    ));
    {
        let backups = backups.clone();
        if let Err(e) = tokio::task::spawn_blocking(move || backups.recover()).await {
            error!("Backup recovery failed: {e}");
        }
    }

    let instances = restore_instances(&path_to_instances, tx.clone(), code_execution.clone())
        .await
        .map_err(|_| Error {
            kind: ErrorKind::Internal,
            source: Report::msg("failed to restore instances"),
        })?;
    // servers whose restore couldn't be undone must not start (auto start included)
    backups.lock_broken_servers(&instances).await;

    let mut allocated_ports = HashSet::new();
    for (_, instance) in snapshot_instances(&instances) {
        allocated_ports.insert(instance.port().await);
    }
    // claims a previous run left are never finished
    let local_claims = local_claim::LocalClaims::new(&lodestone_path);
    local_claims.clean_up().await;
    let shared_state = AppState {
        backups: backups.clone(),
        remote_access: remote_access.clone(),
        local_claims: Arc::new(local_claims),
        connect: Arc::new(connect::ConnectShares::new(
            connect::connect_dir(path_to_stores()),
            connect::ConnectShares::service_from_env(),
            lodestone_connect::GatherOptions::default(),
        )),
        product,
        launched_by,
        instances: Arc::new(instances),
        users_manager: Arc::new(RwLock::new(users_manager)),
        events_buffer: Arc::new(Mutex::new(AllocRingBuffer::new(512))),
        console_out_buffer: Arc::new(Mutex::new(HashMap::new())),
        monitor_buffer: Arc::new(Mutex::new(HashMap::new())),
        event_broadcaster: tx.clone(),
        uuid: core_uuid::load_or_create(&path_to_stores().join("core_uuid")),
        up_since: chrono::Utc::now().timestamp(),
        port_manager: Arc::new(Mutex::new(PortManager::new(allocated_ports))),
        first_time_setup_key: Arc::new(Mutex::new(first_time_setup_key)),
        system: Arc::new(Mutex::new(crate::util::cpu_and_memory_system())),
        download_urls: Arc::new(Mutex::new(HashMap::new())),
        safe_mode: global_settings.safe_mode_flag(),
        code_execution,
        login_limiter: Arc::new(auth::login_limiter::LoginRateLimiter::default()),
        tls_info: Arc::new(tls_setup.info.clone()),
        mods: Arc::new(mods::ModsState::new(
            Arc::new(modrinth::ModrinthClient::new()),
            modpack::store::PackStore::new(path_to_tmp().join("modpacks")),
        )),
        public_listing: Arc::new(
            public_listing::PublicListing::load(
                path_to_stores().join("public_listing.json"),
                path_to_tmp().join("client-packs"),
            )
            .await,
        ),
        minecraft_links: Arc::new(
            minecraft_links::MinecraftLinks::load(
                path_to_stores().join("minecraft_links.json"),
                minecraft_links::MINECRAFT_PROFILE_URL,
            )
            .await,
        ),
        global_settings: Arc::new(Mutex::new(global_settings)),
        sqlite_pool: db::open_event_database(&path_to_stores().join("data.db"))
            .await
            .map_err(|_| Error {
                kind: ErrorKind::Internal,
                source: Report::msg("Failed to create sqlite pool"),
            })?,
        docker_bridge: docker_bridge::DockerBridge::new(
            tx.clone(),
            path_to_stores().join("docker_bridge.json"),
        )
        .await?,
    };

    // the core's events, for the owner's debug traces (local only)
    crate::trace::forward_events(shared_state.event_broadcaster.subscribe());

    tokio::spawn(flush_api_keys_periodically(
        shared_state.users_manager.clone(),
    ));

    // a second signal kills these
    stop_requests.set_instances(shared_state.instances.clone());

    // the commands typed so far run now
    console.ready(shared_state.clone());
    init_app_state(shared_state.clone());
    // mod changes made while an instance runs need a restart; this notices the restart
    tokio::spawn(shared_state.mods.restart.clone().watch(tx.subscribe()));
    // newer versions of servers' modpacks
    tokio::spawn(crate::modpack::updates::watch(shared_state.clone()));
    // scheduled backups; ends when the backup service is cancelled at shutdown
    tokio::spawn(backups::scheduler::run(shared_state.clone()));

    // Asked to stop while starting: start no server only to stop it again. The shutdown
    // below runs as soon as the caller runs the core.
    let stopping = stop_requested(&stop_signal);
    if stopping {
        info!("Asked to stop while starting: no server is started");
    }

    // instances shared with friends when the core last stopped
    if !stopping {
        let state = shared_state.clone();
        tokio::spawn(async move { state.connect.resume_all(&state).await });
    }

    for (_, instance) in snapshot_instances(&shared_state.instances) {
        // asked to stop meanwhile: start no more
        if stop_requested(&stop_signal) {
            break;
        }
        if instance.auto_start().await {
            let name = instance.name().await;
            info!("Auto starting instance {}", name);
            if let Err(e) = instance.start(CausedBy::System, false).await {
                error!("Failed to start instance {}: {:?}", name, e);
            }
        }
    }

    let event_buffer_task = {
        let event_buffer = shared_state.events_buffer.clone();
        let console_out_buffer = shared_state.console_out_buffer.clone();
        let mut event_receiver = tx.subscribe();
        async move {
            loop {
                let event = match event_receiver.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => {
                        warn!("Event buffer lagged");
                        continue;
                    }
                    Err(RecvError::Closed) => {
                        warn!("Event buffer closed");
                        break;
                    }
                };
                match event.get_instance_uuid() {
                    Some(instance_uuid) if event.is_event_console_message() => {
                        console_out_buffer
                            .lock()
                            .await
                            .entry(instance_uuid)
                            .or_insert_with(|| AllocRingBuffer::new(1024))
                            .enqueue(event);
                    }
                    _ => {
                        event_buffer.lock().await.enqueue(event);
                    }
                }
            }
        }
    };

    // Not part of the shutdown `select!` below: a database problem must never stop the
    // core. The task logs and retries errors itself.
    tokio::spawn(write_event_to_db_task(
        tx.subscribe(),
        shared_state.sqlite_pool.clone(),
    ));

    let monitor_report_task = {
        let monitor_buffer = shared_state.monitor_buffer.clone();
        let console_out_buffer = shared_state.console_out_buffer.clone();
        let instances = shared_state.instances.clone();
        let docker_bridge = shared_state.docker_bridge.clone();
        let mut events = tx.subscribe();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut ticks: u64 = 0;
            // Ticks since a server process last ran, up to `MONITOR_HISTORY`. By then the
            // history holds only empty reports, which clients show as no history.
            let mut idle_ticks = MONITOR_HISTORY;
            loop {
                if any_server_running(&instances).await {
                    idle_ticks = 0;
                } else if idle_ticks < MONITOR_HISTORY {
                    idle_ticks += 1;
                } else {
                    // Nothing to monitor: rather than wake every second, wait until a
                    // server starts, which sends events. A `/monitor` stream asks its
                    // instance for reports itself, so it needs no history.
                    monitor_buffer.lock().await.clear();
                    prune_console_buffers(&console_out_buffer, &instances, &docker_bridge).await;
                    // only events from now on, not the backlog from while servers ran
                    events = events.resubscribe();
                    while !any_server_running(&instances).await {
                        loop {
                            match events.recv().await {
                                // console lines (of watched Docker containers) start nothing
                                Ok(event) if event.is_event_console_message() => {}
                                Ok(_) | Err(RecvError::Lagged(_)) => break,
                                Err(RecvError::Closed) => std::future::pending::<()>().await,
                            }
                        }
                        // maybe an instance was deleted: its console lines go too
                        prune_console_buffers(&console_out_buffer, &instances, &docker_bridge)
                            .await;
                    }
                    continue;
                }
                interval.tick().await;
                let snapshot = snapshot_instances(&instances);
                // Query all instances at once, each with a timeout, so one instance that
                // does not answer cannot stall the others.
                let reports =
                    futures::future::join_all(snapshot.iter().map(|(uuid, instance)| async move {
                        match tokio::time::timeout(MONITOR_TIMEOUT, instance.monitor()).await {
                            Ok(report) => Some((uuid.clone(), report)),
                            Err(_) => {
                                debug!("Monitor report for {uuid} timed out");
                                None
                            }
                        }
                    }))
                    .await;
                {
                    let mut monitor_buffer = monitor_buffer.lock().await;
                    for (uuid, report) in reports.into_iter().flatten() {
                        monitor_buffer
                            .entry(uuid)
                            .or_insert_with(|| AllocRingBuffer::new(MONITOR_HISTORY))
                            .enqueue(report);
                    }
                    // drop the buffers of deleted instances
                    monitor_buffer.retain(|uuid, _| instances.contains_key(uuid));
                }
                if ticks.is_multiple_of(BUFFER_PRUNE_INTERVAL_TICKS) {
                    prune_console_buffers(&console_out_buffer, &instances, &docker_bridge).await;
                }
                ticks = ticks.wrapping_add(1);
            }
        }
    };

    let tls_dir = lodestone_path.join("tls");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let listen_description = listeners.describe();

    Ok((
        {
            let shared_state = shared_state.clone();
            async move {
                let api_routes = api_router(shared_state.clone());
                let app = Router::new()
                    // a page to open in a browser to trust the core's certificate
                    .route("/", axum::routing::get(tls::root_page))
                    .nest("/api/v1", api_routes);
                let axum_server_handles: Vec<_> = listeners
                    .listeners
                    .iter()
                    .map(|_| axum_server::Handle::<SocketAddr>::new())
                    .collect();
                let server_task = tokio::spawn({
                    let axum_server_handles = axum_server_handles.clone();
                    async move {
                        let https = tls_setup.config.is_some();
                        let acceptor_allows_lan_http = allow_lan_http.clone();
                        let acceptor = tls::DualProtocolAcceptor::new(
                            tls_setup.config,
                            allow_lan_http,
                            remote_access,
                        );
                        if https {
                            let plain_http = if acceptor_allows_lan_http
                                .load(std::sync::atomic::Ordering::SeqCst)
                            {
                                "from any device"
                            } else {
                                "from this machine only"
                            };
                            info!("Lodestone Core live on {listen_description}, over HTTPS, and plain HTTP {plain_http}");
                        } else {
                            warn!(
                                "HTTPS is not available. Lodestone Core is serving plain HTTP on all network interfaces, \
                                 so passwords and login tokens are sent unencrypted to anyone on your network. \
                                 To enable HTTPS, put cert.pem and key.pem in {}",
                                tls_dir.display()
                            );
                            info!("Lodestone Core live on {listen_description}, over plain HTTP");
                        }
                        info!("Note that Lodestone Core does not host the web dashboard itself. Please visit https://www.lodestone.cc/host for setup instructions.");
                        // one server per listener (two only where there are no dual-stack
                        // sockets), all behind the same acceptor, so the same TLS and
                        // LAN-HTTP rules apply whichever address family a client uses
                        futures::future::try_join_all(
                            listeners
                                .listeners
                                .into_iter()
                                .zip(axum_server_handles)
                                .map(|(listener, handle)| {
                                    let acceptor = acceptor.clone();
                                    let app = app.clone();
                                    async move {
                                        axum_server::from_tcp(listener)?
                                            .acceptor(acceptor)
                                            .handle(handle)
                                            .serve(
                                                app.into_make_service_with_connect_info::<
                                                    SocketAddr,
                                                >(),
                                            )
                                            .await
                                    }
                                }),
                        )
                        .await
                        .map(|_| ())
                    }
                });
                // capture file into the move block
                let _lock_file = lock_file;
                // what stopped the core, and where that came from unless it is already
                // recorded as a request to stop
                let (reason, source) = select! {
                    result = server_task => {
                        match result {
                            Ok(Ok(())) => info!("Web server stopped"),
                            Ok(Err(e)) => error!("Web server failed: {e}"),
                            Err(e) => error!("Web server task failed: {e}"),
                        }
                        ("Web server stopped", Some(StopSource::Core))
                    }
                    _ = event_buffer_task => ("Event buffer task exited", Some(StopSource::Core)),
                    _ = monitor_report_task => {
                        ("Monitor report task exited", Some(StopSource::Core))
                    }
                    _ = shutdown_rx => ("Shutdown signal received", Some(StopSource::Caller)),
                    // Ctrl+C, SIGTERM or stdin closing (only with `stop_with_stdin`), maybe
                    // while the core was starting; never fails, since `stop_requests`
                    // keeps the sender
                    Ok(reason) = stop_signal.wait_for(Option::is_some) => {
                        (reason.unwrap_or_default(), None)
                    }
                };
                info!("{reason}");
                // From here on no server starts (a crashed one restarting, say), and a
                // signal is a first signal, or a second one only after an earlier signal.
                if let Some(source) = source {
                    stop_requests.request(source, reason);
                }
                // the first request already marked it; this is only a backstop
                shutdown::begin();
                info!("Shutting down web server");
                for handle in &axum_server_handles {
                    handle.shutdown();
                }
                // Backups and restores are cancelled first and get a few seconds to stop
                // (saving is turned back on, staging folders are deleted), so the servers
                // they held stop normally below. A crash restart waiting for a cancelled
                // backup's lock is refused: the shutdown has begun.
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    shared_state.backups.shutdown(),
                )
                .await;
                shared_state.download_urls.lock().await.clear();
                info!("Signalling all instances to stop");
                tokio::join!(
                    stop_servers_for_shutdown(&shared_state.instances, SHUTDOWN_STOP_TIMEOUT),
                    // friends joined through Lodestone Connect hear that sharing stopped
                    async {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(5),
                            shared_state.connect.stop_all(),
                        )
                        .await;
                    },
                    async {
                        if let Err(e) = shared_state
                            .users_manager
                            .read()
                            .await
                            .flush_api_keys()
                            .await
                        {
                            error!("Failed to save API key last-used times: {e}");
                        }
                    },
                );
                // Servers refuse to start once the shutdown has begun, so this should find
                // none; but one that got past that would run on without the core: kill it.
                kill_servers(&shared_state.instances, &[State::Starting, State::Running]).await;
                // only now: servers may use temporary files until they stop
                let tmp = path_to_tmp();
                if tokio::fs::try_exists(tmp).await.unwrap_or(true) {
                    if let Err(e) = tokio::fs::remove_dir_all(tmp).await {
                        error!("Failed to remove tmp dir : {}", e);
                    }
                }
                shared_state.instances.clear();
                info!("Shutdown complete");
                // The caller decides how to exit the process once this future completes.
            }
        },
        shared_state,
        guard,
        shutdown_tx,
    ))
}

#[cfg(test)]
mod restore_tests {
    use std::{collections::BTreeMap, path::Path};

    use super::*;
    use crate::global_settings::CodeExecutionSwitch;
    use crate::implementations::minecraft::{Flavour, RestoreConfig};

    /// Every file under `dir` and its contents.
    fn snapshot_files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        walkdir::WalkDir::new(dir)
            .into_iter()
            .map(|entry| entry.unwrap())
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                let path = entry.path().strip_prefix(dir).unwrap().to_owned();
                (path, std::fs::read(entry.path()).unwrap())
            })
            .collect()
    }

    fn write_json(path: PathBuf, value: &impl serde::Serialize) {
        std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    /// Sets the global Lodestone paths, which restoring a Minecraft instance looks up, to a
    /// temporary folder shared by the tests here. It is never deleted, so the paths stay
    /// valid for the whole test run.
    fn init_test_paths() {
        static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let lodestone_dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
        try_init_paths(lodestone_dir.path().to_owned()).unwrap();
    }

    /// Writes a stopped Minecraft instance called `name` into `instances`.
    fn write_minecraft_instance(instances: &Path, name: &str) -> (PathBuf, InstanceUuid) {
        let server = instances.join(name);
        std::fs::create_dir_all(&server).unwrap();
        let uuid = InstanceUuid::default();
        write_json(
            server.join(".lodestone_config"),
            &DotLodestoneConfig::new(uuid.clone(), GameType::MinecraftJava),
        );
        write_json(
            server.join(".lodestone_minecraft_config.json"),
            &RestoreConfig {
                name: name.to_string(),
                version: "1.20.1".to_string(),
                flavour: Flavour::Vanilla,
                description: String::new(),
                cmd_args: Vec::new(),
                java_cmd: Some("java".to_string()),
                port: 25565,
                min_ram: 1024,
                max_ram: 2048,
                auto_start: false,
                restart_on_crash: false,
                backup_period: None,
                jre_major_version: 17,
                has_started: false,
                modpack: None,
            },
        );
        std::fs::write(server.join("server.properties"), "server-port=25565\n").unwrap();
        (server, uuid)
    }

    /// Instances written by an older core that had Deno: an atom instance, and a Minecraft
    /// instance with a macro and its stored config. The core must start on them: it
    /// restores the Minecraft instance, skips the atom, and leaves every file as it was.
    #[tokio::test]
    async fn atoms_and_macros_on_disk_do_not_stop_startup() {
        init_test_paths();

        let dir = tempfile::tempdir().unwrap();
        let instances = dir.path().join("instances");

        let atom = instances.join("my atom");
        std::fs::create_dir_all(&atom).unwrap();
        let atom_uuid = InstanceUuid::default();
        write_json(
            atom.join(".lodestone_config"),
            &DotLodestoneConfig::new(atom_uuid.clone(), GameType::Generic),
        );
        std::fs::write(atom.join("run.ts"), "import * as a from \"./main.ts\";\n").unwrap();

        let (server, server_uuid) = write_minecraft_instance(&instances, "my server");
        std::fs::create_dir_all(server.join("macros").join("greet")).unwrap();
        std::fs::write(
            server.join("macros").join("greet").join("index.ts"),
            "class LodestoneConfig { greeting: string = \"hi\"; }\n",
        )
        .unwrap();
        std::fs::write(
            server
                .join("macros")
                .join("greet")
                .join("greet_config.json"),
            r#"{"greeting":{"value":{"type":"String","value":"hello"}}}"#,
        )
        .unwrap();
        let before = snapshot_files(&instances);

        let (tx, _rx) = EventBroadcaster::new(16);
        let restored = restore_instances(&instances, tx, CodeExecutionSwitch::new(false))
            .await
            .unwrap();

        assert_eq!(restored.len(), 1);
        let server_instance = restored.get(&server_uuid).unwrap().value().clone();
        assert_eq!(server_instance.name().await, "my server");
        assert!(!restored.contains_key(&atom_uuid));
        assert_eq!(snapshot_files(&instances), before);
    }

    /// The shutdown leaves servers that are not running alone, and returns at once.
    #[tokio::test]
    async fn the_shutdown_leaves_stopped_servers_alone() {
        init_test_paths();
        let dir = tempfile::tempdir().unwrap();
        let instances = dir.path().join("instances");
        let (_, uuid) = write_minecraft_instance(&instances, "stopped server");
        let (tx, mut events) = EventBroadcaster::new(16);
        let restored = restore_instances(&instances, tx, CodeExecutionSwitch::new(false))
            .await
            .unwrap();
        assert_eq!(restored.len(), 1);

        tokio::time::timeout(
            Duration::from_secs(10),
            stop_servers_for_shutdown(&restored, Duration::from_secs(45)),
        )
        .await
        .expect("stopping no running server takes no time");
        let instance = restored.get(&uuid).unwrap().value().clone();
        assert_eq!(instance.state().await, State::Stopped);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    /// The shutdown kills a server still starting, and the sweep after it kills one that
    /// started meanwhile, leaving stopped ones alone.
    #[tokio::test]
    async fn the_shutdown_kills_servers_that_are_starting() {
        init_test_paths();
        let dir = tempfile::tempdir().unwrap();
        let instances = dir.path().join("instances");
        let (_, starting) = write_minecraft_instance(&instances, "starting server");
        let (_, late) = write_minecraft_instance(&instances, "late server");
        let (_, stopped) = write_minecraft_instance(&instances, "stopped server");
        let (tx, _events) = EventBroadcaster::new(64);
        let restored = restore_instances(&instances, tx, CodeExecutionSwitch::new(false))
            .await
            .unwrap();
        let instance = |uuid: &InstanceUuid| restored.get(uuid).unwrap().value().clone();
        // no process runs: the state alone says starting
        let set_state = |uuid: &InstanceUuid, state: State| {
            let GameInstance::MinecraftInstance(minecraft) = instance(uuid);
            async move { minecraft.set_state_for_test(state).await }
        };
        set_state(&starting, State::Starting).await;

        tokio::time::timeout(
            Duration::from_secs(10),
            stop_servers_for_shutdown(&restored, Duration::from_secs(45)),
        )
        .await
        .expect("a starting server is killed at once");
        assert_eq!(instance(&starting).state().await, State::Stopped);

        set_state(&late, State::Running).await;
        tokio::time::timeout(
            Duration::from_secs(10),
            kill_servers(&restored, &[State::Starting, State::Running]),
        )
        .await
        .unwrap();
        assert_eq!(instance(&late).state().await, State::Stopped);
        assert_eq!(instance(&stopped).state().await, State::Stopped);
    }
}

#[cfg(test)]
mod stop_request_tests {
    use super::*;

    // The process's shutdown flag is never set in tests: other tests would then fail to
    // start servers.
    fn requests() -> (StopRequests, StopSignal) {
        StopRequests::with_shutdown(|| {})
    }

    /// Whatever asks first starts the shutdown, with its reason; a request after it
    /// changes nothing.
    #[tokio::test]
    async fn the_first_request_to_stop_counts() {
        let (requests, mut signal) = requests();
        assert!(!stop_requested(&signal));
        assert_eq!(
            requests.request(StopSource::Stdin, "Stdin closed, shutting down"),
            EarlierStop::Nothing
        );
        assert!(stop_requested(&signal));

        assert_ne!(
            requests.request(StopSource::Signal, "Ctrl+C received"),
            EarlierStop::Nothing,
            "a second request"
        );
        assert_ne!(
            requests
                .clone()
                .request(StopSource::Signal, "SIGTERM received"),
            EarlierStop::Nothing
        );
        // the shutdown, waiting only now, still sees the first request and its reason
        let reason = tokio::time::timeout(Duration::from_secs(5), signal.wait_for(Option::is_some))
            .await
            .unwrap()
            .map(|reason| *reason)
            .unwrap();
        assert_eq!(reason, Some("Stdin closed, shutting down"));
    }

    /// Only a signal after an earlier signal is a second signal (which force-quits): a
    /// signal after stdin closed, the caller asked or the web server ended is the first.
    #[tokio::test]
    async fn only_a_signal_after_a_signal_is_a_second_signal() {
        for first in [StopSource::Stdin, StopSource::Caller, StopSource::Core] {
            let (requests, _signal) = requests();
            assert_eq!(requests.request(first, "stop"), EarlierStop::Nothing);
            assert_eq!(
                requests.request(first, "stop again"),
                EarlierStop::NoSignal,
                "{first:?} twice"
            );
            assert_eq!(
                requests.request(StopSource::Signal, "SIGTERM received"),
                EarlierStop::NoSignal,
                "a first signal after {first:?}"
            );
            assert_eq!(
                requests.request(StopSource::Caller, "Shutdown signal received"),
                EarlierStop::Signal
            );
            assert_eq!(
                requests.request(StopSource::Signal, "Ctrl+C received"),
                EarlierStop::Signal,
                "a second signal after {first:?}"
            );
        }

        let (requests, _signal) = requests();
        assert_eq!(
            requests.request(StopSource::Signal, "Ctrl+C received"),
            EarlierStop::Nothing
        );
        assert_eq!(
            requests.request(StopSource::Signal, "Ctrl+C received"),
            EarlierStop::Signal
        );
    }

    /// The first request marks the core as shutting down at once, so no server starts
    /// even while the core is still starting; later ones leave it marked.
    #[test]
    fn the_first_request_marks_the_shutdown() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static MARKED: AtomicUsize = AtomicUsize::new(0);
        fn mark() {
            MARKED.fetch_add(1, Ordering::SeqCst);
        }
        let (requests, _signal) = StopRequests::with_shutdown(mark);
        assert_eq!(MARKED.load(Ordering::SeqCst), 0);
        requests.request(StopSource::Stdin, "Stdin closed, shutting down");
        assert_eq!(MARKED.load(Ordering::SeqCst), 1);
        requests.request(StopSource::Signal, "SIGTERM received");
        assert_eq!(MARKED.load(Ordering::SeqCst), 1);
    }
}

#[cfg(test)]
mod update_check_tests {
    use super::{newest_core_release, GithubRelease};
    use semver::Version;

    fn release(tag_name: &str, draft: bool) -> GithubRelease {
        GithubRelease {
            tag_name: tag_name.to_string(),
            draft,
        }
    }

    #[test]
    fn only_the_cores_own_releases_count() {
        // as the Lodestone repository lists them, newest first
        let releases = [
            release("launcher-v1.2.0", false),
            release("host-v0.9.0", false),
            release("host-dev-3989-c716273", false),
            release("launcher-latest", false),
            release("v0.7.0", true),
            release("v0.6.0-beta.2", false),
            release("rc", false),
            release("v0.5.1", false),
            release("v0.5.0", false),
            release("v0.0.1-alpha", false),
        ];
        assert_eq!(
            newest_core_release(&releases, false),
            Some(Version::new(0, 5, 1))
        );
        assert_eq!(
            newest_core_release(&releases, true),
            Some(Version::parse("0.6.0-beta.2").unwrap())
        );
        assert_eq!(
            newest_core_release(&[release("launcher-v1.0.0", false)], true),
            None
        );
    }
}

#[cfg(test)]
mod setup_key_tests {
    use super::setup_key_notice;

    #[test]
    fn the_setup_key_goes_to_the_console_but_never_under_the_host() {
        let notice = setup_key_notice("abc123", false).expect("a standalone core shows it");
        assert!(notice.contains("First time setup key: abc123"));
        assert_eq!(setup_key_notice("abc123", true), None);
    }
}
