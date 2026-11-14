//! The collector: the image store's mark and sweep (`Store::gc`), which deletes the objects
//! and chunks no manifest names any more.
//!
//! Collections run in the background, one at a time: [`ImageService::request_gc`] starts
//! one, or, while one runs, has it run once more when it ends (what was deleted meanwhile
//! may have freed more). Pruning that deleted manifests asks for one, and so does anything
//! else that deletes images.
//!
//! No lock of the core's is needed around a collection. The store's writers claim what they
//! rely on (a record that loses an object to a collection fails to save and records again),
//! and the store holds its own collection lock while it collects, which a caller that
//! relies on objects without claiming them takes shared (`Store::hold_off_collection`); a
//! thread holding that must not call [`ImageService::collect_blocking`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use lodestone_image::store::gc::{GcOptions, GcReport};
use lodestone_image::store::Store;
use tracing::{info, warn};

use crate::error::Error;

use super::ImageService;

/// Whether a collection runs, and whether another was asked for meanwhile.
#[derive(Default)]
pub(crate) struct GcState {
    pub(super) running: AtomicBool,
    pub(super) pending: AtomicBool,
}

impl ImageService {
    /// Deletes what no image uses (see the module). Blocking; waits for a collection that
    /// is running and for callers holding collections off.
    pub fn collect_blocking(&self) -> Result<GcReport, Error> {
        collect(&self.store, &GcOptions::default())
    }

    /// Starts a collection in the background, or, while one runs, has it run again when it
    /// ends. Does nothing outside a tokio runtime.
    pub fn request_gc(&self) {
        let state = self.gc.clone();
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
