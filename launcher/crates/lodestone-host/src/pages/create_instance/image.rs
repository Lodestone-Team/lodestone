//! Making a server from a `.lodeimg` file another Host or a Launcher exported: choose the
//! file, which is sent to the core and read there, then see what it holds (with the
//! warning for the mods it carries itself, what will be downloaded, what the sender took
//! out and what the server won't take), pick its world, name and port, tick the EULA and
//! create it. The core makes the server in the background (`images::share` in the core);
//! the sidebar shows its progress.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Task, Window, div, prelude::*, relative,
};
use launcher_core::cores::admin::images::{
    CreateFromImageRequest, ImageMaker, ImageSummary, ImageUpload, NotTaken, is_image_file,
};
use launcher_core::cores::admin::models::CorePermission;
use launcher_core::game::download::{CancelToken, Progress};
use lodestone_ui::{
    components::{Button, ButtonVariant, Checkbox, ListRow, TextInput, Toast},
    file_dialog::{Pick, ShowsError},
    theme::{colors, ink, px, text},
};

use super::{
    error_line, eula, fields, loading_line,
    modpack::{NAME_FORBIDDEN, check_settings, first_free_port},
    wizard,
};
use crate::{
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, format, t, tr, trn},
    pages::parts,
    state::HostState,
};

/// What the flow tells the dialog.
pub enum ImageEvent {
    /// Back to the game selection.
    Back,
    /// The server is being made: close the dialog.
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    File,
    Summary,
}

/// A file being sent to the core.
struct Uploading {
    progress: Arc<Progress>,
    cancel: CancelToken,
}

/// The summary step's text fields.
struct Settings {
    name: Entity<TextInput>,
    port: Entity<TextInput>,
}

pub struct ImageFlow {
    state: Entity<HostState>,
    step: Step,
    uploading: Option<Uploading>,
    error: Option<String>,
    /// The file the core holds, and the settings made for it.
    upload: Option<(ImageUpload, Settings)>,
    /// The world (its folder) that becomes the server's.
    world: Option<String>,
    /// The server software picked, one of the summary's choices.
    server: Option<String>,
    keep_operators: bool,
    eula: bool,
    errors: std::collections::HashMap<&'static str, String>,
    submitting: bool,
    _tasks: Vec<Task<()>>,
    /// Discards the file the core holds when the dialog closes on the summary.
    _release: Subscription,
}

impl EventEmitter<ImageEvent> for ImageFlow {}

impl ShowsError for ImageFlow {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast(Toast::error(message), cx);
    }
}

/// A server's name from the file: its label without control and invisible characters,
/// else the file's name without `.lodeimg`, at most 64 characters, with the characters a
/// server's name can't have replaced (an unnamed backup's label has the time, `14:10`).
pub fn default_name(summary: &ImageSummary) -> String {
    let label = summary.label.as_deref().unwrap_or_default();
    // the core writes invisible characters visibly (`\u{…}`): those go too
    let visible = |text: &str| -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find("\\u{") {
            out.push_str(&rest[..at]);
            rest = rest[at..].split_once('}').map_or("", |(_, after)| after);
        }
        out.push_str(rest);
        out.chars().filter(|c| !c.is_control()).collect()
    };
    let label = visible(label);
    let name = if label.trim().is_empty() {
        let file = visible(&summary.file_name);
        file.strip_suffix(".lodeimg").unwrap_or(&file).to_string()
    } else {
        label
    };
    name.chars()
        .map(|c| match c {
            ':' => '.',
            c if NAME_FORBIDDEN.contains(&c) => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect()
}

/// The note on the server made, in the Host's words: a Bukkit-family server the file names
/// (Spigot, Purpur…) is made as Paper, said only while Paper is the server picked.
pub fn server_note(summary: &ImageSummary, picked: Option<&str>) -> Option<String> {
    let flavour = summary.flavour.as_deref()?;
    let paper = picked.or(summary.server.as_deref()) == Some("Paper");
    let relative = ["spigot", "bukkit", "purpur", "folia"]
        .iter()
        .any(|f| flavour.eq_ignore_ascii_case(f));
    (paper && relative).then(|| {
        let mut name = flavour.to_string();
        if let Some(first) = name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        tr!(M::ImageMadeAsPaper, flavour = name)
    })
}

/// Why no server can be made from the file, in the Host's words, when none can.
pub fn unsupported(summary: &ImageSummary) -> Option<String> {
    summary.unsupported.as_ref()?;
    let mut loader = summary.loader.clone();
    if let Some(first) = loader.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    Some(tr!(M::ImageUnsupported, loader = loader))
}

/// The world picked when the summary opens: the file's own choice when it names one it
/// has (a Host's level, a Launcher's last played world).
pub fn default_world(summary: &ImageSummary) -> Option<String> {
    summary
        .default_world
        .clone()
        .filter(|w| summary.worlds.iter().any(|world| &world.folder == w))
        .or_else(|| summary.worlds.first().map(|w| w.folder.clone()))
}

/// The game in a line: "Fabric 0.16.10 · Minecraft 1.21.1", "Paper · Minecraft 1.21.1".
pub fn game_line(summary: &ImageSummary) -> String {
    let software = summary
        .server
        .clone()
        .or_else(|| summary.flavour.clone())
        .unwrap_or_else(|| summary.loader.clone());
    let software = match (&summary.loader_version, summary.loader.as_str()) {
        (Some(version), loader) if loader != "vanilla" => format!("{software} {version}"),
        _ => software,
    };
    format!("{software} · Minecraft {}", summary.minecraft)
}

/// The server software to ask for: the one picked, only when the file's game allows a
/// choice and it is one of them; otherwise the core makes the file's own.
pub fn chosen_server(summary: &ImageSummary, picked: Option<&str>) -> Option<String> {
    let picked = picked?;
    (summary.server_choices.len() > 1 && summary.server_choices.iter().any(|c| c == picked))
        .then(|| picked.to_string())
}

/// What the server won't take from the file, as a list.
fn not_taken(what: &[NotTaken]) -> String {
    what.iter()
        .filter_map(|n| {
            Some(t(match n {
                NotTaken::ServerSoftware => M::NotTakenSoftware,
                NotTaken::Eula => M::NotTakenEula,
                NotTaken::ClientFiles => M::NotTakenClientFiles,
                NotTaken::Operators => M::NotTakenOperators,
                NotTaken::Scripts => M::NotTakenScripts,
                NotTaken::Unknown => return None,
            }))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl ImageFlow {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            step: Step::File,
            uploading: None,
            error: None,
            upload: None,
            world: None,
            server: None,
            keep_operators: false,
            eula: false,
            errors: Default::default(),
            submitting: false,
            _tasks: Vec::new(),
            // the dialog closed (its X, Escape) with a file the core holds: forget it; one
            // still being sent stops
            _release: cx.on_release(|this, cx| {
                if let Some(run) = &this.uploading {
                    run.cancel.cancel();
                }
                let admin = this.state.read(cx).admin.clone();
                Self::discard_with(admin, this.upload.take(), cx);
            }),
        }
    }

    /// Keeps the tasks still running.
    fn push_task(&mut self, task: Task<()>) {
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    fn admin(&self, cx: &Context<Self>) -> launcher_core::cores::admin::AdminClient {
        self.state.read(cx).admin.clone()
    }

    fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    /// The system's file dialog, then the upload.
    fn browse(&mut self, cx: &mut Context<Self>) {
        if self.uploading.is_some() {
            return;
        }
        let picked = cx.prompt_for_paths(Pick::Files.options(false, t(M::ImageFileTitle)));
        let task = cx.spawn(async move |this, cx| {
            let Some(paths) = lodestone_ui::file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.upload(path, cx)).ok();
        });
        self.push_task(task);
    }

    /// Sends the file at `path` to the core, which reads it and says what it holds.
    fn upload(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        self.error = None;
        if !is_image_file(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.error = Some(tr!(M::ImageNotLodeimg, name = name));
            cx.notify();
            return;
        }
        let run = Uploading {
            progress: Arc::new(Progress::default()),
            cancel: CancelToken::new(),
        };
        let (progress, cancel) = (run.progress.clone(), run.cancel.clone());
        self.uploading = Some(run);
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let ticker = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(250))
                            .await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    }
                }
            });
            let result = cx
                .background_spawn({
                    let cancel = cancel.clone();
                    async move { admin.upload_image(&path, progress, &cancel).await }
                })
                .await;
            drop(ticker);
            this.update(cx, |this, cx| {
                this.uploading = None;
                match result {
                    Ok(upload) => this.open_summary(upload, cx),
                    Err(_) if cancel.is_cancelled() => {}
                    Err(err) => this.error = Some(tr!(M::ImageUploadFailed, error = err)),
                }
                cx.notify();
            })
            .ok();
        });
        self.push_task(task);
        cx.notify();
    }

    fn cancel_upload(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &self.uploading {
            run.cancel.cancel();
        }
        cx.notify();
    }

    /// The summary of an uploaded file, with settings made for it.
    fn open_summary(&mut self, upload: ImageUpload, cx: &mut Context<Self>) {
        let used: Vec<u32> = self
            .state
            .read(cx)
            .instances
            .iter()
            .flatten()
            .map(|instance| instance.port)
            .collect();
        let input = |text: String, label: M, cx: &mut Context<Self>| {
            cx.new(|cx| {
                TextInput::new(cx)
                    .with_text(SharedString::from(text))
                    .with_label(label)
            })
        };
        let settings = Settings {
            name: input(default_name(&upload.summary), M::FieldName, cx),
            port: input(first_free_port(&used).to_string(), M::FieldPort, cx),
        };
        self.world = default_world(&upload.summary);
        self.server = upload.summary.server.clone();
        self.keep_operators = false;
        self.eula = false;
        self.errors.clear();
        self.upload = Some((upload, settings));
        self.step = Step::Summary;
    }

    /// Forgets the file the core holds, when the person backs out.
    fn discard(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin(cx);
        Self::discard_with(admin, self.upload.take(), cx);
    }

    fn discard_with(
        admin: launcher_core::cores::admin::AdminClient,
        upload: Option<(ImageUpload, Settings)>,
        cx: &gpui::App,
    ) {
        let Some((upload, _)) = upload else {
            return;
        };
        if upload.upload_id.is_empty() {
            return;
        }
        cx.background_spawn(async move {
            // the core forgets it after an hour anyway
            let _ = admin.discard_image_upload(&upload.upload_id).await;
        })
        .detach();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Summary => {
                self.discard(cx);
                self.step = Step::File;
                cx.notify();
            }
            Step::File => {
                self.cancel_upload(cx);
                cx.emit(ImageEvent::Back);
            }
        }
    }

    fn go_to(&mut self, step: usize, cx: &mut Context<Self>) {
        match step {
            0 => {
                self.discard(cx);
                cx.emit(ImageEvent::Back);
            }
            1 if self.step == Step::Summary => self.back(cx),
            _ => {}
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        if self.submitting || !self.eula {
            return;
        }
        let Some((upload, settings)) = &self.upload else {
            return;
        };
        let read = |input: &Entity<TextInput>| input.read(cx).text().to_string();
        // RAM isn't asked here: the server gets the core's defaults
        let checked = check_settings(&read(&settings.name), &read(&settings.port), "1", "1");
        let (name, port) = match checked {
            Ok((name, port, _, _)) => (name, port),
            Err(errors) => {
                self.errors = errors.into_iter().collect();
                cx.notify();
                return;
            }
        };
        self.errors.clear();
        let request = CreateFromImageRequest {
            upload_id: upload.upload_id.clone(),
            image: upload.summary.image.clone(),
            name,
            port,
            accept_eula: self.eula,
            world: self.world.clone(),
            keep_operators: self.keep_operators,
            flavour: chosen_server(&upload.summary, self.server.as_deref()),
            ..Default::default()
        };
        self.submitting = true;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.create_from_image(&request).await })
                .await;
            this.update(cx, |this, cx| {
                this.submitting = false;
                match result {
                    Ok(_) => {
                        // the core owns the file now, and forgets it once the server is made
                        this.upload = None;
                        cx.emit(ImageEvent::Done)
                    }
                    Err(err) => {
                        let text = tr!(M::ImageCreateFailed, error = err);
                        this.toast(Toast::error(text).key("create-instance"), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push_task(task);
        cx.notify();
    }

    /// The flow on a sample file's summary, for the development screenshots' `--create
    /// image` (a Host's backup) and `--create image-launcher` (a Launcher's snapshot).
    #[cfg(feature = "screenshot")]
    pub fn with_sample(state: Entity<HostState>, launcher: bool, cx: &mut Context<Self>) -> Self {
        let summary = if launcher {
            serde_json::json!({
                "image": "d18e86ae", "file_name": "Survival island.lodeimg",
                "label": "Survival island", "made_by": "launcher", "producer_version": "1.4.0",
                "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10",
                "server": "Fabric", "server_choices": ["Fabric"],
                "mods_from_modrinth": {"names": ["lithium.jar"], "count": 1},
                "carried_mods": {"names": ["my-own-mod.jar"], "count": 1},
                "server_mods": 2,
                "client_only": {"names": ["sodium.jar"], "count": 1},
                "not_downloadable": {"names": ["mods/jade.jar"], "count": 1},
                "worlds": [
                    {"folder": "Island", "name": "Survival island", "size": 182_000_000u64,
                     "last_played": 1_794_000_000},
                    {"folder": "New World", "size": 6_400_000u64, "last_played": 1_793_000_000}
                ],
                "default_world": "Island", "files": 640, "total_bytes": 260_000_000u64,
                "download_bytes": 14_000_000u64, "needed_bytes": 470_000_000u64,
                "disk_free": 182_000_000_000u64,
                "not_taken": ["client_files"], "needs_global_files": true
            })
        } else {
            serde_json::json!({
                "image": "a41f02bc", "file_name": "Survival SMP - 20261112-141000-a1b2.lodeimg",
                "label": "Survival SMP – Before updating to 1.21.4", "made_by": "host",
                "producer_version": "0.6.0", "minecraft": "1.21.3", "loader": "vanilla",
                "flavour": "purpur", "server": "Paper", "server_choices": ["Vanilla", "Paper"],
                "server_note": "Lodestone makes purpur servers as Paper servers, which run the same plugins",
                "carried_mods": {"names": ["EssentialsX.jar", "LuckPerms.jar"], "count": 2},
                "worlds": [{"folder": "world", "size": 1_100_000_000u64}],
                "default_world": "world", "files": 2880, "total_bytes": 1_240_000_000u64,
                "needed_bytes": 2_480_000_000u64, "disk_free": 182_000_000_000u64,
                "left_out": {"names": [".lodestone*", "/eula.txt", "/libraries/", "plugins/*/"],
                             "count": 18},
                "blanked": {"names": ["server.properties: rcon.password"], "count": 1},
                "secrets_not_taken": {"names": ["forwarding.secret"], "count": 1},
                "server_mods": 2,
                "not_taken": ["operators"],
                "operators": {"names": ["Notch", "jeb_"], "count": 2},
                "offline_mode": true, "needs_global_files": true
            })
        };
        let summary: ImageSummary = serde_json::from_value(summary).expect("a sample summary");
        let mut this = Self::new(state, cx);
        this.open_summary(
            ImageUpload {
                upload_id: String::new(),
                summary,
            },
            cx,
        );
        this
    }
}

/// Whether a server can be made from the file `summary` describes by someone who may
/// (`may_write_global`) or may not change global files: not when Lodestone makes no server
/// for it, nor when it carries protected files the person may not add, which the core
/// always refuses whatever world is picked. No room for the default world doesn't stop
/// it: a smaller world may fit, and the core refuses clearly when it doesn't.
fn can_create(summary: &ImageSummary, may_write_global: bool) -> bool {
    summary.unsupported.is_none() && (may_write_global || !summary.needs_global_files)
}

/// The trust warning of a file whose server takes mods or plugins, which run on it: how
/// many, as the Launcher's import says it, and the ones whose bytes the file carries
/// (from whoever made it, not a download). A file with no mods that still carries
/// protected files of its own (jars, programs) gets a warning too.
fn trust_warning(summary: &ImageSummary) -> Option<String> {
    let count = summary.server_mods.max(summary.carried_mods.count);
    if count == 0 {
        return summary
            .needs_global_files
            .then(|| t(M::ImageProtectedWarning).to_string());
    }
    let mut text = trn!(P::ImageModsWarning, count as usize);
    if !summary.carried_mods.is_empty() {
        text.push(' ');
        text.push_str(&tr!(M::ImageCarried, names = summary.carried_mods.joined()));
        if !text.ends_with(['.', '…', '。']) {
            text.push('.');
        }
    }
    Some(text)
}

/// A muted line of text.
fn note(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

impl ImageFlow {
    fn render_file(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let uploading = self.uploading.as_ref().map(|run| run.progress.snapshot());
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(wizard::title(t(M::ImageFileTitle), t(M::ImageFileSubtitle)))
            .child(
                div().flex().flex_row().child(
                    Button::new("image-browse", t(M::ImageBrowse))
                        .variant(ButtonVariant::Primary)
                        .disabled(uploading.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.browse(cx))),
                ),
            )
            .when_some(uploading, |this, snapshot| {
                let share = if snapshot.bytes_total == 0 {
                    0.0
                } else {
                    snapshot.bytes_done as f32 / snapshot.bytes_total as f32
                };
                this.child(loading_line(t(M::ImageUploading))).child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .h(px(4.))
                                .overflow_hidden()
                                .rounded_full()
                                .bg(colors::GRAY_700)
                                .child(
                                    div()
                                        .h_full()
                                        .rounded_full()
                                        .bg(colors::BLUE)
                                        .w(relative(share.clamp(0.02, 1.0))),
                                ),
                        )
                        .child(
                            Button::new("image-upload-cancel", t(CommonMsg::Cancel))
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_upload(cx))),
                        ),
                )
            })
            .when_some(self.error.clone(), |this, error| {
                this.child(error_line(error))
            })
            .into_any_element()
    }

    fn render_summary(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((upload, settings)) = &self.upload else {
            return div().into_any_element();
        };
        let summary = &upload.summary;
        // the game line names the server picked
        let picked = ImageSummary {
            server: self.server.clone().or_else(|| summary.server.clone()),
            ..summary.clone()
        };
        let mut notes = vec![note(format!(
            "{} · {} · {}",
            game_line(&picked),
            trn!(P::ImportFiles, summary.files as usize),
            format::bytes(summary.total_bytes)
        ))];
        notes.push(note(match summary.made_by {
            ImageMaker::Host => tr!(M::ImageMadeByHost, version = summary.producer_version),
            ImageMaker::Launcher => {
                tr!(M::ImageMadeByLauncher, version = summary.producer_version)
            }
            ImageMaker::Unknown => t(M::ImageMadeByOther).to_string(),
        }));
        if let Some(server_note) = server_note(summary, self.server.as_deref()) {
            notes.push(note(server_note));
        }
        if let Some(base) = &summary.base {
            notes.push(note(base.clone()));
        }

        let mut alerts = Vec::new();
        if let Some(why) = unsupported(summary) {
            alerts.push(parts::error(why));
        }
        if let Some(warning) = trust_warning(summary) {
            alerts.push(parts::warning(warning));
        }
        if !summary.not_downloadable.is_empty() {
            alerts.push(parts::warning(tr!(
                M::ImageNotDownloadable,
                names = summary.not_downloadable.joined()
            )));
        }
        if summary.offline_mode {
            alerts.push(parts::warning(t(M::ImageOfflineMode)));
        }
        let may_write_global = self
            .state
            .read(cx)
            .me
            .may_core(CorePermission::WriteGlobalFile);
        if summary.needs_global_files && !may_write_global {
            alerts.push(parts::error(t(M::ImageNeedsGlobal)));
        }

        let mut details = Vec::new();
        if summary.download_bytes > 0 {
            let size = format::bytes(summary.download_bytes);
            details.push(note(if summary.mods_from_modrinth.is_empty() {
                tr!(M::ImageDownloads, size = size)
            } else {
                tr!(
                    M::ImageDownloadsMods,
                    size = size,
                    names = summary.mods_from_modrinth.joined()
                )
            }));
        }
        for (list, message) in [
            (&summary.client_only, M::ImageClientOnly),
            (&summary.left_out, M::ImageTakenOut),
            (&summary.withheld, M::ImageWithheld),
        ] {
            if !list.is_empty() {
                details.push(note(tr!(message, names = list.joined())));
            }
        }
        for (list, message) in [
            (&summary.blanked, M::ImageBlanked),
            (&summary.secrets_not_taken, M::ImageSecretsNotTaken),
        ] {
            if !list.is_empty() {
                details.push(note(tr!(message, names = list.joined())));
            }
        }
        // the operators' tickbox below says itself whether they are taken
        let what = not_taken(
            &summary
                .not_taken
                .iter()
                .filter(|n| !(self.keep_operators && **n == NotTaken::Operators))
                .copied()
                .collect::<Vec<_>>(),
        );
        if !what.is_empty() {
            details.push(note(tr!(M::ImageNotTaken, what = what)));
        }
        // for the world picked: the core checks again
        let world_picked = self.world.as_deref();
        if let Some(free) = summary.disk_free {
            details.push(note(tr!(
                M::ImageDisk,
                needed = format::bytes(
                    summary
                        .needed_bytes_for(world_picked)
                        .saturating_add(ImageSummary::SPARE_BYTES)
                ),
                free = format::bytes(free)
            )));
        }
        if !summary.has_room_for(world_picked) {
            alerts.push(parts::warning(t(M::ImageNoRoom)));
        }

        // the world: a Launcher's file gives only the one picked, or none
        let world = (!summary.worlds.is_empty()).then(|| {
            let mut rows: Vec<AnyElement> = summary
                .worlds
                .iter()
                .enumerate()
                .map(|(i, world)| {
                    let folder = world.folder.clone();
                    let label = match &world.name {
                        Some(_) => format!("{} ({})", world.shown_name(), world.shown_folder()),
                        None => world.shown_folder().to_string(),
                    };
                    ListRow::new(SharedString::from(format!("image-world-{i}")), label)
                        .detail(format::bytes(world.size))
                        .selected(self.world.as_deref() == Some(folder.as_str()))
                        .disabled(self.submitting)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.world = Some(folder.clone());
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .collect();
            if summary.made_by != ImageMaker::Host {
                rows.push(
                    ListRow::new("image-world-none", t(M::ImageNoWorld))
                        .selected(self.world.is_none())
                        .disabled(self.submitting)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.world = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            fields::row(
                t(M::ImageWorld),
                t(M::ImageWorldHint),
                false,
                div().flex().flex_col().children(rows).into_any_element(),
                None,
            )
        });

        // the server software, when the file's game allows a choice (vanilla or Paper)
        let software = (summary.server_choices.len() > 1).then(|| {
            let rows: Vec<AnyElement> = summary
                .server_choices
                .iter()
                .map(|choice| {
                    let picked = choice.clone();
                    ListRow::new(
                        SharedString::from(format!("image-server-{}", choice.to_lowercase())),
                        choice.clone(),
                    )
                    .selected(self.server.as_deref() == Some(choice.as_str()))
                    .disabled(self.submitting)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.server = Some(picked.clone());
                        cx.notify();
                    }))
                    .into_any_element()
                })
                .collect();
            fields::row(
                t(M::ImageServerSoftware),
                t(M::ImageServerSoftwareHint),
                false,
                div().flex().flex_col().children(rows).into_any_element(),
                None,
            )
        });

        let operators = (!summary.operators.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(16.))
                .py(px(12.))
                .child(
                    Checkbox::new(
                        "image-keep-operators",
                        self.keep_operators,
                        tr!(M::ImageKeepOperators, names = summary.operators.joined()),
                    )
                    .disabled(self.submitting)
                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                        this.keep_operators = *value;
                        cx.notify();
                    })),
                )
                .child(
                    div()
                        .pl(px(24.))
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .child(t(M::ImageKeepOperatorsHint)),
                )
                .into_any_element()
        });

        let error = |key: &str| self.errors.get(key).cloned();
        let text_row = |name: M, description: M, input: &Entity<TextInput>, key: &str| {
            fields::row(
                t(name),
                t(description),
                false,
                input.clone().into_any_element(),
                error(key),
            )
        };
        div()
            .id("image-summary")
            .size_full()
            .overflow_y_scroll()
            .pr(px(4.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(wizard::title(
                t(M::ImageSummaryTitle),
                t(M::ImageSummarySubtitle),
            ))
            .child(
                div()
                    .mt(px(-12.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(notes),
            )
            .children(alerts)
            .child(div().flex().flex_col().gap(px(4.)).children(details))
            .child(fields::section(
                software.into_iter().chain(world).chain(operators).collect(),
            ))
            .child(fields::section(vec![
                text_row(
                    M::FieldName,
                    M::FieldNameDescription,
                    &settings.name,
                    "name",
                ),
                text_row(
                    M::FieldPort,
                    M::FieldPortDescription,
                    &settings.port,
                    "port",
                ),
            ]))
            .child(eula::eula_box(
                self.eula,
                self.submitting,
                cx.listener(|this, value: &bool, _, cx| {
                    this.eula = *value;
                    cx.notify();
                }),
            ))
            .into_any_element()
    }
}

impl Render for ImageFlow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let steps = [
            t(M::StepSelectGame),
            t(M::StepChooseFile),
            t(M::StepImageSummary),
        ];
        let current = match self.step {
            Step::File => 1,
            Step::Summary => 2,
        };
        let weak = cx.entity().downgrade();
        let on_select: Option<wizard::StepHandler> = (!self.submitting).then(|| {
            std::rc::Rc::new(move |i: usize, _: &mut Window, cx: &mut gpui::App| {
                weak.update(cx, |this, cx| this.go_to(i, cx)).ok();
            }) as wizard::StepHandler
        });
        let rail = wizard::rail(&steps, current, |_| true, on_select);
        let body = match self.step {
            Step::File => self.render_file(cx),
            Step::Summary => self.render_summary(cx),
        };
        let may_write_global = self
            .state
            .read(cx)
            .me
            .may_core(CorePermission::WriteGlobalFile);
        let can_create = self
            .upload
            .as_ref()
            .is_some_and(|(upload, _)| can_create(&upload.summary, may_write_global));
        let next: Option<AnyElement> = (self.step == Step::Summary).then(|| {
            Button::new("image-create", t(M::ImageCreate))
                .variant(ButtonVariant::Primary)
                .loading(self.submitting)
                .disabled(self.submitting || !self.eula || !can_create)
                .on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                .into_any_element()
        });
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(colors::GRAY_850)
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(32.))
                    .pt(px(40.))
                    .child(div().flex_1().min_h_0().child(body))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .pt(px(20.))
                            .child(
                                Button::new("image-back", t(CommonMsg::Back))
                                    .disabled(self.submitting)
                                    .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                            )
                            .children(next),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::images::ImageWorld;

    fn summary(json: serde_json::Value) -> ImageSummary {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn a_server_is_named_after_the_file() {
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "Survival \\u{202E}SMP", "file_name": "x.lodeimg"
            }))),
            "Survival SMP"
        );
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "  ", "file_name": "Island.lodeimg"
            }))),
            "Island"
        );
        let long = "a".repeat(100);
        assert_eq!(
            default_name(&summary(serde_json::json!({ "label": long })))
                .chars()
                .count(),
            64
        );
        // an unnamed backup's label has the time, and a name may have any character: the
        // name made is one the core takes
        let name = default_name(&summary(serde_json::json!({
            "label": "Survival – 2026-11-12 14:10 UTC"
        })));
        assert_eq!(name, "Survival – 2026-11-12 14.10 UTC");
        let name = default_name(&summary(serde_json::json!({
            "label": "Before 1/2 \"big\" update? <a|b> *\\"
        })));
        assert!(!name.contains(NAME_FORBIDDEN), "{name}");
        assert!(check_settings(&name, "25565", "1", "1").is_ok(), "{name}");
    }

    #[test]
    fn the_files_own_world_is_picked() {
        let world = |folder: &str| ImageWorld {
            folder: folder.into(),
            ..Default::default()
        };
        let mut s = ImageSummary {
            worlds: vec![world("A"), world("B")],
            default_world: Some("B".into()),
            ..Default::default()
        };
        assert_eq!(default_world(&s).as_deref(), Some("B"));
        s.default_world = Some("gone".into());
        assert_eq!(default_world(&s).as_deref(), Some("A"));
        s.worlds.clear();
        assert_eq!(default_world(&s), None);
    }

    #[test]
    fn server_software_is_asked_for_only_when_there_is_a_choice() {
        let vanilla = summary(serde_json::json!({
            "server": "Vanilla", "server_choices": ["Vanilla", "Paper"]
        }));
        assert_eq!(
            chosen_server(&vanilla, Some("Paper")).as_deref(),
            Some("Paper")
        );
        assert_eq!(chosen_server(&vanilla, Some("Fabric")), None);
        assert_eq!(chosen_server(&vanilla, None), None);
        let fabric = summary(serde_json::json!({
            "server": "Fabric", "server_choices": ["Fabric"]
        }));
        assert_eq!(chosen_server(&fabric, Some("Fabric")), None);
    }

    #[test]
    fn notes_on_the_server_are_the_hosts_own_words() {
        crate::i18n::force(crate::i18n::Locale::En);
        let purpur = summary(serde_json::json!({
            "loader": "vanilla", "flavour": "purpur", "server": "Paper",
            "server_choices": ["Vanilla", "Paper"],
            "server_note": "the core's English"
        }));
        assert_eq!(
            server_note(&purpur, None).as_deref(),
            Some("Lodestone makes Purpur servers as Paper servers, which run the same plugins.")
        );
        assert_eq!(
            server_note(&purpur, Some("Vanilla")),
            None,
            "not once Vanilla is picked"
        );
        let paper = summary(serde_json::json!({ "flavour": "paper", "server": "Paper" }));
        assert_eq!(server_note(&paper, None), None);
        let quilt = summary(serde_json::json!({
            "loader": "quilt", "unsupported": "Lodestone can't make Quilt servers"
        }));
        assert_eq!(
            unsupported(&quilt).as_deref(),
            Some("Lodestone can’t make Quilt servers.")
        );
        assert_eq!(unsupported(&paper), None);
    }

    #[test]
    fn mods_from_anywhere_get_the_trust_warning() {
        crate::i18n::force(crate::i18n::Locale::En);
        let downloaded = summary(serde_json::json!({
            "server_mods": 40,
            "mods_from_modrinth": {"names": ["lithium.jar"], "count": 40}
        }));
        assert_eq!(
            trust_warning(&downloaded).as_deref(),
            Some(
                "It has 40 mods or plugins: programs that run on the server. Only use files from people you trust."
            )
        );
        let carried = summary(serde_json::json!({
            "server_mods": 1, "carried_mods": {"names": ["own.jar"], "count": 1}
        }));
        assert_eq!(
            trust_warning(&carried).as_deref(),
            Some(
                "It has 1 mod or plugin: a program that runs on the server. Only use files from people you trust. These are in the file itself, not downloaded from Modrinth: own.jar."
            )
        );
        assert_eq!(trust_warning(&summary(serde_json::json!({}))), None);
        // no mods, but protected files of its own: warned too
        let programs = summary(serde_json::json!({ "needs_global_files": true }));
        assert_eq!(
            trust_warning(&programs).as_deref(),
            Some(t(M::ImageProtectedWarning))
        );
    }

    #[test]
    fn a_file_the_core_would_refuse_cant_be_created() {
        let fine = summary(serde_json::json!({ "server": "Fabric" }));
        assert!(can_create(&fine, false));
        let jars = summary(serde_json::json!({ "server": "Fabric", "needs_global_files": true }));
        assert!(can_create(&jars, true));
        assert!(!can_create(&jars, false));
        let quilt = summary(serde_json::json!({ "unsupported": "Quilt" }));
        assert!(!can_create(&quilt, true));
    }

    #[test]
    fn the_game_reads_in_one_line() {
        crate::i18n::force(crate::i18n::Locale::En);
        let fabric = summary(serde_json::json!({
            "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10",
            "server": "Fabric"
        }));
        assert_eq!(game_line(&fabric), "Fabric 0.16.10 · Minecraft 1.21.1");
        let paper = summary(serde_json::json!({
            "minecraft": "1.21.3", "loader": "vanilla", "flavour": "purpur", "server": "Paper"
        }));
        assert_eq!(game_line(&paper), "Paper · Minecraft 1.21.3");
        assert_eq!(
            not_taken(&[NotTaken::Eula, NotTaken::Unknown, NotTaken::Operators]),
            "the EULA, the operators"
        );
    }
}
