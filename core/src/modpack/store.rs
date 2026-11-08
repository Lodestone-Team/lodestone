//! Downloaded and uploaded `.mrpack` files, kept for an hour so that `create_modpack` can
//! reuse them by `upload_id`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use color_eyre::eyre::Context;

use super::ModpackInfo;
use crate::auth::user_id::UserId;
use crate::error::Error;

/// How long a pack is kept.
pub const PACK_TTL: Duration = Duration::from_secs(60 * 60);
/// At most this many packs are kept; the oldest is removed first.
const MAX_PACKS: usize = 10;

/// A stored pack. The file is deleted when the last clone is dropped, so a pack that is
/// being used for an instance stays until that instance is created.
#[derive(Clone)]
pub struct StoredPack {
    pub file: Arc<tempfile::TempPath>,
    pub info: ModpackInfo,
    /// The Modrinth version the pack was downloaded from.
    pub version_id: Option<String>,
    /// The user who uploaded the pack. Only they may use it. `None` for packs from
    /// Modrinth, which anyone may reuse.
    pub uploader: Option<UserId>,
    created: Instant,
    /// Insertion order, to find the oldest pack.
    seq: u64,
}

impl StoredPack {
    pub fn path(&self) -> PathBuf {
        self.file.to_path_buf()
    }
}

pub struct PackStore {
    dir: PathBuf,
    packs: Mutex<HashMap<String, StoredPack>>,
    next_seq: std::sync::atomic::AtomicU64,
}

impl PackStore {
    /// A store in `dir`. Packs left there by an earlier run are removed.
    pub fn new(dir: PathBuf) -> Self {
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!(
                    "Could not remove old modpack downloads in {}: {e}",
                    dir.display()
                );
            }
        }
        Self {
            dir,
            packs: Mutex::new(HashMap::new()),
            next_seq: Default::default(),
        }
    }

    /// A new empty file for a pack, deleted when dropped unless it is stored.
    pub fn new_file(&self) -> Result<tempfile::TempPath, Error> {
        std::fs::create_dir_all(&self.dir).context("Failed to create the modpack folder")?;
        Ok(tempfile::Builder::new()
            .prefix("pack-")
            .suffix(".mrpack")
            .tempfile_in(&self.dir)
            .context("Failed to create a file for the modpack")?
            .into_temp_path())
    }

    fn prune(packs: &mut HashMap<String, StoredPack>) {
        packs.retain(|_, p| p.created.elapsed() < PACK_TTL);
        while packs.len() > MAX_PACKS {
            let Some(oldest) = packs
                .iter()
                .min_by_key(|(_, p)| p.seq)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            packs.remove(&oldest);
        }
    }

    /// Stores a pack and returns its `upload_id`.
    pub fn insert(
        &self,
        file: tempfile::TempPath,
        info: ModpackInfo,
        version_id: Option<String>,
        uploader: Option<UserId>,
    ) -> String {
        let id = crate::util::rand_alphanumeric(32);
        let mut packs = self.packs.lock().unwrap_or_else(|e| e.into_inner());
        packs.insert(
            id.clone(),
            StoredPack {
                file: Arc::new(file),
                info: ModpackInfo {
                    upload_id: Some(id.clone()),
                    ..info
                },
                version_id,
                uploader,
                created: Instant::now(),
                seq: self
                    .next_seq
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            },
        );
        Self::prune(&mut packs);
        id
    }

    pub fn get(&self, id: &str) -> Option<StoredPack> {
        let mut packs = self.packs.lock().unwrap_or_else(|e| e.into_inner());
        Self::prune(&mut packs);
        packs.get(id).cloned()
    }

    /// A stored pack downloaded from the Modrinth version `version_id`.
    pub fn find_version(&self, version_id: &str) -> Option<StoredPack> {
        let mut packs = self.packs.lock().unwrap_or_else(|e| e.into_inner());
        Self::prune(&mut packs);
        packs
            .values()
            .filter(|p| p.version_id.as_deref() == Some(version_id))
            .max_by_key(|p| p.seq)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> ModpackInfo {
        ModpackInfo {
            name: "P".into(),
            version_number: None,
            summary: None,
            game_version: "1.20.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.15.0".into()),
            server_file_count: 0,
            server_download_size: 0,
            unsupported_reason: None,
            upload_id: None,
        }
    }

    #[test]
    fn stores_and_bounds_packs() {
        let dir = tempfile::tempdir().unwrap();
        let store = PackStore::new(dir.path().join("packs"));
        let file = store.new_file().unwrap();
        let path = file.to_path_buf();
        let id = store.insert(file, info(), Some("v1".into()), None);
        let pack = store.get(&id).unwrap();
        assert_eq!(pack.info.upload_id.as_deref(), Some(id.as_str()));
        assert_eq!(pack.path(), path);
        assert!(store.find_version("v1").is_some());
        assert!(store.find_version("v2").is_none());
        assert!(store.get("unknown").is_none());

        for _ in 0..MAX_PACKS {
            let file = store.new_file().unwrap();
            store.insert(file, info(), None, None);
        }
        // the oldest pack was removed, but its file stays while it is used
        assert!(store.get(&id).is_none());
        assert!(path.exists());
        drop(pack);
        assert!(!path.exists());
    }

    #[test]
    fn removes_old_files_on_start() {
        let dir = tempfile::tempdir().unwrap();
        let packs = dir.path().join("packs");
        std::fs::create_dir_all(&packs).unwrap();
        std::fs::write(packs.join("pack-old.mrpack"), "x").unwrap();
        let _store = PackStore::new(packs.clone());
        assert!(!packs.join("pack-old.mrpack").exists());
    }
}
