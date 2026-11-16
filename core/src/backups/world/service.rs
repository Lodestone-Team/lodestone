//! [`WorldBackupService`]: making, listing, restoring and deleting a server's world
//! backups, and the jobs around them.
//!
//! World backups share the server's job lock with snapshots ([`BackupService`]): a world
//! backup, a snapshot and either's restore never run at once, and the lists' `running`
//! report whichever holds it. A world job also registers its cancel token here, so
//! `…/tasks/{task_id}/cancel` can stop it until a restore begins its swap.
//!
//! Locks, outermost first: the server's job lock; `lock_stopped` while a stopped server is
//! backed up or restored, so it can't start meanwhile; and the server's record lock here,
//! around a list, a backup's final rename with its sidecar, a rename and a delete.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use chrono::Utc;
use lodestone_image::ServerLayout;
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

use crate::backups::live::{with_saving_off, ConsoleLines, InstanceLines, Limits, LiveServer};
use crate::backups::restore::{self, stage_folders, sync_dir};
use crate::backups::service::{swap_outcome, Job, Phase, SwapOutcome};
use crate::backups::store::{cancelled, check_backup_name, err, new_backup_id, rename_with_retry};
use crate::backups::types::MAX_NAME_LEN;
use crate::backups::{BackupService, JobKind};
use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event, ProgressionEndValue, ProgressionStartValue};
use crate::images::layout::server_flavour;
use crate::images::{blocking, ImageService, InstanceContext};
use crate::implementations::minecraft::MinecraftInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State, TServer};
use crate::types::InstanceUuid;
use crate::util::disk::{free_space, same_disk, LargeWrites};
use crate::AppState;

use super::reader::{extract_into, plan_restore_with, RestoreZipPlan, ZipLimits};
use super::store::{not_found, WorldStore};
use super::types::{
    WorldBackupInfo, WorldBackupKind, WorldBackupList, WorldBackupRecord, WorldBackupUsage,
    WORLD_RECORD_VERSION,
};
use super::writer::{check_world, plan_world, world_tops, write_world_zip, WorldPlan};

fn conflict(message: impl Into<String>) -> Error {
    err(ErrorKind::Conflict, message)
}

/// A server's world as a backup reads it, from its context.
#[derive(Debug, Clone)]
pub struct WorldSource {
    pub uuid: InstanceUuid,
    pub root: PathBuf,
    pub layout: ServerLayout,
    pub instance_name: String,
    pub game_version: String,
    pub flavour: String,
}

impl WorldSource {
    /// The world of the server `ctx` describes. A `level-name` that isn't one folder name
    /// (`worlds/survival`) is a bad request: its world can't be backed up on its own.
    pub fn of(ctx: &InstanceContext, instance_name: String) -> Result<WorldSource, Error> {
        let layout = ServerLayout::new(
            ctx.level_name.clone(),
            [],
            server_flavour(&ctx.flavour).splits_worlds(),
        )
        .map_err(|_| {
            err(
                ErrorKind::BadRequest,
                "The server's level-name isn't a single folder name, so Lodestone can't back up its world",
            )
        })?;
        Ok(WorldSource {
            uuid: ctx.uuid.clone(),
            root: ctx.root.clone(),
            layout,
            instance_name,
            game_version: ctx.game_version.clone(),
            flavour: ctx.flavour.to_string(),
        })
    }
}

/// A running server's save pause, for a live world backup.
pub struct Live<'a> {
    pub server: &'a dyn LiveServer,
    pub lines: &'a mut dyn ConsoleLines,
    pub limits: Limits,
}

/// What a new world backup is.
#[derive(Debug, Clone)]
pub struct NewBackup {
    pub kind: WorldBackupKind,
    pub name: Option<String>,
    pub before_restoring: Option<String>,
    /// Refuse a level without a `level.dat` (NotFound); a restore's safety backup takes
    /// one all the same.
    pub require_level_dat: bool,
    /// Refuse (Conflict) a backup that left anything out: a restore deletes what its
    /// safety backup doesn't hold, so that backup must hold everything.
    pub refuse_left_out: bool,
}

/// Checks a restore of a world backup against the requester's permissions: given what it
/// places and what the folders it replaces hold now (disk paths, with whether each is a
/// folder). Made by the handler.
pub type WorldCheck =
    Arc<dyn Fn(&RestoreZipPlan, &[(String, bool)]) -> Result<(), Error> + Send + Sync>;

/// A running world job, as the cancel route finds it.
struct WorldJob {
    task_id: String,
    kind: JobKind,
    cancel: CancellationToken,
    /// Whether it can still be cancelled; a restore clears it as its swap begins. Cancelling
    /// and clearing both hold this lock, so a cancel never reports success for a swap that
    /// went ahead.
    cancellable: Arc<StdMutex<bool>>,
}

/// What a restore did: the safety backup it took.
struct Restored {
    safety_id: Option<String>,
}

/// How a restore's unpacking and swap failed: with the world as it was, or with a swap
/// that couldn't be undone.
enum Failed {
    Unchanged(Error),
    Broken(Error),
}

impl From<Error> for Failed {
    fn from(e: Error) -> Failed {
        Failed::Unchanged(e)
    }
}

/// A restore checked before anything is written ([`WorldBackupService::prepare_restore`]).
pub(crate) struct PreparedRestore {
    pub plan: RestoreZipPlan,
    /// What the restore replaces now, for the permission check.
    pub current: Vec<(String, bool)>,
    /// Whether the server has any of the world's folders now, so a safety backup is taken.
    pub safety: bool,
}

/// Removes a world job from the service however its task ends, a panic included, so a
/// dead job never keeps deletes refused.
struct JobCleanup {
    service: Arc<WorldBackupService>,
    uuid: InstanceUuid,
}

impl Drop for JobCleanup {
    fn drop(&mut self) {
        self.service.remove_job(&self.uuid);
        self.service.backups.unregister(&self.uuid);
    }
}

/// A world backup's label in messages: its name, or its date.
pub fn backup_label(record: &WorldBackupRecord) -> String {
    record.name.clone().unwrap_or_else(|| {
        chrono::DateTime::from_timestamp(record.created, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| record.id.clone())
    })
}

/// The safety backup's name: "Before restoring “…”", shortened to fit [`MAX_NAME_LEN`] as a
/// snapshot's is (restoring a safety backup nests them).
pub fn safety_name(label: &str) -> String {
    let wrap = |label: &str| format!("Before restoring “{label}”");
    let room = MAX_NAME_LEN - wrap("").chars().count();
    if label.chars().count() <= room {
        return wrap(label);
    }
    let cut: String = label.chars().take(room - 1).collect();
    wrap(&format!("{}…", cut.trim_end()))
}

/// Server world backups: the store, the record locks and the running world jobs.
pub struct WorldBackupService {
    backups: Arc<BackupService>,
    images: Arc<ImageService>,
    store: WorldStore,
    records: StdMutex<HashMap<InstanceUuid, Arc<Mutex<()>>>>,
    jobs: StdMutex<HashMap<InstanceUuid, WorldJob>>,
    /// The bounds of a world backup: what a restore reads, so every backup made restores.
    limits: ZipLimits,
}

impl WorldBackupService {
    /// World backups in `root` (`<lodestone>/world-backups`), sharing `backups`' job locks
    /// and `images`' count of large writes.
    pub fn new(
        backups: Arc<BackupService>,
        images: Arc<ImageService>,
        root: PathBuf,
    ) -> WorldBackupService {
        WorldBackupService {
            backups,
            images,
            store: WorldStore::new(root),
            records: Default::default(),
            jobs: Default::default(),
            limits: ZipLimits::default(),
        }
    }

    pub fn store(&self) -> &WorldStore {
        &self.store
    }

    /// At startup, before the core serves requests: deletes the part files and temporary
    /// sidecars a crash left. It would delete a running backup's, so nothing may run yet.
    /// Blocking.
    pub fn sweep(&self) {
        self.store.sweep();
    }

    /// The server's record lock (see the module).
    async fn lock_records(&self, uuid: &InstanceUuid) -> OwnedMutexGuard<()> {
        let lock = {
            let mut map = self.records.lock().unwrap_or_else(|e| e.into_inner());
            map.retain(|_, lock| Arc::strong_count(lock) > 1);
            map.entry(uuid.clone()).or_default().clone()
        };
        lock.lock_owned().await
    }

    fn world_job_running(&self, uuid: &InstanceUuid) -> bool {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(uuid)
    }

    fn add_job(&self, uuid: &InstanceUuid, job: WorldJob) {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(uuid.clone(), job);
    }

    fn remove_job(&self, uuid: &InstanceUuid) {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
    }

    // -----------------------------------------------------------------------------------
    // Reading

    /// `GET /instance/{uuid}/world-backups`.
    pub async fn list(&self, uuid: &InstanceUuid) -> Result<WorldBackupList, Error> {
        self.store.dir(uuid)?;
        let records = {
            let _lock = self.lock_records(uuid).await;
            let (store, id) = (self.store.clone(), uuid.clone());
            blocking(move || store.list(&id)).await?
        };
        let root = self.store.root().to_path_buf();
        let disk_free = blocking(move || Ok(free_space(&root))).await?;
        Ok(WorldBackupList {
            usage: WorldBackupUsage {
                total_bytes: records.iter().map(|r| r.size).fold(0, u64::saturating_add),
                disk_free,
            },
            backups: records.iter().map(WorldBackupInfo::from).collect(),
            running: self.backups.running_job(uuid),
        })
    }

    /// A world backup's sidecar: NotFound when there is no such backup.
    pub async fn load(&self, uuid: &InstanceUuid, id: &str) -> Result<WorldBackupRecord, Error> {
        let (store, uuid, id) = (self.store.clone(), uuid.clone(), id.to_string());
        blocking(move || store.load(&uuid, &id)).await
    }

    /// The stored zip of a backup and its size, for a download link.
    pub async fn zip_for_download(
        &self,
        uuid: &InstanceUuid,
        id: &str,
    ) -> Result<(PathBuf, u64, WorldBackupRecord), Error> {
        let record = self.load(uuid, id).await?;
        let path = self.store.zip_path(uuid, id)?;
        let size = {
            let path = path.clone();
            blocking(move || {
                std::fs::metadata(&path)
                    .map(|m| m.len())
                    .map_err(|_| not_found())
            })
            .await?
        };
        Ok((path, size, record))
    }

    // -----------------------------------------------------------------------------------
    // Changing

    /// `PATCH /instance/{uuid}/world-backups/{id}`: renames a backup (`None` clears the
    /// name).
    pub async fn rename(
        &self,
        uuid: &InstanceUuid,
        id: &str,
        name: Option<String>,
    ) -> Result<WorldBackupInfo, Error> {
        let name = match name {
            Some(name) => check_backup_name(&name).map_err(|e| err(ErrorKind::BadRequest, e))?,
            None => None,
        };
        let _lock = self.lock_records(uuid).await;
        let (store, uuid, id) = (self.store.clone(), uuid.clone(), id.to_string());
        blocking(move || {
            let mut record = store.load(&uuid, &id)?;
            record.name = name;
            store.save_record(&uuid, &record)?;
            Ok(WorldBackupInfo::from(&record))
        })
        .await
    }

    /// `DELETE /instance/{uuid}/world-backups/{id}`. Refused while a world job of the
    /// server runs: a restore may be reading the zip, and a backup writing its folder.
    pub async fn delete(&self, uuid: &InstanceUuid, id: &str) -> Result<(), Error> {
        self.store.dir(uuid)?;
        if self.world_job_running(uuid) {
            return Err(conflict(
                "A world backup or restore of this server is running; try again when it is done",
            ));
        }
        let _lock = self.lock_records(uuid).await;
        let (store, uuid, id) = (self.store.clone(), uuid.clone(), id.to_string());
        blocking(move || store.delete(&uuid, &id)).await
    }

    /// When a server is deleted: deletes all its world backups. The caller holds the
    /// server's job lock.
    pub async fn delete_server(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let _lock = self.lock_records(uuid).await;
        let (store, uuid) = (self.store.clone(), uuid.clone());
        blocking(move || store.delete_server(&uuid)).await
    }

    // -----------------------------------------------------------------------------------
    // Cancelling

    /// The kind of the world job `task_id` of the server, if it runs: the cancel route
    /// checks the permission of the route that started it.
    pub fn job_kind(&self, uuid: &InstanceUuid, task_id: &str) -> Option<JobKind> {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .filter(|job| job.task_id == task_id)
            .map(|job| job.kind)
    }

    /// `POST /instance/{uuid}/world-backups/tasks/{task_id}/cancel`: NotFound when no such
    /// world job runs, Conflict once a restore has begun its swap.
    pub fn cancel(&self, uuid: &InstanceUuid, task_id: &str) -> Result<(), Error> {
        let jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let job = jobs
            .get(uuid)
            .filter(|job| job.task_id == task_id)
            .ok_or_else(|| {
                err(
                    ErrorKind::NotFound,
                    "No such world backup or restore is running",
                )
            })?;
        let cancellable = job.cancellable.lock().unwrap_or_else(|e| e.into_inner());
        if !*cancellable {
            return Err(conflict(
                "The world is being put in place and can't be cancelled now",
            ));
        }
        job.cancel.cancel();
        Ok(())
    }

    // -----------------------------------------------------------------------------------
    // Making a world backup

    /// Writes a world backup of `source` (see the module): the zip inside `live`'s save
    /// pause when given (the server runs), else as it is (the caller holds
    /// `lock_stopped`), then its sidecar. `phase` gets 0..95 for the copy and the rest for
    /// saving. On failure or cancellation (`token`) nothing is left.
    pub(crate) async fn write_backup(
        &self,
        source: &WorldSource,
        live: Option<Live<'_>>,
        new: NewBackup,
        phase: Phase,
        token: &CancellationToken,
    ) -> Result<WorldBackupRecord, Error> {
        let uuid = source.uuid.clone();
        let dir = self.store.dir(&uuid)?;
        let id = new_backup_id(Utc::now());
        let part = self.store.part_path(&uuid, &id)?;
        let zip = self.store.zip_path(&uuid, &id)?;
        phase.report(0.0, Some("Looking at the world"));
        // the copy re-walks inside the save pause; this sizes it and refuses a world too
        // large to restore before anything is reserved or paused
        let plan = {
            let (root, layout) = (source.root.clone(), source.layout.clone());
            let (require, limits) = (new.require_level_dat, self.limits);
            blocking(move || plan_world(&root, &layout, require, limits)).await?
        };
        let reserved = {
            let (large_writes, dir) = (self.images.large_writes.clone(), dir.clone());
            let total = plan.total_bytes;
            Arc::new(
                blocking(move || {
                    std::fs::create_dir_all(&dir).map_err(|e| {
                        err(
                            ErrorKind::Internal,
                            format!("Failed to create the world backups' folder: {e}"),
                        )
                    })?;
                    large_writes.require(&dir, total, "for a world backup")
                })
                .await?,
            )
        };
        let write = |work_token: CancellationToken| {
            let (plan, part, phase, reserved) =
                (plan.clone(), part.clone(), phase.clone(), reserved.clone());
            blocking(move || {
                phase.report(0.0, Some("Copying the world"));
                let written = write_world_zip(
                    &plan,
                    &part,
                    &|done, total| {
                        reserved.set_remaining(total.saturating_sub(done));
                        let share = if total == 0 {
                            1.0
                        } else {
                            (done as f64 / total as f64).min(1.0)
                        };
                        phase.report(95.0 * share, None);
                    },
                    &|| work_token.is_cancelled(),
                )?;
                // stopped as it finished (the save pause's cap): whoever stopped it no
                // longer waits for this zip, so it is deleted here
                if work_token.is_cancelled() {
                    let _ = std::fs::remove_file(&part);
                    return Err(cancelled());
                }
                Ok(written)
            })
        };
        let remove_part = |part: PathBuf| async move {
            let _ = blocking(move || {
                let _ = std::fs::remove_file(&part);
                Ok(())
            })
            .await;
        };
        let live_now = live.is_some();
        let outcome = match live {
            Some(live) => {
                let marker = self.backups.store().marker_path(&uuid)?;
                with_saving_off(
                    live.server,
                    live.lines,
                    &marker,
                    live.limits,
                    token,
                    |_, work_token| write(work_token),
                )
                .await
            }
            None => write(token.clone()).await.map(|written| (written, true)),
        };
        drop(reserved);
        let (written, quiesced) = match outcome {
            Ok(done) => done,
            Err(e) => {
                // the writer deletes its own part file when it stops; this catches one a
                // dropped copy (the save pause's cap) finished as it was dropped
                remove_part(part.clone()).await;
                return Err(e);
            }
        };
        if token.is_cancelled() {
            remove_part(part.clone()).await;
            return Err(cancelled());
        }
        if new.refuse_left_out && written.left_out > 0 {
            remove_part(part.clone()).await;
            return Err(left_out_refusal(written.left_out, &written.left_out_sample));
        }

        phase.report(95.0, Some("Saving the world backup"));
        let mut record = WorldBackupRecord {
            version: WORLD_RECORD_VERSION,
            id: id.clone(),
            name: new.name,
            kind: new.kind,
            created: Utc::now().timestamp(),
            level: source.layout.level_name().to_string(),
            folders: plan.folders.clone(),
            instance_name: source.instance_name.clone(),
            game_version: source.game_version.clone(),
            flavour: source.flavour.clone(),
            size: written.size,
            file_count: written.file_count,
            left_out: written.left_out,
            live: live_now,
            quiesced,
            before_restoring: new.before_restoring,
        };
        let record = {
            let _lock = self.lock_records(&uuid).await;
            let (store, uuid) = (self.store.clone(), uuid.clone());
            blocking(move || {
                let mut zip = zip;
                let mut renamed = false;
                let saved = (|| {
                    // ids are to the second with 4 random hex digits: should another backup
                    // of this server have this one (renaming over its zip would replace
                    // it), take another. The record lock makes the check and rename one step
                    let taken = |zip: &Path, id: &str| -> Result<bool, Error> {
                        Ok(std::fs::symlink_metadata(zip).is_ok()
                            || std::fs::symlink_metadata(store.record_path(&uuid, id)?).is_ok())
                    };
                    let mut tries = 0;
                    while taken(&zip, &record.id)? {
                        tries += 1;
                        if tries > 16 {
                            return Err(err(
                                ErrorKind::Internal,
                                "Failed to save the world backup: no free name for it",
                            ));
                        }
                        record.id = new_backup_id(Utc::now());
                        zip = store.zip_path(&uuid, &record.id)?;
                    }
                    rename_with_retry(&part, &zip, restore::SWAP_RENAME_BUDGET).map_err(|e| {
                        err(
                            ErrorKind::Internal,
                            format!("Failed to save the world backup: {e}"),
                        )
                    })?;
                    renamed = true;
                    if let Err(e) = sync_dir(&dir) {
                        warn!("Failed to flush {}: {e}", dir.display());
                    }
                    store.save_record(&uuid, &record)
                })();
                match saved {
                    Ok(()) => Ok(record),
                    Err(e) => {
                        // a failed backup leaves nothing, not a zip that would be listed
                        // again (`zip` is this backup's only once renamed)
                        let _ = std::fs::remove_file(&part);
                        if renamed {
                            let _ = std::fs::remove_file(&zip);
                        }
                        Err(e)
                    }
                }
            })
            .await?
        };
        phase.report(100.0, None);
        Ok(record)
    }

    /// The end message of a world backup.
    fn backup_message(server_name: &str, record: &WorldBackupRecord) -> String {
        let mut message = format!("Backed up the world of {server_name}");
        match record.left_out {
            0 => {}
            1 => message.push_str(". 1 file couldn't be read or was a link"),
            n => message.push_str(&format!(". {n} files couldn't be read or were links")),
        }
        if record.live && !record.quiesced {
            message.push_str(
                ". The server didn't confirm it had saved, so the backup may be inconsistent",
            );
        }
        message
    }

    /// A failure's end message, or the cancellation's.
    fn failure_message(&self, what: &str, e: &Error, token: &CancellationToken) -> String {
        if token.is_cancelled() && !self.backups.cancel_token().is_cancelled() {
            format!("{what} was cancelled")
        } else {
            self.backups.failure_message(what, e)
        }
    }

    /// `POST /instance/{uuid}/world-backups`: checks and locks, then backs the world up in
    /// the background. Returns the progression event's id.
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
        self.store.dir(&uuid)?;
        let guards = self
            .backups
            .take_guards(&instance, "backing up its world")
            .await?;
        let server_name = instance.name().await;
        let ctx = InstanceContext::of(&instance, Vec::new()).await;
        let source = WorldSource::of(&ctx, server_name.clone())?;
        {
            let (root, layout, limits) = (source.root.clone(), source.layout.clone(), self.limits);
            blocking(move || check_world(&root, &layout, limits)).await?;
        }
        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::WorldBackup,
            format!("Backing up the world of {server_name}"),
            ProgressionStartValue::WorldBackup {
                instance_uuid: uuid.clone(),
            },
            caused_by,
        );
        job.begin();
        self.backups.register(&uuid, &job);
        let token = self.backups.cancel_token().child_token();
        self.add_job(
            &uuid,
            WorldJob {
                task_id: job.task_id(),
                kind: JobKind::WorldBackup,
                cancel: token.clone(),
                cancellable: Arc::new(StdMutex::new(true)),
            },
        );
        let task_id = job.task_id();
        let (this, state) = (self.clone(), state.clone());
        let cleanup = JobCleanup {
            service: self.clone(),
            uuid: uuid.clone(),
        };
        self.backups.spawn(async move {
            let _cleanup = cleanup;
            let console = |message: &str| {
                state.event_broadcaster.send(Event::new_system_message(
                    uuid.clone(),
                    server_name.clone(),
                    message.to_string(),
                ))
            };
            let new = NewBackup {
                kind: WorldBackupKind::Manual,
                name,
                before_restoring: None,
                require_level_dat: true,
                refuse_left_out: false,
            };
            let result = if guards.stopped.is_none() {
                console("Backing up the world…");
                let mut lines =
                    InstanceLines::new(state.event_broadcaster.subscribe(), uuid.clone());
                let live = Live {
                    server: &instance,
                    lines: &mut lines,
                    limits: Limits::default(),
                };
                let result = this
                    .write_backup(&source, Some(live), new, job.phase(0.0, 100.0), &token)
                    .await;
                console(if result.is_ok() {
                    "World backup done"
                } else {
                    "World backup failed"
                });
                result
            } else {
                this.write_backup(&source, None, new, job.phase(0.0, 100.0), &token)
                    .await
            };
            drop(guards);
            let (success, message, id) = match &result {
                Ok(record) => (
                    true,
                    Self::backup_message(&server_name, record),
                    Some(record.id.clone()),
                ),
                Err(e) => (
                    false,
                    this.failure_message("The world backup", e, &token),
                    None,
                ),
            };
            job.end(
                success,
                &message,
                ProgressionEndValue::WorldBackupCompleted {
                    instance_uuid: uuid.clone(),
                    world_backup_id: id,
                    success,
                    message: message.clone(),
                },
            );
        });
        Ok(task_id)
    }

    // -----------------------------------------------------------------------------------
    // Restoring

    /// `POST /instance/{uuid}/world-backups/{id}/restore`, after the handler checked the
    /// route's permission: locks the stopped server, plans the restore and checks it with
    /// `check` (so a refusal is the response), then restores in the background. Returns the
    /// progression event's id.
    pub async fn start_restore(
        self: &Arc<Self>,
        state: &AppState,
        instance: MinecraftInstance,
        id: &str,
        check: Option<WorldCheck>,
        caused_by: CausedBy,
    ) -> Result<String, Error> {
        let uuid = instance.uuid().await;
        self.store.dir(&uuid)?;
        let record = self.load(&uuid, id).await?;
        if self.backups.is_broken(&uuid) {
            return Err(conflict(
                "The last restore of this server couldn't be undone; restart Lodestone to repair it",
            ));
        }
        let must_stop = || conflict("Stop the server before restoring a world backup");
        if !matches!(instance.state().await, State::Stopped) {
            return Err(must_stop());
        }
        let job_guard = self.backups.lock_job(&uuid)?;
        let stopped = instance
            .lock_stopped("restoring a world backup")
            .await
            .map_err(|_| must_stop())?;
        let server_name = instance.name().await;
        let ctx = InstanceContext::of(&instance, Vec::new()).await;
        let source = WorldSource::of(&ctx, server_name.clone())?;
        let prepared = self.prepare_restore(&source, &record).await?;
        if let Some(check) = &check {
            check(&prepared.plan, &prepared.current)?;
        }

        let job = Job::new(
            state.event_broadcaster.clone(),
            JobKind::WorldRestore,
            format!("Restoring the world of {server_name}"),
            ProgressionStartValue::WorldRestore {
                instance_uuid: uuid.clone(),
            },
            caused_by,
        );
        job.begin();
        self.backups.register(&uuid, &job);
        let token = self.backups.cancel_token().child_token();
        let cancellable = Arc::new(StdMutex::new(true));
        self.add_job(
            &uuid,
            WorldJob {
                task_id: job.task_id(),
                kind: JobKind::WorldRestore,
                cancel: token.clone(),
                cancellable: cancellable.clone(),
            },
        );
        let task_id = job.task_id();
        let this = self.clone();
        let cleanup = JobCleanup {
            service: self.clone(),
            uuid: uuid.clone(),
        };
        self.backups.spawn(async move {
            let _cleanup = cleanup;
            let _job_guard = job_guard;
            let mut stopped = Some(stopped);
            let result = this
                .restore_locked(
                    &source,
                    &record,
                    prepared,
                    &mut stopped,
                    &cancellable,
                    &job,
                    &token,
                )
                .await;
            drop(stopped);
            let (success, message, safety_id) = match result {
                Ok(restored) => (
                    true,
                    format!(
                        "Restored the world of {server_name} from “{}”",
                        backup_label(&record)
                    ),
                    restored.safety_id,
                ),
                Err(e) => (
                    false,
                    this.failure_message("The world restore", &e, &token),
                    None,
                ),
            };
            job.end(
                success,
                &message,
                ProgressionEndValue::WorldRestoreCompleted {
                    instance_uuid: uuid.clone(),
                    world_backup_id: Some(record.id.clone()),
                    safety_id,
                    success,
                    message: message.clone(),
                },
            );
        });
        Ok(task_id)
    }

    /// Plans a restore of `record` into the stopped server `source` and checks it before
    /// anything is written, so a refusal is the route's response: the zip's plan; the
    /// world's folders now (none may be a link or a file), for the permission check; that
    /// the safety backup can hold everything the restore replaces and fits the limits; and
    /// that the disks have room for the safety backup and the unpacked world together.
    pub(crate) async fn prepare_restore(
        &self,
        source: &WorldSource,
        record: &WorldBackupRecord,
    ) -> Result<PreparedRestore, Error> {
        let zip = self.store.zip_path(&source.uuid, &record.id)?;
        let (source, level, limits) = (source.clone(), record.level.clone(), self.limits);
        let (large_writes, backups_dir) = (
            self.images.large_writes.clone(),
            self.store.dir(&source.uuid)?,
        );
        blocking(move || {
            let plan = plan_restore_with(&zip, &level, &source.layout, limits)?;
            let current = current_paths(&source.root, &plan.tops)?;
            let safety = safety_plan(&source.root, &source.layout, limits)?;
            if let Some(safety) = &safety {
                if safety.left_out > 0 {
                    return Err(left_out_refusal(safety.left_out, &safety.left_out_sample));
                }
            }
            check_room(
                &large_writes,
                &source.root,
                plan.total_bytes,
                &backups_dir,
                safety.as_ref().map_or(0, |s| s.total_bytes),
            )?;
            Ok(PreparedRestore {
                plan,
                current,
                safety: safety.is_some(),
            })
        })
        .await
    }

    /// The restore itself, in the background, inside the job lock and `stopped`: the
    /// safety backup (0..40), the zip extracted into staging (40..90), then the swap. A
    /// restore that stops before the swap, or whose swap was undone, leaves the world as
    /// it was, so its safety backup (a copy of that world) is deleted again. A swap that
    /// couldn't be undone moves `stopped` into the service's broken servers, so the server
    /// can't start until the core repairs it.
    #[allow(clippy::too_many_arguments)]
    async fn restore_locked(
        &self,
        source: &WorldSource,
        record: &WorldBackupRecord,
        prepared: PreparedRestore,
        stopped: &mut Option<OwnedMutexGuard<()>>,
        cancellable: &Arc<StdMutex<bool>>,
        job: &Job,
        token: &CancellationToken,
    ) -> Result<Restored, Error> {
        let uuid = source.uuid.clone();
        // the world as it is now, so the restore can be undone: whenever any of its folders
        // is there, even without a level.dat
        let safety_id = if prepared.safety {
            job.report(0.0, Some("Backing up the world as it is now"));
            let label = backup_label(record);
            let safety = self
                .write_safety_backup(source, &label, job.phase(0.0, 40.0), token)
                .await?;
            Some(safety.id)
        } else {
            None
        };
        match self
            .unpack_and_swap(
                source,
                record,
                prepared.plan,
                stopped,
                cancellable,
                job,
                token,
            )
            .await
        {
            Ok(()) => Ok(Restored { safety_id }),
            Err(Failed::Broken(e)) => Err(e),
            Err(Failed::Unchanged(e)) => {
                if let Some(id) = safety_id {
                    let _lock = self.lock_records(&uuid).await;
                    let (store, uuid) = (self.store.clone(), uuid.clone());
                    if let Err(e) = blocking(move || store.delete(&uuid, &id)).await {
                        warn!(
                            "Failed to delete the unneeded safety world backup: {}",
                            e.source
                        );
                    }
                }
                Err(e)
            }
        }
    }

    /// The zip extracted into staging (40..90), then the swap (see [`Self::restore_locked`]).
    #[allow(clippy::too_many_arguments)]
    async fn unpack_and_swap(
        &self,
        source: &WorldSource,
        record: &WorldBackupRecord,
        plan: RestoreZipPlan,
        stopped: &mut Option<OwnedMutexGuard<()>>,
        cancellable: &Arc<StdMutex<bool>>,
        job: &Job,
        token: &CancellationToken,
    ) -> Result<(), Failed> {
        let uuid = source.uuid.clone();
        let root = source.root.clone();
        job.report(40.0, Some("Unpacking the world"));
        let zip = self.store.zip_path(&uuid, &record.id)?;
        let staged = {
            let (large_writes, root, phase, token) = (
                self.images.large_writes.clone(),
                root.clone(),
                job.phase(40.0, 50.0),
                token.clone(),
            );
            blocking(move || {
                let reserved =
                    large_writes.require(&root, plan.total_bytes, "to restore the world")?;
                stage_folders(&root, &plan.tops, &mut |staging| {
                    extract_into(
                        &zip,
                        &plan,
                        staging,
                        &|done, total| {
                            reserved.set_remaining(total.saturating_sub(done));
                            phase.report(
                                if total == 0 {
                                    100.0
                                } else {
                                    100.0 * done as f64 / total as f64
                                },
                                None,
                            );
                        },
                        &|| token.is_cancelled(),
                    )
                })
            })
            .await?
        };
        // a cancel from here on is refused; one that came first stops the restore
        let cancelled_first = {
            let mut cancellable = cancellable.lock().unwrap_or_else(|e| e.into_inner());
            *cancellable = false;
            token.is_cancelled()
        };
        if cancelled_first {
            blocking(move || {
                staged.discard();
                Ok(())
            })
            .await?;
            return Err(cancelled().into());
        }

        // from here on the restore isn't cancellable: it is short, and half a swap is worse
        job.report(90.0, Some("Putting the world in place"));
        let journal = self.backups.store().journal_path(&uuid)?;
        let joined = tokio::task::spawn_blocking(move || staged.swap(&journal)).await;
        match swap_outcome(joined) {
            Ok(()) => Ok(()),
            Err(SwapOutcome::RolledBack(e)) => Err(e.into()),
            Err(SwapOutcome::Broken(e)) => {
                error!(
                    "The world restore of {uuid} could not be undone: {}",
                    e.source
                );
                if let Some(guard) = stopped.take() {
                    self.backups.mark_broken(&uuid, guard);
                }
                Err(Failed::Broken(err(
                    ErrorKind::Internal,
                    "The restore could not be undone; restart Lodestone to repair it",
                )))
            }
        }
    }

    /// The safety world backup a restore takes of the stopped server's world, which may
    /// have no `level.dat` (it is kept all the same).
    async fn write_safety_backup(
        &self,
        source: &WorldSource,
        label: &str,
        phase: Phase,
        token: &CancellationToken,
    ) -> Result<WorldBackupRecord, Error> {
        self.write_backup(
            source,
            None,
            NewBackup {
                kind: WorldBackupKind::BeforeRestore,
                name: Some(safety_name(label)),
                before_restoring: Some(label.to_string()),
                require_level_dat: false,
                refuse_left_out: true,
            },
            phase,
            token,
        )
        .await
    }
}

/// The refusal of a restore whose safety backup can't hold everything the restore would
/// delete: `count` paths left out, `sample` the first few.
fn left_out_refusal(count: u64, sample: &[String]) -> Error {
    let mut named = sample.join(", ");
    let unnamed = count.saturating_sub(sample.len() as u64);
    match unnamed {
        0 => {}
        1 => named.push_str(" and 1 other"),
        n => named.push_str(&format!(" and {n} others")),
    }
    conflict(format!(
        "Couldn't include {named} in the safety world backup (links, files Lodestone can't \
         read, or names it can't restore), and the restore would delete them. Move them out \
         of the world's folders, or check that Lodestone can read them"
    ))
}

/// What the safety backup of the world in `root` would hold, within `limits`: `None` when
/// none of the world's folders is there. A world too large for it can't be restored over.
/// Blocking.
fn safety_plan(
    root: &Path,
    layout: &ServerLayout,
    limits: ZipLimits,
) -> Result<Option<WorldPlan>, Error> {
    if !world_tops(layout).iter().any(|top| root.join(top).is_dir()) {
        return Ok(None);
    }
    plan_world(root, layout, false, limits)
        .map(Some)
        .map_err(|e| {
            conflict(format!(
                "The world as it is now can't be saved first, so the restore couldn't be undone. {}",
                e.source
            ))
        })
}

/// Checks the disks have room for a restore: `restore_bytes` unpacked beside the server in
/// `root`, and the safety backup's `safety_bytes` in `backups_dir`, added up when both are
/// on one disk. Holds nothing afterwards; the steps reserve their own space as they write.
fn check_room(
    large_writes: &Arc<LargeWrites>,
    root: &Path,
    restore_bytes: u64,
    backups_dir: &Path,
    safety_bytes: u64,
) -> Result<(), Error> {
    let what = "to restore the world";
    if safety_bytes > 0 && !same_disk(root, backups_dir) {
        drop(large_writes.require(backups_dir, safety_bytes, "for the safety world backup")?);
        drop(large_writes.require(root, restore_bytes, what)?);
    } else {
        drop(large_writes.require(root, restore_bytes.saturating_add(safety_bytes), what)?);
    }
    Ok(())
}

/// The files and folders under `tops` in `root` now, by disk path, with whether each is a
/// folder: what a restore replaces, for the permission check. A top that is a link or a
/// file is refused here, before anything is walked (as `stage_folders` would refuse it);
/// links inside are listed, not followed. Blocking.
fn current_paths(root: &Path, tops: &[String]) -> Result<Vec<(String, bool)>, Error> {
    let mut found = Vec::new();
    for top in tops {
        let dir = root.join(top);
        match std::fs::symlink_metadata(&dir) {
            Err(_) => continue,
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(err(
                    ErrorKind::BadRequest,
                    format!("{top} is a link; restore can't replace it"),
                ))
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(err(
                    ErrorKind::BadRequest,
                    format!("{top} is a file in the way of the restore; move it first"),
                ))
            }
            Ok(_) => {}
        }
        for entry in walkdir::WalkDir::new(&dir).follow_links(false) {
            let entry = entry.map_err(|e| {
                err(
                    ErrorKind::Internal,
                    format!("Failed to read the server's world: {e}"),
                )
            })?;
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            found.push((rel, entry.file_type().is_dir()));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests;
