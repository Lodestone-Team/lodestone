//! The editor's find bar (⌘F, or Ctrl+F): finds text in the open file, ignoring
//! case, highlights every match, steps through them (Enter, Shift+Enter, the arrows),
//! and replaces one or all of them (Monaco's find widget, in the dashboard's editor).

use std::ops::Range;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, KeyDownEvent, ParentElement, Styled,
    Subscription, Window, div, prelude::*,
};
use lodestone_ui::{
    components::{Button, ButtonSize, Icon, IconButton, Pos, TextInput},
    theme::{colors, ink, px, radius, text},
};

use super::FilesPage;
use crate::i18n::{ConsoleFilesMsg as M, t, tr};

pub struct FindBar {
    query: Entity<TextInput>,
    replacement: Entity<TextInput>,
    matches: Vec<Range<Pos>>,
    /// The match selected, into `matches`.
    current: usize,
    /// Focus the query at the next frame.
    focus: bool,
    _subscriptions: Vec<Subscription>,
}

/// The match to start from: the first at or after `cursor`, else the first.
pub fn first_from(matches: &[Range<Pos>], cursor: Pos) -> usize {
    matches
        .iter()
        .position(|range| range.start >= cursor)
        .unwrap_or(0)
}

/// `current` moved by `step` (1 or -1) among `count` matches, going round.
pub fn step_index(current: usize, step: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    (current as isize + step).rem_euclid(count as isize) as usize
}

impl FilesPage {
    /// Opens the find bar on the open file, with the selected text when it is on one
    /// line; again, it selects the query.
    pub(super) fn open_find(&mut self, cx: &mut Context<Self>) {
        if !matches!(
            self.opened.as_ref().map(|o| &o.content),
            Some(super::Content::Loaded(_))
        ) {
            return;
        }
        let selected = {
            let editor = self.editor.read(cx);
            let selection = editor.selection();
            (selection.start.line == selection.end.line && selection.start != selection.end)
                .then(|| editor.text_in(selection))
        };
        if let Some(find) = &mut self.find {
            if let Some(selected) = selected {
                find.query
                    .update(cx, |input, cx| input.set_text(selected, cx));
            }
            find.focus = true;
            cx.notify();
            return;
        }
        let query = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(t(M::FilesFind))
                .with_text(selected.unwrap_or_default())
                .with_automation_id("files-find")
        });
        let replacement = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(t(M::FilesReplaceWith))
                .with_automation_id("files-replace")
        });
        // the matches follow the query as it is typed
        let subscriptions = vec![cx.observe(&query, |this, _, cx| this.refind(true, cx))];
        self.find = Some(FindBar {
            query,
            replacement,
            matches: Vec::new(),
            current: 0,
            focus: true,
            _subscriptions: subscriptions,
        });
        self.refind(true, cx);
    }

    /// Opens the find bar on `query`, for the development screenshots (`--find`).
    pub fn find_for_screenshot(&mut self, query: &str, cx: &mut Context<Self>) {
        self.open_find(cx);
        if let Some(find) = &self.find {
            find.query
                .update(cx, |input, cx| input.set_text(query.to_string(), cx));
        }
        self.refind(true, cx);
    }

    pub(super) fn close_find(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.find.take().is_none() {
            return;
        }
        self.editor
            .update(cx, |editor, cx| editor.set_highlights(Vec::new(), cx));
        if let Some(window) = window {
            let handle = gpui::Focusable::focus_handle(self.editor.read(cx), cx);
            window.focus(&handle, cx);
        }
        cx.notify();
    }

    /// Finds the query again (it or the text changed). With `jump`, selects the
    /// match at or after the cursor.
    pub(super) fn refind(&mut self, jump: bool, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else {
            return;
        };
        let query = find.query.read(cx).text().to_string();
        let editor = self.editor.read(cx);
        find.matches = editor.find(&query);
        let cursor = editor.selection().start;
        find.current = if jump {
            first_from(&find.matches, cursor)
        } else {
            find.current.min(find.matches.len().saturating_sub(1))
        };
        let matches = find.matches.clone();
        let selected = jump.then(|| matches.get(find.current).cloned()).flatten();
        self.editor.update(cx, |editor, cx| {
            editor.set_highlights(matches, cx);
            if let Some(range) = selected {
                editor.select(range, cx);
            }
        });
        cx.notify();
    }

    fn step(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else {
            return;
        };
        find.current = step_index(find.current, step, find.matches.len());
        if let Some(range) = find.matches.get(find.current).cloned() {
            self.editor
                .update(cx, |editor, cx| editor.select(range, cx));
        }
        cx.notify();
    }

    fn replace_current(&mut self, cx: &mut Context<Self>) {
        let Some(find) = &self.find else {
            return;
        };
        let Some(range) = find.matches.get(find.current).cloned() else {
            return;
        };
        let text = find.replacement.read(cx).text().to_string();
        self.editor
            .update(cx, |editor, cx| editor.replace(range, &text, cx));
        // on to the next match, after the replaced text
        self.refind(true, cx);
    }

    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let Some(find) = &self.find else {
            return;
        };
        let text = find.replacement.read(cx).text().to_string();
        let matches = find.matches.clone();
        self.editor.update(cx, |editor, cx| {
            // the last first, so the earlier ranges still point at their text
            for range in matches.into_iter().rev() {
                editor.replace(range, &text, cx);
            }
        });
        self.refind(false, cx);
    }

    /// The bar above the editor.
    pub(super) fn find_bar(
        &mut self,
        can_write: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let find = self.find.as_mut()?;
        if std::mem::take(&mut find.focus) {
            let handle = gpui::Focusable::focus_handle(find.query.read(cx), cx);
            window.focus(&handle, cx);
        }
        let query_empty = find.query.read(cx).text().is_empty();
        let count = if find.matches.is_empty() {
            (!query_empty).then(|| t(M::FilesFindNone).to_string())
        } else {
            Some(tr!(
                M::FilesFindCount,
                current = find.current + 1,
                total = find.matches.len()
            ))
        };
        let none = find.matches.is_empty();
        let field = |input: &Entity<TextInput>| div().w(px(220.)).child(input.clone());
        Some(
            div()
                .id("files-find-bar")
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .py(px(6.))
                .border_b_1()
                .border_color(colors::GRAY_FADED.opacity(30))
                .bg(colors::GRAY_850)
                .rounded_tr(radius::LG)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            let step = if event.keystroke.modifiers.shift {
                                -1
                            } else {
                                1
                            };
                            this.step(step, cx);
                        }
                        "escape" => this.close_find(Some(window), cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }))
                .child(field(&find.query))
                .child(
                    div()
                        .min_w(px(72.))
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .children(count),
                )
                .child(
                    IconButton::new("files-find-previous", Icon::ArrowUp)
                        .label(t(M::FilesFindPrevious))
                        .disabled(none)
                        .on_click(cx.listener(|this, _, _, cx| this.step(-1, cx))),
                )
                .child(
                    IconButton::new("files-find-next", Icon::ArrowDown)
                        .label(t(M::FilesFindNext))
                        .disabled(none)
                        .on_click(cx.listener(|this, _, _, cx| this.step(1, cx))),
                )
                .when(can_write, |this| {
                    this.child(field(&find.replacement))
                        .child(
                            Button::new("files-replace", t(M::FilesReplace))
                                .size(ButtonSize::Slim)
                                .disabled(none)
                                .on_click(cx.listener(|this, _, _, cx| this.replace_current(cx))),
                        )
                        .child(
                            Button::new("files-replace-all", t(M::FilesReplaceAll))
                                .size(ButtonSize::Slim)
                                .disabled(none)
                                .on_click(cx.listener(|this, _, _, cx| this.replace_all(cx))),
                        )
                })
                .child(div().flex_1())
                .child(
                    IconButton::new("files-find-close", Icon::Xmark)
                        .label(t(M::FilesFindClose))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.close_find(Some(window), cx)),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_starts_at_the_cursor_and_goes_round() {
        let matches = [
            Pos::new(0, 0)..Pos::new(0, 3),
            Pos::new(4, 2)..Pos::new(4, 5),
            Pos::new(9, 0)..Pos::new(9, 3),
        ];
        assert_eq!(first_from(&matches, Pos::new(0, 0)), 0);
        assert_eq!(first_from(&matches, Pos::new(3, 7)), 1);
        assert_eq!(
            first_from(&matches, Pos::new(9, 1)),
            0,
            "after the last: the first"
        );
        assert_eq!(step_index(2, 1, 3), 0);
        assert_eq!(step_index(0, -1, 3), 2);
        assert_eq!(step_index(0, 1, 0), 0);
    }
}
