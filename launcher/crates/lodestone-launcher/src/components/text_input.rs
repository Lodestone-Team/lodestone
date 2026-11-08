//! A single-line text field.
//!
//! The editing model, the IME support (`EntityInputHandler`) and the text element are
//! adapted from GPUI's `examples/input.rs` (Apache-2.0), trimmed to what the launcher
//! needs. Cursor movement steps over `char`s, not grapheme clusters, which avoids a
//! dependency on `unicode-segmentation`.

use std::ops::Range;

use gpui::{
    App, Bounds, BoxShadow, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div,
    fill, point, prelude::*, px, relative, size,
};

use crate::theme::{self, FOCUS_RING_WIDTH, colors, leading, radius, text, weights};

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
    ]
);

pub(super) fn init(cx: &mut App) {
    let context = Some("TextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("cmd-x", Cut, context),
    ]);
}

/// A single-line text field, styled like the dashboard's `InputField`.
pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    disabled: bool,
    /// Shows a mask instead of the text, and never copies it, for passwords.
    masked: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: SharedString::default(),
            disabled: false,
            masked: false,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
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

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), window, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
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

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let index = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(index, window, cx);
        } else {
            self.move_to(index, window, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), window, cx);
        }
    }

    fn move_to(&mut self, offset: usize, window: &Window, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
    }

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
        line.closest_index_for_x(position.x - bounds.left())
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
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify()
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
        self.selected_range = self.range_from_utf16(&range_utf16);
        self.selection_reversed = false;
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
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
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
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
        Some(self.offset_to_utf16(last_layout.closest_index_for_x(line_point.x)))
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
        div()
            .id("text-input")
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .flex()
            .items_center()
            .w_full()
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
                    this.bg(colors::GRAY_900)
                        .text_color(colors::GRAY_300)
                        .cursor(CursorStyle::IBeam)
                        .on_action(cx.listener(Self::backspace))
                        .on_action(cx.listener(Self::delete))
                        .on_action(cx.listener(Self::left))
                        .on_action(cx.listener(Self::right))
                        .on_action(cx.listener(Self::select_left))
                        .on_action(cx.listener(Self::select_right))
                        .on_action(cx.listener(Self::select_all))
                        .on_action(cx.listener(Self::home))
                        .on_action(cx.listener(Self::end))
                        .on_action(cx.listener(Self::paste))
                        .on_action(cx.listener(Self::cut))
                        .on_action(cx.listener(Self::copy))
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                        // A click anywhere else takes the focus away, as in a browser.
                        .on_mouse_down_out(cx.listener(|this, _, window, cx| {
                            if this.focus_handle.is_focused(window) {
                                window.blur(cx);
                            }
                        }))
                        .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                        .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                        .on_mouse_move(cx.listener(Self::on_mouse_move))
                }
            })
            // `focus-visible:ring-4 ring-blue-faded/50`
            .when(focused && !self.disabled, |this| {
                this.shadow(vec![BoxShadow {
                    color: colors::BLUE_FADED.opacity(50).into(),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: FOCUS_RING_WIDTH,
                    inset: false,
                }])
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
            (input.placeholder.clone(), colors::WHITE.opacity(30).into())
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

        let selected_range = input.selected_range.clone();
        let (selection, cursor) = if input.content.is_empty() || selected_range.is_empty() {
            let x = if input.content.is_empty() {
                px(0.)
            } else {
                line.x_for_index(input.cursor_offset())
            };
            let cursor = fill(
                Bounds::new(
                    point(bounds.left() + x, bounds.top()),
                    size(px(1.5), bounds.size.height),
                ),
                colors::GRAY_300,
            );
            (None, Some(cursor))
        } else {
            let selection = fill(
                Bounds::from_corners(
                    point(
                        bounds.left() + line.x_for_index(selected_range.start),
                        bounds.top(),
                    ),
                    point(
                        bounds.left() + line.x_for_index(selected_range.end),
                        bounds.bottom(),
                    ),
                ),
                colors::BLUE_FADED.opacity(50),
            );
            (Some(selection), None)
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
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
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        // Painting a shaped line only fails if a glyph cannot be rasterized; the
        // field then shows no text rather than taking the window down.
        line.paint(
            bounds.origin,
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

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
