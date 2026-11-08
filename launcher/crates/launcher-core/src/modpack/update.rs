//! Updating an instance to another version of its modpack.
//!
//! `modpack.json` records every file the pack wrote, with the pack's SHA-1. Comparing
//! that, the new pack and the files on disk gives the plan, which is shown before
//! anything changes:
//!
//! - **The pack's downloaded files** (the index): a file the new pack lists is added
//!   when missing, and replaced when the old pack wrote it and it is unchanged; a file the
//!   new pack no longer lists is removed when unchanged. A pack file the user changed is
//!   left as it is. A file the old pack didn't write but the new pack lists at the same
//!   path is replaced (it is in the snapshot). A disabled mod stays disabled, also when
//!   its new version has another file name (same Modrinth project).
//! - **Overrides** (configs and bundled files): written when missing, or when the old
//!   pack wrote them and they are unchanged; never over the user's own files or changes.
//!   Overrides the new pack dropped are removed only in `mods/`, `resourcepacks/` and
//!   `shaderpacks/`, and only when unchanged; configs stay.
//! - **Everything else** (the user's own mods, worlds, options, screenshots) is never
//!   touched.
//!
//! Applying it downloads and extracts every new file into a staging folder first, then
//! copies what will be replaced or removed into a content snapshot
//! (`backups/modpack-update/<time>/`, with `instance.json`, `content.json` and
//! `modpack.json`), optionally backs up every world, and only then changes the game
//! folder. The loader and Minecraft version follow the new pack; they install on the
//! next Play.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::mrpack::PackDownload;

use super::{
    FileSource, ModpackError, ModpackRecord, OverrideFile, PackInfo, cdn_ids,
    import::{content_entries, download_files, extract_overrides, record_files, sha1_on_disk},
    io_error, slash_path,
};
use crate::{
    content::{
        ContentKind, InstanceDirs, Modrinth, disabled_name, enabled_name,
        install::InstallProgress,
        is_disabled_name,
        provenance::{Provenance, key},
    },
    game::{
        download::{CancelToken, Downloader},
        loaders::LoaderChoice,
    },
    worlds,
};

/// The folders whose files a pack's dropped overrides are removed from.
const CONTENT_DIRS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// What updating will do. Paths are relative to the game folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdatePlan {
    /// The new record (the new version, with the new pack's files).
    pub record: ModpackRecord,
    pub game_version: String,
    pub loader: Option<LoaderChoice>,
    /// New files.
    pub added: Vec<String>,
    /// Files replaced by a new version: (old path on disk, new path).
    pub updated: Vec<(String, String)>,
    /// Files removed.
    pub removed: Vec<String>,
    /// Pack files the user changed, left as they are.
    pub kept_changed: Vec<String>,
    /// Files the old pack didn't write at a path the new pack uses, replaced.
    pub replaced_own: Vec<String>,
    /// Config and other override files written.
    pub overrides_written: Vec<String>,
    /// Override files left as the user has them.
    pub overrides_kept: Vec<String>,
    /// The downloads to make, with the path on disk (relative) each goes to.
    downloads: Vec<(PackDownload, String)>,
    /// The overrides to copy, with their path on disk.
    overrides: Vec<(OverrideFile, String)>,
    /// Files to remove (their path on disk).
    removals: Vec<String>,
}

impl UpdatePlan {
    /// Whether the update changes any file.
    pub fn changes_files(&self) -> bool {
        !self.downloads.is_empty() || !self.overrides.is_empty() || !self.removals.is_empty()
    }

    /// The files written or removed, on disk.
    pub fn touched(&self) -> Vec<String> {
        let mut paths: BTreeSet<String> = self.removals.iter().cloned().collect();
        paths.extend(self.downloads.iter().map(|(_, p)| p.clone()));
        paths.extend(self.overrides.iter().map(|(_, p)| p.clone()));
        paths.into_iter().collect()
    }

    pub fn download_size(&self) -> u64 {
        self.downloads.iter().map(|(d, _)| d.size).sum()
    }
}

/// The file at `relative` (or, for a mod, its `.disabled` name): its path on disk and
/// SHA-1.
fn on_disk(game: &Path, relative: &str) -> Option<(String, String)> {
    let mut candidates = vec![relative.to_string()];
    if relative.starts_with("mods/") && !is_disabled_name(relative) {
        candidates.push(disabled_name(relative));
    }
    candidates
        .into_iter()
        .find_map(|path| sha1_on_disk(&game.join(&path)).map(|sha1| (path, sha1)))
}

/// The plan for updating the instance at `dirs`, made from the pack `current` records,
/// to `pack` (from the version `record` describes).
pub fn plan(
    dirs: &InstanceDirs,
    current: &ModpackRecord,
    pack: &PackInfo,
    record: ModpackRecord,
    loader: Option<LoaderChoice>,
) -> UpdatePlan {
    let game = &dirs.game;
    let mut plan = UpdatePlan {
        game_version: pack.game_version.clone(),
        loader,
        ..UpdatePlan::default()
    };
    let new_index: BTreeMap<String, &PackDownload> = pack
        .downloads
        .iter()
        .map(|d| (slash_path(&d.path), d))
        .collect();
    let new_overrides: BTreeMap<String, &OverrideFile> =
        pack.overrides.iter().map(|o| (o.path.clone(), o)).collect();

    // The old pack's files that go away, by project, to pair them with new versions.
    let mut removed_by_project: HashMap<String, String> = HashMap::new();
    for (path, old) in &current.files {
        if new_index.contains_key(path) || new_overrides.contains_key(path) {
            continue;
        }
        let removable = match old.source {
            FileSource::Index => true,
            FileSource::Override => CONTENT_DIRS
                .iter()
                .any(|dir| path.starts_with(&format!("{dir}/"))),
        };
        if !removable {
            continue;
        }
        match on_disk(game, path) {
            None => {}
            Some((actual, sha1)) if sha1 == old.sha1 => {
                if let Some(project) = &old.project {
                    removed_by_project.insert(project.clone(), actual.clone());
                }
                plan.removals.push(actual);
            }
            Some((actual, _)) => plan.kept_changed.push(actual),
        }
    }

    for (path, download) in &new_index {
        if new_overrides.contains_key(path) {
            // An override replaces it anyway.
            continue;
        }
        let project = download
            .urls
            .iter()
            .find_map(|u| cdn_ids(u).map(|(p, _)| p));
        match (current.files.get(path), on_disk(game, path)) {
            (_, Some((_, sha1))) if sha1 == download.sha1 => {}
            (Some(old), Some((actual, sha1))) if sha1 == old.sha1 => {
                plan.updated.push((actual.clone(), actual.clone()));
                plan.downloads.push(((*download).clone(), actual));
            }
            (Some(_), Some((actual, _))) => plan.kept_changed.push(actual),
            (None, Some((actual, _))) => {
                plan.replaced_own.push(actual.clone());
                plan.downloads.push(((*download).clone(), actual));
            }
            (_, None) => {
                // A new version of a mod the old pack had: it stays disabled if it was.
                let old = project.as_ref().and_then(|p| removed_by_project.get(p));
                let dest = match old {
                    Some(old) if is_disabled_name(old) && path.starts_with("mods/") => {
                        disabled_name(path)
                    }
                    _ => path.clone(),
                };
                match old {
                    Some(old) => plan.updated.push((old.clone(), dest.clone())),
                    None => plan.added.push(dest.clone()),
                }
                plan.downloads.push(((*download).clone(), dest));
            }
        }
    }
    // Paired removals are shown as updates, not as removals.
    let paired: BTreeSet<&String> = plan.updated.iter().map(|(old, _)| old).collect();
    plan.removed = plan
        .removals
        .iter()
        .filter(|p| !paired.contains(p))
        .cloned()
        .collect();

    for (path, file) in &new_overrides {
        match (current.files.get(path), on_disk(game, path)) {
            (_, None) => {
                plan.overrides_written.push(path.clone());
                plan.overrides.push(((*file).clone(), path.clone()));
            }
            (_, Some((_, sha1))) if sha1 == file.sha1 => {}
            (Some(old), Some((actual, sha1))) if sha1 == old.sha1 => {
                plan.overrides_written.push(actual.clone());
                plan.overrides.push(((*file).clone(), actual));
            }
            (_, Some((actual, _))) => plan.overrides_kept.push(actual),
        }
    }
    plan.record = ModpackRecord {
        files: record_files(pack),
        ..record
    };
    plan
}

/// What an update did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOutcome {
    /// The content snapshot.
    pub snapshot: PathBuf,
    /// The world backups made first.
    pub world_backups: Vec<PathBuf>,
}

/// Copies `relative` (under `game`) into the snapshot.
fn snapshot_file(game: &Path, snapshot: &Path, relative: &str) -> Result<(), ModpackError> {
    let from = game.join(relative);
    if !from.is_file() {
        return Ok(());
    }
    let to = snapshot.join("minecraft").join(relative);
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    fs::copy(&from, &to).map_err(io_error(&to))?;
    Ok(())
}

/// Applies `plan` (for the pack at `pack_path`) to the instance at `dirs`: see the
/// module's documentation. The caller saves the instance's new Minecraft version and
/// loader.
#[allow(clippy::too_many_arguments)]
pub async fn apply(
    api: Option<&Modrinth>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    plan: &UpdatePlan,
    pack_path: &Path,
    backup_worlds: bool,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<UpdateOutcome, ModpackError> {
    let staging = dirs.instance.join(format!(
        ".lodestone-update-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    ));
    fs::create_dir_all(&staging).map_err(io_error(&staging))?;
    let result = apply_staged(
        api,
        downloader,
        dirs,
        plan,
        pack_path,
        backup_worlds,
        progress,
        cancel,
        &staging,
    )
    .await;
    let _ = fs::remove_dir_all(&staging);
    result
}

#[allow(clippy::too_many_arguments)]
async fn apply_staged(
    api: Option<&Modrinth>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    plan: &UpdatePlan,
    pack_path: &Path,
    backup_worlds: bool,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &Path,
) -> Result<UpdateOutcome, ModpackError> {
    let game = &dirs.game;
    // 1. Everything new, into the staging folder.
    if !plan.downloads.is_empty() {
        let api = api.ok_or_else(|| {
            ModpackError::Invalid("Modrinth can't be reached to download the pack's files.".into())
        })?;
        progress.set_label(format!("Downloading {}", plan.record.title));
        let downloads: Vec<(PackDownload, PathBuf)> = plan
            .downloads
            .iter()
            .map(|(d, dest)| (d.clone(), staging.join(dest)))
            .collect();
        download_files(api, downloader, staging, &downloads, progress, cancel).await?;
    }
    let overrides: Vec<(OverrideFile, PathBuf)> = plan
        .overrides
        .iter()
        .map(|(o, dest)| (o.clone(), staging.join(dest)))
        .collect();
    extract_overrides(pack_path, staging, &overrides, cancel)?;
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }

    // 2. The snapshot, and the world backups.
    progress.set_label("Saving a snapshot of the instance's files");
    let snapshot = worlds::backups_dir(dirs)
        .join("modpack-update")
        .join(worlds::timestamp(SystemTime::now()));
    fs::create_dir_all(&snapshot).map_err(io_error(&snapshot))?;
    for path in plan.touched() {
        snapshot_file(game, &snapshot, &path)?;
    }
    for file in [
        "instance.json",
        crate::content::provenance::FILE,
        super::RECORD_FILE,
    ] {
        let from = dirs.instance.join(file);
        if from.is_file() {
            fs::copy(&from, snapshot.join(file)).map_err(io_error(&from))?;
        }
    }
    let mut world_backups = Vec::new();
    if backup_worlds {
        for world in worlds::list(dirs) {
            if cancel.is_cancelled() {
                return Err(ModpackError::Cancelled);
            }
            progress.set_label(format!("Backing up {}", world.name()));
            let backup = worlds::backup(dirs, &world.folder, &progress.downloads, cancel).map_err(
                |err| match err {
                    worlds::WorldError::Cancelled => ModpackError::Cancelled,
                    other => {
                        ModpackError::Invalid(format!("Couldn't back up {}: {other}", world.name()))
                    }
                },
            )?;
            world_backups.push(backup.path);
        }
    }
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }

    // 3. The game folder.
    progress.set_label(format!("Updating to {}", plan.record.version_number));
    let snapshot_note = |err: ModpackError| {
        ModpackError::Invalid(format!(
            "The update stopped part way ({err}). The files it replaced are in {}.",
            snapshot.display()
        ))
    };
    for path in &plan.removals {
        let target = game.join(path);
        if let Err(err) = fs::remove_file(&target)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            return Err(snapshot_note(io_error(&target)(err)));
        }
    }
    let staged: Vec<&String> = plan
        .downloads
        .iter()
        .map(|(_, p)| p)
        .chain(plan.overrides.iter().map(|(_, p)| p))
        .collect();
    for path in staged {
        let from = staging.join(path);
        let to = game.join(path);
        let moved = (|| {
            super::import::prepare_parent(game, &to)?;
            if to.is_file() {
                fs::remove_file(&to).map_err(io_error(&to))?;
            }
            fs::rename(&from, &to).map_err(io_error(&to))
        })();
        moved.map_err(snapshot_note)?;
    }

    // 4. The records.
    let removed_keys: Vec<(ContentKind, String)> = plan
        .removals
        .iter()
        .chain(plan.downloads.iter().map(|(_, p)| p))
        .filter_map(|path| {
            let (dir, name) = path.split_once('/')?;
            let kind = ContentKind::ALL.into_iter().find(|k| k.dir() == dir)?;
            Some((kind, enabled_name(name)))
        })
        .collect();
    let downloaded: Vec<(String, &PackDownload)> =
        plan.downloads.iter().map(|(d, p)| (p.clone(), d)).collect();
    let entries = content_entries(api, game, &downloaded).await;
    Provenance::update(&dirs.instance, |provenance| {
        for (kind, name) in &removed_keys {
            provenance.files.remove(&key(*kind, name));
        }
        for (kind, name, entry) in &entries {
            provenance.insert(*kind, name, entry.clone());
        }
    })?;
    plan.record.save(&dirs.instance)?;
    Ok(UpdateOutcome {
        snapshot,
        world_backups,
    })
}
