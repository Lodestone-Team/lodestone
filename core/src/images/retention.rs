//! Which of an instance's image manifests to keep.
//!
//! The spec keeps the content lineage like modpack history: the newest
//! [`KEEP_CONTENT`] content images, plus every one a data image links to. Data and full
//! images are backups: their own retention deletes them (the backup policy), never this.
//! [`ImageService::prune`] keeps:
//!
//! - every data and full image;
//! - the content image each of those links to (`content`);
//! - the image a modpack restore in progress goes back to
//!   ([`ImageService::keep_while_restoring`]);
//! - the instance's current content image, and its parent (the image recorded before a
//!   modpack restore holds the only copy of what the restore took off, even when a clock
//!   that went back ranks it among older images);
//! - the newest [`KEEP_CONTENT`] content images ([`newest_content_first`], the order the
//!   modpack history lists them in).
//!
//! Every other content image is deleted (manifests only: objects go when the collector
//! finds nothing uses them). While any manifest can't be read, nothing is deleted: what it
//! links to can't be known.
//!
//! Listing reads manifest headers ([`ManifestHeader`]), not whole manifests: a backup's data
//! image lists every file of the server, and pruning runs after every content record. A
//! data image's kind and link are part of its id, so they are read once per id and kept in
//! memory ([`ImageService`]'s link cache).
//!
//! Prune runs under the instance's lock, so a writer that saves a manifest and then
//! something naming it (both under that lock) is never caught in between.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex};

use lodestone_image::store::{ManifestHeader, Store};
use lodestone_image::Kind;
use tracing::warn;

use crate::error::Error;
use crate::types::InstanceUuid;

use super::{blocking, ImageService};

/// How many of the newest content images are kept beyond those something names.
pub const KEEP_CONTENT: usize = 30;

/// The content image each data or full image links to, by the data image's id: an id is
/// here only once its header said it isn't a content image. Kind and link are part of the
/// id, so an entry never goes stale.
pub(crate) type LinkCache = StdMutex<HashMap<String, Option<String>>>;

/// An instance's manifests as retention and the history see them.
#[derive(Debug, Default)]
pub(crate) struct Listing {
    /// The content images' headers, in no order.
    pub content: Vec<ManifestHeader>,
    /// Each data and full image's id, with the content image it links to.
    pub links: Vec<(String, Option<String>)>,
    /// Manifests whose header can't be read, by id, with why.
    pub unreadable: Vec<(String, String)>,
}

/// Lists the instance's manifests (see the module). Blocking.
pub(crate) fn list(
    store: &Store,
    links: &LinkCache,
    uuid: &InstanceUuid,
) -> Result<Listing, Error> {
    let ids = store
        .manifest_ids(uuid.as_ref())
        .map_err(|e| color_eyre::eyre::eyre!("Failed to list the images: {e}"))?;
    let mut listing = Listing::default();
    for id in ids {
        let cached = links
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned();
        if let Some(content) = cached {
            listing.links.push((id, content));
            continue;
        }
        match store.load_header(uuid.as_ref(), &id) {
            Ok(header) if header.kind == Kind::Content => listing.content.push(header),
            Ok(header) => {
                links
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(id.clone(), header.content.clone());
                listing.links.push((id, header.content));
            }
            Err(e) => listing.unreadable.push((id, e.to_string())),
        }
    }
    listing.unreadable.sort();
    Ok(listing)
}

/// Keeps one image of an instance from pruning while it lives
/// ([`ImageService::keep_while_restoring`]).
pub struct Restoring {
    service: Arc<ImageService>,
    uuid: InstanceUuid,
    id: String,
}

impl Drop for Restoring {
    fn drop(&mut self) {
        let mut restoring = self
            .service
            .restoring
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ids) = restoring.get_mut(&self.uuid) {
            if let Some(i) = ids.iter().position(|id| *id == self.id) {
                ids.swap_remove(i);
            }
            if ids.is_empty() {
                restoring.remove(&self.uuid);
            }
        }
    }
}

impl ImageService {
    /// Keeps the image `id` of `uuid` until the guard is dropped: the record before a
    /// modpack restore may make the image it goes back to the 31st newest, which pruning
    /// would delete right before the restore reads it.
    pub fn keep_while_restoring(self: &Arc<Self>, uuid: &InstanceUuid, id: &str) -> Restoring {
        self.restoring
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_default()
            .push(id.to_string());
        Restoring {
            service: self.clone(),
            uuid: uuid.clone(),
            id: id.to_string(),
        }
    }

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
        let mut keep: HashSet<String> = self
            .restoring
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();
        let store = self.store();
        let links = self.links.clone();
        let root = self.root().to_path_buf();
        let id = uuid.clone();
        blocking(move || {
            let listing = list(&store, &links, &id)?;
            if let Some((unreadable, why)) = listing.unreadable.first() {
                warn!(
                    "Not pruning the images of {id}: the image {unreadable} can't be read ({why}), so what it links to is unknown"
                );
                return Ok(0);
            }
            let current = super::state::load(&root, &id).current;
            keep.extend(current.clone());
            // And its parent, whatever the clocks said: the image recorded right before a
            // modpack restore holds the only copy of what the restore took off.
            keep.extend(
                current
                    .as_deref()
                    .and_then(|current| listing.content.iter().find(|i| i.id == current))
                    .and_then(|image| image.parent.clone()),
            );
            // Backups, and the content they ran with.
            for (image, content) in &listing.links {
                keep.insert(image.clone());
                keep.extend(content.clone());
            }
            keep.extend(
                newest_content_first(&listing.content, current.as_deref())
                    .into_iter()
                    .take(KEEP_CONTENT)
                    .map(|image| image.id.clone()),
            );
            let mut deleted = 0;
            for image in &listing.content {
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
/// recorded in the same second (a restore and the record before it are often a second
/// apart at most) by the lineage. Every record takes the image current before it as its
/// parent, so going back from the instance's `current` image, parent by parent, lists the
/// images newest first. Returning to earlier content saves an image again under the id it
/// had, with the image it came from as its new parent, so parents can go round in a circle:
/// the walk stops at an image it has seen. Images the walk doesn't reach (the parent they
/// were recorded after was saved again since) come after those it does, the one with more
/// ancestors first. Retention keeps the first [`KEEP_CONTENT`] of these, so the history
/// shows what it keeps.
pub(crate) fn newest_content_first<'a>(
    images: &'a [ManifestHeader],
    current: Option<&str>,
) -> Vec<&'a ManifestHeader> {
    let content: Vec<&ManifestHeader> = images.iter().filter(|i| i.kind == Kind::Content).collect();
    let by_id: HashMap<&str, &ManifestHeader> =
        content.iter().map(|i| (i.id.as_str(), *i)).collect();
    let parent_of = |image: &ManifestHeader| image.parent.as_deref().and_then(|p| by_id.get(p));
    // How far back from the current image each is.
    let mut back: HashMap<&str, usize> = HashMap::new();
    let mut at = current.and_then(|id| by_id.get(id)).copied();
    while let Some(image) = at {
        if back.contains_key(image.id.as_str()) {
            break;
        }
        back.insert(image.id.as_str(), back.len());
        at = parent_of(image).copied();
    }
    // How many ancestors one the walk doesn't reach has.
    let depth = |image: &ManifestHeader| {
        let mut seen = HashSet::new();
        let mut at = image;
        while seen.insert(at.id.as_str()) {
            match parent_of(at) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        seen.len()
    };
    let mut ordered: Vec<(&ManifestHeader, usize, usize)> = content
        .into_iter()
        .map(|i| {
            let back = back.get(i.id.as_str()).copied().unwrap_or(usize::MAX);
            let depth = if back == usize::MAX { depth(i) } else { 0 };
            (i, back, depth)
        })
        .collect();
    ordered.sort_by(|(a, a_back, da), (b, b_back, db)| {
        b.created
            .cmp(&a.created)
            .then(a_back.cmp(b_back))
            .then(db.cmp(da))
            .then(a.id.cmp(&b.id))
    });
    ordered.into_iter().map(|(image, _, _)| image).collect()
}
