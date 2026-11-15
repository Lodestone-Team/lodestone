//! Windows: the window's own caption buttons and system menu, for an app whose top bar
//! replaces the system's title bar (the launcher's and Lodestone Host's).
//!
//! The app hides the title bar (`TitlebarOptions::appears_transparent` on Windows),
//! marks its top bar as the caption (`WindowControlArea::Drag`), leaves
//! [`BUTTONS_WIDTH`] and [`GAP`] free at the bar's right end, and adds
//! [`buttons`] last in the window's root, over the bar. A right click on the bar
//! opens [`system_menu::show`].

use std::sync::OnceLock;

use gpui::{AnyElement, IntoElement, Pixels, Window, WindowControlArea, div, prelude::*};

use crate::{
    components::a11y::{Accessible, AccessibleAction, Role},
    i18n::{CommonMsg, t},
    theme::{self, colors},
};

/// The width of one caption button. Windows draws them 46px wide.
pub const BUTTON_WIDTH: f32 = 46.0;

/// The minimize, maximize and close buttons, side by side.
pub const BUTTONS_WIDTH: f32 = 3.0 * BUTTON_WIDTH;

/// The gap between the caption buttons and the bar's other controls.
pub const GAP: f32 = 8.0;

/// The minimize, maximize (or restore) and close buttons at the top right, in place of
/// the system's title bar, `height` tall (the top bar's height above its bottom
/// border).
///
/// They are marked as the window's caption buttons, so Windows handles them as its
/// own: their clicks, and the snap layouts shown on hovering maximize.
pub fn buttons(window: &Window, height: Pixels) -> AnyElement {
    let maximize = if window.is_maximized() {
        CaptionButton::Restore
    } else {
        CaptionButton::Maximize
    };
    let active = window.is_window_active();
    div()
        .id("window-controls")
        .absolute()
        .top_0()
        .right_0()
        .flex()
        .flex_row()
        .h(height)
        .font_family(caption_font())
        .children(
            [CaptionButton::Minimize, maximize, CaptionButton::Close]
                .map(|button| button.render(active)),
        )
        .into_any_element()
}

/// The icon font of Windows' own caption buttons: Segoe Fluent Icons on Windows 11,
/// or Segoe MDL2 Assets on Windows 10, which has the same glyphs.
fn caption_font() -> &'static str {
    static FONT: OnceLock<&'static str> = OnceLock::new();
    FONT.get_or_init(|| {
        const FLUENT: &str = "Segoe Fluent Icons";
        // Its file, rather than listing every installed font family, which takes a
        // while and happens during the first frame. Either font has the glyphs.
        let installed = std::env::var_os("SystemRoot")
            .or_else(|| std::env::var_os("WINDIR"))
            .map(|dir| {
                std::path::Path::new(&dir)
                    .join("Fonts")
                    .join("SegoeIcons.ttf")
            })
            .is_some_and(|file| file.is_file());
        if installed {
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
        // Windows handles their clicks itself (they are caption areas), so assistive
        // technology gets its own: Minimize and Maximize here, and Close through the
        // window's own Close (UI Automation's `WindowPattern`), which asks first.
        let name = match self {
            Self::Minimize => t(CommonMsg::MinimizeWindow),
            Self::Maximize => t(CommonMsg::MaximizeWindow),
            Self::Restore => t(CommonMsg::RestoreWindow),
            Self::Close => t(CommonMsg::Close),
        };
        div()
            .id(id)
            .accessible(Role::Button, id, name)
            .map(|this| match self {
                Self::Minimize => this.on_a11y_action(AccessibleAction::Click, |_, window, _| {
                    window.minimize_window()
                }),
                Self::Maximize | Self::Restore => this
                    .on_a11y_action(AccessibleAction::Click, |_, window, _| window.zoom_window()),
                Self::Close => this,
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .w(gpui::px(BUTTON_WIDTH))
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

/// The window's system menu (Restore, Move, Size, Minimize, Maximize, Close).
///
/// Windows opens it by itself for a right click on a native title bar, but not for an
/// app's own: the bar is in the window's client area, which `DefWindowProc` checks
/// before showing the menu. GPUI's `show_window_menu` does nothing on Windows, so the
/// app shows it itself, as Windows Terminal does.
pub mod system_menu {
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
