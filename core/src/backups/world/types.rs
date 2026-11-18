//! World backups' stored and API types.
//!
//! [`WorldBackupRecord`] is only stored (the `<id>.json` beside each zip, see
//! [`super::store`]); the rest is what the HTTP API sends and receives, exported to
//! TypeScript.

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

use crate::backups::RunningJob;

/// The version of the sidecar format. A sidecar of another version is an error, never
/// read as something it isn't.
pub const WORLD_RECORD_VERSION: u32 = 1;

/// How a world backup was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum WorldBackupKind {
    /// Someone asked for it ("Back up world").
    Manual,
    /// The safety backup a restore takes of the world it replaces.
    BeforeRestore,
}

/// One world backup's sidecar, `<id>.json` beside `<id>.zip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldBackupRecord {
    /// [`WORLD_RECORD_VERSION`].
    pub version: u32,
    /// `YYYYMMDD-HHMMSS-xxxx`, the zip's file name.
    pub id: String,
    /// Given by the user; at most [`crate::backups::types::MAX_NAME_LEN`] characters.
    pub name: Option<String>,
    pub kind: WorldBackupKind,
    /// Unix seconds.
    pub created: i64,
    /// The level's folder name in the zip (`level-name` when it was made).
    pub level: String,
    /// The zip's top folders: the level, and on Paper and Spigot its `_nether` and
    /// `_the_end` folders when the server had them.
    pub folders: Vec<String>,
    /// The server's name, Minecraft version and flavour when it was made.
    pub instance_name: String,
    pub game_version: String,
    pub flavour: String,
    /// The zip's size on disk.
    pub size: u64,
    /// Files in the zip.
    pub file_count: u64,
    /// Files left out: links, files that couldn't be read, names the server's layout
    /// can't place.
    pub left_out: u64,
    /// Made while the server ran.
    pub live: bool,
    /// For a live backup, whether the server confirmed it had saved and kept running while
    /// the world was copied; true when not live.
    pub quiesced: bool,
    /// For a safety backup, what the restore it was taken before put back (its name, or
    /// its date).
    pub before_restoring: Option<String>,
}

/// A world backup in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorldBackupInfo {
    pub id: String,
    pub name: Option<String>,
    pub kind: WorldBackupKind,
    #[ts(type = "number")]
    pub created: i64,
    /// The level's folder name in the zip.
    pub level: String,
    /// The zip's size.
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number")]
    pub file_count: u64,
    /// Links, unreadable files and names the server's layout can't place, left out.
    #[ts(type = "number")]
    pub left_out_count: u64,
    pub game_version: String,
    pub live: bool,
    pub quiesced: bool,
    /// For a safety backup, what the restore it was taken before put back.
    pub before_restoring: Option<String>,
}

impl From<&WorldBackupRecord> for WorldBackupInfo {
    fn from(r: &WorldBackupRecord) -> Self {
        WorldBackupInfo {
            id: r.id.clone(),
            name: r.name.clone(),
            kind: r.kind,
            created: r.created,
            level: r.level.clone(),
            size: r.size,
            file_count: r.file_count,
            left_out_count: r.left_out,
            game_version: r.game_version.clone(),
            live: r.live,
            quiesced: r.quiesced,
            before_restoring: r.before_restoring.clone(),
        }
    }
}

/// How much space a server's world backups take.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorldBackupUsage {
    /// The sum of the zips' sizes.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// Free space on the disk that holds the world backups.
    #[ts(type = "number | null")]
    pub disk_free: Option<u64>,
}

/// `GET /instance/{uuid}/world-backups`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorldBackupList {
    /// Newest first.
    pub backups: Vec<WorldBackupInfo>,
    /// The job that holds the server's job lock now, whichever it is: a world backup or
    /// restore, or a snapshot or its restore (they share one lock).
    pub running: Option<RunningJob>,
    pub usage: WorldBackupUsage,
}

/// `POST /instance/{uuid}/world-backups`.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct CreateWorldBackupRequest {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

/// Tells a field set to `null` (`Some(None)`) from a missing one (`None`).
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

/// `PATCH /instance/{uuid}/world-backups/{id}`.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct UpdateWorldBackupRequest {
    /// A new name; `null` clears it.
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub name: Option<Option<String>>,
}

/// `POST /instance/{uuid}/world-backups/{id}/download`: a key for `GET /file/{key}`, which
/// serves the stored zip itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorldBackupDownload {
    pub key: String,
    pub file_name: String,
    #[ts(type = "number")]
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backups::JobKind;

    /// The JSON the Host reads. Its client (`launcher-core`'s `cores/admin/world_backups.rs`)
    /// parses this same sample in `reads_the_cores_json`: change both together.
    #[test]
    fn the_world_list_is_the_json_the_host_reads() {
        let list = WorldBackupList {
            backups: vec![WorldBackupInfo {
                id: "20261112-101500-ab12".to_string(),
                name: Some("Before the raid".to_string()),
                kind: WorldBackupKind::BeforeRestore,
                created: 1762942500,
                level: "world".to_string(),
                size: 5000000,
                file_count: 1200,
                left_out_count: 2,
                game_version: "1.21.4".to_string(),
                live: true,
                quiesced: false,
                before_restoring: Some("Spawn built".to_string()),
            }],
            running: Some(RunningJob {
                kind: JobKind::WorldBackup,
                task_id: "7380000000000000001".to_string(),
                started: 1762942600,
                progress: 42.5,
                message: "Copying the world".to_string(),
            }),
            usage: WorldBackupUsage {
                total_bytes: 10000000,
                disk_free: Some(50000000000),
            },
        };
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::json!({
              "backups": [
                {"id": "20261112-101500-ab12", "name": "Before the raid",
                 "kind": "before_restore", "created": 1762942500, "level": "world",
                 "size": 5000000, "file_count": 1200, "left_out_count": 2,
                 "game_version": "1.21.4", "live": true, "quiesced": false,
                 "before_restoring": "Spawn built"}
              ],
              "running": {"kind": "world_backup", "task_id": "7380000000000000001",
                "started": 1762942600, "progress": 42.5, "message": "Copying the world"},
              "usage": {"total_bytes": 10000000, "disk_free": 50000000000u64}
            })
        );
    }

    #[test]
    fn requests_are_read() {
        let create: CreateWorldBackupRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(create.name, None);
        let update: UpdateWorldBackupRequest = serde_json::from_str(r#"{"name": null}"#).unwrap();
        assert_eq!(update.name, Some(None));
        let update: UpdateWorldBackupRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(update.name, None);
    }
}
