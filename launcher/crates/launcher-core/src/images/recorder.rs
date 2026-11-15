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
//! did. An operation that only changes which files are content
//! ([`Images::begin_rescope`]) records with no label of its own. A new instance is
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

use lodestone_image::layout::CLIENT_GAME_FOLDER;
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
/// Room the records leave on the disk beyond what they will store: 1 GiB, as the
/// snapshot store keeps, since most records are work nobody asked for (first images,
/// merged records) and the game's world saves need the room more.
pub(crate) const SPARE_BYTES: u64 = 1 << 30;
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
    /// An operation that changes which files are content succeeded
    /// ([`Images::begin_rescope`]): a record is due even without a label.
    rescoped: bool,
    first: Option<Instant>,
    last: Option<Instant>,
    /// After a failed record: not before then.
    not_before: Option<Instant>,
}

/// What [`Pending::take`] took.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Taken {
    pub(crate) labels: Vec<Queued>,
    /// Whether an operation that changes which files are content succeeded too.
    pub(crate) rescoped: bool,
}

impl Taken {
    fn is_empty(&self) -> bool {
        self.labels.is_empty() && !self.rescoped
    }
}

impl Pending {
    /// Queues a success. An empty label counts as one to wait for, but isn't kept: an
    /// operation that says nothing (a job that did nothing) records nothing of its own.
    pub(crate) fn push(&mut self, label: String, generation: u64, now: Instant) {
        if !label.trim().is_empty() {
            self.labels.push(Queued { label, generation });
        }
        self.first.get_or_insert(now);
        self.last = Some(now);
    }

    /// Queues the success of an operation that changed which files are content.
    pub(crate) fn push_rescope(&mut self, now: Instant) {
        self.rescoped = true;
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

    /// Takes what is queued.
    pub(crate) fn take(&mut self) -> Taken {
        self.first = None;
        self.last = None;
        self.not_before = None;
        Taken {
            labels: std::mem::take(&mut self.labels),
            rescoped: std::mem::take(&mut self.rescoped),
        }
    }

    /// Puts what a failed record took back in front, due again after [`RETRY_AFTER`].
    pub(crate) fn restore(&mut self, mut taken: Taken, now: Instant) {
        if taken.is_empty() {
            return;
        }
        taken.labels.append(&mut self.labels);
        self.labels = taken.labels;
        self.rescoped |= taken.rescoped;
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
    /// It changes which files are content ([`Images::begin_rescope`]).
    rescope: bool,
}

impl Operation {
    /// The operation succeeded: its label (with its notes) is recorded with whatever else
    /// succeeds within the merge window.
    pub fn succeed(mut self) {
        if let Some(images) = self.images.take() {
            if self.rescope {
                images
                    .slot(&self.instance)
                    .pending()
                    .push_rescope(Instant::now());
                images.wake_worker();
                return;
            }
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

    /// Succeeds when `result` is `Ok`; otherwise the operation records nothing. For an
    /// action that changes nothing when it fails (one file renamed, one file added).
    pub fn finish<T, E>(self, result: &Result<T, E>) {
        if result.is_ok() {
            self.succeed();
        }
    }

    /// Succeeds when `done`: for an action whose error can come after it changed files in
    /// one known way (a removal that failed to forget the file in `options.txt` once the
    /// file was in the Trash: `done` is whether the file is gone).
    pub fn finish_if(self, done: bool) {
        if done {
            self.succeed();
        }
    }

    /// Succeeds whatever `result` is, for an action that may have changed files before it
    /// failed or was cancelled (a pack update, a share code, a revert): on failure, as
    /// [`Self::succeed_part_way`].
    pub fn finish_partly<T, E>(self, result: &Result<T, E>) {
        match result {
            Ok(_) => self.succeed(),
            Err(_) => self.succeed_part_way(),
        }
    }

    /// The action failed or was cancelled after it may have changed files: its label says
    /// it stopped part way ([`super::labels::stopped_part_way`]), so what it did change is
    /// its own image rather than "Changed by hand" at the next launch. When it changed
    /// nothing, nothing is recorded, as for any operation.
    pub fn succeed_part_way(mut self) {
        let label = self.take_label();
        self.label = super::labels::stopped_part_way(&label);
        self.succeed();
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
                rescope: false,
            };
        }
        let generation = self.slot(instance).generation.load(Ordering::SeqCst);
        Operation {
            images: Some(self.clone()),
            instance: instance.to_string(),
            label,
            notes: Vec::new(),
            generation,
            rescope: false,
        }
    }

    /// Starts an operation that changes which files are content without changing one (a
    /// rebuilt server pack whose `modpack.json` lists other files): when it succeeds, a
    /// record runs with no label of its own, merged with what else succeeds, so the
    /// baseline follows the new rules and the next launch's check doesn't take the change
    /// for one made by hand. When no content file changed, that record only takes the
    /// baseline again.
    pub fn begin_rescope(self: &Arc<Self>, instance: &str) -> Operation {
        Operation {
            rescope: true,
            ..self.begin(instance, "")
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
                    // Only instances with something queued: the checks below look at disk.
                    if slot.pending().is_empty()
                        || slot.in_session()
                        || this.suspended()
                        || this.finder_running(&id)
                    {
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
    /// own), as does an operation that changed which files are content; with none of
    /// these, nothing is recorded. It is never added to an image that holds the same contents
    /// already. On failure what was queued goes back on the queue, unless the instance is
    /// gone.
    pub(crate) fn record_held(
        &self,
        slot: &Slot,
        instance: &str,
        label: Option<&str>,
    ) -> Result<Option<Recorded>, ImageError> {
        let taken = slot.pending().take();
        let own = label.or(taken.rescoped.then_some(""));
        if taken.labels.is_empty() && own.is_none() {
            return Ok(None);
        }
        match self.record_locked(slot, instance, &taken.labels, own) {
            Ok(recorded) => Ok(Some(recorded)),
            Err(ImageError::Gone(id)) => Err(ImageError::Gone(id)),
            Err(e) => {
                slot.pending().restore(taken, Instant::now());
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
                // A session that runs now is noted (the record may have held the check
                // before its launch off); one no end was seen of is over once a record has
                // taken the content again.
                running_since: slot
                    .in_session()
                    .then(|| state.running_since.unwrap_or(image.created)),
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
        let loaded = state::load_present(self.store(), &ctx.id)?;
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
        let _room = self.reserve_space(plan.bytes_to_read())?;
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
        // The walk follows no link under the instance folder, the game folder included, so a
        // linked game folder records as empty (see "What content is" in the spec).
        if std::fs::symlink_metadata(ctx.dir.join(CLIENT_GAME_FOLDER))
            .is_ok_and(|meta| meta.file_type().is_symlink())
        {
            super::warn(
                "images.record",
                &ctx.id,
                "the game folder is a link, which content images don't follow",
            );
        }
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

    /// Reserves `bytes` of the disk for one record until the returned guard is dropped,
    /// or refuses when the free space can't hold them, what the other records under way
    /// reserved, and [`SPARE_BYTES`]: records of different instances run at once (merged
    /// records, first images, imports), and each checking alone could let them fill the
    /// disk together. Conservative: what a record under way already wrote is counted
    /// twice (in its reservation and gone from the free space), and an object the disk
    /// clones (APFS) costs almost nothing yet counts whole.
    pub(crate) fn reserve_space(&self, bytes: u64) -> Result<Reservation<'_>, ImageError> {
        let mut reserved = self.reserved.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(free) = self.free_space() {
            let needed = bytes.saturating_add(SPARE_BYTES);
            if needed.saturating_add(*reserved) > free {
                let others = if *reserved > 0 {
                    format!(
                        ", other records under way need {} MB,",
                        *reserved / 1_000_000
                    )
                } else {
                    String::new()
                };
                return Err(ImageError::Invalid(format!(
                    "Recording the instance's content needs {} MB{others} and the disk has \
                     {} MB free",
                    needed / 1_000_000,
                    free / 1_000_000
                )));
            }
        }
        *reserved = reserved.saturating_add(bytes);
        Ok(Reservation {
            reserved: &self.reserved,
            bytes,
        })
    }

    /// Whether a "Find it for me" search of the instance is saved: it renames mods on
    /// purpose, so nothing is recorded or checked until it ends.
    pub(crate) fn finder_running(&self, instance: &str) -> bool {
        crate::content::mod_finder::is_saved(&self.instances().dir(instance))
    }
}

/// Disk space one record reserved ([`Images::reserve_space`]), given back when dropped.
pub(crate) struct Reservation<'a> {
    reserved: &'a std::sync::Mutex<u64>,
    bytes: u64,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut reserved = self.reserved.lock().unwrap_or_else(|e| e.into_inner());
        *reserved = reserved.saturating_sub(self.bytes);
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
            // Never waits on a record of the instance under way (its first image can take
            // minutes): the other instances' labels would wait behind it.
            match this.flush_unless_busy(&id) {
                Ok(_) => {}
                Err(ImageError::Busy) => {
                    wake_at(now + timing.merge);
                    continue;
                }
                Err(e) => super::warn("images.record", &id, &e.to_string()),
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
        assert_eq!(busy.take().labels.len(), 40);
        assert!(busy.is_empty());
        // An empty label still counts as a success to wait for, but isn't kept; a change
        // of which files are content is kept as one.
        busy.push(" ".into(), 0, start);
        assert!(!busy.is_empty());
        assert_eq!(busy.take(), Taken::default());
        busy.push_rescope(start);
        assert_eq!(busy.deadline(timing), Some(start + timing.merge));
        assert_eq!(
            busy.take(),
            Taken {
                labels: Vec::new(),
                rescoped: true,
            }
        );
        // What a failed record took goes back in front, due after the retry wait.
        let queued = |label: &str| Queued {
            label: label.into(),
            generation: 3,
        };
        busy.push("later".into(), 3, start);
        busy.restore(
            Taken {
                labels: vec![queued("first")],
                rescoped: true,
            },
            start,
        );
        assert_eq!(busy.deadline(timing), Some(start + RETRY_AFTER));
        assert_eq!(
            busy.take(),
            Taken {
                labels: vec![queued("first"), queued("later")],
                rescoped: true,
            }
        );
        assert_eq!(busy.deadline(timing), None);
        // Nothing taken, nothing restored.
        busy.restore(Taken::default(), start);
        assert!(busy.is_empty());
    }
}
