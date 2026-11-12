//! The history of a server's modpack: every install, update and restore, which makes
//! any of them undoable, and shows which update brought each file (like `git blame`).
//!
//! [`HISTORY_FILE`] keeps, for each entry, the pack's files after it (path and SHA-1, as
//! the record has them), the changes it made, and the Minecraft version and server
//! software. Restoring an entry is an update whose new files are that entry's.
//!
//! It is small because file contents are kept only when they couldn't be got back
//! otherwise ([`History::keep`]):
//!
//! - A file the pack downloads (most mods) is kept as its download: its URLs, its size
//!   and its SHA-512, which is checked when it is downloaded again. Lodestone only keeps
//!   URLs on the hosts packs may download from ([`is_allowed_download`]): the history is
//!   a file anyone who can write the server's files can edit.
//! - A file that can't be downloaded (configs and other overrides, files from before the
//!   history) is copied into [`OBJECTS_DIR`] when an update replaces or removes it, once
//!   per content (the store is keyed by SHA-1). Files that stay on the server need no copy.
//!
//! Only the newest [`MAX_ENTRIES`] entries are kept, and stored files no entry refers to
//! are deleted with them.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use lodestone_minecraft::mrpack::{is_allowed_download, PathPolicy};
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use super::update::{
    record_path, FileAction, FilePlan, PackFile, PackRecord, RecordedFile, MODPACK_DIR,
};
use super::{InstancePathPolicy, ModpackOrigin, PackDownload};
use crate::error::Error;
use crate::implementations::minecraft::Flavour;

/// The history, in the instance folder.
pub const HISTORY_FILE: &str = ".lodestone_modpack/history.json";
/// Stored file contents, as `<first two hex digits>/<SHA-1>`.
pub const OBJECTS_DIR: &str = ".lodestone_modpack/objects";
/// How many entries are kept.
pub const MAX_ENTRIES: usize = 30;

/// What an entry was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum HistoryKind {
    /// The server was made from the pack.
    Install,
    /// What the server had when its history began: a server made before Lodestone kept
    /// one.
    Baseline,
    Update,
    /// The server went back to entry `to`.
    Restore {
        to: u32,
    },
}

/// What a change did to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ChangeAction {
    Added,
    Replaced,
    Removed,
}

/// One file an entry changed. `path` is the pack's path, `target` the path on disk (the
/// disabled name of a disabled mod).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryChange {
    pub path: String,
    pub target: String,
    pub action: ChangeAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u32,
    /// Unix milliseconds.
    pub time: i64,
    pub kind: HistoryKind,
    pub origin: ModpackOrigin,
    pub game_version: String,
    pub flavour: Flavour,
    /// The pack's files after this entry, as the record has them.
    pub files: BTreeMap<String, RecordedFile>,
    #[serde(default)]
    pub changes: Vec<HistoryChange>,
    /// Pack files the user had changed, which this entry left as they were.
    #[serde(default)]
    pub kept: Vec<String>,
}

/// How to get a file back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    /// A copy is in [`OBJECTS_DIR`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stored: bool,
}

impl FileRef {
    /// The allowed URLs, if the file can be downloaded again and checked.
    fn download_urls(&self) -> Option<(Vec<url::Url>, String)> {
        let sha512 = self
            .sha512
            .clone()
            .filter(|h| h.len() == 128 && h.chars().all(|c| c.is_ascii_hexdigit()))?;
        let urls: Vec<url::Url> = self
            .urls
            .iter()
            .filter_map(|u| url::Url::parse(u).ok())
            .filter(is_allowed_download)
            .collect();
        (!urls.is_empty()).then_some((urls, sha512))
    }
}

/// [`HISTORY_FILE`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// Oldest first.
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
    /// How to get each file of the entries back, by SHA-1.
    #[serde(default)]
    pub files: BTreeMap<String, FileRef>,
}

fn is_sha1(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Where the stored copy of the file with `sha1` is.
pub fn object_path(root: &Path, sha1: &str) -> Option<PathBuf> {
    let sha1 = sha1.to_lowercase();
    is_sha1(&sha1).then(|| root.join(OBJECTS_DIR).join(&sha1[..2]).join(&sha1))
}

/// Unix milliseconds now.
pub fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl History {
    /// The history in `root`; empty when there is none, or it can't be read.
    pub async fn load(root: &Path) -> History {
        let Ok(bytes) = tokio::fs::read(root.join(HISTORY_FILE)).await else {
            return History::default();
        };
        serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            warn!("Ignoring {HISTORY_FILE}, which can't be read: {e}");
            History::default()
        })
    }

    pub async fn save(&self, root: &Path) -> Result<(), Error> {
        let json = serde_json::to_vec(self).context("Failed to write the modpack history")?;
        tokio::fs::create_dir_all(root.join(MODPACK_DIR))
            .await
            .context("Failed to create the modpack folder")?;
        crate::persist::write_atomic(&root.join(HISTORY_FILE), json, None).await
    }

    pub fn latest(&self) -> Option<&HistoryEntry> {
        self.entries.last()
    }

    pub fn entry(&self, id: u32) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Remembers where the pack's downloaded files can be downloaded again.
    pub fn remember_downloads(&mut self, files: &BTreeMap<String, PackFile>) {
        for file in files.values() {
            let Some(download) = &file.download else {
                continue;
            };
            let known = self.files.entry(file.sha1.to_lowercase()).or_default();
            known.size = download.size;
            known.sha512 = Some(download.sha512.clone());
            known.urls = download.urls.iter().map(url::Url::to_string).collect();
        }
    }

    /// Whether the file with `sha1` can be got back without the server's copy.
    fn can_get_back(&self, root: &Path, sha1: &str) -> bool {
        self.files.get(&sha1.to_lowercase()).is_some_and(|known| {
            known.download_urls().is_some()
                || (known.stored && object_path(root, sha1).is_some_and(|p| p.is_file()))
        })
    }

    /// Keeps the file with `sha1` that an update is taking off the server (`from`, in the
    /// update's snapshot), unless it can be downloaded again: moves it into the store.
    /// Reads and moves files, so call it on a blocking thread.
    pub fn keep(&mut self, root: &Path, sha1: &str, from: &Path) -> Result<(), Error> {
        let sha1 = sha1.to_lowercase();
        if self.can_get_back(root, &sha1) {
            return Ok(());
        }
        let Some(object) = object_path(root, &sha1) else {
            return Ok(());
        };
        if !object.is_file() {
            let parent = object.parent().expect("an object path has a folder");
            std::fs::create_dir_all(parent).context("Failed to create the modpack store")?;
            if std::fs::rename(from, &object).is_err() {
                std::fs::copy(from, &object)
                    .with_context(|| format!("Failed to keep {}", from.display()))?;
            }
        }
        let size = std::fs::metadata(&object).map(|m| m.len()).unwrap_or(0);
        let known = self.files.entry(sha1).or_default();
        known.stored = true;
        known.size = size;
        Ok(())
    }

    /// Adds `entry` with the next id, then drops entries beyond [`MAX_ENTRIES`] and the
    /// stored files nothing refers to any more. Deletes files, so call it on a blocking
    /// thread. Returns the new entry's id.
    pub fn push(&mut self, root: &Path, mut entry: HistoryEntry) -> u32 {
        entry.id = self.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let id = entry.id;
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            let extra = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..extra);
        }
        self.prune(root);
        id
    }

    /// Adds the change `plan` made (after its files are placed, before the update's
    /// snapshot is removed): keeps the files it took off the server that can't be
    /// downloaded again, remembers where the new ones come from, and adds `entry`.
    /// Moves and deletes files, so call it on a blocking thread. Returns the entry's id.
    pub fn record_change(
        &mut self,
        root: &Path,
        plan: &FilePlan,
        new_files: &BTreeMap<String, PackFile>,
        entry: HistoryEntry,
    ) -> u32 {
        self.remember_downloads(new_files);
        for change in plan.changes.iter().filter(|c| c.action != FileAction::Add) {
            let Some(sha1) = &change.old_sha1 else {
                continue;
            };
            let snapshot = super::update::snapshot_path(root, &change.target);
            if let Err(e) = self.keep(root, sha1, &snapshot) {
                warn!("Could not keep {} for undoing: {e}", change.target);
            }
        }
        self.push(root, entry)
    }

    /// Forgets files no entry refers to, and deletes their stored copies.
    fn prune(&mut self, root: &Path) {
        let mut used: BTreeSet<String> = BTreeSet::new();
        for entry in &self.entries {
            used.extend(entry.files.values().map(|f| f.sha1.to_lowercase()));
            for change in &entry.changes {
                used.extend(change.old_sha1.iter().map(|s| s.to_lowercase()));
                used.extend(change.new_sha1.iter().map(|s| s.to_lowercase()));
            }
        }
        self.files.retain(|sha1, _| used.contains(sha1));
        let Ok(folders) = std::fs::read_dir(root.join(OBJECTS_DIR)) else {
            return;
        };
        for folder in folders.flatten() {
            let Ok(objects) = std::fs::read_dir(folder.path()) else {
                continue;
            };
            for object in objects.flatten() {
                let name = object.file_name().to_string_lossy().to_lowercase();
                let wanted = self.files.get(&name).is_some_and(|known| known.stored);
                if !wanted {
                    if let Err(e) = std::fs::remove_file(object.path()) {
                        warn!("Could not delete {}: {e}", object.path().display());
                    }
                }
            }
            // an empty folder goes too
            let _ = std::fs::remove_dir(folder.path());
        }
    }

    /// How many bytes the stored copies take.
    pub fn stored_bytes(&self) -> u64 {
        self.files
            .values()
            .filter(|known| known.stored)
            .map(|known| known.size)
            .sum()
    }

    /// For each file of the newest entry, the id of the entry its current version came
    /// with: the oldest entry of the run of newest entries that all had it.
    pub fn blame(&self) -> BTreeMap<String, u32> {
        let Some(latest) = self.latest() else {
            return BTreeMap::new();
        };
        latest
            .files
            .iter()
            .map(|(path, file)| {
                let mut id = latest.id;
                for entry in self.entries.iter().rev().skip(1) {
                    match entry.files.get(path) {
                        Some(f) if f.sha1.eq_ignore_ascii_case(&file.sha1) => id = entry.id,
                        _ => break,
                    }
                }
                (path.clone(), id)
            })
            .collect()
    }

    /// The files of entry `id`, with where each one's bytes come from, for restoring it.
    /// Paths that a pack couldn't write either (Lodestone's own files, and scripts unless
    /// `allow_scripts`) are left out. A file with no source is still listed: it can be
    /// restored only if the server already has it.
    pub fn files_of(
        &self,
        root: &Path,
        id: u32,
        allow_scripts: bool,
    ) -> Option<BTreeMap<String, PackFile>> {
        let entry = self.entry(id)?;
        let policy = InstancePathPolicy { allow_scripts };
        let mut files = BTreeMap::new();
        for (path, recorded) in &entry.files {
            let Some(relative) = record_path(path) else {
                continue;
            };
            let components: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            let components: Vec<&str> = components.iter().map(String::as_str).collect();
            if policy.skip_reason(&relative, &components, false).is_some() {
                continue;
            }
            let sha1 = recorded.sha1.to_lowercase();
            let known = self.files.get(&sha1);
            let stored = known
                .filter(|k| k.stored)
                .and_then(|_| object_path(root, &sha1))
                .filter(|p| p.is_file());
            let download = known.and_then(|k| {
                let (urls, sha512) = k.download_urls()?;
                Some(PackDownload {
                    path: relative.clone(),
                    urls,
                    sha1: sha1.clone(),
                    sha512,
                    size: k.size,
                })
            });
            files.insert(
                path.clone(),
                PackFile {
                    sha1,
                    download,
                    from_overrides: recorded.from_overrides,
                    project_id: recorded.project_id.clone(),
                    stored,
                },
            );
        }
        Some(files)
    }
}

/// An entry for an install, update or restore that `plan` describes, from `old` (the
/// record before it) to the plan's record.
pub fn entry_for(
    kind: HistoryKind,
    origin: ModpackOrigin,
    game_version: String,
    flavour: Flavour,
    old: &PackRecord,
    plan: &FilePlan,
) -> HistoryEntry {
    let changes = plan
        .changes
        .iter()
        .map(|change| {
            let new = plan.record.files.get(&change.path);
            let project_id = new
                .and_then(|f| f.project_id.clone())
                .or_else(|| old.files.get(&change.path)?.project_id.clone());
            HistoryChange {
                path: change.path.clone(),
                target: change.target.clone(),
                action: match change.action {
                    FileAction::Add => ChangeAction::Added,
                    FileAction::Replace => ChangeAction::Replaced,
                    FileAction::Remove => ChangeAction::Removed,
                },
                old_sha1: change.old_sha1.clone(),
                new_sha1: match change.action {
                    FileAction::Remove => None,
                    _ => new.map(|f| f.sha1.clone()),
                },
                project_id,
            }
        })
        .collect();
    HistoryEntry {
        id: 0,
        time: now_millis(),
        kind,
        origin,
        game_version,
        flavour,
        files: plan.record.files.clone(),
        changes,
        kept: plan.kept.clone(),
    }
}

/// An entry for a server that already has `record`, with nothing changed: the first
/// entry of a new server, or the baseline of a server made before histories.
pub fn entry_as_is(
    kind: HistoryKind,
    origin: ModpackOrigin,
    game_version: String,
    flavour: Flavour,
    record: &PackRecord,
) -> HistoryEntry {
    HistoryEntry {
        id: 0,
        time: now_millis(),
        kind,
        origin,
        game_version,
        flavour,
        files: record.files.clone(),
        changes: Vec::new(),
        kept: Vec::new(),
    }
}

// ---- what the dashboard shows ----

/// A change as the dashboard shows it. A removed and an added file of the same Modrinth
/// project are one `updated` change, from `old_path` to `path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ChangeKind {
    Added,
    Updated,
    Replaced,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChangeInfo {
    pub kind: ChangeKind,
    /// The file's path on disk after the change (before it, for a removed file).
    pub path: String,
    /// The old path of an updated file.
    pub old_path: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryEntryInfo {
    pub id: u32,
    /// Unix milliseconds.
    #[ts(type = "number")]
    pub time: i64,
    pub kind: HistoryKind,
    pub name: String,
    pub version_number: Option<String>,
    pub game_version: String,
    /// `fabric`, `forge` or `neoforge`.
    pub loader: String,
    pub loader_version: Option<String>,
    pub changes: Vec<ChangeInfo>,
    pub kept: Vec<String>,
    pub file_count: u32,
    /// The server is at this entry.
    pub current: bool,
    /// How many of its files Lodestone can't get back: it can only be restored when
    /// this is 0.
    pub missing: u32,
}

/// `GET /instance/{uuid}/modpack/history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackHistory {
    /// Newest first.
    pub entries: Vec<HistoryEntryInfo>,
    /// For each file of the current pack (its path in the pack, with `/`), the id of the
    /// entry its current version came with.
    pub blame: BTreeMap<String, u32>,
    /// The bytes the stored copies take.
    #[ts(type = "number")]
    pub stored_bytes: u64,
    /// A newer version of the modpack on Modrinth, when the last check found one.
    pub update: Option<super::updates::AvailableUpdate>,
}

/// The changes of `entry`, with a removed and an added file of one project paired.
fn change_infos(entry: &HistoryEntry) -> Vec<ChangeInfo> {
    let mut removed_by_project: HashMap<&str, &HistoryChange> = HashMap::new();
    for change in &entry.changes {
        if let (ChangeAction::Removed, Some(project)) = (change.action, &change.project_id) {
            removed_by_project.insert(project, change);
        }
    }
    let mut paired: BTreeSet<&str> = BTreeSet::new();
    let mut infos: Vec<ChangeInfo> = Vec::new();
    for change in &entry.changes {
        let info = match change.action {
            ChangeAction::Added => {
                let old = change
                    .project_id
                    .as_deref()
                    .and_then(|p| removed_by_project.get(p));
                match old {
                    Some(old) => {
                        paired.insert(old.target.as_str());
                        ChangeInfo {
                            kind: ChangeKind::Updated,
                            path: change.target.clone(),
                            old_path: Some(old.target.clone()),
                            project_id: change.project_id.clone(),
                        }
                    }
                    None => ChangeInfo {
                        kind: ChangeKind::Added,
                        path: change.target.clone(),
                        old_path: None,
                        project_id: change.project_id.clone(),
                    },
                }
            }
            ChangeAction::Replaced => ChangeInfo {
                kind: ChangeKind::Replaced,
                path: change.target.clone(),
                old_path: None,
                project_id: change.project_id.clone(),
            },
            ChangeAction::Removed => continue,
        };
        infos.push(info);
    }
    for change in &entry.changes {
        if change.action == ChangeAction::Removed && !paired.contains(change.target.as_str()) {
            infos.push(ChangeInfo {
                kind: ChangeKind::Removed,
                path: change.target.clone(),
                old_path: None,
                project_id: change.project_id.clone(),
            });
        }
    }
    infos.sort_by(|a, b| (a.kind as u8, &a.path).cmp(&(b.kind as u8, &b.path)));
    infos
}

impl History {
    /// What the dashboard shows. `root` is used to check which stored copies are there.
    pub fn info(
        &self,
        root: &Path,
        update: Option<super::updates::AvailableUpdate>,
    ) -> ModpackHistory {
        let current = self.latest();
        let entries = self
            .entries
            .iter()
            .rev()
            .map(|entry| {
                let missing = entry
                    .files
                    .iter()
                    .filter(|(path, file)| {
                        let on_server = current
                            .and_then(|c| c.files.get(*path))
                            .is_some_and(|c| c.sha1.eq_ignore_ascii_case(&file.sha1));
                        !on_server && !self.can_get_back(root, &file.sha1)
                    })
                    .count() as u32;
                HistoryEntryInfo {
                    id: entry.id,
                    time: entry.time,
                    kind: entry.kind.clone(),
                    name: entry.origin.name.clone(),
                    version_number: entry.origin.version_number.clone(),
                    game_version: entry.game_version.clone(),
                    loader: entry.flavour.to_string(),
                    loader_version: entry.flavour.loader_version(),
                    changes: change_infos(entry),
                    kept: entry.kept.clone(),
                    file_count: entry.files.len() as u32,
                    current: current.is_some_and(|c| c.id == entry.id),
                    missing,
                }
            })
            .collect();
        ModpackHistory {
            entries,
            blame: self.blame(),
            stored_bytes: self.stored_bytes(),
            update,
        }
    }
}

#[cfg(test)]
mod tests;
