//! The backups' stored and API types.
//!
//! [`Manifest`], [`FileEntry`] and [`SkippedFile`] are only stored (one manifest per
//! backup, in `backups/servers/<uuid>/<id>.json`); the rest is what the HTTP API sends and
//! receives, exported to TypeScript.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::InstanceUuid;

use super::pattern;

/// The manifest schema this version writes and reads. Manifests with a newer schema are
/// refused (and garbage collection deletes nothing while one exists).
pub const MANIFEST_SCHEMA: u32 = 1;

/// A backup's names are at most this many characters.
pub const MAX_NAME_LEN: usize = 100;

/// One backup: every file it holds, by content hash, so it restores on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    /// Must equal the file name (without `.json`).
    pub id: String,
    /// Must equal the folder the manifest is in.
    pub instance_uuid: InstanceUuid,
    pub instance_name: String,
    pub game_version: String,
    pub flavour: String,
    /// Given by the user; at most [`MAX_NAME_LEN`] characters.
    pub name: Option<String>,
    pub kind: BackupKind,
    /// Unix seconds.
    pub created: i64,
    /// Unix nanoseconds when the walk began. A later backup doesn't trust this manifest's
    /// hash of a file modified within 2 s before it (the file may have changed again in the
    /// same timestamp tick).
    pub scan_started_ns: i128,
    pub pinned: bool,
    /// Taken while the server ran.
    pub live: bool,
    /// For a live backup, whether the server confirmed it had saved and kept running while
    /// its files were copied (false when the wait for "Saved the game" timed out, or the
    /// server stopped or restarted during the copy: the copy may be inconsistent). True when
    /// not live.
    pub quiesced: bool,
    /// `level-name` from server.properties when it is a valid world folder.
    pub level_name: Option<String>,
    /// Every file was read and hashed (no stat cache).
    pub full_scan: bool,
    /// Sorted by path.
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub skipped: Vec<SkippedFile>,
    pub total_bytes: u64,
    /// Uncompressed bytes of the objects no earlier backup of this server used.
    pub new_bytes: u64,
    /// Compressed size of those objects.
    pub stored_bytes: u64,
    pub duration_ms: u64,
    pub core_version: String,
}

/// One file of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative to the server folder, `/`-separated.
    pub path: String,
    pub size: u64,
    /// SHA-256 of the contents, 64 lowercase hex characters; the object's name.
    pub sha256: String,
    /// Unix nanoseconds, full precision.
    pub modified_ns: i128,
    /// The owner's execute bit (unix); always false on Windows.
    #[serde(default)]
    pub executable: bool,
}

/// A file a backup left out or couldn't read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: SkipReason,
    /// The previous backup's copy of the file was kept in this one.
    #[serde(default)]
    pub carried_over: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// A symbolic link or junction: never followed.
    Symlink,
    /// Couldn't be opened or read (locked, in use, permission denied, not a regular file).
    Unreadable,
    /// Changed while being read, every try.
    KeptChanging,
    /// The name can't be stored (not UTF-8, or has `\`, `:` or control characters).
    BadName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BackupKind {
    Manual,
    Scheduled,
    /// The safety backup a restore takes first.
    BeforeRestore,
}

/// A server's schedule, retention and exclusions, in `backups/servers/<uuid>/policy.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct BackupPolicy {
    pub enabled: bool,
    /// 15..=10080 (a week).
    pub interval_minutes: u32,
    /// Only back up when players were on since the last backup.
    pub only_when_active: bool,
    /// The newest scheduled backups to keep; 1..=1000.
    pub keep_last: u32,
    /// The newest backup of each of this many recent hours; 0..=1000.
    pub keep_hourly: u32,
    pub keep_daily: u32,
    pub keep_weekly: u32,
    /// Gitignore-like patterns, see [`super::pattern`]; at most 100 of 256 characters.
    pub exclude: Vec<String>,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        BackupPolicy {
            enabled: false,
            interval_minutes: 360,
            only_when_active: true,
            keep_last: 6,
            keep_hourly: 24,
            keep_daily: 7,
            keep_weekly: 4,
            exclude: pattern::default_patterns(),
        }
    }
}

impl BackupPolicy {
    /// Checks the bounds; the error names the field, for a 400 response.
    pub fn validate(&self) -> Result<(), String> {
        fn within(field: &str, value: u32, min: u32, max: u32) -> Result<(), String> {
            if value < min || value > max {
                return Err(format!("{field} must be between {min} and {max}"));
            }
            Ok(())
        }
        within("interval_minutes", self.interval_minutes, 15, 10080)?;
        within("keep_last", self.keep_last, 1, 1000)?;
        within("keep_hourly", self.keep_hourly, 0, 1000)?;
        within("keep_daily", self.keep_daily, 0, 1000)?;
        within("keep_weekly", self.keep_weekly, 0, 1000)?;
        if self.exclude.len() > pattern::MAX_PATTERNS {
            return Err(format!(
                "exclude can have at most {} patterns",
                pattern::MAX_PATTERNS
            ));
        }
        for p in &self.exclude {
            if let Some(error) = pattern::pattern_error(p) {
                return Err(format!("exclude: {error}"));
            }
        }
        Ok(())
    }
}

/// A backup in the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupInfo {
    pub id: String,
    pub name: Option<String>,
    pub kind: BackupKind,
    #[ts(type = "number")]
    pub created: i64,
    pub pinned: bool,
    pub live: bool,
    pub quiesced: bool,
    #[ts(type = "number")]
    pub file_count: u64,
    #[ts(type = "number")]
    pub skipped_count: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number")]
    pub new_bytes: u64,
    #[ts(type = "number")]
    pub stored_bytes: u64,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub game_version: String,
    /// The backup knows its world folder, so "only the world" can be restored.
    pub has_world: bool,
}

impl From<&Manifest> for BackupInfo {
    fn from(m: &Manifest) -> Self {
        BackupInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            kind: m.kind,
            created: m.created,
            pinned: m.pinned,
            live: m.live,
            quiesced: m.quiesced,
            file_count: m.files.len() as u64,
            skipped_count: m.skipped.len() as u64,
            total_bytes: m.total_bytes,
            new_bytes: m.new_bytes,
            stored_bytes: m.stored_bytes,
            duration_ms: m.duration_ms,
            game_version: m.game_version.clone(),
            has_world: m.level_name.is_some(),
        }
    }
}

/// `GET /instance/{uuid}/backups`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupList {
    /// Newest first.
    pub backups: Vec<BackupInfo>,
    pub policy: BackupPolicy,
    pub running: Option<RunningJob>,
    pub usage: BackupUsage,
    /// Unix seconds of the next scheduled backup, when the schedule is on.
    #[ts(type = "number | null")]
    pub next_scheduled: Option<i64>,
}

/// The backup or restore of a server that is running now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RunningJob {
    pub kind: JobKind,
    /// The progression event's id.
    pub task_id: String,
    /// Unix seconds.
    #[ts(type = "number")]
    pub started: i64,
    /// 0..=100.
    pub progress: f64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JobKind {
    Backup,
    Restore,
}

/// How much space a server's backups take. Only counts this server's own backups, so it
/// says nothing about other servers' files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupUsage {
    /// The sum of the backups' `total_bytes`.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// The compressed size of the objects this server's backups use.
    #[ts(type = "number")]
    pub stored_bytes: u64,
    /// Free space on the disk that holds the backups.
    #[ts(type = "number | null")]
    pub disk_free: Option<u64>,
}

fn yes() -> bool {
    true
}

/// `POST /instance/{uuid}/backups/{id}/restore`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RestoreRequest {
    pub scope: RestoreScope,
    /// Stop the server first if it runs (otherwise a running server refuses the restore).
    #[serde(default = "yes")]
    pub stop: bool,
    /// Start it again afterwards if this restore stopped it.
    #[serde(default = "yes")]
    pub start_after: bool,
    /// Back up the server as it is now first, so the restore can be undone.
    #[serde(default = "yes")]
    pub safety_backup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RestoreScope {
    /// The whole server folder, except what is never backed up or the policy excludes.
    Everything,
    /// The world folder and its `_nether` and `_the_end` siblings.
    World,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_valid() {
        BackupPolicy::default().validate().unwrap();
    }

    #[test]
    fn validate_names_the_field() {
        let check = |f: fn(&mut BackupPolicy), field: &str| {
            let mut policy = BackupPolicy::default();
            f(&mut policy);
            let error = policy.validate().unwrap_err();
            assert!(error.starts_with(field), "{error}");
        };
        check(|p| p.interval_minutes = 14, "interval_minutes");
        check(|p| p.interval_minutes = 10081, "interval_minutes");
        check(|p| p.keep_last = 0, "keep_last");
        check(|p| p.keep_hourly = 1001, "keep_hourly");
        check(|p| p.keep_daily = 1001, "keep_daily");
        check(|p| p.keep_weekly = 1001, "keep_weekly");
        check(|p| p.exclude = vec!["x".to_string(); 101], "exclude");
        check(|p| p.exclude = vec!["a\\b".to_string()], "exclude");
        let policy = BackupPolicy {
            keep_hourly: 0,
            keep_daily: 0,
            keep_weekly: 0,
            interval_minutes: 15,
            ..BackupPolicy::default()
        };
        policy.validate().unwrap();
    }

    #[test]
    fn policy_fills_missing_fields() {
        let policy: BackupPolicy = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert!(policy.enabled);
        assert_eq!(policy.keep_last, 6);
        assert_eq!(policy.exclude, pattern::default_patterns());
    }

    #[test]
    fn restore_request_defaults() {
        let request: RestoreRequest = serde_json::from_str(r#"{"scope": "world"}"#).unwrap();
        assert_eq!(request.scope, RestoreScope::World);
        assert!(request.stop && request.start_after && request.safety_backup);
        let request: RestoreRequest =
            serde_json::from_str(r#"{"scope": "everything", "safety_backup": false}"#).unwrap();
        assert!(!request.safety_backup);
    }

    /// The JSON the Host reads. Its client (`launcher-core`'s `cores/admin/backups.rs`)
    /// parses this same sample in `reads_the_cores_json`: change both together.
    #[test]
    fn the_list_is_the_json_the_host_reads() {
        let list = BackupList {
            backups: vec![BackupInfo {
                id: "20261112-101500-ab12".to_string(),
                name: None,
                kind: BackupKind::BeforeRestore,
                created: 1762942500,
                pinned: true,
                live: true,
                quiesced: false,
                file_count: 1200,
                skipped_count: 2,
                total_bytes: 5000000,
                new_bytes: 12000,
                stored_bytes: 4000,
                duration_ms: 900,
                game_version: "1.21.4".to_string(),
                has_world: true,
            }],
            policy: BackupPolicy {
                enabled: true,
                interval_minutes: 60,
                exclude: vec!["dynmap/web/tiles/".to_string()],
                ..BackupPolicy::default()
            },
            running: Some(RunningJob {
                kind: JobKind::Restore,
                task_id: "7380000000000000001".to_string(),
                started: 1762942600,
                progress: 42.5,
                message: "Restoring files".to_string(),
            }),
            usage: BackupUsage {
                total_bytes: 10000000,
                stored_bytes: 2000000,
                disk_free: Some(50000000000),
            },
            next_scheduled: Some(1762946100),
        };
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::json!({
              "backups": [
                {"id": "20261112-101500-ab12", "name": null, "kind": "before_restore",
                 "created": 1762942500, "pinned": true, "live": true, "quiesced": false,
                 "file_count": 1200, "skipped_count": 2, "total_bytes": 5000000,
                 "new_bytes": 12000, "stored_bytes": 4000, "duration_ms": 900,
                 "game_version": "1.21.4", "has_world": true}
              ],
              "policy": {"enabled": true, "interval_minutes": 60, "only_when_active": true,
                "keep_last": 6, "keep_hourly": 24, "keep_daily": 7, "keep_weekly": 4,
                "exclude": ["dynmap/web/tiles/"]},
              "running": {"kind": "restore", "task_id": "7380000000000000001",
                "started": 1762942600, "progress": 42.5, "message": "Restoring files"},
              "usage": {"total_bytes": 10000000, "stored_bytes": 2000000,
                "disk_free": 50000000000u64},
              "next_scheduled": 1762946100
            })
        );
    }

    /// The progression values the Host reads, see `launcher-core`'s
    /// `cores/admin/events.rs`.
    #[test]
    fn progressions_are_the_json_the_host_reads() {
        use crate::events::{ProgressionEndValue, ProgressionStartValue};
        let uuid = InstanceUuid::from("i1".to_string());
        assert_eq!(
            serde_json::to_value(ProgressionStartValue::Restore {
                instance_uuid: uuid.clone()
            })
            .unwrap(),
            serde_json::json!({"type": "Restore", "instance_uuid": "i1"})
        );
        assert_eq!(
            serde_json::to_value(ProgressionEndValue::BackupCompleted {
                instance_uuid: uuid,
                backup_id: None,
                success: false,
                message: "Not enough free disk space".to_string(),
            })
            .unwrap(),
            serde_json::json!({"type": "BackupCompleted", "instance_uuid": "i1",
                "backup_id": null, "success": false, "message": "Not enough free disk space"})
        );
    }

    #[test]
    fn kinds_serialize_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&BackupKind::BeforeRestore).unwrap(),
            r#""before_restore""#
        );
        assert_eq!(
            serde_json::to_string(&SkipReason::KeptChanging).unwrap(),
            r#""kept_changing""#
        );
        assert_eq!(
            serde_json::to_string(&JobKind::Restore).unwrap(),
            r#""restore""#
        );
    }
}
