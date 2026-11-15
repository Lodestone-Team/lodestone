//! Keeps the Host's core running: starts it, waits until it answers, restarts it when
//! it exits on its own, and stops it when the Host quits.
//!
//! A core that keeps exiting soon after it starts (a port taken by another program, a
//! data folder another core has locked) is given up after [`MAX_QUICK_EXITS`] tries,
//! and the window shows why, with the end of its log.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gpui::{AppContext as _, Context, EventEmitter, Task};
use launcher_core::cores::CoreClient;

use crate::core_process::{self, CoreConfig, CoreProcess};

/// How long a core may take to answer after it starts.
const READY_TIMEOUT: Duration = Duration::from_secs(90);
/// A core that exits sooner than this after answering counts as a quick exit.
const QUICK_EXIT: Duration = Duration::from_secs(60);
/// After this many quick exits in a row, the Host stops trying.
pub const MAX_QUICK_EXITS: u32 = 4;
const POLL: Duration = Duration::from_millis(250);

/// What the core is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum CoreStatus {
    /// Started, not answering yet.
    Starting,
    /// Answering at its address on this computer.
    Running {
        config: CoreConfig,
    },
    /// Exited on its own; starting again after a pause.
    Restarting {
        reason: String,
        attempt: u32,
    },
    /// Gave up: why, and the end of the core's log.
    Failed {
        reason: String,
        log: Vec<String>,
    },
    /// Stopping, because the Host is quitting.
    Stopping,
    Stopped,
}

/// What the window hears from the supervisor.
#[derive(Clone, Debug)]
pub enum SupervisorEvent {
    /// The status changed.
    Status,
    /// A (new) core answers: sign in to it.
    Ready(CoreConfig),
}

pub struct Supervisor {
    pub status: CoreStatus,
    process: Arc<Mutex<Option<CoreProcess>>>,
    /// The core's log, for the error page.
    pub log_file: std::path::PathBuf,
    stopping: bool,
    /// Stopping to start again ([`Supervisor::restart`]); quitting cancels it.
    restarting: bool,
    _task: Option<Task<()>>,
}

impl EventEmitter<SupervisorEvent> for Supervisor {}

impl Supervisor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            status: CoreStatus::Starting,
            process: Arc::new(Mutex::new(None)),
            log_file: core_process::host_dir().join("logs").join("core.log"),
            stopping: false,
            restarting: false,
            _task: None,
        };
        this.start(cx);
        this
    }

    /// Starts (or, after a failure, retries) the core.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.stopping = false;
        self.set_status(CoreStatus::Starting, cx);
        self._task = Some(cx.spawn(async move |this, cx| {
            let mut quick_exits = 0;
            loop {
                let config = match cx.background_spawn(async { prepare() }).await {
                    Ok(config) => config,
                    Err(reason) => {
                        this.update(cx, |this, cx| this.fail(reason, cx)).ok();
                        return;
                    }
                };
                let Ok(log_file) = this.update(cx, |this, _| this.log_file.clone()) else {
                    return;
                };
                let config = CoreConfig { log_file, ..config };
                let process = match CoreProcess::spawn(&config) {
                    Ok(process) => process,
                    Err(err) => {
                        let reason = format!("Lodestone Core couldn't start: {err}");
                        this.update(cx, |this, cx| this.fail(reason, cx)).ok();
                        return;
                    }
                };
                let Ok(shared) = this.update(cx, |this, _| {
                    *this.process.lock().unwrap_or_else(|e| e.into_inner()) = Some(process);
                    this.process.clone()
                }) else {
                    return;
                };

                // wait until it answers, or exits
                let client = launcher_core::http::default_client()
                    .map(|http| CoreClient::new(http, config.local_url()));
                let started = Instant::now();
                let mut ready = false;
                while started.elapsed() < READY_TIMEOUT {
                    if exited(&shared).is_some() {
                        break;
                    }
                    if let Ok(client) = &client
                        && client.info().await.is_ok()
                    {
                        ready = true;
                        break;
                    }
                    cx.background_executor().timer(POLL).await;
                }
                if ready {
                    let config = config.clone();
                    if this
                        .update(cx, |this, cx| {
                            this.set_status(
                                CoreStatus::Running {
                                    config: config.clone(),
                                },
                                cx,
                            );
                            cx.emit(SupervisorEvent::Ready(config));
                        })
                        .is_err()
                    {
                        return;
                    }
                }

                // watch it until it exits
                let answered = Instant::now();
                let status = loop {
                    if let Some(status) = exited(&shared) {
                        break Some(status);
                    }
                    if !ready {
                        // never answered in time
                        break None;
                    }
                    cx.background_executor().timer(POLL).await;
                };
                let stopping = this.update(cx, |this, _| this.stopping).unwrap_or(true);
                if stopping {
                    return;
                }
                if status.is_none() {
                    // it hung: stop it before trying again
                    let shared = shared.clone();
                    cx.background_spawn(async move {
                        if let Some(process) =
                            shared.lock().unwrap_or_else(|e| e.into_inner()).as_mut()
                        {
                            process.stop(Duration::from_secs(5));
                        }
                    })
                    .await;
                }
                let reason = match status {
                    Some(status) => format!("Lodestone Core stopped ({status})."),
                    None => "Lodestone Core didn't answer in time.".to_string(),
                };
                if !ready || answered.elapsed() < QUICK_EXIT {
                    quick_exits += 1;
                } else {
                    quick_exits = 0;
                }
                if quick_exits >= MAX_QUICK_EXITS {
                    this.update(cx, |this, cx| this.fail(reason, cx)).ok();
                    return;
                }
                let attempt = quick_exits;
                if this
                    .update(cx, |this, cx| {
                        this.set_status(CoreStatus::Restarting { reason, attempt }, cx)
                    })
                    .is_err()
                {
                    return;
                }
                // 1, 2, 4… seconds
                let pause = Duration::from_secs(1 << quick_exits.min(5));
                cx.background_executor().timer(pause).await;
            }
        }));
    }

    fn fail(&mut self, reason: String, cx: &mut Context<Self>) {
        let log = core_process::log_tail(&self.log_file, 12);
        self.set_status(CoreStatus::Failed { reason, log }, cx);
    }

    fn set_status(&mut self, status: CoreStatus, cx: &mut Context<Self>) {
        self.status = status;
        cx.emit(SupervisorEvent::Status);
        cx.notify();
    }

    /// The running core's settings.
    pub fn config(&self) -> Option<&CoreConfig> {
        match &self.status {
            CoreStatus::Running { config } => Some(config),
            _ => None,
        }
    }

    /// Stops the core (its servers first) and starts it again, with the settings as
    /// they are now: after the Connect service changed, say.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        let stopped = self.shutdown(cx);
        self.restarting = true;
        cx.spawn(async move |this, cx| {
            stopped.await;
            this.update(cx, |this, cx| {
                if std::mem::take(&mut this.restarting) {
                    this.start(cx)
                }
            })
            .ok();
        })
        .detach();
    }

    /// Stops the core: its servers stop first. Resolves once it has exited (or was
    /// killed after [`core_process::STOP_TIMEOUT`]).
    pub fn shutdown(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.stopping = true;
        self.restarting = false;
        self._task = None;
        self.set_status(CoreStatus::Stopping, cx);
        let process = self.process.clone();
        let stop = cx.background_spawn(async move {
            let taken = process.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(mut process) = taken {
                process.stop(core_process::STOP_TIMEOUT);
            }
        });
        cx.spawn(async move |this, cx| {
            stop.await;
            this.update(cx, |this, cx| this.set_status(CoreStatus::Stopped, cx))
                .ok();
        })
    }
}

/// The core's settings, before it starts: its program, folder and port.
fn prepare() -> Result<CoreConfig, String> {
    let program = core_process::find_core()?;
    let port = core_process::pick_port(core_process::port_is_free)?;
    Ok(CoreConfig {
        program,
        data_dir: core_process::data_dir(),
        port,
        log_file: Default::default(),
        connect_service: CoreConfig::connect_service_now(),
    })
}

/// The core's exit status, once it has exited.
fn exited(process: &Mutex<Option<CoreProcess>>) -> Option<std::process::ExitStatus> {
    process
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(CoreProcess::try_wait)
}
