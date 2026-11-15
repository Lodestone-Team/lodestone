//! An instance's content images, newest first: what `lodestone-launcher-cli content
//! history` lists, and what milestone 5's history view will show.

use lodestone_image::lineage::newest_content_first;
use lodestone_image::{Base, Game};
use serde::Serialize;

use super::context::InstanceContext;
use super::{ImageError, Images, state};

/// An instance's content images.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct History {
    /// The current image's id.
    pub current: Option<String>,
    /// Newest first ([`newest_content_first`], the order retention keeps them in).
    pub images: Vec<HistoryEntry>,
}

/// One content image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    pub id: String,
    /// Unix seconds.
    pub created: i64,
    /// As recorded: show it through `lodestone_image::display_text`.
    pub label: Option<String>,
    pub parent: Option<String>,
    pub current: bool,
    pub game: Game,
    pub base: Option<Base>,
    /// How many files it lists.
    pub files: usize,
    /// How many files it couldn't carry (links, unreadable files, bad names).
    pub withheld: usize,
}

impl Images {
    /// The instance's content images (see [`History`]). Each is loaded whole, so its id is
    /// checked; one that can't be read is left out. [`ImageError::Gone`] for an instance
    /// that doesn't exist; an empty history for one with no images, or whose images are an
    /// earlier instance's of the same id. Blocking, and waits for a record of the instance
    /// that is running.
    pub fn history(&self, instance: &str) -> Result<History, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        if !self.accepts(instance) {
            return Ok(History::default());
        }
        // Not in the middle of a record, which saves the manifest before the pointer.
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let loaded = state::load_present(self.store(), instance);
        let listing = self.listing(instance)?;
        let has_manifests = !listing.content.is_empty() || !listing.links.is_empty();
        if state::belongs_elsewhere(&loaded, has_manifests, ctx.instance.created) {
            return Ok(History::default());
        }
        let current = loaded.state.current;
        let images = newest_content_first(&listing.content, current.as_deref())
            .into_iter()
            .filter_map(|header| {
                let image = self.store().load_manifest(instance, &header.id).ok()?;
                Some(HistoryEntry {
                    current: current.as_deref() == Some(image.id.as_str()),
                    files: image.entries.len(),
                    withheld: image.withheld.len(),
                    id: image.id,
                    created: image.created,
                    label: image.label,
                    parent: image.parent,
                    game: image.game,
                    base: image.base,
                })
            })
            .collect();
        Ok(History { current, images })
    }
}
