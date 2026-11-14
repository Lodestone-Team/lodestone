//! The Launcher's snapshot manifests and `.lodesnap` files (schema 1) as images, and back.
//!
//! A schema-1 snapshot lists the instance folder's files by path (`minecraft/mods/…`,
//! `icon.png`), each in a category, with the instance's whole `instance.json` beside them.
//! Its objects are raw files named by their SHA-256, which is what the Launcher's image
//! store holds, so converting touches no object.
//!
//! - Files in the game folder (`minecraft/…`) go through the client [`DiskLayout`]: worlds
//!   are `saves/<world>/…` and the categories follow from the paths. Files the layout can't
//!   map, or that an image never records (`session.lock`, `cache/`), are withheld and kept
//!   in the rest.
//! - Files beside the game folder (the icon, the Play background, `content.json`,
//!   `modpack.json`, shared options) have no canonical path. They are kept in the rest; the
//!   icon and background also become the client settings' `icon` and `background`.
//!   `content.json` and `modpack.json` have their own reader ([`super::launcher_modpack`]).
//! - The instance gives the game (Minecraft version, loader, loader version) and the
//!   recommended memory. Everything else in it is local (Java, arguments, memory, window,
//!   play time, appearance) and never goes into an image: it is reported in a note and kept
//!   in the rest, so a snapshot imported from someone else can still offer their settings
//!   only when the user chooses them.
//! - The kind follows the selection: content categories only (mods, packs, configs,
//!   settings) make a content image, worlds, screenshots and other files only a data image,
//!   both a full image.
//!
//! [`from_image`] writes the manifest back, equal to the original and byte for byte the
//! same once serialized ([`Snapshot::to_json`]).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{Read, Seek};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{clean_text, finish, version_probe, LegacyError, Note};
use crate::hex::is_sha256;
use crate::layout::{DiskLayout, CLIENT_GAME_FOLDER};
use crate::manifest::WithheldReason;
use crate::manifest::{bounded_list, capped_value, UnknownBudget};
use crate::manifest::{App, Entry, Game, Image, Kind, Loader, Producer, Source, World};
use crate::settings::{ClientSettings, Settings, StoredFile, MAX_STORED_FILE};

/// The manifest schema this reads.
pub const SCHEMA: u32 = 1;
/// Limits on a `.lodesnap` from someone else, as the Launcher's reader has them.
pub const MAX_ENTRIES: usize = 200_000;
pub const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

/// How much of a snapshot's `instance.json` is read: a real one is a few hundred bytes, and
/// one from elsewhere mustn't make reading it cost far more than its size.
pub const INSTANCE_BUDGET: UnknownBudget = UnknownBudget {
    values: 4096,
    bytes: 64 << 10,
};
/// A selection names at most this many worlds.
pub const MAX_SELECTED_WORLDS: usize = 10_000;

/// The instance fields an image carries; the rest of `instance.json` is local.
const GAME_VERSION: &str = "game_version";
const LOADER: &str = "loader";
const LOADER_VERSION: &str = "loader_version";
const RECOMMENDED_MEMORY: &str = "recommended_memory";
/// Files beside the game folder that are client settings.
const ICON_FILE: &str = "icon.png";
const BACKGROUND_FILE: &str = "background.jpg";

// The types below mirror `launcher/crates/launcher-core/src/snapshots.rs` at commit f5ce27ad
// (`Snapshot`, `FileEntry`, `Selection`, `Category`, `classify`), field for field and in the
// same order, so a manifest serializes to the same bytes. The embedded `Instance` is kept
// as JSON, in its own order: the image reads four of its fields and the rest is local.

/// What a file is, for choosing what a snapshot takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Mods,
    ResourcePacks,
    ShaderPacks,
    Configs,
    Worlds,
    Screenshots,
    Settings,
    Other,
}

impl Category {
    /// Whether the category is content (recorded at every change) rather than data.
    fn is_content(self) -> bool {
        matches!(
            self,
            Category::Mods
                | Category::ResourcePacks
                | Category::ShaderPacks
                | Category::Configs
                | Category::Settings
        )
    }
}

/// What a snapshot takes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    #[serde(deserialize_with = "categories_set")]
    pub categories: BTreeSet<Category>,
    /// The worlds, by folder name; empty means every world.
    #[serde(default, deserialize_with = "worlds_set")]
    pub worlds: BTreeSet<String>,
}

fn categories_set<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeSet<Category>, D::Error> {
    Ok(bounded_list::<D, Category, 64>(d)?.into_iter().collect())
}

fn worlds_set<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeSet<String>, D::Error> {
    Ok(bounded_list::<D, String, MAX_SELECTED_WORLDS>(d)?
        .into_iter()
        .collect())
}

fn instance_map<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Map<String, Value>, D::Error> {
    let mut budget = INSTANCE_BUDGET;
    match capped_value(d, &mut budget)? {
        Value::Object(map) => Ok(map),
        _ => Err(serde::de::Error::custom("the instance isn't a JSON object")),
    }
}

fn files_list<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<FileEntry>, D::Error> {
    bounded_list::<D, FileEntry, MAX_ENTRIES>(d)
}

/// One file of a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Under the instance folder, `/`-separated.
    pub path: String,
    pub category: Category,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    pub size: u64,
    pub sha256: String,
    /// Unix permission bits; 0 in manifests from before they were kept.
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
    /// `instance.json` when the snapshot was taken, read within [`INSTANCE_BUDGET`].
    #[serde(deserialize_with = "instance_map")]
    pub instance: Map<String, Value>,
    pub selection: Selection,
    /// At most [`MAX_ENTRIES`], as the Launcher's import allows.
    #[serde(deserialize_with = "files_list")]
    pub files: Vec<FileEntry>,
    pub total_bytes: u64,
    pub new_bytes: u64,
    #[serde(default)]
    pub automatic: bool,
}

impl Snapshot {
    /// Reads and checks a manifest the way the Launcher does (`parse_manifest`): a newer
    /// schema is refused, and every path must be safe, listed once, hashed and in the
    /// category its place gives. A manifest can come from someone else's `.lodesnap`, so
    /// its shape is checked first and its lists and `instance` are read within bounds:
    /// fields it doesn't know are skipped without being kept.
    pub fn from_json(bytes: &[u8]) -> Result<Snapshot, LegacyError> {
        let schema = version_probe(bytes, "schema")?
            .ok_or_else(|| LegacyError::Invalid("no schema".into()))?;
        if schema > u64::from(SCHEMA) {
            return Err(LegacyError::Newer {
                found: schema,
                supported: u64::from(SCHEMA),
            });
        }
        let snapshot: Snapshot =
            serde_json::from_slice(bytes).map_err(|e| LegacyError::Invalid(e.to_string()))?;
        snapshot.validate().map_err(LegacyError::Invalid)?;
        Ok(snapshot)
    }

    /// The manifest as the Launcher writes it: pretty JSON.
    pub fn to_json(&self) -> Vec<u8> {
        // Strings, numbers and JSON values: serializing can't fail.
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    fn validate(&self) -> Result<(), String> {
        let mut seen = HashSet::new();
        let mut total = 0u64;
        for file in &self.files {
            if !safe_relative(&file.path) {
                return Err(format!("{} isn't a safe path", file.path));
            }
            if !seen.insert(file.path.as_str()) {
                return Err(format!("{} is listed twice", file.path));
            }
            if !is_sha256(&file.sha256) {
                return Err(format!("{} has no valid checksum", file.path));
            }
            match classify(&file.path) {
                Some((category, world)) if category == file.category && world == file.world => {}
                _ => return Err(format!("{} isn't where it should be", file.path)),
            }
            total = total.saturating_add(file.size);
        }
        if total > MAX_TOTAL_BYTES {
            return Err("it is larger than 64 GB".into());
        }
        match self.instance.get(GAME_VERSION) {
            Some(Value::String(version)) if !version.is_empty() => Ok(()),
            _ => Err("its instance has no game version".into()),
        }
    }
}

/// The Launcher's `checked_relative`: a relative `/`-separated path without `..`, `\`, `:`
/// or NUL.
fn safe_relative(path: &str) -> bool {
    !(path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".."))
}

/// Files in the game folder that are never taken.
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
/// Files in the instance folder that belong to the settings.
const SETTINGS_FILES: [&str; 4] = [
    ICON_FILE,
    BACKGROUND_FILE,
    "backgrounds",
    "shared_options.json",
];
/// Files in the instance folder that go with the mods.
const MOD_RECORDS: [&str; 2] = ["content.json", "modpack.json"];

/// The category of `relative` (under the instance folder) and, for a world, its folder;
/// `None` for files a snapshot never takes. The Launcher's `classify`.
pub fn classify(relative: &str) -> Option<(Category, Option<String>)> {
    let mut parts = relative.split('/');
    let first = parts.next()?;
    if first != CLIENT_GAME_FOLDER {
        if MOD_RECORDS.contains(&first) {
            return Some((Category::Mods, None));
        }
        if SETTINGS_FILES.contains(&first) {
            return Some((Category::Settings, None));
        }
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
            parts.next()?;
            return Some((Category::Worlds, Some(world.to_string())));
        }
        _ => Category::Other,
    };
    Some((category, None))
}

/// Reads the manifest of a `.lodesnap` file with the Launcher's checks for files from
/// other people: known entry names only (`manifest.json`, `objects/<sha256>`), at most
/// [`MAX_ENTRIES`] entries and [`MAX_TOTAL_BYTES`] unpacked, a manifest of at most
/// [`MAX_MANIFEST_BYTES`], and an object for every file it lists. The objects are not read:
/// the importer checks each one's hash as it unpacks it.
pub fn read_lodesnap<R: Read + Seek>(reader: R) -> Result<Snapshot, LegacyError> {
    let invalid = |m: &str| LegacyError::Invalid(m.to_string());
    let mut archive = zip::ZipArchive::new(reader).map_err(|_| invalid("it isn't a zip file"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("it has too many files"));
    }
    // A name given twice, or local headers that say something other than the directory,
    // is a zip other tools read differently: refused, as image exports refuse it.
    archive
        .validate_header_consistency()
        .map_err(|e| LegacyError::Invalid(format!("its zip structure is inconsistent ({e})")))?;
    let mut total = 0u64;
    let mut objects: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| LegacyError::Invalid(format!("its entries: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| LegacyError::Invalid(format!("its entries: {e}")))?
            .into_owned();
        if !seen.insert(name.clone()) {
            return Err(LegacyError::Invalid(format!("{name} is in it twice")));
        }
        total = total.saturating_add(entry.size());
        if entry.is_dir() {
            if name == "objects/" {
                continue;
            }
            return Err(LegacyError::Invalid(format!("unexpected folder {name}")));
        }
        match name.as_str() {
            "manifest.json" if entry.size() <= MAX_MANIFEST_BYTES => {}
            "manifest.json" => return Err(invalid("its manifest is too big")),
            other => {
                let Some(hash) = other.strip_prefix("objects/").filter(|h| is_sha256(h)) else {
                    return Err(LegacyError::Invalid(format!("unexpected file {other}")));
                };
                objects.insert(hash.to_string());
            }
        }
    }
    if total > MAX_TOTAL_BYTES {
        return Err(invalid("it is larger than 64 GB unpacked"));
    }
    let mut bytes = Vec::new();
    archive
        .by_name("manifest.json")
        .map_err(|_| invalid("it has no manifest"))?
        .take(MAX_MANIFEST_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("its manifest isn't readable"))?;
    let snapshot = Snapshot::from_json(&bytes)?;
    if let Some(file) = snapshot.files.iter().find(|f| !objects.contains(&f.sha256)) {
        return Err(LegacyError::Invalid(format!(
            "{} is missing from it",
            file.path
        )));
    }
    Ok(snapshot)
}

/// What a schema-1 snapshot has that its image doesn't. Kept beside the image (it is
/// serde, so it can be stored), never in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rest {
    pub id: String,
    pub instance_id: String,
    /// The snapshot's name, when the label had to change it (control characters, length).
    pub name: Option<String>,
    pub note: Option<String>,
    pub selection: Selection,
    pub total_bytes: u64,
    pub new_bytes: u64,
    pub automatic: bool,
    /// The whole `instance.json`; the image carries its game and recommended memory, and
    /// writing back takes those from the image.
    pub instance: Map<String, Value>,
    /// Files beside the game folder, as they were.
    pub instance_files: Vec<FileEntry>,
    /// Game-folder files the image withholds, with the reason it gives.
    pub unmapped: Vec<(FileEntry, WithheldReason)>,
    /// Modification times too far from 1970 for an image's nanoseconds, by path.
    pub wide_times: BTreeMap<String, u64>,
    /// Files whose mode was 0 (manifests from before modes were kept), by path. Their
    /// entries have 0o644, what the Launcher's restore always gave them; writing back
    /// gives them 0 again.
    pub zero_modes: BTreeSet<String>,
    /// Modes with bits beyond the permission bits (setuid, setgid, sticky, file type) as
    /// they were, by path: an image keeps only the permission bits.
    #[serde(default)]
    pub wide_modes: BTreeMap<String, u32>,
}

impl Rest {
    /// The objects the rest names that the image doesn't: files beside the game folder
    /// and game-folder files the image withholds. A store's collection must keep them
    /// ([`crate::store::gc::GcOptions::also_live`]) for as long as this rest may be
    /// written back, and a restore that should put back everything the snapshot had
    /// places them itself, at their instance paths, with [`crate::store::Store::read_object`]:
    /// materializing places only the image's entries.
    pub fn objects(&self) -> impl Iterator<Item = &str> {
        self.instance_files
            .iter()
            .chain(self.unmapped.iter().map(|(file, _)| file))
            .map(|file| file.sha256.as_str())
    }
}

/// A converted snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub image: Image,
    pub rest: Rest,
    pub notes: Vec<Note>,
}

/// Reads a schema-1 snapshot manifest and converts it.
pub fn read(bytes: &[u8]) -> Result<Converted, LegacyError> {
    to_image(&Snapshot::from_json(bytes)?)
}

/// The image of a schema-1 snapshot; see the module.
pub fn to_image(snapshot: &Snapshot) -> Result<Converted, LegacyError> {
    let instance = &snapshot.instance;
    let text = |key: &str| match instance.get(key) {
        Some(Value::String(text)) => Some(text.clone()),
        _ => None,
    };
    let minecraft = text(GAME_VERSION)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| LegacyError::Invalid("its instance has no game version".into()))?;
    let loader = match text(LOADER).as_deref() {
        None | Some("none") => Loader::Vanilla,
        Some(other) => Loader::parse(other),
    };
    let kind = {
        let content = snapshot.selection.categories.iter().any(|c| c.is_content());
        let data = snapshot
            .selection
            .categories
            .iter()
            .any(|c| !c.is_content());
        match (content, data) {
            (true, true) => Kind::Full,
            (true, false) => Kind::Content,
            (false, _) => Kind::Data,
        }
    };
    let mut image = Image::new(
        kind,
        Game {
            minecraft,
            loader,
            loader_version: text(LOADER_VERSION),
        },
    );
    image.created = i64::try_from(snapshot.created)
        .map_err(|_| LegacyError::Invalid("its creation time is out of range".into()))?;
    image.producer = Producer {
        app: App::Launcher,
        version: snapshot.launcher_version.clone(),
    };
    let label = clean_text(&snapshot.name);
    let name = (label != snapshot.name).then(|| snapshot.name.clone());
    image.label = Some(label).filter(|l| !l.is_empty());

    let mut notes = Vec::new();
    let mut instance_files = Vec::new();
    let mut unmapped = Vec::new();
    let mut wide_times = BTreeMap::new();
    let mut zero_modes = BTreeSet::new();
    let mut wide_modes = BTreeMap::new();
    let game_prefix = format!("{CLIENT_GAME_FOLDER}/");
    for file in &snapshot.files {
        if !file.path.starts_with(&game_prefix) {
            instance_files.push(file.clone());
            continue;
        }
        let reason = match DiskLayout::Client.from_disk(&file.path) {
            Ok(Some(path)) => {
                let mut entry = Entry::new(path, file.size, &file.sha256, Source::Object {});
                entry.mode = if file.mode == 0 {
                    // Not kept back then; the Launcher restored such files as 0o644.
                    zero_modes.insert(file.path.clone());
                    0o644
                } else {
                    // Images keep only the permission bits (the Launcher's own writer never
                    // wrote more; a file from elsewhere might, and gets them back).
                    if file.mode & !0o777 != 0 {
                        wide_modes.insert(file.path.clone(), file.mode);
                    }
                    match file.mode & 0o777 {
                        0 => 0o644,
                        bits => bits,
                    }
                };
                entry.modified = file.modified.and_then(|seconds| {
                    let nanos = i64::try_from(seconds)
                        .ok()
                        .and_then(|s| s.checked_mul(1_000_000_000));
                    if nanos.is_none() {
                        wide_times.insert(file.path.clone(), seconds);
                    }
                    nanos
                });
                image.entries.push(entry);
                continue;
            }
            Ok(None) => WithheldReason::Excluded,
            Err(_) => WithheldReason::BadName,
        };
        super::withhold(&mut image, &mut notes, &file.path, reason.clone());
        unmapped.push((file.clone(), reason));
    }

    let mut world_sizes: BTreeMap<String, u64> = BTreeMap::new();
    for entry in &image.entries {
        if let Some(world) = &entry.world {
            // Sizes come from the old manifest: a hostile one can't overflow the sum.
            let size = world_sizes.entry(world.clone()).or_default();
            *size = size.saturating_add(entry.size);
        }
    }
    image.worlds = world_sizes
        .into_iter()
        .map(|(name, size)| World {
            name,
            level_name: None,
            data_version: None,
            game_version: None,
            size: Some(size),
            last_played: None,
            quiesced: None,
        })
        .collect();

    let mut stored = |name: &str| {
        let file = instance_files
            .iter()
            .find(|f: &&FileEntry| f.path == name)?;
        if file.size > MAX_STORED_FILE {
            // An image's settings can't name a file that large: left out, not fatal.
            notes.push(Note::SettingFileTooLarge(name.to_string()));
            return None;
        }
        Some(StoredFile {
            sha256: file.sha256.clone(),
            size: file.size,
        })
    };
    let client = ClientSettings {
        recommended_memory_mb: instance
            .get(RECOMMENDED_MEMORY)
            .and_then(Value::as_u64)
            .and_then(|mb| u32::try_from(mb).ok()),
        resource_packs: Vec::new(),
        shader: None,
        icon: stored(ICON_FILE),
        background: stored(BACKGROUND_FILE),
    };
    if client != ClientSettings::default() {
        image.settings = Settings {
            server: None,
            client: Some(client),
        };
    }
    // A selection of content only whose files include worlds (a manifest from elsewhere)
    // is a full image: a content image has no worlds.
    if image.kind == Kind::Content && !image.worlds.is_empty() {
        image.kind = Kind::Full;
    }

    let local: Vec<String> = instance
        .keys()
        .filter(|key| {
            ![GAME_VERSION, LOADER, LOADER_VERSION, RECOMMENDED_MEMORY].contains(&key.as_str())
        })
        .cloned()
        .collect();
    if !local.is_empty() {
        notes.push(Note::LocalSettings(local));
    }
    if !instance_files.is_empty() {
        notes.push(Note::OutsideGameFolder(
            instance_files.iter().map(|f| f.path.clone()).collect(),
        ));
    }

    let rest = Rest {
        id: snapshot.id.clone(),
        instance_id: snapshot.instance_id.clone(),
        name,
        note: snapshot.note.clone(),
        selection: snapshot.selection.clone(),
        total_bytes: snapshot.total_bytes,
        new_bytes: snapshot.new_bytes,
        automatic: snapshot.automatic,
        instance: instance.clone(),
        instance_files,
        unmapped,
        wide_times,
        zero_modes,
        wide_modes,
    };
    Ok(Converted {
        image: finish(image)?,
        rest,
        notes,
    })
}

/// The schema-1 manifest of `image` and `rest`: the original one, for an image and rest
/// [`to_image`] made. The instance's game and recommended memory come from the image.
pub fn from_image(image: &Image, rest: &Rest) -> Result<Snapshot, LegacyError> {
    let mut files = Vec::with_capacity(image.entries.len() + rest.instance_files.len());
    for entry in &image.entries {
        let path = DiskLayout::Client
            .to_disk(&entry.path)
            .map_err(|e| LegacyError::Invalid(format!("{}: {e}", entry.path)))?;
        let (category, world) = classify(&path).ok_or_else(|| {
            LegacyError::Invalid(format!("{path} is a file snapshots never take"))
        })?;
        let modified = match rest.wide_times.get(&path) {
            Some(seconds) => Some(*seconds),
            None => entry
                .modified
                .and_then(|nanos| u64::try_from(nanos / 1_000_000_000).ok()),
        };
        let as_converted = |mode: u32| match mode & 0o777 {
            0 => 0o644,
            bits => bits,
        };
        let mode = match rest.wide_modes.get(&path) {
            Some(&wide) if entry.mode == as_converted(wide) => wide,
            _ if rest.zero_modes.contains(&path) && entry.mode == 0o644 => 0,
            _ => entry.mode,
        };
        files.push(FileEntry {
            path,
            category,
            world,
            size: entry.size,
            sha256: entry.sha256.clone(),
            mode,
            modified,
        });
    }
    files.extend(rest.instance_files.iter().cloned());
    files.extend(rest.unmapped.iter().map(|(file, _)| file.clone()));
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let mut instance = rest.instance.clone();
    let loader = match &image.game.loader {
        Loader::Vanilla => None,
        other => Some(other.as_str().to_string()),
    };
    set(
        &mut instance,
        GAME_VERSION,
        Some(Value::from(image.game.minecraft.clone())),
    );
    match loader {
        Some(loader) => set(&mut instance, LOADER, Some(Value::from(loader))),
        // A vanilla instance says "none", or nothing at all.
        None if instance.contains_key(LOADER) => {
            set(&mut instance, LOADER, Some(Value::from("none")))
        }
        None => {}
    }
    set(
        &mut instance,
        LOADER_VERSION,
        image.game.loader_version.clone().map(Value::from),
    );
    let memory = image
        .settings
        .client
        .as_ref()
        .and_then(|c| c.recommended_memory_mb);
    set(&mut instance, RECOMMENDED_MEMORY, memory.map(Value::from));

    Ok(Snapshot {
        schema: SCHEMA,
        id: rest.id.clone(),
        instance_id: rest.instance_id.clone(),
        name: rest
            .name
            .clone()
            .unwrap_or_else(|| image.label.clone().unwrap_or_default()),
        note: rest.note.clone(),
        created: u64::try_from(image.created)
            .map_err(|_| LegacyError::Invalid("its creation time is before 1970".into()))?,
        launcher_version: image.producer.version.clone(),
        instance,
        selection: rest.selection.clone(),
        files,
        total_bytes: rest.total_bytes,
        new_bytes: rest.new_bytes,
        automatic: rest.automatic,
    })
}

/// Sets `key` to `value` in place (keeping its position), or removes it for `None`.
fn set(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    match value {
        Some(value) => {
            map.insert(key.to_string(), value);
        }
        None => {
            map.shift_remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    fn file(path: &str, n: usize) -> FileEntry {
        let (category, world) = classify(path).unwrap_or_else(|| panic!("{path}"));
        FileEntry {
            path: path.to_string(),
            category,
            world,
            size: n as u64,
            sha256: format!("{n:064x}"),
            mode: 0o644,
            modified: Some(1_794_645_624),
        }
    }

    fn snapshot(instance: Value, mut files: Vec<FileEntry>) -> Snapshot {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let Value::Object(instance) = instance else {
            panic!("an instance is an object")
        };
        Snapshot {
            schema: 1,
            id: "20261114-125244-a7d2".into(),
            instance_id: "i".into(),
            name: "Snap".into(),
            note: None,
            created: 1_794_660_764,
            launcher_version: "0.1.0".into(),
            instance,
            selection: Selection {
                categories: [Category::Mods, Category::Worlds].into(),
                worlds: BTreeSet::new(),
            },
            total_bytes: files.iter().map(|f| f.size).sum(),
            new_bytes: 0,
            files,
            automatic: false,
        }
    }

    fn plain() -> Value {
        serde_json::json!({"id": "i", "name": "I", "game_version": "1.20.1", "created": 1, "play_time": 0})
    }

    fn round_trip(s: &Snapshot) -> Converted {
        let converted = to_image(s).unwrap();
        converted.image.verify().unwrap();
        let back = from_image(&converted.image, &converted.rest).unwrap();
        assert_eq!(&back, s);
        assert_eq!(back.to_json(), s.to_json());
        converted
    }

    #[test]
    fn newer_and_malformed_manifests_are_refused() {
        let ok = snapshot(plain(), vec![file("minecraft/mods/a.jar", 1)]);
        Snapshot::from_json(&ok.to_json()).unwrap();
        assert_eq!(
            Snapshot::from_json(br#"{"schema": 2, "files": 7}"#),
            Err(LegacyError::Newer {
                found: 2,
                supported: 1
            })
        );
        assert!(matches!(
            Snapshot::from_json(b"nope"),
            Err(LegacyError::Json(_))
        ));
        assert!(Snapshot::from_json(br#"{"schema": 1}"#).is_err());
        let check = |change: &dyn Fn(&mut Snapshot)| {
            let mut s = ok.clone();
            change(&mut s);
            Snapshot::from_json(&s.to_json())
        };
        assert!(check(&|s| s.files[0].category = Category::Configs).is_err());
        assert!(check(&|s| s.files[0].path = "minecraft/../x".into()).is_err());
        assert!(check(&|s| s.files[0].path = "/minecraft/mods/a.jar".into()).is_err());
        assert!(check(&|s| s.files[0].sha256 = "x".into()).is_err());
        assert!(check(&|s| s.files.push(s.files[0].clone())).is_err());
        assert!(check(&|s| {
            s.instance.shift_remove(GAME_VERSION);
        })
        .is_err());
        assert!(check(&|s| s.files[0].size = MAX_TOTAL_BYTES + 1).is_err());
        let mut v = serde_json::to_value(&ok).unwrap();
        v["instance"] = Value::from("not an object");
        assert!(Snapshot::from_json(v.to_string().as_bytes()).is_err());
    }

    #[test]
    fn a_vanilla_instance_stays_as_written() {
        // Older instances have no `loader`; others say "none".
        let s = snapshot(plain(), vec![file("minecraft/saves/W/level.dat", 1)]);
        let converted = round_trip(&s);
        assert_eq!(converted.image.game.loader, Loader::Vanilla);
        assert_eq!(converted.image.kind, Kind::Full);
        let mut with_none = plain();
        with_none["loader"] = "none".into();
        round_trip(&snapshot(with_none, vec![]));
        let mut quilt = plain();
        quilt["loader"] = "quilt".into();
        quilt["loader_version"] = "0.27.1".into();
        let converted = round_trip(&snapshot(quilt, vec![]));
        assert_eq!(converted.image.game.loader, Loader::Quilt);
    }

    #[test]
    fn names_and_times_an_image_cant_hold_come_back() {
        let mut s = snapshot(plain(), vec![file("minecraft/mods/a.jar", 1)]);
        s.name = "two\nlines".into();
        s.files[0].modified = Some(u64::MAX);
        s.files[0].mode = 0;
        let converted = round_trip(&s);
        assert_eq!(converted.image.label.as_deref(), Some("two lines"));
        assert_eq!(converted.image.entries[0].modified, None);
        // A manifest from before modes were kept: restored as the Launcher did, written
        // back as it was.
        assert_eq!(converted.image.entries[0].mode, 0o644);
        assert!(converted.rest.zero_modes.contains("minecraft/mods/a.jar"));
    }

    #[test]
    fn kinds_follow_the_selection() {
        let kind = |categories: &[Category]| {
            let mut s = snapshot(plain(), vec![]);
            s.selection.categories = categories.iter().copied().collect();
            to_image(&s).unwrap().image.kind
        };
        assert_eq!(kind(&[Category::Mods, Category::Settings]), Kind::Content);
        assert_eq!(kind(&[Category::Worlds, Category::Screenshots]), Kind::Data);
        assert_eq!(kind(&[Category::Configs, Category::Other]), Kind::Full);
        assert_eq!(kind(&[]), Kind::Data);
    }

    fn lodesnap(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, bytes) in entries {
                if let Some(folder) = name.strip_suffix('/') {
                    zip.add_directory(folder, options).unwrap();
                } else {
                    zip.start_file(*name, options).unwrap();
                    zip.write_all(bytes).unwrap();
                }
            }
            zip.finish().unwrap();
        }
        out.into_inner()
    }

    #[test]
    fn lodesnap_files_from_others_are_checked() {
        let s = snapshot(plain(), vec![file("minecraft/mods/a.jar", 1)]);
        let manifest = s.to_json();
        let object = format!("objects/{:064x}", 1);
        let good = lodesnap(&[
            ("manifest.json", &manifest),
            ("objects/", b""),
            (&object, b"x"),
        ]);
        assert_eq!(read_lodesnap(Cursor::new(good)).unwrap(), s);
        let missing = lodesnap(&[("manifest.json", &manifest)]);
        assert!(read_lodesnap(Cursor::new(missing)).is_err());
        let stray = lodesnap(&[
            ("manifest.json", &manifest),
            (&object, b"x"),
            ("run.sh", b""),
        ]);
        assert!(read_lodesnap(Cursor::new(stray)).is_err());
        let folder = lodesnap(&[
            ("manifest.json", &manifest),
            (&object, b"x"),
            ("mods/", b""),
        ]);
        assert!(read_lodesnap(Cursor::new(folder)).is_err());
        let bad_object = lodesnap(&[("manifest.json", &manifest), ("objects/ABC", b"x")]);
        assert!(read_lodesnap(Cursor::new(bad_object)).is_err());
        let no_manifest = lodesnap(&[(&object, b"x")]);
        assert!(read_lodesnap(Cursor::new(no_manifest)).is_err());
        let newer = lodesnap(&[("manifest.json", br#"{"schema": 9}"#)]);
        assert!(matches!(
            read_lodesnap(Cursor::new(newer)),
            Err(LegacyError::Newer { found: 9, .. })
        ));
    }

    #[test]
    fn a_lodesnap_with_a_name_twice_is_refused() {
        let s = snapshot(plain(), vec![file("minecraft/mods/a.jar", 1)]);
        let manifest = s.to_json();
        let object = format!("objects/{:064x}", 1);
        // Zip writers refuse a name twice: write a twin and give it the same name.
        let mut bytes = lodesnap(&[
            ("manifest.json", &manifest),
            ("manifest.jsoX", br#"{"schema": 1}"#),
            (&object, b"x"),
        ]);
        let mut at = 0;
        while let Some(i) = bytes[at..].windows(13).position(|w| w == b"manifest.jsoX") {
            bytes[at + i + 12] = b'n';
            at += i + 13;
        }
        let error = read_lodesnap(Cursor::new(bytes)).unwrap_err().to_string();
        assert!(
            error.contains("inconsistent") || error.contains("twice"),
            "{error}"
        );
    }

    #[test]
    fn a_huge_instance_from_elsewhere_is_refused_without_buffering() {
        // The shape of a `.lodesnap` built to exhaust memory: an unknown field in the
        // instance holding a long array, which deflates a thousand to one.
        let s = snapshot(plain(), vec![file("minecraft/mods/a.jar", 1)]);
        let mut value = serde_json::to_value(&s).unwrap();
        value["instance"]["zz"] = Value::from(vec![0; INSTANCE_BUDGET.values]);
        let error = Snapshot::from_json(value.to_string().as_bytes()).unwrap_err();
        assert!(
            error.to_string().contains("larger than it keeps"),
            "{error}"
        );
        // Unknown fields elsewhere are skipped, not kept.
        let mut value = serde_json::to_value(&s).unwrap();
        value["zz"] = Value::from(vec![0; 100_000]);
        value["files"][0]["zz"] = Value::from(vec![0; 1000]);
        assert_eq!(
            Snapshot::from_json(value.to_string().as_bytes()).unwrap(),
            s
        );
        // And a list past the Launcher's bound is refused while read.
        let mut value = serde_json::to_value(&s).unwrap();
        value["selection"]["worlds"] = Value::from(vec!["w"; MAX_SELECTED_WORLDS + 1]);
        assert!(Snapshot::from_json(value.to_string().as_bytes()).is_err());
    }

    #[test]
    fn manifests_from_elsewhere_convert_and_come_back() {
        // Content-only selection with world files: a full image.
        let mut s = snapshot(
            plain(),
            vec![
                file("minecraft/mods/a.jar", 1),
                file("minecraft/saves/w/level.dat", 2),
            ],
        );
        s.selection.categories = [Category::Mods].into();
        let converted = round_trip(&s);
        assert_eq!(converted.image.kind, Kind::Full);
        // Modes beyond the permission bits are kept in the rest and written back.
        let mut s = snapshot(
            plain(),
            vec![
                file("minecraft/mods/a.jar", 1),
                file("minecraft/mods/b.jar", 2),
            ],
        );
        s.files[0].mode = 0o4755;
        s.files[1].mode = 0o1000;
        let converted = round_trip(&s);
        assert_eq!(converted.image.entries[0].mode, 0o755);
        assert_eq!(converted.image.entries[1].mode, 0o644);
        // An icon too large for the settings is left out of them, with a note.
        let mut icon = file("icon.png", 3);
        icon.size = crate::settings::MAX_STORED_FILE + 1;
        let s = snapshot(plain(), vec![icon]);
        let converted = round_trip(&s);
        assert_eq!(converted.image.settings.client, None);
        assert!(converted
            .notes
            .iter()
            .any(|n| matches!(n, Note::SettingFileTooLarge(name) if name == "icon.png")));
        assert_eq!(converted.rest.objects().count(), 1);
        // The rest can be stored beside the image.
        let json = serde_json::to_vec(&converted.rest).unwrap();
        let rest: Rest = serde_json::from_slice(&json).unwrap();
        assert_eq!(from_image(&converted.image, &rest).unwrap(), s);
    }
}
