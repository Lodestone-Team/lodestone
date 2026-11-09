//! A single-line text field.
//!
//! The editing model, the IME support (`EntityInputHandler`) and the text element are
//! adapted from GPUI's `examples/input.rs` (Apache-2.0), trimmed to what the launcher
//! needs. Moving the cursor one step goes over a `char`, not a grapheme cluster.
//! Moving, selecting and deleting by words goes by grapheme clusters, so a letter
//! and its combining marks go together.

use crate::skin::Skinned;
use crate::theme::px;
use launcher_core::ui_skin::SurfaceId;
use std::{
    ops::Range,
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation as _;

use gpui::{
    App, Bounds, BoxShadow, ClipboardItem, ContentMask, Context, CursorStyle, DispatchPhase,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine, SharedString, Style, Task, TextRun,
    UTF16Selection, UnderlineStyle, Window, actions, div, fill, point, prelude::*, relative, size,
};

use super::a11y::{Accessible, AccessibleAction, ActionData, Role};
use crate::{
    i18n::Text,
    theme::{self, colors, focus_ring_width, leading, radius, text, weights},
};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWordBackward,
        DeleteWordForward,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectToStart,
        SelectToEnd,
        Paste,
        Cut,
        Copy,
    ]
);

pub(super) fn init(cx: &mut App) {
    let context = Some("TextInput");
    let secondary = [
        ("secondary-a", Box::new(SelectAll) as Box<dyn gpui::Action>),
        ("secondary-v", Box::new(Paste)),
        ("secondary-c", Box::new(Copy)),
        ("secondary-x", Box::new(Cut)),
    ]
    .into_iter()
    .map(|(key, action)| {
        KeyBinding::load(
            key,
            action,
            Some(
                gpui::KeyBindingContextPredicate::parse("TextInput")
                    .unwrap()
                    .into(),
            ),
            false,
            None,
            cx,
        )
        .expect("valid text editing shortcut")
    })
    .collect::<Vec<_>>();
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("shift-home", SelectToStart, context),
        KeyBinding::new("shift-end", SelectToEnd, context),
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("cmd-shift-left", SelectToStart, context),
        KeyBinding::new("cmd-shift-right", SelectToEnd, context),
    ]);
    // Up and Down go to the start and the end, as in a single-line field in Firefox
    // and Chromium on Linux and in every macOS field. Not inside a menu or a list (an
    // `ArrowKeys` scope, such as the search field in the game version popover), where
    // they keep moving through its items as they did. Shift+Up and Shift+Down mean
    // nothing there, so they select to the ends in every field.
    let outside_lists = format!("TextInput && !{}", super::focus::ARROW_KEYS);
    cx.bind_keys([
        KeyBinding::new("up", Home, Some(&outside_lists)),
        KeyBinding::new("down", End, Some(&outside_lists)),
        KeyBinding::new("shift-up", SelectToStart, context),
        KeyBinding::new("shift-down", SelectToEnd, context),
    ]);
    // Moving and deleting by words: Option on macOS, Ctrl on Linux and Windows.
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("alt-left", WordLeft, context),
        KeyBinding::new("alt-right", WordRight, context),
        KeyBinding::new("alt-shift-left", SelectWordLeft, context),
        KeyBinding::new("alt-shift-right", SelectWordRight, context),
        KeyBinding::new("alt-backspace", DeleteWordBackward, context),
        KeyBinding::new("alt-delete", DeleteWordForward, context),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-left", WordLeft, context),
        KeyBinding::new("ctrl-right", WordRight, context),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, context),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, context),
        KeyBinding::new("ctrl-backspace", DeleteWordBackward, context),
        KeyBinding::new("ctrl-delete", DeleteWordForward, context),
        // The old IBM CUA keys, which Windows and the Linux toolkits still honour.
        // A Mac keyboard has no Insert key.
        KeyBinding::new("shift-insert", Paste, context),
        KeyBinding::new("ctrl-insert", Copy, context),
        KeyBinding::new("shift-delete", Cut, context),
    ]);
    cx.bind_keys(secondary);
}

/// A single-line text field, styled like the dashboard's `InputField`.
pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    /// Looked up when drawn, so it follows the language.
    placeholder: Text,
    /// The field's name for assistive technology and automation; the placeholder when
    /// empty.
    label: Text,
    /// What automation finds the field by; `text-input` when empty.
    automation_id: SharedString,
    disabled: bool,
    /// Shows a mask instead of the text, and never copies it, for passwords.
    masked: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// How far the text is scrolled to the left, when it is wider than the field: set
    /// as the field is drawn, so the cursor stays in view. Hit-testing goes by the
    /// last drawn one.
    scroll_x: Pixels,
    /// While the left button is held after a press in the field: what dragging selects by.
    dragging: Option<Drag>,
    /// Where the pointer was last while dragging.
    drag_position: Option<Point<Pixels>>,
    /// Scrolls the field while a drag holds the pointer past its left or right edge.
    autoscroll: Option<Task<()>>,
    history: TextHistory,
    /// Shown before the text.
    leading_icon: Option<crate::components::Icon>,
}

/// What a drag selects by, from how many clicks began it, as in GTK and browsers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Drag {
    /// A single press: from where it was to the pointer.
    Chars,
    /// A double-click: from the word it selected to the word under the pointer.
    Words(Range<usize>),
    /// A triple-click selected everything, and dragging keeps it so.
    All,
}

/// The field's own Undo and Redo: its text before each burst of typing or deleting.
/// Undo and Redo go to the focused field first, so they change its text, not a
/// setting (see `crate::undo`).
#[derive(Default)]
struct TextHistory {
    undo: Vec<(SharedString, Range<usize>)>,
    redo: Vec<(SharedString, Range<usize>)>,
    /// When the last edit was, and whether it inserted text: typing on within a moment
    /// is the same step.
    last: Option<(Instant, bool)>,
}

/// The cursor's width.
fn cursor_width() -> Pixels {
    px(1.5)
}

/// How close the cursor may come to an edge of the field before the text scrolls.
fn scroll_margin() -> Pixels {
    px(4.)
}

/// How often a drag past an edge of the field scrolls it on.
const AUTOSCROLL_TICK: Duration = Duration::from_millis(40);

/// How many pixels past an edge the pointer must go for each more `char` a tick
/// scrolls by: the farther out, the faster, as in native fields.
fn autoscroll_distance_per_char() -> Pixels {
    px(16.)
}

/// How many steps a field keeps.
const TEXT_HISTORY: usize = 100;
/// Edits closer together than this are one step.
const TEXT_STEP: Duration = Duration::from_millis(1000);

impl TextHistory {
    fn remember(&mut self, text: &SharedString, selection: &Range<usize>, inserting: bool) {
        let now = Instant::now();
        let same_step = self
            .last
            .is_some_and(|(at, kind)| kind == inserting && now.duration_since(at) < TEXT_STEP);
        if !same_step {
            self.undo.push((text.clone(), selection.clone()));
            if self.undo.len() > TEXT_HISTORY {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some((now, inserting));
    }
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: Text::default(),
            label: Text::default(),
            automation_id: SharedString::default(),
            disabled: false,
            masked: false,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            scroll_x: Pixels::ZERO,
            dragging: None,
            drag_position: None,
            autoscroll: None,
            history: TextHistory::default(),
            leading_icon: None,
        }
    }

    /// An icon at the start of the field, such as a magnifying glass for a search.
    pub fn with_leading_icon(mut self, icon: crate::components::Icon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// The text shown while the field is empty: a message (`InstancesMsg::SearchLog`),
    /// so it follows the language, or text as it is.
    pub fn with_placeholder(mut self, placeholder: impl Into<Text>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The field's name for assistive technology and automation, such as "Instance
    /// name". Without one, the placeholder names it.
    pub fn with_label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }

    /// What automation finds the field by, such as `instance-name`: stable across
    /// languages, so never the label.
    pub fn with_automation_id(mut self, id: impl Into<SharedString>) -> Self {
        self.automation_id = id.into();
        self
    }

    /// Replaces the whole text, as assistive technology or automation asks (UI
    /// Automation's `ValuePattern.SetValue`). Like typing it, it can be undone.
    fn set_value_from_accessibility(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let text = text.replace(['\r', '\n'], " ");
        self.replace(0..self.content.len(), &text);
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    /// Changes the placeholder of a field already shown.
    pub fn set_placeholder(&mut self, placeholder: impl Into<Text>, cx: &mut Context<Self>) {
        let placeholder = placeholder.into();
        if placeholder.get() != self.placeholder.get() {
            self.placeholder = placeholder;
            cx.notify();
        }
    }

    pub fn with_text(mut self, text: impl Into<SharedString>) -> Self {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Turns a field already shown on or off, such as a console's command line while
    /// the server is stopped.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            cx.notify();
        }
    }

    /// A password field: the text is shown masked and cannot be copied or cut.
    pub fn with_masked(mut self) -> Self {
        self.masked = true;
        self
    }

    /// The text as drawn: the content, or for a masked field a mask with the same
    /// length in bytes, so every offset into the content is one into the mask too.
    fn display_content(&self) -> SharedString {
        if !self.masked {
            return self.content.clone();
        }
        self.content
            .chars()
            .map(|c| match c.len_utf8() {
                3 => "\u{2022}",
                2 => "\u{00b7}",
                1 => "*",
                _ => "****",
            })
            .collect::<String>()
            .into()
    }

    pub fn text(&self) -> &SharedString {
        &self.content
    }

    /// Replaces the text and puts the cursor at its end.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.history = TextHistory::default();
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.marked_range = None;
        cx.notify();
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    /// To the start of the word before the cursor, or with a selection to its start,
    /// as Left does. GTK collapses a selection the same way; Chromium moves a word
    /// on from it instead.
    fn word_left(&mut self, _: &WordLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.word_start_before(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.start, window, cx)
        }
    }

    /// To the end of the word after the cursor (where GTK stops, not the start of the
    /// next word), or with a selection to its end.
    fn word_right(&mut self, _: &WordRight, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.word_end_after(self.cursor_offset()), window, cx);
        } else {
            self.move_to(self.selected_range.end, window, cx)
        }
    }

    fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.word_start_before(self.cursor_offset()), window, cx);
    }

    fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.word_end_after(self.cursor_offset()), window, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), window, cx);
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), window, cx);
    }

    fn select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
        self.select_to(self.content.len(), window, cx)
    }

    fn home(&mut self, _: &Home, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, window, cx);
    }

    fn end(&mut self, _: &End, window: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), window, cx);
    }

    fn select_to_start(&mut self, _: &SelectToStart, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, window, cx);
    }

    fn select_to_end(&mut self, _: &SelectToEnd, window: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), window, cx);
    }

    /// Undo while the field has the focus: its text before the last edit.
    fn undo(&mut self, _: &crate::Undo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((text, selection)) = self.history.undo.pop() {
            let now = (self.content.clone(), self.selected_range.clone());
            self.history.redo.push(now);
            self.restore(text, selection, window, cx);
        }
    }

    fn redo(&mut self, _: &crate::Redo, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((text, selection)) = self.history.redo.pop() {
            let now = (self.content.clone(), self.selected_range.clone());
            self.history.undo.push(now);
            self.restore(text, selection, window, cx);
        }
    }

    fn restore(
        &mut self,
        text: SharedString,
        selection: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.content = text;
        self.selected_range =
            selection.start.min(self.content.len())..selection.end.min(self.content.len());
        self.selection_reversed = false;
        self.marked_range = None;
        self.history.last = None;
        // Not offered as PRIMARY: the selection put back is the one the edit replaced,
        // often just the text a Backspace deleted, not one the user has just made.
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(self.previous_boundary(self.cursor_offset()), window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_to(self.next_boundary(self.cursor_offset()), window, cx)
    }

    /// Deletes the selection, or from the cursor to `offset` when nothing is selected.
    /// The range goes straight to the edit instead of through `select_to`: selecting
    /// the text first would offer it as PRIMARY, so every Backspace would take PRIMARY
    /// from the app that had it and leave the deleted text in it. GTK and Qt fields
    /// leave PRIMARY alone when deleting.
    fn delete_to(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        if self.selected_range.is_empty() {
            let cursor = self.cursor_offset();
            self.selected_range = cursor.min(offset)..cursor.max(offset);
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    /// Where the word before `offset` starts. A password field is one word: its words
    /// would show where the spaces are.
    fn word_start_before(&self, offset: usize) -> usize {
        if self.masked {
            0
        } else {
            previous_word_start(&self.content, offset)
        }
    }

    /// Where the word after `offset` ends.
    fn word_end_after(&self, offset: usize) -> usize {
        if self.masked {
            self.content.len()
        } else {
            next_word_end(&self.content, offset)
        }
    }

    /// The word a double-click on the `char` at `offset` selects.
    fn word_at(&self, offset: usize) -> Range<usize> {
        if self.masked {
            0..self.content.len()
        } else {
            word_range_at(&self.content, offset)
        }
    }

    /// Deletes the selection, or back to the start of the word before the cursor.
    fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_to(self.word_start_before(self.cursor_offset()), window, cx)
    }

    /// Deletes the selection, or on to the end of the word after the cursor.
    fn delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_to(self.word_end_after(self.cursor_offset()), window, cx)
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.masked {
            self.copy(&Copy, window, cx);
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    /// A press puts the cursor there (with Shift, selects to there), a double-click
    /// selects the word under the pointer and a triple-click everything, as in GTK
    /// and browsers.
    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag_position = Some(event.position);
        self.dragging = Some(match event.click_count {
            0 | 1 => {
                let index = self.index_for_mouse_position(event.position);
                if event.modifiers.shift {
                    self.select_to(index, window, cx);
                } else {
                    self.move_to(index, window, cx)
                }
                Drag::Chars
            }
            2 => {
                let word = self.word_at(self.char_index_for_mouse_position(event.position));
                self.select_range(word.clone(), false, window, cx);
                Drag::Words(word)
            }
            _ => {
                self.select_range(0..self.content.len(), false, window, cx);
                Drag::All
            }
        });
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.dragging = None;
        self.drag_position = None;
        self.autoscroll = None;
    }

    /// Any move of the pointer in the window while a drag that began in the field
    /// goes on, even outside it.
    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.dragging, Some(Drag::Chars | Drag::Words(_))) {
            return;
        }
        self.drag_position = Some(event.position);
        self.drag_to(
            self.index_for_mouse_position(event.position),
            self.char_index_for_mouse_position(event.position),
            window,
            cx,
        );
        if self.autoscroll.is_none() && self.past_edge(event.position) != Pixels::ZERO {
            self.autoscroll = Some(cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(AUTOSCROLL_TICK).await;
                    let more = this
                        .update_in(cx, |this, window, cx| {
                            let more = this.autoscroll_step(window, cx);
                            if !more {
                                this.autoscroll = None;
                            }
                            more
                        })
                        .unwrap_or(false);
                    if !more {
                        break;
                    }
                }
            }));
        }
    }

    /// How far `position` is past the text's left edge (negative) or right edge
    /// (positive), or zero inside them.
    fn past_edge(&self, position: Point<Pixels>) -> Pixels {
        let Some(bounds) = self.last_bounds else {
            return Pixels::ZERO;
        };
        if position.x < bounds.left() {
            position.x - bounds.left()
        } else if position.x > bounds.right() {
            position.x - bounds.right()
        } else {
            Pixels::ZERO
        }
    }

    /// One tick of scrolling while a drag holds the pointer past an edge: the
    /// selection grows by a `char` or more that way, and drawing scrolls the cursor
    /// into view. False once the drag ended or the pointer came back.
    fn autoscroll_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(position) = self.drag_position else {
            return false;
        };
        if !matches!(self.dragging, Some(Drag::Chars | Drag::Words(_))) {
            return false;
        }
        let past = self.past_edge(position);
        if past == Pixels::ZERO {
            return false;
        }
        let chars = 1 + (past.abs() / autoscroll_distance_per_char()) as usize;
        let mut index = self.cursor_offset();
        for _ in 0..chars {
            index = if past > Pixels::ZERO {
                self.next_boundary(index)
            } else {
                self.previous_boundary(index)
            };
        }
        self.drag_to(index, index, window, cx);
        true
    }

    /// Moves the dragged end of the selection to `index` (a gap), or by words to the
    /// word of the `char` at `char_index`.
    fn drag_to(
        &mut self,
        index: usize,
        char_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (range, reversed) = match &self.dragging {
            None | Some(Drag::All) => return,
            Some(Drag::Chars) => {
                let anchor = if self.selection_reversed {
                    self.selected_range.end
                } else {
                    self.selected_range.start
                };
                (anchor.min(index)..anchor.max(index), index < anchor)
            }
            // Whole words, from the one double-clicked to the one under the pointer.
            Some(Drag::Words(anchor)) => {
                let word = self.word_at(char_index);
                (
                    anchor.start.min(word.start)..anchor.end.max(word.end),
                    word.start < anchor.start,
                )
            }
        };
        // The pointer moves far more often than it crosses a character, and each new
        // selection is offered as PRIMARY.
        if range != self.selected_range || reversed != self.selection_reversed {
            self.select_range(range, reversed, window, cx);
        }
    }

    /// A middle-click pastes the PRIMARY selection where it lands, the X11 and
    /// Wayland convention that GTK and Qt fields follow.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn paste_primary(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self.index_for_mouse_position(event.position);
        self.move_to(index, window, cx);
        if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.select_range(offset..offset, false, window, cx)
    }

    /// Selects `range`, with the cursor at its start if `reversed`.
    fn select_range(
        &mut self,
        range: Range<usize>,
        reversed: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = range;
        self.selection_reversed = reversed;
        self.selection_changed(window, cx)
    }

    fn selection_changed(&self, window: &Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
        self.offer_primary(cx);
    }

    /// Offers the selected text as the PRIMARY selection, for a middle-click to paste
    /// here or in another app, as every X11 and Wayland toolkit does. Never a
    /// password: any app on the desktop can read PRIMARY.
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn offer_primary(&self, cx: &mut App) {
        if !self.masked && !self.selected_range.is_empty() {
            cx.write_to_primary(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    /// PRIMARY is an X11 and Wayland convention.
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    fn offer_primary(&self, _: &mut App) {}

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if self.content.is_empty() || position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(self.line_x(position, bounds))
    }

    /// Where `position` falls along the shaped line: inside the field's text area,
    /// and past the part scrolled out of view.
    fn line_x(&self, position: Point<Pixels>, bounds: &Bounds<Pixels>) -> Pixels {
        position.x.clamp(bounds.left(), bounds.right()) - bounds.left() + self.scroll_x
    }

    /// The `char` under the pointer, rather than the gap nearest to it: the one a
    /// double-click selects the word of. Past the end, the end.
    fn char_index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if self.content.is_empty() || position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.index_for_x(self.line_x(position, bounds))
            .unwrap_or(self.content.len())
    }

    fn select_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.selection_changed(window, cx)
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf16_to_utf8(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        utf8_to_utf16(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        previous_char_boundary(&self.content, offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        next_char_boundary(&self.content, offset)
    }

    fn replace(&mut self, range: Range<usize>, new_text: &str) {
        if range.is_empty() && new_text.is_empty() {
            return;
        }
        self.history
            .remember(&self.content, &self.selected_range, !new_text.is_empty());
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
    }
}

/// The byte offset of the `char` before `offset`, or 0 at the start.
fn previous_char_boundary(text: &str, offset: usize) -> usize {
    text[..offset]
        .char_indices()
        .next_back()
        .map_or(0, |(index, _)| index)
}

/// The byte offset after the `char` at `offset`, or the end of `text`.
fn next_char_boundary(text: &str, offset: usize) -> usize {
    text[offset..]
        .chars()
        .next()
        .map_or(text.len(), |ch| offset + ch.len_utf8())
}

/// What a word is made of, for deleting by words: a run of letters, digits and `_`,
/// or a run of other symbols. Spaces separate words and belong to none.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Space,
    Word,
    Symbol,
}

/// The kind of a grapheme cluster, from its first `char`: a letter's combining marks
/// (an accent, a virama, a Thai tone mark) go with the letter, never on their own.
fn cluster_kind(cluster: &str) -> CharKind {
    match cluster.chars().next() {
        Some(ch) if ch.is_whitespace() => CharKind::Space,
        Some(ch) if ch.is_alphanumeric() || ch == '_' => CharKind::Word,
        _ => CharKind::Symbol,
    }
}

/// Where Ctrl+Backspace (Option+Backspace on macOS) deletes back to from `offset`: over
/// any spaces, then over the word or the run of symbols before them, as GTK, Qt and
/// browsers do. "foo bar|" → "foo |", "foo.bar|" → "foo.|", "foo  |" → "|".
fn previous_word_start(text: &str, offset: usize) -> usize {
    let mut clusters = text[..offset].grapheme_indices(true).rev().peekable();
    while clusters
        .next_if(|&(_, cluster)| cluster_kind(cluster) == CharKind::Space)
        .is_some()
    {}
    let Some(&(_, first)) = clusters.peek() else {
        return 0;
    };
    let kind = cluster_kind(first);
    let mut start = offset;
    while let Some((index, _)) = clusters.next_if(|&(_, cluster)| cluster_kind(cluster) == kind) {
        start = index;
    }
    start
}

/// Where Ctrl+Delete (Option+Delete on macOS) deletes on to from `offset`: over any
/// spaces, then over the word or the run of symbols after them. "|foo bar" → "| bar".
fn next_word_end(text: &str, offset: usize) -> usize {
    let mut clusters = text[offset..].grapheme_indices(true).peekable();
    while clusters
        .next_if(|&(_, cluster)| cluster_kind(cluster) == CharKind::Space)
        .is_some()
    {}
    let Some(&(_, first)) = clusters.peek() else {
        return text.len();
    };
    let kind = cluster_kind(first);
    while clusters
        .next_if(|&(_, cluster)| cluster_kind(cluster) == kind)
        .is_some()
    {}
    clusters
        .peek()
        .map_or(text.len(), |&(index, _)| offset + index)
}

/// What a double-click on the `char` at `offset` selects: the run of grapheme clusters
/// of its kind, so a word, a run of symbols, or the spaces between two words, as GTK
/// does. At the end of the text, the run before it. "foo b|ar" → "foo [bar]",
/// "foo | bar" → "foo[   ]bar".
fn word_range_at(text: &str, offset: usize) -> Range<usize> {
    let clusters = text.grapheme_indices(true).collect::<Vec<_>>();
    let Some(at) = clusters.iter().rposition(|&(start, _)| start <= offset) else {
        return offset..offset;
    };
    let kind = cluster_kind(clusters[at].1);
    let same = |&&(_, cluster): &&(usize, &str)| cluster_kind(cluster) == kind;
    let first = at - clusters[..at].iter().rev().take_while(same).count();
    let last = at + clusters[at + 1..].iter().take_while(same).count();
    clusters[first].0..clusters[last].0 + clusters[last].1.len()
}

/// Converts a UTF-16 offset (as the platform's input methods use) to a byte offset.
fn utf16_to_utf8(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

/// Converts a byte offset to a UTF-16 offset.
fn utf8_to_utf16(text: &str, offset: usize) -> usize {
    text[..offset].encode_utf16().count()
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.range_from_utf16(&range_utf16);
        self.select_range(range, false, window, cx);
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.replace(range.clone(), new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range = None;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.replace(range.clone(), new_text);
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        let insertion_start_utf16 = self.offset_to_utf16(range.start);
        let inserted_length_utf16 = new_text.encode_utf16().count();
        self.selected_range = new_selected_range_utf16
            .map(|selected| {
                self.range_from_utf16(
                    &(insertion_start_utf16 + selected.start.min(inserted_length_utf16)
                        ..insertion_start_utf16 + selected.end.min(inserted_length_utf16)),
                )
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.text != self.display_content() {
            return None;
        }
        let range = self.range_from_utf16(&range_utf16);
        let left = bounds.left() - self.scroll_x;
        Some(Bounds::from_corners(
            point(left + last_layout.x_for_index(range.start), bounds.top()),
            point(left + last_layout.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        if last_layout.text != self.display_content() {
            return None;
        }
        Some(self.offset_to_utf16(last_layout.closest_index_for_x(line_point.x + self.scroll_x)))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let label = Some(self.label.get())
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| self.placeholder.get());
        let automation_id = Some(self.automation_id.clone())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| "text-input".into());
        div()
            .id("text-input")
            .map(|this| {
                if self.masked {
                    // A password field: never its text.
                    this.accessible(Role::PasswordInput, automation_id, label)
                } else {
                    this.accessible(Role::TextInput, automation_id, label)
                        .aria_value(self.content.clone())
                }
            })
            .aria_placeholder(self.placeholder.get())
            .when(!self.disabled, |this| {
                let input = cx.entity().downgrade();
                this.on_a11y_action(AccessibleAction::SetValue, move |data, window, cx| {
                    if let Some(ActionData::Value(text)) = data {
                        input
                            .update(cx, |input, cx| {
                                input.set_value_from_accessibility(text, window, cx)
                            })
                            .ok();
                    }
                })
            })
            .key_context("TextInput")
            // A tab stop unless disabled.
            .track_focus(&self.focus_handle.clone().tab_stop(!self.disabled))
            .flex()
            .items_center()
            .w_full()
            // Text longer than the field (a pasted share code) stays inside it.
            .overflow_hidden()
            // `input-shape`: rounded py-2 px-2.5
            .py_2()
            .px(px(10.))
            .rounded(radius::DEFAULT)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(theme::line_height(text::MEDIUM, leading::SNUG))
            .font_weight(weights::MEDIUM.into())
            .map(|this| {
                if self.disabled {
                    this.bg(colors::GRAY_800)
                        .text_color(colors::WHITE.opacity(50))
                } else {
                    let this = this
                        .bg(colors::GRAY_900)
                        .text_color(crate::theme::ink::text())
                        .surface(SurfaceId::Input)
                        .cursor(CursorStyle::IBeam)
                        .on_action(cx.listener(Self::backspace))
                        .on_action(cx.listener(Self::delete))
                        .on_action(cx.listener(Self::delete_word_backward))
                        .on_action(cx.listener(Self::delete_word_forward))
                        .on_action(cx.listener(Self::left))
                        .on_action(cx.listener(Self::right))
                        .on_action(cx.listener(Self::word_left))
                        .on_action(cx.listener(Self::word_right))
                        .on_action(cx.listener(Self::select_left))
                        .on_action(cx.listener(Self::select_right))
                        .on_action(cx.listener(Self::select_word_left))
                        .on_action(cx.listener(Self::select_word_right))
                        .on_action(cx.listener(Self::select_all))
                        .on_action(cx.listener(Self::home))
                        .on_action(cx.listener(Self::end))
                        .on_action(cx.listener(Self::select_to_start))
                        .on_action(cx.listener(Self::select_to_end))
                        .on_action(cx.listener(Self::paste))
                        .on_action(cx.listener(Self::cut))
                        .on_action(cx.listener(Self::copy))
                        .on_action(cx.listener(Self::undo))
                        .on_action(cx.listener(Self::redo))
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                        // A click anywhere else takes the focus away, as in a browser.
                        .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                            if this.focus_handle.is_focused(window) {
                                window.blur(cx);
                            }
                        }))
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up));
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    let this =
                        this.on_mouse_down(MouseButton::Middle, cx.listener(Self::paste_primary));
                    this
                }
            })
            // `focus-visible:ring-4 ring-blue-faded/50`
            .when(focused && !self.disabled, |this| {
                this.shadow(vec![BoxShadow {
                    color: crate::theme::accent::faded().opacity(50).into(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: focus_ring_width(),
                    inset: false,
                }])
            })
            .when_some(self.leading_icon, |this, icon| {
                this.gap_2().child(icon.render(px(13.), colors::GRAY_500))
            })
            .child(TextElement { input: cx.entity() })
    }
}

/// Paints the text, the selection and the cursor, and registers the input handler.
struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    /// How far the text is scrolled to the left.
    scroll_x: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let style = window.text_style();

        // `placeholder:text-white/30`
        let (display_text, text_color): (SharedString, Hsla) = if input.content.is_empty() {
            (input.placeholder.get(), colors::WHITE.opacity(30).into())
        } else {
            (input.display_content(), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.marked_range.as_ref() {
            Some(marked) if !input.content.is_empty() => [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            _ => vec![run],
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        // Text wider than the field scrolls to keep the cursor (the moving end of a
        // selection) in view. A field without the focus shows the text's start, as
        // NSTextField does.
        let scroll_x = if input.content.is_empty() || !input.focus_handle.is_focused(window) {
            Pixels::ZERO
        } else {
            scroll_to_show(
                input.scroll_x,
                line.x_for_index(input.cursor_offset()),
                line.width,
                bounds.size.width,
            )
        };
        let left = bounds.left() - scroll_x;

        let selected_range = input.selected_range.clone();
        let (selection, cursor) = if input.content.is_empty() || selected_range.is_empty() {
            let x = if input.content.is_empty() {
                px(0.)
            } else {
                line.x_for_index(input.cursor_offset())
            };
            let cursor = fill(
                Bounds::new(
                    point(left + x, bounds.top()),
                    size(cursor_width(), bounds.size.height),
                ),
                colors::GRAY_300,
            );
            (None, Some(cursor))
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(left + line.x_for_index(selected_range.start), bounds.top()),
                    point(left + line.x_for_index(selected_range.end), bounds.bottom()),
                ),
                crate::theme::accent::faded().opacity(50),
            );
            (Some(selection), None)
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
            scroll_x,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        // A drag that began in the field follows the pointer anywhere in the window.
        let input = self.input.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && input.read(cx).dragging.is_some() {
                input.update(cx, |input, cx| input.on_mouse_move(event, window, cx));
            }
        });

        let Some(line) = prepaint.line.take() else {
            return;
        };
        let scroll_x = prepaint.scroll_x;
        // Scrolled text stays inside the field's padding.
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(selection) = prepaint.selection.take() {
                window.paint_quad(selection)
            }
            // Painting a shaped line only fails if a glyph cannot be rasterized; the
            // field then shows no text rather than taking the window down.
            line.paint(
                point(bounds.left() - scroll_x, bounds.top()),
                window.line_height(),
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();

            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_x = scroll_x;
        });
    }
}

/// How far to scroll a line `line_width` wide, in a field `field_width` wide, from
/// `scroll_x`, to show the cursor at `cursor_x`: as little as keeps it a margin from
/// either edge, and never past either end of the text.
fn scroll_to_show(
    scroll_x: Pixels,
    cursor_x: Pixels,
    line_width: Pixels,
    field_width: Pixels,
) -> Pixels {
    let margin = scroll_margin().min(field_width * 0.25);
    let mut scroll_x = scroll_x;
    if cursor_x + cursor_width() + margin > scroll_x + field_width {
        scroll_x = cursor_x + cursor_width() + margin - field_width;
    }
    if cursor_x - margin < scroll_x {
        scroll_x = cursor_x - margin;
    }
    let max = (line_width + cursor_width() - field_width).max(Pixels::ZERO);
    scroll_x.clamp(Pixels::ZERO, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_shortcuts_follow_the_platform() {
        use gpui::{Keystroke, OperatingSystem};

        for key in ["a", "v", "c", "x"] {
            for (os, modifier) in [
                (OperatingSystem::Mac, "cmd"),
                (OperatingSystem::Windows, "ctrl"),
                (OperatingSystem::Linux, "ctrl"),
            ] {
                assert_eq!(
                    Keystroke::parse_with_os(&format!("secondary-{key}"), os).unwrap(),
                    Keystroke::parse_with_os(&format!("{modifier}-{key}"), os).unwrap(),
                );
            }
        }
    }

    #[test]
    fn boundaries_step_over_whole_chars() {
        let text = "a🙂é";
        assert_eq!(next_char_boundary(text, 0), 1);
        assert_eq!(next_char_boundary(text, 1), 5);
        assert_eq!(next_char_boundary(text, 5), 7);
        assert_eq!(next_char_boundary(text, 7), 7);
        assert_eq!(previous_char_boundary(text, 7), 5);
        assert_eq!(previous_char_boundary(text, 5), 1);
        assert_eq!(previous_char_boundary(text, 0), 0);
    }

    #[test]
    fn typing_is_one_step_and_deleting_another() {
        let mut history = TextHistory::default();
        let text = |s: &str| SharedString::from(s.to_string());
        history.remember(&text(""), &(0..0), true);
        history.remember(&text("a"), &(1..1), true);
        history.remember(&text("ab"), &(2..2), true);
        assert_eq!(history.undo.len(), 1);
        history.remember(&text("abc"), &(3..3), false);
        assert_eq!(history.undo.len(), 2);
        assert_eq!(history.undo[1].0.as_ref(), "abc");
        history.redo.push((text("x"), 0..0));
        history.remember(&text("ab"), &(2..2), false);
        assert!(
            history.redo.is_empty(),
            "a new edit forgets what was undone"
        );
    }

    /// Deletes back a word from the `|` in `text`, as Ctrl+Backspace does.
    fn word_backward(text: &str) -> String {
        let cursor = text.find('|').unwrap();
        let text = text.replacen('|', "", 1);
        let start = previous_word_start(&text, cursor);
        format!("{}|{}", &text[..start], &text[cursor..])
    }

    /// Deletes on a word from the `|` in `text`, as Ctrl+Delete does.
    fn word_forward(text: &str) -> String {
        let cursor = text.find('|').unwrap();
        let text = text.replacen('|', "", 1);
        let end = next_word_end(&text, cursor);
        format!("{}|{}", &text[..cursor], &text[end..])
    }

    #[test]
    fn deleting_back_a_word() {
        for (before, after) in [
            ("foo bar|", "foo |"),
            ("foo bar |", "foo |"),
            ("foo bar  \t|", "foo |"),
            ("foo|", "|"),
            ("|", "|"),
            ("   |", "|"),
            ("|foo", "|foo"),
            ("foo ba|r", "foo |r"),
            // Symbols are a word of their own, as in GTK, Qt and browsers.
            ("foo.bar|", "foo.|"),
            ("foo.|", "foo|"),
            ("foo...|", "foo|"),
            ("-Xmx4G -XX:+UseG1GC|", "-Xmx4G -XX:+|"),
            ("/home/me/.minecraft|", "/home/me/.|"),
            ("snake_case|", "|"),
            ("v1.21.1|", "v1.21.|"),
            // Letters beyond ASCII, and characters of several bytes.
            ("héllo wörld|", "héllo |"),
            ("日本語 テキスト|", "日本語 |"),
            ("a 🙂🙂|", "a |"),
            ("a🙂|", "a|"),
            // A letter's combining marks belong to its word: decomposed é, a Thai
            // tone mark, a Devanagari virama, a Tamil pulli.
            ("cafe\u{301}|", "|"),
            ("x cafe\u{301}|", "x |"),
            ("ไม่|", "|"),
            ("हिन्दी|", "|"),
            ("தமிழ்|", "|"),
            // An emoji joined into one picture is one symbol.
            ("a 👩\u{200d}💻|", "a |"),
        ] {
            assert_eq!(word_backward(before), after, "from {before:?}");
        }
    }

    #[test]
    fn deleting_on_a_word() {
        for (before, after) in [
            ("|foo bar", "| bar"),
            ("| foo bar", "| bar"),
            ("foo|", "foo|"),
            ("|", "|"),
            ("|   ", "|"),
            ("f|oo bar", "f| bar"),
            ("|foo.bar", "|.bar"),
            ("|.bar", "|bar"),
            ("|-Xmx4G -Xms1G", "|Xmx4G -Xms1G"),
            ("|héllo wörld", "| wörld"),
            ("|🙂🙂 a", "| a"),
            ("|cafe\u{301} x", "| x"),
            ("|हिन्दी x", "| x"),
        ] {
            assert_eq!(word_forward(before), after, "from {before:?}");
        }
    }

    /// What a double-click on the `char` after the `|` in `text` selects, in brackets.
    fn double_click(text: &str) -> String {
        let at = text.find('|').unwrap();
        let text = text.replacen('|', "", 1);
        let word = word_range_at(&text, at);
        format!(
            "{}[{}]{}",
            &text[..word.start],
            &text[word.clone()],
            &text[word.end..]
        )
    }

    #[test]
    fn a_double_click_selects_a_word() {
        for (click, selected) in [
            ("foo b|ar", "foo [bar]"),
            ("|foo bar", "[foo] bar"),
            ("fo|o bar", "[foo] bar"),
            // The end of the text is the end of its last word.
            ("foo bar|", "foo [bar]"),
            // Spaces select the spaces between two words, as in GTK.
            ("foo| bar", "foo[ ]bar"),
            ("foo  |  bar", "foo[    ]bar"),
            // Symbols are words of their own, as Ctrl+Backspace sees them.
            ("-Xmx4G -XX:+Use|G1GC", "-Xmx4G -XX:+[UseG1GC]"),
            ("-Xmx4G -XX|:+UseG1GC", "-Xmx4G -XX[:+]UseG1GC"),
            ("/home/me/.mine|craft", "/home/me/.[minecraft]"),
            ("/home/me|/.minecraft", "/home/me[/.]minecraft"),
            ("snake_|case", "[snake_case]"),
            ("héllo w|örld", "héllo [wörld]"),
            ("日本|語 テキスト", "[日本語] テキスト"),
            ("x cafe\u{301}|", "x [cafe\u{301}]"),
            ("a 👩\u{200d}💻|", "a [👩\u{200d}💻]"),
            ("|", "[]"),
        ] {
            assert_eq!(double_click(click), selected, "at {click:?}");
        }
        // A click on a combining mark selects the word it belongs to.
        let text = "x cafe\u{301}";
        assert_eq!(
            word_range_at(text, text.len() - "\u{301}".len()),
            2..text.len()
        );
    }

    #[test]
    fn word_boundaries_fall_between_chars() {
        let text = "é🙂 漢字.x cafe\u{301} हिन्दी";
        for offset in (0..=text.len()).filter(|&o| text.is_char_boundary(o)) {
            assert!(text.is_char_boundary(previous_word_start(text, offset)));
            assert!(text.is_char_boundary(next_word_end(text, offset)));
            assert!(previous_word_start(text, offset) <= offset);
            assert!(next_word_end(text, offset) >= offset);
        }
    }

    #[test]
    fn utf16_offsets_round_trip() {
        // 🙂 is two UTF-16 units and four bytes; 漢 is one unit and three bytes.
        let text = "e🙂漢";
        assert_eq!(utf8_to_utf16(text, 0), 0);
        assert_eq!(utf8_to_utf16(text, 1), 1);
        assert_eq!(utf8_to_utf16(text, 5), 3);
        assert_eq!(utf8_to_utf16(text, 8), 4);
        for byte_offset in [0, 1, 5, 8] {
            assert_eq!(
                utf16_to_utf8(text, utf8_to_utf16(text, byte_offset)),
                byte_offset
            );
        }
        // Offsets past the end clamp to the end.
        assert_eq!(utf16_to_utf8(text, 99), text.len());
    }
}

/// Shortcuts typed and clicks made into a field in a test window. GPUI's test platform
/// comes with its `test-support` feature, which the `screenshot` feature turns on:
/// `cargo test -p lodestone-launcher --features screenshot`.
#[cfg(all(test, feature = "screenshot"))]
mod keystroke_tests {
    use gpui::{
        AppContext as _, Context, Entity, Focusable as _, IntoElement, Modifiers, MouseButton,
        MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
        TestAppContext, VisualTestContext, Window, point,
    };

    use super::{TextInput, next_char_boundary};
    use crate::components::{Button, FocusScope};

    /// The word modifier of the platform the tests run on.
    const WORD: &str = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };

    fn field<'a>(
        text: &str,
        masked: bool,
        cx: &'a mut TestAppContext,
    ) -> (Entity<TextInput>, &'a mut VisualTestContext) {
        field_with(
            text,
            cx,
            move |input| {
                if masked { input.with_masked() } else { input }
            },
        )
    }

    fn field_with<'a>(
        text: &str,
        cx: &'a mut TestAppContext,
        build: impl FnOnce(TextInput) -> TextInput + 'static,
    ) -> (Entity<TextInput>, &'a mut VisualTestContext) {
        cx.update(crate::components::init);
        let text = text.to_string();
        let (input, cx) =
            cx.add_window_view(move |_, cx| build(TextInput::new(cx).with_text(text)));
        cx.update(|window, cx| {
            let handle = input.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        });
        cx.run_until_parked();
        (input, cx)
    }

    fn text(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
        input.read_with(cx, |input, _| input.text().to_string())
    }

    #[test]
    fn word_backspace_deletes_back_a_word() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("-Xmx4G -XX:+UseG1GC", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-backspace"));
        assert_eq!(text(&input, cx), "-Xmx4G -XX:+");
        cx.simulate_keystrokes(&format!("{WORD}-backspace"));
        assert_eq!(text(&input, cx), "-Xmx4G -XX");
        cx.simulate_keystrokes(&format!("{WORD}-backspace {WORD}-backspace"));
        assert_eq!(text(&input, cx), "-Xmx4G ");
        // Plain Backspace still deletes one character.
        cx.simulate_keystrokes("backspace");
        assert_eq!(text(&input, cx), "-Xmx4G");
    }

    #[test]
    fn word_delete_deletes_on_a_word() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("foo bar", false, &mut cx);
        cx.simulate_keystrokes("home");
        cx.simulate_keystrokes(&format!("{WORD}-delete"));
        assert_eq!(text(&input, cx), " bar");
        cx.simulate_keystrokes(&format!("{WORD}-delete"));
        assert_eq!(text(&input, cx), "");
    }

    #[test]
    fn word_backspace_deletes_the_selection_first() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two three", false, &mut cx);
        cx.simulate_keystrokes("shift-left shift-left");
        cx.simulate_keystrokes(&format!("{WORD}-backspace"));
        assert_eq!(text(&input, cx), "one two thr");
    }

    #[test]
    fn a_password_is_one_word() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("correct horse", true, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-backspace"));
        assert_eq!(text(&input, cx), "");
    }

    #[test]
    fn a_word_delete_undoes() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("foo bar", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-backspace"));
        assert_eq!(text(&input, cx), "foo ");
        input.update_in(cx, |input, window, cx| input.undo(&crate::Undo, window, cx));
        assert_eq!(text(&input, cx), "foo bar");
    }

    /// The field's text with the selection in brackets, or a `|` at the cursor.
    fn shown(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
        input.read_with(cx, |input, _| {
            let range = input.selected_range.clone();
            let text = &input.content;
            if range.is_empty() {
                format!("{}|{}", &text[..range.start], &text[range.end..])
            } else {
                format!(
                    "{}[{}]{}",
                    &text[..range.start],
                    &text[range.clone()],
                    &text[range.end..]
                )
            }
        })
    }

    /// The gap before the `char` at `offset`, in the window, or with `inside` a little
    /// into the `char` (still nearer that gap than the next).
    fn at(
        input: &Entity<TextInput>,
        offset: usize,
        inside: bool,
        cx: &mut VisualTestContext,
    ) -> Point<Pixels> {
        input.read_with(cx, |input, _| {
            let bounds = input.last_bounds.expect("the field was drawn");
            let line = input.last_layout.as_ref().expect("the field was drawn");
            let mut x = line.x_for_index(offset);
            if inside {
                let next = line.x_for_index(next_char_boundary(&input.content, offset));
                x += (next - x) * 0.4;
            }
            point(bounds.left() - input.scroll_x + x, bounds.center().y)
        })
    }

    fn press(
        cx: &mut VisualTestContext,
        position: Point<Pixels>,
        button: MouseButton,
        clicks: usize,
    ) {
        cx.simulate_event(MouseDownEvent {
            button,
            position,
            modifiers: Modifiers::none(),
            click_count: clicks,
            first_mouse: false,
        });
    }

    fn release(
        cx: &mut VisualTestContext,
        position: Point<Pixels>,
        button: MouseButton,
        clicks: usize,
    ) {
        cx.simulate_event(MouseUpEvent {
            button,
            position,
            modifiers: Modifiers::none(),
            click_count: clicks,
        });
    }

    /// Clicks the left button `clicks` times at `position`, as the platform reports it.
    fn click(cx: &mut VisualTestContext, position: Point<Pixels>, clicks: usize) {
        for count in 1..=clicks {
            press(cx, position, MouseButton::Left, count);
            release(cx, position, MouseButton::Left, count);
        }
    }

    #[test]
    fn word_keys_move_by_words() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("-Xmx4G -XX:+UseG1GC", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-left"));
        assert_eq!(shown(&input, cx), "-Xmx4G -XX:+|UseG1GC");
        cx.simulate_keystrokes(&format!("{WORD}-left {WORD}-left"));
        assert_eq!(shown(&input, cx), "-Xmx4G -|XX:+UseG1GC");
        cx.simulate_keystrokes(&format!("{WORD}-left"));
        assert_eq!(shown(&input, cx), "-Xmx4G |-XX:+UseG1GC");
        // On to the end of the next word, over the spaces before it.
        cx.simulate_keystrokes(&format!("home {WORD}-right"));
        assert_eq!(shown(&input, cx), "-|Xmx4G -XX:+UseG1GC");
        cx.simulate_keystrokes(&format!("{WORD}-right"));
        assert_eq!(shown(&input, cx), "-Xmx4G| -XX:+UseG1GC");
        cx.simulate_keystrokes(&format!("{WORD}-right"));
        assert_eq!(shown(&input, cx), "-Xmx4G -|XX:+UseG1GC");
        cx.simulate_keystrokes(&format!("end {WORD}-right"));
        assert_eq!(shown(&input, cx), "-Xmx4G -XX:+UseG1GC|");
        cx.simulate_keystrokes(&format!("home {WORD}-left"));
        assert_eq!(shown(&input, cx), "|-Xmx4G -XX:+UseG1GC");
    }

    #[test]
    fn shift_with_the_word_keys_selects_by_words() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two three", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-shift-left {WORD}-shift-left"));
        assert_eq!(shown(&input, cx), "one [two three]");
        // The cursor end moves; the other stays.
        cx.simulate_keystrokes(&format!("{WORD}-shift-right"));
        assert_eq!(shown(&input, cx), "one two[ three]");
        cx.simulate_keystrokes(&format!("home {WORD}-shift-right {WORD}-shift-right"));
        assert_eq!(shown(&input, cx), "[one two] three");
        cx.simulate_keystrokes("shift-left");
        assert_eq!(shown(&input, cx), "[one tw]o three");
    }

    #[test]
    fn a_word_key_without_shift_ends_the_selection() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two three", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-left shift-left shift-left shift-left"));
        assert_eq!(shown(&input, cx), "one t[wo ]three");
        // At the selection's start or end, as Left and Right go and as GTK does.
        cx.simulate_keystrokes(&format!("{WORD}-left"));
        assert_eq!(shown(&input, cx), "one t|wo three");
        cx.simulate_keystrokes(&format!("shift-right shift-right {WORD}-right"));
        assert_eq!(shown(&input, cx), "one two| three");
    }

    #[test]
    fn a_password_moves_as_one_word() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("correct horse", true, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-left"));
        assert_eq!(shown(&input, cx), "|correct horse");
        cx.simulate_keystrokes(&format!("{WORD}-right"));
        assert_eq!(shown(&input, cx), "correct horse|");
    }

    #[test]
    fn shift_home_and_end_select_to_the_ends() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two", false, &mut cx);
        cx.simulate_keystrokes("left left left shift-home");
        assert_eq!(shown(&input, cx), "[one ]two");
        cx.simulate_keystrokes("shift-end");
        assert_eq!(shown(&input, cx), "one [two]");
        // Cmd+Shift+Left and Right are the same on a Mac.
        cx.simulate_keystrokes("cmd-shift-left");
        assert_eq!(shown(&input, cx), "[one ]two");
        cx.simulate_keystrokes("cmd-shift-right");
        assert_eq!(shown(&input, cx), "one [two]");
        // Plain Home and End still move.
        cx.simulate_keystrokes("home");
        assert_eq!(shown(&input, cx), "|one two");
    }

    #[test]
    fn up_and_down_go_to_the_ends() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two", false, &mut cx);
        cx.simulate_keystrokes("left left left up");
        assert_eq!(shown(&input, cx), "|one two");
        cx.simulate_keystrokes("down");
        assert_eq!(shown(&input, cx), "one two|");
        // A selection collapses to the end they go to.
        cx.simulate_keystrokes("shift-left shift-left down");
        assert_eq!(shown(&input, cx), "one two|");
        cx.simulate_keystrokes("shift-left shift-left up");
        assert_eq!(shown(&input, cx), "|one two");
    }

    #[test]
    fn shift_up_and_down_select_to_the_ends() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two", false, &mut cx);
        cx.simulate_keystrokes("left left left shift-up");
        assert_eq!(shown(&input, cx), "[one ]two");
        // The cursor end moves; the other stays.
        cx.simulate_keystrokes("shift-down");
        assert_eq!(shown(&input, cx), "one [two]");
        cx.simulate_keystrokes("up");
        assert_eq!(shown(&input, cx), "|one two");
    }

    /// A field in a menu, before a button, as the search field in the game version
    /// popover sits above its filters and list.
    struct InMenu {
        input: Entity<TextInput>,
    }

    impl Render for InMenu {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            FocusScope::new("menu")
                .arrow_keys()
                .child(self.input.clone())
                .child(Button::new("after", "After"))
        }
    }

    #[test]
    fn up_and_down_in_a_menu_still_move_through_it() {
        let mut cx = TestAppContext::single();
        cx.update(crate::components::init);
        let (view, cx) = cx.add_window_view(|_, cx| InMenu {
            input: cx.new(|cx| TextInput::new(cx).with_text("one two")),
        });
        cx.run_until_parked();
        let input = view.read_with(cx, |view, _| view.input.clone());
        let handle = input.read_with(cx, |input, cx| input.focus_handle(cx));
        cx.update(|window, cx| window.focus(&handle, cx));
        cx.run_until_parked();
        let focused = |cx: &mut VisualTestContext| cx.update(|window, _| handle.is_focused(window));
        // Left and Right still move the cursor.
        cx.simulate_keystrokes("left left");
        assert_eq!(shown(&input, cx), "one t|wo");
        cx.simulate_keystrokes("down");
        assert!(!focused(cx), "Down goes on to the button");
        cx.simulate_keystrokes("up");
        assert!(focused(cx), "Up comes back to the field");
        assert_eq!(shown(&input, cx), "one t|wo", "the cursor stays");
        cx.simulate_keystrokes("shift-up");
        assert!(focused(cx));
        assert_eq!(shown(&input, cx), "[one t]wo");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_insert_keys_copy_cut_and_paste() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two", false, &mut cx);
        cx.simulate_keystrokes(&format!("{WORD}-shift-left ctrl-insert"));
        let clipboard =
            |cx: &mut VisualTestContext| cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(clipboard(cx).as_deref(), Some("two"));
        assert_eq!(text(&input, cx), "one two");
        cx.simulate_keystrokes("shift-home shift-delete");
        assert_eq!(clipboard(cx).as_deref(), Some("one two"));
        assert_eq!(text(&input, cx), "");
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("a\nb".into()));
        cx.simulate_keystrokes("shift-insert");
        assert_eq!(shown(&input, cx), "a b|");
    }

    /// ⌘C (Ctrl+C) copies the field's selection, not the selected rich text: the
    /// text fields' bindings come after `RichText`'s context-free one.
    #[test]
    fn the_copy_shortcut_copies_the_selection() {
        let mut cx = TestAppContext::single();
        let (_input, cx) = field("one two", false, &mut cx);
        cx.simulate_keystrokes("secondary-a secondary-c");
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("one two")
        );
    }

    #[test]
    fn a_double_click_selects_the_word_and_a_triple_click_all() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("-Xmx4G -XX:+UseG1GC", false, &mut cx);
        let on_use = at(&input, 13, true, cx);
        click(cx, on_use, 1);
        assert_eq!(shown(&input, cx), "-Xmx4G -XX:+U|seG1GC");
        click(cx, on_use, 2);
        assert_eq!(shown(&input, cx), "-Xmx4G -XX:+[UseG1GC]");
        click(cx, on_use, 3);
        assert_eq!(shown(&input, cx), "[-Xmx4G -XX:+UseG1GC]");
        // On a space, the spaces.
        let on_space = at(&input, 6, true, cx);
        click(cx, on_space, 2);
        assert_eq!(shown(&input, cx), "-Xmx4G[ ]-XX:+UseG1GC");
        // On a symbol, its run of symbols.
        let on_colon = at(&input, 10, true, cx);
        click(cx, on_colon, 2);
        assert_eq!(shown(&input, cx), "-Xmx4G -XX[:+]UseG1GC");
    }

    #[test]
    fn dragging_after_a_double_click_selects_whole_words() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("one two three four", false, &mut cx);
        let on_two = at(&input, 5, true, cx);
        click(cx, on_two, 1);
        press(cx, on_two, MouseButton::Left, 2);
        let drag_to = |offset: usize, cx: &mut VisualTestContext| {
            let position = at(&input, offset, true, cx);
            cx.simulate_event(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Modifiers::none(),
            });
        };
        drag_to(10, cx);
        assert_eq!(shown(&input, cx), "one [two three] four");
        drag_to(1, cx);
        assert_eq!(shown(&input, cx), "[one two] three four");
        // The cursor is at the start, where the pointer went.
        cx.simulate_keystrokes("shift-right");
        assert_eq!(shown(&input, cx), "o[ne two] three four");
    }

    #[test]
    fn a_password_double_clicks_as_one_word() {
        let mut cx = TestAppContext::single();
        let (input, cx) = field("correct horse", true, &mut cx);
        let position = at(&input, 2, true, cx);
        click(cx, position, 2);
        assert_eq!(shown(&input, cx), "[correct horse]");
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    mod primary {
        use super::*;
        use gpui::ClipboardItem;

        fn primary(cx: &mut VisualTestContext) -> Option<String> {
            cx.update(|_, cx| cx.read_from_primary().and_then(|item| item.text()))
        }

        fn set_primary(text: &str, cx: &mut VisualTestContext) {
            let item = ClipboardItem::new_string(text.into());
            cx.update(|_, cx| cx.write_to_primary(item));
        }

        #[test]
        fn a_selection_is_offered_as_primary() {
            let mut cx = TestAppContext::single();
            let (input, cx) = field("one two", false, &mut cx);
            // Moving the cursor offers nothing.
            cx.simulate_keystrokes("left");
            assert_eq!(primary(cx), None);
            cx.simulate_keystrokes("shift-home");
            assert_eq!(primary(cx).as_deref(), Some("one tw"));
            let on_one = at(&input, 1, true, cx);
            click(cx, on_one, 2);
            assert_eq!(primary(cx).as_deref(), Some("one"));
            // Ending the selection leaves PRIMARY as it was, as in GTK.
            cx.simulate_keystrokes("end");
            assert_eq!(primary(cx).as_deref(), Some("one"));
        }

        #[test]
        fn a_password_is_never_offered_as_primary() {
            let mut cx = TestAppContext::single();
            let (input, cx) = field("correct horse", true, &mut cx);
            set_primary("elsewhere", cx);
            cx.simulate_keystrokes("shift-home");
            let position = at(&input, 2, true, cx);
            click(cx, position, 2);
            click(cx, position, 3);
            cx.simulate_keystrokes("secondary-a");
            assert_eq!(shown(&input, cx), "[correct horse]");
            assert_eq!(primary(cx).as_deref(), Some("elsewhere"));
        }

        #[test]
        fn a_middle_click_pastes_primary_where_it_lands() {
            let mut cx = TestAppContext::single();
            let (input, cx) = field("one three", false, &mut cx);
            set_primary("two\nand a half ", cx);
            let before_three = at(&input, 4, false, cx);
            press(cx, before_three, MouseButton::Middle, 1);
            release(cx, before_three, MouseButton::Middle, 1);
            assert_eq!(shown(&input, cx), "one two and a half |three");
        }

        #[test]
        fn a_disabled_field_takes_no_middle_click() {
            let mut cx = TestAppContext::single();
            let (input, cx) = field_with("one", &mut cx, |input| input.with_disabled(true));
            set_primary("two", cx);
            let position = at(&input, 1, false, cx);
            press(cx, position, MouseButton::Middle, 1);
            // The text alone would stay "one" even if the click got through, as a
            // disabled field takes no edits; the cursor would still move.
            assert_eq!(shown(&input, cx), "one|");
        }

        #[test]
        fn deleting_leaves_primary_alone() {
            let mut cx = TestAppContext::single();
            let (input, cx) = field("one two three four", false, &mut cx);
            set_primary("elsewhere", cx);
            cx.simulate_keystrokes("backspace");
            cx.simulate_keystrokes(&format!("{WORD}-backspace"));
            cx.simulate_keystrokes("home delete");
            cx.simulate_keystrokes(&format!("{WORD}-delete"));
            assert_eq!(shown(&input, cx), "| two three ");
            input.update_in(cx, |input, window, cx| input.undo(&crate::Undo, window, cx));
            assert_eq!(text(&input, cx), "one two three four");
            assert_eq!(primary(cx).as_deref(), Some("elsewhere"));
        }
    }

    /// A field 120 px wide: too narrow for a folder path.
    struct Narrow {
        input: Entity<TextInput>,
    }

    impl Render for Narrow {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            use gpui::Styled as _;
            gpui::div()
                .w(gpui::px(120.))
                .child(self.input.clone())
                .child(Button::new("after", "After"))
        }
    }

    const PATH: &str = "/Users/peter/dev/lodestone/launcher/target/debug/lodestone_core";

    fn narrow_field<'a>(
        text: &str,
        masked: bool,
        cx: &'a mut TestAppContext,
    ) -> (Entity<TextInput>, &'a mut VisualTestContext) {
        cx.update(crate::components::init);
        let text = text.to_string();
        let (view, cx) = cx.add_window_view(move |_, cx| Narrow {
            input: cx.new(|cx| {
                let input = TextInput::new(cx).with_text(text);
                if masked { input.with_masked() } else { input }
            }),
        });
        cx.run_until_parked();
        let input = view.read_with(cx, |view, _| view.input.clone());
        let handle = input.read_with(cx, |input, cx| input.focus_handle(cx));
        cx.update(|window, cx| window.focus(&handle, cx));
        cx.run_until_parked();
        (input, cx)
    }

    /// How far the text is scrolled, and the most it can be.
    fn scroll(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> (Pixels, Pixels) {
        input.read_with(cx, |input, _| {
            let bounds = input.last_bounds.expect("the field was drawn");
            let line = input.last_layout.as_ref().expect("the field was drawn");
            let max = (line.width + super::cursor_width() - bounds.size.width).max(Pixels::ZERO);
            (input.scroll_x, max)
        })
    }

    /// Whether the cursor is drawn inside the field's text area.
    fn cursor_shown(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> bool {
        input.read_with(cx, |input, _| {
            let bounds = input.last_bounds.expect("the field was drawn");
            let line = input.last_layout.as_ref().expect("the field was drawn");
            let x = line.x_for_index(input.cursor_offset()) - input.scroll_x;
            x >= Pixels::ZERO && x + super::cursor_width() <= bounds.size.width
        })
    }

    #[test]
    fn typing_past_the_width_scrolls_to_the_cursor() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field("", false, &mut cx);
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO);
        cx.simulate_input("/Users/peter/dev");
        assert!(cursor_shown(&input, cx));
        cx.simulate_input("/lodestone/launcher/target");
        assert_eq!(
            text(&input, cx),
            "/Users/peter/dev/lodestone/launcher/target"
        );
        let (scroll_x, max) = scroll(&input, cx);
        assert!(scroll_x > Pixels::ZERO, "the text scrolled");
        assert_eq!(scroll_x, max, "to the end, with the cursor at the edge");
        assert!(cursor_shown(&input, cx));
    }

    #[test]
    fn the_keys_keep_the_cursor_in_view() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field(PATH, false, &mut cx);
        assert_eq!(scroll(&input, cx).0, scroll(&input, cx).1, "at the end");
        cx.simulate_keystrokes("home");
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO, "Home scrolls back");
        assert!(cursor_shown(&input, cx));
        cx.simulate_keystrokes("end");
        assert_eq!(scroll(&input, cx).0, scroll(&input, cx).1, "End scrolls on");
        // Left only scrolls once the cursor would leave the field.
        let at_end = scroll(&input, cx).0;
        cx.simulate_keystrokes("left");
        assert_eq!(scroll(&input, cx).0, at_end);
        for _ in 0..30 {
            cx.simulate_keystrokes("left");
            assert!(cursor_shown(&input, cx));
        }
        assert!(scroll(&input, cx).0 < at_end);
        for _ in 0..4 {
            cx.simulate_keystrokes(&format!("{WORD}-left"));
            assert!(cursor_shown(&input, cx));
        }
        cx.simulate_keystrokes("home shift-end");
        assert!(cursor_shown(&input, cx), "the selection's moving end");
        assert_eq!(scroll(&input, cx).0, scroll(&input, cx).1);
        cx.simulate_keystrokes("home");
        let select_all = if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        };
        cx.simulate_keystrokes(select_all);
        assert_eq!(shown(&input, cx), format!("[{PATH}]"));
        assert!(cursor_shown(&input, cx));
    }

    #[test]
    fn a_click_after_scrolling_lands_on_the_char_under_it() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field(PATH, false, &mut cx);
        assert!(scroll(&input, cx).0 > Pixels::ZERO);
        // Three chars before the end, drawn near the field's right edge.
        let offset = PATH.len() - 3;
        let position = at(&input, offset, true, cx);
        click(cx, position, 1);
        assert_eq!(
            shown(&input, cx),
            format!("{}|{}", &PATH[..offset], &PATH[offset..])
        );
        // A double-click selects the word under the pointer, not one at the start.
        let word = PATH.rfind('/').unwrap() + 1;
        let position = at(&input, word + 2, true, cx);
        click(cx, position, 2);
        assert_eq!(
            shown(&input, cx),
            format!("{}[lodestone_core]", &PATH[..word])
        );
        // The field's left edge is the first char shown, not the text's start.
        let left = input.read_with(cx, |input, _| input.last_bounds.unwrap().left());
        let y = at(&input, 0, false, cx).y;
        click(cx, point(left, y), 1);
        let index = input.read_with(cx, |input, _| input.cursor_offset());
        assert!(index > PATH.len() / 2, "the cursor went to {index}");
        assert!(cursor_shown(&input, cx));
    }

    #[test]
    fn deleting_back_scrolls_back() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field(PATH, false, &mut cx);
        let before = scroll(&input, cx).0;
        for _ in 0..10 {
            cx.simulate_keystrokes("backspace");
            let (scroll_x, max) = scroll(&input, cx);
            assert!(scroll_x <= max, "never past the end of the text");
            assert!(cursor_shown(&input, cx));
        }
        assert!(scroll(&input, cx).0 < before);
        cx.simulate_keystrokes(&format!("{WORD}-backspace {WORD}-backspace"));
        assert_eq!(scroll(&input, cx).0, scroll(&input, cx).1);
        // Short enough to fit: no scroll at all.
        input.update_in(cx, |input, _, cx| input.set_text("/Users", cx));
        cx.run_until_parked();
        assert_eq!(scroll(&input, cx), (Pixels::ZERO, Pixels::ZERO));
    }

    #[test]
    fn a_field_without_the_focus_shows_the_start() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field(PATH, false, &mut cx);
        assert!(scroll(&input, cx).0 > Pixels::ZERO);
        cx.update(|window, cx| window.blur(cx));
        cx.run_until_parked();
        let handle = input.read_with(cx, |input, _| input.focus_handle.clone());
        assert!(!cx.update(|window, _| handle.is_focused(window)));
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO);
        // A click into it lands where it was drawn.
        let position = at(&input, 5, true, cx);
        click(cx, position, 1);
        assert_eq!(shown(&input, cx), format!("{}|{}", &PATH[..5], &PATH[5..]));
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO);
    }

    #[test]
    fn a_password_scrolls_too() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field("correct horse battery staple", true, &mut cx);
        assert!(scroll(&input, cx).0 > Pixels::ZERO);
        assert!(cursor_shown(&input, cx));
        cx.simulate_keystrokes("home");
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO);
    }

    #[test]
    fn dragging_past_an_edge_scrolls_on() {
        let mut cx = TestAppContext::single();
        let (input, cx) = narrow_field(PATH, false, &mut cx);
        cx.simulate_keystrokes("home");
        let start = at(&input, 1, false, cx);
        press(cx, start, MouseButton::Left, 1);
        let right = input.read_with(cx, |input, _| input.last_bounds.unwrap().right());
        // Held past the right edge, the selection grows tick by tick to the end.
        let outside = point(right + gpui::px(40.), start.y);
        cx.simulate_event(MouseMoveEvent {
            position: outside,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
        let first = input.read_with(cx, |input, _| input.selected_range.clone());
        assert_eq!(first.start, 1);
        for _ in 0..200 {
            cx.executor().advance_clock(super::AUTOSCROLL_TICK);
            cx.run_until_parked();
            assert!(cursor_shown(&input, cx));
        }
        assert_eq!(shown(&input, cx), format!("/[{}]", &PATH[1..]));
        assert_eq!(scroll(&input, cx).0, scroll(&input, cx).1);
        // And back past the left edge, to the start.
        let left = input.read_with(cx, |input, _| input.last_bounds.unwrap().left());
        let outside = point(left - gpui::px(40.), start.y);
        cx.simulate_event(MouseMoveEvent {
            position: outside,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
        for _ in 0..200 {
            cx.executor().advance_clock(super::AUTOSCROLL_TICK);
            cx.run_until_parked();
        }
        assert_eq!(shown(&input, cx), format!("[/]{}", &PATH[1..]));
        assert_eq!(scroll(&input, cx).0, Pixels::ZERO);
        // Released, it stops.
        release(cx, outside, MouseButton::Left, 1);
        assert!(input.read_with(cx, |input, _| input.autoscroll.is_none()));
    }
}
