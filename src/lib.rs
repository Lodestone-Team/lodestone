#![allow(clippy::comparison_chain, clippy::type_complexity)]

use crate::{
    handlers::{
        checks::get_checks_routes, core_info::get_core_info_routes, events::get_events_routes,
        global_fs::get_global_fs_routes, global_settings::get_global_settings_routes, instance::*,
        instance_config::get_instance_config_routes, instance_fs::get_instance_fs_routes,
        instance_macro::get_instance_macro_routes, instance_manifest::get_instance_manifest_routes,
        instance_players::get_instance_players_routes, instance_server::get_instance_server_routes,
        instance_setup_configs::get_instance_setup_config_routes, monitor::get_monitor_routes,
        setup::get_setup_route, system::get_system_routes, users::get_user_routes,
    },
    prelude::{LODESTONE_PATH, PATH_TO_BINARIES, PATH_TO_STORES, PATH_TO_USERS},
    util::{download_file, rand_alphanumeric},
};
use auth::user::{User, UsersManager};
use axum::{Extension, Router};
use events::{CausedBy, Event};
use implementations::minecraft;
use log::{debug, error, info, warn};
use port_allocator::PortAllocator;
use prelude::GameInstance;
use reqwest::{header, Method};
use ringbuffer::{AllocRingBuffer, RingBufferWrite};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use sysinfo::SystemExt;
use tokio::{
    fs::create_dir_all,
    io::AsyncWriteExt,
    process::Command,
    select,
    sync::{
        broadcast::{self, error::RecvError, Receiver, Sender},
        Mutex, RwLock,
    },
    task::JoinHandle,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
pub use traits::Error;
use traits::{
    t_configurable::TConfigurable, t_server::MonitorReport, t_server::TServer, ErrorInner,
};
use ts_rs::TS;
use util::list_dir;
use uuid::Uuid;
pub mod auth;
mod events;
mod handlers;
mod implementations;
pub mod macro_executor;
mod output_types;
mod port_allocator;
pub mod prelude;
pub mod tauri_export;
mod traits;
pub mod types;
mod util;

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct GlobalSettings {
    #[serde(skip)]
    path_to_global_settings: PathBuf,
    core_name: String,
    safe_mode: bool,
}

impl GlobalSettings {
    pub async fn new() -> Self {
        let path_to_global_settings = PATH_TO_STORES.with(|v| v.join("global_settings.json"));
        if path_to_global_settings.exists() {
            if let Ok(v) =
                serde_json::from_reader(std::fs::File::open(&path_to_global_settings).unwrap())
            {
                return v;
            }
        }
        let ret = Self {
            path_to_global_settings,
            core_name: format!("{}'s Lodestone Core", whoami::realname()),
            safe_mode: true,
        };
        ret.save().await.unwrap();
        ret
    }
    async fn save(&self) -> Result<(), Error> {
        let mut file = tokio::fs::File::create(&self.path_to_global_settings)
            .await
            .map_err(|_| Error {
                inner: ErrorInner::FailedToCreateFileOrDir,
                detail: "Failed to create global settings file".to_string(),
            })?;
        file.write_all(serde_json::to_string_pretty(self).unwrap().as_bytes())
            .await
            .map_err(|_| Error {
                inner: ErrorInner::FailedToWriteFileOrDir,
                detail: "Failed to write to global settings file".to_string(),
            })?;
        Ok(())
    }
    pub async fn set_core_name(&mut self, name: String) -> Result<(), Error> {
        self.core_name = name;
        self.save().await
    }

    pub fn core_name(&self) -> String {
        self.core_name.clone()
    }

    pub async fn set_safe_mode(&mut self, safe_mode: bool) -> Result<(), Error> {
        self.safe_mode = safe_mode;
        self.save().await
    }

    pub fn safe_mode(&self) -> bool {
        self.safe_mode
    }
}

#[derive(Clone)]
pub struct AppState {
    instances: Arc<Mutex<HashMap<String, GameInstance>>>,
    users_manager: Arc<RwLock<UsersManager>>,
    events_buffer: Arc<Mutex<AllocRingBuffer<Event>>>,
    console_out_buffer: Arc<Mutex<HashMap<String, AllocRingBuffer<Event>>>>,
    monitor_buffer: Arc<Mutex<HashMap<String, AllocRingBuffer<MonitorReport>>>>,
    event_broadcaster: Sender<Event>,
    uuid: String,
    up_since: i64,
    global_settings: Arc<Mutex<GlobalSettings>>,
    system: Arc<Mutex<sysinfo::System>>,
    port_allocator: Arc<Mutex<PortAllocator>>,
    first_time_setup_key: Arc<Mutex<Option<String>>>,
    download_urls: Arc<Mutex<HashMap<String, PathBuf>>>,
}

async fn restore_instances(
    lodestone_path: &Path,
    event_broadcaster: &Sender<Event>,
) -> HashMap<String, GameInstance> {
    let mut ret: HashMap<String, GameInstance> = HashMap::new();

    for instance_future in list_dir(&lodestone_path.join("instances"), Some(true))
        .await
        .unwrap()
        .iter()
        .filter(|path| {
            debug!("{}", path.display());
            path.join(".lodestone_config").is_file()
        })
        .map(|path| {
            // read config as json
            let config: Value = serde_json::from_reader(
                std::fs::File::open(path.join(".lodestone_config")).unwrap(),
            )
            .unwrap();
            config
        })
        .map(|config| {
            match config["game_type"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .as_str()
            {
                "minecraft" => {
                    debug!(
                        "Restoring Minecraft instance {}",
                        config["name"].as_str().unwrap()
                    );
                    minecraft::MinecraftInstance::restore(
                        serde_json::from_value(config).unwrap(),
                        event_broadcaster.clone(),
                    )
                }
                _ => unimplemented!(),
            }
        })
    {
        let instance = instance_future.await;
        ret.insert(instance.uuid().await.to_string(), instance.into());
    }
    ret
}

async fn restore_users() -> HashMap<String, User> {
    let path_to_user_json = PATH_TO_USERS.with(|v| v.clone());
    // create user file if it doesn't exist
    if tokio::fs::OpenOptions::new()
        .read(true)
        .create(true)
        .write(true)
        .open(&path_to_user_json)
        .await
        .unwrap()
        .metadata()
        .await
        .unwrap()
        .len()
        == 0
    {
        return HashMap::new();
    }
    let users: HashMap<String, User> = serde_json::from_reader(
        tokio::fs::File::open(path_to_user_json)
            .await
            .unwrap()
            .into_std()
            .await,
    )
    .unwrap();
    users
}

async fn download_dependencies() -> Result<(), Error> {
    let arch = if std::env::consts::ARCH == "x86_64" {
        "x64"
    } else {
        std::env::consts::ARCH
    };

    let os = std::env::consts::OS;
    let _7zip_name = format!("7z_{}_{}", os, arch);
    let path_to_7z = PATH_TO_BINARIES.with(|v| v.join("7zip"));
    // check if 7z is already downloaded
    if !path_to_7z.join(&_7zip_name).exists() {
        info!("Downloading 7z");
        let _7z = download_file(
            format!(
                "https://github.com/Lodestone-Team/dependencies/raw/main/7z_{}_{}",
                os, arch
            )
            .as_str(),
            path_to_7z.as_ref(),
            Some(_7zip_name.as_str()),
            &|_| {},
            false,
        )
        .await?;
    } else {
        info!("7z already downloaded");
    }
    if os != "windows" {
        Command::new("chmod")
            .arg("+x")
            .arg(path_to_7z.join(&_7zip_name))
            .output()
            .await
            .unwrap();
    }
    Ok(())
}

pub async fn run() -> (JoinHandle<()>, AppState) {
    env_logger::builder()
        .filter(Some("lodestone_client"), log::LevelFilter::Debug)
        .format_module_path(false)
        .format_target(false)
        .init();
    let lodestone_path = LODESTONE_PATH.with(|path| path.clone());
    create_dir_all(&lodestone_path).await.unwrap();
    std::env::set_current_dir(&lodestone_path).expect("Failed to set current dir");

    create_dir_all(PATH_TO_BINARIES.with(|path| path.clone()))
        .await
        .unwrap();

    create_dir_all(PATH_TO_STORES.with(|path| path.clone()))
        .await
        .unwrap();

    let web_path = lodestone_path.join("web");
    let path_to_intances = lodestone_path.join("instances");
    create_dir_all(&web_path).await.unwrap();
    create_dir_all(&path_to_intances).await.unwrap();
    info!("Lodestone path: {}", lodestone_path.display());

    download_dependencies().await.unwrap();

    let (tx, _rx): (Sender<Event>, Receiver<Event>) = broadcast::channel(256);

    let users = restore_users().await;

    let first_time_setup_key = if !users.iter().any(|(_, user)| user.is_owner) {
        let key = rand_alphanumeric(16);
        // log the first time setup key in green so it's easy to find
        info!("\x1b[32mFirst time setup key: {}\x1b[0m", key);
        Some(key)
    } else {
        None
    };
    let mut instances = restore_instances(&lodestone_path, &tx).await;
    for (_, instance) in instances.iter_mut() {
        if instance.auto_start().await {
            info!("Auto starting instance {}", instance.name().await);
            if let Err(e) = instance.start(CausedBy::System).await {
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
        users_manager: Arc::new(RwLock::new(UsersManager::new(tx.clone(), users))),
        events_buffer: Arc::new(Mutex::new(AllocRingBuffer::with_capacity(512))),
        console_out_buffer: Arc::new(Mutex::new(HashMap::new())),
        monitor_buffer: Arc::new(Mutex::new(HashMap::new())),
        event_broadcaster: tx.clone(),
        uuid: Uuid::new_v4().to_string(),
        up_since: chrono::Utc::now().timestamp(),
        port_allocator: Arc::new(Mutex::new(PortAllocator::new(allocated_ports))),
        first_time_setup_key: Arc::new(Mutex::new(first_time_setup_key)),
        system: Arc::new(Mutex::new(sysinfo::System::new_all())),
        download_urls: Arc::new(Mutex::new(HashMap::new())),
        global_settings: Arc::new(Mutex::new(GlobalSettings::new().await)),
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
                        .or_insert_with(|| AllocRingBuffer::with_capacity(512))
                        .push(event.clone());
                } else {
                    event_buffer.lock().await.push(event.clone());
                }
            }
        }
    };

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
    (
        tokio::spawn({
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
                    .merge(get_events_routes())
                    .merge(get_instance_setup_config_routes())
                    .merge(get_instance_manifest_routes())
                    .merge(get_instance_server_routes())
                    .merge(get_instance_config_routes())
                    .merge(get_instance_players_routes())
                    .merge(get_instance_routes())
                    .merge(get_system_routes())
                    .merge(get_checks_routes())
                    .merge(get_user_routes())
                    .merge(get_core_info_routes())
                    .merge(get_setup_route())
                    .merge(get_monitor_routes())
                    .merge(get_instance_macro_routes())
                    .merge(get_instance_fs_routes())
                    .merge(get_global_fs_routes())
                    .merge(get_global_settings_routes())
                    .layer(Extension(shared_state.clone()))
                    .layer(cors)
                    .layer(trace);
                let app = Router::new().nest("/api/v1", api_routes);
                let addr = SocketAddr::from(([0, 0, 0, 0], 16_662));
                select! {
                    _ = event_buffer_task => info!("Event buffer task exited"),
                    _ = monitor_report_task => info!("Monitor report task exited"),
                    _ = axum::Server::bind(&addr)
                    .serve(app.into_make_service()) => info!("Server exited"),
                    _ = tokio::signal::ctrl_c() => info!("Ctrl+C received"),
                }
                // cleanup
                let mut instances = shared_state.instances.lock().await;
                for (_, instance) in instances.iter_mut() {
                    let _ = instance.stop(CausedBy::System).await;
                }
            }
        }),
        shared_state,
    )
}
