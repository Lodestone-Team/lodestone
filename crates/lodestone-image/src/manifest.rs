//! The image manifest (schema 2) and its parts.
//!
//! Field names are the stable names of `docs/instance-images.md`. Reading is forward
//! compatible within a schema: fields this version doesn't know are kept in `extra` (on the
//! image and on each entry) and written back as they were, so the image's id still
//! verifies; informational values it doesn't know (a withheld reason, an `env_from` input,
//! a loader) are kept as text. Values that change what a file is or where it comes from (an
//! unknown `source`, `kind`, `base` or `env` value) can't be guessed and make the manifest
//! invalid, and a newer schema is refused with [`ManifestError::NewerSchema`], so nothing
//! acts on a manifest it only half understands.
//!
//! Only the image and its entries have room for unknown fields. Anywhere else (the game,
//! the producer, the base, a source, an env, a world, a withheld file) an unknown field
//! makes the manifest invalid with an error that names it, rather than being dropped: a
//! dropped field would change the id, and the image would then fail its check with a
//! misleading "contents hash to …" instead. So a new field there needs a new schema. (Settings drop
//! unknown fields instead, so local settings can never get into an image.) Numbers in
//! unknown fields must be whole numbers that fit 64 bits: this version can't write any
//! other number back exactly as it was, so its id wouldn't verify.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::env::{Env, EnvFrom};
use crate::hex::{is_sha1, is_sha256, is_sha512};
use crate::paths::{always_excluded, validate_component, CanonicalPath, MAX_PATH_BYTES};
use crate::settings::Settings;
use crate::strings::open_enum;
use crate::Side;

mod probe;
mod read;

pub(crate) use probe::{whole_number_field, ProbeError};

pub use read::MAX_SETTING_ITEMS;
pub(crate) use read::{
    bounded_list, capped_fields, capped_value, entry_list, null_as_default, setting_list,
    setting_map, ENTRY_FIELDS, IMAGE_FIELDS,
};
pub use read::{
    prescan, UnknownBudget, ENTRIES_UNKNOWN, ENTRY_MAP_COST, ENTRY_UNKNOWN, IMAGE_UNKNOWN,
    MAX_JSON_DEPTH, MAX_JSON_VALUES, MAX_UNKNOWN_DEPTH,
};

/// The manifest schema this version writes and reads.
pub const SCHEMA: u32 = 2;
/// The version of the view and env rules this version implements ([`mod@crate::view`]).
pub const RULES: u32 = 1;
/// The first version of the view and env rules: what a manifest without `rules` means,
/// whatever [`RULES`] is now.
pub const FIRST_RULES: u32 = 1;
/// An image lists at most this many entries (and withholds at most this many files).
pub const MAX_ENTRIES: usize = 1_000_000;
/// An image lists at most this many worlds.
pub const MAX_WORLDS: usize = 10_000;
/// An entry's `env_from` lists at most this many inputs (there are five today).
pub const MAX_ENV_FROM: usize = 16;
/// At most this many exclude patterns, each at most [`MAX_EXCLUDE_LEN`] characters.
pub const MAX_EXCLUDES: usize = 1000;
pub const MAX_EXCLUDE_LEN: usize = 256;
/// Free text (a label, a version string) is at most this many characters.
pub const MAX_TEXT_LEN: usize = 1000;
/// Modrinth sources must download from Modrinth's CDN.
pub const MODRINTH_CDN: &str = "https://cdn.modrinth.com/";

/// Why a manifest can't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("the manifest isn't readable JSON: {0}")]
    Json(String),
    #[error("the manifest has schema {found}, newer than schema {SCHEMA} that this version reads")]
    NewerSchema { found: u64 },
    #[error("the manifest has schema {found}; older formats have their own readers")]
    OlderSchema { found: u64 },
    #[error("the manifest is invalid: {0}")]
    Invalid(String),
}

/// What an image holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Mods, configs, versions, settings: recorded at every change.
    Content,
    /// Worlds and everything else: recorded by backups and snapshots, linked to the content
    /// image they ran with.
    Data,
    /// Both.
    Full,
}

open_enum! {
    /// The program that recorded an image.
    pub enum App {
        Core => "core",
        Launcher => "launcher",
    }
}

/// Who recorded an image. Informational: not part of the id. An unknown field in it makes
/// the manifest invalid, like anywhere else outside the image and its entries, and `null`
/// reads as the default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Producer {
    #[serde(default = "no_app", deserialize_with = "null_app")]
    pub app: App,
    #[serde(default, deserialize_with = "null_as_default")]
    pub version: String,
}

fn no_app() -> App {
    App::Other(String::new())
}

fn null_app<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<App, D::Error> {
    Ok(Option::<App>::deserialize(deserializer)?.unwrap_or_else(no_app))
}

impl Default for Producer {
    fn default() -> Self {
        Producer {
            app: no_app(),
            version: String::new(),
        }
    }
}

/// A whole pack the instance is built on, both sides of it. Each view reads its own share
/// (`overrides/` plus `server-overrides/` or `client-overrides/`, index files by `env`).
/// The pack's files are not listed in `entries` unless the instance differs there.
///
/// Unknown fields inside a base make the manifest invalid (see the module documentation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Base {
    ModrinthPack {
        project: String,
        version: String,
        /// Of the `.mrpack` file.
        sha512: String,
    },
    CurseforgePack {
        project: u64,
        file: u64,
    },
    /// Nothing but the image's `game`: a plain instance.
    Loader {},
}

open_enum! {
    /// The mod loader. A Paper or Spigot server is `vanilla` here (players run vanilla);
    /// its flavour is a server setting.
    pub enum Loader {
        Vanilla => "vanilla",
        Fabric => "fabric",
        Quilt => "quilt",
        Forge => "forge",
        NeoForge => "neoforge",
    }
}

/// The game both sides run. The Minecraft version and the loader are fixed when an
/// instance is made; only the loader version changes in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub minecraft: String,
    pub loader: Loader,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
}

/// Where an entry's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    /// The base pack's copy, changed nowhere. Carried in an export as nothing.
    Base {},
    /// A Modrinth file from its CDN, checked against the entry's `sha512`. Carried as a
    /// reference.
    Modrinth {
        project: String,
        version: String,
        url: String,
    },
    /// A CurseForge file. Carried as a reference, and listed for the user when its author
    /// doesn't allow downloads by other apps.
    Curseforge {
        project: u64,
        file: u64,
        downloadable: bool,
    },
    /// Bytes only this store has. Carried as the object.
    Object {},
}

/// One file of an image. Read by hand, so unknown fields are kept within a budget
/// ([`ENTRY_UNKNOWN`]) instead of being buffered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub path: CanonicalPath,
    pub size: u64,
    /// SHA-256 of the contents; the store key.
    pub sha256: String,
    /// When a source publishes it (Modrinth, CurseForge).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    /// When a source publishes it; required for Modrinth sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
    /// Unix permission bits (at most 0o777: no setuid, setgid or sticky); Windows keeps
    /// only read-only (0o444 against 0o644). 0o644 when missing.
    pub mode: u32,
    /// Unix nanoseconds. Informational: not part of the id, and views leave it out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<i64>,
    pub source: Source,
    pub env: Env,
    /// The inputs that decided `env`, sorted ([`crate::env::resolve`]).
    pub env_from: Vec<EnvFrom>,
    /// The world, for files in `saves/<world>/`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    /// Fields this version doesn't know, kept as they were.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub(crate) fn default_mode() -> u32 {
    0o644
}

impl Entry {
    /// An entry with the usual defaults: mode 0o644, no timestamp, `env` unknown, and the
    /// world taken from the path.
    pub fn new(path: CanonicalPath, size: u64, sha256: impl Into<String>, source: Source) -> Entry {
        let world = path.world().map(str::to_string);
        Entry {
            path,
            size,
            sha256: sha256.into(),
            sha1: None,
            sha512: None,
            mode: default_mode(),
            modified: None,
            source,
            env: Env::default(),
            env_from: Vec::new(),
            world,
            extra: BTreeMap::new(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        let path = &self.path;
        if !is_sha256(&self.sha256) {
            return Err(format!("{path}: sha256 isn't 64 lowercase hex characters"));
        }
        if self.sha1.as_deref().is_some_and(|h| !is_sha1(h)) {
            return Err(format!("{path}: sha1 isn't 40 lowercase hex characters"));
        }
        if self.sha512.as_deref().is_some_and(|h| !is_sha512(h)) {
            return Err(format!("{path}: sha512 isn't 128 lowercase hex characters"));
        }
        if self.mode > 0o777 {
            return Err(format!(
                "{path}: mode has bits other than read, write and execute"
            ));
        }
        if always_excluded(path) {
            return Err(format!("{path}: is never recorded in an image"));
        }
        if self.world.as_deref() != path.world() {
            return Err(format!("{path}: world doesn't match the path"));
        }
        match &self.source {
            Source::Modrinth {
                project,
                version,
                url,
            } => {
                if project.is_empty() || version.is_empty() {
                    return Err(format!(
                        "{path}: Modrinth source without project or version"
                    ));
                }
                if !url.starts_with(MODRINTH_CDN) || url.chars().any(char::is_control) {
                    return Err(format!("{path}: Modrinth source not on {MODRINTH_CDN}"));
                }
                if self.sha512.is_none() {
                    return Err(format!("{path}: Modrinth source without sha512"));
                }
            }
            Source::Curseforge { .. } | Source::Base {} | Source::Object {} => {}
        }
        Ok(())
    }
}

/// A world of a data or full image. `name` is its folder (`saves/<name>/`); the rest is
/// read from its `level.dat` where known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct World {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    /// Bytes of its files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<i64>,
    /// Saved and not written to while it was copied (always for a stopped server).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiesced: Option<bool>,
}

open_enum! {
    /// Why a known file isn't carried.
    pub enum WithheldReason {
        /// Its CurseForge author doesn't allow downloads by other apps.
        CurseforgeNotDownloadable => "curseforge_not_downloadable",
        /// A symbolic link or junction, never followed.
        Symlink => "symlink",
        /// Couldn't be opened or read.
        Unreadable => "unreadable",
        /// Its name isn't a valid canonical path.
        BadName => "bad_name",
        /// Left out by the image's excludes.
        Excluded => "excluded",
        /// Changed while being read, every try.
        KeptChanging => "kept_changing",
    }
}

/// A known file an image doesn't carry. `path` is as found, which may not be a valid
/// canonical path (that may be why it was withheld): show it, never place it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Withheld {
    pub path: String,
    pub reason: WithheldReason,
}

/// An instance image. See the crate documentation and `docs/instance-images.md`. Read by
/// hand ([`Image::from_json`]), with bounds on every list and on unknown fields
/// ([`IMAGE_UNKNOWN`]), so a manifest from elsewhere can't make reading it cost far more
/// memory than its size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Image {
    pub schema: u32,
    /// SHA-256 of the canonical manifest ([`crate::identity`]); empty until
    /// [`Image::seal`].
    pub id: String,
    pub kind: Kind,
    /// The image this one was recorded after. Lineage, informational: not part of the id.
    pub parent: Option<String>,
    /// Unix seconds. Not part of the id.
    pub created: i64,
    /// Not part of the id.
    pub producer: Producer,
    /// What made it ("Added JEI"). Not part of the id.
    pub label: Option<String>,
    pub base: Option<Base>,
    pub game: Game,
    pub settings: Settings,
    /// Sorted by path.
    pub entries: Vec<Entry>,
    /// Base files deliberately taken out.
    pub removed: Vec<CanonicalPath>,
    /// Data and full images.
    pub worlds: Vec<World>,
    /// Data images: the content image they ran with.
    pub content: Option<String>,
    /// What this image left out on purpose (the backup's exclude patterns).
    pub excludes: Vec<String>,
    /// Known files not carried, and why.
    pub withheld: Vec<Withheld>,
    /// The version of the view and env rules the entries' `env` follows.
    pub rules: u32,
    /// Set on a view: the side it is for. Part of the id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<Side>,
    /// Set on a view: the id of the image it was computed from. Informational, like
    /// `parent`: not part of the id, so equal views have equal ids whatever they came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projected_from: Option<String>,
    /// Fields this version doesn't know, kept as they were.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Image {
    /// An empty, unsealed image of this schema and rules.
    pub fn new(kind: Kind, game: Game) -> Image {
        Image {
            schema: SCHEMA,
            id: String::new(),
            kind,
            parent: None,
            created: 0,
            producer: Producer::default(),
            label: None,
            base: None,
            game,
            settings: Settings::default(),
            entries: Vec::new(),
            removed: Vec::new(),
            worlds: Vec::new(),
            content: None,
            excludes: Vec::new(),
            withheld: Vec::new(),
            rules: RULES,
            side: None,
            projected_from: None,
            extra: BTreeMap::new(),
        }
    }

    /// Reads and validates a manifest. Its shape is checked first ([`prescan`]) and every
    /// list and unknown field is read within a bound, so a hostile manifest costs little
    /// more memory than its size. Lists are normalized ([`Image::normalize`]); the id
    /// is not checked here (see [`Image::verify`]), since a store's own manifests are
    /// trusted and checking costs a hash of the whole manifest.
    pub fn from_json(bytes: &[u8]) -> Result<Image, ManifestError> {
        // Serde would read a struct from a JSON array too; a manifest is an object.
        let first = bytes.iter().find(|b| !b.is_ascii_whitespace());
        if first.is_some_and(|&b| b != b'{') {
            return Err(ManifestError::Json(
                "the manifest isn't a JSON object".into(),
            ));
        }
        // Only the schema first, so a newer manifest is refused for its schema and not for
        // whatever else changed. Probed without building JSON values, so a huge value in
        // `schema` costs no memory.
        let schema = match whole_number_field(bytes, "schema") {
            Ok(Some(schema)) => schema,
            Ok(None) => return Err(ManifestError::Invalid("no schema".into())),
            Err(ProbeError::Shape(e)) => {
                return Err(ManifestError::Invalid(format!("the manifest {e}")))
            }
            Err(ProbeError::Json(e)) => return Err(ManifestError::Json(e)),
            Err(ProbeError::NotObject) => {
                return Err(ManifestError::Json(
                    "the manifest isn't a JSON object".into(),
                ))
            }
            Err(ProbeError::NotWhole) => {
                return Err(ManifestError::Invalid("schema isn't a whole number".into()))
            }
            Err(ProbeError::NotNumber) => {
                return Err(ManifestError::Invalid("schema isn't a number".into()))
            }
        };
        if schema > u64::from(SCHEMA) {
            return Err(ManifestError::NewerSchema { found: schema });
        }
        if schema < u64::from(SCHEMA) {
            return Err(ManifestError::OlderSchema { found: schema });
        }
        let mut image: Image =
            serde_json::from_slice(bytes).map_err(|e| ManifestError::Invalid(e.to_string()))?;
        image.normalize();
        image.validate().map_err(ManifestError::Invalid)?;
        Ok(image)
    }

    /// The manifest as pretty JSON, the way stores write it.
    pub fn to_json_pretty(&self) -> Vec<u8> {
        // An image is maps with string keys and plain values: serializing can't fail.
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    /// Sorts every list whose order means nothing, so equal images serialize alike:
    /// entries by path (and each entry's `env_from`), removed paths, worlds by name,
    /// excludes, withheld files, and the settings' sets. Repeats are dropped from the lists
    /// that are sets; repeated entry paths are left for [`Image::validate`] to refuse.
    pub fn normalize(&mut self) {
        // Sorting allocates scratch space for half the list even when it is sorted already,
        // as a read or sealed image is: only sort when needed.
        if !self.entries.is_sorted_by(|a, b| a.path <= b.path) {
            self.entries.sort_by(|a, b| a.path.cmp(&b.path));
        }
        for entry in &mut self.entries {
            entry.env_from.sort();
            entry.env_from.dedup();
        }
        if !self.removed.is_sorted() {
            self.removed.sort();
        }
        self.removed.dedup();
        if !self.worlds.is_sorted_by(|a, b| a.name <= b.name) {
            self.worlds.sort_by(|a, b| a.name.cmp(&b.name));
        }
        self.excludes.sort();
        self.excludes.dedup();
        if !self.withheld.is_sorted() {
            self.withheld.sort();
        }
        self.withheld.dedup();
        self.settings.normalize();
    }

    /// Checks everything a manifest from elsewhere could get wrong: hashes and ids are
    /// lowercase hex, no path is listed twice or is both a file and a folder, no entry is a
    /// path that is never recorded, worlds and sources are consistent, sizes are bounded,
    /// and the settings carry no secrets.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema {} isn't {SCHEMA}", self.schema));
        }
        let hash_field = |name: &str, value: Option<&str>| match value {
            Some(id) if !is_sha256(id) => Err(format!("{name} isn't an image id")),
            _ => Ok(()),
        };
        hash_field("id", Some(self.id.as_str()).filter(|id| !id.is_empty()))?;
        hash_field("parent", self.parent.as_deref())?;
        hash_field("content", self.content.as_deref())?;
        hash_field("projected_from", self.projected_from.as_deref())?;
        if !self.id.is_empty() && self.parent.as_deref() == Some(self.id.as_str()) {
            // Images with the same contents share an id, so an image recorded with nothing
            // changed would name itself: its parent is the one before that.
            return Err("parent is the image itself".into());
        }
        if self.projected_from.is_some() && self.side.is_none() {
            return Err("projected_from without side".into());
        }
        if self.rules == 0 {
            return Err("rules must be at least 1".into());
        }
        check_text("label", self.label.as_deref().unwrap_or_default())?;
        check_text("producer.version", &self.producer.version)?;
        check_text("game.minecraft", &self.game.minecraft)?;
        if self.game.minecraft.is_empty() {
            return Err("game.minecraft is empty".into());
        }
        check_text(
            "game.loader_version",
            self.game.loader_version.as_deref().unwrap_or_default(),
        )?;
        match &self.base {
            Some(Base::ModrinthPack {
                project,
                version,
                sha512,
            }) => {
                if project.is_empty() || version.is_empty() || !is_sha512(sha512) {
                    return Err("base.modrinth_pack needs project, version and sha512".into());
                }
            }
            Some(Base::CurseforgePack { .. } | Base::Loader {}) | None => {}
        }
        self.settings.validate()?;
        check_unknown("the image", &self.extra, IMAGE_FIELDS)?;
        if self.kind == Kind::Content && !self.worlds.is_empty() {
            return Err("a content image has no worlds".into());
        }

        if self.entries.len() > MAX_ENTRIES
            || self.withheld.len() > MAX_ENTRIES
            || self.removed.len() > MAX_ENTRIES
        {
            return Err(format!("more than {MAX_ENTRIES} entries"));
        }
        if self.worlds.len() > MAX_WORLDS {
            return Err(format!("more than {MAX_WORLDS} worlds"));
        }
        let mut world_names = BTreeSet::new();
        for world in &self.worlds {
            validate_component(&world.name).map_err(|e| format!("world {:?}: {e}", world.name))?;
            if !world_names.insert(world.name.as_str()) {
                return Err(format!("world {:?} is listed twice", world.name));
            }
            check_text(
                "world.level_name",
                world.level_name.as_deref().unwrap_or_default(),
            )?;
            check_text(
                "world.game_version",
                world.game_version.as_deref().unwrap_or_default(),
            )?;
        }
        let mut paths: HashSet<&CanonicalPath> = HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            if entry.env_from.len() > MAX_ENV_FROM {
                return Err(format!(
                    "{}: more than {MAX_ENV_FROM} env_from inputs",
                    entry.path
                ));
            }
            entry.validate()?;
            check_unknown(entry.path.as_str(), &entry.extra, ENTRY_FIELDS)?;
            if let Some(world) = &entry.world {
                if !world_names.contains(world.as_str()) {
                    return Err(format!("{}: world {world:?} isn't in worlds", entry.path));
                }
            }
            if !paths.insert(&entry.path) {
                return Err(format!("{} is listed twice", entry.path));
            }
        }
        // A path can't be both a file and a folder of another file. In byte order, the
        // paths inside a folder `f` come right after `f/`, so one binary search per entry
        // finds them: O(n log n) however deep the paths are (checking every prefix of every
        // path is quadratic in their depth, minutes for a manifest of deep paths).
        let mut sorted: Vec<&str> = self.entries.iter().map(|e| e.path.as_str()).collect();
        sorted.sort_unstable();
        let mut folder = String::new();
        for path in &sorted {
            folder.clear();
            folder.push_str(path);
            folder.push('/');
            let next = sorted.partition_point(|other| *other < folder.as_str());
            if sorted
                .get(next)
                .is_some_and(|other| other.starts_with(&folder))
            {
                return Err(format!("{path} is both a file and a folder"));
            }
        }
        for removed in &self.removed {
            if paths.contains(removed) {
                return Err(format!("{removed} is both removed and an entry"));
            }
        }
        if self.excludes.len() > MAX_EXCLUDES {
            return Err(format!("more than {MAX_EXCLUDES} excludes"));
        }
        // Only patterns a snapshot applies: `excludes` says what an image left out, and a
        // pattern the matcher refuses (and so ignores) left nothing out.
        for pattern in &self.excludes {
            if let Some(why) = crate::store::excludes::pattern_error(pattern) {
                return Err(format!("exclude {pattern:?}: {why}"));
            }
        }
        for withheld in &self.withheld {
            if withheld.path.len() > MAX_PATH_BYTES {
                return Err("a withheld path is too long".into());
            }
        }
        Ok(())
    }

    /// The entry at `path`, if any. Entries must be sorted ([`Image::normalize`]).
    pub fn entry(&self, path: &CanonicalPath) -> Option<&Entry> {
        self.entries
            .binary_search_by(|e| e.path.cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }
}

/// Checks the fields kept as they were (`extra`). None may be named after a known field:
/// serialized after the known ones, it would take that field's place in the manifest and
/// in the id, and make a manifest that can't be read back. And every number in them must
/// be a whole number fitting 64 bits (not a float, or an integer beyond `u64` or `i64`):
/// serde_json reads the others as floats and writes them back differently (`1.0`,
/// `1.8446744073709552e19`), so the canonical manifest, and the id, would no longer be the
/// writer's.
fn check_unknown(
    owner: &str,
    extra: &BTreeMap<String, Value>,
    known: &[&str],
) -> Result<(), String> {
    if let Some(key) = extra.keys().find(|key| known.contains(&key.as_str())) {
        return Err(format!(
            "{owner}: the unknown field {key:?} is named after a known field"
        ));
    }
    fn whole(value: &Value) -> bool {
        match value {
            Value::Number(n) => n.is_i64() || n.is_u64(),
            Value::Array(items) => items.iter().all(whole),
            Value::Object(map) => map.values().all(whole),
            Value::Null | Value::Bool(_) | Value::String(_) => true,
        }
    }
    match extra.iter().find(|(_, value)| !whole(value)) {
        Some((key, _)) => Err(format!(
            "{owner}: the unknown field {key:?} holds a number that isn't a whole 64-bit number"
        )),
        None => Ok(()),
    }
}

fn check_text(name: &str, text: &str) -> Result<(), String> {
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(format!("{name} is longer than {MAX_TEXT_LEN} characters"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!("{name} has control characters"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::env::EnvValue;
    use crate::hex::sha256_hex;

    pub(crate) fn path(text: &str) -> CanonicalPath {
        CanonicalPath::new(text).unwrap()
    }

    pub(crate) fn fabric() -> Game {
        Game {
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.10".into()),
        }
    }

    pub(crate) fn object(text: &str) -> Entry {
        Entry::new(
            path(text),
            text.len() as u64,
            sha256_hex(text.as_bytes()),
            Source::Object {},
        )
    }

    fn sample() -> Image {
        let mut image = Image::new(Kind::Full, fabric());
        image.created = 1_794_645_624;
        image.label = Some("Added JEI".into());
        image.producer = Producer {
            app: App::Core,
            version: "0.6.0".into(),
        };
        image.base = Some(Base::ModrinthPack {
            project: "abc".into(),
            version: "def".into(),
            sha512: "a".repeat(128),
        });
        let mut jei = object("mods/jei.jar");
        jei.source = Source::Modrinth {
            project: "u6dRKJwZ".into(),
            version: "v1".into(),
            url: "https://cdn.modrinth.com/data/u6dRKJwZ/versions/v1/jei.jar".into(),
        };
        jei.sha512 = Some("b".repeat(128));
        jei.env = Env::new(EnvValue::Required, EnvValue::Optional);
        jei.env_from = vec![EnvFrom::OwnerMark, EnvFrom::ModrinthEnvironment];
        jei.modified = Some(1_794_645_624_000_000_000);
        image.entries = vec![object("saves/world/level.dat"), jei];
        image.worlds = vec![World {
            name: "world".into(),
            level_name: Some("world".into()),
            data_version: Some(3953),
            game_version: Some("1.21.1".into()),
            size: Some(123_456),
            last_played: Some(1_794_645_624),
            quiesced: Some(true),
        }];
        image.removed = vec![path("mods/some-pack-mod.jar")];
        image.excludes = vec!["dynmap/web/tiles/".into()];
        image.withheld = vec![Withheld {
            path: "mods/blocked.jar".into(),
            reason: WithheldReason::CurseforgeNotDownloadable,
        }];
        image.normalize();
        image
    }

    #[test]
    fn round_trips_through_json() {
        let image = sample();
        image.validate().unwrap();
        let json = image.to_json_pretty();
        let back = Image::from_json(&json).unwrap();
        assert_eq!(back, image);
    }

    #[test]
    fn stable_field_names() {
        let value = serde_json::to_value(sample()).unwrap();
        for key in [
            "schema", "id", "kind", "parent", "created", "producer", "label", "base", "game",
            "settings", "entries", "removed", "worlds", "content", "excludes", "withheld", "rules",
        ] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
        assert_eq!(value["kind"], "full");
        assert_eq!(value["base"]["modrinth_pack"]["project"], "abc");
        let entry = &value["entries"][0];
        assert_eq!(entry["path"], "mods/jei.jar");
        assert_eq!(entry["source"]["modrinth"]["project"], "u6dRKJwZ");
        assert_eq!(entry["env"]["client"], "required");
        assert_eq!(entry["env_from"][0], "modrinth_environment");
        assert_eq!(entry["mode"], 420);
        assert_eq!(value["entries"][1]["world"], "world");
        assert_eq!(
            value["withheld"][0]["reason"],
            "curseforge_not_downloadable"
        );
        assert_eq!(
            serde_json::to_value(Source::Base {}).unwrap(),
            serde_json::json!({"base": {}})
        );
        assert_eq!(
            serde_json::to_value(Base::Loader {}).unwrap(),
            serde_json::json!({"loader": {}})
        );
        assert_eq!(
            serde_json::to_value(Base::CurseforgePack {
                project: 1,
                file: 2
            })
            .unwrap(),
            serde_json::json!({"curseforge_pack": {"project": 1, "file": 2}})
        );
    }

    #[test]
    fn reads_the_spec_example_shape() {
        let json = br#"{
          "schema": 2,
          "kind": "content",
          "parent": null,
          "created": 1794645624,
          "producer": { "app": "core", "version": "0.6.0" },
          "label": "Added JEI",
          "base": { "curseforge_pack": { "project": 1, "file": 2 } },
          "game": { "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10" },
          "settings": {},
          "entries": [{
            "path": "mods/jei-19.21.jar", "size": 1234,
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "mode": 420, "modified": 1794645624000000000,
            "source": { "curseforge": { "project": 1, "file": 2, "downloadable": false } },
            "env": { "client": "required", "server": "optional" },
            "env_from": [ "modrinth_environment", "owner_mark" ]
          }],
          "removed": [ "mods/some-pack-mod.jar" ],
          "content": null,
          "rules": 1
        }"#;
        let image = Image::from_json(json).unwrap();
        assert_eq!(image.kind, Kind::Content);
        assert_eq!(image.entries.len(), 1);
        assert!(image.worlds.is_empty());
        assert!(image.id.is_empty());
    }

    #[test]
    fn unknown_fields_are_kept() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["future_field"] = serde_json::json!({"x": [1, 2]});
        value["entries"][0]["future_entry_field"] = serde_json::json!("y");
        let image = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            image.extra["future_field"],
            serde_json::json!({"x": [1, 2]})
        );
        assert_eq!(image.entries[0].extra["future_entry_field"], "y");
        let again = serde_json::to_value(&image).unwrap();
        assert_eq!(again["future_field"], value["future_field"]);
        assert_eq!(again["entries"][0]["future_entry_field"], "y");
    }

    #[test]
    fn unknown_fields_below_the_entries_are_refused_by_name() {
        // Dropping them would change the id; refusing names the field instead.
        // (where, the field added there)
        let cases = [
            ("/worlds/0", "seed"),
            ("/game", "future"),
            ("/entries/1/source/object", "future"),
            ("/entries/0/source/modrinth", "future"),
            ("/base/modrinth_pack", "future"),
            ("/entries/0/env", "future"),
            ("/withheld/0", "future"),
        ];
        for (pointer, field) in cases {
            let mut image = sample();
            image.withheld.push(Withheld {
                path: "mods/x.jar".into(),
                reason: WithheldReason::Symlink,
            });
            let mut value = serde_json::to_value(&image).unwrap();
            value
                .pointer_mut(pointer)
                .and_then(serde_json::Value::as_object_mut)
                .unwrap_or_else(|| panic!("{pointer}"))
                .insert(field.to_string(), 42.into());
            let error = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert!(
                matches!(&error, ManifestError::Invalid(m) if m.contains(field)),
                "{pointer}: {error}"
            );
        }
    }

    #[test]
    fn unknown_numbers_must_be_whole_64_bit_numbers() {
        for (number, ok) in [
            ("18446744073709551615", true),
            ("-9223372036854775808", true),
            ("18446744073709551616", false),
            ("1.0", false),
            ("1e3", false),
        ] {
            let mut value = serde_json::to_value(sample()).unwrap();
            value["future"] = serde_json::from_str(&format!("[{{\"n\": {number}}}]")).unwrap();
            value["entries"][0]["future"] = serde_json::from_str(number).unwrap();
            let read = Image::from_json(&serde_json::to_vec(&value).unwrap());
            assert_eq!(read.is_ok(), ok, "{number}: {read:?}");
            if let Ok(mut image) = read {
                image.seal();
                let again = Image::from_json(&image.to_json_pretty()).unwrap();
                assert_eq!(again.verify(), Ok(()));
            }
        }
        // In an entry alone too.
        let mut value = serde_json::to_value(sample()).unwrap();
        value["entries"][0]["future"] = serde_json::json!(0.5);
        let error = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(error.to_string().contains("mods/jei.jar"), "{error}");
    }

    #[test]
    fn content_images_have_no_worlds() {
        let mut image = sample();
        image.kind = Kind::Content;
        assert!(image.validate().unwrap_err().contains("no worlds"));
        image.worlds.clear();
        // Its world files then name a world that isn't there.
        assert!(image.validate().is_err());
        image.entries.retain(|e| e.world.is_none());
        assert_eq!(image.validate(), Ok(()));
    }

    #[test]
    fn unknown_informational_values_are_kept() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["withheld"][0]["reason"] = "too_big".into();
        value["entries"][0]["env_from"] = serde_json::json!(["future_input"]);
        value["game"]["loader"] = "liteloader".into();
        let image = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(image.withheld[0].reason.as_str(), "too_big");
        assert_eq!(image.entries[0].env_from[0].as_str(), "future_input");
        assert_eq!(image.game.loader.as_str(), "liteloader");
    }

    #[test]
    fn schema_is_checked_first() {
        assert_eq!(
            Image::from_json(br#"{"schema": 3, "anything": "goes"}"#),
            Err(ManifestError::NewerSchema { found: 3 })
        );
        assert_eq!(
            Image::from_json(br#"{"schema": 1, "files": []}"#),
            Err(ManifestError::OlderSchema { found: 1 })
        );
        assert!(matches!(
            Image::from_json(br#"{"kind": "content"}"#),
            Err(ManifestError::Invalid(_))
        ));
        assert!(matches!(
            Image::from_json(br#"{"schema": "2"}"#),
            Err(ManifestError::Invalid(_))
        ));
        assert!(matches!(
            Image::from_json(br#"{"schema": -1}"#),
            Err(ManifestError::Invalid(_))
        ));
        assert!(matches!(
            Image::from_json(b"[2]"),
            Err(ManifestError::Json(_))
        ));
        assert!(matches!(
            Image::from_json(b"not json"),
            Err(ManifestError::Json(_))
        ));
        assert!(matches!(Image::from_json(b""), Err(ManifestError::Json(_))));
    }

    fn invalid_after(edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut value = serde_json::to_value(sample()).unwrap();
        edit(&mut value);
        match Image::from_json(&serde_json::to_vec(&value).unwrap()) {
            Err(ManifestError::Invalid(reason)) => reason,
            other => panic!("expected invalid, got {other:?}"),
        }
    }

    #[test]
    fn hostile_manifests_are_refused() {
        invalid_after(|v| v["entries"][0]["path"] = "../../etc/passwd".into());
        invalid_after(|v| v["entries"][0]["path"] = "C:\\Windows\\x".into());
        invalid_after(|v| v["entries"][0]["path"] = "mods/CON.jar".into());
        invalid_after(|v| v["entries"][0]["path"] = ".lodestone_config".into());
        invalid_after(|v| v["entries"][0]["path"] = "logs/latest.log".into());
        invalid_after(|v| v["entries"][0]["sha256"] = "ABC".into());
        invalid_after(|v| v["entries"][0]["sha256"] = "A".repeat(64).into());
        invalid_after(|v| v["entries"][0]["sha512"] = "zz".into());
        invalid_after(|v| v["entries"][0]["mode"] = 0o17777.into());
        // setuid, setgid and sticky aren't recorded, so an image can't carry them.
        invalid_after(|v| v["entries"][0]["mode"] = 0o4755.into());
        invalid_after(|v| v["entries"][0]["mode"] = 0o1777.into());
        invalid_after(|v| v["entries"][0]["size"] = (-1).into());
        invalid_after(|v| {
            v["entries"][0]["source"]["modrinth"]["url"] = "https://evil.example/jei.jar".into()
        });
        invalid_after(|v| {
            v["entries"][0]["source"]["modrinth"]["url"] =
                "https://cdn.modrinth.com.evil.example/x".into()
        });
        invalid_after(|v| v["entries"][0]["sha512"] = serde_json::Value::Null);
        invalid_after(|v| v["entries"][0]["source"] = serde_json::json!({"torrent": {}}));
        invalid_after(|v| v["entries"][0]["env"]["client"] = "sometimes".into());
        invalid_after(|v| v["kind"] = "everything".into());
        invalid_after(|v| v["base"] = serde_json::json!({"git": {}}));
        invalid_after(|v| v["id"] = "not-an-id".into());
        invalid_after(|v| v["parent"] = "../x".into());
        invalid_after(|v| v["label"] = "a\u{0}b".into());
        invalid_after(|v| v["game"]["minecraft"] = "".into());
        invalid_after(|v| v["rules"] = 0.into());
        invalid_after(|v| v["projected_from"] = "a".repeat(64).into());
        invalid_after(|v| v["excludes"] = serde_json::json!([""]));
        invalid_after(|v| {
            v["settings"] = serde_json::json!({"server": {"properties": {"rcon.password": "x"}}})
        });
        // World files must name a listed world, and match their path.
        invalid_after(|v| v["worlds"] = serde_json::json!([]));
        invalid_after(|v| v["entries"][1]["world"] = "other".into());
        invalid_after(|v| v["entries"][0]["world"] = "world".into());
        invalid_after(|v| v["worlds"][0]["name"] = "a/b".into());
        invalid_after(|v| {
            let world = v["worlds"][0].clone();
            v["worlds"].as_array_mut().unwrap().push(world);
        });
    }

    #[test]
    fn duplicate_and_overlapping_paths_are_refused() {
        let reason = invalid_after(|v| {
            let entry = v["entries"][0].clone();
            v["entries"].as_array_mut().unwrap().push(entry);
        });
        assert!(reason.contains("twice"), "{reason}");

        let mut image = sample();
        image.entries.push(object("mods"));
        image.normalize();
        assert!(image
            .validate()
            .unwrap_err()
            .contains("both a file and a folder"));

        let mut image = sample();
        image.removed.push(path("mods/jei.jar"));
        image.normalize();
        assert!(image
            .validate()
            .unwrap_err()
            .contains("both removed and an entry"));
    }

    #[test]
    fn deep_paths_validate_in_linear_time() {
        // Entries at the deepest a path can be: checking every prefix of every path was
        // quadratic in the depth (about a minute for this many in a release build).
        let mut image = Image::new(Kind::Content, fabric());
        for i in 0..300 {
            image
                .entries
                .push(object(&format!("config/{}f{i}", "a/".repeat(2040))));
        }
        image.normalize();
        let json = image.to_json_pretty();
        let started = std::time::Instant::now();
        Image::from_json(&json).unwrap();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
        // And a deep file that is also a folder is still found.
        image.entries.push(object("config/a/a"));
        image.normalize();
        assert!(image
            .validate()
            .unwrap_err()
            .contains("config/a/a is both a file and a folder"));
    }

    #[test]
    fn an_image_isnt_its_own_parent() {
        let mut image = sample();
        image.seal();
        image.parent = Some(image.id.clone());
        assert!(image.validate().unwrap_err().contains("parent"));
        image.parent = Some("a".repeat(64));
        image.validate().unwrap();
    }

    #[test]
    fn producer_fields_are_checked_and_null_is_the_default() {
        let error = invalid_after(|v| v["producer"]["build"] = 7.into());
        assert!(error.contains("build"), "{error}");
        let mut value = serde_json::to_value(sample()).unwrap();
        value["producer"] = serde_json::json!({"app": null, "version": null});
        let image = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(image.producer, Producer::default());
        value["producer"] = serde_json::json!({});
        assert_eq!(
            Image::from_json(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .producer,
            Producer::default()
        );
    }

    #[test]
    fn limits() {
        let mut image = sample();
        image.excludes = (0..=MAX_EXCLUDES).map(|i| format!("p{i}")).collect();
        assert!(image.validate().is_err());
        let mut image = sample();
        image.excludes = vec!["x".repeat(MAX_EXCLUDE_LEN + 1)];
        assert!(image.validate().is_err());
        let mut image = sample();
        image.label = Some("x".repeat(MAX_TEXT_LEN + 1));
        assert!(image.validate().is_err());
        // Patterns the matcher would ignore can't be recorded as applied.
        for pattern in ["!keep", "/", "a//b", "a\\b", " "] {
            let mut image = sample();
            image.excludes = vec![pattern.into()];
            assert!(image.validate().is_err(), "{pattern:?}");
        }
    }

    #[test]
    fn normalize_sorts_sets() {
        let mut image = sample();
        image.entries.reverse();
        image.entries[1].env_from = vec![EnvFrom::OwnerMark, EnvFrom::BasePack, EnvFrom::OwnerMark];
        image.excludes = vec!["b".into(), "a".into(), "b".into()];
        image.normalize();
        assert_eq!(image.entries[0].path.as_str(), "mods/jei.jar");
        assert_eq!(image.excludes, ["a", "b"]);
        let entry = image
            .entries
            .iter()
            .find(|e| e.path.as_str() == "saves/world/level.dat")
            .unwrap();
        assert!(entry.env_from.is_empty());
        let jei = image.entry(&path("mods/jei.jar")).unwrap();
        assert_eq!(jei.env_from, [EnvFrom::BasePack, EnvFrom::OwnerMark]);
        assert!(image.entry(&path("mods/none.jar")).is_none());
    }

    #[test]
    fn entry_defaults() {
        let entry = object("saves/w/region/r.0.0.mca");
        assert_eq!(entry.world.as_deref(), Some("w"));
        assert_eq!(entry.mode, 0o644);
        assert_eq!(entry.env, Env::UNKNOWN);
        let parsed: Entry = serde_json::from_str(
            r#"{"path":"mods/a.jar","size":1,"sha256":"00","source":{"object":{}}}"#,
        )
        .unwrap();
        assert_eq!(parsed.mode, 0o644);
        assert_eq!(parsed.env, Env::UNKNOWN);
    }
}
