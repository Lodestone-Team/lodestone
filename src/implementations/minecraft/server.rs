use std::collections::HashSet;
use std::env;
use std::process::Stdio;
use std::time::Duration;

use futures::TryFutureExt;
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::util::read_properties_from_path;
use crate::macro_executor::ExecutionInstruction;
use crate::prelude::{get_snowflake, LODESTONE_PATH};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_macro::TMacro;
use crate::traits::t_server::{MonitorReport, State, TServer};

use crate::traits::{Error, ErrorInner};
use crate::util::dont_spawn_terminal;

use super::MinecraftInstance;
use log::{debug, error, info, warn};
use tokio::task;

#[async_trait::async_trait]
impl TServer for MinecraftInstance {
    async fn start(&mut self, cause_by: CausedBy) -> Result<(), Error> {
        self.state.lock().await.update(State::Starting, cause_by)?;
        env::set_current_dir(&self.config.path).unwrap();

        let prelaunch = self.path().await.join("prelaunch.js");
        if prelaunch.exists() {
            let uuid = self.macro_executor.spawn(ExecutionInstruction {
                name: "prelaunch".to_string(),
                args: vec![],
                executor: None,
                runtime: self.macro_std(),
                is_in_game: false,
            });
            let _ = self.macro_executor.wait_with_timeout(uuid, Some(5.0)).await;
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
                    use std::collections::HashSet;
                    let event_broadcaster = self.event_broadcaster.clone();
                    let settings = self.settings.clone();
                    let state = self.state.clone();
                    let path_to_properties = self.path_to_properties.clone();
                    let uuid = self.config.uuid.clone();
                    let name = self.config.name.clone();
                    let players = self.players.clone();
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
                                snowflake: get_snowflake(),
                                caused_by: CausedBy::System,
                            });

                            if parse_server_started(&line) && !did_start {
                                did_start = true;
                                state
                                    .lock()
                                    .await
                                    .update(State::Running, CausedBy::System)
                                    .expect("Failed to update state");
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
                                    snowflake: get_snowflake(),
                                    caused_by: CausedBy::System,
                                });
                                if let Some(player_name) = parse_player_joined(&system_msg) {
                                    let _ = players.lock().await.transform_cmp(
                                        Box::new(move |this: &mut HashSet<String>| {
                                            this.insert(player_name.clone());
                                            Ok(())
                                        }),
                                        CausedBy::System,
                                    );
                                } else if let Some(player_name) = parse_player_left(&system_msg) {
                                    let _ = players.lock().await.transform_cmp(
                                        Box::new(move |this: &mut HashSet<String>| {
                                            this.remove(&player_name);
                                            Ok(())
                                        }),
                                        CausedBy::System,
                                    );
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
                                    snowflake: get_snowflake(),
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
                        let _ = state.lock().await.transform(
                            Box::new(|v| {
                                *v = State::Stopped;
                                Ok(())
                            }),
                            CausedBy::Unknown,
                        );
                    }
                });
            }
            Err(e) => {
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).unwrap();
                error!("Failed to start server, {}", e);
                self.state
                    .lock()
                    .await
                    .transform(
                        Box::new(|v: &mut State| {
                            *v = State::Stopped;
                            Ok(())
                        }),
                        CausedBy::System,
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
        self.state.lock().await.update(State::Stopping, cause_by)?;
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
        self.players.lock().await.transform(
            Box::new(|v: &mut HashSet<String>| {
                v.clear();
                Ok(())
            }),
            CausedBy::System,
        )
    }
    async fn kill(&mut self, cause_by: CausedBy) -> Result<(), crate::traits::Error> {
        if self.state().await == State::Stopped {
            warn!("[{}] Instance is already stopped", self.config.name.clone());
            return Err(Error {
                inner: ErrorInner::InstanceStopped,
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
                    inner: ErrorInner::InstanceStopped,
                    detail: "Failed to kill instance, instance already existed".to_string(),
                }
            })?;
        self.state
            .lock()
            .await
            .transform(
                Box::new(|v: &mut State| {
                    *v = State::Stopped;
                    Ok(())
                }),
                cause_by,
            )
            .unwrap();
        self.players
            .lock()
            .await
            .transform(
                Box::new(|v: &mut HashSet<String>| {
                    v.clear();
                    Ok(())
                }),
                CausedBy::System,
            )
            .unwrap();
        Ok(())
    }

    async fn state(&self) -> State {
        self.state.lock().await.get()
    }

    async fn send_command(&self, command: &str, cause_by: CausedBy) -> Result<(), Error> {
        if self.state().await == State::Stopped {
            Err(Error {
                inner: ErrorInner::InstanceStopped,
                detail: "Instance not running".to_string(),
            })
        } else {
            match self.stdin.lock().await.as_mut() {
                Some(stdin) => match {
                    if command == "stop" {
                        self.state
                            .lock()
                            .await
                            .update(State::Stopping, cause_by)
                            .expect("Failed to update state")
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
        if let Some(pid) = self.process.lock().await.as_ref().and_then(|p| p.id()) {
            let mut sys = self.system.lock().await;
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
