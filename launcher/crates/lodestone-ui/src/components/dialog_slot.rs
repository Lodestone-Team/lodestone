//! Keeps a dialog on screen while it animates closed.
//!
//! GPUI drops an element as soon as it is no longer rendered, so a dialog that its
//! owner stores as an `Option` vanishes the moment the owner sets it to `None`. A
//! [`DialogSlot`] takes the place of that `Option`: closing moves the dialog's state to
//! a list of closing dialogs, which are still rendered, fading out, until their exit
//! is over. See the module docs of [`super::dialog`] for how to use it.

use crate::theme::px;
use std::{
    cell::Cell,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AnyWindowHandle, Context, ElementId, FocusHandle, IntoElement, Window, div,
    prelude::*,
};

use super::dialog::prefers_reduced_motion;

/// How long a dialog takes to close, like the dashboard's `leave` transitions
/// (`ease-in duration-150`).
pub const EXIT: Duration = Duration::from_millis(150);
/// The shorter fade, without the slide, when the system asks for reduced motion.
pub const EXIT_REDUCED: Duration = Duration::from_millis(120);
/// How far the panel rises while it fades out.
const SLIDE_OUT: f32 = 10.;

/// A quadratic ease-in, close to the CSS `ease-in` (`cubic-bezier(0.4, 0, 1, 1)`).
fn ease_in_quad(delta: f32) -> f32 {
    delta * delta
}

/// Each dialog shown gets a key that no other dialog in the app has, so that a dialog
/// that is closing and one that just opened in its place never share element ids.
fn next_key() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The state behind a [`DialogSlot`], without GPUI: the open dialog, and the dialogs
/// that are animating closed with the time they started to.
#[derive(Debug)]
pub struct Presence<T> {
    current: Option<(u64, T)>,
    closing: Vec<Closing<T>>,
}

#[derive(Debug)]
struct Closing<T> {
    key: u64,
    value: T,
    since: Instant,
    duration: Duration,
}

impl<T> Closing<T> {
    /// How far the exit is at `now`, from 0 to 1.
    fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.;
        }
        let elapsed = now.saturating_duration_since(self.since);
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.)
    }
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Presence<T> {
    pub const fn new() -> Self {
        Self {
            current: None,
            closing: Vec::new(),
        }
    }

    /// Shows `value`. A dialog that was open starts closing, under the new one.
    pub fn open(&mut self, value: T, now: Instant, exit: Duration) {
        self.close(now, exit);
        self.current = Some((next_key(), value));
    }

    /// Starts closing the open dialog, if any, and returns its key and its state, which
    /// stays until [`Self::finish`] or [`Self::prune`] drops it.
    pub fn close(&mut self, now: Instant, exit: Duration) -> Option<(u64, &mut T)> {
        let (key, value) = self.current.take()?;
        self.prune(now);
        self.closing.push(Closing {
            key,
            value,
            since: now,
            duration: exit,
        });
        self.closing.last_mut().map(|c| (c.key, &mut c.value))
    }

    /// Drops the closing dialog with `key`: its exit is over.
    pub fn finish(&mut self, key: u64) {
        self.closing.retain(|c| c.key != key);
    }

    /// Drops the closing dialogs whose exit is over at `now`.
    pub fn prune(&mut self, now: Instant) {
        self.closing.retain(|c| c.progress(now) < 1.);
    }

    pub fn current(&self) -> Option<&T> {
        self.current.as_ref().map(|(_, value)| value)
    }

    pub fn current_mut(&mut self) -> Option<&mut T> {
        self.current.as_mut().map(|(_, value)| value)
    }

    pub fn is_open(&self) -> bool {
        self.current.is_some()
    }

    /// Whether a dialog is still animating closed.
    pub fn is_closing(&self) -> bool {
        !self.closing.is_empty()
    }

    /// What to draw at `now`, bottom first: each dialog that is closing with how far its
    /// exit is, then the open one with `None`. A closing dialog whose exit is over is left
    /// out even before it is dropped.
    pub fn shown(&self, now: Instant) -> impl Iterator<Item = (u64, &T, Option<f32>)> {
        self.closing
            .iter()
            .map(move |c| (c.key, &c.value, Some(c.progress(now))))
            .filter(|(_, _, progress)| progress.is_some_and(|p| p < 1.))
            .chain(self.current.iter().map(|(key, value)| (*key, value, None)))
    }
}

/// What a slot keeps for each dialog besides the owner's state.
struct Shown<T> {
    value: T,
    /// Tracked by the layer around the dialog, to tell whether focus is inside it.
    scope: FocusHandle,
    /// Read when the dialog starts closing: fade without the slide.
    reduced: bool,
    /// The window the dialog was last drawn in, to move focus out of it as it closes.
    window: Cell<Option<AnyWindowHandle>>,
}

/// The place of a dialog in its owner `V`, with what the dialog needs to render (`T`):
/// the `Modal` enum of the shell, a prompt struct, an `Entity` of a dialog view.
///
/// Use it instead of an `Option<T>`: [`Self::open`] instead of `= Some(..)`,
/// [`Self::close`] instead of `= None` or `.take()`, and [`Self::render`] where the
/// dialog was rendered. A closed dialog stays in the slot, rendered with its last state
/// and fading out, for [`EXIT`]. Meanwhile it takes no clicks (a layer over it catches
/// them, and keeps them off the page under it), and if focus was in it, focus leaves it,
/// so keys go to the page again.
pub struct DialogSlot<V: 'static, T: 'static> {
    presence: Presence<Shown<T>>,
    /// Finds this slot in its owner, for the timer that drops a dialog after its exit.
    slot: fn(&mut V) -> &mut DialogSlot<V, T>,
}

impl<V: 'static, T: 'static> DialogSlot<V, T> {
    /// An empty slot. `slot` returns this slot from its owner, as in
    /// `DialogSlot::new(|this: &mut Servers| &mut this.trust_prompt)`.
    pub fn new(slot: fn(&mut V) -> &mut DialogSlot<V, T>) -> Self {
        Self {
            presence: Presence::new(),
            slot,
        }
    }

    /// Shows a dialog. One that was open closes under it, animated.
    pub fn open(&mut self, value: T, cx: &mut Context<V>) {
        self.close(cx);
        let shown = Shown {
            value,
            scope: cx.focus_handle(),
            reduced: false,
            window: Cell::new(None),
        };
        self.presence.open(shown, Instant::now(), EXIT);
        cx.notify();
    }

    /// Closes the open dialog, if any, and returns its state. It stays on screen, fading
    /// out, and is dropped when the exit is over. Take from the returned state only what
    /// rendering it doesn't need (a pending download, say); clone the rest.
    pub fn close(&mut self, cx: &mut Context<V>) -> Option<&mut T> {
        let reduced = prefers_reduced_motion();
        let exit = if reduced { EXIT_REDUCED } else { EXIT };
        let slot = self.slot;
        let (key, shown) = self.presence.close(Instant::now(), exit)?;
        shown.reduced = reduced;
        cx.notify();
        // Focus leaves the dialog now, for what opened it: it no longer takes keys.
        if let Some(window) = shown.window.get() {
            let scope = shown.scope.clone();
            super::focus::mark_closing(&scope);
            cx.defer(move |cx| {
                window
                    .update(cx, |_, window, cx| {
                        if scope.contains_focused(window, cx) {
                            super::focus::leave(&scope, window, cx);
                        }
                    })
                    .ok();
            });
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            // The owner may be gone; then so is the dialog.
            this.update(cx, |this, cx| {
                slot(this).presence.finish(key);
                cx.notify();
            })
            .ok();
        })
        .detach();
        Some(&mut shown.value)
    }

    /// The open dialog's state.
    pub fn current(&self) -> Option<&T> {
        self.presence.current().map(|shown| &shown.value)
    }

    pub fn current_mut(&mut self) -> Option<&mut T> {
        self.presence.current_mut().map(|shown| &mut shown.value)
    }

    pub fn is_open(&self) -> bool {
        self.presence.is_open()
    }

    /// Renders the dialogs of this slot, bottom first: those closing, then the open one.
    /// `render` builds a dialog from its state, as for the open dialog; it may return
    /// `None` to leave one out. Call it from the render of whichever view shows the
    /// dialogs, and add the elements as the last children of a full-window, relatively
    /// positioned element.
    pub fn render(
        &self,
        window: &Window,
        render: impl FnMut(&T) -> Option<AnyElement>,
    ) -> Vec<AnyElement> {
        self.render_layers(window, true, render)
    }

    /// Renders only the dialogs that are closing, for a view that shows another slot's
    /// dialog instead of this slot's open one.
    pub fn render_closing(
        &self,
        window: &Window,
        render: impl FnMut(&T) -> Option<AnyElement>,
    ) -> Vec<AnyElement> {
        self.render_layers(window, false, render)
    }

    fn render_layers(
        &self,
        window: &Window,
        with_open: bool,
        mut render: impl FnMut(&T) -> Option<AnyElement>,
    ) -> Vec<AnyElement> {
        self.presence
            .shown(Instant::now())
            .filter(|(_, _, exit)| with_open || exit.is_some())
            .filter_map(|(key, shown, exit)| {
                let content = render(&shown.value)?;
                Some(layer(key, shown, exit, content, window))
            })
            .collect()
    }
}

/// Puts a dialog in a full-window layer keyed by its slot key, so it keeps its element
/// ids (and the state of its open animation) when it starts closing. While it closes,
/// the layer fades and lifts it, and covers it with an element that takes the clicks.
fn layer<T>(
    key: u64,
    shown: &Shown<T>,
    exit: Option<f32>,
    content: AnyElement,
    window: &Window,
) -> AnyElement {
    shown.window.set(Some(window.window_handle()));
    let layer = div()
        .id(ElementId::from(("dialog-layer", key)))
        .absolute()
        .inset_0()
        .track_focus(&shown.scope);
    let Some(progress) = exit else {
        return layer.child(content).into_any_element();
    };

    // Draw the next frame of the exit, and move focus out of the dialog: back to what
    // opened it, to the dialog under it, or to the window. Both run on the next frame,
    // as focus can't change while the window draws.
    window.request_animation_frame();
    let scope = shown.scope.clone();
    super::focus::mark_closing(&scope);
    window.on_next_frame(move |window, cx| {
        if scope.contains_focused(window, cx) {
            super::focus::leave(&scope, window, cx);
        }
    });

    let delta = ease_in_quad(progress);
    let lift = if shown.reduced { 0. } else { SLIDE_OUT * delta };
    layer
        // The panel is centred in the layer: raising the layer's top edge by twice the
        // lift raises the panel by the lift, while the backdrop still covers the window.
        .top(px(-2. * lift))
        // Opacity applies to everything inside: the backdrop's dim and the panel.
        .opacity(1. - delta)
        .child(content)
        .child(div().absolute().inset_0().occlude())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn shown(presence: &Presence<&'static str>, now: Instant) -> Vec<(&'static str, bool)> {
        presence
            .shown(now)
            .map(|(_, value, exit)| (*value, exit.is_some()))
            .collect()
    }

    #[test]
    fn opens_and_closes() {
        let start = Instant::now();
        let mut presence = Presence::new();
        assert!(!presence.is_open());
        assert!(shown(&presence, start).is_empty());

        presence.open("a", start, EXIT);
        assert_eq!(presence.current(), Some(&"a"));
        assert_eq!(shown(&presence, start), [("a", false)]);

        let (_, value) = presence.close(start, EXIT).unwrap();
        assert_eq!(*value, "a");
        assert!(!presence.is_open());
        assert!(presence.is_closing());
        // still drawn, closing
        assert_eq!(shown(&presence, start + 50 * MS), [("a", true)]);
        // closing again does nothing
        assert!(presence.close(start, EXIT).is_none());
    }

    #[test]
    fn exit_progress() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open("a", start, EXIT);
        presence.close(start, EXIT);
        let progress = |now| presence.shown(now).next().and_then(|(_, _, exit)| exit);
        assert_eq!(progress(start), Some(0.));
        let half = progress(start + 75 * MS).unwrap();
        assert!((half - 0.5).abs() < 1e-4, "{half}");
        // over: no longer drawn, even before it is dropped
        assert_eq!(progress(start + EXIT), None);
        assert!(presence.is_closing());
    }

    #[test]
    fn drops_after_the_exit() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open("a", start, EXIT);
        let (key, _) = presence.close(start, EXIT).unwrap();

        presence.prune(start + 100 * MS);
        assert!(presence.is_closing());
        presence.prune(start + EXIT);
        assert!(!presence.is_closing());

        // or by key, as the timer does
        presence.open("b", start, EXIT);
        let (other, _) = presence.close(start, EXIT).unwrap();
        assert_ne!(key, other);
        presence.finish(key);
        assert!(presence.is_closing());
        presence.finish(other);
        assert!(!presence.is_closing());
    }

    #[test]
    fn reopens_while_closing() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open("a", start, EXIT);
        presence.close(start, EXIT);
        presence.open("b", start + 50 * MS, EXIT);

        // the old dialog finishes under the new one
        assert_eq!(presence.current(), Some(&"b"));
        assert_eq!(
            shown(&presence, start + 60 * MS),
            [("a", true), ("b", false)]
        );
        assert_eq!(shown(&presence, start + EXIT), [("b", false)]);

        // the same dialog again gets another key
        let keys: Vec<u64> = presence.shown(start).map(|(key, ..)| key).collect();
        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1]);
    }

    #[test]
    fn opening_over_an_open_dialog_closes_it() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open("a", start, EXIT);
        presence.open("b", start, EXIT);
        assert_eq!(shown(&presence, start), [("a", true), ("b", false)]);

        // several can be closing at once, oldest at the bottom
        presence.close(start + 10 * MS, EXIT);
        presence.open("c", start + 20 * MS, EXIT);
        assert_eq!(
            shown(&presence, start + 30 * MS),
            [("a", true), ("b", true), ("c", false)]
        );
        assert_eq!(
            shown(&presence, start + EXIT + 5 * MS),
            [("b", true), ("c", false)]
        );
    }

    #[test]
    fn edits_the_open_dialog() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open(1, start, EXIT);
        *presence.current_mut().unwrap() += 1;
        assert_eq!(presence.current(), Some(&2));
        // the closing state is what was shown last
        let (_, value) = presence.close(start, EXIT).unwrap();
        assert_eq!(*value, 2);
        assert_eq!(presence.current_mut(), None);
    }

    #[test]
    fn reduced_exit_is_shorter() {
        let start = Instant::now();
        let mut presence = Presence::new();
        presence.open("a", start, EXIT_REDUCED);
        presence.close(start, EXIT_REDUCED);
        assert_eq!(shown(&presence, start + 119 * MS), [("a", true)]);
        assert!(shown(&presence, start + EXIT_REDUCED).is_empty());
    }
}
