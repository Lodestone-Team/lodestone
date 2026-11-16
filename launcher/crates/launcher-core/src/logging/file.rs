//! The log file, rotated by size: past [`MAX_BYTES`] it becomes `.1`, `.1` becomes
//! `.2`, and so on to `.KEEP`, and the oldest is dropped.
//!
//! The file is opened for appending, so two processes writing it (two dev builds side
//! by side) interleave whole lines; each counts only its own bytes, so their files
//! grow past the limit. Writes are buffered ([`BUFFER`] bytes at most); the logger
//! flushes warnings at once and the rest within a second.
//!
//! The logs hold account and server names, addresses and paths, so on Unix the
//! folder is the user's alone (0700) and the files too (0600), wherever it is, as far
//! as the drive allows (a network drive may refuse to change permissions; logging
//! carries on).
//!
//! The file someone moved away (to the Trash, to start afresh) or deleted is noticed
//! within a few seconds, and a new one is opened where it was ([`RotatingFile::reattach`]):
//! otherwise the rest of the run would be written where nobody looks.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// The size a file is rotated at.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Rotated files kept: `.1` (the newest) to `.5`.
pub const KEEP: usize = launcher_control::logs::KEEP;
/// The write buffer.
const BUFFER: usize = 32 * 1024;
/// After a failed rotation (Windows: another program holds the file without sharing
/// deletes), how much more is written before trying again.
const RETRY_AFTER: u64 = 1024 * 1024;
/// After the file couldn't be opened, how long before trying again.
const REOPEN_AFTER: Duration = Duration::from_secs(10);
/// How often a write looks whether the open file is still at its path.
const CHECK_EVERY: Duration = Duration::from_secs(5);

pub struct RotatingFile {
    dir: PathBuf,
    name: String,
    max: u64,
    keep: usize,
    out: Option<BufWriter<File>>,
    /// The file's size, as far as this process knows.
    written: u64,
    /// Rotation is next tried past this size.
    rotate_at: u64,
    /// When the file last couldn't be opened.
    failed: Option<Instant>,
    /// When a write last looked whether the file is still at its path.
    checked: Instant,
    /// The file is closed on purpose ([`Self::release`]): lines wait in `pending`.
    released: bool,
    /// Lines written while released, at most [`BUFFER`] bytes.
    pending: String,
    /// Renames a file; a seam for tests, where the rename has to fail.
    rename: fn(&Path, &Path) -> io::Result<()>,
}

impl RotatingFile {
    /// `dir/name`, created if needed, rotated at [`MAX_BYTES`] keeping [`KEEP`].
    pub fn open(dir: &Path, name: &str) -> Self {
        Self::with_limits(dir, name, MAX_BYTES, KEEP)
    }

    pub fn with_limits(dir: &Path, name: &str, max: u64, keep: usize) -> Self {
        let mut file = Self {
            dir: dir.to_path_buf(),
            name: name.to_string(),
            max,
            keep,
            out: None,
            written: 0,
            rotate_at: max,
            failed: None,
            checked: Instant::now(),
            released: false,
            pending: String::new(),
            rename: |from, to| fs::rename(from, to),
        };
        // A rotation that stopped halfway (the app quit in it) is finished first.
        if file.staging().exists() {
            file.shift_in_staging();
        }
        file.reopen();
        file
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.name)
    }

    /// `name.N`.
    pub fn rotated(&self, n: usize) -> PathBuf {
        self.dir.join(format!("{}.{n}", self.name))
    }

    /// Where the file waits while the older ones move up: `name.rotating`.
    fn staging(&self) -> PathBuf {
        self.dir.join(format!("{}.rotating", self.name))
    }

    fn reopen(&mut self) {
        self.checked = Instant::now();
        let opened = private_dir(&self.dir).and_then(|()| open_private(&self.path()));
        match opened {
            Ok(file) => {
                self.written = file.metadata().map(|m| m.len()).unwrap_or(0);
                self.rotate_at = self.max;
                self.out = Some(BufWriter::with_capacity(BUFFER, file));
                self.failed = None;
            }
            Err(_) => {
                self.out = None;
                self.failed = Some(Instant::now());
            }
        }
    }

    /// Appends `text` (whole lines), rotating first when it would pass the limit.
    pub fn write(&mut self, text: &str) {
        if self.released {
            if self.pending.len() + text.len() <= BUFFER {
                self.pending.push_str(text);
            }
            return;
        }
        if self.checked.elapsed() >= CHECK_EVERY {
            self.reattach();
        }
        if self.out.is_none() {
            if self.failed.is_some_and(|at| at.elapsed() < REOPEN_AFTER) {
                return;
            }
            self.reopen();
        }
        if self.written > 0 && self.written + text.len() as u64 > self.rotate_at {
            self.rotate();
        }
        let Some(out) = &mut self.out else {
            return;
        };
        if out.write_all(text.as_bytes()).is_ok() {
            self.written += text.len() as u64;
        }
    }

    pub fn flush(&mut self) {
        if let Some(out) = &mut self.out {
            let _ = out.flush();
        }
    }

    /// Closes the file until [`Self::resume`], so its folder can be renamed (Windows
    /// refuses while a file in it is open). Lines meanwhile wait in memory.
    pub fn release(&mut self) {
        self.flush();
        self.out = None;
        self.released = true;
    }

    /// Opens the file again after [`Self::release`] and writes what waited.
    pub fn resume(&mut self) {
        if !self.released {
            return;
        }
        self.released = false;
        self.reopen();
        let pending = std::mem::take(&mut self.pending);
        if !pending.is_empty() {
            self.write(&pending);
        }
    }

    /// When the open file is no longer at its path (moved to the Trash, deleted, its
    /// folder renamed), opens a new one there; what was still buffered goes to the
    /// new file.
    pub fn reattach(&mut self) {
        self.checked = Instant::now();
        let Some(out) = &self.out else {
            return;
        };
        if same_file(out.get_ref(), &self.path()) {
            return;
        }
        let Some(out) = self.out.take() else {
            return;
        };
        let buffered = match out.into_parts() {
            (_, Ok(buffered)) => buffered,
            (_, Err(_)) => Vec::new(),
        };
        self.reopen();
        if let Some(out) = &mut self.out
            && out.write_all(&buffered).is_ok()
        {
            self.written += buffered.len() as u64;
        }
    }

    /// Moves `name` to `name.1` and the older files up one, dropping the oldest. The
    /// file is closed first: Windows can't rename an open file. It is moved aside
    /// before anything older is touched, so when that fails (Windows: another program
    /// holds it without sharing deletes) the history is kept: writing carries on in
    /// the same file and rotation is tried again later.
    fn rotate(&mut self) {
        self.flush();
        self.out = None;
        // A rotation that couldn't finish left its file staged: finish it first, and
        // never write over it.
        if self.staging().exists() {
            self.shift_in_staging();
        }
        if !self.staging().exists() && (self.rename)(&self.path(), &self.staging()).is_ok() {
            self.shift_in_staging();
            self.reopen();
        } else {
            self.reopen();
            self.rotate_at = self.written + RETRY_AFTER;
        }
    }

    /// Moves the older files up one (dropping the oldest) and `name.rotating` to
    /// `name.1`.
    fn shift_in_staging(&mut self) {
        let _ = fs::remove_file(self.rotated(self.keep));
        for n in (1..self.keep).rev() {
            let from = self.rotated(n);
            if from.exists() {
                let _ = (self.rename)(&from, &self.rotated(n + 1));
            }
        }
        let _ = (self.rename)(&self.staging(), &self.rotated(1));
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Whether `file` is the file at `path`: on Unix the same device and inode, elsewhere
/// only that something is there (Windows can't move an open file without sharing
/// deletes, which the log allows, so a deleted or moved one shows as missing).
fn same_file(file: &File, path: &Path) -> bool {
    let Ok(at_path) = fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        file.metadata()
            .is_ok_and(|open| open.dev() == at_path.dev() && open.ino() == at_path.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = (file, at_path);
        true
    }
}

/// Creates `dir` if needed; on Unix it is made, or made again, the user's alone when
/// the drive allows it.
pub fn private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
        if dir.is_dir() {
            if fs::metadata(dir)?.permissions().mode() & 0o777 != 0o700 {
                // a network or shared drive may refuse; writing still works there
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
            return Ok(());
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    fs::create_dir_all(dir)
}

/// Opens `path` for appending, created if needed; on Unix it is the user's alone.
fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        options.mode(0o600);
        let file = options.open(path)?;
        // a file from before (or made by another program) is made private too, when
        // the drive allows it
        if file
            .metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o077 != 0)
        {
            let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_the_newest_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = RotatingFile::with_limits(dir.path(), "app.log", 100, 3);
        // 10 lines of 40 bytes: two to a file
        for n in 0..10 {
            file.write(&format!("{n:<39}\n"));
        }
        file.flush();
        let names: Vec<String> = launcher_control::logs::files(dir.path(), "app.log")
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["app.log", "app.log.1", "app.log.2", "app.log.3"]);
        let read = |name: &str| fs::read_to_string(dir.path().join(name)).unwrap();
        assert!(read("app.log").starts_with('8'));
        assert!(read("app.log.1").starts_with('6'));
        assert!(
            read("app.log.3").starts_with('2'),
            "the oldest were dropped"
        );
        assert!(!dir.path().join("app.log.4").exists());
        for name in ["app.log", "app.log.1", "app.log.2", "app.log.3"] {
            assert!(read(name).len() as u64 <= 100, "{name}");
        }
    }

    #[test]
    fn a_reopened_file_carries_on_counting() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut file = RotatingFile::with_limits(dir.path(), "app.log", 100, 2);
            file.write(&format!("{:<79}\n", "first"));
        }
        let mut file = RotatingFile::with_limits(dir.path(), "app.log", 100, 2);
        file.write(&format!("{:<79}\n", "second"));
        file.flush();
        assert!(
            fs::read_to_string(dir.path().join("app.log.1"))
                .unwrap()
                .starts_with("first")
        );
        assert!(
            fs::read_to_string(dir.path().join("app.log"))
                .unwrap()
                .starts_with("second")
        );
    }

    #[test]
    fn a_failed_rename_keeps_the_history() {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=3 {
            fs::write(
                dir.path().join(format!("app.log.{n}")),
                format!("history {n}\n"),
            )
            .unwrap();
        }
        let mut file = RotatingFile::with_limits(dir.path(), "app.log", 100, 3);
        // the current file can't be moved; the others could
        file.rename = |from, to| {
            if from.extension().is_some_and(|e| e == "log") {
                Err(io::Error::other("held by another program"))
            } else {
                fs::rename(from, to)
            }
        };
        for n in 0..40 {
            file.write(&format!("{n:<39}\n"));
        }
        file.flush();
        for n in 1..=3 {
            assert_eq!(
                fs::read_to_string(dir.path().join(format!("app.log.{n}"))).unwrap(),
                format!("history {n}\n"),
                "app.log.{n} survived"
            );
        }
        assert_eq!(
            fs::read_to_string(dir.path().join("app.log"))
                .unwrap()
                .lines()
                .count(),
            40,
            "writing carried on in the same file"
        );
    }

    #[test]
    fn a_rotation_cut_short_is_finished_at_open() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.log.1"), "older\n").unwrap();
        fs::write(dir.path().join("app.log.rotating"), "newest rotated\n").unwrap();
        let _file = RotatingFile::with_limits(dir.path(), "app.log", 100, 3);
        let read = |name: &str| fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(read("app.log.1"), "newest rotated\n");
        assert_eq!(read("app.log.2"), "older\n");
        assert!(!dir.path().join("app.log.rotating").exists());
    }

    #[test]
    fn a_staged_file_is_never_written_over() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = RotatingFile::with_limits(dir.path(), "app.log", 100, 3);
        // `.1` can't be moved (Windows: a viewer holds it), so the staged file stays
        file.rename = |from, to| {
            if from
                .extension()
                .is_some_and(|e| e == "rotating" || e == "1")
            {
                Err(io::Error::other("held by another program"))
            } else {
                fs::rename(from, to)
            }
        };
        fs::write(dir.path().join("app.log.1"), "held\n").unwrap();
        fs::write(dir.path().join("app.log.rotating"), "staged\n").unwrap();
        for n in 0..10 {
            file.write(&format!("{n:<39}\n"));
        }
        file.flush();
        let read = |name: &str| fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(read("app.log.rotating"), "staged\n");
        assert_eq!(read("app.log.1"), "held\n");
        assert_eq!(read("app.log").lines().count(), 10, "writing carried on");
    }

    #[test]
    fn a_file_moved_away_is_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut file = RotatingFile::with_limits(dir.path(), "app.log", 1000, 3);
        file.write("before\n");
        file.flush();
        // moved to the Trash
        let trash = dir.path().join("trashed.log");
        fs::rename(dir.path().join("app.log"), &trash).unwrap();
        file.write("buffered\n");
        file.reattach();
        file.write("after\n");
        file.flush();
        assert_eq!(fs::read_to_string(&trash).unwrap(), "before\n");
        assert_eq!(
            fs::read_to_string(dir.path().join("app.log")).unwrap(),
            "buffered\nafter\n"
        );
        // the whole folder deleted
        fs::remove_dir_all(dir.path()).unwrap();
        file.reattach();
        file.write("in a new folder\n");
        file.flush();
        assert_eq!(
            fs::read_to_string(dir.path().join("app.log")).unwrap(),
            "in a new folder\n"
        );
    }

    #[test]
    fn a_released_file_keeps_its_lines_for_later() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        let mut file = RotatingFile::with_limits(&logs, "app.log", 1000, 3);
        file.write("one\n");
        file.release();
        // the folder can be renamed now (Windows refuses while the file is open)
        let moved = dir.path().join("moved");
        fs::rename(&logs, &moved).unwrap();
        file.write("two\n");
        fs::rename(&moved, &logs).unwrap();
        file.resume();
        file.write("three\n");
        file.flush();
        assert_eq!(
            fs::read_to_string(logs.join("app.log")).unwrap(),
            "one\ntwo\nthree\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn logs_are_the_users_alone() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        fs::create_dir(&logs).unwrap();
        fs::set_permissions(&logs, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(logs.join("app.log"), "from before\n").unwrap();
        fs::set_permissions(logs.join("app.log"), fs::Permissions::from_mode(0o644)).unwrap();
        let mut file = RotatingFile::with_limits(&logs, "app.log", 100, 3);
        for n in 0..6 {
            file.write(&format!("{n:<39}\n"));
        }
        file.flush();
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&logs), 0o700);
        for path in launcher_control::logs::files(&logs, "app.log") {
            assert_eq!(mode(&path), 0o600, "{}", path.display());
        }
        assert!(logs.join("app.log.1").exists(), "it rotated");
        // a new folder is made private
        let fresh = dir.path().join("fresh/logs");
        let _file = RotatingFile::with_limits(&fresh, "app.log", 100, 3);
        assert_eq!(mode(&fresh), 0o700);
    }
}
