use crate::skin::Skinned;
use crate::theme::px;
use launcher_core::ui_skin::SurfaceId;

use gpui::{
    AnyElement, App, Context, IntoElement, MouseButton, Window, WindowControlArea, div, img,
    prelude::*,
};

use super::{LauncherShell, Page};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, KeyboardFocus,
        a11y::{Accessible, Role},
        caption, text_tooltip,
        window_frame::{self, Side},
    },
    i18n::{ShellMsg as M, t},
    theme::{self, colors, layout, radius, text},
    updater::UpdateState,
};

/// Whether this is a development build (`cargo build`): the logo has a DEV badge and
/// the bar an amber line under it, as in Lodestone Host.
const IS_DEV: bool = cfg!(debug_assertions);

/// The top bar's height without its bottom border: `py-2` around the dashboard's
/// 26px notification button, which sets the row height there.
pub const TOP_BAR_HEIGHT: f32 = 42.0;

/// The top bar, after the dashboard's `TopNav`. It replaces the title bar.
///
/// On macOS the logo is centred, the window's traffic-light buttons sit in the left
/// column, followed by the back and forward arrows, and the empty areas move the
/// window, as in the dashboard app. The right column holds the update notice.
///
/// On Windows the logo sits on the left, where Windows apps put their icon, with the
/// arrows after it, and the
/// right end leaves room for the caption buttons of [`window_controls`]. A centred
/// logo would leave too little room beside it, at the narrowest window, for the
/// update notice and the buttons. Windows itself moves the window from the bar's
/// empty areas (they answer its hit test as the caption), so a double click there
/// maximizes and dragging snaps to screen edges. A right click there opens the
/// window's system menu (see [`caption::system_menu`]).
///
/// On Linux desktops that leave the window's frame to the app (GNOME), the logo is
/// centred as on macOS, and the side columns leave room for the window's buttons
/// where the desktop puts them (see [`window_frame`] and [`window_controls`]). A
/// double click maximizes, and a right click opens the window menu.
pub fn render(
    shell: &LauncherShell,
    window: &Window,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let windows = cfg!(target_os = "windows");
    // On macOS and Linux, equal side columns keep the logo centred in the window.
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
        .pl_4()
        // On Windows the caption buttons end the bar, flush with the window's edge.
        .when(!windows, |bar| bar.pr_4())
        .bg(colors::GRAY_800)
        .map(|this| window_frame::fit_top_corners(this, window))
        .border_b_1()
        .border_color(if IS_DEV {
            colors::YELLOW
        } else {
            colors::GRAY_FADED.opacity(30)
        })
        .surface(SurfaceId::TopBar)
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
        .on_press(move |event, window, _| {
            if event.click_count() == 2 {
                if double_click_zooms {
                    window.zoom_window();
                } else {
                    window.titlebar_double_click();
                }
            }
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
        // With its own frame on Linux, the compositor opens the window menu. It opens
        // on the press, as Zed's does: Wayland asks for the press that opened it.
        .when(own_frame && offered.window_menu, |bar| {
            bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position)
            })
        })
        // The back and forward arrows start the bar: after the window's buttons on
        // macOS and Linux, after the logo on Windows, where the logo starts it.
        .when(!windows, |bar| {
            bar.child(
                side()
                    .flex()
                    .flex_row()
                    .items_center()
                    .children(controls_room(Side::Left, window, cx))
                    // past macOS's traffic lights, which start 16px in and are 52px
                    // wide, as the performance monitor's bar leaves them
                    .when(cfg!(target_os = "macos"), |this| {
                        this.child(div().flex_none().w(gpui::px(MAC_TRAFFIC_LIGHTS)))
                    })
                    .child(history_arrows(shell, cx)),
            )
        })
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .child(logo(windows || own_frame, cx))
                .when(IS_DEV, |this| this.child(dev_badge()))
                .when(windows, |this| {
                    this.child(div().ml(px(8.)).child(history_arrows(shell, cx)))
                }),
        )
        .child(
            side()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .children(update_notice(shell, cx))
                // Room for the window's buttons, which `window_controls` draws above
                // everything else, with a gap before them.
                .children(controls_room(Side::Right, window, cx)),
        )
}

/// The logo, which opens the instance page. `occlude`: it isn't part of the window's
/// caption (Windows, or the launcher's own frame on Linux).
fn logo(occlude: bool, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .id("logo")
        .accessible(Role::Button, "logo", "Lodestone Launcher")
        .keyboard_focus()
        .rounded(radius::DEFAULT)
        .flex_none()
        .cursor_pointer()
        // The logo is not part of the caption: a right click on it doesn't
        // open Windows' system menu or the compositor's window menu.
        .when(occlude, |logo| logo.occlude())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_press(cx.listener(|this, _, _, cx| this.show(Page::Instance, cx)))
        .child(
            img(crate::images::logo(cx))
                .w(layout::logo_width())
                // The logo's viewBox is 106 by 22.
                .h(layout::logo_width() * (22.0 / 106.0)),
        )
}

/// macOS: the room the traffic lights take after the bar's left padding, with a gap
/// before the arrows (the lights end 68px in; the arrows start at 84px).
const MAC_TRAFFIC_LIGHTS: f32 = 68.0;

/// The back and forward arrows: the place shown before, and back again (see
/// [`super::history`]). Each is greyed out while there is nowhere to go.
fn history_arrows(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let (back_keys, forward_keys) = if cfg!(target_os = "macos") {
        ("⌘[", "⌘]")
    } else {
        ("Alt+Left", "Alt+Right")
    };
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(2.))
        .child(arrow(
            "history-back",
            Icon::ChevronLeft,
            t(M::GoBack),
            back_keys,
            shell.can_go_back(),
            cx.listener(|this, _, _, cx| this.go_back(cx)),
        ))
        .child(arrow(
            "history-forward",
            Icon::ChevronRight,
            t(M::GoForward),
            forward_keys,
            shell.can_go_forward(),
            cx.listener(|this, _, _, cx| this.go_forward(cx)),
        ))
}

/// One of the [`history_arrows`]. It isn't part of the window's caption: pressing it
/// doesn't move the window, and on Windows it takes the click instead of the caption.
fn arrow(
    id: &'static str,
    icon: Icon,
    label: &'static str,
    keys: &'static str,
    enabled: bool,
    on_press: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let color = if enabled {
        crate::theme::ink::secondary()
    } else {
        crate::theme::ink::muted().opacity(30)
    };
    div()
        .id(id)
        .accessible(Role::Button, id, label)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(22.))
        .rounded(radius::DEFAULT)
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .tooltip(text_tooltip(format!("{label} ({keys})")))
        .when(enabled, |this| {
            this.keyboard_focus()
                .cursor_pointer()
                .hover(|style| style.bg(colors::GRAY_700))
                .active(|style| style.bg(colors::GRAY_850))
                .on_press(on_press)
        })
        .child(icon.render(px(11.), color))
}

/// The DEV badge beside the logo in a development build, as Lodestone Host's.
fn dev_badge() -> impl IntoElement {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(radius::DEFAULT)
        .bg(colors::YELLOW)
        .text_color(colors::GRAY_900)
        .text_size(theme::text_size(text::SMALL))
        .font_weight(theme::weights::EXTRABOLD.face())
        .child(t(M::DevBadge))
}

/// Linux: the gap between the round buttons of the launcher's own frame and the bar's
/// other controls.
const FRAME_BUTTONS_GAP: f32 = 12.0;

/// An empty block in `side`'s column of the bar, as wide as the window's buttons that
/// [`window_controls`] draws over that end of the bar, and the gap before them.
pub(crate) fn controls_room(
    side: Side,
    window: &Window,
    cx: &App,
) -> Option<impl IntoElement + use<>> {
    let width = if cfg!(target_os = "windows") {
        (side == Side::Right).then_some(gpui::px(caption::BUTTONS_WIDTH + caption::GAP))
    } else {
        window_frame::controls_width(side, window, cx)
            .map(|buttons| buttons + gpui::px(FRAME_BUTTONS_GAP))
    }?;
    Some(div().flex_none().w(width))
}

/// The window's own buttons, where the launcher draws them in place of a title bar's:
/// Windows' caption buttons (see [`caption::buttons`]), or the buttons of the
/// launcher's own frame on Linux desktops that leave it to apps (see
/// [`window_frame::controls`]). Nothing on macOS, or where the Linux desktop draws the
/// frame.
///
/// The window's root adds them last, over the top bar, which leaves room for them (see
/// `controls_room`). So they stay above dialogs' backdrops and keep working while a
/// dialog is open, as macOS's traffic lights do.
pub fn window_controls(window: &Window, cx: &App) -> Vec<AnyElement> {
    if cfg!(target_os = "windows") {
        return vec![caption::buttons(window, gpui::px(TOP_BAR_HEIGHT))];
    }
    [Side::Left, Side::Right]
        .into_iter()
        .filter_map(|side| {
            let row = window_frame::controls(side, window, cx)?;
            Some(
                div()
                    .absolute()
                    .top_0()
                    // Inside the bar's padding, where its own controls end.
                    .map(|this| match side {
                        Side::Left => this.left_4(),
                        Side::Right => this.right_4(),
                    })
                    .flex()
                    .items_center()
                    .h(gpui::px(TOP_BAR_HEIGHT))
                    .child(row)
                    .into_any_element(),
            )
        })
        .collect()
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
            .accessible(Role::Status, "update-notice", label)
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
            .border_color(crate::theme::ink::border())
            // Clicks here don't move the window: the notice is not part of the caption
            // (on Windows the bar answers the hit test as the caption, which swallows
            // the click), and pressing it doesn't arm the bar's drag.
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().size(px(6.)).rounded_full().bg(colors::GREEN_400))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(crate::theme::ink::text())
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
