use crate::theme::px;
use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::Icon;
use super::a11y::{Accessible, Role, Toggled};
use super::focus::KeyboardFocus;
use crate::theme::{colors, ink, text};

type ChangeHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// A box to tick, with its label beside it, after the dashboard's `Checkbox` atom. A
/// click on the box or the label flips it; Tab reaches it, and Space or Enter flips it.
///
/// The label is text, then any elements added with [`Checkbox::child`] (such as a
/// link, which should stop its click from reaching the box).
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    disabled: bool,
    label: SharedString,
    after: Vec<gpui::AnyElement>,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            checked,
            disabled: false,
            label: label.into(),
            after: Vec::new(),
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Adds an element after the label's text, in the same line.
    pub fn child(mut self, element: impl IntoElement) -> Self {
        self.after.push(element.into_any_element());
        self
    }

    /// Called with the new value when the box is clicked.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        let id = self.id.clone();
        self.on_change = Some(Box::new(move |value, window, cx| {
            super::trace_ui(
                "ui.checkbox",
                "checkbox",
                &id,
                serde_json::json!({ "value": value }),
            );
            handler(value, window, cx)
        }));
        self
    }
}

/// The box alone, ticked or not.
pub fn check_box(checked: bool, disabled: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(crate::theme::radius::DEFAULT)
        .border_1()
        .map(|this| {
            if checked {
                let fill = if disabled {
                    colors::GRAY_600
                } else {
                    crate::theme::accent::base()
                };
                this.bg(fill).border_color(fill)
            } else {
                this.bg(colors::GRAY_900)
                    .border_color(colors::GRAY_FADED.opacity(if disabled { 20 } else { 50 }))
            }
        })
        .when(checked, |this| {
            this.child(Icon::Check.render(px(10.), colors::WHITE))
        })
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let checked = self.checked;
        div()
            .id(self.id.clone())
            .accessible_id(Role::CheckBox, &self.id, self.label.clone())
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .when(!self.disabled, |this| this.keyboard_focus())
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .rounded(crate::theme::radius::DEFAULT)
            .text_size(text::MEDIUM)
            .text_color(if self.disabled {
                ink::faint()
            } else {
                ink::secondary()
            })
            .child(div().pt(px(2.)).child(check_box(checked, self.disabled)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(self.label)
                    .children(self.after),
            )
            .when(!self.disabled, |this| {
                this.cursor_pointer()
                    .when_some(self.on_change, |this, on_change| {
                        this.on_press(move |_, window, cx| on_change(&!checked, window, cx))
                    })
            })
    }
}
