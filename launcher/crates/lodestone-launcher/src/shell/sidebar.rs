use gpui::{Context, IntoElement, div, prelude::*, px};

use super::{LauncherShell, Page};
use crate::{
    components::{Align, Button, ButtonVariant, Icon, IconButton},
    theme::{self, colors, layout, leading, radius, text, weights},
};

/// The left sidebar, after the dashboard's `LeftNav`: the account box and settings
/// gear, the instance list and the "New instance" button, and a status footer.
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
                .id("sidebar-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_2()
                .pt_10()
                .child(account_box(shell, cx))
                .child(instance_list(cx)),
        )
        .child(footer())
}

/// The dashboard's `UserMenu` row: an account button and the settings gear.
fn account_box(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let settings_open = shell.page == Page::Settings;
    div()
        .flex()
        .flex_row()
        .gap_2()
        .w_full()
        .mb_6()
        .px_2()
        .child(
            div().flex_1().min_w_0().child(
                // Accounts come in a later milestone; this is a placeholder.
                Button::new("sign-in", "Sign in")
                    .icon(Icon::User)
                    .align(Align::Between)
                    .full_width()
                    .disabled(true),
            ),
        )
        .child(
            IconButton::new("settings", Icon::Gear).on_click(cx.listener(move |this, _, _, cx| {
                let page = if settings_open {
                    Page::Home
                } else {
                    Page::Settings
                };
                this.show(page, cx);
            })),
        )
}

/// The dashboard's `InstanceList`, empty until instances exist.
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

/// The bar at the bottom of the sidebar, where the dashboard shows the core's status.
fn footer() -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .w_full()
        .px_4()
        .py_2()
        .border_t_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::MEDIUM.into())
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(concat!("Lodestone Launcher v", env!("CARGO_PKG_VERSION")))
}
