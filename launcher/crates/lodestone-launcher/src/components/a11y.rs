//! Exposes the launcher's controls to assistive technology and UI automation.
//!
//! GPUI reports an element to AccessKit (UI Automation on Windows, the accessibility
//! API on macOS, AT-SPI on Linux) only when it has both an element id and a role, so a
//! control drawn from plain `div`s is invisible to screen readers and to automation
//! tools, which then have to work from screenshots. The shared components call
//! [`Accessible::accessible`] to give themselves a role, a name and an
//! `AutomationId`; a hand-built control does the same:
//!
//! ```ignore
//! div()
//!     .id("open-folder")
//!     .accessible(Role::Button, "open-folder", t(InstancesMsg::OpenFolder))
//!     .on_press(...)
//! ```
//!
//! Use [`Accessible::on_press`] in place of `on_click` on such an element: it also
//! answers assistive technology's Click (UIA's Invoke, Toggle and Select) by calling
//! the handler, where GPUI alone would click the element's centre on screen and miss
//! one scrolled out of view. A tracked focus handle gives the Focus action. Values, toggles and selections need their
//! `aria_*` setters, and setting a value needs an `on_a11y_action` handler: see
//! [`TextInput`](super::TextInput), [`Toggle`](super::Toggle) and
//! [`Slider`](super::Slider).
//!
//! The automation id is what scripts and agents find a control by, so it should stay
//! the same across languages and releases: the control's element id, never its label.
//! A list's items add the item's own id (an instance id, an index) so each is unique.

use std::rc::Rc;

use gpui::{App, ClickEvent, ElementId, SharedString, StatefulInteractiveElement, Window};

pub use accesskit::ActionData;
pub use gpui::{AccessibleAction, Role, Toggled};

/// The automation id for an element id: its text, as GPUI writes it (`play`, `tab-2`).
pub fn automation_id(id: &ElementId) -> SharedString {
    match id {
        ElementId::Name(name) => name.clone(),
        other => other.to_string().into(),
    }
}

/// [`StatefulInteractiveElement`]s that can name themselves for assistive technology.
pub trait Accessible: StatefulInteractiveElement + Sized {
    /// Reports the element with `role`, under `label`, found by `automation_id`.
    fn accessible(
        self,
        role: Role,
        automation_id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        let label = label.into();
        let this = self.role(role).accessibility_id(automation_id);
        // AccessKit names a label by its value (its text), not by its label.
        if role == Role::Label {
            this.aria_value(label.clone()).aria_label(label)
        } else {
            this.aria_label(label)
        }
    }

    /// Like [`accessible`](Self::accessible), with the automation id taken from the
    /// element id the control was built with.
    fn accessible_id(self, role: Role, id: &ElementId, label: impl Into<SharedString>) -> Self {
        self.accessible(role, automation_id(id), label)
    }

    /// Something that opens and closes, such as a menu button or a folding group:
    /// reports whether it is open, and lets assistive technology open and close it
    /// (UI Automation's `ExpandCollapsePattern`) with `toggle`, which a click calls too.
    fn expandable(
        self,
        expanded: bool,
        toggle: impl Fn(&mut Window, &mut App) + Clone + 'static,
    ) -> Self {
        let collapse = toggle.clone();
        self.aria_expanded(expanded)
            .on_a11y_action(AccessibleAction::Expand, move |_, window, cx| {
                if !expanded {
                    toggle(window, cx)
                }
            })
            .on_a11y_action(AccessibleAction::Collapse, move |_, window, cx| {
                if expanded {
                    collapse(window, cx)
                }
            })
    }

    /// `on_click`, which assistive technology's Click (UI Automation's Invoke, Toggle
    /// and Select) also calls, with a keyboard click event, as Enter or Space would.
    ///
    /// Without it, GPUI answers an accessible Click by clicking the element's centre on
    /// screen, which misses an element scrolled out of view or covered by another.
    fn on_press(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        let handler = Rc::new(handler);
        let pressed = handler.clone();
        self.on_click(move |event, window, cx| handler(event, window, cx))
            .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                pressed(&ClickEvent::default(), window, cx)
            })
    }
}

impl<E: StatefulInteractiveElement> Accessible for E {}

/// An automation id for a child of `parent`: `parent` and `part` joined by a dash.
pub fn child_id(parent: &ElementId, part: impl std::fmt::Display) -> SharedString {
    format!("{}-{part}", automation_id(parent)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automation_ids_follow_element_ids() {
        assert_eq!(automation_id(&ElementId::from("play")), "play");
        assert_eq!(automation_id(&ElementId::from(("tab", 2usize))), "tab-2");
        assert_eq!(
            child_id(&ElementId::from("settings-tabs"), 1),
            "settings-tabs-1"
        );
    }
}
