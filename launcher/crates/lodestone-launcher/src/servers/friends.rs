//! Friends' servers: joining a server a friend shares with a Lodestone Connect invite
//! link (`launcher_core::cores::connect`), and the ones joined before
//! (`launcher_core::cores::friends`).
//!
//! An invite arrives as a `lodestone://join?invite=…` deep link (the invite page opens
//! it), from "Join a friend" on the Servers page, or pasted into the window. The join
//! dialog then:
//!
//! 1. opens the tunnel to the friend's computer (a few seconds), saying how far it is;
//! 2. shows the server: its name, version, loader, mods, players and how this computer
//!    reaches it, and which instance plays it;
//! 3. on Play, gets the right instance as for a server on a core: one made for it
//!    (with its mods kept in step), one of the user's that matches a server without
//!    mods, or a new one with the server's client pack, after the mod advisory unless
//!    the user trusts the server for mods. The pack comes through the tunnel's info port.
//! 4. starts Minecraft joining `127.0.0.1:<the tunnel's game port>`. Each server keeps
//!    its game port from session to session, so the same address works in Minecraft's
//!    server list whenever its tunnel is open here. When something else has the port,
//!    that session uses another one and the launcher says so. The instance's own
//!    server list (`servers.dat`) gets the server under its name at that address
//!    (`launcher_core::cores::server_list`), written before the game starts.
//!
//! The tunnel stays open while the game runs, and closes when it exits. If the host
//! goes away mid-game, the launcher says so. The server is remembered (the invite link
//! in the Keychain) so Play on its row joins it again; nothing is polled in the
//! background: "Check" asks once.
//!
//! A server shared through the friend list (`super::friend_list`) joins the same way,
//! from [`Source::FriendList`]: the tunnel comes from the friend list instead of a
//! link, and nothing goes to the Keychain.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use gpui::{AppContext as _, Context, Entity, FocusHandle, SharedString, Task};
pub use launcher_core::cores::friends::game_address;
use launcher_core::cores::{
    CoreClient, ServerInstance,
    connect::{self, ConnectionKind, GamePort, Invite, JoinError, Progress, SharedServer, Tunnel},
    friends::{self, FriendServer, FriendsFile, InstanceMatch},
    link::ServerLink,
};

use super::{Notice, Servers, ServersEvent};
use futures::{FutureExt as _, future::LocalBoxFuture};
use crate::{
    components::{DialogSlot, TextInput},
    i18n::{ServersMsg as M, t, tr},
};

/// What a tunnel goes to: a server shared with an invite link, or one a friend shares
/// with this person, through the friend list.
#[derive(Clone)]
pub enum Source {
    Invite(Invite),
    FriendList {
        /// The friend's key, 64 hex digits.
        friend: String,
        /// Their core's friend certificate fingerprint, 64 hex digits.
        fingerprint: String,
        instance: String,
        /// The server's name, for the dialog while it connects.
        name: Option<String>,
    },
}

/// What a friend's server said when last asked, this session.
#[derive(Clone, Debug, Default)]
pub enum FriendStatus {
    /// Not asked since the launcher started.
    #[default]
    Unknown,
    Checking,
    Online {
        server: Box<SharedServer>,
        kind: ConnectionKind,
        rtt: Duration,
    },
    Offline(JoinError),
}

/// A friend's server on the page.
pub struct FriendEntry {
    pub saved: FriendServer,
    pub status: FriendStatus,
}

/// An open tunnel to a friend's server, while its dialog is open or its game runs.
pub struct Session {
    /// `None` only in the offscreen screenshots.
    tunnel: Option<Tunnel>,
    /// What the host said when the tunnel opened.
    shared: SharedServer,
    kind: ConnectionKind,
    /// The info port's client: the server's details and its client pack.
    client: CoreClient,
    /// The shared server as a server on a core, for the instance flows.
    pub server: ServerInstance,
    /// Where Minecraft joins it: `127.0.0.1:<the game port>`.
    pub address: String,
    /// The server's saved game port, when something else had it, so this session uses
    /// another one.
    pub port_taken: Option<u16>,
    /// The instance Play started, once it did.
    pub playing: Option<String>,
    /// Play waits for a new instance to be made (and linked), then joins.
    play_when_linked: bool,
    _ended: Task<()>,
}

impl Session {
    pub fn kind(&self) -> ConnectionKind {
        self.kind
    }

    pub fn rtt(&self) -> Duration {
        self.tunnel
            .as_ref()
            .map_or(Duration::from_millis(18), Tunnel::rtt)
    }

    pub fn shared(&self) -> &SharedServer {
        &self.shared
    }
}

/// Where the join dialog is.
pub enum JoinStage {
    /// Pasting a link ("Join a friend").
    Paste {
        input: Entity<TextInput>,
        error: Option<SharedString>,
    },
    /// Opening the tunnel.
    Connecting {
        progress: Progress,
        since: Instant,
        /// The server's name, when it was joined before.
        name: Option<String>,
    },
    /// The tunnel is open: the server's card, and Play.
    Ready {
        id: String,
        /// What went wrong after Play, to try again.
        error: Option<(String, String)>,
    },
    /// Play was pressed: the instance is being made or brought in step.
    Preparing { id: String },
    Failed {
        error: JoinError,
        /// What "Try again" joins.
        source: Option<Source>,
        name: Option<String>,
    },
}

/// The join dialog.
pub struct JoinPrompt {
    pub stage: JoinStage,
    pub focus: FocusHandle,
}

/// The friends' part of the Servers page.
pub struct Friends {
    file: PathBuf,
    pub list: Vec<FriendEntry>,
    pub sessions: HashMap<String, Session>,
    pub join: DialogSlot<Servers, JoinPrompt>,
    /// The connection the dialog waits for; dropping it forgets the answer.
    connecting: Option<Task<()>>,
    /// Counts connection attempts, so a late answer to an abandoned one is dropped.
    attempt: u64,
    /// Redraws the dialog while it connects.
    ticker: Option<Task<()>>,
    checks: HashMap<String, Task<()>>,
    /// Links read from the Keychain (or saved there) this session, so Check and Play
    /// ask the Keychain once per friend, not on every press.
    invites: HashMap<String, Invite>,
}

impl Friends {
    pub fn new(data_root: &std::path::Path) -> Self {
        Self {
            file: friends::friends_file(data_root),
            list: Vec::new(),
            sessions: HashMap::new(),
            join: DialogSlot::new(|this: &mut Servers| &mut this.friends.join),
            connecting: None,
            attempt: 0,
            ticker: None,
            checks: HashMap::new(),
            invites: HashMap::new(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&FriendEntry> {
        self.list.iter().find(|f| f.saved.id == id)
    }

    /// The id of the saved server `invite` is the link of (it keeps the first link's id
    /// when a new link replaced it), or the link's own id when it isn't saved.
    fn id_of(&self, invite: &Invite) -> String {
        friends::saved_id(self.list.iter().map(|f| &f.saved), invite)
    }

    fn id_of_source(&self, source: &Source) -> String {
        match source {
            Source::Invite(invite) => self.id_of(invite),
            Source::FriendList {
                fingerprint,
                instance,
                ..
            } => friends::friend_list_id(fingerprint, instance),
        }
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut FriendEntry> {
        self.list.iter_mut().find(|f| f.saved.id == id)
    }

    /// Whether `instance_id` plays through a tunnel now.
    pub fn playing(&self, instance_id: &str) -> bool {
        self.sessions
            .values()
            .any(|s| s.playing.as_deref() == Some(instance_id))
    }
}

/// What to say when the saved port `saved` was taken, so this session is at `address`.
pub fn port_taken_text(saved: u16, address: &str) -> (String, String) {
    (
        tr!(M::FriendPortTaken, port = saved.to_string()),
        tr!(
            M::FriendPortTakenBody,
            address = address,
            port = saved.to_string()
        ),
    )
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Why joining failed, in a title and a sentence of what to do.
pub fn join_error_text(error: &JoinError) -> (String, String) {
    match error {
        JoinError::Ended => (
            t(M::FriendLinkEnded).into(),
            t(M::FriendLinkEndedBody).into(),
        ),
        JoinError::HostOffline => (
            t(M::FriendNotSharing).into(),
            t(M::FriendNotSharingBody).into(),
        ),
        JoinError::NoPath => (t(M::FriendNoPath).into(), t(M::FriendNoPathBody).into()),
        JoinError::WrongHost => (
            t(M::FriendWrongHost).into(),
            t(M::FriendWrongHostBody).into(),
        ),
        JoinError::Offline => (t(M::FriendOffline).into(), t(M::FriendOfflineBody).into()),
        JoinError::Unreachable(detail) => (
            t(M::FriendServiceUnreachable).into(),
            tr!(M::FriendServiceUnreachableBody, detail = detail),
        ),
        JoinError::Failed(detail) => (
            t(M::FriendJoinFailed).into(),
            tr!(M::FriendJoinFailedBody, detail = detail),
        ),
    }
}

/// How this computer reaches the host, in a few words.
pub fn connection_label(kind: ConnectionKind) -> &'static str {
    t(match kind {
        ConnectionKind::SameComputer => M::ViaSameComputer,
        ConnectionKind::LocalNetwork => M::ViaLocalNetwork,
        ConnectionKind::Ipv6 => M::ViaIpv6,
        ConnectionKind::Internet => M::ViaInternet,
    })
}

impl Servers {
    /// Reads `friends.json`.
    pub(super) fn load_friends(&mut self) {
        let file = match friends::load(&self.friends.file) {
            Ok(file) => file,
            Err(err) => {
                let err = err.to_string();
                self.notice = Some(Notice::new(move || {
                    (t(M::FriendsReadFailed).into(), err.clone())
                }));
                FriendsFile::default()
            }
        };
        self.friends.list = file
            .friends
            .into_iter()
            .map(|saved| FriendEntry {
                saved,
                status: FriendStatus::Unknown,
            })
            .collect();
    }

    /// Saves the friends' servers (for the friend list's trust changes).
    pub(super) fn save_friend_servers(&mut self) {
        self.save_friends();
    }

    fn save_friends(&mut self) {
        if self.preview {
            return;
        }
        let file = FriendsFile {
            friends: self.friends.list.iter().map(|f| f.saved.clone()).collect(),
        };
        if let Err(err) = friends::save(&self.friends.file, &file) {
            let err = err.to_string();
            self.notice = Some(Notice::new(move || {
                (t(M::FriendsSaveFailed).into(), err.clone())
            }));
        }
    }

    /// The join dialog: for `link` (any of the three forms), or empty to paste one.
    pub fn open_join(&mut self, link: Option<String>, cx: &mut Context<Self>) {
        match link.as_deref().map(Invite::parse) {
            Some(Ok(invite)) => self.connect_invite(invite, false, cx),
            Some(Err(_)) => {
                let text = link.unwrap_or_default();
                self.show_paste(text, Some(t(M::NotAnInvite).into()), cx);
            }
            None => self.show_paste(String::new(), None, cx),
        }
    }

    fn show_paste(&mut self, text: String, error: Option<SharedString>, cx: &mut Context<Self>) {
        self.drop_unused_sessions();
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::InvitePlaceholder)
                .with_automation_id("join-friend-link")
                .with_text(text)
        });
        let focus = cx.focus_handle();
        self.set_join_stage(JoinStage::Paste { input, error }, focus, cx);
    }

    /// Opens the dialog at `stage`, or moves the open one there.
    fn set_join_stage(&mut self, stage: JoinStage, focus: FocusHandle, cx: &mut Context<Self>) {
        match self.friends.join.current_mut() {
            Some(prompt) => prompt.stage = stage,
            None => self.friends.join.open(JoinPrompt { stage, focus }, cx),
        }
        cx.notify();
    }

    fn join_stage(&mut self, stage: JoinStage, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.set_join_stage(stage, focus, cx);
    }

    /// "Join" in the paste stage.
    pub fn submit_invite(&mut self, cx: &mut Context<Self>) {
        let Some(JoinPrompt {
            stage: JoinStage::Paste { input, .. },
            ..
        }) = self.friends.join.current()
        else {
            return;
        };
        let text = input.read(cx).text().to_string();
        match Invite::parse(&text) {
            Ok(invite) => self.connect_invite(invite, false, cx),
            Err(_) => {
                if let Some(JoinPrompt {
                    stage: JoinStage::Paste { error, .. },
                    ..
                }) = self.friends.join.current_mut()
                {
                    *error = Some(
                        t(if text.trim().is_empty() {
                            M::PasteInviteFirst
                        } else {
                            M::NotAnInvite
                        })
                        .into(),
                    );
                }
                cx.notify();
            }
        }
    }

    /// Opens a tunnel to the host of `invite` and shows its server; with `then_play`,
    /// goes on to Play once it is open.
    pub fn connect_invite(&mut self, invite: Invite, then_play: bool, cx: &mut Context<Self>) {
        self.connect_source(Source::Invite(invite), then_play, cx)
    }

    /// The tunnel for `source`: from its invite, or through the friend list.
    fn open_source(
        &mut self,
        source: &Source,
        game_port: GamePort,
        progress: Progress,
        cx: &mut Context<Self>,
    ) -> LocalBoxFuture<'static, Result<Tunnel, JoinError>> {
        match source {
            Source::Invite(invite) => {
                connect::open(invite.clone(), game_port, progress).boxed_local()
            }
            Source::FriendList {
                friend,
                fingerprint,
                instance,
                ..
            } => self.friend_list_tunnel(
                friend.clone(),
                fingerprint,
                instance.clone(),
                game_port,
                progress,
                cx,
            ),
        }
    }

    /// Opens a tunnel for `source` and shows its server; with `then_play`, goes on to
    /// Play once it is open.
    pub fn connect_source(&mut self, source: Source, then_play: bool, cx: &mut Context<Self>) {
        let id = self.friends.id_of_source(&source);
        // already open: the dialog shows it again
        if let Some(session) = self.friends.sessions.get(&id) {
            if session.playing.is_some() {
                let instance = session.playing.clone().unwrap_or_default();
                self.friends.join.close(cx);
                cx.emit(ServersEvent::OpenInstance(instance));
                return;
            }
            self.join_stage(
                JoinStage::Ready {
                    id: id.clone(),
                    error: None,
                },
                cx,
            );
            if then_play {
                self.friend_play(cx);
            }
            return;
        }
        self.drop_unused_sessions();
        self.friends.attempt += 1;
        let attempt = self.friends.attempt;
        let progress = Progress::default();
        let name = self.friends.get(&id).map(|f| f.saved.name.clone()).or(match &source {
            Source::FriendList { name, .. } => name.clone(),
            Source::Invite(_) => None,
        });
        self.join_stage(
            JoinStage::Connecting {
                progress: progress.clone(),
                since: Instant::now(),
                name: name.clone(),
            },
            cx,
        );
        self.friends.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let connecting = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        matches!(
                            this.friends.join.current().map(|p| &p.stage),
                            Some(JoinStage::Connecting { .. })
                        )
                    })
                    .unwrap_or(false);
                if !connecting {
                    break;
                }
            }
        }));
        let file = FriendsFile {
            friends: self.friends.list.iter().map(|f| f.saved.clone()).collect(),
        };
        let game_port = file.game_port(&id);
        let opening = self.open_source(&source, game_port.clone(), progress, cx);
        self.friends.connecting = Some(cx.spawn(async move |this, cx| {
            let result = opening.await;
            this.update(cx, |this, cx| {
                this.friends.connecting = None;
                let waiting = this.friends.attempt == attempt
                    && matches!(
                        this.friends.join.current().map(|p| &p.stage),
                        Some(JoinStage::Connecting { .. })
                    );
                match result {
                    // the dialog was closed or moved on: the tunnel closes as it drops
                    Ok(_) if !waiting => {}
                    Ok(tunnel) => this.connected(source, tunnel, &game_port, then_play, cx),
                    Err(error) => {
                        if let Some(entry) = this.friends.get_mut(&id) {
                            entry.status = FriendStatus::Offline(error.clone());
                        }
                        if waiting {
                            this.join_stage(
                                JoinStage::Failed {
                                    error,
                                    source: Some(source),
                                    name,
                                },
                                cx,
                            );
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// The tunnel is open: remember the server, keep the session, show the card.
    fn connected(
        &mut self,
        source: Source,
        mut tunnel: Tunnel,
        game_port: &GamePort,
        then_play: bool,
        cx: &mut Context<Self>,
    ) {
        let shared = tunnel.server.clone();
        // remember it: the file, and the link in the Keychain. A new link for a server
        // saved before takes the old link's place, under the old entry's id.
        let mut file = FriendsFile {
            friends: self.friends.list.iter().map(|f| f.saved.clone()).collect(),
        };
        let invite = match &source {
            Source::Invite(invite) => Some(invite.clone()),
            Source::FriendList { .. } => None,
        };
        let index = match &source {
            Source::Invite(invite) => file.upsert(invite, &shared, now()),
            Source::FriendList {
                friend,
                fingerprint,
                ..
            } => file.upsert_friend_list(friend, fingerprint, &shared, now()),
        };
        let port_taken = file.friends[index].joined_on_port(game_port, tunnel.game_port());
        let saved = file.friends[index].clone();
        let id = saved.id.clone();
        let status = FriendStatus::Online {
            server: Box::new(shared.clone()),
            kind: tunnel.kind,
            rtt: tunnel.rtt(),
        };
        match self.friends.get_mut(&id) {
            Some(entry) => {
                entry.saved = saved;
                entry.status = status;
            }
            None => self.friends.list.push(FriendEntry { saved, status }),
        }
        self.save_friends();
        if let Some(invite) = invite
            && !self.preview
        {
            self.friends.invites.insert(id.clone(), invite.clone());
            let secrets = self.accounts.read(cx).secrets().clone();
            let to_save = invite.clone();
            let save_as = id.clone();
            self.tasks.push(cx.spawn(async move |this, cx| {
                let store = secrets.store(cx);
                if let Err(err) = friends::write_invite(&*store, &save_as, &to_save).await {
                    let err = err.to_string();
                    this.update(cx, |this, cx| {
                        this.notice = Some(Notice::new(move || {
                            (t(M::FriendsSaveFailed).into(), err.clone())
                        }));
                        cx.notify();
                    })
                    .ok();
                }
            }));
        }
        let client = CoreClient::new(self.http.clone(), tunnel.info_url());
        let server = connect::server_instance(&shared);
        let ended = tunnel.take_ended();
        let watch_id = id.clone();
        let watch = cx.spawn(async move |this, cx| {
            let Some(ended) = ended else {
                return;
            };
            // the host ended it; a tunnel the launcher closed sends nothing
            if let Ok(why) = ended.await {
                this.update(cx, |this, cx| this.tunnel_ended(&watch_id, why, cx))
                    .ok();
            }
        });
        self.friends.sessions.insert(
            id.clone(),
            Session {
                kind: tunnel.kind,
                address: game_address(tunnel.game_port()),
                port_taken,
                tunnel: Some(tunnel),
                shared,
                client,
                server,
                playing: None,
                play_when_linked: false,
                _ended: watch,
            },
        );
        self.join_stage(JoinStage::Ready { id, error: None }, cx);
        if then_play {
            self.friend_play(cx);
        }
    }

    /// The host closed the tunnel.
    fn tunnel_ended(&mut self, id: &str, _why: String, cx: &mut Context<Self>) {
        let Some(session) = self.friends.sessions.remove(id) else {
            return;
        };
        if let Some(entry) = self.friends.get_mut(id) {
            entry.status = FriendStatus::Offline(JoinError::HostOffline);
        }
        let name = session.server.name.clone();
        let in_dialog = matches!(
            self.friends.join.current().map(|p| &p.stage),
            Some(JoinStage::Ready { id: shown, .. } | JoinStage::Preparing { id: shown })
                if shown == id
        );
        if in_dialog {
            self.join_stage(
                JoinStage::Failed {
                    error: JoinError::HostOffline,
                    source: None,
                    name: Some(name),
                },
                cx,
            );
        } else if session.playing.is_some() {
            self.fail(
                move || {
                    (
                        tr!(M::FriendTunnelEnded, name = name),
                        t(M::FriendTunnelEndedBody).into(),
                    )
                },
                cx,
            );
        }
        cx.notify();
    }

    /// Closes the tunnels that neither a dialog nor a game uses.
    fn drop_unused_sessions(&mut self) {
        let shown = match self.friends.join.current().map(|p| &p.stage) {
            Some(JoinStage::Ready { id, .. } | JoinStage::Preparing { id }) => Some(id.clone()),
            _ => None,
        };
        self.friends
            .sessions
            .retain(|id, session| session.playing.is_some() || Some(id) == shown.as_ref());
    }

    /// The dialog's close button, Escape or Cancel: any tunnel it opened closes.
    pub fn close_join(&mut self, cx: &mut Context<Self>) {
        self.friends.join.close(cx);
        self.friends.connecting = None;
        self.friends.ticker = None;
        self.friends.attempt += 1;
        self.drop_unused_sessions();
        cx.notify();
    }

    /// "Try again" after a failure.
    pub fn retry_join(&mut self, cx: &mut Context<Self>) {
        let Some(JoinPrompt {
            stage: JoinStage::Failed { source, .. },
            ..
        }) = self.friends.join.current()
        else {
            return;
        };
        match source.clone() {
            Some(source) => self.connect_source(source, false, cx),
            None => self.show_paste(String::new(), None, cx),
        }
    }

    /// Which instance plays the friend's server `id`.
    pub fn friend_instance(&self, id: &str, cx: &gpui::App) -> Option<InstanceMatch> {
        let session = self.friends.sessions.get(id)?;
        let remembered = self
            .friends
            .get(id)
            .and_then(|f| f.saved.instance_id.clone());
        let links: Vec<(String, ServerLink)> = self
            .instance_links
            .iter()
            .map(|(instance, link)| (instance.clone(), link.clone()))
            .collect();
        Some(friends::match_instance(
            id,
            session.shared(),
            remembered.as_deref(),
            self.instances.read(cx).list(),
            &links,
        ))
    }

    /// Play in the dialog: gets the instance ready, then joins through the tunnel.
    pub fn friend_play(&mut self, cx: &mut Context<Self>) {
        let Some(JoinPrompt {
            stage: JoinStage::Ready { id, .. },
            ..
        }) = self.friends.join.current()
        else {
            return;
        };
        let id = id.clone();
        let Some(found) = self.friend_instance(&id, cx) else {
            return;
        };
        let Some(server) = self.friends.sessions.get(&id).map(|s| s.server.clone()) else {
            return;
        };
        if let Some(instance) = found.instance_id()
            && self.instances.read(cx).is_active(instance)
        {
            let name = server.name.clone();
            return self.friend_failed(
                &id,
                move || {
                    (
                        tr!(M::JoinFailed, name = name),
                        t(M::FriendInstanceBusy).into(),
                    )
                },
                cx,
            );
        }
        self.join_stage(JoinStage::Preparing { id: id.clone() }, cx);
        match found {
            InstanceMatch::Linked(_) => self.play(&id, server, cx),
            InstanceMatch::Matching(instance) => self.join(&id, &server, &instance, cx),
            InstanceMatch::Create => {
                if let Some(session) = self.friends.sessions.get_mut(&id) {
                    session.play_when_linked = true;
                }
                self.create_instance(&id, server, cx);
            }
        }
    }

    /// Something failed after Play: the dialog shows it, and Play can be tried again.
    fn friend_failed(
        &mut self,
        id: &str,
        text: impl Fn() -> (String, String) + 'static,
        cx: &mut Context<Self>,
    ) {
        let error = Some(text());
        self.join_stage(
            JoinStage::Ready {
                id: id.to_string(),
                error,
            },
            cx,
        );
    }

    /// A failure on the Servers page while the dialog prepares a friend's server goes
    /// to the dialog too. Returns whether it did.
    pub(super) fn friend_failure(
        &mut self,
        title: &str,
        message: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(JoinPrompt {
            stage: JoinStage::Preparing { id },
            ..
        }) = self.friends.join.current()
        else {
            return false;
        };
        let id = id.clone();
        if let Some(session) = self.friends.sessions.get_mut(&id) {
            session.play_when_linked = false;
        }
        let (title, message) = (title.to_string(), message.to_string());
        self.friend_failed(&id, move || (title.clone(), message.clone()), cx);
        true
    }

    /// A question about a friend's server was answered with Cancel: back to its card.
    pub(super) fn friend_cancelled(&mut self, core: &str, cx: &mut Context<Self>) {
        if !friends::is_friend_id(core) {
            return;
        }
        if let Some(session) = self.friends.sessions.get_mut(core) {
            session.play_when_linked = false;
        }
        if matches!(
            self.friends.join.current().map(|p| &p.stage),
            Some(JoinStage::Preparing { id }) if id == core
        ) {
            self.join_stage(
                JoinStage::Ready {
                    id: core.to_string(),
                    error: None,
                },
                cx,
            );
        }
    }

    /// The info port's client for a friend's server, while its tunnel is open.
    pub(super) fn friend_client(&self, id: &str) -> Option<CoreClient> {
        Some(self.friends.sessions.get(id)?.client.clone())
    }

    /// Whether a pull from the friend's server `id` needs the advisory, and the name to
    /// show in it.
    /// For a server from the friend list, trust holds only for the client pack it was
    /// given with (`etag`): a changed pack asks again.
    pub(super) fn friend_pull_source(
        &self,
        id: &str,
        kind: launcher_core::cores::trust::PullKind,
        etag: Option<&str>,
    ) -> Option<(bool, String)> {
        let entry = self.friends.get(id)?;
        Some((
            kind.is_executable() && !entry.saved.trusts_pack(etag),
            entry.saved.core_name.clone(),
        ))
    }

    /// "Trust and install" for a friend's server, with the client pack `etag` it is
    /// about to install.
    pub(super) fn trust_friend(&mut self, id: &str, etag: Option<&str>) {
        if let Some(entry) = self.friends.get_mut(id) {
            entry.saved.trust_pack(etag);
            self.save_friends();
        }
    }

    /// Stops trusting a friend's server for mods (its row's menu).
    pub fn untrust_friend(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(entry) = self.friends.get_mut(id) {
            entry.saved.set_trusted(false);
            self.save_friends();
        }
        cx.notify();
    }

    /// A new instance was linked to the friend's server `id`: join it when Play waits.
    pub(super) fn friend_linked(&mut self, id: &str, instance_id: &str, cx: &mut Context<Self>) {
        let Some(session) = self.friends.sessions.get_mut(id) else {
            return;
        };
        if !std::mem::take(&mut session.play_when_linked) {
            return;
        }
        let server = session.server.clone();
        // the success notice of a new instance says "press Play"; Play goes on here
        self.notice = None;
        self.join(id, &server, instance_id, cx);
    }

    /// Where Minecraft joins the friend's server `id`, and the game is tied to its
    /// tunnel. `None` when its tunnel closed. `game_dir` is the instance's game folder,
    /// to list the server in; `None` while its game runs (the game rewrites the list).
    pub(super) fn friend_join_target(
        &mut self,
        id: &str,
        instance_id: &str,
        game_dir: Option<&Path>,
    ) -> Option<launcher_core::game::launch::JoinServer> {
        let session = self.friends.sessions.get_mut(id)?;
        let target = session.tunnel.as_ref()?.join_target();
        session.playing = Some(instance_id.to_string());
        // the dialog said so; it closes as the game starts, so the page says it too
        if let Some(saved) = session.port_taken {
            let address = session.address.clone();
            self.notice = Some(Notice::info(move || port_taken_text(saved, &address)));
        }
        let linked = self
            .instance_links
            .get(instance_id)
            .is_some_and(|link| link.core_uuid == id);
        if let Some(entry) = self.friends.get_mut(id) {
            entry.saved.instance_id = (!linked).then(|| instance_id.to_string());
            entry.saved.last_joined = Some(now());
            // the game still joins when this fails; the list is only a convenience
            if let Some(game_dir) = game_dir
                && let Err(err) = entry.saved.list_in_game(instance_id, game_dir, false)
            {
                log::warn!("couldn't update {instance_id}'s server list: {err}");
            }
        }
        self.save_friends();
        Some(target)
    }

    /// The game started: the dialog has done its part.
    pub(super) fn friend_started(&mut self, cx: &mut Context<Self>) {
        self.friends.join.close(cx);
    }

    /// Closes the tunnels of games that have exited (or never started).
    pub(super) fn friend_games_changed(&mut self, cx: &mut Context<Self>) {
        let instances = self.instances.read(cx);
        let before = self.friends.sessions.len();
        self.friends
            .sessions
            .retain(|_, session| match &session.playing {
                Some(instance) => instances.is_active(instance),
                None => true,
            });
        if self.friends.sessions.len() != before {
            cx.notify();
        }
    }

    /// Play on a friend's server's row (or a linked instance's Play): reads its link
    /// from the Keychain and joins.
    pub fn play_friend(&mut self, id: &str, cx: &mut Context<Self>) {
        self.open_friend(id, true, cx)
    }

    /// Opens the join dialog for a friend's server joined before; with `then_play`,
    /// goes on to Play once the tunnel is open.
    pub fn open_friend(&mut self, id: &str, then_play: bool, cx: &mut Context<Self>) {
        if self
            .friends
            .sessions
            .get(id)
            .is_some_and(|s| s.playing.is_some())
        {
            if let Some(instance) = self.friends.sessions[id].playing.clone() {
                cx.emit(ServersEvent::OpenInstance(instance));
            }
            return;
        }
        let name = self.friends.get(id).map(|f| f.saved.name.clone());
        // the dialog says it is connecting at once (the Keychain answers first), and an
        // earlier attempt's answer no longer counts
        self.friends.connecting = None;
        self.friends.attempt += 1;
        self.join_stage(
            JoinStage::Connecting {
                progress: Progress::default(),
                since: Instant::now(),
                name: name.clone(),
            },
            cx,
        );
        if let Some(source) = self.friend_list_source(id) {
            self.connect_source(source, then_play, cx);
            return;
        }
        if let Some(invite) = self.friends.invites.get(id).cloned() {
            self.connect_invite(invite, then_play, cx);
            return;
        }
        let secrets = self.accounts.read(cx).secrets().clone();
        let id = id.to_string();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let store = secrets.store(cx);
            let invite = friends::read_invite(&*store, &id).await;
            this.update(cx, |this, cx| match invite {
                Ok(Some(invite)) => {
                    this.friends.invites.insert(id.clone(), invite.clone());
                    this.connect_invite(invite, then_play, cx)
                }
                _ => this.join_stage(
                    JoinStage::Failed {
                        error: JoinError::Failed(t(M::FriendLinkMissing).into()),
                        source: None,
                        name,
                    },
                    cx,
                ),
            })
            .ok();
        }));
    }

    /// "Check": opens the tunnel once to see whether the server is shared now and who
    /// plays, then closes it.
    pub fn check_friend(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.friends.checks.contains_key(id) {
            return;
        }
        if let Some(session) = self.friends.sessions.get(id) {
            let (server, kind, rtt) = (
                Box::new(session.shared().clone()),
                session.kind(),
                session.rtt(),
            );
            if let Some(entry) = self.friends.get_mut(id) {
                entry.status = FriendStatus::Online { server, kind, rtt };
            }
            cx.notify();
            return;
        }
        let Some(entry) = self.friends.get_mut(id) else {
            return;
        };
        entry.status = FriendStatus::Checking;
        let secrets = self.accounts.read(cx).secrets().clone();
        let check_id = id.to_string();
        let cached = self.friends.invites.get(id).cloned();
        // a server from the friend list opens its tunnel through the list
        // any free port: the saved one stays free for Play
        let listed = self
            .friend_list_source(id)
            .map(|source| self.open_source(&source, GamePort::Any, Progress::default(), cx));
        let task = cx.spawn(async move |this, cx| {
            let invite = match (cached, &listed) {
                (Some(invite), _) => Ok(Some(invite)),
                (None, Some(_)) => Ok(None),
                (None, None) => {
                    let store = secrets.store(cx);
                    friends::read_invite(&*store, &check_id).await
                }
            };
            let read = invite.as_ref().ok().cloned().flatten();
            let opening = match (invite, listed) {
                (_, Some(opening)) => Some(opening),
                // any free port: the saved one stays free for Play
                (Ok(Some(invite)), None) => Some(
                    connect::open(invite, GamePort::Any, Progress::default()).boxed_local(),
                ),
                _ => None,
            };
            let result = match opening {
                Some(opening) => {
                    opening
                        .await
                        .map(|tunnel| {
                            let status = FriendStatus::Online {
                                server: Box::new(tunnel.server.clone()),
                                kind: tunnel.kind,
                                rtt: tunnel.rtt(),
                            };
                            drop(tunnel);
                            status
                        })
                }
                _ => Err(JoinError::Failed(t(M::FriendLinkMissing).into())),
            };
            this.update(cx, |this, cx| {
                this.friends.checks.remove(&check_id);
                if let Some(invite) = read {
                    this.friends.invites.insert(check_id.clone(), invite);
                }
                if let Some(entry) = this.friends.get_mut(&check_id) {
                    entry.status = match result {
                        Ok(status) => status,
                        Err(error) => FriendStatus::Offline(error),
                    };
                }
                cx.notify();
            })
            .ok();
        });
        self.friends.checks.insert(id.to_string(), task);
        cx.notify();
    }

    /// Forgets a friend's server: its row, and its link in the Keychain. Instances made
    /// for it stay.
    pub fn remove_friend(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = self.friends.list.iter().position(|f| f.saved.id == id) else {
            return;
        };
        let removed = self.friends.list.remove(index);
        // out of the server lists it was added to; a running game's list is left as it
        // is (the game rewrites it)
        if !self.preview {
            let instances = self.instances.read(cx);
            let failed = removed.saved.unlist_in_games(
                |instance| instances.game_dir(instance),
                |instance| instances.is_active(instance),
            );
            for (instance, err) in failed {
                log::warn!("couldn't update {instance}'s server list: {err}");
            }
        }
        self.friends.checks.remove(id);
        self.friends.invites.remove(id);
        self.save_friends();
        if self.preview || friends::is_friend_list_id(id) {
            cx.notify();
            return;
        }
        let secrets = self.accounts.read(cx).secrets().clone();
        let id = id.to_string();
        self.tasks.push(cx.spawn(async move |_, cx| {
            let store = secrets.store(cx);
            let _ = friends::delete_invite(&*store, &id).await;
        }));
        cx.notify();
    }
}

impl Servers {
    /// The friend-list source of a saved friend's server, when it came from the list.
    fn friend_list_source(&self, id: &str) -> Option<Source> {
        let entry = self.friends.get(id)?;
        if !entry.saved.is_friend_list() {
            return None;
        }
        let (fingerprint, instance) = friends::parse_friend_list_id(id)?;
        Some(Source::FriendList {
            friend: entry.saved.friend_key.clone(),
            fingerprint: fingerprint.to_string(),
            instance: instance.to_string(),
            name: Some(entry.saved.name.clone()),
        })
    }
}

/// What the join dialog shows in an offscreen screenshot.
pub enum PreviewJoin {
    Paste {
        text: String,
        error: bool,
    },
    Connecting {
        name: Option<String>,
        reading: bool,
    },
    Ready {
        server: SharedServer,
        kind: ConnectionKind,
        /// The saved game port, and the one this session uses when it was taken.
        port: u16,
        taken_port: Option<u16>,
    },
    Failed {
        error: JoinError,
        name: Option<String>,
    },
}

impl Servers {
    /// Shows made-up friends' servers, for the offscreen screenshots.
    pub fn preview_friends(&mut self, list: Vec<(FriendServer, FriendStatus)>) {
        self.friends.list = list
            .into_iter()
            .map(|(saved, status)| FriendEntry { saved, status })
            .collect();
    }

    /// Opens the join dialog at a made-up stage, for the offscreen screenshots. Nothing
    /// is fetched or opened.
    pub fn preview_join(&mut self, stage: PreviewJoin, cx: &mut Context<Self>) {
        let stage = match stage {
            PreviewJoin::Paste { text, error } => {
                return self.show_paste(text, error.then(|| t(M::NotAnInvite).into()), cx);
            }
            PreviewJoin::Connecting { name, reading } => {
                let progress = Progress::default();
                if reading {
                    progress.preview_reading();
                }
                JoinStage::Connecting {
                    progress,
                    since: Instant::now(),
                    name,
                }
            }
            PreviewJoin::Ready {
                server,
                kind,
                port,
                taken_port,
            } => {
                let id = format!("{}preview/{}", friends::ID_PREFIX, server.instance_uuid);
                let client = CoreClient::new(
                    self.http.clone(),
                    url::Url::parse("http://127.0.0.1:9").expect("a URL"),
                );
                self.friends.sessions.insert(
                    id.clone(),
                    Session {
                        tunnel: None,
                        kind,
                        address: game_address(taken_port.unwrap_or(port)),
                        port_taken: taken_port.map(|_| port),
                        client,
                        server: connect::server_instance(&server),
                        shared: server,
                        playing: None,
                        play_when_linked: false,
                        _ended: Task::ready(()),
                    },
                );
                JoinStage::Ready { id, error: None }
            }
            PreviewJoin::Failed { error, name } => JoinStage::Failed {
                error,
                source: None,
                name,
            },
        };
        self.join_stage(stage, cx);
    }
}
