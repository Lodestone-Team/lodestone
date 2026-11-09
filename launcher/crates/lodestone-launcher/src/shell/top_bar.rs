use crate::theme::px;
use std::sync::OnceLock;

use gpui::{
    AnyElement, App, Context, IntoElement, MouseButton, Window, WindowControlArea, div, img,
    prelude::*,
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

/// Windows: the width of one caption button. Windows draws them 46px wide.
const CAPTION_BUTTON_WIDTH: f32 = 46.0;

/// Windows: the minimize, maximize and close buttons, side by side.
const CAPTION_BUTTONS_WIDTH: f32 = 3.0 * CAPTION_BUTTON_WIDTH;

/// The top bar, after the dashboard's `TopNav`. It replaces the title bar.
///
/// On macOS the logo is centred, the window's traffic-light buttons sit in the left
/// column and the empty areas move the window, as in the dashboard app. The right
/// column holds the update notice.
///
/// On Windows the logo sits on the left, where Windows apps put their icon, and the
/// right end leaves room for the caption buttons of [`window_controls`]. A centred
/// logo would leave too little room beside it, at the narrowest window, for the
/// update notice and the buttons. Windows itself moves the window from the bar's
/// empty areas (they answer its hit test as the caption), so a double click there
/// maximizes and dragging snaps to screen edges. A right click there opens the
/// window's system menu (see [`system_menu`]).
///
/// On Linux desktops that leave the window's frame to the app (GNOME), it holds the
/// window's buttons where the desktop puts them (see [`window_frame`]), a double click
/// maximizes, and a right click opens the window menu.
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
        // On Windows a right click opens the window's system menu, as on the native
        // title bar. Taking the press keeps Windows from tracking it as a caption
        // click, which would swallow the release.
        .when(windows, |bar| {
            bar.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Right, |event, window, cx| {
                    system_menu::show(window, event.position, cx)
                })
        })
        .when(own_frame && offered.window_menu, |this| {
            this.on_mouse_down(MouseButton::Right, |event, window, _| {
                window.show_window_menu(event.position)
            })
        })
        .when(!windows, |bar| {
            bar.child(
                side()
                    .flex()
                    .flex_row()
                    .items_center()
                    .children(window_frame::controls(Side::Left, window, cx)),
            )
        })
        .child(
            div()
                .id("logo")
                .keyboard_focus()
                .rounded(radius::DEFAULT)
                .flex_none()
                .cursor_pointer()
                // On Windows the logo is not part of the caption: a right click on it
                // doesn't open the system menu.
                .when(windows, |logo| logo.occlude())
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
                .children(window_frame::controls(Side::Right, window, cx))
                // Room for the caption buttons, which `window_controls` draws above
                // everything else, with a gap before them.
                .when(windows, |side| {
                    side.child(div().flex_none().w(gpui::px(CAPTION_BUTTONS_WIDTH + 8.0)))
                }),
        )
}

/// Windows: the minimize, maximize (or restore) and close buttons at the top right,
/// in place of the system's title bar. `None` elsewhere.
///
/// They are marked as the window's caption buttons, so Windows handles them as its
/// own: their clicks, and the snap layouts shown on hovering maximize. The window's
/// root adds them last, so they stay above dialogs' backdrops and keep working while
/// a dialog is open, as macOS's traffic lights do.
pub fn window_controls(window: &Window, cx: &App) -> Option<AnyElement> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let maximize = if window.is_maximized() {
        CaptionButton::Restore
    } else {
        CaptionButton::Maximize
    };
    let active = window.is_window_active();
    Some(
        div()
            .id("window-controls")
            .absolute()
            .top_0()
            .right_0()
            .flex()
            .flex_row()
            // The buttons fill the bar above its bottom border, at any zoom.
            .h(gpui::px(TOP_BAR_HEIGHT))
            .font_family(caption_font(cx))
            .children(
                [CaptionButton::Minimize, maximize, CaptionButton::Close]
                    .map(|button| button.render(active)),
            )
            .into_any_element(),
    )
}

/// The icon font of Windows' own caption buttons: Segoe Fluent Icons on Windows 11,
/// or Segoe MDL2 Assets on Windows 10, which has the same glyphs.
fn caption_font(cx: &App) -> &'static str {
    static FONT: OnceLock<&'static str> = OnceLock::new();
    FONT.get_or_init(|| {
        const FLUENT: &str = "Segoe Fluent Icons";
        let names = cx.text_system().all_font_names();
        if names.iter().any(|name| name == FLUENT) {
            FLUENT
        } else {
            "Segoe MDL2 Assets"
        }
    })
}

#[derive(Clone, Copy)]
enum CaptionButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl CaptionButton {
    fn render(self, active: bool) -> impl IntoElement {
        let (id, glyph, area) = match self {
            Self::Minimize => ("minimize", "\u{e921}", WindowControlArea::Min),
            Self::Maximize => ("maximize", "\u{e922}", WindowControlArea::Max),
            Self::Restore => ("restore", "\u{e923}", WindowControlArea::Max),
            Self::Close => ("close", "\u{e8bb}", WindowControlArea::Close),
        };
        // Windows 11's colours: a faint white over the bar, and red behind close. There
        // is no pressed look: a press on the window's top edge above a button resizes
        // the window, and Windows keeps its release, so the look would stay on.
        let hover = match self {
            Self::Close => CLOSE_RED,
            _ => colors::WHITE.opacity(6),
        };
        // Windows greys the glyphs out while the window is inactive.
        let glyph_color = if active {
            colors::GRAY_300
        } else {
            colors::GRAY_500
        };
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .w(gpui::px(CAPTION_BUTTON_WIDTH))
            .h_full()
            // The mouse is over the button and not the bar under it, so Windows hears
            // that this is a caption button rather than the caption.
            .occlude()
            .window_control_area(area)
            .text_size(gpui::px(10.))
            .text_color(glyph_color)
            .hover(|style| style.bg(hover).text_color(colors::WHITE))
            .child(glyph)
    }
}

/// The background of Windows 11's close button under the mouse.
const CLOSE_RED: theme::Color = theme::Color::rgb(0xC42B1C);

/// Windows: the window's system menu (Restore, Move, Size, Minimize, Maximize, Close).
///
/// Windows opens it by itself for a right click on a native title bar, but not for
/// this one: the bar is in the window's client area, which `DefWindowProc` checks
/// before showing the menu. GPUI's `show_window_menu` does nothing on Windows, so the
/// launcher shows it itself, as Windows Terminal does.
mod system_menu {
    use gpui::{App, Pixels, Point, Window};

    /// Opens the menu at `position` in the window, once the current event is done.
    #[cfg(target_os = "windows")]
    pub fn show(window: &Window, position: Point<Pixels>, cx: &mut App) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Foundation::POINT;

        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
        let scale = window.scale_factor();
        let point = POINT {
            x: (f32::from(position.x) * scale).round() as i32,
            y: (f32::from(position.y) * scale).round() as i32,
        };
        // The menu runs its own message loop until it closes. Running it from a task
        // keeps the app free meanwhile, so the window still draws.
        cx.spawn(async move |_| {
            // SAFETY: the handle is this window's, and a closed window's handle only
            // makes the calls fail.
            unsafe { track(hwnd, point) }
        })
        .detach();
    }

    #[cfg(not(target_os = "windows"))]
    pub fn show(_: &Window, _: Point<Pixels>, _: &mut App) {}

    /// Shows the menu at `point` in the window's client area, with the items that
    /// apply to its state, and carries out the chosen command.
    #[cfg(target_os = "windows")]
    unsafe fn track(
        hwnd: windows_sys::Win32::Foundation::HWND,
        mut point: windows_sys::Win32::Foundation::POINT,
    ) {
        use windows_sys::Win32::{Graphics::Gdi::ClientToScreen, UI::WindowsAndMessaging::*};

        unsafe {
            if ClientToScreen(hwnd, &mut point) == 0 {
                return;
            }
            let menu = GetSystemMenu(hwnd, 0);
            if menu.is_null() {
                return;
            }
            let maximized = IsZoomed(hwnd) != 0;
            for (command, enabled) in [
                (SC_RESTORE, maximized),
                (SC_MOVE, !maximized),
                (SC_SIZE, !maximized),
                (SC_MINIMIZE, true),
                (SC_MAXIMIZE, !maximized),
                (SC_CLOSE, true),
            ] {
                let state = if enabled { MF_ENABLED } else { MF_GRAYED };
                EnableMenuItem(menu, command, MF_BYCOMMAND | state);
            }
            SetMenuDefaultItem(menu, SC_CLOSE, 0);
            let align = if GetSystemMetrics(SM_MENUDROPALIGNMENT) != 0 {
                TPM_RIGHTALIGN
            } else {
                TPM_LEFTALIGN
            };
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | align,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            );
            if command != 0 {
                PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0);
            }
        }
    }
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
