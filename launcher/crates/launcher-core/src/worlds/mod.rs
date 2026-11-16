//! An instance's worlds (`minecraft/saves/`): listing them with what their `level.dat`
//! says, renaming, deleting (to the Trash), and world zips. A world's backups are zips
//! too ([`crate::world_backups`]); snapshots hold every world of the instance
//! ([`crate::images`]).
//!
//! - **World zips** (export, import, backups) are written and checked by [`zip`]; an
//!   imported zip needs `level.dat` at its root or one folder down.
//!
//! Nothing here checks whether the game is running; the app refuses changes then.

pub mod zip;

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::level::{self, LevelSummary, MAX_LEVEL_DAT_BYTES};

use crate::{
    content::{InstanceDirs, is_plain_name},
    game::download::{CancelToken, Progress},
    storage,
};

pub use zip::{CheckedZip, Limits, MAX_ZIP_BYTES, MAX_ZIP_ENTRIES, Shape, Written};

pub const SAVES: &str = "saves";
/// What an unpacked world must leave free on its disk, besides its own size.
pub const SPARE_BYTES: u64 = 1 << 30;
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
    /// The disk lacks room: `needed` bytes asked for, `free` there.
    #[error("It needs {} MB and the disk has {} MB free", .needed.div_ceil(1_000_000), .free / 1_000_000)]
    NoRoom { needed: u64, free: u64 },
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> WorldError + '_ {
    move |source| WorldError::Io {
        path: path.to_owned(),
        source,
    }
}

/// `rel` (a `/`-separated relative path) under `base`.
pub(crate) fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Refuses when the disk holding `dir` (or its nearest folder that exists) has less than
/// `bytes` and [`SPARE_BYTES`] free. A disk whose free space can't be read passes.
pub(crate) fn check_room(dir: &Path, bytes: u64, free: Option<u64>) -> Result<(), WorldError> {
    let free = free.or_else(|| {
        dir.ancestors()
            .find(|d| d.is_dir())
            .and_then(crate::disk::free_space)
    });
    let needed = bytes.saturating_add(SPARE_BYTES);
    match free {
        Some(free) if free < needed => Err(WorldError::NoRoom { needed, free }),
        _ => Ok(()),
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

pub fn saves_dir(dirs: &InstanceDirs) -> PathBuf {
    dirs.game.join(SAVES)
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

pub(crate) fn random_suffix() -> String {
    format!("{:016x}", u64::from_le_bytes(crate::random_bytes()))
}

/// Renames a world: its folder (to [`folder_name`] of `name`) and, when `level.dat`
/// reads and writes back unchanged, the name inside it. Returns the new folder. If
/// writing `level.dat` fails, the world is moved back and the error returned.
///
/// Snapshots know a world by its folder: the Worlds tab lists the older snapshots of a
/// renamed world under no world (they stay in Settings › Snapshots, and putting the world
/// back from one restores the old folder name).
pub fn rename(dirs: &InstanceDirs, folder: &str, name: &str) -> Result<String, WorldError> {
    rename_with(dirs, folder, name, |from, to| crate::fsx::rename(from, to))
}

/// Like [`rename`], with `move_dir` moving the world's folder (tests make it fail).
pub fn rename_with(
    dirs: &InstanceDirs,
    folder: &str,
    name: &str,
    move_dir: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<String, WorldError> {
    rename_in(dirs, folder, name, &[], move_dir)
}

/// Like [`rename_with`], with the new folder a name free in `saves/` and in each of
/// `taken` too (the world backups' folder, whose world folders follow the world).
/// `move_dir` is called with the world's folder and its new place, and again the other
/// way round to put it back when its `level.dat` can't be renamed.
pub fn rename_in(
    dirs: &InstanceDirs,
    folder: &str,
    name: &str,
    taken: &[&Path],
    mut move_dir: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<String, WorldError> {
    let dir = world_dir(dirs, folder)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(WorldError::Invalid("Give the world a name.".into()));
    }
    let saves = saves_dir(dirs);
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
    let mut free_in: Vec<&Path> = vec![&saves];
    free_in.extend_from_slice(taken);
    let new_folder = unused_in(&free_in, &wanted);
    let target = saves.join(&new_folder);
    move_dir(&dir, &target).map_err(io_error(&dir))?;
    let written = match &renamed_level {
        Some(renamed) => {
            let path = level_dat(&target);
            storage::write_atomic(&path, renamed).map_err(io_error(&path))
        }
        None => Ok(()),
    };
    let Err(err) = written else {
        // The data packs installed from Modrinth are remembered by world folder. The
        // world is renamed either way; if the record can't follow, its packs only stop
        // updating in place.
        let _ = crate::datapacks::world_renamed(dirs, folder, &new_folder);
        return Ok(new_folder);
    };
    Err(if move_dir(&target, &dir).is_ok() {
        WorldError::Invalid(format!(
            "Couldn't rename the world, so it was left as it was: {err}"
        ))
    } else {
        WorldError::Invalid(format!(
            "Couldn't rename the world ({err}), and couldn't put it back as it was. \
             The world is in {}.",
            target.display()
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

/// The first of the worlds in `saves` (only those named in `only`, when given) that a game
/// has open, by folder name: one whose `session.lock` another process holds locked.
/// Minecraft 1.16 and later lock it for as long as the world is open, so this sees a game
/// that runs although this Launcher didn't start it (it was restarted while the game ran,
/// or a game started by hand). Older versions lock nothing, and a world that can't be read
/// counts as closed.
pub fn open_in_game(saves: &Path, only: Option<&[String]>) -> Option<String> {
    let names: Vec<String> = match only {
        Some(names) => names.to_vec(),
        None => fs::read_dir(saves)
            .ok()?
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect(),
    };
    names
        .into_iter()
        .find(|name| session_lock_held(&saves.join(name).join("session.lock")))
}

/// Whether another process holds `lock` (a world's `session.lock`) locked. Java's
/// `FileChannel.tryLock`, which the game uses, takes an `fcntl` lock on Unix, which
/// `F_GETLK` reports.
#[cfg(unix)]
fn session_lock_held(lock: &Path) -> bool {
    use std::os::fd::AsRawFd as _;
    let Ok(file) = fs::File::open(lock) else {
        return false;
    };
    // SAFETY: a zeroed flock is a valid value to fill in.
    let mut query: libc::flock = unsafe { std::mem::zeroed() };
    query.l_type = libc::F_WRLCK as _;
    query.l_whence = libc::SEEK_SET as _;
    query.l_start = 0;
    query.l_len = 0;
    // SAFETY: `file` is open for as long as the call runs, and `query` is a valid flock
    // the call writes the conflicting lock into.
    let asked = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut query) };
    asked == 0 && query.l_type != libc::F_UNLCK as libc::c_short
}

/// See the Unix version. On Windows the game keeps the file open, and opening it without
/// sharing fails while it does.
#[cfg(windows)]
fn session_lock_held(lock: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    match fs::OpenOptions::new().read(true).share_mode(0).open(lock) {
        Ok(_) => false,
        Err(e) => matches!(
            e.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
        ),
    }
}

#[cfg(not(any(unix, windows)))]
fn session_lock_held(_: &Path) -> bool {
    false
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

/// Sets the name inside an extracted world's `level.dat`, when it round-trips.
pub(crate) fn set_level_name(dir: &Path, name: &str) {
    let level_dat = dir.join("level.dat");
    if let Ok(bytes) = fs::read(&level_dat)
        && let Some(renamed) = level::rename(&bytes, name)
    {
        let _ = storage::write_atomic(&level_dat, &renamed);
    }
}

/// Imports a world zip into `saves/`: checked whole first, unpacked into a hidden folder
/// there and renamed into place. Returns the new folder.
pub fn import_zip(
    dirs: &InstanceDirs,
    path: &Path,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<String, WorldError> {
    let checked = zip::check(path, Shape::Import, Limits::default())?;
    let saves = saves_dir(dirs);
    fs::create_dir_all(&saves).map_err(io_error(&saves))?;
    check_room(&saves, checked.declared_bytes, None)?;
    let staging = saves.join(format!(".lodestone-extract-{}", random_suffix()));
    zip::extract_into(path, &checked, &staging, progress, &|| {
        cancel.is_cancelled()
    })?;
    let folder = unused(&saves, &folder_name(&checked.folder_hint));
    let target = saves.join(&folder);
    if let Err(e) = crate::fsx::rename(&staging, &target) {
        let _ = fs::remove_dir_all(&staging);
        return Err(io_error(&target)(e));
    }
    Ok(folder)
}

/// Exports a world as a zip to `out`, returning the zip's size.
pub fn export(
    dirs: &InstanceDirs,
    folder: &str,
    out: &Path,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<u64, WorldError> {
    let dir = world_dir(dirs, folder)?;
    let stop = || {
        if cancel.is_cancelled() {
            Err(WorldError::Cancelled)
        } else {
            Ok(())
        }
    };
    Ok(zip::write_zip(&dir, folder, out, progress, &stop)?.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Not a test on its own: run by [`a_world_another_process_has_locked_is_open_in_game`]
    /// as a child process, it locks the file named by `LODESTONE_TEST_HOLD_LOCK` as the game
    /// does (an `fcntl` lock), says so on stdout and holds it until stdin closes.
    #[cfg(unix)]
    #[test]
    fn hold_session_lock_helper() {
        use std::io::{Read as _, Write as _};
        use std::os::fd::AsRawFd as _;
        let Some(path) = std::env::var_os("LODESTONE_TEST_HOLD_LOCK") else {
            return;
        };
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let mut lock: libc::flock = unsafe { std::mem::zeroed() };
        lock.l_type = libc::F_WRLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        // SAFETY: a valid fd and flock.
        assert_eq!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
            0
        );
        println!("LOCKED");
        std::io::stdout().flush().unwrap();
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
    }

    #[cfg(unix)]
    #[test]
    fn a_world_another_process_has_locked_is_open_in_game() {
        use std::io::BufRead as _;
        let temp = tempfile::tempdir().unwrap();
        let saves = temp.path().join("saves");
        for world in ["Open", "Closed"] {
            fs::create_dir_all(saves.join(world)).unwrap();
            fs::write(saves.join(world).join("session.lock"), "☃").unwrap();
        }
        fs::create_dir_all(saves.join("No lock")).unwrap();
        assert_eq!(open_in_game(&saves, None), None);
        // a lock this process holds doesn't count: fcntl locks are per process
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worlds::tests::hold_session_lock_helper",
                "--nocapture",
            ])
            .env("LODESTONE_TEST_HOLD_LOCK", saves.join("Open/session.lock"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut out = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("LOCKED") {
            line.clear();
            assert!(out.read_line(&mut line).unwrap() > 0, "the helper ended");
        }
        assert_eq!(open_in_game(&saves, None).as_deref(), Some("Open"));
        assert_eq!(
            open_in_game(&saves, Some(&["Closed".into(), "Open".into()])).as_deref(),
            Some("Open")
        );
        assert_eq!(open_in_game(&saves, Some(&["Closed".into()])), None);
        drop(child.stdin.take());
        child.wait().unwrap();
        assert_eq!(open_in_game(&saves, None), None);
        assert_eq!(open_in_game(&temp.path().join("missing"), None), None);
    }

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
