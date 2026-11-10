//! Sharing this computer's servers with friends (launcher/docs/friends.md).
//!
//! The friend list is Lodestone Launcher's: `friend_list.json` in the launcher's data
//! folder, sealed with a key from the Keychain ([`crate::keychain`]), so this person is
//! one friend whichever app they use. The Host adds friends nowhere and never makes a
//! friend identity: it opens the list that is there ([`Friends::open_existing`]) and
//! keeps its own core in step with it ([`core_sync`]): each friend who is active gets
//! their core secret for this core, the core keeps what's shared with each (hidden
//! friends too, serving them nothing), and this person's state lists the core so
//! friends find it.
//!
//! - At the start (once the core is running and signed in as its owner), after the
//!   person ticks or unticks a friend, and whenever the friend list changes (it's read
//!   every 60 s and compared with what was last synced, [`SyncView`]), the Host syncs
//!   the core, then publishes the state to friends who don't have it yet. A sync that
//!   failed is tried again every minute; opening the list (the Keychain) only on Try
//!   again or when the file changes.
//! - Without a friend file, or with no name or no active friends, it does nothing and
//!   the instance's Settings say "Add friends in Lodestone Launcher".
//! - **A missing file stops it, never empties the core.** When the file goes (the data
//!   folder moved, say) or can't be opened any more, the Host drops the list, looks
//!   for the data folder again and opens what it finds. Only a list whose identity was
//!   retired empties the core, on purpose.
//! - **Nothing is shared by default.** A friend sees an instance only once it's ticked
//!   for them, and the first tick for a friend whose safety code wasn't compared asks
//!   first.
//! - A test copy (its core's data folder isn't the usual one, or the launcher's is
//!   `LODESTONE_LAUNCHER_DATA_DIR` while the core's is the usual one) has sharing off
//!   ([`sharing_allowed`]), so it never syncs a real friend list into a test core, or a
//!   test one into the real core.
//!
//! Failures are kept in [`FriendSharing::error`], in plain words, beside the last list
//! that worked.

use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use futures::lock::Mutex;
use gpui::{AppContext as _, Context, Task, WeakEntity};
use launcher_core::{
    cores::admin::AdminClient,
    friend_list::{
        self, Book, FriendStatus, Friends, FriendsError, Opened, StatePlayer,
        core_sync::{self, CoreFriends, SyncError},
    },
    paths::DataDir,
};
use lodestone_connect::Service;

use crate::{
    core_process,
    i18n::{InstanceMsg as M, tr},
    keychain::KeychainStore,
    state::HostState,
};

/// How often the friend list is read and compared with what was synced.
pub const CHECK_EVERY: Duration = Duration::from_secs(60);

/// Whether this Host may share from its core with the friend list it would find: only
/// when both are real (the core in `~/.lodestone`, the launcher's usual data folder) or
/// both are test ones (`LODESTONE_PATH` elsewhere and `LODESTONE_LAUNCHER_DATA_DIR`
/// set). A test core never gets the real friend list (a dev bundle would otherwise
/// share from the person's real identity), and the real core never gets a test one
/// (which would replace its friends).
pub fn sharing_allowed(core_data: &Path, home: Option<&Path>, launcher_overridden: bool) -> bool {
    let real_core = home.is_some_and(|home| core_data == home.join(".lodestone"));
    real_core != launcher_overridden
}

fn launcher_overridden() -> bool {
    std::env::var_os("LODESTONE_LAUNCHER_DATA_DIR").is_some_and(|dir| !dir.is_empty())
}

/// Which core this is, for the person's state ([`friend_list::MyCore::source`]): its
/// data folder. When this core comes back with another friend certificate (its data
/// was reset, say), the old one leaves the state; cores added elsewhere stay.
pub fn core_source(core_data: &Path) -> String {
    let digest = lodestone_connect::friend::crypto::sha256(core_data.to_string_lossy().as_bytes());
    format!("host:{}", friend_list::hex(&digest[..8]))
}

/// A friend the instance's Settings list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FriendRow {
    /// Their public key: their id.
    pub key: String,
    /// The nickname this person gave them, else their name.
    pub name: String,
    /// The two compared safety codes.
    pub verified: bool,
    /// Their safety code, so a friend named like another can be told apart.
    pub safety_code: String,
    /// Their Minecraft accounts, as their state lists them: put on a server's
    /// whitelist when it's shared with them ([`crate::whitelist`]).
    pub players: Vec<StatePlayer>,
}

/// The friends things can be shared with: active and not hidden (a hidden friend is
/// served nothing, so sharing with them would do nothing yet), by name.
pub fn listed(book: &Book) -> Vec<FriendRow> {
    let mut rows: Vec<FriendRow> = book
        .friends
        .iter()
        .filter(|f| f.status == FriendStatus::Active && !f.hidden)
        .map(|f| FriendRow {
            key: f.key.clone(),
            name: f.display_name().to_string(),
            verified: f.verified,
            safety_code: f.safety_code(),
            players: f
                .state
                .as_ref()
                .map(|state| state.players.clone())
                .unwrap_or_default(),
        })
        .collect();
    rows.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.key.cmp(&b.key))
    });
    rows
}

/// Whether the person still has to set up friends in Lodestone Launcher: no name, or
/// no one to share with.
pub fn needs_launcher(book: &Book) -> bool {
    book.name.trim().is_empty() || listed(book).is_empty()
}

/// What of the friend list a core's sync depends on (and the rows the Settings show).
/// The Host syncs again when it changes, and only then: its own writes to the file
/// (state numbers) don't count, and a change the other app made in the same second is
/// still seen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncView {
    name: String,
    /// Each friend's key, name shown, status, hidden and verified.
    friends: Vec<(String, String, FriendStatus, bool, bool)>,
    retired_cores: Vec<String>,
}

impl SyncView {
    pub fn of(book: &Book) -> Self {
        Self {
            name: book.name.clone(),
            friends: book
                .friends
                .iter()
                .map(|f| {
                    (
                        f.key.clone(),
                        f.display_name().to_string(),
                        f.status,
                        f.hidden,
                        f.verified,
                    )
                })
                .collect(),
            retired_cores: book.retired_cores.clone(),
        }
    }
}

/// What the friend `key` should have shared on this core once `instance` is shared
/// (`share`) or not: their list on the core now, with it added or taken out.
pub fn shared_after(current: &CoreFriends, key: &str, instance: &str, share: bool) -> Vec<String> {
    let mut list: Vec<String> = current
        .friends
        .iter()
        .find(|f| f.key == key)
        .map(|f| f.shared.clone())
        .unwrap_or_default();
    list.retain(|uuid| uuid != instance);
    if share {
        list.push(instance.to_string());
    }
    list
}

/// Whether `instance` is shared with the friend `key`, as the core said last.
pub fn is_shared(core: Option<&CoreFriends>, key: &str, instance: &str) -> bool {
    core.and_then(|core| core.friends.iter().find(|f| f.key == key))
        .is_some_and(|f| f.shared.iter().any(|uuid| uuid == instance))
}

/// Whether anything is shared with the friend `key` on this core, as the core said last.
pub fn shares_anything(core: Option<&CoreFriends>, key: &str) -> bool {
    core.and_then(|core| core.friends.iter().find(|f| f.key == key))
        .is_some_and(|f| !f.shared.is_empty())
}

/// Whether the friend `key` is connected to the core now.
pub fn is_connected(core: Option<&CoreFriends>, key: &str) -> bool {
    core.and_then(|core| core.friends.iter().find(|f| f.key == key))
        .is_some_and(|f| f.connected)
}

/// Where sharing with friends stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Reading the friend list and the core's.
    Opening,
    /// No friend file, no name or no active friends: "Add friends in Lodestone
    /// Launcher".
    NeedsLauncher,
    /// The friend identity was retired: the core was emptied, and nothing is shared
    /// until the person starts again in the Launcher.
    Retired,
    /// A test copy whose core and friend list don't belong together
    /// ([`sharing_allowed`]): sharing is off.
    Off,
    /// The friends are listed, with what's shared with each.
    Ready,
    /// The friend list couldn't be opened ([`FriendSharing::error`] says why). Tried
    /// again on Try again, or when the file changes.
    Failed,
}

/// What a background run does.
#[derive(Clone, Debug)]
enum Job {
    /// Reads the friend list and syncs the core when it changed since `last` (always
    /// when `force`).
    Check { last: Option<SyncView>, force: bool },
    /// Shares `instance` with the friend `key`, or stops.
    Share {
        key: String,
        instance: String,
        share: bool,
    },
    /// Gets the core a new friend certificate and syncs, retiring its current one
    /// first when `retire_current` (from the Host; the Launcher may have retired it
    /// already).
    Renew { retire_current: bool },
}

/// Where giving the core a new friend certificate stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renew {
    #[default]
    Idle,
    /// "Retire this core's certificate" was pressed: waiting for the person to confirm.
    Confirming,
    Working,
    /// The core is too old to make one: the Settings say how to by hand.
    Unsupported,
}

/// What a background run found.
#[derive(Default)]
struct Outcome {
    rows: Vec<FriendRow>,
    needs_launcher: bool,
    /// The core's list, or why it couldn't be read or changed.
    core: Option<Result<CoreFriends, String>>,
    /// The friend list as synced, when a sync worked.
    view: Option<SyncView>,
    /// Nothing changed since the last sync: nothing was done.
    unchanged: bool,
    /// Why some friends couldn't be told about the core.
    problems: Vec<(String, String)>,
    /// Why the friend file couldn't be read again.
    reload_error: Option<String>,
    /// The file is gone or needs other keys: drop the list and open it again.
    lost: bool,
    /// The identity was retired (and the core emptied, unless that failed: `core`).
    retired: bool,
    /// The core's friend certificate is retired: it needs a new one to share.
    core_retired: bool,
    /// The core can't make a new certificate (an older core).
    no_certificate_route: bool,
    /// After a share: the friend's players put on the server's whitelist.
    whitelisted: Vec<String>,
    /// After a share: why the whitelist couldn't be changed (the friend's name, why).
    whitelist_failed: Option<(String, String)>,
}

/// Sharing with friends, for the Host's own core while signed in as its owner. Pages
/// observe it.
pub struct FriendSharing {
    state: WeakEntity<HostState>,
    data: Option<DataDir>,
    service: Service,
    /// Which core this is, for the person's state ([`core_source`]).
    source: String,
    friends: Option<Arc<Mutex<Friends>>>,
    pub phase: Phase,
    pub rows: Vec<FriendRow>,
    /// The core's list, as it was last read or set.
    pub core: Option<CoreFriends>,
    /// The last failure, in plain words.
    pub error: Option<String>,
    /// The core has the friend list as it is now: the last sync worked and every
    /// friend has this person's state. Removals reach the core only this way, so the
    /// Settings say so.
    pub synced: bool,
    /// Ticks being saved, by (friend, instance), with the value shown meanwhile.
    pub pending: HashMap<(String, String), bool>,
    /// A first share with a friend whose safety code wasn't compared, waiting for the
    /// person to confirm: (friend, instance).
    pub confirm: Option<(String, String)>,
    /// This core's friend certificate was retired (here or in the Launcher): friends
    /// don't see it until it has a new one.
    pub certificate_retired: bool,
    /// Giving the core a new friend certificate.
    pub renew: Renew,
    in_flight: usize,
    /// The friend list as last synced: a sync runs again when it changes.
    synced_view: Option<SyncView>,
    /// The last sync failed: try again at the next check.
    sync_failed: bool,
    /// The friend file's stamp ([`friend_list::file_stamp`]) when it was last opened,
    /// or seen missing: opened again only when it changes (or on Try again).
    opened_stamp: Option<Option<[u8; 32]>>,
    /// The core's name at the last sync, to sync again when it's renamed.
    synced_name: Option<String>,
    /// Sample data for the screenshots: nothing runs.
    demo: bool,
    _tasks: Vec<Task<()>>,
}

impl FriendSharing {
    pub fn new(state: WeakEntity<HostState>, cx: &mut Context<Self>) -> Self {
        let core_data = core_process::data_dir();
        let allowed = sharing_allowed(
            &core_data,
            dirs::home_dir().as_deref(),
            launcher_overridden(),
        );
        let mut this = Self {
            state,
            data: DataDir::locate(),
            service: friend_list::service(),
            source: core_source(&core_data),
            friends: None,
            phase: if allowed { Phase::Opening } else { Phase::Off },
            rows: Vec::new(),
            core: None,
            error: None,
            synced: false,
            pending: HashMap::new(),
            confirm: None,
            certificate_retired: false,
            renew: Renew::Idle,
            in_flight: 0,
            synced_view: None,
            sync_failed: false,
            opened_stamp: None,
            synced_name: None,
            demo: false,
            _tasks: Vec::new(),
        };
        if !allowed {
            return this;
        }
        this.check(cx);
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CHECK_EVERY).await;
                if this.update(cx, |this, cx| this.check(cx)).is_err() {
                    break;
                }
            }
        });
        this._tasks.push(poll);
        this
    }

    fn file_stamp(&self) -> Option<[u8; 32]> {
        friend_list::file_stamp(self.data.as_ref()?)
    }

    fn core_name(&self, cx: &Context<Self>) -> Option<String> {
        Some(self.state.upgrade()?.read(cx).info.core_name.clone())
    }

    /// Opens the list when it's there and changed since the last try, or syncs when the
    /// list, the core's name or a failed sync calls for it.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.in_flight > 0 || self.phase == Phase::Off {
            return;
        }
        if self.friends.is_some() {
            let renamed = self.phase == Phase::Ready && self.synced_name != self.core_name(cx);
            let job = Job::Check {
                last: self.synced_view.clone(),
                force: renamed || self.sync_failed,
            };
            self.run(job, cx);
            return;
        }
        let mut stamp = self.file_stamp();
        if stamp.is_none() {
            // the data folder may have moved (Settings › Storage): look again
            self.data = DataDir::locate();
            stamp = self.file_stamp();
        }
        if self.opened_stamp == Some(stamp) {
            // nothing changed since the last try: wait for a change or Try again
            return;
        }
        self.opened_stamp = Some(stamp);
        if stamp.is_none() {
            if self.phase != Phase::NeedsLauncher {
                self.phase = Phase::NeedsLauncher;
                self.error = None;
                cx.notify();
            }
            return;
        }
        self.open(cx);
    }

    /// Opens the friend list, without making anything (the Keychain is read on GPUI's
    /// thread: the store's futures aren't `Send`; the file off it), then syncs.
    fn open(&mut self, cx: &mut Context<Self>) {
        let Some(data) = self.data.clone() else {
            return;
        };
        let service = self.service.clone();
        if self.rows.is_empty() {
            self.phase = Phase::Opening;
        }
        self.in_flight += 1;
        cx.notify();
        let task = cx.spawn(async move |this, cx| {
            let store = KeychainStore::new(cx.clone());
            let keys = friend_list::read_keys(&data, &store).await;
            let opened = match keys {
                Ok(keys) => {
                    cx.background_spawn(async move { Friends::open_with(&data, &keys, service) })
                        .await
                }
                Err(err) => Err(err),
            };
            this.update(cx, |this, cx| {
                this.in_flight -= 1;
                match opened {
                    Ok(Opened::Ready(friends)) => {
                        this.error = None;
                        this.friends = Some(Arc::new(Mutex::new(*friends)));
                        this.synced_view = None;
                        this.run(
                            Job::Check {
                                last: None,
                                force: true,
                            },
                            cx,
                        );
                    }
                    Ok(Opened::None) => {
                        this.error = None;
                        this.phase = Phase::NeedsLauncher;
                    }
                    Ok(Opened::Retired) => this.clear_retired(cx),
                    Err(err) => {
                        this.phase = Phase::Failed;
                        this.synced = false;
                        this.error = Some(tr!(M::FriendsOpenFailed, error = err));
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// The friend identity was retired: empties the core's friend list (its
    /// friendships are over on both sides).
    fn clear_retired(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let admin = state.read(cx).admin.clone();
        self.friends = None;
        self.synced_view = None;
        self.rows.clear();
        self.phase = Phase::Retired;
        self.in_flight += 1;
        let task = cx.spawn(async move |this, cx| {
            let cleared = cx
                .background_spawn(async move { core_sync::clear(&admin).await })
                .await;
            this.update(cx, |this, cx| {
                this.in_flight -= 1;
                match cleared {
                    Ok(core) => {
                        this.core = Some(core);
                        this.error = None;
                    }
                    Err(err) => this.error = Some(tr!(M::FriendsSyncFailed, error = err)),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Shares `instance` with the friend `key`, or stops (a tick in the instance's
    /// Settings). The first share with a friend whose safety code wasn't compared waits
    /// for the person to confirm ([`Self::confirm`]).
    pub fn set_shared(&mut self, key: &str, instance: &str, share: bool, cx: &mut Context<Self>) {
        let unverified = self.rows.iter().any(|r| r.key == key && !r.verified);
        if share && unverified && !shares_anything(self.core.as_ref(), key) {
            self.confirm = Some((key.to_string(), instance.to_string()));
            cx.notify();
            return;
        }
        self.share_now(key, instance, share, cx);
    }

    /// "Share anyway" on the confirmation.
    pub fn confirm_share(&mut self, cx: &mut Context<Self>) {
        if let Some((key, instance)) = self.confirm.take() {
            self.share_now(&key, &instance, true, cx);
        }
        cx.notify();
    }

    /// "Cancel" on the confirmation.
    pub fn cancel_share(&mut self, cx: &mut Context<Self>) {
        self.confirm = None;
        cx.notify();
    }

    fn share_now(&mut self, key: &str, instance: &str, share: bool, cx: &mut Context<Self>) {
        if self.demo {
            if let Some(core) = &mut self.core {
                let list = shared_after(core, key, instance, share);
                if let Some(friend) = core.friends.iter_mut().find(|f| f.key == key) {
                    friend.shared = list;
                }
            }
            cx.notify();
            return;
        }
        self.pending
            .insert((key.to_string(), instance.to_string()), share);
        self.run(
            Job::Share {
                key: key.to_string(),
                instance: instance.to_string(),
                share,
            },
            cx,
        );
    }

    /// "Retire this core's certificate": asks to confirm first.
    pub fn ask_renew(&mut self, cx: &mut Context<Self>) {
        if self.renew == Renew::Idle {
            self.renew = Renew::Confirming;
        }
        cx.notify();
    }

    /// "Cancel" on that confirmation.
    pub fn cancel_renew(&mut self, cx: &mut Context<Self>) {
        if self.renew == Renew::Confirming {
            self.renew = Renew::Idle;
        }
        cx.notify();
    }

    /// Gives the core a new friend certificate, so sharing from this computer goes on:
    /// "Retire and get a new one" (retiring the current one first), or "Get a new
    /// certificate" once it was retired (in the Launcher, say). Friends drop the old
    /// one and find the core under the new one.
    pub fn renew_certificate(&mut self, cx: &mut Context<Self>) {
        if self.demo
            || self.friends.is_none()
            || matches!(self.renew, Renew::Working | Renew::Unsupported)
        {
            return;
        }
        let retire_current = !self.certificate_retired;
        self.renew = Renew::Working;
        self.error = None;
        self.run(Job::Renew { retire_current }, cx);
        cx.notify();
    }

    /// Where the core's friend certificate is, to delete by hand when the core can't
    /// make a new one.
    pub fn certificate_file(&self) -> std::path::PathBuf {
        core_process::data_dir()
            .join("stores")
            .join("connect")
            .join("friends")
            .join("core.json")
    }

    /// Opens or syncs again now: the Try again button.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.phase == Phase::Off {
            return;
        }
        self.error = None;
        if self.friends.is_none() {
            self.opened_stamp = None;
            self.phase = Phase::Opening;
            self.check(cx);
        } else {
            self.run(
                Job::Check {
                    last: None,
                    force: true,
                },
                cx,
            );
        }
        cx.notify();
    }

    /// Runs `job` in the background; runs queue on the friend list's lock.
    fn run(&mut self, job: Job, cx: &mut Context<Self>) {
        let Some(friends) = self.friends.clone() else {
            return;
        };
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let (admin, name) = {
            let state = state.read(cx);
            (state.admin.clone(), state.info.core_name.clone())
        };
        let source = self.source.clone();
        self.in_flight += 1;
        cx.notify();
        let pending = match &job {
            Job::Share { key, instance, .. } => Some((key.clone(), instance.clone())),
            Job::Check { .. } | Job::Renew { .. } => None,
        };
        let task = cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_spawn(run(friends, admin, name.clone(), source, job))
                .await;
            this.update(cx, |this, cx| {
                this.in_flight -= 1;
                if let Some(pending) = pending {
                    this.pending.remove(&pending);
                }
                this.finish(outcome, name, cx);
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn finish(&mut self, outcome: Outcome, name: String, cx: &mut Context<Self>) {
        if outcome.lost {
            // the file went (or needs other keys): never sync from the old copy. Look
            // for it again now.
            self.friends = None;
            self.synced_view = None;
            self.synced = false;
            self.opened_stamp = None;
            self.check(cx);
            return;
        }
        if outcome.retired {
            self.friends = None;
            self.synced_view = None;
            self.synced = false;
            self.rows.clear();
            self.phase = Phase::Retired;
            self.opened_stamp = Some(self.file_stamp());
            self.error = match outcome.core {
                Some(Err(why)) => Some(tr!(M::FriendsSyncFailed, error = why)),
                Some(Ok(core)) => {
                    self.core = Some(core);
                    None
                }
                None => None,
            };
            return;
        }
        self.rows = outcome.rows;
        self.phase = if outcome.needs_launcher {
            Phase::NeedsLauncher
        } else {
            Phase::Ready
        };
        if outcome.unchanged {
            return;
        }
        if self.renew == Renew::Working {
            self.renew = Renew::Idle;
        }
        if outcome.no_certificate_route {
            self.renew = Renew::Unsupported;
        }
        if outcome.core_retired {
            // not a failure to retry every minute: it waits for a new certificate
            self.certificate_retired = true;
            self.synced = false;
            self.sync_failed = false;
            self.error = None;
            return;
        }
        if matches!(outcome.core, Some(Ok(_))) {
            self.certificate_retired = false;
        }
        let mut error = outcome.reload_error;
        let mut synced = false;
        match outcome.core {
            Some(Ok(core)) => {
                self.core = Some(core);
                self.synced_name = Some(name);
                synced = true;
                if outcome.view.is_some() {
                    self.synced_view = outcome.view;
                }
            }
            Some(Err(why)) => error = Some(tr!(M::FriendsSyncFailed, error = why)),
            None => {}
        }
        if error.is_none()
            && let Some((key, why)) = outcome.problems.first()
        {
            let name = self.rows.iter().find(|row| &row.key == key);
            error = Some(match name {
                Some(row) => tr!(M::FriendsPublishFailed, name = row.name, error = why),
                None => tr!(M::FriendsPublishFailedAll, error = why),
            });
        }
        self.synced = synced && error.is_none();
        self.sync_failed = error.is_some();
        self.error = error;
        // who went on the server's whitelist with this share
        let toast = if !outcome.whitelisted.is_empty() {
            Some(lodestone_ui::components::Toast::success(tr!(
                M::FriendsWhitelisted,
                names = lodestone_ui::i18n::format::list(&outcome.whitelisted)
            )))
        } else {
            outcome.whitelist_failed.map(|(name, why)| {
                lodestone_ui::components::Toast::warning(tr!(
                    M::FriendsWhitelistFailed,
                    name = name,
                    error = why
                ))
            })
        };
        if let Some(toast) = toast {
            self.state
                .update(cx, |state, cx| state.toast(toast, cx))
                .ok();
        }
    }

    /// Whether the Host tries again by itself after the current error: after a failed
    /// sync, yes (every minute); after the list failed to open, only on Try again or
    /// when the file changes.
    pub fn retries_by_itself(&self) -> bool {
        self.phase != Phase::Failed
    }

    fn push(&mut self, task: Task<()>) {
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    /// Sample states for the development screenshots: `none`, `loading`, `off`,
    /// `retired`, `shared` (three friends, one not verified, one connected, `instance`
    /// shared with two), `confirm` (the same, asking before sharing with the friend
    /// not verified), `renew` (the same, confirming "Retire this core's certificate"),
    /// `certificate` (its certificate retired, a new one offered), `certificate-manual`
    /// (the same, from a core too old to make one) or `error` (the same, after a sync
    /// that failed).
    pub fn demo(&mut self, scene: &str, instance: &str, cx: &mut Context<Self>) {
        use core_sync::CoreFriend;
        self.demo = true;
        self.pending.clear();
        self.confirm = None;
        self.renew = Renew::Idle;
        self.certificate_retired = false;
        self.error = None;
        self.synced = false;
        let key = |c: char| c.to_string().repeat(64);
        let row = |c: char, name: &str, verified: bool, code: &str| FriendRow {
            key: key(c),
            name: name.into(),
            verified,
            safety_code: code.into(),
            players: vec![StatePlayer {
                name: format!("{name}_mc"),
                uuid: [c as u8; 16],
                offline: false,
            }],
        };
        let friend = |c: char, name: &str, shared: bool, connected: bool| CoreFriend {
            key: key(c),
            name: name.into(),
            shared: if shared {
                vec![instance.to_string()]
            } else {
                Vec::new()
            },
            connected,
            hidden: false,
        };
        match scene {
            "none" | "loading" | "off" | "retired" => {
                self.phase = match scene {
                    "none" => Phase::NeedsLauncher,
                    "loading" => Phase::Opening,
                    "off" => Phase::Off,
                    _ => Phase::Retired,
                };
                self.rows.clear();
                self.core = None;
            }
            _ => {
                self.phase = Phase::Ready;
                self.rows = vec![
                    row('a', "Alex", true, "K7Q2-M9XD-4TRW-H3BE"),
                    row('b', "Jordan", true, "P3VA-8NQE-2WZT-K5MC"),
                    row('c', "Sam", false, "D6HR-T1XB-9QJE-W4NA"),
                ];
                self.core = Some(CoreFriends {
                    fingerprint: "0".repeat(64),
                    version: 3,
                    friends: vec![
                        friend('a', "Alex", true, true),
                        friend('b', "Jordan", false, false),
                        friend('c', "Sam", false, false),
                    ],
                    host: None,
                });
                self.synced = scene != "error";
                if scene == "error" {
                    self.error = Some(tr!(
                        M::FriendsSyncFailed,
                        error = "Lodestone Core didn’t answer"
                    ));
                }
                match scene {
                    "confirm" => self.confirm = Some((key('c'), instance.to_string())),
                    "renew" => self.renew = Renew::Confirming,
                    "certificate" => self.certificate_retired = true,
                    "certificate-manual" => {
                        self.certificate_retired = true;
                        self.renew = Renew::Unsupported;
                    }
                    _ => {}
                }
                if scene.starts_with("certificate") {
                    self.synced = false;
                }
            }
        }
        cx.notify();
    }
}

/// A background run: holds the friend list's lock throughout, so runs never overlap.
async fn run(
    friends: Arc<Mutex<Friends>>,
    admin: AdminClient,
    name: String,
    source: String,
    job: Job,
) -> Outcome {
    let mut friends = friends.lock().await;
    let mut outcome = Outcome::default();
    match friends.reload() {
        Ok(()) => {}
        Err(FriendsError::Retired) => {
            outcome.retired = true;
            outcome.core = Some(core_sync::clear(&admin).await.map_err(|e| e.to_string()));
            return outcome;
        }
        Err(err) if err.is_stale() => {
            outcome.lost = true;
            return outcome;
        }
        Err(err) => {
            // keep the friends read last
            outcome.rows = listed(&friends.book);
            outcome.needs_launcher = needs_launcher(&friends.book);
            outcome.reload_error = Some(tr!(M::FriendsOpenFailed, error = err));
            return outcome;
        }
    }
    outcome.rows = listed(&friends.book);
    outcome.needs_launcher = needs_launcher(&friends.book);
    let view = SyncView::of(&friends.book);
    // a share with a friend: their players go on the server's whitelist after it
    let whitelist_for = match &job {
        Job::Share {
            key,
            instance,
            share: true,
        } => Some((key.clone(), instance.clone())),
        _ => None,
    };
    let synced = match job {
        Job::Check { last, force } => {
            if !force && last.as_ref() == Some(&view) {
                outcome.unchanged = true;
                return outcome;
            }
            // nothing to set up, unless the core still has friends from before (who
            // were removed since): then the sync drops them
            let current = match core_sync::read(&admin).await {
                Ok(current) => current,
                Err(err) => {
                    outcome.core = Some(Err(err.to_string()));
                    return outcome;
                }
            };
            if outcome.needs_launcher && current.friends.is_empty() {
                outcome.core = Some(Ok(current));
                outcome.view = Some(view);
                return outcome;
            }
            core_sync::sync(&mut friends, &admin, &name, Some(&source)).await
        }
        Job::Share {
            key,
            instance,
            share,
        } => match core_sync::read(&admin).await {
            Ok(current) => {
                let list = shared_after(&current, &key, &instance, share);
                core_sync::share(&mut friends, &admin, &name, Some(&source), &key, list).await
            }
            Err(err) => Err(err.into()),
        },
        Job::Renew { retire_current } => {
            match renew(&mut friends, &admin, retire_current).await {
                Ok(()) => core_sync::sync(&mut friends, &admin, &name, Some(&source)).await,
                Err(SyncError::NoCertificateRoute) => {
                    outcome.no_certificate_route = true;
                    outcome.core_retired = true;
                    return outcome;
                }
                Err(err) => {
                    // shown, and tried again with the next sync, which finds the
                    // certificate retired (if it was) and offers a new one again
                    outcome.core = Some(Err(err.to_string()));
                    return outcome;
                }
            }
        }
    };
    match synced {
        Ok(core) => {
            outcome.core = Some(Ok(core));
            // what the core got: the book as the sync read it
            outcome.view = Some(SyncView::of(&friends.book));
            match friends.publish_states().await {
                Ok(problems) => outcome.problems = problems,
                Err(err) => outcome.problems = vec![(String::new(), err.to_string())],
            }
            if let Some((key, instance)) = whitelist_for
                && let Some(row) = listed(&friends.book).into_iter().find(|r| r.key == key)
            {
                match crate::whitelist::add(&admin, &instance, &row.players).await {
                    Ok(added) => outcome.whitelisted = added,
                    Err(err) => outcome.whitelist_failed = Some((row.name, err.to_string())),
                }
            }
        }
        Err(SyncError::Friends(FriendsError::Retired)) => {
            outcome.retired = true;
            outcome.core = Some(core_sync::clear(&admin).await.map_err(|e| e.to_string()));
            return outcome;
        }
        Err(err) if err.is_core_retired() => {
            // retired (in the Launcher, say): offered a new certificate
            outcome.core_retired = true;
            return outcome;
        }
        Err(SyncError::Friends(err)) if err.is_stale() => {
            outcome.lost = true;
            return outcome;
        }
        Err(err) => outcome.core = Some(Err(err.to_string())),
    }
    // the sync may have changed the book (this core in the person's state)
    outcome.rows = listed(&friends.book);
    outcome.needs_launcher = needs_launcher(&friends.book);
    outcome
}

/// Retires the core's current friend certificate (when `retire_current`, and it isn't
/// already) and tells friends, then has the core make a new one. The sync that follows
/// lists the new one.
async fn renew(
    friends: &mut Friends,
    admin: &AdminClient,
    retire_current: bool,
) -> Result<(), SyncError> {
    if retire_current {
        let current = core_sync::read(admin).await?;
        if !friends.book.retired_cores.contains(&current.fingerprint) {
            friends.retire_my_core(&current.fingerprint)?;
        }
        // friends drop the old certificate whatever happens next (a problem reaching
        // one is retried with later syncs)
        let _ = friends.publish_states().await;
    }
    core_sync::new_certificate(admin).await.map(|_| ())
}

#[cfg(test)]
mod tests;
