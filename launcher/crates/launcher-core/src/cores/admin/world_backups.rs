//! A server's world backups (`core/src/handlers/world_backups.rs`): plain `.zip` files
//! of its world, beside its snapshots ([`super::backups`]). The list with the job
//! running and the space used, making one, deleting, restoring one into the stopped
//! server, downloading the stored zip, and cancelling a world job: what the Host's
//! Snapshots tab does. The core's rename route (`PATCH`) is left to API clients, as the
//! Host doesn't rename snapshots either.
//!
//! Making and restoring run in the background, as snapshots do: the request returns the
//! job's `task_id`, and the progression's end (`WorldBackupCompleted`,
//! `WorldRestoreCompleted`) says how it went. World jobs share the server's job lock
//! with snapshots, so either list's `running` shows whichever runs.
//!
//! Values a newer core adds read as `Unknown`, and missing fields as their defaults, so
//! an older Host keeps working.

use std::path::Path;

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::backups::{MAX_BACKUP_DOWNLOAD_BYTES, RunningJob};
use super::{AdminClient, events::snowflake, segment};
use crate::cores::{CoreError, download::save_via_partial};
use crate::game::download::{CancelToken, Progress};

/// How a world backup was made (`WorldBackupKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldBackupKind {
    /// Someone pressed "Back up world".
    #[default]
    Manual,
    /// The safety backup a restore takes of the world it replaces.
    BeforeRestore,
    #[serde(other)]
    Unknown,
}

/// One world backup, as the list shows it (`WorldBackupInfo`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldBackupInfo {
    /// `YYYYMMDD-HHMMSS-xxxx`, in UTC.
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: WorldBackupKind,
    /// Unix seconds.
    #[serde(default)]
    pub created: i64,
    /// The world's folder in the zip.
    #[serde(default)]
    pub level: String,
    /// The zip's size.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub file_count: u64,
    /// Links and files that couldn't be read, left out.
    #[serde(default)]
    pub left_out_count: u64,
    #[serde(default)]
    pub game_version: String,
    /// Made while the server ran.
    #[serde(default)]
    pub live: bool,
    /// The server said it saved the world before the copy (always true when not live).
    #[serde(default = "yes")]
    pub quiesced: bool,
    /// For a safety backup, what the restore it was taken before put back.
    #[serde(default)]
    pub before_restoring: Option<String>,
}

impl WorldBackupInfo {
    /// Made while the server ran without it confirming it saved: the world may be caught
    /// mid-write.
    pub fn may_be_inconsistent(&self) -> bool {
        self.live && !self.quiesced
    }
}

/// The space the server's world backups use (`WorldBackupUsage`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldBackupUsage {
    /// The zips' sizes added up.
    pub total_bytes: u64,
    /// Free space on the disk the world backups are on.
    pub disk_free: Option<u64>,
}

/// `GET /instance/{uuid}/world-backups` (`WorldBackupList`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldBackupList {
    /// Newest first.
    pub backups: Vec<WorldBackupInfo>,
    /// The server's job now, whichever it is: a world backup or restore, or a snapshot
    /// or its restore.
    pub running: Option<RunningJob>,
    pub usage: WorldBackupUsage,
}

/// A link to a stored world backup: `GET /file/{key}` gets it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldBackupDownload {
    pub key: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub size: u64,
}

/// The reply of a request that starts a job.
#[derive(Deserialize)]
struct Started {
    #[serde(deserialize_with = "snowflake")]
    task_id: i64,
}

fn yes() -> bool {
    true
}

fn world_backups(uuid: &str) -> String {
    format!("instance/{}/world-backups", segment(uuid))
}

fn world_backup(uuid: &str, id: &str) -> String {
    format!("{}/{}", world_backups(uuid), segment(id))
}

impl AdminClient {
    /// `GET /instance/{uuid}/world-backups`: the world backups, newest first, with the
    /// job running and the space used.
    pub async fn list_world_backups(&self, uuid: &str) -> Result<WorldBackupList, CoreError> {
        self.get(&world_backups(uuid)).await
    }

    /// `POST /instance/{uuid}/world-backups`: backs the world up now, optionally named.
    /// Returns the job's task id; `WorldBackupCompleted` says how it went.
    pub async fn create_world_backup(
        &self,
        uuid: &str,
        name: Option<&str>,
    ) -> Result<i64, CoreError> {
        let body = match name {
            Some(name) => serde_json::json!({ "name": name }),
            None => serde_json::json!({}),
        };
        let started: Started = self
            .json_slow(Method::POST, &world_backups(uuid), &body)
            .await?;
        Ok(started.task_id)
    }

    /// `DELETE /instance/{uuid}/world-backups/{id}`.
    pub async fn delete_world_backup(&self, uuid: &str, id: &str) -> Result<(), CoreError> {
        self.call(Method::DELETE, &world_backup(uuid, id), None::<()>)
            .await
    }

    /// `POST /instance/{uuid}/world-backups/{id}/restore`: puts the backup's world in
    /// place of the stopped server's, after a safety backup of the world as it is. Returns
    /// once the core's checks pass, with the job's task id; `WorldRestoreCompleted` says
    /// how it went.
    pub async fn restore_world_backup(&self, uuid: &str, id: &str) -> Result<i64, CoreError> {
        let path = format!("{}/restore", world_backup(uuid, id));
        let started: Started = self
            .json_slow(Method::POST, &path, &serde_json::json!({}))
            .await?;
        Ok(started.task_id)
    }

    /// `POST /instance/{uuid}/world-backups/tasks/{task_id}/cancel`: stops a world backup
    /// or restore (a restore only until it puts the world in place: then a conflict).
    pub async fn cancel_world_backup_task(
        &self,
        uuid: &str,
        task_id: i64,
    ) -> Result<(), CoreError> {
        let path = format!(
            "{}/tasks/{}/cancel",
            world_backups(uuid),
            segment(&task_id.to_string())
        );
        self.call(Method::POST, &path, None::<()>).await
    }

    /// `POST /instance/{uuid}/world-backups/{id}/download`: a key for `GET /file/{key}`,
    /// which serves the stored zip itself (nothing is written for it).
    pub async fn world_backup_download_key(
        &self,
        uuid: &str,
        id: &str,
    ) -> Result<WorldBackupDownload, CoreError> {
        let path = format!("{}/download", world_backup(uuid, id));
        self.json(Method::POST, &path, Some(serde_json::json!({})))
            .await
    }

    /// Downloads the world backup into `out`; the bytes saved. The zip is written next to
    /// `out` and only replaces it once complete, so a failed, cancelled or dropped
    /// download leaves a file already there as it was. The link is let go of once the
    /// download ends, however it ends.
    pub async fn download_world_backup_to(
        &self,
        uuid: &str,
        id: &str,
        out: &Path,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<u64, CoreError> {
        let download = self.world_backup_download_key(uuid, id).await?;
        let release = super::images::ReleaseOnDrop::new(self, &download.key);
        let saved = save_via_partial(out, |partial| async move {
            self.core()
                .download_file(
                    &download.key,
                    &partial,
                    MAX_BACKUP_DOWNLOAD_BYTES,
                    progress,
                    cancel,
                )
                .await
        })
        .await;
        release.now().await;
        saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cores::admin::backups::JobKind;

    /// The core's own sample, from `core/src/backups/world/types.rs`'s
    /// `the_world_list_is_the_json_the_host_reads`: change both together.
    #[test]
    fn reads_the_cores_json() {
        let list: WorldBackupList = serde_json::from_str(
            r#"{
              "backups": [
                {"id": "20261112-101500-ab12", "name": "Before the raid",
                 "kind": "before_restore", "created": 1762942500, "level": "world",
                 "size": 5000000, "file_count": 1200, "left_out_count": 2,
                 "game_version": "1.21.4", "live": true, "quiesced": false,
                 "before_restoring": "Spawn built"}
              ],
              "running": {"kind": "world_backup", "task_id": "7380000000000000001",
                "started": 1762942600, "progress": 42.5, "message": "Copying the world"},
              "usage": {"total_bytes": 10000000, "disk_free": 50000000000}
            }"#,
        )
        .unwrap();
        assert_eq!(
            list,
            WorldBackupList {
                backups: vec![WorldBackupInfo {
                    id: "20261112-101500-ab12".into(),
                    name: Some("Before the raid".into()),
                    kind: WorldBackupKind::BeforeRestore,
                    created: 1_762_942_500,
                    level: "world".into(),
                    size: 5_000_000,
                    file_count: 1200,
                    left_out_count: 2,
                    game_version: "1.21.4".into(),
                    live: true,
                    quiesced: false,
                    before_restoring: Some("Spawn built".into()),
                }],
                running: Some(RunningJob {
                    kind: JobKind::WorldBackup,
                    task_id: 7_380_000_000_000_000_001,
                    started: 1_762_942_600,
                    progress: 42.5,
                    message: "Copying the world".into(),
                }),
                usage: WorldBackupUsage {
                    total_bytes: 10_000_000,
                    disk_free: Some(50_000_000_000),
                },
            }
        );
        assert!(list.backups[0].may_be_inconsistent());
    }

    #[test]
    fn reads_values_this_build_doesnt_know() {
        let list: WorldBackupList = serde_json::from_str(
            r#"{"backups": [{"id": "20261112-101500-ab12", "kind": "imported",
                "something_new": 1}], "running": {"kind": "verify", "task_id": "1"}}"#,
        )
        .unwrap();
        let backup = &list.backups[0];
        assert_eq!(backup.kind, WorldBackupKind::Unknown);
        assert!(backup.quiesced && !backup.may_be_inconsistent());
        assert_eq!(list.running.unwrap().kind, JobKind::Unknown);
        let empty: WorldBackupList = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, WorldBackupList::default());
    }

    #[test]
    fn paths_escape_their_parts() {
        assert_eq!(world_backups("a b"), "instance/a%20b/world-backups");
        assert_eq!(
            world_backup("i1", "../x"),
            format!("instance/i1/world-backups/{}", segment("../x"))
        );
        assert!(!world_backup("i1", "../x").contains("/../"));
    }
}
