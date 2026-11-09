//! File system calls that ride out Windows' brief file locks.
//!
//! On Windows a rename or delete fails while any other process has the file open
//! without `FILE_SHARE_DELETE`. Defender scans every new file (above all a new
//! `.exe` or `.jar`) the moment it is closed, and the search indexer, backup and
//! sync tools (OneDrive, Dropbox) open files too; each holds it for a few to a few
//! hundred milliseconds. Unix has no such locks, so a rename right after a write
//! works there and fails here now and then. These wrappers retry those errors for a
//! couple of seconds, the way rustup, Go's `robustio` and npm's `graceful-fs` do.
//! Elsewhere they are the plain `std::fs` calls.

use std::{io, path::Path};

/// [`std::fs::rename`], retried while another process briefly holds either file.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let (from, to) = (from.as_ref(), to.as_ref());
    retry(|| std::fs::rename(from, to))
}

/// [`std::fs::remove_file`], retried like [`rename`].
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    retry(|| std::fs::remove_file(path))
}

/// Runs `op`, and on Windows runs it again while it fails because another process
/// has the file open: about 2 seconds in all, then the last error.
pub fn retry<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    #[cfg(windows)]
    {
        let mut delay = std::time::Duration::from_millis(5);
        let mut waited = std::time::Duration::ZERO;
        loop {
            match op() {
                Err(err) if is_transient(&err) && waited < MAX_WAIT => {
                    std::thread::sleep(delay);
                    waited += delay;
                    delay = (delay * 2).min(std::time::Duration::from_millis(250));
                }
                result => return result,
            }
        }
    }
    #[cfg(not(windows))]
    op()
}

#[cfg(windows)]
const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Access denied, sharing violation and lock violation: what Windows reports while
/// another process has the file (or a file in the folder) open.
#[cfg(windows)]
fn is_transient(err: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    matches!(
        err.raw_os_error(),
        Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn renames_over_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"new").unwrap();
        fs::write(&b, b"old").unwrap();
        rename(&a, &b).unwrap();
        assert_eq!(fs::read(&b).unwrap(), b"new");
        assert!(!a.exists());
    }

    #[test]
    fn a_missing_file_fails_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let err = rename(dir.path().join("missing"), dir.path().join("b")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
    }

    /// A file another handle has open without delete sharing can't be renamed until
    /// that handle closes; the retry waits for it.
    #[cfg(windows)]
    #[test]
    fn waits_for_another_handle_to_close() {
        use std::os::windows::fs::OpenOptionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"x").unwrap();
        const FILE_SHARE_READ: u32 = 1;
        let held = fs::File::options()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&a)
            .unwrap();
        assert!(fs::rename(&a, &b).is_err());
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            drop(held);
        });
        rename(&a, &b).unwrap();
        release.join().unwrap();
        assert!(b.exists());
    }
}
