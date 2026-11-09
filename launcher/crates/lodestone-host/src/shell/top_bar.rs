//! The top bar, after the dashboard's `TopNav`: the logo in the middle, with a DEV
//! badge in development builds (and an amber line under the bar), and on the right
//! the notifications (work package D). On macOS the window's traffic lights sit in
//! the left column and the bar moves the window.

use gpui::{
    Context, IntoElement, ParentElement, Styled, Window, WindowControlArea, div, img, prelude::*,
};
use lodestone_ui::theme::{colors, layout, px, radius, text, weights};

use super::HostShell;
use crate::notifications::Notifications;
use crate::{
    IS_DEV, TOP_BAR_HEIGHT,
    i18n::{ShellMsg, t},
    routes::Route,
};
use gpui::Entity;

pub fn render(
    notifications: Entity<Notifications>,
    _window: &Window,
    cx: &mut Context<HostShell>,
) -> impl IntoElement {
    let side = || div().flex_1().flex_basis(px(0.)).h_full();
    div()
        .id("top-bar")
        .window_control_area(WindowControlArea::Drag)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        .h(gpui::px(TOP_BAR_HEIGHT + 1.0))
        .px_4()
        .bg(colors::GRAY_800)
        .border_b_1()
        .border_color(if IS_DEV {
            colors::YELLOW
        } else {
            colors::GRAY_FADED.opacity(30)
        })
        .child(side())
        .child(
            div()
                .id("logo")
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Home, cx)))
                .child(
                    img(lodestone_ui::theme::palette::logo())
                        .w(layout::logo_width())
                        .h(gpui::px(26.)),
                )
                .when(IS_DEV, |this| {
                    this.child(
                        div()
                            .px(px(6.))
                            .rounded(radius::DEFAULT)
                            .bg(colors::YELLOW)
                            .text_color(colors::GRAY_900)
                            .text_size(text::SMALL)
                            .font_weight(weights::EXTRABOLD.face())
                            .child(t(ShellMsg::DevBadge)),
                    )
                }),
        )
        // the notifications (work package D)
        .child(
            side()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .child(notifications),
        )
}
