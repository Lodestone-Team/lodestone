//! The collector: the image store's mark and sweep (`Store::gc`), which deletes the objects
//! and chunks no manifest names any more.
//!
//! Collections run in the background, one at a time: [`ImageService::request_gc`] starts
//! one, or, while one runs, has it run once more when it ends (what was deleted meanwhile
//! may have freed more). Deleting backups asks for one. Pruning content images asks at most
//! every [`PRUNE_GC_INTERVAL`] ([`ImageService::request_gc_after_pruning`]): once a server
//! has more than 30 content images nearly every record prunes one, a pruned content image
//! frees a few small files, and a collection reads every manifest of every server.
//!
//! No lock of the core's is needed around a collection. The store's writers claim what they
//! rely on (a record that loses an object to a collection fails to save and records again),
//! and the store holds its own collection lock while it collects, which a caller that
//! relies on objects without claiming them takes shared (`Store::hold_off_collection`); a
//! thread holding that must not start a collection and wait for it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lodestone_image::store::gc::{GcOptions, GcReport};
use lodestone_image::store::Store;
use tracing::{info, warn};

use crate::error::Error;

use super::ImageService;

/// The least time between two collections that pruning asks for.
pub const PRUNE_GC_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Whether a collection runs, and whether another was asked for meanwhile.
#[derive(Default)]
pub(crate) struct GcState {
    pub(super) running: AtomicBool,
    pub(super) pending: AtomicBool,
    /// When pruning last asked for a collection.
    last_for_pruning: Mutex<Option<Instant>>,
    /// How many collections were asked for, for tests of who asks.
    #[cfg(test)]
    pub(crate) requests: std::sync::atomic::AtomicUsize,
    /// How many collections were asked for later ([`ImageService::request_gc_after`]).
    #[cfg(test)]
    pub(crate) delayed_requests: std::sync::atomic::AtomicUsize,
}

/// How long after a failed write a collection is asked for: past the store's grace, which
/// keeps every object written in the last hour, by a margin for the clocks of the disk.
pub const AFTER_GRACE: Duration =
    Duration::from_secs(lodestone_image::store::gc::GRACE.as_secs() + 5 * 60);

impl ImageService {
    /// Deletes what no image uses (see the module). Blocking; waits for a collection that
    /// is running and for callers holding collections off.
    #[cfg(test)]
    pub fn collect_blocking(&self) -> Result<GcReport, Error> {
        collect(&self.store, &GcOptions::default())
    }

    /// How many collections were asked for.
    #[cfg(test)]
    pub(crate) fn gc_requests(&self) -> usize {
        self.gc.requests.load(Ordering::SeqCst)
    }

    /// [`Self::request_gc`] for pruning, at most every [`PRUNE_GC_INTERVAL`]: what a
    /// skipped request would have freed goes at the next collection.
    pub fn request_gc_after_pruning(&self) {
        let now = Instant::now();
        {
            let mut last = self
                .gc
                .last_for_pruning
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if last.is_some_and(|at| now.duration_since(at) < PRUNE_GC_INTERVAL) {
                return;
            }
            *last = Some(now);
        }
        self.request_gc();
    }

    /// How many collections were asked for later.
    #[cfg(test)]
    pub(crate) fn delayed_gc_requests(&self) -> usize {
        self.gc.delayed_requests.load(Ordering::SeqCst)
    }

    /// [`Self::request_gc`] once `delay` has passed. For objects a failed or interrupted
    /// write left (a server made from a file unpacks and downloads gigabytes before it can
    /// fail): a collection now would keep them, since they are newer than the store's
    /// grace, and nothing else may ask for one for days on a quiet core. Does nothing
    /// outside a tokio runtime.
    pub fn request_gc_after(self: &Arc<Self>, delay: Duration) {
        #[cfg(test)]
        self.gc.delayed_requests.fetch_add(1, Ordering::SeqCst);
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let service = Arc::downgrade(self);
        runtime.spawn(async move {
            tokio::time::sleep(delay).await;
            if let Some(service) = service.upgrade() {
                service.request_gc();
            }
        });
    }

    /// Starts a collection in the background, or, while one runs, has it run again when it
    /// ends. Does nothing outside a tokio runtime.
    pub fn request_gc(&self) {
        let state = self.gc.clone();
        #[cfg(test)]
        state.requests.fetch_add(1, Ordering::SeqCst);
        state.pending.store(true, Ordering::SeqCst);
        if state.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            state.running.store(false, Ordering::SeqCst);
            return;
        };
        let store = self.store.clone();
        runtime.spawn_blocking(move || run(&store, &state));
    }
}

/// Collects until no request came in while the last collection ran. A request sets
/// `pending` before it looks at `running`, and this clears `running` before it looks at
/// `pending`, so a request is never lost between the two.
fn run(store: &Arc<Store>, state: &GcState) {
    loop {
        state.pending.store(false, Ordering::SeqCst);
        match collect(store, &GcOptions::default()) {
            Ok(report) if report.objects_removed + report.chunks_removed > 0 => info!(
                "Image cleanup removed {} unused files ({} bytes)",
                report.objects_removed + report.chunks_removed,
                report.bytes_removed
            ),
            Ok(_) => {}
            Err(e) => warn!("{}", e.source),
        }
        state.running.store(false, Ordering::SeqCst);
        if !state.pending.load(Ordering::SeqCst) || state.running.swap(true, Ordering::SeqCst) {
            return;
        }
    }
}

pub(crate) fn collect(store: &Store, options: &GcOptions) -> Result<GcReport, Error> {
    store
        .gc(options)
        .map_err(|e| Error::from(color_eyre::eyre::eyre!("Image cleanup: {e}")))
}
