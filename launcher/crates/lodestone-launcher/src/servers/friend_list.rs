//! The friend list on the Servers page (launcher/docs/friends.md): people added once
//! with a friend link, whose servers the launcher lists and joins without links.
//!
//! The list itself is `launcher_core::friend_list`: [`Friends`] keeps the book (sealed
//! in `<data>/friend_list.json`), the person key (Keychain) and the service, and does
//! the network calls on a Tokio thread of its own. This module keeps one `Friends` per
//! window, opened the first time it is needed without making anything
//! ([`Friends::open_existing`]: the section opens it only when the friend file exists,
//! so visiting the page, opening a friend link or a read-only call makes no key). It
//! is started ([`Friends::create`]) only when the person does something that needs
//! it: choosing a name, making or answering a friend link.
//!
//! Every operation ([`Servers::friend_op`]) takes a copy of the list and runs on the
//! background executor, never on the window's thread: file work (the lock, sealing,
//! writing) and network calls happen there, and the list's lock is held only to copy
//! it, so a refresh or a server listing never holds up Play, Accept or Remove. Copies
//! can't undo each other's changes: each change reloads the file under its lock first,
//! as with the Host. Each operation leaves a copy of the book ([`Snapshot`]) for
//! drawing.
//!
//! - **Refreshing** (`refresh` then `presence`, and the answers to open friend links)
//!   happens when the section is shown and every [`REFRESH_EVERY`] while it stays
//!   shown; never in the background otherwise.
//! - **Adding** never happens without the person's click: a pasted or opened link shows
//!   the card's name and safety code first ([`AddFriend`]), and answers to this person's
//!   own links wait for Accept.
//! - **Names and skins.** Signed in with Microsoft, the person's name is their
//!   Minecraft name until they choose another ([`Friends::set_players`], before every
//!   change that needs a name), and their card and answers say which Minecraft profile
//!   they play as. Friends' faces come from Mojang's session server by that profile's
//!   UUID ([`Servers::request_friend_heads`]), cached on disk, in the background: the
//!   list never waits for them, and without one it looks as before.
//! - **Playing** on a friend's server opens a tunnel through the friend list and goes
//!   on as the invite flow does (`super::friends`, `Source::FriendList`). Such a server
//!   is a `FriendServer` with the id `friend:<core fingerprint>/<instance>`; its mods
//!   start untrusted, and trust ends when the friend retires the core or is removed.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{
    FutureExt as _,
    future::{BoxFuture, LocalBoxFuture, Shared},
    lock::Mutex,
};
use gpui::{
    App, AppContext as _, AsyncApp, Context, Entity, FocusHandle, RenderImage, SharedString, Task,
};
use launcher_core::{
    SecretStore,
    accounts::AccountKind,
    cores::{
        connect::{GamePort, JoinError, Progress, SharedServer, Tunnel},
        friends::{FriendLink, FriendsFile},
    },
    friend_list::{
        self, Book, Friend, FriendStatus as Status, Friends, FriendsError, Opened, StatePlayer,
        StoredKeys,
        ops::{self, CorePresence, LinkPreview, LinkReply, Refreshed},
        profile_of,
    },
    paths::DataDir,
    skin,
};

pub use launcher_core::friend_list::{hex, service, unhex32};

use super::{Notice, Servers, friends::Source};
use crate::{
    components::{DialogSlot, TextInput},
    i18n::{FriendListMsg as M, t, tr},
};

/// What an operation says when there's no friend list (yet, or any more).
const NO_LIST: &str = "You have no friend list yet. Add a friend to start one.";

/// How often the section checks on friends while it is shown.
pub const REFRESH_EVERY: Duration = Duration::from_secs(5 * 60);

/// Showing the section again within this long doesn't check again.
const REFRESH_AGAIN_AFTER: Duration = Duration::from_secs(20);

/// A short, stable piece of a key or fingerprint for automation ids.
pub fn short(key: &str) -> &str {
    key.get(..12).unwrap_or(key)
}

/// The open friend list; `None` when there's none (never started, or retired).
type Handle = Arc<Mutex<Option<Friends>>>;
type Opening = Shared<LocalBoxFuture<'static, Result<Handle, String>>>;

/// An operation on a copy of the friend list, run on the background executor.
pub type FriendOp<T> =
    Box<dyn for<'a> FnOnce(&'a mut Friends) -> BoxFuture<'a, Result<T, FriendsError>> + Send>;

/// What the window shows of the friend list: a copy taken after each change.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub book: Book,
    /// This person's safety code and key; empty until the list is open.
    pub safety_code: String,
    pub key: String,
    /// Friends whose name looks like another friend's: their key → the other names.
    pub name_clashes: BTreeMap<String, Vec<String>>,
}

impl Snapshot {
    pub fn of(friends: &Friends) -> Self {
        Self {
            name_clashes: friends.book.name_clashes(),
            book: friends.book.clone(),
            safety_code: friends.safety_code(),
            key: friends.me().to_hex(),
        }
    }
}

/// Opens the friend list in `data` if there is one, making nothing: the Keychain is
/// read on the window's thread (its futures aren't `Send`), the file off it. A retired
/// list counts as none.
async fn open_list(
    data: DataDir,
    store: Rc<dyn SecretStore>,
    cx: &mut AsyncApp,
) -> Result<Option<Friends>, FriendsError> {
    if !Friends::exists(&data) {
        return Ok(None);
    }
    let keys = friend_list::read_keys(&data, &*store).await?;
    let opened = cx
        .background_spawn(async move { Friends::open_with(&data, &keys, service()) })
        .await?;
    Ok(match opened {
        Opened::Ready(friends) => Some(*friends),
        Opened::None | Opened::Retired => None,
    })
}

/// Starts the friend list in `data` (or opens the one there): the person did something
/// that needs it. A retired list is deleted first, with its Keychain items, and a new
/// identity made. The Keychain is used on the window's thread, the files off it.
async fn start_list(
    data: DataDir,
    store: Rc<dyn SecretStore>,
    cx: &mut AsyncApp,
) -> Result<Friends, FriendsError> {
    let keys = friend_list::read_keys(&data, &*store).await?;
    let opening = data.clone();
    let (opened, keys) = cx
        .background_spawn(async move {
            let opened = Friends::open_with(&opening, &keys, service());
            (opened, keys)
        })
        .await;
    let keys = match opened? {
        Opened::Ready(friends) => return Ok(*friends),
        Opened::None => keys,
        Opened::Retired => {
            let removing = data.clone();
            cx.background_spawn(async move { friend_list::remove_file(&removing) })
                .await?;
            friend_list::forget_keys(&data, &*store).await?;
            StoredKeys::default()
        }
    };
    let keys = friend_list::make_keys(&data, &*store, keys).await?;
    cx.background_spawn(async move { Friends::start_with(&data, &keys, service()) })
        .await
}

/// A Minecraft profile's face, for the friend list: by the profile's UUID.
#[derive(Clone)]
pub enum Head {
    Loading,
    Ready(Arc<RenderImage>),
    /// No skin of its own, or it couldn't be fetched: the list draws what it drew
    /// before. Asked again when the launcher next starts.
    None,
}

/// The servers of one of a friend's cores, once asked.
#[derive(Clone, Debug)]
pub enum CoreServers {
    Loading,
    Loaded(Vec<SharedServer>),
    Failed(String),
}

/// A friend link this person made, while the window shows it.
#[derive(Clone, Debug)]
pub struct MadeLink {
    pub link: String,
    pub expires: u64,
}

/// The Add friend dialog.
pub struct AddFriend {
    /// While this person has no name yet.
    pub name_input: Option<Entity<TextInput>>,
    pub saving_name: bool,
    pub making: bool,
    pub copied: bool,
    /// The friend link to open.
    pub input: Entity<TextInput>,
    pub looking: bool,
    /// The link looked up and what its card says.
    pub preview: Option<(String, LinkPreview)>,
    pub adding: bool,
    pub error: Option<SharedString>,
    /// The pasted link's service, when it isn't this launcher's: looked up only on a
    /// click.
    pub other_service: Option<String>,
    /// The name of the friend just added.
    pub added: Option<String>,
}

/// The friend list's dialogs.
pub enum FriendDialog {
    Name {
        input: Entity<TextInput>,
        error: Option<SharedString>,
        saving: bool,
    },
    Add(Box<AddFriend>),
    Verify {
        key: String,
    },
    Rename {
        key: String,
        input: Entity<TextInput>,
    },
    Remove {
        key: String,
        removing: bool,
        error: Option<SharedString>,
    },
    /// "Retire my friend identity": every friend is told, then the key and the list go.
    Retire {
        understood: bool,
        retiring: bool,
        error: Option<SharedString>,
    },
    /// "Retire" on one of this person's cores: friends drop it, and it's never listed
    /// again.
    RetireCore {
        fingerprint: String,
        name: String,
        retiring: bool,
        error: Option<SharedString>,
    },
}

pub struct FriendPrompt {
    pub dialog: FriendDialog,
    pub focus: FocusHandle,
}

/// The friend list's part of the Servers page.
pub struct FriendList {
    file: PathBuf,
    opening: Option<Opening>,
    /// The list was opened this session.
    pub opened: bool,
    pub open_error: Option<String>,
    pub snapshot: Snapshot,
    /// Answers to this person's friend links, as last asked.
    pub replies: Vec<LinkReply>,
    /// Answers this person chose to leave: (code, key).
    pub ignored: HashSet<(String, String)>,
    /// Whether each friend's core is sharing with this person: (friend, fingerprint).
    pub presence: HashMap<(String, String), bool>,
    pub refreshing: bool,
    pub refreshed_at: Option<Instant>,
    /// What couldn't be checked last time.
    pub problem: Option<String>,
    /// Friends whose servers are shown.
    pub expanded: HashSet<String>,
    pub servers: HashMap<(String, String), CoreServers>,
    pub made: Option<MadeLink>,
    pub dialog: DialogSlot<Servers, FriendPrompt>,
    /// Friends or answers being changed: their keys.
    pub busy: HashSet<String>,
    /// Faces of the Minecraft profiles friends (and people answering) say they play
    /// as, by UUID.
    pub heads: HashMap<[u8; 16], Head>,
    poller: Option<Task<()>>,
    refresh_task: Option<Task<()>>,
}

impl FriendList {
    pub fn new(data_root: &std::path::Path) -> Self {
        Self {
            file: data_root.join(friend_list::FILE_NAME),
            opening: None,
            opened: false,
            open_error: None,
            snapshot: Snapshot::default(),
            replies: Vec::new(),
            ignored: HashSet::new(),
            presence: HashMap::new(),
            refreshing: false,
            refreshed_at: None,
            problem: None,
            expanded: HashSet::new(),
            servers: HashMap::new(),
            made: None,
            dialog: DialogSlot::new(|this: &mut Servers| &mut this.friend_list.dialog),
            busy: HashSet::new(),
            heads: HashMap::new(),
            poller: None,
            refresh_task: None,
        }
    }

    pub fn friend(&self, key: &str) -> Option<&Friend> {
        self.snapshot.book.friend(key)
    }

    /// Starts over after the identity was retired (or the file went): the next use
    /// opens the list again. Open dialogs stay, to close themselves.
    fn forget(&mut self) {
        self.opening = None;
        self.opened = false;
        self.open_error = None;
        self.snapshot = Snapshot::default();
        self.replies.clear();
        self.ignored.clear();
        self.presence.clear();
        self.refreshed_at = None;
        self.problem = None;
        self.expanded.clear();
        self.servers.clear();
        self.made = None;
        self.busy.clear();
        self.poller = None;
        self.refresh_task = None;
        self.refreshing = false;
    }

    /// The friends, as the list shows them: by name.
    pub fn sorted(&self) -> Vec<&Friend> {
        let mut friends: Vec<&Friend> = self.snapshot.book.friends.iter().collect();
        friends.sort_by_key(|f| (f.display_name().to_lowercase(), f.key.clone()));
        friends
    }

    /// The answers still waiting: not ignored, not from a friend already.
    pub fn waiting_replies(&self) -> Vec<&LinkReply> {
        self.replies
            .iter()
            .filter(|r| !self.ignored.contains(&(r.code.clone(), r.key.clone())))
            .filter(|r| self.snapshot.book.friend(&r.key).is_none())
            .collect()
    }

    /// `Some(true)` when one of the friend's cores is sharing with this person now,
    /// `Some(false)` when none is, `None` before the first check.
    pub fn online(&self, friend: &Friend) -> Option<bool> {
        friend_online(friend, &self.presence)
    }

    /// The face of the Minecraft profile `player`, once fetched.
    pub fn head(&self, player: Option<&StatePlayer>) -> Option<Arc<RenderImage>> {
        let player = player.filter(|p| !p.offline)?;
        match self.heads.get(&player.uuid) {
            Some(Head::Ready(image)) => Some(image.clone()),
            _ => None,
        }
    }
}

/// Whether any of `friend`'s joinable cores is online in `presence`.
pub fn friend_online(friend: &Friend, presence: &HashMap<(String, String), bool>) -> Option<bool> {
    let cores = friend.joinable_cores(friend_list::now());
    let mut known = false;
    for core in cores {
        match presence.get(&(friend.key.clone(), hex(&core.fingerprint))) {
            Some(true) => return Some(true),
            Some(false) => known = true,
            None => {}
        }
    }
    known.then_some(false)
}

/// Why a friend reference didn't match.
#[derive(Debug, PartialEq, Eq)]
pub enum FindError {
    NotFound,
    Ambiguous,
}

/// A friend by key, a key's first 8 or more hex digits, or name or nickname (any
/// case). Returns their key.
pub fn find_friend(book: &Book, reference: &str) -> Result<String, FindError> {
    let reference = reference.trim();
    if let Some(friend) = book.friend(reference) {
        return Ok(friend.key.clone());
    }
    let lower = reference.to_lowercase();
    let by_prefix: Vec<&Friend> =
        if lower.len() >= 8 && lower.chars().all(|c| c.is_ascii_hexdigit()) {
            book.friends
                .iter()
                .filter(|f| f.key.starts_with(&lower))
                .collect()
        } else {
            Vec::new()
        };
    let found: Vec<&Friend> = if by_prefix.is_empty() {
        book.friends
            .iter()
            .filter(|f| f.display_name().to_lowercase() == lower || f.name.to_lowercase() == lower)
            .collect()
    } else {
        by_prefix
    };
    match found.as_slice() {
        [friend] => Ok(friend.key.clone()),
        [] => Err(FindError::NotFound),
        _ => Err(FindError::Ambiguous),
    }
}

/// When a link stops working, for people: "Today, 14:05" or "2026-11-10 14:05".
pub fn expiry_label(expires: u64) -> String {
    use chrono::TimeZone as _;
    let Some(time) = chrono::Local.timestamp_opt(expires as i64, 0).single() else {
        return String::new();
    };
    let today = chrono::Local::now().date_naive();
    if time.date_naive() == today {
        crate::i18n::format::day_and_time(time, today)
    } else {
        time.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// What a refresh found, for the window and the control API.
#[derive(Clone, Debug, Default)]
pub struct RefreshReport {
    pub refreshed: Refreshed,
    pub presence: Vec<CorePresence>,
    pub replies: Vec<LinkReply>,
    pub problems: Vec<String>,
}

impl Servers {
    /// The friend list, opening it the first time, without making anything. `None` in
    /// the offscreen screenshots.
    fn friends_opening(&mut self, cx: &mut Context<Self>) -> Option<Opening> {
        if self.preview {
            return None;
        }
        if let Some(opening) = &self.friend_list.opening {
            return Some(opening.clone());
        }
        let accounts = self.accounts.read(cx);
        let secrets = accounts.secrets().clone();
        let data = accounts.data_dir().clone();
        let mut async_cx = cx.to_async();
        let opening = async move {
            let store = secrets.store(&async_cx);
            open_list(data, store, &mut async_cx)
                .await
                .map(|friends| Arc::new(Mutex::new(friends)))
                .map_err(|err| err.to_string())
        }
        .boxed_local()
        .shared();
        self.friend_list.opening = Some(opening.clone());
        Some(opening)
    }

    /// The open list, or why it couldn't be opened (the window shows it, and the next
    /// attempt opens it again).
    async fn opened_handle(
        opening: Opening,
        this: &gpui::WeakEntity<Self>,
        cx: &mut AsyncApp,
    ) -> Result<Handle, String> {
        opening.await.inspect_err(|err| {
            this.update(cx, |this, cx| {
                this.friend_list.opening = None;
                this.friend_list.open_error = Some(err.clone());
                cx.notify();
            })
            .ok();
        })
    }

    /// Runs `op` on a copy of the friend list, on the background executor, then keeps a
    /// copy of the book for the window. With `create`, a list that isn't there yet is
    /// started first (the person's first name, link or answer); otherwise there must be
    /// one. Operations run side by side: each change reloads the file under its lock.
    pub fn friend_op<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        create: bool,
        op: FriendOp<T>,
    ) -> Task<Result<T, String>> {
        let Some(opening) = self.friends_opening(cx) else {
            return Task::ready(Err("The friend list isn't available here.".into()));
        };
        let accounts = self.accounts.read(cx);
        let secrets = accounts.secrets().clone();
        let data = accounts.data_dir().clone();
        cx.spawn(async move |this, cx| {
            let handle = Self::opened_handle(opening, &this, cx).await?;
            let copy = {
                let mut guard = handle.lock().await;
                if guard.is_none() && create {
                    let store = secrets.store(cx);
                    *guard = Some(
                        start_list(data, store, cx)
                            .await
                            .map_err(|err| err.to_string())?,
                    );
                }
                guard.clone()
            };
            let Some(mut friends) = copy else {
                return Err(NO_LIST.to_string());
            };
            let (result, friends) = cx
                .background_spawn(async move {
                    let result = op(&mut friends).await;
                    (result, friends)
                })
                .await;
            let stale = matches!(&result, Err(err) if err.is_stale());
            let snapshot = {
                let mut guard = handle.lock().await;
                match guard.as_mut() {
                    Some(current) if !stale => {
                        // keep the newer of the two books
                        if friends.book.generation >= current.book.generation {
                            current.book = friends.book;
                        }
                        Some(Snapshot::of(current))
                    }
                    _ => None,
                }
            };
            let result = result.map_err(|err| err.to_string());
            this.update(cx, |this, cx| {
                let Some(snapshot) = snapshot else {
                    // the file went, was retired, or needs other keys: open it again
                    // next time, never from this copy
                    this.friend_list.forget();
                    cx.notify();
                    return;
                };
                let first = !this.friend_list.opened;
                this.friend_list.opened = true;
                this.friend_list.open_error = None;
                this.friend_list.snapshot = snapshot;
                if first && this.visible {
                    this.friend_list_visible(true, cx);
                }
                this.request_friend_heads(cx);
                cx.notify();
            })
            .ok();
            result
        })
    }

    /// The friend list as it is, for looking only: opens it if needed (making nothing),
    /// starts no refresh. `None` when there's none.
    pub fn friend_peek(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<Option<Snapshot>, String>> {
        let Some(opening) = self.friends_opening(cx) else {
            return Task::ready(Err("The friend list isn't available here.".into()));
        };
        cx.spawn(async move |this, cx| {
            let handle = Self::opened_handle(opening, &this, cx).await?;
            let snapshot = handle.lock().await.as_ref().map(Snapshot::of);
            if let Some(snapshot) = &snapshot {
                let snapshot = snapshot.clone();
                this.update(cx, |this, cx| {
                    this.friend_list.open_error = None;
                    this.friend_list.snapshot = snapshot;
                    this.request_friend_heads(cx);
                    cx.notify();
                })
                .ok();
            }
            Ok(snapshot)
        })
    }

    /// "Try again" after the list failed to open: opens it again, making nothing.
    pub fn reopen_friend_list(&mut self, cx: &mut Context<Self>) {
        self.friend_list.opening = None;
        self.friend_peek(cx).detach();
    }

    /// The section is shown or hidden: check on friends now and every few minutes
    /// while it is shown, once the friend list is in use.
    pub(super) fn friend_list_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if !visible || self.preview {
            self.friend_list.poller = None;
            return;
        }
        if !self.friend_list.opened && !self.friend_list.file.exists() {
            // never used on this computer: nothing to check, and no key to make
            return;
        }
        self.friend_list.poller = Some(cx.spawn(async move |this, cx| {
            loop {
                let ok = this
                    .update(cx, |this, cx| {
                        let due = this
                            .friend_list
                            .refreshed_at
                            .is_none_or(|at| at.elapsed() >= REFRESH_AGAIN_AFTER);
                        if due {
                            this.start_friend_refresh(cx);
                        }
                    })
                    .is_ok();
                if !ok {
                    break;
                }
                cx.background_executor().timer(REFRESH_EVERY).await;
            }
        }));
    }

    /// Refresh, for the window: runs unless one is running.
    pub fn start_friend_refresh(&mut self, cx: &mut Context<Self>) {
        if self.friend_list.refreshing || self.preview {
            return;
        }
        let task = self.friend_refresh(cx);
        self.friend_list.refresh_task = Some(cx.spawn(async move |_, _| {
            let _ = task.await;
        }));
    }

    /// This launcher's Minecraft accounts as friends see them, so their Hosts can put
    /// them on a server's whitelist: Microsoft accounts first, the active one first of
    /// all. The first Microsoft one is the person's profile: its name is their name
    /// until they choose one, and its skin is shown beside it.
    pub(super) fn my_players(&self, cx: &App) -> Vec<StatePlayer> {
        let accounts = self.accounts.read(cx);
        let active = accounts.active().map(|account| account.id.clone());
        let mut accounts: Vec<_> = accounts.accounts().to_vec();
        accounts.sort_by_key(|account| {
            (
                account.kind == AccountKind::Offline,
                Some(&account.id) != active.as_ref(),
            )
        });
        accounts
            .into_iter()
            .map(|account| StatePlayer {
                name: account.username,
                uuid: *account.uuid.as_bytes(),
                offline: account.kind == AccountKind::Offline,
            })
            .collect()
    }

    /// The Minecraft profile this person shows friends (see [`Self::my_players`]).
    pub(super) fn my_profile(&self, cx: &App) -> Option<StatePlayer> {
        profile_of(&self.my_players(cx)).cloned()
    }

    /// Whether the person must choose a name before adding a friend: they have none,
    /// and no Microsoft account whose Minecraft name would be it.
    pub(super) fn needs_friend_name(&self, cx: &App) -> bool {
        self.friend_list.snapshot.book.name.is_empty() && self.my_profile(cx).is_none()
    }

    /// Fetches, in the background, the faces of the Minecraft profiles the friend list
    /// shows: friends', the answers to this person's links, and a looked-up link's
    /// sender. Each profile is asked once a run (and its skin is cached on disk);
    /// nothing waits for it.
    pub fn request_friend_heads(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let list = &self.friend_list;
        let mut wanted: Vec<[u8; 16]> = list
            .snapshot
            .book
            .friends
            .iter()
            .filter_map(|f| f.minecraft())
            .chain(list.replies.iter().filter_map(|r| r.player.as_ref()))
            .filter(|p| !p.offline)
            .map(|p| p.uuid)
            .collect();
        if let Some(FriendDialog::Add(add)) = list.dialog.current().map(|p| &p.dialog)
            && let Some((_, preview)) = &add.preview
            && let Some(player) = preview.player.as_ref().filter(|p| !p.offline)
        {
            wanted.push(player.uuid);
        }
        wanted.retain(|uuid| !list.heads.contains_key(uuid));
        wanted.sort_unstable();
        wanted.dedup();
        if wanted.is_empty() {
            return;
        }
        let accounts = self.accounts.read(cx);
        let http = accounts.http();
        let endpoints = accounts.authenticator().config().endpoints.clone();
        let cache_dir = accounts.data_dir().avatar_cache_dir();
        for uuid in wanted {
            self.friend_list.heads.insert(uuid, Head::Loading);
            let (http, endpoints, cache_dir) = (http.clone(), endpoints.clone(), cache_dir.clone());
            let load = cx.background_spawn(async move {
                let path = skin::cached_profile_head(
                    &*http,
                    &endpoints.textures,
                    &endpoints.sessions,
                    &cache_dir,
                    &uuid,
                )
                .await?;
                match path {
                    Some(path) => crate::images::decode_png(&std::fs::read(path)?).map(Some),
                    None => Ok(None),
                }
            });
            cx.spawn(async move |this, cx| {
                let head = match load.await {
                    Ok(Some(image)) => Head::Ready(image),
                    Ok(None) => Head::None,
                    Err(err) => {
                        eprintln!("lodestone-launcher: loading a friend's skin: {err:#}");
                        Head::None
                    }
                };
                this.update(cx, |this, cx| {
                    this.friend_list.heads.insert(uuid, head);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Reads friends' states (confirming pending friends, dropping retired cores),
    /// checks which cores share now, and asks for answers to open friend links.
    pub fn friend_refresh(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<RefreshReport, String>> {
        self.friend_list.refreshing = true;
        cx.notify();
        let players = self.my_players(cx);
        let task = self.friend_op(
            cx,
            false,
            Box::new(|friends| {
                async move {
                    // this launcher's Minecraft accounts, for friends' whitelists: a
                    // change goes out with the states the refresh publishes
                    friends.set_players(players)?;
                    let mut report = RefreshReport {
                        refreshed: friends.refresh().await?,
                        ..RefreshReport::default()
                    };
                    report.presence = friends.presence().await?;
                    if !friends.book.links.is_empty() {
                        match friends.link_replies().await {
                            Ok(replies) => report.replies = replies,
                            Err(err) => report.problems.push(err.to_string()),
                        }
                    }
                    Ok(report)
                }
                .boxed()
            }),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.friend_list.refreshing = false;
                this.friend_list.refreshed_at = Some(Instant::now());
                match &result {
                    Ok(report) => this.apply_refresh(report),
                    // no list yet is how everyone starts, not a failure
                    Err(err) if err == NO_LIST => this.friend_list.problem = None,
                    Err(err) => this.friend_list.problem = Some(err.clone()),
                }
                this.request_friend_heads(cx);
                cx.notify();
            })
            .ok();
            result
        })
    }

    fn apply_refresh(&mut self, report: &RefreshReport) {
        let list = &mut self.friend_list;
        // cores not checked this time (the service asked to slow down) keep what was
        // seen last
        for p in &report.presence {
            list.presence
                .insert((p.friend.clone(), hex(&p.core.fingerprint)), p.online);
        }
        list.replies = report.replies.clone();
        // the made link is shown until it is answered, cancelled or expires
        if let Some(made) = &list.made
            && (friend_list::now() >= made.expires || list.snapshot.book.links.is_empty())
        {
            list.made = None;
        }
        let mut problems: Vec<String> = report.problems.clone();
        problems.extend(report.refreshed.problems.iter().map(|(key, why)| {
            if key.is_empty() {
                // not about one friend
                return why.clone();
            }
            let name = list
                .snapshot
                .book
                .friend(key)
                .map_or_else(|| short(key).to_string(), |f| f.display_name().to_string());
            format!("{name}: {why}")
        }));
        list.problem = (!problems.is_empty()).then(|| problems.join("; "));
        for fingerprint in &report.refreshed.dropped_cores {
            self.clear_friend_core_trust(fingerprint);
            self.friend_list
                .servers
                .retain(|(_, fp), _| fp != fingerprint);
        }
        for key in &report.refreshed.retired {
            self.clear_friend_trust(key);
            self.friend_list
                .servers
                .retain(|(friend, _), _| friend != key);
        }
    }

    /// Stops trusting the mods of a friend's friend-list servers.
    fn clear_friend_trust(&mut self, key: &str) {
        self.edit_friend_servers(|file| file.clear_friend_trust(key));
    }

    /// Stops trusting the mods of the friend-list servers on a core.
    fn clear_friend_core_trust(&mut self, fingerprint: &str) {
        self.edit_friend_servers(|file| file.clear_core_trust(fingerprint));
    }

    /// Changes the saved friends' servers with `edit`, saving them when it says so.
    fn edit_friend_servers(&mut self, edit: impl FnOnce(&mut FriendsFile) -> bool) {
        let mut file = FriendsFile {
            friends: self.friends.list.iter().map(|f| f.saved.clone()).collect(),
        };
        if !edit(&mut file) {
            return;
        }
        for (entry, saved) in self.friends.list.iter_mut().zip(file.friends) {
            entry.saved = saved;
        }
        self.save_friend_servers();
    }

    /// Shows an error from the friend list at the top of the page.
    fn friend_list_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.notice = Some(Notice::new(move || {
            (t(M::ActionFailed).into(), error.clone())
        }));
        cx.notify();
    }

    // -------------------------------------------------------------------------
    // Operations, for the window and the control API. Each updates what the window
    // shows when it ends, and answers with the result.

    /// Opens the list (making the keys on first use) and answers with its copy.
    pub fn friend_me(&mut self, cx: &mut Context<Self>) -> Task<Result<Snapshot, String>> {
        let players = self.my_players(cx);
        let task = self.friend_op(
            cx,
            true,
            Box::new(|friends| {
                async move {
                    friends.set_players(players)?;
                    Ok(Snapshot::of(friends))
                }
                .boxed()
            }),
        );
        cx.spawn(async move |_, _| task.await)
    }

    pub fn friend_set_name(
        &mut self,
        name: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let players = self.my_players(cx);
        self.friend_op(
            cx,
            true,
            Box::new(move |friends| {
                async move {
                    friends.set_players(players)?;
                    friends.set_name(&name)?;
                    // friends see the new name with the next state
                    let _ = friends.publish_states().await;
                    Ok(())
                }
                .boxed()
            }),
        )
    }

    /// Goes back to the Minecraft name, and follows it from now on.
    pub fn friend_use_minecraft_name(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let players = self.my_players(cx);
        self.friend_op(
            cx,
            true,
            Box::new(move |friends| {
                async move {
                    friends.set_players(players)?;
                    friends.use_profile_name()?;
                    let _ = friends.publish_states().await;
                    Ok(())
                }
                .boxed()
            }),
        )
    }

    pub fn friend_create_link(&mut self, cx: &mut Context<Self>) -> Task<Result<MadeLink, String>> {
        // the Minecraft name is the name, without asking, when there's no other
        let players = self.my_players(cx);
        let task = self.friend_op(
            cx,
            true,
            Box::new(|friends| {
                async move {
                    friends.set_players(players)?;
                    let link = friends.create_link().await?;
                    let expires = friends.book.links.last().map_or(0, |l| l.expires);
                    Ok(MadeLink { link, expires })
                }
                .boxed()
            }),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            if let Ok(made) = &result {
                let made = made.clone();
                this.update(cx, |this, cx| {
                    this.friend_list.made = Some(made);
                    cx.notify();
                })
                .ok();
            }
            result
        })
    }

    pub fn friend_cancel_link(
        &mut self,
        code: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let cancel = code.clone();
        let task = self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.cancel_link(&cancel).await }.boxed()),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.friend_list.replies.retain(|r| r.code != code);
                if this
                    .friend_list
                    .made
                    .as_ref()
                    .is_some_and(|m| m.link.contains(&code))
                {
                    this.friend_list.made = None;
                }
                cx.notify();
            })
            .ok();
            result
        })
    }

    pub fn friend_replies(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<LinkReply>, String>> {
        let task = self.friend_op(
            cx,
            false,
            Box::new(|friends| async move { friends.link_replies().await }.boxed()),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            if let Ok(replies) = &result {
                let replies = replies.clone();
                this.update(cx, |this, cx| {
                    this.friend_list.replies = replies;
                    this.request_friend_heads(cx);
                    cx.notify();
                })
                .ok();
            }
            result
        })
    }

    /// Accepts `key`'s answer to the link `code`: they become a friend (not verified),
    /// and this person's first state goes to them as the confirmation.
    pub fn friend_accept(
        &mut self,
        code: String,
        key: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        self.friend_list.busy.insert(key.clone());
        let (accept_code, accept_key) = (code.clone(), key.clone());
        let task = self.friend_op(
            cx,
            false,
            Box::new(move |friends| {
                async move { friends.accept_reply(&accept_code, &accept_key).await }.boxed()
            }),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.friend_list.busy.remove(&key);
                if result.is_ok() {
                    // the link is used up: its other answers go with it
                    this.friend_list.replies.retain(|r| r.code != code);
                    if this
                        .friend_list
                        .made
                        .as_ref()
                        .is_some_and(|m| m.link.contains(&code))
                    {
                        this.friend_list.made = None;
                    }
                }
                cx.notify();
            })
            .ok();
            result
        })
    }

    pub fn friend_preview(
        &mut self,
        link: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<LinkPreview, String>> {
        // Looking at a link makes nothing: without a friend list (a link opened from
        // outside the launcher, say), the card is checked without one.
        let Some(opening) = self.friends_opening(cx) else {
            return Task::ready(Err("The friend list isn't available here.".into()));
        };
        cx.spawn(async move |this, cx| {
            let handle = Self::opened_handle(opening, &this, cx).await?;
            let copy = handle.lock().await.clone();
            cx.background_spawn(async move {
                match copy {
                    Some(mut friends) => friends.preview_link(&link).await,
                    None => ops::preview_link(&service(), None, &link).await,
                }
            })
            .await
            .map_err(|err| err.to_string())
        })
    }

    /// Answers a friend link: its sender becomes a pending friend. Only after the
    /// person saw the card (the window's Add, or the control API with confirmation).
    pub fn friend_answer(
        &mut self,
        link: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<String, String>> {
        let players = self.my_players(cx);
        self.friend_op(
            cx,
            true,
            Box::new(move |friends| {
                async move {
                    friends.set_players(players)?;
                    friends.answer_link(&link).await
                }
                .boxed()
            }),
        )
    }

    pub fn friend_set_verified(
        &mut self,
        key: String,
        verified: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.set_verified(&key, verified) }.boxed()),
        )
    }

    pub fn friend_set_nickname(
        &mut self,
        key: String,
        nickname: Option<String>,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        self.friend_op(
            cx,
            false,
            Box::new(move |friends| {
                async move { friends.set_nickname(&key, nickname.as_deref()) }.boxed()
            }),
        )
    }

    pub fn friend_set_hidden(
        &mut self,
        key: String,
        hidden: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.set_hidden(&key, hidden) }.boxed()),
        )
    }

    /// Removes a friend, and stops trusting the mods of their cores' servers.
    pub fn friend_remove(
        &mut self,
        key: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let cores: Vec<String> = self
            .friend_list
            .friend(&key)
            .and_then(|f| f.state.as_ref())
            .map(|state| state.cores.iter().map(|c| hex(&c.fingerprint)).collect())
            .unwrap_or_default();
        let remove = key.clone();
        let task = self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.remove(&remove).await }.boxed()),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            if result.is_ok() {
                this.update(cx, |this, cx| {
                    this.clear_friend_trust(&key);
                    for fingerprint in &cores {
                        this.clear_friend_core_trust(fingerprint);
                    }
                    this.friend_list.expanded.remove(&key);
                    this.friend_list
                        .presence
                        .retain(|(friend, _), _| friend != &key);
                    this.friend_list
                        .servers
                        .retain(|(friend, _), _| friend != &key);
                    cx.notify();
                })
                .ok();
            }
            result
        })
    }

    /// Retires this person's friend identity: a final, retired state goes to every
    /// friend, then the person key, the file key and the friend file are deleted. The
    /// window starts over with an empty list.
    pub fn friend_retire(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<(String, String)>, String>> {
        let Some(opening) = self.friends_opening(cx) else {
            return Task::ready(Err("The friend list isn't available here.".into()));
        };
        let accounts = self.accounts.read(cx);
        let secrets = accounts.secrets().clone();
        let data = accounts.data_dir().clone();
        let keys: Vec<String> = self
            .friend_list
            .snapshot
            .book
            .friends
            .iter()
            .map(|f| f.key.clone())
            .collect();
        cx.spawn(async move |this, cx| {
            let handle = opening.await?;
            // takes the list: no operation starts on it after this, and those running
            // on copies find the book retired when they next change it
            let mut friends = handle
                .lock()
                .await
                .take()
                .ok_or_else(|| NO_LIST.to_string())?;
            let result = cx
                .background_spawn(async move { friends.retire_identity().await })
                .await;
            // the person key goes last: if deleting it fails, the book still says
            // retired, so the identity is never used again, and starting a new one
            // deletes it
            let result = match result {
                Ok(unreached) => {
                    let store = secrets.store(cx);
                    friend_list::forget_identity(&data, &*store)
                        .await
                        .map(|()| unreached)
                }
                Err(err) => Err(err),
            }
            .map_err(|err| err.to_string());
            this.update(cx, |this, cx| {
                // retired or not, the old list is gone: the next use opens a new one
                for key in &keys {
                    this.clear_friend_trust(key);
                }
                this.friend_list.forget();
                cx.notify();
            })
            .ok();
            result
        })
    }

    /// The servers a friend's core shares with this person.
    pub fn friend_servers(
        &mut self,
        key: String,
        fingerprint: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<SharedServer>, String>> {
        let Some(core) = unhex32(&fingerprint) else {
            return Task::ready(Err(format!("{fingerprint:?} isn't a core fingerprint.")));
        };
        let slot = (key.clone(), fingerprint);
        self.friend_list
            .servers
            .insert(slot.clone(), CoreServers::Loading);
        cx.notify();
        let task = self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.servers(&key, &core).await }.boxed()),
        );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let shown = match &result {
                    Ok(servers) => CoreServers::Loaded(servers.clone()),
                    Err(err) => CoreServers::Failed(err.clone()),
                };
                this.friend_list.servers.insert(slot, shown);
                cx.notify();
            })
            .ok();
            result
        })
    }

    /// Retires one of this person's cores: it leaves their state, friends drop it, and
    /// it's never listed again. Answers with the friends it couldn't reach.
    pub fn friend_retire_core(
        &mut self,
        fingerprint: String,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<(String, String)>, String>> {
        self.friend_op(
            cx,
            false,
            Box::new(move |friends| async move { friends.retire_core(&fingerprint).await }.boxed()),
        )
    }

    /// A tunnel to `instance` on a friend's core, for the join dialog. It opens on a
    /// copy of the list, so the list isn't held while it does (a few seconds).
    pub(super) fn friend_list_tunnel(
        &mut self,
        friend: String,
        fingerprint: &str,
        instance: String,
        game_port: GamePort,
        progress: Progress,
        cx: &mut Context<Self>,
    ) -> LocalBoxFuture<'static, Result<Tunnel, JoinError>> {
        let Some(core) = unhex32(fingerprint) else {
            return futures::future::ready(Err(JoinError::Failed(
                "The friend's core isn't known.".into(),
            )))
            .boxed_local();
        };
        let Some(opening) = self.friends_opening(cx) else {
            return futures::future::ready(Err(JoinError::Failed(
                "The friend list isn't available here.".into(),
            )))
            .boxed_local();
        };
        async move {
            let handle = opening.await.map_err(JoinError::Failed)?;
            let friends = handle
                .lock()
                .await
                .clone()
                .ok_or_else(|| JoinError::Failed(NO_LIST.to_string()))?;
            friends
                .open_tunnel(&friend, &core, &instance, game_port, progress)
                .await
        }
        .boxed_local()
    }

    // -------------------------------------------------------------------------
    // The window

    /// Play on a server a friend's core shares: the join dialog, through the friend
    /// list, then Play. A server joined before whose core runs on this computer, with
    /// the launcher as its owner, is joined straight at the core, as from its row.
    pub fn play_friend_server(
        &mut self,
        friend: &str,
        fingerprint: &str,
        server: &SharedServer,
        cx: &mut Context<Self>,
    ) {
        let saved =
            launcher_core::cores::friends::friend_list_id(fingerprint, &server.instance_uuid);
        if self.friend_direct(&saved).is_some() {
            self.open_friend(&saved, true, cx);
            return;
        }
        self.connect_source(
            Source::FriendList {
                friend: friend.to_string(),
                fingerprint: fingerprint.to_string(),
                instance: server.instance_uuid.clone(),
                name: Some(server.name.clone()),
            },
            true,
            cx,
        );
    }

    /// Shows or hides a friend's servers, asking their cores when shown.
    pub fn toggle_friend(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.friend_list.expanded.remove(key) {
            self.friend_list.expanded.insert(key.to_string());
            self.load_friend_servers(key, cx);
        }
        cx.notify();
    }

    /// Asks each of a friend's cores that isn't known to be offline for its servers.
    pub fn load_friend_servers(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let Some(friend) = self.friend_list.friend(key) else {
            return;
        };
        let cores: Vec<String> = friend
            .joinable_cores(friend_list::now())
            .iter()
            .map(|c| hex(&c.fingerprint))
            .filter(|fp| {
                self.friend_list
                    .presence
                    .get(&(key.to_string(), fp.clone()))
                    != Some(&false)
            })
            .collect();
        for fingerprint in cores {
            self.friend_servers(key.to_string(), fingerprint, cx)
                .detach();
        }
    }

    fn friend_dialog(&mut self, dialog: FriendDialog, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.friend_list
            .dialog
            .open(FriendPrompt { dialog, focus }, cx);
        cx.notify();
    }

    pub fn close_friend_dialog(&mut self, cx: &mut Context<Self>) {
        self.friend_list.dialog.close(cx);
        cx.notify();
    }

    fn name_input(&self, cx: &mut Context<Self>) -> Entity<TextInput> {
        let mut name = self.friend_list.snapshot.book.name.clone();
        if name.is_empty() {
            name = self.my_profile(cx).map(|p| p.name).unwrap_or_default();
        }
        cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::NamePlaceholder)
                .with_automation_id("friend-name-input")
                .with_text(name)
        })
    }

    /// "Choose name" or "Edit name".
    pub fn open_friend_name(&mut self, cx: &mut Context<Self>) {
        let input = self.name_input(cx);
        self.friend_dialog(
            FriendDialog::Name {
                input,
                error: None,
                saving: false,
            },
            cx,
        );
    }

    /// Saves the name in the Name dialog, or in the Add friend dialog's name field.
    pub fn save_friend_name(&mut self, cx: &mut Context<Self>) {
        let input = match self.friend_list.dialog.current().map(|p| &p.dialog) {
            Some(FriendDialog::Name { input, .. }) => input.clone(),
            Some(FriendDialog::Add(add)) => match &add.name_input {
                Some(input) => input.clone(),
                None => return,
            },
            _ => return,
        };
        let name = input.read(cx).text().trim().to_string();
        if name.is_empty() {
            return self.set_name_state(false, Some(t(M::NameEmpty).into()), cx);
        }
        self.set_name_state(true, None, cx);
        let task = self.friend_set_name(name, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => match this.friend_list.dialog.current_mut().map(|p| &mut p.dialog) {
                    Some(FriendDialog::Add(add)) => {
                        add.name_input = None;
                        add.saving_name = false;
                        cx.notify();
                    }
                    _ => this.close_friend_dialog(cx),
                },
                Err(err) => this.set_name_state(false, Some(err.into()), cx),
            })
            .ok();
        })
        .detach();
    }

    /// "Use my Minecraft name" in the Name dialog.
    pub fn use_minecraft_name(&mut self, cx: &mut Context<Self>) {
        self.set_name_state(true, None, cx);
        let task = self.friend_use_minecraft_name(cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.close_friend_dialog(cx),
                Err(err) => this.set_name_state(false, Some(err.into()), cx),
            })
            .ok();
        })
        .detach();
    }

    fn set_name_state(
        &mut self,
        busy: bool,
        failure: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        match self.friend_list.dialog.current_mut().map(|p| &mut p.dialog) {
            Some(FriendDialog::Name { error, saving, .. }) => {
                *error = failure;
                *saving = busy;
            }
            Some(FriendDialog::Add(add)) => {
                add.error = failure;
                add.saving_name = busy;
            }
            _ => {}
        }
        cx.notify();
    }

    /// "Add friend", or a friend link opened from outside the launcher (or pasted):
    /// the dialog with the link filled in. A link to this launcher's service is looked
    /// up at once; nothing is added until the person presses Add.
    pub fn open_add_friend(&mut self, link: Option<String>, cx: &mut Context<Self>) {
        let parsed = link.as_deref().map(FriendLink::parse);
        let text = match (&parsed, &link) {
            (Some(Ok(parsed)), _) => parsed.link(),
            (_, Some(text)) => text.trim().to_string(),
            _ => String::new(),
        };
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::LinkPlaceholder)
                .with_automation_id("friend-link-input")
                .with_text(text.clone())
        });
        let name_input = self.needs_friend_name(cx).then(|| self.name_input(cx));
        // a link for another service is refused before anything is sent to it
        let (error, other_service, look_up) = match &parsed {
            Some(Ok(parsed)) if parsed.service == service() => (None, None, true),
            Some(Ok(parsed)) => (
                Some(tr!(M::OtherService, host = parsed.service.host).into()),
                Some(parsed.service.host.clone()),
                false,
            ),
            Some(Err(_)) => (Some(t(M::NotAFriendLink).into()), None, false),
            None => (None, None, false),
        };
        let add = AddFriend {
            name_input,
            saving_name: false,
            making: false,
            copied: false,
            input,
            looking: false,
            preview: None,
            adding: false,
            error,
            other_service,
            added: None,
        };
        self.friend_dialog(FriendDialog::Add(Box::new(add)), cx);
        if look_up {
            self.look_up_friend_link(cx);
        }
    }

    fn add_state(&mut self) -> Option<&mut AddFriend> {
        match self.friend_list.dialog.current_mut().map(|p| &mut p.dialog) {
            Some(FriendDialog::Add(add)) => Some(add),
            _ => None,
        }
    }

    /// "Make a link" in the Add friend dialog.
    pub fn make_friend_link(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add_state() else {
            return;
        };
        if add.making {
            return;
        }
        add.making = true;
        add.error = None;
        let task = self.friend_create_link(cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(add) = this.add_state() {
                    add.making = false;
                    add.copied = false;
                    if let Err(err) = result {
                        add.error = Some(err.into());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// "Copy" beside the made link.
    pub fn copy_friend_link(&mut self, cx: &mut Context<Self>) {
        let Some(made) = self.friend_list.made.clone() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(made.link));
        if let Some(add) = self.add_state() {
            add.copied = true;
        }
        cx.notify();
    }

    /// "Look up" in the Add friend dialog: opens the link's card.
    pub fn look_up_friend_link(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add_state() else {
            return;
        };
        let text = add.input.read(cx).text().trim().to_string();
        if text.is_empty() {
            add.error = Some(t(M::PasteFirst).into());
            add.preview = None;
            cx.notify();
            return;
        }
        match FriendLink::parse(&text) {
            Err(_) => {
                add.error = Some(t(M::NotAFriendLink).into());
                add.preview = None;
                cx.notify();
                return;
            }
            Ok(link) if link.service != service() => {
                add.error = Some(tr!(M::OtherService, host = link.service.host).into());
                add.other_service = Some(link.service.host.clone());
                add.preview = None;
                cx.notify();
                return;
            }
            Ok(_) => {}
        }
        add.looking = true;
        add.error = None;
        add.preview = None;
        add.other_service = None;
        let task = self.friend_preview(text.clone(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(add) = this.add_state() {
                    add.looking = false;
                    match result {
                        Ok(preview) => add.preview = Some((text, preview)),
                        Err(err) => add.error = Some(err.into()),
                    }
                }
                this.request_friend_heads(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// "Back" from a looked-up card.
    pub fn clear_friend_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(add) = self.add_state() {
            add.preview = None;
            add.error = None;
        }
        cx.notify();
    }

    /// "Add" under a looked-up card: answers the link.
    pub fn add_friend_from_link(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add_state() else {
            return;
        };
        let Some((link, preview)) = add.preview.clone() else {
            return;
        };
        if add.adding || preview.already_friends {
            return;
        }
        add.adding = true;
        add.error = None;
        let task = self.friend_answer(link, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(add) = this.add_state() {
                    add.adding = false;
                    match result {
                        Ok(_) => {
                            add.added = Some(preview.name.clone());
                            add.preview = None;
                        }
                        Err(err) => add.error = Some(err.into()),
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Accept on an answer.
    pub fn accept_friend_reply(&mut self, code: &str, key: &str, cx: &mut Context<Self>) {
        let task = self.friend_accept(code.to_string(), key.to_string(), cx);
        cx.spawn(async move |this, cx| {
            if let Err(err) = task.await {
                this.update(cx, |this, cx| this.friend_list_failed(err, cx))
                    .ok();
            }
        })
        .detach();
        cx.notify();
    }

    /// Ignore on an answer: it is no longer shown. The link stays open.
    pub fn ignore_friend_reply(&mut self, code: &str, key: &str, cx: &mut Context<Self>) {
        self.friend_list
            .ignored
            .insert((code.to_string(), key.to_string()));
        cx.notify();
    }

    pub fn cancel_friend_link(&mut self, code: &str, cx: &mut Context<Self>) {
        let task = self.friend_cancel_link(code.to_string(), cx);
        cx.spawn(async move |this, cx| {
            if let Err(err) = task.await {
                this.update(cx, |this, cx| this.friend_list_failed(err, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub fn check_friend_replies(&mut self, cx: &mut Context<Self>) {
        let task = self.friend_replies(cx);
        cx.spawn(async move |this, cx| {
            if let Err(err) = task.await {
                this.update(cx, |this, cx| this.friend_list_failed(err, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Runs a small change and reports a failure at the top of the page.
    fn friend_change(&mut self, task: Task<Result<(), String>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if let Err(err) = task.await {
                this.update(cx, |this, cx| this.friend_list_failed(err, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub fn open_friend_verify(&mut self, key: &str, cx: &mut Context<Self>) {
        self.friend_dialog(
            FriendDialog::Verify {
                key: key.to_string(),
            },
            cx,
        );
    }

    /// "Codes match" in the compare dialog.
    pub fn set_friend_verified(&mut self, key: &str, verified: bool, cx: &mut Context<Self>) {
        // the box ticks at once; the file follows
        if let Some(friend) = self.friend_list.snapshot.book.friend_mut(key) {
            friend.verified = verified;
        }
        let task = self.friend_set_verified(key.to_string(), verified, cx);
        self.friend_change(task, cx);
        cx.notify();
    }

    pub fn open_friend_rename(&mut self, key: &str, cx: &mut Context<Self>) {
        let nickname = self
            .friend_list
            .friend(key)
            .and_then(|f| f.nickname.clone())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(M::NicknamePlaceholder)
                .with_automation_id("friend-nickname-input")
                .with_text(nickname)
        });
        self.friend_dialog(
            FriendDialog::Rename {
                key: key.to_string(),
                input,
            },
            cx,
        );
    }

    /// Save (or "Use their name", with `clear`) in the rename dialog.
    pub fn save_friend_nickname(&mut self, clear: bool, cx: &mut Context<Self>) {
        let Some(FriendDialog::Rename { key, input }) =
            self.friend_list.dialog.current().map(|p| &p.dialog)
        else {
            return;
        };
        let key = key.clone();
        let text = input.read(cx).text().trim().to_string();
        let nickname = (!clear && !text.is_empty()).then_some(text);
        let task = self.friend_set_nickname(key, nickname, cx);
        self.friend_change(task, cx);
        self.close_friend_dialog(cx);
    }

    pub fn toggle_friend_hidden(&mut self, key: &str, cx: &mut Context<Self>) {
        let hidden = self.friend_list.friend(key).is_some_and(|f| f.hidden);
        let task = self.friend_set_hidden(key.to_string(), !hidden, cx);
        self.friend_change(task, cx);
    }

    pub fn open_friend_remove(&mut self, key: &str, cx: &mut Context<Self>) {
        self.friend_dialog(
            FriendDialog::Remove {
                key: key.to_string(),
                removing: false,
                error: None,
            },
            cx,
        );
    }

    /// "Remove friend" in the confirmation.
    pub fn confirm_friend_remove(&mut self, cx: &mut Context<Self>) {
        let Some(FriendDialog::Remove {
            key,
            removing,
            error,
        }) = self.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
        else {
            return;
        };
        if *removing {
            return;
        }
        *removing = true;
        *error = None;
        let key = key.clone();
        let task = self.friend_remove(key, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.close_friend_dialog(cx),
                Err(err) => {
                    if let Some(FriendDialog::Remove {
                        removing, error, ..
                    }) = this.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
                    {
                        *removing = false;
                        *error = Some(err.into());
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Servers {
    pub fn open_friend_retire(&mut self, cx: &mut Context<Self>) {
        self.friend_dialog(
            FriendDialog::Retire {
                understood: false,
                retiring: false,
                error: None,
            },
            cx,
        );
    }

    /// The "I understand" box of the retire dialog.
    pub fn set_retire_understood(&mut self, value: bool, cx: &mut Context<Self>) {
        if let Some(FriendDialog::Retire { understood, .. }) =
            self.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
        {
            *understood = value;
        }
        cx.notify();
    }

    /// "Retire identity" in the dialog, once the box is ticked.
    pub fn confirm_friend_retire(&mut self, cx: &mut Context<Self>) {
        let Some(FriendDialog::Retire {
            understood: true,
            retiring,
            error,
        }) = self.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
        else {
            return;
        };
        if *retiring {
            return;
        }
        *retiring = true;
        *error = None;
        let task = self.friend_retire(cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(_) => this.close_friend_dialog(cx),
                Err(err) => {
                    if let Some(FriendDialog::Retire {
                        retiring, error, ..
                    }) = this.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
                    {
                        *retiring = false;
                        *error = Some(err.into());
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Servers {
    /// "Retire" beside one of this person's cores in the "You" row.
    pub fn open_friend_retire_core(&mut self, fingerprint: &str, cx: &mut Context<Self>) {
        let name = self
            .friend_list
            .snapshot
            .book
            .my_cores
            .iter()
            .find(|c| c.fingerprint == fingerprint)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        self.friend_dialog(
            FriendDialog::RetireCore {
                fingerprint: fingerprint.to_string(),
                name,
                retiring: false,
                error: None,
            },
            cx,
        );
    }

    /// "Retire core" in the confirmation.
    pub fn confirm_friend_retire_core(&mut self, cx: &mut Context<Self>) {
        let Some(FriendDialog::RetireCore {
            fingerprint,
            retiring,
            error,
            ..
        }) = self.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
        else {
            return;
        };
        if *retiring {
            return;
        }
        *retiring = true;
        *error = None;
        let fingerprint = fingerprint.clone();
        let task = self.friend_retire_core(fingerprint, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(_) => this.close_friend_dialog(cx),
                Err(err) => {
                    if let Some(FriendDialog::RetireCore {
                        retiring, error, ..
                    }) = this.friend_list.dialog.current_mut().map(|p| &mut p.dialog)
                    {
                        *retiring = false;
                        *error = Some(err.into());
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

// -----------------------------------------------------------------------------
// Offscreen screenshots

/// What the friend list shows in an offscreen screenshot.
pub struct PreviewFriendList {
    pub snapshot: Snapshot,
    pub replies: Vec<LinkReply>,
    pub presence: Vec<((String, String), bool)>,
    pub expanded: Vec<(String, Vec<(String, CoreServers)>)>,
    pub made: Option<MadeLink>,
    pub opened: bool,
    /// Minecraft profiles whose faces show: a made-up skin's head for each (with a
    /// hat or without).
    pub heads: Vec<([u8; 16], bool)>,
}

/// The Add friend dialog in an offscreen screenshot.
pub enum PreviewAddFriend {
    Empty,
    Preview { link: String, preview: LinkPreview },
}

impl Servers {
    /// Shows a made-up friend list. Nothing is opened, read or sent.
    pub fn preview_friend_list(&mut self, preview: PreviewFriendList) {
        let list = &mut self.friend_list;
        list.opened = preview.opened;
        list.snapshot = preview.snapshot;
        list.replies = preview.replies;
        list.presence = preview.presence.into_iter().collect();
        list.made = preview.made;
        list.refreshed_at = Some(Instant::now());
        for (uuid, hat) in preview.heads {
            // through the same code as real skins
            let image = skin::encode_png(&skin::sample_skin(hat))
                .and_then(|png| skin::render_head(&png, skin::HEAD_SCALE))
                .and_then(|head| skin::encode_png(&head))
                .ok()
                .and_then(|png| crate::images::decode_png(&png).ok());
            if let Some(image) = image {
                list.heads.insert(uuid, Head::Ready(image));
            }
        }
        for (key, cores) in preview.expanded {
            for (fingerprint, servers) in cores {
                list.servers.insert((key.clone(), fingerprint), servers);
            }
            list.expanded.insert(key);
        }
    }

    /// Opens one of the friend list's dialogs with made-up content.
    pub fn preview_friend_dialog(
        &mut self,
        which: &str,
        add: PreviewAddFriend,
        cx: &mut Context<Self>,
    ) {
        match which {
            "add" => {
                self.open_add_friend(None, cx);
                if let PreviewAddFriend::Preview { link, preview } = add
                    && let Some(state) = self.add_state()
                {
                    state
                        .input
                        .update(cx, |input, cx| input.set_text(link.clone(), cx));
                    state.preview = Some((link, preview));
                }
            }
            "verify" => {
                if let Some(friend) = self.friend_list.snapshot.book.friends.first() {
                    let key = friend.key.clone();
                    self.open_friend_verify(&key, cx);
                }
            }
            "remove" => {
                if let Some(friend) = self.friend_list.snapshot.book.friends.first() {
                    let key = friend.key.clone();
                    self.open_friend_remove(&key, cx);
                }
            }
            "name" => self.open_friend_name(cx),
            "retire" => self.open_friend_retire(cx),
            _ => {}
        }
    }

    /// Opens a dialog about one made-up friend.
    pub fn preview_friend_dialog_for(&mut self, which: &str, key: &str, cx: &mut Context<Self>) {
        match which {
            "verify" => self.open_friend_verify(key, cx),
            "remove" => self.open_friend_remove(key, cx),
            _ => {}
        }
    }
}

/// Whether the friend's status is pending (for the window and the control API).
pub fn status_name(status: Status) -> &'static str {
    match status {
        Status::Pending => "pending",
        Status::Active => "active",
        Status::Retired => "retired",
    }
}

/// A well-formed friend link for `service`, for tests.
#[cfg(test)]
pub fn test_link(service: &launcher_core::cores::friends::Service) -> String {
    let sender = Friend {
        key: "ab".repeat(32),
        ..Friend::default()
    };
    FriendLink::new(service.clone(), &sender.public_key().expect("a key"))
        .expect("a link")
        .link()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::friend_list::FriendState;

    fn friend(key: &str, name: &str, nickname: Option<&str>) -> Friend {
        Friend {
            key: key.to_string(),
            name: name.to_string(),
            nickname: nickname.map(str::to_string),
            ..Friend::default()
        }
    }

    fn book() -> Book {
        Book {
            name: "Me".into(),
            friends: vec![
                friend(&"ab".repeat(32), "Alex", None),
                friend(&"abcd".repeat(16), "Sam", Some("Sammy")),
                friend(&"cd".repeat(32), "alex", Some("Other Alex")),
            ],
            ..Book::default()
        }
    }

    #[test]
    fn friends_are_found_by_key_prefix_or_name() {
        let book = book();
        assert_eq!(find_friend(&book, &"ab".repeat(32)), Ok("ab".repeat(32)));
        assert_eq!(find_friend(&book, "CDCDCDCD"), Ok("cd".repeat(32)));
        // too short to be a prefix, and no one is called that
        assert_eq!(find_friend(&book, "cdcd"), Err(FindError::NotFound));
        // two keys start with abab…
        assert_eq!(find_friend(&book, "abababab"), Ok("ab".repeat(32)));
        assert_eq!(find_friend(&book, "abcdabcd"), Ok("abcd".repeat(16)));
        assert_eq!(find_friend(&book, "abab"), Err(FindError::NotFound));
        // nickname or chosen name, any case
        assert_eq!(find_friend(&book, "sammy"), Ok("abcd".repeat(16)));
        assert_eq!(find_friend(&book, "Sam"), Ok("abcd".repeat(16)));
        assert_eq!(find_friend(&book, " other alex "), Ok("cd".repeat(32)));
        // "alex" is one friend's name and another's chosen name
        assert_eq!(find_friend(&book, "ALEX"), Err(FindError::Ambiguous));
        assert_eq!(find_friend(&book, "nobody"), Err(FindError::NotFound));
    }

    #[test]
    fn a_friend_is_online_when_any_core_shares() {
        let mut alex = friend(&"ab".repeat(32), "Alex", None);
        let core = |byte: u8| {
            serde_json::from_value(
                serde_json::json!({ "fingerprint": hex(&[byte; 32]), "name": "PC" }),
            )
            .unwrap()
        };
        alex.status = Status::Active;
        alex.state = Some(FriendState {
            players: Vec::new(),
            seq: 1,
            expires: friend_list::now() + 3600,
            cores: vec![core(1), core(2)],
        });
        let mut presence = HashMap::new();
        assert_eq!(friend_online(&alex, &presence), None);
        presence.insert((alex.key.clone(), hex(&[1; 32])), false);
        assert_eq!(friend_online(&alex, &presence), Some(false));
        presence.insert((alex.key.clone(), hex(&[2; 32])), true);
        assert_eq!(friend_online(&alex, &presence), Some(true));
        // a pending friend has no cores to join, whatever was seen
        alex.status = Status::Pending;
        assert_eq!(friend_online(&alex, &presence), None);
        // an expired state's cores aren't joined
        alex.status = Status::Active;
        alex.state.as_mut().unwrap().expires = 1;
        assert_eq!(friend_online(&alex, &presence), None);
    }

    #[test]
    fn short_ids_are_a_key_s_start() {
        assert_eq!(short(&"ab".repeat(32)), "abababababab");
        assert_eq!(short("abc"), "abc");
    }
}
