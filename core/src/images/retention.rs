//! Which of an instance's image manifests to keep.
//!
//! The spec keeps the content lineage like modpack history: the newest
//! [`KEEP_CONTENT`] content images, plus every one a data image links to. Data and full
//! images are backups: their own retention deletes them (the backup policy), never this.
//! Other parts of the core can name images to keep too, registered at startup
//! ([`ImageRefs`]). [`ImageService::prune`] keeps:
//!
//! - every data and full image;
//! - the content image each of those links to (`content`);
//! - every image a registered source names;
//! - the instance's current content image;
//! - the newest [`KEEP_CONTENT`] content images, by `created`.
//!
//! Every other content image that can be read is deleted (manifests only: objects go when
//! the collector finds nothing uses them). A manifest that can't be read is kept: it may be
//! a newer core's. If any source fails to say what it names, nothing is deleted.
//!
//! Prune runs under the instance's lock, so a writer that saves a manifest and then
//! something naming it (both under that lock) is never caught in between.

use std::collections::HashSet;

use async_trait::async_trait;
use lodestone_image::Kind;
use tracing::warn;

use crate::error::Error;
use crate::types::InstanceUuid;

use super::{blocking, ImageService};

/// Something that names images an instance must keep (see the module).
#[async_trait]
pub trait ImageRefs: Send + Sync {
    /// The ids of the images of `uuid` this source still needs.
    async fn refs(&self, uuid: &InstanceUuid) -> Result<HashSet<String>, Error>;
}

/// How many of the newest content images are kept beyond those something names.
pub const KEEP_CONTENT: usize = 30;

impl ImageService {
    /// Deletes the instance's images nothing keeps (see the module). Returns how many
    /// manifests were deleted; the caller asks for a collection when that is more than 0.
    pub async fn prune(&self, uuid: &InstanceUuid) -> Result<usize, Error> {
        if !Self::accepts(uuid) {
            return Ok(0);
        }
        let _lock = self.instance_lock(uuid).await;
        self.prune_locked(uuid).await
    }

    /// [`Self::prune`] for a caller that holds the instance's lock.
    pub(crate) async fn prune_locked(&self, uuid: &InstanceUuid) -> Result<usize, Error> {
        let sources: Vec<_> = self.refs.read().unwrap_or_else(|e| e.into_inner()).clone();
        let mut keep: HashSet<String> = HashSet::new();
        for source in sources {
            match source.refs(uuid).await {
                Ok(ids) => keep.extend(ids),
                Err(e) => {
                    warn!(
                        "Not pruning the images of {uuid}: a source of what they're kept for failed: {}",
                        e.source
                    );
                    return Ok(0);
                }
            }
        }
        let store = self.store();
        let root = self.root().to_path_buf();
        let id = uuid.clone();
        blocking(move || {
            let listing = store
                .list_manifests(id.as_ref())
                .map_err(|e| color_eyre::eyre::eyre!("Failed to list the images: {e}"))?;
            if let Some(current) = super::state::load(&root, &id).current {
                keep.insert(current);
            }
            // Backups, and the content they ran with.
            for image in listing.images.iter().filter(|i| i.kind != Kind::Content) {
                keep.insert(image.id.clone());
                keep.extend(image.content.clone());
            }
            // Newest first already (by created, then id).
            keep.extend(
                listing
                    .images
                    .iter()
                    .filter(|image| image.kind == Kind::Content)
                    .take(KEEP_CONTENT)
                    .map(|image| image.id.clone()),
            );
            let mut deleted = 0;
            for image in &listing.images {
                if keep.contains(&image.id) {
                    continue;
                }
                match store.delete_manifest(id.as_ref(), &image.id) {
                    Ok(true) => deleted += 1,
                    Ok(false) => {}
                    Err(e) => warn!("Failed to delete the image {} of {id}: {e}", image.id),
                }
            }
            Ok(deleted)
        })
        .await
    }
}
