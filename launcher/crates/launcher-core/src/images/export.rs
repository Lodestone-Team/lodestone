//! Sharing a snapshot as a file: an image export (`docs/instance-images.md`, "Sharing"),
//! and making a new instance from one.
//!
//! **Exporting** ([`Images::export_snapshot`]) writes the shared crate's export container
//! (`lodestone_image::container`): `image.json` (the snapshot's data or full image), the
//! content image a data image links to, and `objects/` with only the files whose source is
//! `object`. Mods and packs from Modrinth or CurseForge travel as references, not jars,
//! so an export of a modded instance is mostly its worlds and configs. Files that must
//! never leave the computer ([`PRIVATE_FILES`]: the commands the player typed, a
//! server's `/login` password among them, and account switchers' account files) are taken
//! out of both images, which say so in their `excludes`; the export's ids are then those
//! of the stripped images. Best effort: an account file of a mod not listed still travels,
//! and the dialog says what was left out.
//!
//! **Importing** ([`import_file`]) always makes a new instance. The file comes from someone
//! else, so it trusts nothing: the container's reader checks every name, size, count and
//! hash; names Lodestone keeps for itself (`.lodestone…`) and paths the client can't place
//! are refused; the caller is asked whether the disk has room for everything first; the
//! objects are imported (every byte checked), Modrinth files downloaded from Modrinth's CDN
//! (SHA-512 and SHA-256 checked) and CurseForge files through CurseForge's API when their
//! authors allow it; the rest are listed as left out. The instance gets the image's game
//! and recommended memory and default launch settings (images carry none), and its
//! `content.json` the files' sources. Its first content image is "Made from a snapshot",
//! and the imported image is kept as an "imported" snapshot, so the user can go back to
//! what they were sent.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lodestone_image::container::{self, Limits};
use lodestone_image::store::excludes::Excludes;
use lodestone_image::store::materialize::{MaterializeOptions, Selection, materialize};
use lodestone_image::store::{Cancel, NEVER};
use lodestone_image::{CanonicalPath, DiskLayout, Entry, Image, Kind, Source};
use serde::Serialize;
use sha2::Digest as _;

use super::labels;
use super::recorder::SPARE_BYTES;
use super::scope::is_lodestone_name;
use super::snapshots::{SnapshotKind, SnapshotRecord, new_record_id, newest_first, snapshot_name};
use super::{ImageError, Images};
use crate::content::{ContentKind, CurseForge, InstanceDirs, Modrinth, enabled_name};
use crate::game::download::{CancelToken, DownloadItem, Downloader, Progress};
use crate::instances::{self, Instance};

/// An image export's file extension.
pub const EXTENSION: &str = "lodeimg";

/// Files an export never carries, as image exclude patterns (a name at any depth, a path
/// with `/` from the game folder): the commands the player typed (passwords given to a
/// server's `/login` among them), the saved hotbars, and the account files of account
/// switchers that keep sign-ins in the game folder (In-Game Account Switcher's
/// `ias.json`, Essential's account files).
pub const PRIVATE_FILES: &[&str] = &[
    "command_history.txt",
    "hotbar.nbt",
    "ias.json",
    "essential/microsoft_accounts.json",
    "essential/mojang_accounts.json",
    // The JVM's crash logs and dumps (`scope::is_crash_name`), which snapshots leave out:
    // a crash log holds the command line, access token included.
    "/hs_err_pid*",
    "/replay_pid*",
    "/java_pid*.hprof",
    "/*.mdmp",
    "/core",
];

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportReport {
    pub file: PathBuf,
    /// Its size.
    pub bytes: u64,
    /// Files whose bytes it carries.
    pub objects: usize,
    /// Files it names as Modrinth or CurseForge downloads.
    pub references: usize,
    /// CurseForge files whose authors don't allow downloads elsewhere, by path: named, not
    /// carried, so whoever imports the file won't get them.
    pub not_downloadable: Vec<String>,
    /// Files taken out ([`PRIVATE_FILES`]), by path.
    pub left_out: Vec<String>,
}

/// What an export file holds, for the dialog before importing it. Text came from someone
/// else: show it through `lodestone_image::display_text` (the `_shown` fields already
/// are).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportPreview {
    /// The image's id, for [`import_file`]'s `expect_image`.
    pub image: String,
    /// The snapshot's name, written visibly.
    pub name_shown: String,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    /// Unix seconds.
    pub created: i64,
    /// The Lodestone version that made it, written visibly.
    pub producer_shown: String,
    /// Its worlds, by their own names when known, written visibly.
    pub worlds: Vec<String>,
    /// The files loaders load (anything under `mods/`, and `coremods/`), by their path
    /// under `mods/` when on, written visibly: programs that run when the game starts.
    pub mods: Vec<String>,
    /// Those of `mods` whose bytes the file carries (not Modrinth or CurseForge downloads):
    /// they come from whoever made the file.
    pub carried_mods: Vec<String>,
    /// How many files it has.
    pub files: usize,
    /// What importing unpacks into the image store.
    pub unpacked_bytes: u64,
    /// What its files take once placed.
    pub files_bytes: u64,
    /// What it downloads (Modrinth and CurseForge files).
    pub download_bytes: u64,
    /// Everything the import writes and 1 GiB to spare: what the free-space check asks for.
    pub needed_bytes: u64,
    /// CurseForge files whose authors don't allow downloads: left out.
    pub not_downloadable: Vec<String>,
    /// A full image (no linked content).
    pub full: bool,
}

/// What [`import_file`] made.
#[derive(Debug, Clone)]
pub struct Imported {
    pub instance: Instance,
    /// Files it couldn't get (CurseForge files that can't be downloaded), by path.
    pub left_out: Vec<String>,
}

/// Where an import downloads from.
pub struct Sources<'a> {
    pub downloader: &'a Downloader,
    pub modrinth: &'a Modrinth,
    /// `None` without a CurseForge key: CurseForge files are then left out.
    pub curseforge: Option<&'a CurseForge>,
}

/// Takes the private files out of `image`, naming them in its excludes. Returns their
/// paths.
fn strip_private(image: &mut Image) -> Vec<String> {
    let patterns: Vec<String> = PRIVATE_FILES.iter().map(|p| p.to_string()).collect();
    let excludes = Excludes::new(&patterns);
    let mut left_out = Vec::new();
    image.entries.retain(|entry| {
        let private = excludes.excludes(entry.path.as_str(), false);
        if private {
            left_out.push(entry.path.to_string());
        }
        !private
    });
    for pattern in patterns {
        if !image.excludes.contains(&pattern) {
            image.excludes.push(pattern);
        }
    }
    image.normalize();
    left_out
}

impl Images {
    /// Writes the snapshot `id` of `instance` as an image export at `to` (see the module):
    /// written to `<to>.part` and renamed when whole. Blocking.
    pub fn export_snapshot(
        &self,
        instance: &str,
        id: &str,
        to: &Path,
        cancel: Cancel,
    ) -> Result<ExportReport, ImageError> {
        let (record, mut image, mut content) = {
            let slot = self.slot(instance);
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            let record = self
                .reconciled_locked(instance)?
                .into_iter()
                .find(|record| record.id == id)
                .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))?;
            let image = self.store().load_manifest(instance, &record.image)?;
            let content = match image.content.as_deref() {
                Some(link) if image.kind == Kind::Data => {
                    Some(self.store().load_manifest(instance, link)?)
                }
                _ => None,
            };
            (record, image, content)
        };
        let mut left_out = strip_private(&mut image);
        if let Some(content) = &mut content {
            left_out.extend(strip_private(content));
            content.seal();
            content.validate().map_err(|e| {
                ImageError::Invalid(format!("The exported content image isn't valid: {e}"))
            })?;
            image.content = Some(content.id.clone());
        }
        image.label = Some(record.name.clone());
        image.parent = None;
        image.seal();
        image
            .validate()
            .map_err(|e| ImageError::Invalid(format!("The exported image isn't valid: {e}")))?;
        let linked: Vec<Image> = content.into_iter().collect();
        let references = image
            .entries
            .iter()
            .chain(linked.iter().flat_map(|c| c.entries.iter()))
            .filter(|e| !matches!(e.source, Source::Object {}))
            .count();
        let mut not_downloadable: Vec<String> = image
            .entries
            .iter()
            .chain(linked.iter().flat_map(|c| c.entries.iter()))
            .filter(|e| {
                matches!(
                    e.source,
                    Source::Curseforge {
                        downloadable: false,
                        ..
                    }
                )
            })
            .map(|e| e.path.to_string())
            .collect();
        not_downloadable.sort();
        not_downloadable.dedup();
        let report = {
            let _writing = self.enter()?;
            let _hold = self.store().hold_off_collection()?;
            if let Some(dir) = to.parent() {
                std::fs::create_dir_all(dir).map_err(ImageError::io(format!(
                    "Failed to create {}",
                    dir.display()
                )))?;
            }
            container::export(self.store(), &image, &linked, to, cancel).map_err(container_error)?
        };
        left_out.sort();
        left_out.dedup();
        Ok(ExportReport {
            file: to.to_path_buf(),
            bytes: std::fs::metadata(to).map_or(0, |m| m.len()),
            objects: report.objects,
            references,
            not_downloadable,
            left_out,
        })
    }

    /// Keeps an imported image (and the content it links to) as a snapshot of the new
    /// instance `instance` ([`SnapshotKind::Imported`]), so the user can go back to what
    /// they were sent. The content first, then the image, then the record.
    fn adopt_imported(
        &self,
        instance: &str,
        image: &Image,
        linked: &[Image],
    ) -> Result<SnapshotRecord, ImageError> {
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        for content in linked {
            self.store().save_manifest(instance, content)?;
        }
        self.store().save_manifest(instance, image)?;
        let mut records = self.reconciled_locked(instance)?;
        let record = SnapshotRecord {
            id: new_record_id(chrono::Utc::now().timestamp(), &records),
            image: image.id.clone(),
            kind: SnapshotKind::Imported,
            name: snapshot_name(image.label.as_deref()),
            created: chrono::Utc::now().timestamp(),
            pinned: false,
            full: image.kind == Kind::Full,
            worlds: image.worlds.iter().map(|w| w.name.clone()).collect(),
            world_details: super::snapshots::record_worlds(image),
            content: image.content.clone(),
            files: image.entries.len(),
            withheld: image.withheld.len(),
            bytes: container::files_size(image),
            new_bytes: 0,
        };
        records.retain(|r| r.image != image.id);
        records.insert(0, record.clone());
        newest_first(&mut records);
        self.save_records(instance, &records)?;
        Ok(record)
    }
}

fn container_error(e: container::ContainerError) -> ImageError {
    match e {
        container::ContainerError::Cancelled => ImageError::Cancelled,
        container::ContainerError::Store(e) => ImageError::Store(e),
        other => ImageError::Invalid(other.to_string()),
    }
}

/// Every file an export places: its image's and its content link's (the image wins at a
/// path both have).
fn placed_entries(image: &Image, linked: &[Image]) -> Vec<Entry> {
    let mut by_path = std::collections::BTreeMap::new();
    for entry in linked.iter().flat_map(|c| c.entries.iter()) {
        by_path.insert(entry.path.to_string(), entry.clone());
    }
    for entry in &image.entries {
        by_path.insert(entry.path.to_string(), entry.clone());
    }
    by_path.into_values().collect()
}

/// Refuses entries an import must never place: Lodestone's own names, and paths the client
/// layout can't place.
fn check_entries(entries: &[Entry]) -> Result<(), ImageError> {
    for entry in entries {
        if entry.path.components().any(is_lodestone_name) {
            return Err(ImageError::Invalid(format!(
                "the export has {}, a name Lodestone keeps for its own files",
                lodestone_image::display_text(entry.path.as_str())
            )));
        }
        if let Err(e) = DiskLayout::Client.to_disk(&entry.path) {
            return Err(ImageError::Invalid(format!(
                "the export has {}, which an instance can't hold: {e}",
                lodestone_image::display_text(entry.path.as_str())
            )));
        }
    }
    Ok(())
}

/// Whether `entry` can be downloaded: a Modrinth file, or a CurseForge one its author
/// allows (with a client to ask).
fn downloadable(entry: &Entry, curseforge: bool) -> bool {
    match &entry.source {
        Source::Modrinth { .. } => entry.sha512.is_some(),
        Source::Curseforge { downloadable, .. } => *downloadable && curseforge,
        _ => false,
    }
}

/// Whether the file at `path` is one a loader loads: anything under `mods/` at any depth
/// (Quilt loads subfolders, old Forge `mods/<version>/`) or `coremods/` (old Forge).
fn is_mod_path(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    let first = parts.next().unwrap_or_default();
    parts.next().is_some()
        && (first.eq_ignore_ascii_case(ContentKind::Mod.dir())
            || first.eq_ignore_ascii_case("coremods"))
}

/// How a mod file is named to the user: its path under `mods/` (`sodium.jar`,
/// `1.21.1/x.jar`), else its whole path, as when on.
fn mod_name(path: &CanonicalPath) -> String {
    let text = path.as_str();
    let under = text
        .split_once('/')
        .filter(|(first, _)| first.eq_ignore_ascii_case(ContentKind::Mod.dir()))
        .map_or(text, |(_, rest)| rest);
    enabled_name(under)
}

/// Everything an import of `entries` writes, as [`import_file`] asks the free-space check
/// for it and [`ImportPreview::needed_bytes`] shows it: the objects it unpacks into the
/// store (`unpacked`), the files once placed, what it may download (every Modrinth file
/// and each CurseForge one its author allows), and 1 GiB to spare.
fn space_needed(unpacked: u64, entries: &[Entry]) -> u64 {
    let files = entries.iter().map(|e| e.size).fold(0, u64::saturating_add);
    let downloads = entries
        .iter()
        .filter(|e| downloadable(e, true))
        .map(|e| e.size)
        .fold(0, u64::saturating_add);
    unpacked
        .saturating_add(files)
        .saturating_add(downloads)
        .saturating_add(SPARE_BYTES)
}

/// What the export at `path` holds (see [`ImportPreview`]), after the container's checks
/// and [`check_entries`]. Blocking.
pub fn inspect(path: &Path) -> Result<ImportPreview, ImageError> {
    let container = container::open(path, &Limits::default()).map_err(container_error)?;
    let image = container.image();
    let linked = container.linked();
    let entries = placed_entries(image, linked);
    check_entries(&entries)?;
    let shown = |text: &str| lodestone_image::display_text(text).into_owned();
    let mods = entries
        .iter()
        .filter(|e| is_mod_path(&e.path))
        .map(|e| shown(&mod_name(&e.path)))
        .collect();
    let carried_mods = entries
        .iter()
        .filter(|e| is_mod_path(&e.path) && matches!(e.source, Source::Object {}))
        .map(|e| shown(&mod_name(&e.path)))
        .collect();
    let download_bytes = entries
        .iter()
        .filter(|e| downloadable(e, true))
        .map(|e| e.size)
        .fold(0, u64::saturating_add);
    let not_downloadable = entries
        .iter()
        .filter(|e| {
            matches!(
                e.source,
                Source::Curseforge {
                    downloadable: false,
                    ..
                }
            )
        })
        .map(|e| shown(e.path.as_str()))
        .collect();
    Ok(ImportPreview {
        image: image.id.clone(),
        name_shown: shown(image.label.as_deref().unwrap_or(labels::SNAPSHOT)),
        minecraft: image.game.minecraft.clone(),
        loader: image.game.loader.as_str().to_string(),
        loader_version: image.game.loader_version.clone(),
        created: image.created,
        producer_shown: shown(&image.producer.version),
        worlds: image
            .worlds
            .iter()
            .map(|w| shown(w.level_name.as_deref().unwrap_or(&w.name)))
            .collect(),
        mods,
        carried_mods,
        files: entries.len(),
        unpacked_bytes: container.unpacked_bytes(),
        files_bytes: entries.iter().map(|e| e.size).fold(0, u64::saturating_add),
        download_bytes,
        needed_bytes: space_needed(container.unpacked_bytes(), &entries),
        not_downloadable,
        full: image.kind == Kind::Full,
    })
}

/// A free-space check for [`import_file`] on `dir`'s disk: refuses with
/// [`ImageError::NoRoom`] when it has less room than asked.
pub fn room_on(dir: PathBuf) -> impl Fn(u64) -> Result<(), ImageError> + Send + Sync {
    move |bytes| match crate::disk::free_space(&dir) {
        Some(free) if bytes > free => Err(ImageError::NoRoom {
            needed: bytes,
            free,
        }),
        _ => Ok(()),
    }
}

/// Makes a new instance from the export at `path`, named `name` (else the snapshot's
/// name, made unique), see the module. With `expect_image`, the image id the preview the
/// user approved showed: a file whose image is another by now (replaced since) is
/// refused. `check_room` is asked once, before anything is unpacked, for everything the
/// import will write and 1 GiB to spare ([`ImportPreview::needed_bytes`]). Blocks while
/// it reads, unpacks and places files: run it on a background thread.
#[allow(clippy::too_many_arguments)]
pub async fn import_file(
    images: &Arc<Images>,
    sources: Sources<'_>,
    path: &Path,
    name: Option<&str>,
    expect_image: Option<&str>,
    check_room: &(dyn Fn(u64) -> Result<(), ImageError> + Sync),
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Imported, ImageError> {
    let mut opened = container::open(path, &Limits::default()).map_err(container_error)?;
    let image = opened.image().clone();
    if expect_image.is_some_and(|expected| expected != image.id) {
        return Err(ImageError::Invalid(
            "The file changed since it was opened; open it again to see what it holds".into(),
        ));
    }
    let linked = opened.linked().to_vec();
    let entries = placed_entries(&image, &linked);
    check_entries(&entries)?;
    let store = images.store().clone();
    check_room(space_needed(opened.unpacked_bytes(), &entries))?;

    let writing = images.enter()?;
    let hold = store.hold_off_collection()?;
    let stop = || cancel.is_cancelled();
    opened
        .import_objects(&store, &|_| Ok(()), &stop)
        .map_err(container_error)?;
    drop(opened);
    let staging = images
        .instances()
        .new_staging()
        .map_err(|e| ImageError::Instance(e.to_string()))?;
    let built = build(
        images, &sources, &store, &image, &entries, &staging, name, progress, cancel,
    )
    .await;
    drop(hold);
    drop(writing);
    let (instance, left_out) = match built {
        Ok(made) => made,
        Err(e) => {
            let _ = super::remove_tree(&staging);
            return Err(e);
        }
    };
    if let Err(e) = images.record_now(&instance.id, labels::FROM_SNAPSHOT) {
        super::warn("images.record", &instance.id, &e.to_string());
    }
    if let Err(e) = images.adopt_imported(&instance.id, &image, &linked) {
        super::warn("images.import", &instance.id, &e.to_string());
    }
    Ok(Imported { instance, left_out })
}

/// How many downloads an import runs at once.
const IMPORT_DOWNLOADS: usize = 8;

/// SHA-512 of `reader`'s bytes, read in pieces.
fn sha512_of(mut reader: impl std::io::Read) -> std::io::Result<String> {
    let mut hasher = sha2::Sha512::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// A writer that only hashes, for checking a stored object's SHA-512.
struct Sha512Writer(sha2::Sha512);

impl std::io::Write for Sha512Writer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The URLs of the entries the store lacks, by index in `missing`; those that can't be
/// downloaded are left out. CurseForge files are looked up in one request.
async fn download_urls(sources: &Sources<'_>, missing: &[&Entry]) -> Vec<Option<String>> {
    let wanted: Vec<u64> = missing
        .iter()
        .filter_map(|e| match &e.source {
            Source::Curseforge {
                file,
                downloadable: true,
                ..
            } => Some(*file),
            _ => None,
        })
        .collect();
    let mut curseforge_urls = std::collections::HashMap::new();
    if let (Some(curseforge), false) = (sources.curseforge, wanted.is_empty()) {
        match curseforge.files_by_id(&wanted).await {
            Ok(files) => {
                for file in files {
                    if let Ok(url) = curseforge.download_url(&file) {
                        curseforge_urls.insert((file.mod_id, file.id), url);
                    }
                }
            }
            Err(e) => super::warn("images.import", "", &e.to_string()),
        }
    }
    missing
        .iter()
        .map(|entry| match &entry.source {
            Source::Modrinth { url, .. } if entry.sha512.is_some() => {
                sources.modrinth.cdn_url(url).ok()
            }
            Source::Curseforge {
                project,
                file,
                downloadable: true,
            } => curseforge_urls.get(&(*project, *file)).cloned(),
            _ => None,
        })
        .collect()
}

/// Downloads `entry` from `url` into the store (through `temp`), checking its size,
/// SHA-256 and, when the export names one, SHA-512.
async fn download_into_store(
    sources: &Sources<'_>,
    store: &lodestone_image::store::Store,
    entry: &Entry,
    url: String,
    temp: PathBuf,
    cancel: &CancelToken,
) -> Result<(), ImageError> {
    let shown = || lodestone_image::display_text(entry.path.as_str()).into_owned();
    let item = DownloadItem {
        url,
        dest: temp.clone(),
        sha1: entry.sha1.clone().unwrap_or_default(),
        size: Some(entry.size),
        executable: false,
    };
    sources
        .downloader
        .fetch_sha256(
            &item,
            &entry.sha256,
            Some(entry.size),
            &Progress::default(),
            cancel,
        )
        .await
        .map_err(|e| ImageError::Invalid(format!("{} couldn't be downloaded: {e}", shown())))?;
    let stored = (|| -> Result<(), ImageError> {
        if let Some(sha512) = &entry.sha512 {
            let file =
                std::fs::File::open(&temp).map_err(ImageError::io("Failed to read a download"))?;
            let found = sha512_of(std::io::BufReader::new(file))
                .map_err(ImageError::io("Failed to read a download"))?;
            if !found.eq_ignore_ascii_case(sha512) {
                return Err(ImageError::Invalid(format!(
                    "{} didn't download as the export says (SHA-512)",
                    shown()
                )));
            }
        }
        let put = store.put_file(&temp, NEVER)?;
        if put.sha256 != entry.sha256 || put.size != entry.size {
            return Err(ImageError::Invalid(format!(
                "{} didn't download as the export says (SHA-256)",
                shown()
            )));
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(&temp);
    stored
}

/// Checks that the stored bytes of a Modrinth or CurseForge `entry` that wasn't downloaded
/// in this import (the store had them: the export carried the same bytes under another
/// path, or the Launcher stored them before) are the ones its SHA-512 names; one without a
/// SHA-512 is refused.
fn check_reference(store: &lodestone_image::store::Store, entry: &Entry) -> Result<(), ImageError> {
    let shown = || lodestone_image::display_text(entry.path.as_str()).into_owned();
    let Some(sha512) = &entry.sha512 else {
        return Err(ImageError::Invalid(format!(
            "the export names {} as a download without its SHA-512",
            shown()
        )));
    };
    let mut hasher = Sha512Writer(sha2::Sha512::new());
    store.read_object(&entry.sha256, entry.size, &mut hasher, NEVER)?;
    let found = format!("{:x}", hasher.0.finalize());
    if !found.eq_ignore_ascii_case(sha512) {
        return Err(ImageError::Invalid(format!(
            "the export says {} is a download, but the bytes it carries for it are another file",
            shown()
        )));
    }
    Ok(())
}

/// The import's downloads, files and `instance.json` in `staging`, committed as a new
/// instance. Returns it, with the files left out.
#[allow(clippy::too_many_arguments)]
async fn build(
    images: &Arc<Images>,
    sources: &Sources<'_>,
    store: &lodestone_image::store::Store,
    image: &Image,
    entries: &[Entry],
    staging: &Path,
    name: Option<&str>,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<(Instance, Vec<String>), ImageError> {
    use futures::TryStreamExt as _;
    let mut left_out = Vec::new();
    let missing: Vec<&Entry> = entries
        .iter()
        .filter(|e| !store.object_complete(&e.sha256, e.size))
        .collect();
    let urls = download_urls(sources, &missing).await;
    let mut downloads = Vec::new();
    for (n, (entry, url)) in missing.iter().zip(urls).enumerate() {
        match url {
            Some(url) => downloads.push((n, *entry, url)),
            None => left_out.push(entry.path.to_string()),
        }
    }
    progress.reset(
        downloads.len() as u64,
        downloads
            .iter()
            .map(|(_, e, _)| e.size)
            .fold(0, u64::saturating_add),
    );
    futures::stream::iter(downloads.into_iter().map(Ok::<_, ImageError>))
        .try_for_each_concurrent(IMPORT_DOWNLOADS, |(n, entry, url)| async move {
            if cancel.is_cancelled() {
                return Err(ImageError::Cancelled);
            }
            let temp = staging.join(format!(".lodestone-download-{n}"));
            download_into_store(sources, store, entry, url, temp, cancel).await?;
            progress.add_bytes(entry.size);
            progress.file_done();
            Ok(())
        })
        .await?;
    let downloaded: std::collections::HashSet<&str> = missing
        .iter()
        .filter(|e| !left_out.contains(&e.path.to_string()))
        .map(|e| e.sha256.as_str())
        .collect();
    // A download entry not downloaded now: the store had its bytes already, from the
    // export (carried under another path) or from anywhere else. Those bytes must be the
    // ones its SHA-512 names, or the file would pass for that project's in content.json.
    for entry in entries {
        if matches!(entry.source, Source::Object {})
            || downloaded.contains(entry.sha256.as_str())
            || !store.object_complete(&entry.sha256, entry.size)
        {
            continue;
        }
        check_reference(store, entry)?;
    }
    let mut placed = Image::new(Kind::Full, image.game.clone());
    placed.entries = entries
        .iter()
        .filter(|e| store.object_complete(&e.sha256, e.size))
        .cloned()
        .collect();
    let stop = || cancel.is_cancelled();
    let options = MaterializeOptions {
        selection: Selection::All,
        durable: true,
        cancel: &stop,
    };
    materialize(store, &placed, &DiskLayout::Client, staging, &options)
        .map_err(|e| ImageError::Invalid(format!("The export's files couldn't be placed: {e}")))?;
    super::restore::learn_sources(&InstanceDirs::new(staging), &placed.entries)
        .map_err(ImageError::Invalid)?;
    let game = &image.game;
    let loader = super::restore::loader_of(&game.loader).ok_or_else(|| {
        ImageError::Invalid(format!(
            "The export is for {}, which the Launcher can't run",
            lodestone_image::display_text(game.loader.as_str())
        ))
    })?;
    let label = image.label.as_deref().unwrap_or(labels::SNAPSHOT);
    let wanted = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map_or_else(|| snapshot_name(Some(label)), str::to_string);
    let instance = Instance {
        name: wanted.chars().take(64).collect(),
        game_version: game.minecraft.clone(),
        loader,
        loader_version: game.loader_version.clone(),
        recommended_memory: image
            .settings
            .client
            .as_ref()
            .and_then(|c| c.recommended_memory_mb),
        created: instances::now(),
        ..Instance::default()
    };
    let made = images
        .instances()
        .commit_staged(staging, instance)
        .map_err(|e| ImageError::Instance(e.to_string()))?;
    Ok((made, left_out))
}
