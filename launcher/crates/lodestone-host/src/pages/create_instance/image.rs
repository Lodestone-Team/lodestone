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
    CreateFromImageRequest, ImageMaker, ImageUpload, is_image_file,
};
use launcher_core::cores::admin::models::CorePermission;
use launcher_core::game::download::{CancelToken, Progress};
use lodestone_ui::sharing::{
    Picked, SummaryView, can_create, chosen_server, default_name, default_world, summary_lines,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Checkbox, CountBadge, Dropdown, Icon, Kind, ListRow, ObjectChip,
        TextInput, Toast,
        a11y::Accessible as _,
        settings::{popover_panel, select_button},
    },
    file_dialog::{Pick, ShowsError},
    theme::{colors, ink, px, text},
};

use super::{
    error_line, eula, fields, loading_line,
    modpack::{check_settings, first_free_port},
    wizard,
};
use crate::{
    i18n::{CommonMsg, InstancesMsg as M, SharingMsg as S, format, t, tr},
    state::HostState,
};

/// What the flow tells the dialog.
pub enum ImageEvent {
    /// Back to the game selection.
    Back,
    /// The server is being made: close the dialog.
    Done,
}

/// The summary's pickers, one open at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    World,
    Server,
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
    /// The file picked that isn't a `.lodeimg`, shown after the error by its name.
    rejected: Option<String>,
    /// The file the core holds, and the settings made for it.
    upload: Option<(ImageUpload, Settings)>,
    /// The world (its folder) that becomes the server's.
    world: Option<String>,
    /// The server software picked, one of the summary's choices.
    server: Option<String>,
    /// The picker whose list is open.
    menu: Option<Menu>,
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

impl ImageFlow {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            step: Step::File,
            uploading: None,
            error: None,
            rejected: None,
            upload: None,
            world: None,
            menu: None,
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
        self.rejected = None;
        if !is_image_file(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.error = Some(t(M::ImageNotLodeimg).to_string());
            self.rejected = Some(name);
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

    /// Opens the world list, for the development screenshots' `--create image-launcher-world`.
    #[cfg(feature = "screenshot")]
    pub fn open_world_menu_for_screenshot(&mut self) {
        self.menu = Some(Menu::World);
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
        let summary: launcher_core::cores::admin::images::ImageSummary =
            serde_json::from_value(summary).expect("a sample summary");
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

impl ImageFlow {
    fn render_file(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let uploading = self.uploading.as_ref().map(|run| run.progress.snapshot());
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(wizard::title(
                t(M::ImageFileTitle),
                Some(t(M::ImageFileSubtitle)),
            ))
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
            // the file picked, by its exact name
            .when_some(self.rejected.clone(), |this, name| {
                this.child(
                    div().flex().child(
                        ObjectChip::new(name)
                            .kind(Kind::File)
                            .mono()
                            .id("image-rejected-file"),
                    ),
                )
            })
            .into_any_element()
    }

    /// A button showing the current choice, which opens the list of choices.
    #[allow(clippy::too_many_arguments)]
    fn picker(
        &self,
        menu: Menu,
        id: &'static str,
        label: SharedString,
        options: Vec<(Option<String>, SharedString, Option<SharedString>)>,
        current: Option<String>,
        apply: fn(&mut Self, Option<String>),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu == Some(menu) && !self.submitting;
        let shown = options
            .iter()
            .find(|(value, _, _)| *value == current)
            .map(|(_, name, _)| name.clone())
            .unwrap_or_default();
        let page = cx.entity().downgrade();
        let mut button = select_button(
            SharedString::from(format!("{id}-select")),
            div().truncate().child(shown.clone()),
        )
        .aria_label(label)
        .aria_value(shown)
        .w_full()
        .justify_between();
        if !self.submitting {
            button = button
                .expandable(open, move |_, cx| {
                    page.update(cx, |this, cx| this.toggle_menu(menu, cx)).ok();
                })
                .on_press(cx.listener(move |this, _, _, cx| this.toggle_menu(menu, cx)));
        }
        Dropdown::new(button)
            .when(open, |this| {
                let mut panel =
                    popover_panel(SharedString::from(format!("{id}-list"))).min_w(px(240.));
                for (index, (value, name, detail)) in options.into_iter().enumerate() {
                    let selected = value == current;
                    let mut row =
                        ListRow::new(SharedString::from(format!("{id}-option-{index}")), name)
                            .selected(selected);
                    if let Some(detail) = detail {
                        row = row.detail(detail);
                    }
                    panel = panel.child(row.on_click(cx.listener(move |this, _, _, cx| {
                        apply(this, value.clone());
                        this.menu = None;
                        cx.notify();
                    })));
                }
                let page = cx.entity().downgrade();
                this.popover(
                    SharedString::from(format!("{id}-menu")),
                    panel,
                    move |_, cx| {
                        page.update(cx, |this, cx| {
                            this.menu = None;
                            cx.notify();
                        })
                        .ok();
                    },
                )
            })
            .into_any_element()
    }

    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        cx.notify();
    }

    fn render_summary(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((upload, settings)) = &self.upload else {
            return div().into_any_element();
        };
        let summary = &upload.summary;
        let may_write_global = self
            .state
            .read(cx)
            .me
            .may_core(CorePermission::WriteGlobalFile);
        let lines = summary_lines(
            summary,
            Picked {
                world: self.world.as_deref(),
                server: self.server.as_deref(),
                keep_operators: self.keep_operators,
                may_write_global,
                own_file: false,
            },
        );
        // what the file holds, laid out as the Launcher's "Host this" lays it out
        let view = SummaryView::new("image-summary-lines", lines);

        // the world: a Launcher's file gives only the one picked, or none
        let world = (!summary.worlds.is_empty()).then(|| {
            let mut options: Vec<(Option<String>, SharedString, Option<SharedString>)> = summary
                .worlds
                .iter()
                .map(|world| {
                    // its name; the folder and the size are the row's detail
                    let label = world.shown_name().to_string();
                    (
                        Some(world.folder.clone()),
                        label.into(),
                        Some(format::bytes(world.size).into()),
                    )
                })
                .collect();
            if summary.made_by != ImageMaker::Host {
                options.push((None, t(S::ImageNoWorld).into(), None));
            }
            let current = self.world.clone();
            fields::row(
                t(S::ImageWorld),
                Some(t(S::ImageWorldHint).into()),
                false,
                self.picker(
                    Menu::World,
                    "image-world",
                    t(S::ImageWorld).into(),
                    options,
                    current,
                    |this, value| this.world = value,
                    cx,
                ),
                None,
            )
        });

        // the server software, when the file's game allows a choice (vanilla or Paper)
        let software = (summary.server_choices.len() > 1).then(|| {
            let options = summary
                .server_choices
                .iter()
                .map(|choice| {
                    (
                        Some(choice.clone()),
                        SharedString::from(choice.clone()),
                        None,
                    )
                })
                .collect();
            let current = self.server.clone();
            fields::row(
                t(S::ImageServerSoftware),
                Some(t(S::ImageServerSoftwareHint).into()),
                false,
                self.picker(
                    Menu::Server,
                    "image-server",
                    t(S::ImageServerSoftware).into(),
                    options,
                    current,
                    |this, value| this.server = value,
                    cx,
                ),
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
                        t(M::ImageKeepOperators),
                    )
                    .disabled(self.submitting)
                    .on_change(cx.listener(|this, value: &bool, _, cx| {
                        this.keep_operators = *value;
                        cx.notify();
                    })),
                )
                // the operators, a chip each, not inside the box's label
                .child(
                    div()
                        .pl(px(24.))
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(summary.operators.names.iter().enumerate().map(
                            |(index, name)| {
                                ObjectChip::new(
                                    launcher_core::images::display_text(name).into_owned(),
                                )
                                .icon(Icon::User)
                                .mono()
                                .id(SharedString::from(format!("image-operator-{index}")))
                            },
                        ))
                        .when(
                            summary.operators.count > summary.operators.names.len() as u64,
                            |this| {
                                this.child(CountBadge::new(
                                    summary.operators.count - summary.operators.names.len() as u64,
                                ))
                            },
                        ),
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
        let text_row = |name: M, input: &Entity<TextInput>, key: &str| {
            fields::row(
                t(name),
                None,
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
            .child(wizard::title(t(M::ImageSummaryTitle), None))
            .child(view)
            .child(fields::section(
                software.into_iter().chain(world).chain(operators).collect(),
            ))
            .child(fields::section(vec![
                text_row(M::FieldName, &settings.name, "name"),
                text_row(M::FieldPort, &settings.port, "port"),
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
