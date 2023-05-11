#![allow(clippy::comparison_chain, clippy::type_complexity)]

use crate::event_broadcaster::EventBroadcaster;
use crate::migration::migrate;
use crate::traits::t_configurable::GameType;
use crate::{
    db::write::write_event_to_db_task,
    global_settings::GlobalSettingsData,
    handlers::{
        checks::get_checks_routes, core_info::get_core_info_routes, events::get_events_routes,
        gateway::get_gateway_routes, global_fs::get_global_fs_routes,
        global_settings::get_global_settings_routes, instance::*,
        instance_config::get_instance_config_routes, instance_fs::get_instance_fs_routes,
        instance_macro::get_instance_macro_routes, instance_players::get_instance_players_routes,
        instance_server::get_instance_server_routes,
        instance_setup_configs::get_instance_setup_config_routes, monitor::get_monitor_routes,
        setup::get_setup_route, system::get_system_routes, users::get_user_routes,
    },
    prelude::{
        LODESTONE_PATH, PATH_TO_BINARIES, PATH_TO_GLOBAL_SETTINGS, PATH_TO_STORES, PATH_TO_USERS,
    },
    util::rand_alphanumeric,
};

use auth::user::UsersManager;
use axum::Router;

use axum_server::tls_rustls::RustlsConfig;
use color_eyre::eyre::Context;
use error::Error;
use events::{CausedBy, Event};
use futures::Future;
use global_settings::GlobalSettings;
use implementations::{generic, minecraft};
use macro_executor::MacroExecutor;
use port_manager::PortManager;
use prelude::GameInstance;
use reqwest::{header, Method};
use ringbuffer::{AllocRingBuffer, RingBufferWrite};

use sqlx::{sqlite::SqliteConnectOptions, Pool};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use sysinfo::SystemExt;
use time::macros::format_description;
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
use tracing_subscriber::{
    fmt::time::LocalTime, prelude::__tracing_subscriber_SubscriberExt, EnvFilter,
};
use traits::{t_configurable::TConfigurable, t_server::MonitorReport, t_server::TServer};
use types::{DotLodestoneConfig, InstanceUuid};
use uuid::Uuid;
pub mod auth;
pub mod db;
mod deno_ops;
pub mod error;
mod event_broadcaster;
mod events;
pub mod global_settings;
mod handlers;
pub mod implementations;
pub mod macro_executor;
mod migration;
mod output_types;
mod port_manager;
pub mod prelude;
pub mod tauri_export;
mod traits;
pub mod types;
pub mod util;

#[derive(Clone)]
pub struct AppState {
    instances: Arc<Mutex<HashMap<InstanceUuid, GameInstance>>>,
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
    download_urls: Arc<Mutex<HashMap<String, PathBuf>>>,
    macro_executor: MacroExecutor,
    sqlite_pool: sqlx::SqlitePool,
}
async fn restore_instances(
    instances_path: &Path,
    event_broadcaster: EventBroadcaster,
    macro_executor: MacroExecutor,
) -> Result<HashMap<InstanceUuid, GameInstance>, Error> {
    let mut ret: HashMap<InstanceUuid, GameInstance> = HashMap::new();

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
        if let GameType::MinecraftJava = dot_lodestone_config.game_type() {
            let instance = minecraft::MinecraftInstance::restore(
                path.to_owned(),
                dot_lodestone_config.clone(),
                event_broadcaster.clone(),
                macro_executor.clone(),
            )
            .await
            .unwrap();
            debug!("Restored successfully");
            ret.insert(dot_lodestone_config.uuid().to_owned(), instance.into());
        }
    }
    Ok(ret)
}

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::hourly(
        LODESTONE_PATH.with(|v| v.join("log")),
        "lodestone_core.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

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
            .init();
    }

    _guard
}

pub async fn run() -> (
    impl Future<Output = ()>,
    AppState,
    tracing_appender::non_blocking::WorkerGuard,
) {
    let _ = color_eyre::install().map_err(|e| {
        error!("Failed to install color_eyre: {}", e);
    });
    let guard = setup_tracing();
    let lodestone_path = LODESTONE_PATH.with(|path| path.clone());
    let _ = migrate(&lodestone_path).map_err(|e| {
        error!("Error while migrating lodestone: {}. Lodestone will still start, but one or more instance may be in an erroneous state", e);
    });
    let path_to_instances = lodestone_path.join("instances");

    std::fs::create_dir_all(&lodestone_path)
        .and_then(|_| std::env::set_current_dir(&lodestone_path))
        .and_then(|_| std::fs::create_dir_all(PATH_TO_BINARIES.with(|path| path.clone())))
        .and_then(|_| std::fs::create_dir_all(PATH_TO_STORES.with(|path| path.clone())))
        .and_then(|_| std::fs::create_dir_all(&path_to_instances))
        .and_then(|_| std::fs::create_dir_all(lodestone_path.join("tmp")))
        .map_err(|e| {
            error!(
                "Failed to create lodestone path: {}. Lodestone will now crash...",
                e
            );
        })
        .unwrap();

    info!("Lodestone path: {}", lodestone_path.display());

    let (tx, _rx) = EventBroadcaster::new(512);

    let mut users_manager = UsersManager::new(
        tx.clone(),
        HashMap::new(),
        PATH_TO_USERS.with(|path| path.clone()),
    );

    users_manager.load_users().await.unwrap();

    let mut global_settings = GlobalSettings::new(
        PATH_TO_GLOBAL_SETTINGS.with(|path| path.clone()),
        tx.clone(),
        GlobalSettingsData::default(),
    );

    global_settings.load_from_file().await.unwrap();

    let first_time_setup_key = if !users_manager.as_ref().iter().any(|(_, user)| user.is_owner) {
        let key = rand_alphanumeric(16);
        // log the first time setup key in green so it's easy to find
        info!(
            "First time setup key: {}",
            ansi_term::Color::Green.paint(key.clone())
        );
        info!("This is a one-time, in-memory randomly generated key that allows you to create the owner account.");
        info!(
            "{}",
            ansi_term::Color::Red.paint("DO NOT SHARE THIS KEY WITH ANYONE!")
        );
        Some(key)
    } else {
        None
    };
    let macro_executor = MacroExecutor::new(tx.clone());
    let mut instances = restore_instances(&path_to_instances, tx.clone(), macro_executor.clone())
        .await
        .map_err(|e| {
            error!(
                "Failed to restore instances: {}, lodestone will now crash...",
                e
            );
        })
        .unwrap();
    for (_, instance) in instances.iter_mut() {
        if instance.auto_start().await {
            info!("Auto starting instance {}", instance.name().await);
            if let Err(e) = instance.start(CausedBy::System, false).await {
                error!(
                    "Failed to start instance {}: {:?}",
                    instance.name().await,
                    e
                );
            }
        }
    }
    let mut allocated_ports = HashSet::new();
    for (_, instance) in instances.iter() {
        allocated_ports.insert(instance.port().await);
    }
    let shared_state = AppState {
        instances: Arc::new(Mutex::new(instances)),
        users_manager: Arc::new(RwLock::new(users_manager)),
        events_buffer: Arc::new(Mutex::new(AllocRingBuffer::with_capacity(512))),
        console_out_buffer: Arc::new(Mutex::new(HashMap::new())),
        monitor_buffer: Arc::new(Mutex::new(HashMap::new())),
        event_broadcaster: tx.clone(),
        uuid: Uuid::new_v4().to_string(),
        up_since: chrono::Utc::now().timestamp(),
        port_manager: Arc::new(Mutex::new(PortManager::new(allocated_ports))),
        first_time_setup_key: Arc::new(Mutex::new(first_time_setup_key)),
        system: Arc::new(Mutex::new(sysinfo::System::new_all())),
        download_urls: Arc::new(Mutex::new(HashMap::new())),
        global_settings: Arc::new(Mutex::new(global_settings)),
        macro_executor,
        sqlite_pool: Pool::connect_with(
            SqliteConnectOptions::from_str(&format!(
                "sqlite://{}/data.db",
                PATH_TO_STORES.with(|p| p.clone()).display()
            ))
            .unwrap()
            .create_if_missing(true),
        )
        .await
        .unwrap(),
    };

    let event_buffer_task = {
        let event_buffer = shared_state.events_buffer.clone();
        let console_out_buffer = shared_state.console_out_buffer.clone();
        let mut event_receiver = tx.subscribe();
        async move {
            loop {
                let result = event_receiver.recv().await;
                if let Err(error) = result.as_ref() {
                    match error {
                        RecvError::Lagged(_) => {
                            warn!("Event buffer lagged");
                            continue;
                        }
                        RecvError::Closed => {
                            warn!("Event buffer closed");
                            break;
                        }
                    }
                }
                let event = result.unwrap();
                if event.is_event_console_message() {
                    console_out_buffer
                        .lock()
                        .await
                        .entry(event.get_instance_uuid().unwrap())
                        .or_insert_with(|| AllocRingBuffer::with_capacity(1024))
                        .push(event.clone());
                } else {
                    event_buffer.lock().await.push(event.clone());
                }
            }
        }
    };

    let write_to_db_task = write_event_to_db_task(tx.subscribe(), shared_state.sqlite_pool.clone());

    let monitor_report_task = {
        let monitor_buffer = shared_state.monitor_buffer.clone();
        let instances = shared_state.instances.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                for (uuid, instance) in instances.lock().await.iter() {
                    let report = instance.monitor().await;
                    monitor_buffer
                        .lock()
                        .await
                        .entry(uuid.to_owned())
                        .or_insert_with(|| AllocRingBuffer::with_capacity(64))
                        .push(report);
                }
                interval.tick().await;
            }
        }
    };

    let tls_config_result = RustlsConfig::from_pem_file(
        lodestone_path.join("tls").join("cert.pem"),
        lodestone_path.join("tls").join("key.pem"),
    )
    .await;

    (
        {
            let shared_state = shared_state.clone();
            async move {
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
                    .allow_origin(Any);

                let trace = TraceLayer::new_for_http();

                let api_routes = Router::new()
                    .merge(get_events_routes(shared_state.clone()))
                    .merge(get_instance_setup_config_routes(shared_state.clone()))
                    .merge(get_instance_server_routes(shared_state.clone()))
                    .merge(get_instance_config_routes(shared_state.clone()))
                    .merge(get_instance_players_routes(shared_state.clone()))
                    .merge(get_instance_routes(shared_state.clone()))
                    .merge(get_system_routes(shared_state.clone()))
                    .merge(get_checks_routes(shared_state.clone()))
                    .merge(get_user_routes(shared_state.clone()))
                    .merge(get_core_info_routes(shared_state.clone()))
                    .merge(get_setup_route(shared_state.clone()))
                    .merge(get_monitor_routes(shared_state.clone()))
                    .merge(get_instance_macro_routes(shared_state.clone()))
                    .merge(get_instance_fs_routes(shared_state.clone()))
                    .merge(get_global_fs_routes(shared_state.clone()))
                    .merge(get_global_settings_routes(shared_state.clone()))
                    .merge(get_gateway_routes(shared_state.clone()))
                    .layer(cors)
                    .layer(trace);
                let app = Router::new().nest("/api/v1", api_routes);
                #[allow(unused_variables, unused_mut)]
                let mut port = 16_662_u16;
                #[cfg(not(debug_assertions))]
                if port_scanner::scan_port(port) {
                    error!("Port {port} is already in use, exiting");
                    std::process::exit(1);
                }
                #[cfg(debug_assertions)]
                while port_scanner::scan_port(port) {
                    debug!("Port {port} is already in use, trying next port");
                    port += 1;
                }
                let addr = SocketAddr::from(([0, 0, 0, 0], port));
                let axum_server_handle = axum_server::Handle::new();
                tokio::spawn({
                    let axum_server_handle = axum_server_handle.clone();
                    async move {
                        info!("Lodestone Core live on {addr}");
                        info!("Note that Lodestone Core does not host the web dashboard itself. Please visit https://www.lodestone.cc for setup instructions.");
                        match tls_config_result {
                            Ok(config) => {
                                info!("TLS enabled");
                                axum_server::bind_rustls(addr, config)
                                    .handle(axum_server_handle)
                                    .serve(app.into_make_service())
                                    .await
                            }
                            Err(e) => {
                                warn!("Invalid TLS config : {e}, using HTTP");
                                axum_server::bind(addr)
                                    .handle(axum_server_handle)
                                    .serve(app.into_make_service())
                                    .await
                            }
                        }
                        .unwrap();
                    }
                });
                select! {
                    _ = write_to_db_task => info!("Write to db task exited"),
                    _ = event_buffer_task => info!("Event buffer task exited"),
                    _ = monitor_report_task => info!("Monitor report task exited"),
                    _ = tokio::signal::ctrl_c() => info!("Ctrl+C received"),
                }
                axum_server_handle.shutdown();
                // cleanup
                let mut instances = shared_state.instances.lock().await;
                for (_, instance) in instances.iter_mut() {
                    let _ = instance.stop(CausedBy::System, false).await;
                }
            }
        },
        shared_state,
        guard,
    )
}
