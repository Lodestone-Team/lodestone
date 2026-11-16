//! Making many files durable at once, for callers outside the crate that write a batch of
//! files (the Launcher unpacking a world backup): each file and folder handed to the drive
//! as it is written ([`write_out`], [`write_out_dir`]), then one flush of the drive's cache
//! for the whole batch ([`full_flush`]). A flush of the drive's cache per file (std's
//! `sync_all`, `F_FULLFSYNC` on macOS) costs about 6 ms each and doesn't overlap between
//! threads: thousands of files take minutes that way. See `fsx::write_out` for what each
//! platform does.

use std::{fs::File, io, path::Path};

/// Hands `file`'s data to the drive as part of a batch that [`full_flush`] ends.
pub fn write_out(file: &File) -> io::Result<()> {
    super::fsx::write_out(file)
}

/// Hands a folder's entries to the drive as part of a batch that [`full_flush`] ends.
pub fn write_out_dir(dir: &Path) -> io::Result<()> {
    super::fsx::write_out_dir(dir)
}

/// Ends a batch of [`write_out`]s and [`write_out_dir`]s on the file system holding `dir`,
/// making every file and folder of the batch durable.
pub fn full_flush(dir: &Path) -> io::Result<()> {
    super::fsx::full_flush(dir)
}
