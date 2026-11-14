//! The core's backup manifests (schema 1) as data images, and back.
//!
//! A schema-1 backup lists every file of a server folder by its path on the server's disk,
//! SHA-256, size, modification time and execute bit, plus what it skipped. Its objects are
//! already what an image's store holds (zstd frames named by the SHA-256 of their bytes),
//! so converting touches no object: only the listing changes.
//!
//! - Paths go through the server's [`DiskLayout`]: the level (`level-name`) and every other
//!   top-level folder with a `level.dat` are worlds, `saves/<world>/…`, and a Paper or
//!   Spigot server's split level is folded back into one world. A file the layout can't map
//!   (a folder named `saves`, a world name like `mods`) or that an image never records is
//!   withheld, and kept in the rest.
//! - `name` is the label, `game_version` and `flavour` the game, `level_name` the
//!   `level-name` property, `quiesced` each world's, `skipped` the withheld files.
//! - Schema 1 didn't record its exclude patterns: the image's `excludes` stay empty and a
//!   note says so.
//! - The backup's own bookkeeping (its id and server, kind, pinned, live, statistics, the
//!   stat cache's scan time) is not part of an image: the backup store keeps it beside the
//!   image. It is in the [`Rest`].
//!
//! [`from_image`] writes the manifest back: equal to the original, and byte for byte the
//! same once serialized ([`Manifest::to_json`] writes it the way the core does).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{finish, json_object, version_field, LegacyError, Note};
use crate::hex::is_sha256;
use crate::layout::{DiskLayout, ServerLayout};
use crate::manifest::{App, Game, Image, Kind, Loader, Producer, Source, Withheld, WithheldReason};
use crate::manifest::{Entry, World};
use crate::settings::{ServerFlavour, ServerSettings, Settings};
use crate::store::StatCache;

/// The manifest schema this reads.
pub const SCHEMA: u32 = 1;
/// `server.properties`' key for the level, where the image keeps `level_name`.
const LEVEL_NAME: &str = "level-name";
/// The core's limit on a backup's name.
const MAX_NAME_LEN: usize = 100;

// The types below mirror `core/src/backups/types.rs` at commit f5ce27ad (`Manifest`,
// `FileEntry`, `SkippedFile`, `SkipReason`, `BackupKind`), field for field and in the same
// order, so a manifest serializes to the same bytes.

/// One backup: every file it holds, by content hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub instance_uuid: String,
    pub instance_name: String,
    pub game_version: String,
    pub flavour: String,
    pub name: Option<String>,
    pub kind: BackupKind,
    /// Unix seconds.
    pub created: i64,
    /// Unix nanoseconds when the walk began (the stat cache's reference).
    pub scan_started_ns: i128,
    pub pinned: bool,
    pub live: bool,
    pub quiesced: bool,
    pub level_name: Option<String>,
    pub full_scan: bool,
    /// Sorted by path.
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub skipped: Vec<SkippedFile>,
    pub total_bytes: u64,
    pub new_bytes: u64,
    pub stored_bytes: u64,
    pub duration_ms: u64,
    pub core_version: String,
}

/// One file of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative to the server folder, `/`-separated.
    pub path: String,
    pub size: u64,
    pub sha256: String,
    /// Unix nanoseconds.
    pub modified_ns: i128,
    #[serde(default)]
    pub executable: bool,
}

/// A file a backup left out or couldn't read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: SkipReason,
    /// The previous backup's copy of the file was kept in this one.
    #[serde(default)]
    pub carried_over: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Symlink,
    Unreadable,
    KeptChanging,
    BadName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Manual,
    Scheduled,
    BeforeRestore,
}

impl Manifest {
    /// Reads a manifest the way the core does: a newer schema is refused, and paths and
    /// hashes are checked (`core/src/backups/store.rs`, `validate_manifest`, without the
    /// checks against the file's name and folder, which only its store knows).
    pub fn from_json(bytes: &[u8]) -> Result<Manifest, LegacyError> {
        let map = json_object(bytes)?;
        let schema = version_field(&map, "schema")?
            .ok_or_else(|| LegacyError::Invalid("no schema".into()))?;
        if schema > u64::from(SCHEMA) {
            return Err(LegacyError::Newer {
                found: schema,
                supported: u64::from(SCHEMA),
            });
        }
        let manifest: Manifest =
            serde_json::from_slice(bytes).map_err(|e| LegacyError::Invalid(e.to_string()))?;
        manifest.validate().map_err(LegacyError::Invalid)?;
        Ok(manifest)
    }

    /// The manifest as the core writes it: compact JSON.
    pub fn to_json(&self) -> Vec<u8> {
        // Strings, numbers and lists of them: serializing can't fail.
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The SHA-256 of every object this backup uses: what an image store's GC must keep
    /// for it ([`crate::store::gc::GcOptions::also_live`]) while schema-1 manifests remain.
    pub fn objects(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|file| file.sha256.as_str())
    }

    fn validate(&self) -> Result<(), String> {
        if self
            .name
            .as_ref()
            .is_some_and(|n| n.chars().count() > MAX_NAME_LEN)
        {
            return Err("its name is too long".into());
        }
        let mut seen = BTreeSet::new();
        for file in &self.files {
            let path = file.path.as_str();
            if !valid_rel_path(path) {
                return Err(format!("it has an invalid path {path:?}"));
            }
            if path == ".lodestone_config"
                || path
                    .split('/')
                    .any(|part| part.starts_with(".lodestone-restore"))
            {
                return Err(format!("it has a path it can't restore: {path:?}"));
            }
            if !is_sha256(&file.sha256) {
                return Err(format!("it has an invalid hash for {path:?}"));
            }
            if !seen.insert(path) {
                return Err(format!("it has {path:?} twice"));
            }
        }
        Ok(())
    }
}

/// The core's check for a stored path (`valid_rel_path` in `core/src/backups/store.rs`).
fn valid_rel_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part
                    .chars()
                    .any(|c| c.is_control() || c == '\\' || c == ':')
        })
}

/// What a schema-1 manifest has that its image doesn't: the backup's bookkeeping, and the
/// files the image couldn't list.
#[derive(Debug, Clone, PartialEq)]
pub struct Rest {
    pub id: String,
    pub instance_uuid: String,
    pub instance_name: String,
    pub kind: BackupKind,
    pub scan_started_ns: i128,
    pub pinned: bool,
    pub live: bool,
    pub full_scan: bool,
    pub total_bytes: u64,
    pub new_bytes: u64,
    pub stored_bytes: u64,
    pub duration_ms: u64,
    /// The level the disk layout used: `level_name`, or a name no file has when that isn't
    /// a world folder images can name.
    pub layout_level: String,
    /// `quiesced`, when the image has no world to carry it.
    pub quiesced: Option<bool>,
    /// Skipped files whose earlier copy the backup kept.
    pub carried_over: BTreeSet<String>,
    /// Files the image withholds, with the reason it gives.
    pub unmapped: Vec<(FileEntry, WithheldReason)>,
    /// Modification times too far from 1970 for an image's 64 bits, by disk path.
    pub wide_times: BTreeMap<String, i128>,
}

/// A converted backup.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub image: Image,
    pub rest: Rest,
    pub notes: Vec<Note>,
}

/// Reads a schema-1 manifest and converts it.
pub fn read(bytes: &[u8]) -> Result<Converted, LegacyError> {
    to_image(&Manifest::from_json(bytes)?)
}

/// The data image of a schema-1 backup; see the module.
pub fn to_image(manifest: &Manifest) -> Result<Converted, LegacyError> {
    if manifest.game_version.is_empty() {
        return Err(LegacyError::Invalid("it has no game version".into()));
    }
    let mut notes = Vec::new();
    let flavour = ServerFlavour::parse(&manifest.flavour);
    let (layout, level_is_world) = server_layout(manifest, &flavour)?;
    if let (Some(level), false) = (&manifest.level_name, level_is_world) {
        notes.push(Note::LevelNotAWorld(level.clone()));
    }
    let disk = DiskLayout::Server(layout.clone());

    let mut image = Image::new(
        Kind::Data,
        Game {
            minecraft: manifest.game_version.clone(),
            loader: loader_of(&flavour),
            loader_version: None,
        },
    );
    image.created = manifest.created;
    image.producer = Producer {
        app: App::Core,
        version: manifest.core_version.clone(),
    };
    image.label = manifest.name.clone();
    let mut server = ServerSettings {
        flavour: (!manifest.flavour.is_empty()).then(|| flavour.clone()),
        ..ServerSettings::default()
    };
    if let Some(level) = &manifest.level_name {
        server.properties.insert(LEVEL_NAME.into(), level.clone());
    }
    image.settings = Settings {
        server: Some(server),
        client: None,
    };

    let mut unmapped = Vec::new();
    let mut wide_times = BTreeMap::new();
    for file in &manifest.files {
        let reason = match disk.from_disk(&file.path) {
            Ok(Some(path)) => {
                let mut entry = Entry::new(path, file.size, &file.sha256, Source::Object {});
                entry.mode = if file.executable { 0o755 } else { 0o644 };
                entry.modified = i64::try_from(file.modified_ns).ok();
                if entry.modified.is_none() {
                    wide_times.insert(file.path.clone(), file.modified_ns);
                }
                image.entries.push(entry);
                continue;
            }
            Ok(None) => WithheldReason::Excluded,
            Err(_) => WithheldReason::BadName,
        };
        notes.push(Note::NotListed {
            path: file.path.clone(),
            reason: reason.clone(),
        });
        image.withheld.push(Withheld {
            path: file.path.clone(),
            reason: reason.clone(),
        });
        unmapped.push((file.clone(), reason));
    }
    for skipped in &manifest.skipped {
        image.withheld.push(Withheld {
            path: skipped.path.clone(),
            reason: skip_reason(skipped.reason),
        });
    }

    let mut worlds: Vec<&str> = layout.worlds().collect();
    if !level_is_world {
        worlds.retain(|w| *w != layout.level_name());
    }
    for name in &worlds {
        let size = image
            .entries
            .iter()
            .filter(|e| e.world.as_deref() == Some(*name))
            .map(|e| e.size)
            .sum();
        image.worlds.push(World {
            name: name.to_string(),
            level_name: None,
            data_version: None,
            game_version: None,
            size: Some(size),
            last_played: None,
            quiesced: Some(manifest.quiesced),
        });
    }
    notes.push(Note::ExcludesNotRecorded);

    let rest = Rest {
        id: manifest.id.clone(),
        instance_uuid: manifest.instance_uuid.clone(),
        instance_name: manifest.instance_name.clone(),
        kind: manifest.kind,
        scan_started_ns: manifest.scan_started_ns,
        pinned: manifest.pinned,
        live: manifest.live,
        full_scan: manifest.full_scan,
        total_bytes: manifest.total_bytes,
        new_bytes: manifest.new_bytes,
        stored_bytes: manifest.stored_bytes,
        duration_ms: manifest.duration_ms,
        layout_level: layout.level_name().to_string(),
        quiesced: worlds.is_empty().then_some(manifest.quiesced),
        carried_over: manifest
            .skipped
            .iter()
            .filter(|s| s.carried_over)
            .map(|s| s.path.clone())
            .collect(),
        unmapped,
        wide_times,
    };
    Ok(Converted {
        image: finish(image)?,
        rest,
        notes,
    })
}

/// The server disk layout [`to_image`] mapped the backup's paths through. Materializing a
/// converted backup with it (`store::materialize`) puts every file back at the path the
/// backup had, even when its level wasn't a world folder an image can name (the layout's
/// level is then a name no file has, so those files keep their server paths).
pub fn disk_layout(image: &Image, rest: &Rest) -> Result<DiskLayout, LegacyError> {
    let split = image
        .settings
        .server
        .as_ref()
        .and_then(|server| server.flavour.as_ref())
        .is_some_and(|f| f.splits_worlds());
    let others = image
        .worlds
        .iter()
        .map(|w| w.name.clone())
        .filter(|name| *name != rest.layout_level);
    let layout = ServerLayout::new(rest.layout_level.clone(), others, split)
        .map_err(|e| LegacyError::Invalid(format!("its worlds: {e}")))?;
    Ok(DiskLayout::Server(layout))
}

/// The stat cache a schema-1 backup amounts to: its files by server path, with the size,
/// time and hash it read, trusted up to the time its scan began. The core has always used
/// the last backup this way; seeding the image store's first snapshot of a server with it
/// means that snapshot reads only what changed since. Times too far from 1970 for 64 bits
/// are left out (those files are read again), and a scan time that doesn't fit trusts
/// nothing.
pub fn stat_cache(manifest: &Manifest) -> StatCache {
    let Ok(started) = i64::try_from(manifest.scan_started_ns) else {
        return StatCache::new(i64::MIN);
    };
    let mut cache = StatCache::new(started);
    for file in &manifest.files {
        if let Ok(modified_ns) = i64::try_from(file.modified_ns) {
            cache.insert(
                file.path.clone(),
                file.size,
                modified_ns,
                file.sha256.clone(),
            );
        }
    }
    cache
}

/// The schema-1 manifest of `image` and `rest`: the original one, for an image and rest
/// [`to_image`] made.
pub fn from_image(image: &Image, rest: &Rest) -> Result<Manifest, LegacyError> {
    let server = image.settings.server.clone().unwrap_or_default();
    let flavour = server
        .flavour
        .as_ref()
        .map(|f| f.as_str().to_string())
        .unwrap_or_default();
    let disk = disk_layout(image, rest)?;

    let mut files = Vec::with_capacity(image.entries.len() + rest.unmapped.len());
    for entry in &image.entries {
        let path = disk
            .to_disk(&entry.path)
            .map_err(|e| LegacyError::Invalid(format!("{}: {e}", entry.path)))?;
        let modified_ns = rest
            .wide_times
            .get(&path)
            .copied()
            .unwrap_or_else(|| i128::from(entry.modified.unwrap_or(0)));
        files.push(FileEntry {
            path,
            size: entry.size,
            sha256: entry.sha256.clone(),
            modified_ns,
            executable: entry.mode & 0o100 != 0,
        });
    }
    files.extend(rest.unmapped.iter().map(|(file, _)| file.clone()));
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let mut unmapped: Vec<(&str, &WithheldReason)> = rest
        .unmapped
        .iter()
        .map(|(file, reason)| (file.path.as_str(), reason))
        .collect();
    let mut skipped = Vec::new();
    for withheld in &image.withheld {
        if let Some(i) = unmapped
            .iter()
            .position(|(path, reason)| *path == withheld.path && **reason == withheld.reason)
        {
            unmapped.swap_remove(i);
            continue;
        }
        let Some(reason) = skip_reason_back(&withheld.reason) else {
            return Err(LegacyError::Invalid(format!(
                "{} is withheld as {}, which schema 1 can't say",
                withheld.path, withheld.reason
            )));
        };
        skipped.push(SkippedFile {
            path: withheld.path.clone(),
            reason,
            carried_over: rest.carried_over.contains(&withheld.path),
        });
    }
    skipped.sort_by(|a, b| a.path.cmp(&b.path));

    let quiesced = image
        .worlds
        .iter()
        .find_map(|w| w.quiesced)
        .or(rest.quiesced)
        .unwrap_or(true);
    Ok(Manifest {
        schema: SCHEMA,
        id: rest.id.clone(),
        instance_uuid: rest.instance_uuid.clone(),
        instance_name: rest.instance_name.clone(),
        game_version: image.game.minecraft.clone(),
        flavour,
        name: image.label.clone(),
        kind: rest.kind,
        created: image.created,
        scan_started_ns: rest.scan_started_ns,
        pinned: rest.pinned,
        live: rest.live,
        quiesced,
        level_name: server.properties.get(LEVEL_NAME).cloned(),
        full_scan: rest.full_scan,
        files,
        skipped,
        total_bytes: rest.total_bytes,
        new_bytes: rest.new_bytes,
        stored_bytes: rest.stored_bytes,
        duration_ms: rest.duration_ms,
        core_version: image.producer.version.clone(),
    })
}

/// The loader players run with this server flavour: Paper and its kin run vanilla clients.
pub(crate) fn loader_of(flavour: &ServerFlavour) -> Loader {
    match flavour {
        ServerFlavour::Fabric => Loader::Fabric,
        ServerFlavour::Quilt => Loader::Quilt,
        ServerFlavour::Forge => Loader::Forge,
        ServerFlavour::NeoForge => Loader::NeoForge,
        ServerFlavour::Vanilla
        | ServerFlavour::Paper
        | ServerFlavour::Spigot
        | ServerFlavour::Bukkit
        | ServerFlavour::Purpur
        | ServerFlavour::Folia => Loader::Vanilla,
        ServerFlavour::Other(text) => Loader::Other(text.clone()),
    }
}

fn skip_reason(reason: SkipReason) -> WithheldReason {
    match reason {
        SkipReason::Symlink => WithheldReason::Symlink,
        SkipReason::Unreadable => WithheldReason::Unreadable,
        SkipReason::KeptChanging => WithheldReason::KeptChanging,
        SkipReason::BadName => WithheldReason::BadName,
    }
}

fn skip_reason_back(reason: &WithheldReason) -> Option<SkipReason> {
    match reason {
        WithheldReason::Symlink => Some(SkipReason::Symlink),
        WithheldReason::Unreadable => Some(SkipReason::Unreadable),
        WithheldReason::KeptChanging => Some(SkipReason::KeptChanging),
        WithheldReason::BadName => Some(SkipReason::BadName),
        _ => None,
    }
}

/// The server's disk layout as the backup shows it, and whether the level is a world
/// images can name. The worlds are the level and every other top-level folder with a
/// `level.dat` (not the split level's own `_nether` and `_the_end` folders). When the level
/// can't be a world (no `level_name`, or one like `worlds/survival`), the layout gets a level
/// no file is in, so every file keeps its disk path.
fn server_layout(
    manifest: &Manifest,
    flavour: &ServerFlavour,
) -> Result<(ServerLayout, bool), LegacyError> {
    let split = flavour.splits_worlds();
    let tops: BTreeSet<&str> = manifest
        .files
        .iter()
        .filter_map(|f| f.path.split('/').next())
        .collect();
    let real_level = manifest
        .level_name
        .clone()
        .filter(|level| ServerLayout::new(level.clone(), [], split).is_ok());
    let level_is_world = real_level.is_some();
    let level = real_level.unwrap_or_else(|| unused_level(&tops));
    let candidates: BTreeSet<&str> = manifest
        .files
        .iter()
        .filter_map(|f| f.path.strip_suffix("/level.dat"))
        .filter(|folder| !folder.contains('/') && *folder != level)
        .collect();
    // Each candidate on its own, so one folder that can't be a world (`mods/level.dat`)
    // doesn't keep the others from being worlds.
    let others: Vec<String> = candidates
        .into_iter()
        .filter(|world| ServerLayout::new(level.clone(), [world.to_string()], split).is_ok())
        .map(str::to_string)
        .collect();
    let layout = ServerLayout::new(level, others, split)
        .map_err(|e| LegacyError::Invalid(format!("its worlds: {e}")))?;
    Ok((layout, level_is_world))
}

/// A level name that no top-level file or folder has, nor its split folders.
fn unused_level(tops: &BTreeSet<&str>) -> String {
    let free = |name: &str| {
        !tops.contains(name)
            && !tops.contains(format!("{name}_nether").as_str())
            && !tops.contains(format!("{name}_the_end").as_str())
    };
    if free("world") {
        return "world".to_string();
    }
    // There are fewer top-level names than numbers to try, so this ends.
    (1u64..)
        .map(|n| format!("world{n}"))
        .find(|name| free(name))
        .unwrap_or_else(|| "world".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::TestRng;
    use crate::store::statcache::RACY_NS;

    fn hash(n: usize) -> String {
        format!("{n:064x}")
    }

    fn file(path: &str, n: usize) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size: n as u64,
            sha256: hash(n),
            modified_ns: 1_794_645_624_000_000_000 + n as i128,
            executable: false,
        }
    }

    /// A manifest as the core writes them: files sorted by path.
    fn manifest(flavour: &str, level: Option<&str>, mut files: Vec<FileEntry>) -> Manifest {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Manifest {
            schema: 1,
            id: "20261114-101500-ab12".into(),
            instance_uuid: "INSTANCE_x".into(),
            instance_name: "Test".into(),
            game_version: "1.21.1".into(),
            flavour: flavour.into(),
            name: None,
            kind: BackupKind::Scheduled,
            created: 1_794_645_624,
            scan_started_ns: 1_794_645_624_500_000_000,
            pinned: false,
            live: false,
            quiesced: true,
            level_name: level.map(str::to_string),
            full_scan: false,
            files,
            skipped: Vec::new(),
            total_bytes: 0,
            new_bytes: 0,
            stored_bytes: 0,
            duration_ms: 5,
            core_version: "0.6.0".into(),
        }
    }

    /// Converts, writes back, and checks the manifest and its bytes came back.
    fn round_trip(m: &Manifest) -> Converted {
        let converted = to_image(m).unwrap_or_else(|e| panic!("{e}: {m:?}"));
        converted.image.verify().unwrap();
        let back = from_image(&converted.image, &converted.rest).unwrap();
        assert_eq!(&back, m);
        assert_eq!(back.to_json(), m.to_json());
        converted
    }

    #[test]
    fn newer_and_malformed_manifests_are_refused() {
        let ok = manifest("vanilla", Some("world"), vec![file("a", 1)]);
        let json = |m: &Manifest| serde_json::to_value(m).unwrap();
        Manifest::from_json(&ok.to_json()).unwrap();

        // The schema is read first: a newer manifest is refused for it, whatever else
        // changed.
        let newer = br#"{"schema": 2, "files": "something new"}"#;
        assert_eq!(
            Manifest::from_json(newer),
            Err(LegacyError::Newer {
                found: 2,
                supported: 1
            })
        );
        assert!(matches!(
            Manifest::from_json(b"{"),
            Err(LegacyError::Json(_))
        ));
        assert!(matches!(
            Manifest::from_json(b"[]"),
            Err(LegacyError::Invalid(_))
        ));
        assert!(matches!(
            Manifest::from_json(br#"{"id": "x"}"#),
            Err(LegacyError::Invalid(_))
        ));
        let mut v = json(&ok);
        v["schema"] = "1".into();
        assert!(Manifest::from_json(v.to_string().as_bytes()).is_err());
        let mut v = json(&ok);
        v["skipped"] = serde_json::json!([{"path": "x", "reason": "eaten"}]);
        assert!(Manifest::from_json(v.to_string().as_bytes()).is_err());
        for bad in [
            "../x",
            "/etc/passwd",
            "a\\b",
            "C:/x",
            "a//b",
            ".lodestone_config",
            "w/.lodestone-restore-1/x",
        ] {
            let m = manifest("vanilla", None, vec![file(bad, 1)]);
            assert!(Manifest::from_json(&m.to_json()).is_err(), "{bad}");
        }
        let mut bad_hash = ok.clone();
        bad_hash.files[0].sha256 = "ABC".into();
        assert!(Manifest::from_json(&bad_hash.to_json()).is_err());
        let mut twice = manifest("vanilla", None, vec![file("a", 1)]);
        twice.files.push(file("a", 2));
        assert!(Manifest::from_json(&twice.to_json()).is_err());
        let mut long = ok.clone();
        long.name = Some("x".repeat(101));
        assert!(Manifest::from_json(&long.to_json()).is_err());
        let mut no_version = ok;
        no_version.game_version.clear();
        assert!(to_image(&no_version).is_err());
    }

    #[test]
    fn a_level_that_isnt_a_world_folder_keeps_disk_paths() {
        let m = manifest(
            "vanilla",
            Some("worlds/survival"),
            vec![
                file("worlds/survival/level.dat", 1),
                file("worlds/survival/region/r.0.0.mca", 2),
                file("world/notes.txt", 3),
            ],
        );
        let converted = round_trip(&m);
        let image = &converted.image;
        assert!(image.worlds.is_empty());
        assert_eq!(converted.rest.quiesced, Some(true));
        // "world" has files but no level.dat, so the layout's level is another name.
        assert_eq!(converted.rest.layout_level, "world1");
        assert!(image.entry(&CanonicalPath_new("world/notes.txt")).is_some());
        assert!(converted
            .notes
            .contains(&Note::LevelNotAWorld("worlds/survival".into())));
        assert_eq!(
            image.settings.server.as_ref().unwrap().properties[LEVEL_NAME],
            "worlds/survival"
        );
    }

    #[test]
    fn the_disk_layout_puts_every_entry_back_at_its_backup_path() {
        let m = manifest(
            "paper",
            Some("survival"),
            vec![
                file("server.properties", 1),
                file("survival/level.dat", 2),
                file("survival_nether/DIM-1/region/r.0.0.mca", 3),
                file("survival_the_end/DIM1/region/r.0.0.mca", 4),
                file("creative/level.dat", 5),
                file("plugins/a.jar", 6),
            ],
        );
        let converted = round_trip(&m);
        let disk = disk_layout(&converted.image, &converted.rest).unwrap();
        let mut placed: Vec<String> = converted
            .image
            .entries
            .iter()
            .map(|e| disk.to_disk(&e.path).unwrap())
            .collect();
        placed.sort();
        let original: Vec<&str> = m.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(placed, original);

        // A level that isn't a world folder: its files keep their server paths.
        let m = manifest(
            "vanilla",
            Some("worlds/survival"),
            vec![
                file("worlds/survival/level.dat", 1),
                file("world/notes.txt", 2),
            ],
        );
        let converted = round_trip(&m);
        let disk = disk_layout(&converted.image, &converted.rest).unwrap();
        let mut placed: Vec<String> = converted
            .image
            .entries
            .iter()
            .map(|e| disk.to_disk(&e.path).unwrap())
            .collect();
        placed.sort();
        assert_eq!(placed, ["world/notes.txt", "worlds/survival/level.dat"]);
    }

    #[test]
    fn a_backup_is_a_stat_cache_trusted_up_to_its_scan() {
        let mut far = file("far.dat", 3);
        far.modified_ns = i128::from(i64::MAX) + 1;
        let mut racy = file("racy.dat", 4);
        racy.modified_ns = 1_794_645_624_000_000_000;
        let mut m = manifest(
            "vanilla",
            Some("world"),
            vec![file("a", 1), far, racy, file("world/level.dat", 2)],
        );
        m.scan_started_ns = 1_794_645_624_000_000_000 + 10 * i128::from(RACY_NS);
        let cache = stat_cache(&m);
        let a = &m.files[0];
        assert_eq!(a.path, "a");
        assert_eq!(
            cache.lookup("a", a.size, a.modified_ns as i64),
            Some(a.sha256.as_str())
        );
        assert_eq!(cache.lookup("a", a.size + 1, a.modified_ns as i64), None);
        // Out of 64 bits: not cached, read again.
        assert!(!cache.files.contains_key("far.dat"));
        // Paths are the server's, as a snapshot of its folder sees them.
        assert!(cache.files.contains_key("world/level.dat"));
        m.scan_started_ns = 1_794_645_624_000_000_000 + i128::from(RACY_NS);
        let cache = stat_cache(&m);
        assert_eq!(cache.lookup("racy.dat", 4, 1_794_645_624_000_000_000), None);
        // A scan time out of range trusts nothing.
        m.scan_started_ns = i128::MAX;
        assert_eq!(
            stat_cache(&m).lookup("a", a.size, a.modified_ns as i64),
            None
        );
    }

    #[allow(non_snake_case)]
    fn CanonicalPath_new(path: &str) -> crate::paths::CanonicalPath {
        crate::paths::CanonicalPath::new(path).unwrap()
    }

    #[test]
    fn what_an_image_cant_hold_comes_back_from_the_rest() {
        let mut far = file("far-future.dat", 4);
        far.modified_ns = i128::from(i64::MAX) + 1;
        let mut m = manifest(
            "",
            None,
            vec![
                file("logs/latest.log", 1),
                file("mods/a.jar", 2),
                file("unstable.db", 3),
                far,
                file("mods/level.dat", 5),
            ],
        );
        m.skipped = vec![
            SkippedFile {
                path: "unstable.db".into(),
                reason: SkipReason::KeptChanging,
                carried_over: true,
            },
            SkippedFile {
                path: "bad\u{fffd}name".into(),
                reason: SkipReason::BadName,
                carried_over: false,
            },
        ];
        m.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        let converted = round_trip(&m);
        let image = &converted.image;
        assert!(image.settings.server.as_ref().unwrap().flavour.is_none());
        assert!(image.worlds.is_empty(), "mods can't be a world");
        let reasons: Vec<(&str, &str)> = image
            .withheld
            .iter()
            .map(|w| (w.path.as_str(), w.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("bad\u{fffd}name", "bad_name"),
                ("logs/latest.log", "excluded"),
                ("unstable.db", "kept_changing"),
            ]
        );
        assert!(image.entry(&CanonicalPath_new("unstable.db")).is_some());
        let far = image.entry(&CanonicalPath_new("far-future.dat")).unwrap();
        assert_eq!(far.modified, None);
    }

    #[test]
    fn writing_back_refuses_what_schema_1_cant_say() {
        let m = manifest("vanilla", Some("world"), vec![file("world/level.dat", 1)]);
        let mut converted = to_image(&m).unwrap();
        converted.image.withheld.push(Withheld {
            path: "x".into(),
            reason: WithheldReason::CurseforgeNotDownloadable,
        });
        assert!(from_image(&converted.image, &converted.rest).is_err());
    }

    /// Random server folders of every flavour convert and come back unchanged.
    #[test]
    fn random_backups_round_trip() {
        let mut rng = TestRng::new(0x01de_570e);
        let tops = [
            "world",
            "world_nether",
            "world_the_end",
            "creative",
            "mods",
            "config",
            "plugins",
            "saves",
            "logs",
            "server.properties",
            "eula.txt",
            "start.sh",
            "Spawn",
            "CON",
            "café",
            "world1",
        ];
        let inner = [
            "level.dat",
            "region",
            "DIM-1",
            "DIM1",
            "data",
            "x.toml",
            "session.lock",
            "r.0.0.mca",
        ];
        let flavours = [
            "vanilla", "fabric", "paper", "spigot", "forge", "neoforge", "", "folia",
        ];
        let levels = [
            Some("world"),
            Some("creative"),
            None,
            Some("a/b"),
            Some("mods"),
        ];
        let mut tried = 0;
        for _ in 0..300 {
            let mut files = Vec::new();
            for n in 0..rng.below(12) {
                let mut path = rng.pick(&tops).to_string();
                for _ in 0..rng.below(4) {
                    path.push('/');
                    path.push_str(rng.pick(&inner));
                }
                let mut f = file(&path, n);
                f.executable = rng.below(4) == 0;
                files.push(f);
            }
            files.sort_by(|a, b| a.path.cmp(&b.path));
            files.dedup_by(|a, b| a.path == b.path);
            let mut m = manifest(rng.pick(&flavours), *rng.pick(&levels), files);
            m.quiesced = rng.below(2) == 0;
            if m.validate().is_err() {
                continue;
            }
            // A file and a folder of the same name can't both be on a disk.
            let clash = m.files.iter().any(|a| {
                m.files
                    .iter()
                    .any(|b| b.path.starts_with(&format!("{}/", a.path)))
            });
            if clash {
                continue;
            }
            round_trip(&m);
            tried += 1;
        }
        assert!(tried > 150, "only {tried} cases");
    }
}
