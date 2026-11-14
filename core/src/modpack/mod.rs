//! Modrinth modpacks (`.mrpack`): reading and checking a pack, and putting its server files
//! into a new instance folder.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.
//! Fabric, Forge and NeoForge packs are supported. The index, the zip limits and the path
//! checks are in [`lodestone_minecraft::mrpack`]; the zip crate's own `extract` is never
//! used.

pub mod history;
pub mod store;
pub mod update;
pub mod updates;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use futures::TryStreamExt;
use lodestone_minecraft::mrpack::{
    self, check_files, check_overrides, check_zip_limits, for_each_override, open_pack, read_index,
    OverrideItem, PathPolicy, Side,
};
pub use lodestone_minecraft::mrpack::{
    PackDownload, SkippedPath, MAX_ENTRIES, MAX_PACK_FILE_BYTES, MAX_UNCOMPRESSED_BYTES,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::handlers::util::is_script_path;
use crate::implementations::minecraft::util::forge_build_for_pack;
use crate::implementations::minecraft::{
    FabricLoaderVersion, Flavour, ForgeBuildVersion, NeoForgeBuildVersion,
};
use crate::modrinth::ModrinthApi;
use crate::mods::bad_request;

/// Files are downloaded this many at a time.
const PARALLEL_DOWNLOADS: usize = 4;
/// Files at the root of the instance a pack may not replace: the EULA, which the user
/// accepts, and the files Lodestone uses to start the server.
const PROTECTED_ROOT_FILES: [&str; 5] = [
    "eula.txt",
    "server.jar",
    "user_jvm_args.txt",
    "unix_args.txt",
    "win_args.txt",
];

/// What the dashboard shows about a pack before an instance is created from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackInfo {
    pub name: String,
    pub version_number: Option<String>,
    pub summary: Option<String>,
    pub game_version: String,
    /// `fabric`, `forge` or `neoforge` for a supported pack.
    pub loader: String,
    pub loader_version: Option<String>,
    pub server_file_count: u32,
    #[ts(type = "number")]
    pub server_download_size: u64,
    /// Set when no instance can be created from the pack.
    pub unsupported_reason: Option<String>,
    /// Pass this to `create_modpack` to reuse the downloaded or uploaded pack.
    pub upload_id: Option<String>,
}

/// Where an instance created from a modpack came from. Kept in the instance's config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ModpackOrigin {
    /// Empty for an uploaded pack.
    pub project_id: String,
    /// Empty for an uploaded pack.
    pub version_id: String,
    pub name: String,
    pub version_number: Option<String>,
    /// The project icon, only ever a `https://cdn.modrinth.com/` URL.
    pub icon_url: Option<String>,
}

/// `url` if it is an `https://cdn.modrinth.com/` URL, else `None`.
pub fn modrinth_cdn_url(url: Option<&str>) -> Option<String> {
    let parsed = url::Url::parse(url?).ok()?;
    let ok = parsed.scheme() == "https"
        && parsed.host_str() == Some(crate::modrinth::CDN_HOST)
        && parsed.port().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none();
    ok.then(|| parsed.to_string())
}

/// The changelog of the Modrinth version `version_id`, if it has one.
pub async fn modrinth_changelog(api: &dyn ModrinthApi, version_id: &str) -> Option<String> {
    api.version(version_id).await.ok()?.changelog
}

impl ModpackOrigin {
    /// The origin of a pack from the Modrinth version `version_id`: the project's title and
    /// icon, and the version number.
    pub async fn from_modrinth(api: &dyn ModrinthApi, version_id: &str) -> Result<Self, Error> {
        let version = api.version(version_id).await?;
        let project = api.project(&version.project_id).await?;
        Ok(ModpackOrigin {
            project_id: project.id,
            version_id: version.id,
            name: project.title,
            version_number: Some(version.version_number).filter(|v| !v.trim().is_empty()),
            icon_url: modrinth_cdn_url(project.icon_url.as_deref()),
        })
    }

    /// The origin of an uploaded pack, from its index.
    pub fn from_upload(info: &ModpackInfo) -> Self {
        ModpackOrigin {
            project_id: String::new(),
            version_id: String::new(),
            name: info.name.clone(),
            version_number: info.version_number.clone(),
            icon_url: None,
        }
    }
}

/// A pack that was read and checked.
#[derive(Debug, Clone)]
pub struct Pack {
    pub info: ModpackInfo,
    pub downloads: Vec<PackDownload>,
    pub skipped: Vec<SkippedPath>,
}

fn invalid_pack(message: impl std::fmt::Display) -> Error {
    mrpack::invalid_pack(message).into()
}

fn unsafe_pack(message: impl std::fmt::Display) -> Error {
    mrpack::unsafe_pack(message).into()
}

/// The paths a pack may not write into an instance: anything named `.lodestone*` at any
/// depth (Lodestone's own files), the protected files at the root
/// (such as `eula.txt`), and scripts unless `allow_scripts`.
struct InstancePathPolicy {
    allow_scripts: bool,
}

impl PathPolicy for InstancePathPolicy {
    fn skip_reason(&self, relative: &Path, components: &[&str], is_dir: bool) -> Option<String> {
        if components
            .iter()
            .any(|c| c.to_lowercase().starts_with(".lodestone"))
        {
            return Some("Lodestone's own files cannot be replaced".into());
        }
        if components.len() == 1
            && PROTECTED_ROOT_FILES.contains(&components[0].to_lowercase().as_str())
        {
            return Some("this file cannot come from a modpack".into());
        }
        if !is_dir && !self.allow_scripts && is_script_path(relative) {
            return Some(
                "script file: only the owner can add scripts, while code execution is on".into(),
            );
        }
        None
    }
}

/// Whether `value` is a plausible Minecraft or loader version, which is used in URLs.
fn is_valid_version(value: &str) -> bool {
    crate::modrinth::search::is_valid_filter_value(value)
}

/// The mod loaders a pack can need, by their key in the index's `dependencies`, with the
/// loader name Lodestone uses.
const PACK_LOADERS: [(&str, &str); 3] = [
    ("fabric-loader", "fabric"),
    ("forge", "forge"),
    ("neoforge", "neoforge"),
];

/// The loader name for a dependency key Lodestone has no server for (`quilt-loader` is
/// `quilt`).
fn loader_name(key: &str) -> String {
    key.trim_end_matches("-loader").to_string()
}

/// The display name of a loader Lodestone runs.
fn loader_title(loader: &str) -> &'static str {
    match loader {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "mod loader",
    }
}

/// Which loader a pack with these dependencies needs, its version, and why no instance
/// can be made from it, if that is so. Lodestone runs Fabric, Forge and NeoForge servers;
/// a pack must need exactly one of them, and nothing else.
fn check_dependencies(
    dependencies: &HashMap<String, String>,
    game_version: Option<&str>,
) -> (String, Option<String>, Option<String>) {
    let mut others: Vec<&str> = dependencies
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "minecraft" && !PACK_LOADERS.iter().any(|(key, _)| key == k))
        .collect();
    others.sort();
    let needed: Vec<(&str, &str)> = PACK_LOADERS
        .into_iter()
        .filter(|(key, _)| dependencies.contains_key(*key))
        .collect();
    if let Some(other) = others.first() {
        return (
            loader_name(other),
            dependencies.get(*other).cloned(),
            Some(format!(
                "Only Fabric, Forge and NeoForge modpacks are supported, and this pack needs {}",
                others.join(", ")
            )),
        );
    }
    let (key, loader) = match needed.as_slice() {
        [] => {
            return (
                "vanilla".into(),
                None,
                Some("Only modded packs are supported, and this pack needs no mod loader".into()),
            )
        }
        [one] => *one,
        [first, ..] => {
            let keys: Vec<&str> = needed.iter().map(|(key, _)| *key).collect();
            return (
                first.1.to_string(),
                dependencies.get(first.0).cloned(),
                Some(format!(
                    "The pack needs more than one mod loader ({})",
                    keys.join(", ")
                )),
            );
        }
    };
    let version = dependencies[key].clone();
    let title = loader_title(loader);
    let reason = if !is_valid_version(&version) {
        Some(format!("The {title} version {version:?} is not valid"))
    } else if loader == "neoforge" {
        // NeoForge for 1.20.1 was a Forge fork with other versions (`47.1.x`); Lodestone
        // runs NeoForge for 1.20.2 and later
        let for_game = lodestone_minecraft::versions::neoforge::neoforge_game_version(&version);
        match (for_game, game_version) {
            (Some(for_game), Some(game)) if for_game != game => Some(format!(
                "NeoForge {version} is for Minecraft {for_game}, but the pack is for {game}"
            )),
            (None, Some(game)) => Some(format!(
                "NeoForge {version} for Minecraft {game} is not supported: NeoForge servers                  need Minecraft 1.20.2 or later"
            )),
            _ => None,
        }
    } else {
        None
    };
    (loader.to_string(), Some(version), reason)
}

/// The server software for a supported pack: its loader at the version it names. A Forge
/// pack names `47.2.0`, which is looked up in Forge's version list as `1.20.1-47.2.0`.
pub async fn server_flavour(info: &ModpackInfo) -> Result<Flavour, Error> {
    let title = loader_title(&info.loader);
    let version = info
        .loader_version
        .clone()
        .ok_or_else(|| bad_request(format!("The modpack does not say which {title} it needs")))?;
    Ok(match info.loader.as_str() {
        "fabric" => Flavour::Fabric {
            loader_version: Some(FabricLoaderVersion::new(version)),
            installer_version: None,
        },
        "forge" => Flavour::Forge {
            build_version: Some(ForgeBuildVersion::new(
                forge_build_for_pack(&info.game_version, &version).await?,
            )),
        },
        "neoforge" => Flavour::NeoForge {
            build_version: Some(NeoForgeBuildVersion::new(version)),
        },
        other => return Err(bad_request(format!("{other} modpacks are not supported"))),
    })
}

/// Reads and checks the pack at `path`. A pack that cannot be used for an instance is
/// returned with `unsupported_reason` set; a pack that is not a valid `.mrpack`, or is
/// unsafe, is an error.
pub fn read_pack(path: &Path, allow_scripts: bool) -> Result<Pack, Error> {
    let policy = InstancePathPolicy { allow_scripts };
    let mut archive = open_pack(path)?;
    check_zip_limits(&mut archive, MAX_ENTRIES, MAX_UNCOMPRESSED_BYTES)?;
    let index = read_index(&mut archive)?;
    let game_version = index.dependencies.get("minecraft").cloned();
    let (loader, loader_version, mut reason) =
        check_dependencies(&index.dependencies, game_version.as_deref());
    if index.format_version != 1 {
        reason = Some(format!(
            "Pack format version {} is not supported",
            index.format_version
        ));
    } else if index.game != "minecraft" {
        reason = Some(format!("The pack is for {}, not Minecraft", index.game));
    } else if game_version.is_none() {
        reason = Some("The pack does not say which Minecraft version it needs".into());
    } else if game_version
        .as_deref()
        .is_some_and(|v| !is_valid_version(v))
    {
        reason = Some("The pack's Minecraft version is not valid".into());
    }
    let (downloads, mut skipped) = check_files(&index, Side::Server, &policy)?;
    skipped.extend(check_overrides(&mut archive, Side::Server, &policy)?);
    let name = index.name.trim().to_string();
    let info = ModpackInfo {
        name: if name.is_empty() {
            "Modpack".to_string()
        } else {
            name
        },
        version_number: index
            .version_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(String::from),
        summary: index
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(String::from),
        game_version: game_version.unwrap_or_default(),
        loader,
        loader_version,
        server_file_count: downloads.len() as u32,
        server_download_size: downloads.iter().map(|d| d.size).sum(),
        unsupported_reason: reason,
        upload_id: None,
    };
    Ok(Pack {
        info,
        downloads,
        skipped,
    })
}

/// Reads and checks a pack on a blocking thread, see [`read_pack`].
pub async fn read_pack_async(path: PathBuf, allow_scripts: bool) -> Result<Pack, Error> {
    tokio::task::spawn_blocking(move || read_pack(&path, allow_scripts))
        .await
        .map_err(|e| eyre!("The modpack task failed: {e}"))?
}

/// Creates the parent folders of `target` and checks that they are inside `root` once
/// links are resolved.
fn prepare_parent(root: &Path, target: &Path) -> Result<PathBuf, Error> {
    let parent = target
        .parent()
        .ok_or_else(|| eyre!("{} has no parent folder", target.display()))?;
    std::fs::create_dir_all(parent).context("Failed to create a folder for the modpack")?;
    let canonical_root = std::fs::canonicalize(root).context("Failed to read the server folder")?;
    let canonical_parent =
        std::fs::canonicalize(parent).context("Failed to read a folder of the modpack")?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(unsafe_pack("a path leaves the server folder"));
    }
    Ok(parent.to_path_buf())
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// A file written next to where it goes and moved there once complete, or deleted if
/// it never is. `tempfile`'s own `persist` can't move a file whose path is longer than
/// Windows' 260 characters (a pack's deep config path in a server folder can be);
/// `std::fs` adds the long-path prefix itself.
struct PartFile {
    path: PathBuf,
    moved: bool,
}

impl PartFile {
    /// A new, empty file named `<prefix><random>.tmp` in `dir`.
    fn create(dir: &Path, prefix: &str) -> std::io::Result<(PartFile, std::fs::File)> {
        loop {
            let path = dir.join(format!("{prefix}{}.tmp", uuid::Uuid::new_v4().simple()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok((PartFile { path, moved: false }, file)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Moves the file to `target`, replacing what is there.
    fn persist(mut self, target: &Path) -> std::io::Result<()> {
        std::fs::rename(&self.path, target)?;
        self.moved = true;
        Ok(())
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if !self.moved {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Downloads one file of the pack, trying its URLs in order.
async fn download_one(
    api: &dyn ModrinthApi,
    root: &Path,
    download: &PackDownload,
) -> Result<(), Error> {
    let target = root.join(&download.path);
    let parent = {
        let (root, target) = (root.to_path_buf(), target.clone());
        tokio::task::spawn_blocking(move || prepare_parent(&root, &target))
            .await
            .map_err(|e| eyre!("The modpack task failed: {e}"))??
    };
    if is_link(&target) {
        return Err(unsafe_pack("a path leaves the server folder"));
    }
    let mut last_error = None;
    for url in &download.urls {
        let (temp, file) = PartFile::create(&parent, ".lodestone-download-")
            .context("Failed to create a temporary file")?;
        let mut file = tokio::fs::File::from_std(file);
        let result = api.download(url, &mut file, MAX_PACK_FILE_BYTES).await;
        drop(file);
        match result {
            Ok(hashes) if hashes.sha1 == download.sha1 && hashes.sha512 == download.sha512 => {
                temp.persist(&target)
                    .map_err(|e| eyre!("Failed to save {}: {e}", download.path.display()))?;
                return Ok(());
            }
            Ok(_) => {
                last_error = Some(format!(
                    "the download from {} is damaged (hash mismatch)",
                    url.host_str().unwrap_or("")
                ));
            }
            Err(e) => last_error = Some(e.source.to_string()),
        }
    }
    Err(Error {
        kind: ErrorKind::External,
        source: eyre!(
            "Could not download {}: {}",
            download.path.display(),
            last_error.unwrap_or_else(|| "no download URL".into())
        ),
    })
}

/// Copies one override file into place through a temporary file.
fn write_override<R: Read>(
    root: &Path,
    relative: &Path,
    entry: &mut zip::read::ZipFile<'_, R>,
    budget: &mut u64,
) -> Result<(), Error> {
    let target = root.join(relative);
    let parent = prepare_parent(root, &target)?;
    if is_link(&target) {
        return Err(unsafe_pack("a path leaves the server folder"));
    }
    let (temp, mut file) = PartFile::create(&parent, ".lodestone-extract-")
        .context("Failed to create a temporary file")?;
    // never trust the size in the zip header
    let limit = (*budget).min(entry.size()).saturating_add(1);
    let copied = std::io::copy(&mut entry.take(limit), &mut file)
        .map_err(|e| invalid_pack(format!("{} cannot be read ({e})", relative.display())))?;
    if copied > entry.size() || copied > *budget {
        return Err(unsafe_pack(format!(
            "{} is larger than its zip header says",
            relative.display()
        )));
    }
    *budget -= copied;
    file.flush().context("Failed to write a modpack file")?;
    drop(file);
    temp.persist(&target)
        .map_err(|e| eyre!("Failed to save {}: {e}", relative.display()))?;
    Ok(())
}

/// Copies `overrides/`, then `server-overrides/`, into `root`.
fn extract_overrides(pack_path: &Path, root: &Path, allow_scripts: bool) -> Result<usize, Error> {
    let mut archive = open_pack(pack_path)?;
    check_zip_limits(&mut archive, MAX_ENTRIES, MAX_UNCOMPRESSED_BYTES)?;
    let mut budget = MAX_UNCOMPRESSED_BYTES;
    let mut written = 0;
    let policy = InstancePathPolicy { allow_scripts };
    for_each_override(&mut archive, Side::Server, &policy, |_, item| {
        match item {
            OverrideItem::Dir(path) => {
                let dir = root.join(&path);
                prepare_parent(root, &dir.join("x"))?;
            }
            OverrideItem::File(path, mut entry) => {
                write_override(root, &path, &mut entry, &mut budget)?;
                written += 1;
            }
            OverrideItem::Skip { .. } => {}
        }
        Ok::<_, Error>(())
    })?;
    Ok(written)
}

/// The SHA-1 of each override file a pack writes into an instance, by its path with `/`
/// separators. `server-overrides/` comes after `overrides/`, so its files win.
fn override_hashes(
    pack_path: &Path,
    allow_scripts: bool,
) -> Result<std::collections::BTreeMap<String, String>, Error> {
    use sha1::Digest;
    let mut archive = open_pack(pack_path)?;
    check_zip_limits(&mut archive, MAX_ENTRIES, MAX_UNCOMPRESSED_BYTES)?;
    let mut budget = MAX_UNCOMPRESSED_BYTES;
    let mut hashes = std::collections::BTreeMap::new();
    let policy = InstancePathPolicy { allow_scripts };
    for_each_override(&mut archive, Side::Server, &policy, |_, item| {
        if let OverrideItem::File(path, entry) = item {
            let size = entry.size();
            let mut hasher = sha1::Sha1::new();
            let mut reader = entry.take(budget.min(size).saturating_add(1));
            let mut buffer = vec![0_u8; 64 * 1024];
            let mut read_total = 0_u64;
            loop {
                let read = reader.read(&mut buffer).map_err(|e| {
                    invalid_pack(format!("{} cannot be read ({e})", path.display()))
                })?;
                if read == 0 {
                    break;
                }
                read_total += read as u64;
                hasher.update(&buffer[..read]);
            }
            if read_total > size || read_total > budget {
                return Err(unsafe_pack(format!(
                    "{} is larger than its zip header says",
                    path.display()
                )));
            }
            budget -= read_total;
            hashes.insert(slash_path(&path), format!("{:x}", hasher.finalize()));
        }
        Ok::<_, Error>(())
    })?;
    Ok(hashes)
}

/// A relative path with `/` separators, as records and messages show it.
pub(crate) fn slash_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The result of [`install_pack_contents`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackInstallReport {
    pub downloaded: usize,
    pub extracted: usize,
    pub skipped: Vec<SkippedPath>,
    /// The pack's files, as recorded.
    pub files: std::collections::BTreeMap<String, update::PackFile>,
}

/// Puts the server files of a checked pack into the instance folder `root`: downloads
/// `files[]` (at most [`PARALLEL_DOWNLOADS`] at a time), then copies `overrides/` and
/// `server-overrides/`, and records them in [`update::RECORD_FILE`]. `progress` gets a message and the progress made.
pub async fn install_pack_contents(
    api: &dyn ModrinthApi,
    pack: &Pack,
    pack_path: &Path,
    root: &Path,
    allow_scripts: bool,
    progress: &(dyn Fn(String, f64) + Send + Sync),
) -> Result<PackInstallReport, Error> {
    let total = pack.downloads.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    futures::stream::iter(pack.downloads.iter().map(Ok::<_, Error>))
        .try_for_each_concurrent(PARALLEL_DOWNLOADS, |download| {
            let done = &done;
            async move {
                download_one(api, root, download).await?;
                let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                progress(
                    format!("Downloading modpack files ({n}/{total})"),
                    4.0 / total.max(1) as f64,
                );
                Ok(())
            }
        })
        .await?;

    progress("Copying modpack overrides".to_string(), 0.0);
    let extracted = {
        let (pack_path, root) = (pack_path.to_path_buf(), root.to_path_buf());
        tokio::task::spawn_blocking(move || extract_overrides(&pack_path, &root, allow_scripts))
            .await
            .map_err(|e| eyre!("The modpack task failed: {e}"))??
    };
    progress("Copied modpack overrides".to_string(), 1.0);
    // the record lets the server be updated to another version of the pack later
    let files = {
        let (pack, pack_path) = (pack.clone(), pack_path.to_path_buf());
        tokio::task::spawn_blocking(move || update::pack_files(&pack, &pack_path, allow_scripts))
            .await
            .map_err(|e| eyre!("The modpack task failed: {e}"))??
    };
    update::record_of(&files).save(root).await?;
    Ok(PackInstallReport {
        downloaded: total,
        extracted,
        skipped: pack.skipped.clone(),
        files,
    })
}

/// A short list of skipped files for a message, such as the end of instance creation.
pub fn describe_skipped(skipped: &[SkippedPath]) -> Option<String> {
    const SHOWN: usize = 10;
    if skipped.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = skipped
        .iter()
        .take(SHOWN)
        .map(|s| format!("{} ({})", s.path, s.reason))
        .collect();
    if skipped.len() > SHOWN {
        parts.push(format!("and {} more", skipped.len() - SHOWN));
    }
    Some(format!(
        "{} modpack file(s) were skipped: {}",
        skipped.len(),
        parts.join("; ")
    ))
}

/// Writes `server-port` into `server.properties` again, keeping the rest of the file:
/// overrides can contain that file.
pub async fn write_port(root: &Path, port: u32) -> Result<(), Error> {
    use crate::implementations::minecraft::properties::PropertiesDocument;
    let path = root.join("server.properties");
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).context("Failed to read server.properties")?,
    };
    let (mut doc, _) = PropertiesDocument::from_bytes(&bytes);
    doc.set("server-port", &port.to_string());
    crate::persist::write_atomic(&path, doc.to_bytes(), None).await
}

#[cfg(test)]
mod tests;
