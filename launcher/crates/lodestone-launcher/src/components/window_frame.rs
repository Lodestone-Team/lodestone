//! The window's frame and buttons, on Linux desktops that leave them to the app.
//!
//! On Wayland, GPUI asks the compositor to draw the window's title bar and borders
//! (the xdg-decoration protocol) and, where it can't, the app has to: KDE draws them,
//! GNOME (Mutter) doesn't. A window left to itself there is a bare rectangle, with no
//! buttons, no resize edges and no shadow. So, as Zed does
//! (`workspace::client_side_decorations` and `platform_title_bar`), the launcher then
//! draws a transparent band around the window that resizes it and holds its shadow,
//! rounds its top corners, and puts minimize, maximize and close buttons in the top
//! bar ([`controls`]). On X11 GPUI asks the window manager for decorations, and macOS
//! always draws its own, so there this module changes nothing. Windows draws the
//! frame too; the launcher's top bar replaces only its title bar, with caption buttons
//! of Windows' own look (`shell::top_bar`), so this module changes nothing there either.
//!
//! Set `LODESTONE_LAUNCHER_CLIENT_DECORATIONS=1` to ask for the app's own frame on a
//! desktop that would draw one (KDE), to see it there (see `crate::window_options`).

use gpui::{
    AnyElement, App, Bounds, CursorStyle, Decorations, Div, Global, HitboxBehavior, Hsla,
    MouseButton, Pixels, Point, ResizeEdge, Size, Stateful, Styled, Tiling, Window, WindowButton,
    WindowButtonLayout, WindowControlArea, WindowControls, canvas, div, point, prelude::*, size,
};

use super::{
    Icon,
    a11y::{Accessible, Role},
    text_tooltip,
};
use crate::{
    i18n::{CommonMsg, ShellMsg, t},
    theme::{colors, shadow},
};

/// The band around the window that holds its shadow, and the width of the edges that
/// resize it. Zed's size. The frame doesn't zoom with the interface, so its sizes use
/// `gpui::px`.
pub const SHADOW: Pixels = gpui::px(10.);
/// The window's top corners: `rounded-lg`.
pub const ROUNDING: Pixels = gpui::px(8.);
/// The outline around the window, which keeps its edge visible on a dark desktop.
const BORDER: Pixels = gpui::px(1.);
/// The window buttons: round, as GNOME's are, and small enough to leave the top bar
/// its height.
const BUTTON: Pixels = gpui::px(24.);
const BUTTON_ICON: Pixels = gpui::px(14.);
const BUTTON_GAP: Pixels = gpui::px(8.);

/// The window's tiling while the launcher draws its frame, or `None` while the system
/// does. A tiled edge is against the screen's edge or another window (and all four
/// are when maximized), so it has no shadow, resize edge or rounded corners.
pub fn client_tiling(window: &Window) -> Option<Tiling> {
    if cfg!(target_os = "macos") {
        return None;
    }
    match window.window_decorations() {
        Decorations::Client { tiling } => Some(tiling),
        Decorations::Server => None,
    }
}

/// The room the window's content has: the viewport, less the frame's shadow band and
/// outline on each untiled edge while the launcher draws its own frame. Use it, not
/// `window.viewport_size()`, to size or lay out anything by the window's size.
pub fn content_size(window: &Window) -> Size<Pixels> {
    content_bounds(window).size
}

/// Where the window's content is, in window coordinates (those of mouse events):
/// inside the frame's shadow band and outline while the launcher draws its own frame,
/// or the whole viewport.
pub fn content_bounds(window: &Window) -> Bounds<Pixels> {
    let viewport = window.viewport_size();
    match client_tiling(window) {
        Some(tiling) => inside_frame(viewport, tiling),
        None => Bounds::new(Point::default(), viewport),
    }
}

/// `viewport` less what [`frame`] puts around the content for `tiling`.
fn inside_frame(viewport: Size<Pixels>, tiling: Tiling) -> Bounds<Pixels> {
    let edge = |tiled: bool| if tiled { Pixels::ZERO } else { SHADOW + BORDER };
    Bounds::new(
        point(edge(tiling.left), edge(tiling.top)),
        size(
            (viewport.width - edge(tiling.left) - edge(tiling.right)).max(Pixels::ZERO),
            (viewport.height - edge(tiling.top) - edge(tiling.bottom)).max(Pixels::ZERO),
        ),
    )
}

/// Rounds `element`'s top corners to sit inside the frame's, for the window's content
/// and anything drawn over all of it (a dialog's backdrop). Without the app's own
/// frame it is unchanged.
///
/// Only the top corners are round, as in Chrome's and Firefox's own frames on Linux:
/// GPUI clips to rectangles, so the many things that reach the bottom of the window
/// (the sidebar, the Play background, dialog backdrops) would each need their own
/// rounding to keep a round bottom corner clear.
pub fn fit_top_corners<E: Styled>(element: E, window: &Window) -> E {
    let Some(tiling) = client_tiling(window) else {
        return element;
    };
    round_top(element, tiling, ROUNDING - BORDER)
}

fn round_top<E: Styled>(mut element: E, tiling: Tiling, radius: Pixels) -> E {
    if !tiling.top && !tiling.left {
        element = element.rounded_tl(radius);
    }
    if !tiling.top && !tiling.right {
        element = element.rounded_tr(radius);
    }
    element
}

/// The resize edge the pointer was last over, so the frame redraws (and the cursor
/// changes) only when it moves onto another.
#[derive(Default)]
struct HoveredEdge(Option<ResizeEdge>);

impl Global for HoveredEdge {}

/// The window's root: `content` inside the frame, while the launcher draws one.
///
/// The structure is the same either way, so the content keeps its state when the
/// compositor takes over the frame (KDE does, shortly after the window opens).
pub fn frame(content: impl IntoElement, window: &mut Window) -> Stateful<Div> {
    let tiling = client_tiling(window);
    match tiling {
        // What the compositor counts as the window's own area: the shadow is outside.
        Some(_) => window.set_client_inset(SHADOW),
        // Reset only after a frame of our own, so macOS and Windows are never told.
        None if window
            .client_inset()
            .is_some_and(|inset| inset > Pixels::ZERO) =>
        {
            window.set_client_inset(Pixels::ZERO)
        }
        None => {}
    }
    let resizable = tiling.is_some() && window.is_resizable();

    div()
        .id("window-frame")
        .size_full()
        .when_some(tiling, |this, tiling| {
            this.when(!tiling.top, |this| this.pt(SHADOW))
                .when(!tiling.bottom, |this| this.pb(SHADOW))
                .when(!tiling.left, |this| this.pl(SHADOW))
                .when(!tiling.right, |this| this.pr(SHADOW))
                .when(resizable, |this| {
                    // Moves inside the window come here too, so leaving an edge for
                    // the content redraws, and the cursor goes back to the content's.
                    this.on_mouse_move(move |event, window, cx| {
                        let edge = resize_edge(event.position, window_size(window), tiling);
                        if cx.default_global::<HoveredEdge>().0 != edge {
                            cx.set_global(HoveredEdge(edge));
                            window.refresh();
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        move |event, window, _| {
                            if let Some(edge) =
                                resize_edge(event.position, window_size(window), tiling)
                            {
                                window.start_window_resize(edge);
                            }
                        },
                    )
                })
        })
        .child(
            div()
                .size_full()
                .when_some(tiling, |this, tiling| {
                    round_top(this, tiling, ROUNDING)
                        // Keeps the content out of the shadow band: a closing dialog
                        // rises, backdrop and all, and would otherwise draw above the
                        // window's top edge.
                        .overflow_hidden()
                        .cursor(CursorStyle::Arrow)
                        .border_color(colors::GRAY_FADED.opacity(30))
                        .when(!tiling.top, |this| this.border_t(BORDER))
                        .when(!tiling.bottom, |this| this.border_b(BORDER))
                        .when(!tiling.left, |this| this.border_l(BORDER))
                        .when(!tiling.right, |this| this.border_r(BORDER))
                        .when(!tiling.is_tiled(), |this| {
                            this.shadow(vec![
                                gpui::BoxShadow::new(
                                    gpui::px(0.),
                                    gpui::px(0.),
                                    Hsla::from(shadow::COLOR.opacity(45)),
                                )
                                .blur_radius(SHADOW / 2.),
                            ])
                        })
                })
                .child(content),
        )
        .when_some(tiling.filter(|_| resizable), |this, tiling| {
            this.child(resize_cursor(tiling))
        })
}

/// Shows the resize cursor over the edges. A hitbox over the whole window, drawn last
/// so it is on top, sets the cursor wherever the pointer is near an edge (including
/// the corners' few pixels inside the window); it does not block the pointer.
fn resize_cursor(tiling: Tiling) -> impl IntoElement {
    canvas(
        |_, window, _| {
            window.insert_hitbox(
                Bounds::new(point(Pixels::ZERO, Pixels::ZERO), window_size(window)),
                HitboxBehavior::Normal,
            )
        },
        move |_, hitbox, window, _| {
            let Some(edge) = resize_edge(window.mouse_position(), window_size(window), tiling)
            else {
                return;
            };
            let cursor = match edge {
                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            };
            window.set_cursor_style(cursor, &hitbox);
        },
    )
    .absolute()
    .size_full()
}

/// The window's size, shadow band included.
fn window_size(window: &Window) -> Size<Pixels> {
    window.window_bounds().get_bounds().size
}

/// The edge or corner at `position` in a window of `window_size`, as Zed finds it:
/// the shadow band along each untiled edge, and a square of one and a half bands at
/// each corner, so the corners are easier to catch.
fn resize_edge(
    position: Point<Pixels>,
    window_size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let corner = SHADOW * 1.5;
    if Bounds::new(Point::default(), window_size)
        .inset(corner)
        .contains(&position)
    {
        return None;
    }
    let corner_at = |x, y| Bounds::new(point(x, y), size(corner, corner)).contains(&position);
    let (right, bottom) = (window_size.width - corner, window_size.height - corner);
    if !tiling.top && corner_at(Pixels::ZERO, Pixels::ZERO) {
        Some(ResizeEdge::TopLeft)
    } else if !tiling.top && corner_at(right, Pixels::ZERO) {
        Some(ResizeEdge::TopRight)
    } else if !tiling.bottom && corner_at(Pixels::ZERO, bottom) {
        Some(ResizeEdge::BottomLeft)
    } else if !tiling.bottom && corner_at(right, bottom) {
        Some(ResizeEdge::BottomRight)
    } else if !tiling.top && position.y < SHADOW {
        Some(ResizeEdge::Top)
    } else if !tiling.bottom && position.y > window_size.height - SHADOW {
        Some(ResizeEdge::Bottom)
    } else if !tiling.left && position.x < SHADOW {
        Some(ResizeEdge::Left)
    } else if !tiling.right && position.x > window_size.width - SHADOW {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

/// A side of the top bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Where the buttons go when the platform doesn't say: minimize, maximize and close
/// on the right, as GPUI's own default on Linux.
const DEFAULT_LAYOUT: WindowButtonLayout = WindowButtonLayout {
    left: [None; gpui::MAX_BUTTONS_PER_SIDE],
    right: [
        Some(WindowButton::Minimize),
        Some(WindowButton::Maximize),
        Some(WindowButton::Close),
    ],
};

/// The buttons on `side`, in order: where the desktop puts them (GNOME's
/// `button-layout` setting, read by GPUI), without those the compositor doesn't offer
/// for this window. Close is always offered.
fn buttons_on(
    side: Side,
    layout: Option<WindowButtonLayout>,
    offered: WindowControls,
    resizable: bool,
    minimizable: bool,
) -> Vec<WindowButton> {
    let layout = layout.unwrap_or(DEFAULT_LAYOUT);
    let buttons = match side {
        Side::Left => layout.left,
        Side::Right => layout.right,
    };
    buttons
        .into_iter()
        .flatten()
        .filter(|button| match button {
            WindowButton::Minimize => offered.minimize && minimizable,
            WindowButton::Maximize => offered.maximize && resizable,
            WindowButton::Close => true,
        })
        .collect()
}

/// The window's buttons for `side` of the top bar, while the launcher draws its frame:
/// empty without its own frame, or where the desktop puts none on that side.
fn window_buttons(side: Side, window: &Window, cx: &App) -> Vec<WindowButton> {
    if client_tiling(window).is_none() {
        return Vec::new();
    }
    buttons_on(
        side,
        cx.button_layout(),
        window.window_controls(),
        window.is_resizable(),
        window.is_minimizable(),
    )
}

/// The width of `count` buttons side by side.
fn row_width(count: usize) -> Pixels {
    BUTTON * count as f32 + BUTTON_GAP * count.saturating_sub(1) as f32
}

/// The width of [`controls`] for `side`, for the top bar to leave room for them, or
/// `None` where there are none.
pub fn controls_width(side: Side, window: &Window, cx: &App) -> Option<Pixels> {
    let count = window_buttons(side, window, cx).len();
    (count > 0).then(|| row_width(count))
}

/// The window's buttons for `side` of the top bar in a row, while the launcher draws
/// its frame and the desktop puts buttons on that side. The top bar places them (see
/// `shell::top_bar::window_controls`).
pub fn controls(side: Side, window: &Window, cx: &App) -> Option<AnyElement> {
    let buttons = window_buttons(side, window, cx);
    if buttons.is_empty() {
        return None;
    }
    let maximized = window.is_maximized();
    Some(
        div()
            .id(match side {
                Side::Left => "window-controls-left",
                Side::Right => "window-controls-right",
            })
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(BUTTON_GAP)
            // The pointer is over the buttons, not the top bar under them: a press
            // between two buttons doesn't move the window.
            .occlude()
            .children(
                buttons
                    .into_iter()
                    .map(|button| window_button(button, maximized)),
            )
            .into_any_element(),
    )
}

fn window_button(button: WindowButton, maximized: bool) -> impl IntoElement {
    let (id, icon, label, area) = match button {
        WindowButton::Minimize => (
            "window-minimize",
            Icon::WindowMinimize,
            t(ShellMsg::MinimizeWindow),
            WindowControlArea::Min,
        ),
        WindowButton::Maximize if maximized => (
            "window-restore",
            Icon::WindowRestore,
            t(ShellMsg::RestoreWindow),
            WindowControlArea::Max,
        ),
        WindowButton::Maximize => (
            "window-maximize",
            Icon::WindowMaximize,
            t(ShellMsg::MaximizeWindow),
            WindowControlArea::Max,
        ),
        WindowButton::Close => (
            "window-close",
            Icon::WindowClose,
            t(CommonMsg::Close),
            WindowControlArea::Close,
        ),
    };
    let close = button == WindowButton::Close;
    div()
        .id(id)
        .accessible(Role::Button, id, label)
        .window_control_area(area)
        .tooltip(text_tooltip(label))
        .group(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(BUTTON)
        .rounded_full()
        .bg(colors::GRAY_850)
        .cursor_pointer()
        .hover(|style| {
            style.bg(if close {
                colors::RED_400
            } else {
                colors::GRAY_700
            })
        })
        .active(|style| style.bg(if close { colors::RED } else { colors::GRAY_600 }))
        .child(
            icon.render(BUTTON_ICON, colors::GRAY_400)
                .group_hover(id, |style| {
                    style.text_color(if close {
                        colors::WHITE
                    } else {
                        colors::GRAY_300
                    })
                }),
        )
        .on_press(move |_, window, cx| {
            cx.stop_propagation();
            super::trace_ui(
                "ui.click",
                "window_button",
                &gpui::ElementId::from(id),
                serde_json::json!({}),
            );
            match button {
                WindowButton::Minimize => window.minimize_window(),
                WindowButton::Maximize => window.zoom_window(),
                // As the compositor's close would: the launcher quits with its window.
                WindowButton::Close => window.remove_window(),
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Size<Pixels> = size(gpui::px(800.), gpui::px(600.));

    fn at(x: f32, y: f32) -> Point<Pixels> {
        point(gpui::px(x), gpui::px(y))
    }

    #[test]
    fn the_band_resizes_and_the_inside_does_not() {
        let free = Tiling::default();
        assert_eq!(resize_edge(at(400., 300.), WINDOW, free), None);
        assert_eq!(
            resize_edge(at(400., 3.), WINDOW, free),
            Some(ResizeEdge::Top)
        );
        assert_eq!(
            resize_edge(at(400., 597.), WINDOW, free),
            Some(ResizeEdge::Bottom)
        );
        assert_eq!(
            resize_edge(at(3., 300.), WINDOW, free),
            Some(ResizeEdge::Left)
        );
        assert_eq!(
            resize_edge(at(797., 300.), WINDOW, free),
            Some(ResizeEdge::Right)
        );
        // Just inside the window, along an edge: the content's.
        assert_eq!(resize_edge(at(400., 12.), WINDOW, free), None);
    }

    #[test]
    fn corners_reach_a_little_into_the_window() {
        let free = Tiling::default();
        assert_eq!(
            resize_edge(at(2., 2.), WINDOW, free),
            Some(ResizeEdge::TopLeft)
        );
        assert_eq!(
            resize_edge(at(12., 12.), WINDOW, free),
            Some(ResizeEdge::TopLeft)
        );
        assert_eq!(
            resize_edge(at(790., 4.), WINDOW, free),
            Some(ResizeEdge::TopRight)
        );
        assert_eq!(
            resize_edge(at(4., 590.), WINDOW, free),
            Some(ResizeEdge::BottomLeft)
        );
        assert_eq!(
            resize_edge(at(796., 596.), WINDOW, free),
            Some(ResizeEdge::BottomRight)
        );
    }

    #[test]
    fn the_content_is_inside_the_band_and_the_outline() {
        assert_eq!(
            inside_frame(WINDOW, Tiling::default()),
            Bounds::new(at(11., 11.), size(gpui::px(778.), gpui::px(578.)))
        );
        assert_eq!(
            inside_frame(WINDOW, Tiling::tiled()),
            Bounds::new(at(0., 0.), WINDOW)
        );
        let left = Tiling {
            left: true,
            ..Tiling::default()
        };
        assert_eq!(
            inside_frame(WINDOW, left),
            Bounds::new(at(0., 11.), size(gpui::px(789.), gpui::px(578.)))
        );
    }

    #[test]
    fn the_top_bar_leaves_room_for_each_button_and_the_gaps() {
        assert_eq!(row_width(1), gpui::px(24.));
        assert_eq!(row_width(3), gpui::px(88.));
    }

    #[test]
    fn tiled_edges_do_not_resize() {
        let maximized = Tiling::tiled();
        for (x, y) in [(2., 2.), (400., 3.), (3., 300.), (797., 300.), (796., 596.)] {
            assert_eq!(resize_edge(at(x, y), WINDOW, maximized), None, "{x},{y}");
        }
        // Snapped to the left half: only the right edge is free.
        let left_half = Tiling {
            top: true,
            bottom: true,
            left: true,
            right: false,
        };
        assert_eq!(resize_edge(at(3., 300.), WINDOW, left_half), None);
        assert_eq!(
            resize_edge(at(797., 300.), WINDOW, left_half),
            Some(ResizeEdge::Right)
        );
        assert_eq!(
            resize_edge(at(797., 3.), WINDOW, left_half),
            Some(ResizeEdge::Right)
        );
    }

    fn layout(left: &[WindowButton], right: &[WindowButton]) -> WindowButtonLayout {
        let side = |buttons: &[WindowButton]| {
            let mut side = [None; gpui::MAX_BUTTONS_PER_SIDE];
            for (slot, button) in side.iter_mut().zip(buttons) {
                *slot = Some(*button);
            }
            side
        };
        WindowButtonLayout {
            left: side(left),
            right: side(right),
        }
    }

    use WindowButton::{Close, Maximize, Minimize};

    #[test]
    fn buttons_go_where_the_desktop_puts_them() {
        let all = WindowControls::default();
        // GNOME's default, "appmenu:close": only Close, on the right.
        let gnome = Some(layout(&[], &[Close]));
        assert_eq!(buttons_on(Side::Left, gnome, all, true, true), []);
        assert_eq!(buttons_on(Side::Right, gnome, all, true, true), [Close]);
        // "close,minimize,maximize:", as on elementary OS or a macOS-like setup.
        let left = Some(layout(&[Close, Minimize, Maximize], &[]));
        assert_eq!(
            buttons_on(Side::Left, left, all, true, true),
            [Close, Minimize, Maximize]
        );
        assert_eq!(buttons_on(Side::Right, left, all, true, true), []);
        // Nothing from the platform: all three on the right.
        assert_eq!(
            buttons_on(Side::Right, None, all, true, true),
            [Minimize, Maximize, Close]
        );
    }

    #[test]
    fn buttons_the_window_cannot_use_are_left_out() {
        let full = Some(layout(&[], &[Minimize, Maximize, Close]));
        let no_minimize = WindowControls {
            minimize: false,
            ..WindowControls::default()
        };
        assert_eq!(
            buttons_on(Side::Right, full, no_minimize, true, true),
            [Maximize, Close]
        );
        assert_eq!(
            buttons_on(Side::Right, full, WindowControls::default(), false, false),
            [Close]
        );
    }
}
