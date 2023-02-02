pub mod configurable;
pub mod r#macro;
pub mod manifest;
pub mod player;
mod players_manager;
pub mod resource;
pub mod server;
mod util;
pub mod versions;

use color_eyre::eyre::{eyre, Context};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::SystemExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;

use tokio::sync::Mutex;

use ::serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use tokio::sync::broadcast::Sender;
use tracing::{debug, error, info};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::{self};
use ts_rs::TS;

use crate::error::Error;
use crate::events::{CausedBy, Event, EventInner, ProgressionEvent, ProgressionEventInner};
use crate::macro_executor::MacroExecutor;
use crate::prelude::PATH_TO_BINARIES;
use crate::traits::t_configurable::PathBuf;

use crate::traits::t_server::State;
use crate::traits::TInstance;
use crate::types::{InstanceUuid, Snowflake};
use crate::util::{download_file, format_byte, format_byte_download, unzip_file};

use self::players_manager::PlayersManager;
use self::util::{get_fabric_jar_url, get_jre_url, get_vanilla_jar_url, read_properties_from_path};

#[derive(Debug, Clone, Copy, TS, Serialize, Deserialize)]
#[serde(rename = "MinecraftFlavour", rename_all = "snake_case")]
#[ts(export)]
pub enum Flavour {
    Vanilla,
    Fabric,
    Paper,
    Spigot,
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
    pub uuid: InstanceUuid,
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub port: u32,
    pub path: PathBuf,
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
    pub uuid: InstanceUuid,
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
    pub backup_period: Option<u32>,
    pub jre_major_version: u64,
    pub has_started: bool,
}

#[derive(Clone)]
pub struct MinecraftInstance {
    config: RestoreConfig,
    state: Arc<Mutex<State>>,
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
    backup_period: Option<u32>,
    process: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    system: Arc<Mutex<sysinfo::System>>,
    players_manager: Arc<Mutex<PlayersManager>>,
    settings: Arc<Mutex<HashMap<String, String>>>,
    macro_executor: MacroExecutor,
    backup_sender: UnboundedSender<BackupInstruction>,
    pub rcon_conn: Arc<Mutex<Option<rcon::Connection<tokio::net::TcpStream>>>>,
}

#[derive(Debug, Clone)]
enum BackupInstruction {
    SetPeriod(Option<u32>),
    BackupNow,
    Pause,
    Resume,
}

impl MinecraftInstance {
    pub async fn new(
        config: SetupConfig,
        progression_event_id: Snowflake,
        event_broadcaster: Sender<Event>,
        macro_executor: MacroExecutor,
    ) -> Result<MinecraftInstance, Error> {
        let path_to_config = config.path.join(".lodestone_config");
        let path_to_eula = config.path.join("eula.txt");
        let path_to_macros = config.path.join("macros");
        let path_to_resources = config.path.join("resources");
        let path_to_properties = config.path.join("server.properties");
        let path_to_runtimes = PATH_TO_BINARIES.with(|path| path.clone());

        let _ = event_broadcaster.send(Event {
            event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                event_id: progression_event_id,
                progression_event_inner: ProgressionEventInner::ProgressionUpdate {
                    progress: 1.0,
                    progress_message: "1/4: Creating directories".to_string(),
                },
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by: CausedBy::Unknown,
        });
        tokio::fs::create_dir_all(&config.path)
            .await
            .and(tokio::fs::create_dir_all(&path_to_macros).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("mods")).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("worlds")).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("defaults")).await)
            .and(tokio::fs::write(&path_to_eula, "#generated by Lodestone\neula=true").await)
            .and(
                tokio::fs::write(&path_to_properties, format!("server-port={}", config.port)).await,
            )
            .context("Could not create some files or directories for instance")
            .map_err(|e| {
                error!("{e}");
                e
            })?;

        let (url, jre_major_version) = get_jre_url(config.version.as_str())
            .await
            .ok_or(eyre!("Could not find a JRE for version {}", config.version))?;
        if !path_to_runtimes
            .join("java")
            .join(format!("jre{}", jre_major_version))
            .exists()
        {
            let _progression_parent_id = progression_event_id;
            let downloaded = download_file(
                &url,
                &path_to_runtimes.join("java"),
                None,
                {
                    let event_broadcaster = event_broadcaster.clone();
                    let _uuid = config.uuid.clone();
                    let progression_event_id = progression_event_id;
                    &move |dl| {
                        if let Some(total) = dl.total {
                            let _ = event_broadcaster.send(Event {
                                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                                    event_id: progression_event_id,
                                    progression_event_inner:
                                        ProgressionEventInner::ProgressionUpdate {
                                            progress: (dl.step as f64 / total as f64) * 4.0,
                                            progress_message: format!(
                                                "2/4: Downloading JRE {}",
                                                format_byte_download(dl.downloaded, total)
                                            ),
                                        },
                                }),
                                details: "".to_string(),
                                snowflake: Snowflake::default(),
                                caused_by: CausedBy::Unknown,
                            });
                        }
                    }
                },
                true,
            )
            .await?;

            let unzipped_content =
                unzip_file(&downloaded, &path_to_runtimes.join("java"), true).await?;
            if unzipped_content.len() != 1 {
                return Err(eyre!(
                    "Expected only one file in the JRE archive, got {}",
                    unzipped_content.len()
                )
                .into());
            }

            tokio::fs::remove_file(&downloaded).await.context(format!(
                "Could not remove downloaded JRE file {}",
                downloaded.display()
            ))?;

            tokio::fs::rename(
                unzipped_content.iter().last().unwrap(),
                path_to_runtimes
                    .join("java")
                    .join(format!("jre{}", jre_major_version)),
            )
            .await
            .unwrap();
        } else {
            let _ = event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: progression_event_id,
                    progression_event_inner: ProgressionEventInner::ProgressionUpdate {
                        progress: 4.0,
                        progress_message: "2/4: JRE already downloaded".to_string(),
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: CausedBy::Unknown,
            });
        }
        match config.flavour {
            Flavour::Vanilla => {
                download_file(
                    get_vanilla_jar_url(config.version.as_str())
                        .await
                        .ok_or_else({
                            || {
                                eyre!(
                                    "Could not find a vanilla server.jar for version {}",
                                    config.version
                                )
                            }
                        })?
                        .as_str(),
                    &config.path,
                    Some("server.jar"),
                    {
                        let event_broadcaster = event_broadcaster.clone();
                        let progression_event_id = progression_event_id;
                        &move |dl| {
                            if let Some(total) = dl.total {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                                        event_id: progression_event_id,
                                        progression_event_inner:
                                            ProgressionEventInner::ProgressionUpdate {
                                                progress: (dl.step as f64 / total as f64) * 5.0,
                                                progress_message: format!(
                                                    "3/4: Downloading vanilla server.jar {}",
                                                    format_byte_download(dl.downloaded, total)
                                                ),
                                            },
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::Unknown,
                                });
                            } else {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                                        event_id: progression_event_id,
                                        progression_event_inner:
                                            ProgressionEventInner::ProgressionUpdate {
                                                progress: 0.0,
                                                progress_message: format!(
                                                    "3/4: Downloading vanilla server.jar {:.1} MB",
                                                    format_byte(dl.downloaded),
                                                ),
                                            },
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::Unknown,
                                });
                            }
                        }
                    },
                    true,
                )
                .await
            }
            Flavour::Fabric => {
                download_file(
                    get_fabric_jar_url(
                        &config.version,
                        config.fabric_installer_version.as_deref(),
                        config.fabric_loader_version.as_deref(),
                    )
                    .await
                    .ok_or_else({
                        || {
                            eyre!(
                                "Could not find a Fabric server.jar for version {}",
                                config.version
                            )
                        }
                    })?
                    .as_str(),
                    &config.path,
                    Some("server.jar"),
                    {
                        let event_broadcaster = event_broadcaster.clone();
                        let progression_event_id = progression_event_id;
                        &move |dl| {
                            if let Some(total) = dl.total {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                                        event_id: progression_event_id,
                                        progression_event_inner:
                                            ProgressionEventInner::ProgressionUpdate {
                                                progress: (dl.step as f64 / total as f64) * 5.0,
                                                progress_message: format!(
                                                    "3/4: Downloading Fabric server.jar {}",
                                                    format_byte_download(dl.downloaded, total),
                                                ),
                                            },
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::Unknown,
                                });
                            } else {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                                        event_id: progression_event_id,
                                        progression_event_inner:
                                            ProgressionEventInner::ProgressionUpdate {
                                                progress: 0.0,
                                                progress_message: format!(
                                                    "3/4: Downloading Fabric server.jar {}",
                                                    format_byte(dl.downloaded),
                                                ),
                                            },
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::Unknown,
                                });
                            }
                        }
                    },
                    true,
                )
                .await
            }
            Flavour::Paper => todo!(),
            Flavour::Spigot => todo!(),
        }
        .map_err(|e| {
            // delete the server.jar if it exists
            error!("Error downloading server.jar: {}", e);
            let _ = std::fs::remove_file(config.path.join("server.jar"));
            e
        })?;

        let _ = event_broadcaster.send(Event {
            event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                event_id: progression_event_id,
                progression_event_inner: ProgressionEventInner::ProgressionUpdate {
                    progress: 1.0,
                    progress_message: "4/4: Finishing up".to_string(),
                },
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by: CausedBy::Unknown,
        });

        let restore_config = RestoreConfig {
            game_type: config.game_type,
            uuid: config.uuid,
            name: config.name,
            version: config.version,
            fabric_loader_version: config.fabric_loader_version,
            fabric_installer_version: config.fabric_installer_version,
            flavour: config.flavour,
            description: config.description.unwrap_or_default(),
            cmd_args: config.cmd_args.unwrap_or_default(),
            path: config.path,
            port: config.port,
            min_ram: config.min_ram.unwrap_or(2048),
            max_ram: config.max_ram.unwrap_or(4096),
            creation_time: chrono::Utc::now().timestamp(),
            auto_start: config.auto_start.unwrap_or(false),
            restart_on_crash: config.restart_on_crash.unwrap_or(false),
            backup_period: config.backup_period,
            jre_major_version,
            has_started: false,
        };
        // create config file
        tokio::fs::write(
            &path_to_config,
            to_string_pretty(&restore_config).context(
                "Failed to serialize config to string. This is a bug, please report it.",
            )?,
        )
        .await
        .context(format!(
            "Failed to write config file at {}",
            &path_to_config.display()
        ))?;
        Ok(MinecraftInstance::restore(restore_config, event_broadcaster, macro_executor).await)
    }

    pub async fn restore(
        config: RestoreConfig,
        event_broadcaster: Sender<Event>,
        _macro_executor: MacroExecutor,
    ) -> MinecraftInstance {
        let path_to_config = config.path.join(".lodestone_config");
        let path_to_macros = config.path.join("macros");
        let path_to_resources = config.path.join("resources");
        let path_to_properties = config.path.join("server.properties");
        let path_to_runtimes = PATH_TO_BINARIES.with(|path| path.clone());
        // if the properties file doesn't exist, create it
        if !path_to_properties.exists() {
            tokio::fs::write(&path_to_properties, format!("server-port={}", config.port))
                .await
                .expect("failed to write to server.properties");
        };
        let state = Arc::new(Mutex::new(State::Stopped));
        let (backup_tx, mut backup_rx): (
            UnboundedSender<BackupInstruction>,
            UnboundedReceiver<BackupInstruction>,
        ) = tokio::sync::mpsc::unbounded_channel();
        let _backup_task = tokio::spawn({
            let backup_period = config.backup_period;
            let path_to_resources = path_to_resources.clone();
            let path_to_instance = config.path.clone();
            let state = state.clone();
            async move {
                let backup_now = || async {
                    debug!("Backing up instance");
                    let backup_dir = &path_to_resources.join("worlds/backup");
                    tokio::fs::create_dir_all(&backup_dir).await.ok();
                    // get current time in human readable format
                    let time = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S");
                    let backup_name = format!("backup-{}", time);
                    let backup_path = backup_dir.join(&backup_name);
                    if let Err(e) = tokio::task::spawn_blocking({
                        let path_to_instance = path_to_instance.clone();
                        let backup_path = backup_path.clone();
                        let mut copy_option = fs_extra::dir::CopyOptions::new();
                        copy_option.copy_inside = true;
                        move || {
                            fs_extra::dir::copy(
                                path_to_instance.join("world"),
                                &backup_path,
                                &copy_option,
                            )
                        }
                    })
                    .await
                    {
                        error!("Failed to backup instance: {}", e);
                    }
                };
                let mut backup_period = backup_period;
                let mut counter = 0;
                loop {
                    tokio::select! {
                           instruction = backup_rx.recv() => {
                             if instruction.is_none() {
                                 info!("Backup task exiting");
                                 break;
                             }
                             let instruction = instruction.unwrap();
                             match instruction {
                             BackupInstruction::SetPeriod(new_period) => {
                                 backup_period = new_period;
                             },
                             BackupInstruction::BackupNow => backup_now().await,
                             BackupInstruction::Pause => {
                                     loop {
                                         if let Some(BackupInstruction::Resume) = backup_rx.recv().await {
                                             break;
                                         } else {
                                             continue
                                         }
                                     }

                             },
                             BackupInstruction::Resume => {
                                 continue;
                             },
                             }
                           }
                           _ = tokio::time::sleep(Duration::from_secs(1)) => {
                             if let Some(period) = backup_period {
                                 if *state.lock().await == State::Running {
                                     debug!("counter is {}", counter);
                                     counter += 1;
                                     if counter >= period {
                                         counter = 0;
                                         backup_now().await;
                                     }
                                 }
                             }
                           }
                    }
                }
            }
        });
        let mut instance = MinecraftInstance {
            state: Arc::new(Mutex::new(State::Stopped)),
            auto_start: Arc::new(AtomicBool::new(config.auto_start)),
            restart_on_crash: Arc::new(AtomicBool::new(config.restart_on_crash)),
            backup_period: config.backup_period,
            players_manager: Arc::new(Mutex::new(PlayersManager::new(
                event_broadcaster.clone(),
                config.uuid.clone(),
            ))),
            config,
            path_to_config,
            path_to_properties,
            path_to_macros,
            path_to_resources,
            macro_executor: MacroExecutor::new(event_broadcaster.clone()),
            event_broadcaster,
            path_to_runtimes,
            process: Arc::new(Mutex::new(None)),

            settings: Arc::new(Mutex::new(HashMap::new())),
            system: Arc::new(Mutex::new(sysinfo::System::new_all())),
            stdin: Arc::new(Mutex::new(None)),
            backup_sender: backup_tx,
            rcon_conn: Arc::new(Mutex::new(None)),
        };
        instance
            .read_properties()
            .await
            .expect("Failed to read properties");
        instance
    }

    async fn write_config_to_file(&self) -> Result<(), Error> {
        tokio::fs::write(
            &self.path_to_config,
            to_string_pretty(&self.config)
                .context("Failed to serialize config to string, this is a bug, please report it")?,
        )
        .await
        .context(format!(
            "Failed to write config to file at {}",
            &self.path_to_config.display()
        ))?;
        Ok(())
    }

    async fn read_properties(&mut self) -> Result<(), Error> {
        *self.settings.lock().await = read_properties_from_path(&self.path_to_properties).await?;
        Ok(())
    }

    async fn write_properties_to_file(&self) -> Result<(), Error> {
        // open the file in write-only mode, returns `io::Result<File>`
        let mut file = tokio::fs::File::create(&self.path_to_properties)
            .await
            .context(format!(
                "Failed to open properties file at {}",
                &self.path_to_properties.display()
            ))?;
        let mut setting_str = "".to_string();
        for (key, value) in self.settings.lock().await.iter() {
            // print the key and value separated by a =
            // println!("{}={}", key, value);
            setting_str.push_str(&format!("{}={}\n", key, value));
        }
        file.write_all(setting_str.as_bytes())
            .await
            .context(format!(
                "Failed to write properties to file at {}",
                &self.path_to_properties.display()
            ))?;
        Ok(())
    }

    pub async fn send_rcon(&self, cmd: &str) -> Result<String, Error> {
        let a = self
            .rcon_conn
            .clone()
            .lock()
            .await
            .as_mut()
            .ok_or_else(|| {
                eyre!("Failed to send rcon command, rcon connection is not initialized")
            })?
            .cmd(cmd)
            .await
            .context("Failed to send rcon command")?;
        Ok(a)
    }
}

impl TInstance for MinecraftInstance {}
