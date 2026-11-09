//! Installing a CurseForge file into an instance, by the same rules as
//! [`super::install`] for Modrinth: a file that doesn't fit is refused unless forced, a
//! mod's required dependencies come too (the newest fitting release, else the newest
//! fitting file, at most [`MAX_DEPENDENCY_DEPTH`] levels deep), nothing is installed if a
//! planned file is incompatible with an installed one, an older file of the same project
//! is replaced, and a mod with the same id as an installed one replaces it (a dependency
//! whose id is installed already isn't added).
//!
//! Some authors don't let launchers download their files. Such a file is first looked
//! for on Modrinth by its SHA-1: the same bytes there are downloaded from Modrinth's CDN.
//! Otherwise it is left in [`CfInstallPlan::blocked`]: the app sends the user to the
//! file's page on curseforge.com, and [`find_downloaded`] spots the file once their
//! browser has saved it, for [`install_downloaded`] to put in place after checking it.
//! No CDN address is ever made up for such a file.
//!
//! Downloads come only from CurseForge's CDN (or Modrinth's), through the
//! [`Downloader`] (SHA-1 and size checked); the fingerprint is checked too when
//! CurseForge gave one, and the SHA-512 is computed for `content.json`.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use lodestone_minecraft::{hash, modrinth::RawVersion};

use super::{
    ContentError, ContentKind, CurseForge, CurseForgeError, InstanceDirs, MAX_FILE_BYTES, Modrinth,
    Platforms, Target,
    curseforge::{self, CfFile, CfMod},
    install::{
        InstallOutcome, InstallProgress, MAX_DEPENDENCY_DEPTH, Placed, Skipped, is_hex, on_disk,
        place, remove_old, same_mod, save, target_name,
    },
    io_error,
    listing::refresh,
    provenance::{CurseForgeSource, Entry, ModrinthSource, Provenance},
    safe_download_name,
};
use crate::game::download::{CancelToken, DownloadItem, Downloader, ProgressSnapshot};

#[derive(Clone, Debug)]
pub struct CfInstallRequest {
    pub kind: ContentKind,
    pub file: CfFile,
    /// The file's project, when the caller has it already.
    pub project: Option<CfMod>,
    pub install_dependencies: bool,
    /// Install even if the file doesn't fit the instance.
    pub force: bool,
}

#[derive(Clone, Debug)]
pub struct CfPlannedFile {
    pub kind: ContentKind,
    pub file: CfFile,
    pub project: Option<CfMod>,
    /// The same file on Modrinth, for one CurseForge doesn't let launchers download.
    pub on_modrinth: Option<RawVersion>,
    /// 0 for the requested file, 1 for its dependencies, and so on.
    pub depth: usize,
}

impl CfPlannedFile {
    pub fn title(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| self.file.display_name_or_file().to_string())
    }

    /// The project's page on curseforge.com.
    pub fn project_page(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.page_url())
            .unwrap_or_else(|| format!("https://www.curseforge.com/projects/{}", self.file.mod_id))
    }

    /// Where the user downloads it by hand.
    pub fn file_page(&self) -> String {
        self.file.page_url(&self.project_page())
    }

    /// Whether the launcher can download it (from CurseForge, or the same bytes from
    /// Modrinth).
    pub fn downloadable(&self) -> bool {
        self.file.downloadable() || self.on_modrinth.is_some()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CfInstallPlan {
    /// What the launcher downloads: the requested file first, then its dependencies.
    pub files: Vec<CfPlannedFile>,
    /// What the user has to download from curseforge.com.
    pub blocked: Vec<CfPlannedFile>,
    pub skipped: Vec<Skipped>,
}

impl CfInstallPlan {
    pub fn download_size(&self) -> u64 {
        self.files.iter().map(|f| f.file.file_length).sum()
    }
}

fn skipped(project: impl Into<String>, reason: impl Into<String>) -> Skipped {
    Skipped {
        project: project.into(),
        reason: reason.into(),
    }
}

/// The newest file of `files` that fits: a release if one fits, else any. Files the
/// launcher can download are preferred over ones it can't.
pub fn pick_file(target: &Target, kind: ContentKind, mut files: Vec<CfFile>) -> Option<CfFile> {
    files.retain(|f| target.curseforge_incompatibility(kind, f).is_none());
    curseforge::sort_newest_first(&mut files);
    let rank = |f: &CfFile| (!f.downloadable(), f.release_type != 1);
    // Stable: the newest of the best rank.
    files.sort_by_key(rank);
    files.into_iter().next()
}

/// The single loader CurseForge can filter by for `target`, if there is one.
fn loader_filter(target: &Target, kind: ContentKind) -> Option<crate::instances::Loader> {
    match (
        kind,
        curseforge::accepted_loaders(target.loader, &target.game_version).as_slice(),
    ) {
        (ContentKind::Mod, [only]) => Some(*only),
        _ => None,
    }
}

/// Plans what installing `request` downloads. Matches the installed files first (on both
/// platforms), so installed projects are known.
pub async fn plan(
    cf: &CurseForge,
    platforms: Platforms<'_>,
    dirs: &InstanceDirs,
    target: &Target,
    request: CfInstallRequest,
) -> Result<CfInstallPlan, ContentError> {
    let kind = request.kind;
    if !request.force
        && let Some(reason) = target.curseforge_incompatibility(kind, &request.file)
    {
        return Err(ContentError::Invalid(reason));
    }
    let (_, provenance, _) = refresh(platforms, dirs, kind).await?;
    let installed = provenance.by_curseforge_project(kind);

    let mut plan = vec![CfPlannedFile {
        kind,
        file: request.file,
        project: request.project,
        on_modrinth: None,
        depth: 0,
    }];
    let mut seen: HashSet<u64> = HashSet::from([plan[0].file.mod_id]);
    let mut skipped_list = Vec::new();
    if request.install_dependencies && kind == ContentKind::Mod {
        let mut queue = VecDeque::from([0_usize]);
        while let Some(index) = queue.pop_front() {
            let depth = plan[index].depth + 1;
            let dependencies: Vec<u64> = plan[index]
                .file
                .dependencies
                .iter()
                .filter(|d| d.required())
                .map(|d| d.mod_id)
                .collect();
            for project in dependencies {
                if installed.contains_key(&project) || seen.contains(&project) {
                    continue;
                }
                seen.insert(project);
                if depth > MAX_DEPENDENCY_DEPTH {
                    skipped_list.push(skipped(
                        project.to_string(),
                        format!("More than {MAX_DEPENDENCY_DEPTH} levels of dependencies"),
                    ));
                    continue;
                }
                let files = match cf
                    .files(
                        project,
                        Some(&target.game_version),
                        loader_filter(target, kind),
                    )
                    .await
                {
                    Ok(files) => files,
                    Err(CurseForgeError::NotFound(_)) => {
                        skipped_list.push(skipped(project.to_string(), "Not found on CurseForge"));
                        continue;
                    }
                    Err(err) => return Err(err.into()),
                };
                match pick_file(target, kind, files) {
                    Some(file) => {
                        plan.push(CfPlannedFile {
                            kind,
                            file,
                            project: None,
                            on_modrinth: None,
                            depth,
                        });
                        queue.push_back(plan.len() - 1);
                    }
                    None => skipped_list.push(skipped(
                        project.to_string(),
                        format!(
                            "No file for {} on Minecraft {}",
                            target.loader.label(),
                            target.game_version
                        ),
                    )),
                }
            }
        }
    }

    // Nothing is installed if a planned file is incompatible with an installed project.
    for planned in &plan {
        for dependency in planned
            .file
            .dependencies
            .iter()
            .filter(|d| d.incompatible())
        {
            if dependency.mod_id == planned.file.mod_id {
                continue;
            }
            let conflict = provenance
                .files
                .values()
                .filter_map(|e| e.curseforge.as_ref())
                .find(|c| c.project_id == dependency.mod_id);
            if let Some(conflict) = conflict {
                return Err(ContentError::Invalid(format!(
                    "{} is incompatible with the installed {}. Remove it first.",
                    planned.file.display_name_or_file(),
                    conflict.title
                )));
            }
        }
    }

    let missing: Vec<u64> = plan
        .iter()
        .filter(|p| p.project.is_none())
        .map(|p| p.file.mod_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        let projects: HashMap<u64, CfMod> = cf
            .projects(&missing)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect();
        for planned in plan.iter_mut().filter(|p| p.project.is_none()) {
            planned.project = projects.get(&planned.file.mod_id).cloned();
        }
    }

    // Files the author keeps on CurseForge: the same bytes on Modrinth, else by hand.
    if let Some(modrinth) = platforms.modrinth {
        let hashes: Vec<String> = plan
            .iter()
            .filter(|p| !p.file.downloadable())
            .filter_map(|p| p.file.sha1())
            .collect();
        if !hashes.is_empty() {
            // Modrinth being unreachable only means the user downloads them.
            let found = modrinth.versions_by_sha1(&hashes).await.unwrap_or_default();
            for planned in plan.iter_mut().filter(|p| !p.file.downloadable()) {
                planned.on_modrinth = planned
                    .file
                    .sha1()
                    .and_then(|sha1| found.get(&sha1).cloned())
                    .filter(|version| modrinth_file(version, &planned.file).is_some());
            }
        }
    }
    let (files, blocked): (Vec<_>, Vec<_>) = plan.into_iter().partition(|p| p.downloadable());
    Ok(CfInstallPlan {
        files,
        blocked,
        skipped: skipped_list,
    })
}

/// The file of a Modrinth `version` with the same SHA-1 as `file`.
fn modrinth_file<'a>(
    version: &'a RawVersion,
    file: &CfFile,
) -> Option<&'a lodestone_minecraft::modrinth::RawVersionFile> {
    let sha1 = file.sha1()?;
    version.files.iter().find(|f| {
        f.hashes
            .sha1
            .as_deref()
            .is_some_and(|h| h.eq_ignore_ascii_case(&sha1))
    })
}

/// Downloads and puts in place every downloadable file of `plan`, dependencies first.
/// The blocked ones are left for [`install_downloaded`].
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    plan: &CfInstallPlan,
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
        done_bytes += planned.file.file_length;
        match install_file(
            cf, modrinth, downloader, dirs, target, planned, progress, cancel,
        )
        .await?
        {
            Placed::Installed { name, removed } => {
                outcome.installed.push((planned.title(), name));
                outcome.removed.extend(removed);
            }
            Placed::AlreadyInstalled { name } => outcome.skipped.push(skipped(
                planned.title(),
                format!("Already installed as {name}"),
            )),
        }
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

/// The files of `kind` from `project` on disk, and the one that is `file_id` if any.
fn installed_files(
    dirs: &InstanceDirs,
    kind: ContentKind,
    provenance: &Provenance,
    project: u64,
    file_id: u64,
) -> (Vec<String>, Option<String>) {
    let old: Vec<String> = provenance
        .by_curseforge_project(kind)
        .remove(&project)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|name| on_disk(dirs, kind, &name))
        .collect();
    let same = old
        .iter()
        .find(|name| {
            provenance
                .get(kind, name)
                .and_then(|e| e.curseforge.as_ref())
                .is_some_and(|c| c.file_id == file_id)
        })
        .cloned();
    (old, same)
}

#[allow(clippy::too_many_arguments)]
async fn install_file(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    planned: &CfPlannedFile,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<Placed, ContentError> {
    let kind = planned.kind;
    let file = &planned.file;
    let dir = dirs.content_dir(kind);
    let mut provenance = Provenance::load(&dirs.instance);
    let (old, same) = installed_files(dirs, kind, &provenance, file.mod_id, file.id);
    if let Some(same) = same {
        let removed = remove_old(dirs, kind, &old, &same, &mut provenance)?;
        save(dirs, kind, &removed, &same, None)?;
        return Ok(Placed::Installed {
            name: same,
            removed,
        });
    }

    let (url, via_modrinth) = if file.downloadable() {
        (cf.download_url(file)?, None)
    } else {
        let version = planned.on_modrinth.as_ref().ok_or_else(|| {
            ContentError::Invalid(format!(
                "{} can only be downloaded from CurseForge's website",
                file.file_name
            ))
        })?;
        let modrinth = modrinth.ok_or_else(|| {
            ContentError::Invalid("Modrinth can't be reached right now".to_string())
        })?;
        let found = modrinth_file(version, file).ok_or_else(|| {
            ContentError::Invalid(format!("{} isn't on Modrinth after all", file.file_name))
        })?;
        (modrinth.cdn_url(&found.url)?, Some(version))
    };
    if file.file_length > MAX_FILE_BYTES {
        return Err(ContentError::Invalid(format!(
            "{} is larger than {} MB",
            file.file_name,
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let sha1 = file.sha1().ok_or_else(|| {
        ContentError::Invalid(format!(
            "CurseForge sent no SHA-1 for {}, so it can't be checked",
            file.file_name
        ))
    })?;
    let name = safe_download_name(kind, &file.file_name).ok_or_else(|| {
        ContentError::Invalid(format!(
            "{} isn't a file name that can be used",
            file.file_name
        ))
    })?;
    target_name(dirs, kind, &name, &old)?;

    fs::create_dir_all(&dir).map_err(io_error(&dir))?;
    let staging = dir.join(format!(".lodestone-{sha1}.download"));
    let item = DownloadItem {
        url,
        dest: staging.clone(),
        sha1: sha1.clone(),
        size: (file.file_length > 0).then_some(file.file_length),
        executable: false,
    };
    if let Err(err) = downloader.fetch(&item, &progress.downloads, cancel).await {
        let _ = fs::remove_file(&staging);
        return Err(err.into());
    }
    let source = CurseForgeSource::new(file, planned.project.as_ref());
    let modrinth_source = via_modrinth.map(|version| {
        let sha512 = modrinth_file(version, file)
            .and_then(|f| f.hashes.sha512.clone())
            .unwrap_or_default();
        ModrinthSource::new(version, None, &sha512)
    });
    finish(
        dirs,
        target,
        planned,
        &staging,
        &name,
        old,
        source,
        modrinth_source,
    )
}

/// Checks the downloaded `staging` file against `planned` (fingerprint), then puts it in
/// place as [`install_file`] does.
#[allow(clippy::too_many_arguments)]
fn finish(
    dirs: &InstanceDirs,
    target: &Target,
    planned: &CfPlannedFile,
    staging: &Path,
    name: &str,
    mut old: Vec<String>,
    source: CurseForgeSource,
    modrinth: Option<ModrinthSource>,
) -> Result<Placed, ContentError> {
    let kind = planned.kind;
    let fail = |err: ContentError| {
        let _ = fs::remove_file(staging);
        Err(err)
    };
    let print = match curseforge::fingerprint_file(staging) {
        Ok(print) => print,
        Err(err) => return fail(io_error(staging)(err)),
    };
    if planned.file.file_fingerprint != 0 && u64::from(print) != planned.file.file_fingerprint {
        return fail(ContentError::Invalid(format!(
            "The download of {} isn't the file CurseForge describes",
            planned.file.file_name
        )));
    }
    let sha512 = match hash::sha512_of(staging) {
        Ok(sha512) => sha512,
        Err(err) => return fail(io_error(staging)(err)),
    };
    let sha1 = match hash::sha1_of(staging) {
        Ok(sha1) => sha1,
        Err(err) => return fail(io_error(staging)(err)),
    };
    let same = match same_mod(dirs, kind, staging, &old) {
        Ok(same) => same,
        Err(err) => return fail(err),
    };
    if planned.depth > 0
        && let Some(installed) = same.first()
    {
        let _ = fs::remove_file(staging);
        return Ok(Placed::AlreadyInstalled {
            name: installed.clone(),
        });
    }
    old.extend(same);
    let final_name = match target_name(dirs, kind, name, &old) {
        Ok(final_name) => final_name,
        Err(err) => return fail(err),
    };
    let size = fs::metadata(staging).map(|m| m.len()).unwrap_or_default();
    let removed = place(dirs, target, kind, staging, &final_name, &old, |modified| {
        Entry {
            size,
            modified,
            sha1,
            sha512,
            modrinth,
            curseforge: Some(source),
            fingerprint: Some(print),
            ..Entry::default()
        }
    })?;
    Ok(Placed::Installed {
        name: final_name,
        removed,
    })
}

/// Whether the file at `path` is `planned`'s file: its SHA-1 matches CurseForge's (or
/// its fingerprint, when CurseForge gave no SHA-1).
pub fn is_planned_file(path: &Path, planned: &CfPlannedFile) -> bool {
    match planned.file.sha1() {
        Some(expected) => {
            hash::sha1_of(path).is_ok_and(|actual| actual.eq_ignore_ascii_case(&expected))
        }
        None => {
            planned.file.file_fingerprint != 0
                && curseforge::fingerprint_file(path)
                    .is_ok_and(|print| u64::from(print) == planned.file.file_fingerprint)
        }
    }
}

/// A file name compared loosely: lowercase, with `-+._` and whitespace runs as one space.
/// Browsers rename downloads (`mod (1).jar`) and CurseForge's names vary, so this only
/// narrows which files are hashed.
fn loose_name(name: &str) -> String {
    let mapped: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if matches!(c, '-' | '+' | '.' | '_') {
                ' '
            } else {
                c
            }
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `stem` without the " (1)" a browser adds when a file is downloaded again.
fn without_copy_number(stem: &str) -> &str {
    if let Some(open) = stem.rfind(" (")
        && let Some(number) = stem[open + 2..].strip_suffix(')')
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
    {
        return &stem[..open];
    }
    stem
}

/// Looks in `folder` (not its subfolders) for the `blocked` files the user downloaded by
/// hand: files named like one of them whose SHA-1 matches. Returns (index in `blocked`,
/// path) for each found.
pub fn find_downloaded(folder: &Path, blocked: &[CfPlannedFile]) -> Vec<(usize, PathBuf)> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut candidates: Vec<(PathBuf, String)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| (e.path(), e.file_name().to_string_lossy().into_owned()))
        .collect();
    candidates.sort();
    let mut found = Vec::new();
    for (index, planned) in blocked.iter().enumerate() {
        let wanted = &planned.file.file_name;
        let wanted_loose = loose_name(wanted.trim_end_matches(".jar").trim_end_matches(".zip"));
        let named_like = |name: &str| {
            if name.eq_ignore_ascii_case(wanted) {
                return true;
            }
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            loose_name(without_copy_number(stem)) == wanted_loose
        };
        if let Some((path, _)) = candidates
            .iter()
            .find(|(path, name)| named_like(name) && is_planned_file(path, planned))
        {
            found.push((index, path.clone()));
        }
    }
    found
}

/// Puts the file the user downloaded by hand (`downloaded`, checked to be `planned`'s) in
/// place, copied so their download stays where it is.
pub fn install_downloaded(
    dirs: &InstanceDirs,
    target: &Target,
    planned: &CfPlannedFile,
    downloaded: &Path,
) -> Result<Placed, ContentError> {
    if !is_planned_file(downloaded, planned) {
        return Err(ContentError::Invalid(format!(
            "{} isn't {} from CurseForge",
            downloaded.display(),
            planned.file.file_name
        )));
    }
    let kind = planned.kind;
    let file = &planned.file;
    let provenance = Provenance::load(&dirs.instance);
    let (old, same) = installed_files(dirs, kind, &provenance, file.mod_id, file.id);
    if let Some(same) = same {
        return Ok(Placed::AlreadyInstalled { name: same });
    }
    let name = safe_download_name(kind, &file.file_name).ok_or_else(|| {
        ContentError::Invalid(format!(
            "{} isn't a file name that can be used",
            file.file_name
        ))
    })?;
    target_name(dirs, kind, &name, &old)?;
    let dir = dirs.content_dir(kind);
    fs::create_dir_all(&dir).map_err(io_error(&dir))?;
    let staging = dir.join(format!(".lodestone-{}.download", file.id));
    fs::copy(downloaded, &staging).map_err(io_error(&staging))?;
    if !is_hex(&file.sha1().unwrap_or_default(), 40) && file.file_fingerprint == 0 {
        let _ = fs::remove_file(&staging);
        return Err(ContentError::Invalid(format!(
            "CurseForge sent nothing to check {} against",
            file.file_name
        )));
    }
    let source = CurseForgeSource::new(file, planned.project.as_ref());
    finish(dirs, target, planned, &staging, &name, old, source, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_downloads_number_is_ignored() {
        assert_eq!(without_copy_number("Secret-Mod-1.0 (1)"), "Secret-Mod-1.0");
        assert_eq!(without_copy_number("Secret-Mod-1.0 (12)"), "Secret-Mod-1.0");
        assert_eq!(without_copy_number("Mod (beta)"), "Mod (beta)");
        assert_eq!(without_copy_number("Mod-1.0"), "Mod-1.0");
    }

    #[test]
    fn loose_names_ignore_separators_and_case() {
        assert_eq!(loose_name("Create-1.20.1_0.5.1.f"), "create 1 20 1 0 5 1 f");
        assert_eq!(
            loose_name("create 1.20.1  0.5.1+f"),
            "create 1 20 1 0 5 1 f"
        );
    }
}
