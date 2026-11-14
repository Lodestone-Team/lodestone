//! The jobs of an instance's tabs (Content's installs, Worlds' backups and restores,
//! snapshots) that write its files over a while. They live in their views, but Play
//! has to know about them: [`BusyFiles`] marks the instance while one runs, and
//! [`Parked`] keeps a tab's job running, and tracked, while the tab shows another
//! instance.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use super::Instances;

/// How many jobs are writing each instance's files, by instance. A job's work holds
/// its mark on a background thread (a snapshot's), so it is shared across threads.
#[derive(Clone, Default)]
pub(super) struct BusyCount(Arc<Mutex<HashMap<String, usize>>>);

impl BusyCount {
    fn jobs(&self) -> MutexGuard<'_, HashMap<String, usize>> {
        // a count is whole whenever the lock is let go, even by a thread that panicked
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn mark(&self, id: &str) -> BusyFiles {
        *self.jobs().entry(id.to_string()).or_default() += 1;
        BusyFiles {
            id: id.to_string(),
            count: self.clone(),
        }
    }

    fn is_busy(&self, id: &str) -> bool {
        self.jobs().contains_key(id)
    }
}

/// An instance's files are being written by a tab's job for as long as this is held:
/// Play waits ([`Instances::files_busy`]). It is held by the job, so the instance is
/// free again however the job ends, even when its view goes away.
pub struct BusyFiles {
    id: String,
    count: BusyCount,
}

impl Drop for BusyFiles {
    fn drop(&mut self) {
        let mut count = self.count.jobs();
        if let Some(jobs) = count.get_mut(&self.id) {
            *jobs -= 1;
            if *jobs == 0 {
                count.remove(&self.id);
            }
        }
    }
}

impl Instances {
    /// Marks instance `id`'s files busy for a job of one of its tabs, until the
    /// returned value is dropped.
    pub fn mark_files_busy(&self, id: &str) -> BusyFiles {
        self.busy_files.mark(id)
    }

    /// Whether a job of one of the instance's tabs is writing its files: mods being
    /// installed or updated, a world being restored. The game mustn't start meanwhile.
    pub fn files_busy(&self, id: &str) -> bool {
        self.busy_files.is_busy(id)
    }
}

/// The jobs of a tab that shows one instance at a time, for the instances it isn't
/// showing. A job keeps running when another instance is selected (dropping it would
/// stop an install part-way, or lose track of a restore that goes on regardless), and
/// it is the tab's job again when its instance is. Each instance has one job at most:
/// the tab's job is the shown instance's.
pub struct Parked<J> {
    jobs: HashMap<String, J>,
}

impl<J> Default for Parked<J> {
    fn default() -> Self {
        Self {
            jobs: HashMap::new(),
        }
    }
}

impl<J> Parked<J> {
    /// The tab went from showing `from` to showing `to`: `job`, `from`'s, is kept for
    /// it, and `to`'s comes back if it has one.
    pub fn switch(&mut self, job: &mut Option<J>, from: Option<&str>, to: Option<&str>) {
        if let (Some(from), Some(running)) = (from, job.take()) {
            self.jobs.insert(from.to_string(), running);
        }
        *job = to.and_then(|to| self.jobs.remove(to));
    }

    /// The job of instance `id` finished: takes it from where it is, the tab's (when
    /// the tab shows `id`, returned with `true`) or kept aside.
    pub fn finish(
        &mut self,
        job: &mut Option<J>,
        shown: Option<&str>,
        id: &str,
    ) -> Option<(J, bool)> {
        if shown == Some(id) {
            job.take().map(|job| (job, true))
        } else {
            self.jobs.remove(id).map(|job| (job, false))
        }
    }

    /// Whether instance `id`, not shown, has a job running.
    #[cfg(test)]
    fn has(&self, id: &str) -> bool {
        self.jobs.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_busy_while_any_job_holds_them() {
        let count = BusyCount::default();
        assert!(!count.is_busy("a"));
        let install = count.mark("a");
        let restore = count.mark("a");
        assert!(count.is_busy("a"));
        assert!(!count.is_busy("b"));
        drop(install);
        assert!(count.is_busy("a"), "the restore still runs");
        drop(restore);
        assert!(!count.is_busy("a"));
    }

    #[test]
    fn a_job_survives_switching_away_and_back() {
        let mut parked = Parked::default();
        let mut job = Some("restore A");
        // A's job keeps running while B is shown, and B starts its own
        parked.switch(&mut job, Some("a"), Some("b"));
        assert_eq!(job, None);
        assert!(parked.has("a"));
        job = Some("install B");
        // back on A, its job is the tab's again (so a second one waits)
        parked.switch(&mut job, Some("b"), Some("a"));
        assert_eq!(job, Some("restore A"));
        assert!(parked.has("b"));
    }

    #[test]
    fn a_finished_job_is_taken_from_where_it_is() {
        let mut parked = Parked::default();
        let mut job = Some("A");
        parked.switch(&mut job, Some("a"), Some("b"));
        job = Some("B");
        // A finishes while B is shown: B's job stays the tab's
        assert_eq!(parked.finish(&mut job, Some("b"), "a"), Some(("A", false)));
        assert_eq!(job, Some("B"));
        assert_eq!(parked.finish(&mut job, Some("b"), "b"), Some(("B", true)));
        assert_eq!(job, None);
        assert_eq!(parked.finish(&mut job, Some("b"), "a"), None);
    }
}

/// Play in the launcher, with the sample instances of the offscreen screenshots,
/// signed out: a Play that goes ahead stops at "add an account first", having started
/// the instance's log, and nothing is downloaded or started.
#[cfg(all(test, feature = "screenshot"))]
mod play_tests {
    use gpui::TestAppContext;

    use crate::instances::Instances;

    #[test]
    fn play_waits_for_a_tab_job_writing_the_files() {
        let mut cx = TestAppContext::single();
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        cx.run_until_parked();
        let id = instances.read_with(&cx, |instances, _| instances.list()[0].id.clone());
        let played = |instances: &gpui::Entity<Instances>, cx: &mut TestAppContext| {
            instances.read_with(cx, |instances, _| {
                instances.log(&id).is_some_and(|log| !log.rows().is_empty())
            })
        };

        // LS-48: "Update all" is replacing the mods' jars
        let busy = instances.read_with(&cx, |instances, _| instances.mark_files_busy(&id));
        instances.update(&mut cx, |instances, cx| instances.play(&id, cx));
        cx.run_until_parked();
        assert!(!played(&instances, &mut cx), "Play waits for the install");

        drop(busy);
        instances.update(&mut cx, |instances, cx| instances.play(&id, cx));
        cx.run_until_parked();
        assert!(played(&instances, &mut cx), "then it plays");
    }
}
