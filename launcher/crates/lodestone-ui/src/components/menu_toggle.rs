//! Closing a menu by clicking the button that opened it.
//!
//! A menu closes on a mouse-down outside it (`on_mouse_down_out`), and that runs
//! before the button under the pointer gets the click. Without help, a click on the
//! menu's own button would close the menu on mouse-down and then toggle it open again
//! on mouse-up. So a menu that closes this way calls [`closed_by`] with its name, and
//! its button asks [`toggles`] before it toggles: the press that began with the
//! mouse-down that just closed the menu leaves it closed. A click on another menu's
//! button still opens that menu.

use std::cell::RefCell;

use gpui::{ClickEvent, MouseDownEvent, Pixels, Point};

thread_local! {
    /// The menu last closed by a mouse-down outside it, and where that mouse-down was.
    static CLOSED: RefCell<Option<(String, Point<Pixels>)>> = const { RefCell::new(None) };
}

/// Call from the `on_mouse_down_out` handler that closes the menu `menu`.
pub fn closed_by(menu: impl Into<String>, event: &MouseDownEvent) {
    CLOSED.with(|closed| *closed.borrow_mut() = Some((menu.into(), event.position)));
}

/// Whether the press on the button of `menu` should toggle it: false when the press
/// began with the mouse-down that just closed it.
pub fn toggles(menu: &str, event: &ClickEvent) -> bool {
    let closed = CLOSED.with(|closed| closed.borrow_mut().take());
    match (event, closed) {
        (ClickEvent::Mouse(click), Some((name, at))) => {
            !(name == menu && at == click.down.position)
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{MouseButton, MouseClickEvent, MouseUpEvent, point, px};

    fn down(x: f32) -> MouseDownEvent {
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(5.)),
            click_count: 1,
            ..Default::default()
        }
    }

    fn click(x: f32) -> ClickEvent {
        ClickEvent::Mouse(MouseClickEvent {
            down: down(x),
            up: MouseUpEvent {
                button: MouseButton::Left,
                position: point(px(x), px(5.)),
                click_count: 1,
                ..Default::default()
            },
        })
    }

    #[test]
    fn the_click_that_closed_the_menu_does_not_reopen_it() {
        closed_by("language", &down(10.));
        assert!(!toggles("language", &click(10.)));
        // the next click on the button opens it again
        assert!(toggles("language", &click(10.)));
    }

    #[test]
    fn another_menus_button_still_opens() {
        closed_by("language", &down(10.));
        assert!(toggles("theme", &click(10.)));
    }

    #[test]
    fn a_click_elsewhere_then_on_the_button_opens_it() {
        closed_by("language", &down(300.));
        assert!(toggles("language", &click(10.)));
    }

    #[test]
    fn keyboard_presses_always_toggle() {
        closed_by("language", &down(10.));
        assert!(toggles("language", &ClickEvent::default()));
    }
}
