//! A server's modpack history: the lineage of its content images
//! (`docs/instance-images.md`, "M2 decisions").
//!
//! There is no history file. Every content image the core records for a server (a modpack
//! install, update or restore, a Mods-tab change, an edit through the Files API, a change
//! found before a start) is an entry, and the store keeps them as it keeps every content
//! image: the newest 30, the current one, and every one a backup links to
//! ([`crate::images::retention`]).
//!
//! - **Writes.** A modpack install, update or restore records one content image of the
//!   whole server once it succeeded, after `pack.json` ([`super::pack_info`]) says which
//!   pack the server is now built on, so the image's base and sources are the pack's.
//!   Before an update or restore changes anything, the content as it is on disk is recorded
//!   ([`before_change`]), so every file it replaces or removes is in an image.
//! - **Listing** ([`history`]): the content images newest first, each with what it changed
//!   from the one before it, and for each current file the image its version came with.
//! - **Restoring** ([`plan_restore`]): putting back a content image's mods and configs
//!   through the update's journaled placement ([`super::update`]), from the image store. A
//!   restore records a new content image; going back again is restoring that one's parent.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use color_eyre::eyre::eyre;
use lodestone_image::store::{Store, NEVER};
use lodestone_image::{Base, CanonicalPath, DiskLayout, Entry, Game, Image, Kind, Loader, Source};
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use super::update::{record_of, FileAction, FileChange, FilePlan, FileSource, ImageFile, PackFile};
use super::{InstancePathPolicy, ModpackOrigin, PackDownload};
use crate::error::{Error, ErrorKind};
use crate::images::recorder::{INITIAL_LABEL, OUTSIDE_LABEL};
use crate::images::scope::is_server_settings_file;
use crate::images::{ImageService, InstanceContext};
use crate::implementations::minecraft::{
    FabricLoaderVersion, Flavour, ForgeBuildVersion, NeoForgeBuildVersion,
};
use crate::mods::{disabled_name, enabled_name};
use crate::types::InstanceUuid;

// ---- labels ----

/// "Test Pack 2", or "Test Pack" when the version has no number.
fn pack_and_version(origin: &ModpackOrigin, joiner: &str) -> String {
    let version = origin.version_number.as_deref().unwrap_or("").trim();
    let pack = origin.name.trim();
    if version.is_empty() {
        pack.to_string()
    } else {
        format!("{pack}{joiner}{version}")
    }
}

/// The label of the content image a new server made from a pack records.
pub fn install_label(origin: &ModpackOrigin) -> String {
    format!("Installed {}", pack_and_version(origin, " "))
}

/// The label of the content image an update records.
pub fn update_label(origin: &ModpackOrigin) -> String {
    format!("Updated {}", pack_and_version(origin, " to "))
}

/// The label of the content image a restore records, from the label of the image it went
/// back to.
pub fn restore_label(target: Option<&str>) -> String {
    match target.map(str::trim).filter(|label| !label.is_empty()) {
        Some(label) => format!("Restored \u{201c}{label}\u{201d}"),
        None => "Restored an earlier version".to_string(),
    }
}

// ---- recording ----

/// Records the first content image of a server just made from the pack at `pack_path`
/// (its files, as installed, are `files`), before the core lists it: `pack.json` from the
/// pack, then the image, labelled "Installed <pack> <version>". Returns its id; `None` when
/// nothing could be recorded, which is logged and never fails the creation.
#[allow(clippy::too_many_arguments)]
pub async fn record_install(
    state: &crate::AppState,
    uuid: &InstanceUuid,
    root: &Path,
    origin: &ModpackOrigin,
    pack_path: &Path,
    files: &BTreeMap<String, PackFile>,
    game_version: &str,
    flavour: &Flavour,
) -> Option<String> {
    if !ImageService::accepts(uuid) {
        return None;
    }
    let info = {
        let (origin, pack_path, files) = (origin.clone(), pack_path.to_path_buf(), files.clone());
        tokio::task::spawn_blocking(move || {
            super::pack_info::base_info(&origin, Some(&pack_path), &record_of(&files), &files)
        })
        .await
        .ok()?
    };
    let images = state.images();
    super::pack_info::save(images, uuid, info).await;
    let exclude = state
        .backups
        .policy(uuid)
        .await
        .map(|policy| policy.exclude)
        .unwrap_or_default();
    let ctx = new_server_context(
        uuid.clone(),
        root.to_path_buf(),
        game_version.to_string(),
        flavour.clone(),
        exclude,
    )
    .await;
    match images
        .record_now(ctx, install_label(origin))
        .await
    {
        Ok(recorded) => recorded.map(|r| r.id),
        Err(e) => {
            warn!("Could not record the content image of {uuid}: {}", e.source);
            None
        }
    }
}

/// The `level-name` of the server in `root`, or the default `world`.
pub async fn level_name(root: &Path) -> String {
    use crate::implementations::minecraft::properties::PropertiesDocument;
    let bytes = tokio::fs::read(root.join("server.properties"))
        .await
        .unwrap_or_default();
    let (doc, _) = PropertiesDocument::from_bytes(&bytes);
    doc.get("level-name")
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| crate::images::layout::DEFAULT_LEVEL_NAME.to_string())
}

/// The context of a server being created (not in the core's list yet), stopped.
pub async fn new_server_context(
    uuid: InstanceUuid,
    root: PathBuf,
    game_version: String,
    flavour: Flavour,
    policy_exclude: Vec<String>,
) -> InstanceContext {
    InstanceContext {
        uuid,
        level_name: level_name(&root).await,
        root,
        game_version,
        flavour,
        policy_exclude,
        running: false,
    }
}

/// Before an update or restore of the server `ctx` changes its files: records its content
/// as it is now (what the Mods tab or the Files API changed in the last seconds, and what
/// changed outside Lodestone), so the current image is the content on disk and every file
/// the change takes off the server is in an image. A server with no image yet gets its
/// first. A restore must not go on when this fails ([`plan_restore`] trusts the current
/// image to be what is on disk); an update compares the files themselves and may.
pub async fn before_change(images: &Arc<ImageService>, ctx: InstanceContext) -> Result<(), Error> {
    if !ImageService::accepts(&ctx.uuid) {
        return Ok(());
    }
    let label = match images.current_content(&ctx.uuid).await {
        Some(_) => OUTSIDE_LABEL,
        None => INITIAL_LABEL,
    };
    images
        .record_now(ctx, label.to_string())
        .await
        .map(|_| ())
}

// ---- which files a restore changes ----

/// The server software, which a restore never copies: when the loader version differs,
/// the restore installs it again instead.
const SOFTWARE_DIRS: [&str; 2] = ["libraries/", "versions/"];
/// The server software's start files, which a restore leaves as they are (the loader
/// version a restore installs writes its own).
const SOFTWARE_FILES: [&str; 3] = ["run.sh", "run.bat", "user_jvm_args.txt"];

/// Whether a restore puts back the content file at canonical `path`: mods, configs,
/// plugins and the rest of the content, but not the server software (`libraries/`,
/// `versions/`, top-level jars, [`SOFTWARE_FILES`]) or the server's settings and admin
/// lists ([`is_server_settings_file`]). Scripts only when
/// `allow_scripts`, as for installing a pack. Names match ignoring ASCII case, as the
/// content scope's do.
fn restorable(path: &str, allow_scripts: bool) -> bool {
    let lower = path.to_ascii_lowercase();
    let top_level = !lower.contains('/');
    if SOFTWARE_DIRS.iter().any(|dir| lower.starts_with(dir))
        || (top_level && (lower.ends_with(".jar") || SOFTWARE_FILES.contains(&lower.as_str())))
        || CanonicalPath::new(path).is_ok_and(|path| is_server_settings_file(&path))
    {
        return false;
    }
    let Some(relative) = super::update::record_path(path) else {
        return false;
    };
    let components: Vec<String> = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let components: Vec<&str> = components.iter().map(String::as_str).collect();
    use lodestone_minecraft::mrpack::PathPolicy;
    InstancePathPolicy { allow_scripts }
        .skip_reason(&relative, &components, false)
        .is_none()
}

/// An image's entries a restore puts back, by canonical path. History listings count
/// scripts too: whether one may be restored depends on who asks.
fn restorable_entries(image: &Image, allow_scripts: bool) -> BTreeMap<&str, &Entry> {
    image
        .entries
        .iter()
        .filter(|e| restorable(e.path.as_str(), allow_scripts))
        .map(|e| (e.path.as_str(), e))
        .collect()
}

/// Whether `entry` can be downloaded again: a Modrinth file with the hashes a download is
/// checked against.
fn download_of(entry: &Entry, disk: &str) -> Option<PackDownload> {
    let Source::Modrinth { url, .. } = &entry.source else {
        return None;
    };
    let url = url::Url::parse(url).ok()?;
    if !lodestone_minecraft::mrpack::is_allowed_download(&url) {
        return None;
    }
    Some(PackDownload {
        path: PathBuf::from(disk),
        urls: vec![url],
        sha1: entry.sha1.clone()?.to_ascii_lowercase(),
        sha512: entry.sha512.clone()?.to_ascii_lowercase(),
        size: entry.size,
    })
}

// ---- the History API ----

/// A change as the dashboard shows it. A removed and an added file of the same Modrinth
/// project, or with the same contents (a mod enabled or disabled), are one `updated`
/// change, from `old_path` to `path`.
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
    /// The file's path after the change (before it, for a removed file).
    pub path: String,
    /// The old path of an updated file.
    pub old_path: Option<String>,
    pub project_id: Option<String>,
}

/// One entry of the history: a content image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryEntryInfo {
    /// The content image's id: what a restore names.
    pub id: String,
    /// When it was recorded, unix milliseconds.
    #[ts(type = "number")]
    pub time: i64,
    /// What made it: "Installed Test Pack 1", "Updated Test Pack to 2", "Added JEI",
    /// "Changed outside Lodestone".
    pub label: String,
    pub game_version: String,
    /// `fabric`, `forge`, `neoforge` (or `vanilla` for a server without mods).
    pub loader: String,
    pub loader_version: Option<String>,
    /// The mods and configs it changed, from the image before it. Empty for the first.
    pub changes: Vec<ChangeInfo>,
    /// How many files a restore of it puts back (mods, configs and other content, not
    /// the server software or the server's own settings).
    pub file_count: u32,
    /// The server's content is this image.
    pub current: bool,
    /// How many of the files a restore would write Lodestone can't get (the store lost
    /// them and they can't be downloaded): it can only be restored when this is 0.
    pub missing: u32,
}

/// `GET /instance/{uuid}/modpack/history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackHistory {
    /// Newest first.
    pub entries: Vec<HistoryEntryInfo>,
    /// For each mod and config of the current content (by path, a mod by its enabled name),
    /// the id of the entry its current version came with.
    pub blame: BTreeMap<String, String>,
    /// A newer version of the modpack on Modrinth, when the last check found one.
    pub update: Option<super::updates::AvailableUpdate>,
}

/// The Modrinth project of an entry, from its source.
fn project_of(entry: &Entry) -> Option<String> {
    match &entry.source {
        Source::Modrinth { project, .. } => Some(project.clone()),
        _ => None,
    }
}

/// What `newer` changed from `older` (both by path), with removed and added files of one
/// project, or with the same contents, paired as updates.
fn change_infos(newer: &BTreeMap<&str, &Entry>, older: &BTreeMap<&str, &Entry>) -> Vec<ChangeInfo> {
    let removed: Vec<&Entry> = older
        .iter()
        .filter(|(path, _)| !newer.contains_key(*path))
        .map(|(_, e)| *e)
        .collect();
    let mut paired: HashSet<&str> = HashSet::new();
    let mut infos = Vec::new();
    for (path, entry) in newer {
        match older.get(path) {
            Some(old) if old.sha256 == entry.sha256 => {}
            Some(_) => infos.push(ChangeInfo {
                kind: ChangeKind::Replaced,
                path: path.to_string(),
                old_path: None,
                project_id: project_of(entry),
            }),
            None => {
                let project = project_of(entry);
                let old = removed.iter().find(|old| {
                    !paired.contains(old.path.as_str())
                        && (old.sha256 == entry.sha256
                            || (project.is_some() && project_of(old) == project))
                });
                match old {
                    Some(old) => {
                        paired.insert(old.path.as_str());
                        infos.push(ChangeInfo {
                            kind: ChangeKind::Updated,
                            path: path.to_string(),
                            old_path: Some(old.path.to_string()),
                            project_id: project,
                        });
                    }
                    None => infos.push(ChangeInfo {
                        kind: ChangeKind::Added,
                        path: path.to_string(),
                        old_path: None,
                        project_id: project,
                    }),
                }
            }
        }
    }
    for old in removed {
        if !paired.contains(old.path.as_str()) {
            infos.push(ChangeInfo {
                kind: ChangeKind::Removed,
                path: old.path.to_string(),
                old_path: None,
                project_id: project_of(old),
            });
        }
    }
    infos.sort_by(|a, b| (a.kind as u8, &a.path).cmp(&(b.kind as u8, &b.path)));
    infos
}

/// Whether `files` has the file at `path` with these contents, under its own name or the
/// mod's other (enabled or disabled) name.
fn has_file(files: &BTreeMap<&str, &Entry>, path: &str, sha256: &str) -> bool {
    let enabled = enabled_name(path);
    [path.to_string(), enabled.clone(), disabled_name(&enabled)]
        .iter()
        .any(|candidate| {
            files
                .get(candidate.as_str())
                .is_some_and(|e| e.sha256 == sha256)
        })
}

/// The loader an image's game runs, as the dashboard names it.
fn loader_of(game: &Game) -> String {
    game.loader.as_str().to_string()
}

/// The server's modpack history (see the module): its content images, newest first. A
/// manifest that can't be read is left out, with a warning. Reads the image store, so call
/// it on a blocking thread.
pub fn history(
    images: &ImageService,
    uuid: &InstanceUuid,
    update: Option<super::updates::AvailableUpdate>,
) -> ModpackHistory {
    let mut history = ModpackHistory {
        entries: Vec::new(),
        blame: BTreeMap::new(),
        update,
    };
    if !ImageService::accepts(uuid) {
        return history;
    }
    let store = images.store();
    let listing = match crate::images::retention::list(&store, &images.links, uuid) {
        Ok(listing) => listing,
        Err(e) => {
            warn!("Could not list the content images of {uuid}: {}", e.source);
            return history;
        }
    };
    for (id, why) in &listing.unreadable {
        warn!("Leaving the image {id} of {uuid} out of its history: {why}");
    }
    let current_id = crate::images::state::load(store.root(), uuid).current;
    // Whole manifests only for the content images, in the history's order.
    let images: Vec<Image> =
        crate::images::retention::newest_content_first(&listing.content, current_id.as_deref())
            .into_iter()
            .filter_map(|header| match store.load_manifest(uuid.as_ref(), &header.id) {
                Ok(image) => Some(image),
                Err(e) => {
                    warn!("Leaving the image {} of {uuid} out of its history: {e}", header.id);
                    None
                }
            })
            .collect();
    let current = current_id
        .as_deref()
        .and_then(|id| images.iter().position(|image| image.id == id));
    let index: HashMap<&str, usize> = images
        .iter()
        .enumerate()
        .map(|(i, image)| (image.id.as_str(), i))
        .collect();
    let files: Vec<BTreeMap<&str, &Entry>> = images
        .iter()
        .map(|image| restorable_entries(image, true))
        .collect();
    let empty = BTreeMap::new();
    let current_files = current.map_or(&empty, |c| &files[c]);
    let mut available: HashMap<(&str, u64), bool> = HashMap::new();
    for (i, image) in images.iter().enumerate() {
        // The image it was recorded after; one pruned since stands for the next older.
        let older = image
            .parent
            .as_ref()
            .map(|parent| {
                index
                    .get(parent.as_str())
                    .copied()
                    .filter(|&o| o != i)
                    .unwrap_or(i + 1)
            })
            .filter(|&o| o < images.len());
        let changes = match older {
            Some(o) => change_infos(&files[i], &files[o]),
            None => Vec::new(),
        };
        let missing = files[i]
            .values()
            .filter(|entry| {
                let needed = current_files
                    .get(entry.path.as_str())
                    .is_none_or(|have| have.sha256 != entry.sha256);
                needed
                    && !*available
                        .entry((entry.sha256.as_str(), entry.size))
                        .or_insert_with(|| store.object_complete(&entry.sha256, entry.size))
                    && download_of(entry, entry.path.as_str()).is_none()
            })
            .count();
        history.entries.push(HistoryEntryInfo {
            id: image.id.clone(),
            time: image.created.saturating_mul(1000),
            label: image.label.clone().unwrap_or_default(),
            game_version: image.game.minecraft.clone(),
            loader: loader_of(&image.game),
            loader_version: image.game.loader_version.clone(),
            changes,
            file_count: files[i].len() as u32,
            current: Some(i) == current,
            missing: missing as u32,
        });
    }
    if let Some(c) = current {
        for (path, entry) in &files[c] {
            let mut came_with = c;
            while came_with + 1 < images.len()
                && has_file(&files[came_with + 1], path, &entry.sha256)
            {
                came_with += 1;
            }
            history
                .blame
                .insert(enabled_name(path), images[came_with].id.clone());
        }
    }
    history
}

// ---- restoring ----

/// What a restore goes back to: the parts of its content image the restore needs beyond
/// the files.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreTarget {
    pub id: String,
    pub label: Option<String>,
    pub base: Option<Base>,
    pub game: Game,
    /// The SHA-256 each file the restore replaces or removes had in the current image, by
    /// disk path: checked again right before the files are placed ([`check_unchanged`]),
    /// so a file changed since (by the Files API, or by hand) is never thrown away.
    pub replaced: BTreeMap<String, String>,
}

/// A restore, worked out before anything changes.
#[derive(Debug, Clone)]
pub struct RestorePlan {
    pub target: RestoreTarget,
    /// The changes, by disk path (a change's `path` and `target` are the same).
    pub plan: FilePlan,
    /// Where each added or replaced file's bytes come from, by disk path.
    pub sources: BTreeMap<String, FileSource>,
    /// Files the restore must write that Lodestone can't get, by disk path.
    pub missing: Vec<String>,
    /// Paths where the restore would write a file but something else is in the way (a
    /// folder, a link), which it never moves.
    pub blocked: Vec<String>,
    /// Files on disk the restore would replace that the current image has no copy of (it
    /// withheld them, or they appeared since it was recorded): replacing them would lose
    /// them for good, so the restore is refused.
    pub unrecorded: Vec<String>,
    /// The bytes it downloads.
    pub download_size: u64,
    /// How many files the image has that a restore puts back.
    pub file_count: u32,
}

fn not_in_history() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("That version is no longer in the server's history"),
    }
}

/// Whether `id` can be an image id (64 lowercase hex), before it is used in a path.
pub fn is_image_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Plans restoring the content image `target` on the server in `root` whose current content
/// image is `current`: every content file a restore puts back ([`restorable`]) becomes as
/// the target has it. A file the target has that the current image doesn't, or with other
/// contents, is added or replaced from the image store (or downloaded again from Modrinth
/// when the store lost it); a file the current image has that the target doesn't is
/// removed. Files neither image lists (left out by the backup policy's excludes, withheld,
/// or not content at all) are never touched. Reads manifests and looks at the server's
/// files, so call it on a blocking thread.
#[allow(clippy::too_many_arguments)]
pub fn plan_restore(
    store: &Store,
    uuid: &InstanceUuid,
    root: &Path,
    layout: &DiskLayout,
    current: &str,
    target: &str,
    allow_scripts: bool,
) -> Result<RestorePlan, Error> {
    if !is_image_id(target) || !ImageService::accepts(uuid) {
        return Err(not_in_history());
    }
    let target_image = store
        .load_manifest(uuid.as_ref(), target)
        .map_err(|_| not_in_history())?;
    if target_image.kind != Kind::Content {
        return Err(not_in_history());
    }
    let current_image = store
        .load_manifest(uuid.as_ref(), current)
        .map_err(|e| eyre!("Could not read the server's current content: {e}"))?;
    let on_disk = |image: &Image| -> BTreeMap<String, Entry> {
        restorable_entries(image, allow_scripts)
            .into_values()
            .filter_map(|entry| Some((layout.to_disk(&entry.path).ok()?, entry.clone())))
            .filter(|(disk, _)| super::update::record_path(disk).is_some())
            .collect()
    };
    let wanted = on_disk(&target_image);
    let have = on_disk(&current_image);
    let file_count = wanted.len() as u32;
    let mut plan = RestorePlan {
        target: RestoreTarget {
            id: target_image.id.clone(),
            label: target_image.label.clone(),
            base: target_image.base.clone(),
            game: target_image.game.clone(),
            replaced: BTreeMap::new(),
        },
        plan: FilePlan::default(),
        sources: BTreeMap::new(),
        missing: Vec::new(),
        blocked: Vec::new(),
        unrecorded: Vec::new(),
        download_size: 0,
        file_count,
    };
    // What is at a path now: a regular file, nothing, or something a restore never moves.
    enum There {
        File,
        Nothing,
        Other,
    }
    let there = |disk: &str| match std::fs::symlink_metadata(root.join(disk)) {
        Ok(meta) if meta.is_file() => There::File,
        Ok(_) => There::Other,
        Err(_) => There::Nothing,
    };
    let change = |path: &str, action| FileChange {
        path: path.to_string(),
        target: path.to_string(),
        action,
    };
    for (disk, entry) in &have {
        if !wanted.contains_key(disk) && matches!(there(disk), There::File) {
            plan.plan.changes.push(change(disk, FileAction::Remove));
            plan.target
                .replaced
                .insert(disk.clone(), entry.sha256.clone());
        }
    }
    for (disk, entry) in &wanted {
        let current = have.get(disk);
        if current.is_some_and(|c| c.sha256 == entry.sha256) {
            continue;
        }
        let action = match (there(disk), current) {
            (There::File, Some(current)) => {
                plan.target
                    .replaced
                    .insert(disk.clone(), current.sha256.clone());
                FileAction::Replace
            }
            (There::File, None) => {
                plan.unrecorded.push(disk.clone());
                continue;
            }
            (There::Nothing, _) => FileAction::Add,
            (There::Other, _) => {
                plan.blocked.push(disk.clone());
                continue;
            }
        };
        let source = if store.object_complete(&entry.sha256, entry.size) {
            FileSource::Image(ImageFile {
                sha256: entry.sha256.clone(),
                size: entry.size,
            })
        } else if let Some(download) = download_of(entry, disk) {
            plan.download_size += download.size;
            FileSource::Download(download)
        } else {
            plan.missing.push(disk.clone());
            continue;
        };
        plan.sources.insert(disk.clone(), source);
        plan.plan.changes.push(change(disk, action));
    }
    Ok(plan)
}

/// Fails unless each file in `replaced` (disk path to SHA-256, [`RestoreTarget::replaced`])
/// is still a file with those contents: what a restore checks right before it moves them
/// aside, so a change made since it was planned is never lost. Blocking.
pub fn check_unchanged(root: &Path, replaced: &BTreeMap<String, String>) -> Result<(), Error> {
    use sha2::Digest;
    let changed: Vec<&str> = replaced
        .iter()
        .filter(|(disk, sha256)| {
            let same = (|| -> std::io::Result<bool> {
                let path = root.join(disk.as_str());
                if !std::fs::symlink_metadata(&path)?.is_file() {
                    return Ok(false);
                }
                let mut hasher = sha2::Sha256::new();
                std::io::copy(&mut std::fs::File::open(&path)?, &mut hasher)?;
                let hex: String = hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                Ok(hex == **sha256)
            })();
            !same.unwrap_or(false)
        })
        .map(|(disk, _)| disk.as_str())
        .collect();
    if changed.is_empty() {
        return Ok(());
    }
    let first: Vec<&str> = changed.iter().take(5).copied().collect();
    Err(Error {
        kind: ErrorKind::Conflict,
        source: eyre!(
            "{} file(s) the restore would replace changed since it was planned ({}); try again",
            changed.len(),
            first.join(", ")
        ),
    })
}

/// The server software an image's game ran, for a restore that installs it again. `None`
/// for a server without a mod loader, or a loader version the image doesn't say.
pub fn flavour_of_game(game: &Game) -> Option<Flavour> {
    let version = game.loader_version.clone()?;
    Some(match game.loader {
        Loader::Fabric => Flavour::Fabric {
            loader_version: Some(FabricLoaderVersion::new(version)),
            installer_version: None,
        },
        Loader::Forge => Flavour::Forge {
            build_version: Some(ForgeBuildVersion::new(version)),
        },
        Loader::NeoForge => Flavour::NeoForge {
            build_version: Some(NeoForgeBuildVersion::new(version)),
        },
        _ => return None,
    })
}

/// The SHA-512 of the file `image` stands for, read and checked by `store` (never what a
/// manifest says about it): what a restore asks Modrinth about before it puts a jar back
/// for a user who can't write global files. Blocking.
pub fn image_sha512(store: &Store, image: &ImageFile) -> Option<String> {
    struct Hashing(sha2::Sha512);
    impl std::io::Write for Hashing {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            sha2::Digest::update(&mut self.0, bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hashing = Hashing(<sha2::Sha512 as sha2::Digest>::new());
    store
        .read_object(&image.sha256, image.size, &mut hashing, NEVER)
        .ok()?;
    Some(
        sha2::Digest::finalize(hashing.0)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod tests;
