//! Making a server from a Modrinth modpack (`ModpackCreateForm.tsx`): pick a pack
//! (search, link or an uploaded `.mrpack`), pick a version, which the core downloads
//! to read, then the server's settings. The core makes the server in the background.

use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, ScrollHandle,
    SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        instances::{CreateModpackRequest, ModpackSource},
        mods::{
            ModpackInfo, ModrinthProject, ModrinthProjectType, ModrinthSearchHit, ModrinthVersion,
            ModrinthVersionType,
        },
    },
};
use lodestone_ui::{
    components::{Button, ButtonSize, ButtonVariant, Icon, LoadingDots, TextInput, Toast, Toggle},
    file_dialog::{Pick, ShowsError},
    theme::{colors, ink, px, radius, text, weights},
};

use super::{
    Load, eula, fields,
    modrinth_search::{ModrinthPick, ModrinthSearchEvent, ModrinthSearchView, project_icon},
    wizard,
};
use crate::{
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, t, tr, trn},
    instance_icons,
    state::HostState,
};

/// What the flow tells the dialog.
pub enum ModpackEvent {
    /// Back to the game selection.
    Back,
    /// The server is being made: close the dialog.
    Done,
}

/// A modpack picked elsewhere (Mods › Browse: "Create server from this modpack").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModpackPreset {
    pub project_id: String,
    pub version_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Source,
    Version,
    Settings,
}

const VERSIONS_PAGE: usize = 20;

/// The settings step's fields.
struct Settings {
    name: Entity<TextInput>,
    description: Entity<TextInput>,
    port: Entity<TextInput>,
    min_ram: Entity<TextInput>,
    max_ram: Entity<TextInput>,
    auto_start: bool,
    restart_on_crash: bool,
}

pub struct ModpackFlow {
    state: Entity<HostState>,
    preset: Option<ModpackPreset>,
    step: Step,
    search: Entity<ModrinthSearchView>,
    source_scroll: ScrollHandle,
    project_id: Option<String>,
    picked_hit: Option<ModrinthSearchHit>,
    project: Option<Load<ModrinthProject>>,
    versions: Option<Load<Vec<ModrinthVersion>>>,
    shown: usize,
    info: Option<ModpackInfo>,
    source: Option<ModpackSource>,
    /// The version being read, and which request is the latest.
    inspecting: Option<String>,
    inspect_request: u64,
    uploading: Option<String>,
    settings: Option<(ModpackSource, Settings)>,
    errors: std::collections::HashMap<&'static str, String>,
    eula: bool,
    submitting: bool,
    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<ModpackEvent> for ModpackFlow {}

impl ShowsError for ModpackFlow {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast(Toast::error(message), cx);
    }
}

/// The first port from 25565 that no instance uses (`firstFreePort`).
pub fn first_free_port(used: &[u32]) -> u32 {
    let mut port = 25565;
    while used.contains(&port) {
        port += 1;
    }
    port
}

/// A name, port, minimum and maximum RAM that passed the checks.
pub type CheckedSettings = (String, u32, u32, u32);

/// The settings step's checks (the dashboard's `settingsSchema`): the errors by field.
pub fn check_settings(
    name: &str,
    port: &str,
    min_ram: &str,
    max_ram: &str,
) -> Result<CheckedSettings, Vec<(&'static str, String)>> {
    let mut errors = Vec::new();
    let integer = |field: &'static str,
                   label: M,
                   text: &str,
                   min: u64,
                   max: u64,
                   errors: &mut Vec<(&'static str, String)>| {
        let name = t(label);
        let text = text.trim();
        if text.is_empty() {
            errors.push((field, tr!(M::ValidationRequired, name = name)));
            return None;
        }
        let Ok(value) = text.parse::<f64>() else {
            errors.push((field, tr!(M::ValidationNumber, name = name)));
            return None;
        };
        if value.fract() != 0.0 {
            errors.push((field, tr!(M::ValidationInteger, name = name)));
            return None;
        }
        if value < min as f64 {
            errors.push((field, tr!(M::ValidationMin, name = name, min = min)));
            return None;
        }
        if value > max as f64 {
            errors.push((field, tr!(M::ValidationMax, name = name, max = max)));
            return None;
        }
        Some(value as u32)
    };
    let name = name.trim().to_string();
    if name.is_empty() {
        errors.push(("name", tr!(M::ValidationRequired, name = t(M::FieldName))));
    }
    let port = integer("port", M::FieldPort, port, 1, 65535, &mut errors);
    let min = integer(
        "min_ram",
        M::FieldMinRam,
        min_ram,
        1,
        u32::MAX as u64,
        &mut errors,
    );
    let max = integer(
        "max_ram",
        M::FieldMaxRam,
        max_ram,
        1,
        u32::MAX as u64,
        &mut errors,
    );
    if let (Some(min), Some(max)) = (min, max)
        && max < min
    {
        errors.push(("max_ram", t(M::MaxRamBelowMin).to_string()));
    }
    match (port, min, max) {
        (Some(port), Some(min), Some(max)) if errors.is_empty() => Ok((name, port, min, max)),
        _ => Err(errors),
    }
}

impl ModpackFlow {
    pub fn with_preset(
        state: Entity<HostState>,
        preset: Option<ModpackPreset>,
        cx: &mut Context<Self>,
    ) -> Self {
        let admin = state.read(cx).admin.clone();
        let search = cx.new(|cx| {
            ModrinthSearchView::new(
                admin,
                ModrinthProjectType::Modpack,
                t(M::ModpackSearchPlaceholder),
                cx,
            )
            .with_loader_filter("fabric", t(M::ModpackOnlyFabric), cx)
        });
        let subscription = cx.subscribe(&search, |this, _, event, cx| match event {
            ModrinthSearchEvent::Picked(pick) => this.pick(pick.clone(), cx),
        });
        let mut this = Self {
            state,
            preset: preset.clone(),
            step: Step::Source,
            search,
            source_scroll: ScrollHandle::new(),
            project_id: None,
            picked_hit: None,
            project: None,
            versions: None,
            shown: VERSIONS_PAGE,
            info: None,
            source: None,
            inspecting: None,
            inspect_request: 0,
            uploading: None,
            settings: None,
            errors: Default::default(),
            eula: false,
            submitting: false,
            _subscriptions: vec![subscription],
            _tasks: Vec::new(),
        };
        if let Some(preset) = preset {
            this.open_project(preset.project_id.clone(), None, cx);
            if let Some(version) = preset.version_id {
                this.inspect(version, cx);
            }
        }
        this
    }

    fn admin(&self, cx: &Context<Self>) -> launcher_core::cores::admin::AdminClient {
        self.state.read(cx).admin.clone()
    }

    fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    fn pick(&mut self, pick: ModrinthPick, cx: &mut Context<Self>) {
        if pick.project_type != "modpack" {
            let text = tr!(M::ModpackNotAModpack, r#type = pick.project_type);
            return self.toast(Toast::error(text).key("modpack-not-a-modpack"), cx);
        }
        self.info = None;
        self.source = None;
        self.open_project(pick.project_id, pick.hit, cx);
        if let Some(version) = pick.version_id {
            self.inspect(version, cx);
        }
    }

    /// Shows a project's versions.
    /// For screenshots: opens the search's sort list.
    #[cfg(feature = "screenshot")]
    pub fn open_sort_for_screenshot(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| {
            search.sort_open = true;
            cx.notify();
        });
    }

    fn open_project(&mut self, id: String, hit: Option<ModrinthSearchHit>, cx: &mut Context<Self>) {
        self.project_id = Some(id.clone());
        self.picked_hit = hit;
        self.project = Some(Load::Loading);
        self.versions = Some(Load::Loading);
        self.shown = VERSIONS_PAGE;
        self.step = Step::Version;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let (project, versions) = cx
                .background_spawn(async move {
                    let project = admin.modrinth_project(&id).await;
                    let versions = admin.modrinth_versions(&id, None, None).await;
                    (project, versions)
                })
                .await;
            this.update(cx, |this, cx| {
                this.project = Some(match project {
                    Ok(project) => Load::Ready(project),
                    Err(err) => Load::Failed(err.to_string()),
                });
                this.versions = Some(match versions {
                    Ok(versions) => Load::Ready(versions),
                    Err(err) => Load::Failed(err.to_string()),
                });
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    /// Asks the core to download a version and say what's in it. Only the latest
    /// request counts.
    fn inspect(&mut self, version_id: String, cx: &mut Context<Self>) {
        self.inspect_request += 1;
        let request = self.inspect_request;
        self.inspecting = Some(version_id.clone());
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let id = version_id.clone();
            let info = cx
                .background_spawn(async move { admin.inspect_modpack(&id).await })
                .await;
            this.update(cx, |this, cx| {
                if request != this.inspect_request {
                    return;
                }
                this.inspecting = None;
                match info {
                    Ok(info) => {
                        this.info = Some(info);
                        this.source = Some(ModpackSource::Modrinth { version_id });
                    }
                    Err(err) => {
                        let text = tr!(M::ModpackInspectFailed, error = err);
                        this.toast(Toast::error(text), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    fn cancel_inspect(&mut self) {
        self.inspect_request += 1;
        self.inspecting = None;
    }

    fn upload(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(Pick::Files.options(false, t(M::ModpackUpload)));
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let Some(paths) = lodestone_ui::file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !name.to_lowercase().ends_with(".mrpack") {
                this.update(cx, |this, cx| {
                    this.toast(Toast::error(t(M::ModpackChooseMrpack)), cx)
                })
                .ok();
                return;
            }
            this.update(cx, |this, cx| {
                this.uploading = Some(name.clone());
                cx.notify();
            })
            .ok();
            let file_name = name.clone();
            let result = cx
                .background_spawn(async move {
                    let bytes =
                        std::fs::read(&path).map_err(|err| CoreError::Invalid(err.to_string()))?;
                    admin.upload_modpack(&file_name, &bytes).await
                })
                .await;
            this.update(cx, |this, cx| {
                this.uploading = None;
                match result {
                    Ok(info) => match info.upload_id.clone() {
                        Some(upload_id) => {
                            this.cancel_inspect();
                            this.project_id = None;
                            this.picked_hit = None;
                            this.project = None;
                            this.versions = None;
                            this.info = Some(info);
                            this.source = Some(ModpackSource::Upload { upload_id });
                            this.step = Step::Version;
                        }
                        None => this.toast(
                            Toast::error(tr!(
                                M::ModpackUploadFailed,
                                error = t(M::ModpackNoUploadId)
                            )),
                            cx,
                        ),
                    },
                    Err(err) => {
                        let text = tr!(M::ModpackUploadFailed, error = err);
                        this.toast(Toast::error(text), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
    }

    fn can_continue(&self) -> bool {
        self.info
            .as_ref()
            .is_some_and(|info| info.unsupported_reason.is_none())
            && self.source.is_some()
    }

    fn back_to_source(&mut self, cx: &mut Context<Self>) {
        self.cancel_inspect();
        self.step = Step::Source;
        self.info = None;
        self.source = None;
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Settings => {
                self.step = Step::Version;
                cx.notify();
            }
            Step::Version if self.preset.is_none() => self.back_to_source(cx),
            _ => cx.emit(ModpackEvent::Back),
        }
    }

    fn go_to(&mut self, step: usize, cx: &mut Context<Self>) {
        match step {
            0 => cx.emit(ModpackEvent::Back),
            1 if self.preset.is_none() && self.step != Step::Source => self.back_to_source(cx),
            2 if self.step == Step::Settings => {
                self.step = Step::Version;
                cx.notify();
            }
            _ => {}
        }
    }

    /// On to the settings, made for the pack the first time.
    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if !self.can_continue() || self.inspecting.is_some() || self.uploading.is_some() {
            return;
        }
        let (Some(info), Some(source)) = (self.info.clone(), self.source.clone()) else {
            return;
        };
        if self.settings.as_ref().map(|(s, _)| s) != Some(&source) {
            let used: Vec<u32> = self
                .state
                .read(cx)
                .instances
                .iter()
                .flatten()
                .map(|i| i.port)
                .collect();
            let input = |text: String, label: M, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    TextInput::new(cx)
                        .with_text(SharedString::from(text))
                        .with_label(label)
                })
            };
            let settings = Settings {
                name: input(info.name.clone(), M::FieldName, cx),
                description: input(
                    info.summary.clone().unwrap_or_default(),
                    M::FieldDescription,
                    cx,
                ),
                port: input(first_free_port(&used).to_string(), M::FieldPort, cx),
                min_ram: input("1024".into(), M::FieldMinRam, cx),
                max_ram: input("4096".into(), M::FieldMaxRam, cx),
                auto_start: false,
                restart_on_crash: false,
            };
            self.settings = Some((source, settings));
        }
        self.errors.clear();
        self.step = Step::Settings;
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        if self.submitting || !self.eula {
            return;
        }
        let Some((source, settings)) = &self.settings else {
            return;
        };
        let read = |input: &Entity<TextInput>| input.read(cx).text().to_string();
        let checked = check_settings(
            &read(&settings.name),
            &read(&settings.port),
            &read(&settings.min_ram),
            &read(&settings.max_ram),
        );
        let (name, port, min_ram, max_ram) = match checked {
            Ok(values) => values,
            Err(errors) => {
                self.errors = errors.into_iter().collect();
                cx.notify();
                return;
            }
        };
        self.errors.clear();
        let request = CreateModpackRequest {
            source: source.clone(),
            name,
            port,
            description: read(&settings.description),
            min_ram,
            max_ram,
            auto_start: settings.auto_start,
            restart_on_crash: settings.restart_on_crash,
            // the user ticked the box themselves
            accept_eula: self.eula,
        };
        self.submitting = true;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.create_modpack_instance(&request).await })
                .await;
            this.update(cx, |this, cx| {
                this.submitting = false;
                match result {
                    Ok(_) => cx.emit(ModpackEvent::Done),
                    Err(err) => {
                        let text = tr!(M::CreateFailed, error = err);
                        this.toast(Toast::error(text).key("create-instance"), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }
}

fn version_type_label(kind: ModrinthVersionType) -> Option<AnyElement> {
    let (label, color) = match kind {
        ModrinthVersionType::Release => (t(M::VersionRelease), colors::GREEN),
        ModrinthVersionType::Beta => (t(M::VersionBeta), colors::YELLOW),
        ModrinthVersionType::Alpha => (t(M::VersionAlpha), colors::RED_200),
        ModrinthVersionType::Unknown => return None,
    };
    Some(
        div()
            .flex_none()
            .px(px(6.))
            .rounded(radius::DEFAULT)
            .bg(color.opacity(15))
            .text_size(text::CAPTION)
            .font_weight(weights::BOLD.face())
            .text_color(color)
            .child(label)
            .into_any_element(),
    )
}

/// An ISO date as a short date in the user's language.
fn short_date(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|date| lodestone_ui::i18n::format::short_date(date.date_naive(), true))
        .unwrap_or_else(|_| iso.chars().take(10).collect())
}

/// What the core found in the pack (`ModpackSummary`).
fn summary(info: &ModpackInfo) -> AnyElement {
    let with = |a: &str, b: &Option<String>| match b {
        Some(b) => format!("{a} {b}"),
        None => a.to_string(),
    };
    let rows = [
        (
            t(M::ModpackSummaryModpack).to_string(),
            with(&info.name, &info.version_number),
        ),
        ("Minecraft".to_string(), info.game_version.clone()),
        (
            t(M::ModpackSummaryLoader).to_string(),
            with(&info.loader, &info.loader_version),
        ),
        (
            t(M::ModpackSummaryServerFiles).to_string(),
            tr!(
                M::ModpackSummaryServerFilesValue,
                count = info.server_file_count,
                size = lodestone_ui::i18n::format::bytes(info.server_download_size)
            ),
        ),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(fields::section(
            rows.into_iter()
                .map(|(label, value)| {
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .gap(px(16.))
                        .px(px(16.))
                        .py(px(8.))
                        .text_size(text::MEDIUM)
                        .child(div().flex_none().text_color(ink::muted()).child(label))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(ink::text())
                                .child(value),
                        )
                        .into_any_element()
                })
                .collect(),
        ))
        .when_some(info.summary.clone(), |this, summary| {
            this.child(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .line_clamp(2)
                    .child(summary),
            )
        })
        .when_some(info.unsupported_reason.clone(), |this, reason| {
            this.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(radius::MD)
                    .bg(colors::YELLOW_FADED.opacity(20))
                    .text_size(text::MEDIUM)
                    .text_color(colors::YELLOW)
                    .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                    .child(tr!(M::ModpackUnsupported, reason = reason)),
            )
        })
        .into_any_element()
}

impl ModpackFlow {
    fn render_source(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("modpack-source")
            .track_scroll(&self.source_scroll)
            .size_full()
            .overflow_y_scroll()
            .pr(px(4.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_start()
                    .gap(px(16.))
                    .child(div().flex_1().min_w_0().child(wizard::title(
                        t(M::ModpackSourceTitle),
                        t(M::ModpackSourceDescription),
                    )))
                    .child(
                        Button::new("modpack-upload", t(M::ModpackUpload))
                            .size(ButtonSize::Slim)
                            .icon(Icon::Upload)
                            .loading(self.uploading.is_some())
                            .disabled(self.uploading.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.upload(cx))),
                    ),
            )
            .when_some(self.uploading.clone(), |this, name| {
                this.child(
                    div()
                        .pb(px(12.))
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .items_center()
                        .text_size(text::MEDIUM)
                        .text_color(ink::muted())
                        .child(LoadingDots)
                        .child(tr!(M::ModpackUploading, name = name)),
                )
            })
            .child(self.search.clone())
            .into_any_element()
    }

    fn render_version(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let uploaded = matches!(self.source, Some(ModpackSource::Upload { .. }));
        let header = self.project_id.is_some().then(|| {
            let (title, description, icon_url) = match &self.project {
                Some(Load::Ready(project)) => (
                    Some(project.title.clone()),
                    project.description.clone(),
                    project.icon_url.clone(),
                ),
                _ => match &self.picked_hit {
                    Some(hit) => (
                        Some(hit.title.clone()),
                        hit.description.clone(),
                        hit.icon_url.clone(),
                    ),
                    None => (None, String::new(), None),
                },
            };
            let icon = icon_url
                .as_deref()
                .and_then(|url| instance_icons::modrinth_image(url, cx));
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .h(px(40.))
                .child(project_icon(icon, px(40.)))
                .child(match title {
                    Some(title) => div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .text_size(text::H3)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::text())
                                .child(title),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(text::SMALL)
                                .text_color(ink::muted())
                                .child(description),
                        )
                        .into_any_element(),
                    None => div()
                        .flex()
                        .gap(px(8.))
                        .items_center()
                        .text_size(text::MEDIUM)
                        .text_color(ink::muted())
                        .child(LoadingDots)
                        .child(t(M::ModpackLoading))
                        .into_any_element(),
                })
        });
        let selected = match &self.source {
            Some(ModpackSource::Modrinth { version_id }) => Some(version_id.clone()),
            _ => None,
        };
        let list: Option<AnyElement> = self.versions.as_ref().map(|versions| match versions {
            Load::Failed(error) => div()
                .text_size(text::MEDIUM)
                .text_color(colors::RED_200)
                .child(tr!(M::ModpackVersionsFailed, error = error))
                .into_any_element(),
            Load::Loading => div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .items_center()
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .child(LoadingDots)
                .child(t(M::ModpackLoadingVersions))
                .into_any_element(),
            Load::Ready(versions) if versions.is_empty() => div()
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .child(t(M::ModpackNoVersions))
                .into_any_element(),
            Load::Ready(versions) => {
                let versions = versions.clone();
                let left = versions.len().saturating_sub(self.shown);
                let rows: Vec<AnyElement> = versions
                    .iter()
                    .take(self.shown)
                    .map(|version| self.version_row(version, selected.as_deref(), cx))
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(fields::section(rows))
                    .when(left > 0, |this| {
                        this.child(
                            div().flex().justify_center().child(
                                Button::new("show-more-versions", trn!(P::ModpackShowMore, left))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.shown += VERSIONS_PAGE;
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
                    .into_any_element()
            }
        });
        let inspecting_size = self
            .inspecting
            .as_ref()
            .and_then(|id| match &self.versions {
                Some(Load::Ready(versions)) => versions
                    .iter()
                    .find(|v| &v.id == id)
                    .and_then(|v| v.primary_file())
                    .map(|f| f.size),
                _ => None,
            });
        let below: AnyElement = if self.inspecting.is_some() {
            div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .items_center()
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .child(LoadingDots)
                .child(match inspecting_size {
                    Some(size) => tr!(
                        M::ModpackDownloadingSize,
                        size = lodestone_ui::i18n::format::bytes(size)
                    ),
                    None => t(M::ModpackDownloading).to_string(),
                })
                .into_any_element()
        } else if let Some(info) = &self.info {
            summary(info)
        } else {
            div()
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .child(t(M::ModpackVersionHint))
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(wizard::title(
                t(M::ModpackVersionTitle),
                if uploaded {
                    t(M::ModpackVersionUploaded)
                } else {
                    t(M::ModpackVersionDescription)
                },
            ))
            .children(header)
            .when_some(list, |this, list| {
                this.child(
                    div()
                        .id("modpack-versions")
                        .mt(px(16.))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .pr(px(4.))
                        .child(list),
                )
            })
            .child(
                div()
                    .mt(px(16.))
                    .flex_none()
                    .min_h(px(152.))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(below),
            )
            .into_any_element()
    }

    fn version_row(
        &self,
        version: &ModrinthVersion,
        selected: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_selected = selected == Some(version.id.as_str());
        let busy = self.inspecting.is_some();
        let inspecting = self.inspecting.as_deref() == Some(version.id.as_str());
        let mut detail = format!(
            "{} · {} · {}",
            version.game_versions.join(", "),
            version.loaders.join(", "),
            short_date(&version.date_published)
        );
        if let Some(file) = version.primary_file() {
            detail = format!(
                "{detail} · {}",
                lodestone_ui::i18n::format::bytes(file.size)
            );
        }
        let id = version.id.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .min_h(px(56.))
            .px(px(16.))
            .py(px(8.))
            .when(is_selected, |this| this.bg(colors::GRAY_700))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(text::MEDIUM)
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(version.version_number.clone()),
                            )
                            .children(version_type_label(version.version_type)),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(detail),
                    ),
            )
            .child(
                Button::new(
                    SharedString::from(format!("select-{}", version.id)),
                    if is_selected {
                        t(M::ModpackSelected)
                    } else {
                        t(M::ModpackSelect)
                    },
                )
                .size(ButtonSize::Slim)
                .variant(if is_selected {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .loading(inspecting)
                .disabled(busy && !inspecting)
                .on_click(cx.listener(move |this, _, _, cx| this.inspect(id.clone(), cx))),
            )
            .into_any_element()
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((_, settings)) = &self.settings else {
            return div().into_any_element();
        };
        let error = |key: &str| self.errors.get(key).cloned();
        let text_row =
            |name: M, description: M, input: &Entity<TextInput>, optional: bool, key: &str| {
                fields::row(
                    t(name),
                    t(description),
                    optional,
                    input.clone().into_any_element(),
                    error(key),
                )
            };
        let line = self.info.as_ref().map(|info| {
            format!(
                "{} · Minecraft {} · Fabric {}",
                info.name,
                info.game_version,
                info.loader_version.clone().unwrap_or_default()
            )
        });
        let auto_start = settings.auto_start;
        let restart = settings.restart_on_crash;
        div()
            .id("modpack-settings")
            .size_full()
            .overflow_y_scroll()
            .pr(px(4.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(wizard::title(
                t(M::ModpackSettingsTitle),
                t(M::ServerSettingsDescription),
            ))
            .when_some(line, |this, line| {
                this.child(
                    div()
                        .mt(px(-12.))
                        .text_size(text::MEDIUM)
                        .text_color(ink::muted())
                        .child(line),
                )
            })
            .child(fields::section(vec![
                text_row(
                    M::FieldName,
                    M::FieldNameDescription,
                    &settings.name,
                    false,
                    "name",
                ),
                text_row(
                    M::FieldDescription,
                    M::FieldDescriptionDescription,
                    &settings.description,
                    true,
                    "description",
                ),
                text_row(
                    M::FieldPort,
                    M::FieldPortDescription,
                    &settings.port,
                    false,
                    "port",
                ),
            ]))
            .child(fields::section(vec![
                text_row(
                    M::FieldMinRam,
                    M::FieldMinRamDescription,
                    &settings.min_ram,
                    false,
                    "min_ram",
                ),
                text_row(
                    M::FieldMaxRam,
                    M::FieldMaxRamDescription,
                    &settings.max_ram,
                    false,
                    "max_ram",
                ),
            ]))
            .child(fields::section(vec![
                fields::row(
                    t(M::FieldAutoStart),
                    t(M::FieldAutoStartDescription),
                    false,
                    Toggle::new("modpack-auto-start", auto_start)
                        .label(t(M::FieldAutoStart))
                        .on_change(cx.listener(|this, value: &bool, _, cx| {
                            if let Some((_, s)) = this.settings.as_mut() {
                                s.auto_start = *value;
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                    None,
                ),
                fields::row(
                    t(M::FieldRestartOnCrash),
                    t(M::FieldRestartOnCrashDescription),
                    false,
                    Toggle::new("modpack-restart", restart)
                        .label(t(M::FieldRestartOnCrash))
                        .on_change(cx.listener(|this, value: &bool, _, cx| {
                            if let Some((_, s)) = this.settings.as_mut() {
                                s.restart_on_crash = *value;
                            }
                            cx.notify();
                        }))
                        .into_any_element(),
                    None,
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

impl Render for ModpackFlow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let steps = [
            t(M::StepSelectGame),
            t(M::StepSelectModpack),
            t(M::StepSelectVersion),
            t(M::StepServerSettings),
        ];
        let current = match self.step {
            Step::Source => 1,
            Step::Version => 2,
            Step::Settings => 3,
        };
        let preset = self.preset.is_some();
        let weak = cx.entity().downgrade();
        let on_select: Option<wizard::StepHandler> = (!self.submitting).then(|| {
            std::rc::Rc::new(move |i: usize, _: &mut Window, cx: &mut gpui::App| {
                weak.update(cx, |this, cx| this.go_to(i, cx)).ok();
            }) as wizard::StepHandler
        });
        let rail = wizard::rail(&steps, current, |i| !(i == 1 && preset), on_select);
        let body = match self.step {
            Step::Source => self.render_source(cx),
            Step::Version => self.render_version(cx),
            Step::Settings => self.render_settings(cx),
        };
        let busy = self.inspecting.is_some() || self.uploading.is_some();
        let next: AnyElement = match self.step {
            Step::Settings => Button::new("modpack-create", t(M::CreateServer))
                .variant(ButtonVariant::Primary)
                .loading(self.submitting)
                .disabled(!self.can_continue() || !self.eula || self.submitting)
                .on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                .into_any_element(),
            Step::Version => Button::new("modpack-next", t(CommonMsg::Next))
                .variant(ButtonVariant::Primary)
                .disabled(!self.can_continue() || busy)
                .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx)))
                .into_any_element(),
            Step::Source => div().into_any_element(),
        };
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
                                Button::new("modpack-back", t(CommonMsg::Back))
                                    .disabled(self.submitting)
                                    .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                            )
                            .child(next),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_free_port() {
        assert_eq!(first_free_port(&[]), 25565);
        assert_eq!(first_free_port(&[25565, 25566, 25568]), 25567);
    }

    #[test]
    fn settings_are_checked_as_the_dashboard_does() {
        assert_eq!(
            check_settings(" Pack ", "25565", "1024", "4096"),
            Ok(("Pack".into(), 25565, 1024, 4096))
        );
        let errors = check_settings("", "0", "x", "512").unwrap_err();
        let fields: Vec<_> = errors.iter().map(|(f, _)| *f).collect();
        assert_eq!(fields, ["name", "port", "min_ram"]);
        let errors = check_settings("a", "70000", "2048", "1024").unwrap_err();
        let fields: Vec<_> = errors.iter().map(|(f, _)| *f).collect();
        assert_eq!(fields, ["port", "max_ram"]);
    }
}
