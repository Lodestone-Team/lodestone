//! A server's page, opened from its row on a signed-in core: live state and players
//! from the core's event stream, the console when this device's key may read it, a
//! chat box when it may write to it, and linking the selected Minecraft account to the
//! Lodestone user. How it looks is in [`super::detail_view`].
//!
//! What the key may do comes from the core (`/launcher/instances/{uuid}/access`): the
//! console opt-ins chosen at sign-in, within the user's own permissions. The streams
//! run on their own threads and stop when the page closes.

use futures::{StreamExt as _, channel::mpsc};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, SharedString, Task,
    UniformListScrollHandle,
};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError, ServerInstance, ServerState,
        live::{
            self, ConsoleLine, LauncherAccess, LiveMessage, LiveStream, MinecraftLink, Player,
            StreamKind,
        },
    },
    game::launch::UserType,
};
use lodestone_minecraft::console_format::{ParsedConsoleLine, parse_console_line};

use super::core_error;
use crate::{
    accounts::Accounts,
    components::TextInput,
    i18n::{CommonMsg, ServersMsg as M, t, tr},
};

/// How a stream is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamStatus {
    Connecting,
    Live,
    /// Dropped; trying again. Why, for people (in English: the connection's error).
    Reconnecting(SharedString),
    /// Not followed.
    Off(OffReason),
}

/// Why a stream isn't followed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OffReason {
    /// The console wasn't asked for at sign-in (the screenshots say so).
    NotAllowedAtSignIn,
    /// This device's key may not read the console.
    ConsoleNotAllowed,
    Failed(CoreError),
}

impl OffReason {
    /// Why, for people, in the current language.
    pub fn label(&self) -> String {
        let computer = t(CommonMsg::ThisComputer);
        match self {
            OffReason::NotAllowedAtSignIn => {
                tr!(M::ConsoleNotAllowedAtSignIn, computer = computer)
            }
            OffReason::ConsoleNotAllowed => tr!(
                M::ConsoleKeyCantRead,
                computer = computer,
                option = t(M::ReadConsole)
            ),
            OffReason::Failed(err) => core_error(err),
        }
    }
}

/// A console line, parsed once.
pub struct ConsoleRow {
    pub snowflake: i64,
    pub line: ParsedConsoleLine,
}

pub enum DetailEvent {
    /// Back to the list.
    Close,
    /// Start or stop the server, as its row on the list does.
    Start,
    Stop,
    /// The core refused the key: sign the core out.
    SignedOut,
}

/// Whether the server's Minecraft EULA is accepted, for those who may start it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EulaState {
    /// Not known: not asked yet, a core too old to say, or this key can't start it.
    #[default]
    Unknown,
    Accepted,
    /// The server can't start until someone accepts it.
    NotAccepted,
    Accepting,
    /// The user just accepted it here.
    JustAccepted,
    /// Accepting failed: why, for people.
    Failed(SharedString),
}

/// The Minecraft account part of the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkState {
    Loading,
    /// The core is too old to link accounts.
    Unsupported,
    Known(Vec<MinecraftLink>),
    Failed(CoreError),
}

pub struct ServerDetail {
    pub core_uuid: String,
    pub core_name: String,
    /// The Lodestone user the key belongs to.
    pub username: Option<String>,
    client: Option<CoreClient>,
    key: Secret,
    accounts: Entity<Accounts>,
    pub server: ServerInstance,
    pub players: Option<Vec<Player>>,
    /// `None` until the core says.
    pub access: Option<LauncherAccess>,
    pub events_status: StreamStatus,
    pub console_status: StreamStatus,
    pub console: Vec<ConsoleRow>,
    pub chat: Entity<TextInput>,
    pub sending: bool,
    pub chat_error: Option<SharedString>,
    pub links: LinkState,
    pub linking: bool,
    /// The server's Minecraft EULA.
    pub eula: EulaState,
    /// The result of the last link or unlink: `(is_error, message)`.
    pub link_notice: Option<(bool, SharedString)>,
    pub scroll: UniformListScrollHandle,
    /// Whether the console follows new lines.
    pub follow: bool,
    /// The number of console rows last scrolled to.
    pub seen_rows: usize,
    pub focus: FocusHandle,
    streams: Vec<LiveStream>,
    tasks: Vec<Task<()>>,
}

impl EventEmitter<DetailEvent> for ServerDetail {}

impl ServerDetail {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: CoreClient,
        key: Secret,
        core_uuid: String,
        core_name: String,
        username: Option<String>,
        server: ServerInstance,
        accounts: Entity<Accounts>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(core_uuid, core_name, username, server, key, accounts, cx);
        this.client = Some(client);
        this.start(cx);
        this
    }

    fn empty(
        core_uuid: String,
        core_name: String,
        username: Option<String>,
        server: ServerInstance,
        key: Secret,
        accounts: Entity<Accounts>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            core_uuid,
            core_name,
            username,
            client: None,
            key,
            accounts,
            server,
            players: None,
            access: None,
            events_status: StreamStatus::Connecting,
            console_status: StreamStatus::Connecting,
            console: Vec::new(),
            chat: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder(M::ChatPlaceholder)
                    .with_automation_id("server-chat")
            }),
            sending: false,
            chat_error: None,
            links: LinkState::Loading,
            linking: false,
            eula: EulaState::Unknown,
            link_notice: None,
            scroll: UniformListScrollHandle::new(),
            follow: true,
            seen_rows: 0,
            focus: cx.focus_handle(),
            streams: Vec::new(),
            tasks: Vec::new(),
        }
    }

    /// A page with made-up contents, for the offscreen screenshots: nothing is fetched.
    #[allow(clippy::too_many_arguments)]
    pub fn preview(
        core_name: &str,
        username: &str,
        server: ServerInstance,
        access: LauncherAccess,
        players: Vec<Player>,
        console: &[&str],
        links: Vec<MinecraftLink>,
        accounts: Entity<Accounts>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(
            "preview".into(),
            core_name.into(),
            Some(username.into()),
            server,
            Secret::new("preview"),
            accounts,
            cx,
        );
        this.access = Some(access);
        this.players = Some(players);
        this.events_status = StreamStatus::Live;
        this.console_status = if access.console_read {
            StreamStatus::Live
        } else {
            StreamStatus::Off(OffReason::NotAllowedAtSignIn)
        };
        this.push_console(console.iter().enumerate().map(|(i, message)| ConsoleLine {
            snowflake: i as i64 + 1,
            message: (*message).to_string(),
        }));
        this.links = LinkState::Known(links);
        this
    }

    /// Fetches what the key may do, then follows the streams it may follow.
    fn start(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = self.key.clone();
        let uuid = self.server.uuid.clone();
        let fetch = cx.background_spawn(async move {
            let access = client.access(&key, &uuid).await;
            let players = client.players(&key, &uuid).await.ok();
            // a core too old to say, or a server that isn't Minecraft: nothing to show
            let eula = client.eula_accepted(&key, &uuid).await.ok();
            (access, players, eula)
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let (access, players, eula) = fetch.await;
            this.update(cx, |this, cx| {
                this.eula = match eula {
                    Some(true) => EulaState::Accepted,
                    Some(false) => EulaState::NotAccepted,
                    None => EulaState::Unknown,
                };
                match access {
                    Ok(access) => {
                        // a core without the route: viewing, starting and stopping as the
                        // list shows, and no console
                        let access = access.unwrap_or(LauncherAccess {
                            view: true,
                            start: true,
                            stop: true,
                            console_read: false,
                            console_write: false,
                        });
                        this.access = Some(access);
                        this.follow_stream(StreamKind::Events, cx);
                        if access.console_read {
                            this.follow_stream(StreamKind::Console, cx);
                        } else {
                            this.console_status = StreamStatus::Off(OffReason::ConsoleNotAllowed);
                        }
                    }
                    Err(CoreError::SignedOut) => cx.emit(DetailEvent::SignedOut),
                    Err(err) => {
                        this.events_status = StreamStatus::Off(OffReason::Failed(err.clone()));
                        this.console_status = StreamStatus::Off(OffReason::Failed(err));
                    }
                }
                if this.players.is_none() {
                    this.players = players;
                }
                cx.notify();
            })
            .ok();
        }));
        self.load_links(cx);
    }

    /// "Accept the Minecraft EULA": the user agreed, with their own click. Never called
    /// for them.
    pub fn accept_eula(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if !matches!(self.eula, EulaState::NotAccepted | EulaState::Failed(_)) {
            return;
        }
        self.eula = EulaState::Accepting;
        let key = self.key.clone();
        let uuid = self.server.uuid.clone();
        let work = cx.background_spawn(async move { client.accept_eula(&key, &uuid).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.eula = match result {
                    Ok(()) => EulaState::JustAccepted,
                    Err(CoreError::SignedOut) => {
                        cx.emit(DetailEvent::SignedOut);
                        EulaState::NotAccepted
                    }
                    Err(err) => {
                        EulaState::Failed(tr!(M::EulaAcceptFailed, error = core_error(&err)).into())
                    }
                };
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Shows the EULA's state, for the offscreen screenshots.
    pub fn preview_eula(&mut self, eula: EulaState) {
        self.eula = eula;
    }

    fn follow_stream(&mut self, kind: StreamKind, cx: &mut Context<Self>) {
        let Some(client) = &self.client else {
            return;
        };
        let (tx, mut rx) = mpsc::unbounded();
        self.streams.push(LiveStream::open(
            client,
            self.key.clone(),
            &self.server.uuid,
            kind.clone(),
            tx,
        ));
        self.tasks.push(cx.spawn(async move |this, cx| {
            while let Some(message) = rx.next().await {
                let alive = this
                    .update(cx, |this, cx| this.on_message(&kind, message, cx))
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    fn status_mut(&mut self, kind: &StreamKind) -> &mut StreamStatus {
        match kind {
            StreamKind::Events | StreamKind::All | StreamKind::Monitor => &mut self.events_status,
            StreamKind::Console => &mut self.console_status,
        }
    }

    fn on_message(&mut self, kind: &StreamKind, message: LiveMessage, cx: &mut Context<Self>) {
        match message {
            LiveMessage::Connected => {
                *self.status_mut(kind) = StreamStatus::Live;
                // fill in whatever was missed before (or while) connecting
                match kind {
                    StreamKind::Console => self.fetch_buffer(cx),
                    StreamKind::Events => self.fetch_players(cx),
                    // the launcher doesn't follow every event
                    StreamKind::All | StreamKind::Monitor => {}
                }
            }
            LiveMessage::Disconnected(reason) => {
                *self.status_mut(kind) = StreamStatus::Reconnecting(reason.into());
            }
            LiveMessage::State(state) => {
                self.server.state = state;
                if !matches!(state, ServerState::Running) {
                    self.server.player_count = None;
                    if matches!(state, ServerState::Stopped | ServerState::Error) {
                        self.players = Some(Vec::new());
                    }
                }
            }
            LiveMessage::Players(players) => {
                self.server.player_count = Some(players.len() as u32);
                self.players = Some(players);
            }
            LiveMessage::Output(line) => self.push_console([line]),
            LiveMessage::Frame(_) => {}
            LiveMessage::Ended(CoreError::SignedOut) => cx.emit(DetailEvent::SignedOut),
            LiveMessage::Ended(err) => {
                *self.status_mut(kind) = StreamStatus::Off(OffReason::Failed(err))
            }
        }
        cx.notify();
    }

    fn push_console(&mut self, lines: impl IntoIterator<Item = ConsoleLine>) {
        for line in lines {
            let row = ConsoleRow {
                snowflake: line.snowflake,
                line: parse_console_line(&line.message),
            };
            match self.console.last() {
                Some(last) if last.snowflake >= row.snowflake => {
                    if let Err(at) = self
                        .console
                        .binary_search_by_key(&row.snowflake, |r| r.snowflake)
                    {
                        self.console.insert(at, row);
                    }
                }
                _ => self.console.push(row),
            }
        }
        if self.console.len() > live::MAX_CONSOLE_LINES {
            let extra = self.console.len() - live::MAX_CONSOLE_LINES;
            self.console.drain(..extra);
        }
    }

    fn fetch_buffer(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = self.key.clone();
        let uuid = self.server.uuid.clone();
        let fetch = cx.background_spawn(async move { client.console_buffer(&key, &uuid).await });
        self.tasks.retain(|t| !t.is_ready());
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(lines) => this.push_console(lines),
                    Err(CoreError::SignedOut) => cx.emit(DetailEvent::SignedOut),
                    // the live lines still arrive
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn fetch_players(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = self.key.clone();
        let uuid = self.server.uuid.clone();
        let fetch = cx.background_spawn(async move { client.players(&key, &uuid).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            // a stopped server has no player list: keep what the stream says
            if let Ok(players) = fetch.await {
                this.update(cx, |this, cx| {
                    this.server.player_count = Some(players.len() as u32);
                    this.players = Some(players);
                    cx.notify();
                })
                .ok();
            }
        }));
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.streams.clear();
        cx.emit(DetailEvent::Close);
    }

    /// Sends what is in the chat box.
    pub fn send_chat(&mut self, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let Some(command) = live::chat_command(self.chat.read(cx).text()) else {
            return;
        };
        let Some(client) = self.client.clone() else {
            return;
        };
        self.sending = true;
        self.chat_error = None;
        let key = self.key.clone();
        let uuid = self.server.uuid.clone();
        let send =
            cx.background_spawn(async move { client.send_command(&key, &uuid, &command).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = send.await;
            this.update(cx, |this, cx| {
                this.sending = false;
                match result {
                    Ok(()) => {
                        this.chat.update(cx, |input, cx| input.set_text("", cx));
                        this.follow = true;
                    }
                    Err(CoreError::SignedOut) => cx.emit(DetailEvent::SignedOut),
                    Err(err) => this.chat_error = Some(core_error(&err).into()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn load_links(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = self.key.clone();
        let fetch = cx.background_spawn(async move { client.minecraft_links(&key).await });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.links = match result {
                    Ok(links) => LinkState::Known(links),
                    Err(CoreError::TooOld) => LinkState::Unsupported,
                    Err(err) => LinkState::Failed(err),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// The selected account, if it is a Microsoft one: `(uuid, name)`.
    pub fn selected_microsoft_account(&self, cx: &gpui::App) -> Option<(String, String)> {
        let account = self.accounts.read(cx).active()?;
        (!account.is_offline()).then(|| {
            (
                account.uuid.hyphenated().to_string(),
                account.username.clone(),
            )
        })
    }

    /// Whether an account is selected but it is an offline one.
    pub fn selected_is_offline(&self, cx: &gpui::App) -> bool {
        self.accounts
            .read(cx)
            .active()
            .is_some_and(|account| account.is_offline())
    }

    /// Links the selected Microsoft account to the Lodestone user. Its Minecraft token
    /// (refreshed first if it has expired) goes to the core once, which checks it with
    /// Minecraft's services; the launcher doesn't keep anything new.
    pub fn link_selected(&mut self, cx: &mut Context<Self>) {
        if self.linking {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        self.linking = true;
        self.link_notice = None;
        let account = self
            .accounts
            .update(cx, |accounts, cx| accounts.launch_account(cx));
        let key = self.key.clone();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = match account.await {
                Ok(account) if account.user_type == UserType::Msa => {
                    let uuid = account.uuid.hyphenated().to_string();
                    let name = account.username.clone();
                    let token = account.access_token;
                    cx.background_spawn(async move {
                        client
                            .link_minecraft_account(&key, &uuid, &name, &token)
                            .await
                            .map_err(|err| core_error(&err))
                    })
                    .await
                }
                Ok(_) => Err(t(M::OfflineCantLink).to_string()),
                Err(message) => Err(message),
            };
            this.update(cx, |this, cx| {
                this.linking = false;
                match result {
                    Ok(link) => {
                        this.link_notice = Some((
                            false,
                            tr!(
                                M::LinkedTo,
                                account = link.minecraft_name,
                                user = this.user_label()
                            )
                            .into(),
                        ));
                        this.load_links(cx);
                    }
                    Err(message) => this.link_notice = Some((true, message.into())),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn unlink(&mut self, minecraft_uuid: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let key = self.key.clone();
        let remove = cx.background_spawn(async move {
            client.unlink_minecraft_account(&key, &minecraft_uuid).await
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = remove.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.load_links(cx),
                    Err(err) => this.link_notice = Some((true, core_error(&err).into())),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// "peter", or "your Lodestone user".
    pub fn user_label(&self) -> String {
        self.username
            .clone()
            .unwrap_or_else(|| t(M::YourLodestoneUser).into())
    }

    /// Whether the selected Microsoft account is linked already.
    pub fn selected_is_linked(&self, cx: &gpui::App) -> bool {
        let Some((uuid, _)) = self.selected_microsoft_account(cx) else {
            return false;
        };
        matches!(&self.links, LinkState::Known(links) if links.iter().any(|l| l.minecraft_uuid == uuid))
    }
}

impl Drop for ServerDetail {
    fn drop(&mut self) {
        self.streams.clear();
    }
}

impl super::Servers {
    pub fn accounts_entity(&self) -> &Entity<Accounts> {
        &self.accounts
    }

    /// The open server page, if any.
    pub fn detail(&self) -> Option<&Entity<ServerDetail>> {
        self.detail.as_ref().map(|(detail, _)| detail)
    }

    /// Opens `server`'s page. Only on a signed-in core: the page needs the key.
    pub fn open_detail(&mut self, core_uuid: &str, server: ServerInstance, cx: &mut Context<Self>) {
        let Some(core) = self.cores.iter().find(|c| c.saved.uuid == core_uuid) else {
            return;
        };
        let Some(key) = core.key.clone() else {
            return;
        };
        let (client, accounts) = (self.client(core), self.accounts.clone());
        let (name, username) = (core.saved.name.clone(), core.saved.username.clone());
        let core_uuid = core_uuid.to_string();
        let detail = cx.new(|cx| {
            ServerDetail::new(
                client,
                key,
                core_uuid.clone(),
                name,
                username,
                server,
                accounts,
                cx,
            )
        });
        self.show_detail(detail, cx);
    }

    /// Shows `detail` and handles what it asks for.
    pub fn show_detail(&mut self, detail: Entity<ServerDetail>, cx: &mut Context<Self>) {
        let subscription = cx.subscribe(&detail, |this, detail, event, cx| {
            let (core_uuid, server) = {
                let detail = detail.read(cx);
                (detail.core_uuid.clone(), detail.server.clone())
            };
            match event {
                DetailEvent::Close => this.close_detail(cx),
                DetailEvent::Start => this.start(&core_uuid, server, false, cx),
                DetailEvent::Stop => this.stop(&core_uuid, server, cx),
                DetailEvent::SignedOut => {
                    this.close_detail(cx);
                    if let Some(index) = this.cores.iter().position(|c| c.saved.uuid == core_uuid) {
                        this.signed_out(index, cx);
                    }
                }
            }
        });
        self.detail = Some((detail, subscription));
        cx.notify();
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.detail = None;
        self.refresh_all(cx);
        cx.notify();
    }
}
