//! Where an instance's content files came from: `content.json` in the instance's folder.
//!
//! Each entry is keyed by `<folder>/<name without .disabled>`, so enabling or disabling a
//! mod keeps it. It holds the file's size, modification time and hashes, and the Modrinth
//! version it is (from an install, or from a lookup by SHA-1). A file whose size and
//! modification time still match its entry isn't hashed again. `looked_up` records that
//! Modrinth didn't know a file, so it isn't asked again on every listing.
//!
//! The file is a cache: deleting it only means the files are hashed and looked up again.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use lodestone_minecraft::modrinth::{ModrinthProject, RawVersion};
use serde::{Deserialize, Serialize};

use super::{ContentKind, enabled_name, scan::ScannedFile};
use crate::storage::{self, Document, Migration, StorageError};

pub const FILE: &str = "content.json";

/// The Modrinth version a file is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModrinthSource {
    pub project_id: String,
    pub version_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub date_published: String,
    /// The file's download URL on Modrinth's CDN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
}

impl ModrinthSource {
    /// The source of the file with `sha512` in `version` of `project`.
    pub fn new(version: &RawVersion, project: Option<&ModrinthProject>, sha512: &str) -> Self {
        let file = version
            .files
            .iter()
            .find(|f| {
                f.hashes
                    .sha512
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(sha512))
            })
            .or_else(|| version.primary_file());
        Self {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            slug: project.map(|p| p.slug.clone()).unwrap_or_default(),
            title: project
                .map(|p| p.title.clone())
                .unwrap_or_else(|| version.name.clone()),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            version_number: version.version_number.clone(),
            date_published: version.date_published.clone(),
            url: file.map(|f| f.url.clone()),
            client_side: project.map(|p| p.client_side.clone()).unwrap_or_default(),
            server_side: project.map(|p| p.server_side.clone()).unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub size: u64,
    /// Nanoseconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
    pub sha1: String,
    pub sha512: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrinth: Option<ModrinthSource>,
    /// Modrinth was asked about this file and didn't know it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looked_up: bool,
}

/// `content.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(default)]
    pub files: BTreeMap<String, Entry>,
}

impl Document for Provenance {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}

/// Nanoseconds since the Unix epoch.
pub fn nanos(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_nanos()).ok())
}

/// The key of a file of `kind` named `file_name`.
pub fn key(kind: ContentKind, file_name: &str) -> String {
    format!("{}/{}", kind.dir(), enabled_name(file_name))
}

pub fn path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(FILE)
}

/// Serialises the read-modify-write of `content.json` within the app: listings and
/// installs run in the background at the same time.
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl Provenance {
    /// Reads `content.json`, applies `change` and writes it back if it changed, with no
    /// other update in between.
    pub fn update(
        instance_dir: &Path,
        change: impl FnOnce(&mut Provenance),
    ) -> Result<Provenance, StorageError> {
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut provenance = Self::load(instance_dir);
        let before = provenance.clone();
        change(&mut provenance);
        if provenance != before {
            provenance.save(instance_dir)?;
        }
        Ok(provenance)
    }

    /// Takes in what a listing found for `kind` (`refreshed`) without undoing changes made
    /// since it started: for each file, the entry that matches the file on disk now wins
    /// (the one with a Modrinth source if both do), and files that are gone are dropped.
    pub fn merge_refreshed(&mut self, kind: ContentKind, refreshed: &Provenance, dir: &Path) {
        let prefix = format!("{}/", kind.dir());
        let stat = |name: &str| {
            let candidates = [name.to_string(), super::disabled_name(name)];
            candidates.iter().find_map(|candidate| {
                let metadata = std::fs::symlink_metadata(dir.join(candidate)).ok()?;
                Some((metadata.len(), metadata.modified().ok().and_then(nanos)))
            })
        };
        let mut keys: Vec<String> = self
            .files
            .keys()
            .chain(refreshed.files.keys())
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let name = &key[prefix.len()..];
            let Some((size, modified)) = stat(name) else {
                self.files.remove(&key);
                continue;
            };
            let fits = |e: &&Entry| e.size == size && e.modified == modified;
            let mine = self.files.get(&key).filter(fits);
            let theirs = refreshed.files.get(&key).filter(fits);
            let chosen = match (mine, theirs) {
                (Some(mine), Some(theirs))
                    if mine.modrinth.is_none()
                        && (theirs.modrinth.is_some() || theirs.looked_up) =>
                {
                    Some(theirs.clone())
                }
                (Some(mine), _) => Some(mine.clone()),
                (None, Some(theirs)) => Some(theirs.clone()),
                (None, None) => None,
            };
            match chosen {
                Some(entry) => {
                    self.files.insert(key, entry);
                }
                None => {
                    self.files.remove(&key);
                }
            }
        }
    }

    /// Reads `content.json`. A broken file is treated as empty: it is only a cache.
    pub fn load(instance_dir: &Path) -> Self {
        storage::load::<Provenance>(&path(instance_dir))
            .map(|loaded| loaded.document)
            .unwrap_or_default()
    }

    pub fn save(&self, instance_dir: &Path) -> Result<(), StorageError> {
        storage::save(&path(instance_dir), self)
    }

    pub fn get(&self, kind: ContentKind, file_name: &str) -> Option<&Entry> {
        self.files.get(&key(kind, file_name))
    }

    /// The entry of `file` if its size and modification time still match.
    pub fn current(&self, file: &ScannedFile) -> Option<&Entry> {
        let entry = self.get(file.kind, &file.file_name)?;
        let modified = file.modified.and_then(nanos);
        (entry.size == file.size && entry.modified == modified && modified.is_some())
            .then_some(entry)
    }

    pub fn insert(&mut self, kind: ContentKind, file_name: &str, entry: Entry) {
        self.files.insert(key(kind, file_name), entry);
    }

    pub fn remove(&mut self, kind: ContentKind, file_name: &str) -> Option<Entry> {
        self.files.remove(&key(kind, file_name))
    }

    /// Drops the entries of `kind` whose files are gone.
    pub fn retain_present(&mut self, kind: ContentKind, present: &[ScannedFile]) {
        let prefix = format!("{}/", kind.dir());
        let keep: Vec<String> = present.iter().map(|f| key(kind, &f.file_name)).collect();
        self.files
            .retain(|k, _| !k.starts_with(&prefix) || keep.contains(k));
    }

    /// The installed files of `kind` by Modrinth project: project id → file keys.
    pub fn by_project(&self, kind: ContentKind) -> BTreeMap<String, Vec<String>> {
        let prefix = format!("{}/", kind.dir());
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (k, entry) in &self.files {
            if let (Some(name), Some(source)) = (k.strip_prefix(&prefix), &entry.modrinth) {
                map.entry(source.project_id.clone())
                    .or_default()
                    .push(name.to_string());
            }
        }
        map
    }

    /// SHA-512 → Modrinth CDN URL, for files that came from Modrinth: what the `.mrpack`
    /// export lists in its index.
    pub fn download_urls(&self) -> std::collections::HashMap<String, (String, ModrinthSource)> {
        self.files
            .values()
            .filter_map(|entry| {
                let source = entry.modrinth.as_ref()?;
                let url = source.url.clone()?;
                Some((entry.sha512.clone(), (url, source.clone())))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_keys_ignore_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let mut provenance = Provenance::default();
        provenance.insert(
            ContentKind::Mod,
            "sodium.jar.disabled",
            Entry {
                size: 3,
                modified: Some(5),
                sha1: "a".into(),
                sha512: "b".into(),
                modrinth: Some(ModrinthSource {
                    project_id: "AANobbMI".into(),
                    version_id: "v1".into(),
                    url: Some("https://cdn.modrinth.com/x.jar".into()),
                    ..ModrinthSource::default()
                }),
                looked_up: false,
            },
        );
        provenance.save(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(text.contains("\"version\": 1"));
        assert!(text.contains("\"mods/sodium.jar\""));
        let loaded = Provenance::load(dir.path());
        assert_eq!(loaded, provenance);
        assert!(loaded.get(ContentKind::Mod, "sodium.jar").is_some());
        assert_eq!(
            loaded.by_project(ContentKind::Mod)["AANobbMI"],
            ["sodium.jar"]
        );
        assert!(loaded.by_project(ContentKind::ShaderPack).is_empty());
        assert_eq!(
            loaded.download_urls()["b"].0,
            "https://cdn.modrinth.com/x.jar"
        );
        // a broken file is an empty cache
        std::fs::write(dir.path().join(FILE), b"{oops").unwrap();
        assert_eq!(Provenance::load(dir.path()), Provenance::default());
    }

    #[test]
    fn a_listing_does_not_undo_a_newer_install() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("minecraft/mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"new file").unwrap();
        std::fs::write(mods.join("b.jar.disabled"), b"b").unwrap();
        let stat = |name: &str| {
            let m = std::fs::metadata(mods.join(name)).unwrap();
            (m.len(), m.modified().ok().and_then(nanos))
        };
        let entry = |name: &str, project: Option<&str>| {
            let (size, modified) = stat(name);
            Entry {
                size,
                modified,
                modrinth: project.map(|p| ModrinthSource {
                    project_id: p.into(),
                    ..ModrinthSource::default()
                }),
                ..Entry::default()
            }
        };
        // what the install wrote, after the listing had scanned
        let mut current = Provenance::default();
        current.insert(ContentKind::Mod, "a.jar", entry("a.jar", Some("new")));
        current.insert(ContentKind::Mod, "gone.jar", Entry::default());
        current.insert(ContentKind::ResourcePack, "p.zip", Entry::default());
        // what the listing found: a stale entry for a.jar, and b.jar looked up
        let mut refreshed = Provenance::default();
        refreshed.insert(
            ContentKind::Mod,
            "a.jar",
            Entry {
                size: 1,
                ..Entry::default()
            },
        );
        refreshed.insert(
            ContentKind::Mod,
            "b.jar.disabled",
            entry("b.jar.disabled", Some("b")),
        );
        current.merge_refreshed(ContentKind::Mod, &refreshed, &mods);
        assert_eq!(
            current
                .get(ContentKind::Mod, "a.jar")
                .unwrap()
                .modrinth
                .as_ref()
                .unwrap()
                .project_id,
            "new"
        );
        assert_eq!(
            current
                .get(ContentKind::Mod, "b.jar")
                .unwrap()
                .modrinth
                .as_ref()
                .unwrap()
                .project_id,
            "b"
        );
        assert!(current.get(ContentKind::Mod, "gone.jar").is_none());
        // other kinds are not touched
        assert!(current.get(ContentKind::ResourcePack, "p.zip").is_some());
    }
}
