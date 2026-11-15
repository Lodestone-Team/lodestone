//! Deleted instances' snapshots: kept, under a folder of their own, until the user deletes
//! them.
//!
//! Instance ids are folder names, reused by the next instance of the same name, and an
//! instance can vanish without the Launcher deleting it (moved to the Trash in Finder, a
//! sync tool removing it for a while). Its snapshots must not go with it. So deleting an
//! instance's images ([`Images::delete_instance`]), forgetting one deleted outside the
//! Launcher ([`Images::forget_deleted`]) and the id-reuse guard (images under an id that
//! belong to an earlier instance) all **retire** the folder rather than delete it, when it
//! holds any snapshot (a data or full image, or a record):
//!
//! 1. `retired.json` is written into `images/<id>/` (the former id, the instance's name
//!    and creation time, when it was deleted);
//! 2. the folder is renamed to `images/_deleted_<16 hex>/` (`_` never starts an id the
//!    Launcher makes);
//! 3. its content state, stat caches and restore journal are deleted, and the content
//!    images no snapshot links to.
//!
//! A folder without snapshots is deleted outright, as before. A crash after step 1 leaves a
//! marked folder under a live id: the marker wins over everything else there (the guard
//! and the next forgetting retire it), so it is never taken for the new instance's. A crash
//! after step 2 leaves files step 3 deletes the next time a collection or clean-up looks.
//! A retired folder's manifests are marked by every collection like any other, so their
//! objects stay. Settings › Storage lists them ([`Images::deleted_snapshots`]) and its
//! clean-up deletes them ([`Images::clean_up`]).

use std::collections::HashSet;
use std::path::Path;

use lodestone_image::Kind;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::valid_instance_id;
use serde::{Deserialize, Serialize};

use super::snapshots::{RECORDS_FILE, SnapshotRecord};
use super::{ImageError, Images, remove_tree};

/// The marker in a retired folder.
pub const RETIRED_FILE: &str = "retired.json";
/// The start of a retired folder's name in `images/`.
pub const RETIRED_PREFIX: &str = "_deleted_";
/// A marker is small; a larger one is damaged (and still a marker).
const MAX_MARKER_BYTES: u64 = 64 << 10;

/// `retired.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredMarker {
    /// The instance id the folder had.
    #[serde(default)]
    pub former_id: String,
    /// The instance's name, when it was known.
    #[serde(default)]
    pub name: Option<String>,
    /// The instance's `created` time.
    #[serde(default)]
    pub instance_created: Option<u64>,
    /// When it was retired, unix seconds.
    #[serde(default)]
    pub deleted: i64,
}

/// A deleted instance's snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Retired {
    /// The folder in `images/` (`_deleted_<16 hex>`).
    pub folder: String,
    pub marker: RetiredMarker,
    /// Newest first.
    pub records: Vec<SnapshotRecord>,
}

/// What [`Images::clean_up`] deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CleanedUp {
    /// Snapshot records deleted from live instances.
    pub records: usize,
    /// Deleted instances whose snapshots went.
    pub retired: usize,
}

impl Images {
    /// Retires the instance `id`'s image folder (see the module), or deletes it when it
    /// holds no snapshot. The caller holds the instance's record lock inside a counted
    /// write. A rename that fails leaves the folder where it is (and the error), never
    /// deletes it.
    pub(crate) fn retire_locked(&self, id: &str) -> Result<(), ImageError> {
        let dir = self.store().instance_dir(id)?;
        if !dir.is_dir() {
            return Ok(());
        }
        let marked = dir.join(RETIRED_FILE).exists();
        if !marked && !self.holds_snapshots(id)? {
            return remove_tree(&dir).map_err(ImageError::io(format!(
                "Failed to delete {}",
                dir.display()
            )));
        }
        if !marked {
            let state = super::state::load(self.store(), id);
            let marker = RetiredMarker {
                former_id: id.to_string(),
                name: self.records_instance_name(id),
                instance_created: state.instance_created,
                deleted: chrono::Utc::now().timestamp(),
            };
            let bytes = serde_json::to_vec_pretty(&marker)
                .map_err(|e| ImageError::Invalid(format!("Failed to mark the images: {e}")))?;
            super::write_small_atomic(&dir.join(RETIRED_FILE), &bytes)?;
        }
        let images_dir = self.store().images_dir();
        let folder = loop {
            let name = format!(
                "{RETIRED_PREFIX}{:016x}",
                u64::from_le_bytes(crate::random_bytes())
            );
            if !images_dir.join(&name).exists() {
                break name;
            }
        };
        crate::fsx::rename(&dir, images_dir.join(&folder)).map_err(ImageError::io(format!(
            "Failed to set aside the images of {id}"
        )))?;
        super::warn(
            "images.retired",
            id,
            &format!("the instance's snapshots are kept in {folder}"),
        );
        self.tidy_retired(&folder);
        Ok(())
    }

    /// Whether `images/<id>/` holds a snapshot: a data or full manifest (or one that can't
    /// be read, which may be one), or a records file with records.
    fn holds_snapshots(&self, id: &str) -> Result<bool, ImageError> {
        for image in self.store().manifest_ids(id)? {
            match self.store().load_header(id, &image) {
                Ok(header) if header.kind == Kind::Content => {}
                _ => return Ok(true),
            }
        }
        let records = self.store().instance_dir(id)?.join(RECORDS_FILE);
        Ok(std::fs::metadata(records).is_ok_and(|meta| meta.len() > 0))
    }

    /// The instance name the records file keeps, if any.
    fn records_instance_name(&self, id: &str) -> Option<String> {
        #[derive(Deserialize)]
        struct Named {
            #[serde(default)]
            instance_name: Option<String>,
        }
        let path = self.store().instance_dir(id).ok()?.join(RECORDS_FILE);
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice::<Named>(&bytes).ok()?.instance_name
    }

    /// Deletes what a retired folder no longer needs: the content state, the stat caches,
    /// a restore journal (its instance is gone), and the content images no snapshot there
    /// links to. Best effort: what fails stays until the next look.
    fn tidy_retired(&self, folder: &str) {
        let Ok(dir) = self.store().instance_dir(folder) else {
            return;
        };
        for file in [
            super::state::FILE,
            super::swap::JOURNAL_FILE,
            "stat-cache.json",
        ] {
            let _ = std::fs::remove_file(dir.join(file));
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name
                    .to_str()
                    .is_some_and(|n| n.starts_with("stat-cache.") && n.ends_with(".json"))
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        let Ok(listing) = self.listing(folder) else {
            return;
        };
        if !listing.unreadable.is_empty() {
            return;
        }
        let linked: HashSet<&String> = listing
            .links
            .iter()
            .filter_map(|(_, content)| content.as_ref())
            .collect();
        for header in &listing.content {
            if !linked.contains(&header.id) {
                let _ = self.store().delete_manifest(folder, &header.id);
            }
        }
    }

    /// Whether `images/<id>/` has a retirement marker: its images are a deleted instance's,
    /// whatever else is there (a crash between the marker and the rename).
    pub(crate) fn retired_marker_in(&self, id: &str) -> bool {
        self.store()
            .instance_dir(id)
            .is_ok_and(|dir| dir.join(RETIRED_FILE).exists())
    }

    /// The deleted instances whose snapshots are kept, most recently deleted first.
    /// Blocking.
    pub fn deleted_snapshots(&self) -> Result<Vec<Retired>, ImageError> {
        let mut retired = Vec::new();
        for folder in self.retired_folders() {
            let slot = self.slot(&folder);
            let _record = slot.lock_record();
            let Ok(_writing) = self.enter() else {
                break;
            };
            let marker = read_marker(&self.store().instance_dir(&folder)?);
            let records = self.reconciled_locked(&folder)?;
            retired.push(Retired {
                folder,
                marker,
                records,
            });
        }
        retired.sort_by_key(|r| std::cmp::Reverse(r.marker.deleted));
        Ok(retired)
    }

    /// The retired folders in `images/`.
    pub(crate) fn retired_folders(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.store().images_dir()) else {
            return Vec::new();
        };
        let mut folders: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|name| name.starts_with(RETIRED_PREFIX) && valid_instance_id(name))
            .collect();
        folders.sort();
        folders
    }

    /// Settings › Storage's clean-up: deletes the snapshots the Launcher took on its own
    /// (automatic and safety snapshots, not pinned) of every live instance in `live`, and
    /// every deleted instance's snapshots, then collects at once. Manual, imported and
    /// pinned snapshots of live instances stay. Objects written within the last hour stay
    /// until a later collection (the store's grace). Blocking.
    pub fn clean_up(&self, live: &[String]) -> Result<CleanedUp, ImageError> {
        let mut cleaned = CleanedUp::default();
        for id in live {
            if !self.accepts(id) {
                continue;
            }
            let slot = self.slot(id);
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            let Ok(mut records) = self.reconciled_locked(id) else {
                continue;
            };
            let ids: Vec<String> = records
                .iter()
                .filter(|r| r.kind.removable() && !r.pinned)
                .map(|r| r.id.clone())
                .collect();
            if ids.is_empty() {
                continue;
            }
            self.delete_records_locked(id, &mut records, &ids)?;
            cleaned.records += ids.len();
            let current = super::state::load(self.store(), id).current;
            if let Err(e) = self.prune_locked(id, current.as_deref()) {
                super::warn("images.prune", id, &e.to_string());
            }
        }
        for folder in self.retired_folders() {
            let slot = self.slot(&folder);
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            let dir = self.store().instance_dir(&folder)?;
            remove_tree(&dir).map_err(ImageError::io(format!(
                "Failed to delete {}",
                dir.display()
            )))?;
            cleaned.retired += 1;
        }
        self.collect_now()?;
        Ok(cleaned)
    }

    /// A collection now, on this thread (Settings › Storage, which then measures what it
    /// freed). Waits for one running in the background. Blocking.
    pub fn collect_now(&self) -> Result<(), ImageError> {
        let _writing = self.enter()?;
        self.forget_deleted_held();
        self.store()
            .gc(&GcOptions::default())
            .map_err(|e| ImageError::Invalid(format!("The collection failed: {e}")))?;
        Ok(())
    }
}

/// The marker in `dir`; the default (no name, no time) when it is damaged.
fn read_marker(dir: &Path) -> RetiredMarker {
    let path = dir.join(RETIRED_FILE);
    let readable = std::fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_MARKER_BYTES);
    readable
        .then(|| std::fs::read(&path).ok())
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}
