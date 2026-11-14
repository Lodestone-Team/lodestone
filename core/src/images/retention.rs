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
//! - the newest [`KEEP_CONTENT`] content images ([`newest_content_first`], the order the
//!   modpack history lists them in).
//!
//! Every other content image that can be read is deleted (manifests only: objects go when
//! the collector finds nothing uses them). A manifest that can't be read is kept, since
//! what it links to can't be known. If any source fails to say what it names, nothing is
//! deleted.
//!
//! Prune runs under the instance's lock, so a writer that saves a manifest and then
//! something naming it (both under that lock) is never caught in between.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use lodestone_image::{Image, Kind};
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
            let current = super::state::load(&root, &id).current;
            keep.extend(current.clone());
            // Backups, and the content they ran with.
            for image in listing.images.iter().filter(|i| i.kind != Kind::Content) {
                keep.insert(image.id.clone());
                keep.extend(image.content.clone());
            }
            keep.extend(
                newest_content_first(&listing.images, current.as_deref())
                    .into_iter()
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

/// The content images among `images`, newest first: by when they were recorded, and those
/// recorded in the same second by their lineage, since a restore and the record before it
/// are often a second apart at most. Within one second the instance's `current` image comes
/// first (it is always the last recorded), then an image before the one it was recorded
/// after. Retention keeps the first [`KEEP_CONTENT`] of these, so the history shows what it
/// keeps.
pub(crate) fn newest_content_first<'a>(
    images: &'a [Image],
    current: Option<&str>,
) -> Vec<&'a Image> {
    let content: Vec<&Image> = images.iter().filter(|i| i.kind == Kind::Content).collect();
    let by_id: HashMap<&str, &Image> = content.iter().map(|i| (i.id.as_str(), *i)).collect();
    // How many ancestors each has in the listing. A walk stops at an id it has seen:
    // returning to earlier content records an image under an id it had before, with the
    // image it came from as its parent, so parents can go round in a circle. That is why
    // the current image is put first rather than left to its depth.
    let depth = |image: &Image| {
        let mut seen = HashSet::new();
        let mut at = image;
        while seen.insert(at.id.as_str()) {
            match at.parent.as_deref().and_then(|p| by_id.get(p)) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        seen.len()
    };
    let mut ordered: Vec<(&Image, bool, usize)> = content
        .into_iter()
        .map(|i| (i, Some(i.id.as_str()) == current, depth(i)))
        .collect();
    ordered.sort_by(|(a, a_current, da), (b, b_current, db)| {
        b.created
            .cmp(&a.created)
            .then(b_current.cmp(a_current))
            .then(db.cmp(da))
            .then(a.id.cmp(&b.id))
    });
    ordered.into_iter().map(|(image, _, _)| image).collect()
}
