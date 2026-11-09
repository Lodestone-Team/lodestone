//! Making a new instance from a pack.
//!
//! Everything is built in a hidden folder next to the instances
//! (`.lodestone-staging-…`, which the instance list skips) and renamed into place at
//! the end, so a failed or cancelled import leaves nothing behind.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
};

use futures::{StreamExt as _, TryStreamExt as _};
use lodestone_minecraft::{hash, mrpack::PackDownload};

use super::{
    FileSource, ModpackError, ModpackRecord, OverrideFile, PackFile, PackInfo, cdn_ids, io_error,
    slash_path,
};
use crate::{
    content::{
        ContentKind, Modrinth, enabled_name,
        install::InstallProgress,
        is_content_name,
        provenance::{Entry, ModrinthSource, Provenance, nanos},
    },
    game::{
        download::{CancelToken, DownloadItem, Downloader, ProgressSnapshot},
        loaders::LoaderChoice,
    },
    instances::{CUSTOM_ICON, ICON_FILE, Instance, InstanceStore, Loader, icon_png},
    storage,
};

/// Files are downloaded this many at a time.
const PARALLEL_DOWNLOADS: usize = 6;

/// What the new instance gets besides the pack's files.
#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    pub name: String,
    pub loader: Option<LoaderChoice>,
    /// Where the pack came from (its files are filled in by the import).
    pub record: ModpackRecord,
    /// The project's icon (PNG, JPEG, WebP or GIF), made the instance's icon.
    pub icon: Option<Vec<u8>>,
}

/// Creates the parent folders of `target` and checks that they stay inside `root` once
/// links are resolved.
pub(crate) fn prepare_parent(root: &Path, target: &Path) -> Result<(), ModpackError> {
    let parent = target
        .parent()
        .ok_or_else(|| ModpackError::Invalid(format!("{} has no folder", target.display())))?;
    fs::create_dir_all(parent).map_err(io_error(parent))?;
    let canonical_root = fs::canonicalize(root).map_err(io_error(root))?;
    let canonical_parent = fs::canonicalize(parent).map_err(io_error(parent))?;
    if !canonical_parent.starts_with(&canonical_root)
        || fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(ModpackError::Invalid(
            "The modpack is unsafe: a path leaves the instance folder".into(),
        ));
    }
    Ok(())
}

/// Downloads `downloads` to the paths `dest` gives them (under `root`), trying each URL
/// in order, with SHA-1 and size checked by the downloader and SHA-512 checked after.
pub(crate) async fn download_files(
    api: &Modrinth,
    downloader: &Downloader,
    root: &Path,
    downloads: &[(PackDownload, PathBuf)],
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<(), ModpackError> {
    progress.downloads.set(ProgressSnapshot {
        files_total: downloads.len() as u64,
        bytes_total: downloads.iter().map(|(d, _)| d.size).sum(),
        ..ProgressSnapshot::default()
    });
    // Boxed, so the future stays `Send` for any lifetime (a closure over the borrowed
    // slice trips the compiler's higher-ranked `Send` check).
    let fetches: Vec<futures::future::BoxFuture<'_, Result<(), ModpackError>>> = downloads
        .iter()
        .map(|(download, dest)| {
            Box::pin(fetch_one(
                api, downloader, root, download, dest, progress, cancel,
            )) as futures::future::BoxFuture<'_, _>
        })
        .collect();
    futures::stream::iter(fetches)
        .buffer_unordered(PARALLEL_DOWNLOADS)
        .try_collect::<Vec<()>>()
        .await?;
    Ok(())
}

/// Downloads one file of a pack, trying its URLs in order.
async fn fetch_one(
    api: &Modrinth,
    downloader: &Downloader,
    root: &Path,
    download: &PackDownload,
    dest: &Path,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<(), ModpackError> {
    prepare_parent(root, dest)?;
    let mut last_error = None;
    for url in &download.urls {
        if cancel.is_cancelled() {
            return Err(ModpackError::Cancelled);
        }
        let item = DownloadItem {
            url: api.pack_file_url(url)?,
            dest: dest.to_path_buf(),
            sha1: download.sha1.clone(),
            size: (download.size > 0).then_some(download.size),
            executable: false,
        };
        match downloader.fetch(&item, &progress.downloads, cancel).await {
            Ok(()) => {}
            Err(crate::game::download::DownloadError::Cancelled) => {
                return Err(ModpackError::Cancelled);
            }
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        }
        let sha512 = hash::sha512_of(dest).map_err(io_error(dest))?;
        if sha512.eq_ignore_ascii_case(&download.sha512) {
            return Ok(());
        }
        let _ = fs::remove_file(dest);
        last_error = Some(format!(
            "the download from {} is damaged: its SHA-512 hash doesn't match",
            url.host_str().unwrap_or_default()
        ));
    }
    Err(ModpackError::Invalid(format!(
        "Couldn't download {}: {}",
        slash_path(&download.path),
        last_error.unwrap_or_else(|| "it has no download URL".into())
    )))
}

/// Copies the override files `files` from the pack at `pack_path` to the paths `dest`
/// gives them (under `root`), each through a temporary file.
pub(crate) fn extract_overrides(
    pack_path: &Path,
    root: &Path,
    files: &[(OverrideFile, PathBuf)],
    cancel: &CancelToken,
) -> Result<(), ModpackError> {
    let file = File::open(pack_path).map_err(io_error(pack_path))?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(super::zip_error)?;
    for (file, dest) in files {
        if cancel.is_cancelled() {
            return Err(ModpackError::Cancelled);
        }
        prepare_parent(root, dest)?;
        let mut entry = archive.by_index(file.index).map_err(super::zip_error)?;
        storage::write_atomic_with(dest, |out| {
            let copied = io::copy(&mut (&mut entry).take(file.size.saturating_add(1)), out)?;
            if copied != file.size {
                return Err(io::Error::other("the file isn't the size the zip says"));
            }
            out.flush()
        })
        .map_err(io_error(dest))?;
    }
    Ok(())
}

/// The `content.json` entries of downloaded pack files that are content (mods, resource
/// packs, shader packs directly in their folder). `files` are the paths on disk relative
/// to `game` with the downloads they came from. Modrinth is asked which versions they are;
/// when it can't be, the source comes from the CDN URL.
pub(crate) async fn content_entries(
    api: Option<&Modrinth>,
    game: &Path,
    files: &[(String, &PackDownload)],
) -> Vec<(ContentKind, String, Entry)> {
    let mut candidates = Vec::new();
    for (relative, download) in files {
        let Some((dir, name)) = relative.split_once('/') else {
            continue;
        };
        let Some(kind) = ContentKind::ALL.into_iter().find(|k| k.dir() == dir) else {
            continue;
        };
        if !is_content_name(kind, name, false) {
            continue;
        }
        let Ok(metadata) = fs::metadata(game.join(relative)) else {
            continue;
        };
        candidates.push((kind, name.to_string(), metadata, *download));
    }
    let hashes: Vec<String> = candidates
        .iter()
        .map(|(_, _, _, d)| d.sha1.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let found = match api {
        Some(api) if !hashes.is_empty() => crate::content::listing::lookup(api, &hashes).await.ok(),
        _ => None,
    };
    candidates
        .into_iter()
        .map(|(kind, name, metadata, download)| {
            let looked_up = found.as_ref().and_then(|f| f.get(&download.sha1));
            let modrinth = match looked_up {
                Some((version, project)) => Some(ModrinthSource::new(
                    version,
                    project.as_ref(),
                    &download.sha512,
                )),
                None => download.urls.iter().find_map(|url| {
                    let (project_id, version_id) = cdn_ids(url)?;
                    Some(ModrinthSource {
                        project_id,
                        version_id,
                        url: Some(url.to_string()),
                        ..ModrinthSource::default()
                    })
                }),
            };
            let entry = Entry {
                size: metadata.len(),
                modified: metadata.modified().ok().and_then(nanos),
                sha1: download.sha1.clone(),
                sha512: download.sha512.clone(),
                looked_up: found.is_some() && modrinth.is_none(),
                modrinth,
            };
            (kind, enabled_name(&name), entry)
        })
        .collect()
}

/// The `.mrpack` file of a Modrinth version: the primary `.mrpack`, or the first one.
pub fn pack_file(
    version: &lodestone_minecraft::modrinth::RawVersion,
) -> Option<&lodestone_minecraft::modrinth::RawVersionFile> {
    let is_mrpack = |name: &str| name.to_ascii_lowercase().ends_with(".mrpack");
    version
        .files
        .iter()
        .find(|f| f.primary && is_mrpack(&f.filename))
        .or_else(|| version.files.iter().find(|f| is_mrpack(&f.filename)))
}

/// Downloads the `.mrpack` of a Modrinth version into `dir` (under a hidden name), from
/// Modrinth's CDN only, with its SHA-1, size and SHA-512 checked. The caller removes the
/// file when done.
pub async fn download_pack(
    api: &Modrinth,
    downloader: &Downloader,
    version: &lodestone_minecraft::modrinth::RawVersion,
    dir: &Path,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<PathBuf, ModpackError> {
    let file = pack_file(version)
        .ok_or_else(|| ModpackError::Invalid(format!("{} has no .mrpack file.", version.name)))?;
    if file.size > lodestone_minecraft::mrpack::MAX_PACK_FILE_BYTES {
        return Err(ModpackError::Invalid(format!(
            "{} is larger than 1 GB.",
            file.filename
        )));
    }
    let hex = |value: &Option<String>, len: usize| {
        value
            .as_deref()
            .map(str::to_ascii_lowercase)
            .filter(|h| h.len() == len && h.chars().all(|c| c.is_ascii_hexdigit()))
    };
    let (Some(sha1), Some(sha512)) = (hex(&file.hashes.sha1, 40), hex(&file.hashes.sha512, 128))
    else {
        return Err(ModpackError::Invalid(format!(
            "Modrinth sent no hashes for {}, so it can't be checked.",
            file.filename
        )));
    };
    fs::create_dir_all(dir).map_err(io_error(dir))?;
    let dest = dir.join(format!(".lodestone-pack-{sha1}.mrpack"));
    let item = DownloadItem {
        url: api.cdn_url(&file.url)?,
        dest: dest.clone(),
        sha1,
        size: Some(file.size),
        executable: false,
    };
    progress.set_label(format!("Downloading {}", file.filename));
    progress.downloads.set(ProgressSnapshot {
        files_total: 1,
        bytes_total: file.size,
        ..ProgressSnapshot::default()
    });
    match downloader.fetch(&item, &progress.downloads, cancel).await {
        Ok(()) => {}
        Err(crate::game::download::DownloadError::Cancelled) => {
            let _ = fs::remove_file(&dest);
            return Err(ModpackError::Cancelled);
        }
        Err(err) => {
            let _ = fs::remove_file(&dest);
            return Err(err.into());
        }
    }
    let actual = hash::sha512_of(&dest).map_err(io_error(&dest))?;
    if !actual.eq_ignore_ascii_case(&sha512) {
        let _ = fs::remove_file(&dest);
        return Err(ModpackError::Invalid(format!(
            "The download of {} is damaged: its SHA-512 hash doesn't match.",
            file.filename
        )));
    }
    Ok(dest)
}

/// The record of a pack's files, as the pack has them.
pub(crate) fn record_files(pack: &PackInfo) -> std::collections::BTreeMap<String, PackFile> {
    let mut files = std::collections::BTreeMap::new();
    for download in &pack.downloads {
        files.insert(
            slash_path(&download.path),
            PackFile {
                sha1: download.sha1.clone(),
                source: FileSource::Index,
                project: download
                    .urls
                    .iter()
                    .find_map(|u| cdn_ids(u).map(|(p, _)| p)),
            },
        );
    }
    // Overrides are copied after the downloads, so they win.
    for file in &pack.overrides {
        files.insert(
            file.path.clone(),
            PackFile {
                sha1: file.sha1.clone(),
                source: FileSource::Override,
                project: None,
            },
        );
    }
    files
}

/// Makes a new instance from the pack at `pack_path` (read by [`super::read_pack`]).
#[allow(clippy::too_many_arguments)]
pub async fn import(
    store: &InstanceStore,
    api: Option<&Modrinth>,
    downloader: &Downloader,
    pack_path: &Path,
    pack: &PackInfo,
    options: ImportOptions,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<Instance, ModpackError> {
    let staging = store.new_staging()?;
    let result = build(
        store, api, downloader, pack_path, pack, options, progress, cancel, &staging,
    )
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn build(
    store: &InstanceStore,
    api: Option<&Modrinth>,
    downloader: &Downloader,
    pack_path: &Path,
    pack: &PackInfo,
    options: ImportOptions,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &Path,
) -> Result<Instance, ModpackError> {
    let game = staging.join("minecraft");
    fs::create_dir_all(&game).map_err(io_error(&game))?;
    if !pack.downloads.is_empty() {
        let api = api.ok_or_else(|| {
            ModpackError::Invalid("Modrinth can't be reached to download the pack's files.".into())
        })?;
        progress.set_label(format!("Downloading {} files", pack.downloads.len()));
        let downloads: Vec<(PackDownload, PathBuf)> = pack
            .downloads
            .iter()
            .map(|d| (d.clone(), game.join(&d.path)))
            .collect();
        download_files(api, downloader, &game, &downloads, progress, cancel).await?;
    }
    progress.set_label("Copying the pack's configs");
    let overrides: Vec<(OverrideFile, PathBuf)> = pack
        .overrides
        .iter()
        .map(|o| (o.clone(), game.join(&o.path)))
        .collect();
    extract_overrides(pack_path, &game, &overrides, cancel)?;
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }

    progress.set_label("Recording the pack's files");
    // Files an override replaced aren't the downloaded ones any more.
    let overridden: BTreeSet<&str> = pack.overrides.iter().map(|o| o.path.as_str()).collect();
    let downloaded: Vec<(String, &PackDownload)> = pack
        .downloads
        .iter()
        .map(|d| (slash_path(&d.path), d))
        .filter(|(path, _)| !overridden.contains(path.as_str()))
        .collect();
    let mut provenance = Provenance::default();
    for (kind, name, entry) in content_entries(api, &game, &downloaded).await {
        provenance.insert(kind, &name, entry);
    }
    provenance.save(staging)?;
    let record = ModpackRecord {
        files: record_files(pack),
        ..options.record
    };
    record.save(staging)?;

    let mut instance = Instance {
        name: options.name,
        game_version: pack.game_version.clone(),
        loader: Loader::from_kind(options.loader.as_ref().map(|l| l.kind)),
        loader_version: options.loader.map(|l| l.version),
        created: crate::instances::now(),
        ..Instance::default()
    };
    if let Some(icon) = options
        .icon
        .as_deref()
        .and_then(|bytes| icon_png(bytes).ok())
    {
        let path = staging.join(ICON_FILE);
        storage::write_atomic(&path, &icon).map_err(io_error(&path))?;
        instance.icon = Some(CUSTOM_ICON.into());
    }
    Ok(store.commit_staged(staging, instance)?)
}

/// The SHA-1 of every file a pack wrote that is on disk, for tests and the update.
pub(crate) fn sha1_on_disk(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    hash::sha1_of(path).ok()
}
