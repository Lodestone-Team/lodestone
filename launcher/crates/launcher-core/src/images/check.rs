//! The check before the game launches, and keeping the baseline across a session.
//!
//! Content can change outside the Launcher (a mod dropped into the folder by hand, a config
//! edited in a text editor). Before each launch, [`Images::before_launch`] records what is
//! queued and then compares the content files' sizes and modification times with the
//! baseline the last record left, reading no file; when they differ, or the instance's game
//! is no longer the current image's (a loader version edited into `instance.json`), it
//! records "Changed by hand". An instance with no image yet gets its first ("Content when
//! the Launcher began recording"); one whose baseline was taken under other content rules
//! is recorded again without a label. The check waits at most [`GATE_WAIT`] for a record in
//! progress and never fails the launch: the caller notes what it returns.
//!
//! While the game runs it writes files of its own (configs mods rewrite at startup). Those
//! never make an image of their own: the check marks the session ([`super::Slot::session`],
//! which holds merged records off, set first, whatever the check then does; and
//! `running_since` in the state), and when the session ends, however it ends (the game
//! exited, crashed, or never started), [`Images::session_ended`] takes the baseline again
//! without recording, so they join the next image an operation records. A Launcher that
//! died while the game ran finds `running_since` still set at the next launch and takes the
//! baseline again instead of calling what the game wrote "Changed by hand".
//!
//! When the check can't run (a record still held the instance after [`GATE_WAIT`], or the
//! store was suspended), the session is marked all the same, and its end runs the check
//! instead of taking the baseline again: a change made by hand before the launch is still
//! "Changed by hand" rather than folded into the baseline, with what the game wrote in the
//! same image, since nothing tells the two apart then.
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
use super::labels::{INITIAL_LABEL, OUTSIDE_LABEL};
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

impl Images {
    /// Before the game launches (see the module). Blocking: call it in the launch's
    /// background task, before the game's files are prepared.
    pub fn before_launch(self: &Arc<Self>, instance: &str) -> Checked {
        if !self.accepts(instance) {
            return Checked::Skipped("the image store can't name this instance");
        }
        let slot = self.slot(instance);
        // From here on the game may write files: merged records wait for the session's end
        // whatever happens below.
        slot.session.store(true, Ordering::SeqCst);
        slot.unchecked.store(false, Ordering::SeqCst);
        if self.finder_running(instance) {
            return Checked::Skipped("a Find it for me search is saved");
        }
        let Some(_record) = lock_within(&slot.record, self.timing().gate) else {
            slot.unchecked.store(true, Ordering::SeqCst);
            return Checked::Skipped("a record of the instance is still running");
        };
        let Ok(_writing) = self.enter() else {
            slot.unchecked.store(true, Ordering::SeqCst);
            return Checked::Skipped("the image store is paused");
        };
        let checked = match self.check_held(&slot, instance) {
            Ok(Some(recorded)) => Checked::Recorded(recorded),
            Ok(None) => Checked::Unchanged,
            Err(e) => Checked::Failed(e.to_string()),
        };
        if let Err(e) = self.note_running(instance) {
            super::warn("images.check", instance, &e.to_string());
        }
        checked
    }

    fn check_held(&self, slot: &Slot, instance: &str) -> Result<Option<Recorded>, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let state = self.guarded_state(&ctx)?;
        if state.running_since.is_some() && state.current.is_some() {
            // The last session never ended (the Launcher died while the game ran): what the
            // game wrote is no change by hand.
            self.retake_baseline(&ctx, state)?;
            return Ok(None);
        }
        if !slot.pending().is_empty() {
            return self.record_held(slot, instance, None);
        }
        match self.change_since_baseline(&ctx, &state)? {
            Some(label) => self.record_held(slot, instance, Some(label)),
            None => Ok(None),
        }
    }

    /// What to record for a change since the baseline `state` keeps, as the image's label:
    /// [`INITIAL_LABEL`] with no image yet, `""` (no label) for a baseline taken under other
    /// rules, [`OUTSIDE_LABEL`] when a content file or the game changed; `None` when nothing
    /// did.
    fn change_since_baseline(
        &self,
        ctx: &InstanceContext,
        state: &ContentState,
    ) -> Result<Option<&'static str>, ImageError> {
        let have = state.current.as_deref().is_some_and(|id| {
            self.store()
                .manifest_path(&ctx.id, id)
                .is_ok_and(|path| path.is_file())
        });
        if !have {
            return Ok(Some(INITIAL_LABEL));
        }
        if state.scope_rules != SCOPE_RULES {
            return Ok(Some(""));
        }
        if state.game.as_ref() != Some(&ctx.game()) || self.stat_content(ctx)? != state.baseline {
            return Ok(Some(OUTSIDE_LABEL));
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

    /// [`Self::session_ended`], blocking.
    pub fn end_session(self: &Arc<Self>, instance: &str) {
        let slot = self.slot(instance);
        {
            let _record = slot.lock_record();
            let unchecked = slot.unchecked.swap(false, Ordering::SeqCst);
            let ended = self.enter().and_then(|_writing| {
                if unchecked && !self.finder_running(instance) {
                    return self.check_held(&slot, instance).map(|_| ());
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
                // check take the baseline again.
                Ok(()) | Err(ImageError::Gone(_)) | Err(ImageError::Suspended) => {}
                Err(e) => super::warn("images.session", instance, &e.to_string()),
            }
            slot.session.store(false, Ordering::SeqCst);
        }
        self.wake_worker();
    }

    /// Before a change that replaces files (a modpack update; a server's update in
    /// milestone 4's second half): records the content as it is, so the current image is
    /// what is on disk and every file the change replaces or removes is in an image. This is
    /// the one record that takes in what the game wrote while it ran (which otherwise only
    /// joins the next operation's image). Labelled by what is queued; else "Changed by hand"
    /// when the quick check finds a change; else `label`, which names the change about to be
    /// made. Nothing new is recorded when the content is the current image; an instance
    /// without an image gets its first. Blocking.
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
        if !slot.pending().is_empty() {
            return self.record_held(&slot, instance, None);
        }
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let state = self.guarded_state(&ctx)?;
        let label = match self.change_since_baseline(&ctx, &state)? {
            Some("") | None => label,
            Some(changed) => changed,
        };
        self.record_held(&slot, instance, Some(label))
    }

    /// Takes the baseline again and forgets `running_since`, recording nothing.
    fn retake_baseline(
        &self,
        ctx: &InstanceContext,
        state: ContentState,
    ) -> Result<(), ImageError> {
        let baseline = self.stat_content(ctx)?;
        state::save(
            self.store(),
            &ctx.id,
            &ContentState {
                baseline,
                scope_rules: SCOPE_RULES,
                game: Some(ctx.game()),
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
