//! The app's instances: the list from `launcher-core`, the selected one, and each
//! instance's run (installing, playing) with its console log.

pub mod console;
pub mod new_instance;
pub mod settings;

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, SharedString, Task};
use launcher_core::{
    game::{
        crash::{self, CrashSummary, Outcome},
        download::{CancelToken, Downloader, ProgressSnapshot, Sleep},
        install::{InstallError, InstallProgress, Installer, JavaChoice, Stage},
        launch::{self, LaunchCommand},
        log4j::{self, Level, LogLine, Redactor, Stream},
        mojang::MojangUrls,
        platform::{self, Platform},
        process::{GameEvent, GameProcess},
    },
    instances::{Instance, InstanceStore},
    paths::DataDir,
};
use lodestone_minecraft::versions::mojang::MojangVersion;

use crate::accounts::Accounts;

/// The console keeps this many rows; older ones are dropped.
pub const MAX_LOG_ROWS: usize = 10_000;

/// One row of the console. A multi-line message or a stack trace is several rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRow {
    pub level: Level,
    pub time: Option<SharedString>,
    /// The thread, for rows that start an event.
    pub source: Option<SharedString>,
    pub text: SharedString,
    pub stream: Stream,
}

/// An instance's console.
#[derive(Debug, Default)]
pub struct GameLog {
    rows: VecDeque<LogRow>,
    /// The last events, for the crash summary.
    recent: VecDeque<LogLine>,
    /// Bumped on every change, so views know to scroll.
    pub revision: u64,
}

impl GameLog {
    pub fn rows(&self) -> &VecDeque<LogRow> {
        &self.rows
    }

    pub fn push(&mut self, line: LogLine) {
        let time = line
            .timestamp
            .map(|ms| SharedString::from(log4j::format_time(ms)));
        let mut first = true;
        let message_lines = line.message.lines().map(|text| (text, false));
        let throwable_lines = line
            .throwable
            .iter()
            .flat_map(|t| t.lines())
            .filter(|l| !l.trim().is_empty())
            .map(|text| (text, true));
        for (text, _) in message_lines.chain(throwable_lines) {
            self.rows.push_back(LogRow {
                level: line.level,
                time: if first { time.clone() } else { None },
                source: if first {
                    line.thread.clone().map(SharedString::from)
                } else {
                    None
                },
                text: SharedString::from(text.replace('\t', "    ")),
                stream: line.stream,
            });
            first = false;
        }
        while self.rows.len() > MAX_LOG_ROWS {
            self.rows.pop_front();
        }
        self.recent.push_back(line);
        while self.recent.len() > 500 {
            self.recent.pop_front();
        }
        self.revision += 1;
    }

    /// A line from the launcher itself.
    pub fn note(&mut self, level: Level, text: impl Into<String>) {
        let mut line = LogLine::plain(Stream::Launcher, &text.into());
        line.level = level;
        line.timestamp = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        );
        line.thread = Some("Launcher".into());
        self.push(line);
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.recent.clear();
        self.revision += 1;
    }

    /// The whole console as text, for "Copy log".
    pub fn to_text(&self, filter: impl Fn(&LogRow) -> bool) -> String {
        let mut text = String::new();
        for row in self.rows.iter().filter(|row| filter(row)) {
            if let Some(time) = &row.time {
                text.push_str(&format!("[{time}] "));
            }
            if let Some(source) = &row.source {
                text.push_str(&format!("[{source}/{}] ", row.level));
            }
            text.push_str(&row.text);
            text.push('\n');
        }
        text
    }

    pub fn recent(&self) -> Vec<LogLine> {
        self.recent.iter().cloned().collect()
    }
}

/// What an instance is doing.
pub enum RunState {
    Idle,
    /// Signing in, downloading and starting Java.
    Preparing {
        progress: Arc<InstallProgress>,
        cancel: CancelToken,
    },
    Running {
        process: Arc<GameProcess>,
        started: Instant,
    },
}

/// The Play button's state for the selected instance.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayState {
    NoInstance,
    Ready,
    Preparing {
        label: SharedString,
        detail: Option<SharedString>,
        /// 0 to 1, when the size is known.
        fraction: Option<f32>,
    },
    Playing,
}

struct Run {
    state: RunState,
    log: GameLog,
    /// The preparation or the output reader. Dropping it cancels it.
    task: Option<Task<()>>,
    /// Redraws the progress while preparing.
    ticker: Option<Task<()>>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            state: RunState::Idle,
            log: GameLog::default(),
            task: None,
            ticker: None,
        }
    }
}

/// Mojang's version list, for the New instance dialog.
#[derive(Clone, Debug, Default)]
pub enum Versions {
    #[default]
    NotLoaded,
    Loading,
    Ready(Arc<Vec<MojangVersion>>),
    Failed(SharedString),
}

/// What the crash dialog shows.
#[derive(Clone, Debug)]
pub struct CrashInfo {
    pub instance_name: String,
    pub summary: CrashSummary,
    pub exit_code: Option<i32>,
    pub report: Option<PathBuf>,
    pub log_text: String,
}

pub enum InstancesEvent {
    Crashed(CrashInfo),
    LaunchFailed {
        title: SharedString,
        message: SharedString,
    },
}

pub struct Instances {
    store: InstanceStore,
    data_dir: DataDir,
    installer: Installer,
    accounts: Entity<Accounts>,
    list: Vec<Instance>,
    selected: Option<String>,
    runs: HashMap<String, Run>,
    versions: Versions,
    total_memory: Option<u64>,
    machine: Platform,
    notice: Option<SharedString>,
    versions_task: Option<Task<()>>,
    /// Replaces the Play button's state, for the offscreen screenshots.
    preview_state: Option<PlayState>,
}

impl EventEmitter<InstancesEvent> for Instances {}

/// GPUI's timer as the download engine's [`Sleep`].
fn gpui_sleep(cx: &App) -> Sleep {
    let executor = cx.background_executor().clone();
    Arc::new(move |duration| {
        let timer = executor.timer(duration);
        Box::pin(timer)
    })
}

impl Instances {
    pub fn new(data_dir: DataDir, accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        Self::with_urls(data_dir, accounts, MojangUrls::production(), cx)
    }

    pub fn with_urls(
        data_dir: DataDir,
        accounts: Entity<Accounts>,
        urls: MojangUrls,
        cx: &mut Context<Self>,
    ) -> Self {
        let http = launcher_core::http::default_client().expect("could not start the HTTP client");
        let download_http =
            launcher_core::http::download_client().expect("could not start the HTTP client");
        let installer = Installer::new(
            http,
            Downloader::new(download_http, gpui_sleep(cx)),
            urls,
            data_dir.clone(),
        );
        let store = InstanceStore::new(data_dir.instances_dir());
        let mut this = Self {
            store,
            data_dir,
            installer,
            accounts,
            list: Vec::new(),
            selected: None,
            runs: HashMap::new(),
            versions: Versions::NotLoaded,
            total_memory: platform::total_memory(),
            machine: Platform::current(),
            notice: None,
            versions_task: None,
            preview_state: None,
        };
        this.reload(cx);
        this.selected = this.list.first().map(|i| i.id.clone());
        this
    }

    /// Reads the instance folders again.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let (list, broken) = self.store.list();
        self.list = list;
        self.notice = (!broken.is_empty()).then(|| {
            format!(
                "{} instance folder(s) couldn't be read, such as {}.",
                broken.len(),
                broken[0].dir.display()
            )
            .into()
        });
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.list.iter().any(|i| &i.id == id))
        {
            self.selected = self.list.first().map(|i| i.id.clone());
        }
        cx.notify();
    }

    pub fn list(&self) -> &[Instance] {
        &self.list
    }

    pub fn notice(&self) -> Option<&SharedString> {
        self.notice.as_ref()
    }

    pub fn total_memory(&self) -> Option<u64> {
        self.total_memory
    }

    pub fn selected(&self) -> Option<&Instance> {
        let id = self.selected.as_ref()?;
        self.list.iter().find(|i| &i.id == id)
    }

    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        self.selected = Some(id.to_string());
        cx.notify();
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.runs
            .get(id)
            .is_some_and(|run| !matches!(run.state, RunState::Idle))
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.runs
            .get(id)
            .is_some_and(|run| matches!(run.state, RunState::Running { .. }))
    }

    pub fn log(&self, id: &str) -> Option<&GameLog> {
        self.runs.get(id).map(|run| &run.log)
    }

    pub fn clear_log(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.get_mut(id) {
            run.log.clear();
            cx.notify();
        }
    }

    pub fn versions(&self) -> &Versions {
        &self.versions
    }

    /// Fetches Mojang's version list, unless it is loaded or loading.
    pub fn load_versions(&mut self, cx: &mut Context<Self>) {
        if matches!(self.versions, Versions::Loading | Versions::Ready(_)) {
            return;
        }
        self.versions = Versions::Loading;
        let installer = self.installer.clone();
        let fetch = cx.background_spawn(async move { installer.version_manifest().await });
        self.versions_task = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.versions = match result {
                    Ok(manifest) => Versions::Ready(Arc::new(manifest.versions)),
                    Err(err) => Versions::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// Shows a prepared version list, for the offscreen screenshots.
    pub fn set_versions(&mut self, versions: Versions, cx: &mut Context<Self>) {
        self.versions = versions;
        cx.notify();
    }

    pub fn create(
        &mut self,
        name: &str,
        version: &str,
        cx: &mut Context<Self>,
    ) -> Result<Instance, String> {
        let instance = self
            .store
            .create(name, version, self.total_memory)
            .map_err(|err| err.to_string())?;
        self.selected = Some(instance.id.clone());
        self.reload(cx);
        Ok(instance)
    }

    pub fn save(&mut self, instance: &Instance, cx: &mut Context<Self>) -> Result<(), String> {
        self.store.save(instance).map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }

    /// Moves the instance to the Trash.
    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if self.is_active(id) {
            return Err("Stop the game before deleting its instance.".into());
        }
        self.store.delete(id).map_err(|err| err.to_string())?;
        self.runs.remove(id);
        self.reload(cx);
        Ok(())
    }

    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.store.game_dir(id)
    }

    /// The Play button's state for the selected instance.
    pub fn play_state(&self) -> PlayState {
        if let Some(state) = &self.preview_state {
            return state.clone();
        }
        let Some(instance) = self.selected() else {
            return PlayState::NoInstance;
        };
        match self.runs.get(&instance.id).map(|run| &run.state) {
            None | Some(RunState::Idle) => PlayState::Ready,
            Some(RunState::Running { .. }) => PlayState::Playing,
            Some(RunState::Preparing { progress, .. }) => {
                let (stage, snapshot) = progress.snapshot();
                progress_state(stage, snapshot)
            }
        }
    }

    /// Shows a run in progress without starting one, for the offscreen screenshots.
    pub fn preview_preparing(&mut self, id: &str, progress: Arc<InstallProgress>) {
        let run = self.runs.entry(id.to_string()).or_default();
        run.state = RunState::Preparing {
            progress,
            cancel: CancelToken::new(),
        };
    }

    /// Shows the Play button in `state`, for the offscreen screenshots.
    pub fn preview_play_state(&mut self, state: PlayState) {
        self.preview_state = Some(state);
    }

    /// Adds lines to an instance's console, for the offscreen screenshots.
    pub fn preview_log(&mut self, id: &str, lines: impl IntoIterator<Item = LogLine>) {
        let run = self.runs.entry(id.to_string()).or_default();
        for line in lines {
            run.log.push(line);
        }
    }

    /// Installs what the instance needs and starts the game.
    pub fn play(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.is_active(id) {
            return;
        }
        let Ok(instance) = self.store.get(id) else {
            return;
        };
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let run = self.runs.entry(id.to_string()).or_default();
        run.log.note(
            Level::Info,
            format!("Preparing Minecraft {}", instance.game_version),
        );
        run.state = RunState::Preparing {
            progress: progress.clone(),
            cancel: cancel.clone(),
        };
        run.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
        cx.notify();

        let account = self
            .accounts
            .update(cx, |accounts, cx| accounts.launch_account(cx));
        let installer = self.installer.clone();
        let data = self.data_dir.clone();
        let machine = self.machine.clone();
        let game_dir = self.store.game_dir(id);
        let natives_dir = self.store.natives_dir(id);
        let id = id.to_string();
        let run_id = id.clone();
        let task = cx.spawn(async move |this, cx| {
            let account = match account.await {
                Ok(account) => account,
                Err(message) => {
                    this.update(cx, |this, cx| {
                        this.fail(&id, "Couldn't sign in".into(), message.into(), cx)
                    })
                    .ok();
                    return;
                }
            };
            let java = match &instance.java_override {
                Some(path) => JavaChoice::Override(path.clone()),
                None => JavaChoice::Mojang,
            };
            let settings = instance.game_settings();
            let version = instance.game_version.clone();
            let prepare_progress = progress.clone();
            let prepared = cx
                .background_spawn(async move {
                    std::fs::create_dir_all(&game_dir).map_err(|source| InstallError::Io {
                        path: game_dir.clone(),
                        source,
                    })?;
                    let prepared = installer
                        .prepare(
                            &version,
                            &machine,
                            &java,
                            &game_dir,
                            &prepare_progress,
                            &cancel,
                        )
                        .await?;
                    prepare_progress.set_stage(Stage::Natives);
                    prepared.extract_natives(&data, &natives_dir)?;
                    let paths = prepared.launch_paths(&data, &game_dir, &natives_dir)?;
                    let command = launch::build(
                        &prepared.version,
                        &prepared.java_platform,
                        &paths,
                        &account,
                        &settings,
                    )
                    .map_err(|err| InstallError::Invalid(err.to_string()))?;
                    Ok::<_, InstallError>((command, prepared.rosetta))
                })
                .await;
            this.update(cx, |this, cx| match prepared {
                Ok((command, rosetta)) => this.start(&id, command, rosetta, cx),
                Err(InstallError::Cancelled) => {
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(Level::Warn, "Cancelled");
                    }
                    this.set_idle(&id, cx);
                }
                Err(InstallError::RosettaMissing) => this.fail(
                    &id,
                    "Rosetta 2 is needed".into(),
                    platform::ROSETTA_MISSING.into(),
                    cx,
                ),
                Err(err) => this.fail(
                    &id,
                    "Couldn't start Minecraft".into(),
                    err.to_string().into(),
                    cx,
                ),
            })
            .ok();
        });
        if let Some(run) = self.runs.get_mut(&run_id) {
            run.task = Some(task);
        }
    }

    fn set_idle(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.get_mut(id) {
            run.state = RunState::Idle;
            run.task = None;
            run.ticker = None;
        }
        cx.notify();
    }

    fn fail(
        &mut self,
        id: &str,
        title: SharedString,
        message: SharedString,
        cx: &mut Context<Self>,
    ) {
        if let Some(run) = self.runs.get_mut(id) {
            run.log.note(Level::Error, format!("{title}: {message}"));
        }
        self.set_idle(id, cx);
        cx.emit(InstancesEvent::LaunchFailed { title, message });
    }

    fn start(&mut self, id: &str, command: LaunchCommand, rosetta: bool, cx: &mut Context<Self>) {
        let game_dir = self.store.game_dir(id);
        let before = crash::crash_reports(&game_dir);
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        if let RunState::Preparing { progress, .. } = &run.state {
            progress.set_stage(Stage::Starting);
        }
        if rosetta {
            run.log.note(
                Level::Info,
                "This version runs in an Intel (x86_64) Java under Rosetta 2.",
            );
        }
        // The command line is shown with the access token redacted.
        run.log
            .note(Level::Info, format!("Starting Java: {}", command.display()));
        let (process, events) = match GameProcess::spawn(&command, Redactor::new(command.secrets()))
        {
            Ok(spawned) => spawned,
            Err(err) => {
                self.fail(
                    id,
                    "Couldn't start Java".into(),
                    format!("{}: {err}", command.program.display()).into(),
                    cx,
                );
                return;
            }
        };
        let started = Instant::now();
        let started_unix = launcher_core::instances::now();
        run.state = RunState::Running {
            process: Arc::new(process),
            started,
        };
        run.ticker = None;
        let id = id.to_string();
        run.task = Some(cx.spawn(async move |this, cx| {
            let mut events = events.ready_chunks(256);
            while let Some(batch) = events.next().await {
                let mut exited = None;
                let alive = this.update(cx, |this, cx| {
                    if let Some(run) = this.runs.get_mut(&id) {
                        for event in batch {
                            match event {
                                GameEvent::Line(line) => run.log.push(line),
                                GameEvent::Exited { code, stopped } => {
                                    exited = Some((code, stopped))
                                }
                            }
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
                if let Some((code, stopped)) = exited {
                    this.update(cx, |this, cx| {
                        this.finished(&id, code, stopped, started, started_unix, &before, cx)
                    })
                    .ok();
                    return;
                }
            }
        }));
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn finished(
        &mut self,
        id: &str,
        code: Option<i32>,
        stopped: bool,
        started: Instant,
        started_unix: u64,
        before: &std::collections::BTreeSet<String>,
        cx: &mut Context<Self>,
    ) {
        let game_dir = self.store.game_dir(id);
        let report = crash::new_crash_report(&game_dir, before);
        let outcome = crash::outcome(code, stopped, report);
        if let Err(err) = self
            .store
            .record_play(id, started_unix, started.elapsed().as_secs())
        {
            self.notice = Some(format!("Couldn't save the play time: {err}").into());
        }
        let name = self
            .list
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        if let Some(run) = self.runs.get_mut(id) {
            let message = match (code, stopped) {
                (_, true) => "Stopped".to_string(),
                (Some(code), _) => format!("Minecraft exited with code {code}"),
                (None, _) => "Minecraft was ended by a signal".to_string(),
            };
            let level = if matches!(outcome, Outcome::Crashed { .. }) {
                Level::Error
            } else {
                Level::Info
            };
            run.log.note(level, message);
            if let Outcome::Crashed { exit_code, report } = outcome {
                let report_text = report
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok());
                let summary = crash::summarize(report_text.as_deref(), &run.log.recent());
                cx.emit(InstancesEvent::Crashed(CrashInfo {
                    instance_name: name,
                    summary,
                    exit_code,
                    report,
                    log_text: run.log.to_text(|_| true),
                }));
            }
        }
        self.set_idle(id, cx);
        self.reload(cx);
    }

    /// Cancels the preparation, or kills the game.
    pub fn stop(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        match &run.state {
            RunState::Idle => {}
            RunState::Preparing { cancel, .. } => {
                cancel.cancel();
                run.log.note(Level::Warn, "Cancelling…");
            }
            RunState::Running { process, .. } => {
                run.log.note(Level::Warn, "Stopping Minecraft");
                process.stop();
            }
        }
        cx.notify();
    }
}

/// Creates the instances entity for the app, in the same data directory as the
/// accounts.
pub fn create(accounts: &Entity<Accounts>, cx: &mut App) -> Entity<Instances> {
    let data_dir = accounts.read(cx).data_dir().clone();
    let accounts = accounts.clone();
    cx.new(|cx| Instances::new(data_dir, accounts, cx))
}

/// The progress label: "Downloading assets 1,234/4,567 · 210 MB/380 MB".
pub fn progress_state(stage: Stage, snapshot: ProgressSnapshot) -> PlayState {
    let downloading = matches!(stage, Stage::Java | Stage::Libraries | Stage::Assets);
    if !downloading || snapshot.files_total == 0 {
        return PlayState::Preparing {
            label: stage.label().into(),
            detail: None,
            fraction: None,
        };
    }
    let mb = |bytes: u64| (bytes as f64 / (1024.0 * 1024.0)).round() as u64;
    let detail = format!(
        "{}/{} · {} MB/{} MB",
        thousands(snapshot.files_done),
        thousands(snapshot.files_total),
        thousands(mb(snapshot.bytes_done)),
        thousands(mb(snapshot.bytes_total)),
    );
    let fraction = if snapshot.bytes_total > 0 {
        snapshot.bytes_done as f32 / snapshot.bytes_total as f32
    } else {
        snapshot.files_done as f32 / snapshot.files_total as f32
    };
    PlayState::Preparing {
        label: stage.label().into(),
        detail: Some(detail.into()),
        fraction: Some(fraction.clamp(0.0, 1.0)),
    }
}

/// `1234567` as `1,234,567`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "2 h 5 min", "12 min", "Never".
pub fn play_time(seconds: u64) -> String {
    match seconds {
        0 => "Not played yet".into(),
        1..60 => "Less than a minute".into(),
        60..3600 => format!("{} min", seconds / 60),
        _ => format!("{} h {} min", seconds / 3600, seconds / 60 % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_progress() {
        let state = progress_state(
            Stage::Assets,
            ProgressSnapshot {
                files_total: 4567,
                files_done: 1234,
                bytes_total: 380 * 1024 * 1024,
                bytes_done: 210 * 1024 * 1024,
            },
        );
        assert_eq!(
            state,
            PlayState::Preparing {
                label: "Downloading assets".into(),
                detail: Some("1,234/4,567 · 210 MB/380 MB".into()),
                fraction: Some(210.0 / 380.0),
            }
        );
        assert_eq!(
            progress_state(Stage::Starting, ProgressSnapshot::default()),
            PlayState::Preparing {
                label: "Starting Java".into(),
                detail: None,
                fraction: None
            }
        );
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(play_time(0), "Not played yet");
        assert_eq!(play_time(7_500), "2 h 5 min");
    }

    #[test]
    fn the_log_keeps_the_last_rows() {
        let mut log = GameLog::default();
        for n in 0..(MAX_LOG_ROWS + 5) {
            log.push(LogLine::plain(Stream::Stdout, &format!("line {n}")));
        }
        assert_eq!(log.rows().len(), MAX_LOG_ROWS);
        assert_eq!(log.rows().front().unwrap().text, "line 5");
        let mut event = LogLine::plain(Stream::Stdout, "two\nlines");
        event.throwable = Some("java.lang.Error\n\tat x\n".into());
        event.thread = Some("main".into());
        log.clear();
        log.push(event);
        let texts: Vec<_> = log.rows().iter().map(|r| r.text.to_string()).collect();
        assert_eq!(texts, ["two", "lines", "java.lang.Error", "    at x"]);
        assert_eq!(log.rows()[0].source.as_deref(), Some("main"));
        assert_eq!(log.rows()[1].source, None);
        assert_eq!(
            log.to_text(|_| true),
            "[main/INFO] two\nlines\njava.lang.Error\n    at x\n"
        );
    }
}
