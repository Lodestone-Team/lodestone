//! An instance's Console tab (`components/GameConsole.tsx`, `data/ConsoleStream.ts`,
//! `components/ConsoleLine.tsx`): the core's console history and the live console,
//! merged in order, with the dashboard's colours; a search (plain or regex); the
//! list follows new lines until scrolled up; a command line with ↑/↓ history.
//!
//! The list is virtualized, so it shows all 5,000 lines kept (the dashboard drew
//! only the newest 2,000, for the browser's sake). It selects like one big text
//! field (`lodestone_ui::components::log_view`): drag across lines, ⌘A selects the
//! lines shown, ⌘C copies.

use std::collections::{HashMap, VecDeque};

use gpui::{
    AnyElement, App, Context, Entity, FollowMode, Global, IntoElement, KeyBinding, ListAlignment,
    ListState, ParentElement, Pixels, Render, SharedString, Styled, StyledText, Subscription, Task,
    Window, actions, div, list, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        InstanceState,
        console::{CommandHistory, ConsoleLine, LinkedPlayers, MAX_CONSOLE_LINES},
        models::InstancePermission,
    },
    live::{LiveMessage, LiveStream, StreamKind},
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, Checkbox, CopySelection, Icon, Scrollbar, TextInput, Toast,
        a11y::{Accessible, Role},
        console_line::{ParsedConsoleLine, console_text, parse_console_line},
        log_view::{self, LineSource, LogSelection, SelectAll},
        text_tooltip,
    },
    theme::{Color, colors, fonts, ink, px, radius, text, weights},
};
use regex::{Regex, RegexBuilder};

use crate::{
    i18n::{ConsoleFilesMsg as M, t, tr},
    state::HostState,
};

actions!(host_console, [OlderCommand, NewerCommand, SendCommand]);

/// The command line's key context: ↑ and ↓ go through the history, Enter sends.
const COMMAND_CONTEXT: &str = "ConsoleCommand > TextInput";

pub fn init(cx: &mut App) {
    // Bound after the text field's own Up and Down (to the start and end), so these win.
    cx.bind_keys([
        KeyBinding::new("up", OlderCommand, Some(COMMAND_CONTEXT)),
        KeyBinding::new("down", NewerCommand, Some(COMMAND_CONTEXT)),
        KeyBinding::new("enter", SendCommand, Some(COMMAND_CONTEXT)),
    ]);
}

/// Each instance's sent commands, while the Host runs.
#[derive(Default)]
struct CommandHistories(HashMap<String, CommandHistory>);

impl Global for CommandHistories {}

/// Whether consoles show colours: Settings › Profile › Coloured console (on by
/// default).
pub fn colored(cx: &App) -> bool {
    cx.try_global::<crate::prefs::Prefs>()
        .is_none_or(|prefs| prefs.data.colored_console)
}

/// A console line, parsed once.
pub struct LogLine {
    pub snowflake: i64,
    pub parsed: ParsedConsoleLine,
    /// The Lodestone user of the linked player the line is about.
    pub linked: Option<SharedString>,
}

/// What the search matches.
enum Filter {
    All,
    /// Case-insensitive, on the plain text.
    Text(String),
    /// Case-insensitive; an invalid expression matches nothing.
    Regex(Option<Regex>),
}

impl Filter {
    fn new(text: &str, regex: bool) -> Self {
        if text.is_empty() {
            Filter::All
        } else if regex {
            Filter::Regex(
                RegexBuilder::new(text)
                    .case_insensitive(true)
                    .size_limit(1 << 20)
                    .build()
                    .ok(),
            )
        } else {
            Filter::Text(text.to_lowercase())
        }
    }

    fn matches(&self, plain: &str) -> bool {
        match self {
            Filter::All => true,
            Filter::Text(needle) => plain.to_lowercase().contains(needle),
            Filter::Regex(regex) => regex.as_ref().is_some_and(|r| r.is_match(plain)),
        }
    }
}

/// How the shown lines changed, for the list.
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    /// `removed` lines went from the start and `added` came at the end.
    Appended { removed: usize, added: usize },
    /// Lines came in between: the list is redrawn.
    Rebuilt,
}

/// The console's lines, in snowflake order without repeats, at most
/// [`MAX_CONSOLE_LINES`], and those the search shows.
pub struct ConsoleLog {
    lines: VecDeque<LogLine>,
    /// How many lines were ever dropped from the start: a line's number is
    /// `dropped + its index`.
    dropped: u64,
    filter: Filter,
    /// The numbers of the lines shown.
    shown: Vec<u64>,
}

impl Default for ConsoleLog {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            dropped: 0,
            filter: Filter::All,
            shown: Vec::new(),
        }
    }
}

impl ConsoleLog {
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn shown_len(&self) -> usize {
        self.shown.len()
    }

    /// The `ix`th line shown.
    pub fn shown(&self, ix: usize) -> Option<&LogLine> {
        let number = *self.shown.get(ix)?;
        self.lines.get((number - self.dropped) as usize)
    }

    fn line(snowflake: i64, message: &str, linked: &LinkedPlayers) -> LogLine {
        let parsed = parse_console_line(message);
        let linked = linked
            .in_line(&parsed.plain)
            .map(|(_, user)| SharedString::from(user.to_string()));
        LogLine {
            snowflake,
            parsed,
            linked,
        }
    }

    /// Adds lines from the history or the live console.
    pub fn merge(&mut self, mut new: Vec<ConsoleLine>, linked: &LinkedPlayers) -> Change {
        new.sort_by_key(|line| line.snowflake);
        new.dedup_by_key(|line| line.snowflake);
        let Some(first) = new.first() else {
            return Change::Appended {
                removed: 0,
                added: 0,
            };
        };
        let appends = self
            .lines
            .back()
            .is_none_or(|last| first.snowflake > last.snowflake);
        if appends {
            let before = self.shown.len();
            for line in new {
                let line = Self::line(line.snowflake, &line.message, linked);
                let number = self.dropped + self.lines.len() as u64;
                if self.filter.matches(&line.parsed.plain) {
                    self.shown.push(number);
                }
                self.lines.push_back(line);
            }
            let added = self.shown.len() - before;
            let removed = self.trim();
            if removed > before {
                // more came than is kept
                return Change::Rebuilt;
            }
            Change::Appended { removed, added }
        } else {
            for line in new {
                match self
                    .lines
                    .binary_search_by_key(&line.snowflake, |l| l.snowflake)
                {
                    Ok(_) => {}
                    Err(at) => self
                        .lines
                        .insert(at, Self::line(line.snowflake, &line.message, linked)),
                }
            }
            self.trim();
            self.refilter();
            Change::Rebuilt
        }
    }

    /// Drops the oldest lines past the limit; returns how many shown lines went.
    fn trim(&mut self) -> usize {
        let excess = self.lines.len().saturating_sub(MAX_CONSOLE_LINES);
        if excess == 0 {
            return 0;
        }
        self.lines.drain(..excess);
        self.dropped += excess as u64;
        let gone = self
            .shown
            .iter()
            .take_while(|&&number| number < self.dropped)
            .count();
        self.shown.drain(..gone);
        gone
    }

    /// Shows the lines that match `text` (a regular expression if `regex`).
    pub fn set_filter(&mut self, text: &str, regex: bool) {
        self.filter = Filter::new(text, regex);
        self.refilter();
    }

    fn refilter(&mut self) {
        let dropped = self.dropped;
        self.shown = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| self.filter.matches(&line.parsed.plain))
            .map(|(ix, _)| dropped + ix as u64)
            .collect();
    }

    /// Notes the linked players again, when they arrive after the lines.
    fn relink(&mut self, linked: &LinkedPlayers) {
        for line in &mut self.lines {
            line.linked = linked
                .in_line(&line.parsed.plain)
                .map(|(_, user)| SharedString::from(user.to_string()));
        }
    }
}

/// The lines shown, for the selection. Their ids are their snowflakes, which grow
/// down the list as the lines are kept in snowflake order.
impl LineSource for ConsoleLog {
    fn line_count(&self) -> usize {
        self.shown_len()
    }

    fn line_id(&self, ix: usize) -> u64 {
        self.shown(ix).map_or(0, |line| line.snowflake as u64)
    }

    fn line_text(&self, ix: usize) -> &str {
        self.shown(ix).map_or("", |line| line.parsed.plain.as_str())
    }
}

/// The console's connection, for the status dot (`ConsoleStreamStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamStatus {
    NoPermission,
    Loading,
    /// The history, but no live console.
    Buffered,
    Live,
    /// Live, but the history couldn't be fetched.
    LiveNoBuffer,
    /// The core closed the console for good.
    Closed,
    /// The connection dropped; it is tried again.
    Error,
}

impl StreamStatus {
    fn message(self) -> M {
        match self {
            StreamStatus::NoPermission => M::ConsoleStatusNoPermission,
            StreamStatus::Loading => M::ConsoleStatusLoading,
            StreamStatus::Buffered => M::ConsoleStatusBuffered,
            StreamStatus::Live => M::ConsoleStatusLive,
            StreamStatus::LiveNoBuffer => M::ConsoleStatusLiveNoBuffer,
            StreamStatus::Closed => M::ConsoleStatusClosed,
            StreamStatus::Error => M::ConsoleStatusError,
        }
    }

    fn color(self) -> Color {
        match self {
            StreamStatus::Live => colors::GREEN_200,
            StreamStatus::LiveNoBuffer => colors::YELLOW,
            StreamStatus::Closed | StreamStatus::Error => colors::RED_200,
            _ => colors::GRAY_500,
        }
    }
}

/// Where the history is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Buffer {
    Pending,
    Fetched,
    Failed,
}

pub struct ConsolePage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    pub log: ConsoleLog,
    pub status: StreamStatus,
    buffer: Buffer,
    socket_open: bool,
    /// Whether the stream has been connected before (a new connection is a reconnect).
    connected_once: bool,
    can_read: bool,
    linked: LinkedPlayers,
    list: ListState,
    /// About how tall a line is, for the scroll bar: the list counts lines it hasn't
    /// drawn yet as this tall until it draws them.
    row_hint: Pixels,
    selection: LogSelection,
    /// Following new lines is paused while a press selects.
    follow_paused: bool,
    search: Entity<TextInput>,
    regex: bool,
    command: Entity<TextInput>,
    /// The text the history put in the command line: typing something else stops
    /// browsing the history.
    from_history: Option<String>,
    stream: Option<LiveStream>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ConsolePage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::ConsoleSearch)
                .with_automation_id("console-search")
                .with_leading_icon(Icon::MagnifyingGlass)
                .with_clear_button()
        });
        let command = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::ConsoleInputPlaceholder)
                .with_label(M::ConsoleCommand)
                .with_automation_id("console-command")
        });
        let list = ListState::new(0, ListAlignment::Top, px(400.));
        list.set_follow_mode(FollowMode::Tail);
        let page = cx.entity().downgrade();
        list.set_scroll_handler(move |_, _, cx| {
            page.update(cx, |_, cx| cx.notify()).ok();
        });
        let selection = LogSelection::new(cx);
        {
            let page = cx.entity().downgrade();
            selection.on_drag(move |dragging, _, cx| {
                page.update(cx, |this, cx| this.selecting(dragging, cx))
                    .ok();
            });
        }
        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.state_changed(cx)),
            cx.observe(&search, |this, _, cx| this.refilter(cx)),
            cx.observe(&command, |this, command, cx| {
                let text = command.read(cx).text().to_string();
                if this.from_history.as_ref().is_some_and(|from| *from != text) {
                    this.from_history = None;
                    this.history(cx, CommandHistory::reset);
                }
            }),
        ];
        let mut this = Self {
            state,
            uuid,
            log: ConsoleLog::default(),
            status: StreamStatus::Loading,
            buffer: Buffer::Pending,
            socket_open: false,
            connected_once: false,
            can_read: false,
            linked: LinkedPlayers::default(),
            list,
            row_hint: px(24.),
            selection,
            follow_paused: false,
            search,
            regex: true,
            command,
            from_history: None,
            stream: None,
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.state_changed(cx);
        this.fetch_linked(cx);
        this
    }

    fn may_read(&self, cx: &App) -> bool {
        self.state
            .read(cx)
            .me
            .may(InstancePermission::Console, &self.uuid)
    }

    /// Starts or stops following the console when the permission changes, and
    /// updates the command line.
    fn state_changed(&mut self, cx: &mut Context<Self>) {
        let can_read = self.may_read(cx);
        if can_read != self.can_read {
            self.can_read = can_read;
            if can_read {
                self.follow(cx);
            } else {
                self.stream = None;
                self.status = StreamStatus::NoPermission;
            }
        }
        self.update_command_line(cx);
        cx.notify();
    }

    /// Fetches the history and opens the live console.
    fn follow(&mut self, cx: &mut Context<Self>) {
        self.status = StreamStatus::Loading;
        self.buffer = Buffer::Pending;
        self.socket_open = false;
        self.connected_once = false;
        self.fetch_buffer(cx);
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let admin = self.state.read(cx).admin.clone();
        self.stream = Some(LiveStream::open(
            admin.core(),
            admin.token().clone(),
            &self.uuid,
            StreamKind::Console,
            tx,
        ));
        let task = cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(message) = rx.next().await {
                if this
                    .update(cx, |this, cx| this.on_message(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        self._tasks.push(task);
    }

    fn fetch_buffer(&mut self, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.console_buffer(&uuid).await })
                .await;
            this.update(cx, |this, cx| this.on_buffer(result, cx)).ok();
        });
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    fn fetch_linked(&mut self, cx: &mut Context<Self>) {
        let (admin, me) = {
            let state = self.state.read(cx);
            (state.admin.clone(), state.me.clone())
        };
        let task = cx.spawn(async move |this, cx| {
            let linked = cx
                .background_spawn(
                    async move { admin.linked_players(me.is_owner, &me.username).await },
                )
                .await;
            if let Ok(linked) = linked {
                let linked = LinkedPlayers::from_links(linked);
                this.update(cx, |this, cx| {
                    this.log.relink(&linked);
                    this.linked = linked;
                    cx.notify();
                })
                .ok();
            }
        });
        self._tasks.push(task);
    }

    /// The history arrived (or failed).
    pub fn on_buffer(
        &mut self,
        result: Result<Vec<ConsoleLine>, CoreError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(lines) => {
                self.buffer = Buffer::Fetched;
                self.add(lines, cx);
            }
            Err(CoreError::SignedOut) => {
                self.buffer = Buffer::Failed;
            }
            Err(_) => self.buffer = Buffer::Failed,
        }
        // the socket may have dropped meanwhile; that status stays
        if self.socket_open || self.status == StreamStatus::Loading {
            self.update_status();
        }
        cx.notify();
    }

    /// A message from the live console.
    pub fn on_message(&mut self, message: LiveMessage, cx: &mut Context<Self>) {
        match message {
            LiveMessage::Connected => {
                self.socket_open = true;
                if self.connected_once {
                    // fill in the lines sent while disconnected
                    self.buffer = Buffer::Pending;
                    self.fetch_buffer(cx);
                }
                self.connected_once = true;
                self.update_status();
            }
            LiveMessage::Disconnected(_) => {
                self.socket_open = false;
                self.status = StreamStatus::Error;
            }
            LiveMessage::Ended(CoreError::Forbidden(_)) => {
                self.socket_open = false;
                self.status = StreamStatus::NoPermission;
            }
            LiveMessage::Ended(_) => {
                self.socket_open = false;
                self.status = StreamStatus::Closed;
            }
            LiveMessage::Output(line) => self.add(vec![line], cx),
            _ => {}
        }
        self.update_command_line(cx);
        cx.notify();
    }

    /// The status from the history and the connection, as `ConsoleStream.ts` decides
    /// it: it moves on only once the history has been fetched (or failed).
    fn update_status(&mut self) {
        match (self.buffer, self.socket_open) {
            (Buffer::Pending, _) => self.status = StreamStatus::Loading,
            (Buffer::Fetched, true) => self.status = StreamStatus::Live,
            (Buffer::Failed, true) => self.status = StreamStatus::LiveNoBuffer,
            (Buffer::Fetched, false) => self.status = StreamStatus::Buffered,
            // failed and not connected yet: stays as it is until the socket opens
            (Buffer::Failed, false) => {}
        }
    }

    fn add(&mut self, lines: Vec<ConsoleLine>, cx: &mut Context<Self>) {
        let first = self.list.item_count() == 0;
        match self.log.merge(lines, &self.linked) {
            Change::Appended { added: 0, .. } if first => {}
            // New lines after the last, which the list draws as they come when it
            // follows. The first lines (the history) are laid out all at once below.
            Change::Appended { removed, added } if !first => {
                if removed > 0 {
                    self.list.splice(0..removed, 0);
                }
                if added > 0 {
                    let at = self.log.shown_len() - added;
                    self.list.splice(at..at, added);
                }
            }
            Change::Appended { .. } | Change::Rebuilt => {
                let following = self.list.is_following_tail();
                self.reset_list();
                if following {
                    self.list.set_follow_mode(FollowMode::Tail);
                }
            }
        }
        cx.notify();
    }

    /// A press starts or stops selecting: the list stops following new lines
    /// meanwhile, so they don't move the text under the pointer, and follows again
    /// after if it is still at the bottom.
    fn selecting(&mut self, dragging: bool, cx: &mut Context<Self>) {
        if dragging {
            if self.list.is_following_tail() {
                self.list.set_follow_mode(FollowMode::Normal);
                self.follow_paused = true;
            }
        } else if std::mem::take(&mut self.follow_paused) {
            let top = self.list.logical_scroll_top();
            self.list.set_follow_mode(FollowMode::Tail);
            // Back where it was: following resumes once it shows the bottom.
            self.list.scroll_to(top);
        }
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selection.select_all(&self.log);
        cx.notify();
    }

    fn copy(&mut self, _: &CopySelection, _: &mut Window, cx: &mut Context<Self>) {
        self.selection.copy(&self.log, cx);
    }

    /// Sets the list to the lines shown, each as tall as a line until drawn, so the
    /// scroll bar's thumb is about the right size from the start.
    fn reset_list(&self) {
        self.list
            .reset_with_uniform_height(self.log.shown_len(), self.row_hint);
    }

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let text = self.search.read(cx).text().to_string();
        self.log.set_filter(&text, self.regex);
        self.reset_list();
        self.list.set_follow_mode(FollowMode::Tail);
        cx.notify();
    }

    /// Why commands can't be sent now, if they can't (`consoleInputMessage`).
    fn command_blocked(&self, cx: &App) -> Option<M> {
        let state = self.state.read(cx);
        let instance = state.instance(&self.uuid);
        if !self.can_read || self.status == StreamStatus::NoPermission {
            Some(M::ConsoleInputNoPermission)
        } else if instance.is_some_and(|i| i.is_atom()) && !state.me.is_owner {
            Some(M::ConsoleOwnerOnlyAtom)
        } else if instance.is_some_and(|i| i.state == InstanceState::Stopped) {
            Some(M::ConsoleServerStopped)
        } else if self.status == StreamStatus::Closed {
            Some(M::ConsoleStatusClosed)
        } else {
            None
        }
    }

    fn update_command_line(&mut self, cx: &mut Context<Self>) {
        let blocked = self.command_blocked(cx);
        self.command.update(cx, |command, cx| {
            command.set_disabled(blocked.is_some(), cx);
            command.set_placeholder(blocked.unwrap_or(M::ConsoleInputPlaceholder), cx);
        });
    }

    fn history<R>(&self, cx: &mut App, f: impl FnOnce(&mut CommandHistory) -> R) -> R {
        let histories = cx.default_global::<CommandHistories>();
        f(histories.0.entry(self.uuid.clone()).or_default())
    }

    fn show_command(&mut self, command: Option<String>, cx: &mut Context<Self>) {
        let Some(command) = command else {
            return;
        };
        self.from_history = Some(command.clone());
        self.command
            .update(cx, |input, cx| input.set_text(command, cx));
    }

    fn older_command(&mut self, _: &OlderCommand, _: &mut Window, cx: &mut Context<Self>) {
        let command = self.history(cx, |h| h.older().map(str::to_string));
        self.show_command(command, cx);
    }

    fn newer_command(&mut self, _: &NewerCommand, _: &mut Window, cx: &mut Context<Self>) {
        let command = self.history(cx, |h| h.newer().map(str::to_string));
        self.show_command(command, cx);
    }

    /// Enter: sends the command line, as typed, and remembers it.
    fn send_command(&mut self, _: &SendCommand, _: &mut Window, cx: &mut Context<Self>) {
        if self.command_blocked(cx).is_some() {
            return;
        }
        let command = self.command.read(cx).text().to_string();
        if command.trim().is_empty() {
            return;
        }
        self.history(cx, |h| h.push(command.clone()));
        self.from_history = None;
        self.command.update(cx, |input, cx| input.set_text("", cx));
        self.list.set_follow_mode(FollowMode::Tail);
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let state = self.state.clone();
        let task = cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn(async move { admin.send_console(&uuid, &command).await })
                .await;
            if let Err(err) = result {
                state.update(cx, |state, cx| {
                    state.toast(
                        Toast::error(tr!(M::ConsoleSendFailed, error = err.to_string())),
                        cx,
                    )
                });
            }
        });
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
        cx.notify();
    }

    /// Sample lines in the dashboard's colours, for the development screenshots.
    #[cfg(feature = "screenshot")]
    pub fn demo(&mut self, cx: &mut Context<Self>) {
        self.linked = LinkedPlayers::new([(
            "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
            "Notch".into(),
            "peter".into(),
        )]);
        let messages = [
            "[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.21.1",
            "[12:00:00] [Server thread/INFO]: Loading properties",
            "[12:00:01] [Server thread/WARN]: Can't keep up! Is the server overloaded? Running 2034ms or 40 ticks behind",
            "[12:00:01] [Server thread/ERROR]: Encountered an unexpected exception",
            "\u{1b}[32m[12:00:02 INFO]\u{1b}[0m: \u{1b}[1mDone\u{1b}[0m (3.2s)! For help, type \"help\"",
            "[12:00:03] [Server thread/INFO]: §6Gold §lbold§r and §bcyan§r, §mstruck§r, §nunderlined",
            "\u{1b}[41;97m FATAL \u{1b}[0m \u{1b}[38;5;208m256 colours\u{1b}[0m \u{1b}[38;2;120;200;255m24-bit\u{1b}[0m \u{1b}[2mdim\u{1b}[0m \u{1b}[3mitalic\u{1b}[0m \u{1b}[7minverse\u{1b}[0m",
            "[12:00:04] [Server thread/INFO]: Notch joined the game",
            "[12:00:05] [Server thread/INFO]: <Notch> hello! This is a long chat message that wraps onto the next line when the window is too narrow to show it on one line, as the dashboard's console does.",
            "[12:00:06] [Server thread/INFO]: Alex[/127.0.0.1:50000] logged in with entity id 120 at (0.5, 64.0, 0.5)",
        ];
        // Earlier lines, so the console scrolls.
        let earlier = (0..40).map(|i| ConsoleLine {
            snowflake: 500 + i,
            message: format!(
                "[11:59:{:02}] [Worker-Main-{}/INFO]: Preparing spawn area: {}%",
                i + 10,
                i % 4 + 1,
                i * 5 / 2
            ),
        });
        let lines = earlier
            .chain(messages.iter().enumerate().map(|(i, message)| ConsoleLine {
                snowflake: 1_000 + i as i64,
                message: message.to_string(),
            }))
            .collect();
        self.add(lines, cx);
        self.history(cx, |h| h.push("say hello".into()));
        cx.notify();
    }

    /// Selects from the middle of the third of the last ten lines shown to the middle
    /// of the sixth, for the development screenshots' `--select-lines`.
    #[cfg(feature = "screenshot")]
    pub fn select_for_screenshot(&mut self, cx: &mut Context<Self>) {
        use lodestone_ui::components::log_view::{Caret, Selection};
        if self.log.shown_len() < 6 {
            return;
        }
        let first = self.log.shown_len().saturating_sub(10);
        self.selection.set(Some(Selection::new(
            Caret::new(self.log.line_id(first + 2), 12),
            Caret::new(self.log.line_id(first + 5), 30),
        )));
        cx.notify();
    }

    fn row(&self, ix: usize, cx: &App) -> AnyElement {
        let Some(line) = self.log.shown(ix) else {
            return div().into_any_element();
        };
        let styled = console_text(&line.parsed, colored(cx));
        let text = StyledText::new(styled.text.clone()).with_highlights(styled.highlights);
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_start()
            .px(px(16.))
            .py(px(2.))
            .text_color(styled.color)
            .hover(|style| style.bg(colors::GRAY_800))
            .child(div().min_w_0().child(log_view::LogLine::styled(
                &self.selection,
                line.snowflake as u64,
                styled.text,
                text,
            )))
            .when_some(line.linked.clone(), |this, user| {
                this.child(
                    div()
                        .id(("console-linked", line.snowflake as u64))
                        .flex_none()
                        .tooltip(text_tooltip(t(M::ConsoleLinkedUser)))
                        .ml(px(8.))
                        .px(px(4.))
                        .rounded(radius::DEFAULT)
                        .bg(colors::GRAY_800)
                        .text_color(colors::WHITE.opacity(50))
                        .child(user),
                )
            })
            .into_any_element()
    }
}

/// A large icon with a message, for empty and refused states (the dashboard's
/// `ErrorGraphic`).
pub(crate) fn graphic(
    icon: Icon,
    message: impl Into<SharedString>,
    detail: Option<SharedString>,
) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .p(px(24.))
        .child(icon.render(px(48.), colors::GRAY_400))
        .child(
            div()
                .text_size(text::H3)
                .font_weight(weights::MEDIUM.face())
                .text_color(colors::WHITE.opacity(50))
                .child(message.into()),
        )
        .when_some(detail, |this, detail| {
            this.child(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(colors::GRAY_400)
                    .child(detail),
            )
        })
}

/// Lines of code being loaded (the dashboard's `SkeletonCode`).
pub(crate) fn skeleton_code(lines: usize) -> gpui::Div {
    const WIDTHS: [f32; 6] = [0.62, 0.84, 0.45, 0.72, 0.9, 0.38];
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children((0..lines).map(|i| {
            div()
                .h(px(10.))
                .w(gpui::relative(WIDTHS[i % WIDTHS.len()]))
                .rounded(radius::DEFAULT)
                .bg(colors::GRAY_700)
        }))
}

impl Render for ConsolePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A line of small text (at gpui's 1.618 line height) and the row's padding.
        self.row_hint = text::SMALL.to_pixels(window.rem_size()) * 1.618 + px(4.);
        let stopped = self
            .state
            .read(cx)
            .instance(&self.uuid)
            .is_some_and(|i| i.state == InstanceState::Stopped);
        let (status_text, status_color) = if stopped {
            (t(M::ConsoleServerStopped), colors::GRAY_500)
        } else {
            (t(self.status.message()), self.status.color())
        };
        let no_permission = !self.can_read || self.status == StreamStatus::NoPermission;
        let following = self.list.is_following_tail();
        let this = cx.entity().downgrade();

        let content: AnyElement = if no_permission {
            graphic(Icon::Server, t(M::ConsoleNoPermission), None).into_any_element()
        } else if self.log.is_empty() && self.status == StreamStatus::Loading {
            div()
                .id("console-loading")
                .accessible(Role::Status, "console-loading", t(M::ConsoleLoading))
                .size_full()
                .px(px(16.))
                .py(px(12.))
                .overflow_hidden()
                .child(skeleton_code(18))
                .into_any_element()
        } else if self.log.is_empty() {
            graphic(Icon::Server, t(M::ConsoleEmpty), None).into_any_element()
        } else {
            let state = self.list.clone();
            log_view::log_view(
                "console-log",
                &self.selection,
                list(
                    self.list.clone(),
                    cx.processor(|this, ix, _, cx| this.row(ix, cx)),
                )
                .size_full()
                .py(px(12.)),
                move |by, _, _| state.scroll_by(by),
            )
            .relative()
            .child({
                let page = cx.entity().downgrade();
                Scrollbar::new("console-scrollbar", self.list.clone())
                    .label(t(M::Console))
                    .on_scroll(move |_, cx| {
                        // "Jump to latest" shows while the list doesn't follow.
                        page.update(cx, |_, cx| cx.notify()).ok();
                    })
            })
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .into_any_element()
        };

        div()
            .size_full()
            .min_h(px(320.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .font_family(fonts::mono())
                    .child(div().flex_1().min_w_0().child(self.search.clone()))
                    .child(
                        Checkbox::new("console-regex", self.regex, t(M::ConsoleRegex)).on_change(
                            move |regex, _, cx| {
                                this.update(cx, |this, cx| {
                                    this.regex = *regex;
                                    this.refilter(cx);
                                })
                                .ok();
                            },
                        ),
                    ),
            )
            .child(
                div()
                    .id("console")
                    .accessible(Role::Log, "console", t(M::Console))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .rounded_t(radius::LG)
                            .border_b_1()
                            .border_color(colors::GRAY_FADED.opacity(30))
                            .bg(colors::GRAY_900)
                            .font_family(fonts::mono())
                            .text_size(text::SMALL)
                            .text_color(colors::GRAY_300)
                            .child(content),
                    )
                    .child(
                        div()
                            .id("console-status")
                            .accessible(Role::Status, "console-status", status_text)
                            .tooltip(text_tooltip(status_text))
                            .absolute()
                            .top(px(6.))
                            .right(px(6.))
                            .p(px(5.))
                            .child(div().size(px(9.)).rounded_full().bg(status_color)),
                    )
                    .when(
                        !following && !self.follow_paused && !self.log.is_empty() && !no_permission,
                        |this| {
                            let page = cx.entity().downgrade();
                            this.child(
                                div().absolute().bottom(px(64.)).right(px(16.)).child(
                                    Button::new("console-follow", t(M::ConsoleJumpToLatest))
                                        .size(ButtonSize::Slim)
                                        .icon(Icon::ArrowDown)
                                        .on_click(move |_, _, cx| {
                                            page.update(cx, |this, cx| {
                                                this.list.set_follow_mode(FollowMode::Tail);
                                                cx.notify();
                                            })
                                            .ok();
                                        }),
                                ),
                            )
                        },
                    )
                    .child(
                        div()
                            .key_context("ConsoleCommand")
                            .on_action(cx.listener(Self::older_command))
                            .on_action(cx.listener(Self::newer_command))
                            .on_action(cx.listener(Self::send_command))
                            .p(px(8.))
                            .rounded_b(radius::LG)
                            .bg(colors::GRAY_850)
                            .font_family(fonts::mono())
                            .text_color(ink::text())
                            .child(self.command.clone()),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(range: std::ops::Range<i64>) -> Vec<ConsoleLine> {
        range
            .map(|n| ConsoleLine {
                snowflake: n,
                message: format!("line {n}"),
            })
            .collect()
    }

    fn shown(log: &ConsoleLog) -> Vec<i64> {
        (0..log.shown_len())
            .map(|ix| log.shown(ix).unwrap().snowflake)
            .collect()
    }

    #[test]
    fn live_lines_append_and_the_history_merges_in() {
        let none = LinkedPlayers::default();
        let mut log = ConsoleLog::default();
        // the live console opens before the history arrives
        assert_eq!(
            log.merge(lines(10..12), &none),
            Change::Appended {
                removed: 0,
                added: 2
            }
        );
        assert_eq!(log.merge(lines(5..11), &none), Change::Rebuilt);
        assert_eq!(shown(&log), (5..12).collect::<Vec<_>>());
        // a repeated line isn't added twice
        assert_eq!(
            log.merge(lines(11..13), &none),
            Change::Rebuilt,
            "11 is already there"
        );
        assert_eq!(shown(&log), (5..13).collect::<Vec<_>>());
    }

    #[test]
    fn keeps_the_newest_lines() {
        let none = LinkedPlayers::default();
        let mut log = ConsoleLog::default();
        log.merge(lines(0..MAX_CONSOLE_LINES as i64), &none);
        assert_eq!(
            log.merge(
                lines(MAX_CONSOLE_LINES as i64..MAX_CONSOLE_LINES as i64 + 3),
                &none
            ),
            Change::Appended {
                removed: 3,
                added: 3
            }
        );
        assert_eq!(log.len(), MAX_CONSOLE_LINES);
        assert_eq!(log.shown(0).unwrap().snowflake, 3);
    }

    #[test]
    fn the_search_filters_plain_text_and_regexes() {
        let none = LinkedPlayers::default();
        let mut log = ConsoleLog::default();
        log.merge(
            vec![
                ConsoleLine {
                    snowflake: 1,
                    message: "\u{1b}[33m[Server thread/WARN]\u{1b}[0m Can't keep up!".into(),
                },
                ConsoleLine {
                    snowflake: 2,
                    message: "§aSteve joined the game".into(),
                },
                ConsoleLine {
                    snowflake: 3,
                    message: "Done (1.2s)!".into(),
                },
            ],
            &none,
        );
        log.set_filter("STEVE", false);
        assert_eq!(shown(&log), vec![2]);
        log.set_filter("§a", false);
        assert_eq!(shown(&log), Vec::<i64>::new(), "codes aren't searched");
        log.set_filter(r"done \(\d", true);
        assert_eq!(shown(&log), vec![3]);
        log.set_filter("warn|done", true);
        assert_eq!(shown(&log), vec![1, 3]);
        log.set_filter("(", true);
        assert_eq!(
            shown(&log),
            Vec::<i64>::new(),
            "an invalid regex matches nothing"
        );
        log.set_filter("(", false);
        assert_eq!(shown(&log), vec![3]);
        // new lines are searched as they come
        log.set_filter("joined", false);
        log.merge(
            vec![ConsoleLine {
                snowflake: 4,
                message: "Alex joined the game".into(),
            }],
            &none,
        );
        assert_eq!(shown(&log), vec![2, 4]);
        log.set_filter("", true);
        assert_eq!(log.shown_len(), 4);
    }

    #[test]
    fn a_selection_keeps_to_its_lines_through_trims_and_filters() {
        use lodestone_ui::components::log_view::{Caret, Selection};
        let none = LinkedPlayers::default();
        let mut log = ConsoleLog::default();
        log.merge(lines(0..MAX_CONSOLE_LINES as i64), &none);
        // "line 10" to "line 12", from the middle of each end.
        let selection = Selection::new(Caret::new(10, 5), Caret::new(12, 6));
        assert_eq!(selection.text(&log), "10\nline 11\nline 1");
        // ⌘A takes the lines shown, by the filter.
        log.set_filter(r"^line 1\d$", true);
        assert_eq!(
            Selection::all(&log).unwrap().text(&log),
            (10..20)
                .map(|n| format!("line {n}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(selection.text(&log), "10\nline 11\nline 1");
        log.set_filter("", true);
        // New lines push the oldest out: the selection stays on its lines, until its
        // first line goes and it starts at the next.
        let end = MAX_CONSOLE_LINES as i64;
        log.merge(lines(end..end + 11), &none);
        assert_eq!(selection.text(&log), "line 11\nline 1");
    }

    #[test]
    fn lines_about_linked_players_name_their_user() {
        let linked = LinkedPlayers::new([("u".into(), "Steve".into(), "peter".into())]);
        let mut log = ConsoleLog::default();
        log.merge(
            vec![
                ConsoleLine {
                    snowflake: 1,
                    message: "[12:00:00] [Server thread/INFO]: <Steve> hi".into(),
                },
                ConsoleLine {
                    snowflake: 2,
                    message: "[12:00:00] [Server thread/INFO]: <Alex> hi".into(),
                },
            ],
            &linked,
        );
        assert_eq!(log.shown(0).unwrap().linked.as_deref(), Some("peter"));
        assert_eq!(log.shown(1).unwrap().linked, None);
    }

    /// The command line's keys, in a window: ↑ and ↓ reach the console's history
    /// rather than moving the cursor, and Enter sends.
    #[cfg(feature = "screenshot")]
    mod keys {
        use std::{cell::RefCell, rc::Rc};

        use gpui::{
            AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _,
            Render, TestAppContext, Window, div, prelude::*,
        };
        use lodestone_ui::components::TextInput;

        use super::super::{NewerCommand, OlderCommand, SendCommand};

        struct CommandLine {
            input: Entity<TextInput>,
            heard: Rc<RefCell<Vec<&'static str>>>,
        }

        impl Render for CommandLine {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let (a, b, c) = (self.heard.clone(), self.heard.clone(), self.heard.clone());
                div()
                    .key_context("ConsoleCommand")
                    .on_action(move |_: &OlderCommand, _, _| a.borrow_mut().push("older"))
                    .on_action(move |_: &NewerCommand, _, _| b.borrow_mut().push("newer"))
                    .on_action(move |_: &SendCommand, _, _| c.borrow_mut().push("send"))
                    .child(self.input.clone())
            }
        }

        #[gpui::test]
        fn arrows_browse_the_history_and_enter_sends(cx: &mut TestAppContext) {
            cx.update(|cx| {
                lodestone_ui::components::init(cx);
                super::super::init(cx);
            });
            let heard = Rc::new(RefCell::new(Vec::new()));
            let (view, cx) = cx.add_window_view({
                let heard = heard.clone();
                move |_, cx| CommandLine {
                    input: cx.new(|cx| TextInput::new(cx).with_text("list")),
                    heard,
                }
            });
            let input = view.read_with(cx, |view, _| view.input.clone());
            cx.update(|window, cx| {
                let handle = input.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
            });
            cx.run_until_parked();
            cx.simulate_keystrokes("up up down enter");
            assert_eq!(*heard.borrow(), vec!["older", "older", "newer", "send"]);
            // other keys still edit the text
            cx.simulate_keystrokes("backspace");
            assert_eq!(
                input.read_with(cx, |input, _| input.text().to_string()),
                "lis"
            );
        }
    }
}
