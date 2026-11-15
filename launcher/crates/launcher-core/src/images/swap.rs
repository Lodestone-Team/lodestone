//! The journaled swap a restore ends with, and its recovery when the Launcher starts: the
//! core's (`core/src/backups/restore.rs`) for an instance folder.
//!
//! A restore stages the files it puts back in `<instance>/.lodestone-restore-staging-<16>/`
//! (laid out as the instance folder is: `minecraft/…`), then swaps each **owned** path (a
//! world's folder, a content folder, a pack file, a top-level entry of the game folder)
//! for its staged copy:
//!
//! 1. the journal (`images/<instance>/restore-journal.json`) is written, phase
//!    `moving_old`;
//! 2. each owned path that exists is renamed into `.lodestone-restore-old-<16>/`;
//! 3. the journal says `moving_new`; each staged path is renamed into place;
//! 4. the **carried** paths (inside an owned folder, what the restore never touches:
//!    `session.lock`, files no snapshot could take, links, folders it changes nothing in)
//!    are moved back from the old folder into the new one; where a staged copy is in the
//!    way, the one on disk wins and the staged one goes to the staging folder's
//!    `.replaced/`;
//! 5. the folders are flushed, the journal says `done`, the work folders and the journal
//!    are deleted.
//!
//! A failure in steps 2 to 4 rolls back (what was moved goes back; nothing at a
//! destination is ever deleted, what is in the way goes into the staging folder). A crash
//! leaves the journal, and [`recover`] does the same rollback at the next start, before any
//! instance can be played; a `done` journal only has its leftovers deleted. The journal
//! holds only the folders' random suffixes and relative paths, checked strictly before
//! use, and no rename goes through a link.
//!
//! Launcher-local, not shared with the core: the core's swap also carries server software
//! adoption, Paper's split worlds and pinned settings.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ImageError;

/// The staging folder's name, before its 16 hex digits.
pub const STAGING_PREFIX: &str = ".lodestone-restore-staging-";
/// The folder the replaced paths go to, before its 16 hex digits.
pub const OLD_PREFIX: &str = ".lodestone-restore-old-";
/// The journal in an instance's image folder.
pub const JOURNAL_FILE: &str = "restore-journal.json";
/// Where a staged copy goes when the carried path on disk wins over it.
const REPLACED_DIR: &str = ".replaced";

/// Why a swap failed.
#[derive(Debug)]
pub enum SwapError {
    /// Undone: the instance folder is as it was.
    RolledBack(ImageError),
    /// Couldn't be undone. The journal stays for the next start, which tries again; the
    /// instance mustn't be played until then.
    Broken(ImageError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Moving the owned paths into the old folder.
    MovingOld,
    /// Moving the staged paths into place, then the carried ones back into them.
    MovingNew,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Relative to the instance folder. An entry inside another one is a carried path.
    pub path: String,
    pub had_old: bool,
    pub had_new: bool,
}

/// `images/<instance>/restore-journal.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub phase: Phase,
    /// The 16 hex digits of `.lodestone-restore-staging-<16>`.
    pub staging: String,
    /// The 16 hex digits of `.lodestone-restore-old-<16>`.
    pub old: String,
    pub entries: Vec<JournalEntry>,
}

/// A journal is small; one larger than this is damaged.
const MAX_JOURNAL_BYTES: u64 = 64 << 20;

/// Simulated failures for the tests.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Faults {
    /// Stop as if the process died after this many renames.
    pub crash_after: Option<usize>,
    /// Fail the rename after this many.
    pub fail_after: Option<usize>,
    /// Make the rollback fail too.
    pub break_rollback: bool,
}

/// A restore's files staged inside the instance folder, ready to swap in.
#[derive(Debug)]
pub(crate) struct Staged {
    root: PathBuf,
    staging: String,
    old: String,
    owned: Vec<JournalEntry>,
    carried: Vec<String>,
    #[cfg(test)]
    pub(crate) faults: Faults,
}

enum MoveError {
    Failed(ImageError),
    #[cfg(test)]
    Crash,
}

/// A new random suffix for a work folder.
pub(crate) fn random_suffix() -> String {
    format!("{:016x}", u64::from_le_bytes(crate::random_bytes()))
}

/// `rel` (a `/`-separated relative path) under `base`.
pub(crate) fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Whether `rel` is a plain relative path into the game folder, as a journal must hold:
/// `minecraft/` then parts that are neither empty, `.` nor `..`, with no `\`, `:` or NUL.
pub(crate) fn valid_rel_path(rel: &str) -> bool {
    let mut parts = rel.split('/');
    parts.next() == Some(lodestone_image::layout::CLIENT_GAME_FOLDER)
        && rel.len() <= 8192
        && rel.split('/').skip(1).count() > 0
        && parts.all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains(['\\', ':', '\0'])
        })
}

/// Whether `rel` is `prefix` or inside it.
fn under(rel: &str, prefix: &str) -> bool {
    rel == prefix
        || rel
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir())
}

fn io_error(what: impl Into<String>) -> impl FnOnce(io::Error) -> ImageError {
    ImageError::io(what)
}

/// Every folder that holds one of `paths` (relative), all the way up.
fn folders_of<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let mut folders = BTreeSet::new();
    for path in paths {
        let mut rest = path;
        while let Some((parent, _)) = rest.rsplit_once('/') {
            if !folders.insert(parent.to_string()) {
                break;
            }
            rest = parent;
        }
    }
    folders
}

impl Staged {
    /// The swap of `owned` (relative paths) for what `staging` holds there. `replaced` are
    /// the files on disk the restore replaces or removes (everything else inside an owned
    /// folder is carried), `placed` the files it stages. Refuses an owned path that is a
    /// link, and a name inside an owned folder it can't handle.
    pub(crate) fn new(
        root: &Path,
        staging: String,
        owned: &[String],
        replaced: &HashSet<String>,
        placed: &[String],
    ) -> Result<Staged, ImageError> {
        let staging_dir = root.join(format!("{STAGING_PREFIX}{staging}"));
        let touched = folders_of(
            replaced
                .iter()
                .map(String::as_str)
                .chain(placed.iter().map(String::as_str)),
        );
        let mut entries = Vec::new();
        let mut carried = Vec::new();
        for path in owned {
            if !valid_rel_path(path) {
                return Err(ImageError::Invalid(format!(
                    "the restore can't handle the path {path:?}"
                )));
            }
            let on_disk = join_rel(root, path);
            let meta = fs::symlink_metadata(&on_disk).ok();
            if meta.as_ref().is_some_and(|m| m.file_type().is_symlink()) {
                return Err(ImageError::Invalid(format!(
                    "{path} is a link, which a restore can't replace; remove it first"
                )));
            }
            entries.push(JournalEntry {
                path: path.clone(),
                had_old: meta.is_some(),
                had_new: exists(&join_rel(&staging_dir, path)),
            });
            if meta.is_some_and(|m| m.is_dir()) {
                carried.extend(kept_inside(root, path, replaced, &touched)?);
            }
        }
        Ok(Staged {
            root: root.to_path_buf(),
            staging,
            old: random_suffix(),
            owned: entries,
            carried,
            #[cfg(test)]
            faults: Faults::default(),
        })
    }

    fn staging_dir(&self) -> PathBuf {
        self.root.join(format!("{STAGING_PREFIX}{}", self.staging))
    }

    fn old_dir(&self) -> PathBuf {
        self.root.join(format!("{OLD_PREFIX}{}", self.old))
    }

    /// Deletes the staged files without changing anything else.
    pub(crate) fn discard(self) {
        let _ = super::remove_tree(&self.staging_dir());
    }

    /// Swaps the staged files in, with the journal at `journal_path` (see the module).
    /// Not cancellable: it is short, and stopping halfway would leave a half-restored
    /// instance.
    pub(crate) fn swap(self, journal_path: &Path) -> Result<(), SwapError> {
        let staging_dir = self.staging_dir();
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
                    had_new: exists(&join_rel(&staging_dir, path)),
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
                let done = Journal {
                    phase: Phase::Done,
                    ..journal.clone()
                };
                match write_journal(journal_path, &done) {
                    Ok(()) => {
                        self.remove_work_dirs();
                        remove_journal(journal_path);
                        Ok(())
                    }
                    // The journal still says the swap is under way, and the next start
                    // would undo it after the game ran: undo it now instead.
                    Err(error) => self.undo(journal_path, &journal, error),
                }
            }
            #[cfg(test)]
            Err(MoveError::Crash) => Err(SwapError::Broken(ImageError::Invalid(
                "crashed on purpose".into(),
            ))),
            Err(MoveError::Failed(error)) => self.undo(journal_path, &journal, error),
        }
    }

    /// Rolls back a swap that failed after the journal was written.
    fn undo(&self, journal_path: &Path, journal: &Journal, error: ImageError) -> Result<(), SwapError> {
        #[cfg(test)]
        let undone = if self.faults.break_rollback {
            Err(vec!["broken on purpose".to_string()])
        } else {
            rollback(&self.root, journal)
        };
        #[cfg(not(test))]
        let undone = rollback(&self.root, journal);
        match undone {
            Ok(()) => {
                remove_journal(journal_path);
                self.remove_work_dirs();
                Err(SwapError::RolledBack(error))
            }
            Err(problems) => {
                super::warn(
                    "images.restore",
                    "",
                    &format!("the restore couldn't be undone: {}", problems.join("; ")),
                );
                Err(SwapError::Broken(ImageError::Invalid(
                    "The restore couldn't be undone; restart the Launcher to repair it".into(),
                )))
            }
        }
    }

    /// Flushes the folders the swap renamed entries into. A failure only leaves the swap
    /// as durable as the file system makes it.
    fn sync_moved(&self) {
        #[cfg(unix)]
        {
            let mut dirs = BTreeSet::new();
            for path in self.owned.iter().map(|e| &e.path).chain(&self.carried) {
                if let Some(parent) = join_rel(&self.root, path).parent() {
                    dirs.insert(parent.to_path_buf());
                }
            }
            for dir in dirs {
                if let Ok(file) = fs::File::open(&dir) {
                    let _ = file.sync_all();
                }
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
                    return Err(MoveError::Failed(ImageError::Invalid(
                        "failed on purpose".into(),
                    )));
                }
            }
            renames += 1;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| MoveError::Failed(io_error(format!("Failed to move {what}"))(e)))?;
            }
            crate::fsx::rename(from, to)
                .map_err(|e| MoveError::Failed(io_error(format!("Failed to move {what}"))(e)))
        };
        fs::create_dir(&old_dir).map_err(|e| {
            MoveError::Failed(io_error("Failed to create the restore's folder")(e))
        })?;
        for entry in self.owned.iter().filter(|e| e.had_old) {
            rename(
                &join_rel(&self.root, &entry.path),
                &join_rel(&old_dir, &entry.path),
                &entry.path,
            )?;
        }
        journal.phase = Phase::MovingNew;
        write_journal(journal_path, journal).map_err(MoveError::Failed)?;
        for entry in self.owned.iter().filter(|e| e.had_new) {
            rename(
                &join_rel(&staging_dir, &entry.path),
                &join_rel(&self.root, &entry.path),
                &entry.path,
            )?;
        }
        let replaced = staging_dir.join(REPLACED_DIR);
        for path in &self.carried {
            let destination = join_rel(&self.root, path);
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
            if let Err(e) = super::remove_tree(&dir) {
                super::warn(
                    "images.restore",
                    "",
                    &format!("Failed to delete {}: {e}", dir.display()),
                );
            }
        }
    }
}

/// The paths inside the owned folder `owned` that the restore keeps as they are: links,
/// files it doesn't replace, and folders holding nothing it replaces or places.
fn kept_inside(
    root: &Path,
    owned: &str,
    replaced: &HashSet<String>,
    touched: &BTreeSet<String>,
) -> Result<Vec<String>, ImageError> {
    let mut kept = Vec::new();
    let mut stack = vec![owned.to_string()];
    while let Some(folder) = stack.pop() {
        let dir = join_rel(root, &folder);
        let entries = fs::read_dir(&dir).map_err(io_error(format!("Failed to read {folder}")))?;
        for entry in entries {
            let entry = entry.map_err(io_error(format!("Failed to read {folder}")))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                return Err(ImageError::Invalid(format!(
                    "{folder} has a file whose name the Launcher can't handle; rename or remove it first"
                )));
            };
            let rel = format!("{folder}/{name}");
            if !valid_rel_path(&rel) {
                return Err(ImageError::Invalid(format!(
                    "{folder} has a file whose name the Launcher can't handle ({name}); rename or remove it first"
                )));
            }
            let kind = entry
                .file_type()
                .map_err(io_error(format!("Failed to read {rel}")))?;
            if kind.is_symlink() {
                kept.push(rel);
            } else if kind.is_dir() {
                if touched.contains(&rel) {
                    stack.push(rel);
                } else {
                    kept.push(rel);
                }
            } else if !replaced.contains(&rel) {
                kept.push(rel);
            }
        }
    }
    kept.sort();
    Ok(kept)
}

fn valid_suffix(suffix: &str) -> bool {
    suffix.len() == 16
        && suffix
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn write_journal(path: &Path, journal: &Journal) -> Result<(), ImageError> {
    let json = serde_json::to_vec_pretty(journal)
        .map_err(|e| ImageError::Invalid(format!("Failed to write the restore journal: {e}")))?;
    super::write_small_atomic(path, &json)
}

fn remove_journal(path: &Path) {
    if let Err(e) = fs::remove_file(path)
        && e.kind() != io::ErrorKind::NotFound
    {
        super::warn(
            "images.restore",
            "",
            &format!("Failed to delete the restore journal {}: {e}", path.display()),
        );
    }
}

/// Whether no folder on the way to `rel` under `base` is a link or a file.
fn no_links_above(base: &Path, rel: &str) -> bool {
    let mut current = base.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len().saturating_sub(1)] {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return false,
            _ => {}
        }
    }
    true
}

/// Whether `path` is there (a link counts, not followed). An error other than "not found"
/// is returned, never taken for a missing path.
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Undoes an interrupted or failed swap of the instance folder `root`. Never deletes
/// anything at a destination: what is in the way goes into the staging folder. Returns
/// what couldn't be done.
pub(crate) fn rollback(root: &Path, journal: &Journal) -> Result<(), Vec<String>> {
    if !valid_suffix(&journal.staging) || !valid_suffix(&journal.old) {
        return Err(vec!["the journal names invalid folders".to_string()]);
    }
    if let Some(bad) = journal.entries.iter().find(|e| !valid_rel_path(&e.path)) {
        return Err(vec![format!("the journal has an invalid path {:?}", bad.path)]);
    }
    let staging_dir = root.join(format!("{STAGING_PREFIX}{}", journal.staging));
    let old_dir = root.join(format!("{OLD_PREFIX}{}", journal.old));
    for dir in [&staging_dir, &old_dir] {
        if exists(dir) && !is_real_dir(dir) {
            return Err(vec![format!("{} is not a folder", dir.display())]);
        }
    }
    // The old folder is made before anything moves and deleted only once the swap is
    // finished or undone, so without it there is nothing to undo.
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
            .and_then(|_| crate::fsx::rename(from, to));
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
        // Carried paths already moved into the new entries go back into the old ones.
        for entry in kept.iter().rev() {
            let at_old = join_rel(&old_dir, &entry.path);
            let at_root = join_rel(root, &entry.path);
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
            if !no_links_above(root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
                problems.push(format!("{}: a folder on the way is a link", entry.path));
                continue;
            }
            move_one(&at_root, &at_old, &entry.path, &mut problems);
        }
    }
    for entry in owned.iter().rev() {
        let at_old = join_rel(&old_dir, &entry.path);
        let at_root = join_rel(root, &entry.path);
        if !no_links_above(root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
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

/// What [`recover`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Recovery {
    /// Instances whose interrupted restore was undone (or finished, when it was done).
    pub repaired: Vec<String>,
    /// Instances whose restore couldn't be undone: the journal stays for the next start.
    pub broken: Vec<String>,
}

/// Reads the journal at `path`: `None` when there is none; an error when it can't be read
/// as one (it stays, and the instance with it).
pub(crate) fn read_journal(path: &Path) -> Result<Option<Journal>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() || meta.len() > MAX_JOURNAL_BYTES {
        return Err("the journal isn't a file of a journal's size".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Finishes or undoes the swap `journal_path` describes in the instance folder `root`,
/// then deletes the journal and the work folders. Ok(None) when there was no journal.
pub(crate) fn recover_one(root: &Path, journal_path: &Path) -> Result<Option<Phase>, String> {
    let Some(journal) = read_journal(journal_path)? else {
        return Ok(None);
    };
    rollback(root, &journal).map_err(|problems| problems.join("; "))?;
    remove_journal(journal_path);
    for dir in [
        root.join(format!("{STAGING_PREFIX}{}", journal.staging)),
        root.join(format!("{OLD_PREFIX}{}", journal.old)),
    ] {
        if is_real_dir(&dir) {
            let _ = super::remove_tree(&dir);
        }
    }
    Ok(Some(journal.phase))
}

/// Deletes restore work folders in the instance folder `root` that no journal names (a
/// crash while staging, before the journal was written).
pub(crate) fn remove_leftovers(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let is_work = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(STAGING_PREFIX) || n.starts_with(OLD_PREFIX));
        if is_work && is_real_dir(&entry.path()) {
            let _ = super::remove_tree(&entry.path());
        }
    }
}
