//! Choosing a mod loader and its version for a Minecraft version: a row of loaders
//! (Vanilla, Fabric, Quilt, Forge, NeoForge) and the chosen loader's versions, newest
//! first, with the recommended one picked. Used by "New instance" and by an
//! instance's Settings tab.

use gpui::{
    Context, Entity, IntoElement, Render, SharedString, Subscription, UniformListScrollHandle,
    Window, div, prelude::*, px, uniform_list,
};
use launcher_core::game::loaders::{LoaderChoice, LoaderKind, LoaderVersion};

use super::{Instances, LoaderVersions};
use crate::{
    components::LoadingDots,
    theme::{self, colors, radius, text, weights},
};

pub struct LoaderPicker {
    instances: Entity<Instances>,
    game_version: Option<String>,
    kind: Option<LoaderKind>,
    /// The chosen loader version's id.
    selected: Option<String>,
    list_height: f32,
    scroll: UniformListScrollHandle,
    _observe: Subscription,
}

impl LoaderPicker {
    pub fn new(
        instances: Entity<Instances>,
        game_version: Option<String>,
        initial: Option<LoaderChoice>,
        list_height: f32,
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
            list_height,
            scroll: UniformListScrollHandle::new(),
            _observe: observe,
        };
        this.load(cx);
        this
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

    fn set_kind(&mut self, kind: Option<LoaderKind>, cx: &mut Context<Self>) {
        if self.kind == kind {
            return;
        }
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
            None => Err(format!("Choose a {} version.", kind.label())),
        }
    }

    pub fn kind(&self) -> Option<LoaderKind> {
        self.kind
    }

    fn chip(
        &self,
        index: usize,
        label: &'static str,
        kind: Option<LoaderKind>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.kind == kind;
        div()
            .id(("loader", index))
            .px_3()
            .py_1()
            .rounded(radius::MD)
            .border_1()
            .cursor_pointer()
            .font_weight(weights::BOLD.into())
            .text_size(theme::text_size(text::SMALL))
            .map(|this| {
                if selected {
                    this.bg(colors::BLUE.opacity(35))
                        .border_color(colors::BLUE_200.opacity(60))
                        .text_color(colors::WHITE)
                } else {
                    this.border_color(colors::FADE_700)
                        .bg(colors::GRAY_800)
                        .text_color(colors::GRAY_400)
                        .hover(|style| style.text_color(colors::GRAY_300).bg(colors::GRAY_750))
                }
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.set_kind(kind, cx)))
    }
}

fn version_row(index: usize, version: &LoaderVersion, selected: bool) -> gpui::Stateful<gpui::Div> {
    let tag: Option<(&'static str, theme::Color)> = if version.recommended {
        Some(("Recommended", colors::GREEN_300))
    } else if !version.stable {
        Some(("Beta", colors::YELLOW))
    } else {
        None
    };
    div()
        .id(("loader-version", index))
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

impl Render for LoaderPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chips: Vec<_> = std::iter::once(("Vanilla", None))
            .chain(LoaderKind::ALL.map(|k| (k.label(), Some(k))))
            .enumerate()
            .map(|(index, (label, kind))| self.chip(index, label, kind, cx).into_any_element())
            .collect();
        let message = |text: SharedString, color: theme::Color| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .size_full()
                .p_3()
                .text_center()
                .text_size(theme::text_size(text::SMALL))
                .text_color(color)
                .child(text)
                .into_any_element()
        };
        let body = match (self.kind, &self.game_version) {
            (None, _) => message("Plain Minecraft, without mods.".into(), colors::GRAY_500),
            (Some(_), None) => {
                message("Choose a Minecraft version first.".into(), colors::GRAY_500)
            }
            (Some(kind), Some(game)) => match self.versions(cx) {
                LoaderVersions::NotLoaded | LoaderVersions::Loading => div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .size_full()
                    .text_color(colors::GRAY_500)
                    .child(LoadingDots)
                    .child(format!("Loading {} versions", kind.label()))
                    .into_any_element(),
                LoaderVersions::Failed(error) => message(
                    format!("Couldn't load the {} versions: {error}", kind.label()).into(),
                    colors::RED_200,
                ),
                LoaderVersions::Ready(list) if list.is_empty() => message(
                    format!("{} has no versions for Minecraft {game}.", kind.label()).into(),
                    colors::GRAY_500,
                ),
                LoaderVersions::Ready(list) => uniform_list(
                    "loader-versions",
                    list.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| list.get(index).map(|v| (index, v)))
                            .map(|(index, version)| {
                                let selected = this.selected.as_deref() == Some(&version.id);
                                let id = version.id.clone();
                                version_row(index, version, selected).on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.selected = Some(id.clone());
                                        cx.notify();
                                    },
                                ))
                            })
                            .collect()
                    }),
                )
                .track_scroll(&self.scroll)
                .size_full()
                .into_any_element(),
            },
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .child(div().flex().flex_row().flex_wrap().gap_1().children(chips))
            .child(
                div()
                    .h(px(self.list_height))
                    .w_full()
                    .p_1()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .bg(colors::GRAY_900)
                    .child(body),
            )
    }
}
