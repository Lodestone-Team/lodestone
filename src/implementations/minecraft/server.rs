use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::events::{Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::util::read_properties_from_path;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State, TServer};

use crate::traits::{Error, ErrorInner, MaybeUnsupported, Supported};
use crate::util::rand_alphanumeric;

use super::Instance;
use log::{error, info, warn};
use tokio::task;

#[async_trait::async_trait]
impl TServer for Instance {
    async fn start(&mut self) -> Result<(), Error> {
        self.state.lock().await.update(State::Starting)?;
        let prelaunch = self.path().await.join("prelaunch.sh");
        if prelaunch.exists() {
            let _ = Command::new("bash")
                .arg(&self.path().await.join("prelaunch.sh"))
                .output()
                .await
                .map_err(|e| {
                    error!(
                        "[{}] Failed to run prelaunch script: {}",
                        self.config.name.clone(),
                        e.to_string()
                    );
                });
        } else {
            info!(
                "[{}] No prelaunch script found, skipping",
                self.config.name.clone()
            );
        }

        env::set_current_dir(&self.config.path).unwrap();

        let jre = if std::env::consts::OS == "macos" {
            self.path_to_runtimes
                .join("java")
                .join("Contents")
                .join("Home")
                .join("bin")
        } else {
            self.path_to_runtimes.join("java")
        }
        .join(format!("jre{}", self.config.jre_major_version))
        .join("bin")
        .join("java");
        match Command::new(&jre)
            .arg(format!("-Xmx{}M", self.config.max_ram))
            .arg(format!("-Xms{}M", self.config.min_ram))
            .args(&self.config.cmd_args)
            .arg("-jar")
            .arg(&self.config.path.join("server.jar"))
            .arg("nogui")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(mut proc) => {
                env::set_current_dir("../..").unwrap();
                proc.stdin.as_mut().ok_or_else(|| {
                    error!(
                        "[{}] Failed to take stdin during startup",
                        self.config.name.clone()
                    );
                    Error {
                        inner: ErrorInner::FailedToAcquireStdin,
                        detail: "Failed to take stdin during startup".to_string(),
                    }
                })?;
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

                        fn parse_server_started(system_msg: &str) -> bool {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r#"Done \(.+\)!"#).unwrap();
                            }
                            RE.is_match(system_msg).unwrap()
                        }

                        let mut did_start = false;

                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            info!("[{}] {}", name, line);
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
                            }
                        }
                        let _ = state.lock().await.update(State::Stopped);
                    }
                });
            }
            Err(_) => {
                env::set_current_dir("../..").unwrap();
            }
        }
        self.config.has_started = true;
        self.write_config_to_file().await?;
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), Error> {
        self.state.lock().await.update(State::Stopping)?;
        let name = self.config.name.clone();
        let uuid = self.config.uuid.clone();
        self.process
            .as_mut()
            .ok_or_else(|| {
                error!("[{}] Failed to stop instance: process not available", name);
                Error {
                    inner: ErrorInner::FailedToAcquireStdin,
                    detail: "Failed to stop instance: process not available".to_string(),
                }
            })?
            .stdin
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
                    "[{}] Failed to kill instance: stdin not open",
                    self.config.name.clone()
                );
                Error {
                    inner: ErrorInner::StdinNotOpen,
                    detail: "Failed to kill instance: stdin not open".to_string(),
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
            match self.process.as_mut() {
                Some(proc) => match {
                    if command == "stop" {
                        self.state
                            .lock()
                            .await
                            .update(State::Stopping)
                            .expect("Failed to update state")
                    }
                    proc.stdin
                        .as_mut()
                        .unwrap()
                        .write_all(format!("{}\n", command).as_bytes())
                        .await
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
}
