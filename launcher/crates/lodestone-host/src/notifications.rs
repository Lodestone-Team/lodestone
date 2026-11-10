//! The notifications, after the dashboard's `data/NotificationContext.tsx`,
//! `data/EventStream.ts` and `NotificationPopover.tsx` (`launcher/docs/host-parity.md`,
//! §1.7): the core's past events when the Host signs in, then live ones, as a list
//! behind the bell in the top bar, with the work in progress (instances being made or
//! deleted) above it.
//!
//! Some events also show a toast (a server's warning, a finished zip or unzip), and
//! while the Host's window is in the background a crashed server, or a core that
//! can't start, shows a notification of the system's ([`native`]).

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, MouseButton, ParentElement, Render,
    SharedString, Styled, Subscription, Window, anchored, deferred, div, point, prelude::*,
};
use launcher_core::cores::admin::{
    ClientEvent, EventInner, InstanceState,
    events::{
        EventLevel, InstanceEventInner, ProgressionEndValue, ProgressionEventInner, UserEventInner,
    },
};
use lodestone_ui::{
    components::{Icon, IconButton, Toast},
    theme::{colors, fonts, ink, px, radius, shadow, text, weights},
};

use crate::{
    i18n::{CoreMsg, t, tr},
    state::{HostEvent, HostState},
};

/// Only the newest notifications are kept.
pub const MAX_NOTIFICATIONS: usize = 200;

/// A notification in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    /// The event's snowflake (or a progression's id): the same event from the history
    /// and the stream is listed once.
    pub key: i64,
    pub title: String,
    pub message: String,
    /// Unix milliseconds.
    pub millis: i64,
    pub level: EventLevel,
}

/// Work in progress, from a `ProgressionStart` until its `ProgressionEnd`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ongoing {
    pub event_id: i64,
    pub title: String,
    pub message: Option<String>,
    pub progress: f64,
    pub total: Option<f64>,
    pub millis: i64,
}

impl Ongoing {
    /// The share done, 0 to 0.99 while it runs (`NotificationCard`).
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total.filter(|t| *t > 0.0)?;
        Some(((self.progress / total) as f32).clamp(0.0, 0.99))
    }
}

/// The title and message a notification has for `event`, or `None` for an event
/// that isn't listed (`EventStream.ts`). Progressions are [`List::apply`]'s.
pub fn describe(event: &ClientEvent) -> Option<(String, String)> {
    let title = match &event.event_inner {
        EventInner::InstanceEvent(instance) => {
            let name = &instance.instance_name;
            match &instance.instance_event_inner {
                InstanceEventInner::StateTransition { to } => tr!(
                    match to {
                        InstanceState::Starting => CoreMsg::EventStarting,
                        InstanceState::Running => CoreMsg::EventRunning,
                        InstanceState::Stopping => CoreMsg::EventStopping,
                        InstanceState::Stopped => CoreMsg::EventStopped,
                        InstanceState::Error => CoreMsg::EventCrashed,
                        InstanceState::Unknown => return None,
                    },
                    name = name
                ),
                InstanceEventInner::InstanceWarning { .. } => {
                    tr!(CoreMsg::EventWarning, name = name)
                }
                InstanceEventInner::InstanceError { .. } => tr!(CoreMsg::EventError, name = name),
                InstanceEventInner::PlayerChange {
                    players_joined,
                    players_left,
                    ..
                } => {
                    let names = |players: &[launcher_core::cores::admin::models::Player]| {
                        players
                            .iter()
                            .map(|p| p.name().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let (joined, left) = (names(players_joined), names(players_left));
                    match (joined.is_empty(), left.is_empty()) {
                        (false, false) => tr!(
                            CoreMsg::PlayersJoinedAndLeft,
                            joined = joined,
                            left = left,
                            name = name
                        ),
                        (false, true) => tr!(CoreMsg::PlayersJoined, players = joined, name = name),
                        (true, false) => tr!(CoreMsg::PlayersLeft, players = left, name = name),
                        (true, true) => return None,
                    }
                }
                InstanceEventInner::PlayerMessage {
                    player,
                    player_message,
                } => tr!(
                    CoreMsg::PlayerMessage,
                    player = player,
                    message = player_message,
                    name = name
                ),
                _ => return None,
            }
        }
        EventInner::UserEvent(user) => match &user.user_event_inner {
            UserEventInner::ApiKeyCreated { name, .. } => {
                tr!(CoreMsg::EventApiKeyCreated, name = name)
            }
            UserEventInner::ApiKeyRevoked { name, .. } => {
                tr!(CoreMsg::EventApiKeyRevoked, name = name)
            }
            _ => return None,
        },
        EventInner::MacroEvent(macro_event) => {
            let pid = match &macro_event.macro_pid {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let server = macro_event
                .instance_uuid
                .clone()
                .unwrap_or_else(|| t(CoreMsg::CoreFallback).to_string());
            match macro_event
                .macro_event_inner
                .get("type")
                .and_then(|v| v.as_str())
            {
                Some("Started") => tr!(CoreMsg::MacroStarted, pid = pid, server = server),
                Some("Detach") => tr!(CoreMsg::MacroDetached, pid = pid, server = server),
                Some("Stopped") => {
                    let status = macro_event.macro_event_inner["exit_status"]["type"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    tr!(
                        CoreMsg::MacroStopped,
                        pid = pid,
                        server = server,
                        status = status
                    )
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((title, String::new()))
}

/// The lists, without the view: what the tests check.
#[derive(Default)]
pub struct List {
    /// Oldest first.
    pub items: Vec<Notification>,
    pub ongoing: Vec<Ongoing>,
}

impl List {
    fn push(&mut self, item: Notification) -> bool {
        // the duplicate is almost always one of the last few
        if self.items.iter().rev().any(|i| i.key == item.key) {
            return false;
        }
        self.items.push(item);
        if self.items.len() > MAX_NOTIFICATIONS {
            let extra = self.items.len() - MAX_NOTIFICATIONS;
            self.items.drain(..extra);
        }
        true
    }

    /// Adds what `event` says. Returns whether something new was listed.
    pub fn apply(&mut self, event: &ClientEvent) -> bool {
        if let EventInner::ProgressionEvent(progression) = &event.event_inner {
            let id = progression.event_id;
            return match &progression.progression_event_inner {
                ProgressionEventInner::ProgressionStart {
                    progression_name,
                    total,
                    ..
                } => {
                    if self.ongoing.iter().any(|o| o.event_id == id) {
                        return false;
                    }
                    self.ongoing.push(Ongoing {
                        event_id: id,
                        title: progression_name.clone(),
                        message: None,
                        progress: 0.0,
                        total: *total,
                        millis: event.millis(),
                    });
                    true
                }
                ProgressionEventInner::ProgressionUpdate {
                    progress,
                    progress_message,
                } => {
                    if let Some(item) = self.ongoing.iter_mut().find(|o| o.event_id == id) {
                        item.progress += progress;
                        if let Some(total) = item.total {
                            item.progress = item.progress.min(total);
                        }
                        if !progress_message.is_empty() {
                            item.message = Some(progress_message.clone());
                        }
                    }
                    false
                }
                ProgressionEventInner::ProgressionEnd { message, .. } => {
                    let Some(index) = self.ongoing.iter().position(|o| o.event_id == id) else {
                        return false;
                    };
                    // it moves from the work in progress to the list
                    let item = self.ongoing.remove(index);
                    self.push(Notification {
                        key: event.snowflake,
                        title: item.title,
                        message: message.clone().or(item.message).unwrap_or_default(),
                        millis: event.millis(),
                        level: event.level,
                    })
                }
                ProgressionEventInner::Unknown => false,
            };
        }
        match describe(event) {
            Some((title, message)) => self.push(Notification {
                key: event.snowflake,
                title,
                message,
                millis: event.millis(),
                level: event.level,
            }),
            None => false,
        }
    }
}

pub struct Notifications {
    state: Entity<HostState>,
    pub list: List,
    /// The past events have been fetched.
    history_loaded: bool,
    /// Something new arrived since the list was last opened: the bell's red dot.
    unseen: bool,
    open: bool,
    /// The thank-you for a beta core, once per version (`DashboardLayout.tsx`).
    beta: lodestone_ui::components::DialogSlot<Self, gpui::FocusHandle>,
    _subscriptions: Vec<Subscription>,
}

/// Whether to thank the user for running core `version`: a beta not thanked for yet.
pub fn thank_for_beta(version: &str, last: Option<&str>) -> bool {
    version.contains("beta") && last != Some(version)
}

impl Notifications {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe(&state, |this, _, event, cx| match event {
            HostEvent::Core(event) => this.live(event, cx),
            HostEvent::SignedOut => {
                this.list = List::default();
                cx.notify();
            }
            _ => {}
        });
        let version = state.read(cx).info.version.clone();
        let last = cx
            .try_global::<crate::prefs::Prefs>()
            .and_then(|p| p.data.latest_version.clone());
        let beta = thank_for_beta(&version, last.as_deref());
        let mut this = Self {
            state,
            list: List::default(),
            history_loaded: false,
            unseen: false,
            open: false,
            beta: lodestone_ui::components::DialogSlot::new(|this: &mut Self| &mut this.beta),
            _subscriptions: vec![subscription],
        };
        if beta {
            let focus = cx.focus_handle();
            this.beta.open(focus, cx);
        }
        this.load_history(cx);
        this
    }

    /// `GET /events/all/buffer`: the events from before the Host signed in, which
    /// aren't new (no dot, no toasts).
    fn load_history(&mut self, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let task = cx.background_spawn(async move { admin.event_history().await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.history_loaded = true;
                match result {
                    Ok(events) => {
                        for event in events.iter().filter(|e| !is_console(e)) {
                            this.list.apply(event);
                        }
                    }
                    Err(err) => {
                        let toast = Toast::error(tr!(CoreMsg::LoadFailed, error = err))
                            .key("event-buffer-error");
                        this.state.update(cx, |state, cx| state.toast(toast, cx));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// An event from the stream.
    fn live(&mut self, event: &ClientEvent, cx: &mut Context<Self>) {
        if is_console(event) {
            return;
        }
        let ongoing_before = self.list.ongoing.len();
        if self.list.apply(event) || self.list.ongoing.len() > ongoing_before {
            self.unseen = true;
        }
        self.side_effects(event, cx);
        cx.notify();
    }

    /// Toasts and the system's notifications for a new event.
    fn side_effects(&mut self, event: &ClientEvent, cx: &mut Context<Self>) {
        let background = cx.active_window().is_none();
        match &event.event_inner {
            EventInner::InstanceEvent(instance) => match &instance.instance_event_inner {
                InstanceEventInner::InstanceWarning { .. } => {
                    let toast = Toast::error(t(CoreMsg::WarningToast));
                    self.state.update(cx, |state, cx| state.toast(toast, cx));
                }
                InstanceEventInner::InstanceError { .. }
                | InstanceEventInner::StateTransition {
                    to: InstanceState::Error,
                } if background => {
                    if let Some((title, _)) = describe(event) {
                        native::notify(crate::APP_NAME, &title);
                    }
                }
                _ => {}
            },
            EventInner::ProgressionEvent(progression) => {
                if let ProgressionEventInner::ProgressionEnd {
                    inner:
                        Some(ProgressionEndValue::FsOperationCompleted {
                            instance_uuid,
                            success,
                            message,
                        }),
                    ..
                } = &progression.progression_event_inner
                {
                    // a page that started it shows its own, fuller toast
                    let own = self
                        .state
                        .update(cx, |state, _| state.take_own_file_toast(instance_uuid));
                    if own {
                        return;
                    }
                    let toast = if *success {
                        Toast::success(message.clone())
                    } else {
                        Toast::error(message.clone())
                    };
                    self.state.update(cx, |state, cx| state.toast(toast, cx));
                }
            }
            _ => {}
        }
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        self.unseen = false;
        cx.notify();
    }

    fn close_beta(&mut self, cx: &mut Context<Self>) {
        self.beta.close(cx);
        let version = self.state.read(cx).info.version.clone();
        crate::prefs::Prefs::update(cx, |p| p.latest_version = Some(version));
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    fn card(&self, item: &Notification) -> AnyElement {
        let color = match item.level {
            EventLevel::Error => colors::RED_200,
            EventLevel::Warning => colors::YELLOW,
            _ => ink::muted(),
        };
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .px_3()
            .py_2()
            .rounded(radius::LG)
            .bg(colors::GRAY_850)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .mt(px(6.))
                            .flex_none()
                            .size(px(6.))
                            .rounded_full()
                            .bg(color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(text::MEDIUM)
                            .font_weight(weights::MEDIUM.face())
                            .text_color(ink::text())
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(text::SMALL)
                            .text_color(ink::faint())
                            .child(crate::pages::parts::ago(item.millis / 1000)),
                    ),
            )
            .when(!item.message.is_empty(), |this| {
                this.child(
                    div()
                        .pl(px(14.))
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .child(item.message.clone()),
                )
            })
            .into_any_element()
    }

    fn ongoing_card(&self, item: &Ongoing) -> AnyElement {
        let fraction = item.fraction();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .rounded(radius::LG)
            .bg(colors::GRAY_850)
            .child(
                div()
                    .text_size(text::MEDIUM)
                    .font_weight(weights::MEDIUM.face())
                    .text_color(ink::text())
                    .child(item.title.clone()),
            )
            .children(item.message.clone().map(|message| {
                div()
                    .text_size(text::SMALL)
                    .text_color(ink::muted())
                    .child(message)
            }))
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .rounded_full()
                    .bg(colors::GRAY_700)
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(colors::BLUE)
                            .w(gpui::relative(fraction.unwrap_or(0.1))),
                    ),
            )
            .into_any_element()
    }

    fn heading(text_: &'static str) -> impl IntoElement {
        div()
            .pt_2()
            .text_size(text::SMALL)
            .font_weight(weights::BOLD.face())
            .text_color(ink::secondary())
            .child(text_)
    }

    fn panel(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let height = window.viewport_size().height * 0.8;
        let mut body = div()
            .id("notifications-list")
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll()
            .max_h(height - px(60.));
        if !self.list.ongoing.is_empty() {
            body = body.child(Self::heading(t(CoreMsg::InProgress))).children(
                self.list
                    .ongoing
                    .iter()
                    .map(|item| self.ongoing_card(item))
                    .collect::<Vec<_>>(),
            );
        }
        body = body.child(Self::heading(t(CoreMsg::AllNotifications)));
        if !self.history_loaded {
            body = body.child(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(t(CoreMsg::LoadingNotifications)),
            );
        } else if self.list.items.is_empty() {
            body = body.child(
                div()
                    .py_4()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(t(CoreMsg::NoNotifications)),
            );
        } else {
            body = body.children(
                self.list
                    .items
                    .iter()
                    .rev()
                    .map(|item| self.card(item))
                    .collect::<Vec<_>>(),
            );
        }
        div()
            .id("notifications-panel")
            .occlude()
            .w(px(480.))
            .max_h(height)
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded(radius::XL)
            .border_1()
            .border_color(ink::border())
            .bg(colors::GRAY_800)
            .shadow(vec![gpui::BoxShadow {
                color: shadow::COLOR.into(),
                offset: point(px(0.), px(4.)),
                blur_radius: px(16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close(cx)))
            .child(
                div()
                    .font_family(fonts::heading())
                    .text_size(text::H3)
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::text())
                    .child(t(CoreMsg::Notifications)),
            )
            .child(body)
            .into_any_element()
    }
}

impl Render for Notifications {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dot = self.unseen || !self.list.ongoing.is_empty();
        let panel = self.open.then(|| {
            let panel = self.panel(window, cx);
            // under the bell, at the window's right edge
            let corner = point(
                window.viewport_size().width - px(12.),
                gpui::px(crate::TOP_BAR_HEIGHT + 4.0),
            );
            deferred(
                anchored()
                    .position(corner)
                    .anchor(gpui::Anchor::TopRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(panel),
            )
            .with_priority(3)
        });
        let close = cx.entity().downgrade();
        let beta = self.beta.render(window, |focus| {
            let close = close.clone();
            Some(
                lodestone_ui::components::Dialog::new(
                    "beta-thanks",
                    t(CoreMsg::BetaTitle),
                    focus.clone(),
                )
                .on_close(move |_, cx| {
                    close.update(cx, |this, cx| this.close_beta(cx)).ok();
                })
                .child(crate::pages::parts::paragraph(t(CoreMsg::BetaBody)))
                .into_any_element(),
            )
        });
        let beta = lodestone_ui::components::window_layer(window, beta);
        div()
            .relative()
            .flex()
            .flex_col()
            .items_end()
            .children(beta)
            .child(
                div()
                    .id("notifications-bell")
                    .relative()
                    // The bell sits in the top bar, which moves the window: it is not
                    // part of the caption (on Windows the bar answers the hit test as
                    // the caption, which swallows the click), and pressing it doesn't
                    // arm the bar's drag.
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new("notifications", Icon::Bell)
                            .label(SharedString::from(t(CoreMsg::Notifications)))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle(cx))),
                    )
                    .when(dot, |this| {
                        this.child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .right(px(4.))
                                .size(px(8.))
                                .rounded_full()
                                .bg(colors::RED_200),
                        )
                    }),
            )
            .children(panel)
    }
}

/// The system's own notifications, for when the Host's window is in the background,
/// as the Tauri Host's `notify-rust`: Notification Center on macOS (through
/// `osascript`), `notify-send` on Linux desktops. Nothing on Windows yet.
pub mod native {
    /// Shows a notification of the system's. Best effort: errors are ignored.
    pub fn notify(title: &str, body: &str) {
        if cfg!(test) {
            return;
        }
        // Windows: from the notification area icon, which Windows shows as a toast
        #[cfg(windows)]
        if crate::tray::notify(title, body) {
            return;
        }
        let (title, body) = (title.to_string(), body.to_string());
        std::thread::spawn(move || {
            let _ = command(&title, &body).and_then(|mut c| c.status().ok());
        });
    }

    /// AppleScript's string for `text`: quoted, with `"` and `\` escaped.
    pub fn applescript_string(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }

    fn command(title: &str, body: &str) -> Option<std::process::Command> {
        if cfg!(target_os = "macos") {
            let mut command = std::process::Command::new("osascript");
            command.arg("-e").arg(format!(
                "display notification {} with title {}",
                applescript_string(body),
                applescript_string(title)
            ));
            Some(command)
        } else if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            let mut command = std::process::Command::new("notify-send");
            command.arg("--app-name").arg(title).arg(title).arg(body);
            Some(command)
        } else {
            None
        }
    }
}

/// A console line (output, input, chat), which the console shows and the list leaves out.
fn is_console(event: &ClientEvent) -> bool {
    matches!(&event.event_inner, EventInner::InstanceEvent(i) if i.instance_event_inner.is_console())
}

/// Whether the app is in the background (no window of it is active).
pub fn in_background(cx: &App) -> bool {
    cx.active_window().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: serde_json::Value) -> ClientEvent {
        serde_json::from_value(json).unwrap()
    }

    fn state_change(snowflake: i64, to: &str) -> ClientEvent {
        event(serde_json::json!({
            "event_inner": { "type": "InstanceEvent",
                "instance_uuid": "i", "instance_name": "SMP",
                "instance_event_inner": { "type": "StateTransition", "to": to } },
            "details": "", "snowflake": snowflake.to_string(), "level": "Info",
            "caused_by": { "type": "System" },
        }))
    }

    #[test]
    fn events_become_notifications_once() {
        lodestone_ui::i18n::force(lodestone_ui::i18n::Locale::En);
        let mut list = List::default();
        assert!(list.apply(&state_change(1 << 22, "Running")));
        assert!(
            !list.apply(&state_change(1 << 22, "Running")),
            "seen already"
        );
        assert_eq!(list.items[0].title, "Server SMP started!");
        assert_eq!(list.items[0].millis, 1 + 1_667_530_800_000);
        for i in 0..MAX_NOTIFICATIONS as i64 + 5 {
            list.apply(&state_change((i + 2) << 22, "Stopped"));
        }
        assert_eq!(list.items.len(), MAX_NOTIFICATIONS);
    }

    #[test]
    fn progress_moves_to_the_list_when_it_ends() {
        let progression = |snowflake: i64, inner: serde_json::Value| {
            event(serde_json::json!({
                "event_inner": { "type": "ProgressionEvent", "event_id": "77",
                    "progression_event_inner": inner },
                "details": "", "snowflake": snowflake.to_string(), "level": "Info",
                "caused_by": { "type": "System" },
            }))
        };
        let mut list = List::default();
        list.apply(&progression(
            10,
            serde_json::json!({ "type": "ProgressionStart", "progression_name": "Setting up SMP",
                "total": 10.0, "inner": null }),
        ));
        list.apply(&progression(
            11,
            serde_json::json!({ "type": "ProgressionUpdate", "progress_message": "Downloading",
                "progress": 25.0 }),
        ));
        assert_eq!(list.ongoing[0].progress, 10.0, "clamped to the total");
        assert_eq!(list.ongoing[0].fraction(), Some(0.99));
        assert_eq!(list.ongoing[0].message.as_deref(), Some("Downloading"));
        list.apply(&progression(
            12,
            serde_json::json!({ "type": "ProgressionEnd", "success": true, "message": "Done",
                "inner": null }),
        ));
        assert!(list.ongoing.is_empty());
        assert_eq!(list.items[0].title, "Setting up SMP");
        assert_eq!(list.items[0].message, "Done");
    }

    #[test]
    fn players_and_keys_have_titles() {
        lodestone_ui::i18n::force(lodestone_ui::i18n::Locale::En);
        let players = event(serde_json::json!({
            "event_inner": { "type": "InstanceEvent",
                "instance_uuid": "i", "instance_name": "SMP",
                "instance_event_inner": { "type": "PlayerChange", "player_list": [],
                    "players_joined": [{ "type": "MinecraftPlayer", "name": "Steve" }],
                    "players_left": [] } },
            "details": "", "snowflake": "5", "level": "Info", "caused_by": { "type": "System" },
        }));
        assert_eq!(describe(&players).unwrap().0, "Steve Joined SMP");
        let console = event(serde_json::json!({
            "event_inner": { "type": "InstanceEvent",
                "instance_uuid": "i", "instance_name": "SMP",
                "instance_event_inner": { "type": "InstanceOutput", "message": "hi" } },
            "details": "", "snowflake": "6", "level": "Info", "caused_by": { "type": "System" },
        }));
        assert!(describe(&console).is_none());
    }

    #[test]
    fn betas_are_thanked_for_once() {
        assert!(thank_for_beta("0.5.2-beta.1", None));
        assert!(!thank_for_beta("0.5.2-beta.1", Some("0.5.2-beta.1")));
        assert!(thank_for_beta("0.5.2-beta.2", Some("0.5.2-beta.1")));
        assert!(!thank_for_beta("0.5.2", None));
    }

    #[test]
    fn applescript_strings_are_escaped() {
        assert_eq!(
            native::applescript_string(r#"a "b" \c"#),
            r#""a \"b\" \\c""#
        );
    }
}
