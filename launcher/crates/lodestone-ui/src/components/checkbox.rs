use crate::theme::px;
use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::Icon;
use super::a11y::{Accessible, Role, Toggled};
use super::focus::KeyboardFocus;
use crate::theme::{self, colors, text};

type ChangeHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// A box to tick, with its label after it, after the dashboard's `Checkbox` atom: for
/// choices in a list, such as the instances a permission applies to. Tab reaches it,
/// and Space (or Enter) ticks it. A click on the label ticks it too.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    value: bool,
    disabled: bool,
    label: SharedString,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, value: bool) -> Self {
        Self {
            id: id.into(),
            value,
            disabled: false,
            label: label.into(),
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let value = self.value;
        let (fill, border) = match (value, self.disabled) {
            (true, false) => (theme::accent::base(), theme::accent::base()),
            (true, true) => (colors::GRAY_FADED.opacity(40), colors::GRAY_FADED.opacity(40)),
            (false, _) => (colors::GRAY_900, colors::GRAY_FADED.opacity(50)),
        };
        div()
            .id(self.id.clone())
            .accessible_id(Role::CheckBox, &self.id, self.label.clone())
            .aria_toggled(if value { Toggled::True } else { Toggled::False })
            .when(!self.disabled, |this| this.keyboard_focus())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .min_w_0()
            .rounded(theme::radius::DEFAULT)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .rounded(theme::radius::DEFAULT)
                    .border_1()
                    .border_color(border)
                    .bg(fill)
                    .when(value, |this| {
                        this.child(Icon::Check.render(px(10.), colors::WHITE))
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(theme::text_size(text::MEDIUM))
                    .text_color(if self.disabled {
                        theme::ink::muted()
                    } else {
                        theme::ink::text()
                    })
                    .child(self.label),
            )
            .when(!self.disabled, |this| {
                this.cursor_pointer()
                    .when_some(self.on_change, |this, on_change| {
                        this.on_press(move |_, window, cx| on_change(&!value, window, cx))
                    })
            })
    }
}
