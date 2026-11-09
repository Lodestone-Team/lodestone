//! Share codes: an instance's Modrinth content as one string another Lodestone
//! Launcher can recreate the instance from.
//!
//! A code is a prefix and the base64url (no padding) of a deflated JSON payload.
//!
//! - `LDS1:` lists the Minecraft version, the loader, an optional name, and every mod,
//!   resource pack and shader pack that came from Modrinth as `project/version` (pinned
//!   to that exact version). A `-` in front marks a mod or pack that is turned off;
//!   resource packs are listed highest priority first, as the game lists them, and the
//!   shader pack in use has a `*` in front.
//! - `LDS2:` is for an instance made from a Modrinth modpack. It names the pack's
//!   project and version, and then only what differs from it: Modrinth content added on
//!   top, the pack's files that were removed or turned off or on (by their path in the
//!   game folder), the resource packs in use in order and the shader pack in use. The
//!   recipient installs the pack (with the files bundled in it) and applies the rest.
//!
//! Only content Modrinth serves goes in: files added by hand, local folders and
//! CurseForge-only files are left out (see [`SharePlan::left_out`]), and so are
//! worlds, configs, options, accounts and paths. A code is untrusted input: it is
//! limited in size before and after it is inflated, its fields are checked against a
//! strict shape, and installing it downloads only through the usual Modrinth path,
//! which checks every file's hash. The paths of an `LDS2` code are only ever acted on
//! when the installed pack has a file there.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{
    ContentError, ContentKind, InstanceDirs, Modrinth, Target, disabled_name, enabled_name, files,
    install::{self, InstallPlan, InstallProgress, PlannedFile},
    is_disabled_name, listing, options, packs,
    provenance::Provenance,
    scan,
};
use crate::game::download::{CancelToken, Downloader};
use crate::instances::Loader;
use crate::modpack::{FileSource, ModpackRecord};

/// What a code without a modpack starts with; the digit is the format's version.
pub const PREFIX: &str = "LDS1:";
/// What a code based on a Modrinth modpack starts with.
pub const PACK_PREFIX: &str = "LDS2:";

/// The longest code accepted, in characters.
pub const MAX_CODE_CHARS: usize = 256 * 1024;
/// The largest payload a code may inflate to.
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
/// The most items (and paths) a code may list.
pub const MAX_ITEMS: usize = 5000;
const MAX_NAME_CHARS: usize = 100;
const MAX_PATH_CHARS: usize = 255;

/// Why a code can't be read.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShareError {
    #[error("That isn't a Lodestone share code.")]
    NotACode,
    #[error("This share code is from a newer Lodestone Launcher.")]
    NewerFormat,
    #[error("The share code is too long.")]
    TooLarge,
    #[error("The share code is damaged: {0}")]
    Damaged(String),
}

/// One piece of content, pinned to a Modrinth version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedItem {
    pub kind: ContentKind,
    pub project: String,
    pub version: String,
    /// A mod or resource pack that is on, or the shader pack in use.
    pub enabled: bool,
}

/// The Modrinth modpack a code is based on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedPack {
    pub project: String,
    pub version: String,
    /// The pack's title and version number as the sender saw them, to show before
    /// anything is downloaded. Only ever shown; the ids decide what is installed.
    pub title: String,
    pub version_number: String,
}

/// A resource pack or shader pack of a code based on a modpack: one of the pack's files
/// (by its name in its folder) or one of the code's items (by its project).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileRef {
    Pack(String),
    Added(String),
}

/// What differs from the modpack, besides the items added on top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackChanges {
    /// The pack's files and folders that were removed, as `mods/name.jar`.
    pub removed: Vec<String>,
    /// The pack's mods that were turned off, and those the pack has off that were turned
    /// on, by the pack's path.
    pub turned_off: Vec<String>,
    pub turned_on: Vec<String>,
    /// The resource packs in use, highest priority first.
    pub resource_packs: Vec<FileRef>,
    /// The shader pack in use.
    pub shader: Option<FileRef>,
}

/// What a share code holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareCode {
    pub name: Option<String>,
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// The modpack the code is based on. `items` and `changes` are then what differs
    /// from it.
    pub pack: Option<SharedPack>,
    /// Mods, then resource packs (highest priority first), then shader packs.
    pub items: Vec<SharedItem>,
    /// Empty without a pack.
    pub changes: PackChanges,
}

/// The JSON inside an `LDS1` code, with short keys.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    g: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    l: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lv: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    m: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    r: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    s: Vec<String>,
}

/// The JSON inside an `LDS2` code. `m`, `r` and `s` are the items added on top of the
/// pack (`-` marks a mod that is off); whether an added pack is in use is in `ro` and
/// `sh`, which name `f:<file name>` for the pack's files and `m:<project>` for items.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackPayload {
    v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    g: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    l: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lv: Option<String>,
    /// `project/version` of the modpack.
    p: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pv: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    m: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    r: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    s: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    x: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    off: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    on: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ro: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sh: Option<String>,
}

fn deflate_code(prefix: &str, json: &[u8]) -> String {
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = encoder.write_all(json);
    let deflated = encoder.finish().unwrap_or_default();
    format!(
        "{prefix}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(deflated)
    )
}

fn item_entry(item: &SharedItem) -> String {
    format!("{}/{}", item.project, item.version)
}

impl FileRef {
    fn encode(&self) -> String {
        match self {
            FileRef::Pack(name) => format!("f:{name}"),
            FileRef::Added(project) => format!("m:{project}"),
        }
    }

    fn decode(text: &str) -> Option<Self> {
        if let Some(name) = text.strip_prefix("f:") {
            is_file_name(name).then(|| FileRef::Pack(name.to_string()))
        } else {
            let project = text.strip_prefix("m:")?;
            is_modrinth_id(project).then(|| FileRef::Added(project.to_string()))
        }
    }
}

impl ShareCode {
    /// How many items of `kind` it lists (added on top of the pack, for a pack code).
    pub fn count(&self, kind: ContentKind) -> usize {
        self.items.iter().filter(|i| i.kind == kind).count()
    }

    /// How many things differ from the pack: items added, files removed and mods
    /// turned off or on.
    pub fn change_count(&self) -> usize {
        self.items.len()
            + self.changes.removed.len()
            + self.changes.turned_off.len()
            + self.changes.turned_on.len()
    }

    pub fn encode(&self) -> String {
        let entries = |kind: ContentKind, marks: bool| -> Vec<String> {
            self.items
                .iter()
                .filter(|i| i.kind == kind)
                .map(|i| {
                    let mark = match (marks, kind, i.enabled) {
                        (false, ContentKind::Mod, false) => "-",
                        (false, _, _) => "",
                        (true, ContentKind::ShaderPack, true) => "*",
                        (true, ContentKind::ShaderPack, false) => "",
                        (true, _, true) => "",
                        (true, _, false) => "-",
                    };
                    format!("{mark}{}", item_entry(i))
                })
                .collect()
        };
        let Some(pack) = &self.pack else {
            let payload = Payload {
                v: 1,
                n: self.name.clone(),
                g: self.game_version.clone(),
                l: loader_key(self.loader).map(str::to_string),
                lv: self.loader_version.clone(),
                m: entries(ContentKind::Mod, true),
                r: entries(ContentKind::ResourcePack, true),
                s: entries(ContentKind::ShaderPack, true),
            };
            return deflate_code(PREFIX, &serde_json::to_vec(&payload).unwrap_or_default());
        };
        let changes = &self.changes;
        let payload = PackPayload {
            v: 2,
            n: self.name.clone(),
            g: self.game_version.clone(),
            l: loader_key(self.loader).map(str::to_string),
            lv: self.loader_version.clone(),
            p: format!("{}/{}", pack.project, pack.version),
            pt: pack.title.clone(),
            pv: pack.version_number.clone(),
            m: entries(ContentKind::Mod, false),
            r: entries(ContentKind::ResourcePack, false),
            s: entries(ContentKind::ShaderPack, false),
            x: changes.removed.clone(),
            off: changes.turned_off.clone(),
            on: changes.turned_on.clone(),
            ro: changes.resource_packs.iter().map(FileRef::encode).collect(),
            sh: changes.shader.as_ref().map(FileRef::encode),
        };
        deflate_code(
            PACK_PREFIX,
            &serde_json::to_vec(&payload).unwrap_or_default(),
        )
    }

    /// Reads a code, checking everything in it. Whitespace around it (and inside, as
    /// when it was wrapped in a message) is ignored.
    pub fn decode(code: &str) -> Result<Self, ShareError> {
        if code.len() > MAX_CODE_CHARS {
            return Err(ShareError::TooLarge);
        }
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let Some((head, body)) = code.split_once(':') else {
            return Err(ShareError::NotACode);
        };
        let format = match head {
            "LDS1" => 1,
            "LDS2" => 2,
            other if other.starts_with("LDS") && other[3..].parse::<u32>().is_ok() => {
                return Err(ShareError::NewerFormat);
            }
            _ => return Err(ShareError::NotACode),
        };
        let deflated = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| ShareError::Damaged("it isn't valid base64".into()))?;
        let mut json = Vec::new();
        flate2::read::DeflateDecoder::new(deflated.as_slice())
            .take(MAX_PAYLOAD_BYTES as u64 + 1)
            .read_to_end(&mut json)
            .map_err(|_| ShareError::Damaged("it can't be unpacked".into()))?;
        if json.len() > MAX_PAYLOAD_BYTES {
            return Err(ShareError::TooLarge);
        }
        if format == 1 {
            let payload: Payload = serde_json::from_slice(&json)
                .map_err(|err| ShareError::Damaged(err.to_string()))?;
            if payload.v != 1 {
                return Err(ShareError::NewerFormat);
            }
            Self::from_payload(payload)
        } else {
            let payload: PackPayload = serde_json::from_slice(&json)
                .map_err(|err| ShareError::Damaged(err.to_string()))?;
            if payload.v != 2 {
                return Err(ShareError::NewerFormat);
            }
            Self::from_pack_payload(payload)
        }
    }

    fn from_payload(payload: Payload) -> Result<Self, ShareError> {
        let base = Base::check(payload.n, payload.g, payload.l, payload.lv)?;
        if payload.m.len() + payload.r.len() + payload.s.len() > MAX_ITEMS {
            return Err(ShareError::TooLarge);
        }
        let mut items = Vec::new();
        for (kind, list) in [
            (ContentKind::Mod, payload.m),
            (ContentKind::ResourcePack, payload.r),
            (ContentKind::ShaderPack, payload.s),
        ] {
            for entry in list {
                let (enabled, rest) = match (kind, entry.as_bytes().first()) {
                    (ContentKind::ShaderPack, Some(b'*')) => (true, &entry[1..]),
                    (ContentKind::ShaderPack, _) => (false, entry.as_str()),
                    (_, Some(b'-')) => (false, &entry[1..]),
                    _ => (true, entry.as_str()),
                };
                items.push(parse_item(kind, rest, enabled)?);
            }
        }
        if items
            .iter()
            .filter(|i| i.kind == ContentKind::ShaderPack && i.enabled)
            .count()
            > 1
        {
            return Err(damaged("more than one shader pack in use"));
        }
        Ok(base.into_code(None, items, PackChanges::default()))
    }

    fn from_pack_payload(payload: PackPayload) -> Result<Self, ShareError> {
        let base = Base::check(payload.n, payload.g, payload.l, payload.lv)?;
        let total = payload.m.len()
            + payload.r.len()
            + payload.s.len()
            + payload.x.len()
            + payload.off.len()
            + payload.on.len()
            + payload.ro.len();
        if total > MAX_ITEMS {
            return Err(ShareError::TooLarge);
        }
        let (project, version) = payload
            .p
            .split_once('/')
            .filter(|(p, v)| is_modrinth_id(p) && is_modrinth_id(v))
            .ok_or_else(|| damaged("the modpack"))?;
        let pack = SharedPack {
            project: project.to_string(),
            version: version.to_string(),
            title: shown_text(&payload.pt, MAX_NAME_CHARS).ok_or_else(|| damaged("the modpack"))?,
            version_number: shown_text(&payload.pv, 64).ok_or_else(|| damaged("the modpack"))?,
        };
        let resource_packs = payload
            .ro
            .iter()
            .map(|r| FileRef::decode(r))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| damaged("a resource pack in use"))?;
        let shader = match &payload.sh {
            Some(text) => Some(FileRef::decode(text).ok_or_else(|| damaged("the shader pack"))?),
            None => None,
        };
        let mut items = Vec::new();
        for (kind, list) in [
            (ContentKind::Mod, payload.m),
            (ContentKind::ResourcePack, payload.r),
            (ContentKind::ShaderPack, payload.s),
        ] {
            for entry in list {
                let (off, rest) = match (kind, entry.strip_prefix('-')) {
                    (ContentKind::Mod, Some(rest)) => (true, rest),
                    _ => (false, entry.as_str()),
                };
                let mut item = parse_item(kind, rest, !off)?;
                let added = FileRef::Added(item.project.clone());
                item.enabled = match kind {
                    ContentKind::Mod => !off,
                    ContentKind::ResourcePack => resource_packs.contains(&added),
                    ContentKind::ShaderPack => shader.as_ref() == Some(&added),
                };
                items.push(item);
            }
        }
        let paths = |list: Vec<String>, mods_only: bool| -> Result<Vec<String>, ShareError> {
            list.into_iter()
                .map(|path| {
                    pack_path_kind(&path)
                        .filter(|kind| !mods_only || *kind == ContentKind::Mod)
                        .map(|_| path)
                        .ok_or_else(|| damaged("a path"))
                })
                .collect()
        };
        let changes = PackChanges {
            removed: paths(payload.x, false)?,
            turned_off: paths(payload.off, true)?,
            turned_on: paths(payload.on, true)?,
            resource_packs,
            shader,
        };
        Ok(base.into_code(Some(pack), items, changes))
    }
}

fn damaged(what: &str) -> ShareError {
    ShareError::Damaged(what.to_string())
}

/// The name, Minecraft version and loader of a code, checked.
struct Base {
    name: Option<String>,
    game_version: String,
    loader: Loader,
    loader_version: Option<String>,
}

impl Base {
    fn check(
        name: Option<String>,
        game_version: String,
        loader: Option<String>,
        loader_version: Option<String>,
    ) -> Result<Self, ShareError> {
        if !is_version_text(&game_version) {
            return Err(damaged("the Minecraft version"));
        }
        let loader = match loader.as_deref() {
            None => Loader::None,
            Some(key) => loader_from_key(key).ok_or_else(|| damaged("the loader"))?,
        };
        if let Some(version) = &loader_version
            && !is_version_text(version)
        {
            return Err(damaged("the loader version"));
        }
        if loader != Loader::None && loader_version.is_none() {
            return Err(damaged("the loader version"));
        }
        let name = match name {
            Some(name) => {
                let name = shown_text(&name, MAX_NAME_CHARS).ok_or_else(|| damaged("the name"))?;
                (!name.is_empty()).then_some(name)
            }
            None => None,
        };
        Ok(Self {
            name,
            game_version,
            loader,
            loader_version,
        })
    }

    fn into_code(
        self,
        pack: Option<SharedPack>,
        items: Vec<SharedItem>,
        changes: PackChanges,
    ) -> ShareCode {
        ShareCode {
            name: self.name,
            game_version: self.game_version,
            loader: self.loader,
            loader_version: self.loader_version,
            pack,
            items,
            changes,
        }
    }
}

/// Text that is only shown: without control characters, trimmed, at most `max`
/// characters.
fn shown_text(text: &str, max: usize) -> Option<String> {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim().to_string();
    (text.chars().count() <= max).then_some(text)
}

fn parse_item(kind: ContentKind, entry: &str, enabled: bool) -> Result<SharedItem, ShareError> {
    let (project, version) = entry
        .split_once('/')
        .filter(|(p, v)| is_modrinth_id(p) && is_modrinth_id(v))
        .ok_or_else(|| damaged("an item"))?;
    Ok(SharedItem {
        kind,
        project: project.to_string(),
        version: version.to_string(),
        enabled,
    })
}

/// A Modrinth project or version id: short; letters and digits, and `-` or `_` after
/// the first character. Never a path.
fn is_modrinth_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A file or folder name in a content folder: one path part, never `.` or `..`, without
/// separators, drive letters or control characters.
fn is_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_PATH_CHARS
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

/// The kind of a pack path like `mods/name.jar`: a content folder and one name.
fn pack_path_kind(path: &str) -> Option<ContentKind> {
    let (dir, name) = path.split_once('/')?;
    let kind = ContentKind::ALL.into_iter().find(|k| k.dir() == dir)?;
    is_file_name(name).then_some(kind)
}

/// A Minecraft or loader version: short, without separators or anything odd.
fn is_version_text(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+ ".contains(&b))
}

fn loader_key(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::None => None,
        Loader::Fabric => Some("fabric"),
        Loader::Quilt => Some("quilt"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    }
}

fn loader_from_key(key: &str) -> Option<Loader> {
    Some(match key {
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        _ => return None,
    })
}

/// Why a file won't arrive with a share code as it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeftOutReason {
    /// Modrinth doesn't know it (or it is a folder), and there's no modpack it came
    /// from.
    NotOnModrinth,
    /// Not in the modpack and not on Modrinth: added by hand.
    AddedByHand,
    /// One of the modpack's files, changed since: the recipient gets the pack's one.
    ChangedSincePack,
    /// From CurseForge, which a share code can't download from.
    CurseForge,
    /// Not checked against Modrinth yet (offline).
    Unchecked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    pub kind: ContentKind,
    pub file_name: String,
    pub reason: LeftOutReason,
}

/// How a code based on a modpack differs from it, for the dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeCounts {
    /// Modrinth content added on top.
    pub added: usize,
    /// The pack's files removed.
    pub removed: usize,
    /// The pack's files replaced by another Modrinth version.
    pub changed: usize,
    /// The pack's mods turned off or on.
    pub toggled: usize,
}

/// A code for an instance, and what it leaves out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharePlan {
    pub code: ShareCode,
    pub left_out: Vec<LeftOut>,
    /// How many files of each kind arrive as they are: with the pack's, for a pack code.
    pub shared: BTreeMap<ContentKind, usize>,
    /// For a code based on a modpack.
    pub changes: Option<ChangeCounts>,
}

impl SharePlan {
    pub fn shared(&self, kind: ContentKind) -> usize {
        self.shared.get(&kind).copied().unwrap_or(0)
    }
}

/// A top-level file or folder of a content folder the modpack wrote.
#[derive(Debug, Default)]
struct PackEntry {
    is_dir: bool,
    /// Every file, by its path in the game folder, with the pack's SHA-1.
    files: BTreeMap<String, String>,
    /// The Modrinth project of a downloaded file.
    project: Option<String>,
}

/// The pack's files of `kind`, by their name in the content folder.
fn pack_entries(record: &ModpackRecord, kind: ContentKind) -> BTreeMap<String, PackEntry> {
    let prefix = format!("{}/", kind.dir());
    let mut entries: BTreeMap<String, PackEntry> = BTreeMap::new();
    for (path, file) in &record.files {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        let (top, inner) = match rest.split_once('/') {
            Some((top, inner)) => (top, !inner.is_empty()),
            None => (rest, false),
        };
        if !is_file_name(top) {
            continue;
        }
        let entry = entries.entry(top.to_string()).or_default();
        entry.is_dir |= inner;
        entry
            .files
            .insert(path.clone(), file.sha1.to_ascii_lowercase());
        if file.source == FileSource::Index && file.project.is_some() {
            entry.project = file.project.clone();
        }
    }
    entries
}

/// Whether the file or folder `name` of `kind` is still what the pack wrote. `sha1` is
/// the file's, when it is known already.
fn unchanged(
    dirs: &InstanceDirs,
    kind: ContentKind,
    name: &str,
    entry: &PackEntry,
    sha1: Option<&str>,
) -> bool {
    let path = dirs.content_dir(kind).join(name);
    if !entry.is_dir {
        let Some((_, expected)) = entry.files.iter().next() else {
            return false;
        };
        let actual = match sha1 {
            Some(sha1) => Some(sha1.to_ascii_lowercase()),
            None => crate::modpack::import::sha1_on_disk(&path),
        };
        return actual.is_some_and(|a| a.eq_ignore_ascii_case(expected));
    }
    // A folder: the same files, each as the pack wrote it.
    let mut on_disk = Vec::new();
    if !walk_files(&path, &mut on_disk, 0) || on_disk.len() != entry.files.len() {
        return false;
    }
    let base = format!("{}/{name}", kind.dir());
    on_disk.iter().all(|file| {
        let Ok(relative) = file.strip_prefix(&path) else {
            return false;
        };
        let key = format!(
            "{base}/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        entry.files.get(&key).is_some_and(|expected| {
            crate::modpack::import::sha1_on_disk(file).is_some_and(|a| a == *expected)
        })
    })
}

/// The files under `dir`, without following links. False when it can't be read whole.
fn walk_files(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let Ok(kind) = entry.file_type() else {
            return false;
        };
        let path = entry.path();
        if kind.is_dir() {
            if !walk_files(&path, out, depth + 1) {
                return false;
            }
        } else {
            out.push(path);
        }
    }
    true
}

/// The instance's Modrinth modpack, if a code can name it.
fn shareable_pack(dirs: &InstanceDirs) -> Option<ModpackRecord> {
    ModpackRecord::load(&dirs.instance).filter(|record| {
        record.on_modrinth()
            && !record.on_curseforge()
            && is_modrinth_id(&record.project_id)
            && is_modrinth_id(&record.version_id)
    })
}

/// The share code for the instance in `dirs`. With `api`, files not matched to
/// Modrinth yet are looked up first, so they aren't left out by mistake. An instance
/// made from a Modrinth modpack gets a code based on the pack.
pub async fn collect(
    api: Option<&Modrinth>,
    dirs: &InstanceDirs,
    target: &Target,
    name: Option<String>,
    loader: Loader,
    loader_version: Option<String>,
) -> Result<SharePlan, ContentError> {
    let record = shareable_pack(dirs);
    let mut items = Vec::new();
    let mut left_out = Vec::new();
    let mut shared = BTreeMap::new();
    let mut changes = PackChanges::default();
    let mut counts = ChangeCounts::default();
    // The projects of the pack's downloaded files that were removed, to tell an update
    // (removed and added again in another version) from a removal.
    let mut removed_projects = BTreeSet::new();
    for kind in ContentKind::ALL {
        let (files, provenance, _) = listing::refresh(api, dirs, kind).await?;
        let enabled_packs = if kind == ContentKind::ResourcePack {
            options::PackOptions::read(&dirs.game)
                .map(|o| o.enabled_files(&target.game_version))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let shader = if kind == ContentKind::ShaderPack {
            listing::shader_state(dirs)?.selection.pack
        } else {
            None
        };
        let entries = record
            .as_ref()
            .map(|r| pack_entries(r, kind))
            .unwrap_or_default();
        let mut seen = BTreeSet::new();
        // Each file that arrives, and how the code refers to it.
        let mut refs: BTreeMap<String, FileRef> = BTreeMap::new();
        let mut arrive = 0;
        let mut found = Vec::new();
        for file in &files {
            let entry = provenance.current(file);
            let source = entry
                .and_then(|e| e.modrinth.as_ref())
                .filter(|_| !file.is_dir);
            let enabled = match kind {
                ContentKind::Mod => file.enabled_by_name(),
                ContentKind::ResourcePack => enabled_packs.contains(&file.file_name),
                ContentKind::ShaderPack => shader.as_deref() == Some(file.file_name.as_str()),
            };
            let item = |source: &super::provenance::ModrinthSource| SharedItem {
                kind,
                project: source.project_id.clone(),
                version: source.version_id.clone(),
                enabled,
            };
            // The pack's file, by name: a mod may have been turned off or on since.
            let in_pack = [
                file.file_name.clone(),
                enabled_name(&file.file_name),
                disabled_name(&file.file_name),
            ]
            .into_iter()
            .find(|n| (kind == ContentKind::Mod || *n == file.file_name) && entries.contains_key(n));
            if let Some(pack_name) = in_pack {
                seen.insert(pack_name.clone());
                let pack_entry = &entries[&pack_name];
                let pack_path = format!("{}/{pack_name}", kind.dir());
                let sha1 = entry.map(|e| e.sha1.as_str()).filter(|s| !s.is_empty());
                if unchanged(dirs, kind, &file.file_name, pack_entry, sha1) {
                    arrive += 1;
                    if kind == ContentKind::Mod {
                        match (!is_disabled_name(&pack_name), enabled) {
                            (true, false) => changes.turned_off.push(pack_path),
                            (false, true) => changes.turned_on.push(pack_path),
                            _ => {}
                        }
                    }
                    refs.insert(file.file_name.clone(), FileRef::Pack(pack_name));
                    continue;
                }
                if let Some(source) = source {
                    // Replaced by another Modrinth version: the pack's file goes, and
                    // this one comes.
                    arrive += 1;
                    counts.changed += 1;
                    changes.removed.push(pack_path);
                    refs.insert(
                        file.file_name.clone(),
                        FileRef::Added(source.project_id.clone()),
                    );
                    found.push((file.file_name.clone(), item(source)));
                    continue;
                }
                left_out.push(LeftOut {
                    kind,
                    file_name: file.file_name.clone(),
                    reason: LeftOutReason::ChangedSincePack,
                });
                // The recipient has the pack's file under its name.
                refs.insert(file.file_name.clone(), FileRef::Pack(pack_name));
                continue;
            }
            if let Some(source) = source {
                arrive += 1;
                counts.added += 1;
                refs.insert(
                    file.file_name.clone(),
                    FileRef::Added(source.project_id.clone()),
                );
                found.push((file.file_name.clone(), item(source)));
                continue;
            }
            let reason = if entry.is_some_and(|e| e.curseforge.is_some()) {
                LeftOutReason::CurseForge
            } else if !file.is_dir && !entry.is_some_and(|e| e.looked_up) {
                LeftOutReason::Unchecked
            } else if record.is_some() {
                LeftOutReason::AddedByHand
            } else {
                LeftOutReason::NotOnModrinth
            };
            left_out.push(LeftOut {
                kind,
                file_name: file.file_name.clone(),
                reason,
            });
        }
        let on_disk = |name: &str| {
            let dir = dirs.content_dir(kind);
            let names = if kind == ContentKind::Mod {
                vec![enabled_name(name), disabled_name(name)]
            } else {
                vec![name.to_string()]
            };
            names
                .iter()
                .any(|n| fs::symlink_metadata(dir.join(n)).is_ok())
        };
        for (name, entry) in &entries {
            // A file the listing skips (a pack's `.txt` note, say) is still there.
            if !seen.contains(name) && !on_disk(name) {
                changes.removed.push(format!("{}/{name}", kind.dir()));
                counts.removed += 1;
                if let Some(project) = &entry.project {
                    removed_projects.insert((kind, project.clone()));
                }
            }
        }
        if kind == ContentKind::ResourcePack {
            // Enabled packs in the game's order (highest priority first), then the rest.
            let rank = |name: &str| {
                enabled_packs
                    .iter()
                    .position(|n| n == name)
                    .unwrap_or(usize::MAX)
            };
            found.sort_by_key(|(name, _)| rank(name));
            if record.is_some() {
                changes.resource_packs = enabled_packs
                    .iter()
                    .filter_map(|name| refs.get(name).cloned())
                    .collect();
            }
        }
        if kind == ContentKind::ShaderPack && record.is_some() {
            changes.shader = shader.as_ref().and_then(|name| refs.get(name).cloned());
        }
        shared.insert(kind, arrive);
        items.extend(found.into_iter().map(|(_, item)| item));
    }
    // A pack's file removed and its project added in another version is a change.
    for item in &items {
        if removed_projects.remove(&(item.kind, item.project.clone())) {
            counts.changed += 1;
            counts.added -= 1;
            counts.removed -= 1;
        }
    }
    counts.toggled = changes.turned_off.len() + changes.turned_on.len();
    let pack = record.as_ref().map(|r| SharedPack {
        project: r.project_id.clone(),
        version: r.version_id.clone(),
        title: shown_text(&r.title, MAX_NAME_CHARS).unwrap_or_default(),
        version_number: shown_text(&r.version_number, 64).unwrap_or_default(),
    });
    let name = name.and_then(|n| shown_text(&n, MAX_NAME_CHARS)).filter(|n| !n.is_empty());
    Ok(SharePlan {
        changes: pack.is_some().then_some(counts),
        code: ShareCode {
            name,
            game_version: target.game_version.clone(),
            loader,
            loader_version: (loader != Loader::None).then_some(loader_version).flatten(),
            pack,
            items,
            changes,
        },
        left_out,
        shared,
    })
}

/// What installing a code did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShareOutcome {
    /// Titles of what was installed.
    pub installed: Vec<String>,
    /// What couldn't be installed: a title (or the project id) and why.
    pub failed: Vec<(String, String)>,
}

/// The modpack version a code is based on, checked against Modrinth: the project is a
/// modpack and the version is one of its versions.
pub async fn pack_version(
    api: &Modrinth,
    pack: &SharedPack,
) -> Result<
    (
        lodestone_minecraft::modrinth::ModrinthProject,
        lodestone_minecraft::modrinth::RawVersion,
    ),
    ContentError,
> {
    let project = api.project(&pack.project).await?;
    let version = api.version(&pack.version).await?;
    if project.project_type != "modpack" || version.project_id != project.id {
        return Err(ContentError::Invalid(
            "The share code's modpack isn't a modpack on Modrinth.".into(),
        ));
    }
    Ok((project, version))
}

/// Installs the content of `code` into the (new) instance in `dirs`: every listed
/// version exactly, then turns off what was off, orders the resource packs and picks
/// the shader pack. Items Modrinth no longer has are reported, not fatal.
///
/// For a code based on a modpack, the instance must have been made from that pack
/// version first (see [`pack_version`]); the pack's files the code names are removed or
/// turned off or on before the rest is installed.
pub async fn install(
    api: &Modrinth,
    downloader: &Downloader,
    dirs: &InstanceDirs,
    target: &Target,
    code: &ShareCode,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<ShareOutcome, ContentError> {
    if let Some(pack) = &code.pack {
        apply_pack_changes(dirs, target, pack, &code.changes)?;
    }
    let mut outcome = ShareOutcome::default();
    if !code.items.is_empty() {
        let ids: Vec<String> = code.items.iter().map(|i| i.version.clone()).collect();
        let versions = api.versions_by_id(&ids).await?;
        let project_ids: Vec<String> = {
            let mut ids: Vec<String> = code.items.iter().map(|i| i.project.clone()).collect();
            ids.sort();
            ids.dedup();
            ids
        };
        let projects = api.projects(&project_ids).await.unwrap_or_default();
        let mut planned = Vec::new();
        for item in &code.items {
            let project = projects.iter().find(|p| p.id == item.project).cloned();
            let title = project
                .as_ref()
                .map(|p| p.title.clone())
                .unwrap_or_else(|| item.project.clone());
            match versions
                .iter()
                .find(|v| v.id == item.version && v.project_id == item.project)
            {
                Some(version) => planned.push(PlannedFile {
                    kind: item.kind,
                    version: version.clone(),
                    project,
                    depth: 0,
                }),
                None => outcome
                    .failed
                    .push((title, "No longer on Modrinth".to_string())),
            }
        }
        let result = install::execute(
            api,
            downloader,
            dirs,
            target,
            InstallPlan {
                files: planned,
                skipped: Vec::new(),
            },
            progress,
            cancel,
        )
        .await?;
        outcome.installed = result
            .installed
            .into_iter()
            .map(|(title, _)| title)
            .collect();
        outcome
            .failed
            .extend(result.skipped.into_iter().map(|s| (s.project, s.reason)));
    }
    apply_states(dirs, target, code)?;
    Ok(outcome)
}

/// Removes the pack's files the code removed and turns its mods off or on, after
/// checking the instance was made from the code's pack. Paths that aren't the pack's are
/// ignored.
fn apply_pack_changes(
    dirs: &InstanceDirs,
    target: &Target,
    pack: &SharedPack,
    changes: &PackChanges,
) -> Result<(), ContentError> {
    let record = ModpackRecord::load(&dirs.instance)
        .filter(|r| r.project_id == pack.project && r.version_id == pack.version)
        .ok_or_else(|| {
            ContentError::Invalid("The instance wasn't made from the share code's modpack.".into())
        })?;
    let mut tops: BTreeMap<ContentKind, BTreeMap<String, PackEntry>> = BTreeMap::new();
    for kind in ContentKind::ALL {
        tops.insert(kind, pack_entries(&record, kind));
    }
    let of_pack = |path: &str| -> Option<(ContentKind, String)> {
        let kind = pack_path_kind(path)?;
        let (_, name) = path.split_once('/')?;
        tops[&kind].contains_key(name).then(|| (kind, name.to_string()))
    };
    for path in &changes.removed {
        let Some((kind, name)) = of_pack(path) else {
            continue;
        };
        let names = if kind == ContentKind::Mod {
            vec![enabled_name(&name), disabled_name(&name)]
        } else {
            vec![name.clone()]
        };
        for name in names {
            let full = dirs.content_dir(kind).join(&name);
            let Ok(metadata) = fs::symlink_metadata(&full) else {
                continue;
            };
            files::delete_with(dirs, target, kind, &name, |path| {
                if metadata.is_dir() {
                    fs::remove_dir_all(path)
                } else {
                    fs::remove_file(path)
                }
                .map_err(|err| err.to_string())
            })?;
        }
    }
    for (paths, enabled) in [(&changes.turned_off, false), (&changes.turned_on, true)] {
        for path in paths {
            let Some((ContentKind::Mod, name)) = of_pack(path) else {
                continue;
            };
            match files::set_mod_enabled(dirs, &name, enabled) {
                Ok(_) | Err(ContentError::NotFound(_)) => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(())
}

/// Turns off the mods that were off, enables the resource packs in order and picks the
/// shader pack, by the files the install recorded for each project (and, for a pack
/// code, by the pack's file names).
fn apply_states(
    dirs: &InstanceDirs,
    target: &Target,
    code: &ShareCode,
) -> Result<(), ContentError> {
    let provenance = Provenance::load(&dirs.instance);
    let file_of = |kind: ContentKind, project: &str| -> Option<String> {
        provenance
            .by_project(kind)
            .remove(project)
            .and_then(|names| names.into_iter().next())
    };
    for item in code
        .items
        .iter()
        .filter(|i| i.kind == ContentKind::Mod && !i.enabled)
    {
        if let Some(name) = file_of(ContentKind::Mod, &item.project) {
            files::set_mod_enabled(dirs, &name, false)?;
        }
    }
    let packs_scanned = scan::scan(dirs, ContentKind::ResourcePack)?;
    let present = |kind: ContentKind, name: &str| {
        fs::symlink_metadata(dirs.content_dir(kind).join(name)).is_ok()
    };
    // The packs to have in use, highest priority first.
    let (wanted, shader): (Vec<String>, Option<Option<String>>) = if code.pack.is_some() {
        let name_of = |kind: ContentKind, r: &FileRef| match r {
            FileRef::Pack(name) => present(kind, name).then(|| name.clone()),
            FileRef::Added(project) => file_of(kind, project),
        };
        (
            code.changes
                .resource_packs
                .iter()
                .filter_map(|r| name_of(ContentKind::ResourcePack, r))
                .collect(),
            Some(
                code.changes
                    .shader
                    .as_ref()
                    .and_then(|r| name_of(ContentKind::ShaderPack, r)),
            ),
        )
    } else {
        (
            code.items
                .iter()
                .filter(|i| i.kind == ContentKind::ResourcePack && i.enabled)
                .filter_map(|i| file_of(ContentKind::ResourcePack, &i.project))
                .collect(),
            code.items
                .iter()
                .find(|i| i.kind == ContentKind::ShaderPack && i.enabled)
                .and_then(|i| file_of(ContentKind::ShaderPack, &i.project))
                .map(Some),
        )
    };
    let current = options::PackOptions::read(&dirs.game)
        .map(|o| o.enabled_files(&target.game_version))
        .unwrap_or_default();
    if current != wanted && (code.pack.is_some() || !wanted.is_empty()) {
        // Enabling puts a pack on top, so the lowest priority goes first.
        for name in &current {
            files::change_pack(
                dirs,
                target,
                &options::PackChange::Disable { name: name.clone() },
            )?;
        }
        for name in wanted.iter().rev() {
            let formats = packs_scanned
                .iter()
                .find(|f| f.file_name == *name)
                .and_then(|f| f.meta.formats);
            let compatibility = packs::compatibility(formats.as_ref(), &target.game_version);
            files::set_pack_enabled(dirs, target, name, true, compatibility)?;
        }
    }
    if let Some(shader) = shader
        && let Some(loader) = listing::shader_state(dirs)?.loader
    {
        let current = listing::shader_state(dirs)?.selection.pack;
        if current != shader && (code.pack.is_some() || shader.is_some()) {
            files::select_shader(dirs, loader, shader.as_deref())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ShareCode {
        ShareCode {
            name: Some("Survival".into()),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.14".into()),
            items: vec![
                SharedItem {
                    kind: ContentKind::Mod,
                    project: "AANobbMI".into(),
                    version: "rAfhHfow".into(),
                    enabled: true,
                },
                SharedItem {
                    kind: ContentKind::Mod,
                    project: "P7dR8mSH".into(),
                    version: "fapi0922".into(),
                    enabled: false,
                },
                SharedItem {
                    kind: ContentKind::ResourcePack,
                    project: "faithful".into(),
                    version: "f32v1201".into(),
                    enabled: true,
                },
                SharedItem {
                    kind: ContentKind::ShaderPack,
                    project: "complemen".into(),
                    version: "comp2500".into(),
                    enabled: true,
                },
            ],
            pack: None,
            changes: PackChanges::default(),
        }
    }

    fn code_of(json: &str) -> String {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(json.as_bytes()).unwrap();
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(encoder.finish().unwrap())
        )
    }

    #[test]
    fn round_trips() {
        let code = sample();
        let text = code.encode();
        assert!(text.starts_with(PREFIX));
        assert!(
            text[PREFIX.len()..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "URL-safe: {text}"
        );
        assert_eq!(ShareCode::decode(&text).unwrap(), code);
        // wrapped in a message
        let wrapped = format!("  {}\n{}  ", &text[..20], &text[20..]);
        assert_eq!(ShareCode::decode(&wrapped).unwrap(), code);
    }

    #[test]
    fn vanilla_without_loader() {
        let code = ShareCode {
            name: None,
            game_version: "1.21.4".into(),
            loader: Loader::None,
            loader_version: None,
            pack: None,
            items: Vec::new(),
            changes: PackChanges::default(),
        };
        assert_eq!(ShareCode::decode(&code.encode()).unwrap(), code);
    }

    #[test]
    fn a_large_instance_stays_a_few_kilobytes() {
        // 308 mods with random 8-character ids, like a big modpack
        let mut seed = 7u64;
        let mut id = || {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            (0..8)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    CHARS[(seed >> 33) as usize % CHARS.len()] as char
                })
                .collect::<String>()
        };
        let mut code = sample();
        code.items = (0..308)
            .map(|i| SharedItem {
                kind: ContentKind::Mod,
                project: id(),
                version: id(),
                enabled: i % 20 != 0,
            })
            .collect();
        let text = code.encode();
        assert!(text.len() < 8 * 1024, "{} chars", text.len());
        assert_eq!(ShareCode::decode(&text).unwrap(), code);
    }

    #[test]
    fn rejects_what_isnt_a_code() {
        assert_eq!(ShareCode::decode("hello"), Err(ShareError::NotACode));
        assert_eq!(ShareCode::decode("ABC:xyz"), Err(ShareError::NotACode));
        assert_eq!(ShareCode::decode("LDS3:abc"), Err(ShareError::NewerFormat));
        assert!(matches!(
            ShareCode::decode("LDS1:not*base64"),
            Err(ShareError::Damaged(_))
        ));
        assert!(matches!(
            ShareCode::decode("LDS1:AAAA"),
            Err(ShareError::Damaged(_))
        ));
        let too_long = format!("{PREFIX}{}", "A".repeat(MAX_CODE_CHARS));
        assert_eq!(ShareCode::decode(&too_long), Err(ShareError::TooLarge));
    }

    #[test]
    fn rejects_a_zip_bomb() {
        // a tiny code that inflates past the limit
        let json = format!(
            r#"{{"v":1,"g":"1.20.1","n":"{}"}}"#,
            " ".repeat(2 * MAX_PAYLOAD_BYTES)
        );
        let code = code_of(&json);
        assert!(code.len() < 20_000, "{}", code.len());
        assert_eq!(ShareCode::decode(&code), Err(ShareError::TooLarge));
    }

    #[test]
    fn rejects_bad_fields() {
        for json in [
            r#"{"v":2,"g":"1.20.1"}"#,
            r#"{"v":1,"g":"../../etc"}"#,
            r#"{"v":1,"g":"1.20.1","l":"rift","lv":"1"}"#,
            r#"{"v":1,"g":"1.20.1","l":"fabric"}"#,
            r#"{"v":1,"g":"1.20.1","m":["../evil/x"]}"#,
            r#"{"v":1,"g":"1.20.1","m":["only-one-part"]}"#,
            r#"{"v":1,"g":"1.20.1","s":["*aaaa/bbbb","*cccc/dddd"]}"#,
            r#"{"v":1,"g":"1.20.1","extra":true}"#,
        ] {
            assert!(ShareCode::decode(&code_of(json)).is_err(), "{json}");
        }
        assert_eq!(
            ShareCode::decode(&code_of(r#"{"v":2,"g":"1.20.1"}"#)),
            Err(ShareError::NewerFormat)
        );
    }

    #[test]
    fn rejects_too_many_items() {
        let items: Vec<String> = (0..=MAX_ITEMS).map(|i| format!("\"p{i}/v{i}\"")).collect();
        let json = format!(r#"{{"v":1,"g":"1.20.1","m":[{}]}}"#, items.join(","));
        assert_eq!(
            ShareCode::decode(&code_of(&json)),
            Err(ShareError::TooLarge)
        );
    }

    fn pack_code() -> ShareCode {
        let mut code = sample();
        code.pack = Some(SharedPack {
            project: "a0TMbfWJ".into(),
            version: "xcEwT3IK".into(),
            title: "Gaycraft".into(),
            version_number: "5.0".into(),
        });
        // an added mod that is off, an added resource pack in use, a shader pack not
        code.items[1].enabled = false;
        code.items[3].enabled = false;
        code.changes = PackChanges {
            removed: vec!["mods/old.jar".into(), "resourcepacks/A Folder".into()],
            turned_off: vec!["mods/sodium.jar".into()],
            turned_on: vec!["mods/extra.jar.disabled".into()],
            resource_packs: vec![
                FileRef::Pack("§aReimagined.zip".into()),
                FileRef::Added("faithful".into()),
                FileRef::Pack("3D Chiseled Bookshelves".into()),
            ],
            shader: Some(FileRef::Pack("Complementary.zip".into())),
        };
        code
    }

    #[test]
    fn a_pack_code_round_trips() {
        let code = pack_code();
        let text = code.encode();
        assert!(text.starts_with(PACK_PREFIX), "{text}");
        assert_eq!(ShareCode::decode(&text).unwrap(), code);
        assert_eq!(code.change_count(), 4 + 4);
        // without changes it is a few dozen characters
        let mut bare = pack_code();
        bare.items.clear();
        bare.changes = PackChanges::default();
        let text = bare.encode();
        assert!(text.len() < 200, "{} chars", text.len());
        assert_eq!(ShareCode::decode(&text).unwrap(), bare);
    }

    #[test]
    fn a_code_without_a_pack_is_still_lds1() {
        // made by the first version of the format
        let text = "LDS1:q1YqU7Iy1FFKV7JSMtQzMtAzVNJRygFy0hKTijKTQRygAiWguJmeoQmQm6tkFa3k6OiXn5Tk66lf5JiW4ZGWXw6U0A0wTwmyyA320E9LLMg0sDQyUoqtBQA";
        let code = ShareCode::decode(text).unwrap();
        assert_eq!(code.pack, None);
        assert_eq!(code.game_version, "1.20.1");
        assert_eq!(code.loader, Loader::Fabric);
        assert_eq!(code.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(code.items.len(), 2);
        assert!(!code.items[1].enabled);
        assert!(sample().encode().starts_with(PREFIX));
    }

    fn pack_code_of(json: &str) -> String {
        code_of(json).replacen(PREFIX, PACK_PREFIX, 1)
    }

    #[test]
    fn a_pack_code_rejects_bad_fields() {
        let base = r#""v":2,"g":"1.20.1","l":"fabric","lv":"0.16.10","p":"a0TMbfWJ/xcEwT3IK""#;
        assert!(ShareCode::decode(&pack_code_of(&format!("{{{base}}}"))).is_ok());
        for extra in [
            r#","x":["../../etc/passwd"]"#,
            r#","x":["mods/../../x"]"#,
            r#","x":["mods/a/b.jar"]"#,
            r#","x":["config/x.toml"]"#,
            r#","x":["mods/C:\\x"]"#,
            r#","x":["mods/.."]"#,
            r#","x":["mods/"]"#,
            r#","off":["resourcepacks/x.zip"]"#,
            r#","ro":["f:../x"]"#,
            r#","ro":["x"]"#,
            r#","sh":"m:../x""#,
            r#","m":["../a/b"]"#,
            r#","extra":1"#,
        ] {
            assert!(
                ShareCode::decode(&pack_code_of(&format!("{{{base}{extra}}}"))).is_err(),
                "{extra}"
            );
        }
        for bad in [
            r#"{"v":2,"g":"1.20.1","p":"../x"}"#,
            r#"{"v":2,"g":"1.20.1","p":"onlyone"}"#,
            r#"{"v":2,"g":"1.20.1"}"#,
        ] {
            assert!(ShareCode::decode(&pack_code_of(bad)).is_err(), "{bad}");
        }
        assert_eq!(
            ShareCode::decode(&pack_code_of(r#"{"v":3,"g":"1.20.1","p":"a/b"}"#)),
            Err(ShareError::NewerFormat)
        );
    }
}
