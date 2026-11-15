//! Instance images in the Launcher (`docs/instance-images.md`): content images of each
//! instance, recorded as the Launcher changes its mods, packs and configs and before the
//! game launches, kept in one image store for every instance on the machine.
//!
//! The store is `<data>/image-store/` ([`crate::paths::DataDir::image_store_dir`]): plain
//! files (the Launcher has no zstd; APFS clones them where it can), `objects/`, `lists/`,
//! `chunks/` and `images/<instance>/`. It sits beside the snapshot store
//! (`<data>/snapshots/`, [`crate::snapshots`]), never inside it: each has its own objects
//! and its own collector, and neither collector reads the other's folder, so neither can
//! delete what the other names. The image store's only collector is its own GC
//! ([`Images::request_gc`]).
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

pub mod check;
pub mod context;
pub mod history;
pub mod labels;
pub mod recorder;
pub mod retention;
pub mod scope;
pub mod state;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::snapshot::SnapshotError;
use lodestone_image::store::{Encoding, Store, StoreError, valid_instance_id};

use crate::instances::InstanceStore;
use crate::paths::DataDir;

pub use check::Checked;
pub use history::{History, HistoryEntry};
pub use recorder::{Operation, Recorded};
pub use retention::{KEEP_CONTENT, Keep};

/// Successes this close together are one image.
pub const MERGE_WINDOW: Duration = Duration::from_secs(3);
/// Queued labels are recorded at the latest this long after the first, however busy.
pub const MAX_DEFER: Duration = Duration::from_secs(30);
/// The least time between two collections that pruning asks for: past 30 images nearly
/// every record prunes one, which frees a few small files, while a collection reads every
/// manifest of every instance.
pub const PRUNE_GC_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How the merge window runs: [`MERGE_WINDOW`] and [`MAX_DEFER`], or shorter in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub merge: Duration,
    pub max_defer: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            merge: MERGE_WINDOW,
            max_defer: MAX_DEFER,
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
    /// Set from the check before a launch until the session ends: the game writes files
    /// of its own meanwhile, so nothing is recorded.
    pub(crate) session: AtomicBool,
}

impl Slot {
    pub(crate) fn lock_record(&self) -> MutexGuard<'_, ()> {
        self.record.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn pending(&self) -> MutexGuard<'_, recorder::Pending> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The merged records' thread: woken when a label is queued or a session ends.
#[derive(Default)]
pub(crate) struct Worker {
    started: AtomicBool,
    wake: Mutex<bool>,
    signal: Condvar,
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
}

impl Images {
    /// The images of the Launcher whose data folder is `data`. Nothing is read or written
    /// until it is used.
    pub fn open(data: &DataDir) -> Arc<Images> {
        Self::with_timing(data, Timing::default())
    }

    /// [`Self::open`] with another merge window (tests).
    pub fn with_timing(data: &DataDir, timing: Timing) -> Arc<Images> {
        Arc::new(Images {
            store: Arc::new(Store::new(data.image_store_dir(), Encoding::Raw)),
            data_root: data.root().to_path_buf(),
            instances: InstanceStore::new(data.instances_dir()),
            slots: Mutex::default(),
            worker: Arc::default(),
            timing,
            keep: Mutex::default(),
            gc: Arc::default(),
            refused: Mutex::default(),
        })
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

    /// Forgets a deleted instance: its manifests, content state and stat cache
    /// (`images/<id>/`), and anything queued for it. Its objects go at the collection this
    /// asks for. Call it once the instance's folder is gone (moved to the Trash): instance
    /// ids are reused, so a new instance of the same name must not inherit this history.
    /// Waits for a record of it that is running.
    pub fn delete_instance(&self, id: &str) -> Result<(), ImageError> {
        if !self.accepts(id) {
            return Ok(());
        }
        let slot = self.slot(id);
        {
            let _record = slot.lock_record();
            slot.pending().take();
            self.wipe_locked(id)?;
        }
        self.request_gc();
        Ok(())
    }

    /// Deletes `images/<id>/`. The caller holds the instance's record lock.
    pub(crate) fn wipe_locked(&self, id: &str) -> Result<(), ImageError> {
        let dir = self.store.instance_dir(id)?;
        remove_tree(&dir).map_err(ImageError::io(format!(
            "Failed to delete {}",
            dir.display()
        )))
    }

    /// Starts a collection in the background, or, while one runs, has it run once more
    /// when it ends (what was deleted meanwhile may have freed more).
    pub fn request_gc(&self) {
        let state = self.gc.clone();
        state.requests.fetch_add(1, Ordering::SeqCst);
        state.pending.store(true, Ordering::SeqCst);
        if state.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let store = self.store.clone();
        let spawned = std::thread::Builder::new()
            .name("lodestone-images-gc".into())
            .spawn(move || run_gc(&store, &state));
        if spawned.is_err() {
            self.gc.running.store(false, Ordering::SeqCst);
        }
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
        crate::snapshots::free_space(&self.data_root)
    }
}

/// Waits on the worker's signal until woken or `until`, whichever comes first.
pub(crate) fn wait_for_wake(worker: &Worker, until: Instant) {
    let mut woken = worker.wake.lock().unwrap_or_else(|e| e.into_inner());
    while !*woken {
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

/// For the merged records' thread, which holds the images only while it works.
pub(crate) type WeakImages = Weak<Images>;

/// Collects until no request came in while the last collection ran. A request sets
/// `pending` before it looks at `running`, and this clears `running` before it looks at
/// `pending`, so a request is never lost between the two.
fn run_gc(store: &Store, state: &GcState) {
    loop {
        state.pending.store(false, Ordering::SeqCst);
        match store.gc(&GcOptions::default()) {
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
fn remove_tree(dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    fn writable(dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                writable(&entry.path());
            } else if meta.permissions().readonly() {
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
