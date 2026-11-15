//! Making a server from one the user already has: choose its folder, which the core
//! reads to tell the Minecraft version and the server software, then the server's
//! settings. The core makes the server in the background and copies the folder into
//! it; the folder itself is never changed.

use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString,
    Styled, Task, Window, div, prelude::*,
};
use launcher_core::cores::admin::instances::{
    ConfigurableValue, ConfigurableValueType, ImportServerRequest, ServerFolderInfo,
    ServerSoftware, SetupManifest,
};
use lodestone_ui::{
    components::{Button, ButtonVariant, TextInput, Toast, segmented},
    file_dialog::{Pick, ShowsError},
    theme::{colors, ink, px, text, weights},
};

use super::{
    Load, error_line,
    fields::{self, Field, HasFields},
    loading_line,
    modpack::{check_settings, first_free_port},
    wizard,
};
use crate::{
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, t, tr, trn},
    state::HostState,
};

/// What the flow tells the dialog.
pub enum ImportEvent {
    /// Back to the game selection.
    Back,
    /// The server is being made: close the dialog.
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Folder,
    Settings,
}

/// The settings step's text fields.
struct Settings {
    name: Entity<TextInput>,
    port: Entity<TextInput>,
    min_ram: Entity<TextInput>,
    max_ram: Entity<TextInput>,
}

pub struct ImportFlow {
    state: Entity<HostState>,
    step: Step,
    path: Entity<TextInput>,
    reading: bool,
    read_error: Option<String>,
    /// What the core found in the folder, and the settings made for it.
    found: Option<(ServerFolderInfo, Settings)>,
    software: Option<ServerSoftware>,
    /// The chosen software's Minecraft versions, as a field.
    version: Option<(ServerSoftware, Load<Field>)>,
    errors: std::collections::HashMap<&'static str, String>,
    submitting: bool,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<ImportEvent> for ImportFlow {}

impl ShowsError for ImportFlow {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast(Toast::error(message), cx);
    }
}

impl HasFields for ImportFlow {
    fn field_mut(&mut self, _: usize) -> Option<&mut Field> {
        match &mut self.version {
            Some((_, Load::Ready(field))) => Some(field),
            _ => None,
        }
    }
}

/// The port the imported server starts with: its own, unless another server here has
/// it, and then the first free one.
pub fn import_port(own: Option<u32>, used: &[u32]) -> u32 {
    match own {
        Some(port) if !used.contains(&port) => port,
        _ => first_free_port(used),
    }
}

/// The versions field for a software's setup form: its Minecraft versions, starting at
/// `wanted` when the software has it, and at nothing otherwise, so a version is never
/// picked for the user.
pub fn version_setting(
    manifest: &SetupManifest,
    wanted: Option<&str>,
) -> Option<launcher_core::cores::admin::instances::SettingManifest> {
    let mut setting = manifest
        .setting_sections
        .iter()
        .flat_map(|section| section.settings.iter())
        .find(|setting| setting.setting_id == "version")?
        .clone();
    let ConfigurableValueType::Enum { options } = &setting.value_type else {
        return None;
    };
    let start = wanted
        .filter(|wanted| options.iter().any(|option| option == wanted))
        .map(|wanted| ConfigurableValue::Enum(wanted.to_string()));
    setting.default_value = start.clone();
    setting.value = start;
    setting.is_required = true;
    setting.name = t(M::FieldVersion).into();
    setting.description = t(M::FieldVersionDescription).into();
    Some(setting)
}

/// The loader version to ask for: the server's own, while the software and the
/// Minecraft version are still the ones it was found with.
pub fn loader_to_keep(
    info: &ServerFolderInfo,
    software: ServerSoftware,
    version: &str,
) -> Option<String> {
    (info.flavour == Some(software) && info.version.as_deref() == Some(version))
        .then(|| info.loader_version.clone())
        .flatten()
}

impl ImportFlow {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let path = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::ImportFolderPlaceholder)
                .with_label(M::ImportFolderPlaceholder)
                .with_automation_id("import-path")
        });
        Self {
            state,
            step: Step::Folder,
            path,
            reading: false,
            read_error: None,
            found: None,
            software: None,
            version: None,
            errors: Default::default(),
            submitting: false,
            _tasks: Vec::new(),
        }
    }

    /// The flow with a folder already typed and read, for screenshots.
    pub fn with_path(state: Entity<HostState>, path: String, cx: &mut Context<Self>) -> Self {
        let mut this = Self::new(state, cx);
        this.path.update(cx, |input, cx| input.set_text(path, cx));
        this.read(cx);
        this
    }

    fn admin(&self, cx: &Context<Self>) -> launcher_core::cores::admin::AdminClient {
        self.state.read(cx).admin.clone()
    }

    fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    /// The system's folder dialog. The Host runs its core on this computer, so a folder
    /// picked here is one the core can read.
    fn browse(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(Pick::Folders.options(false, t(M::ImportFolderTitle)));
        let task = cx.spawn(async move |this, cx| {
            let Some(paths) = lodestone_ui::file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let text = path.to_string_lossy().to_string();
                this.path.update(cx, |input, cx| input.set_text(text, cx));
                this.read(cx);
            })
            .ok();
        });
        self._tasks.push(task);
    }

    /// Asks the core what the folder holds, and goes on to the settings.
    fn read(&mut self, cx: &mut Context<Self>) {
        if self.reading || self.submitting {
            return;
        }
        let path = self.path.read(cx).text().trim().to_string();
        if path.is_empty() {
            self.read_error = Some(t(M::ImportChooseFolder).to_string());
            cx.notify();
            return;
        }
        self.reading = true;
        self.read_error = None;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.inspect_server_folder(&path).await })
                .await;
            this.update(cx, |this, cx| {
                this.reading = false;
                match result {
                    Ok(info) => this.open_settings(info, cx),
                    Err(err) => this.read_error = Some(tr!(M::ImportReadFailed, error = err)),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    /// The settings for a folder that was read: made again for another folder, kept
    /// for the same one.
    fn open_settings(&mut self, info: ServerFolderInfo, cx: &mut Context<Self>) {
        if self.found.as_ref().map(|(found, _)| found) != Some(&info) {
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
                name: input(info.name.clone(), M::FieldName, cx),
                port: input(import_port(info.port, &used).to_string(), M::FieldPort, cx),
                min_ram: input(info.min_ram.unwrap_or(1024).to_string(), M::FieldMinRam, cx),
                max_ram: input(info.max_ram.unwrap_or(4096).to_string(), M::FieldMaxRam, cx),
            };
            let software = info.flavour.filter(|s| *s != ServerSoftware::Unknown);
            self.found = Some((info, settings));
            self.software = None;
            self.version = None;
            if let Some(software) = software {
                self.choose_software(software, cx);
            }
        }
        self.errors.clear();
        self.step = Step::Settings;
    }

    /// The Minecraft version picked, or else the one the folder was found with.
    fn wanted_version(&self) -> Option<String> {
        let picked = match &self.version {
            Some((_, Load::Ready(field))) => match &field.input {
                fields::Input::Choice { value, .. } => value.clone(),
                _ => None,
            },
            _ => None,
        };
        picked.or_else(|| {
            self.found
                .as_ref()
                .and_then(|(info, _)| info.version.clone())
        })
    }

    /// Picks the server software, and fetches the Minecraft versions it has.
    fn choose_software(&mut self, software: ServerSoftware, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let Some(game) = software.game_type() else {
            return;
        };
        let loaded = self
            .version
            .as_ref()
            .is_some_and(|(s, load)| *s == software && !matches!(load, Load::Failed(_)));
        self.software = Some(software);
        self.errors.remove("software");
        if loaded {
            cx.notify();
            return;
        }
        let wanted = self.wanted_version();
        self.version = Some((software, Load::Loading));
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let manifest = cx
                .background_spawn(async move { admin.setup_manifest(&game).await })
                .await;
            this.update(cx, |this, cx| {
                // only the software still chosen
                if this.software != Some(software) {
                    return;
                }
                let field = match manifest {
                    Ok(manifest) => match version_setting(&manifest, wanted.as_deref()) {
                        Some(setting) => Load::Ready(Field::new(setting, None, cx)),
                        None => Load::Failed(t(M::ImportNoVersions).to_string()),
                    },
                    Err(err) => Load::Failed(err.to_string()),
                };
                this.version = Some((software, field));
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Settings => {
                self.step = Step::Folder;
                cx.notify();
            }
            Step::Folder => cx.emit(ImportEvent::Back),
        }
    }

    fn go_to(&mut self, step: usize, cx: &mut Context<Self>) {
        match step {
            0 => cx.emit(ImportEvent::Back),
            1 if self.step == Step::Settings => {
                self.step = Step::Folder;
                cx.notify();
            }
            _ => {}
        }
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let Some((info, settings)) = &self.found else {
            return;
        };
        let read = |input: &Entity<TextInput>| input.read(cx).text().to_string();
        let mut errors: std::collections::HashMap<&'static str, String> = Default::default();
        let checked = check_settings(
            &read(&settings.name),
            &read(&settings.port),
            &read(&settings.min_ram),
            &read(&settings.max_ram),
        )
        .map_err(|found| errors.extend(found))
        .ok();
        if self.software.is_none() {
            errors.insert("software", t(M::ImportChooseSoftware).to_string());
        }
        let version = match &self.version {
            Some((_, Load::Ready(field))) => match field.value(cx) {
                Ok(Some(ConfigurableValue::Enum(version))) => Some(version),
                Ok(_) => None,
                Err(error) => {
                    errors.insert("version", error);
                    None
                }
            },
            _ => None,
        };
        let (Some((name, port, min_ram, max_ram)), Some(software), Some(version)) =
            (checked, self.software, version)
        else {
            self.errors = errors;
            cx.notify();
            return;
        };
        self.errors.clear();
        let request = ImportServerRequest {
            path: info.path.clone(),
            name,
            loader_version: loader_to_keep(info, software, &version),
            version,
            flavour: software,
            port,
            min_ram,
            max_ram,
        };
        self.submitting = true;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.import_server(&request).await })
                .await;
            this.update(cx, |this, cx| {
                this.submitting = false;
                match result {
                    Ok(_) => cx.emit(ImportEvent::Done),
                    Err(err) => {
                        let text = tr!(M::ImportFailed, error = err);
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

/// A muted line of text under a step's title or a field.
fn note(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

/// What was found, in a line: "Paper 1.20.1 · 3,412 files · 1.2 GB".
pub fn found_line(info: &ServerFolderInfo) -> String {
    let software = match (&info.other_software, info.flavour) {
        (Some(other), _) => Some(other.clone()),
        (None, Some(software)) if software != ServerSoftware::Unknown => {
            Some(software.title().to_string())
        }
        _ => None,
    };
    let what = match (software, &info.version) {
        (Some(software), Some(version)) => Some(format!("{software} {version}")),
        (Some(software), None) => Some(software),
        (None, Some(version)) => Some(format!("Minecraft {version}")),
        (None, None) => None,
    };
    what.into_iter()
        .chain([
            trn!(P::ImportFiles, info.file_count as usize),
            launcher_core::import::copy::format_size(info.size_bytes),
        ])
        .collect::<Vec<_>>()
        .join(" · ")
}

impl ImportFlow {
    fn render_folder(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(wizard::title(
                t(M::ImportFolderTitle),
                t(M::ImportFolderSubtitle),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().child(self.path.clone()))
                    .child(
                        Button::new("import-browse", t(M::ImportBrowse))
                            .disabled(self.reading)
                            .on_click(cx.listener(|this, _, _, cx| this.browse(cx))),
                    ),
            )
            .when(self.reading, |this| {
                this.child(loading_line(t(M::ImportReading)))
            })
            .when_some(self.read_error.clone(), |this, error| {
                this.child(error_line(error))
            })
            .child(note(t(M::ImportStopFirst)))
            .into_any_element()
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((info, _)) = &self.found else {
            return div().into_any_element();
        };
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
        let mut notes = vec![note(found_line(info))];
        match (&info.other_software, info.flavour) {
            (Some(other), Some(software)) if software != ServerSoftware::Unknown => {
                notes.push(note(tr!(
                    M::ImportOtherSoftware,
                    other = other,
                    software = software.title()
                )))
            }
            (Some(other), _) => notes.push(note(tr!(M::ImportNotOffered, other = other))),
            (None, None) => notes.push(note(t(M::ImportSoftwareUnknown))),
            _ => {}
        }
        // the version the folder has, when the chosen software doesn't
        let missing_version = match (&self.version, &info.version, self.software) {
            (Some((_, Load::Ready(field))), Some(version), Some(software)) => {
                match &field.setting.value_type {
                    ConfigurableValueType::Enum { options }
                        if !options.iter().any(|option| option == version) =>
                    {
                        Some(tr!(
                            M::ImportVersionUnavailable,
                            software = software.title(),
                            version = version
                        ))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        let weak = cx.entity().downgrade();
        let software = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .px(px(16.))
            .py(px(12.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(text::MEDIUM)
                            .font_weight(weights::BOLD.face())
                            .text_color(ink::text())
                            .child(t(M::FieldSoftware)),
                    )
                    .child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(t(M::FieldSoftwareDescription)),
                    ),
            )
            .child(segmented(
                "import-software",
                ServerSoftware::ALL
                    .into_iter()
                    .map(|software| (software, SharedString::from(software.title())))
                    .collect(),
                self.software.as_ref(),
                self.submitting,
                move |software, _, cx| {
                    weak.update(cx, |this, cx| this.choose_software(software, cx))
                        .ok();
                },
            ))
            .when_some(error("software"), |this, error| {
                this.child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(colors::RED_200)
                        .child(error),
                )
            })
            .into_any_element();
        let version = match &self.version {
            Some((_, Load::Ready(field))) => Some(fields::field_row(
                field,
                0,
                "import",
                error("version").or(missing_version),
                cx,
            )),
            Some((_, Load::Loading)) => Some(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .child(loading_line(t(M::ImportLoadingVersions)))
                    .into_any_element(),
            ),
            Some((_, Load::Failed(error))) => Some(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .child(error_line(tr!(M::ImportVersionsFailed, error = error)))
                    .into_any_element(),
            ),
            None => None,
        };
        let Some((_, settings)) = &self.found else {
            return div().into_any_element();
        };
        div()
            .id("import-settings")
            .size_full()
            .overflow_y_scroll()
            .pr(px(4.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(wizard::title(
                t(M::ModpackSettingsTitle),
                t(M::ImportSettingsSubtitle),
            ))
            .child(
                div()
                    .mt(px(-12.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(notes),
            )
            .child(fields::section(
                [software].into_iter().chain(version).collect(),
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
            .child(fields::section(vec![
                text_row(
                    M::FieldMinRam,
                    M::FieldMinRamDescription,
                    &settings.min_ram,
                    "min_ram",
                ),
                text_row(
                    M::FieldMaxRam,
                    M::FieldMaxRamDescription,
                    &settings.max_ram,
                    "max_ram",
                ),
            ]))
            .into_any_element()
    }
}

impl Render for ImportFlow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let steps = [
            t(M::StepSelectGame),
            t(M::StepChooseFolder),
            t(M::StepServerSettings),
        ];
        let current = match self.step {
            Step::Folder => 1,
            Step::Settings => 2,
        };
        let weak = cx.entity().downgrade();
        let on_select: Option<wizard::StepHandler> = (!self.submitting).then(|| {
            std::rc::Rc::new(move |i: usize, _: &mut Window, cx: &mut gpui::App| {
                weak.update(cx, |this, cx| this.go_to(i, cx)).ok();
            }) as wizard::StepHandler
        });
        let rail = wizard::rail(&steps, current, |_| true, on_select);
        let body = match self.step {
            Step::Folder => self.render_folder(cx),
            Step::Settings => self.render_settings(cx),
        };
        let next: AnyElement = match self.step {
            Step::Folder => Button::new("import-next", t(CommonMsg::Next))
                .variant(ButtonVariant::Primary)
                .loading(self.reading)
                .disabled(self.reading)
                .on_click(cx.listener(|this, _, _, cx| this.read(cx)))
                .into_any_element(),
            Step::Settings => Button::new("import-create", t(M::ImportServer))
                .variant(ButtonVariant::Primary)
                .loading(self.submitting)
                // nothing to import as until the software's versions are in
                .disabled(self.submitting || !matches!(self.version, Some((_, Load::Ready(_)))))
                .on_click(cx.listener(|this, _, _, cx| this.import(cx)))
                .into_any_element(),
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
                                Button::new("import-back", t(CommonMsg::Back))
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
    use launcher_core::cores::admin::instances::{SectionManifest, SettingManifest};

    #[test]
    fn the_server_keeps_its_port_unless_it_is_taken() {
        assert_eq!(import_port(Some(25570), &[25565]), 25570);
        assert_eq!(import_port(Some(25565), &[25565, 25566]), 25567);
        assert_eq!(import_port(None, &[25565]), 25566);
    }

    fn manifest(versions: &[&str]) -> SetupManifest {
        let setting = |id: &str, value_type: ConfigurableValueType| SettingManifest {
            setting_id: id.into(),
            name: id.into(),
            description: String::new(),
            value: None,
            value_type,
            default_value: Some(ConfigurableValue::Enum("1.21.1".into())),
            is_secret: false,
            is_required: true,
            is_mutable: true,
        };
        SetupManifest {
            setting_sections: vec![SectionManifest {
                section_id: "section_1".into(),
                name: String::new(),
                description: String::new(),
                settings: vec![
                    setting(
                        "port",
                        ConfigurableValueType::UnsignedInteger {
                            min: None,
                            max: None,
                        },
                    ),
                    setting(
                        "version",
                        ConfigurableValueType::Enum {
                            options: versions.iter().map(|v| v.to_string()).collect(),
                        },
                    ),
                ],
            }],
        }
    }

    #[test]
    fn a_version_is_never_picked_for_the_user() {
        let manifest = manifest(&["1.21.1", "1.20.1"]);
        // the folder's version, when the software has it
        let found = version_setting(&manifest, Some("1.20.1")).unwrap();
        assert_eq!(
            found.default_value,
            Some(ConfigurableValue::Enum("1.20.1".into()))
        );
        assert!(found.is_required);
        // else nothing, not the form's newest version
        for wanted in [Some("1.8.9"), None] {
            let none = version_setting(&manifest, wanted).unwrap();
            assert_eq!((none.default_value, none.value), (None, None));
        }
        assert!(
            version_setting(
                &SetupManifest {
                    setting_sections: Vec::new()
                },
                Some("1.20.1")
            )
            .is_none()
        );
    }

    #[test]
    fn the_loader_is_kept_while_nothing_else_changed() {
        let info = ServerFolderInfo {
            version: Some("1.20.1".into()),
            flavour: Some(ServerSoftware::Forge),
            loader_version: Some("1.20.1-47.2.0".into()),
            ..Default::default()
        };
        assert_eq!(
            loader_to_keep(&info, ServerSoftware::Forge, "1.20.1").as_deref(),
            Some("1.20.1-47.2.0")
        );
        assert_eq!(loader_to_keep(&info, ServerSoftware::Forge, "1.20.4"), None);
        assert_eq!(
            loader_to_keep(&info, ServerSoftware::NeoForge, "1.20.1"),
            None
        );
    }
}
