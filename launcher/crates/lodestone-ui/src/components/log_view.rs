//! A long list of lines, such as a log or a console, that selects like one big
//! read-only text field.
//!
//! [`RichText`](super::RichText) selects inside one element. A log is drawn as a
//! virtualized list with one element per line, so its selection must run across
//! lines, including lines scrolled out of view. Here:
//!
//! - [`LogSelection`] keeps the selection. It is anchored to line ids, not to list
//!   positions, so new lines, trimmed lines and a changed filter don't move it.
//! - [`LogLine`] draws one line's text (styled as the caller likes) and its part of
//!   the selection. Put it in each row of the list.
//! - [`log_view`] wraps the list. It takes the presses and drags: press and drag
//!   across lines (the list scrolls while the pointer is above or below it),
//!   Shift-click extends, double-click selects a word, triple-click a line. A press
//!   focuses it; the right-click menu has Copy.
//!
//! The caller owns the lines and gives them through [`LineSource`]. It handles
//! [`SelectAll`] (⌘A / Ctrl+A) and [`CopySelection`] (⌘C / Ctrl+C, and the menu's
//! Copy) on the element `log_view` returns, with [`LogSelection::select_all`] and
//! [`LogSelection::copy`]. Copies are plain text, lines joined with `\n`.

use std::cell::RefCell;
use std::ops::Range;
use std::panic::Location;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, CursorStyle, DispatchPhase, Div, Element, ElementId,
    FocusHandle, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, InteractiveElement,
    IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, SharedString, Stateful, Styled, StyledText, TextLayout, Window,
    actions, div, fill, point,
};

use super::rich_text::{
    CopySelection, copy_menu, floor_char_boundary, line_at, paint_selection, word_at,
};
use crate::theme::px;

actions!(log_view, [SelectAll]);

/// The key context of the element [`log_view`] returns.
pub const CONTEXT: &str = "LogView";

/// Binds ⌘A / Ctrl+A to [`SelectAll`] inside a log. (⌘C is `CopySelection`'s own
/// binding.)
pub(super) fn init(cx: &mut App) {
    let binding = KeyBinding::load(
        "secondary-a",
        Box::new(SelectAll),
        Some(
            gpui::KeyBindingContextPredicate::parse(CONTEXT)
                .expect("valid context")
                .into(),
        ),
        false,
        None,
        cx,
    )
    .expect("valid select-all shortcut");
    cx.bind_keys([binding]);
}

/// The lines a log shows, in order. Ids must grow down the list (a line number
/// that never repeats, a snowflake): the selection keeps to them.
pub trait LineSource {
    fn line_count(&self) -> usize;
    fn line_id(&self, ix: usize) -> u64;
    /// The line's plain text, as copied. It is the text its [`LogLine`] draws.
    fn line_text(&self, ix: usize) -> &str;
}

/// For tests and small lists: `(id, text)` pairs.
impl<S: AsRef<str>> LineSource for [(u64, S)] {
    fn line_count(&self) -> usize {
        self.len()
    }

    fn line_id(&self, ix: usize) -> u64 {
        self[ix].0
    }

    fn line_text(&self, ix: usize) -> &str {
        self[ix].1.as_ref()
    }
}

/// The first line at or after `id`, or `line_count` if none is.
fn first_at_or_after(lines: &(impl LineSource + ?Sized), id: u64) -> usize {
    let (mut low, mut high) = (0, lines.line_count());
    while low < high {
        let mid = low + (high - low) / 2;
        if lines.line_id(mid) < id {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

/// A place in the log: a line, and a byte offset in its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Caret {
    pub line: u64,
    pub offset: usize,
}

impl Caret {
    pub fn new(line: u64, offset: usize) -> Self {
        Self { line, offset }
    }
}

/// A selection: from where it started (`anchor`) to where it is now (`head`),
/// either way round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Caret,
    pub head: Caret,
}

impl Selection {
    pub fn new(anchor: Caret, head: Caret) -> Self {
        Self { anchor, head }
    }

    /// Every line `lines` has, or nothing if it has none.
    pub fn all(lines: &(impl LineSource + ?Sized)) -> Option<Self> {
        let last = lines.line_count().checked_sub(1)?;
        Some(Self::new(
            Caret::new(lines.line_id(0), 0),
            Caret::new(lines.line_id(last), lines.line_text(last).len()),
        ))
    }

    pub fn start(&self) -> Caret {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> Caret {
        self.anchor.max(self.head)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The part of line `id` (whose text is `text`) that is selected, and whether
    /// the selection goes on past its end (so takes its line break).
    pub fn in_line(&self, id: u64, text: &str) -> Option<(Range<usize>, bool)> {
        let (start, end) = (self.start(), self.end());
        if self.is_empty() || id < start.line || id > end.line {
            return None;
        }
        let from = if id == start.line {
            floor_char_boundary(text, start.offset)
        } else {
            0
        };
        let to = if id == end.line {
            floor_char_boundary(text, end.offset)
        } else {
            text.len()
        };
        Some((from.min(to)..to, id < end.line))
    }

    /// The selected text in `lines`, lines joined with `\n`. Lines no longer there
    /// (trimmed, or hidden by a filter) are left out; a selection whose first line
    /// went starts at the next one's start.
    pub fn text(&self, lines: &(impl LineSource + ?Sized)) -> String {
        let end = self.end();
        let mut text = String::new();
        let mut first = true;
        for ix in first_at_or_after(lines, self.start().line)..lines.line_count() {
            let id = lines.line_id(ix);
            if id > end.line {
                break;
            }
            let line = lines.line_text(ix);
            let Some((range, _)) = self.in_line(id, line) else {
                continue;
            };
            if !first {
                text.push('\n');
            }
            first = false;
            text.push_str(&line[range]);
        }
        text
    }
}

/// What a press and drag selects by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Unit {
    #[default]
    Char,
    Word,
    Line,
}

impl Unit {
    /// The unit around `offset` in `text`.
    fn around(self, text: &str, offset: usize) -> Range<usize> {
        match self {
            Unit::Char => offset..offset,
            Unit::Word => word_at(text, offset),
            Unit::Line => line_at(text, offset),
        }
    }
}

/// The selection from a press on `origin` (a place, word or line) dragged to
/// `unit` (the same kind of thing): both, whole.
fn extend(origin: (Caret, Caret), unit: (Caret, Caret)) -> Selection {
    if unit.0 < origin.0 {
        Selection::new(origin.1, unit.0)
    } else {
        Selection::new(origin.0, unit.1.max(origin.1))
    }
}

/// A line drawn this frame, for finding the line under the pointer.
struct Painted {
    id: u64,
    bounds: Bounds<Pixels>,
    layout: TextLayout,
    text: SharedString,
}

type DragHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ScrollBy = Rc<dyn Fn(Pixels, &mut Window, &mut App)>;

struct State {
    focus: FocusHandle,
    selection: Option<Selection>,
    unit: Unit,
    /// The word or line the press selected: a drag by words or lines keeps it.
    origin: (Caret, Caret),
    /// The left button is down and moving it moves the head.
    dragging: bool,
    /// The right-click menu is open, at this window position.
    menu: Option<Point<Pixels>>,
    /// The lines drawn in the last frame, top to bottom.
    painted: Vec<Painted>,
    on_drag: Option<DragHandler>,
}

impl State {
    /// The place under `position`: on the line at that height (the nearest one
    /// above or below the lines drawn), and the text's line under it.
    fn caret_at(&self, position: Point<Pixels>) -> Option<(Caret, SharedString)> {
        let line = self
            .painted
            .iter()
            .find(|line| position.y < line.bounds.bottom())
            .or_else(|| self.painted.last())?;
        let index = line
            .layout
            .index_for_position(position)
            .unwrap_or_else(|index| index);
        let offset = floor_char_boundary(&line.text, index);
        Some((Caret::new(line.id, offset), line.text.clone()))
    }

    /// Starts a selection by `unit` at `caret`, on a line whose text is `text`.
    fn start(&mut self, caret: Caret, text: &str, unit: Unit) {
        let range = unit.around(text, caret.offset);
        let origin = (
            Caret::new(caret.line, range.start),
            Caret::new(caret.line, range.end),
        );
        self.unit = unit;
        self.origin = origin;
        self.selection = Some(Selection::new(origin.0, origin.1));
    }

    /// Moves the head to `position`, keeping the press's word or line whole.
    /// Returns whether the selection changed.
    fn drag_to(&mut self, position: Point<Pixels>) -> bool {
        let Some((caret, text)) = self.caret_at(position) else {
            return false;
        };
        let range = self.unit.around(&text, caret.offset);
        let next = extend(
            self.origin,
            (
                Caret::new(caret.line, range.start),
                Caret::new(caret.line, range.end),
            ),
        );
        let changed = self.selection != Some(next);
        self.selection = Some(next);
        changed
    }
}

/// The selection of one log, shared by the page that shows it, its [`LogLine`]s
/// and [`log_view`]. Cheap to clone.
#[derive(Clone)]
pub struct LogSelection(Rc<RefCell<State>>);

impl LogSelection {
    pub fn new(cx: &mut App) -> Self {
        Self(Rc::new(RefCell::new(State {
            focus: cx.focus_handle(),
            selection: None,
            unit: Unit::Char,
            origin: (Caret::new(0, 0), Caret::new(0, 0)),
            dragging: false,
            menu: None,
            painted: Vec::new(),
            on_drag: None,
        })))
    }

    /// Calls `f(true)` when a press starts a selection and `f(false)` when it is
    /// released: a log that follows new lines pauses meanwhile, as the text would
    /// otherwise move under the pointer.
    pub fn on_drag(&self, f: impl Fn(bool, &mut Window, &mut App) + 'static) {
        self.0.borrow_mut().on_drag = Some(Rc::new(f));
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.0.borrow().focus.clone()
    }

    /// Whether the left button is down, selecting.
    pub fn is_dragging(&self) -> bool {
        self.0.borrow().dragging
    }

    pub fn selection(&self) -> Option<Selection> {
        self.0.borrow().selection
    }

    /// Sets the selection (for tests and screenshots).
    pub fn set(&self, selection: Option<Selection>) {
        let mut state = self.0.borrow_mut();
        state.selection = selection;
        state.unit = Unit::Char;
        state.menu = None;
    }

    pub fn clear(&self) {
        self.set(None);
    }

    /// Selects every line `lines` has.
    pub fn select_all(&self, lines: &(impl LineSource + ?Sized)) {
        self.set(Selection::all(lines));
    }

    /// The selected text, if any is selected.
    pub fn text(&self, lines: &(impl LineSource + ?Sized)) -> Option<String> {
        let selection = self.selection()?;
        let text = selection.text(lines);
        (!text.is_empty()).then_some(text)
    }

    /// Copies the selected text, and says whether there was any.
    pub fn copy(&self, lines: &(impl LineSource + ?Sized), cx: &mut App) -> bool {
        let Some(text) = self.text(lines) else {
            return false;
        };
        // The size only: what was copied may be anything in the log.
        super::trace_ui(
            "ui.copy",
            "log",
            &ElementId::Name("log".into()),
            serde_json::json!({
                "length": text.chars().count(),
                "lines": text.lines().count(),
            }),
        );
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    fn drag_handler(&self) -> Option<DragHandler> {
        self.0.borrow().on_drag.clone()
    }

    fn end_drag(&self, window: &mut Window, cx: &mut App) {
        let was = std::mem::take(&mut self.0.borrow_mut().dragging);
        if was && let Some(on_drag) = self.drag_handler() {
            on_drag(false, window, cx);
        }
    }
}

/// One line of a log: `text` drawn as `styled` (which must draw that text), with
/// its part of the selection.
pub struct LogLine {
    selection: LogSelection,
    id: u64,
    text: SharedString,
    styled: StyledText,
}

impl LogLine {
    /// Plain text in the surrounding style.
    pub fn new(selection: &LogSelection, id: u64, text: impl Into<SharedString>) -> Self {
        let text = text.into();
        Self::styled(selection, id, text.clone(), StyledText::new(text))
    }

    /// Text styled by the caller, such as a console line with its colours.
    pub fn styled(
        selection: &LogSelection,
        id: u64,
        text: impl Into<SharedString>,
        styled: StyledText,
    ) -> Self {
        Self {
            selection: selection.clone(),
            id,
            text: text.into(),
            styled,
        }
    }
}

impl IntoElement for LogLine {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LogLine {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.styled.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.styled
            .prepaint(None, inspector_id, bounds, state, window, cx);
        self.selection.0.borrow_mut().painted.push(Painted {
            id: self.id,
            bounds,
            layout: self.styled.layout().clone(),
            text: self.text.clone(),
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let selected = self
            .selection
            .0
            .borrow()
            .selection
            .and_then(|selection| selection.in_line(self.id, &self.text));
        if let Some((range, continues)) = selected {
            paint_line_selection(self.styled.layout(), range, continues, bounds, window);
        }
        self.styled
            .paint(None, inspector_id, bounds, state, &mut (), window, cx);
    }
}

/// Fills a line's selected part, and a little past its end when the selection
/// takes its line break.
fn paint_line_selection(
    layout: &TextLayout,
    range: Range<usize>,
    continues: bool,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    // A truncated line draws less text than it has.
    let drawn = layout.text();
    let clamp = |index: usize| floor_char_boundary(&drawn, index);
    let range = clamp(range.start)..clamp(range.end);
    if !range.is_empty() {
        paint_selection(layout, range.clone(), bounds, window);
    }
    if continues && let Some(end) = layout.position_for_index(range.end) {
        let color = crate::theme::accent::faded().opacity(50);
        window.paint_quad(fill(
            Bounds::from_corners(end, point(end.x + px(6.), end.y + layout.line_height())),
            color,
        ));
    }
}

/// Wraps a list of [`LogLine`]s (a gpui `list` or `uniform_list`, `size_full`) so
/// the pointer selects across them; see the module docs. `scroll_by` scrolls the
/// list down by a distance (up when negative), for selecting past its edges.
///
/// The caller adds the actions:
///
/// ```ignore
/// log_view("console-log", &selection, list, move |by, _, _| list_state.scroll_by(by))
///     .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
///         this.selection.select_all(&this.lines);
///         cx.notify();
///     }))
///     .on_action(cx.listener(|this, _: &CopySelection, _, cx| {
///         this.selection.copy(&this.lines, cx);
///     }))
/// ```
pub fn log_view(
    id: impl Into<ElementId>,
    selection: &LogSelection,
    list: impl IntoElement,
    scroll_by: impl Fn(Pixels, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .key_context(CONTEXT)
        .track_focus(&selection.focus_handle())
        .size_full()
        .child(Tracker {
            selection: selection.clone(),
            child: list.into_any_element(),
            scroll_by: Rc::new(scroll_by),
        })
}

/// Takes the presses and drags over the list it wraps.
struct Tracker {
    selection: LogSelection,
    child: AnyElement,
    scroll_by: ScrollBy,
}

impl IntoElement for Tracker {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// How far the list scrolls in a frame while the pointer is `distance` past its
/// edge.
fn scroll_step(distance: Pixels) -> Pixels {
    (distance / 3.).clamp(px(2.), px(40.))
}

impl Element for Tracker {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Under the lines, so they and their rows keep their hover.
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let state = &self.selection.0;
        let dragging = state.borrow().dragging;
        let mouse = window.mouse_position();
        if dragging {
            // Past an edge: scroll that way, faster the further out, every frame.
            let by = if mouse.y < bounds.top() {
                -scroll_step(bounds.top() - mouse.y)
            } else if mouse.y > bounds.bottom() {
                scroll_step(mouse.y - bounds.bottom())
            } else {
                px(0.)
            };
            if by != px(0.) {
                (self.scroll_by)(by, window, cx);
                window.request_animation_frame();
            }
        }
        state.borrow_mut().painted.clear();
        self.child.prepaint(window, cx);
        if dragging {
            state.borrow_mut().drag_to(mouse);
        }
        let menu_at = state.borrow().menu;
        if let Some(position) = menu_at {
            let selection = self.selection.clone();
            let mut menu = copy_menu(position, move |window, cx| {
                let focus = selection.focus_handle();
                focus.dispatch_action(&CopySelection, window, cx);
                selection.0.borrow_mut().menu = None;
            });
            menu.layout_as_root(window.viewport_size().into(), window, cx);
            window.defer_draw(menu, Point::default(), 2, None);
        }
        hitbox
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        // A press here selects.
        {
            let hitbox = hitbox.clone();
            let selection = self.selection.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                    return;
                }
                let found = selection.0.borrow().caret_at(event.position);
                let Some((caret, text)) = found else {
                    return;
                };
                let focus = selection.focus_handle();
                window.focus(&focus, cx);
                // One selection at a time.
                super::rich_text::clear_selection();
                match event.button {
                    MouseButton::Left => {
                        {
                            let mut state = selection.0.borrow_mut();
                            state.menu = None;
                            match (event.modifiers.shift, state.selection) {
                                (true, Some(current)) => {
                                    state.unit = Unit::Char;
                                    state.origin = (current.anchor, current.anchor);
                                    state.selection = Some(Selection::new(current.anchor, caret));
                                }
                                _ => {
                                    let unit = match event.click_count {
                                        0 | 1 => Unit::Char,
                                        2 => Unit::Word,
                                        _ => Unit::Line,
                                    };
                                    state.start(caret, &text, unit);
                                }
                            }
                            state.dragging = true;
                        }
                        if let Some(on_drag) = selection.drag_handler() {
                            on_drag(true, window, cx);
                        }
                    }
                    MouseButton::Right => {
                        let mut state = selection.0.borrow_mut();
                        let inside = state.selection.is_some_and(|s| {
                            !s.is_empty() && s.start() <= caret && caret <= s.end()
                        });
                        if !inside {
                            state.start(caret, &text, Unit::Word);
                        }
                        state.menu = Some(event.position);
                    }
                    _ => return,
                }
                cx.stop_propagation();
                window.refresh();
            });
        }

        let (has_selection, dragging, menu_open) = {
            let state = self.selection.0.borrow();
            (
                state.selection.is_some() || state.menu.is_some(),
                state.dragging,
                state.menu.is_some(),
            )
        };

        // A press anywhere else clears it. With the menu open, the menu's Copy takes
        // the press first (in the bubble phase), so clear after it instead.
        if has_selection {
            let hitbox = hitbox.clone();
            let selection = self.selection.clone();
            window.on_mouse_event(move |_: &MouseDownEvent, phase, window, _| {
                let now = if menu_open {
                    DispatchPhase::Bubble
                } else {
                    DispatchPhase::Capture
                };
                if phase == now && !hitbox.is_hovered(window) {
                    selection.clear();
                    window.refresh();
                }
            });
        }

        if !dragging {
            return;
        }
        {
            let selection = self.selection.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    // Released outside the window.
                    selection.end_drag(window, cx);
                } else {
                    selection.0.borrow_mut().drag_to(event.position);
                }
                window.refresh();
            });
        }
        let selection = self.selection.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                return;
            }
            selection.0.borrow_mut().drag_to(event.position);
            selection.end_drag(window, cx);
            window.refresh();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(range: Range<u64>) -> Vec<(u64, String)> {
        range.map(|n| (n, format!("line {n}"))).collect()
    }

    #[test]
    fn a_selection_runs_across_lines_either_way() {
        let lines = lines(10..20);
        let forward = Selection::new(Caret::new(11, 2), Caret::new(13, 4));
        assert_eq!(forward.text(&lines[..]), "ne 11\nline 12\nline");
        let backward = Selection::new(Caret::new(13, 4), Caret::new(11, 2));
        assert_eq!(backward.text(&lines[..]), forward.text(&lines[..]));
        // In one line.
        let one = Selection::new(Caret::new(12, 5), Caret::new(12, 7));
        assert_eq!(one.text(&lines[..]), "12");
        assert_eq!(one.in_line(12, "line 12"), Some((5..7, false)));
        assert_eq!(one.in_line(13, "line 13"), None);
        // A middle line is whole and takes its line break; the last doesn't.
        assert_eq!(forward.in_line(12, "line 12"), Some((0..7, true)));
        assert_eq!(forward.in_line(11, "line 11"), Some((2..7, true)));
        assert_eq!(forward.in_line(13, "line 13"), Some((0..4, false)));
        // Empty: nothing.
        let empty = Selection::new(Caret::new(12, 3), Caret::new(12, 3));
        assert!(empty.is_empty());
        assert_eq!(empty.text(&lines[..]), "");
        assert_eq!(empty.in_line(12, "line 12"), None);
    }

    #[test]
    fn select_all_takes_every_line_shown() {
        let lines = lines(5..9);
        let all = Selection::all(&lines[..]).unwrap();
        assert_eq!(all.text(&lines[..]), "line 5\nline 6\nline 7\nline 8");
        assert_eq!(Selection::all(&Vec::<(u64, String)>::new()[..]), None);
        // An empty line in the middle keeps its place.
        let with_blank = [(1, "a".to_string()), (2, String::new()), (3, "b".into())];
        assert_eq!(
            Selection::all(&with_blank[..])
                .unwrap()
                .text(&with_blank[..]),
            "a\n\nb"
        );
    }

    #[test]
    fn the_selection_stays_on_its_lines_as_lines_come_and_go() {
        let selection = Selection::new(Caret::new(12, 5), Caret::new(14, 4));
        let before = lines(10..20);
        assert_eq!(selection.text(&before[..]), "12\nline 13\nline");
        // New lines at the end, and old ones trimmed from the start: the same text.
        let moved = lines(11..40);
        assert_eq!(selection.text(&moved[..]), "12\nline 13\nline");
        // Its first line trimmed: it starts at the next line's start.
        let trimmed = lines(13..40);
        assert_eq!(selection.text(&trimmed[..]), "line 13\nline");
        // All of it trimmed: nothing.
        let gone = lines(15..40);
        assert_eq!(selection.text(&gone[..]), "");
        // A filter hiding a line in the middle leaves it out.
        let filtered: Vec<_> = before.iter().filter(|(id, _)| *id != 13).cloned().collect();
        assert_eq!(selection.text(&filtered[..]), "12\nline");
        // Offsets past a line's end (or inside a character) are clamped.
        let short = [(12, "é".to_string()), (14, "x".to_string())];
        assert_eq!(selection.text(&short[..]), "\nx");
        let inside = Selection::new(Caret::new(1, 1), Caret::new(1, 9));
        assert_eq!(inside.text(&[(1, "éa")][..]), "éa");
    }

    #[test]
    fn words_and_lines_from_a_press_stay_whole_while_dragging() {
        let word = |line, text: &str, offset| {
            let range = Unit::Word.around(text, offset);
            (Caret::new(line, range.start), Caret::new(line, range.end))
        };
        // A double-click in "hello world" on line 3 selects "world"...
        let press = word(3, "hello world", 8);
        assert_eq!(press, (Caret::new(3, 6), Caret::new(3, 11)));
        // ...dragged forward to "two" on line 5, it takes all of both.
        let selection = extend(press, word(5, "one two", 5));
        assert_eq!(
            selection,
            Selection::new(Caret::new(3, 6), Caret::new(5, 7))
        );
        // Dragged back to "hello", it keeps "world" too.
        let selection = extend(press, word(3, "hello world", 1));
        assert_eq!(
            selection,
            Selection::new(Caret::new(3, 11), Caret::new(3, 0))
        );
        // A plain press and drag selects from place to place.
        let at = Caret::new(4, 2);
        assert_eq!(
            extend((at, at), (Caret::new(2, 1), Caret::new(2, 1))),
            Selection::new(at, Caret::new(2, 1))
        );
        // A triple-click selects the text's line, between line breaks.
        assert_eq!(Unit::Line.around("first\nsecond", 8), 6..12);
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod ui_tests {
    use gpui::{
        Context, IntoElement, ListAlignment, ListState, Modifiers, MouseButton, ParentElement,
        Render, Styled, TestAppContext, VisualTestContext, Window, div, list, point, prelude::*,
        px,
    };

    use super::{CopySelection, LogLine, LogSelection, SelectAll, log_view};

    const LINE: f32 = 20.;

    /// A log of `n` lines, 400 × 100 px: five lines show at a time.
    struct Log {
        selection: LogSelection,
        lines: Vec<(u64, String)>,
        list: ListState,
    }

    impl Render for Log {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let state = self.list.clone();
            div().size_full().child(
                div().w(px(400.)).h(px(100.)).child(
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
                    .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                        this.selection.select_all(&this.lines[..]);
                        cx.notify();
                    }))
                    .on_action(cx.listener(
                        |this, _: &CopySelection, _, cx| {
                            this.selection.copy(&this.lines[..], cx);
                        },
                    )),
                ),
            )
        }
    }

    fn open(cx: &mut TestAppContext, n: u64) -> (gpui::Entity<Log>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        let (log, cx) = cx.add_window_view(|_, cx| Log {
            selection: LogSelection::new(cx),
            // Ids that don't start at 0, with gaps: only their order counts.
            lines: (0..n)
                .map(|i| (100 + i * 3, format!("line {i} text")))
                .collect(),
            list: ListState::new(n as usize, ListAlignment::Top, px(100.)),
        });
        cx.run_until_parked();
        (log, cx)
    }

    /// The middle of line `n` (from the top of the view), `x` from its left.
    fn at(x: f32, n: usize) -> gpui::Point<gpui::Pixels> {
        point(px(x), px(LINE * n as f32 + LINE / 2.))
    }

    fn copied(cx: &mut VisualTestContext) -> Option<String> {
        cx.simulate_keystrokes("secondary-c");
        cx.read_from_clipboard().and_then(|item| item.text())
    }

    #[gpui::test]
    fn a_drag_selects_across_lines_and_copies(cx: &mut TestAppContext) {
        let (log, cx) = open(cx, 10);
        let none = Modifiers::default();
        // From the start of line 1 to past the end of line 3.
        cx.simulate_mouse_down(at(1., 1), MouseButton::Left, none);
        cx.simulate_mouse_move(at(200., 2), MouseButton::Left, none);
        cx.simulate_mouse_move(at(390., 3), MouseButton::Left, none);
        cx.simulate_mouse_up(at(390., 3), MouseButton::Left, none);
        assert_eq!(
            copied(cx).as_deref(),
            Some("line 1 text\nline 2 text\nline 3 text")
        );

        // A double-click selects a word; Shift-click extends from it.
        cx.simulate_click(at(1., 2), none);
        cx.simulate_event(gpui::MouseDownEvent {
            position: at(1., 2),
            button: MouseButton::Left,
            modifiers: none,
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_mouse_up(at(1., 2), MouseButton::Left, none);
        assert_eq!(copied(cx).as_deref(), Some("line"));
        let shift = Modifiers::shift();
        cx.simulate_mouse_down(at(390., 4), MouseButton::Left, shift);
        cx.simulate_mouse_up(at(390., 4), MouseButton::Left, shift);
        assert_eq!(
            copied(cx).as_deref(),
            Some("line 2 text\nline 3 text\nline 4 text")
        );

        // A click outside clears it.
        cx.simulate_click(point(px(600.), px(300.)), none);
        let selection = log.read_with(cx, |log, _| log.selection.selection());
        assert_eq!(selection, None);
    }

    #[gpui::test]
    fn a_drag_past_the_bottom_scrolls_and_keeps_selecting(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, 40);
        let none = Modifiers::default();
        cx.simulate_mouse_down(at(1., 0), MouseButton::Left, none);
        // Below the list: each frame scrolls it on.
        for _ in 0..30 {
            cx.simulate_mouse_move(point(px(390.), px(250.)), MouseButton::Left, none);
        }
        cx.simulate_mouse_up(point(px(390.), px(250.)), MouseButton::Left, none);
        let text = copied(cx).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], "line 0 text");
        assert!(lines.len() > 10, "selected past the first view: {lines:?}");
        assert_eq!(
            lines.last().copied(),
            Some(format!("line {} text", lines.len() - 1).as_str()),
            "whole lines, in order"
        );
    }

    #[gpui::test]
    fn select_all_copies_every_line(cx: &mut TestAppContext) {
        let (_, cx) = open(cx, 30);
        // Nothing selected, nothing focused: nothing copied.
        assert_eq!(copied(cx), None);
        // A click focuses the log.
        cx.simulate_click(at(1., 0), Modifiers::default());
        cx.simulate_keystrokes("secondary-a");
        let expected: Vec<String> = (0..30).map(|i| format!("line {i} text")).collect();
        assert_eq!(copied(cx), Some(expected.join("\n")));
    }
}
