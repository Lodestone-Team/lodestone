//! An instance's Overview tab (`InstanceOverview.tsx`): the name and description,
//! edited in place; the labels (game, state, version, players, address); "Share with
//! friends" (Lodestone Connect); the CPU and memory graphs of the last minute
//! (`/monitor/{uuid}`); the players with their linked Lodestone users; and the details.

use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, FocusHandle, Global, IntoElement,
    KeyDownEvent, ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div,
    img, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        InstanceInfo, InstanceState,
        console::LinkedPlayers,
        instances::{InstanceShare, PerformanceReport, ShareConnection, ShareState},
        models::{Game, MinecraftVariant, Player},
    },
    live::{LiveMessage, LiveStream, StreamKind},
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Icon, IconButton, LineChart, LoadingDots,
        TextInput, Toast, text_tooltip, window_layer,
    },
    theme::{Color, colors, fonts, ink, px, radius, text, weights},
};

use crate::{
    i18n::{CommonMsg, InstancesMsg as M, InstancesPlural as P, t, tr, trn},
    instance_icons,
    shell::sidebar::{instance_address, state_color, state_name},
    state::{HostEvent, HostState},
};

/// One report a second, for the last minute.
pub const SAMPLES: usize = 60;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Whether the details are open, for the session (the dashboard keeps it in
/// `localStorage.instanceDetailsOpen`).
static DETAILS_OPEN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditField {
    Name,
    Description,
}

struct Editing {
    field: EditField,
    input: Entity<TextInput>,
    error: Option<String>,
    saving: bool,
}

/// The share status last fetched, by instance: the card shows it at once when the
/// Overview opens again, while it fetches anew, so it doesn't pop in.
#[derive(Default)]
struct ShareCache(HashMap<String, InstanceShare>);

impl Global for ShareCache {}

/// Keeps (or, with `None`, forgets) an instance's share status for the next opening.
fn remember_share(uuid: &str, share: Option<&InstanceShare>, cx: &mut App) {
    let cache = &mut cx.default_global::<ShareCache>().0;
    match share {
        Some(share) => {
            cache.insert(uuid.to_string(), share.clone());
        }
        None => {
            cache.remove(uuid);
        }
    }
}

/// The "Stop sharing?" confirmation.
struct StopPrompt {
    focus: FocusHandle,
}

pub struct OverviewPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    editing: Option<Editing>,
    /// `None`: not shown (not a Java server, a user who may not, a core without it).
    share: Option<InstanceShare>,
    share_busy: bool,
    /// Start sharing as soon as the status arrives (the sidebar's "Share").
    share_on_load: bool,
    stop_prompt: DialogSlot<Self, StopPrompt>,
    /// Oldest first, at most [`SAMPLES`].
    reports: VecDeque<PerformanceReport>,
    /// Reports received; `None` before the first.
    received: Option<u64>,
    linked: LinkedPlayers,
    sort_ascending: bool,
    _monitor: Option<LiveStream>,
    _poll: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl OverviewPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let share_on_load = state.update(cx, |state, _| {
            if state.share_requested.as_deref() == Some(uuid.as_str()) {
                state.share_requested = None;
                true
            } else {
                false
            }
        });
        let subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                // the sidebar's Share while this Overview is open
                let requested = state.read(cx).share_requested.as_deref() == Some(&this.uuid);
                if requested {
                    state.update(cx, |state, _| state.share_requested = None);
                    this.request_share(cx);
                }
                cx.notify()
            }),
            cx.subscribe(&state, |this, _, event, cx| {
                // a session made again: follow the monitor again
                if matches!(event, HostEvent::InstancesReloaded) && this._monitor.is_none() {
                    this.follow_monitor(cx);
                }
            }),
        ];
        let share = cx
            .try_global::<ShareCache>()
            .and_then(|cache| cache.0.get(&uuid).cloned());
        let mut this = Self {
            state,
            uuid,
            editing: None,
            share,
            share_busy: false,
            share_on_load,
            stop_prompt: DialogSlot::new(|this: &mut Self| &mut this.stop_prompt),
            reports: VecDeque::new(),
            received: None,
            linked: LinkedPlayers::default(),
            sort_ascending: true,
            _monitor: None,
            _poll: None,
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.follow_monitor(cx);
        this.poll_share(cx);
        this.load_links(cx);
        this
    }

    fn instance(&self, cx: &gpui::App) -> Option<InstanceInfo> {
        self.state.read(cx).instance(&self.uuid).cloned()
    }

    fn admin(&self, cx: &gpui::App) -> launcher_core::cores::admin::AdminClient {
        self.state.read(cx).admin.clone()
    }

    fn toast(&self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    // ---- The monitor ----

    fn follow_monitor(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin(cx);
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        self._monitor = Some(LiveStream::open(
            admin.core(),
            admin.token().clone(),
            &self.uuid,
            StreamKind::Monitor,
            tx,
        ));
        let task = cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            let mut connected = false;
            while let Some(message) = rx.next().await {
                let alive = this
                    .update(cx, |this, cx| match message {
                        // the core replays the last minute on every connection
                        LiveMessage::Connected => {
                            if connected {
                                this.reports.clear();
                            }
                            connected = true;
                        }
                        LiveMessage::Frame(frame) => {
                            if let Some(report) = PerformanceReport::parse(&frame) {
                                this.add_report(report);
                                cx.notify();
                            }
                        }
                        LiveMessage::Ended(_) => this._monitor = None,
                        _ => {}
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        });
        self._tasks.push(task);
    }

    fn add_report(&mut self, report: PerformanceReport) {
        self.reports.push_back(report);
        while self.reports.len() > SAMPLES {
            self.reports.pop_front();
        }
        self.received = Some(self.received.unwrap_or(0) + 1);
    }

    // ---- Share with friends ----

    /// Fetches the share status, and again every 5 s while sharing (2 s while it
    /// starts or reconnects), as the dashboard does. A failure hides the card.
    fn poll_share(&mut self, cx: &mut Context<Self>) {
        let is_java = self
            .instance(cx)
            .is_some_and(|i| matches!(i.game_type, Game::MinecraftJava { .. }));
        if !is_java {
            return;
        }
        let admin = self.admin(cx);
        let uuid = self.uuid.clone();
        self._poll = Some(cx.spawn(async move |this, cx| {
            loop {
                let fetch = {
                    let admin = admin.clone();
                    let uuid = uuid.clone();
                    cx.background_spawn(async move { admin.share(&uuid).await })
                };
                let result = fetch.await;
                let next = this
                    .update(cx, |this, cx| {
                        let next = match result {
                            Ok(share) => {
                                let next = share_poll_interval(&share);
                                remember_share(&this.uuid, Some(&share), cx);
                                this.share = Some(share);
                                if this.share_on_load {
                                    this.share_on_load = false;
                                    if !this.share.as_ref().is_some_and(|s| s.sharing) {
                                        this.start_share(cx);
                                    }
                                }
                                next
                            }
                            Err(CoreError::SignedOut) => None,
                            Err(_) => {
                                remember_share(&this.uuid, None, cx);
                                this.share = None;
                                None
                            }
                        };
                        cx.notify();
                        next
                    })
                    .ok()
                    .flatten();
                let Some(next) = next else { break };
                cx.background_executor().timer(next).await;
            }
        }));
    }

    /// Starts sharing unless the server is shared already; before the first answer
    /// about sharing, once it comes.
    fn request_share(&mut self, cx: &mut Context<Self>) {
        match &self.share {
            Some(share) if share.sharing => {}
            Some(_) => self.start_share(cx),
            None => self.share_on_load = true,
        }
    }

    fn start_share(&mut self, cx: &mut Context<Self>) {
        if self.share_busy {
            return;
        }
        self.share_busy = true;
        let admin = self.admin(cx);
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.start_share(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                this.share_busy = false;
                match result {
                    Ok(share) => {
                        remember_share(&this.uuid, Some(&share), cx);
                        this.share = Some(share);
                        this.poll_share(cx);
                    }
                    Err(err) => this.toast(Toast::error(err.to_string()).key("share"), cx),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    /// What the Stop sharing dialog's confirm button does.
    pub fn confirm_stop_share(&mut self, cx: &mut Context<Self>) {
        self.stop_share(cx);
    }

    fn stop_share(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin(cx);
        let uuid = self.uuid.clone();
        self.stop_prompt.close(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.stop_share(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(share) => {
                        remember_share(&this.uuid, Some(&share), cx);
                        this.share = Some(share);
                        this._poll = None;
                    }
                    Err(err) => this.toast(Toast::error(err.to_string()).key("share"), cx),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
    }

    fn copy(&self, text: String, message: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(Toast::success(message).key("copied"), cx);
    }

    // ---- Linked players ----

    fn load_links(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin(cx);
        let me = self.state.read(cx).me.clone();
        let task = cx.spawn(async move |this, cx| {
            let links = cx
                .background_spawn(
                    async move { admin.linked_players(me.is_owner, &me.username).await },
                )
                .await;
            if let Ok(links) = links {
                this.update(cx, |this, cx| {
                    this.linked = LinkedPlayers::from_links(links);
                    cx.notify();
                })
                .ok();
            }
        });
        self._tasks.push(task);
    }

    // ---- Editing the name and description ----

    fn edit(&mut self, field: EditField, window: &mut Window, cx: &mut Context<Self>) {
        let Some(instance) = self.instance(cx) else {
            return;
        };
        let text = SharedString::from(match field {
            EditField::Name => instance.name,
            EditField::Description => instance.description,
        });
        let input = cx.new(|cx| {
            TextInput::new(cx).with_text(text).with_label(match field {
                EditField::Name => M::DetailsName,
                EditField::Description => M::DetailsDescription,
            })
        });
        use gpui::Focusable as _;
        input.focus_handle(cx).focus(window, cx);
        self.editing = Some(Editing {
            field,
            input,
            error: None,
            saving: false,
        });
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|e| e.saving) {
            return;
        }
        self.editing = None;
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        if editing.saving {
            return;
        }
        let value = editing.input.read(cx).text().trim().to_string();
        let field = editing.field;
        if field == EditField::Name && value.is_empty() {
            editing.error = Some(t(M::NameEmpty).into());
            cx.notify();
            return;
        }
        editing.saving = true;
        editing.error = None;
        let admin = self.admin(cx);
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let sent = value.clone();
            let result = cx
                .background_spawn(async move {
                    match field {
                        EditField::Name => admin.set_instance_name(&uuid, &sent).await,
                        EditField::Description => {
                            admin.set_instance_description(&uuid, &sent).await
                        }
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.editing = None;
                        let uuid = this.uuid.clone();
                        this.state.update(cx, |state, cx| {
                            state.update_instance(
                                &uuid,
                                |instance| match field {
                                    EditField::Name => instance.name = value,
                                    EditField::Description => instance.description = value,
                                },
                                cx,
                            )
                        });
                    }
                    Err(err) => {
                        if let Some(editing) = this.editing.as_mut() {
                            editing.saving = false;
                            editing.error = Some(err.to_string());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    /// Sample reports, players and a share, for the development screenshots.
    #[cfg(feature = "screenshot")]
    pub fn demo(&mut self, cx: &mut Context<Self>) {
        for i in 0..SAMPLES {
            let gap = (20..24).contains(&i);
            let wave = ((i as f32) / 6.0).sin();
            self.add_report(PerformanceReport {
                memory_usage: (!gap).then_some(((2.4 + 0.3 * wave as f64) * GIB) as u64),
                cpu_usage: (!gap).then_some(18.0 + 12.0 * wave + (i % 7) as f32),
                start_time: None,
            });
        }
        self.linked = LinkedPlayers::new([(
            "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
            "Notch".into(),
            "peter".into(),
        )]);
        let uuid = self.uuid.clone();
        self.state.update(cx, |state, cx| {
            state.update_instance(
                &uuid,
                |instance| {
                    instance.player_count = Some(2);
                    instance.player_list = Some(vec![
                        Player::MinecraftPlayer {
                            name: "Notch".into(),
                            uuid: Some("069a79f4-44e9-4726-a5be-fca90e38aaf5".into()),
                        },
                        Player::MinecraftPlayer {
                            name: "Alex".into(),
                            uuid: None,
                        },
                    ]);
                },
                cx,
            )
        });
        self.share = Some(InstanceShare {
            sharing: true,
            state: Some(ShareState::Sharing),
            link: Some("https://connect.lodestone.cc/j/7KQ2M9XB4D#3f9a….c01d".into()),
            stable_link: true,
            joins: 3,
            friends: vec![launcher_core::cores::admin::instances::SharedFriend {
                id: 1,
                address: "203.0.113.42".into(),
                connection: ShareConnection::Internet,
                connected_at: 0,
                rtt_ms: 38,
            }],
            problem: None,
            hard_network: true,
        });
        self._poll = None;
        DETAILS_OPEN.store(true, Ordering::Relaxed);
        cx.notify();
    }
}

/// "Dev relay: connect-dev.lodestone.cc" when the share's link goes through another
/// Lodestone Connect service than connect.lodestone.cc (the Connect service setting).
pub fn share_relay_note(share: &InstanceShare) -> Option<String> {
    let invite = launcher_core::cores::connect::Invite::parse(share.link.as_deref()?).ok()?;
    super::connect_service::relay_note(&invite.service)
}

/// How long until the share status is fetched again, or `None` to stop.
pub fn share_poll_interval(share: &InstanceShare) -> Option<Duration> {
    if !share.sharing {
        return None;
    }
    Some(match share.state {
        Some(ShareState::Sharing) => Duration::from_secs(5),
        _ => Duration::from_secs(2),
    })
}

/// A section's title and the line under it.
fn heading(title: &'static str, lead: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(text::H2)
                .font_weight(weights::EXTRABOLD.face())
                .text_color(ink::text())
                .child(title),
        )
        .child(
            div()
                .text_size(text::H3)
                .italic()
                .text_color(ink::muted())
                .child(lead),
        )
        .into_any_element()
}

/// A coloured label (`Label size="large"`).
fn label(text_: impl Into<SharedString>, color: Color) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .py(px(2.))
        .rounded(radius::MD)
        .bg(color.opacity(15))
        .text_size(text::MEDIUM)
        .font_weight(weights::MEDIUMBOLD.face())
        .text_color(color)
        .child(text_.into())
}

fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .p(px(16.))
        .rounded(radius::LG)
        .border_1()
        .border_color(ink::border())
        .bg(colors::GRAY_850)
}

/// The loader of a Minecraft Java instance, `None` for vanilla.
fn loader_name(instance: &InstanceInfo) -> Option<String> {
    match &instance.game_type {
        Game::MinecraftJava { variant } => match variant {
            MinecraftVariant::Vanilla | MinecraftVariant::Unknown => None,
            MinecraftVariant::Other { name } => Some(name.clone()),
            other => Some(format!("{other:?}")),
        },
        _ => None,
    }
}

impl OverviewPage {
    fn render_header(&mut self, instance: &InstanceInfo, cx: &mut Context<Self>) -> AnyElement {
        let me = &self.state.read(cx).me;
        let read_only = instance.is_atom() && !me.is_owner;
        let address = instance_address(self.state.read(cx).settings.as_ref(), instance.port);
        let editable =
            |field: EditField, this: &mut Self, cx: &mut Context<Self>| -> AnyElement {
                let (value, placeholder, size, color, edit_label) = match field {
                    EditField::Name => (
                        instance.name.clone(),
                        t(M::NoName),
                        text::XLARGE_2,
                        ink::text(),
                        t(M::EditName),
                    ),
                    EditField::Description => (
                        instance.description.clone(),
                        t(M::NoDescription),
                        text::H3,
                        ink::muted(),
                        t(M::EditDescription),
                    ),
                };
                if let Some(editing) = this.editing.as_ref().filter(|e| e.field == field) {
                    let saving = editing.saving;
                    return div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.))
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    match event.keystroke.key.as_str() {
                                        "enter" => this.save_edit(cx),
                                        "escape" => this.cancel_edit(cx),
                                        _ => return,
                                    }
                                    cx.stop_propagation();
                                }))
                                .child(div().flex_1().min_w_0().child(editing.input.clone()))
                                .child(if saving {
                                    div().px(px(6.)).child(LoadingDots).into_any_element()
                                } else {
                                    IconButton::new(
                                        SharedString::from(format!("save-{field:?}")),
                                        Icon::FloppyDisk,
                                    )
                                    .label(t(CommonMsg::Save))
                                    .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx)))
                                    .into_any_element()
                                })
                                .child(
                                    IconButton::new(
                                        SharedString::from(format!("cancel-{field:?}")),
                                        Icon::Xmark,
                                    )
                                    .label(t(CommonMsg::Cancel))
                                    .disabled(saving)
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_edit(cx))),
                                ),
                        )
                        .when_some(editing.error.clone(), |this, error| {
                            this.child(
                                div()
                                    .text_size(text::SMALL)
                                    .text_color(colors::RED_200)
                                    .child(error),
                            )
                        })
                        .into_any_element();
                }
                let shown = if value.is_empty() {
                    placeholder.to_string()
                } else {
                    value
                };
                div()
                    .id(SharedString::from(format!("overview-{field:?}")))
                    .group("editable")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .min_w_0()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .when(field == EditField::Name, |this| {
                                this.font_family(fonts::heading())
                                    .font_weight(weights::BOLD.face())
                            })
                            .when(field == EditField::Description, |this| this.italic())
                            .text_size(size)
                            .text_color(color)
                            .child(shown),
                    )
                    .map(|this| {
                        if read_only {
                            this.tooltip(text_tooltip(t(M::OwnerOnlyAtom)))
                        } else {
                            this.cursor_pointer()
                                .child(
                                    div()
                                        .flex_none()
                                        .invisible()
                                        .group_hover("editable", |style| style.visible())
                                        .child(Icon::Pen.render(px(14.), ink::muted())),
                                )
                                .tooltip(text_tooltip(edit_label))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit(field, window, cx)
                                }))
                        }
                    })
                    .into_any_element()
            };
        let name = editable(EditField::Name, self, cx);
        let description = editable(EditField::Description, self, cx);
        let players = tr!(
            M::PlayerCount,
            players = instance
                .player_count
                .map(|n| n.to_string())
                .unwrap_or_default(),
            max = instance
                .max_player_count
                .map(|n| n.to_string())
                .unwrap_or_default()
        );
        let copy_address = address.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(name)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.))
                    .child(instance_icons::game_icon(
                        "overview-game",
                        &instance.game_type,
                        instance.icon_url.as_deref(),
                        px(24.),
                        cx,
                    ))
                    .child(label(
                        state_name(instance.state),
                        state_color(instance.state),
                    ))
                    .child(label(
                        tr!(M::VersionLabel, version = instance.version),
                        colors::BLUE_200,
                    ))
                    .child(label(players, colors::BLUE_200))
                    .child(
                        label(address, colors::BLUE_200)
                            .id("overview-address")
                            .font_family(fonts::mono())
                            .cursor_pointer()
                            .child(Icon::Copy.render(px(11.), colors::BLUE_200))
                            .tooltip(text_tooltip(t(M::CopyAddress)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let message = instance_icons::copied(&copy_address);
                                this.copy(copy_address.clone(), message, cx)
                            })),
                    ),
            )
            .child(description)
            .into_any_element()
    }

    fn render_share(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let share = self.share.clone()?;
        let body: AnyElement = if !share.sharing {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(16.))
                .child(
                    div()
                        .max_w(px(460.))
                        .text_size(text::MEDIUM)
                        .text_color(ink::secondary())
                        .child(t(M::ShareOff)),
                )
                .child(
                    Button::new("share-start", t(M::ShareStart))
                        .variant(ButtonVariant::Primary)
                        .icon(Icon::Link)
                        .loading(self.share_busy)
                        .disabled(self.share_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.start_share(cx))),
                )
                .into_any_element()
        } else {
            let (state_text, state_color) = match share.state {
                Some(ShareState::Sharing) => (t(M::ShareSharing), colors::GREEN),
                Some(ShareState::Reconnecting) => (t(M::ShareReconnecting), colors::YELLOW),
                _ => (t(M::ShareStarting), colors::GRAY_400),
            };
            let link: AnyElement = match share.link.clone() {
                Some(link) => {
                    let copy = link.clone();
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .id("share-link")
                                .flex_1()
                                .min_w_0()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(radius::DEFAULT)
                                .border_1()
                                .border_color(colors::GRAY_FADED.opacity(30))
                                .bg(colors::GRAY_900)
                                .font_family(fonts::mono())
                                .text_size(text::SMALL)
                                .text_color(ink::text())
                                .truncate()
                                .child(link)
                                .tooltip(text_tooltip(t(M::ShareLinkLabel))),
                        )
                        .child(
                            Button::new("share-copy", t(M::ShareCopy))
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::Copy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy(copy.clone(), t(M::ShareCopied).into(), cx)
                                })),
                        )
                        .into_any_element()
                }
                None => div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(LoadingDots)
                    .child(t(M::SharePreparing))
                    .into_any_element(),
            };
            let note = |text_: String, color: Color| {
                div()
                    .text_size(text::SMALL)
                    .font_weight(weights::MEDIUM.face())
                    .text_color(color)
                    .child(text_)
            };
            let friends: Vec<AnyElement> = share
                .friends
                .iter()
                .map(|friend| {
                    let connection = t(match friend.connection {
                        ShareConnection::SameComputer => M::ShareSameComputer,
                        ShareConnection::LocalNetwork => M::ShareLocalNetwork,
                        ShareConnection::Ipv6 => M::ShareIpv6,
                        ShareConnection::Internet | ShareConnection::Unknown => M::ShareInternet,
                    });
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .justify_between()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(8.))
                        .text_size(text::SMALL)
                        .child(
                            div()
                                .font_family(fonts::mono())
                                .text_color(ink::text())
                                .child(friend.address.clone()),
                        )
                        .child(div().text_color(ink::muted()).child(tr!(
                            M::ShareFriendLine,
                            connection = connection,
                            ms = friend.rtt_ms
                        )))
                        .into_any_element()
                })
                .collect();
            let friend_count = friends.len();
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(12.))
                        .child(label(state_text, state_color))
                        .when(share.stable_link, |this| {
                            this.child(note(t(M::ShareStableLink).into(), colors::GRAY_500))
                        }),
                )
                .child(link)
                // the link names its service: say so when it isn't connect.lodestone.cc,
                // so testers don't send a dev link to friends
                .when_some(share_relay_note(&share), |this, relay| {
                    this.child(note(relay, colors::YELLOW))
                })
                .child(note(t(M::ShareHowFriendsJoin).into(), colors::GRAY_500))
                .when_some(share.problem.clone(), |this, problem| {
                    this.child(note(problem, colors::YELLOW))
                })
                .when(share.hard_network, |this| {
                    this.child(note(t(M::ShareHardNetwork).into(), colors::YELLOW))
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .text_size(text::MEDIUM)
                        .font_weight(weights::MEDIUM.face())
                        .text_color(ink::text())
                        .child(Icon::UserGroup.render(px(14.), colors::GRAY_500))
                        .child(trn!(P::ShareConnected, friend_count))
                        .child(
                            div()
                                .text_size(text::SMALL)
                                .text_color(colors::GRAY_500)
                                .child(trn!(P::ShareJoins, share.joins)),
                        ),
                )
                .when(friend_count > 0, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(radius::DEFAULT)
                            .border_1()
                            .border_color(colors::GRAY_FADED.opacity(30))
                            .children(friends.into_iter().enumerate().map(|(i, row)| {
                                div()
                                    .when(i > 0, |this| {
                                        this.border_t_1()
                                            .border_color(colors::GRAY_FADED.opacity(30))
                                    })
                                    .child(row)
                            })),
                    )
                })
                .child(
                    div().flex().flex_row().justify_end().child(
                        Button::new("share-stop", t(M::ShareStop))
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let focus = cx.focus_handle();
                                this.stop_prompt.open(StopPrompt { focus }, cx);
                            })),
                    ),
                )
                .into_any_element()
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(heading(t(M::ShareTitle), t(M::ShareLead)))
                .child(card().child(body))
                .into_any_element(),
        )
    }

    fn stop_dialog(&self, prompt: &StopPrompt, cx: &mut Context<Self>) -> AnyElement {
        let close = std::rc::Rc::new(cx.listener(|this, _: &(), _, cx| {
            this.stop_prompt.close(cx);
        }));
        Dialog::new(
            "share-stop-dialog",
            t(M::ShareStopTitle),
            prompt.focus.clone(),
        )
        .on_close({
            let close = close.clone();
            move |window, cx| close(&(), window, cx)
        })
        .child(
            div()
                .text_size(text::MEDIUM)
                .text_color(ink::secondary())
                .child(t(M::ShareStopBody)),
        )
        .footer(
            Button::new("share-stop-confirm", t(M::ShareStop))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(|this, _, _, cx| this.stop_share(cx))),
        )
        .into_any_element()
    }

    fn render_performance(&self, instance: &InstanceInfo, cx: &Context<Self>) -> AnyElement {
        let total_ram = self.state.read(cx).info.total_ram;
        let waiting = self.received.is_none()
            && matches!(
                instance.state,
                InstanceState::Running | InstanceState::Starting
            );
        // padded with gaps at the start, so the newest is on the right
        let padded = |pick: &dyn Fn(&PerformanceReport) -> Option<f32>| -> Vec<Option<f32>> {
            let mut samples = vec![None; SAMPLES.saturating_sub(self.reports.len())];
            samples.extend(self.reports.iter().map(pick));
            samples
        };
        let cpu = padded(&|r| r.cpu_usage.map(|c| (c * 10.0).round() / 10.0));
        let memory = padded(&|r| {
            r.memory_usage
                .map(|m| ((m as f64 / GIB) * 10.0).round() as f32 / 10.0)
        });
        let latest = self.reports.back();
        let one_decimal = |v: f64| lodestone_ui::i18n::format::decimal(v, 1);
        let cpu_now = latest
            .and_then(|r| r.cpu_usage)
            .map(|c| one_decimal(c as f64))
            .unwrap_or_else(|| "0".into());
        let memory_now = one_decimal(
            latest
                .and_then(|r| r.memory_usage)
                .map(|m| m as f64 / GIB)
                .unwrap_or(0.0),
        );
        let max_gib = ((total_ram as f64 / GIB) * 10.0).round() / 10.0;
        // the light theme's darker green stands out on white, as the dashboard's
        let (line, fill) = if lodestone_ui::theme::palette::is_light() {
            (Color::rgb(0x1D7A3A), Color::rgb(0x1D7A3A).opacity(12))
        } else {
            (Color::rgb(0x62DD76), colors::GREEN_FADED.opacity(25))
        };
        let graph = |id: &'static str,
                     title: String,
                     samples: Vec<Option<f32>>,
                     max: f32,
                     unit: &'static str,
                     max_label: String| {
            let body: AnyElement = if waiting {
                div()
                    .h(px(160.))
                    .w_full()
                    .rounded(radius::MD)
                    .bg(colors::GRAY_800)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_size(text::SMALL)
                    .text_color(ink::muted())
                    .child(LoadingDots)
                    .child(t(M::LoadingGraph))
                    .into_any_element()
            } else {
                LineChart::new(id, samples, max)
                    .colors(line, fill)
                    .x_labels(tr!(M::GraphSeconds, seconds = SAMPLES - 1), "0")
                    .y_labels(max_label, "0")
                    .hover_label(move |ago, value| {
                        tr!(
                            M::GraphSecondsAgo,
                            seconds = ago,
                            value = format!(
                                "{}{unit}",
                                lodestone_ui::i18n::format::decimal(value as f64, 1)
                            )
                        )
                        .into()
                    })
                    .accessible_label(title.clone())
                    .into_any_element()
            };
            div()
                .flex_1()
                .min_w(px(260.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(text::SMALL)
                        .font_weight(weights::MEDIUMBOLD.face())
                        .text_color(colors::GRAY_FADED.opacity(60))
                        .child(title),
                )
                .child(
                    div()
                        .p(px(12.))
                        .rounded(radius::LG)
                        .bg(colors::GRAY_900)
                        .child(body),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(heading(t(M::PerformanceTitle), t(M::PerformanceLead)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(24.))
                    .child(graph(
                        "cpu-graph",
                        tr!(M::CpuLoad, value = cpu_now),
                        cpu,
                        100.0,
                        t(M::UnitPercent),
                        format!("100{}", t(M::UnitPercent)),
                    ))
                    .child(graph(
                        "memory-graph",
                        tr!(M::MemoryUsed, value = memory_now),
                        memory,
                        max_gib.max(0.1) as f32,
                        t(M::UnitGiB),
                        format!("{}{}", one_decimal(max_gib), t(M::UnitGiB)),
                    )),
            )
            .into_any_element()
    }

    fn render_players(&mut self, instance: &InstanceInfo, cx: &mut Context<Self>) -> AnyElement {
        let mut players = instance.player_list.clone().unwrap_or_default();
        players.sort_by_key(|p| p.name().to_lowercase());
        if !self.sort_ascending {
            players.reverse();
        }
        let rows: Vec<AnyElement> = players
            .iter()
            .enumerate()
            .map(|(i, player)| {
                let uuid = match player {
                    Player::MinecraftPlayer { uuid, .. } => uuid.clone(),
                    _ => None,
                };
                let head = uuid
                    .as_deref()
                    .and_then(|u| instance_icons::player_head(u, cx));
                let linked = self
                    .linked
                    .user_of(player.name(), uuid.as_deref())
                    .map(str::to_string);
                div()
                    .id(("player", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .when(i > 0, |this| this.border_t_1().border_color(ink::border()))
                    .child(match head {
                        Some(head) => img(head).size(px(16.)).flex_none().into_any_element(),
                        None => Icon::CircleUser
                            .render(px(16.), ink::faint())
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .text_size(text::MEDIUM)
                            .text_color(ink::text())
                            .child(player.name().to_string()),
                    )
                    .when_some(linked, |this, user| {
                        this.child(
                            div()
                                .id(("linked", i))
                                .text_size(text::SMALL)
                                .font_weight(weights::MEDIUM.face())
                                .text_color(ink::muted())
                                .child(user)
                                .tooltip(text_tooltip(t(M::PlayersLinkedUser))),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let list: AnyElement = if rows.is_empty() {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .px(px(12.))
                .py(px(10.))
                .rounded(radius::LG)
                .border_2()
                .border_dashed()
                .border_color(ink::border())
                .child(Icon::CircleUser.render(px(16.), ink::muted()))
                .child(
                    div()
                        .text_size(text::MEDIUM)
                        .italic()
                        .text_color(ink::muted())
                        .child(t(M::PlayersNone)),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .rounded(radius::LG)
                .border_1()
                .border_color(ink::border())
                .bg(colors::GRAY_850)
                .children(rows)
                .into_any_element()
        };
        let ascending = self.sort_ascending;
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(t(M::PlayersTitle), t(M::PlayersLead)))
            .child(
                div()
                    .id("players-sort")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .ml(px(8.))
                    .cursor_pointer()
                    .text_size(text::SMALL)
                    .font_weight(weights::MEDIUM.face())
                    .text_color(ink::muted())
                    .hover(|style| style.text_color(ink::text()))
                    .child(t(M::PlayersSortByName))
                    .child(
                        if ascending {
                            Icon::ArrowDown
                        } else {
                            Icon::ArrowUp
                        }
                        .render(px(10.), ink::muted()),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sort_ascending = !this.sort_ascending;
                        cx.notify();
                    })),
            )
            .child(list)
            .into_any_element()
    }

    fn render_details(&self, instance: &InstanceInfo, cx: &mut Context<Self>) -> AnyElement {
        let open = DETAILS_OPEN.load(Ordering::Relaxed);
        let yes_no =
            |value: bool| t(if value { CommonMsg::Yes } else { CommonMsg::No }).to_string();
        let is_minecraft = matches!(
            instance.game_type,
            Game::MinecraftJava { .. } | Game::MinecraftBedrock
        );
        let created = (instance.creation_time > 0)
            .then(|| chrono::DateTime::from_timestamp(instance.creation_time, 0))
            .flatten()
            .map(|time| {
                let local = time.with_timezone(&chrono::Local);
                lodestone_ui::i18n::format::day_and_time(local, chrono::Local::now().date_naive())
            });
        let ram_value = |mb: Option<u32>| {
            mb.map_or("?".to_string(), |mb| {
                lodestone_ui::i18n::format::number(mb.into())
            })
        };
        let ram = (instance.min_ram.is_some() || instance.max_ram.is_some()).then(|| {
            tr!(
                M::DetailsRamValue,
                min = ram_value(instance.min_ram),
                max = ram_value(instance.max_ram)
            )
        });
        let players = (instance.player_count.is_some() || instance.max_player_count.is_some())
            .then(|| {
                let n = |v: Option<u32>| v.map_or("?".into(), |v| v.to_string());
                format!(
                    "{} / {}",
                    n(instance.player_count),
                    n(instance.max_player_count)
                )
            });
        let loader = loader_name(instance).map(|loader| match &instance.loader_version {
            Some(version) => format!("{loader} {version}"),
            None => loader,
        });
        let copyable = |id: &'static str, value: String, cx: &mut Context<Self>| {
            let copy = value.clone();
            div()
                .id(id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .font_family(fonts::mono())
                .text_size(text::SMALL)
                .text_color(ink::text())
                .hover(|style| style.text_color(colors::BLUE_200))
                .child(div().min_w_0().child(value))
                .child(Icon::Copy.render(px(10.), ink::muted()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let message = instance_icons::copied(&copy);
                    this.copy(copy.clone(), message, cx)
                }))
                .into_any_element()
        };
        let plain = |value: String| div().child(value).into_any_element();
        let modpack = instance.modpack.as_ref().map(|modpack| {
            let label = match &modpack.version_number {
                Some(version) => format!("{} {version}", modpack.name),
                None => modpack.name.clone(),
            };
            if modpack.project_id.is_empty() {
                return plain(label);
            }
            let url = format!(
                "https://modrinth.com/modpack/{}",
                launcher_core::cores::admin::segment(&modpack.project_id)
            );
            div()
                .id("details-modpack")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_color(colors::BLUE_200)
                .hover(|style| style.underline())
                .child(label)
                .child(Icon::ArrowUpRightFromSquare.render(px(10.), colors::BLUE_200))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .into_any_element()
        });
        let rows: Vec<(&'static str, Option<AnyElement>)> = vec![
            (t(M::DetailsName), Some(plain(instance.name.clone()))),
            (
                t(M::DetailsDescription),
                (!instance.description.is_empty()).then(|| plain(instance.description.clone())),
            ),
            (
                t(M::DetailsUuid),
                Some(copyable("details-uuid", instance.uuid.clone(), cx)),
            ),
            (
                t(M::DetailsGame),
                Some(plain(instance_icons::game_title(&instance.game_type))),
            ),
            (
                t(if is_minecraft {
                    M::DetailsMinecraftVersion
                } else {
                    M::DetailsVersion
                }),
                (!instance.version.is_empty()).then(|| plain(instance.version.clone())),
            ),
            (t(M::DetailsLoader), loader.map(plain)),
            (t(M::DetailsPort), Some(plain(instance.port.to_string()))),
            (
                t(M::DetailsState),
                Some(plain(state_name(instance.state).into())),
            ),
            (t(M::DetailsPlayers), players.map(plain)),
            (t(M::DetailsCreated), created.map(plain)),
            (
                t(M::DetailsAutoStart),
                Some(plain(yes_no(instance.auto_start))),
            ),
            (
                t(M::DetailsRestartOnCrash),
                Some(plain(yes_no(instance.restart_on_crash))),
            ),
            (t(M::DetailsRam), ram.map(plain)),
            (
                t(M::DetailsFolder),
                (!instance.path.is_empty())
                    .then(|| copyable("details-folder", instance.path.clone(), cx)),
            ),
            (t(M::DetailsModpack), modpack),
        ];
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .id("details-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .text_size(text::H3)
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::secondary())
                    .hover(|style| style.text_color(ink::text()))
                    .child(
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        }
                        .render(px(12.), ink::muted()),
                    )
                    .child(t(M::DetailsTitle))
                    .on_click(cx.listener(|_, _, _, cx| {
                        DETAILS_OPEN.fetch_xor(true, Ordering::Relaxed);
                        cx.notify();
                    })),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .p(px(16.))
                        .rounded(radius::LG)
                        .border_1()
                        .border_color(ink::border())
                        .bg(colors::GRAY_850)
                        .text_size(text::MEDIUM)
                        .children(rows.into_iter().filter_map(|(name, value)| {
                            let value = value?;
                            Some(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(24.))
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(160.))
                                            .text_color(ink::muted())
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_color(ink::text())
                                            .child(value),
                                    ),
                            )
                        })),
                )
            })
            .into_any_element()
    }
}

impl Render for OverviewPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(instance) = self.instance(cx) else {
            return div()
                .text_size(text::H2)
                .text_color(ink::muted())
                .child(t(M::NotFound))
                .into_any_element();
        };
        let header = self.render_header(&instance, cx);
        let share = self.render_share(cx);
        let performance = self.render_performance(&instance, cx);
        let players = self.render_players(&instance, cx);
        let details = self.render_details(&instance, cx);
        let dialogs = self
            .stop_prompt
            .render(window, |prompt| Some(self.stop_dialog(prompt, cx)));
        div()
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(32.))
            .pb(px(32.))
            .child(div().max_w(px(900.)).child(header))
            .children(share.map(|share| div().max_w(px(900.)).child(share)))
            .child(div().max_w(px(900.)).child(performance))
            .child(div().max_w(px(900.)).child(players))
            .child(div().max_w(px(900.)).child(details))
            .children(window_layer(window, dialogs))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_through_another_service_says_so() {
        let secret = format!("{}.{}", "ab".repeat(32), "01".repeat(16));
        let mut share = InstanceShare {
            link: Some(format!(
                "https://connect.lodestone.cc/j/ABCDEFGH23#{secret}"
            )),
            ..Default::default()
        };
        assert_eq!(share_relay_note(&share), None, "the default service");
        share.link = Some(format!(
            "https://connect-dev.lodestone.cc/j/ABCDEFGH23#{secret}"
        ));
        assert_eq!(
            share_relay_note(&share).as_deref(),
            Some("Dev relay: connect-dev.lodestone.cc")
        );
        share.link = None;
        assert_eq!(share_relay_note(&share), None);
    }

    #[test]
    fn sharing_is_checked_as_often_as_the_dashboard_does() {
        let mut share = InstanceShare::default();
        assert_eq!(share_poll_interval(&share), None, "not sharing: once");
        share.sharing = true;
        share.state = Some(ShareState::Starting);
        assert_eq!(share_poll_interval(&share), Some(Duration::from_secs(2)));
        share.state = Some(ShareState::Sharing);
        assert_eq!(share_poll_interval(&share), Some(Duration::from_secs(5)));
    }
}
