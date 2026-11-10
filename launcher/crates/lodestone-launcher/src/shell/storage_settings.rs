//! Settings > Storage: what the data folder takes, by instance, game files, Java
//! runtimes, the cache and logs, with a way to remove what can go, and where the
//! data folder is, with a way to move it (to another drive, say).
//!
//! The count runs on a background thread while the page is shown and stops when it
//! is left. Removing asks first (the shell's dialog), runs in the background, and
//! counts again. The rules for what is safe to remove are `launcher_core::disk`'s.
//!
//! Moving the data folder asks first too (`data_folder`'s dialogs), then runs
//! `launcher_core::data_move` in the background while the dialog shows its progress,
//! and restarts the launcher into the new folder.

use crate::components::a11y::Accessible as _;
use crate::theme::px;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, Render, SharedString, Task, Window,
    div, prelude::*, relative,
};
use launcher_control::paths::{DataDirLocation, DataDirSource};
use launcher_core::{
    data_move::{self, MoveError, MoveProgress, MoveRequest},
    disk::{self, Cancel, Clean, Report},
};
use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, LoadingDots,
        settings::{SettingRow, Toast, card, muted_link, section, tag, value},
    },
    file_dialog,
    i18n::{ShellMsg as M, format, t, tr},
    instances::Instances,
    theme::{self, Color, colors, radius, text, weights},
};

/// The count, as it stands.
#[derive(Clone)]
enum Count {
    NotStarted,
    Counting,
    Ready(Box<Report>),
    Failed(SharedString),
}

/// What the page asks the shell for.
pub enum StorageEvent {
    /// "Delete…" on an instance: the shell's delete dialog.
    DeleteInstance(String),
    /// A removal to confirm, with what it frees.
    Confirm { what: Clean, bytes: u64 },
    /// A move of the data folder to `to` to confirm; `back` when `to` is the
    /// default folder.
    ConfirmMove { to: PathBuf, back: bool },
    /// The data folder moved: the launcher restarts into it.
    Moved,
    /// The move was cancelled, and the data is where it was.
    MoveCancelled,
}

/// A move of the data folder that is running.
pub struct Moving {
    pub progress: Arc<MoveProgress>,
    cancel: Cancel,
    _task: Task<()>,
    /// Redraws the progress while it runs.
    _tick: Task<()>,
}

pub struct StorageSettings {
    instances: Entity<Instances>,
    count: Count,
    /// The running count or removal, and how to stop it.
    work: Option<(Task<()>, Cancel)>,
    /// A removal is running.
    cleaning: bool,
    toast: Toast,
    /// Where the data folder is and why, as the launcher found it at start.
    location: Option<DataDirLocation>,
    /// The folder dialog for "Move…", while it is open.
    picking: Option<Task<()>>,
    moving: Option<Moving>,
    /// Why the last move didn't happen.
    move_failed: Option<SharedString>,
}

impl file_dialog::ShowsError for StorageSettings {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast
            .show(message, true, |this: &mut Self| &mut this.toast, cx);
    }
}

impl EventEmitter<StorageEvent> for StorageSettings {}

/// The colors of the categories, in the bar and its legend.
const INSTANCES: Color = colors::BLUE_200;
const GAME_FILES: Color = colors::GREEN_300;
const RUNTIMES: Color = colors::VIOLET;
const CACHE: Color = colors::YELLOW;
const OTHER: Color = colors::GRAY_500;

impl StorageSettings {
    pub fn new(instances: Entity<Instances>) -> Self {
        Self {
            instances,
            count: Count::NotStarted,
            work: None,
            cleaning: false,
            toast: Toast::default(),
            location: launcher_control::paths::locate(),
            picking: None,
            moving: None,
            move_failed: None,
        }
    }

    pub fn toast(&self) -> &Toast {
        &self.toast
    }

    /// Counts again, stopping a count that is running.
    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        self.stop(cx);
        let (data, list) = {
            let instances = self.instances.read(cx);
            (instances.data_dir().clone(), instances.list().to_vec())
        };
        let cancel = Cancel::new();
        let stop = cancel.clone();
        let work = cx.background_spawn(async move { disk::scan(&data, &list, &stop) });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.work = None;
                match result {
                    Ok(report) => this.count = Count::Ready(Box::new(report)),
                    Err(disk::DiskError::Cancelled) => {}
                    Err(err) => this.count = Count::Failed(err.to_string().into()),
                }
                cx.notify();
            })
            .ok();
        });
        if !matches!(self.count, Count::Ready(_)) {
            self.count = Count::Counting;
        }
        self.work = Some((task, cancel));
        cx.notify();
    }

    /// Counts if it hasn't yet (the page was opened).
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if matches!(self.count, Count::NotStarted) {
            self.rescan(cx);
        }
    }

    /// Stops a count (the page was left). A removal goes on.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.cleaning {
            return;
        }
        if let Some((_, cancel)) = self.work.take() {
            cancel.cancel();
            if matches!(self.count, Count::Counting) {
                self.count = Count::NotStarted;
            }
            cx.notify();
        }
    }

    /// Whether a game is starting or running.
    fn game_running(&self, cx: &gpui::App) -> bool {
        let instances = self.instances.read(cx);
        instances.list().iter().any(|i| instances.is_active(&i.id))
    }

    /// Removes `what`, after the shell's dialog, then counts again.
    pub fn clean(&mut self, what: Clean, cx: &mut Context<Self>) {
        if self.cleaning {
            return;
        }
        let touches_game_files =
            !matches!(what, Clean::OldLogs | Clean::AllLogs | Clean::Leftovers);
        if touches_game_files && self.game_running(cx) {
            self.toast.show(
                t(M::StorageStopGame),
                true,
                |this: &mut Self| &mut this.toast,
                cx,
            );
            return;
        }
        if let Some((_, cancel)) = self.work.take() {
            cancel.cancel();
        }
        let (data, list, running) = {
            let instances = self.instances.read(cx);
            let running: Vec<String> = instances
                .list()
                .iter()
                .filter(|i| instances.is_active(&i.id))
                .map(|i| i.id.clone())
                .collect();
            (
                instances.data_dir().clone(),
                instances.list().to_vec(),
                running,
            )
        };
        let cancel = Cancel::new();
        let stop = cancel.clone();
        let work =
            cx.background_spawn(async move { disk::clean(&data, &list, &running, what, &stop) });
        self.cleaning = true;
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.cleaning = false;
                this.work = None;
                launcher_core::trace::emit("storage.clean", || match &result {
                    Ok(cleaned) => serde_json::json!({
                        "what": what.name(),
                        "freed_bytes": cleaned.bytes,
                        "files": cleaned.files,
                    }),
                    Err(err) => {
                        serde_json::json!({ "what": what.name(), "error": err.to_string() })
                    }
                });
                let (message, error) = match result {
                    Ok(cleaned) => (
                        tr!(M::StorageFreed, size = format::bytes(cleaned.bytes)),
                        false,
                    ),
                    Err(err) => (tr!(M::StorageCouldntRemove, error = err), true),
                };
                this.toast
                    .show(message, error, |this: &mut Self| &mut this.toast, cx);
                this.rescan(cx);
            })
            .ok();
        });
        self.work = Some((task, cancel));
        cx.notify();
    }

    /// The data folder.
    pub fn data_root(&self, cx: &gpui::App) -> PathBuf {
        self.instances.read(cx).data_dir().root().to_path_buf()
    }

    /// The default folder and why the data is where it is, when the data folder can
    /// be moved from here: not when it isn't the folder the launcher finds (the
    /// offscreen scenes).
    fn movable(&self, cx: &gpui::App) -> Option<(PathBuf, DataDirSource)> {
        let location = self.location.as_ref()?;
        (location.dir == self.data_root(cx)).then(|| (location.default.clone(), location.source))
    }

    /// Whether a move can start now; shows why not when it can't.
    fn can_start_move(&mut self, cx: &mut Context<Self>) -> bool {
        if self.moving.is_some() || self.cleaning {
            return false;
        }
        if self.game_running(cx) {
            self.toast.show(
                t(M::StorageMoveStopGame),
                true,
                |this: &mut Self| &mut this.toast,
                cx,
            );
            return false;
        }
        true
    }

    /// "Move…": asks for a folder, then for confirmation (the shell's dialog).
    /// Choosing a folder that has other things in it, such as a games folder, moves
    /// the data into a "Lodestone Launcher" folder in it.
    pub fn choose_move_target(&mut self, cx: &mut Context<Self>) {
        let Some((default, _)) = self.movable(cx) else {
            return;
        };
        if !self.can_start_move(cx) {
            return;
        }
        let from = self.data_root(cx);
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(M::StorageMoveHere).into()),
        });
        self.picking = Some(cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(chosen) = paths.into_iter().next() else {
                return;
            };
            let Ok(check) = this.update(cx, |_, cx| {
                cx.background_spawn(async move { data_move::target_for(&chosen, &from, &default) })
            }) else {
                return;
            };
            let result = check.await;
            this.update(cx, |this, cx| {
                this.picking = None;
                match result {
                    Ok(to) => {
                        let back = this
                            .location
                            .as_ref()
                            .is_some_and(|location| location.default == to);
                        cx.emit(StorageEvent::ConfirmMove { to, back });
                    }
                    Err(err) => this.toast.show(
                        tr!(M::StorageMoveCantUse, error = err),
                        true,
                        |this: &mut Self| &mut this.toast,
                        cx,
                    ),
                }
            })
            .ok();
        }));
    }

    /// "Move back…": the default folder, after confirmation.
    pub fn choose_move_back(&mut self, cx: &mut Context<Self>) {
        let Some((default, DataDirSource::Moved)) = self.movable(cx) else {
            return;
        };
        if !self.can_start_move(cx) {
            return;
        }
        let request = MoveRequest {
            from: self.data_root(cx),
            to: default.clone(),
            default: default.clone(),
        };
        match data_move::check(&request) {
            Ok(()) => cx.emit(StorageEvent::ConfirmMove {
                to: default,
                back: true,
            }),
            Err(err) => self.toast.show(
                tr!(M::StorageMoveCantUse, error = err),
                true,
                |this: &mut Self| &mut this.toast,
                cx,
            ),
        }
    }

    /// Moves the data folder to `to` in the background (confirmed in the shell's
    /// dialog, which shows the progress). Ends with [`StorageEvent::Moved`],
    /// [`StorageEvent::MoveCancelled`] or [`Self::move_failed`].
    pub fn start_move(&mut self, to: PathBuf, cx: &mut Context<Self>) {
        let Some((default, _)) = self.movable(cx) else {
            return;
        };
        if self.moving.is_some() {
            return;
        }
        if self.game_running(cx) {
            self.move_failed = Some(t(M::StorageMoveStopGame).into());
            cx.notify();
            return;
        }
        // A count would read files as they move.
        if let Some((_, cancel)) = self.work.take() {
            cancel.cancel();
        }
        let request = MoveRequest {
            from: self.data_root(cx),
            to,
            default,
        };
        let progress = Arc::new(MoveProgress::default());
        let cancel = Cancel::new();
        let work = {
            let (progress, cancel) = (progress.clone(), cancel.clone());
            let request = request.clone();
            cx.background_spawn(async move { data_move::run(&request, &progress, &cancel) })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.moving = None;
                launcher_core::trace::emit("storage.move", || {
                    serde_json::json!({
                        "to": request.to.display().to_string(),
                        "error": result.as_ref().err().map(|e| e.to_string()),
                    })
                });
                match result {
                    Ok(()) => cx.emit(StorageEvent::Moved),
                    Err(MoveError::Cancelled) => cx.emit(StorageEvent::MoveCancelled),
                    Err(err) => this.move_failed = Some(sentence(&err.to_string()).into()),
                }
                cx.notify();
            })
            .ok();
        });
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        self.move_failed = None;
        self.moving = Some(Moving {
            progress,
            cancel,
            _task: task,
            _tick: tick,
        });
        cx.notify();
    }

    /// Stops the running move; the data stays where it was.
    pub fn cancel_move(&mut self) {
        if let Some(moving) = &self.moving {
            moving.cancel.cancel();
        }
    }

    pub fn moving(&self) -> Option<&Moving> {
        self.moving.as_ref()
    }

    pub fn move_failed(&self) -> Option<SharedString> {
        self.move_failed.clone()
    }

    pub fn clear_move_failed(&mut self, cx: &mut Context<Self>) {
        self.move_failed = None;
        cx.notify();
    }

    /// The total the last count found, if there is one.
    pub fn counted_total(&self) -> Option<u64> {
        match &self.count {
            Count::Ready(report) => Some(report.total()),
            _ => None,
        }
    }

    /// Shows a message under the page, such as why the launcher didn't restart.
    pub fn show_toast(&mut self, message: SharedString, error: bool, cx: &mut Context<Self>) {
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
    }

    /// The "Data folder" section: where it is, and moving it.
    fn data_folder_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let root = self.data_root(cx);
        let movable = self.movable(cx);
        let busy = self.cleaning || self.moving.is_some() || self.picking.is_some();
        let mut row = SettingRow::new(root.display().to_string())
            .key("storage-data-folder")
            .hint(t(M::StorageDataFolderHint))
            .child(
                muted_link("storage-open-data", t(M::StorageOpenFolder)).on_press(cx.listener(
                    |this, _, _, cx| {
                        let root = this.data_root(cx);
                        cx.open_with_system(&root);
                    },
                )),
            );
        if let Some((_, source)) = movable {
            if source == DataDirSource::Moved {
                row = row.child(
                    Button::new("storage-move-back", t(M::StorageMoveBackEllipsis))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.choose_move_back(cx))),
                );
            }
            row = row.child(
                Button::new("storage-move", t(M::StorageMoveEllipsis))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_move_target(cx))),
            );
        }
        section(t(M::StorageDataFolder), None)
            .child(card([row.into_any_element()]))
            .into_any_element()
    }

    /// Shows the data folder as moved (or not), for the offscreen screenshots.
    pub fn preview_location(&mut self, moved: bool, cx: &mut Context<Self>) {
        let root = self.data_root(cx);
        self.location = Some(DataDirLocation {
            default: if moved {
                PathBuf::from("C:\\Users\\Alex\\AppData\\Roaming\\Lodestone Launcher")
            } else {
                root.clone()
            },
            dir: root,
            source: if moved {
                DataDirSource::Moved
            } else {
                DataDirSource::Default
            },
        });
        cx.notify();
    }

    /// Shows a count, for the offscreen screenshots.
    pub fn preview(&mut self, report: Report, cx: &mut Context<Self>) {
        if let Some((_, cancel)) = self.work.take() {
            cancel.cancel();
        }
        self.count = Count::Ready(Box::new(report));
        cx.notify();
    }
}

/// `error` as a sentence of its own: a capital first letter and a full stop.
fn sentence(error: &str) -> String {
    let mut chars = error.chars();
    let mut text: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    if !text.ends_with(['.', '!', '?']) {
        text.push('.');
    }
    text
}

/// A size, as a row's value.
fn size(bytes: u64) -> impl IntoElement {
    value(format::bytes(bytes), false)
}

/// A small button that asks before removing.
fn remove_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<StorageSettings>,
    what: Clean,
    bytes: u64,
) -> impl IntoElement {
    Button::new(id, label)
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Slim)
        .disabled(!enabled)
        .on_click(cx.listener(move |_, _, _, cx| {
            cx.emit(StorageEvent::Confirm { what, bytes });
        }))
}

/// "Counting…", in a card that waits for the count.
fn counting() -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .py_3()
        .text_color(crate::theme::ink::muted())
        .child(LoadingDots)
        .child(t(M::StorageCounting))
        .into_any_element()
}

impl Render for StorageSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.game_running(cx);
        let busy = self.cleaning;
        let report = match &self.count {
            Count::Ready(report) => Some((**report).clone()),
            _ => None,
        };
        let failed = match &self.count {
            Count::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let page = div()
            .flex()
            .flex_col()
            .gap_6()
            .w_full()
            .child(self.data_folder_section(cx));
        let Some(report) = report else {
            return page
                .child(card([match failed {
                    Some(error) => div()
                        .px_4()
                        .py_3()
                        .text_color(colors::RED_200)
                        .child(tr!(M::StorageCouldntCount, error = error))
                        .into_any_element(),
                    None => counting(),
                }]))
                .into_any_element();
        };

        // The total, and a bar by category.
        let parts: Vec<(Color, &'static str, u64)> = vec![
            (
                INSTANCES,
                t(M::StorageInstances),
                report.instances_total() + report.snapshots.bytes,
            ),
            (
                GAME_FILES,
                t(M::StorageGameFiles),
                report.game_files.total(),
            ),
            (RUNTIMES, t(M::StorageRuntimes), report.runtimes_total()),
            (
                CACHE,
                t(M::StorageCache),
                report.cache + report.leftovers.bytes,
            ),
            (OTHER, t(M::StorageOther), report.other),
        ];
        let total = report.total().max(1);
        let bar =
            div()
                .flex()
                .flex_row()
                .w_full()
                .h(px(10.))
                .rounded_full()
                .overflow_hidden()
                .bg(colors::GRAY_750)
                .children(parts.iter().filter(|(_, _, bytes)| *bytes > 0).map(
                    |(color, _, bytes)| {
                        div()
                            .h_full()
                            .w(relative(*bytes as f32 / total as f32))
                            .bg(*color)
                    },
                ));
        let legend = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_x_4()
            .gap_y_1()
            .text_size(theme::text_size(text::SMALL))
            .text_color(crate::theme::ink::secondary())
            .children(parts.iter().map(|(color, label, bytes)| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(div().size(px(8.)).rounded_full().bg(*color))
                    .child(format!("{label} {}", format::bytes(*bytes)))
            }));
        let summary = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .text_size(theme::text_size(text::H2))
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::theme::ink::text())
                            .child(format::bytes(report.total())),
                    )
                    .when(self.work.is_some() && !busy, |this| this.child(LoadingDots)),
            )
            .child(bar)
            .child(legend)
            .when(running, |this| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::YELLOW)
                        .child(t(M::StorageStopGame)),
                )
            });

        // Instances.
        let mut instance_rows: Vec<AnyElement> = if report.instances.is_empty() {
            vec![SettingRow::new(t(M::StorageNoInstances)).into_any_element()]
        } else {
            report
                .instances
                .iter()
                .enumerate()
                .map(|(index, usage)| {
                    let id = usage.id.clone();
                    let folder_id = usage.id.clone();
                    let active = self.instances.read(cx).is_active(&usage.id);
                    let other = usage.logs + usage.screenshots + usage.other;
                    let detail = [
                        (usage.worlds, M::StorageWorlds),
                        (usage.content, M::StorageContent),
                        (other, M::StorageOtherFiles),
                    ]
                    .into_iter()
                    .filter(|(bytes, _)| *bytes > 0)
                    .map(|(bytes, message)| tr!(message, size = format::bytes(bytes)))
                    .collect::<Vec<_>>()
                    .join(" · ");
                    SettingRow::new(usage.name.clone())
                        .key(("storage-instance", index))
                        .hint(detail)
                        .child(size(usage.total()))
                        .child(
                            muted_link(("storage-open-instance", index), t(M::StorageOpen))
                                .on_press(cx.listener(move |this, _, _, cx| {
                                    let dir = this
                                        .instances
                                        .read(cx)
                                        .data_dir()
                                        .instances_dir()
                                        .join(&folder_id);
                                    cx.open_with_system(&dir);
                                })),
                        )
                        .child(
                            Button::new(("storage-delete-instance", index), t(M::StorageDelete))
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .disabled(active || busy)
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(StorageEvent::DeleteInstance(id.clone()))
                                })),
                        )
                        .into_any_element()
                })
                .collect()
        };

        // Game files.
        let unused = report.game_files.unused.total();
        let game_rows = vec![
            SettingRow::new(t(M::StorageVersions))
                .child(size(report.game_files.versions))
                .into_any_element(),
            SettingRow::new(t(M::StorageLibraries))
                .child(size(report.game_files.libraries))
                .into_any_element(),
            SettingRow::new(t(M::StorageAssets))
                .child(size(report.game_files.assets))
                .into_any_element(),
            SettingRow::new(t(M::StorageUnused))
                .hint_opt(
                    (!report.game_files.unused.versions.is_empty())
                        .then(|| report.game_files.unused.versions.join(", ")),
                )
                .child(size(unused))
                .child(remove_button(
                    "storage-clean-game-files",
                    t(M::StorageRemoveEllipsis),
                    unused > 0 && !running && !busy,
                    cx,
                    Clean::UnusedGameFiles,
                    unused,
                ))
                .into_any_element(),
        ];

        // Snapshots, shared by every instance.
        let snapshots = report.snapshots;
        if snapshots.snapshots > 0 || snapshots.bytes > 0 {
            instance_rows.push(
                SettingRow::new(t(M::StorageSnapshots))
                    .hint(tr!(
                        M::StorageSnapshotsHint,
                        count = snapshots.snapshots,
                        size = format::bytes(snapshots.removable_bytes)
                    ))
                    .child(size(snapshots.bytes))
                    .child(remove_button(
                        "storage-clean-snapshots",
                        t(M::StorageRemoveEllipsis),
                        snapshots.removable_bytes > 0 && !busy,
                        cx,
                        Clean::Snapshots,
                        snapshots.removable_bytes,
                    ))
                    .into_any_element(),
            );
        }

        // Java runtimes.
        let unused_runtimes = report.unused_runtimes();
        let mut runtime_rows: Vec<AnyElement> = report
            .runtimes
            .iter()
            .map(|runtime| {
                let name = match runtime.java {
                    Some(major) => tr!(M::StorageJava, major = major),
                    None => runtime.component.clone(),
                };
                // "Java 17" names the component in its hint; an unknown one is named by it.
                let mut detail = Vec::new();
                if runtime.java.is_some() {
                    detail.push(runtime.component.clone());
                }
                if !runtime.used_by.is_empty() {
                    detail.push(runtime.used_by.join(", "));
                }
                SettingRow::new(name)
                    .key(SharedString::from(format!("runtime-{}", runtime.component)))
                    .hint_opt((!detail.is_empty()).then(|| detail.join(" · ")))
                    .when(runtime.unused(), |row| {
                        row.child(tag(t(M::StorageUnusedTag), colors::GRAY_500))
                    })
                    .child(size(runtime.bytes))
                    .into_any_element()
            })
            .collect();
        if runtime_rows.is_empty() {
            runtime_rows.push(SettingRow::new(t(M::StorageNoRuntimes)).into_any_element());
        } else {
            runtime_rows.push(
                SettingRow::new(t(M::StorageUnused))
                    .child(size(unused_runtimes))
                    .child(remove_button(
                        "storage-clean-runtimes",
                        t(M::StorageRemoveEllipsis),
                        unused_runtimes > 0 && !running && !busy,
                        cx,
                        Clean::UnusedRuntimes,
                        unused_runtimes,
                    ))
                    .into_any_element(),
            );
        }

        // Cache, logs and the rest.
        let logs = &report.logs;
        let mut other_rows = vec![
            SettingRow::new(t(M::StorageCache))
                .child(size(report.cache))
                .child(remove_button(
                    "storage-clean-cache",
                    t(M::StorageClearEllipsis),
                    report.cache > 0 && !running && !busy,
                    cx,
                    Clean::Cache,
                    report.cache,
                ))
                .into_any_element(),
            SettingRow::new(t(M::StorageLogs))
                .hint(tr!(M::StorageOldLogs, size = format::bytes(logs.old_bytes)))
                .child(size(logs.bytes))
                .child(remove_button(
                    "storage-clean-old-logs",
                    t(M::StorageDeleteOld),
                    logs.old_files > 0 && !busy,
                    cx,
                    Clean::OldLogs,
                    logs.old_bytes,
                ))
                .child(remove_button(
                    "storage-clean-logs",
                    t(M::StorageDeleteAll),
                    logs.files > 0 && !busy,
                    cx,
                    Clean::AllLogs,
                    logs.bytes,
                ))
                .into_any_element(),
            SettingRow::new(t(M::StorageOther))
                .child(size(report.other))
                .into_any_element(),
        ];
        if report.leftovers.bytes > 0 {
            other_rows.insert(
                1,
                SettingRow::new(t(M::StorageLeftovers))
                    .child(size(report.leftovers.bytes))
                    .child(remove_button(
                        "storage-clean-leftovers",
                        t(M::StorageRemoveEllipsis),
                        !busy,
                        cx,
                        Clean::Leftovers,
                        report.leftovers.bytes,
                    ))
                    .into_any_element(),
            );
        }

        page.child(
            div()
                .p_4()
                .rounded(radius::LG)
                .border_1()
                .border_color(colors::GRAY_FADED.opacity(30))
                .bg(colors::GRAY_800)
                .child(summary),
        )
        .child(section(t(M::StorageInstances), None).child(card(instance_rows)))
        .child(section(t(M::StorageGameFiles), None).child(card(game_rows)))
        .child(section(t(M::StorageRuntimes), None).child(card(runtime_rows)))
        .child(section(t(M::StorageCacheAndLogs), None).child(card(other_rows)))
        .into_any_element()
    }
}

impl super::LauncherShell {
    /// The dialog that asks before Settings > Storage removes `what`.
    pub(super) fn render_storage_clean(
        &self,
        what: Clean,
        bytes: u64,
        focus: gpui::FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let size = format::bytes(bytes);
        let (title, body, action) = match what {
            Clean::UnusedGameFiles => (
                M::StorageCleanGameFilesTitle,
                tr!(M::StorageCleanGameFilesBody, size = size),
                M::StorageRemove,
            ),
            Clean::UnusedRuntimes => (
                M::StorageCleanRuntimesTitle,
                tr!(M::StorageCleanRuntimesBody, size = size),
                M::StorageRemove,
            ),
            Clean::Cache => (
                M::StorageClearCacheTitle,
                tr!(M::StorageClearCacheBody, size = size),
                M::StorageClear,
            ),
            Clean::Leftovers => (
                M::StorageCleanLeftoversTitle,
                tr!(M::StorageCleanLeftoversBody, size = size),
                M::StorageRemove,
            ),
            Clean::OldLogs => (
                M::StorageDeleteOldLogsTitle,
                tr!(M::StorageDeleteOldLogsBody, size = size),
                M::StorageDeleteConfirm,
            ),
            Clean::AllLogs => (
                M::StorageDeleteAllLogsTitle,
                tr!(M::StorageDeleteAllLogsBody, size = size),
                M::StorageDeleteConfirm,
            ),
            Clean::Snapshots => (
                M::StorageCleanSnapshotsTitle,
                tr!(M::StorageCleanSnapshotsBody, size = size),
                M::StorageRemove,
            ),
        };
        let entity = cx.entity();
        crate::components::Dialog::new("storage-clean", t(title), focus)
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
            .child(div().text_color(crate::theme::ink::secondary()).child(body))
            .footer(
                Button::new("confirm-storage-clean", t(action))
                    .variant(ButtonVariant::Danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.close_modal(cx);
                        this.storage
                            .update(cx, |storage, cx| storage.clean(what, cx));
                    })),
            )
            .footer(
                Button::new("cancel-storage-clean", t(crate::i18n::CommonMsg::Cancel))
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
            )
            .into_any_element()
    }

    /// Shows Settings > Storage with a made-up count, for the offscreen screenshots.
    pub fn preview_storage(
        &mut self,
        report: Report,
        confirm: Option<Clean>,
        cx: &mut Context<Self>,
    ) {
        self.storage
            .update(cx, |storage, cx| storage.preview(report.clone(), cx));
        if let Some(what) = confirm {
            let bytes = match what {
                Clean::UnusedGameFiles => report.game_files.unused.total(),
                Clean::UnusedRuntimes => report.unused_runtimes(),
                Clean::Cache => report.cache,
                Clean::Leftovers => report.leftovers.bytes,
                Clean::OldLogs => report.logs.old_bytes,
                Clean::AllLogs => report.logs.bytes,
                Clean::Snapshots => report.snapshots.removable_bytes,
            };
            let focus = cx.focus_handle();
            self.modal
                .open(super::Modal::StorageClean { what, bytes, focus }, cx);
        }
    }
}
