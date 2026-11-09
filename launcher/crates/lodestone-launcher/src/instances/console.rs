//! The Logs tab: the selected instance's console, live.
//!
//! Rows are coloured by level, can be filtered by level and searched, and copied.
//! Their messages select like one big text field (`components::log_view`): drag
//! across rows, ⌘A selects the rows shown, ⌘C copies the selected messages.
//! The list follows new output until the user scrolls up, and follows again once
//! they scroll back to the bottom. It keeps the last 10,000 rows.

use crate::theme::px;
use std::collections::BTreeSet;

use gpui::{
    ClipboardItem, Context, Entity, IntoElement, Render, ScrollEvent, ScrollStrategy, SharedString,
    Subscription, UniformListScrollHandle, Window, div, prelude::*, uniform_list,
};
use launcher_core::game::log4j::{Level, Stream};

use super::{GameLog, Instances, LogRow};
use crate::components::{
    CopySelection, KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
    log_view::{self, LineSource, LogLine, LogSelection, SelectAll},
};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, TextInput},
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, t, trn},
    theme::{self, Color, colors, fonts, radius, text, weights},
};

/// The levels the filter chips toggle, with their ids and labels. TRACE goes with
/// DEBUG, FATAL with ERROR.
const CHIPS: [(Level, &str, M); 4] = [
    (Level::Debug, "debug", M::LevelDebug),
    (Level::Info, "info", M::LevelInfo),
    (Level::Warn, "warn", M::LevelWarn),
    (Level::Error, "error", M::LevelError),
];

fn chip_of(level: Level) -> Level {
    match level {
        Level::Trace | Level::Debug => Level::Debug,
        Level::Info => Level::Info,
        Level::Warn => Level::Warn,
        Level::Error | Level::Fatal => Level::Error,
    }
}

pub fn level_color(level: Level) -> Color {
    match level {
        Level::Trace | Level::Debug => colors::GRAY_500,
        Level::Info => colors::GRAY_300,
        Level::Warn => colors::YELLOW,
        Level::Error | Level::Fatal => colors::RED_200,
    }
}

pub struct Console {
    instances: Entity<Instances>,
    search: Entity<TextInput>,
    shown: BTreeSet<Level>,
    scroll: UniformListScrollHandle,
    /// Whether the list follows new rows.
    follow: bool,
    /// The log revision last scrolled to.
    seen: (Option<String>, u64),
    copied: bool,
    selection: LogSelection,
    _observe: Vec<Subscription>,
}

/// The rows shown, for the selection: their messages, by row id.
struct Shown<'a> {
    log: &'a GameLog,
    visible: &'a [usize],
}

impl LineSource for Shown<'_> {
    fn line_count(&self) -> usize {
        self.visible.len()
    }

    fn line_id(&self, ix: usize) -> u64 {
        self.log.row_id(self.visible[ix])
    }

    fn line_text(&self, ix: usize) -> &str {
        self.log
            .rows()
            .get(self.visible[ix])
            .map_or("", |row| row.text.as_ref())
    }
}

impl Console {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::SearchLog)
                .with_automation_id("log-search")
        });
        let selection = LogSelection::new(cx);
        {
            let console = cx.entity().downgrade();
            selection.on_drag(move |dragging, _, cx| {
                console
                    .update(cx, |this, cx| {
                        // Once released, follow again only if still at the bottom.
                        if !dragging {
                            this.follow = this.scroll.is_scrolled_to_end() != Some(false);
                        }
                        cx.notify();
                    })
                    .ok();
            });
        }
        let observe = vec![
            cx.observe(&instances, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];
        Self {
            instances,
            search,
            shown: CHIPS.iter().map(|(level, _, _)| *level).collect(),
            scroll: UniformListScrollHandle::new(),
            follow: true,
            seen: (None, 0),
            copied: false,
            selection,
            _observe: observe,
        }
    }

    /// Fills in the search, for the offscreen screenshots.
    pub fn with_search(self, text: &str, cx: &mut Context<Self>) -> Self {
        self.search
            .update(cx, |input, cx| input.set_text(text.to_string(), cx));
        self
    }

    fn matches(shown: &BTreeSet<Level>, search: &str, row: &LogRow) -> bool {
        shown.contains(&chip_of(row.level))
            && (search.is_empty()
                || row.text.to_lowercase().contains(search)
                || row
                    .source
                    .as_ref()
                    .is_some_and(|s| s.to_lowercase().contains(search)))
    }

    /// The indices of the rows that pass the filter and search.
    fn visible(&self, cx: &gpui::App) -> Vec<usize> {
        let instances = self.instances.read(cx);
        let Some(log) = instances
            .selected()
            .and_then(|instance| instances.log(&instance.id))
        else {
            return Vec::new();
        };
        let search = self.search.read(cx).text().trim().to_lowercase();
        log.rows()
            .iter()
            .enumerate()
            .filter(|(_, row)| Self::matches(&self.shown, &search, row))
            .map(|(index, _)| index)
            .collect()
    }

    fn toggle(&mut self, level: Level, cx: &mut Context<Self>) {
        if !self.shown.remove(&level) {
            self.shown.insert(level);
        }
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let search = self.search.read(cx).text().trim().to_lowercase();
        let instances = self.instances.read(cx);
        let text = instances
            .selected()
            .and_then(|instance| instances.log(&instance.id))
            .map(|log| log.to_text(|row| Self::matches(&self.shown, &search, row)))
            .unwrap_or_default();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = true;
        cx.notify();
    }

    /// Selects from the middle of the third row shown to the middle of the seventh,
    /// for the offscreen screenshots.
    pub fn preview_select(&mut self, cx: &mut Context<Self>) {
        use crate::components::log_view::{Caret, Selection};
        let selection = self.with_shown(cx, |shown| {
            (shown.line_count() >= 7).then(|| {
                Selection::new(
                    Caret::new(shown.line_id(2), 6),
                    Caret::new(shown.line_id(6), 12),
                )
            })
        });
        self.selection.set(selection.flatten());
        cx.notify();
    }

    /// Runs `f` on the rows shown, for the selection.
    fn with_shown<R>(&self, cx: &gpui::App, f: impl FnOnce(&Shown) -> R) -> Option<R> {
        let visible = self.visible(cx);
        let instances = self.instances.read(cx);
        let log = instances
            .selected()
            .and_then(|instance| instances.log(&instance.id))?;
        Some(f(&Shown {
            log,
            visible: &visible,
        }))
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let selection = self.selection.clone();
        self.with_shown(cx, |shown| selection.select_all(shown));
        cx.notify();
    }

    fn copy_selection(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .with_shown(cx, |shown| self.selection.text(shown))
            .flatten();
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(16.));
        if delta.y > px(0.) {
            // Scrolling up, towards older rows.
            self.follow = false;
        } else if self.scroll.is_scrolled_to_end() == Some(true) {
            self.follow = true;
        }
        cx.notify();
    }

    fn jump_to_end(&mut self, cx: &mut Context<Self>) {
        self.follow = true;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }
}

fn row_element(
    index: usize,
    row: &LogRow,
    selection: &LogSelection,
    id: u64,
) -> gpui::Stateful<gpui::Div> {
    let color = level_color(row.level);
    div()
        .id(("log-row", index))
        .flex()
        .flex_row()
        .gap_2()
        .h(px(20.))
        .px_3()
        .items_center()
        .whitespace_nowrap()
        .font_family(fonts::mono())
        .text_size(theme::text_size(text::SMALL))
        .hover(|style| style.bg(colors::GRAY_850))
        .child(
            div()
                .w(px(58.))
                .flex_none()
                .text_color(colors::GRAY_550)
                .children(row.time.clone()),
        )
        .child(
            div()
                .w(px(46.))
                .flex_none()
                .font_weight(weights::BOLD.into())
                .text_color(color)
                .when(row.source.is_some(), |this| this.child(row.level.label())),
        )
        .when_some(row.source.clone(), |this, source| {
            this.child(
                div()
                    .flex_none()
                    .max_w(px(160.))
                    .truncate()
                    .text_color(if row.stream == Stream::Launcher {
                        crate::theme::accent::light()
                    } else {
                        colors::GRAY_500
                    })
                    .child(source),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(match row.level {
                    Level::Info | Level::Trace | Level::Debug => {
                        if row.stream == Stream::Launcher {
                            crate::theme::accent::light()
                        } else {
                            colors::GRAY_300
                        }
                    }
                    _ => color,
                })
                .child(LogLine::new(selection, id, row.text.clone())),
        )
}

impl Render for Console {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible(cx);
        let count = visible.len();
        let (selected_id, revision) = {
            let instances = self.instances.read(cx);
            let id = instances.selected().map(|i| i.id.clone());
            let revision = id
                .as_ref()
                .and_then(|id| instances.log(id))
                .map_or(0, |log| log.revision);
            (id, revision)
        };
        if self.seen.0.is_some() && self.seen.0 != selected_id {
            // Another instance's log: its row ids are its own.
            self.selection.clear();
        }
        // Not while selecting: the rows would move under the pointer.
        if self.seen != (selected_id.clone(), revision) && !self.selection.is_dragging() {
            self.seen = (selected_id.clone(), revision);
            self.copied = false;
            if self.follow && count > 0 {
                self.scroll
                    .scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
        }
        let total = selected_id
            .as_ref()
            .and_then(|id| self.instances.read(cx).log(id))
            .map_or(0, |log| log.rows().len());

        let chips = CHIPS.iter().map(|(level, key, label)| {
            let on = self.shown.contains(level);
            let level = *level;
            div()
                .id(SharedString::from(format!("chip-{key}")))
                .accessible(Role::CheckBox, format!("log-level-{key}"), t(*label))
                .aria_toggled(if on { Toggled::True } else { Toggled::False })
                .flex_none()
                .whitespace_nowrap()
                .keyboard_focus()
                .px_2p5()
                .py_1()
                .rounded(radius::MD)
                .border_1()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .cursor_pointer()
                .map(|this| {
                    if on {
                        this.bg(colors::GRAY_700)
                            .border_color(level_color(level).opacity(60))
                            .text_color(level_color(level))
                    } else {
                        this.border_color(colors::GRAY_FADED.opacity(20))
                            .text_color(colors::GRAY_550)
                    }
                })
                .on_press(cx.listener(move |this, _, _, cx| this.toggle(level, cx)))
                .child(t(*label))
        });

        let body = if count == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(colors::GRAY_FADED.opacity(30))
                .font_weight(weights::BOLD.into())
                .px_8()
                .text_center()
                .whitespace_normal()
                .child(t(if total == 0 {
                    M::LogEmpty
                } else {
                    M::NoLinesMatch
                }))
                .into_any_element()
        } else {
            let scroll = self.scroll.clone();
            let list = uniform_list(
                "log-rows",
                count,
                cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                    let instances = this.instances.read(cx);
                    let Some(log) = instances
                        .selected()
                        .and_then(|instance| instances.log(&instance.id))
                    else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|index| {
                            let at = *visible.get(index)?;
                            let row = log.rows().get(at)?;
                            Some(row_element(index, row, &this.selection, log.row_id(at)))
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .py_2();
            log_view::log_view("log-lines", &self.selection, list, move |by, _, _| {
                // Offsets are negative going down.
                let base = &scroll.0.borrow().base_handle;
                let mut offset = base.offset();
                offset.y = (offset.y - by).clamp(-base.max_offset().y, px(0.));
                base.set_offset(offset);
            })
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy_selection))
            .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(260.)).child(self.search.clone()))
                    .children(chips)
                    .child(div().flex_1())
                    .when(!self.follow, |this| {
                        this.child(
                            Button::new("log-follow", t(M::JumpToLatest))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.jump_to_end(cx))),
                        )
                    })
                    .child(
                        Button::new(
                            "log-copy",
                            if self.copied {
                                t(CommonMsg::Copied)
                            } else {
                                t(M::CopyLog)
                            },
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .icon(if self.copied { Icon::Check } else { Icon::Copy })
                        .disabled(count == 0)
                        .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                    ),
            )
            .child(
                div()
                    .id("log-view")
                    .flex_1()
                    .min_h_0()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(crate::theme::ink::border())
                    .bg(colors::GRAY_900)
                    .overflow_hidden()
                    .on_scroll(cx.listener(Self::on_scroll))
                    .child(body),
            )
            .child(
                div()
                    .text_size(theme::text_size(text::CAPTION))
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::GRAY_550)
                    .child(trn!(
                        if self.follow {
                            P::LinesShownFollowing
                        } else {
                            P::LinesShown
                        },
                        total,
                        shown = super::thousands(count as u64)
                    )),
            )
    }
}
