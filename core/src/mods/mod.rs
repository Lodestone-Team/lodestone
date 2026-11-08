//! Mods and plugins of a Minecraft instance: the `mods/` or `plugins/` folder.
//!
//! Nothing is stored besides the jars themselves. The list is a scan of the folder, the
//! metadata inside each jar, and a Modrinth lookup by SHA-512 hash (cached in memory).
//! A mod is disabled by renaming `<name>.jar` to `<name>.jar.disabled`.

pub mod install;
pub mod metadata;
pub mod restart;
pub mod scan;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::modrinth::{search::compatible_loaders, ModrinthApi, ModrinthProject, ModrinthVersion};
use crate::traits::t_configurable::{Game, MinecraftVariant};
use crate::types::InstanceUuid;

pub use scan::ScannedFile;

/// Mods and plugins may be at most this large.
pub const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum InstalledModKind {
    Mod,
    Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum InstalledModLoader {
    Fabric,
    Forge,
    Paper,
    Spigot,
}

impl InstalledModLoader {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstalledModLoader::Fabric => "fabric",
            InstalledModLoader::Forge => "forge",
            InstalledModLoader::Paper => "paper",
            InstalledModLoader::Spigot => "spigot",
        }
    }

    pub fn kind(&self) -> InstalledModKind {
        match self {
            InstalledModLoader::Fabric | InstalledModLoader::Forge => InstalledModKind::Mod,
            InstalledModLoader::Paper | InstalledModLoader::Spigot => InstalledModKind::Plugin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModFileMetadata {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModrinthUpdate {
    pub version_id: String,
    pub version_number: String,
}

/// The Modrinth project and version an installed file belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModrinthMatch {
    pub project_id: String,
    pub version_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
    /// Only filled when updates were checked.
    pub update: Option<ModrinthUpdate>,
}

impl ModrinthMatch {
    pub fn new(version: &ModrinthVersion, project: Option<&ModrinthProject>) -> Self {
        ModrinthMatch {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            slug: project
                .map(|p| p.slug.clone())
                .unwrap_or_else(|| version.project_id.clone()),
            title: project
                .map(|p| p.title.clone())
                .unwrap_or_else(|| version.name.clone()),
            icon_url: project.and_then(|p| p.icon_url.clone()),
            version_number: version.version_number.clone(),
            update: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InstalledMod {
    /// The current name on disk, including `.disabled`.
    pub file_name: String,
    pub enabled: bool,
    #[ts(type = "number")]
    pub size: u64,
    pub sha512: String,
    pub metadata: Option<ModFileMetadata>,
    pub modrinth: Option<ModrinthMatch>,
}

impl InstalledMod {
    pub fn new(file: ScannedFile, modrinth: Option<ModrinthMatch>) -> Self {
        InstalledMod {
            file_name: file.file_name,
            enabled: file.enabled,
            size: file.size,
            sha512: file.sha512,
            metadata: file.metadata,
            modrinth,
        }
    }

    /// The name shown to users: the Modrinth title, else the name in the jar, else the
    /// file name.
    pub fn display_name(&self) -> &str {
        self.modrinth
            .as_ref()
            .map(|m| m.title.as_str())
            .or(self.metadata.as_ref().and_then(|m| m.name.as_deref()))
            .unwrap_or(&self.file_name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InstalledModList {
    pub kind: InstalledModKind,
    pub loader: InstalledModLoader,
    pub game_version: String,
    /// `mods` or `plugins`, relative to the instance.
    pub directory: String,
    pub restart_required: bool,
    /// Sorted by display name, ignoring case.
    pub mods: Vec<InstalledMod>,
}

/// The `mods` or `plugins` folder of one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ModFolder {
    pub loader: InstalledModLoader,
    pub game_version: String,
    /// The absolute path of the folder.
    pub dir: PathBuf,
}

pub fn unsupported_instance() -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("This instance does not support mods or plugins"),
    }
}

pub(crate) fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

impl ModFolder {
    /// The folder of an instance of type `game` whose root is `instance_root`. Only Fabric,
    /// Forge, Paper and Spigot instances have one.
    pub fn new(game: &Game, game_version: String, instance_root: &Path) -> Result<Self, Error> {
        let loader = match game {
            Game::MinecraftJava { variant } => match variant {
                MinecraftVariant::Fabric => InstalledModLoader::Fabric,
                MinecraftVariant::Forge => InstalledModLoader::Forge,
                MinecraftVariant::Paper => InstalledModLoader::Paper,
                MinecraftVariant::Spigot => InstalledModLoader::Spigot,
                _ => return Err(unsupported_instance()),
            },
            _ => return Err(unsupported_instance()),
        };
        let folder = ModFolder {
            loader,
            game_version,
            dir: PathBuf::new(),
        };
        let dir = instance_root.join(folder.directory());
        Ok(ModFolder { dir, ..folder })
    }

    pub fn kind(&self) -> InstalledModKind {
        self.loader.kind()
    }

    /// The folder name, relative to the instance root (the server's working directory).
    pub fn directory(&self) -> &'static str {
        match self.kind() {
            InstalledModKind::Mod => "mods",
            InstalledModKind::Plugin => "plugins",
        }
    }

    /// The Modrinth loaders whose files run on this instance.
    pub fn accepted_loaders(&self) -> Vec<String> {
        compatible_loaders(self.loader.as_str())
    }

    /// Why `version` does not run on this instance, or `None` if it does.
    pub fn incompatibility(&self, version: &ModrinthVersion) -> Option<String> {
        fn list(values: &[String]) -> String {
            let mut shown: Vec<&str> = values.iter().take(5).map(String::as_str).collect();
            if values.len() > shown.len() {
                shown.push("…");
            }
            if shown.is_empty() {
                "nothing".to_string()
            } else {
                shown.join(", ")
            }
        }
        let accepted = self.accepted_loaders();
        if !version.loaders.iter().any(|l| accepted.contains(l)) {
            return Some(format!(
                "{} {} is for {}, but this instance uses {}",
                version.name,
                version.version_number,
                list(&version.loaders),
                self.loader.as_str()
            ));
        }
        if !version.game_versions.contains(&self.game_version) {
            return Some(format!(
                "{} {} is for Minecraft {}, but this instance runs {}",
                version.name,
                version.version_number,
                list(&version.game_versions),
                self.game_version
            ));
        }
        None
    }
}

/// Whether a file name is a disabled jar (`.jar.disabled`, ignoring case).
pub fn is_disabled_name(name: &str) -> bool {
    name.to_lowercase().ends_with(".jar.disabled")
}

/// Whether a file name is listed: `.jar` or `.jar.disabled`, ignoring case.
pub fn is_mod_file_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jar") || lower.ends_with(".jar.disabled")
}

/// The enabled name of a mod file: without `.disabled`.
pub fn enabled_name(name: &str) -> String {
    if is_disabled_name(name) {
        name[..name.len() - DISABLED_SUFFIX.len()].to_string()
    } else {
        name.to_string()
    }
}

/// The disabled name of a mod file: with `.disabled`.
pub fn disabled_name(name: &str) -> String {
    if is_disabled_name(name) {
        name.to_string()
    } else {
        format!("{name}{DISABLED_SUFFIX}")
    }
}

/// Checks a `file_name` sent by a client: one path segment ending in `.jar` or
/// `.jar.disabled`, without `/`, `\`, `..`, NUL, `:` or control characters.
pub fn check_mod_file_name(name: &str) -> Result<(), Error> {
    let invalid = name.is_empty()
        || name.len() > 255
        || name.contains(['/', '\\', ':', '\0'])
        || name.contains("..")
        || name.starts_with('.')
        || name.chars().any(|c| c.is_control())
        || !is_mod_file_name(name)
        // the name must have something before the extension
        || enabled_name(name).len() <= ".jar".len();
    if invalid {
        Err(bad_request(
            "Invalid file name: it must be a single .jar or .jar.disabled file name",
        ))
    } else {
        Ok(())
    }
}

/// A plain file name ending in `.jar` made from a name from Modrinth or an upload, or `None`
/// if no safe name can be made.
pub fn safe_jar_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let sanitized = sanitize_filename::sanitize(base);
    let sanitized = sanitized.trim().trim_start_matches('.').to_string();
    if !sanitized.to_lowercase().ends_with(".jar") || check_mod_file_name(&sanitized).is_err() {
        return None;
    }
    Some(sanitized)
}

/// The error for a file operation that failed, with a hint to stop the server when the
/// file may be in use (Windows locks the jars of a running server).
pub fn file_error(e: std::io::Error, action: &str, file_name: &str) -> Error {
    // ERROR_SHARING_VIOLATION and ERROR_LOCK_VIOLATION
    let in_use = e.kind() == std::io::ErrorKind::PermissionDenied
        || matches!(e.raw_os_error(), Some(32) | Some(33)) && cfg!(windows);
    if in_use {
        return Error {
            kind: ErrorKind::BadRequest,
            source: eyre!(
                "Could not {action} {file_name}: the file is in use or cannot be changed. Stop the server first, then try again"
            ),
        };
    }
    if e.kind() == std::io::ErrorKind::NotFound {
        return Error {
            kind: ErrorKind::NotFound,
            source: eyre!("{file_name} does not exist"),
        };
    }
    Error {
        kind: ErrorKind::Internal,
        source: eyre!("Could not {action} {file_name}: {e}"),
    }
}

fn file_name_conflict(file_name: &str) -> Error {
    Error {
        kind: ErrorKind::Conflict,
        source: eyre!("A file named {file_name} already exists"),
    }
}

/// Looks up the Modrinth project and version of each file, keyed by SHA-512. Files that
/// are not on Modrinth are left out.
pub async fn match_files(
    api: &dyn ModrinthApi,
    files: &[ScannedFile],
) -> Result<HashMap<String, (ModrinthMatch, ModrinthVersion)>, Error> {
    if files.is_empty() {
        return Ok(HashMap::new());
    }
    let hashes: Vec<String> = files.iter().map(|f| f.sha512.clone()).collect();
    let versions = api.versions_by_hash(&hashes).await?;
    let mut project_ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
    project_ids.sort();
    project_ids.dedup();
    let projects: HashMap<String, ModrinthProject> = if project_ids.is_empty() {
        HashMap::new()
    } else {
        api.projects(&project_ids)
            .await?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect()
    };
    Ok(versions
        .into_iter()
        .map(|(hash, version)| {
            let m = ModrinthMatch::new(&version, projects.get(&version.project_id));
            (hash, (m, version))
        })
        .collect())
}

/// Builds the list for `files`. Modrinth is asked for matches; if it cannot be reached
/// the list is still returned, without matches. With `check_updates`, Modrinth is also
/// asked for newer versions, and failing to reach it is an error.
pub async fn build_list(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
    files: Vec<ScannedFile>,
    check_updates: bool,
    restart_required: bool,
) -> Result<InstalledModList, Error> {
    let mut matches = match match_files(api, &files).await {
        Ok(matches) => matches,
        Err(e) if !check_updates => {
            warn!("Could not look up mods on Modrinth: {}", e.source);
            HashMap::new()
        }
        Err(e) => return Err(e),
    };
    if check_updates && !matches.is_empty() {
        let hashes: Vec<String> = matches.keys().cloned().collect();
        let latest = api
            .latest_versions_by_hash(
                &hashes,
                &folder.accepted_loaders(),
                std::slice::from_ref(&folder.game_version),
            )
            .await?;
        for (hash, (m, current)) in matches.iter_mut() {
            if let Some(newest) = latest.get(hash) {
                if is_newer(newest, current) {
                    m.update = Some(ModrinthUpdate {
                        version_id: newest.id.clone(),
                        version_number: newest.version_number.clone(),
                    });
                }
            }
        }
    }
    let mut mods: Vec<InstalledMod> = files
        .into_iter()
        .map(|file| {
            let m = matches.get(&file.sha512).map(|(m, _)| m.clone());
            InstalledMod::new(file, m)
        })
        .collect();
    sort_mods(&mut mods);
    Ok(InstalledModList {
        kind: folder.kind(),
        loader: folder.loader,
        game_version: folder.game_version.clone(),
        directory: folder.directory().to_string(),
        restart_required,
        mods,
    })
}

/// Sorts by display name, ignoring case, then by file name.
pub fn sort_mods(mods: &mut [InstalledMod]) {
    mods.sort_by(|a, b| {
        a.display_name()
            .to_lowercase()
            .cmp(&b.display_name().to_lowercase())
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
}

/// Whether `candidate` is a different and newer version than `current`.
fn is_newer(candidate: &ModrinthVersion, current: &ModrinthVersion) -> bool {
    if candidate.id == current.id {
        return false;
    }
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    match (
        parse(&candidate.date_published),
        parse(&current.date_published),
    ) {
        (Some(c), Some(o)) => c > o,
        _ => candidate.date_published > current.date_published,
    }
}

/// Matches for `files`, or none if Modrinth cannot be reached.
pub async fn match_files_or_none(
    api: &dyn ModrinthApi,
    files: Vec<ScannedFile>,
) -> Vec<InstalledMod> {
    let matches = match match_files(api, &files).await {
        Ok(matches) => matches,
        Err(e) => {
            warn!("Could not look up mods on Modrinth: {}", e.source);
            HashMap::new()
        }
    };
    files
        .into_iter()
        .map(|file| {
            let m = matches.get(&file.sha512).map(|(m, _)| m.clone());
            InstalledMod::new(file, m)
        })
        .collect()
}

/// The path of an existing mod file in `folder`, after checking the name. Symbolic links
/// and folders are not mod files.
pub async fn existing_mod_path(folder: &ModFolder, file_name: &str) -> Result<PathBuf, Error> {
    check_mod_file_name(file_name)?;
    let path = folder.dir.join(file_name);
    match tokio::fs::symlink_metadata(&path).await {
        Ok(m) if m.is_file() => Ok(path),
        _ => Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("{file_name} does not exist"),
        }),
    }
}

/// Enables or disables a mod by renaming it. Returns the file under its new name; a file
/// that already has the wanted state is returned unchanged.
pub async fn set_enabled(
    folder: &ModFolder,
    file_name: &str,
    enabled: bool,
) -> Result<(ScannedFile, bool), Error> {
    let path = existing_mod_path(folder, file_name).await?;
    let currently_enabled = !is_disabled_name(file_name);
    if currently_enabled == enabled {
        return Ok((scan::scan_file(&path, folder.loader).await?, false));
    }
    let target_name = if enabled {
        enabled_name(file_name)
    } else {
        disabled_name(file_name)
    };
    let target = folder.dir.join(&target_name);
    if tokio::fs::symlink_metadata(&target).await.is_ok() {
        return Err(file_name_conflict(&target_name));
    }
    let action = if enabled { "enable" } else { "disable" };
    tokio::fs::rename(&path, &target)
        .await
        .map_err(|e| file_error(e, action, file_name))?;
    Ok((scan::scan_file(&target, folder.loader).await?, true))
}

/// Deletes a mod file.
pub async fn delete(folder: &ModFolder, file_name: &str) -> Result<(), Error> {
    let path = existing_mod_path(folder, file_name).await?;
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| file_error(e, "delete", file_name))
}

/// Moves uploaded jars from their temporary files (in the mods folder) into place, as one
/// batch: first every jar is checked to be a zip with the loader's metadata file, and,
/// unless `overwrite` is set, that no file with its name exists. If any check fails,
/// nothing is written. Returns the saved files in upload order.
pub async fn save_uploads(
    folder: &ModFolder,
    uploads: Vec<(tempfile::TempPath, String)>,
    overwrite: bool,
) -> Result<Vec<ScannedFile>, Error> {
    let loader = folder.loader;
    let dir = folder.dir.clone();
    let names: Vec<String> = uploads.iter().map(|(_, name)| name.clone()).collect();
    tokio::task::spawn_blocking(move || -> Result<(), Error> {
        let mut seen = std::collections::HashSet::new();
        for (temp, file_name) in &uploads {
            if !seen.insert(file_name.to_lowercase()) {
                return Err(bad_request(format!("{file_name} was uploaded twice")));
            }
            if !metadata::jar_has_metadata(temp, loader)? {
                return Err(bad_request(format!(
                    "{file_name} is not a {} {}: it has no {}",
                    loader.as_str(),
                    match loader.kind() {
                        InstalledModKind::Mod => "mod",
                        InstalledModKind::Plugin => "plugin",
                    },
                    metadata::metadata_files(loader).join(" or ")
                )));
            }
        }
        if !overwrite {
            let existing: Vec<&str> = uploads
                .iter()
                .map(|(_, name)| name.as_str())
                .filter(|name| std::fs::symlink_metadata(dir.join(name)).is_ok())
                .collect();
            if !existing.is_empty() {
                return Err(Error {
                    kind: ErrorKind::Conflict,
                    source: eyre!("These files already exist: {}", existing.join(", ")),
                });
            }
        }
        let mut saved: Vec<PathBuf> = Vec::new();
        for (temp, file_name) in uploads {
            let target = dir.join(&file_name);
            let result = if overwrite {
                temp.persist(&target)
                    .map_err(|e| file_error(e.error, "replace", &file_name))
            } else {
                temp.persist_noclobber(&target).map_err(|e| {
                    if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                        file_name_conflict(&file_name)
                    } else {
                        file_error(e.error, "save", &file_name)
                    }
                })
            };
            if let Err(e) = result {
                // without overwrite, a file appeared meanwhile: take back this batch
                if !overwrite {
                    for path in &saved {
                        let _ = std::fs::remove_file(path);
                    }
                }
                return Err(e);
            }
            saved.push(target);
        }
        Ok(())
    })
    .await
    .map_err(|e| eyre!("The upload task failed: {e}"))??;
    let mut files = Vec::new();
    for name in names {
        files.push(scan::scan_file(&folder.dir.join(name), loader).await?);
    }
    Ok(files)
}

/// State shared by the mod and modpack handlers.
pub struct ModsState {
    pub modrinth: Arc<dyn ModrinthApi>,
    pub restart: restart::RestartTracker,
    pub packs: crate::modpack::store::PackStore,
    locks: std::sync::Mutex<HashMap<InstanceUuid, Arc<tokio::sync::Mutex<()>>>>,
}

impl ModsState {
    pub fn new(modrinth: Arc<dyn ModrinthApi>, packs: crate::modpack::store::PackStore) -> Self {
        Self {
            modrinth,
            restart: restart::RestartTracker::default(),
            packs,
            locks: Default::default(),
        }
    }

    /// A lock that serializes changes to one instance's mods folder.
    pub fn lock_for(&self, uuid: &InstanceUuid) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(|e| e.into_inner());
        // forget locks nobody holds, so deleted instances do not pile up
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
        locks.entry(uuid.clone()).or_default().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_file_names() {
        for ok in [
            "sodium.jar",
            "Sodium-0.5.jar.disabled",
            "a b+c.JAR",
            "x.jar.DISABLED",
        ] {
            assert!(check_mod_file_name(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            ".jar",
            ".jar.disabled",
            "a.zip",
            "a.jar.bak",
            "a.disabled",
            "../a.jar",
            "a/b.jar",
            "a\\b.jar",
            "a..jar",
            "a\0.jar",
            "C:a.jar",
            ".hidden.jar",
            "line\nbreak.jar",
        ] {
            assert!(check_mod_file_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_mod_file_name(&format!("{}.jar", "a".repeat(252))).is_err());
    }

    #[test]
    fn enabled_and_disabled_names() {
        assert_eq!(disabled_name("a.jar"), "a.jar.disabled");
        assert_eq!(disabled_name("a.jar.disabled"), "a.jar.disabled");
        assert_eq!(enabled_name("a.jar.disabled"), "a.jar");
        assert_eq!(enabled_name("a.JAR.Disabled"), "a.JAR");
        assert_eq!(enabled_name("a.jar"), "a.jar");
        assert!(is_disabled_name("a.jar.disabled"));
        assert!(!is_disabled_name("a.jar"));
    }

    #[test]
    fn safe_jar_names() {
        assert_eq!(
            safe_jar_name("sodium-0.5.jar").as_deref(),
            Some("sodium-0.5.jar")
        );
        assert_eq!(safe_jar_name("../../evil.jar").as_deref(), Some("evil.jar"));
        assert_eq!(
            safe_jar_name("C:\\x\\evil.jar").as_deref(),
            Some("evil.jar")
        );
        assert_eq!(safe_jar_name(".hidden.jar").as_deref(), Some("hidden.jar"));
        assert_eq!(safe_jar_name("a<b>.jar").as_deref(), Some("ab.jar"));
        assert_eq!(safe_jar_name("mod.zip"), None);
        assert_eq!(safe_jar_name("x.jar.disabled"), None);
        assert_eq!(safe_jar_name(".jar"), None);
        assert_eq!(safe_jar_name(""), None);
    }

    fn folder(dir: &Path, loader: InstalledModLoader) -> ModFolder {
        ModFolder {
            loader,
            game_version: "1.20.1".into(),
            dir: dir.to_path_buf(),
        }
    }

    #[test]
    fn folders_for_instances() {
        let root = Path::new("/srv/instance");
        let fabric = ModFolder::new(
            &Game::MinecraftJava {
                variant: MinecraftVariant::Fabric,
            },
            "1.20.1".into(),
            root,
        )
        .unwrap();
        assert_eq!(fabric.dir, root.join("mods"));
        assert_eq!(fabric.kind(), InstalledModKind::Mod);
        let paper = ModFolder::new(
            &Game::MinecraftJava {
                variant: MinecraftVariant::Paper,
            },
            "1.20.1".into(),
            root,
        )
        .unwrap();
        assert_eq!(paper.dir, root.join("plugins"));
        assert_eq!(paper.accepted_loaders(), vec!["paper", "spigot", "bukkit"]);
        for game in [
            Game::MinecraftJava {
                variant: MinecraftVariant::Vanilla,
            },
            Game::MinecraftBedrock,
        ] {
            let err = ModFolder::new(&game, "1.20.1".into(), root).unwrap_err();
            assert_eq!(
                err.source.to_string(),
                "This instance does not support mods or plugins"
            );
        }
    }

    #[tokio::test]
    async fn enable_and_disable_rename_files() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path(), InstalledModLoader::Fabric);
        crate::mods::metadata::tests::make_zip(
            &dir.path().join("a.jar"),
            &[("fabric.mod.json", r#"{"id":"a"}"#)],
        );

        let (file, changed) = set_enabled(&folder, "a.jar", false).await.unwrap();
        assert!(changed);
        assert_eq!(file.file_name, "a.jar.disabled");
        assert!(!file.enabled);
        assert!(!dir.path().join("a.jar").exists());
        assert!(dir.path().join("a.jar.disabled").exists());

        // disabling again changes nothing
        let (file, changed) = set_enabled(&folder, "a.jar.disabled", false).await.unwrap();
        assert!(!changed);
        assert_eq!(file.file_name, "a.jar.disabled");

        let (file, changed) = set_enabled(&folder, "a.jar.disabled", true).await.unwrap();
        assert!(changed);
        assert_eq!(file.file_name, "a.jar");
        assert!(file.enabled);
        assert_eq!(file.metadata.unwrap().id.as_deref(), Some("a"));
    }

    #[tokio::test]
    async fn enable_and_disable_conflicts_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path(), InstalledModLoader::Fabric);
        std::fs::write(dir.path().join("a.jar"), "1").unwrap();
        std::fs::write(dir.path().join("a.jar.disabled"), "2").unwrap();

        let err = set_enabled(&folder, "a.jar", false).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Conflict));
        let err = set_enabled(&folder, "a.jar.disabled", true)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Conflict));
        // nothing was renamed
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.jar")).unwrap(),
            "1"
        );

        let err = set_enabled(&folder, "missing.jar", true).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
        let err = set_enabled(&folder, "../a.jar", true).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        // a folder named like a jar is not a mod
        std::fs::create_dir(dir.path().join("dir.jar")).unwrap();
        let err = delete(&folder, "dir.jar").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));

        delete(&folder, "a.jar").await.unwrap();
        assert!(!dir.path().join("a.jar").exists());
    }

    #[tokio::test]
    async fn uploads_need_metadata_and_respect_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let folder = folder(dir.path(), InstalledModLoader::Fabric);
        let temp = |name: &str, contents: &[(&str, &str)]| {
            let path = tempfile::Builder::new()
                .suffix(".tmp")
                .tempfile_in(dir.path())
                .unwrap()
                .into_temp_path();
            crate::mods::metadata::tests::make_zip(&path, contents);
            (path, name.to_string())
        };
        let fabric = [("fabric.mod.json", r#"{"id":"m","name":"M"}"#)];
        let listing = || {
            let mut names: Vec<String> = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                .collect();
            names.sort();
            names
        };

        let files = save_uploads(&folder, vec![temp("m.jar", &fabric)], false)
            .await
            .unwrap();
        assert_eq!(files[0].file_name, "m.jar");
        assert_eq!(
            files[0].metadata.as_ref().unwrap().name.as_deref(),
            Some("M")
        );

        // one conflict in a batch writes nothing
        let err = save_uploads(
            &folder,
            vec![temp("new.jar", &fabric), temp("m.jar", &fabric)],
            false,
        )
        .await
        .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::Conflict));
        assert!(err.source.to_string().contains("m.jar"));
        assert_eq!(listing(), vec!["m.jar"]);

        // the same batch again with overwrite writes both
        let files = save_uploads(
            &folder,
            vec![temp("new.jar", &fabric), temp("m.jar", &fabric)],
            true,
        )
        .await
        .unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(listing(), vec!["m.jar", "new.jar"]);

        // a jar without the loader's metadata fails the whole batch
        let err = save_uploads(
            &folder,
            vec![
                temp("ok.jar", &fabric),
                temp("p.jar", &[("plugin.yml", "name: P")]),
            ],
            false,
        )
        .await
        .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        let err = save_uploads(
            &folder,
            vec![temp("x.jar", &fabric), temp("X.JAR", &fabric)],
            true,
        )
        .await
        .unwrap_err();
        assert!(err.source.to_string().contains("uploaded twice"));
        // only the jars are left: the temporary files are gone
        assert_eq!(listing(), vec!["m.jar", "new.jar"]);
    }

    #[test]
    fn incompatibility_reasons() {
        let folder = folder(Path::new("/x"), InstalledModLoader::Paper);
        let version = |loaders: &[&str], game_versions: &[&str]| ModrinthVersion {
            id: "v".into(),
            project_id: "p".into(),
            name: "P".into(),
            version_number: "1.0".into(),
            version_type: crate::modrinth::ModrinthVersionType::Release,
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
            date_published: String::new(),
            downloads: 0,
            files: vec![],
            dependencies: vec![],
        };
        assert_eq!(
            folder.incompatibility(&version(&["bukkit"], &["1.20.1"])),
            None
        );
        assert_eq!(
            folder.incompatibility(&version(&["paper"], &["1.20.1"])),
            None
        );
        assert!(folder
            .incompatibility(&version(&["fabric"], &["1.20.1"]))
            .unwrap()
            .contains("is for fabric, but this instance uses paper"));
        assert!(folder
            .incompatibility(&version(&["paper"], &["1.19.4"]))
            .unwrap()
            .contains("is for Minecraft 1.19.4, but this instance runs 1.20.1"));
    }

    #[test]
    fn newer_versions() {
        let version = |id: &str, date: &str| ModrinthVersion {
            id: id.into(),
            project_id: "p".into(),
            name: String::new(),
            version_number: String::new(),
            version_type: crate::modrinth::ModrinthVersionType::Release,
            game_versions: vec![],
            loaders: vec![],
            date_published: date.into(),
            downloads: 0,
            files: vec![],
            dependencies: vec![],
        };
        let old = version("a", "2024-01-01T00:00:00Z");
        let new = version("b", "2024-02-01T00:00:00.123+00:00");
        assert!(is_newer(&new, &old));
        assert!(!is_newer(&old, &new));
        assert!(!is_newer(&old, &old));
    }
}
