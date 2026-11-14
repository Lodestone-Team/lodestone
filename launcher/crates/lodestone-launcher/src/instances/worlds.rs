//! The Worlds tab: the instance's worlds with their backups and data packs, and its
//! screenshots.
//!
//! The work is `launcher_core::worlds`, `launcher_core::datapacks` and
//! `launcher_core::screenshots`; this view runs it in the background, one job at a time,
//! and refuses changes while the game runs. Deleting a world and replacing it with a
//! backup ask the window for a confirmation ([`WorldsEvent`]).
//!
//! A world's row opens one panel at a time below it: its backups, or its data packs
//! (turned on and off in the world's own list, added from files, or found on Modrinth).

use crate::components::RichText;
use crate::skin::Skinned;
use crate::theme::px;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use gpui::{
    AnyElement, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, ExternalPaths,
    Image, ImageFormat, IntoElement, ObjectFit, PathPromptOptions, Render, RenderImage,
    SharedString, Subscription, Task, Window, div, img, prelude::*,
};
use launcher_core::{
    content::{InstanceDirs, packs::Compatibility},
    datapacks::{self, PackState, WorldDataPacks},
    game::download::{CancelToken, Progress},
    screenshots::{self, Screenshot},
    worlds::{self, Backup, RestoreMode, World, WorldError},
};
use lodestone_minecraft::{
    level::GameMode,
    modrinth::{ClientProjectType, ClientSearchParams, ModrinthProject, ModrinthSearchHit},
};

use super::{
    Instances,
    content::{
        Load, RemoteIcon,
        view::{badge, card, list_box, muted, pills, row_base, small, small_rich},
    },
    jobs::{BusyFiles, Parked},
};
use crate::components::{
    KeyboardFocus,
    a11y::{Accessible, Role},
};
use crate::i18n::{CommonMsg, ContentMsg, WorldsMsg as M, WorldsPlural as P, format, t, tr, trn};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, LoadingDots, TextInput, Toggle},
    file_dialog,
    theme::{self, colors, fonts, radius, text, weights},
};

/// Screenshot thumbnails are at most this many pixels on a side.
const THUMB_PIXELS: u32 = 480;
/// The data pack search waits this long after the last key.
const SEARCH_DELAY: Duration = Duration::from_millis(300);
/// The data pack search shows this many results.
const PACK_RESULTS: u32 = 8;

/// The panel open below a world's row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorldPanel {
    #[default]
    Backups,
    DataPacks,
}

/// A data pack found on Modrinth.
#[derive(Clone, Debug, PartialEq)]
pub struct PackHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
}

impl From<ModrinthSearchHit> for PackHit {
    fn from(hit: ModrinthSearchHit) -> Self {
        Self {
            project_id: hit.project_id,
            title: hit.title,
            description: hit.description,
            author: hit.author,
            icon_url: hit.icon_url,
            downloads: hit.downloads,
        }
    }
}

impl From<ModrinthProject> for PackHit {
    fn from(project: ModrinthProject) -> Self {
        Self {
            project_id: project.id,
            title: project.title,
            description: project.description,
            author: String::new(),
            icon_url: project.icon_url,
            downloads: project.downloads,
        }
    }
}
/// The large view decodes at most this many pixels on a side.
const LARGE_PIXELS: u32 = 2560;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Worlds,
    Screenshots,
}

/// What the Worlds tab asks the window to confirm.
pub enum WorldsEvent {
    Delete {
        folder: String,
        name: String,
    },
    Replace {
        folder: String,
        name: String,
        backup: Backup,
    },
}

/// What a job is doing, shown over its progress bar in the current language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobLabel {
    BackingUp(String),
    Restoring,
    Replacing,
    MovingToTrash(String),
    MovingBackupToTrash,
    Renaming,
    Exporting(String),
    Importing,
    AddingDataPacks,
    InstallingDataPack(String),
    SavingPackList,
}

impl JobLabel {
    pub fn text(&self) -> String {
        let trash = t(CommonMsg::TheTrash);
        match self {
            JobLabel::BackingUp(name) => tr!(M::BackingUp, name = name),
            JobLabel::Restoring => t(M::RestoringBackup).to_string(),
            JobLabel::Replacing => t(M::ReplacingWorld).to_string(),
            JobLabel::MovingToTrash(name) => tr!(M::MovingToTrash, name = name, trash = trash),
            JobLabel::MovingBackupToTrash => tr!(M::MovingBackupToTrash, trash = trash),
            JobLabel::Renaming => t(M::Renaming).to_string(),
            JobLabel::Exporting(name) => tr!(M::Exporting, name = name),
            JobLabel::Importing => t(M::ImportingWorlds).to_string(),
            JobLabel::AddingDataPacks => t(M::AddingDataPacks).to_string(),
            JobLabel::InstallingDataPack(name) => {
                tr!(crate::i18n::InstancesMsg::InstallingName, name = name)
            }
            JobLabel::SavingPackList => t(M::SavingPackList).to_string(),
        }
    }
}

/// A world change running in the background.
pub struct Job {
    pub label: JobLabel,
    pub progress: Arc<Progress>,
    pub cancel: CancelToken,
    _task: Task<()>,
    _ticker: Task<()>,
    /// Play waits while the job runs.
    _busy: BusyFiles,
}

enum Thumb {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

pub struct WorldsView {
    instances: Entity<Instances>,
    instance_id: Option<String>,
    pub section: Section,
    pub worlds: Load<Vec<World>>,
    icons: HashMap<String, Arc<RenderImage>>,
    /// The world whose panel is open, and which panel.
    pub expanded: Option<String>,
    pub panel: WorldPanel,
    backups: HashMap<String, Vec<Backup>>,
    /// Data packs, by world folder, read when a world's panel opens.
    pub datapacks: HashMap<String, Load<WorldDataPacks>>,
    /// `pack.png`s, by world folder and pack name.
    pack_icons: HashMap<(String, String), Arc<RenderImage>>,
    pack_search: Entity<TextInput>,
    /// What the search was last run with.
    pack_query: String,
    pub pack_results: Load<Vec<PackHit>>,
    hit_icons: HashMap<String, RemoteIcon>,
    pack_search_task: Option<Task<()>>,
    datapacks_task: Option<Task<()>>,
    /// The world being renamed, with its name field.
    renaming: Option<(String, Entity<TextInput>)>,
    /// The shown instance's job.
    pub job: Option<Job>,
    /// The jobs of instances not shown, which go on meanwhile: a backup or restore
    /// can't stop part-way, and a second job mustn't start on the same world.
    parked: Parked<Job>,
    /// How jobs went that finished while their instance wasn't shown, by instance.
    finished: HashMap<String, (SharedString, bool, Section)>,
    /// The last result, whether it is an error, and the section it is about. It is
    /// only shown in that section.
    pub message: Option<(SharedString, bool, Section)>,
    pub screenshots: Load<Vec<Screenshot>>,
    thumbs: HashMap<PathBuf, (Option<SystemTime>, Thumb)>,
    /// The screenshot shown large, by index, and its image.
    pub viewing: Option<usize>,
    large: Option<(PathBuf, Load<Arc<RenderImage>>)>,
    confirm_delete_shot: Option<String>,
    was_active: bool,
    load_task: Option<Task<()>>,
    shots_task: Option<Task<()>>,
    large_task: Option<Task<()>>,
    _prompt: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl EventEmitter<WorldsEvent> for WorldsView {}

/// "Today, 14:30", "Yesterday, 09:12" or "2026-10-30".
pub fn when(time: SystemTime) -> String {
    let local = chrono::DateTime::<chrono::Local>::from(time);
    crate::i18n::format::day_and_time(local, chrono::Local::now().date_naive())
}

/// A plain file name for a world zip of `name`.
fn zip_name(name: &str) -> String {
    format!("{}.zip", worlds::folder_name(name))
}

impl file_dialog::ShowsError for WorldsView {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.set_message(message, true, cx);
    }
}

impl WorldsView {
    /// Adds the decoded world icons, for the performance monitor.
    pub fn tally_images(&self, tally: &mut crate::perf::ImageTally) {
        for image in self.icons.values() {
            tally.add(image);
        }
    }

    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let pack_search = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::SearchDataPacks)
                .with_automation_id("datapack-search")
        });
        let observe = vec![
            cx.observe(&instances, |this, _, cx| this.sync(cx)),
            cx.observe(&pack_search, |this, _, cx| this.pack_search_changed(cx)),
        ];
        let mut this = Self {
            instances,
            instance_id: None,
            section: Section::Worlds,
            worlds: Load::NotLoaded,
            icons: HashMap::new(),
            expanded: None,
            panel: WorldPanel::Backups,
            backups: HashMap::new(),
            datapacks: HashMap::new(),
            pack_icons: HashMap::new(),
            pack_search,
            pack_query: String::new(),
            pack_results: Load::NotLoaded,
            hit_icons: HashMap::new(),
            pack_search_task: None,
            datapacks_task: None,
            renaming: None,
            job: None,
            parked: Parked::default(),
            finished: HashMap::new(),
            message: None,
            screenshots: Load::NotLoaded,
            thumbs: HashMap::new(),
            viewing: None,
            large: None,
            confirm_delete_shot: None,
            was_active: false,
            load_task: None,
            shots_task: None,
            large_task: None,
            _prompt: None,
            _observe: observe,
        };
        this.sync(cx);
        this
    }

    /// Follows the selected instance, and reads the lists again when its game stops.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let (selected, active) = {
            let instances = self.instances.read(cx);
            let selected = instances.selected().map(|i| i.id.clone());
            let active = selected
                .as_deref()
                .is_some_and(|id| instances.is_active(id));
            (selected, active)
        };
        if selected != self.instance_id {
            let from = std::mem::replace(&mut self.instance_id, selected);
            // a job goes on, and is the tab's again when its instance is
            self.parked
                .switch(&mut self.job, from.as_deref(), self.instance_id.as_deref());
            self.worlds = Load::NotLoaded;
            self.screenshots = Load::NotLoaded;
            self.icons.clear();
            self.backups.clear();
            self.datapacks.clear();
            self.pack_icons.clear();
            // Another instance may run another Minecraft version.
            self.pack_results = Load::NotLoaded;
            self.pack_query.clear();
            self.thumbs.clear();
            self.expanded = None;
            self.renaming = None;
            self.message = self
                .instance_id
                .as_ref()
                .and_then(|id| self.finished.remove(id));
            self.viewing = None;
            self.large = None;
            self.confirm_delete_shot = None;
        } else if self.was_active && !active {
            self.reload(cx);
            self.reload_screenshots(cx);
        }
        self.was_active = active;
        cx.notify();
    }

    fn dirs(&self, cx: &gpui::App) -> Option<(String, InstanceDirs)> {
        let id = self.instance_id.clone()?;
        let dirs = self.instances.read(cx).content_dirs(&id);
        Some((id, dirs))
    }

    /// Whether changes are refused: the game is starting or running.
    pub fn locked(&self, cx: &gpui::App) -> bool {
        self.instance_id
            .as_deref()
            .is_some_and(|id| self.instances.read(cx).is_active(id))
    }

    fn can_change(&mut self, cx: &mut Context<Self>) -> bool {
        if self.locked(cx) {
            self.set_message(t(M::StopGameFirst), true, cx);
            return false;
        }
        if self.job.is_some() {
            self.set_message(t(M::WaitForTask), true, cx);
            return false;
        }
        true
    }

    /// Shows `message` in the section that is open.
    pub fn set_message(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        self.message = Some((message.into(), error, self.section));
        cx.notify();
    }

    pub fn select_section(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    /// Reads `saves/` and the backups again.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some((id, dirs)) = self.dirs(cx) else {
            return;
        };
        if !matches!(self.worlds, Load::Ready(_)) {
            self.worlds = Load::Loading;
        }
        let work = cx.background_spawn(async move {
            let list = worlds::list(&dirs);
            let icons: Vec<(String, image::RgbaImage)> = list
                .iter()
                .filter_map(|w| {
                    let icon = screenshots::thumbnail(w.icon.as_deref()?, 128).ok()?;
                    Some((w.folder.clone(), icon))
                })
                .collect();
            let backups: HashMap<String, Vec<Backup>> = list
                .iter()
                .map(|w| (w.folder.clone(), worlds::backups(&dirs, &w.folder)))
                .collect();
            (list, icons, backups)
        });
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let (list, icons, backups) = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.icons = icons
                    .into_iter()
                    .map(|(folder, icon)| (folder, crate::images::render_image(icon)))
                    .collect();
                this.backups = backups;
                this.worlds = Load::Ready(list);
                cx.notify();
            })
            .ok();
        }));
        if let (Some(folder), WorldPanel::DataPacks) = (self.expanded.clone(), self.panel) {
            self.load_datapacks(folder, cx);
        }
        cx.notify();
    }

    pub fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if matches!(self.worlds, Load::NotLoaded) {
            self.reload(cx);
        }
        if self.section == Section::Screenshots && matches!(self.screenshots, Load::NotLoaded) {
            self.reload_screenshots(cx);
        }
    }

    pub fn backups(&self, folder: &str) -> &[Backup] {
        self.backups.get(folder).map_or(&[], Vec::as_slice)
    }

    /// Runs `work` in the background as the tab's one job, with progress, and says how it
    /// went with `done`.
    fn run(
        &mut self,
        label: JobLabel,
        work: impl FnOnce(&InstanceDirs, &Progress, &CancelToken) -> Result<String, WorldError>
        + Send
        + 'static,
        cx: &mut Context<Self>,
    ) {
        self.run_async(
            label,
            move |dirs, progress, cancel| async move { work(&dirs, &progress, &cancel) },
            cx,
        );
    }

    /// Like [`Self::run`], for work that waits on the network.
    fn run_async<F>(
        &mut self,
        label: JobLabel,
        work: impl FnOnce(InstanceDirs, Arc<Progress>, CancelToken) -> F,
        cx: &mut Context<Self>,
    ) where
        F: std::future::Future<Output = Result<String, WorldError>> + Send + 'static,
    {
        if !self.can_change(cx) {
            return;
        }
        let Some((id, dirs)) = self.dirs(cx) else {
            return;
        };
        let busy = self.instances.read(cx).mark_files_busy(&id);
        let progress = Arc::new(Progress::default());
        let cancel = CancelToken::new();
        let task = {
            let work = cx.background_spawn(work(dirs, progress.clone(), cancel.clone()));
            cx.spawn(async move |this, cx| {
                let result = work.await;
                this.update(cx, |this, cx| {
                    let Some((_, shown)) =
                        this.parked
                            .finish(&mut this.job, this.instance_id.as_deref(), &id)
                    else {
                        return;
                    };
                    // The jobs are all about worlds, whichever section is open now.
                    let message = match result {
                        Ok(message) => (message.into(), false, Section::Worlds),
                        Err(WorldError::Cancelled) => {
                            (t(M::Cancelled).into(), false, Section::Worlds)
                        }
                        Err(err) => (err.to_string().into(), true, Section::Worlds),
                    };
                    if !shown {
                        this.finished.insert(id, message);
                        return;
                    }
                    this.message = Some(message);
                    cx.notify();
                    this.reload(cx);
                })
                .ok();
            })
        };
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        self.message = None;
        self.job = Some(Job {
            label,
            progress,
            cancel,
            _task: task,
            _ticker: ticker,
            _busy: busy,
        });
        cx.notify();
    }

    pub fn cancel_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.job {
            job.cancel.cancel();
        }
        cx.notify();
    }

    pub fn back_up(&mut self, folder: String, name: String, cx: &mut Context<Self>) {
        self.expanded = Some(folder.clone());
        self.run(
            JobLabel::BackingUp(name.clone()),
            move |dirs, progress, cancel| {
                let backup = worlds::backup(dirs, &folder, progress, cancel)?;
                Ok(tr!(
                    M::BackedUp,
                    name = name,
                    size = format::bytes(backup.size)
                ))
            },
            cx,
        );
    }

    pub fn restore_copy(&mut self, folder: String, backup: Backup, cx: &mut Context<Self>) {
        self.run(
            JobLabel::Restoring,
            move |dirs, progress, cancel| {
                let restored = worlds::restore_with(
                    dirs,
                    &folder,
                    &backup.path,
                    RestoreMode::Copy,
                    |_| Ok(()),
                    progress,
                    cancel,
                )?;
                Ok(tr!(M::RestoredAsCopy, folder = restored.folder))
            },
            cx,
        );
    }

    /// Replaces the world with a backup (after the window's confirmation).
    pub fn restore_replace(&mut self, folder: String, backup: Backup, cx: &mut Context<Self>) {
        self.run(
            JobLabel::Replacing,
            move |dirs, progress, cancel| {
                let restored = worlds::restore(
                    dirs,
                    &folder,
                    &backup.path,
                    RestoreMode::Replace,
                    progress,
                    cancel,
                )?;
                Ok(match restored.safety_backup {
                    Some(safety) => tr!(
                        M::RestoredWithSafety,
                        folder = restored.folder,
                        backup = safety.file_name,
                        trash = t(CommonMsg::TheTrash)
                    ),
                    None => tr!(M::Restored, folder = restored.folder),
                })
            },
            cx,
        );
    }

    /// Moves the world to the Trash (after the window's confirmation).
    pub fn delete_world(&mut self, folder: String, name: String, cx: &mut Context<Self>) {
        self.run(
            JobLabel::MovingToTrash(name.clone()),
            move |dirs, _, _| {
                worlds::delete(dirs, &folder)?;
                Ok(tr!(M::InTrash, name = name, trash = t(CommonMsg::TheTrash)))
            },
            cx,
        );
    }

    pub fn delete_backup(&mut self, backup: Backup, cx: &mut Context<Self>) {
        self.run(
            JobLabel::MovingBackupToTrash,
            move |dirs, _, _| {
                worlds::delete_backup(dirs, &backup.path)?;
                Ok(tr!(
                    M::InTrash,
                    name = backup.file_name,
                    trash = t(CommonMsg::TheTrash)
                ))
            },
            cx,
        );
    }

    pub fn start_rename(&mut self, folder: String, name: String, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_text(name)
                .with_label(CommonMsg::Rename)
                .with_automation_id("world-rename")
        });
        self.renaming = Some((folder, input));
        cx.notify();
    }

    pub fn finish_rename(&mut self, cx: &mut Context<Self>) {
        let Some((folder, input)) = self.renaming.take() else {
            return;
        };
        let name = input.read(cx).text().trim().to_string();
        self.run(
            JobLabel::Renaming,
            move |dirs, _, _| {
                let new_folder = worlds::rename(dirs, &folder, &name)?;
                Ok(if new_folder == folder {
                    tr!(M::RenamedTo, name = name)
                } else {
                    tr!(M::RenamedToFolder, name = name, folder = new_folder)
                })
            },
            cx,
        );
    }

    pub fn export_world(&mut self, folder: String, name: String, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let picked =
            cx.prompt_for_new_path(&super::settings::dirs_downloads(), Some(&zip_name(&name)));
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Some(path) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.run(
                    JobLabel::Exporting(name.clone()),
                    move |dirs, progress, cancel| {
                        let size = worlds::export(dirs, &folder, &path, progress, cancel)?;
                        Ok(tr!(
                            M::Exported,
                            name = name,
                            path = path.display(),
                            size = format::bytes(size)
                        ))
                    },
                    cx,
                )
            })
            .ok();
        }));
    }

    pub fn import_worlds(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
            })
            .collect();
        if paths.is_empty() {
            self.set_message(t(M::ZipsOnly), true, cx);
            return;
        }
        self.run(
            JobLabel::Importing,
            move |dirs, progress, cancel| {
                let mut imported = Vec::new();
                for path in &paths {
                    let folder = worlds::import_zip(dirs, path, progress, cancel).map_err(
                        |err| match err {
                            WorldError::Cancelled => WorldError::Cancelled,
                            other => WorldError::Invalid(tr!(
                                M::FileError,
                                name = path
                                    .file_name()
                                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                                error = other
                            )),
                        },
                    )?;
                    imported.push(folder);
                }
                Ok(tr!(M::Imported, worlds = format::list(&imported)))
            },
            cx,
        );
    }

    pub fn pick_import(&mut self, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(M::ImportWorldPrompt).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            this.update(cx, |this, cx| this.import_worlds(paths, cx))
                .ok();
        }));
    }

    fn open_saves(&self, cx: &mut Context<Self>) {
        if let Some((_, dirs)) = self.dirs(cx) {
            let dir = worlds::saves_dir(&dirs);
            let _ = std::fs::create_dir_all(&dir);
            cx.open_with_system(&dir);
        }
    }

    // ---- data packs ----

    /// Opens `panel` below the world `folder`, or closes it if it is open.
    pub fn toggle_panel(&mut self, folder: String, panel: WorldPanel, cx: &mut Context<Self>) {
        if self.expanded.as_deref() == Some(folder.as_str()) && self.panel == panel {
            self.expanded = None;
        } else {
            self.expanded = Some(folder.clone());
            self.panel = panel;
            if panel == WorldPanel::DataPacks {
                self.load_datapacks(folder, cx);
                if matches!(self.pack_results, Load::NotLoaded) {
                    self.run_pack_search(cx);
                }
            }
        }
        cx.notify();
    }

    /// The Minecraft version the instance runs.
    fn game_version(&self, cx: &gpui::App) -> Option<String> {
        let id = self.instance_id.as_deref()?;
        Some(self.instances.read(cx).content_target(id)?.game_version)
    }

    /// Reads the world's data packs and their icons.
    pub fn load_datapacks(&mut self, folder: String, cx: &mut Context<Self>) {
        let Some((id, dirs)) = self.dirs(cx) else {
            return;
        };
        if !matches!(self.datapacks.get(&folder), Some(Load::Ready(_))) {
            self.datapacks.insert(folder.clone(), Load::Loading);
        }
        let world = folder.clone();
        let work = cx.background_spawn(async move { datapacks::list(&dirs, &world) });
        self.datapacks_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.pack_icons.retain(|(world, _), _| world != &folder);
                let loaded = match result {
                    Ok(packs) => {
                        for pack in &packs.packs {
                            if let Some(icon) = &pack.meta.icon {
                                this.pack_icons.insert(
                                    (folder.clone(), pack.name.clone()),
                                    crate::images::render_image((**icon).clone()),
                                );
                            }
                        }
                        Load::Ready(packs)
                    }
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                this.datapacks.insert(folder, loaded);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// "Add data pack…": zips and folders.
    pub fn pick_datapacks(&mut self, folder: String, name: String, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some(t(M::AddDataPackPrompt).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            this.update(cx, |this, cx| this.add_datapacks(folder, name, paths, cx))
                .ok();
        }));
    }

    /// Copies `paths` into the world's data packs. Each that can't be added says why; the
    /// others are added anyway.
    pub fn add_datapacks(
        &mut self,
        folder: String,
        name: String,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        self.run(
            JobLabel::AddingDataPacks,
            move |dirs, _, _| {
                let mut added = Vec::new();
                let mut failed = Vec::new();
                for path in &paths {
                    match datapacks::add(dirs, &folder, path) {
                        Ok(pack) => added.push(pack),
                        Err(err) => failed.push(tr!(
                            M::FileError,
                            name = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                            error = err
                        )),
                    }
                }
                match (added.is_empty(), failed.is_empty()) {
                    (_, true) => Ok(tr!(
                        M::AddedDataPacks,
                        packs = format::list(&added),
                        name = name
                    )),
                    (true, false) => Err(WorldError::Invalid(failed.join(" "))),
                    (false, false) => Ok(format!(
                        "{} {}",
                        tr!(M::AddedDataPacks, packs = format::list(&added), name = name),
                        failed.join(" ")
                    )),
                }
            },
            cx,
        );
    }

    /// Turns the pack on or off in the world's list.
    pub fn set_pack_enabled(
        &mut self,
        folder: String,
        pack: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.run(
            JobLabel::SavingPackList,
            move |dirs, _, _| {
                datapacks::set_enabled(dirs, &folder, &pack, enabled)?;
                Ok(if enabled {
                    tr!(M::PackTurnedOn, pack = pack)
                } else {
                    tr!(M::PackTurnedOff, pack = pack)
                })
            },
            cx,
        );
    }

    /// Moves the pack to the Trash.
    pub fn delete_pack(&mut self, folder: String, pack: String, cx: &mut Context<Self>) {
        self.run(
            JobLabel::MovingToTrash(pack.clone()),
            move |dirs, _, _| {
                datapacks::delete(dirs, &folder, &pack)?;
                Ok(tr!(M::InTrash, name = pack, trash = t(CommonMsg::TheTrash)))
            },
            cx,
        );
    }

    /// Installs the newest data pack version of the Modrinth project that fits the
    /// instance's Minecraft version (a release when there is one) into the world.
    pub fn install_pack(
        &mut self,
        folder: String,
        name: String,
        hit: PackHit,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.instances.read(cx).modrinth() else {
            self.set_message(t(M::ModrinthUnavailable), true, cx);
            return;
        };
        let Some(game_version) = self.game_version(cx) else {
            return;
        };
        let downloader = self.instances.read(cx).content_downloader();
        self.run_async(
            JobLabel::InstallingDataPack(hit.title.clone()),
            move |dirs, progress, cancel| async move {
                let versions = api
                    .versions(
                        &hit.project_id,
                        &[datapacks::MODRINTH_LOADER.to_string()],
                        std::slice::from_ref(&game_version),
                    )
                    .await
                    .map_err(|err| WorldError::Invalid(err.to_string()))?;
                let version =
                    datapacks::pick_version(&game_version, versions).ok_or_else(|| {
                        WorldError::Invalid(tr!(
                            M::NoDataPackVersion,
                            name = hit.title,
                            version = game_version
                        ))
                    })?;
                let installed = datapacks::install(
                    &api,
                    &downloader,
                    &dirs,
                    &folder,
                    &version,
                    &progress,
                    &cancel,
                )
                .await?;
                Ok(match installed.replaced {
                    Some(old) => tr!(
                        M::UpdatedDataPack,
                        pack = installed.name,
                        name = name,
                        old = old
                    ),
                    None => tr!(M::InstalledDataPack, pack = installed.name, name = name),
                })
            },
            cx,
        );
    }

    fn pack_search_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.pack_search.read(cx).text().trim().to_string();
        if text == self.pack_query {
            return;
        }
        self.pack_search_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            this.update(cx, |this, cx| this.run_pack_search(cx)).ok();
        }));
    }

    /// Searches Modrinth for data packs for the instance's Minecraft version, or opens
    /// a pasted modrinth.com link.
    pub fn run_pack_search(&mut self, cx: &mut Context<Self>) {
        let text = self.pack_search.read(cx).text().trim().to_string();
        self.pack_query = text.clone();
        let Some(api) = self.instances.read(cx).modrinth() else {
            self.pack_results = Load::Failed(t(M::ModrinthUnavailable).into());
            cx.notify();
            return;
        };
        let Some(game_version) = self.game_version(cx) else {
            return;
        };
        self.pack_results = Load::Loading;
        let work = if text.contains("modrinth.com/") {
            cx.background_spawn(async move {
                let target = api.resolve_link(&text).await.map_err(|e| e.to_string())?;
                if target
                    .project
                    .loaders
                    .iter()
                    .any(|l| l == datapacks::MODRINTH_LOADER)
                {
                    Ok(vec![PackHit::from(target.project)])
                } else {
                    Err(t(M::LinkNotDataPack).to_string())
                }
            })
        } else {
            let params = ClientSearchParams::new(
                (!text.is_empty()).then_some(text),
                ClientProjectType::DataPack,
                Some(game_version),
                Vec::new(),
                None,
                Some(0),
                Some(PACK_RESULTS),
            );
            cx.background_spawn(async move {
                let params = params.map_err(|e| e.to_string())?;
                let results = api.search(&params).await.map_err(|e| e.to_string())?;
                Ok(results.hits.into_iter().map(PackHit::from).collect())
            })
        };
        self.pack_search_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<Vec<PackHit>, String> = work.await;
            this.update(cx, |this, cx| {
                this.pack_results = match result {
                    Ok(hits) => Load::Ready(hits),
                    Err(err) => Load::Failed(err.into()),
                };
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn hit_icon(&mut self, url: &str, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        let api = self.instances.read(cx).modrinth();
        super::modpack_browser::fetch_image(&mut self.hit_icons, url, false, api, cx, |this| {
            &mut this.hit_icons
        })
    }

    // ---- screenshots ----

    pub fn reload_screenshots(&mut self, cx: &mut Context<Self>) {
        let Some((id, dirs)) = self.dirs(cx) else {
            return;
        };
        if !matches!(self.screenshots, Load::Ready(_)) {
            self.screenshots = Load::Loading;
        }
        let work = cx.background_spawn(async move { screenshots::list(&dirs) });
        self.shots_task = Some(cx.spawn(async move |this, cx| {
            let list = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                // Thumbnails of files that are gone, or changed, are dropped.
                let current: HashMap<&Path, Option<SystemTime>> = list
                    .iter()
                    .map(|s| (s.path.as_path(), s.modified))
                    .collect();
                this.thumbs
                    .retain(|path, (time, _)| current.get(path.as_path()) == Some(time));
                if let Some(index) = this.viewing
                    && index >= list.len()
                {
                    this.viewing = list.len().checked_sub(1);
                }
                this.screenshots = Load::Ready(list);
                // After a delete, the next screenshot takes its place.
                if let Some(index) = this.viewing
                    && this.large.is_none()
                {
                    this.view(index, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// The thumbnail of a screenshot, decoded in the background the first time.
    fn thumbnail(&mut self, shot: &Screenshot, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        match self.thumbs.get(&shot.path) {
            Some((time, Thumb::Ready(image))) if *time == shot.modified => {
                return Some(image.clone());
            }
            Some((time, _)) if *time == shot.modified => return None,
            _ => {}
        }
        self.thumbs
            .insert(shot.path.clone(), (shot.modified, Thumb::Loading));
        let path = shot.path.clone();
        let modified = shot.modified;
        let work = cx.background_spawn({
            let path = path.clone();
            async move { screenshots::thumbnail(&path, THUMB_PIXELS) }
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                let thumb = match result {
                    Ok(image) => Thumb::Ready(crate::images::render_image(image)),
                    Err(_) => Thumb::Failed,
                };
                this.thumbs.insert(path, (modified, thumb));
                cx.notify();
            })
            .ok();
        })
        .detach();
        None
    }

    fn shot(&self, index: usize) -> Option<&Screenshot> {
        self.screenshots.ready()?.get(index)
    }

    pub fn view(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(shot) = self.shot(index).cloned() else {
            return;
        };
        self.viewing = Some(index);
        self.confirm_delete_shot = None;
        if self.large.as_ref().is_some_and(|(p, _)| p == &shot.path) {
            cx.notify();
            return;
        }
        self.large = Some((shot.path.clone(), Load::Loading));
        let work = cx.background_spawn({
            let path = shot.path.clone();
            async move { screenshots::thumbnail(&path, LARGE_PIXELS) }
        });
        self.large_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.large.as_ref().is_some_and(|(p, _)| p == &shot.path) {
                    this.large = Some((
                        shot.path.clone(),
                        match result {
                            Ok(image) => Load::Ready(crate::images::render_image(image)),
                            Err(err) => Load::Failed(err.into()),
                        },
                    ));
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn close_view(&mut self, cx: &mut Context<Self>) {
        self.viewing = None;
        self.large = None;
        self.confirm_delete_shot = None;
        cx.notify();
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (Some(index), Some(list)) = (self.viewing, self.screenshots.ready()) else {
            return;
        };
        let next = index as isize + delta;
        if next >= 0 && (next as usize) < list.len() {
            self.view(next as usize, cx);
        }
    }

    fn copy_shot(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let work = cx.background_spawn(async move { screenshots::png_bytes(&path) });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| match result {
                Ok(bytes) => {
                    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
                        ImageFormat::Png,
                        bytes,
                    )));
                    this.set_message(t(M::CopiedScreenshot), false, cx);
                }
                Err(err) => this.set_message(tr!(M::CouldntCopy, error = err), true, cx),
            })
            .ok();
        }));
    }

    fn delete_shot(&mut self, file_name: String, cx: &mut Context<Self>) {
        if self.confirm_delete_shot.as_deref() != Some(file_name.as_str()) {
            self.confirm_delete_shot = Some(file_name);
            cx.notify();
            return;
        }
        self.confirm_delete_shot = None;
        let Some((_, dirs)) = self.dirs(cx) else {
            return;
        };
        match screenshots::delete(&dirs, &file_name) {
            Ok(()) => self.set_message(
                tr!(M::InTrash, name = file_name, trash = t(CommonMsg::TheTrash)),
                false,
                cx,
            ),
            Err(err) => self.set_message(err, true, cx),
        }
        self.large = None;
        self.reload_screenshots(cx);
    }

    // ---- previews ----

    /// Shows prepared state, for the offscreen screenshots.
    pub fn preview(
        &mut self,
        section: Section,
        expanded: Option<&str>,
        view: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.section = section;
        self.expanded = expanded.map(str::to_string);
        if let Some(index) = view {
            self.view(index, cx);
        }
        cx.notify();
    }

    /// Loads the lists right away, for the offscreen screenshots.
    pub fn preview_load(&mut self, cx: &mut Context<Self>) {
        let Some((_, dirs)) = self.dirs(cx) else {
            return;
        };
        let list = worlds::list(&dirs);
        self.icons = list
            .iter()
            .filter_map(|w| {
                let icon = screenshots::thumbnail(w.icon.as_deref()?, 128).ok()?;
                Some((w.folder.clone(), crate::images::render_image(icon)))
            })
            .collect();
        self.backups = list
            .iter()
            .map(|w| (w.folder.clone(), worlds::backups(&dirs, &w.folder)))
            .collect();
        self.worlds = Load::Ready(list);
        let shots = screenshots::list(&dirs);
        for shot in &shots {
            if let Ok(image) = screenshots::thumbnail(&shot.path, THUMB_PIXELS) {
                self.thumbs.insert(
                    shot.path.clone(),
                    (
                        shot.modified,
                        Thumb::Ready(crate::images::render_image(image)),
                    ),
                );
            }
        }
        self.screenshots = Load::Ready(shots);
    }

    /// Opens the world's data packs with `hits` as the Modrinth results, and their icons
    /// by URL, read right away for the offscreen screenshots.
    pub fn preview_datapacks(
        &mut self,
        folder: &str,
        query: &str,
        hits: Vec<PackHit>,
        icons: Vec<(String, image::RgbaImage)>,
        cx: &mut Context<Self>,
    ) {
        let Some((_, dirs)) = self.dirs(cx) else {
            return;
        };
        self.section = Section::Worlds;
        self.expanded = Some(folder.to_string());
        self.panel = WorldPanel::DataPacks;
        if let Ok(packs) = datapacks::list(&dirs, folder) {
            for pack in &packs.packs {
                if let Some(icon) = &pack.meta.icon {
                    self.pack_icons.insert(
                        (folder.to_string(), pack.name.clone()),
                        crate::images::render_image((**icon).clone()),
                    );
                }
            }
            self.datapacks
                .insert(folder.to_string(), Load::Ready(packs));
        }
        self.pack_query = query.to_string();
        self.pack_search
            .update(cx, |input, cx| input.set_text(query.to_string(), cx));
        for (url, icon) in icons {
            self.hit_icons
                .insert(url, RemoteIcon::Ready(crate::images::render_image(icon)));
        }
        self.pack_results = Load::Ready(hits);
        // Typing the query above must not start a real search.
        self.pack_search_task = None;
        cx.notify();
    }

    /// Decodes the large view right away, for the offscreen screenshots.
    pub fn preview_view(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(shot) = self.shot(index).cloned() else {
            return;
        };
        self.viewing = Some(index);
        if let Ok(image) = screenshots::thumbnail(&shot.path, LARGE_PIXELS) {
            self.large = Some((shot.path, Load::Ready(crate::images::render_image(image))));
        }
        cx.notify();
    }
}

// ---- rendering ----

fn world_details(world: &World) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(summary) = &world.summary {
        if summary.hardcore {
            parts.push(t(M::Hardcore).to_string());
        } else if let Some(mode) = summary.game_mode {
            parts.push(
                t(match mode {
                    GameMode::Survival => M::Survival,
                    GameMode::Creative => M::Creative,
                    GameMode::Adventure => M::Adventure,
                    GameMode::Spectator => M::Spectator,
                })
                .to_string(),
            );
        }
        if summary.allow_commands == Some(true) {
            parts.push(t(M::Cheats).to_string());
        }
        match (&summary.version_name, summary.snapshot) {
            (Some(version), true) => parts.push(tr!(M::Snapshot, version = version)),
            (Some(version), false) => parts.push(version.clone()),
            (None, _) => {}
        }
    }
    parts
}

impl WorldsView {
    fn render_job(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let job = self.job.as_ref()?;
        let snapshot = job.progress.snapshot();
        let fraction = (snapshot.files_total > 0)
            .then(|| snapshot.files_done as f32 / snapshot.files_total as f32);
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .px_4()
                .py_2()
                .rounded(radius::LG)
                .border_1()
                .border_color(crate::theme::accent::light().opacity(40))
                .bg(crate::theme::accent::base().opacity(10))
                .child(LoadingDots)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            div()
                                .truncate()
                                .font_weight(weights::BOLD.into())
                                .child(job.label.text()),
                        )
                        .when_some(fraction, |this, fraction| {
                            this.child(
                                div()
                                    .h(px(4.))
                                    .w_full()
                                    .rounded_full()
                                    .bg(colors::GRAY_700)
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded_full()
                                            .bg(crate::theme::accent::light())
                                            .surface(launcher_core::ui_skin::SurfaceId::Progress)
                                            .w(gpui::relative(fraction.clamp(0.0, 1.0))),
                                    ),
                            )
                        }),
                )
                .child(
                    Button::new("cancel-world-job", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_job(cx))),
                ),
        )
    }

    fn render_message(&self) -> Option<gpui::Div> {
        let (message, error, section) = self.message.clone()?;
        if section != self.section {
            return None;
        }
        Some(
            small(message)
                .px_1()
                .whitespace_normal()
                .font_weight(weights::BOLD.into())
                .text_color(if error {
                    colors::RED_200
                } else {
                    colors::GREEN_300
                }),
        )
    }

    fn render_world(
        &mut self,
        index: usize,
        world: &World,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let folder = world.folder.clone();
        let name = world.name();
        let open = self.expanded.as_deref() == Some(folder.as_str());
        let expanded = open && self.panel == WorldPanel::Backups;
        let packs_open = open && self.panel == WorldPanel::DataPacks;
        let pack_count = self
            .datapacks
            .get(&folder)
            .and_then(Load::ready)
            .map(|p| p.packs.len());
        let backups = self.backups(&folder).to_vec();
        let mut details = world_details(world);
        if let Some(time) = world.last_played() {
            details.insert(0, tr!(M::PlayedAt, time = when(time)));
        }
        details.push(format::bytes(world.size));
        let icon = match self.icons.get(&folder) {
            Some(image) => img(image.clone())
                .flex_none()
                .size(px(48.))
                .rounded(radius::MD)
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => img("images/instance-icons/grass.png")
                .flex_none()
                .size(px(48.))
                .rounded(radius::MD)
                .into_any_element(),
        };
        let renaming = self
            .renaming
            .as_ref()
            .filter(|(f, _)| f == &folder)
            .map(|(_, input)| input.clone());
        let hardcore = world.summary.as_ref().is_some_and(|s| s.hardcore);
        let title: AnyElement = match renaming {
            Some(input) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(div().w(px(260.)).child(input))
                .child(
                    Button::new(("rename-save", index), t(CommonMsg::Save))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.finish_rename(cx))),
                )
                .child(
                    Button::new(("rename-cancel", index), t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.renaming = None;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            None => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::text())
                        .child(RichText::minecraft(name.clone())),
                )
                .when(hardcore, |this| {
                    this.child(badge(t(M::Hardcore), colors::RED_200))
                })
                .when(name != folder, |this| {
                    this.child(
                        small_rich(RichText::minecraft(folder.clone()))
                            .truncate()
                            .text_color(crate::theme::ink::muted()),
                    )
                })
                .into_any_element(),
        };
        let description = details.join(" · ");
        let mut row = div().flex().flex_col().child(
            row_base(("world", index), index == 0)
                .accessible(Role::Group, format!("world-{folder}"), name.clone())
                .aria_description(description)
                .child(icon)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(title)
                        .child(
                            small_rich(RichText::new(details.join(" · ")))
                                .truncate()
                                .text_color(crate::theme::ink::muted()),
                        )
                        .when_some(world.problem.clone(), |this, problem| {
                            this.child(
                                small_rich(RichText::new(problem)).text_color(colors::YELLOW_300),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_none()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new(("backup", index), t(M::BackUp))
                                .variant(ButtonVariant::Secondary)
                                .icon(Icon::Download)
                                .size(ButtonSize::Slim)
                                .disabled(busy)
                                .on_click({
                                    let (folder, name) = (folder.clone(), name.clone());
                                    cx.listener(move |this, _, _, cx| {
                                        this.back_up(folder.clone(), name.clone(), cx)
                                    })
                                }),
                        )
                        .child(
                            Button::new(
                                ("backups", index),
                                if backups.is_empty() {
                                    t(M::Backups).to_string()
                                } else {
                                    format!(
                                        "{} {}",
                                        t(M::Backups),
                                        format::number(backups.len() as u64)
                                    )
                                },
                            )
                            .variant(if expanded {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Ghost
                            })
                            .size(ButtonSize::Slim)
                            .on_click({
                                let folder = folder.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_panel(folder.clone(), WorldPanel::Backups, cx)
                                })
                            }),
                        )
                        .child(
                            Button::new(
                                ("datapacks", index),
                                match pack_count {
                                    Some(count) if count > 0 => format!(
                                        "{} {}",
                                        t(M::DataPacks),
                                        format::number(count as u64)
                                    ),
                                    _ => t(M::DataPacks).to_string(),
                                },
                            )
                            .variant(if packs_open {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Ghost
                            })
                            .size(ButtonSize::Slim)
                            .on_click({
                                let folder = folder.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_panel(folder.clone(), WorldPanel::DataPacks, cx)
                                })
                            }),
                        )
                        .child(
                            Button::new(("rename", index), t(CommonMsg::Rename))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .disabled(busy)
                                .on_click({
                                    let (folder, name) = (folder.clone(), name.clone());
                                    cx.listener(move |this, _, _, cx| {
                                        this.start_rename(folder.clone(), name.clone(), cx)
                                    })
                                }),
                        )
                        .child(
                            Button::new(("export", index), t(M::Export))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .disabled(busy)
                                .on_click({
                                    let (folder, name) = (folder.clone(), name.clone());
                                    cx.listener(move |this, _, _, cx| {
                                        this.export_world(folder.clone(), name.clone(), cx)
                                    })
                                }),
                        )
                        .child(
                            Button::new(("open-world", index), t(CommonMsg::OpenFolder))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click({
                                    let folder = folder.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        if let Some((_, dirs)) = this.dirs(cx) {
                                            cx.open_with_system(
                                                &worlds::saves_dir(&dirs).join(&folder),
                                            );
                                        }
                                    })
                                }),
                        )
                        .child(
                            Button::new(("delete-world", index), t(M::DeleteWorld))
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Slim)
                                .disabled(busy)
                                .on_click({
                                    let (folder, name) = (folder.clone(), name.clone());
                                    cx.listener(move |this, _, _, cx| {
                                        if this.can_change(cx) {
                                            cx.emit(WorldsEvent::Delete {
                                                folder: folder.clone(),
                                                name: name.clone(),
                                            });
                                        }
                                    })
                                }),
                        ),
                ),
        );
        if expanded {
            row = row.child(self.render_backups(index, &folder, &name, &backups, busy, cx));
        }
        if packs_open {
            row = row.child(self.render_datapacks(&folder, &name, busy, cx));
        }
        row.into_any_element()
    }

    /// The panel below a world: its data packs, and a Modrinth search.
    fn render_datapacks(
        &mut self,
        folder: &str,
        name: &str,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let game_version = self.game_version(cx).unwrap_or_default();
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                Button::new("add-datapack", t(M::AddDataPack))
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::Upload)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click({
                        let (folder, name) = (folder.to_string(), name.to_string());
                        cx.listener(move |this, _, _, cx| {
                            this.pick_datapacks(folder.clone(), name.clone(), cx)
                        })
                    }),
            )
            .child(
                Button::new("open-datapacks", t(CommonMsg::OpenFolder))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .on_click({
                        let folder = folder.to_string();
                        cx.listener(move |this, _, _, cx| {
                            if let Some((_, dirs)) = this.dirs(cx)
                                && let Ok(dir) = datapacks::dir(&dirs, &folder)
                            {
                                let _ = std::fs::create_dir_all(&dir);
                                cx.open_with_system(&dir);
                            }
                        })
                    }),
            );

        let installed_projects: Vec<String>;
        let list: AnyElement = match self.datapacks.get(folder) {
            None | Some(Load::NotLoaded | Load::Loading) => {
                installed_projects = Vec::new();
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .text_color(crate::theme::ink::muted())
                    .child(LoadingDots)
                    .child(t(M::ReadingDataPacks))
                    .into_any_element()
            }
            Some(Load::Failed(err)) => {
                installed_projects = Vec::new();
                small(err.clone())
                    .whitespace_normal()
                    .text_color(colors::RED_200)
                    .into_any_element()
            }
            Some(Load::Ready(packs)) if packs.packs.is_empty() => {
                installed_projects = Vec::new();
                small(t(M::NoDataPacksYet))
                    .whitespace_normal()
                    .text_color(crate::theme::ink::muted())
                    .into_any_element()
            }
            Some(Load::Ready(packs)) => {
                let packs = packs.clone();
                installed_projects = packs
                    .packs
                    .iter()
                    .filter_map(|p| p.modrinth_project.clone())
                    .collect();
                let mut rows = div().flex().flex_col().gap_1();
                if !packs.switchable {
                    rows = rows.child(
                        small(t(M::PacksNotSwitchable))
                            .whitespace_normal()
                            .text_color(colors::YELLOW_300),
                    );
                }
                for (index, pack) in packs.packs.iter().enumerate() {
                    rows = rows.child(self.render_pack(
                        index,
                        folder,
                        pack,
                        packs.switchable,
                        &game_version,
                        busy,
                        cx,
                    ));
                }
                rows.into_any_element()
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .pl(px(76.))
            .pr_4()
            .py_2()
            .bg(colors::GRAY_850)
            .border_t_1()
            .border_color(colors::GRAY_FADED.opacity(15))
            .child(toolbar)
            .child(list)
            .child(self.render_pack_search(folder, name, &installed_projects, busy, cx))
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_pack(
        &self,
        index: usize,
        folder: &str,
        pack: &datapacks::DataPack,
        switchable: bool,
        game_version: &str,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = match self
            .pack_icons
            .get(&(folder.to_string(), pack.name.clone()))
        {
            Some(image) => img(image.clone())
                .flex_none()
                .size(px(32.))
                .rounded(radius::MD)
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div()
                .flex_none()
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(radius::MD)
                .bg(colors::GRAY_800)
                .child(Icon::Folder.render(px(14.), colors::GRAY_500))
                .into_any_element(),
        };
        let compatibility = datapacks::compatibility(pack.meta.formats.as_ref(), game_version);
        let mut details = Vec::new();
        if let Some(description) = &pack.meta.description {
            details.push(description.clone());
        }
        details.push(format::bytes(pack.size));
        let on = pack.state.is_on();
        div()
            .id(("datapack-row", index))
            .accessible(
                Role::Group,
                format!("datapack-{}", pack.name),
                pack.name.clone(),
            )
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .py_1()
            .child(icon)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .when(!on, |this| this.text_color(crate::theme::ink::muted()))
                                    .child(pack.name.clone()),
                            )
                            .when(pack.state == PackState::New, |this| {
                                this.child(badge(t(M::NewPack), colors::GREEN_300))
                            })
                            .when(compatibility.warning().is_some(), |this| {
                                this.child(badge(
                                    if compatibility == Compatibility::TooOld {
                                        t(ContentMsg::OlderVersion)
                                    } else {
                                        t(ContentMsg::NewerVersion)
                                    },
                                    colors::YELLOW_300,
                                ))
                            }),
                    )
                    .child(
                        small_rich(RichText::minecraft(details.join(" · ")))
                            .truncate()
                            .text_color(crate::theme::ink::muted()),
                    )
                    .when(pack.state == PackState::New, |this| {
                        this.child(small(t(M::NewPackHint)).text_color(crate::theme::ink::muted()))
                    })
                    .when_some(pack.meta.problem.clone(), |this, problem| {
                        this.child(
                            small(problem)
                                .whitespace_normal()
                                .text_color(colors::YELLOW_300),
                        )
                    }),
            )
            .child(
                Toggle::new(("datapack-on", index), on)
                    .label(if on {
                        t(CommonMsg::On)
                    } else {
                        t(CommonMsg::Off)
                    })
                    .disabled(busy || !switchable)
                    .on_change({
                        let (folder, pack) = (folder.to_string(), pack.name.clone());
                        cx.listener(move |this, value: &bool, _, cx| {
                            this.set_pack_enabled(folder.clone(), pack.clone(), *value, cx)
                        })
                    }),
            )
            .child(
                Button::new(("delete-datapack", index), t(CommonMsg::Delete))
                    .variant(ButtonVariant::Danger)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click({
                        let (folder, pack) = (folder.to_string(), pack.name.clone());
                        cx.listener(move |this, _, _, cx| {
                            this.delete_pack(folder.clone(), pack.clone(), cx)
                        })
                    }),
            )
            .into_any_element()
    }

    fn render_pack_search(
        &mut self,
        folder: &str,
        name: &str,
        installed: &[String],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_1()
            .pt_2()
            .border_t_1()
            .border_color(colors::GRAY_FADED.opacity(15))
            .child(self.pack_search.clone());
        let hits = match &self.pack_results {
            Load::NotLoaded => return panel.into_any_element(),
            Load::Loading => {
                return panel
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .py_1()
                            .text_color(crate::theme::ink::muted())
                            .child(LoadingDots)
                            .child(t(M::SearchingModrinth)),
                    )
                    .into_any_element();
            }
            Load::Failed(err) => {
                return panel
                    .child(
                        small(err.clone())
                            .whitespace_normal()
                            .text_color(colors::RED_200),
                    )
                    .into_any_element();
            }
            Load::Ready(hits) => hits.clone(),
        };
        if hits.is_empty() {
            let version = self.game_version(cx).unwrap_or_default();
            return panel
                .child(
                    small(tr!(M::NoDataPacksFound, version = version))
                        .text_color(crate::theme::ink::muted()),
                )
                .into_any_element();
        }
        for (index, hit) in hits.into_iter().enumerate() {
            let icon = hit
                .icon_url
                .as_deref()
                .and_then(|url| self.hit_icon(url, cx));
            let is_installed = installed.contains(&hit.project_id);
            let mut details = Vec::new();
            if !hit.author.is_empty() {
                details.push(hit.author.clone());
            }
            details.push(tr!(
                ContentMsg::DownloadsCompact,
                count = format::compact(hit.downloads)
            ));
            panel = panel.child(
                div()
                    .id(("datapack-hit", index))
                    .accessible(
                        Role::Group,
                        format!("datapack-hit-{}", hit.project_id),
                        hit.title.clone(),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .py_1()
                    .child(match icon {
                        Some(image) => img(image)
                            .flex_none()
                            .size(px(32.))
                            .rounded(radius::MD)
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(radius::MD)
                            .bg(colors::GRAY_800)
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(weights::BOLD.into())
                                            .child(hit.title.clone()),
                                    )
                                    .child(
                                        small(details.join(" · "))
                                            .flex_none()
                                            .text_color(crate::theme::ink::muted()),
                                    )
                                    .when(is_installed, |this| {
                                        this.child(badge(
                                            t(ContentMsg::InstalledBadge),
                                            colors::GREEN_300,
                                        ))
                                    }),
                            )
                            .child(
                                small(hit.description.clone())
                                    .truncate()
                                    .text_color(crate::theme::ink::muted()),
                            ),
                    )
                    .child(
                        Button::new(
                            ("install-datapack", index),
                            if is_installed {
                                t(CommonMsg::Update)
                            } else {
                                t(CommonMsg::Install)
                            },
                        )
                        .variant(if is_installed {
                            ButtonVariant::Ghost
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Slim)
                        .disabled(busy)
                        .on_click({
                            let (folder, name) = (folder.to_string(), name.to_string());
                            cx.listener(move |this, _, _, cx| {
                                this.install_pack(folder.clone(), name.clone(), hit.clone(), cx)
                            })
                        }),
                    ),
            );
        }
        panel.into_any_element()
    }

    fn render_backups(
        &self,
        world_index: usize,
        folder: &str,
        name: &str,
        backups: &[Backup],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut list = div()
            .flex()
            .flex_col()
            .gap_1()
            .pl(px(76.))
            .pr_4()
            .py_2()
            .bg(colors::GRAY_850)
            .border_t_1()
            .border_color(colors::GRAY_FADED.opacity(15));
        if backups.is_empty() {
            return list
                .child(
                    small(t(M::NoBackupsYet))
                        .whitespace_normal()
                        .text_color(crate::theme::ink::muted()),
                )
                .into_any_element();
        }
        for (index, backup) in backups.iter().enumerate() {
            let id = world_index * 1000 + index;
            let date = backup
                .created
                .map(when)
                .unwrap_or_else(|| backup.file_name.clone());
            list = list.child(
                div()
                    .id(("backup-row", id))
                    .accessible(
                        Role::Group,
                        format!("backup-{}", backup.file_name),
                        date.clone(),
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .py_1()
                    .child(Icon::Download.render(px(12.), colors::GRAY_500))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .child(date),
                            )
                            .child(
                                small(format!(
                                    "{} · {}",
                                    backup.file_name,
                                    format::bytes(backup.size)
                                ))
                                .truncate()
                                .text_color(crate::theme::ink::muted()),
                            ),
                    )
                    .child(
                        Button::new(("restore-copy", id), t(M::RestoreAsCopy))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .disabled(busy)
                            .on_click({
                                let (folder, backup) = (folder.to_string(), backup.clone());
                                cx.listener(move |this, _, _, cx| {
                                    this.restore_copy(folder.clone(), backup.clone(), cx)
                                })
                            }),
                    )
                    .child(
                        Button::new(("restore-replace", id), t(M::ReplaceWorld))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .disabled(busy)
                            .on_click({
                                let (folder, name, backup) =
                                    (folder.to_string(), name.to_string(), backup.clone());
                                cx.listener(move |this, _, _, cx| {
                                    if this.can_change(cx) {
                                        cx.emit(WorldsEvent::Replace {
                                            folder: folder.clone(),
                                            name: name.clone(),
                                            backup: backup.clone(),
                                        });
                                    }
                                })
                            }),
                    )
                    .child(
                        Button::new(("reveal-backup", id), t(CommonMsg::ShowInFileManager))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .on_click({
                                let path = backup.path.clone();
                                move |_, _, cx| cx.reveal_path(&path)
                            }),
                    )
                    .child(
                        Button::new(("delete-backup", id), t(CommonMsg::Delete))
                            .variant(ButtonVariant::Danger)
                            .size(ButtonSize::Slim)
                            .disabled(busy)
                            .on_click({
                                let backup = backup.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.delete_backup(backup.clone(), cx)
                                })
                            }),
                    ),
            );
        }
        list.into_any_element()
    }

    fn render_worlds(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let locked = self.locked(cx);
        let busy = locked || self.job.is_some();
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                Button::new("import-world", t(M::ImportWorld))
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::Upload)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.pick_import(cx))),
            )
            .child(
                Button::new("open-saves", t(M::OpenSavesFolder))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| this.open_saves(cx))),
            )
            .child(div().flex_1())
            .child(
                Button::new("reload-worlds", t(CommonMsg::Refresh))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
            );
        let body: AnyElement = match &self.worlds {
            Load::NotLoaded | Load::Loading => div()
                .flex()
                .flex_row()
                .gap_2()
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::ReadingWorlds))
                .into_any_element(),
            Load::Failed(err) => small(err.clone())
                .text_color(colors::RED_200)
                .into_any_element(),
            Load::Ready(list) if list.is_empty() => card()
                .child(
                    div()
                        .font_weight(weights::BOLD.into())
                        .child(t(M::NoWorldsYet)),
                )
                .child(muted(t(M::NoWorldsHint)).whitespace_normal())
                .into_any_element(),
            Load::Ready(list) => {
                let list = list.clone();
                let mut rows = list_box();
                for (index, world) in list.iter().enumerate() {
                    rows = rows.child(self.render_world(index, world, busy, cx));
                }
                rows.into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(toolbar)
            .when(locked, |this| {
                this.child(
                    small(t(M::GameRunning))
                        .whitespace_normal()
                        .text_color(colors::YELLOW_300),
                )
            })
            .child(body)
            .into_any_element()
    }

    fn render_screenshots(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let list = match &self.screenshots {
            Load::NotLoaded | Load::Loading => {
                return div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .text_color(crate::theme::ink::muted())
                    .child(LoadingDots)
                    .child(t(M::ReadingScreenshots))
                    .into_any_element();
            }
            Load::Failed(err) => {
                return small(err.clone())
                    .text_color(colors::RED_200)
                    .into_any_element();
            }
            Load::Ready(list) => list.clone(),
        };
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                muted(trn!(P::ScreenshotsSummary, list.len()))
                    .flex_1()
                    .min_w_0()
                    .truncate(),
            )
            .child(div().flex_1())
            .child(
                Button::new("open-screenshots", t(CommonMsg::OpenFolder))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some((_, dirs)) = this.dirs(cx) {
                            let dir = screenshots::dir(&dirs);
                            let _ = std::fs::create_dir_all(&dir);
                            cx.open_with_system(&dir);
                        }
                    })),
            )
            .child(
                Button::new("reload-screenshots", t(CommonMsg::Refresh))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| this.reload_screenshots(cx))),
            );
        if list.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap_3()
                .child(toolbar)
                .child(
                    card()
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .child(t(M::NoScreenshotsYet)),
                        )
                        .child(muted(t(M::NoScreenshotsHint)).whitespace_normal()),
                )
                .into_any_element();
        }
        let mut grid = div().flex().flex_row().flex_wrap().gap_3();
        for (index, shot) in list.iter().enumerate() {
            let thumb = self.thumbnail(shot, cx);
            let date = shot.modified.map(when).unwrap_or_default();
            grid = grid.child(
                div()
                    .id(("shot", index))
                    .accessible(
                        Role::Button,
                        format!("screenshot-{}", shot.file_name),
                        shot.file_name.clone(),
                    )
                    .aria_description(date.clone())
                    .keyboard_focus_outer()
                    .flex()
                    .flex_col()
                    .w(px(220.))
                    .rounded(radius::LG)
                    .overflow_hidden()
                    .border_1()
                    .border_color(crate::theme::ink::border())
                    .bg(colors::GRAY_800)
                    .cursor_pointer()
                    .hover(|style| style.border_color(crate::theme::accent::light().opacity(60)))
                    .on_press(cx.listener(move |this, _, _, cx| this.view(index, cx)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_full()
                            .h(px(124.))
                            .bg(colors::GRAY_900)
                            .child(match thumb {
                                Some(image) => img(image)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .into_any_element(),
                                None => LoadingDots.into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .px_2()
                            .py_1p5()
                            .child(
                                small(shot.file_name.clone())
                                    .truncate()
                                    .text_color(crate::theme::ink::text()),
                            )
                            .child(
                                small(format!("{date} · {}", format::bytes(shot.size)))
                                    .truncate()
                                    .text_color(crate::theme::ink::muted()),
                            ),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(toolbar)
            .child(grid)
            .into_any_element()
    }

    /// The large view of a screenshot, over the tab.
    fn render_viewer(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let index = self.viewing?;
        let shot = self.shot(index)?.clone();
        let count = self.screenshots.ready().map_or(0, Vec::len);
        let image = self
            .large
            .as_ref()
            .filter(|(p, _)| p == &shot.path)
            .map(|(_, load)| load.clone());
        let confirming = self.confirm_delete_shot.as_deref() == Some(shot.file_name.as_str());
        let path = shot.path.clone();
        let picture: AnyElement = match image {
            Some(Load::Ready(image)) => img(image)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            Some(Load::Failed(err)) => small(err).text_color(colors::RED_200).into_any_element(),
            _ => LoadingDots.into_any_element(),
        };
        Some(
            div()
                .id("screenshot-viewer")
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded(radius::LG)
                .bg(colors::GRAY_875)
                .occlude()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .font_family(fonts::heading())
                                        .font_weight(weights::BOLD.into())
                                        .child(shot.file_name.clone()),
                                )
                                .child(
                                    small(format!(
                                        "{} · {}{}",
                                        tr!(
                                            M::Position,
                                            index = format::number(index as u64 + 1),
                                            count = format::number(count as u64)
                                        ),
                                        format::bytes(shot.size),
                                        shot.modified
                                            .map(|t| format!(" · {}", when(t)))
                                            .unwrap_or_default()
                                    ))
                                    .text_color(crate::theme::ink::muted()),
                                ),
                        )
                        .child(
                            Button::new("shot-prev", t(CommonMsg::Previous))
                                .variant(ButtonVariant::Ghost)
                                .icon(Icon::ArrowLeft)
                                .size(ButtonSize::Slim)
                                .disabled(index == 0)
                                .on_click(cx.listener(|this, _, _, cx| this.step(-1, cx))),
                        )
                        .child(
                            Button::new("shot-next", t(CommonMsg::Next))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .disabled(index + 1 >= count)
                                .on_click(cx.listener(|this, _, _, cx| this.step(1, cx))),
                        )
                        .child(
                            Button::new("shot-close", t(CommonMsg::Close))
                                .variant(ButtonVariant::Secondary)
                                .icon(Icon::Xmark)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.close_view(cx))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .items_center()
                        .justify_center()
                        .rounded(radius::MD)
                        .overflow_hidden()
                        .bg(colors::GRAY_900)
                        .child(picture),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("shot-open", t(CommonMsg::OpenImage))
                                .variant(ButtonVariant::Secondary)
                                .icon(Icon::ArrowUpRightFromSquare)
                                .size(ButtonSize::Slim)
                                .on_click({
                                    let path = path.clone();
                                    move |_, _, cx| cx.open_with_system(&path)
                                }),
                        )
                        .child(
                            Button::new("shot-reveal", t(CommonMsg::ShowInFileManager))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click({
                                    let path = path.clone();
                                    move |_, _, cx| cx.reveal_path(&path)
                                }),
                        )
                        .child(
                            Button::new("shot-copy", t(M::CopyImage))
                                .variant(ButtonVariant::Ghost)
                                .icon(Icon::Copy)
                                .size(ButtonSize::Slim)
                                .on_click({
                                    let path = path.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.copy_shot(path.clone(), cx)
                                    })
                                }),
                        )
                        .child(
                            div().flex_1().min_w_0().children(
                                self.render_message().map(|message| message.text_right()),
                            ),
                        )
                        .child(
                            Button::new(
                                "shot-delete",
                                if confirming {
                                    t(CommonMsg::MoveToTrash)
                                } else {
                                    t(M::DeleteWorld)
                                },
                            )
                            .variant(ButtonVariant::Danger)
                            .icon(Icon::TrashCan)
                            .size(ButtonSize::Slim)
                            .on_click({
                                let name = shot.file_name.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.delete_shot(name.clone(), cx)
                                })
                            }),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for WorldsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_loaded(cx);
        let section = self.section;
        let worlds_label: SharedString = match self.worlds.ready() {
            Some(list) if !list.is_empty() => {
                format!("{} {}", t(M::Worlds), format::number(list.len() as u64)).into()
            }
            _ => t(M::Worlds).into(),
        };
        let shots_label: SharedString = match self.screenshots.ready() {
            Some(list) if !list.is_empty() => format!(
                "{} {}",
                t(M::Screenshots),
                format::number(list.len() as u64)
            )
            .into(),
            _ => t(M::Screenshots).into(),
        };
        let entity = cx.entity();
        let switch = pills(
            "worlds-sections",
            vec![worlds_label, shots_label],
            match section {
                Section::Worlds => 0,
                Section::Screenshots => 1,
            },
            move |index, _, cx| {
                let section = if index == 0 {
                    Section::Worlds
                } else {
                    Section::Screenshots
                };
                entity.update(cx, |this, cx| {
                    this.select_section(section, cx);
                    this.ensure_loaded(cx);
                });
            },
        );
        let body = match section {
            Section::Worlds => self.render_worlds(cx),
            Section::Screenshots => self.render_screenshots(cx),
        };
        let viewer = if section == Section::Screenshots {
            self.render_viewer(cx)
        } else {
            None
        };
        let accepts_drop = section == Section::Worlds;
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .child(div().flex().flex_row().items_center().gap_3().child(switch))
            .children(self.render_job(cx))
            .when(viewer.is_none(), |this| {
                this.children(self.render_message())
            })
            .child(
                div()
                    .id(("worlds-body", section as usize))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(radius::LG)
                    .border_2()
                    .border_color(colors::TRANSPARENT)
                    .when(accepts_drop, |this| {
                        this.drag_over::<ExternalPaths>(|style, _, _, _| {
                            style
                                .border_color(crate::theme::accent::light().opacity(60))
                                .bg(crate::theme::accent::base().opacity(8))
                        })
                        .on_drop(cx.listener(
                            |this, paths: &ExternalPaths, _, cx| {
                                this.import_worlds(paths.paths().to_vec(), cx)
                            },
                        ))
                    })
                    .child(div().pb_4().child(body)),
            )
            .children(viewer)
            .text_size(theme::text_size(text::MEDIUM))
    }
}

/// The tab's jobs across a change of the selected instance, with the sample instances
/// of the offscreen screenshots.
#[cfg(all(test, feature = "screenshot"))]
mod job_tests {
    use futures::channel::oneshot;
    use gpui::TestAppContext;

    use super::*;

    struct Setup {
        instances: Entity<Instances>,
        view: Entity<WorldsView>,
        a: String,
        b: String,
    }

    fn setup(cx: &mut TestAppContext) -> Setup {
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        let (a, b) = instances.read_with(cx, |instances, _| {
            (
                instances.list()[0].id.clone(),
                instances.list()[1].id.clone(),
            )
        });
        instances.update(cx, |instances, cx| instances.select(&a, cx));
        let view = cx.new(|cx| WorldsView::new(instances.clone(), cx));
        cx.run_until_parked();
        Setup {
            instances,
            view,
            a,
            b,
        }
    }

    /// Starts a job on the shown instance that runs until `oneshot` is sent.
    fn start(view: &Entity<WorldsView>, cx: &mut TestAppContext) -> oneshot::Sender<()> {
        let (done, wait) = oneshot::channel::<()>();
        view.update(cx, |view, cx| {
            view.run_async(
                JobLabel::SavingPackList,
                move |_, _, _| async move {
                    wait.await.ok();
                    Ok("Restored".to_string())
                },
                cx,
            )
        });
        done
    }

    fn select(s: &Setup, id: &str, cx: &mut TestAppContext) {
        s.instances
            .update(cx, |instances, cx| instances.select(id, cx));
        cx.run_until_parked();
    }

    fn busy(s: &Setup, id: &str, cx: &mut TestAppContext) -> bool {
        s.instances
            .read_with(cx, |instances, _| instances.files_busy(id))
    }

    #[test]
    fn a_job_goes_on_and_is_the_tabs_again_on_its_instance() {
        let mut cx = TestAppContext::single();
        let s = setup(&mut cx);
        let done = start(&s.view, &mut cx);
        assert!(s.view.read_with(&cx, |view, _| view.job.is_some()));
        assert!(busy(&s, &s.a, &mut cx), "Play waits for it (LS-48)");

        // LS-49: another instance is shown, and the job goes on
        select(&s, &s.b, &mut cx);
        assert!(s.view.read_with(&cx, |view, _| view.job.is_none()));
        assert!(busy(&s, &s.a, &mut cx));
        assert!(!busy(&s, &s.b, &mut cx));

        // back on its instance it is the tab's job again: a second one waits
        select(&s, &s.a, &mut cx);
        assert!(s.view.read_with(&cx, |view, _| view.job.is_some()));
        let second = s.view.update(&mut cx, |view, cx| view.can_change(cx));
        assert!(!second, "a second job on the same worlds waits");

        done.send(()).unwrap();
        cx.run_until_parked();
        s.view.read_with(&cx, |view, _| {
            assert!(view.job.is_none());
            let (message, error, _) = view.message.clone().expect("how it went");
            assert_eq!((message.as_ref(), error), ("Restored", false));
        });
        assert!(!busy(&s, &s.a, &mut cx));
    }

    #[test]
    fn a_job_that_finished_meanwhile_says_how_it_went() {
        let mut cx = TestAppContext::single();
        let s = setup(&mut cx);
        let done = start(&s.view, &mut cx);
        select(&s, &s.b, &mut cx);
        // the other instance has a job of its own meanwhile, which stays the tab's
        let other = start(&s.view, &mut cx);
        done.send(()).unwrap();
        cx.run_until_parked();
        assert!(!busy(&s, &s.a, &mut cx));
        s.view.read_with(&cx, |view, _| {
            assert!(view.job.is_some(), "the shown instance's job runs on");
            assert!(view.message.is_none(), "said on its own instance");
        });
        select(&s, &s.a, &mut cx);
        s.view.read_with(&cx, |view, _| {
            assert!(view.job.is_none());
            let (message, _, _) = view.message.clone().expect("how it went");
            assert_eq!(message.as_ref(), "Restored");
        });
        other.send(()).unwrap();
        cx.run_until_parked();
        assert!(!busy(&s, &s.b, &mut cx));
    }
}
