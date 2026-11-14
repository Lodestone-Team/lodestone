//! Restoring a backup: which paths a restore replaces, staging the backup's files next to
//! them, the journaled swap, and repairing an interrupted swap at startup.
//!
//! An in-place restore goes:
//! 1. [`plan`]: the owned paths (what the restore replaces) and the backup's files in them.
//! 2. [`diff`]: what would be added, changed and removed, for the permission check.
//! 3. [`stage`]: the files are written to `<server>/.lodestone-restore-staging-<16>/`,
//!    verified, with the current name, port and command line pinned in the staged config.
//! 4. [`Staged::swap`]: the owned entries are renamed into
//!    `<server>/.lodestone-restore-old-<16>/`, the staged ones into place, and what the
//!    restore never touches (excluded paths, links) back from the old entries into the new
//!    ones. A journal outside the server folder records the swap so a crash in the middle is
//!    undone at the next start ([`recover`]).
//!
//! Everything here is blocking; the caller holds the server's locks.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{Error, ErrorKind};
use crate::types::{DotLodestoneConfig, InstanceUuid};

use super::pattern::{always_excluded, always_excluded_path, Matcher};
use super::store::{
    cancelled, err, join_rel, random_suffix, rename_with_retry, valid_rel_path, valid_server_id,
    Cancel, Store, TEMP_MAX_AGE,
};
use super::types::{FileEntry, Manifest, RestoreScope};

pub const STAGING_PREFIX: &str = ".lodestone-restore-staging-";
pub const OLD_PREFIX: &str = ".lodestone-restore-old-";
/// `<instances>/.lodestone-restore-new-<16>/`: a restore as a new server, until it is
/// complete and renamed to its final folder.
pub const NEW_PREFIX: &str = ".lodestone-restore-new-";
pub const CONFIG_FILE: &str = ".lodestone_minecraft_config.json";
const PROPERTIES_FILE: &str = "server.properties";
/// Where staged copies go when the on-disk version of a path wins over them.
const REPLACED_DIR: &str = ".replaced";

/// How long to keep retrying a rename while Windows reports a file in use (Explorer, a
/// terminal in the world folder, antivirus).
#[cfg(windows)]
pub const SWAP_RENAME_BUDGET: Duration = Duration::from_secs(10);
#[cfg(not(windows))]
pub const SWAP_RENAME_BUDGET: Duration = Duration::ZERO;

fn bad_request(message: impl Into<String>) -> Error {
    err(ErrorKind::BadRequest, message)
}

fn io_error(e: io::Error, what: impl Into<String>) -> Error {
    Error::from(eyre!(e).wrap_err(what.into()))
}

/// Whether `path` is `prefix` or inside it (by whole parts: `world2` isn't in `world`).
fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || (path.len() > prefix.len()
            && path.starts_with(prefix)
            && path.as_bytes()[prefix.len()] == b'/')
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

// ---------------------------------------------------------------------------------------
// What a restore replaces

/// A path the restore replaces as a whole, relative to the server folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPath {
    pub path: String,
    /// There is a real (not linked) file or folder there now.
    pub on_disk: bool,
    /// The backup has it.
    pub in_backup: bool,
}

#[derive(Debug, Clone)]
pub struct RestorePlan {
    pub scope: RestoreScope,
    pub owned: Vec<OwnedPath>,
    /// The backup's files inside the owned paths, without those the restore never touches
    /// (always excluded, or excluded by the current policy). Sorted by path.
    pub files: Vec<FileEntry>,
}

impl RestorePlan {
    /// Whether `path` is one of the owned paths or inside one.
    pub fn owns(&self, path: &str) -> bool {
        self.owned.iter().any(|o| under(path, &o.path))
    }

    /// The bytes staging writes: what the free-space check needs.
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// What restoring `manifest` with `scope` replaces in `server_root`.
///
/// - Everything: the top-level entries of the backup and of the server folder, except the
///   always-excluded ones, those the current policy (`exclude`) excludes, and links, which are
///   never owned (a backup with a real entry where the server has a link is refused), and
///   `.lodestone_minecraft_config.json` when the backup doesn't have it.
/// - World: `<level>`, `<level>_nether` and `<level>_the_end` (nested like `worlds/survival`
///   when `level-name` is), each when the backup or the server has it. On disk each must be a
///   real folder inside the server folder.
pub fn plan(
    server_root: &Path,
    manifest: &Manifest,
    scope: RestoreScope,
    exclude: &Matcher,
) -> Result<RestorePlan, Error> {
    let owned = match scope {
        RestoreScope::Everything => owned_everything(server_root, manifest, exclude)?,
        RestoreScope::World => owned_world(server_root, manifest, exclude)?,
    };
    let mut files: Vec<FileEntry> = manifest
        .files
        .iter()
        .filter(|f| owned.iter().any(|o| under(&f.path, &o.path)))
        .filter(|f| {
            !always_excluded_path(&f.path, false) && !exclude.is_excluded_path(&f.path, false)
        })
        .cloned()
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(RestorePlan {
        scope,
        owned,
        files,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiskKind {
    File,
    Dir,
    Link,
}

fn disk_kind(path: &Path) -> Option<DiskKind> {
    let meta = fs::symlink_metadata(path).ok()?;
    Some(if meta.file_type().is_symlink() {
        DiskKind::Link
    } else if meta.is_dir() {
        DiskKind::Dir
    } else {
        DiskKind::File
    })
}

fn owned_everything(
    server_root: &Path,
    manifest: &Manifest,
    exclude: &Matcher,
) -> Result<Vec<OwnedPath>, Error> {
    // name -> (on disk, in the backup as a folder)
    let mut names: BTreeMap<String, (Option<DiskKind>, Option<bool>)> = BTreeMap::new();
    for file in &manifest.files {
        let (first, rest) = match file.path.split_once('/') {
            Some((first, _)) => (first, true),
            None => (file.path.as_str(), false),
        };
        let entry = names.entry(first.to_string()).or_default();
        entry.1 = Some(entry.1.unwrap_or(false) || rest);
    }
    let entries =
        fs::read_dir(server_root).map_err(|e| io_error(e, "Failed to read the server folder"))?;
    for entry in entries.flatten() {
        // names that can't be stored were never backed up, so they are never owned
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        if !valid_rel_path(&name) {
            continue;
        }
        names.entry(name).or_default().0 = disk_kind(&entry.path());
    }
    let mut owned = Vec::new();
    for (name, (disk, backup)) in names {
        if disk == Some(DiskKind::Link) {
            if backup.is_some() {
                return Err(bad_request(format!(
                    "{name} is a link; restore can't replace it"
                )));
            }
            continue;
        }
        let is_dir = match disk {
            Some(kind) => kind == DiskKind::Dir,
            None => backup.unwrap_or(false),
        };
        if always_excluded(&name, is_dir) || exclude.is_excluded(&name, is_dir) {
            continue;
        }
        // a backup without the server's settings (excluded when it was taken) keeps the
        // current ones: without them the server can't load
        if name == CONFIG_FILE && backup.is_none() {
            continue;
        }
        owned.push(OwnedPath {
            path: name,
            on_disk: disk.is_some(),
            in_backup: backup.is_some(),
        });
    }
    Ok(owned)
}

fn owned_world(
    server_root: &Path,
    manifest: &Manifest,
    exclude: &Matcher,
) -> Result<Vec<OwnedPath>, Error> {
    let level = manifest.level_name.as_deref().ok_or_else(|| {
        bad_request("This backup doesn't know the server's world folder, so only the whole server can be restored")
    })?;
    // checked again: the manifest may come from an older or edited file
    crate::handlers::server_content::world_folder(level)?;
    let mut owned = Vec::new();
    let candidates = [
        level.to_string(),
        format!("{level}_nether"),
        format!("{level}_the_end"),
    ];
    for path in candidates {
        let in_backup = manifest.files.iter().any(|f| under(&f.path, &path));
        let on_disk = world_path_on_disk(server_root, &path)?;
        if !in_backup && !on_disk {
            continue;
        }
        if always_excluded_path(&path, true) || exclude.is_excluded_path(&path, true) {
            continue;
        }
        owned.push(OwnedPath {
            path,
            on_disk,
            in_backup,
        });
    }
    if !owned.iter().any(|o| o.path == level && o.in_backup) {
        return Err(bad_request("This backup has no world"));
    }
    Ok(owned)
}

/// Whether a world folder is on disk, refusing links and anything outside the server folder.
fn world_path_on_disk(server_root: &Path, path: &str) -> Result<bool, Error> {
    let parts: Vec<&str> = path.split('/').collect();
    let mut current = server_root.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        current.push(part);
        match disk_kind(&current) {
            None => return Ok(false),
            Some(DiskKind::Link) => {
                return Err(bad_request(format!(
                    "{} is a link; restore can't replace it",
                    parts[..=i].join("/")
                )))
            }
            Some(DiskKind::File) if i + 1 < parts.len() => {
                return Err(bad_request(format!(
                    "{} is not a folder",
                    parts[..=i].join("/")
                )))
            }
            Some(_) => {}
        }
    }
    let root = server_root
        .canonicalize()
        .context("Failed to resolve the server folder")?;
    let resolved = current
        .canonicalize()
        .context("Failed to resolve the world folder")?;
    if !resolved.starts_with(&root) || resolved == root {
        return Err(bad_request("The world folder is outside the server folder"));
    }
    Ok(true)
}

// ---------------------------------------------------------------------------------------
// The permission check

/// What a restore would change inside its owned paths. The handler checks permissions
/// against it (protected files, code files).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreDiff {
    /// Files the backup has and the server doesn't.
    pub added: Vec<String>,
    /// Files whose contents differ.
    pub changed: Vec<String>,
    /// Files the server has and the backup doesn't.
    pub removed: Vec<String>,
    pub created_dirs: Vec<String>,
    pub removed_dirs: Vec<String>,
}

impl RestoreDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.removed.is_empty()
            && self.created_dirs.is_empty()
            && self.removed_dirs.is_empty()
    }
}

/// Compares the plan's files with `current`, the server's files as a backup records them
/// (the safety backup's, or [`Store::hash_current`]'s). `.lodestone_minecraft_config.json`
/// is left out: its sensitive fields are pinned instead.
pub fn diff(
    server_root: &Path,
    plan: &RestorePlan,
    current: &[FileEntry],
    exclude: &Matcher,
) -> Result<RestoreDiff, Error> {
    let considered = |path: &str| {
        path != CONFIG_FILE
            && plan.owns(path)
            && !always_excluded_path(path, false)
            && !exclude.is_excluded_path(path, false)
    };
    let backup: HashMap<&str, &FileEntry> = plan
        .files
        .iter()
        .filter(|f| considered(&f.path))
        .map(|f| (f.path.as_str(), f))
        .collect();
    let now: HashMap<&str, &FileEntry> = current
        .iter()
        .filter(|f| considered(&f.path))
        .map(|f| (f.path.as_str(), f))
        .collect();
    let mut result = RestoreDiff::default();
    for (path, entry) in &backup {
        match now.get(path) {
            None => result.added.push(path.to_string()),
            Some(current) if current.sha256 != entry.sha256 => {
                result.changed.push(path.to_string())
            }
            Some(_) => {}
        }
    }
    for path in now.keys() {
        if !backup.contains_key(path) {
            result.removed.push(path.to_string());
        }
    }

    let mut backup_dirs = BTreeSet::new();
    for file in &plan.files {
        for (i, c) in file.path.char_indices() {
            if c == '/' && plan.owns(&file.path[..i]) {
                backup_dirs.insert(file.path[..i].to_string());
            }
        }
    }
    let disk_dirs = owned_dirs_on_disk(server_root, plan, exclude);
    result.created_dirs = backup_dirs.difference(&disk_dirs).cloned().collect();
    result.removed_dirs = disk_dirs.difference(&backup_dirs).cloned().collect();
    result.added.sort();
    result.changed.sort();
    result.removed.sort();
    Ok(result)
}

/// The real folders inside the owned paths (the owned paths included), skipping what the
/// restore never touches.
fn owned_dirs_on_disk(
    server_root: &Path,
    plan: &RestorePlan,
    exclude: &Matcher,
) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for owned in &plan.owned {
        let top = join_rel(server_root, &owned.path);
        if !is_real_dir(&top) {
            continue;
        }
        dirs.insert(owned.path.clone());
        let mut entries = walkdir::WalkDir::new(&top)
            .follow_links(false)
            .min_depth(1)
            .into_iter();
        while let Some(entry) = entries.next() {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_dir() {
                continue;
            }
            let Some(rel) = relative_to(server_root, entry.path()) else {
                entries.skip_current_dir();
                continue;
            };
            if !valid_rel_path(&rel)
                || always_excluded(&rel, true)
                || exclude.is_excluded(&rel, true)
            {
                entries.skip_current_dir();
                continue;
            }
            dirs.insert(rel);
        }
    }
    dirs
}

fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

// ---------------------------------------------------------------------------------------
// Pinned settings

/// Settings a restore keeps from the current server (or sets, for a new one) instead of
/// taking the backup's. `None` leaves the backup's value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pin {
    pub name: Option<String>,
    pub port: Option<u32>,
    /// `Some(None)` clears it (the default Java).
    pub java_cmd: Option<Option<String>>,
    pub cmd_args: Option<Vec<String>>,
}

/// `.lodestone_minecraft_config.json` with the pinned fields set. Edited as JSON, so fields
/// this version doesn't know survive.
pub fn pin_config_json(text: &str, pin: &Pin) -> Result<String, Error> {
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        err(
            ErrorKind::Internal,
            format!("The backup's server settings can't be read: {e}"),
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        err(
            ErrorKind::Internal,
            "The backup's server settings can't be read",
        )
    })?;
    if let Some(name) = &pin.name {
        object.insert("name".to_string(), serde_json::json!(name));
    }
    if let Some(port) = pin.port {
        object.insert("port".to_string(), serde_json::json!(port));
    }
    if let Some(java_cmd) = &pin.java_cmd {
        object.insert("java_cmd".to_string(), serde_json::json!(java_cmd));
    }
    if let Some(cmd_args) = &pin.cmd_args {
        object.insert("cmd_args".to_string(), serde_json::json!(cmd_args));
    }
    serde_json::to_string_pretty(&value)
        .context("Failed to write the server settings")
        .map_err(Error::from)
}

/// `server.properties` with `server-port` set to `port`; every other line stays as it is.
pub fn pin_server_port(text: &[u8], port: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 20);
    let mut found = false;
    for line in text.split_inclusive(|b| *b == b'\n') {
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        let (content, ending): (&[u8], &[u8]) = match content.strip_suffix(b"\r") {
            Some(c) => (c, b"\r\n"),
            None if line.ends_with(b"\n") => (content, b"\n"),
            None => (content, b""),
        };
        let trimmed = content.trim_ascii_start();
        let is_comment = trimmed.starts_with(b"#") || trimmed.starts_with(b"!");
        let key_end = trimmed
            .iter()
            .position(|b| matches!(b, b'=' | b':' | b' ' | b'\t'))
            .unwrap_or(trimmed.len());
        if !is_comment && &trimmed[..key_end] == b"server-port" {
            out.extend_from_slice(format!("server-port={port}").as_bytes());
            out.extend_from_slice(if ending.is_empty() { b"\n" } else { ending });
            found = true;
        } else {
            out.extend_from_slice(line);
        }
    }
    if !found {
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(format!("server-port={port}\n").as_bytes());
    }
    out
}

/// Pins the settings in the config and `server.properties` under `dir` (staged files),
/// when they are there.
pub fn pin_settings(dir: &Path, pin: &Pin) -> Result<(), Error> {
    let config = dir.join(CONFIG_FILE);
    if config.is_file() {
        let text =
            fs::read_to_string(&config).context("Failed to read the backup's server settings")?;
        write_synced(&config, pin_config_json(&text, pin)?.as_bytes())
            .context("Failed to write the server settings")?;
    }
    if let Some(port) = pin.port {
        let properties = dir.join(PROPERTIES_FILE);
        if properties.is_file() {
            let text = fs::read(&properties).context("Failed to read server.properties")?;
            write_synced(&properties, &pin_server_port(&text, port))
                .context("Failed to write server.properties")?;
        }
    }
    Ok(())
}

/// Writes a staged file and flushes it to disk, like the files [`write_files`] writes.
fn write_synced(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    io::Write::write_all(&mut file, contents)?;
    file.sync_all()
}

// ---------------------------------------------------------------------------------------
// Staging

fn time_from_ns(ns: i128) -> SystemTime {
    let magnitude = Duration::from_nanos(ns.unsigned_abs().min(u64::MAX as u128) as u64);
    if ns >= 0 {
        UNIX_EPOCH + magnitude
    } else {
        UNIX_EPOCH - magnitude
    }
}

/// Writes `files` from the store under `dest`, verifying each, with its modified time and
/// (unix) execute bit.
fn write_files(
    store: &Store,
    backup_id: &str,
    files: &[FileEntry],
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<(), Error> {
    let total: u64 = files.iter().map(|f| f.size).sum();
    let mut done = 0;
    for file in files {
        if cancel() {
            return Err(cancelled());
        }
        let path = join_rel(dest, &file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                io_error(e, format!("Failed to create the folder of {}", file.path))
            })?;
        }
        let out = File::create(&path)
            .map_err(|e| io_error(e, format!("Failed to write {}", file.path)))?;
        let mut writer = BufWriter::new(out);
        store
            .read_object(&file.sha256, file.size, &mut writer, cancel)
            .map_err(|e| e.into_error(backup_id, &file.path))?;
        let out = writer
            .into_inner()
            .map_err(|e| io_error(e.into_error(), format!("Failed to write {}", file.path)))?;
        out.set_modified(time_from_ns(file.modified_ns))
            .map_err(|e| io_error(e, format!("Failed to write {}", file.path)))?;
        #[cfg(unix)]
        if file.executable {
            use std::os::unix::fs::PermissionsExt;
            let mode = out
                .metadata()
                .map(|m| m.permissions().mode())
                .unwrap_or(0o644);
            // execute wherever it may be read
            let _ = out.set_permissions(fs::Permissions::from_mode(mode | ((mode & 0o444) >> 2)));
        }
        // on disk before the swap puts it in place and deletes the file it replaces. Only
        // on unix, where the folders are flushed too (`sync_tree`): on Windows a folder
        // can't be, so flushing every file (FlushFileBuffers) would make restores of big
        // worlds minutes slower and still not make the swap durable
        #[cfg(unix)]
        out.sync_all()
            .map_err(|e| io_error(e, format!("Failed to write {}", file.path)))?;
        done += file.size;
        progress(done, total);
    }
    Ok(())
}

/// Flushes a folder's entries (unix; elsewhere the file system does it).
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Flushes `dir` and every folder in it, so files written into them can't vanish in a
/// power loss after the swap.
fn sync_tree(dir: &Path) -> Result<(), Error> {
    if cfg!(not(unix)) {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry =
            entry.map_err(|e| Error::from(eyre!(e).wrap_err("Failed to flush the restore")))?;
        if entry.file_type().is_dir() {
            sync_dir(entry.path()).map_err(|e| io_error(e, "Failed to flush the restore"))?;
        }
    }
    Ok(())
}

/// A backup's files staged inside the server folder, ready to [`swap`](Staged::swap) in.
#[derive(Debug)]
pub struct Staged {
    server_root: PathBuf,
    staging: String,
    old: String,
    /// The owned paths.
    owned: Vec<JournalEntry>,
    /// Paths inside the owned ones that the restore never touches (links, excluded and
    /// always-excluded paths), moved from the old entries into the new ones.
    carried: Vec<String>,
    rename_budget: Duration,
    #[cfg(test)]
    faults: Faults,
}

/// Simulated failures for the tests.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
struct Faults {
    /// Stop as if the process died after this many renames.
    crash_after: Option<usize>,
    /// Fail the rename after this many.
    fail_after: Option<usize>,
    /// Make the rollback fail too.
    break_rollback: bool,
    /// The journal can't be marked done or deleted.
    stuck_journal: bool,
}

/// Stages the plan's files and pins `pin` in them. Cancellable; on failure the staging
/// folder is deleted and nothing else has changed.
#[allow(clippy::too_many_arguments)]
pub fn stage(
    store: &Store,
    manifest: &Manifest,
    server_root: &Path,
    plan: &RestorePlan,
    exclude: &Matcher,
    pin: &Pin,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<Staged, Error> {
    let staging = random_suffix();
    let staging_dir = server_root.join(format!("{STAGING_PREFIX}{staging}"));
    fs::create_dir(&staging_dir)
        .map_err(|e| io_error(e, "Failed to create the restore's staging folder"))?;
    let result = (|| -> Result<Staged, Error> {
        write_files(
            store,
            &manifest.id,
            &plan.files,
            &staging_dir,
            progress,
            cancel,
        )?;
        pin_settings(&staging_dir, pin)?;
        sync_tree(&staging_dir)?;
        let mut owned = Vec::new();
        let mut carried = Vec::new();
        for path in &plan.owned {
            let on_disk = join_rel(server_root, &path.path);
            let kind = disk_kind(&on_disk);
            if kind == Some(DiskKind::Link) {
                return Err(bad_request(format!(
                    "{} is a link; restore can't replace it",
                    path.path
                )));
            }
            owned.push(JournalEntry {
                path: path.path.clone(),
                had_old: kind.is_some(),
                had_new: exists(&join_rel(&staging_dir, &path.path)),
            });
            if kind == Some(DiskKind::Dir) {
                carried.extend(kept_inside(server_root, &path.path, exclude)?);
            }
        }
        Ok(Staged {
            server_root: server_root.to_path_buf(),
            staging: staging.clone(),
            old: random_suffix(),
            owned,
            carried,
            rename_budget: SWAP_RENAME_BUDGET,
            #[cfg(test)]
            faults: Faults::default(),
        })
    })();
    if result.is_err() {
        if let Err(e) = fs::remove_dir_all(&staging_dir) {
            warn!("Failed to delete {}: {e}", staging_dir.display());
        }
    }
    result
}

/// The paths inside the owned folder `owned` that the restore keeps as they are: links,
/// always-excluded paths and paths the policy excludes.
fn kept_inside(server_root: &Path, owned: &str, exclude: &Matcher) -> Result<Vec<String>, Error> {
    let mut kept = Vec::new();
    let mut entries = walkdir::WalkDir::new(join_rel(server_root, owned))
        .follow_links(false)
        .min_depth(1)
        .into_iter();
    while let Some(entry) = entries.next() {
        let entry =
            entry.map_err(|e| Error::from(eyre!(e).wrap_err(format!("Failed to read {owned}"))))?;
        let is_dir = entry.file_type().is_dir();
        let rel = match relative_to(server_root, entry.path()) {
            Some(rel) if valid_rel_path(&rel) => rel,
            _ => {
                return Err(bad_request(format!(
                    "{owned} has a file whose name Lodestone can't handle ({}); rename or remove it first",
                    entry.file_name().to_string_lossy()
                )))
            }
        };
        if entry.path_is_symlink()
            || entry.file_type().is_symlink()
            || always_excluded(&rel, is_dir)
            || exclude.is_excluded(&rel, is_dir)
        {
            kept.push(rel);
            if is_dir {
                entries.skip_current_dir();
            }
        }
    }
    Ok(kept)
}

/// Why a swap failed.
#[derive(Debug)]
pub enum SwapError {
    /// Undone: the server folder is as it was.
    RolledBack(Error),
    /// Couldn't be undone. The journal stays for the next start, and the server must not
    /// start until then.
    Broken(Error),
}

enum MoveError {
    Failed(Error),
    #[cfg(test)]
    Crash,
}

impl Staged {
    fn staging_dir(&self) -> PathBuf {
        self.server_root
            .join(format!("{STAGING_PREFIX}{}", self.staging))
    }

    fn old_dir(&self) -> PathBuf {
        self.server_root.join(format!("{OLD_PREFIX}{}", self.old))
    }

    /// Deletes the staged files without changing anything else.
    pub fn discard(self) {
        let dir = self.staging_dir();
        if let Err(e) = fs::remove_dir_all(&dir) {
            warn!("Failed to delete {}: {e}", dir.display());
        }
    }

    /// Swaps the staged files in, with the journal at `journal_path`
    /// (`backups/servers/<uuid>/restore-journal.json`). Not cancellable: it is short, and
    /// stopping halfway would leave a half-restored server.
    pub fn swap(self, journal_path: &Path) -> Result<(), SwapError> {
        let mut journal = Journal {
            phase: Phase::MovingOld,
            staging: self.staging.clone(),
            old: self.old.clone(),
            entries: self
                .owned
                .iter()
                .cloned()
                .chain(self.carried.iter().map(|path| JournalEntry {
                    path: path.clone(),
                    had_old: true,
                    had_new: exists(&join_rel(&self.staging_dir(), path)),
                }))
                .collect(),
        };
        if let Err(e) = write_journal(journal_path, &journal) {
            self.discard();
            return Err(SwapError::RolledBack(e));
        }
        match self.move_all(&mut journal, journal_path) {
            Ok(()) => {
                self.sync_moved();
                match self.finish(journal_path, &journal) {
                    Ok(()) => {
                        self.remove_work_dirs();
                        Ok(())
                    }
                    // the journal still says the swap is under way, and the next start would
                    // undo it after the server ran: undo it now instead
                    Err(error) => self.undo(journal_path, &journal, error),
                }
            }
            #[cfg(test)]
            Err(MoveError::Crash) => Err(SwapError::Broken(err(ErrorKind::Internal, "crashed"))),
            Err(MoveError::Failed(error)) => self.undo(journal_path, &journal, error),
        }
    }

    /// Marks the swap done and deletes the journal. Fails only when the journal is left
    /// saying the swap is under way.
    fn finish(&self, journal_path: &Path, journal: &Journal) -> Result<(), Error> {
        let done = Journal {
            phase: Phase::Done,
            ..journal.clone()
        };
        let written = write_journal(journal_path, &done);
        #[cfg(test)]
        let written = if self.faults.stuck_journal {
            Err(err(ErrorKind::Internal, "failed on purpose"))
        } else {
            written
        };
        self.remove_journal(journal_path);
        match written {
            Err(error) if !journal_gone(journal_path) => Err(error),
            // a journal left saying "done" only makes the next start delete the work folders
            _ => Ok(()),
        }
    }

    /// Rolls back a swap that failed after the journal was written.
    fn undo(&self, journal_path: &Path, journal: &Journal, error: Error) -> Result<(), SwapError> {
        warn!("Restore failed, undoing it: {}", error.source);
        #[cfg(test)]
        let undone = if self.faults.break_rollback {
            Err(vec!["broken on purpose".to_string()])
        } else {
            rollback(&self.server_root, journal, self.rename_budget)
        };
        #[cfg(not(test))]
        let undone = rollback(&self.server_root, journal, self.rename_budget);
        match undone {
            Ok(()) => {
                // should the journal stay, the next start finds no old folder and leaves
                // everything as it is
                self.remove_journal(journal_path);
                self.remove_work_dirs();
                Err(SwapError::RolledBack(error))
            }
            Err(problems) => {
                warn!("The restore could not be undone: {}", problems.join("; "));
                Err(SwapError::Broken(err(
                    ErrorKind::Internal,
                    "The restore could not be undone; restart Lodestone to repair it",
                )))
            }
        }
    }

    fn remove_journal(&self, journal_path: &Path) {
        #[cfg(test)]
        if self.faults.stuck_journal {
            return;
        }
        remove_journal(journal_path);
    }

    /// Flushes the folders the swap renamed entries into, before the old entries are
    /// deleted. A failure only leaves the swap as durable as the file system makes it.
    fn sync_moved(&self) {
        let mut dirs = BTreeSet::new();
        for path in self.owned.iter().map(|e| &e.path).chain(&self.carried) {
            if let Some(parent) = join_rel(&self.server_root, path).parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
        for dir in dirs {
            if let Err(e) = sync_dir(&dir) {
                warn!("Failed to flush {}: {e}", dir.display());
            }
        }
    }

    fn move_all(&self, journal: &mut Journal, journal_path: &Path) -> Result<(), MoveError> {
        let staging_dir = self.staging_dir();
        let old_dir = self.old_dir();
        let mut renames = 0usize;
        let mut rename = |from: &Path, to: &Path, what: &str| -> Result<(), MoveError> {
            #[cfg(test)]
            {
                if self.faults.crash_after == Some(renames) {
                    return Err(MoveError::Crash);
                }
                if self.faults.fail_after == Some(renames) {
                    return Err(MoveError::Failed(err(
                        ErrorKind::Internal,
                        "failed on purpose",
                    )));
                }
            }
            renames += 1;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    MoveError::Failed(io_error(e, format!("Failed to move {what}")))
                })?;
            }
            rename_with_retry(from, to, self.rename_budget)
                .map_err(|e| MoveError::Failed(io_error(e, format!("Failed to move {what}"))))
        };

        fs::create_dir(&old_dir)
            .map_err(|e| MoveError::Failed(io_error(e, "Failed to create the restore's folder")))?;
        for entry in self.owned.iter().filter(|e| e.had_old) {
            rename(
                &join_rel(&self.server_root, &entry.path),
                &join_rel(&old_dir, &entry.path),
                &entry.path,
            )?;
        }
        journal.phase = Phase::MovingNew;
        write_journal(journal_path, journal).map_err(MoveError::Failed)?;
        for entry in self.owned.iter().filter(|e| e.had_new) {
            rename(
                &join_rel(&staging_dir, &entry.path),
                &join_rel(&self.server_root, &entry.path),
                &entry.path,
            )?;
        }
        // what the restore never touches goes back; the on-disk version wins over a staged
        // copy of the same path
        let replaced = staging_dir.join(REPLACED_DIR);
        for path in &self.carried {
            let destination = join_rel(&self.server_root, path);
            if exists(&destination) {
                rename(&destination, &join_rel(&replaced, path), path)?;
            }
            rename(&join_rel(&old_dir, path), &destination, path)?;
        }
        #[cfg(test)]
        if self.faults.crash_after == Some(renames) {
            return Err(MoveError::Crash);
        }
        Ok(())
    }

    fn remove_work_dirs(&self) {
        for dir in [self.staging_dir(), self.old_dir()] {
            match fs::remove_dir_all(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => warn!("Failed to delete {}: {e}", dir.display()),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// The journal and recovery

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Moving the owned entries into the old folder.
    MovingOld,
    /// Moving the staged entries into place, then the kept paths back into them.
    MovingNew,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Relative to the server folder. An entry inside another entry is a kept path, moved
    /// from the old entry into the new one at the end of `moving_new`.
    pub path: String,
    pub had_old: bool,
    pub had_new: bool,
}

/// `backups/servers/<uuid>/restore-journal.json`. Holds only the folders' random suffixes
/// and relative paths, and is checked strictly before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub phase: Phase,
    /// The 16 hex digits of `.lodestone-restore-staging-<16>`.
    pub staging: String,
    /// The 16 hex digits of `.lodestone-restore-old-<16>`.
    pub old: String,
    pub entries: Vec<JournalEntry>,
}

fn valid_suffix(suffix: &str) -> bool {
    suffix.len() == 16
        && suffix
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn write_journal(path: &Path, journal: &Journal) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create the restore journal's folder")?;
    }
    let json = serde_json::to_vec_pretty(journal).context("Failed to write the restore journal")?;
    crate::persist::write_atomic_blocking(path, &json, None)
}

fn remove_journal(path: &Path) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != io::ErrorKind::NotFound {
            warn!(
                "Failed to delete the restore journal {}: {e}",
                path.display()
            );
        }
    }
}

/// Whether the journal is surely gone (not merely unreadable).
fn journal_gone(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
}

/// Whether no folder on the way to `rel` under `base` is a link or a file.
fn no_links_above(base: &Path, rel: &str) -> bool {
    let mut current = base.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return false,
            _ => {}
        }
    }
    true
}

/// Whether `path` is there (a link counts, not followed). `Ok(false)` only when it surely
/// isn't: an error other than "not found" (no permission, an I/O error, a Windows sharing
/// or delete-pending error) is returned, never taken for a missing path.
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Undoes an interrupted or failed swap. Never deletes anything at a destination: what is
/// in the way goes into the staging folder. Returns what couldn't be done.
fn rollback(server_root: &Path, journal: &Journal, budget: Duration) -> Result<(), Vec<String>> {
    rollback_with(server_root, journal, budget, &present)
}

/// [`rollback`], with `present` telling whether the old folder and each entry are there.
fn rollback_with(
    server_root: &Path,
    journal: &Journal,
    budget: Duration,
    present: &dyn Fn(&Path) -> io::Result<bool>,
) -> Result<(), Vec<String>> {
    if !valid_suffix(&journal.staging) || !valid_suffix(&journal.old) {
        return Err(vec!["the journal names invalid folders".to_string()]);
    }
    if let Some(bad) = journal.entries.iter().find(|e| !valid_rel_path(&e.path)) {
        return Err(vec![format!(
            "the journal has an invalid path {:?}",
            bad.path
        )]);
    }
    let staging_dir = server_root.join(format!("{STAGING_PREFIX}{}", journal.staging));
    let old_dir = server_root.join(format!("{OLD_PREFIX}{}", journal.old));
    for dir in [&staging_dir, &old_dir] {
        if exists(dir) && !is_real_dir(dir) {
            return Err(vec![format!("{} is not a folder", dir.display())]);
        }
    }
    // The old folder is made before anything moves and deleted only once the swap is
    // finished or undone, so without it there is nothing to undo: moving entries aside now
    // would throw away a finished restore, or the files a rollback already put back.
    // Only a folder that is surely gone counts as gone: one that can't be read may hold the
    // server's original entries.
    if journal.phase == Phase::Done {
        return Ok(());
    }
    match present(&old_dir) {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(e) => return Err(vec![format!("{} couldn't be read: {e}", old_dir.display())]),
    }
    let is_kept = |entry: &JournalEntry| {
        journal
            .entries
            .iter()
            .any(|other| other.path != entry.path && under(&entry.path, &other.path))
    };
    // whether the old entry a kept path came from is still in the old folder
    let owner_in_old = |entry: &JournalEntry| {
        journal.entries.iter().any(|other| {
            other.path != entry.path
                && under(&entry.path, &other.path)
                && !is_kept(other)
                && exists(&join_rel(&old_dir, &other.path))
        })
    };
    let (kept, owned): (Vec<&JournalEntry>, Vec<&JournalEntry>) =
        journal.entries.iter().partition(|e| is_kept(e));
    let mut problems = Vec::new();
    let mut aside = 0usize;
    let move_one = |from: &Path, to: &Path, rel: &str, problems: &mut Vec<String>| {
        let result = to
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| rename_with_retry(from, to, budget));
        if let Err(e) = result {
            problems.push(format!("{rel}: {e}"));
        }
    };
    let mut move_aside = |from: &Path, rel: &str, problems: &mut Vec<String>| {
        let mut to = join_rel(&staging_dir, rel);
        while exists(&to) {
            aside += 1;
            to = join_rel(&staging_dir.join(format!(".rollback-{aside}")), rel);
        }
        move_one(from, &to, rel, problems);
    };

    if journal.phase == Phase::MovingNew {
        // kept paths already moved into the new entries go back into the old ones first;
        // an old entry that is back in place already holds its kept paths (a repeated
        // rollback)
        for entry in kept.iter().rev() {
            let at_old = join_rel(&old_dir, &entry.path);
            let at_root = join_rel(server_root, &entry.path);
            let (old_there, root_there) = match (present(&at_old), present(&at_root)) {
                (Ok(old_there), Ok(root_there)) => (old_there, root_there),
                (Err(e), _) | (_, Err(e)) => {
                    problems.push(format!("{}: {e}", entry.path));
                    continue;
                }
            };
            if old_there || !root_there || !owner_in_old(entry) {
                continue;
            }
            if !no_links_above(server_root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
                problems.push(format!("{}: a folder on the way is a link", entry.path));
                continue;
            }
            move_one(&at_root, &at_old, &entry.path, &mut problems);
        }
    }
    for entry in owned.iter().rev() {
        let at_old = join_rel(&old_dir, &entry.path);
        let at_root = join_rel(server_root, &entry.path);
        if !no_links_above(server_root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
            problems.push(format!("{}: a folder on the way is a link", entry.path));
            continue;
        }
        let (old_there, root_there) = match (present(&at_old), present(&at_root)) {
            (Ok(old_there), Ok(root_there)) => (old_there, root_there),
            (Err(e), _) | (_, Err(e)) => {
                problems.push(format!("{}: {e}", entry.path));
                continue;
            }
        };
        if old_there {
            if root_there {
                move_aside(&at_root, &entry.path, &mut problems);
            }
            move_one(&at_old, &at_root, &entry.path, &mut problems);
        } else if !entry.had_old && journal.phase == Phase::MovingNew && root_there {
            move_aside(&at_root, &entry.path, &mut problems);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// What [`recover`] found.
#[derive(Debug, Default)]
pub struct Recovery {
    /// Servers whose interrupted restore was undone (or finished, if it was done).
    pub repaired: Vec<InstanceUuid>,
    /// Servers whose restore couldn't be undone; they must not start until it is.
    pub broken: Vec<InstanceUuid>,
}

/// At startup, before the servers are loaded: undoes interrupted restores, and deletes
/// leftovers (restore folders no journal names, half-made new servers, old temporary
/// objects, save-off markers).
pub fn recover(store: &Store, instances_dir: &Path) -> Recovery {
    recover_with(store, instances_dir, SWAP_RENAME_BUDGET)
}

fn recover_with(store: &Store, instances_dir: &Path, budget: Duration) -> Recovery {
    let mut recovery = Recovery::default();
    let (servers, all_read) = server_folders(instances_dir);
    // server folders whose journal couldn't be carried out: their restore folders may hold
    // the server's files, so none of them is deleted
    let mut protected: HashSet<PathBuf> = HashSet::new();
    // false when a journal may exist that couldn't be seen: then no leftover restore folder
    // is deleted, since it may hold a server's original files
    let mut journals_seen = true;

    match fs::read_dir(store.servers_dir()) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            warn!("Keeping every restore folder: the backups folder can't be read: {e}");
            journals_seen = false;
        }
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        warn!(
                            "Keeping every restore folder: the backups folder can't be read: {e}"
                        );
                        journals_seen = false;
                        continue;
                    }
                };
                let Some(name) = entry.file_name().to_str().map(String::from) else {
                    continue;
                };
                if !valid_server_id(&name) {
                    continue;
                }
                let journal_path = entry.path().join("restore-journal.json");
                match fs::symlink_metadata(&journal_path) {
                    Ok(meta) if meta.is_file() => {}
                    Ok(_) => continue,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => {
                        let uuid = InstanceUuid::from(name);
                        warn!("The restore journal of {uuid} can't be checked, so the server stays locked: {e}");
                        match servers.get(&uuid) {
                            Some(root) => {
                                protected.insert(root.clone());
                                recovery.broken.push(uuid);
                            }
                            None => journals_seen = false,
                        }
                        continue;
                    }
                }
                let uuid = InstanceUuid::from(name);
                let Some(root) = servers.get(&uuid) else {
                    if all_read {
                        warn!("Deleting the restore journal of {uuid}, whose server is gone");
                        remove_journal(&journal_path);
                    } else {
                        warn!("Keeping the restore journal of {uuid}: its server wasn't found, but not every server folder could be read");
                    }
                    continue;
                };
                let journal: Journal = match fs::read(&journal_path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
                {
                    Ok(journal) => journal,
                    Err(e) => {
                        warn!("The restore journal of {uuid} can't be read, so the server stays locked: {e}");
                        protected.insert(root.clone());
                        recovery.broken.push(uuid);
                        continue;
                    }
                };
                match rollback(root, &journal, budget) {
                    Ok(()) => {
                        if journal.phase == Phase::Done {
                            info!("Finishing the restore of {uuid}");
                        } else {
                            warn!("Undid the interrupted restore of {uuid}");
                        }
                        remove_journal(&journal_path);
                        for dir in [
                            root.join(format!("{STAGING_PREFIX}{}", journal.staging)),
                            root.join(format!("{OLD_PREFIX}{}", journal.old)),
                        ] {
                            if is_real_dir(&dir) {
                                if let Err(e) = fs::remove_dir_all(&dir) {
                                    warn!("Failed to delete {}: {e}", dir.display());
                                }
                            }
                        }
                        recovery.repaired.push(uuid);
                    }
                    Err(problems) => {
                        warn!(
                        "The interrupted restore of {uuid} couldn't be undone, so the server stays locked: {}",
                        problems.join("; ")
                    );
                        protected.insert(root.clone());
                        recovery.broken.push(uuid);
                    }
                }
            }
        }
    }

    // leftovers no journal names
    for root in servers.values() {
        if !journals_seen || protected.contains(root) {
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_restore_dir = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(".lodestone-restore"));
            if is_restore_dir && is_real_dir(&path) {
                warn!("Deleting the leftover restore folder {}", path.display());
                if let Err(e) = fs::remove_dir_all(&path) {
                    warn!("Failed to delete {}: {e}", path.display());
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir(instances_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_new = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(NEW_PREFIX));
            if is_new && is_real_dir(&path) {
                warn!("Deleting the unfinished restored server {}", path.display());
                if let Err(e) = fs::remove_dir_all(&path) {
                    warn!("Failed to delete {}: {e}", path.display());
                }
            }
        }
    }
    store.sweep_temp(TEMP_MAX_AGE);
    super::live::remove_leftover_markers(store);
    recovery
}

/// The server folders in `instances_dir`, by the uuid in their `.lodestone_config`, and
/// whether every folder could be read (otherwise a server that wasn't found may still be
/// there). A folder may be a link, as the core loads servers through links too; the paths
/// go through it.
fn server_folders(instances_dir: &Path) -> (HashMap<InstanceUuid, PathBuf>, bool) {
    let mut servers = HashMap::new();
    let entries = match fs::read_dir(instances_dir) {
        Ok(entries) => entries,
        Err(e) => return (servers, e.kind() == io::ErrorKind::NotFound),
    };
    let mut all_read = true;
    for entry in entries {
        let Ok(entry) = entry else {
            all_read = false;
            continue;
        };
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => continue,
            Err(_) => {
                all_read = false;
                continue;
            }
        }
        let bytes = match fs::read(path.join(".lodestone_config")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                all_read = false;
                continue;
            }
        };
        match serde_json::from_slice::<DotLodestoneConfig>(&bytes) {
            Ok(config) => {
                servers.insert(config.uuid().clone(), path);
            }
            Err(_) => all_read = false,
        }
    }
    (servers, all_read)
}

// ---------------------------------------------------------------------------------------
// Restore as a new server

/// A new `<instances>/.lodestone-restore-new-<16>` path for [`stage_all`].
pub fn new_server_dir(instances_dir: &Path) -> PathBuf {
    instances_dir.join(format!("{NEW_PREFIX}{}", random_suffix()))
}

/// Writes every file of the backup into `dest`, a new folder. On failure `dest` is deleted.
/// The caller then pins the settings ([`pin_settings`]), writes `.lodestone_config` last and
/// renames the folder.
pub fn stage_all(
    store: &Store,
    manifest: &Manifest,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<(), Error> {
    fs::create_dir(dest).map_err(|e| io_error(e, "Failed to create the new server's folder"))?;
    let result = write_files(store, &manifest.id, &manifest.files, dest, progress, cancel)
        .and_then(|()| sync_tree(dest));
    if result.is_err() {
        if let Err(e) = fs::remove_dir_all(dest) {
            warn!("Failed to delete {}: {e}", dest.display());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backups::store::tests::{backup, link_dir, uuid, write, NO_CANCEL};
    use crate::backups::types::BackupKind;
    use crate::traits::t_configurable::GameType;

    struct Env {
        _dir: tempfile::TempDir,
        store: Store,
        instances: PathBuf,
        root: PathBuf,
    }

    fn env() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("backups"));
        let instances = dir.path().join("instances");
        let root = instances.join("Test-12345678");
        fs::create_dir_all(&root).unwrap();
        let config = DotLodestoneConfig::new(uuid(), GameType::MinecraftJava);
        fs::write(
            root.join(".lodestone_config"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        Env {
            _dir: dir,
            store,
            instances,
            root,
        }
    }

    /// Every file and folder under `root`, with file contents.
    fn snapshot(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        let mut out = BTreeMap::new();
        for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
            let entry = entry.unwrap();
            let rel = relative_to(root, entry.path()).unwrap();
            let contents = if entry.file_type().is_file() {
                Some(fs::read(entry.path()).unwrap())
            } else if entry.file_type().is_symlink() {
                Some(b"<link>".to_vec())
            } else {
                None
            };
            out.insert(rel, contents);
        }
        out
    }

    fn read(root: &Path, path: &str) -> Option<String> {
        fs::read_to_string(join_rel(root, path)).ok()
    }

    fn restore(env: &Env, manifest: &Manifest, scope: RestoreScope, exclude: &Matcher, pin: &Pin) {
        let plan = plan(&env.root, manifest, scope, exclude).unwrap();
        let staged = stage(
            &env.store,
            manifest,
            &env.root,
            &plan,
            exclude,
            pin,
            &mut |_, _| {},
            NO_CANCEL,
        )
        .unwrap();
        staged
            .swap(&env.store.journal_path(&uuid()).unwrap())
            .unwrap();
    }

    fn base_server(root: &Path) {
        write(root, "server.properties", b"motd=hi\nserver-port=25565\n");
        write(
            root,
            CONFIG_FILE,
            br#"{"name":"Old name","port":25565,"java_cmd":null,"cmd_args":[],"version":"1.20"}"#,
        );
        write(root, "server.jar", b"jar v1");
        write(root, "world/level.dat", b"level v1");
        write(root, "world/region/r.0.0.mca", b"region v1");
        write(root, "world_nether/DIM-1/r.mca", b"nether v1");
        write(root, "plugins/dynmap/config.txt", b"dynmap config v1");
    }

    #[test]
    fn restore_everything() {
        let env = env();
        let root = &env.root;
        base_server(root);
        let exclude = Matcher::with_case(
            &[
                "plugins/dynmap/web/tiles/".to_string(),
                "/old-worlds/".to_string(),
            ],
            false,
        );
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &exclude);

        // the server changes after the backup
        write(root, "server.jar", b"jar v2");
        write(root, "world/level.dat", b"level v2");
        write(root, "world/new-file.dat", b"new");
        fs::remove_file(root.join("world_nether/DIM-1/r.mca")).unwrap();
        write(root, "world/session.lock", b"lock");
        write(root, "added-later.txt", b"added");
        write(root, "logs/latest.log", b"log");
        write(root, "old-worlds/w/level.dat", b"excluded top level");
        write(root, "plugins/dynmap/web/tiles/0.png", b"tile");
        write(root, "plugins/dynmap/config.txt", b"dynmap config v2");
        write(root, CONFIG_FILE, br#"{"name":"New name","port":25570,"java_cmd":"/opt/java","cmd_args":["-Dx"],"version":"1.21","future_field":1}"#);
        write(
            root,
            "server.properties",
            b"motd=changed\nserver-port=25570\n",
        );
        let config_before = fs::read(root.join(".lodestone_config")).unwrap();
        let outside = env.instances.parent().unwrap().join("outside");
        write(&outside, "data.txt", b"outside");
        let linked = link_dir(&outside, &root.join("linked-top"))
            && link_dir(&outside, &root.join("world/linked-inside"));

        let pin = Pin {
            name: Some("New name".to_string()),
            port: Some(25570),
            java_cmd: Some(Some("/opt/java".to_string())),
            cmd_args: Some(vec!["-Dx".to_string()]),
        };
        restore(&env, &manifest, RestoreScope::Everything, &exclude, &pin);

        assert_eq!(read(root, "server.jar").unwrap(), "jar v1");
        assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
        assert_eq!(read(root, "world/new-file.dat"), None);
        assert_eq!(read(root, "added-later.txt"), None);
        assert_eq!(read(root, "world_nether/DIM-1/r.mca").unwrap(), "nether v1");
        assert_eq!(
            read(root, "plugins/dynmap/config.txt").unwrap(),
            "dynmap config v1"
        );
        // never touched
        assert_eq!(read(root, "world/session.lock").unwrap(), "lock");
        assert_eq!(read(root, "logs/latest.log").unwrap(), "log");
        assert_eq!(
            read(root, "old-worlds/w/level.dat").unwrap(),
            "excluded top level"
        );
        assert_eq!(
            read(root, "plugins/dynmap/web/tiles/0.png").unwrap(),
            "tile"
        );
        assert_eq!(
            fs::read(root.join(".lodestone_config")).unwrap(),
            config_before
        );
        if linked {
            assert!(fs::symlink_metadata(root.join("linked-top"))
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(fs::symlink_metadata(root.join("world/linked-inside"))
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(read(&outside, "data.txt").unwrap(), "outside");
        }
        // pinned settings, other fields from the backup
        let config: serde_json::Value =
            serde_json::from_str(&read(root, CONFIG_FILE).unwrap()).unwrap();
        assert_eq!(config["name"], "New name");
        assert_eq!(config["port"], 25570);
        assert_eq!(config["java_cmd"], "/opt/java");
        assert_eq!(config["cmd_args"], serde_json::json!(["-Dx"]));
        assert_eq!(config["version"], "1.20");
        assert_eq!(
            read(root, "server.properties").unwrap(),
            "motd=hi\nserver-port=25570\n"
        );
        // restored modified times
        let restored = fs::metadata(root.join("server.jar"))
            .unwrap()
            .modified()
            .unwrap();
        let entry = manifest
            .files
            .iter()
            .find(|f| f.path == "server.jar")
            .unwrap();
        assert_eq!(
            crate::backups::store::unix_ns(restored) / 1_000_000,
            entry.modified_ns / 1_000_000
        );
        // nothing left behind
        assert!(!fs::read_dir(root).unwrap().flatten().any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with(".lodestone-restore")));
        assert!(!env.store.journal_path(&uuid()).unwrap().exists());
    }

    #[test]
    fn a_link_where_the_backup_has_a_folder_is_refused() {
        let env = env();
        write(&env.root, "plugins/a.jar", b"a");
        let manifest = backup(
            &env.store,
            &env.root,
            "b1",
            BackupKind::Manual,
            &Matcher::default(),
        );
        fs::remove_dir_all(env.root.join("plugins")).unwrap();
        let outside = env.instances.parent().unwrap().join("shared-plugins");
        fs::create_dir_all(&outside).unwrap();
        if !link_dir(&outside, &env.root.join("plugins")) {
            return;
        }
        let error = plan(
            &env.root,
            &manifest,
            RestoreScope::Everything,
            &Matcher::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.source.to_string(),
            "plugins is a link; restore can't replace it"
        );
    }

    #[test]
    fn restore_world_nested_and_not_its_neighbours() {
        let env = env();
        let root = &env.root;
        write(root, "server.properties", b"level-name=worlds/survival\n");
        write(root, "worlds/survival/level.dat", b"level v1");
        write(root, "worlds/survival_nether/DIM-1/r.mca", b"nether v1");
        write(root, "worlds/survival2/level.dat", b"other world v1");
        write(root, "server.jar", b"jar v1");
        let mut manifest = backup(
            &env.store,
            root,
            "b1",
            BackupKind::Manual,
            &Matcher::default(),
        );
        manifest.level_name = Some("worlds/survival".to_string());

        write(root, "worlds/survival/level.dat", b"level v2");
        write(root, "worlds/survival/extra.dat", b"extra");
        write(root, "worlds/survival/session.lock", b"lock");
        write(root, "worlds/survival_the_end/DIM1/r.mca", b"end v2");
        write(root, "worlds/survival2/level.dat", b"other world v2");
        write(root, "server.jar", b"jar v2");

        let plan_ = plan(root, &manifest, RestoreScope::World, &Matcher::default()).unwrap();
        let owned: Vec<&str> = plan_.owned.iter().map(|o| o.path.as_str()).collect();
        assert_eq!(
            owned,
            vec![
                "worlds/survival",
                "worlds/survival_nether",
                "worlds/survival_the_end"
            ]
        );
        assert!(plan_
            .files
            .iter()
            .all(|f| !f.path.starts_with("worlds/survival2")));
        restore(
            &env,
            &manifest,
            RestoreScope::World,
            &Matcher::default(),
            &Pin::default(),
        );

        assert_eq!(read(root, "worlds/survival/level.dat").unwrap(), "level v1");
        assert_eq!(read(root, "worlds/survival/extra.dat"), None);
        assert_eq!(read(root, "worlds/survival/session.lock").unwrap(), "lock");
        assert_eq!(
            read(root, "worlds/survival_nether/DIM-1/r.mca").unwrap(),
            "nether v1"
        );
        // the backup had no end: the end is removed
        assert!(!root.join("worlds/survival_the_end").exists());
        // not part of the world
        assert_eq!(
            read(root, "worlds/survival2/level.dat").unwrap(),
            "other world v2"
        );
        assert_eq!(read(root, "server.jar").unwrap(), "jar v2");
    }

    #[test]
    fn world_restore_refuses_bad_level_names() {
        let env = env();
        write(&env.root, "world/level.dat", b"x");
        let mut manifest = backup(
            &env.store,
            &env.root,
            "b1",
            BackupKind::Manual,
            &Matcher::default(),
        );
        manifest.level_name = Some("../Other-1a2b3c4d/world".to_string());
        assert!(plan(
            &env.root,
            &manifest,
            RestoreScope::World,
            &Matcher::default()
        )
        .is_err());
        manifest.level_name = None;
        assert!(plan(
            &env.root,
            &manifest,
            RestoreScope::World,
            &Matcher::default()
        )
        .is_err());
        manifest.level_name = Some("nothere".to_string());
        assert!(plan(
            &env.root,
            &manifest,
            RestoreScope::World,
            &Matcher::default()
        )
        .is_err());
    }

    #[test]
    fn diff_lists_changes_in_owned_paths() {
        let env = env();
        let root = &env.root;
        base_server(root);
        let none = Matcher::default();
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &none);
        write(root, "server.jar", b"jar v2");
        write(root, "mods/new.jar", b"new mod");
        fs::remove_dir_all(root.join("plugins")).unwrap();
        write(root, CONFIG_FILE, b"{}");
        fs::create_dir_all(root.join("empty-dir")).unwrap();
        let current = backup(&env.store, root, "b2", BackupKind::BeforeRestore, &none);

        let plan_ = plan(root, &manifest, RestoreScope::Everything, &none).unwrap();
        let d = diff(root, &plan_, &current.files, &none).unwrap();
        assert_eq!(d.added, vec!["plugins/dynmap/config.txt"]);
        assert_eq!(d.changed, vec!["server.jar"]);
        assert_eq!(d.removed, vec!["mods/new.jar"]);
        assert_eq!(d.created_dirs, vec!["plugins", "plugins/dynmap"]);
        assert_eq!(d.removed_dirs, vec!["empty-dir", "mods"]);

        // a world restore doesn't touch the jar
        let mut with_world = manifest.clone();
        with_world.level_name = Some("world".to_string());
        let plan_ = plan(root, &with_world, RestoreScope::World, &none).unwrap();
        let d = diff(root, &plan_, &current.files, &none).unwrap();
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn pin_config_keeps_unknown_fields() {
        let text = r#"{"name":"Backup","port":1,"java_cmd":"/bad/java","cmd_args":["-evil"],"modpack":{"x":1}}"#;
        let pinned = pin_config_json(
            text,
            &Pin {
                name: Some("Current".to_string()),
                port: Some(25566),
                java_cmd: Some(None),
                cmd_args: Some(vec![]),
            },
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&pinned).unwrap();
        assert_eq!(value["name"], "Current");
        assert_eq!(value["port"], 25566);
        assert!(value["java_cmd"].is_null());
        assert_eq!(value["cmd_args"], serde_json::json!([]));
        assert_eq!(value["modpack"]["x"], 1);
        // nothing pinned: unchanged values
        let same = pin_config_json(text, &Pin::default()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&same).unwrap();
        assert_eq!(value["java_cmd"], "/bad/java");
        assert!(pin_config_json("not json", &Pin::default()).is_err());
    }

    #[test]
    fn pin_server_port_keeps_other_lines() {
        let table: &[(&str, &str)] = &[
            (
                "motd=a\nserver-port=1\npvp=true\n",
                "motd=a\nserver-port=7\npvp=true\n",
            ),
            (
                "motd=a\r\nserver-port = 1\r\n",
                "motd=a\r\nserver-port=7\r\n",
            ),
            (
                "#server-port=1\nmotd=a",
                "#server-port=1\nmotd=a\nserver-port=7\n",
            ),
            ("server-port=1", "server-port=7\n"),
            ("server-portal=1\n", "server-portal=1\nserver-port=7\n"),
            ("", "server-port=7\n"),
        ];
        for (input, expected) in table {
            let out = pin_server_port(input.as_bytes(), 7);
            assert_eq!(String::from_utf8(out).unwrap(), *expected, "{input:?}");
        }
    }

    /// Sets up a server, backs it up, changes it, and stages the backup.
    fn staged_restore(env: &Env) -> (Staged, BTreeMap<String, Option<Vec<u8>>>) {
        let root = &env.root;
        base_server(root);
        let exclude = Matcher::with_case(&["plugins/dynmap/web/tiles/".to_string()], false);
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &exclude);
        write(root, "world/level.dat", b"level v2");
        write(root, "world/session.lock", b"lock");
        write(root, "added.txt", b"added");
        write(root, "plugins/dynmap/web/tiles/0.png", b"tile");
        fs::remove_dir_all(root.join("world_nether")).unwrap();
        let before = snapshot(root);
        let plan_ = plan(root, &manifest, RestoreScope::Everything, &exclude).unwrap();
        let staged = stage(
            &env.store,
            &manifest,
            root,
            &plan_,
            &exclude,
            &Pin::default(),
            &mut |_, _| {},
            NO_CANCEL,
        )
        .unwrap();
        (staged, before)
    }

    fn count_renames(env: &Env) -> usize {
        let (mut staged, _) = staged_restore(env);
        staged.faults.crash_after = Some(usize::MAX);
        let owned_old = staged.owned.iter().filter(|e| e.had_old).count();
        let owned_new = staged.owned.iter().filter(|e| e.had_new).count();
        let carried = staged.carried.len();
        staged.discard();
        // each kept path whose staged copy exists takes two renames; here none has one
        owned_old + owned_new + carried
    }

    #[test]
    fn recovery_after_a_crash_in_each_phase() {
        let total = count_renames(&env());
        assert!(total >= 8, "{total}");
        for crash_after in 0..=total {
            let env = env();
            let (mut staged, before) = staged_restore(&env);
            staged.faults.crash_after = Some(crash_after);
            let journal = env.store.journal_path(&uuid()).unwrap();
            assert!(matches!(staged.swap(&journal), Err(SwapError::Broken(_))));
            assert!(journal.exists());
            let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
            assert_eq!(recovery.repaired, vec![uuid()], "crash after {crash_after}");
            assert!(recovery.broken.is_empty());
            assert!(!journal.exists());
            assert_eq!(
                snapshot(&env.root),
                before,
                "crash after {crash_after} renames"
            );
        }
    }

    #[test]
    fn recovery_when_done_keeps_the_restore() {
        let env = env();
        let (staged, _) = staged_restore(&env);
        let staging = staged.staging_dir();
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        staged.swap(&journal_path).unwrap();
        let after = snapshot(&env.root);
        // as if the core died after writing "done" but before cleaning up
        fs::create_dir_all(staging.join("leftover")).unwrap();
        let suffix = staging
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .trim_start_matches(STAGING_PREFIX)
            .to_string();
        write_journal(
            &journal_path,
            &Journal {
                phase: Phase::Done,
                staging: suffix,
                old: "0123456789abcdef".to_string(),
                entries: vec![JournalEntry {
                    path: "world".to_string(),
                    had_old: true,
                    had_new: true,
                }],
            },
        )
        .unwrap();
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert_eq!(snapshot(&env.root), after);
        assert!(!staging.exists());
    }

    #[test]
    fn a_failed_swap_is_rolled_back() {
        for fail_after in [0, 2, 5, 9] {
            let env = env();
            let (mut staged, before) = staged_restore(&env);
            staged.faults.fail_after = Some(fail_after);
            let journal = env.store.journal_path(&uuid()).unwrap();
            assert!(matches!(
                staged.swap(&journal),
                Err(SwapError::RolledBack(_))
            ));
            assert!(!journal.exists());
            assert_eq!(snapshot(&env.root), before, "fail after {fail_after}");
        }
    }

    #[test]
    fn a_rollback_that_fails_keeps_the_journal() {
        let env = env();
        let (mut staged, _) = staged_restore(&env);
        staged.faults.fail_after = Some(3);
        staged.faults.break_rollback = true;
        let journal = env.store.journal_path(&uuid()).unwrap();
        match staged.swap(&journal) {
            Err(SwapError::Broken(e)) => assert_eq!(
                e.source.to_string(),
                "The restore could not be undone; restart Lodestone to repair it"
            ),
            other => panic!("{other:?}"),
        }
        assert!(journal.exists());
        // the next start repairs it
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
    }

    #[test]
    fn recovery_refuses_a_tampered_journal() {
        let env = env();
        write(&env.root, "world/level.dat", b"x");
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        for journal in [
            Journal {
                phase: Phase::MovingNew,
                staging: "../../stores".to_string(),
                old: "0123456789abcdef".to_string(),
                entries: vec![],
            },
            Journal {
                phase: Phase::MovingNew,
                staging: "0123456789abcdef".to_string(),
                old: "0123456789abcdef".to_string(),
                entries: vec![JournalEntry {
                    path: "../../stores/users.json".to_string(),
                    had_old: true,
                    had_new: false,
                }],
            },
        ] {
            write_journal(&journal_path, &journal).unwrap();
            let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
            assert_eq!(recovery.broken, vec![uuid()]);
            assert!(journal_path.exists());
        }
        assert_eq!(read(&env.root, "world/level.dat").unwrap(), "x");
    }

    #[test]
    fn recovery_deletes_leftovers() {
        let env = env();
        write(&env.root, "world/level.dat", b"x");
        write(
            &env.root,
            ".lodestone-restore-staging-0123456789abcdef/world/level.dat",
            b"staged",
        );
        write(
            &env.root,
            ".lodestone-restore-old-0123456789abcdef/world/level.dat",
            b"old",
        );
        write(
            &env.instances,
            ".lodestone-restore-new-0123456789abcdef/server.jar",
            b"half",
        );
        fs::create_dir_all(env.store.marker_path(&uuid()).unwrap().parent().unwrap()).unwrap();
        fs::write(env.store.marker_path(&uuid()).unwrap(), b"").unwrap();
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert!(recovery.repaired.is_empty() && recovery.broken.is_empty());
        let names: Vec<String> = fs::read_dir(&env.root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !names.iter().any(|n| n.starts_with(".lodestone-restore")),
            "{names:?}"
        );
        assert!(!env
            .instances
            .join(".lodestone-restore-new-0123456789abcdef")
            .exists());
        assert!(!env.store.marker_path(&uuid()).unwrap().exists());
        assert_eq!(read(&env.root, "world/level.dat").unwrap(), "x");
    }

    #[test]
    fn stage_all_and_damaged_backups() {
        let env = env();
        base_server(&env.root);
        let manifest = backup(
            &env.store,
            &env.root,
            "b1",
            BackupKind::Manual,
            &Matcher::default(),
        );
        let dest = new_server_dir(&env.instances);
        stage_all(&env.store, &manifest, &dest, &mut |_, _| {}, NO_CANCEL).unwrap();
        assert_eq!(read(&dest, "world/level.dat").unwrap(), "level v1");
        assert!(!dest.join(".lodestone_config").exists());
        pin_settings(
            &dest,
            &Pin {
                name: Some("Copy".to_string()),
                port: Some(25566),
                java_cmd: Some(None),
                cmd_args: Some(vec![]),
            },
        )
        .unwrap();
        assert_eq!(
            read(&dest, "server.properties").unwrap(),
            "motd=hi\nserver-port=25566\n"
        );

        // a damaged object fails staging and leaves nothing behind
        let entry = manifest
            .files
            .iter()
            .find(|f| f.path == "world/level.dat")
            .unwrap();
        let object = env.store.object_path(&entry.sha256);
        crate::backups::store::remove_file_forced(&object).unwrap();
        fs::write(&object, b"garbage").unwrap();
        let dest = new_server_dir(&env.instances);
        let error = stage_all(&env.store, &manifest, &dest, &mut |_, _| {}, NO_CANCEL).unwrap_err();
        assert_eq!(
            error.source.to_string(),
            "Backup b1 is damaged: world/level.dat"
        );
        assert!(!dest.exists());
        let plan_ = plan(
            &env.root,
            &manifest,
            RestoreScope::Everything,
            &Matcher::default(),
        )
        .unwrap();
        assert!(stage(
            &env.store,
            &manifest,
            &env.root,
            &plan_,
            &Matcher::default(),
            &Pin::default(),
            &mut |_, _| {},
            NO_CANCEL
        )
        .is_err());
        assert!(!fs::read_dir(&env.root).unwrap().flatten().any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with(".lodestone-restore")));
    }

    #[test]
    fn a_journal_that_cant_be_carried_out_keeps_the_restore_folders() {
        let tampered = serde_json::to_vec(&Journal {
            phase: Phase::MovingNew,
            staging: "../../stores".to_string(),
            old: "0123456789abcdef".to_string(),
            entries: vec![],
        })
        .unwrap();
        // from a newer core, or damaged
        let unreadable = br#"{"phase":"verifying","staging":"0123456789abcdef"}"#.to_vec();
        for text in [unreadable, tampered] {
            let env = env();
            write(&env.root, "world/region/r.0.0.mca", b"staged");
            write(
                &env.root,
                ".lodestone-restore-old-0123456789abcdef/world/level.dat",
                b"original",
            );
            write(
                &env.root,
                ".lodestone-restore-staging-0123456789abcdef/plugins/a.jar",
                b"staged",
            );
            let journal_path = env.store.journal_path(&uuid()).unwrap();
            fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
            fs::write(&journal_path, &text).unwrap();
            let before = snapshot(&env.root);
            let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
            assert_eq!(recovery.broken, vec![uuid()]);
            assert!(journal_path.exists());
            assert_eq!(snapshot(&env.root), before);
        }
    }

    #[test]
    fn recovery_through_a_linked_server_folder() {
        let env = env();
        let root = &env.root;
        base_server(root);
        let none = Matcher::default();
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &none);
        let elsewhere = env.instances.parent().unwrap().join("elsewhere");
        fs::rename(root, &elsewhere).unwrap();
        if !link_dir(&elsewhere, root) {
            return;
        }
        write(root, "world/level.dat", b"level v2");
        write(root, "world/session.lock", b"lock");
        write(root, "added.txt", b"added");
        let before = snapshot(&elsewhere);
        let plan_ = plan(root, &manifest, RestoreScope::Everything, &none).unwrap();
        let mut staged = stage(
            &env.store,
            &manifest,
            root,
            &plan_,
            &none,
            &Pin::default(),
            &mut |_, _| {},
            NO_CANCEL,
        )
        .unwrap();
        staged.faults.crash_after = Some(4);
        let journal = env.store.journal_path(&uuid()).unwrap();
        assert!(matches!(staged.swap(&journal), Err(SwapError::Broken(_))));
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert!(!journal.exists());
        assert_eq!(snapshot(&elsewhere), before);
    }

    /// A server whose folder is a link (or junction) is backed up and restored through it:
    /// the link stays, and its target gets the backup's files.
    #[test]
    fn backup_and_restore_through_a_linked_server_folder() {
        let env = env();
        let root = &env.root;
        base_server(root);
        let elsewhere = env.instances.parent().unwrap().join("elsewhere");
        fs::rename(root, &elsewhere).unwrap();
        if !link_dir(&elsewhere, root) {
            return;
        }
        let none = Matcher::default();
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &none);
        assert!(manifest.files.iter().any(|f| f.path == "world/level.dat"));
        write(root, "world/level.dat", b"level v2");
        write(root, "added.txt", b"added");
        restore(
            &env,
            &manifest,
            RestoreScope::Everything,
            &none,
            &Pin::default(),
        );
        assert_eq!(read(&elsewhere, "world/level.dat").unwrap(), "level v1");
        assert_eq!(read(&elsewhere, "added.txt"), None);
        assert!(fs::symlink_metadata(root).unwrap().file_type().is_symlink());
        assert!(!fs::read_dir(&elsewhere).unwrap().flatten().any(|e| e
            .file_name()
            .to_string_lossy()
            .starts_with(".lodestone-restore")));
    }

    #[test]
    fn a_journal_of_a_server_not_found_stays_when_a_folder_cant_be_read() {
        let env = env();
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        write_journal(
            &journal_path,
            &Journal {
                phase: Phase::MovingNew,
                staging: "0123456789abcdef".to_string(),
                old: "0123456789abcdef".to_string(),
                entries: vec![],
            },
        )
        .unwrap();
        // the server's settings can't be read, so it may be the journal's server
        fs::write(env.root.join(".lodestone_config"), b"damaged").unwrap();
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert!(recovery.repaired.is_empty() && recovery.broken.is_empty());
        assert!(journal_path.exists());
        // the server is gone for sure
        fs::remove_dir_all(&env.root).unwrap();
        recover_with(&env.store, &env.instances, Duration::ZERO);
        assert!(!journal_path.exists());
    }

    #[test]
    fn a_backup_without_the_settings_keeps_the_current_ones() {
        let env = env();
        let root = &env.root;
        base_server(root);
        let json = Matcher::with_case(&["*.json".to_string()], false);
        let manifest = backup(&env.store, root, "b1", BackupKind::Manual, &json);
        assert!(!manifest.files.iter().any(|f| f.path == CONFIG_FILE));
        let settings = fs::read(root.join(CONFIG_FILE)).unwrap();
        write(root, "server.jar", b"jar v2");

        // the exclude is gone by the time of the restore
        let none = Matcher::default();
        let plan_ = plan(root, &manifest, RestoreScope::Everything, &none).unwrap();
        assert!(!plan_.owns(CONFIG_FILE));
        restore(
            &env,
            &manifest,
            RestoreScope::Everything,
            &none,
            &Pin::default(),
        );
        assert_eq!(fs::read(root.join(CONFIG_FILE)).unwrap(), settings);
        assert_eq!(read(root, "server.jar").unwrap(), "jar v1");
    }

    #[test]
    fn a_journal_that_cant_be_finished_undoes_the_swap() {
        let env = env();
        let (mut staged, before) = staged_restore(&env);
        staged.faults.stuck_journal = true;
        let journal = env.store.journal_path(&uuid()).unwrap();
        assert!(matches!(
            staged.swap(&journal),
            Err(SwapError::RolledBack(_))
        ));
        assert_eq!(snapshot(&env.root), before);
        // the journal stayed, saying the swap is under way; the next start changes nothing
        assert!(journal.exists());
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert!(!journal.exists());
        assert_eq!(snapshot(&env.root), before);
    }

    #[test]
    fn a_finished_restore_with_a_stale_journal_is_kept() {
        let env = env();
        let (staged, _) = staged_restore(&env);
        let stale = Journal {
            phase: Phase::MovingNew,
            staging: staged.staging.clone(),
            old: staged.old.clone(),
            entries: staged
                .owned
                .iter()
                .cloned()
                .chain(staged.carried.iter().map(|path| JournalEntry {
                    path: path.clone(),
                    had_old: true,
                    had_new: false,
                }))
                .collect(),
        };
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        staged.swap(&journal_path).unwrap();
        let after = snapshot(&env.root);
        // as if the journal couldn't be marked done or deleted, and the folders were deleted
        write_journal(&journal_path, &stale).unwrap();
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert_eq!(snapshot(&env.root), after);
        assert_eq!(read(&env.root, "world/session.lock").unwrap(), "lock");
        assert!(!journal_path.exists());
    }

    #[test]
    fn a_repeated_rollback_changes_nothing() {
        let total = count_renames(&env());
        let env = env();
        let (mut staged, before) = staged_restore(&env);
        staged.faults.crash_after = Some(total);
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        assert!(matches!(
            staged.swap(&journal_path),
            Err(SwapError::Broken(_))
        ));
        let journal: Journal = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
        assert_eq!(journal.phase, Phase::MovingNew);
        rollback(&env.root, &journal, Duration::ZERO).unwrap();
        let once = snapshot(&env.root);
        rollback(&env.root, &journal, Duration::ZERO).unwrap();
        assert_eq!(snapshot(&env.root), once);
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert_eq!(snapshot(&env.root), before);
    }

    /// An old folder (or an entry in it) that can't be read isn't taken for a missing one:
    /// the rollback fails, moving nothing, so the journal stays and the server is protected.
    #[test]
    fn a_rollback_never_takes_an_unreadable_old_folder_for_a_missing_one() {
        let env = env();
        let (mut staged, _) = staged_restore(&env);
        staged.faults.crash_after = Some(4);
        let journal_path = env.store.journal_path(&uuid()).unwrap();
        assert!(matches!(
            staged.swap(&journal_path),
            Err(SwapError::Broken(_))
        ));
        let journal: Journal = serde_json::from_slice(&fs::read(&journal_path).unwrap()).unwrap();
        let old_dir = env.root.join(format!("{OLD_PREFIX}{}", journal.old));
        assert!(old_dir.is_dir());
        let half_swapped = snapshot(&env.root);
        let denied = || io::Error::from(io::ErrorKind::PermissionDenied);

        let unreadable_folder = |path: &Path| {
            if path == old_dir {
                Err(denied())
            } else {
                present(path)
            }
        };
        let problems =
            rollback_with(&env.root, &journal, Duration::ZERO, &unreadable_folder).unwrap_err();
        assert!(problems[0].contains("couldn't be read"), "{problems:?}");
        assert_eq!(snapshot(&env.root), half_swapped);

        let unreadable_entries = |path: &Path| {
            if path != old_dir && path.starts_with(&old_dir) {
                Err(denied())
            } else {
                present(path)
            }
        };
        assert!(rollback_with(&env.root, &journal, Duration::ZERO, &unreadable_entries).is_err());
        assert_eq!(snapshot(&env.root), half_swapped);

        // once it can be read, the next start undoes the swap
        assert!(journal_path.exists());
        let recovery = recover_with(&env.store, &env.instances, Duration::ZERO);
        assert_eq!(recovery.repaired, vec![uuid()]);
        assert!(!journal_path.exists());
    }

    #[test]
    fn under_matches_whole_parts() {
        assert!(under("world", "world"));
        assert!(under("world/level.dat", "world"));
        assert!(!under("world2/level.dat", "world"));
        assert!(!under("worl", "world"));
    }
}
