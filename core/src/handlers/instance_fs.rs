use std::path::{Path as FsPath, PathBuf};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, Path},
    http::{header::CONTENT_LENGTH, HeaderMap},
    routing::{delete, get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tracing::error;
use ts_rs::TS;
use walkdir::WalkDir;

use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::{new_fs_event, Event, FSOperation, FSTarget, ProgressionEndValue, ProgressionEventID},
    prelude::path_to_tmp,
    traits::t_configurable::TConfigurable,
    types::InstanceUuid,
    util::{
        format_byte, format_byte_download, list_dir, resolve_path_conflict, scoped_join_win_safe,
        unzip_file_checked_async, zip_files_async, UnzipEntryCheck, UnzipOption,
    },
    AppState,
};

use super::{
    global_fs::{
        create_upload_file, file_entries, finish_upload, multipart_error, register_download,
        upload_file_name, zip_folder_for_download, DownloadScope, FileEntry, MAX_READ_BYTES,
        MAX_WRITE_BODY_BYTES,
    },
    util::{
        check_code_write, contains_script, decode_base64, is_code_path, is_script_path,
        require_code_write,
    },
};
use crate::global_settings::CodeExecutionSwitch;

/// File extensions that only users who can write global files may create, change, move
/// or delete, because the server, the JVM or Lodestone may run or load them.
const PROTECTED_EXTENSIONS: [&str; 19] = [
    "jar",
    "lua",
    "sh",
    "exe",
    "bat",
    "cmd",
    "msi",
    "lodestone_config",
    "out",
    "inf",
    // scripts, which a plugin runtime may run
    "js",
    "mjs",
    "cjs",
    "ts",
    "ps1",
    "command",
    "dll",
    "so",
    "dylib",
];

/// File names that are protected whatever their extension, because they hold the command
/// line, JVM arguments or configuration Lodestone uses to start the server.
const PROTECTED_FILE_NAMES: [&str; 5] = [
    ".lodestone_config",
    ".lodestone_minecraft_config.json",
    "user_jvm_args.txt",
    "unix_args.txt",
    "win_args.txt",
];

/// Folder names that are protected.
const PROTECTED_DIR_NAMES: [&str; 1] = ["mods"];

/// Returns true if a user without `WriteGlobalFile` must not create, change, move or delete
/// `path`. `is_dir` says whether the path is (or will be) a folder.
///
/// Names are compared the way Windows and macOS resolve them: ignoring case and trailing
/// dots and spaces, so `SERVER.JAR` and `server.jar.` count as `server.jar`. Names that
/// Windows would treat specially, such as alternate data streams (`start.bat::$DATA`) and
/// 8.3 short names (`USER_J~1.TXT`), are always protected because they can point at a
/// protected file under another name. Anything that cannot be checked is protected.
pub(crate) fn is_protected(path: &FsPath, is_dir: bool) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return true;
    };
    let name = name.trim_end_matches(['.', ' ']).to_lowercase();
    if name.is_empty() || name.contains(':') || looks_like_short_name(&name) {
        return true;
    }
    if is_dir {
        return PROTECTED_DIR_NAMES.contains(&name.as_str());
    }
    if PROTECTED_FILE_NAMES.contains(&name.as_str()) {
        return true;
    }
    // a disabled jar (`x.jar.disabled`) is a jar that the Mods tab enables by renaming it
    if let Some(enabled) = name.strip_suffix(".disabled") {
        let enabled = FsPath::new(enabled.trim_end_matches(['.', ' ']));
        if enabled
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| PROTECTED_EXTENSIONS.contains(&ext))
        {
            return true;
        }
    }
    match FsPath::new(&name).extension().and_then(|e| e.to_str()) {
        Some(ext) => PROTECTED_EXTENSIONS.contains(&ext),
        // files without an extension, such as `.lodestone_config` or native binaries
        None => true,
    }
}

/// Folders an in-place restore works in (`.lodestone-restore-staging-…` and
/// `.lodestone-restore-old-…`, see [`crate::backups::restore`]) start with this. The file
/// API never reads or changes them, for anyone: startup recovery moves their contents back
/// into place.
const RESTORE_FOLDER_PREFIX: &str = ".lodestone-restore";

/// Whether `name` (one path part) names a restore's folder ([`RESTORE_FOLDER_PREFIX`]),
/// compared the way Windows and macOS resolve names.
fn names_restore_folder(name: &str) -> bool {
    name.trim_end_matches(['.', ' '])
        .to_lowercase()
        .starts_with(RESTORE_FOLDER_PREFIX)
}

/// Whether `name` (one path part) may be the Windows 8.3 short name of a restore's folder:
/// `LODEST~1` (its first 6 letters, without the leading dot), or `LO1F2A~1` (its first 2
/// and a hash) once there are many alike.
pub(crate) fn may_be_restore_folder_short_name(name: &str) -> bool {
    let name = name.trim_end_matches(['.', ' ']).to_lowercase();
    name.starts_with("lo") && looks_like_short_name(&name)
}

fn restore_folder_error() -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!("This folder belongs to a restore in progress"),
    }
}

/// Fails when a part of `path` names a restore's folder ([`names_restore_folder`]).
fn refuse_restore_folders(path: &FsPath) -> Result<(), Error> {
    let touches = path
        .components()
        .any(|part| part.as_os_str().to_str().is_some_and(names_restore_folder));
    if touches {
        return Err(restore_folder_error());
    }
    Ok(())
}

/// Fails when a part of `path` (inside `root`) is a Windows 8.3 short name that leads to a
/// restore's folder on disk, as `LODEST~1` may. Only such names are looked up, by the long
/// name the disk has for them; a name that isn't there leads nowhere. Short names don't
/// exist elsewhere.
fn refuse_restore_folder_short_names(root: &FsPath, path: &FsPath) -> Result<(), Error> {
    if !cfg!(windows) {
        return Ok(());
    }
    let Ok(inside) = path.strip_prefix(root) else {
        return Ok(());
    };
    let mut at = root.to_path_buf();
    for part in inside.components() {
        let name = part.as_os_str().to_str();
        if let Some(name) = name.filter(|name| may_be_restore_folder_short_name(name)) {
            // without a stream (`LODEST~1::$INDEX_ALLOCATION` opens the folder too)
            let base = name.split(':').next().unwrap_or(name);
            let long_name = std::fs::canonicalize(at.join(base)).ok().and_then(|real| {
                real.file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
            });
            if long_name.is_some_and(|long| names_restore_folder(&long)) {
                return Err(restore_folder_error());
            }
        }
        at.push(part.as_os_str());
    }
    Ok(())
}

/// [`scoped_join_win_safe`] for the instance file routes: also refuses a restore's folders,
/// in the path asked for and in the path it resolves to, also by their short names.
fn instance_join(root: impl AsRef<FsPath>, relative: impl AsRef<FsPath>) -> Result<PathBuf, Error> {
    let root = root.as_ref();
    refuse_restore_folders(relative.as_ref())?;
    let path = scoped_join_win_safe(root, relative)?;
    if let Ok(inside) = path.strip_prefix(root) {
        refuse_restore_folders(inside)?;
    }
    refuse_restore_folder_short_names(root, &path)?;
    Ok(path)
}

/// Joins `relative` to the instance folder `root` like [`instance_join`], except that the
/// last part is kept as it is rather than resolved: the entry itself, not what a symbolic
/// link there points to. Deleting or renaming a link then deletes or renames the link:
/// deleting a link to `world` used to delete the world, and a link pointing out of the
/// instance couldn't be deleted or renamed at all. Returns the entry and the path it
/// resolves to, for the checks (the same path unless the entry is a link).
///
/// Fails for a path that names the instance folder itself, such as `""`, `/`, `.` or
/// `world/..`: moving or zipping to it used to pick a free name next to the instance
/// folder (`<folder>_1`), outside it.
fn instance_join_entry(
    root: impl AsRef<FsPath>,
    relative: impl AsRef<FsPath>,
) -> Result<(PathBuf, PathBuf), Error> {
    let root = root.as_ref();
    let relative = relative.as_ref();
    let resolved = instance_join(root, relative)?;
    let not_an_entry = || Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Choose a file or folder in the server, not the server's folder itself"),
    };
    // `None` when the path ends in `..`, or names no file at all (`""`, `/`, `.`)
    let name = relative.file_name().ok_or_else(not_an_entry)?;
    let parent = instance_join(root, relative.parent().unwrap_or(FsPath::new("")))?;
    let entry = parent.join(name);
    if entry == root || !entry.starts_with(root) {
        return Err(not_an_entry());
    }
    Ok((entry, resolved))
}

/// Whether a user without `WriteGlobalFile` may not delete or rename `entry`, an entry of
/// the instance folder `root` (`metadata` is its own, not following links) that resolves to
/// `target`. A symbolic link is checked as the kind of entry it points to (a link to a
/// folder as a folder), and what it points to is checked too: by the instance's rules when
/// it is in the instance, and by name alone when it is outside, since only the link is
/// deleted or renamed.
async fn is_entry_protected(
    root: &FsPath,
    entry: &FsPath,
    target: &FsPath,
    metadata: &std::fs::Metadata,
) -> bool {
    if !metadata.is_symlink() {
        return is_instance_protected(root, entry, metadata.is_dir());
    }
    let target_is_dir = tokio::fs::metadata(target)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    if is_instance_protected(root, entry, target_is_dir) {
        return true;
    }
    if target.starts_with(root) {
        is_path_protected(root, target).await
    } else {
        is_protected(target, target_is_dir)
    }
}

/// Deletes the symbolic link `path` itself, never what it points to. On Windows a link to a
/// folder (or a junction) is a folder entry that `remove_file` refuses, so that falls back
/// to `remove_dir`, which also deletes only the link.
async fn remove_link(path: &FsPath) -> std::io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(e) if cfg!(windows) => tokio::fs::remove_dir(path).await.map_err(|_| e),
        result => result,
    }
}

/// Deletes the entry `relative` of the instance folder `root` for the `rm` (`folder` false)
/// and `rmdir` (`folder` true) routes, and returns its path and whether it was a folder. A
/// symbolic link is deleted itself by either route, since the file list shows a link to a
/// folder as a folder; what it points to is left alone.
async fn delete_entry(
    root: &FsPath,
    relative: &str,
    folder: bool,
    can_write_protected: bool,
) -> Result<(PathBuf, bool), Error> {
    let (path, target) = instance_join_entry(root, relative)?;
    let metadata = tokio::fs::symlink_metadata(&path)
        .await
        .context(format!("Failed to find {relative}"))?;
    if metadata.is_symlink() {
        if !can_write_protected && is_entry_protected(root, &path, &target, &metadata).await {
            return Err(protected_error("File extension is protected"));
        }
        remove_link(&path)
            .await
            .context(format!("Failed to remove {relative}"))?;
        return Ok((path, false));
    }
    if folder {
        if !metadata.is_dir() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("{relative} is not a folder"),
            });
        }
        // the folder itself, or anything in it, is protected
        if !can_write_protected && contains_protected_entry(root, path.clone()).await? {
            return Err(protected_error("Directory contains protected files"));
        }
        tokio::fs::remove_dir_all(&path)
            .await
            .context("Failed to remove directory")?;
    } else {
        if metadata.is_dir() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("{relative} is a folder"),
            });
        }
        // if target has a protected extension, or no extension, deny
        if !can_write_protected && is_instance_protected(root, &path, false) {
            return Err(protected_error("File extension is protected"));
        }
        tokio::fs::remove_file(&path)
            .await
            .context(format!("Failed to remove {relative}"))?;
    }
    Ok((path, folder))
}

/// Where moving `source` to `dest` (both entries from [`instance_join_entry`]) puts it:
/// `None` when they are the same path, so there is nothing to do, `dest` when it is free or
/// is only `source` spelled with other letter case, and otherwise a free name next to it
/// (`name_1.ext`). Fails when `dest` is inside the folder `source`.
///
/// On macOS's and Windows's file systems, which ignore case, a new name that only changes
/// letter case (`Server.properties` to `server.properties`) "exists" as the source itself,
/// and picking a free name made it `server_1.properties`.
fn move_destination(source: &FsPath, dest: &FsPath) -> Result<Option<PathBuf>, Error> {
    if source == dest {
        return Ok(None);
    }
    if dest.starts_with(source) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Destination is a subdirectory of the source"),
        });
    }
    if is_other_case_of(source, dest) {
        return Ok(Some(dest.to_path_buf()));
    }
    // a symbolic link counts as taken even when what it points to doesn't exist
    let taken = |path: &FsPath| std::fs::symlink_metadata(path).is_ok();
    Ok(Some(resolve_path_conflict(
        dest.to_path_buf(),
        Some(&taken),
    )))
}

/// Whether `dest` exists only as `source` under another spelling: both are in the same
/// folder, their names differ only in letter case, and the folder has no entry with
/// exactly `dest`'s name although the file system finds one. On a file system that
/// tells case apart, a `dest` that exists is listed under its own name, so it is never
/// mistaken for the source.
fn is_other_case_of(source: &FsPath, dest: &FsPath) -> bool {
    let (Some(folder), Some(source_name), Some(dest_name)) =
        (dest.parent(), source.file_name(), dest.file_name())
    else {
        return false;
    };
    if source.parent() != Some(folder)
        || source_name == dest_name
        || source_name.to_string_lossy().to_lowercase()
            != dest_name.to_string_lossy().to_lowercase()
        || std::fs::symlink_metadata(dest).is_err()
    {
        return false;
    }
    match std::fs::read_dir(folder) {
        Ok(entries) => !entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() == dest_name),
        Err(_) => false,
    }
}

/// Windows 8.3 short names end the stem with `~` and a digit, as in `PROGRA~1`.
fn looks_like_short_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    stem.rsplit_once('~')
        .is_some_and(|(_, tail)| tail.starts_with(|c: char| c.is_ascii_digit()))
}

/// Whether `name` (one path part) is one of Lodestone's own files or folders in an
/// instance, whose names start with `.lodestone` (`.lodestone_config`, the modpack's
/// `.lodestone_modpack`), compared the way Windows and macOS resolve names. A Windows 8.3
/// short name that may be one of them (`LODEST~1`) counts too.
fn names_lodestone_entry(name: &str) -> bool {
    let trimmed = name.trim_end_matches(['.', ' ']).to_lowercase();
    trimmed.starts_with(".lodestone") || may_be_restore_folder_short_name(&trimmed)
}

/// [`is_protected`] for `path` in the instance folder `root`, which also protects
/// Lodestone's own files there: everything at or below a name starting with `.lodestone`.
/// Lodestone trusts what it wrote there: the modpack's record says which files an update
/// may remove, and its history which files a restore downloads and puts back. Someone who
/// could edit them could have a restore put any jar into `mods/`. A path outside `root`
/// is protected.
pub(crate) fn is_instance_protected(root: &FsPath, path: &FsPath, is_dir: bool) -> bool {
    if is_protected(path, is_dir) {
        return true;
    }
    match path.strip_prefix(root) {
        Ok(inside) => inside
            .components()
            .any(|part| part.as_os_str().to_str().is_none_or(names_lodestone_entry)),
        Err(_) => true,
    }
}

/// Like [`is_instance_protected`], reading from disk whether `path` is a folder. Symbolic
/// links are not followed, so a link is checked as a file.
async fn is_path_protected(root: &FsPath, path: &FsPath) -> bool {
    let is_dir = tokio::fs::symlink_metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    is_instance_protected(root, path, is_dir)
}

/// Returns true if `dir` (in the instance folder `root`) or anything below it is protected,
/// see [`is_instance_protected`]. Runs on a blocking thread and does not follow symbolic
/// links.
async fn contains_protected_entry(root: &FsPath, dir: PathBuf) -> Result<bool, Error> {
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        for entry in WalkDir::new(&dir) {
            let entry =
                entry.context("Failed to walk directory while scanning for protected files")?;
            if is_instance_protected(&root, entry.path(), entry.file_type().is_dir()) {
                return Ok(true);
            }
        }
        Ok(false)
    })
    .await
    .context("Failed to scan directory for protected files")?
}

fn protected_error(message: &str) -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!("{message}"),
    }
}

/// Whether `requester` may change protected files. This is the same permission the other
/// handlers use to guard protected files.
fn can_write_protected(requester: &User) -> bool {
    requester.can_perform_action(&UserAction::WriteGlobalFile)
}

/// Whether creating or changing `path` (inside the instance folder `root`) creates or
/// changes code, see [`is_code_path`]. A path outside `root` counts as code.
fn is_instance_code_path(root: &FsPath, path: &FsPath, is_dir: bool) -> bool {
    match path.strip_prefix(root) {
        Ok(relative_path) => is_code_path(relative_path, is_dir),
        Err(_) => true,
    }
}

/// The instance's folder with its symlinks resolved, as `scoped_join` resolves the
/// paths it joins. Otherwise a folder reached through a symlink (macOS's temporary
/// folders, `/var` → `/private/var`, or a data folder moved behind a link) doesn't
/// prefix the joined paths: listing it showed nothing, moving failed, and every
/// write counted as code. Not on Windows, where a resolved path takes the `\\?\`
/// form that `scoped_join_win_safe` doesn't expect.
async fn real_root(root: PathBuf) -> PathBuf {
    if cfg!(windows) {
        return root;
    }
    tokio::fs::canonicalize(&root).await.unwrap_or(root)
}

fn fs_target(path: PathBuf, is_dir: bool) -> FSTarget {
    if is_dir {
        FSTarget::Directory(path)
    } else {
        FSTarget::File(path)
    }
}

async fn list_instance_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<FileEntry>>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    requester.try_action(
        &UserAction::ReadInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    if uuid.to_string().starts_with("DOCKER-") {
        let files = state
            .docker_bridge
            .list_files(&uuid, relative_path.into())
            .await?;
        return Ok(Json(files));
    }
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(&root, relative_path)?;

    let ret: Vec<FileEntry> = file_entries(list_dir(&path, None).await?)
        .await?
        .into_iter()
        .filter_map(|mut r| {
            // remove the root path from the file path
            r.path = FsPath::new(&r.path)
                .strip_prefix(&root)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_owned())?;
            // a restore's folders aren't shown either
            refuse_restore_folders(FsPath::new(&r.path)).ok()?;
            Some(r)
        })
        .collect();
    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(ret))
}

async fn read_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::ReadInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    if uuid.to_string().starts_with("DOCKER-") {
        let file = state
            .docker_bridge
            .read_container_file(&uuid, relative_path.into())
            .await?;
        return Ok(file);
    }
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(root, relative_path)?;

    let ret = crate::util::fs::read_to_string_capped(&path, MAX_READ_BYTES).await?;
    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(ret)
}

async fn write_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
    body: Bytes,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let can_write_protected = can_write_protected(&requester);
    if uuid.to_string().starts_with("DOCKER-") {
        // Container paths are `<mount name>/<path in the mount>`, so check them by name:
        // the path asked for, and the path it resolves to (through links in the mount).
        let check = |path: &FsPath| {
            if !can_write_protected && is_instance_protected(FsPath::new(""), path, false) {
                return Err(protected_error(
                    "You don't have permission to write to this file",
                ));
            }
            if is_code_path(path, false) {
                require_code_write(&state, &requester)?;
            }
            Ok(())
        };
        check(FsPath::new(&relative_path))?;
        state
            .docker_bridge
            .write_container_file(&uuid, relative_path.into(), &body, check)
            .await?;
        return Ok(Json(()));
    }
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(&root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !can_write_protected && is_path_protected(&root, &path).await {
        return Err(protected_error(
            "You don't have permission to write to this file",
        ));
    }
    if is_instance_code_path(&root, &path, false) {
        require_code_write(&state, &requester)?;
    }
    let mut file = tokio::fs::File::create(&path)
        .await
        .context("Failed to create file")?;
    file.write_all(&body)
        .await
        .context("Failed to write to file")?;
    file.flush().await.context("Failed to write to file")?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Write,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn make_instance_directory(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(&root, relative_path)?;
    if is_instance_code_path(&root, &path, true) {
        require_code_write(&state, &requester)?;
    }
    // create the file if it doesn't exist
    crate::util::fs::create_dir_all(&path).await?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(()))
}

/// Copies each of `sources` into the folder `dest` under its own name, or a free name next
/// to it (`name_1.ext`) when that is taken, and returns where each went. Everything is
/// copied into a temporary folder in `tmp_root` first and moved into `dest` once all of it
/// copied, so a copy that fails leaves nothing behind.
///
/// The first file or folder that can't be read or written ends the copy with an error that
/// names it. `fs_extra`, used before, retried a file it couldn't read forever, on a thread
/// that never finished. Folders are walked without following symbolic links, and links
/// are skipped, as zipping skips them: a link in a copied folder that pointed out of the
/// instance (into Lodestone's stores, say) had what it pointed to copied in, where it could
/// be read. Other special files, such as sockets and pipes, are skipped too. Sources with
/// the same name (`a/config.yml` and `b/config.yml`) each get their own name instead of
/// the second being dropped.
///
/// `progress` gets the bytes copied so far, the bytes to copy in all and the name of the
/// file being copied: first once with nothing copied yet, then as each file copies.
fn copy_entries_in(
    sources: &[PathBuf],
    dest: &FsPath,
    tmp_root: &FsPath,
    mut progress: impl FnMut(u64, u64, &str),
) -> Result<Vec<PathBuf>, Error> {
    use std::io::{Read, Write};

    // a path as the person copying sees it: from the folder the source is in
    let shown = |source: &FsPath, path: &FsPath| -> String {
        let base = source.parent().unwrap_or(source);
        path.strip_prefix(base)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let walk_error = |source: &FsPath, e: walkdir::Error| -> Error {
        let path = e.path().unwrap_or(source).to_path_buf();
        eyre!(e)
            .wrap_err(format!("Failed to read {}", shown(source, &path)))
            .into()
    };

    let mut names = Vec::with_capacity(sources.len());
    let mut total_bytes = 0_u64;
    for source in sources {
        let name = source.file_name().ok_or_else(|| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Cannot copy {}", source.display()),
        })?;
        names.push(name.to_owned());
        for entry in WalkDir::new(source) {
            let entry = entry.map_err(|e| walk_error(source, e))?;
            if entry.file_type().is_file() {
                total_bytes += entry.metadata().map_err(|e| walk_error(source, e))?.len();
            }
        }
    }
    progress(0, total_bytes, "");

    let tmp_dir = tempfile::tempdir_in(tmp_root).context("Failed to create temporary folder")?;
    let mut copied_bytes = 0_u64;
    let mut buffer = vec![0_u8; 256 * 1024];
    let mut staged = Vec::with_capacity(sources.len());
    for (i, (source, name)) in sources.iter().zip(&names).enumerate() {
        // each source in a folder of its own, so sources with the same name don't meet
        let folder = tmp_dir.path().join(i.to_string());
        std::fs::create_dir(&folder).context("Failed to create temporary folder")?;
        let to = folder.join(name);
        for entry in WalkDir::new(source) {
            let entry = entry.map_err(|e| walk_error(source, e))?;
            let from = entry.path();
            let target = match from.strip_prefix(source) {
                Ok(inside) if inside.as_os_str().is_empty() => to.clone(),
                Ok(inside) => to.join(inside),
                Err(_) => continue,
            };
            let file_type = entry.file_type();
            if file_type.is_dir() {
                std::fs::create_dir(&target)
                    .context(format!("Failed to copy {}", shown(source, from)))?;
            } else if file_type.is_file() {
                let read_error = || format!("Failed to read {}", shown(source, from));
                let write_error = || format!("Failed to copy {}", shown(source, from));
                let mut reader = std::fs::File::open(from).with_context(read_error)?;
                let permissions = reader.metadata().with_context(read_error)?.permissions();
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .with_context(write_error)?;
                let file_name = entry.file_name().to_string_lossy();
                loop {
                    let read = reader.read(&mut buffer).with_context(read_error)?;
                    if read == 0 {
                        break;
                    }
                    writer
                        .write_all(&buffer[..read])
                        .with_context(write_error)?;
                    copied_bytes += read as u64;
                    progress(copied_bytes, total_bytes, &file_name);
                }
                drop(writer);
                // keeps a script runnable, as copying it did before
                std::fs::set_permissions(&target, permissions).with_context(write_error)?;
            }
            // symbolic links and special files are skipped, see above
        }
        staged.push(to);
    }

    let mut copied = Vec::with_capacity(staged.len());
    for (from, name) in staged.iter().zip(&names) {
        // a symbolic link counts as taken even when what it points to doesn't exist
        let taken = |path: &FsPath| std::fs::symlink_metadata(path).is_ok();
        let to = resolve_path_conflict(dest.join(name), Some(&taken));
        std::fs::rename(from, &to).context(format!(
            "Failed to move {} into place",
            name.to_string_lossy()
        ))?;
        copied.push(to);
    }
    Ok(copied)
}

#[derive(Deserialize, TS)]
#[ts(export)]
struct CopyInstanceFileRequest {
    relative_paths_source: Vec<PathBuf>,
    relative_path_dest: PathBuf,
}

async fn copy_instance_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(CopyInstanceFileRequest {
        relative_paths_source,
        relative_path_dest,
    }): Json<CopyInstanceFileRequest>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    // join each path to the root
    let paths_source = relative_paths_source
        .iter()
        .map(|p| instance_join(root.clone(), p))
        .collect::<Result<Vec<_>, _>>()?;

    let path_dest = instance_join(&root, &relative_path_dest)?;

    // copying a script creates code
    for source in &paths_source {
        let is_dir = tokio::fs::symlink_metadata(source)
            .await
            .context(format!("Failed to read {}", source.display()))?
            .is_dir();
        let name = source.file_name().ok_or_else(|| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Cannot copy {}", source.display()),
        })?;
        if is_instance_code_path(&root, &path_dest.join(name), is_dir)
            || (is_dir && contains_script(source.clone()).await?)
        {
            require_code_write(&state, &requester)?;
        }
    }

    if !can_write_protected(&requester) {
        if is_path_protected(&root, &path_dest).await {
            return Err(protected_error(
                "You don't have permission to write to this file",
            ));
        }
        // each source lands in `path_dest` under its own name, so check those names, and
        // the contents of copied folders
        for source in &paths_source {
            let is_dir = tokio::fs::symlink_metadata(source)
                .await
                .context(format!("Failed to read {}", source.display()))?
                .is_dir();
            let name = source.file_name().ok_or_else(|| Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot copy {}", source.display()),
            })?;
            if is_instance_protected(&root, &path_dest.join(name), is_dir)
                || (is_dir && contains_protected_entry(&root, source.clone()).await?)
            {
                return Err(protected_error(
                    "You don't have permission to copy protected files",
                ));
            }
        }
    }

    // if the destination path is a subdirectory of any of the source paths, deny
    if paths_source.iter().any(|p| path_dest.starts_with(p)) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You can't copy a directory to a subdirectory of itself"),
        });
    }

    let event_broadcaster = state.event_broadcaster.clone();
    let caused_by = requester.caused_by();

    tokio::task::spawn_blocking(move || {
        let mut first = true;
        let mut threshold = 500000_u64;
        let mut last_progression = 0_u64;
        let mut progression_event_id: Option<ProgressionEventID> = None;

        let handle = |copied_bytes: u64, total_bytes: u64, file_name: &str| {
            if first {
                // at least 1, so small copies do not divide by zero below
                threshold = (total_bytes / 100).max(1);
                let (progression_event_start, id) = Event::new_progression_event_start(
                    "Copying files(s)",
                    Some(total_bytes as f64),
                    None,
                    caused_by.clone(),
                );
                event_broadcaster.send(progression_event_start);
                progression_event_id = Some(id);
                first = false;
            } else if let Some(id) = progression_event_id.as_ref() {
                let progression = copied_bytes / threshold;
                if progression > last_progression {
                    let steps = progression - last_progression;
                    last_progression = progression;
                    event_broadcaster.send(Event::new_progression_event_update(
                        id,
                        format!(
                            "Copying file {}, {}",
                            file_name,
                            format_byte_download(copied_bytes, total_bytes)
                        ),
                        (steps * threshold) as f64,
                    ));
                }
            }
        };

        let inner = || -> Result<(), Error> {
            copy_entries_in(&paths_source, &path_dest, &path_to_tmp(), handle)?;
            Ok(())
        };

        let result = inner();
        // the copy can fail before the first progress callback, for example when a source
        // is missing, so make sure there is an event to end
        let event_id = progression_event_id.unwrap_or_else(|| {
            let (start, id) =
                Event::new_progression_event_start("Copying files(s)", None, None, caused_by);
            event_broadcaster.send(start);
            id
        });
        match result {
            Err(e) => {
                error!("Error copying file(s): {}", e);
                event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    false,
                    Some(&format!("Error copying file(s): {}", e)),
                    Some(ProgressionEndValue::FSOperationCompleted {
                        instance_uuid: uuid,
                        success: false,
                        message: format!("Error copying file(s): {}", e),
                    }),
                ));
            }
            Ok(()) => {
                event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    true,
                    None::<&str>,
                    Some(ProgressionEndValue::FSOperationCompleted {
                        instance_uuid: uuid,
                        success: true,
                        message: "File(s) copied successfully".to_string(),
                    }),
                ));
            }
        }
    });
    Ok(Json(()))
}

async fn move_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path_source, base64_relative_path_dest)): Path<(
        InstanceUuid,
        String,
        String,
    )>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path_source = decode_base64(&base64_relative_path_source)?;
    let relative_path_dest = decode_base64(&base64_relative_path_dest)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    // the entries themselves: renaming a symbolic link renames the link
    let (path_source, source_target) = instance_join_entry(&root, relative_path_source)?;
    let (path_dest, _) = instance_join_entry(&root, relative_path_dest)?;

    let relative_path_source = path_source
        .strip_prefix(&root)
        .context("Error stripping prefix")?;
    let relative_path_dest = path_dest
        .strip_prefix(&root)
        .context("Error stripping prefix")?;

    let metadata = tokio::fs::symlink_metadata(&path_source)
        .await
        .context(format!(
            "Error moving file from {} to {}",
            relative_path_source.display(),
            relative_path_dest.display()
        ))?;
    let is_dir = metadata.is_dir();
    let is_link = metadata.is_symlink();
    // a link is checked as what it points to as well, as it was before links were
    // renamed themselves
    let target_is_dir = if is_link {
        tokio::fs::metadata(&source_target)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false)
    } else {
        is_dir
    };

    // moving a script creates code at the destination
    if is_instance_code_path(&root, &path_dest, target_is_dir)
        || (!is_dir && is_script_path(&path_source))
        || (is_link && !target_is_dir && is_script_path(&source_target))
        || (target_is_dir && contains_script(source_target.clone()).await?)
    {
        require_code_write(&state, &requester)?;
    }

    // the destination becomes the same kind of entry as the source. A link moves alone, so
    // what is in the folder it points to stays where it is.
    if !can_write_protected(&requester)
        && (is_entry_protected(&root, &path_source, &source_target, &metadata).await
            || is_instance_protected(&root, &path_dest, target_is_dir)
            || (is_dir && contains_protected_entry(&root, path_source.clone()).await?))
    {
        return Err(protected_error(
            "You don't have permission to write to this file",
        ));
    }

    let Some(path_dest) = move_destination(&path_source, &path_dest)? else {
        // renamed to the name it has
        return Ok(Json(()));
    };

    tokio::fs::rename(&path_source, &path_dest)
        .await
        .context(format!(
            "Error moving file from {} to {}",
            relative_path_source.display(),
            relative_path_dest.display()
        ))?;

    let caused_by = requester.caused_by();

    state.event_broadcaster.send(new_fs_event(
        FSOperation::Move {
            source: path_source,
        },
        fs_target(path_dest, is_dir),
        caused_by,
    ));

    Ok(Json(()))
}

async fn remove_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let (path, _) = delete_entry(
        &root,
        &relative_path,
        false,
        can_write_protected(&requester),
    )
    .await?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn remove_instance_dir(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let (path, is_dir) =
        delete_entry(&root, &relative_path, true, can_write_protected(&requester)).await?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        fs_target(path, is_dir),
        caused_by,
    ));
    Ok(Json(()))
}

async fn new_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(&root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !can_write_protected(&requester) && is_instance_protected(&root, &path, false) {
        return Err(protected_error("File extension is protected"));
    }
    if is_instance_code_path(&root, &path, false) {
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

async fn get_instance_file_url(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::ReadInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path = instance_join(&root, &relative_path)?;

    let is_dir = tokio::fs::metadata(&path)
        .await
        .map_err(|_| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Could not read file metadata"),
        })?
        .is_dir();
    let (downloadable_path, temp_dir) = if is_dir {
        let (start_event, id) = Event::new_progression_event_start(
            format!("Zipping {} for download", relative_path),
            None,
            None,
            requester.caused_by(),
        );
        state.event_broadcaster.send(start_event);
        match zip_folder_for_download(path.clone()).await {
            Ok((zip_path, temp_dir)) => {
                let end_event =
                    Event::new_progression_event_end(id, true, Some("Zipping complete"), None);
                state.event_broadcaster.send(end_event);
                (zip_path, Some(temp_dir))
            }
            Err(e) => {
                let end_event =
                    Event::new_progression_event_end(id, false, Some(e.to_string()), None);
                state.event_broadcaster.send(end_event);
                return Err(e);
            }
        }
    } else {
        (path.clone(), None)
    };

    let key = register_download(
        &state,
        downloadable_path,
        temp_dir,
        requester.clone(),
        DownloadScope::InstanceFile(uuid),
    )
    .await;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(key)
}

async fn upload_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    headers: HeaderMap,
    AuthBearer(token): AuthBearer,
    mut multipart: Multipart,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let can_write_protected = can_write_protected(&requester);
    let caused_by = requester.caused_by();
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path_to_dir = instance_join(&root, relative_path)?;
    if is_instance_code_path(&root, &path_to_dir, true) {
        require_code_write(&state, &requester)?;
    }
    crate::util::fs::create_dir_all(&path_to_dir).await?;

    let total = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());
    let (progression_start_event, event_id) =
        Event::new_progression_event_start("Uploading files", total, None, caused_by.clone());
    state.event_broadcaster.send(progression_start_event);
    // at least 1 byte, so progress below never divides by zero
    let threshold = (total.unwrap_or(500000.0) / 100.0).max(1.0);

    let mut created_files = Vec::new();
    let result: Result<(), Error> = async {
        while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
            let name = upload_file_name(&field)?;
            let path = resolve_path_conflict(instance_join(&path_to_dir, &name)?, None);
            // if the file has a protected extension, or no extension, deny
            if !can_write_protected && is_instance_protected(&root, &path, false) {
                return Err(protected_error("File extension is protected"));
            }
            if is_instance_code_path(&root, &path, false) {
                require_code_write(&state, &requester)?;
            }

            let mut file = create_upload_file(&path).await?;
            created_files.push(path.clone());

            let mut elapsed_bytes = 0_u64;
            let mut last_progression = 0_u64;

            while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                elapsed_bytes += chunk.len() as u64;
                let progression = (elapsed_bytes as f64 / threshold).floor() as u64;
                if progression > last_progression {
                    last_progression = progression;
                    state
                        .event_broadcaster
                        .send(Event::new_progression_event_update(
                            &event_id,
                            if let Some(total) = total {
                                format!(
                                    "Uploading {name}, {}",
                                    format_byte_download(elapsed_bytes, total as u64)
                                )
                            } else {
                                format!("Uploading {name}, {} uploaded", format_byte(elapsed_bytes))
                            },
                            threshold,
                        ));
                }
                file.write_all(&chunk)
                    .await
                    .context(format!("Failed to write chunk of {name}"))?;
            }
            file.flush()
                .await
                .context(format!("Failed to write chunk of {name}"))?;

            state.event_broadcaster.send(new_fs_event(
                FSOperation::Upload,
                FSTarget::File(path),
                caused_by.clone(),
            ));
        }
        Ok(())
    }
    .await;

    finish_upload(
        &state,
        event_id,
        &result,
        &created_files,
        |success, message| {
            Some(ProgressionEndValue::FSOperationCompleted {
                instance_uuid: uuid,
                success,
                message,
            })
        },
    )
    .await;
    result.map(Json)
}

/// Rejects archive entries that a user without `WriteGlobalFile` could not create
/// directly: protected files and folders, and symbolic links (which could point anywhere).
fn check_unzipped_entry(
    relative_path: &FsPath,
    file_type: &std::fs::FileType,
) -> Result<(), Error> {
    if file_type.is_symlink() {
        Err(protected_error(&format!(
            "The archive contains a symbolic link ({})",
            relative_path.display()
        )))
    } else if is_protected(relative_path, file_type.is_dir()) {
        Err(protected_error(&format!(
            "The archive contains a protected file ({})",
            relative_path.display()
        )))
    } else {
        Ok(())
    }
}

/// The folders an archive may be extracted into (see `unzip_file_in`): for the automatic
/// options the archive's folder or a new folder named after it, else the chosen folder.
fn unzip_destinations(path_to_zip_file: &FsPath, unzip_option: &UnzipOption) -> Vec<PathBuf> {
    match unzip_option {
        UnzipOption::ToDir(dir) => vec![dir.clone()],
        UnzipOption::Normal | UnzipOption::Smart | UnzipOption::ToDirectoryWithFileName => {
            let parent = path_to_zip_file
                .parent()
                .map(FsPath::to_path_buf)
                .unwrap_or_default();
            let mut destinations = vec![parent.clone()];
            if let Some(stem) = path_to_zip_file.file_stem() {
                destinations.push(parent.join(stem));
            }
            destinations
        }
    }
}

/// The check for every extracted entry: the protected file rules for users who cannot
/// write protected files, and the code rules (see [`check_code_write`]) for scripts, for
/// every user.
fn code_entry_check(
    root: PathBuf,
    destinations: Vec<PathBuf>,
    can_write_protected: bool,
    is_owner: bool,
    code_execution: CodeExecutionSwitch,
) -> Box<UnzipEntryCheck> {
    Box::new(move |relative_path, file_type| {
        refuse_restore_folders(relative_path)?;
        for dir in &destinations {
            refuse_restore_folder_short_names(&root, &dir.join(relative_path))?;
        }
        let is_dir = file_type.is_dir();
        if !can_write_protected {
            check_unzipped_entry(relative_path, file_type)?;
            // and where it lands: not in Lodestone's own folders
            let lands_protected = destinations
                .iter()
                .any(|dir| is_instance_protected(&root, &dir.join(relative_path), is_dir));
            if lands_protected {
                return Err(protected_error(&format!(
                    "The archive contains a protected file ({})",
                    relative_path.display()
                )));
            }
        }
        let is_code = (!is_dir && is_script_path(relative_path))
            || destinations
                .iter()
                .any(|dir| is_instance_code_path(&root, &dir.join(relative_path), is_dir));
        if is_code {
            check_code_write(is_owner, &code_execution).map_err(|e| Error {
                kind: e.kind,
                source: e.source.wrap_err(format!(
                    "The archive contains a script ({})",
                    relative_path.display()
                )),
            })?;
        }
        Ok(())
    })
}

pub async fn unzip_instance_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
    Json(unzip_option): Json<UnzipOption>,
) -> Result<Json<ProgressionEventID>, Error> {
    let relative_path = decode_base64(&base64_relative_path)?;
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let can_write_protected = can_write_protected(&requester);
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let path_to_zip_file = instance_join(&root, &relative_path)?;

    // a custom destination is relative to the instance root, like every other path here
    let unzip_option = match unzip_option {
        UnzipOption::ToDir(dir) => {
            let dir = instance_join(&root, dir)?;
            if !can_write_protected && is_instance_protected(&root, &dir, true) {
                return Err(protected_error("Destination is protected"));
            }
            UnzipOption::ToDir(dir)
        }
        other => other,
    };
    let destinations = unzip_destinations(&path_to_zip_file, &unzip_option);
    if destinations
        .iter()
        .any(|dir| is_instance_code_path(&root, dir, true))
    {
        require_code_write(&state, &requester)?;
    }
    let check = code_entry_check(
        root,
        destinations,
        can_write_protected,
        requester.is_owner,
        state.code_execution.clone(),
    );
    let event_broadcaster = state.event_broadcaster.clone();
    // created here, so the response can return the task's id
    let (progression_event_start, event_id) = Event::new_progression_event_start(
        format!("Unzipping {relative_path}"),
        None,
        None,
        requester.caused_by(),
    );
    let task_id = event_id.clone();
    tokio::spawn(async move {
        event_broadcaster.send(progression_event_start);

        let result = unzip_file_checked_async(path_to_zip_file, unzip_option, check).await;
        if let Err(e) = result {
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                false,
                Some(&format!("Unzip failed: {}", e)),
                Some(ProgressionEndValue::FSOperationCompleted {
                    instance_uuid: uuid,
                    success: false,
                    message: format!("Unzip {} failed : {e}", relative_path),
                }),
            ));
        } else {
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                true,
                Some("Unzip complete"),
                Some(ProgressionEndValue::FSOperationCompleted {
                    instance_uuid: uuid,
                    success: true,
                    message: format!("Unzipped {relative_path}"),
                }),
            ));
        }
    });

    Ok(Json(task_id))
}

#[derive(Deserialize, TS)]
#[ts(export)]
struct ZipRequest {
    target_relative_paths: Vec<PathBuf>,
    destination_relative_path: PathBuf,
}

async fn zip_instance_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(zip_request): Json<ZipRequest>,
) -> Result<Json<ProgressionEventID>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::WriteInstanceFile(uuid.clone()),
        state.safe_mode(),
    )?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    let root = real_root(instance.path().await).await;
    drop(instance);
    let ZipRequest {
        mut target_relative_paths,
        mut destination_relative_path,
    } = zip_request;

    // apply instance_join to all paths
    for path in &mut target_relative_paths {
        *path = instance_join(&root, &*path)?;
    }
    // never the instance folder itself, which `zip_files_async` would replace with a free
    // name next to it, outside the instance
    (destination_relative_path, _) = instance_join_entry(&root, &destination_relative_path)?;

    if !can_write_protected(&requester)
        && is_instance_protected(&root, &destination_relative_path, false)
    {
        return Err(protected_error("Destination is protected"));
    }
    if is_instance_code_path(&root, &destination_relative_path, false) {
        require_code_write(&state, &requester)?;
    }

    let event_broadcaster = state.event_broadcaster.clone();
    let aggregate_name = {
        let combined_file_name = target_relative_paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy())
            .collect::<Vec<_>>()
            .join(", ");
        if combined_file_name.len() < 100 {
            combined_file_name
        } else {
            format!("{} files", target_relative_paths.len())
        }
    };
    // created here, so the response can return the task's id
    let (progression_start_event, event_id) = Event::new_progression_event_start(
        format!("Zipping {aggregate_name}"),
        None,
        None,
        requester.caused_by(),
    );
    let task_id = event_id.clone();
    tokio::spawn(async move {
        event_broadcaster.send(progression_start_event);

        if let Err(e) =
            zip_files_async(&target_relative_paths, destination_relative_path, false).await
        {
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                false,
                Some(&format!("Zipping failed: {e}")),
                Some(ProgressionEndValue::FSOperationCompleted {
                    instance_uuid: uuid,
                    success: false,
                    message: format!("Zipping {aggregate_name} failed : {e}"),
                }),
            ));
        } else {
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                true,
                Some("Zip complete"),
                Some(ProgressionEndValue::FSOperationCompleted {
                    instance_uuid: uuid,
                    success: true,
                    message: format!("Zipped {aggregate_name}"),
                }),
            ));
        }
    });

    Ok(Json(task_id))
}

pub fn get_instance_fs_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/ls",
            get(list_instance_files),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/read",
            get(read_instance_file),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/write",
            put(write_instance_file).layer(DefaultBodyLimit::max(MAX_WRITE_BODY_BYTES)),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/mkdir",
            put(make_instance_directory),
        )
        .route("/instance/{uuid}/fs/cpr", put(copy_instance_files))
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/move/{base64_relative_path_dest}",
            put(move_instance_file),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/rm",
            delete(remove_instance_file),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/rmdir",
            delete(remove_instance_dir),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/new",
            put(new_instance_file),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/url",
            get(get_instance_file_url),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/upload",
            // uploads are streamed to disk, so they have no size limit
            put(upload_instance_file).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/instance/{uuid}/fs/{base64_relative_path}/unzip",
            put(unzip_instance_file),
        )
        .route("/instance/{uuid}/fs/zip", put(zip_instance_files))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> bool {
        is_protected(FsPath::new(name), false)
    }

    fn dir(name: &str) -> bool {
        is_protected(FsPath::new(name), true)
    }

    #[test]
    fn short_names_of_a_restores_folders() {
        for name in [
            "LODEST~1",
            "lodest~2",
            "LO1F2A~1",
            "LODEST~1.",
            "LODEST~1::$DATA",
        ] {
            assert!(may_be_restore_folder_short_name(name), "{name}");
        }
        for name in ["world", "world~1", "lodestone", "logs", "LOGS~"] {
            assert!(!may_be_restore_folder_short_name(name), "{name}");
        }
    }

    /// A short name is refused only when it leads to a restore's folder on disk, which only
    /// happens on Windows, on volumes that make short names.
    #[test]
    fn a_restores_folder_is_refused_by_its_short_name() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join(".lodestone-restore-old-0123456789abcdef/world"))
            .unwrap();
        std::fs::create_dir_all(root.join("lobby~1")).unwrap();
        // a real long name that only looks like a short one
        assert!(instance_join(root, "lobby~1/a.txt").is_ok());
        assert!(instance_join(root, "LOST~1/a.txt").is_ok(), "not there");
        assert!(instance_join(root, "world/a.txt").is_ok());
        if root.join("LODEST~1").is_dir() {
            for path in ["LODEST~1", "LODEST~1/world/level.dat", "lodest~1./world"] {
                assert!(instance_join(root, path).is_err(), "{path}");
            }
            let check = code_entry_check(
                root.to_path_buf(),
                vec![root.to_path_buf()],
                true,
                true,
                CodeExecutionSwitch::new(true),
            );
            let file_type = std::fs::symlink_metadata(root.join("lobby~1"))
                .unwrap()
                .file_type();
            assert!(check(FsPath::new("LODEST~1/world"), &file_type).is_err());
            assert!(check(FsPath::new("lobby~1"), &file_type).is_ok());
        }
    }

    /// A folder reached through a symlink prefixes the paths joined to it.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_root_is_resolved_as_joined_paths_are() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("config")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let root = real_root(link.clone()).await;
        let joined = scoped_join_win_safe(&root, "config").unwrap();
        assert_eq!(joined.strip_prefix(&root).unwrap(), FsPath::new("config"));
        assert!(!is_instance_code_path(
            &root,
            &root.join("config/a.yml"),
            false
        ));
    }

    /// With the root resolved, symlinks inside the instance still can't lead out of it:
    /// they resolve as if the instance folder were the whole file system. A link to a
    /// code folder or a protected file is checked as its target.
    #[cfg(unix)]
    #[tokio::test]
    async fn symlinks_in_the_instance_stay_inside_and_are_checked_as_their_targets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        let real = temp.path().join("real");
        std::fs::create_dir_all(real.join("macros")).unwrap();
        std::fs::write(real.join("start.sh"), "").unwrap();
        std::fs::write(real.join("prelaunch.ts"), "").unwrap();
        // the instance folder is reached through a link, as /var is on macOS
        let link = temp.path().join("link");
        symlink(&real, &link).unwrap();
        let root = real_root(link).await;
        assert_eq!(root, std::fs::canonicalize(&real).unwrap());

        symlink(&outside, real.join("abs")).unwrap();
        symlink("../outside", real.join("up")).unwrap();
        symlink("../../../../../..", real.join("deep")).unwrap();
        symlink(outside.join("secret.txt"), real.join("notes.txt")).unwrap();
        symlink("macros", real.join("cfg")).unwrap();
        symlink("/macros", real.join("cfg_abs")).unwrap();
        symlink("start.sh", real.join("readme.txt")).unwrap();
        symlink("prelaunch.ts", real.join("boot.txt")).unwrap();

        let outside = std::fs::canonicalize(&outside).unwrap();
        for path in [
            "abs/secret.txt",
            "up/secret.txt",
            "deep/outside/secret.txt",
            "notes.txt",
            "../outside/secret.txt",
            "../../../../outside/secret.txt",
            "/outside/secret.txt",
            outside.join("secret.txt").to_str().unwrap(),
            "cfg/../../outside/secret.txt",
        ] {
            let joined = scoped_join_win_safe(&root, path).unwrap();
            assert!(joined.starts_with(&root), "{path} -> {}", joined.display());
            assert!(!joined.starts_with(&outside), "{path}");
            assert!(
                std::fs::read_to_string(&joined).is_err(),
                "{path} reads the outside file"
            );
        }

        // a link to a folder in the instance is that folder
        for path in ["cfg/a.txt", "cfg_abs/a.txt", "cfg"] {
            let joined = scoped_join_win_safe(&root, path).unwrap();
            assert_eq!(
                joined.strip_prefix(&root).unwrap().iter().next().unwrap(),
                "macros"
            );
        }
        // a harmless name linked to a protected file is that file
        let joined = scoped_join_win_safe(&root, "readme.txt").unwrap();
        assert_eq!(joined, root.join("start.sh"));
        assert!(is_protected(&joined, false));
        // and one linked to a script is that script
        let joined = scoped_join_win_safe(&root, "boot.txt").unwrap();
        assert_eq!(joined, root.join("prelaunch.ts"));
        assert!(is_instance_code_path(&root, &joined, false));
    }

    /// Moving or zipping to the instance folder itself is refused, since it would pick a
    /// free name next to the instance folder, outside it (LS-12).
    #[test]
    fn the_instance_folder_itself_is_not_an_entry() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        std::fs::create_dir_all(root.join("world")).unwrap();
        for path in ["", ".", "/", "./", "world/..", "world/../.", "../.."] {
            assert!(instance_join_entry(&root, path).is_err(), "{path:?}");
            // as it was: the whole path resolves to the root
            assert_eq!(instance_join(&root, path).unwrap(), root, "{path:?}");
        }
        for (path, entry) in [
            ("world", "world"),
            ("world/.", "world"),
            ("world/../level.dat", "level.dat"),
            ("/world/level.dat", "world/level.dat"),
            ("../../world", "world"),
        ] {
            let (joined, resolved) = instance_join_entry(&root, path).unwrap();
            assert_eq!(joined, root.join(entry), "{path:?}");
            assert_eq!(resolved, joined, "{path:?}");
        }
        // and the move picks no name outside it
        assert!(move_destination(&root.join("world"), &root.join("world"))
            .unwrap()
            .is_none());
    }

    /// Deleting or renaming a symbolic link acts on the link, not on what it points to
    /// (LS-14).
    #[cfg(unix)]
    #[tokio::test]
    async fn links_are_deleted_and_renamed_themselves() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(temp.path()).unwrap();
        let root = base.join("server");
        std::fs::create_dir_all(root.join("world")).unwrap();
        std::fs::write(root.join("world/level.dat"), "level").unwrap();
        std::fs::write(root.join("start.sh"), "").unwrap();
        std::fs::create_dir_all(base.join("outside")).unwrap();
        std::fs::write(base.join("outside/keep.txt"), "keep").unwrap();
        symlink("world", root.join("backup_world")).unwrap();
        symlink(base.join("outside"), root.join("out")).unwrap();
        symlink("world/level.dat", root.join("level.txt")).unwrap();
        symlink("start.sh", root.join("readme.txt")).unwrap();

        let (entry, resolved) = instance_join_entry(&root, "backup_world").unwrap();
        assert_eq!(entry, root.join("backup_world"));
        assert_eq!(resolved, root.join("world"));

        // a link to a folder, deleted as a folder: the folder stays
        let (path, is_dir) = delete_entry(&root, "backup_world", true, false)
            .await
            .unwrap();
        assert_eq!((path, is_dir), (root.join("backup_world"), false));
        assert!(std::fs::symlink_metadata(root.join("backup_world")).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("world/level.dat")).unwrap(),
            "level"
        );

        // a link that points out of the instance can be deleted, and only the link goes.
        // What it points to isn't in the instance, so its name is checked as a file's, and
        // a name without an extension is protected.
        assert!(delete_entry(&root, "out", true, false).await.is_err());
        delete_entry(&root, "out", true, true).await.unwrap();
        assert!(std::fs::symlink_metadata(root.join("out")).is_err());
        assert!(base.join("outside/keep.txt").exists());

        // a link to a file, deleted as a file
        delete_entry(&root, "level.txt", false, false)
            .await
            .unwrap();
        assert!(root.join("world/level.dat").exists());

        // a harmless name linked to a protected file is protected as that file
        assert!(delete_entry(&root, "readme.txt", false, false)
            .await
            .is_err());
        delete_entry(&root, "readme.txt", false, true)
            .await
            .unwrap();
        assert!(root.join("start.sh").exists());

        // a real folder is deleted, and still isn't deleted as a file
        assert!(delete_entry(&root, "world", false, true).await.is_err());
        delete_entry(&root, "world", true, true).await.unwrap();
        assert!(!root.join("world").exists());

        // a rename moves the link: its new name is free, so it is used as it is
        symlink("start.sh", root.join("run")).unwrap();
        let (source, _) = instance_join_entry(&root, "run").unwrap();
        let (dest, _) = instance_join_entry(&root, "run2").unwrap();
        assert_eq!(
            move_destination(&source, &dest).unwrap(),
            Some(root.join("run2"))
        );
        // a dangling link is taken too, rather than replaced
        symlink("nowhere", root.join("dangling")).unwrap();
        let (dest, _) = instance_join_entry(&root, "dangling").unwrap();
        assert_eq!(
            move_destination(&source, &dest).unwrap(),
            Some(root.join("dangling_1"))
        );
    }

    /// Renaming to the same name changes nothing, and a rename that only changes letter
    /// case keeps the new name, also where the file system ignores case (LS-31).
    #[test]
    fn renames_to_the_same_name_or_other_case() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        std::fs::write(root.join("Server.properties"), "motd=hi").unwrap();
        let source = root.join("Server.properties");
        assert_eq!(move_destination(&source, &source).unwrap(), None);

        let dest = root.join("server.properties");
        assert_eq!(
            move_destination(&source, &dest).unwrap(),
            Some(dest.clone())
        );
        std::fs::rename(&source, &dest).unwrap();
        let names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["server.properties"]);

        // where both spellings are separate files, the other one isn't replaced
        std::fs::write(root.join("A.txt"), "upper").unwrap();
        if !root.join("a.txt").exists() {
            std::fs::write(root.join("a.txt"), "lower").unwrap();
            assert_eq!(
                move_destination(&root.join("A.txt"), &root.join("a.txt")).unwrap(),
                Some(root.join("a_1.txt"))
            );
        }
        // a different file whose name differs only in case is never "the source"
        std::fs::write(root.join("b.txt"), "b").unwrap();
        assert_eq!(
            move_destination(&root.join("A.txt"), &root.join("b.txt")).unwrap(),
            Some(root.join("b_1.txt"))
        );
        // a folder still can't move into itself
        std::fs::create_dir(root.join("world")).unwrap();
        assert!(move_destination(&root.join("world"), &root.join("world/sub")).is_err());
    }

    /// Copying a folder with a file that can't be read ends with an error naming the file,
    /// and leaves nothing behind (LS-37). `fs_extra` retried it forever.
    #[cfg(unix)]
    #[test]
    fn a_copy_that_cant_read_a_file_fails_and_leaves_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("server");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(root.join("world/region")).unwrap();
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(root.join("world/level.dat"), "level").unwrap();
        let locked = root.join("world/region/r.0.0.mca");
        std::fs::write(&locked, "region").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&locked).is_ok() {
            // running as root, which reads anything
            return;
        }
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let (sources, dest, tmp_root) =
            (vec![root.join("world")], root.join("backup"), tmp.clone());
        std::fs::create_dir_all(&dest).unwrap();
        std::thread::spawn(move || {
            let _ = done_tx.send(copy_entries_in(&sources, &dest, &tmp_root, |_, _, _| {}));
        });
        let result = done_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the copy never finished");
        let error = result.unwrap_err().to_string();
        assert!(error.contains("world/region/r.0.0.mca"), "{error}");
        assert_eq!(std::fs::read_dir(root.join("backup")).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
    }

    /// A copied folder's symbolic links aren't followed: what they point to, maybe outside
    /// the instance, isn't copied in (LS-37).
    #[cfg(unix)]
    #[test]
    fn a_copy_skips_symbolic_links() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("server");
        let tmp = temp.path().join("tmp");
        let stores = temp.path().join("stores");
        std::fs::create_dir_all(root.join("d")).unwrap();
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::create_dir_all(&stores).unwrap();
        std::fs::write(stores.join("users.json"), "secret").unwrap();
        std::fs::write(root.join("d/start.sh"), "echo").unwrap();
        std::fs::set_permissions(
            root.join("d/start.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        symlink(&stores, root.join("d/s")).unwrap();
        symlink(stores.join("users.json"), root.join("d/users.json")).unwrap();

        let mut calls = Vec::new();
        let copied = copy_entries_in(&[root.join("d")], &root, &tmp, |copied, total, _| {
            calls.push((copied, total))
        })
        .unwrap();
        assert_eq!(copied, [root.join("d_1")]);
        assert!(std::fs::symlink_metadata(root.join("d_1/s")).is_err());
        assert!(std::fs::symlink_metadata(root.join("d_1/users.json")).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("d_1/start.sh")).unwrap(),
            "echo"
        );
        // still runnable
        let mode = std::fs::metadata(root.join("d_1/start.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
        // starts with nothing copied, and counts only the real file
        assert_eq!(calls.first(), Some(&(0, 4)));
        assert_eq!(calls.last(), Some(&(4, 4)));
    }

    /// Copying sources that share a name keeps each of them (LS-37).
    #[test]
    fn a_copy_keeps_sources_with_the_same_name() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("server");
        let tmp = temp.path().join("tmp");
        for folder in ["a", "b", "backup"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(root.join("a/config.yml"), "a").unwrap();
        std::fs::write(root.join("b/config.yml"), "b").unwrap();
        let copied = copy_entries_in(
            &[root.join("a/config.yml"), root.join("b/config.yml")],
            &root.join("backup"),
            &tmp,
            |_, _, _| {},
        )
        .unwrap();
        assert_eq!(
            copied,
            [
                root.join("backup/config.yml"),
                root.join("backup/config_1.yml")
            ]
        );
        assert_eq!(std::fs::read_to_string(&copied[0]).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(&copied[1]).unwrap(), "b");
        // and copying a folder to where it is gives the copy a new name
        let copied = copy_entries_in(&[root.join("a")], &root, &tmp, |_, _, _| {}).unwrap();
        assert_eq!(copied, [root.join("a_1")]);
        assert_eq!(
            std::fs::read_to_string(root.join("a_1/config.yml")).unwrap(),
            "a"
        );
    }

    #[test]
    fn test_protected_extensions() {
        assert!(file("server.jar"));
        assert!(file("plugins/worldedit.jar"));
        assert!(file("start.sh"));
        assert!(file("run.bat"));
        assert!(file("macros/hello.ts"));
        assert!(file("macros/hello.js"));
        assert!(file("foo.lodestone_config"));
        assert!(!file("server.properties"));
        assert!(!file("world/level.dat"));
        assert!(!file("logs/latest.log"));
        assert!(!file("config/settings.json"));
        assert!(!file("archive.zip"));
    }

    #[test]
    fn test_protected_ignores_case() {
        assert!(file("SERVER.JAR"));
        assert!(file("Server.Jar"));
        assert!(file("START.SH"));
        assert!(file("USER_JVM_ARGS.TXT"));
        assert!(file(".LODESTONE_MINECRAFT_CONFIG.JSON"));
        assert!(dir("MODS"));
        assert!(dir("Mods"));
    }

    #[test]
    fn test_protected_ignores_trailing_dots_and_spaces() {
        assert!(file("server.jar."));
        assert!(file("server.jar..."));
        assert!(file("server.jar "));
        assert!(file("server.jar. ."));
        assert!(file("user_jvm_args.txt."));
        assert!(dir("mods."));
        assert!(dir("mods "));
        // a name made only of dots or spaces is never allowed
        assert!(file("..."));
        assert!(file(" "));
        // "notes." becomes "notes", which has no extension
        assert!(file("notes."));
    }

    #[test]
    fn test_protected_windows_style_names() {
        // alternate data streams
        assert!(file("start.bat::$DATA"));
        assert!(file("notes.txt:hidden.jar"));
        assert!(file("notes.txt:stream"));
        // 8.3 short names can refer to protected files under another name
        assert!(file("USER_J~1.TXT"));
        assert!(file("SERVER~1.JAR"));
        assert!(file("LODEST~2.JSO"));
        assert!(dir("PROGRA~1"));
        // a tilde that is not a short name is fine
        assert!(!file("backup~old.txt"));
        assert!(!file("notes~.txt"));
    }

    #[test]
    fn test_protected_file_names() {
        assert!(file(".lodestone_config"));
        assert!(file(".lodestone_minecraft_config.json"));
        assert!(file("user_jvm_args.txt"));
        assert!(file(
            "libraries/net/minecraftforge/forge/1.20.1-47.2.0/unix_args.txt"
        ));
        assert!(file(
            "libraries/net/minecraftforge/forge/1.20.1-47.2.0/win_args.txt"
        ));
        assert!(!file("other_args.txt"));
        assert!(!file("lodestone_minecraft_config.json.bak"));
    }

    #[test]
    fn test_protected_missing_extension_and_odd_names() {
        // no extension, as for native binaries and dotfiles
        assert!(file("java"));
        assert!(file(".bashrc"));
        // no file name at all
        assert!(file(""));
        assert!(file(".."));
        assert!(file("/"));
        assert!(dir(".."));
    }

    #[test]
    fn test_protected_dirs() {
        assert!(dir("mods"));
        assert!(dir("instance/mods"));
        assert!(!dir("plugins"));
        assert!(!dir("world"));
        // a folder named like a protected file is only a folder
        assert!(!dir("start.sh"));
        assert!(!dir("config"));
    }

    #[cfg(unix)]
    #[test]
    fn test_protected_non_utf8_name() {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::OsStr::from_bytes(b"\xffname.txt");
        assert!(is_protected(FsPath::new(name), false));
    }

    #[test]
    fn test_check_unzipped_entry() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("notes.txt"), "").unwrap();
        std::fs::write(temp.path().join("evil.jar"), "").unwrap();
        std::fs::create_dir(temp.path().join("mods")).unwrap();
        std::fs::create_dir(temp.path().join("world")).unwrap();
        let file_type = |name: &str| {
            std::fs::symlink_metadata(temp.path().join(name))
                .unwrap()
                .file_type()
        };

        assert!(check_unzipped_entry(FsPath::new("notes.txt"), &file_type("notes.txt")).is_ok());
        assert!(check_unzipped_entry(FsPath::new("world"), &file_type("world")).is_ok());
        assert!(check_unzipped_entry(FsPath::new("evil.jar"), &file_type("evil.jar")).is_err());
        assert!(check_unzipped_entry(FsPath::new("mods"), &file_type("mods")).is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", temp.path().join("link.txt")).unwrap();
            assert!(check_unzipped_entry(FsPath::new("link.txt"), &file_type("link.txt")).is_err());
        }
    }

    #[tokio::test]
    async fn test_contains_protected_entry() {
        let temp = tempfile::tempdir().unwrap();
        let world = temp.path().join("world");
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::write(world.join("level.dat"), "").unwrap();
        let root = temp.path();
        assert!(!contains_protected_entry(root, world.clone()).await.unwrap());

        std::fs::write(world.join("region").join("Payload.JAR"), "").unwrap();
        assert!(contains_protected_entry(root, world.clone()).await.unwrap());

        let modded = temp.path().join("modded");
        std::fs::create_dir_all(modded.join("mods")).unwrap();
        assert!(contains_protected_entry(root, modded).await.unwrap());

        // Lodestone's own folders, and any folder in them
        let modpack = temp.path().join(".lodestone_modpack");
        std::fs::create_dir_all(modpack.join("objects")).unwrap();
        assert!(contains_protected_entry(root, modpack.clone())
            .await
            .unwrap());
        assert!(contains_protected_entry(root, modpack.join("objects"))
            .await
            .unwrap());
        let parent = temp.path().join("old");
        std::fs::create_dir_all(parent.join(".lodestone_modpack")).unwrap();
        assert!(contains_protected_entry(root, parent).await.unwrap());
    }

    /// The Mods tab enables a disabled jar by renaming it, so it is protected as a jar is
    /// (LS-17).
    #[test]
    fn disabled_jars_are_protected() {
        for name in [
            "x.jar.disabled",
            "X.JAR.Disabled",
            "x.jar.disabled.",
            "x.jar..disabled",
        ] {
            assert!(file(name), "{name}");
        }
        assert!(file("mods/x.jar.disabled"));
        for name in ["config.toml.disabled", "notes.txt.disabled"] {
            assert!(!file(name), "{name}");
        }
    }

    /// The modpack's record and history decide what an update removes and what a restore
    /// downloads, so only users who can write protected files may change them (LS-02).
    #[test]
    fn lodestones_own_files_are_protected() {
        let root = FsPath::new("/home/me/.lodestone/instances/a");
        for path in [
            ".lodestone_modpack/history.json",
            ".lodestone_modpack/record.json",
            ".lodestone_modpack/objects/ab/abcdef",
            ".Lodestone_Modpack./history.json",
            "LODEST~1/history.json",
            ".lodestone_config",
            ".lodestone_whatever/notes.txt",
        ] {
            assert!(
                is_instance_protected(root, &root.join(path), false),
                "{path}"
            );
        }
        assert!(is_instance_protected(
            root,
            &root.join(".lodestone_modpack"),
            true
        ));
        // the data folder's own `.lodestone` above the instance doesn't count
        for path in [
            "world/level.dat",
            "config/a.json",
            "notes.txt",
            "lodestone.txt",
        ] {
            assert!(
                !is_instance_protected(root, &root.join(path), false),
                "{path}"
            );
        }
        assert!(!is_instance_protected(root, &root.join("world"), true));
        // the name rules still hold, and a path elsewhere is protected
        assert!(is_instance_protected(root, &root.join("mods/a.jar"), false));
        assert!(is_instance_protected(
            root,
            FsPath::new("/elsewhere/a.txt"),
            false
        ));
        // a container's paths are relative
        assert!(is_instance_protected(
            FsPath::new(""),
            FsPath::new(".lodestone_modpack/history.json"),
            false
        ));
        assert!(!is_instance_protected(
            FsPath::new(""),
            FsPath::new("config/a.json"),
            false
        ));
    }

    #[test]
    fn archives_cant_write_into_lodestones_folders() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::write(root.join("history.json"), "").unwrap();
        let file_type = std::fs::symlink_metadata(root.join("history.json"))
            .unwrap()
            .file_type();
        let restricted = |destination: PathBuf| {
            code_entry_check(
                root.clone(),
                vec![destination],
                false,
                true,
                CodeExecutionSwitch::new(true),
            )
        };
        let into_root = restricted(root.clone());
        assert!(into_root(FsPath::new(".lodestone_modpack/history.json"), &file_type).is_err());
        assert!(into_root(FsPath::new("config/history.json"), &file_type).is_ok());
        let into_modpack = restricted(root.join(".lodestone_modpack"));
        assert!(into_modpack(FsPath::new("history.json"), &file_type).is_err());
    }

    #[test]
    fn test_instance_code_paths() {
        let root = FsPath::new("/lodestone/instances/a");
        // the old macros folder is an ordinary folder now
        assert!(!is_instance_code_path(root, &root.join("macros"), true));
        assert!(!is_instance_code_path(
            root,
            &root.join("macros/x/config.json"),
            false
        ));
        assert!(is_instance_code_path(
            root,
            &root.join("prelaunch.ts"),
            false
        ));
        assert!(is_instance_code_path(root, &root.join("run.ts"), false));
        assert!(!is_instance_code_path(root, &root.join("world"), true));
        assert!(!is_instance_code_path(
            root,
            &root.join("server.properties"),
            false
        ));
        // anything outside the instance counts as code
        assert!(is_instance_code_path(
            root,
            FsPath::new("/elsewhere/a.txt"),
            false
        ));
    }

    #[test]
    fn test_unzip_destinations() {
        let zip = FsPath::new("/root/macros.zip");
        for option in [
            UnzipOption::Normal,
            UnzipOption::Smart,
            UnzipOption::ToDirectoryWithFileName,
        ] {
            assert_eq!(
                unzip_destinations(zip, &option),
                vec![PathBuf::from("/root"), PathBuf::from("/root/macros")]
            );
        }
        assert_eq!(
            unzip_destinations(zip, &UnzipOption::ToDir(PathBuf::from("/root/world"))),
            vec![PathBuf::from("/root/world")]
        );
    }

    #[test]
    fn test_code_entry_check() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("macros")).unwrap();
        std::fs::write(root.join("a.ts"), "").unwrap();
        std::fs::write(root.join("a.txt"), "").unwrap();
        let file_type = |name: &str| {
            std::fs::symlink_metadata(root.join(name))
                .unwrap()
                .file_type()
        };
        let check = |is_owner: bool, enabled: bool, destination: &str| {
            code_entry_check(
                root.clone(),
                vec![root.join(destination)],
                true,
                is_owner,
                CodeExecutionSwitch::new(enabled),
            )
        };

        for (is_owner, enabled) in [(false, true), (true, false), (false, false)] {
            let check = check(is_owner, enabled, "");
            assert!(check(FsPath::new("a.ts"), &file_type("a.ts")).is_err());
            assert!(check(FsPath::new("a.txt"), &file_type("a.txt")).is_ok());
            // the old macros folder is an ordinary folder now
            assert!(check(FsPath::new("macros"), &file_type("macros")).is_ok());
            let into_macros = check_into(&root, is_owner, enabled);
            assert!(into_macros(FsPath::new("a.txt"), &file_type("a.txt")).is_ok());
        }
        let owner = check(true, true, "");
        assert!(owner(FsPath::new("a.ts"), &file_type("a.ts")).is_ok());

        // the protected file rules still apply to users who cannot write protected files
        let restricted = code_entry_check(
            root.clone(),
            vec![root.join("world")],
            false,
            true,
            CodeExecutionSwitch::new(true),
        );
        std::fs::write(root.join("server.jar"), "").unwrap();
        assert!(restricted(FsPath::new("server.jar"), &file_type("server.jar")).is_err());
    }

    fn check_into(root: &FsPath, is_owner: bool, enabled: bool) -> Box<UnzipEntryCheck> {
        code_entry_check(
            root.to_path_buf(),
            vec![root.join("macros")],
            true,
            is_owner,
            CodeExecutionSwitch::new(enabled),
        )
    }
}
