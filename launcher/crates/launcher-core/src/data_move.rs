//! Moving the data directory to another folder, such as a games folder on another
//! drive (Settings › Storage).
//!
//! [`run`] moves everything in the data directory to an empty folder: a rename when
//! both are on the same drive, otherwise a copy, with progress and a way to cancel.
//! It then points the default directory's `data-location.json` at the new folder
//! ([`launcher_control::paths`]), so the launcher, the CLI and the uninstaller find it,
//! and notes the old folder for removal. The launcher restarts right after, and the
//! new process removes the old folder ([`finish_pending`]) once nothing has it open.
//!
//! Until the location file is written, nothing has changed: a copy that fails or is
//! cancelled is removed again, and the data stays where it was. After it is written,
//! the copy is the data.
//!
//! Moving back to the default directory is the same move, with the location file
//! removed instead.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use launcher_control::paths::{LOCATION_FILE, SOCKET_FILE, write_location};
use serde::{Deserialize, Serialize};

use crate::disk::Cancel;

/// In the default directory: the folder to remove once the launcher has restarted
/// into the new one.
pub const CLEANUP_FILE: &str = "data-move-cleanup.json";

/// Names a target folder may already have and still count as empty: the location
/// files, and what Windows and macOS put in folders on their own.
const IGNORABLE: &[&str] = &[".DS_Store", "desktop.ini", "Thumbs.db"];

/// Top-level names that are never moved: the location files, which belong to the
/// default directory, and the control socket, which the running launcher owns.
fn stays_behind(name: &str) -> bool {
    name == LOCATION_FILE
        || name == CLEANUP_FILE
        || name == SOCKET_FILE
        || name.starts_with(&format!("{LOCATION_FILE}."))
        || name.starts_with(&format!("{CLEANUP_FILE}."))
}

/// A move to make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveRequest {
    /// The data directory now.
    pub from: PathBuf,
    /// Where it goes.
    pub to: PathBuf,
    /// The default data directory, which holds the location file.
    pub default: PathBuf,
}

/// How far a move is, in bytes. `total` is 0 until it is known.
#[derive(Debug, Default)]
pub struct MoveProgress {
    pub total: AtomicU64,
    pub done: AtomicU64,
}

impl MoveProgress {
    pub fn fraction(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.done.load(Ordering::Relaxed) as f32 / total as f32).min(1.0))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("cancelled")]
    Cancelled,
    #[error("choose a folder by its full path")]
    NotAbsolute,
    #[error("the data is already in that folder")]
    Same,
    #[error("the new folder can't be inside the current data folder")]
    InsideData,
    #[error("{path} isn't empty. Choose an empty folder, or make a new one")]
    NotEmpty { path: String },
    #[error("can't write to {path}: {reason}")]
    NotWritable { path: String, reason: String },
    #[error("the drive has {} free, and the data needs {}", gigabytes(*free), gigabytes(*needed))]
    NoRoom { needed: u64, free: u64 },
    #[error("couldn't copy {path}: {reason}")]
    Copy { path: String, reason: String },
    #[error("couldn't save the new location: {0}")]
    Location(String),
}

fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

impl MoveError {
    fn copy(path: &Path, err: impl std::fmt::Display) -> Self {
        Self::Copy {
            path: path.display().to_string(),
            reason: err.to_string(),
        }
    }
}

/// The checks a move needs before it starts, quick enough for the UI to run before
/// asking: a full path, not the data folder or inside it, empty, writable.
pub fn check(request: &MoveRequest) -> Result<(), MoveError> {
    let MoveRequest { from, to, default } = request;
    if !to.is_absolute() {
        return Err(MoveError::NotAbsolute);
    }
    let (from_key, to_key) = (comparable(from), comparable(to));
    if from_key == to_key {
        return Err(MoveError::Same);
    }
    if to_key.starts_with(&from_key) {
        return Err(MoveError::InsideData);
    }
    if to.exists() {
        // Back to the default directory, which keeps the location file.
        let allowed_default = comparable(default) == to_key;
        if !is_empty_enough(to, allowed_default).map_err(|err| not_writable(to, err))? {
            return Err(MoveError::NotEmpty {
                path: to.display().to_string(),
            });
        }
    }
    probe_writable(to)
}

/// The folder to move to when the person chose `chosen`: `chosen` itself when it is
/// empty, otherwise a "Lodestone Launcher" folder in it, so choosing a games folder
/// on another drive works. Checked like [`check`].
pub fn target_for(chosen: &Path, from: &Path, default: &Path) -> Result<PathBuf, MoveError> {
    let request = |to: &Path| MoveRequest {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        default: default.to_path_buf(),
    };
    match check(&request(chosen)) {
        Ok(()) => Ok(chosen.to_path_buf()),
        Err(MoveError::NotEmpty { .. }) => {
            let inside = chosen.join(launcher_control::paths::APP_DIR_NAME);
            check(&request(&inside)).map(|()| inside)
        }
        Err(err) => Err(err),
    }
}

/// Moves the data in `request.from` to `request.to` and points the launcher at it.
/// The caller restarts the launcher right after a success.
pub fn run(
    request: &MoveRequest,
    progress: &MoveProgress,
    cancel: &Cancel,
) -> Result<(), MoveError> {
    check(request)?;
    if cancel.is_cancelled() {
        return Err(MoveError::Cancelled);
    }
    let MoveRequest { from, to, default } = request;

    // Same drive: one rename, when the target doesn't exist yet (an empty folder is
    // removed first). A file open in the data folder makes Windows refuse it; then
    // it is copied like across drives.
    if try_rename(from, to) {
        // The running launcher's socket; the restarted one makes its own.
        let _ = fs::remove_file(to.join(SOCKET_FILE));
        progress.total.store(1, Ordering::Relaxed);
        progress.done.store(1, Ordering::Relaxed);
        return point_at(request);
    }

    let total = measure(from, cancel)?;
    progress.total.store(total.max(1), Ordering::Relaxed);
    if let Some(parent) = existing_ancestor(to)
        && let Some(free) = crate::snapshots::free_space(&parent)
        && total > free
    {
        return Err(MoveError::NoRoom {
            needed: total,
            free,
        });
    }

    let created = !to.exists();
    let copied = fs::create_dir_all(to)
        .map_err(|err| not_writable(to, err))
        .and_then(|()| copy_dir(from, to, true, progress, cancel));
    if let Err(err) = copied.and_then(|()| point_at(request)) {
        undo_copy(to, created, comparable(default) == comparable(to));
        return Err(err);
    }
    Ok(())
}

/// Writes the location file for `request.to`, and notes `request.from` for removal.
fn point_at(request: &MoveRequest) -> Result<(), MoveError> {
    let MoveRequest { from, to, default } = request;
    let target = (comparable(default) != comparable(to)).then_some(to.as_path());
    write_location(default, target).map_err(|err| MoveError::Location(err.to_string()))?;
    // Only the old folder's leftovers depend on this; the move itself is done.
    let cleanup = Cleanup {
        version: 1,
        remove: from.clone(),
        into: to.clone(),
    };
    if let Err(err) = write_cleanup(default, &cleanup) {
        eprintln!(
            "lodestone-launcher: moved the data, but couldn't note {} for removal: {err}",
            from.display()
        );
    }
    Ok(())
}

/// [`CLEANUP_FILE`]'s contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Cleanup {
    version: u32,
    /// The old data folder.
    remove: PathBuf,
    /// The folder the data moved to. Nothing is removed unless the launcher runs
    /// from it.
    into: PathBuf,
}

fn write_cleanup(default: &Path, cleanup: &Cleanup) -> io::Result<()> {
    fs::create_dir_all(default)?;
    let text = serde_json::to_string_pretty(cleanup).map_err(io::Error::other)?;
    let partial = default.join(format!("{CLEANUP_FILE}.part"));
    fs::write(&partial, text)?;
    crate::fsx::rename(&partial, default.join(CLEANUP_FILE))
}

/// What [`finish_pending`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum Finished {
    /// No move was waiting.
    Nothing,
    /// The old folder is gone.
    Removed(PathBuf),
    /// The record didn't match where the launcher runs from; it was dropped and
    /// nothing was removed.
    Ignored,
    /// Some of the old folder couldn't be removed (a file in use); the next start
    /// tries again.
    Failed { path: PathBuf, error: String },
}

/// Removes the folder a move left behind, if one did and the launcher now runs from
/// the folder it moved to (`data_dir`). Run it at startup, after an earlier launcher
/// has quit. When the old folder was the default directory, only the location files
/// stay.
pub fn finish_pending(default: &Path, data_dir: &Path) -> Finished {
    let record = default.join(CLEANUP_FILE);
    let Ok(text) = fs::read_to_string(&record) else {
        return Finished::Nothing;
    };
    let drop_record = || {
        let _ = fs::remove_file(&record);
    };
    let Ok(cleanup) = serde_json::from_str::<Cleanup>(&text) else {
        drop_record();
        return Finished::Ignored;
    };
    let (remove, into, here) = (
        comparable(&cleanup.remove),
        comparable(&cleanup.into),
        comparable(data_dir),
    );
    // Only from the folder the data moved to, and never a folder that holds it or
    // that it holds.
    let safe = into == here
        && data_dir.is_dir()
        && remove != here
        && !here.starts_with(&remove)
        && !remove.starts_with(&here)
        && cleanup.remove.is_absolute();
    if !safe {
        drop_record();
        return Finished::Ignored;
    }
    let result = if remove == comparable(default) {
        remove_contents(&cleanup.remove, stays_behind)
    } else {
        match fs::remove_dir_all(&cleanup.remove) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    };
    match result {
        Ok(()) => {
            drop_record();
            Finished::Removed(cleanup.remove)
        }
        Err(err) => Finished::Failed {
            path: cleanup.remove,
            error: err.to_string(),
        },
    }
}

/// Removes everything in `dir` except the top-level names `keep` accepts. Links are
/// removed, never followed.
fn remove_contents(dir: &Path, keep: impl Fn(&str) -> bool) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        other => other?,
    };
    let mut first_error = None;
    for entry in entries {
        let entry = entry?;
        if keep(&entry.file_name().to_string_lossy()) {
            continue;
        }
        if let Err(err) = remove_entry(&entry.path()) {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Removes a file, a link (without following it) or a folder and what is in it.
fn remove_entry(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else if meta.is_dir() {
        // A directory link on Windows (junction or symbolic link).
        fs::remove_dir(path).or_else(|_| fs::remove_file(path))
    } else {
        crate::fsx::remove_file(path).or_else(|err| {
            // A link to a folder on Windows is removed like a folder.
            fs::remove_dir(path).map_err(|_| err)
        })
    }
}

/// Renames `from` to `to` when `to` doesn't exist or is an empty folder. Returns
/// whether it did; when it didn't, nothing changed.
fn try_rename(from: &Path, to: &Path) -> bool {
    let removed_empty = to.is_dir() && fs::remove_dir(to).is_ok();
    if to.exists() {
        return false;
    }
    if let Some(parent) = to.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    if fs::rename(from, to).is_ok() {
        return true;
    }
    if removed_empty {
        let _ = fs::create_dir(to);
    }
    false
}

/// The bytes [`copy_dir`] will copy.
fn measure(dir: &Path, cancel: &Cancel) -> Result<u64, MoveError> {
    fn walk(dir: &Path, top: bool, cancel: &Cancel) -> Result<u64, MoveError> {
        if cancel.is_cancelled() {
            return Err(MoveError::Cancelled);
        }
        let mut bytes = 0;
        for entry in fs::read_dir(dir).map_err(|err| MoveError::copy(dir, err))? {
            let entry = entry.map_err(|err| MoveError::copy(dir, err))?;
            if top && stays_behind(&entry.file_name().to_string_lossy()) {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|err| MoveError::copy(&entry.path(), err))?;
            if kind.is_dir() {
                bytes += walk(&entry.path(), false, cancel)?;
            } else if kind.is_file() {
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        Ok(bytes)
    }
    walk(dir, true, cancel)
}

/// Copies what is in `from` into the existing folder `to`, keeping modification
/// times and read-only flags. Links are copied as links.
fn copy_dir(
    from: &Path,
    to: &Path,
    top: bool,
    progress: &MoveProgress,
    cancel: &Cancel,
) -> Result<(), MoveError> {
    for entry in fs::read_dir(from).map_err(|err| MoveError::copy(from, err))? {
        if cancel.is_cancelled() {
            return Err(MoveError::Cancelled);
        }
        let entry = entry.map_err(|err| MoveError::copy(from, err))?;
        let name = entry.file_name();
        if top && stays_behind(&name.to_string_lossy()) {
            continue;
        }
        let (source, target) = (entry.path(), to.join(&name));
        let kind = entry
            .file_type()
            .map_err(|err| MoveError::copy(&source, err))?;
        if kind.is_symlink() {
            copy_link(&source, &target).map_err(|err| MoveError::copy(&source, err))?;
        } else if kind.is_dir() {
            fs::create_dir(&target).map_err(|err| MoveError::copy(&target, err))?;
            copy_dir(&source, &target, false, progress, cancel)?;
        } else {
            copy_file(&source, &target, progress, cancel)?;
        }
    }
    Ok(())
}

/// Copies one file in pieces, counting them and stopping when cancelled.
fn copy_file(
    source: &Path,
    target: &Path,
    progress: &MoveProgress,
    cancel: &Cancel,
) -> Result<(), MoveError> {
    use io::{Read as _, Write as _};
    let fail = |err: io::Error| MoveError::copy(source, err);
    let meta = fs::metadata(source).map_err(fail)?;
    let mut input = fs::File::open(source).map_err(fail)?;
    let mut output = fs::File::create(target).map_err(|err| MoveError::copy(target, err))?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        if cancel.is_cancelled() {
            return Err(MoveError::Cancelled);
        }
        let read = input.read(&mut buffer).map_err(fail)?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|err| MoveError::copy(target, err))?;
        progress.done.fetch_add(read as u64, Ordering::Relaxed);
    }
    output.flush().map_err(|err| MoveError::copy(target, err))?;
    if let Ok(modified) = meta.modified() {
        let _ = output.set_modified(modified);
    }
    drop(output);
    if meta.permissions().readonly() {
        let _ = fs::set_permissions(target, meta.permissions());
    }
    Ok(())
}

#[cfg(unix)]
fn copy_link(source: &Path, target: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(source)?, target)
}

#[cfg(windows)]
fn copy_link(source: &Path, target: &Path) -> io::Result<()> {
    let link = fs::read_link(source)?;
    if fs::metadata(source).map(|m| m.is_dir()).unwrap_or(false) {
        std::os::windows::fs::symlink_dir(link, target)
    } else {
        std::os::windows::fs::symlink_file(link, target)
    }
}

#[cfg(not(any(unix, windows)))]
fn copy_link(source: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::other(format!(
        "can't copy the link {}",
        source.display()
    )))
}

/// Removes a copy that didn't finish: the whole folder when the move made it,
/// otherwise what was copied into it.
fn undo_copy(to: &Path, created: bool, is_default: bool) {
    let result = if created {
        fs::remove_dir_all(to)
    } else if is_default {
        remove_contents(to, stays_behind)
    } else {
        remove_contents(to, |name| IGNORABLE.contains(&name))
    };
    if let Err(err) = result {
        eprintln!(
            "lodestone-launcher: couldn't remove the unfinished copy in {}: {err}",
            to.display()
        );
    }
}

/// Whether `dir` has nothing in it but what Windows or macOS add on their own (and,
/// in the default directory, the location files).
fn is_empty_enough(dir: &Path, is_default: bool) -> io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if IGNORABLE.contains(&name.as_ref()) || (is_default && stays_behind(&name)) {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

/// Makes `dir` (if it doesn't exist) and writes a file in it, then removes both
/// again: the closest check that the move will be able to write there.
fn probe_writable(dir: &Path) -> Result<(), MoveError> {
    let made = existing_ancestor(dir);
    let result = fs::create_dir_all(dir).and_then(|()| {
        let probe = dir.join(".lodestone-write-test");
        fs::write(&probe, b"test")?;
        fs::remove_file(&probe)
    });
    // Remove the folders made for the check, deepest first.
    if let Some(made) = made {
        let mut current = dir.to_path_buf();
        while current != made && current.starts_with(&made) {
            let _ = fs::remove_dir(&current);
            if !current.pop() {
                break;
            }
        }
    }
    result.map_err(|err| not_writable(dir, err))
}

fn not_writable(dir: &Path, err: io::Error) -> MoveError {
    MoveError::NotWritable {
        path: dir.display().to_string(),
        reason: err.to_string(),
    }
}

/// The closest folder at or above `path` that exists.
fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.is_dir()).map(Path::to_path_buf)
}

/// `path` for comparing: without trailing separators, and on Windows and macOS,
/// whose file systems ignore case, in lower case.
fn comparable(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    let trimmed = text.trim_end_matches(['/', '\\']);
    let trimmed = if trimmed.is_empty() {
        text.as_ref()
    } else {
        trimmed
    };
    if cfg!(any(windows, target_os = "macos")) {
        PathBuf::from(trimmed.to_lowercase())
    } else {
        PathBuf::from(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        _temp: tempfile::TempDir,
        default: PathBuf,
        elsewhere: PathBuf,
    }

    fn world() -> World {
        let temp = tempfile::tempdir().unwrap();
        let default = temp.path().join("Lodestone Launcher");
        let elsewhere = temp.path().join("other drive");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        World {
            _temp: temp,
            default,
            elsewhere,
        }
    }

    /// A data folder with settings, an instance with a world, and a read-only file.
    fn fill(dir: &Path) {
        let world = dir.join("instances/survival/minecraft/saves/World");
        fs::create_dir_all(&world).unwrap();
        fs::write(dir.join("settings.json"), "{}").unwrap();
        fs::write(world.join("level.dat"), vec![7u8; 3 << 20]).unwrap();
        fs::write(dir.join(SOCKET_FILE), "").unwrap();
        let readonly = dir.join("instances/survival/instance.json");
        fs::write(&readonly, "{}").unwrap();
        let mut permissions = fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&readonly, permissions).unwrap();
    }

    fn assert_filled(dir: &Path) {
        assert_eq!(fs::read_to_string(dir.join("settings.json")).unwrap(), "{}");
        let level = dir.join("instances/survival/minecraft/saves/World/level.dat");
        assert_eq!(fs::metadata(level).unwrap().len(), 3 << 20);
        assert!(
            fs::metadata(dir.join("instances/survival/instance.json"))
                .unwrap()
                .permissions()
                .readonly()
        );
    }

    fn request(from: &Path, to: &Path, default: &Path) -> MoveRequest {
        MoveRequest {
            from: from.into(),
            to: to.into(),
            default: default.into(),
        }
    }

    #[test]
    fn refuses_bad_targets() {
        let w = world();
        fill(&w.default);
        let check_to = |to: &Path| check(&request(&w.default, to, &w.default));
        assert!(matches!(
            check_to(Path::new("relative")),
            Err(MoveError::NotAbsolute)
        ));
        assert!(matches!(check_to(&w.default), Err(MoveError::Same)));
        assert!(matches!(
            check_to(&w.default.join("instances/x")),
            Err(MoveError::InsideData)
        ));
        fs::write(w.elsewhere.join("someone's file.txt"), "hi").unwrap();
        assert!(matches!(
            check_to(&w.elsewhere),
            Err(MoveError::NotEmpty { .. })
        ));
        // A new folder, or an empty one with what Windows adds, is fine, and the
        // check leaves nothing behind.
        let new = w.elsewhere.join("Games/Lodestone");
        check_to(&new).unwrap();
        assert!(!w.elsewhere.join("Games").exists());
        let empty = w.elsewhere.join("Empty");
        fs::create_dir(&empty).unwrap();
        fs::write(empty.join("desktop.ini"), "").unwrap();
        check_to(&empty).unwrap();
    }

    #[test]
    fn a_games_folder_gets_a_folder_of_its_own() {
        let w = world();
        fill(&w.default);
        fs::write(w.elsewhere.join("other game.exe"), "").unwrap();
        let to = target_for(&w.elsewhere, &w.default, &w.default).unwrap();
        assert_eq!(to, w.elsewhere.join("Lodestone Launcher"));
        let empty = w.elsewhere.join("Empty");
        fs::create_dir(&empty).unwrap();
        assert_eq!(target_for(&empty, &w.default, &w.default).unwrap(), empty);
        // Choosing the data folder itself, or a folder in it, is still refused.
        assert!(matches!(
            target_for(&w.default, &w.default, &w.default),
            Err(MoveError::Same)
        ));
    }

    #[test]
    fn moves_to_another_folder_and_back() {
        let w = world();
        fill(&w.default);
        let to = w.elsewhere.join("Lodestone");
        let progress = MoveProgress::default();
        run(
            &request(&w.default, &to, &w.default),
            &progress,
            &Cancel::new(),
        )
        .unwrap();
        assert_filled(&to);
        assert!(!to.join(SOCKET_FILE).exists(), "the socket stays behind");
        assert_eq!(
            launcher_control::paths::read_location(&w.default),
            Some(to.clone())
        );

        // The restarted launcher removes the old data, keeping the location file.
        assert_eq!(
            finish_pending(&w.default, &to),
            Finished::Removed(w.default.clone())
        );
        let left: Vec<_> = fs::read_dir(&w.default)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec![LOCATION_FILE.to_string()]);
        assert_eq!(finish_pending(&w.default, &to), Finished::Nothing);

        // And back to the default folder.
        run(
            &request(&to, &w.default, &w.default),
            &progress,
            &Cancel::new(),
        )
        .unwrap();
        assert_filled(&w.default);
        assert_eq!(launcher_control::paths::read_location(&w.default), None);
        assert_eq!(
            finish_pending(&w.default, &w.default),
            Finished::Removed(to.clone())
        );
        assert!(!to.exists());
    }

    #[test]
    fn copies_when_it_cant_rename() {
        let w = world();
        fill(&w.default);
        // An existing, non-empty target can't be renamed onto; this one only holds
        // what Windows adds, so it's copied into.
        let to = w.elsewhere.join("Lodestone");
        fs::create_dir(&to).unwrap();
        fs::write(to.join("Thumbs.db"), "").unwrap();
        let progress = MoveProgress::default();
        run(
            &request(&w.default, &to, &w.default),
            &progress,
            &Cancel::new(),
        )
        .unwrap();
        assert_filled(&to);
        assert_eq!(progress.fraction(), Some(1.0));
        assert!(progress.total.load(Ordering::Relaxed) >= 3 << 20);
    }

    #[test]
    fn a_cancelled_move_changes_nothing() {
        let w = world();
        fill(&w.default);
        let to = w.elsewhere.join("Lodestone");
        fs::create_dir(&to).unwrap();
        fs::write(to.join("desktop.ini"), "").unwrap();
        let cancel = Cancel::new();
        cancel.cancel();
        let result = run(
            &request(&w.default, &to, &w.default),
            &MoveProgress::default(),
            &cancel,
        );
        assert!(matches!(result, Err(MoveError::Cancelled)), "{result:?}");
        assert_filled(&w.default);
        assert_eq!(launcher_control::paths::read_location(&w.default), None);
        let left: Vec<_> = fs::read_dir(&to).unwrap().collect();
        assert_eq!(left.len(), 1, "only desktop.ini is left");
        assert_eq!(finish_pending(&w.default, &w.default), Finished::Nothing);
    }

    #[test]
    fn never_removes_the_folder_in_use() {
        let w = world();
        fill(&w.default);
        let to = w.elsewhere.join("Lodestone");
        // The record says the data moved to `to`, but the launcher runs from the
        // default folder (the move was undone by hand, say): nothing is removed.
        write_cleanup(
            &w.default,
            &Cleanup {
                version: 1,
                remove: w.default.clone(),
                into: to.clone(),
            },
        )
        .unwrap();
        assert_eq!(finish_pending(&w.default, &w.default), Finished::Ignored);
        assert_filled(&w.default);
        assert!(!w.default.join(CLEANUP_FILE).exists());
    }
}
