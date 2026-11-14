//! Garbage collection: deleting the objects and chunks no manifest uses.
//!
//! Mark and sweep. Every manifest under `images/` is read; the objects its entries and
//! settings name are live (a whole file in `objects/`, a chunk list in `lists/`, or both),
//! and so are the chunks of every live chunk list. Then every whole object, list and chunk
//! that isn't live is deleted, along with temporary files a crash left.
//!
//! It never guesses:
//!
//! - If any manifest can't be read or parsed, or has a schema other than this version's
//!   (a newer Lodestone wrote it, or it is from before images), nothing is deleted: its
//!   objects can't be told apart from garbage.
//! - If a live chunk list can't be read, no chunk is deleted (its chunks are unknown);
//!   unused objects still are.
//! - Whether a live object has a chunk list is decided from the store, never from the
//!   sizes manifests name it with: a manifest received from elsewhere can give any size.
//!   A live object's list in `lists/` is read whatever size anything gives it, and its
//!   chunks are live. Names with sizes neither the whole object nor the list has are
//!   counted in [`GcReport::wrong_sizes`] and block nothing.
//! - Objects, lists and chunks modified within the grace period (an hour) before the
//!   collection started are kept, with the chunks of such lists. Age is judged against the
//!   moment marking started, never the moment a file is swept, so a manifest saved while
//!   the collection runs (which it didn't mark) never loses an object it names: that
//!   object was either new when marking started, or claimed (touched) by the save, which
//!   makes it newer than the start. A snapshot running at the same time stores its objects before
//!   it writes the manifest that uses them, touches the objects it reuses (see
//!   `Store::put_file`) and those it takes from its stat cache (`Store::claim_object`),
//!   and claims them all again when it saves the manifest (`Store::save_manifest`, which
//!   fails if one is gone), so what it relies on is always new or the save says so.
//!   Temporary files are kept for the same hour.
//! - Writers that claim (this store's own: `put_*`, snapshots, `save_manifest`) need no
//!   lock. A file found old is renamed away first (to a temporary name in its folder), its
//!   time is read again from the renamed file, and it is deleted only if it is still old;
//!   one touched meanwhile is put back. A claim touches an old file and then checks that
//!   it is still there, so either the touch lands before the rename (and the collection
//!   sees it and puts the file back) or the claim finds the file gone and fails, and the
//!   manifest isn't saved. A grace shorter than twice the touch interval would let a
//!   collection take a file a claim found new enough not to touch, so [`GcOptions::grace`]
//!   is never taken below [`MIN_GRACE`]: an object a claim didn't touch was at most ten
//!   minutes old then, and a remembered claim vouches for at most five minutes more, so it
//!   is still within the grace counted back from any collection that started before the
//!   save.
//! - Writers that don't claim are held off by the collection lock. The formats images
//!   replace share the folder and still write there until they are retired (the
//!   Launcher's snapshot store until M5, the core's schema-1 backups until M2), and they
//!   neither touch an object they reuse nor give a clone a new time, so the claims can't
//!   protect what they are about to name. Such a writer holds
//!   [`Store::hold_off_collection`] (a shared lock on `images/collection.lock`) from
//!   before it stores its first object until its manifest is saved, and a collection
//!   holds the same lock exclusively for its whole run, from before it reads any manifest
//!   until its last deletion. The objects of older manifests are read only inside that
//!   hold ([`Store::gc_with_older_formats`] calls the caller's reader once it has the
//!   lock), so every older manifest a collection doesn't read was saved after it ended,
//!   by a writer that started after it ended too, and nothing it deleted was there for
//!   that writer to reuse. The lock is an operating-system file lock, so it holds across
//!   processes and threads, and it is released if a process dies. A thread that holds the
//!   shared lock must not start a collection: it would wait for itself.
//! - [`Store::gc`] is for a folder no older format writes to: it refuses, deleting
//!   nothing, while an older manifest is there (a `.json` in a folder under `servers/` or
//!   `instances/` other than the guard's), since it can't know what that names.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use crate::hex::is_sha256;
use crate::manifest::{whole_number_field, ProbeError, SCHEMA};

use super::chunklist::{ChunkList, MAX_LIST_BYTES};
use super::encoding::{decoded_len, read_limited};
use super::{fsx, manifest_id, read_capped, Area, Store, MAX_MANIFEST_BYTES, TEMP_MAX_AGE};
use super::{StoreError, TOUCH_AFTER};

/// How long new objects, chunks and temporary files are kept even when nothing uses them.
pub const GRACE: Duration = Duration::from_secs(3600);
/// The shortest grace a collection uses, whatever [`GcOptions::grace`] says: twice the
/// time after which a claim touches what it finds (see the module).
pub const MIN_GRACE: Duration = Duration::from_secs(2 * TOUCH_AFTER.as_secs());

#[cfg(test)]
thread_local! {
    static SHORT_GRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Lets collections on this thread use a grace below [`MIN_GRACE`], for tests about what
/// is used rather than what is new.
#[cfg(test)]
pub(crate) fn allow_short_grace() {
    SHORT_GRACE.with(|short| short.set(true));
}

fn min_grace() -> Duration {
    #[cfg(test)]
    if SHORT_GRACE.with(std::cell::Cell::get) {
        return Duration::ZERO;
    }
    MIN_GRACE
}

#[derive(Debug, Clone)]
pub struct GcOptions {
    /// Unused objects and chunks newer than this are kept; never less than [`MIN_GRACE`].
    pub grace: Duration,
    /// Temporary files older than this are deleted.
    pub temp_max_age: Duration,
}

impl Default for GcOptions {
    fn default() -> Self {
        GcOptions {
            grace: GRACE,
            temp_max_age: TEMP_MAX_AGE,
        }
    }
}

/// What reads the objects older formats' manifests name, inside a collection's hold.
type ReadOlder<'a> = Box<dyn FnOnce() -> Result<HashSet<String>, GcError> + 'a>;

/// The collection lock's file, in `images/` (see the module).
pub const COLLECTION_LOCK: &str = "collection.lock";

/// A writer's shared hold on the collection lock ([`Store::hold_off_collection`]): no
/// collection runs while it is held. Released when dropped.
#[derive(Debug)]
#[must_use = "the hold ends when it is dropped"]
pub struct CollectionHold {
    _file: fs::File,
}

/// What a collection did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GcReport {
    pub manifests: usize,
    /// Whole objects and chunk lists.
    pub objects_removed: u64,
    pub chunks_removed: u64,
    /// Encoded bytes.
    pub bytes_removed: u64,
    pub temp_removed: u64,
    /// Unused objects and chunks kept because they are new.
    pub kept_new: u64,
    /// Why no chunk was deleted, when a live chunk list couldn't be read.
    pub chunks_skipped: Option<String>,
    /// Live objects some manifest names with a size they don't stand for (that manifest
    /// is wrong; the object and its chunks are kept as they are).
    pub wrong_sizes: u64,
}

/// Why a collection deleted nothing.
#[derive(Debug, thiserror::Error)]
pub enum GcError {
    #[error("not deleting unused files: {path} {why}")]
    Refused { path: String, why: String },
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
}

/// Only what marking needs from a manifest, so a manifest with fields this version doesn't
/// know still marks its objects. The schema is probed first, on its own.
#[derive(Deserialize)]
struct MarkManifest {
    /// `null` is no entries, as in any manifest.
    #[serde(default)]
    entries: Option<Vec<MarkObject>>,
    #[serde(default)]
    settings: Option<MarkSettings>,
}

#[derive(Deserialize)]
struct MarkObject {
    sha256: String,
    size: u64,
}

#[derive(Deserialize, Default)]
struct MarkSettings {
    #[serde(default)]
    client: Option<MarkClient>,
}

#[derive(Deserialize, Default)]
struct MarkClient {
    #[serde(default)]
    icon: Option<MarkObject>,
    #[serde(default)]
    background: Option<MarkObject>,
}

impl Store {
    /// Holds collections off until the hold is dropped, for a writer that stores objects
    /// in this folder without claiming them (an older format's writer; see the module):
    /// take it before storing the first object and keep it until the manifest naming them
    /// is saved. Waits for a collection that is running. Many writers can hold it at once.
    pub fn hold_off_collection(&self) -> Result<CollectionHold, StoreError> {
        self.ensure_guard()?;
        let file = self
            .open_collection_lock()
            .map_err(super::io_err("Failed to open the store's collection lock"))?;
        file.lock_shared()
            .map_err(super::io_err("Failed to take the store's collection lock"))?;
        Ok(CollectionHold { _file: file })
    }

    fn open_collection_lock(&self) -> io::Result<fs::File> {
        let dir = self.images_dir();
        fs::create_dir_all(&dir)?;
        fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(COLLECTION_LOCK))
    }

    /// Deletes the objects and chunks no manifest uses (see the module documentation), in
    /// a folder no older format writes to: refuses while an older format's manifest is
    /// there. Waits for writers that hold collections off ([`Store::hold_off_collection`]).
    pub fn gc(&self, options: &GcOptions) -> Result<GcReport, GcError> {
        self.collect(options, None)
    }

    /// [`Store::gc`] in a folder older formats share: `read_older` returns every object
    /// their manifests (and the rests of those replaced by images) name, by SHA-256, or
    /// refuses. It is called once the collection holds the lock that holds those formats'
    /// writers off, so nothing it doesn't see can be saved until the collection ends.
    pub fn gc_with_older_formats(
        &self,
        options: &GcOptions,
        read_older: impl FnOnce() -> Result<HashSet<String>, GcError>,
    ) -> Result<GcReport, GcError> {
        self.collect(options, Some(Box::new(read_older)))
    }

    fn collect(
        &self,
        options: &GcOptions,
        read_older: Option<ReadOlder<'_>>,
    ) -> Result<GcReport, GcError> {
        let options = &GcOptions {
            grace: options.grace.max(min_grace()),
            ..options.clone()
        };
        if !self.root.exists() {
            // Nothing to collect, and nothing is written here for nothing.
            return Ok(GcReport::default());
        }
        let io = |what: &str| {
            let what = what.to_string();
            move |source| GcError::Io { what, source }
        };
        // The lock file is a write to the folder: the guard goes first, as for any.
        self.ensure_guard().map_err(|e| GcError::Io {
            what: "Failed to write the store's downgrade guard".into(),
            source: io::Error::other(e.to_string()),
        })?;
        let lock = self
            .open_collection_lock()
            .map_err(io("Failed to open the store's collection lock"))?;
        lock.lock()
            .map_err(io("Failed to take the store's collection lock"))?;
        let older = match read_older {
            Some(read) => read()?,
            None => {
                self.refuse_older_manifests()?;
                HashSet::new()
            }
        };
        // What is new is judged against this, taken before any manifest is read.
        let started = SystemTime::now();
        let mut report = GcReport::default();
        // Every size each live object is named with; objects only older formats name
        // have none. Filled as each manifest is read, so memory follows the objects, not
        // the manifests (backups share most of theirs).
        let mut sizes: HashMap<String, Vec<u64>> = HashMap::new();
        self.mark_manifests(&mut sizes, &mut report)?;
        for sha256 in older {
            sizes.entry(sha256).or_default();
        }

        let mut live_chunks: HashSet<String> = HashSet::new();
        let mut chunks_blocked: Option<String> = None;
        // Each object once, by its hash: what it is doesn't depend on who names it.
        let mut names: Vec<(&String, &Vec<u64>)> = sizes.iter().collect();
        names.sort();
        for (sha256, named) in names {
            match self.live_object(sha256, named) {
                Ok((list, wrong)) => {
                    report.wrong_sizes += u64::from(wrong);
                    if let Some(list) = list {
                        live_chunks.extend(list.chunks.into_iter().map(|c| c.sha256));
                    }
                }
                Err(why) => {
                    chunks_blocked.get_or_insert(why);
                }
            }
        }

        report.temp_removed = self.sweep_temp(options.temp_max_age);
        let sweep = Sweep { started, options };
        self.sweep(Area::Objects, &sweep, &mut report, |sha256, new| {
            if sizes.contains_key(sha256) {
                Verdict::Live
            } else if new {
                Verdict::KeptNew
            } else {
                Verdict::Delete
            }
        })?;
        // Lists before chunks: new unused ones keep their chunks.
        self.sweep(Area::Lists, &sweep, &mut report, |sha256, new| {
            if sizes.contains_key(sha256) {
                return Verdict::Live;
            }
            if !new {
                return Verdict::Delete;
            }
            match self.read_list(sha256) {
                Ok(Some(list)) => live_chunks.extend(list.chunks.into_iter().map(|c| c.sha256)),
                Ok(None) => {}
                Err(why) => {
                    chunks_blocked.get_or_insert(why);
                }
            }
            Verdict::KeptNew
        })?;
        self.forget_claims_if_collected(&report);
        if let Some(why) = chunks_blocked {
            report.chunks_skipped = Some(why);
            return Ok(report);
        }
        self.sweep(Area::Chunks, &sweep, &mut report, |sha256, new| {
            if live_chunks.contains(sha256) {
                Verdict::Live
            } else if new {
                Verdict::KeptNew
            } else {
                Verdict::Delete
            }
        })?;
        self.forget_claims_if_collected(&report);
        Ok(report)
    }

    /// Refuses when an older format's manifest is in the folder (see the module).
    fn refuse_older_manifests(&self) -> Result<(), GcError> {
        for top in ["servers", "instances"] {
            let top = self.root.join(top);
            let refuse = |path: &Path, why: String| GcError::Refused {
                path: path.display().to_string(),
                why,
            };
            let folders = match fs::read_dir(&top) {
                Ok(folders) => folders,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(refuse(&top, format!("can't be listed ({e})"))),
            };
            for folder in folders {
                let folder = folder.map_err(|e| refuse(&top, format!("can't be listed ({e})")))?;
                if folder.file_name() == super::guard::GUARD_FOLDER
                    || !folder.file_type().is_ok_and(|t| t.is_dir())
                {
                    continue;
                }
                let files = fs::read_dir(folder.path())
                    .map_err(|e| refuse(&folder.path(), format!("can't be listed ({e})")))?;
                for file in files.flatten() {
                    if file.path().extension().is_some_and(|x| x == "json") {
                        return Err(refuse(
                            &file.path(),
                            "is an older format's manifest; collect with \
                             Store::gc_with_older_formats"
                                .into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// A claim this store remembers vouches for an object only while no collection could
    /// have taken it ([`MIN_GRACE`]); one that did delete files here, with a test's shorter
    /// grace, say, makes this store claim again.
    fn forget_claims_if_collected(&self, report: &GcReport) {
        if report.objects_removed + report.chunks_removed > 0 {
            if let Ok(mut claimed) = self.claimed.lock() {
                claimed.clear();
            }
        }
    }

    /// Reads every manifest under `images/` for the objects it names.
    fn mark_manifests(
        &self,
        live: &mut HashMap<String, Vec<u64>>,
        report: &mut GcReport,
    ) -> Result<(), GcError> {
        let instances = match fs::read_dir(self.images_dir()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(GcError::Io {
                    what: "Failed to list the images".into(),
                    source,
                })
            }
        };
        for instance in instances {
            let instance = instance.map_err(|source| GcError::Io {
                what: "Failed to list the images".into(),
                source,
            })?;
            if !instance.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let manifests = fs::read_dir(instance.path()).map_err(|source| GcError::Io {
                what: format!("Failed to list {}", instance.path().display()),
                source,
            })?;
            for manifest in manifests {
                let manifest = manifest.map_err(|source| GcError::Io {
                    what: format!("Failed to list {}", instance.path().display()),
                    source,
                })?;
                let name = manifest.file_name();
                if !name.to_str().is_some_and(|n| manifest_id(n).is_some()) {
                    continue;
                }
                mark_one(&manifest.path(), live)?;
                report.manifests += 1;
            }
        }
        Ok(())
    }

    /// The chunk list of the live object `sha256`, if the store has one (its chunks are
    /// live), and whether a manifest names it with a size neither its whole object nor its
    /// list has (`named`: the sizes, empty when only an older format names it). Decided
    /// from the store, so no name can hide a list's chunks. An error when the list is there
    /// and can't be read.
    fn live_object(
        &self,
        sha256: &str,
        named: &[u64],
    ) -> Result<(Option<ChunkList>, bool), String> {
        let list = self.read_list(sha256)?;
        if named.is_empty() {
            return Ok((list, false));
        }
        let whole = self
            .object_path(sha256)
            .ok()
            .and_then(|path| decoded_len(self.encoding, &path));
        let listed = list.as_ref().map(|list| list.size);
        let wrong = (whole.is_some() || listed.is_some())
            && named
                .iter()
                .any(|&size| Some(size) != whole && Some(size) != listed);
        Ok((list, wrong))
    }

    /// The chunk list in `lists/` for `sha256`; `None` when there is none, an error when
    /// one is there and can't be read or isn't a list.
    fn read_list(&self, sha256: &str) -> Result<Option<ChunkList>, String> {
        let Ok(path) = self.list_path(sha256) else {
            return Ok(None);
        };
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("the chunk list {sha256} can't be read ({e})")),
        }
        let bytes = read_limited(self.encoding, &path, MAX_LIST_BYTES)
            .map_err(|e| format!("the chunk list {sha256} can't be read ({e})"))?;
        ChunkList::parse(&bytes)
            .map(Some)
            .map_err(|why| format!("the chunk list {sha256} can't be read ({why})"))
    }

    /// Deletes the files of `area` that `verdict(name, is_new)` says to. Files that aren't
    /// content-addressed names in their right folder are left alone.
    fn sweep(
        &self,
        area: Area,
        sweep: &Sweep,
        report: &mut GcReport,
        mut verdict: impl FnMut(&str, bool) -> Verdict,
    ) -> Result<(), GcError> {
        let Ok(prefixes) = fs::read_dir(self.area_dir(area)) else {
            return Ok(());
        };
        for prefix in prefixes.flatten() {
            if !prefix.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let prefix_name = prefix.file_name();
            let Some(prefix_name) = prefix_name.to_str().map(str::to_string) else {
                continue;
            };
            let Ok(files) = fs::read_dir(prefix.path()) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name();
                let Some(name) = name.to_str() else { continue };
                if !is_sha256(name) || name[..2] != prefix_name {
                    continue;
                }
                let Ok(meta) = file.metadata() else { continue };
                let new = sweep.is_new(&meta);
                match verdict(name, new) {
                    Verdict::Live => continue,
                    Verdict::KeptNew => {
                        report.kept_new += 1;
                        continue;
                    }
                    Verdict::Delete => {}
                }
                match collect(&file.path(), sweep) {
                    Ok(Some(len)) => {
                        match area {
                            Area::Objects | Area::Lists => report.objects_removed += 1,
                            Area::Chunks => report.chunks_removed += 1,
                        }
                        report.bytes_removed += len;
                    }
                    // Claimed while it was being collected: put back.
                    Ok(None) => report.kept_new += 1,
                    // Someone else deleted it, or it is in use: it stays for next time.
                    Err(_) => continue,
                }
            }
        }
        Ok(())
    }
}

/// When a sweep counts a file as new.
struct Sweep<'o> {
    /// When the collection started, before marking.
    started: SystemTime,
    options: &'o GcOptions,
}

impl Sweep<'_> {
    /// Whether a file with this metadata was modified within the grace before the
    /// collection started, or since (a time that can't be told counts as new).
    fn is_new(&self, meta: &fs::Metadata) -> bool {
        fsx::age(meta, self.started).is_none_or(|age| age < self.options.grace)
    }
}

/// What a sweep does with one file.
enum Verdict {
    /// Something uses it.
    Live,
    /// Nothing uses it, but it is new.
    KeptNew,
    Delete,
}

/// Deletes the unused, old file at `path`, unless something claims it meanwhile (see the
/// module): renames it away, reads its time again from the renamed file, and puts it back
/// if it was touched since it was found old. Returns the bytes freed, or `None` when it was
/// put back.
fn collect(path: &Path, sweep: &Sweep) -> io::Result<Option<u64>> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a store file without a folder"))?;
    let aside = fsx::temp_path(dir);
    fsx::rename_with_retry(path, &aside)?;
    let meta = match fs::symlink_metadata(&aside) {
        Ok(meta) => meta,
        Err(e) => {
            let _ = fs::rename(&aside, path);
            return Err(e);
        }
    };
    // Against the start, as when it was found old: a touch since then makes it new.
    let touched = sweep.is_new(&meta);
    if touched {
        // On unix this replaces a copy written meanwhile (the same bytes, by its name);
        // where a rename can't replace, that copy is as good and ours goes.
        if fsx::rename_with_retry(&aside, path).is_err() {
            let _ = fsx::remove_file_forced(&aside);
        }
        return Ok(None);
    }
    // Should this fail, the file is a temporary one now, swept within the hour.
    let _ = fsx::remove_file_forced(&aside);
    Ok(Some(meta.len()))
}

/// Marks the objects of the manifest at `path`, or refuses.
fn mark_one(path: &Path, live: &mut HashMap<String, Vec<u64>>) -> Result<(), GcError> {
    let refuse = |why: String| GcError::Refused {
        path: path.display().to_string(),
        why,
    };
    let bytes = read_capped(path, MAX_MANIFEST_BYTES).map_err(|e| match e {
        StoreError::Manifest(_) => refuse(format!("is larger than {MAX_MANIFEST_BYTES} bytes")),
        other => refuse(format!("can't be read ({other})")),
    })?;
    match whole_number_field(&bytes, "schema") {
        Ok(Some(schema)) if schema == u64::from(SCHEMA) => {}
        Ok(Some(schema)) if schema > u64::from(SCHEMA) => {
            return Err(refuse(format!(
                "was made by a newer Lodestone (schema {schema})"
            )))
        }
        Err(ProbeError::Json(e)) => return Err(refuse(format!("can't be parsed ({e})"))),
        Err(ProbeError::Shape(e)) => return Err(refuse(format!("can't be parsed (it {e})"))),
        _ => return Err(refuse("has no schema this version reads".into())),
    }
    let manifest: MarkManifest =
        serde_json::from_slice(&bytes).map_err(|e| refuse(format!("can't be parsed ({e})")))?;
    let client = manifest.settings.and_then(|s| s.client).unwrap_or_default();
    for object in manifest
        .entries
        .unwrap_or_default()
        .into_iter()
        .chain(client.icon)
        .chain(client.background)
    {
        let named = live.entry(object.sha256).or_default();
        if !named.contains(&object.size) {
            named.push(object.size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
