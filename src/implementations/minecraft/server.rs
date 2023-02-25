use std::collections::{BTreeMap};
use std::env;
use std::process::Stdio;
use std::time::Duration;

use color_eyre::eyre::{eyre, Context};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::player::MinecraftPlayer;
use crate::implementations::minecraft::util::{name_to_uuid, read_properties_from_path};
use crate::prelude::LODESTONE_PATH;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_macro::TMacro;
use crate::traits::t_server::{MonitorReport, State, StateAction, TServer};

use crate::types::Snowflake;
use crate::util::dont_spawn_terminal;

use super::r#macro::{resolve_macro_invocation, MinecraftMainWorkerGenerator};
use super::{Flavour, ForgeBuildVersion, MinecraftInstance};
use tracing::{debug, error, info, warn};

#[async_trait::async_trait]
impl TServer for MinecraftInstance {
    async fn start(&mut self, cause_by: CausedBy, block: bool) -> Result<(), Error> {
        self.state.lock().await.try_transition(
            StateAction::UserStart,
            Some(&|state| {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_name: self.config.name.clone(),
                        instance_uuid: self.uuid.clone(),
                        instance_event_inner: InstanceEventInner::StateTransition { to: state },
                    }),
                    snowflake: Snowflake::default(),
                    details: "Starting server".to_string(),
                    caused_by: cause_by.clone(),
                });
            }),
        )?;

        if !port_scanner::local_port_available(self.config.port as u16) {
            return Err(Error {
                kind: ErrorKind::Internal,
                source: eyre!("Port {} is already in use", self.config.port),
            });
        }

        env::set_current_dir(&self.path_to_instance).context(
            "Failed to set current directory to the instance's path, is the path valid?",
        )?;

        let prelaunch = resolve_macro_invocation(&self.path_to_macros, "prelaunch", false);
        if let Some(prelaunch) = prelaunch {
            let _ = self
                .macro_executor
                .spawn(
                    prelaunch,
                    Vec::new(),
                    cause_by.clone(),
                    Box::new(MinecraftMainWorkerGenerator::new(self.clone())),
                    Some(self.uuid.clone()),
                )
                .await
                .map_err(|e| {
                    error!("Failed to spawn prelaunch script: {}", e);
                    e
                })?;
        } else {
            info!(
                "[{}] No prelaunch script found, skipping",
                self.config.name.clone()
            );
        }

        let jre = self
            .path_to_runtimes
            .join("java")
            .join(format!("jre{}", self.config.jre_major_version))
            .join(if std::env::consts::OS == "macos" {
                "Contents/Home/bin"
            } else {
                "bin"
            })
            .join("java");

        let mut server_start_command = Command::new(&jre);
        let server_start_command = server_start_command
            .arg(format!("-Xmx{}M", self.config.max_ram))
            .arg(format!("-Xms{}M", self.config.min_ram))
            .args(
                &self
                    .config
                    .cmd_args
                    .iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<&String>>(),
            );

        let server_start_command = match &self.config.flavour {
            Flavour::Forge { build_version } => {
                let ForgeBuildVersion(build_version) = build_version
                    .as_ref()
                    .ok_or_else(|| eyre!("Forge version not found"))?;
                let forge_args = match std::env::consts::OS {
                    "windows" => "win_args.txt",
                    _ => "unix_args.txt",
                };
                let mut full_forge_args = std::ffi::OsString::from("@");
                full_forge_args.push(
                    self.path_to_instance
                        .join("libraries")
                        .join("net")
                        .join("minecraftforge")
                        .join("forge")
                        .join(format!(
                            "{}-{}",
                            self.config.version,
                            build_version.as_str()
                        ))
                        .join(forge_args)
                        .into_os_string()
                        .as_os_str(),
                );
                server_start_command.arg(full_forge_args)
            }
            _ => server_start_command
                .arg("-jar")
                .arg(&self.path_to_instance.join("server.jar")),
        };

        let server_start_command = server_start_command.arg("nogui");

        match dont_spawn_terminal(server_start_command)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut proc) => {
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).context(
                    "Failed to set current directory to the Lodestone path, is the path valid?",
                )?;
                let stdin = proc.stdin.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stdin during startup",
                        self.config.name.clone()
                    );
                    eyre!("Failed to take stdin during startup")
                })?;
                self.stdin.lock().await.replace(stdin);
                let stdout = proc.stdout.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stdout during startup",
                        self.config.name.clone()
                    );
                    eyre!("Failed to take stdout during startup")
                })?;
                let stderr = proc.stderr.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stderr during startup",
                        self.config.name.clone()
                    );
                    eyre!("Failed to take stderr during startup")
                })?;
                *self.process.lock().await = Some(proc);
                tokio::task::spawn({
                    use fancy_regex::Regex;
                    use lazy_static::lazy_static;

                    let event_broadcaster = self.event_broadcaster.clone();
                    let server_properties_buffer = self.server_properties_buffer.clone();
                    let _state = self.state.clone();
                    let path_to_properties = self.path_to_properties.clone();
                    let uuid = self.uuid.clone();
                    let name = self.config.name.clone();
                    let players_manager = self.players_manager.clone();
                    let macro_executor = self.macro_executor.clone();
                    let mut __self = self.clone();
                    async move {
                        fn parse_system_msg(msg: &str) -> Option<String> {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r"\[.+\]+: (?!<)(.+)").unwrap();
                            }
                            if RE.is_match(msg).ok()? {
                                RE.captures(msg)
                                    .ok()?
                                    .map(|caps| caps.get(1).unwrap().as_str().to_string())
                            } else {
                                None
                            }
                        }

                        fn parse_player_msg(msg: &str) -> Option<(String, String)> {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r"\[.+\]+: <(.+)> (.+)").unwrap();
                            }
                            if RE.is_match(msg).unwrap() {
                                if let Some(cap) = RE.captures(msg).ok()? {
                                    Some((
                                        cap.get(1)?.as_str().to_string(),
                                        cap.get(2)?.as_str().to_string(),
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }

                        fn parse_player_joined(system_msg: &str) -> Option<String> {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r"(.+) joined the game").unwrap();
                            }
                            if RE.is_match(system_msg).unwrap() {
                                if let Some(cap) = RE.captures(system_msg).ok()? {
                                    Some(cap.get(1)?.as_str().to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }

                        fn parse_player_left(system_msg: &str) -> Option<String> {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r"(.+) left the game").unwrap();
                            }
                            if RE.is_match(system_msg).unwrap() {
                                if let Some(cap) = RE.captures(system_msg).ok()? {
                                    Some(cap.get(1)?.as_str().to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }

                        enum MacroInstruction {
                            Abort(usize),
                            Spawn {
                                player_name: String,
                                macro_name: String,
                                args: Vec<String>,
                            },
                        }

                        fn parse_macro_invocation(msg: &str) -> Option<MacroInstruction> {
                            if let Some((player, msg)) = parse_player_msg(msg) {
                                lazy_static! {
                                    static ref RE: Regex =
                                        Regex::new(r"\.macro abort (.+)").unwrap();
                                }
                                if RE.is_match(&msg).unwrap() {
                                    if let Some(cap) = RE.captures(&msg).ok()? {
                                        Some(MacroInstruction::Abort(
                                            cap.get(1)?.as_str().parse().ok()?,
                                        ))
                                    } else {
                                        None
                                    }
                                } else {
                                    lazy_static! {
                                        static ref RE: Regex =
                                            Regex::new(r"\.macro spawn (.+)").unwrap();
                                    }
                                    if RE.is_match(&msg).unwrap() {
                                        if let Some(cap) = RE.captures(&msg).ok()? {
                                            // the first capture is the whole string
                                            // the first word is the macro name
                                            // the rest are the arguments
                                            // read the first word as the macro name
                                            let macro_name = cap
                                                .get(1)?
                                                .as_str()
                                                .to_string()
                                                .split_whitespace()
                                                .next()?
                                                .to_string();
                                            let args = cap
                                                .get(1)?
                                                .as_str()
                                                .split_whitespace()
                                                .skip(1)
                                                .map(|s| s.to_string())
                                                .collect::<Vec<String>>();

                                            Some(MacroInstruction::Spawn {
                                                player_name: player,
                                                macro_name,
                                                args,
                                            })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        }

                        fn parse_server_started(system_msg: &str) -> bool {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r#"Done \(.+\)!"#).unwrap();
                            }
                            RE.is_match(system_msg).unwrap()
                        }

                        let mut did_start = false;

                        let mut stdout_lines = BufReader::new(stdout).lines();
                        let mut stderr_lines = BufReader::new(stderr).lines();

                        while let (Ok(Some(line)), is_stdout) = tokio::select!(
                            line = stdout_lines.next_line() => {
                                (line, true)
                            }
                            line = stderr_lines.next_line() => {
                                (line, false)
                            }
                        ) {
                            if is_stdout {
                                // info!("[{}] {}", name, line);
                            } else {
                                warn!("[{}] {}", name, line);
                            }
                            let _ = event_broadcaster.send(Event {
                                event_inner: EventInner::InstanceEvent(InstanceEvent {
                                    instance_uuid: uuid.clone(),
                                    instance_event_inner: InstanceEventInner::InstanceOutput {
                                        message: line.clone(),
                                    },
                                    instance_name: name.clone(),
                                }),
                                details: "".to_string(),
                                snowflake: Snowflake::default(),
                                caused_by: CausedBy::System,
                            });

                            if parse_server_started(&line) && !did_start {
                                did_start = true;
                                self.state
                                    .lock()
                                    .await
                                    .try_transition(
                                        StateAction::InstanceStart,
                                        Some(&|state| {
                                            self.event_broadcaster.send(Event {
                                                event_inner: EventInner::InstanceEvent(
                                                    InstanceEvent {
                                                        instance_name: self.config.name.clone(),
                                                        instance_uuid: self.uuid.clone(),
                                                        instance_event_inner:
                                                            InstanceEventInner::StateTransition {
                                                                to: state,
                                                            },
                                                    },
                                                ),
                                                snowflake: Snowflake::default(),
                                                details: "Starting server".to_string(),
                                                caused_by: cause_by.clone(),
                                            });
                                        }),
                                    )
                                    .unwrap();

                                *server_properties_buffer.lock().await =
                                    read_properties_from_path(&path_to_properties)
                                        .await.map_err(|e| {
                                            error!("Failed to read properties: {}, falling back to empty properties map", e);
                                            e
                                        }).map_or_else(|_| BTreeMap::new(), |v| v);

                                if let (Some(Ok(true)), Some(rcon_psw), Some(Ok(rcon_port))) = {
                                    let lock = self.server_properties_buffer.lock().await;
                                    (
                                        lock.get("enable-rcon").map(|v| v.parse()),
                                        lock.get("rcon.password").cloned(),
                                        lock.get("rcon.port").map(|v| v.parse::<u16>()),
                                    )
                                } {
                                    let max_retry = 3;
                                    for i in 0..max_retry {
                                        let rcon =
                                            <rcon::Connection<tokio::net::TcpStream>>::builder()
                                                .enable_minecraft_quirks(true)
                                                .connect(
                                                    &format!("localhost:{}", rcon_port),
                                                    &rcon_psw,
                                                )
                                                .await
                                                .map_err(|e| {
                                                    warn!(
                                                    "Failed to connect to RCON: {}, retry {}/{}",
                                                    e, i, max_retry
                                                );
                                                    e
                                                });
                                        if let Ok(rcon) = rcon {
                                            info!("Connected to RCON");
                                            self.rcon_conn.lock().await.replace(rcon);
                                            break;
                                        }
                                        tokio::time::sleep(Duration::from_secs(2_u64.pow(i))).await;
                                    }
                                } else {
                                    warn!("RCON is not enabled or misconfigured, skipping");
                                    self.rcon_conn.lock().await.take();
                                }
                            }
                            if let Some(system_msg) = parse_system_msg(&line) {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                                        instance_uuid: uuid.clone(),
                                        instance_event_inner: InstanceEventInner::SystemMessage {
                                            message: line,
                                        },
                                        instance_name: name.clone(),
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::System,
                                });
                                if let Some(player_name) = parse_player_joined(&system_msg) {
                                    players_manager.lock().await.add_player(
                                        MinecraftPlayer {
                                            name: player_name.clone(),
                                            uuid: name_to_uuid(&player_name).await,
                                        },
                                        self.name().await,
                                    );
                                } else if let Some(player_name) = parse_player_left(&system_msg) {
                                    players_manager
                                        .lock()
                                        .await
                                        .remove_by_name(&player_name, self.name().await);
                                }
                            } else if let Some((player, msg)) = parse_player_msg(&line) {
                                let _ = event_broadcaster.send(Event {
                                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                                        instance_uuid: uuid.clone(),
                                        instance_event_inner: InstanceEventInner::PlayerMessage {
                                            player,
                                            player_message: msg,
                                        },
                                        instance_name: name.clone(),
                                    }),
                                    details: "".to_string(),
                                    snowflake: Snowflake::default(),
                                    caused_by: CausedBy::System,
                                });
                                if let Some(macro_instruction) = parse_macro_invocation(&line) {
                                    match macro_instruction {
                                        MacroInstruction::Abort(macro_id) => {
                                            let _ = macro_executor.abort_macro(&macro_id).await;
                                        }
                                        MacroInstruction::Spawn {
                                            player_name: _,
                                            macro_name,
                                            args,
                                        } => {
                                            debug!(
                                                "Invoking macro {} with args {:?}",
                                                macro_name, args
                                            );
                                            let _ = self
                                                .run_macro(
                                                    &macro_name,
                                                    args,
                                                    CausedBy::Unknown,
                                                    true,
                                                )
                                                .await
                                                .map_err(|e| {
                                                    warn!("Failed to run macro: {}", e);
                                                    e
                                                });
                                        }
                                    }
                                }
                            }
                        }
                        info!("Instance {} process shutdown", name);
                        self.state
                            .lock()
                            .await
                            .try_transition(
                                StateAction::InstanceStop,
                                Some(&|state| {
                                    self.event_broadcaster.send(Event {
                                        event_inner: EventInner::InstanceEvent(InstanceEvent {
                                            instance_name: self.config.name.clone(),
                                            instance_uuid: self.uuid.clone(),
                                            instance_event_inner:
                                                InstanceEventInner::StateTransition { to: state },
                                        }),
                                        snowflake: Snowflake::default(),
                                        details: "Instance stopping as server process exited"
                                            .to_string(),
                                        caused_by: cause_by.clone(),
                                    });
                                }),
                            )
                            .unwrap();
                        self.players_manager.lock().await.clear(name);
                    }
                });

                self.config.has_started = true;
                self.write_config_to_file().await?;
                let instance_uuid = self.uuid.clone();
                let mut rx = self.event_broadcaster.subscribe();

                if block {
                    while let Ok(event) = rx.recv().await {
                        if let EventInner::InstanceEvent(InstanceEvent {
                            instance_uuid: event_instance_uuid,
                            instance_event_inner: InstanceEventInner::StateTransition { to },
                            ..
                        }) = event.event_inner
                        {
                            if instance_uuid == event_instance_uuid {
                                if to == State::Running {
                                    return Ok(()); // Instance started successfully
                                } else if to == State::Stopped {
                                    return Err(eyre!(
                                        "Instance exited unexpectedly before starting"
                                    )
                                    .into());
                                }
                            }
                        }
                    }
                    Err(eyre!("Sender shutdown").into())
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                error!("Failed to start server, {}", e);
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).unwrap();
                self.state
                    .lock()
                    .await
                    .try_transition(
                        StateAction::InstanceStop,
                        Some(&|state| {
                            self.event_broadcaster.send(Event {
                                event_inner: EventInner::InstanceEvent(InstanceEvent {
                                    instance_name: self.config.name.clone(),
                                    instance_uuid: self.uuid.clone(),
                                    instance_event_inner: InstanceEventInner::StateTransition {
                                        to: state,
                                    },
                                }),
                                snowflake: Snowflake::default(),
                                details: "Starting server".to_string(),
                                caused_by: cause_by.clone(),
                            });
                        }),
                    )
                    .unwrap();
                Err(e).context("Failed to start server")?;
                unreachable!();
            }
        }
    }
    async fn stop(&mut self, cause_by: CausedBy, block: bool) -> Result<(), Error> {
        self.state.lock().await.try_transition(
            StateAction::UserStop,
            Some(&|state| {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_name: self.config.name.clone(),
                        instance_uuid: self.uuid.clone(),
                        instance_event_inner: InstanceEventInner::StateTransition { to: state },
                    }),
                    snowflake: Snowflake::default(),
                    details: "Stopping server".to_string(),
                    caused_by: cause_by.clone(),
                });
            }),
        )?;
        let name = self.config.name.clone();
        let _uuid = self.uuid.clone();
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or_else(|| {
                error!("[{}] Failed to stop instance: stdin not available", name);
                eyre!("Failed to stop instance: stdin not available")
            })?
            .write_all(b"stop\n")
            .await
            .context("Failed to write to stdin")
            .map_err(|e| {
                error!("[{}] Failed to stop instance: {}", name, e);
                e
            })?;
        self.rcon_conn.lock().await.take();
        let mut rx = self.event_broadcaster.subscribe();
        let instance_uuid = self.uuid.clone();

        if block {
            while let Ok(event) = rx.recv().await {
                if let EventInner::InstanceEvent(InstanceEvent {
                    instance_uuid: event_instance_uuid,
                    instance_event_inner: InstanceEventInner::StateTransition { to },
                    ..
                }) = event.event_inner
                {
                    if instance_uuid == event_instance_uuid && to == State::Stopped {
                        return Ok(());
                    }
                }
            }
            Err(eyre!("Sender shutdown").into())
        } else {
            Ok(())
        }
    }

    async fn restart(&mut self, caused_by: CausedBy, block: bool) -> Result<(), Error> {
        if block {
            self.stop(caused_by.clone(), block).await?;
            self.start(caused_by, block).await
        } else {
            self.state
                .lock()
                .await
                .try_new_state(StateAction::UserStop, None)?;

            let mut __self = self.clone();
            tokio::task::spawn(async move {
                self.stop(caused_by.clone(), true).await.unwrap();
                self.start(caused_by, block).await.unwrap()
            });
            Ok(())
        }
    }

    async fn kill(&mut self, _cause_by: CausedBy) -> Result<(), Error> {
        if self.state().await == State::Stopped {
            warn!("[{}] Instance is already stopped", self.config.name.clone());
            return Err(eyre!("Instance is already stopped").into());
        }
        self.process
            .lock()
            .await
            .as_mut()
            .ok_or_else(|| {
                error!(
                    "[{}] Failed to kill instance: process not available",
                    self.config.name.clone()
                );
                eyre!("Failed to kill instance: process not available")
            })?
            .kill()
            .await
            .context("Failed to kill process")
            .map_err(|e| {
                error!(
                    "[{}] Failed to kill instance: {}",
                    self.config.name.clone(),
                    e
                );
                e
            })?;
        Ok(())
    }

    async fn state(&self) -> State {
        *self.state.lock().await
    }

    async fn send_command(&self, command: &str, cause_by: CausedBy) -> Result<(), Error> {
        if self.state().await == State::Stopped {
            Err(eyre!("Instance is stopped").into())
        } else {
            match self.stdin.lock().await.as_mut() {
                Some(stdin) => match {
                    if command == "stop" {
                        self.state.lock().await.try_new_state(
                            StateAction::UserStop,
                            Some(&|state| {
                                self.event_broadcaster.send(Event {
                                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                                        instance_name: self.config.name.clone(),
                                        instance_uuid: self.uuid.clone(),
                                        instance_event_inner: InstanceEventInner::StateTransition {
                                            to: state,
                                        },
                                    }),
                                    snowflake: Snowflake::default(),
                                    details: "Starting server".to_string(),
                                    caused_by: cause_by.clone(),
                                });
                            }),
                        )?;
                    }
                    stdin.write_all(format!("{}\n", command).as_bytes()).await
                } {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        warn!(
                            "[{}] Failed to send command to instance: {}",
                            self.config.name.clone(),
                            e
                        );
                        Err(e).context("Failed to send command to instance")?;
                        unreachable!()
                    }
                },
                None => {
                    let err_msg =
                        "Failed to write to stdin because stdin is None. Please report this bug.";
                    error!("[{}] {}", self.config.name.clone(), err_msg);
                    Err(eyre!(err_msg).into())
                }
            }
        }
    }
    async fn monitor(&self) -> MonitorReport {
        let mut sys = self.system.lock().await;
        sys.refresh_memory();
        if let Some(pid) = self.process.lock().await.as_ref().and_then(|p| p.id()) {
            sys.refresh_process(Pid::from_u32(pid));
            let proc = (*sys).process(Pid::from_u32(pid));
            if let Some(proc) = proc {
                let cpu_usage =
                    sys.process(Pid::from_u32(pid)).unwrap().cpu_usage() / sys.cpus().len() as f32;

                let memory_usage = proc.memory();
                let disk_usage = proc.disk_usage();
                let start_time = proc.start_time();
                MonitorReport {
                    memory_usage: Some(memory_usage),
                    disk_usage: Some(disk_usage.into()),
                    cpu_usage: Some(cpu_usage),
                    start_time: Some(start_time),
                }
            } else {
                MonitorReport::default()
            }
        } else {
            MonitorReport::default()
        }
    }
}
