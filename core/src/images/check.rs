//! The check before a server starts, and keeping its baseline.
//!
//! Content can change outside Lodestone (a mod dropped in over SFTP, a config edited by
//! hand). Before a server starts, [`ImageService::before_start`] records what is queued and
//! then compares the content files' sizes and modification times with the baseline the last
//! record left, reading no file, under the excludes the baseline was taken under; when they
//! differ it records "Changed outside Lodestone". When they don't but the backup policy's
//! excludes changed since (which changes what an image holds, and its id), it records
//! "Changed the backup excludes".
//! The check waits at most [`GATE_WAIT`] for an operation or record in progress and never
//! fails the start: the caller logs what it returns.
//!
//! A running server writes files itself (configs mods rewrite at startup, ops and the
//! whitelist changed by commands). Those never make an image of their own: when the server
//! stops, [`ImageService::server_stopped`] takes the baseline again without recording, so
//! they join the next image an operation records. The one exception is the record a modpack
//! update or restore takes first ([`ImageService::record_before_change`]), which must hold
//! every file the change replaces. A core that died while its server ran does the
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

use super::recorder::{content_scope, Recorded, EXCLUDES_LABEL, INITIAL_LABEL, OUTSIDE_LABEL};
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
        let Some(label) = self.changed_since_baseline(&ctx, &state).await? else {
            return Ok(None);
        };
        info!("The content of {} changed: {label}", ctx.uuid);
        self.record_held(&slot, ctx, Some(label.to_string()), 0)
            .await
            .map(Some)
    }

    /// What changed since the baseline `state` keeps, as the label of the image to record:
    /// [`OUTSIDE_LABEL`] when a content file did (compared under the excludes the baseline
    /// was taken under, so a policy change alone isn't taken for one), else
    /// [`EXCLUDES_LABEL`] when the policy's excludes did; `None` when nothing did.
    async fn changed_since_baseline(
        &self,
        ctx: &InstanceContext,
        state: &ContentState,
    ) -> Result<Option<&'static str>, Error> {
        let store = self.store();
        let check = ctx.clone();
        let baseline = state.baseline.clone();
        let before = state.excludes.clone();
        blocking(move || {
            let (layout, excludes) = content_scope(&check)?;
            if quick_changed(&store, &check.root, &layout, &before, &baseline)? {
                Ok(Some(OUTSIDE_LABEL))
            } else if excludes != before {
                Ok(Some(EXCLUDES_LABEL))
            } else {
                Ok(None)
            }
        })
        .await
    }

    /// Before a modpack update or restore changes the content of the stopped server `ctx`:
    /// records it as it is, so the current image is what is on disk and every file the
    /// change replaces or removes is in an image. This is the one record that takes in what
    /// the server wrote while it ran (which otherwise only joins the next operation's image):
    /// a restore must be able to give those files back. The image is labelled by what is
    /// queued; else "Changed outside Lodestone" when the quick check finds content changed
    /// since the last record or stop; else "Changed the backup excludes" when only the
    /// policy's excludes did (those that can reach content); else `label`, which names the
    /// change about to be made (the difference is then what the server wrote while it ran,
    /// or a file that couldn't be read at the last record); a server without an image gets
    /// its first.
    /// Nothing new is recorded when the content is the current image.
    pub async fn record_before_change(
        self: &Arc<Self>,
        ctx: InstanceContext,
        label: &str,
    ) -> Result<Option<Recorded>, Error> {
        if !Self::accepts(&ctx.uuid) {
            return Ok(None);
        }
        let slot = self.slot(&ctx.uuid);
        let _gate = slot.gate.clone().write_owned().await;
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
        let label = if state.current.is_none() {
            INITIAL_LABEL
        } else if state.has_baseline() {
            match self.changed_since_baseline(&ctx, &state).await? {
                Some(changed) => changed,
                None => label,
            }
        } else {
            label
        };
        self.record_held(&slot, ctx, Some(label.to_string()), 0)
            .await
            .map(Some)
    }

    /// The server is about to start: its files may change on their own from now until it
    /// stops. Call it before the server's state leaves Stopped, so the stop of a run that
    /// ends at once ([`Self::server_stopped`]) always comes after it and clears it. Returns
    /// whether this set `running_since` (it wasn't set already), for
    /// [`Self::start_refused`].
    pub async fn server_started(&self, uuid: &InstanceUuid) -> bool {
        if !Self::accepts(uuid) {
            return false;
        }
        let _lock = self.instance_lock(uuid).await;
        let root = self.root.clone();
        let id = uuid.clone();
        let saved = blocking(move || {
            let mut current = state::load(&root, &id);
            if current.running_since.is_some() {
                return Ok(false);
            }
            current.running_since = Some(chrono::Utc::now().timestamp());
            state::save(&root, &id, &current).map(|()| true)
        })
        .await;
        saved.unwrap_or_else(|e| {
            warn!("Failed to note that {uuid} started: {}", e.source);
            false
        })
    }

    /// The start [`Self::server_started`] noted (it returned true) was refused before the
    /// state left Stopped: nothing ran, so `running_since` goes and the baseline stays.
    pub async fn start_refused(&self, uuid: &InstanceUuid) {
        if !Self::accepts(uuid) {
            return;
        }
        let _lock = self.instance_lock(uuid).await;
        let root = self.root.clone();
        let id = uuid.clone();
        let saved = blocking(move || {
            let mut current = state::load(&root, &id);
            if current.running_since.take().is_none() {
                return Ok(());
            }
            state::save(&root, &id, &current)
        })
        .await;
        if let Err(e) = saved {
            warn!("Failed to note that {uuid} didn't start: {}", e.source);
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

    /// Takes the baseline again (and forgets `running_since`), recording nothing. Under the
    /// excludes it was taken under, which are the current image's: a policy whose excludes
    /// changed meanwhile is still noticed before the next start.
    async fn refresh_baseline(
        &self,
        ctx: &InstanceContext,
        state: ContentState,
    ) -> Result<(), Error> {
        let store = self.store();
        let check = ctx.clone();
        let excludes = state.excludes.clone();
        let baseline = blocking(move || {
            let (layout, _) = content_scope(&check)?;
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
