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
use fs_extra::TransitProcess;
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
    // macros and the generic instance bootstrap (`run.ts`) are run by Deno
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
fn is_protected(path: &FsPath, is_dir: bool) -> bool {
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
    match FsPath::new(&name).extension().and_then(|e| e.to_str()) {
        Some(ext) => PROTECTED_EXTENSIONS.contains(&ext),
        // files without an extension, such as `.lodestone_config` or native binaries
        None => true,
    }
}

/// Windows 8.3 short names end the stem with `~` and a digit, as in `PROGRA~1`.
fn looks_like_short_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    stem.rsplit_once('~')
        .is_some_and(|(_, tail)| tail.starts_with(|c: char| c.is_ascii_digit()))
}

/// Like [`is_protected`], reading from disk whether `path` is a folder. Symbolic links are
/// not followed, so a link is checked as a file.
async fn is_path_protected(path: &FsPath) -> bool {
    let is_dir = tokio::fs::symlink_metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false);
    is_protected(path, is_dir)
}

/// Returns true if `dir` or anything below it is protected. Runs on a blocking thread and
/// does not follow symbolic links.
async fn contains_protected_entry(dir: PathBuf) -> Result<bool, Error> {
    tokio::task::spawn_blocking(move || {
        for entry in WalkDir::new(&dir) {
            let entry =
                entry.context("Failed to walk directory while scanning for protected files")?;
            if is_protected(entry.path(), entry.file_type().is_dir()) {
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, relative_path)?;

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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(root, relative_path)?;

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
        // container paths are not on this disk, so check the name only
        if !can_write_protected && is_protected(FsPath::new(&relative_path), false) {
            return Err(protected_error(
                "You don't have permission to write to this file",
            ));
        }
        if is_code_path(FsPath::new(&relative_path), false) {
            require_code_write(&state, &requester)?;
        }
        state
            .docker_bridge
            .write_container_file(&uuid, relative_path.into(), &body)
            .await?;
        return Ok(Json(()));
    }
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !can_write_protected && is_path_protected(&path).await {
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, relative_path)?;
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    // join each path to the root
    let paths_source = relative_paths_source
        .iter()
        .map(|p| scoped_join_win_safe(root.clone(), p))
        .collect::<Result<Vec<_>, _>>()?;

    let path_dest = scoped_join_win_safe(&root, &relative_path_dest)?;

    // copying a script, or anything into a macros folder, creates code
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
        if is_path_protected(&path_dest).await {
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
            if is_protected(&path_dest.join(name), is_dir)
                || (is_dir && contains_protected_entry(source.clone()).await?)
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

        let handle = |process_info: TransitProcess| {
            if first {
                // at least 1, so small copies do not divide by zero below
                threshold = (process_info.total_bytes / 100).max(1);
                let (progression_event_start, id) = Event::new_progression_event_start(
                    "Copying files(s)",
                    Some(process_info.total_bytes as f64),
                    None,
                    caused_by.clone(),
                );
                event_broadcaster.send(progression_event_start);
                progression_event_id = Some(id);
                first = false;
            } else if let Some(id) = progression_event_id.as_ref() {
                let progression = process_info.copied_bytes / threshold;
                if progression > last_progression {
                    last_progression = progression;
                    event_broadcaster.send(Event::new_progression_event_update(
                        id,
                        format!(
                            "Copying file {}, {}",
                            process_info.file_name,
                            format_byte_download(
                                process_info.copied_bytes,
                                process_info.total_bytes
                            )
                        ),
                        threshold as f64,
                    ));
                }
            }
            fs_extra::dir::TransitProcessResult::SkipAll
        };

        let inner = || -> Result<(), Error> {
            let tmp_dir =
                tempfile::tempdir_in(path_to_tmp()).context("Failed to create temporary file")?;
            let temp_dir_path = tmp_dir.path().to_owned();

            fs_extra::copy_items_with_progress(
                &paths_source,
                &temp_dir_path,
                &fs_extra::dir::CopyOptions::new(),
                handle,
            )
            .context("Failed to copy file(s)")?;

            for entry in std::fs::read_dir(temp_dir_path).context("Failed to read tmp directory")? {
                let entry = entry.context("Failed to read tmp directory")?;
                let dest_path = resolve_path_conflict(path_dest.join(entry.file_name()), None);
                std::fs::rename(entry.path(), dest_path).context("Failed to move file")?;
            }
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path_source = scoped_join_win_safe(&root, relative_path_source)?;
    let path_dest = scoped_join_win_safe(&root, relative_path_dest)?;

    let relative_path_source = path_source
        .strip_prefix(&root)
        .context("Error stripping prefix")?;
    let relative_path_dest = path_dest
        .strip_prefix(&root)
        .context("Error stripping prefix")?;

    let is_dir = tokio::fs::symlink_metadata(&path_source)
        .await
        .context(format!(
            "Error moving file from {} to {}",
            relative_path_source.display(),
            relative_path_dest.display()
        ))?
        .is_dir();

    // moving a script, or anything into a macros folder, creates code at the destination
    if is_instance_code_path(&root, &path_dest, is_dir)
        || (!is_dir && is_script_path(&path_source))
        || (is_dir && contains_script(path_source.clone()).await?)
    {
        require_code_write(&state, &requester)?;
    }

    // the destination becomes the same kind of entry as the source
    if !can_write_protected(&requester)
        && (is_protected(&path_source, is_dir)
            || is_protected(&path_dest, is_dir)
            || (is_dir && contains_protected_entry(path_source.clone()).await?))
    {
        return Err(protected_error(
            "You don't have permission to write to this file",
        ));
    }

    // if the destination is a subdirectory of the source, we reject the request
    if path_dest.starts_with(&path_source) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Destination is a subdirectory of the source"),
        });
    }

    let path_dest = resolve_path_conflict(path_dest.to_owned(), None);

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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(root, &relative_path)?;
    let metadata = tokio::fs::symlink_metadata(&path)
        .await
        .context(format!("Failed to find {relative_path}"))?;
    if metadata.is_dir() {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("{relative_path} is a folder"),
        });
    }
    // if target has a protected extension, or no extension, deny
    if !can_write_protected(&requester) && is_protected(&path, false) {
        return Err(protected_error("File extension is protected"));
    }

    tokio::fs::remove_file(&path)
        .await
        .context(format!("Failed to remove {relative_path}"))?;

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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, relative_path)?;
    if path == root {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Cannot delete instance root"),
        });
    }
    // the folder itself, or anything in it, is protected
    if !can_write_protected(&requester) && contains_protected_entry(path.clone()).await? {
        return Err(protected_error("Directory contains protected files"));
    }

    tokio::fs::remove_dir_all(&path)
        .await
        .context("Failed to remove directory")?;

    let caused_by = requester.caused_by();
    state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::Directory(path),
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !can_write_protected(&requester) && is_protected(&path, false) {
        return Err(protected_error("File extension is protected"));
    }
    if is_instance_code_path(&root, &path, false) {
        require_code_write(&state, &requester)?;
    }

    crate::util::fs::create(&path).await?;

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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path = scoped_join_win_safe(&root, &relative_path)?;

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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path_to_dir = scoped_join_win_safe(&root, relative_path)?;
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
            let path = resolve_path_conflict(scoped_join_win_safe(&path_to_dir, &name)?, None);
            // if the file has a protected extension, or no extension, deny
            if !can_write_protected && is_protected(&path, false) {
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
/// write protected files, and the code rules (see [`check_code_write`]) for scripts and
/// anything that lands in a macros folder, for every user.
fn code_entry_check(
    root: PathBuf,
    destinations: Vec<PathBuf>,
    can_write_protected: bool,
    is_owner: bool,
    code_execution: CodeExecutionSwitch,
) -> Box<UnzipEntryCheck> {
    Box::new(move |relative_path, file_type| {
        if !can_write_protected {
            check_unzipped_entry(relative_path, file_type)?;
        }
        let is_dir = file_type.is_dir();
        let is_code = (!is_dir && is_script_path(relative_path))
            || destinations
                .iter()
                .any(|dir| is_instance_code_path(&root, &dir.join(relative_path), is_dir));
        if is_code {
            check_code_write(is_owner, &code_execution).map_err(|e| Error {
                kind: e.kind,
                source: e.source.wrap_err(format!(
                    "The archive contains a script or macro ({})",
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let path_to_zip_file = scoped_join_win_safe(&root, &relative_path)?;

    // a custom destination is relative to the instance root, like every other path here
    let unzip_option = match unzip_option {
        UnzipOption::ToDir(dir) => {
            let dir = scoped_join_win_safe(&root, dir)?;
            if !can_write_protected && is_protected(&dir, true) {
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
        source: eyre!("Instance not found"),
    })?;
    let root = instance.path().await;
    drop(instance);
    let ZipRequest {
        mut target_relative_paths,
        mut destination_relative_path,
    } = zip_request;

    // apply scoped_join_win_safe to all paths
    for path in &mut target_relative_paths {
        *path = scoped_join_win_safe(&root, &*path)?;
    }
    destination_relative_path = scoped_join_win_safe(&root, &destination_relative_path)?;

    if !can_write_protected(&requester) && is_protected(&destination_relative_path, false) {
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
        assert!(!contains_protected_entry(world.clone()).await.unwrap());

        std::fs::write(world.join("region").join("Payload.JAR"), "").unwrap();
        assert!(contains_protected_entry(world.clone()).await.unwrap());

        let modded = temp.path().join("modded");
        std::fs::create_dir_all(modded.join("mods")).unwrap();
        assert!(contains_protected_entry(modded).await.unwrap());
    }

    #[test]
    fn test_instance_code_paths() {
        let root = FsPath::new("/lodestone/instances/a");
        assert!(is_instance_code_path(root, &root.join("macros"), true));
        assert!(is_instance_code_path(
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
        // "macros" only counts at the top of the instance
        assert!(!is_instance_code_path(
            root,
            &root.join("world/macros/a.txt"),
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
            assert!(check(FsPath::new("macros"), &file_type("macros")).is_err());
            assert!(check(FsPath::new("a.txt"), &file_type("a.txt")).is_ok());
            // a plain file extracted into a macros folder
            let into_macros = check_into(&root, is_owner, enabled);
            assert!(into_macros(FsPath::new("a.txt"), &file_type("a.txt")).is_err());
        }
        let owner = check(true, true, "");
        assert!(owner(FsPath::new("a.ts"), &file_type("a.ts")).is_ok());
        assert!(owner(FsPath::new("macros"), &file_type("macros")).is_ok());
        let owner_into_macros = check_into(&root, true, true);
        assert!(owner_into_macros(FsPath::new("a.txt"), &file_type("a.txt")).is_ok());

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
