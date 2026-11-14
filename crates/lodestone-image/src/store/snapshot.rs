//! Snapshotting a folder: walking a client instance or a server folder, storing its files
//! and listing them as an image's entries.
//!
//! The walk maps every file to its canonical path with the side's [`DiskLayout`], so a
//! server's worlds come out as `saves/<world>/…` and a client's game folder as the image's
//! root. What is never recorded ([`crate::paths::always_excluded`], and on a client
//! everything outside its game folder) and what the image's excludes leave out is skipped
//! without a word. Files that should be in the image but can't be are listed as withheld,
//! with why:
//!
//! - `symlink`: links and junctions, never followed (the root itself may be one);
//! - `unreadable`: folders that can't be listed, files that can't be opened or read, and
//!   things that aren't files (fifos, sockets, devices: opening some of them blocks);
//! - `bad_name`: names that aren't valid canonical paths, files the layout can't read back
//!   as themselves, and files never placed on this side;
//! - `kept_changing`: files that changed while being read, every try.
//!
//! A withheld path is the canonical path where the layout gives one, otherwise the path as
//! found relative to the game folder; it is for showing, never placing.
//!
//! Snapshotting is two steps so a caller can check free space between them: [`plan`] walks
//! the folder and takes what it can from the stat cache, and [`run`] hashes and stores the
//! rest on several threads.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::layout::{DiskLayout, CLIENT_GAME_FOLDER};
use crate::manifest::{Entry, Image, Source, Withheld, WithheldReason, World};
use crate::paths::{validate_component, CanonicalPath};

use super::excludes::Excludes;
use super::fsx;
use super::statcache::StatCache;
use super::{Cancel, Store, StoreError, NEVER};

/// How a snapshot runs.
#[derive(Clone, Copy)]
pub struct SnapshotOptions<'a> {
    /// The image's exclude patterns ([`super::excludes`]).
    pub excludes: &'a [String],
    /// The last snapshot's stat cache; `None` hashes every file (a full scan).
    pub cache: Option<&'a StatCache>,
    /// Threads hashing and storing files; 0 is the default, half the processors and at
    /// most [`DEFAULT_MAX_THREADS`]. A snapshot of a running server should ask for few
    /// (one or two), so the game keeps its processors.
    pub threads: usize,
    /// How many times a file that changes while being read is read; then it is withheld
    /// as `kept_changing`.
    pub tries: usize,
    pub cancel: Cancel<'a>,
}

/// The most threads a snapshot uses when [`SnapshotOptions::threads`] is 0.
pub const DEFAULT_MAX_THREADS: usize = 4;

impl Default for SnapshotOptions<'_> {
    fn default() -> Self {
        SnapshotOptions {
            excludes: &[],
            cache: None,
            threads: 0,
            tries: 3,
            cancel: NEVER,
        }
    }
}

/// Why a snapshot failed. Files it couldn't take don't fail it; they are withheld.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("the folder {0} is missing")]
    RootMissing(PathBuf),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("cancelled")]
    Cancelled,
}

/// A file the walk found to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// Relative to the walked folder, `/`-separated: the stat cache's key.
    pub disk: String,
    pub path: CanonicalPath,
    pub size: u64,
    pub modified_ns: i64,
    pub mode: u32,
}

/// What [`plan`] found: files whose hash the stat cache gave, files to read, and files
/// withheld.
#[derive(Debug, Clone)]
pub struct Plan {
    pub root: PathBuf,
    pub scan_started_ns: i64,
    /// With their cached hashes.
    pub cached: Vec<(FoundFile, String)>,
    pub to_read: Vec<FoundFile>,
    pub withheld: Vec<Withheld>,
}

impl Plan {
    /// The bytes [`run`] will read: at most what it can add to the store.
    pub fn bytes_to_read(&self) -> u64 {
        self.to_read
            .iter()
            .map(|f| f.size)
            .fold(0, u64::saturating_add)
    }
}

/// A finished snapshot: entries (all `source: object`, sorted by path), withheld files, and
/// the stat cache to save for the next one.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub withheld: Vec<Withheld>,
    pub cache: StatCache,
    /// The worlds the entries are in.
    pub worlds: BTreeSet<String>,
    pub files_read: usize,
    pub bytes_read: u64,
    /// Bytes added to the store (encoded).
    pub bytes_added: u64,
}

impl Snapshot {
    /// Puts the entries and withheld files into `image`, and adds its worlds that `image`
    /// doesn't list yet (by name only). The caller seals it after its own changes.
    pub fn fill(&self, image: &mut Image) {
        image.entries = self.entries.clone();
        image.withheld = self.withheld.clone();
        for name in &self.worlds {
            if !image.worlds.iter().any(|w| &w.name == name) {
                image.worlds.push(World {
                    name: name.clone(),
                    level_name: None,
                    data_version: None,
                    game_version: None,
                    size: None,
                    last_played: None,
                    quiesced: None,
                });
            }
        }
        image.normalize();
    }
}

/// [`plan`] then [`run`].
pub fn snapshot(
    store: &Store,
    root: &Path,
    layout: &DiskLayout,
    options: &SnapshotOptions,
) -> Result<Snapshot, SnapshotError> {
    let plan = plan(store, root, layout, options)?;
    run(store, plan, options, &|_, _| {})
}

/// Walks `root` (a client's instance folder or a server's folder) and decides which files
/// need reading.
pub fn plan(
    store: &Store,
    root: &Path,
    layout: &DiskLayout,
    options: &SnapshotOptions,
) -> Result<Plan, SnapshotError> {
    if !fs::metadata(root).is_ok_and(|m| m.is_dir()) {
        return Err(SnapshotError::RootMissing(root.to_path_buf()));
    }
    let scan_started_ns = fsx::unix_ns(SystemTime::now());
    let excludes = Excludes::new(options.excludes);
    let mut plan = Plan {
        root: root.to_path_buf(),
        scan_started_ns,
        cached: Vec::new(),
        to_read: Vec::new(),
        withheld: Vec::new(),
    };
    let found = walk(root, layout, &excludes, options.cancel, &mut plan.withheld)?;
    for file in found {
        let cached = options
            .cache
            .and_then(|cache| cache.lookup(&file.disk, file.size, file.modified_ns))
            // Claimed, not only looked up: a file taken from the cache is never read, so
            // this is the only time its object is touched before the manifest names it.
            .filter(|sha256| store.claim_object(sha256, file.size))
            .map(str::to_string);
        match cached {
            Some(sha256) => plan.cached.push((file, sha256)),
            None => plan.to_read.push(file),
        }
    }
    Ok(plan)
}

/// What became of one file [`run`] read.
enum Outcome {
    Stored(super::StoredFile),
    /// Deleted since the walk.
    Vanished,
    Withheld(WithheldReason),
}

/// Reads and stores the files `plan` found to read, on several threads, and lists every
/// file as an entry. `progress` gets (bytes read, bytes to read) as files finish.
pub fn run(
    store: &Store,
    plan: Plan,
    options: &SnapshotOptions,
    progress: &(dyn Fn(u64, u64) + Sync),
) -> Result<Snapshot, SnapshotError> {
    let total = plan.bytes_to_read();
    let threads = match options.threads {
        0 => std::thread::available_parallelism()
            .map_or(2, |n| n.get() / 2)
            .clamp(1, DEFAULT_MAX_THREADS),
        n => n,
    }
    .clamp(1, plan.to_read.len().max(1));
    let next = AtomicUsize::new(0);
    let done = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let failure: Mutex<Option<SnapshotError>> = Mutex::new(None);
    let outcomes: Mutex<Vec<(usize, Outcome)>> = Mutex::new(Vec::with_capacity(plan.to_read.len()));
    let fail = |error: SnapshotError| {
        stop.store(true, Ordering::Relaxed);
        if let Ok(mut failure) = failure.lock() {
            failure.get_or_insert(error);
        }
    };
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                let mut mine = Vec::new();
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if (options.cancel)() {
                        fail(SnapshotError::Cancelled);
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(file) = plan.to_read.get(index) else {
                        break;
                    };
                    match read_one(store, &plan.root, file, options) {
                        Ok(outcome) => mine.push((index, outcome)),
                        Err(error) => {
                            fail(error);
                            break;
                        }
                    }
                    let now = done.fetch_add(file.size, Ordering::Relaxed) + file.size;
                    progress(now, total);
                }
                if let Ok(mut outcomes) = outcomes.lock() {
                    outcomes.extend(mine);
                }
            });
        }
    });
    if let Some(error) = failure.into_inner().ok().flatten() {
        return Err(error);
    }
    let outcomes = outcomes.into_inner().unwrap_or_default();

    let mut snapshot = Snapshot {
        entries: Vec::with_capacity(plan.cached.len() + plan.to_read.len()),
        withheld: plan.withheld,
        cache: StatCache::new(plan.scan_started_ns),
        worlds: BTreeSet::new(),
        files_read: 0,
        bytes_read: 0,
        bytes_added: 0,
    };
    let mut add = |file: &FoundFile, sha256: String, size: u64, modified_ns: i64, mode: u32| {
        let mut entry = Entry::new(file.path.clone(), size, sha256.clone(), Source::Object {});
        entry.mode = mode;
        entry.modified = Some(modified_ns);
        if let Some(world) = &entry.world {
            snapshot.worlds.insert(world.clone());
        }
        snapshot
            .cache
            .insert(file.disk.clone(), size, modified_ns, sha256);
        snapshot.entries.push(entry);
    };
    for (file, sha256) in &plan.cached {
        add(file, sha256.clone(), file.size, file.modified_ns, file.mode);
    }
    let mut read_withheld = Vec::new();
    let (mut files_read, mut bytes_read, mut bytes_added) = (0, 0, 0);
    for (index, outcome) in outcomes {
        let Some(file) = plan.to_read.get(index) else {
            continue;
        };
        match outcome {
            Outcome::Stored(stored) => {
                files_read += 1;
                bytes_read += stored.size;
                bytes_added += stored.added_bytes;
                add(
                    file,
                    stored.sha256,
                    stored.size,
                    stored.modified_ns,
                    stored.mode,
                );
            }
            Outcome::Vanished => {}
            Outcome::Withheld(reason) => read_withheld.push(Withheld {
                path: file.path.to_string(),
                reason,
            }),
        }
    }
    snapshot.withheld.extend(read_withheld);
    snapshot.files_read = files_read;
    snapshot.bytes_read = bytes_read;
    snapshot.bytes_added = bytes_added;
    snapshot.entries.sort_by(|a, b| a.path.cmp(&b.path));
    snapshot.withheld.sort();
    snapshot.withheld.dedup();
    Ok(snapshot)
}

/// Reads one file, trying again while it changes under the read.
fn read_one(
    store: &Store,
    root: &Path,
    file: &FoundFile,
    options: &SnapshotOptions,
) -> Result<Outcome, SnapshotError> {
    let absolute = crate::layout::join_disk(root, &file.disk);
    let tries = options.tries.max(1);
    for attempt in 0..tries {
        match store.put_file(&absolute, options.cancel) {
            Ok(stored) => return Ok(Outcome::Stored(stored)),
            Err(StoreError::Changed) => {
                if attempt + 1 < tries {
                    std::thread::sleep(Duration::from_millis(20 << attempt.min(5)));
                }
            }
            Err(StoreError::Source(e)) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Outcome::Vanished)
            }
            Err(StoreError::Source(_)) => return Ok(Outcome::Withheld(WithheldReason::Unreadable)),
            Err(StoreError::Cancelled) => return Err(SnapshotError::Cancelled),
            Err(e) => return Err(SnapshotError::Store(e)),
        }
    }
    Ok(Outcome::Withheld(WithheldReason::KeptChanging))
}

/// The path to show for a file the layout couldn't map: relative to the game folder.
fn as_found(layout: &DiskLayout, disk: &str) -> String {
    match layout {
        DiskLayout::Client => disk
            .strip_prefix(CLIENT_GAME_FOLDER)
            .and_then(|rest| rest.strip_prefix('/'))
            .unwrap_or(disk)
            .to_string(),
        DiskLayout::Server(_) => disk.to_string(),
    }
}

/// Walks `root` without following links below it. Returns the files to take; adds what it
/// withholds to `withheld`.
fn walk(
    root: &Path,
    layout: &DiskLayout,
    excludes: &Excludes,
    cancel: Cancel,
    withheld: &mut Vec<Withheld>,
) -> Result<Vec<FoundFile>, SnapshotError> {
    let mut files = Vec::new();
    let mut withhold = |path: String, reason: WithheldReason| {
        withheld.push(Withheld { path, reason });
    };
    let mut entries = walkdir::WalkDir::new(root)
        .follow_root_links(true)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter();
    while let Some(entry) = entries.next() {
        if cancel() {
            return Err(SnapshotError::Cancelled);
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                // A folder that can't be listed: what is in it is unknown.
                if let Some(disk) = e.path().and_then(|p| relative(root, p)) {
                    if recorded_folder(layout, &disk, excludes) != Folder::Skip {
                        withhold(as_found(layout, &disk), WithheldReason::Unreadable);
                    }
                }
                continue;
            }
        };
        let file_type = entry.file_type();
        let Some(disk) = relative(root, entry.path()) else {
            // Not UTF-8: no canonical path can name it.
            let lossy = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            if lossy.split('/').next() == Some(CLIENT_GAME_FOLDER)
                || matches!(layout, DiskLayout::Server(_))
            {
                withhold(as_found(layout, &lossy), WithheldReason::BadName);
            }
            if file_type.is_dir() {
                entries.skip_current_dir();
            }
            continue;
        };
        if file_type.is_dir() {
            match recorded_folder(layout, &disk, excludes) {
                Folder::Descend => {}
                Folder::Skip => entries.skip_current_dir(),
                Folder::BadName => {
                    withhold(as_found(layout, &disk), WithheldReason::BadName);
                    entries.skip_current_dir();
                }
            }
            continue;
        }
        let path = match layout.from_disk(&disk) {
            Ok(Some(path)) => path,
            Ok(None) => continue,
            Err(_) => {
                withhold(as_found(layout, &disk), WithheldReason::BadName);
                continue;
            }
        };
        if excludes.excludes(path.as_str(), false) {
            continue;
        }
        if file_type.is_symlink() {
            withhold(path.to_string(), WithheldReason::Symlink);
            continue;
        }
        // The listing can be stale for open files on Windows; ask the file itself.
        let meta = match fs::symlink_metadata(entry.path()) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                withhold(path.to_string(), WithheldReason::Unreadable);
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            withhold(path.to_string(), WithheldReason::Symlink);
            continue;
        }
        if !meta.is_file() {
            withhold(path.to_string(), WithheldReason::Unreadable);
            continue;
        }
        files.push(FoundFile {
            disk,
            path,
            size: meta.len(),
            modified_ns: fsx::modified_ns(&meta),
            mode: fsx::mode_of(&meta),
        });
    }
    Ok(files)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Folder {
    Descend,
    /// Nothing in it is recorded (never recorded, outside a client's game folder, or
    /// excluded).
    Skip,
    /// Its own name can't be in a canonical path: everything in it is withheld as one.
    BadName,
}

/// Whether anything in the folder at `disk` can be recorded, asked of the layout with
/// files inside it.
///
/// A folder is skipped as excluded only when its files' canonical paths all lie in one
/// canonical folder, which then matches the excludes. Most folders map that way, but not
/// all: a client's game folder is the image's root (no folder to match), and a Paper
/// level's `world_nether/` holds both `DIM-1/…` (the world's Nether, `saves/world/DIM-1/…`)
/// and its other files (`saves/world/.bukkit_nether/…`). So the probe asks for a plain
/// file and for files under `DIM-1/` and `DIM1/`, and descends unless all three land in
/// the same folder; each file is then checked against the excludes on its own.
fn recorded_folder(layout: &DiskLayout, disk: &str, excludes: &Excludes) -> Folder {
    match layout.from_disk(&format!("{disk}/x")) {
        Ok(None) => Folder::Skip,
        Ok(Some(probe)) => {
            if excludes.is_empty() {
                return Folder::Descend;
            }
            let Some(folder) = probe.as_str().strip_suffix("/x") else {
                // The game folder itself: the image's root.
                return Folder::Descend;
            };
            let same_folder = |inside: &str| {
                layout
                    .from_disk(&format!("{disk}/{inside}/x"))
                    .ok()
                    .flatten()
                    .is_some_and(|p| {
                        p.as_str().strip_suffix(&format!("/{inside}/x")) == Some(folder)
                    })
            };
            if same_folder("DIM-1") && same_folder("DIM1") && excludes.excludes(folder, true) {
                Folder::Skip
            } else {
                Folder::Descend
            }
        }
        Err(_) => {
            let name = disk.rsplit('/').next().unwrap_or(disk);
            if validate_component(name).is_err() {
                Folder::BadName
            } else {
                // A layout conflict or a side's file: each file in it is withheld on its
                // own, since some may map.
                Folder::Descend
            }
        }
    }
}

/// `path` relative to `root`, `/`-separated, if it is valid UTF-8.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests;
