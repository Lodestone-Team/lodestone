//! The mods (or plugins) of an instance: `/instance/{uuid}/mods`, and the client modpack
//! of a Fabric instance: `/instance/{uuid}/mods/client_pack`.
//!
//! Listing needs `ReadInstanceFile`; changes need `WriteInstanceFile`. A jar that Modrinth
//! does not know is arbitrary code, so uploading or enabling one also needs
//! `WriteGlobalFile`, the permission the file API needs to create `.jar` files.

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use super::global_fs::{multipart_error, register_download, DownloadScope};
use super::instance::sanitize_instance_dir_name;
use super::util::{get_instance, AuthedUser};
use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{Event, ProgressionEndValue};
use crate::modrinth::links::check_api_segment;
use crate::mods::client::{build_client_pack, ClientPackDownload};
use crate::mods::install::{self, ModInstallRequest, ModInstallResult};
use crate::mods::scan::{scan_folder, sha512_of};
use crate::mods::{
    bad_request, build_list, match_files_or_none, safe_jar_name, save_uploads, InstalledMod,
    InstalledModList, InstalledModLoader, ModFolder, MAX_MOD_BYTES,
};
use crate::prelude::{path_to_tmp, GameInstance};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::TServer;
use crate::types::InstanceUuid;
use crate::AppState;

/// The instance and its mods folder. Fails for instances without mods or plugins.
pub(super) async fn mod_folder(
    state: &AppState,
    uuid: &InstanceUuid,
) -> Result<(GameInstance, ModFolder), Error> {
    let instance = get_instance(state, uuid)?;
    let folder = ModFolder::new(
        &instance.game_type().await,
        instance.version().await,
        &instance.path().await,
    )?;
    Ok((instance, folder))
}

/// The Fabric loader version of a Fabric instance.
pub(super) async fn fabric_loader_version(instance: &GameInstance) -> Option<String> {
    match instance {
        GameInstance::MinecraftInstance(i) => i.fabric_loader_version().await,
    }
}

/// Remembers that the instance needs a restart if it is running.
async fn note_change(state: &AppState, uuid: &InstanceUuid, instance: &GameInstance) {
    state.mods.restart.note_change(uuid, instance.state().await);
}

fn not_on_modrinth(file_name: &str) -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!(
            "{file_name} is not on Modrinth. Only users who can write global files can add jars that are not on Modrinth"
        ),
    }
}

/// For users without `WriteGlobalFile`: checks that every hash belongs to a Modrinth
/// version, failing with the first file that does not.
async fn require_known_jars(
    state: &AppState,
    requester: &AuthedUser,
    files: &[(String, String)],
) -> Result<(), Error> {
    if requester.can_perform_action(&UserAction::WriteGlobalFile) {
        return Ok(());
    }
    let hashes: Vec<String> = files.iter().map(|(_, hash)| hash.clone()).collect();
    let known = state.mods.modrinth.versions_by_hash(&hashes).await?;
    match files.iter().find(|(_, hash)| !known.contains_key(hash)) {
        Some((name, _)) => Err(not_on_modrinth(name)),
        None => Ok(()),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    check_updates: Option<bool>,
}

async fn list_mods(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<InstalledModList>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    let files = scan_folder(&folder).await?;
    let list = build_list(
        state.mods.modrinth.as_ref(),
        &folder,
        files,
        query.check_updates.unwrap_or(false),
        state.mods.restart.is_required(&uuid),
        fabric_loader_version(&instance).await,
    )
    .await?;
    Ok(Json(list))
}

async fn install_mod(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ModInstallRequest>,
) -> Result<Json<ModInstallResult>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let version_id = check_api_segment(&request.version_id, "version id")?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    let api = state.mods.modrinth.clone();
    let version = api.version(version_id).await?;
    let project = api.project(&version.project_id).await?;

    let lock = state.mods.lock_for(&uuid);
    let _guard = lock.lock().await;
    let (start, event_id) = Event::new_progression_event_start(
        format!("Installing {}", project.title),
        None,
        None,
        requester.caused_by(),
    );
    state.event_broadcaster.send(start);
    let result =
        install::install(api.as_ref(), &folder, version, &project, (&request).into()).await;
    let (success, message) = match &result {
        Ok(r) => (
            true,
            format!(
                "Installed {} file(s){}",
                r.installed.len(),
                if r.skipped.is_empty() {
                    String::new()
                } else {
                    format!(", skipped {}", r.skipped.len())
                }
            ),
        ),
        Err(e) => (
            false,
            format!("Installing {} failed: {}", project.title, e.source),
        ),
    };
    state
        .event_broadcaster
        .send(Event::new_progression_event_end(
            event_id,
            success,
            Some(&message),
            Some(ProgressionEndValue::FSOperationCompleted {
                instance_uuid: uuid.clone(),
                success,
                message: message.clone(),
            }),
        ));
    if success {
        note_change(&state, &uuid, &instance).await;
    }
    result.map(Json)
}

#[derive(Deserialize)]
struct UploadQuery {
    overwrite: Option<bool>,
}

async fn upload_mods(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<Vec<InstalledMod>>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    tokio::fs::create_dir_all(&folder.dir)
        .await
        .context(format!(
            "Failed to create the {} folder",
            folder.directory()
        ))?;

    // stream every file to a temporary file first; nothing is in place until all are checked
    let mut uploads: Vec<(tempfile::TempPath, String)> = Vec::new();
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }
        let raw_name = field.file_name().unwrap_or_default().to_string();
        let name = safe_jar_name(&raw_name)
            .ok_or_else(|| bad_request(format!("{raw_name:?} is not a .jar file")))?;
        let temp = tempfile::Builder::new()
            .prefix(".lodestone-upload-")
            .suffix(".tmp")
            .tempfile_in(&folder.dir)
            .context("Failed to create a temporary file")?
            .into_temp_path();
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&temp)
            .await
            .context("Failed to open a temporary file")?;
        let mut size = 0_u64;
        while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
            size += chunk.len() as u64;
            if size > MAX_MOD_BYTES {
                return Err(crate::modrinth::client::too_large(MAX_MOD_BYTES));
            }
            file.write_all(&chunk)
                .await
                .context(format!("Failed to write {name}"))?;
        }
        file.flush()
            .await
            .context(format!("Failed to write {name}"))?;
        uploads.push((temp, name));
    }
    if uploads.is_empty() {
        return Err(bad_request("No file was uploaded"));
    }

    if !requester.can_perform_action(&UserAction::WriteGlobalFile) {
        let paths: Vec<(String, std::path::PathBuf)> = uploads
            .iter()
            .map(|(temp, name)| (name.clone(), temp.to_path_buf()))
            .collect();
        let hashes = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|(name, path)| sha512_of(&path).map(|hash| (name, hash)))
                .collect::<std::io::Result<Vec<_>>>()
        })
        .await
        .map_err(|e| eyre!("The upload task failed: {e}"))?
        .context("Failed to read an uploaded file")?;
        require_known_jars(&state, &requester, &hashes).await?;
    }

    let lock = state.mods.lock_for(&uuid);
    let _guard = lock.lock().await;
    let files = save_uploads(&folder, uploads, query.overwrite.unwrap_or(false)).await?;
    note_change(&state, &uuid, &instance).await;
    Ok(Json(
        match_files_or_none(state.mods.modrinth.as_ref(), files, folder.loader).await,
    ))
}

async fn set_enabled(
    state: AppState,
    uuid: InstanceUuid,
    file_name: String,
    requester: AuthedUser,
    enabled: bool,
) -> Result<Json<InstalledMod>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    let lock = state.mods.lock_for(&uuid);
    let _guard = lock.lock().await;
    // the copy that is checked is the one enabled, so the file can't be swapped meanwhile
    let (file, changed) = if enabled {
        crate::mods::enable_checked(&folder, &file_name, |name, sha512| {
            let (state, requester) = (&state, &requester);
            async move { require_known_jars(state, requester, &[(name, sha512)]).await }
        })
        .await?
    } else {
        crate::mods::set_enabled(&folder, &file_name, enabled).await?
    };
    if changed {
        note_change(&state, &uuid, &instance).await;
    }
    let mut mods =
        match_files_or_none(state.mods.modrinth.as_ref(), vec![file], folder.loader).await;
    mods.pop()
        .map(Json)
        .ok_or_else(|| eyre!("The mod disappeared while it was changed").into())
}

async fn enable_mod(
    State(state): State<AppState>,
    Path((uuid, file_name)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<InstalledMod>, Error> {
    set_enabled(state, uuid, file_name, requester, true).await
}

async fn disable_mod(
    State(state): State<AppState>,
    Path((uuid, file_name)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<InstalledMod>, Error> {
    set_enabled(state, uuid, file_name, requester, false).await
}

async fn delete_mod(
    State(state): State<AppState>,
    Path((uuid, file_name)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    let lock = state.mods.lock_for(&uuid);
    let _guard = lock.lock().await;
    crate::mods::delete(&folder, &file_name).await?;
    if let Err(e) = crate::mods::client::forget_mark(&folder, &file_name).await {
        tracing::warn!(
            "Failed to unmark {file_name} after deleting it: {}",
            e.source
        );
    }
    note_change(&state, &uuid, &instance).await;
    Ok(Json(()))
}

#[derive(Deserialize, TS)]
#[ts(export)]
pub struct SetClientMark {
    /// Whether players need the mod: `true` marks it, `false` takes the mark off and
    /// leaves it out of the client pack (where Modrinth says players can join without it).
    pub required: bool,
}

/// `PUT /instance/{uuid}/mods/{file_name}/client`: marks a mod as needed by players, so
/// the client pack gives it to them (with the mods it needs), or leaves it out.
/// Players' copies change the next time they join; the server needs no restart.
async fn set_client_mark(
    State(state): State<AppState>,
    Path((uuid, file_name)): Path<(InstanceUuid, String)>,
    requester: AuthedUser,
    Json(body): Json<SetClientMark>,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let (_, folder) = mod_folder(&state, &uuid).await?;
    let lock = state.mods.lock_for(&uuid);
    let _guard = lock.lock().await;
    crate::mods::client::set_marked(&folder, &file_name, body.required).await?;
    Ok(Json(()))
}

/// Builds the client modpack and returns a key for `GET /file/{key}`, the download
/// mechanism of the file API. The pack is deleted when the key expires.
async fn client_pack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<ClientPackDownload>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let (instance, folder) = mod_folder(&state, &uuid).await?;
    if folder.loader != InstalledModLoader::Fabric {
        return Err(bad_request(
            "Client modpacks are only available for Fabric servers",
        ));
    }
    let name = instance.name().await;
    tokio::fs::create_dir_all(path_to_tmp())
        .await
        .context("Failed to create the temporary folder")?;
    let temp_dir =
        tempfile::tempdir_in(path_to_tmp()).context("Failed to create a temporary folder")?;
    let file_name = format!("{}-client.mrpack", sanitize_instance_dir_name(&name));
    let out = temp_dir.path().join(&file_name);
    let plan = build_client_pack(
        state.mods.modrinth.as_ref(),
        &folder,
        &name,
        fabric_loader_version(&instance).await.as_deref(),
        &out,
    )
    .await?;
    let key = register_download(
        &state,
        out,
        Some(temp_dir),
        // the whole user, so an API key's scope is checked again when the link is used
        requester.0.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;
    Ok(Json(ClientPackDownload {
        key,
        file_name,
        mod_count: (plan.index.files.len() + plan.included.len()) as u32,
        included_jars: plan.included.len() as u32,
    }))
}

pub fn get_instance_mods_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/{uuid}/mods", get(list_mods))
        .route("/instance/{uuid}/mods/install", post(install_mod))
        .route("/instance/{uuid}/mods/client_pack", get(client_pack))
        .route(
            "/instance/{uuid}/mods/upload",
            post(upload_mods).layer(DefaultBodyLimit::disable()),
        )
        .route("/instance/{uuid}/mods/{file_name}", delete(delete_mod))
        .route("/instance/{uuid}/mods/{file_name}/enable", put(enable_mod))
        .route(
            "/instance/{uuid}/mods/{file_name}/disable",
            put(disable_mod),
        )
        .route(
            "/instance/{uuid}/mods/{file_name}/client",
            put(set_client_mark),
        )
        .with_state(state)
}
