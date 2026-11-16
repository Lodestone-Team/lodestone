//! The log file, rotated by size: past [`MAX_BYTES`] it becomes `.1`, `.1` becomes
//! `.2`, and so on to `.KEEP`, and the oldest is dropped.
//!
//! The file is opened for appending, so two processes writing it (two dev builds side
//! by side) interleave whole lines. Writes are buffered ([`BUFFER`] bytes at most);
//! the logger flushes warnings at once and the rest within a second.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// The size a file is rotated at.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Rotated files kept: `.1` (the newest) to `.5`.
pub const KEEP: usize = 5;
/// The write buffer.
const BUFFER: usize = 32 * 1024;
/// After a failed rotation (Windows: another program holds the file without sharing
/// deletes), how much more is written before trying again.
const RETRY_AFTER: u64 = 1024 * 1024;
/// After the file couldn't be opened, how long before trying again.
const REOPEN_AFTER: Duration = Duration::from_secs(10);

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
        };
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

    fn reopen(&mut self) {
        let opened = fs::create_dir_all(&self.dir).and_then(|()| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.path())
        });
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

    /// Moves `name` to `name.1` and the older files up one, dropping the oldest. The
    /// file is closed first: Windows can't rename an open file. If the rename fails,
    /// writing carries on in the same file and rotation is tried again later.
    fn rotate(&mut self) {
        self.flush();
        self.out = None;
        let _ = fs::remove_file(self.rotated(self.keep));
        for n in (1..self.keep).rev() {
            let from = self.rotated(n);
            if from.exists() {
                let _ = fs::rename(&from, self.rotated(n + 1));
            }
        }
        let moved = fs::rename(self.path(), self.rotated(1));
        self.reopen();
        if moved.is_err() {
            self.rotate_at = self.written + RETRY_AFTER;
        }
    }

    /// The files, newest first: `name`, `name.1`, … that exist.
    pub fn files(dir: &Path, name: &str) -> Vec<PathBuf> {
        std::iter::once(dir.join(name))
            .chain((1..=KEEP).map(|n| dir.join(format!("{name}.{n}"))))
            .filter(|path| path.is_file())
            .collect()
    }
}

impl Drop for RotatingFile {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Reads the end of a file: at most `max` bytes, from the start of a line.
pub fn read_tail(path: &Path, max: u64) -> io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0
        && let Some(newline) = text.find('\n')
    {
        text.drain(..=newline);
    }
    Ok(text)
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
        let names: Vec<String> = RotatingFile::files(dir.path(), "app.log")
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
    fn a_tail_starts_at_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.log");
        fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(read_tail(&path, 9).unwrap(), "three\n");
        assert_eq!(read_tail(&path, 100).unwrap(), "one\ntwo\nthree\n");
    }
}
