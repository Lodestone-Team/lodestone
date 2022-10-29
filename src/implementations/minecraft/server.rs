use std::env;
use std::process::Stdio;

use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::events::{Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::util::read_properties_from_path;
use crate::macro_executor::LuaExecutionInstruction;
use crate::prelude::LODESTONE_PATH;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{MonitorReport, State, TServer};

use crate::traits::{Error, ErrorInner, MaybeUnsupported, Supported};
use crate::util::{rand_alphanumeric, scoped_join_win_safe};

use super::Instance;
use log::{debug, error, info, warn};
use tokio::task;

#[async_trait::async_trait]
impl TServer for Instance {
    async fn start(&mut self) -> Result<(), Error> {
        self.state.lock().await.update(State::Starting)?;
        env::set_current_dir(&self.config.path).unwrap();

        let prelaunch = self.path().await.join("prelaunch.lua");
        if prelaunch.exists() {
            // read prelaunch.lua
            let prelaunch = tokio::fs::read_to_string(prelaunch).await.unwrap();
            // execute prelaunch.lua
            let uuid = self.macro_executor.spawn(LuaExecutionInstruction {
                lua: None,
                content: prelaunch,
                args: vec![],
                executor: None,
            });
            // wait for prelaunch.lua to finish
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
        match Command::new(&jre)
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
            .arg("nogui")
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
                self.process = Some(proc);
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
                    let path_to_macros = self.path_to_macros.clone();
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
                            Abort(String),
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
                                            cap.get(1)?.as_str().to_string(),
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
                                timestamp: chrono::Utc::now().timestamp(),
                                idempotency: rand_alphanumeric(5),
                            });

                            if parse_server_started(&line) && !did_start {
                                did_start = true;
                                state
                                    .lock()
                                    .await
                                    .update(State::Running)
                                    .expect("Failed to update state");
                                *settings.lock().await =
                                    read_properties_from_path(&path_to_properties)
                                        .await
                                        .expect("Failed to read properties");
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
                                    timestamp: chrono::Utc::now().timestamp(),
                                    idempotency: rand_alphanumeric(5),
                                });
                                if let Some(player_name) = parse_player_joined(&system_msg) {
                                    let _ = players.lock().await.transform_cmp(Box::new(
                                        move |this: &mut HashSet<String>| {
                                            this.insert(player_name.clone());
                                            Ok(())
                                        },
                                    ));
                                } else if let Some(player_name) = parse_player_left(&system_msg) {
                                    let _ = players.lock().await.transform_cmp(Box::new(
                                        move |this: &mut HashSet<String>| {
                                            this.remove(&player_name);
                                            Ok(())
                                        },
                                    ));
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
                                    timestamp: chrono::Utc::now().timestamp(),
                                    idempotency: rand_alphanumeric(5),
                                });
                                if let Some(macro_instruction) = parse_macro_invocation(&line) {
                                    match macro_instruction {
                                        MacroInstruction::Abort(macro_id) => {
                                            macro_executor.abort_macro(&macro_id).await;
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
                                            let path = scoped_join_win_safe(
                                                &path_to_macros.join("in_game"),
                                                &format!("{}.lua", macro_name),
                                            )
                                            .unwrap();
                                            if let Ok(content) =
                                                tokio::fs::read_to_string(&path).await
                                            {
                                                let exec = LuaExecutionInstruction {
                                                    content,
                                                    args,
                                                    executor: Some(player_name),
                                                    lua: None,
                                                };
                                                macro_executor.spawn(exec);
                                            } else {
                                                warn!("Failed to read macro file {:?}", path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let _ = state.lock().await.update(State::Stopped);
                    }
                });
            }
            Err(e) => {
                env::set_current_dir(LODESTONE_PATH.with(|v| v.clone())).unwrap();
                error!("Failed to start server, {}", e);
                self.state.lock().await.update(State::Stopped);
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
    async fn stop(&mut self) -> Result<(), Error> {
        self.state.lock().await.update(State::Stopping)?;
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
    async fn kill(&mut self) -> Result<(), crate::traits::Error> {
        if self.state().await == State::Stopped {
            warn!("[{}] Instance is already stopped", self.config.name.clone());
            return Err(Error {
                inner: ErrorInner::InstanceStopped,
                detail: "Instance is already stopped".to_string(),
            });
        }
        self.process
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
            })
    }

    async fn state(&self) -> State {
        self.state.lock().await.get()
    }

    async fn send_command(&mut self, command: &str) -> MaybeUnsupported<Result<(), Error>> {
        Supported(if self.state().await == State::Stopped {
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
                            .update(State::Stopping)
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
        })
    }
    async fn monitor(&self) -> MonitorReport {
        if let Some(pid) = self.process.as_ref().and_then(|p| p.id()) {
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
