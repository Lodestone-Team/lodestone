//! Garbage collection: deleting the objects and chunks no manifest uses.
//!
//! Mark and sweep. Every manifest under `images/` is read; the objects its entries and
//! settings name are live, and so are the chunks of every live chunk list. Then every
//! object and chunk that isn't live is deleted, along with temporary files a crash left.
//!
//! It never guesses:
//!
//! - If any manifest can't be read or parsed, or has a schema other than this version's
//!   (a newer Lodestone wrote it, or it is from before images), nothing is deleted: its
//!   objects can't be told apart from garbage.
//! - If a live chunk list can't be read, no chunk is deleted (its chunks are unknown);
//!   unused objects still are.
//! - Objects and chunks modified within the grace period (an hour) are kept, with the
//!   chunks of such lists: a snapshot running at the same time stores its objects before
//!   it writes the manifest that uses them, and touches the objects it reuses (see
//!   `Store::put_file`), so what it relies on is always new. Temporary files are kept for
//!   the same hour.
//! - Objects that only older formats use (the core's schema 1 backups, the Launcher's
//!   snapshots) are kept when the caller passes them in [`GcOptions::also_live`]; the
//!   caller reads those manifests and refuses as above when it can't.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde_json::Value;

use crate::hex::is_sha256;
use crate::manifest::SCHEMA;

use super::chunklist::{ChunkList, CHUNKED_ABOVE};
use super::encoding::{decoded_len, read_limited};
use super::{fsx, manifest_id, Area, Store, TEMP_MAX_AGE};

/// How long new objects, chunks and temporary files are kept even when nothing uses them.
pub const GRACE: Duration = Duration::from_secs(3600);
/// The most bytes read from an object that isn't known to be a chunk list, to tell
/// whether it is one.
const MAX_UNKNOWN_LIST: u64 = 64 << 20;

#[derive(Debug, Clone)]
pub struct GcOptions {
    /// Unused objects and chunks newer than this are kept.
    pub grace: Duration,
    /// Temporary files older than this are deleted.
    pub temp_max_age: Duration,
    /// Objects other manifests use (older formats), by SHA-256.
    pub also_live: HashSet<String>,
}

impl Default for GcOptions {
    fn default() -> Self {
        GcOptions {
            grace: GRACE,
            temp_max_age: TEMP_MAX_AGE,
            also_live: HashSet::new(),
        }
    }
}

/// What a collection did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GcReport {
    pub manifests: usize,
    pub objects_removed: u64,
    pub chunks_removed: u64,
    /// Encoded bytes.
    pub bytes_removed: u64,
    pub temp_removed: u64,
    /// Unused objects and chunks kept because they are new.
    pub kept_new: u64,
    /// Why no chunk was deleted, when a live chunk list couldn't be read.
    pub chunks_skipped: Option<String>,
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
/// know still marks its objects.
#[derive(Deserialize)]
struct MarkManifest {
    schema: Value,
    #[serde(default)]
    entries: Vec<MarkObject>,
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
    /// Deletes the objects and chunks no manifest uses (see the module documentation).
    pub fn gc(&self, options: &GcOptions) -> Result<GcReport, GcError> {
        let mut report = GcReport::default();
        // (sha256, size) of every object a manifest names.
        let mut live: Vec<(String, u64)> = Vec::new();
        self.mark_manifests(&mut live, &mut report)?;
        let mut live_objects: HashSet<String> = live.iter().map(|(h, _)| h.clone()).collect();
        live_objects.extend(options.also_live.iter().cloned());

        let mut live_chunks: HashSet<String> = HashSet::new();
        let mut chunks_blocked: Option<String> = None;
        let mut checked: HashSet<&str> = HashSet::new();
        for (sha256, size) in &live {
            if *size <= CHUNKED_ABOVE || !checked.insert(sha256.as_str()) {
                continue;
            }
            match self.live_list(sha256, *size) {
                Ok(Some(list)) => live_chunks.extend(list.chunks.into_iter().map(|c| c.sha256)),
                Ok(None) => {}
                Err(why) => {
                    chunks_blocked.get_or_insert(why);
                }
            }
        }

        let now = SystemTime::now();
        report.temp_removed = self.sweep_temp(options.temp_max_age);
        // Objects first: new unused ones keep their chunks.
        self.sweep(Area::Objects, now, options, &mut report, |sha256, new| {
            if live_objects.contains(sha256) {
                return Verdict::Live;
            }
            if !new {
                return Verdict::Delete;
            }
            if let Err(why) = self.mark_unknown_list(sha256, &mut live_chunks) {
                chunks_blocked.get_or_insert(why);
            }
            Verdict::KeptNew
        })?;
        if let Some(why) = chunks_blocked {
            report.chunks_skipped = Some(why);
            return Ok(report);
        }
        self.sweep(Area::Chunks, now, options, &mut report, |sha256, new| {
            if live_chunks.contains(sha256) {
                Verdict::Live
            } else if new {
                Verdict::KeptNew
            } else {
                Verdict::Delete
            }
        })?;
        Ok(report)
    }

    /// Reads every manifest under `images/` for the objects it names.
    fn mark_manifests(
        &self,
        live: &mut Vec<(String, u64)>,
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

    /// The chunk list of a live object a manifest names with `size` bytes: `None` when the
    /// object is whole or missing (nothing to keep), an error when it should be a list and
    /// can't be read as one.
    fn live_list(&self, sha256: &str, size: u64) -> Result<Option<ChunkList>, String> {
        let Ok(path) = self.object_path(sha256) else {
            return Ok(None);
        };
        let Some(len) = decoded_len(self.encoding, &path) else {
            return if fs::symlink_metadata(&path).is_ok() {
                Err(format!("the chunk list {sha256} can't be read"))
            } else {
                Ok(None)
            };
        };
        if len == size {
            return Ok(None);
        }
        let list = read_limited(self.encoding, &path, ChunkList::max_encoded_len(size))
            .map_err(|e| e.to_string())
            .and_then(|bytes| ChunkList::parse(&bytes))
            .map_err(|why| format!("the chunk list {sha256} can't be read ({why})"))?;
        if list.size != size {
            return Err(format!("the chunk list {sha256} is for another size"));
        }
        Ok(Some(list))
    }

    /// Marks the chunks of a new object no manifest names yet, if it is a chunk list.
    fn mark_unknown_list(
        &self,
        sha256: &str,
        live_chunks: &mut HashSet<String>,
    ) -> Result<(), String> {
        let Ok(path) = self.object_path(sha256) else {
            return Ok(());
        };
        let Some(len) = decoded_len(self.encoding, &path) else {
            return Ok(());
        };
        if len > MAX_UNKNOWN_LIST {
            return Ok(());
        }
        let Ok(bytes) = read_limited(self.encoding, &path, len) else {
            return Err(format!("the new object {sha256} can't be read"));
        };
        if !ChunkList::looks_like(&bytes) {
            return Ok(());
        }
        // A small file that starts like a list but isn't one is kept anyway (it is new);
        // its "chunks" can't be told, so none are deleted this time.
        let list = ChunkList::parse(&bytes).map_err(|why| {
            format!("the new object {sha256} looks like a chunk list but isn't ({why})")
        })?;
        live_chunks.extend(list.chunks.into_iter().map(|c| c.sha256));
        Ok(())
    }

    /// Deletes the files of `area` that `verdict(name, is_new)` says to. Files that aren't
    /// content-addressed names in their right folder are left alone.
    fn sweep(
        &self,
        area: Area,
        now: SystemTime,
        options: &GcOptions,
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
                let new = fsx::age(&meta, now).is_none_or(|age| age < options.grace);
                match verdict(name, new) {
                    Verdict::Live => continue,
                    Verdict::KeptNew => {
                        report.kept_new += 1;
                        continue;
                    }
                    Verdict::Delete => {}
                }
                match remove(&file.path()) {
                    Ok(()) => {
                        match area {
                            Area::Objects => report.objects_removed += 1,
                            Area::Chunks => report.chunks_removed += 1,
                        }
                        report.bytes_removed += meta.len();
                    }
                    // Someone else deleted it, or it is in use: it stays for next time.
                    Err(_) => continue,
                }
            }
        }
        Ok(())
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

fn remove(path: &Path) -> io::Result<()> {
    fsx::remove_file_forced(path)
}

/// Marks the objects of the manifest at `path`, or refuses.
fn mark_one(path: &Path, live: &mut Vec<(String, u64)>) -> Result<(), GcError> {
    let refuse = |why: String| GcError::Refused {
        path: path.display().to_string(),
        why,
    };
    let bytes = fs::read(path).map_err(|e| refuse(format!("can't be read ({e})")))?;
    let manifest: MarkManifest =
        serde_json::from_slice(&bytes).map_err(|e| refuse(format!("can't be parsed ({e})")))?;
    match manifest.schema.as_u64() {
        Some(schema) if schema == u64::from(SCHEMA) => {}
        Some(schema) if schema > u64::from(SCHEMA) => {
            return Err(refuse(format!(
                "was made by a newer Lodestone (schema {schema})"
            )))
        }
        _ => return Err(refuse("has no schema this version reads".into())),
    }
    let client = manifest.settings.and_then(|s| s.client).unwrap_or_default();
    for object in manifest
        .entries
        .into_iter()
        .chain(client.icon)
        .chain(client.background)
    {
        live.push((object.sha256, object.size));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
