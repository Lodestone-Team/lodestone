//! Sharing this computer's servers with friends (launcher/docs/friends.md).
//!
//! The friend list is Lodestone Launcher's: `friend_list.json` in the launcher's data
//! folder, sealed with a key from the Keychain ([`crate::keychain`]), so this person is
//! one friend whichever app they use. The Host adds friends nowhere; it only keeps its
//! own core in step with the list ([`core_sync`]): each friend who is active gets
//! their core secret for this core, the core keeps what's shared with each, and this
//! person's state lists the core so friends find it.
//!
//! - At the start (once the core is running and signed in as its owner), whenever the
//!   friend file changes (its modification time, checked every 60 s) and after the
//!   person ticks or unticks a friend, the Host syncs the core, then publishes the
//!   state to friends who don't have it yet.
//! - Without a friend file, or with no name or no active friends, it does nothing and
//!   the instance's Settings say "Add friends in Lodestone Launcher". It doesn't even
//!   read the Keychain then, so it never makes a friend identity the launcher didn't.
//! - **Nothing is shared by default.** A friend sees an instance only once it's ticked
//!   for them.
//!
//! Failures are kept in [`FriendSharing::error`], in plain words, beside the last list
//! that worked.

use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime},
};

use futures::lock::Mutex;
use gpui::{AppContext as _, Context, Task, WeakEntity};
use launcher_core::{
    cores::admin::AdminClient,
    friend_list::{
        Book, FILE_NAME, FriendStatus, Friends,
        core_sync::{self, CoreFriends},
    },
    paths::DataDir,
};
use lodestone_connect::Service;

use crate::{
    i18n::{InstanceMsg as M, tr},
    keychain::KeychainStore,
    state::HostState,
};

/// The environment variable naming another introduction service (a local one, for
/// development), as the core's.
pub const SERVICE_ENV: &str = "LODESTONE_CONNECT_SERVICE";

/// How often the friend file's modification time is checked.
pub const CHECK_EVERY: Duration = Duration::from_secs(60);

/// The service in `value` (the environment's), else connect.lodestone.cc.
pub fn service_from(value: Option<&str>) -> Service {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(text) => Service::parse(text).unwrap_or_else(|| {
            eprintln!("{SERVICE_ENV} isn't a service address ({text}); using the default");
            Service::default()
        }),
        None => Service::default(),
    }
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
}

/// The friends things can be shared with: active and not hidden (the core never gets
/// hidden friends, so sharing with them would do nothing), by name.
pub fn listed(book: &Book) -> Vec<FriendRow> {
    let mut rows: Vec<FriendRow> = book
        .friends
        .iter()
        .filter(|f| f.status == FriendStatus::Active && !f.hidden)
        .map(|f| FriendRow {
            key: f.key.clone(),
            name: f.display_name().to_string(),
            verified: f.verified,
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

/// Whether the friend `key` is connected to the core now.
pub fn is_connected(core: Option<&CoreFriends>, key: &str) -> bool {
    core.and_then(|core| core.friends.iter().find(|f| f.key == key))
        .is_some_and(|f| f.connected)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Where sharing with friends stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Reading the friend list and the core's.
    Opening,
    /// No friend file, no name or no active friends: "Add friends in Lodestone
    /// Launcher".
    NeedsLauncher,
    /// The friends are listed, with what's shared with each.
    Ready,
    /// The friend list couldn't be opened ([`FriendSharing::error`] says why).
    Failed,
}

/// What a background run does.
#[derive(Clone, Debug)]
enum Job {
    /// Puts the current friends on the core, reading the friend file first when
    /// `reload`.
    Sync { reload: bool },
    /// Shares `instance` with the friend `key`, or stops.
    Share {
        key: String,
        instance: String,
        share: bool,
    },
}

/// What a background run found.
struct Outcome {
    rows: Vec<FriendRow>,
    needs_launcher: bool,
    /// The core's list, or why it couldn't be read or changed.
    core: Option<Result<CoreFriends, String>>,
    /// Why some friends couldn't be told about the core.
    problems: Vec<(String, String)>,
    /// Why the friend file couldn't be read again.
    reload_error: Option<String>,
}

/// Sharing with friends, for the Host's own core while signed in as its owner. Pages
/// observe it.
pub struct FriendSharing {
    state: WeakEntity<HostState>,
    data: Option<DataDir>,
    service: Service,
    friends: Option<Arc<Mutex<Friends>>>,
    pub phase: Phase,
    pub rows: Vec<FriendRow>,
    /// The core's list, as it was last read or set.
    pub core: Option<CoreFriends>,
    /// The last failure, in plain words. Syncing is tried again every minute while
    /// there's one.
    pub error: Option<String>,
    /// The core has the friend list as it is now: the last sync worked and every
    /// friend has this person's state. Removals reach the core only this way, so the
    /// Settings say so.
    pub synced: bool,
    /// Ticks being saved, by (friend, instance), with the value shown meanwhile.
    pub pending: HashMap<(String, String), bool>,
    in_flight: usize,
    /// The friend file's modification time when it was last read.
    file_modified: Option<SystemTime>,
    /// The core's name at the last sync, to sync again when it's renamed.
    synced_name: Option<String>,
    /// Sample data for the screenshots: nothing runs.
    demo: bool,
    _tasks: Vec<Task<()>>,
}

impl FriendSharing {
    pub fn new(state: WeakEntity<HostState>, cx: &mut Context<Self>) -> Self {
        let service = service_from(std::env::var(SERVICE_ENV).ok().as_deref());
        let mut this = Self {
            state,
            data: DataDir::locate(),
            service,
            friends: None,
            phase: Phase::Opening,
            rows: Vec::new(),
            core: None,
            error: None,
            synced: false,
            pending: HashMap::new(),
            in_flight: 0,
            file_modified: None,
            synced_name: None,
            demo: false,
            _tasks: Vec::new(),
        };
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

    fn file(&self) -> Option<std::path::PathBuf> {
        Some(self.data.as_ref()?.root().join(FILE_NAME))
    }

    fn core_name(&self, cx: &Context<Self>) -> Option<String> {
        Some(self.state.upgrade()?.read(cx).info.core_name.clone())
    }

    /// Opens or syncs when something changed: the friend file, the core's name, or a
    /// last attempt that failed.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.in_flight > 0 {
            return;
        }
        let Some(file) = self.file() else {
            // no home folder: nowhere the launcher could keep friends
            self.phase = Phase::NeedsLauncher;
            cx.notify();
            return;
        };
        let modified = modified(&file);
        let changed = modified != self.file_modified;
        if self.friends.is_none() {
            if modified.is_none() {
                self.file_modified = None;
                if self.phase != Phase::NeedsLauncher {
                    self.phase = Phase::NeedsLauncher;
                    cx.notify();
                }
                return;
            }
            if changed || self.phase == Phase::Failed {
                self.file_modified = modified;
                self.open(cx);
            }
            return;
        }
        let renamed = self.phase == Phase::Ready && self.synced_name != self.core_name(cx);
        if changed || renamed || self.error.is_some() {
            self.file_modified = modified;
            // after a failure, read the file again too: it may be what failed
            let reload = changed || self.error.is_some();
            self.run(Job::Sync { reload }, cx);
        }
    }

    /// Opens the friend list (the Keychain is read on GPUI's thread: the store's
    /// futures aren't `Send`), then syncs.
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
            let opened = Friends::open(&data, &store, service).await;
            this.update(cx, |this, cx| {
                this.in_flight -= 1;
                match opened {
                    Ok(friends) => {
                        this.friends = Some(Arc::new(Mutex::new(friends)));
                        this.run(Job::Sync { reload: false }, cx);
                    }
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

    /// Shares `instance` with the friend `key`, or stops (a tick in the instance's
    /// Settings).
    pub fn set_shared(&mut self, key: &str, instance: &str, share: bool, cx: &mut Context<Self>) {
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

    /// Syncs again now: the Try again button.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        self.error = None;
        if self.friends.is_none() {
            self.file_modified = None;
            self.phase = Phase::Opening;
            self.check(cx);
        } else {
            self.run(Job::Sync { reload: true }, cx);
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
        self.in_flight += 1;
        cx.notify();
        let pending = match &job {
            Job::Share { key, instance, .. } => Some((key.clone(), instance.clone())),
            Job::Sync { .. } => None,
        };
        let task = cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_spawn(run(friends, admin, name.clone(), job))
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

    fn finish(&mut self, outcome: Outcome, name: String, _cx: &mut Context<Self>) {
        self.rows = outcome.rows;
        self.phase = if outcome.needs_launcher {
            Phase::NeedsLauncher
        } else {
            Phase::Ready
        };
        let mut error = outcome.reload_error;
        let mut synced = false;
        match outcome.core {
            Some(Ok(core)) => {
                self.core = Some(core);
                self.synced_name = Some(name);
                synced = true;
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
        self.error = error;
    }

    fn push(&mut self, task: Task<()>) {
        self._tasks.push(task);
        self._tasks.retain(|task| !task.is_ready());
    }

    /// Sample states for the development screenshots: `none`, `loading`, `shared`
    /// (three friends, one not verified, one connected, `instance` shared with two)
    /// or `error` (the same, after a sync that failed).
    pub fn demo(&mut self, scene: &str, instance: &str, cx: &mut Context<Self>) {
        use core_sync::CoreFriend;
        self.demo = true;
        self.pending.clear();
        self.error = None;
        self.synced = false;
        let key = |c: char| c.to_string().repeat(64);
        let row = |c: char, name: &str, verified: bool| FriendRow {
            key: key(c),
            name: name.into(),
            verified,
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
        };
        match scene {
            "none" => {
                self.phase = Phase::NeedsLauncher;
                self.rows.clear();
                self.core = None;
            }
            "loading" => {
                self.phase = Phase::Opening;
                self.rows.clear();
                self.core = None;
            }
            _ => {
                self.phase = Phase::Ready;
                self.rows = vec![
                    row('a', "Alex", true),
                    row('b', "Jordan", true),
                    row('c', "Sam", false),
                ];
                self.core = Some(CoreFriends {
                    fingerprint: "0".repeat(64),
                    version: 3,
                    friends: vec![
                        friend('a', "Alex", true, true),
                        friend('b', "Jordan", false, false),
                        friend('c', "Sam", true, false),
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
            }
        }
        cx.notify();
    }
}

/// A background run: holds the friend list's lock throughout, so runs never overlap.
async fn run(friends: Arc<Mutex<Friends>>, admin: AdminClient, name: String, job: Job) -> Outcome {
    let mut friends = friends.lock().await;
    let mut outcome = Outcome {
        rows: Vec::new(),
        needs_launcher: false,
        core: None,
        problems: Vec::new(),
        reload_error: None,
    };
    if matches!(job, Job::Sync { reload: true })
        && let Err(err) = friends.reload()
    {
        // keep the friends read last
        outcome.reload_error = Some(tr!(M::FriendsOpenFailed, error = err));
    }
    outcome.rows = listed(&friends.book);
    outcome.needs_launcher = needs_launcher(&friends.book);
    if outcome.reload_error.is_some() {
        return outcome;
    }
    let synced = match job {
        Job::Sync { .. } => {
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
                return outcome;
            }
            core_sync::sync(&mut friends, &admin, &name).await
        }
        Job::Share {
            key,
            instance,
            share,
        } => match core_sync::read(&admin).await {
            Ok(current) => {
                let list = shared_after(&current, &key, &instance, share);
                core_sync::share(&mut friends, &admin, &name, &key, list).await
            }
            Err(err) => Err(err.into()),
        },
    };
    match synced {
        Ok(core) => {
            outcome.core = Some(Ok(core));
            match friends.publish_states().await {
                Ok(problems) => outcome.problems = problems,
                Err(err) => outcome.problems = vec![(String::new(), err.to_string())],
            }
        }
        Err(err) => outcome.core = Some(Err(err.to_string())),
    }
    // the sync may have changed the book (this core in the person's state)
    outcome.rows = listed(&friends.book);
    outcome.needs_launcher = needs_launcher(&friends.book);
    outcome
}

#[cfg(test)]
mod tests;
