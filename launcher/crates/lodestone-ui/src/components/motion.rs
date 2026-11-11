//! Small state-change animations for controls: a switch's knob sliding, a selected
//! tab's highlight fading in. GPUI has no CSS-style transitions, so [`tween`] keeps,
//! per element, where a value is going and since when, and asks for frames only
//! while it moves. Nothing redraws once it has settled.

use std::time::{Duration, Instant};

use gpui::{App, ElementId, IntoElement, RenderOnce, Window};

/// How long a control takes to change state.
pub const CHANGE: Duration = Duration::from_millis(160);

/// Per element: the value the animation started from and is heading to, and when.
struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Tween {
    fn value_at(&self, now: Instant) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= self.duration || self.duration.is_zero() {
            return (self.to, true);
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        (self.from + (self.to - self.from) * ease_out_cubic(t), false)
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The value of the element `key`, easing toward `target` over `duration` whenever
/// `target` changes. The first time it is drawn it starts at `target`, so nothing
/// animates as a control appears. With reduced motion it jumps.
pub fn tween(
    key: impl Into<ElementId>,
    target: f32,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    if super::dialog::prefers_reduced_motion() {
        return target;
    }
    tween_even_reduced(key, target, duration, window, cx)
}

/// [`tween`], also with reduced motion: for a fade, which reduced motion keeps (only
/// movement goes).
pub fn tween_even_reduced(
    key: impl Into<ElementId>,
    target: f32,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let state = window.use_keyed_state(key, cx, |_, _| Tween {
        from: target,
        to: target,
        start: Instant::now(),
        duration,
    });
    let now = Instant::now();
    let (value, done) = state.update(cx, |tween, _| {
        if tween.to != target {
            // Start from wherever it is now, so a quick second change doesn't jump.
            let (current, _) = tween.value_at(now);
            *tween = Tween {
                from: current,
                to: target,
                start: now,
                duration,
            };
        }
        tween.value_at(now)
    });
    if !done {
        window.request_animation_frame();
    }
    value
}

/// `tween` for an on/off state: 0.0 off, 1.0 on.
pub fn switch(key: impl Into<ElementId>, on: bool, window: &mut Window, cx: &mut App) -> f32 {
    tween(key, if on { 1.0 } else { 0.0 }, CHANGE, window, cx)
}

/// An element styled by how far a state change has gone: `style` gets the element
/// and 0.0 (off) to 1.0 (on), easing whenever `on` changes. For controls built by
/// plain functions, which have no window to ask for frames.
#[derive(IntoElement)]
pub struct Animated<E: IntoElement + 'static> {
    key: ElementId,
    on: bool,
    element: E,
    style: Box<dyn FnOnce(E, f32) -> E>,
}

impl<E: IntoElement + 'static> Animated<E> {
    pub fn new(
        key: impl Into<ElementId>,
        on: bool,
        element: E,
        style: impl FnOnce(E, f32) -> E + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            on,
            element,
            style: Box::new(style),
        }
    }
}

impl<E: IntoElement + 'static> RenderOnce for Animated<E> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = switch(self.key, self.on, window, cx);
        (self.style)(self.element, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_from_start_to_end() {
        let start = Instant::now();
        let tween = Tween {
            from: 0.0,
            to: 1.0,
            start,
            duration: Duration::from_millis(100),
        };
        assert_eq!(tween.value_at(start), (0.0, false));
        let (mid, done) = tween.value_at(start + Duration::from_millis(50));
        assert!(!done && mid > 0.5 && mid < 1.0, "eased past halfway: {mid}");
        assert_eq!(
            tween.value_at(start + Duration::from_millis(100)),
            (1.0, true)
        );
    }
}
