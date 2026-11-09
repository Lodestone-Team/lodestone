//! Modrinth modpacks (`.mrpack`): the index, and the checks that make a pack safe to put
//! into a folder.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.
//! Every path in a pack is checked here; the zip crate's own `extract` is never used.
//! Reading the zip itself needs the `zip` feature. [`write`] makes packs.

pub mod write;

use std::collections::HashMap;
#[cfg(feature = "zip")]
use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::Error;

/// The index file at the root of a pack.
pub const INDEX_FILE: &str = "modrinth.index.json";
/// The index may be at most this large.
pub const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;
/// A pack may have at most this many zip entries.
pub const MAX_ENTRIES: usize = 20_000;
/// The files in a pack may be at most this large together, uncompressed.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// A `.mrpack` file, and each file it downloads, may be at most this large.
pub const MAX_PACK_FILE_BYTES: u64 = 1024 * 1024 * 1024;
/// A path in a pack may be at most this many characters long.
pub const MAX_PATH_CHARS: usize = 512;
/// The hosts files may be downloaded from (the allow-list of the Modrinth pack format).
pub const DOWNLOAD_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

/// The side a pack is installed for: a dedicated server or a game client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Client,
    Server,
}

impl Side {
    /// The folders of a pack whose contents are copied, in order: later ones win. The
    /// other side's folder is skipped.
    pub fn override_dirs(self) -> [&'static str; 2] {
        match self {
            Side::Client => ["overrides/", "client-overrides/"],
            Side::Server => ["overrides/", "server-overrides/"],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEnv {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

/// A file of the index, downloaded into the instance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFile {
    pub path: String,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<IndexEnv>,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: u64,
}

impl IndexFile {
    /// Files without `env`, or whose server side is not `unsupported`, are server files.
    pub fn is_server_file(&self) -> bool {
        self.is_for(Side::Server)
    }

    /// Files without `env`, or whose value for `side` is not `unsupported`, are installed
    /// on that side.
    pub fn is_for(&self, side: Side) -> bool {
        self.env
            .as_ref()
            .and_then(|e| match side {
                Side::Client => e.client.as_deref(),
                Side::Server => e.server.as_deref(),
            })
            .is_none_or(|s| s != "unsupported")
    }
}

/// `modrinth.index.json`, as read from a pack.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub format_version: u32,
    pub game: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<IndexFile>,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
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

/// The error for a pack that is not a valid `.mrpack`.
pub fn invalid_pack(message: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("Not a valid Modrinth modpack: {message}"))
}

/// The error for a pack that would write outside its folder, or is too large.
pub fn unsafe_pack(message: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("The modpack is unsafe: {message}"))
}

/// What to do with a path from a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCheck {
    Write(PathBuf),
    Skip(String),
}

/// Which safe paths of a pack are left out, for example the files the program that
/// installs the pack keeps for itself.
pub trait PathPolicy {
    /// Why the safe path `relative` (split into `components`) is not written, if it is not.
    fn skip_reason(&self, relative: &Path, components: &[&str], is_dir: bool) -> Option<String>;
}

/// A [`PathPolicy`] that writes every safe path.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteAll;

impl PathPolicy for WriteAll {
    fn skip_reason(&self, _: &Path, _: &[&str], _: bool) -> Option<String> {
        None
    }
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
/// Skipped: what `policy` skips.
pub fn check_pack_path(
    path: &str,
    is_dir: bool,
    policy: &dyn PathPolicy,
) -> Result<PathCheck, Error> {
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

    Ok(match policy.skip_reason(&relative, &components, is_dir) {
        Some(reason) => PathCheck::Skip(reason),
        None => PathCheck::Write(relative),
    })
}

/// Opens the pack at `path`. A file that is not a zip is [`invalid_pack`].
#[cfg(feature = "zip")]
pub fn open_pack(path: &Path) -> Result<zip::ZipArchive<std::io::BufReader<std::fs::File>>, Error> {
    let file = std::fs::File::open(path).map_err(Error::io("Failed to open the modpack"))?;
    zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| invalid_pack(format!("it is not a zip file ({e})")))
}

/// Checks the zip limits: entry count and total uncompressed size.
#[cfg(feature = "zip")]
pub fn check_zip_limits<R: Read + Seek>(
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

/// Reads [`INDEX_FILE`].
#[cfg(feature = "zip")]
pub fn read_index<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<Index, Error> {
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

/// Checks the files of the index that are for `side`: their paths, hosts, hashes and sizes.
pub fn check_files(
    index: &Index,
    side: Side,
    policy: &dyn PathPolicy,
) -> Result<(Vec<PackDownload>, Vec<SkippedPath>), Error> {
    let mut downloads = Vec::new();
    let mut skipped = Vec::new();
    for file in index.files.iter().filter(|f| f.is_for(side)) {
        let path = match check_pack_path(&file.path, false, policy)? {
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

/// Checks the override entries for `side`: their paths, and that each is a file, folder or
/// link.
#[cfg(feature = "zip")]
pub fn check_overrides<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    side: Side,
    policy: &dyn PathPolicy,
) -> Result<Vec<SkippedPath>, Error> {
    let mut skipped = Vec::new();
    for_each_override(archive, side, policy, |_, item| {
        if let OverrideItem::Skip { path, reason } = item {
            skipped.push(SkippedPath { path, reason });
        }
        Ok::<_, Error>(())
    })?;
    Ok(skipped)
}

/// An entry of an override folder.
#[cfg(feature = "zip")]
pub enum OverrideItem<'a, R: Read> {
    Dir(PathBuf),
    File(PathBuf, zip::read::ZipFile<'a, R>),
    Skip { path: String, reason: String },
}

/// Calls `f` with the folder and the item of each entry in the override folders of
/// `side`, in the order they are applied.
#[cfg(feature = "zip")]
pub fn for_each_override<R: Read + Seek, E: From<Error>>(
    archive: &mut zip::ZipArchive<R>,
    side: Side,
    policy: &dyn PathPolicy,
    mut f: impl FnMut(&str, OverrideItem<'_, R>) -> Result<(), E>,
) -> Result<(), E> {
    for prefix in side.override_dirs() {
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
            let item = match check_pack_path(relative, is_dir, policy)? {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_safety_rejects() {
        for path in [
            "",
            "/etc/passwd",
            "../escape.txt",
            "mods/../../escape.txt",
            "a/..",
            "mods\\a.jar",
            "a\0b",
            "C:/Windows/x",
            "C:x",
            "//server/share/x",
            "a//b",
            "./a",
            "a/./b",
            "a/b.",
            "a/b ",
            "con.txt",
            "mods/nul",
            "mods/COM1.jar",
            "line\nbreak",
        ] {
            let err = check_pack_path(path, false, &WriteAll).unwrap_err();
            assert!(
                err.to_string().starts_with("The modpack is unsafe"),
                "{path:?}"
            );
        }
        let long = format!("config/{}", "a".repeat(MAX_PATH_CHARS));
        assert!(check_pack_path(&long, false, &WriteAll).is_err());
        let just_short_enough = "a".repeat(MAX_PATH_CHARS);
        assert!(check_pack_path(&just_short_enough, false, &WriteAll).is_ok());
    }

    #[cfg(feature = "zip")]
    #[test]
    fn zip_limits() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pack.mrpack");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, contents) in [
            (INDEX_FILE, r#"{"formatVersion": 1, "game": "minecraft"}"#),
            ("overrides/a.txt", "0123456789"),
            ("overrides/b.txt", "0123456789"),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();

        let mut archive = open_pack(&path).unwrap();
        assert!(check_zip_limits(&mut archive, 3, 1000).is_ok());
        let err = check_zip_limits(&mut archive, 2, 1000).unwrap_err();
        assert!(err.to_string().contains("more than 2 files"));
        let err = check_zip_limits(&mut archive, 10, 15).unwrap_err();
        assert!(err.to_string().contains("uncompressed"));
    }
}
