use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::{env, thread};

use crate::events::{Event, EventInner};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State, TServer};

use crate::traits::{Error, ErrorInner, MaybeUnsupported};

use super::Instance;
use log::{error, info, warn};

impl TServer for Instance {
    fn start(&mut self) -> Result<(), Error> {
        self.state
            .write()
            .map_err(|e| {
                error!(
                    "[{}] Failed to aquired lock while getting state mutex: {}",
                    self.name(),
                    e
                );
                Error {
                    inner: ErrorInner::FailedToAquireLock,
                    detail: "Failed to aquired lock while getting state mutex".to_string(),
                }
            })?
            .update(State::Starting)?;
        let prelaunch = self.path().join("prelaunch.sh");
        if prelaunch.exists() {
            let _ = Command::new("bash")
                .arg(&self.path().join("prelaunch.sh"))
                .output()
                .map_err(|e| {
                    error!(
                        "[{}] Failed to run prelaunch script: {}",
                        self.name(),
                        e.to_string()
                    );
                    let _ = self.event_broadcaster.send(Event::new(
                        EventInner::InstanceError,
                        self.uuid(),
                        self.name(),
                        format!("Failed to run prelaunch script: {}", e),
                        None,
                    ));
                });
        } else {
            info!("[{}] No prelaunch script found, skipping", self.name());
        }

        env::set_current_dir(&self.config.path).unwrap();

        let jre = self
            .path_to_runtimes
            .join("java")
            .join(format!("jre{}", self.config.jre_major_version.unwrap()))
            .join("bin")
            .join("java");
        match Command::new(&jre)
            .arg(format!("-Xmx{}M", self.config.max_ram))
            .arg(format!("-Xms{}M", self.config.min_ram))
            .args(&self.config.jvm_args)
            .arg("-jar")
            .arg(&self.path().join("server.jar"))
            .arg("nogui")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
        {
            Ok(mut proc) => {
                env::set_current_dir("../..").unwrap();
                proc.stdin.as_mut().ok_or_else(|| {
                    error!("[{}] Failed to take stdin during startup", self.name());
                    let _ = self.event_broadcaster.send(Event::new(
                        EventInner::InstanceError,
                        self.uuid(),
                        self.name(),
                        "Failed to take stdin during startup".to_string(),
                        None,
                    ));
                    Error {
                        inner: ErrorInner::FailedToAquireStdin,
                        detail: "Failed to take stdin during startup".to_string(),
                    }
                })?;
                let stdout = proc.stdout.take().ok_or_else(|| {
                    error!("[{}] Failed to take stdout during startup", self.name());
                    let _ = self.event_broadcaster.send(Event::new(
                        EventInner::InstanceError,
                        self.uuid(),
                        self.name(),
                        "Failed to take stdout during startup".to_string(),
                        None,
                    ));
                    Error {
                        inner: ErrorInner::FailedToAquireStdout,
                        detail: "Failed to take stdout during startup".to_string(),
                    }
                })?;
                self.process = Some(proc);
                thread::spawn({
                    use fancy_regex::Regex;
                    use lazy_static::lazy_static;
                    use std::collections::HashSet;
                    let event_broadcaster = self.event_broadcaster.clone();
                    let state = self.state.clone();
                    let uuid = self.uuid();
                    let name = self.name();
                    let players = self.players.clone();
                    move || {
                        fn parse_system_msg(msg: &str) -> Option<String> {
                            lazy_static! {
                                static ref RE: Regex = Regex::new(r"\[.+\]+: (?!<)(.+)").unwrap();
                            }
                            if RE.is_match(msg).ok()? {
                                RE.captures(msg).ok()?.map(|caps| caps.get(1).unwrap().as_str().to_string())
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
                                static ref RE: Regex =
                                    Regex::new(r#"Done \(.+\)! For help, type "help""#).unwrap();
                            }
                            RE.is_match(system_msg).unwrap()
                        }

                        let mut did_start = false;

                        let reader = BufReader::new(stdout);
                        for line in reader.lines().filter(|x| x.is_ok()).map(|x| x.unwrap()) {
                            info!("[{}] {}", name, line);
                            let _ = event_broadcaster.send(Event::new(
                                EventInner::InstanceOutput(line.clone()),
                                uuid.clone(),
                                name.clone(),
                                "".to_string(),
                                None,
                            ));

                            if let Some(system_msg) = parse_system_msg(&line) {
                                if parse_server_started(&system_msg) && !did_start {
                                    did_start = true;
                                    let _ = state.write().map_err(|e| {
                                        let err_msg = "Failed to aquired lock while getting state mutex";
                                        error!(
                                            "[{}] : {} {}",
                                            name, err_msg, e
                                        );
                                        let _ = event_broadcaster.send(Event::new (
                                            EventInner::InstanceError,
                                            uuid.clone(),
                                            name.clone(),
                                            err_msg.to_string(),
                                            None,
                                    ));
                                        Error {
                                            inner: ErrorInner::FailedToAquireLock,
                                            detail: err_msg.to_string(),
                                        }
                                    }).map(|mut v| {v.update(State::Running).unwrap()});
                                }
                                let _ = event_broadcaster.send(Event::new(
                                    EventInner::SystemMessage(system_msg.to_owned()),
                                    uuid.clone(),
                                    name.clone(),
                                    "".to_string(),
                                    None,
                                ));
                                if let Some(player_name) = parse_player_joined(&system_msg) {
                                    let _ = players
                                        .write()
                                        .map_err(|e| {
                                            let err_msg =
                                                "Failed to aquired lock while getting state mutex";
                                            error!("[{}] : {} {}", name, err_msg, e);
                                            let _ = event_broadcaster.send(Event::new(
                                                EventInner::InstanceError,
                                                uuid.clone(),
                                                name.clone(),
                                                err_msg.to_string(),
                                                None,
                                            ));
                                            Error {
                                                inner: ErrorInner::FailedToAquireLock,
                                                detail: err_msg.to_string(),
                                            }
                                        })
                                        .map(|mut v| {
                                            v.transform_cmp(Box::new(
                                                move |this: &mut HashSet<String>| {
                                                    this.insert(player_name.clone());
                                                },
                                            ))
                                        });
                                } else if let Some(player_name) = parse_player_left(&system_msg) {
                                    let _ = players
                                        .write()
                                        .map_err(|e| {
                                            let err_msg =
                                                "Failed to aquired lock while getting state mutex";
                                            error!("[{}] : {} {}", name, err_msg, e);
                                            let _ = event_broadcaster.send(Event::new(
                                                EventInner::InstanceError,
                                                uuid.clone(),
                                                name.clone(),
                                                err_msg.to_string(),
                                                None,
                                            ));
                                            Error {
                                                inner: ErrorInner::FailedToAquireLock,
                                                detail: err_msg.to_string(),
                                            }
                                        })
                                        .map(|mut v| {
                                            v.transform_cmp(Box::new(
                                                move |this: &mut HashSet<String>| {
                                                    this.remove(&player_name);
                                                },
                                            ))
                                        });
                                }
                            } else if let Some((player, msg)) = parse_player_msg(&line) {
                                // debug!("[{}] Got a player message: <{}> {}", name, player, msg);
                                let _ = event_broadcaster.send(Event::new(
                                    EventInner::PlayerMessage(player, msg),
                                    uuid.clone(),
                                    name.clone(),
                                    "".to_string(),
                                    None,
                                ));
                            }
                        }
                        let _ = state
                            .write()
                            .map_err(|e| {
                                error!(
                                    "[{}] Failed to aquired lock while getting state mutex: {}",
                                    name, e
                                );
                                let _ = event_broadcaster.send(Event::new(
                                    EventInner::InstanceError,
                                    uuid.clone(),
                                    name.clone(),
                                    "Failed to aquired lock while getting state mutex".to_string(),
                                    None,
                                ));
                                Error {
                                    inner: ErrorInner::FailedToAquireLock,
                                    detail: "Failed to aquired lock while getting state mutex"
                                        .to_string(),
                                }
                            })
                            .unwrap()
                            .update(State::Stopped);
                    }
                });
            }
            Err(_) => {
                env::set_current_dir("../..").unwrap();
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), Error> {
        self.state
            .write()
            .map_err(|e| {
                error!(
                    "[{}] Failed to aquired lock while getting state mutex: {}",
                    self.name(),
                    e
                );
                Error {
                    inner: ErrorInner::FailedToAquireLock,
                    detail: "Failed to aquired lock while getting state mutex".to_string(),
                }
            })?
            .update(State::Stopping)?;
        let name = self.name();
        let uuid = self.uuid();
        self.process
            .as_mut()
            .ok_or_else(|| {
                error!("[{}] Failed to stop instance: process not available", name);
                let _ = self.event_broadcaster.send(Event::new(
                    EventInner::InstanceError,
                    uuid.clone(),
                    name.clone(),
                    "Failed to stop instance: process not available".to_string(),
                    None,
                ));
                Error {
                    inner: ErrorInner::FailedToAquireStdin,
                    detail: "Failed to stop instance: process not available".to_string(),
                }
            })?
            .stdin
            .as_mut()
            .ok_or_else(|| {
                error!("[{}] Failed to stop instance: stdin not available", name);
                let _ = self.event_broadcaster.send(Event::new(
                    EventInner::InstanceError,
                    uuid,
                    name,
                    "Failed to stop instance: stdin not available".to_string(),
                    None,
                ));
                Error {
                    inner: ErrorInner::FailedToAquireStdin,
                    detail: "Failed to stop instance: stdin not available".to_string(),
                }
            })?
            .write_all(b"stop\n")
            .map_err(|e| {
                error!(
                    "[{}] Failed to write to stdin: {}",
                    self.name(),
                    e.to_string()
                );
                let _ = self.event_broadcaster.send(Event::new(
                    EventInner::InstanceError,
                    self.uuid(),
                    self.name(),
                    format!("Failed to write to stdin: {}", e),
                    None,
                ));
                Error {
                    inner: ErrorInner::FailedToWriteStdin,
                    detail: format!("Failed to write to stdin: {}", e),
                }
            })?;
        Ok(())
    }
    fn kill(&mut self) -> Result<(), crate::traits::Error> {
        if self.state() == State::Stopped {
            warn!("[{}] Instance is already stopped", self.name());
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
                    self.config.name
                );
                Error {
                    inner: ErrorInner::StdinNotOpen,
                    detail: "Failed to kill instance: stdin not open".to_string(),
                }
            })?
            .kill()
            .map_err(|_| {
                error!(
                    "[{}] Failed to kill instance, instance already existed",
                    self.name()
                );
                let _ = self.event_broadcaster.send(Event::new(
                    EventInner::InstanceError,
                    self.uuid(),
                    self.name(),
                    "Failed to kill instance, instance already existed".to_string(),
                    None,
                ));
                Error {
                    inner: ErrorInner::InstanceStopped,
                    detail: "Failed to kill instance, instance already existed".to_string(),
                }
            })
    }

    fn state(&self) -> State {
        self.state.read().unwrap().get()
    }

    fn send_command(&mut self, command: &str) -> MaybeUnsupported<Result<(), Error>> {
        MaybeUnsupported::Supported(if self.state() == State::Stopped {
            Err(Error {
                inner: ErrorInner::InstanceStopped,
                detail: "Instance not running".to_string(),
            })
        } else {
            match self.process.as_mut() {
                Some(proc) => match proc
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(format!("{}\n", command).as_bytes())
                {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        let _ = self.event_broadcaster.send(Event::new(
                            EventInner::InstanceWarning,
                            self.uuid(),
                            self.name(),
                            format!("Failed to send command to instance: {}", e),
                            None,
                        ));
                        warn!(
                            "[{}] Failed to send command to instance: {}",
                            self.name(),
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
                    let _ = self.event_broadcaster.send(Event::new(
                        EventInner::InstanceError,
                        self.uuid(),
                        self.name(),
                        err_msg.to_string(),
                        None,
                    ));
                    error!("[{}] {}", self.name(), err_msg);
                    Err(Error {
                        inner: ErrorInner::StdinNotOpen,
                        detail: err_msg.to_string(),
                    })
                }
            }
        })
    }
}
