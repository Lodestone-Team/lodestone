//! The check before a server starts, and keeping its baseline.
//!
//! Content can change outside Lodestone (a mod dropped in over SFTP, a config edited by
//! hand). Before a server starts, [`ImageService::before_start`] records what is queued and
//! then compares the content files' sizes and modification times with the baseline the last
//! record left, reading no file; when they differ it records "Changed outside Lodestone".
//! The check waits at most [`GATE_WAIT`] for an operation or record in progress and never
//! fails the start: the caller logs what it returns.
//!
//! A running server writes files itself (configs mods rewrite at startup, ops and the
//! whitelist changed by commands). Those never make an image: when the server stops,
//! [`ImageService::server_stopped`] takes the baseline again without recording, so they
//! join the next image an operation records. A core that died while its server ran does the
//! same at its next start ([`ImageService::after_restart`]). A server that has no content
//! image yet gets its first when it stops, or at startup if it is stopped.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use lodestone_image::store::snapshot::{self, SnapshotOptions};
use lodestone_image::store::Store;
use lodestone_image::DiskLayout;
use tracing::{info, warn};

use crate::error::Error;
use crate::types::InstanceUuid;

use super::recorder::{content_scope, Recorded, INITIAL_LABEL, OUTSIDE_LABEL};
use super::scope::{content_filter, SCOPE_RULES};
use super::state::{self, ContentState};
use super::{blocking, ImageService, InstanceContext, Slot};

/// The longest the check before a start waits for an operation or record in progress.
pub const GATE_WAIT: Duration = Duration::from_secs(10);

/// Whether the content files under `root` differ from `baseline` (disk path to size and
/// modification time): a file added, removed, resized or touched. Reads no file, only
/// their metadata. Blocking.
pub fn quick_changed(
    store: &Store,
    root: &Path,
    layout: &DiskLayout,
    excludes: &[String],
    baseline: &BTreeMap<String, (u64, i64)>,
) -> Result<bool, Error> {
    Ok(stat_content(store, root, layout, excludes)? != *baseline)
}

/// Every content file's size and modification time, by disk path, without reading any.
fn stat_content(
    store: &Store,
    root: &Path,
    layout: &DiskLayout,
    excludes: &[String],
) -> Result<BTreeMap<String, (u64, i64)>, Error> {
    let options = SnapshotOptions {
        excludes,
        cache: None,
        filter: Some(&content_filter),
        ..SnapshotOptions::default()
    };
    // Without a stat cache, planning only walks: it reads and claims nothing.
    let plan = snapshot::plan(store, root, layout, &options)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to look at the server's content: {e}"))?;
    Ok(plan
        .to_read
        .into_iter()
        .map(|file| (file.disk, (file.size, file.modified_ns)))
        .collect())
}

impl ImageService {
    /// Before a server starts (see the module). `Ok(Some)` when it recorded an image.
    pub async fn before_start(
        self: &Arc<Self>,
        ctx: InstanceContext,
    ) -> Result<Option<Recorded>, Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(None);
        }
        let slot = self.slot(&ctx.uuid);
        let Ok(_gate) = tokio::time::timeout(GATE_WAIT, slot.gate.clone().write_owned()).await
        else {
            warn!(
                "Not checking the content of {} before it starts: an operation on it is still running",
                ctx.uuid
            );
            return Ok(None);
        };
        let _lock = slot.lock.clone().lock_owned().await;
        let queued = !slot
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty();
        if queued {
            return self.record_held(&slot, ctx, None, 0).await.map(Some);
        }
        let state = self.load_state(&ctx.uuid).await?;
        if !state.has_baseline() {
            if state.current.is_some() {
                // A baseline taken under other rules: take it again rather than guess.
                self.refresh_baseline(&ctx, state).await?;
            }
            return Ok(None);
        }
        let store = self.store();
        let check = ctx.clone();
        let baseline = state.baseline.clone();
        let changed = blocking(move || {
            let (layout, excludes) = content_scope(&check)?;
            quick_changed(&store, &check.root, &layout, &excludes, &baseline)
        })
        .await?;
        if !changed {
            return Ok(None);
        }
        info!("The content of {} changed outside Lodestone", ctx.uuid);
        self.record_held(&slot, ctx, Some(OUTSIDE_LABEL.to_string()), 0)
            .await
            .map(Some)
    }

    /// The server started: its files may now change on their own until it stops.
    pub async fn server_started(&self, uuid: &InstanceUuid) {
        if !Self::accepts(uuid) {
            return;
        }
        let _lock = self.instance_lock(uuid).await;
        let root = self.root.clone();
        let id = uuid.clone();
        let saved = blocking(move || {
            let mut current = state::load(&root, &id);
            current.running_since = Some(chrono::Utc::now().timestamp());
            state::save(&root, &id, &current)
        })
        .await;
        if let Err(e) = saved {
            warn!("Failed to note that {uuid} started: {}", e.source);
        }
    }

    /// The server stopped: what it wrote while it ran joins the baseline without an image,
    /// and a server without a content image gets its first. Waits for a record in progress.
    pub async fn server_stopped(self: &Arc<Self>, ctx: InstanceContext) -> Result<(), Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(());
        }
        let slot = self.slot(&ctx.uuid);
        let _gate = slot.gate.clone().write_owned().await;
        let _lock = slot.lock.clone().lock_owned().await;
        self.absorb_held(&slot, ctx).await
    }

    /// At startup, for a stopped server: if the core stopped while it ran, its baseline is
    /// taken again; if it has no content image, its first is recorded.
    pub async fn after_restart(self: &Arc<Self>, ctx: InstanceContext) -> Result<(), Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(());
        }
        let slot = self.slot(&ctx.uuid);
        let _gate = slot.gate.clone().write_owned().await;
        let _lock = slot.lock.clone().lock_owned().await;
        let state = self.load_state(&ctx.uuid).await?;
        if state.current.is_some() && state.running_since.is_none() {
            return Ok(());
        }
        self.absorb_held(&slot, ctx).await
    }

    /// Records what is queued, or the first image, or else takes the baseline again.
    async fn absorb_held(&self, slot: &Slot, ctx: InstanceContext) -> Result<(), Error> {
        let queued = !slot
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty();
        let state = self.load_state(&ctx.uuid).await?;
        if queued || state.current.is_none() {
            let fallback = INITIAL_LABEL.to_string();
            let threads = if state.current.is_none() { 1 } else { 0 };
            self.record_held(slot, ctx.clone(), Some(fallback), threads)
                .await?;
            // The record kept `running_since`; the server isn't running now.
            let state = self.load_state(&ctx.uuid).await?;
            if state.running_since.is_some() {
                self.save_content_state(
                    &ctx.uuid,
                    ContentState {
                        running_since: None,
                        ..state
                    },
                )
                .await?;
            }
            return Ok(());
        }
        self.refresh_baseline(&ctx, state).await
    }

    /// Takes the baseline again (and forgets `running_since`), recording nothing.
    async fn refresh_baseline(
        &self,
        ctx: &InstanceContext,
        state: ContentState,
    ) -> Result<(), Error> {
        let store = self.store();
        let check = ctx.clone();
        let baseline = blocking(move || {
            let (layout, excludes) = content_scope(&check)?;
            stat_content(&store, &check.root, &layout, &excludes)
        })
        .await?;
        self.save_content_state(
            &ctx.uuid,
            ContentState {
                baseline,
                scope_rules: SCOPE_RULES,
                running_since: None,
                ..state
            },
        )
        .await
    }

    async fn load_state(&self, uuid: &InstanceUuid) -> Result<ContentState, Error> {
        let root = self.root.clone();
        let id = uuid.clone();
        blocking(move || Ok(state::load(&root, &id))).await
    }

    async fn save_content_state(
        &self,
        uuid: &InstanceUuid,
        next: ContentState,
    ) -> Result<(), Error> {
        let root = self.root.clone();
        let id = uuid.clone();
        blocking(move || state::save(&root, &id, &next)).await
    }
}
