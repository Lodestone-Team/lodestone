//! `images/<instance>/content-state.json`: an instance's current content image and what
//! its content files looked like then.
//!
//! - `current` is the content image the instance's folder is in now: the parent of the
//!   next one. It moves only after that image's manifest is saved, so it never names a
//!   manifest that isn't there. A crash between the two leaves the old pointer (none, at
//!   the first record), and the next record finds the same image again and moves it.
//! - `baseline` is each content file's size and modification time as of the last record
//!   (or the end of the last game session): the check before a launch compares the folder
//!   against it without reading any file.
//! - `game` is the game the current image has, so a loader version edited into
//!   `instance.json` by hand is a change too.
//! - `running_since` is set from the check before a launch (or a record during the
//!   session) until the session ends, so a Launcher that died while the game ran knows at
//!   the next launch that the game may have changed files since the baseline: that check
//!   records any change as "Changed while the game ran or by hand" ([`super::check`]).
//! - `instance_created` is the instance's `created` time: instance ids are folder names
//!   and are reused once an instance is deleted, and a different time means these images
//!   are another instance's ([`super::recorder`]'s id-reuse guard).
//!
//! The file isn't named `<sha256>.json`, so the store never takes it for a manifest. A
//! missing or damaged one is no state: no pointer and no baseline.

use std::collections::BTreeMap;
use std::path::PathBuf;

use lodestone_image::Game;
use lodestone_image::store::Store;
use serde::{Deserialize, Serialize};

use super::ImageError;

pub const FILE: &str = "content-state.json";
/// A state file is small; one larger than this is damaged.
const MAX_BYTES: u64 = 64 << 20;

/// Content files by disk path (relative to the instance folder, `minecraft/…`): size and
/// modification time in unix nanoseconds.
pub type Baseline = BTreeMap<String, (u64, i64)>;

/// See the module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentState {
    /// The current content image's id.
    #[serde(default)]
    pub current: Option<String>,
    /// When `current` was recorded, unix seconds.
    #[serde(default)]
    pub recorded: i64,
    /// The content files as of the last record or session end.
    #[serde(default)]
    pub baseline: Baseline,
    /// The [`super::scope::SCOPE_RULES`] the baseline was taken under; another version's
    /// baseline is retaken rather than trusted.
    #[serde(default)]
    pub scope_rules: u32,
    /// The current image's game.
    #[serde(default)]
    pub game: Option<Game>,
    /// Unix seconds since the check before the last launch, while the session lasts.
    #[serde(default)]
    pub running_since: Option<i64>,
    /// The `created` time of the instance these images are of.
    #[serde(default)]
    pub instance_created: Option<u64>,
}

/// The state file of `instance` and whether there is one at all (a damaged file is there;
/// only a missing one isn't).
pub(crate) struct Loaded {
    pub state: ContentState,
    pub present: bool,
}

/// The state file's path in the store.
pub fn path(store: &Store, instance: &str) -> Result<PathBuf, ImageError> {
    Ok(store.instance_dir(instance)?.join(FILE))
}

/// The instance's state; the default (no pointer, no baseline) when it is missing or
/// can't be read.
pub fn load(store: &Store, instance: &str) -> ContentState {
    load_present(store, instance).state
}

pub(crate) fn load_present(store: &Store, instance: &str) -> Loaded {
    let missing = Loaded {
        state: ContentState::default(),
        present: false,
    };
    let Ok(path) = path(store, instance) else {
        return missing;
    };
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return missing;
    };
    let damaged = Loaded {
        state: ContentState::default(),
        present: true,
    };
    if !meta.is_file() || meta.len() > MAX_BYTES {
        super::warn("images.state", instance, "the content state isn't readable");
        return damaged;
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return damaged;
    };
    match serde_json::from_slice::<ContentState>(&bytes) {
        Ok(state)
            if state
                .current
                .as_deref()
                .is_none_or(lodestone_image::hex::is_sha256) =>
        {
            Loaded {
                state,
                present: true,
            }
        }
        _ => {
            super::warn("images.state", instance, "the content state is damaged");
            damaged
        }
    }
}

/// Replaces the instance's state, crash-safely (temp file, flushed, renamed).
pub fn save(store: &Store, instance: &str, state: &ContentState) -> Result<(), ImageError> {
    let path = path(store, instance)?;
    let bytes = serde_json::to_vec(state)
        .map_err(|e| ImageError::Invalid(format!("Failed to write the content state: {e}")))?;
    super::write_small_atomic(&path, &bytes)
}

/// Whether images under `state` (`present`: a state file was there) belong to another
/// instance than the one created at `created`: a state naming another creation time, or
/// manifests with no state at all (a deleted instance's folder whose state went first). An
/// instance's first record writes the state naming it before any manifest
/// ([`super::recorder`]), so a crash part way through a record never leaves manifests
/// without one. A damaged state names nobody, so it never counts.
pub(crate) fn belongs_elsewhere(loaded: &Loaded, has_manifests: bool, created: u64) -> bool {
    match loaded.state.instance_created {
        Some(theirs) => theirs != created,
        None => !loaded.present && has_manifests,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lodestone_image::store::Encoding;

    #[test]
    fn missing_damaged_and_odd_states_are_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path(), Encoding::Raw);
        let loaded = load_present(&store, "a");
        assert!(!loaded.present);
        assert_eq!(loaded.state, ContentState::default());
        let mut state = ContentState {
            current: Some("a".repeat(64)),
            recorded: 5,
            scope_rules: super::super::scope::SCOPE_RULES,
            game: Some(Game {
                minecraft: "1.21.1".into(),
                loader: lodestone_image::Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            }),
            instance_created: Some(7),
            ..ContentState::default()
        };
        state.baseline.insert("minecraft/mods/a.jar".into(), (3, 7));
        save(&store, "a", &state).unwrap();
        assert_eq!(load(&store, "a"), state);

        let file = path(&store, "a").unwrap();
        std::fs::write(&file, b"{oops").unwrap();
        let loaded = load_present(&store, "a");
        assert!(loaded.present, "a damaged file is there");
        assert_eq!(loaded.state, ContentState::default());
        std::fs::write(&file, br#"{"current":"../x"}"#).unwrap();
        assert_eq!(load(&store, "a"), ContentState::default());
        // Fields a newer Launcher adds are ignored, and missing ones default.
        std::fs::write(&file, br#"{"later":1}"#).unwrap();
        assert_eq!(load(&store, "a"), ContentState::default());
        // A folder where the file goes is damaged, not missing.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert!(load_present(&store, "a").present);
        // An id the store can't name has no state.
        assert!(path(&store, "../x").is_err());
        assert_eq!(load(&store, "con"), ContentState::default());
    }

    #[test]
    fn images_of_another_instance_are_told_apart() {
        let with = |created: Option<u64>, present: bool| Loaded {
            state: ContentState {
                instance_created: created,
                ..ContentState::default()
            },
            present,
        };
        assert!(!belongs_elsewhere(&with(Some(5), true), true, 5));
        assert!(belongs_elsewhere(&with(Some(4), true), true, 5));
        // Manifests and no state: a deleted instance's leftovers.
        assert!(belongs_elsewhere(&with(None, false), true, 5));
        assert!(!belongs_elsewhere(&with(None, false), false, 5));
        // A damaged state names nobody: keep its images.
        assert!(!belongs_elsewhere(&with(None, true), true, 5));
    }
}
