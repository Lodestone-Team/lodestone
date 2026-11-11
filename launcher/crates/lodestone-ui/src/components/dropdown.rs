//! A control and the list it opens: [`Dropdown`].
//!
//! The list is placed against the control's own bounds, measured each frame, not
//! where the control's parent happens to lay out a child after it. Before this, each
//! menu sat wherever its parent put an `anchored()` element, so every caller had to
//! wrap it just so (a column with `items_end`, a box of its own, an absolute box);
//! in a toolbar row the list landed under the next button.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels, Point,
    SharedString, Size, Styled, Window, div, point, prelude::FluentBuilder as _,
};

use super::FocusScope;
use crate::theme::px;

/// How long an animated menu takes to open or close (the dashboard's menus take
/// 150 ms to open).
const POPOVER_MOTION: Duration = Duration::from_millis(150);
/// How far an animated menu slides down as it opens (the dashboard's `-translate-y-1`).
const POPOVER_SLIDE: f32 = 4.;
/// Space between the control and its list.
const GAP: f32 = 4.;
/// The list keeps this far from the window's edges.
const MARGIN: f32 = 8.;
/// Drawn above the page (and under [`window_layer`](super::window_layer)'s dialogs).
const PRIORITY: usize = 1;

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Which edge of the control the list lines up with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// The list's left edge on the control's left edge.
    Start,
    /// The list's right edge on the control's right edge.
    End,
}

/// A control with the list it opens, drawn over the page.
///
/// The list goes right under the control, lined up with its left edge (or its right
/// edge, with [`align_end`](Self::align_end)), and above it when there's no room
/// below; it is moved in to stay inside the window, and never covers the control
/// (one too tall for either side goes on the roomier one). It follows the
/// control as the page scrolls. The position comes from the control's bounds, so it
/// doesn't matter what the control sits in: put the `Dropdown` where the control goes.
///
/// While open, the list takes focus and keeps Tab inside it. Escape, or a press
/// anywhere outside it, calls `on_dismiss`, which should close it; a press on the
/// control itself is left to the control, so its own click toggles the list closed.
///
/// ```ignore
/// Dropdown::new(select_button("sort", label).on_press(toggle))
///     .when(open, |this| this.popover("sort-menu", popover_panel("sort-list"), close))
/// ```
pub struct Dropdown {
    trigger: AnyElement,
    popover: Option<Popover>,
    align: Align,
    priority: usize,
}

struct Popover {
    id: SharedString,
    panel: AnyElement,
    on_dismiss: DismissHandler,
    open: bool,
    animated: bool,
}

impl Dropdown {
    pub fn new(trigger: impl IntoElement) -> Self {
        Self {
            trigger: trigger.into_any_element(),
            popover: None,
            align: Align::Start,
            priority: PRIORITY,
        }
    }

    /// Shows `panel` (usually a [`popover_panel`](super::settings::popover_panel))
    /// under the control. Add it only while the list is open.
    pub fn popover(
        self,
        id: impl Into<SharedString>,
        panel: impl IntoElement,
        on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.with_popover(id, true, false, panel, on_dismiss)
    }

    /// [`popover`](Self::popover) that fades in, sliding down a little, as it opens,
    /// and fades out as it closes; with reduced motion it only fades (the dashboard's
    /// power menu). Add it whether open or not: it draws nothing while closed. A
    /// closing panel still takes clicks for that moment, so give its items their
    /// handlers only while it is open.
    pub fn animated_popover(
        self,
        id: impl Into<SharedString>,
        open: bool,
        panel: impl IntoElement,
        on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.with_popover(id, open, true, panel, on_dismiss)
    }

    fn with_popover(
        mut self,
        id: impl Into<SharedString>,
        open: bool,
        animated: bool,
        panel: impl IntoElement,
        on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.popover = Some(Popover {
            id: id.into(),
            panel: panel.into_any_element(),
            on_dismiss: Rc::new(on_dismiss),
            open,
            animated,
        });
        self
    }

    /// Lines the list's right edge up with the control's, for a control at the right
    /// of a row.
    pub fn align_end(mut self) -> Self {
        self.align = Align::End;
        self
    }

    /// Draws the list above other deferred layers, such as the dialogs of
    /// [`window_layer`](super::window_layer) (priority 2).
    pub fn priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

impl Popover {
    /// The list as drawn this frame, and how far up it is while sliding in; `None`
    /// once it has faded out.
    fn build(
        self,
        trigger: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<(AnyElement, Pixels)> {
        let (shown, slide) = if self.animated {
            let shown = super::motion::tween_even_reduced(
                ElementId::Name(format!("{}-motion", self.id).into()),
                if self.open { 1. } else { 0. },
                POPOVER_MOTION,
                window,
                cx,
            );
            if !self.open && shown <= 0. {
                return None;
            }
            let slide = if super::prefers_reduced_motion() {
                0.
            } else {
                POPOVER_SLIDE
            };
            (shown, slide * (1. - shown))
        } else {
            (1., 0.)
        };
        let on_dismiss = self.on_dismiss;
        let dismiss_outside = on_dismiss.clone();
        let element = div()
            .opacity(shown)
            .when(self.open, |this| {
                this.on_mouse_down_out(move |event, window, cx| {
                    // A press on the control is the control's: closing here would have
                    // its click open the list again.
                    if !trigger.contains(&event.position) {
                        dismiss_outside(window, cx);
                    }
                })
            })
            .child(
                FocusScope::new(self.id)
                    .arrow_keys()
                    .on_dismiss(move |window, cx| on_dismiss(window, cx))
                    .child(self.panel),
            )
            .into_any_element();
        Some((element, px(slide)))
    }
}

/// Where a list of `menu` size goes for a control at `trigger`, in a window of
/// `window` size: under the control, or above it when it fits there and not below;
/// lined up with the control's `align` edge, and moved in to stay inside the window.
/// It never covers the control: when it fits on neither side, it goes on the side
/// with more room.
pub fn place(
    trigger: Bounds<Pixels>,
    menu: Size<Pixels>,
    window: Size<Pixels>,
    align: Align,
) -> Point<Pixels> {
    let (gap, margin) = (px(GAP), px(MARGIN));
    let below = trigger.bottom() + gap;
    let above = trigger.top() - gap - menu.height;
    let room_below = window.height - margin - below;
    let room_above = trigger.top() - gap - margin;
    let y = if menu.height <= room_below || (menu.height > room_above && room_below >= room_above) {
        below
    } else {
        above
    };
    let x = match align {
        Align::Start => trigger.left(),
        Align::End => trigger.right() - menu.width,
    };
    let x = x.min(window.width - margin - menu.width).max(margin);
    point(x, y)
}

impl IntoElement for Dropdown {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Dropdown {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    /// Lays out as the control alone: the list is laid out on its own, over the page.
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.trigger.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.trigger.prepaint(window, cx);
        let Some((mut menu, slide)) = self
            .popover
            .take()
            .and_then(|popover| popover.build(bounds, window, cx))
        else {
            return;
        };
        let size = menu.layout_as_root(AvailableSpace::min_size(), window, cx);
        let mut origin = place(bounds, size, window.viewport_size(), self.align);
        origin.y -= slide;
        let origin = point(origin.x.round(), origin.y.round());
        window.defer_draw(menu, origin, self.priority, None);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.trigger.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use gpui::{bounds, px, size};

    use super::*;

    const WINDOW: Size<Pixels> = Size {
        width: px(800.),
        height: px(600.),
    };

    fn button(x: f32, y: f32) -> Bounds<Pixels> {
        bounds(point(px(x), px(y)), size(px(120.), px(32.)))
    }

    #[test]
    fn goes_under_the_control_on_its_left_edge() {
        let at = place(
            button(100., 50.),
            size(px(200.), px(150.)),
            WINDOW,
            Align::Start,
        );
        assert_eq!(at, point(px(100.), px(50. + 32. + GAP)));
    }

    #[test]
    fn end_aligned_lines_up_the_right_edges() {
        let at = place(
            button(300., 50.),
            size(px(200.), px(150.)),
            WINDOW,
            Align::End,
        );
        assert_eq!(at, point(px(300. + 120. - 200.), px(50. + 32. + GAP)));
    }

    #[test]
    fn flips_above_near_the_bottom_edge() {
        let control = button(100., 500.);
        let at = place(control, size(px(200.), px(150.)), WINDOW, Align::Start);
        assert_eq!(at.y, px(500. - GAP - 150.));
        assert!(
            at.y + px(150.) <= control.top(),
            "it doesn't cover the control"
        );
    }

    #[test]
    fn stays_below_when_it_fits_nowhere_and_below_has_more_room() {
        let control = button(100., 100.);
        let at = place(control, size(px(200.), px(700.)), WINDOW, Align::Start);
        assert_eq!(at.y, control.bottom() + px(GAP));
    }

    #[test]
    fn goes_above_when_it_fits_nowhere_and_above_has_more_room() {
        let control = button(100., 400.);
        let at = place(control, size(px(200.), px(700.)), WINDOW, Align::Start);
        assert_eq!(at.y, px(400. - GAP - 700.));
    }

    #[test]
    fn is_moved_in_at_the_sides() {
        let wide = size(px(300.), px(100.));
        let right = place(button(700., 50.), wide, WINDOW, Align::Start);
        assert_eq!(right.x, WINDOW.width - px(MARGIN) - px(300.));
        let left = place(button(10., 50.), wide, WINDOW, Align::End);
        assert_eq!(left.x, px(MARGIN));
    }
}

/// Window tests, which need GPUI's `test-support` feature (the `screenshot` feature):
/// `cargo test -p lodestone-ui --features screenshot`.
#[cfg(all(test, feature = "screenshot"))]
mod window_tests {
    use gpui::{
        Context, Entity, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext,
        prelude::*,
    };

    use super::*;

    /// A toolbar of two controls, the first of which opens a list.
    struct Toolbar {
        open: bool,
        dismissed: usize,
    }

    impl Render for Toolbar {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let open = self.open;
            div()
                .size_full()
                .flex()
                .flex_row()
                .gap_2()
                .p_4()
                .child(
                    Dropdown::new(
                        div()
                            .id("sort")
                            .debug_selector(|| "SORT".into())
                            .w(px(120.))
                            .h(px(32.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open = !this.open;
                                cx.notify();
                            })),
                    )
                    .when(open, |this| {
                        this.popover(
                            "sort-menu",
                            div()
                                .id("sort-list")
                                .debug_selector(|| "LIST".into())
                                .occlude()
                                .w(px(180.))
                                .h(px(100.)),
                            {
                                let view = cx.entity().downgrade();
                                move |_: &mut Window, cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.open = false;
                                        this.dismissed += 1;
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            },
                        )
                    }),
                )
                .child(
                    div()
                        .id("filters")
                        .debug_selector(|| "FILTERS".into())
                        .w(px(120.))
                        .h(px(32.)),
                )
        }
    }

    fn toolbar(cx: &mut TestAppContext) -> (Entity<Toolbar>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        cx.add_window_view(|_, _| Toolbar {
            open: true,
            dismissed: 0,
        })
    }

    fn debug_bounds(name: &'static str, cx: &mut VisualTestContext) -> Option<Bounds<Pixels>> {
        cx.debug_bounds(name)
    }

    fn mouse_down(at: Point<Pixels>, cx: &mut VisualTestContext) {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    fn is_open(view: &Entity<Toolbar>, cx: &mut VisualTestContext) -> bool {
        view.read_with(cx, |this, _| this.open)
    }

    #[test]
    fn in_a_toolbar_the_list_sits_under_its_own_control() {
        let mut cx = TestAppContext::single();
        let (_view, cx) = toolbar(&mut cx);
        cx.run_until_parked();
        let sort = debug_bounds("SORT", cx).expect("the control is drawn");
        let filters = debug_bounds("FILTERS", cx).expect("the next control is drawn");
        let list = debug_bounds("LIST", cx).expect("the list is drawn");
        assert_eq!(list.left(), sort.left());
        assert_eq!(list.top(), sort.bottom() + px(GAP));
        assert!(list.left() < filters.left());
    }

    #[test]
    fn a_press_outside_closes_it() {
        let mut cx = TestAppContext::single();
        let (view, cx) = toolbar(&mut cx);
        cx.run_until_parked();
        let filters = debug_bounds("FILTERS", cx).unwrap();
        mouse_down(filters.center(), cx);
        assert!(!is_open(&view, cx));
    }

    #[test]
    fn a_press_inside_keeps_it_open() {
        let mut cx = TestAppContext::single();
        let (view, cx) = toolbar(&mut cx);
        cx.run_until_parked();
        let list = debug_bounds("LIST", cx).unwrap();
        mouse_down(list.center(), cx);
        assert!(is_open(&view, cx));
    }

    #[test]
    fn a_click_on_its_control_closes_it_and_doesnt_reopen_it() {
        let mut cx = TestAppContext::single();
        let (view, cx) = toolbar(&mut cx);
        cx.run_until_parked();
        let sort = debug_bounds("SORT", cx).unwrap();
        cx.simulate_click(sort.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(!is_open(&view, cx));
        assert_eq!(view.read_with(cx, |this, _| this.dismissed), 0);
        assert!(debug_bounds("LIST", cx).is_none());
    }

    #[test]
    fn escape_closes_it() {
        let mut cx = TestAppContext::single();
        let (view, cx) = toolbar(&mut cx);
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        assert!(!is_open(&view, cx));
    }
}
