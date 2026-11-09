use crate::theme::px;
use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, ElementId, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, RenderOnce, SharedString, Window, canvas, div,
    prelude::*,
};

use super::a11y::{Accessible, AccessibleAction, ActionData, Role};
use super::focus::KeyboardFocus;
use crate::theme::{self, colors, text};

type ChangeHandler = Rc<dyn Fn(f64, &mut Window, &mut App) + 'static>;
type Formatter = Rc<dyn Fn(f64) -> SharedString + 'static>;

/// What a slider remembers between frames while it is dragged.
#[derive(Default)]
struct Drag {
    /// The track's bounds at the last paint.
    bounds: Option<Bounds<Pixels>>,
    dragging: bool,
    /// The value under the pointer while dragging; saved when the button comes up.
    preview: Option<f64>,
}

/// A number in steps, like the game's option sliders, with its value beside it.
/// Dragging shows the value as it changes and calls `on_change` once, on release.
/// Tab reaches it; the arrow keys move a step, Home and End to the ends.
#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    width: Pixels,
    disabled: bool,
    label: Option<SharedString>,
    format: Option<Formatter>,
    on_change: Option<ChangeHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            id: id.into(),
            value,
            min,
            max,
            step,
            width: px(180.),
            disabled: false,
            label: None,
            format: None,
            on_change: None,
        }
    }

    /// What the slider sets, for assistive technology and automation. In a settings
    /// row it is the row's name.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The text shown beside the track for a value.
    pub fn format(mut self, format: impl Fn(f64) -> SharedString + 'static) -> Self {
        self.format = Some(Rc::new(format));
        self
    }

    /// Called with the new value, snapped to a step, when the user lets go or presses
    /// a key.
    pub fn on_change(mut self, handler: impl Fn(f64, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

/// `value` moved to the nearest step and kept inside `min..=max`.
fn snap(value: f64, min: f64, max: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value.clamp(min, max);
    }
    (min + ((value - min) / step).round() * step).clamp(min, max)
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            value,
            min,
            max,
            step,
            width,
            disabled,
            label: name,
            format,
            on_change,
        } = self;
        let drag = window.use_keyed_state(id.clone(), cx, |_, _| Drag::default());
        let shown = drag.read(cx).preview.unwrap_or(value);
        let span = (max - min).max(f64::EPSILON);
        let fraction = ((shown - min) / span).clamp(0.0, 1.0) as f32;
        let at = move |x: Pixels, bounds: Bounds<Pixels>| {
            let fraction = ((x - bounds.left()) / bounds.size.width).clamp(0.0, 1.0) as f64;
            snap(min + fraction * span, min, max, step)
        };
        let label = format.map_or_else(|| SharedString::from(format!("{shown}")), |f| f(shown));
        let (fill, knob) = if disabled {
            (
                colors::GRAY_FADED.opacity(40),
                colors::GRAY_FADED.opacity(60),
            )
        } else {
            (colors::GREEN_400, colors::WHITE)
        };

        let track = div()
            .id(id.clone())
            .accessible_id(Role::Slider, &id, name.unwrap_or_default())
            .aria_numeric_value(shown)
            .aria_min_numeric_value(min)
            .aria_max_numeric_value(max)
            .when(step > 0.0, |this| this.aria_numeric_value_step(step))
            .aria_value(label.clone())
            .relative()
            .flex_none()
            .w(width)
            .h(px(20.))
            .flex()
            .items_center()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .h(px(4.))
                    .rounded_full()
                    .bg(colors::GRAY_600),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w(width * fraction)
                    .h(px(4.))
                    .rounded_full()
                    .bg(fill),
            )
            .child(
                div()
                    .absolute()
                    .left(width * fraction - px(7.))
                    .size(px(14.))
                    .rounded_full()
                    .bg(knob),
            )
            .child({
                let drag = drag.clone();
                let on_change = on_change.clone();
                canvas(
                    |bounds, _, _| bounds,
                    move |_, bounds, window, cx| {
                        drag.update(cx, |drag, _| drag.bounds = Some(bounds));
                        if disabled {
                            return;
                        }
                        // Window-wide, so the drag keeps following the pointer off the
                        // track and still ends if the button comes up elsewhere.
                        let moved = drag.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Bubble || !moved.read(cx).dragging {
                                return;
                            }
                            let value = at(event.position.x, bounds);
                            moved.update(cx, |drag, cx| {
                                if drag.preview != Some(value) {
                                    drag.preview = Some(value);
                                    cx.notify();
                                }
                            });
                        });
                        let released = drag.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || event.button != MouseButton::Left
                                || !released.read(cx).dragging
                            {
                                return;
                            }
                            let value = released.update(cx, |drag, cx| {
                                drag.dragging = false;
                                cx.notify();
                                drag.preview.take()
                            });
                            if let (Some(value), Some(on_change)) = (value, on_change.as_ref()) {
                                on_change(value, window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_full()
            })
            .when(!disabled, |this| {
                // Assistive technology sets the value, or moves it a step.
                let set = |target: fn(f64, f64, f64, f64, Option<&ActionData>) -> Option<f64>| {
                    let on_change = on_change.clone();
                    move |data: Option<&ActionData>, window: &mut Window, cx: &mut App| {
                        let Some(next) = target(value, min, max, step, data) else {
                            return;
                        };
                        let next = snap(next, min, max, step);
                        if next != value
                            && let Some(on_change) = on_change.as_ref()
                        {
                            on_change(next, window, cx);
                        }
                    }
                };
                this.on_a11y_action(
                    AccessibleAction::SetValue,
                    set(|_, _, _, _, data| match data {
                        Some(ActionData::NumericValue(value)) => Some(*value),
                        Some(ActionData::Value(text)) => text.trim().parse().ok(),
                        _ => None,
                    }),
                )
                .on_a11y_action(
                    AccessibleAction::Increment,
                    set(|value, _, _, step, _| Some(value + step)),
                )
                .on_a11y_action(
                    AccessibleAction::Decrement,
                    set(|value, _, _, step, _| Some(value - step)),
                )
            })
            .when(!disabled, |this| {
                let pressed = drag.clone();
                let keys = on_change.clone();
                this.keyboard_focus_outer()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                        let Some(bounds) = pressed.read(cx).bounds else {
                            return;
                        };
                        let value = at(event.position.x, bounds);
                        pressed.update(cx, |drag, cx| {
                            drag.dragging = true;
                            drag.preview = Some(value);
                            cx.notify();
                        });
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let next = match event.keystroke.key.as_str() {
                            "left" | "down" => value - step,
                            "right" | "up" => value + step,
                            "home" => min,
                            "end" => max,
                            _ => return,
                        };
                        let next = snap(next, min, max, step);
                        if next != value
                            && let Some(on_change) = keys.as_ref()
                        {
                            cx.stop_propagation();
                            on_change(next, window, cx);
                        }
                    })
            });

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(track)
            .child(
                div()
                    .min_w(px(72.))
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(if disabled {
                        colors::GRAY_500
                    } else {
                        colors::GRAY_300
                    })
                    .child(label),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::snap;

    #[test]
    fn snapping_keeps_to_steps_and_ends() {
        assert_eq!(snap(12.4, 2.0, 32.0, 1.0), 12.0);
        assert_eq!(snap(40.0, 2.0, 32.0, 1.0), 32.0);
        assert_eq!(snap(-3.0, 2.0, 32.0, 1.0), 2.0);
        assert_eq!(snap(133.0, 10.0, 260.0, 10.0), 130.0);
        assert!((snap(0.456, 0.0, 1.0, 0.01) - 0.46).abs() < 1e-9);
    }
}
