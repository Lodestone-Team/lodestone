use gpui::{Context, IntoElement, div, prelude::*, px};

use super::{LauncherShell, Page, account_box};
use crate::{
    components::{Align, Button, ButtonVariant, Icon},
    theme::{self, colors, layout, leading, radius, text, weights},
};

/// The left sidebar: the account box at the top, the instance list and "New
/// instance" in the middle, and Settings pinned at the bottom.
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .id("sidebar")
        .flex()
        .flex_col()
        .flex_none()
        .w(layout::SIDEBAR_WIDTH)
        .h_full()
        .bg(colors::GRAY_850)
        .border_r_1()
        .border_color(colors::FADE_700)
        .child(
            div()
                .flex_none()
                .px_2()
                .pt_3()
                .pb_4()
                .child(account_box::render(shell, cx)),
        )
        .child(
            div()
                .id("sidebar-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_2()
                .child(instance_list(cx)),
        )
        .child(footer(shell, cx))
}

/// The instances, each a row with its icon, name and version; empty for now.
fn instance_list(cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_y_1()
        .mx_1()
        .px_1()
        .child(section_label("INSTANCES"))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .mt_2()
                .py_4()
                .px_6()
                .rounded(radius::XL)
                .border_2()
                .border_dashed()
                .border_color(colors::GRAY_FADED.opacity(10))
                .child(Icon::Expand.render(px(28.), colors::GRAY_FADED.opacity(30)))
                .child(
                    div()
                        .text_center()
                        .italic()
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_FADED.opacity(30))
                        .child("No instances yet."),
                ),
        )
        .child(
            div().w_full().pt_1().pb_8().child(
                Button::new("new-instance", "New instance...")
                    .variant(ButtonVariant::Ghost)
                    .icon(Icon::SquarePlus)
                    .align(Align::Start)
                    .full_width()
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_new_instance(window, cx)),
                    ),
            ),
        )
}

/// A small uppercase heading, like the dashboard's "ALL INSTANCES".
fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::SNUG))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(label)
}

/// Settings, pinned to the bottom, and the version.
fn footer(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let settings_open = shell.page == Page::Settings;
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap_1()
        .w_full()
        .px_2()
        .py_2()
        .border_t_1()
        .border_color(colors::FADE_700)
        .child(
            div()
                .id("settings")
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .py_1p5()
                .rounded(radius::MD)
                .cursor_pointer()
                .font_weight(weights::BOLD.into())
                .map(|this| {
                    if settings_open {
                        this.bg(colors::GRAY_800).text_color(colors::GRAY_300)
                    } else {
                        this.text_color(colors::GRAY_400)
                            .hover(|style| style.bg(colors::GRAY_800).text_color(colors::GRAY_300))
                    }
                })
                .child(Icon::Gear.render(px(14.), colors::GRAY_FADED.opacity(50)))
                .child("Settings")
                .on_click(cx.listener(move |this, _, _, cx| {
                    let page = if settings_open {
                        Page::Instance
                    } else {
                        Page::Settings
                    };
                    this.show(page, cx);
                })),
        )
        .child(
            div()
                .px_2()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::MEDIUM.into())
                .text_color(colors::GRAY_FADED.opacity(30))
                .child(concat!("Lodestone Launcher v", env!("CARGO_PKG_VERSION"))),
        )
}
