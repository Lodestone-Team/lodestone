//! A server's client modpack in a launcher instance: reading the `.mrpack` a core
//! builds, putting its mods into the instance's `mods/`, and remembering which files
//! came from it, so "keep mods in step" can compare them with the server's current
//! pack.
//!
//! This is a minimal import made for the core's client packs, which hold only mods:
//! index files from Modrinth's CDN and uploaded jars in `overrides/mods/`. Anything
//! else in a pack is left out. It is kept apart from the rest of the launcher so it
//! can move onto the general `.mrpack` import (milestone 5b) once that exists.
//!
//! The link between an instance and its server is `<instance>/lodestone_server.json`.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read as _},
    path::{Path, PathBuf},
};

use lodestone_minecraft::{
    hash,
    mrpack::{self, PathCheck, PathPolicy, Side},
};
use serde::{Deserialize, Serialize};

use crate::{
    content::{Modrinth, is_plain_name},
    game::download::{
        CancelToken, DownloadError, DownloadItem, Downloader, Progress, ProgressSnapshot, sha1_hex,
    },
    storage::{self, Document, Migration},
};

/// The link file in an instance folder.
pub const LINK_FILE: &str = "lodestone_server.json";
/// An uploaded jar in a pack may be at most this large.
pub const MAX_INCLUDED_JAR_BYTES: u64 = 128 * 1024 * 1024;
/// A pack may have at most this many mods.
pub const MAX_PACK_MODS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackError {
    #[error("The server's mod pack isn't valid: {0}")]
    Invalid(String),
    #[error("Couldn't download {name}: {reason}")]
    Download { name: String, reason: String },
    #[error("Couldn't write {path}: {reason}")]
    Io { path: String, reason: String },
    #[error("Cancelled")]
    Cancelled,
}

fn io_error(path: &Path) -> impl Fn(std::io::Error) -> PackError + '_ {
    move |err| PackError::Io {
        path: path.display().to_string(),
        reason: err.to_string(),
    }
}

/// One mod of a server's pack, by its file name in `mods/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackMod {
    pub name: String,
    /// Lowercase hex.
    pub sha1: String,
    pub source: PackSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackSource {
    /// A Modrinth file, checked against both hashes after the download.
    Download {
        url: String,
        sha512: String,
        size: u64,
    },
    /// An uploaded jar inside the pack.
    Included(Vec<u8>),
}

/// A server's client pack, read and checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientPack {
    pub name: String,
    pub game_version: Option<String>,
    pub fabric_loader: Option<String>,
    pub mods: Vec<PackMod>,
    /// Paths of the pack that were left out, and why.
    pub skipped: Vec<(String, String)>,
}

impl ClientPack {
    fn files(&self) -> BTreeMap<&str, &str> {
        self.mods
            .iter()
            .map(|m| (m.name.as_str(), m.sha1.as_str()))
            .collect()
    }
}

/// Takes only plain `mods/<name>.jar` files from a pack.
struct ModsOnly;

impl PathPolicy for ModsOnly {
    fn skip_reason(&self, _: &Path, components: &[&str], is_dir: bool) -> Option<String> {
        let is_mod = !is_dir
            && components.len() == 2
            && components[0] == "mods"
            && components[1].to_ascii_lowercase().ends_with(".jar")
            && is_plain_name(components[1]);
        (!is_mod).then(|| "Only mods are taken from a server's pack".to_string())
    }
}

fn invalid(err: impl std::fmt::Display) -> PackError {
    PackError::Invalid(err.to_string())
}

/// Reads and checks a client pack: the index (format 1, Minecraft), its files for the
/// client (Modrinth's CDN, both hashes), and the jars in `overrides/mods/` and
/// `client-overrides/mods/`. Paths go through the shared `.mrpack` checks.
pub fn read_pack(bytes: &[u8]) -> Result<ClientPack, PackError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|err| invalid(format!("it isn't a zip file ({err})")))?;
    if zip.len() > mrpack::MAX_ENTRIES {
        return Err(invalid("it has too many files"));
    }
    let index: mrpack::Index = {
        let entry = zip
            .by_name(mrpack::INDEX_FILE)
            .map_err(|_| invalid(format!("it has no {}", mrpack::INDEX_FILE)))?;
        if entry.size() > mrpack::MAX_INDEX_BYTES {
            return Err(invalid(format!("{} is too large", mrpack::INDEX_FILE)));
        }
        let mut text = Vec::new();
        entry
            .take(mrpack::MAX_INDEX_BYTES)
            .read_to_end(&mut text)
            .map_err(invalid)?;
        serde_json::from_slice(&text).map_err(invalid)?
    };
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(invalid("it isn't a Minecraft modpack of format 1"));
    }
    let (downloads, skipped) =
        mrpack::check_files(&index, Side::Client, &ModsOnly).map_err(invalid)?;
    let mut skipped: Vec<(String, String)> =
        skipped.into_iter().map(|s| (s.path, s.reason)).collect();
    let mut mods: BTreeMap<String, PackMod> = BTreeMap::new();
    for download in downloads {
        let name = download
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("a file has no name"))?
            .to_string();
        let url = download
            .urls
            .iter()
            .find(|u| u.host_str() == Some("cdn.modrinth.com"))
            .ok_or_else(|| invalid(format!("{name} isn't on Modrinth's CDN")))?
            .to_string();
        mods.insert(
            name.clone(),
            PackMod {
                name,
                sha1: download.sha1,
                source: PackSource::Download {
                    url,
                    sha512: download.sha512,
                    size: download.size,
                },
            },
        );
    }
    // overrides, then client overrides, which win
    for prefix in Side::Client.override_dirs() {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(invalid)?;
            let Some(rest) = entry.name().strip_prefix(prefix).map(str::to_string) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            let is_link = entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000);
            if entry.is_dir() {
                continue;
            }
            let check = mrpack::check_pack_path(&rest, false, &ModsOnly).map_err(invalid)?;
            let name = match check {
                PathCheck::Write(path) if !is_link => path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .ok_or_else(|| invalid("a file has no name"))?,
                PathCheck::Write(_) => {
                    skipped.push((rest, "Links are left out".into()));
                    continue;
                }
                PathCheck::Skip(reason) => {
                    skipped.push((rest, reason));
                    continue;
                }
            };
            if entry.size() > MAX_INCLUDED_JAR_BYTES {
                return Err(invalid(format!("{name} is too large")));
            }
            let mut data = Vec::new();
            (&mut entry)
                .take(MAX_INCLUDED_JAR_BYTES + 1)
                .read_to_end(&mut data)
                .map_err(invalid)?;
            if data.len() as u64 > MAX_INCLUDED_JAR_BYTES {
                return Err(invalid(format!("{name} is too large")));
            }
            mods.insert(
                name.clone(),
                PackMod {
                    name,
                    sha1: sha1_hex(&data),
                    source: PackSource::Included(data),
                },
            );
        }
    }
    if mods.len() > MAX_PACK_MODS {
        return Err(invalid("it has too many mods"));
    }
    Ok(ClientPack {
        name: index.name,
        game_version: index.dependencies.get("minecraft").cloned(),
        fabric_loader: index.dependencies.get("fabric-loader").cloned(),
        mods: mods.into_values().collect(),
        skipped,
    })
}

/// A mod the instance got from its server's pack.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedFile {
    /// The file name in `mods/`.
    pub name: String,
    pub sha1: String,
}

/// Which server an instance was made for, and what its pack put into `mods/`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerLink {
    pub core_uuid: String,
    pub core_url: String,
    pub server_uuid: String,
    pub server_name: String,
    /// The ETag of the pack the files came from.
    #[serde(default)]
    pub pack_etag: Option<String>,
    #[serde(default)]
    pub files: Vec<LinkedFile>,
}

impl Document for ServerLink {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

pub fn link_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(LINK_FILE)
}

/// The instance's server link, if it has a readable one.
pub fn load_link(instance_dir: &Path) -> Option<ServerLink> {
    let path = link_path(instance_dir);
    if !path.is_file() {
        return None;
    }
    storage::load::<ServerLink>(&path)
        .ok()
        .map(|loaded| loaded.document)
        .filter(|link| !link.core_uuid.is_empty() && !link.server_uuid.is_empty())
}

pub fn save_link(instance_dir: &Path, link: &ServerLink) -> Result<(), PackError> {
    let path = link_path(instance_dir);
    storage::save(&path, link).map_err(|err| PackError::Io {
        path: path.display().to_string(),
        reason: err.to_string(),
    })
}

/// Every instance under `instances_root` with a server link: `(instance id, link)`.
pub fn linked_instances(instances_root: &Path) -> Vec<(String, ServerLink)> {
    let Ok(entries) = fs::read_dir(instances_root) else {
        return Vec::new();
    };
    let mut linked: Vec<(String, ServerLink)> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_string();
            if id.starts_with('.') {
                return None;
            }
            Some((id, load_link(&entry.path())?))
        })
        .collect();
    linked.sort_by(|a, b| a.0.cmp(&b.0));
    linked
}

/// What updating an instance to a pack changes, by file name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackChanges {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}

impl PackChanges {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }

    /// "2 added, 1 updated, 1 removed".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (count, word) in [
            (self.added.len(), "added"),
            (self.updated.len(), "updated"),
            (self.removed.len(), "removed"),
        ] {
            if count > 0 {
                parts.push(format!("{count} {word}"));
            }
        }
        if parts.is_empty() {
            "no changes".into()
        } else {
            parts.join(", ")
        }
    }
}

/// The changes from what `link` recorded to `pack`. Compared by file name and SHA-1.
pub fn changes(link: Option<&ServerLink>, pack: &ClientPack) -> PackChanges {
    let old: BTreeMap<&str, &str> = link
        .map(|l| {
            l.files
                .iter()
                .map(|f| (f.name.as_str(), f.sha1.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let new = pack.files();
    let mut changes = PackChanges::default();
    for (name, sha1) in &new {
        match old.get(name) {
            None => changes.added.push(name.to_string()),
            Some(old_sha1) if !old_sha1.eq_ignore_ascii_case(sha1) => {
                changes.updated.push(name.to_string())
            }
            Some(_) => {}
        }
    }
    for name in old.keys() {
        if !new.contains_key(name) {
            changes.removed.push(name.to_string());
        }
    }
    changes
}

/// The linked files that are missing from `mods/` or were changed since.
pub fn changed_on_disk(link: &ServerLink, mods_dir: &Path) -> Vec<String> {
    link.files
        .iter()
        .filter(|f| {
            !hash::sha1_of(&mods_dir.join(&f.name)).is_ok_and(|h| h.eq_ignore_ascii_case(&f.sha1))
        })
        .map(|f| f.name.clone())
        .collect()
}

/// Puts the pack's mods into `<game_dir>/mods`:
///
/// - Modrinth files are downloaded from the CDN (through `modrinth`'s URL check, which
///   only allows `cdn.modrinth.com`) with the `downloader`, which checks the SHA-1 and
///   size, then checked against their SHA-512; uploaded jars come from the pack;
/// - everything is staged first, so a failed download changes nothing;
/// - files the previous pack put there (same name and SHA-1 as recorded in `previous`)
///   and that the new pack no longer has are removed; the user's own files are never
///   touched, except one with the same name as a file of the pack, which is replaced.
///
/// Returns the files the instance now has from the pack.
pub async fn apply(
    pack: &ClientPack,
    game_dir: &Path,
    previous: Option<&ServerLink>,
    modrinth: &Modrinth,
    downloader: &Downloader,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Vec<LinkedFile>, PackError> {
    let mods_dir = game_dir.join("mods");
    fs::create_dir_all(&mods_dir).map_err(io_error(&mods_dir))?;
    let staging = game_dir.join(".lodestone-server-pack");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(io_error(&staging))?;
    let result = stage(
        pack, &mods_dir, &staging, modrinth, downloader, progress, cancel,
    )
    .await;
    let result = match result {
        Ok(staged) => commit(pack, &mods_dir, &staging, &staged, previous),
        Err(err) => Err(err),
    };
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Downloads or writes every mod that isn't in `mods/` already. Returns the staged names.
async fn stage(
    pack: &ClientPack,
    mods_dir: &Path,
    staging: &Path,
    modrinth: &Modrinth,
    downloader: &Downloader,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<BTreeSet<String>, PackError> {
    let needed: Vec<&PackMod> = pack
        .mods
        .iter()
        .filter(|m| {
            !hash::sha1_of(&mods_dir.join(&m.name)).is_ok_and(|h| h.eq_ignore_ascii_case(&m.sha1))
        })
        .collect();
    let total_bytes = needed
        .iter()
        .map(|m| match &m.source {
            PackSource::Download { size, .. } => *size,
            PackSource::Included(data) => data.len() as u64,
        })
        .sum();
    progress.set(ProgressSnapshot {
        files_total: needed.len() as u64,
        files_done: 0,
        bytes_total: total_bytes,
        bytes_done: 0,
    });
    let mut staged = BTreeSet::new();
    let mut done = ProgressSnapshot {
        files_total: needed.len() as u64,
        bytes_total: total_bytes,
        ..ProgressSnapshot::default()
    };
    for m in needed {
        if cancel.is_cancelled() {
            return Err(PackError::Cancelled);
        }
        let dest = staging.join(&m.name);
        match &m.source {
            PackSource::Download { url, sha512, size } => {
                let url = modrinth.cdn_url(url).map_err(|err| PackError::Download {
                    name: m.name.clone(),
                    reason: err.to_string(),
                })?;
                let item = DownloadItem {
                    url,
                    dest: dest.clone(),
                    sha1: m.sha1.clone(),
                    size: Some(*size),
                    executable: false,
                };
                downloader
                    .fetch(&item, progress, cancel)
                    .await
                    .map_err(|err| match err {
                        DownloadError::Cancelled => PackError::Cancelled,
                        err => PackError::Download {
                            name: m.name.clone(),
                            reason: err.to_string(),
                        },
                    })?;
                let actual = hash::sha512_of(&dest).map_err(io_error(&dest))?;
                if !actual.eq_ignore_ascii_case(sha512) {
                    return Err(PackError::Download {
                        name: m.name.clone(),
                        reason: "the file is damaged: its SHA-512 hash doesn't match".into(),
                    });
                }
                done.bytes_done += size;
            }
            PackSource::Included(data) => {
                storage::write_atomic(&dest, data).map_err(io_error(&dest))?;
                done.bytes_done += data.len() as u64;
            }
        }
        done.files_done += 1;
        progress.set(done);
        staged.insert(m.name.clone());
    }
    Ok(staged)
}

fn commit(
    pack: &ClientPack,
    mods_dir: &Path,
    staging: &Path,
    staged: &BTreeSet<String>,
    previous: Option<&ServerLink>,
) -> Result<Vec<LinkedFile>, PackError> {
    let new = pack.files();
    if let Some(previous) = previous {
        for old in &previous.files {
            if new.contains_key(old.name.as_str()) {
                continue;
            }
            let path = mods_dir.join(&old.name);
            // only a file the pack put there, unchanged since
            if hash::sha1_of(&path).is_ok_and(|h| h.eq_ignore_ascii_case(&old.sha1)) {
                fs::remove_file(&path).map_err(io_error(&path))?;
            }
        }
    }
    for name in staged {
        let from = staging.join(name);
        let to = mods_dir.join(name);
        if fs::symlink_metadata(&to).is_ok() {
            fs::remove_file(&to).map_err(io_error(&to))?;
        }
        fs::rename(&from, &to).map_err(io_error(&to))?;
    }
    Ok(pack
        .mods
        .iter()
        .map(|m| LinkedFile {
            name: m.name.clone(),
            sha1: m.sha1.clone(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    /// A pack with the given index files and override entries.
    pub(crate) fn make_pack(files: serde_json::Value, overrides: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let options = zip::write::FileOptions::default();
            zip.start_file(mrpack::INDEX_FILE, options).unwrap();
            let index = serde_json::json!({
                "formatVersion": 1, "game": "minecraft", "versionId": "SMP (Lodestone server)",
                "name": "SMP client mods",
                "dependencies": { "minecraft": "1.20.1", "fabric-loader": "0.16.14" },
                "files": files,
            });
            zip.write_all(index.to_string().as_bytes()).unwrap();
            for (path, data) in overrides {
                zip.start_file(*path, options).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        out.into_inner()
    }

    fn index_file(path: &str, data: &[u8], host: &str) -> serde_json::Value {
        use sha2::Digest as _;
        serde_json::json!({
            "path": path,
            "hashes": { "sha1": sha1_hex(data), "sha512": hex(&sha2::Sha512::digest(data)) },
            "env": { "client": "required", "server": "unsupported" },
            "downloads": [format!("https://{host}/data/abc/versions/v1/{}", path.trim_start_matches("mods/"))],
            "fileSize": data.len(),
        })
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn reads_index_mods_and_included_jars() {
        let bytes = make_pack(
            serde_json::json!([
                index_file("mods/sodium.jar", b"sodium", "cdn.modrinth.com"),
                index_file("config/x.toml", b"cfg", "cdn.modrinth.com"),
            ]),
            &[
                ("overrides/mods/uploaded.jar", b"uploaded"),
                ("overrides/config/secret.txt", b"nope"),
                ("client-overrides/mods/uploaded.jar", b"client wins"),
            ],
        );
        let pack = read_pack(&bytes).unwrap();
        assert_eq!(pack.game_version.as_deref(), Some("1.20.1"));
        assert_eq!(pack.fabric_loader.as_deref(), Some("0.16.14"));
        let names: Vec<&str> = pack.mods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["sodium.jar", "uploaded.jar"]);
        assert_eq!(
            pack.mods[1].source,
            PackSource::Included(b"client wins".to_vec())
        );
        assert_eq!(pack.mods[1].sha1, sha1_hex(b"client wins"));
        let skipped: Vec<&str> = pack.skipped.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(skipped, ["config/x.toml", "config/secret.txt"]);
    }

    #[test]
    fn refuses_unsafe_packs() {
        // a path that leaves the folder fails the whole pack
        let bad = make_pack(
            serde_json::json!([]),
            &[("overrides/mods/../../evil.jar", b"x")],
        );
        assert!(read_pack(&bad).is_err());
        // a download from another host
        let other_host = make_pack(
            serde_json::json!([index_file("mods/a.jar", b"a", "example.com")]),
            &[],
        );
        assert!(read_pack(&other_host).is_err());
        // on GitHub (allowed by the format) but not on Modrinth's CDN
        let github = make_pack(
            serde_json::json!([index_file("mods/a.jar", b"a", "github.com")]),
            &[],
        );
        assert!(matches!(read_pack(&github), Err(PackError::Invalid(m)) if m.contains("CDN")));
        assert!(read_pack(b"not a zip").is_err());
    }

    #[test]
    fn compares_by_name_and_hash() {
        let pack = read_pack(&make_pack(
            serde_json::json!([index_file("mods/a.jar", b"a2", "cdn.modrinth.com")]),
            &[("overrides/mods/b.jar", b"b")],
        ))
        .unwrap();
        let link = ServerLink {
            core_uuid: "c".into(),
            server_uuid: "s".into(),
            files: vec![
                LinkedFile {
                    name: "a.jar".into(),
                    sha1: sha1_hex(b"a1"),
                },
                LinkedFile {
                    name: "old.jar".into(),
                    sha1: sha1_hex(b"old"),
                },
            ],
            ..ServerLink::default()
        };
        let changes = changes(Some(&link), &pack);
        assert_eq!(changes.added, ["b.jar"]);
        assert_eq!(changes.updated, ["a.jar"]);
        assert_eq!(changes.removed, ["old.jar"]);
        assert_eq!(changes.summary(), "1 added, 1 updated, 1 removed");
        assert!(changes_in_step(&pack));
    }

    fn changes_in_step(pack: &ClientPack) -> bool {
        let link = ServerLink {
            files: pack
                .mods
                .iter()
                .map(|m| LinkedFile {
                    name: m.name.clone(),
                    sha1: m.sha1.clone(),
                })
                .collect(),
            ..ServerLink::default()
        };
        changes(Some(&link), pack).is_empty()
    }

    #[test]
    fn links_are_found_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let instance = dir.path().join("smp");
        fs::create_dir_all(&instance).unwrap();
        fs::create_dir_all(dir.path().join("plain")).unwrap();
        assert!(load_link(&instance).is_none());
        let link = ServerLink {
            core_uuid: "c".into(),
            core_url: "http://localhost:16662".into(),
            server_uuid: "s".into(),
            server_name: "SMP".into(),
            pack_etag: Some("\"e\"".into()),
            files: vec![LinkedFile {
                name: "a.jar".into(),
                sha1: "00".into(),
            }],
        };
        save_link(&instance, &link).unwrap();
        assert_eq!(load_link(&instance), Some(link.clone()));
        assert_eq!(
            linked_instances(dir.path()),
            vec![("smp".to_string(), link)]
        );
        fs::write(link_path(&instance), "{broken").unwrap();
        assert!(linked_instances(dir.path()).is_empty());
    }
}
