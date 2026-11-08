//! Modrinth modpacks (`.mrpack`): reading and checking a pack, and putting its server files
//! into a new instance folder.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.
//! Only Fabric packs are supported. Every path in a pack is checked here; the zip crate's
//! own `extract` is never used.

pub mod store;

use std::collections::HashMap;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::handlers::util::{is_in_macros_dir, is_script_path};
use crate::modrinth::ModrinthApi;

/// The index file at the root of a pack.
const INDEX_FILE: &str = "modrinth.index.json";
/// The index may be at most this large.
const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
/// A pack may have at most this many zip entries.
pub const MAX_ENTRIES: usize = 20_000;
/// The files in a pack may be at most this large together, uncompressed.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// A `.mrpack` file, and each file it downloads, may be at most this large.
pub const MAX_PACK_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// A path in a pack may be at most this many characters long.
pub const MAX_PATH_CHARS: usize = 512;
/// Files are downloaded this many at a time.
const PARALLEL_DOWNLOADS: usize = 4;
/// The hosts files may be downloaded from (the allow-list of the Modrinth pack format).
pub const DOWNLOAD_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];
/// The folders of a pack whose contents are copied into the instance, in order: later ones
/// win. `client-overrides/` is skipped.
const OVERRIDE_DIRS: [&str; 2] = ["overrides/", "server-overrides/"];
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexEnv {
    #[serde(default)]
    server: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<IndexEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

impl IndexFile {
    /// Files without `env`, or whose server side is not `unsupported`, are server files.
    fn is_server_file(&self) -> bool {
        self.env
            .as_ref()
            .and_then(|e| e.server.as_deref())
            .is_none_or(|s| s != "unsupported")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    format_version: u32,
    game: String,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    files: Vec<IndexFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

/// A file to download into the instance.
#[derive(Debug, Clone, PartialEq)]
pub struct PackDownload {
    /// Relative to the instance folder.
    pub path: PathBuf,
    pub urls: Vec<url::Url>,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
}

/// A file of a pack that was not put into the instance, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedPath {
    pub path: String,
    pub reason: String,
}

/// A pack that was read and checked.
#[derive(Debug, Clone)]
pub struct Pack {
    pub info: ModpackInfo,
    pub downloads: Vec<PackDownload>,
    pub skipped: Vec<SkippedPath>,
}

fn invalid_pack(message: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Not a valid Modrinth modpack: {message}"),
    }
}

fn unsafe_pack(message: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("The modpack is unsafe: {message}"),
    }
}

/// What to do with a path from a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCheck {
    Write(PathBuf),
    Skip(String),
}

/// Windows device names, which cannot be used as file names there.
fn is_windows_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Checks a path from a pack (a `files[]` path, or an override relative to its folder).
///
/// Rejected, so the whole pack fails: empty or absolute paths, `..`, `\`, NUL, `:` (drive
/// letters and Windows alternate data streams), control characters, empty or `.`
/// components, names that Windows would change (trailing dots or spaces, device names),
/// and paths longer than [`MAX_PATH_CHARS`].
///
/// Skipped: anything named `.lodestone*` at any depth (Lodestone's own files), anything in
/// `macros/`, the protected files at the root (such as `eula.txt`), and scripts unless
/// `allow_scripts`.
pub fn check_pack_path(path: &str, is_dir: bool, allow_scripts: bool) -> Result<PathCheck, Error> {
    let trimmed = if is_dir {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    let reject = |why: &str| Err(unsafe_pack(format!("{why}: {path:?}")));
    if trimmed.is_empty() {
        return reject("empty path");
    }
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(unsafe_pack(format!(
            "a path is longer than {MAX_PATH_CHARS} characters"
        )));
    }
    if trimmed.starts_with('/') {
        return reject("absolute path");
    }
    if trimmed.contains(['\\', '\0', ':']) || trimmed.chars().any(|c| c.is_control()) {
        return reject("invalid characters in path");
    }
    if trimmed.contains("..") {
        return reject("path contains \"..\"");
    }
    let components: Vec<&str> = trimmed.split('/').collect();
    for component in &components {
        if component.is_empty() || *component == "." {
            return reject("empty path component");
        }
        if component.ends_with(['.', ' ']) {
            return reject("name ends with a dot or space");
        }
        if is_windows_device_name(component) {
            return reject("reserved file name");
        }
    }
    let relative: PathBuf = components.iter().collect();
    // anything else could resolve outside the instance folder
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return reject("path leaves the instance folder");
    }

    if components
        .iter()
        .any(|c| c.to_lowercase().starts_with(".lodestone"))
    {
        return Ok(PathCheck::Skip(
            "Lodestone's own files cannot be replaced".into(),
        ));
    }
    if is_in_macros_dir(&relative) {
        return Ok(PathCheck::Skip("macros cannot come from a modpack".into()));
    }
    if components.len() == 1
        && PROTECTED_ROOT_FILES.contains(&components[0].to_lowercase().as_str())
    {
        return Ok(PathCheck::Skip(
            "this file cannot come from a modpack".into(),
        ));
    }
    if !is_dir && !allow_scripts && is_script_path(&relative) {
        return Ok(PathCheck::Skip(
            "script file: only the owner can add scripts, while code execution is on".into(),
        ));
    }
    Ok(PathCheck::Write(relative))
}

/// Whether `value` is a plausible Minecraft or loader version, which is used in URLs.
fn is_valid_version(value: &str) -> bool {
    crate::modrinth::search::is_valid_filter_value(value)
}

fn open_pack(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, Error> {
    let file = std::fs::File::open(path).context("Failed to open the modpack")?;
    zip::ZipArchive::new(file).map_err(|e| invalid_pack(format!("it is not a zip file ({e})")))
}

/// Checks the zip limits: entry count and total uncompressed size.
fn check_zip_limits<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    max_entries: usize,
    max_bytes: u64,
) -> Result<(), Error> {
    if archive.len() > max_entries {
        return Err(unsafe_pack(format!("it has more than {max_entries} files")));
    }
    let mut total = 0_u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| invalid_pack(format!("a zip entry cannot be read ({e})")))?;
        total = total.saturating_add(entry.size());
        if total > max_bytes {
            return Err(unsafe_pack(format!(
                "its files are larger than {} uncompressed",
                crate::util::format_byte(max_bytes)
            )));
        }
    }
    Ok(())
}

fn read_index<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<Index, Error> {
    let entry = archive
        .by_name(INDEX_FILE)
        .map_err(|_| invalid_pack(format!("it has no {INDEX_FILE}")))?;
    if entry.size() > MAX_INDEX_BYTES {
        return Err(invalid_pack(format!("{INDEX_FILE} is too large")));
    }
    let mut bytes = Vec::new();
    entry
        .take(MAX_INDEX_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid_pack(format!("{INDEX_FILE} cannot be read ({e})")))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| invalid_pack(format!("{INDEX_FILE} is invalid ({e})")))
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

/// Checks the server files of the index: their paths, hosts, hashes and sizes.
fn check_files(
    index: &Index,
    allow_scripts: bool,
) -> Result<(Vec<PackDownload>, Vec<SkippedPath>), Error> {
    let mut downloads = Vec::new();
    let mut skipped = Vec::new();
    for file in index.files.iter().filter(|f| f.is_server_file()) {
        let path = match check_pack_path(&file.path, false, allow_scripts)? {
            PathCheck::Write(path) => path,
            PathCheck::Skip(reason) => {
                skipped.push(SkippedPath {
                    path: file.path.clone(),
                    reason,
                });
                continue;
            }
        };
        if file.file_size > MAX_PACK_FILE_BYTES {
            return Err(unsafe_pack(format!(
                "{} is larger than {}",
                file.path,
                crate::util::format_byte(MAX_PACK_FILE_BYTES)
            )));
        }
        let hash = |name: &str, len: usize| {
            file.hashes
                .get(name)
                .map(|h| h.to_lowercase())
                .filter(|h| h.len() == len && h.chars().all(|c| c.is_ascii_hexdigit()))
                .ok_or_else(|| invalid_pack(format!("{} has no valid {name} hash", file.path)))
        };
        let sha1 = hash("sha1", 40)?;
        let sha512 = hash("sha512", 128)?;
        let urls: Vec<url::Url> = file
            .downloads
            .iter()
            .filter_map(|u| url::Url::parse(u).ok())
            .filter(|u| {
                u.scheme() == "https"
                    && u.port().is_none()
                    && u.username().is_empty()
                    && u.host_str().is_some_and(|h| DOWNLOAD_HOSTS.contains(&h))
            })
            .collect();
        if urls.is_empty() {
            return Err(unsafe_pack(format!(
                "{} is not downloaded from an allowed host ({})",
                file.path,
                DOWNLOAD_HOSTS.join(", ")
            )));
        }
        downloads.push(PackDownload {
            path,
            urls,
            sha1,
            sha512,
            size: file.file_size,
        });
    }
    Ok((downloads, skipped))
}

/// Checks the override entries: their paths, and that each is a file, folder or link.
fn check_overrides<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    allow_scripts: bool,
) -> Result<Vec<SkippedPath>, Error> {
    let mut skipped = Vec::new();
    for_each_override(archive, allow_scripts, |_, item| {
        if let OverrideItem::Skip { path, reason } = item {
            skipped.push(SkippedPath { path, reason });
        }
        Ok(())
    })?;
    Ok(skipped)
}

enum OverrideItem<'a, R: Read> {
    Dir(PathBuf),
    File(PathBuf, zip::read::ZipFile<'a, R>),
    Skip { path: String, reason: String },
}

/// Calls `f` for each entry in the override folders, in the order they are applied.
fn for_each_override<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    allow_scripts: bool,
    mut f: impl FnMut(&str, OverrideItem<'_, R>) -> Result<(), Error>,
) -> Result<(), Error> {
    for prefix in OVERRIDE_DIRS {
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| invalid_pack(format!("a zip entry cannot be read ({e})")))?;
            let name = entry
                .name()
                .map_err(|e| invalid_pack(format!("a zip entry name cannot be read ({e})")))?
                .to_string();
            let Some(relative) = name.strip_prefix(prefix) else {
                continue;
            };
            if relative.is_empty() {
                continue;
            }
            let is_dir = entry.is_dir();
            let item = match check_pack_path(relative, is_dir, allow_scripts)? {
                PathCheck::Skip(reason) => OverrideItem::Skip {
                    path: relative.to_string(),
                    reason,
                },
                PathCheck::Write(_) if entry.is_symlink() => OverrideItem::Skip {
                    path: relative.to_string(),
                    reason: "symbolic links are not copied".into(),
                },
                PathCheck::Write(path) if is_dir => OverrideItem::Dir(path),
                PathCheck::Write(path) => OverrideItem::File(path, entry),
            };
            f(prefix, item)?;
        }
    }
    Ok(())
}

/// Reads and checks the pack at `path`. A pack that cannot be used for an instance is
/// returned with `unsupported_reason` set; a pack that is not a valid `.mrpack`, or is
/// unsafe, is an error.
pub fn read_pack(path: &Path, allow_scripts: bool) -> Result<Pack, Error> {
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
    let (downloads, mut skipped) = check_files(&index, allow_scripts)?;
    skipped.extend(check_overrides(&mut archive, allow_scripts)?);
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
    for_each_override(&mut archive, allow_scripts, |_, item| {
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
        Ok(())
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
