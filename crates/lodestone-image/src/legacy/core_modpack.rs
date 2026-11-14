//! The core's modpack record and history (`.lodestone_modpack/record.json` and
//! `history.json`) as a lineage of content images, and back.
//!
//! The history keeps, for each install, update, restore or baseline, the files the pack
//! wrote (path and SHA-1, whether it was an override, the Modrinth project of a download),
//! the pack's origin, the Minecraft version and the server software. The record is the pack
//! files the server has now. Each becomes a content image:
//!
//! - `base` is the pack (`modrinth_pack`) when the origin is on Modrinth and the caller
//!   knows the SHA-512 of its `.mrpack` ([`Known::pack_sha512`]); otherwise there is no base
//!   and a note says so (an uploaded pack, say).
//! - Each file the pack wrote is an entry: a download from Modrinth's CDN has a `modrinth`
//!   source (project and version from its URL, SHA-512 from the history's index), an
//!   override of a pack named as the base a `base` source, anything else `object`. Entries
//!   need a SHA-256 and a size, which the old files never kept, so the caller supplies them
//!   by SHA-1 ([`Known::file`]) from the files on disk and in the old store. A file no one
//!   could hash is withheld (`unreadable`) and kept in the rest.
//! - `game` is the history's version and server software (a Paper server is `vanilla`,
//!   Fabric's loader version and Forge's or NeoForge's build its loader version), and the
//!   flavour is a server setting.
//! - Images are chained by `parent`, oldest first; the label says what the entry was, and
//!   `created` is its time in seconds.
//!
//! The images list only what the pack wrote, which is all the record ever knew: the
//! server's own mods and configs join the lineage with the first content image the core
//! records itself. A disabled mod is listed by its pack path, since the history only names
//! the disabled file for the entry that disabled it.
//!
//! The record and history have no version field and the core ignores fields it doesn't
//! know, so this reader does too: a newer file can't be told apart.
//!
//! [`from_images`] writes both files back from the images and the rest: equal to the
//! originals, and byte for byte the same once serialized ([`PackRecord::to_json`] and
//! [`History::to_json`] write them as the core does).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{clean_text, finish, json_object, Known, LegacyError, Note, UNHASHED};
use crate::hex::{is_sha1, is_sha512};
use crate::manifest::{App, Base, Entry, Game, Image, Kind, Loader, Producer, Source};
use crate::manifest::{Withheld, WithheldReason, MODRINTH_CDN};
use crate::paths::{always_excluded, placement, CanonicalPath};
use crate::settings::{ServerFlavour, ServerSettings, Settings};
use crate::Side;

// The types below mirror `core/src/modpack/update.rs` (`RecordedFile`, `PackRecord`),
// `core/src/modpack/history.rs` (`HistoryKind`, `ChangeAction`, `HistoryChange`,
// `HistoryEntry`, `FileRef`, `History`), `core/src/modpack/mod.rs` (`ModpackOrigin`) and
// `core/src/implementations/minecraft/mod.rs` (`Flavour`, its version newtypes as their
// inner values) at commit f5ce27ad, field for field and in the same order, so the files
// serialize to the same bytes.

/// One file of a pack, as the record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedFile {
    pub sha1: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub from_overrides: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// `record.json`: the files a pack wrote, by path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRecord {
    pub files: BTreeMap<String, RecordedFile>,
}

impl PackRecord {
    pub fn from_json(bytes: &[u8]) -> Result<PackRecord, LegacyError> {
        json_object(bytes)?;
        serde_json::from_slice(bytes).map_err(|e| LegacyError::Invalid(e.to_string()))
    }

    /// The record as the core writes it: pretty JSON.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }
}

/// What an entry was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HistoryKind {
    Install,
    Baseline,
    Update,
    Restore { to: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    Added,
    Replaced,
    Removed,
}

/// One file an entry changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryChange {
    pub path: String,
    pub target: String,
    pub action: ChangeAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// Where a pack came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackOrigin {
    /// Empty for an uploaded pack.
    pub project_id: String,
    /// Empty for an uploaded pack.
    pub version_id: String,
    pub name: String,
    pub version_number: Option<String>,
    pub icon_url: Option<String>,
}

/// The server software, with the versions the core keeps for each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavour {
    Vanilla,
    Fabric {
        loader_version: Option<String>,
        installer_version: Option<String>,
    },
    Paper {
        build_version: Option<i64>,
    },
    Spigot,
    Forge {
        build_version: Option<String>,
    },
    #[serde(rename = "neoforge")]
    NeoForge {
        build_version: Option<String>,
    },
}

impl Flavour {
    /// The flavour's name, as the core writes it in backups.
    fn name(&self) -> &'static str {
        match self {
            Flavour::Vanilla => "vanilla",
            Flavour::Fabric { .. } => "fabric",
            Flavour::Paper { .. } => "paper",
            Flavour::Spigot => "spigot",
            Flavour::Forge { .. } => "forge",
            Flavour::NeoForge { .. } => "neoforge",
        }
    }

    /// The loader players run and its version.
    fn game(&self) -> (Loader, Option<String>) {
        match self {
            Flavour::Vanilla | Flavour::Paper { .. } | Flavour::Spigot => (Loader::Vanilla, None),
            Flavour::Fabric { loader_version, .. } => (Loader::Fabric, loader_version.clone()),
            Flavour::Forge { build_version } => (Loader::Forge, build_version.clone()),
            Flavour::NeoForge { build_version } => (Loader::NeoForge, build_version.clone()),
        }
    }

    /// This flavour with the loader version `version` (from an image).
    fn with_loader_version(&self, version: Option<String>) -> Flavour {
        match self {
            Flavour::Fabric {
                installer_version, ..
            } => Flavour::Fabric {
                loader_version: version,
                installer_version: installer_version.clone(),
            },
            Flavour::Forge { .. } => Flavour::Forge {
                build_version: version,
            },
            Flavour::NeoForge { .. } => Flavour::NeoForge {
                build_version: version,
            },
            other => other.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u32,
    /// Unix milliseconds.
    pub time: i64,
    pub kind: HistoryKind,
    pub origin: ModpackOrigin,
    pub game_version: String,
    pub flavour: Flavour,
    pub files: BTreeMap<String, RecordedFile>,
    #[serde(default)]
    pub changes: Vec<HistoryChange>,
    #[serde(default)]
    pub kept: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
}

/// How to get a file back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRef {
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stored: bool,
}

/// `history.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    /// Oldest first.
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
    /// How to get each file back, by SHA-1.
    #[serde(default)]
    pub files: BTreeMap<String, FileRef>,
}

impl History {
    pub fn from_json(bytes: &[u8]) -> Result<History, LegacyError> {
        json_object(bytes)?;
        serde_json::from_slice(bytes).map_err(|e| LegacyError::Invalid(e.to_string()))
    }

    /// The history as the core writes it: compact JSON.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// What a set of pack files has that its image doesn't.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesRest {
    /// Files the image withholds (no SHA-256 known, or a path an image can't hold).
    pub unresolved: BTreeMap<String, RecordedFile>,
    /// Files whose record the image's entry doesn't give back exactly (an override of a
    /// pack that isn't the base, a SHA-1 in capitals), as they were.
    pub corrections: BTreeMap<String, RecordedFile>,
}

/// What a history entry has that its image doesn't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRest {
    pub id: u32,
    /// Unix milliseconds; the image keeps seconds.
    pub time: i64,
    pub kind: HistoryKind,
    pub origin: ModpackOrigin,
    /// The flavour; the image's loader version replaces its loader or build version.
    pub flavour: Flavour,
    pub changes: Vec<HistoryChange>,
    pub kept: Vec<String>,
    pub changelog: Option<String>,
    pub files: FilesRest,
}

/// What the record and history have that their images don't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rest {
    pub entries: Vec<EntryRest>,
    /// The history's index of how to get files back: the old store's business.
    pub files: BTreeMap<String, FileRef>,
    /// The record's files the image doesn't give back, when there is a record image.
    pub record: Option<FilesRest>,
    /// The record, whole, when there is no history to take the game from (a server from
    /// before histories that was never updated): no image is made of it.
    pub record_without_history: Option<PackRecord>,
}

/// A converted record and history.
#[derive(Debug, Clone, PartialEq)]
pub struct Lineage {
    /// One content image per history entry, oldest first, each the next one's parent.
    pub images: Vec<Image>,
    /// The record's image: the pack files the server has now, on the newest entry's base
    /// and game, its parent the newest entry's image (with the same id when nothing changed
    /// since).
    pub record: Option<Image>,
    pub rest: Rest,
    pub notes: Vec<Note>,
}

/// Reads `record.json` and `history.json` (either may be missing) and converts them.
pub fn read(
    record: Option<&[u8]>,
    history: Option<&[u8]>,
    known: &dyn Known,
) -> Result<Lineage, LegacyError> {
    let record = record.map(PackRecord::from_json).transpose()?;
    let history = history
        .map(History::from_json)
        .transpose()?
        .unwrap_or_default();
    to_images(record.as_ref(), &history, known)
}

/// The lineage of `history`, and the image of `record`; see the module.
pub fn to_images(
    record: Option<&PackRecord>,
    history: &History,
    known: &dyn Known,
) -> Result<Lineage, LegacyError> {
    let mut notes = Vec::new();
    let mut images: Vec<Image> = Vec::with_capacity(history.entries.len());
    let mut entries = Vec::with_capacity(history.entries.len());
    for entry in &history.entries {
        let (loader, loader_version) = entry.flavour.game();
        let mut image = Image::new(
            Kind::Content,
            Game {
                minecraft: entry.game_version.clone(),
                loader,
                loader_version,
            },
        );
        image.created = entry.time.div_euclid(1000);
        image.producer = Producer {
            app: App::Core,
            version: String::new(),
        };
        image.label = Some(clean_text(&label(entry)));
        image.parent = images.last().map(|parent| parent.id.clone());
        image.base = pack_base(&entry.origin, known);
        if image.base.is_none()
            && !notes.iter().any(
                |n| matches!(n, Note::PackNotReferenced { name } if *name == entry.origin.name),
            )
        {
            notes.push(Note::PackNotReferenced {
                name: entry.origin.name.clone(),
            });
        }
        image.settings = Settings {
            server: Some(ServerSettings {
                flavour: Some(ServerFlavour::parse(entry.flavour.name())),
                ..ServerSettings::default()
            }),
            client: None,
        };
        let files = add_files(&mut image, &entry.files, &history.files, known, &mut notes);
        let image = finish(image)
            .map_err(|e| LegacyError::Invalid(format!("history entry {}: {e}", entry.id)))?;
        images.push(image);
        entries.push(EntryRest {
            id: entry.id,
            time: entry.time,
            kind: entry.kind.clone(),
            origin: entry.origin.clone(),
            flavour: entry.flavour.clone(),
            changes: entry.changes.clone(),
            kept: entry.kept.clone(),
            changelog: entry.changelog.clone(),
            files,
        });
    }

    let mut record_image = None;
    let mut record_rest = None;
    let mut record_without_history = None;
    match (record, images.last()) {
        (Some(record), Some(newest)) => {
            let mut image = Image::new(Kind::Content, newest.game.clone());
            image.base = newest.base.clone();
            image.settings = newest.settings.clone();
            image.parent = Some(newest.id.clone());
            image.created = newest.created;
            image.producer = newest.producer.clone();
            image.label = Some("The pack's files now".to_string());
            let files = add_files(&mut image, &record.files, &history.files, known, &mut notes);
            record_image =
                Some(finish(image).map_err(|e| LegacyError::Invalid(format!("the record: {e}")))?);
            record_rest = Some(files);
        }
        (Some(record), None) => record_without_history = Some(record.clone()),
        (None, _) => {}
    }

    Ok(Lineage {
        images,
        record: record_image,
        rest: Rest {
            entries,
            files: history.files.clone(),
            record: record_rest,
            record_without_history,
        },
        notes,
    })
}

/// The record and history of `images`, `record` and `rest`: the originals, for what
/// [`to_images`] made. Each entry's version and loader version come from its image.
pub fn from_images(
    images: &[Image],
    record: Option<&Image>,
    rest: &Rest,
) -> Result<(Option<PackRecord>, History), LegacyError> {
    if images.len() != rest.entries.len() {
        return Err(LegacyError::Invalid(format!(
            "{} images for {} history entries",
            images.len(),
            rest.entries.len()
        )));
    }
    let entries = images
        .iter()
        .zip(&rest.entries)
        .map(|(image, entry)| HistoryEntry {
            id: entry.id,
            time: entry.time,
            kind: entry.kind.clone(),
            origin: entry.origin.clone(),
            game_version: image.game.minecraft.clone(),
            flavour: entry
                .flavour
                .with_loader_version(image.game.loader_version.clone()),
            files: files_of(image, &entry.files),
            changes: entry.changes.clone(),
            kept: entry.kept.clone(),
            changelog: entry.changelog.clone(),
        })
        .collect();
    let record = match (record, &rest.record, &rest.record_without_history) {
        (Some(image), Some(files), _) => Some(PackRecord {
            files: files_of(image, files),
        }),
        (None, None, Some(record)) => Some(record.clone()),
        (None, None, None) => None,
        _ => {
            return Err(LegacyError::Invalid(
                "the record image and its rest don't go together".into(),
            ))
        }
    };
    Ok((
        record,
        History {
            entries,
            files: rest.files.clone(),
        },
    ))
}

/// What an entry was, in words.
fn label(entry: &HistoryEntry) -> String {
    let pack = match &entry.origin.version_number {
        Some(version) => format!("{} {version}", entry.origin.name),
        None => entry.origin.name.clone(),
    };
    match entry.kind {
        HistoryKind::Install => format!("Installed {pack}"),
        HistoryKind::Baseline => format!("Modpack history began with {pack}"),
        HistoryKind::Update => format!("Updated to {pack}"),
        HistoryKind::Restore { to } => format!("Went back to entry {to} ({pack})"),
    }
}

/// The pack as a base, when it is on Modrinth and its `.mrpack` hash is known.
fn pack_base(origin: &ModpackOrigin, known: &dyn Known) -> Option<Base> {
    if origin.project_id.is_empty() || origin.version_id.is_empty() {
        return None;
    }
    let sha512 = known.pack_sha512(&origin.project_id, &origin.version_id)?;
    Some(Base::ModrinthPack {
        project: origin.project_id.clone(),
        version: origin.version_id.clone(),
        sha512,
    })
}

/// The project and version of a `https://cdn.modrinth.com/data/<project>/versions/<version>/…`
/// URL.
pub(crate) fn modrinth_cdn_ids(url: &str) -> Option<(&str, &str)> {
    let rest = url.strip_prefix(MODRINTH_CDN)?.strip_prefix("data/")?;
    let mut parts = rest.split('/');
    let project = parts.next().filter(|p| !p.is_empty())?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next().filter(|v| !v.is_empty())?;
    parts.next().filter(|file| !file.is_empty())?;
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    Some((project, version))
}

/// Adds the entries of a set of pack files to `image`, withholding what can't be listed,
/// and returns what the image doesn't give back.
fn add_files(
    image: &mut Image,
    files: &BTreeMap<String, RecordedFile>,
    index: &BTreeMap<String, FileRef>,
    known: &dyn Known,
    notes: &mut Vec<Note>,
) -> FilesRest {
    let has_base = image.base.is_some();
    let mut rest = FilesRest::default();
    for (path, recorded) in files {
        match entry_of(path, recorded, index, has_base, known) {
            Ok(entry) => {
                if recorded_of(&entry) != *recorded {
                    rest.corrections.insert(path.clone(), recorded.clone());
                }
                image.entries.push(entry);
            }
            Err(reason) => {
                notes.push(Note::NotListed {
                    path: path.clone(),
                    reason: reason.clone(),
                });
                image.withheld.push(Withheld {
                    path: path.clone(),
                    reason,
                });
                rest.unresolved.insert(path.clone(), recorded.clone());
            }
        }
    }
    rest
}

/// The entry of one pack file, or why it can't be one.
fn entry_of(
    path: &str,
    recorded: &RecordedFile,
    index: &BTreeMap<String, FileRef>,
    has_base: bool,
    known: &dyn Known,
) -> Result<Entry, WithheldReason> {
    let path = CanonicalPath::new(path).map_err(|_| WithheldReason::BadName)?;
    if always_excluded(&path) {
        return Err(WithheldReason::Excluded);
    }
    // Pack files are a server's: never client-only, and never a world's (a content image
    // has no worlds).
    if !placement(&path).allows(Side::Server) || path.world().is_some() {
        return Err(WithheldReason::BadName);
    }
    let sha1 = recorded.sha1.to_ascii_lowercase();
    if !is_sha1(&sha1) {
        return Err(UNHASHED);
    }
    let file = known.file(&sha1).ok_or(UNHASHED)?;
    let reference = index.get(&recorded.sha1).or_else(|| index.get(&sha1));
    let sha512 = reference
        .and_then(|r| r.sha512.as_deref())
        .map(str::to_ascii_lowercase)
        .filter(|h| is_sha512(h));
    let modrinth = recorded.project_id.as_deref().and_then(|project| {
        sha512.as_ref()?;
        reference?.urls.iter().find_map(|url| {
            let (p, version) = modrinth_cdn_ids(url)?;
            (p == project).then(|| Source::Modrinth {
                project: p.to_string(),
                version: version.to_string(),
                url: url.clone(),
            })
        })
    });
    let source = match modrinth {
        Some(source) if !recorded.from_overrides => source,
        _ if recorded.from_overrides && has_base => Source::Base {},
        _ => Source::Object {},
    };
    let mut entry = Entry::new(path, file.size, file.sha256, source);
    entry.sha1 = Some(sha1);
    entry.sha512 = sha512;
    Ok(entry)
}

/// The record of an entry, as far as the image says.
fn recorded_of(entry: &Entry) -> RecordedFile {
    RecordedFile {
        sha1: entry.sha1.clone().unwrap_or_default(),
        from_overrides: matches!(entry.source, Source::Base {}),
        project_id: match &entry.source {
            Source::Modrinth { project, .. } => Some(project.clone()),
            _ => None,
        },
    }
}

/// The pack files of `image` and their rest.
fn files_of(image: &Image, rest: &FilesRest) -> BTreeMap<String, RecordedFile> {
    let mut files: BTreeMap<String, RecordedFile> = image
        .entries
        .iter()
        .map(|entry| {
            let path = entry.path.to_string();
            let recorded = rest
                .corrections
                .get(&path)
                .cloned()
                .unwrap_or_else(|| recorded_of(entry));
            (path, recorded)
        })
        .collect();
    files.extend(
        rest.unresolved
            .iter()
            .map(|(path, recorded)| (path.clone(), recorded.clone())),
    );
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::KnownFiles;

    fn sha1(n: u8) -> String {
        format!("{n:02x}").repeat(20)
    }

    fn known(ns: &[u8]) -> KnownFiles {
        let mut known = KnownFiles::new();
        for n in ns {
            known.add_file(&sha1(*n), &format!("{n:02x}").repeat(32), u64::from(*n));
        }
        known
    }

    fn recorded(n: u8, from_overrides: bool, project: Option<&str>) -> RecordedFile {
        RecordedFile {
            sha1: sha1(n),
            from_overrides,
            project_id: project.map(str::to_string),
        }
    }

    fn entry(files: &[(&str, RecordedFile)], origin: ModpackOrigin) -> HistoryEntry {
        HistoryEntry {
            id: 1,
            time: 1_794_660_827_578,
            kind: HistoryKind::Install,
            origin,
            game_version: "1.21.1".into(),
            flavour: Flavour::Paper {
                build_version: Some(120),
            },
            files: files
                .iter()
                .map(|(p, r)| (p.to_string(), r.clone()))
                .collect(),
            changes: Vec::new(),
            kept: Vec::new(),
            changelog: Some("New: *everything*\n".into()),
        }
    }

    fn uploaded() -> ModpackOrigin {
        ModpackOrigin {
            project_id: String::new(),
            version_id: String::new(),
            name: "Mine".into(),
            version_number: None,
            icon_url: None,
        }
    }

    fn round_trip(record: Option<&PackRecord>, history: &History, known: &dyn Known) -> Lineage {
        let lineage = to_images(record, history, known).unwrap();
        let (record_back, history_back) =
            from_images(&lineage.images, lineage.record.as_ref(), &lineage.rest).unwrap();
        assert_eq!(record_back.as_ref(), record);
        assert_eq!(&history_back, history);
        assert_eq!(history_back.to_json(), history.to_json());
        lineage
    }

    #[test]
    fn malformed_files_are_refused() {
        assert!(matches!(
            History::from_json(b"{"),
            Err(LegacyError::Json(_))
        ));
        assert!(matches!(
            History::from_json(b"[]"),
            Err(LegacyError::Invalid(_))
        ));
        assert!(History::from_json(br#"{"entries": [{"id": "one"}]}"#).is_err());
        assert!(PackRecord::from_json(br#"{"files": {"a": {"sha1": 5}}}"#).is_err());
        assert!(PackRecord::from_json(br#"{}"#).is_err());
        // Unknown fields are ignored, as the core ignores them.
        let history = History::from_json(br#"{"entries": [], "files": {}, "future": 1}"#).unwrap();
        assert_eq!(history, History::default());
        let lineage = read(None, None, &()).unwrap();
        assert!(lineage.images.is_empty() && lineage.record.is_none());
    }

    #[test]
    fn records_the_image_cant_say_come_back_as_corrections() {
        let mut upper = recorded(0xab, false, None);
        upper.sha1 = upper.sha1.to_ascii_uppercase();
        let files = [
            // An override of a pack that can't be the base, so its source is an object.
            ("config/a.toml", recorded(2, true, None)),
            // A download from GitHub: not a Modrinth source.
            ("mods/b.jar", recorded(3, false, Some("bbbb"))),
            ("mods/upper.jar", upper),
            // Paths an image can't hold.
            ("options.txt", recorded(4, true, None)),
            ("saves/w/level.dat", recorded(5, true, None)),
            ("../escape", recorded(6, true, None)),
            ("mods/unknown.jar", recorded(7, false, None)),
        ];
        let mut history = History {
            entries: vec![entry(&files, uploaded())],
            files: BTreeMap::new(),
        };
        history.files.insert(
            sha1(3),
            FileRef {
                size: 3,
                sha512: Some("3".repeat(128)),
                urls: vec!["https://github.com/b/b.jar".into()],
                stored: false,
            },
        );
        let lineage = round_trip(None, &history, &known(&[0xab, 2, 3, 4, 5, 6]));
        let image = &lineage.images[0];
        assert_eq!(image.base, None);
        assert_eq!(image.game.loader, Loader::Vanilla);
        assert_eq!(
            image
                .settings
                .server
                .as_ref()
                .unwrap()
                .flavour
                .as_ref()
                .unwrap()
                .as_str(),
            "paper"
        );
        assert!(image.entries.iter().all(|e| e.source == Source::Object {}));
        let withheld: Vec<(&str, &str)> = image
            .withheld
            .iter()
            .map(|w| (w.path.as_str(), w.reason.as_str()))
            .collect();
        assert_eq!(
            withheld,
            [
                ("../escape", "bad_name"),
                ("mods/unknown.jar", "unreadable"),
                ("options.txt", "bad_name"),
                ("saves/w/level.dat", "bad_name"),
            ]
        );
        let corrections = &lineage.rest.entries[0].files.corrections;
        let corrected: Vec<&str> = corrections.keys().map(String::as_str).collect();
        assert_eq!(corrected, ["config/a.toml", "mods/b.jar", "mods/upper.jar"]);
    }

    #[test]
    fn a_record_without_history_is_kept_whole() {
        let record = PackRecord {
            files: [("mods/a.jar".to_string(), recorded(1, false, Some("a")))].into(),
        };
        let lineage = round_trip(Some(&record), &History::default(), &known(&[1]));
        assert!(lineage.record.is_none());
        assert_eq!(lineage.rest.record_without_history, Some(record));
    }

    #[test]
    fn the_loader_version_comes_from_the_image() {
        let mut history = History {
            entries: vec![entry(&[], uploaded())],
            files: BTreeMap::new(),
        };
        history.entries[0].flavour = Flavour::NeoForge {
            build_version: Some("21.1.209".into()),
        };
        let mut lineage = round_trip(None, &history, &());
        assert_eq!(lineage.images[0].game.loader, Loader::NeoForge);
        lineage.images[0].game.loader_version = Some("21.1.210".into());
        let (_, back) = from_images(&lineage.images, None, &lineage.rest).unwrap();
        assert_eq!(
            back.entries[0].flavour,
            Flavour::NeoForge {
                build_version: Some("21.1.210".into())
            }
        );
        assert!(from_images(&[], None, &lineage.rest).is_err());
    }

    #[test]
    fn cdn_urls_name_their_project_and_version() {
        assert_eq!(
            modrinth_cdn_ids("https://cdn.modrinth.com/data/AANobbMI/versions/u1OtTp2l/sodium.jar"),
            Some(("AANobbMI", "u1OtTp2l"))
        );
        for bad in [
            "https://cdn.modrinth.com/data/AANobbMI/versions/u1OtTp2l/",
            "https://cdn.modrinth.com/data/AANobbMI/files/u1OtTp2l/x.jar",
            "https://cdn.modrinth.com.evil.example/data/a/versions/b/x.jar",
            "http://cdn.modrinth.com/data/a/versions/b/x.jar",
            "https://cdn.modrinth.com/data//versions/b/x.jar",
            "https://cdn.modrinth.com/data/a/versions/b/x y.jar",
        ] {
            assert_eq!(modrinth_cdn_ids(bad), None, "{bad}");
        }
    }
}
