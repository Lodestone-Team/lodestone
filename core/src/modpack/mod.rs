//! Modrinth modpacks (`.mrpack`): reading and checking a pack, and putting its server files
//! into a new instance folder.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.
//! Only Fabric packs are supported. The index, the zip limits and the path checks are in
//! [`lodestone_minecraft::mrpack`]; the zip crate's own `extract` is never used.

pub mod store;

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
use crate::handlers::util::{is_in_macros_dir, is_script_path};
use crate::modrinth::ModrinthApi;

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
    /// For example `fabric`.
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
/// depth (Lodestone's own files), anything in `macros/`, the protected files at the root
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
        if is_in_macros_dir(relative) {
            return Some("macros cannot come from a modpack".into());
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

/// Why no instance can be made from a pack with these dependencies, if any. Returns the
/// loader name and version too.
fn check_dependencies(
    dependencies: &HashMap<String, String>,
) -> (String, Option<String>, Option<String>) {
    let mut others: Vec<&str> = dependencies
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "minecraft" && *k != "fabric-loader")
        .collect();
    others.sort();
    let loader_name = |key: &str| key.trim_end_matches("-loader").to_string();
    match (dependencies.get("fabric-loader"), others.first()) {
        (Some(version), None) => {
            let reason = (!is_valid_version(version))
                .then(|| format!("The Fabric loader version {version:?} is not valid"));
            ("fabric".into(), Some(version.clone()), reason)
        }
        (_, Some(other)) => (
            loader_name(other),
            dependencies.get(*other).cloned(),
            Some(format!(
                "Only Fabric modpacks are supported, and this pack needs {}",
                others.join(", ")
            )),
        ),
        (None, None) => (
            "vanilla".into(),
            None,
            Some("Only Fabric modpacks are supported, and this pack needs no mod loader".into()),
        ),
    }
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
    let (loader, loader_version, mut reason) = check_dependencies(&index.dependencies);
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
        version_number: index.version_id.clone().filter(|v| !v.trim().is_empty()),
        summary: index.summary.clone().filter(|v| !v.trim().is_empty()),
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
    let canonical_root =
        std::fs::canonicalize(root).context("Failed to read the instance folder")?;
    let canonical_parent =
        std::fs::canonicalize(parent).context("Failed to read a folder of the modpack")?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(unsafe_pack("a path leaves the instance folder"));
    }
    Ok(parent.to_path_buf())
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
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
        return Err(unsafe_pack("a path leaves the instance folder"));
    }
    let mut last_error = None;
    for url in &download.urls {
        let temp = tempfile::Builder::new()
            .prefix(".lodestone-download-")
            .suffix(".tmp")
            .tempfile_in(&parent)
            .context("Failed to create a temporary file")?
            .into_temp_path();
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&temp)
            .await
            .context("Failed to open a temporary file")?;
        let result = api.download(url, &mut file, MAX_PACK_FILE_BYTES).await;
        drop(file);
        match result {
            Ok(hashes) if hashes.sha1 == download.sha1 && hashes.sha512 == download.sha512 => {
                temp.persist(&target).map_err(|e| {
                    eyre!("Failed to save {}: {}", download.path.display(), e.error)
                })?;
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
        return Err(unsafe_pack("a path leaves the instance folder"));
    }
    let mut temp = tempfile::Builder::new()
        .prefix(".lodestone-extract-")
        .suffix(".tmp")
        .tempfile_in(&parent)
        .context("Failed to create a temporary file")?;
    // never trust the size in the zip header
    let limit = (*budget).min(entry.size()).saturating_add(1);
    let copied = std::io::copy(&mut entry.take(limit), temp.as_file_mut())
        .map_err(|e| invalid_pack(format!("{} cannot be read ({e})", relative.display())))?;
    if copied > entry.size() || copied > *budget {
        return Err(unsafe_pack(format!(
            "{} is larger than its zip header says",
            relative.display()
        )));
    }
    *budget -= copied;
    temp.as_file_mut()
        .flush()
        .context("Failed to write a modpack file")?;
    temp.persist(&target)
        .map_err(|e| eyre!("Failed to save {}: {}", relative.display(), e.error))?;
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

/// The result of [`install_pack_contents`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PackInstallReport {
    pub downloaded: usize,
    pub extracted: usize,
    pub skipped: Vec<SkippedPath>,
}

/// Puts the server files of a checked pack into the instance folder `root`: downloads
/// `files[]` (at most [`PARALLEL_DOWNLOADS`] at a time), then copies `overrides/` and
/// `server-overrides/`. `progress` gets a message and the progress made.
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
    Ok(PackInstallReport {
        downloaded: total,
        extracted,
        skipped: pack.skipped.clone(),
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
