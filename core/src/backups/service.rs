//! [`BackupService`]: the locks, jobs and cancellation around backups, and the operations
//! the HTTP handlers and the scheduler call.
//!
//! A backup is a data image of the server in the core's image store, owned by
//! [`ImageService`], plus its record in `backups.json` (see [`super::store`]). A backup is
//! written in this order: the image's objects (the snapshot), its manifest (the store makes
//! the objects durable first), then the record, both under the instance's image lock. A
//! crash before the record leaves a manifest no record names, which is listed as a manual
//! backup again before a retention or a delete picks anything ([`reconcile_records`]): an
//! image holds a whole copy of the server and is never thrown away for lack of a record.
//! Deleting is the mirror image: the manifest first (once no other record names it), then
//! the record, then pruning the content images nothing links to any more, then a
//! collection. A crash between leaves a record whose image is gone, which the same pass
//! drops, so a deleted backup never comes back.
//!
//! Locks, outermost first:
//! - the server's job mutex: one backup or restore per server at a time (`try_lock`, so a
//!   second one is refused rather than queued);
//! - for a restore, the server's mods lock, like a modpack update;
//! - `lock_stopped` on the instance while a stopped server is backed up or restored, so it
//!   can't start meanwhile;
//! - for a restore that changes content, its content operation ([`ImageService::begin`]),
//!   so no content image is recorded from half-swapped files;
//! - the instance's image lock ([`ImageService::instance_lock`]), around every write of the
//!   records and the manifests.
//!
//! A job holds the image store's collections off ([`ImageStore::hold_off_collection`])
//! while it relies on objects nothing has claimed: a backup between its snapshot and its
//! save, a restore while it reads the image (whose backup may be deleted meanwhile), a
//! download while it zips. A collection then waits for it; it never runs halfway through.
//!
//! A job takes its guards once ([`JobGuards`]) and a restore's safety backup runs inside the
//! restore's guards ([`BackupService::create_locked`]): neither `lock_stopped` nor the job
//! mutex is re-entrant.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use color_eyre::eyre::{eyre, Context};
use dashmap::DashMap;
use lodestone_image::store::gc::CollectionHold;
use lodestone_image::store::{ObjectForm, Store as ImageStore};
use lodestone_image::{Image, Kind, WithheldReason};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{error, info, warn};

use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{
    CausedBy, Event, ProgressionEndValue, ProgressionEventID, ProgressionStartValue,
};
use crate::images::recorder::INITIAL_LABEL;
use crate::images::{blocking, ImageService, InstanceContext, Operation};
use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::{GameType, TConfigurable};
use crate::traits::t_server::{State, TServer};
use crate::traits::TInstance;
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::AppState;

use super::live::{with_saving_off, InstanceLines, Limits, LiveServer};
use super::restore::{self, Pin, RestoreDiff, RestorePlan, Source, SwapError};
use super::retention::{self, LastAttempt, RetentionItem};
use super::store::{cancelled, check_backup_name, err, new_backup_id, valid_backup_id, Store};
use super::take::{self, needs_full_scan, Meta, Prepared, Request};
use super::types::{
    BackupInfo, BackupKind, BackupList, BackupPolicy, BackupRecord, BackupUsage, JobKind, Records,
    RestoreRequest, RestoreScope, RunningJob, MAX_NAME_LEN,
};
use crate::util::disk::{free_space, require_free_space};

/// At most one manual backup per server in this time.
const MANUAL_BACKUP_INTERVAL: Duration = Duration::from_secs(60);
/// How long a restore waits for the Mods tab (or a modpack update) to let go of the server.
const MODS_LOCK_WAIT: Duration = Duration::from_secs(5);
/// How long a backup waits for the content gate to record the content changes queued before
/// it, so it links to the content the server has. Past this (a long modpack update holds the
/// content), it links to the content image recorded before. A record that got the gate runs
/// to its end, however long it takes.
const FLUSH_WAIT: Duration = Duration::from_secs(10);
/// Progress updates are sent at most this often, unless they move a whole percent.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

fn conflict(message: impl Into<String>) -> Error {
    err(ErrorKind::Conflict, message)
}

fn not_found() -> Error {
    err(ErrorKind::NotFound, "Backup not found")
}

/// Holds the image store's collections off (see the module), waiting for one that runs.
async fn hold_off_collection(store: Arc<ImageStore>) -> Result<CollectionHold, Error> {
    blocking(move || {
        store
            .hold_off_collection()
            .map_err(|e| err(ErrorKind::Internal, format!("{e}")))
    })
    .await
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
    /// The new backup's image; `None` when nothing had changed.
    pub image: Option<Image>,
    /// Files that couldn't be read.
    pub unreadable: usize,
    pub live: bool,
    pub quiesced: bool,
}

/// The number of files an image withheld because they couldn't be read.
fn unreadable_in(image: &Image) -> usize {
    image
        .withheld
        .iter()
        .filter(|w| {
            w.reason == WithheldReason::Unreadable || w.reason == WithheldReason::KeptChanging
        })
        .count()
}

/// Whether a backup of `kind` named `name` writes nothing when nothing changed since
/// `previous`, the newest backup: scheduled backups, and unnamed manual ones.
///
/// An unnamed manual backup is skipped only when `previous` is one retention never deletes
/// (manual or pinned). Otherwise the state the person asked to keep would live only in,
/// say, a scheduled backup that cleanup deletes hours later; a manual backup is written
/// instead, which costs only its record, since the image is the same.
fn skips_unchanged(kind: BackupKind, named: bool, previous: &BackupRecord) -> bool {
    match kind {
        BackupKind::Scheduled => true,
        BackupKind::Manual => !named && retention::never_deleted(previous.kind, previous.pinned),
        BackupKind::BeforeRestore => false,
    }
}

/// The label a backup's image gets: its name, or what kind of backup it is.
fn image_label(kind: BackupKind, name: Option<&str>) -> String {
    match (name, kind) {
        (Some(name), _) => name.to_string(),
        (None, BackupKind::Manual) => "Backup".to_string(),
        (None, BackupKind::Scheduled) => "Scheduled backup".to_string(),
        (None, BackupKind::BeforeRestore) => "Before a restore".to_string(),
    }
}

/// A backup as a restore or a download reads it: its record, its data image, and the
/// content image the data image links to, if it is still there.
#[derive(Debug, Clone)]
pub struct Backup {
    pub record: BackupRecord,
    pub image: Image,
    pub content: Option<Image>,
}

impl Backup {
    fn source(&self) -> Source<'_> {
        Source {
            image: &self.image,
            level: self.record.level.as_deref(),
            content: self.content.as_ref(),
        }
    }

    /// Whether the backup's server splits its level the Paper way, from the image.
    fn splits_worlds(&self) -> bool {
        self.image
            .settings
            .server
            .as_ref()
            .and_then(|s| s.flavour.as_ref())
            .is_some_and(|f| f.splits_worlds())
    }

    /// Every file of the backup, where a new server gets it ([`restore::all_files`]).
    pub fn files_on_disk(&self) -> Result<Vec<restore::Target>, Error> {
        let layout = restore::image_layout(
            &self.image,
            self.record.level.as_deref(),
            self.splits_worlds(),
        )?;
        restore::all_files(&self.image, &layout)
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

/// The name of the safety backup a restore of `record` takes first: "Before restoring
/// “…”" with the backup's name, or its date when it has none. The name is shortened to fit
/// [`MAX_NAME_LEN`], since the wrapper adds 19 characters to a name that may already be that
/// long, and restoring a safety backup nests them; a name that didn't fit would only be
/// refused after every file was read.
fn safety_backup_name(record: &BackupRecord) -> String {
    let wrap = |label: &str| format!("Before restoring “{label}”");
    let label = backup_label(record);
    let room = MAX_NAME_LEN - wrap("").chars().count();
    if label.chars().count() <= room {
        return wrap(&label);
    }
    let cut: String = label.chars().take(room - 1).collect();
    wrap(&format!("{}…", cut.trim_end()))
}

/// A backup as messages name it: its name, or its date.
fn backup_label(record: &BackupRecord) -> String {
    record.name.clone().unwrap_or_else(|| {
        chrono::DateTime::from_timestamp(record.created, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| record.id.clone())
    })
}

/// The files a restore with `plan` replaces that its safety backup's image `safety`
/// couldn't read, so has no copy of. The restore would delete them for good, so it is
/// refused instead.
fn not_in_safety_backup<'a>(plan: &RestorePlan, safety: &'a Image) -> Vec<&'a str> {
    safety
        .withheld
        .iter()
        .filter(|w| {
            w.reason == WithheldReason::Unreadable || w.reason == WithheldReason::KeptChanging
        })
        .filter(|w| plan.replaces(&w.path))
        .map(|w| w.path.as_str())
        .collect()
}

/// The backup's copy of the server's settings (`.lodestone_minecraft_config.json`), as
/// JSON: `None` when the backup has none. Blocking.
fn backup_settings(
    store: &ImageStore,
    backup: &Backup,
) -> Result<Option<serde_json::Value>, Error> {
    /// A settings file is a few KiB; anything this large is damaged.
    const MAX_CONFIG_BYTES: u64 = 4 << 20;
    let files = backup.files_on_disk()?;
    let Some(config) = files.iter().find(|f| f.disk == restore::CONFIG_FILE) else {
        return Ok(None);
    };
    if config.entry.size > MAX_CONFIG_BYTES {
        return Err(eyre!("its settings file is too large").into());
    }
    let bytes = store
        .read_object_bytes(&config.entry.sha256, config.entry.size)
        .map_err(|e| eyre!("Failed to read its settings: {e}"))?;
    Ok(Some(
        serde_json::from_slice(&bytes).context("Its settings file is damaged")?,
    ))
}

/// The stored size of an object (its whole file, or its chunk list and chunks): what a
/// backup's files take on disk. 0 when it isn't there.
fn stored_size(store: &ImageStore, sha256: &str, size: u64) -> u64 {
    let file_len = |path: Result<PathBuf, _>| {
        path.ok()
            .and_then(|p| std::fs::metadata(p).ok())
            .map_or(0, |m| m.len())
    };
    match store.object_form(sha256, size) {
        Ok(ObjectForm::Whole) => file_len(store.object_path(sha256)),
        Ok(ObjectForm::Chunked(list)) => {
            let mut seen = HashSet::new();
            list.chunks
                .iter()
                .filter(|chunk| seen.insert(chunk.sha256.as_str()))
                .map(|chunk| file_len(store.chunk_path(&chunk.sha256)))
                .fold(file_len(store.list_path(sha256)), u64::saturating_add)
        }
        Err(_) => 0,
    }
}

/// Brings the records in line with the images, both ways. A record whose image is gone is
/// dropped: what a crash in the middle of a delete leaves (the manifest goes first). A data
/// or full image no record names gets a record, so it is a backup again: what a crash
/// between a backup's manifest and its record leaves, or every backup when `backups.json`
/// went missing. An image holds a whole copy of the server, so it is never deleted just
/// because the list doesn't name it. The record is a manual backup (retention never deletes
/// those) named by the image's label, at the image's time, with no server name (whoever
/// needs one takes the server's current name), with its world as the level when it has
/// exactly one. Returns whether it
/// changed anything; the caller saves the records. A manifest that can't be read is left as
/// it is, and nothing changes when the images can't be listed. Blocking; the caller holds
/// the instance's lock.
fn reconcile_records(store: &ImageStore, uuid: &InstanceUuid, records: &mut Records) -> bool {
    let Ok(ids) = store.manifest_ids(uuid.as_ref()) else {
        return false;
    };
    let present: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let before = records.backups.len();
    records.backups.retain(|record| {
        let there = present.contains(record.image.as_str());
        if !there {
            info!(
                "Dropped the backup {} of {uuid}, whose image is gone",
                record.id
            );
        }
        there
    });
    let mut changed = records.backups.len() != before;
    for id in &ids {
        let id = id.as_str();
        if records.names_image(id) {
            continue;
        }
        match store.load_header(uuid.as_ref(), id) {
            Ok(header) if header.kind != Kind::Content => {}
            _ => continue,
        }
        let image = match store.load_manifest(uuid.as_ref(), id) {
            Ok(image) => image,
            Err(e) => {
                warn!("The image {id} of {uuid} names no backup and can't be read: {e}");
                continue;
            }
        };
        let created = chrono::DateTime::from_timestamp(image.created, 0).unwrap_or_default();
        let name = image.label.as_deref().and_then(|label| {
            let clean: String = label
                .chars()
                .filter(|c| !c.is_control())
                .take(MAX_NAME_LEN)
                .collect();
            check_backup_name(&clean).ok().flatten()
        });
        let level = match image.worlds.as_slice() {
            [world] => Some(world.name.clone()),
            _ => None,
        };
        records.backups.push(BackupRecord {
            id: new_backup_id(created),
            image: image.id.clone(),
            name,
            kind: BackupKind::Manual,
            created: image.created,
            pinned: false,
            live: false,
            quiesced: image.worlds.iter().all(|w| w.quiesced != Some(false)),
            level,
            instance_name: String::new(),
            game_version: image.game.minecraft.clone(),
            file_count: image.entries.len() as u64,
            withheld_count: image.withheld.len() as u64,
            total_bytes: image
                .entries
                .iter()
                .map(|e| e.size)
                .fold(0, u64::saturating_add),
            new_bytes: 0,
            stored_bytes: 0,
            duration_ms: 0,
        });
        info!("Listed the image {id} of {uuid}, which no backup named, as a backup");
        changed = true;
    }
    changed
}

/// The objects a server's backups use, by the images they were worked out from, for the
/// usage the list shows.
struct UsedObjects {
    images: Vec<String>,
    /// SHA-256 to size.
    objects: Arc<HashMap<String, u64>>,
}

/// Server backups: the jobs, their locks and the scheduler's state.
pub struct BackupService {
    images: Arc<ImageService>,
    store: Store,
    instances_dir: PathBuf,
    jobs: StdMutex<HashMap<InstanceUuid, Arc<Mutex<()>>>>,
    running: StdMutex<HashMap<InstanceUuid, Job>>,
    last_manual: StdMutex<HashMap<InstanceUuid, Instant>>,
    schedule: StdMutex<HashMap<InstanceUuid, ScheduleState>>,
    /// Servers whose restore couldn't be undone, found by [`Self::recover`], until
    /// [`Self::lock_broken_servers`] locks them.
    unrepaired: StdMutex<Vec<InstanceUuid>>,
    /// The `lock_stopped` guards of servers whose restore couldn't be undone: they can't
    /// start until the core restarts and repairs them.
    broken: StdMutex<HashMap<InstanceUuid, OwnedMutexGuard<()>>>,
    used: StdMutex<HashMap<InstanceUuid, UsedObjects>>,
    /// Stored sizes of objects, which never change for an object.
    object_sizes: StdMutex<HashMap<String, u64>>,
    cancel: CancellationToken,
    tracker: TaskTracker,
    /// Notified when a schedule is saved, for the scheduler while no server has one.
    schedule_changed: tokio::sync::Notify,
}

impl BackupService {
    /// A service for the servers in `instances_dir`, keeping their backups in the image
    /// store of `images`. Nothing is read until it is used.
    pub fn new(images: Arc<ImageService>, instances_dir: PathBuf) -> BackupService {
        BackupService {
            store: Store::new(images.root().to_path_buf()),
            images,
            instances_dir,
            jobs: Default::default(),
            running: Default::default(),
            last_manual: Default::default(),
            schedule: Default::default(),
            unrepaired: Default::default(),
            broken: Default::default(),
            used: Default::default(),
            object_sizes: Default::default(),
            cancel: CancellationToken::new(),
            tracker: TaskTracker::new(),
            schedule_changed: tokio::sync::Notify::new(),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    fn image_store(&self) -> Arc<ImageStore> {
        self.images.store()
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
    /// Whether a backup is being restored into the server (in place) now.
    pub fn restoring(&self, uuid: &InstanceUuid) -> bool {
        self.running_job(uuid)
            .is_some_and(|job| job.kind == JobKind::Restore)
    }

    pub fn job_running(&self, uuid: &InstanceUuid) -> bool {
        lock_in(&self.jobs, uuid).try_lock().is_err()
    }

    fn lock_job(&self, uuid: &InstanceUuid) -> Result<OwnedMutexGuard<()>, Error> {
        self.try_lock_job(uuid)
            .ok_or_else(|| conflict("A backup or restore of this server is already running"))
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
    // Reading

    /// The server's backups.
    pub async fn records(&self, uuid: &InstanceUuid) -> Result<Records, Error> {
        let store = self.store.clone();
        let uuid = uuid.clone();
        blocking(move || store.load_records(&uuid)).await
    }

    /// The objects the server's backups use, worked out again only when its backups name
    /// other images than last time.
    async fn used_objects(
        &self,
        uuid: &InstanceUuid,
        records: &Records,
    ) -> Result<Arc<HashMap<String, u64>>, Error> {
        let mut images: Vec<String> = records.backups.iter().map(|b| b.image.clone()).collect();
        images.sort();
        images.dedup();
        if let Some(used) = self
            .used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .filter(|used| used.images == images)
        {
            return Ok(used.objects.clone());
        }
        let store = self.image_store();
        let (id, wanted) = (uuid.clone(), images.clone());
        let objects = blocking(move || {
            let mut objects = HashMap::new();
            for image in &wanted {
                match store.load_manifest(id.as_ref(), image) {
                    Ok(image) => objects.extend(
                        image
                            .entries
                            .into_iter()
                            .map(|entry| (entry.sha256, entry.size)),
                    ),
                    Err(e) => warn!("Failed to read the backup image {image} of {id}: {e}"),
                }
            }
            Ok(Arc::new(objects))
        })
        .await?;
        self.used.lock().unwrap_or_else(|e| e.into_inner()).insert(
            uuid.clone(),
            UsedObjects {
                images,
                objects: objects.clone(),
            },
        );
        Ok(objects)
    }

    async fn usage(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        records: &Records,
    ) -> Result<BackupUsage, Error> {
        let objects = self.used_objects(uuid, records).await?;
        let total_bytes = records
            .backups
            .iter()
            .map(|b| b.total_bytes)
            .fold(0, u64::saturating_add);
        let this = self.clone();
        blocking(move || {
            let store = this.image_store();
            let mut stored_bytes = 0u64;
            let mut found = Vec::new();
            {
                let sizes = this.object_sizes.lock().unwrap_or_else(|e| e.into_inner());
                for (sha256, size) in objects.iter() {
                    match sizes.get(sha256) {
                        Some(stored) => stored_bytes = stored_bytes.saturating_add(*stored),
                        None => found.push((sha256.clone(), *size)),
                    }
                }
            }
            let found: Vec<(String, u64)> = found
                .into_iter()
                .map(|(sha256, size)| {
                    let stored = stored_size(&store, &sha256, size);
                    (sha256, stored)
                })
                .collect();
            stored_bytes = found
                .iter()
                .map(|(_, stored)| *stored)
                .fold(stored_bytes, u64::saturating_add);
            let mut sizes = this.object_sizes.lock().unwrap_or_else(|e| e.into_inner());
            // a bound, not an eviction policy: it is only a cache
            if sizes.len() > 1_000_000 {
                sizes.clear();
            }
            sizes.extend(found.into_iter().filter(|(_, stored)| *stored > 0));
            drop(sizes);
            Ok(BackupUsage {
                total_bytes,
                stored_bytes,
                disk_free: free_space(this.store.root()),
            })
        })
        .await
    }

    /// `GET /instance/{uuid}/backups`.
    pub async fn list(self: &Arc<Self>, uuid: &InstanceUuid) -> Result<BackupList, Error> {
        self.store.instance_dir(uuid)?;
        let records = self.records(uuid).await?;
        let policy = self.policy(uuid).await?;
        let usage = self.usage(uuid, &records).await?;
        let last_attempt = self.schedule_state(uuid).last_attempt;
        let next_scheduled = retention::next_due(
            &policy,
            newest_for_schedule(&records),
            last_attempt,
            Utc::now().timestamp(),
        );
        Ok(BackupList {
            backups: records
                .newest_first()
                .into_iter()
                .map(BackupInfo::from)
                .collect(),
            policy,
            running: self.running_job(uuid),
            usage,
            next_scheduled,
        })
    }

    /// A backup with its images: NotFound when there is no such backup, or its image is
    /// gone.
    pub async fn load(&self, uuid: &InstanceUuid, id: &str) -> Result<Backup, Error> {
        if !valid_backup_id(id) {
            return Err(not_found());
        }
        let records = self.records(uuid).await?;
        let record = records.get(id).cloned().ok_or_else(not_found)?;
        let store = self.image_store();
        let uuid = uuid.clone();
        blocking(move || {
            let image = store
                .load_manifest(uuid.as_ref(), &record.image)
                .map_err(|e| {
                    err(
                        ErrorKind::NotFound,
                        format!("The backup's image can't be read: {e}"),
                    )
                })?;
            let content = image
                .content
                .as_deref()
                .and_then(|content| store.load_manifest(uuid.as_ref(), content).ok());
            Ok(Backup {
                record,
                image,
                content,
            })
        })
        .await
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
        let store = self.store.clone();
        let (id, saved) = (uuid.clone(), policy.clone());
        blocking(move || store.save_policy(&id, &saved)).await?;
        self.schedule_changed.notify_one();
        Ok(policy)
    }

    /// Changes the server's records under its image lock: `change` edits them and returns
    /// what to answer; they are saved when it succeeds.
    async fn change_records<T: Send + 'static>(
        &self,
        uuid: &InstanceUuid,
        change: impl FnOnce(&mut Records) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let _lock = self.images.instance_lock(uuid).await;
        let store = self.store.clone();
        let uuid = uuid.clone();
        blocking(move || {
            let mut records = store.load_records(&uuid)?;
            let answer = change(&mut records)?;
            store.save_records(&uuid, &records)?;
            Ok(answer)
        })
        .await
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
        if !valid_backup_id(id) {
            return Err(not_found());
        }
        let id = id.to_string();
        self.change_records(uuid, move |records| {
            let record = records
                .backups
                .iter_mut()
                .find(|b| b.id == id)
                .ok_or_else(not_found)?;
            if let Some(name) = name {
                record.name = name;
            }
            if let Some(pinned) = pinned {
                record.pinned = pinned;
            }
            Ok(BackupInfo::from(&*record))
        })
        .await
    }

    /// Deletes the backups of the server `select` picks from its records (read under the
    /// instance's image lock, so a backup pinned meanwhile is never picked from an older
    /// list, and reconciled with the images first, [`reconcile_records`]): each of their
    /// images no other record names, then their records, then prunes the content images
    /// nothing links to and asks for a collection. A backup whose image couldn't be deleted
    /// keeps its record and is listed as before. Returns the ids it deleted, and the first
    /// failure.
    async fn delete_backups(
        &self,
        uuid: &InstanceUuid,
        select: impl FnOnce(&Records) -> HashSet<String> + Send + 'static,
    ) -> Result<(Vec<String>, Option<Error>), Error> {
        let deleted = {
            let _lock = self.images.instance_lock(uuid).await;
            let store = self.store.clone();
            let images = self.image_store();
            let uuid = uuid.clone();
            blocking(move || {
                let mut records = store.load_records(&uuid)?;
                // Before anything is chosen: a missing list must never make every backup
                // look like a leftover.
                let reconciled = reconcile_records(&images, &uuid, &mut records);
                let ids = select(&records);
                let mut deleted = HashSet::new();
                let mut failure = None;
                for record in records.backups.iter().filter(|b| ids.contains(&b.id)) {
                    let shared = records
                        .backups
                        .iter()
                        .any(|b| b.image == record.image && !ids.contains(&b.id));
                    if !shared {
                        if let Err(e) = images.delete_manifest(uuid.as_ref(), &record.image) {
                            warn!("Failed to delete the image {} of {uuid}: {e}", record.image);
                            failure.get_or_insert_with(|| {
                                Error::from(eyre!("Failed to delete the backup {}: {e}", record.id))
                            });
                            continue;
                        }
                    }
                    deleted.insert(record.id.clone());
                }
                records.backups.retain(|b| !deleted.contains(&b.id));
                if reconciled || !deleted.is_empty() {
                    store.save_records(&uuid, &records)?;
                }
                Ok((deleted.into_iter().collect::<Vec<_>>(), failure))
            })
            .await?
        };
        match self.images.prune(uuid).await {
            Ok(_) => {}
            Err(e) => warn!("Failed to prune the images of {uuid}: {}", e.source),
        }
        self.images.request_gc();
        Ok(deleted)
    }

    /// `DELETE /instance/{uuid}/backups/{id}`: removes the backup; its files go at the next
    /// collection, unless another backup uses them.
    pub async fn delete(&self, uuid: &InstanceUuid, id: &str) -> Result<(), Error> {
        if !valid_backup_id(id) {
            return Err(not_found());
        }
        let id = id.to_string();
        let (deleted, failure) = self
            .delete_backups(uuid, move |_| HashSet::from([id]))
            .await?;
        if let Some(failure) = failure {
            return Err(failure);
        }
        if deleted.is_empty() {
            return Err(not_found());
        }
        Ok(())
    }

    /// When a server is deleted: deletes all its backups, its policy and its other images
    /// (the server's image folder). The caller holds the server's job mutex.
    pub async fn delete_server(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        self.store.instance_dir(uuid)?;
        self.images.delete_instance(uuid).await?;
        self.schedule
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
        self.used
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
        self.images.request_gc();
        Ok(())
    }

    // -----------------------------------------------------------------------------------
    // Retention

    /// Deletes the backups the server's retention doesn't keep. `just_created` is never
    /// deleted.
    async fn apply_retention(&self, uuid: &InstanceUuid, just_created: Option<&str>) {
        let result = async {
            let policy = self.policy(uuid).await?;
            let now = Utc::now().timestamp();
            let just_created = just_created.map(str::to_string);
            let doomed = move |records: &Records| -> HashSet<String> {
                let items: Vec<RetentionItem> = records
                    .backups
                    .iter()
                    .map(|b| RetentionItem {
                        id: &b.id,
                        kind: b.kind,
                        created: b.created,
                        pinned: b.pinned,
                    })
                    .collect();
                retention::to_delete(&items, now, &policy, just_created.as_deref())
                    .into_iter()
                    .collect()
            };
            let (deleted, failure) = self.delete_backups(uuid, doomed).await?;
            for id in deleted {
                info!("Deleted old backup {id} of {uuid}");
            }
            failure.map_or(Ok(()), Err)
        }
        .await;
        if let Err(e) = result {
            warn!(
                "Failed to apply the backup retention of {uuid}: {}",
                e.source
            );
        }
    }

    // -----------------------------------------------------------------------------------
    // Taking a backup

    /// Brings the server's content image up to date, so a backup links to the content it
    /// has. For a stopped server that is the check before a start (what is queued, then what
    /// changed outside Lodestone; see `MinecraftInstance::check_content_before_start`), and
    /// its first content image when it has none. A running server's own writes never make a
    /// content image, so for one only what is queued is recorded. Waits at most about
    /// [`FLUSH_WAIT`] for an operation in progress; a failure is only logged, since the backup
    /// has every file anyway.
    async fn record_content(&self, instance: &MinecraftInstance, policy_exclude: &[String]) {
        let ctx = InstanceContext::of(instance, policy_exclude.to_vec()).await;
        let uuid = ctx.uuid.clone();
        // The waits bound only getting the content gate: a record that has it runs to its
        // end, so it never loses the labels it took.
        let recorded = if ctx.running {
            self.images
                .flush_within(ctx, Some(FLUSH_WAIT))
                .await
                .map(|_| ())
        } else {
            // logs its own failures, and waits for an operation in progress a while
            instance.check_content_before_start(&self.images).await;
            if self.images.current_content(&uuid).await.is_none() {
                self.images
                    .record_now_within(ctx, INITIAL_LABEL.to_string(), Some(FLUSH_WAIT))
                    .await
                    .map(|_| ())
            } else {
                Ok(())
            }
        };
        if let Err(e) = recorded {
            warn!(
                "Backing up {uuid} without recording its content first: {}",
                e.source
            );
        }
    }

    /// Takes a backup inside `guards`, with collections held off by `hold`: live (saving
    /// paused) when they don't hold `lock_stopped`.
    ///
    /// `policy_exclude` is what to leave out; `None` reads the server's policy. A restore's
    /// safety backup passes the patterns its plan used, so the safety backup has every file
    /// the restore replaces.
    #[allow(clippy::too_many_arguments)]
    async fn create_locked(
        &self,
        state: &AppState,
        guards: &JobGuards,
        _hold: &CollectionHold,
        instance: &MinecraftInstance,
        kind: BackupKind,
        name: Option<String>,
        policy_exclude: Option<Vec<String>>,
        phase: Phase,
    ) -> Result<Created, Error> {
        let uuid = instance.uuid().await;
        let instance_name = instance.name().await;
        let policy_exclude = match policy_exclude {
            Some(exclude) => exclude,
            None => self.policy(&uuid).await?.exclude,
        };
        let ctx = InstanceContext::of(instance, policy_exclude).await;
        let live = guards.stopped.is_none();
        let records = self.records(&uuid).await?;
        let full_scan = needs_full_scan(
            kind == BackupKind::Scheduled,
            records.backups.is_empty(),
            records.scheduled_since_full,
        );
        let request = Request {
            uuid: uuid.clone(),
            root: ctx.root.clone(),
            game_version: ctx.game_version.clone(),
            flavour: ctx.flavour.clone(),
            level_name: ctx.level_name.clone(),
            policy_exclude: ctx.policy_exclude.clone(),
            full_scan,
            threads: if live { 1 } else { 0 },
        };
        let store = self.image_store();
        let prepared = {
            let store = store.clone();
            blocking(move || Prepared::new(&store, request)).await?
        };

        // Walk 5 %, read 90 %, save 5 %. Started (for a scheduled backup) once there is
        // something to read.
        let take = |prepared: Prepared, phase: Phase, token: CancellationToken| {
            let store = store.clone();
            blocking(move || {
                phase.report(0.0, Some("Looking for changed files"));
                let taken = prepared.take(
                    &store,
                    &|to_read| {
                        if to_read > 0 {
                            phase.begin();
                        }
                        phase.report(5.0, Some("Copying files"));
                    },
                    &|done, total| {
                        let share = if total == 0 {
                            1.0
                        } else {
                            done as f64 / total as f64
                        };
                        phase.report(5.0 + 90.0 * share, None);
                    },
                    &|| token.is_cancelled(),
                )?;
                Ok((prepared, taken))
            })
        };
        let ((prepared, taken), quiesced) = if live {
            let console = |message: &str| {
                state.event_broadcaster.send(Event::new_system_message(
                    uuid.clone(),
                    instance_name.clone(),
                    message.to_string(),
                ))
            };
            console("Backing up…");
            let token = self.cancel.clone();
            let reading = phase.job.phase(phase.base, phase.span * 0.7);
            let rest = phase
                .job
                .phase(phase.base + phase.span * 0.7, phase.span * 0.3);
            let prefetched = {
                let (store, reading) = (store.clone(), reading.clone());
                let mut prepared = prepared;
                blocking(move || {
                    let read_ahead = prepared.prefetch(
                        &store,
                        &|done, total| {
                            reading.begin();
                            reading.report(
                                100.0 * done as f64 / total.max(1) as f64,
                                Some("Copying files"),
                            );
                        },
                        &|| token.is_cancelled(),
                    )?;
                    Ok((prepared, read_ahead))
                })
                .await
            };
            let result = match prefetched {
                Ok((prepared, read_ahead)) => {
                    let phase = if read_ahead { rest } else { phase.clone() };
                    let mut lines =
                        InstanceLines::new(state.event_broadcaster.subscribe(), uuid.clone());
                    let marker = self.store.marker_path(&uuid)?;
                    with_saving_off(
                        instance,
                        &mut lines,
                        &marker,
                        Limits::default(),
                        &self.cancel,
                        |_, token| take(prepared, phase, token),
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
            (
                take(prepared, phase.clone(), self.cancel.clone()).await?,
                true,
            )
        };

        phase.report(95.0, Some("Saving the backup"));
        let saved = self
            .save(
                &uuid,
                &instance_name,
                kind,
                name,
                live,
                quiesced,
                &prepared,
                &taken,
            )
            .await?;
        // the stat cache only makes the next backup faster
        let cache = taken.snapshot.cache;
        let id = uuid.clone();
        if let Err(e) = blocking(move || {
            store
                .save_stat_cache(id.as_ref(), &cache)
                .map_err(|e| err(ErrorKind::Internal, format!("{e}")))
        })
        .await
        {
            warn!(
                "Failed to save the backup stat cache of {uuid}: {}",
                e.source
            );
        }
        self.mark_backed_up(&uuid);
        if saved.image.is_some() {
            phase.begin();
        }
        phase.report(100.0, None);
        Ok(Created {
            unreadable: saved.image.as_ref().map_or(0, unreadable_in),
            id: saved.id,
            image: saved.image,
            live,
            quiesced,
        })
    }

    /// Builds the backup's image and saves it and its record, under the instance's image
    /// lock (so the content image it links to can't be pruned in between); or, when nothing
    /// changed and the backup skips that ([`skips_unchanged`]), saves nothing but the count
    /// of scheduled backups since a full scan.
    #[allow(clippy::too_many_arguments)]
    async fn save(
        &self,
        uuid: &InstanceUuid,
        instance_name: &str,
        kind: BackupKind,
        name: Option<String>,
        live: bool,
        quiesced: bool,
        prepared: &Prepared,
        taken: &take::Taken,
    ) -> Result<Saved, Error> {
        let _lock = self.images.instance_lock(uuid).await;
        let content = self.images.current_content(uuid).await;
        let records = self.records(uuid).await?;
        let previous = records.newest_first().first().map(|r| (*r).clone());
        let previous_image = match &previous {
            Some(previous) => {
                let store = self.image_store();
                let (id, image) = (uuid.clone(), previous.image.clone());
                blocking(move || Ok(store.load_manifest(id.as_ref(), &image).ok())).await?
            }
            None => None,
        };
        let now = Utc::now();
        let image = take::build(
            prepared,
            taken,
            Meta {
                label: image_label(kind, name.as_deref()),
                created: now.timestamp(),
                quiesced,
                content,
                parent: previous.as_ref().map(|p| p.image.clone()),
            },
        )?;
        let full_scan = prepared.request.full_scan;
        let scheduled_since_full = |saved: bool, records: &Records| {
            if full_scan {
                0
            } else if saved && kind == BackupKind::Scheduled {
                records.scheduled_since_full + 1
            } else {
                records.scheduled_since_full
            }
        };
        if let Some(previous) = previous
            .as_ref()
            .filter(|p| p.image == image.id && skips_unchanged(kind, name.is_some(), p))
        {
            let mut records = records.clone();
            let count = scheduled_since_full(false, &records);
            if count != records.scheduled_since_full {
                records.scheduled_since_full = count;
                let store = self.store.clone();
                let id = uuid.clone();
                // only makes the next full scan come early or late
                if let Err(e) = blocking(move || store.save_records(&id, &records)).await {
                    warn!("Failed to record the backup scans of {uuid}: {}", e.source);
                }
            }
            return Ok(Saved {
                id: previous.id.clone(),
                image: None,
            });
        }
        let record = BackupRecord {
            id: new_backup_id(now),
            image: image.id.clone(),
            name,
            kind,
            created: now.timestamp(),
            pinned: false,
            live,
            quiesced,
            level: prepared.level_in(&image),
            instance_name: instance_name.to_string(),
            game_version: prepared.request.game_version.clone(),
            file_count: image.entries.len() as u64,
            withheld_count: image.withheld.len() as u64,
            total_bytes: image
                .entries
                .iter()
                .map(|e| e.size)
                .fold(0, u64::saturating_add),
            new_bytes: take::new_bytes(&image, previous_image.as_ref()),
            stored_bytes: taken.snapshot.bytes_added,
            duration_ms: taken.duration_ms,
        };
        let store = self.store.clone();
        let images = self.image_store();
        let (id, to_save) = (uuid.clone(), image.clone());
        let mut records = records;
        records.scheduled_since_full = scheduled_since_full(true, &records);
        records.backups.push(record.clone());
        blocking(move || {
            images.save_manifest(id.as_ref(), &to_save).map_err(|e| {
                err(
                    ErrorKind::Internal,
                    format!("Failed to save the backup's image: {e}"),
                )
            })?;
            store.save_records(&id, &records)
        })
        .await?;
        Ok(Saved {
            id: record.id,
            image: Some(image),
        })
    }

    /// The end message of a backup.
    fn backup_message(server_name: &str, created: &Created) -> String {
        if created.image.is_none() {
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
        self.store.instance_dir(&uuid)?;
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
        let guards = self.take_guards(&instance, "backing it up").await?;
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
            let result = this
                .backup_now(&state, &guards, &instance, BackupKind::Manual, name, &job)
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

    /// A manual or scheduled backup inside `guards`: brings the content image up to date
    /// ([`Self::record_content`]), holds collections off, and backs up.
    async fn backup_now(
        &self,
        state: &AppState,
        guards: &JobGuards,
        instance: &MinecraftInstance,
        kind: BackupKind,
        name: Option<String>,
        job: &Job,
    ) -> Result<Created, Error> {
        let uuid = instance.uuid().await;
        let policy = self.policy(&uuid).await?;
        self.record_content(instance, &policy.exclude).await;
        let hold = hold_off_collection(self.image_store()).await?;
        self.create_locked(
            state,
            guards,
            &hold,
            instance,
            kind,
            name,
            Some(policy.exclude),
            job.phase(0.0, 100.0),
        )
        .await
    }

    /// A scheduled backup, run to the end. Sends no events when nothing changed. Then
    /// applies retention.
    pub async fn scheduled_backup(
        self: &Arc<Self>,
        state: &AppState,
        instance: MinecraftInstance,
    ) -> Result<(), Error> {
        let uuid = instance.uuid().await;
        self.store.instance_dir(&uuid)?;
        let guards = self.take_guards(&instance, "backing it up").await?;
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
            .backup_now(state, &guards, &instance, BackupKind::Scheduled, None, &job)
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
            image: Some(_), id, ..
        }) = &result
        {
            self.apply_retention(&uuid, Some(id)).await;
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
        let newest = match self.records(uuid).await {
            Ok(records) => newest_for_schedule(&records),
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
        backup: Backup,
        request: RestoreRequest,
        may_stop: bool,
        check: Option<DiffCheck>,
        caused_by: CausedBy,
    ) -> Result<String, Error> {
        let uuid = instance.uuid().await;
        self.store.instance_dir(&uuid)?;
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
        };
        if !was_running {
            guards.stopped = Some(instance.lock_stopped("restoring a backup").await?);
        }
        // the restore keeps using this policy: the check holds only for what it excludes
        let policy_exclude = self.policy(&uuid).await?.exclude;
        // checked now, so a refusal is the response; checked again once the server is
        // stopped, against what is then on disk
        let plan = self
            .plan(&instance, &backup, request.scope, &policy_exclude)
            .await?;
        if let Some(check) = &check {
            let diff = self.diff(&instance, plan.clone(), None).await?;
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
            let backup_id = backup.record.id.clone();
            let result = this
                .restore_locked(
                    &state,
                    guards,
                    &instance,
                    backup,
                    &request,
                    policy_exclude,
                    plan,
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
            }
        });
        Ok(task_id)
    }

    /// What restoring `backup` into the server would replace, as it is on disk now.
    async fn plan(
        &self,
        instance: &MinecraftInstance,
        backup: &Backup,
        scope: RestoreScope,
        policy_exclude: &[String],
    ) -> Result<RestorePlan, Error> {
        let ctx = InstanceContext::of(instance, policy_exclude.to_vec()).await;
        let store = self.image_store();
        let backup = backup.clone();
        blocking(move || restore::plan(&store, &ctx, &backup.source(), scope)).await
    }

    /// What `plan` would change, with the server's files hashed from `current` (the safety
    /// backup's image) or, without it, from the stat cache and the files themselves.
    async fn diff(
        &self,
        instance: &MinecraftInstance,
        plan: RestorePlan,
        current: Option<&Image>,
    ) -> Result<RestoreDiff, Error> {
        let hashes: Option<HashMap<String, String>> = current.map(|image| {
            image
                .entries
                .iter()
                .map(|e| (e.path.to_string(), e.sha256.clone()))
                .collect()
        });
        let uuid = instance.uuid().await;
        let root = instance.path().await;
        let store = self.image_store();
        let token = self.cancel.clone();
        blocking(move || {
            let hashes = match hashes {
                Some(hashes) => hashes,
                None => {
                    let cache = store.load_stat_cache(uuid.as_ref());
                    restore::hash_current(&root, &plan, &cache, &|| token.is_cancelled())?
                }
            };
            Ok(restore::diff(&plan, &hashes))
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
        backup: Backup,
        request: &RestoreRequest,
        policy_exclude: Vec<String>,
        checked: RestorePlan,
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
                &backup,
                request,
                &policy_exclude,
                &checked,
                check,
                was_running,
                job,
            )
            .await;
        let (safety_id, operation, software) = match swapped {
            Ok(swapped) => swapped,
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
        // A "world and its mods" restore leaves the server's settings file: what it says
        // about the software and content (the Minecraft version, the server software and its
        // loader version, the Java it runs on, the modpack origin) comes from the backup's
        // copy, since the restore put back the backup's server software and mods. Without it
        // a Forge server restored from an older build would start the newer build's
        // libraries, which are gone. Before the settings are read again.
        if let Some(settings) = software {
            let root = instance.path().await;
            if let Err(e) = blocking(move || restore::adopt_software(&root, &settings)).await {
                warn!(
                    "Failed to take the server software of {uuid} from the backup's settings: {}",
                    e.source
                );
            }
        }
        if let Err(e) = instance.reload_from_disk().await {
            return Err(err(
                ErrorKind::Internal,
                format!(
                    "The backup was restored, but the server's settings couldn't be read again: {}",
                    e.source
                ),
            ));
        }
        if let Some(operation) = operation {
            self.follow_restored_pack(instance, &backup).await;
            // the content as restored, before the server can start and change it
            let ctx = InstanceContext::of(instance, policy_exclude).await;
            if let Err(e) = operation.succeed_and_record(ctx).await {
                warn!(
                    "Failed to record the restored content of {uuid}: {}",
                    e.source
                );
            }
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

    /// After a restore that put back a backup's content: `pack.json` (with the images, so no
    /// restore brings it back) follows the base of the backup's content image
    /// ([`crate::modpack::pack_info::follow_base`]), so the next modpack update compares
    /// against the pack those files came from, and the restore's content image gets the
    /// right base. The modpack origin came back with the settings.
    async fn follow_restored_pack(&self, instance: &MinecraftInstance, backup: &Backup) {
        let Some(content) = &backup.content else {
            return;
        };
        let uuid = instance.uuid().await;
        crate::modpack::pack_info::follow_base(&self.images, &uuid, content.base.clone()).await;
    }

    /// [`Self::restore_locked`] from the stopped server to the swap. Returns the safety
    /// backup's id, the content operation the restore holds when it changed content (to
    /// succeed once the server's settings are read again), and for a world and mods restore
    /// the backup's settings, whose server software the server takes on.
    #[allow(clippy::too_many_arguments)]
    async fn swap_in(
        self: &Arc<Self>,
        state: &AppState,
        guards: &mut JobGuards,
        instance: &MinecraftInstance,
        backup: &Backup,
        request: &RestoreRequest,
        policy_exclude: &[String],
        checked: &RestorePlan,
        check: Option<DiffCheck>,
        was_running: bool,
        job: &Job,
    ) -> Result<(Option<String>, Option<Operation>, Option<serde_json::Value>), RestoreFailure>
    {
        let uuid = instance.uuid().await;
        let root = instance.path().await;
        if was_running {
            guards.stopped = Some(instance.lock_stopped("restoring a backup").await?);
        }
        // the content image first, so the safety backup links to the content as it is; then
        // the restore holds the content until its own image is recorded
        self.record_content(instance, policy_exclude).await;
        let operation = match request.scope {
            RestoreScope::World => None,
            RestoreScope::Everything | RestoreScope::WorldAndMods => Some(
                self.images
                    .begin(
                        &uuid,
                        format!("Restored backup “{}”", backup_label(&backup.record)),
                    )
                    .await,
            ),
        };
        let hold = hold_off_collection(self.image_store()).await?;
        let plan = self
            .plan(instance, backup, request.scope, policy_exclude)
            .await?;
        // read while the hold keeps the backup's objects: it may be deleted meanwhile
        let software = if request.scope == RestoreScope::WorldAndMods {
            let (store, backup) = (self.image_store(), backup.clone());
            match blocking(move || backup_settings(&store, &backup)).await {
                Ok(settings) => settings,
                Err(e) => {
                    warn!(
                        "Failed to read the server software in the backup of {uuid}: {}",
                        e.source
                    );
                    None
                }
            }
        } else {
            None
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
                    &hold,
                    instance,
                    BackupKind::BeforeRestore,
                    Some(safety_backup_name(&backup.record)),
                    // the check below compares against these files, under this exclude
                    Some(policy_exclude.to_vec()),
                    job.phase(0.0, safety_share),
                )
                .await?;
            if let Some(safety) = &created.image {
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
            Some((created.id, created.image))
        } else {
            None
        };
        let safety_id = safety.as_ref().map(|(id, _)| id.clone());

        // a server that was stopped all along was checked against these same files, under
        // `lock_stopped`; without a safety backup, don't hash it all again, unless the plan
        // changed since (a Files API change, or another level-name, between the two)
        let check = check.filter(|_| was_running || safety.is_some() || !plan.same_as(checked));
        if let Some(check) = &check {
            job.report(safety_share, Some("Checking what the restore changes"));
            let current = safety.as_ref().and_then(|(_, image)| image.as_ref());
            let diff = self.diff(instance, plan.clone(), current).await?;
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
            let store = self.image_store();
            let root = root.clone();
            let phase = job.phase(safety_share, 90.0 - safety_share);
            let token = self.cancel.clone();
            blocking(move || {
                restore::stage(
                    &store,
                    &root,
                    &plan,
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
        drop(hold);
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
            Ok(()) => Ok((safety_id, operation, software)),
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
        backup: Backup,
        request: NewServerRequest,
    ) -> Result<String, Error> {
        crate::implementations::minecraft::validate_new_instance_name(&request.name)?;
        let files = backup.files_on_disk()?;
        let base = backup.content.as_ref().and_then(|c| c.base.clone());
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
                    this.restore_as_new_locked(&state, files, &request, &new_uuid, port, &job)
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
                    let policy_exclude = this
                        .policy(&new_uuid)
                        .await
                        .map(|p| p.exclude)
                        .unwrap_or_default();
                    // The pack its content is built on and its first content image, as a
                    // new server gets, before any client can see it: nothing else can then
                    // start it or change its content first.
                    crate::modpack::pack_info::follow_base(&this.images, &new_uuid, base.clone())
                        .await;
                    let ctx = InstanceContext::of(&instance, policy_exclude).await;
                    if let Err(e) = this
                        .images
                        .record_now(ctx, format!("Created {}", request.name))
                        .await
                    {
                        warn!(
                            "Failed to record the content of the new server {new_uuid}: {}",
                            e.source
                        );
                    }
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
        files: Vec<restore::Target>,
        request: &NewServerRequest,
        new_uuid: &InstanceUuid,
        port: u32,
        job: &Job,
    ) -> Result<MinecraftInstance, Error> {
        let hold = hold_off_collection(self.image_store()).await?;
        let instances_dir = self.instances_dir.clone();
        let total = files
            .iter()
            .map(|f| f.entry.size)
            .fold(0, u64::saturating_add);
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
            let store = self.image_store();
            let (staging, final_dir) = (staging.clone(), final_dir.clone());
            let dot_config = dot_config.clone();
            let phase = job.phase(0.0, 95.0);
            let token = self.cancel.clone();
            blocking(move || {
                restore::stage_all(
                    &store,
                    &files,
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
        drop(hold);
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

    /// Zips a backup into a new temporary folder in `tmp_dir`, laid out as a server folder,
    /// from verified objects. Returns the zip, the folder that owns it and its size.
    ///
    /// Stops at the next file once `cancel` (a child of [`Self::cancel_token`]) is
    /// cancelled: the handler does that when the request is dropped.
    pub async fn zip(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        id: &str,
        tmp_dir: PathBuf,
        file_name: String,
        cancel: CancellationToken,
    ) -> Result<(PathBuf, tempfile::TempDir, u64), Error> {
        let hold = hold_off_collection(self.image_store()).await?;
        let backup = self.load(uuid, id).await?;
        let files = backup.files_on_disk()?;
        let store = self.image_store();
        let token = cancel;
        let zipped = blocking(move || {
            let total = files
                .iter()
                .map(|f| f.entry.size)
                .fold(0, u64::saturating_add);
            require_free_space(&tmp_dir, total, "to download the backup")?;
            std::fs::create_dir_all(&tmp_dir).context("Failed to create the temporary folder")?;
            let temp_dir =
                tempfile::tempdir_in(&tmp_dir).context("Failed to create a temporary folder")?;
            let out = temp_dir.path().join(&file_name);
            let file = std::fs::File::create(&out).context("Failed to create the zip")?;
            let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
            for file in &files {
                if token.is_cancelled() {
                    return Err(cancelled());
                }
                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .large_file(file.entry.size >= u32::MAX as u64)
                    .unix_permissions(lodestone_image::store::materialize::placed_mode(
                        file.entry.mode,
                    ));
                zip.start_file(file.disk.as_str(), options)
                    .context("Failed to write the zip")?;
                store
                    .read_object(&file.entry.sha256, file.entry.size, &mut zip, &|| {
                        token.is_cancelled()
                    })
                    .map_err(|e| {
                        err(
                            ErrorKind::Internal,
                            format!("The backup's copy of {} can't be read: {e}", file.disk),
                        )
                    })?;
            }
            let mut writer = zip.finish().context("Failed to write the zip")?;
            std::io::Write::flush(&mut writer).context("Failed to write the zip")?;
            drop(writer);
            let size = std::fs::metadata(&out)
                .context("Failed to read the zip")?
                .len();
            Ok((out, temp_dir, size))
        })
        .await;
        drop(hold);
        zipped
    }
}

/// What [`BackupService::save`] did: the backup's id, and its image when it saved one.
struct Saved {
    id: String,
    image: Option<Image>,
}

/// The newest Scheduled or Manual backup's time, which the schedule counts from.
fn newest_for_schedule(records: &Records) -> Option<i64> {
    records
        .backups
        .iter()
        .filter(|b| matches!(b.kind, BackupKind::Scheduled | BackupKind::Manual))
        .map(|b| b.created)
        .max()
}

#[cfg(test)]
mod tests;
