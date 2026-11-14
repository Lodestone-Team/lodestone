//! The Launcher's `modpack.json` and `content.json` as a content image, and back.
//!
//! `modpack.json` names the pack an instance was made from (on Modrinth or CurseForge) and
//! the files it wrote; `content.json` is the Launcher's cache of the content files it has
//! seen (mods, resource packs, shader packs), with their hashes and the Modrinth version or
//! CurseForge file each one is. Together they become one content image:
//!
//! - `base` is the pack: `modrinth_pack` when it is on Modrinth and the caller knows the
//!   SHA-512 of its `.mrpack` ([`Known::pack_sha512`]), `curseforge_pack` for a CurseForge
//!   pack, none (with a note) for a pack that is neither, and `loader` for an instance that
//!   wasn't made from a pack (no `modpack.json`).
//! - Each file in either is an entry. A file `content.json` knows on Modrinth's CDN has a
//!   `modrinth` source and its env from the project's `client_side` and `server_side`; one
//!   it knows on CurseForge a `curseforge` source; a pack file still as the pack wrote it a
//!   `base` source; anything else `object`. Entries need a SHA-256, which neither file has,
//!   so the caller supplies it by SHA-1 ([`Known::file`]); a file no one could hash is
//!   withheld (`unreadable`).
//! - `content.json` keys files by their enabled name (`mods/x.jar` for
//!   `mods/x.jar.disabled`). The caller says which files are disabled on disk
//!   ([`Known::disabled`]), and their entries have the disabled name, as the disk does;
//!   writing back keys them by the enabled name again. A file the caller doesn't say is
//!   disabled is listed enabled.
//!
//! The image doesn't have the game: neither file does, so the caller passes the instance's.
//! What the files have beyond the image (titles, slugs, icons, dates, what was looked up,
//! the pack's own SHA-1 of files the user changed) is kept in the rest, and [`from_image`]
//! writes both files back from the image and the rest: equal to the originals, and byte for
//! byte the same once serialized ([`Document::to_json`] writes them as the Launcher does).

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{version_probe, Known, LegacyError, Note, DISABLED, UNHASHED};
use crate::env::{resolve, EnvInputs, SideValues};
use crate::hex::{is_sha1, is_sha512};
use crate::manifest::{App, Base, Entry, Game, Image, Kind, Producer, Source};
use crate::manifest::{Withheld, WithheldReason};
use crate::paths::{always_excluded, placement, CanonicalPath};
use crate::Side;

use super::core_modpack::modrinth_cdn_ids;
use super::finish;

/// The document version both files have, which this reads.
pub const VERSION: u64 = 1;

// The types below mirror `launcher/crates/launcher-core/src/modpack/mod.rs`
// (`ModpackRecord`, `PackFile`, `FileSource`, `CurseForgePack`) and
// `launcher/crates/launcher-core/src/content/provenance.rs` (`Provenance`, `Entry` as
// `ContentEntry`, `ModrinthSource`, `CurseForgeSource`) at commit f5ce27ad, field for field
// and in the same order, so the files serialize to the same bytes. `version_type` is kept as
// text rather than the Launcher's enum.

/// Where a file of the pack came from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    #[default]
    Index,
    Override,
}

/// A file the pack put into the game folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFile {
    /// The pack's SHA-1 of the file (the user may have changed it since).
    pub sha1: String,
    #[serde(default)]
    pub source: FileSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// The CurseForge file a pack was made from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurseForgePack {
    pub project_id: u64,
    pub file_id: u64,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub page_url: String,
}

/// `modpack.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackRecord {
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub files: BTreeMap<String, PackFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge: Option<CurseForgePack>,
}

/// The Modrinth version a file is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModrinthSource {
    pub project_id: String,
    pub version_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_type: Option<String>,
}

/// The CurseForge file a file is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurseForgeSource {
    pub project_id: u64,
    pub file_id: u64,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub file_date: String,
    #[serde(default)]
    pub page_url: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub downloadable: bool,
}

/// What `content.json` knows about one file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentEntry {
    pub size: u64,
    /// Nanoseconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
    pub sha1: String,
    pub sha512: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modrinth: Option<ModrinthSource>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub looked_up: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge: Option<CurseForgeSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub curseforge_looked_up: bool,
}

/// `content.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(default)]
    pub files: BTreeMap<String, ContentEntry>,
}

/// One of the Launcher's versioned documents (`launcher-core/src/storage.rs`): a JSON
/// object with a `"version"` next to the document's fields, none meaning version 0.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document<T> {
    /// The file's version, `None` when it had none.
    pub version: Option<u64>,
    pub document: T,
}

impl<T: Serialize + DeserializeOwned> Document<T> {
    /// Reads a document; a newer version is refused, as the Launcher refuses it. Neither
    /// file has had a migration, so version 0 reads like version 1.
    pub fn from_json(bytes: &[u8]) -> Result<Document<T>, LegacyError> {
        let version = version_probe(bytes, "version")?;
        if let Some(found) = version.filter(|v| *v > VERSION) {
            return Err(LegacyError::Newer {
                found,
                supported: VERSION,
            });
        }
        // The document's types have no `version` field and skip fields they don't know.
        let document =
            serde_json::from_slice(bytes).map_err(|e| LegacyError::Invalid(e.to_string()))?;
        Ok(Document { version, document })
    }

    /// The document as the Launcher writes it: `"version"` first, pretty, a newline last.
    pub fn to_json(&self) -> Vec<u8> {
        let mut out = Map::new();
        if let Some(version) = self.version {
            out.insert("version".into(), version.into());
        }
        if let Ok(Value::Object(fields)) = serde_json::to_value(&self.document) {
            out.extend(fields);
        }
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(out)).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }
}

/// `modpack.json` and `content.json`, each when the instance has it.
pub type Records = (
    Option<Document<ModpackRecord>>,
    Option<Document<Provenance>>,
);

/// The two files, as they were: the image overrides what it carries when writing back.
/// Kept beside the image (serde), never in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rest {
    pub modpack: Option<Document<ModpackRecord>>,
    pub content: Option<Document<Provenance>>,
}

/// A converted pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub image: Image,
    pub rest: Rest,
    pub notes: Vec<Note>,
}

/// Reads `modpack.json` and `content.json` (either may be missing) and converts them for
/// an instance running `game`.
pub fn read(
    modpack: Option<&[u8]>,
    content: Option<&[u8]>,
    game: Game,
    known: &dyn Known,
) -> Result<Converted, LegacyError> {
    let rest = Rest {
        modpack: modpack.map(Document::from_json).transpose()?,
        content: content.map(Document::from_json).transpose()?,
    };
    to_image(rest, game, known)
}

/// The content image of the two files; see the module.
pub fn to_image(rest: Rest, game: Game, known: &dyn Known) -> Result<Converted, LegacyError> {
    let mut notes = Vec::new();
    let mut image = Image::new(Kind::Content, game);
    image.producer = Producer {
        app: App::Launcher,
        version: String::new(),
    };
    let modpack = rest.modpack.as_ref().map(|d| &d.document);
    image.base = match modpack {
        None => Some(Base::Loader {}),
        Some(record) => {
            let modrinth = (!record.project_id.is_empty() && !record.version_id.is_empty())
                .then(|| known.pack_sha512(&record.project_id, &record.version_id))
                .flatten()
                .map(|sha512| Base::ModrinthPack {
                    project: record.project_id.clone(),
                    version: record.version_id.clone(),
                    sha512,
                });
            let base = modrinth.or_else(|| {
                record.curseforge.as_ref().map(|cf| Base::CurseforgePack {
                    project: cf.project_id,
                    file: cf.file_id,
                })
            });
            if base.is_none() {
                notes.push(Note::PackNotReferenced {
                    name: record.title.clone(),
                });
            }
            base
        }
    };
    let pack_is_base = matches!(
        image.base,
        Some(Base::ModrinthPack { .. } | Base::CurseforgePack { .. })
    );
    let empty_pack = BTreeMap::new();
    let empty_content = BTreeMap::new();
    let pack_files = modpack.map_or(&empty_pack, |r| &r.files);
    let content = rest
        .content
        .as_ref()
        .map_or(&empty_content, |d| &d.document.files);
    let mut paths: Vec<&String> = pack_files.keys().chain(content.keys()).collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        let pack = pack_files.get(path);
        let cached = content.get(path);
        // A file disabled on disk is listed by its disabled name, as it is there.
        let on_disk = known.disabled(path).then(|| format!("{path}{DISABLED}"));
        match entry_of(
            on_disk.as_deref().unwrap_or(path),
            pack,
            cached,
            pack_is_base,
            known,
        ) {
            Ok(entry) => image.entries.push(entry),
            Err(reason) => {
                notes.push(Note::NotListed {
                    path: path.clone(),
                    reason: reason.clone(),
                });
                image.withheld.push(Withheld {
                    path: path.clone(),
                    reason,
                });
            }
        }
    }
    Ok(Converted {
        image: finish(image)?,
        rest,
        notes,
    })
}

/// The entry of a file the pack wrote (`pack`) or `content.json` knows (`cached`), or why
/// it can't be one.
fn entry_of(
    path: &str,
    pack: Option<&PackFile>,
    cached: Option<&ContentEntry>,
    pack_is_base: bool,
    known: &dyn Known,
) -> Result<Entry, WithheldReason> {
    let path = CanonicalPath::new(path).map_err(|_| WithheldReason::BadName)?;
    if always_excluded(&path) {
        return Err(WithheldReason::Excluded);
    }
    if !placement(&path).allows(Side::Client) || path.world().is_some() {
        return Err(WithheldReason::BadName);
    }
    // The file as the instance has it: the cache's when it has one, else the pack's.
    let sha1 = cached
        .map(|c| c.sha1.as_str())
        .or(pack.map(|p| p.sha1.as_str()))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !is_sha1(&sha1) {
        return Err(UNHASHED);
    }
    let file = known.file(&sha1).ok_or(UNHASHED)?;
    if cached.is_some_and(|c| c.size != file.size) {
        // The cache describes other bytes than the hash the caller found.
        return Err(UNHASHED);
    }
    let sha512 = cached
        .map(|c| c.sha512.to_ascii_lowercase())
        .filter(|h| is_sha512(h));
    let modrinth = cached.and_then(|c| c.modrinth.as_ref()).and_then(|m| {
        sha512.as_ref()?;
        let url = m.url.as_deref()?;
        let (project, version) = modrinth_cdn_ids(url)?;
        (project == m.project_id && version == m.version_id).then(|| Source::Modrinth {
            project: m.project_id.clone(),
            version: m.version_id.clone(),
            url: url.to_string(),
        })
    });
    let curseforge = cached
        .and_then(|c| c.curseforge.as_ref())
        .map(|cf| Source::Curseforge {
            project: cf.project_id,
            file: cf.file_id,
            downloadable: cf.downloadable,
        });
    let unchanged_pack_file = pack.is_some_and(|p| p.sha1.eq_ignore_ascii_case(&sha1));
    let source = modrinth
        .or(curseforge)
        .unwrap_or(if unchanged_pack_file && pack_is_base {
            Source::Base {}
        } else {
            Source::Object {}
        });
    let mut entry = Entry::new(path, file.size, file.sha256, source);
    entry.sha1 = Some(sha1);
    entry.sha512 = sha512;
    entry.modified = cached
        .and_then(|c| c.modified)
        .and_then(|nanos| i64::try_from(nanos).ok());
    if let Some(m) = cached.and_then(|c| c.modrinth.as_ref()) {
        let project = SideValues::parse(Some(&m.client_side), Some(&m.server_side));
        let (env, env_from) = resolve(&EnvInputs {
            modrinth_project: Some(project),
            ..EnvInputs::default()
        });
        entry.env = env;
        entry.env_from = env_from;
    }
    Ok(entry)
}

/// `modpack.json` and `content.json` from `image` and `rest`: the originals, for an image
/// and rest [`to_image`] made.
///
/// The image gives the pack's identity (from its base), each unchanged pack file's SHA-1,
/// and for every file `content.json` knows its size, time, hashes and Modrinth or
/// CurseForge ids. A file the image no longer has (and doesn't withhold) leaves
/// `content.json`; the pack's own list stays as the pack wrote it. Titles, slugs, dates and
/// lookups come from the rest.
pub fn from_image(image: &Image, rest: &Rest) -> Result<Records, LegacyError> {
    // By the name the files key it with, or disabled on disk (see `Known::disabled`).
    let entry = |path: &str| {
        let find = |path: &str| {
            CanonicalPath::new(path)
                .ok()
                .and_then(|path| image.entry(&path))
        };
        find(path).or_else(|| find(&format!("{path}{DISABLED}")))
    };
    let withheld = |path: &str| image.withheld.iter().any(|w| w.path == path);

    let modpack = rest.modpack.clone().map(|mut doc| {
        let record = &mut doc.document;
        match &image.base {
            Some(Base::ModrinthPack {
                project, version, ..
            }) => {
                record.project_id = project.clone();
                record.version_id = version.clone();
            }
            Some(Base::CurseforgePack { project, file }) => {
                if let Some(cf) = &mut record.curseforge {
                    cf.project_id = *project;
                    cf.file_id = *file;
                }
            }
            _ => {}
        }
        for (path, file) in &mut record.files {
            if let Some(entry) = entry(path).filter(|e| matches!(e.source, Source::Base {})) {
                if let Some(sha1) = &entry.sha1 {
                    if !file.sha1.eq_ignore_ascii_case(sha1) {
                        file.sha1 = sha1.clone();
                    }
                }
            }
        }
        doc
    });

    let content = rest.content.clone().map(|mut doc| {
        doc.document
            .files
            .retain(|path, _| entry(path).is_some() || withheld(path));
        for (path, cached) in &mut doc.document.files {
            let Some(entry) = entry(path) else { continue };
            cached.size = entry.size;
            match entry.modified {
                Some(nanos) => cached.modified = u64::try_from(nanos).ok(),
                // An image can't hold a time past 2262; keep such a time as it was.
                None if cached.modified.is_some_and(|m| i64::try_from(m).is_ok()) => {
                    cached.modified = None
                }
                None => {}
            }
            if let Some(sha1) = &entry.sha1 {
                if !cached.sha1.eq_ignore_ascii_case(sha1) {
                    cached.sha1 = sha1.clone();
                }
            }
            if let Some(sha512) = &entry.sha512 {
                if !cached.sha512.eq_ignore_ascii_case(sha512) {
                    cached.sha512 = sha512.clone();
                }
            }
            match &entry.source {
                Source::Modrinth {
                    project,
                    version,
                    url,
                } => {
                    let m = cached.modrinth.get_or_insert_with(ModrinthSource::default);
                    m.project_id = project.clone();
                    m.version_id = version.clone();
                    m.url = Some(url.clone());
                }
                Source::Curseforge {
                    project,
                    file,
                    downloadable,
                } => {
                    let cf = cached
                        .curseforge
                        .get_or_insert_with(CurseForgeSource::default);
                    cf.project_id = *project;
                    cf.file_id = *file;
                    cf.downloadable = *downloadable;
                }
                Source::Base {} | Source::Object {} => {}
            }
        }
        doc
    });
    Ok((modpack, content))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::KnownFiles;
    use crate::manifest::Loader;

    fn sha1(n: u8) -> String {
        format!("{n:02x}").repeat(20)
    }

    fn sha512(n: u8) -> String {
        format!("{n:02x}").repeat(64)
    }

    fn known(ns: &[u8]) -> KnownFiles {
        let mut known = KnownFiles::new();
        for n in ns {
            known.add_file(&sha1(*n), &format!("{n:02x}").repeat(32), u64::from(*n));
        }
        known
    }

    fn game() -> Game {
        Game {
            minecraft: "1.20.1".into(),
            loader: Loader::Forge,
            loader_version: Some("47.3.0".into()),
        }
    }

    fn cached(n: u8) -> ContentEntry {
        ContentEntry {
            size: u64::from(n),
            sha1: sha1(n),
            sha512: sha512(n),
            ..ContentEntry::default()
        }
    }

    fn curseforge_pack() -> ModpackRecord {
        ModpackRecord {
            title: "All the Mods".into(),
            files: [
                (
                    "mods/jei.jar".to_string(),
                    PackFile {
                        sha1: sha1(1),
                        source: FileSource::Index,
                        project: None,
                    },
                ),
                (
                    "config/jei.toml".to_string(),
                    PackFile {
                        sha1: sha1(2),
                        source: FileSource::Override,
                        project: None,
                    },
                ),
            ]
            .into(),
            curseforge: Some(CurseForgePack {
                project_id: 925200,
                file_id: 6000000,
                file_date: "2026-01-01".into(),
                page_url: "https://www.curseforge.com/minecraft/modpacks/atm".into(),
            }),
            ..ModpackRecord::default()
        }
    }

    fn round_trip(modpack: Option<&[u8]>, content: Option<&[u8]>, known: &KnownFiles) -> Converted {
        let converted = read(modpack, content, game(), known).unwrap();
        converted.image.verify().unwrap();
        let (m, c) = from_image(&converted.image, &converted.rest).unwrap();
        assert_eq!(m.map(|d| d.to_json()).as_deref(), modpack);
        assert_eq!(c.map(|d| d.to_json()).as_deref(), content);
        converted
    }

    #[test]
    fn newer_and_malformed_documents_are_refused() {
        type Pack = Document<ModpackRecord>;
        assert_eq!(
            Pack::from_json(br#"{"version": 2, "files": []}"#),
            Err(LegacyError::Newer {
                found: 2,
                supported: 1
            })
        );
        assert!(matches!(Pack::from_json(b"{"), Err(LegacyError::Json(_))));
        assert!(matches!(
            Pack::from_json(b"1"),
            Err(LegacyError::Invalid(_))
        ));
        assert!(Pack::from_json(br#"{"version": "1"}"#).is_err());
        assert!(Pack::from_json(br#"{"version": 1, "files": {"a": {"sha1": 1}}}"#).is_err());
        assert!(Document::<Provenance>::from_json(br#"{"files": {"a": {"size": 1}}}"#).is_err());
        // Version 0 (no version) reads like 1 and is written back without one.
        let doc = Pack::from_json(br#"{"title": "Old"}"#).unwrap();
        assert_eq!(doc.version, None);
        assert_eq!(doc.document.title, "Old");
        assert!(!String::from_utf8(doc.to_json())
            .unwrap()
            .contains("version\""));
    }

    #[test]
    fn a_curseforge_pack_is_the_base() {
        let modpack = Document {
            version: Some(1),
            document: curseforge_pack(),
        }
        .to_json();
        let converted = round_trip(Some(&modpack), None, &known(&[1, 2]));
        let image = &converted.image;
        assert_eq!(
            image.base,
            Some(Base::CurseforgePack {
                project: 925200,
                file: 6000000
            })
        );
        assert!(image.entries.iter().all(|e| e.source == Source::Base {}));
        assert!(image.entries.iter().all(|e| e.sha512.is_none()));
    }

    #[test]
    fn a_pack_that_cant_be_the_base_lists_its_files_as_objects() {
        let mut record = curseforge_pack();
        record.curseforge = None;
        record.project_id = "proj".into();
        record.version_id = "ver".into();
        let modpack = Document {
            version: Some(1),
            document: record,
        }
        .to_json();
        // No `.mrpack` hash known: not a base.
        let converted = round_trip(Some(&modpack), None, &known(&[1, 2]));
        assert_eq!(converted.image.base, None);
        assert!(converted
            .image
            .entries
            .iter()
            .all(|e| e.source == Source::Object {}));
        assert!(converted.notes.contains(&Note::PackNotReferenced {
            name: "All the Mods".into()
        }));
    }

    #[test]
    fn stale_or_unknown_files_are_withheld() {
        let mut stale = cached(3);
        stale.size = 99;
        let mut not_cdn = cached(4);
        not_cdn.modrinth = Some(ModrinthSource {
            project_id: "p".into(),
            version_id: "v".into(),
            url: Some("https://example.com/x.jar".into()),
            client_side: "required".into(),
            server_side: "optional".into(),
            ..ModrinthSource::default()
        });
        let content = Document {
            version: Some(1),
            document: Provenance {
                files: [
                    ("mods/stale.jar".to_string(), stale),
                    ("mods/unknown.jar".to_string(), cached(5)),
                    ("mods/elsewhere.jar".to_string(), not_cdn),
                    ("logs/x.jar".to_string(), cached(4)),
                ]
                .into(),
            },
        }
        .to_json();
        let converted = round_trip(None, Some(&content), &known(&[3, 4]));
        let image = &converted.image;
        let withheld: Vec<(&str, &str)> = image
            .withheld
            .iter()
            .map(|w| (w.path.as_str(), w.reason.as_str()))
            .collect();
        assert_eq!(
            withheld,
            [
                ("logs/x.jar", "excluded"),
                ("mods/stale.jar", "unreadable"),
                ("mods/unknown.jar", "unreadable"),
            ]
        );
        // Not on Modrinth's CDN: an object, though its project still says where it runs.
        let elsewhere = &image.entries[0];
        assert_eq!(elsewhere.source, Source::Object {});
        assert_eq!(elsewhere.env.server, crate::env::EnvValue::Optional);
    }

    #[test]
    fn writing_back_follows_an_edited_image() {
        let content = Document {
            version: Some(1),
            document: Provenance {
                files: [
                    ("mods/a.jar".to_string(), cached(1)),
                    ("mods/b.jar".to_string(), cached(2)),
                ]
                .into(),
            },
        };
        let converted = read(None, Some(&content.to_json()), game(), &known(&[1, 2])).unwrap();
        let mut image = converted.image.clone();
        image.entries.retain(|e| e.path.as_str() != "mods/b.jar");
        image.entries[0].sha1 = Some(sha1(9));
        image.seal();
        let (_, back) = from_image(&image, &converted.rest).unwrap();
        let files = back.unwrap().document.files;
        assert_eq!(files.len(), 1);
        assert_eq!(files["mods/a.jar"].sha1, sha1(9));
    }
}
