//! An instance's Event logs tab. The dashboard's tab is a "coming soon" placeholder;
//! the Host lists what happened to the server instead: its state changes, warnings
//! and errors, players joining and leaving, and chat, newest first. They come from
//! the events the core keeps (`GET /events/all/buffer`), then live from the event
//! stream. Console output is in the Console tab, not here.

use std::collections::HashSet;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::admin::{
    ClientEvent, EventInner, InstanceState,
    events::{CausedBy, EventLevel, InstanceEventInner},
    models::Player,
};
use lodestone_ui::{
    components::{
        Icon, LoadingDots,
        settings::{card, content_width},
    },
    i18n::format,
    theme::{colors, fonts, ink, leading, line_height, px, text},
};

use crate::{
    i18n::{InstanceMsg as M, ShellMsg, t, tr},
    state::{HostEvent, HostState},
};

/// At most this many lines are kept.
const MAX_LINES: usize = 500;

/// One line of the log.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub snowflake: i64,
    /// Unix milliseconds.
    pub millis: i64,
    pub level: EventLevel,
    pub text: String,
    /// Who caused it, when a user did.
    pub by: Option<String>,
}

fn state_name(state: InstanceState) -> &'static str {
    t(match state {
        InstanceState::Starting => ShellMsg::StateStarting,
        InstanceState::Running => ShellMsg::StateRunning,
        InstanceState::Stopping => ShellMsg::StateStopping,
        InstanceState::Stopped => ShellMsg::StateStopped,
        InstanceState::Error => ShellMsg::StateError,
        InstanceState::Unknown => ShellMsg::StateUnknown,
    })
}

fn names(players: &[Player]) -> String {
    let names: Vec<&str> = players.iter().map(Player::name).collect();
    format::list(&names)
}

/// The lines `event` makes in the log of `uuid`: none for another instance's event,
/// console output, or what the log doesn't show.
pub fn lines_of(event: &ClientEvent, uuid: &str) -> Vec<LogLine> {
    let EventInner::InstanceEvent(inner) = &event.event_inner else {
        return Vec::new();
    };
    if inner.instance_uuid != uuid {
        return Vec::new();
    }
    let by = match &event.caused_by {
        CausedBy::User { user_name, .. } => Some(user_name.clone()),
        _ => None,
    };
    let line = |level: EventLevel, text: String| LogLine {
        snowflake: event.snowflake,
        millis: event.millis(),
        level,
        text,
        by: by.clone(),
    };
    match &inner.instance_event_inner {
        InstanceEventInner::StateTransition { to } => {
            let level = if *to == InstanceState::Error {
                EventLevel::Error
            } else {
                EventLevel::Info
            };
            vec![line(level, tr!(M::EventState, state = state_name(*to)))]
        }
        InstanceEventInner::InstanceWarning { message } => {
            vec![line(EventLevel::Warning, message.clone())]
        }
        InstanceEventInner::InstanceError { message } => {
            vec![line(EventLevel::Error, message.clone())]
        }
        InstanceEventInner::PlayerChange {
            players_joined,
            players_left,
            ..
        } => {
            let mut lines = Vec::new();
            if !players_joined.is_empty() {
                lines.push(line(
                    EventLevel::Info,
                    tr!(M::EventJoined, names = names(players_joined)),
                ));
            }
            if !players_left.is_empty() {
                lines.push(line(
                    EventLevel::Info,
                    tr!(M::EventLeft, names = names(players_left)),
                ));
            }
            lines
        }
        InstanceEventInner::PlayerMessage {
            player,
            player_message,
        } => vec![line(
            EventLevel::Info,
            format!("<{player}> {player_message}"),
        )],
        _ => Vec::new(),
    }
}

/// Adds `event`'s lines to `log` (newest first), once each, keeping [`MAX_LINES`].
pub fn add(log: &mut Vec<LogLine>, seen: &mut HashSet<i64>, event: &ClientEvent, uuid: &str) {
    if !seen.insert(event.snowflake) {
        return;
    }
    let lines = lines_of(event, uuid);
    if lines.is_empty() {
        return;
    }
    log.extend(lines);
    log.sort_by_key(|line| std::cmp::Reverse(line.snowflake));
    log.truncate(MAX_LINES);
}

pub struct EventLogsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    /// `None` while the history loads.
    log: Option<Vec<LogLine>>,
    seen: HashSet<i64>,
    error: Option<String>,
    _task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventLogsPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(&state, |this, _, event, cx| {
            if let HostEvent::Core(event) = event
                && let Some(log) = &mut this.log
            {
                add(log, &mut this.seen, event, &this.uuid);
                cx.notify();
            }
        })];
        let admin = state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let history = cx
                .background_spawn(async move { admin.event_history().await })
                .await;
            this.update(cx, |this, cx| {
                let mut log = Vec::new();
                match history {
                    Ok(events) => {
                        for event in &events {
                            add(&mut log, &mut this.seen, event, &this.uuid);
                        }
                    }
                    Err(err) => this.error = Some(err.to_string()),
                }
                this.log = Some(log);
                cx.notify();
            })
            .ok();
        });
        Self {
            state,
            uuid,
            log: None,
            seen: HashSet::new(),
            error: None,
            _task: Some(task),
            _subscriptions: subscriptions,
        }
    }
}

/// When an event happened: the time today, else the date and time.
fn when(millis: i64) -> String {
    use chrono::TimeZone as _;
    let Some(time) = chrono::Local.timestamp_millis_opt(millis).single() else {
        return String::new();
    };
    if time.date_naive() == chrono::Local::now().date_naive() {
        time.format("%H:%M:%S").to_string()
    } else {
        time.format("%Y-%m-%d %H:%M").to_string()
    }
}

fn row(index: usize, line: &LogLine) -> AnyElement {
    let (icon, color) = match line.level {
        EventLevel::Error => (Icon::CircleExclamation, colors::RED_200),
        EventLevel::Warning => (Icon::TriangleExclamation, colors::YELLOW),
        _ => (Icon::CircleInfo, colors::GRAY_500),
    };
    div()
        .id(("event-line", index))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(10.))
        .px(px(16.))
        .py(px(8.))
        .child(div().pt(px(3.)).child(icon.render(px(12.), color)))
        .child(
            div()
                .flex_none()
                .w(px(120.))
                .font_family(fonts::mono())
                .text_size(text::SMALL)
                .text_color(ink::muted())
                .pt(px(1.))
                .child(when(line.millis)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(if line.level == EventLevel::Info {
                    ink::text()
                } else {
                    color
                })
                .child(SharedString::from(line.text.clone()))
                .when_some(line.by.clone(), |this, by| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(tr!(M::EventBy, user = by)),
                    )
                }),
        )
        .into_any_element()
}

impl Render for EventLogsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = match &self.log {
            None => vec![
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(64.))
                    .child(LoadingDots)
                    .into_any_element(),
            ],
            Some(log) if log.is_empty() => vec![
                div()
                    .px(px(16.))
                    .py(px(14.))
                    .text_color(ink::muted())
                    .child(t(M::EventLogsEmpty))
                    .into_any_element(),
            ],
            Some(log) => log
                .iter()
                .enumerate()
                .map(|(index, line)| row(index, line))
                .collect(),
        };
        div()
            .id("event-logs")
            .size_full()
            .overflow_y_scroll()
            .text_size(text::MEDIUM)
            .line_height(line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div().w_full().max_w(content_width()).pb(px(64.)).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(text::SMALL)
                                .text_color(ink::muted())
                                .child(t(M::EventLogsLead)),
                        )
                        .when_some(self.error.clone(), |this, error| {
                            this.child(
                                div()
                                    .text_color(colors::RED_200)
                                    .child(tr!(M::EventLogsFailed, error = error)),
                            )
                        })
                        .child(card(rows)),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::events::parse;

    fn event(snowflake: i64, uuid: &str, inner: &str) -> ClientEvent {
        parse(&format!(
            r#"{{"event_inner":{{"type":"InstanceEvent","instance_uuid":"{uuid}","instance_name":"SMP",
                "instance_event_inner":{inner}}},
                "details":"","snowflake":"{snowflake}","level":"Info",
                "caused_by":{{"type":"User","user_id":"u","user_name":"steve"}}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn keeps_this_instances_events_newest_first() {
        let mut log = Vec::new();
        let mut seen = HashSet::new();
        let running = event(
            1 << 22,
            "i1",
            r#"{"type":"StateTransition","to":"Running"}"#,
        );
        let joined = event(
            2 << 22,
            "i1",
            r#"{"type":"PlayerChange","player_list":[],"players_joined":[{"type":"MinecraftPlayer","name":"Alex"}],"players_left":[]}"#,
        );
        let other = event(
            3 << 22,
            "i2",
            r#"{"type":"InstanceError","message":"boom"}"#,
        );
        let output = event(
            4 << 22,
            "i1",
            r#"{"type":"InstanceOutput","message":"[INFO] hi"}"#,
        );
        for e in [&running, &joined, &other, &output, &joined] {
            add(&mut log, &mut seen, e, "i1");
        }
        assert_eq!(log.len(), 2, "{log:?}");
        assert!(log[0].text.contains("Alex"));
        assert!(log[1].text.contains(t(ShellMsg::StateRunning)));
        assert_eq!(log[1].by.as_deref(), Some("steve"));
    }

    #[test]
    fn errors_are_errors() {
        let error = event(1, "i1", r#"{"type":"InstanceError","message":"crashed"}"#);
        let lines = lines_of(&error, "i1");
        assert_eq!(lines[0].level, EventLevel::Error);
        assert_eq!(lines[0].text, "crashed");
    }
}
