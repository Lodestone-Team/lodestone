//! The main area: the selected instance's header and tabs, the Play tab's hero
//! banner, and the bottom bar with the large Play button.
//!
//! Instances and launching come in Milestone 3, so for now there is no selected
//! instance and the Play button is disabled.

use gpui::{Context, IntoElement, ObjectFit, SharedString, div, img, prelude::*, px};

use super::{InstanceTab, LauncherShell};
use crate::{
    components::{Align, Button, ButtonVariant, Icon, PlayButton, Tabs},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let tab = shell.tab;
    let selected = InstanceTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            // The header: the instance's name and its tabs.
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap_3()
                .px_8()
                .pt_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_family(fonts::TITLE)
                                .text_size(theme::text_size(text::H1))
                                .line_height(theme::line_height(text::H1, leading::TIGHT))
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_300)
                                .child("No instance selected"),
                        )
                        .child(
                            div()
                                .text_color(colors::GRAY_500)
                                .font_weight(weights::BOLD.into())
                                .child(
                                    "Create an instance to choose a Minecraft version and loader.",
                                ),
                        ),
                )
                .child(
                    Tabs::new("instance-tabs", InstanceTab::ALL.map(InstanceTab::label))
                        .selected(selected)
                        .on_select({
                            let entity = cx.entity();
                            move |index, _, cx| {
                                let tab = InstanceTab::ALL[*index];
                                entity.update(cx, |this, cx| this.select_tab(tab, cx));
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .px_8()
                .py_5()
                .child(match tab {
                    InstanceTab::Play => hero(cx).into_any_element(),
                    other => placeholder(other).into_any_element(),
                }),
        )
        .child(play_bar())
}

/// The banner: the instance's art when it has some (Milestone 3), otherwise our
/// own artwork in the dashboard's colours.
fn hero(cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .relative()
        .flex()
        .flex_col()
        .justify_end()
        .size_full()
        .rounded(radius::XL)
        .overflow_hidden()
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_900)
        .child(
            img(crate::images::hero(cx))
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .gap_2()
                .p_8()
                .max_w(px(460.))
                .child(
                    div()
                        .font_family(fonts::TITLE)
                        .text_size(theme::text_size(text::XLARGE_2))
                        .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::WHITE)
                        .child("Create your first instance"),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::H3))
                        .line_height(theme::line_height(text::H3, leading::NORMAL))
                        .text_color(colors::GRAY_400)
                        .child("An instance is a Minecraft version with its own mods, worlds and settings."),
                )
                .child(
                    div().flex().mt_2().child(
                        Button::new("hero-new-instance", "New instance...")
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::SquarePlus)
                            .align(Align::Start)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_new_instance(window, cx)
                            })),
                    ),
                ),
        )
}

fn placeholder(tab: InstanceTab) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .rounded(radius::XL)
        .border_2()
        .border_dashed()
        .border_color(colors::GRAY_FADED.opacity(10))
        .text_color(colors::GRAY_FADED.opacity(30))
        .font_weight(weights::BOLD.into())
        .child(format!(
            "{} appear here once you have an instance.",
            tab.label()
        ))
}

/// A picker in the bottom bar: a caption over a value, with a caret.
fn picker(
    id: &'static str,
    caption: &'static str,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .w(px(200.))
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_800)
        .opacity(0.6)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(theme::text_size(text::CAPTION))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_500)
                        .child(caption),
                )
                .child(
                    div()
                        .truncate()
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_400)
                        .child(value.into()),
                ),
        )
        .child(Icon::CaretDown.render(px(12.), colors::GRAY_FADED.opacity(50)))
}

/// The bar under the tabs: the instance picker, Play, and the version picker.
fn play_bar() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_6()
        .w_full()
        .px_8()
        .py_4()
        .border_t_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
        .child(picker("instance-picker", "INSTANCE", "No instances"))
        .child(PlayButton::new("play").disabled(true))
        .child(picker("version-picker", "VERSION", "—"))
}

/// A placeholder for the settings page, which has nothing to configure yet.
pub fn render_settings() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .px_8()
        .pt_10()
        .child(
            div()
                .font_family(fonts::TITLE)
                .text_size(theme::text_size(text::H1))
                .line_height(theme::line_height(text::H1, leading::TIGHT))
                .font_weight(weights::BOLD.into())
                .child("Settings"),
        )
        .child(
            div()
                .text_color(colors::GRAY_FADED.opacity(30))
                .font_weight(weights::BOLD.into())
                .pt(px(2.))
                .child("There is nothing to configure yet."),
        )
}
