//! "New instance": a name, a Minecraft version from Mojang's version list (with a
//! releases / snapshots / old versions filter and search), and a mod loader with its
//! version.

use crate::theme::px;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, PathPromptOptions,
    Render, SharedString, Subscription, Task, UniformListScrollHandle, Window, div, prelude::*,
    uniform_list,
};
use lodestone_minecraft::versions::mojang::{MojangVersion, MojangVersionType};

use super::{Instances, Versions, loader_picker::LoaderPicker, modpack_browser::ModpackBrowser};
use crate::components::KeyboardFocus;
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots, TextInput, Toggle},
    i18n::{CommonMsg, InstanceSettingsMsg as M, format, t, tr},
    theme::{self, colors, leading, radius, text, weights},
};

/// Which kinds of versions the list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionFilter {
    pub releases: bool,
    pub snapshots: bool,
    pub old: bool,
}

impl Default for VersionFilter {
    fn default() -> Self {
        Self {
            releases: true,
            snapshots: false,
            old: false,
        }
    }
}

impl VersionFilter {
    pub fn shows(&self, kind: MojangVersionType) -> bool {
        match kind {
            MojangVersionType::Release => self.releases,
            MojangVersionType::Snapshot | MojangVersionType::Other => self.snapshots,
            MojangVersionType::OldBeta | MojangVersionType::OldAlpha => self.old,
        }
    }
}

/// The versions matching the filter and search, in the manifest's order.
pub fn filter_versions<'a>(
    versions: &'a [MojangVersion],
    filter: VersionFilter,
    search: &str,
) -> Vec<&'a MojangVersion> {
    let search = search.trim().to_ascii_lowercase();
    versions
        .iter()
        .filter(|v| filter.shows(v.kind))
        .filter(|v| search.is_empty() || v.id.to_ascii_lowercase().contains(&search))
        .collect()
}

pub struct NewInstanceDismissed;

/// What the dialog makes the instance from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// A Minecraft version and a loader.
    #[default]
    Version,
    /// A Modrinth modpack.
    Modpack,
}

pub struct NewInstanceDialog {
    mode: Mode,
    modpacks: Entity<ModpackBrowser>,
    _prompt: Option<Task<()>>,
    instances: Entity<Instances>,
    name: Entity<TextInput>,
    search: Entity<TextInput>,
    filter: VersionFilter,
    selected: Option<String>,
    loader: Entity<LoaderPicker>,
    error: Option<SharedString>,
    scroll: UniformListScrollHandle,
    /// The dialog's own focus, which the name field is inside.
    focus: FocusHandle,
    _observe: Vec<Subscription>,
}

impl EventEmitter<NewInstanceDismissed> for NewInstanceDialog {}

impl Focusable for NewInstanceDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name.focus_handle(cx)
    }
}

impl NewInstanceDialog {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        instances.update(cx, |instances, cx| instances.load_versions(cx));
        let name = cx.new(|cx| TextInput::new(cx).with_placeholder(M::MyInstance));
        let search = cx.new(|cx| TextInput::new(cx).with_placeholder(M::SearchVersions));
        let observe = vec![
            cx.observe(&instances, |this, _, cx| {
                this.pick_default(cx);
                cx.notify();
            }),
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&name, |_, _, cx| cx.notify()),
        ];
        let modpacks = {
            let instances = instances.clone();
            cx.new(|cx| ModpackBrowser::new(instances, cx))
        };
        let mut observe = observe;
        observe.push(cx.observe(&modpacks, |_, _, cx| cx.notify()));
        let loader = {
            let instances = instances.clone();
            cx.new(|cx| LoaderPicker::new(instances, None, None, 220., cx))
        };
        let mut this = Self {
            mode: Mode::Version,
            modpacks,
            _prompt: None,
            instances,
            name,
            search,
            filter: VersionFilter::default(),
            selected: None,
            loader,
            error: None,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            _observe: observe,
        };
        this.pick_default(cx);
        this
    }

    /// Fills in the fields, for the offscreen screenshots.
    pub fn with_fields(self, name: &str, search: &str, cx: &mut Context<Self>) -> Self {
        self.name
            .update(cx, |input, cx| input.set_text(name.to_string(), cx));
        self.search
            .update(cx, |input, cx| input.set_text(search.to_string(), cx));
        self
    }

    /// Selects the newest release once the list has loaded.
    fn pick_default(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_some() {
            return;
        }
        if let Versions::Ready(versions) = self.instances.read(cx).versions() {
            self.selected = versions
                .iter()
                .find(|v| v.kind == MojangVersionType::Release)
                .map(|v| v.id.clone());
            self.sync_loader(cx);
        }
    }

    fn sync_loader(&mut self, cx: &mut Context<Self>) {
        let version = self.selected.clone();
        self.loader
            .update(cx, |picker, cx| picker.set_game_version(version, cx));
    }

    /// Chooses a loader, as clicking it would.
    pub fn choose_loader(
        &mut self,
        kind: launcher_core::game::loaders::LoaderKind,
        cx: &mut Context<Self>,
    ) {
        self.loader
            .update(cx, |picker, cx| picker.set_kind(Some(kind), cx));
    }

    /// The loader picker, for the offscreen screenshots.
    pub fn loader_picker(&self) -> &Entity<LoaderPicker> {
        &self.loader
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.error = None;
        cx.notify();
    }

    /// The modpack browser, for the offscreen screenshots.
    pub fn modpack_browser(&self) -> &Entity<ModpackBrowser> {
        &self.modpacks
    }

    /// Installs the picked modpack version as a new instance.
    fn install_modpack(&mut self, cx: &mut Context<Self>) {
        let Some((project, version)) = self.modpacks.read(cx).selection() else {
            self.error = Some(t(M::PickModpack).into());
            cx.notify();
            return;
        };
        let typed = self.name.read(cx).text().trim().to_string();
        let name = (!typed.is_empty()).then_some(typed);
        self.instances.update(cx, |instances, cx| {
            instances.import_modrinth(project, version, name, cx)
        });
        cx.emit(NewInstanceDismissed);
    }

    /// "Import .mrpack…": the system's file dialog.
    fn pick_mrpack(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(M::Import).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.instances
                    .update(cx, |instances, cx| instances.import_files(paths, cx));
                cx.emit(NewInstanceDismissed);
            })
            .ok();
        }));
    }

    fn set_filter(&mut self, filter: VersionFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let Some(version) = self.selected.clone() else {
            self.error = Some(t(M::ChooseGameVersion).into());
            cx.notify();
            return;
        };
        let loader = match self.loader.read(cx).choice() {
            Ok(loader) => loader,
            Err(err) => {
                self.error = Some(err.into());
                cx.notify();
                return;
            }
        };
        let typed = self.name.read(cx).text().trim().to_string();
        let name = if typed.is_empty() {
            match &loader {
                Some(loader) => format!("{version} {}", loader.kind.label()),
                None => version.clone(),
            }
        } else {
            typed
        };
        match self.instances.update(cx, |instances, cx| {
            instances.create(&name, &version, loader.as_ref(), cx)
        }) {
            Ok(_) => cx.emit(NewInstanceDismissed),
            Err(err) => {
                self.error = Some(err.into());
                cx.notify();
            }
        }
    }

    fn version_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let versions = self.instances.read(cx).versions().clone();
        let body = match versions {
            Versions::NotLoaded | Versions::Loading => div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .size_full()
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child(t(M::LoadingVersionList))
                .into_any_element(),
            Versions::Failed(error) => div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .p_4()
                .text_center()
                .text_color(colors::RED_200)
                .text_size(theme::text_size(text::SMALL))
                .child(tr!(M::CouldntLoadVersions, error = error))
                .into_any_element(),
            Versions::Ready(all) => {
                let search = self.search.read(cx).text().to_string();
                let count = filter_versions(&all, self.filter, &search).len();
                if count == 0 {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .text_color(colors::GRAY_500)
                        .child(t(M::NoVersionsMatch))
                        .into_any_element()
                } else {
                    uniform_list(
                        "versions",
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            let search = this.search.read(cx).text().to_string();
                            let shown = filter_versions(&all, this.filter, &search);
                            range
                                .filter_map(|index| shown.get(index).map(|v| (index, *v)))
                                .map(|(index, version)| {
                                    let selected = this.selected.as_deref() == Some(&version.id);
                                    let id = version.id.clone();
                                    version_row(index, version, selected).on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.selected = Some(id.clone());
                                            this.error = None;
                                            this.sync_loader(cx);
                                            cx.notify();
                                        },
                                    ))
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&self.scroll)
                    .size_full()
                    .into_any_element()
                }
            }
        };
        div()
            .h(px(220.))
            .w_full()
            .p_1()
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .bg(colors::GRAY_900)
            .child(body)
    }
}

fn kind_label(kind: MojangVersionType) -> &'static str {
    t(match kind {
        MojangVersionType::Release => M::KindRelease,
        MojangVersionType::Snapshot => M::KindSnapshot,
        MojangVersionType::OldBeta => M::KindBeta,
        MojangVersionType::OldAlpha => M::KindAlpha,
        MojangVersionType::Other => M::KindOther,
    })
}

/// A version's release day, "Jun 13, 2024", from the manifest's `2024-06-13T…`.
fn release_date(time: Option<&str>) -> String {
    let Some(day) = time.and_then(|time| time.get(..10)) else {
        return String::new();
    };
    match chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d") {
        Ok(date) => format::short_date(date, true),
        Err(_) => day.to_string(),
    }
}

fn version_row(index: usize, version: &MojangVersion, selected: bool) -> gpui::Stateful<gpui::Div> {
    let date: SharedString = release_date(version.release_time.as_deref()).into();
    div()
        .id(("version", index))
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(30.))
        .px_2()
        .rounded(radius::MD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(colors::BLUE.opacity(35)).text_color(colors::WHITE)
            } else {
                this.text_color(colors::GRAY_300)
                    .hover(|style| style.bg(colors::GRAY_800))
            }
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(weights::BOLD.into())
                .child(version.id.clone()),
        )
        .child(
            div()
                .flex_none()
                .w(px(72.))
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(kind_label(version.kind)),
        )
        .child(
            div()
                .flex_none()
                .w(px(96.))
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(date),
        )
}

fn label(text_: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_400)
        .child(text_)
}

fn filter_toggle(
    id: &'static str,
    caption: &'static str,
    value: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Toggle::new(id, value).on_change(on_change))
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_400)
                .child(caption),
        )
}

impl Render for NewInstanceDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let filter = self.filter;
        let selected: SharedString = self
            .selected
            .clone()
            .map(|id| match self.loader.read(cx).choice() {
                Ok(Some(loader)) => tr!(
                    M::GameWithLoader,
                    version = id,
                    loader = loader.kind.label(),
                    loader_version = loader.version
                ),
                _ => format!("Minecraft {id}"),
            })
            .unwrap_or_else(|| t(M::NoVersionSelected).into())
            .into();
        let mode = self.mode;
        let switch = crate::instances::content::view::pills(
            "new-instance-mode",
            vec![t(M::MinecraftVersion).into(), t(M::Modpack).into()],
            match mode {
                Mode::Version => 0,
                Mode::Modpack => 1,
            },
            {
                let entity = entity.clone();
                move |index, _, cx| {
                    let mode = if index == 0 {
                        Mode::Version
                    } else {
                        Mode::Modpack
                    };
                    entity.update(cx, |this, cx| this.set_mode(mode, cx));
                }
            },
        );
        let modpack_selection = self.modpacks.read(cx).selection();
        let footer_status: SharedString = match (&self.error, mode, &modpack_selection) {
            (Some(error), _, _) => error.clone(),
            (None, Mode::Version, _) => selected.clone(),
            (None, Mode::Modpack, Some((project, version))) => format!(
                "{} {} · Minecraft {} · {}",
                project.title,
                version.version_number,
                version.game_versions.first().cloned().unwrap_or_default(),
                version.loaders.first().cloned().unwrap_or_default()
            )
            .into(),
            (None, Mode::Modpack, None) => t(M::NoModpackPicked).into(),
        };
        let body = match mode {
            Mode::Modpack => self.modpacks.clone().into_any_element(),
            Mode::Version => self.version_body(&entity, filter, cx).into_any_element(),
        };
        Dialog::new("new-instance", t(M::NewInstance), self.focus.clone())
            .width(px(860.))
            .on_close({
                let entity = entity.clone();
                move |_, cx| entity.update(cx, |_, cx| cx.emit(NewInstanceDismissed))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(div().flex().child(switch))
                    .child(label(t(M::NameLabel)))
                    .child(self.name.clone())
                    .child(body)
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .font_weight(weights::BOLD.into())
                            .text_color(if self.error.is_some() {
                                colors::RED_200
                            } else {
                                colors::GRAY_500
                            })
                            .child(footer_status),
                    ),
            )
            .map(|dialog| match mode {
                Mode::Version => dialog.footer(
                    Button::new("create-instance", t(M::CreateInstance))
                        .variant(ButtonVariant::Primary)
                        .disabled(self.selected.is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.create(cx))),
                ),
                Mode::Modpack => dialog.footer(
                    Button::new("install-modpack", t(M::InstallModpack))
                        .variant(ButtonVariant::Primary)
                        .disabled(modpack_selection.is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.install_modpack(cx))),
                ),
            })
            .footer(
                Button::new("import-mrpack", t(M::ImportMrpack))
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|this, _, _, cx| this.pick_mrpack(cx))),
            )
            .footer(
                Button::new("cancel-new-instance", t(CommonMsg::Cancel))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(NewInstanceDismissed))),
            )
    }
}

impl NewInstanceDialog {
    /// The Minecraft version list and the loader picker.
    fn version_body(
        &mut self,
        entity: &Entity<Self>,
        filter: VersionFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let toggle = |field: fn(&mut VersionFilter) -> &mut bool| {
            let entity = entity.clone();
            move |value: &bool, _: &mut Window, cx: &mut App| {
                let mut filter = filter;
                *field(&mut filter) = *value;
                entity.update(cx, |this, cx| this.set_filter(filter, cx));
            }
        };
        div()
            .flex()
            .flex_row()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .child(div().pt_1().child(label(t(M::MinecraftVersionLabel))))
                    .child(self.search.clone())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_x_4()
                            .gap_y_2()
                            .child(filter_toggle(
                                "filter-releases",
                                t(M::Releases),
                                filter.releases,
                                toggle(|f| &mut f.releases),
                            ))
                            .child(filter_toggle(
                                "filter-snapshots",
                                t(M::Snapshots),
                                filter.snapshots,
                                toggle(|f| &mut f.snapshots),
                            ))
                            .child(filter_toggle(
                                "filter-old",
                                t(M::OldVersions),
                                filter.old,
                                toggle(|f| &mut f.old),
                            )),
                    )
                    .child(self.version_list(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .child(div().pt_1().child(label(t(M::ModLoaderLabel))))
                    .child(self.loader.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions() -> Vec<MojangVersion> {
        let manifest: lodestone_minecraft::versions::mojang::MojangVersionManifest =
            serde_json::from_str(include_str!(
                "../../../launcher-core/testdata/version_manifest_v2.json"
            ))
            .unwrap();
        manifest.versions
    }

    #[test]
    fn filters_and_searches() {
        let versions = versions();
        let ids = |filter, search| -> Vec<String> {
            filter_versions(&versions, filter, search)
                .into_iter()
                .map(|v| v.id.clone())
                .collect()
        };
        let releases = ids(VersionFilter::default(), "");
        assert_eq!(releases[0], "26.3");
        assert!(releases.contains(&"1.8.9".to_string()));
        assert!(
            !releases
                .iter()
                .any(|id| id.contains("snapshot") || id.starts_with('b'))
        );
        let snapshots = VersionFilter {
            releases: false,
            snapshots: true,
            old: false,
        };
        assert!(
            ids(snapshots, "")
                .iter()
                .all(|id| id.contains("snapshot") || id.contains("rc"))
        );
        let old = VersionFilter {
            releases: false,
            snapshots: false,
            old: true,
        };
        assert!(ids(old, "").contains(&"b1.7.3".to_string()));
        assert_eq!(ids(VersionFilter::default(), " 1.20 "), ["1.20.1"]);
        assert!(ids(VersionFilter::default(), "nope").is_empty());
    }
}
