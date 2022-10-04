pub mod configurable;
pub mod r#macro;
pub mod player;
pub mod resource;
pub mod server;
mod util;
pub mod manifest;
pub mod versions;

use std::collections::{HashSet, HashMap};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::Mutex;


use ::serde::{Deserialize, Serialize};
use log::{debug, error, info, warn};
use serde_json::{to_string_pretty};
use tokio::{self};
use tokio::sync::broadcast::Sender;

use crate::events::{Event, EventInner};
use crate::prelude::PATH_TO_BINARIES;
use crate::stateful::Stateful;
use crate::traits::t_configurable::PathBuf;

use crate::traits::t_server::State;
use crate::traits::{Error, ErrorInner, TInstance};
use crate::util::{download_file, unzip_file, SetupProgress, rand_alphanumeric};

use self::util::{get_fabric_jar_url, get_jre_url, get_vanilla_jar_url, read_properties_from_path};

#[derive(Debug, Clone, Copy)]
pub enum Flavour {
    Vanilla,
    Fabric,
    Paper,
    Spigot,
}

impl<'de> serde::Deserialize<'de> for Flavour {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        debug!("Deserializing Flavour, {}", s);
        match s.to_lowercase().as_str() {
            "vanilla" => Ok(Flavour::Vanilla),
            "fabric" => Ok(Flavour::Fabric),
            "paper" => Ok(Flavour::Paper),
            "spigot" => Ok(Flavour::Spigot),
            _ => Err(serde::de::Error::custom(format!("Unknown flavour: {}", s))),
        }
    }
}
impl serde::Serialize for Flavour {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Flavour::Vanilla => serializer.serialize_str("vanilla"),
            Flavour::Fabric => serializer.serialize_str("fabric"),
            Flavour::Paper => serializer.serialize_str("paper"),
            Flavour::Spigot => serializer.serialize_str("spigot"),
        }
    }
}

impl ToString for Flavour {
    fn to_string(&self) -> String {
        match self {
            Flavour::Vanilla => "vanilla".to_string(),
            Flavour::Fabric => "fabric".to_string(),
            Flavour::Paper => "paper".to_string(),
            Flavour::Spigot => "spigot".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetupConfig {
    pub game_type: String,
    pub uuid : String,
    pub name : String,
    pub version : String,
    pub flavour : Flavour,
    pub port : u32,
    pub path : PathBuf,
    pub cmd_args: Option<Vec<String>>,
    pub description: Option<String>,
    pub fabric_loader_version: Option<String>,
    pub fabric_installer_version: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: Option<bool>,
    pub backup_period: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub game_type: String,
    pub uuid: String,
    pub name: String,
    pub version: String,
    pub fabric_loader_version: Option<String>,
    pub fabric_installer_version: Option<String>,
    // TODO: add paper support
    pub flavour: Flavour,
    pub description: String,
    pub cmd_args: Vec<String>,
    pub path: PathBuf,
    pub port: u32,
    pub min_ram: u32,
    pub max_ram: u32,
    pub creation_time: i64,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: bool,
    pub backup_period: Option<u32>,
    pub jre_major_version: u64,
    pub has_started : bool,
}

pub struct Instance {
    config: RestoreConfig,
    state: Arc<Mutex<Stateful<State>>>,
    event_broadcaster: Sender<Event>,
    // file paths
    path_to_config: PathBuf,
    path_to_properties: PathBuf,

    // directory paths
    path_to_macros: PathBuf,
    path_to_resources: PathBuf,
    path_to_runtimes: PathBuf,

    // variables which can be changed at runtime
    auto_start: Arc<AtomicBool>,
    restart_on_crash: Arc<AtomicBool>,
    timeout_last_left: Arc<Mutex<Option<u32>>>,
    timeout_no_activity: Arc<Mutex<Option<u32>>>,
    start_on_connection: Arc<AtomicBool>,
    backup_period: Arc<Mutex<Option<u32>>>,
    process: Option<Child>,
    players: Arc<Mutex<Stateful<HashSet<String>>>>,
    settings: Arc<Mutex<HashMap<String, String>>>,
}

impl Instance {
    pub async fn new(
         config: SetupConfig,
        event_broadcaster: Sender<Event>,
    ) -> Result<Instance, Error> {
        let path_to_config = config.path.join(".lodestone_config");
        let path_to_eula = config.path.join("eula.txt");
        let path_to_macros = config.path.join("macros");
        let path_to_resources = config.path.join("resources");
        let path_to_properties = config.path.join("server.properties");
        let path_to_runtimes = PATH_TO_BINARIES.with(|path| path.clone());

        let _ = event_broadcaster.send(Event::new(
            EventInner::Setup(SetupProgress {
                current_step: (1, "Creating directories".to_string()),
                total_steps: 4,
            }),
            config.uuid.clone(),
            config.name.clone(),
            "".to_string(),
            Some(rand_alphanumeric(5)),
        ));
        tokio::fs::create_dir_all(&config.path).await.map_err(|_| Error {
            inner: ErrorInner::FailedToWriteFileOrDir,
            detail: format!("failed to create directory {}", &config.path.display()),
        })?;

        // create eula file
        tokio::fs::write(&path_to_eula, "#generated by Lodestone\neula=true").await.map_err(|_| Error {
            inner: ErrorInner::FailedToWriteFileOrDir,
            detail: format!("failed to write to eula file {}", &config.path.display()),
        })?;

        tokio::fs::write(&path_to_properties, format!("server-port={}", config.port)).await.map_err(|_| Error {
            inner: ErrorInner::FailedToWriteFileOrDir,
            detail: format!("failed to write to server.properties file {}", &config.path.display()),
        })?;

        // create macros directory
        tokio::fs::create_dir_all(&path_to_macros).await.map_err(|_| Error {
            inner: ErrorInner::FailedToCreateFileOrDir,
            detail: format!("failed to create {}", &path_to_macros.display()),
        })?;

        // create resources directory
        tokio::fs::create_dir_all(path_to_resources.join("mods")).await.map_err(|_| Error {
            inner: ErrorInner::FailedToCreateFileOrDir,
            detail: format!(
                "failed to create mods directory {}",
                &path_to_resources.display()
            ),
        })?;
        tokio::fs::create_dir_all(path_to_resources.join("worlds")).await.map_err(|_| Error {
            inner: ErrorInner::FailedToCreateFileOrDir,
            detail: format!(
                "failed to create worlds directory {}",
                &path_to_resources.display()
            ),
        })?;
        tokio::fs::create_dir_all(path_to_resources.join("defaults")).await.map_err(|_| Error {
            inner: ErrorInner::FailedToCreateFileOrDir,
            detail: format!(
                "failed to create defaults directory {}",
                &path_to_resources.display()
            ),
        })?;
        let _ = event_broadcaster.send(Event::new(
            EventInner::Setup(SetupProgress {
                current_step: (2, "Downloading JRE".to_string()),
                total_steps: 4,
            }),
            config.uuid.clone(),
            config.name.clone(),
            "".to_string(),
            Some(rand_alphanumeric(5)),
        ));
        let (url, jre_major_version) = get_jre_url(config.version.as_str()).await.ok_or(Error {
            inner: ErrorInner::VersionNotFound,
            detail: format!("Cannot get the jre version for version {}", config.version),
        })?;


        // TODO: check if jre is already downloaded
        if !path_to_runtimes
            .join("java")
            .join(format!("jre{}", jre_major_version))
            .exists()
        {
            let downloaded = download_file(&url, &path_to_runtimes.join("java"), None, {
                let event_broadcaster = event_broadcaster.clone();
                let uuid = config.uuid.clone();
                let name = config.name.clone();
                &move |dl| {
                    let _ = event_broadcaster.send(Event::new(
                        EventInner::Downloading(dl),
                        uuid.clone(),
                        name.clone(),
                        "".to_string(),
                        Some(rand_alphanumeric(5)),
                    ));
                }
            }, true)
            .await?;
            let _ = event_broadcaster.send(Event::new(
                EventInner::Setup(SetupProgress {
                    current_step: (3, "Unzipping JRE".to_string()),
                    total_steps: 4,
                }),
                config.uuid.clone(),
                config.name.clone(),
                "".to_string(),
                Some(rand_alphanumeric(5)),
            ));
            let unzipped_content = unzip_file(
                &downloaded,
                &path_to_runtimes.join("java"),
            ).await?;
            if unzipped_content.len() != 1 {
                return Err(Error {
                    inner: ErrorInner::APIChanged,
                    detail: format!(
                        "Unzipped content has {} entries, expected 1. Please report this issue.",
                        unzipped_content.len()
                    ),
                });
            }

            tokio::fs::remove_file(&downloaded).await.map_err(|_| Error {
                inner: ErrorInner::FailedToRemoveFileOrDir,
                detail: format!("failed to delete {}", &downloaded.display()),
            })?;

            tokio::fs::rename(
                unzipped_content.iter().last().unwrap(),
                path_to_runtimes
                    .join("java")
                    .join(format!("jre{}", jre_major_version)),
            )
            .await.unwrap();
        }

        let _ = event_broadcaster.send(Event::new(
            EventInner::Setup(SetupProgress {
                current_step: (4, "Downloading server.jar".to_string()),
                total_steps: 4,
            }),
            config.uuid.clone(),
            config.name.clone(),
            "".to_string(),
            Some(rand_alphanumeric(5)),
        ));

        match config.flavour {
            Flavour::Vanilla => {
                download_file(
                    get_vanilla_jar_url(config.version.as_str())
                        .await
                        .ok_or(Error {
                            inner: ErrorInner::VersionNotFound,
                            detail: format!(
                                "Cannot get the vanilla jar version for version {}",
                                config.version
                            ),
                        })?
                        .as_str(),
                    &config.path,
                    Some("server.jar"),
                    {
                        let event_broadcaster = event_broadcaster.clone();
                        let uuid = config.uuid.clone();
                        let name = config.name.clone();
                        &move |dl| {
                            let _ = event_broadcaster.send(Event::new(
                                EventInner::Downloading(dl),
                                uuid.clone(),
                                name.clone(),
                                "".to_string(),
                                Some(rand_alphanumeric(5)),
                            ));
                        }
                    },
                    true,

                )
                .await?
            }
            Flavour::Fabric => {
                download_file(
                    get_fabric_jar_url(
                        &config.version,
                        config.fabric_installer_version.as_deref(),
                        config.fabric_loader_version.as_deref(),
                    )
                    .await
                    .ok_or(Error {
                        inner: ErrorInner::VersionNotFound,
                        detail: format!(
                            "Cannot get the fabric jar version for version {}",
                            config.version
                        ),
                    })?
                    .as_str(),
                    &config.path,
                    Some("server.jar"),
                    {
                        let event_broadcaster = event_broadcaster.clone();
                        let uuid = config.uuid.clone();
                        let name = config.name.clone();
                        &move |dl| {
                            let _ = event_broadcaster.send(Event::new(
                                EventInner::Downloading(dl),
                                uuid.clone(),
                                name.clone(),
                                "".to_string(),
                                Some(rand_alphanumeric(5)),
                            ));
                        }
                    },
                    true,
                )
                .await?
            }
            Flavour::Paper => todo!(),
            Flavour::Spigot => todo!(),
        };


        let restore_config = RestoreConfig {
            game_type: config.game_type,
            uuid: config.uuid,
            name: config.name,
            version: config.version,
            fabric_loader_version: config.fabric_loader_version,
            fabric_installer_version: config.fabric_installer_version,
            flavour: config.flavour,
            description: config.description.unwrap_or_else(|| "".to_string()),
            cmd_args: config.cmd_args.unwrap_or_default(),
            path: config.path,
            port: config.port,
            min_ram: config.min_ram.unwrap_or(2048),
            max_ram: config.max_ram.unwrap_or(4096),
            creation_time: chrono::Utc::now().timestamp(),
            auto_start: config.auto_start.unwrap_or(false),
            restart_on_crash: config.restart_on_crash.unwrap_or(false),
            timeout_last_left: config.timeout_last_left,
            timeout_no_activity: config.timeout_no_activity,
            start_on_connection: config.start_on_connection.unwrap_or(false),
            backup_period: config.backup_period,
            jre_major_version,
            has_started: false,
        };
                // create config file
                tokio::fs::write(
                    &path_to_config,
                    to_string_pretty(&restore_config).map_err(|_| Error {
                        inner: ErrorInner::MalformedFile,
                        detail: "config json malformed".to_string(),
                    })?,
                )
                .await.map_err(|_| Error {
                    inner: ErrorInner::FailedToWriteFileOrDir,
                    detail: format!("failed to write to config {}", &path_to_config.display()),
                })?;
        Ok(Instance::restore(restore_config, event_broadcaster))
    }

    pub fn restore(
        config: RestoreConfig,
        event_broadcaster: Sender<Event>,
    ) -> Instance {
        let path_to_config = config.path.join(".lodestone_config");
        let path_to_macros = config.path.join("macros");
        let path_to_resources = config.path.join("resources");
        let path_to_properties = config.path.join("server.properties");
        let path_to_runtimes = PATH_TO_BINARIES.with(|path| path.clone());
        let state_callback = {
            let event_broadcaster = event_broadcaster.clone();
            let uuid = config.uuid.clone();
            let name = config.name.clone();
            move |old_state: &State, new_state: &State| -> Result<(), Error> {
                debug!(
                    "[{}] Transitioning from {} to {}",
                    name,
                    old_state.to_string(),
                    new_state.to_string()
                );
                let (ret, event_inner, details, log): (
                    Result<(), Error>,
                    EventInner,
                    String,
                    Box<dyn Fn()>,
                ) = match (old_state, new_state) {
                    (State::Starting, State::Starting) => {
                        let err_message = "Cannot start, instance is already starting";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStarting,
                                detail: err_message.to_owned(),
                            }),
                            EventInner::InstanceStarting,
                            err_message.to_owned(),
                            Box::new(|| warn!("[{}] {}", &name, err_message.to_owned())),
                        )
                    }
                    (State::Starting, State::Running) => {
                        let msg = "Instance started";
                        (
                            Ok(()),
                            EventInner::InstanceStarted,
                            msg.to_owned(),
                            Box::new(|| info!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Starting, State::Stopping) => {
                        let msg = "Cannot stop, instance is not fully started";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStopping,
                                detail: msg.to_owned(),
                            }),
                            EventInner::InstanceStopping,
                            msg.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Starting, State::Stopped) => {
                        let msg = "Instance exited unexpectly before fully started up";
                        (
                            Ok(()),
                            EventInner::InstanceError,
                            msg.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Running, State::Starting) => {
                        let msg = "Instance is already running";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceErrored,
                                detail: msg.to_owned(),
                            }),
                            EventInner::InstanceError,
                            msg.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Running, State::Running) => {
                        let msg = "Instance is already running";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStarted,
                                detail: msg.to_owned(),
                            }),
                            EventInner::InstanceError,
                            msg.to_owned(),
                            Box::new(|| warn!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Running, State::Stopping) => {
                        let msg = "Instance is stopping";
                        (
                            Ok(()),
                            EventInner::InstanceStopping,
                            msg.to_owned(),
                            Box::new(|| info!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Running, State::Stopped) => {
                        let msg = "Instance transitioned from Running to Stopped state without the Stopping state. \
                            This probably mean the instance has crashed while running, or got killed by the system. But could also mean Lodestone failed to detect when the instance is stopping. \
                            If you believe this is a bug, please report it";
                        (
                            Ok(()),
                            EventInner::InstanceError,
                            msg.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, msg.to_owned())),
                        )
                    }
                    (State::Stopping, State::Starting) => {
                        let err_msg = "Cannot start, instance is stopping";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStarting,
                                detail: err_msg.to_owned(),
                            }),
                            EventInner::InstanceStarting,
                            err_msg.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, err_msg.to_owned())),
                        )
                    }
                    (State::Stopping, State::Running) => todo!(),
                    (State::Stopping, State::Stopping) => {
                        let err_message = "Instance is already stopping";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStopping,
                                detail: err_message.to_owned(),
                            }),
                            EventInner::InstanceStopping,
                            err_message.to_owned(),
                            Box::new({
                                let name = name.clone();
                                // let err_message = err_message.clone();
                                move || warn!("[{}] {}", &name, &err_message)
                            }),
                        )
                    }
                    (State::Stopping, State::Stopped) => {
                        let msg = "Instance stopped";
                        (
                            Ok(()),
                            EventInner::InstanceStopped,
                            msg.to_owned(),
                            Box::new({
                                let name = name.clone();
                                move || info!("[{}] {}", &name, &msg)
                            }),
                        )
                    }
                    (State::Stopped, State::Starting) => {
                        let msg = "Instance is starting";
                        (
                            Ok(()),
                            EventInner::InstanceStarting,
                            msg.to_owned(),
                            Box::new({
                                let name = name.clone();
                                move || info!("[{}] {}", &name, &msg)
                            }),
                        )
                    }
                    (State::Stopped, State::Running) => todo!(),
                    (State::Stopped, State::Stopping) => {
                        let err_msg = "Instance is already stopped";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStopped,
                                detail: err_msg.to_owned(),
                            }),
                            EventInner::InstanceStopping,
                            err_msg.to_owned(),
                            Box::new({
                                let name = name.clone();
                                move || warn!("[{}] {}", &name, &err_msg)
                            }),
                        )
                    }
                    (State::Stopped, State::Stopped) => {
                        let err_message = "Instance is already stopped";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceStopped,
                                detail: err_message.to_owned(),
                            }),
                            EventInner::InstanceStopped,
                            err_message.to_owned(),
                            Box::new({
                                let name = name.clone();
                                // let err_message = err_message.clone();
                                move || warn!("[{}] {}", &name, &err_message)
                            }),
                        )
                    }
                    (State::Error, State::Error) => {
                        let err_message = "The instance errored, and somehow it launched, and errored again. Idk how you managed to get here, but please report this bug";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceErrored,
                                detail: err_message.to_owned(),
                            }),
                            EventInner::InstanceError,
                            err_message.to_owned(),
                            Box::new(|| error!("[{}] {}", &name, err_message.to_owned())),
                        )
                    }
                    (_, State::Error) => {
                        let err_message = 
                            "Instance entering error state. To protect your server, it will not be able to start again until Lodestone is restarted. A manual inspection of the instance is highly recommended.";
                        (
                            Err(Error {
                                inner: ErrorInner::InstanceErrored,
                                detail: err_message.to_owned(),
                            }),
                            EventInner::InstanceError,
                            err_message.to_owned(),
                            Box::new({
                                let name = name.clone();
                                // let err_message = err_message.clone();
                                move || error!("[{}] {}", &name, &err_message)
                            }),
                        )
                    }
                    (State::Error, _) => {
                        let err_message = format!(
                            "Cannot transit from Error state to {}, please inspect your instance manually and restart Lodestone",
                            new_state.to_string()
                        );

                        (
                            Err(Error {
                                inner: ErrorInner::InstanceErrored,
                                detail: "instance errored".to_string(),
                            }),
                            EventInner::InstanceError,
                            err_message.clone(),
                            Box::new({
                                let err_message = err_message;
                                let name = name.clone();

                                move || error!("[{}] {}", &name, &err_message)
                            }),
                        )
                    }
                };
                log();
                let _ = event_broadcaster
                    .send(Event::new(
                        event_inner,
                        uuid.clone(),
                        name.clone(),
                        details,
                        None,
                    ))
                    .map_err(|e| {
                        warn!(
                            "Failed to send event to event broadcaster: {}",
                            e.to_string()
                        )
                    });
                ret
            }
        };

        let players_callback = {
            let event_broadcaster = event_broadcaster.clone();
            let uuid = config.uuid.clone();
            let name = config.name.clone();
            move |old_players: &HashSet<String>, new_players: &HashSet<String>| {
                if old_players.len() > new_players.len() {
                    let player_diff = old_players.difference(new_players).last().unwrap();
                    debug!("[{}] Detected player joined: {}", name, player_diff);
                    let _ = event_broadcaster.send(Event::new(
                        EventInner::PlayerJoined(player_diff.to_owned()),
                        uuid.clone(),
                        name.clone(),
                        format!("Player joined: {}", player_diff),
                        None,
                    ));
                } else if old_players.len() < new_players.len() {
                    let player_diff = new_players.difference(old_players).last().unwrap();
                    debug!("[{}] Detected player left: {}", name, player_diff);
                    let _ = event_broadcaster.send(Event::new(
                        EventInner::PlayerLeft(player_diff.to_owned()),
                        uuid.clone(),
                        name.clone(),
                        format!("Player left: {}", player_diff),
                        None,
                    ));
                }
                Ok(())
            }
        };
        
        let mut instance = Instance {
            state: Arc::new(Mutex::new(Stateful::new(
                State::Stopped,
                Box::new(state_callback),
                Box::new(|_, _| Ok(())),
            ))),
            auto_start: Arc::new(AtomicBool::new(config.auto_start)),
            restart_on_crash: Arc::new(AtomicBool::new(config.restart_on_crash)),
            timeout_last_left: Arc::new(Mutex::new(config.timeout_last_left)),
            timeout_no_activity: Arc::new(Mutex::new(config.timeout_no_activity)),
            start_on_connection: Arc::new(AtomicBool::new(config.start_on_connection)),
            backup_period: Arc::new(Mutex::new(config.backup_period)),
            config,
            path_to_config,
            path_to_properties,
            path_to_macros,
            path_to_resources,
            event_broadcaster,
            path_to_runtimes,
            process: None,
            players: Arc::new(Mutex::new(Stateful::new(
                HashSet::new(),
                Box::new(players_callback.clone()),
                Box::new(players_callback),
            ))),
            settings: Arc::new(Mutex::new(HashMap::new())),
        };
        let _ = instance.read_properties();
        instance
    }

    async fn write_config_to_file(&self) -> Result<(), Error> {
        tokio::fs::write(
            &self.path_to_config,
            to_string_pretty(&self.config).map_err(|_| Error {
                inner: ErrorInner::MalformedFile,
                detail: "config json malformed".to_string(),
            })?,
        )
        .await.map_err(|_| Error {
            inner: ErrorInner::FailedToWriteFileOrDir,
            detail: format!(
                "failed to write to config {}",
                &self.path_to_config.display()
            ),
        })
    }

    async fn read_properties(&mut self) -> Result<(), Error> {
            *self.settings.lock().await = read_properties_from_path(&self.path_to_properties).map_err(|_| Error { inner: ErrorInner::FailedToReadFileOrDir, detail: "".to_string() })?;
            Ok(())
    }

    async fn write_properties_to_file(&self) -> Result<(), Error> {
        let file = File::open(&self.path_to_properties).await.map_err(|_| Error {
            inner: ErrorInner::FailedToWriteFileOrDir,
            detail: String::new(),
        })?;
        let mut file_writer = tokio::io::BufWriter::new(file);
        
        for (key, value) in self.settings
        .lock().await.iter() {
            file_writer
                .write_all(format!("{}={}", key, value).as_bytes()).await
                .map_err(|_| Error {
                    inner: ErrorInner::FailedToWriteFileOrDir,
                    detail: String::new(),
                })?;
        }
        Ok(())
    }

}

impl TInstance for Instance {}

