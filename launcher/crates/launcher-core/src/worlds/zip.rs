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
//! [`check`] reads only the zip's central directory, the list every later step uses (a
//! local header naming another path is never read), and refuses the whole zip for any
//! name it can't take, rather than leaving a file out without a word. [`extract_into`]
//! then never trusts a size the zip states: each file is copied up to its stated size
//! plus one byte, and one that is larger refuses the zip.

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
}

impl CheckedZip {
    /// The files it places, relative to the world folder.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter(|e| !e.is_dir)
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

/// The files and folders under `dir` a zip takes, relative with `/`, and the names it
/// leaves out. Links are never followed. A folder whose name is left out is left out
/// whole.
struct Collected {
    entries: Vec<(String, PathBuf, bool)>,
    left_out: Vec<String>,
    left_out_count: u64,
}

fn collect(dir: &Path) -> Result<Collected, WorldError> {
    let mut found = Collected {
        entries: Vec::new(),
        left_out: Vec::new(),
        left_out_count: 0,
    };
    fn leave_out(found: &mut Collected, name: String) {
        found.left_out_count += 1;
        if found.left_out.len() < MAX_LEFT_OUT_NAMES {
            found.left_out.push(name);
        }
    }
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
                leave_out(&mut found, child);
                continue;
            }
            let kind = entry.file_type().map_err(io_error(&entry.path()))?;
            let canonical = match world_path(&child) {
                Ok(canonical) => canonical,
                Err(_) => {
                    leave_out(&mut found, child);
                    continue;
                }
            };
            if always_excluded(&canonical) {
                continue;
            }
            if kind.is_dir() {
                found.entries.push((child.clone(), entry.path(), true));
                stack.push((entry.path(), child));
            } else if kind.is_file() {
                found.entries.push((child, entry.path(), false));
            } else {
                leave_out(&mut found, child);
            }
        }
    }
    found.entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

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
    let collected = collect(dir)?;
    let entries = &collected.entries;
    let total_bytes: u64 = entries
        .iter()
        .filter(|(_, _, is_dir)| !is_dir)
        .filter_map(|(_, path, _)| fs::symlink_metadata(path).ok())
        .map(|m| m.len())
        .sum();
    let total_files = entries.iter().filter(|(_, _, d)| !d).count() as u64;
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
        let base = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let zip_err = |err: zip::result::ZipError| {
            WorldError::Invalid(format!("couldn't write the zip: {err}"))
        };
        zip.add_directory(
            format!("{prefix}/"),
            base.last_modified_time(zip_time(fs::metadata(dir).and_then(|m| m.modified()).ok())),
        )
        .map_err(zip_err)?;
        for (relative, path, is_dir) in entries {
            stop()?;
            let metadata = fs::symlink_metadata(path).map_err(io_error(path))?;
            let options = base
                .last_modified_time(zip_time(metadata.modified().ok()))
                .large_file(metadata.len() >= u32::MAX as u64);
            if *is_dir {
                zip.add_directory(format!("{prefix}/{relative}/"), options)
                    .map_err(zip_err)?;
                continue;
            }
            zip.start_file(format!("{prefix}/{relative}"), options)
                .map_err(zip_err)?;
            let mut source =
                io::BufReader::with_capacity(256 * 1024, File::open(path).map_err(io_error(path))?);
            let copied = io::copy(&mut source, &mut zip).map_err(io_error(path))?;
            files.push(relative.clone());
            done.files_done += 1;
            done.bytes_done += copied;
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
            left_out: collected.left_out,
            left_out_count: collected.left_out_count,
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

/// Opens the world zip at `path` and checks every entry (see the module): its size and
/// entry count against `limits`, encryption, each name, two names one file on a disk that
/// ignores case, a file that is also a folder, and the world's layout for `shape`. Reads
/// `level.dat`, nothing else.
pub fn check(path: &Path, shape: Shape, limits: Limits) -> Result<CheckedZip, WorldError> {
    let file = File::open(path).map_err(io_error(path))?;
    let mut archive = zip::ZipArchive::new(io::BufReader::new(file)).map_err(zip_error(path))?;
    if archive.len() > limits.max_entries {
        return Err(WorldError::Invalid(format!(
            "The zip has more than {} files.",
            limits.max_entries
        )));
    }
    let too_large = || {
        WorldError::Invalid(format!(
            "The zip's files are larger than {} GB unpacked.",
            limits.max_bytes / 1_000_000_000
        ))
    };
    let mut total = 0u64;
    let mut raws = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(zip_error(path))?;
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
        Shape::OneWorldFolder => {
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
            if !raws
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
        .find(|e| !e.is_dir && e.relative == "level.dat")
        .ok_or_else(|| WorldError::Invalid("The zip's level.dat is missing.".into()))?;
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
    let summary = level::summarize(&bytes)
        .map_err(|e| WorldError::Invalid(format!("Its level.dat can't be read: {e}")))?;
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
    })
}

/// Unpacks the world of the checked zip at `path` into `dest`, a new folder (it must not
/// exist; its parent must). Each file is created new (never over another), copied up to
/// its stated size plus one byte (one that is larger refuses the zip), and flushed. Cancel
/// is asked before each entry. On any failure `dest` is removed. Returns the files placed,
/// relative to `dest` with `/`.
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
        let file = File::open(path).map_err(io_error(path))?;
        let mut archive =
            zip::ZipArchive::new(io::BufReader::new(file)).map_err(zip_error(path))?;
        let mut budget = zip.declared_bytes;
        let mut done = ProgressSnapshot {
            files_total: zip.entries.iter().filter(|e| !e.is_dir).count() as u64,
            bytes_total: zip.declared_bytes,
            ..ProgressSnapshot::default()
        };
        progress.set(done);
        let mut placed = Vec::new();
        for planned in &zip.entries {
            if cancel() {
                return Err(WorldError::Cancelled);
            }
            let target = super::join_rel(dest, &planned.relative);
            if planned.is_dir {
                fs::create_dir_all(&target).map_err(io_error(&target))?;
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_error(parent))?;
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
            let copied = io::copy(&mut (&mut entry).take(limit), &mut out).map_err(|e| {
                WorldError::Invalid(format!("{} can't be unpacked: {e}", planned.relative))
            })?;
            if copied > planned.size || copied > budget {
                return Err(WorldError::Invalid(format!(
                    "{} is larger than the zip says.",
                    planned.relative
                )));
            }
            out.sync_all().map_err(io_error(&target))?;
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
}
