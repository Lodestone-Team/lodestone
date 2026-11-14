//! A server's backups (`core/src/handlers/backups.rs`): the list with the schedule,
//! the job running and the storage used, taking one, naming and pinning, deleting,
//! restoring (in place or as a new server), downloading as a zip, and the schedule.
//!
//! A backup is a full copy of the server whose files are stored once, by content, so
//! each one restores on its own. Taking and restoring run in the background: the
//! request returns the job's `task_id` (its progression's event id), and the
//! progression's end (`BackupCompleted`, `RestoreCompleted`, or `InstanceCreation` for
//! a restore as a new server) says how it went.
//!
//! Values a newer core adds read as `Unknown`, and missing fields as their defaults,
//! so an older Host keeps working.

use std::{path::Path, time::Duration};

use http_client::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use super::{AdminClient, events::snowflake, segment};
use crate::cores::{CoreError, client::MAX_JSON_BYTES, download::save_via_partial};
use crate::game::download::{CancelToken, Progress};

/// How long a request that starts a job, or zips a backup for download, may take.
/// The core answers a job's start once its checks pass, but zipping a large server
/// takes minutes.
pub const BACKUP_JOB_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// The largest backup zip saved.
pub const MAX_BACKUP_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024 * 1024;

/// What made a backup (`BackupKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// Someone pressed "Back up now".
    #[default]
    Manual,
    /// The schedule.
    Scheduled,
    /// The safety backup taken before a restore, so the restore can be undone.
    BeforeRestore,
    #[serde(other)]
    Unknown,
}

/// One backup, as the list shows it (`BackupInfo`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupInfo {
    /// `YYYYMMDD-HHMMSS-xxxx`, in UTC.
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: BackupKind,
    /// Unix seconds.
    #[serde(default)]
    pub created: i64,
    /// Kept until unpinned: the schedule never deletes it.
    #[serde(default)]
    pub pinned: bool,
    /// Taken while the server ran.
    #[serde(default)]
    pub live: bool,
    /// The server said it saved the world before the copy (always true when not live).
    #[serde(default = "yes")]
    pub quiesced: bool,
    #[serde(default)]
    pub file_count: u64,
    /// Files that couldn't be read (in use, links, bad names).
    #[serde(default)]
    pub skipped_count: u64,
    /// The size of the server's files in the backup.
    #[serde(default)]
    pub total_bytes: u64,
    /// The files no earlier backup of this server had.
    #[serde(default)]
    pub new_bytes: u64,
    /// What those took on disk, compressed.
    #[serde(default)]
    pub stored_bytes: u64,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub game_version: String,
    /// It has the world folder, so "Only the world" can be restored.
    #[serde(default)]
    pub has_world: bool,
}

impl BackupInfo {
    /// Taken while the server ran without it confirming it saved: the world may be
    /// caught mid-write.
    pub fn may_be_inconsistent(&self) -> bool {
        self.live && !self.quiesced
    }
}

/// When and how often the server is backed up, and which scheduled backups are kept
/// (`BackupPolicy`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupPolicy {
    pub enabled: bool,
    /// 15 to 10080.
    pub interval_minutes: u32,
    /// Only after players were on since the last backup.
    pub only_when_active: bool,
    /// The newest scheduled backups kept, then the newest of each recent hour, day
    /// and ISO week.
    pub keep_last: u32,
    pub keep_hourly: u32,
    pub keep_daily: u32,
    pub keep_weekly: u32,
    /// Paths left out, gitignore-like.
    pub exclude: Vec<String>,
    /// Fields a newer core has that this build doesn't know, sent back as they came,
    /// so saving the schedule never resets them.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Default for BackupPolicy {
    /// The core's defaults for a server that has no policy yet.
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 360,
            only_when_active: true,
            keep_last: 6,
            keep_hourly: 24,
            keep_daily: 7,
            keep_weekly: 4,
            exclude: [
                "dynmap/web/tiles/",
                "plugins/dynmap/web/tiles/",
                "bluemap/web/maps/",
                "plugins/BlueMap/web/maps/",
                "squaremap/web/tiles/",
            ]
            .map(String::from)
            .to_vec(),
            other: Map::new(),
        }
    }
}

/// What the running job does (`JobKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    #[default]
    Backup,
    Restore,
    #[serde(other)]
    Unknown,
}

/// The backup or restore of the server that is running (`RunningJob`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunningJob {
    #[serde(default)]
    pub kind: JobKind,
    /// Its progression's event id.
    #[serde(deserialize_with = "snowflake")]
    pub task_id: i64,
    /// Unix seconds.
    #[serde(default)]
    pub started: i64,
    /// 0 to 100.
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub message: String,
}

/// The storage the server's backups use (`BackupUsage`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupUsage {
    /// The backups' sizes added up, as if each were a full copy.
    pub total_bytes: u64,
    /// What they take on disk: each file once, compressed.
    pub stored_bytes: u64,
    /// Free space on the disk the backups are on.
    pub disk_free: Option<u64>,
}

/// `GET /instance/{uuid}/backups` (`BackupList`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupList {
    /// Newest first.
    pub backups: Vec<BackupInfo>,
    pub policy: BackupPolicy,
    pub running: Option<RunningJob>,
    pub usage: BackupUsage,
    /// When the schedule backs up next, in unix seconds.
    pub next_scheduled: Option<i64>,
}

/// What a restore in place replaces (`RestoreScope`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreScope {
    /// The whole server folder (its name, port and Java command stay).
    #[default]
    Everything,
    /// The world, its Nether and its End.
    World,
}

/// `POST /instance/{uuid}/backups/{id}/restore` (`RestoreRequest`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreRequest {
    pub scope: RestoreScope,
    /// Stop the server first if it runs (the core refuses to restore over a running
    /// server otherwise).
    #[serde(default = "yes")]
    pub stop: bool,
    /// Start it again afterwards, if the restore stopped it.
    #[serde(default = "yes")]
    pub start_after: bool,
    /// Back up the server as it is first, so the restore can be undone.
    #[serde(default = "yes")]
    pub safety_backup: bool,
}

impl Default for RestoreRequest {
    fn default() -> Self {
        Self {
            scope: RestoreScope::Everything,
            stop: true,
            start_after: true,
            safety_backup: true,
        }
    }
}

/// A backup zipped for download: `GET /file/{key}` gets it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDownload {
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

fn backups(uuid: &str) -> String {
    format!("instance/{}/backups", segment(uuid))
}

fn backup(uuid: &str, id: &str) -> String {
    format!("{}/{}", backups(uuid), segment(id))
}

/// The body of `PATCH /instance/{uuid}/backups/{id}`: only what changes. A name of
/// `Some(None)` removes the name.
fn update_body(name: Option<Option<&str>>, pinned: Option<bool>) -> Value {
    let mut body = Map::new();
    if let Some(name) = name {
        body.insert("name".into(), name.map_or(Value::Null, Value::from));
    }
    if let Some(pinned) = pinned {
        body.insert("pinned".into(), Value::Bool(pinned));
    }
    Value::Object(body)
}

impl AdminClient {
    /// A JSON request with [`BACKUP_JOB_TIMEOUT`].
    async fn json_slow<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, CoreError> {
        let body = serde_json::to_vec(body).map_err(|err| CoreError::Invalid(err.to_string()))?;
        let bytes = self
            .send_slow(
                method,
                path,
                Some((body, "application/json".into())),
                MAX_JSON_BYTES,
                BACKUP_JOB_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `GET /instance/{uuid}/backups`: the backups, newest first, with the schedule,
    /// the job running and the storage used.
    pub async fn list_backups(&self, uuid: &str) -> Result<BackupList, CoreError> {
        self.get(&backups(uuid)).await
    }

    /// `POST /instance/{uuid}/backups`: backs the server up now, optionally named.
    /// Returns the job's task id; `BackupCompleted` says how it went.
    pub async fn create_backup(&self, uuid: &str, name: Option<&str>) -> Result<i64, CoreError> {
        let body = match name {
            Some(name) => serde_json::json!({ "name": name }),
            None => serde_json::json!({}),
        };
        let started: Started = self.json_slow(Method::POST, &backups(uuid), &body).await?;
        Ok(started.task_id)
    }

    /// `PATCH /instance/{uuid}/backups/{id}`: renames (`Some(None)` removes the name)
    /// and pins or unpins. Fields left `None` don't change.
    pub async fn update_backup(
        &self,
        uuid: &str,
        id: &str,
        name: Option<Option<&str>>,
        pinned: Option<bool>,
    ) -> Result<BackupInfo, CoreError> {
        self.json(
            Method::PATCH,
            &backup(uuid, id),
            Some(update_body(name, pinned)),
        )
        .await
    }

    /// `DELETE /instance/{uuid}/backups/{id}`.
    pub async fn delete_backup(&self, uuid: &str, id: &str) -> Result<(), CoreError> {
        self.call(Method::DELETE, &backup(uuid, id), None::<()>)
            .await
    }

    /// `POST /instance/{uuid}/backups/{id}/restore`: restores the backup over the
    /// server. Returns once the core's checks pass, with the job's task id;
    /// `RestoreCompleted` says how it went.
    pub async fn restore_backup(
        &self,
        uuid: &str,
        id: &str,
        request: &RestoreRequest,
    ) -> Result<i64, CoreError> {
        let path = format!("{}/restore", backup(uuid, id));
        let started: Started = self.json_slow(Method::POST, &path, request).await?;
        Ok(started.task_id)
    }

    /// `POST /instance/{uuid}/backups/{id}/restore_as_new`: makes a new server from
    /// the backup (named `<name> (restored)` by default). The new server comes as an
    /// `InstanceCreation` progression.
    pub async fn restore_backup_as_new(
        &self,
        uuid: &str,
        id: &str,
        name: Option<&str>,
    ) -> Result<i64, CoreError> {
        let path = format!("{}/restore_as_new", backup(uuid, id));
        let body = serde_json::json!({ "name": name });
        let started: Started = self.json_slow(Method::POST, &path, &body).await?;
        Ok(started.task_id)
    }

    /// `POST /instance/{uuid}/backups/{id}/download`: zips the backup and returns a
    /// key for `GET /file/{key}`. Zipping a large server takes minutes.
    pub async fn backup_download_key(
        &self,
        uuid: &str,
        id: &str,
    ) -> Result<BackupDownload, CoreError> {
        let path = format!("{}/download", backup(uuid, id));
        self.json_slow(Method::POST, &path, &serde_json::json!({}))
            .await
    }

    /// Downloads the backup as a zip into `out`; the bytes saved. The zip is written
    /// next to `out` and only replaces it once complete, so a failed or dropped
    /// download leaves a file already there as it was, and no partial zip.
    pub async fn download_backup_to(
        &self,
        uuid: &str,
        id: &str,
        out: &Path,
    ) -> Result<u64, CoreError> {
        let download = self.backup_download_key(uuid, id).await?;
        save_via_partial(out, |partial| async move {
            self.core()
                .download_file(
                    &download.key,
                    &partial,
                    MAX_BACKUP_DOWNLOAD_BYTES,
                    &Progress::default(),
                    &CancelToken::new(),
                )
                .await
        })
        .await
    }

    /// `GET /instance/{uuid}/backups/policy`.
    pub async fn backup_policy(&self, uuid: &str) -> Result<BackupPolicy, CoreError> {
        self.get(&format!("{}/policy", backups(uuid))).await
    }

    /// `PUT /instance/{uuid}/backups/policy`: the policy as the core saved it.
    pub async fn set_backup_policy(
        &self,
        uuid: &str,
        policy: &BackupPolicy,
    ) -> Result<BackupPolicy, CoreError> {
        self.json(
            Method::PUT,
            &format!("{}/policy", backups(uuid)),
            Some(policy),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_list_with_values_this_build_doesnt_know() {
        let list: BackupList = serde_json::from_str(
            r#"{
              "backups": [
                {"id":"20261112-101500-ab12","name":null,"kind":"scheduled","created":1762942500,
                 "pinned":false,"live":true,"quiesced":false,"file_count":1200,"skipped_count":2,
                 "total_bytes":5000000,"new_bytes":12000,"stored_bytes":4000,"duration_ms":900,
                 "game_version":"1.21.4","has_world":true},
                {"id":"20261111-090000-cd34","kind":"imported_from_elsewhere","created":1762851600,
                 "something_new":[1,2]}
              ],
              "policy":{"enabled":true,"interval_minutes":60,"only_when_active":false,
                "keep_last":6,"keep_hourly":24,"keep_daily":7,"keep_weekly":4,"exclude":[],
                "keep_monthly":3},
              "running":{"kind":"verify","task_id":"7380000000000000001","started":1762942600,
                "progress":42.5,"message":"Storing files"},
              "usage":{"total_bytes":10000000,"stored_bytes":2000000,"disk_free":null},
              "next_scheduled":1762946100
            }"#,
        )
        .unwrap();
        assert_eq!(list.backups.len(), 2);
        let first = &list.backups[0];
        assert_eq!(first.kind, BackupKind::Scheduled);
        assert!(first.may_be_inconsistent());
        let second = &list.backups[1];
        assert_eq!(second.kind, BackupKind::Unknown);
        assert!(second.quiesced && !second.may_be_inconsistent());
        assert!(!second.has_world);
        let running = list.running.as_ref().unwrap();
        assert_eq!(running.kind, JobKind::Unknown);
        assert_eq!(running.task_id, 7_380_000_000_000_000_001);
        assert_eq!(list.usage.disk_free, None);
        assert_eq!(list.next_scheduled, Some(1_762_946_100));
        // a field from a newer core goes back as it came
        let saved = serde_json::to_value(&list.policy).unwrap();
        assert_eq!(saved["keep_monthly"], 3);
        assert_eq!(saved["interval_minutes"], 60);
    }

    /// The core's own sample, from `core/src/backups/types.rs`'s
    /// `the_list_is_the_json_the_host_reads`: change both together.
    #[test]
    fn reads_the_cores_json() {
        let list: BackupList = serde_json::from_str(
            r#"{
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
                "disk_free": 50000000000},
              "next_scheduled": 1762946100
            }"#,
        )
        .unwrap();
        assert_eq!(
            list,
            BackupList {
                backups: vec![BackupInfo {
                    id: "20261112-101500-ab12".into(),
                    name: None,
                    kind: BackupKind::BeforeRestore,
                    created: 1_762_942_500,
                    pinned: true,
                    live: true,
                    quiesced: false,
                    file_count: 1200,
                    skipped_count: 2,
                    total_bytes: 5_000_000,
                    new_bytes: 12_000,
                    stored_bytes: 4000,
                    duration_ms: 900,
                    game_version: "1.21.4".into(),
                    has_world: true,
                }],
                policy: BackupPolicy {
                    enabled: true,
                    interval_minutes: 60,
                    exclude: vec!["dynmap/web/tiles/".into()],
                    ..BackupPolicy::default()
                },
                running: Some(RunningJob {
                    kind: JobKind::Restore,
                    task_id: 7_380_000_000_000_000_001,
                    started: 1_762_942_600,
                    progress: 42.5,
                    message: "Restoring files".into(),
                }),
                usage: BackupUsage {
                    total_bytes: 10_000_000,
                    stored_bytes: 2_000_000,
                    disk_free: Some(50_000_000_000),
                },
                next_scheduled: Some(1_762_946_100),
            }
        );
        // the policy goes back with exactly the core's fields
        let saved = serde_json::to_value(&list.policy).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({"enabled": true, "interval_minutes": 60, "only_when_active": true,
                "keep_last": 6, "keep_hourly": 24, "keep_daily": 7, "keep_weekly": 4,
                "exclude": ["dynmap/web/tiles/"]})
        );
    }

    #[test]
    fn an_empty_list_reads_as_the_defaults() {
        let list: BackupList = serde_json::from_str("{}").unwrap();
        assert!(list.backups.is_empty());
        assert_eq!(list.policy, BackupPolicy::default());
        assert!(!list.policy.enabled);
        assert_eq!(list.policy.interval_minutes, 360);
        assert!(list.running.is_none());
    }

    #[test]
    fn requests_are_the_cores_json() {
        assert_eq!(
            serde_json::to_value(RestoreRequest {
                scope: RestoreScope::World,
                stop: true,
                start_after: false,
                safety_backup: true,
            })
            .unwrap(),
            serde_json::json!({
                "scope": "world", "stop": true, "start_after": false, "safety_backup": true
            })
        );
        assert_eq!(
            serde_json::to_value(BackupKind::BeforeRestore).unwrap(),
            serde_json::json!("before_restore")
        );
        assert_eq!(
            update_body(Some(None), Some(true)),
            serde_json::json!({ "name": null, "pinned": true })
        );
        assert_eq!(
            update_body(Some(Some("Before 1.21")), None),
            serde_json::json!({ "name": "Before 1.21" })
        );
        assert_eq!(update_body(None, None), serde_json::json!({}));
        let started: Started = serde_json::from_str(r#"{"task_id":"123"}"#).unwrap();
        assert_eq!(started.task_id, 123);
        let download: BackupDownload =
            serde_json::from_str(r#"{"key":"abc","file_name":"SMP.zip","size":10}"#).unwrap();
        assert_eq!(download.file_name, "SMP.zip");
    }

    #[test]
    fn paths_are_encoded() {
        assert_eq!(backups("a b"), "instance/a%20b/backups");
        assert_eq!(
            backup("i1", "20261112-101500-ab12"),
            "instance/i1/backups/20261112-101500-ab12"
        );
    }
}
