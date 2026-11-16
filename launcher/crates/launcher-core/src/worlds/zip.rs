//! World zips: writing one from a world folder ([`write_zip`]), and checking
//! ([`check`]) and unpacking ([`extract_into`]) one that may come from anyone. World
//! exports and imports use them, and so do world backups ([`crate::world_backups`]),
//! whose zips sit in the data folder where anything that can write it could replace them.
//!
//! A zip holds what an image could hold of the world: every name is checked as the
//! world's path in the game folder would be (`saves/<world>/<path>`,
//! [`CanonicalPath::new`]), so a name a zip writes is one every disk Lodestone runs on can
//! keep, and a name it reads is one that can't escape the world, alias another file on
//! Windows or macOS, or hide as an invisible character. Links are never followed nor
//! written, and `session.lock` and Lodestone's working files (`.lodestone-…`) never go in.
//!
//! [`check`] reads only the zip's end records and central directory (and `level.dat`),
//! the list every later step uses (a local header naming another path is never read),
//! and refuses the whole zip for any name it can't take, rather than leaving a file out
//! without a word. Opening a zip never scans its body ([`open_archive`]), so checking a
//! large backup costs about as much as a small one. [`extract_into`]
//! then never trusts a size the zip states: each file is copied up to its stated size
//! plus one byte, and one that is larger refuses the zip.
//!
//! Files that are already compressed (region files, whose chunks are zlib streams, and
//! gzip NBT such as `level.dat`) are stored as they are rather than deflated again, which
//! gains almost nothing and would make zipping a large world bound by one CPU core; every
//! zip tool reads stored entries.

use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_image::paths::{CanonicalPath, always_excluded, case_collisions};
use lodestone_minecraft::{
    level::{self, LevelSummary, MAX_LEVEL_DAT_BYTES},
    mrpack::{PathCheck, WriteAll, check_pack_path},
};

use super::{WorldError, io_error, random_suffix};
use crate::game::download::{Progress, ProgressSnapshot};

/// A world zip may have at most this many entries, and this many bytes unpacked.
pub const MAX_ZIP_ENTRIES: usize = 500_000;
pub const MAX_ZIP_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// How many of the names a zip left out are kept to show.
pub const MAX_LEFT_OUT_NAMES: usize = 20;

/// What [`check`] accepts: at most this many entries, and this many bytes unpacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: MAX_ZIP_ENTRIES,
            max_bytes: MAX_ZIP_BYTES,
        }
    }
}

/// How a zip's world is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Every entry is under one top folder, which holds `level.dat`: a world backup.
    OneWorldFolder,
    /// `level.dat` at the root or one folder down, entries elsewhere ignored: a world
    /// someone shared, which may come from any tool.
    Import,
    /// Like [`Shape::OneWorldFolder`], with `level.dat` optional (its summary empty when
    /// missing): a backup the startup sweep found without its details, which may be the
    /// safety backup of a world that had none, kept so it can be listed and deleted.
    OneFolder,
}

/// What [`write_zip`] wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    /// The zip's size.
    pub bytes: u64,
    /// The bytes of the files zipped.
    pub world_bytes: u64,
    /// The files zipped, relative to the world folder with `/`.
    pub files: Vec<String>,
    /// Names left out (links, names no disk can keep everywhere), at most
    /// [`MAX_LEFT_OUT_NAMES`] of them.
    pub left_out: Vec<String>,
    pub left_out_count: u64,
}

/// One entry [`extract_into`] writes.
#[derive(Debug, Clone, PartialEq)]
struct Planned {
    index: usize,
    /// Where it goes, relative to the world folder with `/`.
    relative: String,
    is_dir: bool,
    size: u64,
}

/// A world zip that passed [`check`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckedZip {
    /// `""` when `level.dat` is at the root, else `"<folder>/"`.
    pub prefix: String,
    pub summary: LevelSummary,
    /// A folder name to use: the zip's folder, or the zip's name.
    pub folder_hint: String,
    /// The bytes its files say they unpack to.
    pub declared_bytes: u64,
    entries: Vec<Planned>,
    /// The shape it was checked as, which [`extract_into`] opens it as again.
    shape: Shape,
}

impl CheckedZip {
    /// The files it places, relative to the world folder.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.relative.as_str())
    }

    /// The folders it creates, relative to the world folder (empty ones included).
    pub fn folders(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.relative.as_str())
    }
}

fn zip_error(path: &Path) -> impl FnOnce(zip::result::ZipError) -> WorldError + '_ {
    move |err| {
        WorldError::Invalid(format!(
            "{} isn't a zip that can be read: {err}",
            path.display()
        ))
    }
}

/// The canonical path a file of a world would have in the game folder, or why it can't.
/// The world's own folder is checked elsewhere ([`crate::content::is_plain_name`]).
fn world_path(relative: &str) -> Result<CanonicalPath, String> {
    CanonicalPath::new(format!("saves/w/{relative}")).map_err(|e| e.to_string())
}

/// Why a zip of the world folder `folder` couldn't be restored, if it couldn't: its name
/// must pass what [`check`] asks of every entry (the top folder's included) and be a
/// canonical path in the game folder (`saves/<folder>`), as a restore's swap asks.
/// [`crate::content::is_plain_name`], which finds the world, allows names these refuse,
/// such as `aux`, or `World.` outside Windows. World backups check this before zipping,
/// so a backup is never made that no restore would take.
pub fn check_world_folder(folder: &str) -> Result<(), WorldError> {
    let refused = |why: String| {
        WorldError::Invalid(format!(
            "The world's folder name {folder:?} can't be restored from a zip ({why}). Rename the folder and try again."
        ))
    };
    check_pack_path(&format!("{folder}/"), true, &WriteAll).map_err(|e| refused(e.to_string()))?;
    CanonicalPath::new(format!("saves/{folder}")).map_err(|e| refused(e.to_string()))?;
    Ok(())
}

/// What a zip of a world folder will take ([`plan`]): the files and folders under it,
/// relative with `/`, with each file's size, and the names it leaves out.
#[derive(Debug, Clone, Default)]
pub struct ZipPlan {
    entries: Vec<Entry>,
    left_out: Vec<String>,
    left_out_count: u64,
}

#[derive(Debug, Clone)]
struct Entry {
    relative: String,
    path: PathBuf,
    is_dir: bool,
    /// The file's size when listed.
    size: u64,
}

impl ZipPlan {
    /// The files it takes, relative to the world folder.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.relative.as_str())
    }

    /// The bytes of the files it takes, as listed.
    pub fn bytes(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| e.size)
            .fold(0, u64::saturating_add)
    }

    /// Whether the zip [`write_planned`] makes of it passes [`check`] with `limits`, which
    /// counts every entry (the top folder, each folder and each file) and adds up the
    /// files' sizes. A world backup is only made within the limits its restore reads, or
    /// it would be listed like any other and refused the day it is needed. `Err` says
    /// which limit the world is over, in words that follow "it has" or "its files are".
    pub fn within(&self, limits: Limits) -> Result<(), String> {
        // `+ 1`: the top folder's own entry.
        if self.entries.len().saturating_add(1) > limits.max_entries {
            return Err(format!(
                "more than {} files and folders",
                limits.max_entries
            ));
        }
        if self.bytes() > limits.max_bytes {
            return Err(format!("more than {} GiB of files", limits.max_bytes >> 30));
        }
        Ok(())
    }
}

/// What a zip of the world folder `dir`, its files under `prefix/`, takes. Links are never
/// followed. A folder whose name is left out is left out whole. A name is taken only when
/// [`check`] would take it in the zip.
///
/// Of two names that differ only in letter case (a disk that keeps case apart can hold
/// both), the later in sorted order is left out, with what is in it: [`check`] refuses
/// such a zip, so it could never be restored.
///
/// A folder that holds only names the zip doesn't take is left out too, rather than going
/// in empty: restoring in place carries such a folder back whole from the world on disk,
/// which it couldn't do were the zip to place an empty folder of that name there.
pub fn plan(dir: &Path, prefix: &str) -> Result<ZipPlan, WorldError> {
    let mut found = ZipPlan::default();
    fn leave_out(found: &mut ZipPlan, name: String) {
        found.left_out_count += 1;
        if found.left_out.len() < MAX_LEFT_OUT_NAMES {
            found.left_out.push(name);
        }
    }
    // The folders holding a name not taken (left out, or never zipped).
    let mut held_back: HashSet<String> = HashSet::new();
    let mut hold_back = |child: &str| {
        let mut folder = child;
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if !held_back.insert(parent.to_string()) {
                break;
            }
            folder = parent;
        }
    };
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((path, relative)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&path)
            .map_err(io_error(&path))?
            .collect::<Result<_, _>>()
            .map_err(io_error(&path))?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if entry.file_name().to_str().is_none() {
                hold_back(&child);
                leave_out(&mut found, child);
                continue;
            }
            let kind = entry.file_type().map_err(io_error(&entry.path()))?;
            let canonical = match world_path(&child) {
                // `check` also refuses some names a canonical path allows (`a..b`), and
                // zip names longer than it takes.
                Ok(canonical)
                    if check_pack_path(&format!("{prefix}/{child}"), kind.is_dir(), &WriteAll)
                        .is_ok() =>
                {
                    canonical
                }
                _ => {
                    hold_back(&child);
                    leave_out(&mut found, child);
                    continue;
                }
            };
            if always_excluded(&canonical) {
                hold_back(&child);
                continue;
            }
            if kind.is_dir() {
                found.entries.push(Entry {
                    relative: child.clone(),
                    path: entry.path(),
                    is_dir: true,
                    size: 0,
                });
                stack.push((entry.path(), child));
            } else if kind.is_file() {
                // The entry's own metadata: a link is never followed.
                let size = entry.metadata().map_err(io_error(&entry.path()))?.len();
                found.entries.push(Entry {
                    relative: child,
                    path: entry.path(),
                    is_dir: false,
                    size,
                });
            } else {
                hold_back(&child);
                leave_out(&mut found, child);
            }
        }
    }
    // Two names one file on a disk that ignores case (only a disk that keeps case apart
    // can hold both): the later is left out, with everything in it, since `check` would
    // refuse the zip on every restore.
    found.entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    let canonical: Vec<CanonicalPath> = found
        .entries
        .iter()
        .filter_map(|e| world_path(&e.relative).ok())
        .collect();
    if canonical.len() == found.entries.len() {
        let later: HashSet<String> = case_collisions(canonical.iter())
            .into_iter()
            .map(|(_, later)| later.as_str()["saves/w/".len()..].to_string())
            .collect();
        let under_later = |relative: &str| {
            let mut rest = relative;
            while let Some((parent, _)) = rest.rsplit_once('/') {
                if later.contains(parent) {
                    return true;
                }
                rest = parent;
            }
            false
        };
        let mut dropped = Vec::new();
        found.entries.retain(|e| {
            let gone = later.contains(&e.relative) || under_later(&e.relative);
            if gone && !under_later(&e.relative) {
                dropped.push(e.relative.clone());
            }
            !gone
        });
        for relative in dropped {
            hold_back(&relative);
            leave_out(&mut found, relative);
        }
    }
    // A held-back folder stays when the zip takes something inside it: a file, or a folder
    // that isn't held back itself.
    let mut keep: HashSet<String> = HashSet::new();
    for entry in &found.entries {
        if entry.is_dir && held_back.contains(&entry.relative) {
            continue;
        }
        let mut folder = entry.relative.as_str();
        while let Some((parent, _)) = folder.rsplit_once('/') {
            if !keep.insert(parent.to_string()) {
                break;
            }
            folder = parent;
        }
    }
    found
        .entries
        .retain(|e| !e.is_dir || !held_back.contains(&e.relative) || keep.contains(&e.relative));
    found.entries.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(found)
}

/// Files this large or larger are stored as they are, whatever their name. A large file in
/// a world is region-like data or a mod's database of chunks it compressed itself
/// (Distant Horizons' `DistantHorizons.sqlite`, often several GiB), never text: deflating
/// it gains nothing and runs on one core at about 60 MB/s, minutes for a few GiB.
const STORE_FROM: u64 = 64 << 20;

/// Whether a zip stores the file `name` of `len` bytes as it is rather than deflating it:
/// files already compressed (region files' zlib chunks, gzip NBT such as `level.dat` and
/// player data, mods' SQLite databases of compressed chunks, images, sound and archives),
/// and any file of [`STORE_FROM`] or more.
fn stored_as_is(name: &str, len: u64) -> bool {
    let lower = name.to_ascii_lowercase();
    len >= STORE_FROM
        || [
            ".mca",
            ".mcc",
            ".mcr",
            ".dat",
            ".dat_old",
            ".nbt",
            ".png",
            ".jpg",
            ".jpeg",
            ".ogg",
            ".gz",
            ".zip",
            ".jar",
            ".zst",
            ".lz4",
            ".xz",
            ".7z",
            ".sqlite",
            ".sqlite-wal",
            ".sqlite-shm",
        ]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// Opens the file at `path` for zipping, never through a link (one swapped in after the
/// world was listed would put a file from outside the world in the zip) and never anything
/// but a plain file (a pipe would make the zip wait forever).
fn open_listed(path: &Path) -> Result<File, WorldError> {
    let mut options = File::options();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(io_error(path))?;
    if !file.metadata().map_err(io_error(path))?.is_file() {
        return Err(WorldError::Invalid(format!(
            "{} changed while it was being zipped; try again.",
            path.display()
        )));
    }
    Ok(file)
}

/// How much of a file is copied between two checks of Cancel (and progress updates).
const COPY_CHUNK: usize = 8 * 1024 * 1024;

/// Files this large or larger are written with zip64 sizes. Below `u32::MAX` with room to
/// spare: a deflated file that doesn't compress (Distant Horizons' database, say) comes
/// out a little larger than it went in, and one just under 4 GiB would otherwise overflow
/// its 32-bit sizes and fail every backup of its world. Zip64 costs 20 bytes a file.
const LARGE_FILE_FROM: u64 = 2 << 30;

fn zip_time(time: Option<SystemTime>) -> zip::DateTime {
    use chrono::{Datelike as _, Timelike as _};
    time.map(chrono::DateTime::<chrono::Local>::from)
        .and_then(|t| {
            zip::DateTime::from_date_and_time(
                u16::try_from(t.year()).ok()?,
                t.month() as u8,
                t.day() as u8,
                t.hour() as u8,
                t.minute() as u8,
                t.second() as u8,
            )
            .ok()
        })
        .unwrap_or_default()
}

/// Zips the world folder `dir` to `out`, its files under `<prefix>/`. Written to
/// `.<name>.<16 hex>.part` beside `out`, flushed, then renamed, so `out` is never half a
/// zip; a failure or `stop` leaves nothing. `stop` is asked before each file and ends the
/// zip with its error (Cancel, or the game opening the world).
pub fn write_zip(
    dir: &Path,
    prefix: &str,
    out: &Path,
    progress: &Progress,
    stop: &dyn Fn() -> Result<(), WorldError>,
) -> Result<Written, WorldError> {
    let planned = plan(dir, prefix)?;
    write_planned(dir, prefix, planned, out, progress, stop)
}

/// [`write_zip`] of what [`plan`] listed. `stop` is also asked between chunks of a large
/// file.
pub fn write_planned(
    dir: &Path,
    prefix: &str,
    planned: ZipPlan,
    out: &Path,
    progress: &Progress,
    stop: &dyn Fn() -> Result<(), WorldError>,
) -> Result<Written, WorldError> {
    let entries = &planned.entries;
    let total_bytes = planned.bytes();
    let total_files = entries.iter().filter(|e| !e.is_dir).count() as u64;
    let mut done = ProgressSnapshot {
        files_total: total_files,
        bytes_total: total_bytes,
        ..ProgressSnapshot::default()
    };
    progress.set(done);
    let parent = out
        .parent()
        .ok_or_else(|| WorldError::Invalid(format!("{} has no folder", out.display())))?;
    fs::create_dir_all(parent).map_err(io_error(parent))?;
    let tmp = parent.join(format!(
        ".{}.{}.part",
        out.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("world.zip"),
        random_suffix()
    ));
    let mut files = Vec::with_capacity(total_files as usize);
    let result = (|| {
        let file = File::create(&tmp).map_err(io_error(&tmp))?;
        // Buffered: the zip writer's own writes (headers, stored entries) are small.
        let mut zip = zip::ZipWriter::new(io::BufWriter::with_capacity(1 << 20, file));
        let base = zip::write::SimpleFileOptions::default();
        let zip_err = |err: zip::result::ZipError| {
            WorldError::Invalid(format!("couldn't write the zip: {err}"))
        };
        zip.add_directory(
            format!("{prefix}/"),
            base.last_modified_time(zip_time(fs::metadata(dir).and_then(|m| m.modified()).ok())),
        )
        .map_err(zip_err)?;
        let mut buffer = vec![0u8; 256 * 1024];
        for entry in entries {
            stop()?;
            let (relative, path) = (&entry.relative, &entry.path);
            if entry.is_dir {
                let modified = fs::symlink_metadata(path).and_then(|m| m.modified()).ok();
                zip.add_directory(
                    format!("{prefix}/{relative}/"),
                    base.last_modified_time(zip_time(modified)),
                )
                .map_err(zip_err)?;
                continue;
            }
            let mut source = open_listed(path)?;
            let metadata = source.metadata().map_err(io_error(path))?;
            let method = if stored_as_is(relative, metadata.len()) {
                zip::CompressionMethod::Stored
            } else {
                zip::CompressionMethod::Deflated
            };
            let options = base
                .compression_method(method)
                .last_modified_time(zip_time(metadata.modified().ok()))
                .large_file(metadata.len() >= LARGE_FILE_FROM);
            zip.start_file(format!("{prefix}/{relative}"), options)
                .map_err(zip_err)?;
            // In chunks, asking `stop` between them, so Cancel works in a large file too.
            let mut since_check = 0usize;
            loop {
                let read = match source.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(io_error(path)(e)),
                };
                io::Write::write_all(&mut zip, &buffer[..read]).map_err(io_error(&tmp))?;
                done.bytes_done += read as u64;
                since_check += read;
                if since_check >= COPY_CHUNK {
                    since_check = 0;
                    progress.set(done);
                    stop()?;
                }
            }
            files.push(relative.clone());
            done.files_done += 1;
            progress.set(done);
        }
        stop()?;
        let file = zip
            .finish()
            .map_err(zip_err)?
            .into_inner()
            .map_err(|err| io_error(&tmp)(err.into_error()))?;
        file.sync_all().map_err(io_error(&tmp))?;
        drop(file);
        crate::fsx::rename(&tmp, out).map_err(io_error(out))?;
        // The rename itself, durably (NTFS journals it on Windows).
        #[cfg(unix)]
        if let Ok(folder) = File::open(parent) {
            let _ = folder.sync_all();
        }
        Ok(fs::metadata(out).map_or(0, |m| m.len()))
    })();
    match result {
        Ok(bytes) => Ok(Written {
            bytes,
            world_bytes: done.bytes_done,
            files,
            left_out: planned.left_out,
            left_out_count: planned.left_out_count,
        }),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Files macOS adds to zips it makes, which no world needs.
fn is_mac_metadata(name: &str) -> bool {
    name.starts_with("__MACOSX/") || name.split('/').any(|c| c == ".DS_Store")
}

/// One entry of the central directory, as read.
struct Raw {
    index: usize,
    name: String,
    is_dir: bool,
    size: u64,
}

fn unsafe_zip(err: impl std::fmt::Display) -> WorldError {
    WorldError::Invalid(
        err.to_string()
            .replace("The modpack is unsafe", "The zip is unsafe"),
    )
}

/// Opens the zip at `path`, read through `open` (a new reader each time it's called), for
/// reading its central directory. Its archive starts at the file's first byte, as every
/// zip Lodestone writes does: `zip`'s default instead finds the start by scanning the file
/// from its first byte to the central directory, reading the whole zip just to open it
/// (seconds for a backup of a few GiB, minutes from a hard drive, with neither progress
/// nor Cancel). Only a zip to import, which may come from any tool, falls back to that
/// scan, and only when something other than a zip record starts the file (a
/// self-extracting stub): a damaged zip that starts like one is refused at once.
fn open_archive<R: io::Read + io::Seek>(
    path: &Path,
    open: &impl Fn() -> io::Result<R>,
    shape: Shape,
) -> Result<zip::ZipArchive<R>, WorldError> {
    let open = || open().map_err(io_error(path));
    match open_at_start(open()?) {
        Err(err) => {
            if shape == Shape::Import && !starts_as_zip(open()?) {
                zip::ZipArchive::new(open()?).map_err(zip_error(path))
            } else {
                Err(zip_error(path)(err))
            }
        }
        opened => opened.map_err(zip_error(path)),
    }
}

/// Whether `reader` starts with a zip record: a file's local header, or the end record of
/// an empty zip. Anything else (or a file too short for either) is something before the
/// archive.
fn starts_as_zip(mut reader: impl io::Read) -> bool {
    let mut start = [0u8; 4];
    reader.read_exact(&mut start).is_ok()
        && matches!(&start, b"PK\x03\x04" | b"PK\x05\x06" | b"PK\x07\x08")
}

/// `reader`'s zip, taken to start at its first byte ([`open_archive`]).
fn open_at_start<R: io::Read + io::Seek>(reader: R) -> zip::result::ZipResult<zip::ZipArchive<R>> {
    let config = zip::read::Config {
        archive_offset: zip::read::ArchiveOffset::Known(0),
    };
    zip::ZipArchive::with_config(config, reader)
}

/// Opens the world zip at `path` and checks every entry (see the module): its size and
/// entry count against `limits`, encryption, each name, two names one file on a disk that
/// ignores case, a file that is also a folder, and the world's layout for `shape`. Reads
/// `level.dat`, nothing else.
pub fn check(path: &Path, shape: Shape, limits: Limits) -> Result<CheckedZip, WorldError> {
    check_with(
        path,
        &|| File::open(path).map(io::BufReader::new),
        shape,
        limits,
    )
}

/// [`check`], reading the zip through `open` (a new reader each time it's called), so a
/// test can count what a check reads. `path` names the zip in errors.
fn check_with<R: io::Read + io::Seek>(
    path: &Path,
    open: &impl Fn() -> io::Result<R>,
    shape: Shape,
    limits: Limits,
) -> Result<CheckedZip, WorldError> {
    let mut archive = open_archive(path, open, shape)?;
    if archive.len() > limits.max_entries {
        return Err(WorldError::Invalid(format!(
            "The zip has more than {} files.",
            limits.max_entries
        )));
    }
    let too_large = || {
        WorldError::Invalid(format!(
            "The zip's files are larger than {} GiB unpacked.",
            limits.max_bytes >> 30
        ))
    };
    let mut total = 0u64;
    let mut raws = Vec::new();
    for index in 0..archive.len() {
        // The central directory's record only: no local header is read.
        let entry = archive.by_index_data(index).map_err(zip_error(path))?;
        total = total.saturating_add(entry.size());
        if total > limits.max_bytes {
            return Err(too_large());
        }
        let name = entry.name().map_err(zip_error(path))?.into_owned();
        if is_mac_metadata(&name) {
            continue;
        }
        // A link is never written, wherever it points.
        if entry.is_symlink()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            continue;
        }
        if entry.encrypted() {
            return Err(WorldError::Invalid(format!(
                "{name} in the zip is encrypted, which Lodestone can't open."
            )));
        }
        let is_dir = entry.is_dir();
        // Escapes, absolute paths, `..`, `\`, `:`, device names: anywhere in the zip.
        match check_pack_path(&name, is_dir, &WriteAll) {
            Ok(PathCheck::Write(_)) => {}
            Ok(PathCheck::Skip(_)) => continue,
            Err(err) => return Err(unsafe_zip(err)),
        }
        raws.push(Raw {
            index,
            name: name.trim_end_matches('/').to_string(),
            is_dir,
            size: entry.size(),
        });
    }

    let prefix = match shape {
        Shape::OneWorldFolder | Shape::OneFolder => {
            let mut tops = raws.iter().map(|r| r.name.split('/').next().unwrap_or(""));
            let Some(top) = tops.next() else {
                return Err(WorldError::Invalid("That zip is empty.".into()));
            };
            if raws
                .iter()
                .any(|r| r.name.split('/').next() != Some(top) || (r.name == top && !r.is_dir))
            {
                return Err(WorldError::Invalid(
                    "That zip isn't one world: its files aren't all in one folder.".into(),
                ));
            }
            if shape == Shape::OneWorldFolder
                && !raws
                    .iter()
                    .any(|r| !r.is_dir && r.name == format!("{top}/level.dat"))
            {
                return Err(WorldError::Invalid(
                    "That zip isn't a world: its folder has no level.dat.".into(),
                ));
            }
            format!("{top}/")
        }
        Shape::Import => {
            let mut candidates: Vec<String> = Vec::new();
            for raw in raws.iter().filter(|r| !r.is_dir) {
                match raw.name.split('/').collect::<Vec<_>>().as_slice() {
                    ["level.dat"] => candidates.push(String::new()),
                    [folder, "level.dat"] => candidates.push(format!("{folder}/")),
                    _ => {}
                }
            }
            // A world at the root wins over folders inside it (such as datapacks).
            if candidates.iter().any(String::is_empty) {
                String::new()
            } else {
                match candidates.as_slice() {
                    [] => {
                        return Err(WorldError::Invalid(
                            "That zip isn't a world: there is no level.dat at its top or in one folder."
                                .into(),
                        ));
                    }
                    [one] => one.clone(),
                    _ => {
                        return Err(WorldError::Invalid(
                            "That zip has several worlds; import them one at a time.".into(),
                        ));
                    }
                }
            }
        }
    };

    let mut entries = Vec::new();
    let mut canonical = Vec::new();
    for raw in &raws {
        let Some(relative) = raw
            .name
            .strip_prefix(prefix.as_str())
            .filter(|rest| !rest.is_empty())
        else {
            continue;
        };
        let path = world_path(relative).map_err(|why| {
            WorldError::Invalid(format!(
                "The zip is unsafe: {} can't be kept on every disk ({why}).",
                raw.name
            ))
        })?;
        if always_excluded(&path) {
            continue;
        }
        canonical.push(path);
        entries.push(Planned {
            index: raw.index,
            relative: relative.to_string(),
            is_dir: raw.is_dir,
            size: raw.size,
        });
    }

    // Two names one file on a disk that ignores case (folding as such disks do, `ſ` and
    // `s` too), and a file that is also a folder or is listed twice.
    if let Some((first, second)) = case_collisions(canonical.iter()).into_iter().next() {
        let shown = |p: &CanonicalPath| format!("{prefix}{}", &p.as_str()["saves/w/".len()..]);
        return Err(WorldError::Conflict(format!(
            "The zip has both {} and {}, whose names differ only in case. On Windows and \
             macOS one would overwrite the other, so nothing was imported.",
            shown(&first),
            shown(&second)
        )));
    }
    let mut folders: HashSet<&str> = HashSet::new();
    for entry in &entries {
        let rel = entry.relative.as_str();
        let mut rest = rel;
        while let Some((parent, _)) = rest.rsplit_once('/') {
            folders.insert(parent);
            rest = parent;
        }
        if entry.is_dir {
            folders.insert(rel);
        }
    }
    let mut files: HashSet<&str> = HashSet::new();
    for entry in entries.iter().filter(|e| !e.is_dir) {
        if folders.contains(entry.relative.as_str()) || !files.insert(&entry.relative) {
            return Err(WorldError::Conflict(format!(
                "The zip has {prefix}{} more than once (as a file and a folder), so one \
                 would overwrite the other. Nothing was imported.",
                entry.relative
            )));
        }
    }

    let level = entries
        .iter()
        .find(|e| !e.is_dir && e.relative == "level.dat");
    let summary = match (level, shape) {
        (None, Shape::OneFolder) => LevelSummary::default(),
        (None, _) => {
            return Err(WorldError::Invalid(
                "The zip's level.dat is missing.".into(),
            ));
        }
        (Some(level), _) => {
            if level.size > MAX_LEVEL_DAT_BYTES {
                return Err(WorldError::Invalid("Its level.dat is too large.".into()));
            }
            let mut bytes = Vec::new();
            let mut entry = archive.by_index(level.index).map_err(zip_error(path))?;
            (&mut entry)
                .take(MAX_LEVEL_DAT_BYTES)
                .read_to_end(&mut bytes)
                .map_err(|e| WorldError::Invalid(format!("Its level.dat can't be read: {e}")))?;
            drop(entry);
            level::summarize(&bytes)
                .map_err(|e| WorldError::Invalid(format!("Its level.dat can't be read: {e}")))?
        }
    };
    let folder_hint = match prefix.strip_suffix('/') {
        Some(folder) => folder.to_string(),
        None => path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .or_else(|| summary.name.clone())
            .unwrap_or_else(|| "World".into()),
    };
    let declared_bytes = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size)
        .fold(0u64, u64::saturating_add);
    Ok(CheckedZip {
        prefix,
        summary,
        folder_hint,
        declared_bytes,
        entries,
        shape,
    })
}

/// Unpacks the world of the checked zip at `path` into `dest`, a new folder (it must not
/// exist; its parent must). Each file is created new (never over another) and copied up
/// to its stated size plus one byte (one that is larger refuses the zip). Everything is
/// made durable as one batch ([`lodestone_image::store::flush`]): each file and folder
/// handed to the drive as it is written, then one flush of the drive's cache at the end,
/// rather than a flush of the cache per file. Cancel is asked before each entry and every
/// 8 MiB of a large one. On any failure `dest` is removed. Returns the files placed, relative to `dest` with `/`.
pub fn extract_into(
    path: &Path,
    zip: &CheckedZip,
    dest: &Path,
    progress: &Progress,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<String>, WorldError> {
    fs::create_dir(dest).map_err(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => {
            WorldError::Conflict(format!("{} already exists.", dest.display()))
        }
        _ => io_error(dest)(e),
    })?;
    let result = (|| {
        let mut archive = open_archive(
            path,
            &|| File::open(path).map(io::BufReader::new),
            zip.shape,
        )?;
        let mut budget = zip.declared_bytes;
        let mut done = ProgressSnapshot {
            files_total: zip.entries.iter().filter(|e| !e.is_dir).count() as u64,
            bytes_total: zip.declared_bytes,
            ..ProgressSnapshot::default()
        };
        progress.set(done);
        let mut placed = Vec::new();
        let mut buffer = vec![0u8; 1 << 20];
        // Every folder made, flushed once each at the end.
        let mut folders: std::collections::BTreeSet<PathBuf> = [dest.to_path_buf()].into();
        for planned in &zip.entries {
            if cancel() {
                return Err(WorldError::Cancelled);
            }
            let target = super::join_rel(dest, &planned.relative);
            if planned.is_dir {
                fs::create_dir_all(&target).map_err(io_error(&target))?;
                folders.insert(target);
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_error(parent))?;
                folders.insert(parent.to_path_buf());
            }
            let mut out = match File::options().write(true).create_new(true).open(&target) {
                Ok(out) => out,
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(WorldError::Conflict(format!(
                        "Two files in the zip would both be saved as {}, so one would be \
                         lost. Nothing was imported.",
                        planned.relative
                    )));
                }
                Err(err) => return Err(io_error(&target)(err)),
            };
            let mut entry = archive.by_index(planned.index).map_err(zip_error(path))?;
            // Never trust the size in the zip: at most what it says, plus one byte to see
            // a file that is larger.
            let limit = budget.min(planned.size).saturating_add(1);
            let mut source = (&mut entry).take(limit);
            // In large chunks (std's copy moves 8 KiB per call), asking Cancel and moving
            // the bar between them, so a file of many gigabytes can be stopped too.
            let mut copied = 0u64;
            let mut since_check = 0usize;
            loop {
                let read = match source.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        return Err(WorldError::Invalid(format!(
                            "{} can't be unpacked: {e}",
                            planned.relative
                        )));
                    }
                };
                io::Write::write_all(&mut out, &buffer[..read]).map_err(io_error(&target))?;
                copied += read as u64;
                since_check += read;
                if since_check >= COPY_CHUNK {
                    since_check = 0;
                    progress.set(ProgressSnapshot {
                        bytes_done: done.bytes_done + copied,
                        ..done
                    });
                    if cancel() {
                        return Err(WorldError::Cancelled);
                    }
                }
            }
            if copied > planned.size || copied > budget {
                return Err(WorldError::Invalid(format!(
                    "{} is larger than the zip says.",
                    planned.relative
                )));
            }
            lodestone_image::store::flush::write_out(&out).map_err(io_error(&target))?;
            budget -= copied;
            done.files_done += 1;
            done.bytes_done += copied;
            progress.set(done);
            placed.push(planned.relative.clone());
        }
        if !dest.join("level.dat").is_file() {
            return Err(WorldError::Invalid(
                "The zip's level.dat is missing.".into(),
            ));
        }
        // A folder made inside another is listed in it: the deepest first.
        for folder in folders.iter().rev() {
            lodestone_image::store::flush::write_out_dir(folder).map_err(io_error(folder))?;
        }
        if let Some(parent) = dest.parent() {
            lodestone_image::store::flush::write_out_dir(parent).map_err(io_error(parent))?;
        }
        lodestone_image::store::flush::full_flush(dest).map_err(io_error(dest))?;
        Ok(placed)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    const LEVEL_DAT: &[u8] =
        include_bytes!("../../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");

    fn write(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    /// A world folder: `level.dat` and region files.
    fn world(dir: &Path) {
        fs::create_dir_all(dir.join("region")).unwrap();
        fs::create_dir_all(dir.join("DIM-1/region")).unwrap();
        fs::write(dir.join("level.dat"), LEVEL_DAT).unwrap();
        fs::write(dir.join("region/r.0.0.mca"), vec![3u8; 50_000]).unwrap();
        fs::write(dir.join("DIM-1/region/r.0.0.mca"), b"nether").unwrap();
        fs::write(dir.join("session.lock"), b"lock").unwrap();
    }

    fn checked(entries: &[(&str, &[u8])], shape: Shape) -> Result<CheckedZip, WorldError> {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("w.zip");
        write(&path, entries);
        check(&path, shape, Limits::default())
    }

    fn refused(entries: &[(&str, &[u8])], shape: Shape, expected: &str) {
        let err = checked(entries, shape).unwrap_err();
        assert!(err.to_string().contains(expected), "{entries:?}: {err}");
    }

    /// A reader that counts the bytes read through it.
    struct Counting<R> {
        inner: R,
        read: std::rc::Rc<std::cell::Cell<u64>>,
    }

    impl<R: Read> Read for Counting<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.read.set(self.read.get() + n as u64);
            Ok(n)
        }
    }

    impl<R: io::Seek> io::Seek for Counting<R> {
        fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn opening_a_zip_reads_its_end_not_its_files() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        // Region files are stored as they are, so the zip is at least this large.
        fs::write(src.join("region/r.1.0.mca"), vec![7u8; 32 << 20]).unwrap();
        let out = temp.path().join("w.zip");
        write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        assert!(fs::metadata(&out).unwrap().len() > 32 << 20);

        // The whole check, level.dat included, for each shape, counting every byte read
        // through every reader it opens.
        for shape in [Shape::OneWorldFolder, Shape::OneFolder, Shape::Import] {
            let read = std::rc::Rc::new(std::cell::Cell::new(0));
            let open = || {
                File::open(&out).map(|file| Counting {
                    inner: io::BufReader::new(file),
                    read: read.clone(),
                })
            };
            let checked = check_with(&out, &open, shape, Limits::default()).unwrap();
            assert!(
                checked.entries.len() >= 4,
                "{} entries",
                checked.entries.len()
            );
            assert!(read.get() < 1 << 20, "{shape:?}: read {} bytes", read.get());
        }
    }

    /// A damaged zip to import that starts like a zip is refused at once, not scanned from
    /// its first byte looking for an archive further in.
    #[test]
    fn a_damaged_import_is_refused_without_a_scan() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::write(src.join("region/r.1.0.mca"), vec![7u8; 8 << 20]).unwrap();
        let out = temp.path().join("w.zip");
        write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        // Cut off its end records.
        let len = fs::metadata(&out).unwrap().len();
        File::options()
            .write(true)
            .open(&out)
            .unwrap()
            .set_len(len - 4096)
            .unwrap();
        let read = std::rc::Rc::new(std::cell::Cell::new(0));
        let open = || {
            File::open(&out).map(|file| Counting {
                inner: io::BufReader::new(file),
                read: read.clone(),
            })
        };
        assert!(check_with(&out, &open, Shape::Import, Limits::default()).is_err());
        assert!(read.get() < 1 << 20, "read {} bytes", read.get());
    }

    #[test]
    fn only_an_import_may_have_something_before_its_zip() {
        let temp = tempfile::tempdir().unwrap();
        let plain = temp.path().join("plain.zip");
        write(&plain, &[("W/level.dat", LEVEL_DAT)]);
        let stubbed = temp.path().join("stubbed.zip");
        let mut bytes = b"a self-extracting stub".to_vec();
        bytes.extend(fs::read(&plain).unwrap());
        fs::write(&stubbed, bytes).unwrap();

        let imported = check(&stubbed, Shape::Import, Limits::default()).unwrap();
        assert_eq!(imported.folder_hint, "W");
        let unpacked = temp.path().join("unpacked");
        extract_into(
            &stubbed,
            &imported,
            &unpacked,
            &Progress::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(fs::read(unpacked.join("level.dat")).unwrap(), LEVEL_DAT);
        // Lodestone's own zips start at their first byte; one that doesn't was changed.
        assert!(check(&stubbed, Shape::OneWorldFolder, Limits::default()).is_err());
    }

    #[test]
    fn a_found_backup_may_lack_level_dat() {
        let entries: &[(&str, &[u8])] = &[("W/region/r.0.0.mca", b"region")];
        refused(entries, Shape::OneWorldFolder, "no level.dat");
        let found = checked(entries, Shape::OneFolder).unwrap();
        assert_eq!(found.summary, LevelSummary::default());
        assert_eq!(found.files().collect::<Vec<_>>(), ["region/r.0.0.mca"]);
        // Still one folder.
        refused(
            &[("W/a", b"a"), ("X/b", b"b")],
            Shape::OneFolder,
            "aren't all in one folder",
        );
        // A level.dat it has is still read.
        let with = checked(&[("W/level.dat", LEVEL_DAT)], Shape::OneFolder).unwrap();
        assert!(with.summary.name.is_some());
    }

    #[test]
    fn a_world_round_trips_through_a_zip() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        let out = temp.path().join("backups/w.zip");
        let written = write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        assert_eq!(
            written.files,
            ["DIM-1/region/r.0.0.mca", "level.dat", "region/r.0.0.mca"]
        );
        assert_eq!(written.left_out_count, 0, "session.lock isn't counted");
        assert_eq!(written.bytes, fs::metadata(&out).unwrap().len());
        assert_eq!(written.world_bytes, LEVEL_DAT.len() as u64 + 50_006);
        // nothing else beside it
        assert_eq!(fs::read_dir(out.parent().unwrap()).unwrap().count(), 1);

        let zip = check(&out, Shape::OneWorldFolder, Limits::default()).unwrap();
        assert_eq!(zip.prefix, "World/");
        assert_eq!(zip.declared_bytes, written.world_bytes);
        let dest = temp.path().join("Restored");
        let placed = extract_into(&out, &zip, &dest, &Progress::default(), &|| false).unwrap();
        assert_eq!(placed.len(), 3);
        assert_eq!(fs::read(dest.join("level.dat")).unwrap(), LEVEL_DAT);
        assert_eq!(
            fs::read(dest.join("DIM-1/region/r.0.0.mca")).unwrap(),
            b"nether"
        );
        assert!(!dest.join("session.lock").exists());
        // A plain zip any tool opens: one top folder.
        let mut archive = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        assert!(archive.by_name("World/level.dat").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn links_and_names_no_disk_keeps_are_left_out_and_counted() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        std::os::unix::fs::symlink("/etc", src.join("link")).unwrap();
        fs::write(src.join("CON.txt"), b"x").unwrap();
        fs::write(src.join(".lodestone-tmp"), b"x").unwrap();
        let out = temp.path().join("w.zip");
        let written = write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        assert_eq!(written.left_out_count, 2);
        assert_eq!(written.left_out, ["CON.txt", "link"]);
        assert!(!written.files.iter().any(|f| f.contains("lodestone")));
    }

    #[test]
    fn folders_holding_only_names_left_out_stay_out_and_every_zip_passes_the_check() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        // Only a name no disk keeps, nested: both folders stay out of the zip.
        fs::create_dir_all(src.join("odd/deeper")).unwrap();
        fs::write(src.join("odd/deeper/CON.txt"), b"x").unwrap();
        // Two dots in a name, and a zip name longer than `check` takes.
        let long = "d".repeat(250);
        fs::create_dir_all(src.join(format!("data/{long}"))).unwrap();
        fs::write(src.join(format!("data/{long}/{}", "e".repeat(255))), b"x").unwrap();
        fs::write(src.join("data/a..b.dat"), b"x").unwrap();
        // An empty folder, and one holding a name left out beside one taken, stay in.
        fs::create_dir_all(src.join("empty")).unwrap();
        fs::create_dir_all(src.join("mixed")).unwrap();
        fs::write(src.join("mixed/aux"), b"x").unwrap();
        fs::write(src.join("mixed/kept.dat"), b"x").unwrap();
        let out = temp.path().join("w.zip");
        let written = write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        assert_eq!(written.left_out_count, 4, "{:?}", written.left_out);
        let checked = check(&out, Shape::OneWorldFolder, Limits::default()).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        let long_folder = format!("World/data/{long}/");
        for gone in [
            "World/odd/",
            "World/odd/deeper/",
            "World/data/",
            &long_folder,
        ] {
            assert!(!names.iter().any(|n| n == gone), "{gone} in {names:?}");
        }
        for kept in ["World/empty/", "World/mixed/", "World/mixed/kept.dat"] {
            assert!(names.iter().any(|n| n == kept), "{kept} not in {names:?}");
        }
        assert!(checked.files().any(|f| f == "mixed/kept.dat"));
    }

    #[test]
    fn a_world_folder_name_no_restore_takes_is_refused() {
        for folder in ["World", "Odd name (1) é", "World.zip"] {
            check_world_folder(folder).unwrap();
        }
        for folder in [
            "aux", "COM1", "nul.txt", "World.", "World ", "a..b", "LODEST~1",
        ] {
            assert!(check_world_folder(folder).is_err(), "{folder:?}");
        }
    }

    #[test]
    fn a_stopped_zip_leaves_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        let out = temp.path().join("out/w.zip");
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let err = write_zip(&src, "World", &out, &Progress::default(), &|| {
            if asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 2 {
                Err(WorldError::Cancelled)
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(matches!(err, WorldError::Cancelled));
        assert_eq!(fs::read_dir(temp.path().join("out")).unwrap().count(), 0);
    }

    #[test]
    fn hostile_names_refuse_the_whole_zip() {
        let w = Shape::OneWorldFolder;
        let level: (&str, &[u8]) = ("W/level.dat", LEVEL_DAT);
        refused(&[level, ("W/../x", b"x")], w, "unsafe");
        refused(&[level, ("/abs", b"x")], w, "unsafe");
        refused(&[level, ("W/C:x", b"x")], w, "unsafe");
        refused(&[level, ("W/a\\b", b"x")], w, "unsafe");
        refused(&[level, ("W/CON.txt", b"x")], w, "unsafe");
        refused(&[level, ("W/LEVEL~1.DAT", b"x")], w, "unsafe");
        refused(&[level, ("W/a\u{200C}b", b"x")], w, "unsafe");
        refused(
            &[
                level,
                ("W/Region/r.0.0.mca", b"a"),
                ("W/region/r.0.1.mca", b"b"),
            ],
            w,
            "differ only in case",
        );
        // a long s folds to s on a disk that ignores case
        refused(
            &[
                level,
                ("W/data/ſcore.dat", b"a"),
                ("W/data/score.dat", b"b"),
            ],
            w,
            "differ only in case",
        );
        refused(
            &[level, ("W/stats", b"a"), ("W/stats/p.json", b"b")],
            w,
            "more than once",
        );
        refused(&[level, ("V/level.dat", LEVEL_DAT)], w, "one folder");
        refused(&[level, ("readme.txt", b"x")], w, "one folder");
        refused(&[("W/region/r.0.0.mca", b"x")], w, "no level.dat");
        refused(&[("W/level.dat", b"not nbt")], w, "can't be read");
    }

    #[test]
    fn links_and_mac_files_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("w.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("W/level.dat", options).unwrap();
        zip.write_all(LEVEL_DAT).unwrap();
        zip.add_symlink("W/region", "../../../..", options).unwrap();
        zip.start_file("W/session.lock", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.start_file("__MACOSX/W/._level.dat", options).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
        let checked = check(&path, Shape::OneWorldFolder, Limits::default()).unwrap();
        assert_eq!(checked.files().collect::<Vec<_>>(), ["level.dat"]);
    }

    #[test]
    fn too_many_entries_or_bytes_refuse_the_zip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("w.zip");
        let mut entries: Vec<(String, Vec<u8>)> = vec![("W/level.dat".into(), LEVEL_DAT.to_vec())];
        entries.extend((0..20).map(|i| (format!("W/data/{i}.dat"), vec![0u8; 1000])));
        let borrowed: Vec<(&str, &[u8])> = entries
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        write(&path, &borrowed);
        let few = Limits {
            max_entries: 10,
            ..Limits::default()
        };
        let err = check(&path, Shape::OneWorldFolder, few).unwrap_err();
        assert!(err.to_string().contains("more than 10 files"), "{err}");
        // A zip bomb: small packed, large unpacked.
        let small = Limits {
            max_bytes: 10_000,
            ..Limits::default()
        };
        let err = check(&path, Shape::OneWorldFolder, small).unwrap_err();
        assert!(err.to_string().contains("larger than"), "{err}");
        assert!(check(&path, Shape::OneWorldFolder, Limits::default()).is_ok());
    }

    /// A plan is within limits exactly when the zip written from it passes the check with
    /// them: a world backup refused for its size is never one a restore would take, and
    /// one made is never refused by its restore.
    #[test]
    fn a_plan_within_limits_is_a_zip_the_check_takes() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::create_dir_all(src.join("data/empty")).unwrap();
        fs::write(src.join("data/a.dat"), vec![1u8; 3000]).unwrap();
        let planned = plan(&src, "World").unwrap();
        let out = temp.path().join("w.zip");
        write_planned(
            &src,
            "World",
            planned.clone(),
            &out,
            &Progress::default(),
            &|| Ok(()),
        )
        .unwrap();
        let entries = zip::ZipArchive::new(File::open(&out).unwrap())
            .unwrap()
            .len();
        let bytes = planned.bytes();
        let exact = Limits {
            max_entries: entries,
            max_bytes: bytes,
        };
        let one_entry_short = Limits {
            max_entries: entries - 1,
            ..exact
        };
        let one_byte_short = Limits {
            max_bytes: bytes - 1,
            ..exact
        };
        assert_eq!(planned.within(exact), Ok(()));
        check(&out, Shape::OneWorldFolder, exact).unwrap();
        let err = planned.within(one_entry_short).unwrap_err();
        assert!(err.contains("files and folders"), "{err}");
        assert!(check(&out, Shape::OneWorldFolder, one_entry_short).is_err());
        let err = planned.within(one_byte_short).unwrap_err();
        assert!(err.contains("GiB of files"), "{err}");
        assert!(check(&out, Shape::OneWorldFolder, one_byte_short).is_err());
    }

    /// Rewrites the stated unpacked size of the entry `name` in both its headers.
    fn shrink_stated_size(path: &Path, name: &str, size: u32) {
        let mut bytes = fs::read(path).unwrap();
        let mut patched = 0;
        let mut at = 0;
        while at + 46 <= bytes.len() {
            let sig = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let (name_len_at, name_at, size_at) = match sig {
                0x04034b50 => (at + 26, at + 30, at + 22),
                0x02014b50 => (at + 28, at + 46, at + 24),
                _ => {
                    at += 1;
                    continue;
                }
            };
            let len = u16::from_le_bytes(bytes[name_len_at..name_len_at + 2].try_into().unwrap())
                as usize;
            if bytes.get(name_at..name_at + len) == Some(name.as_bytes()) {
                bytes[size_at..size_at + 4].copy_from_slice(&size.to_le_bytes());
                patched += 1;
            }
            at += 4;
        }
        assert_eq!(patched, 2);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn an_entry_larger_than_it_says_refuses_the_zip_and_leaves_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("w.zip");
        let big = vec![b'a'; 100_000];
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("W/level.dat", options).unwrap();
        zip.write_all(LEVEL_DAT).unwrap();
        zip.start_file("W/region/r.0.0.mca", options).unwrap();
        zip.write_all(&big).unwrap();
        zip.finish().unwrap();
        shrink_stated_size(&path, "W/region/r.0.0.mca", 10);
        let checked = check(&path, Shape::OneWorldFolder, Limits::default()).unwrap();
        let dest = temp.path().join("out");
        let err =
            extract_into(&path, &checked, &dest, &Progress::default(), &|| false).unwrap_err();
        assert!(matches!(err, WorldError::Invalid(_)), "{err:?}");
        assert!(!dest.exists());
    }

    #[test]
    fn a_cancelled_unpack_leaves_nothing_and_never_writes_over_a_folder() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("w.zip");
        write(&path, &[("W/level.dat", LEVEL_DAT), ("W/a", b"a")]);
        let checked = check(&path, Shape::OneWorldFolder, Limits::default()).unwrap();
        let dest = temp.path().join("out");
        let err = extract_into(&path, &checked, &dest, &Progress::default(), &|| true).unwrap_err();
        assert!(matches!(err, WorldError::Cancelled));
        assert!(!dest.exists());
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("mine"), b"keep").unwrap();
        let err =
            extract_into(&path, &checked, &dest, &Progress::default(), &|| false).unwrap_err();
        assert!(matches!(err, WorldError::Conflict(_)));
        assert_eq!(fs::read(dest.join("mine")).unwrap(), b"keep");
    }

    /// Region files, gzip NBT and mods' databases are stored as they are; text is deflated.
    #[test]
    fn already_compressed_files_are_stored() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::write(src.join("stats.json"), "{}".repeat(1000)).unwrap();
        fs::create_dir_all(src.join("data")).unwrap();
        fs::write(src.join("data/DistantHorizons.sqlite"), vec![9u8; 4096]).unwrap();
        let out = temp.path().join("w.zip");
        write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let method = |archive: &mut zip::ZipArchive<File>, name: &str| {
            archive.by_name(name).unwrap().compression()
        };
        assert_eq!(
            method(&mut archive, "World/region/r.0.0.mca"),
            zip::CompressionMethod::Stored
        );
        assert_eq!(
            method(&mut archive, "World/level.dat"),
            zip::CompressionMethod::Stored
        );
        assert_eq!(
            method(&mut archive, "World/data/DistantHorizons.sqlite"),
            zip::CompressionMethod::Stored
        );
        assert_eq!(
            method(&mut archive, "World/stats.json"),
            zip::CompressionMethod::Deflated
        );
    }

    /// A large file is stored whatever its name: it's never text, and deflating it is
    /// the slowest part of a backup.
    #[test]
    fn large_files_are_stored_whatever_their_name() {
        assert!(!stored_as_is("data/mod.bin", STORE_FROM - 1));
        assert!(stored_as_is("data/mod.bin", STORE_FROM));
        assert!(stored_as_is("DIM-1/data/DistantHorizons.SQLITE", 10));
        assert!(stored_as_is("data/DistantHorizons.sqlite-wal", 10));
        assert!(!stored_as_is("stats/abc.json", 10));
    }

    /// Cancel is asked between chunks of a large file, not only between files.
    #[test]
    fn a_large_file_can_be_stopped_part_way() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::write(src.join("region/r.9.9.mca"), vec![1u8; 3 * COPY_CHUNK]).unwrap();
        let out = temp.path().join("out/w.zip");
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let entries = plan(&src, "World").unwrap().entries.len();
        let err = write_zip(&src, "World", &out, &Progress::default(), &|| {
            // Every entry once, and then some: stopped inside the large file.
            if asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > entries {
                Err(WorldError::Cancelled)
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(matches!(err, WorldError::Cancelled));
        assert_eq!(fs::read_dir(temp.path().join("out")).unwrap().count(), 0);
    }

    /// The folders a zip lists, empty ones too, are given to the restore.
    #[test]
    fn a_checked_zip_lists_its_folders() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::create_dir_all(src.join("datapacks")).unwrap();
        let out = temp.path().join("w.zip");
        write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        let checked = check(&out, Shape::OneWorldFolder, Limits::default()).unwrap();
        let mut folders: Vec<&str> = checked.folders().collect();
        folders.sort();
        assert_eq!(folders, ["DIM-1", "DIM-1/region", "datapacks", "region"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_swapped_for_a_link_while_zipping_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        let planned = plan(&src, "World").unwrap();
        let secret = temp.path().join("secret");
        fs::write(&secret, b"secret").unwrap();
        fs::remove_file(src.join("region/r.0.0.mca")).unwrap();
        std::os::unix::fs::symlink(&secret, src.join("region/r.0.0.mca")).unwrap();
        let out = temp.path().join("w.zip");
        assert!(
            write_planned(&src, "World", planned, &out, &Progress::default(), &|| Ok(
                ()
            ))
            .is_err()
        );
        assert!(!out.exists());
    }

    /// Unpacking asks Cancel inside a large file too, and moves the bar while it copies.
    #[test]
    fn a_large_file_can_be_stopped_part_way_through_unpacking() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::write(src.join("region/r.9.9.mca"), vec![1u8; 3 * COPY_CHUNK]).unwrap();
        let out = temp.path().join("w.zip");
        write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        let checked = check(&out, Shape::OneWorldFolder, Limits::default()).unwrap();
        let entries = checked.entries.len();
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let progress = Progress::default();
        let dest = temp.path().join("out");
        let err = extract_into(&out, &checked, &dest, &progress, &|| {
            // Every entry once, and then some: stopped inside the large file.
            asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > entries
        })
        .unwrap_err();
        assert!(matches!(err, WorldError::Cancelled), "{err:?}");
        assert!(!dest.exists());
        assert!(progress.snapshot().bytes_done >= COPY_CHUNK as u64);
        // Not stopped, it unpacks whole.
        extract_into(&out, &checked, &dest, &Progress::default(), &|| false).unwrap();
        assert_eq!(
            fs::read(dest.join("region/r.9.9.mca")).unwrap().len(),
            3 * COPY_CHUNK
        );
    }

    /// On a disk that keeps letter case apart a world can hold two names one other disks
    /// take for one: the later is left out (and counted), so the zip passes the check.
    #[test]
    fn names_differing_only_in_case_keep_the_first() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("World");
        world(&src);
        fs::write(src.join("a.txt"), b"lower").unwrap();
        fs::write(src.join("A.txt"), b"upper").unwrap();
        fs::create_dir_all(src.join("Data")).unwrap();
        fs::write(src.join("Data/x"), b"x").unwrap();
        fs::create_dir_all(src.join("data")).unwrap();
        fs::write(src.join("data/y"), b"y").unwrap();
        let case_sensitive = fs::read_dir(&src).unwrap().count() == 7;
        let out = temp.path().join("w.zip");
        let written = write_zip(&src, "World", &out, &Progress::default(), &|| Ok(())).unwrap();
        check(&out, Shape::OneWorldFolder, Limits::default()).unwrap();
        if case_sensitive {
            assert_eq!(written.left_out, ["a.txt", "data"]);
            assert_eq!(written.left_out_count, 2);
            assert!(written.files.contains(&"A.txt".to_string()));
            assert!(written.files.contains(&"Data/x".to_string()));
        } else {
            assert_eq!(written.left_out_count, 0);
        }
    }
}
