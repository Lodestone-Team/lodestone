//! The Logs tab: the selected instance's console, live.
//!
//! Rows are coloured by level, can be filtered by level and searched, and copied.
//! The list follows new output until the user scrolls up, and follows again once
//! they scroll back to the bottom. It keeps the last 10,000 rows.

use std::collections::BTreeSet;

use gpui::{
    ClipboardItem, Context, Entity, IntoElement, Render, ScrollEvent, ScrollStrategy, SharedString,
    Subscription, UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};
use launcher_core::game::log4j::{Level, Stream};

use super::{Instances, LogRow};
use crate::components::KeyboardFocus;
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, TextInput},
    theme::{self, Color, colors, fonts, radius, text, weights},
};

/// The levels the filter chips toggle. TRACE goes with DEBUG, FATAL with ERROR.
const CHIPS: [(Level, &str); 4] = [
    (Level::Debug, "Debug"),
    (Level::Info, "Info"),
    (Level::Warn, "Warn"),
    (Level::Error, "Error"),
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
    _observe: Vec<Subscription>,
}

impl Console {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new(cx).with_placeholder("Search the log"));
        let observe = vec![
            cx.observe(&instances, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];
        Self {
            instances,
            search,
            shown: CHIPS.iter().map(|(level, _)| *level).collect(),
            scroll: UniformListScrollHandle::new(),
            follow: true,
            seen: (None, 0),
            copied: false,
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

fn row_element(index: usize, row: &LogRow) -> gpui::Stateful<gpui::Div> {
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
        .font_family(fonts::MONO)
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
                        colors::BLUE_200
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
                            colors::BLUE_200
                        } else {
                            colors::GRAY_300
                        }
                    }
                    _ => color,
                })
                .child(row.text.clone()),
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
        if self.seen != (selected_id.clone(), revision) {
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

        let chips = CHIPS.iter().map(|(level, label)| {
            let on = self.shown.contains(level);
            let level = *level;
            div()
                .id(SharedString::from(format!("chip-{label}")))
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
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(level, cx)))
                .child(*label)
        });

        let body = if count == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(colors::GRAY_FADED.opacity(30))
                .font_weight(weights::BOLD.into())
                .child(if total == 0 {
                    "The game's output appears here when you press Play."
                } else {
                    "No lines match the filter."
                })
                .into_any_element()
        } else {
            uniform_list(
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
                            let row = log.rows().get(*visible.get(index)?)?;
                            Some(row_element(index, row))
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .py_2()
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
                            Button::new("log-follow", "Jump to latest")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.jump_to_end(cx))),
                        )
                    })
                    .child(
                        Button::new("log-copy", if self.copied { "Copied" } else { "Copy log" })
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
                    .border_color(colors::FADE_700)
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
                    .child(format!(
                        "{} of {} lines{}",
                        super::thousands(count as u64),
                        super::thousands(total as u64),
                        if self.follow { " · following" } else { "" }
                    )),
            )
    }
}
