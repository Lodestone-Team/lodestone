//! Reading a world backup back: the checked plan of what a zip places, and its extraction
//! into a staging folder.
//!
//! The zip is treated as untrusted even though the core wrote it: its folder is protected
//! from the file API, but a zip is still a file on a disk anyone with access to the
//! computer can change, and a mistake here writes into a server. So before anything is
//! written:
//!
//! - at most [`MAX_UNZIP_ENTRIES`] entries and [`MAX_UNZIP_BYTES`] declared, in all and per
//!   entry; only Stored and Deflated entries, none encrypted, none a link;
//! - local headers must match the central directory, with no name twice (a reader that
//!   takes the local name would otherwise write another path than the one checked);
//! - names must be UTF-8, `/`-separated, without `\`, a leading `/`, or empty, `.` or `..`
//!   parts, and only folders may end with `/`;
//! - the top folders must be the backup's level and its split `_nether` and `_the_end`
//!   folders;
//! - each path maps through its canonical world path (`saves/<level>/…`, which checks every
//!   part: Windows names, NFC, invisible characters, lengths) into the server's current
//!   level and layout, and no two of them may be one file or folder on a disk that ignores
//!   letter case, nor a file where a folder goes.
//!
//! Extraction then writes each file with `create_new` and never more bytes than declared
//! (the zip crate checks each file's CRC too). The caller checks the disk has room for the
//! declared total first.

use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

use color_eyre::eyre::eyre;
use lodestone_image::layout::join_disk;
use lodestone_image::paths::case_collisions;
use lodestone_image::{CanonicalPath, DiskLayout, ServerLayout};
use zip::CompressionMethod;

use crate::backups::store::{cancelled, err, Cancel};
use crate::error::{Error, ErrorKind};
use crate::util::{MAX_UNZIP_BYTES, MAX_UNZIP_ENTRIES};

/// Copied this much at a time, checking for cancellation between.
const CHUNK: usize = 64 * 1024;

/// The bounds a zip is read within; [`ZipLimits::default`] outside tests.
#[derive(Debug, Clone, Copy)]
pub struct ZipLimits {
    pub max_entries: u64,
    pub max_bytes: u64,
}

impl Default for ZipLimits {
    fn default() -> Self {
        ZipLimits {
            max_entries: MAX_UNZIP_ENTRIES,
            max_bytes: MAX_UNZIP_BYTES,
        }
    }
}

/// A file the restore places.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// Where it goes, relative to the server folder, `/`-separated.
    pub disk: String,
    /// Its entry in the zip.
    pub index: usize,
    /// Its declared size.
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

/// What restoring a zip places, checked (see the module).
#[derive(Debug, Clone)]
pub struct RestoreZipPlan {
    pub files: Vec<Placed>,
    /// Folders the zip has, relative to the server folder (empty ones are kept).
    pub folders: Vec<String>,
    /// The files' declared sizes added up.
    pub total_bytes: u64,
    /// The top-level folders the restore owns: the server's level, and on a split server
    /// its `_nether` and `_the_end` folders.
    pub tops: Vec<String>,
}

fn refused(why: impl std::fmt::Display) -> Error {
    err(
        ErrorKind::BadRequest,
        format!("The world backup can't be restored: {why}"),
    )
}

/// `saves/<from>/rest` as `saves/<to>/rest`, checked again.
fn into_level(path: &CanonicalPath, to: &str) -> Result<CanonicalPath, Error> {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        ["saves", _, rest @ ..] if !rest.is_empty() => {
            CanonicalPath::new(format!("saves/{to}/{}", rest.join("/")))
                .map_err(|e| refused(format!("{path}: {e}")))
        }
        _ => Err(refused(format!("{path} isn't in the world"))),
    }
}

/// Checks a name from the zip (see the module); returns it without a folder's final `/`.
fn checked_name(raw: &[u8], is_dir: bool) -> Result<String, Error> {
    let name = std::str::from_utf8(raw).map_err(|_| refused("a name isn't UTF-8"))?;
    if name.contains('\\') || name.starts_with('/') {
        return Err(refused(format!("{name:?} isn't a relative path")));
    }
    let trimmed = match name.strip_suffix('/') {
        Some(trimmed) if is_dir => trimmed,
        Some(_) => return Err(refused(format!("{name:?} isn't a file name"))),
        None => name,
    };
    if trimmed
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(refused(format!("{name:?} isn't a plain path")));
    }
    Ok(trimmed.to_string())
}

fn system_time(time: zip::DateTime) -> Option<std::time::SystemTime> {
    let date = chrono::NaiveDate::from_ymd_opt(
        i32::from(time.year()),
        u32::from(time.month()),
        u32::from(time.day()),
    )?;
    let at = date.and_hms_opt(
        u32::from(time.hour()),
        u32::from(time.minute()),
        u32::from(time.second()),
    )?;
    Some(at.and_utc().into())
}

fn open(zip_path: &Path) -> Result<zip::ZipArchive<BufReader<File>>, Error> {
    let file = File::open(zip_path)
        .map_err(|e| Error::from(eyre!(e).wrap_err("Failed to open the world backup")))?;
    zip::ZipArchive::new(BufReader::new(file)).map_err(refused)
}

/// The checked plan of restoring the zip at `zip_path`, whose level is `level` (its
/// sidecar's), into a server whose layout is `target` now.
pub fn plan_restore(
    zip_path: &Path,
    level: &str,
    target: &ServerLayout,
) -> Result<RestoreZipPlan, Error> {
    plan_restore_with(zip_path, level, target, ZipLimits::default())
}

/// [`plan_restore`] within `limits`.
pub fn plan_restore_with(
    zip_path: &Path,
    level: &str,
    target: &ServerLayout,
    limits: ZipLimits,
) -> Result<RestoreZipPlan, Error> {
    let mut archive = open(zip_path)?;
    if archive.len() as u64 > limits.max_entries {
        return Err(refused(format!(
            "it has more than {} entries",
            limits.max_entries
        )));
    }
    // duplicate names, and local headers that differ from the central directory
    archive.validate_header_consistency().map_err(refused)?;
    let nether = format!("{level}_nether");
    let end = format!("{level}_the_end");

    // (name, is_dir, index, size, modified)
    let mut entries = Vec::with_capacity(archive.len());
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index_data(index).map_err(refused)?;
        if entry.encrypted() {
            return Err(refused("it has an encrypted entry"));
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(refused(format!(
                "it uses the compression method {:?}",
                entry.compression()
            )));
        }
        if entry.is_symlink() {
            return Err(refused("it has a link"));
        }
        let is_dir = entry.is_dir();
        let name = checked_name(entry.name_raw(), is_dir)?;
        let size = if is_dir { 0 } else { entry.size() };
        if size > limits.max_bytes {
            return Err(refused(format!("{name} is too large")));
        }
        total_bytes = total_bytes.saturating_add(size);
        if total_bytes > limits.max_bytes {
            return Err(refused("it expands to too much"));
        }
        let top = name.split('/').next().unwrap_or_default();
        if top != level && top != nether && top != end {
            return Err(refused(format!("{name} isn't in the world {level}")));
        }
        let modified = entry.last_modified().and_then(system_time);
        entries.push((name, is_dir, index, size, modified));
    }
    let split = entries.iter().any(|(name, ..)| {
        let top = name.split('/').next().unwrap_or_default();
        top == nether || top == end
    });
    let source = ServerLayout::new(level.to_string(), [], split)
        .map(DiskLayout::Server)
        .map_err(|e| refused(format!("its level {level:?} isn't a world: {e}")))?;
    let target_disk = DiskLayout::Server(target.clone());
    let target_level = target.level_name();

    let mut files = Vec::new();
    let mut folders = Vec::new();
    let mut canonical = Vec::new();
    for (name, is_dir, index, size, modified) in entries {
        // a folder is mapped by a file inside it
        let probe = if is_dir {
            format!("{name}/x")
        } else {
            name.clone()
        };
        let Some(path) = source
            .from_disk(&probe)
            .map_err(|e| refused(format!("{name}: {e}")))?
        else {
            // never recorded: `session.lock`, Lodestone's working files
            continue;
        };
        let path = into_level(&path, target_level)?;
        let disk = target_disk
            .to_disk(&path)
            .map_err(|e| refused(format!("{name}: {e}")))?;
        if is_dir {
            let folder = disk.strip_suffix("/x").unwrap_or(&disk).to_string();
            if let Ok(path) =
                CanonicalPath::new(path.as_str().strip_suffix("/x").unwrap_or(path.as_str()))
            {
                canonical.push(path);
            }
            folders.push(folder);
        } else {
            canonical.push(path);
            files.push(Placed {
                disk,
                index,
                size,
                modified,
            });
        }
    }
    if let Some((first, second)) = case_collisions(canonical.iter()).into_iter().next() {
        return Err(refused(format!(
            "{first} and {second} are one file on some disks"
        )));
    }
    // a file twice, or a file where a folder goes
    let mut placed = HashSet::new();
    for file in &files {
        if !placed.insert(file.disk.as_str()) {
            return Err(refused(format!("{} is in it twice", file.disk)));
        }
    }
    let mut implied: BTreeSet<&str> = folders.iter().map(String::as_str).collect();
    for file in &files {
        let mut path = file.disk.as_str();
        while let Some((parent, _)) = path.rsplit_once('/') {
            implied.insert(parent);
            path = parent;
        }
    }
    if let Some(file) = files.iter().find(|f| implied.contains(f.disk.as_str())) {
        return Err(refused(format!(
            "{} is both a file and a folder",
            file.disk
        )));
    }
    Ok(RestoreZipPlan {
        files,
        folders,
        total_bytes,
        tops: super::writer::world_tops(target),
    })
}

/// Extracts what `plan` places from the zip at `zip_path` into `staging` (an empty
/// folder), telling `progress` the bytes written and the declared total. The caller
/// deletes `staging` when this fails or is cancelled.
pub fn extract_into(
    zip_path: &Path,
    plan: &RestoreZipPlan,
    staging: &Path,
    progress: &dyn Fn(u64, u64),
    cancel: Cancel,
) -> Result<(), Error> {
    let io_error = |e: io::Error, what: &str| {
        Error::from(eyre!(e).wrap_err(format!("Failed to restore {what}")))
    };
    let mut archive = open(zip_path)?;
    for folder in &plan.folders {
        fs::create_dir_all(join_disk(staging, folder)).map_err(|e| io_error(e, folder))?;
    }
    let mut written = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    for file in &plan.files {
        if cancel() {
            return Err(cancelled());
        }
        let dest = join_disk(staging, &file.disk);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(e, &file.disk))?;
        }
        let mut out = File::options()
            .write(true)
            .create_new(true)
            .open(&dest)
            .map_err(|e| io_error(e, &file.disk))?;
        let entry = archive.by_index(file.index).map_err(refused)?;
        // never more than declared: one byte past it shows the entry lied
        let mut reader = entry.take(file.size.saturating_add(1));
        let mut copied = 0u64;
        loop {
            if cancel() {
                return Err(cancelled());
            }
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(refused(format!("{} is damaged: {e}", file.disk))),
            };
            if read == 0 {
                break;
            }
            copied += read as u64;
            if copied > file.size {
                return Err(refused(format!("{} is larger than it says", file.disk)));
            }
            out.write_all(&buffer[..read])
                .map_err(|e| io_error(e, &file.disk))?;
            written = written.saturating_add(read as u64);
            progress(written, plan.total_bytes);
        }
        if copied != file.size {
            return Err(refused(format!(
                "{} is damaged (shorter than it says)",
                file.disk
            )));
        }
        if let Some(modified) = file.modified {
            // only the time it shows; the world is right either way
            let _ = out.set_modified(modified);
        }
        // on disk before the swap puts it in place and deletes the world it replaces. Only
        // on unix, where the folders are flushed too (`sync_tree`): on Windows a folder
        // can't be, so flushing every file (FlushFileBuffers) would make restores of big
        // worlds minutes slower and still not make the swap durable
        #[cfg(unix)]
        out.sync_all().map_err(|e| io_error(e, &file.disk))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
