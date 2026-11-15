//! An instance's content images, newest first: what `lodestone-launcher-cli content
//! history` lists, and what milestone 5's history view will show.

use lodestone_image::lineage::newest_content_first;
use lodestone_image::{Base, Game};
use serde::Serialize;

use super::context::InstanceContext;
use super::{ImageError, Images, state};

/// The longest [`Images::history`] waits for a record of the instance under way.
pub const HISTORY_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

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

impl HistoryEntry {
    /// The label to show: images made elsewhere may hold any character, so control and
    /// invisible ones are written visibly (`lodestone_image::display_text`). Empty when
    /// there is none.
    pub fn label_shown(&self) -> String {
        self.label
            .as_deref()
            .map(|label| lodestone_image::display_text(label).into_owned())
            .unwrap_or_default()
    }

    /// The game to show, written the same way: "fabric 0.16.10 on 1.21.1".
    pub fn game_shown(&self) -> String {
        let game = &self.game;
        let text = match &game.loader_version {
            Some(version) => format!("{} {version} on {}", game.loader.as_str(), game.minecraft),
            None => format!("{} {}", game.loader.as_str(), game.minecraft),
        };
        lodestone_image::display_text(&text).into_owned()
    }
}

impl Images {
    /// The instance's content images (see [`History`]). Each is loaded whole, so its id is
    /// checked; one that can't be read is left out. [`ImageError::Gone`] for an instance
    /// that doesn't exist; an empty history for one with no images, or whose images are an
    /// earlier instance's of the same id. Blocking: waits at most [`HISTORY_WAIT`] for a
    /// record of the instance that is running (the first image of a big pack can take
    /// minutes), then lists without it.
    pub fn history(&self, instance: &str) -> Result<History, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        if !self.accepts(instance) {
            return Ok(History::default());
        }
        // Not in the middle of a record if it can help it. Listed during one, the worst is
        // that its image is missing, or listed without being current yet: a record saves
        // the manifest before the pointer.
        let slot = self.slot(instance);
        let _record = super::check::lock_within(&slot.record, HISTORY_WAIT);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_games_are_shown_with_hidden_characters_written() {
        let entry = HistoryEntry {
            id: "a".repeat(64),
            created: 0,
            label: Some("Added JEI\u{202e}gpj.exe".into()),
            parent: None,
            current: true,
            game: Game {
                minecraft: "1.21.1\n".into(),
                loader: lodestone_image::Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
            base: None,
            files: 0,
            withheld: 0,
        };
        assert_eq!(entry.label_shown(), "Added JEI\\u{202E}gpj.exe");
        assert_eq!(entry.game_shown(), "fabric 0.16.10 on 1.21.1\\u{000A}");
        let none = HistoryEntry {
            label: None,
            game: Game {
                minecraft: "1.21.1".into(),
                loader: lodestone_image::Loader::Vanilla,
                loader_version: None,
            },
            ..entry
        };
        assert_eq!(none.label_shown(), "");
        assert_eq!(none.game_shown(), "vanilla 1.21.1");
    }
}
