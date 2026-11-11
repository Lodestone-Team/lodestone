//! "Import from another launcher": the instances of the launchers found in their usual
//! places (or in a folder the user picks), each with a checkbox, its version, loader and
//! size, and a queue that imports the checked ones one after another with progress and
//! Cancel. The work is `launcher_core::import`'s.
//!
//! Closing the dialog stops the queue, so while an import runs, closing (by the ×,
//! Escape or a click on the backdrop) first asks "Stop importing?" in a small dialog
//! over this one.

use crate::skin::Skinned;
use crate::theme::px;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, PathPromptOptions,
    Render, RenderImage, SharedString, Task, Window, div, img, prelude::*,
};
use launcher_core::{
    game::download::CancelToken,
    import::{
        self, CopyOptions, Found, IconSource, ImportProgress, ImportRequest, Launcher, Locations,
        Stage, WantedVersion, detect,
    },
};

use super::Instances;
use crate::components::{
    KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
};
use crate::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Icon, IconButton, LoadingDots, Toggle,
    },
    file_dialog,
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, format, t, tr, trn},
    theme::{self, colors, leading, radius, text, weights},
};

/// A size, as the list shows it: "1.2 GB".
fn format_size(bytes: u64) -> String {
    format::bytes(bytes)
}

/// The Minecraft version an instance asks for, with its loader: "1.20.1 · Fabric
/// 0.16.10".
fn version_label(found: &Found) -> String {
    let game = match &found.game_version {
        WantedVersion::Exact(version) => version.clone(),
        WantedVersion::LatestRelease => t(M::NewestRelease).into(),
        WantedVersion::LatestSnapshot => t(M::NewestSnapshot).into(),
    };
    match &found.loader {
        None => game,
        Some(loader) => match &loader.version {
            Some(version) => format!("{game} · {} {version}", loader.kind.label()),
            None => format!("{game} · {}", loader.kind.label()),
        },
    }
}

pub struct ImportDismissed;

/// The size of an instance's game folder, once measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub total: u64,
    pub worlds: u64,
}

/// Where an import in the queue is.
#[derive(Clone, Debug, PartialEq)]
pub enum QueueState {
    Waiting,
    /// Finding the version and loader, then copying and matching (the progress says
    /// which).
    Importing,
    Done {
        id: String,
        notes: Vec<String>,
    },
    Failed(SharedString),
    Cancelled,
}

/// A queue entry for the screenshots: the instance's key, its state, and bytes copied
/// out of the total.
pub type PreviewQueueItem = (String, QueueState, Option<(u64, u64)>);

pub struct QueueItem {
    pub key: String,
    pub name: String,
    pub state: QueueState,
    pub progress: Arc<ImportProgress>,
    pub cancel: CancelToken,
    found: Found,
    options: CopyOptions,
}

impl QueueItem {
    fn finished(&self) -> bool {
        matches!(
            self.state,
            QueueState::Done { .. } | QueueState::Failed(_) | QueueState::Cancelled
        )
    }
}

pub struct ImportDialog {
    instances: Entity<Instances>,
    focus: FocusHandle,
    scanning: bool,
    launchers: Vec<Launcher>,
    checked: HashSet<String>,
    sizes: HashMap<String, Size>,
    icons: HashMap<String, Arc<RenderImage>>,
    without_worlds: bool,
    queue: Vec<QueueItem>,
    error: Option<SharedString>,
    /// Detection, sizes and icons, and the folder prompt.
    tasks: Vec<Task<()>>,
    runner: Option<Task<()>>,
    ticker: Option<Task<()>>,
    /// "Stop importing?", over the dialog, with its own focus.
    stop_prompt: DialogSlot<ImportDialog, FocusHandle>,
}

impl EventEmitter<ImportDismissed> for ImportDialog {}

impl Focusable for ImportDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl file_dialog::ShowsError for ImportDialog {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.error = Some(message);
        cx.notify();
    }
}

impl ImportDialog {
    /// Looks for the other launchers in their usual places, in the background.
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let mut this = Self::empty(instances, cx);
        this.scanning = true;
        let detect = cx.background_spawn(async move {
            Locations::current()
                .map(|locations| detect::detect(&locations))
                .unwrap_or_default()
        });
        this.tasks.push(cx.spawn(async move |this, cx| {
            let launchers = detect.await;
            this.update(cx, |this, cx| {
                this.scanning = false;
                this.add_launchers(launchers, false, cx);
            })
            .ok();
        }));
        this
    }

    fn empty(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        Self {
            instances,
            focus: cx.focus_handle(),
            scanning: false,
            launchers: Vec::new(),
            checked: HashSet::new(),
            sizes: HashMap::new(),
            icons: HashMap::new(),
            without_worlds: false,
            queue: Vec::new(),
            error: None,
            tasks: Vec::new(),
            runner: None,
            ticker: None,
            stop_prompt: DialogSlot::new(|this: &mut ImportDialog| &mut this.stop_prompt),
        }
    }

    /// Shows made-up launchers, sizes and queue, for the offscreen screenshots.
    pub fn preview(
        instances: Entity<Instances>,
        launchers: Vec<Launcher>,
        sizes: HashMap<String, Size>,
        icons: HashMap<String, Arc<RenderImage>>,
        queue: Vec<PreviewQueueItem>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(instances, cx);
        this.sizes = sizes;
        this.icons = icons;
        let all: Vec<Found> = launchers.iter().flat_map(|l| l.instances.clone()).collect();
        let queued: HashSet<&String> = queue.iter().map(|(key, _, _)| key).collect();
        this.checked = all
            .iter()
            .filter(|f| !queued.contains(&f.key))
            .take(2)
            .map(|f| f.key.clone())
            .collect();
        this.launchers = launchers;
        for (key, state, bytes) in queue {
            let Some(found) = all.iter().find(|f| f.key == key) else {
                continue;
            };
            let progress = Arc::new(ImportProgress::default());
            if let Some((done, total)) = bytes {
                progress.set_stage(Stage::Copying);
                progress.bytes_done.store(done, Ordering::Relaxed);
                progress.bytes_total.store(total, Ordering::Relaxed);
            }
            this.queue.push(QueueItem {
                key,
                name: found.name.clone(),
                state,
                progress,
                cancel: CancelToken::new(),
                found: found.clone(),
                options: CopyOptions::default(),
            });
        }
        this
    }

    /// Adds launchers (from detection or a picked folder), measures their instances and
    /// reads their icons. Picked folders' instances start checked.
    fn add_launchers(&mut self, launchers: Vec<Launcher>, check: bool, cx: &mut Context<Self>) {
        let known: HashSet<String> = self.all().map(|f| f.key.clone()).collect();
        let mut new = Vec::new();
        for mut launcher in launchers {
            launcher.instances.retain(|f| !known.contains(&f.key));
            if launcher.instances.is_empty() && launcher.problems.is_empty() {
                if !self.launchers.iter().any(|l| l.root == launcher.root) {
                    self.launchers.push(launcher);
                }
                continue;
            }
            new.extend(launcher.instances.iter().cloned());
            match self.launchers.iter_mut().find(|l| l.root == launcher.root) {
                Some(existing) => {
                    existing.instances.extend(launcher.instances);
                    existing.problems.extend(launcher.problems);
                }
                None => self.launchers.push(launcher),
            }
        }
        if check {
            self.checked.extend(new.iter().map(|f| f.key.clone()));
        }
        cx.notify();
        if new.is_empty() {
            return;
        }
        let measure = cx.background_spawn(async move {
            new.into_iter()
                .map(|found| {
                    let size = import::copy::plan(&found.game_dir, &CopyOptions::default())
                        .ok()
                        .map(|plan| Size {
                            total: plan.total_bytes,
                            worlds: plan.worlds_bytes,
                        });
                    let icon = found.icon.as_ref().and_then(decode_icon);
                    (found.key, size, icon)
                })
                .collect::<Vec<_>>()
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let measured = measure.await;
            this.update(cx, |this, cx| {
                for (key, size, icon) in measured {
                    if let Some(size) = size {
                        this.sizes.insert(key.clone(), size);
                    }
                    if let Some(icon) = icon {
                        this.icons.insert(key, icon);
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn all(&self) -> impl Iterator<Item = &Found> {
        self.launchers.iter().flat_map(|l| l.instances.iter())
    }

    fn is_running(&self) -> bool {
        self.queue.iter().any(|item| !item.finished())
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(M::ImportFromHere).into()),
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Ok(read) = this.update(cx, |_, cx| {
                cx.background_spawn(async move { detect::read_folder(&path) })
            }) else {
                return;
            };
            let result = read.await;
            this.update(cx, |this, cx| match result {
                Ok(launchers) => {
                    this.error = None;
                    this.add_launchers(launchers, true, cx);
                }
                Err(err) => {
                    this.error = Some(err.into());
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    /// Ticks or unticks an instance. Like every change to what will be copied, it
    /// clears the error, so the summary of the selection shows again (a file dialog
    /// that can't open would otherwise keep it hidden until the dialog is closed).
    fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.checked.remove(key) {
            self.checked.insert(key.to_string());
        }
        self.error = None;
        cx.notify();
    }

    fn set_without_worlds(&mut self, value: bool, cx: &mut Context<Self>) {
        self.without_worlds = value;
        self.error = None;
        cx.notify();
    }

    /// The checked instances that aren't queued yet.
    fn to_import(&self) -> Vec<Found> {
        let queued: HashSet<&str> = self
            .queue
            .iter()
            .filter(|i| !matches!(i.state, QueueState::Failed(_) | QueueState::Cancelled))
            .map(|i| i.key.as_str())
            .collect();
        self.all()
            .filter(|f| self.checked.contains(&f.key) && !queued.contains(f.key.as_str()))
            .cloned()
            .collect()
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let options = CopyOptions {
            without_worlds: self.without_worlds,
        };
        for found in self.to_import() {
            self.checked.remove(&found.key);
            self.queue.retain(|i| i.key != found.key);
            self.queue.push(QueueItem {
                key: found.key.clone(),
                name: found.name.clone(),
                state: QueueState::Waiting,
                progress: Arc::new(ImportProgress::default()),
                cancel: CancelToken::new(),
                found,
                options,
            });
        }
        self.run_next(cx);
        if self.ticker.is_none() {
            self.ticker = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(150))
                        .await;
                    let running = this.update(cx, |this, cx| {
                        cx.notify();
                        this.is_running()
                    });
                    if !running.unwrap_or(false) {
                        this.update(cx, |this, _| this.ticker = None).ok();
                        break;
                    }
                }
            }));
        }
        cx.notify();
    }

    /// Starts the next waiting import, unless one is running.
    fn run_next(&mut self, cx: &mut Context<Self>) {
        if self.runner.is_some() {
            return;
        }
        let Some(index) = self
            .queue
            .iter()
            .position(|i| i.state == QueueState::Waiting)
        else {
            return;
        };
        let item = &mut self.queue[index];
        item.state = QueueState::Importing;
        let key = item.key.clone();
        let request_found = item.found.clone();
        let options = item.options;
        let progress = item.progress.clone();
        let cancel = item.cancel.clone();
        let (installer, store, api) = {
            let instances = self.instances.read(cx);
            (
                instances.installer(),
                instances.store(),
                instances.modrinth(),
            )
        };
        let work = cx.background_spawn({
            let this_progress = progress.clone();
            async move {
                let resolved = import::resolve(&installer, &request_found).await?;
                if cancel.is_cancelled() {
                    return Err(import::ImportError::Cancelled);
                }
                let request = ImportRequest {
                    found: request_found,
                    resolved,
                    options,
                };
                import::import(&store, &request, api.as_deref(), &this_progress, &cancel).await
            }
        });
        self.runner = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.runner = None;
                if let Some(item) = this.queue.iter_mut().find(|i| i.key == key) {
                    item.state = match result {
                        Ok(imported) => {
                            let id = imported.instance.id.clone();
                            this.instances.update(cx, |instances, cx| {
                                instances.select(&id, cx);
                                instances.reload(cx);
                            });
                            let mut notes = imported.notes;
                            if imported.skipped.iter().any(|(_, why)| why.contains("link")) {
                                notes.push(t(M::LinksLeftOut).into());
                            }
                            QueueState::Done { id, notes }
                        }
                        Err(import::ImportError::Cancelled) => QueueState::Cancelled,
                        Err(err) => QueueState::Failed(err.to_string().into()),
                    };
                }
                this.advance(cx);
            })
            .ok();
        }));
    }

    /// After an import ends: starts the next one, and puts away "Stop importing?"
    /// if nothing is importing any more, since its question no longer applies.
    fn advance(&mut self, cx: &mut Context<Self>) {
        self.run_next(cx);
        if !self.is_running() && self.stop_prompt.is_open() {
            self.stop_prompt.close(cx);
        }
        cx.notify();
    }

    fn cancel(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(item) = self.queue.iter_mut().find(|i| i.key == key) {
            item.cancel.cancel();
            if item.state == QueueState::Waiting {
                item.state = QueueState::Cancelled;
            }
        }
        cx.notify();
    }

    /// Closing by the ×, Escape or the backdrop: at once when nothing is importing,
    /// or after asking "Stop importing?" when something is. The question takes focus
    /// when it is drawn, and gives it back when it closes.
    pub fn request_close(&mut self, cx: &mut Context<Self>) {
        if !self.is_running() {
            return self.close(cx);
        }
        if !self.stop_prompt.is_open() {
            self.stop_prompt.open(cx.focus_handle(), cx);
        }
    }

    /// Whether "Stop importing?" is showing.
    pub fn asking_to_stop(&self) -> bool {
        self.stop_prompt.is_open()
    }

    /// "Keep importing": back to the dialog, with the imports still running.
    pub fn keep_importing(&mut self, cx: &mut Context<Self>) {
        self.stop_prompt.close(cx);
    }

    /// "Stop importing": stops the queue and closes.
    pub fn stop_importing(&mut self, cx: &mut Context<Self>) {
        self.stop_prompt.close(cx);
        self.close(cx);
    }

    /// Stops every import (what was copied is removed) and closes.
    fn close(&mut self, cx: &mut Context<Self>) {
        for item in &self.queue {
            item.cancel.cancel();
        }
        cx.emit(ImportDismissed);
    }

    fn instance_row(&self, found: &Found, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let key = found.key.clone();
        let checked = self.checked.contains(&key);
        let size = self.sizes.get(&key).copied();
        let size_text: SharedString = match size {
            Some(size) if self.without_worlds && size.worlds > 0 => {
                format_size(size.total - size.worlds).into()
            }
            Some(size) => format_size(size.total).into(),
            None => "…".into(),
        };
        let icon = match self.icons.get(&key) {
            Some(image) => img(image.clone()),
            None => img("images/instance-icons/grass.png"),
        };
        let note = found.notes.first().cloned();
        let id = SharedString::from(format!("import-{key}"));
        div()
            .id(id.clone())
            .accessible(Role::CheckBox, id, found.name.clone())
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .aria_description(format!("{} · {size_text}", version_label(found)))
            .keyboard_focus()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_2()
            .py_1p5()
            .rounded(radius::MD)
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_800))
            .child(checkbox(checked))
            .child(icon.flex_none().size(px(32.)).rounded(radius::DEFAULT))
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
                            .text_color(crate::theme::ink::text())
                            .child(found.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(crate::theme::ink::muted())
                            .child(version_label(found)),
                    )
                    .when_some(note, |this, note| {
                        this.child(
                            div()
                                .truncate()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::YELLOW_300.opacity(80))
                                .child(note),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(crate::theme::ink::secondary())
                    .child(size_text),
            )
            .on_press(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
    }

    fn launcher_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for launcher in &self.launchers {
            rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap_2()
                    .pt_2()
                    .px_2()
                    .child(
                        div()
                            .flex_none()
                            .text_size(theme::text_size(text::SMALL))
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::theme::ink::secondary())
                            .child(launcher.source.label().to_uppercase()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .text_size(theme::text_size(text::CAPTION))
                            .text_color(crate::theme::ink::faint())
                            .child(launcher.root.display().to_string()),
                    )
                    .into_any_element(),
            );
            if launcher.instances.is_empty() && launcher.problems.is_empty() {
                rows.push(
                    div()
                        .px_2()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::muted())
                        .child(t(M::NoInstances))
                        .into_any_element(),
                );
            }
            for found in &launcher.instances {
                rows.push(self.instance_row(found, cx).into_any_element());
            }
            if let Some(problem) = launcher.problems.first() {
                rows.push(
                    div()
                        .px_2()
                        .whitespace_normal()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::RED_200)
                        .child(if launcher.problems.len() > 1 {
                            trn!(
                                P::ProblemAndMore,
                                launcher.problems.len() - 1,
                                problem = problem
                            )
                        } else {
                            problem.clone()
                        })
                        .into_any_element(),
                );
            }
        }
        let body = if self.scanning {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .h(px(120.))
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::LookingForLaunchers))
                .into_any_element()
        } else if rows.is_empty() {
            div()
                .flex()
                .items_center()
                .h(px(120.))
                .px_8()
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .whitespace_normal()
                        .text_color(crate::theme::ink::muted())
                        .child(t(M::NoLaunchersFound)),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .children(rows)
                .into_any_element()
        };
        // Room for the queue below while there is one.
        let max_height = if self.queue.is_empty() { 330. } else { 190. };
        div()
            .id("import-list")
            .max_h(px(max_height))
            .overflow_y_scroll()
            .p_1()
            .pb_2()
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .bg(colors::GRAY_900)
            .child(body)
    }

    fn queue_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("import-queue")
            .max_h(px(200.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .children(self.queue.iter().map(|item| {
                let key = item.key.clone();
                let (status, color, fraction): (String, _, Option<f32>) = match &item.state {
                    QueueState::Waiting => (t(M::Waiting).into(), crate::theme::ink::muted(), None),
                    QueueState::Importing => {
                        (t(M::FindingVersion).into(), colors::GRAY_400, Some(0.))
                    }
                    QueueState::Done { notes, .. } => (
                        match notes.first() {
                            Some(note) => tr!(M::ImportedNote, note = note),
                            None => t(M::Imported).into(),
                        },
                        colors::GREEN_300,
                        None,
                    ),
                    QueueState::Failed(error) => (error.to_string(), colors::RED_200, None),
                    QueueState::Cancelled => {
                        (t(M::Cancelled).into(), crate::theme::ink::muted(), None)
                    }
                };
                let (status, fraction) = match (&item.state, item.progress.stage()) {
                    (QueueState::Importing, Stage::Copying) => {
                        let done = item.progress.bytes_done.load(Ordering::Relaxed);
                        let total = item.progress.bytes_total.load(Ordering::Relaxed);
                        (
                            tr!(
                                M::Copying,
                                done = format_size(done),
                                total = format_size(total)
                            ),
                            item.progress.fraction().or(Some(0.)),
                        )
                    }
                    (QueueState::Importing, Stage::Matching) => {
                        (t(M::MatchingModrinth).into(), Some(1.))
                    }
                    _ => (status, fraction),
                };
                let running = !item.finished();
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(radius::LG)
                    .bg(colors::GRAY_900)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_none()
                                            .font_weight(weights::BOLD.into())
                                            .text_color(crate::theme::ink::text())
                                            .child(item.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .whitespace_normal()
                                            .text_size(theme::text_size(text::SMALL))
                                            .line_height(theme::line_height(
                                                text::SMALL,
                                                leading::NORMAL,
                                            ))
                                            .text_color(color)
                                            .child(status),
                                    ),
                            )
                            .when_some(fraction, |this, fraction| {
                                this.child(progress_bar(fraction))
                            }),
                    )
                    .when(running, |this| {
                        this.child(
                            IconButton::new(
                                SharedString::from(format!("cancel-{key}")),
                                Icon::Xmark,
                            )
                            .label(t(CommonMsg::Cancel))
                            .on_click(cx.listener(move |this, _, _, cx| this.cancel(&key, cx))),
                        )
                    })
            }))
    }
}

/// An icon's pixels, small, for the list.
fn decode_icon(icon: &IconSource) -> Option<Arc<RenderImage>> {
    let bytes = match icon {
        IconSource::Bytes(bytes) => bytes.clone(),
        IconSource::File(path) => {
            let metadata = std::fs::metadata(path).ok()?;
            if metadata.len() > 16 * 1024 * 1024 {
                return None;
            }
            std::fs::read(path).ok()?
        }
    };
    let image = image::load_from_memory(&bytes).ok()?;
    let small = image.resize_exact(64, 64, image::imageops::FilterType::Triangle);
    Some(crate::images::render_image(small.to_rgba8()))
}

fn checkbox(checked: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .rounded(radius::DEFAULT)
        .border_1()
        .map(|this| {
            if checked {
                this.bg(crate::theme::accent::base())
                    .border_color(crate::theme::accent::base())
            } else {
                this.border_color(colors::GRAY_FADED.opacity(50))
            }
        })
        .when(checked, |this| {
            this.child(Icon::Check.render(px(11.), colors::WHITE))
        })
}

fn progress_bar(fraction: f32) -> impl IntoElement {
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
                .w(gpui::relative(fraction.clamp(0., 1.))),
        )
}

impl Render for ImportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let to_import = self.to_import();
        let count = to_import.len();
        let selected_worlds: u64 = to_import
            .iter()
            .filter_map(|f| self.sizes.get(&f.key))
            .map(|s| s.worlds)
            .sum();
        let selected_total: u64 = to_import
            .iter()
            .filter_map(|f| self.sizes.get(&f.key))
            .map(|s| s.total)
            .sum();
        let summary: SharedString = match count {
            0 => t(M::CheckToImport).into(),
            n => {
                let total = if self.without_worlds {
                    selected_total - selected_worlds
                } else {
                    selected_total
                };
                trn!(P::InstancesToCopy, n, size = format_size(total)).into()
            }
        };
        let worlds_caption: SharedString = if selected_worlds > 0 {
            tr!(M::LeaveWorldsOutSize, size = format_size(selected_worlds)).into()
        } else {
            t(M::LeaveWorldsOut).into()
        };
        let without_worlds = self.without_worlds;
        let dialog = Dialog::new("import", t(M::ImportTitle), self.focus.clone())
            .width(px(760.))
            .on_close({
                let entity = entity.clone();
                move |_, cx| entity.update(cx, |this, cx| this.request_close(cx))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(self.launcher_list(cx))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .whitespace_normal()
                                    .text_size(theme::text_size(text::SMALL))
                                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                    .font_weight(weights::BOLD.into())
                                    .map(|this| match &self.error {
                                        Some(error) => {
                                            this.text_color(colors::RED_200).child(error.clone())
                                        }
                                        None => this
                                            .text_color(crate::theme::ink::muted())
                                            .child(summary),
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_none()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Toggle::new("import-without-worlds", without_worlds)
                                            .label(worlds_caption.clone())
                                            .on_change(move |value, _, cx| {
                                                let value = *value;
                                                entity.update(cx, |this, cx| {
                                                    this.set_without_worlds(value, cx)
                                                })
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme::text_size(text::SMALL))
                                            .font_weight(weights::BOLD.into())
                                            .text_color(crate::theme::ink::secondary())
                                            .child(worlds_caption),
                                    ),
                            ),
                    )
                    .when(!self.queue.is_empty(), |this| {
                        this.child(self.queue_list(cx))
                    }),
            )
            .footer(
                Button::new(
                    "start-import",
                    match count {
                        0 | 1 => t(M::Import).to_string(),
                        n => trn!(P::ImportInstances, n),
                    },
                )
                .variant(ButtonVariant::Primary)
                .icon(Icon::Download)
                .disabled(count == 0)
                .on_click(cx.listener(|this, _, _, cx| this.start(cx))),
            )
            .footer(
                Button::new("import-choose-folder", t(M::ChooseFolder))
                    .variant(ButtonVariant::Secondary)
                    .disabled(self.scanning)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_folder(cx))),
            );
        let stop_prompt = self.stop_prompt.render(window, |focus| {
            Some(stop_prompt(focus.clone(), cx).into_any_element())
        });
        // The prompt is a second dialog over this one, so this view fills the window
        // for both of them.
        div()
            .absolute()
            .inset_0()
            .child(dialog)
            .children(stop_prompt)
    }
}

/// "Stop importing?": Stop importing, or Keep importing (which Escape and the
/// backdrop also mean).
fn stop_prompt(focus: FocusHandle, cx: &mut Context<ImportDialog>) -> Dialog {
    let entity = cx.entity();
    Dialog::new("import-stop", t(M::StopImportingTitle), focus)
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.keep_importing(cx)))
        .child(
            div()
                .text_color(crate::theme::ink::secondary())
                .child(t(M::StopImportingBody)),
        )
        .footer(
            Button::new("import-stop-confirm", t(M::StopImporting))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(|this, _, _, cx| this.stop_importing(cx))),
        )
        .footer(
            Button::new("import-keep-going", t(M::KeepImporting))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.keep_importing(cx))),
        )
}

/// The file dialog's error leaves room for the selection's summary again at the next
/// change to the selection.
#[cfg(all(test, feature = "screenshot"))]
mod gpui_tests {
    use futures::channel::oneshot;
    use gpui::TestAppContext;

    use super::*;
    use crate::i18n::CommonMsg;

    #[test]
    fn changing_the_selection_clears_the_file_dialog_error() {
        let mut cx = TestAppContext::single();
        let dialog = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let instances = crate::preview::instances("signed-out", &accounts, cx);
            cx.new(|cx| ImportDialog::empty(instances, cx))
        });
        let error = |cx: &mut TestAppContext| dialog.read_with(cx, |this, _| this.error.clone());
        let fail = |cx: &mut TestAppContext| {
            let (tx, rx) = oneshot::channel::<anyhow::Result<Option<Vec<std::path::PathBuf>>>>();
            let _ = tx.send(Err(anyhow::anyhow!("no portal")));
            let weak = dialog.downgrade();
            cx.spawn(|mut cx| async move {
                file_dialog::chosen(rx, &weak, &mut cx).await;
            })
            .detach();
            cx.run_until_parked();
        };

        fail(&mut cx);
        assert_eq!(
            error(&mut cx),
            Some(t(CommonMsg::FileDialogUnavailable).into())
        );
        dialog.update(&mut cx, |this, cx| this.toggle("Vanilla:Survival", cx));
        assert_eq!(error(&mut cx), None);

        fail(&mut cx);
        dialog.update(&mut cx, |this, cx| this.set_without_worlds(true, cx));
        assert_eq!(error(&mut cx), None);

        fail(&mut cx);
        dialog.update(&mut cx, |this, cx| this.start(cx));
        assert_eq!(error(&mut cx), None);
    }
}

/// Closing while an import runs asks "Stop importing?" first.
#[cfg(all(test, feature = "screenshot"))]
mod stop_prompt_tests {
    use std::{cell::Cell, path::PathBuf, rc::Rc};

    use gpui::{Modifiers, TestAppContext, VisualTestContext, point};
    use launcher_core::import::Source;

    use super::*;
    use crate::components::EXIT;

    struct Opened<'a> {
        dialog: Entity<ImportDialog>,
        cx: &'a mut VisualTestContext,
        dismissed: Rc<Cell<usize>>,
    }

    impl Opened<'_> {
        fn asking(&mut self) -> bool {
            self.dialog
                .read_with(self.cx, |this, _| this.asking_to_stop())
        }

        fn dismissed(&self) -> usize {
            self.dismissed.get()
        }

        fn cancelled(&mut self) -> bool {
            self.dialog
                .read_with(self.cx, |this, _| this.queue[0].cancel.is_cancelled())
        }

        /// Lets a closing prompt finish fading out, so it no longer takes clicks.
        fn settle(&mut self) {
            self.cx.executor().advance_clock(EXIT * 2);
            self.cx.run_until_parked();
        }

        fn click_close_button(&mut self) {
            let bounds = self.cx.debug_bounds("dialog-close").expect("the ×");
            self.cx.simulate_click(bounds.center(), Modifiers::none());
        }

        /// A click at the window's corner: on the backdrop of the dialog on top.
        fn click_backdrop(&mut self) {
            self.cx
                .simulate_click(point(px(4.), px(4.)), Modifiers::none());
        }
    }

    /// The import dialog in a window, with one instance in its queue, importing
    /// (`running`) or already imported.
    fn open(running: bool, cx: &mut TestAppContext) -> Opened<'_> {
        cx.update(crate::components::init);
        let (dialog, cx) = cx.add_window_view(|_, cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let instances = crate::preview::instances("signed-out", &accounts, cx);
            let mut this = ImportDialog::empty(instances, cx);
            let found = Found {
                source: Source::Vanilla,
                key: "Vanilla:Survival".into(),
                name: "Survival".into(),
                game_dir: PathBuf::from("/nonexistent/Survival"),
                game_version: WantedVersion::LatestRelease,
                loader: None,
                memory: None,
                jvm_args: None,
                icon: None,
                notes: Vec::new(),
            };
            this.queue.push(QueueItem {
                key: found.key.clone(),
                name: found.name.clone(),
                state: if running {
                    QueueState::Importing
                } else {
                    QueueState::Done {
                        id: "survival".into(),
                        notes: Vec::new(),
                    }
                },
                progress: Arc::new(ImportProgress::default()),
                cancel: CancelToken::new(),
                found,
                options: CopyOptions::default(),
            });
            this
        });
        let dismissed = Rc::new(Cell::new(0));
        let count = dismissed.clone();
        cx.update(|_, cx| {
            cx.subscribe(&dialog, move |_, _: &ImportDismissed, _| {
                count.set(count.get() + 1)
            })
            .detach()
        });
        cx.run_until_parked();
        Opened {
            dialog,
            cx,
            dismissed,
        }
    }

    #[test]
    fn closing_while_nothing_imports_is_immediate() {
        for close in ["escape", "×", "backdrop"] {
            let mut cx = TestAppContext::single();
            let mut opened = open(false, &mut cx);
            match close {
                "escape" => opened.cx.simulate_keystrokes("escape"),
                "×" => opened.click_close_button(),
                _ => opened.click_backdrop(),
            }
            assert!(!opened.asking(), "{close}: no question");
            assert_eq!(opened.dismissed(), 1, "{close} closes the dialog");
        }
    }

    #[test]
    fn closing_while_importing_asks_by_every_route() {
        for close in ["escape", "×", "backdrop"] {
            let mut cx = TestAppContext::single();
            let mut opened = open(true, &mut cx);
            match close {
                "escape" => opened.cx.simulate_keystrokes("escape"),
                "×" => opened.click_close_button(),
                _ => opened.click_backdrop(),
            }
            assert!(opened.asking(), "{close} asks first");
            assert_eq!(opened.dismissed(), 0, "{close}: still open");
            assert!(!opened.cancelled(), "{close}: still importing");
        }
    }

    #[test]
    fn escape_or_the_backdrop_on_the_question_keeps_importing() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.cx.simulate_keystrokes("escape");
        assert!(opened.asking());
        // Escape on the question: Keep importing.
        opened.cx.simulate_keystrokes("escape");
        assert!(!opened.asking());
        assert_eq!(opened.dismissed(), 0);
        assert!(!opened.cancelled());
        opened.settle();

        // So does a click beside it.
        opened.click_close_button();
        assert!(opened.asking());
        opened.click_backdrop();
        assert!(!opened.asking());
        assert_eq!(opened.dismissed(), 0);
        assert!(!opened.cancelled());
        let state = opened
            .dialog
            .read_with(opened.cx, |this, _| this.queue[0].state.clone());
        assert_eq!(state, QueueState::Importing);
    }

    #[test]
    fn keep_importing_returns_to_the_dialog() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.cx.simulate_keystrokes("escape");
        assert!(opened.asking());
        // The question took focus: past its ×, Stop importing, then Keep importing.
        opened.cx.simulate_keystrokes("tab tab tab");
        crate::components::focus::press_enter(opened.cx);
        assert!(!opened.asking());
        assert_eq!(opened.dismissed(), 0);
        assert!(!opened.cancelled());
        // Asked again on the next close.
        opened.settle();
        opened.cx.simulate_keystrokes("escape");
        assert!(opened.asking());
    }

    #[test]
    fn the_question_goes_away_when_the_last_import_ends() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.cx.simulate_keystrokes("escape");
        assert!(opened.asking());
        // The import finishes while the question is showing.
        opened.dialog.update(opened.cx, |this, cx| {
            this.queue[0].state = QueueState::Done {
                id: "survival".into(),
                notes: Vec::new(),
            };
            this.advance(cx);
        });
        opened.cx.run_until_parked();
        assert!(!opened.asking(), "nothing to stop any more");
        assert_eq!(opened.dismissed(), 0, "the dialog stays open");
        // And the next close is immediate.
        opened.settle();
        opened.cx.simulate_keystrokes("escape");
        assert!(!opened.asking());
        assert_eq!(opened.dismissed(), 1);
    }

    #[test]
    fn stop_importing_cancels_the_queue_and_closes() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.click_close_button();
        assert!(opened.asking());
        // Past the question's ×, to Stop importing.
        opened.cx.simulate_keystrokes("tab tab");
        crate::components::focus::press_enter(opened.cx);
        assert!(!opened.asking());
        assert!(opened.cancelled(), "the running import is cancelled");
        assert_eq!(opened.dismissed(), 1, "and the dialog closes");
    }
}
