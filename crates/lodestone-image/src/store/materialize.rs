//! Materializing an image: placing its files (or some of them) into a staging folder, with
//! their modes, checking every byte against its SHA-256 on the way.
//!
//! A placed file gets its entry's permission bits plus the owner's write bit, as both the
//! core's and the Launcher's restores always did: the game, its mods and Lodestone must be
//! able to change and replace what a restore puts back, and a file recorded read-only
//! (on Windows, say) would otherwise come back read-only and block updates.
//!
//! The staging folder is the first step of a restore or an install: the caller swaps it
//! into place afterwards (the core's journaled swap), so a failure here leaves the instance
//! untouched. Every file is created new (never opened if it exists), so two entries can't
//! write one file, and a case-insensitive disk is detected first and refuses entries whose
//! names differ only in letter case rather than merging them.
//!
//! The staging folder must be on the same volume as the folder it will replace (beside
//! it, say): the swap is a rename, which only works within a volume, and the case check
//! looks at the staging folder's disk, which is then the instance's. A staging folder on a
//! case-sensitive volume would let names through that clash once swapped into a
//! case-insensitive one.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read};
use std::path::{Path, PathBuf};

use crate::sha256::Sha256;

use crate::layout::{join_disk, DiskLayout};
use crate::manifest::{Entry, Image};
use crate::paths::{case_collisions_of, CanonicalPath, PathError};

use super::encoding::buffer_len;
use super::{fsx, Cancel, Encoding, ObjectForm, Store, StoreError, NEVER};

/// Which of an image's entries to place.
#[derive(Clone, Copy)]
pub enum Selection<'a> {
    All,
    /// Everything but world files.
    Content,
    /// The files of these worlds.
    Worlds(&'a [String]),
    /// The entries for which this is true.
    Matching(&'a (dyn Fn(&Entry) -> bool + Sync)),
}

impl Selection<'_> {
    pub fn includes(&self, entry: &Entry) -> bool {
        match self {
            Selection::All => true,
            Selection::Content => entry.world.is_none(),
            Selection::Worlds(worlds) => entry
                .world
                .as_ref()
                .is_some_and(|world| worlds.contains(world)),
            Selection::Matching(matches) => matches(entry),
        }
    }
}

/// How to materialize.
#[derive(Clone, Copy)]
pub struct MaterializeOptions<'a> {
    pub selection: Selection<'a>,
    /// Flush every file and folder to disk before returning (for a staging folder that is
    /// swapped in next). Off for throwaway copies. Flushed as one batch, as the store's
    /// writes are (`fsx::write_out`): each file and folder handed to the drive, then one
    /// flush of the drive's cache at the end (one `F_FULLFSYNC` on Apple platforms, one
    /// `syncfs` on Linux, one `FlushFileBuffers` on Windows), never a device flush per
    /// file, which made placing 3,100 files take 20 s instead of 4.5 s on a Mac.
    pub durable: bool,
    pub cancel: Cancel<'a>,
}

impl Default for MaterializeOptions<'_> {
    fn default() -> Self {
        MaterializeOptions {
            selection: Selection::All,
            durable: true,
            cancel: NEVER,
        }
    }
}

/// Why materializing failed. What was placed so far stays in the staging folder for the
/// caller to delete.
#[derive(Debug, thiserror::Error)]
pub enum MaterializeError {
    /// The store lacks these objects: download or import them first
    /// ([`missing_objects`]). Nothing was placed.
    #[error("the store lacks {} of the image's files, such as {}", .0.len(), .0.first().map(|p| p.as_str()).unwrap_or_default())]
    Missing(Vec<CanonicalPath>),
    #[error("{path} can't be placed on this side: {error}")]
    Layout {
        path: CanonicalPath,
        error: PathError,
    },
    /// These two would be one file on this case-insensitive disk. Nothing was placed.
    #[error("{0} and {1} differ only in letter case, which this disk ignores")]
    CaseClash(CanonicalPath, CanonicalPath),
    #[error("{0} is already in the staging folder")]
    Exists(PathBuf),
    #[error("the staging folder {0} is missing")]
    NoStaging(PathBuf),
    /// The stored bytes of this file are damaged (they don't match its SHA-256).
    #[error("the stored copy of {path} is damaged")]
    Damaged { path: CanonicalPath },
    /// The image gives this file a size its stored copy, which is healthy, doesn't have:
    /// the image is wrong, not the store.
    #[error("the image gives {path} a size its stored copy doesn't have")]
    WrongSize { path: CanonicalPath },
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Store(StoreError),
    #[error("cancelled")]
    Cancelled,
}

/// What was placed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Materialized {
    pub files: usize,
    pub bytes: u64,
    /// Selected entries this side never holds (client-only files on a server, say), left
    /// out.
    pub not_placed: Vec<CanonicalPath>,
}

/// The selected entries whose objects the store lacks, or holds incompletely (a chunk
/// list missing a chunk): what to download or import before materializing.
pub fn missing_objects<'i>(
    store: &Store,
    image: &'i Image,
    selection: Selection,
) -> Vec<&'i Entry> {
    image
        .entries
        .iter()
        .filter(|e| selection.includes(e) && !store.object_complete(&e.sha256, e.size))
        .collect()
}

/// Places the selected entries of `image` into `staging` (an existing folder, best empty,
/// on the volume of the folder it will replace: see the module) as `layout` lays them out.
pub fn materialize(
    store: &Store,
    image: &Image,
    layout: &DiskLayout,
    staging: &Path,
    options: &MaterializeOptions,
) -> Result<Materialized, MaterializeError> {
    if !fs::metadata(staging).is_ok_and(|m| m.is_dir()) {
        return Err(MaterializeError::NoStaging(staging.to_path_buf()));
    }
    let mut report = Materialized::default();
    let mut placed: Vec<(&Entry, String)> = Vec::new();
    for entry in image
        .entries
        .iter()
        .filter(|e| options.selection.includes(e))
    {
        match layout.to_disk(&entry.path) {
            Ok(disk) => placed.push((entry, disk)),
            Err(PathError::WrongSide(_)) => report.not_placed.push(entry.path.clone()),
            Err(error) => {
                return Err(MaterializeError::Layout {
                    path: entry.path.clone(),
                    error,
                })
            }
        }
    }
    let missing: Vec<CanonicalPath> = placed
        .iter()
        .filter(|(e, _)| !store.object_complete(&e.sha256, e.size))
        .map(|(e, _)| e.path.clone())
        .collect();
    if !missing.is_empty() {
        return Err(MaterializeError::Missing(missing));
    }
    if ignores_case(staging)? {
        // On the disk paths themselves (a client's are under `minecraft/`, and can be
        // longer than a canonical path), every one of them; reported by the entries.
        let clash = case_collisions_of(placed.iter().map(|(_, disk)| disk.as_str()))
            .into_iter()
            .next();
        if let Some((a, b)) = clash {
            return Err(MaterializeError::CaseClash(
                placed[a].0.path.clone(),
                placed[b].0.path.clone(),
            ));
        }
    }

    let mut folders: BTreeSet<PathBuf> = BTreeSet::new();
    for (entry, disk) in placed {
        if (options.cancel)() {
            return Err(MaterializeError::Cancelled);
        }
        let target = join_disk(staging, &disk);
        if let Some(parent) = target.parent() {
            if folders.insert(parent.to_path_buf()) {
                fs::create_dir_all(parent).map_err(io_err(parent))?;
                // Every folder it made, for the flush at the end.
                for above in parent.ancestors().skip(1) {
                    if above == staging || !above.starts_with(staging) {
                        break;
                    }
                    folders.insert(above.to_path_buf());
                }
            }
        }
        place(store, entry, &target, options)?;
        report.files += 1;
        report.bytes = report.bytes.saturating_add(entry.size);
    }
    if options.durable {
        // Deepest first, so each folder's entry in its parent is written after it.
        for folder in folders.iter().rev() {
            fsx::write_out_dir(folder).map_err(io_err(folder))?;
        }
        fsx::write_out_dir(staging).map_err(io_err(staging))?;
        fsx::full_flush(staging).map_err(io_err(staging))?;
    }
    Ok(report)
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> MaterializeError + '_ {
    move |source| MaterializeError::Io {
        what: format!("Failed to write {}", path.display()),
        source,
    }
}

/// Places one file at `target`, which must not exist, then sets its time and mode. On an
/// error the file is deleted.
fn place(
    store: &Store,
    entry: &Entry,
    target: &Path,
    options: &MaterializeOptions,
) -> Result<(), MaterializeError> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(MaterializeError::Exists(target.to_path_buf()));
    }
    let mut guard = fsx::TempGuard::new(target.to_path_buf());
    let damaged = || MaterializeError::Damaged {
        path: entry.path.clone(),
    };
    let wrong_size = || MaterializeError::WrongSize {
        path: entry.path.clone(),
    };
    let form = store
        .object_form(&entry.sha256, entry.size)
        .map_err(|e| match e {
            StoreError::Damaged(_) => damaged(),
            StoreError::WrongSize { .. } => wrong_size(),
            other => MaterializeError::Store(other),
        })?;
    let cloned = cfg!(target_os = "macos")
        && store.encoding() == Encoding::Raw
        && form == ObjectForm::Whole
        && clone_checked(store, entry, target, options)?;
    if !cloned {
        let _ = fsx::remove_file_forced(target);
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::AlreadyExists {
                    MaterializeError::Exists(target.to_path_buf())
                } else {
                    io_err(target)(e)
                }
            })?;
        let mut writer = BufWriter::with_capacity(buffer_len(entry.size), file);
        store
            .read_object(&entry.sha256, entry.size, &mut writer, options.cancel)
            .map_err(|e| match e {
                StoreError::Damaged(_) | StoreError::Missing(_) => damaged(),
                StoreError::WrongSize { .. } => wrong_size(),
                StoreError::Cancelled => MaterializeError::Cancelled,
                other => MaterializeError::Store(other),
            })?;
        let file = writer
            .into_inner()
            .map_err(|e| io_err(target)(e.into_error()))?;
        finish_file(file, entry, options)?;
    }
    fsx::set_mode(target, placed_mode(entry.mode)).map_err(io_err(target))?;
    guard.disarm();
    Ok(())
}

/// The bits a file is placed with: the entry's, and always the owner's write bit (see the
/// module).
pub fn placed_mode(mode: u32) -> u32 {
    (mode & 0o777) | 0o200
}

/// Sets the file's time (when the entry has one) and flushes it.
fn finish_file(
    file: File,
    entry: &Entry,
    options: &MaterializeOptions,
) -> Result<(), MaterializeError> {
    if let Some(time) = entry.modified.and_then(fsx::from_unix_ns) {
        // Informational: a file system that can't keep the time still gets the file.
        let _ = file.set_modified(time);
    }
    if options.durable {
        // Durable with the batch's flush at the end (`materialize`).
        fsx::write_out(&file).map_err(|source| MaterializeError::Io {
            what: format!("Failed to flush {}", entry.path),
            source,
        })?;
    }
    Ok(())
}

/// Clones a raw whole object to `target` (APFS shares the blocks) and checks the clone's
/// bytes. False when cloning didn't work; the caller copies instead.
fn clone_checked(
    store: &Store,
    entry: &Entry,
    target: &Path,
    options: &MaterializeOptions,
) -> Result<bool, MaterializeError> {
    let Ok(object) = store.object_path(&entry.sha256) else {
        return Ok(false);
    };
    if fsx::clone_or_copy(&object, target).is_err() {
        return Ok(false);
    }
    // The clone is read-only like the object; it gets the entry's mode at the end.
    fsx::set_mode(target, 0o644).map_err(io_err(target))?;
    let mut file = File::options()
        .read(true)
        .write(true)
        .open(target)
        .map_err(io_err(target))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; buffer_len(entry.size)];
    let mut total = 0u64;
    loop {
        let n = match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_err(target)(e)),
        };
        total += n as u64;
        hasher.update(&buffer[..n]);
    }
    if total != entry.size || super::hex_digest(hasher) != entry.sha256 {
        return Err(MaterializeError::Damaged {
            path: entry.path.clone(),
        });
    }
    finish_file(file, entry, options)?;
    Ok(true)
}

/// Whether the disk under `dir` ignores letter case: a probe file's name in upper case
/// finds it.
fn ignores_case(dir: &Path) -> Result<bool, MaterializeError> {
    let name = format!(".lodestone-case-probe-{}", fsx::random_suffix());
    let probe = dir.join(&name);
    File::options()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(io_err(&probe))?;
    let found = fs::symlink_metadata(dir.join(name.to_uppercase())).is_ok();
    let _ = fs::remove_file(&probe);
    Ok(found)
}

#[cfg(test)]
mod tests;
