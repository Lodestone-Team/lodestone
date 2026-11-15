//! Making a new server from a `.lodeimg` file, a Host's or a Launcher's
//! (`POST /instance/create_from_image`).
//!
//! The server is made the way every new server is (`spawn_minecraft_instance_creation`):
//! the core's own installers set up the server software for the file's Minecraft version
//! and loader, with the port the request names and the EULA accepted only when the
//! request says the user ticked it, and the server is never started. Then the file's
//! files are placed into the new folder:
//!
//! 1. [`plan`] decides, before anything is made, what the server takes: the server's view
//!    of the file's files (`lodestone_image::view::in_view`: a player's own files and mods
//!    only a client runs are left out, but for a Host's file, whose mods were all on its
//!    server, the ones it gave players stay),
//!    never the server software (every program or script at the top included),
//!    `eula.txt`, the operators unless asked (`ops.json`, Bukkit's `permissions.yml` and
//!    `commands.yml`, permission mods' and plugins' data: LuckPerms', FTB Ranks'…), a file
//!    holding a password or token (`super::secrets`, found by `inspect`),
//!    or scripts unless the owner may add them, and nothing runnable (files are placed
//!    without their execute bits); its worlds laid out as a server's (one picked world from a
//!    Launcher's file, renamed to the level; every world of a Host's), split the Paper way
//!    for a Paper server. A file that can't be laid out is refused.
//! 2. [`place`] imports the objects of the files the plan takes into the backup store
//!    (every byte checked; a Launcher file's other worlds and a player's files are never
//!    unpacked),
//!    downloads every Modrinth file from Modrinth's CDN and checks its SHA-512 and SHA-256
//!    (one that can't be downloaded is placed from the file's bytes as a plain file when
//!    it carries them, else left out and listed), and materializes the files straight into
//!    the new folder: every file is created new, so nothing the installer wrote is
//!    overwritten (the file's copy of one is skipped). Then a jar of a Launcher's file
//!    that says it runs only on clients is deleted, the server's settings pinned (the
//!    port and query port, the level, no bound address, RCON and the management server
//!    off, every secret empty, functions at most at level 2), Paper's forwarding secret
//!    and the other YAML secrets an export blanks emptied, Spigot's restart script and
//!    Bukkit's permissions file, world folder and old command aliases set to their
//!    defaults, the owner's "For players" marks
//!    written, the pack the file
//!    is built on followed, and the server's first content image recorded, "Created from
//!    <file>". With the files that hold a key to the server or a service's login never
//!    taken (`export::is_way_in`), and nothing that runs the sender's files or gives
//!    operators' powers without the owner's say, the file's settings give its sender no
//!    way in; its mods and plugins are code from whoever made it, which the summary's
//!    trust advisory is for.
//!
//! Collections of the store are held off from the import of the objects to the end of
//! placing them: until then no manifest names them. Servers are made from files one at a
//! time (`Uploads::placing`), at most one from each upload (`Uploads::take`), each holding
//! what it writes among the core's large writes (`util::disk::LargeWrites`: uploads
//! arriving, exports and zips for download) and checking the free space again, less what
//! the others still need, before it unpacks and before it places. The objects a failed
//! creation unpacked are collected once past the store's grace.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use lodestone_image::container::Container;
use lodestone_image::paths::{in_names, placement, same_name};
use lodestone_image::share::placed_entries;
use lodestone_image::store::materialize::{materialize, MaterializeOptions, Selection};
use lodestone_image::store::{Store, NEVER};
use lodestone_image::view::in_view;
use lodestone_image::{
    display_text, CanonicalPath, DiskLayout, EnvFrom, EnvValue, Image, Kind, ServerLayout, Side,
    Source,
};
use tracing::warn;

use crate::error::{Error, ErrorKind};
use crate::images::layout::{server_settings, InstanceContext};
use crate::images::ModrinthHint;
use crate::implementations::minecraft::properties::PropertiesDocument;
use crate::implementations::minecraft::{Flavour, FlavourKind};
use crate::mods::client::{ClientMarks, MARKS_FILE};
use crate::types::InstanceUuid;
use crate::AppState;

use super::export::{
    is_way_in, java_line_breaks, pinned_yaml_of, rebuilt_properties, yaml_secrets_of, YamlCleaning,
};
use super::inspect::{
    check_entries, download_key, downloadable, is_script, is_server_software, level_name,
    needs_global, wanted_downloads, Caps, ImageMaker,
};

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: color_eyre::eyre::eyre!(message.into()),
    }
}

/// Why [`plan`] left a file out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// A player's own file (`options.txt`, resource packs): a server never holds it.
    ClientFile,
    /// A mod only clients run, by its sides.
    ClientOnly,
    /// Server software, which Lodestone installs itself.
    ServerSoftware,
    /// `eula.txt`: accepted by whoever runs the server.
    Eula,
    /// `ops.json`, Bukkit's `permissions.yml` or `commands.yml`, or permissions data
    /// ([`is_permissions_data`]: LuckPerms', FTB Ranks'…), without `keep_operators`.
    Operators,
    /// A script the importer may not add.
    Script,
    /// A world of a Launcher's file that wasn't picked.
    OtherWorld,
    /// A file with no bytes here that this core can't download (CurseForge, the pack's),
    /// or a settings file named as a download.
    NoSource,
    /// A key that lets its holder in (a proxy's forwarding secret, Floodgate's key, a
    /// Discord bot's token) or points the server at someone's database (LuckPerms'
    /// settings), or any file holding a password or token ([`super::secrets`]): the sender
    /// would keep a way into the new server, or their logins would configure it.
    Secret,
}

/// What the person asked for, besides the file.
#[derive(Debug, Clone, Default)]
pub struct PlanRequest {
    /// The world (its folder in the file) that becomes the server's level. For a
    /// Launcher's file the only world taken, none when `None`; for a Host's, every world is
    /// taken and `None` keeps the file's own level.
    pub world: Option<String>,
    /// Take the file's `ops.json`, Bukkit's `permissions.yml` and `commands.yml` and
    /// LuckPerms' data, which say who can run any command on the server.
    pub keep_operators: bool,
    /// The server to make, when the file's game allows a choice (a vanilla game: a vanilla
    /// or a Paper server, see [`super::inspect::server_choices`]); `None` takes the file's
    /// own ([`super::inspect::server_kind`]).
    pub flavour: Option<FlavourKind>,
}

/// What [`plan`] decided.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The server Lodestone makes.
    pub kind: FlavourKind,
    /// The files to place, as an image (`full`, its worlds the server's).
    pub image: Image,
    pub layout: ServerLayout,
    /// Each distinct download (a Modrinth file and claimed source) once.
    pub downloads: Vec<lodestone_image::Entry>,
    /// What it leaves out and why, by path.
    pub dropped: Vec<(String, Why)>,
    /// It places carried files a user without `WriteGlobalFile` couldn't put there.
    pub needs_global: bool,
    /// The carried objects its files name: all an import unpacks.
    pub objects: HashSet<String>,
    /// A Host made the file: its files were a server's, so they are taken as they were
    /// (mods the old server gave players included, no jar dropped for its sides).
    pub made_by_host: bool,
}

impl Plan {
    /// What making the server writes, for the free-space check (which adds 1 GiB to
    /// spare): the objects of `container` it unpacks into the store, the files it places
    /// and the downloads.
    pub fn needed_bytes(&self, container: &Container) -> u64 {
        let unpacked = container.unpacked_bytes_of(&|sha256| self.objects.contains(sha256));
        let files = self
            .image
            .entries
            .iter()
            .map(|e| e.size)
            .fold(0, u64::saturating_add);
        let downloads = self
            .downloads
            .iter()
            .map(|e| e.size)
            .fold(0, u64::saturating_add);
        unpacked.saturating_add(files).saturating_add(downloads)
    }
}

/// Whether `path` is the top-level file `name`, in any letter case.
fn is_top(path: &CanonicalPath, name: &str) -> bool {
    let mut parts = path.components();
    matches!((parts.next(), parts.next()), (Some(first), None) if same_name(first, name))
}

/// Whether `path` says who may run any command on the server: `ops.json`; Bukkit's
/// `permissions.yml`, which can give every player the operator commands' permissions;
/// Bukkit's `commands.yml`, whose aliases replace any command, `stop` and `save-off`
/// included, which the core types into the console (as the console, so an alias could
/// make anyone an operator when the owner stops the server); and LuckPerms' data
/// ([`is_permissions_data`]). Taken only with `keep_operators`; without them the server
/// writes its defaults.
pub fn is_operators_file(path: &CanonicalPath) -> bool {
    OPERATORS_FILES.iter().any(|name| is_top(path, name)) || is_permissions_data(path)
}

/// The top-level files [`is_operators_file`] names.
pub const OPERATORS_FILES: [&str; 3] = ["ops.json", "permissions.yml", "commands.yml"];

/// The permission plugins whose whole folder (`plugins/<folder>/`, any letter case) says
/// who has which permission, operators' commands included: PermissionsEx, GroupManager,
/// UltraPermissions, bPermissions, zPermissions and PowerRanks.
const PERMISSION_PLUGINS: &[&str] = &[
    "permissionsex",
    "groupmanager",
    "ultrapermissions",
    "bpermissions",
    "zpermissions",
    "powerranks",
];

/// Whether `path` is the data of a mod or plugin that gives players permissions or ranks
/// that can run operators' commands, or runs commands as the console, which say who can
/// run any command as surely as `ops.json` says who is an operator (see "M6a decisions"):
/// LuckPerms' folder (as a mod, `config/luckperms/`; as a plugin, `plugins/LuckPerms/`) but
/// its settings (a way in, never taken), whose local storage (an H2 or SQLite database,
/// `yaml-storage/`, `json-storage/`) can give anyone `*`; FTB Ranks' ranks and players in
/// any world's `serverconfig/ftbranks/` and the modpack's `defaultconfigs/ftbranks/` (its
/// `command.*` nodes override every command's permission level), and FTB Utilities' ranks
/// (`local/ftbutilities/` files whose name says rank); the folders of [`PERMISSION_PLUGINS`];
/// player-roles' `config/roles.json` (roles that give commands and operator powers);
/// ForgeEssentials' folder and its world data (`ForgeEssentials/`, `<world>/FEData/`: its
/// permissions and groups); and Command Aliases' aliases (`config/commandaliases*`, which
/// can run any command as the console, like Bukkit's `commands.yml`). Taken only with
/// `keep_operators`, and named in the summary's operators.
pub fn is_permissions_data(path: &CanonicalPath) -> bool {
    permissions_data_root(path).is_some()
}

/// The folder or file the summary names for permissions data ([`is_permissions_data`]):
/// `plugins/LuckPerms/`, `saves/world/serverconfig/ftbranks/`, `config/roles.json`.
fn permissions_data_root(path: &CanonicalPath) -> Option<String> {
    if is_way_in(path) {
        return None;
    }
    let parts: Vec<&str> = path.components().collect();
    let folder = |n: usize| format!("{}/", parts[..n].join("/"));
    match parts.as_slice() {
        [top, dir, _, ..]
            if in_names(top, &["config", "plugins"]) && same_name(dir, "luckperms") =>
        {
            Some(folder(2))
        }
        [top, dir, _, ..] if same_name(top, "plugins") && in_names(dir, PERMISSION_PLUGINS) => {
            Some(folder(2))
        }
        [saves, _, serverconfig, ftbranks, _, ..]
            if same_name(saves, "saves")
                && same_name(serverconfig, "serverconfig")
                && same_name(ftbranks, "ftbranks") =>
        {
            Some(folder(4))
        }
        [top, ftbranks, _, ..]
            if same_name(top, "defaultconfigs") && same_name(ftbranks, "ftbranks") =>
        {
            Some(folder(2))
        }
        [top, dir, name]
            if same_name(top, "local")
                && same_name(dir, "ftbutilities")
                && name.to_lowercase().contains("rank") =>
        {
            Some(path.to_string())
        }
        [top, name] if same_name(top, "config") && same_name(name, "roles.json") => {
            Some(path.to_string())
        }
        [top, name, ..]
            if same_name(top, "config") && name.to_lowercase().starts_with("commandaliases") =>
        {
            Some(format!("{top}/{name}"))
        }
        [top, _, ..] if same_name(top, "forgeessentials") => Some(folder(1)),
        [saves, _, data, _, ..] if same_name(saves, "saves") && same_name(data, "fedata") => {
            Some(folder(3))
        }
        _ => None,
    }
}

/// How the summary names an operators' file: permissions data by its folder (or file,
/// [`permissions_data_root`]), every other by its path.
pub fn operators_shown(path: &CanonicalPath) -> String {
    permissions_data_root(path).unwrap_or_else(|| path.to_string())
}

/// The top-level settings files the server reads by these exact names: `server.properties`,
/// which the summary reads before anything is placed, and the operators' files. A file
/// spelling one in other letters is refused ([`super::inspect::check_entries`]), and one is
/// placed only from the file's own bytes, never a download, so the summary says what is
/// placed.
pub const SETTINGS_FILES: [&str; 4] = [
    "server.properties",
    "ops.json",
    "permissions.yml",
    "commands.yml",
];

/// Whether `path` is one of [`SETTINGS_FILES`], in any letter case.
pub fn is_settings_file(path: &CanonicalPath) -> bool {
    SETTINGS_FILES.iter().any(|name| is_top(path, name))
}

/// The permission bits a file from an upload is placed with: never runnable, so nothing
/// the sender placed can be run as a program by name.
fn placed_from_upload(mode: u32) -> u32 {
    mode & 0o666
}

/// [`plan_checked`] with no file found holding a credential: for working out what a file
/// would place from its paths alone. Making a server passes `inspect`'s
/// [`super::inspect::Inspected::credential_files`] to [`plan_checked`].
pub fn plan(
    image: &Image,
    linked: &[Image],
    properties: Option<&[u8]>,
    carried: &HashSet<String>,
    request: &PlanRequest,
    caps: Caps,
) -> Result<Plan, Error> {
    plan_checked(
        image,
        linked,
        properties,
        carried,
        request,
        caps,
        &BTreeSet::new(),
    )
}

/// Decides what a server made from the file takes (see the module): the file's `image`
/// and `linked` content image, its `server.properties` (for the level), the objects it
/// `carried`, what was asked, what the person may add, and the files `credentials` names
/// (by path: the ones [`super::inspect::credential_files`] found holding a password or
/// token, never taken). Refuses, before anything is made, a file Lodestone can't make a
/// server from or lay out, and a world that isn't in the file.
pub fn plan_checked(
    image: &Image,
    linked: &[Image],
    properties: Option<&[u8]>,
    carried: &HashSet<String>,
    request: &PlanRequest,
    caps: Caps,
    credentials: &BTreeSet<String>,
) -> Result<Plan, Error> {
    let (kind, _) = super::inspect::chosen_server(image, request.flavour).map_err(bad_request)?;
    let entries = placed_entries(image, linked);
    check_entries(&entries)?;
    super::inspect::check_game(image)?;
    let made_by_host = matches!(made_by(image), ImageMaker::Host);
    if let Some(world) = &request.world {
        if !image.worlds.iter().any(|w| &w.name == world) {
            return Err(bad_request(format!(
                "The file has no world {}",
                display_text(world)
            )));
        }
    }

    let mut dropped = Vec::new();
    let mut kept = Vec::new();
    for entry in entries {
        let path = entry.path.clone();
        let server_path = placement(&path).allows(Side::Server);
        // A Host's file is a server's own folder: a mod at a server's path that its server
        // doesn't run (a client-only mod the owner gives players) was on that server, and
        // its client view gives it to players, so the new server keeps it for them too.
        let for_players = made_by_host && server_path;
        let why = if !in_view(&entry, Side::Server) && !for_players {
            Some(if server_path {
                Why::ClientOnly
            } else {
                Why::ClientFile
            })
        } else if is_top(&path, "eula.txt") {
            Some(Why::Eula)
        } else if is_server_software(&path) {
            Some(Why::ServerSoftware)
        } else if is_settings_file(&path) && !matches!(entry.source, Source::Object {}) {
            // read from the file's own bytes or not at all, so the summary says what is
            // placed
            Some(Why::NoSource)
        } else if is_operators_file(&path) && !request.keep_operators {
            Some(Why::Operators)
        } else if is_script(&path) && !caps.may_write_code {
            Some(Why::Script)
        } else if is_way_in(&path) || credentials.contains(path.as_str()) {
            Some(Why::Secret)
        } else if !matches!(entry.source, Source::Object {})
            && !downloadable(&entry)
            && !carried.contains(&entry.sha256)
        {
            Some(Why::NoSource)
        } else {
            None
        };
        match why {
            Some(why) => dropped.push((path.to_string(), why)),
            None => kept.push(entry),
        }
    }

    // The worlds, laid out as a server's.
    let tops: BTreeSet<String> = kept
        .iter()
        .filter(|e| e.world.is_none())
        .map(|e| e.path.first().to_string())
        .collect();
    let (level, worlds) = if made_by_host {
        let level = request
            .world
            .clone()
            .or_else(|| {
                level_name(properties).filter(|level| image.worlds.iter().any(|w| &w.name == level))
            })
            .or_else(|| image.worlds.first().map(|w| w.name.clone()))
            .unwrap_or_else(|| ServerLayout::unused_level(&tops));
        (level, image.worlds.clone())
    } else {
        let level = ServerLayout::unused_level(&tops);
        let mut worlds = Vec::new();
        let picked = request.world.as_deref();
        let mut renamed = Vec::with_capacity(kept.len());
        for mut entry in kept {
            match entry.world.clone() {
                None => renamed.push(entry),
                Some(world) if Some(world.as_str()) == picked => {
                    let rest = entry
                        .path
                        .as_str()
                        .splitn(3, '/')
                        .nth(2)
                        .unwrap_or_default()
                        .to_string();
                    entry.path = CanonicalPath::new(format!("saves/{level}/{rest}"))
                        .map_err(|e| bad_request(format!("A world's file can't be placed: {e}")))?;
                    entry.world = Some(level.clone());
                    renamed.push(entry);
                }
                Some(_) => dropped.push((entry.path.to_string(), Why::OtherWorld)),
            }
        }
        kept = renamed;
        if let Some(world) = image
            .worlds
            .iter()
            .find(|w| Some(w.name.as_str()) == picked)
        {
            let mut world = world.clone();
            world.name = level.clone();
            worlds.push(world);
        }
        (level, worlds)
    };
    let others: Vec<String> = worlds
        .iter()
        .map(|w| w.name.clone())
        .filter(|name| *name != level)
        .collect();
    let split = kind == FlavourKind::Paper;
    let layout = ServerLayout::new(level, others, split)
        .map_err(|e| bad_request(format!("The file's worlds can't be a server's: {e}")))?;
    let disk = DiskLayout::Server(layout.clone());
    for entry in &kept {
        disk.to_disk(&entry.path).map_err(|e| {
            bad_request(format!(
                "The file has {}, which a server can't hold: {e}",
                display_text(entry.path.as_str())
            ))
        })?;
    }

    for entry in &mut kept {
        entry.mode = placed_from_upload(entry.mode);
    }
    let needs_global = kept
        .iter()
        .any(|e| carried.contains(&e.sha256) && needs_global(&e.path));
    let downloads = wanted_downloads(&kept)?.into_iter().cloned().collect();
    let objects = kept
        .iter()
        .filter(|e| carried.contains(&e.sha256))
        .map(|e| e.sha256.clone())
        .collect();
    let mut placed = Image::new(Kind::Full, image.game.clone());
    placed.base = image.base.clone();
    placed.settings = server_settings(&Flavour::from(kind));
    placed.entries = kept;
    placed.worlds = worlds;
    placed.normalize();
    dropped.sort();
    Ok(Plan {
        kind,
        image: placed,
        layout,
        downloads,
        dropped,
        needs_global,
        objects,
        made_by_host,
    })
}

/// Who made the file, as [`super::inspect::ImageSummary::made_by`] says.
pub fn made_by(image: &Image) -> ImageMaker {
    match &image.producer.app {
        lodestone_image::manifest::App::Core => ImageMaker::Host,
        lodestone_image::manifest::App::Launcher => ImageMaker::Launcher,
        lodestone_image::manifest::App::Other(_) => ImageMaker::Unknown,
    }
}

/// How many downloads run at once.
const DOWNLOADS_AT_ONCE: usize = 8;

/// Once this many downloads in a row failed, no new one starts: the core's downloads don't
/// say whether anyone answered, so a whole round of failures is taken for no network or a
/// blocked host, and the rest count as not downloaded rather than each waiting its turn.
const FAILURES_IN_A_ROW: usize = DOWNLOADS_AT_ONCE;

/// Stops an import's downloads early ([`FAILURES_IN_A_ROW`]).
#[derive(Default)]
struct Breaker {
    failed_in_a_row: AtomicUsize,
    tripped: AtomicBool,
}

impl Breaker {
    fn open(&self) -> bool {
        self.tripped.load(Ordering::Relaxed)
    }

    fn succeeded(&self) {
        self.failed_in_a_row.store(0, Ordering::Relaxed);
    }

    fn failed(&self) {
        if self.failed_in_a_row.fetch_add(1, Ordering::Relaxed) + 1 >= FAILURES_IN_A_ROW {
            self.tripped.store(true, Ordering::Relaxed);
        }
    }
}

/// Why a download didn't store its file.
enum Missed {
    /// It wasn't downloaded, or not as the file says: logged, and the import goes on.
    NotDownloaded(String),
    /// The disk or the store failed: the import stops.
    Stop(Error),
}

/// What a failed download means for the import: a host that failed or answered no file, or
/// a file over its size, is one file not downloaded; anything else (the download's own
/// file couldn't be written: a full disk) stops the import, so a server isn't made
/// without its mods with the network blamed.
fn missed(shown: &str, e: Error) -> Missed {
    match e.kind {
        ErrorKind::External | ErrorKind::BadRequest => {
            Missed::NotDownloaded(format!("{shown} couldn't be downloaded: {}", e.source))
        }
        _ => Missed::Stop(Error {
            kind: e.kind,
            source: color_eyre::eyre::eyre!("{shown} couldn't be downloaded: {}", e.source),
        }),
    }
}

/// Downloads `entry` from Modrinth's CDN into `temp`, checks its size and SHA-512, and puts
/// it in the store, checking its SHA-256.
async fn download_one(
    state: &AppState,
    store: &std::sync::Arc<Store>,
    entry: &lodestone_image::Entry,
    temp: PathBuf,
) -> Result<(), Missed> {
    let shown = display_text(entry.path.as_str()).into_owned();
    let Source::Modrinth { url, .. } = &entry.source else {
        return Err(Missed::NotDownloaded(format!("{shown} isn't a download")));
    };
    let url = url::Url::parse(url)
        .map_err(|_| Missed::NotDownloaded(format!("{shown} has no download URL")))?;
    let mut file = tokio::fs::File::create(&temp).await.map_err(|e| {
        Missed::Stop(color_eyre::eyre::eyre!("Failed to create a download: {e}").into())
    })?;
    let fetched = state
        .mods
        .modrinth
        .download(&url, &mut file, entry.size)
        .await;
    drop(file);
    let result = match fetched {
        Err(e) => Err(missed(&shown, e)),
        Ok(hashes)
            if hashes.size != entry.size
                || !entry
                    .sha512
                    .as_deref()
                    .is_some_and(|sha512| hashes.sha512.eq_ignore_ascii_case(sha512)) =>
        {
            Err(Missed::NotDownloaded(format!(
                "{shown} didn't download as the file says (size or SHA-512)"
            )))
        }
        Ok(_) => {
            let (store, path, expected, size) = (
                store.clone(),
                temp.clone(),
                entry.sha256.clone(),
                entry.size,
            );
            let stored = crate::images::blocking(move || {
                store
                    .put_file(&path, NEVER)
                    .map_err(|e| color_eyre::eyre::eyre!("Failed to store a download: {e}").into())
            })
            .await
            .map_err(Missed::Stop)?;
            if stored.sha256 != expected || stored.size != size {
                // stored, but nothing names it: a collection takes it
                Err(Missed::NotDownloaded(format!(
                    "{shown} didn't download as the file says (SHA-256)"
                )))
            } else {
                Ok(())
            }
        }
    };
    let _ = tokio::fs::remove_file(&temp).await;
    result
}

/// Downloads `downloads` into the store, [`DOWNLOADS_AT_ONCE`] at a time, through files in
/// `root` (`.lodestone-download-<n>`, which images never record). Returns the downloads
/// that stored their file, by [`download_key`]. A download that fails is logged and the
/// import goes on; one that fails the disk or the store stops it.
async fn download_all(
    state: &AppState,
    store: &std::sync::Arc<Store>,
    downloads: &[lodestone_image::Entry],
    root: &Path,
    progress: &(dyn Fn(usize) + Sync),
) -> Result<HashSet<(String, String)>, Error> {
    use futures::StreamExt as _;
    let breaker = Breaker::default();
    let breaker = &breaker;
    let done = AtomicUsize::new(0);
    let done = &done;
    let results: Vec<Result<Option<(String, String)>, Error>> =
        // owned, so the stream's closure takes no borrowed argument (its future must be
        // Send for any lifetime)
        futures::stream::iter(downloads.iter().cloned().enumerate())
            .map(|(n, entry)| async move {
                let result = if breaker.open() {
                    Ok(None)
                } else {
                    let temp = root.join(format!(".lodestone-download-{n}"));
                    match download_one(state, store, &entry, temp).await {
                        Ok(()) => {
                            breaker.succeeded();
                            Ok(Some(download_key(&entry)))
                        }
                        Err(Missed::NotDownloaded(why)) => {
                            warn!("{why}");
                            breaker.failed();
                            Ok(None)
                        }
                        Err(Missed::Stop(e)) => Err(e),
                    }
                };
                progress(done.fetch_add(1, Ordering::Relaxed) + 1);
                result
            })
            .buffer_unordered(DOWNLOADS_AT_ONCE)
            .collect()
            .await;
    if breaker.open() {
        warn!("An image's downloads kept failing, so the rest weren't tried");
    }
    let mut verified = HashSet::new();
    for result in results {
        if let Some(key) = result? {
            verified.insert(key);
        }
    }
    Ok(verified)
}

/// What [`place`] did, for the creation's last message.
#[derive(Debug, Default)]
pub struct Placed {
    /// Files that couldn't be had (no download, no bytes in the file), by path.
    pub left_out: Vec<String>,
    /// Mods deleted because their jar says they run only on clients, by name.
    pub client_jars: Vec<String>,
    /// Settings files deleted because they couldn't be cleaned for sure (see
    /// [`clear_yaml_secrets`]), by path: the server writes its defaults instead.
    pub settings_removed: Vec<String>,
}

/// What a new server needs to know about itself while its files are placed.
pub struct NewServer<'a> {
    pub uuid: &'a InstanceUuid,
    pub root: &'a Path,
    pub version: &'a str,
    pub flavour: &'a Flavour,
    pub port: u32,
    /// The uploaded file's name, for the first content image's label.
    pub file_name: &'a str,
}

/// Places `plan`'s files from the open file `container` into the new server's folder (see
/// the module). The folder holds what the installer wrote; nothing of it is overwritten.
pub async fn place(
    state: &AppState,
    mut container: Container,
    plan: Plan,
    server: NewServer<'_>,
    progress: &(dyn Fn(&str, f64) + Sync),
) -> Result<Placed, Error> {
    // one creation from a file at a time: each counts on the free space it checks
    let _placing = state.image_uploads.placing().await;
    // what it writes, held among the core's large writes (uploads arriving, exports and
    // zips for download) until the files are placed, so none counts on the same space
    let space = Arc::new(
        state
            .images
            .large_writes
            .reserve(plan.needed_bytes(&container)),
    );
    let store = state.images.store();
    let hold = {
        let store = store.clone();
        crate::images::blocking(move || {
            store
                .hold_off_collection()
                .map_err(|e| color_eyre::eyre::eyre!("{e}").into())
        })
        .await?
    };
    progress("Unpacking the file", 0.0);
    {
        let store = store.clone();
        let root = server.root.to_path_buf();
        let wanted = plan.objects.clone();
        let space = space.clone();
        // the rest of what this creation writes, beside what it unpacks
        let rest = plan
            .needed_bytes(&container)
            .saturating_sub(container.unpacked_bytes_of(&|sha256| wanted.contains(sha256)));
        container = crate::images::blocking(move || {
            // every carried byte checked, the objects the store already has too: a hash
            // alone mustn't reach another server's files
            container
                .import_objects_where_verified(
                    &store,
                    &|sha256| wanted.contains(sha256),
                    &|bytes| {
                        space
                            .check(&root, bytes.saturating_add(rest), "to unpack the file")
                            .map_err(|e| e.source.to_string())
                    },
                    NEVER,
                )
                .map_err(super::inspect::container_error)?;
            space.set_remaining(rest);
            Ok(container)
        })
        .await?;
    }
    drop(container);

    let total = plan.downloads.len().max(1);
    let verified = download_all(state, &store, &plan.downloads, server.root, &|done| {
        if done % 8 == 0 || done == total {
            progress(
                &format!("Downloaded {done} of {} files", plan.downloads.len()),
                0.0,
            );
        }
    })
    .await?;

    // A file no download confirmed is placed from the file's own bytes when it carries
    // them (imported above, every byte checked), as a plain file: nothing says it is that
    // project's. Never from the store's other objects, which are other servers' files a
    // hash alone mustn't reach. Else it is left out.
    let mut image = plan.image;
    let mut left_out = Vec::new();
    image.entries.retain_mut(|entry| {
        if matches!(entry.source, Source::Object {}) {
            return true;
        }
        if verified.contains(&download_key(entry)) {
            return true;
        }
        if plan.objects.contains(&entry.sha256) {
            entry.source = Source::Object {};
            true
        } else {
            left_out.push(entry.path.to_string());
            false
        }
    });
    for entry in &image.entries {
        if let (Source::Modrinth { url, .. }, Some(sha512)) = (&entry.source, &entry.sha512) {
            state.images.note_modrinth(
                sha512,
                ModrinthHint {
                    url: Some(url.clone()),
                    ..Default::default()
                },
            );
        }
    }

    progress("Placing the files", 0.0);
    let has_properties = image
        .entries
        .iter()
        .any(|e| is_top(&e.path, "server.properties"));
    let image = {
        let (store, root, layout, space) = (
            store.clone(),
            server.root.to_path_buf(),
            DiskLayout::Server(plan.layout.clone()),
            space.clone(),
        );
        let mut image = image;
        crate::images::blocking(move || {
            if has_properties {
                // the file's own settings replace the installer's
                match std::fs::remove_file(root.join("server.properties")) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        return Err(color_eyre::eyre::eyre!(
                            "Failed to replace server.properties: {e}"
                        )
                        .into())
                    }
                    _ => {}
                }
            }
            // what the installer wrote stays the new server's own (a log it writes, a
            // file of its loader's): every file is created new, so one already there
            // would fail the whole placing
            image.entries.retain(|entry| {
                let there = layout
                    .to_disk(&entry.path)
                    .is_ok_and(|disk| std::fs::symlink_metadata(root.join(disk)).is_ok());
                if there {
                    warn!(
                        "The new server's setup already wrote {}, so the file's copy isn't placed",
                        display_text(entry.path.as_str())
                    );
                }
                !there
            });
            // the downloads took their share since the last check
            let files = image
                .entries
                .iter()
                .map(|e| e.size)
                .fold(0, u64::saturating_add);
            space.check(&root, files, "to place the file's files")?;
            let options = MaterializeOptions {
                selection: Selection::All,
                durable: true,
                cancel: NEVER,
            };
            materialize(&store, &image, &layout, &root, &options).map_err(|e| {
                Error::from(color_eyre::eyre::eyre!(
                    "The file's files couldn't be placed: {e}"
                ))
            })?;
            Ok(image)
        })
        .await?
    };
    drop(hold);
    drop(space);

    let client_jars = if plan.made_by_host {
        Vec::new()
    } else {
        remove_client_jars(&image, server.root, server.flavour).await
    };
    pin_settings(server.root, server.port, plan.layout.level_name()).await?;
    let settings_removed = clear_yaml_secrets(
        &image,
        server.root,
        &DiskLayout::Server(plan.layout.clone()),
    )
    .await?;
    write_marks(&image, server.root).await?;
    if image.base.is_some() {
        crate::modpack::pack_info::follow_base(&state.images, server.uuid, &image).await;
    }
    let ctx = InstanceContext {
        uuid: server.uuid.clone(),
        root: server.root.to_path_buf(),
        game_version: server.version.to_string(),
        flavour: server.flavour.clone(),
        level_name: plan.layout.level_name().to_string(),
        policy_exclude: Vec::new(),
        running: false,
    };
    // the uploader named the file: written visibly, so it can't spoof the history
    let label: String = format!("Created from {}", display_text(server.file_name))
        .chars()
        .take(lodestone_image::manifest::MAX_TEXT_LEN)
        .collect();
    if let Err(e) = state.images.record_now(ctx, label).await {
        warn!(
            "Failed to record the content of the new server {}: {}",
            server.uuid, e.source
        );
    }
    left_out.sort();
    Ok(Placed {
        left_out,
        client_jars,
        settings_removed,
    })
}

/// Deletes the placed mods whose sides nothing knew that say themselves (their
/// `fabric.mod.json` or `quilt.mod.json`) they run only on clients: a server that loads
/// one crashes. Returns their names. A jar that can't be read stays.
async fn remove_client_jars(image: &Image, root: &Path, flavour: &Flavour) -> Vec<String> {
    let Some(loader) = crate::images::env::mod_loader(flavour) else {
        return Vec::new();
    };
    if loader.kind() != crate::mods::InstalledModKind::Mod {
        return Vec::new();
    }
    let mut removed = Vec::new();
    for entry in &image.entries {
        let parts: Vec<&str> = entry.path.components().collect();
        let [folder, name] = parts.as_slice() else {
            continue;
        };
        if !same_name(folder, "mods")
            || entry.env.server != EnvValue::Unknown
            || !crate::mods::is_mod_file_name(name)
        {
            continue;
        }
        let path = root.join(folder).join(name);
        let Ok(scanned) = crate::mods::scan::scan_file(&path, loader).await else {
            continue;
        };
        if scanned.environment == Some(crate::mods::metadata::JarEnvironment::Client) {
            match tokio::fs::remove_file(&path).await {
                Ok(()) => removed.push(name.to_string()),
                Err(e) => warn!("Failed to remove the client-only mod {name}: {e}"),
            }
        }
    }
    removed
}

/// The new server's `server.properties`: its port (the query port too, which was the old
/// computer's and may be another server's here) and level, no address to bind
/// (`server-ip` was the old computer's), RCON and the management server off, and every
/// secret empty, whatever the file said. Read with every line break Java takes (a lone
/// carriage return too), so no setting hides inside another's line, and written again from
/// the properties the reader parsed alone ([`rebuilt_properties`]), so no line it couldn't
/// parse reaches the server: what the server reads is what the summary read.
pub(super) async fn pin_settings(root: &Path, port: u32, level: &str) -> Result<(), Error> {
    let path = root.join("server.properties");
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            return Err(color_eyre::eyre::eyre!("Failed to read server.properties: {e}").into())
        }
    };
    let (read, _) = PropertiesDocument::from_bytes(&java_line_breaks(&bytes));
    let mut document = rebuilt_properties(&read);
    document.set("server-port", &port.to_string());
    document.set("query.port", &port.to_string());
    // the address the old computer bound: no address on this one
    document.set("server-ip", "");
    document.set("level-name", level);
    document.set("enable-rcon", "false");
    document.set("management-server-enabled", "false");
    // online mode written as the `true` or `false` Minecraft reads, so the placed file says
    // plainly what the summary showed (`\ true` is off to Minecraft)
    let online = document
        .get("online-mode")
        .map(super::inspect::online_mode_value);
    if let Some(online) = online {
        document.set("online-mode", if online { "true" } else { "false" });
    }
    // functions (a world's datapacks) run at this level: above 2 they can run /op
    let level = document
        .get("function-permission-level")
        .and_then(|level| level.trim().parse::<u8>().ok());
    if !level.is_some_and(|level| level <= MAX_FUNCTION_LEVEL) {
        document.set("function-permission-level", &MAX_FUNCTION_LEVEL.to_string());
    }
    crate::persist::write_atomic(&path, document.to_bytes(), None).await
}

/// The highest `function-permission-level` a server from a file keeps: Minecraft's
/// default. At 3 or 4 a datapack's functions can run `/op`, `/whitelist` and `/ban`, so a
/// world from the file could make its sender an operator.
const MAX_FUNCTION_LEVEL: u8 = 2;

/// Empties the secrets of the placed YAML files an export cleans (Paper's Velocity
/// forwarding secret, Pufferfish's Sentry key), as an export would have: a sender's
/// forwarding secret would let them join through their own proxy as any player. With it
/// empty, Paper refuses forwarded players until the owner sets their own. And sets the
/// settings of [`super::export::pinned_yaml_of`] (Spigot's restart script, Bukkit's
/// permissions file, world folder and old command aliases), which could run the sender's
/// file, make them an operator or put the worlds outside the server's folder. A file
/// that can't be cleaned (too big, written in a form the cleaning doesn't read) is
/// deleted, and the server writes its defaults at its first start. Returns the deleted
/// files' paths, which the creation's last message names: the owner's settings from the
/// file are gone.
async fn clear_yaml_secrets(
    image: &Image,
    root: &Path,
    layout: &DiskLayout,
) -> Result<Vec<String>, Error> {
    let mut removed = Vec::new();
    for entry in &image.entries {
        let pinned = pinned_yaml_of(&entry.path);
        let secrets = yaml_secrets_of(&entry.path);
        if pinned.is_none() && secrets.is_none() {
            continue;
        }
        let Ok(disk) = layout.to_disk(&entry.path) else {
            continue;
        };
        let path = root.join(disk);
        let shown = display_text(entry.path.as_str()).into_owned();
        let cleaning = match tokio::fs::read(&path).await {
            Ok(bytes) => match (secrets, pinned) {
                (Some(keys), _) => super::export::clean_yaml(entry.size, &bytes, keys),
                (None, Some(pins)) => super::export::pin_yaml_all(entry.size, &bytes, pins),
                (None, None) => continue,
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => YamlCleaning::Withhold,
        };
        match cleaning {
            YamlCleaning::Unchanged => {}
            YamlCleaning::Blanked(bytes, _) => {
                crate::persist::write_atomic(&path, bytes, None).await?;
            }
            YamlCleaning::Withhold => {
                warn!("{shown} can't be cleaned, so it isn't placed");
                tokio::fs::remove_file(&path).await.map_err(|e| {
                    Error::from(color_eyre::eyre::eyre!("Failed to remove {shown}: {e}"))
                })?;
                removed.push(entry.path.as_str().to_string());
            }
        }
    }
    removed.sort();
    Ok(removed)
}

/// The owner's "For players" marks the file's mods carry, as the marks file the Mods tab
/// and the recorder read.
async fn write_marks(image: &Image, root: &Path) -> Result<(), Error> {
    let mut marks = ClientMarks::default();
    for entry in &image.entries {
        let parts: Vec<&str> = entry.path.components().collect();
        let [folder, name] = parts.as_slice() else {
            continue;
        };
        if !same_name(folder, "mods") || !entry.env_from.contains(&EnvFrom::OwnerMark) {
            continue;
        }
        let name = crate::mods::enabled_name(name);
        match entry.env.client {
            EnvValue::Required => {
                marks.required.insert(name);
            }
            EnvValue::Unsupported => {
                marks.excluded.insert(name);
            }
            _ => {}
        }
    }
    if marks.required.is_empty() && marks.excluded.is_empty() {
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&marks)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to write the marks: {e}"))?;
    crate::persist::write_atomic(&root.join("mods").join(MARKS_FILE), text, None).await
}

/// The creation's last message: what the new server didn't get.
pub fn describe(plan_dropped: &[(String, Why)], placed: &Placed) -> Option<String> {
    let mut parts = Vec::new();
    let count = |why: Why| plan_dropped.iter().filter(|(_, w)| *w == why).count();
    let names = |paths: &[String]| {
        let shown: Vec<String> = paths
            .iter()
            .take(8)
            .map(|p| display_text(p).into_owned())
            .collect();
        let more = if paths.len() > 8 { ", …" } else { "" };
        format!("{}{more}", shown.join(", "))
    };
    if !placed.left_out.is_empty() {
        parts.push(format!(
            "{} file(s) couldn't be downloaded and were left out: {}",
            placed.left_out.len(),
            names(&placed.left_out)
        ));
    }
    let no_source: Vec<String> = plan_dropped
        .iter()
        .filter(|(_, w)| *w == Why::NoSource)
        .map(|(p, _)| p.clone())
        .collect();
    if !no_source.is_empty() {
        parts.push(format!(
            "{} file(s) can't be downloaded here and were left out: {}",
            no_source.len(),
            names(&no_source)
        ));
    }
    if !placed.client_jars.is_empty() {
        parts.push(format!(
            "{} mod(s) that run only on clients were removed: {}",
            placed.client_jars.len(),
            names(&placed.client_jars)
        ));
    }
    let secrets: Vec<String> = plan_dropped
        .iter()
        .filter(|(_, w)| *w == Why::Secret)
        .map(|(p, _)| p.clone())
        .collect();
    if !secrets.is_empty() {
        parts.push(format!(
            "{} file(s) holding a key to the server, a password or a token were left out, so the server writes its defaults: {}",
            secrets.len(),
            names(&secrets)
        ));
    }
    if !placed.settings_removed.is_empty() {
        parts.push(format!(
            "{} settings file(s) couldn't be checked for keys to the server and were left out, so the server writes its defaults: {}",
            placed.settings_removed.len(),
            names(&placed.settings_removed)
        ));
    }
    let scripts = count(Why::Script);
    if scripts > 0 {
        parts.push(format!(
            "{scripts} script file(s) were left out: only the owner can add scripts, while code execution is on"
        ));
    }
    (!parts.is_empty()).then(|| parts.join(". "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lodestone_image::manifest::App;
    use lodestone_image::{Entry, Game, Loader, Producer, World};

    fn entry(path: &str) -> Entry {
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            path.len() as u64,
            lodestone_image::hex::sha256_hex(path.as_bytes()),
            Source::Object {},
        )
    }

    fn world(name: &str) -> World {
        World {
            name: name.into(),
            level_name: None,
            data_version: None,
            game_version: None,
            size: None,
            last_played: None,
            quiesced: None,
        }
    }

    fn image(app: App, paths: &[&str], worlds: &[&str]) -> Image {
        let mut image = Image::new(
            Kind::Data,
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
        );
        image.producer = Producer {
            app,
            version: "1".into(),
        };
        image.entries = paths.iter().map(|p| entry(p)).collect();
        image.worlds = worlds.iter().map(|w| world(w)).collect();
        image.normalize();
        image
    }

    fn carried(image: &Image) -> HashSet<String> {
        image.entries.iter().map(|e| e.sha256.clone()).collect()
    }

    fn placed(plan: &Plan) -> Vec<&str> {
        plan.image.entries.iter().map(|e| e.path.as_str()).collect()
    }

    fn dropped(plan: &Plan, why: Why) -> Vec<&str> {
        plan.dropped
            .iter()
            .filter(|(_, w)| *w == why)
            .map(|(p, _)| p.as_str())
            .collect()
    }

    const OWNER: Caps = Caps {
        may_write_global: true,
        may_write_code: true,
    };

    #[test]
    fn only_a_download_that_failed_its_host_is_one_file_left_out() {
        let error = |kind| Error {
            kind,
            source: color_eyre::eyre::eyre!("x"),
        };
        for kind in [ErrorKind::External, ErrorKind::BadRequest] {
            assert!(matches!(
                missed("a.jar", error(kind)),
                Missed::NotDownloaded(_)
            ));
        }
        // the download's own file couldn't be written (a full disk): the import stops
        let write_failed: Error = color_eyre::eyre::eyre!("Failed to write the download").into();
        assert!(matches!(missed("a.jar", write_failed), Missed::Stop(_)));
    }

    #[test]
    fn a_launchers_picked_world_becomes_the_level() {
        let file = image(
            App::Launcher,
            &[
                "saves/A/level.dat",
                "saves/B/level.dat",
                "saves/B/DIM1/region/r.0.0.mca",
                "options.txt",
                "mods/a.jar",
            ],
            &["A", "B"],
        );
        let request = PlanRequest {
            world: Some("B".into()),
            keep_operators: false,
            flavour: None,
        };
        let plan = super::plan(&file, &[], None, &carried(&file), &request, OWNER).unwrap();
        assert_eq!(
            placed(&plan),
            [
                "mods/a.jar",
                "saves/world/DIM1/region/r.0.0.mca",
                "saves/world/level.dat"
            ]
        );
        assert_eq!(plan.layout.level_name(), "world");
        assert_eq!(plan.image.worlds.len(), 1);
        assert_eq!(plan.image.worlds[0].name, "world");
        assert_eq!(dropped(&plan, Why::OtherWorld), ["saves/A/level.dat"]);
        assert_eq!(dropped(&plan, Why::ClientFile), ["options.txt"]);
        assert!(!plan.layout.is_split());

        // no world picked: none taken; a folder named like the level moves it aside
        let file = image(
            App::Launcher,
            &["saves/A/level.dat", "world/notes.txt"],
            &["A"],
        );
        let none = super::plan(
            &file,
            &[],
            None,
            &carried(&file),
            &PlanRequest::default(),
            OWNER,
        )
        .unwrap();
        assert_eq!(placed(&none), ["world/notes.txt"]);
        assert_eq!(none.layout.level_name(), "world1");
        let picked = PlanRequest {
            world: Some("A".into()),
            keep_operators: false,
            flavour: None,
        };
        let aside = super::plan(&file, &[], None, &carried(&file), &picked, OWNER).unwrap();
        assert_eq!(
            placed(&aside),
            ["saves/world1/level.dat", "world/notes.txt"]
        );
    }

    #[test]
    fn a_hosts_worlds_all_come_and_its_level_stays() {
        let file = image(
            App::Core,
            &[
                "saves/survival/level.dat",
                "saves/creative/level.dat",
                "server.properties",
                "ops.json",
                "commands.yml",
                "permissions.yml",
                "eula.txt",
                "server.jar",
                "libraries/x.jar",
                "scripts/a.js",
            ],
            &["survival", "creative"],
        );
        let properties = b"level-name=survival\n";
        let caps = Caps {
            may_write_global: true,
            may_write_code: false,
        };
        let plan = super::plan(
            &file,
            &[],
            Some(properties),
            &carried(&file),
            &PlanRequest::default(),
            caps,
        )
        .unwrap();
        assert_eq!(plan.layout.level_name(), "survival");
        assert_eq!(
            plan.layout.worlds().collect::<Vec<_>>(),
            ["creative", "survival"]
        );
        assert_eq!(
            placed(&plan),
            [
                "saves/creative/level.dat",
                "saves/survival/level.dat",
                "server.properties"
            ]
        );
        // a command alias runs as the console when the core types `stop`
        assert_eq!(
            dropped(&plan, Why::Operators),
            ["commands.yml", "ops.json", "permissions.yml"]
        );
        assert_eq!(dropped(&plan, Why::Eula), ["eula.txt"]);
        assert_eq!(
            dropped(&plan, Why::ServerSoftware),
            ["libraries/x.jar", "server.jar"]
        );
        assert_eq!(dropped(&plan, Why::Script), ["scripts/a.js"]);

        let keep = PlanRequest {
            world: Some("creative".into()),
            keep_operators: true,
            flavour: None,
        };
        let plan =
            super::plan(&file, &[], Some(properties), &carried(&file), &keep, OWNER).unwrap();
        assert_eq!(plan.layout.level_name(), "creative");
        assert!(placed(&plan).contains(&"ops.json"));
        assert!(placed(&plan).contains(&"commands.yml"));
        assert!(placed(&plan).contains(&"scripts/a.js"));
    }

    /// Floodgate's folder is never taken (its settings name the key's file, so a key under
    /// another name is read as surely as `key.pem`), and LuckPerms' local storage, which
    /// can give anyone every permission, comes only with the operators.
    #[test]
    fn floodgate_never_comes_and_luckperms_data_only_with_the_operators() {
        let paths = [
            "config/floodgate/config.yml",
            "config/floodgate/k2.pem",
            "plugins/floodgate/config.yml",
            "plugins/floodgate/k2.pem",
            "plugins/LuckPerms/config.yml",
            "plugins/LuckPerms/luckperms-h2-v2.mv.db",
            "plugins/LuckPerms/yaml-storage/users/x.yml",
            "config/luckperms/luckperms.conf",
            "config/luckperms/json-storage/groups/default.json",
            "config/sodium.json",
        ];
        let mut file = image(App::Core, &paths, &[]);
        file.game.loader = Loader::Vanilla;
        file.game.loader_version = None;
        file.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let sorted = |mut paths: Vec<&str>| {
            paths.sort();
            paths.into_iter().map(str::to_string).collect::<Vec<_>>()
        };
        let plan = super::plan(
            &file,
            &[],
            None,
            &carried(&file),
            &PlanRequest::default(),
            OWNER,
        )
        .unwrap();
        assert_eq!(placed(&plan), ["config/sodium.json"]);
        assert_eq!(
            sorted(dropped(&plan, Why::Secret)),
            sorted(vec![
                "config/floodgate/config.yml",
                "config/floodgate/k2.pem",
                "config/luckperms/luckperms.conf",
                "plugins/LuckPerms/config.yml",
                "plugins/floodgate/config.yml",
                "plugins/floodgate/k2.pem",
            ])
        );
        let luckperms_data = sorted(vec![
            "config/luckperms/json-storage/groups/default.json",
            "plugins/LuckPerms/luckperms-h2-v2.mv.db",
            "plugins/LuckPerms/yaml-storage/users/x.yml",
        ]);
        assert_eq!(sorted(dropped(&plan, Why::Operators)), luckperms_data);

        let keep = PlanRequest {
            keep_operators: true,
            ..Default::default()
        };
        let plan = super::plan(&file, &[], None, &carried(&file), &keep, OWNER).unwrap();
        let mut expected = luckperms_data.clone();
        expected.push("config/sodium.json".into());
        assert_eq!(
            sorted(placed(&plan)),
            sorted(expected.iter().map(String::as_str).collect())
        );
        assert_eq!(
            operators_shown(
                &CanonicalPath::new("plugins/LuckPerms/yaml-storage/users/x.yml").unwrap()
            ),
            "plugins/LuckPerms/"
        );
        assert_eq!(
            operators_shown(&CanonicalPath::new("ops.json").unwrap()),
            "ops.json"
        );
    }

    #[test]
    fn what_a_server_cant_hold_or_get_is_refused_or_left_out() {
        let mut file = image(App::Launcher, &["saves/A/level.dat"], &["A"]);
        let unknown = PlanRequest {
            world: Some("Z".into()),
            keep_operators: false,
            flavour: None,
        };
        assert!(super::plan(&file, &[], None, &carried(&file), &unknown, OWNER).is_err());

        // a Host's world named like server content
        let bad = image(App::Core, &["saves/mods/level.dat"], &["mods"]);
        let error = super::plan(
            &bad,
            &[],
            None,
            &carried(&bad),
            &PlanRequest::default(),
            OWNER,
        )
        .unwrap_err();
        assert!(error.source.to_string().contains("worlds"), "{error}");

        // a CurseForge file it doesn't carry, a client-only mod, a carried jar
        let mut curseforge = entry("mods/cf.jar");
        curseforge.source = Source::Curseforge {
            project: 1,
            file: 2,
            downloadable: true,
        };
        let mut client = entry("mods/client.jar");
        client.env.server = EnvValue::Unsupported;
        file.entries
            .extend([curseforge, client, entry("mods/own.jar")]);
        file.normalize();
        let mut carried_here = carried(&file);
        carried_here.remove(&lodestone_image::hex::sha256_hex(b"mods/cf.jar"));
        let caps = Caps::default();
        let plan = super::plan(
            &file,
            &[],
            None,
            &carried_here,
            &PlanRequest::default(),
            caps,
        )
        .unwrap();
        assert_eq!(dropped(&plan, Why::NoSource), ["mods/cf.jar"]);
        assert_eq!(dropped(&plan, Why::ClientOnly), ["mods/client.jar"]);
        assert!(plan.needs_global, "a carried jar in the mods folder");
    }

    #[test]
    fn a_paper_server_splits_its_level() {
        let mut file = image(
            App::Launcher,
            &["saves/A/level.dat", "saves/A/DIM-1/region/r.0.0.mca"],
            &["A"],
        );
        file.game.loader = Loader::Vanilla;
        file.game.loader_version = None;
        file.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Purpur),
            ..Default::default()
        });
        let picked = PlanRequest {
            world: Some("A".into()),
            keep_operators: false,
            flavour: None,
        };
        let plan = super::plan(&file, &[], None, &carried(&file), &picked, OWNER).unwrap();
        assert_eq!(plan.kind, FlavourKind::Paper);
        assert!(plan.layout.is_split());
        let disk = DiskLayout::Server(plan.layout.clone());
        assert_eq!(
            disk.to_disk(&CanonicalPath::new("saves/world/DIM-1/region/r.0.0.mca").unwrap())
                .unwrap(),
            "world_nether/DIM-1/region/r.0.0.mca"
        );
    }
}
