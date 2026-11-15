//! The Logs tab: the selected instance's console, live, and the logs and crash
//! reports its earlier sessions left on disk, picked from the list beside the search
//! (`launcher_core::game::past_logs`).
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
    Subscription, Task, UniformListScrollHandle, Window, div, prelude::*, uniform_list,
};
use launcher_core::game::{
    log4j::{Level, Stream},
    past_logs::{self, Kind, LogFile},
};

use super::{GameLog, Instances, LogRow};
use crate::components::{
    CopySelection, KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
    log_view::{self, LineSource, LogLine, LogSelection, SelectAll},
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dropdown, Icon, ListRow, Scrollbar, TextInput,
        settings::{popover_panel, select_button},
    },
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, ShellMsg, format, t, tr, trn},
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

/// Presses a level's chip. With every level shown (as at first), it narrows the
/// log to that level alone; after that, pressing others adds them, and pressing a
/// shown one takes it away. Taking away the last one shows every level again.
fn pick(shown: &mut BTreeSet<Level>, level: Level) {
    if shown.len() == CHIPS.len() {
        shown.clear();
        shown.insert(level);
    } else if !shown.remove(&level) {
        shown.insert(level);
    }
    if shown.is_empty() {
        shown.extend(CHIPS.iter().map(|(level, _, _)| *level));
    }
}

pub fn level_color(level: Level) -> Color {
    match level {
        Level::Trace | Level::Debug => crate::theme::ink::muted(),
        Level::Info => colors::GRAY_300,
        Level::Warn => colors::YELLOW,
        Level::Error | Level::Fatal => colors::RED_200,
    }
}

/// A log or crash report from disk, shown in place of the live console.
struct Past {
    file: LogFile,
    log: GameLog,
    /// The earlier lines of a very long file, which aren't shown.
    skipped: usize,
}

/// What a kind of file is called in the list.
fn kind_label(kind: Kind) -> M {
    match kind {
        Kind::Log => M::LogKindLog,
        Kind::CrashReport => M::LogKindCrash,
        Kind::JvmCrash => M::LogKindJvm,
    }
}

/// A file's line as a console row.
fn past_row(line: past_logs::Line) -> LogRow {
    LogRow {
        level: line.level,
        time: line.time.map(SharedString::from),
        source: line.source.map(SharedString::from),
        text: SharedString::from(line.text.replace('\t', "    ")),
        stream: Stream::Stdout,
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
    /// The selected instance's logs and crash reports on disk, listed when the list
    /// opens, and the instance they are of.
    files: Vec<LogFile>,
    files_for: Option<String>,
    menu_open: bool,
    /// The file shown instead of the live console.
    past: Option<Past>,
    /// The name of the file being read.
    loading: Option<String>,
    /// Why the last file couldn't be read.
    error: Option<String>,
    _load: Option<Task<()>>,
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
            files: Vec::new(),
            files_for: None,
            menu_open: false,
            past: None,
            loading: None,
            error: None,
            _load: None,
            _observe: observe,
        }
    }

    /// The log shown: a file picked from the list, else the selected instance's
    /// console.
    fn log<'a>(&'a self, cx: &'a gpui::App) -> Option<&'a GameLog> {
        if let Some(past) = &self.past {
            return Some(&past.log);
        }
        let instances = self.instances.read(cx);
        instances
            .selected()
            .and_then(|instance| instances.log(&instance.id))
    }

    fn selected_id(&self, cx: &gpui::App) -> Option<String> {
        self.instances
            .read(cx)
            .selected()
            .map(|instance| instance.id.clone())
    }

    /// Opens or closes the list of files, looking on disk again as it opens.
    fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        self.menu_open = !self.menu_open;
        if self.menu_open {
            self.files = match self.selected_id(cx) {
                Some(id) => past_logs::list(&self.instances.read(cx).game_dir(&id)),
                None => Vec::new(),
            };
        }
        cx.notify();
    }

    /// Back to the live console.
    fn show_live(&mut self, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.past = None;
        self.loading = None;
        self.error = None;
        self._load = None;
        self.follow = true;
        self.selection.clear();
        cx.notify();
    }

    /// Reads a log or crash report and shows it.
    fn open_file(&mut self, file: LogFile, cx: &mut Context<Self>) {
        let Some(instance) = self.selected_id(cx) else {
            return;
        };
        self.menu_open = false;
        self.error = None;
        self.loading = Some(file.name.clone());
        self.selection.clear();
        self._load = Some(cx.spawn(async move |this, cx| {
            let read = cx
                .background_spawn({
                    let file = file.clone();
                    async move { past_logs::read(&file) }
                })
                .await;
            this.update(cx, |this, cx| {
                // another instance was selected while the file was read
                if this.selected_id(cx).as_ref() != Some(&instance) {
                    return;
                }
                this.loading = None;
                match read {
                    Ok(contents) => this.show_past(file, contents),
                    Err(error) => {
                        this.past = None;
                        this.error = Some(tr!(M::LogReadFailed, name = file.name, error = error));
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn show_past(&mut self, file: LogFile, contents: past_logs::Contents) {
        self.past = Some(Past {
            file,
            skipped: contents.skipped,
            log: GameLog::from_rows(contents.lines.into_iter().map(past_row)),
        });
    }

    /// Shows `text` as the file `name`, with `files` in the list (open or not), for
    /// the offscreen screenshots.
    pub fn preview_past(
        &mut self,
        files: Vec<LogFile>,
        shown: usize,
        text: &str,
        menu_open: bool,
        cx: &mut Context<Self>,
    ) {
        let (Some(instance), Some(file)) = (self.selected_id(cx), files.get(shown).cloned()) else {
            return;
        };
        let contents = past_logs::parse(text, file.kind);
        self.files_for = Some(instance);
        self.files = files;
        self.menu_open = menu_open;
        self.show_past(file, contents);
        cx.notify();
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
        let Some(log) = self.log(cx) else {
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
        pick(&mut self.shown, level);
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let search = self.search.read(cx).text().trim().to_lowercase();
        let text = self
            .log(cx)
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
        let log = self.log(cx)?;
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
    plain: bool,
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
        // a crash report has no times or levels to line up
        .when(!plain, |this| {
            this.child(
                div()
                    .w(px(58.))
                    .flex_none()
                    .text_color(crate::theme::ink::faint())
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
        })
        .when_some(row.source.clone(), |this, source| {
            this.child(
                div()
                    .flex_none()
                    .max_w(px(160.))
                    .truncate()
                    .text_color(if row.stream == Stream::Launcher {
                        crate::theme::accent::light()
                    } else {
                        crate::theme::ink::muted()
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
        let instance_id = self.selected_id(cx);
        // The files, and the one shown, are the instance's they were listed for.
        if self.files_for != instance_id {
            self.files_for = instance_id.clone();
            self.files.clear();
            self.menu_open = false;
            self.past = None;
            self.loading = None;
            self.error = None;
            self._load = None;
        }
        let visible = self.visible(cx);
        let count = visible.len();
        // What is shown: the instance, and the file when one is picked.
        let selected_id = match (&instance_id, &self.past) {
            (Some(id), Some(past)) => Some(format!("{id}\n{}", past.file.name)),
            _ => instance_id.clone(),
        };
        let revision = self.log(cx).map_or(0, |log| log.revision);
        if self.seen.0.is_some() && self.seen.0 != selected_id {
            // Another instance's log, or another file: its row ids are its own.
            self.selection.clear();
        }
        // Not while selecting: the rows would move under the pointer.
        if self.seen != (selected_id.clone(), revision) && !self.selection.is_dragging() {
            self.seen = (selected_id.clone(), revision);
            self.copied = false;
            if self.past.is_some() {
                // a file is read from its start
                self.scroll.scroll_to_item(0, ScrollStrategy::Top);
            } else if self.follow && count > 0 {
                self.scroll
                    .scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
        }
        let total = self.log(cx).map_or(0, |log| log.rows().len());
        let live = self.past.is_none();
        let skipped = self.past.as_ref().map_or(0, |past| past.skipped);

        // The list of what to show: this session, then the files on disk.
        let current: SharedString = match &self.past {
            Some(past) => past.file.name.clone().into(),
            None => t(M::LogThisSession).into(),
        };
        let menu = Dropdown::new(
            select_button(
                "log-source",
                div()
                    .max_w(px(220.))
                    .truncate()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .child(current.clone()),
            )
            .aria_label(t(M::LogSource))
            .aria_value(current)
            .expandable(self.menu_open, {
                let view = cx.entity().downgrade();
                move |_: &mut Window, cx: &mut gpui::App| {
                    view.update(cx, |this, cx| this.toggle_menu(cx)).ok();
                }
            })
            .on_press(cx.listener(|this, _, _, cx| this.toggle_menu(cx))),
        )
        .when(self.menu_open, |this| {
            let close = cx.listener(|this, _: &(), _, cx| {
                this.menu_open = false;
                cx.notify();
            });
            let mut panel = popover_panel("log-source-list")
                .w(px(360.))
                .max_h(px(320.))
                .overflow_y_scroll()
                .child(
                    ListRow::new("log-source-live", t(M::LogThisSession))
                        .selected(live)
                        .on_click(cx.listener(|this, _, _, cx| this.show_live(cx))),
                );
            if self.files.is_empty() {
                panel = panel.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::faint())
                        .child(t(M::LogNoFiles)),
                );
            }
            for (i, file) in self.files.iter().enumerate() {
                let shown = self
                    .past
                    .as_ref()
                    .is_some_and(|past| past.file.path == file.path);
                let pick = file.clone();
                panel = panel.child(
                    ListRow::new(("log-source-file", i), file.name.clone())
                        .detail(format!(
                            "{} · {}",
                            t(kind_label(file.kind)),
                            format::bytes(file.size)
                        ))
                        .selected(shown)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.open_file(pick.clone(), cx)),
                        ),
                );
            }
            this.popover("log-source-popover", panel, move |window, cx| {
                close(&(), window, cx)
            })
        });

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
                            .text_color(crate::theme::ink::faint())
                    }
                })
                .on_press(cx.listener(move |this, _, _, cx| this.toggle(level, cx)))
                .child(t(*label))
        });

        let notice = match (&self.loading, &self.error) {
            (Some(name), _) => Some(tr!(M::LogReading, name = name)),
            (None, Some(error)) => Some(error.clone()),
            (None, None) => None,
        };
        let body = if let Some(notice) = notice {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(crate::theme::ink::faint())
                .font_weight(weights::BOLD.into())
                .px_8()
                .text_center()
                .whitespace_normal()
                .child(notice)
                .into_any_element()
        } else if count == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(crate::theme::ink::faint())
                .font_weight(weights::BOLD.into())
                .px_8()
                .text_center()
                .whitespace_normal()
                .child(t(if total == 0 && !live {
                    M::LogFileEmpty
                } else if total == 0 {
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
                    let Some(log) = this.log(cx) else {
                        return Vec::new();
                    };
                    let plain = this
                        .past
                        .as_ref()
                        .is_some_and(|past| past.file.kind != Kind::Log);
                    range
                        .filter_map(|index| {
                            let at = *visible.get(index)?;
                            let row = log.rows().get(at)?;
                            Some(row_element(
                                index,
                                row,
                                &this.selection,
                                log.row_id(at),
                                plain,
                            ))
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
            .relative()
            .child({
                let console = cx.entity().downgrade();
                Scrollbar::new("log-scrollbar", self.scroll.clone())
                    .label(t(ShellMsg::TabLogs))
                    .on_scroll(move |_, cx| {
                        // As the wheel: up stops following, the bottom follows again.
                        console
                            .update(cx, |this, cx| {
                                this.follow = this.scroll.is_scrolled_to_end() != Some(false);
                                cx.notify();
                            })
                            .ok();
                    })
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
                    .child(menu)
                    .child(div().w(px(220.)).child(self.search.clone()))
                    .children(chips)
                    .child(div().flex_1())
                    .when(!self.follow && live, |this| {
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
                    .text_color(crate::theme::ink::faint())
                    .child({
                        let lines = trn!(
                            if self.follow && live {
                                P::LinesShownFollowing
                            } else {
                                P::LinesShown
                            },
                            total,
                            shown = super::thousands(count as u64)
                        );
                        if skipped > 0 {
                            format!("{lines} · {}", trn!(P::LogLinesSkipped, skipped))
                        } else {
                            lines
                        }
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> BTreeSet<Level> {
        CHIPS.iter().map(|(level, _, _)| *level).collect()
    }

    #[test]
    fn first_press_narrows_to_that_level() {
        let mut shown = all();
        pick(&mut shown, Level::Warn);
        assert_eq!(shown, BTreeSet::from([Level::Warn]));
    }

    #[test]
    fn later_presses_add_and_remove() {
        let mut shown = all();
        pick(&mut shown, Level::Warn);
        pick(&mut shown, Level::Error);
        assert_eq!(shown, BTreeSet::from([Level::Warn, Level::Error]));
        pick(&mut shown, Level::Warn);
        assert_eq!(shown, BTreeSet::from([Level::Error]));
    }

    #[test]
    fn removing_the_last_level_shows_all_again() {
        let mut shown = all();
        pick(&mut shown, Level::Info);
        pick(&mut shown, Level::Info);
        assert_eq!(shown, all());
    }
}
