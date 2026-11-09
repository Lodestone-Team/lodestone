//! Modrinth modpacks (`.mrpack`) as instances: reading and checking a pack, making a new
//! instance from it ([`import`]), and updating an instance to another version of its
//! pack ([`update`]).
//!
//! The rules follow the dashboard's server import (`core/src/modpack`), for the client:
//!
//! - the index is `modrinth.index.json`, format 1, for `minecraft`, with the Minecraft
//!   version and at most one loader in `dependencies` (`fabric-loader`, `quilt-loader`,
//!   `forge`, `neoforge`);
//! - the files whose `env.client` is `unsupported` are left out; the others are checked
//!   by [`check_files`] (paths, hosts, hashes and sizes) and downloaded from the hosts the
//!   format allows only, with their SHA-1 and SHA-512 checked;
//! - `overrides/` and then `client-overrides/` are copied, every path checked by
//!   [`check_pack_path`], links skipped and sizes checked against the zip header; paths
//!   starting with `.lodestone` are Lodestone's own and are skipped.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.

pub mod curseforge;
pub mod import;
pub mod update;

use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use lodestone_minecraft::{
    modrinth::search::is_valid_filter_value,
    mrpack::{
        INDEX_FILE, Index, MAX_ENTRIES, MAX_INDEX_BYTES, MAX_UNCOMPRESSED_BYTES, PackDownload,
        PathCheck, PathPolicy, Side, SkippedPath, check_files, check_pack_path,
    },
};
use serde::{Deserialize, Serialize};
use sha1::Digest as _;

use crate::{
    content::ModrinthError,
    game::{
        download::DownloadError,
        loaders::{
            LoaderChoice, LoaderKind, LoaderVersion,
            forge::{forge_display, neoforge_game_version},
        },
    },
    instances::InstanceError,
    storage::{self, Document, Migration, StorageError},
};

/// The record of an instance made from a modpack, next to `instance.json`.
pub const RECORD_FILE: &str = "modpack.json";

#[derive(Debug, thiserror::Error)]
pub enum ModpackError {
    #[error("{0}")]
    Invalid(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error(transparent)]
    Modrinth(#[from] ModrinthError),
    #[error(transparent)]
    CurseForge(#[from] crate::content::CurseForgeError),
    #[error(transparent)]
    Instance(#[from] InstanceError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("cancelled")]
    Cancelled,
}

impl From<lodestone_minecraft::Error> for ModpackError {
    fn from(err: lodestone_minecraft::Error) -> Self {
        ModpackError::Invalid(err.to_string())
    }
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ModpackError + '_ {
    move |source| ModpackError::Io {
        path: path.to_owned(),
        source,
    }
}

fn invalid(message: impl std::fmt::Display) -> ModpackError {
    ModpackError::Invalid(format!("Not a valid Modrinth modpack: {message}"))
}

fn zip_error(err: zip::result::ZipError) -> ModpackError {
    invalid(format!("the zip can't be read ({err})"))
}

/// Where a file of the pack came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    /// Listed in the index and downloaded.
    #[default]
    Index,
    /// Copied from `overrides/` or `client-overrides/`.
    Override,
}

/// A file the pack put into the game folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFile {
    /// The pack's SHA-1 of the file (the user may have changed it since).
    pub sha1: String,
    #[serde(default)]
    pub source: FileSource,
    /// The Modrinth project of a downloaded file, from its CDN URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// `modpack.json`: the pack and version an instance was made from, and the files the pack
/// wrote, by path in the game folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackRecord {
    /// Empty for a pack that isn't on Modrinth.
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub date_published: String,
    /// Only ever a `https://cdn.modrinth.com/` URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub files: BTreeMap<String, PackFile>,
    /// The CurseForge project and file, for a pack from CurseForge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge: Option<CurseForgePack>,
}

/// The CurseForge file a modpack instance was made from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurseForgePack {
    pub project_id: u64,
    pub file_id: u64,
    #[serde(default)]
    pub file_date: String,
    /// The project's page on curseforge.com.
    #[serde(default)]
    pub page_url: String,
}

impl Document for ModpackRecord {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}

impl ModpackRecord {
    pub fn path(instance_dir: &Path) -> PathBuf {
        instance_dir.join(RECORD_FILE)
    }

    /// The instance's record, if it was made from a modpack. `None` when the instance has
    /// no `modpack.json` (`storage::load` would otherwise hand back a default record, and
    /// every instance would look like it came from a pack that isn't on Modrinth).
    pub fn load(instance_dir: &Path) -> Option<Self> {
        storage::load::<ModpackRecord>(&Self::path(instance_dir))
            .ok()
            .filter(|loaded| loaded.file_version.is_some())
            .map(|loaded| loaded.document)
    }

    pub fn save(&self, instance_dir: &Path) -> Result<(), StorageError> {
        storage::save(&Self::path(instance_dir), self)
    }

    /// Whether the pack is from CurseForge.
    pub fn on_curseforge(&self) -> bool {
        self.curseforge.is_some()
    }

    /// Whether the pack is on Modrinth, so it can be updated.
    pub fn on_modrinth(&self) -> bool {
        !self.project_id.is_empty() && !self.version_id.is_empty()
    }

    /// The record of `version` of `project`, without files.
    pub fn from_modrinth(
        project: &lodestone_minecraft::modrinth::ModrinthProject,
        version: &lodestone_minecraft::modrinth::RawVersion,
    ) -> Self {
        Self {
            project_id: project.id.clone(),
            version_id: version.id.clone(),
            slug: project.slug.clone(),
            title: project.title.clone(),
            version_number: version.version_number.clone(),
            date_published: version.date_published.clone(),
            icon_url: project
                .icon_url
                .as_deref()
                .and_then(crate::markdown::loadable_image),
            files: BTreeMap::new(),
            curseforge: None,
        }
    }
}

/// An override file of a pack: the one that wins when `client-overrides/` has the same
/// path as `overrides/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideFile {
    /// Relative to the game folder, with `/`.
    pub path: String,
    /// The zip entry.
    pub index: usize,
    pub size: u64,
    pub sha1: String,
}

/// A pack that was read and checked.
#[derive(Clone, Debug)]
pub struct PackInfo {
    pub name: String,
    pub version_number: Option<String>,
    pub summary: Option<String>,
    pub game_version: String,
    /// The loader and its version as the pack writes it (`47.4.28` for Forge).
    pub loader: Option<(LoaderKind, String)>,
    /// The files to download for the client.
    pub downloads: Vec<PackDownload>,
    pub overrides: Vec<OverrideFile>,
    pub skipped: Vec<SkippedPath>,
}

impl PackInfo {
    pub fn download_size(&self) -> u64 {
        self.downloads.iter().map(|d| d.size).sum()
    }

    /// "Fabric 0.16.14", or "Vanilla".
    pub fn loader_label(&self) -> String {
        match &self.loader {
            Some((kind, version)) => format!("{} {version}", kind.label()),
            None => "Vanilla".into(),
        }
    }
}

/// Paths a pack may not write: Lodestone's own temporary files.
pub(crate) struct ClientPolicy;

impl PathPolicy for ClientPolicy {
    fn skip_reason(&self, _: &Path, components: &[&str], _: bool) -> Option<String> {
        components
            .iter()
            .any(|c| c.to_ascii_lowercase().starts_with(".lodestone"))
            .then(|| "Lodestone's own files can't come from a modpack".to_string())
    }
}

/// The loader a pack's `dependencies` ask for, or why the pack can't be used.
pub fn pack_loader(
    dependencies: &std::collections::HashMap<String, String>,
) -> Result<Option<(LoaderKind, String)>, String> {
    let mut loader = None;
    let mut keys: Vec<&String> = dependencies.keys().collect();
    keys.sort();
    for key in keys {
        let version = dependencies[key].trim();
        let kind = match key.as_str() {
            "minecraft" => continue,
            "fabric-loader" => LoaderKind::Fabric,
            "quilt-loader" => LoaderKind::Quilt,
            "forge" => LoaderKind::Forge,
            "neoforge" => LoaderKind::NeoForge,
            other => {
                return Err(format!(
                    "The pack needs {other}, which Lodestone Launcher doesn't support."
                ));
            }
        };
        if !is_valid_filter_value(version) {
            return Err(format!(
                "The pack's {} version {version:?} isn't valid.",
                kind.label()
            ));
        }
        if let Some((first, _)) = &loader {
            return Err(format!(
                "The pack asks for two loaders, {} and {}.",
                LoaderKind::label(*first),
                kind.label()
            ));
        }
        loader = Some((kind, version.to_string()));
    }
    Ok(loader)
}

/// The instance's loader for a pack's loader on `game_version`. Forge's version (`47.4.28`
/// in a pack) becomes the full Maven version (`1.20.1-47.4.28`); when `forge_versions` is
/// given (Forge's list for that Minecraft version), it is matched against it, which also
/// finds old builds such as `1.8.9-11.15.1.2318-1.8.9`.
pub fn loader_choice(
    kind: LoaderKind,
    version: &str,
    game_version: &str,
    forge_versions: Option<&[LoaderVersion]>,
) -> Result<LoaderChoice, String> {
    let version = version.trim();
    let version = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => version.to_string(),
        LoaderKind::Forge => {
            let listed = forge_versions.and_then(|list| {
                list.iter()
                    .find(|v| v.id == version || forge_display(&v.id, game_version) == version)
                    .map(|v| v.id.clone())
            });
            match listed {
                Some(id) => id,
                None if version.starts_with(&format!("{game_version}-")) => version.to_string(),
                None if forge_versions.is_some_and(|l| !l.is_empty()) => {
                    return Err(format!(
                        "Forge {version} isn't in Forge's list for Minecraft {game_version}."
                    ));
                }
                None => format!("{game_version}-{version}"),
            }
        }
        LoaderKind::NeoForge => match neoforge_game_version(version) {
            Some(game) if game == game_version => version.to_string(),
            _ if game_version == "1.20.1" => {
                return Err(
                    "The pack uses NeoForge for Minecraft 1.20.1 (its Forge fork), which Lodestone Launcher doesn't install."
                        .into(),
                );
            }
            _ => {
                return Err(format!(
                    "NeoForge {version} isn't for Minecraft {game_version}."
                ));
            }
        },
    };
    Ok(LoaderChoice { kind, version })
}

/// The Modrinth project of a file on Modrinth's CDN, from its URL
/// (`https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`), with the version.
pub fn cdn_ids(url: &url::Url) -> Option<(String, String)> {
    if url.scheme() != "https" || url.host_str() != Some(lodestone_minecraft::modrinth::CDN_HOST) {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.collect();
    match segments.as_slice() {
        ["data", project, "versions", version, _file] if !project.is_empty() => {
            Some((project.to_string(), version.to_string()))
        }
        _ => None,
    }
}

/// Opens the zip at `path` after checking it isn't unsafe: too many entries, or too large
/// once unpacked.
pub(crate) fn open_pack(path: &Path) -> Result<zip::ZipArchive<File>, ModpackError> {
    let file = File::open(path).map_err(io_error(path))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error)?;
    if archive.len() > MAX_ENTRIES {
        return Err(ModpackError::Invalid(format!(
            "The modpack is unsafe: it has more than {MAX_ENTRIES} files"
        )));
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(zip_error)?;
        total = total.saturating_add(entry.size());
    }
    if total > MAX_UNCOMPRESSED_BYTES {
        return Err(ModpackError::Invalid(
            "The modpack is unsafe: its files are larger than 4 GB unpacked".into(),
        ));
    }
    Ok(archive)
}

/// The override files under `prefixes` (later folders winning), each checked and hashed;
/// the paths that can't be written go to `skipped`.
pub(crate) fn read_overrides<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    prefixes: &[&str],
    skipped: &mut Vec<SkippedPath>,
) -> Result<Vec<OverrideFile>, ModpackError> {
    // The overrides, later folders winning, each hashed.
    let mut overrides: BTreeMap<String, OverrideFile> = BTreeMap::new();
    for prefix in prefixes {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(zip_error)?;
            let name = entry.name().to_string();
            let Some(relative) = name.strip_prefix(prefix) else {
                continue;
            };
            if relative.is_empty() {
                continue;
            }
            let is_dir = entry.is_dir();
            let relative_path = match check_pack_path(relative, is_dir, &ClientPolicy)? {
                PathCheck::Write(path) => path,
                PathCheck::Skip(reason) => {
                    skipped.push(SkippedPath {
                        path: relative.to_string(),
                        reason,
                    });
                    continue;
                }
            };
            if is_dir {
                continue;
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                skipped.push(SkippedPath {
                    path: relative.to_string(),
                    reason: "symbolic links are not copied".into(),
                });
                continue;
            }
            let size = entry.size();
            let mut hasher = sha1::Sha1::new();
            let mut buffer = [0u8; 64 * 1024];
            let mut read = 0u64;
            let mut limited = (&mut entry).take(size.saturating_add(1));
            loop {
                let n = limited
                    .read(&mut buffer)
                    .map_err(|e| invalid(format!("{relative} can't be read ({e})")))?;
                if n == 0 {
                    break;
                }
                read += n as u64;
                hasher.update(&buffer[..n]);
            }
            if read != size {
                return Err(ModpackError::Invalid(format!(
                    "The modpack is unsafe: {relative} isn't the size its zip header says"
                )));
            }
            let path = relative_path
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            overrides.insert(
                path.clone(),
                OverrideFile {
                    path,
                    index: i,
                    size,
                    sha1: format!("{:x}", hasher.finalize()),
                },
            );
        }
    }
    Ok(overrides.into_values().collect())
}

/// Reads and checks the pack at `path`. A pack that can't be used is an error that says
/// why.
pub fn read_pack(path: &Path) -> Result<PackInfo, ModpackError> {
    let mut archive = open_pack(path)?;
    let index: Index = {
        let entry = archive
            .by_name(INDEX_FILE)
            .map_err(|_| invalid(format!("it has no {INDEX_FILE}")))?;
        if entry.size() > MAX_INDEX_BYTES {
            return Err(invalid(format!("{INDEX_FILE} is too large")));
        }
        let mut bytes = Vec::new();
        entry
            .take(MAX_INDEX_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(format!("{INDEX_FILE} can't be read ({e})")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("{INDEX_FILE} is invalid ({e})")))?
    };
    if index.format_version != 1 {
        return Err(ModpackError::Invalid(format!(
            "Modpack format version {} isn't supported.",
            index.format_version
        )));
    }
    if index.game != "minecraft" {
        return Err(ModpackError::Invalid(format!(
            "The modpack is for {}, not Minecraft.",
            index.game
        )));
    }
    let game_version = index
        .dependencies
        .get("minecraft")
        .map(|v| v.trim().to_string())
        .ok_or_else(|| {
            ModpackError::Invalid(
                "The modpack doesn't say which Minecraft version it needs.".into(),
            )
        })?;
    if !is_valid_filter_value(&game_version) {
        return Err(ModpackError::Invalid(
            "The modpack's Minecraft version isn't valid.".into(),
        ));
    }
    let loader = pack_loader(&index.dependencies).map_err(ModpackError::Invalid)?;
    let (downloads, mut skipped) = check_files(&index, Side::Client, &ClientPolicy)?;

    let overrides = read_overrides(&mut archive, &Side::Client.override_dirs(), &mut skipped)?;
    let name = index.name.trim().to_string();
    Ok(PackInfo {
        name: if name.is_empty() {
            "Modpack".into()
        } else {
            name
        },
        version_number: index.version_id.filter(|v| !v.trim().is_empty()),
        summary: index.summary.filter(|v| !v.trim().is_empty()),
        game_version,
        loader,
        downloads,
        overrides,
        skipped,
    })
}

/// A path of a download with `/`, as `modpack.json` and `content.json` key them.
pub(crate) fn slash_path(path: &Path) -> String {
    path.iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instance_without_modpack_json_has_no_modpack_record() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ModpackRecord::load(dir.path()).is_none());
        let record = ModpackRecord {
            slug: "pack".into(),
            ..ModpackRecord::default()
        };
        record.save(dir.path()).unwrap();
        assert_eq!(ModpackRecord::load(dir.path()).unwrap().slug, "pack");
    }

    fn versions(ids: &[&str]) -> Vec<LoaderVersion> {
        ids.iter()
            .map(|id| LoaderVersion {
                id: id.to_string(),
                display: id.to_string(),
                stable: true,
                recommended: false,
            })
            .collect()
    }

    #[test]
    fn loaders() {
        let deps = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<std::collections::HashMap<_, _>>()
        };
        assert_eq!(
            pack_loader(&deps(&[
                ("minecraft", "1.20.1"),
                ("fabric-loader", "0.16.14")
            ])),
            Ok(Some((LoaderKind::Fabric, "0.16.14".into())))
        );
        assert_eq!(pack_loader(&deps(&[("minecraft", "1.20.1")])), Ok(None));
        assert!(
            pack_loader(&deps(&[("forge", "47.4.28"), ("fabric-loader", "0.16.14")]))
                .unwrap_err()
                .contains("two loaders")
        );
        assert!(
            pack_loader(&deps(&[("liteloader", "1")]))
                .unwrap_err()
                .contains("liteloader")
        );
        assert!(pack_loader(&deps(&[("forge", "47/../x")])).is_err());

        let choice = |kind, v: &str, game: &str, list: Option<&[LoaderVersion]>| {
            loader_choice(kind, v, game, list).map(|c| c.version)
        };
        assert_eq!(
            choice(LoaderKind::Forge, "47.4.28", "1.20.1", None).unwrap(),
            "1.20.1-47.4.28"
        );
        assert_eq!(
            choice(LoaderKind::Forge, "1.20.1-47.4.28", "1.20.1", None).unwrap(),
            "1.20.1-47.4.28"
        );
        let old = versions(&["1.8.9-11.15.1.2318-1.8.9", "1.8.9-11.15.1.1722"]);
        assert_eq!(
            choice(LoaderKind::Forge, "11.15.1.2318", "1.8.9", Some(&old)).unwrap(),
            "1.8.9-11.15.1.2318-1.8.9"
        );
        assert!(choice(LoaderKind::Forge, "99.0", "1.8.9", Some(&old)).is_err());
        assert_eq!(
            choice(LoaderKind::NeoForge, "21.1.209", "1.21.1", None).unwrap(),
            "21.1.209"
        );
        assert!(choice(LoaderKind::NeoForge, "21.1.209", "1.20.4", None).is_err());
        assert!(
            choice(LoaderKind::NeoForge, "47.1.106", "1.20.1", None)
                .unwrap_err()
                .contains("1.20.1")
        );
        assert_eq!(
            choice(LoaderKind::Quilt, "0.29.2", "1.21.1", None).unwrap(),
            "0.29.2"
        );
    }

    #[test]
    fn cdn_urls() {
        let url = |s: &str| url::Url::parse(s).unwrap();
        assert_eq!(
            cdn_ids(&url(
                "https://cdn.modrinth.com/data/AANobbMI/versions/OihdIimA/sodium.jar"
            )),
            Some(("AANobbMI".into(), "OihdIimA".into()))
        );
        assert_eq!(
            cdn_ids(&url(
                "https://github.com/data/AANobbMI/versions/x/sodium.jar"
            )),
            None
        );
        assert_eq!(cdn_ids(&url("https://cdn.modrinth.com/data/x.jar")), None);
    }
}
