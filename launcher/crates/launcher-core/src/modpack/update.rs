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
//! folder. If changing the game folder or the records fails part way (a file another
//! program holds open, say), the files changed so far and the records are put back
//! from the snapshot. The loader and Minecraft version follow the new pack; they
//! install on the next Play.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::mrpack::PackDownload;

use super::{
    ModpackError, ModpackRecord, OverrideFile, PackInfo,
    fetch::{Sources, stage},
    import::content_entries,
    io_error,
    merge::{self, Fetch, Wanted},
    slash_path,
};
use crate::{
    content::{
        ContentKind, InstanceDirs, Modrinth, enabled_name,
        install::InstallProgress,
        provenance::{Provenance, key},
    },
    game::{
        download::{CancelToken, Downloader},
        loaders::LoaderChoice,
    },
    worlds,
};

/// What updating will do. Paths are relative to the game folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdatePlan {
    /// The new record (the new version, with the new pack's files).
    pub record: ModpackRecord,
    pub game_version: String,
    pub loader: Option<LoaderChoice>,
    /// The memory the new version recommends, MiB.
    pub recommended_memory: Option<u32>,
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
    /// The files to write, with the path on disk (relative) each goes to.
    writes: Vec<(Fetch, String)>,
    /// Files to remove (their path on disk).
    removals: Vec<String>,
}

impl UpdatePlan {
    /// Whether the update changes any file.
    pub fn changes_files(&self) -> bool {
        !self.writes.is_empty() || !self.removals.is_empty()
    }

    /// The files written or removed, on disk.
    pub fn touched(&self) -> Vec<String> {
        let mut paths: BTreeSet<String> = self.removals.iter().cloned().collect();
        paths.extend(self.writes.iter().map(|(_, p)| p.clone()));
        paths.into_iter().collect()
    }

    /// What the update downloads, from anywhere: the pack's hosts and the server.
    pub fn download_size(&self) -> u64 {
        self.writes
            .iter()
            .filter(|(f, _)| !matches!(f, Fetch::Override { .. }))
            .map(|(f, _)| f.size())
            .sum()
    }

    /// Every file the update writes, with the path on disk each goes to.
    pub fn writes(&self) -> impl Iterator<Item = (&Fetch, &str)> {
        self.writes.iter().map(|(f, p)| (f, p.as_str()))
    }

    /// The files the update downloads, with the path on disk each goes to.
    pub fn downloads(&self) -> impl Iterator<Item = (&PackDownload, &str)> {
        self.writes.iter().filter_map(|(f, p)| match f {
            Fetch::Download { download, .. } => Some((download, p.as_str())),
            _ => None,
        })
    }

    /// The override files the update writes, with their path on disk.
    pub fn override_writes(&self) -> impl Iterator<Item = (&OverrideFile, &str)> {
        self.writes.iter().filter_map(|(f, p)| match f {
            Fetch::Override { file, .. } => Some((file, p.as_str())),
            _ => None,
        })
    }

    /// Also removes the files at `paths` (on disk), which aren't the pack's: the mods
    /// of added packs that the new version supersedes ([`super::addon::superseded`]).
    pub(crate) fn remove_also(&mut self, paths: Vec<String>) {
        for path in paths {
            let written = self.writes.iter().any(|(_, p)| *p == path);
            if written || self.removals.contains(&path) {
                continue;
            }
            self.removed.push(path.clone());
            self.removals.push(path);
        }
    }
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
    plan_with(dirs, current, pack, record, loader, &[])
}

/// Like [`plan`], but the pack's files under the folders `authoritative` (such as
/// `mods/`, for a server's pack, whose mods must match the server's) replace the user's
/// changed copies instead of keeping them. Dropped files the user changed still stay.
pub fn plan_with(
    dirs: &InstanceDirs,
    current: &ModpackRecord,
    pack: &PackInfo,
    record: ModpackRecord,
    loader: Option<LoaderChoice>,
    authoritative: &[&str],
) -> UpdatePlan {
    let mut new: BTreeMap<String, Wanted> = pack
        .downloads
        .iter()
        .map(|d| (slash_path(&d.path), Wanted::download(d)))
        .collect();
    // Overrides are copied after the downloads, so they win.
    for file in &pack.overrides {
        new.insert(file.path.clone(), Wanted::override_file(file));
    }
    let plan = plan_files(dirs, current, &new, record, authoritative);
    UpdatePlan {
        game_version: pack.game_version.clone(),
        loader,
        recommended_memory: pack.recommended_memory,
        ..plan
    }
}

/// The plan for moving the instance at `dirs` from the files `current` records to the
/// files `new` places (a pack's, or a server's image's), whose record is `record` with
/// those files. The caller fills in the game.
pub fn plan_files(
    dirs: &InstanceDirs,
    current: &ModpackRecord,
    new: &BTreeMap<String, Wanted>,
    record: ModpackRecord,
    authoritative: &[&str],
) -> UpdatePlan {
    let merged = merge::plan(&dirs.game, &current.files, new, authoritative);
    UpdatePlan {
        record: ModpackRecord {
            files: merge::record_files(new),
            ..record
        },
        added: merged.added,
        updated: merged.updated,
        removed: merged.removed,
        kept_changed: merged.kept_changed,
        replaced_own: merged.replaced_own,
        overrides_written: merged.overrides_written,
        overrides_kept: merged.overrides_kept,
        writes: merged.writes,
        removals: merged.removals,
        ..UpdatePlan::default()
    }
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
    let sources = Sources {
        platforms: crate::content::Platforms::from(api),
        downloader,
        pack: Some(pack_path),
        objects: None,
    };
    apply_from(&sources, dirs, plan, backup_worlds, progress, cancel).await
}

/// [`apply`], with the files from `sources`: a pack's, or a server's image's (whose
/// objects come from the server).
pub async fn apply_from(
    sources: &Sources<'_>,
    dirs: &InstanceDirs,
    plan: &UpdatePlan,
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
        sources,
        dirs,
        plan,
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
    sources: &Sources<'_>,
    dirs: &InstanceDirs,
    plan: &UpdatePlan,
    backup_worlds: bool,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &Path,
) -> Result<UpdateOutcome, ModpackError> {
    let game = &dirs.game;
    // 1. Everything new, into the staging folder.
    stage(
        sources,
        staging,
        &plan.record.title,
        &plan.writes,
        progress,
        cancel,
    )
    .await?;
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
        super::addon::ADDED_FILE,
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

    // 3. The game folder. If a file can't be changed (another program has it open, say),
    // what was changed so far is put back from the snapshot.
    progress.set_label(format!("Updating to {}", plan.record.version_number));
    let mut changed = BTreeSet::new();
    if let Err(err) = swap_in(game, staging, plan, &mut changed) {
        return Err(undo(dirs, &snapshot, &changed, err));
    }

    // 4. The records. What was downloaded or came from the server isn't what
    // `content.json` said was at its path any more.
    let removed_keys: Vec<(ContentKind, String)> = plan
        .removals
        .iter()
        .chain(
            plan.writes
                .iter()
                .filter(|(f, _)| !matches!(f, Fetch::Override { .. }))
                .map(|(_, p)| p),
        )
        .filter_map(|path| {
            let (dir, name) = path.split_once('/')?;
            let kind = ContentKind::ALL.into_iter().find(|k| k.dir() == dir)?;
            Some((kind, enabled_name(name)))
        })
        .collect();
    let downloaded: Vec<(String, &PackDownload)> =
        plan.downloads().map(|(d, p)| (p.to_string(), d)).collect();
    let entries = content_entries(sources.platforms.modrinth, game, &downloaded).await;
    let saved = (|| -> Result<(), ModpackError> {
        Provenance::update(&dirs.instance, |provenance| {
            for (kind, name) in &removed_keys {
                provenance.files.remove(&key(*kind, name));
            }
            for (kind, name, entry) in &entries {
                provenance.insert(*kind, name, entry.clone());
            }
        })?;
        plan.record.save(&dirs.instance)?;
        Ok(())
    })();
    if let Err(err) = saved {
        return Err(undo(dirs, &snapshot, &changed, err));
    }
    // The added packs' files the update replaced or removed aren't theirs any more. If
    // the record can't be saved it only names files that are gone, which removing an
    // added pack skips.
    let _ = super::addon::forget_missing(dirs);
    Ok(UpdateOutcome {
        snapshot,
        world_backups,
    })
}

/// Removes the files `plan` removes and moves the staged files into the game folder.
/// Every path changed so far is added to `changed`, also when the step that fails is
/// part way through it (the old file removed, the new one not moved in yet).
fn swap_in(
    game: &Path,
    staging: &Path,
    plan: &UpdatePlan,
    changed: &mut BTreeSet<String>,
) -> Result<(), ModpackError> {
    for path in &plan.removals {
        let target = game.join(path);
        match fs::remove_file(&target) {
            Ok(()) => {
                changed.insert(path.clone());
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(io_error(&target)(err)),
        }
    }
    let staged = plan.writes.iter().map(|(_, p)| p);
    let mut parents = super::import::ParentCheck::new(game)?;
    for path in staged {
        let from = staging.join(path);
        let to = game.join(path);
        parents.prepare(&to)?;
        // The rename replaces an older file at `to`.
        changed.insert(path.clone());
        crate::fsx::rename(&from, &to).map_err(io_error(&to))?;
    }
    Ok(())
}

/// Puts the instance back as it was before an update that stopped with `err`: each of
/// the `changed` game files is copied back from the snapshot, or removed when the update
/// added it, and `content.json` and `modpack.json` are restored. Returns the error to
/// show, which says whether that worked.
fn undo(
    dirs: &InstanceDirs,
    snapshot: &Path,
    changed: &BTreeSet<String>,
    err: ModpackError,
) -> ModpackError {
    let restore = |saved: &Path, target: &Path| -> std::io::Result<()> {
        if saved.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(saved, target).map(drop)
        } else if target.is_file() {
            fs::remove_file(target)
        } else {
            Ok(())
        }
    };
    let mut failed = Vec::new();
    for path in changed {
        let saved = snapshot.join("minecraft").join(path);
        if restore(&saved, &dirs.game.join(path)).is_err() {
            failed.push(path.clone());
        }
    }
    for file in [
        crate::content::provenance::FILE,
        super::RECORD_FILE,
        super::addon::ADDED_FILE,
    ] {
        if restore(&snapshot.join(file), &dirs.instance.join(file)).is_err() {
            failed.push(file.to_string());
        }
    }
    if failed.is_empty() {
        ModpackError::Invalid(format!(
            "The update stopped part way ({err}), so the files it had changed were put \
             back. The instance is as it was before the update."
        ))
    } else {
        ModpackError::Invalid(format!(
            "The update stopped part way ({err}), and these couldn't be put back: {}. \
             The files it replaced are in {}.",
            failed.join(", "),
            snapshot.display()
        ))
    }
}
