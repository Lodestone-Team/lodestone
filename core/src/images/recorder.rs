//! Recording content images: one per operation, written only after it succeeded.
//!
//! A Lodestone action that changes a server's content wraps its work in an [`Operation`]
//! ([`ImageService::begin`]): it holds the instance's content gate shared, so no record
//! reads its files halfway. [`Operation::succeed`] queues its label; dropping it without
//! succeeding records nothing (a failed operation's leftovers are found by the check before
//! the next start). Queued labels are recorded together once nothing more has succeeded
//! for [`MERGE_WINDOW`], or at the latest [`MAX_DEFER`] after the first: a job (Update all)
//! and changes made within a few seconds of it are one image, labelled with what each did.
//! The modpack code, which needs the image's id, records at once
//! ([`Operation::succeed_and_record`]), and so do a backup, a start and a modpack operation
//! that must see the content as it is ([`ImageService::flush`]).
//!
//! A record takes the gate alone and then the instance's lock, snapshots the server's
//! content ([`super::scope`], the policy's excludes as canonical patterns, the "content"
//! stat cache so only changed files are read, one thread while the server runs), builds
//! the image (base, sources and `env` from what is known: [`super::base`],
//! [`super::env`]), and seals it. The same contents give the same id: then nothing is
//! written but the baseline. Otherwise the manifest is saved (the store makes its objects
//! durable first, then writes it to a temporary file and renames it), then the pointer
//! and baseline ([`super::state`]), then the stat cache, then old manifests are pruned. A
//! failure before the manifest is saved writes nothing and keeps the labels queued for the
//! next try; a crash after it leaves the old pointer, which the next record moves.
//!
//! A collection running at the same time can take an object the snapshot was about to
//! name; saving then fails as missing (the store never names what it lost), and the record
//! is taken again once.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lodestone_image::manifest::{App, MAX_TEXT_LEN};
use lodestone_image::store::snapshot::{self, Snapshot, SnapshotOptions};
use lodestone_image::store::{Store, StoreError};
use lodestone_image::{DiskLayout, Entry, Image, Kind, Producer, Source};
use sha1::Digest;
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard};
use tracing::warn;

use crate::error::Error;
use crate::types::InstanceUuid;

use super::base::{BaseFile, BaseInfo};
use super::env::{self, JarFacts, ModrinthHint};
use super::excludes::canonical_excludes_where;
use super::layout::{game_of, server_layout, server_settings, InstanceContext};
use super::scope::{content_filter, exclude_reaches_content, SCOPE_RULES};
use super::state::ContentState;
use super::{blocking, ImageService, Slot};

/// Successes this close together are one image.
pub const MERGE_WINDOW: Duration = Duration::from_secs(3);
/// Queued labels are recorded at the latest this long after the first, however busy.
pub const MAX_DEFER: Duration = Duration::from_secs(30);
/// The stat cache content records keep, apart from the backups' ("data").
pub const CONTENT_STAT_CACHE: &str = "content";
/// The label of the first image of a server.
pub const INITIAL_LABEL: &str = "Content when Lodestone began recording";
/// The label of an image the before-start check records.
pub const OUTSIDE_LABEL: &str = "Changed outside Lodestone";
/// The label of an image the before-start check records when no content file changed but
/// the backup policy's excludes did (the image's `excludes` are part of its id).
pub const EXCLUDES_LABEL: &str = "Changed the backup excludes";

/// What a record did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// The instance's current content image now.
    pub id: String,
    /// The image it was recorded after, if any.
    pub parent: Option<String>,
    /// Whether a new image was written (false: the content was what `id` already had).
    pub changed: bool,
}

/// A label waiting to be recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Queued {
    pub(crate) label: String,
    /// For a job's label ([`ImageService::begin_job`]). A job holds no gate, so a record of
    /// another operation can take in its finished files before it succeeds; its own record
    /// then finds nothing changed and adds its label to the current image, but only when
    /// the entries under what the job wrote differ between the image that was current when
    /// it began and that one ([`Jobs`]). Otherwise the job changed no content (an unzip of a
    /// world, a copy into a folder the policy excludes, an install that rewrote the same
    /// bytes), whatever else was recorded meanwhile, and its label is dropped, as any
    /// operation's that changed nothing.
    pub(crate) job: Option<JobLabel>,
}

/// A job's part in its queued label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobLabel {
    /// Its id among the instance's [`Jobs`].
    pub(crate) id: u64,
    /// What it wrote ([`Operation::wrote`]): disk paths relative to the server folder,
    /// `/`-separated, a folder standing for everything in it. `None`: not known, so any
    /// content file.
    pub(crate) wrote: Option<Vec<String>>,
}

/// An instance's jobs ([`ImageService::begin_job`]) that began and whose labels aren't
/// recorded yet (nor dropped): for each, the image that was current before the first new
/// image recorded since it began.
#[derive(Debug, Default)]
pub(crate) struct Jobs {
    next: u64,
    /// `None` while no new image was recorded since the job began; then the image the
    /// first one was recorded after (`Some(None)`: there was none).
    running: HashMap<u64, Option<Option<String>>>,
}

impl Jobs {
    fn begin(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        self.running.insert(id, None);
        id
    }

    /// A record wrote a new image after `parent`.
    fn recorded(&mut self, parent: &Option<String>) {
        for since in self.running.values_mut() {
            since.get_or_insert_with(|| parent.clone());
        }
    }

    /// Forgets the job `id`; returns the image that was current before the first new image
    /// recorded since it began, if one was.
    fn end(&mut self, id: u64) -> Option<Option<String>> {
        self.running.remove(&id).flatten()
    }

    /// Forgets the jobs of labels that will never be recorded.
    pub(crate) fn forget(&mut self, labels: &[Queued]) {
        for queued in labels {
            if let Some(job) = &queued.job {
                self.running.remove(&job.id);
            }
        }
    }
}

/// Labels waiting to be recorded, and when they came.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    labels: Vec<Queued>,
    first: Option<Instant>,
    last: Option<Instant>,
    /// Whether a task is waiting to record them.
    pub(crate) worker: bool,
}

impl Pending {
    pub(crate) fn push(&mut self, label: String, job: Option<JobLabel>, now: Instant) {
        if !label.is_empty() {
            self.labels.push(Queued { label, job });
        }
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    /// When the queued labels are due: [`MERGE_WINDOW`] after the last, or [`MAX_DEFER`]
    /// after the first, whichever is sooner. `None` when nothing is queued.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        Some((last + MERGE_WINDOW).min(first + MAX_DEFER))
    }

    pub(crate) fn is_due(&self, now: Instant) -> bool {
        self.deadline().is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.first.is_none()
    }

    /// Takes the queued labels.
    pub(crate) fn take(&mut self) -> Vec<Queued> {
        self.first = None;
        self.last = None;
        std::mem::take(&mut self.labels)
    }

    /// Puts labels a failed record took back in front, due at once on the next try.
    pub(crate) fn restore(&mut self, mut labels: Vec<Queued>, now: Instant) {
        labels.append(&mut self.labels);
        self.labels = labels;
        self.first.get_or_insert(now);
        self.last.get_or_insert(now);
    }
}

/// How an operation holds the instance's content gate: the guards are only held, never
/// read.
enum Gate {
    /// Beside other operations ([`ImageService::begin`]).
    Shared(#[allow(dead_code)] OwnedRwLockReadGuard<()>),
    /// Alone ([`ImageService::begin_exclusive`]).
    Alone(#[allow(dead_code)] OwnedRwLockWriteGuard<()>),
}

/// A content-changing action in progress (see the module). Holds the instance's content
/// gate until it ends. Never begin a second operation on the same instance while holding
/// one: a record waiting between them would wait for both.
#[must_use = "an operation records nothing unless it succeeds"]
pub struct Operation {
    service: Option<Arc<ImageService>>,
    uuid: InstanceUuid,
    label: String,
    gate: Option<Gate>,
    /// For a job: its id among the instance's [`Jobs`] and what it wrote.
    job: Option<JobLabel>,
}

impl Operation {
    /// For a job: what it wrote, as disk paths relative to the server folder,
    /// `/`-separated, a folder standing for everything in it (replacing what was said
    /// before). Its label is added to an image another record took its files in with only
    /// when the entries under these differ ([`Queued::job`]). Not said, or `None`: any
    /// content file.
    /// Over [`MAX_WROTE`] paths (an unzip of a flat archive places one per top-level
    /// entry) stand for their folders instead.
    pub fn wrote(&mut self, paths: impl Into<Option<Vec<String>>>) {
        if let Some(job) = &mut self.job {
            job.wrote = bounded_wrote(paths.into());
        }
    }

    /// The operation succeeded: its label is recorded with whatever else succeeds within
    /// the merge window.
    pub fn succeed(mut self) {
        if let Some(service) = self.service.take() {
            service.queue(
                &self.uuid,
                std::mem::take(&mut self.label),
                self.job.take(),
                true,
            );
        }
    }

    /// [`Self::succeed`] with a label known only once the operation is done (an install
    /// that turned out to replace an older version is "Updated", not "Added").
    pub fn succeed_as(mut self, label: impl Into<String>) {
        self.label = label.into();
        self.succeed();
    }

    /// The operation succeeded: records now (with anything queued) and returns the image.
    /// For callers that need the id, such as the modpack history.
    pub async fn succeed_and_record(
        mut self,
        ctx: InstanceContext,
    ) -> Result<Option<Recorded>, Error> {
        let Some(service) = self.service.take() else {
            return Ok(None);
        };
        service.queue(
            &self.uuid,
            std::mem::take(&mut self.label),
            self.job.take(),
            false,
        );
        // The record takes the gate alone: let go of this operation's share first.
        drop(self.gate.take());
        service.record(ctx, None, None).await
    }
}

impl Drop for Operation {
    /// A job that ends without succeeding is forgotten.
    fn drop(&mut self) {
        if let (Some(service), Some(job)) = (&self.service, &self.job) {
            service
                .slot(&self.uuid)
                .jobs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .end(job.id);
        }
    }
}

impl ImageService {
    /// Starts an operation on the instance's content; waits only while a record of it is
    /// running. For an instance images aren't kept for, the operation does nothing.
    pub async fn begin(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        label: impl Into<String>,
    ) -> Operation {
        let label = label.into();
        if !Self::accepts(uuid) {
            return Operation {
                service: None,
                uuid: uuid.clone(),
                label,
                gate: None,
                job: None,
            };
        }
        let gate = self.slot(uuid).gate.clone().read_owned().await;
        Operation {
            service: Some(self.clone()),
            uuid: uuid.clone(),
            label,
            gate: Some(Gate::Shared(gate)),
            job: None,
        }
    }

    /// [`Self::begin`] for a long job whose length the caller doesn't control (a copy or an
    /// unzip into the server folder): it holds no gate, so it never holds up the server's
    /// records, or the operations waiting behind one, for as long as it runs. A record that
    /// runs meanwhile may take the job's files half written; the record of the job's own
    /// label, queued when it succeeds, takes them whole. Say what it wrote with
    /// [`Operation::wrote`].
    pub fn begin_job(self: &Arc<Self>, uuid: &InstanceUuid, label: impl Into<String>) -> Operation {
        let accepted = Self::accepts(uuid);
        Operation {
            service: accepted.then(|| self.clone()),
            uuid: uuid.clone(),
            label: label.into(),
            gate: None,
            job: accepted.then(|| JobLabel {
                id: self
                    .slot(uuid)
                    .jobs
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .begin(),
                wrote: None,
            }),
        }
    }

    /// [`Self::begin`] for an operation that must have the content to itself: no other
    /// operation (a Files API change) runs while it does, though a job
    /// ([`Self::begin_job`]) holds no gate and may. A modpack update or restore, which
    /// checks the files it replaces are as planned right before it moves them.
    pub async fn begin_exclusive(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        label: impl Into<String>,
    ) -> Operation {
        let label = label.into();
        let gate = if Self::accepts(uuid) {
            Some(Gate::Alone(
                self.slot(uuid).gate.clone().write_owned().await,
            ))
        } else {
            None
        };
        Operation {
            service: gate.as_ref().map(|_| self.clone()),
            uuid: uuid.clone(),
            label,
            gate,
            job: None,
        }
    }

    /// Queues a label (`job`: from a job); with `spawn`, makes sure a task records it when
    /// it is due.
    fn queue(
        self: &Arc<Self>,
        uuid: &InstanceUuid,
        label: String,
        job: Option<JobLabel>,
        spawn: bool,
    ) {
        let slot = self.slot(uuid);
        let now = self.clock.now();
        if label.is_empty() {
            // Nothing to label an image with: the job is done.
            if let Some(job) = &job {
                slot.jobs
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .end(job.id);
            }
        }
        let start = {
            let mut pending = slot.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.push(label, job, now);
            let start = spawn && !pending.worker && tokio::runtime::Handle::try_current().is_ok();
            if start {
                pending.worker = true;
            }
            start
        };
        if start {
            self.spawn_worker(uuid.clone(), slot);
        }
    }

    /// Records the instance's queued labels when they are due, until none are queued.
    fn spawn_worker(self: &Arc<Self>, uuid: InstanceUuid, slot: Arc<Slot>) {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                let deadline = {
                    let mut pending = slot.pending.lock().unwrap_or_else(|e| e.into_inner());
                    match pending.deadline() {
                        Some(deadline) => deadline,
                        None => {
                            pending.worker = false;
                            return;
                        }
                    }
                };
                this.clock.sleep_until(deadline).await;
                let due = slot
                    .pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .is_due(this.clock.now());
                if !due {
                    continue;
                }
                let Some(ctx) = this.context(&uuid).await else {
                    // The server is gone.
                    let dropped = {
                        let mut pending = slot.pending.lock().unwrap_or_else(|e| e.into_inner());
                        pending.worker = false;
                        pending.take()
                    };
                    slot.jobs
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .forget(&dropped);
                    return;
                };
                if let Err(e) = this.flush(ctx).await {
                    // The labels stay queued for the next operation, flush or start.
                    warn!("Failed to record the content of {uuid}: {}", e.source);
                    slot.pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .worker = false;
                    return;
                }
            }
        });
    }

    /// Records what is queued for the instance now, if anything (before a backup, a start
    /// or a modpack operation).
    pub async fn flush(self: &Arc<Self>, ctx: InstanceContext) -> Result<Option<Recorded>, Error> {
        self.flush_within(ctx, None).await
    }

    /// [`Self::flush`], waiting at most `wait` for the content gate (an operation in
    /// progress). Once it has the gate, the record runs to its end: a record is never cut
    /// short, which would lose the labels it took.
    pub async fn flush_within(
        self: &Arc<Self>,
        ctx: InstanceContext,
        wait: Option<Duration>,
    ) -> Result<Option<Recorded>, Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(None);
        }
        let empty = self
            .slot(&ctx.uuid)
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty();
        if empty {
            return Ok(None);
        }
        self.record(ctx, None, wait).await
    }

    /// Records the instance's content now, labelled `label` (with anything queued).
    pub async fn record_now(
        self: &Arc<Self>,
        ctx: InstanceContext,
        label: String,
    ) -> Result<Option<Recorded>, Error> {
        self.record_now_within(ctx, label, None).await
    }

    /// [`Self::record_now`], waiting at most `wait` for the content gate, like
    /// [`Self::flush_within`].
    pub async fn record_now_within(
        self: &Arc<Self>,
        ctx: InstanceContext,
        label: String,
        wait: Option<Duration>,
    ) -> Result<Option<Recorded>, Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(None);
        }
        self.record(ctx, Some(label), wait).await
    }

    /// Takes the gate alone (waiting at most `wait` for it) and records (see the module).
    /// `fallback` labels the image when nothing is queued.
    pub(crate) async fn record(
        self: &Arc<Self>,
        ctx: InstanceContext,
        fallback: Option<String>,
        wait: Option<Duration>,
    ) -> Result<Option<Recorded>, Error> {
        let slot = self.slot(&ctx.uuid);
        let gate = slot.gate.clone().write_owned();
        let _gate = match wait {
            Some(wait) => tokio::time::timeout(wait, gate).await.map_err(|_| Error {
                kind: crate::error::ErrorKind::Conflict,
                source: color_eyre::eyre::eyre!(
                    "An operation on the server's content is still running"
                ),
            })?,
            None => gate.await,
        };
        let _lock = slot.lock.clone().lock_owned().await;
        self.record_held(&slot, ctx, fallback, 0).await.map(Some)
    }

    /// [`Self::record`] for a caller holding the gate alone and the instance's lock.
    pub(crate) async fn record_held(
        &self,
        slot: &Slot,
        ctx: InstanceContext,
        fallback: Option<String>,
        threads: usize,
    ) -> Result<Recorded, Error> {
        let mut labels = slot
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if labels.is_empty() {
            labels.extend(fallback.map(|label| Queued { label, job: None }));
        }
        match self.record_locked(&ctx, &labels, threads).await {
            Ok(recorded) => {
                let mut finished = Vec::new();
                {
                    let mut jobs = slot.jobs.lock().unwrap_or_else(|e| e.into_inner());
                    if recorded.changed {
                        jobs.recorded(&recorded.parent);
                    }
                    for queued in &labels {
                        let Some(job) = &queued.job else { continue };
                        // A job whose label this image has, or one no new image was
                        // recorded since it began: nothing more to add.
                        if let Some(before) = jobs.end(job.id).filter(|_| !recorded.changed) {
                            finished.push(FinishedJob {
                                label: queued.label.clone(),
                                wrote: job.wrote.clone(),
                                before,
                            });
                        }
                    }
                }
                self.add_job_labels(&ctx, &recorded.id, finished).await;
                Ok(recorded)
            }
            Err(e) => {
                slot.pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .restore(labels, self.clock.now());
                Err(e)
            }
        }
    }

    async fn record_locked(
        &self,
        ctx: &InstanceContext,
        labels: &[Queued],
        threads: usize,
    ) -> Result<Recorded, Error> {
        let threads = if ctx.running { 1 } else { threads };
        let root = self.root.clone();
        let uuid = ctx.uuid.clone();
        let (state, base) = blocking(move || {
            Ok((
                super::state::load(&root, &uuid),
                super::base::load_base(&root, &uuid),
            ))
        })
        .await?;
        let texts: Vec<String> = labels.iter().map(|q| q.label.clone()).collect();
        let label = join_labels(&texts);
        let mut retried = false;
        loop {
            let built = self
                .build(ctx, &state, base.as_ref(), label.clone(), threads)
                .await?;
            let Built {
                image,
                snapshot,
                baseline,
                hints,
            } = built;
            let mut next = ContentState {
                current: Some(image.id.clone()),
                recorded: image.created,
                baseline,
                scope_rules: SCOPE_RULES,
                excludes: image.excludes.clone(),
                running_since: state.running_since,
                modrinth: hints,
            };
            let store = self.store();
            let id = ctx.uuid.clone();
            let image_id = image.id.clone();
            let have = blocking(move || {
                Ok(store
                    .manifest_path(id.as_ref(), &image_id)
                    .is_ok_and(|path| path.is_file()))
            })
            .await?;
            if have && state.current.as_deref() == Some(image.id.as_str()) {
                // Nothing changed: keep the image's time, take the baseline again.
                next.recorded = state.recorded;
                self.save_state(ctx, next, snapshot).await?;
                return Ok(Recorded {
                    id: image.id,
                    parent: image.parent,
                    changed: false,
                });
            }
            let store = self.store();
            let id = ctx.uuid.clone();
            let to_save = image.clone();
            let saved =
                blocking(move || Ok(store.save_manifest(id.as_ref(), &to_save).map(|_| ())))
                    .await?;
            match saved {
                Ok(()) => {}
                Err(StoreError::Missing(sha256)) if !retried => {
                    // A collection took an object the snapshot relied on: take it again.
                    warn!(
                        "An object of the content of {} was collected while it was recorded ({sha256}); recording again",
                        ctx.uuid
                    );
                    retried = true;
                    continue;
                }
                Err(e) => {
                    return Err(Error::from(color_eyre::eyre::eyre!(
                        "Failed to save the content image of {}: {e}",
                        ctx.uuid
                    )))
                }
            }
            self.save_state(ctx, next, snapshot).await?;
            let pruned = self.prune_locked(&ctx.uuid).await.unwrap_or_else(|e| {
                warn!("Failed to prune the images of {}: {}", ctx.uuid, e.source);
                0
            });
            if pruned > 0 {
                self.request_gc_after_pruning();
            }
            return Ok(Recorded {
                id: image.id,
                parent: image.parent,
                changed: true,
            });
        }
    }

    /// Saves the pointer and baseline, then the content stat cache.
    async fn save_state(
        &self,
        ctx: &InstanceContext,
        next: ContentState,
        snapshot: Snapshot,
    ) -> Result<(), Error> {
        let root = self.root.clone();
        let store = self.store();
        let uuid = ctx.uuid.clone();
        blocking(move || {
            super::state::save(&root, &uuid, &next)?;
            if let Err(e) =
                store.save_stat_cache_named(uuid.as_ref(), CONTENT_STAT_CACHE, &snapshot.cache)
            {
                // Only slower next time.
                warn!("Failed to save the content stat cache of {uuid}: {e}");
            }
            Ok(())
        })
        .await
    }

    /// A record that found nothing changed, with the jobs `jobs` a new image was recorded
    /// after they began ([`Queued::job`]): when the entries under what a job wrote differ
    /// between the image current before then and the current image `id`, a record took in
    /// the job's finished files before it succeeded, so `id` holds what it did. Those jobs'
    /// labels are added to its label (the label isn't part of the id, so the manifest is
    /// saved again under the same id), rather than lost; the others changed no content.
    /// When that can't be told (an image since pruned), the label is added. A failure is
    /// only logged: the files are recorded either way. Called with the instance's lock held.
    async fn add_job_labels(&self, ctx: &InstanceContext, id: &str, jobs: Vec<FinishedJob>) {
        if jobs.is_empty() {
            return;
        }
        let store = self.store();
        let uuid = ctx.uuid.clone();
        let id = id.to_string();
        let ctx = ctx.clone();
        let saved = blocking(move || {
            let mut image = store
                .load_manifest(uuid.as_ref(), &id)
                .map_err(|e| color_eyre::eyre::eyre!("{e}"))?;
            let layout = server_layout(&ctx.root, &ctx.level_name, &ctx.flavour)
                .ok()
                .map(|(layout, _)| layout);
            let mut earlier: HashMap<String, Option<Image>> = HashMap::new();
            let mut added = Vec::new();
            for job in jobs {
                let before = match &job.before {
                    None => Some(None),
                    Some(before) => earlier
                        .entry(before.clone())
                        .or_insert_with(|| store.load_manifest(uuid.as_ref(), before).ok())
                        .as_ref()
                        .map(Some),
                };
                let changed = match (before, &layout) {
                    (Some(before), Some(layout)) => {
                        wrote_differs(before, &image, layout, job.wrote.as_deref())
                    }
                    // Can't tell: keep the label rather than lose it.
                    _ => true,
                };
                if changed {
                    added.push(job.label);
                }
            }
            if added.is_empty() {
                return Ok(());
            }
            let texts: Vec<String> = image.label.iter().cloned().chain(added).collect();
            let label = join_labels(&texts);
            if label == image.label {
                return Ok(());
            }
            image.label = label;
            store
                .save_manifest(uuid.as_ref(), &image)
                .map(|_| ())
                .map_err(|e| Error::from(color_eyre::eyre::eyre!("{e}")))
        })
        .await;
        if let Err(e) = saved {
            warn!(
                "Failed to add what finished to the content image of {}: {}",
                ctx.uuid, e.source
            );
        }
    }

    /// Snapshots the server's content and builds its image, sealed.
    async fn build(
        &self,
        ctx: &InstanceContext,
        state: &ContentState,
        base: Option<&BaseInfo>,
        label: Option<String>,
        threads: usize,
    ) -> Result<Built, Error> {
        let (layout, excludes, snapshot, baseline) =
            take_snapshot(self.store(), ctx.clone(), threads).await?;
        let files: Vec<(String, String)> = snapshot
            .entries
            .iter()
            .map(|e| (e.path.to_string(), e.sha256.clone()))
            .collect();
        let facts = env::jar_facts(ctx, &files, &self.jar_scans).await;
        let pack_files = match base {
            Some(base) => {
                matching_base_files(&self.root, ctx, &layout, &snapshot.entries, base).await?
            }
            None => HashMap::new(),
        };
        let has_pack = base.and_then(|b| b.base.as_ref()).is_some();
        let mut hints = BTreeMap::new();
        let mut entries = Vec::with_capacity(snapshot.entries.len());
        for entry in &snapshot.entries {
            let path = entry.path.to_string();
            let fact = facts.get(&path);
            let hint = fact.and_then(|f| f.sha512.as_ref()).and_then(|sha512| {
                self.modrinth_hint(sha512)
                    .or_else(|| state.modrinth.get(sha512).cloned())
                    .map(|hint| (sha512.clone(), hint))
            });
            let pack_file = pack_files.get(&path);
            entries.push(describe(
                entry.clone(),
                fact,
                pack_file,
                has_pack,
                hint.as_ref().map(|(_, h)| h),
            ));
            if let Some((sha512, hint)) = hint {
                hints.insert(sha512, hint);
            }
        }

        let mut image = Image::new(Kind::Content, game_of(&ctx.game_version, &ctx.flavour));
        snapshot.fill(&mut image);
        image.entries = entries;
        image.settings = server_settings(&ctx.flavour);
        image.base = base.and_then(|b| b.base.clone());
        image.excludes = excludes;
        image.created = chrono::Utc::now().timestamp();
        image.producer = Producer {
            app: App::Core,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        image.label = label;
        image.normalize();
        image.seal();
        image.parent = state.current.clone().filter(|current| *current != image.id);
        image
            .validate()
            .map_err(|e| color_eyre::eyre::eyre!("The content image isn't valid: {e}"))?;
        if !snapshot.too_long.is_empty() {
            warn!(
                "{} content files of {} have paths too long to record",
                snapshot.too_long.len(),
                ctx.uuid
            );
        }
        Ok(Built {
            image,
            snapshot,
            baseline,
            hints,
        })
    }
}

/// A job whose label a record that found nothing changed took ([`Queued::job`]).
struct FinishedJob {
    label: String,
    wrote: Option<Vec<String>>,
    /// The image current before the first new image recorded since it began (`None`:
    /// there was none).
    before: Option<String>,
}

/// The most paths a job's [`JobLabel::wrote`] keeps: more stand for the folders they are in,
/// and when those are still more, for any content file. It is queued, and every record
/// that takes the job's label in checks image entries against it.
const MAX_WROTE: usize = 1024;

/// `paths` for [`JobLabel::wrote`], at most [`MAX_WROTE`] of them (see there).
fn bounded_wrote(paths: Option<Vec<String>>) -> Option<Vec<String>> {
    let paths = paths?;
    if paths.len() <= MAX_WROTE {
        return Some(paths);
    }
    let folders: Vec<String> = paths
        .iter()
        .map(|path| {
            let path = path.trim_matches('/');
            path.rsplit_once('/').map_or("", |(folder, _)| folder)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect();
    (folders.len() <= MAX_WROTE).then_some(folders)
}

/// What a job wrote (disk paths, a folder standing for everything in it), to look up whether
/// it is or holds a disk path by the path and its folders: as many lookups as the path is
/// deep, however many it wrote. Letter case is ignored (ASCII, as `scope::same` does): a
/// request's path keeps the case it was typed in, the image the case on disk, and on a disk
/// that ignores case both name the same file. Erring this way only keeps a label.
struct WroteSet {
    /// A path is the server folder itself: it covers everything.
    all: bool,
    paths: HashSet<String>,
}

impl WroteSet {
    fn new(wrote: &[String]) -> Self {
        let paths: HashSet<String> = wrote
            .iter()
            .map(|path| path.trim_matches('/').to_ascii_lowercase())
            .collect();
        Self {
            all: paths.contains(""),
            paths,
        }
    }

    /// Whether one of the paths is or holds `disk`.
    fn covers(&self, disk: &str) -> bool {
        if self.all {
            return true;
        }
        let disk = disk.to_ascii_lowercase();
        if self.paths.contains(disk.as_str()) {
            return true;
        }
        let mut folder = disk.as_str();
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if self.paths.contains(parent) {
                return true;
            }
            folder = parent;
        }
        false
    }
}

/// Whether the entries under `wrote` (disk paths, a folder standing for everything in it;
/// `None`: all) differ between `before` (`None`: no image) and `now`.
fn wrote_differs(
    before: Option<&Image>,
    now: &Image,
    layout: &DiskLayout,
    wrote: Option<&[String]>,
) -> bool {
    let wrote = wrote.map(WroteSet::new);
    let under = |entry: &Entry| match &wrote {
        None => true,
        Some(paths) => match layout.to_disk(&entry.path) {
            Ok(disk) => paths.covers(&disk),
            // Can't tell where it is: count it.
            Err(_) => true,
        },
    };
    let side = |image: Option<&Image>| -> BTreeMap<String, (String, u32)> {
        image
            .into_iter()
            .flat_map(|image| image.entries.iter())
            .filter(|entry| under(entry))
            .map(|entry| (entry.path.to_string(), (entry.sha256.clone(), entry.mode)))
            .collect()
    };
    side(before) != side(Some(now))
}

struct Built {
    image: Image,
    snapshot: Snapshot,
    baseline: BTreeMap<String, (u64, i64)>,
    hints: BTreeMap<String, ModrinthHint>,
}

/// The server's disk layout and canonical excludes, for its content: blocking. Only the
/// excludes that can reach a content file ([`exclude_reaches_content`]), chosen before the
/// patterns are cut to an image's budgets: a world's, say, change neither what a content
/// image holds nor its id, nor which content patterns fit. They are sorted and without
/// duplicates, as an image keeps them, so they compare equal to the ones a record saved
/// with its baseline.
pub(crate) fn content_scope(ctx: &InstanceContext) -> Result<(DiskLayout, Vec<String>), Error> {
    let (layout, _) = server_layout(&ctx.root, &ctx.level_name, &ctx.flavour)?;
    // Left out before the patterns are cut to an image's budgets, so a world's patterns
    // never push out one that reaches content.
    let mut excludes = match &layout {
        DiskLayout::Server(server) => {
            canonical_excludes_where(&ctx.policy_exclude, server, exclude_reaches_content).canonical
        }
        DiskLayout::Client => Vec::new(),
    };
    excludes.sort();
    excludes.dedup();
    Ok((layout, excludes))
}

/// Walks and stores the server's content files (see the module). Also returns the baseline:
/// every content file the walk found, with the size and time it found, including those the
/// snapshot then couldn't read, so the check before a start compares like with like.
async fn take_snapshot(
    store: Arc<Store>,
    ctx: InstanceContext,
    threads: usize,
) -> Result<
    (
        DiskLayout,
        Vec<String>,
        Snapshot,
        BTreeMap<String, (u64, i64)>,
    ),
    Error,
> {
    blocking(move || {
        let (layout, excludes) = content_scope(&ctx)?;
        let cache = store.load_stat_cache_named(ctx.uuid.as_ref(), CONTENT_STAT_CACHE);
        let options = SnapshotOptions {
            excludes: &excludes,
            cache: Some(&cache),
            threads,
            filter: Some(&content_filter),
            ..SnapshotOptions::default()
        };
        let plan = snapshot::plan(&store, &ctx.root, &layout, &options)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to read the server's content: {e}"))?;
        crate::util::disk::require_free_space(
            store.root(),
            plan.bytes_to_read(),
            "to record the server's content",
        )?;
        let baseline = plan
            .cached
            .iter()
            .map(|(file, _)| file)
            .chain(plan.to_read.iter())
            .map(|file| (file.disk.clone(), (file.size, file.modified_ns)))
            .collect();
        let snapshot = snapshot::run(&store, plan, &options, &|_, _| {})
            .map_err(|e| color_eyre::eyre::eyre!("Failed to record the server's content: {e}"))?;
        Ok((layout, excludes, snapshot, baseline))
    })
    .await
}

/// The pack files among `entries` that still have the pack's bytes, by canonical path. A
/// mod the Mods tab disabled is the pack's file under its enabled name. Matched by SHA-256
/// when `pack.json` has it; else by size and SHA-1, read from disk once, after which the
/// file's SHA-256 is written into `pack.json` ([`learn_sha256`]) so later records read
/// nothing. A pack file the user changed (same size, other bytes) is read at each record.
async fn matching_base_files(
    store_root: &Path,
    ctx: &InstanceContext,
    layout: &DiskLayout,
    entries: &[Entry],
    base: &BaseInfo,
) -> Result<HashMap<String, BaseFile>, Error> {
    let candidates: Vec<(String, String, String, Entry, BaseFile)> = entries
        .iter()
        .filter_map(|entry| {
            let disk = layout.to_disk(&entry.path).ok()?;
            let (key, file) = match base.files.get(&disk) {
                Some(file) => (disk.clone(), file),
                None => {
                    let enabled = crate::mods::enabled_name(&disk);
                    let file = base.files.get(&enabled)?;
                    (enabled, file)
                }
            };
            Some((
                entry.path.to_string(),
                disk,
                key,
                entry.clone(),
                file.clone(),
            ))
        })
        .collect();
    let root = ctx.root.clone();
    let uuid = ctx.uuid.clone();
    let store_root = store_root.to_path_buf();
    let pack_base = base.base.clone();
    blocking(move || {
        let mut matched = HashMap::new();
        let mut learned = Vec::new();
        for (path, disk, key, entry, mut file) in candidates {
            let same = match &file.sha256 {
                Some(sha256) => *sha256 == entry.sha256,
                None => {
                    let same = file.size.is_none_or(|size| size == entry.size)
                        && sha1_of(&lodestone_image::layout::join_disk(&root, &disk))
                            .is_some_and(|sha1| sha1 == file.sha1);
                    if same {
                        learned.push((key, entry.sha256.clone(), entry.size));
                        file.sha256 = Some(entry.sha256.clone());
                        file.size = Some(entry.size);
                    }
                    same
                }
            };
            if same {
                matched.insert(path, file);
            }
        }
        if !learned.is_empty() {
            learn_sha256(&store_root, &uuid, pack_base.as_ref(), &learned);
        }
        Ok(matched)
    })
    .await
}

/// Writes the SHA-256 and size of pack files a record matched by SHA-1 (`learned`: the
/// `pack.json` key, SHA-256, size) into the instance's `pack.json`, when it still names the
/// same pack and the file has no SHA-256 yet. The same SHA-1 is the same bytes, so this
/// holds for as long as the entry does. Runs inside a record, which has the content gate to
/// itself, so no `pack.json` writer (each runs inside a content operation) is in between.
/// A failure is only logged: the next record reads the files again. Blocking.
fn learn_sha256(
    store_root: &Path,
    uuid: &InstanceUuid,
    pack: Option<&lodestone_image::Base>,
    learned: &[(String, String, u64)],
) {
    let Some(mut info) = super::base::load_base(store_root, uuid) else {
        return;
    };
    if info.base.as_ref() != pack {
        return;
    }
    let mut changed = false;
    for (key, sha256, size) in learned {
        if let Some(file) = info.files.get_mut(key) {
            if file.sha256.is_none() {
                file.sha256 = Some(sha256.clone());
                file.size = Some(*size);
                changed = true;
            }
        }
    }
    if changed {
        if let Err(e) = super::base::save_base(store_root, uuid, &info) {
            warn!(
                "Failed to note the pack files' hashes of {uuid}: {}",
                e.source
            );
        }
    }
}

fn sha1_of(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = sha1::Sha1::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// An entry with its source and `env` from what is known of the file.
fn describe(
    mut entry: Entry,
    facts: Option<&JarFacts>,
    pack_file: Option<&BaseFile>,
    has_pack: bool,
    hint: Option<&ModrinthHint>,
) -> Entry {
    let sha512 = facts.and_then(|f| f.sha512.clone());
    let pack_download = pack_file.and_then(|file| {
        let source = Source::modrinth_from_url(file.modrinth_url.as_deref()?)?;
        let sha512 = file.sha512.clone().filter(|s| env::is_sha512(s))?;
        Some((source, sha512))
    });
    let hinted_download = hint.and_then(|hint| {
        let source = Source::modrinth_from_url(hint.url.as_deref()?)?;
        Some((source, sha512.clone()?))
    });
    if let Some((source, sha512)) = pack_download.or(hinted_download) {
        entry.source = source;
        entry.sha512 = Some(sha512);
        entry.sha1 = pack_file
            .map(|f| f.sha1.clone())
            .filter(|sha1| sha1.len() == 40 && sha1.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|sha1| sha1.to_ascii_lowercase());
    } else if pack_file.is_some() && has_pack {
        entry.source = Source::Base {};
    } else {
        entry.source = Source::Object {};
    }
    let inputs = env::inputs(facts, pack_file.and_then(|f| f.env), hint);
    let (resolved, from) = lodestone_image::env::resolve(&inputs);
    entry.env = resolved;
    entry.env_from = from;
    entry
}

/// The image's label: each operation's, in order, without repeats in a row, joined with
/// "; " and cut to what an image may hold.
pub(crate) fn join_labels(labels: &[String]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for label in labels {
        let clean: String = label
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let clean = clean.trim().to_string();
        if !clean.is_empty() && parts.last() != Some(&clean) {
            parts.push(clean);
        }
    }
    if parts.is_empty() {
        return None;
    }
    let joined = parts.join("; ");
    if joined.chars().count() <= MAX_TEXT_LEN {
        return Some(joined);
    }
    let mut cut: String = joined.chars().take(MAX_TEXT_LEN - 1).collect();
    cut.push('…');
    Some(cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_join_and_fit() {
        assert_eq!(join_labels(&[]), None);
        assert_eq!(
            join_labels(&["Added JEI".into(), "Added JEI".into(), "Removed X\n".into()]),
            Some("Added JEI; Removed X".into())
        );
        let long: Vec<String> = (0..500).map(|n| format!("Updated mod {n}")).collect();
        let joined = join_labels(&long).unwrap();
        assert_eq!(joined.chars().count(), MAX_TEXT_LEN);
        assert!(joined.ends_with('…'));
    }

    #[test]
    fn pending_merges_within_the_window_and_caps() {
        let start = Instant::now();
        let mut pending = Pending::default();
        assert_eq!(pending.deadline(), None);
        pending.push("a".into(), None, start);
        assert_eq!(pending.deadline(), Some(start + MERGE_WINDOW));
        pending.push("b".into(), Some(job(0)), start + Duration::from_secs(2));
        assert_eq!(
            pending.deadline(),
            Some(start + Duration::from_secs(2) + MERGE_WINDOW)
        );
        assert!(!pending.is_due(start + Duration::from_secs(4)));
        assert!(pending.is_due(start + Duration::from_secs(5)));
        // However busy, the first label waits at most MAX_DEFER.
        let mut busy = Pending::default();
        for second in 0..40 {
            busy.push(
                format!("{second}"),
                None,
                start + Duration::from_secs(second),
            );
        }
        assert_eq!(busy.deadline(), Some(start + MAX_DEFER));
        assert_eq!(busy.take().len(), 40);
        assert!(busy.is_empty());
        // Labels a failed record took go back in front.
        let queued = |label: &str, job| Queued {
            label: label.into(),
            job,
        };
        busy.push("later".into(), Some(job(0)), start);
        busy.restore(vec![queued("first", None)], start);
        assert_eq!(
            busy.take(),
            vec![queued("first", None), queued("later", Some(job(0)))]
        );
    }

    fn job(id: u64) -> JobLabel {
        JobLabel { id, wrote: None }
    }

    fn covers(path: &str, disk: &str) -> bool {
        WroteSet::new(&[path.to_string()]).covers(disk)
    }

    #[test]
    fn a_folder_a_job_wrote_covers_what_is_in_it() {
        assert!(covers("", "server.properties"));
        assert!(covers("/", "server.properties"));
        assert!(covers("world", "world/region/r.0.0.mca"));
        assert!(covers("world/", "world/level.dat"));
        assert!(covers("mods/a.jar", "mods/a.jar"));
        assert!(!covers("world", "world_nether/level.dat"));
        assert!(!covers("mods/a.jar", "mods/a.jar.disabled"));
        assert!(!covers("mods/a.jar", "mods"));
        assert!(!covers("config/x", "config/xy/z.toml"));
        let set = ["mods/a.jar".to_string(), "config/sub".to_string()];
        let set = WroteSet::new(&set);
        assert!(set.covers("config/sub/deep/c.toml"));
        assert!(set.covers("mods/a.jar"));
        assert!(!set.covers("config/other.toml"));
        assert!(!set.covers("mods/b.jar"));
        // a request typed `Config`, the folder on disk is `config` (or `Mods`, from "mods")
        assert!(covers("Config/a.toml", "config/a.toml"));
        assert!(covers("Config", "config/sub/a.toml"));
        assert!(covers("mods/x.jar", "Mods/x.jar"));
        assert!(!covers("Config/a.toml", "config/b.toml"));
    }

    #[test]
    fn many_paths_a_job_wrote_stand_for_their_folders() {
        let few: Vec<String> = (0..MAX_WROTE).map(|i| format!("config/{i}.toml")).collect();
        assert_eq!(bounded_wrote(Some(few.clone())), Some(few));
        assert_eq!(bounded_wrote(None), None);
        // a flat archive unzipped into config/ and the server folder
        let flat: Vec<String> = (0..=MAX_WROTE)
            .map(|i| format!("config/{i}.toml"))
            .collect();
        assert_eq!(bounded_wrote(Some(flat)), Some(vec!["config".to_string()]));
        let flat: Vec<String> = (0..=MAX_WROTE).map(|i| format!("{i}.txt")).collect();
        assert_eq!(bounded_wrote(Some(flat)), Some(vec![String::new()]));
        // too many folders: any content file
        let spread: Vec<String> = (0..=MAX_WROTE).map(|i| format!("d{i}/f")).collect();
        assert_eq!(bounded_wrote(Some(spread)), None);
    }

    /// Comparing an image under many paths a job wrote looks each entry up by its folders,
    /// rather than against every path: a record holds the content gate while it does.
    #[test]
    fn what_many_paths_a_job_wrote_hold_is_found_quickly() {
        let layout =
            DiskLayout::Server(lodestone_image::ServerLayout::new("world", [], true).unwrap());
        let game = lodestone_image::Game {
            minecraft: "1.21.1".into(),
            loader: lodestone_image::Loader::Fabric,
            loader_version: Some("0.16.10".into()),
        };
        let image = |n: usize, changed: Option<usize>| {
            let mut image = Image::new(Kind::Content, game.clone());
            image.entries = (0..n)
                .map(|i| {
                    let text = format!("config/f{i}.toml");
                    let sha = if Some(i) == changed { "b" } else { "a" }.repeat(64);
                    Entry::new(
                        lodestone_image::CanonicalPath::new(text).unwrap(),
                        1,
                        sha,
                        Source::Object {},
                    )
                })
                .collect();
            image
        };
        let before = image(5_000, None);
        let now = image(5_000, Some(4_999));
        // what an unzip of a flat archive placed in the server folder, none of it content
        let wrote: Vec<String> = (0..200_000).map(|i| format!(".lodestone-x{i}")).collect();
        let start = Instant::now();
        assert!(!wrote_differs(Some(&before), &now, &layout, Some(&wrote)));
        let mut wrote = wrote;
        wrote.push("config/f4999.toml".into());
        assert!(wrote_differs(Some(&before), &now, &layout, Some(&wrote)));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        );
    }
}
