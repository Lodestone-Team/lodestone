//! The backups' own small files, and helpers the backup code shares.
//!
//! A server's backups are data images in the core's one image store (`<lodestone>/backups`,
//! owned by [`crate::images::ImageService`]). What the backups keep beside the images is in
//! the server's image folder, `images/<uuid>/`, so a server has one folder, which goes when
//! the server is deleted:
//!
//! | File | What |
//! | --- | --- |
//! | `<image id>.json`, `stat-cache*.json` | the image store's (manifests, stat caches) |
//! | `backups.json` | the server's backups ([`Records`]), each naming its data image |
//! | `backup-policy.json` | the schedule, retention and exclude patterns |
//! | `restore-journal.json` | a restore's swap in progress (`super::restore`) |
//! | `save-off.marker` | a live backup has the server's saving off (`super::live`) |
//!
//! The store takes only `<64 hex>.json` for a manifest, so it ignores these. Each is
//! replaced crash-safely through a temporary file named as the store names its own
//! (`.incoming-<16 hex>.tmp`), which the store's sweep deletes after a crash.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use color_eyre::eyre::eyre;
use rand::RngExt;
use tracing::warn;

use crate::error::{Error, ErrorKind};
use crate::images::{write_small_atomic, ImageService};
use crate::types::InstanceUuid;

use super::types::{BackupPolicy, Records, MAX_NAME_LEN};

const RECORDS_FILE: &str = "backups.json";
const POLICY_FILE: &str = "backup-policy.json";
const JOURNAL_FILE: &str = "restore-journal.json";
const MARKER_FILE: &str = "save-off.marker";

/// Checked between files; true means stop.
pub type Cancel<'a> = &'a (dyn Fn() -> bool + Sync);

pub fn err(kind: ErrorKind, message: impl Into<String>) -> Error {
    Error {
        kind,
        source: eyre!(message.into()),
    }
}

/// The error of work stopped by its cancellation check.
pub fn cancelled() -> Error {
    err(ErrorKind::Internal, "Cancelled")
}

// ---------------------------------------------------------------------------------------
// Names and ids

/// Backup ids: `YYYYMMDD-HHMMSS-xxxx` when made here; any `[A-Za-z0-9-]{1,64}` is
/// accepted. They are only ever looked up in `backups.json`, never used as file names.
pub fn valid_backup_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// A new backup id: the UTC time and 4 random hex digits.
pub fn new_backup_id(now: DateTime<Utc>) -> String {
    format!(
        "{}-{:04x}",
        now.format("%Y%m%d-%H%M%S"),
        rand::rng().random::<u16>()
    )
}

/// Whether `hash` is a SHA-256 as the store names things: 64 lowercase hex digits.
pub fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `path` can be joined onto a folder safely: `/`-separated plain names, without
/// `\`, `:`, control characters, `.`, `..` or empty parts.
pub fn valid_rel_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part
                    .chars()
                    .any(|c| c.is_control() || c == '\\' || c == ':')
        })
}

/// `path` (valid per [`valid_rel_path`]) under `base`.
pub fn join_rel(base: &Path, path: &str) -> PathBuf {
    let mut joined = base.to_path_buf();
    for part in path.split('/') {
        joined.push(part);
    }
    joined
}

/// A backup name from the user: trimmed, `None` when empty.
pub fn check_backup_name(name: &str) -> Result<Option<String>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(format!(
            "Backup names can be at most {MAX_NAME_LEN} characters long"
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("Backup names can't have control characters".to_string());
    }
    Ok(Some(name.to_string()))
}

/// Renames, retrying for up to `budget` while Windows reports the file in use. Elsewhere,
/// and for other errors, fails at once.
pub fn rename_with_retry(from: &Path, to: &Path, budget: Duration) -> io::Result<()> {
    let started = Instant::now();
    let mut wait = Duration::from_millis(20);
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if is_in_use(&e) && started.elapsed() + wait <= budget => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(500));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Access denied, sharing violation or lock violation: on Windows, something has the file
/// (or a file in the folder) open.
fn is_in_use(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

// ---------------------------------------------------------------------------------------
// The files

/// The backups' files in the image store's folder (see the module). Cheap to make; reads
/// and writes are blocking.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// The files under `root`, the image store's folder (`<lodestone>/backups`).
    pub fn new(root: PathBuf) -> Store {
        Store { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `images/`, which holds a folder per server.
    pub fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    /// `images/<uuid>/`. A uuid images can't name (Docker containers, or a uuid from a URL
    /// that isn't one of the core's) is a bad request: it never reaches the disk.
    pub fn instance_dir(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        if !ImageService::accepts(uuid) {
            return Err(err(ErrorKind::BadRequest, "Invalid server id"));
        }
        Ok(self.images_dir().join(uuid.as_ref()))
    }

    pub fn records_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.instance_dir(uuid)?.join(RECORDS_FILE))
    }

    pub fn policy_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.instance_dir(uuid)?.join(POLICY_FILE))
    }

    pub fn journal_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.instance_dir(uuid)?.join(JOURNAL_FILE))
    }

    pub fn marker_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.instance_dir(uuid)?.join(MARKER_FILE))
    }

    /// The server's backups; none when it has no `backups.json`. A file that can't be read
    /// or has a record that isn't valid is an error, never taken for no backups: the images
    /// no record names are deleted as leftovers.
    pub fn load_records(&self, uuid: &InstanceUuid) -> Result<Records, Error> {
        let path = self.records_path(uuid)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Records::default()),
            Err(e) => {
                return Err(Error::from(
                    eyre!(e).wrap_err(format!("Failed to read {}", path.display())),
                ))
            }
        };
        let records: Records = serde_json::from_slice(&bytes).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("The backup list {} can't be read: {e}", path.display()),
            )
        })?;
        if let Some(bad) = records
            .backups
            .iter()
            .find(|r| !valid_backup_id(&r.id) || !valid_sha256(&r.image))
        {
            return Err(err(
                ErrorKind::Internal,
                format!(
                    "The backup list {} has an invalid backup {:?}",
                    path.display(),
                    bad.id
                ),
            ));
        }
        Ok(records)
    }

    pub fn save_records(&self, uuid: &InstanceUuid, records: &Records) -> Result<(), Error> {
        let json = serde_json::to_vec_pretty(records).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("Failed to write the backups: {e}"),
            )
        })?;
        write_small_atomic(&self.records_path(uuid)?, &json)
    }

    /// The server's policy; the default when it has none (or it can't be read).
    pub fn load_policy(&self, uuid: &InstanceUuid) -> Result<BackupPolicy, Error> {
        let path = self.policy_path(uuid)?;
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(policy) => Ok(policy),
                Err(e) => {
                    warn!(
                        "The backup policy {} can't be read, using the default: {e}",
                        path.display()
                    );
                    Ok(BackupPolicy::default())
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BackupPolicy::default()),
            Err(e) => Err(Error::from(
                eyre!(e).wrap_err("Failed to read the backup policy"),
            )),
        }
    }

    pub fn save_policy(&self, uuid: &InstanceUuid, policy: &BackupPolicy) -> Result<(), Error> {
        policy
            .validate()
            .map_err(|e| err(ErrorKind::BadRequest, e))?;
        let json = serde_json::to_vec_pretty(policy).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("Failed to write the policy: {e}"),
            )
        })?;
        write_small_atomic(&self.policy_path(uuid)?, &json)
    }

    /// The uuids of the server folders under `images/`, by name. Folders that aren't a
    /// uuid images take are skipped. `Err` when `images/` can't be listed (missing is no
    /// servers).
    pub fn instances(&self) -> io::Result<Vec<InstanceUuid>> {
        let entries = match fs::read_dir(self.images_dir()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut found = Vec::new();
        for entry in entries {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(String::from) else {
                continue;
            };
            let uuid = InstanceUuid::from(name);
            if ImageService::accepts(&uuid) && entry.file_type().is_ok_and(|t| t.is_dir()) {
                found.push(uuid);
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::backups::types::{BackupKind, BackupRecord};

    pub fn uuid() -> InstanceUuid {
        InstanceUuid::from("INSTANCE_test".to_string())
    }

    pub fn record(id: &str, image: &str, kind: BackupKind, created: i64) -> BackupRecord {
        BackupRecord {
            id: id.to_string(),
            image: image.to_string(),
            name: None,
            kind,
            created,
            pinned: false,
            live: false,
            quiesced: true,
            level: Some("world".to_string()),
            instance_name: "Survival".to_string(),
            game_version: "1.21.1".to_string(),
            file_count: 1,
            withheld_count: 0,
            total_bytes: 10,
            new_bytes: 10,
            stored_bytes: 5,
            duration_ms: 1,
        }
    }

    #[test]
    fn ids_and_names() {
        let id = new_backup_id(Utc::now());
        assert!(valid_backup_id(&id), "{id}");
        assert_eq!(id.len(), "20260101-000000-abcd".len());
        for bad in ["", "a/b", "../x", "a.json", &"a".repeat(65), "a b"] {
            assert!(!valid_backup_id(bad), "{bad:?}");
        }
        assert_eq!(check_backup_name("  x  "), Ok(Some("x".to_string())));
        assert_eq!(check_backup_name("   "), Ok(None));
        assert!(check_backup_name(&"x".repeat(MAX_NAME_LEN + 1)).is_err());
        assert!(check_backup_name("a\nb").is_err());
        assert!(valid_sha256(&"ab".repeat(32)));
        assert!(!valid_sha256(&"AB".repeat(32)));
        assert!(!valid_sha256("ab"));
    }

    #[test]
    fn rel_paths() {
        for good in ["a", "a/b", "world/region/r.0.0.mca", ".lodestone_config"] {
            assert!(valid_rel_path(good), "{good}");
        }
        for bad in [
            "", "/a", "a/", "a//b", "a/./b", "../a", "a\\b", "c:x", "a\nb",
        ] {
            assert!(!valid_rel_path(bad), "{bad:?}");
        }
        assert_eq!(
            join_rel(Path::new("/r"), "a/b"),
            Path::new("/r").join("a").join("b")
        );
    }

    #[test]
    fn records_round_trip_and_refuse_what_isnt_valid() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let uuid = uuid();
        assert_eq!(store.load_records(&uuid).unwrap(), Records::default());
        let records = Records {
            backups: vec![
                record(
                    "20260101-000000-aaaa",
                    &"aa".repeat(32),
                    BackupKind::Manual,
                    1,
                ),
                record(
                    "20260101-000100-bbbb",
                    &"bb".repeat(32),
                    BackupKind::Scheduled,
                    2,
                ),
            ],
            scheduled_since_full: 3,
        };
        store.save_records(&uuid, &records).unwrap();
        assert_eq!(store.load_records(&uuid).unwrap(), records);
        let newest: Vec<&str> = records
            .newest_first()
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(newest, ["20260101-000100-bbbb", "20260101-000000-aaaa"]);
        assert!(records.names_image(&"aa".repeat(32)));
        assert!(!records.names_image(&"cc".repeat(32)));
        // no temporary file is left
        let names: Vec<String> = fs::read_dir(store.instance_dir(&uuid).unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["backups.json"]);

        // a damaged list is an error, never "no backups"
        fs::write(store.records_path(&uuid).unwrap(), b"{not json").unwrap();
        assert!(store.load_records(&uuid).is_err());
        let mut bad = records.clone();
        bad.backups[0].image = "../x".to_string();
        fs::write(
            store.records_path(&uuid).unwrap(),
            serde_json::to_vec(&bad).unwrap(),
        )
        .unwrap();
        assert!(store.load_records(&uuid).is_err());
    }

    #[test]
    fn uuids_images_cant_name_never_reach_the_disk() {
        let store = Store::new(PathBuf::from("/nowhere"));
        for bad in ["", "..", "a/b", "DOCKER-x", "con"] {
            let uuid = InstanceUuid::from(bad.to_string());
            assert!(store.instance_dir(&uuid).is_err(), "{bad:?}");
            assert!(store.load_records(&uuid).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn policy_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let uuid = uuid();
        assert_eq!(store.load_policy(&uuid).unwrap(), BackupPolicy::default());
        let policy = BackupPolicy {
            enabled: true,
            keep_last: 3,
            ..BackupPolicy::default()
        };
        store.save_policy(&uuid, &policy).unwrap();
        assert_eq!(store.load_policy(&uuid).unwrap(), policy);
        let invalid = BackupPolicy {
            keep_last: 0,
            ..BackupPolicy::default()
        };
        assert!(store.save_policy(&uuid, &invalid).is_err());
        // a damaged policy falls back to the default
        fs::write(store.policy_path(&uuid).unwrap(), b"x").unwrap();
        assert_eq!(store.load_policy(&uuid).unwrap(), BackupPolicy::default());
        assert_eq!(store.instances().unwrap(), vec![uuid]);
    }
}
