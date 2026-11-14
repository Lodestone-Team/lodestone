//! Instance images in the core (`docs/instance-images.md`): the one image store, the
//! content images recorded as Lodestone changes a server's mods and configs, and the
//! store's collector.
//!
//! The service owns the store, in the backup folder (`<lodestone>/backups`: `objects/`,
//! `lists/`, `chunks/`, `images/<uuid>/`), zstd-encoded, and its collections ([`gc`]).
//!
//! - [`recorder`]: operations ([`ImageService::begin`]), merging what happens within a few
//!   seconds into one image, and recording a content image.
//! - [`check`]: the quick check before a server starts ("Changed outside Lodestone") and
//!   keeping its baseline when a server stops.
//! - [`scope`]: which files are content. [`excludes`]: a backup policy's patterns as an
//!   image's excludes. [`layout`]: a server's game, settings and disk layout.
//! - [`state`]: the current content image and the baseline. [`base`]: the modpack a
//!   server is built on. [`env`]: which sides a file is for.
//! - [`retention`]: which manifests to keep. [`gc`]: the collector.
//!
//! Locks, outermost first: an instance's mods lock (`ModsState::lock_for`), its content
//! gate (operations share it, a record has it alone), then the instance's lock
//! ([`ImageService::instance_lock`]: saving manifests, the pointer, pruning). The store
//! needs none of the core's: its writers claim what they rely on, and a collection holds
//! the store's own collection lock (see [`gc`]).
//!
//! Every blocking store call runs on `spawn_blocking`. Instance ids are the core's instance
//! uuids; Docker instances and ids the store can't name are ignored everywhere.

pub mod base;
pub mod check;
pub mod env;
pub mod excludes;
pub mod gc;
pub mod layout;
pub mod recorder;
pub mod retention;
pub mod scope;
pub mod state;

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock as StdRwLock};
use std::time::Instant;

use async_trait::async_trait;
use dashmap::DashMap;
use lodestone_image::store::{valid_instance_id, Encoding, Store};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use tracing::{info, warn};

use crate::backups::BackupService;
use crate::error::Error;
use crate::prelude::GameInstance;
use crate::types::InstanceUuid;

pub use env::ModrinthHint;
pub use layout::InstanceContext;
pub use recorder::{OpKind, Operation, Recorded, MAX_DEFER, MERGE_WINDOW};
pub use retention::{ImageRefs, KEEP_CONTENT};

/// A boxed future, for [`Clock`].
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The time the merge window runs on: the system's, or a test's.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()>;
}

/// The system's monotonic clock and tokio's timers.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        Box::pin(tokio::time::sleep_until(deadline.into()))
    }
}

/// Where the service finds a server's [`InstanceContext`] when a merged record is due.
#[async_trait]
pub trait InstanceSource: Send + Sync {
    /// `None` when there is no such server (any more).
    async fn context(&self, uuid: &InstanceUuid) -> Option<InstanceContext>;
}

/// The core's servers, with their backup policies' excludes.
pub struct AppInstances {
    pub instances: Arc<DashMap<InstanceUuid, GameInstance>>,
    pub backups: Arc<BackupService>,
}

#[async_trait]
impl InstanceSource for AppInstances {
    async fn context(&self, uuid: &InstanceUuid) -> Option<InstanceContext> {
        let instance = self.instances.get(uuid).map(|i| i.value().clone())?;
        let GameInstance::MinecraftInstance(instance) = instance;
        let exclude = match self.backups.policy(uuid).await {
            Ok(policy) => policy.exclude,
            Err(e) => {
                warn!("Failed to read the backup policy of {uuid}: {}", e.source);
                Vec::new()
            }
        };
        Some(InstanceContext::of(&instance, exclude).await)
    }
}

/// The [`InstanceContext`] of a server of `state`, for the HTTP handlers and hooks.
pub async fn context_of(state: &crate::AppState, uuid: &InstanceUuid) -> Option<InstanceContext> {
    AppInstances {
        instances: state.instances.clone(),
        backups: state.backups.clone(),
    }
    .context(uuid)
    .await
}

/// What the recorder keeps per instance.
#[derive(Default)]
pub(crate) struct Slot {
    /// Operations hold it shared while they change files; a record holds it alone, so it
    /// never reads a half-written file and an operation that starts meanwhile waits.
    /// tokio's lock is fair: a waiting record holds off new operations.
    pub(crate) gate: Arc<RwLock<()>>,
    /// Around saving manifests, moving the pointer and pruning.
    pub(crate) lock: Arc<Mutex<()>>,
    pub(crate) pending: StdMutex<recorder::Pending>,
}

/// Images in the core: see the module.
pub struct ImageService {
    store: Arc<Store>,
    root: PathBuf,
    slots: StdMutex<HashMap<InstanceUuid, Arc<Slot>>>,
    pub(crate) refs: StdRwLock<Vec<Arc<dyn ImageRefs>>>,
    hints: env::ModrinthHints,
    pub(crate) clock: Arc<dyn Clock>,
    source: StdRwLock<Option<Arc<dyn InstanceSource>>>,
    gc: Arc<gc::GcState>,
}

static GLOBAL: OnceLock<Arc<ImageService>> = OnceLock::new();

/// Makes `service` the one [`global`] returns. Once, at startup.
pub fn set_global(service: Arc<ImageService>) {
    if GLOBAL.set(service).is_err() {
        warn!("The image service was set up twice; keeping the first");
    }
}

/// The core's image service, for code that has no `AppState` (a server's start and stop).
/// `None` in unit tests, where those hooks then do nothing.
pub fn global() -> Option<Arc<ImageService>> {
    GLOBAL.get().cloned()
}

impl ImageService {
    /// The service for the store at `backups_root` (`<lodestone>/backups`). Nothing is read
    /// or written until it is used.
    pub fn new(backups_root: PathBuf) -> ImageService {
        Self::with_clock(backups_root, Arc::new(SystemClock))
    }

    /// [`Self::new`] with another clock for the merge window (tests).
    pub fn with_clock(backups_root: PathBuf, clock: Arc<dyn Clock>) -> ImageService {
        ImageService {
            store: Arc::new(Store::new(backups_root.clone(), Encoding::Zstd)),
            root: backups_root,
            slots: Default::default(),
            refs: Default::default(),
            hints: Default::default(),
            clock,
            source: Default::default(),
            gc: Default::default(),
        }
    }

    pub fn store(&self) -> Arc<Store> {
        self.store.clone()
    }

    /// The store's folder, `<lodestone>/backups`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Adds a source of the images an instance must keep ([`retention`]). At startup.
    pub fn register_refs(&self, refs: Arc<dyn ImageRefs>) {
        self.refs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push(refs);
    }

    /// Where merged records find their server ([`InstanceSource`]). At startup.
    pub fn set_instances(&self, source: Arc<dyn InstanceSource>) {
        *self.source.write().unwrap_or_else(|e| e.into_inner()) = Some(source);
    }

    pub(crate) async fn context(&self, uuid: &InstanceUuid) -> Option<InstanceContext> {
        let source = self
            .source
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        source.context(uuid).await
    }

    /// Notes what Modrinth said about a file (by its SHA-512), for the `env` and source of
    /// the next content image that has it. Call it with what was fetched anyway (installing
    /// or listing mods); recording never asks Modrinth itself.
    pub fn note_modrinth(&self, sha512: &str, hint: ModrinthHint) {
        self.hints.note(sha512, hint);
    }

    pub(crate) fn modrinth_hint(&self, sha512: &str) -> Option<ModrinthHint> {
        self.hints.get(sha512)
    }

    /// Whether images are kept for this instance: a valid store id, and not a Docker
    /// container.
    pub fn accepts(uuid: &InstanceUuid) -> bool {
        let id: &str = uuid.as_ref();
        valid_instance_id(id) && !id.starts_with("DOCKER-")
    }

    pub(crate) fn slot(&self, uuid: &InstanceUuid) -> Arc<Slot> {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_default()
            .clone()
    }

    /// The instance's lock: held around saving its manifests, moving its pointer and
    /// pruning. A writer that saves a manifest and then something naming it holds it
    /// across both.
    pub async fn instance_lock(&self, uuid: &InstanceUuid) -> OwnedMutexGuard<()> {
        self.slot(uuid).lock.clone().lock_owned().await
    }

    /// The instance's current content image, if it has one.
    pub async fn current_content(&self, uuid: &InstanceUuid) -> Option<String> {
        if !Self::accepts(uuid) {
            return None;
        }
        let root = self.root.clone();
        let uuid = uuid.clone();
        blocking(move || Ok(state::load(&root, &uuid).current))
            .await
            .ok()
            .flatten()
    }

    /// Forgets a deleted instance: its manifests, content state, pack and stat caches
    /// (`images/<uuid>/`). Its objects go at the next collection. Waits for a record that
    /// is running.
    pub async fn delete_instance(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        if !Self::accepts(uuid) {
            return Ok(());
        }
        let slot = self.slot(uuid);
        let _gate = slot.gate.clone().write_owned().await;
        let _lock = slot.lock.clone().lock_owned().await;
        slot.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let store = self.store();
        let id = uuid.clone();
        blocking(move || {
            let dir = store
                .instance_dir(id.as_ref())
                .map_err(|e| color_eyre::eyre::eyre!("{e}"))?;
            remove_tree(&dir)
                .map_err(|e| color_eyre::eyre::eyre!("Failed to delete {}: {e}", dir.display()))?;
            Ok(())
        })
        .await?;
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
        Ok(())
    }

    /// At startup, in the background: sweeps temporary files a crash left, retakes the
    /// baseline of servers that were running when the core stopped (they rewrote files
    /// meanwhile), and records the first content image of each stopped server that has
    /// none, one at a time on one thread. A running server gets its first image when it
    /// stops.
    pub async fn startup_sweep(self: Arc<Self>, uuids: Vec<InstanceUuid>) {
        let store = self.store();
        let swept = blocking(move || Ok(store.sweep_temp(lodestone_image::store::TEMP_MAX_AGE)))
            .await
            .unwrap_or(0);
        if swept > 0 {
            info!("Deleted {swept} temporary image files left by a crash");
        }
        for uuid in uuids {
            if !Self::accepts(&uuid) {
                continue;
            }
            let Some(ctx) = self.context(&uuid).await else {
                continue;
            };
            if ctx.running {
                continue;
            }
            if let Err(e) = self.after_restart(ctx).await {
                warn!("Failed to record the content of {uuid}: {}", e.source);
            }
        }
    }
}

/// Deletes a folder and everything in it, clearing read-only first where that stops a
/// delete (Windows). Missing is fine.
fn remove_tree(dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if let Ok(meta) = entry.metadata() {
            let mut permissions = meta.permissions();
            if permissions.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(entry.path(), permissions);
            }
        }
    }
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Replaces the small file at `path` (in an instance's image folder) crash-safely: written
/// to a temporary file named as the store names its own (`.incoming-<16 hex>.tmp`, so the
/// store's sweep deletes one a crash left), flushed, renamed over `path`, and the folder
/// flushed. Blocking.
pub(crate) fn write_small_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let dir = path
        .parent()
        .ok_or_else(|| color_eyre::eyre::eyre!("{} has no folder", path.display()))?;
    std::fs::create_dir_all(dir)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to create {}: {e}", dir.display()))?;
    let temp = dir.join(format!(
        ".incoming-{}.tmp",
        crate::backups::store::random_suffix()
    ));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        // The rename itself, durably.
        #[cfg(unix)]
        std::fs::File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(Error::from(color_eyre::eyre::eyre!(
            "Failed to write {}: {e}",
            path.display()
        )));
    }
    Ok(())
}

/// Runs blocking work on tokio's blocking threads.
pub(crate) async fn blocking<T, F>(work: F) -> Result<T, Error>
where
    F: FnOnce() -> Result<T, Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| Error::from(color_eyre::eyre::eyre!("An image task failed: {e}")))?
}

#[cfg(test)]
pub(crate) mod tests;
