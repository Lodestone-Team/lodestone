//! Server world backups over HTTP, see [`crate::backups::world`].
//!
//! | Route | Permission |
//! |---|---|
//! | `GET /instance/{uuid}/world-backups` | ReadInstanceFile |
//! | `POST /instance/{uuid}/world-backups` | AccessSetting |
//! | `PATCH /instance/{uuid}/world-backups/{id}` | AccessSetting |
//! | `DELETE /instance/{uuid}/world-backups/{id}` | WriteInstanceFile |
//! | `POST /instance/{uuid}/world-backups/{id}/restore` | WriteInstanceFile, and WriteGlobalFile for protected files it adds, changes or removes, and the owner with code execution for scripts it places. The server must be stopped |
//! | `POST /instance/{uuid}/world-backups/{id}/download` | ReadInstanceFile |
//! | `POST /instance/{uuid}/world-backups/tasks/{task_id}/cancel` | AccessSetting for a world backup, WriteInstanceFile for a restore |
//!
//! The same permissions as the matching snapshot routes (`super::backups`). Every route
//! checks the permission, then that the server exists (404 otherwise), then the uuid's
//! shape, before touching the store.

#[cfg(test)]
mod tests;

use std::path::Path as FsPath;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::eyre;

use crate::auth::user::{User, UserAction};
use crate::backups::world::reader::RestoreZipPlan;
use crate::backups::world::service::WorldCheck;
use crate::backups::world::types::{
    CreateWorldBackupRequest, UpdateWorldBackupRequest, WorldBackupDownload, WorldBackupInfo,
    WorldBackupList,
};
use crate::backups::JobKind;
use crate::error::{Error, ErrorKind};
use crate::events::{new_fs_event, FSOperation, FSTarget};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State as InstanceState, TServer};
use crate::types::InstanceUuid;
use crate::AppState;

use super::backups::{can_write_global_files, denied, minecraft, restore_protected, BackupTask};
use super::global_fs::{register_stored_download, DownloadScope};
use super::instance::sanitize_instance_dir_name;
use super::util::{is_code_path, require_code_write, AuthedUser};

async fn list_world_backups(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<WorldBackupList>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    Ok(Json(state.world_backups.list(&uuid).await?))
}

async fn create_world_backup(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    body: Option<Json<CreateWorldBackupRequest>>,
) -> Result<Json<BackupTask>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    let request = body.map(|Json(body)| body).unwrap_or_default();
    let task_id = state
        .world_backups
        .start_backup(&state, instance, request.name, requester.caused_by())
        .await?;
    Ok(Json(BackupTask { task_id }))
}

async fn update_world_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    Json(request): Json<UpdateWorldBackupRequest>,
) -> Result<Json<WorldBackupInfo>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    state.world_backups.store().dir(&uuid)?;
    match request.name {
        Some(name) => Ok(Json(state.world_backups.rename(&uuid, &id, name).await?)),
        // nothing to change
        None => Ok(Json(WorldBackupInfo::from(
            &state.world_backups.load(&uuid, &id).await?,
        ))),
    }
}

async fn delete_world_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<serde_json::Value>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    state.world_backups.delete(&uuid, &id).await?;
    Ok(Json(serde_json::json!({})))
}

/// The rule for what a world restore changes, as for a snapshot's: adding, replacing or
/// removing protected files and folders (jars, Lodestone's own files) needs
/// `WriteGlobalFile`, and placing scripts needs the owner with code execution on. Every
/// file and folder under the folders it replaces is removed or replaced.
fn check_world_restore(
    state: &AppState,
    user: &User,
    plan: &RestoreZipPlan,
    current: &[(String, bool)],
) -> Result<(), Error> {
    let touches_protected = plan
        .files
        .iter()
        .any(|file| restore_protected(&file.disk, false))
        || plan
            .folders
            .iter()
            .any(|folder| restore_protected(folder, true))
        || current
            .iter()
            .any(|(path, is_dir)| restore_protected(path, *is_dir));
    if touches_protected && can_write_global_files(state, user).is_err() {
        return Err(denied(
            "Restoring this world backup changes protected files (such as .jar files), which needs permission to change global files",
        ));
    }
    if plan
        .files
        .iter()
        .any(|file| is_code_path(FsPath::new(&file.disk), false))
    {
        require_code_write(state, user)?;
    }
    Ok(())
}

async fn restore_world_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<BackupTask>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.world_backups.store().dir(&uuid)?;
    if instance.state().await != InstanceState::Stopped {
        return Err(Error {
            kind: ErrorKind::Conflict,
            source: eyre!("Stop the server before restoring a world backup"),
        });
    }
    // only worth comparing the files when some change could be refused
    let user = requester.0.clone();
    let needs_check = can_write_global_files(&state, &user).is_err()
        || require_code_write(&state, &user).is_err();
    let check: Option<WorldCheck> = needs_check.then(|| {
        let state = state.clone();
        Arc::new(move |plan: &RestoreZipPlan, current: &[(String, bool)]| {
            check_world_restore(&state, &user, plan, current)
        }) as WorldCheck
    });
    let task_id = state
        .world_backups
        .start_restore(&state, instance, &id, check, requester.caused_by())
        .await?;
    Ok(Json(BackupTask { task_id }))
}

async fn download_world_backup(
    State(state): State<AppState>,
    Path((uuid, id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<WorldBackupDownload>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft(&state, &uuid)?;
    state.world_backups.store().dir(&uuid)?;
    let (path, size, record) = state.world_backups.zip_for_download(&uuid, &id).await?;
    let file_name = format!(
        "{}-{}-{}.zip",
        sanitize_instance_dir_name(&instance.name().await),
        sanitize_filename::sanitize(&record.level),
        record.id
    );
    let key = register_stored_download(
        &state,
        path.clone(),
        file_name.clone(),
        // the whole user, so an API key's scope is checked again when the link is used
        requester.0.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(path),
        requester.caused_by(),
    ));
    Ok(Json(WorldBackupDownload {
        key,
        file_name,
        size,
    }))
}

async fn cancel_world_task(
    State(state): State<AppState>,
    Path((uuid, task_id)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<serde_json::Value>, Error> {
    let not_running = || Error {
        kind: ErrorKind::NotFound,
        source: eyre!("No such world backup or restore is running"),
    };
    // the permission of the route that started it; nothing is said about a job of a
    // server the requester can't see
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    minecraft(&state, &uuid)?;
    state.world_backups.store().dir(&uuid)?;
    let action = match state.world_backups.job_kind(&uuid, &task_id) {
        Some(JobKind::WorldBackup) => UserAction::AccessSetting(uuid.clone()),
        Some(JobKind::WorldRestore) => UserAction::WriteInstanceFile(uuid.clone()),
        _ => return Err(not_running()),
    };
    requester.require(&state, action).await?;
    state.world_backups.cancel(&uuid, &task_id)?;
    Ok(Json(serde_json::json!({})))
}

pub fn get_world_backup_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/world-backups",
            get(list_world_backups).post(create_world_backup),
        )
        .route(
            "/instance/{uuid}/world-backups/{id}",
            axum::routing::patch(update_world_backup).delete(delete_world_backup),
        )
        .route(
            "/instance/{uuid}/world-backups/{id}/restore",
            post(restore_world_backup),
        )
        .route(
            "/instance/{uuid}/world-backups/{id}/download",
            post(download_world_backup),
        )
        // a fixed segment, matched before `{id}`
        .route(
            "/instance/{uuid}/world-backups/tasks/{task_id}/cancel",
            post(cancel_world_task),
        )
        .with_state(state)
}
