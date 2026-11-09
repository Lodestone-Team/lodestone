//! Instance snapshots: an instance's files, all or some, kept so it can go back to
//! them, be shared as one file, or start a new instance.
//!
//! Space: files are stored once, by content, in a store shared by every instance
//! (`<data>/snapshots/objects/ab/<sha256>`). A mod jar in ten snapshots of five
//! instances is one object. On APFS an object is a copy-on-write clone of the file
//! (`clonefile`), so taking a snapshot costs almost no space until the instance's
//! files change; elsewhere it is a copy. Hard links are never used: the game edits
//! world files in place, which would change the snapshot too.
//!
//! A snapshot is a small manifest (`<data>/snapshots/<instance>/<id>.json`): which files,
//! by path, size, mode, time and SHA-256, in which [`Category`], plus the instance's
//! settings. Deleting a snapshot removes the objects nothing else uses.
//!
//! A `.lodesnap` file is a zip of the manifest and the objects it needs. It comes from
//! other people, so reading one is strict: known entry names only, checked paths,
//! size and ratio limits, and every object's hash verified.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    instances::{Instance, InstanceStore},
    paths::DataDir,
};

/// The manifest's format; a newer one is refused.
pub const SCHEMA: u32 = 1;
/// The file extension of an exported snapshot.
pub const EXTENSION: &str = "lodesnap";

/// Limits on a `.lodesnap` from someone else.
const MAX_ENTRIES: usize = 200_000;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
/// An entry that inflates more than this many times is refused (above 1 MB).
const MAX_RATIO: u64 = 200;

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("The snapshot file isn't valid: {0}")]
    Invalid(String),
    #[error("The snapshot was made by a newer Lodestone Launcher.")]
    TooNew,
    #[error("No snapshot {0}.")]
    NotFound(String),
    #[error("{0}")]
    Instance(String),
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> SnapshotError + '_ {
    move |source| SnapshotError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn invalid(reason: impl Into<String>) -> SnapshotError {
    SnapshotError::Invalid(reason.into())
}

/// What a file is, for choosing what a snapshot takes and what a revert puts back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Mods,
    ResourcePacks,
    ShaderPacks,
    /// `config/`, `options.txt`, `servers.dat` and the like.
    Configs,
    /// One entry per world, in [`Selection::worlds`].
    Worlds,
    Screenshots,
    /// `instance.json`'s settings, the icon and the Play background.
    Settings,
    /// Everything else in the game folder, except logs, crash reports and caches.
    Other,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Mods,
        Category::ResourcePacks,
        Category::ShaderPacks,
        Category::Configs,
        Category::Worlds,
        Category::Screenshots,
        Category::Settings,
        Category::Other,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Category::Mods => "mods",
            Category::ResourcePacks => "resource_packs",
            Category::ShaderPacks => "shader_packs",
            Category::Configs => "configs",
            Category::Worlds => "worlds",
            Category::Screenshots => "screenshots",
            Category::Settings => "settings",
            Category::Other => "other",
        }
    }

    pub fn from_id(id: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.id() == id)
    }
}

/// Files in the game folder that are never taken: rebuilt or not worth keeping.
const EXCLUDED: [&str; 8] = [
    "logs",
    "crash-reports",
    ".cache",
    ".fabric",
    ".mixin.out",
    "debug",
    "natives",
    ".lodestone-snapshot",
];
/// Config files at the top of the game folder.
const CONFIG_FILES: [&str; 6] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
    "usercache.json",
];
/// Files in the instance folder (next to `minecraft/`) that belong to the settings.
const SETTINGS_FILES: [&str; 4] = [
    "icon.png",
    "background.jpg",
    "backgrounds",
    "shared_options.json",
];
/// Files in the instance folder that go with the mods: where they came from.
const MOD_RECORDS: [&str; 2] = ["content.json", "modpack.json"];

/// The category of `relative` (a path under the instance folder, `/`-separated), and
/// for a world its folder name; `None` for files never taken.
pub fn classify(relative: &str) -> Option<(Category, Option<String>)> {
    let mut parts = relative.split('/');
    let first = parts.next()?;
    if first != "minecraft" {
        if MOD_RECORDS.contains(&first) {
            return Some((Category::Mods, None));
        }
        if SETTINGS_FILES.contains(&first) {
            return Some((Category::Settings, None));
        }
        // instance.json is kept in the manifest itself; natives and staging are not.
        return None;
    }
    let top = parts.next()?;
    if EXCLUDED.contains(&top) || top.starts_with(".lodestone") {
        return None;
    }
    let category = match top {
        "mods" => Category::Mods,
        "resourcepacks" => Category::ResourcePacks,
        "shaderpacks" => Category::ShaderPacks,
        "screenshots" => Category::Screenshots,
        "config" | "defaultconfigs" => Category::Configs,
        name if CONFIG_FILES.contains(&name) => Category::Configs,
        "saves" => {
            let world = parts.next()?;
            parts.next()?; // a file in saves/ itself isn't a world
            return Some((Category::Worlds, Some(world.to_string())));
        }
        _ => Category::Other,
    };
    Some((category, None))
}

/// The top-level entries (under the instance folder) a category owns, for a revert: the
/// whole entry is replaced. `Other` owns every top-level game-folder entry no other
/// category claims; see [`other_entries`].
fn owned_entries(category: Category, world: Option<&str>) -> Vec<String> {
    match category {
        Category::Mods => vec![
            "minecraft/mods".into(),
            "content.json".into(),
            "modpack.json".into(),
        ],
        Category::ResourcePacks => vec!["minecraft/resourcepacks".into()],
        Category::ShaderPacks => vec!["minecraft/shaderpacks".into()],
        Category::Screenshots => vec!["minecraft/screenshots".into()],
        Category::Configs => {
            let mut entries: Vec<String> =
                vec!["minecraft/config".into(), "minecraft/defaultconfigs".into()];
            entries.extend(CONFIG_FILES.iter().map(|f| format!("minecraft/{f}")));
            entries
        }
        Category::Worlds => world
            .map(|w| vec![format!("minecraft/saves/{w}")])
            .unwrap_or_default(),
        Category::Settings => SETTINGS_FILES.iter().map(|f| f.to_string()).collect(),
        Category::Other => Vec::new(),
    }
}

/// The game folder's top-level entries in [`Category::Other`] now.
fn other_entries(instance_dir: &Path) -> Vec<String> {
    let game = instance_dir.join("minecraft");
    let Ok(entries) = fs::read_dir(&game) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .map(|name| format!("minecraft/{name}"))
        // The top-level name decides the category, for files and folders alike.
        .filter(|relative| {
            matches!(
                classify(&format!("{relative}/x")),
                Some((Category::Other, _))
            )
        })
        .collect()
}

/// What a snapshot takes, or a revert puts back.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub categories: BTreeSet<Category>,
    /// The worlds, by folder name, when [`Category::Worlds`] is chosen. Empty means
    /// every world.
    #[serde(default)]
    pub worlds: BTreeSet<String>,
}

impl Selection {
    pub fn all() -> Self {
        Self {
            categories: Category::ALL.into_iter().collect(),
            worlds: BTreeSet::new(),
        }
    }

    pub fn takes(&self, category: Category, world: Option<&str>) -> bool {
        if !self.categories.contains(&category) {
            return false;
        }
        match (category, world) {
            (Category::Worlds, Some(world)) => {
                self.worlds.is_empty() || self.worlds.contains(world)
            }
            _ => true,
        }
    }
}

/// One file of a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Under the instance folder, `/`-separated (`minecraft/mods/sodium.jar`).
    pub path: String,
    pub category: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    pub size: u64,
    pub sha256: String,
    /// Unix permission bits.
    #[serde(default)]
    pub mode: u32,
    /// Seconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

/// A snapshot's manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    pub id: String,
    pub instance_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Seconds since the epoch.
    pub created: u64,
    pub launcher_version: String,
    /// The instance's settings when the snapshot was taken.
    pub instance: Instance,
    pub selection: Selection,
    pub files: Vec<FileEntry>,
    /// All the files' sizes added up.
    pub total_bytes: u64,
    /// Bytes the store didn't have yet when the snapshot was taken.
    pub new_bytes: u64,
    /// Taken by Lodestone before a revert, not by the user.
    #[serde(default)]
    pub automatic: bool,
}

impl Snapshot {
    /// Sizes and file counts by category (worlds by folder).
    pub fn summary(&self) -> Vec<Part> {
        summarize(
            self.files
                .iter()
                .map(|f| (f.category, f.world.clone(), f.size)),
        )
    }

    /// The jar and zip files in `mods/`: code that runs when the game starts.
    pub fn mods(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| f.category == Category::Mods && f.path.starts_with("minecraft/mods/"))
            .map(|f| f.path.trim_start_matches("minecraft/mods/").to_string())
            .collect()
    }
}

/// A category (or one world) and how big it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Part {
    pub category: Category,
    pub world: Option<String>,
    pub files: usize,
    pub bytes: u64,
}

fn summarize(files: impl Iterator<Item = (Category, Option<String>, u64)>) -> Vec<Part> {
    let mut parts: BTreeMap<(Category, Option<String>), (usize, u64)> = BTreeMap::new();
    for (category, world, size) in files {
        let part = parts.entry((category, world)).or_default();
        part.0 += 1;
        part.1 += size;
    }
    parts
        .into_iter()
        .map(|((category, world), (files, bytes))| Part {
            category,
            world,
            files,
            bytes,
        })
        .collect()
}

/// The snapshot store under the data directory.
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

/// How far a long job has got.
pub type Progress<'a> = &'a (dyn Fn(u64, u64) + Sync);

impl Store {
    pub fn new(data: &DataDir) -> Self {
        Self::at(data.root().join("snapshots"))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn objects(&self) -> PathBuf {
        self.root.join("objects")
    }

    fn object(&self, sha256: &str) -> PathBuf {
        self.objects().join(&sha256[..2]).join(sha256)
    }

    fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.root.join("instances").join(instance_id)
    }

    fn manifest_path(&self, instance_id: &str, id: &str) -> PathBuf {
        self.instance_dir(instance_id).join(format!("{id}.json"))
    }

    /// An instance's snapshots, newest first.
    pub fn list(&self, instance_id: &str) -> Vec<Snapshot> {
        let mut snapshots: Vec<Snapshot> = fs::read_dir(self.instance_dir(instance_id))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| read_manifest(&e.path()).ok())
            .collect();
        snapshots.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
        snapshots
    }

    pub fn get(&self, instance_id: &str, id: &str) -> Result<Snapshot, SnapshotError> {
        if !valid_id(id) || !valid_id(instance_id) {
            return Err(SnapshotError::NotFound(id.into()));
        }
        read_manifest(&self.manifest_path(instance_id, id))
            .map_err(|_| SnapshotError::NotFound(id.into()))
    }

    /// The store's size on disk as files: objects plus manifests. On APFS clones share
    /// blocks, so the space really used can be less.
    pub fn size(&self) -> u64 {
        crate::worlds::folder_size(&self.root)
    }

    /// Sizes of what a snapshot of the instance at `instance_dir` could take.
    pub fn plan(&self, instance_dir: &Path) -> Vec<Part> {
        let files = walk(instance_dir).unwrap_or_default();
        summarize(files.into_iter().map(|(_, rel, size)| {
            let (category, world) = classify(&rel).expect("walk only returns classified files");
            (category, world, size)
        }))
    }

    /// Takes a snapshot of `instance` (in `instance_dir`) with `selection`.
    pub fn create(
        &self,
        instance: &Instance,
        instance_dir: &Path,
        name: &str,
        note: Option<String>,
        selection: &Selection,
        automatic: bool,
        progress: Progress<'_>,
    ) -> Result<Snapshot, SnapshotError> {
        let files: Vec<(PathBuf, String, u64)> = walk(instance_dir)?
            .into_iter()
            .filter(|(_, rel, _)| {
                let (category, world) = classify(rel).expect("classified");
                selection.takes(category, world.as_deref())
            })
            .collect();
        let total: u64 = files.iter().map(|f| f.2).sum();
        let done = std::sync::atomic::AtomicU64::new(0);
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(files.len().max(1));
        let chunk = files.len().div_ceil(threads).max(1);
        let results: Vec<Result<(FileEntry, u64), SnapshotError>> = std::thread::scope(|scope| {
            let handles: Vec<_> = files
                .chunks(chunk)
                .map(|chunk| {
                    let done = &done;
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|(path, rel, size)| {
                                let entry = self.store_file(path, rel)?;
                                let n = done.fetch_add(*size, std::sync::atomic::Ordering::Relaxed);
                                progress(n + size, total);
                                Ok(entry)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        let mut entries = Vec::with_capacity(results.len());
        let mut new_bytes = 0;
        for result in results {
            let (entry, added) = result?;
            new_bytes += added;
            entries.push(entry);
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let created = crate::instances::now();
        let snapshot = Snapshot {
            schema: SCHEMA,
            id: new_id(created),
            instance_id: instance.id.clone(),
            name: name.trim().to_string(),
            note: note.filter(|n| !n.trim().is_empty()),
            created,
            launcher_version: env!("CARGO_PKG_VERSION").to_string(),
            instance: instance.clone(),
            selection: selection.clone(),
            total_bytes: entries.iter().map(|e| e.size).sum(),
            new_bytes,
            files: entries,
            automatic,
        };
        self.save_manifest(&snapshot)?;
        Ok(snapshot)
    }

    fn save_manifest(&self, snapshot: &Snapshot) -> Result<(), SnapshotError> {
        let path = self.manifest_path(&snapshot.instance_id, &snapshot.id);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_error(dir))?;
        }
        let json = serde_json::to_vec_pretty(snapshot).map_err(|e| invalid(e.to_string()))?;
        crate::storage::write_atomic(&path, &json).map_err(io_error(&path))
    }

    /// Hashes `path` and puts it in the store if it isn't there. Returns its entry and the
    /// bytes that were new.
    fn store_file(&self, path: &Path, rel: &str) -> Result<(FileEntry, u64), SnapshotError> {
        let metadata = fs::metadata(path).map_err(io_error(path))?;
        let sha256 = sha256_of(path)?;
        let object = self.object(&sha256);
        let mut added = 0;
        if !object.is_file() {
            let dir = object.parent().expect("objects have a parent");
            fs::create_dir_all(dir).map_err(io_error(dir))?;
            let tmp = dir.join(format!(
                ".{sha256}.{:016x}",
                u64::from_le_bytes(crate::random_bytes())
            ));
            clone_or_copy(path, &tmp)?;
            // Check what was stored, in case the file changed while it was read.
            if sha256_of(&tmp)? != sha256 {
                let _ = fs::remove_file(&tmp);
                return Err(SnapshotError::Instance(format!(
                    "{rel} changed while the snapshot was taken. Try again."
                )));
            }
            set_read_only(&tmp);
            fs::rename(&tmp, &object).map_err(io_error(&object))?;
            added = metadata.len();
        }
        let (category, world) = classify(rel).expect("classified");
        Ok((
            FileEntry {
                path: rel.to_string(),
                category,
                world,
                size: metadata.len(),
                sha256,
                mode: file_mode(&metadata),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            },
            added,
        ))
    }

    /// Deletes a snapshot and the objects only it used.
    pub fn delete(&self, instance_id: &str, id: &str) -> Result<u64, SnapshotError> {
        let snapshot = self.get(instance_id, id)?;
        let path = self.manifest_path(instance_id, &snapshot.id);
        fs::remove_file(&path).map_err(io_error(&path))?;
        Ok(self.collect_garbage())
    }

    /// Deletes every snapshot of an instance (when it is deleted).
    pub fn delete_instance(&self, instance_id: &str) -> u64 {
        if valid_id(instance_id) {
            let _ = fs::remove_dir_all(self.instance_dir(instance_id));
        }
        self.collect_garbage()
    }

    /// Removes the objects no snapshot uses. Returns the bytes freed.
    pub fn collect_garbage(&self) -> u64 {
        let mut used: HashSet<String> = HashSet::new();
        for dir in fs::read_dir(self.root.join("instances"))
            .into_iter()
            .flatten()
            .flatten()
        {
            for entry in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
                match read_manifest(&entry.path()) {
                    Ok(snapshot) => used.extend(snapshot.files.into_iter().map(|f| f.sha256)),
                    // An unreadable manifest: keep everything rather than guess.
                    Err(_) if entry.path().extension().is_some_and(|x| x == "json") => return 0,
                    Err(_) => {}
                }
            }
        }
        let mut freed = 0;
        for prefix in fs::read_dir(self.objects()).into_iter().flatten().flatten() {
            for object in fs::read_dir(prefix.path()).into_iter().flatten().flatten() {
                let name = object.file_name().to_string_lossy().to_string();
                if !used.contains(&name) {
                    freed += object.metadata().map(|m| m.len()).unwrap_or(0);
                    let _ = fs::remove_file(object.path());
                }
            }
            let _ = fs::remove_dir(prefix.path()); // only when empty
        }
        freed
    }

    /// Puts back what `selection` chooses from `snapshot` into the instance at
    /// `instance_dir`, replacing each chosen category whole. Takes an automatic snapshot
    /// of the same categories first. Returns the instance's new settings when they were
    /// put back (the caller saves them), and the automatic snapshot.
    pub fn revert(
        &self,
        snapshot: &Snapshot,
        current: &Instance,
        instance_dir: &Path,
        selection: &Selection,
        progress: Progress<'_>,
    ) -> Result<(Option<Instance>, Snapshot), SnapshotError> {
        let safety = self.create(
            current,
            instance_dir,
            &format!("Before going back to {}", snapshot.name),
            None,
            selection,
            true,
            progress,
        )?;
        // The top-level entries each chosen part replaces.
        let mut entries: BTreeSet<String> = BTreeSet::new();
        for category in &selection.categories {
            match category {
                Category::Worlds => {
                    let mut worlds: BTreeSet<String> = snapshot
                        .files
                        .iter()
                        .filter_map(|f| f.world.clone())
                        .collect();
                    if !selection.worlds.is_empty() {
                        worlds.retain(|w| selection.worlds.contains(w));
                    }
                    for world in worlds {
                        entries.extend(owned_entries(Category::Worlds, Some(&world)));
                    }
                }
                Category::Other => {
                    entries.extend(other_entries(instance_dir));
                    entries.extend(
                        snapshot
                            .files
                            .iter()
                            .filter(|f| f.category == Category::Other)
                            .map(|f| f.path.split('/').take(2).collect::<Vec<_>>().join("/")),
                    );
                }
                other => entries.extend(owned_entries(*other, None)),
            }
        }
        let wanted: Vec<&FileEntry> = snapshot
            .files
            .iter()
            .filter(|f| selection.takes(f.category, f.world.as_deref()))
            .collect();
        let tag = format!("{:016x}", u64::from_le_bytes(crate::random_bytes()));
        let staging = instance_dir.join(format!(".lodestone-restore-{tag}"));
        let old = instance_dir.join(format!(".lodestone-restore-old-{tag}"));
        let result = (|| {
            for file in &wanted {
                let relative = checked_relative(&file.path)?;
                let to = staging.join(&relative);
                if let Some(dir) = to.parent() {
                    fs::create_dir_all(dir).map_err(io_error(dir))?;
                }
                let object = self.object(&file.sha256);
                clone_or_copy(&object, &to)?;
                set_mode(&to, file.mode);
            }
            swap_in(instance_dir, &staging, &old, &entries)
        })();
        let _ = fs::remove_dir_all(&staging);
        let _ = fs::remove_dir_all(&old);
        result?;
        let settings = selection
            .categories
            .contains(&Category::Settings)
            .then(|| Instance {
                // The instance stays itself: its id, name and history.
                id: current.id.clone(),
                name: current.name.clone(),
                created: current.created,
                last_played: current.last_played,
                play_time: current.play_time,
                ..snapshot.instance.clone()
            });
        Ok((settings, safety))
    }

    /// Writes `snapshot` and the objects it needs as a `.lodesnap` file at `to`.
    pub fn export(
        &self,
        snapshot: &Snapshot,
        to: &Path,
        progress: Progress<'_>,
    ) -> Result<(), SnapshotError> {
        let tmp = to.with_extension(format!("{EXTENSION}.part"));
        let result = (|| {
            let file = File::create(&tmp).map_err(io_error(&tmp))?;
            let mut zip = zip::ZipWriter::new(BufWriter::new(file));
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .large_file(true);
            let manifest =
                serde_json::to_vec_pretty(snapshot).map_err(|e| invalid(e.to_string()))?;
            zip.start_file("manifest.json", options)
                .map_err(zip_error)?;
            zip.write_all(&manifest).map_err(io_error(&tmp))?;
            let unique: BTreeMap<&str, u64> = snapshot
                .files
                .iter()
                .map(|f| (f.sha256.as_str(), f.size))
                .collect();
            let total: u64 = unique.values().sum();
            let mut done = 0;
            for (sha256, size) in unique {
                let object = self.object(sha256);
                let mut source = File::open(&object).map_err(io_error(&object))?;
                zip.start_file(format!("objects/{sha256}"), options)
                    .map_err(zip_error)?;
                io::copy(&mut source, &mut zip).map_err(io_error(&object))?;
                done += size;
                progress(done, total);
            }
            zip.finish()
                .map_err(zip_error)?
                .flush()
                .map_err(io_error(&tmp))?;
            fs::rename(&tmp, to).map_err(io_error(to))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    /// Adds the snapshot in a `.lodesnap` file to the store (checking everything) and
    /// returns its manifest, under the instance id `instance_id`.
    pub fn import(
        &self,
        path: &Path,
        instance_id: &str,
        progress: Progress<'_>,
    ) -> Result<Snapshot, SnapshotError> {
        let (mut snapshot, mut archive) = open_lodesnap(path)?;
        let needed: BTreeMap<String, u64> = snapshot
            .files
            .iter()
            .map(|f| (f.sha256.clone(), f.size))
            .collect();
        let total: u64 = needed.values().sum();
        let mut done = 0;
        for (sha256, size) in &needed {
            let object = self.object(sha256);
            if !object.is_file() {
                let mut entry = archive.by_name(&format!("objects/{sha256}")).map_err(|_| {
                    invalid(format!("a file ({}) is missing from it", &sha256[..12]))
                })?;
                if entry.size() != *size {
                    return Err(invalid("a file's size doesn't match its manifest"));
                }
                let dir = object.parent().expect("objects have a parent");
                fs::create_dir_all(dir).map_err(io_error(dir))?;
                let tmp = dir.join(format!(".{sha256}.import"));
                let written = (|| {
                    let mut out = BufWriter::new(File::create(&tmp).map_err(io_error(&tmp))?);
                    let mut hasher = Sha256::new();
                    let mut buffer = vec![0u8; 256 * 1024];
                    let mut read_total = 0u64;
                    loop {
                        let n = entry.read(&mut buffer).map_err(io_error(path))?;
                        if n == 0 {
                            break;
                        }
                        read_total += n as u64;
                        if read_total > *size {
                            return Err(invalid("a file is bigger than its manifest says"));
                        }
                        hasher.update(&buffer[..n]);
                        out.write_all(&buffer[..n]).map_err(io_error(&tmp))?;
                    }
                    out.flush().map_err(io_error(&tmp))?;
                    if hex(&hasher.finalize()) != *sha256 {
                        return Err(invalid("a file doesn't match its checksum"));
                    }
                    Ok(())
                })();
                if let Err(err) = written {
                    let _ = fs::remove_file(&tmp);
                    return Err(err);
                }
                set_read_only(&tmp);
                fs::rename(&tmp, &object).map_err(io_error(&object))?;
            }
            done += size;
            progress(done, total);
        }
        snapshot.instance_id = instance_id.to_string();
        snapshot.instance.id = instance_id.to_string();
        // A new id here, so importing the same file twice keeps both.
        snapshot.id = new_id(crate::instances::now());
        self.save_manifest(&snapshot)?;
        Ok(snapshot)
    }

    /// Makes a new instance from a `.lodesnap` file, with the parts `selection` chooses.
    /// The snapshot is kept as the new instance's first snapshot.
    pub fn import_as_instance(
        &self,
        instances: &InstanceStore,
        path: &Path,
        selection: &Selection,
        progress: Progress<'_>,
    ) -> Result<Instance, SnapshotError> {
        let staging = instances
            .new_staging()
            .map_err(|e| SnapshotError::Instance(e.to_string()))?;
        let result = (|| {
            let imported = self.import(path, ".importing", progress)?;
            for file in imported
                .files
                .iter()
                .filter(|f| selection.takes(f.category, f.world.as_deref()))
            {
                let to = staging.join(checked_relative(&file.path)?);
                if let Some(dir) = to.parent() {
                    fs::create_dir_all(dir).map_err(io_error(dir))?;
                }
                clone_or_copy(&self.object(&file.sha256), &to)?;
                set_mode(&to, file.mode);
            }
            let instance = Instance {
                created: crate::instances::now(),
                last_played: None,
                play_time: 0,
                ..imported.instance.clone()
            };
            let instance = instances
                .commit_staged(&staging, instance)
                .map_err(|e| SnapshotError::Instance(e.to_string()))?;
            // Keep the snapshot, now under the new instance.
            let old = self.manifest_path(".importing", &imported.id);
            let moved = Snapshot {
                instance_id: instance.id.clone(),
                instance: Instance {
                    id: instance.id.clone(),
                    ..imported.instance.clone()
                },
                ..imported
            };
            self.save_manifest(&moved)?;
            let _ = fs::remove_file(&old);
            let _ = fs::remove_dir(self.instance_dir(".importing"));
            Ok(instance)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
            let _ = fs::remove_dir_all(self.instance_dir(".importing"));
            self.collect_garbage();
        }
        result
    }
}

/// What a `.lodesnap` file holds, for the preview before importing it.
pub fn inspect(path: &Path) -> Result<Snapshot, SnapshotError> {
    open_lodesnap(path).map(|(snapshot, _)| snapshot)
}

type Archive = zip::ZipArchive<BufReader<File>>;

/// Opens a `.lodesnap`, checks its entries and limits, and reads its manifest.
fn open_lodesnap(path: &Path) -> Result<(Snapshot, Archive), SnapshotError> {
    let file = File::open(path).map_err(io_error(path))?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|_| invalid("it isn't a zip file"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("it has too many files"));
    }
    let mut total = 0u64;
    let mut objects: HashSet<String> = HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(zip_error)?;
        let name = entry.name().to_string();
        let size = entry.size();
        total = total.saturating_add(size);
        if entry.is_dir() {
            if name == "objects/" {
                continue;
            }
            return Err(invalid(format!("unexpected folder {name}")));
        }
        if size > 1024 * 1024 && size / entry.compressed_size().max(1) > MAX_RATIO {
            return Err(invalid("a file inflates too much to be safe"));
        }
        match name.as_str() {
            "manifest.json" if size <= MAX_MANIFEST_BYTES => {}
            "manifest.json" => return Err(invalid("its manifest is too big")),
            other => {
                let Some(hash) = other.strip_prefix("objects/").filter(|h| is_sha256(h)) else {
                    return Err(invalid(format!("unexpected file {other}")));
                };
                objects.insert(hash.to_string());
            }
        }
    }
    if total > MAX_TOTAL_BYTES {
        return Err(invalid("it is larger than 64 GB unpacked"));
    }
    let mut text = String::new();
    archive
        .by_name("manifest.json")
        .map_err(|_| invalid("it has no manifest"))?
        .take(MAX_MANIFEST_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| invalid("its manifest isn't readable"))?;
    let snapshot = parse_manifest(&text)?;
    for file in &snapshot.files {
        if !objects.contains(&file.sha256) {
            return Err(invalid(format!("{} is missing from it", file.path)));
        }
    }
    Ok((snapshot, archive))
}

fn read_manifest(path: &Path) -> Result<Snapshot, SnapshotError> {
    let text = fs::read_to_string(path).map_err(io_error(path))?;
    parse_manifest(&text)
}

/// Parses and checks a manifest: schema, file paths and categories, hashes, sizes.
fn parse_manifest(text: &str) -> Result<Snapshot, SnapshotError> {
    let snapshot: Snapshot =
        serde_json::from_str(text).map_err(|e| invalid(format!("its manifest: {e}")))?;
    if snapshot.schema > SCHEMA {
        return Err(SnapshotError::TooNew);
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for file in &snapshot.files {
        checked_relative(&file.path)?;
        if !seen.insert(file.path.as_str()) {
            return Err(invalid(format!("{} is listed twice", file.path)));
        }
        if !is_sha256(&file.sha256) {
            return Err(invalid(format!("{} has no valid checksum", file.path)));
        }
        match classify(&file.path) {
            Some((category, world)) if category == file.category && world == file.world => {}
            _ => return Err(invalid(format!("{} isn't where it should be", file.path))),
        }
        total = total.saturating_add(file.size);
    }
    if total > MAX_TOTAL_BYTES {
        return Err(invalid("it is larger than 64 GB"));
    }
    Ok(snapshot)
}

/// `path` as a safe relative path under the instance folder.
fn checked_relative(path: &str) -> Result<PathBuf, SnapshotError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.contains(':')
    {
        return Err(invalid(format!("{path} isn't a safe path")));
    }
    Ok(path.split('/').collect())
}

/// Moves the current `entries` (top-level paths under `root`) aside to `old`, then the
/// staged ones from `staging` into place. On a failure, puts everything back.
fn swap_in(
    root: &Path,
    staging: &Path,
    old: &Path,
    entries: &BTreeSet<String>,
) -> Result<(), SnapshotError> {
    let mut moved_aside: Vec<&String> = Vec::new();
    let mut moved_in: Vec<&String> = Vec::new();
    let result = (|| {
        for entry in entries {
            let current = root.join(entry);
            if fs::symlink_metadata(&current).is_ok() {
                let aside = old.join(entry);
                if let Some(dir) = aside.parent() {
                    fs::create_dir_all(dir).map_err(io_error(dir))?;
                }
                fs::rename(&current, &aside).map_err(io_error(&current))?;
                moved_aside.push(entry);
            }
        }
        for entry in entries {
            let staged = staging.join(entry);
            if fs::symlink_metadata(&staged).is_ok() {
                let target = root.join(entry);
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir).map_err(io_error(dir))?;
                }
                fs::rename(&staged, &target).map_err(io_error(&staged))?;
                moved_in.push(entry);
            }
        }
        Ok(())
    })();
    if result.is_err() {
        for entry in moved_in {
            let _ = fs::rename(root.join(entry), staging.join(entry));
        }
        for entry in moved_aside {
            let _ = fs::rename(old.join(entry), root.join(entry));
        }
    }
    result
}

/// Every file under `instance_dir` a snapshot can take: (path, relative path, size).
/// Symbolic links are skipped.
fn walk(instance_dir: &Path) -> Result<Vec<(PathBuf, String, u64)>, SnapshotError> {
    fn visit(
        dir: &Path,
        rel: &str,
        out: &mut Vec<(PathBuf, String, u64)>,
    ) -> Result<(), SnapshotError> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(io_error(dir)(err)),
        };
        for entry in entries {
            let entry = entry.map_err(io_error(dir))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let relative = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            // Skip whole excluded folders without walking them.
            if file_type.is_dir() {
                if classify(&format!("{relative}/x")).is_none() && !relative_is_prefix(&relative) {
                    continue;
                }
                visit(&entry.path(), &relative, out)?;
            } else if file_type.is_file() && classify(&relative).is_some() {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push((entry.path(), relative, size));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(instance_dir, "", &mut out)?;
    Ok(out)
}

/// Folders that are walked into although they aren't a category themselves.
fn relative_is_prefix(relative: &str) -> bool {
    relative == "minecraft" || relative == "minecraft/saves"
}

pub fn sha256_of(path: &Path) -> Result<String, SnapshotError> {
    let mut file = BufReader::new(File::open(path).map_err(io_error(path))?);
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(io_error(path))?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('/')
        && id != ".."
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A sortable, unique snapshot id: `20261108-153012-a1b2`.
fn new_id(created: u64) -> String {
    let time = chrono::DateTime::from_timestamp(created as i64, 0).unwrap_or_default();
    format!(
        "{}-{:04x}",
        time.format("%Y%m%d-%H%M%S"),
        u16::from_le_bytes(crate::random_bytes::<2>())
    )
}

fn zip_error(err: zip::result::ZipError) -> SnapshotError {
    invalid(err.to_string())
}

/// A copy-on-write clone on APFS (no space used until either changes); a copy
/// elsewhere.
fn clone_or_copy(from: &Path, to: &Path) -> Result<(), SnapshotError> {
    let _ = fs::remove_file(to);
    #[cfg(target_os = "macos")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        if let (Ok(src), Ok(dst)) = (
            CString::new(from.as_os_str().as_bytes()),
            CString::new(to.as_os_str().as_bytes()),
        ) {
            // SAFETY: two valid NUL-terminated paths; no flags.
            if unsafe { libc::clonefile(src.as_ptr(), dst.as_ptr(), 0) } == 0 {
                return Ok(());
            }
        }
    }
    fs::copy(from, to).map(|_| ()).map_err(io_error(to))
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(_: &fs::Metadata) -> u32 {
    0o644
}

/// Gives a restored file its mode, made writable (store objects are read-only).
fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if mode == 0 { 0o644 } else { mode | 0o200 };
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        if let Ok(metadata) = fs::metadata(path) {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

fn set_read_only(path: &Path) {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, contents: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn instance_at(root: &Path, id: &str) -> (Instance, PathBuf) {
        let dir = root.join("instances").join(id);
        write(&dir, "minecraft/mods/sodium.jar", b"sodium");
        write(&dir, "minecraft/mods/iris.jar", b"iris");
        write(&dir, "minecraft/resourcepacks/faithful.zip", b"faithful");
        write(&dir, "minecraft/config/sodium.json", b"{}");
        write(&dir, "minecraft/options.txt", b"fov:70");
        write(&dir, "minecraft/saves/World/level.dat", b"world");
        write(&dir, "minecraft/saves/Other/level.dat", b"other");
        write(&dir, "minecraft/logs/latest.log", b"log");
        write(&dir, "minecraft/kubejs/a.js", b"js");
        write(&dir, "icon.png", b"png");
        let instance = Instance {
            id: id.into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            ..Instance::default()
        };
        (instance, dir)
    }

    fn none(_: u64, _: u64) {}

    #[test]
    fn classifies_paths() {
        assert_eq!(
            classify("minecraft/mods/a.jar"),
            Some((Category::Mods, None))
        );
        assert_eq!(classify("content.json"), Some((Category::Mods, None)));
        assert_eq!(
            classify("minecraft/options.txt"),
            Some((Category::Configs, None))
        );
        assert_eq!(
            classify("minecraft/saves/W/region/r.0.0.mca"),
            Some((Category::Worlds, Some("W".into())))
        );
        assert_eq!(classify("minecraft/logs/latest.log"), None);
        assert_eq!(
            classify("minecraft/kubejs/a.js"),
            Some((Category::Other, None))
        );
        assert_eq!(classify("instance.json"), None);
        assert_eq!(classify("icon.png"), Some((Category::Settings, None)));
    }

    #[test]
    fn snapshots_share_objects_and_garbage_is_collected() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("snapshots"));
        let (instance, dir) = instance_at(temp.path(), "a");
        let first = store
            .create(
                &instance,
                &dir,
                "one",
                None,
                &Selection::all(),
                false,
                &none,
            )
            .unwrap();
        assert!(first.new_bytes > 0);
        assert!(!first.files.iter().any(|f| f.path.contains("logs")));
        let second = store
            .create(
                &instance,
                &dir,
                "two",
                None,
                &Selection::all(),
                false,
                &none,
            )
            .unwrap();
        assert_eq!(second.new_bytes, 0, "the second snapshot adds nothing");
        // A second instance with the same mod shares its object.
        let (other, other_dir) = instance_at(temp.path(), "b");
        let third = store
            .create(
                &other,
                &other_dir,
                "b",
                None,
                &Selection::all(),
                false,
                &none,
            )
            .unwrap();
        assert_eq!(third.new_bytes, 0);
        assert_eq!(store.list("a").len(), 2);
        // Deleting one of two snapshots frees nothing; deleting all frees the objects.
        assert_eq!(store.delete("a", &first.id).unwrap(), 0);
        store.delete("a", &second.id).unwrap();
        store.delete("b", &third.id).unwrap();
        assert_eq!(fs::read_dir(store.objects()).unwrap().count(), 0);
    }

    #[test]
    fn takes_only_the_chosen_parts() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("snapshots"));
        let (instance, dir) = instance_at(temp.path(), "a");
        let selection = Selection {
            categories: [Category::Mods, Category::Worlds].into(),
            worlds: ["World".to_string()].into(),
        };
        let snapshot = store
            .create(&instance, &dir, "s", None, &selection, false, &none)
            .unwrap();
        let paths: Vec<&str> = snapshot.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"minecraft/mods/sodium.jar"));
        assert!(paths.contains(&"minecraft/saves/World/level.dat"));
        assert!(!paths.contains(&"minecraft/saves/Other/level.dat"));
        assert!(!paths.contains(&"minecraft/options.txt"));
        let parts = store.plan(&dir);
        assert!(
            parts
                .iter()
                .any(|p| p.category == Category::Other && p.files == 1)
        );
    }

    #[test]
    fn reverts_only_the_chosen_parts_after_a_safety_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("snapshots"));
        let (instance, dir) = instance_at(temp.path(), "a");
        let snapshot = store
            .create(
                &instance,
                &dir,
                "before",
                None,
                &Selection::all(),
                false,
                &none,
            )
            .unwrap();
        // Change mods and options after the snapshot.
        fs::remove_file(dir.join("minecraft/mods/iris.jar")).unwrap();
        write(&dir, "minecraft/mods/new.jar", b"new");
        write(&dir, "minecraft/options.txt", b"fov:110");
        let changed = Instance {
            game_version: "1.21.1".into(),
            ..instance.clone()
        };
        let selection = Selection {
            categories: [Category::Mods, Category::Settings].into(),
            worlds: BTreeSet::new(),
        };
        let (settings, safety) = store
            .revert(&snapshot, &changed, &dir, &selection, &none)
            .unwrap();
        assert!(dir.join("minecraft/mods/iris.jar").is_file());
        assert!(!dir.join("minecraft/mods/new.jar").exists());
        // Options weren't chosen: they keep the change.
        assert_eq!(
            fs::read(dir.join("minecraft/options.txt")).unwrap(),
            b"fov:110"
        );
        assert_eq!(settings.unwrap().game_version, "1.20.1");
        assert!(safety.automatic);
        assert!(
            safety
                .files
                .iter()
                .any(|f| f.path == "minecraft/mods/new.jar")
        );
        // Restored files are writable.
        assert!(
            !fs::metadata(dir.join("minecraft/mods/iris.jar"))
                .unwrap()
                .permissions()
                .readonly()
        );
    }

    #[test]
    fn exports_and_imports_as_a_new_instance() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("snapshots"));
        let (instance, dir) = instance_at(temp.path(), "a");
        let snapshot = store
            .create(
                &instance,
                &dir,
                "share",
                None,
                &Selection::all(),
                false,
                &none,
            )
            .unwrap();
        let file = temp.path().join("share.lodesnap");
        store.export(&snapshot, &file, &none).unwrap();
        let preview = inspect(&file).unwrap();
        assert_eq!(preview.mods(), vec!["iris.jar", "sodium.jar"]);
        // Into another launcher's store and instances.
        let other = Store::at(temp.path().join("other-snapshots"));
        let instances = InstanceStore::new(temp.path().join("other-instances"));
        fs::create_dir_all(instances.root()).unwrap();
        let selection = Selection {
            categories: [Category::Mods, Category::Settings].into(),
            worlds: BTreeSet::new(),
        };
        let created = other
            .import_as_instance(&instances, &file, &selection, &none)
            .unwrap();
        let new_dir = instances.dir(&created.id);
        assert_eq!(
            fs::read(new_dir.join("minecraft/mods/sodium.jar")).unwrap(),
            b"sodium"
        );
        assert!(!new_dir.join("minecraft/saves").exists());
        assert_eq!(created.game_version, "1.20.1");
        assert_eq!(other.list(&created.id).len(), 1);
    }

    fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::FileOptions::default();
        for (name, data) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn refuses_unsafe_files() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::at(temp.path().join("snapshots"));
        let (instance, dir) = instance_at(temp.path(), "a");
        let snapshot = store
            .create(&instance, &dir, "s", None, &Selection::all(), false, &none)
            .unwrap();
        let good = |s: &Snapshot| serde_json::to_vec(s).unwrap();
        let object = |f: &FileEntry| fs::read(store.object(&f.sha256)).unwrap();
        // A path that climbs out.
        let mut climbing = snapshot.clone();
        climbing.files[0].path = "minecraft/mods/../../../evil.jar".into();
        let bad = temp.path().join("climb.lodesnap");
        let first = &snapshot.files[0];
        zip_with(
            &bad,
            &[
                ("manifest.json", &good(&climbing)),
                (&format!("objects/{}", first.sha256), &object(first)),
            ],
        );
        assert!(inspect(&bad).is_err());
        // An unexpected entry.
        let extra = temp.path().join("extra.lodesnap");
        zip_with(
            &extra,
            &[("manifest.json", &good(&snapshot)), ("run.sh", b"rm -rf /")],
        );
        assert!(inspect(&extra).is_err());
        // An object whose bytes don't match its hash.
        let mut one = snapshot.clone();
        one.files.truncate(1);
        let tampered = temp.path().join("tampered.lodesnap");
        let mut wrong = object(&one.files[0]);
        wrong.push(b'!');
        one.files[0].size = wrong.len() as u64;
        zip_with(
            &tampered,
            &[
                ("manifest.json", &good(&one)),
                (&format!("objects/{}", one.files[0].sha256), &wrong),
            ],
        );
        let fresh = Store::at(temp.path().join("fresh"));
        assert!(fresh.import(&tampered, "x", &none).is_err());
        // A highly compressible entry (a zip bomb's shape).
        let bomb = temp.path().join("bomb.lodesnap");
        let mut zip = zip::ZipWriter::new(File::create(&bomb).unwrap());
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(format!("objects/{}", "a".repeat(64)), options)
            .unwrap();
        zip.write_all(&vec![0u8; 64 * 1024 * 1024]).unwrap();
        zip.finish().unwrap();
        assert!(inspect(&bomb).is_err());
    }
}
