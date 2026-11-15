//! Instance images in the Launcher (`docs/instance-images.md`): content images of each
//! instance, recorded as the Launcher changes its mods, packs and configs and before the
//! game launches, and its snapshots (data and full images), kept in one image store for
//! every instance on the machine.
//!
//! The store is `<data>/image-store/` ([`crate::paths::DataDir::image_store_dir`]): plain
//! files (the Launcher has no zstd; APFS clones them where it can), `objects/`, `lists/`,
//! `chunks/` and `images/<instance>/`. Its only collector is its own GC
//! ([`Images::request_gc`]), which marks every manifest: content images, snapshots and the
//! content they link to, and deleted instances' snapshots ([`retired`]).
//!
//! - [`recorder`]: operations ([`Images::begin`]), merging what succeeds within a few
//!   seconds into one image, and recording a content image.
//! - [`check`]: the quick check before the game launches ("Changed by hand"), and taking
//!   the baseline again when a game session ends.
//! - [`scope`]: which files are content. [`context`]: an instance's game, base, settings
//!   and the sources of its files, from its own files.
//! - [`state`]: the current content image and the baseline. [`labels`]: what each action
//!   calls its image.
//! - [`retention`]: which manifests to keep. [`history`]: listing an instance's images.
//! - [`snapshots`]: taking snapshots and their records; [`restore`] and [`swap`]: going
//!   back to one; [`export`]: sharing one as a file; [`retired`]: deleted instances'
//!   snapshots.
//!
//! Everything here blocks (it reads and hashes files) except [`Images::begin`] and
//! [`recorder::Operation`]'s success, which only queue a label: call the rest on a
//! background thread, never the window's. Merged records run on one thread of their own
//! ("lodestone-images"). Instance ids are the Launcher's folder names; an id the store
//! can't name (a Windows device name such as `con`) records nothing.
//!
//! Locks, outermost first: an instance's record lock ([`Slot::record`]: records, the
//! state file, pruning and deleting), then its queue ([`Slot::pending`]). The store needs
//! none: its writers claim what they rely on, and a collection holds the store's own
//! collection lock.
//!
//! Every write to the store (a record, the check, a session's end, deleting an instance's
//! images, a collection) runs inside [`Activity`]'s count, so [`Images::suspend`] can stop
//! them all and wait for the last to end: a data move copies or renames the store, and a
//! write during it would leave a manifest naming objects the new place lacks, or remake the
//! old folder.

pub mod check;
pub mod context;
pub mod export;
pub mod history;
pub mod labels;
pub mod recorder;
pub mod restore;
pub mod retention;
pub mod retired;
pub mod scope;
pub mod snapshots;
pub mod state;
pub mod swap;

#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::snapshot::SnapshotError;
use lodestone_image::store::{Encoding, Store, StoreError, valid_instance_id};

use crate::instances::InstanceStore;
use crate::paths::DataDir;

pub use check::Checked;
pub use history::{History, HistoryEntry};
/// For the app, which shows labels and names from images and passes no way to cancel.
pub use lodestone_image::display_text;
pub use lodestone_image::store::{Cancel, NEVER};
pub use recorder::{Operation, Recorded};
pub use restore::{ContentDiff, RestorePreview, RestoreScope, Restored, WorldChange};
pub use retention::{DEFAULT_POLICY, KEEP_CONTENT, Keep, SnapshotPolicy};
pub use retired::{CleanedUp, Retired, RetiredMarker};
pub use snapshots::{RecordWorld, RiskyChange, SnapshotKind, SnapshotRecord, Snapshots, Take};

/// Successes this close together are one image.
pub const MERGE_WINDOW: Duration = Duration::from_secs(3);
/// Queued labels are recorded at the latest this long after the first, however busy.
pub const MAX_DEFER: Duration = Duration::from_secs(30);
/// The least time between two collections that pruning asks for: past 30 images nearly
/// every record prunes one, which frees a few small files, while a collection reads every
/// manifest of every instance.
pub const PRUNE_GC_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How the merge window runs: [`MERGE_WINDOW`] and [`MAX_DEFER`], and how long the check
/// before a launch waits for a record ([`check::GATE_WAIT`]), or shorter in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub merge: Duration,
    pub max_defer: Duration,
    pub gate: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            merge: MERGE_WINDOW,
            max_defer: MAX_DEFER,
            gate: check::GATE_WAIT,
        }
    }
}

/// Why recording or reading an instance's images failed.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("there is no instance {0}")]
    Gone(String),
    #[error("{0}")]
    Instance(String),
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },
    #[error("another record of this instance is still running")]
    Busy,
    #[error("the image store is paused while the data folder moves")]
    Suspended,
    #[error("the game is running; close it first")]
    Running,
    #[error("there is no snapshot {0}")]
    NoSnapshot(String),
    #[error("cancelled")]
    Cancelled,
    /// The disk lacks room: `needed` bytes asked for, `free` there (an import's check).
    #[error("It needs {} MB and the disk has {} MB free", .needed.div_ceil(1_000_000), .free / 1_000_000)]
    NoRoom { needed: u64, free: u64 },
    #[error("{0}")]
    Invalid(String),
}

impl ImageError {
    pub(crate) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> ImageError {
        let what = what.into();
        move |source| ImageError::Io { what, source }
    }
}

/// What the recorder keeps per instance.
#[derive(Default)]
pub(crate) struct Slot {
    /// Held across a record, a change of the state file, pruning and deleting the
    /// instance's images, so none of them sees another half done.
    pub(crate) record: Mutex<()>,
    /// Labels waiting to be recorded.
    pub(crate) pending: Mutex<recorder::Pending>,
    /// Goes up by one each time a new image of the instance is written; an operation
    /// notes it when it begins ([`recorder::Queued::generation`]).
    pub(crate) generation: AtomicU64,
    /// How many game sessions of the instance run: one from each check before a launch
    /// until that session ends. The game writes files of its own meanwhile, so nothing is
    /// recorded. A count, not a flag: Play pressed again just as the last run's end is
    /// handled begins a session before the old one is over, and the old end must not take
    /// the baseline again under the new game ([`check`]).
    pub(crate) session: AtomicUsize,
    /// The check before this session's launch couldn't run (the store was suspended, the
    /// record failed, or there was no image and no record under way), so the session's end
    /// runs it instead of taking the baseline again ([`check`]).
    pub(crate) unchecked: AtomicBool,
}

impl Slot {
    pub(crate) fn lock_record(&self) -> MutexGuard<'_, ()> {
        self.record.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn pending(&self) -> MutexGuard<'_, recorder::Pending> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether a game session of the instance runs.
    pub(crate) fn in_session(&self) -> bool {
        self.session.load(Ordering::SeqCst) > 0
    }

    /// One session fewer (never below none).
    pub(crate) fn session_over(&self) {
        let mut now = self.session.load(Ordering::SeqCst);
        while now > 0 {
            match self.session.compare_exchange_weak(
                now,
                now - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return,
                Err(actual) => now = actual,
            }
        }
    }
}

/// The merged records' thread: woken when a label is queued or a session ends.
#[derive(Default)]
pub(crate) struct Worker {
    started: AtomicBool,
    wake: Mutex<bool>,
    signal: Condvar,
}

/// How many writes to the store run, and whether new ones may start
/// ([`Images::suspend`]).
#[derive(Default)]
pub(crate) struct Activity {
    state: Mutex<ActivityState>,
    idle: Condvar,
}

#[derive(Default)]
struct ActivityState {
    suspended: bool,
    /// Suspended for the rest of the process ([`Suspension::keep`]): nothing resumes it.
    kept: bool,
    writers: usize,
}

impl Activity {
    fn lock(&self) -> MutexGuard<'_, ActivityState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Counts a write until the guard is dropped; `None` while suspended.
    pub(crate) fn enter(self: &Arc<Self>) -> Option<Writing> {
        let mut state = self.lock();
        if state.suspended {
            return None;
        }
        state.writers += 1;
        Some(Writing {
            activity: self.clone(),
        })
    }

    pub(crate) fn suspended(&self) -> bool {
        self.lock().suspended
    }
}

/// A write to the store in progress ([`Activity::enter`]).
pub(crate) struct Writing {
    activity: Arc<Activity>,
}

impl Drop for Writing {
    fn drop(&mut self) {
        let mut state = self.activity.lock();
        state.writers -= 1;
        if state.writers == 0 {
            self.activity.idle.notify_all();
        }
    }
}

/// Every write to the store stopped, until it is dropped ([`Images::suspend`]).
#[must_use = "the store resumes when this is dropped"]
pub struct Suspension {
    images: Option<Arc<Images>>,
}

impl Suspension {
    /// Keeps the store suspended for the rest of the process: after a data move, whose
    /// Launcher restarts to read the store at its new place. A write here would remake the
    /// old folder. Remembered: a later suspension that is dropped (another move, cancelled
    /// in a Launcher whose restart failed) doesn't resume it.
    pub fn keep(mut self) {
        if let Some(images) = self.images.take() {
            images.activity.lock().kept = true;
        }
    }
}

impl Drop for Suspension {
    fn drop(&mut self) {
        if let Some(images) = self.images.take() {
            images.resume();
        }
    }
}

/// Whether a collection runs, and whether another was asked for meanwhile.
#[derive(Default)]
pub(crate) struct GcState {
    running: AtomicBool,
    pending: AtomicBool,
    /// When pruning last asked for a collection.
    last_for_pruning: Mutex<Option<Instant>>,
    /// How many collections were asked for, for tests of who asks.
    pub(crate) requests: AtomicU64,
}

/// The Launcher's images: see the module.
pub struct Images {
    /// Itself, for the collection's thread.
    this: Weak<Images>,
    store: Arc<Store>,
    data_root: PathBuf,
    instances: InstanceStore,
    slots: Mutex<HashMap<String, Arc<Slot>>>,
    worker: Arc<Worker>,
    timing: Timing,
    /// Images kept from pruning while a guard lives ([`Images::keep_while`]).
    pub(crate) keep: Mutex<HashMap<String, Vec<String>>>,
    gc: Arc<GcState>,
    /// Ids already logged as ones the store can't name.
    refused: Mutex<std::collections::HashSet<String>>,
    /// The writes running, and whether the store is suspended.
    activity: Arc<Activity>,
    /// Whether the store's folder was made private ([`Images::enter`]).
    root_private: AtomicBool,
    /// Bytes the records under way may still write: each record's free-space check counts
    /// the others' ([`recorder`]'s `reserve_space`).
    pub(crate) reserved: Mutex<u64>,
    /// Who hears of each change to an instance's snapshot list ([`Images::watch_snapshots`]).
    snapshot_watchers: Mutex<Vec<futures::channel::mpsc::UnboundedSender<String>>>,
    /// The free space tests pretend the disk has.
    #[cfg(test)]
    pub(crate) fake_free: Mutex<Option<u64>>,
}

impl Drop for Images {
    /// Wakes the merged records' thread, which then finds the images gone and ends: it
    /// sleeps without a timeout while nothing is queued.
    fn drop(&mut self) {
        *self.worker.wake.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.worker.signal.notify_all();
    }
}

impl Images {
    /// The images of the Launcher whose data folder is `data`. Nothing is read or written
    /// until it is used.
    pub fn open(data: &DataDir) -> Arc<Images> {
        Self::with_timing(data, Timing::default())
    }

    /// [`Self::open`] with another merge window (tests).
    pub fn with_timing(data: &DataDir, timing: Timing) -> Arc<Images> {
        Arc::new_cyclic(|this| Images {
            this: this.clone(),
            store: Arc::new(Store::new(data.image_store_dir(), Encoding::Raw)),
            data_root: data.root().to_path_buf(),
            instances: InstanceStore::new(data.instances_dir()),
            slots: Mutex::default(),
            worker: Arc::default(),
            timing,
            keep: Mutex::default(),
            gc: Arc::default(),
            refused: Mutex::default(),
            activity: Arc::default(),
            root_private: AtomicBool::new(false),
            reserved: Mutex::default(),
            snapshot_watchers: Mutex::default(),
            #[cfg(test)]
            fake_free: Mutex::default(),
        })
    }

    /// The ids of the instances whose snapshot list changes from now on, one per change:
    /// a snapshot taken (by hand, or one the Launcher takes on its own: after playing,
    /// before a change), pinned, renamed or deleted, retention's deletions, an import. A
    /// window showing a list reads it again. Stops when the receiver is dropped.
    pub fn watch_snapshots(&self) -> futures::channel::mpsc::UnboundedReceiver<String> {
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        self.snapshot_watchers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(sender);
        receiver
    }

    /// Tells the watchers that `instance`'s snapshot list changed.
    pub(crate) fn snapshots_changed(&self, instance: &str) {
        self.snapshot_watchers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|watcher| watcher.unbounded_send(instance.to_string()).is_ok());
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    pub(crate) fn instances(&self) -> &InstanceStore {
        &self.instances
    }

    /// Whether images are kept for the instance `id`: one the store can name. Logged once
    /// per id when not.
    pub fn accepts(&self, id: &str) -> bool {
        if valid_instance_id(id) {
            return true;
        }
        let first = self
            .refused
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string());
        if first {
            warn(
                "images.refused",
                id,
                "the image store can't name this instance",
            );
        }
        false
    }

    pub(crate) fn slot(&self, id: &str) -> Arc<Slot> {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_default()
            .clone()
    }

    /// The slots that exist now, by instance.
    pub(crate) fn slots(&self) -> Vec<(String, Arc<Slot>)> {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(id, slot)| (id.clone(), slot.clone()))
            .collect()
    }

    /// Counts a write to the store until the guard is dropped, or refuses while the store
    /// is suspended. Callers holding a record lock take it after that lock.
    pub(crate) fn enter(&self) -> Result<Writing, ImageError> {
        let writing = self.activity.enter().ok_or(ImageError::Suspended)?;
        if !self.root_private.swap(true, Ordering::SeqCst) {
            self.make_root_private();
        }
        Ok(writing)
    }

    /// Makes the store's folder readable by its owner only, before the first write. The
    /// Launcher writes its own files that way (they hold account names), and an image takes
    /// a mod's config whatever its mode, as an object anyone may read: where the data
    /// folder's parents are readable by others (a Linux home folder), the folder keeps them
    /// out. Only the store's own folder; what fails is only slower to keep private.
    fn make_root_private(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let root = self.store.root();
            let made = std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(root)
                .and_then(|()| {
                    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
                });
            if let Err(e) = made {
                warn("images.store", "", &e.to_string());
            }
        }
    }

    /// Stops every write to the store and waits for the running ones to end: a record (however
    /// long the first record of a big pack takes), a check, a collection. Until the guard is
    /// dropped, operations still queue their labels, but records, checks, deleting an
    /// instance's images and collections refuse ([`ImageError::Suspended`]) or wait, and the
    /// merged records' thread sleeps. For a data move, which copies or renames the store:
    /// drop the guard if the move fails or is cancelled, [`Suspension::keep`] it once the
    /// data is at its new place. Blocking.
    pub fn suspend(self: &Arc<Self>) -> Suspension {
        self.suspend_unless(&|| false)
            .expect("a suspension that can't be cancelled")
    }

    /// [`Self::suspend`], given up when `cancelled` turns true while it waits for a write
    /// (the data move's Cancel): then the store resumes and this returns `None`.
    pub fn suspend_unless(self: &Arc<Self>, cancelled: &dyn Fn() -> bool) -> Option<Suspension> {
        let mut state = self.activity.lock();
        state.suspended = true;
        while state.writers > 0 {
            if cancelled() {
                drop(state);
                self.resume();
                return None;
            }
            state = self
                .activity
                .idle
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        Some(Suspension {
            images: Some(self.clone()),
        })
    }

    /// Lets writes run again, with the collection and merged records that waited, unless
    /// a suspension was kept.
    fn resume(self: &Arc<Self>) {
        {
            let mut state = self.activity.lock();
            if state.kept {
                return;
            }
            state.suspended = false;
        }
        if self.gc.pending.load(Ordering::SeqCst) {
            self.request_gc();
        }
        self.wake_worker();
    }

    /// Whether [`Self::suspend`] stopped writes.
    pub fn suspended(&self) -> bool {
        self.activity.suspended()
    }

    /// Forgets a deleted instance: its content images, state and stat caches
    /// (`images/<id>/`), and anything queued for it; its snapshots are kept under a folder
    /// of their own until the user deletes them ([`retired`]). What nothing names any more
    /// goes at the collection this asks for. Call it once the instance's folder is gone
    /// (moved to the Trash): instance ids are reused, so a new instance of the same name
    /// must not inherit this history.
    /// Waits for a record of it that is running, however long (its first image of a big
    /// pack may take minutes; Delete's spinner waits that long, which is rare and safe). An
    /// instance folder deleted outside the Launcher is forgotten too, by
    /// [`Self::forget_deleted`].
    pub fn delete_instance(&self, id: &str) -> Result<(), ImageError> {
        if !self.accepts(id) {
            return Ok(());
        }
        let slot = self.slot(id);
        {
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            slot.pending().take();
            self.retire_locked(id)?;
        }
        self.request_gc();
        Ok(())
    }

    /// Forgets every instance whose folder is gone without the Launcher deleting it
    /// (deleted in Finder or Explorer, or by a sync tool): its `images/<id>/` would
    /// otherwise keep its objects from every collection. Asks for a collection when it
    /// forgot any, and returns how many. Run once when the Launcher starts, and before
    /// every collection. Blocking.
    pub fn forget_deleted(&self) -> Result<usize, ImageError> {
        let forgotten = {
            let _writing = self.enter()?;
            self.forget_deleted_held()
        };
        if forgotten > 0 {
            self.request_gc();
        }
        Ok(forgotten)
    }

    /// [`Self::forget_deleted`] without asking for a collection, inside a counted write.
    /// Nothing is forgotten while the instances folder itself is missing (a drive that
    /// isn't there). See [`instance_gone`] for when one instance counts as gone. A folder
    /// with a retirement marker is retired whether or not its instance is there (a crash
    /// between the marker and the rename); retired folders themselves are left alone.
    pub(crate) fn forget_deleted_held(&self) -> usize {
        if !self.instances.root().is_dir() {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(self.store.images_dir()) else {
            return 0;
        };
        let gone = |id: &str| instance_gone(&self.instances.dir(id));
        let mut forgotten = 0;
        for entry in entries.flatten() {
            let Some(id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !valid_instance_id(&id)
                || id.starts_with(retired::RETIRED_PREFIX)
                || !entry.file_type().is_ok_and(|t| t.is_dir())
                || !(gone(&id) || self.retired_marker_in(&id))
            {
                continue;
            }
            let slot = self.slot(&id);
            let _record = slot.lock_record();
            // Looked at again under the lock: an instance made meanwhile keeps its images.
            if !(gone(&id) || self.retired_marker_in(&id)) {
                continue;
            }
            slot.pending().take();
            match self.retire_locked(&id) {
                Ok(()) => {
                    warn(
                        "images.forgotten",
                        &id,
                        "the instance's folder is gone; its content images are deleted",
                    );
                    forgotten += 1;
                }
                Err(e) => warn("images.forgotten", &id, &e.to_string()),
            }
        }
        forgotten
    }

    /// Starts a collection in the background, or, while one runs, has it run once more
    /// when it ends (what was deleted meanwhile may have freed more). While the store is
    /// suspended, the collection waits for it to resume. Each collection first forgets the
    /// instances deleted outside the Launcher ([`Self::forget_deleted`]).
    pub fn request_gc(&self) {
        let state = self.gc.clone();
        state.requests.fetch_add(1, Ordering::SeqCst);
        state.pending.store(true, Ordering::SeqCst);
        if self.suspended() || state.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(images) = self.this.upgrade() else {
            state.running.store(false, Ordering::SeqCst);
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("lodestone-images-gc".into())
            .spawn(move || run_gc(&images));
        if spawned.is_err() {
            self.gc.running.store(false, Ordering::SeqCst);
        }
    }

    /// Whether a collection runs now.
    #[cfg(test)]
    pub(crate) fn gc_running(&self) -> bool {
        self.gc.running.load(Ordering::SeqCst)
    }

    /// [`Self::request_gc`] for pruning, at most every [`PRUNE_GC_INTERVAL`]: what a
    /// skipped request would have freed goes at the next collection.
    pub(crate) fn request_gc_after_pruning(&self) {
        let now = Instant::now();
        {
            let mut last = self
                .gc
                .last_for_pruning
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if last.is_some_and(|at| now.duration_since(at) < PRUNE_GC_INTERVAL) {
                return;
            }
            *last = Some(now);
        }
        self.request_gc();
    }

    /// How many collections were asked for.
    #[cfg(test)]
    pub(crate) fn gc_requests(&self) -> u64 {
        self.gc.requests.load(Ordering::SeqCst)
    }

    /// Wakes the merged records' thread, starting it the first time.
    pub(crate) fn wake_worker(self: &Arc<Self>) {
        *self.worker.wake.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.worker.signal.notify_all();
        if !self.worker.started.swap(true, Ordering::SeqCst) {
            let weak = Arc::downgrade(self);
            let worker = self.worker.clone();
            let spawned = std::thread::Builder::new()
                .name("lodestone-images".into())
                .spawn(move || recorder::run_worker(weak, worker));
            if spawned.is_err() {
                self.worker.started.store(false, Ordering::SeqCst);
            }
        }
    }

    pub(crate) fn timing(&self) -> Timing {
        self.timing
    }

    /// The free space on the data folder's disk, for the check before a snapshot.
    pub(crate) fn free_space(&self) -> Option<u64> {
        #[cfg(test)]
        if let Some(free) = *self.fake_free.lock().unwrap_or_else(|e| e.into_inner()) {
            return Some(free);
        }
        crate::disk::free_space(&self.data_root)
    }
}

/// Waits on the worker's signal until woken or `until`, whichever comes first; with no
/// `until`, until woken.
pub(crate) fn wait_for_wake(worker: &Worker, until: Option<Instant>) {
    let mut woken = worker.wake.lock().unwrap_or_else(|e| e.into_inner());
    while !*woken {
        let Some(until) = until else {
            woken = worker.signal.wait(woken).unwrap_or_else(|e| e.into_inner());
            continue;
        };
        let now = Instant::now();
        if now >= until {
            return;
        }
        woken = worker
            .signal
            .wait_timeout(woken, until - now)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    *woken = false;
}

/// Whether the instance whose folder is `dir` is gone, so its images may be forgotten:
/// nothing at all is at `dir`, or `dir` is a real folder without `instance.json`. A link
/// there (a pack kept on another drive, linked into the instances folder) whose target
/// can't be reached is no deletion: the drive may only be unplugged, and the instance comes
/// back with it. Nor is anything that can't be looked at.
fn instance_gone(dir: &Path) -> bool {
    let not_found = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    match std::fs::symlink_metadata(dir) {
        Err(e) => not_found(&e),
        // A symbolic link, or on Windows a junction (std reports both as links).
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => false,
        Ok(_) => matches!(
            std::fs::symlink_metadata(dir.join("instance.json")),
            Err(e) if not_found(&e)
        ),
    }
}

/// For the merged records' thread, which holds the images only while it works.
pub(crate) type WeakImages = Weak<Images>;

/// Collects until no request came in while the last collection ran. A request sets
/// `pending` before it looks at `running`, and this clears `running` before it looks at
/// `pending`, so a request is never lost between the two. Suspended, it stops and leaves
/// the request pending for [`Images::resume`].
fn run_gc(images: &Images) {
    let state = &images.gc;
    loop {
        let Some(writing) = images.activity.enter() else {
            state.pending.store(true, Ordering::SeqCst);
            state.running.store(false, Ordering::SeqCst);
            // Resumed in between, before it saw the request: collect after all, unless
            // another collection took over.
            if images.suspended() || state.running.swap(true, Ordering::SeqCst) {
                return;
            }
            continue;
        };
        state.pending.store(false, Ordering::SeqCst);
        images.forget_deleted_held();
        match images.store.gc(&GcOptions::default()) {
            Ok(report) if report.objects_removed + report.chunks_removed > 0 => {
                crate::trace::emit("images.gc", || {
                    serde_json::json!({
                        "removed": report.objects_removed + report.chunks_removed,
                        "bytes": report.bytes_removed,
                    })
                });
            }
            Ok(_) => {}
            Err(e) => warn("images.gc", "", &e.to_string()),
        }
        drop(writing);
        state.running.store(false, Ordering::SeqCst);
        if !state.pending.load(Ordering::SeqCst) || state.running.swap(true, Ordering::SeqCst) {
            return;
        }
    }
}

/// Notes a problem that doesn't stop anything (a record that failed is tried again; the
/// files are untouched) in the trace, where `lodestone-launcher-cli trace` shows it.
pub(crate) fn warn(topic: &str, instance: &str, message: &str) {
    crate::trace::emit(
        topic,
        || serde_json::json!({ "instance": instance, "warning": message }),
    );
}

/// Deletes a folder and everything in it, clearing read-only first where that stops a
/// delete (Windows; the store writes its files read-only). Missing is fine.
pub(crate) fn remove_tree(dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    fn writable(dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            // Never through a link or junction: only the store's own files.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                writable(&entry.path());
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.permissions().readonly() {
                let mut permissions = meta.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(entry.path(), permissions);
            }
        }
    }
    #[cfg(windows)]
    writable(dir);
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Replaces the small file at `path` (in an instance's image folder) crash-safely: written
/// to a temporary file named as the store names its own (`.incoming-<16 hex>.tmp`, so the
/// store's sweep deletes one a crash left), flushed, renamed over `path`, and the folder
/// flushed.
pub(crate) fn write_small_atomic(path: &Path, bytes: &[u8]) -> Result<(), ImageError> {
    use std::io::Write;
    let dir = path
        .parent()
        .ok_or_else(|| ImageError::Invalid(format!("{} has no folder", path.display())))?;
    std::fs::create_dir_all(dir).map_err(ImageError::io(format!(
        "Failed to create {}",
        dir.display()
    )))?;
    let temp = dir.join(format!(
        ".incoming-{:016x}.tmp",
        u64::from_le_bytes(crate::random_bytes())
    ));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        crate::fsx::rename(&temp, path)?;
        // The rename itself, durably. Windows can't open a folder this way (NTFS journals
        // the rename).
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(ImageError::io(format!(
            "Failed to write {}",
            path.display()
        ))(e));
    }
    Ok(())
}
