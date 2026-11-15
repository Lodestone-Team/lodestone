//! The check before the game launches, keeping the baseline across a session, and the
//! first image of each instance.
//!
//! Content can change outside the Launcher (a mod dropped into the folder by hand, a config
//! edited in a text editor). Before each launch, [`Images::before_launch`] records what is
//! queued and then compares the content files' sizes and modification times with the
//! baseline the last record left, reading no file; when they differ, or the instance's game
//! is no longer the current image's (a loader version edited into `instance.json`), it
//! records "Changed by hand". One whose baseline was taken under other content rules is
//! recorded again without a label. The check waits at most [`GATE_WAIT`] for a record in
//! progress and never fails the launch: the caller notes what it returns.
//!
//! **First images.** An instance with no image yet (one from before the Launcher recorded
//! images) gets its first ("Content when the Launcher began recording") from
//! [`Images::record_first_images`], which the Launcher runs in the background when it
//! starts, one instance at a time, most recently played first: so the first operation on it
//! has a parent holding what it replaces. The check never records a first image itself:
//! hashing a whole pack would hold the launch for seconds, so it leaves the instance to its
//! session's end, which records it with what the game wrote (nothing earlier to compare it
//! with).
//!
//! While the game runs it writes files of its own (configs mods rewrite at startup). Those
//! never make an image of their own: the check marks the session ([`super::Slot::session`],
//! which holds merged records off, counted first, whatever the check then does; and
//! `running_since` in the state), and when the session ends, however it ends (the game
//! exited, crashed, or never started), [`Images::session_ended`] takes the baseline again
//! without recording, so they join the next image an operation records. The current image's
//! game stays the baseline's game: a game changed by hand meanwhile is still found.
//!
//! A session no end was seen of (the Launcher quit after the launch, as Settings › After
//! launch can have it, or died while the game ran) leaves `running_since` in the state. The
//! next check then can't tell what the game wrote from what changed by hand, and records
//! any change all the same, as "Changed while the game ran or by hand": a change by hand is
//! never folded into the baseline, at the cost of an image of the game's own writes after
//! each such session.
//!
//! When the check can't run (a record still held the instance after [`GATE_WAIT`], the
//! store was suspended, recording failed, or there is no image yet), the session is marked
//! all the same, and its end runs the check instead of taking the baseline again: a change
//! made by hand before the launch is still "Changed by hand" rather than folded into the
//! baseline, with what the game wrote in the same image, since nothing tells the two apart
//! then.
//!
//! A "Find it for me" search renames mods on purpose: while one is saved the check does
//! nothing (the session is still marked), and its end is one operation of its own.
//!
//! [`Images::record_before_change`] is the one record that takes in what the game wrote: it
//! runs before a modpack update (and, in milestone 4's second half, before a server's
//! update is applied), so every file the change replaces is in an image.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use lodestone_image::DiskLayout;
use lodestone_image::store::snapshot::{self, Candidate, SnapshotOptions};

use super::context::InstanceContext;
use super::labels::{INITIAL_LABEL, OUTSIDE_LABEL, WHILE_RUNNING_LABEL};
use super::recorder::Recorded;
use super::scope::SCOPE_RULES;
use super::state::{self, Baseline, ContentState};
use super::{ImageError, Images, Slot};

/// The longest the check before a launch waits for a record in progress.
pub const GATE_WAIT: Duration = Duration::from_secs(10);

/// What the check before a launch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// It recorded (what was queued, the first image, or a change made by hand).
    Recorded(Recorded),
    /// Nothing had changed.
    Unchanged,
    /// It didn't look, and why.
    Skipped(&'static str),
    /// Looking or recording failed; the launch goes on.
    Failed(String),
}

/// What [`Images::check_held`] found.
enum Check {
    Recorded(Recorded),
    Unchanged,
    /// The instance has no image yet, and the caller left its first to later.
    NoImageYet,
}

impl Images {
    /// Before the game launches (see the module). Blocking: call it in the launch's
    /// background task, before the game's files are prepared. Each call begins a session,
    /// which [`Self::session_ended`] must end.
    pub fn before_launch(self: &Arc<Self>, instance: &str) -> Checked {
        if !self.accepts(instance) {
            return Checked::Skipped("the image store can't name this instance");
        }
        let slot = self.slot(instance);
        // From here on the game may write files: merged records wait for the session's end
        // whatever happens below.
        slot.session.fetch_add(1, Ordering::SeqCst);
        slot.unchecked.store(false, Ordering::SeqCst);
        if self.finder_running(instance) {
            return Checked::Skipped("a Find it for me search is saved");
        }
        // What can't be checked now is checked when the session ends.
        let unchecked = |why: Checked| {
            slot.unchecked.store(true, Ordering::SeqCst);
            why
        };
        let Some(_record) = lock_within(&slot.record, self.timing().gate) else {
            return unchecked(Checked::Skipped("a record of the instance is still running"));
        };
        let Ok(_writing) = self.enter() else {
            return unchecked(Checked::Skipped("the image store is paused"));
        };
        let checked = match self.check_held(&slot, instance, true) {
            Ok(Check::Recorded(recorded)) => Checked::Recorded(recorded),
            Ok(Check::Unchanged) => Checked::Unchanged,
            Ok(Check::NoImageYet) => {
                return unchecked(Checked::Skipped("the instance has no content image yet"));
            }
            Err(e) => return unchecked(Checked::Failed(e.to_string())),
        };
        if let Err(e) = self.note_running(instance) {
            super::warn("images.check", instance, &e.to_string());
        }
        checked
    }

    /// The check, for a caller holding the record lock. With `defer_first`, an instance
    /// with no image yet is left alone ([`Check::NoImageYet`]).
    fn check_held(
        &self,
        slot: &Slot,
        instance: &str,
        defer_first: bool,
    ) -> Result<Check, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let state = self.guarded_state(&ctx)?;
        if defer_first && !self.has_current(&ctx, &state) {
            return Ok(Check::NoImageYet);
        }
        // What is queued first; a queue of only empty labels records nothing, and the
        // quick check still runs.
        if !slot.pending().is_empty()
            && let Some(recorded) = self.record_held(slot, instance, None)?
        {
            return Ok(Check::Recorded(recorded));
        }
        let recorded = match self.change_since_baseline(&ctx, &state)? {
            Some(label) => self.record_held(slot, instance, Some(label))?,
            None => None,
        };
        Ok(recorded.map_or(Check::Unchanged, Check::Recorded))
    }

    /// Whether the state's current image is there.
    fn has_current(&self, ctx: &InstanceContext, state: &ContentState) -> bool {
        state.current.as_deref().is_some_and(|id| {
            self.store()
                .manifest_path(&ctx.id, id)
                .is_ok_and(|path| path.is_file())
        })
    }

    /// What to record for a change since the baseline `state` keeps, as the image's label:
    /// [`INITIAL_LABEL`] with no image yet, `""` (no label) for a baseline taken under other
    /// rules, [`OUTSIDE_LABEL`] when a content file or the game changed, or
    /// [`WHILE_RUNNING_LABEL`] when it did after a session no end was seen of; `None` when
    /// nothing did.
    fn change_since_baseline(
        &self,
        ctx: &InstanceContext,
        state: &ContentState,
    ) -> Result<Option<&'static str>, ImageError> {
        if !self.has_current(ctx, state) {
            return Ok(Some(INITIAL_LABEL));
        }
        if state.scope_rules != SCOPE_RULES {
            return Ok(Some(""));
        }
        if state.game.as_ref() != Some(&ctx.game()) || self.stat_content(ctx)? != state.baseline {
            return Ok(Some(if state.running_since.is_some() {
                WHILE_RUNNING_LABEL
            } else {
                OUTSIDE_LABEL
            }));
        }
        Ok(None)
    }

    /// Notes that a session began (`running_since`), for an instance with an image.
    fn note_running(&self, instance: &str) -> Result<(), ImageError> {
        let mut current = state::load(self.store(), instance);
        if current.current.is_none() {
            return Ok(());
        }
        current.running_since = Some(chrono::Utc::now().timestamp());
        state::save(self.store(), instance, &current)
    }

    /// Forgets `running_since`: the session it marks ended.
    fn forget_running(&self, instance: &str) -> Result<(), ImageError> {
        let mut current = state::load(self.store(), instance);
        if current.running_since.take().is_none() {
            return Ok(());
        }
        state::save(self.store(), instance, &current)
    }

    /// The game session that [`Self::before_launch`] began ended, however it ended (the
    /// game exited or crashed, or the launch failed or was cancelled before it ran): takes
    /// the baseline again without recording (or, when the check before the launch couldn't
    /// run, runs it now), in the background, then lets merged records run. Never blocks.
    pub fn session_ended(self: &Arc<Self>, instance: &str) {
        if !self.accepts(instance) {
            return;
        }
        let this = self.clone();
        let id = instance.to_string();
        let spawned = std::thread::Builder::new()
            .name("lodestone-images-session".into())
            .spawn(move || this.end_session(&id));
        if spawned.is_err() {
            self.end_session(instance);
        }
    }

    /// [`Self::session_ended`], blocking. When another session of the instance began
    /// meanwhile (Play pressed again as the last run ended), it only counts this one out:
    /// the baseline is that session's to take.
    pub fn end_session(self: &Arc<Self>, instance: &str) {
        let slot = self.slot(instance);
        {
            let _record = slot.lock_record();
            if slot.session.load(Ordering::SeqCst) <= 1 {
                let unchecked = slot.unchecked.swap(false, Ordering::SeqCst);
                let ended = self.enter().and_then(|_writing| {
                    if unchecked && !self.finder_running(instance) {
                        self.check_held(&slot, instance, false)?;
                        return self.forget_running(instance);
                    }
                    let ctx = InstanceContext::load(self.instances(), instance)?;
                    let state = self.guarded_state(&ctx)?;
                    if state.current.is_none() {
                        return Ok(());
                    }
                    self.retake_baseline(&ctx, state)
                });
                match ended {
                    // Suspended: `running_since` (set when there was an image) has the next
                    // check record any change, rather than fold it into the baseline.
                    Ok(()) | Err(ImageError::Gone(_)) | Err(ImageError::Suspended) => {}
                    Err(e) => super::warn("images.session", instance, &e.to_string()),
                }
            }
            slot.session_over();
        }
        self.wake_worker();
    }

    /// Records the first image of every instance that has none yet (see the module), one
    /// at a time, most recently played first: when the Launcher starts, in the background.
    /// Leaves an instance whose game runs (its session's end records it) or whose "Find it
    /// for me" search is saved, and stops while the store is suspended. Returns how many it
    /// recorded. Blocking.
    pub fn record_first_images(&self) -> usize {
        let (mut instances, _) = self.instances().list();
        instances.sort_by_key(|instance| {
            std::cmp::Reverse(instance.last_played.unwrap_or(instance.created))
        });
        let mut recorded = 0;
        for instance in instances {
            let id = instance.id.as_str();
            if self.suspended() {
                break;
            }
            if !self.accepts(id) || self.has_state(id) {
                continue;
            }
            match self.record_first(id) {
                Ok(true) => recorded += 1,
                Ok(false) | Err(ImageError::Gone(_)) | Err(ImageError::Suspended) => {}
                Err(e) => super::warn("images.first", id, &e.to_string()),
            }
        }
        recorded
    }

    /// Whether the instance has a state file (an image, or a first record under way).
    fn has_state(&self, instance: &str) -> bool {
        state::path(self.store(), instance).is_ok_and(|path| path.exists())
    }

    /// The first image of one instance, unless it has a state file by now. Labelled with
    /// what is queued when an operation came first (its files are already in place), and
    /// [`INITIAL_LABEL`] otherwise.
    fn record_first(&self, instance: &str) -> Result<bool, ImageError> {
        let slot = self.slot(instance);
        if slot.in_session() || self.finder_running(instance) {
            return Ok(false);
        }
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        if self.has_state(instance) || slot.in_session() {
            return Ok(false);
        }
        let own = slot.pending().is_empty().then_some(INITIAL_LABEL);
        Ok(self.record_held(&slot, instance, own)?.is_some())
    }

    /// Before a change that replaces files (a modpack update; a server's update in
    /// milestone 4's second half): records the content as it is, so the current image is
    /// what is on disk and every file the change replaces or removes is in an image. This is
    /// the one record that takes in what the game wrote while it ran (which otherwise only
    /// joins the next operation's image). Labelled by what is queued; else "Changed by hand"
    /// (or "Changed while the game ran or by hand") when the quick check finds a change;
    /// else `label`, which names the change about to be made. Nothing new is recorded when
    /// the content is the current image; an instance without an image gets its first.
    /// Blocking.
    pub fn record_before_change(
        &self,
        instance: &str,
        label: &str,
    ) -> Result<Option<Recorded>, ImageError> {
        if !self.accepts(instance) {
            return Ok(None);
        }
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        if !slot.pending().is_empty()
            && let Some(recorded) = self.record_held(&slot, instance, None)?
        {
            return Ok(Some(recorded));
        }
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let state = self.guarded_state(&ctx)?;
        let label = match self.change_since_baseline(&ctx, &state)? {
            Some("") | None => label,
            Some(changed) => changed,
        };
        self.record_held(&slot, instance, Some(label))
    }

    /// Takes the baseline again and forgets `running_since`, recording nothing. The game
    /// stays the current image's, so a game changed by hand meanwhile is still found.
    fn retake_baseline(
        &self,
        ctx: &InstanceContext,
        state: ContentState,
    ) -> Result<(), ImageError> {
        let baseline = self.stat_content(ctx)?;
        let game = state.game.clone().or_else(|| {
            let current = state.current.as_deref()?;
            let image = self.store().load_manifest(&ctx.id, current).ok()?;
            Some(image.game)
        });
        state::save(
            self.store(),
            &ctx.id,
            &ContentState {
                baseline,
                scope_rules: SCOPE_RULES,
                game: game.or_else(|| Some(ctx.game())),
                running_since: None,
                instance_created: Some(ctx.instance.created),
                ..state
            },
        )
    }

    /// Every content file's size and modification time, by disk path, without reading any.
    pub(crate) fn stat_content(&self, ctx: &InstanceContext) -> Result<Baseline, ImageError> {
        let filter = |candidate: &Candidate| ctx.scope.accepts(candidate);
        let options = SnapshotOptions {
            cache: None,
            filter: Some(&filter),
            ..SnapshotOptions::default()
        };
        // Without a stat cache, planning only walks: it reads and claims nothing.
        let plan = snapshot::plan(self.store(), &ctx.dir, &DiskLayout::Client, &options)?;
        Ok(plan
            .to_read
            .into_iter()
            .map(|file| (file.disk, (file.size, file.modified_ns)))
            .collect())
    }
}

/// `lock`, waiting at most `max`.
fn lock_within(lock: &Mutex<()>, max: Duration) -> Option<MutexGuard<'_, ()>> {
    let until = Instant::now() + max;
    loop {
        match lock.try_lock() {
            Ok(guard) => return Some(guard),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) if Instant::now() < until => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(std::sync::TryLockError::WouldBlock) => return None,
        }
    }
}
