//! Disks: free space, and the room Lodestone always leaves on one.

use std::path::{Path, PathBuf};

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
