//! "New instance": a short form. A name, a Minecraft version in a dropdown (with a
//! search and releases / snapshots / old versions filters), the mod loaders as cards
//! that say what each is for, and the chosen loader's version. The Modpack tab
//! installs a Modrinth modpack instead.

use crate::theme::px;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, PathPromptOptions,
    Render, SharedString, Subscription, Task, UniformListScrollHandle, Window, div, prelude::*,
    uniform_list,
};
use lodestone_minecraft::versions::mojang::{MojangVersion, MojangVersionType};

use launcher_core::game::loaders::LoaderKind;

use super::{Instances, Versions, loader_picker::LoaderPicker, modpack_browser::ModpackBrowser};
use crate::components::KeyboardFocus;
use crate::{
    components::{
        Button, ButtonVariant, Dialog, Icon, IconButton, LoadingDots, TextInput,
        motion::Animated,
        settings::{popover, popover_panel, select_button},
    },
    i18n::{CommonMsg, InstanceSettingsMsg as M, format, t, tr},
    theme::{self, Color, colors, leading, radius, text, weights},
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
    /// The Minecraft version dropdown is open.
    versions_open: bool,
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
            cx.new(|cx| LoaderPicker::new(instances, None, None, cx))
        };
        observe.push(cx.observe(&loader, |_, _, cx| cx.notify()));
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
            versions_open: false,
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

    /// Opens the Minecraft version dropdown, for the offscreen screenshots.
    pub fn open_versions(&mut self, cx: &mut Context<Self>) {
        self.versions_open = true;
        cx.notify();
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
                                            this.versions_open = false;
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
        // As tall as its rows, up to a screenful of them.
        let rows = match self.instances.read(cx).versions() {
            Versions::Ready(all) => {
                let search = self.search.read(cx).text().to_string();
                filter_versions(all, self.filter, &search).len()
            }
            _ => 0,
        };
        let height = if rows == 0 {
            96.
        } else {
            (rows as f32 * 30.).min(240.)
        };
        div().h(px(height)).w_full().child(body)
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

/// A row of the version list: the version, its kind and its day in columns.
fn version_row(index: usize, version: &MojangVersion, selected: bool) -> gpui::Stateful<gpui::Div> {
    let date: SharedString = release_date(version.release_time.as_deref()).into();
    div()
        .id(("version", index))
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .w_full()
        .h(px(30.))
        .px_2()
        .rounded(radius::MD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(colors::BLUE.opacity(35)).text_color(colors::WHITE)
            } else {
                this.text_color(colors::GRAY_300)
                    .hover(|style| style.bg(colors::GRAY_700))
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
                .w(px(76.))
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(kind_label(version.kind)),
        )
        .child(
            div()
                .flex_none()
                .w(px(96.))
                .flex()
                .justify_end()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(date),
        )
}

/// A row of the form: its name in a column on the left, then the control.
fn form_row(name: impl Into<SharedString>, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_4()
        .child(
            div()
                .flex_none()
                .w(px(132.))
                .font_weight(weights::MEDIUM.into())
                .text_color(colors::GRAY_400)
                .child(name.into()),
        )
        .child(div().flex_1().min_w_0().child(control))
}

/// A filter of the version list: on or off.
fn filter_chip(id: &'static str, caption: &'static str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2p5()
        .py_0p5()
        .rounded_full()
        .border_1()
        .cursor_pointer()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .when(!on, |this| {
            this.hover(|style| style.text_color(colors::GRAY_300).bg(colors::GRAY_750))
        })
        .when(on, |this| {
            this.child(Icon::Check.render(px(10.), colors::WHITE))
        })
        .child(caption)
}

/// How a loader's card looks: its letter, the letter's tile and ink, and what the
/// loader is for.
fn loader_look(kind: Option<LoaderKind>) -> (&'static str, &'static str, Color, Color, M) {
    match kind {
        None => (
            t(M::Vanilla),
            "V",
            Color::rgb(0x4CAF50),
            colors::WHITE,
            M::AboutVanilla,
        ),
        Some(LoaderKind::Fabric) => (
            LoaderKind::Fabric.label(),
            "F",
            Color::rgb(0xDBD0B4),
            colors::GRAY_900,
            M::AboutFabric,
        ),
        Some(LoaderKind::Quilt) => (
            LoaderKind::Quilt.label(),
            "Q",
            Color::rgb(0x8E55D9),
            colors::WHITE,
            M::AboutQuilt,
        ),
        Some(LoaderKind::Forge) => (
            LoaderKind::Forge.label(),
            "F",
            Color::rgb(0x2E3A57),
            colors::WHITE,
            M::AboutForge,
        ),
        Some(LoaderKind::NeoForge) => (
            LoaderKind::NeoForge.label(),
            "N",
            Color::rgb(0xE07B2E),
            colors::WHITE,
            M::AboutNeoForge,
        ),
    }
}

fn loader_card(
    index: usize,
    kind: Option<LoaderKind>,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    let (name, letter, tile, ink, about) = loader_look(kind);
    div()
        .id(("loader-card", index))
        .keyboard_focus()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded(radius::LG)
        .border_1()
        .cursor_pointer()
        .when(!selected, |this| {
            this.hover(|style| style.bg(colors::GRAY_750))
        })
        .child(
            div()
                .size(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(radius::MD)
                .bg(tile)
                .text_color(ink)
                .font_weight(weights::BOLD.into())
                .text_size(theme::text_size(text::SMALL))
                .child(letter),
        )
        .child(
            div()
                .font_weight(weights::BOLD.into())
                .text_color(if selected {
                    colors::WHITE
                } else {
                    colors::GRAY_300
                })
                .child(name),
        )
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                .text_color(colors::GRAY_500)
                .child(t(about)),
        )
}

impl Render for NewInstanceDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mode = self.mode;
        // The name it gets if the field stays empty.
        let default_name = self.default_name(cx);
        self.name
            .update(cx, |input, cx| input.set_placeholder(default_name, cx));
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
        let body = match mode {
            Mode::Modpack => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(form_row(t(M::Name), self.name.clone()))
                .child(self.modpacks.clone())
                .into_any_element(),
            Mode::Version => self.version_form(cx).into_any_element(),
        };
        Dialog::new("new-instance", t(M::NewInstance), self.focus.clone())
            .width(px(820.))
            .on_close({
                let entity = entity.clone();
                move |_, cx| entity.update(cx, |_, cx| cx.emit(NewInstanceDismissed))
            })
            .header_end(switch)
            .header_end(
                IconButton::new("close-new-instance", Icon::Xmark)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(NewInstanceDismissed))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(body)
                    .when_some(self.error.clone(), |this, error| {
                        this.child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::RED_200)
                                .child(error),
                        )
                    }),
            )
            .footer_start(
                Button::new("import-mrpack", t(M::ImportMrpack))
                    .variant(ButtonVariant::Ghost)
                    .icon(Icon::Download)
                    .on_click(cx.listener(|this, _, _, cx| this.pick_mrpack(cx))),
            )
            .footer(
                Button::new("cancel-new-instance", t(CommonMsg::Cancel))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(NewInstanceDismissed))),
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
    }
}

impl NewInstanceDialog {
    /// The name an instance gets when the field is left empty, as `create` names it.
    fn default_name(&self, cx: &App) -> String {
        match self.mode {
            Mode::Modpack => self
                .modpacks
                .read(cx)
                .selection()
                .map(|(project, _)| project.title)
                .unwrap_or_else(|| t(M::MyInstance).to_string()),
            Mode::Version => match (&self.selected, self.loader.read(cx).kind()) {
                (Some(version), Some(kind)) => format!("{version} {}", kind.label()),
                (Some(version), None) => version.clone(),
                (None, _) => t(M::MyInstance).to_string(),
            },
        }
    }

    /// Name, Minecraft version, the loaders as cards, and the loader's version.
    fn version_form(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.loader.read(cx).kind();
        let cards = std::iter::once(None)
            .chain(LoaderKind::ALL.map(Some))
            .enumerate()
            .map(|(index, option)| {
                let selected = option == kind;
                let card = loader_card(index, option, selected).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.loader
                            .update(cx, |picker, cx| picker.set_kind(option, cx));
                        this.error = None;
                        cx.notify();
                    },
                ));
                // The chosen card's tint and border fade in, the last one's out.
                Animated::new(("loader-card-state", index), selected, card, |this, t| {
                    this.bg(colors::GRAY_800.mix(colors::BLUE.opacity(15), t))
                        .border_color(colors::FADE_700.mix(colors::BLUE_200.opacity(70), t))
                })
                .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(form_row(t(M::Name), self.name.clone()))
            .child(form_row("Minecraft", self.version_select(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(weights::MEDIUM.into())
                            .text_color(colors::GRAY_400)
                            .child(t(M::ModLoader)),
                    )
                    .child(div().flex().flex_row().gap_2().children(cards)),
            )
            .when(kind.is_some(), |this| {
                this.child(form_row(t(M::LoaderVersion), self.loader.clone()))
            })
    }

    /// The chosen Minecraft version in a dropdown, which opens on a search, the
    /// filters and the list.
    fn version_select(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let versions = self.instances.read(cx).versions().clone();
        let chosen = match &versions {
            Versions::Ready(all) => self
                .selected
                .as_ref()
                .and_then(|id| all.iter().find(|v| &v.id == id))
                .cloned(),
            _ => None,
        };
        let label = match (&versions, chosen) {
            (_, Some(version)) => div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap_2()
                .child(
                    div()
                        .font_weight(weights::BOLD.into())
                        .child(version.id.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_500)
                        .child(format!(
                            "{} · {}",
                            kind_label(version.kind),
                            release_date(version.release_time.as_deref())
                        )),
                )
                .into_any_element(),
            (Versions::Failed(_), None) => div()
                .text_color(colors::RED_200)
                .child(t(M::ChooseGameVersion))
                .into_any_element(),
            (Versions::Ready(_), None) => div().child(t(M::ChooseVersion)).into_any_element(),
            _ => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child(t(M::LoadingVersionList))
                .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                select_button("game-version-select", label)
                    .w(px(300.))
                    .justify_between()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.versions_open = !this.versions_open;
                        if this.versions_open {
                            // Type to search straight away.
                            this.search.focus_handle(cx).focus(window, cx);
                        }
                        cx.notify();
                    })),
            )
            .when(self.versions_open, |this| {
                let close = cx.listener(|this, _: &(), _, cx| {
                    this.versions_open = false;
                    cx.notify();
                });
                // In a box of its own under the button: the popover starts where
                // its parent does.
                this.child(div().child(popover(
                    "game-version-popover",
                    self.version_panel(cx),
                    move |window, cx| close(&(), window, cx),
                )))
            })
    }

    fn version_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let filter = self.filter;
        let chip = |id: &'static str,
                    caption: &'static str,
                    on: bool,
                    flip: fn(&mut VersionFilter),
                    cx: &mut Context<Self>| {
            let chip = filter_chip(id, caption, on).on_click(cx.listener(move |this, _, _, cx| {
                let mut filter = this.filter;
                flip(&mut filter);
                this.set_filter(filter, cx);
            }));
            Animated::new(
                SharedString::from(format!("{id}-state")),
                on,
                chip,
                |this, t| {
                    this.bg(colors::BLUE.opacity(0).mix(colors::BLUE.opacity(25), t))
                        .border_color(colors::FADE_700.mix(colors::BLUE_200.opacity(60), t))
                        .text_color(colors::GRAY_400.mix(colors::WHITE, t))
                },
            )
        };
        popover_panel("game-version-list")
            .w(px(420.))
            .gap_2()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.versions_open = false;
                cx.notify();
            }))
            .child(self.search.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1p5()
                    .px_1()
                    .child(chip(
                        "filter-releases",
                        t(M::Releases),
                        filter.releases,
                        |f| f.releases = !f.releases,
                        cx,
                    ))
                    .child(chip(
                        "filter-snapshots",
                        t(M::Snapshots),
                        filter.snapshots,
                        |f| f.snapshots = !f.snapshots,
                        cx,
                    ))
                    .child(chip(
                        "filter-old",
                        t(M::OldVersions),
                        filter.old,
                        |f| f.old = !f.old,
                        cx,
                    )),
            )
            .child(self.version_list(cx))
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
