//! Snapshots in the launcher: the list in an instance's Settings tab, and the dialog
//! that takes one, puts one back, shares one as a `.lodesnap` file (dragged out of the
//! window, or saved), deletes one, or makes a new instance from a file.
//!
//! The work runs on background threads; the dialog shows its progress.

use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, ExternalDragPayload, FileDragPaths,
    FocusHandle, Focusable, IntoElement, Render, SharedString, Task, Window, div, prelude::*,
};
use launcher_core::snapshots::{Category, Part, Selection, Snapshot, Store};

use super::Instances;
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Icon, LoadingDots, TextInput, Toggle,
        settings::{SettingRow, card},
    },
    i18n::{CommonMsg, InstanceSettingsMsg as M, format, t, tr},
    theme::{self, colors, px, radius, text, weights},
};

/// A snapshot in the list: its manifest without the file list.
#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub total_bytes: u64,
    pub new_bytes: u64,
    pub parts: Vec<Part>,
    pub automatic: bool,
}

impl SnapshotRow {
    fn of(snapshot: &Snapshot) -> Self {
        Self {
            id: snapshot.id.clone(),
            name: snapshot.name.clone(),
            created: snapshot.created,
            total_bytes: snapshot.total_bytes,
            new_bytes: snapshot.new_bytes,
            parts: snapshot.summary(),
            automatic: snapshot.automatic,
        }
    }
}

/// A part's name: a category, or a world's folder name.
pub fn part_label(part: &Part) -> SharedString {
    match (&part.category, &part.world) {
        (Category::Worlds, Some(world)) => world.clone().into(),
        (category, _) => t(category_message(*category)).into(),
    }
}

fn category_message(category: Category) -> M {
    match category {
        Category::Mods => M::PartMods,
        Category::ResourcePacks => M::PartResourcePacks,
        Category::ShaderPacks => M::PartShaderPacks,
        Category::Configs => M::PartConfigs,
        Category::Worlds => M::PartWorlds,
        Category::Screenshots => M::PartScreenshots,
        Category::Settings => M::PartSettings,
        Category::Other => M::PartOther,
    }
}

/// "Mods, Worlds, Instance settings": what a snapshot holds.
pub fn contents_line(parts: &[Part]) -> String {
    let mut seen = BTreeSet::new();
    let names: Vec<&'static str> = parts
        .iter()
        .filter(|p| seen.insert(p.category))
        .map(|p| t(category_message(p.category)))
        .collect();
    format::list(&names)
}

/// When a snapshot was taken, in the user's time.
pub fn when(created: u64) -> String {
    let time = chrono::DateTime::from_timestamp(created as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    format::day_and_time(time, chrono::Local::now().date_naive())
}

impl Instances {
    /// The store shared by every instance's snapshots.
    pub fn snapshot_store(&self) -> Store {
        Store::new(&self.data_dir)
    }

    /// The instance's snapshots, newest first, once read; reads them when not yet.
    pub fn snapshot_rows(&mut self, id: &str, cx: &mut Context<Self>) -> Option<&[SnapshotRow]> {
        if !self.snapshot_rows.contains_key(id) && !self.snapshot_tasks.contains_key(id) {
            self.load_snapshots(id, cx);
        }
        self.snapshot_rows.get(id).map(Vec::as_slice)
    }

    /// Reads the instance's snapshots again, in the background.
    pub fn load_snapshots(&mut self, id: &str, cx: &mut Context<Self>) {
        let store = self.snapshot_store();
        let key = id.to_string();
        let read = cx.background_spawn({
            let key = key.clone();
            async move {
                store
                    .list(&key)
                    .iter()
                    .map(SnapshotRow::of)
                    .collect::<Vec<_>>()
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let rows = read.await;
            this.update(cx, |this, cx| {
                this.snapshot_tasks.remove(&key);
                this.snapshot_rows.insert(key, rows);
                cx.notify();
            })
            .ok();
        });
        self.snapshot_tasks.insert(id.to_string(), task);
    }

    /// Saves settings a revert put back, and reads the instance again.
    pub fn apply_reverted_settings(
        &mut self,
        instance: launcher_core::instances::Instance,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store.save(&instance).map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }
}

/// What the Settings tab asks the shell to open.
#[derive(Clone, Debug)]
pub enum SnapshotRequest {
    Take(String),
    Revert(String, String),
    Share(String, String),
    Delete(String, String),
    /// A `.lodesnap` file to make a new instance from.
    Import(PathBuf),
}

pub struct SnapshotRequested(pub SnapshotRequest);

/// The dialog is done and can close.
pub struct SnapshotDismissed;

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Take,
    Revert,
    Share,
    Delete,
    Import,
}

/// What the dialog is doing.
enum State {
    /// Waiting for the user (or, while `parts` loads, for the disk).
    Ready,
    Working(M),
    /// Share: the file is ready to drag or save.
    Shared(PathBuf),
    Failed(SharedString),
}

pub struct SnapshotDialog {
    instances: Entity<Instances>,
    mode: Mode,
    instance_id: Option<String>,
    snapshot: Option<Snapshot>,
    import_path: Option<PathBuf>,
    /// The parts to choose from, with their sizes.
    parts: Option<Vec<Part>>,
    /// The chosen parts, as (category, world).
    chosen: BTreeSet<(Category, Option<String>)>,
    name: Entity<TextInput>,
    state: State,
    progress: Arc<(AtomicU64, AtomicU64)>,
    focus: FocusHandle,
    _work: Option<Task<()>>,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<SnapshotDismissed> for SnapshotDialog {}

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
        let name = cx.new(|cx| TextInput::new(cx).with_placeholder(M::InstanceSnapshots));
        let (mode, instance_id, snapshot_id, import_path) = match request {
            SnapshotRequest::Take(id) => (Mode::Take, Some(id), None, None),
            SnapshotRequest::Revert(id, s) => (Mode::Revert, Some(id), Some(s), None),
            SnapshotRequest::Share(id, s) => (Mode::Share, Some(id), Some(s), None),
            SnapshotRequest::Delete(id, s) => (Mode::Delete, Some(id), Some(s), None),
            SnapshotRequest::Import(path) => (Mode::Import, None, None, Some(path)),
        };
        let mut this = Self {
            instances,
            mode,
            instance_id,
            snapshot: None,
            import_path,
            parts: None,
            chosen: BTreeSet::new(),
            name,
            state: State::Ready,
            progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),
            focus: cx.focus_handle(),
            _work: None,
            _ticker: None,
        };
        this.load(snapshot_id, cx);
        this
    }

    /// Reads what the dialog shows: the instance's parts, a snapshot, or a file.
    fn load(&mut self, snapshot_id: Option<String>, cx: &mut Context<Self>) {
        let store = self.instances.read(cx).snapshot_store();
        match self.mode {
            Mode::Take => {
                let Some(id) = self.instance_id.clone() else {
                    return;
                };
                let dir = self.instances.read(cx).store().dir(&id);
                let default_name = format::short_date(chrono::Local::now().date_naive(), true);
                self.name
                    .update(cx, |input, cx| input.set_text(default_name, cx));
                let read = cx.background_spawn(async move { store.plan(&dir) });
                self._work = Some(cx.spawn(async move |this, cx| {
                    let parts = read.await;
                    this.update(cx, |this, cx| {
                        // Everything is chosen, except screenshots and other files.
                        this.chosen = parts
                            .iter()
                            .filter(|p| {
                                !matches!(p.category, Category::Screenshots | Category::Other)
                            })
                            .map(|p| (p.category, p.world.clone()))
                            .collect();
                        // The instance's settings are in the manifest, not files: always offered.
                        this.chosen.insert((Category::Settings, None));
                        let mut parts = parts;
                        if !parts.iter().any(|p| p.category == Category::Settings) {
                            parts.push(Part {
                                category: Category::Settings,
                                world: None,
                                files: 0,
                                bytes: 0,
                            });
                        }
                        parts.sort_by(|a, b| (a.category, &a.world).cmp(&(b.category, &b.world)));
                        this.parts = Some(parts);
                        cx.notify();
                    })
                    .ok();
                }));
            }
            Mode::Revert | Mode::Share | Mode::Delete => {
                let (Some(id), Some(snapshot_id)) = (self.instance_id.clone(), snapshot_id) else {
                    return;
                };
                match store.get(&id, &snapshot_id) {
                    Ok(snapshot) => {
                        let mut parts = snapshot.summary();
                        if snapshot.selection.categories.contains(&Category::Settings)
                            && !parts.iter().any(|p| p.category == Category::Settings)
                        {
                            parts.push(Part {
                                category: Category::Settings,
                                world: None,
                                files: 0,
                                bytes: 0,
                            });
                            parts.sort_by(|a, b| {
                                (a.category, &a.world).cmp(&(b.category, &b.world))
                            });
                        }
                        self.chosen = parts
                            .iter()
                            .map(|p| (p.category, p.world.clone()))
                            .collect();
                        self.parts = Some(parts);
                        self.snapshot = Some(snapshot);
                        if self.mode == Mode::Share {
                            self.share(cx);
                        }
                    }
                    Err(err) => self.state = State::Failed(err.to_string().into()),
                }
            }
            Mode::Import => {
                let Some(path) = self.import_path.clone() else {
                    return;
                };
                self.state = State::Working(M::ReadingSnapshot);
                let read =
                    cx.background_spawn(async move { launcher_core::snapshots::inspect(&path) });
                self._work = Some(cx.spawn(async move |this, cx| {
                    let result = read.await;
                    this.update(cx, |this, cx| {
                        match result {
                            Ok(snapshot) => {
                                let mut parts = snapshot.summary();
                                if !parts.iter().any(|p| p.category == Category::Settings) {
                                    parts.push(Part {
                                        category: Category::Settings,
                                        world: None,
                                        files: 0,
                                        bytes: 0,
                                    });
                                    parts.sort_by(|a, b| {
                                        (a.category, &a.world).cmp(&(b.category, &b.world))
                                    });
                                }
                                this.chosen = parts
                                    .iter()
                                    .map(|p| (p.category, p.world.clone()))
                                    .collect();
                                this.parts = Some(parts);
                                this.snapshot = Some(snapshot);
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

    fn selection(&self) -> Selection {
        let categories: BTreeSet<Category> = self.chosen.iter().map(|(c, _)| *c).collect();
        let worlds = self.chosen.iter().filter_map(|(_, w)| w.clone()).collect();
        Selection { categories, worlds }
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

    fn progress_fn(&self) -> impl Fn(u64, u64) + Sync + Send + 'static {
        let progress = self.progress.clone();
        move |done, total| {
            progress.0.store(done, Ordering::Relaxed);
            progress.1.store(total, Ordering::Relaxed);
        }
    }

    fn game_running(&self, cx: &App) -> bool {
        self.instance_id
            .as_deref()
            .is_some_and(|id| self.instances.read(cx).is_active(id))
    }

    /// Runs `work` in the background with progress, then `done` on the main thread.
    fn run<T: Send + 'static>(
        &mut self,
        label: M,
        work: impl FnOnce(&(dyn Fn(u64, u64) + Sync)) -> Result<T, String> + Send + 'static,
        done: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.state = State::Working(label);
        self.progress.0.store(0, Ordering::Relaxed);
        self.progress.1.store(0, Ordering::Relaxed);
        let progress = self.progress_fn();
        let job = cx.background_spawn(async move { work(&progress) });
        self._work = Some(cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| {
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
        }));
        self.tick(cx);
        cx.notify();
    }

    fn refresh_list(&self, cx: &mut Context<Self>) {
        if let Some(id) = self.instance_id.clone() {
            self.instances
                .update(cx, |instances, cx| instances.load_snapshots(&id, cx));
        }
    }

    fn take(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.instance_id.clone() else {
            return;
        };
        if self.game_running(cx) {
            self.state = State::Failed(t(M::SnapshotGameRunning).into());
            return cx.notify();
        }
        let selection = self.selection();
        if selection.categories.is_empty() {
            self.state = State::Failed(t(M::ChooseSomething).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        let Some(instance) = instances.list().iter().find(|i| i.id == id).cloned() else {
            return;
        };
        let dir = instances.store().dir(&id);
        let store = instances.snapshot_store();
        let typed = self.name.read(cx).text().trim().to_string();
        let name = if typed.is_empty() {
            format::short_date(chrono::Local::now().date_naive(), true)
        } else {
            typed
        };
        launcher_core::trace::emit("snapshot.create", || serde_json::json!({ "instance": id }));
        self.run(
            M::TakingSnapshot,
            move |progress| {
                store
                    .create(&instance, &dir, &name, None, &selection, false, progress)
                    .map_err(|err| err.to_string())
            },
            |this, _, cx| {
                this.refresh_list(cx);
                cx.emit(SnapshotDismissed);
            },
            cx,
        );
    }

    fn revert(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(snapshot)) = (self.instance_id.clone(), self.snapshot.clone()) else {
            return;
        };
        if self.game_running(cx) {
            self.state = State::Failed(t(M::SnapshotGameRunning).into());
            return cx.notify();
        }
        let selection = self.selection();
        if selection.categories.is_empty() {
            self.state = State::Failed(t(M::ChooseSomething).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        let Some(current) = instances.list().iter().find(|i| i.id == id).cloned() else {
            return;
        };
        let dir = instances.store().dir(&id);
        let store = instances.snapshot_store();
        launcher_core::trace::emit(
            "snapshot.revert",
            || serde_json::json!({ "instance": id, "snapshot": snapshot.id }),
        );
        self.run(
            M::RevertingSnapshot,
            move |progress| {
                store
                    .revert(&snapshot, &current, &dir, &selection, progress)
                    .map(|(settings, _)| settings)
                    .map_err(|err| err.to_string())
            },
            |this, settings, cx| {
                if let Some(settings) = settings
                    && let Err(err) = this.instances.update(cx, |instances, cx| {
                        instances.apply_reverted_settings(settings, cx)
                    })
                {
                    this.state = State::Failed(err.into());
                    return;
                }
                this.instances
                    .update(cx, |instances, cx| instances.reload(cx));
                this.refresh_list(cx);
                cx.emit(SnapshotDismissed);
            },
            cx,
        );
    }

    /// Writes the `.lodesnap` to the cache, to drag out or save.
    fn share(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.snapshot.clone() else {
            return;
        };
        let instances = self.instances.read(cx);
        let store = instances.snapshot_store();
        let instance_name = instances
            .list()
            .iter()
            .find(|i| Some(&i.id) == self.instance_id.as_ref())
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let folder = store.root().join("exports");
        let file = folder.join(format!(
            "{} - {}.{}",
            file_safe(&instance_name),
            file_safe(&snapshot.name),
            launcher_core::snapshots::EXTENSION
        ));
        launcher_core::trace::emit(
            "snapshot.export",
            || serde_json::json!({ "snapshot": snapshot.id }),
        );
        self.run(
            M::PreparingFile,
            move |progress| {
                std::fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
                store
                    .export(&snapshot, &file, progress)
                    .map_err(|err| err.to_string())?;
                Ok(file)
            },
            |this, file, _| this.state = State::Shared(file),
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
            let copied = cx
                .background_spawn(async move { std::fs::copy(&file, &to).map(|_| ()) })
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
        let store = self.instances.read(cx).snapshot_store();
        launcher_core::trace::emit(
            "snapshot.delete",
            || serde_json::json!({ "snapshot": snapshot.id }),
        );
        self.run(
            M::InstanceSnapshots,
            move |_| {
                store
                    .delete(&id, &snapshot.id)
                    .map_err(|err| err.to_string())
            },
            |this, _, cx| {
                this.refresh_list(cx);
                cx.emit(SnapshotDismissed);
            },
            cx,
        );
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.import_path.clone() else {
            return;
        };
        let selection = self.selection();
        if selection.categories.is_empty() {
            self.state = State::Failed(t(M::ChooseSomething).into());
            return cx.notify();
        }
        let instances = self.instances.read(cx);
        let store = instances.snapshot_store();
        let instance_store = instances.store();
        launcher_core::trace::emit("snapshot.import", || serde_json::json!({}));
        self.run(
            M::ImportingSnapshot,
            move |progress| {
                store
                    .import_as_instance(&instance_store, &path, &selection, progress)
                    .map_err(|err| err.to_string())
            },
            |this, instance, cx| {
                this.instances.update(cx, |instances, cx| {
                    instances.reload(cx);
                    instances.select(&instance.id, cx);
                });
                cx.emit(SnapshotDismissed);
            },
            cx,
        );
    }

    fn title(&self) -> SharedString {
        let name = self
            .snapshot
            .as_ref()
            .map(|s| s.name.clone())
            .unwrap_or_default();
        match self.mode {
            Mode::Take => t(M::TakeSnapshotTitle).into(),
            Mode::Revert => tr!(M::RevertTitle, name = name).into(),
            Mode::Share => tr!(M::ShareTitle, name = name).into(),
            Mode::Delete => tr!(M::DeleteSnapshotTitle, name = name).into(),
            Mode::Import => t(M::ImportSnapshotTitle).into(),
        }
    }

    fn render_parts(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let Some(parts) = &self.parts else {
            return div().child(LoadingDots).into_any_element();
        };
        let rows: Vec<_> = parts
            .iter()
            .enumerate()
            .map(|(index, part)| {
                let key = (part.category, part.world.clone());
                let on = self.chosen.contains(&key);
                let size = if part.files == 0 {
                    String::new()
                } else {
                    format::bytes(part.bytes)
                };
                SettingRow::new(part_label(part))
                    .when(part.world.is_some(), |row| row.indent())
                    .when(!size.is_empty(), |row| {
                        row.child(div().text_color(colors::GRAY_500).child(size))
                    })
                    .child(
                        Toggle::new(("snapshot-part", index), on).on_change(cx.listener(
                            move |this, value: &bool, _, cx| {
                                if *value {
                                    this.chosen.insert(key.clone());
                                } else {
                                    this.chosen.remove(&key);
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .max_h(px(320.))
            .id("snapshot-parts")
            .overflow_y_scroll()
            .child(card(rows))
            .into_any_element()
    }

    fn render_progress(&self, label: M) -> impl IntoElement + use<> {
        let done = self.progress.0.load(Ordering::Relaxed);
        let total = self.progress.1.load(Ordering::Relaxed);
        let fraction = if total == 0 {
            0.0
        } else {
            (done as f32 / total as f32).clamp(0.0, 1.0)
        };
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
                    .child(t(label)),
            )
            .child(
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
    }

    /// The finished file, to drag into another app.
    fn render_file(&self, file: &PathBuf, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let name: SharedString = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
        let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
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
                                    .text_color(colors::GRAY_500)
                                    .child(format::bytes(size)),
                            ),
                    ),
            )
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

/// A name usable in a file name.
fn file_safe(name: &str) -> String {
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
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
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
            (_, State::Working(label)) => body = body.child(self.render_progress(*label)),
            (Mode::Share, State::Shared(file)) => {
                let file = file.clone();
                body = body.child(self.render_file(&file, cx));
            }
            (Mode::Take, _) => {
                body = body
                    .child(
                        SettingRow::new(t(M::SnapshotName))
                            .child(div().w(px(260.)).child(self.name.clone())),
                    )
                    .child(
                        div()
                            .font_weight(weights::MEDIUM.into())
                            .text_color(colors::GRAY_400)
                            .child(t(M::WhatToInclude)),
                    )
                    .child(self.render_parts(cx));
            }
            (Mode::Revert, _) => {
                body = body
                    .child(div().text_color(colors::GRAY_400).child(t(M::RevertBody)))
                    .child(
                        div()
                            .font_weight(weights::MEDIUM.into())
                            .text_color(colors::GRAY_400)
                            .child(t(M::WhatToRestore)),
                    )
                    .child(self.render_parts(cx));
            }
            (Mode::Delete, _) => {
                body = body.child(
                    div()
                        .text_color(colors::GRAY_400)
                        .child(t(M::DeleteSnapshotBody)),
                );
            }
            (Mode::Import, _) => {
                if let Some(snapshot) = &self.snapshot {
                    let mods = snapshot.mods();
                    body = body.child(div().font_weight(weights::BOLD.into()).child(tr!(
                        M::SnapshotFrom,
                        name = snapshot.instance.name,
                        version = snapshot.instance.game_version
                    )));
                    if !mods.is_empty() {
                        body = body.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .p_3()
                                .rounded(radius::LG)
                                .border_1()
                                .border_color(colors::YELLOW.opacity(40))
                                .bg(colors::YELLOW.opacity(8))
                                .child(
                                    div()
                                        .text_color(colors::YELLOW_300)
                                        .child(tr!(M::SnapshotModsWarning, count = mods.len())),
                                )
                                .child(
                                    div()
                                        .text_size(theme::text_size(text::SMALL))
                                        .text_color(colors::GRAY_400)
                                        .child(format::list(&mods)),
                                ),
                        );
                    }
                    body = body
                        .child(
                            div()
                                .font_weight(weights::MEDIUM.into())
                                .text_color(colors::GRAY_400)
                                .child(t(M::WhatToInclude)),
                        )
                        .child(self.render_parts(cx));
                }
            }
            (Mode::Share, _) => {}
        }
        if let State::Failed(error) = &self.state {
            body = body.child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::RED_200)
                    .child(error.clone()),
            );
        }
        let primary: Option<(SharedString, ButtonVariant)> = match self.mode {
            Mode::Take => Some((t(M::TakeSnapshot).into(), ButtonVariant::Primary)),
            Mode::Revert => Some((t(M::Revert).into(), ButtonVariant::Primary)),
            Mode::Delete => Some((t(CommonMsg::Delete).into(), ButtonVariant::Danger)),
            Mode::Import => Some((t(M::CreateInstanceButton).into(), ButtonVariant::Primary)),
            Mode::Share => None,
        };
        let ready = !working
            && (self.parts.is_some() || self.mode == Mode::Delete)
            && !matches!(self.state, State::Shared(_));
        Dialog::new("snapshot-dialog", self.title(), self.focus.clone())
            .width(px(560.))
            .on_close({
                let entity = entity.clone();
                move |_, cx| entity.update(cx, |_, cx| cx.emit(SnapshotDismissed))
            })
            .child(body)
            .footer(
                Button::new(
                    "snapshot-cancel",
                    t(if self.mode == Mode::Share {
                        CommonMsg::Close
                    } else {
                        CommonMsg::Cancel
                    }),
                )
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(SnapshotDismissed))),
            )
            .when_some(primary, |dialog, (label, variant)| {
                dialog.footer(
                    Button::new("snapshot-go", label)
                        .variant(variant)
                        .disabled(!ready)
                        .on_click(cx.listener(|this, _, _, cx| match this.mode {
                            Mode::Take => this.take(cx),
                            Mode::Revert => this.revert(cx),
                            Mode::Delete => this.delete(cx),
                            Mode::Import => this.import(cx),
                            Mode::Share => {}
                        })),
                )
            })
    }
}
