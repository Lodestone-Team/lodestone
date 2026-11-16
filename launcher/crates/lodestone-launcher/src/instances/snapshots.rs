//! Snapshots in the Launcher: the list in an instance's Settings tab, and the dialog that
//! takes one, restores one (its worlds, the worlds and their mods, everything, or as a new
//! instance), exports one as an image file (dragged out of the window, or saved), deletes
//! one, or makes a new instance from an image file.
//!
//! Snapshots are images in the image store (`launcher_core::images`, "M5 decisions" in
//! `docs/instance-images.md`). The work runs on background threads; the dialog shows its
//! progress.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::StreamExt as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, ExternalDragPayload,
    FileDragPaths, FocusHandle, Focusable, IntoElement, Render, SharedString, Task, Window, div,
    prelude::*,
};
use launcher_core::game::download::CancelToken;
use launcher_core::images::export::{self, ExportReport, ImportPreview};
use launcher_core::images::from_server::{PackPlatform, PackShare, SkipReason};
use launcher_core::images::{
    ContentDiff, RestorePreview, RestoreScope, SnapshotKind, SnapshotRecord, Take,
};

use super::{Instances, new_instance::form_row};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Checkbox, Dialog, Icon, LoadingDots, TextInput, Toggle,
        segmented, settings::card,
    },
    i18n::{CommonMsg, InstanceSettingsMsg as M, InstanceSettingsPlural as P, format, t, tr, trn},
    theme::{self, colors, px, radius, text, weights},
};

/// An instance's snapshots, as the list shows them.
#[derive(Clone, Debug, Default)]
pub struct SnapshotList {
    /// Newest first.
    pub records: Vec<SnapshotRecord>,
    /// Images of the instance that can't be read.
    pub unreadable: usize,
}

/// When a snapshot was taken, in the user's time.
pub fn when(created: i64) -> String {
    let time = chrono::DateTime::from_timestamp(created, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    format::day_and_time(time, chrono::Local::now().date_naive())
}

/// The badge a snapshot's row shows, if any: why it was taken, or that it is pinned. A
/// safety snapshot has none: its name ("Before going back to “…”") says why already.
pub fn badge(record: &SnapshotRecord) -> Option<M> {
    if record.pinned {
        return Some(M::BadgePinned);
    }
    match record.kind {
        SnapshotKind::Manual => None,
        SnapshotKind::BeforeChange => Some(M::BadgeAutomatic),
        SnapshotKind::AfterPlay => Some(M::BadgeAfterPlaying),
        SnapshotKind::BeforeRestore => None,
        SnapshotKind::Imported => Some(M::BadgeImported),
    }
}

/// "Monday 14:30 · Survival Island, Castle Build · 412 MB, 38 MB new · With mods": a
/// snapshot's details, its worlds by their own names (as the restore dialog and the Worlds
/// tab show them), else by their folders, and whether it holds the mods (a full image).
/// Worlds of one name ("New World", the game's default) are told apart by their folders.
pub fn details(record: &SnapshotRecord) -> String {
    let mut parts = vec![when(record.created)];
    if !record.worlds.is_empty() {
        let names: Vec<String> = record.worlds.iter().map(|w| record.world_name(w)).collect();
        let worlds: Vec<String> = record
            .worlds
            .iter()
            .zip(&names)
            .map(|(folder, name)| {
                let shared = names.iter().filter(|other| *other == name).count() > 1;
                let shown = if shared && name != folder {
                    format!("{name} ({folder})")
                } else {
                    name.clone()
                };
                launcher_core::images::display_text(&shown).into_owned()
            })
            .collect();
        parts.push(format::list(&worlds));
    }
    // A snapshot that only refers to files others have stores nothing new: say so, so its
    // size doesn't read as space it takes.
    parts.push(if record.new_bytes == 0 {
        tr!(M::SnapshotNothingNew, total = format::bytes(record.bytes))
    } else {
        tr!(
            M::SnapshotSizes,
            total = format::bytes(record.bytes),
            new = format::bytes(record.new_bytes)
        )
    });
    parts.join(" · ")
}

/// Whether the content that differs includes mods (anything under `mods/` or `coremods/`)
/// or the loader version, rather than only configs and other files a mod rewrites when the
/// game starts.
pub fn mods_differ(diff: &ContentDiff) -> bool {
    let is_mod = |path: &String| {
        path.split_once('/').is_some_and(|(first, _)| {
            first.eq_ignore_ascii_case("mods") || first.eq_ignore_ascii_case("coremods")
        })
    };
    diff.loader_version.is_some()
        || diff
            .added
            .iter()
            .chain(&diff.removed)
            .chain(&diff.updated)
            .any(is_mod)
}

/// How many names a list of files in the import and export dialogs shows before "…".
const LISTED_NAMES: usize = 8;

/// Why a downloaded world's instance lacks its server's modpack's own files, said in the
/// window's language.
pub(crate) fn pack_skipped(why: SkipReason) -> String {
    t(match why {
        SkipReason::Gone => M::ImportPackSkippedGone,
        SkipReason::Curseforge => M::ImportPackSkippedCurseforge,
        SkipReason::Failed => M::ImportPackSkipped,
    })
    .to_string()
}

/// The first [`LISTED_NAMES`] of `names` as a list, and "…" when there are more: a file
/// from someone else can name a million, and the dialog draws the line every frame.
fn shortened(names: &[String]) -> String {
    let mut shown: Vec<String> = names.iter().take(LISTED_NAMES).cloned().collect();
    if names.len() > LISTED_NAMES {
        shown.push("…".into());
    }
    format::list(&shown)
}

/// How many names a line about differing mods shows before "…".
const DIFF_NAMES: usize = 5;

/// What putting the snapshot's mods back would do, one line each: the files it puts back
/// (in the snapshot, not now), removes (now, not in the snapshot) and changes, with a few
/// names each.
pub fn restore_changes(diff: &ContentDiff) -> Vec<String> {
    let names = |paths: &[String]| {
        let mut shown: Vec<String> = paths
            .iter()
            .take(DIFF_NAMES)
            .map(|p| launcher_core::images::display_text(p).into_owned())
            .collect();
        if paths.len() > DIFF_NAMES {
            shown.push("…".into());
        }
        format::list(&shown)
    };
    let mut lines = Vec::new();
    if !diff.added.is_empty() {
        lines.push(trn!(
            P::RestorePutsBack,
            diff.added.len(),
            names = names(&diff.added)
        ));
    }
    if !diff.removed.is_empty() {
        lines.push(trn!(
            P::RestoreRemoves,
            diff.removed.len(),
            names = names(&diff.removed)
        ));
    }
    if !diff.updated.is_empty() {
        lines.push(trn!(
            P::RestoreChanges,
            diff.updated.len(),
            names = names(&diff.updated)
        ));
    }
    lines
}

impl Instances {
    /// Reads an instance's snapshots again whenever the image store says they changed,
    /// whoever changed them: the snapshots the Launcher takes on its own (after playing,
    /// before an update or a loader change), retention's deletions, the Worlds tab, the
    /// control API.
    pub(super) fn follow_snapshot_changes(&mut self, cx: &mut Context<Self>) {
        let mut changes = self.images().watch_snapshots();
        cx.spawn(async move |this, cx| {
            while let Some(id) = changes.next().await {
                let alive = this.update(cx, |this, cx| {
                    *this.snapshot_revisions.entry(id.clone()).or_default() += 1;
                    if this.snapshot_rows.contains_key(&id) || this.snapshot_tasks.contains_key(&id)
                    {
                        this.load_snapshots(&id, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// How many times the instance's snapshot list changed since the Launcher started.
    pub fn snapshot_revision(&self, id: &str) -> u64 {
        self.snapshot_revisions.get(id).copied().unwrap_or(0)
    }

    /// The instance's snapshots, once read; reads them when not yet.
    pub fn snapshot_list(&mut self, id: &str, cx: &mut Context<Self>) -> Option<&SnapshotList> {
        if !self.snapshot_rows.contains_key(id) && !self.snapshot_tasks.contains_key(id) {
            self.load_snapshots(id, cx);
        }
        self.snapshot_rows.get(id)
    }

    /// Reads the instance's snapshots again, in the background.
    pub fn load_snapshots(&mut self, id: &str, cx: &mut Context<Self>) {
        let images = self.images();
        let key = id.to_string();
        let read = cx.background_spawn({
            let key = key.clone();
            async move {
                images
                    .snapshots(&key)
                    .map(|found| SnapshotList {
                        records: found.records,
                        unreadable: found.unreadable.len(),
                    })
                    .unwrap_or_default()
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let list = read.await;
            this.update(cx, |this, cx| {
                this.snapshot_tasks.remove(&key);
                this.snapshot_rows.insert(key, list);
                cx.notify();
            })
            .ok();
        });
        self.snapshot_tasks.insert(id.to_string(), task);
    }

    /// Pins or unpins a snapshot, then reads the list again.
    pub fn pin_snapshot(&mut self, id: &str, snapshot: &str, pinned: bool, cx: &mut Context<Self>) {
        let images = self.images();
        let (instance, snapshot) = (id.to_string(), snapshot.to_string());
        let work = cx.background_spawn({
            let instance = instance.clone();
            async move { images.set_pinned(&instance, &snapshot, pinned) }
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    log::warn!("Couldn't pin the snapshot: {err}");
                    cx.emit(super::InstancesEvent::Notice {
                        title: t(M::SnapshotsNotChanged).into(),
                        message: err.to_string().into(),
                        about: super::About::Instance(instance.clone()),
                    });
                }
                this.load_snapshots(&instance, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Deletes the instance's images that can't be read, then reads the list again.
    pub fn delete_unreadable_images(&mut self, id: &str, cx: &mut Context<Self>) {
        let images = self.images();
        let instance = id.to_string();
        let work = cx.background_spawn({
            let instance = instance.clone();
            async move {
                let found = images.snapshots(&instance)?;
                for (image, _) in found.unreadable {
                    images.delete_unreadable(&instance, &image)?;
                }
                Ok::<_, launcher_core::images::ImageError>(())
            }
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if let Err(err) = result {
                    log::warn!("Couldn't delete the images that can't be read: {err}");
                    cx.emit(super::InstancesEvent::Notice {
                        title: t(M::SnapshotsNotChanged).into(),
                        message: err.to_string().into(),
                        about: super::About::Instance(instance.clone()),
                    });
                }
                this.load_snapshots(&instance, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Sets whether the instance takes a snapshot after each play session: `None`
    /// follows Settings › Instance defaults. A failure is also shown, as the toggle only
    /// springs back.
    pub fn set_snapshot_after_play(
        &mut self,
        id: &str,
        after_play: Option<bool>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let saved = self
            .store
            .get(id)
            .map_err(|e| e.to_string())
            .and_then(|mut instance| {
                let mut settings = instance.snapshots.unwrap_or_default();
                settings.after_play = after_play;
                instance.snapshots = (!settings.is_empty()).then_some(settings);
                self.save(&instance, cx)
            });
        if let Err(err) = &saved {
            cx.emit(super::InstancesEvent::Notice {
                title: t(M::SnapshotsNotChanged).into(),
                message: err.clone().into(),
                about: super::About::Instance(id.to_string()),
            });
        }
        saved
    }

    /// Where exported snapshot files are written before they are dragged out or saved:
    /// the cache, which Settings › Storage can clean.
    pub fn snapshot_exports_dir(&self) -> PathBuf {
        self.data_dir.cache_dir().join("snapshot-exports")
    }

    /// Where "Host this" writes the file it sends ([`launcher_core::cores::hosting`]).
    pub fn host_uploads_dir(&self) -> PathBuf {
        self.data_dir
            .cache_dir()
            .join(launcher_core::cores::hosting::UPLOADS_FOLDER)
    }

    /// Where "Download this world" saves the file it gets
    /// ([`launcher_core::cores::world_download`]).
    pub fn world_downloads_dir(&self) -> PathBuf {
        self.data_dir
            .cache_dir()
            .join(launcher_core::cores::world_download::DOWNLOADS_FOLDER)
    }
}

/// What the Settings tab (and the window) asks the shell to open.
#[derive(Clone, Debug)]
pub enum SnapshotRequest {
    Take(String),
    Restore(String, String),
    Export(String, String),
    Delete(String, String),
    /// An image file to make a new instance from.
    Import(PathBuf),
    /// A server's world "Download this world" got: an instance is made from it as from a
    /// file, named after the server, and the file is deleted when the dialog goes.
    ImportDownloaded {
        file: Arc<launcher_core::cores::world_download::Downloaded>,
        server: String,
    },
}

pub struct SnapshotRequested(pub SnapshotRequest);

/// The dialog is done and can close.
pub struct SnapshotDismissed;

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Take,
    Restore,
    Export,
    Delete,
    Import,
}

/// What a restore puts back, as the dialog offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Worlds,
    WorldsAndMods,
    Everything,
    AsNew,
}

/// What the dialog is doing.
enum State {
    /// Waiting for the user (or, while `preview` loads, for the disk).
    Ready,
    Working(M),
    /// Export: the file is ready to drag or save.
    Exported(ExportReport),
    /// Finished, with what to say about it (a restore in place).
    Done(SharedString),
    Failed(SharedString),
}

pub struct SnapshotDialog {
    instances: Entity<Instances>,
    mode: Mode,
    instance_id: Option<String>,
    snapshot: Option<SnapshotRecord>,
    import_path: Option<PathBuf>,
    /// Import: a world downloaded from this server, kept (and so its file) while the
    /// dialog is open.
    downloaded: Option<(
        Arc<launcher_core::cores::world_download::Downloaded>,
        String,
    )>,
    /// Restore: what it would do.
    preview: Option<RestorePreview>,
    /// Import: what the file holds.
    import: Option<ImportPreview>,
    choice: Choice,
    /// The choice to open on instead of the default, when the snapshot offers it (the
    /// offscreen screenshots).
    preset: Option<Choice>,
    /// Restore: the worlds chosen.
    worlds: Vec<String>,
    name: Entity<TextInput>,
    state: State,
    progress: Arc<(AtomicU64, AtomicU64)>,
    /// Import: the downloads' progress, which the bar shows while it runs.
    import_progress: Option<Arc<launcher_core::game::download::Progress>>,
    /// Import: the free space on the instances' disk, read with the file.
    import_free: Option<u64>,
    /// Cancels the work under way: Cancel does, and so does the dialog going (an export
    /// nobody waits for any more stops being written). A new one for each piece of work.
    cancel: CancelToken,
    /// Cancel was pressed: the work stops at its next check (a restore's swap, once
    /// started, finishes).
    stopping: bool,
    focus: FocusHandle,
    _work: Option<Task<()>>,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<SnapshotDismissed> for SnapshotDialog {}

impl Drop for SnapshotDialog {
    fn drop(&mut self) {
        // An export left before it is written stops (its partial file is removed).
        // A downloaded world's file goes with `downloaded`, deleted away from the window
        // (`Downloaded`'s own drop).
        self.cancel.cancel();
    }
}

impl Focusable for SnapshotDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SnapshotDialog {
    pub fn new(
        instances: Entity<Instances>,
        request: SnapshotRequest,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::InstanceSnapshots)
                .with_automation_id("snapshot-name")
        });
        let mut downloaded = None;
        let (mode, instance_id, snapshot_id, import_path) = match request {
            SnapshotRequest::Take(id) => (Mode::Take, Some(id), None, None),
            SnapshotRequest::Restore(id, s) => (Mode::Restore, Some(id), Some(s), None),
            SnapshotRequest::Export(id, s) => (Mode::Export, Some(id), Some(s), None),
            SnapshotRequest::Delete(id, s) => (Mode::Delete, Some(id), Some(s), None),
            SnapshotRequest::Import(path) => (Mode::Import, None, None, Some(path)),
            SnapshotRequest::ImportDownloaded { file, server } => {
                let path = file.path().to_path_buf();
                downloaded = Some((file, server));
                (Mode::Import, None, None, Some(path))
            }
        };
        let mut this = Self {
            instances,
            mode,
            instance_id,
            snapshot: None,
            import_path,
            downloaded,
            preview: None,
            import: None,
            choice: Choice::Worlds,
            preset: None,
            worlds: Vec::new(),
            name,
            state: State::Ready,
            progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),
            import_progress: None,
            import_free: None,
            cancel: CancelToken::new(),
            stopping: false,
            focus: cx.focus_handle(),
            _work: None,
            _ticker: None,
        };
        this.load(snapshot_id, cx);
        this
    }

    /// Reads what the dialog shows: the snapshot and what restoring it would do, or a file.
    fn load(&mut self, snapshot_id: Option<String>, cx: &mut Context<Self>) {
        let images = self.instances.read(cx).images();
        match self.mode {
            Mode::Take => {
                let default_name = format::short_date(chrono::Local::now().date_naive(), true);
                self.name
                    .update(cx, |input, cx| input.set_text(default_name, cx));
            }
            Mode::Restore | Mode::Export | Mode::Delete => {
                let (Some(id), Some(snapshot_id)) = (self.instance_id.clone(), snapshot_id) else {
                    return;
                };
                let restore = self.mode == Mode::Restore;
                let read = cx.background_spawn(async move {
                    let record = images.snapshot(&id, &snapshot_id)?;
                    let preview = if restore {
                        Some(images.preview_restore(&id, &snapshot_id)?)
                    } else {
                        None
                    };
                    Ok::<_, launcher_core::images::ImageError>((record, preview))
                });
                self.state = State::Working(M::ReadingSnapshot);
                self._work = Some(cx.spawn(async move |this, cx| {
                    let result = read.await;
                    this.update(cx, |this, cx| {
                        match result {
                            Ok((record, preview)) => this.loaded(record, preview, cx),
                            Err(err) => this.state = State::Failed(err.to_string().into()),
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
            Mode::Import => {
                let Some(path) = self.import_path.clone() else {
                    return;
                };
                self.state = State::Working(M::ReadingSnapshot);
                let root = self.instances.read(cx).store().root().to_path_buf();
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().trim().to_string())
                    .filter(|s| !s.is_empty());
                let server = self.downloaded.as_ref().map(|(_, server)| server.clone());
                let read = cx.background_spawn({
                    let server = server.clone();
                    async move {
                        let preview = match &server {
                            Some(server) => export::inspect_from_host(&path, server)?,
                            None => export::inspect(&path)?,
                        };
                        Ok::<_, launcher_core::images::ImageError>((
                            preview,
                            launcher_core::disk::free_space(&root),
                        ))
                    }
                });
                self._work = Some(cx.spawn(async move |this, cx| {
                    let result = read.await;
                    this.update(cx, |this, cx| {
                        match result {
                            Ok((preview, free)) => {
                                // Exports are named "<instance> - <snapshot>": the file's
                                // name says more than the snapshot's label alone. A
                                // server's world is named after the server.
                                let name = match (&preview.from_server, server) {
                                    (_, Some(server)) => export::shown_name(&server),
                                    (Some(from), None) => from.name.clone(),
                                    (None, None) => {
                                        stem.unwrap_or_else(|| preview.name_shown.clone())
                                    }
                                };
                                this.name.update(cx, |input, cx| input.set_text(name, cx));
                                this.import = Some(preview);
                                this.import_free = free;
                                this.state = State::Ready;
                            }
                            Err(err) => this.state = State::Failed(err.to_string().into()),
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
        }
    }

    /// Opens on `choice` once the snapshot is read, when it offers it (the offscreen
    /// screenshots).
    pub fn preset_choice(&mut self, choice: Choice) {
        self.preset = Some(choice);
    }

    fn loaded(
        &mut self,
        record: SnapshotRecord,
        preview: Option<RestorePreview>,
        cx: &mut Context<Self>,
    ) {
        self.state = State::Ready;
        if let Some(preview) = &preview {
            self.worlds = preview.worlds.iter().map(|w| w.name.clone()).collect();
            self.choice = self
                .preset
                .filter(|choice| offered(preview).contains(choice))
                .unwrap_or_else(|| default_choice(preview));
            let instance = self
                .instance_id
                .as_deref()
                .and_then(|id| self.instances.read(cx).list().iter().find(|i| i.id == id))
                .map(|i| i.name.clone())
                .unwrap_or_default();
            let name = format!("{instance} ({})", record.name_shown());
            self.name.update(cx, |input, cx| input.set_text(name, cx));
        }
        self.preview = preview;
        self.snapshot = Some(record);
        if self.mode == Mode::Export {
            self.export(cx);
        }
    }

    /// Redraws while work runs, for the progress bar.
    fn tick(&mut self, cx: &mut Context<Self>) {
        self._ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let working = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        matches!(this.state, State::Working(_))
                    })
                    .unwrap_or(false);
                if !working {
                    break;
                }
            }
        }));
    }

    fn game_running(&self, cx: &App) -> bool {
        self.instance_id
            .as_deref()
            .is_some_and(|id| self.instances.read(cx).is_active(id))
    }

    /// Runs `work` in the background with progress and a token Cancel cancels. Then, on
    /// the main thread, `after` brings the instances up to date with what it did (the
    /// snapshot list, a new instance), and `done` updates the dialog. `after` runs even
    /// when the dialog has gone.
    fn run<T: Send + 'static>(
        &mut self,
        label: M,
        work: impl FnOnce(&(dyn Fn(u64, u64) + Sync), &CancelToken) -> Result<T, String>
        + Send
        + 'static,
        after: impl FnOnce(&mut Instances, &T, &mut Context<Instances>) + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.state = State::Working(label);
        self.stopping = false;
        self.cancel = CancelToken::new();
        self.progress.0.store(0, Ordering::Relaxed);
        self.progress.1.store(0, Ordering::Relaxed);
        let progress = self.progress.clone();
        let cancel = self.cancel.clone();
        let job = cx.background_spawn(async move {
            work(
                &move |done, total| {
                    progress.0.store(done, Ordering::Relaxed);
                    progress.1.store(total, Ordering::Relaxed);
                },
                &cancel,
            )
        });
        let instances = self.instances.clone();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            if let Ok(value) = &result {
                instances.update(cx, |instances, cx| after(instances, value, cx));
            }
            this.update(cx, |this, cx| {
                this.stopping = false;
                match result {
                    Ok(value) => {
                        this.state = State::Ready;
                        done(this, value, cx);
                    }
                    Err(err) => this.state = State::Failed(err.into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        self.tick(cx);
        cx.notify();
    }

    /// Whether closing now would cut work short: taking, restoring or importing goes on
    /// until it ends or Cancel stops it, and the dialog shows how it ends; reading or
    /// writing a file to share, or deleting, can be left. The shell queues a dialog
    /// that would replace this one while it is busy.
    pub(crate) fn busy(&self) -> bool {
        busy(&self.state)
    }

    /// Puts the dialog in the middle of work, or out of it, for the shell's tests.
    #[cfg(all(test, feature = "screenshot"))]
    pub(crate) fn test_set_working(&mut self, working: bool) {
        self.state = if working {
            State::Working(M::ImportingSnapshot)
        } else {
            State::Ready
        };
    }

    /// Cancel: closes the dialog, or, while work it shows runs, stops that work (the
    /// dialog then says it stopped). Deleting is short and can't be stopped.
    fn cancel_or_close(&mut self, cx: &mut Context<Self>) {
        if !self.busy() {
            return cx.emit(SnapshotDismissed);
        }
        self.cancel.cancel();
        self.stopping = true;
        cx.notify();
    }

    fn take(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.instance_id.clone() else {
            return;
        };
        if self.game_running(cx) {
            self.state = State::Failed(t(M::SnapshotGameRunning).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        // A modpack update or an install half done would be recorded as the mods the
        // worlds ran with.
        if instances.files_or_pack_busy(&id) {
            self.state = State::Failed(t(crate::i18n::InstancesMsg::FilesBusy).into());
            return cx.notify();
        }
        let images = instances.images();
        // Play waits until the work is done, even if the dialog is closed meanwhile.
        let busy = instances.mark_files_busy(&id);
        let typed = self.name.read(cx).text().trim().to_string();
        launcher_core::trace::emit("snapshot.create", || serde_json::json!({ "instance": id }));
        let instance = id.clone();
        self.run(
            M::TakingSnapshot,
            move |progress, cancel| {
                let _busy = busy;
                images
                    .take_snapshot(
                        &instance,
                        Take {
                            kind: SnapshotKind::Manual,
                            name: Some(typed),
                            progress,
                            cancel: &|| cancel.is_cancelled(),
                        },
                    )
                    .map_err(|err| err.to_string())
            },
            move |instances, _, cx| instances.load_snapshots(&id, cx),
            |_, _, cx| cx.emit(SnapshotDismissed),
            cx,
        );
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(snapshot)) = (self.instance_id.clone(), self.snapshot.clone()) else {
            return;
        };
        let scope = match self.choice {
            Choice::Worlds | Choice::WorldsAndMods if self.worlds.is_empty() => {
                self.state = State::Failed(t(M::ChooseWorld).into());
                return cx.notify();
            }
            Choice::Worlds => Some(RestoreScope::Worlds(self.worlds.clone())),
            Choice::WorldsAndMods => Some(RestoreScope::WorldsAndContent(self.worlds.clone())),
            Choice::Everything => Some(RestoreScope::Everything),
            Choice::AsNew => None,
        };
        // "As a new instance" only reads the store, so it can run while the game does.
        if scope.is_some() && self.game_running(cx) {
            self.state = State::Failed(t(M::SnapshotGameRunning).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        // Another job writing the instance's files (an install, a modpack update) would
        // lose what it writes into what the restore replaces.
        if scope.is_some() && instances.files_or_pack_busy(&id) {
            self.state = State::Failed(t(crate::i18n::InstancesMsg::FilesBusy).into());
            return cx.notify();
        }
        let images = instances.images();
        let busy = instances.mark_files_busy(&id);
        let name = self.name.read(cx).text().trim().to_string();
        launcher_core::trace::emit(
            "snapshot.restore",
            || serde_json::json!({ "instance": id, "snapshot": snapshot.id }),
        );
        let instance = id.clone();
        let snapshot_name = snapshot.name_shown();
        match scope {
            Some(scope) => self.run(
                M::RestoringSnapshot,
                move |_, cancel| {
                    let _busy = busy;
                    images
                        .restore(&instance, &snapshot.id, &scope, &|| cancel.is_cancelled())
                        .map_err(|err| err.to_string())
                },
                move |instances, restored, cx| {
                    instances.load_snapshots(&id, cx);
                    instances.reload(cx);
                    if restored.instance.is_some() {
                        cx.emit(super::InstancesEvent::LoaderChanged(id.clone()));
                    }
                    if let Some(kept) = &restored.kept_aside {
                        cx.emit(super::InstancesEvent::Notice {
                            title: t(M::RestoreKeptAsideTitle).into(),
                            message: tr!(M::RestoreKeptAside, folder = kept.display()).into(),
                            about: super::About::Instance(id.clone()),
                        });
                    }
                },
                // Stays open to say it worked: the files changed where the dialog can't
                // show it.
                move |this, _, _| {
                    this.state = State::Done(tr!(M::SnapshotRestored, name = snapshot_name).into())
                },
                cx,
            ),
            None => self.run(
                M::ImportingSnapshot,
                move |_, cancel| {
                    drop(busy);
                    images
                        .restore_as_new(&instance, &snapshot.id, &name, &|| cancel.is_cancelled())
                        .map_err(|err| err.to_string())
                },
                |instances, made, cx| {
                    instances.reload(cx);
                    instances.select(&made.id, cx);
                },
                |_, _, cx| cx.emit(SnapshotDismissed),
                cx,
            ),
        }
    }

    /// Writes the export to the cache, to drag out or save.
    fn export(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(snapshot)) = (self.instance_id.clone(), self.snapshot.clone()) else {
            return;
        };
        let instances = self.instances.read(cx);
        let images = instances.images();
        let instance_name = instances
            .list()
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        // A folder per snapshot: two snapshots of one name never write the same file.
        let file = instances
            .snapshot_exports_dir()
            .join(&snapshot.id)
            .join(format!(
                "{} - {}.{}",
                file_safe(&instance_name),
                file_safe(&snapshot.name),
                export::EXTENSION
            ));
        launcher_core::trace::emit(
            "snapshot.export",
            || serde_json::json!({ "snapshot": snapshot.id }),
        );
        self.run(
            M::PreparingFile,
            move |_, cancel| {
                images
                    .export_snapshot(&id, &snapshot.id, &file, &|| cancel.is_cancelled())
                    .map_err(|err| err.to_string())
            },
            |_, _, _| {},
            |this, report, _| this.state = State::Exported(report),
            cx,
        );
    }

    fn save_as(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = super::settings::dirs_downloads();
        let picked = cx.prompt_for_new_path(&folder, Some(&name));
        self._work = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(to))) = picked.await else {
                return;
            };
            // The file was only made to be shared: moved out of the cache (copied, then
            // deleted, to another disk), so it doesn't take its size twice.
            let copied = cx
                .background_spawn(async move {
                    if std::fs::rename(&file, &to).is_err() {
                        std::fs::copy(&file, &to)?;
                        let _ = std::fs::remove_file(&file);
                    }
                    if let Some(folder) = file.parent() {
                        let _ = std::fs::remove_dir(folder);
                    }
                    Ok::<(), std::io::Error>(())
                })
                .await;
            this.update(cx, |this, cx| {
                match copied {
                    Ok(()) => cx.emit(SnapshotDismissed),
                    Err(err) => this.state = State::Failed(err.to_string().into()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(snapshot)) = (self.instance_id.clone(), self.snapshot.clone()) else {
            return;
        };
        let images = self.instances.read(cx).images();
        launcher_core::trace::emit(
            "snapshot.delete",
            || serde_json::json!({ "snapshot": snapshot.id }),
        );
        let instance = id.clone();
        self.run(
            M::DeletingSnapshot,
            move |_, _| {
                images
                    .delete_snapshot(&instance, &snapshot.id)
                    .map_err(|err| err.to_string())
            },
            move |instances, _, cx| instances.load_snapshots(&id, cx),
            |_, _, cx| cx.emit(SnapshotDismissed),
            cx,
        );
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.import_path.clone() else {
            return;
        };
        let instances = self.instances.read(cx);
        let images = instances.images();
        let downloader = instances.content_downloader();
        let modrinth = instances.modrinth();
        let curseforge = instances.curseforge();
        let room = export::room_on(instances.store().root().to_path_buf());
        let name = self.name.read(cx).text().trim().to_string();
        let expected = self.import.as_ref().map(|preview| preview.image.clone());
        // the downloaded file stays while the import reads it, whatever the dialog does
        let keep = self.downloaded.as_ref().map(|(file, _)| file.clone());
        let server = self.downloaded.as_ref().map(|(_, server)| server.clone());
        let progress = Arc::new(launcher_core::game::download::Progress::default());
        self.import_progress = Some(progress.clone());
        launcher_core::trace::emit("snapshot.import", || serde_json::json!({}));
        self.run(
            M::ImportingSnapshot,
            move |_, cancel| {
                let Some(modrinth) = modrinth else {
                    return Err(t(M::ImportNeedsModrinth).to_string());
                };
                let _keep = keep;
                futures::executor::block_on(launcher_core::http::in_http_runtime(
                    export::import_file(
                        &images,
                        export::Sources {
                            downloader: &downloader,
                            modrinth: &modrinth,
                            curseforge: curseforge.as_deref(),
                        },
                        export::ImportRequest {
                            name: Some(&name),
                            expect_image: expected.as_deref(),
                            server: server.as_deref(),
                            ..export::ImportRequest::new(&path)
                        },
                        &room,
                        &progress,
                        cancel,
                    ),
                ))
                .map_err(|err| match err {
                    launcher_core::images::ImageError::NoRoom { needed, free } => tr!(
                        M::ImportNoRoom,
                        size = format::bytes(needed),
                        free = format::bytes(free)
                    ),
                    err => err.to_string(),
                })
            },
            |instances, imported, cx| {
                instances.reload(cx);
                instances.select(&imported.instance.id, cx);
                // What the new instance lacks, in one notice (a second would replace the
                // first): files no download confirmed (it failed, or wasn't possible) whose
                // bytes neither the file nor the store had, and a server's modpack whose
                // client share couldn't be had (the files only its players use).
                let mut said: Vec<(&'static str, String)> = Vec::new();
                if !imported.left_out.is_empty() {
                    let names: Vec<String> = imported
                        .left_out
                        .iter()
                        .map(|p| launcher_core::images::display_text(p).into_owned())
                        .collect();
                    said.push((
                        t(M::ImportLeftOutTitle),
                        tr!(M::ImportLeftOut, files = shortened(&names)),
                    ));
                }
                if let Some(PackShare::Skipped { why, .. }) = &imported.server_pack {
                    said.push((t(M::ImportPackSkippedTitle), pack_skipped(*why)));
                }
                if let Some((title, _)) = said.first() {
                    let message: Vec<String> = said.iter().map(|(_, m)| m.clone()).collect();
                    cx.emit(super::InstancesEvent::Notice {
                        title: (*title).into(),
                        message: message.join("\n\n").into(),
                        about: super::About::Instance(imported.instance.id.clone()),
                    });
                }
            },
            |_, _, cx| cx.emit(SnapshotDismissed),
            cx,
        );
    }

    fn title(&self) -> SharedString {
        let name = self
            .snapshot
            .as_ref()
            .map(SnapshotRecord::name_shown)
            .unwrap_or_default();
        match self.mode {
            Mode::Take => t(M::TakeSnapshotTitle).into(),
            Mode::Restore => tr!(M::RestoreTitle, name = name).into(),
            Mode::Export => tr!(M::ExportTitle, name = name).into(),
            Mode::Delete => tr!(M::DeleteSnapshotTitle, name = name).into(),
            Mode::Import
                if self
                    .import
                    .as_ref()
                    .is_some_and(|p| p.from_server.is_some()) =>
            {
                t(M::ImportServerTitle).into()
            }
            Mode::Import => t(M::ImportSnapshotTitle).into(),
        }
    }

    fn render_progress(&self, label: &'static str) -> impl IntoElement + use<> {
        let (done, total) = match &self.import_progress {
            Some(progress) => {
                let now = progress.snapshot();
                (now.bytes_done, now.bytes_total)
            }
            None => (
                self.progress.0.load(Ordering::Relaxed),
                self.progress.1.load(Ordering::Relaxed),
            ),
        };
        // Work that reports no size (a restore placing files, an import's first steps)
        // shows the label and dots only, not a bar that looks stuck at nothing.
        let fraction = (total > 0).then(|| (done as f32 / total as f32).clamp(0.0, 1.0));
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .text_color(colors::GRAY_400)
                    .child(LoadingDots)
                    .child(label),
            )
            .when_some(fraction, |this, fraction| {
                this.child(
                    div()
                        .h(px(6.))
                        .w_full()
                        .rounded_full()
                        .bg(colors::GRAY_700)
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(colors::GREEN_400)
                                .w(gpui::relative(fraction)),
                        ),
                )
            })
    }

    fn render_restore(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = &self.preview else {
            return div().child(LoadingDots).into_any_element();
        };
        let mut body = div().flex().flex_col().gap_3();
        let choices: Vec<(Choice, SharedString)> = offered(preview)
            .into_iter()
            .map(|choice| {
                let label = match choice {
                    Choice::Worlds => M::RestoreWorldsOnly,
                    Choice::WorldsAndMods => M::RestoreWorldsAndMods,
                    Choice::Everything => M::RestoreEverything,
                    Choice::AsNew => M::RestoreAsNew,
                };
                (choice, t(label).into())
            })
            .collect();
        let entity = cx.entity();
        body = body.child(segmented(
            "snapshot-restore-choice",
            choices,
            Some(&self.choice),
            false,
            move |choice, _, cx| {
                entity.update(cx, |this, cx| {
                    this.choice = choice;
                    cx.notify();
                })
            },
        ));
        match self.choice {
            Choice::Worlds | Choice::WorldsAndMods => {
                let mut rows: Vec<AnyElement> = Vec::new();
                for (index, world) in preview.worlds.iter().enumerate() {
                    let on = self.worlds.contains(&world.name);
                    let name = world.name.clone();
                    let label = launcher_core::images::display_text(
                        world.level_name.as_deref().unwrap_or(&world.name),
                    )
                    .into_owned();
                    let size = if world.exists_now {
                        format::bytes(world.size)
                    } else {
                        tr!(M::WorldGoneNow, size = format::bytes(world.size))
                    };
                    // The folder too when the world's own name differs, as the Worlds tab
                    // shows it: worlds often share a name ("New World", the default).
                    let detail = match world.level_name.as_deref() {
                        Some(level) if level != world.name => format!(
                            "{} · {size}",
                            launcher_core::images::display_text(&world.name)
                        ),
                        _ => size,
                    };
                    rows.push(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_2()
                            .child(
                                Checkbox::new(("snapshot-world", index), on, label).on_change(
                                    cx.listener(move |this, value: &bool, _, cx| {
                                        if *value {
                                            if !this.worlds.contains(&name) {
                                                this.worlds.push(name.clone());
                                            }
                                        } else {
                                            this.worlds.retain(|w| w != &name);
                                        }
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(div().flex_1())
                            .child(muted(detail))
                            .into_any_element(),
                    );
                }
                body = body.child(
                    div()
                        .max_h(px(220.))
                        .id("snapshot-worlds")
                        .overflow_y_scroll()
                        .child(card(rows)),
                );
                if self.choice == Choice::Worlds
                    && preview.can_restore_content
                    && let Some(diff) = preview.content.as_ref().filter(|d| !d.is_empty())
                {
                    let headline = if mods_differ(diff) {
                        M::ModsDiffer
                    } else {
                        M::FilesDiffer
                    };
                    // The toggle first, then what it changes, introduced as such: they
                    // only happen if it is turned on.
                    let mut differs = note(t(headline).to_string()).child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                Toggle::new("snapshot-mods-too", false)
                                    .label(t(M::RestoreModsToo))
                                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                                        if *value {
                                            this.choice = Choice::WorldsAndMods;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(muted(t(M::RestoreModsToo))),
                    );
                    let mut changes = restore_changes(diff);
                    if let Some((_, Some(version))) = &diff.loader_version {
                        changes.push(tr!(M::RestoreLoaderChange, version = version));
                    }
                    if !changes.is_empty() {
                        differs = differs.child(muted(t(M::RestoreModsTooChanges)));
                    }
                    for line in changes {
                        differs = differs.child(muted(line));
                    }
                    body = body.child(differs);
                }
            }
            Choice::Everything => {
                body = body.child(muted(t(M::RestoreEverythingHint)));
            }
            Choice::AsNew => {
                body = body.child(form_row(t(M::RestoreAsNewName), self.name.clone()));
            }
        }
        // What putting the content back changes, before Restore is pressed.
        if matches!(self.choice, Choice::WorldsAndMods | Choice::Everything)
            && let Some(diff) = &preview.content
        {
            for line in restore_changes(diff) {
                body = body.child(muted(line));
            }
            if let Some((_, Some(version))) = &diff.loader_version {
                body = body.child(muted(tr!(M::RestoreLoaderChange, version = version)));
            }
        }
        if preview.other_game {
            body = body.child(muted(t(M::RestoreOtherGame)));
        }
        if !preview.missing.is_empty() {
            body = body.child(note(trn!(P::RestoreMissing, preview.missing.len())));
        }
        if self.choice != Choice::AsNew {
            body = body.child(muted(t(M::RestoreSafety)));
        }
        body.into_any_element()
    }

    fn render_import(&self) -> AnyElement {
        let Some(preview) = &self.import else {
            return div().into_any_element();
        };
        let mut body = div().flex().flex_col().gap_3();
        // The sender wrote both versions: shown so nothing in them can look like more.
        let loader = match &preview.loader_version {
            Some(version) => format!(
                "{} {}",
                loader_name(&preview.loader),
                launcher_core::images::display_text(version)
            ),
            None => loader_name(&preview.loader),
        };
        body = body.child(div().font_weight(weights::BOLD.into()).child(tr!(
            M::ImportFrom,
            name = preview.name_shown,
            version = launcher_core::images::display_text(&preview.minecraft),
            loader = loader
        )));
        if !preview.worlds.is_empty() {
            body = body.child(muted(tr!(
                M::ImportWorlds,
                worlds = shortened(&preview.worlds)
            )));
        }
        if !preview.mods.is_empty() {
            let mut warning = note(trn!(P::ImportModsWarning, preview.mods.len()))
                .child(muted(shortened(&preview.mods)));
            if !preview.carried_mods.is_empty() {
                warning = warning.child(muted(trn!(
                    P::ImportCarriedMods,
                    preview.carried_mods.len(),
                    names = shortened(&preview.carried_mods)
                )));
            }
            body = body.child(warning);
        }
        if let Some(from) = &preview.from_server {
            body = body.child(muted(trn!(
                P::ImportFromServer,
                from.not_taken,
                server = from.server_shown
            )));
            if !from.server_only_mods.is_empty() {
                body = body.child(muted(tr!(
                    M::ImportServerOnlyMods,
                    names = shortened(&from.server_only_mods)
                )));
            }
            match from.modpack {
                // the pack's client share places jars no count above names: warned as mods are
                Some(PackPlatform::Modrinth) => {
                    body = body.child(note(t(M::ImportServerPack).to_string()));
                }
                Some(PackPlatform::Curseforge) => {
                    body = body.child(note(t(M::ImportServerPackCurseforge).to_string()));
                }
                None => {}
            }
            // what the core itself took out of the file before sending it
            if let Some((downloaded, _)) = &self.downloaded {
                let exported = &downloaded.exported;
                // what a player would miss: the server software is never a player's
                let left_out = &exported.left_out_for_players;
                if !left_out.is_empty() {
                    body = body.child(muted(tr!(M::ImportHostLeftOut, files = left_out.joined())));
                }
                if !exported.blanked.is_empty() {
                    body = body.child(muted(tr!(
                        M::ImportHostBlanked,
                        names = exported.blanked.joined()
                    )));
                }
            }
        }
        if preview.download_bytes > 0 {
            body = body.child(muted(tr!(
                M::ImportDownloads,
                size = format::bytes(preview.download_bytes)
            )));
        }
        if !preview.not_downloadable.is_empty() {
            body = body.child(note(tr!(
                M::ImportNotDownloadable,
                files = shortened(&preview.not_downloadable)
            )));
        }
        body = match self.import_free {
            Some(free) if preview.needed_bytes > free => body.child(note(tr!(
                M::ImportNoRoom,
                size = format::bytes(preview.needed_bytes),
                free = format::bytes(free)
            ))),
            Some(free) => body.child(muted(tr!(
                M::ImportSpaceFree,
                size = format::bytes(preview.needed_bytes),
                free = format::bytes(free)
            ))),
            None => body.child(muted(tr!(
                M::ImportSpace,
                size = format::bytes(preview.needed_bytes)
            ))),
        };
        body = body.child(form_row(t(M::SnapshotName), self.name.clone()));
        body.into_any_element()
    }

    /// The finished file, to drag into another app.
    fn render_file(
        &self,
        report: &ExportReport,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let file = &report.file;
        let name: SharedString = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        let save = file.clone();
        let folder = file.parent().map(PathBuf::from).unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("snapshot-file")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded(radius::LG)
                    .border_1()
                    .border_dashed()
                    .border_color(colors::GRAY_600)
                    .bg(colors::GRAY_800)
                    .cursor_grab()
                    .on_drag(file.clone(), {
                        let name = name.clone();
                        move |_, _, _, cx| cx.new(|_| FileChip(name.clone()))
                    })
                    // Leaving the window hands the OS the file.
                    .external_drag_payload(|path: &PathBuf, _, _| {
                        Some(ExternalDragPayload::Files(FileDragPaths::new([(
                            path.clone(),
                            false,
                        )])))
                    })
                    .child(Icon::Copy.render(px(18.), colors::GRAY_400))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(crate::theme::ink::muted())
                                    .child(format::bytes(report.bytes)),
                            ),
                    ),
            )
            .child(muted(t(M::ExportNote)))
            .when(!report.not_downloadable.is_empty(), |this| {
                this.child(note(tr!(
                    M::ExportNotDownloadable,
                    files = shortened(&report.not_downloadable)
                )))
            })
            .when(!report.left_out.is_empty(), |this| {
                this.child(muted(tr!(
                    M::ExportLeftOut,
                    files = shortened(&report.left_out)
                )))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(colors::GRAY_400)
                    .text_size(theme::text_size(text::SMALL))
                    .child(t(M::DragToShare))
                    .child(
                        Button::new("snapshot-save-as", t(M::SaveAsEllipsis))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.save_as(save.clone(), cx)),
                            ),
                    )
                    .child(
                        Button::new("snapshot-show", t(M::ShowInFolder))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .on_click(move |_, _, cx| cx.open_with_system(&folder)),
                    ),
            )
    }
}

/// The choices a restore of the snapshot `preview` describes offers, in the order shown:
/// the worlds (and their mods) when it has worlds, everything when it can put content
/// back, none of those for another game, and always a new instance.
fn offered(preview: &RestorePreview) -> Vec<Choice> {
    let mut choices = Vec::new();
    if !preview.other_game {
        if !preview.worlds.is_empty() {
            choices.push(Choice::Worlds);
            if preview.can_restore_content {
                choices.push(Choice::WorldsAndMods);
            }
        }
        if preview.can_restore_content {
            choices.push(Choice::Everything);
        }
    }
    choices.push(Choice::AsNew);
    choices
}

/// The choice a restore dialog opens on: only "as a new instance" for another game; the
/// worlds when there are any; everything when the snapshot can put content back; else as
/// a new instance (a data snapshot with no worlds whose content link is gone).
fn default_choice(preview: &RestorePreview) -> Choice {
    if preview.other_game {
        Choice::AsNew
    } else if !preview.worlds.is_empty() {
        Choice::Worlds
    } else if preview.can_restore_content {
        Choice::Everything
    } else {
        Choice::AsNew
    }
}

/// A loader's name as the Launcher shows it, from an image's id for it (`fabric`).
fn loader_name(id: &str) -> String {
    match id {
        "vanilla" => t(M::Vanilla).to_string(),
        "fabric" => "Fabric".into(),
        "quilt" => "Quilt".into(),
        "forge" => "Forge".into(),
        "neoforge" => "NeoForge".into(),
        other => launcher_core::images::display_text(other).into_owned(),
    }
}

/// Whether the dialog, in `state`, is doing work that closing would cut short (see
/// [`SnapshotDialog::busy`]).
fn busy(state: &State) -> bool {
    match state {
        State::Working(label) => !matches!(
            label,
            M::ReadingSnapshot | M::PreparingFile | M::DeletingSnapshot
        ),
        _ => false,
    }
}

/// A small grey line.
fn muted(text_: impl Into<SharedString>) -> gpui::Div {
    div()
        .whitespace_normal()
        .text_size(theme::text_size(text::SMALL))
        .text_color(colors::GRAY_400)
        .child(text_.into())
}

/// A note that asks for attention, with room for more under it.
fn note(message: String) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::YELLOW.opacity(40))
        .bg(colors::YELLOW.opacity(8))
        .child(div().text_color(colors::YELLOW_300).child(message))
}

/// The small label that follows the pointer while the file is dragged in the window.
struct FileChip(SharedString);

impl Render for FileChip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .rounded(radius::MD)
            .bg(colors::GRAY_800)
            .border_1()
            .border_color(colors::GRAY_600)
            .text_size(theme::text_size(text::SMALL))
            .child(self.0.clone())
    }
}

/// A name usable in a file name on every system: letters, digits and ` -_.()` only, no
/// leading dot, no trailing dot or space (Windows drops them).
pub(crate) fn file_safe(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .to_string();
    if cleaned.is_empty() {
        "snapshot".into()
    } else {
        cleaned
    }
}

impl Render for SnapshotDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .text_size(theme::text_size(text::MEDIUM));
        let working = matches!(self.state, State::Working(_));
        match (&self.mode, &self.state) {
            (_, State::Working(label)) => {
                let label = if self.stopping {
                    t(crate::i18n::InstancesMsg::CancellingNote)
                } else {
                    t(*label)
                };
                body = body.child(self.render_progress(label));
            }
            (Mode::Export, State::Exported(report)) => {
                let report = report.clone();
                body = body.child(self.render_file(&report, cx));
            }
            (Mode::Take, _) => {
                body = body.child(form_row(t(M::SnapshotName), self.name.clone()));
            }
            (_, State::Done(message)) => body = body.child(message.clone()),
            (Mode::Restore, _) => body = body.child(self.render_restore(cx)),
            (Mode::Delete, _) => body = body.child(muted(t(M::DeleteSnapshotBody))),
            (Mode::Import, _) => body = body.child(self.render_import()),
            (Mode::Export, _) => {}
        }
        if let State::Failed(error) = &self.state {
            body = body.child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::RED_200)
                    .child(error.clone()),
            );
        }
        let done = matches!(self.state, State::Done(_));
        let primary: Option<(SharedString, ButtonVariant)> = match self.mode {
            _ if done => None,
            Mode::Take => Some((t(M::TakeSnapshot).into(), ButtonVariant::Primary)),
            Mode::Restore => Some((t(M::RestoreButton).into(), ButtonVariant::Primary)),
            Mode::Delete => Some((t(CommonMsg::Delete).into(), ButtonVariant::Danger)),
            Mode::Import => Some((t(M::CreateInstanceButton).into(), ButtonVariant::Primary)),
            Mode::Export => None,
        };
        let ready = !working
            && match self.mode {
                Mode::Take => true,
                Mode::Restore => self.preview.is_some(),
                Mode::Delete => self.snapshot.is_some(),
                Mode::Import => self.import.as_ref().is_some_and(|preview| {
                    self.import_free
                        .is_none_or(|free| preview.needed_bytes <= free)
                }),
                Mode::Export => false,
            };
        // While work runs, there is no way to close: no ×, Escape or click outside. Cancel
        // stops the work, and the dialog says how it ended.
        let busy = self.busy();
        // A restore's four choices fit on one line.
        let width = if self.mode == Mode::Restore {
            680.
        } else {
            560.
        };
        Dialog::new("snapshot-dialog", self.title(), self.focus.clone())
            .width(px(width))
            .when(!busy, |dialog| {
                dialog.on_close({
                    let entity = entity.clone();
                    move |_, cx| entity.update(cx, |_, cx| cx.emit(SnapshotDismissed))
                })
            })
            .child(body)
            .footer(
                Button::new(
                    "snapshot-cancel",
                    t(if self.mode == Mode::Export || done {
                        CommonMsg::Close
                    } else {
                        CommonMsg::Cancel
                    }),
                )
                .variant(ButtonVariant::Ghost)
                .disabled(self.stopping)
                .on_click(cx.listener(|this, _, _, cx| this.cancel_or_close(cx))),
            )
            .when_some(primary, |dialog, (label, variant)| {
                dialog.footer(
                    Button::new("snapshot-go", label)
                        .variant(variant)
                        .disabled(!ready)
                        .on_click(cx.listener(|this, _, _, cx| match this.mode {
                            Mode::Take => this.take(cx),
                            Mode::Restore => this.restore(cx),
                            Mode::Delete => this.delete(cx),
                            Mode::Import => this.import(cx),
                            Mode::Export => {}
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cant_be_closed_while_changing_files() {
        for label in [
            M::TakingSnapshot,
            M::RestoringSnapshot,
            M::ImportingSnapshot,
        ] {
            assert!(busy(&State::Working(label)), "{label:?}");
        }
        // Only reading, writing a file to share, or deleting: closing leaves nothing half
        // done.
        assert!(!busy(&State::Working(M::ReadingSnapshot)));
        assert!(!busy(&State::Working(M::PreparingFile)));
        assert!(!busy(&State::Working(M::DeletingSnapshot)));
        assert!(!busy(&State::Ready));
        assert!(!busy(&State::Failed("".into())));
    }

    #[test]
    fn a_restore_opens_on_a_choice_it_offers() {
        let world = launcher_core::images::WorldChange {
            name: "World".into(),
            level_name: None,
            exists_now: true,
            size: 1,
        };
        let preview = |worlds: Vec<_>, content: bool, other: bool| RestorePreview {
            worlds,
            can_restore_content: content,
            other_game: other,
            ..RestorePreview::default()
        };
        assert_eq!(
            default_choice(&preview(vec![world.clone()], true, false)),
            Choice::Worlds
        );
        assert_eq!(
            default_choice(&preview(vec![], true, false)),
            Choice::Everything
        );
        assert_eq!(
            default_choice(&preview(vec![], false, false)),
            Choice::AsNew
        );
        assert_eq!(
            default_choice(&preview(vec![world], true, true)),
            Choice::AsNew
        );
    }

    #[test]
    fn restore_changes_say_what_putting_the_mods_back_does() {
        let diff = ContentDiff {
            added: vec!["mods/old.jar".into()],
            removed: (0..7).map(|n| format!("mods/new-{n}.jar")).collect(),
            updated: Vec::new(),
            loader_version: None,
        };
        let lines = restore_changes(&diff);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("mods/old.jar"), "{lines:?}");
        assert!(
            lines[1].contains("mods/new-0.jar") && lines[1].contains('…'),
            "{lines:?}"
        );
        assert!(!lines[1].contains("mods/new-6.jar"), "{lines:?}");
    }

    #[test]
    fn only_mods_and_the_loader_count_as_other_mods() {
        let configs = ContentDiff {
            updated: vec!["config/foo.json".into()],
            ..ContentDiff::default()
        };
        assert!(!mods_differ(&configs));
        let mods = ContentDiff {
            removed: vec!["mods/new-mod-1.0.jar".into()],
            ..configs.clone()
        };
        assert!(mods_differ(&mods));
        let loader = ContentDiff {
            loader_version: Some((Some("0.16.9".into()), Some("0.16.10".into()))),
            ..configs
        };
        assert!(mods_differ(&loader));
    }

    #[test]
    fn worlds_of_one_name_are_told_apart_by_their_folders() {
        let record: SnapshotRecord = serde_json::from_value(serde_json::json!({
            "id": "20261108-143000-aaaa",
            "image": "a".repeat(64),
            "kind": "manual",
            "name": "Snapshot",
            "created": 0,
            "worlds": ["New World", "New World (1)", "Castle"],
            "world_details": [
                { "name": "New World", "level_name": "New World" },
                { "name": "New World (1)", "level_name": "New World" },
                { "name": "Castle", "level_name": "Castle Build" },
            ],
        }))
        .unwrap();
        let details = details(&record);
        assert!(details.contains("New World (New World (1))"), "{details}");
        assert!(details.contains("Castle Build"), "{details}");
        assert!(!details.contains("Castle Build (Castle)"), "{details}");
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_safe("a/b:c"), "a_b_c");
        assert_eq!(file_safe("..hidden"), "hidden");
        assert_eq!(file_safe("  "), "snapshot");
        assert_eq!(
            file_safe("Pre 1.21? *x* <y>|\"z\". "),
            "Pre 1.21_ _x_ _y___z_"
        );
    }
}
