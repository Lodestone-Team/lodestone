//! The file system steps the store's writes rely on: unique temporary names, flushing to
//! disk, read-only files, renames Windows can briefly refuse, and copies that clone on APFS.
//!
//! Every write in the store goes the same way: a temporary file next to its final place,
//! written out ([`write_out`]), made read-only (objects), then renamed over the final name,
//! so a crash or a power cut leaves either nothing or the whole file, never part of one;
//! the store makes a batch of such files durable before a manifest names them. Temporary files a
//! crash leaves behind end in [`TEMP_SUFFIX`] and are swept once they are an hour old.

use std::collections::hash_map::RandomState;
use std::fs::{self, File};
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Temporary files start with this...
pub(crate) const TEMP_PREFIX: &str = ".incoming-";
/// ...and end with this, so a sweep can tell them from objects and manifests.
pub(crate) const TEMP_SUFFIX: &str = ".tmp";

/// How long a rename into the store may be retried while Windows reports the file in use
/// (an antivirus scanning the new file, say). Elsewhere a failed rename fails at once.
#[cfg(windows)]
const RENAME_BUDGET: Duration = Duration::from_secs(2);
#[cfg(not(windows))]
const RENAME_BUDGET: Duration = Duration::ZERO;

/// Sixteen hex characters that differ between calls, processes and runs. No random number
/// crate is needed: std's hasher keys are random per process, and a counter and the time
/// make each name differ from the last.
pub(crate) fn random_suffix() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
        hasher.write_u128(now.as_nanos());
    }
    format!("{:016x}", hasher.finish())
}

/// A temporary file's name in `dir` (not created).
pub(crate) fn temp_path(dir: &Path) -> PathBuf {
    dir.join(format!("{TEMP_PREFIX}{}{TEMP_SUFFIX}", random_suffix()))
}

/// Creates a new temporary file in `dir`, which must exist. Never opens an existing file.
pub(crate) fn create_temp(dir: &Path) -> io::Result<(File, PathBuf)> {
    let mut last = None;
    for _ in 0..8 {
        let path = temp_path(dir);
        match File::options().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("no free temporary name")))
}

/// Whether `name` is one of the store's temporary files.
pub(crate) fn is_temp_name(name: &str) -> bool {
    name.ends_with(TEMP_SUFFIX)
}

/// Deletes a temporary file when dropped, unless disarmed: an error or a panic part way
/// through a write leaves nothing behind.
pub(crate) struct TempGuard(Option<PathBuf>);

impl TempGuard {
    pub(crate) fn new(path: PathBuf) -> TempGuard {
        TempGuard(Some(path))
    }

    /// The file it will delete, unless disarmed.
    pub(crate) fn path(&self) -> Option<&Path> {
        self.0.as_deref()
    }

    pub(crate) fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = remove_file_forced(&path);
        }
    }
}

/// Makes `path` read-only, so nothing edits an object in place by accident (a stored file
/// opened from the store by hand, a clone written through).
pub(crate) fn set_readonly(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if !permissions.readonly() {
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

/// Clears a file's read-only flag on Windows, which refuses to delete or rename over a
/// read-only file. Elsewhere the flag doesn't matter for either, and this does nothing.
pub(crate) fn clear_readonly(path: &Path) {
    #[cfg(windows)]
    if let Ok(meta) = fs::symlink_metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// Deletes a file even when it is read-only.
pub(crate) fn remove_file_forced(path: &Path) -> io::Result<()> {
    clear_readonly(path);
    fs::remove_file(path)
}

/// Renames, retrying for a moment while Windows reports the file in use. Elsewhere, and
/// for other errors, fails at once.
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let started = Instant::now();
    let mut wait = Duration::from_millis(20);
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if is_in_use(&e) && started.elapsed() + wait <= RENAME_BUDGET => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(500));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Access denied, sharing violation or lock violation: on Windows, something has the file
/// (or a file in the folder) open.
fn is_in_use(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// Flushes a folder's entries (new and renamed files) to disk. Only unix needs it: there a
/// renamed file can be lost in a power cut until its folder is flushed.
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Hands what was written to a file to the drive as part of a batch that [`full_flush`]
/// ends, before the file is renamed into place: what the store does per object and chunk,
/// and materializing per placed file.
///
/// Flushing every file to the drive's own storage is a device cache flush per file (std's
/// `sync_all`), and chunking makes one file per 64 KiB of a large file: a world's first
/// snapshot takes minutes that way. So, as git's `core.fsyncMethod=batch` does, each file
/// is only handed to the drive here, and one flush per batch makes them all durable:
///
/// - Apple platforms: plain `fsync(2)` per file (std's `sync_all` is `F_FULLFSYNC`, about
///   6 ms each and not overlapping between threads), and one `F_FULLFSYNC` at the end.
/// - Linux: nothing per file; at the end one `syncfs(2)` of the file system, which writes
///   out every dirty file and folder on it and flushes the drive's cache.
/// - Windows: `NtFlushBuffersFileEx` with `FLUSH_FLAGS_NO_SYNC` per file (its data to the
///   drive without a cache flush), and one `FlushFileBuffers` at the end, which flushes the
///   volume's cache. Where that call isn't supported, `FlushFileBuffers` per file.
/// - Elsewhere: `fsync` per file, and nothing at the end.
pub(crate) fn write_out(file: &File) -> io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        apple::fsync(file)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = file;
        Ok(())
    }
    #[cfg(windows)]
    {
        windows::flush_no_sync(file)
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux", windows)))]
    {
        file.sync_all()
    }
}

/// Flushes a folder's entries as part of a batch that [`full_flush`] ends: on Apple
/// platforms a plain `fsync(2)`; on Linux nothing (the batch's `syncfs` covers folders);
/// elsewhere [`sync_dir`].
pub(crate) fn write_out_dir(dir: &Path) -> io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        apple::fsync(&File::open(dir)?)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = dir;
        Ok(())
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
    {
        sync_dir(dir)
    }
}

/// Ends a batch of [`write_out`]s on the file system of the folder `dir`, making every file
/// and folder of the batch durable (see [`write_out`]).
pub(crate) fn full_flush(dir: &Path) -> io::Result<()> {
    #[cfg(target_vendor = "apple")]
    {
        // F_FULLFSYNC through any file asks the drive to write out its whole cache.
        File::open(dir)?.sync_all()
    }
    #[cfg(target_os = "linux")]
    {
        linux::syncfs(&File::open(dir)?)
    }
    #[cfg(windows)]
    {
        // FlushFileBuffers on a file of the volume flushes the drive's cache too (git's
        // batch mode on Windows does the same with a file of its own).
        let (file, path) = create_temp(dir)?;
        let _guard = TempGuard::new(path);
        file.sync_all()
    }
    #[cfg(not(any(target_vendor = "apple", target_os = "linux", windows)))]
    {
        let _ = dir;
        Ok(())
    }
}

#[cfg(target_vendor = "apple")]
mod apple {
    use std::ffi::c_int;
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;

    // libSystem's fsync(2), which std links anyway; std only exposes F_FULLFSYNC.
    unsafe extern "C" {
        #[link_name = "fsync"]
        fn c_fsync(fd: c_int) -> c_int;
    }

    /// Plain `fsync(2)`: the file's data and attributes to the drive, without flushing the
    /// drive's cache.
    pub(super) fn fsync(file: &File) -> io::Result<()> {
        loop {
            // SAFETY: the descriptor belongs to `file`, which stays open for the call;
            // fsync reads nothing from memory.
            if unsafe { c_fsync(file.as_raw_fd()) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::c_int;
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;

    // The C library's syncfs(2) (glibc 2.14 on, musl), which std links anyway.
    unsafe extern "C" {
        #[link_name = "syncfs"]
        fn c_syncfs(fd: c_int) -> c_int;
    }

    /// `syncfs(2)`: writes out every dirty file and folder of the file system `file` is
    /// on and flushes the drive's cache, reporting write-back errors (Linux 5.8 on).
    pub(super) fn syncfs(file: &File) -> io::Result<()> {
        loop {
            // SAFETY: the descriptor belongs to `file`, which stays open for the call;
            // syncfs reads nothing from memory.
            if unsafe { c_syncfs(file.as_raw_fd()) } == 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;

    /// `IO_STATUS_BLOCK`: a status (or pointer) and a count.
    #[repr(C)]
    struct IoStatusBlock {
        status: usize,
        information: usize,
    }

    /// Write the data to the drive without asking it to flush its cache.
    const FLUSH_FLAGS_NO_SYNC: u32 = 0x0000_0002;

    // ntdll's NtFlushBuffersFileEx (Windows 8 on), linked without an import library.
    #[link(name = "ntdll", kind = "raw-dylib")]
    unsafe extern "system" {
        fn NtFlushBuffersFileEx(
            file: *mut c_void,
            flags: u32,
            parameters: *mut c_void,
            parameters_size: u32,
            io_status: *mut IoStatusBlock,
        ) -> i32;
    }

    /// Hands the file's data to the drive without a cache flush; where the file system
    /// doesn't support that, flushes it fully (`FlushFileBuffers`).
    pub(super) fn flush_no_sync(file: &File) -> io::Result<()> {
        let mut status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        // SAFETY: the handle belongs to `file`, which stays open for the call; no
        // parameters are passed, and the status block lives on this stack for the call.
        let result = unsafe {
            NtFlushBuffersFileEx(
                file.as_raw_handle(),
                FLUSH_FLAGS_NO_SYNC,
                std::ptr::null_mut(),
                0,
                &mut status,
            )
        };
        if result >= 0 {
            Ok(())
        } else {
            file.sync_all()
        }
    }
}

/// Sets `path`'s modification time to now. The store does this to an object it reuses, so
/// a garbage collection running at the same time sees it as new and keeps it until the
/// manifest that uses it is written (see `gc`). Read-only files can have their times set
/// by their owner; a file another user owns can't, and that is an error the caller must
/// not take for a touch.
pub(crate) fn touch(path: &Path) -> io::Result<()> {
    let now = SystemTime::now();
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_WRITE_ATTRIBUTES: allowed on read-only files.
        File::options().access_mode(0x100).open(path)?
    };
    #[cfg(not(windows))]
    let file = File::open(path)?;
    file.set_modified(now)
}

/// How long ago `path` was last modified; `None` when that can't be told (then callers
/// treat the file as new, the safe side for deleting).
pub(crate) fn age(meta: &fs::Metadata, now: SystemTime) -> Option<Duration> {
    let modified = meta.modified().ok()?;
    now.duration_since(modified).ok()
}

/// Copies `from` to the new file `to` (which must not exist). On macOS std clones the file
/// on APFS (`fclonefileat`), sharing its blocks until either copy changes, and falls back
/// to a plain copy elsewhere (`fcopyfile`); see `std::fs::copy`.
pub(crate) fn clone_or_copy(from: &Path, to: &Path) -> io::Result<u64> {
    fs::copy(from, to)
}

/// Unix nanoseconds of a time, saturating outside `i64`'s range (years 1677 to 2262).
pub(crate) fn unix_ns(time: SystemTime) -> i64 {
    let nanos: i128 = match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
        Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
    };
    i64::try_from(nanos).unwrap_or(if nanos < 0 { i64::MIN } else { i64::MAX })
}

/// A file's modification time in unix nanoseconds, 0 when the file system has none.
pub(crate) fn modified_ns(meta: &fs::Metadata) -> i64 {
    meta.modified().map(unix_ns).unwrap_or(0)
}

/// The time `ns` unix nanoseconds stand for, if it can be represented.
pub(crate) fn from_unix_ns(ns: i64) -> Option<SystemTime> {
    if ns >= 0 {
        UNIX_EPOCH.checked_add(Duration::from_nanos(ns.unsigned_abs()))
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_nanos(ns.unsigned_abs()))
    }
}

/// A file's permission bits the way images record them: the unix bits without setuid,
/// setgid and sticky (an image never carries those), or on Windows, which only knows
/// read-only, 0o444 or 0o644.
pub(crate) fn mode_of(meta: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        if meta.permissions().readonly() {
            0o444
        } else {
            0o644
        }
    }
}

/// Gives a placed file `mode` (unix bits; on Windows only whether it is writable).
pub(crate) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        let readonly = mode & 0o222 == 0;
        if permissions.readonly() != readonly {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(readonly);
            fs::set_permissions(path, permissions)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_differ() {
        let names: std::collections::HashSet<String> = (0..1000).map(|_| random_suffix()).collect();
        assert_eq!(names.len(), 1000);
        assert!(names.iter().all(|n| n.len() == 16));
    }

    #[test]
    fn temp_files_never_reuse_a_name_and_guards_delete_them() {
        let dir = tempfile::tempdir().unwrap();
        let (_, path) = create_temp(dir.path()).unwrap();
        assert!(is_temp_name(path.file_name().unwrap().to_str().unwrap()));
        {
            let _guard = TempGuard::new(path.clone());
        }
        assert!(!path.exists());
        let (_, kept) = create_temp(dir.path()).unwrap();
        let mut guard = TempGuard::new(kept.clone());
        guard.disarm();
        drop(guard);
        assert!(kept.exists());
    }

    #[test]
    fn read_only_files_can_be_touched_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        fs::write(&path, b"x").unwrap();
        let old = SystemTime::now() - Duration::from_secs(7200);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(old)
            .unwrap();
        set_readonly(&path).unwrap();
        touch(&path).unwrap();
        let meta = fs::metadata(&path).unwrap();
        assert!(age(&meta, SystemTime::now()).unwrap() < Duration::from_secs(600));
        remove_file_forced(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn nanosecond_times_round_trip() {
        for ns in [0i64, 1, -1, 1_794_645_624_000_000_007, -86_400_000_000_000] {
            assert_eq!(unix_ns(from_unix_ns(ns).unwrap()), ns);
        }
    }

    #[cfg(unix)]
    #[test]
    fn modes_drop_special_bits() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        fs::write(&path, b"x").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o4755)).unwrap();
        assert_eq!(mode_of(&fs::metadata(&path).unwrap()), 0o755);
        set_mode(&path, 0o7640).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
            0o640
        );
    }
}
