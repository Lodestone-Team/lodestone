//! Choosing a mod loader and its version for a Minecraft version, newest first, with
//! the recommended one picked. An instance's Settings tab shows it in its compact
//! form: two settings rows, the loaders as one segmented control and the version in
//! a dropdown. "New instance" draws the loaders as cards itself and shows only the
//! version dropdown.

use crate::theme::px;
use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, SharedString, Subscription,
    UniformListScrollHandle, Window, div, prelude::*, uniform_list,
};
use launcher_core::game::loaders::{LoaderChoice, LoaderKind, LoaderVersion};

use super::{Instances, LoaderVersions};
use crate::components::{
    ArrowGroup, KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
};
use crate::{
    components::{
        LoadingDots,
        settings::{SettingRow, popover, popover_panel, select_button, tag},
    },
    i18n::{InstanceSettingsMsg as M, t, tr},
    theme::{self, colors, radius, text, weights},
};

pub struct LoaderPicker {
    instances: Entity<Instances>,
    game_version: Option<String>,
    kind: Option<LoaderKind>,
    /// The chosen loader version's id.
    selected: Option<String>,
    scroll: UniformListScrollHandle,
    /// Two settings rows (the instance's Settings tab), instead of only the version
    /// dropdown (the New instance dialog, which draws the loaders itself).
    compact: bool,
    /// The version list is open.
    open: bool,
    /// A loader or a version was picked since the choice was last set.
    touched: bool,
    _observe: Subscription,
}

impl LoaderPicker {
    pub fn new(
        instances: Entity<Instances>,
        game_version: Option<String>,
        initial: Option<LoaderChoice>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&instances, |this, _, cx| {
            this.pick_default(cx);
            cx.notify();
        });
        let mut this = Self {
            instances,
            game_version,
            kind: initial.as_ref().map(|c| c.kind),
            selected: initial.map(|c| c.version),
            scroll: UniformListScrollHandle::new(),
            compact: false,
            open: false,
            touched: false,
            _observe: observe,
        };
        this.load(cx);
        this
    }

    /// The form for an instance's Settings tab: two settings rows.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Shows `choice`, as the instance has it saved.
    pub fn set_choice(&mut self, choice: Option<LoaderChoice>, cx: &mut Context<Self>) {
        let kind = choice.as_ref().map(|c| c.kind);
        let selected = choice.map(|c| c.version);
        if self.kind == kind && self.selected == selected {
            return;
        }
        self.kind = kind;
        self.selected = selected;
        self.open = false;
        self.touched = false;
        self.load(cx);
        cx.notify();
    }

    /// Shows `choice` as Undo puts it back: `touched` when it differs from what the
    /// instance has saved, so saving keeps it.
    pub fn restore_choice(
        &mut self,
        choice: Option<LoaderChoice>,
        touched: bool,
        cx: &mut Context<Self>,
    ) {
        self.kind = choice.as_ref().map(|c| c.kind);
        self.selected = choice.map(|c| c.version);
        self.open = false;
        self.touched = touched;
        self.load(cx);
        cx.notify();
    }

    /// Follows a change of Minecraft version: the loader stays, its version is
    /// picked again.
    pub fn set_game_version(&mut self, game_version: Option<String>, cx: &mut Context<Self>) {
        if self.game_version == game_version {
            return;
        }
        self.game_version = game_version;
        self.selected = None;
        self.load(cx);
        cx.notify();
    }

    pub fn set_kind(&mut self, kind: Option<LoaderKind>, cx: &mut Context<Self>) {
        if self.kind == kind {
            return;
        }
        self.touched = true;
        self.kind = kind;
        self.selected = None;
        self.load(cx);
        cx.notify();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if let (Some(kind), Some(game)) = (self.kind, self.game_version.clone()) {
            self.instances.update(cx, |instances, cx| {
                instances.load_loader_versions(kind, &game, cx)
            });
        }
        self.pick_default(cx);
    }

    fn versions(&self, cx: &Context<Self>) -> LoaderVersions {
        match (self.kind, &self.game_version) {
            (Some(kind), Some(game)) => self.instances.read(cx).loader_versions(kind, game),
            _ => LoaderVersions::NotLoaded,
        }
    }

    /// Picks the recommended version once the list has loaded, unless one that is in
    /// the list is chosen.
    fn pick_default(&mut self, cx: &mut Context<Self>) {
        if let LoaderVersions::Ready(list) = self.versions(cx) {
            let known = self
                .selected
                .as_ref()
                .is_some_and(|id| list.iter().any(|v| &v.id == id));
            if !known {
                self.selected = list
                    .iter()
                    .find(|v| v.recommended)
                    .or_else(|| list.first())
                    .map(|v| v.id.clone());
            }
        }
    }

    /// The choice: `Ok(None)` for vanilla, or an error to show when a loader has no
    /// version chosen.
    pub fn choice(&self) -> Result<Option<LoaderChoice>, String> {
        let Some(kind) = self.kind else {
            return Ok(None);
        };
        match &self.selected {
            Some(version) => Ok(Some(LoaderChoice {
                kind,
                version: version.clone(),
            })),
            None => Err(tr!(M::ChooseLoaderVersion, loader = kind.label())),
        }
    }

    /// Whether a loader or a version was picked by hand, rather than the recommended
    /// version being picked when the list loaded.
    pub fn touched(&self) -> bool {
        self.touched
    }

    pub fn kind(&self) -> Option<LoaderKind> {
        self.kind
    }
}

fn version_row(index: usize, version: &LoaderVersion, selected: bool) -> gpui::Stateful<gpui::Div> {
    let tag = version_tag(version);
    div()
        .id(("loader-version", index))
        .accessible(
            Role::ListItem,
            format!("loader-version-{}", version.id),
            version.display.clone(),
        )
        .aria_selected(selected)
        .when_some(tag, |this, (tag, _)| this.aria_description(tag))
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(28.))
        .px_2()
        .rounded(radius::MD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(crate::theme::accent::base().opacity(35))
                    .text_color(colors::WHITE)
            } else {
                this.text_color(crate::theme::ink::text())
                    .hover(|style| style.bg(colors::GRAY_700))
            }
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(weights::BOLD.into())
                .child(version.display.clone()),
        )
        .when_some(tag, |this, (tag, color)| {
            this.child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .text_color(color)
                    .child(tag),
            )
        })
}

impl LoaderPicker {
    /// The loaders as one segmented control.
    fn segmented(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let options = loader_options();
        ArrowGroup::new("loader-segments")
            .group_role(Role::RadioGroup)
            .flex()
            .flex_row()
            .p(px(2.))
            .gap(px(2.))
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::GRAY_700)
            .bg(colors::GRAY_900)
            .children(options.enumerate().map(|(index, (label, kind))| {
                let selected = self.kind == kind;
                div()
                    .id(("loader-segment", index))
                    .accessible(
                        Role::RadioButton,
                        format!(
                            "loader-{}",
                            kind.map_or("vanilla".to_string(), |kind| kind.label().to_lowercase())
                        ),
                        label,
                    )
                    .aria_toggled(if selected {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .keyboard_focus()
                    .px_2p5()
                    .py_1()
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .font_weight(weights::BOLD.into())
                    .text_size(theme::text_size(text::SMALL))
                    .map(|this| {
                        if selected {
                            this.bg(colors::GRAY_700)
                                .text_color(crate::theme::ink::text())
                        } else {
                            this.text_color(crate::theme::ink::secondary())
                                .hover(|style| style.text_color(crate::theme::ink::text()))
                        }
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_kind(kind, cx)))
            }))
    }

    /// The chosen version in a dropdown, or what the list is doing.
    fn version_control(&self, kind: LoaderKind, game: &str, cx: &mut Context<Self>) -> AnyElement {
        let note = |text_: String, color: theme::Color| {
            div()
                .max_w(px(300.))
                .text_size(theme::text_size(text::SMALL))
                .text_color(color)
                .child(text_)
                .into_any_element()
        };
        let list = match self.versions(cx) {
            LoaderVersions::NotLoaded | LoaderVersions::Loading => {
                return div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(crate::theme::ink::muted())
                    .child(LoadingDots)
                    .child(tr!(M::LoadingLoaderVersions, loader = kind.label()))
                    .into_any_element();
            }
            LoaderVersions::Failed(error) => {
                return note(tr!(M::CouldntLoadVersions, error = error), colors::RED_200);
            }
            LoaderVersions::Ready(list) if list.is_empty() => {
                return note(tr!(M::NoVersionsFor, game = game), colors::GRAY_500);
            }
            LoaderVersions::Ready(list) => list,
        };
        let chosen = self
            .selected
            .as_ref()
            .and_then(|id| list.iter().find(|v| &v.id == id));
        let chosen_text: SharedString = chosen.map_or_else(
            || SharedString::from(t(M::ChooseVersion)),
            |v| v.display.clone().into(),
        );
        let toggle = {
            let picker = cx.entity().downgrade();
            move |_: &mut Window, cx: &mut gpui::App| {
                picker
                    .update(cx, |this, cx| {
                        this.open = !this.open;
                        cx.notify();
                    })
                    .ok();
            }
        };
        let label = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .font_weight(weights::BOLD.into())
                    .child(chosen.map_or_else(
                        || SharedString::from(t(M::ChooseVersion)),
                        |v| v.display.clone().into(),
                    )),
            )
            .when_some(chosen.and_then(version_tag), |this, (text_, color)| {
                this.child(tag(text_, color))
            });
        let rows = list.len();
        let panel_height = (rows as f32 * 28. + 10.).min(260.);
        // In the dialog's form the dropdown lines up on the left; in the Settings
        // rows, on the right.
        let form = !self.compact;
        div()
            .flex()
            .flex_col()
            .map(|this| {
                if form {
                    this.items_start()
                } else {
                    this.items_end()
                }
            })
            .child(
                select_button("loader-version-select", label)
                    .aria_label(t(M::LoaderVersion))
                    .aria_value(chosen_text)
                    .expandable(self.open, toggle)
                    .when(form, |this| this.w(px(300.)).justify_between())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open = !this.open;
                        cx.notify();
                    })),
            )
            .when(self.open, |this| {
                let close = cx.listener(|this, _: &(), _, cx| {
                    this.open = false;
                    cx.notify();
                });
                let popover = popover(
                    "loader-version-popover",
                    popover_panel("loader-version-list")
                        .w(px(if form { 300. } else { 280. }))
                        .h(px(panel_height))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open = false;
                            cx.notify();
                        }))
                        .child(
                            uniform_list(
                                "loader-versions",
                                rows,
                                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .filter_map(|index| list.get(index).map(|v| (index, v)))
                                        .map(|(index, version)| {
                                            let selected =
                                                this.selected.as_deref() == Some(&version.id);
                                            let id = version.id.clone();
                                            version_row(index, version, selected).on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.selected = Some(id.clone());
                                                    this.touched = true;
                                                    this.open = false;
                                                    cx.notify();
                                                }),
                                            )
                                        })
                                        .collect()
                                }),
                            )
                            .track_scroll(&self.scroll)
                            .size_full(),
                        ),
                    move |window, cx| close(&(), window, cx),
                );
                // In the form, in a box of its own under the button: the popover
                // starts where its parent does.
                if form {
                    this.child(div().child(popover))
                } else {
                    this.child(popover)
                }
            })
            .into_any_element()
    }

    fn render_compact(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let version = match (self.kind, self.game_version.clone()) {
            (Some(kind), Some(game)) => Some(self.version_control(kind, &game, cx)),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .child(SettingRow::new(t(M::ModLoader)).child(self.segmented(cx)))
            .when_some(version, |this, version| {
                this.child(
                    div()
                        .border_t_1()
                        .border_color(crate::theme::ink::border())
                        .child(SettingRow::new(t(M::LoaderVersion)).child(version)),
                )
            })
    }
}

/// "Recommended" or "Beta", for a version.
fn version_tag(version: &LoaderVersion) -> Option<(&'static str, theme::Color)> {
    if version.recommended {
        Some((t(M::Recommended), colors::GREEN_300))
    } else if !version.stable {
        Some((t(M::Beta), colors::YELLOW))
    } else {
        None
    }
}

/// "Vanilla" and the loaders, with their names.
fn loader_options() -> impl Iterator<Item = (&'static str, Option<LoaderKind>)> {
    std::iter::once((t(M::Vanilla), None)).chain(LoaderKind::ALL.map(|k| (k.label(), Some(k))))
}

impl Render for LoaderPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.compact {
            return self.render_compact(cx).into_any_element();
        }
        // The dialog's form: only the version dropdown, for a mod loader.
        match (self.kind, self.game_version.clone()) {
            (Some(kind), Some(game)) => self.version_control(kind, &game, cx),
            _ => div().into_any_element(),
        }
    }
}
