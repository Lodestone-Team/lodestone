//! Which of an instance's image manifests to keep.
//!
//! The spec keeps the content lineage like a modpack history: the newest
//! [`KEEP_CONTENT`] content images, the current one and its parent, and every image a data
//! image links to. Pruning (after every record that wrote an image) keeps:
//!
//! - every data and full image (snapshots, from milestone 5: their own retention deletes
//!   them, never this), and the content image each links to (`content`);
//! - the images a caller holds with [`Images::keep_while`] (a restore in milestone 5 reads
//!   the image it goes back to, which the record before it could make the 31st);
//! - the instance's current content image and its parent (the image before a change holds
//!   the only copy of what the change took off, even when a clock that went back ranks it
//!   among older images);
//! - the newest [`KEEP_CONTENT`] content images
//!   ([`lodestone_image::lineage::newest_content_first`], the order the history lists).
//!
//! Every other content image is deleted (manifests only: objects go when the collector
//! finds nothing uses them). While any manifest can't be read, nothing is deleted: it may be
//! a data image linking to anything. Headers are read, not whole manifests.

use std::collections::HashSet;
use std::sync::Arc;

use lodestone_image::Kind;
pub use lodestone_image::lineage::KEEP_CONTENT;
use lodestone_image::lineage::newest_content_first;
use lodestone_image::store::ManifestHeader;

use super::{ImageError, Images};

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

impl Images {
    /// Lists the instance's manifests by their headers.
    pub(crate) fn listing(&self, instance: &str) -> Result<Listing, ImageError> {
        let mut listing = Listing::default();
        for id in self.store().manifest_ids(instance)? {
            match self.store().load_header(instance, &id) {
                Ok(header) if header.kind == Kind::Content => listing.content.push(header),
                Ok(header) => listing.links.push((id, header.content)),
                Err(e) => listing.unreadable.push((id, e.to_string())),
            }
        }
        listing.unreadable.sort();
        Ok(listing)
    }

    /// Deletes the instance's content images nothing keeps (see the module), for a caller
    /// holding its record lock; `current` is its current image. Returns how many manifests
    /// went.
    pub(crate) fn prune_locked(
        &self,
        instance: &str,
        current: Option<&str>,
    ) -> Result<usize, ImageError> {
        let listing = self.listing(instance)?;
        if let Some((unreadable, why)) = listing.unreadable.first() {
            super::warn(
                "images.prune",
                instance,
                &format!("not pruning: the image {unreadable} can't be read ({why})"),
            );
            return Ok(0);
        }
        let mut keep: HashSet<String> = self
            .keep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(instance)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();
        keep.extend(current.map(str::to_string));
        keep.extend(
            current
                .and_then(|current| listing.content.iter().find(|i| i.id == current))
                .and_then(|image| image.parent.clone()),
        );
        for (image, content) in &listing.links {
            keep.insert(image.clone());
            keep.extend(content.clone());
        }
        keep.extend(
            newest_content_first(&listing.content, current)
                .into_iter()
                .take(KEEP_CONTENT)
                .map(|image| image.id.clone()),
        );
        let mut deleted = 0;
        for image in &listing.content {
            if keep.contains(&image.id) {
                continue;
            }
            match self.store().delete_manifest(instance, &image.id) {
                Ok(true) => deleted += 1,
                Ok(false) => {}
                Err(e) => super::warn("images.prune", instance, &e.to_string()),
            }
        }
        Ok(deleted)
    }

    /// Keeps the image `id` of `instance` from pruning until the guard is dropped.
    pub fn keep_while(self: &Arc<Self>, instance: &str, id: &str) -> Keep {
        self.keep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(instance.to_string())
            .or_default()
            .push(id.to_string());
        Keep {
            images: self.clone(),
            instance: instance.to_string(),
            id: id.to_string(),
        }
    }
}

/// Keeps one image from pruning while it lives ([`Images::keep_while`]).
pub struct Keep {
    images: Arc<Images>,
    instance: String,
    id: String,
}

impl Drop for Keep {
    fn drop(&mut self) {
        let mut keep = self.images.keep.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ids) = keep.get_mut(&self.instance) {
            if let Some(i) = ids.iter().position(|id| *id == self.id) {
                ids.swap_remove(i);
            }
            if ids.is_empty() {
                keep.remove(&self.instance);
            }
        }
    }
}
