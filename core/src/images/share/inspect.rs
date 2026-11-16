//! What a `.lodeimg` file holds, read before anything is made from it: the summary the
//! Host's "From a file…" dialog shows ([`ImageSummary`]), and the checks that refuse a
//! file no server may be made from.
//!
//! The file comes from someone else, so nothing in it is trusted. The container's reader
//! checks every name, size, count and hash; then [`check_entries`] refuses Lodestone's own
//! names (`.lodestone…` at any depth), top-level folders spelled so a disk would take them
//! for a content folder (`modſ/`), paths that are one on a disk that ignores letter case,
//! settings files the server wouldn't read as the summary does (spelled in other letters,
//! over 1 MiB, or unreadable), versions that aren't plausible ids and more downloads than
//! any server has. Text from the file goes through `display_text` before it is shown,
//! and lists are cut to [`super::MAX_NAMES`] names with their full counts.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use lodestone_image::container::{self, Container, Limits};
use lodestone_image::paths::{case_collisions, placement, same_name};
use lodestone_image::settings::ServerFlavour;
use lodestone_image::share::{placed_entries, plausible_version, spelled_like, MAX_DOWNLOADS};
use lodestone_image::view::in_view;
use lodestone_image::{display_text, CanonicalPath, Entry, Image, Loader, Side, Source};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::handlers::instance_fs::{is_instance_protected, is_protected};
use crate::handlers::util::is_code_path;
use crate::images::scope::is_lodestone_name;
use crate::implementations::minecraft::properties::PropertiesDocument;
use crate::implementations::minecraft::FlavourKind;

use super::export::BLANKED_FIELD;
use super::MAX_NAMES;

/// What the person importing may add to a server, which decides what a file may place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps {
    /// `WriteGlobalFile`: jars, the mods folder and other protected files.
    pub may_write_global: bool,
    /// The owner, while code execution is on: scripts.
    pub may_write_code: bool,
}

/// Who made a file, from its producer: it only steers what the dialog says (a Host's
/// server image or a Launcher's client image); a file is placed the same way whoever made
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ImageMaker {
    Host,
    Launcher,
    Unknown,
}

/// Names, at most [`MAX_NAMES`] of them (written visibly), and how many there are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NameList {
    pub names: Vec<String>,
    #[ts(type = "number")]
    pub count: u64,
}

/// The most names of one kind a summary counts before it stops.
const MAX_COUNTED_NAMES: usize = 100_000;

impl NameList {
    /// The names, sorted, each once.
    pub fn of<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> NameList {
        let all: BTreeSet<String> = names
            .into_iter()
            .map(|n| display_text(n.as_ref()).into_owned())
            .collect();
        NameList {
            count: all.len() as u64,
            names: all.into_iter().take(MAX_NAMES).collect(),
        }
    }
}

/// A world in a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImageWorld {
    /// Its folder (`saves/<folder>/`) as the file names it: what
    /// `POST /instance/create_from_image`'s `world` sends back. Show [`Self::folder_shown`].
    pub folder: String,
    /// Its folder, written visibly.
    pub folder_shown: String,
    /// Its own name (`level.dat`'s), written visibly, when it differs from the folder.
    pub name: Option<String>,
    #[ts(type = "number")]
    pub size: u64,
    /// Unix seconds.
    #[ts(type = "number | null")]
    pub last_played: Option<i64>,
}

/// What a new server never takes from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum NotTaken {
    /// The server jar, libraries and start scripts: Lodestone installs its own.
    ServerSoftware,
    /// `eula.txt`: the EULA is accepted by whoever runs the server.
    Eula,
    /// A player's own files (`options.txt`, resource packs, screenshots).
    ClientFiles,
    /// `ops.json`, Bukkit's `permissions.yml` and `commands.yml` and permission mods' and
    /// plugins' data (LuckPerms', FTB Ranks'…), unless the importer keeps the operators.
    Operators,
    /// Script files: only the owner adds them, while code execution is on.
    Scripts,
}

/// What a `.lodeimg` file holds, as `POST /instance/image/upload` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImageSummary {
    /// The image's id: `POST /instance/create_from_image` refuses a file that changed.
    pub image: String,
    /// The uploaded file's name, written visibly.
    pub file_name: String,
    /// The image's label (a backup's or snapshot's name), written visibly.
    pub label: Option<String>,
    pub made_by: ImageMaker,
    /// The Lodestone version that made it, written visibly.
    pub producer_version: String,
    /// Unix seconds.
    #[ts(type = "number")]
    pub created: i64,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    /// The server software the file names (`paper`, `purpur`…), written visibly.
    pub flavour: Option<String>,
    /// The server Lodestone makes from it unless another is chosen, when it can.
    pub server: Option<FlavourKind>,
    /// The servers `POST /instance/create_from_image`'s `flavour` may choose: Vanilla and
    /// Paper for a vanilla game (a player's world can become a Paper server, for plugins),
    /// else only [`Self::server`]. Empty when no server can be made.
    pub server_choices: Vec<FlavourKind>,
    /// Why Lodestone can't make a server from it (Quilt, an unknown loader); none when it
    /// can.
    pub unsupported: Option<String>,
    /// A note on the server it makes ("Purpur servers are made as Paper servers").
    pub server_note: Option<String>,
    /// The pack it is built on, written visibly.
    pub base: Option<String>,
    /// The mods and plugins the server downloads from Modrinth (with the default world;
    /// the whole file's when no server can be made from it).
    pub mods_from_modrinth: NameList,
    /// The mods and plugins the server would take that come from CurseForge.
    pub mods_from_curseforge: NameList,
    /// Mod and plugin files the server takes whose bytes the file carries: they come from
    /// whoever made it. A jar of a Launcher's file that says it runs only on clients is
    /// still among them: it is only read once placed (and then deleted).
    pub carried_mods: NameList,
    /// Mods a server never loads (client only by their sides): left out. A Host's file
    /// keeps the ones its server gave players.
    pub client_only: NameList,
    /// The mod and plugin files the server takes, from any source (with the default world;
    /// the whole file's when no server can be made): programs that run on the server, which
    /// the trust advisory counts.
    #[ts(type = "number")]
    pub server_mods: u64,
    pub worlds: Vec<ImageWorld>,
    /// The world that becomes the server's world unless another is picked: a server's
    /// level, or a Launcher file's last played world. One of the worlds' `folder`.
    pub default_world: Option<String>,
    #[ts(type = "number")]
    pub files: u64,
    /// What its files take once placed.
    #[ts(type = "number")]
    pub total_bytes: u64,
    /// What the server downloads (Modrinth files), with the default world.
    #[ts(type = "number")]
    pub download_bytes: u64,
    /// What importing unpacks into the backup store, with the default world (only the
    /// files the server takes are unpacked).
    #[ts(type = "number")]
    pub unpacked_bytes: u64,
    /// Everything making the server with the default world writes: the free-space check
    /// asks for this (worked out again for the world picked) and 1 GiB.
    #[ts(type = "number")]
    pub needed_bytes: u64,
    /// Free space where servers are made, when known.
    #[ts(type = "number | null")]
    pub disk_free: Option<u64>,
    /// Files the server would take that can't be downloaded here, so are left out:
    /// CurseForge files the file doesn't carry, as Lodestone Core has no CurseForge client
    /// (see "M6a decisions").
    pub not_downloadable: NameList,
    /// What the sender left out on purpose: the image's exclude patterns.
    pub left_out: NameList,
    /// Files the sender's copy couldn't hold (links, unreadable files), with no bytes.
    pub withheld: NameList,
    /// Secret values the sender's Lodestone made empty, as `<file>: <key>`.
    pub blanked: NameList,
    /// Files the server never takes because they hold a key to it or point it at someone's
    /// database (proxy forwarding secrets, Floodgate's folder, LuckPerms' settings), or
    /// hold any password or token ([`credential_files`]: a sender's bot token, store key,
    /// webhook): the sender would keep a way in, or their logins would configure it. Paper's
    /// forwarding secret and `server.properties`' secrets are emptied instead.
    pub secrets_not_taken: NameList,
    pub not_taken: Vec<NotTaken>,
    /// Every operator in its `ops.json` (a name, with the uuid when another entry has that
    /// name too, or the uuid of an entry with none), its `permissions.yml` and
    /// `commands.yml` when it has them (Bukkit's permissions can give anyone the operator
    /// commands, and its command aliases can run any command as the console), and
    /// LuckPerms' folder when it has its data (`plugins/LuckPerms/`: it can give anyone
    /// every permission), written visibly: what `keep_operators` takes.
    pub operators: NameList,
    /// Its `server.properties` lets players join without a Minecraft account.
    pub offline_mode: bool,
    /// Carried files that need permission to change global files (jars, the mods folder).
    pub needs_global_files: bool,
    /// Script files left out: only the owner adds them, while code execution is on.
    #[ts(type = "number")]
    pub scripts_skipped: u64,
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: color_eyre::eyre::eyre!(message.into()),
    }
}

/// An error opening a file, for the person who chose it.
pub fn container_error(e: container::ContainerError) -> Error {
    match e {
        container::ContainerError::Cancelled => crate::backups::store::cancelled(),
        container::ContainerError::NoRoom(why) => Error {
            kind: ErrorKind::Conflict,
            source: color_eyre::eyre::eyre!("There isn't room to import it: {why}"),
        },
        container::ContainerError::Store(e) => Error {
            kind: ErrorKind::Internal,
            source: color_eyre::eyre::eyre!("The snapshot store failed: {e}"),
        },
        other => bad_request(format!("This isn't a file Lodestone can open: {other}")),
    }
}

/// Top-level folders a file may only spell with ASCII letters (see the module).
const FOLDERS_SPELLED_PLAINLY: [&str; 8] = [
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "plugins",
    "libraries",
    "versions",
    "saves",
];

/// Refuses a file whose entries a server must never get: Lodestone's own names at any
/// depth, top-level folders spelled so a disk would take them for another, paths that are
/// one on a disk that ignores letter case (a server made from it would hold one of them on
/// macOS and Windows and both on Linux, where the summary could read one and the server
/// the other), and a settings file of [`super::import::SETTINGS_FILES`] spelled in other
/// letters than the server reads it or over 1 MiB, so the summary reads the file the
/// server does.
pub fn check_entries(entries: &[Entry]) -> Result<(), Error> {
    if let Some((first, later)) = case_collisions(entries.iter().map(|e| &e.path))
        .into_iter()
        .next()
    {
        return Err(bad_request(format!(
            "The file has {} and {}, one file on a disk that ignores letter case",
            display_text(first.as_str()),
            display_text(later.as_str())
        )));
    }
    for entry in entries {
        if super::import::is_settings_file(&entry.path) {
            if !super::import::SETTINGS_FILES.contains(&entry.path.as_str()) {
                return Err(bad_request(format!(
                    "The file has {}, a settings file spelled in other letters than a server reads it",
                    display_text(entry.path.as_str())
                )));
            }
            if entry.size > MAX_SETTINGS_FILE {
                return Err(bad_request(format!(
                    "The file's {} is over 1 MiB, more than a server's settings ever are",
                    entry.path
                )));
            }
        }
        if entry.path.components().any(is_lodestone_name) {
            return Err(bad_request(format!(
                "The file has {}, a name Lodestone keeps for its own files",
                display_text(entry.path.as_str())
            )));
        }
        if spelled_like(entry.path.first(), &FOLDERS_SPELLED_PLAINLY) {
            return Err(bad_request(format!(
                "The file has {}, a folder name a disk would take for another",
                display_text(entry.path.as_str())
            )));
        }
    }
    Ok(())
}

/// Refuses Minecraft and loader versions that aren't plausible version ids.
pub fn check_game(image: &Image) -> Result<(), Error> {
    let game = &image.game;
    for version in std::iter::once(game.minecraft.as_str()).chain(game.loader_version.as_deref()) {
        if !plausible_version(version) {
            return Err(bad_request(format!(
                "The file names the version {}, which isn't one",
                display_text(version)
            )));
        }
    }
    Ok(())
}

/// The server Lodestone makes for `image`, and a note when it isn't the file's own
/// software; an error saying why when it makes none. A modded game decides (Fabric, Forge,
/// NeoForge); a vanilla game's server flavour decides between vanilla and Paper, which the
/// Bukkit family's other servers (Spigot, Bukkit, Purpur, Folia) become, as they run its
/// plugins and share its world layout.
pub fn server_kind(image: &Image) -> Result<(FlavourKind, Option<String>), String> {
    let flavour = image
        .settings
        .server
        .as_ref()
        .and_then(|s| s.flavour.clone());
    match &image.game.loader {
        Loader::Fabric => Ok((FlavourKind::Fabric, None)),
        Loader::Forge => Ok((FlavourKind::Forge, None)),
        Loader::NeoForge => Ok((FlavourKind::NeoForge, None)),
        Loader::Quilt => Err("Lodestone can't make Quilt servers".to_string()),
        Loader::Other(other) => Err(format!(
            "Lodestone can't make servers for {}",
            display_text(other)
        )),
        Loader::Vanilla => match flavour {
            None | Some(ServerFlavour::Vanilla) => Ok((FlavourKind::Vanilla, None)),
            Some(ServerFlavour::Paper) => Ok((FlavourKind::Paper, None)),
            Some(
                relative @ (ServerFlavour::Spigot
                | ServerFlavour::Bukkit
                | ServerFlavour::Purpur
                | ServerFlavour::Folia),
            ) => Ok((
                FlavourKind::Paper,
                Some(format!(
                    "Lodestone makes {} servers as Paper servers, which run the same plugins",
                    relative.as_str()
                )),
            )),
            Some(other) => Err(format!(
                "Lodestone can't make {} servers for a vanilla game",
                display_text(other.as_str())
            )),
        },
    }
}

/// The servers a server made from `image` may be: Vanilla and Paper for a vanilla game
/// (its worlds are laid out either way, and Paper runs plugins), else only the one
/// [`server_kind`] picks; none when Lodestone can't make one.
pub fn server_choices(image: &Image) -> Vec<FlavourKind> {
    match (server_kind(image), &image.game.loader) {
        (Err(_), _) => Vec::new(),
        (Ok(_), Loader::Vanilla) => vec![FlavourKind::Vanilla, FlavourKind::Paper],
        (Ok((kind, _)), _) => vec![kind],
    }
}

/// The server to make from `image`: `wanted` when [`server_choices`] allows it, the
/// file's own ([`server_kind`]) when `None`; an error saying why otherwise.
pub fn chosen_server(
    image: &Image,
    wanted: Option<FlavourKind>,
) -> Result<(FlavourKind, Option<String>), String> {
    let own = server_kind(image)?;
    match wanted {
        None => Ok(own),
        Some(kind) if kind == own.0 => Ok(own),
        Some(kind) if server_choices(image).contains(&kind) => Ok((kind, None)),
        Some(kind) => Err(format!(
            "A server made from this file can't be a {} server: its game is {}",
            kind.to_string(),
            display_text(image.game.loader.as_str())
        )),
    }
}

/// Whether `entry` is a file this core downloads: a Modrinth file with its SHA-512 whose
/// URL is its own project's and version's canonical CDN URL ([`names_modrinth_file`]). The
/// core has no CurseForge client, so a CurseForge file is placed only when the file
/// carries its bytes.
pub fn downloadable(entry: &Entry) -> bool {
    match &entry.source {
        Source::Modrinth {
            url,
            project,
            version,
        } => entry.sha512.is_some() && names_modrinth_file(url, project, version),
        _ => false,
    }
}

/// Whether `url` is the Modrinth CDN URL of a file of `project`'s `version`, written the
/// way the URL parser writes it (so no dot segments, backslashes or other spellings that
/// would download another path than the one checked): exactly
/// `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`, with base62 ids,
/// no query, and a file name without an encoded `.`, `/` or `\`. The Launcher's imports
/// take the same URLs, so a file of another project never passes for the one named.
pub fn names_modrinth_file(url: &str, project: &str, version: &str) -> bool {
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

/// An entry's file and claimed source: each claimed source is checked by a download of
/// its own, even for the same bytes.
pub fn download_key(entry: &Entry) -> (String, String) {
    (entry.sha256.clone(), format!("{:?}", entry.source))
}

/// The distinct downloads making a server from `entries` may make (each file and claimed
/// source once), refused past [`MAX_DOWNLOADS`] before anything is asked of Modrinth.
pub fn wanted_downloads(entries: &[Entry]) -> Result<Vec<&Entry>, Error> {
    let mut seen = HashSet::new();
    let wanted: Vec<&Entry> = entries
        .iter()
        .filter(|e| downloadable(e))
        .filter(|e| seen.insert(download_key(e)))
        .collect();
    if wanted.len() > MAX_DOWNLOADS {
        return Err(bad_request(format!(
            "The file names {} files to download, more than a server has",
            wanted.len()
        )));
    }
    Ok(wanted)
}

/// Whether a server's file at `path` is server software, which Lodestone installs itself:
/// the top-level jars, `libraries/`, `versions/`, Forge's start scripts and the
/// installers' logs ([`super::export::is_server_software_name`]).
pub fn is_server_software(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [name] => super::export::is_server_software_name(name),
        [top, _, ..] => lodestone_image::paths::in_names(top, &["libraries", "versions"]),
        _ => false,
    }
}

/// Whether a user without `WriteGlobalFile` couldn't put a file at `path` (canonical,
/// which is the server's disk path but for worlds, whose names can't be protected ones):
/// the file API's rule for a server's files, the protected folders it is in included.
pub fn needs_global(path: &CanonicalPath) -> bool {
    let disk = Path::new(path.as_str());
    is_instance_protected(Path::new(""), disk, false)
        || disk
            .ancestors()
            .skip(1)
            .filter(|dir| !dir.as_os_str().is_empty())
            .any(|dir| is_protected(dir, true))
}

/// Whether `path` is a script, which only the owner adds while code execution is on.
pub fn is_script(path: &CanonicalPath) -> bool {
    is_code_path(Path::new(path.as_str()), false)
}

/// Whether `path` is the top-level file `name`, in any letter case.
fn is_top_named(path: &CanonicalPath, name: &str) -> bool {
    let mut parts = path.components();
    matches!((parts.next(), parts.next()), (Some(first), None) if same_name(first, name))
}

/// The top-level file `name` of `entries`, in any letter case.
fn top_file<'e>(entries: &'e [Entry], name: &str) -> Option<&'e Entry> {
    entries.iter().find(|e| {
        let mut parts = e.path.components();
        matches!((parts.next(), parts.next()), (Some(first), None) if same_name(first, name))
    })
}

/// The largest settings file read before importing.
const MAX_SETTINGS_FILE: u64 = 1 << 20;

/// A small top-level settings file the file carries (`server.properties`, `ops.json`), by
/// its exact name (the one the server reads), read and checked before anything is
/// imported; `None` when it has none or names it as a download (which a server made from
/// it never places). Refuses the file when its bytes can't be read whole: the summary
/// says what these files hold, so they are read or the file is refused.
pub fn read_small(
    container: &mut Container,
    entries: &[Entry],
    name: &str,
) -> Result<Option<Vec<u8>>, Error> {
    let Some(entry) = entries.iter().find(|e| e.path.as_str() == name) else {
        return Ok(None);
    };
    if !matches!(entry.source, Source::Object {}) {
        return Ok(None);
    }
    container
        .read_object(&entry.sha256, MAX_SETTINGS_FILE)
        .map(Some)
        .map_err(|e| bad_request(format!("The file's {name} can't be read: {e}")))
}

/// Refuses a `server.properties` (`properties`) with a line the reader can't parse (a
/// malformed or unpaired-surrogate `\u` escape, an empty key): Java's `Properties` may
/// still read it (it takes `online-mode=false\uD800`, which Minecraft reads as off), so
/// the summary would say what the server doesn't do. A server takes the file written again
/// from the parsed properties alone (`import::pin_settings`), and an export drops such
/// lines, so a file from Lodestone never has one. Read with every line break Java takes.
pub fn check_properties(properties: Option<&[u8]>) -> Result<(), Error> {
    let Some(bytes) = properties else {
        return Ok(());
    };
    let (_, warnings) = PropertiesDocument::from_bytes(&super::export::java_line_breaks(bytes));
    match warnings.first() {
        // the line itself isn't shown: it may hold a password
        Some(warning) => Err(bad_request(format!(
            "The file's server.properties can't be read: line {} isn't a setting Lodestone can read, so it can't say what the server would do with it",
            warning.line_number
        ))),
        None => Ok(()),
    }
}

/// The level a server's `server.properties` names, when it is one folder. Read with every
/// line break Java takes, as the placed file is ([`super::export::java_line_breaks`]).
pub fn level_name(properties: Option<&[u8]>) -> Option<String> {
    let (document, _) =
        PropertiesDocument::from_bytes(&super::export::java_line_breaks(properties?));
    document
        .get("level-name")
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
        .map(str::to_string)
}

/// Whether a server's `server.properties` turns off checking players' accounts: Minecraft
/// reads `online-mode` as on only when its value is exactly `true` (any letter case:
/// `Boolean.valueOf`, which trims nothing), so anything else written there is off, an
/// escaped leading space (`\ true`), a tab or a no-break space before `true` among them.
/// Read with every line break Java takes; [`online_mode_value`] is how it is read.
pub fn offline_mode(properties: Option<&[u8]>) -> bool {
    properties.is_some_and(|bytes| {
        PropertiesDocument::from_bytes(&super::export::java_line_breaks(bytes))
            .0
            .get("online-mode")
            .is_some_and(|v| !online_mode_value(v))
    })
}

/// Whether Minecraft reads the `online-mode` value `value` (as `Properties` parsed it) as
/// on: only `true` in any letter case, nothing around it.
pub fn online_mode_value(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

/// Everyone an `ops.json` makes an operator: by name, with the uuid when the name is
/// another entry's too, or by uuid when it names none. Ops are kept by uuid, so every
/// entry shows, and none hides behind another's name. Refuses an `ops.json` that isn't a
/// JSON list of objects as strict JSON reads it (a byte order mark, text after the list,
/// a comment, a key twice in an entry): Minecraft's Gson reads some of those, and the
/// summary would show no operators where the server has some. An empty file has none.
fn operator_names(bytes: Option<&[u8]>) -> Result<Vec<String>, Error> {
    #[derive(Deserialize)]
    struct Op {
        #[serde(default)]
        name: Option<serde_json::Value>,
        #[serde(default)]
        uuid: Option<serde_json::Value>,
    }
    let text = |value: &Option<serde_json::Value>| match value {
        Some(serde_json::Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        _ => None,
    };
    let ops = match bytes {
        Some(bytes) if !bytes.trim_ascii().is_empty() => serde_json::from_slice::<Vec<Op>>(bytes)
            .map_err(|e| {
            bad_request(format!(
                "The file's ops.json can't be read, so it can't say who the operators are: {e}"
            ))
        })?,
        _ => Vec::new(),
    };
    let mut named: HashMap<String, usize> = HashMap::new();
    for op in &ops {
        if let Some(name) = text(&op.name) {
            *named.entry(name).or_default() += 1;
        }
    }
    Ok(ops
        .iter()
        .map(|op| {
            let uuid = text(&op.uuid).unwrap_or_else(|| "no uuid".to_string());
            match text(&op.name) {
                Some(name) if named.get(&name).is_some_and(|n| *n > 1) => {
                    format!("{name} ({uuid})")
                }
                Some(name) => name,
                None => uuid,
            }
        })
        .collect())
}

/// Whether a mod or plugin loader loads the file at `path`: anything under `mods/`, and
/// jars directly in `plugins/`.
fn is_mod_path(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [top, _, ..] if same_name(top, "mods") => true,
        [top, name] if same_name(top, "plugins") => {
            name.to_uppercase().to_lowercase().ends_with(".jar")
        }
        _ => false,
    }
}

/// A mod's name for the dialog: its path under `mods/` or `plugins/`.
fn mod_name(path: &CanonicalPath) -> String {
    let text = path.as_str();
    text.split_once('/')
        .map_or(text, |(_, rest)| rest)
        .to_string()
}

/// A base, for the dialog.
fn base_text(image: &Image) -> Option<String> {
    match image.base.as_ref()? {
        lodestone_image::Base::ModrinthPack {
            project, version, ..
        } => Some(format!(
            "Modrinth pack {} ({})",
            display_text(project),
            display_text(version)
        )),
        lodestone_image::Base::CurseforgePack { project, file } => {
            Some(format!("CurseForge pack {project} ({file})"))
        }
        lodestone_image::Base::Loader {} => None,
    }
}

/// Everything [`inspect`] works out, with the open container, for making the server.
pub struct Inspected {
    pub container: Container,
    pub entries: Vec<Entry>,
    pub summary: ImageSummary,
    /// `server.properties`, when the file carries one.
    pub properties: Option<Vec<u8>>,
    /// The files that hold a password or token ([`credential_files`]), by path: never
    /// taken ([`super::import::plan_checked`]).
    pub credential_files: BTreeSet<String>,
}

/// The files of `entries` (the placed entries of the open `container`) that hold a
/// password, a token or a key, which a server made from the file never takes, by path
/// ([`super::secrets`], the same rule an export withholds them by): a key or token file by
/// its name, a file too big to check ([`super::secrets::too_large`]), and a settings file
/// whose carried bytes have a credential (a sender's bot token, proxy secret, store key or
/// webhook). A settings file the server would download rather than take from the file's
/// bytes is counted too: its bytes can't be checked before they are placed. Refuses the
/// file when a settings file's carried bytes can't be read or don't match their hash.
pub fn credential_files(
    container: &mut Container,
    entries: &[Entry],
) -> Result<BTreeSet<String>, Error> {
    use super::secrets;
    let mut found = BTreeSet::new();
    let mut verdicts: HashMap<(String, secrets::Format), bool> = HashMap::new();
    for entry in entries {
        let format = secrets::scanned_format(&entry.path);
        let by_name = secrets::withheld_by_name(&entry.path);
        if format.is_none() && !by_name && !secrets::too_large(&entry.path, entry.size) {
            continue;
        }
        if !matches!(entry.source, Source::Object {}) {
            if downloadable(entry) {
                found.insert(entry.path.to_string());
            }
            continue;
        }
        if by_name || secrets::too_large(&entry.path, entry.size) {
            found.insert(entry.path.to_string());
            continue;
        }
        let Some(format) = format else {
            continue;
        };
        let key = (entry.sha256.clone(), format);
        let holds = match verdicts.get(&key) {
            Some(holds) => *holds,
            None => {
                let bytes = container
                    .read_object_any(&entry.sha256, secrets::MAX_SCANNED_FILE)
                    .map_err(|e| {
                        bad_request(format!(
                            "The file's {} can't be read: {e}",
                            display_text(entry.path.as_str())
                        ))
                    })?;
                let holds =
                    secrets::find_credential(&String::from_utf8_lossy(&bytes), format).is_some();
                verdicts.insert(key, holds);
                holds
            }
        };
        if holds {
            found.insert(entry.path.to_string());
        }
    }
    Ok(found)
}

/// The extra space making a server takes beyond what it writes: 1 GiB to spare is added by
/// the free-space check itself.
fn needed(unpacked: u64, entries: &[Entry]) -> u64 {
    let files = entries.iter().map(|e| e.size).fold(0, u64::saturating_add);
    let downloads = entries
        .iter()
        .filter(|e| downloadable(e))
        .map(|e| e.size)
        .fold(0, u64::saturating_add);
    unpacked.saturating_add(files).saturating_add(downloads)
}

/// Opens the file at `path` (named `file_name` by whoever uploaded it) and says what it
/// holds and what making a server from it would do for someone with `caps` (see
/// [`ImageSummary`]), after every check of the module. `disk_dir` is where servers are
/// made, for the free space. Blocking.
pub fn inspect(
    path: &Path,
    file_name: &str,
    caps: Caps,
    disk_dir: &Path,
) -> Result<Inspected, Error> {
    let mut opened = container::open(path, &Limits::default()).map_err(container_error)?;
    let image = opened.image().clone();
    let linked = opened.linked().to_vec();
    let entries = placed_entries(&image, &linked);
    check_entries(&entries)?;
    check_game(&image)?;
    wanted_downloads(&entries)?;
    let images: Vec<&Image> = std::iter::once(&image).chain(&linked).collect();
    let carried: HashSet<String> = container::carried_objects(&images)
        .map_err(bad_request)?
        .into_keys()
        .collect();
    let properties = read_small(&mut opened, &entries, "server.properties")?;
    check_properties(properties.as_deref())?;
    let ops = read_small(&mut opened, &entries, "ops.json")?;
    let credentials = credential_files(&mut opened, &entries)?;
    let operators = operator_names(ops.as_deref())?;
    let (server, unsupported, server_note) = match server_kind(&image) {
        Ok((kind, note)) => (Some(kind), None, note),
        Err(why) => (None, Some(why), None),
    };
    let made_by = super::import::made_by(&image);

    let mut not_taken = BTreeSet::new();
    let mut scripts_skipped = 0;
    let mut needs_global_files = false;
    let mut client_only = Vec::new();
    for entry in &entries {
        let eula = top_file(std::slice::from_ref(entry), "eula.txt").is_some();
        if !in_view(entry, Side::Server) {
            if !placement(&entry.path).allows(Side::Server) {
                not_taken.insert(if eula {
                    NotTaken::Eula
                } else {
                    NotTaken::ClientFiles
                });
            } else if is_mod_path(&entry.path) {
                client_only.push(mod_name(&entry.path));
            }
            continue;
        }
        if eula {
            not_taken.insert(NotTaken::Eula);
            continue;
        }
        if is_server_software(&entry.path) {
            not_taken.insert(NotTaken::ServerSoftware);
            continue;
        }
        if super::import::is_operators_file(&entry.path) {
            not_taken.insert(NotTaken::Operators);
        }
        if is_script(&entry.path) && !caps.may_write_code {
            scripts_skipped += 1;
            not_taken.insert(NotTaken::Scripts);
            continue;
        }
        if carried.contains(&entry.sha256) && needs_global(&entry.path) {
            needs_global_files = true;
        }
    }

    let world_sizes: BTreeMap<&str, u64> =
        entries.iter().fold(BTreeMap::new(), |mut sizes, entry| {
            if let Some(world) = entry.world.as_deref() {
                let size = sizes.entry(world).or_insert(0u64);
                *size = size.saturating_add(entry.size);
            }
            sizes
        });
    let worlds: Vec<ImageWorld> = image
        .worlds
        .iter()
        .map(|world| ImageWorld {
            folder: world.name.clone(),
            folder_shown: display_text(&world.name).into_owned(),
            name: world
                .level_name
                .as_deref()
                .filter(|name| *name != world.name)
                .map(|name| display_text(name).into_owned()),
            size: world
                .size
                .unwrap_or_else(|| world_sizes.get(world.name.as_str()).copied().unwrap_or(0)),
            last_played: world.last_played,
        })
        .collect();
    let default_world = match made_by {
        ImageMaker::Host => level_name(properties.as_deref())
            .filter(|level| image.worlds.iter().any(|w| &w.name == level))
            .or_else(|| image.worlds.first().map(|w| w.name.clone())),
        _ => image
            .worlds
            .iter()
            .max_by_key(|w| w.last_played.unwrap_or(i64::MIN))
            .map(|w| w.name.clone()),
    };
    let blanked: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let keys = entry.extra.get(BLANKED_FIELD)?.as_array()?;
            Some(
                keys.iter()
                    .filter_map(|key| key.as_str())
                    .map(|key| {
                        format!(
                            "{}: {}",
                            display_text(entry.path.as_str()),
                            display_text(key)
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        // counted up to a bound: a hostile file can name 8 keys on each of a million entries
        .take(MAX_COUNTED_NAMES)
        .collect();
    let offline_mode = offline_mode(properties.as_deref());
    // what the server made with the default world unpacks and writes; the whole file's
    // when no server can be made
    let default_plan = server.is_some().then(|| {
        super::import::plan_checked(
            &image,
            &linked,
            properties.as_deref(),
            &carried,
            &super::import::PlanRequest {
                world: default_world.clone(),
                ..Default::default()
            },
            caps,
            &credentials,
        )
    });
    let default_plan = default_plan.and_then(Result::ok);
    let (unpacked_bytes, needed_bytes) = match &default_plan {
        Some(plan) => (
            opened.unpacked_bytes_of(&|sha256| plan.objects.contains(sha256)),
            plan.needed_bytes(&opened),
        ),
        None => (
            opened.unpacked_bytes(),
            needed(opened.unpacked_bytes(), &entries),
        ),
    };
    // What the server takes, with the default world: the plan's files and what it leaves
    // out for want of a source. The whole file's when no server can be made.
    let taken: Vec<&Entry> = match &default_plan {
        Some(plan) => plan.image.entries.iter().collect(),
        None => entries.iter().collect(),
    };
    let mods = |wanted: fn(&Source) -> bool, extra: &[&Entry]| {
        NameList::of(
            taken
                .iter()
                .chain(extra)
                .filter(|e| is_mod_path(&e.path) && wanted(&e.source))
                .map(|e| mod_name(&e.path)),
        )
    };
    let no_source: Vec<&Entry> = match &default_plan {
        Some(plan) => {
            let paths: HashSet<&str> = plan
                .dropped
                .iter()
                .filter(|(_, why)| *why == super::import::Why::NoSource)
                .map(|(path, _)| path.as_str())
                .collect();
            entries
                .iter()
                .filter(|e| paths.contains(e.path.as_str()))
                .collect()
        }
        None => entries
            .iter()
            .filter(|e| {
                matches!(e.source, Source::Curseforge { .. }) && !carried.contains(&e.sha256)
            })
            .collect(),
    };
    let (download_bytes, client_only, needs_global_files, scripts_skipped) = match &default_plan {
        Some(plan) => (
            plan.downloads
                .iter()
                .map(|e| e.size)
                .fold(0, u64::saturating_add),
            NameList::of(
                plan.dropped
                    .iter()
                    .filter(|(path, why)| {
                        *why == super::import::Why::ClientOnly
                            && CanonicalPath::new(path.as_str()).is_ok_and(|p| is_mod_path(&p))
                    })
                    .map(|(path, _)| path.split_once('/').map_or(path.as_str(), |(_, r)| r)),
            ),
            plan.needs_global,
            plan.dropped
                .iter()
                .filter(|(_, why)| *why == super::import::Why::Script)
                .count() as u64,
        ),
        None => (
            entries
                .iter()
                .filter(|e| downloadable(e))
                .map(|e| e.size)
                .fold(0, u64::saturating_add),
            NameList::of(client_only),
            needs_global_files,
            scripts_skipped,
        ),
    };
    let summary = ImageSummary {
        image: image.id.clone(),
        file_name: display_text(file_name).into_owned(),
        label: image.label.as_deref().map(|l| display_text(l).into_owned()),
        made_by,
        producer_version: display_text(&image.producer.version).into_owned(),
        created: image.created,
        minecraft: image.game.minecraft.clone(),
        loader: display_text(image.game.loader.as_str()).into_owned(),
        loader_version: image.game.loader_version.clone(),
        flavour: image
            .settings
            .server
            .as_ref()
            .and_then(|s| s.flavour.as_ref())
            .map(|f| display_text(f.as_str()).into_owned()),
        server,
        server_choices: server_choices(&image),
        unsupported,
        server_note,
        base: base_text(&image),
        mods_from_modrinth: mods(|s| matches!(s, Source::Modrinth { .. }), &[]),
        mods_from_curseforge: mods(|s| matches!(s, Source::Curseforge { .. }), &no_source),
        carried_mods: NameList::of(
            taken
                .iter()
                .filter(|e| is_mod_path(&e.path) && carried.contains(&e.sha256))
                .map(|e| mod_name(&e.path)),
        ),
        client_only,
        server_mods: taken.iter().filter(|e| is_mod_path(&e.path)).count() as u64,
        worlds,
        default_world,
        files: entries.len() as u64,
        total_bytes: container::files_size(&Image {
            entries: entries.clone(),
            ..Image::new(image.kind, image.game.clone())
        }),
        download_bytes,
        unpacked_bytes,
        needed_bytes,
        disk_free: crate::util::disk::free_space(disk_dir),
        not_downloadable: NameList::of(no_source.iter().map(|e| e.path.as_str())),
        left_out: NameList::of(
            image
                .excludes
                .iter()
                .chain(linked.iter().flat_map(|c| c.excludes.iter())),
        ),
        withheld: NameList::of(
            image
                .withheld
                .iter()
                .chain(linked.iter().flat_map(|c| c.withheld.iter()))
                .map(|w| w.path.as_str()),
        ),
        blanked: NameList::of(&blanked),
        secrets_not_taken: NameList::of(
            entries
                .iter()
                .filter(|e| {
                    in_view(e, Side::Server)
                        && (super::export::is_way_in(&e.path)
                            || credentials.contains(e.path.as_str()))
                })
                .map(|e| e.path.as_str()),
        ),
        not_taken: not_taken.into_iter().collect(),
        operators: NameList::of(
            operators.into_iter().chain(
                // Bukkit's permissions can give anyone the operator commands, and its
                // aliases run any command as the console
                entries
                    .iter()
                    .filter(|e| {
                        in_view(e, Side::Server)
                            && super::import::is_operators_file(&e.path)
                            && !is_top_named(&e.path, "ops.json")
                    })
                    .map(|e| super::import::operators_shown(&e.path)),
            ),
        ),
        offline_mode,
        needs_global_files,
        scripts_skipped,
    };
    Ok(Inspected {
        container: opened,
        entries,
        summary,
        properties,
        credential_files: credentials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, size: u64) -> Entry {
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            size,
            lodestone_image::hex::sha256_hex(path.as_bytes()),
            Source::Object {},
        )
    }

    /// Settings files are read by the exact names the server reads, whole: a spelling in
    /// other letters, two paths a disk that ignores letter case takes for one, and a
    /// settings file over 1 MiB are refused, so what the summary reads is what is placed.
    #[test]
    fn settings_files_must_be_the_ones_the_server_reads() {
        for refused in [
            vec![entry("SERVER.PROPERTIES", 10)],
            vec![entry("Ops.json", 10)],
            vec![entry("OPS.JSON", 10), entry("ops.json", 10)],
            vec![entry("notes/A.txt", 1), entry("Notes/a.txt", 1)],
            vec![entry("server.properties", MAX_SETTINGS_FILE + 1)],
            vec![entry("permissions.yml", MAX_SETTINGS_FILE + 1)],
        ] {
            assert!(check_entries(&refused).is_err(), "{refused:?}");
        }
        check_entries(&[
            entry("server.properties", MAX_SETTINGS_FILE),
            entry("ops.json", 10),
            entry("world/Server.properties", 10),
            entry("config/a.toml", 1),
            entry("config/b.toml", 1),
        ])
        .unwrap();
    }

    #[test]
    fn offline_mode_is_read_as_minecraft_reads_it() {
        for offline in [
            "online-mode=false\n",
            "motd=x\ronline-mode=false\n",
            "online-mode=no\n",
            "online-mode=0\n",
            "online-mode=true \n",
            "online-mode=\n",
            // Java trims nothing from the value: an escaped space or tab, or a space Rust
            // calls whitespace and Java's `Properties` doesn't, before `true` is off
            "online-mode=\\ true\n",
            "online-mode=\\ttrue\n",
            "online-mode=\\ TRUE\n",
            "online-mode=\u{3000}true\n",
            "online-mode=\u{a0}true\n",
        ] {
            assert!(offline_mode(Some(offline.as_bytes())), "{offline:?}");
        }
        // a Latin-1 file's no-break space (byte 0xA0) is off too
        assert!(offline_mode(Some(b"online-mode=\xA0true\n")));
        for online in ["online-mode=true\n", "online-mode = TRUE\n", "motd=x\n"] {
            assert!(!offline_mode(Some(online.as_bytes())), "{online:?}");
        }
        assert!(!offline_mode(None));
        assert_eq!(
            level_name(Some(b"motd=x\rlevel-name=other\n")).as_deref(),
            Some("other")
        );
    }

    /// A line the reader can't parse may be read by Java (`\uD800` and `\uDC00` alone are
    /// unpaired surrogates, which Java keeps), so a `server.properties` with one is refused
    /// rather than summarised without it; one the reader reads whole passes.
    #[test]
    fn properties_with_lines_the_reader_cant_parse_are_refused() {
        for refused in [
            "online-mode=false\\uD800\n",
            "online-mode=true\nonline-mode=false\\uDC00\n",
            "motd=\\u12\n",
            "motd=x\r=value\n",
        ] {
            let error = check_properties(Some(refused.as_bytes())).unwrap_err();
            assert!(matches!(error.kind, ErrorKind::BadRequest), "{refused:?}");
            assert!(!error.to_string().contains("online-mode"), "{error}");
        }
        for read in [
            "online-mode=false\n",
            "# comment\n\nmotd=\\u00e9\\uD83D\\uDE00\n",
            "",
        ] {
            check_properties(Some(read.as_bytes())).unwrap();
        }
        check_properties(None).unwrap();
    }

    #[test]
    fn every_operator_shows() {
        let ops = br#"[
            {"uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "name": "Notch"},
            {"uuid": "11111111-2222-3333-4444-555555555555", "name": ""},
            {"uuid": "66666666-2222-3333-4444-555555555555", "name": "Twin"},
            {"uuid": "77777777-2222-3333-4444-555555555555", "name": "Twin"},
            {"name": null}
        ]"#;
        let names = NameList::of(operator_names(Some(ops)).unwrap());
        assert_eq!(names.count, 5, "{names:?}");
        assert!(names
            .names
            .contains(&"11111111-2222-3333-4444-555555555555".to_string()));
        assert!(names
            .names
            .contains(&"Twin (66666666-2222-3333-4444-555555555555)".to_string()));
        assert!(names.names.contains(&"no uuid".to_string()));
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
        assert!(!names_modrinth_file(
            "https://cdn.modrinth.com/data/A%2e/versions/v1/x.jar",
            "A%2e",
            "v1"
        ));
    }

    #[test]
    fn servers_are_made_for_what_lodestone_can_run() {
        let image = |loader: Loader, flavour: Option<&str>| {
            let mut image = Image::new(
                lodestone_image::Kind::Data,
                lodestone_image::Game {
                    minecraft: "1.21.1".into(),
                    loader,
                    loader_version: None,
                },
            );
            image.settings.server = flavour.map(|f| lodestone_image::settings::ServerSettings {
                flavour: Some(ServerFlavour::parse(f)),
                ..Default::default()
            });
            image
        };
        assert_eq!(
            server_kind(&image(Loader::Vanilla, None)),
            Ok((FlavourKind::Vanilla, None))
        );
        assert_eq!(
            server_kind(&image(Loader::Vanilla, Some("paper"))),
            Ok((FlavourKind::Paper, None))
        );
        let (kind, note) = server_kind(&image(Loader::Vanilla, Some("purpur"))).unwrap();
        assert_eq!(kind, FlavourKind::Paper);
        assert!(note.unwrap().contains("purpur"));
        assert_eq!(
            server_kind(&image(Loader::Fabric, Some("fabric"))),
            Ok((FlavourKind::Fabric, None))
        );
        assert_eq!(
            server_kind(&image(Loader::NeoForge, None)),
            Ok((FlavourKind::NeoForge, None))
        );
        assert!(server_kind(&image(Loader::Quilt, None))
            .unwrap_err()
            .contains("Quilt"));
        assert!(server_kind(&image(Loader::Other("rift".into()), None)).is_err());
    }

    #[test]
    fn names_are_cut_and_counted() {
        let list = NameList::of((0..100).map(|n| format!("mod{n:03}.jar")));
        assert_eq!(list.count, 100);
        assert_eq!(list.names.len(), MAX_NAMES);
        assert_eq!(list.names[0], "mod000.jar");
        let shown = NameList::of(["evil\u{202E}rah.jar", "a.jar", "a.jar"]);
        assert_eq!(shown.count, 2);
        assert_eq!(shown.names, ["a.jar", "evil\\u{202E}rah.jar"]);
    }
}
