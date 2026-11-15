//! A server from a `.lodeimg` file another Host or a Launcher made, see
//! [`crate::images::share`].
//!
//! | Route | Permission |
//! |---|---|
//! | `POST /instance/image/upload` | CreateInstance |
//! | `DELETE /instance/image/upload/{upload_id}` | CreateInstance, and only the uploader |
//! | `POST /instance/create_from_image` | CreateInstance, and WriteGlobalFile when the file carries protected files (jars, the mods folder) |
//!
//! An upload is streamed into a temporary file (at most 64 GB, at most four arriving at
//! once and two of one user's, with the disk's free space checked before and while it
//! arrives, less what the others arriving still declare), read and checked, and kept for
//! an hour for the user who uploaded it (at most two of each user's, their oldest going
//! first; deleted when the hour is up, whatever else happens); its answer says what it
//! holds. Files are read one at a time. Creating a server from it is the creation flow
//! every new server takes, with the progression events of `POST /instance/create`; servers
//! are made from files one at a time, each checking the free space again, less what
//! uploads arriving still declare, before it unpacks and before it places.

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, post};
use axum::{Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::Event;
use crate::images::share::import::{self, PlanRequest};
use crate::images::share::inspect::{self, Caps, ImageSummary};
use crate::images::share::uploads::MAX_IMAGE_UPLOAD_BYTES;
use crate::implementations::minecraft::{FlavourKind, SetupConfig};
use crate::prelude::path_to_instances;
use crate::util::disk::require_free_space;
use crate::AppState;

use super::global_fs::multipart_error;
use super::instance::{spawn_minecraft_instance_creation, PostSetupStep};
use super::util::AuthedUser;

/// `POST /instance/image/upload`: the upload's id, for `create_from_image`, and what the
/// file holds.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImageUpload {
    pub upload_id: String,
    pub summary: ImageSummary,
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// What `requester` may add to a new server.
pub(super) fn caps(state: &AppState, requester: &AuthedUser) -> Caps {
    Caps {
        may_write_global: requester
            .try_action(&UserAction::WriteGlobalFile, state.safe_mode())
            .is_ok(),
        may_write_code: requester.is_owner && state.code_execution_enabled(),
    }
}

/// How often an upload checks the disk's free space again as it arrives.
const FREE_SPACE_CHECK_EVERY: u64 = 256 << 20;

async fn upload_image(
    State(state): State<AppState>,
    requester: AuthedUser,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ImageUpload>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    let uploads = state.image_uploads.clone();
    // the whole request's size, when the client says it: refused at once when the disk
    // can't take it, rather than after gigabytes
    let declared = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|size| size > MAX_IMAGE_UPLOAD_BYTES.saturating_add(1 << 20)) {
        return Err(crate::modrinth::client::too_large(MAX_IMAGE_UPLOAD_BYTES));
    }
    let dir = uploads.dir().to_path_buf();
    // counted against the uploads arriving at once, its declared size held against theirs
    let receiving = uploads.start(&requester.uid, declared.unwrap_or(0))?;
    {
        let dir = dir.clone();
        let size = declared
            .unwrap_or(0)
            .saturating_add(receiving.others_remaining());
        crate::images::blocking(move || require_free_space(&dir, size, "to upload the file"))
            .await?;
    }
    let mut stored = None;
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            continue;
        }
        if stored.is_some() {
            return Err(bad_request("Upload one file at a time"));
        }
        let name = field.file_name().unwrap_or_default().to_string();
        let extension = format!(".{}", crate::images::share::EXTENSION);
        if !name.to_lowercase().ends_with(&extension) {
            return Err(bad_request(format!(
                "{} is not a {extension} file",
                lodestone_image::display_text(&name)
            )));
        }
        let temp = uploads.new_file()?;
        // buffered: each write to a tokio file is a trip to a blocking thread, and a body
        // arrives in pieces of a few KiB
        let mut out = tokio::io::BufWriter::with_capacity(
            1 << 20,
            tokio::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&temp)
                .await
                .context("Failed to open the upload's file")?,
        );
        let mut size = 0_u64;
        let mut next_check = FREE_SPACE_CHECK_EVERY;
        while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
            size += chunk.len() as u64;
            if size > MAX_IMAGE_UPLOAD_BYTES {
                return Err(crate::modrinth::client::too_large(MAX_IMAGE_UPLOAD_BYTES));
            }
            out.write_all(&chunk)
                .await
                .context("Failed to write the upload's file")?;
            receiving.wrote(size);
            if size >= next_check {
                next_check = size.saturating_add(FREE_SPACE_CHECK_EVERY);
                let dir = dir.clone();
                let needed = FREE_SPACE_CHECK_EVERY.saturating_add(receiving.others_remaining());
                crate::images::blocking(move || {
                    require_free_space(&dir, needed, "to upload the file")
                })
                .await?;
            }
        }
        out.flush()
            .await
            .context("Failed to write the upload's file")?;
        stored = Some((temp, name));
    }
    let (temp, name) = stored.ok_or_else(|| bad_request("No file was uploaded"))?;
    let caps = caps(&state, &requester);
    let summary = {
        let reading = uploads.reading().await;
        let (path, name) = (temp.to_path_buf(), name.clone());
        // the turn and the upload's slot go into the read, so they last as long as it does
        // even when the client hangs up and this request is dropped
        crate::images::blocking(move || {
            let _hold = (reading, receiving);
            inspect::inspect(&path, &name, caps, path_to_instances()).map(|i| i.summary)
        })
        .await?
    };
    let upload_id = uploads.insert(temp, name, requester.uid.clone(), summary.clone());
    Ok(Json(ImageUpload { upload_id, summary }))
}

async fn discard_upload(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(upload_id): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    if !state.image_uploads.remove(&upload_id, &requester.uid) {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("The file was not found. Uploads are kept for one hour"),
        });
    }
    Ok(Json(serde_json::json!({})))
}

/// `POST /instance/create_from_image`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateFromImageRequest {
    pub upload_id: String,
    /// The image id the summary showed: a file that changed since is refused.
    pub image: String,
    pub name: String,
    pub port: u32,
    /// Whether the user agreed to the Minecraft EULA (https://aka.ms/MinecraftEULA). The
    /// server is only created with the EULA accepted when this is `true`.
    #[serde(default)]
    pub accept_eula: bool,
    /// The world (its folder in the file) that becomes the server's world. A Launcher's
    /// file gives only this world, none when it is left out; a Host's gives every world,
    /// and this one is the level (left out: the file's own).
    #[serde(default)]
    #[ts(optional)]
    pub world: Option<String>,
    /// Take the file's operators, who can run any command on the server. Off by default.
    #[serde(default)]
    pub keep_operators: bool,
    /// The server to make, one of the summary's `server_choices` (a vanilla game's file
    /// can become a Vanilla or a Paper server). Left out: the summary's `server`.
    #[serde(default)]
    #[ts(optional)]
    pub flavour: Option<FlavourKind>,
    #[serde(default)]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub min_ram: Option<u32>,
    #[serde(default)]
    #[ts(optional)]
    pub max_ram: Option<u32>,
}

fn check_settings(request: &CreateFromImageRequest) -> Result<(), Error> {
    crate::implementations::minecraft::validate_new_instance_name(&request.name)?;
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

/// The progress the file's import takes in the creation, beside setting up the server.
const IMPORT_PROGRESS: f64 = 10.0;

async fn create_from_image(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<CreateFromImageRequest>,
) -> Result<Json<crate::types::InstanceUuid>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;
    check_settings(&request)?;
    // one creation from an upload at a time, until this creation ends however it ends
    let (upload, in_use) = state
        .image_uploads
        .take(&request.upload_id, &requester.uid)?;
    let caps = caps(&state, &requester);
    let plan_request = PlanRequest {
        world: request.world.clone().filter(|w| !w.is_empty()),
        keep_operators: request.keep_operators,
        flavour: request.flavour,
    };
    // read again, all of it, as it is now: what was shown must be what is made
    let (container, plan, game, needed) = {
        let reading = state.image_uploads.reading().await;
        let (path, expected) = (upload.path(), request.image.clone());
        let file_name = upload.file_name.clone();
        crate::images::blocking(move || {
            let _reading = reading;
            let inspected = inspect::inspect(&path, &file_name, caps, path_to_instances())?;
            if inspected.summary.image != expected {
                return Err(Error {
                    kind: ErrorKind::Conflict,
                    source: eyre!("The file changed since it was opened; open it again"),
                });
            }
            let image = inspected.container.image().clone();
            let linked = inspected.container.linked().to_vec();
            let images: Vec<&lodestone_image::Image> =
                std::iter::once(&image).chain(&linked).collect();
            let carried = lodestone_image::container::carried_objects(&images)
                .map_err(bad_request)?
                .into_keys()
                .collect();
            let plan = import::plan_checked(
                &image,
                &linked,
                inspected.properties.as_deref(),
                &carried,
                &plan_request,
                caps,
                &inspected.credential_files,
            )?;
            let needed = plan.needed_bytes(&inspected.container);
            Ok((inspected.container, plan, image.game.clone(), needed))
        })
        .await?
    };
    if plan.needs_global && !caps.may_write_global {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "This file has protected files (such as .jar files in the mods folder), so making a server from it needs permission to change global files"
            ),
        });
    }
    let flavour = state
        .server_setup
        .resolve(&game.minecraft, plan.kind, game.loader_version.as_deref())
        .await?;
    {
        // what this plan (the world picked) writes, not the summary's default, nor the
        // space the core's other large writes still need (placing checks again, one at a
        // time, holding its own share)
        let needed = needed.saturating_add(state.images.large_writes.total());
        crate::images::blocking(move || {
            require_free_space(path_to_instances(), needed, "to make the server")
        })
        .await?;
    }
    // the pack the file is built on, when Modrinth knows it (best effort)
    // (the file named the version: only an id goes into Modrinth's API path)
    let modpack = match &plan.image.base {
        Some(lodestone_image::Base::ModrinthPack { version, .. })
            if crate::modrinth::links::check_api_segment(version, "version id").is_ok() =>
        {
            crate::modpack::ModpackOrigin::from_modrinth(state.mods.modrinth.as_ref(), version)
                .await
                .ok()
        }
        _ => None,
    };
    let setup_config = SetupConfig {
        name: request.name.clone(),
        version: game.minecraft.clone(),
        flavour: flavour.clone(),
        port: request.port,
        cmd_args: Vec::new(),
        description: request.description.clone(),
        min_ram: request.min_ram,
        max_ram: request.max_ram,
        auto_start: None,
        restart_on_crash: None,
        backup_period: None,
        modpack,
        accept_eula: request.accept_eula,
    };
    let title = format!(
        "Setting up Minecraft server {} from {}",
        request.name,
        lodestone_image::display_text(&upload.file_name)
    );
    let state_for_setup = state.clone();
    let post_setup: PostSetupStep = Box::new(move |root, event_id, uuid| {
        Box::pin(async move {
            let state = state_for_setup;
            let broadcaster = state.event_broadcaster.clone();
            let progress = |message: &str, step: f64| {
                broadcaster.send(Event::new_progression_event_update(
                    &event_id, message, step,
                ));
            };
            let dropped = plan.dropped.clone();
            let file_name = upload.file_name.clone();
            let placed = import::place(
                &state,
                container,
                plan,
                import::NewServer {
                    uuid: &uuid,
                    root: &root,
                    version: &game.minecraft,
                    flavour: &flavour,
                    port: request.port,
                    file_name: &file_name,
                },
                &progress,
            )
            .await?;
            progress("Placed the file's files", IMPORT_PROGRESS);
            // the upload is done with: its file goes once this last clone is dropped
            state
                .image_uploads
                .remove(&request.upload_id, &upload.uploader);
            drop(upload);
            drop(in_use);
            Ok(import::describe(&dropped, &placed))
        })
    });
    let uuid = spawn_minecraft_instance_creation(
        state,
        &requester,
        setup_config,
        title,
        10.0 + IMPORT_PROGRESS,
        Some(post_setup),
    )
    .await?;
    Ok(Json(uuid))
}

pub fn get_instance_image_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/image/upload",
            post(upload_image).layer(DefaultBodyLimit::disable()),
        )
        .route("/instance/image/upload/{upload_id}", delete(discard_upload))
        .route("/instance/create_from_image", post(create_from_image))
        .with_state(state)
}
