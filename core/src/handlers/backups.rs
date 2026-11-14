//! Server backups over HTTP, see [`crate::backups`].
//!
//! | Route | Permission |
//! |---|---|
//! | `GET /instance/{uuid}/backups` | ReadInstanceFile |
//! | `POST /instance/{uuid}/backups` | AccessSetting |
//! | `PATCH /instance/{uuid}/backups/{id}` | AccessSetting; unpinning also WriteInstanceFile |
//! | `DELETE /instance/{uuid}/backups/{id}` | WriteInstanceFile |
//! | `POST /instance/{uuid}/backups/{id}/restore` | WriteInstanceFile, StopInstance and StartInstance when it stops and starts the server, and WriteGlobalFile or owner rights for what it changes |
//! | `POST /instance/{uuid}/backups/{id}/restore_as_new` | CreateInstance, ReadInstanceFile, and WriteGlobalFile or owner rights for what the backup holds |
//! | `POST /instance/{uuid}/backups/{id}/download` | ReadInstanceFile |
//! | `GET /instance/{uuid}/backups/policy` | ReadInstanceFile |
//! | `PUT /instance/{uuid}/backups/policy` | AccessSetting and WriteInstanceFile (a policy can prune history) |
//!
//! Every route checks the permission, then that the server exists (404 otherwise, also for
//! Docker containers, which have no backups), then the uuid's shape, before touching the
//! store.

#[cfg(test)]
mod tests;

use std::path::Path as FsPath;
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
use crate::backups::{BackupInfo, BackupList, BackupPolicy, RestoreRequest, RestoreScope};
use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{new_fs_event, Event, FSOperation, FSTarget, ProgressionEventID};
use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::{path_to_tmp, GameInstance};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State as InstanceState, TServer};
use crate::types::InstanceUuid;
use crate::AppState;

use super::global_fs::{register_download, DownloadScope};
use super::instance::sanitize_instance_dir_name;
use super::instance_fs::is_protected;
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

/// The Minecraft server `uuid`. 404 when there is none (Docker containers included).
fn minecraft(state: &AppState, uuid: &InstanceUuid) -> Result<MinecraftInstance, Error> {
    let GameInstance::MinecraftInstance(instance) = get_instance(state, uuid)?;
    Ok(instance)
}

fn denied(message: &str) -> Error {
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
    state.backups.store().server_dir(&uuid)?;
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
    state.backups.store().server_dir(&uuid)?;
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

/// The rule for what an in-place restore changes: like the file API, adding, changing or
/// removing protected files and folders (jars, the mods folder, the config) needs
/// `WriteGlobalFile`, and adding or changing scripts needs the owner with code execution on.
fn check_restore_diff(state: &AppState, user: &User, diff: &RestoreDiff) -> Result<(), Error> {
    let touches_protected = diff
        .added
        .iter()
        .chain(&diff.changed)
        .chain(&diff.removed)
        .any(|path| is_protected(FsPath::new(path), false))
        || diff
            .created_dirs
            .iter()
            .chain(&diff.removed_dirs)
            .any(|path| is_protected(FsPath::new(path), true));
    if touches_protected && can_write_global_files(state, user).is_err() {
        return Err(denied(
            "Restoring this backup changes protected files (such as .jar files or the mods folder), which needs permission to change global files",
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

fn can_write_global_files(state: &AppState, user: &User) -> Result<(), Error> {
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
    state.backups.store().server_dir(&uuid)?;
    let manifest = state.backups.load_manifest(&uuid, &id).await?;
    if request.scope == RestoreScope::World {
        let has_world = manifest
            .level_name
            .as_deref()
            .is_some_and(|level| super::server_content::world_folder(level).is_ok());
        if !has_world {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("This backup has no world"),
            });
        }
    }
    // what this checks holds only while the server's state stays as it is now: the
    // service refuses a server that started meanwhile (`may_stop`)
    let runs = instance.state().await == InstanceState::Running;
    if runs {
        if !request.stop {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: eyre!("Stop the server before restoring a backup"),
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
            manifest,
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
    minecraft(&state, &uuid)?;
    state.backups.store().server_dir(&uuid)?;
    let manifest = state.backups.load_manifest(&uuid, &id).await?;
    let user = &requester.0;
    // what the new server gets, the requester could otherwise only put there with these
    let protected = manifest.files.iter().any(|file| {
        let path = FsPath::new(&file.path);
        is_protected(path, false)
            || path
                .ancestors()
                .skip(1)
                .filter(|dir| !dir.as_os_str().is_empty())
                .any(|dir| is_protected(dir, true))
    });
    if protected && can_write_global_files(&state, user).is_err() {
        return Err(denied(
            "This backup has protected files (such as .jar files or the mods folder), so restoring it as a new server needs permission to change global files",
        ));
    }
    if manifest
        .files
        .iter()
        .any(|file| is_code_path(FsPath::new(&file.path), false))
    {
        require_code_write(&state, user)?;
    }
    let name = body
        .and_then(|Json(body)| body.name)
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            let base: String = manifest.instance_name.chars().take(89).collect();
            format!("{} (restored)", base.trim())
        });
    let keep_command_line =
        can_write_global_files(&state, user).is_ok() && state.code_execution_enabled();
    let task_id = state
        .backups
        .start_restore_as_new(
            &state,
            manifest,
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

/// A backup being zipped for download. If the request is dropped first (the client went
/// away, or gave up waiting), dropping this ends the progression, which would otherwise
/// show forever, and stops the zip.
struct Zipping {
    broadcaster: EventBroadcaster,
    /// `None` once the End was sent.
    event_id: Option<ProgressionEventID>,
    cancel: CancellationToken,
}

impl Zipping {
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

impl Drop for Zipping {
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
    state.backups.store().server_dir(&uuid)?;
    let name = instance.name().await;
    if !crate::backups::store::valid_backup_id(&id) {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Backup not found"),
        });
    }
    let file_name = format!("{}-backup-{id}.zip", sanitize_instance_dir_name(&name));
    let (start, event_id) = Event::new_progression_event_start(
        format!("Zipping a backup of {name} for download"),
        None,
        None,
        requester.caused_by(),
    );
    state.event_broadcaster.send(start);
    let zipping = Zipping {
        broadcaster: state.event_broadcaster.clone(),
        event_id: Some(event_id),
        cancel: state.backups.cancel_token().child_token(),
    };
    let zipped = state
        .backups
        .zip(
            &uuid,
            &id,
            path_to_tmp().clone(),
            file_name.clone(),
            zipping.cancel.clone(),
        )
        .await;
    let (out, temp_dir, size) = match zipped {
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
        .with_state(state)
}
