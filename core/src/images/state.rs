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
//! - `excludes` are the canonical excludes (the backup policy's, translated) the baseline
//!   was taken under: files they leave out aren't in it, so the check compares under them,
//!   and a policy whose excludes changed since is a change of its own.
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
    /// The canonical excludes the baseline was taken under.
    #[serde(default)]
    pub excludes: Vec<String>,
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
pub fn path(store_root: &Path, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
    super::instance_file(store_root, uuid, FILE)
}

/// The instance's state; the default (no pointer, no baseline) when it is missing or
/// can't be read. Blocking.
pub fn load(store_root: &Path, uuid: &InstanceUuid) -> ContentState {
    let Ok(path) = path(store_root, uuid) else {
        return ContentState::default();
    };
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
            Ok(state)
                if state
                    .current
                    .as_deref()
                    .is_none_or(lodestone_image::hex::is_sha256) =>
            {
                state
            }
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

/// Only the pointer of a state file ([`current_pointer`]): the baseline and hints are
/// skipped unbuilt.
#[derive(Deserialize)]
struct Pointer {
    #[serde(default)]
    current: Option<String>,
}

/// The instance's current content image, read without building the rest of the state:
/// what serving players reads on every request. `None` when there is none or the file
/// can't be read, like [`load`]. Blocking.
pub fn current_pointer(store_root: &Path, uuid: &InstanceUuid) -> Option<String> {
    let path = path(store_root, uuid).ok()?;
    if std::fs::metadata(&path).ok()?.len() > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice::<Pointer>(&bytes)
        .ok()?
        .current
        .filter(|id| lodestone_image::hex::is_sha256(id))
}

/// Replaces the instance's state, crash-safely (temp file, flushed, renamed). Blocking.
pub fn save(store_root: &Path, uuid: &InstanceUuid, state: &ContentState) -> Result<(), Error> {
    let path = path(store_root, uuid)?;
    let bytes = serde_json::to_vec(state)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to write the content state: {e}"))?;
    super::write_small_atomic(&path, &bytes)
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

        std::fs::write(path(dir.path(), &uuid).unwrap(), b"{oops").unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        std::fs::write(path(dir.path(), &uuid).unwrap(), br#"{"current":"../x"}"#).unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        // Fields a newer core adds are ignored, and missing ones default.
        std::fs::write(path(dir.path(), &uuid).unwrap(), br#"{"later":1}"#).unwrap();
        assert_eq!(load(dir.path(), &uuid), ContentState::default());
        // The pointer alone reads the same.
        std::fs::write(path(dir.path(), &uuid).unwrap(), b"{oops").unwrap();
        assert_eq!(current_pointer(dir.path(), &uuid), None);
        save(dir.path(), &uuid, &state).unwrap();
        assert_eq!(current_pointer(dir.path(), &uuid), state.current);
        std::fs::write(path(dir.path(), &uuid).unwrap(), br#"{"current":"../x"}"#).unwrap();
        assert_eq!(current_pointer(dir.path(), &uuid), None);
        // A baseline taken under other scope rules isn't trusted.
        let old = ContentState {
            scope_rules: 0,
            ..state
        };
        assert!(!old.has_baseline());
    }
}
