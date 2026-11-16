//! Server backups over HTTP, see [`crate::backups`].
//!
//! | Route | Permission |
//! |---|---|
//! | `GET /instance/{uuid}/backups` | ReadInstanceFile |
//! | `POST /instance/{uuid}/backups` | AccessSetting |
//! | `PATCH /instance/{uuid}/backups/{id}` | AccessSetting; unpinning also WriteInstanceFile |
//! | `DELETE /instance/{uuid}/backups/{id}` | WriteInstanceFile |
//! | `POST /instance/{uuid}/backups/{id}/restore` | WriteInstanceFile, StopInstance and StartInstance when it stops and starts the server, and WriteGlobalFile or owner rights for what it changes. Everything and `world_and_mods` can change the server's Minecraft and loader version to the backup's (with the server software they put back), which needs no AccessSetting |
//! | `POST /instance/{uuid}/backups/{id}/restore_as_new` | CreateInstance, ReadInstanceFile, and WriteGlobalFile or owner rights for what the backup holds |
//! | `POST /instance/{uuid}/backups/{id}/download` | ReadInstanceFile |
//! | `POST /instance/{uuid}/backups/{id}/export` | ReadInstanceFile (a `.lodeimg` without the server's secrets) |
//! | `GET /instance/{uuid}/backups/policy` | ReadInstanceFile |
//! | `PUT /instance/{uuid}/backups/policy` | AccessSetting and WriteInstanceFile (a policy can prune history) |
//!
//! Every route checks the permission, then that the server exists (404 otherwise, also for
//! Docker containers, which have no backups), then the uuid's shape, before touching the
//! store.

#[cfg(test)]
mod tests;

use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::eyre;
use serde::{Deserialize, Deserializer, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::auth::user::{User, UserAction};
use crate::backups::restore::RestoreDiff;
use crate::backups::service::{DiffCheck, NewServerRequest};
use crate::backups::{BackupInfo, BackupList, BackupPolicy, RestoreRequest};
use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{new_fs_event, Event, FSOperation, FSTarget, ProgressionEventID};
use crate::images::share::inspect::NameList;
use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::{path_to_tmp, GameInstance};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State as InstanceState, TServer};
use crate::types::InstanceUuid;
use crate::AppState;

use super::global_fs::{register_download, take_backup_file_slot, DownloadScope};
use super::instance::sanitize_instance_dir_name;
use super::instance_fs::{is_instance_protected, is_protected};
use super::util::{get_instance, is_code_path, require_code_write, AuthedUser};

/// `POST /instance/{uuid}/backups`.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct CreateBackupRequest {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

/// The response of the routes that start a job: the progression event's id.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupTask {
    pub task_id: String,
}

/// Tells a field set to `null` (`Some(None)`) from a missing one (`None`).
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

/// `PATCH /instance/{uuid}/backups/{id}`.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct UpdateBackupRequest {
    /// A new name; `null` clears it.
    #[serde(default, deserialize_with = "present")]
    #[ts(optional)]
    pub name: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub pinned: Option<bool>,
}

/// `POST /instance/{uuid}/backups/{id}/restore_as_new`.
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct RestoreAsNewRequest {
    /// Defaults to "<server name> (restored)".
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

/// `POST /instance/{uuid}/backups/{id}/download`: a key for `GET /file/{key}`.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BackupDownload {
    pub key: String,
    pub file_name: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// `POST /instance/{uuid}/backups/{id}/export`: a backup as a `.lodeimg` file for another
/// Host, with its secrets taken out (`images::share::export`). The Launcher opens one only
/// from milestone 6's second part.
#[derive(Debug, Default, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ExportBackupRequest {
    /// Include everything in the plugins' folders (settings, databases, player data),
    /// which can hold passwords. Off by default: only the plugin jars travel.
    #[serde(default)]
    #[ts(optional)]
    pub include_plugin_data: Option<bool>,
}

/// `POST /instance/{uuid}/backups/{id}/export`'s answer: a key for `GET /file/{key}`, and
/// what the file leaves out, for the person who exported it to know.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExportedBackup {
    pub key: String,
    pub file_name: String,
    #[ts(type = "number")]
    pub size: u64,
    /// Files taken out (Lodestone's, the EULA, server software, crash files, plugin
    /// settings), by path.
    pub left_out: NameList,
    /// What of `left_out` a player would miss, for a Launcher making an instance of the
    /// world to show: without the server software, the EULA, the JVM's crash files, the
    /// console history, Lodestone's own files and what a player's instance never takes
    /// (the server's settings, operators and ban lists, plugins' data). A list of its own,
    /// because `left_out`'s first names are mostly `libraries/`' and plugins' files.
    /// It may name a `config/` file the owner never shared with players, which a player
    /// wouldn't get anyway: the export doesn't look up the content image's marks.
    pub left_out_for_players: NameList,
    /// Secret values made empty, as `<file>: <key>`.
    pub blanked: NameList,
    /// CurseForge files whose authors don't allow downloads elsewhere: named, not
    /// carried, so whoever opens the file won't get them.
    pub not_downloadable: NameList,
    /// Files taken out because they hold a password or token (found by their keys' names,
    /// a key or token file, or too big to check), by path; also in `left_out`. The other
    /// server writes its defaults for them.
    pub secrets_withheld: NameList,
}

/// The Minecraft server `uuid`. 404 when there is none (Docker containers included).
pub(super) fn minecraft(state: &AppState, uuid: &InstanceUuid) -> Result<MinecraftInstance, Error> {
    let GameInstance::MinecraftInstance(instance) = get_instance(state, uuid)?;
    Ok(instance)
}

pub(super) fn denied(message: &str) -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!("{message}"),
    }
}

async fn list_backups(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<BackupList>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    Ok(Json(state.backups.list(&uuid).await?))
}

async fn create_backup(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    body: Option<Json<CreateBackupRequest>>,
) -> Result<Json<BackupTask>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    let request = body.map(|Json(body)| body).unwrap_or_default();
    let task_id = state
        .backups
        .start_backup(&state, instance, request.name, requester.caused_by())
        .await?;
    Ok(Json(BackupTask { task_id }))
}

async fn update_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    Json(request): Json<UpdateBackupRequest>,
) -> Result<Json<BackupInfo>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    // an unpinned backup can be pruned, so unpinning is as strong as deleting
    if request.pinned == Some(false) {
        requester
            .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
            .await?;
    }
    minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    Ok(Json(
        state
            .backups
            .update(&uuid, &id, request.name, request.pinned)
            .await?,
    ))
}

async fn delete_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<serde_json::Value>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    state.backups.delete(&uuid, &id).await?;
    Ok(Json(serde_json::json!({})))
}

async fn get_policy(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<BackupPolicy>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    Ok(Json(state.backups.policy(&uuid).await?))
}

async fn put_policy(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(policy): Json<BackupPolicy>,
) -> Result<Json<BackupPolicy>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    Ok(Json(state.backups.set_policy(&uuid, policy).await?))
}

/// Whether a user without `WriteGlobalFile` may not have a restore add, change or remove
/// `path` (relative to the server folder): the file API's rule for a server's files,
/// Lodestone's own `.lodestone*` files included.
pub(super) fn restore_protected(path: &str, is_dir: bool) -> bool {
    is_instance_protected(FsPath::new(""), FsPath::new(path), is_dir)
}

/// The rule for what an in-place restore changes: like the file API, adding, changing or
/// removing protected files and folders (jars, the mods folder, the config, Lodestone's
/// own files) needs `WriteGlobalFile`, and adding or changing scripts needs the owner with
/// code execution on.
fn check_restore_diff(state: &AppState, user: &User, diff: &RestoreDiff) -> Result<(), Error> {
    let touches_protected = diff
        .added
        .iter()
        .chain(&diff.changed)
        .chain(&diff.removed)
        .any(|path| restore_protected(path, false))
        || diff
            .created_dirs
            .iter()
            .chain(&diff.removed_dirs)
            .any(|path| restore_protected(path, true));
    if touches_protected && can_write_global_files(state, user).is_err() {
        return Err(denied(
            "Restoring this snapshot changes protected files (such as .jar files or the mods folder), which needs permission to change global files",
        ));
    }
    if diff
        .added
        .iter()
        .chain(&diff.changed)
        .any(|path| is_code_path(FsPath::new(path), false))
    {
        require_code_write(state, user)?;
    }
    Ok(())
}

pub(super) fn can_write_global_files(state: &AppState, user: &User) -> Result<(), Error> {
    user.try_action(&UserAction::WriteGlobalFile, state.safe_mode())
}

async fn restore_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    Json(request): Json<RestoreRequest>,
) -> Result<Json<BackupTask>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    let backup = state.backups.load(&uuid, &id).await?;
    // before the server is stopped for it; the plan checks again
    crate::backups::restore::check_scope(
        request.scope,
        backup.record.level.is_some(),
        backup.content.is_some(),
    )?;
    // what this checks holds only while the server's state stays as it is now: the
    // service refuses a server that started meanwhile (`may_stop`)
    let runs = instance.state().await == InstanceState::Running;
    if runs {
        if !request.stop {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: eyre!("Stop the server before restoring a snapshot"),
            });
        }
        requester
            .require(&state, UserAction::StopInstance(uuid.clone()))
            .await?;
        if request.start_after {
            requester
                .require(&state, UserAction::StartInstance(uuid.clone()))
                .await?;
        }
    }
    // only worth comparing the files when some change could be refused
    let user = requester.0.clone();
    let needs_check = can_write_global_files(&state, &user).is_err()
        || require_code_write(&state, &user).is_err();
    let check: Option<DiffCheck> = needs_check.then(|| {
        let state = state.clone();
        Arc::new(move |diff: &RestoreDiff| check_restore_diff(&state, &user, diff)) as DiffCheck
    });
    let task_id = state
        .backups
        .start_restore(
            &state,
            instance,
            backup,
            request,
            runs,
            check,
            requester.caused_by(),
        )
        .await?;
    Ok(Json(BackupTask { task_id }))
}

async fn restore_as_new(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    body: Option<Json<RestoreAsNewRequest>>,
) -> Result<Json<BackupTask>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    let backup = state.backups.load(&uuid, &id).await?;
    let files = backup.files_on_disk()?;
    let user = &requester.0;
    // what the new server gets, the requester could otherwise only put there with these
    let protected = files.iter().any(|file| {
        let path = FsPath::new(&file.disk);
        restore_protected(&file.disk, false)
            || path
                .ancestors()
                .skip(1)
                .filter(|dir| !dir.as_os_str().is_empty())
                .any(|dir| is_protected(dir, true))
    });
    if protected && can_write_global_files(&state, user).is_err() {
        return Err(denied(
            "This snapshot has protected files (such as .jar files or the mods folder), so restoring it as a new server needs permission to change global files",
        ));
    }
    if files
        .iter()
        .any(|file| is_code_path(FsPath::new(&file.disk), false))
    {
        require_code_write(&state, user)?;
    }
    let name = match body
        .and_then(|Json(body)| body.name)
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
    {
        Some(name) => name,
        None => {
            // a backup listed again after its record was lost has no server name
            let mut base = backup.record.instance_name.trim().to_string();
            if base.is_empty() {
                base = instance.name().await;
            }
            let base: String = base.chars().take(89).collect();
            format!("{} (restored)", base.trim())
        }
    };
    let keep_command_line =
        can_write_global_files(&state, user).is_ok() && state.code_execution_enabled();
    let task_id = state
        .backups
        .start_restore_as_new(
            &state,
            backup,
            NewServerRequest {
                name,
                keep_command_line,
                requester_uid: user.uid.clone(),
                caused_by: requester.caused_by(),
            },
        )
        .await?;
    Ok(Json(BackupTask { task_id }))
}

/// A backup being written to a file for download (a zip, or a `.lodeimg` export). If the
/// request is dropped first (the client went away, or gave up waiting), dropping this ends
/// the progression, which would otherwise show forever, and stops the work.
struct Preparing {
    broadcaster: EventBroadcaster,
    /// `None` once the End was sent.
    event_id: Option<ProgressionEventID>,
    cancel: CancellationToken,
}

impl Preparing {
    fn end(mut self, success: bool, message: &str) {
        if let Some(event_id) = self.event_id.take() {
            self.broadcaster.send(Event::new_progression_event_end(
                event_id,
                success,
                Some(message),
                None,
            ));
        }
    }
}

impl Drop for Preparing {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(event_id) = self.event_id.take() {
            self.broadcaster.send(Event::new_progression_event_end(
                event_id,
                false,
                Some("The download was cancelled"),
                None,
            ));
        }
    }
}

async fn download_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<BackupDownload>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    let name = instance.name().await;
    if !crate::backups::store::valid_backup_id(&id) {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Snapshot not found"),
        });
    }
    let file_name = format!("{}-backup-{id}.zip", sanitize_instance_dir_name(&name));
    let slot = take_backup_file_slot(&state, &requester, &downloads_dir()).await?;
    let (start, event_id) = Event::new_progression_event_start(
        format!("Zipping a snapshot of {name} for download"),
        None,
        None,
        requester.caused_by(),
    );
    state.event_broadcaster.send(start);
    let zipping = Preparing {
        broadcaster: state.event_broadcaster.clone(),
        event_id: Some(event_id),
        cancel: state.backups.cancel_token().child_token(),
    };
    let zipped = state
        .backups
        .zip(
            &uuid,
            &id,
            downloads_dir(),
            file_name.clone(),
            zipping.cancel.clone(),
            slot,
        )
        .await;
    let (out, temp_dir, size, slot) = match zipped {
        Ok(zipped) => zipped,
        Err(e) => {
            zipping.end(false, &e.source.to_string());
            return Err(e);
        }
    };
    zipping.end(true, "Zipping complete");
    let key = register_download(
        &state,
        out.clone(),
        Some(temp_dir),
        // the whole user, so an API key's scope is checked again when the link is used
        requester.0.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;
    // the link holds the file now, and counts for its owner from here on
    drop(slot);
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(out),
        requester.caused_by(),
    ));
    Ok(Json(BackupDownload {
        key,
        file_name,
        size,
    }))
}

/// Where backups are written for download (zips and `.lodeimg` exports), each in a
/// temporary folder of its own; startup empties it (`images::share::clear_leftovers`), as a
/// core stopped while writing or before the download leaves files the size of a world.
pub(crate) fn downloads_dir() -> PathBuf {
    path_to_tmp().join("backup-downloads")
}

/// Sends a progression's progress in whole percents, as the bytes read grow.
fn percent_progress(
    broadcaster: EventBroadcaster,
    event_id: ProgressionEventID,
) -> impl FnMut(u64, u64) + Send + 'static {
    let mut sent = 0u64;
    move |done, total| {
        let percent = if total == 0 {
            100
        } else {
            (u128::from(done) * 100 / u128::from(total)).min(100) as u64
        };
        if percent > sent {
            broadcaster.send(Event::new_progression_event_update(
                &event_id,
                "Writing the file",
                (percent - sent) as f64,
            ));
            sent = percent;
        }
    }
}

async fn export_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    body: Option<Json<ExportBackupRequest>>,
) -> Result<Json<ExportedBackup>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.backups.store().instance_dir(&uuid)?;
    let name = instance.name().await;
    if !crate::backups::store::valid_backup_id(&id) {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Snapshot not found"),
        });
    }
    let include_plugin_data = body
        .and_then(|Json(body)| body.include_plugin_data)
        .unwrap_or(false);
    let file_name = format!(
        "{} - {id}.{}",
        sanitize_instance_dir_name(&name),
        crate::images::share::EXTENSION
    );
    let slot = take_backup_file_slot(&state, &requester, &downloads_dir()).await?;
    let (start, event_id) = Event::new_progression_event_start(
        format!("Exporting a snapshot of {name}"),
        Some(100.0),
        None,
        requester.caused_by(),
    );
    state.event_broadcaster.send(start);
    let preparing = Preparing {
        broadcaster: state.event_broadcaster.clone(),
        event_id: Some(event_id.clone()),
        cancel: state.backups.cancel_token().child_token(),
    };
    let exported = state
        .backups
        .export(
            &uuid,
            &id,
            name,
            include_plugin_data,
            downloads_dir(),
            file_name.clone(),
            preparing.cancel.clone(),
            percent_progress(state.event_broadcaster.clone(), event_id),
            slot,
        )
        .await;
    let (written, temp_dir, slot) = match exported {
        Ok(exported) => exported,
        Err(e) => {
            preparing.end(false, &e.source.to_string());
            return Err(e);
        }
    };
    preparing.end(true, "Export complete");
    let key = register_download(
        &state,
        written.file.clone(),
        Some(temp_dir),
        // the whole user, so an API key's scope is checked again when the link is used
        requester.0.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;
    drop(slot);
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(written.file.clone()),
        requester.caused_by(),
    ));
    Ok(Json(ExportedBackup {
        key,
        file_name,
        size: written.bytes,
        left_out: NameList::of(&written.left_out),
        left_out_for_players: NameList::of(
            written
                .left_out
                .iter()
                .filter(|path| missed_by_players(path)),
        ),
        blanked: NameList::of(&written.blanked),
        not_downloadable: NameList::of(&written.not_downloadable),
        secrets_withheld: NameList::of(&written.secrets_withheld),
    }))
}

/// Whether a player would miss the file at `path` (as an export's `left_out` names it)
/// from an instance made of the world: nothing the client view never carries (the
/// shared crate's rules: server-only files such as `server.properties`, the operators
/// and ban lists; a server's own state such as `plugins/`, Bukkit's settings files and
/// `usercache.json`; disabled files and Lodestone's own files), nor the server software
/// (`libraries/`, `versions/`, the jars, programs and installers' logs at the top), the
/// EULA, the JVM's crash files, the console history or a proxy's `forwarding.secret`,
/// which no player has. A Paper server's plugin data, which "Download this world" always
/// leaves out, would otherwise bury the few files that matter.
fn missed_by_players(path: &str) -> bool {
    use crate::images::scope::is_lodestone_name;
    use crate::images::share::export::is_server_software_name;
    use lodestone_image::paths::{in_names, is_lodestone_path, placement};
    use lodestone_image::share::is_jvm_crash_name;
    use lodestone_image::view::{is_inactive_or_bookkeeping, is_server_state};
    use lodestone_image::{CanonicalPath, Side};
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.iter().any(|part| is_lodestone_name(part)) {
        return false;
    }
    if let Ok(path) = CanonicalPath::new(path) {
        if !placement(&path).allows(Side::Client)
            || is_lodestone_path(&path)
            || is_server_state(&path)
            || is_inactive_or_bookkeeping(&path)
        {
            return false;
        }
    }
    match parts.as_slice() {
        [name] => {
            !(in_names(name, &["eula.txt", ".console_history", "forwarding.secret"])
                || is_server_software_name(name)
                || is_jvm_crash_name(name))
        }
        [top, _, ..] => !in_names(top, &["libraries", "versions"]),
        [] => false,
    }
}

pub fn get_backup_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/backups",
            get(list_backups).post(create_backup),
        )
        // a fixed segment, matched before `{id}`
        .route(
            "/instance/{uuid}/backups/policy",
            get(get_policy).put(put_policy),
        )
        .route(
            "/instance/{uuid}/backups/{id}",
            axum::routing::patch(update_backup).delete(delete_backup),
        )
        .route(
            "/instance/{uuid}/backups/{id}/restore",
            post(restore_backup),
        )
        .route(
            "/instance/{uuid}/backups/{id}/restore_as_new",
            post(restore_as_new),
        )
        .route(
            "/instance/{uuid}/backups/{id}/download",
            post(download_backup),
        )
        .route("/instance/{uuid}/backups/{id}/export", post(export_backup))
        .with_state(state)
}
