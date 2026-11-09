//! What the Host knows about the core it is signed in to, kept up to date from the
//! core's event stream: its details, the signed-in user, the instances and their live
//! state, the core's settings, and the connection.
//!
//! Pages read it and subscribe to its [`HostEvent`]s; they fetch what only they show
//! (a console, a folder) themselves, through [`HostState::admin`].

use gpui::{AppContext as _, Context, Entity, EventEmitter, Task};
use launcher_core::cores::{
    CoreError,
    admin::{
        AdminClient, ClientEvent, EventInner, FullCoreInfo, InstanceInfo, InstanceState,
        PublicUser,
        events::{
            InstanceEventInner, ProgressionEndValue, ProgressionEventInner, ProgressionStartValue,
            UserEventInner,
        },
        instances::Power,
        models::GlobalSettingsData,
    },
    live::{LiveMessage, LiveStream, StreamKind},
};

use lodestone_ui::components::{Toast, Toasts};

use crate::session::Session;

/// The connection to the core, as the footer shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    Connecting,
    Live,
    /// The event stream dropped and the core doesn't answer either; the stream
    /// reconnects by itself.
    Reconnecting(String),
    /// The core answers requests, but the event stream keeps failing, so live updates
    /// stopped (the dashboard's `degraded`).
    Degraded(String),
    /// The core refused the session.
    SignedOut,
}

/// An instance being made, from its progress events.
#[derive(Clone, Debug, PartialEq)]
pub struct Creating {
    pub event_id: i64,
    pub name: String,
    pub progress: f64,
    pub total: Option<f64>,
}

/// What pages hear from the state.
#[derive(Clone, Debug)]
pub enum HostEvent {
    /// An event from the core, after the state has applied it.
    Core(Box<ClientEvent>),
    /// The instance list was fetched again.
    InstancesReloaded,
    /// The session ended: the window signs in again.
    SignedOut,
    /// Show another core (package D's Cores dialog): `None` for this computer's, or a
    /// session on another core.
    SwitchCore(Option<Box<crate::session::Session>>),
    /// A page asks the shell to show another route (see [`HostState::navigate`]).
    Navigate(crate::routes::Route),
    /// Something asks for the Settings dialog, on a page (see
    /// [`HostState::open_settings`]).
    OpenSettings(crate::routes::SettingsPage),
    /// A page asks for the "New server" dialog.
    OpenCreateInstance,
    /// A page asks for the "New server" dialog on a Modrinth modpack (Mods › Browse:
    /// "Create server from this modpack").
    OpenCreateModpack(crate::pages::create_instance::ModpackPreset),
    /// Starting (or restarting) the instance failed: its Minecraft EULA may not be
    /// accepted (`pages::eula_prompt` asks).
    StartFailed(String),
}

pub struct HostState {
    pub admin: AdminClient,
    pub info: FullCoreInfo,
    pub me: PublicUser,
    /// In the core's order. `None` until the first list arrives.
    pub instances: Option<Vec<InstanceInfo>>,
    pub settings: Option<GlobalSettingsData>,
    pub connection: Connection,
    pub creating: Vec<Creating>,
    /// Why the last list or power request failed.
    pub error: Option<String>,
    /// An instance whose Overview should start sharing it with friends when it opens
    /// (the sidebar's "Share" button).
    pub share_requested: Option<String>,
    /// The window's toasts, which the shell draws over the page (see [`Self::toast`]).
    pub toasts: Entity<Toasts>,
    _stream: Option<LiveStream>,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<HostEvent> for HostState {}

impl HostState {
    pub fn new(session: Session, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            admin: session.admin,
            info: session.info,
            me: session.me,
            instances: None,
            settings: None,
            connection: Connection::Connecting,
            creating: Vec::new(),
            error: None,
            toasts: cx.new(|_| Toasts::new()),
            _stream: None,
            share_requested: None,
            _tasks: Vec::new(),
        };
        this.follow_events(cx);
        this.reload(cx);
        this.expire_with_token(cx);
        this
    }

    /// The instance `uuid`, if the user may see it.
    pub fn instance(&self, uuid: &str) -> Option<&InstanceInfo> {
        self.instances.as_ref()?.iter().find(|i| i.uuid == uuid)
    }

    /// Asks the shell to show `route`: for a page that leads elsewhere.
    pub fn navigate(&mut self, route: crate::routes::Route, cx: &mut Context<Self>) {
        cx.emit(HostEvent::Navigate(route));
    }

    /// Asks the shell for the Settings dialog, on `page`, over the page shown: for a
    /// link to a setting (Account, Updates, …).
    pub fn open_settings(&mut self, page: crate::routes::SettingsPage, cx: &mut Context<Self>) {
        cx.emit(HostEvent::OpenSettings(page));
    }

    /// Asks the shell for the "New server" dialog.
    pub fn open_create_instance(&mut self, cx: &mut Context<Self>) {
        cx.emit(HostEvent::OpenCreateInstance);
    }

    /// Asks the shell for the "New server" dialog on a Modrinth modpack, skipping the
    /// search (and the version, when `preset` has one).
    pub fn open_create_modpack(
        &mut self,
        preset: crate::pages::create_instance::ModpackPreset,
        cx: &mut Context<Self>,
    ) {
        cx.emit(HostEvent::OpenCreateModpack(preset));
    }

    /// Changes the instance `uuid` as the core just did (a new name), without fetching
    /// the list again.
    pub fn update_instance(
        &mut self,
        uuid: &str,
        change: impl FnOnce(&mut InstanceInfo),
        cx: &mut Context<Self>,
    ) {
        if let Some(instance) = self
            .instances
            .as_mut()
            .and_then(|list| list.iter_mut().find(|i| i.uuid == uuid))
        {
            change(instance);
            cx.notify();
        }
    }

    /// Shows a toast at the bottom right of the window: `Toast::success(…)`,
    /// `Toast::error(…)`, with a key to replace an earlier one, or an action button.
    pub fn toast(&mut self, toast: Toast, cx: &mut Context<Self>) {
        self.toasts.update(cx, |toasts, cx| {
            toasts.push(toast, cx);
        });
    }

    /// Fetches the instances, the core's settings and the user again.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let work = cx.background_spawn(async move {
                let instances = admin.instances().await;
                let settings = admin.global_settings().await.ok();
                let me = admin.me().await.ok();
                (instances, settings, me)
            });
            let (instances, settings, me) = work.await;
            this.update(cx, |this, cx| {
                match instances {
                    Ok(instances) => {
                        this.instances = Some(instances);
                        this.error = None;
                    }
                    Err(CoreError::SignedOut) => return this.signed_out(cx),
                    Err(err) => this.error = Some(err.to_string()),
                }
                if settings.is_some() {
                    this.settings = settings;
                }
                if let Some(me) = me {
                    this.me = me;
                }
                cx.emit(HostEvent::InstancesReloaded);
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    /// Starts, stops, restarts or kills an instance. The new state arrives as an event.
    pub fn power(&mut self, uuid: &str, power: Power, cx: &mut Context<Self>) {
        let admin = self.admin.clone();
        let uuid = uuid.to_string();
        let task = cx.spawn(async move |this, cx| {
            let result = {
                let uuid = uuid.clone();
                cx.background_spawn(async move { admin.power(&uuid, power).await })
                    .await
            };
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.error = None,
                    Err(CoreError::SignedOut) => return this.signed_out(cx),
                    Err(err) => {
                        this.error = Some(err.to_string());
                        if matches!(power, Power::Start | Power::Restart) {
                            cx.emit(HostEvent::StartFailed(uuid));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    fn signed_out(&mut self, cx: &mut Context<Self>) {
        self.connection = Connection::SignedOut;
        self._stream = None;
        cx.emit(HostEvent::SignedOut);
        cx.notify();
    }

    /// The event stream failed: asks the core for the user, as the dashboard does,
    /// to tell a core that is down (still reconnecting) from one whose live updates
    /// alone fail (degraded), and from a session that ended.
    fn probe(&mut self, reason: String, cx: &mut Context<Self>) {
        let admin = self.admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let answer = cx.background_spawn(async move { admin.me().await }).await;
            this.update(cx, |this, cx| {
                // the stream came back meanwhile
                if !matches!(this.connection, Connection::Reconnecting(_)) {
                    return;
                }
                match connection_after_probe(answer.map(|_| ()), reason) {
                    Some(connection) => this.connection = connection,
                    None => return this.signed_out(cx),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    /// Opens the event stream again and fetches everything: the banner's Reload when
    /// live updates stopped.
    pub fn reconnect(&mut self, cx: &mut Context<Self>) {
        self._stream = None;
        self.connection = Connection::Connecting;
        self.follow_events(cx);
        self.reload(cx);
        cx.notify();
    }

    /// Ends the session when the core's sign-in token expires (the dashboard's
    /// `sessionExpired`): the window then signs in again, as when the core refuses it.
    fn expire_with_token(&mut self, cx: &mut Context<Self>) {
        let Some(expires) = crate::session::token_expiry(self.admin.token().expose()) else {
            return;
        };
        let now = chrono::Utc::now().timestamp();
        let left = std::time::Duration::from_secs(expires.saturating_sub(now).max(0) as u64);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(left).await;
            this.update(cx, |this, cx| {
                if this.connection != Connection::SignedOut {
                    this.signed_out(cx);
                }
            })
            .ok();
        });
        self._tasks.push(task);
    }

    /// Follows every event the user may see.
    fn follow_events(&mut self, cx: &mut Context<Self>) {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        self._stream = Some(LiveStream::open(
            self.admin.core(),
            self.admin.token().clone(),
            "all",
            StreamKind::All,
            tx,
        ));
        let task = cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(message) = rx.next().await {
                let alive = this
                    .update(cx, |this, cx| this.on_message(message, cx))
                    .is_ok();
                if !alive {
                    break;
                }
            }
        });
        self._tasks.push(task);
    }

    fn on_message(&mut self, message: LiveMessage, cx: &mut Context<Self>) {
        match message {
            LiveMessage::Connected => {
                let reconnected = self.connection != Connection::Connecting;
                self.connection = Connection::Live;
                // fill in what was missed while disconnected
                if reconnected {
                    self.reload(cx);
                }
            }
            LiveMessage::Disconnected(reason) => {
                self.connection = Connection::Reconnecting(reason.clone());
                self.probe(reason, cx);
            }
            LiveMessage::Frame(text) => {
                if let Some(event) = launcher_core::cores::admin::events::parse(&text) {
                    self.apply(&event, cx);
                    cx.emit(HostEvent::Core(Box::new(event)));
                }
            }
            LiveMessage::Ended(CoreError::SignedOut) => return self.signed_out(cx),
            LiveMessage::Ended(err) => {
                self.connection = Connection::Reconnecting(err.to_string());
                self.probe(err.to_string(), cx);
            }
            LiveMessage::State(_) | LiveMessage::Players(_) | LiveMessage::Output(_) => {}
        }
        cx.notify();
    }

    /// Applies an event to what the state holds, as the dashboard updates its cache.
    pub fn apply(&mut self, event: &ClientEvent, cx: &mut Context<Self>) {
        match &event.event_inner {
            EventInner::InstanceEvent(inner) => {
                let Some(instance) = self
                    .instances
                    .as_mut()
                    .and_then(|list| list.iter_mut().find(|i| i.uuid == inner.instance_uuid))
                else {
                    return;
                };
                match &inner.instance_event_inner {
                    InstanceEventInner::StateTransition { to } => {
                        instance.state = *to;
                        if !to.is_live() {
                            instance.player_count = None;
                            instance.player_list = None;
                        }
                    }
                    InstanceEventInner::InstanceError { .. } => {
                        instance.state = InstanceState::Error;
                    }
                    InstanceEventInner::PlayerChange { player_list, .. } => {
                        instance.player_count = Some(player_list.len() as u32);
                        instance.player_list = Some(player_list.clone());
                    }
                    _ => {}
                }
            }
            EventInner::ProgressionEvent(progression) => {
                let id = progression.event_id;
                match &progression.progression_event_inner {
                    ProgressionEventInner::ProgressionStart {
                        progression_name,
                        total,
                        inner: Some(ProgressionStartValue::InstanceCreation { .. }),
                    } => self.creating.push(Creating {
                        event_id: id,
                        name: progression_name.clone(),
                        progress: 0.0,
                        total: *total,
                    }),
                    ProgressionEventInner::ProgressionUpdate { progress, .. } => {
                        if let Some(creating) = self.creating.iter_mut().find(|c| c.event_id == id)
                        {
                            creating.progress += progress;
                        }
                    }
                    ProgressionEventInner::ProgressionEnd { inner, .. } => {
                        self.creating.retain(|c| c.event_id != id);
                        match inner {
                            Some(ProgressionEndValue::InstanceCreation(info)) => {
                                if let Some(list) = self.instances.as_mut()
                                    && !list.iter().any(|i| i.uuid == info.uuid)
                                {
                                    list.push((**info).clone());
                                }
                            }
                            Some(ProgressionEndValue::InstanceDelete { instance_uuid }) => {
                                if let Some(list) = self.instances.as_mut() {
                                    list.retain(|i| &i.uuid != instance_uuid);
                                }
                            }
                            // zipping or unzipping finished: say how it went, whatever
                            // the page (the Files tab lists its folder again)
                            Some(ProgressionEndValue::FsOperationCompleted {
                                success,
                                message,
                                ..
                            }) => {
                                let toast = if *success {
                                    Toast::success(message.clone())
                                } else {
                                    Toast::error(message.clone())
                                };
                                self.toast(toast, cx);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            EventInner::UserEvent(user) if user.user_id == self.me.uid => {
                match &user.user_event_inner {
                    UserEventInner::UsernameChanged { new_username } => {
                        self.me.username = new_username.clone();
                    }
                    UserEventInner::PermissionChanged { new_permissions } => {
                        self.me.permissions = new_permissions.clone();
                        // what the user may see may have changed
                        self.reload(cx);
                    }
                    UserEventInner::UserDeleted => self.signed_out(cx),
                    _ => {}
                }
            }
            EventInner::GlobalSettingsChanged => self.reload(cx),
            _ => {}
        }
    }
}

/// The connection after the event stream failed and the core was asked for the user:
/// it answers (live updates alone fail: degraded), it doesn't (still reconnecting), or
/// it refuses the session (`None`: sign out).
pub fn connection_after_probe(answer: Result<(), CoreError>, reason: String) -> Option<Connection> {
    match answer {
        Ok(()) => Some(Connection::Degraded(reason)),
        Err(CoreError::SignedOut) => None,
        Err(_) => Some(Connection::Reconnecting(reason)),
    }
}

/// What the power button offers for an instance in `state`, as the dashboard's
/// instance card does.
pub fn power_actions(state: InstanceState) -> &'static [Power] {
    match state {
        InstanceState::Stopped => &[Power::Start],
        InstanceState::Running => &[Power::Stop, Power::Restart],
        InstanceState::Starting | InstanceState::Stopping => &[Power::Kill],
        InstanceState::Error => &[Power::Restart],
        InstanceState::Unknown => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_stream_is_degraded_only_while_the_core_answers() {
        let reason = || "stream closed".to_string();
        assert_eq!(
            connection_after_probe(Ok(()), reason()),
            Some(Connection::Degraded(reason()))
        );
        assert_eq!(
            connection_after_probe(Err(CoreError::Unreachable("refused".into())), reason()),
            Some(Connection::Reconnecting(reason()))
        );
        assert_eq!(
            connection_after_probe(Err(CoreError::SignedOut), reason()),
            None
        );
    }

    #[test]
    fn power_follows_the_state() {
        assert_eq!(power_actions(InstanceState::Stopped), [Power::Start]);
        assert_eq!(
            power_actions(InstanceState::Running),
            [Power::Stop, Power::Restart]
        );
        assert_eq!(power_actions(InstanceState::Starting), [Power::Kill]);
        assert_eq!(power_actions(InstanceState::Error), [Power::Restart]);
    }
}
