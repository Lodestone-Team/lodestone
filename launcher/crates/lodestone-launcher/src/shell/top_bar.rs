use crate::theme::px;
use gpui::{
    AnyElement, Context, IntoElement, MouseButton, Window, WindowControlArea, div, img, prelude::*,
};

use super::{LauncherShell, Page};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, KeyboardFocus,
        window_frame::{self, Side},
    },
    i18n::{ShellMsg as M, t},
    theme::{self, colors, layout, radius, text},
    updater::UpdateState,
};

/// The top bar's height without its bottom border: `py-2` around the dashboard's
/// 26px notification button, which sets the row height there.
pub const TOP_BAR_HEIGHT: f32 = 42.0;

/// The top bar with the Lodestone logo centred, after the dashboard's `TopNav`.
///
/// On macOS it replaces the title bar: the window's traffic-light buttons sit in its
/// left column and its empty areas move the window, as in the dashboard app. The
/// right column holds the update notice.
///
/// On Linux desktops that leave the window's frame to the app (GNOME), it is the title
/// bar too: it holds the window's buttons where the desktop puts them (see
/// [`window_frame`]), a double click maximizes, and a right click opens the window
/// menu.
pub fn render(
    shell: &LauncherShell,
    window: &Window,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    // Equal side columns keep the logo centred in the window.
    let side = || div().flex_1().flex_basis(px(0.)).h_full();
    let own_frame = window_frame::client_tiling(window).is_some();
    let offered = window.window_controls();
    let double_click_zooms = own_frame && offered.maximize && window.is_resizable();

    div()
        .id("top-bar")
        .window_control_area(WindowControlArea::Drag)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        // The bar keeps its height at any zoom: the window's buttons sit in it.
        .h(gpui::px(TOP_BAR_HEIGHT + 1.0))
        .px_4()
        .bg(colors::GRAY_800)
        .map(|this| window_frame::fit_top_corners(this, window))
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
        .on_click(move |event, window, _| {
            if event.click_count() == 2 {
                if double_click_zooms {
                    window.zoom_window();
                } else {
                    window.titlebar_double_click();
                }
            }
        })
        .when(own_frame && offered.window_menu, |this| {
            this.on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position)
            })
        })
        .child(
            side()
                .flex()
                .flex_row()
                .items_center()
                .children(window_frame::controls(Side::Left, window, cx)),
        )
        .child(
            div()
                .id("logo")
                .keyboard_focus()
                .rounded(radius::DEFAULT)
                .flex_none()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| this.show(Page::Instance, cx)))
                .child(
                    img(crate::images::logo(cx))
                        .w(layout::logo_width())
                        // The logo's viewBox is 106 by 22.
                        .h(layout::logo_width() * (22.0 / 106.0)),
                ),
        )
        .child(
            side()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .children(update_notice(shell, cx))
                .children(window_frame::controls(Side::Right, window, cx)),
        )
}

/// "Update ready" with a Restart button once an update is installed, or "Update
/// available" with a Download button while "Update automatically" is off. Restart
/// waits while a game runs.
fn update_notice(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> Option<AnyElement> {
    let updater = shell.updater().read(cx);
    let (label, action, enabled) = match updater.state() {
        UpdateState::Ready { .. } => (
            t(M::UpdateReadyBadge),
            t(M::RestartToUpdate),
            updater.can_restart(cx),
        ),
        UpdateState::Available(_) => (t(M::UpdateAvailableBadge), t(M::DownloadUpdate), true),
        _ => return None,
    };
    let ready = matches!(updater.state(), UpdateState::Ready { .. });
    let updater = shell.updater().clone();
    Some(
        div()
            .id("update-notice")
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap_2()
            // The button sits 3px inside the pill, so their ends follow each other.
            .pl_3()
            .pr(px(3.))
            .py(px(3.))
            .rounded_full()
            .bg(colors::GRAY_850)
            .border_1()
            .border_color(colors::FADE_700)
            // Clicks here don't move the window.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().size(px(6.)).rounded_full().bg(colors::GREEN_400))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::GRAY_300)
                    .whitespace_nowrap()
                    .child(label),
            )
            .child(
                Button::new("update-action", action)
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Slim)
                    .pill()
                    .disabled(!enabled)
                    .on_click(move |_, _, cx| {
                        updater.update(cx, |updater, cx| {
                            if ready {
                                updater.restart(cx);
                            } else {
                                updater.download_available(cx);
                            }
                        })
                    }),
            )
            .into_any_element(),
    )
}
