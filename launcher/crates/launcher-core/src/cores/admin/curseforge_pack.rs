//! A CurseForge modpack as a pack a core makes a server from.
//!
//! The core reads Modrinth's pack format and has no CurseForge client, so Lodestone
//! Host turns a CurseForge modpack into a `.mrpack` and uploads it
//! ([`AdminClient::upload_modpack`](super::AdminClient::upload_modpack)); from there the
//! core makes, updates and restores the server as it does for any uploaded pack.
//! [`build`] does the turning:
//!
//! - the pack's zip is downloaded from CurseForge's CDN and read
//!   ([`read_curseforge_pack`]), and CurseForge is asked which files its manifest names;
//! - only mods go to a server: resource packs, shader packs and the like are left out,
//!   and so are mods their file's tags or Modrinth say are for players only. A mod the
//!   pack marks optional comes disabled, as in the Launcher;
//! - a file Modrinth has too (the same SHA-1) is listed from Modrinth's CDN, with
//!   Modrinth's SHA-512. Any other is listed from CurseForge's CDN, and since CurseForge
//!   publishes no SHA-512, it is downloaded here once to work its SHA-512 out (checked
//!   by its SHA-1) and thrown away: the core downloads it again, and checks both;
//! - a file whose author keeps it on CurseForge's website, and that Modrinth doesn't
//!   have, is left out and listed ([`ServerPack::by_hand`]) for the user to download
//!   and upload to the server once it is made;
//! - the pack's overrides are copied as they are, and the index says which CurseForge
//!   file the pack was made from (`lodestone.curseforge`), which the core keeps with
//!   the server so the Host can look for a newer one ([`newer_file`]).

use std::{
    collections::{BTreeMap, HashMap},
    io::Write as _,
    path::{Path, PathBuf},
};

use futures::StreamExt as _;
use lodestone_minecraft::mrpack::{
    CURSEFORGE_DOWNLOAD_HOSTS, INDEX_FILE, MAX_PACK_FILE_BYTES, SkippedPath,
};
use serde_json::json;

use super::{
    curseforge::{ServerCfError, file_label},
    models::CurseForgeOrigin,
};
use crate::{
    content::{
        ContentKind, CurseForge, CurseForgeError, Modrinth,
        curseforge::{CLASS_MODS, CfFile, CfMod},
        disabled_name,
        listing::curseforge_type_allowed,
        safe_download_name,
    },
    game::{
        download::{CancelToken, DownloadItem, Downloader, Progress},
        loaders::LoaderKind,
    },
    modpack::curseforge::read_curseforge_pack,
};

/// How many CurseForge files are downloaded at a time to work out their SHA-512.
const HASHED_AT_ONCE: usize = 4;
/// Modrinth's CDN, where a file both platforms have is listed from.
const MODRINTH_CDN: &str = "cdn.modrinth.com";

/// A file the user has to download from curseforge.com and upload to the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByHand {
    pub title: String,
    pub file_name: String,
    /// Where it is downloaded.
    pub page_url: String,
}

/// What [`build`] is doing, for the window that waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackStep {
    /// Downloading the pack's zip.
    Downloading,
    /// Asking CurseForge and Modrinth about its files.
    Reading,
    /// Working out the SHA-512 of the files only CurseForge has.
    Hashing {
        done: usize,
        total: usize,
    },
    Writing,
}

/// A CurseForge modpack turned into a `.mrpack` for a server.
#[derive(Clone, Debug)]
pub struct ServerPack {
    /// The `.mrpack`, in the staging folder.
    pub path: PathBuf,
    /// The name to upload it under.
    pub file_name: String,
    pub by_hand: Vec<ByHand>,
    /// What was left out, and why (the core says what else it skips).
    pub skipped: Vec<SkippedPath>,
}

fn invalid(message: impl std::fmt::Display) -> ServerCfError {
    ServerCfError::Invalid(message.to_string())
}

/// A file of the pack that goes to the server.
struct Wanted {
    file: CfFile,
    project: Option<CfMod>,
    /// Where it goes, relative to the server's folder.
    path: String,
    sha1: String,
    required: bool,
}

/// A file as the index lists it.
struct Listed {
    path: String,
    url: String,
    sha1: String,
    sha512: String,
    size: u64,
}

fn skip(path: impl Into<String>, reason: &str) -> SkippedPath {
    SkippedPath {
        path: path.into(),
        reason: reason.into(),
    }
}

/// `url` as an HTTPS URL on one of `hosts`, with no port or user.
fn url_on(url: &str, hosts: &[&str]) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let ok = parsed.scheme() == "https"
        && parsed.port().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.host_str().is_some_and(|host| hosts.contains(&host));
    ok.then(|| parsed.to_string())
}

/// Downloads `file` from CurseForge's CDN to `dest`, checked by its SHA-1 and size.
async fn fetch(
    cf: &CurseForge,
    downloader: &Downloader,
    file: &CfFile,
    dest: &Path,
    cancel: &CancelToken,
) -> Result<(), ServerCfError> {
    if file.file_length > MAX_PACK_FILE_BYTES {
        return Err(invalid(format!("{} is too large", file.file_name)));
    }
    let url = cf.download_url(file)?;
    let sha1 = file
        .sha1()
        .ok_or_else(|| invalid(format!("CurseForge sent no SHA-1 for {}", file.file_name)))?;
    let item = DownloadItem {
        url,
        dest: dest.to_path_buf(),
        sha1,
        size: (file.file_length > 0).then_some(file.file_length),
        executable: false,
    };
    downloader
        .fetch(&item, &Progress::default(), cancel)
        .await
        .map_err(Into::into)
}

/// The loader a pack needs, as a `.mrpack` index names it.
fn loader_dependency(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Fabric => "fabric-loader",
        LoaderKind::Quilt => "quilt-loader",
        LoaderKind::Forge => "forge",
        LoaderKind::NeoForge => "neoforge",
    }
}

/// Turns the CurseForge modpack `file` of `project` into a `.mrpack` in `staging` (a
/// folder of the caller's, which it removes), as the module says. The pack's file must
/// be one the Host may download: a pack its author keeps on CurseForge's website can't
/// be made a server from here.
#[allow(clippy::too_many_arguments)]
pub async fn build(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    downloader: &Downloader,
    project: &CfMod,
    file: &CfFile,
    staging: &Path,
    progress: &(dyn Fn(PackStep) + Send + Sync),
    cancel: &CancelToken,
) -> Result<ServerPack, ServerCfError> {
    progress(PackStep::Downloading);
    let zip_path = staging.join(format!("curseforge-pack-{}.zip", file.id));
    fetch(cf, downloader, file, &zip_path, cancel).await?;
    let pack = read_curseforge_pack(&zip_path).map_err(invalid)?;

    progress(PackStep::Reading);
    let ids: Vec<u64> = pack.files.iter().map(|f| f.file_id).collect();
    let found: HashMap<u64, CfFile> = cf
        .files_by_id(&ids)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let mut project_ids: Vec<u64> = found.values().map(|f| f.mod_id).collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let projects: HashMap<u64, CfMod> = if project_ids.is_empty() {
        HashMap::new()
    } else {
        cf.projects(&project_ids)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect()
    };

    let mut skipped: Vec<SkippedPath> = Vec::new();
    let mut wanted: Vec<Wanted> = Vec::new();
    let mut taken: std::collections::HashSet<String> = Default::default();
    for entry in &pack.files {
        let Some(file) = found.get(&entry.file_id) else {
            skipped.push(skip(
                format!("CurseForge file {}", entry.file_id),
                "CurseForge no longer has it",
            ));
            continue;
        };
        let project = projects.get(&file.mod_id);
        let is_mod = match project.and_then(|p| p.class_id) {
            Some(class) => class == CLASS_MODS,
            // without its project, a jar is most likely a mod
            None => file.file_name.to_ascii_lowercase().ends_with(".jar"),
        };
        if !is_mod {
            skipped.push(skip(&file.file_name, "a server doesn't use it"));
            continue;
        }
        let Some(name) = safe_download_name(ContentKind::Mod, &file.file_name) else {
            skipped.push(skip(&file.file_name, "its file name can't be used"));
            continue;
        };
        let tags = file.tags();
        if tags.client && !tags.server {
            skipped.push(skip(name, "it is for players only"));
            continue;
        }
        let Some(sha1) = file.sha1() else {
            skipped.push(skip(name, "CurseForge sent no SHA-1 to check it with"));
            continue;
        };
        // a mod the pack marks optional comes disabled, for the owner to turn on
        let disk_name = if entry.required {
            name
        } else {
            disabled_name(&name)
        };
        let path = format!("mods/{disk_name}");
        if !taken.insert(path.to_lowercase()) {
            skipped.push(skip(path, "another file of the pack has its name"));
            continue;
        }
        wanted.push(Wanted {
            file: file.clone(),
            project: project.cloned(),
            path,
            sha1,
            required: entry.required,
        });
    }

    // The same bytes on Modrinth: its CDN and its SHA-512, and whether servers run it.
    let on_modrinth = match modrinth {
        Some(modrinth) if !wanted.is_empty() => {
            let hashes: Vec<String> = wanted.iter().map(|w| w.sha1.clone()).collect();
            // Modrinth out of reach only means the files come from CurseForge
            modrinth.versions_by_sha1(&hashes).await.unwrap_or_default()
        }
        _ => HashMap::new(),
    };
    let sides: HashMap<String, String> = match modrinth {
        Some(modrinth) if !on_modrinth.is_empty() => {
            let mut ids: Vec<String> = on_modrinth.values().map(|v| v.project_id.clone()).collect();
            ids.sort();
            ids.dedup();
            modrinth
                .projects(&ids)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|p| (p.id, p.server_side))
                .collect()
        }
        _ => HashMap::new(),
    };

    let mut listed: Vec<Listed> = Vec::new();
    let mut to_hash: Vec<Wanted> = Vec::new();
    let mut by_hand: Vec<ByHand> = Vec::new();
    for want in wanted {
        let version = on_modrinth.get(&want.sha1);
        if version
            .is_some_and(|v| sides.get(&v.project_id).map(String::as_str) == Some("unsupported"))
        {
            skipped.push(skip(want.path, "it is for players only"));
            continue;
        }
        let same = version.and_then(|version| {
            let file = version.files.iter().find(|f| {
                f.hashes
                    .sha1
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&want.sha1))
            })?;
            let sha512 = file
                .hashes
                .sha512
                .clone()
                .filter(|h| h.len() == 128 && h.bytes().all(|b| b.is_ascii_hexdigit()))?;
            Some((url_on(&file.url, &[MODRINTH_CDN])?, sha512, file.size))
        });
        if let Some((url, sha512, size)) = same {
            listed.push(Listed {
                path: want.path,
                url,
                sha1: want.sha1,
                sha512: sha512.to_ascii_lowercase(),
                size,
            });
        } else if want.file.downloadable() {
            to_hash.push(want);
        } else if want.required {
            let page = want
                .project
                .as_ref()
                .map(|p| p.page_url())
                .unwrap_or_else(|| {
                    format!("https://www.curseforge.com/projects/{}", want.file.mod_id)
                });
            by_hand.push(ByHand {
                title: want
                    .project
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| file_label(&want.file)),
                file_name: want.file.file_name.clone(),
                page_url: want.file.page_url(&page),
            });
        } else {
            skipped.push(skip(
                want.path,
                "it is optional and only on CurseForge's website",
            ));
        }
    }

    // CurseForge publishes no SHA-512: each of its files is downloaded once to work it out.
    let total = to_hash.len();
    progress(PackStep::Hashing { done: 0, total });
    let done = std::sync::atomic::AtomicUsize::new(0);
    let hashed: Vec<Result<Listed, ServerCfError>> =
        futures::stream::iter(to_hash.into_iter().map(|want| {
            let done = &done;
            async move {
                let url = want
                    .file
                    .download_url
                    .as_deref()
                    .and_then(|url| url_on(url, &CURSEFORGE_DOWNLOAD_HOSTS))
                    .ok_or_else(|| {
                        invalid(format!(
                            "{} isn't downloaded from CurseForge's CDN",
                            want.file.file_name
                        ))
                    })?;
                let dest = staging.join(format!("hash-{}.jar", want.file.id));
                let fetched = fetch(cf, downloader, &want.file, &dest, cancel).await;
                let sha512 = fetched.and_then(|()| {
                    lodestone_minecraft::hash::sha512_of(&dest)
                        .map_err(|err| ServerCfError::Download(err.to_string()))
                });
                let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                let _ = std::fs::remove_file(&dest);
                let sha512 = sha512?;
                let n = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                progress(PackStep::Hashing { done: n, total });
                Ok(Listed {
                    path: want.path,
                    url,
                    sha1: want.sha1,
                    sha512,
                    size,
                })
            }
        }))
        .buffer_unordered(HASHED_AT_ONCE)
        .collect()
        .await;
    for file in hashed {
        listed.push(file?);
    }
    listed.sort_by(|a, b| a.path.cmp(&b.path));

    progress(PackStep::Writing);
    let mut dependencies: BTreeMap<&str, String> = BTreeMap::new();
    dependencies.insert("minecraft", pack.game_version.clone());
    if let Some((kind, version)) = &pack.loader {
        dependencies.insert(loader_dependency(*kind), version.clone());
    }
    let name = if project.name.trim().is_empty() {
        pack.name.clone()
    } else {
        project.name.trim().to_string()
    };
    let version = pack.version.clone().unwrap_or_else(|| file_label(file));
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": version,
        "name": name,
        "summary": project.summary,
        "files": listed.iter().map(|file| json!({
            "path": file.path,
            "hashes": { "sha1": file.sha1, "sha512": file.sha512 },
            "downloads": [file.url],
            "fileSize": file.size,
        })).collect::<Vec<_>>(),
        "dependencies": dependencies,
        "lodestone": { "curseforge": {
            "project": project.id,
            "file": file.id,
            "pageUrl": project.page_url(),
            "iconUrl": project.icon_url(),
        } },
    });
    let out = staging.join(format!("curseforge-pack-{}.mrpack", file.id));
    write_pack(&zip_path, &pack.overrides, &index, &out)?;
    let _ = std::fs::remove_file(&zip_path);
    skipped.extend(pack.skipped.iter().cloned());

    let stem: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Ok(ServerPack {
        path: out,
        file_name: format!("{}-{}.mrpack", stem.trim_matches('-'), file.id),
        by_hand,
        skipped,
    })
}

/// [`build`]s the pack in a staging folder of its own and uploads it to the core:
/// what the core says is in it (with the `upload_id` a server is made or updated
/// from), and the files the user has to fetch by hand.
#[allow(clippy::too_many_arguments)]
pub async fn build_and_upload(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    downloader: &Downloader,
    admin: &super::AdminClient,
    project: &CfMod,
    file: &CfFile,
    progress: &(dyn Fn(PackStep) + Send + Sync),
    cancel: &CancelToken,
) -> Result<(super::mods::ModpackInfo, Vec<ByHand>), ServerCfError> {
    let staging =
        super::curseforge::staging_dir().map_err(|err| ServerCfError::Download(err.to_string()))?;
    let result = async {
        let pack = build(
            cf, modrinth, downloader, project, file, &staging, progress, cancel,
        )
        .await?;
        let bytes =
            std::fs::read(&pack.path).map_err(|err| ServerCfError::Download(err.to_string()))?;
        let info = admin.upload_modpack(&pack.file_name, &bytes).await?;
        Ok((info, pack.by_hand))
    }
    .await;
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Writes the `.mrpack`: its index, then the CurseForge pack's overrides, copied from
/// its zip as they are stored there.
fn write_pack(
    source: &Path,
    overrides: &[crate::modpack::OverrideFile],
    index: &serde_json::Value,
    out: &Path,
) -> Result<(), ServerCfError> {
    let failed = |err: &dyn std::fmt::Display| {
        ServerCfError::Download(format!("Couldn't write the modpack: {err}"))
    };
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
        std::fs::File::open(source).map_err(|e| failed(&e))?,
    ))
    .map_err(|e| failed(&e))?;
    let mut writer = zip::ZipWriter::new(std::fs::File::create(out).map_err(|e| failed(&e))?);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file(INDEX_FILE, deflated)
        .map_err(|e| failed(&e))?;
    writer
        .write_all(&serde_json::to_vec_pretty(index).map_err(|e| failed(&e))?)
        .map_err(|e| failed(&e))?;
    for file in overrides {
        let entry = archive.by_index_raw(file.index).map_err(|e| failed(&e))?;
        writer
            .raw_copy_file_rename(entry, format!("overrides/{}", file.path))
            .map_err(|e| failed(&e))?;
    }
    writer
        .finish()
        .map_err(|e| failed(&e))?
        .sync_all()
        .map_err(|e| failed(&e))
}

/// The newest file of the modpack `origin` names that is newer than the one the server
/// was made from, and at least as stable (the Launcher's rule for CurseForge packs).
/// `None` when the server is up to date. Server packs (an author's own zips of a ready
/// server) are never offered: they aren't modpack files. One request: the project's
/// newest files, among which a server's own file is unless it is far behind.
pub async fn newer_file(
    cf: &CurseForge,
    origin: &CurseForgeOrigin,
) -> Result<Option<CfFile>, CurseForgeError> {
    // newest first
    let files = cf.latest_files(origin.project).await?;
    let current = files.iter().find(|f| f.id == origin.file);
    let (stability, since) = match current {
        Some(current) => (current.version_type(), current.file_date.as_str()),
        // a file CurseForge no longer lists: anything newer than nothing
        None => ("release", ""),
    };
    Ok(files
        .iter()
        .find(|f| curseforge_type_allowed(stability, f.version_type(), false))
        .filter(|newest| newest.id != origin.file && newest.file_date.as_str() > since)
        .cloned())
}
