//! An instance's worlds (`minecraft/saves/`): listing them with what their `level.dat`
//! says, renaming, deleting (to the Trash), backups, restoring, and world zips.
//!
//! - **Backups** are zips at `<instance>/backups/<world folder>/<YYYY-MM-DD_HH-MM-SS>.zip`,
//!   with the world's files under `<world folder>/` (`session.lock` left out). They are
//!   written to a temporary file and renamed into place.
//! - **Restoring** makes a new world `<folder> (restored <date>)` by default. Replacing
//!   the world backs it up first, extracts the backup beside it, swaps the folders and
//!   moves the old one to the Trash.
//! - **World zips** (export, import) have the same layout; an imported zip needs
//!   `level.dat` at its root or one folder down. Every path is checked like a modpack's
//!   ([`check_pack_path`]), links are skipped and sizes are checked against the header.
//!   Names come from the zip's central directory, the same list the checks read; a
//!   local header that names another path is never used.
//!
//! Nothing here checks whether the game is running; the app refuses changes then.

use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::{
    level::{self, LevelSummary, MAX_LEVEL_DAT_BYTES},
    mrpack::{PathCheck, WriteAll, check_pack_path},
};

use crate::{
    content::{InstanceDirs, is_plain_name},
    game::download::{CancelToken, Progress, ProgressSnapshot},
    storage,
};

pub const SAVES: &str = "saves";
pub const BACKUPS: &str = "backups";
/// A world zip may have at most this many entries, and this many bytes unpacked.
pub const MAX_ZIP_ENTRIES: usize = 500_000;
pub const MAX_ZIP_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// Folder names are cut at this many characters.
const MAX_FOLDER_CHARS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("couldn't move {path} to {trash}: {reason}", trash = crate::wording::TRASH)]
    Trash { path: PathBuf, reason: String },
    #[error("cancelled")]
    Cancelled,
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> WorldError + '_ {
    move |source| WorldError::Io {
        path: path.to_owned(),
        source,
    }
}

fn zip_error(path: &Path) -> impl FnOnce(zip::result::ZipError) -> WorldError + '_ {
    move |err| {
        WorldError::Invalid(format!(
            "{} isn't a zip that can be read: {err}",
            path.display()
        ))
    }
}

/// One world.
#[derive(Clone, Debug, PartialEq)]
pub struct World {
    /// The folder in `saves/`.
    pub folder: String,
    pub summary: Option<LevelSummary>,
    /// Why `level.dat` couldn't be read.
    pub problem: Option<String>,
    /// The world's `icon.png`.
    pub icon: Option<PathBuf>,
    /// When `level.dat` was last written.
    pub modified: Option<SystemTime>,
    /// The size of the folder in bytes.
    pub size: u64,
}

impl World {
    /// The name the game shows: `LevelName`, else the folder.
    pub fn name(&self) -> String {
        self.summary
            .as_ref()
            .and_then(|s| s.name.clone())
            .unwrap_or_else(|| self.folder.clone())
    }

    pub fn last_played(&self) -> Option<SystemTime> {
        self.summary
            .as_ref()
            .and_then(|s| s.last_played)
            .filter(|ms| *ms > 0)
            .map(|ms| SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(ms as u64))
            .or(self.modified)
    }
}

/// A backup of a world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    pub file_name: String,
    pub size: u64,
    pub created: Option<SystemTime>,
}

pub fn saves_dir(dirs: &InstanceDirs) -> PathBuf {
    dirs.game.join(SAVES)
}

pub fn backups_dir(dirs: &InstanceDirs) -> PathBuf {
    dirs.instance.join(BACKUPS)
}

/// The world folder `folder`, if it is a plain name of a folder in `saves/`.
pub fn world_dir(dirs: &InstanceDirs, folder: &str) -> Result<PathBuf, WorldError> {
    if !is_plain_name(folder) {
        return Err(WorldError::NotFound(format!(
            "There is no world {folder:?}."
        )));
    }
    let dir = saves_dir(dirs).join(folder);
    match fs::symlink_metadata(&dir) {
        Ok(m) if m.is_dir() => Ok(dir),
        _ => Err(WorldError::NotFound(format!(
            "There is no world {folder:?}."
        ))),
    }
}

/// The bytes of the files under `dir`, without following links.
pub fn folder_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map_or(0, |m| m.len());
            }
        }
    }
    total
}

/// Reads one world folder.
pub fn read_world(dir: &Path) -> Option<World> {
    let folder = dir.file_name()?.to_str()?.to_string();
    let level_dat = dir.join("level.dat");
    let metadata = fs::symlink_metadata(&level_dat).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let (summary, problem) = if metadata.len() > MAX_LEVEL_DAT_BYTES {
        (None, Some("level.dat is too large".to_string()))
    } else {
        match fs::read(&level_dat)
            .map_err(|e| e.to_string())
            .and_then(|bytes| level::summarize(&bytes).map_err(|e| e.to_string()))
        {
            Ok(summary) => (Some(summary), None),
            Err(err) => (None, Some(format!("level.dat can't be read: {err}"))),
        }
    };
    let icon = dir.join("icon.png");
    Some(World {
        folder,
        summary,
        problem,
        icon: fs::symlink_metadata(&icon)
            .is_ok_and(|m| m.is_file())
            .then_some(icon),
        modified: metadata.modified().ok(),
        size: folder_size(dir),
    })
}

/// The worlds in `saves/`, most recently played first. Folders without `level.dat`,
/// hidden folders and links are left out.
pub fn list(dirs: &InstanceDirs) -> Vec<World> {
    let Ok(entries) = fs::read_dir(saves_dir(dirs)) else {
        return Vec::new();
    };
    let mut worlds: Vec<World> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(is_plain_name))
        .filter_map(|e| read_world(&e.path()))
        .collect();
    worlds.sort_by(|a, b| {
        b.last_played()
            .cmp(&a.last_played())
            .then_with(|| a.folder.cmp(&b.folder))
    });
    worlds
}

/// A folder name for a world called `name`, as the game makes them: characters that
/// can't be in file names become `_`.
pub fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(MAX_FOLDER_CHARS)
        .collect();
    let mut cleaned = cleaned
        .trim_start_matches('.')
        .trim_end_matches(['.', ' '])
        .trim()
        .replace("..", "_");
    let stem = cleaned
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        cleaned.push('_');
    }
    if cleaned.is_empty() {
        "World".to_string()
    } else {
        cleaned
    }
}

/// `base`, or `base (2)`, `base (3)`… whichever isn't taken in `dir`.
pub(crate) fn unused(dir: &Path, base: &str) -> String {
    unused_in(&[dir], base)
}

fn random_suffix() -> String {
    format!("{:016x}", u64::from_le_bytes(crate::random_bytes()))
}

/// Renames a world: its folder (to [`folder_name`] of `name`) and, when `level.dat`
/// reads and writes back unchanged, the name inside it. Its backups move along.
/// Returns the new folder.
///
/// The new folder name is one that is free in both `saves/` and `backups/`, so the
/// backups always have somewhere to go. If moving the backups or writing `level.dat`
/// fails, the world is moved back and the error returned.
pub fn rename(dirs: &InstanceDirs, folder: &str, name: &str) -> Result<String, WorldError> {
    rename_with(dirs, folder, name, |from, to| crate::fsx::rename(from, to))
}

/// Like [`rename`], with `move_dir` moving the world's folder and its backups folder
/// (tests make it fail).
pub fn rename_with(
    dirs: &InstanceDirs,
    folder: &str,
    name: &str,
    mut move_dir: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<String, WorldError> {
    let dir = world_dir(dirs, folder)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(WorldError::Invalid("Give the world a name.".into()));
    }
    let saves = saves_dir(dirs);
    let backups = backups_dir(dirs);
    let wanted = folder_name(name);
    let level_dat = |dir: &Path| dir.join("level.dat");
    let renamed_level = fs::read(level_dat(&dir))
        .ok()
        .and_then(|bytes| level::rename(&bytes, name));
    if wanted == folder {
        if let Some(renamed) = renamed_level {
            let path = level_dat(&dir);
            storage::write_atomic(&path, &renamed).map_err(io_error(&path))?;
        }
        return Ok(wanted);
    }
    let new_folder = unused_in(&[&saves, &backups], &wanted);
    let target = saves.join(&new_folder);
    let old_backups = backups.join(folder);
    let new_backups = backups.join(&new_folder);
    let has_backups = old_backups.is_dir();

    move_dir(&dir, &target).map_err(io_error(&dir))?;
    let mut backups_moved = false;
    let finished = (|| {
        if has_backups {
            move_dir(&old_backups, &new_backups).map_err(io_error(&old_backups))?;
            backups_moved = true;
        }
        if let Some(renamed) = &renamed_level {
            let path = level_dat(&target);
            storage::write_atomic(&path, renamed).map_err(io_error(&path))?;
        }
        Ok::<_, WorldError>(())
    })();
    let Err(err) = finished else {
        // The data packs installed from Modrinth are remembered by world folder. The
        // world is renamed either way; if the record can't follow, its packs only stop
        // updating in place.
        let _ = crate::datapacks::world_renamed(dirs, folder, &new_folder);
        return Ok(new_folder);
    };
    // Put everything back as it was, so the backups stay with their world.
    let backups_back = !backups_moved || move_dir(&new_backups, &old_backups).is_ok();
    let world_back = move_dir(&target, &dir).is_ok();
    Err(if backups_back && world_back {
        WorldError::Invalid(format!(
            "Couldn't rename the world, so it was left as it was: {err}"
        ))
    } else {
        WorldError::Invalid(format!(
            "Couldn't rename the world ({err}), and couldn't put it back as it was. \
             The world is in {}, and its backups are in {}.",
            if world_back { &dir } else { &target }.display(),
            if backups_back {
                &old_backups
            } else {
                &new_backups
            }
            .display()
        ))
    })
}

/// `base`, or `base (2)`, `base (3)`… whichever isn't taken in any of `dirs`.
fn unused_in(dirs: &[&Path], base: &str) -> String {
    (1..)
        .map(|n| {
            if n == 1 {
                base.to_string()
            } else {
                format!("{base} ({n})")
            }
        })
        .find(|name| {
            dirs.iter()
                .all(|dir| fs::symlink_metadata(dir.join(name)).is_err())
        })
        .expect("an unused name")
}

/// Moves a world to the Trash.
pub fn delete(dirs: &InstanceDirs, folder: &str) -> Result<(), WorldError> {
    delete_with(dirs, folder, |path| {
        trash::delete(path).map_err(|e| e.to_string())
    })
}

/// Like [`delete`], with `remove` doing the removal (tests don't use the Trash).
pub fn delete_with(
    dirs: &InstanceDirs,
    folder: &str,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), WorldError> {
    let dir = world_dir(dirs, folder)?;
    remove(&dir).map_err(|reason| WorldError::Trash { path: dir, reason })?;
    // Its data packs' Modrinth records go too, so a new world in a folder of the same
    // name doesn't inherit them. The world is gone either way.
    let _ = crate::datapacks::world_deleted(dirs, folder);
    Ok(())
}

/// The files and folders under `dir`, relative with `/`, without links. `session.lock`
/// at the top is left out: the game holds it while the world is open.
fn collect(dir: &Path) -> Result<Vec<(String, PathBuf, bool)>, WorldError> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((path, relative)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&path)
            .map_err(io_error(&path))?
            .collect::<Result<_, _>>()
            .map_err(io_error(&path))?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let kind = entry.file_type().map_err(io_error(&entry.path()))?;
            if kind.is_dir() {
                out.push((child.clone(), entry.path(), true));
                stack.push((entry.path(), child));
            } else if kind.is_file() && !(relative.is_empty() && name == "session.lock") {
                out.push((child, entry.path(), false));
            }
        }
    }
    Ok(out)
}

fn zip_time(time: Option<SystemTime>) -> zip::DateTime {
    use chrono::{Datelike as _, Timelike as _};
    time.map(chrono::DateTime::<chrono::Local>::from)
        .and_then(|t| {
            zip::DateTime::from_date_and_time(
                u16::try_from(t.year()).ok()?,
                t.month() as u8,
                t.day() as u8,
                t.hour() as u8,
                t.minute() as u8,
                t.second() as u8,
            )
            .ok()
        })
        .unwrap_or_default()
}

/// Zips the world folder `dir` to `out`, its files under `<prefix>/`. Written to a
/// temporary file next to `out`, then renamed.
pub fn write_zip(
    dir: &Path,
    prefix: &str,
    out: &Path,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<u64, WorldError> {
    let entries = collect(dir)?;
    let total_bytes: u64 = entries
        .iter()
        .filter(|(_, _, is_dir)| !is_dir)
        .filter_map(|(_, path, _)| fs::metadata(path).ok())
        .map(|m| m.len())
        .sum();
    let total_files = entries.iter().filter(|(_, _, d)| !d).count() as u64;
    let mut done = ProgressSnapshot {
        files_total: total_files,
        bytes_total: total_bytes,
        ..ProgressSnapshot::default()
    };
    progress.set(done);
    let parent = out
        .parent()
        .ok_or_else(|| WorldError::Invalid(format!("{} has no folder", out.display())))?;
    fs::create_dir_all(parent).map_err(io_error(parent))?;
    let tmp = parent.join(format!(
        ".{}.{}.part",
        out.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("world.zip"),
        random_suffix()
    ));
    let result = (|| {
        let file = File::create(&tmp).map_err(io_error(&tmp))?;
        // Buffered: the zip writer's own writes (headers, stored entries) are small.
        let mut zip = zip::ZipWriter::new(io::BufWriter::with_capacity(1 << 20, file));
        let base = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let zip_err = |err: zip::result::ZipError| {
            WorldError::Invalid(format!("couldn't write the zip: {err}"))
        };
        zip.add_directory(
            format!("{prefix}/"),
            base.last_modified_time(zip_time(fs::metadata(dir).and_then(|m| m.modified()).ok())),
        )
        .map_err(zip_err)?;
        for (relative, path, is_dir) in &entries {
            if cancel.is_cancelled() {
                return Err(WorldError::Cancelled);
            }
            let metadata = fs::metadata(path).map_err(io_error(path))?;
            let options = base
                .last_modified_time(zip_time(metadata.modified().ok()))
                .large_file(metadata.len() >= u32::MAX as u64);
            if *is_dir {
                zip.add_directory(format!("{prefix}/{relative}/"), options)
                    .map_err(zip_err)?;
                continue;
            }
            zip.start_file(format!("{prefix}/{relative}"), options)
                .map_err(zip_err)?;
            let mut source =
                io::BufReader::with_capacity(256 * 1024, File::open(path).map_err(io_error(path))?);
            let copied = io::copy(&mut source, &mut zip).map_err(io_error(path))?;
            done.files_done += 1;
            done.bytes_done += copied;
            progress.set(done);
        }
        let file = zip
            .finish()
            .map_err(zip_err)?
            .into_inner()
            .map_err(|err| io_error(&tmp)(err.into_error()))?;
        file.sync_all().map_err(io_error(&tmp))?;
        drop(file);
        crate::fsx::rename(&tmp, out).map_err(io_error(out))?;
        Ok(fs::metadata(out).map_or(0, |m| m.len()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// The local time as `2026-11-08_14-30-05`, for backup names.
pub fn timestamp(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time)
        .format("%Y-%m-%d_%H-%M-%S")
        .to_string()
}

/// The local date as `2026-11-08`.
pub fn date(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(time)
        .format("%Y-%m-%d")
        .to_string()
}

/// Backs up a world into `backups/<folder>/<time>.zip`.
pub fn backup(
    dirs: &InstanceDirs,
    folder: &str,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Backup, WorldError> {
    let dir = world_dir(dirs, folder)?;
    let target_dir = backups_dir(dirs).join(folder);
    let stamp = timestamp(SystemTime::now());
    let name = (1..)
        .map(|n| {
            if n == 1 {
                format!("{stamp}.zip")
            } else {
                format!("{stamp}-{n}.zip")
            }
        })
        .find(|name| !target_dir.join(name).exists())
        .expect("an unused name");
    let path = target_dir.join(&name);
    let size = write_zip(&dir, folder, &path, progress, cancel)?;
    Ok(Backup {
        created: fs::metadata(&path).and_then(|m| m.modified()).ok(),
        path,
        file_name: name,
        size,
    })
}

/// The backups of the world `folder`, newest first.
pub fn backups(dirs: &InstanceDirs, folder: &str) -> Vec<Backup> {
    if !is_plain_name(folder) {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(backups_dir(dirs).join(folder)) else {
        return Vec::new();
    };
    let mut list: Vec<Backup> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file_name = e.file_name().to_str()?.to_string();
            if !file_name.to_ascii_lowercase().ends_with(".zip") || file_name.starts_with('.') {
                return None;
            }
            let metadata = e.metadata().ok()?;
            Some(Backup {
                path: e.path(),
                file_name,
                size: metadata.len(),
                created: metadata.modified().ok(),
            })
        })
        .collect();
    list.sort_by(|a, b| {
        b.created
            .cmp(&a.created)
            .then(b.file_name.cmp(&a.file_name))
    });
    list
}

/// Checks that `path` is one of the instance's backups.
fn check_backup(dirs: &InstanceDirs, path: &Path) -> Result<(), WorldError> {
    let inside = path
        .parent()
        .and_then(Path::parent)
        .is_some_and(|p| p == backups_dir(dirs));
    let name_ok = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| is_plain_name(n) && n.to_ascii_lowercase().ends_with(".zip"));
    if inside && name_ok && path.is_file() {
        Ok(())
    } else {
        Err(WorldError::NotFound(format!(
            "{} isn't a backup of this instance.",
            path.display()
        )))
    }
}

/// Moves a backup to the Trash.
pub fn delete_backup(dirs: &InstanceDirs, path: &Path) -> Result<(), WorldError> {
    delete_backup_with(dirs, path, |path| {
        trash::delete(path).map_err(|e| e.to_string())
    })
}

/// Like [`delete_backup`], with `remove` doing the removal.
pub fn delete_backup_with(
    dirs: &InstanceDirs,
    path: &Path,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), WorldError> {
    check_backup(dirs, path)?;
    remove(path).map_err(|reason| WorldError::Trash {
        path: path.to_owned(),
        reason,
    })
}

/// A world zip that was checked.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldZip {
    /// `""` when `level.dat` is at the root, else `"<folder>/"`.
    pub prefix: String,
    pub summary: LevelSummary,
    /// A folder name to use: the zip's folder, or the zip's name.
    pub folder_hint: String,
}

pub(crate) fn is_mac_metadata(name: &str) -> bool {
    name.starts_with("__MACOSX/") || name.split('/').any(|c| c == ".DS_Store")
}

/// Opens a world zip and checks it: its size, and `level.dat` at the root or one folder
/// down (one world only).
pub fn inspect_zip(path: &Path) -> Result<WorldZip, WorldError> {
    let file = File::open(path).map_err(io_error(path))?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(zip_error(path))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(WorldError::Invalid(format!(
            "The zip has more than {MAX_ZIP_ENTRIES} files."
        )));
    }
    let mut total = 0u64;
    let mut candidates = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(zip_error(path))?;
        total = total.saturating_add(entry.size());
        let name = entry.name().map_err(zip_error(path))?.into_owned();
        if is_mac_metadata(&name) {
            continue;
        }
        let parts: Vec<&str> = name.split('/').collect();
        match parts.as_slice() {
            ["level.dat"] => candidates.push((String::new(), i)),
            [folder, "level.dat"] if !folder.is_empty() => {
                candidates.push((format!("{folder}/"), i))
            }
            _ => {}
        }
    }
    if total > MAX_ZIP_BYTES {
        return Err(WorldError::Invalid(
            "The zip's files are larger than 64 GB unpacked.".into(),
        ));
    }
    // A world at the root wins over folders inside it (such as datapacks).
    let chosen = match candidates.iter().find(|(prefix, _)| prefix.is_empty()) {
        Some(root) => root.clone(),
        None => match candidates.as_slice() {
            [] => {
                return Err(WorldError::Invalid(
                    "That zip isn't a world: there is no level.dat at its top or in one folder."
                        .into(),
                ));
            }
            [one] => one.clone(),
            _ => {
                return Err(WorldError::Invalid(
                    "That zip has several worlds; import them one at a time.".into(),
                ));
            }
        },
    };
    let (prefix, index) = chosen;
    let mut entry = archive.by_index(index).map_err(zip_error(path))?;
    if entry.size() > MAX_LEVEL_DAT_BYTES {
        return Err(WorldError::Invalid("Its level.dat is too large.".into()));
    }
    let mut bytes = Vec::new();
    (&mut entry)
        .take(MAX_LEVEL_DAT_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| WorldError::Invalid(format!("Its level.dat can't be read: {e}")))?;
    let summary = level::summarize(&bytes)
        .map_err(|e| WorldError::Invalid(format!("Its level.dat can't be read: {e}")))?;
    let folder_hint = match prefix.strip_suffix('/') {
        Some(folder) => folder.to_string(),
        None => path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .or_else(|| summary.name.clone())
            .unwrap_or_else(|| "World".into()),
    };
    Ok(WorldZip {
        prefix,
        summary,
        folder_hint,
    })
}

/// Extracts the world in a checked zip into `saves/<folder>` (which must not exist),
/// through a hidden folder renamed into place.
pub fn extract(
    path: &Path,
    world: &WorldZip,
    saves: &Path,
    folder: &str,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<PathBuf, WorldError> {
    extract_mapped(
        path,
        &[(world.prefix.clone(), String::new())],
        saves,
        folder,
        progress,
        cancel,
    )
}

/// Like [`extract`], for zips whose world is in several folders: each entry under one
/// of the `(zip prefix, folder in the world)` pairs goes to that folder (the first pair
/// that matches wins); other entries are left out. The world must end up with a
/// `level.dat`.
///
/// Every entry is checked before anything is written, including that no two of them
/// land on the same file or folder where names ignore case ([`check_collisions`]).
pub fn extract_mapped(
    path: &Path,
    mappings: &[(String, String)],
    saves: &Path,
    folder: &str,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<PathBuf, WorldError> {
    let target = saves.join(folder);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(WorldError::Conflict(format!("{folder} already exists.")));
    }
    let file = File::open(path).map_err(io_error(path))?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(zip_error(path))?;
    let entries = plan_extract(path, &mut archive, mappings)?;
    check_collisions(&entries)?;
    fs::create_dir_all(saves).map_err(io_error(saves))?;
    let staging = saves.join(format!(".lodestone-extract-{}", random_suffix()));
    let result = (|| {
        fs::create_dir(&staging).map_err(io_error(&staging))?;
        let mut budget = MAX_ZIP_BYTES;
        let mut done = ProgressSnapshot {
            files_total: entries.len() as u64,
            ..ProgressSnapshot::default()
        };
        for planned in &entries {
            if cancel.is_cancelled() {
                return Err(WorldError::Cancelled);
            }
            done.files_done += 1;
            progress.set(done);
            let dest = staging.join(&planned.relative);
            if planned.is_dir {
                fs::create_dir_all(&dest).map_err(io_error(&dest))?;
                continue;
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(io_error(parent))?;
            }
            // Never replace a file: two entries for one file would lose one of them.
            let mut out = match File::options().write(true).create_new(true).open(&dest) {
                Ok(out) => out,
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(WorldError::Conflict(format!(
                        "Two files in the zip would both be saved as {}, so one would be \
                         lost. Nothing was imported.",
                        planned.relative.display()
                    )));
                }
                Err(err) => return Err(io_error(&dest)(err)),
            };
            let mut entry = archive.by_index(planned.index).map_err(zip_error(path))?;
            // Never trust the size in the zip header.
            let limit = budget.min(entry.size()).saturating_add(1);
            let copied =
                io::copy(&mut (&mut entry).take(limit), &mut out).map_err(io_error(&dest))?;
            if copied > entry.size() || copied > budget {
                return Err(WorldError::Invalid(format!(
                    "{} is larger than the zip says.",
                    planned.relative.display()
                )));
            }
            budget -= copied;
            done.bytes_done += copied;
        }
        if !staging.join("level.dat").is_file() {
            return Err(WorldError::Invalid(
                "The zip's level.dat is missing.".into(),
            ));
        }
        crate::fsx::rename(&staging, &target).map_err(io_error(&target))?;
        Ok(target.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

/// An entry of a world zip that [`extract_mapped`] writes.
struct PlannedEntry {
    index: usize,
    /// The entry's name in the zip, for messages.
    name: String,
    /// Where it goes in the world, checked by [`check_pack_path`].
    relative: PathBuf,
    is_dir: bool,
}

/// The entries of `archive` that go into the world, and where. Fails on an unsafe path.
fn plan_extract(
    path: &Path,
    archive: &mut zip::ZipArchive<std::io::BufReader<File>>,
    mappings: &[(String, String)],
) -> Result<Vec<PlannedEntry>, WorldError> {
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(zip_error(path))?;
        let name = entry.name().map_err(zip_error(path))?.into_owned();
        let Some(relative) = mappings.iter().find_map(|(prefix, dest)| {
            name.strip_prefix(prefix.as_str())
                .filter(|rest| !rest.is_empty())
                .map(|rest| format!("{dest}{rest}"))
        }) else {
            continue;
        };
        if is_mac_metadata(&name) {
            continue;
        }
        let is_dir = entry.is_dir();
        let relative = match check_pack_path(&relative, is_dir, &WriteAll) {
            Ok(PathCheck::Write(relative)) => relative,
            Ok(PathCheck::Skip(_)) => continue,
            Err(err) => {
                return Err(WorldError::Invalid(
                    err.to_string()
                        .replace("The modpack is unsafe", "The zip is unsafe"),
                ));
            }
        };
        let is_link = entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000);
        if is_link {
            continue;
        }
        entries.push(PlannedEntry {
            index,
            name,
            relative,
            is_dir,
        });
    }
    Ok(entries)
}

/// Fails when two entries would land on the same path on a filesystem that ignores case
/// (as Windows and macOS do by default): `region/r.0.0.mca` and `REGION/r.0.0.mca`, a
/// or a file that is also a folder. Extracting such a zip there would silently keep only
/// one of them. Folders spelled the same way may repeat. (A name listed twice in the zip
/// is one entry: zip reads only one of them.)
fn check_collisions(entries: &[PlannedEntry]) -> Result<(), WorldError> {
    // the path with its case folded -> (as spelled, is a folder, the entry's name)
    let mut seen: HashMap<String, (String, bool, &str)> = HashMap::new();
    for entry in entries {
        let components: Vec<String> = entry
            .relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        for depth in 1..=components.len() {
            let spelled = components[..depth].join("/");
            let is_dir = depth < components.len() || entry.is_dir;
            let folded = spelled.to_lowercase();
            let Some((other, other_is_dir, other_name)) = seen.get(&folded) else {
                seen.insert(folded, (spelled, is_dir, &entry.name));
                continue;
            };
            if *other != spelled {
                return Err(WorldError::Conflict(format!(
                    "The zip has both {other_name} and {}, whose names differ only in case. \
                     On Windows and macOS one would overwrite the other, so nothing was \
                     imported.",
                    entry.name
                )));
            }
            if !(is_dir && *other_is_dir) {
                return Err(WorldError::Conflict(format!(
                    "The zip has {spelled} more than once ({other_name} and {}), so one \
                     would overwrite the other. Nothing was imported.",
                    entry.name
                )));
            }
        }
    }
    Ok(())
}

/// Sets the name inside an extracted world's `level.dat`, when it round-trips.
pub(crate) fn set_level_name(dir: &Path, name: &str) {
    let level_dat = dir.join("level.dat");
    if let Ok(bytes) = fs::read(&level_dat)
        && let Some(renamed) = level::rename(&bytes, name)
    {
        let _ = storage::write_atomic(&level_dat, &renamed);
    }
}

/// Imports a world zip into `saves/`. Returns the new folder.
pub fn import_zip(
    dirs: &InstanceDirs,
    path: &Path,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<String, WorldError> {
    let world = inspect_zip(path)?;
    let saves = saves_dir(dirs);
    let folder = unused(&saves, &folder_name(&world.folder_hint));
    extract(path, &world, &saves, &folder, progress, cancel)?;
    Ok(folder)
}

/// Exports a world as a zip to `out`.
pub fn export(
    dirs: &InstanceDirs,
    folder: &str,
    out: &Path,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<u64, WorldError> {
    let dir = world_dir(dirs, folder)?;
    write_zip(&dir, folder, out, progress, cancel)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreMode {
    /// Into a new world, `<folder> (restored <date>)`.
    Copy,
    /// Over the world, after backing it up.
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    /// The world's folder now.
    pub folder: String,
    /// The backup of the replaced world.
    pub safety_backup: Option<Backup>,
}

/// Restores a backup of the world `folder`; a replaced world goes to the Trash.
pub fn restore(
    dirs: &InstanceDirs,
    folder: &str,
    backup_path: &Path,
    mode: RestoreMode,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Restored, WorldError> {
    restore_with(
        dirs,
        folder,
        backup_path,
        mode,
        |path| trash::delete(path).map_err(|e| e.to_string()),
        progress,
        cancel,
    )
}

/// Restores a backup of the world `folder`. The replaced world is renamed
/// `<folder> (replaced <date>)` and `remove` moves it to the Trash; if it can't, it is
/// kept there.
pub fn restore_with(
    dirs: &InstanceDirs,
    folder: &str,
    backup_path: &Path,
    mode: RestoreMode,
    remove: impl FnOnce(&Path) -> Result<(), String>,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Restored, WorldError> {
    check_backup(dirs, backup_path)?;
    if !is_plain_name(folder) {
        return Err(WorldError::NotFound(format!(
            "There is no world {folder:?}."
        )));
    }
    let world = inspect_zip(backup_path)?;
    let saves = saves_dir(dirs);
    let today = date(SystemTime::now());
    match mode {
        RestoreMode::Copy => {
            let new_folder = unused(
                &saves,
                &folder_name(&format!("{folder} (restored {today})")),
            );
            let dir = extract(backup_path, &world, &saves, &new_folder, progress, cancel)?;
            let name = world
                .summary
                .name
                .clone()
                .unwrap_or_else(|| folder.to_string());
            set_level_name(&dir, &format!("{name} (restored {today})"));
            Ok(Restored {
                folder: new_folder,
                safety_backup: None,
            })
        }
        RestoreMode::Replace => {
            let current = saves.join(folder);
            let safety_backup = if current.is_dir() {
                Some(backup(dirs, folder, progress, cancel)?)
            } else {
                None
            };
            let staged = format!(".lodestone-restore-{}", random_suffix());
            let staged_dir = extract(backup_path, &world, &saves, &staged, progress, cancel)?;
            if !current.is_dir() {
                crate::fsx::rename(&staged_dir, &current).map_err(io_error(&current))?;
                return Ok(Restored {
                    folder: folder.to_string(),
                    safety_backup,
                });
            }
            let replaced = saves.join(format!(".lodestone-replaced-{}", random_suffix()));
            if let Err(err) = crate::fsx::rename(&current, &replaced) {
                let _ = fs::remove_dir_all(&staged_dir);
                return Err(io_error(&current)(err));
            }
            if let Err(err) = crate::fsx::rename(&staged_dir, &current) {
                let _ = crate::fsx::rename(&replaced, &current);
                let _ = fs::remove_dir_all(&staged_dir);
                return Err(io_error(&current)(err));
            }
            // The Trash keeps a folder's name, so it goes there as one the person
            // knows, not hidden under the swap's.
            let keep = saves.join(unused(
                &saves,
                &folder_name(&format!("{folder} (replaced {today})")),
            ));
            let replaced = match crate::fsx::rename(&replaced, &keep) {
                Ok(()) => keep.clone(),
                Err(_) => replaced,
            };
            if remove(&replaced).is_err() && replaced != keep {
                let _ = crate::fsx::rename(&replaced, &keep);
            }
            Ok(Restored {
                folder: folder.to_string(),
                safety_backup,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names() {
        assert_eq!(folder_name("My World"), "My World");
        assert_eq!(folder_name("a/b\\c:d*e?"), "a_b_c_d_e_");
        assert_eq!(folder_name("..hidden"), "hidden");
        assert_eq!(folder_name("dots..."), "dots");
        assert_eq!(folder_name("a..b"), "a_b");
        assert_eq!(folder_name("   "), "World");
        assert_eq!(folder_name("CON"), "CON_");
        assert_eq!(folder_name(&"x".repeat(100)).len(), MAX_FOLDER_CHARS);
        assert!(is_plain_name(&folder_name("line\nbreak")));
    }
}
