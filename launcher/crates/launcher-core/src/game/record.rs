//! Install records: which files were checked, so a launch need not hash them again.
//!
//! When the download engine downloads a file, or finds it and checks its SHA-1, it
//! records the file's path (relative to the data directory), size, modification time
//! and SHA-1. On the next launch, a file whose size and modification time still
//! match its record (and whose expected SHA-1 is the one recorded) is trusted
//! without being read. Anything else is hashed again, and downloaded again if the
//! hash is wrong. "Verify game files" ignores the records.
//!
//! There is one record per version (`versions/<id>/<id>.install.json`: the client
//! jar, the libraries and the logging config) and one per asset index
//! (`assets/indexes/<id>.install.json`). Each is keyed by the SHA-1 of the version
//! file or asset index it was made for; a record with another key is discarded.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::storage::{self, Document, Migration, StorageError};

/// What is known about one checked file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub size: u64,
    /// Nanoseconds since the Unix epoch.
    pub mtime_ns: u64,
    pub sha1: String,
}

/// An install record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecord {
    /// The SHA-1 of the version file or asset index this record is for.
    pub key: String,
    /// By path relative to the data directory, with `/` separators.
    #[serde(default)]
    pub files: BTreeMap<String, FileRecord>,
}

impl Document for InstallRecord {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

/// A file's size and modification time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stat {
    pub size: u64,
    pub mtime_ns: u64,
}

/// Reads a file's size and modification time; `None` if it is missing or not a
/// regular file.
pub fn stat(path: &Path) -> Option<Stat> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(Stat {
        size: meta.len(),
        mtime_ns: u64::try_from(mtime.as_nanos()).ok()?,
    })
}

impl InstallRecord {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            files: BTreeMap::new(),
        }
    }

    /// Loads the record at `path` if it was made for `key`. A missing, unreadable,
    /// newer or differently keyed record gives an empty one for `key`.
    pub fn load(path: &Path, key: &str) -> Self {
        match storage::load::<InstallRecord>(path) {
            Ok(loaded) if loaded.document.key.eq_ignore_ascii_case(key) => loaded.document,
            _ => Self::new(key),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), StorageError> {
        storage::save(path, self)
    }

    /// Whether the file at `relative` can be trusted to have `sha1`: it was recorded
    /// with that hash, and its size and modification time are unchanged.
    pub fn trusts(&self, relative: &str, sha1: &str, now: Option<Stat>) -> bool {
        match (self.files.get(relative), now) {
            (Some(record), Some(now)) => {
                record.sha1.eq_ignore_ascii_case(sha1)
                    && record.size == now.size
                    && record.mtime_ns == now.mtime_ns
            }
            _ => false,
        }
    }

    pub fn insert(&mut self, relative: String, sha1: &str, stat: Stat) {
        self.files.insert(
            relative,
            FileRecord {
                size: stat.size,
                mtime_ns: stat.mtime_ns,
                sha1: sha1.to_ascii_lowercase(),
            },
        );
    }
}

/// `path` relative to `root`, with `/` separators, if it is inside it.
pub fn relative_key(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts: Option<Vec<&str>> = relative
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Where a version's record goes: `versions/<id>/<id>.install.json`.
pub fn version_record_path(versions_dir: &Path, id: &str) -> PathBuf {
    versions_dir.join(id).join(format!("{id}.install.json"))
}

/// Where an asset index's record goes: `assets/indexes/<id>.install.json`.
pub fn assets_record_path(assets_dir: &Path, index_id: &str) -> PathBuf {
    assets_dir
        .join("indexes")
        .join(format!("{index_id}.install.json"))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn saves_atomically_and_reloads_by_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("versions/1.20.1/1.20.1.install.json");
        let mut record = InstallRecord::new("abc");
        record.insert(
            "libraries/a.jar".into(),
            "DEADBEEF",
            Stat {
                size: 3,
                mtime_ns: 1_700_000_000_123_456_789,
            },
        );
        record.save(&path).unwrap();
        // Only the record itself, no temporary files.
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["1.20.1.install.json"]);
        let raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], json!(1));
        assert_eq!(raw["files"]["libraries/a.jar"]["sha1"], json!("deadbeef"));
        assert_eq!(
            raw["files"]["libraries/a.jar"]["mtime_ns"],
            json!(1_700_000_000_123_456_789u64)
        );

        assert_eq!(InstallRecord::load(&path, "ABC"), record);
        // A record for another version file is thrown away.
        assert!(InstallRecord::load(&path, "other").files.is_empty());
        assert_eq!(
            InstallRecord::load(&dir.path().join("missing"), "k").key,
            "k"
        );
    }

    #[test]
    fn migrates_an_unversioned_record_and_ignores_corrupt_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.install.json");
        fs::write(
            &path,
            r#"{"key": "k", "files": {"a": {"size": 1, "mtime_ns": 2, "sha1": "s"}}}"#,
        )
        .unwrap();
        let record = InstallRecord::load(&path, "k");
        assert_eq!(record.files["a"].mtime_ns, 2);
        fs::write(&path, "{ not json").unwrap();
        assert!(InstallRecord::load(&path, "k").files.is_empty());
        fs::write(&path, r#"{"version": 99, "key": "k"}"#).unwrap();
        assert!(InstallRecord::load(&path, "k").files.is_empty());
    }

    #[test]
    fn trusts_only_unchanged_files_with_the_same_hash() {
        let mut record = InstallRecord::new("k");
        let stat = Stat {
            size: 10,
            mtime_ns: 5,
        };
        record.insert("a".into(), "aaaa", stat);
        assert!(record.trusts("a", "AAAA", Some(stat)));
        assert!(!record.trusts("a", "bbbb", Some(stat)));
        assert!(!record.trusts("a", "aaaa", Some(Stat { size: 11, ..stat })));
        assert!(!record.trusts(
            "a",
            "aaaa",
            Some(Stat {
                mtime_ns: 6,
                ..stat
            })
        ));
        assert!(!record.trusts("a", "aaaa", None));
        assert!(!record.trusts("b", "aaaa", Some(stat)));
    }

    #[test]
    fn stats_and_relative_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("libraries/x/y.jar");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"12345").unwrap();
        let stat = stat(&file).unwrap();
        assert_eq!(stat.size, 5);
        assert!(stat.mtime_ns > 0);
        assert_eq!(super::stat(dir.path()), None);
        assert_eq!(super::stat(&dir.path().join("missing")), None);
        assert_eq!(
            relative_key(dir.path(), &file).as_deref(),
            Some("libraries/x/y.jar")
        );
        assert_eq!(relative_key(&dir.path().join("other"), &file), None);
    }
}
