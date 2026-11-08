//! Installing a Modrinth version into an instance, with the same rules as the dashboard's
//! server mods (`core/src/mods/install.rs`):
//!
//! - a version that doesn't fit the instance (loader, Minecraft version) is refused unless
//!   forced;
//! - a mod's required dependencies are installed too: the version the dependency names,
//!   else the newest fitting release (or the newest fitting version), at most
//!   [`MAX_DEPENDENCY_DEPTH`] levels deep, skipping projects already installed and cycles;
//!   optional, incompatible and embedded dependencies never are;
//! - nothing is installed if a planned version is incompatible with an installed one;
//! - an older file of the same project is replaced once the new one is in place; a
//!   disabled mod stays disabled, an enabled resource pack keeps its place, and the
//!   selected shader pack stays selected.
//!
//! Files come only from Modrinth's CDN, through the [`Downloader`] (SHA-1 and size
//! checked, written to a temporary file and renamed), and their SHA-512 is checked before
//! they get their final name.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    sync::Mutex,
};

use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthDependencyType, ModrinthProject, ModrinthVersionType, RawVersion},
};

use super::{
    ContentError, ContentKind, InstanceDirs, MAX_FILE_BYTES, Modrinth, ModrinthError, Target,
    disabled_name, io_error,
    listing::refresh,
    modrinth::sort_newest_first,
    options::{self, PackChange},
    provenance::{Entry, ModrinthSource, Provenance, nanos},
    safe_download_name, shaders,
};
use crate::game::download::{CancelToken, DownloadItem, Downloader, Progress, ProgressSnapshot};

pub const MAX_DEPENDENCY_DEPTH: usize = 3;

/// The newest `release` among `versions` that fits `target`, or else the newest version
/// that fits.
pub fn pick_version(
    target: &Target,
    kind: ContentKind,
    mut versions: Vec<RawVersion>,
) -> Option<RawVersion> {
    versions.retain(|v| target.incompatibility(kind, v).is_none());
    sort_newest_first(&mut versions);
    let release = versions
        .iter()
        .position(|v| v.version_type == ModrinthVersionType::Release);
    match release {
        Some(i) => Some(versions.swap_remove(i)),
        None => versions.into_iter().next(),
    }
}

#[derive(Clone, Debug)]
pub struct InstallRequest {
    pub kind: ContentKind,
    pub version: RawVersion,
    /// The version's project, when the caller has it already.
    pub project: Option<ModrinthProject>,
    pub install_dependencies: bool,
    /// Install even if the version doesn't fit the instance.
    pub force: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub project: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct PlannedFile {
    pub kind: ContentKind,
    pub version: RawVersion,
    pub project: Option<ModrinthProject>,
    /// 0 for the requested version, 1 for its dependencies, and so on.
    pub depth: usize,
}

impl PlannedFile {
    pub fn title(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| self.version.name.clone())
    }
}

#[derive(Clone, Debug)]
pub struct InstallPlan {
    /// The requested version first, then its dependencies.
    pub files: Vec<PlannedFile>,
    pub skipped: Vec<Skipped>,
}

impl InstallPlan {
    /// The bytes to download.
    pub fn download_size(&self) -> u64 {
        self.files
            .iter()
            .filter_map(|f| f.version.primary_file())
            .map(|f| f.size)
            .sum()
    }
}

fn skipped(project: impl Into<String>, reason: impl Into<String>) -> Skipped {
    Skipped {
        project: project.into(),
        reason: reason.into(),
    }
}

/// The version of a required dependency to install, or why there is none.
async fn dependency_version(
    api: &Modrinth,
    target: &Target,
    project_id: &str,
    pinned: Option<RawVersion>,
) -> Result<Result<RawVersion, String>, ContentError> {
    if let Some(version) = pinned {
        return Ok(match target.incompatibility(ContentKind::Mod, &version) {
            None => Ok(version),
            Some(reason) => Err(format!("The required version doesn't fit: {reason}")),
        });
    }
    let versions = match api
        .versions(
            project_id,
            &target.accepted_loaders(ContentKind::Mod),
            std::slice::from_ref(&target.game_version),
        )
        .await
    {
        Ok(versions) => versions,
        Err(ModrinthError::NotFound(_)) => return Ok(Err("Not found on Modrinth".into())),
        Err(err) => return Err(err.into()),
    };
    Ok(
        pick_version(target, ContentKind::Mod, versions).ok_or_else(|| {
            format!(
                "No version for {} on Minecraft {}",
                target.loader.label(),
                target.game_version
            )
        }),
    )
}

/// Plans what installing `request` downloads. Matches the installed files to Modrinth
/// first, so installed projects are known.
pub async fn plan(
    api: &Modrinth,
    dirs: &InstanceDirs,
    target: &Target,
    request: InstallRequest,
) -> Result<InstallPlan, ContentError> {
    let kind = request.kind;
    if !request.force
        && let Some(reason) = target.incompatibility(kind, &request.version)
    {
        return Err(ContentError::Invalid(reason));
    }
    let (_, provenance, _) = refresh(Some(api), dirs, kind).await?;
    let installed = provenance.by_project(kind);

    let mut plan = vec![PlannedFile {
        kind,
        version: request.version,
        project: request.project,
        depth: 0,
    }];
    let mut seen: HashSet<String> = HashSet::from([plan[0].version.project_id.clone()]);
    let mut skipped_list = Vec::new();
    // Resource and shader packs have no dependencies Lodestone installs: a shader pack
    // that names Iris gets the shader mod offer instead.
    if request.install_dependencies && kind == ContentKind::Mod {
        let mut queue = VecDeque::from([0_usize]);
        while let Some(index) = queue.pop_front() {
            let depth = plan[index].depth + 1;
            let dependencies: Vec<_> = plan[index]
                .version
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == ModrinthDependencyType::Required)
                .cloned()
                .collect();
            for dependency in dependencies {
                let mut pinned = None;
                let project_id = match (&dependency.project_id, &dependency.version_id) {
                    (Some(project_id), _) => project_id.clone(),
                    (None, Some(version_id)) => match api.version(version_id).await {
                        Ok(version) => {
                            let project_id = version.project_id.clone();
                            pinned = Some(version);
                            project_id
                        }
                        Err(ModrinthError::NotFound(_)) => {
                            skipped_list.push(skipped(version_id.clone(), "Not found on Modrinth"));
                            continue;
                        }
                        Err(err) => return Err(err.into()),
                    },
                    (None, None) => {
                        let name = dependency
                            .file_name
                            .clone()
                            .unwrap_or_else(|| "A dependency".to_string());
                        skipped_list.push(skipped(
                            name.clone(),
                            format!("{name} isn't on Modrinth; add it yourself"),
                        ));
                        continue;
                    }
                };
                if installed.contains_key(&project_id) || seen.contains(&project_id) {
                    continue;
                }
                if depth > MAX_DEPENDENCY_DEPTH {
                    skipped_list.push(skipped(
                        project_id,
                        format!("More than {MAX_DEPENDENCY_DEPTH} levels of dependencies"),
                    ));
                    continue;
                }
                seen.insert(project_id.clone());
                if pinned.is_none()
                    && let Some(version_id) = &dependency.version_id
                {
                    pinned = match api.version(version_id).await {
                        Ok(version) => Some(version),
                        Err(ModrinthError::NotFound(_)) => None,
                        Err(err) => return Err(err.into()),
                    };
                }
                match dependency_version(api, target, &project_id, pinned).await? {
                    Ok(version) => {
                        plan.push(PlannedFile {
                            kind,
                            version,
                            project: None,
                            depth,
                        });
                        queue.push_back(plan.len() - 1);
                    }
                    Err(reason) => skipped_list.push(skipped(project_id, reason)),
                }
            }
        }
    }

    // Nothing is installed if a planned version is incompatible with an installed one.
    for planned in &plan {
        for dependency in &planned.version.dependencies {
            if dependency.dependency_type != ModrinthDependencyType::Incompatible {
                continue;
            }
            let conflict = provenance
                .files
                .values()
                .filter_map(|e| e.modrinth.as_ref())
                .find(|m| {
                    m.project_id != planned.version.project_id
                        && (dependency.project_id.as_deref() == Some(m.project_id.as_str())
                            || dependency.version_id.as_deref() == Some(m.version_id.as_str()))
                });
            if let Some(conflict) = conflict {
                return Err(ContentError::Invalid(format!(
                    "{} {} is incompatible with the installed {}. Remove it first.",
                    planned.version.name, planned.version.version_number, conflict.title
                )));
            }
        }
    }

    let missing: Vec<String> = plan
        .iter()
        .filter(|p| p.project.is_none())
        .map(|p| p.version.project_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        let projects: HashMap<String, ModrinthProject> = api
            .projects(&missing)
            .await?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        for planned in plan.iter_mut().filter(|p| p.project.is_none()) {
            planned.project = projects.get(&planned.version.project_id).cloned();
        }
    }
    Ok(InstallPlan {
        files: plan,
        skipped: skipped_list,
    })
}

/// How an install is going.
#[derive(Debug, Default)]
pub struct InstallProgress {
    pub downloads: Progress,
    label: Mutex<String>,
}

impl InstallProgress {
    pub fn label(&self) -> String {
        self.label.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set_label(&self, label: impl Into<String>) {
        *self.label.lock().unwrap_or_else(|p| p.into_inner()) = label.into();
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstallOutcome {
    /// Titles and file names.
    pub installed: Vec<(String, String)>,
    /// File names.
    pub removed: Vec<String>,
    pub skipped: Vec<Skipped>,
}

/// Downloads and puts in place every file of `plan`, dependencies first.
pub async fn execute(
    api: &Modrinth,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    plan: InstallPlan,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<InstallOutcome, ContentError> {
    let total_files = plan.files.len() as u64;
    let total_bytes = plan.download_size();
    let mut done_bytes = 0;
    let mut outcome = InstallOutcome {
        skipped: plan.skipped.clone(),
        ..InstallOutcome::default()
    };
    for (index, planned) in plan.files.iter().rev().enumerate() {
        progress.set_label(format!("Installing {}", planned.title()));
        progress.downloads.set(ProgressSnapshot {
            files_total: total_files,
            files_done: index as u64,
            bytes_total: total_bytes,
            bytes_done: done_bytes,
        });
        let (name, removed) =
            install_file(api, downloader, dirs, target, planned, progress, cancel).await?;
        done_bytes += planned.version.primary_file().map_or(0, |f| f.size);
        outcome.installed.push((planned.title(), name));
        outcome.removed.extend(removed);
    }
    progress.downloads.set(ProgressSnapshot {
        files_total: total_files,
        files_done: total_files,
        bytes_total: total_bytes,
        bytes_done: total_bytes,
    });
    outcome.installed.reverse();
    Ok(outcome)
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Installs one planned file. Returns its name and the names of the files it replaced.
async fn install_file(
    api: &Modrinth,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    planned: &PlannedFile,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<(String, Vec<String>), ContentError> {
    let kind = planned.kind;
    let version = &planned.version;
    let dir = dirs.content_dir(kind);
    let mut provenance = Provenance::load(&dirs.instance);
    let old: Vec<String> = provenance
        .by_project(kind)
        .remove(&version.project_id)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|name| on_disk(dirs, kind, &name))
        .collect();

    // The same version is installed already: keep it, drop the others.
    if let Some(same) = old.iter().find(|name| {
        provenance
            .get(kind, name)
            .and_then(|e| e.modrinth.as_ref())
            .is_some_and(|m| m.version_id == version.id)
    }) {
        let same = same.clone();
        let removed = remove_old(dirs, kind, &old, &same, &mut provenance)?;
        save(&provenance, dirs)?;
        return Ok((same, removed));
    }

    let file = version
        .primary_file()
        .ok_or_else(|| ContentError::Invalid(format!("{} has no files", version.name)))?;
    let url = api.cdn_url(&file.url)?;
    if file.size > MAX_FILE_BYTES {
        return Err(ContentError::Invalid(format!(
            "{} is larger than {} MB",
            file.filename,
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let sha1 = file
        .hashes
        .sha1
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let sha512 = file
        .hashes
        .sha512
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !is_hex(&sha1, 40) || !is_hex(&sha512, 128) {
        return Err(ContentError::Invalid(format!(
            "Modrinth sent no hashes for {}, so it can't be checked",
            file.filename
        )));
    }
    let name = safe_download_name(kind, &file.filename).ok_or_else(|| {
        ContentError::Invalid(format!(
            "{} isn't a file name that can be used",
            file.filename
        ))
    })?;
    let disabled = kind == ContentKind::Mod
        && !old.is_empty()
        && old.iter().all(|n| super::is_disabled_name(n));
    let target_name = if disabled { disabled_name(&name) } else { name };
    let target_path = dir.join(&target_name);
    if fs::symlink_metadata(&target_path).is_ok() && !old.contains(&target_name) {
        return Err(ContentError::Conflict(format!(
            "{target_name} already exists in {}. Remove it first.",
            kind.dir()
        )));
    }

    fs::create_dir_all(&dir).map_err(io_error(&dir))?;
    let staging = dir.join(format!(".lodestone-{}.download", sha1));
    let item = DownloadItem {
        url,
        dest: staging.clone(),
        sha1,
        size: Some(file.size),
        executable: false,
    };
    let result = downloader.fetch(&item, &progress.downloads, cancel).await;
    if let Err(err) = result {
        let _ = fs::remove_file(&staging);
        return Err(err.into());
    }
    let actual = hash::sha512_of(&staging).map_err(io_error(&staging))?;
    if !actual.eq_ignore_ascii_case(&sha512) {
        let _ = fs::remove_file(&staging);
        return Err(ContentError::Invalid(format!(
            "The download of {} is damaged: its SHA-512 hash doesn't match",
            file.filename
        )));
    }
    if target_path.exists() {
        // An older file of this project with the same name; Windows can't rename over it.
        fs::remove_file(&target_path).map_err(io_error(&target_path))?;
    }
    if let Err(err) = fs::rename(&staging, &target_path) {
        let _ = fs::remove_file(&staging);
        return Err(io_error(&target_path)(err));
    }

    let enabled_old = match kind {
        ContentKind::ResourcePack => {
            options::PackOptions::read(&dirs.game)
                .ok()
                .and_then(|packs| {
                    let enabled = packs.enabled_files(&target.game_version);
                    old.iter().find(|n| enabled.contains(n)).cloned()
                })
        }
        ContentKind::ShaderPack => target.shader_loader.and_then(|loader| {
            let selected = shaders::read_selection(&dirs.game, loader).ok()?.pack?;
            old.contains(&selected).then_some(selected)
        }),
        ContentKind::Mod => None,
    };
    let removed = remove_old(dirs, kind, &old, &target_name, &mut provenance)?;
    let modified = fs::metadata(&target_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(nanos);
    provenance.insert(
        kind,
        &target_name,
        Entry {
            size: file.size,
            modified,
            sha1: item.sha1.clone(),
            sha512: sha512.clone(),
            modrinth: Some(ModrinthSource::new(
                version,
                planned.project.as_ref(),
                &sha512,
            )),
            looked_up: false,
        },
    );
    save(&provenance, dirs)?;
    if let Some(from) = enabled_old.filter(|from| from != &target_name) {
        match kind {
            ContentKind::ResourcePack => {
                options::change_packs(
                    &dirs.game,
                    &target.game_version,
                    &PackChange::Rename {
                        from,
                        to: target_name.clone(),
                    },
                )
                .map_err(|err| ContentError::Invalid(err.to_string()))?;
            }
            ContentKind::ShaderPack => {
                if let Some(loader) = target.shader_loader {
                    shaders::write_selection(&dirs.game, loader, Some(&target_name))?;
                }
            }
            ContentKind::Mod => {}
        }
    }
    Ok((target_name, removed))
}

/// The name on disk of the file with provenance key name `name` (a mod may be disabled).
fn on_disk(dirs: &InstanceDirs, kind: ContentKind, name: &str) -> Option<String> {
    let dir = dirs.content_dir(kind);
    [name.to_string(), disabled_name(name)]
        .into_iter()
        .take(if kind == ContentKind::Mod { 2 } else { 1 })
        .find(|candidate| fs::symlink_metadata(dir.join(candidate)).is_ok())
}

fn save(provenance: &Provenance, dirs: &InstanceDirs) -> Result<(), ContentError> {
    provenance
        .save(&dirs.instance)
        .map_err(|err| ContentError::Invalid(err.to_string()))
}

/// Removes the `old` files except `keep`.
fn remove_old(
    dirs: &InstanceDirs,
    kind: ContentKind,
    old: &[String],
    keep: &str,
    provenance: &mut Provenance,
) -> Result<Vec<String>, ContentError> {
    let dir = dirs.content_dir(kind);
    let mut removed = Vec::new();
    for name in old.iter().filter(|n| n.as_str() != keep) {
        let path = dir.join(name);
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        result.map_err(io_error(&path))?;
        // `keep` may be the same file under its other name (`.disabled`).
        if super::enabled_name(name) != super::enabled_name(keep) {
            provenance.remove(kind, name);
        }
        removed.push(name.clone());
    }
    Ok(removed)
}
