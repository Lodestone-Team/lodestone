use gpui::{Context, IntoElement, div, img, prelude::*, px};

use super::LauncherShell;
use crate::{
    components::{Align, Button},
    theme::{self, colors, fonts, leading, text, weights},
};

/// The empty state of the main area, after the dashboard's home page.
pub fn render(cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .items_center()
        .justify_center()
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(112.))
                .rounded_full()
                .border_1()
                .border_dashed()
                .border_color(colors::GRAY_FADED.opacity(30))
                .child(img("images/placeholder-cube.png").size(px(80.))),
        )
        .child(
            div()
                .mt_4()
                .text_size(theme::text_size(text::H2))
                .line_height(theme::line_height(text::H2, leading::NORMAL))
                .font_weight(weights::EXTRABOLD.into())
                .child("Create your first instance"),
        )
        .child(
            div()
                .font_weight(weights::BOLD.into())
                .child("Instances you create appear in the sidebar."),
        )
        .child(
            div().mt_5().child(
                Button::new("create-first-instance", "Create a new instance")
                    .align(Align::Start)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_new_instance(window, cx)),
                    ),
            ),
        )
}

/// A placeholder for the settings page, which has nothing to configure yet.
pub fn render_settings() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
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
