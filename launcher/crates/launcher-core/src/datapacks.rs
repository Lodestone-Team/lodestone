//! A world's data packs: `saves/<world>/datapacks/`, where the game finds `.zip` files
//! and folders with a `pack.mcmeta`.
//!
//! - **On and off** is the game's own setting, the `DataPacks` lists in the world's
//!   `level.dat` (what `/datapack enable` and the world's Data Packs screen change). A
//!   pack in neither list is new, and the game turns it on the next time it loads the
//!   world. `level.dat` is only rewritten when it reads and writes back to the same
//!   bytes, so nothing else in it can change.
//! - **Adding** copies a zip or folder in under its own name; a name that is taken is
//!   refused. Jars are refused: data packs that come as mods go on the Mods page.
//! - **Installing from Modrinth** downloads a version's `.zip` (SHA-1 and size checked
//!   while downloading, SHA-512 after) and remembers the project in
//!   `<instance>/datapacks.json`, so a newer version of the same project replaces the
//!   old file and keeps its place in the lists.
//! - **Deleting** moves the pack to the Trash and drops it from the lists.
//!
//! Nothing here checks whether the game is running; the app refuses changes then, since
//! the game rewrites `level.dat` when it saves the world.

use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::{
    hash,
    level::{self, DataPackLists},
    modrinth::{ModrinthVersionType, RawVersion, RawVersionFile},
    versions::compare_versions,
};
use serde::{Deserialize, Serialize};

use crate::{
    content::{
        ContentError, ContentKind, InstanceDirs, MAX_FILE_BYTES, Modrinth, is_plain_name,
        modrinth::sort_newest_first,
        packs::{Compatibility, FormatRange, PackFormat},
        safe_download_name,
        scan::{self, FileMeta},
    },
    game::download::{
        CancelToken, DownloadError, DownloadItem, Downloader, Progress, ProgressSnapshot,
    },
    storage,
    worlds::{WorldError, world_dir},
};

/// The folder in a world.
pub const DATAPACKS: &str = "datapacks";
/// The Modrinth loader data pack files are tagged with.
pub const MODRINTH_LOADER: &str = "datapack";
/// Where Modrinth installs are remembered, in the instance's folder.
const PROVENANCE_FILE: &str = "datapacks.json";

/// The data pack format of each Minecraft release: the first and last release that read
/// it. From the Minecraft Wiki's "Pack format" page (<https://minecraft.wiki/w/Pack_format>),
/// read 2026-11-10.
pub const DATA_FORMATS: &[(&str, &str, PackFormat)] = &[
    ("1.13", "1.14.4", PackFormat::new(4, 0)),
    ("1.15", "1.16.1", PackFormat::new(5, 0)),
    ("1.16.2", "1.16.5", PackFormat::new(6, 0)),
    ("1.17", "1.17.1", PackFormat::new(7, 0)),
    ("1.18", "1.18.1", PackFormat::new(8, 0)),
    ("1.18.2", "1.18.2", PackFormat::new(9, 0)),
    ("1.19", "1.19.3", PackFormat::new(10, 0)),
    ("1.19.4", "1.19.4", PackFormat::new(12, 0)),
    ("1.20", "1.20.1", PackFormat::new(15, 0)),
    ("1.20.2", "1.20.2", PackFormat::new(18, 0)),
    ("1.20.3", "1.20.4", PackFormat::new(26, 0)),
    ("1.20.5", "1.20.6", PackFormat::new(41, 0)),
    ("1.21", "1.21.1", PackFormat::new(48, 0)),
    ("1.21.2", "1.21.3", PackFormat::new(57, 0)),
    ("1.21.4", "1.21.4", PackFormat::new(61, 0)),
    ("1.21.5", "1.21.5", PackFormat::new(71, 0)),
    ("1.21.6", "1.21.6", PackFormat::new(80, 0)),
    ("1.21.7", "1.21.8", PackFormat::new(81, 0)),
    ("1.21.9", "1.21.10", PackFormat::new(88, 0)),
    ("1.21.11", "1.21.11", PackFormat::new(94, 1)),
    ("26.1", "26.1.2", PackFormat::new(101, 1)),
    ("26.2", "26.2", PackFormat::new(107, 1)),
    ("26.3", "26.3", PackFormat::new(121, 0)),
];

/// Whether `version` looks like a release (`1.20.1`, `26.3`), not a snapshot.
fn is_release(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// The data pack format `game_version` reads, if it is a release in the table.
pub fn data_format(game_version: &str) -> Option<PackFormat> {
    if !is_release(game_version) {
        return None;
    }
    DATA_FORMATS
        .iter()
        .find(|(first, last, _)| {
            compare_versions(game_version, first) != Ordering::Less
                && compare_versions(game_version, last) != Ordering::Greater
        })
        .map(|(_, _, format)| *format)
}

/// Whether a data pack with `formats` fits `game_version`, as the game's pack screen
/// says.
pub fn compatibility(formats: Option<&FormatRange>, game_version: &str) -> Compatibility {
    let (Some(formats), Some(game)) = (formats, data_format(game_version)) else {
        return Compatibility::Unknown;
    };
    if formats.max < game {
        Compatibility::TooOld
    } else if formats.min > game {
        Compatibility::TooNew
    } else {
        Compatibility::Compatible
    }
}

/// Whether a data pack loads, by the world's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackState {
    Enabled,
    Disabled,
    /// In neither list: the game turns it on the next time it loads the world.
    New,
}

impl PackState {
    /// Whether it loads (or will, once the game sees it).
    pub fn is_on(self) -> bool {
        !matches!(self, PackState::Disabled)
    }
}

/// One data pack in a world.
#[derive(Clone, Debug, PartialEq)]
pub struct DataPack {
    /// The file or folder name in `datapacks/`.
    pub name: String,
    pub is_dir: bool,
    /// Bytes, for zips; the files' bytes for folders.
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// What `pack.mcmeta` says, and why the game may not load it.
    pub meta: FileMeta,
    pub state: PackState,
    /// The Modrinth project it was installed from.
    pub modrinth_project: Option<String>,
}

impl DataPack {
    /// The id the game gives it in the world's lists.
    pub fn id(&self) -> String {
        DataPackLists::file_id(&self.name)
    }
}

/// A world's data packs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldDataPacks {
    pub packs: Vec<DataPack>,
    /// Whether `level.dat` has the lists, so packs can be turned on and off here. A world
    /// from before 1.13, or one the game hasn't saved since, has none.
    pub switchable: bool,
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> WorldError + '_ {
    move |source| WorldError::Io {
        path: path.to_owned(),
        source,
    }
}

fn content_error(err: ContentError) -> WorldError {
    match err {
        ContentError::Io { path, source } => WorldError::Io { path, source },
        ContentError::Download(DownloadError::Cancelled) => WorldError::Cancelled,
        ContentError::NotFound(message) => WorldError::NotFound(message),
        ContentError::Conflict(message) => WorldError::Conflict(message),
        other => WorldError::Invalid(other.to_string()),
    }
}

/// The world's `datapacks` folder (which may not exist yet).
pub fn dir(dirs: &InstanceDirs, world: &str) -> Result<PathBuf, WorldError> {
    Ok(world_dir(dirs, world)?.join(DATAPACKS))
}

/// Whether `name` (a `.zip` file or a folder) can be a data pack name.
pub fn is_pack_name(name: &str, is_dir: bool) -> bool {
    is_plain_name(name)
        && (is_dir || {
            let lower = name.to_ascii_lowercase();
            lower.ends_with(".zip") && name.len() > ".zip".len()
        })
}

/// What a zip or folder says about itself as a data pack: `pack.mcmeta`, `pack.png`, and
/// a problem when it has no `data` folder (a resource pack, say).
pub fn read_meta(path: &Path, is_dir: bool) -> FileMeta {
    let mut meta = scan::read_meta(ContentKind::ResourcePack, path, is_dir);
    if meta.problem.is_none() && !has_data_folder(path, is_dir) {
        meta.problem = Some(
            "It has no data folder, so it changes nothing in the world (is it a resource pack?)."
                .into(),
        );
    }
    meta
}

fn has_data_folder(path: &Path, is_dir: bool) -> bool {
    if is_dir {
        return path.join("data").is_dir();
    }
    File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(io::BufReader::new(file)).ok())
        .is_some_and(|archive| {
            archive
                .file_names()
                .any(|name| name.is_ok_and(|n| n.starts_with("data/")))
        })
}

fn read_lists(world: &Path) -> Option<DataPackLists> {
    let bytes = fs::read(world.join("level.dat")).ok()?;
    level::data_packs(&level::read_level(&bytes).ok()?)
}

fn folder_size(path: &Path) -> u64 {
    crate::worlds::folder_size(path)
}

/// The world's data packs, by name (ignoring case).
pub fn list(dirs: &InstanceDirs, world: &str) -> Result<WorldDataPacks, WorldError> {
    let world_path = world_dir(dirs, world)?;
    let lists = read_lists(&world_path);
    let provenance = Provenance::load(dirs);
    let folder = world_path.join(DATAPACKS);
    let mut packs = Vec::new();
    if let Ok(entries) = fs::read_dir(&folder) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let is_dir = file_type.is_dir();
            if file_type.is_symlink() || !(is_dir || file_type.is_file()) {
                continue;
            }
            if !is_pack_name(&name, is_dir) {
                continue;
            }
            let path = entry.path();
            let metadata = entry.metadata().ok();
            let id = DataPackLists::file_id(&name);
            let state = match &lists {
                Some(lists) if lists.disabled.contains(&id) => PackState::Disabled,
                Some(lists) if lists.enabled.contains(&id) => PackState::Enabled,
                _ => PackState::New,
            };
            packs.push(DataPack {
                size: if is_dir {
                    folder_size(&path)
                } else {
                    metadata.as_ref().map_or(0, |m| m.len())
                },
                modified: metadata.and_then(|m| m.modified().ok()),
                meta: read_meta(&path, is_dir),
                state,
                modrinth_project: provenance.get(world, &name).map(|e| e.project_id.clone()),
                name,
                is_dir,
            });
        }
    }
    packs.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(WorldDataPacks {
        packs,
        switchable: lists.is_some(),
    })
}

/// The path of the pack `name` in the world, if it is there.
fn existing(dirs: &InstanceDirs, world: &str, name: &str) -> Result<PathBuf, WorldError> {
    let path = dir(dirs, world)?.join(name);
    let missing = || WorldError::NotFound(format!("{name} isn't in this world's data packs."));
    let metadata = fs::symlink_metadata(&path).map_err(|_| missing())?;
    if metadata.file_type().is_symlink() || !is_pack_name(name, metadata.is_dir()) {
        return Err(missing());
    }
    Ok(path)
}

/// Rewrites the world's lists with `change`. Fails if the world has none, or if
/// `level.dat` can't be changed without touching anything else.
fn change_lists(
    dirs: &InstanceDirs,
    world: &str,
    change: impl FnOnce(&mut DataPackLists),
) -> Result<(), WorldError> {
    let level_dat = world_dir(dirs, world)?.join("level.dat");
    let bytes = fs::read(&level_dat).map_err(io_error(&level_dat))?;
    let root = level::read_level(&bytes)
        .map_err(|err| WorldError::Invalid(format!("level.dat can't be read: {err}")))?;
    let mut lists = level::data_packs(&root).ok_or_else(|| {
        WorldError::Invalid(
            "This world has no data pack list yet. Open it in Minecraft 1.13 or later once, \
             then data packs can be turned on and off here."
                .into(),
        )
    })?;
    change(&mut lists);
    let written = level::set_data_packs(&bytes, &lists).ok_or_else(|| {
        WorldError::Invalid(
            "level.dat can't be changed without changing other things in it, so it was left \
             alone. Use the world's Data Packs screen in the game instead."
                .into(),
        )
    })?;
    storage::write_atomic(&level_dat, &written).map_err(io_error(&level_dat))
}

/// Turns the pack `name` on (loading after the others) or off.
pub fn set_enabled(
    dirs: &InstanceDirs,
    world: &str,
    name: &str,
    enabled: bool,
) -> Result<(), WorldError> {
    existing(dirs, world, name)?;
    let id = DataPackLists::file_id(name);
    change_lists(dirs, world, |lists| lists.set_enabled(&id, enabled))
}

/// Checks that `path` can be added as a data pack, and returns the name it gets.
pub fn check_addition(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("The name isn't valid text.")?
        .to_string();
    if metadata.file_type().is_symlink() {
        return Err("Links can't be added; add the file itself.".into());
    }
    let is_dir = metadata.is_dir();
    if !is_dir && name.to_ascii_lowercase().ends_with(".jar") {
        return Err(
            "That's a mod. Data packs are .zip files or folders; one that comes as a mod goes on the Mods page."
                .into(),
        );
    }
    if !is_pack_name(&name, is_dir) {
        return Err("Data packs are .zip files or folders.".into());
    }
    if !is_dir && metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "It is larger than {} MB.",
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    if let Some(problem) = read_meta(path, is_dir).problem {
        return Err(problem);
    }
    Ok(name)
}

/// Copies `path` into the world's data packs under its own name, and returns it. Fails if
/// the name is taken. The copy is written under a temporary name and renamed into place.
pub fn add(dirs: &InstanceDirs, world: &str, path: &Path) -> Result<String, WorldError> {
    let name = check_addition(path).map_err(WorldError::Invalid)?;
    let folder = dir(dirs, world)?;
    fs::create_dir_all(&folder).map_err(io_error(&folder))?;
    let dest = folder.join(&name);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(WorldError::Conflict(format!(
            "{name} is already in this world's data packs."
        )));
    }
    if path.is_dir() {
        crate::content::files::copy_folder(path, &folder, &name).map_err(content_error)?;
    } else {
        let mut source = File::open(path).map_err(io_error(path))?;
        storage::write_atomic_with(&dest, |file| io::copy(&mut source, file).map(|_| ()))
            .map_err(io_error(&dest))?;
    }
    Ok(name)
}

/// Moves the pack `name` to the Trash, and drops it from the lists (when the world has
/// them and `level.dat` can be changed; the game drops missing packs itself anyway).
pub fn delete(dirs: &InstanceDirs, world: &str, name: &str) -> Result<(), WorldError> {
    delete_with(dirs, world, name, |path| {
        trash::delete(path).map_err(|e| e.to_string())
    })
}

/// Like [`delete`], with `remove` doing the removal (tests don't use the Trash).
pub fn delete_with(
    dirs: &InstanceDirs,
    world: &str,
    name: &str,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), WorldError> {
    let path = existing(dirs, world, name)?;
    remove(&path).map_err(|reason| WorldError::Trash {
        path: path.clone(),
        reason,
    })?;
    let id = DataPackLists::file_id(name);
    let _ = change_lists(dirs, world, |lists| {
        lists.enabled.retain(|p| p != &id);
        lists.disabled.retain(|p| p != &id);
    });
    let mut provenance = Provenance::load(dirs);
    if provenance.remove(world, name) {
        provenance.save(dirs).map_err(io_error(&dirs.instance))?;
    }
    Ok(())
}

// ---- Modrinth ----

/// The file of a Modrinth version that is a data pack: its primary file if that is a
/// zip, else its first zip.
pub fn version_file(version: &RawVersion) -> Option<&RawVersionFile> {
    let is_zip = |f: &&RawVersionFile| f.filename.to_ascii_lowercase().ends_with(".zip");
    version
        .primary_file()
        .filter(is_zip)
        .or_else(|| version.files.iter().find(is_zip))
}

/// Why `version` isn't a data pack for `game_version`, or `None` if it is one.
pub fn incompatibility(version: &RawVersion, game_version: &str) -> Option<String> {
    if !version.loaders.iter().any(|l| l == MODRINTH_LOADER) {
        return Some(format!(
            "{} {} isn't a data pack (it is for {})",
            version.name,
            version.version_number,
            version.loaders.join(", ")
        ));
    }
    if !version.game_versions.iter().any(|v| v == game_version) {
        let mut shown: Vec<&str> = version
            .game_versions
            .iter()
            .take(5)
            .map(String::as_str)
            .collect();
        if version.game_versions.len() > 5 {
            shown.push("…");
        }
        return Some(format!(
            "{} {} is for Minecraft {}, but this instance runs {game_version}",
            version.name,
            version.version_number,
            shown.join(", ")
        ));
    }
    if version_file(version).is_none() {
        return Some(format!(
            "{} {} has no .zip file to put in a world",
            version.name, version.version_number
        ));
    }
    None
}

/// The newest release among `versions` that is a data pack for `game_version`, or else
/// the newest such version.
pub fn pick_version(game_version: &str, mut versions: Vec<RawVersion>) -> Option<RawVersion> {
    versions.retain(|v| incompatibility(v, game_version).is_none());
    sort_newest_first(&mut versions);
    let release = versions
        .iter()
        .position(|v| v.version_type == ModrinthVersionType::Release);
    match release {
        Some(i) => Some(versions.swap_remove(i)),
        None => versions.into_iter().next(),
    }
}

/// What installing a Modrinth version did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// The file name in `datapacks/`.
    pub name: String,
    /// The older file of the same project it replaced.
    pub replaced: Option<String>,
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Downloads `version`'s zip into the world's data packs. An older file of the same
/// project is moved to the Trash once the new one is in place, and the new one takes its
/// place in the lists (so a pack turned off stays off).
pub async fn install(
    api: &Modrinth,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    world: &str,
    version: &RawVersion,
    progress: &Progress,
    cancel: &CancelToken,
) -> Result<Installed, WorldError> {
    let folder = dir(dirs, world)?;
    let file = version_file(version).ok_or_else(|| {
        WorldError::Invalid(format!(
            "{} {} has no .zip file to put in a world",
            version.name, version.version_number
        ))
    })?;
    if file.size > MAX_FILE_BYTES {
        return Err(WorldError::Invalid(format!(
            "{} is larger than {} MB",
            file.filename,
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let sha1 = file
        .hashes
        .sha1
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let sha512 = file
        .hashes
        .sha512
        .clone()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !is_hex(&sha1, 40) || !is_hex(&sha512, 128) {
        return Err(WorldError::Invalid(format!(
            "Modrinth sent no hashes for {}, so it can't be checked",
            file.filename
        )));
    }
    let name = safe_download_name(ContentKind::ResourcePack, &file.filename).ok_or_else(|| {
        WorldError::Invalid(format!(
            "{} isn't a file name that can be used",
            file.filename
        ))
    })?;
    let mut provenance = Provenance::load(dirs);
    let old = provenance
        .of_project(world, &version.project_id)
        .filter(|old| existing(dirs, world, old).is_ok());
    if old.as_deref() == Some(name.as_str())
        && provenance
            .get(world, &name)
            .is_some_and(|e| e.version_id == version.id)
    {
        return Ok(Installed {
            name,
            replaced: None,
        });
    }
    let dest = folder.join(&name);
    if old.as_deref() != Some(name.as_str()) && fs::symlink_metadata(&dest).is_ok() {
        return Err(WorldError::Conflict(format!(
            "{name} is already in this world's data packs."
        )));
    }

    fs::create_dir_all(&folder).map_err(io_error(&folder))?;
    let url = api
        .cdn_url(&file.url)
        .map_err(|err| content_error(err.into()))?;
    let staging = folder.join(format!(".lodestone-{sha1}.download"));
    let item = DownloadItem {
        url,
        dest: staging.clone(),
        sha1,
        size: Some(file.size),
        executable: false,
    };
    progress.set(ProgressSnapshot {
        files_total: 1,
        files_done: 0,
        bytes_total: file.size,
        bytes_done: 0,
    });
    if let Err(err) = downloader.fetch(&item, progress, cancel).await {
        let _ = fs::remove_file(&staging);
        return Err(content_error(err.into()));
    }
    let actual = hash::sha512_of(&staging).map_err(io_error(&staging))?;
    if !actual.eq_ignore_ascii_case(&sha512) {
        let _ = fs::remove_file(&staging);
        return Err(WorldError::Invalid(format!(
            "The download of {} is damaged: its SHA-512 hash doesn't match",
            file.filename
        )));
    }
    if let Some(problem) = read_meta(&staging, false).problem {
        let _ = fs::remove_file(&staging);
        return Err(WorldError::Invalid(format!("{}: {problem}", file.filename)));
    }
    crate::fsx::rename(&staging, &dest).map_err(io_error(&dest))?;

    // The new file takes the old one's place in the lists.
    let replaced = old.filter(|old| old != &name);
    if let Some(old) = &replaced {
        let (old_id, new_id) = (DataPackLists::file_id(old), DataPackLists::file_id(&name));
        let _ = change_lists(dirs, world, |lists| {
            for list in [&mut lists.enabled, &mut lists.disabled] {
                for id in list.iter_mut().filter(|id| **id == old_id) {
                    *id = new_id.clone();
                }
            }
        });
        let _ = existing(dirs, world, old).map(trash::delete);
        provenance.remove(world, old);
    }
    provenance.insert(
        world,
        &name,
        Entry {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
        },
    );
    provenance.save(dirs).map_err(io_error(&dirs.instance))?;
    Ok(Installed { name, replaced })
}

// ---- provenance ----

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    project_id: String,
    version_id: String,
}

/// `<instance>/datapacks.json`: world folder → file name → the Modrinth version it is.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Provenance {
    #[serde(default)]
    worlds: BTreeMap<String, BTreeMap<String, Entry>>,
}

impl Provenance {
    fn path(dirs: &InstanceDirs) -> PathBuf {
        dirs.instance.join(PROVENANCE_FILE)
    }

    fn load(dirs: &InstanceDirs) -> Self {
        fs::read(Self::path(dirs))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, dirs: &InstanceDirs) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        storage::write_atomic(&Self::path(dirs), &json)
    }

    fn get(&self, world: &str, name: &str) -> Option<&Entry> {
        self.worlds.get(world)?.get(name)
    }

    fn of_project(&self, world: &str, project: &str) -> Option<String> {
        self.worlds
            .get(world)?
            .iter()
            .find(|(_, e)| e.project_id == project)
            .map(|(name, _)| name.clone())
    }

    fn insert(&mut self, world: &str, name: &str, entry: Entry) {
        self.worlds
            .entry(world.to_string())
            .or_default()
            .insert(name.to_string(), entry);
    }

    fn remove(&mut self, world: &str, name: &str) -> bool {
        let Some(files) = self.worlds.get_mut(world) else {
            return false;
        };
        let removed = files.remove(name).is_some();
        if files.is_empty() {
            self.worlds.remove(world);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_of_releases() {
        assert_eq!(data_format("1.20.1"), Some(PackFormat::new(15, 0)));
        assert_eq!(data_format("1.21.1"), Some(PackFormat::new(48, 0)));
        assert_eq!(data_format("1.21.11"), Some(PackFormat::new(94, 1)));
        assert_eq!(data_format("26.3"), Some(PackFormat::new(121, 0)));
        assert_eq!(data_format("1.12.2"), None);
        assert_eq!(data_format("24w14a"), None);
        for pair in DATA_FORMATS.windows(2) {
            assert!(pair[0].2 < pair[1].2, "{pair:?}");
            assert_eq!(compare_versions(pair[0].1, pair[1].0), Ordering::Less);
        }
        let range = |min: u32, max: u32| FormatRange {
            min: PackFormat::new(min, 0),
            max: PackFormat::new(max, u32::MAX),
        };
        assert_eq!(
            compatibility(Some(&range(48, 48)), "1.21.1"),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(Some(&range(15, 15)), "1.21.1"),
            Compatibility::TooOld
        );
        assert_eq!(
            compatibility(Some(&range(57, 61)), "1.21.1"),
            Compatibility::TooNew
        );
        assert_eq!(compatibility(None, "1.21.1"), Compatibility::Unknown);
    }

    fn version(loaders: &[&str], game_versions: &[&str], files: &[&str]) -> RawVersion {
        serde_json::from_value(serde_json::json!({
            "id": "v", "project_id": "p", "name": "P", "version_number": "1.0",
            "version_type": "release", "loaders": loaders, "game_versions": game_versions,
            "files": files.iter().enumerate().map(|(i, f)| serde_json::json!({
                "url": format!("https://cdn.modrinth.com/{f}"), "filename": f,
                "primary": i == 0, "size": 10,
            })).collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    #[test]
    fn modrinth_versions() {
        let pack = version(&["datapack"], &["1.21.1"], &["pack.zip"]);
        assert_eq!(incompatibility(&pack, "1.21.1"), None);
        assert!(incompatibility(&pack, "1.20.1").unwrap().contains("1.21.1"));
        let mod_only = version(&["fabric"], &["1.21.1"], &["x.jar"]);
        assert!(incompatibility(&mod_only, "1.21.1").is_some());
        // a version that is both a mod and a data pack: the zip is used
        let both = version(&["fabric", "datapack"], &["1.21.1"], &["x.jar", "x.zip"]);
        assert_eq!(version_file(&both).unwrap().filename, "x.zip");
        let jar_only = version(&["datapack"], &["1.21.1"], &["x.jar"]);
        assert!(incompatibility(&jar_only, "1.21.1").is_some());
        assert_eq!(
            pick_version("1.21.1", vec![mod_only, pack.clone(), jar_only]).map(|v| v.id),
            Some(pack.id)
        );
    }
}
