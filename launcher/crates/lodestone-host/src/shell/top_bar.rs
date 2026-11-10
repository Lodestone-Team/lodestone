//! The top bar, after the dashboard's `TopNav`: the logo, with a DEV badge in
//! development builds (and an amber line under the bar), and on the right the
//! notifications (work package D). The bar moves the window.
//!
//! On macOS the logo is centred and the window's traffic lights sit in the left
//! column. On Windows the bar replaces the title bar, as the launcher's does: the logo
//! sits on the left, where Windows apps put their icon, the window's caption buttons
//! ([`window_controls`]) end the bar, and a right click opens the window's system
//! menu. Windows itself moves the window from the bar's empty areas (they answer its
//! hit test as the caption), so a double click maximizes and dragging snaps.

use gpui::{
    AnyElement, Context, IntoElement, MouseButton, ParentElement, Styled, Window,
    WindowControlArea, div, img, prelude::*,
};
use lodestone_ui::{
    components::caption,
    theme::{colors, layout, px, radius, text, weights},
};

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
    let windows = cfg!(target_os = "windows");
    let side = || div().flex_1().flex_basis(px(0.)).h_full();
    div()
        .id("top-bar")
        .window_control_area(WindowControlArea::Drag)
        // Move the window when the bar is dragged, as the launcher's bar does: a mouse
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
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        .h(gpui::px(TOP_BAR_HEIGHT + 1.0))
        .pl_4()
        // On Windows the caption buttons end the bar, flush with the window's edge.
        .when(!windows, |bar| bar.pr_4())
        .bg(colors::GRAY_800)
        .border_b_1()
        .border_color(if IS_DEV {
            colors::YELLOW
        } else {
            colors::GRAY_FADED.opacity(30)
        })
        // On Windows a right click opens the window's system menu, as on the native
        // title bar. Taking the press keeps Windows from tracking it as a caption
        // click, which would swallow the release.
        .when(windows, |bar| {
            bar.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Right, |event, window, cx| {
                    caption::system_menu::show(window, event.position, cx)
                })
        })
        // On macOS equal side columns keep the logo centred, the left one under the
        // traffic lights.
        .when(!windows, |bar| bar.child(side()))
        .child(
            div()
                .id("logo")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                // The logo is not part of the caption: Windows doesn't move the window
                // from it, or open the system menu on it.
                .when(windows, |logo| logo.occlude())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Home, cx)))
                .child(img(super::logo()).w(layout::logo_width()).h(gpui::px(26.)))
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
                .child(notifications)
                // Room for the caption buttons, which `window_controls` draws above
                // everything else, with a gap before them.
                .when(windows, |side| {
                    side.child(
                        div()
                            .flex_none()
                            .w(gpui::px(caption::BUTTONS_WIDTH + caption::GAP)),
                    )
                }),
        )
}

/// Windows: a bare bar for the screens before the shell (the core starting, signing
/// in, the password), which have no top bar: it moves the window, opens the system
/// menu on a right click, and leaves room for the caption buttons. `None` elsewhere.
pub fn frame_bar() -> Option<AnyElement> {
    cfg!(target_os = "windows").then(|| {
        div()
            .id("frame-bar")
            .window_control_area(WindowControlArea::Drag)
            .flex_none()
            .w_full()
            .h(gpui::px(TOP_BAR_HEIGHT))
            .bg(colors::GRAY_900)
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Right, |event, window, cx| {
                caption::system_menu::show(window, event.position, cx)
            })
            .into_any_element()
    })
}

/// Windows: the window's caption buttons, which replace the title bar's. The window's
/// root adds them last, over the top bar, which leaves room for them, so they stay
/// above dialogs and keep working while one is open. Nothing elsewhere.
pub fn window_controls(window: &Window) -> Option<AnyElement> {
    cfg!(target_os = "windows").then(|| caption::buttons(window, gpui::px(TOP_BAR_HEIGHT)))
}
