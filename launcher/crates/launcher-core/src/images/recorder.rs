//! Recording content images: one per operation, written only after it succeeded.
//!
//! A Launcher action that changes an instance's content begins an [`Operation`]
//! ([`Images::begin`]) right before it changes files, in its background task, and ends it
//! with [`Operation::finish`] (or [`Operation::succeed`]) once it did. Success queues its
//! label; dropping it without succeeding records nothing. An action that can fail part way
//! after changing files (an install that placed a dependency, then failed or was
//! cancelled) succeeds all the same, with a label saying what it did: the record runs only
//! once the action has ended, and when it changed nothing, the label is dropped (below). A
//! job is one operation: Update all begins one before its first update, notes each
//! update's label ([`Operation::note`]) and succeeds once at its end, however long it ran.
//! Queued labels are recorded together once nothing more has succeeded for
//! [`super::MERGE_WINDOW`], or at the latest [`super::MAX_DEFER`] after the first: the
//! changes made within a few seconds of each other are one image, labelled with what each
//! did. A new instance is
//! recorded at once ([`Images::record_now`]), and so is anything a caller needs the id of
//! ([`Operation::succeed_and_record`], [`Images::flush`]).
//!
//! **No gate.** Unlike the core, operations hold nothing a record waits for. Launcher
//! downloads land under `.lodestone-*` names (never content) and are renamed into place,
//! and the app refuses content changes while the game runs, so a record never reads a file
//! half written; a file that changes while it is read is withheld as `kept_changing`. A
//! record that runs during a long operation (a pack update, Update all) may take its
//! in-between state under another operation's label; that operation's own record then
//! takes the final state. An operation notes the instance's image generation when it
//! begins ([`Queued::generation`]): when its record finds nothing changed but a new image
//! was written since it began, that image may hold its files, so its label is added to the
//! current image (saved again under the same id: the label isn't part of the id) rather
//! than lost. Otherwise it changed nothing, and its label is dropped.
//!
//! A record holds the instance's record lock, snapshots its content ([`super::scope`],
//! the "content" stat cache so only changed files are read), builds the image
//! ([`super::context`]: game, base, settings, sources and sides), and seals it. The same
//! contents give the same id: then nothing is written but the baseline. Otherwise, in this
//! order: the manifest (the store makes its objects durable first, then writes it to a
//! temporary file and renames it), the pointer and baseline ([`super::state`]), the stat
//! cache, then pruning ([`super::retention`]). An instance's first record (no state file
//! yet) writes the state first, naming only the instance (`instance_created`), so the
//! manifests that follow are known as this instance's. A failure before the manifest is saved writes nothing more and
//! keeps the labels queued for a later try; a crash after it leaves the old pointer (or
//! none), which the next record moves: to the image the crash left when the contents are
//! still its, keeping its label. A collection running at the same time can take
//! an object the snapshot was about to name; saving then fails as missing (the store never
//! names what it lost), and the record is taken again once.
//!
//! The merged records run on one thread ("lodestone-images"). It leaves an instance alone
//! while its game session lasts ([`super::Slot::session`]: the game rewrites configs) and
//! while a "Find it for me" search is saved (it renames mods on purpose), and records it
//! once that ends. It sleeps until woken while nothing is queued, so an idle Launcher has
//! no timer of its own.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lodestone_image::lineage::join_labels;
use lodestone_image::manifest::App;
use lodestone_image::store::StoreError;
use lodestone_image::store::snapshot::{self, Candidate, Snapshot, SnapshotOptions};
use lodestone_image::{DiskLayout, Image, Kind, Producer};

use super::context::InstanceContext;
use super::scope::SCOPE_RULES;
use super::state::{self, Baseline, ContentState};
use super::{ImageError, Images, Slot, Timing, WeakImages, Worker};

/// The stat cache content records keep.
pub const CONTENT_STAT_CACHE: &str = "content";
/// How long a record that failed waits before the merged records' thread tries again.
pub const RETRY_AFTER: Duration = Duration::from_secs(60);
/// Room a record leaves on the disk beyond what it will store.
const SPARE_BYTES: u64 = 256 << 20;
/// How often the merged records' thread looks whether a "Find it for me" search ended,
/// while labels wait for one (nothing wakes it when the search's file goes).
const FINDER_CHECK: Duration = Duration::from_secs(5);

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
    /// The instance's image generation ([`Slot::generation`]) when the operation began.
    /// Lower than the generation at its record means a new image was written while it ran,
    /// which may hold its files (see the module).
    pub(crate) generation: u64,
}

/// Labels waiting to be recorded, and when they came.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    labels: Vec<Queued>,
    first: Option<Instant>,
    last: Option<Instant>,
    /// After a failed record: not before then.
    not_before: Option<Instant>,
}

impl Pending {
    pub(crate) fn push(&mut self, label: String, generation: u64, now: Instant) {
        if !label.trim().is_empty() {
            self.labels.push(Queued { label, generation });
        }
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    /// When the queued labels are due: `timing.merge` after the last, or
    /// `timing.max_defer` after the first, whichever is sooner (and not before a failed
    /// record's wait is over). `None` when nothing is queued.
    pub(crate) fn deadline(&self, timing: Timing) -> Option<Instant> {
        let (first, last) = (self.first?, self.last?);
        let due = (last + timing.merge).min(first + timing.max_defer);
        Some(
            self.not_before
                .map_or(due, |not_before| due.max(not_before)),
        )
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.first.is_none()
    }

    /// Takes the queued labels.
    pub(crate) fn take(&mut self) -> Vec<Queued> {
        self.first = None;
        self.last = None;
        self.not_before = None;
        std::mem::take(&mut self.labels)
    }

    /// Puts labels a failed record took back in front, due again after [`RETRY_AFTER`].
    /// Without any, nothing: an empty queue would hold the check before a launch to what
    /// is queued, and the change it should find would be folded into the baseline.
    pub(crate) fn restore(&mut self, mut labels: Vec<Queued>, now: Instant) {
        if labels.is_empty() {
            return;
        }
        labels.append(&mut self.labels);
        self.labels = labels;
        self.first.get_or_insert(now);
        self.last.get_or_insert(now);
        self.not_before = Some(now + RETRY_AFTER);
    }
}

/// A content-changing action in progress (see the module). It holds nothing: beginning
/// and succeeding never block or touch the disk, so both are safe on the window's thread.
#[must_use = "an operation records nothing unless it succeeds"]
pub struct Operation {
    images: Option<Arc<Images>>,
    instance: String,
    label: String,
    /// What each step of a job did ([`Self::note`]), after `label`.
    notes: Vec<String>,
    generation: u64,
}

impl Operation {
    /// The operation succeeded: its label (with its notes) is recorded with whatever else
    /// succeeds within the merge window.
    pub fn succeed(mut self) {
        if let Some(images) = self.images.take() {
            let label = self.take_label();
            images.queue(&self.instance, label, self.generation);
        }
    }

    /// Notes what one step of a job did (one update of Update all), for the label the job
    /// succeeds with: the label and the notes, joined.
    pub fn note(&mut self, label: impl Into<String>) {
        self.notes.push(label.into());
    }

    fn take_label(&mut self) -> String {
        let texts: Vec<String> = std::iter::once(std::mem::take(&mut self.label))
            .chain(std::mem::take(&mut self.notes))
            .collect();
        join_labels(&texts).unwrap_or_default()
    }

    /// [`Self::succeed`] with a label known only once the operation is done (an install
    /// that turned out to replace an older version is "Updated", not "Added").
    pub fn succeed_as(mut self, label: impl Into<String>) {
        self.label = label.into();
        self.succeed();
    }

    /// Succeeds when `result` is `Ok`; otherwise the operation records nothing.
    pub fn finish<T, E>(self, result: &Result<T, E>) {
        if result.is_ok() {
            self.succeed();
        }
    }

    /// The operation succeeded: records now (with anything queued) and returns the image.
    /// Blocking: for a background thread.
    pub fn succeed_and_record(mut self) -> Result<Option<Recorded>, ImageError> {
        let Some(images) = self.images.take() else {
            return Ok(None);
        };
        let label = self.take_label();
        images
            .slot(&self.instance)
            .pending()
            .push(label, self.generation, Instant::now());
        images.flush(&self.instance)
    }

    /// The instance it is of.
    pub fn instance(&self) -> &str {
        &self.instance
    }
}

impl Images {
    /// Starts an operation on the instance's content, labelled `label` when it succeeds.
    /// Never blocks and reads nothing from the disk. For an instance images aren't kept
    /// for, the operation does nothing.
    pub fn begin(self: &Arc<Self>, instance: &str, label: impl Into<String>) -> Operation {
        let label = label.into();
        if !self.accepts(instance) {
            return Operation {
                images: None,
                instance: instance.to_string(),
                label,
                notes: Vec::new(),
                generation: 0,
            };
        }
        let generation = self.slot(instance).generation.load(Ordering::SeqCst);
        Operation {
            images: Some(self.clone()),
            instance: instance.to_string(),
            label,
            notes: Vec::new(),
            generation,
        }
    }

    /// Queues a label and wakes the merged records' thread.
    fn queue(self: &Arc<Self>, instance: &str, label: String, generation: u64) {
        self.slot(instance)
            .pending()
            .push(label, generation, Instant::now());
        self.wake_worker();
    }

    /// Records what is queued for the instance now, if anything. Blocking.
    pub fn flush(&self, instance: &str) -> Result<Option<Recorded>, ImageError> {
        if !self.accepts(instance) {
            return Ok(None);
        }
        let slot = self.slot(instance);
        if slot.pending().is_empty() {
            return Ok(None);
        }
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        self.record_held(&slot, instance, None)
    }

    /// [`Self::flush`], unless a record of the instance is running: then nothing (its
    /// labels stay queued).
    fn flush_unless_busy(&self, instance: &str) -> Result<Option<Recorded>, ImageError> {
        let slot = self.slot(instance);
        if slot.pending().is_empty() {
            return Ok(None);
        }
        let _record = match slot.record.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Err(ImageError::Busy),
        };
        let _writing = self.enter()?;
        self.record_held(&slot, instance, None)
    }

    /// Records the instance's content now, labelled `label` (with anything queued): for a
    /// new instance, once its files are in place. Blocking.
    pub fn record_now(&self, instance: &str, label: &str) -> Result<Option<Recorded>, ImageError> {
        if !self.accepts(instance) {
            return Ok(None);
        }
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        self.record_held(&slot, instance, Some(label))
    }

    /// Records what is queued for every instance whose game isn't running, waiting at most
    /// `max`: before the Launcher quits or moves its data folder, which would otherwise lose
    /// the labels of the last few seconds. Skips an instance whose "Find it for me" search
    /// is saved (its mods are renamed on purpose) or whose record is running (the first
    /// record of a big pack can take minutes): their changes are found by the check before
    /// the next launch. A record still running at `max` goes on in the background; cut off
    /// by the process ending, it is like a crash (see the module).
    pub fn settle(self: &Arc<Self>, max: Duration) {
        let (done, wait) = std::sync::mpsc::channel();
        let this = self.clone();
        let spawned = std::thread::Builder::new()
            .name("lodestone-images-settle".into())
            .spawn(move || {
                for (id, slot) in this.slots() {
                    if slot.in_session() || this.suspended() || this.finder_running(&id) {
                        continue;
                    }
                    match this.flush_unless_busy(&id) {
                        Ok(_) | Err(ImageError::Busy) => {}
                        Err(e) => super::warn("images.record", &id, &e.to_string()),
                    }
                }
                let _ = done.send(());
            });
        if spawned.is_ok() {
            let _ = wait.recv_timeout(max);
        }
    }

    /// Takes the queued labels and records (see the module), for a caller holding the
    /// instance's record lock. `label` follows the queued labels in the image's label, and
    /// makes a record happen when nothing is queued (`""` records with no label of its
    /// own); with neither, nothing is recorded. It is never added to an image that holds
    /// the same contents already. On failure the queued labels go back on the queue, unless
    /// the instance is gone.
    pub(crate) fn record_held(
        &self,
        slot: &Slot,
        instance: &str,
        label: Option<&str>,
    ) -> Result<Option<Recorded>, ImageError> {
        let labels = slot.pending().take();
        if labels.is_empty() && label.is_none() {
            return Ok(None);
        }
        match self.record_locked(slot, instance, &labels, label) {
            Ok(recorded) => Ok(Some(recorded)),
            Err(ImageError::Gone(id)) => Err(ImageError::Gone(id)),
            Err(e) => {
                slot.pending().restore(labels, Instant::now());
                Err(e)
            }
        }
    }

    fn record_locked(
        &self,
        slot: &Slot,
        instance: &str,
        labels: &[Queued],
        own: Option<&str>,
    ) -> Result<Recorded, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let loaded = self.guarded(&ctx)?;
        let mut state = loaded.state;
        if !loaded.present {
            // The first record: the state names the instance before any manifest is
            // written, so a crash after the manifest leaves images the guard knows as this
            // instance's, not an earlier one's (see [`state::belongs_elsewhere`]).
            state.instance_created = Some(ctx.instance.created);
            state::save(self.store(), instance, &state)?;
        }
        let texts: Vec<String> = labels
            .iter()
            .map(|q| q.label.clone())
            .chain(own.map(str::to_string))
            .collect();
        let label = join_labels(&texts);
        let mut retried = false;
        loop {
            let (image, snapshot, baseline) = self.build(&ctx, &state, label.clone())?;
            let next = ContentState {
                current: Some(image.id.clone()),
                recorded: image.created,
                baseline,
                scope_rules: SCOPE_RULES,
                game: Some(image.game.clone()),
                // Only a session that runs now keeps it: one no end was seen of is over
                // once a record has taken the content again.
                running_since: state.running_since.filter(|_| slot.in_session()),
                instance_created: Some(ctx.instance.created),
            };
            let have = self
                .store()
                .manifest_path(instance, &image.id)
                .is_ok_and(|path| path.is_file());
            let same = state.current.as_deref() == Some(image.id.as_str());
            if have && (same || state.current.is_none()) {
                // Nothing changed: keep the image's time, take the baseline again. With no
                // pointer, the image is the one a first record saved before it crashed (or
                // lost its state): the pointer moves to it, with its own label and time.
                let recorded = if same {
                    state.recorded
                } else {
                    self.store()
                        .load_header(instance, &image.id)
                        .map_or(image.created, |header| header.created)
                };
                self.save_state(instance, &ContentState { recorded, ..next }, &snapshot)?;
                self.carry_labels(slot, instance, &image.id, labels);
                return Ok(Recorded {
                    id: image.id,
                    parent: image.parent,
                    changed: false,
                });
            }
            match self.store().save_manifest(instance, &image) {
                Ok(_) => {}
                Err(StoreError::Missing(sha256)) if !retried => {
                    // A collection took an object the snapshot relied on: take it again.
                    super::warn(
                        "images.record",
                        instance,
                        &format!(
                            "an object was collected while recording ({sha256}); recording again"
                        ),
                    );
                    retried = true;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
            slot.generation.fetch_add(1, Ordering::SeqCst);
            self.save_state(instance, &next, &snapshot)?;
            match self.prune_locked(instance, next.current.as_deref()) {
                Ok(0) => {}
                Ok(_) => self.request_gc_after_pruning(),
                Err(e) => super::warn("images.prune", instance, &e.to_string()),
            }
            return Ok(Recorded {
                id: image.id,
                parent: image.parent,
                changed: true,
            });
        }
    }

    /// The instance's state, after the id-reuse guard: images under its id that belong to
    /// an earlier instance of the same name ([`state::belongs_elsewhere`]) are deleted
    /// first. The caller holds the record lock.
    pub(crate) fn guarded_state(&self, ctx: &InstanceContext) -> Result<ContentState, ImageError> {
        Ok(self.guarded(ctx)?.state)
    }

    /// [`Self::guarded_state`], with whether a state file is there (none after the guard
    /// deleted an earlier instance's images).
    fn guarded(&self, ctx: &InstanceContext) -> Result<state::Loaded, ImageError> {
        let loaded = state::load_present(self.store(), &ctx.id);
        let has_manifests = !self.store().manifest_ids(&ctx.id)?.is_empty();
        if state::belongs_elsewhere(&loaded, has_manifests, ctx.instance.created) {
            super::warn(
                "images.reused",
                &ctx.id,
                "the images under this id are an earlier instance's; deleting them",
            );
            self.wipe_locked(&ctx.id)?;
            return Ok(state::Loaded {
                state: ContentState::default(),
                present: false,
            });
        }
        Ok(loaded)
    }

    /// The labels of operations a new image was written during (see the module), added to
    /// the current image `id`'s label. A failure is only noted: the files are recorded
    /// either way.
    fn carry_labels(&self, slot: &Slot, instance: &str, id: &str, labels: &[Queued]) {
        let now = slot.generation.load(Ordering::SeqCst);
        let carried: Vec<String> = labels
            .iter()
            .filter(|q| q.generation < now)
            .map(|q| q.label.clone())
            .collect();
        if carried.is_empty() {
            return;
        }
        let saved = (|| -> Result<(), ImageError> {
            let mut image = self.store().load_manifest(instance, id)?;
            let texts: Vec<String> = image.label.iter().cloned().chain(carried).collect();
            let label = join_labels(&texts);
            if label != image.label {
                image.label = label;
                self.store().save_manifest(instance, &image)?;
            }
            Ok(())
        })();
        if let Err(e) = saved {
            super::warn("images.record", instance, &e.to_string());
        }
    }

    /// Saves the pointer and baseline, then the content stat cache.
    fn save_state(
        &self,
        instance: &str,
        next: &ContentState,
        snapshot: &Snapshot,
    ) -> Result<(), ImageError> {
        state::save(self.store(), instance, next)?;
        if let Err(e) =
            self.store()
                .save_stat_cache_named(instance, CONTENT_STAT_CACHE, &snapshot.cache)
        {
            // Only slower next time.
            super::warn("images.record", instance, &e.to_string());
        }
        Ok(())
    }

    /// Snapshots the instance's content and builds its image, sealed. Also returns the
    /// baseline: every content file the walk found, with the size and time it found,
    /// including those the snapshot then couldn't read, so the check before a launch
    /// compares like with like.
    fn build(
        &self,
        ctx: &InstanceContext,
        state: &ContentState,
        label: Option<String>,
    ) -> Result<(Image, Snapshot, Baseline), ImageError> {
        let store = self.store();
        let cache = store.load_stat_cache_named(&ctx.id, CONTENT_STAT_CACHE);
        let filter = |candidate: &Candidate| ctx.scope.accepts(candidate);
        let options = SnapshotOptions {
            cache: Some(&cache),
            filter: Some(&filter),
            ..SnapshotOptions::default()
        };
        let plan = snapshot::plan(store, &ctx.dir, &DiskLayout::Client, &options)?;
        if let Some(free) = self.free_space() {
            let needed = plan.bytes_to_read().saturating_add(SPARE_BYTES);
            if needed > free {
                return Err(ImageError::Invalid(format!(
                    "Recording the instance's content needs {} MB and the disk has {} MB free",
                    needed / 1_000_000,
                    free / 1_000_000
                )));
            }
        }
        let baseline = plan
            .cached
            .iter()
            .map(|(file, _)| file)
            .chain(plan.to_read.iter())
            .map(|file| (file.disk.clone(), (file.size, file.modified_ns)))
            .collect();
        let snapshot = snapshot::run(store, plan, &options, &|_, _| {})?;
        let mut image = Image::new(Kind::Content, ctx.game());
        snapshot.fill(&mut image);
        image.entries = ctx.describe(std::mem::take(&mut image.entries));
        image.settings = ctx.settings();
        image.base = ctx.base();
        image.created = chrono::Utc::now().timestamp();
        image.producer = Producer {
            app: App::Launcher,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        image.label = label;
        image.seal();
        image.parent = state.current.clone().filter(|current| *current != image.id);
        image
            .validate()
            .map_err(|e| ImageError::Invalid(format!("The content image isn't valid: {e}")))?;
        if !snapshot.too_long.is_empty() {
            super::warn(
                "images.record",
                &ctx.id,
                &format!(
                    "{} content files have paths too long to record",
                    snapshot.too_long.len()
                ),
            );
        }
        Ok((image, snapshot, baseline))
    }

    /// Whether a "Find it for me" search of the instance is saved: it renames mods on
    /// purpose, so nothing is recorded or checked until it ends.
    pub(crate) fn finder_running(&self, instance: &str) -> bool {
        self.instances()
            .dir(instance)
            .join(crate::content::mod_finder::FILE_NAME)
            .exists()
    }
}

/// The merged records' thread: records each instance's queued labels when they are due,
/// leaving alone an instance whose game is running or whose "Find it for me" search is
/// saved. It holds the images only while it works, and ends once they are dropped.
pub(crate) fn run_worker(images: WeakImages, worker: Arc<Worker>) {
    loop {
        let Some(this) = images.upgrade() else {
            return;
        };
        let timing = this.timing();
        let now = Instant::now();
        // Without labels due later or a search to look at, it sleeps until woken.
        let mut next: Option<Instant> = None;
        let mut wake_at = |at: Instant| next = Some(next.map_or(at, |next| next.min(at)));
        // Suspended, nothing is recorded; resuming wakes this thread.
        let slots = if this.suspended() {
            Vec::new()
        } else {
            this.slots()
        };
        for (id, slot) in slots {
            let Some(deadline) = slot.pending().deadline(timing) else {
                continue;
            };
            if slot.in_session() {
                // Woken when the session ends.
                continue;
            }
            if this.finder_running(&id) {
                wake_at(now + FINDER_CHECK);
                continue;
            }
            if deadline > now {
                wake_at(deadline);
                continue;
            }
            if let Err(e) = this.flush(&id) {
                super::warn("images.record", &id, &e.to_string());
            }
            // Recording took a while: look at every instance again before sleeping.
            wake_at(Instant::now());
        }
        drop(this);
        super::wait_for_wake(&worker, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_merges_within_the_window_and_caps() {
        let timing = Timing::default();
        let start = Instant::now();
        let mut pending = Pending::default();
        assert_eq!(pending.deadline(timing), None);
        pending.push("a".into(), 0, start);
        assert_eq!(pending.deadline(timing), Some(start + timing.merge));
        pending.push("b".into(), 0, start + Duration::from_secs(2));
        assert_eq!(
            pending.deadline(timing),
            Some(start + Duration::from_secs(2) + timing.merge)
        );
        // However busy, the first label waits at most max_defer.
        let mut busy = Pending::default();
        for second in 0..40 {
            busy.push(format!("{second}"), 0, start + Duration::from_secs(second));
        }
        assert_eq!(busy.deadline(timing), Some(start + timing.max_defer));
        assert_eq!(busy.take().len(), 40);
        assert!(busy.is_empty());
        // An empty label still counts as a success to wait for, but isn't kept.
        busy.push(" ".into(), 0, start);
        assert!(!busy.is_empty());
        assert!(busy.take().is_empty());
        // Labels a failed record took go back in front, due after the retry wait.
        let queued = |label: &str| Queued {
            label: label.into(),
            generation: 3,
        };
        busy.push("later".into(), 3, start);
        busy.restore(vec![queued("first")], start);
        assert_eq!(busy.deadline(timing), Some(start + RETRY_AFTER));
        assert_eq!(busy.take(), vec![queued("first"), queued("later")]);
        assert_eq!(busy.deadline(timing), None);
    }
}
