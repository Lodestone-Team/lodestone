//! `images/<instance>/content-state.json`: an instance's current content image and what its
//! content files looked like then.
//!
//! - `current` is the content image the server's folder is in now: the parent of the next
//!   one, and what a backup links to. It moves only after that image's manifest is saved,
//!   so it never names a manifest that isn't there. A crash between the two leaves the old
//!   pointer, and the next record finds the same image again and moves it.
//! - `baseline` is each content file's size and modification time as of the last record
//!   (or the last time a stopped server's files were absorbed): the before-start check
//!   compares the folder against it without reading any file.
//! - `running_since` is set while the server runs, so a core that died while it ran knows
//!   at its next start that the baseline is stale (the server rewrote configs) and takes
//!   it again without recording.
//! - `modrinth` keeps what Lodestone learned from Modrinth about the current image's files
//!   (by SHA-512), so their `env` stays the same across restarts without asking Modrinth
//!   while recording.
//!
//! The file isn't named `<sha256>.json`, so the store never takes it for a manifest. A
//! missing or damaged one is no state: no pointer and no baseline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::types::InstanceUuid;

use super::env::ModrinthHint;

const FILE: &str = "content-state.json";
/// A state file is small; one larger than this is damaged.
const MAX_BYTES: u64 = 64 << 20;

/// See the module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentState {
    /// The current content image's id.
    #[serde(default)]
    pub current: Option<String>,
    /// When `current` was recorded, unix seconds.
    #[serde(default)]
    pub recorded: i64,
    /// Content files by disk path (relative to the server folder): size and modification
    /// time in unix nanoseconds.
    #[serde(default)]
    pub baseline: BTreeMap<String, (u64, i64)>,
    /// The [`super::scope::SCOPE_RULES`] the baseline was taken under; another version's
    /// baseline is retaken rather than trusted.
    #[serde(default)]
    pub scope_rules: u32,
    /// Unix seconds since the server last started, while it runs.
    #[serde(default)]
    pub running_since: Option<i64>,
    /// What Modrinth said about the current image's files, by SHA-512.
    #[serde(default)]
    pub modrinth: BTreeMap<String, ModrinthHint>,
}

impl ContentState {
    /// Whether there is a baseline the before-start check can trust.
    pub fn has_baseline(&self) -> bool {
        self.current.is_some() && self.scope_rules == super::scope::SCOPE_RULES
    }
}

/// The state file's path under the store's root.
pub fn path(store_root: &Path, uuid: &InstanceUuid) -> PathBuf {
    store_root.join("images").join(uuid.as_ref()).join(FILE)
}

/// The instance's state; the default (no pointer, no baseline) when it is missing or
/// can't be read. Blocking.
pub fn load(store_root: &Path, uuid: &InstanceUuid) -> ContentState {
    let path = path(store_root, uuid);
    let Ok(meta) = std::fs::metadata(&path) else {
        return ContentState::default();
    };
    if meta.len() > MAX_BYTES {
        tracing::warn!(
            "{} is too large to be content state; ignoring it",
            path.display()
        );
        return ContentState::default();
    }
    match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<ContentState>(&bytes) {
            Ok(state) if state.current.as_deref().is_none_or(is_image_id) => state,
            Ok(_) => {
                tracing::warn!("{} names no image; ignoring it", path.display());
                ContentState::default()
            }
            Err(e) => {
                tracing::warn!("{} is damaged ({e}); ignoring it", path.display());
                ContentState::default()
            }
        },
        Err(_) => ContentState::default(),
    }
}

/// Replaces the instance's state, crash-safely (temp file, flushed, renamed). Blocking.
pub fn save(store_root: &Path, uuid: &InstanceUuid, state: &ContentState) -> Result<(), Error> {
    let path = path(store_root, uuid);
    let bytes = serde_json::to_vec(state)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to write the content state: {e}"))?;
    super::write_small_atomic(&path, &bytes)
}

fn is_image_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_damaged_and_odd_states_are_none() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::from("INSTANCE_a".to_string());
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        let mut state = ContentState {
            current: Some("a".repeat(64)),
            recorded: 5,
            scope_rules: super::super::scope::SCOPE_RULES,
            ..ContentState::default()
        };
        state.baseline.insert("mods/a.jar".into(), (3, 7));
        save(dir.path(), &uuid, &state).unwrap();
        assert_eq!(load(dir.path(), &uuid), state);
        assert!(state.has_baseline());

        std::fs::write(path(dir.path(), &uuid), b"{oops").unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        std::fs::write(path(dir.path(), &uuid), br#"{"current":"../x"}"#).unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        // Fields a newer core adds are ignored, and missing ones default.
        std::fs::write(path(dir.path(), &uuid), br#"{"later":1}"#).unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        // A baseline taken under other scope rules isn't trusted.
        let old = ContentState {
            scope_rules: 0,
            ..state
        };
        assert!(!old.has_baseline());
    }
}
