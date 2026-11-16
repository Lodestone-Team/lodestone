//! Writing a world backup: a plain zip of the server's level as it is on disk.
//!
//! The zip holds the level folder and, on Paper and Spigot, its split `<level>_nether` and
//! `<level>_the_end` folders, each under its own name, so it is a world any server or tool
//! can use. Each file is checked against the server's disk layout
//! ([`DiskLayout::from_disk`]), as a snapshot checks it: what images never record
//! (`session.lock`, Lodestone's `.lodestone-*` working files) is skipped, and what a
//! restore couldn't place back (a vanilla `DIM-1` inside a split level, a name that isn't
//! UTF-8 or no canonical path holds), links and files that can't be read are left out and
//! counted. Region files and other compressed formats are stored as they are, which keeps
//! the time a running server's saving is paused short; the rest is deflated.
//!
//! Everything here is blocking. The caller holds the server's job lock, and pauses a
//! running server's saving around [`write_world_zip`].

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use color_eyre::eyre::eyre;
use lodestone_image::{DiskLayout, ServerLayout};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::backups::store::{cancelled, err, Cancel};
use crate::error::{Error, ErrorKind};

/// Files whose contents are already compressed, stored as they are: deflating region files
/// gains little and would keep a running server's saving paused longer.
const STORED_EXTENSIONS: [&str; 10] = [
    "mca", "mcr", "mcc", "dat", "dat_old", "nbt", "png", "zip", "gz", "jar",
];

/// Copied this much at a time, checking for cancellation between.
const CHUNK: usize = 64 * 1024;

/// What a world backup will hold, from a walk that reads nothing.
#[derive(Debug, Clone)]
pub struct WorldPlan {
    pub root: PathBuf,
    pub layout: ServerLayout,
    /// The zip's top folders: the level first, then its split folders that exist.
    pub folders: Vec<String>,
    /// The files' sizes added up: the most the zip can take on disk, near enough.
    pub total_bytes: u64,
    pub file_count: u64,
}

/// What [`write_world_zip`] wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub file_count: u64,
    /// Links, files that couldn't be read or vanished, and names the layout can't place.
    pub left_out: u64,
    /// The zip's size.
    pub size: u64,
}

/// One thing in the world's folders, by its disk path relative to the server folder.
enum Item {
    Dir(String),
    File {
        rel: String,
        path: PathBuf,
        size: u64,
    },
}

fn no_world() -> Error {
    err(
        ErrorKind::NotFound,
        "The server has no world yet. Start it once to create one",
    )
}

/// Checks the server in `root` has a world to back up: NotFound when it has no level
/// with a `level.dat`; a level or split folder that is a link is refused.
pub fn check_world(root: &Path, layout: &ServerLayout) -> Result<(), Error> {
    world_folders(root, layout, true).map(|_| ())
}

/// The world's folders in `root` for `layout`, and what they hold. Checked as
/// [`check_world`] checks; with `require_level_dat` false a level folder without a
/// `level.dat` is taken too (a restore's safety backup keeps a damaged world as well).
pub fn plan_world(
    root: &Path,
    layout: &ServerLayout,
    require_level_dat: bool,
) -> Result<WorldPlan, Error> {
    let folders = world_folders(root, layout, require_level_dat)?;
    let mut total_bytes = 0u64;
    let mut file_count = 0u64;
    walk(root, layout, &folders, &mut |item| {
        if let Item::File { size, .. } = item {
            total_bytes = total_bytes.saturating_add(size);
            file_count += 1;
        }
        Ok(())
    })?;
    Ok(WorldPlan {
        root: root.to_path_buf(),
        layout: layout.clone(),
        folders,
        total_bytes,
        file_count,
    })
}

/// The level, then on a split server its `_nether` and `_the_end` folders that are there.
fn world_folders(
    root: &Path,
    layout: &ServerLayout,
    require_level_dat: bool,
) -> Result<Vec<String>, Error> {
    let level = layout.level_name().to_string();
    let mut tops = vec![level.clone()];
    if layout.is_split() {
        tops.push(format!("{level}_nether"));
        tops.push(format!("{level}_the_end"));
    }
    let mut folders = Vec::new();
    for top in tops {
        match fs::symlink_metadata(root.join(&top)) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(err(
                    ErrorKind::BadRequest,
                    format!("{top} is a link, so Lodestone won't back it up"),
                ))
            }
            Ok(meta) if meta.is_dir() => folders.push(top),
            _ => {}
        }
    }
    if folders.first() != Some(&level)
        || (require_level_dat && !root.join(&level).join("level.dat").is_file())
    {
        return Err(no_world());
    }
    Ok(folders)
}

/// Visits the world's folders in a fixed order (by name), giving the folders and files a
/// zip takes, and returns how many it left out.
fn walk(
    root: &Path,
    layout: &ServerLayout,
    folders: &[String],
    visit: &mut dyn FnMut(Item) -> Result<(), Error>,
) -> Result<u64, Error> {
    let disk = DiskLayout::Server(layout.clone());
    let mut left_out = 0u64;
    for top in folders {
        let mut entries = walkdir::WalkDir::new(root.join(top))
            .follow_links(false)
            .sort_by_file_name()
            .into_iter();
        while let Some(entry) = entries.next() {
            let entry = match entry {
                Ok(entry) => entry,
                // a folder that can't be listed: what is in it is left out
                Err(_) => {
                    left_out += 1;
                    continue;
                }
            };
            let Some(rel) = relative(root, entry.path()) else {
                left_out += 1;
                if entry.file_type().is_dir() {
                    entries.skip_current_dir();
                }
                continue;
            };
            if entry.path_is_symlink() || entry.file_type().is_symlink() {
                left_out += 1;
                continue;
            }
            if entry.file_type().is_dir() {
                // a folder is checked by a file inside it
                match disk.from_disk(&format!("{rel}/x")) {
                    Ok(Some(_)) => visit(Item::Dir(rel))?,
                    // never recorded: nothing in it is either
                    Ok(None) => entries.skip_current_dir(),
                    // what is inside is checked (and counted) file by file
                    Err(_) => {}
                }
                continue;
            }
            match disk.from_disk(&rel) {
                Ok(Some(_)) => {
                    let size = entry.metadata().map_or(0, |m| m.len());
                    visit(Item::File {
                        rel,
                        path: entry.path().to_path_buf(),
                        size,
                    })?;
                }
                Ok(None) => {}
                Err(_) => left_out += 1,
            }
        }
    }
    Ok(left_out)
}

/// `path` relative to `root`, `/`-separated; `None` when a part isn't UTF-8.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Whether a file of this name is stored rather than deflated ([`STORED_EXTENSIONS`]).
pub fn stored(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| {
        STORED_EXTENSIONS
            .iter()
            .any(|stored| ext.eq_ignore_ascii_case(stored))
    })
}

/// Whether an entry of `size` bytes needs zip64's large file fields.
pub fn needs_large_file(size: u64) -> bool {
    size >= u64::from(u32::MAX)
}

/// A file's time as a zip has it (to the even second, from 1980); `None` before that.
fn zip_time(time: SystemTime) -> Option<zip::DateTime> {
    use chrono::{Datelike, Timelike};
    let time: chrono::DateTime<chrono::Utc> = time.into();
    zip::DateTime::from_date_and_time(
        u16::try_from(time.year()).ok()?,
        time.month() as u8,
        time.day() as u8,
        time.hour() as u8,
        time.minute() as u8,
        time.second() as u8,
    )
    .ok()
}

fn file_options(name: &str, size: u64, modified: Option<SystemTime>) -> SimpleFileOptions {
    let mut options = SimpleFileOptions::default()
        .compression_method(if stored(name) {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        })
        .large_file(needs_large_file(size))
        .unix_permissions(0o644);
    if let Some(time) = modified.and_then(zip_time) {
        options = options.last_modified_time(time);
    }
    options
}

/// Writes the world `plan` names, walked again now (inside a running server's save pause,
/// so new region files are taken), as a zip at `out_part`, which must not exist. Tells
/// `progress` the bytes copied and the plan's total. On any failure or cancellation the
/// part file is deleted, so nothing is left; on success it is flushed to disk.
pub fn write_world_zip(
    plan: &WorldPlan,
    out_part: &Path,
    progress: &dyn Fn(u64, u64),
    cancel: Cancel,
) -> Result<Written, Error> {
    if let Some(parent) = out_part.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::from(eyre!(e).wrap_err("Failed to create the world backups' folder"))
        })?;
    }
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(out_part)
        .map_err(|e| Error::from(eyre!(e).wrap_err("Failed to create the world backup")))?;
    let written = write_into(plan, file, progress, cancel);
    if written.is_err() {
        if let Err(e) = fs::remove_file(out_part) {
            tracing::warn!("Failed to delete {}: {e}", out_part.display());
        }
    }
    written
}

fn write_into(
    plan: &WorldPlan,
    file: File,
    progress: &dyn Fn(u64, u64),
    cancel: Cancel,
) -> Result<Written, Error> {
    let zip_error = |e: zip::result::ZipError| {
        Error::from(eyre!(e).wrap_err("Failed to write the world backup"))
    };
    let io_error =
        |e: io::Error| Error::from(eyre!(e).wrap_err("Failed to write the world backup"));
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let mut copied = 0u64;
    let mut file_count = 0u64;
    let mut vanished = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    let left_out = walk(&plan.root, &plan.layout, &plan.folders, &mut |item| {
        if cancel() {
            return Err(cancelled());
        }
        match item {
            Item::Dir(rel) => {
                let options = SimpleFileOptions::default().unix_permissions(0o755);
                zip.add_directory(format!("{rel}/"), options)
                    .map_err(zip_error)?;
            }
            Item::File { rel, path, .. } => {
                // gone, or can't be opened: left out
                let Ok(mut source) = File::open(&path) else {
                    vanished += 1;
                    return Ok(());
                };
                let meta = source.metadata().map_err(io_error)?;
                zip.start_file(
                    rel.as_str(),
                    file_options(&rel, meta.len(), meta.modified().ok()),
                )
                .map_err(zip_error)?;
                loop {
                    if cancel() {
                        return Err(cancelled());
                    }
                    let read = match source.read(&mut buffer) {
                        Ok(read) => read,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            // couldn't be read through: taken out again
                            zip.abort_file().map_err(zip_error)?;
                            vanished += 1;
                            return Ok(());
                        }
                    };
                    if read == 0 {
                        break;
                    }
                    zip.write_all(&buffer[..read]).map_err(io_error)?;
                    copied = copied.saturating_add(read as u64);
                    progress(copied, plan.total_bytes);
                }
                file_count += 1;
            }
        }
        Ok(())
    })?;
    let writer = zip.finish().map_err(zip_error)?;
    let file = writer.into_inner().map_err(|e| io_error(e.into_error()))?;
    file.sync_all().map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    Ok(Written {
        file_count,
        left_out: left_out + vanished,
        size,
    })
}

#[cfg(test)]
pub(crate) mod tests;
