//! Disks: free space, and the room Lodestone always leaves on one.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Error, ErrorKind};

/// Free space always left on a disk after a backup, restore, download or recorded image.
pub const FREE_SPACE_MARGIN: u64 = 1 << 30;

/// Sizes for messages: "1.2 GB".
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

// ---------------------------------------------------------------------------------------
// Large writes

/// The large writes in progress (uploads arriving, servers made from files, folders and
/// backups zipped or exported for download), each with what it is still to write, so two
/// can't both pass their free-space checks counting on the same space. One per core
/// (`ImageService::large_writes`), shared by every kind: a check counts all the others.
#[derive(Default)]
pub struct LargeWrites {
    next: std::sync::atomic::AtomicU64,
    writes: std::sync::Mutex<std::collections::HashMap<u64, Arc<AtomicU64>>>,
}

impl LargeWrites {
    /// Reserves `bytes` for a write until the reservation is dropped.
    pub fn reserve(self: &Arc<Self>, bytes: u64) -> Reserved {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let remaining = Arc::new(AtomicU64::new(bytes));
        self.lock().insert(id, remaining.clone());
        Reserved {
            writes: self.clone(),
            id,
            remaining,
        }
    }

    /// Reserves `bytes` and checks the disk holding `path` has them free, beside what the
    /// other large writes still need and [`FREE_SPACE_MARGIN`]. `what` completes "Not
    /// enough free disk space …".
    pub fn require(
        self: &Arc<Self>,
        path: &Path,
        bytes: u64,
        what: &str,
    ) -> Result<Reserved, Error> {
        let reserved = self.reserve(bytes);
        reserved.check(path, bytes, what)?;
        Ok(reserved)
    }

    /// What all the large writes in progress are still to write.
    pub fn total(&self) -> u64 {
        self.lock()
            .values()
            .map(|r| r.load(Ordering::Relaxed))
            .fold(0, u64::saturating_add)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<u64, Arc<AtomicU64>>> {
        self.writes.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A large write's share of [`LargeWrites`], given back when dropped.
pub struct Reserved {
    writes: Arc<LargeWrites>,
    id: u64,
    remaining: Arc<AtomicU64>,
}

impl Reserved {
    /// Says the write is still to write `bytes` (an upload as it arrives, an import once
    /// it has unpacked).
    pub fn set_remaining(&self, bytes: u64) {
        self.remaining.store(bytes, Ordering::Relaxed);
    }

    /// What the other large writes are still to write: the free space this one may count
    /// on is the disk's less that.
    pub fn others(&self) -> u64 {
        self.writes
            .lock()
            .iter()
            .filter(|(id, _)| **id != self.id)
            .map(|(_, r)| r.load(Ordering::Relaxed))
            .fold(0, u64::saturating_add)
    }

    /// Fails unless the disk holding `path` has `bytes` free beside what the other large
    /// writes still need and [`FREE_SPACE_MARGIN`].
    pub fn check(&self, path: &Path, bytes: u64, what: &str) -> Result<(), Error> {
        require_free_space(path, bytes.saturating_add(self.others()), what)
    }
}

impl Drop for Reserved {
    fn drop(&mut self) {
        self.writes.lock().remove(&self.id);
    }
}

// ---------------------------------------------------------------------------------------
// Disks

/// Free space on the disk holding `path` (or its nearest existing parent): the disk whose
/// mount point is the longest prefix of the resolved path.
pub fn free_space(path: &Path) -> Option<u64> {
    let mut existing = path;
    let resolved = loop {
        match existing.canonicalize() {
            Ok(resolved) => break resolved,
            Err(_) => existing = existing.parent()?,
        }
    };
    let resolved = strip_verbatim(resolved);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path_starts_with(&resolved, disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
}

/// `canonicalize` gives `\\?\C:\...` on Windows; mount points are `C:\`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    if cfg!(windows) {
        let path = PathBuf::from(path.to_string_lossy().to_lowercase());
        let prefix = PathBuf::from(prefix.to_string_lossy().to_lowercase());
        path.starts_with(prefix)
    } else {
        path.starts_with(prefix)
    }
}

/// Fails unless the disk holding `path` has `bytes` plus [`FREE_SPACE_MARGIN`] free.
/// `what` completes "Not enough free disk space …", for example "for a backup" or
/// "to restore". Passes when the free space can't be found out.
pub fn require_free_space(path: &Path, bytes: u64, what: &str) -> Result<(), Error> {
    let needed = bytes.saturating_add(FREE_SPACE_MARGIN);
    match free_space(path) {
        Some(free) if free < needed => Err(Error {
            kind: ErrorKind::Conflict,
            source: color_eyre::eyre::eyre!(
                "Not enough free disk space {what}: {} needed, {} free",
                format_bytes(needed),
                format_bytes(free)
            ),
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_writes_count_each_others_share() {
        let writes = Arc::new(LargeWrites::default());
        let upload = writes.reserve(1000);
        let export = writes.reserve(300);
        assert_eq!(upload.others(), 300);
        assert_eq!(export.others(), 1000);
        upload.set_remaining(600);
        assert_eq!(export.others(), 600);
        assert_eq!(writes.total(), 900);
        drop(upload);
        assert_eq!(export.others(), 0);
        // a write the others leave no room for is refused, and holds nothing after
        let huge = writes.reserve(u64::MAX / 2);
        let dir = tempfile::tempdir().unwrap();
        assert!(writes.require(dir.path(), 1, "to test").is_err());
        assert_eq!(writes.total(), 300 + u64::MAX / 2);
        drop(huge);
        assert!(writes.require(dir.path(), 1, "to test").is_ok());
    }
}
