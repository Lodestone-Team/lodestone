//! Installing a Modrinth version, its required dependencies, and replacing older versions
//! of the same projects.

use std::collections::{HashMap, HashSet, VecDeque};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::scan::{scan_file, scan_folder, ScannedFile};
use super::{
    bad_request, file_error, match_files, safe_jar_name, InstalledMod, ModFolder, ModrinthMatch,
    MAX_MOD_BYTES,
};
use crate::error::{Error, ErrorKind};
use crate::modrinth::{
    ModrinthApi, ModrinthDependencyType, ModrinthProject, ModrinthVersion, ModrinthVersionType,
    CDN_HOST,
};

/// Dependencies are installed at most this many levels below the requested version.
pub const MAX_DEPENDENCY_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModInstallRequest {
    pub version_id: String,
    /// Defaults to true.
    #[serde(default)]
    #[ts(optional)]
    pub install_dependencies: Option<bool>,
    /// Install even if the version is not for this instance's loader or game version.
    #[serde(default)]
    #[ts(optional)]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModInstallSkipped {
    pub project_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModInstallResult {
    pub installed: Vec<InstalledMod>,
    /// File names.
    pub removed: Vec<String>,
    pub skipped: Vec<ModInstallSkipped>,
}

#[derive(Debug, Clone, Copy)]
pub struct InstallOptions {
    pub install_dependencies: bool,
    pub force: bool,
}

impl From<&ModInstallRequest> for InstallOptions {
    fn from(request: &ModInstallRequest) -> Self {
        InstallOptions {
            install_dependencies: request.install_dependencies.unwrap_or(true),
            force: request.force.unwrap_or(false),
        }
    }
}

/// An installed file and the Modrinth version it belongs to, if any.
struct Installed {
    file: ScannedFile,
    version: Option<ModrinthVersion>,
}

impl Installed {
    fn display_name(&self) -> &str {
        self.file
            .metadata
            .as_ref()
            .and_then(|m| m.name.as_deref())
            .unwrap_or(&self.file.file_name)
    }
}

struct Planned {
    version: ModrinthVersion,
    depth: usize,
}

fn skipped(project_id: impl Into<String>, reason: impl Into<String>) -> ModInstallSkipped {
    ModInstallSkipped {
        project_id: project_id.into(),
        reason: reason.into(),
    }
}

/// The newest `release` among `versions` that runs on `folder`, or else the newest
/// version that does.
fn newest_compatible(
    folder: &ModFolder,
    versions: Vec<ModrinthVersion>,
) -> Option<ModrinthVersion> {
    let mut compatible: Vec<ModrinthVersion> = versions
        .into_iter()
        .filter(|v| folder.incompatibility(v).is_none())
        .collect();
    compatible.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    let release = compatible
        .iter()
        .position(|v| v.version_type == ModrinthVersionType::Release);
    match release {
        Some(i) => Some(compatible.swap_remove(i)),
        None => compatible.into_iter().next(),
    }
}

/// Whether an error from Modrinth means the thing does not exist (rather than Modrinth not
/// being reachable).
fn is_not_found(e: &Error) -> bool {
    matches!(e.kind, ErrorKind::NotFound)
}

/// Picks the version of a required dependency to install. `Ok(Err(reason))` when none can
/// be installed.
async fn pick_dependency_version(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
    project_id: &str,
    pinned: Option<ModrinthVersion>,
) -> Result<Result<ModrinthVersion, String>, Error> {
    if let Some(version) = pinned {
        return Ok(match folder.incompatibility(&version) {
            None => Ok(version),
            Some(reason) => Err(format!("The required version does not fit: {reason}")),
        });
    }
    let versions = match api
        .project_versions(
            project_id,
            &folder.accepted_loaders(),
            std::slice::from_ref(&folder.game_version),
        )
        .await
    {
        Ok(versions) => versions,
        Err(e) if is_not_found(&e) => return Ok(Err("Not found on Modrinth".to_string())),
        Err(e) => return Err(e),
    };
    Ok(newest_compatible(folder, versions).ok_or_else(|| {
        format!(
            "No version for {} on Minecraft {}",
            folder.loader.as_str(),
            folder.game_version
        )
    }))
}

/// Installs `root` (a version of `root_project`) into `folder`, see the module docs.
pub async fn install(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
    root: ModrinthVersion,
    root_project: &ModrinthProject,
    options: InstallOptions,
) -> Result<ModInstallResult, Error> {
    if !options.force {
        if let Some(reason) = folder.incompatibility(&root) {
            return Err(bad_request(reason));
        }
    }

    // what is installed now, by Modrinth project
    let files = scan_folder(folder).await?;
    let mut matches = match_files(api, &files).await?;
    let installed: Vec<Installed> = files
        .into_iter()
        .map(|file| {
            let version = matches.remove(&file.sha512).map(|(_, v)| v);
            Installed { file, version }
        })
        .collect();
    let mut by_project: HashMap<String, Vec<&Installed>> = HashMap::new();
    for item in &installed {
        if let Some(v) = &item.version {
            by_project
                .entry(v.project_id.clone())
                .or_default()
                .push(item);
        }
    }

    // the versions to install: the root, then required dependencies, breadth first
    let mut plan = vec![Planned {
        version: root,
        depth: 0,
    }];
    let mut seen: HashSet<String> = HashSet::from([plan[0].version.project_id.clone()]);
    let mut skipped_list = Vec::new();
    if options.install_dependencies {
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
                // a dependency may name only a version; its project comes from that version
                let mut pinned = None;
                let project_id = match (&dependency.project_id, &dependency.version_id) {
                    (Some(project_id), _) => project_id.clone(),
                    (None, Some(version_id)) => match api.version(version_id).await {
                        Ok(version) => {
                            let project_id = version.project_id.clone();
                            pinned = Some(version);
                            project_id
                        }
                        Err(e) if is_not_found(&e) => {
                            skipped_list.push(skipped(version_id.clone(), "Not found on Modrinth"));
                            continue;
                        }
                        Err(e) => return Err(e),
                    },
                    (None, None) => {
                        let name = dependency
                            .file_name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        skipped_list.push(skipped(
                            name.clone(),
                            format!("{name} is not on Modrinth; install it yourself"),
                        ));
                        continue;
                    }
                };
                if by_project.contains_key(&project_id) || seen.contains(&project_id) {
                    // already installed, already planned, or a cycle
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
                if pinned.is_none() {
                    if let Some(version_id) = &dependency.version_id {
                        pinned = match api.version(version_id).await {
                            Ok(version) => Some(version),
                            Err(e) if is_not_found(&e) => None,
                            Err(e) => return Err(e),
                        };
                    }
                }
                match pick_dependency_version(api, folder, &project_id, pinned).await? {
                    Ok(version) => {
                        plan.push(Planned { version, depth });
                        queue.push_back(plan.len() - 1);
                    }
                    Err(reason) => skipped_list.push(skipped(project_id, reason)),
                }
            }
        }
    }

    // nothing is installed if any planned version is incompatible with an installed mod
    for planned in &plan {
        for dependency in &planned.version.dependencies {
            if dependency.dependency_type != ModrinthDependencyType::Incompatible {
                continue;
            }
            let conflict = installed.iter().find(|item| {
                item.version.as_ref().is_some_and(|v| {
                    v.project_id != planned.version.project_id
                        && (dependency.project_id.as_deref() == Some(v.project_id.as_str())
                            || dependency.version_id.as_deref() == Some(v.id.as_str()))
                })
            });
            if let Some(item) = conflict {
                return Err(bad_request(format!(
                    "{} {} is incompatible with the installed {}. Remove it first",
                    planned.version.name,
                    planned.version.version_number,
                    item.display_name()
                )));
            }
        }
    }

    let mut project_ids: Vec<String> = plan.iter().map(|p| p.version.project_id.clone()).collect();
    project_ids.retain(|id| id != &root_project.id);
    let mut projects: HashMap<String, ModrinthProject> = if project_ids.is_empty() {
        HashMap::new()
    } else {
        api.projects(&project_ids)
            .await?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect()
    };
    projects.insert(root_project.id.clone(), root_project.clone());

    tokio::fs::create_dir_all(&folder.dir)
        .await
        .context(format!(
            "Failed to create the {} folder",
            folder.directory()
        ))?;
    let mut result = ModInstallResult {
        installed: Vec::new(),
        removed: Vec::new(),
        skipped: skipped_list,
    };
    for planned in plan {
        let old: Vec<&Installed> = by_project
            .get(&planned.version.project_id)
            .cloned()
            .unwrap_or_default();
        let project = projects.get(&planned.version.project_id);
        let (new_file, removed) =
            install_version_file(api, folder, &planned.version, &old, &installed).await?;
        result.removed.extend(removed);
        result.installed.push(InstalledMod::new(
            new_file,
            Some(ModrinthMatch::new(&planned.version, project)),
            folder.loader,
        ));
    }
    Ok(result)
}

/// Removes the `old` files except `keep`, returning their names.
async fn remove_old(
    folder: &ModFolder,
    old: &[&Installed],
    keep: &str,
) -> Result<Vec<String>, Error> {
    let mut removed = Vec::new();
    for item in old {
        if item.file.file_name == keep {
            continue;
        }
        tokio::fs::remove_file(folder.dir.join(&item.file.file_name))
            .await
            .map_err(|e| file_error(e, "remove the old version", &item.file.file_name))?;
        removed.push(item.file.file_name.clone());
    }
    Ok(removed)
}

/// Downloads the file of `version`, puts it in place, then removes the `old` files of the
/// same project. Returns the new file and the names of the removed ones.
///
/// If `version` itself is already installed, nothing is downloaded: that file is kept (and
/// returned as installed), and only other versions of the project are removed.
async fn install_version_file(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
    version: &ModrinthVersion,
    old: &[&Installed],
    installed: &[Installed],
) -> Result<(ScannedFile, Vec<String>), Error> {
    if let Some(same) = old
        .iter()
        .find(|o| o.version.as_ref().is_some_and(|v| v.id == version.id))
    {
        let removed = remove_old(folder, old, &same.file.file_name).await?;
        return Ok((same.file.clone(), removed));
    }
    let file = version
        .primary_file()
        .ok_or_else(|| bad_request(format!("{} has no files", version.name)))?;
    let url = url::Url::parse(&file.url)
        .map_err(|_| eyre!("Modrinth sent an invalid download URL for {}", version.name))?;
    if url.scheme() != "https" || url.host_str() != Some(CDN_HOST) {
        return Err(bad_request(format!(
            "Refusing to download {} from {}: files are only downloaded from {CDN_HOST}",
            file.filename,
            url.host_str().unwrap_or("an unknown host")
        )));
    }
    if file.size > MAX_MOD_BYTES {
        return Err(crate::modrinth::client::too_large(MAX_MOD_BYTES));
    }
    if file.sha512.len() != 128 {
        return Err(eyre!("Modrinth sent no SHA-512 hash for {}", file.filename).into());
    }
    let name = safe_jar_name(&file.filename).ok_or_else(|| {
        bad_request(format!(
            "{} is not a jar file name that can be used",
            file.filename
        ))
    })?;
    // a disabled old version stays disabled
    let disabled = !old.is_empty() && old.iter().all(|o| !o.file.enabled);
    let target_name = if disabled {
        super::disabled_name(&name)
    } else {
        name
    };
    // never overwrite a file of another Modrinth project
    if let Some(other) = installed.iter().find(|i| {
        i.file.file_name == target_name
            && i.version
                .as_ref()
                .is_some_and(|v| v.project_id != version.project_id)
    }) {
        return Err(Error {
            kind: ErrorKind::Conflict,
            source: eyre!(
                "{} already exists and belongs to {}",
                target_name,
                other.display_name()
            ),
        });
    }

    let temp = tempfile::Builder::new()
        .prefix(".lodestone-download-")
        .suffix(".tmp")
        .tempfile_in(&folder.dir)
        .context("Failed to create a temporary file")?
        .into_temp_path();
    let mut out = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&temp)
        .await
        .context("Failed to open a temporary file")?;
    let hashes = api.download(&url, &mut out, MAX_MOD_BYTES).await?;
    drop(out);
    if !hashes.sha512.eq_ignore_ascii_case(&file.sha512) {
        return Err(Error {
            kind: ErrorKind::External,
            source: eyre!(
                "The download of {} is damaged: its SHA-512 hash does not match",
                file.filename
            ),
        });
    }
    let target = folder.dir.join(&target_name);
    temp.persist(&target)
        .map_err(|e| file_error(e.error, "replace", &target_name))?;

    let removed = remove_old(folder, old, &target_name).await?;
    Ok((scan_file(&target, folder.loader).await?, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modrinth::fake::{FakeModrinth, FakeVersion};
    use crate::mods::InstalledModLoader;

    fn folder(dir: &std::path::Path) -> ModFolder {
        ModFolder {
            loader: InstalledModLoader::Fabric,
            game_version: "1.20.1".into(),
            dir: dir.join("mods"),
        }
    }

    const OPTIONS: InstallOptions = InstallOptions {
        install_dependencies: true,
        force: false,
    };

    async fn run(
        fake: &FakeModrinth,
        folder: &ModFolder,
        version_id: &str,
        options: InstallOptions,
    ) -> Result<ModInstallResult, Error> {
        let version = fake.version(version_id).await?;
        let project = fake.project(&version.project_id).await?;
        install(fake, folder, version, &project, options).await
    }

    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn installs_and_replaces_older_versions() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("sodium", "s1", "sodium-1.jar").date("2024-01-01T00:00:00Z"));
        fake.add(FakeVersion::new("sodium", "s2", "sodium-2.jar").date("2024-02-01T00:00:00Z"));

        let result = run(&fake, &folder, "s1", OPTIONS).await.unwrap();
        assert_eq!(result.installed.len(), 1);
        let installed = &result.installed[0];
        assert_eq!(installed.file_name, "sodium-1.jar");
        assert!(installed.enabled);
        let m = installed.modrinth.as_ref().unwrap();
        assert_eq!(m.project_id, "sodium");
        assert_eq!(m.version_id, "s1");
        assert_eq!(m.title, "Title of sodium");
        assert!(result.removed.is_empty());
        assert_eq!(names(&folder.dir), vec!["sodium-1.jar"]);

        let result = run(&fake, &folder, "s2", OPTIONS).await.unwrap();
        assert_eq!(result.installed[0].file_name, "sodium-2.jar");
        assert_eq!(result.removed, vec!["sodium-1.jar"]);
        assert_eq!(names(&folder.dir), vec!["sodium-2.jar"]);

        // installing the same version again downloads nothing and returns the file
        let downloads = fake.downloaded.lock().unwrap().len();
        let result = run(&fake, &folder, "s2", OPTIONS).await.unwrap();
        assert!(result.removed.is_empty());
        assert_eq!(result.installed[0].file_name, "sodium-2.jar");
        assert_eq!(
            result.installed[0].modrinth.as_ref().unwrap().version_id,
            "s2"
        );
        assert_eq!(fake.downloaded.lock().unwrap().len(), downloads);
        assert_eq!(names(&folder.dir), vec!["sodium-2.jar"]);

        // also when it is disabled, and it stays disabled
        std::fs::rename(
            folder.dir.join("sodium-2.jar"),
            folder.dir.join("sodium-2.jar.disabled"),
        )
        .unwrap();
        let result = run(&fake, &folder, "s2", OPTIONS).await.unwrap();
        assert_eq!(result.installed[0].file_name, "sodium-2.jar.disabled");
        assert!(!result.installed[0].enabled);
        assert_eq!(names(&folder.dir), vec!["sodium-2.jar.disabled"]);
    }

    #[tokio::test]
    async fn disabled_old_version_stays_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("lithium", "l1", "lithium-1.jar"));
        fake.add(FakeVersion::new("lithium", "l2", "lithium-2.jar"));
        run(&fake, &folder, "l1", OPTIONS).await.unwrap();
        std::fs::rename(
            folder.dir.join("lithium-1.jar"),
            folder.dir.join("lithium-1.jar.disabled"),
        )
        .unwrap();

        let result = run(&fake, &folder, "l2", OPTIONS).await.unwrap();
        assert_eq!(result.installed[0].file_name, "lithium-2.jar.disabled");
        assert!(!result.installed[0].enabled);
        assert_eq!(result.removed, vec!["lithium-1.jar.disabled"]);
        assert_eq!(names(&folder.dir), vec!["lithium-2.jar.disabled"]);
    }

    #[tokio::test]
    async fn unmatched_files_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        std::fs::create_dir_all(&folder.dir).unwrap();
        std::fs::write(folder.dir.join("custom.jar"), "my own mod").unwrap();
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("a", "a1", "a.jar"));
        let result = run(&fake, &folder, "a1", OPTIONS).await.unwrap();
        assert!(result.removed.is_empty());
        assert_eq!(names(&folder.dir), vec!["a.jar", "custom.jar"]);
    }

    #[tokio::test]
    async fn checks_compatibility_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("old", "o1", "old.jar").game_versions(&["1.19.2"]));
        fake.add(FakeVersion::new("forgemod", "f1", "f.jar").loaders(&["forge"]));

        let err = run(&fake, &folder, "o1", OPTIONS).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert!(err.source.to_string().contains("Minecraft 1.19.2"));
        let err = run(&fake, &folder, "f1", OPTIONS).await.unwrap_err();
        assert!(err.source.to_string().contains("is for forge"));
        assert!(!folder.dir.exists());

        let forced = InstallOptions {
            force: true,
            ..OPTIONS
        };
        let result = run(&fake, &folder, "o1", forced).await.unwrap();
        assert_eq!(result.installed[0].file_name, "old.jar");
    }

    #[tokio::test]
    async fn installs_required_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(
            FakeVersion::new("app", "app1", "app.jar")
                .requires("lib", None)
                .requires("pinned", Some("pin2"))
                .optional("extra")
                .embeds("embedded"),
        );
        // the newest release wins over a newer beta and older releases
        fake.add(FakeVersion::new("lib", "lib1", "lib-1.jar").date("2024-01-01T00:00:00Z"));
        fake.add(FakeVersion::new("lib", "lib2", "lib-2.jar").date("2024-02-01T00:00:00Z"));
        fake.add(
            FakeVersion::new("lib", "lib3", "lib-3-beta.jar")
                .date("2024-03-01T00:00:00Z")
                .beta(),
        );
        // an incompatible newer release is not picked
        fake.add(
            FakeVersion::new("lib", "lib4", "lib-4.jar")
                .date("2024-04-01T00:00:00Z")
                .game_versions(&["1.21"]),
        );
        fake.add(FakeVersion::new("pinned", "pin1", "pin-1.jar").date("2024-01-01T00:00:00Z"));
        fake.add(FakeVersion::new("pinned", "pin2", "pin-2.jar").date("2023-01-01T00:00:00Z"));
        fake.add(FakeVersion::new("extra", "e1", "extra.jar"));
        fake.add(FakeVersion::new("embedded", "em1", "embedded.jar"));

        let result = run(&fake, &folder, "app1", OPTIONS).await.unwrap();
        let installed: Vec<&str> = result
            .installed
            .iter()
            .map(|m| m.file_name.as_str())
            .collect();
        assert_eq!(installed, vec!["app.jar", "lib-2.jar", "pin-2.jar"]);
        assert!(result.skipped.is_empty(), "{:?}", result.skipped);
        assert_eq!(
            names(&folder.dir),
            vec!["app.jar", "lib-2.jar", "pin-2.jar"]
        );

        // without dependencies, only the version itself
        let dir = tempfile::tempdir().unwrap();
        let folder2 = super::super::ModFolder {
            dir: dir.path().join("mods"),
            ..folder.clone()
        };
        let no_deps = InstallOptions {
            install_dependencies: false,
            ..OPTIONS
        };
        let result = run(&fake, &folder2, "app1", no_deps).await.unwrap();
        assert_eq!(result.installed.len(), 1);
    }

    #[tokio::test]
    async fn dependencies_that_are_installed_are_not_reinstalled() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("lib", "lib1", "lib-1.jar").date("2024-01-01T00:00:00Z"));
        fake.add(FakeVersion::new("lib", "lib2", "lib-2.jar").date("2024-02-01T00:00:00Z"));
        fake.add(FakeVersion::new("app", "app1", "app.jar").requires("lib", None));
        run(&fake, &folder, "lib1", OPTIONS).await.unwrap();
        // disabled still counts as installed
        std::fs::rename(
            folder.dir.join("lib-1.jar"),
            folder.dir.join("lib-1.jar.disabled"),
        )
        .unwrap();
        let result = run(&fake, &folder, "app1", OPTIONS).await.unwrap();
        assert_eq!(result.installed.len(), 1);
        assert_eq!(names(&folder.dir), vec!["app.jar", "lib-1.jar.disabled"]);
    }

    #[tokio::test]
    async fn dependency_depth_cycles_and_missing_versions() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        // a -> b -> c -> d -> e, and d -> a (a cycle)
        fake.add(FakeVersion::new("a", "a1", "a.jar").requires("b", None));
        fake.add(FakeVersion::new("b", "b1", "b.jar").requires("c", None));
        fake.add(FakeVersion::new("c", "c1", "c.jar").requires("d", None));
        fake.add(
            FakeVersion::new("d", "d1", "d.jar")
                .requires("e", None)
                .requires("a", None)
                .requires("nofit", None)
                .requires("ghost", None),
        );
        fake.add(FakeVersion::new("e", "e1", "e.jar"));
        fake.add(FakeVersion::new("nofit", "n1", "n.jar").game_versions(&["1.8.9"]));

        let result = run(&fake, &folder, "a1", OPTIONS).await.unwrap();
        let installed: Vec<&str> = result
            .installed
            .iter()
            .map(|m| m.file_name.as_str())
            .collect();
        assert_eq!(installed, vec!["a.jar", "b.jar", "c.jar", "d.jar"]);
        let skipped: Vec<(&str, &str)> = result
            .skipped
            .iter()
            .map(|s| (s.project_id.as_str(), s.reason.as_str()))
            .collect();
        assert_eq!(
            skipped,
            vec![
                ("e", "More than 3 levels of dependencies"),
                ("nofit", "More than 3 levels of dependencies"),
                ("ghost", "More than 3 levels of dependencies"),
            ]
        );

        // at a lower depth, the reasons are about the versions
        let dir = tempfile::tempdir().unwrap();
        let folder = super::super::ModFolder {
            dir: dir.path().join("mods"),
            ..folder.clone()
        };
        let result = run(&fake, &folder, "d1", OPTIONS).await.unwrap();
        let reasons: HashMap<&str, &str> = result
            .skipped
            .iter()
            .map(|s| (s.project_id.as_str(), s.reason.as_str()))
            .collect();
        assert_eq!(
            reasons["nofit"],
            "No version for fabric on Minecraft 1.20.1"
        );
        assert_eq!(reasons["ghost"], "Not found on Modrinth");
        let installed: Vec<&str> = result
            .installed
            .iter()
            .map(|m| m.file_name.as_str())
            .collect();
        // d -> e, d -> a -> b -> c (c's dependency d is the root: a cycle)
        assert_eq!(installed, vec!["d.jar", "e.jar", "a.jar", "b.jar", "c.jar"]);
    }

    #[tokio::test]
    async fn incompatible_installed_mod_blocks_install() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("optifine", "o1", "optifabric.jar"));
        fake.add(FakeVersion::new("sodium", "s1", "sodium.jar").incompatible("optifine"));
        fake.add(FakeVersion::new("app", "app1", "app.jar").requires("sodium", None));
        run(&fake, &folder, "o1", OPTIONS).await.unwrap();

        let err = run(&fake, &folder, "s1", OPTIONS).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert!(
            err.source
                .to_string()
                .contains("incompatible with the installed"),
            "{}",
            err.source
        );
        // a dependency's incompatibility blocks too, before anything is downloaded
        let err = run(&fake, &folder, "app1", OPTIONS).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert_eq!(names(&folder.dir), vec!["optifabric.jar"]);
    }

    #[tokio::test]
    async fn rejects_bad_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("evil", "x1", "x.jar").url("https://evil.example.com/x.jar"));
        fake.add(FakeVersion::new("plain", "p1", "p.jar").url("http://cdn.modrinth.com/p.jar"));
        fake.add(FakeVersion::new("bad", "b1", "b.jar").wrong_hash());
        fake.add(FakeVersion::new("name", "n1", "../../escape.jar"));
        fake.add(FakeVersion::new("zip", "z1", "pack.zip"));

        for id in ["x1", "p1"] {
            let err = run(&fake, &folder, id, OPTIONS).await.unwrap_err();
            assert!(
                err.source.to_string().starts_with("Refusing to download"),
                "{id}"
            );
        }
        let err = run(&fake, &folder, "b1", OPTIONS).await.unwrap_err();
        assert!(err.source.to_string().contains("hash does not match"));
        let err = run(&fake, &folder, "z1", OPTIONS).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        let result = run(&fake, &folder, "n1", OPTIONS).await.unwrap();
        assert_eq!(result.installed[0].file_name, "escape.jar");
        // no temporary files are left behind
        assert_eq!(names(&folder.dir), vec!["escape.jar"]);
    }

    #[tokio::test]
    async fn does_not_overwrite_another_projects_file() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path());
        let fake = FakeModrinth::default();
        fake.add(FakeVersion::new("one", "o1", "same.jar"));
        fake.add(FakeVersion::new("two", "t1", "same.jar"));
        run(&fake, &folder, "o1", OPTIONS).await.unwrap();
        let err = run(&fake, &folder, "t1", OPTIONS).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Conflict));
    }
}
