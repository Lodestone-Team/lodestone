use crate::theme::px;
use gpui::{App, ElementId, IntoElement, RenderOnce, Window, div, prelude::*};

use super::focus::KeyboardFocus;
use crate::theme::colors;

type ChangeHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// An on/off switch, after the dashboard's `Toggle` atom. Tab reaches it, and Space
/// (or Enter) flips it.
#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    value: bool,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, value: bool) -> Self {
        Self {
            id: id.into(),
            value,
            disabled: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new value when the toggle is clicked.
    pub fn on_change(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        let id = self.id.clone();
        self.on_change = Some(Box::new(move |value, window, cx| {
            super::trace_ui(
                "ui.toggle",
                "toggle",
                &id,
                serde_json::json!({ "value": value }),
            );
            handler(value, window, cx)
        }));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // 0.0 off, 1.0 on, easing between them when the value changes.
        let on = super::motion::switch((self.id.clone(), "switch"), self.value, window, cx);
        let track = if self.disabled {
            colors::GRAY_FADED.opacity(30)
        } else {
            colors::WHITE.opacity(50).mix(colors::GREEN_400, on)
        };
        let knob = if self.disabled {
            colors::GRAY_FADED.opacity(40)
        } else {
            colors::WHITE
        };
        let value = self.value;

        // h-6 w-11 track with an h-4 w-4 knob at translate-x-1 or translate-x-6.
        div()
            .id(self.id)
            .when(!self.disabled, |this| this.keyboard_focus_outer())
            .flex()
            .flex_none()
            .items_center()
            .w(px(44.))
            .h(px(24.))
            .rounded_full()
            .bg(track)
            .child(
                div()
                    .size(px(16.))
                    .ml(px(4. + 20. * on))
                    .rounded_full()
                    .bg(knob),
            )
            .when(!self.disabled, |this| {
                this.cursor_pointer()
                    .when_some(self.on_change, |this, on_change| {
                        this.on_click(move |_, window, cx| on_change(&!value, window, cx))
                    })
            })
    }
}
