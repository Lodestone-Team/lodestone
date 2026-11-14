use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
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
    backups::store::require_free_space,
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

/// The most zipped folders one user's download links may hold at once. Each is a zip of a
/// whole folder (maybe a whole server, many GB) kept in the temporary folder for
/// [`DOWNLOAD_LINK_TTL`], so without a cap anyone who may read a server's files could fill
/// the disk by asking for links again and again. Asking again for a folder that hasn't
/// changed reuses its zip, which doesn't count again.
pub(super) const MAX_ZIPPED_FOLDERS_PER_USER: usize = 5;

/// A file behind a download link.
pub struct DownloadableFile {
    path: PathBuf,
    /// For a file made for the link (a backup's or a world's zip, say), the temporary
    /// folder that holds it. Dropping it deletes the file.
    temp_dir: Option<TempDir>,
    /// For a zipped folder, the zip, which other links for the same folder may share.
    /// It is deleted once the last of them is dropped.
    zip: Option<Arc<ZippedFolder>>,
    /// The user who made the link, as their request authenticated (session or API key).
    owner: User,
    scope: DownloadScope,
    expires_at: Instant,
}

/// Drops a download entry. Deleting a zipped folder can take a while, so it runs on a
/// blocking thread.
fn discard_download(file: DownloadableFile) {
    if file.temp_dir.is_some() || file.zip.is_some() {
        tokio::task::spawn_blocking(move || drop(file));
    }
}

/// Stores a download link for `path` and returns its key. The link expires after
/// `DOWNLOAD_LINK_TTL`, and its temporary file (if any) is deleted then.
pub(super) async fn register_download(
    state: &AppState,
    path: PathBuf,
    temp_dir: Option<TempDir>,
    owner: User,
    scope: DownloadScope,
) -> String {
    insert_download(
        state,
        DownloadableFile {
            path,
            temp_dir,
            zip: None,
            owner,
            scope,
            expires_at: Instant::now() + DOWNLOAD_LINK_TTL,
        },
    )
    .await
}

/// Stores a download link for a folder zipped by [`zip_folder_for_download`], like
/// [`register_download`].
pub(super) async fn register_zipped_folder_download(
    state: &AppState,
    zipped: ZippedDownload,
    owner: User,
    scope: DownloadScope,
) -> String {
    let ZippedDownload { zip, slot } = zipped;
    let key = insert_download(
        state,
        DownloadableFile {
            path: zip.zip_path.clone(),
            temp_dir: None,
            zip: Some(zip),
            owner,
            scope,
            expires_at: Instant::now() + DOWNLOAD_LINK_TTL,
        },
    )
    .await;
    // the link holds the zip now, and counts for its owner from here on
    drop(slot);
    key
}

async fn insert_download(state: &AppState, file: DownloadableFile) -> String {
    let key = rand_alphanumeric(32);
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

/// A folder zipped for download links, in a temporary folder of its own that is deleted
/// with it.
pub(super) struct ZippedFolder {
    zip_path: PathBuf,
    source: PathBuf,
    fingerprint: FolderFingerprint,
    _temp_dir: TempDir,
}

/// What a folder held when it was zipped: how many entries and file bytes, and when the
/// last of its entries changed. Changing, adding or removing anything in it changes a
/// file's or a folder's modification time, so a folder with the same fingerprint still
/// has what its zip has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FolderFingerprint {
    entries: u64,
    bytes: u64,
    latest_change: Option<SystemTime>,
}

impl FolderFingerprint {
    /// Walks `path` as zipping does, without following symbolic links.
    fn of(path: &std::path::Path) -> Result<FolderFingerprint, Error> {
        let mut fingerprint = FolderFingerprint {
            entries: 0,
            bytes: 0,
            latest_change: None,
        };
        for entry in walkdir::WalkDir::new(path) {
            let entry =
                entry.context(format!("Failed to read the contents of {}", path.display()))?;
            let metadata = entry
                .metadata()
                .context(format!("Failed to read the contents of {}", path.display()))?;
            fingerprint.entries += 1;
            if metadata.is_file() {
                fingerprint.bytes = fingerprint.bytes.saturating_add(metadata.len());
            }
            if let Ok(modified) = metadata.modified() {
                fingerprint.latest_change = fingerprint.latest_change.max(Some(modified));
            }
        }
        Ok(fingerprint)
    }
}

/// The number of folders each user is zipping for download right now, by user id, so a
/// user's requests at the same time can't all pass [`MAX_ZIPPED_FOLDERS_PER_USER`] before
/// any of them holds its zip. A place is checked for and taken only while `download_urls`
/// is locked, and given back when its [`ZippingSlot`] is dropped: once the zip's link is
/// stored, or once the zip ends when it failed or its request was cancelled.
static ZIPPING_FOR_DOWNLOAD: LazyLock<std::sync::Mutex<HashMap<String, usize>>> =
    LazyLock::new(Default::default);

/// One of a user's places in [`ZIPPING_FOR_DOWNLOAD`], given back when dropped.
pub(super) struct ZippingSlot {
    user: String,
}

impl ZippingSlot {
    fn count(user: &str) -> usize {
        let zipping = ZIPPING_FOR_DOWNLOAD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        zipping.get(user).copied().unwrap_or(0)
    }

    fn take(user: &str) -> ZippingSlot {
        let mut zipping = ZIPPING_FOR_DOWNLOAD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *zipping.entry(user.to_string()).or_default() += 1;
        ZippingSlot {
            user: user.to_string(),
        }
    }
}

impl Drop for ZippingSlot {
    fn drop(&mut self) {
        let mut zipping = ZIPPING_FOR_DOWNLOAD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(count) = zipping.get_mut(&self.user) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                zipping.remove(&self.user);
            }
        }
    }
}

/// A zipped folder ready for [`register_zipped_folder_download`]. A new zip keeps its
/// user's place among the folders being zipped until its link is stored.
pub(super) struct ZippedDownload {
    zip: Arc<ZippedFolder>,
    slot: Option<ZippingSlot>,
}

impl ZippedDownload {
    pub(super) fn zip_path(&self) -> &std::path::Path {
        &self.zip.zip_path
    }
}

/// Zips the folder at `path` for a download link of `requester`, off the async runtime.
/// When a live link already holds a zip of the folder as it is now, that zip is reused.
/// Otherwise the folder is zipped into a new temporary folder, unless the requester's
/// links already hold [`MAX_ZIPPED_FOLDERS_PER_USER`] zips or the disk can't fit the
/// folder's files and still keep its margin free.
pub(super) async fn zip_folder_for_download(
    state: &AppState,
    path: PathBuf,
    requester: &User,
) -> Result<ZippedDownload, Error> {
    let scanned = path.clone();
    let fingerprint = tokio::task::spawn_blocking(move || FolderFingerprint::of(&scanned))
        .await
        .context("Failed to zip file")??;
    let user: &str = requester.uid.as_ref();
    let slot = {
        let download_urls = state.download_urls.lock().await;
        let now = Instant::now();
        let live_zips = || {
            download_urls
                .values()
                .filter(|file| file.expires_at > now)
                .filter_map(|file| Some((file, file.zip.as_ref()?)))
        };
        let reusable = live_zips()
            .find(|(_, zip)| zip.source == path && zip.fingerprint == fingerprint)
            .map(|(_, zip)| zip.clone());
        if let Some(zip) = reusable {
            return Ok(ZippedDownload { zip, slot: None });
        }
        let mut held: Vec<*const ZippedFolder> = live_zips()
            .filter(|(file, _)| file.owner.uid == user)
            .map(|(_, zip)| Arc::as_ptr(zip))
            .collect();
        held.sort();
        held.dedup();
        if held.len() + ZippingSlot::count(user) >= MAX_ZIPPED_FOLDERS_PER_USER {
            return Err(Error {
                kind: ErrorKind::TooManyRequests,
                source: eyre!(
                    "You already have {MAX_ZIPPED_FOLDERS_PER_USER} zipped folders waiting to be downloaded. Their links expire an hour after they were made; try again then"
                ),
            });
        }
        ZippingSlot::take(user)
    };

    // The slot goes with the zip onto its blocking thread, which runs to the end even when
    // the request is cancelled (the client went away): the place stays taken until then,
    // so cancelled requests can't leave any number of zips being made.
    let (zip, slot) = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        drop(tests::ZIP_GATE.lock());
        let lodestone_tmp = path_to_tmp();
        std::fs::create_dir_all(lodestone_tmp).context(format!(
            "Failed to create temporary directory {}",
            lodestone_tmp.display()
        ))?;
        // a world's files hardly compress, so the zip needs about as much room as they use,
        // on top of what the other zips being made still need
        let reserved = SpaceForZip::reserve(fingerprint.bytes);
        require_free_space(
            lodestone_tmp,
            fingerprint.bytes.saturating_add(reserved.others),
            if reserved.others > 0 {
                "to zip the folder for download, with the other folders being zipped"
            } else {
                "to zip the folder for download"
            },
        )?;
        let temp_dir =
            tempfile::tempdir_in(lodestone_tmp).context("Failed to create temporary folder")?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_string());
        let zip_path = temp_dir.path().join(format!("{name}.zip"));
        zip_files(&[&path], &zip_path, true).context("Failed to zip file")?;
        drop(reserved);
        let zip = ZippedFolder {
            zip_path,
            source: path,
            fingerprint,
            _temp_dir: temp_dir,
        };
        Ok::<_, Error>((zip, slot))
    })
    .await
    .context("Failed to zip file")??;
    Ok(ZippedDownload {
        zip: Arc::new(zip),
        slot: Some(slot),
    })
}

/// The bytes the folders being zipped for download still need, by all users together, so
/// zips made at the same time can't all pass the free space check against the same free
/// space.
static ZIPPING_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A zip's place in [`ZIPPING_BYTES`], given back when dropped.
struct SpaceForZip {
    bytes: u64,
    /// What the other zips being made needed when this one started.
    others: u64,
}

impl SpaceForZip {
    fn reserve(bytes: u64) -> SpaceForZip {
        // folder sizes add up to far less than u64::MAX, so this can't overflow
        let others = ZIPPING_BYTES.fetch_add(bytes, std::sync::atomic::Ordering::SeqCst);
        SpaceForZip { bytes, others }
    }
}

impl Drop for SpaceForZip {
    fn drop(&mut self) {
        ZIPPING_BYTES.fetch_sub(self.bytes, std::sync::atomic::Ordering::SeqCst);
    }
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
pub(crate) async fn reject_protected_change(path: &std::path::Path) -> Result<(), Error> {
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
pub(crate) async fn canonicalize_global_read(
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

    // never replaces an existing file, see `create_new`
    crate::util::fs::create_new(&path).await?;

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
    let (downloadable_file_path, key) = if is_dir {
        let zipped = zip_folder_for_download(&state, path, &requester).await?;
        let zip_path = zipped.zip_path().to_path_buf();
        let key = register_zipped_folder_download(
            &state,
            zipped,
            requester.clone(),
            DownloadScope::GlobalFile,
        )
        .await;
        (zip_path, key)
    } else {
        let key = register_download(
            &state,
            path.clone(),
            None,
            requester.clone(),
            DownloadScope::GlobalFile,
        )
        .await;
        (path, key)
    };
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

    /// Holding it keeps folders zipped for download from starting, so a test can cancel a
    /// request while its zip is being made.
    pub(super) static ZIP_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A request cancelled while its folder is being zipped (its client went away) keeps
    /// its user's place among the folders being zipped until the zip ends, since the zip
    /// goes on without it (LS-86). Before, the place was given back at once, so a user
    /// could start any number of zips by cancelling requests.
    #[tokio::test]
    // the gate is held across awaits on purpose, to hold the zip back meanwhile
    #[allow(clippy::await_holding_lock)]
    async fn a_cancelled_zip_keeps_its_place_until_it_ends() {
        let lodestone = tempfile::tempdir().unwrap();
        crate::prelude::try_init_paths(lodestone.path().to_owned()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let user = User::new("canceller".into(), "pw", false, false, Default::default());
        let uid: &str = user.uid.as_ref();
        let uid = uid.to_owned();
        let folder = dir.path().join("server");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("level.dat"), "level").unwrap();

        let gate = ZIP_GATE.lock().unwrap_or_else(|e| e.into_inner());
        let request = tokio::spawn({
            let state = state.clone();
            let user = user.clone();
            async move {
                zip_folder_for_download(&state, folder, &user)
                    .await
                    .map(|_| ())
            }
        });
        while ZippingSlot::count(&uid) == 0 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        assert_eq!(ZippingSlot::count(&uid), 1, "the zip still holds its place");

        drop(gate);
        let started = Instant::now();
        while ZippingSlot::count(&uid) != 0 {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the zip never ended"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Asking again for a link to a folder that hasn't changed reuses its zip, a changed
    /// folder is zipped again, and one user's links hold at most
    /// [`MAX_ZIPPED_FOLDERS_PER_USER`] zips, counting those being made (LS-86). Before,
    /// every request zipped the folder again and kept the zip for an hour.
    #[tokio::test]
    async fn zipped_folders_are_reused_and_capped_per_user() {
        let lodestone = tempfile::tempdir().unwrap();
        crate::prelude::try_init_paths(lodestone.path().to_owned()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let viewer = User::new("viewer".into(), "pw", false, false, Default::default());
        let other = User::new("other".into(), "pw", false, false, Default::default());
        let scope = || DownloadScope::GlobalFile;
        let folder = |name: &str| {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.join("world")).unwrap();
            std::fs::write(path.join("world/level.dat"), name).unwrap();
            path
        };
        let link = |key: &str| {
            let state = state.clone();
            let key = key.to_string();
            async move {
                let urls = state.download_urls.lock().await;
                let file = &urls[&key];
                (file.path.clone(), Arc::as_ptr(file.zip.as_ref().unwrap()))
            }
        };

        let server = folder("server");
        let first = zip_folder_for_download(&state, server.clone(), &viewer)
            .await
            .unwrap();
        let first = register_zipped_folder_download(&state, first, viewer.clone(), scope()).await;
        let again = zip_folder_for_download(&state, server.clone(), &viewer)
            .await
            .unwrap();
        assert!(again.slot.is_none(), "a reused zip takes no place");
        let again = register_zipped_folder_download(&state, again, viewer.clone(), scope()).await;
        let (first_zip, first_ptr) = link(&first).await;
        let (again_zip, again_ptr) = link(&again).await;
        assert_eq!(first_ptr, again_ptr);
        assert_eq!(first_zip, again_zip);
        assert!(first_zip.is_file());

        // a changed file gets a new zip
        std::fs::write(server.join("world/level.dat"), "changed level").unwrap();
        let changed = zip_folder_for_download(&state, server.clone(), &viewer)
            .await
            .unwrap();
        let changed =
            register_zipped_folder_download(&state, changed, viewer.clone(), scope()).await;
        assert_ne!(link(&changed).await.1, first_ptr);

        // two zips held; fill up to the cap, with one still being made
        for i in 2..MAX_ZIPPED_FOLDERS_PER_USER - 1 {
            let zipped = zip_folder_for_download(&state, folder(&format!("f{i}")), &viewer)
                .await
                .unwrap();
            register_zipped_folder_download(&state, zipped, viewer.clone(), scope()).await;
        }
        let being_made = ZippingSlot::take(viewer.uid.as_ref());
        let error = zip_folder_for_download(&state, folder("one_more"), &viewer)
            .await
            .err()
            .unwrap();
        assert!(matches!(error.kind, ErrorKind::TooManyRequests), "{error}");
        // a folder whose zip is held is still reused, and other users aren't limited
        zip_folder_for_download(&state, server.clone(), &viewer)
            .await
            .unwrap();
        zip_folder_for_download(&state, folder("theirs"), &other)
            .await
            .unwrap();
        drop(being_made);
        zip_folder_for_download(&state, folder("one_more"), &viewer)
            .await
            .unwrap();

        // the shared zip is deleted with the last link that holds it
        let removed = state.download_urls.lock().await.remove(&first).unwrap();
        drop(removed);
        assert!(first_zip.is_file());
        let removed = state.download_urls.lock().await.remove(&again).unwrap();
        drop(removed);
        assert!(!first_zip.exists());
    }
}
