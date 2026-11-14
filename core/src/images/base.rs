//! `images/<instance>/pack.json`: the modpack a server's content is built on, as content
//! images record it.
//!
//! The modpack code writes it when it installs, updates or restores a pack: the pack as an
//! image's `base` (`modrinth_pack` with the `.mrpack`'s SHA-512) and every file the pack
//! wrote, by server disk path, with what the pack's index says about it (hashes, its CDN
//! URL, its `env`). The recorder reads it to give a content image its base, a `modrinth`
//! or `base` source for each file the pack wrote and still has the pack's bytes, and the
//! pack's `env` as the `base_pack` input. Without it, files are plain `object`s with what
//! else is known of their `env`.
//!
//! The file isn't named `<sha256>.json`, so the store never takes it for a manifest.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lodestone_image::{Base, Env};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::types::InstanceUuid;

const FILE: &str = "pack.json";
/// A pack's file list is at most this large; a larger file is damaged.
const MAX_BYTES: u64 = 256 << 20;

/// See the module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseInfo {
    /// The pack, as an image's base.
    #[serde(default)]
    pub base: Option<Base>,
    /// The files the pack wrote, by disk path relative to the server folder.
    #[serde(default)]
    pub files: BTreeMap<String, BaseFile>,
}

/// One file a pack wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseFile {
    /// The pack index's SHA-1 (what the modpack record keys files by).
    pub sha1: String,
    /// The SHA-256, when the writer knows it: matching by it needs no reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// The pack index's `env` for the file; unknown when it has none (overrides).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Env>,
    /// Where the pack downloads it from, when that is Modrinth's CDN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrinth_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

/// The file's path under the store's root.
pub fn path(store_root: &Path, uuid: &InstanceUuid) -> PathBuf {
    store_root.join("images").join(uuid.as_ref()).join(FILE)
}

/// The instance's pack, if it has one recorded. A damaged file is none, with a warning.
/// Blocking.
pub fn load_base(store_root: &Path, uuid: &InstanceUuid) -> Option<BaseInfo> {
    let path = path(store_root, uuid);
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_BYTES {
        tracing::warn!("{} is too large; ignoring it", path.display());
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(info) => Some(info),
        Err(e) => {
            tracing::warn!("{} is damaged ({e}); ignoring it", path.display());
            None
        }
    }
}

/// Replaces the instance's pack, crash-safely. Blocking.
pub fn save_base(store_root: &Path, uuid: &InstanceUuid, info: &BaseInfo) -> Result<(), Error> {
    let path = path(store_root, uuid);
    let bytes = serde_json::to_vec(info)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to write the pack's files: {e}"))?;
    super::write_small_atomic(&path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lodestone_image::EnvValue;

    #[test]
    fn round_trip_and_damage() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::from("INSTANCE_b".to_string());
        assert_eq!(load_base(dir.path(), &uuid), None);
        let mut info = BaseInfo {
            base: Some(Base::ModrinthPack {
                project: "p".into(),
                version: "v".into(),
                sha512: "0".repeat(128),
            }),
            files: BTreeMap::new(),
        };
        info.files.insert(
            "mods/a.jar".into(),
            BaseFile {
                sha1: "1".repeat(40),
                env: Some(Env::new(EnvValue::Required, EnvValue::Unsupported)),
                ..BaseFile::default()
            },
        );
        save_base(dir.path(), &uuid, &info).unwrap();
        assert_eq!(load_base(dir.path(), &uuid), Some(info));
        std::fs::write(path(dir.path(), &uuid), b"[").unwrap();
        assert_eq!(load_base(dir.path(), &uuid), None);
    }
}
