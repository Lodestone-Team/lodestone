//! Sharing a snapshot as a file: an image export (`docs/instance-images.md`, "Sharing"),
//! and making a new instance from one.
//!
//! **Exporting** ([`Images::export_snapshot`]) writes the shared crate's export container
//! (`lodestone_image::container`): `image.json` (the snapshot's data or full image), the
//! content image a data image links to, and `objects/` with only the files whose source is
//! `object`. Mods and packs from Modrinth or CurseForge travel as references, not jars,
//! so an export of a modded instance is mostly its worlds and configs. Files that must
//! never leave the computer ([`PRIVATE_FILES`]: the commands the player typed, a
//! server's `/login` password among them, and the account files of the official launcher
//! and of account switchers) are taken
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

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use lodestone_image::container::{self, Limits};
use lodestone_image::paths::{in_names, same_name};
use lodestone_image::store::excludes::Excludes;
use lodestone_image::store::materialize::{MaterializeOptions, Selection, materialize};
use lodestone_image::store::{Cancel, NEVER};
use lodestone_image::{CanonicalPath, DiskLayout, Entry, Image, Kind, Source};
use serde::Serialize;
use sha2::Digest as _;

use super::labels;
use super::recorder::SPARE_BYTES;
use super::scope::{is_crash_name, is_lodestone_name};
use super::snapshots::{SnapshotKind, SnapshotRecord, new_record_id, newest_first, snapshot_name};
use super::{ImageError, Images};
use crate::content::{ContentKind, CurseForge, InstanceDirs, Modrinth, enabled_name};
use crate::game::download::{CancelToken, DownloadItem, Downloader, Progress};
use crate::instances::{self, Instance};

/// An image export's file extension.
pub const EXTENSION: &str = "lodeimg";

/// Files an export never carries, as image exclude patterns (a name at any depth, a path
/// with `/` from the game folder): the commands the player typed (passwords given to a
/// server's `/login` among them), the saved hotbars, the account files of account
/// switchers that keep sign-ins in the game folder (In-Game Account Switcher's
/// `ias.json`, Essential's account files), and the official launcher's account files at
/// the top of the game folder (a `.minecraft` folder pasted into an instance has them;
/// `strip_private` matches them in any case, as `import::copy` does).
pub const PRIVATE_FILES: &[&str] = &[
    "command_history.txt",
    "hotbar.nbt",
    "ias.json",
    "essential/microsoft_accounts.json",
    "essential/mojang_accounts.json",
    "/launcher_profiles.json",
    "/launcher_accounts*.json",
    "/launcher_msa_credentials*.bin",
    // The JVM's crash logs and dumps, which snapshots leave out: a crash log holds the
    // command line, access token included. `strip_private` also takes out every top-level
    // name `scope::is_crash_name` matches (any case), as an image from elsewhere may hold
    // one.
    "/hs_err_pid*",
    "/replay_pid*",
    "/java_pid*.hprof",
    "/*.mdmp",
    "/core",
    "/core.*",
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
    /// Files no download confirmed and whose bytes neither the file nor the store had, by
    /// path: left out of the instance.
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
        let top_level = !entry.path.as_str().contains('/');
        let top_level_secret = top_level
            && (is_crash_name(entry.path.as_str())
                || crate::import::copy::is_account_file(entry.path.as_str()));
        let private = top_level_secret || excludes.excludes(entry.path.as_str(), false);
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
            // The placed content can be exactly what they were sent: the instance's own
            // record of it ("Made from a snapshot") stays as it is.
            if self.store().load_header(instance, &content.id).is_err() {
                self.store().save_manifest(instance, content)?;
            }
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

/// Top-level folders an import only takes spelled with ASCII letters: on a disk that
/// ignores letter case, `modſ/` (a long s) is `mods/`, and a loader loads from it, while
/// a check comparing letters would miss it. Nothing the Launcher writes spells them
/// otherwise.
const FOLDERS_SPELLED_PLAINLY: [&str; 9] = [
    "mods",
    "coremods",
    "config",
    "defaultconfigs",
    "kubejs",
    "resourcepacks",
    "shaderpacks",
    "saves",
    "datapacks",
];

/// Refuses entries an import must never place: Lodestone's own names, top-level folders
/// spelled so a disk would take them for a content folder ([`FOLDERS_SPELLED_PLAINLY`]),
/// and paths the client layout can't place.
fn check_entries(entries: &[Entry]) -> Result<(), ImageError> {
    for entry in entries {
        if entry.path.components().any(is_lodestone_name) {
            return Err(ImageError::Invalid(format!(
                "the export has {}, a name Lodestone keeps for its own files",
                lodestone_image::display_text(entry.path.as_str())
            )));
        }
        let first = entry.path.components().next().unwrap_or_default();
        if !first.is_ascii() && in_names(first, &FOLDERS_SPELLED_PLAINLY) {
            return Err(ImageError::Invalid(format!(
                "the export has {}, a folder name a disk would take for another",
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

/// Refuses an export whose Minecraft or loader version isn't a plausible version id: the
/// sender wrote them, the import dialog shows them and the new instance keeps them, so
/// they mustn't hold prose or invisible characters (`1.21.1 (checked by Lodestone)`).
/// Letters, digits and ` .-_+`, as every Minecraft and loader version has them.
fn check_game(game: &lodestone_image::Game) -> Result<(), ImageError> {
    let plausible = |text: &str| {
        (1..=64).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" .-_+".contains(&b))
    };
    let versions = std::iter::once(game.minecraft.as_str()).chain(game.loader_version.as_deref());
    for version in versions {
        if !plausible(version) {
            return Err(ImageError::Invalid(format!(
                "the export names the version {}, which isn't one",
                lodestone_image::display_text(version)
            )));
        }
    }
    Ok(())
}

/// Whether `entry` can be downloaded: a Modrinth file with its SHA-512 whose project and
/// version are the ones its CDN URL names (as every export the Launcher writes has them,
/// so a file of another project never passes for the one named), or a CurseForge one its
/// author allows (with a client to ask).
fn downloadable(entry: &Entry, curseforge: bool) -> bool {
    match &entry.source {
        Source::Modrinth {
            url,
            project,
            version,
        } => entry.sha512.is_some() && names_modrinth_file(url, project, version),
        Source::Curseforge { downloadable, .. } => *downloadable && curseforge,
        _ => false,
    }
}

/// Whether `url` is the Modrinth CDN URL of a file of `project`'s `version`, written the
/// way the URL parser writes it (so no dot segments, backslashes or other spellings that
/// would download another path than the one checked): exactly
/// `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`, with base62 ids
/// as Modrinth's are, no query, and a file name without an encoded `.`, `/` or `\`.
fn names_modrinth_file(url: &str, project: &str, version: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let id = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric());
    let Some(segments) = parsed.path_segments() else {
        return false;
    };
    let segments: Vec<&str> = segments.collect();
    let file_ok = |file: &str| {
        let lower = file.to_ascii_lowercase();
        !file.is_empty() && !["%2e", "%2f", "%5c"].iter().any(|bad| lower.contains(bad))
    };
    parsed.as_str() == url
        && Source::modrinth_from_url(url).is_some()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && id(project)
        && id(version)
        && matches!(
            segments.as_slice(),
            ["data", p, "versions", v, file] if p == &project && v == &version && file_ok(file)
        )
}

/// An entry's file and claimed source, for telling apart the downloads an import made: two
/// entries with the same bytes but other sources are two downloads, so each claimed source
/// is checked by a download of its own.
fn download_key(entry: &Entry) -> (String, String) {
    (entry.sha256.clone(), format!("{:?}", entry.source))
}

/// The SHA-256 of every object the export carries (its `object` files, icons and
/// backgrounds).
fn carried(image: &Image, linked: &[Image]) -> Result<HashSet<String>, ImageError> {
    let images: Vec<&Image> = std::iter::once(image).chain(linked.iter()).collect();
    container::carried_objects(&images)
        .map(|objects| objects.into_keys().collect())
        .map_err(ImageError::Invalid)
}

/// The names, as [`mod_name`] gives them, of the mod files in `image` and `linked` (each
/// image's own entries, so a file a data image hides at its content link's path is
/// counted too, as a later restore of the content can place it) whose bytes the file
/// carries: they come from whoever made the export. A Modrinth or CurseForge entry counts
/// too, as a download of it that fails places the carried bytes (as a plain file); an
/// export the Launcher writes never carries those. Sorted, without repeats.
fn carried_mod_names(image: &Image, linked: &[Image], carried: &HashSet<String>) -> Vec<String> {
    let mut names: Vec<String> = std::iter::once(image)
        .chain(linked.iter())
        .flat_map(|i| i.entries.iter())
        .filter(|e| is_mod_path(&e.path) && carried.contains(&e.sha256))
        .map(|e| lodestone_image::display_text(&mod_name(&e.path)).into_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The withheld reason of a file an import couldn't download and had no bytes for.
pub const NOT_DOWNLOADED: &str = "not_downloaded";

/// `image` with every Modrinth, CurseForge or base source that no download in this import
/// confirmed (`verified`, by [`download_key`]) recorded as an `object` when `has_object`
/// says the store has its bytes: those came from the export or were in the store already,
/// and nothing says they are that project's file, so nothing may record them as one. One
/// whose bytes the store lacks (its download failed, and the export didn't carry it) is
/// withheld as [`NOT_DOWNLOADED`] instead: an `object` the store can't back can't be
/// saved, and a restore leaves a withheld file as it is.
fn without_unconfirmed_sources(
    image: &Image,
    verified: &HashSet<(String, String)>,
    has_object: &dyn Fn(&Entry) -> bool,
) -> Image {
    let mut image = image.clone();
    let mut not_downloaded = Vec::new();
    image.entries.retain_mut(|entry| {
        if matches!(entry.source, Source::Object {}) || verified.contains(&download_key(entry)) {
            return true;
        }
        if has_object(entry) {
            entry.source = Source::Object {};
            true
        } else {
            not_downloaded.push(lodestone_image::Withheld {
                path: entry.path.to_string(),
                reason: lodestone_image::WithheldReason::Other(NOT_DOWNLOADED.into()),
            });
            false
        }
    });
    image.withheld.extend(not_downloaded);
    image
}

/// Whether the file at `path` is one a loader loads: anything under `mods/` at any depth
/// (Quilt loads subfolders, old Forge `mods/<version>/`) or `coremods/` (old Forge), the
/// folder spelled any way a case-insensitive disk takes for it (`MODS/`, `modſ/`).
fn is_mod_path(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    let first = parts.next().unwrap_or_default();
    parts.next().is_some() && in_names(first, &[ContentKind::Mod.dir(), "coremods"])
}

/// How a mod file is named to the user: its path under `mods/` (`sodium.jar`,
/// `1.21.1/x.jar`), else its whole path, as when on.
fn mod_name(path: &CanonicalPath) -> String {
    let text = path.as_str();
    let under = text
        .split_once('/')
        .filter(|(first, _)| same_name(first, ContentKind::Mod.dir()))
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

/// The distinct downloads an import of `entries` may make (each file and claimed source
/// once), refused past [`MAX_DOWNLOADS`]: checked when the file is opened, before anything
/// is unpacked or asked of Modrinth or CurseForge.
fn wanted_downloads(entries: &[Entry], curseforge: bool) -> Result<Vec<&Entry>, ImageError> {
    let mut seen = HashSet::new();
    let wanted: Vec<&Entry> = entries
        .iter()
        .filter(|e| downloadable(e, curseforge))
        .filter(|e| seen.insert(download_key(e)))
        .collect();
    if wanted.len() > MAX_DOWNLOADS {
        return Err(ImageError::Invalid(format!(
            "the export names {} files to download, more than an instance has",
            wanted.len()
        )));
    }
    Ok(wanted)
}

/// What the export at `path` holds (see [`ImportPreview`]), after the container's checks
/// and [`check_entries`]. Blocking.
pub fn inspect(path: &Path) -> Result<ImportPreview, ImageError> {
    let container = container::open(path, &Limits::default()).map_err(container_error)?;
    let image = container.image();
    let linked = container.linked();
    let entries = placed_entries(image, linked);
    check_entries(&entries)?;
    check_game(&image.game)?;
    wanted_downloads(&entries, true)?;
    let shown = |text: &str| lodestone_image::display_text(text).into_owned();
    let mods = entries
        .iter()
        .filter(|e| is_mod_path(&e.path))
        .map(|e| shown(&mod_name(&e.path)))
        .collect();
    let carried_mods = carried_mod_names(image, linked, &carried(image, linked)?);
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
    check_game(&image.game)?;
    wanted_downloads(&entries, sources.curseforge.is_some())?;
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
    let Built {
        instance,
        left_out,
        verified,
    } = match built {
        Ok(made) => made,
        Err(e) => {
            let _ = super::remove_tree(&staging);
            return Err(e);
        }
    };
    // Collections stay held off until the imported snapshot's manifests are saved: until
    // then nothing names the objects only it uses (its worlds), and a long import may have
    // unpacked them longer ago than the collection's grace.
    if let Err(e) = images.record_now(&instance.id, labels::FROM_SNAPSHOT) {
        super::warn("images.record", &instance.id, &e.to_string());
    }
    // The snapshot kept of what they were sent claims no source this import didn't confirm,
    // so a later restore of it can't record the sender's file as a project's either.
    let has_object = |entry: &Entry| store.has_object(&entry.sha256, entry.size);
    let mut kept_linked: Vec<Image> = linked
        .iter()
        .map(|content| without_unconfirmed_sources(content, &verified, &has_object))
        .collect();
    let mut kept = without_unconfirmed_sources(&image, &verified, &has_object);
    for content in &mut kept_linked {
        content.seal();
    }
    if kept.kind == Kind::Data {
        kept.content = kept_linked.first().map(|c| c.id.clone());
    }
    kept.seal();
    if let Err(e) = images.adopt_imported(&instance.id, &kept, &kept_linked) {
        super::warn("images.import", &instance.id, &e.to_string());
    }
    drop(hold);
    drop(writing);
    Ok(Imported { instance, left_out })
}

/// How many downloads an import runs at once.
const IMPORT_DOWNLOADS: usize = 8;

/// The most downloads an import makes: far more files than any real instance has, so a
/// file listing a million references is refused before anything is asked of Modrinth or
/// CurseForge.
const MAX_DOWNLOADS: usize = 20_000;

/// How many CurseForge files one lookup asks for (CurseForge's own limit).
const CURSEFORGE_LOOKUP: usize = 500;

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

/// The URLs to download `wanted` from, by index; `None` for those that can't be
/// downloaded. CurseForge files are looked up in batches, each file once.
async fn download_urls(
    sources: &Sources<'_>,
    wanted: &[&Entry],
    cancel: &CancelToken,
) -> Result<Vec<Option<String>>, ImageError> {
    let mut ids: Vec<u64> = wanted
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
    ids.sort_unstable();
    ids.dedup();
    let mut curseforge_urls = std::collections::HashMap::new();
    if let Some(curseforge) = sources.curseforge {
        for chunk in ids.chunks(CURSEFORGE_LOOKUP) {
            if cancel.is_cancelled() {
                return Err(ImageError::Cancelled);
            }
            match curseforge.files_by_id(chunk).await {
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
    }
    Ok(wanted
        .iter()
        .map(|entry| match &entry.source {
            Source::Modrinth { url, .. } => sources.modrinth.cdn_url(url).ok(),
            Source::Curseforge { project, file, .. } => {
                curseforge_urls.get(&(*project, *file)).cloned()
            }
            _ => None,
        })
        .collect())
}

/// Why [`download_into_store`] didn't store a file.
enum Missed {
    /// It wasn't downloaded (the server refused, the network failed, or the bytes aren't
    /// the ones the export names): the entry stays unconfirmed, and the import goes on.
    /// `answered` is whether a server answered at all ([`DownloadError::Failed`]).
    ///
    /// [`DownloadError::Failed`]: crate::game::download::DownloadError::Failed
    NotDownloaded { problem: String, answered: bool },
    /// The import must stop: cancelled, or the store or the disk failed.
    Stop(ImageError),
}

/// Stops an import's downloads early. Once [`NO_ANSWER_IN_A_ROW`] downloads in a row got
/// no answer at all (no network, or a blocked host: each takes every retry, and a blocked
/// connection minutes), or one failed so the import must stop, no new download starts;
/// those count as not downloaded, so an import offline ends in moments with the files it
/// couldn't get listed, rather than after hours of retries.
#[derive(Default)]
struct Breaker {
    unanswered_in_a_row: AtomicUsize,
    /// No new download starts.
    tripped: AtomicBool,
    /// A download failed so that the import stops.
    stopped: AtomicBool,
}

/// See [`Breaker`]: as many as run at once, so a whole round of them failed.
const NO_ANSWER_IN_A_ROW: usize = IMPORT_DOWNLOADS;

impl Breaker {
    fn open(&self) -> bool {
        self.tripped.load(Ordering::Relaxed)
    }

    fn answered(&self) {
        self.unanswered_in_a_row.store(0, Ordering::Relaxed);
    }

    fn unanswered(&self) {
        if self.unanswered_in_a_row.fetch_add(1, Ordering::Relaxed) + 1 >= NO_ANSWER_IN_A_ROW {
            self.tripped.store(true, Ordering::Relaxed);
        }
    }

    fn trip(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.tripped.store(true, Ordering::Relaxed);
    }
}

/// Downloads `entry` from `url` into the store (through `temp`), checking its size,
/// SHA-256 and, when the export names one, SHA-512. The check and the store's write run on
/// a thread of their own, so the other downloads keep going meanwhile.
async fn download_into_store(
    sources: &Sources<'_>,
    store: &Arc<lodestone_image::store::Store>,
    entry: &Entry,
    url: String,
    temp: PathBuf,
    cancel: &CancelToken,
) -> Result<(), Missed> {
    use crate::game::download::DownloadError;
    let shown = lodestone_image::display_text(entry.path.as_str()).into_owned();
    let item = DownloadItem {
        url,
        dest: temp.clone(),
        sha1: entry.sha1.clone().unwrap_or_default(),
        size: Some(entry.size),
        executable: false,
    };
    let fetched = sources
        .downloader
        .fetch_sha256(
            &item,
            &entry.sha256,
            Some(entry.size),
            &Progress::default(),
            cancel,
        )
        .await;
    match fetched {
        Ok(()) => {}
        Err(DownloadError::Cancelled) => {
            let _ = std::fs::remove_file(&temp);
            return Err(Missed::Stop(ImageError::Cancelled));
        }
        Err(DownloadError::Io { path, source }) => {
            let _ = std::fs::remove_file(&temp);
            return Err(Missed::Stop(ImageError::io(format!(
                "Failed to write {}",
                path.display()
            ))(source)));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            let answered = !matches!(
                e,
                DownloadError::Failed {
                    answered: false,
                    ..
                }
            );
            return Err(Missed::NotDownloaded {
                problem: format!("{shown} couldn't be downloaded: {e}"),
                answered,
            });
        }
    }
    let (sender, receiver) = futures::channel::oneshot::channel();
    let (store, entry) = (store.clone(), entry.clone());
    // A thread per file, at most IMPORT_DOWNLOADS at once: launcher-core has no blocking
    // pool of its own, and hashing a jar takes far longer than starting a thread.
    std::thread::spawn(move || {
        let checked = check_and_store(&store, &entry, &temp, &shown);
        // Deleted before the result is sent: once every result is in, the staging folder
        // becomes the instance, and the download mustn't be left in it.
        let _ = std::fs::remove_file(&temp);
        let _ = sender.send(checked);
    });
    receiver.await.unwrap_or_else(|_| {
        Err(Missed::Stop(ImageError::Invalid(
            "A download's check stopped unexpectedly".into(),
        )))
    })
}

/// Checks the downloaded file at `temp` against `entry`'s SHA-512 (when it names one) and
/// puts it in the store, checking its SHA-256 and size. Blocking.
fn check_and_store(
    store: &lodestone_image::store::Store,
    entry: &Entry,
    temp: &Path,
    shown: &str,
) -> Result<(), Missed> {
    let read = |e| Missed::Stop(ImageError::io("Failed to read a download")(e));
    if let Some(sha512) = &entry.sha512 {
        let file = std::fs::File::open(temp).map_err(read)?;
        let found = sha512_of(std::io::BufReader::new(file)).map_err(read)?;
        if !found.eq_ignore_ascii_case(sha512) {
            return Err(Missed::NotDownloaded {
                problem: format!("{shown} didn't download as the export says (SHA-512)"),
                answered: true,
            });
        }
    }
    let put = store
        .put_file(temp, NEVER)
        .map_err(|e| Missed::Stop(e.into()))?;
    if put.sha256 != entry.sha256 || put.size != entry.size {
        // The store keeps the bytes it was given, which nothing names: a collection
        // removes them.
        return Err(Missed::NotDownloaded {
            problem: format!("{shown} didn't download as the export says (SHA-256)"),
            answered: true,
        });
    }
    Ok(())
}

/// What [`build`] made.
struct Built {
    instance: Instance,
    /// Files it couldn't get, by path.
    left_out: Vec<String>,
    /// The downloads it made, by [`download_key`]: the sources this import confirmed.
    verified: HashSet<(String, String)>,
}

/// The import's downloads, files and `instance.json` in `staging`, committed as a new
/// instance.
///
/// Every Modrinth and CurseForge file is downloaded from its source in this import, even
/// when the store has its bytes: the export's hashes came from the same file as its
/// sources, so only a download says that a project's file has those bytes. Each distinct
/// file and claimed source is downloaded once. A file that wasn't downloaded (no URL, a
/// download that failed, or bytes that aren't the ones named) is still placed when the
/// store has its bytes (the export carries them, or the store had them before), but as a
/// plain file: `content.json` learns no source for it. The rest are left out. Once the
/// downloads get no answer at all, the rest aren't tried ([`Breaker`]). Only cancelling,
/// or a failure of the store or the disk, stops the import.
#[allow(clippy::too_many_arguments)]
async fn build(
    images: &Arc<Images>,
    sources: &Sources<'_>,
    store: &Arc<lodestone_image::store::Store>,
    image: &Image,
    entries: &[Entry],
    staging: &Path,
    name: Option<&str>,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Built, ImageError> {
    use futures::StreamExt as _;
    let wanted = wanted_downloads(entries, sources.curseforge.is_some())?;
    let urls = download_urls(sources, &wanted, cancel).await?;
    // Owned, so the stream's closure takes no borrowed argument (its future must be Send
    // for any lifetime).
    let downloads: Vec<(usize, Entry, String)> = wanted
        .iter()
        .zip(urls)
        .enumerate()
        .filter_map(|(n, (entry, url))| url.map(|url| (n, (*entry).clone(), url)))
        .collect();
    progress.reset(
        downloads.len() as u64,
        downloads
            .iter()
            .map(|(_, e, _)| e.size)
            .fold(0, u64::saturating_add),
    );
    let breaker = Breaker::default();
    let breaker = &breaker;
    let results: Vec<Result<Option<(String, String)>, ImageError>> =
        futures::stream::iter(downloads)
            .map(|(n, entry, url)| async move {
                if cancel.is_cancelled() {
                    return Err(ImageError::Cancelled);
                }
                if breaker.open() {
                    // Not started: counted as not downloaded, or the import is stopping.
                    progress.add_bytes(entry.size);
                    progress.file_done();
                    return Ok(None);
                }
                let temp = staging.join(format!(".lodestone-download-{n}"));
                let stored = download_into_store(sources, store, &entry, url, temp, cancel).await;
                progress.add_bytes(entry.size);
                progress.file_done();
                match stored {
                    Ok(()) => {
                        breaker.answered();
                        Ok(Some(download_key(&entry)))
                    }
                    Err(Missed::NotDownloaded { problem, answered }) => {
                        super::warn("images.import", "", &problem);
                        if answered {
                            breaker.answered();
                        } else {
                            breaker.unanswered();
                        }
                        Ok(None)
                    }
                    Err(Missed::Stop(e)) => {
                        breaker.trip();
                        Err(e)
                    }
                }
            })
            .buffer_unordered(IMPORT_DOWNLOADS)
            .collect()
            .await;
    if breaker.open() && !breaker.stopped.load(Ordering::Relaxed) {
        super::warn(
            "images.import",
            "",
            "the downloads got no answer, so the rest weren't tried",
        );
    }
    // The downloads that succeeded: the sources this import confirmed. Cancelling wins
    // over another failure, so a cancelled import says so.
    let mut verified = HashSet::new();
    let mut stopped = None;
    for result in results {
        match result {
            Ok(Some(key)) => {
                verified.insert(key);
            }
            Ok(None) => {}
            Err(ImageError::Cancelled) => stopped = Some(ImageError::Cancelled),
            Err(e) => {
                if stopped.is_none() {
                    stopped = Some(e);
                }
            }
        }
    }
    if let Some(e) = stopped {
        return Err(e);
    }
    // Every unconfirmed file becomes a plain one here; those the store lacks are left out
    // just below.
    let mut placed = without_unconfirmed_sources(
        &Image {
            entries: entries.to_vec(),
            ..Image::new(Kind::Full, image.game.clone())
        },
        &verified,
        &|_| true,
    );
    let mut left_out = Vec::new();
    placed.entries.retain(|e| {
        let there = store.object_complete(&e.sha256, e.size);
        if !there {
            left_out.push(e.path.to_string());
        }
        there
    });
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
    let wanted_name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map_or_else(|| snapshot_name(Some(&shown_name(label))), str::to_string);
    let instance = Instance {
        name: wanted_name.chars().take(64).collect(),
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
    Ok(Built {
        instance: made,
        left_out,
        verified,
    })
}

/// A name the sender wrote, fit to name an instance: without control characters and the
/// invisible format characters (direction overrides, zero-width ones:
/// `lodestone_image::paths::is_default_ignorable`), which could make it show as another
/// name.
pub fn shown_name(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && !lodestone_image::paths::is_default_ignorable(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{check_game, names_modrinth_file};

    #[test]
    fn only_plausible_versions_are_taken() {
        let game = |minecraft: &str, loader: Option<&str>| lodestone_image::Game {
            minecraft: minecraft.into(),
            loader: lodestone_image::Loader::Fabric,
            loader_version: loader.map(str::to_string),
        };
        for ok in [
            game("1.21.1", Some("0.16.10")),
            game("24w14a", None),
            game("1.14 Pre-Release 2", Some("1.20.1-47.2.0")),
            game("b1.7.3", Some("0.26.0-beta.1+build.3")),
        ] {
            assert!(check_game(&ok).is_ok(), "{ok:?}");
        }
        for bad in [
            game("1.21.1 (checked by Lodestone: no mods)", None),
            game("1.21.1\u{202E}", None),
            game("", None),
            game("1.21.1", Some("0.16\n10")),
            game(&"1".repeat(65), None),
        ] {
            assert!(check_game(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_a_canonical_url_of_the_named_project_and_version_passes() {
        let named = |url: &str| names_modrinth_file(url, "AANobbMI", "v1");
        assert!(named(
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/sodium.jar"
        ));
        assert!(named(
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/sodium%20fabric.jar"
        ));
        for bad in [
            "https://cdn.modrinth.com/data/EVILPROJ/versions/x1/evil.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/../../../EVILPROJ/versions/x1/e.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/%2e%2e/%2E%2E/x.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/..\\..\\x.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/a%2Fb.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/a%5cb.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/x.jar?y=1",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/x.jar#y",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/a/b.jar",
            "https://cdn.modrinth.com/data/AANobbMI/versions/v1/",
            "https://CDN.modrinth.com/data/AANobbMI/versions/v1/x.jar",
            "https://evil.com/data/AANobbMI/versions/v1/x.jar",
        ] {
            assert!(!named(bad), "{bad}");
        }
        // Ids that aren't base62 never pass, even when the URL names them.
        assert!(!names_modrinth_file(
            "https://cdn.modrinth.com/data/A%2e/versions/v1/x.jar",
            "A%2e",
            "v1"
        ));
    }
}
