//! A vertical scroll bar for a view that scrolls: a gpui `list` ([`ListState`]), a
//! `uniform_list` ([`UniformListScrollHandle`]), a scrolling `div` ([`ScrollHandle`]),
//! or anything else that implements [`ScrollSource`] (the [`TextEditor`]).
//!
//! Put it in a `relative` container over the view. It lies along the container's right
//! edge and shows only while the content is taller than the view:
//!
//! ```ignore
//! div()
//!     .relative()
//!     .size_full()
//!     .child(list(list_state.clone(), render_row).size_full())
//!     .child(Scrollbar::new("log-scrollbar", list_state.clone()))
//! ```
//!
//! The thumb is as long as the part of the content in view (at least
//! [`MIN_THUMB`]) and sits where the view is. Drag it to scroll; a press on the track
//! above or below it scrolls a page that way. It widens and brightens under the
//! pointer. It only takes presses on itself: the wheel still scrolls the view, and
//! everything else under the bar keeps its hover.
//!
//! A [`ListState`] in tail-following mode stops following when the thumb is dragged
//! up and follows again when it reaches the bottom, as the wheel does. Views that
//! keep their own "follow" flag update it in [`Scrollbar::on_scroll`].
//!
//! [`TextEditor`]: super::TextEditor

use std::rc::Rc;

use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, ElementId, Entity, Hitbox, HitboxBehavior,
    IntoElement, ListState, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    RenderOnce, ScrollHandle, SharedString, UniformListScrollHandle, Window, canvas, div, fill,
    point, prelude::*, size,
};

use super::a11y::{Accessible, AccessibleAction, ActionData, Role};
use crate::theme::{colors, px};

/// Where a view is scrolled, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollMetrics {
    /// How far the view is scrolled down from the top: `0` to `max`.
    pub offset: Pixels,
    /// How far the view scrolls at most: the content's height less the view's.
    pub max: Pixels,
    /// The view's height.
    pub viewport: Pixels,
}

/// The shortest the thumb gets, however long the content.
pub const MIN_THUMB: f32 = 24.;
/// The width of the strip the bar takes presses in, along the right edge.
pub const WIDTH: f32 = 12.;
/// The space between the track's ends and the container's top and bottom.
const PAD: f32 = 2.;

impl ScrollMetrics {
    /// Whether the content is taller than the view, so there is something to scroll.
    pub fn overflows(&self) -> bool {
        self.max > gpui::px(0.5) && self.viewport > gpui::px(0.)
    }

    /// The thumb's length in a track `track` long: the part of the content in view,
    /// at least `min`.
    pub fn thumb_len(&self, track: Pixels, min: Pixels) -> Pixels {
        let content = self.viewport + self.max;
        (track * (self.viewport / content)).max(min).min(track)
    }

    /// The thumb's top, from the track's top, when it is `len` long.
    pub fn thumb_top(&self, track: Pixels, len: Pixels) -> Pixels {
        let fraction = (self.offset / self.max).clamp(0., 1.);
        (track - len) * fraction
    }

    /// The offset that puts the thumb's top `top` below the track's top.
    pub fn offset_at(&self, top: Pixels, track: Pixels, len: Pixels) -> Pixels {
        let room = track - len;
        if room <= gpui::px(0.) {
            return gpui::px(0.);
        }
        self.max * (top / room).clamp(0., 1.)
    }
}

/// A view the [`Scrollbar`] can read and scroll.
pub trait ScrollSource: 'static {
    /// Where the view is now, or `None` before it has been laid out.
    fn metrics(&self, cx: &App) -> Option<ScrollMetrics>;
    /// Scrolls the view so its top is `offset` below the content's top (`0` to
    /// [`ScrollMetrics::max`]).
    fn scroll_to(&self, offset: Pixels, window: &mut Window, cx: &mut App);
    /// The thumb was pressed: a drag starts.
    fn drag_started(&self, _cx: &mut App) {}
    /// The thumb was let go.
    fn drag_ended(&self, _cx: &mut App) {}
}

impl ScrollSource for ListState {
    fn metrics(&self, _: &App) -> Option<ScrollMetrics> {
        let viewport = self.viewport_bounds().size.height;
        if viewport <= gpui::px(0.) {
            return None;
        }
        let max = self.max_offset_for_scrollbar().y;
        // At the end (following the tail) the list reports the whole content's
        // height, more than `max`.
        let offset = (-self.scroll_px_offset_for_scrollbar().y).clamp(gpui::px(0.), max);
        Some(ScrollMetrics {
            offset,
            max,
            viewport,
        })
    }

    fn scroll_to(&self, offset: Pixels, _: &mut Window, _: &mut App) {
        let max = self.max_offset_for_scrollbar().y;
        // The list's own end is its padding further down than `max`, and it keeps a
        // position past its end at the end. At the end, it follows its tail again
        // (in `FollowMode::Tail`).
        let offset = if offset >= max - gpui::px(0.5) {
            gpui::px(f32::MAX)
        } else {
            offset.max(gpui::px(0.))
        };
        self.set_offset_from_scrollbar(point(gpui::px(0.), -offset));
    }

    fn drag_started(&self, _: &mut App) {
        self.scrollbar_drag_started();
    }

    fn drag_ended(&self, _: &mut App) {
        self.scrollbar_drag_ended();
    }
}

fn handle_metrics(handle: &ScrollHandle) -> Option<ScrollMetrics> {
    let viewport = handle.bounds().size.height;
    if viewport <= gpui::px(0.) {
        return None;
    }
    let max = handle.max_offset().y.max(gpui::px(0.));
    Some(ScrollMetrics {
        offset: (-handle.offset().y).clamp(gpui::px(0.), max),
        max,
        viewport,
    })
}

fn handle_scroll_to(handle: &ScrollHandle, offset: Pixels) {
    let max = handle.max_offset().y.max(gpui::px(0.));
    let x = handle.offset().x;
    handle.set_offset(point(x, -offset.clamp(gpui::px(0.), max)));
}

impl ScrollSource for ScrollHandle {
    fn metrics(&self, _: &App) -> Option<ScrollMetrics> {
        handle_metrics(self)
    }

    fn scroll_to(&self, offset: Pixels, _: &mut Window, _: &mut App) {
        handle_scroll_to(self, offset);
    }
}

impl ScrollSource for UniformListScrollHandle {
    fn metrics(&self, _: &App) -> Option<ScrollMetrics> {
        handle_metrics(&self.0.borrow().base_handle)
    }

    fn scroll_to(&self, offset: Pixels, _: &mut Window, _: &mut App) {
        let mut state = self.0.borrow_mut();
        // A scroll to an item waiting for the next frame would undo this one.
        state.deferred_scroll_to_item = None;
        handle_scroll_to(&state.base_handle, offset);
    }
}

type ScrollHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// What the bar remembers between frames.
#[derive(Default)]
struct DragState {
    /// While the thumb is dragged: how far below the thumb's top it was pressed.
    grab: Option<Pixels>,
}

/// A vertical scroll bar over the right edge of a `relative` container; see the
/// module docs.
#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    source: Rc<dyn ScrollSource>,
    label: Option<SharedString>,
    on_scroll: Option<ScrollHandler>,
}

impl Scrollbar {
    pub fn new(id: impl Into<ElementId>, source: impl ScrollSource) -> Self {
        Self {
            id: id.into(),
            source: Rc::new(source),
            label: None,
            on_scroll: None,
        }
    }

    /// What the bar scrolls, for assistive technology.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Called after the bar scrolls the view (a drag, a press on the track,
    /// assistive technology) and when a drag ends.
    pub fn on_scroll(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_scroll = Some(Rc::new(handler));
        self
    }
}

/// What a frame draws and handles, worked out once the view is laid out.
struct Frame {
    metrics: ScrollMetrics,
    track: Bounds<Pixels>,
    thumb_top: Pixels,
    thumb_len: Pixels,
    hitbox: Hitbox,
}

impl Frame {
    fn thumb(&self) -> (Pixels, Pixels) {
        let top = self.track.top() + self.thumb_top;
        (top, top + self.thumb_len)
    }
}

fn scrolled(
    source: &Rc<dyn ScrollSource>,
    on_scroll: &Option<ScrollHandler>,
    offset: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    source.scroll_to(offset, window, cx);
    if let Some(on_scroll) = on_scroll {
        on_scroll(window, cx);
    }
    window.refresh();
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            source,
            label,
            on_scroll,
        } = self;
        let drag = window.use_keyed_state(id.clone(), cx, |_, _| DragState::default());
        // For assistive technology: where the view was at the last frame.
        let shown = source.metrics(cx).filter(ScrollMetrics::overflows);

        div()
            .id(id.clone())
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .w(px(WIDTH))
            .when_some(shown, |this, metrics| {
                let set = {
                    let source = source.clone();
                    let on_scroll = on_scroll.clone();
                    move |data: Option<&ActionData>, window: &mut Window, cx: &mut App| {
                        let value = match data {
                            Some(ActionData::NumericValue(value)) => *value,
                            Some(ActionData::Value(text)) => match text.trim().parse() {
                                Ok(value) => value,
                                Err(_) => return,
                            },
                            _ => return,
                        };
                        scrolled(&source, &on_scroll, gpui::px(value as f32), window, cx);
                    }
                };
                let page = |down: bool| {
                    let source = source.clone();
                    let on_scroll = on_scroll.clone();
                    move |_: Option<&ActionData>, window: &mut Window, cx: &mut App| {
                        let Some(now) = source.metrics(cx) else {
                            return;
                        };
                        let by = if down { now.viewport } else { -now.viewport };
                        scrolled(&source, &on_scroll, now.offset + by, window, cx);
                    }
                };
                this.accessible_id(Role::ScrollBar, &id, label.clone().unwrap_or_default())
                    .aria_orientation(accesskit::Orientation::Vertical)
                    .aria_numeric_value(f64::from(f32::from(metrics.offset)))
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(f64::from(f32::from(metrics.max)))
                    .on_a11y_action(AccessibleAction::SetValue, set)
                    .on_a11y_action(AccessibleAction::Increment, page(true))
                    .on_a11y_action(AccessibleAction::Decrement, page(false))
            })
            .child(
                canvas(
                    {
                        let source = source.clone();
                        move |bounds, window, cx| {
                            let metrics = source.metrics(cx).filter(ScrollMetrics::overflows)?;
                            let track = Bounds::from_corners(
                                point(bounds.left(), bounds.top() + px(PAD)),
                                point(bounds.right(), bounds.bottom() - px(PAD)),
                            );
                            let thumb_len = metrics.thumb_len(track.size.height, px(MIN_THUMB));
                            let thumb_top = metrics.thumb_top(track.size.height, thumb_len);
                            // Under nothing else: the view below keeps its hover and
                            // its wheel.
                            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                            Some(Frame {
                                metrics,
                                track,
                                thumb_top,
                                thumb_len,
                                hitbox,
                            })
                        }
                    },
                    move |bounds, frame, window, cx| {
                        let Some(frame) = frame else {
                            return;
                        };
                        paint(bounds, frame, source, on_scroll, drag, window, cx);
                    },
                )
                .size_full(),
            )
    }
}

fn paint(
    bounds: Bounds<Pixels>,
    frame: Frame,
    source: Rc<dyn ScrollSource>,
    on_scroll: Option<ScrollHandler>,
    drag: Entity<DragState>,
    window: &mut Window,
    cx: &mut App,
) {
    let frame = Rc::new(frame);
    let hovered = frame.hitbox.is_hovered(window);
    let dragging = drag.read(cx).grab.is_some();

    // The editor's thumb; wider and brighter under the pointer and while dragged.
    let (width, opacity) = if dragging {
        (px(8.), 55)
    } else if hovered {
        (px(8.), 45)
    } else {
        (px(6.), 30)
    };
    let (top, bottom) = frame.thumb();
    let thumb = Bounds::new(
        point(bounds.right() - px(PAD) - width, top),
        size(width, bottom - top),
    );
    window.paint_quad(fill(thumb, colors::GRAY_FADED.opacity(opacity)).corner_radii(width / 2.));
    window.set_cursor_style(CursorStyle::Arrow, &frame.hitbox);

    // Redraw when the pointer comes or goes.
    {
        let frame = frame.clone();
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && frame.hitbox.is_hovered(window) != hovered {
                window.refresh();
            }
        });
    }

    // A press on the thumb starts a drag; on the track, it scrolls a page that way.
    // The bar is drawn over the view, so it hears the press first, and keeps it.
    {
        let frame = frame.clone();
        let source = source.clone();
        let on_scroll = on_scroll.clone();
        let drag = drag.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.button != MouseButton::Left
                || !frame.hitbox.is_hovered(window)
            {
                return;
            }
            cx.stop_propagation();
            let y = event.position.y;
            let (top, bottom) = frame.thumb();
            if y < top || y > bottom {
                let page = frame.metrics.viewport;
                let by = if y < top { -page } else { page };
                scrolled(&source, &on_scroll, frame.metrics.offset + by, window, cx);
                return;
            }
            drag.update(cx, |drag, _| drag.grab = Some(y - top));
            source.drag_started(cx);
            window.refresh();
        });
    }

    let Some(grab) = drag.read(cx).grab else {
        return;
    };
    // While dragging, anywhere in the window.
    let end = {
        let source = source.clone();
        let on_scroll = on_scroll.clone();
        let drag = drag.clone();
        move |window: &mut Window, cx: &mut App| {
            drag.update(cx, |drag, _| drag.grab = None);
            source.drag_ended(cx);
            if let Some(on_scroll) = &on_scroll {
                on_scroll(window, cx);
            }
            window.refresh();
        }
    };
    {
        let end = end.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture {
                return;
            }
            if event.pressed_button != Some(MouseButton::Left) {
                // Let go outside the window.
                end(window, cx);
                return;
            }
            let top = event.position.y - grab - frame.track.top();
            let offset = frame
                .metrics
                .offset_at(top, frame.track.size.height, frame.thumb_len);
            scrolled(&source, &on_scroll, offset, window, cx);
        });
    }
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
            end(window, cx);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(offset: f32, max: f32, viewport: f32) -> ScrollMetrics {
        ScrollMetrics {
            offset: gpui::px(offset),
            max: gpui::px(max),
            viewport: gpui::px(viewport),
        }
    }

    #[test]
    fn the_thumb_is_the_part_in_view_and_sits_where_the_view_is() {
        // 100 px of 400 in view, in a 100 px track: a quarter of it.
        let at_top = metrics(0., 300., 100.);
        let len = at_top.thumb_len(gpui::px(100.), gpui::px(10.));
        assert_eq!(len, gpui::px(25.));
        assert_eq!(at_top.thumb_top(gpui::px(100.), len), gpui::px(0.));
        let middle = metrics(150., 300., 100.);
        assert_eq!(middle.thumb_top(gpui::px(100.), len), gpui::px(37.5));
        let end = metrics(300., 300., 100.);
        assert_eq!(end.thumb_top(gpui::px(100.), len), gpui::px(75.));
        // Back from a thumb position to an offset.
        assert_eq!(
            end.offset_at(gpui::px(37.5), gpui::px(100.), len),
            gpui::px(150.)
        );
        assert_eq!(
            end.offset_at(gpui::px(-20.), gpui::px(100.), len),
            gpui::px(0.)
        );
        assert_eq!(
            end.offset_at(gpui::px(500.), gpui::px(100.), len),
            gpui::px(300.)
        );
    }

    #[test]
    fn a_long_content_keeps_a_thumb_to_hold() {
        let long = metrics(0., 99_900., 100.);
        assert_eq!(long.thumb_len(gpui::px(100.), gpui::px(24.)), gpui::px(24.));
        // Content that fits has nothing to scroll.
        assert!(!metrics(0., 0., 100.).overflows());
        assert!(long.overflows());
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod ui_tests {
    use gpui::{
        Context, Entity, IntoElement, ListAlignment, ListState, Modifiers, MouseButton,
        ParentElement, Render, Styled, TestAppContext, UniformListScrollHandle, VisualTestContext,
        Window, div, list, point, px, uniform_list,
    };

    use super::{ScrollMetrics, ScrollSource, Scrollbar};
    use crate::components::log_view::{LogLine, LogSelection, log_view};

    const LINE: f32 = 20.;
    /// The test windows' views are 400 × 100 px: five lines show at a time.
    const HEIGHT: f32 = 100.;
    /// The bar's strip, from the view's left.
    const BAR_X: f32 = 395.;

    /// A log in a gpui `list`, with the bar.
    struct ListLog {
        selection: LogSelection,
        lines: Vec<(u64, String)>,
        list: ListState,
        /// Times the bar called back.
        scrolled: usize,
    }

    impl Render for ListLog {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let state = self.list.clone();
            let this = cx.entity().downgrade();
            div().size_full().child(
                div().w(px(400.)).h(px(HEIGHT)).child(
                    log_view(
                        "log",
                        &self.selection,
                        list(
                            self.list.clone(),
                            cx.processor(|this, ix: usize, _, _| {
                                let (id, text) = this.lines[ix].clone();
                                div()
                                    .h(px(LINE))
                                    .text_size(px(14.))
                                    .line_height(px(LINE))
                                    .child(LogLine::new(&this.selection, id, text))
                                    .into_any_element()
                            }),
                        )
                        .size_full(),
                        move |by, _, _| state.scroll_by(by),
                    )
                    .relative()
                    .child(
                        Scrollbar::new("log-scrollbar", self.list.clone()).on_scroll(
                            move |_, cx| {
                                this.update(cx, |this, _| this.scrolled += 1).ok();
                            },
                        ),
                    ),
                ),
            )
        }
    }

    fn open_list(cx: &mut TestAppContext, n: u64) -> (Entity<ListLog>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        let (log, cx) = cx.add_window_view(|_, cx| ListLog {
            selection: LogSelection::new(cx),
            lines: (0..n).map(|i| (i, format!("line {i} text"))).collect(),
            list: ListState::new(n as usize, ListAlignment::Top, px(1000.)),
            scrolled: 0,
        });
        cx.run_until_parked();
        // A second frame, with every line measured.
        log.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        (log, cx)
    }

    fn list_metrics(log: &Entity<ListLog>, cx: &mut VisualTestContext) -> Option<ScrollMetrics> {
        log.read_with(cx, |log, cx| log.list.metrics(cx))
    }

    #[gpui::test]
    fn a_list_s_thumb_matches_its_scroll(cx: &mut TestAppContext) {
        // 20 lines of 20 px: 400 px, 100 in view.
        let (log, cx) = open_list(cx, 20);
        let metrics = list_metrics(&log, cx).unwrap();
        assert_eq!(metrics.viewport, px(HEIGHT));
        assert_eq!(metrics.max, px(300.));
        assert_eq!(metrics.offset, px(0.));
        // The wheel moves the view; the bar reads it.
        log.update(cx, |log, _| log.list.scroll_by(px(150.)));
        cx.run_until_parked();
        assert_eq!(list_metrics(&log, cx).unwrap().offset, px(150.));
    }

    #[gpui::test]
    fn dragging_a_list_s_thumb_scrolls_it(cx: &mut TestAppContext) {
        let (log, cx) = open_list(cx, 20);
        let none = Modifiers::default();
        // The thumb is a quarter of the 96 px track (2 px in at each end): 24 px.
        cx.simulate_mouse_down(point(px(BAR_X), px(10.)), MouseButton::Left, none);
        // Half the room left (72 px): half way down.
        cx.simulate_mouse_move(point(px(BAR_X), px(46.)), MouseButton::Left, none);
        let offset = list_metrics(&log, cx).unwrap().offset;
        assert_eq!(offset, px(150.));
        // Off the bar, the drag goes on; past the end it stops at the end.
        cx.simulate_mouse_move(point(px(100.), px(400.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(100.), px(400.)), MouseButton::Left, none);
        cx.run_until_parked();
        let metrics = list_metrics(&log, cx).unwrap();
        assert_eq!(metrics.offset, metrics.max);
        // The drag didn't select anything.
        assert_eq!(log.read_with(cx, |log, _| log.selection.selection()), None);
        assert!(log.read_with(cx, |log, _| log.scrolled) >= 2);
    }

    #[gpui::test]
    fn a_press_on_the_track_scrolls_a_page(cx: &mut TestAppContext) {
        let (log, cx) = open_list(cx, 20);
        let none = Modifiers::default();
        // Below the thumb: a page down.
        cx.simulate_click(point(px(BAR_X), px(90.)), none);
        cx.run_until_parked();
        assert_eq!(list_metrics(&log, cx).unwrap().offset, px(HEIGHT));
        cx.simulate_click(point(px(BAR_X), px(90.)), none);
        cx.run_until_parked();
        assert_eq!(list_metrics(&log, cx).unwrap().offset, px(HEIGHT * 2.));
        // Above it: a page up.
        cx.simulate_click(point(px(BAR_X), px(5.)), none);
        cx.run_until_parked();
        assert_eq!(list_metrics(&log, cx).unwrap().offset, px(HEIGHT));
        assert_eq!(log.read_with(cx, |log, _| log.selection.selection()), None);
    }

    #[gpui::test]
    fn the_thumb_stops_and_resumes_following_the_tail(cx: &mut TestAppContext) {
        let (log, cx) = open_list(cx, 20);
        log.update(cx, |log, cx| {
            log.list.set_follow_mode(gpui::FollowMode::Tail);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(log.read_with(cx, |log, _| log.list.is_following_tail()));
        let metrics = list_metrics(&log, cx).unwrap();
        assert_eq!(metrics.offset, metrics.max);
        // The thumb is at the bottom; drag it up.
        let none = Modifiers::default();
        cx.simulate_mouse_down(point(px(BAR_X), px(85.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(BAR_X), px(40.)), MouseButton::Left, none);
        assert!(!log.read_with(cx, |log, _| log.list.is_following_tail()));
        // New lines come in: the view stays where it was dragged.
        let before = list_metrics(&log, cx).unwrap().offset;
        log.update(cx, |log, cx| {
            log.lines
                .extend((20..25).map(|i| (i, format!("line {i} text"))));
            log.list.splice(20..20, 5);
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(list_metrics(&log, cx).unwrap().offset, before);
        // Back to the bottom: following again.
        cx.simulate_mouse_move(point(px(BAR_X), px(300.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(BAR_X), px(300.)), MouseButton::Left, none);
        cx.run_until_parked();
        assert!(log.read_with(cx, |log, _| log.list.is_following_tail()));
    }

    #[gpui::test]
    fn no_bar_when_the_lines_fit(cx: &mut TestAppContext) {
        let (log, cx) = open_list(cx, 3);
        assert!(!list_metrics(&log, cx).unwrap().overflows());
        // A press where the bar would be selects, as anywhere in the log.
        let none = Modifiers::default();
        cx.simulate_mouse_down(point(px(BAR_X), px(10.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(BAR_X), px(50.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(BAR_X), px(50.)), MouseButton::Left, none);
        assert!(
            log.read_with(cx, |log, _| log.selection.selection())
                .is_some()
        );
    }

    #[gpui::test]
    fn the_log_still_selects_beside_the_bar(cx: &mut TestAppContext) {
        let (log, cx) = open_list(cx, 20);
        let none = Modifiers::default();
        // From line 0 to line 2, ending over the bar.
        cx.simulate_mouse_down(point(px(1.), px(10.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(200.), px(30.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(BAR_X), px(50.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(BAR_X), px(50.)), MouseButton::Left, none);
        let text = log.read_with(cx, |log, _| log.selection.text(&log.lines[..]));
        assert_eq!(
            text.as_deref(),
            Some("line 0 text\nline 1 text\nline 2 text")
        );
        // A press on the bar keeps the selection.
        cx.simulate_click(point(px(BAR_X), px(90.)), none);
        assert!(
            log.read_with(cx, |log, _| log.selection.selection())
                .is_some()
        );
        assert_eq!(list_metrics(&log, cx).unwrap().offset, px(HEIGHT));
    }

    /// The same in a `uniform_list`.
    struct Uniform {
        scroll: UniformListScrollHandle,
        count: usize,
    }

    impl Render for Uniform {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                div()
                    .relative()
                    .w(px(400.))
                    .h(px(HEIGHT))
                    .child(
                        uniform_list("rows", self.count, |range, _, _| {
                            range
                                .map(|i| div().h(px(LINE)).child(format!("row {i}")))
                                .collect()
                        })
                        .track_scroll(&self.scroll)
                        .size_full(),
                    )
                    .child(Scrollbar::new("rows-scrollbar", self.scroll.clone())),
            )
        }
    }

    #[gpui::test]
    fn a_uniform_list_s_thumb_matches_and_drags(cx: &mut TestAppContext) {
        cx.update(crate::components::init);
        let (rows, cx) = cx.add_window_view(|_, _| Uniform {
            scroll: UniformListScrollHandle::new(),
            count: 40,
        });
        cx.run_until_parked();
        let read = rows
            .read_with(cx, |rows, cx| rows.scroll.metrics(cx))
            .unwrap();
        // 40 rows of 20 px: 800 px, 100 in view.
        assert_eq!(read.viewport, px(HEIGHT));
        assert_eq!(read.max, px(700.));
        assert_eq!(read.offset, px(0.));
        let none = Modifiers::default();
        // The thumb: 100 / 800 of the 96 px track, so the 24 px least. Drag it a
        // third of the way down the 72 px of room.
        cx.simulate_mouse_down(point(px(BAR_X), px(10.)), MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(BAR_X), px(34.)), MouseButton::Left, none);
        cx.simulate_mouse_up(point(px(BAR_X), px(34.)), MouseButton::Left, none);
        cx.run_until_parked();
        let offset = rows
            .read_with(cx, |rows, cx| rows.scroll.metrics(cx))
            .unwrap()
            .offset;
        assert!((offset - px(700. / 3.)).abs() < px(0.5), "{offset:?}");
        // A press on the track below: a page down.
        cx.simulate_click(point(px(BAR_X), px(95.)), none);
        cx.run_until_parked();
        let paged = rows
            .read_with(cx, |rows, cx| rows.scroll.metrics(cx))
            .unwrap()
            .offset;
        assert!((paged - offset - px(HEIGHT)).abs() < px(0.5), "{paged:?}");
    }

    #[gpui::test]
    fn no_bar_when_the_rows_fit(cx: &mut TestAppContext) {
        cx.update(crate::components::init);
        let (rows, cx) = cx.add_window_view(|_, _| Uniform {
            scroll: UniformListScrollHandle::new(),
            count: 3,
        });
        cx.run_until_parked();
        let metrics = rows
            .read_with(cx, |rows, cx| rows.scroll.metrics(cx))
            .unwrap();
        assert!(!metrics.overflows());
        // A press there does nothing.
        cx.simulate_click(point(px(BAR_X), px(90.)), Modifiers::default());
        cx.run_until_parked();
        let after = rows
            .read_with(cx, |rows, cx| rows.scroll.metrics(cx))
            .unwrap();
        assert_eq!(after.offset, px(0.));
    }
}
