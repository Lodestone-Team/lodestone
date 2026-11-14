//! Updating a server to another version of its modpack.
//!
//! [`RECORD_FILE`] records every file the pack wrote, with its SHA-1. Comparing that, the new pack and the files on disk gives the plan, which the
//! dashboard shows before anything changes:
//!
//! - **The pack's downloaded files** (mods): a file the new pack lists is added when
//!   missing, and replaced when the old pack wrote it and it is unchanged, or when the old
//!   pack didn't write it (a mod the user added by hand at the same path). A file the new
//!   pack no longer lists is removed when unchanged. A pack file the user changed is left
//!   as it is. A disabled mod (`.jar.disabled`) stays disabled, also when its new version
//!   has another file name (the same Modrinth project). A mod the pack adds replaces
//!   another version of it on the server that the record doesn't list (one updated in the
//!   Mods tab under a new file name, or added by hand), found by asking Modrinth: two
//!   copies of a mod stop the loader from starting.
//! - **Overrides** (configs and bundled files): written when missing, or when the old
//!   pack wrote them and they are unchanged; never over the user's own files or changes.
//!   Overrides the new pack dropped are removed only from `mods/` and `plugins/`, and only
//!   when unchanged; configs stay.
//! - **Everything else** (worlds, the user's own mods, server.properties) is never
//!   touched.
//!
//! Applying it downloads and extracts every new file into a staging folder first, then
//! moves what will be replaced or removed into a snapshot folder, and only then moves the
//! new files into place. If that fails part way, the files are put back from the
//! snapshot. Without a record (a server made before records were kept), the old pack's
//! files are worked out from the old pack itself when it is on Modrinth.
//!
//! Every install, update and restore is kept in the history ([`super::history`]), which
//! is what makes an update undoable: restoring an earlier entry is an update whose new
//! files come from the history rather than from a pack.
//!
//! Lodestone's files are all in [`MODPACK_DIR`], which packs can't write (their paths
//! may not start with `.lodestone`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use super::{
    download_one, extract_overrides, override_hashes, prepare_parent, slash_path, Pack,
    PackDownload, PARALLEL_DOWNLOADS,
};
use crate::error::Error;
use crate::modrinth::ModrinthApi;
use crate::mods::{disabled_name, is_disabled_name};

/// The folder, in the instance folder, of the modpack's record, history and stored files.
pub const MODPACK_DIR: &str = ".lodestone_modpack";
/// The record of the files the pack wrote.
pub const RECORD_FILE: &str = ".lodestone_modpack/record.json";
/// Where new files are downloaded while an update runs.
pub(super) const STAGING_DIR: &str = ".lodestone_modpack/staging";
/// Where replaced and removed files are kept while an update runs.
pub(super) const SNAPSHOT_DIR: &str = ".lodestone_modpack/snapshot";
/// The update being applied: written before it moves any file, and removed once its
/// record is saved. See [`begin_update`].
pub(super) const JOURNAL_FILE: &str = ".lodestone_modpack/update.json";
/// The folders a dropped override is removed from.
const CONTENT_DIRS: [&str; 2] = ["mods/", "plugins/"];

/// One file of a pack, as the record keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedFile {
    pub sha1: String,
    /// From the pack's overrides rather than its downloads.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub from_overrides: bool,
    /// The Modrinth project of a downloaded file, from its CDN URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// [`RECORD_FILE`]: the files a pack wrote, by path with `/` separators.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PackRecord {
    pub files: BTreeMap<String, RecordedFile>,
}

impl PackRecord {
    /// The record in `root`, or `None` when there is none or it can't be read.
    pub async fn load(root: &Path) -> Option<PackRecord> {
        let bytes = tokio::fs::read(root.join(RECORD_FILE)).await.ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(record) => Some(record),
            Err(e) => {
                warn!("Ignoring {RECORD_FILE}, which can't be read: {e}");
                None
            }
        }
    }

    pub async fn save(&self, root: &Path) -> Result<(), Error> {
        let json = serde_json::to_vec_pretty(self).context("Failed to write the modpack record")?;
        tokio::fs::create_dir_all(root.join(MODPACK_DIR))
            .await
            .context("Failed to create the modpack folder")?;
        crate::persist::write_atomic(&root.join(RECORD_FILE), json, None).await
    }
}

/// A file a pack writes, with where its bytes come from: a stored copy in the history,
/// a download, or else the pack's overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct PackFile {
    pub sha1: String,
    pub download: Option<PackDownload>,
    /// From the pack's overrides rather than its downloads.
    pub from_overrides: bool,
    pub project_id: Option<String>,
    /// A copy of the file in the history's store.
    pub stored: Option<PathBuf>,
}

/// The Modrinth project of a `https://cdn.modrinth.com/data/<project>/versions/...` URL.
fn project_of(download: &PackDownload) -> Option<String> {
    download.urls.iter().find_map(|url| {
        if url.host_str() != Some(crate::modrinth::CDN_HOST) {
            return None;
        }
        let mut segments = url.path_segments()?;
        (segments.next()? == "data").then_some(())?;
        let project = segments.next()?;
        (segments.next()? == "versions").then(|| project.to_string())
    })
}

/// Every file `pack` writes into a server, by path with `/` separators. Reads the pack's
/// overrides, so call it on a blocking thread.
pub fn pack_files(
    pack: &Pack,
    pack_path: &Path,
    allow_scripts: bool,
) -> Result<BTreeMap<String, PackFile>, Error> {
    let mut files = BTreeMap::new();
    for download in &pack.downloads {
        files.insert(
            slash_path(&download.path),
            PackFile {
                sha1: download.sha1.to_lowercase(),
                download: Some(download.clone()),
                from_overrides: false,
                project_id: project_of(download),
                stored: None,
            },
        );
    }
    // an override at the same path as a download is written after it
    for (path, sha1) in override_hashes(pack_path, allow_scripts)? {
        files.insert(
            path,
            PackFile {
                sha1,
                download: None,
                from_overrides: true,
                project_id: None,
                stored: None,
            },
        );
    }
    Ok(files)
}

/// The record of a pack's files once they are written.
pub fn record_of(files: &BTreeMap<String, PackFile>) -> PackRecord {
    PackRecord {
        files: files
            .iter()
            .map(|(path, file)| {
                (
                    path.clone(),
                    RecordedFile {
                        sha1: file.sha1.clone(),
                        from_overrides: file.from_overrides,
                        project_id: file.project_id.clone(),
                    },
                )
            })
            .collect(),
    }
}

/// `path` from a record, if it is a plain relative path a pack could have written: no
/// `..`, root or drive, not one of Lodestone's own files, and not one of the files at the
/// root that no pack may write (such as `server.jar`). Names are compared the way Windows
/// and macOS resolve them, and names that may point at another file there (an alternate
/// data stream, an 8.3 short name) are refused. The record and the history are files in
/// the instance folder, so an update must not trust them to name only pack files.
pub(super) fn record_path(path: &str) -> Option<PathBuf> {
    let relative = PathBuf::from(path);
    let names: Vec<String> = relative
        .components()
        .map(|c| {
            c.as_os_str()
                .to_string_lossy()
                .trim_end_matches(['.', ' '])
                .to_lowercase()
        })
        .collect();
    let looks_like_short_name = |name: &str| {
        let stem = name.split('.').next().unwrap_or(name);
        stem.rsplit_once('~')
            .is_some_and(|(_, tail)| tail.starts_with(|c: char| c.is_ascii_digit()))
    };
    let plain = !path.is_empty()
        && relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !names.iter().any(|name| {
            name.is_empty()
                || name.starts_with(".lodestone")
                || name.contains(':')
                || looks_like_short_name(name)
        })
        && !(names.len() == 1 && super::PROTECTED_ROOT_FILES.contains(&names[0].as_str()));
    plain.then_some(relative)
}

/// What an update does to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    Add,
    Replace,
    Remove,
}

/// One change of an update: the pack's path, and the path on disk it changes (the same,
/// or the disabled name of a disabled mod), with the SHA-1 of the file it replaces or
/// removes.
#[derive(Debug, Clone, PartialEq)]
pub struct FileChange {
    pub path: String,
    pub target: String,
    pub action: FileAction,
    pub old_sha1: Option<String>,
}

/// What an update will do to the files.
#[derive(Debug, Clone, Default)]
pub struct FilePlan {
    pub changes: Vec<FileChange>,
    /// Pack files the user changed (or their own files at a pack's override path), which
    /// are left as they are.
    pub kept: Vec<String>,
    /// The record after the update.
    pub record: PackRecord,
}

impl FilePlan {
    pub fn paths(&self, action: FileAction) -> Vec<String> {
        self.changes
            .iter()
            .filter(|c| c.action == action)
            .map(|c| c.target.clone())
            .collect()
    }
}

/// A file on disk at a pack path: the path itself, or its disabled name for a mod.
/// Returns the path found and its SHA-1. Links are not followed.
pub(super) fn on_disk(root: &Path, path: &str) -> Option<(String, String)> {
    let mut candidates = vec![path.to_string()];
    if path.to_lowercase().ends_with(".jar") {
        candidates.push(disabled_name(path));
    }
    candidates.into_iter().find_map(|candidate| {
        let full = root.join(&candidate);
        let metadata = std::fs::symlink_metadata(&full).ok()?;
        if !metadata.is_file() {
            return None;
        }
        let sha1 = lodestone_minecraft::hash::sha1_of(&full).ok()?;
        Some((candidate, sha1))
    })
}

/// Whether `relative` (with `/` separators) is in `root` under exactly this name, every
/// part of it spelled as the folder lists it. On a disk that ignores case (macOS, Windows)
/// a path can lead to a file whose name differs in case; this tells them apart.
fn exists_exactly(root: &Path, relative: &str) -> bool {
    let mut dir = root.to_path_buf();
    for part in Path::new(relative).components() {
        let Component::Normal(name) = part else {
            return false;
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return false;
        };
        if !entries.flatten().any(|entry| entry.file_name() == name) {
            return false;
        }
        dir.push(name);
    }
    true
}

/// The old record's paths that the new pack lists again with only their case changed, by
/// the new path, when both lead to the same file (a disk that ignores case). Removing the
/// old path would remove the new file too.
fn renamed_by_case<'a>(
    root: &Path,
    old: &'a PackRecord,
    new: &'a BTreeMap<String, PackFile>,
) -> BTreeMap<&'a str, &'a str> {
    let new_by_lower: std::collections::HashMap<String, &String> =
        new.keys().map(|path| (path.to_lowercase(), path)).collect();
    old.files
        .keys()
        .filter(|path| !new.contains_key(*path) && record_path(path).is_some())
        .filter_map(|old_path| {
            let new_path = new_by_lower.get(&old_path.to_lowercase())?;
            let (old_target, _) = on_disk(root, old_path)?;
            let (new_target, _) = on_disk(root, new_path)?;
            let same = new_target.to_lowercase() == old_target.to_lowercase()
                && !exists_exactly(root, &new_target);
            same.then_some((new_path.as_str(), old_path.as_str()))
        })
        .collect()
}

/// Compares the old pack's record, the new pack's files and the files in `root`. Reads
/// files, so call it on a blocking thread.
pub fn plan_files(root: &Path, old: &PackRecord, new: &BTreeMap<String, PackFile>) -> FilePlan {
    let mut plan = FilePlan {
        record: record_of(new),
        ..Default::default()
    };
    let renamed = renamed_by_case(root, old, new);
    let mut disabled_projects = BTreeSet::new();
    for (path, recorded) in &old.files {
        if new.contains_key(path)
            || record_path(path).is_none()
            || renamed.values().any(|old_path| *old_path == path)
        {
            continue;
        }
        let Some((target, sha1)) = on_disk(root, path) else {
            continue;
        };
        if sha1 != recorded.sha1 {
            plan.kept.push(target);
            continue;
        }
        let removable =
            !recorded.from_overrides || CONTENT_DIRS.iter().any(|dir| path.starts_with(dir));
        if !removable {
            continue;
        }
        if is_disabled_name(&target) {
            if let Some(project) = &recorded.project_id {
                disabled_projects.insert(project.clone());
            }
        }
        plan.changes.push(FileChange {
            path: path.clone(),
            target,
            action: FileAction::Remove,
            old_sha1: Some(sha1),
        });
    }
    for (path, file) in new {
        match on_disk(root, path) {
            None => {
                let disabled = path.to_lowercase().ends_with(".jar")
                    && file
                        .project_id
                        .as_ref()
                        .is_some_and(|p| disabled_projects.contains(p));
                plan.changes.push(FileChange {
                    path: path.clone(),
                    target: if disabled {
                        disabled_name(path)
                    } else {
                        path.clone()
                    },
                    action: FileAction::Add,
                    old_sha1: None,
                });
            }
            Some((_, sha1)) if sha1 == file.sha1 => {}
            Some((target, sha1)) => {
                let recorded = old.files.get(path).or_else(|| {
                    renamed
                        .get(path.as_str())
                        .and_then(|old_path| old.files.get(*old_path))
                });
                let replace = match recorded {
                    Some(recorded) => recorded.sha1 == sha1,
                    // a file the old pack didn't write: a mod at the pack's path is
                    // replaced, a config the user wrote is theirs
                    None => !file.from_overrides,
                };
                if replace {
                    plan.changes.push(FileChange {
                        path: path.clone(),
                        target,
                        action: FileAction::Replace,
                        old_sha1: Some(sha1),
                    });
                } else {
                    plan.kept.push(target);
                }
            }
        }
    }
    plan.kept.sort();
    plan
}

/// Keeps in the plan's record the old pack's scripts that `new` leaves out because the
/// person updating may not add scripts (the pack's files leave scripts out then). Those
/// scripts stay on the server as they are, so the record must still list them as the
/// pack's: otherwise a later update by the owner takes them for the user's own changes
/// and never updates them. A script the plan removes isn't kept.
pub fn keep_unwritten_scripts(
    plan: &mut FilePlan,
    old: &PackRecord,
    new: &BTreeMap<String, PackFile>,
) {
    for (path, recorded) in &old.files {
        let removed = plan
            .changes
            .iter()
            .any(|c| c.action == FileAction::Remove && c.path == *path);
        if !new.contains_key(path)
            && !removed
            && crate::handlers::util::is_script_path(Path::new(path))
        {
            plan.record.files.insert(path.clone(), recorded.clone());
        }
    }
}

/// A mod on the server that neither the old record nor the new pack lists (one the user
/// added, or updated in the Mods tab under a new file name), with the Modrinth project it
/// belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct OtherCopy {
    /// Its path on disk, with `/` separators: `mods/x.jar` or `mods/x.jar.disabled`.
    pub target: String,
    pub sha1: String,
    pub project_id: String,
}

/// The jars in the folders `plan` adds jars to that neither `old` nor `new` lists and
/// `plan` doesn't change, with their SHA-1 and SHA-512. Links are not followed. Reads
/// files, so call it on a blocking thread.
fn unlisted_jars(
    root: &Path,
    old: &PackRecord,
    new: &BTreeMap<String, PackFile>,
    plan: &FilePlan,
) -> Vec<(String, String, String)> {
    let listed = |target: &str| {
        let path = crate::mods::enabled_name(target);
        [path.as_str(), target].iter().any(|p| {
            old.files.contains_key(*p)
                || new.contains_key(*p)
                || plan.changes.iter().any(|c| c.target == *p || c.path == *p)
        })
    };
    let folders: BTreeSet<&str> = plan
        .changes
        .iter()
        .filter(|c| c.action == FileAction::Add && c.path.to_lowercase().ends_with(".jar"))
        .filter_map(|c| c.path.rsplit_once('/').map(|(folder, _)| folder))
        .collect();
    let mut jars = Vec::new();
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let is_file = entry.file_type().is_ok_and(|t| t.is_file());
            let target = format!("{folder}/{name}");
            if !is_file || !crate::mods::is_mod_file_name(&name) || listed(&target) {
                continue;
            }
            let path = entry.path();
            let (Ok(sha1), Ok(sha512)) = (
                lodestone_minecraft::hash::sha1_of(&path),
                lodestone_minecraft::hash::sha512_of(&path),
            ) else {
                continue;
            };
            jars.push((target, sha1, sha512));
        }
    }
    jars
}

/// The [`OtherCopy`]s of projects whose mods `plan` adds: the unlisted jars in those
/// folders that Modrinth says belong to such a project. Empty when there are none, or
/// Modrinth can't be reached (the plan then adds the mod next to them, as before).
pub async fn other_copies(
    api: &dyn ModrinthApi,
    root: &Path,
    old: &PackRecord,
    new: &std::sync::Arc<BTreeMap<String, PackFile>>,
    plan: &FilePlan,
) -> Vec<OtherCopy> {
    let added: BTreeSet<String> = plan
        .changes
        .iter()
        .filter(|c| c.action == FileAction::Add)
        .filter_map(|c| new.get(&c.path)?.project_id.clone())
        .collect();
    if added.is_empty() {
        return Vec::new();
    }
    let jars = {
        let (root, old, new, plan) = (root.to_path_buf(), old.clone(), new.clone(), plan.clone());
        match tokio::task::spawn_blocking(move || unlisted_jars(&root, &old, &new, &plan)).await {
            Ok(jars) => jars,
            Err(e) => {
                warn!("Could not look for other copies of the modpack's mods: {e}");
                return Vec::new();
            }
        }
    };
    if jars.is_empty() {
        return Vec::new();
    }
    let hashes: Vec<String> = jars.iter().map(|(_, _, sha512)| sha512.clone()).collect();
    let versions = match api.versions_by_hash(&hashes).await {
        Ok(versions) => versions,
        Err(e) => {
            warn!(
                "Could not look up the server's other mods on Modrinth: {}",
                e.source
            );
            return Vec::new();
        }
    };
    jars.into_iter()
        .filter_map(|(target, sha1, sha512)| {
            let project_id = versions.get(&sha512)?.project_id.clone();
            added.contains(&project_id).then_some(OtherCopy {
                target,
                sha1,
                project_id,
            })
        })
        .collect()
}

/// Plans the removal of `others` whose project `plan` adds a mod of, as the Mods tab
/// replaces the old version of a mod it updates: two copies of a mod stop the loader from
/// starting. When every other copy of the project is disabled, the new mod is added
/// disabled, as a disabled mod's new version stays disabled.
pub fn remove_other_copies(
    plan: &mut FilePlan,
    new: &BTreeMap<String, PackFile>,
    others: &[OtherCopy],
) {
    let mut removals = Vec::new();
    for change in plan
        .changes
        .iter_mut()
        .filter(|c| c.action == FileAction::Add)
    {
        let Some(project) = new.get(&change.path).and_then(|f| f.project_id.as_ref()) else {
            continue;
        };
        let copies: Vec<&OtherCopy> = others.iter().filter(|o| &o.project_id == project).collect();
        if copies.is_empty() {
            continue;
        }
        if change.path.to_lowercase().ends_with(".jar")
            && copies.iter().all(|o| is_disabled_name(&o.target))
        {
            change.target = disabled_name(&change.path);
        }
        removals.extend(copies.into_iter().cloned());
    }
    for other in removals {
        if plan.changes.iter().any(|c| c.target == other.target) {
            continue;
        }
        plan.changes.push(FileChange {
            path: crate::mods::enabled_name(&other.target),
            target: other.target,
            action: FileAction::Remove,
            old_sha1: Some(other.sha1),
        });
    }
}

/// Whether `path`'s folder is inside `root` once links are resolved.
fn parent_inside(root: &Path, path: &Path) -> bool {
    let (Some(parent), Ok(root)) = (path.parent(), std::fs::canonicalize(root)) else {
        return false;
    };
    std::fs::canonicalize(parent).is_ok_and(|parent| parent.starts_with(root))
}

/// Moves `from` to `to`, creating `to`'s folders. Both folders must be inside `root` once
/// links are resolved (a linked `mods/` could lead elsewhere), and neither file may be a
/// link.
fn move_file(root: &Path, from: &Path, to: &Path) -> Result<(), Error> {
    let is_link =
        |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if !parent_inside(root, from) || is_link(from) {
        return Err(super::unsafe_pack("a path leaves the server folder"));
    }
    prepare_parent(root, to)?;
    if is_link(to) {
        return Err(super::unsafe_pack("a path leaves the server folder"));
    }
    std::fs::rename(from, to)
        .with_context(|| format!("Failed to move {} to {}", from.display(), to.display()))?;
    Ok(())
}

/// The file moves done so far, to undo them.
#[derive(Default)]
struct Moves {
    /// Targets moved into the snapshot.
    saved: Vec<String>,
    /// Targets new files were moved to.
    placed: Vec<String>,
}

impl Moves {
    /// Puts the server's files back as they were.
    fn undo(&self, root: &Path) {
        let snapshot = root.join(SNAPSHOT_DIR);
        for target in self.placed.iter().rev() {
            if let Err(e) = std::fs::remove_file(root.join(target)) {
                warn!("Could not remove {target} while undoing a modpack update: {e}");
            }
        }
        for target in self.saved.iter().rev() {
            if let Err(e) = move_file(root, &snapshot.join(target), &root.join(target)) {
                warn!("Could not put back {target} while undoing a modpack update: {e}");
            }
        }
    }
}

/// Where an update keeps the file it took from `target` until it is done.
pub fn snapshot_path(root: &Path, target: &str) -> PathBuf {
    root.join(SNAPSHOT_DIR).join(target)
}

/// Removes the update's staging and snapshot folders, if they are there. The snapshot
/// of an update that isn't finished ([`JOURNAL_FILE`] is there) is kept: it may hold the
/// only copy of the files the update replaced, which [`recover_interrupted`] puts back.
pub async fn remove_work_dirs(root: &Path) {
    let unfinished = tokio::fs::symlink_metadata(root.join(JOURNAL_FILE))
        .await
        .is_ok();
    for dir in [STAGING_DIR, SNAPSHOT_DIR] {
        if unfinished && dir == SNAPSHOT_DIR {
            warn!(
                "Keeping {} for the modpack update that didn't finish",
                root.join(dir).display()
            );
            continue;
        }
        let path = root.join(dir);
        if tokio::fs::symlink_metadata(&path).await.is_ok() {
            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                warn!("Could not remove {}: {e}", path.display());
            }
        }
    }
}

/// Copies `stored` to `path` in the staging folder, checking that it is still the file
/// with `sha1`.
fn copy_stored(staging: &Path, path: &str, stored: &Path, sha1: &str) -> Result<(), Error> {
    let target = staging.join(path);
    prepare_parent(staging, &target)?;
    std::fs::copy(stored, &target)
        .with_context(|| format!("Failed to copy the stored copy of {path}"))?;
    let copied = lodestone_minecraft::hash::sha1_of(&target)
        .with_context(|| format!("Failed to read the stored copy of {path}"))?;
    if !copied.eq_ignore_ascii_case(sha1) {
        return Err(eyre!("The stored copy of {path} is damaged").into());
    }
    Ok(())
}

/// Puts the new files of `plan` into the staging folder: stored copies from the history,
/// then downloads, then the overrides of the pack at `pack_path` (when there is one).
pub async fn stage_files(
    api: &dyn ModrinthApi,
    pack_path: Option<&Path>,
    new: &BTreeMap<String, PackFile>,
    plan: &FilePlan,
    root: &Path,
    allow_scripts: bool,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), Error> {
    use futures::TryStreamExt;
    remove_work_dirs(root).await;
    let staging = root.join(STAGING_DIR);
    tokio::fs::create_dir_all(&staging)
        .await
        .context("Failed to create the modpack update folder")?;
    let wanted: Vec<(&String, &PackFile)> = plan
        .changes
        .iter()
        .filter(|c| c.action != FileAction::Remove)
        .filter_map(|c| Some((&c.path, new.get(&c.path)?)))
        .collect();
    let stored: Vec<(String, PathBuf, String)> = wanted
        .iter()
        .filter_map(|(path, file)| Some(((*path).clone(), file.stored.clone()?, file.sha1.clone())))
        .collect();
    if !stored.is_empty() {
        progress("Copying stored modpack files".to_string());
        let staging = staging.clone();
        tokio::task::spawn_blocking(move || {
            stored
                .iter()
                .try_for_each(|(path, from, sha1)| copy_stored(&staging, path, from, sha1))
        })
        .await
        .map_err(|e| eyre!("The modpack task failed: {e}"))??;
    }
    let downloads: Vec<&PackDownload> = wanted
        .iter()
        .filter(|(_, file)| file.stored.is_none())
        .filter_map(|(_, file)| file.download.as_ref())
        .collect();
    let total = downloads.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    futures::stream::iter(downloads.into_iter().map(Ok::<_, Error>))
        .try_for_each_concurrent(PARALLEL_DOWNLOADS, |download| {
            let (done, staging) = (&done, &staging);
            async move {
                download_one(api, staging, download).await?;
                let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                progress(format!("Downloading modpack files ({n}/{total})"));
                Ok(())
            }
        })
        .await?;
    let needs_overrides = wanted
        .iter()
        .any(|(_, file)| file.stored.is_none() && file.download.is_none());
    if needs_overrides {
        let pack_path =
            pack_path.ok_or_else(|| eyre!("Some files of this version are no longer available"))?;
        progress("Extracting modpack overrides".to_string());
        let (pack_path, staging) = (pack_path.to_path_buf(), staging.clone());
        tokio::task::spawn_blocking(move || extract_overrides(&pack_path, &staging, allow_scripts))
            .await
            .map_err(|e| eyre!("The modpack task failed: {e}"))??;
    }
    Ok(())
}

/// Moves the staged files into place, after moving what they replace and what is
/// removed into the snapshot folder. On failure every move is undone. The staging and
/// snapshot folders are left for [`remove_work_dirs`].
pub fn place_files(root: &Path, plan: &FilePlan) -> Result<(), Error> {
    let staging = root.join(STAGING_DIR);
    let snapshot = root.join(SNAPSHOT_DIR);
    let mut moves = Moves::default();
    let result = (|| {
        for change in &plan.changes {
            if change.action == FileAction::Add {
                continue;
            }
            move_file(
                root,
                &root.join(&change.target),
                &snapshot.join(&change.target),
            )?;
            moves.saved.push(change.target.clone());
        }
        for change in &plan.changes {
            if change.action == FileAction::Remove {
                continue;
            }
            let staged = staging.join(&change.path);
            if !staged.is_file() {
                return Err(eyre!("{} was not downloaded", change.path).into());
            }
            move_file(root, &staged, &root.join(&change.target))?;
            moves.placed.push(change.target.clone());
        }
        Ok::<_, Error>(())
    })();
    if result.is_err() {
        moves.undo(root);
    }
    result
}

/// Puts back the files [`place_files`] changed, from the snapshot folder: for when a later
/// step of the update fails.
pub fn undo_placed_files(root: &Path, plan: &FilePlan) {
    let moves = Moves {
        saved: plan
            .changes
            .iter()
            .filter(|c| c.action != FileAction::Add)
            .map(|c| c.target.clone())
            .collect(),
        placed: plan
            .changes
            .iter()
            .filter(|c| c.action != FileAction::Remove)
            .map(|c| c.target.clone())
            .collect(),
    };
    moves.undo(root);
}

/// One change of the update in [`JOURNAL_FILE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct JournalChange {
    target: String,
    action: FileAction,
    /// The SHA-1 of the file the update puts at `target` (an add or a replace).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    new_sha1: Option<String>,
}

/// [`JOURNAL_FILE`]: what an update changes, and the record it saves when it is done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Journal {
    changes: Vec<JournalChange>,
    record: PackRecord,
}

/// Writes [`JOURNAL_FILE`] for `plan`, before [`place_files`] moves anything. An update
/// moves the files it replaces into the snapshot folder, then may reinstall the server
/// software for minutes before it saves the record and the history. If the core stops in
/// between (quit, a restart to update, a crash), the journal is how the next start knows
/// to put the old files back ([`recover_interrupted`]); without it the next update would
/// take the new files for the user's changes and delete the snapshot with the old ones.
/// Writes a file, so call it on a blocking thread.
pub fn begin_update(root: &Path, plan: &FilePlan) -> Result<(), Error> {
    let journal = Journal {
        changes: plan
            .changes
            .iter()
            .map(|c| JournalChange {
                target: c.target.clone(),
                action: c.action,
                new_sha1: match c.action {
                    FileAction::Remove => None,
                    _ => plan.record.files.get(&c.path).map(|f| f.sha1.clone()),
                },
            })
            .collect(),
        record: plan.record.clone(),
    };
    let json = serde_json::to_vec_pretty(&journal).context("Failed to write the update journal")?;
    std::fs::create_dir_all(root.join(MODPACK_DIR))
        .context("Failed to create the modpack folder")?;
    crate::persist::write_atomic_blocking(&root.join(JOURNAL_FILE), &json, None)
}

/// Removes [`JOURNAL_FILE`]: the update is done, or its files are back as they were.
pub fn end_update(root: &Path) -> Result<(), Error> {
    match std::fs::remove_file(root.join(JOURNAL_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(eyre!("Failed to remove {JOURNAL_FILE}: {e}").into()),
    }
}

/// Finishes with an update that was stopped before it removed [`JOURNAL_FILE`]. When it
/// hadn't saved its record, the files it placed are removed and the ones it moved into
/// the snapshot are put back, so the server's files match the record again (as when an
/// update fails). Files left in the snapshot after that (an update that saved its record
/// but stopped before it finished, or a file that couldn't be put back) are moved to an
/// `interrupted-*` folder next to it rather than deleted. Call it before anything starts
/// the server or another update; returns whether there was such an update. Moves files,
/// so call it on a blocking thread.
pub fn recover_interrupted(root: &Path) -> bool {
    let journal_path = root.join(JOURNAL_FILE);
    let Ok(bytes) = std::fs::read(&journal_path) else {
        return false;
    };
    match serde_json::from_slice::<Journal>(&bytes) {
        Ok(journal) => {
            let saved = std::fs::read(root.join(RECORD_FILE))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<PackRecord>(&bytes).ok());
            if saved.as_ref() == Some(&journal.record) {
                warn!(
                    "A modpack update of {} stopped after saving its record; keeping its files",
                    root.display()
                );
            } else {
                warn!(
                    "A modpack update of {} was interrupted; putting the server's files back",
                    root.display()
                );
                roll_back(root, &journal);
            }
        }
        Err(e) => warn!(
            "{} can't be read ({e}); the interrupted modpack update's files are left as they are",
            journal_path.display()
        ),
    }
    keep_leftover_snapshot(root);
    let staging = root.join(STAGING_DIR);
    if std::fs::symlink_metadata(&staging).is_ok() {
        if let Err(e) = std::fs::remove_dir_all(&staging) {
            warn!("Could not remove {}: {e}", staging.display());
        }
    }
    if let Err(e) = end_update(root) {
        warn!("{}", e.source);
    }
    true
}

/// Puts back the files of the update in `journal`, which may have stopped at any point of
/// [`place_files`] or after it: a file is removed only when it is the one the update
/// placed (its SHA-1 is the new one), and put back from the snapshot only when it is there
/// and nothing took its place.
fn roll_back(root: &Path, journal: &Journal) {
    // the journal is Lodestone's own file, but a path that leads elsewhere isn't followed
    let changes: Vec<&JournalChange> = journal
        .changes
        .iter()
        .filter(|c| record_path(&c.target).is_some())
        .collect();
    for change in changes.iter().filter(|c| c.action != FileAction::Remove) {
        let target = root.join(&change.target);
        let placed = std::fs::symlink_metadata(&target).is_ok_and(|m| m.is_file())
            && parent_inside(root, &target)
            && change.new_sha1.as_ref().is_some_and(|new| {
                lodestone_minecraft::hash::sha1_of(&target)
                    .is_ok_and(|sha1| sha1.eq_ignore_ascii_case(new))
            });
        if placed {
            if let Err(e) = std::fs::remove_file(&target) {
                warn!(
                    "Could not remove {} while undoing a modpack update: {e}",
                    change.target
                );
            }
        }
    }
    for change in changes.iter().filter(|c| c.action != FileAction::Add) {
        let saved = snapshot_path(root, &change.target);
        if !std::fs::symlink_metadata(&saved).is_ok_and(|m| m.is_file()) {
            continue;
        }
        let target = root.join(&change.target);
        if std::fs::symlink_metadata(&target).is_ok() {
            warn!(
                "Not putting back {}, which is there again; the old copy is kept",
                change.target
            );
            continue;
        }
        if let Err(e) = move_file(root, &saved, &target) {
            warn!(
                "Could not put back {} while undoing a modpack update: {e}",
                change.target
            );
        }
    }
}

/// Moves the snapshot folder to `interrupted-<time>` next to it when it still has files,
/// so the next update doesn't delete them, and removes it otherwise.
fn keep_leftover_snapshot(root: &Path) {
    let snapshot = root.join(SNAPSHOT_DIR);
    if std::fs::symlink_metadata(&snapshot).is_err() {
        return;
    }
    let has_files = walkdir::WalkDir::new(&snapshot)
        .into_iter()
        .flatten()
        .any(|entry| !entry.file_type().is_dir());
    if !has_files {
        if let Err(e) = std::fs::remove_dir_all(&snapshot) {
            warn!("Could not remove {}: {e}", snapshot.display());
        }
        return;
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let kept = (0..)
        .map(|n| {
            let name = match n {
                0 => format!("interrupted-{seconds}"),
                n => format!("interrupted-{seconds}-{n}"),
            };
            root.join(MODPACK_DIR).join(name)
        })
        .find(|path| std::fs::symlink_metadata(path).is_err())
        .expect("some name is free");
    match std::fs::rename(&snapshot, &kept) {
        Ok(()) => warn!(
            "Kept the files an interrupted modpack update took off the server in {}",
            kept.display()
        ),
        Err(e) => warn!(
            "Could not keep {} as {}: {e}",
            snapshot.display(),
            kept.display()
        ),
    }
}

/// What the dashboard shows before an update.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackUpdatePlan {
    /// The new version. `unsupported_reason` is set when the server can't be updated to it.
    pub target: super::ModpackInfo,
    pub current_game_version: String,
    /// `fabric`, `forge` or `neoforge`, or the server software of a server without mods.
    pub current_loader: String,
    pub current_loader_version: Option<String>,
    /// Whether the Minecraft or loader version changes, so the server software is
    /// installed again.
    pub reinstalls_server: bool,
    pub added: Vec<String>,
    pub replaced: Vec<String>,
    pub removed: Vec<String>,
    /// Pack files the user changed, left as they are.
    pub kept: Vec<String>,
    /// Pack files that are not written, and why ("path (reason)").
    pub skipped: Vec<String>,
    /// Set when the server has no record of the old pack's files: its files are then
    /// only added or replaced, and old ones aren't removed.
    pub note: Option<String>,
}

/// What an update did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackUpdateResult {
    pub added: u32,
    pub replaced: u32,
    pub removed: u32,
    pub kept: u32,
    /// A message about skipped files, if any.
    pub skipped: Option<String>,
}

#[cfg(test)]
mod tests;
