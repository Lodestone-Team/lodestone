//! The app's instances: the list from `launcher-core`, the selected one, and each
//! instance's run (installing, playing) with its console log.

pub mod console;
pub mod content;
pub mod fields;
pub mod loader_picker;
pub mod modpack_browser;
pub mod modpacks;
pub mod new_instance;
pub mod settings;
pub mod shared_settings;
pub mod worlds;

use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, RenderImage, SharedString, Task};
use launcher_core::{
    content::{InstanceDirs, Modrinth, Target},
    export,
    game::{
        crash::{self, CrashSummary, Outcome},
        download::{CancelToken, Downloader, ProgressSnapshot, Sleep, Verify},
        install::{InstallError, InstallProgress, Installer, JavaChoice, Stage},
        launch::{self, LaunchCommand},
        loaders::{LoaderChoice, LoaderKind, LoaderVersion},
        log4j::{self, Level, LogLine, Redactor, Stream},
        mojang::MojangUrls,
        platform::{self, Platform},
        process::{GameEvent, GameProcess},
    },
    instances::{Instance, InstanceStore},
    paths::DataDir,
    settings::{self as launcher_settings, InstanceDefaults, Settings},
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

/// A mod loader's versions for one Minecraft version.
#[derive(Clone, Debug, Default)]
pub enum LoaderVersions {
    #[default]
    NotLoaded,
    Loading,
    Ready(Arc<Vec<LoaderVersion>>),
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
    /// Something to tell the user, such as the result of "Verify game files".
    Notice {
        title: SharedString,
        message: SharedString,
    },
    /// The instance's loader was changed outside its Settings tab.
    LoaderChanged(String),
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
    /// The launcher's settings: the global instance settings and the sidebar's
    /// folded groups.
    settings: Settings,
    /// Decoded custom icons, with the modification time they were read at.
    icons: HashMap<String, (Option<SystemTime>, Arc<RenderImage>)>,
    loader_versions: HashMap<(LoaderKind, String), LoaderVersions>,
    loader_tasks: HashMap<(LoaderKind, String), Task<()>>,
    /// Exports in progress.
    exports: Vec<Task<()>>,
    /// Replaces the Play button's state, for the offscreen screenshots.
    preview_state: Option<PlayState>,
    /// The Modrinth client the Content tab shares, with its cache. `None` keeps the
    /// offscreen screenshots offline.
    modrinth: Option<Arc<Modrinth>>,
    /// The downloader for content files.
    content_downloader: Downloader,
    /// "Switch to Fabric" in progress.
    loader_switch: Option<Task<()>>,
    /// Modpacks being made into instances.
    imports: Vec<modpacks::ImportJob>,
    next_import: u64,
    /// Modpack updates in progress, by instance.
    modpack_updates: HashMap<String, modpacks::UpdateJob>,
    /// Redraws while imports and updates run.
    import_ticker: Option<Task<()>>,
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
        let modrinth = Arc::new(Modrinth::production(http.clone()));
        let content_downloader = Downloader::new(download_http.clone(), gpui_sleep(cx));
        let installer = Installer::new(
            http,
            Downloader::new(download_http, gpui_sleep(cx)),
            urls,
            data_dir.clone(),
        );
        let store = InstanceStore::new(data_dir.instances_dir());
        let (settings, settings_notice) = match launcher_settings::load(&data_dir.settings_file()) {
            Ok(settings) => (settings, None),
            Err(err) => (
                Settings::default(),
                Some(SharedString::from(err.to_string())),
            ),
        };
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
            settings,
            icons: HashMap::new(),
            loader_versions: HashMap::new(),
            loader_tasks: HashMap::new(),
            exports: Vec::new(),
            preview_state: None,
            modrinth: Some(modrinth),
            content_downloader,
            loader_switch: None,
            imports: Vec::new(),
            next_import: 0,
            modpack_updates: HashMap::new(),
            import_ticker: None,
        };
        this.reload(cx);
        if settings_notice.is_some() {
            this.notice = settings_notice;
        }
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
        self.load_icons();
        cx.notify();
    }

    /// Decodes the custom icons that are new or changed since the last reload.
    fn load_icons(&mut self) {
        let mut icons = HashMap::new();
        for instance in &self.list {
            let Some(path) = self.store.icon_path(instance) else {
                continue;
            };
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            match self.icons.remove(&instance.id) {
                Some((time, image)) if time == modified => {
                    icons.insert(instance.id.clone(), (time, image));
                }
                _ => {
                    if let Ok(image) = image::open(&path) {
                        let image = crate::images::render_image(image.to_rgba8());
                        icons.insert(instance.id.clone(), (modified, image));
                    }
                }
            }
        }
        self.icons = icons;
    }

    /// The instance's custom icon, if it has one.
    pub fn icon(&self, id: &str) -> Option<Arc<RenderImage>> {
        self.icons.get(id).map(|(_, image)| image.clone())
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn defaults(&self) -> &InstanceDefaults {
        &self.settings.instance_defaults
    }

    /// Changes the settings file: read as it is now (the accounts save their own
    /// section of it), changed, and written back.
    fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let result = launcher_settings::update(&self.data_dir.settings_file(), change);
        cx.notify();
        match result {
            Ok(settings) => {
                self.settings = settings;
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// Saves the global instance settings.
    pub fn save_defaults(
        &mut self,
        defaults: InstanceDefaults,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.update_settings(|settings| settings.instance_defaults = defaults, cx)
    }

    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.settings.collapsed_groups.contains(group)
    }

    /// Folds or unfolds a sidebar group, and remembers it.
    pub fn toggle_group(&mut self, group: &str, cx: &mut Context<Self>) {
        let collapse = !self.settings.collapsed_groups.contains(group);
        let group = group.to_string();
        let result = self.update_settings(
            |settings| {
                if collapse {
                    settings.collapsed_groups.insert(group);
                } else {
                    settings.collapsed_groups.remove(&group);
                }
            },
            cx,
        );
        if let Err(err) = result {
            self.notice = Some(err.into());
        }
    }

    /// The group names in use, for suggestions.
    pub fn groups(&self) -> Vec<String> {
        launcher_core::instances::grouped(&self.list)
            .into_iter()
            .filter_map(|(name, _)| name)
            .collect()
    }

    pub fn loader_versions(&self, kind: LoaderKind, game_version: &str) -> LoaderVersions {
        self.loader_versions
            .get(&(kind, game_version.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// Fetches the loader's versions for `game_version`, unless they are loaded or
    /// loading.
    pub fn load_loader_versions(
        &mut self,
        kind: LoaderKind,
        game_version: &str,
        cx: &mut Context<Self>,
    ) {
        let key = (kind, game_version.to_string());
        if matches!(
            self.loader_versions.get(&key),
            Some(LoaderVersions::Loading | LoaderVersions::Ready(_))
        ) {
            return;
        }
        self.loader_versions
            .insert(key.clone(), LoaderVersions::Loading);
        let installer = self.installer.clone();
        let game_version = game_version.to_string();
        let fetch = cx
            .background_spawn(async move { installer.loader_versions(kind, &game_version).await });
        let task_key = key.clone();
        self.loader_tasks.insert(
            task_key,
            cx.spawn(async move |this, cx| {
                let result = fetch.await;
                this.update(cx, |this, cx| {
                    let versions = match result {
                        Ok(list) => LoaderVersions::Ready(Arc::new(list)),
                        Err(err) => LoaderVersions::Failed(err.to_string().into()),
                    };
                    this.loader_versions.insert(key.clone(), versions);
                    this.loader_tasks.remove(&key);
                    cx.notify();
                })
                .ok();
            }),
        );
        cx.notify();
    }

    /// Shows a prepared loader version list, for the offscreen screenshots.
    pub fn set_loader_versions(
        &mut self,
        kind: LoaderKind,
        game_version: &str,
        versions: LoaderVersions,
    ) {
        self.loader_versions
            .insert((kind, game_version.to_string()), versions);
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
        loader: Option<&LoaderChoice>,
        cx: &mut Context<Self>,
    ) -> Result<Instance, String> {
        let instance = self
            .store
            .create(name, version, loader)
            .map_err(|err| err.to_string())?;
        self.selected = Some(instance.id.clone());
        self.reload(cx);
        Ok(instance)
    }

    /// Copies the instance and selects the copy.
    pub fn duplicate(&mut self, id: &str, cx: &mut Context<Self>) -> Result<Instance, String> {
        let copy = self.store.duplicate(id).map_err(|err| err.to_string())?;
        self.selected = Some(copy.id.clone());
        self.reload(cx);
        Ok(copy)
    }

    pub fn set_icon(
        &mut self,
        id: &str,
        image: &Path,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .set_icon(id, image)
            .map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }

    pub fn clear_icon(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.store.clear_icon(id).map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }

    /// Writes the instance as a Modrinth modpack to `out`, in the background, and
    /// says how it went.
    pub fn export(&mut self, id: &str, out: PathBuf, cx: &mut Context<Self>) {
        let Ok(instance) = self.store.get(id) else {
            return;
        };
        let game_dir = self.store.game_dir(id);
        let instance_dir = self.store.dir(id);
        let write = cx.background_spawn(async move {
            // Files that came from Modrinth are listed by URL; the rest are copied.
            let on_modrinth = export::modrinth_files(&instance_dir);
            let plan = export::plan(&instance, &game_dir, &on_modrinth)?;
            export::write(&plan, &out)?;
            Ok::<_, export::ExportError>((plan, out))
        });
        self.exports.push(cx.spawn(async move |this, cx| {
            let result = write.await;
            this.update(cx, |_, cx| {
                cx.emit(match result {
                    Ok((plan, out)) => InstancesEvent::Notice {
                        title: "Modpack exported".into(),
                        message: format!(
                            "{} file(s) saved to {}.{}",
                            plan.overrides.len() + plan.index.files.len(),
                            out.display(),
                            if plan.skipped.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    " {} file(s) were left out, such as {}.",
                                    plan.skipped.len(),
                                    plan.skipped[0].0
                                )
                            }
                        )
                        .into(),
                    },
                    Err(err) => InstancesEvent::LaunchFailed {
                        title: "Couldn't export the modpack".into(),
                        message: err.to_string().into(),
                    },
                });
            })
            .ok();
        }));
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

    /// The Java major version `game_version` needs, if its version file is installed.
    pub fn required_java(&self, game_version: &str) -> Option<u32> {
        if game_version.is_empty()
            || game_version.contains(['/', '\\'])
            || game_version.starts_with('.')
        {
            return None;
        }
        let path = self
            .data_dir
            .versions_dir()
            .join(game_version)
            .join(format!("{game_version}.json"));
        let bytes = std::fs::read(path).ok()?;
        launcher_core::game::version::VersionJson::parse(&bytes)
            .ok()
            .map(|version| version.java_major())
    }

    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.store.game_dir(id)
    }

    /// The Modrinth client, unless the app is offline (the offscreen screenshots).
    pub fn modrinth(&self) -> Option<Arc<Modrinth>> {
        self.modrinth.clone()
    }

    /// Keeps the Content tab off the network, for the offscreen screenshots.
    pub fn preview_offline(&mut self) {
        self.modrinth = None;
    }

    pub fn content_downloader(&self) -> Downloader {
        self.content_downloader.clone()
    }

    pub fn content_dirs(&self, id: &str) -> InstanceDirs {
        InstanceDirs::new(self.store.dir(id))
    }

    /// What the instance accepts, without its shader mod (the Content tab adds it).
    pub fn content_target(&self, id: &str) -> Option<Target> {
        let instance = self.list.iter().find(|i| i.id == id)?;
        Some(Target::new(instance.game_version.clone(), instance.loader))
    }

    pub fn is_switching_loader(&self) -> bool {
        self.loader_switch.is_some()
    }

    /// The mod loader to offer a vanilla instance on `game_version`: Fabric, or Forge
    /// where Fabric doesn't run (Fabric starts at Minecraft 1.14), with the version to
    /// use. `None` when neither has a build for it.
    pub fn mod_loader_for(
        &self,
        game_version: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<(LoaderKind, LoaderVersion)>, InstallError>> {
        let installer = self.installer.clone();
        let game_version = game_version.to_string();
        cx.background_spawn(async move {
            for kind in [LoaderKind::Fabric, LoaderKind::Forge] {
                let list = installer.loader_versions(kind, &game_version).await?;
                let pick = list
                    .iter()
                    .find(|v| v.recommended)
                    .or_else(|| list.iter().find(|v| v.stable))
                    .or_else(|| list.first())
                    .cloned();
                if let Some(version) = pick {
                    return Ok(Some((kind, version)));
                }
            }
            Ok(None)
        })
    }

    /// Makes a vanilla instance a modded one, with the loader [`Self::mod_loader_for`]
    /// offers: the same change as picking that loader in its Settings tab. The loader
    /// installs the next time the game starts.
    pub fn switch_to_mod_loader(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.is_active(id) || self.loader_switch.is_some() {
            return;
        }
        let Ok(instance) = self.store.get(id) else {
            return;
        };
        let fetch = self.mod_loader_for(&instance.game_version, cx);
        let id = id.to_string();
        self.loader_switch = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.loader_switch = None;
                let failed = |message: String, cx: &mut Context<Self>| {
                    cx.emit(InstancesEvent::LaunchFailed {
                        title: "Couldn't switch to a mod loader".into(),
                        message: message.into(),
                    });
                    cx.notify();
                };
                let (kind, version) = match result {
                    Ok(Some(pick)) => pick,
                    Ok(None) => {
                        return failed(
                            format!(
                                "No mod loader Lodestone Launcher supports runs on Minecraft {}: Fabric starts at 1.14, and Forge has no build for it.",
                                instance.game_version
                            ),
                            cx,
                        );
                    }
                    Err(err) => return failed(err.to_string(), cx),
                };
                let mut changed = instance.clone();
                changed.loader = launcher_core::instances::Loader::from_kind(Some(kind));
                changed.loader_version = Some(version.id.clone());
                match this.save(&changed, cx) {
                    Ok(()) => {
                        cx.emit(InstancesEvent::LoaderChanged(id.clone()));
                        cx.emit(InstancesEvent::Notice {
                            title: format!("Switched to {}", kind.label()).into(),
                            message: format!(
                                "{} now uses {} {}. It installs the next time you press Play.",
                                changed.name,
                                kind.label(),
                                version.display
                            )
                            .into(),
                        });
                    }
                    Err(err) => failed(err, cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
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
        if self.modpack_updates.contains_key(id) {
            cx.emit(InstancesEvent::LaunchFailed {
                title: "The modpack is updating".into(),
                message: "Wait for the update to finish, then press Play.".into(),
            });
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
            format!(
                "Preparing Minecraft {} ({})",
                instance.game_version,
                instance.loader_label()
            ),
        );
        run.state = RunState::Preparing {
            progress: progress.clone(),
            cancel: cancel.clone(),
        };
        run.ticker = Some(ticker(id.to_string(), cx));
        cx.notify();

        let account = self
            .accounts
            .update(cx, |accounts, cx| accounts.launch_account(cx));
        let installer = self.installer.clone();
        let data = self.data_dir.clone();
        let machine = self.machine.clone();
        let game_dir = self.store.game_dir(id);
        let natives_dir = self.store.natives_dir(id);
        let settings = instance.game_settings(self.defaults(), self.total_memory);
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
            let version = instance.game_version.clone();
            let loader = instance.loader_choice();
            let prepare_progress = progress.clone();
            let check_started = Instant::now();
            let stats_progress = progress.clone();
            let prepared = cx
                .background_spawn(async move {
                    std::fs::create_dir_all(&game_dir).map_err(|source| InstallError::Io {
                        path: game_dir.clone(),
                        source,
                    })?;
                    let prepared = installer
                        .prepare(
                            &version,
                            loader.as_ref(),
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
                Ok((command, rosetta)) => {
                    this.take_notes(&id);
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(
                            Level::Info,
                            check_summary(&stats_progress, check_started.elapsed()),
                        );
                    }
                    this.start(&id, command, rosetta, cx)
                }
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

    /// "Verify game files": hashes every file of the instance's version, downloads
    /// what is missing or broken, and rewrites the install records. Nothing starts.
    pub fn verify(&mut self, id: &str, cx: &mut Context<Self>) {
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
            format!(
                "Verifying the game files of Minecraft {} ({})",
                instance.game_version,
                instance.loader_label()
            ),
        );
        run.state = RunState::Preparing {
            progress: progress.clone(),
            cancel: cancel.clone(),
        };
        run.ticker = Some(ticker(id.to_string(), cx));
        cx.notify();
        let installer = self.installer.clone();
        let machine = self.machine.clone();
        let game_dir = self.store.game_dir(id);
        let java = match &instance.java_override {
            Some(path) => JavaChoice::Override(path.clone()),
            None => JavaChoice::Mojang,
        };
        let id = id.to_string();
        let run_id = id.clone();
        let task = cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let verify_progress = progress.clone();
            let result = cx
                .background_spawn(async move {
                    installer
                        .prepare_with(
                            &instance.game_version,
                            instance.loader_choice().as_ref(),
                            &machine,
                            &java,
                            &game_dir,
                            Verify::Full,
                            &verify_progress,
                            &cancel,
                        )
                        .await
                })
                .await;
            this.update(cx, |this, cx| match result {
                Ok(_) => {
                    let summary = check_summary(&progress, started.elapsed());
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(Level::Info, summary.clone());
                    }
                    this.set_idle(&id, cx);
                    let (_, _, repaired) = progress.stats.counts();
                    cx.emit(InstancesEvent::Notice {
                        title: "Game files verified".into(),
                        message: if repaired == 0 {
                            format!("Every file was intact. {summary}").into()
                        } else {
                            format!("{repaired} missing or damaged file(s) were downloaded again. {summary}").into()
                        },
                    });
                }
                Err(InstallError::Cancelled) => {
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(Level::Warn, "Cancelled");
                    }
                    this.set_idle(&id, cx);
                }
                Err(err) => this.fail(
                    &id,
                    "Couldn't verify the game files".into(),
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

    /// Moves the installer's lines (the loader installer's output) into the console.
    fn take_notes(&mut self, id: &str) {
        if let Some(run) = self.runs.get_mut(id)
            && let RunState::Preparing { progress, .. } = &run.state
        {
            for line in progress.take_notes() {
                run.log.note(Level::Info, line);
            }
        }
    }

    fn set_idle(&mut self, id: &str, cx: &mut Context<Self>) {
        self.take_notes(id);
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
        self.merge_shared_settings(id);
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
        self.read_back_shared_settings(id);
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

/// Redraws the progress while an instance prepares, and moves the installer's lines
/// into its console.
fn ticker(id: String, cx: &mut Context<Instances>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let alive = this.update(cx, |this, cx| {
                this.take_notes(&id);
                cx.notify();
            });
            if alive.is_err() {
                break;
            }
        }
    })
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
    let downloading = matches!(
        stage,
        Stage::Java | Stage::Libraries | Stage::Loader | Stage::Assets
    );
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

/// "Checked 4,091 game files in 45 ms: 4,091 unchanged, 0 hashed, 0 downloaded."
fn check_summary(progress: &InstallProgress, elapsed: Duration) -> String {
    let (trusted, hashed, downloaded) = progress.stats.counts();
    format!(
        "Checked {} game files in {} ms: {} unchanged since the last check, {} hashed, {} downloaded.",
        thousands(trusted + hashed + downloaded),
        thousands(elapsed.as_millis() as u64),
        thousands(trusted),
        thousands(hashed),
        thousands(downloaded),
    )
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
