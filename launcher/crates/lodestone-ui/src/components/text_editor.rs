//! A multi-line text editor for files: line numbers, a monospace font (JetBrains
//! Mono), selection with the pointer and the keyboard, copy, cut and paste, undo and
//! redo, auto-indent, Tab and Shift+Tab, and simple colouring for the configuration
//! formats servers use (`.properties`, TOML, YAML, JSON). It draws only the lines in
//! view, so a file of many megabytes scrolls and edits as quickly as a short one.
//!
//! The editing model and the IME support follow [`TextInput`](super::TextInput),
//! extended to lines. Lines don't wrap; a long line scrolls sideways. Tabs show as
//! four columns. Line endings are kept as the file had them (`\n` or `\r\n`).
//!
//! The editor tells its view about edits with [`TextEditorEvent::Changed`]; Undo and
//! Redo (`crate::Undo`, `crate::Redo`) change its text while it has the focus, as a
//! text field's do (see `crate::undo`).

use std::{
    ops::Range,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, ScrollEvent, ShapedLine, SharedString, Style, TextRun,
    UTF16Selection, UnderlineStyle, Window, actions, div, fill, point, prelude::*, relative, size,
};
use unicode_segmentation::UnicodeSegmentation as _;

use super::a11y::{Accessible, Role};
use crate::{
    i18n::Text,
    theme::{self, Color, colors, fonts, px, text},
};

actions!(
    text_editor,
    [
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        LineStart,
        LineEnd,
        SelectToLineStart,
        SelectToLineEnd,
        DocumentStart,
        DocumentEnd,
        SelectToDocumentStart,
        SelectToDocumentEnd,
        PageUp,
        PageDown,
        SelectPageUp,
        SelectPageDown,
        Backspace,
        Delete,
        DeleteWordBackward,
        DeleteWordForward,
        DeleteToLineStart,
        Newline,
        Indent,
        Outdent,
        SelectAll,
        Copy,
        Cut,
        Paste,
    ]
);

const CONTEXT: &str = "TextEditor";

pub(super) fn init(cx: &mut App) {
    let context = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("home", LineStart, context),
        KeyBinding::new("end", LineEnd, context),
        KeyBinding::new("shift-home", SelectToLineStart, context),
        KeyBinding::new("shift-end", SelectToLineEnd, context),
        KeyBinding::new("pageup", PageUp, context),
        KeyBinding::new("pagedown", PageDown, context),
        KeyBinding::new("shift-pageup", SelectPageUp, context),
        KeyBinding::new("shift-pagedown", SelectPageDown, context),
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("shift-backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("enter", Newline, context),
        KeyBinding::new("shift-enter", Newline, context),
        KeyBinding::new("tab", Indent, context),
        KeyBinding::new("shift-tab", Outdent, context),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("alt-left", WordLeft, context),
        KeyBinding::new("alt-right", WordRight, context),
        KeyBinding::new("alt-shift-left", SelectWordLeft, context),
        KeyBinding::new("alt-shift-right", SelectWordRight, context),
        KeyBinding::new("alt-backspace", DeleteWordBackward, context),
        KeyBinding::new("alt-delete", DeleteWordForward, context),
        KeyBinding::new("cmd-backspace", DeleteToLineStart, context),
        KeyBinding::new("cmd-left", LineStart, context),
        KeyBinding::new("cmd-right", LineEnd, context),
        KeyBinding::new("cmd-shift-left", SelectToLineStart, context),
        KeyBinding::new("cmd-shift-right", SelectToLineEnd, context),
        KeyBinding::new("cmd-up", DocumentStart, context),
        KeyBinding::new("cmd-down", DocumentEnd, context),
        KeyBinding::new("cmd-shift-up", SelectToDocumentStart, context),
        KeyBinding::new("cmd-shift-down", SelectToDocumentEnd, context),
        KeyBinding::new("cmd-z", crate::Undo, context),
        KeyBinding::new("cmd-shift-z", crate::Redo, context),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-left", WordLeft, context),
        KeyBinding::new("ctrl-right", WordRight, context),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, context),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, context),
        KeyBinding::new("ctrl-backspace", DeleteWordBackward, context),
        KeyBinding::new("ctrl-delete", DeleteWordForward, context),
        KeyBinding::new("ctrl-home", DocumentStart, context),
        KeyBinding::new("ctrl-end", DocumentEnd, context),
        KeyBinding::new("ctrl-shift-home", SelectToDocumentStart, context),
        KeyBinding::new("ctrl-shift-end", SelectToDocumentEnd, context),
        KeyBinding::new("ctrl-z", crate::Undo, context),
        KeyBinding::new("ctrl-shift-z", crate::Redo, context),
        KeyBinding::new("ctrl-y", crate::Redo, context),
        KeyBinding::new("shift-insert", Paste, context),
        KeyBinding::new("ctrl-insert", Copy, context),
        KeyBinding::new("shift-delete", Cut, context),
    ]);
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
                gpui::KeyBindingContextPredicate::parse(CONTEXT)
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
    cx.bind_keys(secondary);
}

/// A place in the text: a line and a byte offset in it (at a `char` boundary).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// What a file's text is, for its colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    Plain,
    /// `.properties`, `.cfg`, `.conf`, `.ini`: `key=value`, `# comments`, `[sections]`.
    Properties,
    Toml,
    Yaml,
    Json,
}

impl Language {
    /// The language of a file, from its name, as the dashboard's editor picks one
    /// (`.lodestone_config` is JSON).
    pub fn of(file_name: &str) -> Self {
        let lower = file_name.to_ascii_lowercase();
        if lower.ends_with(".lodestone_config") {
            return Language::Json;
        }
        match lower.rsplit_once('.').map(|(_, ext)| ext) {
            Some("properties" | "cfg" | "conf" | "config" | "ini" | "env") => Language::Properties,
            Some("toml") => Language::Toml,
            Some("yml" | "yaml") => Language::Yaml,
            Some("json" | "mcmeta" | "json5" | "jsonc") => Language::Json,
            _ => Language::Plain,
        }
    }
}

/// A kind of token, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Comment,
    Key,
    Section,
    String,
    Number,
    Keyword,
}

impl Token {
    /// Its colour: Visual Studio Code's, dark or light (Monaco's, in the dashboard).
    fn color(self) -> Color {
        if crate::theme::palette::is_light() {
            return match self {
                Token::Comment => Color::rgb(0x008000),
                Token::Key => Color::rgb(0x0451A5),
                Token::Section => Color::rgb(0x267F99),
                Token::String => Color::rgb(0xA31515),
                Token::Number => Color::rgb(0x098658),
                Token::Keyword => Color::rgb(0x0000FF),
            };
        }
        match self {
            Token::Comment => Color::rgb(0x6A9955),
            Token::Key => Color::rgb(0x9CDCFE),
            Token::Section => Color::rgb(0x4EC9B0),
            Token::String => Color::rgb(0xCE9178),
            Token::Number => Color::rgb(0xB5CEA8),
            Token::Keyword => Color::rgb(0x569CD6),
        }
    }
}

/// The coloured tokens of one line, as byte ranges in order. Each line is read on
/// its own: a string spread over lines is coloured line by line.
pub fn tokens(language: Language, line: &str) -> Vec<(Range<usize>, Token)> {
    let mut out = Vec::new();
    let trimmed_start = line.len() - line.trim_start().len();
    let rest = &line[trimmed_start..];
    match language {
        Language::Plain => {}
        Language::Properties | Language::Toml => {
            let comment = if language == Language::Toml {
                "#"
            } else if rest.starts_with(';') {
                ";"
            } else if rest.starts_with('!') {
                "!"
            } else {
                "#"
            };
            if rest.starts_with(comment) {
                out.push((trimmed_start..line.len(), Token::Comment));
            } else if rest.starts_with('[') {
                let end = rest.find(']').map_or(line.len(), |e| trimmed_start + e + 1);
                out.push((trimmed_start..end, Token::Section));
                comment_after(line, end, &mut out);
            } else if let Some(eq) = rest.find(['=', ':']).filter(|_| !rest.is_empty()) {
                let key_end = trimmed_start + rest[..eq].trim_end().len();
                if key_end > trimmed_start {
                    out.push((trimmed_start..key_end, Token::Key));
                }
                let value_start = trimmed_start + eq + 1;
                if language == Language::Toml {
                    values(line, value_start, &mut out);
                } else {
                    let value = &line[value_start..];
                    let start = value_start + (value.len() - value.trim_start().len());
                    if start < line.len() {
                        let token = scalar(line[start..].trim_end());
                        out.push((start..line.len(), token));
                    }
                }
            }
        }
        Language::Yaml => {
            if rest.starts_with('#') {
                out.push((trimmed_start..line.len(), Token::Comment));
                return out;
            }
            let mut start = trimmed_start;
            if rest.starts_with("- ") {
                start += 2;
            }
            if let Some(colon) = key_colon(&line[start..]) {
                out.push((start..start + colon, Token::Key));
                values(line, start + colon + 1, &mut out);
            } else {
                values(line, start, &mut out);
            }
        }
        Language::Json => {
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        let end = string_end(line, i);
                        let after = line[end..].trim_start();
                        let token = if after.starts_with(':') {
                            Token::Key
                        } else {
                            Token::String
                        };
                        out.push((i..end, token));
                        i = end;
                    }
                    b'/' if bytes.get(i + 1) == Some(&b'/') => {
                        out.push((i..line.len(), Token::Comment));
                        break;
                    }
                    b'-' | b'0'..=b'9' => {
                        let end = word_end(line, i);
                        out.push((i..end, Token::Number));
                        i = end;
                    }
                    b't' | b'f' | b'n' => {
                        let end = word_end(line, i);
                        if matches!(&line[i..end], "true" | "false" | "null") {
                            out.push((i..end, Token::Keyword));
                        }
                        i = end.max(i + 1);
                    }
                    _ => i += 1,
                }
            }
        }
    }
    out
}

/// Where a `key:` ends in a YAML line: before a colon followed by a space or the end.
fn key_colon(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if text.starts_with(['"', '\'', '{', '[']) {
        return None;
    }
    (0..bytes.len())
        .find(|&i| bytes[i] == b':' && (i + 1 == bytes.len() || bytes[i + 1] == b' ') && i > 0)
}

/// Colours the values after `from`: strings, numbers, booleans, and a comment.
fn values(line: &str, from: usize, out: &mut Vec<(Range<usize>, Token)>) {
    let bytes = line.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                let end = string_end(line, i);
                out.push((i..end, Token::String));
                i = end;
            }
            b'#' if i == from || bytes[i - 1] == b' ' => {
                out.push((i..line.len(), Token::Comment));
                return;
            }
            b if b.is_ascii_alphanumeric() || b == b'-' || b == b'+' || b == b'.' => {
                let end = word_end(line, i);
                let token = scalar(&line[i..end]);
                if token != Token::String {
                    out.push((i..end, token));
                }
                i = end.max(i + 1);
            }
            _ => i += 1,
        }
    }
}

/// The kind of a bare value: a number, a keyword, or text.
fn scalar(value: &str) -> Token {
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
    ) {
        Token::Keyword
    } else if !value.is_empty()
        && value
            .trim_start_matches(['-', '+'])
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '_' | 'e' | 'E'))
        && value.chars().any(|c| c.is_ascii_digit())
    {
        Token::Number
    } else {
        Token::String
    }
}

fn comment_after(line: &str, from: usize, out: &mut Vec<(Range<usize>, Token)>) {
    if let Some(at) = line[from..].find(['#', ';']) {
        out.push((from + at..line.len(), Token::Comment));
    }
}

/// Where the string starting with the quote at `start` ends (after its closing
/// quote, or at the end of the line).
fn string_end(line: &str, start: usize) -> usize {
    let quote = line.as_bytes()[start];
    let bytes = line.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    line.len()
}

/// Where a run of word characters starting at `start` ends.
fn word_end(line: &str, start: usize) -> usize {
    line[start..]
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+')))
        .map_or(line.len(), |e| start + e)
}

/// How many columns a tab takes.
const TAB_WIDTH: usize = 4;

/// A line as drawn: tabs as spaces. Returns the text and the byte offsets of its tabs.
fn expand_tabs(line: &str) -> (String, Vec<usize>) {
    if !line.contains('\t') {
        return (line.to_string(), Vec::new());
    }
    let mut tabs = Vec::new();
    let mut out = String::with_capacity(line.len() + 8);
    for (i, c) in line.char_indices() {
        if c == '\t' {
            tabs.push(i);
            out.push_str(&" ".repeat(TAB_WIDTH));
        } else {
            out.push(c);
        }
    }
    (out, tabs)
}

/// A byte offset in a line, as one in its drawn text.
fn to_display(col: usize, tabs: &[usize]) -> usize {
    col + tabs.iter().filter(|&&t| t < col).count() * (TAB_WIDTH - 1)
}

/// A byte offset in a line's drawn text, as one in the line (inside a tab, the tab).
fn from_display(display: usize, tabs: &[usize]) -> usize {
    let mut col = display;
    for &tab in tabs {
        let start = to_display(tab, tabs);
        if display <= start {
            break;
        }
        if display < start + TAB_WIDTH {
            return if display - start > TAB_WIDTH / 2 {
                tab + 1
            } else {
                tab
            };
        }
        col -= TAB_WIDTH - 1;
    }
    col
}

/// One edit, and how to undo it.
#[derive(Clone, Debug)]
struct Edit {
    at: Pos,
    removed: String,
    inserted: String,
    /// The selection (anchor, head) before and after.
    before: (Pos, Pos),
    after: (Pos, Pos),
}

/// Undo and redo: groups of edits. Typing on within a moment, or deleting, is one
/// group, as in a text field.
#[derive(Default)]
struct History {
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
    /// When the last edit was, and whether it inserted text.
    last: Option<(Instant, bool)>,
}

/// How many groups are kept.
const HISTORY: usize = 500;
/// Edits closer together than this are one group.
const GROUP: Duration = Duration::from_millis(1000);

/// What a drag selects by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drag {
    Chars,
    Words(Pos, Pos),
    Lines(usize),
}

/// What the editor tells its view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextEditorEvent {
    /// The text changed (by typing, pasting, undo, …).
    Changed,
}

/// The last drawn frame, for the pointer and the input method.
struct Frame {
    /// Where the text is drawn (right of the gutter).
    text_bounds: Bounds<Pixels>,
    line_height: Pixels,
    char_width: Pixels,
    first_line: usize,
    /// The drawn lines from `first_line`, with their tabs.
    lines: Vec<(ShapedLine, Vec<usize>)>,
    scroll: Point<Pixels>,
}

pub struct TextEditor {
    focus_handle: FocusHandle,
    lines: Vec<String>,
    crlf: bool,
    anchor: Pos,
    head: Pos,
    /// The x a column-keeping move (Up, Down, Page Up) aims for.
    goal_x: Option<Pixels>,
    marked: Option<(Pos, Pos)>,
    read_only: bool,
    language: Language,
    scroll: Point<Pixels>,
    /// Scroll the head into view at the next frame.
    autoscroll: bool,
    history: History,
    dragging: Option<Drag>,
    frame: Option<Frame>,
    /// The longest line, in drawn columns: how far the text scrolls sideways.
    widest: usize,
    label: Text,
    automation_id: SharedString,
    /// Ranges drawn with a highlight, such as the matches of a search
    /// ([`TextEditor::set_highlights`]).
    highlights: Vec<Range<Pos>>,
}

impl EventEmitter<TextEditorEvent> for TextEditor {}

/// Where `query` is in `lines`, ignoring ASCII case (as editors search by default),
/// each match on one line. An empty query matches nothing.
pub fn find_in_lines(lines: &[String], query: &str) -> Vec<Range<Pos>> {
    let mut found = Vec::new();
    if query.is_empty() || query.contains('\n') {
        return found;
    }
    let needle = query.as_bytes();
    for (line_ix, line) in lines.iter().enumerate() {
        let hay = line.as_bytes();
        let mut start = 0;
        while start + needle.len() <= hay.len() {
            if line.is_char_boundary(start)
                && line.is_char_boundary(start + needle.len())
                && hay[start..start + needle.len()].eq_ignore_ascii_case(needle)
            {
                found.push(Pos::new(line_ix, start)..Pos::new(line_ix, start + needle.len()));
                start += needle.len();
            } else {
                start += 1;
            }
        }
    }
    found
}

impl TextEditor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            lines: vec![String::new()],
            crlf: false,
            anchor: Pos::default(),
            head: Pos::default(),
            goal_x: None,
            marked: None,
            read_only: false,
            language: Language::Plain,
            scroll: Point::default(),
            autoscroll: false,
            history: History::default(),
            dragging: None,
            frame: None,
            widest: 0,
            label: Text::default(),
            automation_id: "text-editor".into(),
            highlights: Vec::new(),
        }
    }

    /// Where `query` is in the text, ignoring ASCII case: for a find bar.
    pub fn find(&self, query: &str) -> Vec<Range<Pos>> {
        find_in_lines(&self.lines, query)
    }

    /// Draws `ranges` highlighted (the matches of a search), until set again.
    pub fn set_highlights(&mut self, ranges: Vec<Range<Pos>>, cx: &mut Context<Self>) {
        self.highlights = ranges;
        cx.notify();
    }

    /// Replaces `range` with `text`, as typing would (it can be undone). Nothing
    /// changes in a read-only editor.
    pub fn replace(&mut self, range: Range<Pos>, text: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let range = self.clamp(range.start)..self.clamp(range.end);
        self.edit(range, text, cx);
    }

    /// The editor's name for assistive technology, such as the file's name.
    pub fn with_label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_automation_id(mut self, id: impl Into<SharedString>) -> Self {
        self.automation_id = id.into();
        self
    }

    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    pub fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.language = language;
        cx.notify();
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Replaces the whole text, forgets the undo history, and puts the cursor at the
    /// start. A file's `\r\n` line endings are kept for [`Self::text`].
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.crlf = text.contains("\r\n");
        self.lines = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
            .collect();
        self.anchor = Pos::default();
        self.head = Pos::default();
        self.goal_x = None;
        self.marked = None;
        self.scroll = Point::default();
        self.history = History::default();
        self.widest = self
            .lines
            .iter()
            .map(|l| display_width(l))
            .max()
            .unwrap_or(0);
        cx.notify();
    }

    /// The text, with the file's line endings.
    pub fn text(&self) -> String {
        self.lines.join(if self.crlf { "\r\n" } else { "\n" })
    }

    /// Whether the text is `other` (with the same line endings), without copying it.
    pub fn text_equals(&self, other: &str) -> bool {
        let separator = if self.crlf { "\r\n" } else { "\n" };
        let mut rest = other;
        for (i, line) in self.lines.iter().enumerate() {
            let Some(after) = rest.strip_prefix(line.as_str()) else {
                return false;
            };
            rest = after;
            if i + 1 < self.lines.len() {
                let Some(after) = rest.strip_prefix(separator) else {
                    return false;
                };
                rest = after;
            }
        }
        rest.is_empty()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// The selection, start first.
    pub fn selection(&self) -> Range<Pos> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    /// Where the cursor is.
    pub fn cursor(&self) -> Pos {
        self.head
    }

    /// Selects `range` (the cursor at its end).
    pub fn select(&mut self, range: Range<Pos>, cx: &mut Context<Self>) {
        self.anchor = self.clamp(range.start);
        self.head = self.clamp(range.end);
        self.autoscroll = true;
        cx.notify();
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    fn clamp(&self, pos: Pos) -> Pos {
        let line = pos.line.min(self.lines.len() - 1);
        let text = &self.lines[line];
        let mut col = pos.col.min(text.len());
        while !text.is_char_boundary(col) {
            col -= 1;
        }
        Pos { line, col }
    }

    /// The text in `range`.
    pub fn text_in(&self, range: Range<Pos>) -> String {
        let (start, end) = (range.start, range.end);
        if start.line == end.line {
            return self.lines[start.line][start.col..end.col].to_string();
        }
        let mut out = self.lines[start.line][start.col..].to_string();
        for line in &self.lines[start.line + 1..end.line] {
            out.push('\n');
            out.push_str(line);
        }
        out.push('\n');
        out.push_str(&self.lines[end.line][..end.col]);
        out
    }

    /// Replaces `range` with `text` (with `\n` line breaks); returns where the
    /// inserted text ends. Not recorded for undo.
    fn splice(&mut self, range: Range<Pos>, text: &str) -> Pos {
        let (start, end) = (range.start, range.end);
        let tail = self.lines[end.line][end.col..].to_string();
        let head = self.lines[start.line][..start.col].to_string();
        let mut new_lines: Vec<String> = text.split('\n').map(str::to_string).collect();
        let last = new_lines.len() - 1;
        let end_col = if last == 0 {
            head.len() + new_lines[0].len()
        } else {
            new_lines[last].len()
        };
        new_lines[0] = head + &new_lines[0];
        new_lines[last].push_str(&tail);
        for line in &new_lines {
            self.widest = self.widest.max(display_width(line));
        }
        self.lines.splice(start.line..=end.line, new_lines);
        Pos {
            line: start.line + last,
            col: end_col,
        }
    }

    /// Replaces `range` with `text`, recorded for undo, and puts the cursor after it.
    fn edit(&mut self, range: Range<Pos>, text: &str, cx: &mut Context<Self>) {
        if self.read_only || (range.start == range.end && text.is_empty()) {
            return;
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let before = (self.anchor, self.head);
        let removed = self.text_in(range.clone());
        let end = self.splice(range.clone(), &text);
        self.anchor = end;
        self.head = end;
        self.goal_x = None;
        self.marked = None;
        self.autoscroll = true;
        let edit = Edit {
            at: range.start,
            removed,
            inserted: text.clone(),
            before,
            after: (end, end),
        };
        self.remember(edit, !text.is_empty() && !text.contains('\n'));
        cx.emit(TextEditorEvent::Changed);
        cx.notify();
    }

    fn remember(&mut self, edit: Edit, typing: bool) {
        let now = Instant::now();
        let history = &mut self.history;
        let same_group = typing
            && history
                .last
                .is_some_and(|(at, kind)| kind && now.duration_since(at) < GROUP)
            && history
                .undo
                .last()
                .and_then(|g| g.last())
                .is_some_and(|last| {
                    // typing on from where the last edit ended
                    last.after.1 == edit.at && last.removed.is_empty()
                });
        match history.undo.last_mut() {
            Some(group) if same_group => group.push(edit),
            _ => {
                history.undo.push(vec![edit]);
                if history.undo.len() > HISTORY {
                    history.undo.remove(0);
                }
            }
        }
        history.redo.clear();
        history.last = Some((now, typing));
    }

    /// Where `text` ends when inserted at `at`.
    fn end_of(at: Pos, text: &str) -> Pos {
        match text.rsplit_once('\n') {
            Some((before, last)) => Pos {
                line: at.line + before.matches('\n').count() + 1,
                col: last.len(),
            },
            None => Pos {
                line: at.line,
                col: at.col + text.len(),
            },
        }
    }

    fn undo(&mut self, _: &crate::Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(group) = self.history.undo.pop() else {
            return;
        };
        for edit in group.iter().rev() {
            let end = Self::end_of(edit.at, &edit.inserted);
            self.splice(edit.at..end, &edit.removed);
        }
        let first = &group[0];
        (self.anchor, self.head) = first.before;
        self.history.redo.push(group);
        self.history.last = None;
        self.changed(cx);
    }

    fn redo(&mut self, _: &crate::Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(group) = self.history.redo.pop() else {
            return;
        };
        for edit in &group {
            let end = Self::end_of(edit.at, &edit.removed);
            self.splice(edit.at..end, &edit.inserted);
        }
        let last = group.last().expect("a group has edits");
        (self.anchor, self.head) = last.after;
        self.history.undo.push(group);
        self.history.last = None;
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.anchor = self.clamp(self.anchor);
        self.head = self.clamp(self.head);
        self.goal_x = None;
        self.marked = None;
        self.autoscroll = true;
        cx.emit(TextEditorEvent::Changed);
        cx.notify();
    }

    // ---- Moving ----

    fn prev_pos(&self, pos: Pos) -> Pos {
        if pos.col > 0 {
            let line = &self.lines[pos.line];
            let col = line[..pos.col]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            Pos::new(pos.line, col)
        } else if pos.line > 0 {
            Pos::new(pos.line - 1, self.lines[pos.line - 1].len())
        } else {
            pos
        }
    }

    fn next_pos(&self, pos: Pos) -> Pos {
        let line = &self.lines[pos.line];
        if pos.col < line.len() {
            let width = line[pos.col..].chars().next().map_or(0, char::len_utf8);
            Pos::new(pos.line, pos.col + width)
        } else if pos.line + 1 < self.lines.len() {
            Pos::new(pos.line + 1, 0)
        } else {
            pos
        }
    }

    fn word_left_of(&self, pos: Pos) -> Pos {
        if pos.col == 0 {
            return self.prev_pos(pos);
        }
        Pos::new(
            pos.line,
            previous_word_start(&self.lines[pos.line], pos.col),
        )
    }

    fn word_right_of(&self, pos: Pos) -> Pos {
        let line = &self.lines[pos.line];
        if pos.col == line.len() {
            return self.next_pos(pos);
        }
        Pos::new(pos.line, next_word_end(line, pos.col))
    }

    /// The first non-blank of the line, or its very start when already there, as
    /// code editors' Home does.
    fn smart_home(&self, pos: Pos) -> Pos {
        let line = &self.lines[pos.line];
        let indent = line.len() - line.trim_start().len();
        Pos::new(pos.line, if pos.col == indent { 0 } else { indent })
    }

    /// The x of `pos` in its line's drawn text.
    fn x_of(&self, pos: Pos) -> Pixels {
        let (shown, tabs) = expand_tabs(&self.lines[pos.line]);
        let display = to_display(pos.col, &tabs);
        if let Some(frame) = &self.frame
            && pos.line >= frame.first_line
            && let Some((line, _)) = frame.lines.get(pos.line - frame.first_line)
        {
            return line.x_for_index(display);
        }
        let char_width = self.frame.as_ref().map_or(px(8.), |f| f.char_width);
        char_width * shown[..display].chars().count() as f32
    }

    /// The column of line `line` nearest to `x`.
    fn col_at_x(&self, line: usize, x: Pixels) -> usize {
        let text = &self.lines[line];
        let (shown, tabs) = expand_tabs(text);
        if let Some(frame) = &self.frame
            && line >= frame.first_line
            && let Some((shaped, tabs)) = frame.lines.get(line - frame.first_line)
        {
            return from_display(shaped.closest_index_for_x(x), tabs).min(text.len());
        }
        let char_width = self.frame.as_ref().map_or(px(8.), |f| f.char_width);
        let chars = (x / char_width).round().max(0.) as usize;
        let display = shown
            .char_indices()
            .nth(chars)
            .map_or(shown.len(), |(i, _)| i);
        from_display(display, &tabs).min(text.len())
    }

    /// `lines` lines up (negative) or down from the head, keeping the column.
    fn vertical(&self, lines: isize) -> Pos {
        let goal = self.goal_x.unwrap_or_else(|| self.x_of(self.head));
        let target = self.head.line as isize + lines;
        if target < 0 {
            return Pos::default();
        }
        let target = target as usize;
        if target >= self.lines.len() {
            let last = self.lines.len() - 1;
            return Pos::new(last, self.lines[last].len());
        }
        Pos::new(target, self.col_at_x(target, goal))
    }

    fn page_lines(&self) -> isize {
        self.frame
            .as_ref()
            .map_or(20, |f| {
                (f.text_bounds.size.height / f.line_height).floor() as isize
            })
            .max(1)
    }

    /// Moves the cursor to `pos`; with `select`, the selection's other end stays.
    fn move_to(&mut self, pos: Pos, select: bool, keep_goal: bool, cx: &mut Context<Self>) {
        if keep_goal {
            if self.goal_x.is_none() {
                self.goal_x = Some(self.x_of(self.head));
            }
        } else {
            self.goal_x = None;
        }
        self.head = pos;
        if !select {
            self.anchor = pos;
        }
        self.marked = None;
        self.autoscroll = true;
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection();
        let to = if sel.start != sel.end {
            sel.start
        } else {
            self.prev_pos(self.head)
        };
        self.move_to(to, false, false, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection();
        let to = if sel.start != sel.end {
            sel.end
        } else {
            self.next_pos(self.head)
        };
        self.move_to(to, false, false, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(-1);
        self.move_to(to, false, true, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(1);
        self.move_to(to, false, true, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.prev_pos(self.head);
        self.move_to(to, true, false, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.next_pos(self.head);
        self.move_to(to, true, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(-1);
        self.move_to(to, true, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.vertical(1);
        self.move_to(to, true, true, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_left_of(self.head);
        self.move_to(to, false, false, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_right_of(self.head);
        self.move_to(to, false, false, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_left_of(self.head);
        self.move_to(to, true, false, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.word_right_of(self.head);
        self.move_to(to, true, false, cx);
    }

    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.smart_home(self.head);
        self.move_to(to, false, false, cx);
    }

    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = Pos::new(self.head.line, self.lines[self.head.line].len());
        self.move_to(to, false, false, cx);
    }

    fn select_to_line_start(
        &mut self,
        _: &SelectToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to = self.smart_home(self.head);
        self.move_to(to, true, false, cx);
    }

    fn select_to_line_end(&mut self, _: &SelectToLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = Pos::new(self.head.line, self.lines[self.head.line].len());
        self.move_to(to, true, false, cx);
    }

    fn document_end_pos(&self) -> Pos {
        let last = self.lines.len() - 1;
        Pos::new(last, self.lines[last].len())
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(Pos::default(), false, false, cx);
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.document_end_pos();
        self.move_to(to, false, false, cx);
    }

    fn select_to_document_start(
        &mut self,
        _: &SelectToDocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(Pos::default(), true, false, cx);
    }

    fn select_to_document_end(
        &mut self,
        _: &SelectToDocumentEnd,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to = self.document_end_pos();
        self.move_to(to, true, false, cx);
    }

    fn page(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let lines = self.page_lines();
        let to = self.vertical(if down { lines } else { -lines });
        if let Some(frame) = &self.frame {
            let delta = frame.line_height * lines as f32;
            self.scroll.y = if down {
                self.scroll.y + delta
            } else {
                (self.scroll.y - delta).max(px(0.))
            };
        }
        self.move_to(to, select, true, cx);
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page(false, false, cx);
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page(true, false, cx);
    }

    fn select_page_up(&mut self, _: &SelectPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page(false, true, cx);
    }

    fn select_page_down(&mut self, _: &SelectPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page(true, true, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.anchor = Pos::default();
        self.head = self.document_end_pos();
        cx.notify();
    }

    // ---- Editing ----

    /// Deletes the selection, or from the cursor to `to`.
    fn delete_to(&mut self, to: Pos, cx: &mut Context<Self>) {
        let sel = self.selection();
        let range = if sel.start != sel.end {
            sel
        } else {
            self.head.min(to)..self.head.max(to)
        };
        self.edit(range, "", cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.prev_pos(self.head);
        self.delete_to(to, cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let to = self.next_pos(self.head);
        self.delete_to(to, cx);
    }

    fn delete_word_backward(
        &mut self,
        _: &DeleteWordBackward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to = self.word_left_of(self.head);
        self.delete_to(to, cx);
    }

    fn delete_word_forward(
        &mut self,
        _: &DeleteWordForward,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to = self.word_right_of(self.head);
        self.delete_to(to, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let to = Pos::new(self.head.line, 0);
        self.delete_to(to, cx);
    }

    /// Enter: a new line indented as the one before.
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        let line = &self.lines[self.head.line];
        let indent: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let indent = &indent[..indent.len().min(self.head.col)];
        let text = format!("\n{indent}");
        self.edit(self.selection(), &text, cx);
    }

    /// Tab: indents the selected lines, or inserts spaces to the next tab stop.
    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection();
        if sel.start.line == sel.end.line {
            let column = expand_tabs(&self.lines[self.head.line][..sel.start.col])
                .0
                .chars()
                .count();
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            self.edit(sel, &" ".repeat(spaces), cx);
            return;
        }
        self.reindent(sel, |line| format!("{}{line}", " ".repeat(TAB_WIDTH)), cx);
    }

    /// Shift+Tab: takes up to a tab stop's spaces (or a tab) off the selected lines.
    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection();
        self.reindent(
            sel,
            |line| {
                if let Some(rest) = line.strip_prefix('\t') {
                    return rest.to_string();
                }
                let spaces = line.len() - line.trim_start_matches(' ').len();
                line[spaces.min(TAB_WIDTH)..].to_string()
            },
            cx,
        );
    }

    /// Replaces the whole lines of `sel` with `change` of each, as one step, keeping
    /// them selected.
    fn reindent(
        &mut self,
        sel: Range<Pos>,
        change: impl Fn(&str) -> String,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let first = sel.start.line;
        // a selection ending at a line's start doesn't take that line
        let last = if sel.end.col == 0 && sel.end.line > sel.start.line {
            sel.end.line - 1
        } else {
            sel.end.line
        };
        let range = Pos::new(first, 0)..Pos::new(last, self.lines[last].len());
        let old: Vec<String> = self.lines[first..=last].to_vec();
        let new: Vec<String> = old.iter().map(|l| change(l)).collect();
        if new == old {
            return;
        }
        let text = new.join("\n");
        self.edit(range, &text, cx);
        self.anchor = Pos::new(first, 0);
        self.head = Pos::new(last, self.lines[last].len());
        if let Some(edit) = self.history.undo.last_mut().and_then(|g| g.last_mut()) {
            edit.after = (self.anchor, self.head);
        }
        self.history.last = None;
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection();
        let text = if sel.start == sel.end {
            // without a selection, the line, as code editors do
            format!("{}\n", self.lines[self.head.line])
        } else {
            self.text_in(sel)
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return self.copy(&Copy, window, cx);
        }
        let sel = self.selection();
        self.copy(&Copy, window, cx);
        if sel.start == sel.end {
            // the whole line
            let line = self.head.line;
            let range = if line + 1 < self.lines.len() {
                Pos::new(line, 0)..Pos::new(line + 1, 0)
            } else if line > 0 {
                Pos::new(line - 1, self.lines[line - 1].len())
                    ..Pos::new(line, self.lines[line].len())
            } else {
                Pos::new(0, 0)..Pos::new(0, self.lines[0].len())
            };
            self.edit(range, "", cx);
        } else {
            self.edit(sel, "", cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.edit(self.selection(), &text, cx);
        }
    }

    // ---- The pointer ----

    /// The place under `position`, from the last frame.
    fn pos_at(&self, position: Point<Pixels>) -> Pos {
        let Some(frame) = &self.frame else {
            return Pos::default();
        };
        let y = position.y - frame.text_bounds.top() + frame.scroll.y;
        if y < px(0.) {
            return Pos::default();
        }
        let line = (y / frame.line_height).floor() as usize;
        if line >= self.lines.len() {
            return self.document_end_pos();
        }
        let x = position.x - frame.text_bounds.left() + frame.scroll.x;
        Pos::new(line, self.col_at_x(line, x.max(px(0.))))
    }

    fn word_at(&self, pos: Pos) -> (Pos, Pos) {
        let range = word_range_at(&self.lines[pos.line], pos.col);
        (
            Pos::new(pos.line, range.start),
            Pos::new(pos.line, range.end),
        )
    }

    fn line_range(&self, line: usize) -> (Pos, Pos) {
        let end = if line + 1 < self.lines.len() {
            Pos::new(line + 1, 0)
        } else {
            Pos::new(line, self.lines[line].len())
        };
        (Pos::new(line, 0), end)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let pos = self.pos_at(event.position);
        self.goal_x = None;
        self.marked = None;
        self.dragging = Some(match event.click_count {
            0 | 1 => {
                self.head = pos;
                if !event.modifiers.shift {
                    self.anchor = pos;
                }
                Drag::Chars
            }
            2 => {
                let (start, end) = self.word_at(pos);
                self.anchor = start;
                self.head = end;
                Drag::Words(start, end)
            }
            _ => {
                let (start, end) = self.line_range(pos.line);
                self.anchor = start;
                self.head = end;
                Drag::Lines(pos.line)
            }
        });
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.dragging = None;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.dragging else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.dragging = None;
            return;
        }
        let pos = self.pos_at(event.position);
        let (anchor, head) = match drag {
            Drag::Chars => (self.anchor, pos),
            Drag::Words(start, end) => {
                let (word_start, word_end) = self.word_at(pos);
                if pos < start {
                    (end, word_start)
                } else {
                    (start, word_end.max(end))
                }
            }
            Drag::Lines(line) => {
                let (start, end) = self.line_range(line);
                let (line_start, line_end) = self.line_range(pos.line);
                if pos.line < line {
                    (end, line_start)
                } else {
                    (start, line_end)
                }
            }
        };
        if (anchor, head) != (self.anchor, self.head) {
            self.anchor = anchor;
            self.head = head;
            self.autoscroll = true;
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line_height = self.frame.as_ref().map_or(px(20.), |f| f.line_height);
        let delta = event.delta.pixel_delta(line_height);
        self.scroll.x -= delta.x;
        self.scroll.y -= delta.y;
        self.clamp_scroll();
        cx.stop_propagation();
        cx.notify();
    }

    fn clamp_scroll(&mut self) {
        let Some(frame) = &self.frame else {
            self.scroll = Point::default();
            return;
        };
        let content_height = frame.line_height * self.lines.len() as f32;
        let max_y =
            (content_height - frame.text_bounds.size.height + frame.line_height * 2.).max(px(0.));
        let content_width = frame.char_width * (self.widest + 2) as f32;
        let max_x = (content_width - frame.text_bounds.size.width).max(px(0.));
        self.scroll.y = self.scroll.y.clamp(px(0.), max_y);
        self.scroll.x = self.scroll.x.clamp(px(0.), max_x);
    }

    // ---- UTF-16 offsets, for the input method ----

    fn offset_to_utf16(&self, pos: Pos) -> usize {
        let before: usize = self.lines[..pos.line]
            .iter()
            .map(|l| l.encode_utf16().count() + 1)
            .sum();
        before + self.lines[pos.line][..pos.col].encode_utf16().count()
    }

    fn pos_from_utf16(&self, mut offset: usize) -> Pos {
        for (i, line) in self.lines.iter().enumerate() {
            let len = line.encode_utf16().count();
            if offset <= len {
                let mut units = 0;
                for (byte, c) in line.char_indices() {
                    if units >= offset {
                        return Pos::new(i, byte);
                    }
                    units += c.len_utf16();
                }
                return Pos::new(i, line.len());
            }
            offset -= len + 1;
        }
        self.document_end_pos()
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<Pos> {
        self.pos_from_utf16(range.start)..self.pos_from_utf16(range.end)
    }
}

/// The drawn width of a line in columns (`char`s, tabs as four).
fn display_width(line: &str) -> usize {
    line.chars()
        .map(|c| if c == '\t' { TAB_WIDTH } else { 1 })
        .sum()
}

/// What a word is made of.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Space,
    Word,
    Symbol,
}

fn cluster_kind(cluster: &str) -> CharKind {
    match cluster.chars().next() {
        Some(ch) if ch.is_whitespace() => CharKind::Space,
        Some(ch) if ch.is_alphanumeric() || ch == '_' => CharKind::Word,
        _ => CharKind::Symbol,
    }
}

fn previous_word_start(text: &str, offset: usize) -> usize {
    let mut clusters = text[..offset].grapheme_indices(true).rev().peekable();
    while clusters
        .next_if(|&(_, c)| cluster_kind(c) == CharKind::Space)
        .is_some()
    {}
    let Some(&(_, first)) = clusters.peek() else {
        return 0;
    };
    let kind = cluster_kind(first);
    let mut start = offset;
    while let Some((index, _)) = clusters.next_if(|&(_, c)| cluster_kind(c) == kind) {
        start = index;
    }
    start
}

fn next_word_end(text: &str, offset: usize) -> usize {
    let mut clusters = text[offset..].grapheme_indices(true).peekable();
    while clusters
        .next_if(|&(_, c)| cluster_kind(c) == CharKind::Space)
        .is_some()
    {}
    let Some(&(_, first)) = clusters.peek() else {
        return text.len();
    };
    let kind = cluster_kind(first);
    while clusters
        .next_if(|&(_, c)| cluster_kind(c) == kind)
        .is_some()
    {}
    clusters
        .peek()
        .map_or(text.len(), |&(index, _)| offset + index)
}

fn word_range_at(text: &str, offset: usize) -> Range<usize> {
    let clusters = text.grapheme_indices(true).collect::<Vec<_>>();
    let Some(at) = clusters.iter().rposition(|&(start, _)| start <= offset) else {
        return offset..offset;
    };
    let kind = cluster_kind(clusters[at].1);
    let same = |&&(_, c): &&(usize, &str)| cluster_kind(c) == kind;
    let first = at - clusters[..at].iter().rev().take_while(same).count();
    let last = at + clusters[at + 1..].iter().take_while(same).count();
    clusters[first].0..clusters[last].0 + clusters[last].1.len()
}

impl EntityInputHandler for TextEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end));
        Some(self.text_in(range))
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let sel = self.selection();
        Some(UTF16Selection {
            range: self.offset_to_utf16(sel.start)..self.offset_to_utf16(sel.end),
            reversed: self.head < self.anchor,
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.range_from_utf16(&range_utf16);
        self.anchor = range.start;
        self.head = range.end;
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.offset_to_utf16(self.document_end_pos()))
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .map(|(start, end)| self.offset_to_utf16(start)..self.offset_to_utf16(end))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.map(|(s, e)| s..e))
            .unwrap_or_else(|| self.selection());
        self.edit(range, new_text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.map(|(s, e)| s..e))
            .unwrap_or_else(|| self.selection());
        let start = range.start;
        self.edit(range, new_text, cx);
        let end = Self::end_of(start, new_text);
        self.marked = (!new_text.is_empty()).then_some((start, end));
        if let Some(selected) = new_selected_range_utf16 {
            let base = self.offset_to_utf16(start);
            let length = new_text.encode_utf16().count();
            self.anchor = self.pos_from_utf16(base + selected.start.min(length));
            self.head = self.pos_from_utf16(base + selected.end.min(length));
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let frame = self.frame.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let top =
            frame.text_bounds.top() + frame.line_height * range.start.line as f32 - frame.scroll.y;
        let left = frame.text_bounds.left() + self.x_of(range.start) - frame.scroll.x;
        Some(Bounds::new(
            point(left, top),
            size(px(2.), frame.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let pos = self.pos_at(point);
        Some(self.offset_to_utf16(pos))
    }
}

impl Focusable for TextEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.label.get();
        div()
            .id("text-editor")
            .accessible(Role::MultilineTextInput, self.automation_id.clone(), label)
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_hidden()
            // Monaco's: the panel colour when dark, white when light
            .bg(if crate::theme::palette::is_light() {
                colors::GRAY_800
            } else {
                colors::GRAY_850
            })
            .font_family(fonts::mono())
            .text_size(theme::text_size(text::SMALL))
            .line_height(px(20.))
            .text_color(colors::GRAY_300)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::line_start))
            .on_action(cx.listener(Self::line_end))
            .on_action(cx.listener(Self::select_to_line_start))
            .on_action(cx.listener(Self::select_to_line_end))
            .on_action(cx.listener(Self::document_start))
            .on_action(cx.listener(Self::document_end))
            .on_action(cx.listener(Self::select_to_document_start))
            .on_action(cx.listener(Self::select_to_document_end))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::select_page_up))
            .on_action(cx.listener(Self::select_page_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .when(!self.read_only, |this| {
                this.on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_backward))
                    .on_action(cx.listener(Self::delete_word_forward))
                    .on_action(cx.listener(Self::delete_to_line_start))
                    .on_action(cx.listener(Self::newline))
                    .on_action(cx.listener(Self::indent))
                    .on_action(cx.listener(Self::outdent))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::undo))
                    .on_action(cx.listener(Self::redo))
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(EditorElement {
                editor: cx.entity(),
            })
    }
}

/// Draws the visible lines, the gutter, the selection and the cursor.
struct EditorElement {
    editor: Entity<TextEditor>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// What the gutter and the text are drawn with.
struct Prepaint {
    gutter: Vec<(ShapedLine, Point<Pixels>)>,
    text: Vec<(ShapedLine, Point<Pixels>)>,
    gutter_bounds: Bounds<Pixels>,
    current_line: Option<Bounds<Pixels>>,
    selection: Vec<Bounds<Pixels>>,
    /// The highlighted ranges in view (search matches).
    highlights: Vec<Bounds<Pixels>>,
    cursor: Option<Bounds<Pixels>>,
    text_bounds: Bounds<Pixels>,
    scrollbar: Option<Bounds<Pixels>>,
}

const GUTTER_PADDING: f32 = 12.;
const TEXT_PADDING: f32 = 8.;

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let style = window.text_style();
        let font = style.font();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let text_system = window.text_system().clone();
        let font_id = text_system.resolve_font(&font);
        let char_width = text_system
            .ch_advance(font_id, font_size)
            .unwrap_or(font_size * 0.6);
        let focused = self.editor.read(cx).focus_handle.is_focused(window);

        self.editor.update(cx, |editor, _| {
            let digits = editor.lines.len().to_string().len().max(2);
            let gutter_width = char_width * digits as f32 + px(GUTTER_PADDING * 2.);
            let text_bounds = Bounds::from_corners(
                point(
                    bounds.left() + gutter_width + px(TEXT_PADDING),
                    bounds.top(),
                ),
                bounds.bottom_right(),
            );
            // keep the cursor in view
            if editor.autoscroll {
                editor.autoscroll = false;
                let head = editor.head;
                let top = line_height * head.line as f32;
                let height = text_bounds.size.height;
                if top < editor.scroll.y {
                    editor.scroll.y = top;
                } else if top + line_height > editor.scroll.y + height {
                    editor.scroll.y = top + line_height - height;
                }
                let (shown, tabs) = expand_tabs(&editor.lines[head.line]);
                let x = char_width * shown[..to_display(head.col, &tabs)].chars().count() as f32;
                let width = text_bounds.size.width;
                if x < editor.scroll.x {
                    editor.scroll.x = (x - char_width * 4.).max(px(0.));
                } else if x + char_width * 2. > editor.scroll.x + width {
                    editor.scroll.x = x + char_width * 4. - width;
                }
            }
            editor.frame = Some(Frame {
                text_bounds,
                line_height,
                char_width,
                first_line: 0,
                lines: Vec::new(),
                scroll: editor.scroll,
            });
            editor.clamp_scroll();
            let scroll = editor.scroll;
            let first_line = (scroll.y / line_height).floor() as usize;
            let visible = (text_bounds.size.height / line_height).ceil() as usize + 1;
            let last_line = (first_line + visible).min(editor.lines.len());

            let run = |len: usize, color: Hsla| TextRun {
                len,
                font: font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let base: Hsla = colors::GRAY_300.into();
            let mut shaped_lines = Vec::new();
            let mut text = Vec::new();
            let mut gutter = Vec::new();
            for line_ix in first_line.min(last_line)..last_line {
                let line = &editor.lines[line_ix];
                let (shown, tabs) = expand_tabs(line);
                // colours, as runs over the drawn text
                let mut runs = Vec::new();
                let mut at = 0;
                for (range, token) in tokens(editor.language, line) {
                    let start = to_display(range.start, &tabs);
                    let end = to_display(range.end, &tabs);
                    if start > at {
                        runs.push(run(start - at, base));
                    }
                    if end > start {
                        runs.push(run(end - start, token.color().into()));
                    }
                    at = end.max(at);
                }
                if shown.len() > at {
                    runs.push(run(shown.len() - at, base));
                }
                if let Some((start, end)) = editor.marked
                    && start.line == line_ix
                    && end.line == line_ix
                {
                    // the input method's text, underlined
                    let (s, e) = (to_display(start.col, &tabs), to_display(end.col, &tabs));
                    runs = split_underline(runs, s..e, base);
                }
                let y = text_bounds.top() + line_height * line_ix as f32 - scroll.y;
                let shaped = text_system.shape_line(shown.into(), font_size, &runs, None);
                text.push((shaped.clone(), point(text_bounds.left() - scroll.x, y)));
                shaped_lines.push((shaped, tabs));

                let number: SharedString = (line_ix + 1).to_string().into();
                let color: Hsla = if line_ix == editor.head.line && focused {
                    colors::GRAY_300.into()
                } else {
                    colors::GRAY_500.into()
                };
                let numeral = text_system.shape_line(
                    number.clone(),
                    font_size,
                    &[run(number.len(), color)],
                    None,
                );
                let x = bounds.left() + gutter_width - px(GUTTER_PADDING) - numeral.width;
                gutter.push((numeral, point(x, y)));
            }

            let sel = editor.selection();
            let head = editor.head;
            let line_count = editor.lines.len();
            let frame = editor.frame.as_mut().expect("set above");
            frame.first_line = first_line;
            frame.lines = shaped_lines;
            frame.scroll = scroll;

            // the selection, line by line
            let mut selection = Vec::new();
            if sel.start != sel.end {
                for line_ix in
                    sel.start.line.max(first_line)..=sel.end.line.min(last_line.saturating_sub(1))
                {
                    let Some((shaped, tabs)) = frame.lines.get(line_ix - first_line) else {
                        continue;
                    };
                    let start = if line_ix == sel.start.line {
                        shaped.x_for_index(to_display(sel.start.col, tabs))
                    } else {
                        px(0.)
                    };
                    let end = if line_ix == sel.end.line {
                        shaped.x_for_index(to_display(sel.end.col, tabs))
                    } else {
                        shaped.width + char_width
                    };
                    let y = text_bounds.top() + line_height * line_ix as f32 - scroll.y;
                    selection.push(Bounds::from_corners(
                        point(text_bounds.left() + start - scroll.x, y),
                        point(text_bounds.left() + end - scroll.x, y + line_height),
                    ));
                }
            }
            // the highlighted ranges (search matches) on the lines in view
            let mut highlights = Vec::new();
            for range in &editor.highlights {
                let line_ix = range.start.line;
                if range.end.line != line_ix || line_ix < first_line || line_ix >= last_line {
                    continue;
                }
                let Some((shaped, tabs)) = frame.lines.get(line_ix - first_line) else {
                    continue;
                };
                let start = shaped.x_for_index(to_display(range.start.col, tabs));
                let end = shaped.x_for_index(to_display(range.end.col, tabs));
                let y = text_bounds.top() + line_height * line_ix as f32 - scroll.y;
                highlights.push(Bounds::from_corners(
                    point(text_bounds.left() + start - scroll.x, y),
                    point(text_bounds.left() + end - scroll.x, y + line_height),
                ));
            }
            let head_y = text_bounds.top() + line_height * head.line as f32 - scroll.y;
            let cursor = (head.line >= first_line && head.line < last_line && focused).then(|| {
                let (shaped, tabs) = &frame.lines[head.line - first_line];
                let x = shaped.x_for_index(to_display(head.col, tabs));
                Bounds::new(
                    point(text_bounds.left() + x - scroll.x, head_y),
                    size(px(1.5), line_height),
                )
            });
            let current_line = (sel.start == sel.end && focused).then(|| {
                Bounds::new(
                    point(bounds.left(), head_y),
                    size(bounds.size.width, line_height),
                )
            });
            // a scroll bar's thumb, when the text is taller than the view
            let content = line_height * (line_count + 2) as f32;
            let scrollbar = (content > bounds.size.height).then(|| {
                let ratio = bounds.size.height / content;
                let height = (bounds.size.height * ratio).max(px(24.));
                let top = bounds.top()
                    + (bounds.size.height - height)
                        * (scroll.y / (content - bounds.size.height)).min(1.);
                Bounds::new(point(bounds.right() - px(8.), top), size(px(6.), height))
            });

            Prepaint {
                gutter,
                text,
                gutter_bounds: Bounds::new(bounds.origin, size(gutter_width, bounds.size.height)),
                current_line,
                selection,
                highlights,
                cursor,
                text_bounds,
                scrollbar,
            }
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let line_height = window.line_height();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(line) = prepaint.current_line {
                window.paint_quad(fill(line, colors::GRAY_800));
            }
            let gutter = if crate::theme::palette::is_light() {
                colors::GRAY_900
            } else {
                colors::GRAY_875
            };
            window.paint_quad(fill(prepaint.gutter_bounds, gutter));
            let text_mask = Bounds::from_corners(
                point(prepaint.text_bounds.left() - px(TEXT_PADDING), bounds.top()),
                bounds.bottom_right(),
            );
            window.with_content_mask(Some(ContentMask { bounds: text_mask }), |window| {
                for highlight in &prepaint.highlights {
                    window.paint_quad(fill(*highlight, colors::YELLOW.opacity(30)));
                }
                for selection in &prepaint.selection {
                    window.paint_quad(fill(*selection, crate::theme::accent::faded().opacity(45)));
                }
                for (line, origin) in &prepaint.text {
                    line.paint(
                        *origin,
                        line_height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                }
                if let Some(cursor) = prepaint.cursor {
                    window.paint_quad(fill(cursor, colors::GRAY_300));
                }
            });
            for (line, origin) in &prepaint.gutter {
                line.paint(
                    *origin,
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            if let Some(thumb) = prepaint.scrollbar {
                window.paint_quad(fill(thumb, colors::GRAY_FADED.opacity(30)).corner_radii(px(3.)));
            }
        });
    }
}

/// `runs` with `range` (in the drawn text) underlined, for the input method's text.
fn split_underline(runs: Vec<TextRun>, range: Range<usize>, color: Hsla) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0;
    for run in runs {
        let (start, end) = (at, at + run.len);
        at = end;
        let cuts = [
            start,
            range.start.clamp(start, end),
            range.end.clamp(start, end),
            end,
        ];
        for pair in cuts.windows(2) {
            let len = pair[1] - pair[0];
            if len == 0 {
                continue;
            }
            let mut part = run.clone();
            part.len = len;
            if pair[0] >= range.start && pair[1] <= range.end {
                part.underline = Some(UnderlineStyle {
                    color: Some(color),
                    thickness: px(1.),
                    wavy: false,
                });
            }
            out.push(part);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_ignores_ascii_case_and_never_overlaps() {
        let lines: Vec<String> = ["motd=A Server", "max-players=20", "aaa", "é motd"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            find_in_lines(&lines, "MOTD"),
            [
                Pos::new(0, 0)..Pos::new(0, 4),
                Pos::new(3, 3)..Pos::new(3, 7)
            ]
        );
        assert_eq!(
            find_in_lines(&lines, "aa"),
            [Pos::new(2, 0)..Pos::new(2, 2)]
        );
        assert!(find_in_lines(&lines, "").is_empty());
        assert!(find_in_lines(&lines, "a\nb").is_empty());
        assert!(find_in_lines(&lines, "nothing").is_empty());
    }

    #[test]
    fn tabs_show_as_four_columns() {
        let (shown, tabs) = expand_tabs("a\tb\tc");
        assert_eq!(shown, "a    b    c");
        assert_eq!(tabs, vec![1, 3]);
        assert_eq!(to_display(0, &tabs), 0);
        assert_eq!(to_display(2, &tabs), 5);
        assert_eq!(to_display(4, &tabs), 10);
        assert_eq!(from_display(5, &tabs), 2);
        assert_eq!(from_display(10, &tabs), 4);
        assert_eq!(from_display(2, &tabs), 1, "early in the tab, before it");
        assert_eq!(from_display(4, &tabs), 2, "late in the tab, after it");
    }

    fn kinds(language: Language, line: &str) -> Vec<(&str, Token)> {
        tokens(language, line)
            .into_iter()
            .map(|(range, token)| (&line[range], token))
            .collect()
    }

    #[test]
    fn colours_configuration_formats() {
        assert_eq!(
            kinds(Language::Properties, "motd=A Minecraft Server"),
            vec![("motd", Token::Key), ("A Minecraft Server", Token::String)]
        );
        assert_eq!(
            kinds(Language::Properties, "max-players=20"),
            vec![("max-players", Token::Key), ("20", Token::Number)]
        );
        assert_eq!(
            kinds(Language::Properties, "# a comment"),
            vec![("# a comment", Token::Comment)]
        );
        assert_eq!(
            kinds(Language::Toml, "[section] # x"),
            vec![("[section]", Token::Section), ("# x", Token::Comment)]
        );
        assert_eq!(
            kinds(Language::Toml, "name = \"x # y\" # z"),
            vec![
                ("name", Token::Key),
                ("\"x # y\"", Token::String),
                ("# z", Token::Comment)
            ]
        );
        assert_eq!(
            kinds(Language::Yaml, "  allow-end: true"),
            vec![("allow-end", Token::Key), ("true", Token::Keyword)]
        );
        assert_eq!(
            kinds(Language::Yaml, "- url: http://x"),
            vec![("url", Token::Key)]
        );
        assert_eq!(
            kinds(
                Language::Json,
                r#"  "pack": {"pack_format": 15, "x": null}"#
            ),
            vec![
                (r#""pack""#, Token::Key),
                (r#""pack_format""#, Token::Key),
                ("15", Token::Number),
                (r#""x""#, Token::Key),
                ("null", Token::Keyword)
            ]
        );
        assert_eq!(Language::of("server.properties"), Language::Properties);
        assert_eq!(Language::of(".lodestone_config"), Language::Json);
        assert_eq!(Language::of("paper.YML"), Language::Yaml);
        assert_eq!(Language::of("latest.log"), Language::Plain);
    }

    #[test]
    fn word_ranges() {
        assert_eq!(word_range_at("foo bar", 5), 4..7);
        assert_eq!(word_range_at("foo bar", 7), 4..7);
        assert_eq!(word_range_at("", 0), 0..0);
    }
}

/// Keys and clicks in a test window. `cargo test -p lodestone-ui --features screenshot`.
#[cfg(all(test, feature = "screenshot"))]
mod window_tests {
    use gpui::{ClipboardItem, Entity, Focusable as _, TestAppContext, VisualTestContext};

    use super::{Pos, TextEditor};

    fn editor<'a>(
        text: &str,
        cx: &'a mut TestAppContext,
    ) -> (Entity<TextEditor>, &'a mut VisualTestContext) {
        cx.update(crate::components::init);
        let text = text.to_string();
        let (editor, cx) = cx.add_window_view(move |_, cx| {
            let mut editor = TextEditor::new(cx);
            editor.set_text(&text, cx);
            editor
        });
        cx.update(|window, cx| {
            let handle = editor.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        });
        cx.run_until_parked();
        (editor, cx)
    }

    fn text(editor: &Entity<TextEditor>, cx: &mut VisualTestContext) -> String {
        editor.read_with(cx, |editor, _| editor.text())
    }

    fn cursor(editor: &Entity<TextEditor>, cx: &mut VisualTestContext) -> Pos {
        editor.read_with(cx, |editor, _| editor.cursor())
    }

    const UNDO: &str = if cfg!(target_os = "macos") {
        "cmd-z"
    } else {
        "ctrl-z"
    };
    const DOC_END: &str = if cfg!(target_os = "macos") {
        "cmd-down"
    } else {
        "ctrl-end"
    };
    const REDO: &str = if cfg!(target_os = "macos") {
        "cmd-shift-z"
    } else {
        "ctrl-shift-z"
    };

    #[test]
    fn typing_new_lines_keep_the_indent() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("settings:", &mut cx);
        cx.simulate_keystrokes(&format!("{DOC_END} end enter tab"));
        cx.simulate_input("allow-end: true");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("x: 1");
        assert_eq!(
            text(&editor, cx),
            "settings:\n    allow-end: true\n    x: 1"
        );
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("abcdef\nab\nabcdef", &mut cx);
        cx.simulate_keystrokes("right right right right down");
        assert_eq!(cursor(&editor, cx), Pos::new(1, 2));
        cx.simulate_keystrokes("down");
        assert_eq!(cursor(&editor, cx), Pos::new(2, 4));
        cx.simulate_keystrokes("up up");
        assert_eq!(cursor(&editor, cx), Pos::new(0, 4));
        cx.simulate_keystrokes("up");
        assert_eq!(
            cursor(&editor, cx),
            Pos::new(0, 0),
            "past the top, the start"
        );
    }

    #[test]
    fn deleting_across_lines_and_undo() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("one\ntwo\nthree", &mut cx);
        cx.simulate_keystrokes("right right shift-down shift-down backspace");
        assert_eq!(text(&editor, cx), "onree");
        cx.simulate_keystrokes(UNDO);
        assert_eq!(text(&editor, cx), "one\ntwo\nthree");
        cx.simulate_keystrokes(REDO);
        assert_eq!(text(&editor, cx), "onree");
        // typing is one step
        cx.simulate_input("abc");
        assert_eq!(text(&editor, cx), "onabcree");
        cx.simulate_keystrokes(UNDO);
        assert_eq!(text(&editor, cx), "onree");
    }

    #[test]
    fn paste_copy_and_cut_lines() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("a\nb", &mut cx);
        cx.write_to_clipboard(ClipboardItem::new_string("x\r\ny\n".into()));
        cx.simulate_keystrokes("secondary-v");
        assert_eq!(text(&editor, cx), "x\ny\na\nb");
        assert_eq!(cursor(&editor, cx), Pos::new(2, 0));
        // without a selection, Cut takes the line
        cx.simulate_keystrokes("secondary-x");
        assert_eq!(text(&editor, cx), "x\ny\nb");
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("a\n")
        );
        cx.simulate_keystrokes("secondary-a secondary-c");
        assert_eq!(
            cx.read_from_clipboard().and_then(|i| i.text()).as_deref(),
            Some("x\ny\nb")
        );
    }

    #[test]
    fn tab_indents_lines_and_shift_tab_outdents() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("a\nb\nc", &mut cx);
        cx.simulate_keystrokes("shift-down shift-right tab");
        assert_eq!(text(&editor, cx), "    a\n    b\nc");
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(text(&editor, cx), "a\nb\nc");
        cx.simulate_keystrokes(UNDO);
        assert_eq!(text(&editor, cx), "    a\n    b\nc");
    }

    #[test]
    fn line_endings_are_kept() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("a\r\nb\r\n", &mut cx);
        cx.simulate_keystrokes("end enter");
        cx.simulate_input("x");
        assert_eq!(text(&editor, cx), "a\r\nx\r\nb\r\n");
        assert!(editor.read_with(cx, |e, _| e.text_equals("a\r\nx\r\nb\r\n")));
        assert!(!editor.read_with(cx, |e, _| e.text_equals("a\nx\nb\n")));
    }

    #[test]
    fn read_only_takes_no_edits() {
        let mut cx = TestAppContext::single();
        let (editor, cx) = editor("abc", &mut cx);
        editor.update(cx, |e, cx| e.set_read_only(true, cx));
        cx.simulate_input("x");
        cx.simulate_keystrokes("backspace enter");
        assert_eq!(text(&editor, cx), "abc");
    }

    #[test]
    fn a_large_file_opens_and_edits() {
        let mut cx = TestAppContext::single();
        let big: String = (0..200_000).map(|i| format!("key_{i} = {i}\n")).collect();
        let (editor, cx) = editor(&big, &mut cx);
        let started = std::time::Instant::now();
        cx.simulate_keystrokes(DOC_END);
        cx.simulate_input("end");
        cx.simulate_keystrokes("up up home");
        cx.simulate_input("!");
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert_eq!(editor.read_with(cx, |e, _| e.line_count()), 200_001);
        assert!(text(&editor, cx).ends_with("!key_199998 = 199998\nkey_199999 = 199999\nend"));
    }
}
