//! The app's instances: the list from `launcher-core`, the selected one, and each
//! instance's run (installing, playing) with its console log.

pub mod add_modpack;
pub mod appearance;
mod auto_config;
pub mod console;
pub mod content;
pub mod fields;
mod files;
pub mod import;
pub mod loader_picker;
mod main_instance;
pub mod mod_finder;
pub mod modpack_browser;
pub mod modpacks;
mod narrator;
pub mod new_instance;
pub mod settings;
pub mod share_dialog;
pub(crate) mod shared_options_i18n;
pub mod shared_settings;
pub mod slideshow;
pub mod snapshots;
mod undo;
pub mod worlds;
pub use files::FileOp;
pub use undo::InstanceSnapshot;
// Read access for the control API (`crate::control`) and the performance sampler.
pub(crate) mod control;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, RenderImage, SharedString, Task};
use launcher_core::settings::AfterLaunch;
use launcher_core::{
    content::{CurseForge, InstanceDirs, Modrinth, Target},
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
use crate::components::LaunchStep;
use crate::i18n::{InstancesMsg as M, InstancesPlural as P, t, tr, trn};

/// The console keeps this many rows; older ones are dropped.
pub const MAX_LOG_ROWS: usize = 10_000;

/// The shortest time between two redraws for game output.
const LOG_BATCH_INTERVAL: Duration = Duration::from_millis(66);

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
    /// Rows pushed since the start, never reset: where a reader of new rows is.
    pushed: u64,
}

impl GameLog {
    pub fn rows(&self) -> &VecDeque<LogRow> {
        &self.rows
    }

    /// The id of the row at `index` in [`Self::rows`]: its number since the start,
    /// which stays as older rows are dropped (for the Logs tab's selection).
    pub fn row_id(&self, index: usize) -> u64 {
        self.pushed - self.rows.len() as u64 + index as u64
    }

    /// A log of `rows` as they are: a file read from disk (the Logs tab's past logs
    /// and crash reports).
    pub fn from_rows(rows: impl IntoIterator<Item = LogRow>) -> Self {
        let rows: VecDeque<LogRow> = rows.into_iter().collect();
        Self {
            pushed: rows.len() as u64,
            revision: 1,
            rows,
            recent: VecDeque::new(),
        }
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
            self.pushed += 1;
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

    /// The game's last line worth showing under "Launching Minecraft": its first
    /// line, without the launcher's own notes, warnings about the JVM or blank lines.
    pub fn latest_game_line(&self) -> Option<SharedString> {
        self.recent.iter().rev().take(40).find_map(|line| {
            if line.stream == Stream::Launcher || !matches!(line.level, Level::Info) {
                return None;
            }
            let text = line.message.lines().next()?.trim();
            (text.chars().filter(|c| c.is_alphabetic()).count() >= 3).then(|| {
                let short: String = text.chars().take(120).collect();
                SharedString::from(short)
            })
        })
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
        step: LaunchStep,
    },
    Playing,
}

struct Run {
    state: RunState,
    log: GameLog,
    /// Whether this run's game has finished loading (see [`crash::reached_menu`]).
    reached_menu: bool,
    /// Whether this run's mod loader refused the mods (see [`crash::mods_refused`]).
    mods_refused: bool,
    /// The preparation or the output reader. Dropping it cancels it.
    task: Option<Task<()>>,
    /// Redraws the progress while preparing.
    ticker: Option<Task<()>>,
    /// The game's window has opened ([`InstancesEvent::GameWindowOpened`] was sent).
    window_opened: bool,
    /// Sends [`InstancesEvent::GameWindowOpened`] after a while if the log never
    /// says the window opened.
    window_wait: Option<Task<()>>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            state: RunState::Idle,
            log: GameLog::default(),
            reached_menu: false,
            mods_refused: false,
            task: None,
            ticker: None,
            window_opened: false,
            window_wait: None,
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
    pub instance_id: String,
    pub instance_name: String,
    pub summary: CrashSummary,
    /// The mods the crash points at, strongest evidence first.
    pub suspects: Vec<crash::suspects::Suspect>,
    pub exit_code: Option<i32>,
    pub report: Option<PathBuf>,
    pub log_text: String,
    /// Whether it crashed before the game finished loading.
    pub at_startup: bool,
    /// Whether the mod loader refused the mods (one is missing what it needs).
    pub mods_refused: bool,
}

/// What a [`InstancesEvent::Notice`] or [`InstancesEvent::LaunchFailed`] is about, so
/// the control API's jobs end with their own outcome and not with a notice about
/// another instance that happened to come first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum About {
    /// Nothing in particular, such as a dropped file that isn't a modpack.
    #[default]
    Nothing,
    /// The instance with this id.
    Instance(String),
    /// The modpack import with this job id (its instance doesn't exist until it ends).
    Import(u64),
}

pub enum InstancesEvent {
    Crashed(CrashInfo),
    /// The game of the instance exited without crashing: closed, or stopped
    /// (`stopped`) from the launcher.
    Exited {
        id: String,
        stopped: bool,
    },
    /// The game of the instance has finished loading.
    ReachedMenu(String),
    /// The instance's mod loader refused its mods. Forge and NeoForge then wait on an
    /// error screen instead of exiting.
    ModsRefused(String),
    LaunchFailed {
        title: SharedString,
        message: SharedString,
        about: About,
    },
    /// Something to tell the user, such as the result of "Verify game files".
    Notice {
        title: SharedString,
        message: SharedString,
        about: About,
    },
    /// The instance's loader was changed outside its Settings tab.
    LoaderChanged(String),
    /// A `.lodesnap` file was dropped or picked: offer to make an instance from it.
    ImportSnapshot(std::path::PathBuf),
    /// A game's window opened: the log said so, or it has run for
    /// [`WINDOW_WAIT`] without saying. Sent once per run.
    GameWindowOpened(String),
    /// A game exited, after any [`InstancesEvent::Crashed`] for it.
    GameExited(String),
    /// The game of the instance opened a world to LAN on `port`: its log said
    /// `Started serving on <port>` (shared with friends when the person chose to).
    LanOpened {
        id: String,
        port: u16,
    },
    /// A new instance, with no main instance yet: offer to make it the main one.
    OfferMain(String),
    /// `id` is to become the main instance, but `current` is: ask first.
    ConfirmMain {
        id: String,
        current: String,
    },
}

/// How long a game runs before it counts as having opened its window when the log
/// never says so (an unusual version or a mod that changes the log).
const WINDOW_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

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
    /// Decodes the icons that are new or changed (see `files.rs`).
    icon_task: Option<Task<()>>,
    /// Instances whose files are being copied, moved to the Trash or given an image,
    /// off the window's thread (see `files.rs`).
    file_ops: HashMap<String, FileOp>,
    /// Instance images read for Undo, with the file's modification time and size when
    /// read: a change that leaves an image alone doesn't read it again.
    undo_images: HashMap<PathBuf, (SystemTime, u64, Arc<Vec<u8>>)>,
    /// Decoded Play backgrounds, and those being decoded.
    backgrounds: HashMap<String, appearance::BackgroundEntry>,
    background_tasks: HashMap<String, appearance::BackgroundTask>,
    /// Small copies of the instances' Play backgrounds, for a skin that shows them
    /// behind the sidebar's rows: by id, with the file's modification time.
    sidebar_banners: HashMap<
        String,
        (
            Option<std::time::SystemTime>,
            appearance::BackgroundSource,
            Arc<RenderImage>,
            launcher_core::ui_skin::readable::Backdrop,
        ),
    >,
    sidebar_banner_tasks: HashMap<String, Task<()>>,
    /// The instance list and skin the sidebar banners were last checked against
    /// ([`Self::load_sidebar_banners`] runs every frame; checking stats files and
    /// reads skins).
    sidebar_banners_checked: Option<(u64, u64)>,
    /// Goes up with each [`Self::reload`].
    list_generation: u64,
    /// The instances linked to a server, as the Lodestone Host page last read their
    /// links: the sidebar groups them under Servers (see [`Self::set_linked`]).
    linked: HashSet<String>,
    /// [`Self::required_java`]'s answers by game version: the instance Settings tab
    /// asks on every frame, and each answer reads and parses a version file. An
    /// installed version's answer never changes; a missing one is asked again after
    /// a reload (the reload generation it was found at).
    required_java: std::cell::RefCell<HashMap<String, (Option<u32>, u64)>>,
    /// Goes up when an instance's slideshow changes, so the Play tab lists it again.
    slideshow_revision: u64,
    /// The instance whose background is decoded for the Play tab, and the one before,
    /// kept for a quick switch back. No other background stays decoded.
    background_shown: Option<String>,
    background_previous: Option<String>,
    /// Display-size copies of the banner's images, in `<data>/cache/hero`.
    hero_cache: launcher_core::hero_cache::HeroCache,
    loader_versions: HashMap<(LoaderKind, String), LoaderVersions>,
    loader_tasks: HashMap<(LoaderKind, String), Task<()>>,
    /// Exports in progress.
    exports: Vec<Task<()>>,
    /// Replaces the Play button's state, for the offscreen screenshots.
    preview_state: Option<PlayState>,
    /// What Play switches the Play button to instead of playing, for the offscreen
    /// screenshots of its transition.
    preview_press: Option<PlayState>,
    /// The Modrinth client the Content tab shares, with its cache. `None` keeps the
    /// offscreen screenshots offline.
    modrinth: Option<Arc<Modrinth>>,
    /// The CurseForge client: `None` without Lodestone's API key (in builds made without
    /// it) or offline.
    curseforge: Option<Arc<CurseForge>>,
    /// The downloader for content files.
    content_downloader: Downloader,
    /// "Switch to Fabric" in progress.
    loader_switch: Option<Task<()>>,
    /// Modpacks being made into instances.
    imports: Vec<modpacks::ImportJob>,
    next_import: u64,
    /// Files of an imported CurseForge pack the user downloads from curseforge.com, by
    /// instance; the Content tab takes them when it shows that instance.
    /// Each instance's snapshots, newest first, once read (see snapshots.rs).
    snapshot_rows: HashMap<String, Vec<snapshots::SnapshotRow>>,
    snapshot_tasks: HashMap<String, Task<()>>,
    pending_downloads:
        HashMap<String, Vec<launcher_core::content::curseforge_install::CfPlannedFile>>,
    /// Modpack updates in progress, by instance.
    modpack_updates: HashMap<String, modpacks::UpdateJob>,
    /// Counts changes to an instance's content made outside its Content tab (a modpack
    /// added, removed or updated), so the tab reads its lists again.
    content_revisions: HashMap<String, u64>,
    /// Redraws while imports and updates run.
    import_ticker: Option<Task<()>>,
    /// Counts changes to the shared game settings made in the launcher's Settings, so
    /// an instance's Settings tab shows the new values.
    shared_revision: u64,
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
    /// Adds the decoded icons, Play backgrounds and sidebar banners, for the
    /// performance monitor.
    pub fn tally_images(&self, tally: &mut crate::perf::ImageTally) {
        for (_, image) in self.icons.values() {
            tally.add(image);
        }
        for (_, _, background) in self.backgrounds.values() {
            tally.add(&background.image);
        }
        for (_, _, image, _) in self.sidebar_banners.values() {
            tally.add(image);
        }
    }

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
        let curseforge = CurseForge::production(http.clone()).map(Arc::new);
        let content_downloader = Downloader::new(download_http.clone(), gpui_sleep(cx));
        let installer = Installer::new(
            http,
            Downloader::new(download_http, gpui_sleep(cx)),
            urls,
            data_dir.clone(),
        );
        let store = InstanceStore::new(data_dir.instances_dir());
        // Copies of images that are gone, and the oldest past the limit.
        let hero_cache = launcher_core::hero_cache::HeroCache::new(&data_dir);
        let pruning = hero_cache.clone();
        cx.background_spawn(async move { pruning.prune() }).detach();
        let (settings, settings_notice) = match launcher_settings::load(&data_dir.settings_file()) {
            Ok(settings) => (settings, None),
            Err(err) => (
                Settings::default(),
                Some(SharedString::from(err.to_string())),
            ),
        };
        crate::i18n::apply(settings.language);
        crate::theme::zoom::set(settings.zoom);
        crate::skin::apply(
            settings.skin.as_deref(),
            &launcher_core::ui_skin::SkinStore::new(&data_dir),
            cx,
        );
        crate::set_menus(cx);
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
            icon_task: None,
            file_ops: HashMap::new(),
            undo_images: HashMap::new(),
            backgrounds: HashMap::new(),
            background_tasks: HashMap::new(),
            sidebar_banners: HashMap::new(),
            sidebar_banner_tasks: HashMap::new(),
            sidebar_banners_checked: None,
            list_generation: 0,
            linked: HashSet::new(),
            required_java: Default::default(),
            slideshow_revision: 0,
            background_shown: None,
            background_previous: None,
            hero_cache,
            loader_versions: HashMap::new(),
            loader_tasks: HashMap::new(),
            exports: Vec::new(),
            preview_state: None,
            preview_press: None,
            shared_revision: 0,
            modrinth: Some(modrinth),
            curseforge,
            content_downloader,
            loader_switch: None,
            imports: Vec::new(),
            next_import: 0,
            pending_downloads: HashMap::new(),
            snapshot_rows: HashMap::new(),
            snapshot_tasks: HashMap::new(),
            modpack_updates: HashMap::new(),
            content_revisions: HashMap::new(),
            import_ticker: None,
        };
        this.reload(cx);
        if settings_notice.is_some() {
            this.notice = settings_notice;
        }
        this.selected = this.list.first().map(|i| i.id.clone());
        // Only the selected instance's background is decoded: now there is one.
        this.load_backgrounds(cx);
        this
    }

    /// Reads the instance folders again. After a change to one instance, its own
    /// `update_listed` or `refresh` (`files.rs`) reads less.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let (list, broken) = self.store.list();
        self.list = list;
        self.notice = (!broken.is_empty()).then(|| {
            trn!(
                P::BrokenFolders,
                broken.len(),
                path = broken[0].dir.display()
            )
            .into()
        });
        self.list_changed(cx);
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

    /// The Lodestone Host page's offer to download Lodestone Host was dismissed: it
    /// doesn't come back (Settings > Developer > "Reset first-time setup" shows it
    /// again).
    pub fn dismiss_host_offer(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.host_offer_dismissed = true, cx)
        {
            self.notice = Some(err.into());
        }
    }

    /// Friends page > "Share LAN worlds with friends".
    pub fn set_share_lan_worlds(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.share_lan_worlds = on, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Changes the settings file: read as it is now (the accounts save their own
    /// section of it), changed, and written back.
    fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let before = undo::LauncherValues::of(&self.settings);
        let result = launcher_settings::update(&self.data_dir.settings_file(), change);
        cx.notify();
        match result {
            Ok(settings) => {
                // The accounts save and trace the developer section themselves, so
                // this copy of it can be older than the file: compare it as it is now.
                let mut previous = self.settings.clone();
                previous.developer = settings.developer.clone();
                launcher_core::trace::settings_changed(
                    "launcher",
                    &previous,
                    &settings,
                    serde_json::Value::Null,
                );
                let cli_changed = settings.cli != self.settings.cli;
                self.settings = settings;
                self.record_settings(before, cx);
                if cli_changed {
                    // opens or closes the control socket; later, as it reads this
                    cx.defer(crate::control::apply);
                }
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

    /// Settings > Developer > Command line.
    pub fn cli_settings(&self) -> launcher_core::settings::CliSettings {
        self.settings.cli
    }

    /// Changes Settings > Developer > Command line. Only the window calls this: no
    /// control method does, so a client can't widen its own access.
    pub fn set_cli_settings(
        &mut self,
        cli: launcher_core::settings::CliSettings,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.update_settings(|settings| settings.cli = cli, cx) {
            self.notice = Some(err.into());
        }
    }

    /// "Group instances automatically".
    pub fn auto_groups(&self) -> bool {
        self.settings.auto_groups
    }

    pub fn set_auto_groups(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.auto_groups = on, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Settings > General > "Update automatically".
    pub fn auto_update(&self) -> bool {
        self.settings.auto_update
    }

    pub fn set_auto_update(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.auto_update = on, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Settings > General > "Beta builds".
    /// Settings > General > "When a game starts".
    pub fn after_launch(&self) -> AfterLaunch {
        self.settings.after_launch
    }

    pub fn set_after_launch(&mut self, after: AfterLaunch, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.after_launch = after, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Whether any instance is installing or starting (not yet running).
    pub fn any_preparing(&self) -> bool {
        self.runs
            .values()
            .any(|run| matches!(run.state, RunState::Preparing { .. }))
    }

    /// Whether any game is running.
    pub fn any_running(&self) -> bool {
        self.runs
            .values()
            .any(|run| matches!(run.state, RunState::Running { .. }))
    }

    pub fn update_channel(&self) -> launcher_core::update::UpdateChannel {
        self.settings.update_channel
    }

    pub fn set_update_channel(
        &mut self,
        channel: launcher_core::update::UpdateChannel,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.update_settings(|settings| settings.update_channel = channel, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Settings > General > Language.
    pub fn language(&self) -> crate::i18n::Language {
        self.settings.language
    }

    /// Changes the language, saves it, and redraws every window in it.
    pub fn set_language(&mut self, language: crate::i18n::Language, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.language = language, cx) {
            self.notice = Some(err.into());
        }
        crate::i18n::apply(language);
        crate::set_menus(cx);
        cx.refresh_windows();
    }

    /// Changes the interface's zoom (within the levels), saves it, and redraws.
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let zoom = crate::theme::zoom::set(zoom);
        if let Err(err) = self.update_settings(|settings| settings.zoom = zoom, cx) {
            self.notice = Some(err.into());
        }
        cx.refresh_windows();
    }

    /// The sidebar's width (logical pixels at 100% zoom) and whether it is folded
    /// into the rail of icons.
    pub fn sidebar(&self) -> (f32, bool) {
        (
            self.settings
                .sidebar_width
                .unwrap_or(crate::theme::layout::SIDEBAR_DEFAULT),
            self.settings.sidebar_collapsed,
        )
    }

    /// Saves the sidebar's width and fold, once a drag ends.
    pub fn set_sidebar(&mut self, width: f32, collapsed: bool, cx: &mut Context<Self>) {
        if (width, collapsed) == self.sidebar() {
            return;
        }
        let width = (width != crate::theme::layout::SIDEBAR_DEFAULT).then_some(width);
        if let Err(err) = self.update_settings(
            |settings| {
                settings.sidebar_width = width;
                settings.sidebar_collapsed = collapsed;
            },
            cx,
        ) {
            self.notice = Some(err.into());
        }
    }

    /// The launcher's skins folder.
    pub fn skins(&self) -> launcher_core::ui_skin::SkinStore {
        launcher_core::ui_skin::SkinStore::new(&self.data_dir)
    }

    /// The chosen skin's id; `None` for the default look.
    pub fn skin(&self) -> Option<&str> {
        self.settings.skin.as_deref()
    }

    /// Uses skin `id` (the default look for `None`), saves the choice and shows it.
    pub fn set_skin(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        let id = id.filter(|id| id != "default");
        if let Err(err) = self.update_settings(|settings| settings.skin = id.clone(), cx) {
            self.notice = Some(err.into());
        }
        crate::skin::apply(self.settings.skin.as_deref(), &self.skins(), cx);
    }

    /// The small copy of an instance's Play background for its sidebar row.
    pub fn sidebar_banner(
        &self,
        id: &str,
    ) -> Option<(Arc<RenderImage>, launcher_core::ui_skin::readable::Backdrop)> {
        self.sidebar_banners
            .get(id)
            .map(|(_, _, image, backdrop)| (image.clone(), *backdrop))
    }

    /// Makes the sidebar rows' copies of the Play backgrounds that are missing or
    /// changed, in the background; forgets those that are gone. Only while the skin
    /// shows them.
    pub fn load_sidebar_banners(&mut self, cx: &mut Context<Self>) {
        // Called on every frame: look again only when the instances or the skin
        // changed (which a reload or an apply follows).
        let checked = (self.list_generation, crate::skin::generation(cx));
        if self.sidebar_banners_checked == Some(checked) {
            return;
        }
        self.sidebar_banners_checked = Some(checked);
        let wanted: Vec<(
            String,
            appearance::BackgroundSource,
            Option<std::time::SystemTime>,
        )> = self
            .list
            .iter()
            .filter_map(|instance| {
                let (source, modified) = self.background_source(instance)?;
                Some((instance.id.clone(), source, modified))
            })
            .collect();
        let mut gone = Vec::new();
        self.sidebar_banners.retain(|id, (time, source, image, _)| {
            let keep = wanted
                .iter()
                .any(|(w, s, m)| w == id && m == time && s == source);
            if !keep {
                gone.push(image.clone());
            }
            keep
        });
        crate::images::release(gone, cx);
        for (id, source, modified) in wanted {
            if self.sidebar_banners.contains_key(&id) || self.sidebar_banner_tasks.contains_key(&id)
            {
                continue;
            }
            let cache = self.hero_cache.clone();
            let decode = cx.background_spawn({
                let source = source.clone();
                async move {
                    let image = image::DynamicImage::ImageRgba8(source.decode(&cache)?);
                    // the row's size at 2×, cropped to fill it, and how it looks behind
                    // the row's text
                    let thumb = image
                        .resize_to_fill(440, 96, image::imageops::FilterType::Triangle)
                        .to_rgba8();
                    let backdrop = launcher_core::ui_skin::readable::Backdrop::measure(
                        thumb.pixels().map(|p| {
                            launcher_core::ui_skin::Rgb(
                                ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32,
                            )
                        }),
                    )?;
                    Some((thumb, backdrop))
                }
            });
            let task_id = id.clone();
            let task = cx.spawn(async move |this, cx| {
                let decoded = decode.await;
                this.update(cx, |this, cx| {
                    this.sidebar_banner_tasks.remove(&task_id);
                    if let Some((image, backdrop)) = decoded {
                        this.sidebar_banners.insert(
                            task_id,
                            (
                                modified,
                                source,
                                crate::images::render_image(image),
                                backdrop,
                            ),
                        );
                        cx.notify();
                    }
                })
                .ok();
            });
            self.sidebar_banner_tasks.insert(id, task);
        }
    }

    /// Forgets the sidebar rows' copies, when the skin stops showing them.
    pub fn forget_sidebar_banners(&mut self, cx: &mut App) {
        let gone = self
            .sidebar_banners
            .drain()
            .map(|(_, (_, _, image, _))| image);
        crate::images::release(gone.collect::<Vec<_>>(), cx);
        self.sidebar_banners_checked = None;
        self.sidebar_banner_tasks.clear();
    }

    /// Shows the chosen skin again, after its pack changed.
    pub fn reload_skin(&mut self, cx: &mut Context<Self>) {
        crate::skin::apply(self.settings.skin.as_deref(), &self.skins(), cx);
    }

    /// Sets an instance's own Play button colour and accent (`None`: the skin's), and
    /// records the change for Undo.
    pub fn set_instance_colors(
        &mut self,
        id: &str,
        play_color: Option<launcher_core::ui_skin::Rgb>,
        accent: Option<launcher_core::ui_skin::Rgb>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.undoable(id, cx, |this, cx| {
            let mut instance = this.store.get(id).map_err(|err| err.to_string())?;
            instance.play_color = play_color;
            instance.accent = accent;
            this.store.save(&instance).map_err(|err| err.to_string())?;
            this.update_listed(instance, cx);
            Ok(())
        })
    }

    /// The instances by sidebar group, as the sidebar lists them.
    pub fn grouped(&self) -> Vec<(Option<String>, Vec<&Instance>)> {
        launcher_core::instances::grouped(&self.list, self.settings.auto_groups, |i| {
            self.linked.contains(&i.id)
        })
    }

    /// Whether the instance is linked to a server: one on a Lodestone core, or a
    /// friend's shared server.
    pub fn is_linked(&self, id: &str) -> bool {
        self.linked.contains(id)
    }

    /// The instances linked to a server. The Lodestone Host page reads the links (the
    /// instances' `lodestone_server.json`), and the shell hands them over here.
    pub fn set_linked(&mut self, linked: HashSet<String>, cx: &mut Context<Self>) {
        if self.linked != linked {
            self.linked = linked;
            cx.notify();
        }
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
        launcher_core::instances::grouped(&self.list, false, |_| false)
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
        self.load_backgrounds(cx);
        cx.notify();
    }

    pub fn shared_revision(&self) -> u64 {
        self.shared_revision
    }

    /// The shared game settings changed in the launcher's Settings.
    pub fn shared_settings_changed(&mut self, cx: &mut Context<Self>) {
        self.shared_revision += 1;
        cx.notify();
    }

    /// How many times the instance's content changed outside its Content tab.
    pub fn content_revision(&self, id: &str) -> u64 {
        self.content_revisions.get(id).copied().unwrap_or(0)
    }

    /// Notes that the instance's content changed outside its Content tab.
    pub(crate) fn content_changed(&mut self, id: &str) {
        *self.content_revisions.entry(id.to_string()).or_default() += 1;
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.runs
            .get(id)
            .is_some_and(|run| !matches!(run.state, RunState::Idle))
    }

    /// Whether any instance is installing, starting or running.
    pub fn any_active(&self) -> bool {
        self.runs
            .values()
            .any(|run| !matches!(run.state, RunState::Idle))
    }

    /// The game has started but its window hasn't opened yet.
    pub fn is_launching(&self, id: &str) -> bool {
        self.runs
            .get(id)
            .is_some_and(|run| matches!(run.state, RunState::Running { .. }) && !run.window_opened)
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
        self.pick_skin_banner(&instance.id);
        self.selected = Some(instance.id.clone());
        self.refresh(&instance.id, cx);
        Ok(instance)
    }

    pub fn clear_icon(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let instance = self.store.clear_icon(id).map_err(|err| err.to_string())?;
        self.update_listed(instance, cx);
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
            // Files only on CurseForge are left out rather than handed out.
            let on_modrinth = export::modrinth_files(&instance_dir);
            let curseforge_only = export::curseforge_only_files(&instance_dir);
            let plan =
                export::plan_leaving_out(&instance, &game_dir, &on_modrinth, &curseforge_only)?;
            export::write(&plan, &out)?;
            Ok::<_, export::ExportError>((plan, out))
        });
        let id = id.to_string();
        self.exports.push(cx.spawn(async move |this, cx| {
            let result = write.await;
            this.update(cx, |_, cx| {
                cx.emit(match result {
                    Ok((plan, out)) => {
                        let saved = trn!(
                            P::FilesSaved,
                            plan.overrides.len() + plan.index.files.len(),
                            path = out.display()
                        );
                        let others = plan.skipped.len() - plan.curseforge_left_out.len();
                        let mut message = match plan
                            .skipped
                            .iter()
                            .find(|(_, why)| !why.contains("CurseForge"))
                        {
                            Some((first, _)) if others > 0 => tr!(
                                M::TwoSentences,
                                first = saved,
                                second = trn!(P::FilesLeftOut, others, name = first)
                            ),
                            _ => saved,
                        };
                        if !plan.curseforge_left_out.is_empty() {
                            message = tr!(
                                M::TwoSentences,
                                first = message,
                                second = trn!(
                                    P::CurseForgeLeftOut,
                                    plan.curseforge_left_out.len(),
                                    names = crate::i18n::format::list(&plan.curseforge_left_out)
                                )
                            );
                        }
                        InstancesEvent::Notice {
                            title: t(M::ModpackExported).into(),
                            message: message.into(),
                            about: About::Instance(id.clone()),
                        }
                    }
                    Err(err) => InstancesEvent::LaunchFailed {
                        title: t(M::CouldntExportModpack).into(),
                        message: err.to_string().into(),
                        about: About::Instance(id.clone()),
                    },
                });
            })
            .ok();
        }));
    }

    pub fn save(&mut self, instance: &Instance, cx: &mut Context<Self>) -> Result<(), String> {
        let before = self.store.get(&instance.id).ok();
        self.store.save(instance).map_err(|err| err.to_string())?;
        if let Some(before) = before {
            launcher_core::trace::settings_changed(
                "instance",
                &before,
                instance,
                serde_json::json!({ "instance": instance.id }),
            );
        }
        self.update_listed(instance.clone(), cx);
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
        if let Some((java, generation)) = self.required_java.borrow().get(game_version)
            && (java.is_some() || *generation == self.list_generation)
        {
            return *java;
        }
        let path = self
            .data_dir
            .versions_dir()
            .join(game_version)
            .join(format!("{game_version}.json"));
        let java = std::fs::read(path).ok().and_then(|bytes| {
            launcher_core::game::version::VersionJson::parse(&bytes)
                .ok()
                .map(|version| version.java_major())
        });
        self.required_java
            .borrow_mut()
            .insert(game_version.to_string(), (java, self.list_generation));
        java
    }

    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.store.game_dir(id)
    }

    /// The Modrinth client, unless the app is offline (the offscreen screenshots).
    pub fn modrinth(&self) -> Option<Arc<Modrinth>> {
        self.modrinth.clone()
    }

    /// Whether an imported pack left files for the user to download into `id`.
    pub fn has_pending_downloads(&self, id: &str) -> bool {
        self.pending_downloads.contains_key(id)
    }

    pub fn take_pending_downloads(
        &mut self,
        id: &str,
    ) -> Option<Vec<launcher_core::content::curseforge_install::CfPlannedFile>> {
        self.pending_downloads.remove(id)
    }

    /// The CurseForge client, when this build has an API key and the app is online.
    pub fn curseforge(&self) -> Option<Arc<CurseForge>> {
        self.curseforge.clone()
    }

    /// The installer, for resolving imported instances' versions and loaders.
    pub fn installer(&self) -> Installer {
        self.installer.clone()
    }

    /// The instance store, for imports that build instances in the background.
    pub fn store(&self) -> InstanceStore {
        self.store.clone()
    }

    /// Keeps the Content tab off the network, for the offscreen screenshots.
    pub fn preview_offline(&mut self) {
        self.modrinth = None;
        self.curseforge = None;
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
                        title: t(M::CouldntSwitchLoader).into(),
                        message: message.into(),
                        about: About::Instance(id.clone()),
                    });
                    cx.notify();
                };
                let (kind, version) = match result {
                    Ok(Some(pick)) => pick,
                    Ok(None) => {
                        return failed(tr!(M::NoModLoaderFor, version = instance.game_version), cx);
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
                            title: tr!(M::SwitchedTo, loader = kind.label()).into(),
                            message: tr!(
                                M::SwitchedToMessage,
                                name = changed.name,
                                loader = kind.label(),
                                version = version.display
                            )
                            .into(),
                            about: About::Instance(id.clone()),
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
        match self.selected() {
            Some(instance) => self.play_state_of(&instance.id),
            None => PlayState::NoInstance,
        }
    }

    /// The Play button's state for the instance `id`, selected or not.
    pub fn play_state_of(&self, id: &str) -> PlayState {
        if let Some(state) = &self.preview_state
            && self.selected().is_some_and(|selected| selected.id == id)
        {
            return state.clone();
        }
        match self.runs.get(id).map(|run| &run.state) {
            None | Some(RunState::Idle) => PlayState::Ready,
            // Java has started, but until the game's window opens there's nothing to
            // see: what the game last logged shows what it's doing.
            Some(RunState::Running { .. }) if !self.runs[id].window_opened => {
                PlayState::Preparing {
                    label: t(M::StageLaunching).into(),
                    detail: self.runs[id].log.latest_game_line(),
                    fraction: None,
                    step: LaunchStep::Launching,
                }
            }
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

    /// Makes Play switch the Play button to `state` instead of playing, for the
    /// offscreen screenshots of its transition (`--click` on Play).
    pub fn preview_press(&mut self, state: PlayState) {
        self.preview_press = Some(state);
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
        self.play_joining(id, None, cx)
    }

    /// Like [`Self::play`], joining the server `join` once the game has started.
    pub fn play_joining(
        &mut self,
        id: &str,
        join: Option<launch::JoinServer>,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = self.preview_press.take() {
            self.preview_state = Some(state);
            cx.notify();
            return;
        }
        if self.is_active(id) {
            return;
        }
        if self.modpack_updates.contains_key(id) {
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::ModpackUpdating).into(),
                message: t(M::WaitForUpdate).into(),
                about: About::Instance(id.to_string()),
            });
            return;
        }
        // its folder is on its way to the Trash
        if self.file_op(id) == Some(FileOp::Delete) {
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntStartMinecraft).into(),
                message: t(M::FilesBusy).into(),
                about: About::Instance(id.to_string()),
            });
            return;
        }
        let Ok(mut instance) = self.store.get(id) else {
            return;
        };
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let run = self.runs.entry(id.to_string()).or_default();
        run.reached_menu = false;
        run.mods_refused = false;
        run.log.note(
            Level::Info,
            tr!(
                M::PreparingNote,
                version = instance.game_version,
                loader = loader_label(&instance)
            ),
        );
        if let Some(join) = &join {
            run.log
                .note(Level::Info, tr!(M::JoiningNote, address = join.address()));
        }
        run.state = RunState::Preparing {
            progress: progress.clone(),
            cancel: cancel.clone(),
        };
        run.ticker = Some(ticker(id.to_string(), cx));
        launcher_core::trace::emit("game.prepare", || {
            serde_json::json!({
                "instance": id,
                "name": instance.name,
                "version": instance.game_version,
                "loader": loader_label(&instance),
                "join": join.as_ref().map(|j| j.address()),
            })
        });
        cx.notify();
        self.auto_configure_launch(&mut instance, cx);

        let account = self
            .accounts
            .update(cx, |accounts, cx| accounts.launch_account(cx));
        let installer = self.installer.clone();
        let data = self.data_dir.clone();
        let machine = self.machine.clone();
        let game_dir = self.store.game_dir(id);
        let natives_dir = self.store.natives_dir(id);
        let settings = instance.game_settings(self.defaults(), self.total_memory);
        // Other launchers' games to copy controls from, looked for while the game
        // prepares, only for an instance automatic setup hasn't given options yet.
        let find_outside = self.settings.auto_configure
            && instance
                .auto_config
                .as_ref()
                .is_some_and(|state| !state.options);
        let id = id.to_string();
        let run_id = id.clone();
        let task = cx.spawn(async move |this, cx| {
            let account = match account.await {
                Ok(account) => account,
                Err(message) => {
                    this.update(cx, |this, cx| {
                        this.fail(&id, t(M::CouldntSignIn).into(), message.into(), cx)
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
                    let command = launch::build_joining(
                        &prepared.version,
                        &prepared.java_platform,
                        &paths,
                        &account,
                        &settings,
                        join.as_ref(),
                    )
                    .map_err(|err| InstallError::Invalid(err.to_string()))?;
                    let outside = if find_outside {
                        launcher_core::configurator::outside_candidates()
                    } else {
                        Vec::new()
                    };
                    Ok::<_, InstallError>((command, prepared.rosetta, outside))
                })
                .await;
            this.update(cx, |this, cx| match prepared {
                Ok((command, rosetta, outside)) => {
                    this.take_notes(&id);
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(
                            Level::Info,
                            check_summary(&stats_progress, check_started.elapsed()),
                        );
                    }
                    this.start(&id, command, rosetta, outside, cx)
                }
                Err(InstallError::Cancelled) => {
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(Level::Warn, t(M::Cancelled));
                    }
                    this.set_idle(&id, cx);
                }
                Err(InstallError::RosettaMissing) => this.fail(
                    &id,
                    t(M::RosettaNeeded).into(),
                    rosetta_missing().into(),
                    cx,
                ),
                Err(err) => this.fail(
                    &id,
                    t(M::CouldntStartMinecraft).into(),
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
            tr!(
                M::VerifyingNote,
                version = instance.game_version,
                loader = loader_label(&instance)
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
                        title: t(M::GameFilesVerified).into(),
                        message: if repaired == 0 {
                            tr!(M::EveryFileIntact, summary = summary).into()
                        } else {
                            trn!(P::FilesRepaired, repaired, summary = summary).into()
                        },
                        about: About::Instance(id.clone()),
                    });
                }
                Err(InstallError::Cancelled) => {
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(Level::Warn, t(M::Cancelled));
                    }
                    this.set_idle(&id, cx);
                }
                Err(err) => this.fail(&id, t(M::CouldntVerify).into(), err.to_string().into(), cx),
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
        launcher_core::trace::emit(
            "game.state",
            || serde_json::json!({ "instance": id, "state": "idle" }),
        );
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
            run.log.note(
                Level::Error,
                tr!(M::TitleAndMessage, title = title, message = message),
            );
        }
        launcher_core::trace::emit(
            "game.failed",
            || serde_json::json!({ "instance": id, "title": title.to_string(), "message": message.to_string() }),
        );
        self.set_idle(id, cx);
        cx.emit(InstancesEvent::LaunchFailed {
            title,
            message,
            about: About::Instance(id.to_string()),
        });
    }

    fn start(
        &mut self,
        id: &str,
        command: LaunchCommand,
        rosetta: bool,
        outside: Vec<launcher_core::configurator::Outside>,
        cx: &mut Context<Self>,
    ) {
        let game_dir = self.store.game_dir(id);
        let before = crash::crash_reports(&game_dir);
        self.auto_configure_options(id, outside, cx);
        self.merge_shared_settings(id);
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        if let RunState::Preparing { progress, .. } = &run.state {
            progress.set_stage(Stage::Starting);
        }
        if rosetta {
            run.log.note(Level::Info, t(M::RosettaNote));
        }
        // The command line is shown with the access token redacted.
        run.log.note(
            Level::Info,
            tr!(M::StartingJavaNote, command = command.display()),
        );
        let (process, events) = match GameProcess::spawn(&command, Redactor::new(command.secrets()))
        {
            Ok(spawned) => spawned,
            Err(err) => {
                self.fail(
                    id,
                    t(M::CouldntStartJava).into(),
                    tr!(
                        M::TitleAndMessage,
                        title = command.program.display(),
                        message = err
                    )
                    .into(),
                    cx,
                );
                return;
            }
        };
        let started = Instant::now();
        let started_unix = launcher_core::instances::now();
        let pid = process.pid();
        launcher_core::trace::emit(
            "game.launch",
            || serde_json::json!({ "instance": id, "pid": pid }),
        );
        run.state = RunState::Running {
            process: Arc::new(process),
            started,
        };
        run.ticker = None;
        run.window_opened = false;
        run.window_wait = Some(cx.spawn({
            let id = id.to_string();
            async move |this, cx| {
                cx.background_executor().timer(WINDOW_WAIT).await;
                this.update(cx, |this, cx| {
                    if let Some(run) = this.runs.get_mut(&id)
                        && matches!(run.state, RunState::Running { .. })
                        && !run.window_opened
                    {
                        run.window_opened = true;
                        cx.emit(InstancesEvent::GameWindowOpened(id.clone()));
                        cx.notify();
                    }
                })
                .ok();
            }
        }));
        let started_id = id.to_string();
        let id = id.to_string();
        run.task = Some(cx.spawn(async move |this, cx| {
            let mut events = events.ready_chunks(4096);
            while let Some(batch) = events.next().await {
                let mut exited = None;
                let alive = this.update(cx, |this, cx| {
                    let mut milestones = Vec::new();
                    if let Some(run) = this.runs.get_mut(&id) {
                        for event in batch {
                            match event {
                                GameEvent::Line(line) => {
                                    if !run.reached_menu && crash::reached_menu(&line.message) {
                                        run.reached_menu = true;
                                        milestones.push(InstancesEvent::ReachedMenu(id.clone()));
                                    }
                                    if let Some(port) =
                                        launcher_core::cores::lan_open::started_serving_port(
                                            &line.message,
                                        )
                                    {
                                        milestones.push(InstancesEvent::LanOpened {
                                            id: id.clone(),
                                            port,
                                        });
                                    }
                                    if !run.mods_refused
                                        && (crash::mods_refused(&line.message)
                                            || line
                                                .throwable
                                                .as_deref()
                                                .is_some_and(crash::mods_refused))
                                    {
                                        run.mods_refused = true;
                                        milestones.push(InstancesEvent::ModsRefused(id.clone()));
                                    }
                                    launcher_core::trace::emit("game.log", || {
                                        serde_json::json!({
                                            "instance": id,
                                            "level": line.level.to_string(),
                                            "thread": line.thread,
                                            "logger": line.logger,
                                            "text": line.message,
                                        })
                                    });
                                    if !run.window_opened && line.opens_game_window() {
                                        run.window_opened = true;
                                        run.window_wait = None;
                                        launcher_core::trace::emit(
                                            "game.window",
                                            || serde_json::json!({ "instance": id }),
                                        );
                                        cx.emit(InstancesEvent::GameWindowOpened(id.clone()));
                                    }
                                    run.log.push(line)
                                }
                                GameEvent::Exited { code, stopped } => {
                                    exited = Some((code, stopped))
                                }
                            }
                        }
                    }
                    for milestone in milestones {
                        cx.emit(milestone);
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
                // Each batch redraws the window. A modded game prints thousands of
                // lines as it starts: let what arrives meanwhile gather into the next
                // batch (the channel is unbounded, so the game never waits), so the
                // log redraws at most about 15 times a second.
                cx.background_executor().timer(LOG_BATCH_INTERVAL).await;
            }
        }));
        // Played as of now, so a game that crashes (or a launcher that quits) before
        // it exits still counts; the play time is added when it exits.
        match self.store.record_start(&started_id, started_unix) {
            Ok(instance) => self.update_listed(instance, cx),
            Err(err) => self.notice = Some(tr!(M::CouldntSavePlayed, error = err).into()),
        }
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
        launcher_core::trace::emit("game.exit", || {
            serde_json::json!({
                "instance": id,
                "code": code,
                "stopped": stopped,
                "crashed": matches!(outcome, Outcome::Crashed { .. }),
                "secs": started.elapsed().as_secs(),
            })
        });
        if let Err(err) = self
            .store
            .record_play(id, started_unix, started.elapsed().as_secs())
        {
            self.notice = Some(tr!(M::CouldntSavePlayTime, error = err).into());
        }
        let name = self
            .list
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let dirs = self.content_dirs(id);
        let exited = !matches!(outcome, Outcome::Crashed { .. });
        if let Some(run) = self.runs.get_mut(id) {
            let message = match (code, stopped) {
                (_, true) => t(M::StoppedNote).to_string(),
                (Some(code), _) => tr!(M::ExitedWithCode, code = code),
                (None, _) => t(M::EndedBySignal).to_string(),
            };
            let level = if matches!(outcome, Outcome::Crashed { .. }) {
                Level::Error
            } else {
                Level::Info
            };
            run.log.note(level, message);
            let at_startup = !run.reached_menu;
            let mods_refused = run.mods_refused;
            if let Outcome::Crashed { exit_code, report } = outcome {
                // Finding the mods involved opens their jars, so the report is read and
                // the crash worked out off the window's thread. It takes well under a
                // second: the game has just read the same files.
                let recent = run.log.recent();
                let log_text = run.log.to_text(|_| true);
                let instance_id = id.to_string();
                let work = cx.background_spawn(async move {
                    let report_text = report
                        .as_ref()
                        .and_then(|path| std::fs::read_to_string(path).ok());
                    let summary = crash::summarize(report_text.as_deref(), &recent);
                    let suspects = crash::suspects::find(&dirs, report_text.as_deref(), &recent);
                    CrashInfo {
                        instance_id,
                        instance_name: name,
                        summary,
                        suspects,
                        exit_code,
                        report,
                        log_text,
                        at_startup,
                        mods_refused,
                    }
                });
                cx.spawn(async move |this, cx| {
                    let crash = work.await;
                    this.update(cx, |_, cx| cx.emit(InstancesEvent::Crashed(crash)))
                        .ok();
                })
                .detach();
            }
        }
        if let Some(run) = self.runs.get_mut(id) {
            run.window_wait = None;
        }
        self.read_back_shared_settings(id);
        self.set_idle(id, cx);
        self.refresh(id, cx);
        // After it is idle, so whoever hears it can start the game again.
        if exited {
            cx.emit(InstancesEvent::Exited {
                id: id.to_string(),
                stopped,
            });
        }
        cx.emit(InstancesEvent::GameExited(id.to_string()));
    }

    /// Cancels the preparation, or kills the game.
    pub fn stop(&mut self, id: &str, cx: &mut Context<Self>) {
        // the offscreen screenshots' Play button, back to Play (`--click` on Cancel)
        if self.preview_state.take().is_some() {
            cx.notify();
            return;
        }
        let Some(run) = self.runs.get_mut(id) else {
            return;
        };
        match &run.state {
            RunState::Idle => {}
            RunState::Preparing { cancel, .. } => {
                cancel.cancel();
                run.log.note(Level::Warn, t(M::CancellingNote));
            }
            RunState::Running { process, .. } => {
                run.log.note(Level::Warn, t(M::StoppingNote));
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
    let step = match stage {
        Stage::Preparing => LaunchStep::Checking,
        Stage::Java | Stage::Libraries | Stage::Loader | Stage::Processors | Stage::Assets => {
            LaunchStep::Downloading
        }
        Stage::Natives | Stage::Starting => LaunchStep::Starting,
    };
    if !downloading || snapshot.files_total == 0 {
        return PlayState::Preparing {
            label: stage_label(stage).into(),
            detail: None,
            fraction: None,
            step,
        };
    }
    let mb = |bytes: u64| (bytes as f64 / (1024.0 * 1024.0)).round() as u64;
    let detail = tr!(
        M::ProgressDetail,
        files_done = thousands(snapshot.files_done),
        files_total = thousands(snapshot.files_total),
        done = thousands(mb(snapshot.bytes_done)),
        total = thousands(mb(snapshot.bytes_total)),
    );
    let fraction = if snapshot.bytes_total > 0 {
        snapshot.bytes_done as f32 / snapshot.bytes_total as f32
    } else {
        snapshot.files_done as f32 / snapshot.files_total as f32
    };
    PlayState::Preparing {
        label: stage_label(stage).into(),
        detail: Some(detail.into()),
        fraction: Some(fraction.clamp(0.0, 1.0)),
        step,
    }
}

/// `1234567` as `1,234,567`.
pub fn thousands(value: u64) -> String {
    crate::i18n::format::number(value)
}

/// "Checked 4,091 game files in 45 ms: 4,091 unchanged, 0 hashed, 0 downloaded."
fn check_summary(progress: &InstallProgress, elapsed: Duration) -> String {
    let (trusted, hashed, downloaded) = progress.stats.counts();
    trn!(
        P::CheckSummary,
        trusted + hashed + downloaded,
        ms = thousands(elapsed.as_millis() as u64),
        unchanged = thousands(trusted),
        hashed = thousands(hashed),
        downloaded = thousands(downloaded),
    )
}

/// What the Play button says while the game is prepared: "Downloading assets".
pub fn stage_label(stage: Stage) -> &'static str {
    t(match stage {
        Stage::Preparing => M::StagePreparing,
        Stage::Java => M::StageJava,
        Stage::Libraries => M::StageLibraries,
        Stage::Loader => M::StageLoader,
        Stage::Processors => M::StageProcessors,
        Stage::Assets => M::StageAssets,
        Stage::Natives => M::StageNatives,
        Stage::Starting => M::StageStarting,
    })
}

/// Why a version needs Rosetta and how to install it, for the "Rosetta 2 is needed"
/// dialog.
pub fn rosetta_missing() -> &'static str {
    t(M::RosettaMissing)
}

/// The instance's loader and its version, "Fabric 0.16.10", or "Vanilla".
pub fn loader_label(instance: &Instance) -> String {
    match instance.loader.kind() {
        Some(_) => instance.loader_label(),
        None => t(M::Vanilla).into(),
    }
}

/// "2 h 5 min", "12 min", "Not played yet".
pub fn play_time(seconds: u64) -> String {
    crate::i18n::format::play_time(seconds)
}

/// When something happened, relative to now, for "Last played".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ago {
    JustNow,
    Minutes(u64),
    Hours(u64),
    Yesterday,
    Days(u64),
    /// Longer ago: the date, "Mar 12", with the year when it isn't this year's.
    On(String),
}

/// The earliest time (Unix seconds) at which an "ago" label drawn since the shell's
/// clock last redrew the window reads differently: see [`Ago::label_changed`].
static NEXT_AGO_CHANGE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

impl Ago {
    /// How long ago `then` (Unix seconds) was, in this computer's time zone. Notes when
    /// the label will change, for the shell's clock.
    pub fn since(then: u64) -> Self {
        let now = launcher_core::instances::now();
        let ago = Self::between(then, now, &chrono::Local);
        NEXT_AGO_CHANGE.fetch_min(
            ago.changes_at(then, now, &chrono::Local),
            std::sync::atomic::Ordering::Relaxed,
        );
        ago
    }

    /// Whether a label drawn since the last call now reads differently, so the window
    /// should be drawn again. Forgets the labels: drawing them again notes them anew.
    pub fn label_changed(now: u64) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        if now < NEXT_AGO_CHANGE.load(Relaxed) {
            return false;
        }
        NEXT_AGO_CHANGE.store(u64::MAX, Relaxed);
        true
    }

    /// When (Unix seconds) this label, for `then` at `now`, next reads differently: at
    /// the next minute or hour it counts, or the next midnight in `zone`, whichever
    /// comes first.
    pub fn changes_at<Tz: chrono::TimeZone>(&self, then: u64, now: u64, zone: &Tz) -> u64 {
        let midnight = zone
            .timestamp_opt(now.min(i64::MAX as u64) as i64, 0)
            .earliest()
            .and_then(|time| time.date_naive().succ_opt())
            .and_then(|day| day.and_hms_opt(0, 0, 0))
            .and_then(|start| zone.from_local_datetime(&start).earliest())
            .map(|start| start.timestamp().max(0) as u64)
            .unwrap_or(now.saturating_add(3600));
        let counted = match self {
            Ago::JustNow => then.saturating_add(60),
            Ago::Minutes(n) => then.saturating_add(60 * (n + 1)),
            Ago::Hours(n) => then.saturating_add(3600 * (n + 1)),
            Ago::Yesterday | Ago::Days(_) | Ago::On(_) => u64::MAX,
        };
        // never before now: a label that should already have changed changes at once
        counted.min(midnight).max(now)
    }

    /// How long before `now` `then` was, by the calendar of `zone`. A time after
    /// `now` (a clock that moved back) is "just now".
    pub fn between<Tz: chrono::TimeZone>(then: u64, now: u64, zone: &Tz) -> Self {
        let elapsed = now.saturating_sub(then);
        let day = |unix: u64| {
            zone.timestamp_opt(unix.min(i64::MAX as u64) as i64, 0)
                .earliest()
                .map(|time| time.date_naive())
        };
        let (Some(then_day), Some(today)) = (day(then), day(now)) else {
            return Ago::JustNow;
        };
        let days = (today - then_day).num_days();
        match elapsed {
            0..60 => Ago::JustNow,
            60..3600 => Ago::Minutes(elapsed / 60),
            // Late last night is still "3 hours ago" in the morning.
            _ if days <= 0 || elapsed < 6 * 3600 => Ago::Hours(elapsed / 3600),
            _ if days == 1 => Ago::Yesterday,
            _ if days < 7 => Ago::Days(days as u64),
            _ => Ago::On(crate::i18n::format::short_date(
                then_day,
                chrono::Datelike::year(&then_day) != chrono::Datelike::year(&today),
            )),
        }
    }

    /// "Just now", "5 minutes ago", "2 hours ago", "Yesterday", "3 days ago" or
    /// "Mar 12".
    pub fn label(&self) -> String {
        use crate::i18n::{FormatMsg, FormatPlural, t, trn};
        match self {
            Ago::JustNow => t(FormatMsg::JustNow).into(),
            Ago::Minutes(n) => trn!(FormatPlural::MinutesAgo, *n),
            Ago::Hours(n) => trn!(FormatPlural::HoursAgo, *n),
            Ago::Yesterday => t(FormatMsg::Yesterday).into(),
            Ago::Days(n) => trn!(FormatPlural::DaysAgo, *n),
            Ago::On(date) => date.clone(),
        }
    }

    /// "Played just now", "Played 2 hours ago", "Played yesterday" or "Played on
    /// Mar 12".
    pub fn played(&self) -> String {
        use crate::i18n::{FormatMsg, t, tr};
        match self {
            Ago::JustNow => t(FormatMsg::PlayedJustNow).into(),
            Ago::Yesterday => t(FormatMsg::PlayedYesterday).into(),
            Ago::On(date) => tr!(FormatMsg::PlayedOn, date = date),
            other => tr!(FormatMsg::PlayedAgo, ago = other.label()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_long_ago() {
        use chrono::TimeZone;
        let utc = chrono::Utc;
        let at = |y, mo, d, h, mi| {
            utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
                .unwrap()
                .timestamp() as u64
        };
        let now = at(2026, 11, 8, 15, 0);
        let ago = |then: u64| Ago::between(then, now, &utc);
        let label = |then: u64| ago(then).label();
        assert_eq!(label(now), "Just now");
        assert_eq!(label(now - 59), "Just now");
        assert_eq!(label(now + 300), "Just now", "a clock that moved back");
        assert_eq!(label(now - 60), "1 minute ago");
        assert_eq!(label(now - 5 * 60), "5 minutes ago");
        assert_eq!(label(now - 3599), "59 minutes ago");
        assert_eq!(label(now - 3600), "1 hour ago");
        assert_eq!(label(at(2026, 11, 8, 0, 30)), "14 hours ago");
        // Before midnight, but only a few hours ago.
        let morning = at(2026, 11, 8, 2, 0);
        assert_eq!(
            Ago::between(at(2026, 11, 7, 23, 0), morning, &utc).label(),
            "3 hours ago"
        );
        assert_eq!(
            Ago::between(at(2026, 11, 7, 18, 0), morning, &utc).label(),
            "Yesterday"
        );
        assert_eq!(label(at(2026, 11, 7, 20, 0)), "Yesterday");
        assert_eq!(label(at(2026, 11, 7, 0, 5)), "Yesterday");
        assert_eq!(label(at(2026, 11, 6, 23, 0)), "2 days ago");
        assert_eq!(label(at(2026, 11, 2, 12, 0)), "6 days ago");
        assert_eq!(label(at(2026, 11, 1, 12, 0)), "Nov 1");
        assert_eq!(label(at(2026, 3, 12, 9, 0)), "Mar 12");
        assert_eq!(label(at(2025, 12, 31, 9, 0)), "Dec 31, 2025");
        // The calendar is the given zone's: 01:00 UTC on the 7th is still the 6th in
        // New York.
        let new_york = chrono::FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(
            Ago::between(at(2026, 11, 7, 1, 0), now, &utc).label(),
            "Yesterday"
        );
        assert_eq!(
            Ago::between(at(2026, 11, 7, 1, 0), now, &new_york).label(),
            "2 days ago"
        );

        assert_eq!(ago(now).played(), "Played just now");
        assert_eq!(ago(now - 7200).played(), "Played 2 hours ago");
        assert_eq!(ago(at(2026, 11, 7, 20, 0)).played(), "Played yesterday");
        assert_eq!(ago(at(2026, 3, 12, 9, 0)).played(), "Played on Mar 12");
    }

    #[test]
    fn knows_when_its_label_changes() {
        use chrono::TimeZone;
        let utc = chrono::Utc;
        let at = |y, mo, d, h, mi, s| {
            utc.with_ymd_and_hms(y, mo, d, h, mi, s)
                .unwrap()
                .timestamp() as u64
        };
        let now = at(2026, 11, 8, 15, 0, 0);
        let changes = |then: u64| Ago::between(then, now, &utc).changes_at(then, now, &utc);
        // "Just now" until a minute has passed, then each minute
        assert_eq!(changes(now - 10), now + 50);
        assert_eq!(changes(now - 5 * 60 - 20), now + 40);
        // each hour, or at midnight if that comes first
        assert_eq!(changes(now - 2 * 3600 - 600), now + 3000);
        assert_eq!(
            changes(at(2026, 11, 8, 0, 30, 0)),
            at(2026, 11, 8, 15, 30, 0)
        );
        let late = at(2026, 11, 8, 23, 40, 0);
        assert_eq!(
            Ago::between(late - 3 * 3600, late, &utc).changes_at(late - 3 * 3600, late, &utc),
            at(2026, 11, 9, 0, 0, 0)
        );
        // days and dates: at midnight
        assert_eq!(changes(at(2026, 11, 7, 12, 0, 0)), at(2026, 11, 9, 0, 0, 0));
        assert_eq!(changes(at(2026, 3, 12, 9, 0, 0)), at(2026, 11, 9, 0, 0, 0));
        // a clock that moved back: "Just now" until a minute after `then`
        assert_eq!(changes(now + 300), now + 360);
        // the label it gives then really is different, and the one a second before isn't
        for then in [now - 10, now - 5 * 60 - 20, now - 2 * 3600 - 600] {
            let change = changes(then);
            let label = |at: u64| Ago::between(then, at, &utc);
            assert_eq!(label(change - 1), label(now));
            assert_ne!(label(change), label(now));
        }
    }

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
                step: LaunchStep::Downloading,
            }
        );
        assert_eq!(
            progress_state(Stage::Starting, ProgressSnapshot::default()),
            PlayState::Preparing {
                label: "Starting Java".into(),
                detail: None,
                fraction: None,
                step: LaunchStep::Starting,
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
