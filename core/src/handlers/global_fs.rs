use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderMap,
    },
    response::Response,
    routing::{delete, get, put},
    Json, Router,
};
use axum_auth::AuthBearer;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};

use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use ts_rs::TS;

use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::{new_fs_event, Event, FSOperation, FSTarget, ProgressionEventID},
    prelude::{lodestone_path, path_to_stores, path_to_tmp},
    types::InstanceUuid,
    util::{list_dir, rand_alphanumeric, resolve_path_conflict, zip_files},
    AppState,
};

use super::util::{contains_script, decode_base64, is_script_path, require_code_write};
use tempfile::TempDir;

/// How long a download link from `/fs/.../download` or `/instance/.../url` keeps working.
/// Links are not single-use, because browsers and download managers retry requests.
pub const DOWNLOAD_LINK_TTL: Duration = Duration::from_secs(60 * 60);

/// Largest request body accepted by the file write routes.
pub(super) const MAX_WRITE_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Largest file the read routes return. Bigger files must be downloaded instead.
pub(crate) const MAX_READ_BYTES: u64 = 16 * 1024 * 1024;

/// The permission a download link's creator must still have when the link is used.
#[derive(Clone)]
pub(super) enum DownloadScope {
    GlobalFile,
    InstanceFile(InstanceUuid),
}

impl DownloadScope {
    fn action(&self) -> UserAction {
        match self {
            DownloadScope::GlobalFile => UserAction::ReadGlobalFile,
            DownloadScope::InstanceFile(uuid) => UserAction::ReadInstanceFile(uuid.clone()),
        }
    }
}

/// A file behind a download link.
pub struct DownloadableFile {
    path: PathBuf,
    /// For a zipped folder, the temporary folder that holds the zip.
    /// Dropping it deletes the zip.
    temp_dir: Option<TempDir>,
    /// The user who made the link, as their request authenticated (session or API key).
    owner: User,
    scope: DownloadScope,
    expires_at: Instant,
}

/// Drops a download entry. Deleting a zipped folder can take a while, so it runs on a
/// blocking thread.
fn discard_download(file: DownloadableFile) {
    if file.temp_dir.is_some() {
        tokio::task::spawn_blocking(move || drop(file));
    }
}

/// Stores a download link for `path` and returns its key. The link expires after
/// `DOWNLOAD_LINK_TTL`, and its temporary zip (if any) is deleted then.
pub(super) async fn register_download(
    state: &AppState,
    path: PathBuf,
    temp_dir: Option<TempDir>,
    owner: User,
    scope: DownloadScope,
) -> String {
    let key = rand_alphanumeric(32);
    let file = DownloadableFile {
        path,
        temp_dir,
        owner,
        scope,
        expires_at: Instant::now() + DOWNLOAD_LINK_TTL,
    };
    state.download_urls.lock().await.insert(key.clone(), file);

    let download_urls = state.download_urls.clone();
    let expired_key = key.clone();
    tokio::spawn(async move {
        tokio::time::sleep(DOWNLOAD_LINK_TTL).await;
        let expired = download_urls.lock().await.remove(&expired_key);
        if let Some(file) = expired {
            discard_download(file);
        }
    });
    key
}

/// Zips the folder at `path` into a new temporary folder, off the async runtime.
/// Returns the zip's path and the temporary folder that owns it.
pub(super) async fn zip_folder_for_download(path: PathBuf) -> Result<(PathBuf, TempDir), Error> {
    tokio::task::spawn_blocking(move || {
        let lodestone_tmp = path_to_tmp();
        std::fs::create_dir_all(lodestone_tmp).context(format!(
            "Failed to create temporary directory {}",
            lodestone_tmp.display()
        ))?;
        let temp_dir =
            tempfile::tempdir_in(lodestone_tmp).context("Failed to create temporary folder")?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_string());
        let zip_path = temp_dir.path().join(format!("{name}.zip"));
        zip_files(&[&path], &zip_path, true).context("Failed to zip file")?;
        Ok((zip_path, temp_dir))
    })
    .await
    .context("Failed to zip file")?
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FileType {
    File,
    Directory,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename = "ClientFile")]
#[ts(export)]
pub struct FileEntry {
    pub name: String,
    pub file_stem: String,
    pub extension: Option<String>,
    pub path: String,
    pub size: Option<u64>,
    pub creation_time: Option<u64>,
    pub modification_time: Option<u64>,
    pub file_type: FileType,
}

/// Seconds since the Unix epoch, or `None` if the time is unknown or before 1970.
fn unix_seconds(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

impl From<&std::path::Path> for FileEntry {
    fn from(path: &std::path::Path) -> Self {
        // one stat call for the type, size and times
        let metadata = std::fs::metadata(path).ok();
        let file_type = match &metadata {
            Some(m) if m.is_dir() => FileType::Directory,
            Some(m) if m.is_file() => FileType::File,
            _ => FileType::Unknown,
        };
        Self {
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.to_string_lossy().into_owned(),
            size: metadata.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
            file_stem: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            extension: path.extension().map(|s| s.to_string_lossy().into_owned()),
            // unix timestamp
            // if we cant get the time, return none
            creation_time: metadata.as_ref().and_then(|m| unix_seconds(m.created())),
            modification_time: metadata.as_ref().and_then(|m| unix_seconds(m.modified())),
            file_type,
        }
    }
}

/// Builds a `FileEntry` for each path on a blocking thread, since each one needs a stat call.
pub(super) async fn file_entries(paths: Vec<PathBuf>) -> Result<Vec<FileEntry>, Error> {
    Ok(tokio::task::spawn_blocking(move || {
        paths.iter().map(|p| FileEntry::from(p.as_path())).collect()
    })
    .await
    .context("Failed to read file information")?)
}

/// Returns true if `path` is, or is inside, one of `secrets`. With `include_parents`, a
/// folder that contains one of `secrets` also counts, for operations such as zipping
/// that read everything below the path. All paths must be canonical.
fn touches_lodestone_secret(
    path: &std::path::Path,
    secrets: &[PathBuf],
    include_parents: bool,
) -> bool {
    secrets
        .iter()
        .any(|secret| path.starts_with(secret) || (include_parents && secret.starts_with(path)))
}

/// What the file API never reads or changes, in the data folder `root` with its `stores`
/// folder: `stores` (users.json with every user's password hash and token secret, the API
/// keys), `local_claim`, whose files grant owner access (see [`crate::local_claim`]),
/// `tls`, the web server's certificate and private key, `backups`
/// ([`crate::prelude::path_to_backups`]: every server's backed-up files, and the restore
/// journals that startup recovery trusts), and `playit.toml`.
///
/// Compatibility: cores before Lodestone Connect saved a playit.gg secret key in
/// `playit.toml`. Playit.gg support is gone, but the file can still be on disk (the core
/// never deletes it), so it stays protected.
fn lodestone_secret_paths(root: &std::path::Path, stores: &std::path::Path) -> [PathBuf; 5] {
    [
        stores.to_path_buf(),
        root.join("playit.toml"),
        root.join(crate::local_claim::CLAIM_DIR),
        root.join("tls"),
        root.join(crate::prelude::BACKUPS_DIR),
    ]
}

/// [`lodestone_secret_paths`] of this core, resolved like the paths checked against them
/// (see [`resolve_existing_prefix`]), so a protected path that doesn't exist yet counts
/// too.
async fn resolved_secret_paths() -> Vec<PathBuf> {
    let mut secrets = Vec::new();
    for secret in lodestone_secret_paths(lodestone_path(), path_to_stores()) {
        secrets.push(resolve_existing_prefix(&secret).await.unwrap_or(secret));
    }
    secrets
}

/// Refuses to write, create, move or delete `path` when it is, or is inside, Lodestone's
/// own data ([`lodestone_secret_paths`]), or is a folder holding some of it (such as the
/// data folder itself). Symbolic links are resolved first; a path that can't be checked
/// is refused. This applies to every user, including users with `WriteGlobalFile`.
async fn reject_protected_change(path: &std::path::Path) -> Result<(), Error> {
    let protected = || Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!("Lodestone's own data cannot be changed through the file API"),
    };
    let resolved = resolve_existing_prefix(path).await.ok_or_else(protected)?;
    if touches_lodestone_secret(&resolved, &resolved_secret_paths().await, true) {
        return Err(protected());
    }
    Ok(())
}

/// Canonicalizes `path` and rejects it if it is Lodestone's own data: the `stores` folder,
/// which holds users.json with every user's password hash and token secret and the API
/// keys, the old playit.gg key file, and the `local_claim` folder. This applies to every user, including users with `ReadGlobalFile`.
pub(super) async fn canonicalize_global_read(
    path: &std::path::Path,
    include_parents: bool,
) -> Result<PathBuf, Error> {
    let path = tokio::fs::canonicalize(path)
        .await
        .context(format!("Failed to find {}", path.display()))?;
    let secrets = resolved_secret_paths().await;
    if touches_lodestone_secret(&path, &secrets, include_parents) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Lodestone's own data stores cannot be read through the file API"),
        });
    }
    Ok(path)
}

/// `path` with its longest existing ancestor canonicalized (so symbolic links are resolved)
/// and the rest appended. `None` if the rest is not a plain relative path.
async fn resolve_existing_prefix(path: &std::path::Path) -> Option<PathBuf> {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(resolved) = tokio::fs::canonicalize(existing).await {
            let mut resolved = resolved;
            for name in rest.iter().rev() {
                resolved.push(name);
            }
            return Some(resolved);
        }
        match (existing.parent(), existing.components().next_back()) {
            (Some(parent), Some(std::path::Component::Normal(name))) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return None,
        }
    }
}

/// Whether creating or changing the absolute `path` creates or changes code: a script file.
/// Symbolic links are resolved first; a path that cannot be checked counts as code.
async fn is_global_code_path(path: &std::path::Path, is_dir: bool) -> bool {
    if is_dir {
        return false;
    }
    if is_script_path(path) {
        return true;
    }
    match resolve_existing_prefix(path).await {
        Some(resolved) => is_script_path(&resolved),
        None => true,
    }
}

fn token_error() -> Error {
    Error {
        kind: ErrorKind::Unauthorized,
        source: eyre!("Token error"),
    }
}

async fn list_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<FileEntry>>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;

    requester.try_action(&UserAction::ReadGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    canonicalize_global_read(&path, false).await?;
    let caused_by = requester.caused_by();
    let ret = file_entries(list_dir(&path, None).await?).await?;
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(ret))
}

async fn read_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;

    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::ReadGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    canonicalize_global_read(&path, false).await?;
    let ret = crate::util::fs::read_to_string_capped(&path, MAX_READ_BYTES).await?;
    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(ret)
}

async fn write_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
    body: Bytes,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;

    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    reject_protected_change(&path).await?;
    if is_global_code_path(&path, false).await {
        require_code_write(&state, &requester)?;
    }

    tokio::fs::write(&path, body)
        .await
        .context(format!("Failed to write to file {}", path.display()))?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Write,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn make_directory(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;

    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    reject_protected_change(&path).await?;
    if is_global_code_path(&path, true).await {
        require_code_write(&state, &requester)?;
    }
    tokio::fs::create_dir(&path)
        .await
        .context(format!("Failed to create directory {}", path.display()))?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn move_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((base64_absolute_path_source, base64_absolute_path_dest)): Path<(String, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let path_source = PathBuf::from(decode_base64(&base64_absolute_path_source)?);
    let path_dest = PathBuf::from(decode_base64(&base64_absolute_path_dest)?);

    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;

    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;
    reject_protected_change(&path_source).await?;
    reject_protected_change(&path_dest).await?;

    let is_dir = tokio::fs::symlink_metadata(&path_source)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    // moving a script, or anything into a macros folder, creates code at the destination
    if is_global_code_path(&path_dest, is_dir).await
        || (!is_dir && is_script_path(&path_source))
        || (is_dir && contains_script(path_source.clone()).await?)
    {
        require_code_write(&state, &requester)?;
    }
    crate::util::fs::rename(&path_source, &path_dest).await?;

    let caused_by = requester.caused_by();

    state.event_broadcaster.send(new_fs_event(
        FSOperation::Move {
            source: path_source,
        },
        if is_dir {
            FSTarget::Directory(path_dest)
        } else {
            FSTarget::File(path_dest)
        },
        caused_by,
    ));

    Ok(Json(()))
}

async fn remove_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    reject_protected_change(&path).await?;

    tokio::fs::remove_file(&path)
        .await
        .context(format!("Failed to remove file {}", path.display()))?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn remove_dir(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    reject_protected_change(&path).await?;

    tokio::fs::remove_dir_all(&path)
        .await
        .context(format!("Failed to remove directory {}", path.display()))?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::Directory(path),
        caused_by,
    ));

    Ok(Json(()))
}

async fn new_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path = PathBuf::from(absolute_path);
    reject_protected_change(&path).await?;
    if is_global_code_path(&path, false).await {
        require_code_write(&state, &requester)?;
    }

    tokio::fs::File::create(&path)
        .await
        .context(format!("Failed to create file {}", path.display()))?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::File(path),
        caused_by,
    ));

    Ok(Json(()))
}

async fn download_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;
    requester.try_action(&UserAction::ReadGlobalFile, state.safe_mode())?;
    // a zipped folder contains everything below it, so folders that contain a store are
    // rejected too
    let path = canonicalize_global_read(&PathBuf::from(absolute_path), true).await?;
    let is_dir = tokio::fs::metadata(&path)
        .await
        .context(format!("Failed to read metadata of {}", path.display()))?
        .is_dir();
    let (downloadable_file_path, temp_dir) = if is_dir {
        let (zip_path, temp_dir) = zip_folder_for_download(path).await?;
        (zip_path, Some(temp_dir))
    } else {
        (path, None)
    };

    let key = register_download(
        &state,
        downloadable_file_path.clone(),
        temp_dir,
        requester.clone(),
        DownloadScope::GlobalFile,
    )
    .await;
    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(downloadable_file_path),
        caused_by,
    ));
    Ok(key)
}

/// Ends an upload's progress event, and on failure removes the files the upload created.
pub(super) async fn finish_upload(
    state: &AppState,
    event_id: ProgressionEventID,
    result: &Result<(), Error>,
    created_files: &[PathBuf],
    end_value: impl FnOnce(bool, String) -> Option<crate::events::ProgressionEndValue>,
) {
    match result {
        Ok(()) => {
            state
                .event_broadcaster
                .send(Event::new_progression_event_end(
                    event_id,
                    true,
                    Some("Upload complete"),
                    end_value(true, "File(s) uploaded".to_string()),
                ));
        }
        Err(e) => {
            for path in created_files {
                tokio::fs::remove_file(path).await.ok();
            }
            let message = format!("Upload failed: {}", e.source);
            state
                .event_broadcaster
                .send(Event::new_progression_event_end(
                    event_id,
                    false,
                    Some(&message),
                    end_value(false, message.clone()),
                ));
        }
    }
}

/// Creates a new file for an upload, never replacing an existing one.
pub(super) async fn create_upload_file(path: &std::path::Path) -> Result<tokio::fs::File, Error> {
    Ok(tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .context(format!("Failed to create file {}", path.display()))?)
}

pub(super) fn multipart_error(e: axum::extract::multipart::MultipartError) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Failed to read the upload: {}", e),
    }
}

/// Sanitizes a file name sent by the client so it stays inside the upload folder.
pub(super) fn upload_file_name(
    field: &axum::extract::multipart::Field<'_>,
) -> Result<String, Error> {
    let name = field.file_name().ok_or_else(|| Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Missing file name"),
    })?;
    let name = sanitize_filename::sanitize(name);
    if name.is_empty() {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Invalid file name"),
        });
    }
    Ok(name)
}

async fn upload_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(base64_absolute_path): Path<String>,
    headers: HeaderMap,
    AuthBearer(token): AuthBearer,
    mut multipart: Multipart,
) -> Result<Json<()>, Error> {
    let absolute_path = decode_base64(&base64_absolute_path)?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(token_error)?;

    requester.try_action(&UserAction::WriteGlobalFile, state.safe_mode())?;

    let path_to_dir = PathBuf::from(absolute_path);
    reject_protected_change(&path_to_dir).await?;
    if is_global_code_path(&path_to_dir, true).await {
        require_code_write(&state, &requester)?;
    }

    tokio::fs::create_dir_all(&path_to_dir)
        .await
        .context(format!(
            "Failed to create directory {}",
            path_to_dir.display()
        ))?;

    let total = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    let caused_by = requester.caused_by();
    let (progression_start_event, event_id) =
        Event::new_progression_event_start("Uploading file(s)", total, None, caused_by.clone());
    state.event_broadcaster.send(progression_start_event);

    let mut created_files = Vec::new();
    let result: Result<(), Error> = async {
        while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
            let name = upload_file_name(&field)?;
            let path = resolve_path_conflict(path_to_dir.join(&name), None);
            reject_protected_change(&path).await?;
            if is_global_code_path(&path, false).await {
                require_code_write(&state, &requester)?;
            }
            let mut file = create_upload_file(&path).await?;
            created_files.push(path.clone());

            while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                state
                    .event_broadcaster
                    .send(Event::new_progression_event_update(
                        &event_id,
                        format!("Uploading {name}"),
                        chunk.len() as f64,
                    ));
                file.write_all(&chunk)
                    .await
                    .context(format!("Failed to write to {}", path.display()))?;
            }
            file.flush()
                .await
                .context(format!("Failed to write to {}", path.display()))?;

            state.event_broadcaster.send(new_fs_event(
                FSOperation::Upload,
                FSTarget::File(path),
                caused_by.clone(),
            ));
        }
        Ok(())
    }
    .await;

    finish_upload(&state, event_id, &result, &created_files, |_, _| None).await;
    result.map(Json)
}

/// Builds a `Content-Disposition: attachment` value. Characters that are not printable
/// ASCII, and quotes and backslashes, are replaced so the header is always valid.
fn attachment_header(path: &std::path::Path) -> String {
    let name: String = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string())
        .chars()
        .map(|c| {
            if (c.is_ascii_graphic() || c == ' ') && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("attachment; filename=\"{name}\"")
}

async fn download(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(key): Path<String>,
) -> Result<Response, Error> {
    let not_found = || Error {
        kind: ErrorKind::NotFound,
        source: eyre!("File not found with the download key"),
    };
    // copy what we need out of the map so the lock is not held while awaiting
    let (path, owner, scope) = {
        let mut download_urls = state.download_urls.lock().await;
        match download_urls.get(&key) {
            Some(file) if file.expires_at > Instant::now() => {
                (file.path.clone(), file.owner.clone(), file.scope.clone())
            }
            Some(_) => {
                if let Some(file) = download_urls.remove(&key) {
                    discard_download(file);
                }
                return Err(not_found());
            }
            None => return Err(not_found()),
        }
    };

    // the link only works while the user who made it still exists and may read the file;
    // for a link made with an API key, while the key is valid and its scope allows it
    let owner = state
        .users_manager
        .read()
        .await
        .refresh_user(&owner)
        .ok_or_else(not_found)?;
    owner.try_action(&scope.action(), state.safe_mode())?;

    let file = tokio::fs::File::open(&path)
        .await
        .context(format!("Failed to open file {}", path.display()))?;

    let mut response = Response::builder()
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_DISPOSITION, attachment_header(&path));
    if let Ok(metadata) = file.metadata().await {
        response = response.header(CONTENT_LENGTH, metadata.len());
    }
    let body = Body::from_stream(ReaderStream::new(file));
    Ok(response
        .body(body)
        .context("Failed to build the download response")?)
}

pub fn get_global_fs_routes(state: AppState) -> Router {
    Router::new()
        .route("/fs/{base64_absolute_path}/ls", get(list_files))
        .route("/fs/{base64_absolute_path}/read", get(read_file))
        .route(
            "/fs/{base64_absolute_path}/write",
            put(write_file).layer(DefaultBodyLimit::max(MAX_WRITE_BODY_BYTES)),
        )
        .route("/fs/{base64_absolute_path}/mkdir", put(make_directory))
        .route(
            "/fs/{base64_absolute_path}/move/{base64_relative_path_dest}",
            put(move_file),
        )
        .route("/fs/{base64_absolute_path}/rm", delete(remove_file))
        .route("/fs/{base64_absolute_path}/rmdir", delete(remove_dir))
        .route("/fs/{base64_absolute_path}/new", put(new_file))
        .route("/fs/{base64_absolute_path}/download", get(download_file))
        .route(
            "/fs/{base64_absolute_path}/upload",
            // uploads are streamed to disk, so they have no size limit
            put(upload_file).layer(DefaultBodyLimit::disable()),
        )
        .route("/file/{key}", get(download))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_claims_are_never_read() {
        let root = std::path::Path::new("/home/u/.lodestone");
        let secrets = lodestone_secret_paths(root, &root.join("stores"));
        for path in [
            "/home/u/.lodestone/stores/users.json",
            "/home/u/.lodestone/stores/api_keys.json",
            "/home/u/.lodestone/local_claim/abc/result.json",
            "/home/u/.lodestone/playit.toml",
            "/home/u/.lodestone/tls/key.pem",
            "/home/u/.lodestone/backups/servers/x/restore-journal.json",
            "/home/u/.lodestone/backups/objects/ab/abcd",
        ] {
            assert!(
                touches_lodestone_secret(std::path::Path::new(path), &secrets, false),
                "{path}"
            );
        }
        // zipping the data folder would include them
        assert!(touches_lodestone_secret(root, &secrets, true));
        assert!(!touches_lodestone_secret(
            std::path::Path::new("/home/u/.lodestone/instances/a/world"),
            &secrets,
            true
        ));
    }

    /// What `reject_protected_change` refuses, for writes, creates, deletes and both ends
    /// of a move: anything in a protected folder, the protected folders themselves (even
    /// before they exist), and folders holding them; nothing else.
    #[test]
    fn changes_to_protected_data_are_refused() {
        let root = std::path::Path::new("/home/u/.lodestone");
        let secrets = lodestone_secret_paths(root, &root.join("stores"));
        let refused = |p: &str| touches_lodestone_secret(std::path::Path::new(p), &secrets, true);
        for path in [
            // writing the claim files, or creating the folder before the core does
            "/home/u/.lodestone/local_claim",
            "/home/u/.lodestone/local_claim/abc/claim",
            "/home/u/.lodestone/local_claim/abc/owner.json",
            // the stores and the TLS key
            "/home/u/.lodestone/stores/users.json",
            "/home/u/.lodestone/stores",
            "/home/u/.lodestone/tls/key.pem",
            // moving or deleting a folder that holds them
            "/home/u/.lodestone",
            "/home/u",
            "/",
        ] {
            assert!(refused(path), "{path}");
        }
        for path in [
            // a move's destination elsewhere, and ordinary files in the data folder
            "/home/u/Desktop/local_claim",
            "/home/u/.lodestone/instances/a/world/level.dat",
            "/home/u/.lodestone/notes.txt",
            "/home/u/.lodestone/local_claim_backup",
            "/home/u/.lodestone/stores_old",
        ] {
            assert!(!refused(path), "{path}");
        }
    }

    #[test]
    fn test_touches_lodestone_secret() {
        let secrets = [
            PathBuf::from("/home/u/.lodestone/stores"),
            PathBuf::from("/home/u/.lodestone/playit.toml"),
        ];
        let check =
            |p: &str, parents| touches_lodestone_secret(std::path::Path::new(p), &secrets, parents);

        assert!(check("/home/u/.lodestone/stores", false));
        assert!(check("/home/u/.lodestone/stores/users.json", false));
        assert!(check("/home/u/.lodestone/playit.toml", false));
        assert!(!check(
            "/home/u/.lodestone/instances/a/server.properties",
            false
        ));
        assert!(!check("/home/u/.lodestone/stores_backup", false));
        // listing the Lodestone folder is fine, zipping it is not
        assert!(!check("/home/u/.lodestone", false));
        assert!(check("/home/u/.lodestone", true));
        assert!(check("/", true));
        assert!(!check("/home/u/.lodestone/instances", true));
    }

    #[test]
    fn test_attachment_header() {
        assert_eq!(
            attachment_header(std::path::Path::new("/a/world.zip")),
            "attachment; filename=\"world.zip\""
        );
        assert_eq!(
            attachment_header(std::path::Path::new("/a/we\"ird wörld.zip")),
            "attachment; filename=\"we_ird w_rld.zip\""
        );
        // `\` is part of the name on Unix, and a path separator on Windows
        #[cfg(unix)]
        assert_eq!(
            attachment_header(std::path::Path::new("/a/we\\ird.zip")),
            "attachment; filename=\"we_ird.zip\""
        );
        #[cfg(windows)]
        assert_eq!(
            attachment_header(std::path::Path::new("/a/we\\ird.zip")),
            "attachment; filename=\"ird.zip\""
        );
        assert_eq!(
            attachment_header(std::path::Path::new("/")),
            "attachment; filename=\"unknown\""
        );
    }

    #[test]
    fn test_file_entry_path_is_full_path() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("a.txt");
        std::fs::write(&file, "abc").unwrap();
        let entry = FileEntry::from(file.as_path());
        assert_eq!(entry.path, file.to_string_lossy());
        assert_eq!(entry.name, "a.txt");
        assert_eq!(entry.size, Some(3));
        assert!(matches!(entry.file_type, FileType::File));
        assert!(entry.modification_time.is_some());

        let dir = FileEntry::from(temp.path());
        assert!(matches!(dir.file_type, FileType::Directory));
        assert_eq!(dir.size, None);

        let missing = FileEntry::from(temp.path().join("missing").as_path());
        assert!(matches!(missing.file_type, FileType::Unknown));
        assert_eq!(missing.modification_time, None);
    }

    #[tokio::test]
    async fn test_resolve_existing_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let root = tokio::fs::canonicalize(temp.path()).await.unwrap();
        std::fs::create_dir_all(root.join("instances/a/macros")).unwrap();
        assert_eq!(
            resolve_existing_prefix(&root.join("instances/a/macros/new/prelaunch.ts")).await,
            Some(root.join("instances/a/macros/new/prelaunch.ts"))
        );
        // `..` after a folder that does not exist: Unix can't resolve it, so the path can't
        // be checked. Windows removes `..` by name before it looks at the disk, so the path
        // resolves to the folder that a write would really use.
        #[cfg(unix)]
        assert_eq!(
            resolve_existing_prefix(&root.join("missing/../instances")).await,
            None
        );
        #[cfg(windows)]
        assert_eq!(
            resolve_existing_prefix(&root.join("missing/../instances")).await,
            Some(root.join("instances"))
        );
        #[cfg(unix)]
        {
            // a link into a macros folder resolves to the folder
            std::os::unix::fs::symlink(root.join("instances/a/macros"), root.join("link")).unwrap();
            assert_eq!(
                resolve_existing_prefix(&root.join("link/x.txt")).await,
                Some(root.join("instances/a/macros/x.txt"))
            );
        }
    }
}
