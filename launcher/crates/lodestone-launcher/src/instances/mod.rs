//! The app's instances: the list from `launcher-core`, the selected one, and each
//! instance's run (installing, playing) with its console log.

pub mod add_modpack;
pub mod appearance;
mod auto_config;
pub mod console;
pub mod content;
mod copy_as;
pub mod fields;
mod files;
pub mod import;
mod jobs;
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
pub mod world_backups;
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
        install::{self, InstallError, InstallProgress, Installer, JavaChoice, Rosetta, Stage},
        launch::{self, LaunchCommand},
        loaders::{LoaderChoice, LoaderKind, LoaderVersion},
        log4j::{self, Level, LogLine, Redactor, Stream},
        mojang::MojangUrls,
        platform::{self, Arch, Os, Platform},
        process::{GameEvent, GameProcess},
    },
    images::{Checked, Images, labels},
    instances::{Instance, InstanceStore},
    paths::DataDir,
    settings::{self as launcher_settings, InstanceDefaults, Settings},
    world_backups::{WorldBackup, WorldBackups},
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
    /// Events pushed to `recent` since the start, never reset.
    recent_pushed: u64,
    /// Where the game's current run started ([`Self::start_run`]): the number of its
    /// first event in `recent_pushed`, and of its first row in `pushed`. The log
    /// lasts the whole session, and a crash is worked out from its own run only.
    run_start: (u64, u64),
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
        let pushed = rows.len() as u64;
        Self {
            pushed,
            revision: 1,
            rows,
            recent: VecDeque::new(),
            recent_pushed: 0,
            run_start: (0, pushed),
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
        self.recent_pushed += 1;
        while self.recent.len() > 500 {
            self.recent.pop_front();
        }
        self.revision += 1;
    }

    /// Marks where a new run of the game starts: the crash summary, its suspects and
    /// the line under "Launching Minecraft" only read what comes after, not what an
    /// earlier run of the session left in the log.
    pub fn start_run(&mut self) {
        self.run_start = (self.recent_pushed, self.pushed);
    }

    /// The current run's events still in `recent`.
    fn run_events(&self) -> impl DoubleEndedIterator<Item = &LogLine> {
        let first = self.recent_pushed - self.recent.len() as u64;
        self.recent
            .iter()
            .skip(self.run_start.0.saturating_sub(first) as usize)
    }

    /// The game's last line worth showing under "Launching Minecraft": its first
    /// line, without the launcher's own notes, warnings about the JVM or blank lines.
    /// Only this run's: until the game says something, there is none.
    pub fn latest_game_line(&self) -> Option<SharedString> {
        self.run_events().rev().take(40).find_map(|line| {
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
            push_row_text(&mut text, row);
        }
        text
    }

    /// The current run's last events, for the crash summary.
    pub fn run_recent(&self) -> Vec<LogLine> {
        self.run_events().cloned().collect()
    }

    /// The current run's rows as text, for the crash dialog's "Copy log".
    pub fn run_text(&self, filter: impl Fn(&LogRow) -> bool) -> String {
        let first = self.pushed - self.rows.len() as u64;
        let skip = self.run_start.1.saturating_sub(first) as usize;
        let mut text = String::new();
        for row in self.rows.iter().skip(skip).filter(|row| filter(row)) {
            push_row_text(&mut text, row);
        }
        text
    }
}

/// A row as a line of text: "[time] [source/LEVEL] text".
fn push_row_text(text: &mut String, row: &LogRow) {
    if let Some(time) = &row.time {
        text.push_str(&format!("[{time}] "));
    }
    if let Some(source) = &row.source {
        text.push_str(&format!("[{source}/{}] ", row.level));
    }
    text.push_str(&row.text);
    text.push('\n');
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
    /// The game, prepared, waiting for the player to agree to run it under Rosetta 2
    /// ([`InstancesEvent::ConfirmRosetta`]); the run stays preparing meanwhile.
    pending: Option<PendingStart>,
    /// The check before the launch ran ([`Images::before_launch`]), so the instance's
    /// content images wait for this run to end ([`Images::session_ended`]).
    image_session: bool,
}

/// One of [`Instances::rosetta_note`]'s answers from a version file: the answer,
/// whether the version file was found, and the reload generation it was looked at.
type RosettaNote = (Option<Rosetta>, bool, u64);

/// What a version file says about Java: the major version it names, and the Java it
/// runs in without an override ([`install::default_java`]).
type VersionJava = (u32, install::DefaultJava);

/// A prepared game that starts once the player agrees to run it under Rosetta 2.
struct PendingStart {
    command: LaunchCommand,
    rosetta: Rosetta,
    outside: Vec<launcher_core::configurator::Outside>,
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
            pending: None,
            image_session: false,
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
    /// Preparing, verifying or starting the game of the instance failed: signing in, a
    /// download, Java. Said apart from [`InstancesEvent::LaunchFailed`] so a "Find it for me"
    /// search can tell its own try's failure from another job's.
    PlayFailed {
        id: String,
        title: SharedString,
        message: SharedString,
    },
    /// The preparation of the instance's game was cancelled before the game started.
    /// Nothing exited or crashed, so no other event says the run is over.
    PlayCancelled(String),
    /// Something to tell the user, such as the result of "Verify game files".
    Notice {
        title: SharedString,
        message: SharedString,
        about: About,
    },
    /// The instance's loader or Minecraft version was changed outside its Settings tab:
    /// by a modpack update, to match its linked server's pack, or by restoring a
    /// snapshot's mods. The user never changes them in place; a copy with another
    /// version or loader is a new instance.
    LoaderChanged(String),
    /// An image file (`.lodeimg`) was dropped or picked: offer to make an instance from it.
    ImportSnapshot(std::path::PathBuf),
    /// The snapshot before a modpack update failed, so nothing changed: ask again, saying
    /// why, and offer to update without one.
    ModpackUpdateNeedsSnapshot(Box<modpacks::UpdatePrompt>),
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
    /// The game of `id` is prepared to run under Rosetta 2 for the first time, for
    /// `reason`: it starts once the player agrees ([`Instances::confirm_rosetta`]).
    ConfirmRosetta {
        id: String,
        version: String,
        reason: Rosetta,
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
    /// Instances whose files a job of their Content or Worlds tab, or a snapshot, is
    /// writing (see `jobs.rs`).
    busy_files: jobs::BusyCount,
    /// Instances whose game change waits for its snapshot before `instance.json` is saved
    /// ([`Instances::save_after_snapshot`]), with the settings saved meanwhile: they are
    /// saved once that save is done, rather than taken for a change made elsewhere.
    saves_after_snapshot: HashMap<String, Option<Instance>>,
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
    /// [`Self::required_java`]'s and [`Self::default_java`]'s answers by game version:
    /// the instance Settings tab asks on every frame, and each answer reads and parses
    /// a version file. An installed version's answer never changes; a missing one is
    /// asked again after a reload (the reload generation it was found at).
    required_java: std::cell::RefCell<HashMap<String, (Option<VersionJava>, u64)>>,
    /// [`Self::rosetta_note`]'s answers from version files, by game version and Java
    /// override: the header asks on every frame. With whether the version file was
    /// found, and the reload generation it was looked for at, as for `required_java`.
    rosetta_notes: std::cell::RefCell<HashMap<(String, Option<PathBuf>), RosettaNote>>,
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
    /// Exports still writing.
    exporting: usize,
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
    /// Modpacks being made into instances.
    imports: Vec<modpacks::ImportJob>,
    next_import: u64,
    /// The instance each finished "Copy with another version or loader" job made, by
    /// job id.
    copies: HashMap<u64, String>,
    /// Each instance's snapshots, newest first, once read (see snapshots.rs).
    snapshot_rows: HashMap<String, snapshots::SnapshotList>,
    snapshot_tasks: HashMap<String, Task<()>>,
    /// How many times each instance's snapshot list changed (taken, pinned, deleted, pruned:
    /// `Images::watch_snapshots`), so views that show snapshots (the Worlds tab's backups)
    /// read them again.
    snapshot_revisions: HashMap<String, u64>,
    /// Files of an imported CurseForge pack the user downloads from curseforge.com, by
    /// instance; the Content tab takes them when it shows that instance.
    pending_downloads:
        HashMap<String, Vec<launcher_core::content::curseforge_install::CfPlannedFile>>,
    /// Modpack updates in progress, by instance.
    modpack_updates: HashMap<String, modpacks::UpdateJob>,
    /// Counts changes to an instance's content made outside its Content tab (a modpack
    /// added, removed or updated), so the tab reads its lists again.
    content_revisions: HashMap<String, u64>,
    /// Redraws while imports and updates run.
    import_ticker: Option<Task<()>>,
    /// The instances' content images: every action that changes an instance's content
    /// begins an operation here ([`Images::begin`]) in its background task.
    images: Arc<Images>,
    /// Zips of single worlds, beside the snapshots.
    world_backups: Arc<WorldBackups>,
    /// Each instance's world backups, newest first, once read (see world_backups.rs).
    world_backup_rows: HashMap<String, Vec<WorldBackup>>,
    world_backup_tasks: HashMap<String, Task<()>>,
    /// How many times each instance's world backups changed (`WorldBackups::watch`), so
    /// the Worlds tab reads them again.
    world_backup_revisions: HashMap<String, u64>,
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
        let images = Images::open(&data_dir);
        let world_backups = WorldBackups::open(&data_dir, images.clone());
        // A restore a crash interrupted is finished or undone before anything can be
        // played: on this thread, since it only renames folders (back, or to the trash
        // that the task below empties).
        let recovery = images.recover_restores();
        if !recovery.broken.is_empty() {
            log::warn!(
                "Interrupted restores of {:?} couldn't be undone; Play tries again and waits until they are",
                recovery.broken
            );
        }
        // Instances deleted outside the Launcher (in Finder or Explorer) since it last ran:
        // their images would keep their files from every collection. And the snapshot
        // store of Launchers from before images, which nothing reads any more (their world
        // backup zips are the user's files and stay where they are).
        let forgetting = images.clone();
        let old_store = data_dir.clone();
        let sweeping = world_backups.clone();
        cx.background_spawn(async move {
            forgetting.empty_restore_trash(None);
            // What a crash left of a world backup being made or deleted.
            sweeping.sweep();
            if let Err(err) = forgetting.forget_deleted() {
                log::warn!("Couldn't forget the images of deleted instances: {err}");
            }
            launcher_core::disk::remove_old_snapshot_store(&old_store);
        })
        .detach();
        // What succeeded in the last few seconds is recorded before the process ends,
        // rather than found as "Changed by hand" at the next launch. On the window's
        // thread: GPUI waits only 200 ms for a quit handler's future. Returns at once when
        // nothing is queued.
        let settling = images.clone();
        cx.on_app_quit(move |_, _| {
            settling.settle(Duration::from_secs(6));
            async {}
        })
        .detach();
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
            busy_files: jobs::BusyCount::default(),
            saves_after_snapshot: HashMap::new(),
            undo_images: HashMap::new(),
            backgrounds: HashMap::new(),
            background_tasks: HashMap::new(),
            sidebar_banners: HashMap::new(),
            sidebar_banner_tasks: HashMap::new(),
            sidebar_banners_checked: None,
            list_generation: 0,
            linked: HashSet::new(),
            required_java: Default::default(),
            rosetta_notes: Default::default(),
            slideshow_revision: 0,
            background_shown: None,
            background_previous: None,
            hero_cache,
            loader_versions: HashMap::new(),
            loader_tasks: HashMap::new(),
            exports: Vec::new(),
            exporting: 0,
            preview_state: None,
            preview_press: None,
            shared_revision: 0,
            modrinth: Some(modrinth),
            curseforge,
            content_downloader,
            imports: Vec::new(),
            next_import: 0,
            copies: HashMap::new(),
            pending_downloads: HashMap::new(),
            snapshot_rows: HashMap::new(),
            snapshot_tasks: HashMap::new(),
            snapshot_revisions: HashMap::new(),
            modpack_updates: HashMap::new(),
            content_revisions: HashMap::new(),
            import_ticker: None,
            images,
            world_backups,
            world_backup_rows: HashMap::new(),
            world_backup_tasks: HashMap::new(),
            world_backup_revisions: HashMap::new(),
        };
        this.reload(cx);
        this.follow_snapshot_changes(cx);
        this.follow_world_backup_changes(cx);
        // Files left from an earlier run (snapshot exports, "Host this" uploads, "Download
        // this world" downloads) were made to be shared or read then: the dialog that
        // made them is gone. Renamed away at once (to a name no earlier run used), so one
        // started now never writes into a folder being deleted, then deleted in the
        // background with any such folder an earlier run quit before deleting.
        let mut doomed = Vec::new();
        for dir in this.data_dir.temporary_dirs() {
            let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let old = dir.with_file_name(format!("{name}-old-{}-{now}", std::process::id()));
            if std::fs::rename(&dir, &old).is_ok() {
                doomed.push(old);
            }
            let prefix = format!("{name}-old-");
            if let Some(parent) = dir.parent() {
                doomed.extend(
                    std::fs::read_dir(parent)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
                        .map(|e| e.path()),
                );
            }
        }
        doomed.sort();
        doomed.dedup();
        if !doomed.is_empty() {
            cx.background_spawn(async move {
                for old in doomed {
                    let _ = std::fs::remove_dir_all(old);
                }
            })
            .detach();
        }
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

    /// Whether a snapshot is taken after each play session of the instance `id`: its own
    /// setting, else Settings › Instance defaults' (off by default).
    pub fn snapshot_after_play(&self, id: &str) -> bool {
        self.list()
            .iter()
            .find(|instance| instance.id == id)
            .and_then(|instance| instance.snapshots.and_then(|s| s.after_play))
            .unwrap_or(self.settings.instance_defaults.snapshot_after_play)
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

    /// Whether the launcher is busy with work that quitting would stop half way: a
    /// modpack being imported or updated, an instance being copied (also with another
    /// version or loader), an export, files moved to the Trash.
    pub fn background_jobs(&self) -> bool {
        !self.imports.is_empty()
            || !self.modpack_updates.is_empty()
            || !self.file_ops.is_empty()
            || self.exporting > 0
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

    /// Whether a "Find it for me" search of the instance is saved: it has some of the mods
    /// turned off until it ends, so an update that replaces files waits for it. One look at
    /// the disk.
    pub fn finder_saved(&self, id: &str) -> bool {
        launcher_core::content::mod_finder::is_saved(&self.content_dirs(id).instance)
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
        let instance = self.create_unrecorded(name, version, loader, cx)?;
        self.record_created(&instance.id, labels::created(&instance.name), cx);
        Ok(instance)
    }

    /// [`Self::create`] without recording the first content image: for a caller that
    /// changes the instance next and records it first ([`record_created`]) in the task
    /// that then changes it, so nothing it adds is in "Created".
    pub(crate) fn create_unrecorded(
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
        self.exporting += 1;
        let id = id.to_string();
        self.exports.push(cx.spawn(async move |this, cx| {
            let result = write.await;
            this.update(cx, |this, cx| {
                this.exporting -= 1;
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
        self.save_now(instance, cx)
    }

    /// [`Self::save`] for a change the user makes to the instance's settings (the
    /// Settings tab, Undo, the CLI): a change of its game ([`game_change`]: a new version
    /// of its loader) takes a snapshot first ([`Self::save_after_snapshot`]) and returns
    /// the save that waits for it; anything else is saved at once (`None`). A modpack's or
    /// server's update saves with [`Self::save`]: it took its own snapshot before it
    /// changed files.
    pub fn save_settings(
        &mut self,
        instance: &Instance,
        cx: &mut Context<Self>,
    ) -> Result<Option<PendingSave>, String> {
        // A game change still waits for its snapshot: this save follows it (compared then
        // with what it saved), so neither is dropped for the other.
        if let Some(queued) = self.saves_after_snapshot.get_mut(&instance.id) {
            *queued = Some(instance.clone());
            self.update_listed(instance.clone(), cx);
            return Ok(None);
        }
        let before = self.store.get(&instance.id).ok();
        if let Some((before, change)) =
            before.and_then(|before| game_change(&before, instance).map(|c| (before, c)))
        {
            return Ok(Some(self.save_after_snapshot(
                before,
                instance.clone(),
                change,
                cx,
            )));
        }
        self.save_now(instance, cx).map(|()| None)
    }

    /// A change of the instance's game is a risky change (the next launch may change the
    /// worlds and configs): a snapshot is taken first, in the background, and
    /// `instance.json` saved once it is done. Meanwhile the list shows the new version,
    /// and Play waits (the instance's files are busy). When something else saved the
    /// instance meanwhile, this change is dropped rather than undo that one. A snapshot
    /// that fails doesn't stop the save (as for every risky change); it, a dropped change
    /// and a save that fails are each said in a notice, and the returned task says them
    /// to a caller that waits (the control API).
    pub(crate) fn save_after_snapshot(
        &mut self,
        before: Instance,
        instance: Instance,
        change: GameChange,
        cx: &mut Context<Self>,
    ) -> PendingSave {
        let busy = self.mark_files_busy(&instance.id);
        self.saves_after_snapshot.insert(instance.id.clone(), None);
        self.update_listed(instance.clone(), cx);
        let images = self.images.clone();
        let id = instance.id.clone();
        let game = instance.game_version.clone();
        let loader = instance.loader;
        let version = instance.loader_version.clone();
        let taken = cx.background_spawn(async move {
            let change = match change {
                GameChange::LoaderVersion => launcher_core::images::RiskyChange::LoaderChange {
                    loader,
                    version: version.as_deref().unwrap_or_default(),
                },
                GameChange::Game => launcher_core::images::RiskyChange::GameChange {
                    game_version: &game,
                    loader,
                    loader_version: version.as_deref(),
                },
            };
            images.before_risky_change(&id, change)
        });
        let (tx, rx) = futures::channel::oneshot::channel();
        cx.spawn(async move |this, cx| {
            let snapshot_failed = taken.await.err().map(|err| err.to_string());
            let result = this.update(cx, |this, cx| {
                let _busy = busy;
                let about = About::Instance(instance.id.clone());
                if let Some(err) = &snapshot_failed {
                    log::warn!(
                        "Couldn't take a snapshot of {} before its loader changed: {err}",
                        instance.id
                    );
                    cx.emit(InstancesEvent::Notice {
                        title: t(M::NoSnapshotBeforeGameChange).into(),
                        message: err.clone().into(),
                        about: about.clone(),
                    });
                }
                let saved = if this.store.get(&instance.id).ok().as_ref() != Some(&before) {
                    log::warn!(
                        "{} was saved while its snapshot was taken; the loader change is dropped",
                        instance.id
                    );
                    Err(t(M::LoaderChangeDropped).to_string())
                } else {
                    this.save_now(&instance, cx)
                };
                if let Err(err) = &saved {
                    log::warn!("Couldn't save {}: {err}", instance.id);
                    this.reload(cx);
                    cx.emit(InstancesEvent::Notice {
                        title: t(M::LoaderChangeNotSaved).into(),
                        message: err.clone().into(),
                        about,
                    });
                }
                // Settings saved while the snapshot was taken follow, compared with what
                // was just saved (another game change takes its own snapshot).
                if let Some(Some(queued)) = this.saves_after_snapshot.remove(&instance.id)
                    && let Err(err) = this.save_settings(&queued, cx)
                {
                    log::warn!("Couldn't save {}: {err}", instance.id);
                    cx.emit(InstancesEvent::Notice {
                        title: t(M::LoaderChangeNotSaved).into(),
                        message: err.into(),
                        about: About::Instance(instance.id.clone()),
                    });
                }
                saved.map(|()| snapshot_failed)
            });
            if let Ok(result) = result {
                tx.send(result).ok();
            }
        })
        .detach();
        files::answer(rx, cx)
    }

    /// Saves `instance.json` at once, recording the change of content it makes.
    fn save_now(&mut self, instance: &Instance, cx: &mut Context<Self>) -> Result<(), String> {
        let before = self.store.get(&instance.id).ok();
        let operation = before
            .as_ref()
            .and_then(|before| self.settings_operation(before, instance));
        self.store.save(instance).map_err(|err| err.to_string())?;
        if let Some(operation) = operation {
            operation.succeed();
        }
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
        self.version_java(game_version).map(|(major, _)| major)
    }

    /// The Java `game_version` runs in when the instance chooses none, if its version
    /// file is installed: on an Apple silicon Mac it may not be the one the version
    /// names ([`install::default_java`]).
    pub fn default_java(&self, game_version: &str) -> Option<install::DefaultJava> {
        self.version_java(game_version).map(|(_, java)| java)
    }

    fn version_java(&self, game_version: &str) -> Option<VersionJava> {
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
                .map(|version| {
                    (
                        version.java_major(),
                        install::default_java(&version, &self.machine),
                    )
                })
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

    /// Gives back files the Content tab took with [`Self::take_pending_downloads`] and
    /// that aren't in the instance yet, when it stops showing the instance: its card
    /// comes back with them. Files left since (a pack added meanwhile) follow them.
    pub fn put_back_pending_downloads(
        &mut self,
        id: &str,
        files: Vec<launcher_core::content::curseforge_install::CfPlannedFile>,
    ) {
        // an instance that is gone has no card to show (and a new one may get its id)
        if files.is_empty() || !self.list.iter().any(|instance| instance.id == id) {
            return;
        }
        let pending = self.pending_downloads.entry(id.to_string()).or_default();
        let newer = std::mem::replace(pending, files);
        pending.extend(newer);
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

    /// The instances' content images, for actions that change content in the background.
    pub fn images(&self) -> Arc<Images> {
        self.images.clone()
    }

    /// The operation saving `after` over `before` is in the instance's content images, if
    /// any: the game (Minecraft version, loader and loader version) and the recommended
    /// memory are in them, so a change from Settings, Undo, a revert, a modpack's or a
    /// server's update, or the CLI is an operation (merged with the update's own when it is
    /// one), and the check before the next launch doesn't take it for a change by hand.
    /// Succeed it once the instance is saved.
    pub(crate) fn settings_operation(
        &self,
        before: &Instance,
        after: &Instance,
    ) -> Option<launcher_core::images::Operation> {
        let minecraft =
            (before.game_version != after.game_version).then_some(after.game_version.as_str());
        let loader = (before.loader != after.loader
            || before.loader_version != after.loader_version)
            .then_some((after.loader, after.loader_version.as_deref()));
        let label = match labels::game_changed(minecraft, loader, before.loader != after.loader) {
            Some(label) => label,
            None if before.recommended_memory != after.recommended_memory => {
                labels::MEMORY_CHANGED.to_string()
            }
            None => return None,
        };
        Some(self.images.begin(&after.id, label))
    }

    /// Records the first content image of the new instance `id`, in the background.
    pub(crate) fn record_created(&self, id: &str, label: String, cx: &mut Context<Self>) {
        let images = self.images.clone();
        let id = id.to_string();
        cx.background_spawn(async move { record_created(&images, &id, &label) })
            .detach();
    }

    /// Keeps the Content tab off the network, for the offscreen screenshots.
    pub fn preview_offline(&mut self) {
        self.modrinth = None;
        self.curseforge = None;
    }

    /// Has the Content tab offer Modrinth's projects while staying off the network:
    /// every request to it fails (the list says Modrinth couldn't be reached), for
    /// the offscreen screenshots of "Looking for something new?".
    pub fn preview_modrinth_unreachable(&mut self) {
        self.modrinth = Some(Arc::new(Modrinth::with_urls(
            Arc::new(http_client::BlockedHttpClient::new()),
            url::Url::parse(launcher_core::content::modrinth::API_BASE).expect("a valid URL"),
            None,
        )));
    }

    /// Where the base packs of servers' images are cached
    /// (`launcher_core::cores::image_sync::base`).
    pub fn server_pack_cache(&self) -> std::path::PathBuf {
        self.data_dir.server_pack_cache_dir()
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

    /// The mod loader to offer a vanilla instance on `game_version` (for a copy with
    /// it): Fabric, or Forge where Fabric doesn't run (Fabric starts at Minecraft 1.14),
    /// with the version to use. `None` when neither has a build for it.
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

    /// Makes `id`'s game prepared and waiting for the Rosetta question, as
    /// [`Self::start_or_ask`] leaves it, and asks it; for tests. Nothing can start: the
    /// command runs a program that doesn't exist.
    #[cfg(test)]
    pub fn test_ask_rosetta(&mut self, id: &str, cx: &mut Context<Self>) {
        use launcher_core::game::{
            launch::{self, GameSettings, LaunchAccount, LaunchPaths},
            version::VersionJson,
        };
        let version = VersionJson::parse(
            br#"{"id": "1.12.2", "mainClass": "net.minecraft.client.main.Main",
                 "minecraftArguments": "--username ${auth_player_name}"}"#,
        )
        .unwrap();
        let paths = LaunchPaths {
            java: "/nonexistent/java".into(),
            game_dir: "/nonexistent/game".into(),
            natives_dir: "/nonexistent/natives".into(),
            libraries_dir: "/nonexistent/libraries".into(),
            assets_dir: "/nonexistent/assets".into(),
            game_assets: "/nonexistent/assets".into(),
            classpath: Vec::new(),
            logging_config: None,
        };
        let account = LaunchAccount {
            username: "Tester".into(),
            uuid: Default::default(),
            access_token: launcher_core::Secret::new("test"),
            xuid: None,
            user_type: launcher_core::game::launch::UserType::Legacy,
        };
        let command = launch::build(
            &version,
            &self.machine,
            &paths,
            &account,
            &GameSettings::default(),
        )
        .unwrap();
        self.preview_preparing(id, Arc::new(InstallProgress::default()));
        if let Some(run) = self.runs.get_mut(id) {
            run.pending = Some(PendingStart {
                command,
                rosetta: Rosetta::Version,
                outside: Vec::new(),
            });
        }
        cx.emit(InstancesEvent::ConfirmRosetta {
            id: id.to_string(),
            version: "1.12.2".into(),
            reason: Rosetta::Version,
        });
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
        // its folder is on its way to the Trash, or a tab is still installing mods or
        // restoring a world into it
        if self.file_op(id) == Some(FileOp::Delete) || self.files_busy(id) {
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntStartMinecraft).into(),
                message: t(M::FilesBusy).into(),
                about: About::Instance(id.to_string()),
            });
            return;
        }
        // A restore whose rollback failed (at startup, or while it ran) may have left
        // worlds or mods renamed aside: the game would start without them and write fresh
        // ones. Undone now if it can be (renames only, so on this thread, with the work
        // folders deleted in the background); Play waits until it is.
        if self.images.restore_unfinished(id) {
            if let Err(err) = self
                .images
                .retry_unfinished_restore(id, Duration::from_millis(300))
            {
                log::warn!("Play of {id} waits for an unfinished restore: {err}");
                cx.emit(InstancesEvent::LaunchFailed {
                    title: t(M::CouldntStartMinecraft).into(),
                    message: t(M::RestoreUnfinished).into(),
                    about: About::Instance(id.to_string()),
                });
                return;
            }
            let images = self.images.clone();
            let instance = id.to_string();
            cx.background_spawn(async move { images.empty_restore_trash(Some(&instance)) })
                .detach();
        }
        let Ok(mut instance) = self.store.get(id) else {
            return;
        };
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let run = self.runs.entry(id.to_string()).or_default();
        run.reached_menu = false;
        run.mods_refused = false;
        run.log.start_run();
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
        let images = self.images.clone();
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
        let stop = cancel.clone();
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
            // Content changed by hand since the last record is recorded before the game
            // can change anything (see `Images::before_launch`); it never stops a launch.
            let checked = cx
                .background_spawn({
                    let images = images.clone();
                    let id = id.clone();
                    async move { images.before_launch(&id) }
                })
                .await;
            this.update(cx, |this, _| {
                let note = content_check_note(&checked);
                match this.runs.get_mut(&id) {
                    Some(run) if !matches!(run.state, RunState::Idle) => {
                        run.image_session = true;
                        if let Some((level, note)) = note {
                            run.log.note(level, note);
                        }
                    }
                    // The run ended while the check ran.
                    _ => images.session_ended(&id),
                }
            })
            .ok();
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
                    // not worth the scan once cancelled
                    let outside = if find_outside && !cancel.is_cancelled() {
                        launcher_core::configurator::outside_candidates()
                    } else {
                        Vec::new()
                    };
                    Ok::<_, InstallError>((command, prepared.rosetta, outside))
                })
                .await;
            this.update(cx, |this, cx| match prepared {
                // Cancelled after the last check that would have seen it (making the
                // natives or the command): not started either.
                Ok(_) if stop.is_cancelled() => this.prepare_cancelled(&id, cx),
                Ok((command, rosetta, outside)) => {
                    this.take_notes(&id);
                    if let Some(run) = this.runs.get_mut(&id) {
                        run.log.note(
                            Level::Info,
                            check_summary(&stats_progress, check_started.elapsed()),
                        );
                    }
                    this.start_or_ask(&id, command, rosetta, outside, cx)
                }
                Err(InstallError::Cancelled) => this.prepare_cancelled(&id, cx),
                Err(InstallError::RosettaMissing) => this.fail(
                    &id,
                    t(M::RosettaNeeded).into(),
                    rosetta_missing().into(),
                    cx,
                ),
                Err(InstallError::NoNativeJava8) => this.fail(
                    &id,
                    t(M::CouldntStartMinecraft).into(),
                    t(M::NoNativeJava8).into(),
                    cx,
                ),
                Err(InstallError::NeedsIntelJava(version)) => this.fail(
                    &id,
                    t(M::CouldntStartMinecraft).into(),
                    tr!(M::NeedsIntelJava, version = version).into(),
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
        let after_play = self.snapshot_after_play(id);
        if let Some(run) = self.runs.get_mut(id) {
            // Only a run whose game started is a play session to take a snapshot after:
            // not a launch that failed or was cancelled while it was prepared.
            let played = matches!(run.state, RunState::Running { .. });
            let after_play = after_play && played;
            run.state = RunState::Idle;
            run.task = None;
            run.ticker = None;
            // However the run ended (the game exited or crashed, or it never started):
            // what the game wrote joins the next content image, not one of its own; and a
            // snapshot follows when the instance asks for one after each session.
            if std::mem::take(&mut run.image_session) {
                self.images.session_ended_with(id, after_play);
            }
        }
        cx.notify();
    }

    /// The game's preparation was cancelled: nothing started.
    fn prepare_cancelled(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.get_mut(id) {
            run.log.note(Level::Warn, t(M::Cancelled));
        }
        self.set_idle(id, cx);
        cx.emit(InstancesEvent::PlayCancelled(id.to_string()));
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
        cx.emit(InstancesEvent::PlayFailed {
            id: id.to_string(),
            title,
            message,
        });
    }

    /// Starts the prepared game, unless it runs under Rosetta 2 on this Apple silicon
    /// Mac for the first time for the instance: then the player is asked first
    /// ([`InstancesEvent::ConfirmRosetta`]), and it starts only once they agree
    /// ([`Self::confirm_rosetta`]). Whether, and why, it runs under Rosetta is recorded
    /// for the instance's header either way. The launcher never runs a game under
    /// Rosetta without saying so.
    fn start_or_ask(
        &mut self,
        id: &str,
        command: LaunchCommand,
        rosetta: Option<Rosetta>,
        outside: Vec<launcher_core::configurator::Outside>,
        cx: &mut Context<Self>,
    ) {
        if self.machine.os != Os::MacOs || self.machine.arch != Arch::Arm64 {
            return self.start(id, command, rosetta, outside, cx);
        }
        let ask = match self.store.record_rosetta(id, rosetta) {
            Ok((instance, ask)) => {
                let version = instance.game_version.clone();
                self.update_listed(instance, cx);
                ask.then_some(version)
            }
            Err(err) => {
                // Not saved: asked all the same, unless the player already agreed.
                eprintln!("lodestone-launcher: saving the instance {id}: {err}");
                let instance = self.store.get(id).ok();
                let told = |reason| {
                    instance
                        .as_ref()
                        .is_some_and(|i| i.told_about_rosetta(reason))
                };
                rosetta
                    .is_some_and(|reason| !told(reason))
                    .then(|| instance.map(|i| i.game_version).unwrap_or_default())
            }
        };
        match (ask, rosetta) {
            (Some(version), Some(reason)) => {
                let Some(run) = self.runs.get_mut(id) else {
                    return;
                };
                run.pending = Some(PendingStart {
                    command,
                    rosetta: reason,
                    outside,
                });
                cx.emit(InstancesEvent::ConfirmRosetta {
                    id: id.to_string(),
                    version,
                    reason,
                });
                cx.notify();
            }
            _ => self.start(id, command, rosetta, outside, cx),
        }
    }

    /// The player's answer to [`InstancesEvent::ConfirmRosetta`]: `play` starts the
    /// waiting game, and it isn't asked again for the instance and this reason;
    /// otherwise nothing starts, and it is asked again next time. Nothing happens when
    /// no game waits (it was cancelled meanwhile).
    pub fn confirm_rosetta(&mut self, id: &str, play: bool, cx: &mut Context<Self>) {
        let Some(pending) = self.runs.get_mut(id).and_then(|run| run.pending.take()) else {
            return;
        };
        if !play {
            self.prepare_cancelled(id, cx);
            return;
        }
        match self.store.record_told_rosetta(id, pending.rosetta) {
            Ok(instance) => self.update_listed(instance, cx),
            // Only the answer is lost: asked again next time.
            Err(err) => eprintln!("lodestone-launcher: saving the instance {id}: {err}"),
        }
        self.start(
            id,
            pending.command,
            Some(pending.rosetta),
            pending.outside,
            cx,
        );
    }

    /// Whether this is an Apple silicon Mac, where some versions run in another Java
    /// than the one they name, or under Rosetta 2.
    pub fn on_apple_silicon(&self) -> bool {
        self.machine.os == Os::MacOs && self.machine.arch == Arch::Arm64
    }

    /// Whether, and why, the instance's game runs under Rosetta 2 on this Mac, for its
    /// header: what its last preparation for this version found, or before that, what
    /// its installed version file and its Java say ([`install::rosetta_for`]). `None`
    /// on other machines, and when nothing is known yet (the version isn't installed:
    /// the player is asked before the first game anyway).
    pub fn rosetta_note(&self, instance: &Instance) -> Option<Rosetta> {
        if !self.on_apple_silicon() {
            return None;
        }
        if let Some(prepared) = instance.prepared_rosetta() {
            return prepared;
        }
        let version = &instance.game_version;
        if version.is_empty() || version.contains(['/', '\\']) || version.starts_with('.') {
            return None;
        }
        let key = (version.clone(), instance.java_override.clone());
        if let Some((rosetta, found, generation)) = self.rosetta_notes.borrow().get(&key)
            && (*found || *generation == self.list_generation)
        {
            return *rosetta;
        }
        let path = self
            .data_dir
            .versions_dir()
            .join(version)
            .join(format!("{version}.json"));
        let json = std::fs::read(path)
            .ok()
            .and_then(|bytes| launcher_core::game::version::VersionJson::parse(&bytes).ok());
        let java = match &instance.java_override {
            Some(path) => JavaChoice::Override(path.clone()),
            None => JavaChoice::Mojang,
        };
        let rosetta = json
            .as_ref()
            .and_then(|json| install::rosetta_for(json, &self.machine, &java));
        self.rosetta_notes
            .borrow_mut()
            .insert(key, (rosetta, json.is_some(), self.list_generation));
        rosetta
    }

    fn start(
        &mut self,
        id: &str,
        command: LaunchCommand,
        rosetta: Option<Rosetta>,
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
        if rosetta.is_some() {
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
                let recent = run.log.run_recent();
                let log_text = run.log.run_text(|_| true);
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

    /// Whether the instance's prepared game waits for the player's answer to
    /// [`InstancesEvent::ConfirmRosetta`].
    pub fn waits_for_rosetta(&self, id: &str) -> bool {
        self.runs.get(id).is_some_and(|run| run.pending.is_some())
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
                // Prepared, waiting for the player to agree to Rosetta 2: nothing else
                // would end the run.
                if run.pending.take().is_some() {
                    self.prepare_cancelled(id, cx);
                    return;
                }
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

/// Records the first content image of the new instance `id` (blocking: for a background
/// task that just made it and changes it next). A failure is only logged: the check before
/// its first launch records it then.
pub(crate) fn record_created(images: &Images, id: &str, label: &str) {
    if let Err(err) = images.record_now(id, label) {
        log::warn!("Couldn't record the content of {id}: {err}");
    }
}

/// [`record_created`] on a thread of its own, for an action that is done once the instance
/// is made (Duplicate, an import): hashing a big pack's every file would hold its spinner.
pub(crate) fn record_created_later(images: &Arc<Images>, id: &str, label: &str) {
    let (images, id, label) = (images.clone(), id.to_string(), label.to_string());
    let spawned = std::thread::Builder::new()
        .name("lodestone-images-created".into())
        .spawn(move || record_created(&images, &id, &label));
    if let Err(err) = spawned {
        log::warn!("Couldn't record the content of a new instance: {err}");
    }
}

/// What the check before a launch did, as a note for the run's log: only a recorded
/// change, or a failure.
fn content_check_note(checked: &Checked) -> Option<(Level, String)> {
    match checked {
        Checked::Recorded(recorded) if recorded.changed => {
            Some((Level::Info, t(M::ContentRecordedNote).to_string()))
        }
        Checked::Failed(err) => Some((Level::Warn, tr!(M::ContentCheckFailedNote, error = err))),
        _ => None,
    }
}

/// Creates the instances entity for the app, in the same data directory as the
/// accounts.
pub fn create(accounts: &Entity<Accounts>, cx: &mut App) -> Entity<Instances> {
    let data_dir = accounts.read(cx).data_dir().clone();
    let accounts = accounts.clone();
    let instances = cx.new(|cx| Instances::new(data_dir, accounts, cx));
    // The first content image of each instance from before the Launcher recorded them,
    // so the first change to one has an image holding what it replaces. Only in the app:
    // the offscreen scenes and tests make their own.
    let images = instances.read(cx).images();
    cx.background_spawn(async move {
        let recorded = images.record_first_images();
        if recorded > 0 {
            log::info!("Recorded the first content image of {recorded} instances");
        }
    })
    .detach();
    instances
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

/// A save of an instance's settings that waits for the snapshot before it
/// ([`Instances::save_after_snapshot`]): `Ok` with the snapshot's error when it failed and
/// the save went on, `Err` when the change was dropped or couldn't be saved.
pub type PendingSave = Task<Result<Option<String>, String>>;

/// What a save of an instance's settings changes of its game, when it does: the changes a
/// snapshot comes before in [`Instances::save_settings`]. Other fields (memory, name)
/// changing in the same save don't matter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameChange {
    /// A new version of its loader (the Settings tab, Undo, `instances
    /// set-loader-version`).
    LoaderVersion,
    /// Another Minecraft version or loader. The Launcher never makes this change in
    /// place (a copy with another version or loader is a new instance; a pack's or
    /// server's update takes its own snapshot), but a save that did would upgrade the
    /// worlds at the next launch, so it is snapshotted too.
    Game,
}

/// See [`GameChange`].
pub(crate) fn game_change(before: &Instance, after: &Instance) -> Option<GameChange> {
    if before.game_version != after.game_version || before.loader != after.loader {
        Some(GameChange::Game)
    } else if before.loader != launcher_core::instances::Loader::None
        && before.loader_version != after.loader_version
        && after.loader_version.is_some()
    {
        Some(GameChange::LoaderVersion)
    } else {
        None
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

    /// A crash is worked out from its own run: an earlier run's exception, causes and
    /// last line, still in the session's log, are left out (LS-59).
    #[test]
    fn a_run_reads_only_its_own_lines() {
        let mut log = GameLog::default();
        log.start_run();
        log.push(LogLine::plain(Stream::Stdout, "Stopping!"));
        log.push(LogLine::plain(
            Stream::Stderr,
            "Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space",
        ));
        log.note(Level::Error, "Exited with code 1");
        assert!(log.latest_game_line().is_some());

        log.start_run();
        log.note(Level::Info, "Preparing 1.21.1");
        assert_eq!(log.latest_game_line(), None, "not the last run's line");
        log.push(LogLine::plain(
            Stream::Stderr,
            "# A fatal error has been detected by the Java Runtime Environment:",
        ));
        log.note(Level::Error, "Exited with code 134");
        let recent = log.run_recent();
        assert_eq!(recent.len(), 3);
        let summary = crash::summarize(None, &recent);
        assert_eq!(summary.exception, None, "{summary:?}");
        assert!(
            !summary.causes.contains(&crash::Cause::OutOfMemory),
            "{summary:?}"
        );
        let text = log.run_text(|_| true);
        assert!(text.contains("Preparing 1.21.1"), "{text}");
        assert!(!text.contains("OutOfMemoryError"), "{text}");
        assert!(
            log.to_text(|_| true).contains("OutOfMemoryError"),
            "the console keeps it"
        );

        // A run longer than what is kept still reads its last events.
        log.start_run();
        for n in 0..600 {
            log.push(LogLine::plain(Stream::Stdout, &format!("line {n}")));
        }
        assert_eq!(log.run_recent().len(), 500);
        assert_eq!(log.latest_game_line().as_deref(), Some("line 599"));
    }
}

#[cfg(test)]
mod game_change_tests {
    use super::*;
    use launcher_core::instances::Loader;

    fn fabric() -> Instance {
        Instance {
            game_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.9".into()),
            ..Instance::default()
        }
    }

    #[test]
    fn a_new_loader_version_is_a_game_change_whatever_else_changes() {
        let before = fabric();
        let mut after = before.clone();
        after.loader_version = Some("0.16.14".into());
        assert_eq!(
            game_change(&before, &after),
            Some(GameChange::LoaderVersion)
        );
        // memory and the name saved with it don't hide it
        after.memory = Some(launcher_core::instances::Memory {
            min: 1024,
            max: 4096,
        });
        after.name = "Renamed".into();
        assert_eq!(
            game_change(&before, &after),
            Some(GameChange::LoaderVersion)
        );
    }

    #[test]
    fn another_minecraft_version_or_loader_is_a_game_change() {
        let before = fabric();
        let mut game = before.clone();
        game.game_version = "1.21.4".into();
        assert_eq!(game_change(&before, &game), Some(GameChange::Game));
        let mut loader = before.clone();
        loader.loader = Loader::Quilt;
        loader.loader_version = Some("0.27.1".into());
        assert_eq!(game_change(&before, &loader), Some(GameChange::Game));
        let mut vanilla = before.clone();
        vanilla.loader = Loader::None;
        vanilla.loader_version = None;
        assert_eq!(game_change(&before, &vanilla), Some(GameChange::Game));
    }

    #[test]
    fn other_settings_arent_a_game_change() {
        let before = fabric();
        let mut after = before.clone();
        after.name = "Renamed".into();
        after.memory = Some(launcher_core::instances::Memory {
            min: 1024,
            max: 4096,
        });
        assert_eq!(game_change(&before, &after), None);
        // a loader version for no loader, or none at all, isn't one either
        let mut vanilla = before.clone();
        vanilla.loader = Loader::None;
        vanilla.loader_version = None;
        let mut odd = vanilla.clone();
        odd.loader_version = Some("1".into());
        assert_eq!(game_change(&vanilla, &odd), None);
        let mut cleared = before.clone();
        cleared.loader_version = None;
        assert_eq!(game_change(&before, &cleared), None);
    }
}

/// Saving a new loader version from Settings, with the sample instances of the offscreen
/// screenshots: the snapshot first, then the save, or the change dropped when the instance
/// was saved meanwhile.
#[cfg(all(test, feature = "screenshot"))]
mod save_tests {
    use gpui::TestAppContext;
    use launcher_core::images::SnapshotKind;
    use launcher_core::instances::Loader;

    use super::*;

    fn setup(cx: &mut TestAppContext) -> (Entity<Instances>, String) {
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        cx.run_until_parked();
        let id = instances.read_with(cx, |instances, _| instances.list()[0].id.clone());
        // a Fabric instance, saved as a modpack update would (no snapshot)
        instances.update(cx, |this, cx| {
            let mut instance = this.store.get(&id).unwrap();
            instance.loader = Loader::Fabric;
            instance.loader_version = Some("0.16.9".into());
            this.save(&instance, cx).unwrap();
        });
        (instances, id)
    }

    fn new_loader_version(
        instances: &Entity<Instances>,
        id: &str,
        cx: &mut TestAppContext,
    ) -> PendingSave {
        instances.update(cx, |this, cx| {
            let mut instance = this.store.get(id).unwrap();
            instance.loader_version = Some("0.16.14".into());
            instance.name = "Renamed too".into();
            this.save_settings(&instance, cx)
                .unwrap()
                .expect("a new loader version waits for a snapshot")
        })
    }

    #[gpui::test]
    fn a_restore_waits_for_a_modpack_update_and_an_install(cx: &mut TestAppContext) {
        let (instances, id) = setup(cx);
        let busy = |cx: &mut TestAppContext| {
            instances.read_with(cx, |this, _| this.files_or_pack_busy(&id))
        };
        assert!(!busy(cx));
        // a modpack update being applied: it marks no files busy, but restores wait for it
        instances.update(cx, |this, _| {
            this.modpack_updates.insert(
                id.clone(),
                modpacks::UpdateJob {
                    progress: Default::default(),
                    cancel: CancelToken::new(),
                    _task: gpui::Task::ready(()),
                },
            );
        });
        assert!(busy(cx));
        instances.update(cx, |this, _| {
            this.modpack_updates.remove(&id);
        });
        assert!(!busy(cx));
        // a tab's install
        let install = instances.read_with(cx, |this, _| this.mark_files_busy(&id));
        assert!(busy(cx));
        drop(install);
        assert!(!busy(cx));
    }

    #[gpui::test]
    fn a_new_loader_version_is_saved_after_a_snapshot(cx: &mut TestAppContext) {
        let (instances, id) = setup(cx);
        let pending = new_loader_version(&instances, &id, cx);
        assert!(
            instances.read_with(cx, |this, _| this.files_busy(&id)),
            "Play waits for the snapshot"
        );
        cx.run_until_parked();
        assert_eq!(futures::executor::block_on(pending), Ok(None));
        let saved = instances.read_with(cx, |this, _| this.store.get(&id).unwrap());
        assert_eq!(saved.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(saved.name, "Renamed too");
        let images = instances.read_with(cx, |this, _| this.images());
        let records = images.snapshots(&id).unwrap().records;
        assert_eq!(records[0].kind, SnapshotKind::BeforeChange);
        assert_eq!(records[0].name, "Before changing Fabric to 0.16.14");
    }

    #[gpui::test]
    fn a_save_meanwhile_wins_and_the_loader_change_says_it_was_dropped(cx: &mut TestAppContext) {
        let (instances, id) = setup(cx);
        let pending = new_loader_version(&instances, &id, cx);
        // the CLI renames it while the snapshot is taken
        instances.read_with(cx, |this, _| {
            let mut instance = this.store.get(&id).unwrap();
            instance.name = "Meanwhile".into();
            this.store.save(&instance).unwrap();
        });
        cx.run_until_parked();
        let result = futures::executor::block_on(pending);
        assert_eq!(
            result,
            Err(crate::i18n::t(crate::i18n::InstancesMsg::LoaderChangeDropped).to_string())
        );
        let saved = instances.read_with(cx, |this, _| this.store.get(&id).unwrap());
        assert_eq!(saved.name, "Meanwhile");
        assert_eq!(saved.loader_version.as_deref(), Some("0.16.9"));
        let listed = instances.read_with(cx, |this, _| {
            this.list()
                .iter()
                .find(|i| i.id == id)
                .map(|i| i.loader_version.clone())
        });
        assert_eq!(
            listed,
            Some(Some("0.16.9".into())),
            "the list shows the saved one"
        );
    }
}
