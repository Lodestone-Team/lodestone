//! A checkbox with an optional label, after the dashboard's `Atoms/Checkbox.tsx`: an
//! outlined square that fills with a check when ticked.

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::{
    Icon, KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
};
use crate::theme::{self, colors, px, text, weights};

type ChangeHandler = Box<dyn Fn(bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    /// Its name for screen readers when it has no label shown.
    accessible_label: Option<SharedString>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            accessible_label: None,
            disabled: false,
            on_change: None,
        }
    }

    /// Text after the box; clicking it toggles the box too.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The name screen readers say for a box without a label, such as a row's
    /// "Select server.properties".
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new state when the box is clicked (or Space or Enter pressed).
    pub fn on_change(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let name = self
            .accessible_label
            .clone()
            .or_else(|| self.label.clone())
            .unwrap_or_default();
        let checked = self.checked;
        let disabled = self.disabled;
        let color = if disabled {
            colors::GRAY_500
        } else if checked {
            colors::GRAY_300
        } else {
            colors::GRAY_400
        };
        let icon = if checked {
            Icon::SquareCheck
        } else {
            Icon::Square
        };
        div()
            .id(self.id.clone())
            .accessible_id(Role::CheckBox, &self.id, name)
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .text_size(theme::text_size(text::MEDIUM))
            .font_weight(weights::MEDIUM.into())
            .text_color(if disabled {
                colors::GRAY_500
            } else {
                colors::GRAY_300
            })
            .when(!disabled, |this| this.keyboard_focus().cursor_pointer())
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .rounded_full()
                    .when(!disabled, |this| {
                        this.hover(|style| style.bg(colors::GRAY_FADED.opacity(30)))
                    })
                    .child(icon.render(px(14.), color)),
            )
            .when_some(self.label, |this, label| {
                this.child(div().truncate().child(label))
            })
            .when_some(self.on_change.filter(|_| !disabled), |this, on_change| {
                this.on_press(move |_, window, cx| on_change(!checked, window, cx))
            })
    }
}
