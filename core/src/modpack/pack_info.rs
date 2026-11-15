//! The pack a server is built on, kept with its images as `images/<uuid>/pack.json`
//! ([`crate::images::base`]).
//!
//! It is the one description of the server's pack:
//!
//! - the recorder gives content images their `base` from it (`modrinth_pack`, with the
//!   SHA-512 of the `.mrpack`), a `modrinth` or `base` source for each file that still has
//!   the pack's bytes, and the pack's `env` for each file;
//! - an update compares the server against the files the pack wrote ([`record_of_base`]),
//!   to tell the pack's unchanged files from the user's changes.
//!
//! It is written when a pack is installed, updated or restored, after the files are placed
//! and before the content image is recorded. It lives outside the server folder, so nobody
//! who can only write the server's files can edit what an update removes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use lodestone_image::{Base, Env, EnvValue};
use lodestone_minecraft::mrpack::{for_each_override, open_pack, read_index, OverrideItem, Side};
use tracing::warn;

use super::update::{PackFile, PackRecord, RecordedFile};
use super::{InstancePathPolicy, ModpackOrigin};
use crate::error::Error;
use crate::images::base::{BaseFile, BaseInfo};
use crate::images::ImageService;
use crate::types::InstanceUuid;

/// The SHA-512 of the `.mrpack` at `path`, lowercase. Blocking.
pub fn pack_sha512(path: &Path) -> Option<String> {
    match lodestone_minecraft::hash::sha512_of(path) {
        Ok(hash) => Some(hash.to_ascii_lowercase()),
        Err(e) => {
            warn!("Could not read the modpack {}: {e}", path.display());
            None
        }
    }
}

/// What the pack at `path` says about each file it writes into a server: an index file's
/// `env` as written, an override's from its folder. `overrides/` is for both sides,
/// `server-overrides/` for the server only (it replaces an `overrides/` file of the same
/// path there), and an `overrides/` file that `client-overrides/` replaces on clients isn't
/// what clients get, so it is unsupported on their side. Blocking.
fn pack_envs(path: &Path) -> Result<HashMap<String, Env>, Error> {
    let mut archive = open_pack(path)?;
    let index = read_index(&mut archive)?;
    let mut envs = HashMap::new();
    for file in &index.files {
        let side = |value: Option<&String>| {
            value
                .and_then(|v| EnvValue::parse_loose(v))
                .unwrap_or(EnvValue::Unknown)
        };
        let env = match &file.env {
            Some(env) => Env::new(side(env.client.as_ref()), side(env.server.as_ref())),
            None => Env::UNKNOWN,
        };
        envs.insert(file.path.clone(), env);
    }
    // Which override paths clients get from their own folder. Only compared with the
    // checked server paths below, so the names needn't be checked themselves.
    let client_only: HashSet<String> = archive
        .file_names()
        .filter_map(|name| name.ok())
        .filter_map(|name| name.strip_prefix("client-overrides/").map(str::to_string))
        .filter(|name| !name.is_empty() && !name.ends_with('/'))
        .collect();
    for_each_override(
        &mut archive,
        Side::Server,
        &InstancePathPolicy {
            allow_scripts: true,
        },
        |prefix, item| {
            if let OverrideItem::File(path, _) = item {
                let path = super::slash_path(&path);
                let client = if prefix == "server-overrides/" || client_only.contains(&path) {
                    EnvValue::Unsupported
                } else {
                    EnvValue::Required
                };
                envs.insert(path, Env::new(client, EnvValue::Required));
            }
            Ok::<_, Error>(())
        },
    )?;
    Ok(envs)
}

/// The Modrinth CDN URL of a download, if it has one.
fn cdn_url(file: &PackFile) -> Option<String> {
    file.download.as_ref()?.urls.iter().find_map(|url| {
        (url.scheme() == "https" && url.host_str() == Some(crate::modrinth::CDN_HOST))
            .then(|| url.to_string())
    })
}

/// The Modrinth project of a `https://cdn.modrinth.com/data/<project>/versions/...` URL,
/// read the way content images read it ([`lodestone_image::Source::modrinth_from_url`]).
pub(super) fn project_of_url(url: &str) -> Option<String> {
    match lodestone_image::Source::modrinth_from_url(url)? {
        lodestone_image::Source::Modrinth { project, .. } => Some(project),
        _ => None,
    }
}

/// `pack.json` for a server whose pack files are now `record` (by disk path), from the pack
/// `origin`:
///
/// - the base is the pack (`modrinth_pack`) when it is on Modrinth and its `.mrpack` is at
///   `pack_path` (hashed here), else none;
/// - each file has its SHA-1 and, when it is one of `files`' downloads, its SHA-512, size
///   and CDN URL (an override has none of these, which is how [`record_of_base`] tells
///   them apart), and the `env` the pack at `pack_path` gives it (unknown without the
///   pack). A pack mod disabled in the Mods tab is found under its enabled name.
///
/// Reads the pack, so call it on a blocking thread.
pub fn base_info(
    origin: &ModpackOrigin,
    pack_path: Option<&Path>,
    record: &PackRecord,
    files: &BTreeMap<String, PackFile>,
) -> BaseInfo {
    let on_modrinth = !origin.project_id.is_empty() && !origin.version_id.is_empty();
    let base = pack_path
        .filter(|_| on_modrinth)
        .and_then(pack_sha512)
        .map(|sha512| Base::ModrinthPack {
            project: origin.project_id.clone(),
            version: origin.version_id.clone(),
            sha512,
        });
    let envs = match pack_path {
        Some(path) => pack_envs(path).unwrap_or_else(|e| {
            warn!("Could not read which sides the modpack's files are for: {e}");
            HashMap::new()
        }),
        None => HashMap::new(),
    };
    let mut info = BaseInfo {
        base,
        files: BTreeMap::new(),
    };
    for (path, recorded) in &record.files {
        let download = files
            .get(path)
            .filter(|_| !recorded.from_overrides)
            .and_then(|file| Some((file, file.download.as_ref()?)));
        let base_file = BaseFile {
            sha1: recorded.sha1.to_ascii_lowercase(),
            sha256: None,
            size: download.map(|(_, d)| d.size),
            env: envs.get(path).copied(),
            modrinth_url: download.and_then(|(file, _)| cdn_url(file)),
            sha512: download.map(|(_, d)| d.sha512.to_ascii_lowercase()),
        };
        info.files.insert(path.clone(), base_file);
    }
    info
}

/// The files the pack wrote, as an update compares them, from `pack.json`. A file without
/// a SHA-512 is one of the pack's overrides; a download's Modrinth project comes from its
/// CDN URL.
pub fn record_of_base(info: &BaseInfo) -> PackRecord {
    let files = info
        .files
        .iter()
        .map(|(path, file)| {
            (
                path.clone(),
                RecordedFile {
                    sha1: file.sha1.clone(),
                    from_overrides: file.sha512.is_none(),
                    project_id: file.modrinth_url.as_deref().and_then(project_of_url),
                },
            )
        })
        .collect();
    PackRecord { files }
}

/// The server's `pack.json`, if it has one. Blocking.
pub fn load(images: &ImageService, uuid: &InstanceUuid) -> Option<BaseInfo> {
    if !ImageService::accepts(uuid) {
        return None;
    }
    crate::images::base::load_base(images.root(), uuid)
}

/// Replaces the server's `pack.json`. A failure is logged: it only loses the base and
/// `env` of the images to come, and makes the next update leave the pack's changed files
/// as the user's.
pub async fn save(images: &Arc<ImageService>, uuid: &InstanceUuid, info: BaseInfo) {
    if !ImageService::accepts(uuid) {
        return;
    }
    let root = images.root().to_path_buf();
    let id = uuid.clone();
    let saved =
        tokio::task::spawn_blocking(move || crate::images::base::save_base(&root, &id, &info))
            .await;
    match saved {
        Ok(Ok(())) => {}
        Ok(Err(e)) => warn!("Could not write the modpack files of {uuid}: {}", e.source),
        Err(e) => warn!("Could not write the modpack files of {uuid}: {e}"),
    }
}

/// After a restore that put back an image's content, which was built on `base`: makes the
/// server's `pack.json` name that base, without the pack's file list (an image doesn't keep
/// it), so the next update falls back to the files of the origin's pack version. A
/// `pack.json` with that base already is left as it is. Call it inside the restore's
/// content operation, before its image is recorded.
pub async fn follow_base(images: &Arc<ImageService>, uuid: &InstanceUuid, base: Option<Base>) {
    if !ImageService::accepts(uuid) {
        return;
    }
    let current = {
        let (images, uuid) = (images.clone(), uuid.clone());
        tokio::task::spawn_blocking(move || load(&images, &uuid))
            .await
            .ok()
            .flatten()
    };
    if current.as_ref().and_then(|info| info.base.as_ref()) == base.as_ref() {
        return;
    }
    let info = BaseInfo {
        base,
        files: BTreeMap::new(),
    };
    save(images, uuid, info).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(sha1: &str, from_overrides: bool, project: Option<&str>) -> RecordedFile {
        RecordedFile {
            sha1: sha1.into(),
            from_overrides,
            project_id: project.map(str::to_string),
        }
    }

    #[test]
    fn the_record_survives_pack_json() {
        let download = |path: &str, sha1: &str| PackFile {
            sha1: sha1.into(),
            download: Some(super::super::PackDownload {
                path: path.into(),
                urls: vec![url::Url::parse(&format!(
                    "https://cdn.modrinth.com/data/proj/versions/v1/{path}"
                ))
                .unwrap()],
                sha1: sha1.into(),
                sha512: "5".repeat(128),
                size: 3,
            }),
            from_overrides: false,
            project_id: Some("proj".into()),
        };
        let files = BTreeMap::from([
            ("mods/a.jar".to_string(), download("mods/a.jar", "aa")),
            (
                "config/c.toml".to_string(),
                PackFile {
                    sha1: "cc".into(),
                    download: None,
                    from_overrides: true,
                    project_id: None,
                },
            ),
        ]);
        let record = PackRecord {
            files: BTreeMap::from([
                ("mods/a.jar".into(), recorded("aa", false, Some("proj"))),
                ("config/c.toml".into(), recorded("cc", true, None)),
                // a script the pack's files left out, kept as the pack's
                ("kubejs/x.js".into(), recorded("dd", true, None)),
                // a pack that ships a disabled mod of its own
                ("mods/b.jar.disabled".into(), recorded("bb", true, None)),
            ]),
        };
        let origin = ModpackOrigin {
            project_id: "pack".into(),
            version_id: "v1".into(),
            name: "Pack".into(),
            version_number: None,
            icon_url: None,
        };
        let info = base_info(&origin, None, &record, &files);
        // without the .mrpack there is no base
        assert_eq!(info.base, None);
        assert!(!info.files.contains_key("mods/a.jar.disabled"));
        assert_eq!(info.files["mods/a.jar"].size, Some(3));
        assert_eq!(record_of_base(&info), record);
    }
}
