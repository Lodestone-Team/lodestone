//! [`BackupService`]: the locks, jobs and cancellation around the store, and the operations
//! the HTTP handlers and the scheduler call.
//!
//! Locks, outermost first:
//! - the server's job mutex: one backup or restore per server at a time (`try_lock`, so a
//!   second one is refused rather than queued);
//! - for a restore, the server's mods lock, like a modpack update;
//! - `lock_stopped` on the instance while a stopped server is backed up or restored, so it
//!   can't start meanwhile;
//! - the store lock: jobs share it; garbage collection takes it alone, and only when it is
//!   free (`try_write`), so it never holds up a job: otherwise it is marked pending and runs
//!   when the lock is next let go (by a job, a download or GC itself);
//! - the server's manifests mutex, around every manifest write and delete.
//!
//! A job takes its guards once ([`JobGuards`]) and a restore's safety backup runs inside
//! the restore's guards ([`BackupService::create_locked`]): neither `lock_stopped` nor the
//! job mutex is re-entrant.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use color_eyre::eyre::{eyre, Context};
use dashmap::DashMap;
use tokio::sync::{Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, RwLock};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{error, info, warn};

use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{
    CausedBy, Event, ProgressionEndValue, ProgressionEventID, ProgressionStartValue,
};
use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::{GameType, TConfigurable};
use crate::traits::t_server::{State, TServer};
use crate::traits::TInstance;
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::AppState;

use super::live::{with_saving_off, InstanceLines, Limits, LiveServer};
use super::pattern::Matcher;
use super::restore::{self, Pin, RestoreDiff, RestorePlan, SwapError};
use super::retention::{self, LastAttempt, RetentionItem};
use super::store::{
    cancelled, check_backup_name, checked_level_name, err, free_space, new_backup_id,
    require_free_space, valid_backup_id, BackupMeta, Prefetched, Scan, ScanRequest, Store,
};
use super::types::{
    BackupInfo, BackupKind, BackupList, BackupPolicy, BackupUsage, FileEntry, JobKind, Manifest,
    RestoreRequest, RestoreScope, RunningJob, SkipReason, MAX_NAME_LEN,
};

/// At most one manual backup per server in this time.
const MANUAL_BACKUP_INTERVAL: Duration = Duration::from_secs(60);
/// How long a restore waits for the Mods tab (or a modpack update) to let go of the server.
const MODS_LOCK_WAIT: Duration = Duration::from_secs(5);
/// Progress updates are sent at most this often, unless they move a whole percent.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// The world folder when server.properties has no `level-name`.
const DEFAULT_LEVEL_NAME: &str = "world";

fn conflict(message: impl Into<String>) -> Error {
    err(ErrorKind::Conflict, message)
}

/// Runs blocking store work off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| Error::from(eyre!("A backup task failed: {e}")))?
}

/// A per-server lock from `map`, made on first use. Locks nobody holds are forgotten, so
/// deleted servers don't pile up.
fn lock_in(
    map: &StdMutex<HashMap<InstanceUuid, Arc<Mutex<()>>>>,
    uuid: &InstanceUuid,
) -> Arc<Mutex<()>> {
    let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
    map.retain(|_, lock| Arc::strong_count(lock) > 1);
    map.entry(uuid.clone()).or_default().clone()
}

#[async_trait]
impl LiveServer for MinecraftInstance {
    async fn send_command(&self, command: &str) -> Result<(), Error> {
        TServer::send_command(self, command, CausedBy::System).await
    }

    async fn is_running(&self) -> bool {
        self.state().await == State::Running
    }
}

/// The guards a backup or restore holds from start to end.
pub struct JobGuards {
    _job: OwnedMutexGuard<()>,
    /// `lock_stopped`: the server is stopped and can't start. `None` for a live backup.
    stopped: Option<OwnedMutexGuard<()>>,
    /// The server's mods lock, for a restore.
    _mods: Option<OwnedMutexGuard<()>>,
    /// The shared store lock. Dropped before retention and GC.
    store: Option<StoreGuard>,
}

/// A share of the store lock ([`BackupService::store_read`]). When it is let go, the GC
/// that waited for it runs ([`BackupService::after_job`]), whoever held it: a backup, a
/// restore, a download, or a request that was dropped half way.
pub struct StoreGuard {
    guard: Option<OwnedRwLockReadGuard<()>>,
    service: Arc<BackupService>,
}

impl Drop for StoreGuard {
    fn drop(&mut self) {
        drop(self.guard.take());
        // GC runs on the runtime, which is gone only when everything is shutting down
        if tokio::runtime::Handle::try_current().is_ok() {
            self.service.after_job();
        }
    }
}

/// A running backup or restore: its progression event and what `GET …/backups` shows.
#[derive(Clone)]
struct Job(Arc<JobInner>);

struct JobInner {
    broadcaster: EventBroadcaster,
    event_id: ProgressionEventID,
    /// The Start event until it is sent: a scheduled backup sends it only once it found
    /// changes.
    pending_start: StdMutex<Option<Event>>,
    progress: StdMutex<JobProgress>,
}

struct JobProgress {
    status: RunningJob,
    /// The progress the update events sent so far add up to.
    sent: f64,
    sent_at: Instant,
    started: bool,
}

impl Job {
    fn new(
        broadcaster: EventBroadcaster,
        kind: JobKind,
        title: String,
        start_value: ProgressionStartValue,
        caused_by: CausedBy,
    ) -> Job {
        let (start, event_id) =
            Event::new_progression_event_start(title, Some(100.0), Some(start_value), caused_by);
        let status = RunningJob {
            kind,
            task_id: event_id.inner().to_string(),
            started: Utc::now().timestamp(),
            progress: 0.0,
            message: String::new(),
        };
        Job(Arc::new(JobInner {
            broadcaster,
            event_id,
            pending_start: StdMutex::new(Some(start)),
            progress: StdMutex::new(JobProgress {
                status,
                sent: 0.0,
                sent_at: Instant::now(),
                started: false,
            }),
        }))
    }

    fn task_id(&self) -> String {
        self.0.event_id.inner().to_string()
    }

    /// Sends the Start event, once.
    fn begin(&self) {
        let start = self
            .0
            .pending_start
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(start) = start {
            self.0.broadcaster.send(start);
            self.0
                .progress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .started = true;
        }
    }

    fn started(&self) -> bool {
        self.0
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .started
    }

    /// The job as the list shows it, once it has started.
    fn status(&self) -> Option<RunningJob> {
        let progress = self.0.progress.lock().unwrap_or_else(|e| e.into_inner());
        progress.started.then(|| progress.status.clone())
    }

    /// Moves the progress to `at` (0..=100; it never goes back), with a new message if
    /// given. The update events carry deltas that add up to 100; they are sent when the
    /// message changes, a whole percent was made, or a while passed.
    fn report(&self, at: f64, message: Option<&str>) {
        let mut progress = self.0.progress.lock().unwrap_or_else(|e| e.into_inner());
        let at = at.clamp(progress.status.progress, 100.0);
        let new_message = message.is_some_and(|m| m != progress.status.message);
        progress.status.progress = at;
        if let Some(message) = message {
            progress.status.message = message.to_string();
        }
        if !progress.started {
            return;
        }
        let delta = at - progress.sent;
        if new_message
            || delta >= 1.0
            || (delta > 0.0 && progress.sent_at.elapsed() >= PROGRESS_INTERVAL)
        {
            self.0.broadcaster.send(Event::new_progression_event_update(
                &self.0.event_id,
                &progress.status.message,
                delta,
            ));
            progress.sent = at;
            progress.sent_at = Instant::now();
        }
    }

    /// Sends the End event, if the Start was sent.
    fn end(&self, success: bool, message: &str, value: ProgressionEndValue) {
        if !self.started() {
            return;
        }
        if success {
            self.report(100.0, None);
        }
        self.0.broadcaster.send(Event::new_progression_event_end(
            self.0.event_id.clone(),
            success,
            Some(message),
            Some(value),
        ));
    }

    /// The part of the job from `base` to `base + span` percent.
    fn phase(&self, base: f64, span: f64) -> Phase {
        Phase {
            job: self.clone(),
            base,
            span,
        }
    }
}

/// A part of a job's progress: 0..=100 within the phase maps onto its share of the job.
#[derive(Clone)]
struct Phase {
    job: Job,
    base: f64,
    span: f64,
}

impl Phase {
    fn report(&self, at: f64, message: Option<&str>) {
        self.job.report(
            self.base + self.span * at.clamp(0.0, 100.0) / 100.0,
            message,
        );
    }

    fn begin(&self) {
        self.job.begin();
    }
}

/// A backup taken by [`BackupService::create_locked`].
pub struct Created {
    /// The new backup, or the newest one when nothing had changed since it.
    pub id: String,
    /// The new backup's manifest; `None` when nothing had changed.
    pub manifest: Option<Manifest>,
    /// Files that couldn't be read.
    pub unreadable: usize,
    pub live: bool,
    pub quiesced: bool,
}

/// What the blocking part of a backup produced.
enum Copied {
    /// Nothing changed since the backup `id`. `scheduled_since_full` is what to record
    /// ([`Scan::scheduled_since_full_after`]):
    /// a full scan counts even when it saved nothing.
    Unchanged {
        id: String,
        scheduled_since_full: Option<usize>,
    },
    Finished {
        manifest: Box<Manifest>,
        unreadable: usize,
        /// What to record once the manifest is saved.
        scheduled_since_full: Option<usize>,
    },
}

/// Everything the blocking part of a backup needs, owned, so it can move to a blocking
/// thread (and keep running there, to its next cancellation check, if the async side is
/// dropped).
struct CopyWork {
    store: Store,
    root: PathBuf,
    uuid: InstanceUuid,
    kind: BackupKind,
    exclude: Matcher,
    /// Write nothing when nothing changed (scheduled backups and unnamed manual ones; see
    /// [`CopyWork::skips_unchanged`]).
    skip_unchanged: bool,
    meta: BackupMeta,
    phase: Phase,
    /// What [`CopyWork::prefetch`] read.
    prefetched: Option<Prefetched>,
}

impl CopyWork {
    /// For a live backup, before saving is turned off: when there is a lot to read, reads it
    /// now with [`Store::prefetch`] (the first 70 % of the phase), so that saving stays off
    /// only while what changed since is read.
    fn prefetch(mut self, cancel: &dyn Fn() -> bool) -> Result<CopyWork, Error> {
        let scan = self.store.scan(
            ScanRequest {
                server_root: &self.root,
                uuid: &self.uuid,
                kind: self.kind,
                exclude: &self.exclude,
            },
            cancel,
        )?;
        if !scan.worth_prefetching() {
            return Ok(self);
        }
        let reading = self.phase.job.phase(self.phase.base, self.phase.span * 0.7);
        reading.begin();
        reading.report(0.0, Some("Copying files"));
        let prefetched = self.store.prefetch(
            &scan,
            &mut |done, total| reading.report(100.0 * done as f64 / total.max(1) as f64, None),
            cancel,
        )?;
        self.prefetched = Some(prefetched);
        self.phase = self
            .phase
            .job
            .phase(reading.base + reading.span, self.phase.span * 0.3);
        Ok(self)
    }

    /// Whether to write nothing because nothing changed since `previous`, the newest backup.
    ///
    /// An unnamed manual backup is skipped only when `previous` is one retention never
    /// deletes (manual or pinned). Otherwise the state the person asked to keep would live
    /// only in, say, a scheduled backup that cleanup deletes hours later; a manual backup is
    /// written instead, which costs just its manifest since the files are already stored.
    fn skips_unchanged(&self, previous: &Manifest) -> bool {
        self.skip_unchanged
            && (self.kind != BackupKind::Manual
                || retention::never_deleted(previous.kind, previous.pinned))
    }

    /// Walk 5 %, copy 90 % (the manifest write is the last 5 %).
    fn run(mut self, quiesced: bool, cancel: &dyn Fn() -> bool) -> Result<Copied, Error> {
        let started = Instant::now();
        self.phase.report(0.0, Some("Looking for changed files"));
        let scan = self.store.scan_with(
            ScanRequest {
                server_root: &self.root,
                uuid: &self.uuid,
                kind: self.kind,
                exclude: &self.exclude,
            },
            self.prefetched.as_ref(),
            cancel,
        )?;
        let unchanged = |work: &CopyWork, scan: &Scan| {
            scan.previous
                .as_ref()
                .filter(|previous| work.skips_unchanged(previous))
                .map(|previous| Copied::Unchanged {
                    id: previous.id.clone(),
                    scheduled_since_full: scan.scheduled_since_full_after(false),
                })
        };
        if !scan.maybe_changed() {
            if let Some(unchanged) = unchanged(&self, &scan) {
                return Ok(unchanged);
            }
        }
        self.phase.begin();
        self.phase.report(5.0, Some("Copying files"));
        require_free_space(self.store.root(), scan.bytes_to_store(), "for a backup")?;
        let phase = self.phase.clone();
        let copied = self.store.copy(
            &scan,
            &mut |done, total| {
                let share = if total == 0 {
                    1.0
                } else {
                    done as f64 / total as f64
                };
                phase.report(5.0 + 90.0 * share, None);
            },
            cancel,
        )?;
        if copied.unchanged_since(scan.previous.as_ref()) {
            if let Some(unchanged) = unchanged(&self, &scan) {
                return Ok(unchanged);
            }
        }
        let unreadable = copied.unreadable_count();
        let scheduled_since_full = scan.scheduled_since_full_after(true);
        self.meta.quiesced = quiesced;
        let manifest = self.store.finish(
            self.meta,
            &scan,
            copied,
            started.elapsed().as_millis() as u64,
        );
        Ok(Copied::Finished {
            manifest: Box::new(manifest),
            unreadable,
            scheduled_since_full,
        })
    }
}

/// What the list and retention need of a server's backups, cached until one changes.
#[derive(Default)]
struct Summary {
    /// Newest first.
    backups: Vec<BackupInfo>,
    /// Every object the server's backups use.
    hashes: HashSet<String>,
}

impl Summary {
    /// The newest Scheduled or Manual backup's time, which the schedule counts from.
    fn newest_for_schedule(&self) -> Option<i64> {
        self.backups
            .iter()
            .filter(|b| matches!(b.kind, BackupKind::Scheduled | BackupKind::Manual))
            .map(|b| b.created)
            .max()
    }
}

/// The scheduler's memory of a server.
#[derive(Debug, Clone, Copy)]
struct ScheduleState {
    last_attempt: Option<LastAttempt>,
    /// Players were on since the last backup (or the core started since).
    active: bool,
}

impl Default for ScheduleState {
    fn default() -> Self {
        ScheduleState {
            last_attempt: None,
            active: true,
        }
    }
}

/// Checks what a restore would change against the requester's permissions (protected
/// files, code files). Made by the handler.
pub type DiffCheck = Arc<dyn Fn(&RestoreDiff) -> Result<(), Error> + Send + Sync>;

/// A backup restored as a new server, as the handler asks for it.
pub struct NewServerRequest {
    pub name: String,
    /// Keep the backup's `java_cmd` and `cmd_args`; otherwise the new server gets the
    /// managed Java and no extra arguments.
    pub keep_command_line: bool,
    pub requester_uid: crate::auth::user_id::UserId,
    pub caused_by: CausedBy,
}

/// Why an in-place restore failed, which decides whether a server it stopped may start
/// again.
enum RestoreFailure {
    /// The server folder is as it was: the restore failed before the swap, or the swap was
    /// undone.
    Unchanged(Error),
    /// The swap couldn't be undone: the server stays locked until Lodestone repairs it.
    Broken(Error),
}

impl From<Error> for RestoreFailure {
    fn from(e: Error) -> Self {
        RestoreFailure::Unchanged(e)
    }
}

/// How a swap ended, as far as the server folder goes.
enum SwapOutcome {
    /// The swap was undone: the folder is as it was.
    RolledBack(Error),
    /// The folder may be half swapped: its journal is left for the repair at the next start.
    Broken(Error),
}

/// The swap's result, from its blocking task. A task that panicked or was cancelled
/// stopped somewhere in the renames, so the folder may be half swapped: that is broken,
/// never unchanged.
fn swap_outcome(
    joined: Result<Result<(), SwapError>, tokio::task::JoinError>,
) -> Result<(), SwapOutcome> {
    match joined {
        Ok(Ok(())) => Ok(()),
        Ok(Err(SwapError::RolledBack(e))) => Err(SwapOutcome::RolledBack(e)),
        Ok(Err(SwapError::Broken(e))) => Err(SwapOutcome::Broken(e)),
        Err(join) => Err(SwapOutcome::Broken(Error::from(eyre!(
            "The restore task failed: {join}"
        )))),
    }
}

/// Whether a server that a failed restore stopped is started again: when the restore was
/// to start it after, nothing changed, and Lodestone isn't shutting down.
fn start_again_after(
    failure: &RestoreFailure,
    was_running: bool,
    start_after: bool,
    shutting_down: bool,
) -> bool {
    matches!(failure, RestoreFailure::Unchanged(_)) && was_running && start_after && !shutting_down
}

/// The name of the safety backup a restore of `manifest` takes first: "Before restoring
/// “…”" with the backup's name, or its date when it has none. The name is shortened to fit
/// [`MAX_NAME_LEN`], since the wrapper adds 19 characters to a name that may already be that
/// long, and restoring a safety backup nests them; a name that didn't fit would only be
/// refused after every file was copied.
fn safety_backup_name(manifest: &Manifest) -> String {
    let label = manifest.name.clone().unwrap_or_else(|| {
        chrono::DateTime::from_timestamp(manifest.created, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| manifest.id.clone())
    });
    let wrap = |label: &str| format!("Before restoring “{label}”");
    let room = MAX_NAME_LEN - wrap("").chars().count();
    if label.chars().count() <= room {
        return wrap(&label);
    }
    let cut: String = label.chars().take(room - 1).collect();
    wrap(&format!("{}…", cut.trim_end()))
}

/// The files a restore with `plan` replaces that its safety backup `safety` couldn't read,
/// so has no copy of as they are now (at most an older one, carried over). The restore
/// would delete them for good, so it is refused instead.
fn not_in_safety_backup<'a>(plan: &RestorePlan, safety: &'a Manifest) -> Vec<&'a str> {
    safety
        .skipped
        .iter()
        .filter(|s| matches!(s.reason, SkipReason::Unreadable | SkipReason::KeptChanging))
        .filter(|s| plan.owns(&s.path))
        .map(|s| s.path.as_str())
        .collect()
}

/// Server backups: the store, its locks, the running jobs and the scheduler's state.
pub struct BackupService {
    store: Store,
    instances_dir: PathBuf,
    store_lock: Arc<RwLock<()>>,
    jobs: StdMutex<HashMap<InstanceUuid, Arc<Mutex<()>>>>,
    manifest_locks: StdMutex<HashMap<InstanceUuid, Arc<Mutex<()>>>>,
    running: StdMutex<HashMap<InstanceUuid, Job>>,
    last_manual: StdMutex<HashMap<InstanceUuid, Instant>>,
    schedule: StdMutex<HashMap<InstanceUuid, ScheduleState>>,
    /// Servers whose restore couldn't be undone, found by [`Self::recover`], until
    /// [`Self::lock_broken_servers`] locks them.
    unrepaired: StdMutex<Vec<InstanceUuid>>,
    /// The `lock_stopped` guards of servers whose restore couldn't be undone: they can't
    /// start until the core restarts and repairs them.
    broken: StdMutex<HashMap<InstanceUuid, OwnedMutexGuard<()>>>,
    gc_pending: AtomicBool,
    cache: StdMutex<HashMap<InstanceUuid, Arc<Summary>>>,
    /// Bumped whenever a summary is dropped, so a summary read before can't be cached.
    cache_generation: AtomicU64,
    /// Compressed object sizes. Objects only go away in GC, which clears this.
    object_sizes: StdMutex<HashMap<String, u64>>,
    cancel: CancellationToken,
    tracker: TaskTracker,
    /// Notified when a schedule is saved, for the scheduler while no server has one.
    schedule_changed: tokio::sync::Notify,
}

impl BackupService {
    /// A service for the store at `root` (`<lodestone>/backups`), for the servers in
    /// `instances_dir`. Nothing is read until it is used.
    pub fn new(root: PathBuf, instances_dir: PathBuf) -> BackupService {
        BackupService {
            store: Store::new(root),
            instances_dir,
            store_lock: Arc::new(RwLock::new(())),
            jobs: Default::default(),
            manifest_locks: Default::default(),
            running: Default::default(),
            last_manual: Default::default(),
            schedule: Default::default(),
            unrepaired: Default::default(),
            broken: Default::default(),
            gc_pending: AtomicBool::new(false),
            cache: Default::default(),
            cache_generation: AtomicU64::new(0),
            object_sizes: Default::default(),
            cancel: CancellationToken::new(),
            tracker: TaskTracker::new(),
            schedule_changed: tokio::sync::Notify::new(),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Resolves once a server's schedule was saved since the last call (or is, if none
    /// was): the scheduler waits on this while no server has scheduled backups on.
    pub async fn schedule_changed(&self) {
        self.schedule_changed.notified().await
    }

    /// Cancelled when the core shuts down.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Runs `task` on the job tracker, so shutdown waits for it (a little).
    pub fn spawn<F>(&self, task: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.tracker.spawn(task);
    }

    /// At shutdown: cancels the jobs and waits for them to stop. The caller bounds the
    /// wait.
    pub async fn shutdown(&self) {
        self.cancel.cancel();
        self.tracker.close();
        self.tracker.wait().await;
    }

    // -----------------------------------------------------------------------------------
    // Startup

    /// At startup, before the servers are loaded: undoes interrupted restores and deletes
    /// leftovers (see [`restore::recover`]). Blocking.
    pub fn recover(&self) -> restore::Recovery {
        let recovery = restore::recover(&self.store, &self.instances_dir);
        for uuid in &recovery.repaired {
            info!("Undid an interrupted restore of {uuid}");
        }
        for uuid in &recovery.broken {
            error!("An interrupted restore of {uuid} could not be undone; the server won't start until Lodestone repairs it at its next start");
        }
        *self.unrepaired.lock().unwrap_or_else(|e| e.into_inner()) = recovery.broken.clone();
        recovery
    }

    /// After the servers are loaded: keeps the servers [`Self::recover`] couldn't repair
    /// from starting.
    pub async fn lock_broken_servers(&self, instances: &DashMap<InstanceUuid, GameInstance>) {
        let unrepaired =
            std::mem::take(&mut *self.unrepaired.lock().unwrap_or_else(|e| e.into_inner()));
        for uuid in unrepaired {
            let Some(instance) = instances.get(&uuid).map(|i| i.value().clone()) else {
                continue;
            };
            let GameInstance::MinecraftInstance(instance) = instance;
            match instance
                .lock_stopped("repairing an interrupted restore")
                .await
            {
                Ok(guard) => {
                    self.broken
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(uuid, guard);
                }
                Err(e) => warn!("Failed to lock {uuid} after a failed repair: {}", e.source),
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // Locks

    /// The server's job mutex, or `None` when a backup or restore of it is running.
    pub fn try_lock_job(&self, uuid: &InstanceUuid) -> Option<OwnedMutexGuard<()>> {
        lock_in(&self.jobs, uuid).try_lock_owned().ok()
    }

    /// Whether a backup or restore of the server is running.
    pub fn job_running(&self, uuid: &InstanceUuid) -> bool {
        lock_in(&self.jobs, uuid).try_lock().is_err()
    }

    fn lock_job(&self, uuid: &InstanceUuid) -> Result<OwnedMutexGuard<()>, Error> {
        self.try_lock_job(uuid)
            .ok_or_else(|| conflict("A backup or restore of this server is already running"))
    }

    async fn lock_manifests(&self, uuid: &InstanceUuid) -> OwnedMutexGuard<()> {
        lock_in(&self.manifest_locks, uuid).lock_owned().await
    }

    async fn store_read(self: &Arc<Self>) -> StoreGuard {
        StoreGuard {
            guard: Some(self.store_lock.clone().read_owned().await),
            service: self.clone(),
        }
    }

    /// The job mutex, and `lock_stopped` when the server is stopped. A running server is
    /// backed up live; one starting or stopping is refused.
    async fn take_guards(
        &self,
        instance: &MinecraftInstance,
        what: &str,
    ) -> Result<JobGuards, Error> {
        let uuid = instance.uuid().await;
        let job = self.lock_job(&uuid)?;
        let stopped = match instance.state().await {
            State::Running => None,
            State::Starting | State::Stopping => {
                return Err(conflict(
                    "Wait until the server has finished starting or stopping",
                ))
            }
            State::Stopped | State::Error => Some(instance.lock_stopped(what).await?),
        };
        Ok(JobGuards {
            _job: job,
            stopped,
            _mods: None,
            store: None,
        })
    }

    fn register(&self, uuid: &InstanceUuid, job: &Job) {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(uuid.clone(), job.clone());
    }

    fn unregister(&self, uuid: &InstanceUuid) {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
    }

    fn running_job(&self, uuid: &InstanceUuid) -> Option<RunningJob> {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .and_then(Job::status)
    }

    // -----------------------------------------------------------------------------------
    // Cache

    fn invalidate(&self, uuid: &InstanceUuid) {
        self.cache_generation.fetch_add(1, Ordering::SeqCst);
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
    }

    async fn summary(&self, uuid: &InstanceUuid) -> Result<Arc<Summary>, Error> {
        if let Some(summary) = self
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
        {
            return Ok(summary.clone());
        }
        let generation = self.cache_generation.load(Ordering::SeqCst);
        let store = self.store.clone();
        let id = uuid.clone();
        let summary = blocking(move || {
            let manifests = store.list_manifests(&id)?;
            let mut hashes = HashSet::new();
            for m in &manifests {
                hashes.extend(m.files.iter().map(|f| f.sha256.clone()));
            }
            Ok(Summary {
                backups: manifests.iter().map(BackupInfo::from).collect(),
                hashes,
            })
        })
        .await?;
        let summary = Arc::new(summary);
        if self.cache_generation.load(Ordering::SeqCst) == generation {
            self.cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(uuid.clone(), summary.clone());
        }
        Ok(summary)
    }

    async fn usage(self: &Arc<Self>, summary: Arc<Summary>) -> Result<BackupUsage, Error> {
        let this = self.clone();
        blocking(move || {
            let mut stored_bytes = 0;
            let mut missing = Vec::new();
            {
                let sizes = this.object_sizes.lock().unwrap_or_else(|e| e.into_inner());
                for hash in &summary.hashes {
                    match sizes.get(hash) {
                        Some(size) => stored_bytes += size,
                        None => missing.push(hash.clone()),
                    }
                }
            }
            let mut found = Vec::new();
            for hash in missing {
                if let Some(size) = this.store.object_stored_size(&hash) {
                    stored_bytes += size;
                    found.push((hash, size));
                }
            }
            this.object_sizes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(found);
            Ok(BackupUsage {
                total_bytes: summary.backups.iter().map(|b| b.total_bytes).sum(),
                stored_bytes,
                disk_free: free_space(this.store.root()),
            })
        })
        .await
    }

    // -----------------------------------------------------------------------------------
    // Reading

    /// `GET /instance/{uuid}/backups`.
    pub async fn list(self: &Arc<Self>, uuid: &InstanceUuid) -> Result<BackupList, Error> {
        self.store.server_dir(uuid)?;
        let summary = self.summary(uuid).await?;
        let policy = self.policy(uuid).await?;
        let usage = self.usage(summary.clone()).await?;
        let last_attempt = self.schedule_state(uuid).last_attempt;
        let next_scheduled = retention::next_due(
            &policy,
            summary.newest_for_schedule(),
            last_attempt,
            Utc::now().timestamp(),
        );
        Ok(BackupList {
            backups: summary.backups.clone(),
            policy,
            running: self.running_job(uuid),
            usage,
            next_scheduled,
        })
    }

    /// A backup's manifest: NotFound when there is no such backup.
    pub async fn load_manifest(&self, uuid: &InstanceUuid, id: &str) -> Result<Manifest, Error> {
        if !valid_backup_id(id) {
            return Err(err(ErrorKind::NotFound, "Backup not found"));
        }
        let store = self.store.clone();
        let (uuid, id) = (uuid.clone(), id.to_string());
        blocking(move || store.load_manifest(&uuid, &id)).await
    }

    pub async fn policy(&self, uuid: &InstanceUuid) -> Result<BackupPolicy, Error> {
        let store = self.store.clone();
        let uuid = uuid.clone();
        blocking(move || store.load_policy(&uuid)).await
    }

    // -----------------------------------------------------------------------------------
    // Changing backups and the policy

    /// `PUT /instance/{uuid}/backups/policy`: validates (400 naming the field) and saves.
    pub async fn set_policy(
        &self,
        uuid: &InstanceUuid,
        policy: BackupPolicy,
    ) -> Result<BackupPolicy, Error> {
        policy
            .validate()
            .map_err(|e| err(ErrorKind::BadRequest, e))?;
        let _manifests = self.lock_manifests(uuid).await;
        let store = self.store.clone();
        let (id, saved) = (uuid.clone(), policy.clone());
        blocking(move || store.save_policy(&id, &saved)).await?;
        self.schedule_changed.notify_one();
        Ok(policy)
    }

    /// `PATCH /instance/{uuid}/backups/{id}`: renames (`Some(None)` clears the name) and
    /// pins or unpins a backup.
    pub async fn update(
        &self,
        uuid: &InstanceUuid,
        id: &str,
        name: Option<Option<String>>,
        pinned: Option<bool>,
    ) -> Result<BackupInfo, Error> {
        let name = match name {
            Some(Some(name)) => {
                Some(check_backup_name(&name).map_err(|e| err(ErrorKind::BadRequest, e))?)
            }
            Some(None) => Some(None),
            None => None,
        };
        let _manifests = self.lock_manifests(uuid).await;
        let mut manifest = self.load_manifest(uuid, id).await?;
        if let Some(name) = name {
            manifest.name = name;
        }
        if let Some(pinned) = pinned {
            manifest.pinned = pinned;
        }
        let store = self.store.clone();
        let saved = manifest.clone();
        blocking(move || store.save_manifest(&saved)).await?;
        self.invalidate(uuid);
        Ok(BackupInfo::from(&manifest))
    }

    /// `DELETE /instance/{uuid}/backups/{id}`: removes the manifest; its objects go at the
    /// next GC.
    pub async fn delete(self: &Arc<Self>, uuid: &InstanceUuid, id: &str) -> Result<(), Error> {
        if !valid_backup_id(id) {
            return Err(err(ErrorKind::NotFound, "Backup not found"));
        }
        {
            let _manifests = self.lock_manifests(uuid).await;
            let store = self.store.clone();
            let (uuid, id) = (uuid.clone(), id.to_string());
            blocking(move || store.delete_manifest(&uuid, &id)).await?;
        }
        self.invalidate(uuid);
        self.request_gc();
        Ok(())
    }

    /// When a server is deleted: deletes all its backups and policy. The caller holds the
    /// server's job mutex.
    pub async fn delete_server(self: &Arc<Self>, uuid: &InstanceUuid) -> Result<(), Error> {
        {
            let _manifests = self.lock_manifests(uuid).await;
            let store = self.store.clone();
            let uuid = uuid.clone();
            blocking(move || store.delete_server(&uuid)).await?;
        }
        self.invalidate(uuid);
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
        self.request_gc();
        Ok(())
    }

    // -----------------------------------------------------------------------------------
    // Retention and garbage collection

    /// Deletes the backups the server's retention doesn't keep. `just_created` is never
    /// deleted.
    async fn apply_retention(&self, uuid: &InstanceUuid, just_created: Option<&str>) {
        let result = async {
            let policy = self.policy(uuid).await?;
            let _manifests = self.lock_manifests(uuid).await;
            let summary = self.summary(uuid).await?;
            let items: Vec<RetentionItem> = summary
                .backups
                .iter()
                .map(|b| RetentionItem {
                    id: &b.id,
                    kind: b.kind,
                    created: b.created,
                    pinned: b.pinned,
                })
                .collect();
            let doomed =
                retention::to_delete(&items, Utc::now().timestamp(), &policy, just_created);
            if doomed.is_empty() {
                return Ok(());
            }
            let store = self.store.clone();
            let id = uuid.clone();
            blocking(move || {
                for backup in doomed {
                    match store.delete_manifest(&id, &backup) {
                        Ok(()) => info!("Deleted old backup {backup} of {id}"),
                        Err(e) => warn!("Failed to delete old backup {backup}: {}", e.source),
                    }
                }
                Ok(())
            })
            .await?;
            self.invalidate(uuid);
            Ok::<_, Error>(())
        }
        .await;
        if let Err(e) = result {
            warn!(
                "Failed to apply the backup retention of {uuid}: {}",
                e.source
            );
        }
    }

    /// Runs garbage collection now if no job uses the store, else when the next job ends.
    pub fn request_gc(self: &Arc<Self>) {
        if self.cancel.is_cancelled() {
            return;
        }
        let guard = match self.store_lock.clone().try_write_owned() {
            Ok(guard) => guard,
            Err(_) => {
                self.gc_pending.store(true, Ordering::SeqCst);
                // the holder may have let go (and found nothing pending) in between
                match self.store_lock.clone().try_write_owned() {
                    Ok(guard) => guard,
                    Err(_) => return,
                }
            }
        };
        self.gc_pending.store(false, Ordering::SeqCst);
        let this = self.clone();
        self.tracker.spawn(async move {
            let store = this.store.clone();
            let result = blocking(move || {
                let _alone = guard;
                store.gc()
            })
            .await;
            this.object_sizes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            // a GC asked for while this one ran (backups deleted meanwhile)
            this.after_job();
            match result {
                Ok(report) if report.objects_removed > 0 || report.temp_removed > 0 => info!(
                    "Backup cleanup removed {} unused files ({} bytes)",
                    report.objects_removed + report.temp_removed,
                    report.bytes_removed
                ),
                Ok(_) => {}
                Err(e) => warn!("Backup cleanup failed: {}", e.source),
            }
        });
    }

    /// When the store lock was let go: the GC that waited for it.
    fn after_job(self: &Arc<Self>) {
        if self.gc_pending.load(Ordering::SeqCst) {
            self.request_gc();
        }
    }

    // -----------------------------------------------------------------------------------
    // Taking a backup

    /// Takes a backup inside `guards`: live (saving paused) when they don't hold
    /// `lock_stopped`. The store guard must be held.
    ///
    /// `exclude` is what to leave out; `None` reads the server's policy. A restore's safety
    /// backup passes the exclude its permission check used, since the check after stopping
    /// compares against the safety backup's files.
    #[allow(clippy::too_many_arguments)]
    async fn create_locked(
        &self,
        state: &AppState,
        guards: &JobGuards,
        instance: &MinecraftInstance,
        kind: BackupKind,
        name: Option<String>,
        exclude: Option<Matcher>,
        phase: Phase,
    ) -> Result<Created, Error> {
        debug_assert!(guards.store.is_some());
        let uuid = instance.uuid().await;
        let root = instance.path().await;
        let instance_name = instance.name().await;
        let (game_version, flavour) = instance.server_software().await;
        let level_name = instance
            .server_properties()
            .await
            .ok()
            .and_then(|p| p.get("level-name").map(str::to_string))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| DEFAULT_LEVEL_NAME.to_string());
        let exclude = match exclude {
            Some(exclude) => exclude,
            None => Matcher::new(&self.policy(&uuid).await?.exclude),
        };
        let live = guards.stopped.is_none();
        let now = Utc::now();
        let skip_unchanged =
            kind == BackupKind::Scheduled || (kind == BackupKind::Manual && name.is_none());
        let work = CopyWork {
            store: self.store.clone(),
            root,
            uuid: uuid.clone(),
            kind,
            exclude,
            skip_unchanged,
            meta: BackupMeta {
                id: new_backup_id(now),
                instance_uuid: uuid.clone(),
                instance_name: instance_name.clone(),
                game_version,
                flavour: flavour.to_string(),
                name,
                kind,
                created: now.timestamp(),
                live,
                quiesced: true,
                level_name: checked_level_name(&level_name),
            },
            phase: phase.clone(),
            prefetched: None,
        };

        let (copied, quiesced) = if live {
            let console = |message: &str| {
                state.event_broadcaster.send(Event::new_system_message(
                    uuid.clone(),
                    instance_name.clone(),
                    message.to_string(),
                ))
            };
            console("Backing up…");
            let token = self.cancel.clone();
            let result = match blocking(move || work.prefetch(&|| token.is_cancelled())).await {
                Ok(work) => {
                    let mut lines =
                        InstanceLines::new(state.event_broadcaster.subscribe(), uuid.clone());
                    let marker = self.store.marker_path(&uuid)?;
                    with_saving_off(
                        instance,
                        &mut lines,
                        &marker,
                        Limits::default(),
                        &self.cancel,
                        |quiesced, token| async move {
                            blocking(move || work.run(quiesced, &|| token.is_cancelled())).await
                        },
                    )
                    .await
                }
                Err(e) => Err(e),
            };
            console(if result.is_ok() {
                "Backup done"
            } else {
                "Backup failed"
            });
            result?
        } else {
            let token = self.cancel.clone();
            let copied = blocking(move || work.run(true, &|| token.is_cancelled())).await?;
            (copied, true)
        };

        let (mut manifest, unreadable, scheduled_since_full) = match copied {
            Copied::Unchanged {
                id,
                scheduled_since_full,
            } => {
                self.record_scheduled_since_full(&uuid, scheduled_since_full)
                    .await;
                self.mark_backed_up(&uuid);
                return Ok(Created {
                    id,
                    manifest: None,
                    unreadable: 0,
                    live,
                    quiesced,
                });
            }
            Copied::Finished {
                manifest,
                unreadable,
                scheduled_since_full,
            } => (*manifest, unreadable, scheduled_since_full),
        };
        // the copy knew whether the server saved, but not whether it stopped meanwhile
        manifest.quiesced = quiesced;
        phase.report(95.0, Some("Saving the backup"));
        {
            let _manifests = self.lock_manifests(&uuid).await;
            let store = self.store.clone();
            let saved = manifest.clone();
            blocking(move || store.save_manifest(&saved)).await?;
        }
        self.record_scheduled_since_full(&uuid, scheduled_since_full)
            .await;
        self.invalidate(&uuid);
        self.mark_backed_up(&uuid);
        phase.report(100.0, None);
        Ok(Created {
            id: manifest.id.clone(),
            manifest: Some(manifest),
            unreadable,
            live,
            quiesced,
        })
    }

    /// Records how many scheduled backups were saved since the last full scan. A failure
    /// only makes the next full scan come early or late, so it is logged, not returned.
    async fn record_scheduled_since_full(&self, uuid: &InstanceUuid, count: Option<usize>) {
        let Some(count) = count else {
            return;
        };
        let store = self.store.clone();
        let id = uuid.clone();
        if let Err(e) = blocking(move || store.record_scheduled_since_full_scan(&id, count)).await {
            warn!("Failed to record the backup scans of {uuid}: {}", e.source);
        }
    }

    /// The end message of a backup.
    fn backup_message(server_name: &str, created: &Created) -> String {
        if created.manifest.is_none() {
            return "Nothing changed since the last backup".to_string();
        }
        let mut message = format!("Backed up {server_name}");
        if created.unreadable == 1 {
            message.push_str(". 1 file couldn't be read");
        } else if created.unreadable > 1 {
            message.push_str(&format!(". {} files couldn't be read", created.unreadable));
        }
        if created.live && !created.quiesced {
            message.push_str(
                ". The server didn't confirm it had saved, so the backup may be inconsistent",
            );
        }
        message
    }

    fn failure_message(&self, what: &str, e: &Error) -> String {
        if self.cancel.is_cancelled() {
            format!("{what} was stopped because Lodestone is shutting down")
        } else {
            format!("{what} failed: {}", e.source)
        }
    }

    /// `POST /instance/{uuid}/backups`: checks and locks, then backs up in the background.
    /// Returns the progression event's id.
    pub async fn start_backup(
        self: &Arc<Self>,
        state: &AppState,
        instance: MinecraftInstance,
        name: Option<String>,
        caused_by: CausedBy,
    ) -> Result<String, Error> {
        let name = match name {
            Some(name) => check_backup_name(&name).map_err(|e| err(ErrorKind::BadRequest, e))?,
            None => None,
        };
        let uuid = instance.uuid().await;
        self.store.server_dir(&uuid)?;
        let recent = self
            .last_manual
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&uuid)
            .is_some_and(|at| at.elapsed() < MANUAL_BACKUP_INTERVAL);
        if recent {
            return Err(err(
                ErrorKind::TooManyRequests,
                "Wait a minute between manual backups",
            ));
        }
        let mut guards = self.take_guards(&instance, "backing it up").await?;
        self.last_manual
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(uuid.clone(), Instant::now());
        let server_name = instance.name().await;
        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::Backup,
            format!("Backing up {server_name}"),
            ProgressionStartValue::Backup {
                instance_uuid: uuid.clone(),
            },
            caused_by,
        );
        job.begin();
        self.register(&uuid, &job);
        let task_id = job.task_id();
        let (this, state) = (self.clone(), state.clone());
        self.tracker.spawn(async move {
            guards.store = Some(this.store_read().await);
            let result = this
                .create_locked(
                    &state,
                    &guards,
                    &instance,
                    BackupKind::Manual,
                    name,
                    None,
                    job.phase(0.0, 100.0),
                )
                .await;
            drop(guards);
            let (success, message, backup_id) = match &result {
                Ok(created) => (
                    true,
                    Self::backup_message(&server_name, created),
                    Some(created.id.clone()),
                ),
                Err(e) => (false, this.failure_message("The backup", e), None),
            };
            job.end(
                success,
                &message,
                ProgressionEndValue::BackupCompleted {
                    instance_uuid: uuid.clone(),
                    backup_id,
                    success,
                    message: message.clone(),
                },
            );
            this.unregister(&uuid);
        });
        Ok(task_id)
    }

    /// A scheduled backup, run to the end. Sends no events when nothing changed. Then
    /// applies retention and runs GC.
    pub async fn scheduled_backup(
        self: &Arc<Self>,
        state: &AppState,
        instance: MinecraftInstance,
    ) -> Result<(), Error> {
        let uuid = instance.uuid().await;
        self.store.server_dir(&uuid)?;
        let mut guards = self.take_guards(&instance, "backing it up").await?;
        guards.store = Some(self.store_read().await);
        let server_name = instance.name().await;
        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::Backup,
            format!("Backing up {server_name}"),
            ProgressionStartValue::Backup {
                instance_uuid: uuid.clone(),
            },
            CausedBy::System,
        );
        self.register(&uuid, &job);
        let result = self
            .create_locked(
                state,
                &guards,
                &instance,
                BackupKind::Scheduled,
                None,
                None,
                job.phase(0.0, 100.0),
            )
            .await;
        drop(guards);
        match &result {
            Ok(created) => {
                let message = Self::backup_message(&server_name, created);
                job.end(
                    true,
                    &message,
                    ProgressionEndValue::BackupCompleted {
                        instance_uuid: uuid.clone(),
                        backup_id: Some(created.id.clone()),
                        success: true,
                        message: message.clone(),
                    },
                );
            }
            Err(e) => {
                // a failure is told even when it came before any change was found
                job.begin();
                let message = self.failure_message("The scheduled backup", e);
                job.end(
                    false,
                    &message,
                    ProgressionEndValue::BackupCompleted {
                        instance_uuid: uuid.clone(),
                        backup_id: None,
                        success: false,
                        message: message.clone(),
                    },
                );
            }
        }
        self.unregister(&uuid);
        if let Ok(Created {
            manifest: Some(manifest),
            ..
        }) = &result
        {
            self.apply_retention(&uuid, Some(&manifest.id)).await;
            self.request_gc();
        }
        result.map(|_| ())
    }

    // -----------------------------------------------------------------------------------
    // Scheduler state

    fn schedule_state(&self, uuid: &InstanceUuid) -> ScheduleState {
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .copied()
            .unwrap_or_default()
    }

    /// Players are on the server.
    pub fn mark_active(&self, uuid: &InstanceUuid) {
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_default()
            .active = true;
    }

    /// A backup of the server succeeded: it isn't active until players are on again.
    fn mark_backed_up(&self, uuid: &InstanceUuid) {
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_default()
            .active = false;
    }

    /// Records a scheduled attempt, successful or not.
    pub fn set_last_attempt(&self, uuid: &InstanceUuid, at: i64, failed: bool) {
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_default()
            .last_attempt = Some(LastAttempt { at, failed });
    }

    /// Whether the server's scheduled backup is due at `now`.
    pub async fn is_due(&self, uuid: &InstanceUuid, policy: &BackupPolicy, now: i64) -> bool {
        if !policy.enabled {
            return false;
        }
        let newest = match self.summary(uuid).await {
            Ok(summary) => summary.newest_for_schedule(),
            Err(e) => {
                warn!("Failed to read the backups of {uuid}: {}", e.source);
                return false;
            }
        };
        let schedule = self.schedule_state(uuid);
        retention::due(policy, newest, schedule.last_attempt, now, schedule.active)
    }

    // -----------------------------------------------------------------------------------
    // Restoring in place

    /// `POST /instance/{uuid}/backups/{id}/restore`, after the handler checked the request's
    /// permissions: locks the server and, when `check` is given, checks what the restore
    /// would change; then restores in the background. Returns the progression event's id.
    ///
    /// A running server is stopped first, and started again after when `start_after`, only
    /// with `may_stop`: the handler checked `stop` and the permissions to stop and start it.
    /// It decided that from the state it saw, which may have changed since, so a server
    /// that is running now is refused without it.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_restore(
        self: &Arc<Self>,
        state: &AppState,
        instance: MinecraftInstance,
        manifest: Manifest,
        request: RestoreRequest,
        may_stop: bool,
        check: Option<DiffCheck>,
        caused_by: CausedBy,
    ) -> Result<String, Error> {
        let uuid = instance.uuid().await;
        self.store.server_dir(&uuid)?;
        let was_running = match instance.state().await {
            State::Running if !may_stop => {
                return Err(conflict(
                    "The server started meanwhile; stop it before restoring a backup",
                ))
            }
            State::Running => true,
            State::Starting | State::Stopping => {
                return Err(conflict(
                    "Wait until the server has finished starting or stopping",
                ))
            }
            State::Stopped | State::Error => false,
        };
        let job_guard = self.lock_job(&uuid)?;
        let mods = tokio::time::timeout(MODS_LOCK_WAIT, state.mods.lock_for(&uuid).lock_owned())
            .await
            .map_err(|_| {
                conflict("The server's mods are being changed; try again when that is done")
            })?;
        let mut guards = JobGuards {
            _job: job_guard,
            stopped: None,
            _mods: Some(mods),
            store: Some(self.store_read().await),
        };
        if !was_running {
            guards.stopped = Some(instance.lock_stopped("restoring a backup").await?);
        }
        let root = instance.path().await;
        // the restore keeps using this policy: the check holds only for what it excludes
        let policy = self.policy(&uuid).await?;
        let exclude = Matcher::new(&policy.exclude);
        // checked now, so a refusal is the response; checked again once the server is
        // stopped, against what is then on disk
        if let Some(check) = &check {
            let diff = self
                .diff(&root, &uuid, &manifest, request.scope, &exclude, None)
                .await?;
            check(&diff)?;
        }

        let server_name = instance.name().await;
        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::Restore,
            format!("Restoring {server_name}"),
            ProgressionStartValue::Restore {
                instance_uuid: uuid.clone(),
            },
            caused_by.clone(),
        );
        job.begin();
        self.register(&uuid, &job);
        let task_id = job.task_id();
        let (this, state) = (self.clone(), state.clone());
        self.tracker.spawn(async move {
            let backup_id = manifest.id.clone();
            let result = this
                .restore_locked(
                    &state,
                    guards,
                    &instance,
                    manifest,
                    &request,
                    exclude,
                    check,
                    was_running,
                    &caused_by,
                    &job,
                )
                .await;
            let (success, message, safety) = match result {
                Ok((message, safety)) => (true, message, safety),
                Err(e) => (false, this.failure_message("The restore", &e), None),
            };
            job.end(
                success,
                &message,
                ProgressionEndValue::RestoreCompleted {
                    instance_uuid: uuid.clone(),
                    backup_id: Some(backup_id),
                    success,
                    message: message.clone(),
                },
            );
            this.unregister(&uuid);
            if let Some(safety) = safety {
                this.apply_retention(&uuid, Some(&safety)).await;
                this.request_gc();
            }
        });
        Ok(task_id)
    }

    /// What restoring `manifest` would change, against `current` (a manifest's files) or,
    /// without it, the files as they are now.
    async fn diff(
        &self,
        root: &Path,
        uuid: &InstanceUuid,
        manifest: &Manifest,
        scope: RestoreScope,
        exclude: &Matcher,
        current: Option<Vec<FileEntry>>,
    ) -> Result<RestoreDiff, Error> {
        let store = self.store.clone();
        let (root, uuid, manifest, exclude) = (
            root.to_path_buf(),
            uuid.clone(),
            manifest.clone(),
            exclude.clone(),
        );
        let token = self.cancel.clone();
        blocking(move || {
            let plan = restore::plan(&root, &manifest, scope, &exclude)?;
            let current = match current {
                Some(files) => files,
                None => store.hash_current(
                    ScanRequest {
                        server_root: &root,
                        uuid: &uuid,
                        kind: BackupKind::Manual,
                        exclude: &exclude,
                    },
                    &|| token.is_cancelled(),
                )?,
            };
            restore::diff(&root, &plan, &current, &exclude)
        })
        .await
    }

    /// The restore itself, in the background. Returns the end message and the safety
    /// backup's id.
    ///
    /// A server it stopped is started again when `start_after`: after the restore, and also
    /// when the restore failed with the server folder as it was (before the swap, or the
    /// swap was undone), since nothing then keeps it from running.
    #[allow(clippy::too_many_arguments)]
    async fn restore_locked(
        self: &Arc<Self>,
        state: &AppState,
        mut guards: JobGuards,
        instance: &MinecraftInstance,
        manifest: Manifest,
        request: &RestoreRequest,
        exclude: Matcher,
        check: Option<DiffCheck>,
        was_running: bool,
        caused_by: &CausedBy,
        job: &Job,
    ) -> Result<(String, Option<String>), Error> {
        let uuid = instance.uuid().await;
        let server_name = instance.name().await;
        if was_running {
            job.report(0.0, Some("Stopping the server"));
            instance.stop(caused_by.clone(), true).await?;
        }
        let swapped = self
            .swap_in(
                state,
                &mut guards,
                instance,
                manifest,
                request,
                exclude,
                check,
                was_running,
                job,
            )
            .await;
        let safety_id = match swapped {
            Ok(safety_id) => safety_id,
            Err(failure) => {
                let start_again = start_again_after(
                    &failure,
                    was_running,
                    request.start_after,
                    self.cancel.is_cancelled(),
                );
                let e = match failure {
                    RestoreFailure::Unchanged(e) | RestoreFailure::Broken(e) => e,
                };
                if !start_again {
                    return Err(e);
                }
                drop(guards);
                let message = match instance.start(caused_by.clone(), false).await {
                    Ok(()) => format!("{}. The server was started again", e.source),
                    Err(start) => format!(
                        "{}. The server couldn't start again: {}",
                        e.source, start.source
                    ),
                };
                return Err(Error {
                    kind: e.kind,
                    source: e.source.wrap_err(message),
                });
            }
        };
        self.invalidate(&uuid);
        if let Err(e) = instance.reload_from_disk().await {
            return Err(err(
                ErrorKind::Internal,
                format!(
                    "The backup was restored, but the server's settings couldn't be read again: {}",
                    e.source
                ),
            ));
        }
        drop(guards);

        let mut message = format!("Restored {server_name}");
        if was_running && request.start_after && !self.cancel.is_cancelled() {
            if let Err(e) = instance.start(caused_by.clone(), false).await {
                message.push_str(&format!(", but it couldn't start again: {}", e.source));
            }
        }
        Ok((message, safety_id))
    }

    /// [`Self::restore_locked`] from the stopped server to the swap. Returns the safety
    /// backup's id.
    #[allow(clippy::too_many_arguments)]
    async fn swap_in(
        self: &Arc<Self>,
        state: &AppState,
        guards: &mut JobGuards,
        instance: &MinecraftInstance,
        manifest: Manifest,
        request: &RestoreRequest,
        exclude: Matcher,
        check: Option<DiffCheck>,
        was_running: bool,
        job: &Job,
    ) -> Result<Option<String>, RestoreFailure> {
        let uuid = instance.uuid().await;
        let root = instance.path().await;
        if was_running {
            guards.stopped = Some(instance.lock_stopped("restoring a backup").await?);
        }
        let plan = {
            let (root, manifest, exclude) = (root.clone(), manifest.clone(), exclude.clone());
            let scope = request.scope;
            blocking(move || restore::plan(&root, &manifest, scope, &exclude)).await?
        };
        let needed = plan.total_bytes();
        {
            let root = root.clone();
            blocking(move || require_free_space(&root, needed, "to restore")).await?;
        }

        let safety_share = if request.safety_backup { 40.0 } else { 0.0 };
        let safety = if request.safety_backup {
            job.report(0.0, Some("Backing up the server as it is now"));
            let created = self
                .create_locked(
                    state,
                    guards,
                    instance,
                    BackupKind::BeforeRestore,
                    Some(safety_backup_name(&manifest)),
                    // the check below compares against these files, under this exclude
                    Some(exclude.clone()),
                    job.phase(0.0, safety_share),
                )
                .await?;
            if let Some(safety) = &created.manifest {
                let missing = not_in_safety_backup(&plan, safety);
                if let Some(first) = missing.first() {
                    let others = match missing.len() - 1 {
                        0 => String::new(),
                        1 => " and 1 other file".to_string(),
                        n => format!(" and {n} other files"),
                    };
                    return Err(conflict(format!(
                        "Couldn't back up {first}{others}, which the restore would replace, \
                         so it couldn't be undone. Check that Lodestone can read them, or \
                         restore without the safety backup"
                    ))
                    .into());
                }
            }
            created.manifest
        } else {
            None
        };
        let safety_id = safety.as_ref().map(|m| m.id.clone());

        // a server that was stopped all along was checked against these same files, under
        // `lock_stopped`; without a safety backup, don't hash it all again
        let check = check.filter(|_| was_running || safety.is_some());
        if let Some(check) = &check {
            job.report(safety_share, Some("Checking what the restore changes"));
            let current = safety.as_ref().map(|m| m.files.clone());
            let diff = self
                .diff(&root, &uuid, &manifest, plan.scope, &exclude, current)
                .await?;
            check(&diff)?;
        }
        if safety.is_some() {
            // the safety backup may have used the same disk
            let root = root.clone();
            blocking(move || require_free_space(&root, needed, "to restore")).await?;
        }

        job.report(safety_share, Some("Restoring files"));
        let config = instance.config_snapshot().await;
        let pin = Pin {
            name: Some(config.name),
            port: Some(config.port),
            java_cmd: Some(config.java_cmd),
            cmd_args: Some(config.cmd_args),
        };
        let staged = {
            let store = self.store.clone();
            let (root, manifest) = (root.clone(), manifest.clone());
            let phase = job.phase(safety_share, 90.0 - safety_share);
            let token = self.cancel.clone();
            blocking(move || {
                restore::stage(
                    &store,
                    &manifest,
                    &root,
                    &plan,
                    &exclude,
                    &pin,
                    &mut |done, total| {
                        let share = if total == 0 {
                            1.0
                        } else {
                            done as f64 / total as f64
                        };
                        phase.report(100.0 * share, None);
                    },
                    &|| token.is_cancelled(),
                )
            })
            .await?
        };
        if self.cancel.is_cancelled() {
            blocking(move || {
                staged.discard();
                Ok(())
            })
            .await?;
            return Err(cancelled().into());
        }

        // from here on the restore isn't cancellable: it is short, and half a swap is worse
        job.report(90.0, Some("Putting the files in place"));
        let journal = self.store.journal_path(&uuid)?;
        let joined = tokio::task::spawn_blocking(move || staged.swap(&journal)).await;
        match swap_outcome(joined) {
            Ok(()) => Ok(safety_id),
            Err(SwapOutcome::RolledBack(e)) => Err(RestoreFailure::Unchanged(e)),
            Err(SwapOutcome::Broken(e)) => {
                error!("The restore of {uuid} could not be undone: {}", e.source);
                // the journal stays for the repair at the next start; until then the server
                // stays stopped
                if let Some(stopped) = guards.stopped.take() {
                    self.broken
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(uuid.clone(), stopped);
                }
                Err(RestoreFailure::Broken(err(
                    ErrorKind::Internal,
                    "The restore could not be undone; restart Lodestone to repair it",
                )))
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // Restoring as a new server

    /// `POST /instance/{uuid}/backups/{id}/restore_as_new`, after the handler checked the
    /// permissions: writes every file of the backup into a new server folder in the
    /// background, then adds the server and gives the requester what creating a server
    /// gives. Reuses the instance creation events, so every client adds the server.
    pub async fn start_restore_as_new(
        self: &Arc<Self>,
        state: &AppState,
        manifest: Manifest,
        request: NewServerRequest,
    ) -> Result<String, Error> {
        crate::implementations::minecraft::validate_new_instance_name(&request.name)?;
        let new_uuid = InstanceUuid::default();
        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::Restore,
            format!("Restoring a backup as {}", request.name),
            ProgressionStartValue::InstanceCreation {
                instance_uuid: new_uuid.clone(),
            },
            request.caused_by.clone(),
        );
        job.begin();
        let task_id = job.task_id();
        let (this, state) = (self.clone(), state.clone());
        self.tracker.spawn(async move {
            let _store = this.store_read().await;
            let port = {
                let mut ports = state.port_manager.lock().await;
                let port = ports.next_free(25565);
                if let Some(port) = port {
                    // taken now, so two restores at once get different ports
                    ports.add_port(port);
                }
                port
            };
            let result = match port {
                Some(port) => {
                    this.restore_as_new_locked(&state, &manifest, &request, &new_uuid, port, &job)
                        .await
                }
                None => Err(conflict("No free port is left for the new server")),
            };
            match result {
                Ok(instance) => {
                    let uuid = new_uuid.clone();
                    if let Err(e) = state
                        .users_manager
                        .write()
                        .await
                        .modify_permissions(
                            &request.requester_uid,
                            |perm| {
                                perm.can_start_instance.insert(uuid.clone());
                                perm.can_stop_instance.insert(uuid.clone());
                                perm.can_view_instance.insert(uuid.clone());
                                perm.can_read_instance_file.insert(uuid.clone());
                                perm.can_write_instance_file.insert(uuid.clone());
                            },
                            CausedBy::System,
                        )
                        .await
                    {
                        // the server is still usable by the owner and admins
                        error!("Failed to update permissions: {:?}", e);
                    }
                    let info = instance.get_instance_info().await;
                    state.instances.insert(new_uuid.clone(), instance.into());
                    job.end(
                        true,
                        &format!("Restored the backup as {}", request.name),
                        ProgressionEndValue::InstanceCreation(info),
                    );
                }
                Err(e) => {
                    if let Some(port) = port {
                        state.port_manager.lock().await.deallocate(port);
                    }
                    let message = this.failure_message("Restoring the backup as a new server", &e);
                    // the creation's End without a value, as a failed creation sends
                    job.0.broadcaster.send(Event::new_progression_event_end(
                        job.0.event_id.clone(),
                        false,
                        Some(message),
                        None,
                    ));
                }
            }
        });
        Ok(task_id)
    }

    async fn restore_as_new_locked(
        &self,
        state: &AppState,
        manifest: &Manifest,
        request: &NewServerRequest,
        new_uuid: &InstanceUuid,
        port: u32,
        job: &Job,
    ) -> Result<MinecraftInstance, Error> {
        let instances_dir = self.instances_dir.clone();
        let total = manifest.total_bytes;
        {
            let dir = instances_dir.clone();
            blocking(move || require_free_space(&dir, total, "to restore")).await?;
        }
        let staging = restore::new_server_dir(&instances_dir);
        let pin = Pin {
            name: Some(request.name.clone()),
            port: Some(port),
            java_cmd: (!request.keep_command_line).then_some(None),
            cmd_args: (!request.keep_command_line).then(Vec::new),
        };
        let dot_config = DotLodestoneConfig::new(new_uuid.clone(), GameType::MinecraftJava);
        let final_dir =
            crate::handlers::instance::instance_setup_path(&instances_dir, &request.name, new_uuid);
        let written = {
            let store = self.store.clone();
            let (manifest, staging, final_dir) =
                (manifest.clone(), staging.clone(), final_dir.clone());
            let dot_config = dot_config.clone();
            let phase = job.phase(0.0, 95.0);
            let token = self.cancel.clone();
            blocking(move || {
                restore::stage_all(
                    &store,
                    &manifest,
                    &staging,
                    &mut |done, total| {
                        let share = if total == 0 {
                            1.0
                        } else {
                            done as f64 / total as f64
                        };
                        phase.report(100.0 * share, Some("Restoring files"));
                    },
                    &|| token.is_cancelled(),
                )?;
                let finish = || -> Result<(), Error> {
                    restore::pin_settings(&staging, &pin)?;
                    // last: until it is there, the folder isn't a server
                    let json = serde_json::to_vec_pretty(&dot_config)
                        .context("Failed to write .lodestone_config")?;
                    crate::persist::write_atomic_blocking(
                        &staging.join(".lodestone_config"),
                        &json,
                        None,
                    )?;
                    std::fs::rename(&staging, &final_dir)
                        .context("Failed to move the new server into place")?;
                    // the rename must reach the disk too, or after a power loss the next
                    // start finds the staging folder and deletes it as unfinished
                    // The files are flushed already (stage_all), so a failure here is only
                    // logged: deleting the folder now could leave a half-deleted server
                    // that loads at the next start.
                    if let Some(parent) = final_dir.parent() {
                        if let Err(e) = restore::sync_dir(parent) {
                            warn!("Failed to flush {}: {e}", parent.display());
                        }
                    }
                    Ok(())
                };
                if let Err(e) = finish() {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err(e);
                }
                Ok(())
            })
            .await
        };
        written?;
        job.report(95.0, Some("Loading the new server"));
        match MinecraftInstance::restore(
            final_dir.clone(),
            dot_config,
            state.event_broadcaster.clone(),
            state.code_execution.clone(),
        )
        .await
        {
            Ok(instance) => Ok(instance),
            Err(e) => {
                crate::handlers::instance::remove_failed_instance_dir(final_dir).await;
                Err(e)
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // Download

    /// Zips a backup into a new temporary folder in `tmp_dir`, from verified objects.
    /// Returns the zip, the folder that owns it and its size.
    ///
    /// Stops at the next object once `cancel` (a child of [`Self::cancel_token`]) is
    /// cancelled: the handler does that when the request is dropped.
    pub async fn zip(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        id: &str,
        tmp_dir: PathBuf,
        file_name: String,
        cancel: CancellationToken,
    ) -> Result<(PathBuf, tempfile::TempDir, u64), Error> {
        let _store = self.store_read().await;
        let manifest = self.load_manifest(uuid, id).await?;
        let store = self.store.clone();
        let token = cancel;
        blocking(move || {
            require_free_space(&tmp_dir, manifest.total_bytes, "to download the backup")?;
            std::fs::create_dir_all(&tmp_dir).context("Failed to create the temporary folder")?;
            let temp_dir =
                tempfile::tempdir_in(&tmp_dir).context("Failed to create a temporary folder")?;
            let out = temp_dir.path().join(&file_name);
            let file = std::fs::File::create(&out).context("Failed to create the zip")?;
            let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
            for entry in &manifest.files {
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .large_file(entry.size >= u32::MAX as u64)
                    .unix_permissions(if entry.executable { 0o755 } else { 0o644 });
                zip.start_file(entry.path.as_str(), options)
                    .context("Failed to write the zip")?;
                store
                    .read_object(&entry.sha256, entry.size, &mut zip, &|| {
                        token.is_cancelled()
                    })
                    .map_err(|e| e.into_error(&manifest.id, &entry.path))?;
            }
            let mut writer = zip.finish().context("Failed to write the zip")?;
            std::io::Write::flush(&mut writer).context("Failed to write the zip")?;
            drop(writer);
            let size = std::fs::metadata(&out)
                .context("Failed to read the zip")?
                .len();
            Ok((out, temp_dir, size))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backups::store::tests::{age, backup, meta, setup, uuid, write, NO_CANCEL};
    use crate::test_util::wait_until;

    /// Runs an unnamed backup of `root` the way [`BackupService::create_locked`] does,
    /// without a server, and saves its manifest when it made one.
    fn copy(store: &Store, root: &Path, id: &str, kind: BackupKind) -> Copied {
        let (broadcaster, _) = EventBroadcaster::new(8);
        let job = Job::new(
            broadcaster,
            JobKind::Backup,
            "Backing up".to_string(),
            ProgressionStartValue::Backup {
                instance_uuid: uuid(),
            },
            CausedBy::System,
        );
        let work = CopyWork {
            store: store.clone(),
            root: root.to_path_buf(),
            uuid: uuid(),
            kind,
            exclude: Matcher::new(&[]),
            skip_unchanged: true,
            meta: meta(id, kind),
            phase: job.phase(0.0, 100.0),
            prefetched: None,
        };
        let copied = work.run(true, NO_CANCEL).unwrap();
        if let Copied::Finished { manifest, .. } = &copied {
            store.save_manifest(manifest).unwrap();
        }
        copied
    }

    fn unchanged_id(copied: &Copied) -> Option<&str> {
        match copied {
            Copied::Unchanged { id, .. } => Some(id),
            Copied::Finished { .. } => None,
        }
    }

    /// "Back up now" (an unnamed manual backup) with nothing changed since a scheduled
    /// backup still makes a manual one, so retention can't delete the only copy of what the
    /// person asked to keep. After a manual or pinned backup, it is skipped as before.
    #[test]
    fn an_unnamed_manual_backup_is_skipped_only_after_one_that_is_kept() {
        let (_dir, store, root) = setup();
        write(&root, "world/level.dat", b"level");
        age(&root, "world/level.dat");
        let first = copy(&store, &root, "20260101-000000-aaaa", BackupKind::Scheduled);
        assert!(unchanged_id(&first).is_none());

        // nothing changed, but the newest backup is a scheduled one cleanup may delete
        let manual = copy(&store, &root, "20260101-000100-bbbb", BackupKind::Manual);
        assert!(unchanged_id(&manual).is_none());
        let Copied::Finished { manifest, .. } = &manual else {
            unreachable!()
        };
        assert_eq!(manifest.kind, BackupKind::Manual);

        // now the newest backup is manual: nothing more to keep
        let again = copy(&store, &root, "20260101-000200-cccc", BackupKind::Manual);
        assert_eq!(unchanged_id(&again), Some("20260101-000100-bbbb"));
        // and scheduled backups skip whatever the newest backup is
        let scheduled = copy(&store, &root, "20260101-000300-dddd", BackupKind::Scheduled);
        assert_eq!(unchanged_id(&scheduled), Some("20260101-000100-bbbb"));

        // a pinned scheduled backup is kept too
        write(&root, "world/level.dat", b"level 2");
        age(&root, "world/level.dat");
        let mut pinned = backup(
            &store,
            &root,
            "20260101-000400-eeee",
            BackupKind::Scheduled,
            &Matcher::new(&[]),
        );
        pinned.pinned = true;
        store.save_manifest(&pinned).unwrap();
        let after_pinned = copy(&store, &root, "20260101-000500-ffff", BackupKind::Manual);
        assert_eq!(unchanged_id(&after_pinned), Some("20260101-000400-eeee"));
    }

    /// Restoring a safety backup nests the names; each one still fits, and a short name is
    /// kept whole.
    #[test]
    fn safety_backup_names_fit() {
        let (_dir, store, root) = setup();
        write(&root, "world/level.dat", b"level");
        let mut manifest = backup(
            &store,
            &root,
            "20260101-000000-aaaa",
            BackupKind::Manual,
            &Matcher::new(&[]),
        );
        manifest.created = 1_767_225_600;
        assert_eq!(
            safety_backup_name(&manifest),
            "Before restoring “2026-01-01 00:00 UTC”"
        );
        for _ in 0..10 {
            let name = safety_backup_name(&manifest);
            assert!(name.chars().count() <= MAX_NAME_LEN, "{name}");
            assert_eq!(check_backup_name(&name), Ok(Some(name.clone())));
            manifest.name = Some(name);
        }
        let name = manifest.name.unwrap();
        assert!(
            name.starts_with("Before restoring “Before restoring “"),
            "{name}"
        );
        assert!(name.ends_with("…”"), "{name}");
    }

    fn service(dir: &Path) -> Arc<BackupService> {
        Arc::new(BackupService::new(
            dir.join("backups"),
            dir.join("instances"),
        ))
    }

    #[test]
    fn a_failed_restore_starts_the_server_again_only_when_nothing_changed() {
        let unchanged = RestoreFailure::Unchanged(err(ErrorKind::Conflict, "no space"));
        let broken = RestoreFailure::Broken(err(ErrorKind::Internal, "broken"));
        assert!(start_again_after(&unchanged, true, true, false));
        // the swap couldn't be undone: the server must stay down
        assert!(!start_again_after(&broken, true, true, false));
        // it wasn't running, or wasn't to start after, or Lodestone is shutting down
        assert!(!start_again_after(&unchanged, false, true, false));
        assert!(!start_again_after(&unchanged, true, false, false));
        assert!(!start_again_after(&unchanged, true, true, true));
        // `?` on an ordinary error means nothing changed yet
        let failure: RestoreFailure = err(ErrorKind::Internal, "x").into();
        assert!(matches!(failure, RestoreFailure::Unchanged(_)));
    }

    /// A swap whose task panicked may have stopped half way through its renames: it is
    /// broken (the journal stays and the server stays locked), never started again.
    #[tokio::test]
    async fn a_swap_that_panicked_is_broken() {
        let joined = tokio::task::spawn_blocking(|| -> Result<(), SwapError> {
            panic!("half way through the renames")
        })
        .await;
        assert!(joined.is_err());
        assert!(matches!(swap_outcome(joined), Err(SwapOutcome::Broken(_))));

        let rolled_back = Ok(Err(SwapError::RolledBack(err(ErrorKind::Internal, "x"))));
        assert!(matches!(
            swap_outcome(rolled_back),
            Err(SwapOutcome::RolledBack(_))
        ));
        let broken = Ok(Err(SwapError::Broken(err(ErrorKind::Internal, "x"))));
        assert!(matches!(swap_outcome(broken), Err(SwapOutcome::Broken(_))));
        assert!(swap_outcome(Ok(Ok(()))).is_ok());
    }

    /// A download (or anything else) holding the store when a backup is deleted: the GC
    /// runs once it lets go, without waiting for another backup.
    #[tokio::test]
    async fn a_gc_that_had_to_wait_runs_when_the_store_is_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let service = service(dir.path());
        let unused = service.store().object_path(&"ab".repeat(32));
        std::fs::create_dir_all(unused.parent().unwrap()).unwrap();
        std::fs::write(&unused, b"left by a deleted backup").unwrap();

        let held = service.store_read().await;
        service.request_gc();
        assert!(service.gc_pending.load(Ordering::SeqCst));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(unused.exists());
        drop(held);
        wait_until("the GC ran", || async { !unused.exists() }).await;
        assert!(!service.gc_pending.load(Ordering::SeqCst));
    }
}
