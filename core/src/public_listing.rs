//! Instances shown without signing in, for Lodestone Launcher: which instances are listed,
//! and the caches that keep the public routes cheap. The routes are in
//! `handlers::public_instances`.
//!
//! Nothing is public unless the owner turns on `list_instances_publicly` in the global
//! settings and someone who may change an instance's settings lists that instance.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use tracing::error;
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    traits::t_server::State,
    types::InstanceUuid,
};

/// How long the public instance list is reused.
pub const LIST_CACHE_TTL: Duration = Duration::from_secs(5);
/// How long the playit.gg tunnel list is reused, including a failed fetch.
pub const PLAYIT_CACHE_TTL: Duration = Duration::from_secs(60);

/// The mod loader of a listed instance.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PublicLoader {
    Vanilla,
    Fabric,
    Forge,
    Paper,
    Spigot,
    Other,
}

/// An instance as anyone may see it once it is listed. Only what a player needs to join:
/// no paths, description, owner, settings, console or player names.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export)]
pub struct PublicInstance {
    pub uuid: InstanceUuid,
    pub name: String,
    pub game_version: String,
    pub loader: PublicLoader,
    /// The Fabric loader version, the full Forge version (`1.20.1-47.4.28`) or the Paper
    /// build, when known.
    pub loader_version: Option<String>,
    /// The game port.
    pub port: u32,
    pub state: State,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
    /// A Fabric instance with at least one enabled mod, so it may have a client modpack.
    pub has_client_pack: bool,
    /// The public address of an active playit.gg tunnel to the game port.
    pub playit_address: Option<String>,
    /// Whether the instance is listed publicly. Always true on the public route.
    pub listed: bool,
}

/// `GET /public/instances` and `GET /launcher/instances`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export)]
pub struct PublicInstanceList {
    pub core_name: String,
    pub core_uuid: String,
    /// The global `list_instances_publicly` setting.
    pub listing_enabled: bool,
    pub instances: Vec<PublicInstance>,
}

/// `GET /instance/{uuid}/public_listing`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export)]
pub struct InstancePublicListing {
    pub listed: bool,
    /// The global switch: while it is off, a listed instance is not shown either.
    pub list_instances_publicly: bool,
}

/// `stores/public_listing.json`.
#[derive(Serialize, Deserialize, Default)]
struct StoredListing {
    listed: Vec<InstanceUuid>,
}

/// A client pack built for one state of an instance's mods folder.
pub struct BuiltPack {
    /// Holds the pack; dropping it deletes the pack.
    _temp_dir: tempfile::TempDir,
    pub path: PathBuf,
    pub file_name: String,
    /// SHA-256 of the pack, as hex.
    pub sha256: String,
    pub mod_count: u32,
    pub included_jars: u32,
}

impl BuiltPack {
    pub fn new(
        temp_dir: tempfile::TempDir,
        path: PathBuf,
        file_name: String,
        sha256: String,
        mod_count: u32,
        included_jars: u32,
    ) -> Self {
        Self {
            _temp_dir: temp_dir,
            path,
            file_name,
            sha256,
            mod_count,
            included_jars,
        }
    }

    /// The value of the `ETag` header.
    pub fn etag(&self) -> String {
        format!("\"{}\"", self.sha256)
    }
}

/// The outcome of building a pack, kept until the mods folder changes, so that repeated
/// requests do not scan and hash the mods again. A failure keeps its kind and message.
pub enum CachedPack {
    Built(Arc<BuiltPack>),
    Failed(ErrorKind, String),
}

/// The pack of one instance, for the fingerprint of its mods folder.
pub struct PackSlot {
    pub fingerprint: String,
    pub pack: CachedPack,
}

/// Which instances are listed, and the caches of the public routes.
pub struct PublicListing {
    path: PathBuf,
    /// Where built client packs are kept until they are replaced.
    pack_dir: PathBuf,
    listed: std::sync::RwLock<HashSet<InstanceUuid>>,
    /// Held while the file is written, so writes reach the disk in order.
    file_write: tokio::sync::Mutex<()>,
    list_cache: tokio::sync::Mutex<Option<(Instant, PublicInstanceList)>>,
    playit_cache: tokio::sync::Mutex<Option<(Instant, Vec<(u16, String)>)>>,
    packs: std::sync::Mutex<HashMap<InstanceUuid, Arc<tokio::sync::Mutex<Option<PackSlot>>>>>,
}

impl PublicListing {
    /// Loads the listed instances from `path`. A missing file means none; an unreadable
    /// one is logged and also means none.
    pub async fn load(path: PathBuf, pack_dir: PathBuf) -> Self {
        let listed = match tokio::fs::read(&path).await {
            Ok(data) => match serde_json::from_slice::<StoredListing>(&data) {
                Ok(stored) => stored.listed.into_iter().collect(),
                Err(e) => {
                    error!(
                        "{} is corrupt ({e}); no instance is listed publicly until one is listed again",
                        path.display()
                    );
                    HashSet::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashSet::new(),
            Err(e) => {
                error!(
                    "Failed to read {} ({e}); no instance is listed publicly",
                    path.display()
                );
                HashSet::new()
            }
        };
        Self {
            path,
            pack_dir,
            listed: std::sync::RwLock::new(listed),
            file_write: Default::default(),
            list_cache: Default::default(),
            playit_cache: Default::default(),
            packs: Default::default(),
        }
    }

    pub fn pack_dir(&self) -> &Path {
        &self.pack_dir
    }

    fn listed_lock(&self) -> std::sync::RwLockReadGuard<'_, HashSet<InstanceUuid>> {
        // only a set of ids: a panic while it was locked cannot leave it half-changed
        self.listed.read().unwrap_or_else(|p| p.into_inner())
    }

    pub fn is_listed(&self, uuid: &InstanceUuid) -> bool {
        self.listed_lock().contains(uuid)
    }

    /// Lists or unlists an instance and saves the set.
    pub async fn set_listed(&self, uuid: &InstanceUuid, listed: bool) -> Result<(), Error> {
        let _write = self.file_write.lock().await;
        let snapshot = {
            let mut set = self.listed.write().unwrap_or_else(|p| p.into_inner());
            let changed = if listed {
                set.insert(uuid.clone())
            } else {
                set.remove(uuid)
            };
            if !changed {
                return Ok(());
            }
            let mut snapshot: Vec<InstanceUuid> = set.iter().cloned().collect();
            snapshot.sort_by_key(|uuid| uuid.to_string());
            snapshot
        };
        self.invalidate().await;
        let json = serde_json::to_string_pretty(&StoredListing { listed: snapshot })
            .context("Failed to serialize the listed instances")?;
        crate::persist::write_atomic(&self.path, json, Some(0o600))
            .await
            .context(format!("Failed to write {}", self.path.display()))?;
        Ok(())
    }

    /// Forgets a deleted instance: unlists it and drops its pack.
    pub async fn forget(&self, uuid: &InstanceUuid) {
        if let Err(e) = self.set_listed(uuid, false).await {
            error!("Failed to unlist a deleted instance: {}", e.source);
        }
        self.packs_lock().remove(uuid);
    }

    /// Drops the cached public list, after a setting changed.
    pub async fn invalidate(&self) {
        *self.list_cache.lock().await = None;
    }

    /// The cached public list, if it is younger than [`LIST_CACHE_TTL`].
    pub async fn cached_list(&self) -> Option<PublicInstanceList> {
        let cache = self.list_cache.lock().await;
        cache
            .as_ref()
            .filter(|(at, _)| at.elapsed() < LIST_CACHE_TTL)
            .map(|(_, list)| list.clone())
    }

    pub async fn store_list(&self, list: PublicInstanceList) {
        *self.list_cache.lock().await = Some((Instant::now(), list));
    }

    /// The playit.gg addresses by local port: from the cache if it is younger than
    /// [`PLAYIT_CACHE_TTL`], else from `fetch`, whose failure counts as no tunnels.
    pub async fn playit_addresses<F, Fut>(&self, fetch: F) -> Vec<(u16, String)>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Vec<(u16, String)>>,
    {
        let mut cache = self.playit_cache.lock().await;
        if let Some((at, addresses)) = cache.as_ref() {
            if at.elapsed() < PLAYIT_CACHE_TTL {
                return addresses.clone();
            }
        }
        let addresses = fetch().await;
        *cache = Some((Instant::now(), addresses.clone()));
        addresses
    }

    fn packs_lock(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<InstanceUuid, Arc<tokio::sync::Mutex<Option<PackSlot>>>>>
    {
        self.packs.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The pack slot of an instance. Holding its lock makes one build at a time.
    pub fn pack_slot(&self, uuid: &InstanceUuid) -> Arc<tokio::sync::Mutex<Option<PackSlot>>> {
        self.packs_lock().entry(uuid.clone()).or_default().clone()
    }
}

/// A fingerprint of a mods folder and what else goes into its client pack: the names,
/// sizes and modification times of its entries, the instance name and the versions.
/// Runs on a blocking thread.
pub async fn pack_fingerprint(
    dir: PathBuf,
    instance_name: String,
    game_version: String,
    loader_version: Option<String>,
) -> Result<String, Error> {
    tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        let mut entries = Vec::new();
        match std::fs::read_dir(&dir) {
            Ok(read) => {
                for entry in read.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let (size, modified) = entry
                        .metadata()
                        .map(|m| {
                            let modified = m
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map_or(0, |d| d.as_nanos());
                            (m.len(), modified)
                        })
                        .unwrap_or_default();
                    entries.push((name, size, modified));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("Failed to read the mods folder")?,
        }
        entries.sort();
        let mut hasher = Sha256::new();
        for part in [
            instance_name.as_str(),
            game_version.as_str(),
            loader_version.as_deref().unwrap_or(""),
        ] {
            hasher.update(part.as_bytes());
            hasher.update([0]);
        }
        for (name, size, modified) in entries {
            hasher.update(format!("{name}\0{size}\0{modified}\n").as_bytes());
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await
    .context("The fingerprint task failed")?
}

/// Whether a mods folder has an enabled `.jar`. A missing folder has none.
pub async fn has_enabled_jar(dir: PathBuf) -> bool {
    tokio::task::spawn_blocking(move || {
        std::fs::read_dir(dir).is_ok_and(|read| {
            read.flatten().any(|entry| {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                !name.starts_with('.')
                    && name.ends_with(".jar")
                    && entry.file_type().is_ok_and(|t| t.is_file())
            })
        })
    })
    .await
    .unwrap_or(false)
}

/// SHA-256 of a file, as hex. Runs on a blocking thread.
pub async fn sha256_of(path: PathBuf) -> Result<String, Error> {
    tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut file =
            std::fs::File::open(&path).context(format!("Failed to open {}", path.display()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .context(format!("Failed to read {}", path.display()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok::<_, Error>(hex::encode(hasher.finalize()))
    })
    .await
    .context("The hashing task failed")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn listed_instances_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("public_listing.json");
        let listing = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        let a = InstanceUuid::from("a".to_string());
        let b = InstanceUuid::from("b".to_string());
        assert!(!listing.is_listed(&a));
        listing.set_listed(&a, true).await.unwrap();
        listing.set_listed(&b, true).await.unwrap();
        listing.set_listed(&b, false).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let again = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        assert!(again.is_listed(&a));
        assert!(!again.is_listed(&b));
        again.forget(&a).await;
        let third = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        assert!(!third.is_listed(&a));

        // a corrupt file lists nothing
        std::fs::write(&path, "{not json").unwrap();
        let corrupt = PublicListing::load(path, dir.path().join("packs")).await;
        assert!(!corrupt.is_listed(&a));
    }

    #[tokio::test]
    async fn fingerprint_follows_the_mods_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let print = || {
            pack_fingerprint(
                mods.clone(),
                "Server".into(),
                "1.20.1".into(),
                Some("0.16.14".into()),
            )
        };
        let missing = print().await.unwrap();
        std::fs::create_dir_all(&mods).unwrap();
        assert_eq!(print().await.unwrap(), missing);
        assert!(!has_enabled_jar(mods.clone()).await);
        std::fs::write(mods.join("a.jar.disabled"), b"x").unwrap();
        assert!(!has_enabled_jar(mods.clone()).await);
        let with_disabled = print().await.unwrap();
        assert_ne!(with_disabled, missing);
        std::fs::write(mods.join("a.jar"), b"x").unwrap();
        assert!(has_enabled_jar(mods.clone()).await);
        let with_jar = print().await.unwrap();
        assert_ne!(with_jar, with_disabled);
        assert_eq!(print().await.unwrap(), with_jar);
        let renamed = pack_fingerprint(mods, "Other".into(), "1.20.1".into(), None)
            .await
            .unwrap();
        assert_ne!(renamed, with_jar);
    }

    #[tokio::test]
    async fn playit_addresses_are_cached() {
        let dir = tempfile::tempdir().unwrap();
        let listing =
            PublicListing::load(dir.path().join("l.json"), dir.path().join("packs")).await;
        let first = listing
            .playit_addresses(|| async { vec![(25565, "a.ply.gg:1".to_string())] })
            .await;
        assert_eq!(first, vec![(25565, "a.ply.gg:1".to_string())]);
        // the second call does not fetch
        let second = listing
            .playit_addresses(|| async { panic!("fetched again") })
            .await;
        assert_eq!(second, first);
    }
}
