use std::env;
use std::process::Stdio;
use std::time::Duration;

use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::player::MinecraftPlayer;
use crate::implementations::minecraft::util::{name_to_uuid, read_properties_from_path};
use crate::macro_executor::ExecutionInstruction;
use crate::prelude::LODESTONE_PATH;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_macro::TMacro;
use crate::traits::t_server::{MonitorReport, State, StateAction, TServer};

use crate::traits::{Error, ErrorInner};
use crate::types::Snowflake;
use crate::util::dont_spawn_terminal;

use super::MinecraftInstance;
use log::{debug, error, info, warn};
use tokio::task;

#[async_trait::async_trait]
impl TServer for MinecraftInstance {
    async fn start(&mut self, cause_by: CausedBy) -> Result<(), Error> {
        self.state.lock().await.try_transition(
            StateAction::UserStart,
            Some(&|state| {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_name: self.config.name.clone(),
                        instance_uuid: self.config.uuid.clone(),
                        instance_event_inner: InstanceEventInner::StateTransition { to: state },
                    }),
                    snowflake: Snowflake::default(),
                    details: "Starting server".to_string(),
                    caused_by: cause_by.clone(),
                });
            }),
        )?;

        env::set_current_dir(&self.config.path).unwrap();

        let prelaunch = self.path().await.join("prelaunch.js");
        if prelaunch.exists() {
            self.macro_executor.spawn(ExecutionInstruction {
                name: "prelaunch".to_string(),
                args: vec![],
                executor: None,
                runtime: self.macro_std(),
                is_in_game: false,
                instance_uuid: self.config.uuid.clone(),
            });
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

        match dont_spawn_terminal(
            Command::new(&jre)
                .arg(format!("-Xmx{}M", self.config.max_ram))
                .arg(format!("-Xms{}M", self.config.min_ram))
                .args(
                    &self
                        .config
                        .cmd_args
                        .iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<&String>>(),
                )
                .arg("-jar")
                .arg(&self.config.path.join("server.jar"))
                .arg("nogui"),
        )
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        {
            Ok(mut proc) => {
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).unwrap();
                let stdin = proc.stdin.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stdin during startup",
                        self.config.name.clone()
                    );
                    Error {
                        inner: ErrorInner::FailedToAcquireStdin,
                        detail: "Failed to take stdin during startup".to_string(),
                    }
                })?;
                self.stdin.lock().await.replace(stdin);
                let stdout = proc.stdout.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stdout during startup",
                        self.config.name.clone()
                    );
                    Error {
                        inner: ErrorInner::FailedToAcquireStdout,
                        detail: "Failed to take stdout during startup".to_string(),
                    }
                })?;
                let stderr = proc.stderr.take().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stderr during startup",
                        self.config.name.clone()
                    );
                    Error {
                        inner: ErrorInner::FailedToAcquireStderr,
                        detail: "Failed to take stderr during startup".to_string(),
                    }
                })?;
                *self.process.lock().await = Some(proc);
                task::spawn({
                    use fancy_regex::Regex;
                    use lazy_static::lazy_static;

                    let event_broadcaster = self.event_broadcaster.clone();
                    let settings = self.settings.clone();
                    let _state = self.state.clone();
                    let path_to_properties = self.path_to_properties.clone();
                    let uuid = self.config.uuid.clone();
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
                                            dbg!(&args);

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
                                                        instance_uuid: self.config.uuid.clone(),
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
                                *settings.lock().await =
                                    read_properties_from_path(&path_to_properties)
                                        .await
                                        .expect("Failed to read properties");
                                if let (Ok(Ok(true)), Ok(rcon_psw), Ok(Ok(rcon_port))) = (
                                    self.get_field("enable-rcon").await.map(|v| v.parse()),
                                    self.get_field("rcon.password").await,
                                    self.get_field("rcon.port").await.map(|v| v.parse::<u32>()),
                                ) {
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
                                    warn!("RCON is not enabled, skipping");
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
                                            player_name,
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
                                                    Some(player_name.as_str()),
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
                                            instance_uuid: self.config.uuid.clone(),
                                            instance_event_inner:
                                                InstanceEventInner::StateTransition { to: state },
                                        }),
                                        snowflake: Snowflake::default(),
                                        details: "Starting server".to_string(),
                                        caused_by: cause_by.clone(),
                                    });
                                }),
                            )
                            .unwrap();
                        self.players_manager
                            .lock()
                            .await
                            .clear(name);
                    }
                });
            }
            Err(e) => {
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).unwrap();
                error!("Failed to start server, {}", e);
                self.state
                    .lock()
                    .await
                    .try_transition(
                        StateAction::InstanceStop,
                        Some(&|state| {
                            self.event_broadcaster.send(Event {
                                event_inner: EventInner::InstanceEvent(InstanceEvent {
                                    instance_name: self.config.name.clone(),
                                    instance_uuid: self.config.uuid.clone(),
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
                return Err(Error {
                    inner: ErrorInner::FailedToExecute,
                    detail: "Failed to start server".into(),
                });
            }
        }
        self.config.has_started = true;
        self.write_config_to_file().await?;
        Ok(())
    }
    async fn stop(&mut self, cause_by: CausedBy) -> Result<(), Error> {
        self.state.lock().await.try_transition(
            StateAction::UserStop,
            Some(&|state| {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_name: self.config.name.clone(),
                        instance_uuid: self.config.uuid.clone(),
                        instance_event_inner: InstanceEventInner::StateTransition { to: state },
                    }),
                    snowflake: Snowflake::default(),
                    details: "Starting server".to_string(),
                    caused_by: cause_by.clone(),
                });
            }),
        )?;
        let name = self.config.name.clone();
        let _uuid = self.config.uuid.clone();
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or_else(|| {
                error!("[{}] Failed to stop instance: stdin not available", name);
                Error {
                    inner: ErrorInner::FailedToAcquireStdin,
                    detail: "Failed to stop instance: stdin not available".to_string(),
                }
            })?
            .write_all(b"stop\n")
            .await
            .map_err(|e| {
                error!(
                    "[{}] Failed to write to stdin: {}",
                    self.config.name.clone(),
                    e.to_string()
                );
                Error {
                    inner: ErrorInner::FailedToWriteStdin,
                    detail: format!("Failed to write to stdin: {}", e),
                }
            })?;
        Ok(())
    }
    async fn kill(&mut self, _cause_by: CausedBy) -> Result<(), crate::traits::Error> {
        if self.state().await == State::Stopped {
            warn!("[{}] Instance is already stopped", self.config.name.clone());
            return Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Instance is already stopped".to_string(),
            });
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
                Error {
                    inner: ErrorInner::StdinNotOpen,
                    detail: "Failed to kill instance:  process not available".to_string(),
                }
            })?
            .kill()
            .await
            .map_err(|_| {
                error!(
                    "[{}] Failed to kill instance, instance already existed",
                    self.config.name.clone()
                );
                Error {
                    inner: ErrorInner::InvalidInstanceState,
                    detail: "Failed to kill instance, instance already existed".to_string(),
                }
            })?;
        Ok(())
    }

    async fn state(&self) -> State {
        *self.state.lock().await
    }

    async fn send_command(&self, command: &str, cause_by: CausedBy) -> Result<(), Error> {
        if self.state().await == State::Stopped {
            Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Instance not running".to_string(),
            })
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
                                        instance_uuid: self.config.uuid.clone(),
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
                        Err(Error {
                            inner: ErrorInner::FailedToWriteStdin,
                            detail: format!("Failed to write to stdin: {}", e),
                        })
                    }
                },
                None => {
                    let err_msg =
                        "Failed to write to stdin because stdin is None. Please report this bug.";
                    error!("[{}] {}", self.config.name.clone(), err_msg);
                    Err(Error {
                        inner: ErrorInner::StdinNotOpen,
                        detail: err_msg.to_string(),
                    })
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
