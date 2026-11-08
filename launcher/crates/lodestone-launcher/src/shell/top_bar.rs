use gpui::{Context, IntoElement, MouseButton, WindowControlArea, div, img, prelude::*, px};

use super::{LauncherShell, Page};
use crate::theme::{colors, layout};

/// The top bar's height without its bottom border: `py-2` around the dashboard's
/// 26px notification button, which sets the row height there.
pub const TOP_BAR_HEIGHT: f32 = 42.0;

/// The top bar with the Lodestone logo centred, after the dashboard's `TopNav`.
///
/// On macOS it replaces the title bar: the window's traffic-light buttons sit in its
/// left column and its empty areas move the window, as in the dashboard app.
pub fn render(cx: &mut Context<LauncherShell>) -> impl IntoElement {
    // Equal side columns keep the logo centred in the window.
    let side = || div().flex_1().flex_basis(px(0.)).h_full();

    div()
        .id("top-bar")
        .window_control_area(WindowControlArea::Drag)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        .h(px(TOP_BAR_HEIGHT + 1.0))
        .px_4()
        .bg(colors::GRAY_800)
        .border_b_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        // Move the window when the bar is dragged, as Zed's title bar does: a mouse
        // down arms the drag and the first move starts it, so clicks still work.
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, _| this.top_bar_drag_pending = true),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, _| this.top_bar_drag_pending = false),
        )
        .on_mouse_down_out(cx.listener(|this, _, _, _| this.top_bar_drag_pending = false))
        .on_mouse_move(cx.listener(|this, _, window, _| {
            if this.top_bar_drag_pending {
                this.top_bar_drag_pending = false;
                window.start_window_move();
            }
        }))
        .on_click(|event, window, _| {
            if event.click_count() == 2 {
                window.titlebar_double_click();
            }
        })
        .child(side())
        .child(
            div()
                .id("logo")
                .flex_none()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| this.show(Page::Instance, cx)))
                .child(
                    img(crate::images::logo(cx))
                        .w(layout::LOGO_WIDTH)
                        // The logo's viewBox is 106 by 22.
                        .h(layout::LOGO_WIDTH * (22.0 / 106.0)),
                ),
        )
        .child(side())
}
