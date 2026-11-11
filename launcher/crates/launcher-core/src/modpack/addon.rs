//! Adding a modpack's files to an instance that already exists: Fabulously Optimized's
//! client mods in an instance made for a server, say.
//!
//! An instance has at most one pack of its own, which `modpack.json` records (the pack it
//! was made from, or a server's client pack that "keep mods in step" follows). A pack
//! added on top of it is recorded apart, in `added_modpacks.json`, so the instance's own
//! pack, and its updates, never take the added files for theirs:
//!
//! - **The instance's pack wins.** A file it wrote, or a mod of a Modrinth project it
//!   has, is never added again or replaced, so a server's mods stay as the server has
//!   them and the instance can still join. When a later version of the instance's pack
//!   brings a project an added pack had, the update removes the added copy (see
//!   [`superseded`]).
//! - **Nothing the user has is replaced.** A file already at a path is kept, and a mod
//!   whose Modrinth project is already installed (at another version, say) isn't added
//!   twice. Only missing files are written.
//! - The instance's Minecraft version must be the pack's, and its loader the pack's
//!   loader (any version of it).
//!
//! Removing an added pack removes the files it wrote that are as it wrote them (a mod the
//! user disabled counts); files the user changed stay.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use lodestone_minecraft::mrpack::PackDownload;
use serde::{Deserialize, Serialize};

use super::{
    FileSource, ModpackError, ModpackRecord, OverrideFile, PackFile, PackInfo, cdn_ids,
    import::{ParentCheck, content_entries, download_files, extract_overrides, sha1_on_disk},
    io_error, slash_path,
};
use crate::{
    content::{
        ContentKind, InstanceDirs, Platforms, disabled_name, enabled_name,
        install::InstallProgress,
        is_disabled_name,
        provenance::{Provenance, key},
    },
    game::download::{CancelToken, Downloader},
    instances::Instance,
    storage::{self, Document, Migration, StorageError},
};

/// The record of the packs added to an instance, next to `modpack.json`.
pub const ADDED_FILE: &str = "added_modpacks.json";

/// A pack added to an instance: where it came from, and the files it wrote (only those:
/// not the files it skipped).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedPack {
    /// Names this pack among the instance's added packs.
    pub id: String,
    #[serde(flatten)]
    pub record: ModpackRecord,
}

/// `added_modpacks.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddedPacks {
    #[serde(default)]
    pub packs: Vec<AddedPack>,
}

impl Document for AddedPacks {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}

impl AddedPacks {
    pub fn path(instance_dir: &Path) -> PathBuf {
        instance_dir.join(ADDED_FILE)
    }

    /// The packs added to the instance; none when it has no readable record.
    pub fn load(instance_dir: &Path) -> Self {
        let path = Self::path(instance_dir);
        if !path.is_file() {
            return Self::default();
        }
        storage::load::<AddedPacks>(&path)
            .map(|loaded| loaded.document)
            .unwrap_or_default()
    }

    /// Saves the record, or removes it when no pack is left.
    pub fn save(&self, instance_dir: &Path) -> Result<(), StorageError> {
        let path = Self::path(instance_dir);
        if self.packs.is_empty() {
            return match fs::remove_file(&path) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                    Err(StorageError::Io { path, source: err })
                }
                _ => Ok(()),
            };
        }
        storage::save(&path, self)
    }

    pub fn get(&self, id: &str) -> Option<&AddedPack> {
        self.packs.iter().find(|p| p.id == id)
    }

    /// The added pack `reference` names: its id, its Modrinth project id or slug, its
    /// CurseForge project id, or its title (any case).
    pub fn find(&self, reference: &str) -> Option<&AddedPack> {
        let reference = reference.trim();
        self.get(reference).or_else(|| {
            self.packs.iter().find(|p| {
                let r = &p.record;
                (!r.project_id.is_empty() && r.project_id == reference)
                    || (!r.slug.is_empty() && r.slug.eq_ignore_ascii_case(reference))
                    || r.title.eq_ignore_ascii_case(reference)
                    || r.curseforge
                        .as_ref()
                        .is_some_and(|cf| cf.project_id.to_string() == reference)
            })
        })
    }

    /// The added pack that is the same project as `record` (any version of it).
    pub fn same_project(&self, record: &ModpackRecord) -> Option<&AddedPack> {
        self.packs.iter().find(|p| same_project(&p.record, record))
    }
}

/// Whether two records are versions of the same Modrinth or CurseForge project.
fn same_project(a: &ModpackRecord, b: &ModpackRecord) -> bool {
    if !a.project_id.is_empty() && a.project_id == b.project_id {
        return true;
    }
    matches!((&a.curseforge, &b.curseforge), (Some(x), Some(y)) if x.project_id == y.project_id)
}

/// Why `pack` can't be added to `instance`, if it can't: another Minecraft version, or
/// another loader.
pub fn incompatibility(instance: &Instance, pack: &PackInfo) -> Option<String> {
    if pack.game_version != instance.game_version {
        return Some(format!(
            "{} is for Minecraft {}, and this instance is {}.",
            pack.name, pack.game_version, instance.game_version
        ));
    }
    let wanted = pack.loader.as_ref().map(|(kind, _)| *kind);
    match (wanted, instance.loader.kind()) {
        (None, _) => None,
        (Some(wanted), Some(has)) if wanted == has => None,
        (Some(wanted), Some(has)) => Some(format!(
            "{} needs {}, and this instance uses {}.",
            pack.name,
            wanted.label(),
            has.label()
        )),
        (Some(wanted), None) => Some(format!(
            "{} needs {}, and this instance has no mod loader.",
            pack.name,
            wanted.label()
        )),
    }
}

/// What adding a pack will do. Paths are relative to the game folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AddPlan {
    /// The record of the added pack, with the files it will write.
    pub pack: AddedPack,
    /// Mods and other downloaded files added.
    pub added: Vec<String>,
    /// Configs and other override files written.
    pub configs: Vec<String>,
    /// Files skipped because the instance's own pack (a server's) has them, or has
    /// another version of the same mod.
    pub kept_instance_pack: Vec<String>,
    /// Files skipped because the instance already has them: the same file, another
    /// version of the same mod, or the user's own file at that path.
    pub already_there: Vec<String>,
    downloads: Vec<(PackDownload, String)>,
    overrides: Vec<(OverrideFile, String)>,
}

impl AddPlan {
    /// Whether adding writes any file.
    pub fn changes_files(&self) -> bool {
        !self.downloads.is_empty() || !self.overrides.is_empty()
    }

    pub fn download_size(&self) -> u64 {
        self.downloads.iter().map(|(d, _)| d.size).sum()
    }

    /// The files written, on disk.
    pub fn written(&self) -> impl Iterator<Item = &str> {
        self.downloads
            .iter()
            .map(|(_, p)| p.as_str())
            .chain(self.overrides.iter().map(|(_, p)| p.as_str()))
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

/// Whether `path` names the same file as one of `paths`, `.disabled` or not.
fn has_path(paths: &BTreeMap<String, PackFile>, path: &str) -> bool {
    paths.contains_key(path)
        || paths.contains_key(&disabled_name(path))
        || paths.contains_key(&enabled_name(path))
}

/// The Modrinth project of a download, from its CDN URL.
fn download_project(download: &PackDownload) -> Option<String> {
    download
        .urls
        .iter()
        .find_map(|u| cdn_ids(u).map(|(p, _)| p))
}

/// The plan for adding `pack` (from the version `record` describes) to the instance at
/// `dirs`. The caller checks [`incompatibility`] first.
pub fn plan(dirs: &InstanceDirs, pack: &PackInfo, record: ModpackRecord) -> AddPlan {
    let game = &dirs.game;
    let own = ModpackRecord::load(&dirs.instance).unwrap_or_default();
    let own_projects: BTreeSet<&str> = own
        .files
        .values()
        .filter_map(|f| f.project.as_deref())
        .collect();
    // The Modrinth projects of the mods installed, from content.json, whose files are
    // still there.
    let provenance = Provenance::load(&dirs.instance);
    let installed_projects: BTreeSet<String> = provenance
        .files
        .iter()
        .filter(|(path, _)| path.starts_with("mods/"))
        .filter(|(path, _)| on_disk(game, path).is_some())
        .filter_map(|(_, entry)| entry.modrinth.as_ref().map(|m| m.project_id.clone()))
        .collect();

    let mut plan = AddPlan::default();
    let overridden: BTreeSet<&str> = pack.overrides.iter().map(|o| o.path.as_str()).collect();
    for download in &pack.downloads {
        let path = slash_path(&download.path);
        if overridden.contains(path.as_str()) {
            // The pack's override replaces it.
            continue;
        }
        let project = download_project(download);
        if has_path(&own.files, &path)
            || project.as_deref().is_some_and(|p| own_projects.contains(p))
        {
            plan.kept_instance_pack.push(path);
            continue;
        }
        if on_disk(game, &path).is_some()
            || project
                .as_ref()
                .is_some_and(|p| installed_projects.contains(p))
        {
            plan.already_there.push(path);
            continue;
        }
        plan.added.push(path.clone());
        plan.downloads.push((download.clone(), path));
    }
    for file in &pack.overrides {
        if has_path(&own.files, &file.path) {
            plan.kept_instance_pack.push(file.path.clone());
        } else if on_disk(game, &file.path).is_some() {
            plan.already_there.push(file.path.clone());
        } else {
            plan.configs.push(file.path.clone());
            plan.overrides.push((file.clone(), file.path.clone()));
        }
    }

    let mut files = BTreeMap::new();
    for (download, path) in &plan.downloads {
        files.insert(
            path.clone(),
            PackFile {
                sha1: download.sha1.clone(),
                source: FileSource::Index,
                project: download_project(download),
            },
        );
    }
    for (file, path) in &plan.overrides {
        files.insert(
            path.clone(),
            PackFile {
                sha1: file.sha1.clone(),
                source: FileSource::Override,
                project: None,
            },
        );
    }
    plan.pack = AddedPack {
        id: format!("{:016x}", u64::from_le_bytes(crate::random_bytes())),
        record: ModpackRecord { files, ..record },
    };
    plan
}

/// Adds the files `plan` writes (for the pack at `pack_path`) to the instance at `dirs`,
/// and records the pack. Everything is downloaded into a staging folder first; if moving
/// the files into the game folder fails part way, the files moved so far are removed.
#[allow(clippy::too_many_arguments)]
pub async fn apply(
    platforms: Platforms<'_>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    plan: &AddPlan,
    pack_path: &Path,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<AddedPack, ModpackError> {
    let staging = dirs.instance.join(format!(
        ".lodestone-add-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    ));
    fs::create_dir_all(&staging).map_err(io_error(&staging))?;
    let result = apply_staged(
        platforms, downloader, dirs, plan, pack_path, progress, cancel, &staging,
    )
    .await;
    let _ = fs::remove_dir_all(&staging);
    result
}

#[allow(clippy::too_many_arguments)]
async fn apply_staged(
    platforms: Platforms<'_>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    plan: &AddPlan,
    pack_path: &Path,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &Path,
) -> Result<AddedPack, ModpackError> {
    let game = &dirs.game;
    fs::create_dir_all(game).map_err(io_error(game))?;
    if !plan.downloads.is_empty() {
        progress.set_label(format!("Downloading {}", plan.pack.record.title));
        let downloads: Vec<(PackDownload, PathBuf)> = plan
            .downloads
            .iter()
            .map(|(d, dest)| (d.clone(), staging.join(dest)))
            .collect();
        download_files(platforms, downloader, staging, &downloads, progress, cancel).await?;
    }
    progress.set_label("Copying the pack's configs");
    let overrides: Vec<(OverrideFile, PathBuf)> = plan
        .overrides
        .iter()
        .map(|(o, dest)| (o.clone(), staging.join(dest)))
        .collect();
    extract_overrides(pack_path, staging, &overrides, cancel)?;
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }

    // Into the game folder. A file that appeared since the plan was made (the user
    // added it meanwhile) is kept, and the pack doesn't record it.
    progress.set_label(format!("Adding {}", plan.pack.record.title));
    let mut moved: Vec<String> = Vec::new();
    let moved_in = (|| -> Result<(), ModpackError> {
        let mut parents = ParentCheck::new(game)?;
        for path in plan.written() {
            let to = game.join(path);
            if on_disk(game, path).is_some() {
                continue;
            }
            parents.prepare(&to)?;
            crate::fsx::rename(staging.join(path), &to).map_err(io_error(&to))?;
            moved.push(path.to_string());
        }
        Ok(())
    })();
    let undo = |moved: &[String]| {
        for path in moved {
            let _ = fs::remove_file(game.join(path));
        }
    };
    if let Err(err) = moved_in {
        undo(&moved);
        return Err(err);
    }

    // The records.
    let moved_set: BTreeSet<&str> = moved.iter().map(String::as_str).collect();
    let downloaded: Vec<(String, &PackDownload)> = plan
        .downloads
        .iter()
        .filter(|(_, p)| moved_set.contains(p.as_str()))
        .map(|(d, p)| (p.clone(), d))
        .collect();
    let entries = content_entries(platforms.modrinth, game, &downloaded).await;
    let mut added = plan.pack.clone();
    added
        .record
        .files
        .retain(|path, _| moved_set.contains(path.as_str()));
    let saved = (|| -> Result<(), ModpackError> {
        Provenance::update(&dirs.instance, |provenance| {
            for (kind, name, entry) in &entries {
                provenance.insert(*kind, name, entry.clone());
            }
        })?;
        let mut packs = AddedPacks::load(&dirs.instance);
        packs.packs.push(added.clone());
        packs.save(&dirs.instance)?;
        Ok(())
    })();
    if let Err(err) = saved {
        undo(&moved);
        return Err(err);
    }
    Ok(added)
}

/// What removing an added pack did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoveOutcome {
    pub removed: Vec<String>,
    /// The pack's files the user changed since, left as they are.
    pub kept_changed: Vec<String>,
}

/// What removing the pack `id` would do, without doing it: the files it wrote that are
/// as it wrote them (paths on disk), and those the user changed since. A file the
/// instance's own pack records now (the server added the same file since) is in
/// neither. `None` when the instance has no added pack `id`.
pub fn removal(dirs: &InstanceDirs, id: &str) -> Option<RemoveOutcome> {
    let packs = AddedPacks::load(&dirs.instance);
    let pack = packs.get(id)?;
    let own = ModpackRecord::load(&dirs.instance).unwrap_or_default();
    let mut outcome = RemoveOutcome::default();
    for (path, file) in &pack.record.files {
        let Some((actual, sha1)) = on_disk(&dirs.game, path) else {
            continue;
        };
        if has_path(&own.files, path) {
            continue;
        }
        if sha1.eq_ignore_ascii_case(&file.sha1) {
            outcome.removed.push(actual);
        } else {
            outcome.kept_changed.push(actual);
        }
    }
    Some(outcome)
}

/// Removes the pack `id` from the instance at `dirs`: the files [`removal`] names, and
/// its record.
pub fn remove(dirs: &InstanceDirs, id: &str) -> Result<RemoveOutcome, ModpackError> {
    let Some(outcome) = removal(dirs, id) else {
        return Ok(RemoveOutcome::default());
    };
    let mut removed_keys = Vec::new();
    for actual in &outcome.removed {
        let target = dirs.game.join(actual);
        fs::remove_file(&target).map_err(io_error(&target))?;
        if let Some((dir, name)) = actual.split_once('/')
            && let Some(kind) = ContentKind::ALL.into_iter().find(|k| k.dir() == dir)
        {
            removed_keys.push(key(kind, &enabled_name(name)));
        }
    }
    Provenance::update(&dirs.instance, |provenance| {
        for key in &removed_keys {
            provenance.files.remove(key);
        }
    })?;
    let mut packs = AddedPacks::load(&dirs.instance);
    packs.packs.retain(|p| p.id != id);
    packs.save(&dirs.instance)?;
    Ok(outcome)
}

/// The mods added packs wrote that `pack`, the instance's own pack's next version,
/// supersedes: the same Modrinth project at another path, as the added pack wrote them
/// (paths on disk). An update of the instance's pack removes them, so the instance
/// doesn't load two versions of a mod. The added pack's copies the user changed stay.
pub fn superseded(dirs: &InstanceDirs, pack: &PackInfo) -> Vec<String> {
    let packs = AddedPacks::load(&dirs.instance);
    if packs.packs.is_empty() {
        return Vec::new();
    }
    let paths: BTreeSet<String> = pack
        .downloads
        .iter()
        .map(|d| slash_path(&d.path))
        .chain(pack.overrides.iter().map(|o| o.path.clone()))
        .collect();
    let projects: BTreeSet<String> = pack.downloads.iter().filter_map(download_project).collect();
    let mut found = Vec::new();
    for added in &packs.packs {
        for (path, file) in &added.record.files {
            let Some(project) = &file.project else {
                continue;
            };
            if !projects.contains(project) || paths.contains(path) {
                continue;
            }
            if let Some((actual, sha1)) = on_disk(&dirs.game, path)
                && sha1.eq_ignore_ascii_case(&file.sha1)
            {
                found.push(actual);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Drops the files that are gone from the added packs' records (an update of the
/// instance's pack replaced or removed them). The packs stay, also with no files left,
/// so the user still sees what they added.
pub fn forget_missing(dirs: &InstanceDirs) -> Result<(), StorageError> {
    let mut packs = AddedPacks::load(&dirs.instance);
    if packs.packs.is_empty() {
        return Ok(());
    }
    let own = ModpackRecord::load(&dirs.instance).unwrap_or_default();
    let before = packs.clone();
    for pack in &mut packs.packs {
        pack.record.files.retain(|path, file| {
            !has_path(&own.files, path)
                && on_disk(&dirs.game, path).is_some_and(|(_, sha1)| {
                    // A file the instance's pack replaced at the same path isn't the
                    // added pack's any more.
                    file.source == FileSource::Override || sha1.eq_ignore_ascii_case(&file.sha1)
                })
        });
    }
    if packs == before {
        return Ok(());
    }
    packs.save(&dirs.instance)
}
