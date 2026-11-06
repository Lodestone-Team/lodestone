use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::Ordering;
use std::time::Duration;

use color_eyre::eyre::{eyre, Context};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};

use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::line_parser::{
    parse_player_joined, parse_player_left, parse_player_msg, parse_server_started,
    parse_system_msg, PlayerMessage,
};
use crate::implementations::minecraft::player::MinecraftPlayer;
use crate::implementations::minecraft::util::name_to_uuid;
use crate::macro_executor::{DefaultWorkerOptionGenerator, SpawnResult};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_macro::TaskEntry;
use crate::traits::t_server::{MonitorReport, State, StateAction, TServer};

use crate::types::Snowflake;
use crate::util::{dont_spawn_terminal, list_dir, local_port_available};

use super::properties::PropertiesDocument;
use super::r#macro::resolve_macro_invocation;
use super::{java_binary, Flavour, ForgeBuildVersion, MinecraftInstance, RestoreConfig};
use tracing::{error, info, warn};

/// Minecraft's default port, used when server.properties has no `server-port`
const DEFAULT_PORT: u16 = 25565;

/// What start() reads from server.properties
struct LaunchSettings {
    port: u16,
    server_ip: String,
    online_mode: bool,
    rcon: Option<(String, u16)>,
}

impl LaunchSettings {
    fn from_properties(doc: &PropertiesDocument) -> Result<Self, Error> {
        let flag = |key: &str, default: bool| {
            doc.get(key).map_or(default, |v| {
                let v = v.trim();
                if default {
                    !v.eq_ignore_ascii_case("false")
                } else {
                    v.eq_ignore_ascii_case("true")
                }
            })
        };
        let port = match doc.get("server-port").map(str::trim) {
            None | Some("") => DEFAULT_PORT,
            Some(raw) => raw
                .parse::<u16>()
                .ok()
                .filter(|p| *p != 0)
                .ok_or_else(|| Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(
                        "server-port in server.properties is \"{raw}\", which is not a valid port"
                    ),
                })?,
        };
        let rcon_password = doc.get("rcon.password").unwrap_or("").to_string();
        let rcon_port = doc
            .get("rcon.port")
            .and_then(|p| p.trim().parse::<u16>().ok())
            .unwrap_or(25575);
        let rcon = (flag("enable-rcon", false) && !rcon_password.is_empty())
            .then_some((rcon_password, rcon_port));
        Ok(Self {
            port,
            server_ip: doc.get("server-ip").unwrap_or("").trim().to_string(),
            online_mode: flag("online-mode", true),
            rcon,
        })
    }
}

/// True if nothing is listening on the port. Checks the address the server binds to as
/// well as 127.0.0.1, because some systems allow binding a wildcard address next to a
/// more specific one. Another program can still take the port before the server starts.
fn port_available(server_ip: &str, port: u16) -> bool {
    let bind_ip = if server_ip.is_empty() {
        "0.0.0.0"
    } else {
        server_ip
    };
    local_port_available(port) && std::net::TcpListener::bind((bind_ip, port)).is_ok()
}

enum PlayerEvent {
    Joined(String),
    Left(String),
}

/// Reads lines from one stream into the channel until it ends. Runs in its own task, so a
/// partially read line is never dropped.
async fn forward_lines(
    stream: impl AsyncRead + Unpin,
    is_stdout: bool,
    tx: mpsc::Sender<(String, bool)>,
    name: String,
) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf).into_owned();
                if tx.send((line, is_stdout)).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                error!("[{}] Failed to read from stdout/stderr: {}", name, e);
                break;
            }
        }
    }
}

/// The first `.jar` file in `dir` whose name starts with `prefix`
async fn find_jar(dir: &Path, prefix: &str) -> Result<PathBuf, Error> {
    let files = list_dir(dir, Some(false))
        .await
        .context(format!("Failed to find {prefix}*.jar"))?;
    files
        .into_iter()
        .find(|p| {
            p.extension().unwrap_or_default() == "jar"
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(prefix))
        })
        .ok_or_else(|| eyre!("Failed to find {prefix}*.jar").into())
}

impl MinecraftInstance {
    fn state_event(&self, name: &str, state: State, caused_by: &CausedBy, details: &str) -> Event {
        Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_name: name.to_string(),
                instance_uuid: self.uuid.clone(),
                instance_event_inner: InstanceEventInner::StateTransition { to: state },
            }),
            snowflake: Snowflake::default(),
            details: details.to_string(),
            caused_by: caused_by.clone(),
        }
    }

    /// Applies a state machine action and broadcasts the new state. Returns the old state.
    async fn transition(
        &self,
        action: StateAction,
        caused_by: &CausedBy,
        details: &str,
    ) -> Result<State, Error> {
        let name = self.name().await;
        let mut state = self.state.lock().await;
        let previous = *state;
        state.try_transition(
            action,
            Some(&|new_state| {
                self.event_broadcaster
                    .send(self.state_event(&name, new_state, caused_by, details))
            }),
        )?;
        Ok(previous)
    }

    /// Sets the state to `new_state` if it is currently one of `expected`, and broadcasts
    /// it. Used to roll back failed starts and stops without overwriting a newer state.
    async fn set_state_if(
        &self,
        expected: &[State],
        new_state: State,
        caused_by: &CausedBy,
        details: &str,
    ) -> bool {
        let name = self.name().await;
        let mut state = self.state.lock().await;
        if !expected.contains(&*state) {
            return false;
        }
        *state = new_state;
        self.event_broadcaster
            .send(self.state_event(&name, new_state, caused_by, details));
        true
    }

    /// Waits for a state transition to `target`. Returns an error if the instance stops
    /// first (when `target` is not Stopped).
    async fn wait_for_state(
        &self,
        mut rx: broadcast::Receiver<Event>,
        target: State,
    ) -> Result<(), Error> {
        let check = |state: State| -> Option<Result<(), Error>> {
            if state == target {
                Some(Ok(()))
            } else if state == State::Stopped {
                Some(Err(
                    eyre!("Instance exited unexpectedly before starting").into()
                ))
            } else {
                None
            }
        };
        loop {
            match rx.recv().await {
                Ok(Event {
                    event_inner:
                        EventInner::InstanceEvent(InstanceEvent {
                            instance_uuid,
                            instance_event_inner: InstanceEventInner::StateTransition { to },
                            ..
                        }),
                    ..
                }) if instance_uuid == self.uuid => {
                    if let Some(result) = check(to) {
                        return result;
                    }
                }
                Ok(_) => {}
                // Missed some events: look at the current state instead.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    if let Some(result) = check(self.state().await) {
                        return result;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(eyre!("Event channel closed").into())
                }
            }
        }
    }

    async fn run_prelaunch(&self, name: &str) {
        // Macros live in the macros folder; older setups kept prelaunch in the instance root.
        let prelaunch = match resolve_macro_invocation(&self.path_to_macros, "prelaunch").await {
            Some(p) => Some(p),
            None => resolve_macro_invocation(&self.path_to_instance, "prelaunch").await,
        };
        let Some(prelaunch) = prelaunch else {
            info!("[{}] No prelaunch script found, skipping", name);
            return;
        };
        if !self.macro_executor.code_execution_enabled() {
            // the server still starts, only the script is skipped
            warn!(
                "[{}] Skipping the prelaunch script {}: code execution is disabled on this core",
                name,
                prelaunch.display()
            );
            self.event_broadcaster.send(Event::new_system_message(
                self.uuid.clone(),
                name.to_string(),
                "Skipped the prelaunch script: code execution is disabled on this core".to_string(),
            ));
            return;
        }
        let res: Result<SpawnResult, Error> = self
            .macro_executor
            .spawn(
                prelaunch,
                Vec::new(),
                CausedBy::System,
                Box::new(DefaultWorkerOptionGenerator),
                None,
                None,
                Some(self.uuid.clone()),
            )
            .await;
        match res {
            Ok(SpawnResult {
                macro_pid: pid,
                exit_future,
                detach_future,
            }) => {
                self.pid_to_task_entry.lock().await.insert(
                    pid,
                    TaskEntry {
                        pid,
                        name: "prelaunch".to_string(),
                        creation_time: chrono::Utc::now().timestamp(),
                    },
                );
                self.prune_task_history().await;
                tokio::select! {
                    _ = exit_future => {
                        info!("Prelaunch script exited");
                    }
                    _ = detach_future => {
                        info!("Prelaunch script requested detach");
                    }
                }
            }
            Err(e) => warn!("[{}] Failed to run the prelaunch script: {}", name, e),
        }
    }

    /// Adds the arguments that select the server jar.
    async fn add_jar_args(
        &self,
        command: &mut Command,
        config: &RestoreConfig,
    ) -> Result<(), Error> {
        let Flavour::Forge { build_version } = &config.flavour else {
            command
                .arg("-jar")
                .arg(self.path_to_instance.join("server.jar"));
            return Ok(());
        };
        let ForgeBuildVersion(build_version) = build_version
            .as_ref()
            .ok_or_else(|| eyre!("Forge version not found"))?;
        let major_version: i32 = config
            .version
            .split('.')
            .nth(1)
            .and_then(|v| v.split(['-', '_']).next())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| {
                eyre!(
                    "Unable to parse major Minecraft version for Forge from {}",
                    config.version
                )
            })?;

        if 17 <= major_version {
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
                    .join(build_version.as_str())
                    .join(forge_args)
                    .into_os_string()
                    .as_os_str(),
            );
            command.arg(full_forge_args);
        } else {
            // 1.7 to 1.16 name the jar forge-<version>-...; 1.5 and 1.6 use minecraftforge
            // (1.4 doesn't work since forge doesn't provide an installer)
            let prefix = if (7..=16).contains(&major_version) {
                format!("forge-{}-", config.version)
            } else {
                "minecraftforge".to_string()
            };
            let jar = find_jar(&self.path_to_instance, &prefix).await?;
            command.arg("-jar").arg(jar);
        }
        Ok(())
    }

    /// Everything start() does after moving to Starting. On error the caller rolls the
    /// state back.
    async fn launch(&self, name: &str, caused_by: &CausedBy) -> Result<(), Error> {
        let doc = self.load_properties().await?;
        let settings = LaunchSettings::from_properties(&doc)?;
        self.sync_port_from(&doc).await;
        if !port_available(&settings.server_ip, settings.port) {
            return Err(Error {
                kind: ErrorKind::Internal,
                source: eyre!("Port {} is already in use", settings.port),
            });
        }

        self.run_prelaunch(name).await;

        // The instance may have been stopped or killed (e.g. on shutdown) while the prelaunch
        // script ran; launching the server now would leave an untracked process behind.
        if self.state().await != State::Starting {
            warn!(
                "[{}] Instance is no longer starting after the prelaunch script, not launching the server",
                name
            );
            return Err(
                eyre!("Instance was stopped while the prelaunch script was running").into(),
            );
        }

        let config = self.config.lock().await.clone();
        let jre = config
            .java_cmd
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| java_binary(&self.path_to_runtimes, config.jre_major_version));

        let mut command = Command::new(&jre);
        command
            .arg(format!("-Xmx{}M", config.max_ram))
            .arg(format!("-Xms{}M", config.min_ram))
            .args(config.cmd_args.iter().filter(|s| !s.is_empty()));
        self.add_jar_args(&mut command, &config).await?;
        command.arg("nogui").current_dir(&self.path_to_instance);
        dont_spawn_terminal(&mut command)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn under the process lock and only while still Starting, so that a stop or
        // kill that ran meanwhile is not lost.
        let (stdout, stderr) = {
            let mut process = self.process.lock().await;
            if self.state().await != State::Starting {
                return Err(eyre!("Instance was stopped before the server process started").into());
            }
            let mut child = command
                .spawn()
                .context(format!("Failed to start the server with {}", jre.display()))?;
            let (Some(stdin), Some(stdout), Some(stderr)) =
                (child.stdin.take(), child.stdout.take(), child.stderr.take())
            else {
                let _ = child.start_kill();
                return Err(eyre!("Failed to take the stdio of the server process").into());
            };
            *self.stdin.lock().await = Some(stdin);
            *process = Some(child);
            (stdout, stderr)
        };

        tokio::task::spawn(self.clone().watch_output(
            stdout,
            stderr,
            caused_by.clone(),
            settings,
            name.to_string(),
        ));

        self.config.lock().await.has_started = true;
        if let Err(e) = self.write_config_to_file().await {
            warn!("[{}] Failed to save the instance config: {}", name, e);
        }
        Ok(())
    }

    /// Takes the child process and waits for its exit status.
    async fn reap_process(&self, name: &str) -> Option<ExitStatus> {
        let mut child = self.process.lock().await.take()?;
        match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
            Ok(Ok(status)) => Some(status),
            Ok(Err(e)) => {
                warn!(
                    "[{}] Failed to get the exit status of the server: {}",
                    name, e
                );
                None
            }
            Err(_) => {
                warn!(
                    "[{}] The server closed its output but did not exit, killing it",
                    name
                );
                let _ = child.kill().await;
                None
            }
        }
    }

    async fn connect_rcon(&self, password: String, port: u16, name: String) {
        let max_retry = 3;
        for i in 0..max_retry {
            let rcon = tokio::time::timeout(
                Duration::from_secs(10),
                <rcon::Connection<tokio::net::TcpStream>>::builder()
                    .enable_minecraft_quirks(true)
                    .connect(&format!("localhost:{}", port), &password),
            )
            .await;
            match rcon {
                Ok(Ok(rcon)) => {
                    if self.state().await == State::Running {
                        info!("[{}] Connected to RCON", name);
                        self.rcon_conn.lock().await.replace(rcon);
                    }
                    return;
                }
                Ok(Err(e)) => warn!(
                    "[{}] Failed to connect to RCON: {}, retry {}/{}",
                    name, e, i, max_retry
                ),
                Err(_) => warn!(
                    "[{}] Timed out connecting to RCON, retry {}/{}",
                    name, i, max_retry
                ),
            }
            tokio::time::sleep(Duration::from_secs(2_u64.pow(i))).await;
        }
    }

    /// Reads the server output until both streams end, then cleans up, and restarts the
    /// server if it crashed and restart on crash is on.
    async fn watch_output(
        self,
        stdout: impl AsyncRead + Unpin + Send + 'static,
        stderr: impl AsyncRead + Unpin + Send + 'static,
        caused_by: CausedBy,
        settings: LaunchSettings,
        name: String,
    ) {
        let (line_tx, mut line_rx) = mpsc::channel::<(String, bool)>(1024);
        tokio::task::spawn(forward_lines(stdout, true, line_tx.clone(), name.clone()));
        tokio::task::spawn(forward_lines(stderr, false, line_tx, name.clone()));

        // Player lookups call Mojang, so they run in their own task (in order) and never
        // block reading the output.
        let (player_tx, mut player_rx) = mpsc::unbounded_channel::<PlayerEvent>();
        let player_task = tokio::task::spawn({
            let this = self.clone();
            let online_mode = settings.online_mode;
            async move {
                while let Some(event) = player_rx.recv().await {
                    match event {
                        PlayerEvent::Joined(player_name) => {
                            // offline-mode names have no Mojang account
                            let uuid = if online_mode {
                                name_to_uuid(&player_name).await
                            } else {
                                None
                            };
                            let instance_name = this.name().await;
                            this.players_manager.lock().await.add_player(
                                MinecraftPlayer {
                                    name: player_name,
                                    uuid,
                                },
                                instance_name,
                            );
                        }
                        PlayerEvent::Left(player_name) => {
                            let instance_name = this.name().await;
                            this.players_manager
                                .lock()
                                .await
                                .remove_by_name(&player_name, instance_name);
                        }
                    }
                }
            }
        });

        let mut did_start = false;
        while let Some((line, is_stdout)) = line_rx.recv().await {
            if !is_stdout {
                warn!("[{}] {}", name, line.trim_end());
            }
            self.event_broadcaster.send(Event {
                event_inner: EventInner::InstanceEvent(InstanceEvent {
                    instance_uuid: self.uuid.clone(),
                    instance_event_inner: InstanceEventInner::InstanceOutput {
                        message: line.clone(),
                    },
                    instance_name: name.clone(),
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: CausedBy::System,
            });

            if !did_start && parse_server_started(&line) {
                did_start = true;
                // A stop requested during startup wins over the "Done" line.
                if self
                    .set_state_if(
                        &[State::Starting],
                        State::Running,
                        &caused_by,
                        "Server started",
                    )
                    .await
                {
                    info!("[{}] Instance started", name);
                    match settings.rcon.clone() {
                        Some((password, port)) => {
                            tokio::task::spawn({
                                let this = self.clone();
                                let name = name.clone();
                                async move { this.connect_rcon(password, port, name).await }
                            });
                        }
                        None => {
                            warn!("RCON is not enabled or misconfigured, skipping");
                            self.rcon_conn.lock().await.take();
                        }
                    }
                }
            }

            if let Some(system_msg) = parse_system_msg(&line) {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_uuid: self.uuid.clone(),
                        instance_event_inner: InstanceEventInner::SystemMessage { message: line },
                        instance_name: name.clone(),
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: CausedBy::System,
                });
                if let Some(player_name) = parse_player_joined(&system_msg) {
                    let _ = player_tx.send(PlayerEvent::Joined(player_name));
                } else if let Some(player_name) = parse_player_left(&system_msg) {
                    let _ = player_tx.send(PlayerEvent::Left(player_name));
                }
            } else if let Some(PlayerMessage { player, message }) = parse_player_msg(&line) {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::InstanceEvent(InstanceEvent {
                        instance_uuid: self.uuid.clone(),
                        instance_event_inner: InstanceEventInner::PlayerMessage {
                            player,
                            player_message: message,
                        },
                        instance_name: name.clone(),
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: CausedBy::System,
                });
            }
        }

        // Both streams have ended: the server is gone.
        info!("Instance {} process shutdown", name);
        drop(player_tx);
        let _ = player_task.await;
        let exit_status = self.reap_process(&name).await;
        self.stdin.lock().await.take();
        self.rcon_conn.lock().await.take();
        let stop_requested = self.stop_requested.swap(false, Ordering::SeqCst);
        if let Err(e) = self
            .transition(
                StateAction::InstanceStop,
                &caused_by,
                "Instance stopping as server process exited",
            )
            .await
        {
            error!("[{}] Failed to mark the instance as stopped: {}", name, e);
        }
        self.players_manager.lock().await.clear(name.clone());

        let crashed = !stop_requested && exit_status.is_some_and(|s| !s.success());
        if !crashed {
            return;
        }
        if let Some(status) = exit_status {
            warn!("[{}] The server exited unexpectedly ({})", name, status);
        }
        // Only restart servers that started successfully, so a broken setup doesn't loop.
        if did_start && self.config.lock().await.restart_on_crash {
            info!("[{}] Restarting the server because it crashed", name);
            tokio::time::sleep(Duration::from_secs(5)).await;
            if self.state().await == State::Stopped {
                if let Err(e) = self.start(CausedBy::System, false).await {
                    error!(
                        "[{}] Failed to restart the server after a crash: {}",
                        name, e
                    );
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl TServer for MinecraftInstance {
    async fn start(&self, cause_by: CausedBy, block: bool) -> Result<(), Error> {
        // Changing the version replaces server.jar, so don't start while it runs.
        let version_guard = self.version_change_lock.try_lock().map_err(|_| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Cannot start the instance while its version is being changed"),
        })?;
        // Subscribe first, so a server that exits right away can't be missed.
        let rx = block.then(|| self.event_broadcaster.subscribe());
        self.transition(StateAction::UserStart, &cause_by, "Starting server")
            .await?;
        drop(version_guard);
        self.stop_requested.store(false, Ordering::SeqCst);

        let name = self.name().await;
        if let Err(e) = self.launch(&name, &cause_by).await {
            error!("[{}] Failed to start server: {}", name, e);
            self.set_state_if(
                &[State::Starting],
                State::Stopped,
                &cause_by,
                "Failed to start server",
            )
            .await;
            return Err(e);
        }

        match rx {
            Some(rx) => self.wait_for_state(rx, State::Running).await,
            None => Ok(()),
        }
    }

    async fn stop(&self, cause_by: CausedBy, block: bool) -> Result<(), Error> {
        let rx = block.then(|| self.event_broadcaster.subscribe());
        let previous = self
            .transition(StateAction::UserStop, &cause_by, "Stopping server")
            .await?;
        self.stop_requested.store(true, Ordering::SeqCst);
        let name = self.name().await;

        let write_result = {
            let process = self.process.lock().await;
            if process.is_none() {
                // Still in the prelaunch phase: nothing runs yet, and start() won't launch
                // the server now that the state changed.
                self.set_state_if(
                    &[State::Stopping],
                    State::Stopped,
                    &cause_by,
                    "Server stopped",
                )
                .await;
                return Ok(());
            }
            match self.stdin.lock().await.as_mut() {
                Some(stdin) => match stdin.write_all(b"stop\n").await {
                    Ok(()) => stdin.flush().await,
                    Err(e) => Err(e),
                },
                None => Err(std::io::Error::other("stdin not available")),
            }
        };
        if let Err(e) = write_result {
            error!("[{}] Failed to stop instance: {}", name, e);
            self.stop_requested.store(false, Ordering::SeqCst);
            self.set_state_if(
                &[State::Stopping],
                previous,
                &cause_by,
                "Failed to stop server",
            )
            .await;
            return Err(color_eyre::Report::new(e)
                .wrap_err("Failed to write to stdin")
                .into());
        }
        self.rcon_conn.lock().await.take();

        match rx {
            Some(rx) => self.wait_for_state(rx, State::Stopped).await,
            None => Ok(()),
        }
    }

    async fn restart(&self, caused_by: CausedBy, block: bool) -> Result<(), Error> {
        if block {
            self.stop(caused_by.clone(), block).await?;
            self.start(caused_by, block).await
        } else {
            self.state
                .lock()
                .await
                .try_new_state(StateAction::UserStop, None)?;

            let this = self.clone();
            tokio::task::spawn(async move {
                let name = this.name().await;
                if let Err(e) = this.stop(caused_by.clone(), true).await {
                    error!("[{}] Failed to stop the server for a restart: {}", name, e);
                    return;
                }
                if let Err(e) = this.start(caused_by, false).await {
                    error!("[{}] Failed to start the server for a restart: {}", name, e);
                }
            });
            Ok(())
        }
    }

    async fn kill(&self, cause_by: CausedBy) -> Result<(), Error> {
        let name = self.name().await;

        if self.state().await == State::Stopped {
            warn!("[{}] Instance is already stopped", name);
            return Err(eyre!("Instance is already stopped").into());
        }
        self.stop_requested.store(true, Ordering::SeqCst);
        let mut process = self.process.lock().await;
        if let Some(process) = process.as_mut() {
            // The output task sees the process end and moves the state to Stopped.
            process
                .kill()
                .await
                .context("Failed to kill process")
                .map_err(|e| {
                    error!("[{}] Failed to kill instance: {}", name, e);
                    self.stop_requested.store(false, Ordering::SeqCst);
                    e
                })?;
        } else {
            error!(
                "[{}] Process not available, assuming instance is stopped",
                name
            );
            self.set_state_if(
                &[
                    State::Starting,
                    State::Running,
                    State::Stopping,
                    State::Error,
                ],
                State::Stopped,
                &cause_by,
                "Instance killed",
            )
            .await;
            Err(eyre!("Process not available, assuming instance is stopped"))?;
        }
        Ok(())
    }

    async fn state(&self) -> State {
        *self.state.lock().await
    }

    async fn send_command(&self, command: &str, cause_by: CausedBy) -> Result<(), Error> {
        let name = self.name().await;
        if self.state().await == State::Stopped {
            return Err(eyre!("Instance is stopped").into());
        }
        let mut stdin = self.stdin.lock().await;
        let Some(stdin) = stdin.as_mut() else {
            let err_msg = "Failed to write to stdin because stdin is None. Please report this bug.";
            error!("[{}] {}", name, err_msg);
            return Err(eyre!(err_msg).into());
        };
        let previous = if command.trim() == "stop" {
            let previous = self
                .transition(StateAction::UserStop, &cause_by, "Stopping server")
                .await?;
            self.stop_requested.store(true, Ordering::SeqCst);
            Some(previous)
        } else {
            None
        };
        let result = match stdin.write_all(format!("{}\n", command).as_bytes()).await {
            Ok(()) => stdin.flush().await,
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            warn!("[{}] Failed to send command to instance: {}", name, e);
            if let Some(previous) = previous {
                self.stop_requested.store(false, Ordering::SeqCst);
                self.set_state_if(
                    &[State::Stopping],
                    previous,
                    &cause_by,
                    "Failed to stop server",
                )
                .await;
            }
            return Err(color_eyre::Report::new(e)
                .wrap_err("Failed to send command to instance")
                .into());
        }
        Ok(())
    }

    async fn monitor(&self) -> MonitorReport {
        let mut sys = self.system.lock().await;
        sys.refresh_memory();
        if let Some(pid) = self.process.lock().await.as_ref().and_then(|p| p.id()) {
            // Same data the old `refresh_process` refreshed; dead processes are not removed.
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
                false,
                ProcessRefreshKind::nothing()
                    .with_memory()
                    .with_cpu()
                    .with_disk_usage(),
            );
            let cpu_count = sys.cpus().len().max(1) as f32;
            if let Some(proc) = sys.process(Pid::from_u32(pid)) {
                MonitorReport {
                    memory_usage: Some(proc.memory()),
                    disk_usage: Some(proc.disk_usage().into()),
                    cpu_usage: Some(proc.cpu_usage() / cpu_count),
                    start_time: Some(proc.start_time()),
                }
            } else {
                MonitorReport::default()
            }
        } else {
            MonitorReport::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(text: &str) -> Result<LaunchSettings, Error> {
        LaunchSettings::from_properties(&PropertiesDocument::parse(text).0)
    }

    #[test]
    fn launch_settings_from_properties() {
        let s = settings("server-port=25570\nonline-mode=false\nenable-rcon=true\nrcon.password=pw\nrcon.port=25580\n").unwrap();
        assert_eq!(s.port, 25570);
        assert!(!s.online_mode);
        assert_eq!(s.rcon, Some(("pw".to_string(), 25580)));

        let s = settings("").unwrap();
        assert_eq!(s.port, DEFAULT_PORT);
        assert!(s.online_mode);
        assert_eq!(s.rcon, None);

        // RCON without a password does not start
        let s = settings("enable-rcon=true\nrcon.password=\n").unwrap();
        assert_eq!(s.rcon, None);

        assert!(settings("server-port=70000\n").is_err());
        assert!(settings("server-port=abc\n").is_err());
    }
}
