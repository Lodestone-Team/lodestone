//! Instances shown without signing in, for Lodestone Launcher: which instances are listed,
//! and the caches that keep the public routes cheap. The routes are in
//! `handlers::public_instances`.
//!
//! Nothing is public unless the owner turns on `list_instances_publicly` in the global
//! settings and lists the instance. Each instance's [`PublicScope`] then chooses what the
//! public routes show of it.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    traits::t_server::State,
    types::InstanceUuid,
};

/// How long the public instance list is reused.
pub const LIST_CACHE_TTL: Duration = Duration::from_secs(5);

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

/// What the public routes show of an instance: independent flags, changed only by the
/// owner. Signed-in routes (`/launcher/...`) ignore them and follow the requester's
/// permissions.
///
/// | flag          | public fields                                   |
/// |---------------|-------------------------------------------------|
/// | `listed`      | `uuid`, `name`, `game_version`, `loader`, `loader_version`, `listed` |
/// | `status`      | `state`, `player_count`, `max_player_count`     |
/// | `address`     | `port`                                          |
/// | `client_pack` | `has_client_pack`, and the client-pack download |
///
/// Without `listed` nothing is shown, whatever the other flags say. The fields of a flag
/// that is off are left out of the JSON.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[serde(default)]
#[ts(export)]
pub struct PublicScope {
    pub listed: bool,
    pub status: bool,
    pub address: bool,
    pub client_pack: bool,
}

impl Default for PublicScope {
    /// Not listed; once listed, status and address are shown, the client pack is not.
    fn default() -> Self {
        Self {
            listed: false,
            status: true,
            address: true,
            client_pack: false,
        }
    }
}

impl PublicScope {
    /// Everything, as an instance was shown when listed before scopes existed.
    pub const ALL: Self = Self {
        listed: true,
        status: true,
        address: true,
        client_pack: true,
    };
}

/// A change to a [`PublicScope`]: the flags given are set, the others kept.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct PublicScopeChange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub listed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub address: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub client_pack: Option<bool>,
}

impl PublicScopeChange {
    pub fn apply(&self, scope: &mut PublicScope) {
        let set = |flag: &mut bool, value: Option<bool>| {
            if let Some(value) = value {
                *flag = value;
            }
        };
        set(&mut scope.listed, self.listed);
        set(&mut scope.status, self.status);
        set(&mut scope.address, self.address);
        set(&mut scope.client_pack, self.client_pack);
    }
}

/// An instance as anyone may see it once it is listed, limited by its [`PublicScope`].
/// Only what a player needs to join: no paths, description, owner, settings, console or
/// player names. A field left out was not shared; a `null` one was shared but is unknown.
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
    /// The game port. Scope `address`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub port: Option<u32>,
    /// Scope `status`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub state: Option<State>,
    /// Scope `status`; `null` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub player_count: Option<Option<u32>>,
    /// Scope `status`; `null` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_player_count: Option<Option<u32>>,
    /// A Fabric instance with at least one enabled mod, so it may have a client modpack.
    /// Scope `client_pack`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub has_client_pack: Option<bool>,
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
    #[serde(flatten)]
    #[ts(flatten)]
    pub scope: PublicScope,
    /// The global switch: while it is off, a listed instance is not shown either.
    pub list_instances_publicly: bool,
}

/// One instance in `GET /public_listing`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export)]
pub struct InstancePublicScope {
    pub uuid: InstanceUuid,
    pub name: String,
    #[serde(flatten)]
    #[ts(flatten)]
    pub scope: PublicScope,
}

/// `GET /public_listing`: every Minecraft Java instance and what it shows publicly, for
/// the owner.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export)]
pub struct PublicListingOverview {
    pub list_instances_publicly: bool,
    pub instances: Vec<InstancePublicScope>,
}

/// The version of `stores/public_listing.json` this core writes.
const STORE_VERSION: u32 = 2;

/// `stores/public_listing.json`: the scope of each instance that has one. An instance
/// that is not in it has the default scope (not listed).
#[derive(Serialize, Deserialize, Default)]
struct StoredListing {
    version: u32,
    instances: BTreeMap<String, PublicScope>,
}

/// The file as M8a wrote it: the listed instances, which showed everything.
#[derive(Deserialize)]
struct StoredListingV1 {
    listed: Vec<InstanceUuid>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredFile {
    Current(StoredListing),
    V1(StoredListingV1),
}

/// Reads the file, and whether it is in the old format. An instance listed in the old
/// format keeps showing everything it showed.
fn parse_stored(
    data: &[u8],
) -> Result<(HashMap<InstanceUuid, PublicScope>, bool), serde_json::Error> {
    Ok(match serde_json::from_slice::<StoredFile>(data)? {
        StoredFile::Current(stored) => (
            stored
                .instances
                .into_iter()
                .map(|(uuid, scope)| (InstanceUuid::from(uuid), scope))
                .collect(),
            false,
        ),
        StoredFile::V1(old) => (
            old.listed
                .into_iter()
                .map(|uuid| (uuid, PublicScope::ALL))
                .collect(),
            true,
        ),
    })
}

fn serialize_stored(scopes: &HashMap<InstanceUuid, PublicScope>) -> Result<String, Error> {
    let stored = StoredListing {
        version: STORE_VERSION,
        instances: scopes
            .iter()
            .map(|(uuid, scope)| (uuid.to_string(), *scope))
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&stored).context("Failed to serialize the public listing")?)
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

/// What each instance shows publicly, and the caches of the public routes.
pub struct PublicListing {
    path: PathBuf,
    /// Where built client packs are kept until they are replaced.
    pack_dir: PathBuf,
    scopes: std::sync::RwLock<HashMap<InstanceUuid, PublicScope>>,
    /// Held while the file is written, so writes reach the disk in order.
    file_write: tokio::sync::Mutex<()>,
    list_cache: tokio::sync::Mutex<Option<(Instant, PublicInstanceList)>>,
    packs: std::sync::Mutex<HashMap<InstanceUuid, Arc<tokio::sync::Mutex<Option<PackSlot>>>>>,
}

impl PublicListing {
    /// Loads the scopes from `path`. A missing file means none is listed; an unreadable
    /// one is logged and also means none. A file in the old format (a list of listed
    /// instances) is rewritten in the current one, each listed instance showing
    /// everything.
    pub async fn load(path: PathBuf, pack_dir: PathBuf) -> Self {
        let mut migrate = false;
        let scopes = match tokio::fs::read(&path).await {
            Ok(data) => match parse_stored(&data) {
                Ok((scopes, old)) => {
                    migrate = old;
                    scopes
                }
                Err(e) => {
                    error!(
                        "{} is corrupt ({e}); no instance is listed publicly until one is listed again",
                        path.display()
                    );
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => {
                error!(
                    "Failed to read {} ({e}); no instance is listed publicly",
                    path.display()
                );
                HashMap::new()
            }
        };
        if migrate {
            let written = match serialize_stored(&scopes) {
                Ok(json) => crate::persist::write_atomic(&path, json, Some(0o600)).await,
                Err(e) => Err(e),
            };
            match written {
                Ok(()) => info!(
                    "Updated {} to the format with public scopes",
                    path.display()
                ),
                Err(e) => error!(
                    "Failed to update {} to the format with public scopes: {}",
                    path.display(),
                    e.source
                ),
            }
        }
        Self {
            path,
            pack_dir,
            scopes: std::sync::RwLock::new(scopes),
            file_write: Default::default(),
            list_cache: Default::default(),
            packs: Default::default(),
        }
    }

    pub fn pack_dir(&self) -> &Path {
        &self.pack_dir
    }

    fn scopes_lock(&self) -> std::sync::RwLockReadGuard<'_, HashMap<InstanceUuid, PublicScope>> {
        // only plain values: a panic while it was locked cannot leave it half-changed
        self.scopes.read().unwrap_or_else(|p| p.into_inner())
    }

    /// The scope of an instance; the default (not listed) when it has none.
    pub fn scope(&self, uuid: &InstanceUuid) -> PublicScope {
        self.scopes_lock().get(uuid).copied().unwrap_or_default()
    }

    pub fn is_listed(&self, uuid: &InstanceUuid) -> bool {
        self.scope(uuid).listed
    }

    /// Changes the scope of an instance and saves the file. Returns the new scope.
    pub async fn change(
        &self,
        uuid: &InstanceUuid,
        change: &PublicScopeChange,
    ) -> Result<PublicScope, Error> {
        let _write = self.file_write.lock().await;
        let (scope, json) = {
            let mut scopes = self.scopes.write().unwrap_or_else(|p| p.into_inner());
            let before = scopes.get(uuid).copied().unwrap_or_default();
            let mut scope = before;
            change.apply(&mut scope);
            if scope == before {
                return Ok(scope);
            }
            if scope == PublicScope::default() {
                scopes.remove(uuid);
            } else {
                scopes.insert(uuid.clone(), scope);
            }
            (scope, serialize_stored(&scopes)?)
        };
        self.invalidate().await;
        crate::persist::write_atomic(&self.path, json, Some(0o600))
            .await
            .context(format!("Failed to write {}", self.path.display()))?;
        Ok(scope)
    }

    /// Lists or unlists an instance, keeping the rest of its scope.
    #[cfg(test)]
    pub async fn set_listed(&self, uuid: &InstanceUuid, listed: bool) -> Result<(), Error> {
        self.change(
            uuid,
            &PublicScopeChange {
                listed: Some(listed),
                ..Default::default()
            },
        )
        .await
        .map(|_| ())
    }

    /// Forgets a deleted instance: drops its scope and its pack.
    pub async fn forget(&self, uuid: &InstanceUuid) {
        let _write = self.file_write.lock().await;
        let json = {
            let mut scopes = self.scopes.write().unwrap_or_else(|p| p.into_inner());
            if scopes.remove(uuid).is_none() {
                None
            } else {
                Some(serialize_stored(&scopes))
            }
        };
        self.packs_lock().remove(uuid);
        let Some(json) = json else {
            return;
        };
        self.invalidate().await;
        let written = match json {
            Ok(json) => crate::persist::write_atomic(&self.path, json, Some(0o600)).await,
            Err(e) => Err(e),
        };
        if let Err(e) = written {
            error!("Failed to unlist a deleted instance: {}", e.source);
        }
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
    use serde_json::{json, Value};

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
    async fn scopes_persist_and_unlisting_keeps_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("public_listing.json");
        let listing = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        let a = InstanceUuid::from("a".to_string());
        assert_eq!(listing.scope(&a), PublicScope::default());
        listing.set_listed(&a, true).await.unwrap();
        // the defaults when listing: status and address, no client pack
        assert_eq!(
            listing.scope(&a),
            PublicScope {
                listed: true,
                status: true,
                address: true,
                client_pack: false
            }
        );
        let scope = listing
            .change(
                &a,
                &PublicScopeChange {
                    address: Some(false),
                    client_pack: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(scope.listed && scope.status && !scope.address && scope.client_pack);
        listing.set_listed(&a, false).await.unwrap();
        let again = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        assert_eq!(
            again.scope(&a),
            PublicScope {
                listed: false,
                ..scope
            }
        );
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            stored,
            json!({ "version": 2, "instances": { "a": {
                "listed": false, "status": true, "address": false, "client_pack": true
            } } })
        );
        // back to the default: dropped from the file
        again
            .change(
                &a,
                &PublicScopeChange {
                    address: Some(true),
                    client_pack: Some(false),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored, json!({ "version": 2, "instances": {} }));
    }

    #[tokio::test]
    async fn the_old_format_migrates_to_showing_everything() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("public_listing.json");
        std::fs::write(&path, r#"{ "listed": ["b", "a"] }"#).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let listing = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        let (a, b, c) = (
            InstanceUuid::from("a".to_string()),
            InstanceUuid::from("b".to_string()),
            InstanceUuid::from("c".to_string()),
        );
        assert_eq!(listing.scope(&a), PublicScope::ALL);
        assert_eq!(listing.scope(&b), PublicScope::ALL);
        assert_eq!(listing.scope(&c), PublicScope::default());

        // rewritten at once, in the new format, owner-only
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let all = json!({ "listed": true, "status": true, "address": true, "client_pack": true });
        assert_eq!(
            stored,
            json!({ "version": 2, "instances": { "a": all, "b": all } })
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // no temporary file is left next to it
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["public_listing.json".to_string()]);

        // and loads the same again, without migrating twice
        let again = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        assert_eq!(again.scope(&a), PublicScope::ALL);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            serde_json::to_vec_pretty(&stored).unwrap()
        );

        // an empty old file migrates to nothing listed
        std::fs::write(&path, r#"{ "listed": [] }"#).unwrap();
        let empty = PublicListing::load(path.clone(), dir.path().join("packs")).await;
        assert!(!empty.is_listed(&a));
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored, json!({ "version": 2, "instances": {} }));
    }

    #[test]
    fn a_scope_change_needs_known_flags() {
        assert!(serde_json::from_str::<PublicScopeChange>(r#"{"listed": true}"#).is_ok());
        assert!(serde_json::from_str::<PublicScopeChange>(r#"{"players": true}"#).is_err());
        // a stored scope missing a flag takes its default
        let scope: PublicScope = serde_json::from_str(r#"{"listed": true}"#).unwrap();
        assert_eq!(
            scope,
            PublicScope {
                listed: true,
                ..Default::default()
            }
        );
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
}
