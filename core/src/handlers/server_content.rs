//! A Minecraft server's world and resource pack, for Lodestone Launcher and the dashboard.
//!
//! - `GET /instance/{uuid}/world/download` zips the world (`level-name` in
//!   `server.properties`, and the `<level>_nether` and `<level>_the_end` folders of
//!   Bukkit-style servers) and returns a key for `GET /file/{key}`, like the file API's
//!   folder downloads. Needs `ReadInstanceFile`. Lodestone has no way to pause saving
//!   (`save-off`/`save-all`), so the server must be stopped: a running world can be
//!   half written.
//! - `GET /instance/{uuid}/resource_pack` (needs `ViewInstance`, since players get the
//!   pack when they join anyway) and `PUT` (needs `AccessSetting`) read and write the four
//!   resource pack lines of `server.properties`, keeping every other line.
//! - `POST /instance/{uuid}/resource_pack/hash` downloads a pack URL and returns its SHA-1
//!   (see [`fetch`]). Needs `AccessSetting`.
//!
//! The core does not host pack files: Minecraft downloads server packs without signing
//! in, and the core has no public file route that could serve them safely.

pub mod fetch;
#[cfg(test)]
mod tests;

use std::path::{Component, Path, PathBuf};

use axum::extract::{Path as AxumPath, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::global_fs::{register_download, DownloadScope};
use super::instance::sanitize_instance_dir_name;
use super::util::{get_instance, AuthedUser};
use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{new_fs_event, Event, FSOperation, FSTarget};
use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::{path_to_tmp, GameInstance};
use crate::traits::t_configurable::manifest::ConfigurableValue;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State as InstanceState, TServer};
use crate::types::InstanceUuid;
use crate::util::zip_files;
use crate::AppState;

/// The world folder when `server.properties` has no `level-name`.
pub const DEFAULT_LEVEL_NAME: &str = "world";
/// The folders of the other dimensions on Bukkit-style servers (Paper, Spigot).
pub const DIMENSION_SUFFIXES: [&str; 2] = ["_nether", "_the_end"];
/// The longest URL and prompt accepted for the resource pack lines.
const MAX_URL_CHARS: usize = 2048;
const MAX_PROMPT_CHARS: usize = 2048;

const KEY_URL: &str = "resource-pack";
const KEY_SHA1: &str = "resource-pack-sha1";
const KEY_REQUIRE: &str = "require-resource-pack";
const KEY_PROMPT: &str = "resource-pack-prompt";

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// What `GET /instance/{uuid}/world/download` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WorldDownload {
    /// For `GET /file/{key}`. The link works for an hour, while its creator may still
    /// read the instance's files.
    pub key: String,
    pub file_name: String,
    /// The world's folder in the zip (the last part of `level-name`).
    pub folder: String,
    /// The other dimensions' folders in the zip, such as `world_nether`.
    pub dimension_folders: Vec<String>,
    #[ts(type = "number")]
    pub size: u64,
}

/// The server resource pack lines of `server.properties`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ServerResourcePack {
    /// `resource-pack`; empty for none.
    pub url: String,
    /// `resource-pack-sha1`, lowercase hex; empty when not set.
    pub sha1: String,
    /// `require-resource-pack`.
    pub required: bool,
    /// `resource-pack-prompt` as plain text when it is a JSON string, else as written.
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ResourcePackHashRequest {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ResourcePackHash {
    pub sha1: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// The world folder for `level_name`, relative to the instance: one or more plain folder
/// names separated by `/`. Absolute paths, `..`, `.`, empty parts, backslashes, colons,
/// control characters and names starting with `.` (such as Lodestone's own files) are
/// refused, and so are names that may be the Windows short name of a restore's folder
/// (`LODEST~1`, see [`super::instance_fs::may_be_restore_folder_short_name`]), which
/// would lead to it.
pub fn world_folder(level_name: &str) -> Result<PathBuf, Error> {
    let unsafe_name = || {
        bad_request(format!("The world folder {level_name:?} in server.properties is not a plain folder name, so Lodestone won't read it"))
    };
    if level_name.is_empty()
        || level_name.len() > 255
        || level_name.starts_with('/')
        || level_name
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
    {
        return Err(unsafe_name());
    }
    let mut path = PathBuf::new();
    for part in level_name.split('/') {
        if part.is_empty()
            || part.starts_with('.')
            || part.trim() != part
            || super::instance_fs::may_be_restore_folder_short_name(part)
        {
            return Err(unsafe_name());
        }
        path.push(part);
    }
    // what is left must be plain names only
    if !path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(unsafe_name());
    }
    Ok(path)
}

fn minecraft(instance: &GameInstance) -> Result<&MinecraftInstance, Error> {
    match instance {
        GameInstance::MinecraftInstance(instance) => Ok(instance),
    }
}

/// `dir` if it is a real folder (not a link) inside `root` once links are resolved.
fn real_folder_inside(root: &Path, dir: &Path) -> Result<Option<PathBuf>, Error> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => {}
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(bad_request(format!(
                "{} is a link, so Lodestone won't zip it",
                dir.file_name().unwrap_or_default().to_string_lossy()
            )))
        }
        _ => return Ok(None),
    }
    let root = root
        .canonicalize()
        .context("Failed to resolve the server folder")?;
    let resolved = dir
        .canonicalize()
        .context("Failed to resolve the world folder")?;
    if !resolved.starts_with(&root) || resolved == root {
        return Err(bad_request("The world folder is outside the server folder"));
    }
    Ok(Some(resolved))
}

/// The folders to zip for the world in `root`: the world, then the other dimensions'
/// folders that exist. Fails when there is no world yet.
pub fn world_folders(root: &Path, level_name: &str) -> Result<Vec<PathBuf>, Error> {
    let relative = world_folder(level_name)?;
    let world = root.join(&relative);
    let Some(world) = real_folder_inside(root, &world)? else {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("The server has no world yet. Start it once to create one"),
        });
    };
    if !world.join("level.dat").is_file() {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!(
                "The world folder has no level.dat. Start the server once to create the world"
            ),
        });
    }
    let mut folders = vec![world];
    let name = relative
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for suffix in DIMENSION_SUFFIXES {
        let sibling = root
            .join(&relative)
            .with_file_name(format!("{name}{suffix}"));
        if let Some(dir) = real_folder_inside(root, &sibling)? {
            folders.push(dir);
        }
    }
    Ok(folders)
}

fn must_be_stopped(state: InstanceState) -> Result<(), Error> {
    if state == InstanceState::Stopped || state == InstanceState::Error {
        return Ok(());
    }
    Err(Error {
        kind: ErrorKind::Conflict,
        source: eyre!(
            "Stop the server before downloading its world. Lodestone can't pause saving, and a running server can leave the copy half written"
        ),
    })
}

async fn download_world(
    State(state): State<AppState>,
    AxumPath(uuid): AxumPath<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<WorldDownload>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    let minecraft = minecraft(&instance)?;
    must_be_stopped(instance.state().await)?;
    let level_name = minecraft
        .server_properties()
        .await?
        .get("level-name")
        .map(str::to_string)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_LEVEL_NAME.to_string());
    let root = instance.path().await;
    let name = instance.name().await;

    let (start, event_id) = Event::new_progression_event_start(
        format!("Zipping the world of {name} for download"),
        None,
        None,
        requester.caused_by(),
    );
    state.event_broadcaster.send(start);
    let zipped = async {
        let folders = {
            let (root, level_name) = (root.clone(), level_name.clone());
            tokio::task::spawn_blocking(move || world_folders(&root, &level_name))
                .await
                .context("The world task failed")??
        };
        let names: Vec<String> = folders
            .iter()
            .map(|f| {
                f.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let file_name = format!(
            "{}-{}.zip",
            sanitize_instance_dir_name(&name),
            sanitize_filename::sanitize(&names[0])
        );
        let zip = {
            let file_name = file_name.clone();
            tokio::task::spawn_blocking(move || {
                std::fs::create_dir_all(path_to_tmp())
                    .context("Failed to create the temporary folder")?;
                let temp_dir = tempfile::tempdir_in(path_to_tmp())
                    .context("Failed to create a temporary folder")?;
                let out = temp_dir.path().join(&file_name);
                zip_files(&folders, &out, true)?;
                let size = std::fs::metadata(&out)
                    .context("Failed to read the zip")?
                    .len();
                Ok::<_, Error>((out, temp_dir, size))
            })
            .await
            .context("The zip task failed")??
        };
        Ok::<_, Error>((names, file_name, zip))
    }
    .await;
    let (names, file_name, (out, temp_dir, size)) = match zipped {
        Ok(done) => done,
        Err(e) => {
            state
                .event_broadcaster
                .send(Event::new_progression_event_end(
                    event_id,
                    false,
                    Some(e.source.to_string()),
                    None,
                ));
            return Err(e);
        }
    };
    // the server may have been started while the world was zipped
    if let Err(e) = must_be_stopped(instance.state().await) {
        state
            .event_broadcaster
            .send(Event::new_progression_event_end(
                event_id,
                false,
                Some("The server was started while its world was zipped"),
                None,
            ));
        tokio::task::spawn_blocking(move || drop(temp_dir));
        return Err(e);
    }
    state
        .event_broadcaster
        .send(Event::new_progression_event_end(
            event_id,
            true,
            Some("Zipping complete"),
            None,
        ));
    let key = register_download(
        &state,
        out,
        Some(temp_dir),
        // the whole user, so an API key's scope is checked again when the link is used
        requester.0.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::Directory(root.join(world_folder(&level_name)?)),
        requester.caused_by(),
    ));
    let mut names = names.into_iter();
    Ok(Json(WorldDownload {
        key,
        file_name,
        folder: names.next().unwrap_or_default(),
        dimension_folders: names.collect(),
        size,
    }))
}

/// `resource-pack-prompt` as shown: the text of a JSON string, else the value as is.
fn prompt_from_property(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::String(text)) => text,
        _ => raw.to_string(),
    }
}

/// The value to write for a prompt: JSON text components (objects and arrays) as they
/// are, anything else as a JSON string, which is a plain text component.
fn prompt_to_property(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(serde_json::Value::Object(_) | serde_json::Value::Array(_)) => trimmed.to_string(),
        _ => serde_json::Value::String(trimmed.to_string()).to_string(),
    }
}

/// Reads the pack lines of a `server.properties` document.
pub fn read_resource_pack(
    doc: &lodestone_minecraft::properties::PropertiesDocument,
) -> ServerResourcePack {
    let get = |key: &str| doc.get(key).unwrap_or_default().trim().to_string();
    ServerResourcePack {
        url: get(KEY_URL),
        sha1: get(KEY_SHA1).to_ascii_lowercase(),
        required: get(KEY_REQUIRE).eq_ignore_ascii_case("true"),
        prompt: prompt_from_property(doc.get(KEY_PROMPT).unwrap_or_default()),
    }
}

/// Checks a pack sent by a client and returns it as it will be written. An empty URL
/// clears every line.
pub fn check_resource_pack(pack: &ServerResourcePack) -> Result<ServerResourcePack, Error> {
    let url = pack.url.trim().to_string();
    if url.is_empty() {
        return Ok(ServerResourcePack::default());
    }
    if url.chars().count() > MAX_URL_CHARS || url.chars().any(char::is_whitespace) {
        return Err(bad_request("The resource pack URL is not valid"));
    }
    let parsed =
        url::Url::parse(&url).map_err(|_| bad_request("The resource pack URL is not valid"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(bad_request(
            "The resource pack URL must start with http:// or https://",
        ));
    }
    let sha1 = pack.sha1.trim().to_ascii_lowercase();
    if !sha1.is_empty() && (sha1.len() != 40 || !sha1.chars().all(|c| c.is_ascii_hexdigit())) {
        return Err(bad_request(
            "The SHA-1 must be 40 hexadecimal characters, or empty",
        ));
    }
    if pack.prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(bad_request("The prompt is too long"));
    }
    Ok(ServerResourcePack {
        url,
        sha1,
        required: pack.required,
        prompt: pack.prompt.trim().to_string(),
    })
}

async fn get_resource_pack(
    State(state): State<AppState>,
    AxumPath(uuid): AxumPath<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<ServerResourcePack>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    let doc = minecraft(&instance)?.server_properties().await?;
    Ok(Json(read_resource_pack(&doc)))
}

async fn set_resource_pack(
    State(state): State<AppState>,
    AxumPath(uuid): AxumPath<InstanceUuid>,
    requester: AuthedUser,
    Json(pack): Json<ServerResourcePack>,
) -> Result<Json<ServerResourcePack>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    let minecraft = minecraft(&instance)?;
    let pack = check_resource_pack(&pack)?;
    minecraft
        .set_server_properties(&[
            (KEY_URL, ConfigurableValue::String(pack.url.clone())),
            (KEY_SHA1, ConfigurableValue::String(pack.sha1.clone())),
            (KEY_REQUIRE, ConfigurableValue::Boolean(pack.required)),
            (
                KEY_PROMPT,
                ConfigurableValue::String(prompt_to_property(&pack.prompt)),
            ),
        ])
        .await?;
    let doc = minecraft.server_properties().await?;
    Ok(Json(read_resource_pack(&doc)))
}

async fn hash_resource_pack(
    State(state): State<AppState>,
    AxumPath(uuid): AxumPath<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ResourcePackHashRequest>,
) -> Result<Json<ResourcePackHash>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    minecraft(&instance)?;
    let checked = check_resource_pack(&ServerResourcePack {
        url: request.url,
        ..Default::default()
    })?;
    if checked.url.is_empty() {
        return Err(bad_request("Enter the resource pack's URL"));
    }
    // a pack on the local network is fine for the owner and admins; anyone else may only
    // make the core fetch public addresses
    let allow_private = requester.is_owner || requester.is_admin;
    let hash = fetch::hash_url(&checked.url, allow_private, fetch::MAX_PACK_BYTES).await?;
    Ok(Json(hash))
}

pub fn get_server_content_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/{uuid}/world/download", get(download_world))
        .route(
            "/instance/{uuid}/resource_pack",
            get(get_resource_pack).put(set_resource_pack),
        )
        .route(
            "/instance/{uuid}/resource_pack/hash",
            post(hash_resource_pack),
        )
        .with_state(state)
}
