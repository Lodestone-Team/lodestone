//! Settings > Storage: what the data folder takes, by instance, game files, Java
//! runtimes, the cache and logs, with a way to remove what can go.
//!
//! The count runs on a background thread while the page is shown and stops when it
//! is left. Removing asks first (the shell's dialog), runs in the background, and
//! counts again. The rules for what is safe to remove are `launcher_core::disk`'s.

use crate::theme::px;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, Render, SharedString, Task, Window,
    div, prelude::*, relative,
};
use launcher_core::disk::{self, Cancel, Clean, Report};

use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, LoadingDots,
        settings::{SettingRow, Toast, card, muted_link, section, tag, value},
    },
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
}

pub struct StorageSettings {
    instances: Entity<Instances>,
    count: Count,
    /// The running count or removal, and how to stop it.
    work: Option<(Task<()>, Cancel)>,
    /// A removal is running.
    cleaning: bool,
    toast: Toast,
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

    /// Shows a count, for the offscreen screenshots.
    pub fn preview(&mut self, report: Report, cx: &mut Context<Self>) {
        if let Some((_, cancel)) = self.work.take() {
            cancel.cancel();
        }
        self.count = Count::Ready(Box::new(report));
        cx.notify();
    }
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
        let page = div().flex().flex_col().gap_6().w_full();
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
                    .when(self.work.is_some() && !busy, |this| this.child(LoadingDots))
                    .child(div().flex_1())
                    .child(
                        muted_link("storage-open-data", t(M::StorageOpenFolder)).on_click(
                            cx.listener(|this, _, _, cx| {
                                let root = this.instances.read(cx).data_dir().root().to_path_buf();
                                cx.open_with_system(&root);
                            }),
                        ),
                    ),
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
                                .on_click(cx.listener(move |this, _, _, cx| {
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
