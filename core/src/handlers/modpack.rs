//! Modpacks: `/modpack/inspect`, `/modpack/upload` and `/instance/create_modpack`.
//! Updating a server's modpack is in [`super::modpack_update`].
//!
//! All need `CreateInstance`. An uploaded pack can hold any jar (in its overrides, or from
//! GitHub), so uploading one, and creating an instance from an upload, also needs
//! `WriteGlobalFile`, the permission the file API needs to create `.jar` files.

use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::routing::post;
use axum::{Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use super::global_fs::multipart_error;
use super::instance::{spawn_minecraft_instance_creation, PostSetupStep};
use super::util::AuthedUser;
use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::Event;
use crate::implementations::minecraft::{validate_new_instance_name, SetupConfig};
use crate::modpack::store::StoredPack;
use crate::modpack::{
    describe_skipped, install_pack_contents, read_pack_async, server_flavour, write_port,
    ModpackInfo, ModpackOrigin, MAX_PACK_FILE_BYTES,
};
use crate::modrinth::links::check_api_segment;
use crate::modrinth::CDN_HOST;
use crate::mods::bad_request;
use crate::AppState;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ModpackInspectRequest {
    pub version_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModpackSource {
    Modrinth { version_id: String },
    Upload { upload_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateModpackInstanceRequest {
    pub source: ModpackSource,
    pub name: String,
    pub port: u32,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub min_ram: Option<u32>,
    #[serde(default)]
    #[ts(optional)]
    pub max_ram: Option<u32>,
    #[serde(default)]
    #[ts(optional)]
    pub auto_start: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub restart_on_crash: Option<bool>,
    /// Whether the user agreed to the Minecraft EULA (https://aka.ms/MinecraftEULA).
    /// The server is only created with the EULA accepted when this is `true`.
    #[serde(default)]
    pub accept_eula: bool,
}

/// The pack of a Modrinth modpack version: a stored copy, or a new download.
pub(super) async fn modrinth_pack(state: &AppState, version_id: &str) -> Result<StoredPack, Error> {
    let version_id = check_api_segment(version_id, "version id")?;
    if let Some(pack) = state.mods.packs.find_version(version_id) {
        return Ok(pack);
    }
    let api = state.mods.modrinth.clone();
    let version = api.version(version_id).await?;
    let is_mrpack = |name: &str| name.to_lowercase().ends_with(".mrpack");
    let file = version
        .files
        .iter()
        .find(|f| f.primary && is_mrpack(&f.filename))
        .or_else(|| version.files.iter().find(|f| is_mrpack(&f.filename)))
        .ok_or_else(|| bad_request(format!("{} has no .mrpack file", version.name)))?;
    let url = url::Url::parse(&file.url)
        .map_err(|_| eyre!("Modrinth sent an invalid download URL for {}", version.name))?;
    if url.scheme() != "https" || url.host_str() != Some(CDN_HOST) {
        return Err(bad_request(format!(
            "Refusing to download {}: modpacks are only downloaded from {CDN_HOST}",
            file.filename
        )));
    }
    if file.size > MAX_PACK_FILE_BYTES {
        return Err(crate::modrinth::client::too_large(MAX_PACK_FILE_BYTES));
    }
    let temp = state.mods.packs.new_file()?;
    let mut out = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&temp)
        .await
        .context("Failed to open the modpack file")?;
    let hashes = api.download(&url, &mut out, MAX_PACK_FILE_BYTES).await?;
    drop(out);
    if !hashes.sha512.eq_ignore_ascii_case(&file.sha512) {
        return Err(Error {
            kind: ErrorKind::External,
            source: eyre!("The modpack download is damaged: its SHA-512 hash does not match"),
        });
    }
    let pack = read_pack_async(temp.to_path_buf(), false).await?;
    let id = state
        .mods
        .packs
        .insert(temp, pack.info, Some(version_id.to_string()), None);
    state
        .mods
        .packs
        .get(&id)
        .ok_or_else(|| eyre!("The modpack was removed right after it was stored").into())
}

/// The pack `source` names: a Modrinth version (downloaded, or the stored copy), or a pack
/// this user uploaded. Using an uploaded pack needs `WriteGlobalFile`, since it can hold
/// any jar.
pub(super) async fn stored_pack(
    state: &AppState,
    requester: &AuthedUser,
    source: &ModpackSource,
) -> Result<StoredPack, Error> {
    Ok(match source {
        ModpackSource::Modrinth { version_id } => modrinth_pack(state, version_id).await?,
        ModpackSource::Upload { upload_id } => {
            let stored = state
                .mods
                .packs
                .get(upload_id)
                .filter(|p| p.uploader.is_none() || p.uploader.as_ref() == Some(&requester.uid))
                .ok_or_else(|| Error {
                    kind: ErrorKind::NotFound,
                    source: eyre!(
                        "The modpack was not found. Packs are kept for one hour; upload it again"
                    ),
                })?;
            // an `upload_id` from `/modpack/inspect` is a Modrinth pack; only packs that
            // were really uploaded need the permission to add any jar
            if stored.uploader.is_some() {
                requester
                    .require(state, UserAction::WriteGlobalFile)
                    .await?;
            }
            stored
        }
    })
}

/// The info of a stored pack; only supported packs get an `upload_id`.
fn public_info(pack: &StoredPack) -> ModpackInfo {
    let mut info = pack.info.clone();
    if info.unsupported_reason.is_some() {
        info.upload_id = None;
    }
    info
}

async fn inspect_modpack(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<ModpackInspectRequest>,
) -> Result<Json<ModpackInfo>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    let pack = modrinth_pack(&state, &request.version_id).await?;
    Ok(Json(public_info(&pack)))
}

async fn upload_modpack(
    State(state): State<AppState>,
    requester: AuthedUser,
    mut multipart: Multipart,
) -> Result<Json<ModpackInfo>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    requester
        .require(&state, UserAction::WriteGlobalFile)
        .await?;
    let mut stored = None;
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }
        if stored.is_some() {
            return Err(bad_request("Upload one modpack at a time"));
        }
        let name = field.file_name().unwrap_or_default().to_string();
        if !name.to_lowercase().ends_with(".mrpack") {
            return Err(bad_request(format!("{name:?} is not a .mrpack file")));
        }
        let temp = state.mods.packs.new_file()?;
        let mut out = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&temp)
            .await
            .context("Failed to open the modpack file")?;
        let mut size = 0_u64;
        while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
            size += chunk.len() as u64;
            if size > MAX_PACK_FILE_BYTES {
                return Err(crate::modrinth::client::too_large(MAX_PACK_FILE_BYTES));
            }
            out.write_all(&chunk)
                .await
                .context("Failed to write the modpack file")?;
        }
        out.flush()
            .await
            .context("Failed to write the modpack file")?;
        stored = Some(temp);
    }
    let temp = stored.ok_or_else(|| bad_request("No modpack was uploaded"))?;
    let pack = read_pack_async(temp.to_path_buf(), false).await?;
    let id = state
        .mods
        .packs
        .insert(temp, pack.info, None, Some(requester.uid.clone()));
    let pack = state
        .mods
        .packs
        .get(&id)
        .ok_or_else(|| eyre!("The modpack was removed right after it was stored"))?;
    Ok(Json(public_info(&pack)))
}

fn check_settings(request: &CreateModpackInstanceRequest) -> Result<(), Error> {
    validate_new_instance_name(&request.name)?;
    if !(1..=65535).contains(&request.port) {
        return Err(bad_request("The port must be between 1 and 65535"));
    }
    if request.min_ram == Some(0) || request.max_ram == Some(0) {
        return Err(bad_request("RAM must be at least 1 MB"));
    }
    if let (Some(min), Some(max)) = (request.min_ram, request.max_ram) {
        if min > max {
            return Err(bad_request("Minimum RAM cannot be larger than maximum RAM"));
        }
    }
    Ok(())
}

async fn create_modpack_instance(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<CreateModpackInstanceRequest>,
) -> Result<Json<crate::types::InstanceUuid>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    check_settings(&request)?;
    let stored = stored_pack(&state, &requester, &request.source).await?;
    // scripts in a pack are only copied for the owner, while code execution is on
    let allow_scripts = requester.is_owner && state.code_execution_enabled();
    let pack = read_pack_async(stored.path(), allow_scripts).await?;
    if let Some(reason) = &pack.info.unsupported_reason {
        return Err(bad_request(reason.clone()));
    }
    let origin = match &stored.version_id {
        Some(version_id) => {
            ModpackOrigin::from_modrinth(state.mods.modrinth.as_ref(), version_id).await?
        }
        None => ModpackOrigin::from_upload(&pack.info),
    };
    let flavour = server_flavour(&pack.info).await?;
    let port = request.port;
    let setup_config = SetupConfig {
        name: request.name.clone(),
        version: pack.info.game_version.clone(),
        flavour,
        port,
        cmd_args: Vec::new(),
        description: request.description.clone(),
        min_ram: request.min_ram,
        max_ram: request.max_ram,
        auto_start: request.auto_start,
        restart_on_crash: request.restart_on_crash,
        backup_period: None,
        modpack: Some(origin),
        accept_eula: request.accept_eula,
    };
    let title = format!(
        "Setting up Minecraft server {} from {}",
        request.name, pack.info.name
    );
    let api = state.mods.modrinth.clone();
    let event_broadcaster = state.event_broadcaster.clone();
    let post_setup: PostSetupStep = Box::new(move |root, event_id| {
        Box::pin(async move {
            // `stored` keeps the pack file until the instance is set up
            let pack_path = stored.path();
            let progress = |message: String, progress: f64| {
                event_broadcaster.send(Event::new_progression_event_update(
                    &event_id, message, progress,
                ));
            };
            let report = install_pack_contents(
                api.as_ref(),
                &pack,
                &pack_path,
                &root,
                allow_scripts,
                &progress,
            )
            .await?;
            write_port(&root, port).await?;
            drop(stored);
            Ok(describe_skipped(&report.skipped))
        })
    });
    let uuid = spawn_minecraft_instance_creation(
        state,
        &requester,
        setup_config,
        title,
        15.0,
        Some(post_setup),
    )
    .await?;
    Ok(Json(uuid))
}

pub fn get_modpack_routes(state: AppState) -> Router {
    Router::new()
        .route("/modpack/inspect", post(inspect_modpack))
        .route(
            "/modpack/upload",
            post(upload_modpack).layer(DefaultBodyLimit::disable()),
        )
        .route("/instance/create_modpack", post(create_modpack_instance))
        .route(
            "/instance/{uuid}/modpack/update/plan",
            post(super::modpack_update::plan_update),
        )
        .route(
            "/instance/{uuid}/modpack/update",
            post(super::modpack_update::update_modpack),
        )
        .with_state(state)
}
