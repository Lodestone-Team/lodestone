//! World backups in the Launcher: zips of one world each (`launcher_core::world_backups`),
//! listed in an instance's Settings tab beside its snapshots and in the Worlds tab, and the
//! dialog that makes one, restores one (in place after a safety backup, or as a new
//! world) or deletes one.
//!
//! The work runs on background threads; the dialog shows its progress.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use futures::StreamExt as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Render, SharedString, Task, Window, div, prelude::*,
};
use launcher_core::{
    game::download::{CancelToken, Progress},
    images::display_text,
    images::labels,
    world_backups::{RestoreTo, WorldBackup, WorldBackupKind, WorldBackups, WorldStep},
    worlds::{self, World},
};

use super::{Instances, new_instance::form_row};
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots, TextInput, segmented},
    i18n::{CommonMsg, InstanceSettingsMsg as M, t, tr},
    theme::{self, colors, px, text},
};

/// A world backup's name in the Launcher's language. A backup's details keep the English
/// names the Launcher gives ([`labels::WORLD_BACKUP`] when none is typed, and
/// [`labels::before_restoring`] for the safety backup), as snapshots do, since the names are
/// what the CLI and control API show too; the zips never leave this Launcher, so the
/// lists show those names translated, from how the backup was made. A name the user typed
/// is shown as it is.
pub fn shown_name(backup: &WorldBackup) -> String {
    fn named(name: &str) -> String {
        if name == labels::WORLD_BACKUP {
            t(M::WorldBackupDefaultName).to_string()
        } else {
            display_text(name).into_owned()
        }
    }
    match (&backup.kind, &backup.restored) {
        (WorldBackupKind::BeforeRestore, Some(restored))
            if backup.name == labels::before_restoring(restored) =>
        {
            tr!(M::WorldBackupBeforeRestoring, name = named(restored)).to_string()
        }
        _ => named(&backup.name),
    }
}

impl Instances {
    /// The instances' world backups.
    pub fn world_backups(&self) -> Arc<WorldBackups> {
        self.world_backups.clone()
    }

    /// Follows changes to every instance's world backups, reading a list it has again.
    pub(super) fn follow_world_backup_changes(&mut self, cx: &mut Context<Self>) {
        let mut changes = self.world_backups.watch();
        cx.spawn(async move |this, cx| {
            while let Some(id) = changes.next().await {
                let alive = this.update(cx, |this, cx| {
                    *this.world_backup_revisions.entry(id.clone()).or_default() += 1;
                    if this.world_backup_rows.contains_key(&id)
                        || this.world_backup_tasks.contains_key(&id)
                    {
                        this.load_world_backups(&id, cx);
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

    /// How many times the instance's world backups changed since the Launcher started.
    pub fn world_backup_revision(&self, id: &str) -> u64 {
        self.world_backup_revisions.get(id).copied().unwrap_or(0)
    }

    /// The instance's world backups, newest first, once read; reads them when not yet.
    pub fn world_backup_list(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Option<&[WorldBackup]> {
        if !self.world_backup_rows.contains_key(id) && !self.world_backup_tasks.contains_key(id) {
            self.load_world_backups(id, cx);
        }
        self.world_backup_rows.get(id).map(Vec::as_slice)
    }

    /// Of the instance's world backups read, the world folders no longer in its `saves/`.
    pub fn world_backup_gone(&self, id: &str) -> Option<&HashSet<String>> {
        self.world_backup_gone.get(id)
    }

    /// Reads the instance's world backups again, in the background, with which of their
    /// worlds are gone (so drawing never looks at the disk).
    pub fn load_world_backups(&mut self, id: &str, cx: &mut Context<Self>) {
        let backups = self.world_backups();
        let saves = worlds::saves_dir(&self.content_dirs(id));
        let key = id.to_string();
        let read = cx.background_spawn({
            let key = key.clone();
            async move {
                let list = backups.list(&key).unwrap_or_default();
                let gone: HashSet<String> = list
                    .iter()
                    .map(|b| b.folder.clone())
                    .filter(|folder| !saves.join(folder).is_dir())
                    .collect();
                (list, gone)
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let (list, gone) = read.await;
            this.update(cx, |this, cx| {
                this.world_backup_tasks.remove(&key);
                this.world_backup_gone.insert(key.clone(), gone);
                this.world_backup_rows.insert(key, list);
                cx.notify();
            })
            .ok();
        });
        self.world_backup_tasks.insert(id.to_string(), task);
    }

    /// Reads the instance's world backups again if they were read (a world was deleted,
    /// renamed or added, so which of their worlds are gone may have changed).
    pub fn world_backups_read_again(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.world_backup_rows.contains_key(id) {
            self.load_world_backups(id, cx);
        }
    }

    /// After a world backup was made, restored or deleted here: the lists read again, and
    /// the Worlds tab follows (a restore as a new world adds a world).
    pub fn world_backups_changed(&mut self, id: &str, cx: &mut Context<Self>) {
        *self
            .world_backup_revisions
            .entry(id.to_string())
            .or_default() += 1;
        self.load_world_backups(id, cx);
        cx.notify();
    }
}

/// What the world backup dialog opens for.
#[derive(Clone, Debug)]
pub enum WorldBackupRequest {
    /// Back up a world of the instance: `world` chosen first, else the most recently
    /// played.
    BackUp {
        instance: String,
        world: Option<String>,
    },
    /// Put a backup back, in place (asking first) or as a new world (at once).
    Restore {
        instance: String,
        backup: WorldBackup,
        to: RestoreTo,
    },
    /// Move a backup's zip to the Trash, asking first.
    Delete {
        instance: String,
        backup: WorldBackup,
    },
}

impl WorldBackupRequest {
    fn instance(&self) -> &str {
        match self {
            WorldBackupRequest::BackUp { instance, .. }
            | WorldBackupRequest::Restore { instance, .. }
            | WorldBackupRequest::Delete { instance, .. } => instance,
        }
    }
}

pub struct WorldBackupRequested(pub WorldBackupRequest);

/// The dialog is done and can close.
pub struct WorldBackupDismissed;

/// What the dialog is doing.
enum State {
    Ready,
    /// Making, restoring or deleting.
    Working,
    Done(SharedString),
    Failed(SharedString),
}

/// What a restore is doing now, for the label over its bar.
const STEP_NONE: u8 = 0;
const STEP_SAVING: u8 = 1;
const STEP_UNPACKING: u8 = 2;

pub struct WorldBackupDialog {
    instances: Entity<Instances>,
    request: WorldBackupRequest,
    /// Back up: the instance's worlds, once read.
    worlds: Option<Vec<World>>,
    chosen: Option<String>,
    name: Entity<TextInput>,
    /// Restore in place: whether the world is in the instance now (else nothing is saved
    /// first), as the instance's world backup list last read it.
    world_exists: bool,
    state: State,
    step: Arc<AtomicU8>,
    progress: Arc<Progress>,
    cancel: CancelToken,
    /// Cancel was pressed: the work stops at its next check (a swap, once started,
    /// finishes).
    stopping: bool,
    focus: FocusHandle,
    _work: Option<Task<()>>,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<WorldBackupDismissed> for WorldBackupDialog {}

impl Focusable for WorldBackupDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WorldBackupDialog {
    pub fn new(
        instances: Entity<Instances>,
        request: WorldBackupRequest,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::WorldBackupDefaultName)
                .with_automation_id("world-backup-name")
        });
        let dirs = instances.read(cx).content_dirs(request.instance());
        // From the list's own read (done in the background), never a look at the disk on
        // the window's thread; a list not read yet takes the world to be there.
        let world_exists = match &request {
            WorldBackupRequest::Restore {
                instance, backup, ..
            } => !instances
                .read(cx)
                .world_backup_gone(instance)
                .is_some_and(|gone| gone.contains(&backup.folder)),
            _ => false,
        };
        let mut this = Self {
            instances,
            chosen: match &request {
                WorldBackupRequest::BackUp { world, .. } => world.clone(),
                _ => None,
            },
            request,
            worlds: None,
            name,
            world_exists,
            state: State::Ready,
            step: Arc::new(AtomicU8::new(STEP_NONE)),
            progress: Arc::new(Progress::default()),
            cancel: CancelToken::new(),
            stopping: false,
            focus: cx.focus_handle(),
            _work: None,
            _ticker: None,
        };
        match &this.request {
            WorldBackupRequest::BackUp { .. } => {
                let read = cx.background_spawn(async move { worlds::list(&dirs) });
                this._work = Some(cx.spawn(async move |this, cx| {
                    let list = read.await;
                    this.update(cx, |this, cx| {
                        if this.chosen.is_none() {
                            this.chosen = list.first().map(|w| w.folder.clone());
                        }
                        this.worlds = Some(list);
                        cx.notify();
                    })
                    .ok();
                }));
            }
            WorldBackupRequest::Restore {
                to: RestoreTo::NewWorld,
                ..
            } => this.go(cx),
            _ => {}
        }
        this
    }

    /// Whether closing now would cut work short: making or restoring goes on until it ends
    /// or Cancel stops it; deleting is short. The shell queues a dialog that would replace
    /// this one meanwhile.
    pub(crate) fn busy(&self) -> bool {
        matches!(self.state, State::Working)
            && !matches!(self.request, WorldBackupRequest::Delete { .. })
    }

    /// Puts the dialog in the middle of work, or out of it, for the shell's tests.
    #[cfg(all(test, feature = "screenshot"))]
    pub(crate) fn test_set_working(&mut self, working: bool) {
        self.state = if working {
            State::Working
        } else {
            State::Ready
        };
    }

    /// Presses the dialog's main button, for the shell's tests.
    #[cfg(all(test, feature = "screenshot"))]
    pub(crate) fn test_go(&mut self, cx: &mut Context<Self>) {
        self.go(cx);
    }

    /// Why the dialog's work failed, if it did, for the shell's tests.
    #[cfg(all(test, feature = "screenshot"))]
    pub(crate) fn test_failed(&self) -> Option<SharedString> {
        match &self.state {
            State::Failed(why) => Some(why.clone()),
            _ => None,
        }
    }

    /// Ends the dialog on `message`, for the offscreen screenshots.
    #[cfg(feature = "screenshot")]
    pub fn preview_done(&mut self, message: impl Into<SharedString>) {
        self.state = State::Done(message.into());
    }

    fn game_running(&self, cx: &App) -> bool {
        self.instances.read(cx).is_active(self.request.instance())
    }

    /// The world chosen to back up, for the shell's tests.
    #[cfg(all(test, feature = "screenshot"))]
    pub(crate) fn test_chosen(&self) -> (Option<usize>, Option<String>) {
        (self.worlds.as_ref().map(Vec::len), self.chosen.clone())
    }

    /// Does what the dialog is for.
    fn go(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let id = self.request.instance().to_string();
        let deleting = matches!(self.request, WorldBackupRequest::Delete { .. });
        if self.game_running(cx) && !deleting {
            self.state = State::Failed(t(M::SnapshotGameRunning).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        // Another job writing the instance's files (an install, a modpack update, the
        // Worlds tab's or another dialog's job): a restore's swap would lose what it
        // writes, a backup could zip a file mid-write, and any of them, a delete too,
        // would wait for the instance's lock meanwhile, for as long as that job runs, with
        // Cancel doing nothing. Refused instead, as Take a snapshot is.
        if instances.files_or_pack_busy(&id) {
            self.state = State::Failed(t(crate::i18n::InstancesMsg::FilesBusy).into());
            return cx.notify();
        }
        let backups = instances.world_backups();
        let busy = instances.mark_files_busy(&id);
        let progress = Arc::new(Progress::default());
        self.progress = progress.clone();
        self.cancel = CancelToken::new();
        self.stopping = false;
        self.step.store(STEP_NONE, Ordering::Relaxed);
        let (cancel, step) = (self.cancel.clone(), self.step.clone());
        let work: Box<dyn FnOnce() -> Result<SharedString, String> + Send> = match &self.request {
            WorldBackupRequest::BackUp { .. } => {
                let Some(folder) = self.chosen.clone() else {
                    return;
                };
                let name = self.name.read(cx).text().trim().to_string();
                let shown = world_label(self.worlds.as_deref().unwrap_or_default(), &folder);
                launcher_core::trace::emit(
                    "world_backup.create",
                    || serde_json::json!({ "instance": id }),
                );
                let id = id.clone();
                Box::new(move || {
                    let _busy = busy;
                    backups
                        .create(&id, &folder, Some(&name), &progress, &|| {
                            cancel.is_cancelled()
                        })
                        .map(|_| tr!(M::WorldBackedUp, name = shown).into())
                        .map_err(|e| e.to_string())
                })
            }
            WorldBackupRequest::Restore { backup, to, .. } => {
                let (backup, to, id) = (backup.clone(), *to, id.clone());
                launcher_core::trace::emit(
                    "world_backup.restore",
                    || serde_json::json!({ "instance": id, "new_world": to == RestoreTo::NewWorld }),
                );
                Box::new(move || {
                    let _busy = busy;
                    let set_step = |s: WorldStep| {
                        step.store(
                            match s {
                                WorldStep::SavingCurrent => STEP_SAVING,
                                WorldStep::Unpacking => STEP_UNPACKING,
                            },
                            Ordering::Relaxed,
                        )
                    };
                    let restored = backups
                        .restore(&id, &backup.id, to, &set_step, &progress, &|| {
                            cancel.is_cancelled()
                        })
                        .map_err(|e| e.to_string())?;
                    Ok(match to {
                        RestoreTo::InPlace => {
                            let mut message =
                                tr!(M::WorldRestored, world = display_text(backup.world_name()));
                            // Where the newer progress went, as the Worlds tab's snapshot
                            // restore names its safety snapshot.
                            if let Some(safety) = &restored.safety {
                                message.push(' ');
                                message.push_str(&tr!(
                                    M::WorldRestoredSafety,
                                    name = shown_name(safety)
                                ));
                            }
                            // Files written into the world meanwhile, moved aside: said here,
                            // as a snapshot's restore says it.
                            if let Some(kept) = &restored.kept_aside {
                                message.push(' ');
                                message
                                    .push_str(&tr!(M::RestoreKeptAside, folder = kept.display()));
                            }
                            message.into()
                        }
                        RestoreTo::NewWorld => {
                            tr!(M::WorldRestoredAsNew, folder = restored.folder).into()
                        }
                    })
                })
            }
            WorldBackupRequest::Delete { backup, .. } => {
                let (backup, id) = (backup.clone(), id.clone());
                Box::new(move || {
                    let _busy = busy;
                    backups
                        .delete(&id, &backup.id)
                        .map(|()| SharedString::default())
                        .map_err(|e| e.to_string())
                })
            }
        };
        self.state = State::Working;
        let job = cx.background_spawn(async move { work() });
        let instances = self.instances.clone();
        self._work = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            if result.is_ok() {
                instances.update(cx, |instances, cx| instances.world_backups_changed(&id, cx));
            }
            this.update(cx, |this, cx| {
                this.stopping = false;
                match result {
                    Ok(_) if deleting => cx.emit(WorldBackupDismissed),
                    Ok(message) => this.state = State::Done(message),
                    Err(err) if this.cancel.is_cancelled() => {
                        log::info!("World backup work stopped: {err}");
                        this.state = State::Failed(t(crate::i18n::WorldsMsg::Cancelled).into())
                    }
                    Err(err) => {
                        log::warn!(
                            "world backup work for instance {} failed: {err}",
                            this.request.instance()
                        );
                        this.state = State::Failed(err.into());
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        // Redraws the bar while it runs.
        self._ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let working = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        matches!(this.state, State::Working)
                    })
                    .unwrap_or(false);
                if !working {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Cancel: closes the dialog, or, while work runs, stops it.
    fn cancel_or_close(&mut self, cx: &mut Context<Self>) {
        if !self.busy() {
            return cx.emit(WorldBackupDismissed);
        }
        self.stopping = true;
        self.cancel.cancel();
        cx.notify();
    }

    fn title(&self) -> SharedString {
        match &self.request {
            WorldBackupRequest::BackUp { .. } => t(M::BackUpWorldTitle).into(),
            WorldBackupRequest::Restore {
                backup,
                to: RestoreTo::InPlace,
                ..
            } => tr!(
                M::RestoreWorldTitle,
                world = display_text(backup.world_name())
            )
            .into(),
            WorldBackupRequest::Restore { .. } => t(M::RestoreAsNewWorld).into(),
            WorldBackupRequest::Delete { .. } => t(M::DeleteWorldBackupTitle).into(),
        }
    }

    fn render_progress(&self) -> AnyElement {
        let label = if self.stopping {
            t(crate::i18n::InstancesMsg::CancellingNote).to_string()
        } else {
            progress_label(&self.request, self.step.load(Ordering::Relaxed), || {
                self.chosen.as_deref().map_or(String::new(), |folder| {
                    world_label(self.worlds.as_deref().unwrap_or_default(), folder)
                })
            })
        };
        let now = self.progress.snapshot();
        let fraction = (now.bytes_total > 0)
            .then(|| (now.bytes_done as f32 / now.bytes_total as f32).clamp(0.0, 1.0));
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
            .into_any_element()
    }

    fn render_back_up(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(list) = &self.worlds else {
            return div().child(LoadingDots).into_any_element();
        };
        if list.is_empty() {
            return muted(t(M::NoWorldsToBackUp)).into_any_element();
        }
        let mut body = div().flex().flex_col().gap_3();
        if list.len() > 1 {
            let choices: Vec<(String, SharedString)> = list
                .iter()
                .map(|w| {
                    (
                        w.folder.clone(),
                        SharedString::from(world_label(list, &w.folder)),
                    )
                })
                .collect();
            let entity = cx.entity();
            body = body.child(form_row(
                t(M::WorldBackupWorld),
                segmented(
                    "world-backup-world",
                    choices,
                    self.chosen.as_ref(),
                    false,
                    move |folder, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.chosen = Some(folder);
                            cx.notify();
                        })
                    },
                ),
            ));
        }
        body.child(form_row(t(M::SnapshotName), self.name.clone()))
            .into_any_element()
    }
}

/// How the world in `folder` is named in the dialog: its name, with its folder when
/// another world of `list` has the same name (the game names every new world "New World",
/// in folders "New World", "New World (1)"…), so a choice, its progress and its result
/// always say which world. The folder alone when it isn't in `list`.
fn world_label(list: &[World], folder: &str) -> String {
    let Some(world) = list.iter().find(|w| w.folder == folder) else {
        return folder.to_string();
    };
    let name = world.name();
    if list
        .iter()
        .any(|other| other.folder != folder && other.name() == name)
    {
        format!("{name} ({folder})")
    } else {
        name
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

impl Render for WorldBackupDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .text_size(theme::text_size(text::MEDIUM));
        let mut primary: Option<(SharedString, ButtonVariant, bool)> = None;
        match (&self.state, &self.request) {
            (State::Working, _) => body = body.child(self.render_progress()),
            (State::Done(message), _) => body = body.child(message.clone()),
            (State::Failed(message), _) => {
                body = body.child(
                    div()
                        .whitespace_normal()
                        .text_color(colors::RED_300)
                        .child(message.clone()),
                )
            }
            (State::Ready, WorldBackupRequest::BackUp { .. }) => {
                body = body.child(self.render_back_up(cx));
                primary = Some((
                    t(M::BackUpButton).into(),
                    ButtonVariant::Primary,
                    self.chosen.is_some(),
                ));
            }
            (State::Ready, WorldBackupRequest::Restore { backup, to, .. }) => {
                let when = super::snapshots::when(backup.created).to_lowercase();
                let world = display_text(backup.world_name()).into_owned();
                if *to == RestoreTo::InPlace {
                    let name = shown_name(backup);
                    body = body.child(div().whitespace_normal().child(if self.world_exists {
                        tr!(M::RestoreWorldBody, world = world, name = name, when = when)
                    } else {
                        tr!(
                            M::RestoreWorldGoneBody,
                            world = world,
                            name = name,
                            when = when
                        )
                    }));
                    primary = Some((
                        t(if self.world_exists {
                            M::SaveAndRestore
                        } else {
                            M::RestoreButton
                        })
                        .into(),
                        ButtonVariant::Danger,
                        true,
                    ));
                }
            }
            (State::Ready, WorldBackupRequest::Delete { backup, .. }) => {
                body = body.child(div().whitespace_normal().child(tr!(
                    M::DeleteWorldBackupBody,
                    name = shown_name(backup),
                    trash = t(CommonMsg::TheTrash)
                )));
                primary = Some((
                    t(CommonMsg::MoveToTrash).into(),
                    ButtonVariant::Danger,
                    true,
                ));
            }
        }
        let finished = matches!(self.state, State::Done(_) | State::Failed(_));
        // While work runs, there is no way to close: no ×, Escape or click outside. Cancel
        // stops the work, and the dialog says how it ended.
        let busy = self.busy();
        Dialog::new("world-backup-dialog", self.title(), self.focus.clone())
            .width(px(560.))
            .when(!busy, |dialog| {
                dialog.on_close({
                    let entity = entity.clone();
                    move |_, cx| entity.update(cx, |_, cx| cx.emit(WorldBackupDismissed))
                })
            })
            .child(body)
            .footer(
                Button::new(
                    "world-backup-cancel",
                    t(if finished {
                        CommonMsg::Close
                    } else {
                        CommonMsg::Cancel
                    }),
                )
                .variant(ButtonVariant::Ghost)
                .disabled(self.stopping)
                .on_click(cx.listener(|this, _, _, cx| this.cancel_or_close(cx))),
            )
            .when_some(primary, |dialog, (label, variant, enabled)| {
                dialog.footer(
                    Button::new("world-backup-go", label)
                        .variant(variant)
                        .disabled(!enabled)
                        .on_click(cx.listener(|this, _, _, cx| this.go(cx))),
                )
            })
    }
}

/// What the dialog says while `request` works, at `step` of a restore; `chosen` names the
/// world being backed up.
fn progress_label(
    request: &WorldBackupRequest,
    step: u8,
    chosen: impl FnOnce() -> String,
) -> String {
    match (request, step) {
        (WorldBackupRequest::BackUp { .. }, _) => tr!(M::BackingUpWorld, name = chosen()),
        (WorldBackupRequest::Delete { backup, .. }, _) => tr!(
            crate::i18n::WorldsMsg::MovingToTrash,
            name = shown_name(backup),
            trash = t(CommonMsg::TheTrash)
        ),
        (_, STEP_SAVING) => t(M::SavingWorld).to_string(),
        _ => t(M::UnpackingWorldBackup).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup(name: &str, kind: WorldBackupKind, restored: Option<&str>) -> WorldBackup {
        WorldBackup {
            version: 1,
            id: "20261115-120000-abcd".into(),
            instance_created: 1,
            instance_name: "Pack".into(),
            folder: "World".into(),
            name: name.into(),
            kind,
            restored: restored.map(str::to_string),
            created: 0,
            level_name: None,
            game_version: None,
            data_version: None,
            bytes: 0,
            world_bytes: 0,
            files: 0,
            left_out: Vec::new(),
            left_out_count: 0,
        }
    }

    /// Each request says what it does while it works: a delete never reads like a restore.
    #[test]
    fn progress_says_what_runs() {
        let manual = backup("Before the dragon fight", WorldBackupKind::Manual, None);
        let delete = WorldBackupRequest::Delete {
            instance: "pack".into(),
            backup: manual.clone(),
        };
        let deleting = progress_label(&delete, STEP_NONE, String::new);
        assert!(deleting.contains("Before the dragon fight"), "{deleting}");
        assert_ne!(deleting, t(M::UnpackingWorldBackup));
        assert_ne!(deleting, t(M::SavingWorld));
        let restore = WorldBackupRequest::Restore {
            instance: "pack".into(),
            backup: manual,
            to: RestoreTo::InPlace,
        };
        assert_eq!(
            progress_label(&restore, STEP_SAVING, String::new),
            t(M::SavingWorld)
        );
        assert_eq!(
            progress_label(&restore, STEP_UNPACKING, String::new),
            t(M::UnpackingWorldBackup)
        );
        let back_up = WorldBackupRequest::BackUp {
            instance: "pack".into(),
            world: None,
        };
        assert!(progress_label(&back_up, STEP_NONE, || "Island".into()).contains("Island"));
    }

    /// The names the Launcher gave are shown in the window's language, from how the backup
    /// was made; a name the user typed (or a safety backup renamed) as it is.
    #[test]
    fn names_the_launcher_gave_are_shown_translated() {
        let default = t(M::WorldBackupDefaultName).to_string();
        let manual = backup(labels::WORLD_BACKUP, WorldBackupKind::Manual, None);
        assert_eq!(shown_name(&manual), default);
        let typed = backup("Before the dragon fight", WorldBackupKind::Manual, None);
        assert_eq!(shown_name(&typed), "Before the dragon fight");

        let safety = |restored: &str| {
            backup(
                &labels::before_restoring(restored),
                WorldBackupKind::BeforeRestore,
                Some(restored),
            )
        };
        assert_eq!(
            shown_name(&safety(labels::WORLD_BACKUP)),
            tr!(M::WorldBackupBeforeRestoring, name = default).to_string()
        );
        assert_eq!(
            shown_name(&safety("Towers")),
            tr!(M::WorldBackupBeforeRestoring, name = "Towers").to_string()
        );
        // A name that isn't the label any more is the user's.
        let renamed = backup("Kept", WorldBackupKind::BeforeRestore, Some("Towers"));
        assert_eq!(shown_name(&renamed), "Kept");
        // Text that would reorder or hide itself is shown plainly.
        let odd = backup("a\u{202e}b", WorldBackupKind::Manual, None);
        assert_eq!(shown_name(&odd), display_text("a\u{202e}b"));
    }
}
