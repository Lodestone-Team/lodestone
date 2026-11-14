//! Restoring a backup: which files a restore replaces, staging the image's files next to
//! them, the journaled swap, and repairing an interrupted swap at startup.
//!
//! A restore makes the files a backup would take now match the backup's image, and leaves
//! everything else as it is: what images never record (logs, caches, `session.lock`,
//! `.lodestone_config`), what the current policy excludes, links, files whose names an
//! image can't hold, and files the backup couldn't carry (its withheld files). So the
//! safety backup taken first has everything the restore replaces or removes. The scope
//! narrows what is replaced:
//!
//! - Everything: every such file.
//! - World: the files of the backup's level (on Paper and Spigot, its `_nether` and
//!   `_the_end` folders too).
//! - World and its mods: the level, and the server's content (`crate::images::scope`) but
//!   its settings and admin lists, as the backup has them. The backup's data image is a
//!   whole snapshot of the server, so its content files are the ones that ran with that
//!   world; the content image it links to may be older, since what a running server writes
//!   (an op removed, a config a mod rewrote) never makes a content image. The link is still
//!   required: it says the backup knows the content it ran with.
//!
//! The image's canonical paths are placed with the server's layout: worlds at the top of the
//! server folder, and on Paper and Spigot the level's `DIM-1` in `<level>_nether/` and its
//! `DIM1` in `<level>_the_end/`.
//!
//! An in-place restore goes:
//! 1. [`plan`]: the server's files the restore replaces (a walk, as a backup's, reading
//!    nothing), the image's files it places, and the top-level names it swaps.
//! 2. [`diff`]: what would be added, changed and removed, for the permission check.
//! 3. [`stage`]: the files are written to `<server>/.lodestone-restore-staging-<16>/`,
//!    every byte checked against its SHA-256, with the current name, port and command line
//!    pinned in the staged config.
//! 4. [`Staged::swap`]: the owned entries are renamed into
//!    `<server>/.lodestone-restore-old-<16>/`, the staged ones into place, and what the
//!    restore never touches back from the old entries into the new ones. A journal in the
//!    server's image folder records the swap so a crash in the middle is undone at the next
//!    start ([`recover`]).
//!
//! Everything here is blocking; the caller holds the server's locks, and holds the image
//! store's collections off while it reads the image's files.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use color_eyre::eyre::{eyre, Context};
use lodestone_image::store::excludes::Excludes;
use lodestone_image::store::materialize::placed_mode;
use lodestone_image::store::snapshot::{self, Candidate, SnapshotOptions};
use lodestone_image::store::{StatCache, Store as ImageStore, StoreError};
use lodestone_image::{CanonicalPath, DiskLayout, Entry, Image, ServerLayout};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tracing::{info, warn};

use crate::error::{Error, ErrorKind};
use crate::images::excludes::canonical_excludes;
use crate::images::layout::{server_flavour, server_layout};
use crate::images::scope::{is_content, is_server_settings_file};
use crate::images::InstanceContext;
use crate::types::{DotLodestoneConfig, InstanceUuid};

use super::store::{
    cancelled, err, join_rel, rename_with_retry, valid_rel_path, Cancel, Store,
};
use super::types::RestoreScope;
use crate::util::random_suffix;

pub const STAGING_PREFIX: &str = ".lodestone-restore-staging-";
pub const OLD_PREFIX: &str = ".lodestone-restore-old-";
/// `<instances>/.lodestone-restore-new-<16>/`: a restore as a new server, until it is
/// complete and renamed to its final folder.
pub const NEW_PREFIX: &str = ".lodestone-restore-new-";
pub const CONFIG_FILE: &str = ".lodestone_minecraft_config.json";
const PROPERTIES_FILE: &str = "server.properties";
/// Where staged copies go when the on-disk version of a path wins over them.
const REPLACED_DIR: &str = ".replaced";

/// How long to keep retrying a rename while Windows reports a file in use (Explorer, a
/// terminal in the world folder, antivirus).
#[cfg(windows)]
pub const SWAP_RENAME_BUDGET: Duration = Duration::from_secs(10);
#[cfg(not(windows))]
pub const SWAP_RENAME_BUDGET: Duration = Duration::ZERO;

fn bad_request(message: impl Into<String>) -> Error {
    err(ErrorKind::BadRequest, message)
}

fn io_error(e: io::Error, what: impl Into<String>) -> Error {
    Error::from(eyre!(e).wrap_err(what.into()))
}

/// Whether `path` is `prefix` or inside it (by whole parts: `world2` isn't in `world`).
fn under(path: &str, prefix: &str) -> bool {
    path == prefix
        || (path.len() > prefix.len()
            && path.starts_with(prefix)
            && path.as_bytes()[prefix.len()] == b'/')
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// The first part of a `/`-separated path.
fn top(path: &str) -> &str {
    path.split('/').next().unwrap_or(path)
}

// ---------------------------------------------------------------------------------------
// What a restore replaces

/// A backup to restore from: its data image, its level, and for "world and its mods" the
/// content image it links to.
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    pub image: &'a Image,
    /// [`super::types::BackupRecord::level`].
    pub level: Option<&'a str>,
    pub content: Option<&'a Image>,
}

/// Where an image's files go on a server's disk: its worlds as the image has them, the level
/// `level` (split the Paper way with `split`). Without a level, one no file of the image
/// uses, so every file keeps its path.
pub fn image_layout(image: &Image, level: Option<&str>, split: bool) -> Result<DiskLayout, Error> {
    let worlds: BTreeSet<String> = image.worlds.iter().map(|w| w.name.clone()).collect();
    let level = match level {
        Some(level) => level.to_string(),
        None => {
            let mut tops: BTreeSet<String> = worlds.clone();
            tops.extend(
                image
                    .entries
                    .iter()
                    .filter(|e| e.world.is_none())
                    .map(|e| e.path.first().to_string()),
            );
            ServerLayout::unused_level(&tops)
        }
    };
    let others: Vec<String> = worlds.into_iter().filter(|w| *w != level).collect();
    let layout = ServerLayout::new(level, others, split).map_err(|e| {
        err(
            ErrorKind::Internal,
            format!("The backup's worlds can't be placed on a server: {e}"),
        )
    })?;
    Ok(DiskLayout::Server(layout))
}

/// A file a restore places: where on the server's disk, and its entry in the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub disk: String,
    pub entry: Entry,
}

/// A file of the server now that the restore replaces or removes, as a backup now would
/// find it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub disk: String,
    pub path: CanonicalPath,
    pub size: u64,
    pub modified_ns: i64,
}

/// A top-level name the swap replaces as a whole, relative to the server folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedPath {
    pub path: String,
    /// There is a real (not linked) file or folder there now.
    pub on_disk: bool,
    /// The restore places something there.
    pub in_backup: bool,
}

#[derive(Debug, Clone)]
pub struct RestorePlan {
    pub scope: RestoreScope,
    pub owned: Vec<OwnedPath>,
    /// What the restore places, sorted by disk path.
    pub files: Vec<Target>,
    /// The server's files it replaces or removes, sorted by disk path.
    pub current: Vec<Current>,
}

impl RestorePlan {
    /// Whether the restore replaces or removes the server's file at the canonical `path`.
    pub fn replaces(&self, path: &str) -> bool {
        self.current.iter().any(|c| c.path.as_str() == path)
    }

    /// Whether `other` places and replaces exactly the same files, as planned from the same
    /// disk: then a permission check of one holds for the other.
    pub fn same_as(&self, other: &RestorePlan) -> bool {
        self.scope == other.scope
            && self.owned == other.owned
            && self.files == other.files
            && self.current == other.current
    }

    /// The bytes staging writes: what the free-space check needs.
    pub fn total_bytes(&self) -> u64 {
        self.files
            .iter()
            .map(|f| f.entry.size)
            .fold(0, u64::saturating_add)
    }
}

/// What restoring `source` with `scope` into the server `ctx` replaces (see the module).
///
/// Refused: a scope the backup can't do (no level, or no content image for "world and its
/// mods"), a file the server's layout can't place, and a top-level name the restore must
/// replace that is a link, or a file in the way of what the restore places.
pub fn plan(
    store: &ImageStore,
    ctx: &InstanceContext,
    source: &Source,
    scope: RestoreScope,
) -> Result<RestorePlan, Error> {
    let level = match scope {
        RestoreScope::Everything => None,
        RestoreScope::World | RestoreScope::WorldAndMods => Some(
            source
                .level
                .filter(|level| source.image.worlds.iter().any(|w| w.name == *level))
                .ok_or_else(|| bad_request("This backup has no world"))?,
        ),
    };
    if scope == RestoreScope::WorldAndMods && source.content.is_none() {
        return Err(bad_request(
            "This backup doesn't know the content it ran with, so its mods can't be restored",
        ));
    }
    let selected = |path: &CanonicalPath| match scope {
        RestoreScope::Everything => true,
        RestoreScope::World => path.world() == level,
        RestoreScope::WorldAndMods => {
            path.world() == level || (is_content(path, false) && !is_server_settings_file(path))
        }
    };

    // Left as they are: what the current policy excludes, and what the backup left out on
    // purpose (it has no copy of those, so they must not be removed either).
    let (current_layout, _) = server_layout(&ctx.root, &ctx.level_name, &ctx.flavour)?;
    let mut excludes = match &current_layout {
        DiskLayout::Server(server) => canonical_excludes(&ctx.policy_exclude, server).canonical,
        DiskLayout::Client => Vec::new(),
    };
    excludes.extend(source.image.excludes.iter().cloned());
    excludes.sort();
    excludes.dedup();
    let excluded = Excludes::new(&excludes);
    let split = server_flavour(&ctx.flavour).splits_worlds();
    let target_layout = image_layout(source.image, source.level, split)?;

    // What the restore places.
    let entries: Vec<&Entry> = source
        .image
        .entries
        .iter()
        .filter(|e| selected(&e.path))
        .collect();
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        if excluded.excludes(entry.path.as_str(), false) {
            continue;
        }
        let disk = target_layout.to_disk(&entry.path).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!(
                    "The backup's {} can't be placed on this server: {e}",
                    entry.path
                ),
            )
        })?;
        files.push(Target {
            disk,
            entry: entry.clone(),
        });
    }
    files.sort_by(|a, b| a.disk.cmp(&b.disk));
    files.dedup_by(|a, b| a.disk == b.disk);
    let has_config = files.iter().any(|f| f.disk == CONFIG_FILE);

    // What it replaces: the selected files a backup now would take, except those the backup
    // couldn't carry, and the server's settings when the backup has none (without them the
    // server can't load).
    let withheld: HashSet<&str> = source
        .image
        .withheld
        .iter()
        .map(|w| w.path.as_str())
        .collect();
    let filter = |candidate: &Candidate| candidate.is_dir || candidate.path.is_some_and(selected);
    let options = SnapshotOptions {
        excludes: &excludes,
        filter: Some(&filter),
        ..SnapshotOptions::default()
    };
    // Without a stat cache, planning only walks: it reads and claims nothing.
    let walked = snapshot::plan(store, &ctx.root, &current_layout, &options).map_err(|e| {
        err(
            ErrorKind::Internal,
            format!("Failed to read the server folder: {e}"),
        )
    })?;
    let mut current: Vec<Current> = walked
        .to_read
        .into_iter()
        .filter(|f| !withheld.contains(f.path.as_str()))
        .filter(|f| has_config || f.disk != CONFIG_FILE)
        .map(|f| Current {
            disk: f.disk,
            path: f.path,
            size: f.size,
            modified_ns: f.modified_ns,
        })
        .collect();
    current.sort_by(|a, b| a.disk.cmp(&b.disk));

    let owned = owned_tops(&ctx.root, &current, &files)?;
    Ok(RestorePlan {
        scope,
        owned,
        files,
        current,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiskKind {
    File,
    Dir,
    Link,
}

fn disk_kind(path: &Path) -> Option<DiskKind> {
    let meta = fs::symlink_metadata(path).ok()?;
    Some(if meta.file_type().is_symlink() {
        DiskKind::Link
    } else if meta.is_dir() {
        DiskKind::Dir
    } else {
        DiskKind::File
    })
}

/// The top-level names the swap replaces: those holding a file the restore replaces or
/// places.
fn owned_tops(
    server_root: &Path,
    current: &[Current],
    files: &[Target],
) -> Result<Vec<OwnedPath>, Error> {
    // name -> (has a current file, has a target)
    let mut tops: BTreeMap<&str, (bool, bool)> = BTreeMap::new();
    for file in current {
        tops.entry(top(&file.disk)).or_default().0 = true;
    }
    for file in files {
        tops.entry(top(&file.disk)).or_default().1 = true;
    }
    let mut owned = Vec::new();
    for (name, (_, placed)) in tops {
        let kind = disk_kind(&server_root.join(name));
        match kind {
            Some(DiskKind::Link) => {
                return Err(bad_request(format!(
                    "{name} is a link; restore can't replace it"
                )))
            }
            // A file a backup wouldn't take (one the backup couldn't carry, say) is kept,
            // so it can't make way for the backup's.
            Some(DiskKind::File) if !current.iter().any(|c| c.disk == name) => {
                return Err(bad_request(format!(
                    "{name} is in the way of the restore; move it or restore everything else first"
                )))
            }
            _ => {}
        }
        owned.push(OwnedPath {
            path: name.to_string(),
            on_disk: kind.is_some(),
            in_backup: placed,
        });
    }
    Ok(owned)
}

/// The fields of a server's settings file that describe its software and content, which a
/// "world and its mods" restore takes from the backup's copy ([`adopt_software`]).
const SOFTWARE_SETTINGS: [&str; 4] = ["version", "flavour", "jre_major_version", "modpack"];

/// Sets [`SOFTWARE_SETTINGS`] in the settings file of the server in `root` to what `backup`
/// (the backup's copy of that file) has, a field the backup lacks removed, leaving the rest
/// (name, port, memory, command line). Blocking.
pub fn adopt_software(root: &Path, backup: &serde_json::Value) -> Result<(), Error> {
    let path = root.join(CONFIG_FILE);
    let bytes = fs::read(&path).map_err(|e| io_error(e, format!("Failed to read {CONFIG_FILE}")))?;
    let mut settings: serde_json::Value =
        serde_json::from_slice(&bytes).context("The server's settings file is damaged")?;
    let (Some(settings_map), Some(backup)) = (settings.as_object_mut(), backup.as_object()) else {
        return Err(eyre!("A settings file isn't a JSON object").into());
    };
    for field in SOFTWARE_SETTINGS {
        match backup.get(field) {
            Some(value) => settings_map.insert(field.to_string(), value.clone()),
            None => settings_map.remove(field),
        };
    }
    let json = serde_json::to_vec_pretty(&settings).context("Failed to write the settings")?;
    crate::persist::write_atomic_blocking(&path, &json, None)
}

// ---------------------------------------------------------------------------------------
// The permission check

/// What a restore would change, by disk path. The handler checks permissions against it
/// (protected files, code files).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreDiff {
    /// Files the backup has and the server doesn't.
    pub added: Vec<String>,
    /// Files whose contents differ.
    pub changed: Vec<String>,
    /// Files the server has and the backup doesn't.
    pub removed: Vec<String>,
    /// Folders only the backup's files are in.
    pub created_dirs: Vec<String>,
    /// Folders only the replaced files are in.
    pub removed_dirs: Vec<String>,
}

impl RestoreDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.changed.is_empty()
            && self.removed.is_empty()
            && self.created_dirs.is_empty()
            && self.removed_dirs.is_empty()
    }
}

/// The folders `paths` are in.
fn folders_of<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for path in paths {
        for (i, c) in path.char_indices() {
            if c == '/' {
                dirs.insert(path[..i].to_string());
            }
        }
    }
    dirs
}

/// Compares the plan's files with the server's, `hashes` giving the SHA-256 of the
/// server's files by canonical path (from the safety backup's image, or [`hash_current`]);
/// a file it has no hash for counts as changed. `.lodestone_minecraft_config.json` is left
/// out: its sensitive fields are pinned instead.
pub fn diff(plan: &RestorePlan, hashes: &HashMap<String, String>) -> RestoreDiff {
    let placed: HashMap<&str, &Target> = plan
        .files
        .iter()
        .filter(|f| f.disk != CONFIG_FILE)
        .map(|f| (f.disk.as_str(), f))
        .collect();
    let now: HashMap<&str, &Current> = plan
        .current
        .iter()
        .filter(|c| c.disk != CONFIG_FILE)
        .map(|c| (c.disk.as_str(), c))
        .collect();
    let mut result = RestoreDiff::default();
    for (disk, target) in &placed {
        match now.get(disk) {
            None => result.added.push(disk.to_string()),
            Some(current) => {
                if hashes.get(current.path.as_str()) != Some(&target.entry.sha256) {
                    result.changed.push(disk.to_string());
                }
            }
        }
    }
    for disk in now.keys() {
        if !placed.contains_key(disk) {
            result.removed.push(disk.to_string());
        }
    }
    let placed_dirs = folders_of(plan.files.iter().map(|f| f.disk.as_str()));
    let current_dirs = folders_of(plan.current.iter().map(|c| c.disk.as_str()));
    result.created_dirs = placed_dirs.difference(&current_dirs).cloned().collect();
    result.removed_dirs = current_dirs.difference(&placed_dirs).cloned().collect();
    result.added.sort();
    result.changed.sort();
    result.removed.sort();
    result
}

/// The SHA-256 of each file the plan replaces, by canonical path: from `cache` (the backups'
/// stat cache) when it knows the file as it is, else read and hashed. A file that can't be
/// read is left out (and so counts as changed).
pub fn hash_current(
    server_root: &Path,
    plan: &RestorePlan,
    cache: &StatCache,
    cancel: Cancel,
) -> Result<HashMap<String, String>, Error> {
    let mut hashes = HashMap::new();
    for file in &plan.current {
        if cancel() {
            return Err(cancelled());
        }
        let hash = match cache.lookup(&file.disk, file.size, file.modified_ns) {
            Some(hash) => Some(hash.to_string()),
            None => sha256_of(&join_rel(server_root, &file.disk), cancel)?,
        };
        if let Some(hash) = hash {
            hashes.insert(file.path.to_string(), hash);
        }
    }
    Ok(hashes)
}

/// The SHA-256 of a file's bytes; `None` when it can't be read.
fn sha256_of(path: &Path, cancel: Cancel) -> Result<Option<String>, Error> {
    let Ok(mut file) = File::open(path) else {
        return Ok(None);
    };
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        if cancel() {
            return Err(cancelled());
        }
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Ok(None),
        }
    }
    Ok(Some(
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    ))
}

// ---------------------------------------------------------------------------------------
// Pinned settings

/// Settings a restore keeps from the current server (or sets, for a new one) instead of
/// taking the backup's. `None` leaves the backup's value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pin {
    pub name: Option<String>,
    pub port: Option<u32>,
    /// `Some(None)` clears it (the default Java).
    pub java_cmd: Option<Option<String>>,
    pub cmd_args: Option<Vec<String>>,
}

/// `.lodestone_minecraft_config.json` with the pinned fields set. Edited as JSON, so fields
/// this version doesn't know survive.
pub fn pin_config_json(text: &str, pin: &Pin) -> Result<String, Error> {
    let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        err(
            ErrorKind::Internal,
            format!("The backup's server settings can't be read: {e}"),
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        err(
            ErrorKind::Internal,
            "The backup's server settings can't be read",
        )
    })?;
    if let Some(name) = &pin.name {
        object.insert("name".to_string(), serde_json::json!(name));
    }
    if let Some(port) = pin.port {
        object.insert("port".to_string(), serde_json::json!(port));
    }
    if let Some(java_cmd) = &pin.java_cmd {
        object.insert("java_cmd".to_string(), serde_json::json!(java_cmd));
    }
    if let Some(cmd_args) = &pin.cmd_args {
        object.insert("cmd_args".to_string(), serde_json::json!(cmd_args));
    }
    serde_json::to_string_pretty(&value)
        .context("Failed to write the server settings")
        .map_err(Error::from)
}

/// `server.properties` with `server-port` set to `port`; every other line stays as it is.
pub fn pin_server_port(text: &[u8], port: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 20);
    let mut found = false;
    for line in text.split_inclusive(|b| *b == b'\n') {
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        let (content, ending): (&[u8], &[u8]) = match content.strip_suffix(b"\r") {
            Some(c) => (c, b"\r\n"),
            None if line.ends_with(b"\n") => (content, b"\n"),
            None => (content, b""),
        };
        let trimmed = content.trim_ascii_start();
        let is_comment = trimmed.starts_with(b"#") || trimmed.starts_with(b"!");
        let key_end = trimmed
            .iter()
            .position(|b| matches!(b, b'=' | b':' | b' ' | b'\t'))
            .unwrap_or(trimmed.len());
        if !is_comment && &trimmed[..key_end] == b"server-port" {
            out.extend_from_slice(format!("server-port={port}").as_bytes());
            out.extend_from_slice(if ending.is_empty() { b"\n" } else { ending });
            found = true;
        } else {
            out.extend_from_slice(line);
        }
    }
    if !found {
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(format!("server-port={port}\n").as_bytes());
    }
    out
}

/// Pins the settings in the config and `server.properties` under `dir` (staged files),
/// when they are there.
pub fn pin_settings(dir: &Path, pin: &Pin) -> Result<(), Error> {
    let config = dir.join(CONFIG_FILE);
    if config.is_file() {
        let text =
            fs::read_to_string(&config).context("Failed to read the backup's server settings")?;
        write_synced(&config, pin_config_json(&text, pin)?.as_bytes())
            .context("Failed to write the server settings")?;
    }
    if let Some(port) = pin.port {
        let properties = dir.join(PROPERTIES_FILE);
        if properties.is_file() {
            let text = fs::read(&properties).context("Failed to read server.properties")?;
            write_synced(&properties, &pin_server_port(&text, port))
                .context("Failed to write server.properties")?;
        }
    }
    Ok(())
}

/// Writes a staged file and flushes it to disk, like the files [`write_files`] writes.
fn write_synced(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    io::Write::write_all(&mut file, contents)?;
    file.sync_all()
}

// ---------------------------------------------------------------------------------------
// Staging

/// Writes `files` from the store under `dest`, each checked against its SHA-256 as it is
/// read, with its time and permission bits (plus the owner's write bit, so the game,
/// mods and Lodestone can change what a restore put back). Every file is created new, so
/// two names a case-insensitive disk takes for one fail rather than overwrite each other.
fn write_files(
    store: &ImageStore,
    files: &[Target],
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<(), Error> {
    let total: u64 = files
        .iter()
        .map(|f| f.entry.size)
        .fold(0, u64::saturating_add);
    let mut done = 0u64;
    for file in files {
        if cancel() {
            return Err(cancelled());
        }
        if !valid_rel_path(&file.disk) {
            return Err(err(
                ErrorKind::Internal,
                format!(
                    "The backup's {} has a path that can't be placed",
                    file.entry.path
                ),
            ));
        }
        let path = join_rel(dest, &file.disk);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                io_error(e, format!("Failed to create the folder of {}", file.disk))
            })?;
        }
        let out = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| io_error(e, format!("Failed to write {}", file.disk)))?;
        let mut writer = BufWriter::new(out);
        store
            .read_object(&file.entry.sha256, file.entry.size, &mut writer, cancel)
            .map_err(|e| match e {
                StoreError::Cancelled => cancelled(),
                StoreError::Missing(_) | StoreError::Damaged(_) | StoreError::WrongSize { .. } => {
                    err(
                        ErrorKind::Internal,
                        format!("The backup's copy of {} is missing or damaged", file.disk),
                    )
                }
                other => err(
                    ErrorKind::Internal,
                    format!("Failed to read the backup's copy of {}: {other}", file.disk),
                ),
            })?;
        let out = writer
            .into_inner()
            .map_err(|e| io_error(e.into_error(), format!("Failed to write {}", file.disk)))?;
        if let Some(time) = file.entry.modified.and_then(time_from_ns) {
            // Informational: a disk that can't keep the time still gets the file.
            let _ = out.set_modified(time);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            out.set_permissions(fs::Permissions::from_mode(placed_mode(file.entry.mode)))
                .map_err(|e| io_error(e, format!("Failed to write {}", file.disk)))?;
        }
        #[cfg(not(unix))]
        let _ = placed_mode;
        // on disk before the swap puts it in place and deletes the file it replaces. Only
        // on unix, where the folders are flushed too (`sync_tree`): on Windows a folder
        // can't be, so flushing every file (FlushFileBuffers) would make restores of big
        // worlds minutes slower and still not make the swap durable
        #[cfg(unix)]
        out.sync_all()
            .map_err(|e| io_error(e, format!("Failed to write {}", file.disk)))?;
        done = done.saturating_add(file.entry.size);
        progress(done, total);
    }
    Ok(())
}

fn time_from_ns(ns: i64) -> Option<std::time::SystemTime> {
    let magnitude = Duration::from_nanos(ns.unsigned_abs());
    if ns >= 0 {
        std::time::UNIX_EPOCH.checked_add(magnitude)
    } else {
        std::time::UNIX_EPOCH.checked_sub(magnitude)
    }
}

/// Flushes a folder's entries (unix; elsewhere the file system does it).
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Flushes `dir` and every folder in it, so files written into them can't vanish in a
/// power loss after the swap.
fn sync_tree(dir: &Path) -> Result<(), Error> {
    if cfg!(not(unix)) {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry =
            entry.map_err(|e| Error::from(eyre!(e).wrap_err("Failed to flush the restore")))?;
        if entry.file_type().is_dir() {
            sync_dir(entry.path()).map_err(|e| io_error(e, "Failed to flush the restore"))?;
        }
    }
    Ok(())
}

/// A backup's files staged inside the server folder, ready to [`swap`](Staged::swap) in.
#[derive(Debug)]
pub struct Staged {
    server_root: PathBuf,
    staging: String,
    old: String,
    /// The owned paths.
    owned: Vec<JournalEntry>,
    /// Paths inside the owned ones that the restore never touches (see the module), moved
    /// from the old entries into the new ones.
    carried: Vec<String>,
    rename_budget: Duration,
    #[cfg(test)]
    faults: Faults,
}

/// Simulated failures for the tests.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
struct Faults {
    /// Stop as if the process died after this many renames.
    crash_after: Option<usize>,
    /// Fail the rename after this many.
    fail_after: Option<usize>,
    /// Make the rollback fail too.
    break_rollback: bool,
    /// The journal can't be marked done or deleted.
    stuck_journal: bool,
}

/// Stages the plan's files and pins `pin` in them. Cancellable; on failure the staging
/// folder is deleted and nothing else has changed.
pub fn stage(
    store: &ImageStore,
    server_root: &Path,
    plan: &RestorePlan,
    pin: &Pin,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<Staged, Error> {
    let staging = random_suffix();
    let staging_dir = server_root.join(format!("{STAGING_PREFIX}{staging}"));
    fs::create_dir(&staging_dir)
        .map_err(|e| io_error(e, "Failed to create the restore's staging folder"))?;
    let result = (|| -> Result<Staged, Error> {
        write_files(store, &plan.files, &staging_dir, progress, cancel)?;
        pin_settings(&staging_dir, pin)?;
        sync_tree(&staging_dir)?;
        let replaced: HashSet<&str> = plan.current.iter().map(|c| c.disk.as_str()).collect();
        // a folder the restore places files in is gone through too, even if nothing in it is
        // replaced: kept whole, it would win over the staged one
        let touched_dirs = folders_of(
            replaced
                .iter()
                .copied()
                .chain(plan.files.iter().map(|f| f.disk.as_str())),
        );
        let mut owned = Vec::new();
        let mut carried = Vec::new();
        for path in &plan.owned {
            let on_disk = join_rel(server_root, &path.path);
            let kind = disk_kind(&on_disk);
            if kind == Some(DiskKind::Link) {
                return Err(bad_request(format!(
                    "{} is a link; restore can't replace it",
                    path.path
                )));
            }
            owned.push(JournalEntry {
                path: path.path.clone(),
                had_old: kind.is_some(),
                had_new: exists(&join_rel(&staging_dir, &path.path)),
            });
            if kind == Some(DiskKind::Dir) {
                carried.extend(kept_inside(
                    server_root,
                    &path.path,
                    &replaced,
                    &touched_dirs,
                )?);
            }
        }
        Ok(Staged {
            server_root: server_root.to_path_buf(),
            staging: staging.clone(),
            old: random_suffix(),
            owned,
            carried,
            rename_budget: SWAP_RENAME_BUDGET,
            #[cfg(test)]
            faults: Faults::default(),
        })
    })();
    if result.is_err() {
        if let Err(e) = fs::remove_dir_all(&staging_dir) {
            warn!("Failed to delete {}: {e}", staging_dir.display());
        }
    }
    result
}

/// The paths inside the owned folder `owned` that the restore keeps as they are: links,
/// files it doesn't replace (`replaced`), and folders holding no file it replaces or places
/// (`touched_dirs` are those that do). Where a kept path and a staged one meet, the kept
/// one wins ([`Staged::swap`]).
fn kept_inside(
    server_root: &Path,
    owned: &str,
    replaced: &HashSet<&str>,
    touched_dirs: &BTreeSet<String>,
) -> Result<Vec<String>, Error> {
    let mut kept = Vec::new();
    let mut entries = walkdir::WalkDir::new(join_rel(server_root, owned))
        .follow_links(false)
        .min_depth(1)
        .into_iter();
    while let Some(entry) = entries.next() {
        let entry =
            entry.map_err(|e| Error::from(eyre!(e).wrap_err(format!("Failed to read {owned}"))))?;
        let is_dir = entry.file_type().is_dir();
        let rel = match relative_to(server_root, entry.path()) {
            Some(rel) if valid_rel_path(&rel) => rel,
            _ => {
                return Err(bad_request(format!(
                    "{owned} has a file whose name Lodestone can't handle ({}); rename or remove it first",
                    entry.file_name().to_string_lossy()
                )))
            }
        };
        let keep = entry.path_is_symlink()
            || entry.file_type().is_symlink()
            || if is_dir {
                !touched_dirs.contains(&rel)
            } else {
                !replaced.contains(rel.as_str())
            };
        if keep {
            kept.push(rel);
            if is_dir {
                entries.skip_current_dir();
            }
        }
    }
    Ok(kept)
}

fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Why a swap failed.
#[derive(Debug)]
pub enum SwapError {
    /// Undone: the server folder is as it was.
    RolledBack(Error),
    /// Couldn't be undone. The journal stays for the next start, and the server must not
    /// start until then.
    Broken(Error),
}

enum MoveError {
    Failed(Error),
    #[cfg(test)]
    Crash,
}

impl Staged {
    fn staging_dir(&self) -> PathBuf {
        self.server_root
            .join(format!("{STAGING_PREFIX}{}", self.staging))
    }

    fn old_dir(&self) -> PathBuf {
        self.server_root.join(format!("{OLD_PREFIX}{}", self.old))
    }

    /// Deletes the staged files without changing anything else.
    pub fn discard(self) {
        let dir = self.staging_dir();
        if let Err(e) = fs::remove_dir_all(&dir) {
            warn!("Failed to delete {}: {e}", dir.display());
        }
    }

    /// Swaps the staged files in, with the journal at `journal_path`
    /// (`images/<uuid>/restore-journal.json`). Not cancellable: it is short, and
    /// stopping halfway would leave a half-restored server.
    pub fn swap(self, journal_path: &Path) -> Result<(), SwapError> {
        let mut journal = Journal {
            phase: Phase::MovingOld,
            staging: self.staging.clone(),
            old: self.old.clone(),
            entries: self
                .owned
                .iter()
                .cloned()
                .chain(self.carried.iter().map(|path| JournalEntry {
                    path: path.clone(),
                    had_old: true,
                    had_new: exists(&join_rel(&self.staging_dir(), path)),
                }))
                .collect(),
        };
        if let Err(e) = write_journal(journal_path, &journal) {
            self.discard();
            return Err(SwapError::RolledBack(e));
        }
        match self.move_all(&mut journal, journal_path) {
            Ok(()) => {
                self.sync_moved();
                match self.finish(journal_path, &journal) {
                    Ok(()) => {
                        self.remove_work_dirs();
                        Ok(())
                    }
                    // the journal still says the swap is under way, and the next start would
                    // undo it after the server ran: undo it now instead
                    Err(error) => self.undo(journal_path, &journal, error),
                }
            }
            #[cfg(test)]
            Err(MoveError::Crash) => Err(SwapError::Broken(err(ErrorKind::Internal, "crashed"))),
            Err(MoveError::Failed(error)) => self.undo(journal_path, &journal, error),
        }
    }

    /// Marks the swap done and deletes the journal. Fails only when the journal is left
    /// saying the swap is under way.
    fn finish(&self, journal_path: &Path, journal: &Journal) -> Result<(), Error> {
        let done = Journal {
            phase: Phase::Done,
            ..journal.clone()
        };
        let written = write_journal(journal_path, &done);
        #[cfg(test)]
        let written = if self.faults.stuck_journal {
            Err(err(ErrorKind::Internal, "failed on purpose"))
        } else {
            written
        };
        self.remove_journal(journal_path);
        match written {
            Err(error) if !journal_gone(journal_path) => Err(error),
            // a journal left saying "done" only makes the next start delete the work folders
            _ => Ok(()),
        }
    }

    /// Rolls back a swap that failed after the journal was written.
    fn undo(&self, journal_path: &Path, journal: &Journal, error: Error) -> Result<(), SwapError> {
        warn!("Restore failed, undoing it: {}", error.source);
        #[cfg(test)]
        let undone = if self.faults.break_rollback {
            Err(vec!["broken on purpose".to_string()])
        } else {
            rollback(&self.server_root, journal, self.rename_budget)
        };
        #[cfg(not(test))]
        let undone = rollback(&self.server_root, journal, self.rename_budget);
        match undone {
            Ok(()) => {
                // should the journal stay, the next start finds no old folder and leaves
                // everything as it is
                self.remove_journal(journal_path);
                self.remove_work_dirs();
                Err(SwapError::RolledBack(error))
            }
            Err(problems) => {
                warn!("The restore could not be undone: {}", problems.join("; "));
                Err(SwapError::Broken(err(
                    ErrorKind::Internal,
                    "The restore could not be undone; restart Lodestone to repair it",
                )))
            }
        }
    }

    fn remove_journal(&self, journal_path: &Path) {
        #[cfg(test)]
        if self.faults.stuck_journal {
            return;
        }
        remove_journal(journal_path);
    }

    /// Flushes the folders the swap renamed entries into, before the old entries are
    /// deleted. A failure only leaves the swap as durable as the file system makes it.
    fn sync_moved(&self) {
        let mut dirs = BTreeSet::new();
        for path in self.owned.iter().map(|e| &e.path).chain(&self.carried) {
            if let Some(parent) = join_rel(&self.server_root, path).parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
        for dir in dirs {
            if let Err(e) = sync_dir(&dir) {
                warn!("Failed to flush {}: {e}", dir.display());
            }
        }
    }

    fn move_all(&self, journal: &mut Journal, journal_path: &Path) -> Result<(), MoveError> {
        let staging_dir = self.staging_dir();
        let old_dir = self.old_dir();
        let mut renames = 0usize;
        let mut rename = |from: &Path, to: &Path, what: &str| -> Result<(), MoveError> {
            #[cfg(test)]
            {
                if self.faults.crash_after == Some(renames) {
                    return Err(MoveError::Crash);
                }
                if self.faults.fail_after == Some(renames) {
                    return Err(MoveError::Failed(err(
                        ErrorKind::Internal,
                        "failed on purpose",
                    )));
                }
            }
            renames += 1;
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    MoveError::Failed(io_error(e, format!("Failed to move {what}")))
                })?;
            }
            rename_with_retry(from, to, self.rename_budget)
                .map_err(|e| MoveError::Failed(io_error(e, format!("Failed to move {what}"))))
        };

        fs::create_dir(&old_dir)
            .map_err(|e| MoveError::Failed(io_error(e, "Failed to create the restore's folder")))?;
        for entry in self.owned.iter().filter(|e| e.had_old) {
            rename(
                &join_rel(&self.server_root, &entry.path),
                &join_rel(&old_dir, &entry.path),
                &entry.path,
            )?;
        }
        journal.phase = Phase::MovingNew;
        write_journal(journal_path, journal).map_err(MoveError::Failed)?;
        for entry in self.owned.iter().filter(|e| e.had_new) {
            rename(
                &join_rel(&staging_dir, &entry.path),
                &join_rel(&self.server_root, &entry.path),
                &entry.path,
            )?;
        }
        // what the restore never touches goes back; the on-disk version wins over a staged
        // copy of the same path
        let replaced = staging_dir.join(REPLACED_DIR);
        for path in &self.carried {
            let destination = join_rel(&self.server_root, path);
            if exists(&destination) {
                rename(&destination, &join_rel(&replaced, path), path)?;
            }
            rename(&join_rel(&old_dir, path), &destination, path)?;
        }
        #[cfg(test)]
        if self.faults.crash_after == Some(renames) {
            return Err(MoveError::Crash);
        }
        Ok(())
    }

    fn remove_work_dirs(&self) {
        for dir in [self.staging_dir(), self.old_dir()] {
            match fs::remove_dir_all(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => warn!("Failed to delete {}: {e}", dir.display()),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// The journal and recovery

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Moving the owned entries into the old folder.
    MovingOld,
    /// Moving the staged entries into place, then the kept paths back into them.
    MovingNew,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Relative to the server folder. An entry inside another entry is a kept path, moved
    /// from the old entry into the new one at the end of `moving_new`.
    pub path: String,
    pub had_old: bool,
    pub had_new: bool,
}

/// `images/<uuid>/restore-journal.json`. Holds only the folders' random suffixes and
/// relative paths, and is checked strictly before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub phase: Phase,
    /// The 16 hex digits of `.lodestone-restore-staging-<16>`.
    pub staging: String,
    /// The 16 hex digits of `.lodestone-restore-old-<16>`.
    pub old: String,
    pub entries: Vec<JournalEntry>,
}

fn valid_suffix(suffix: &str) -> bool {
    suffix.len() == 16
        && suffix
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn write_journal(path: &Path, journal: &Journal) -> Result<(), Error> {
    let json = serde_json::to_vec_pretty(journal).context("Failed to write the restore journal")?;
    crate::images::write_small_atomic(path, &json)
}

fn remove_journal(path: &Path) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != io::ErrorKind::NotFound {
            warn!(
                "Failed to delete the restore journal {}: {e}",
                path.display()
            );
        }
    }
}

/// Whether the journal is surely gone (not merely unreadable).
fn journal_gone(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(e) if e.kind() == io::ErrorKind::NotFound)
}

/// Whether no folder on the way to `rel` under `base` is a link or a file.
fn no_links_above(base: &Path, rel: &str) -> bool {
    let mut current = base.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return false,
            _ => {}
        }
    }
    true
}

/// Whether `path` is there (a link counts, not followed). `Ok(false)` only when it surely
/// isn't: an error other than "not found" (no permission, an I/O error, a Windows sharing
/// or delete-pending error) is returned, never taken for a missing path.
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Undoes an interrupted or failed swap. Never deletes anything at a destination: what is
/// in the way goes into the staging folder. Returns what couldn't be done.
fn rollback(server_root: &Path, journal: &Journal, budget: Duration) -> Result<(), Vec<String>> {
    rollback_with(server_root, journal, budget, &present)
}

/// [`rollback`], with `present` telling whether the old folder and each entry are there.
fn rollback_with(
    server_root: &Path,
    journal: &Journal,
    budget: Duration,
    present: &dyn Fn(&Path) -> io::Result<bool>,
) -> Result<(), Vec<String>> {
    if !valid_suffix(&journal.staging) || !valid_suffix(&journal.old) {
        return Err(vec!["the journal names invalid folders".to_string()]);
    }
    if let Some(bad) = journal.entries.iter().find(|e| !valid_rel_path(&e.path)) {
        return Err(vec![format!(
            "the journal has an invalid path {:?}",
            bad.path
        )]);
    }
    let staging_dir = server_root.join(format!("{STAGING_PREFIX}{}", journal.staging));
    let old_dir = server_root.join(format!("{OLD_PREFIX}{}", journal.old));
    for dir in [&staging_dir, &old_dir] {
        if exists(dir) && !is_real_dir(dir) {
            return Err(vec![format!("{} is not a folder", dir.display())]);
        }
    }
    // The old folder is made before anything moves and deleted only once the swap is
    // finished or undone, so without it there is nothing to undo: moving entries aside now
    // would throw away a finished restore, or the files a rollback already put back.
    // Only a folder that is surely gone counts as gone: one that can't be read may hold the
    // server's original entries.
    if journal.phase == Phase::Done {
        return Ok(());
    }
    match present(&old_dir) {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(e) => return Err(vec![format!("{} couldn't be read: {e}", old_dir.display())]),
    }
    let is_kept = |entry: &JournalEntry| {
        journal
            .entries
            .iter()
            .any(|other| other.path != entry.path && under(&entry.path, &other.path))
    };
    // whether the old entry a kept path came from is still in the old folder
    let owner_in_old = |entry: &JournalEntry| {
        journal.entries.iter().any(|other| {
            other.path != entry.path
                && under(&entry.path, &other.path)
                && !is_kept(other)
                && exists(&join_rel(&old_dir, &other.path))
        })
    };
    let (kept, owned): (Vec<&JournalEntry>, Vec<&JournalEntry>) =
        journal.entries.iter().partition(|e| is_kept(e));
    let mut problems = Vec::new();
    let mut aside = 0usize;
    let move_one = |from: &Path, to: &Path, rel: &str, problems: &mut Vec<String>| {
        let result = to
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|_| rename_with_retry(from, to, budget));
        if let Err(e) = result {
            problems.push(format!("{rel}: {e}"));
        }
    };
    let mut move_aside = |from: &Path, rel: &str, problems: &mut Vec<String>| {
        let mut to = join_rel(&staging_dir, rel);
        while exists(&to) {
            aside += 1;
            to = join_rel(&staging_dir.join(format!(".rollback-{aside}")), rel);
        }
        move_one(from, &to, rel, problems);
    };

    if journal.phase == Phase::MovingNew {
        // kept paths already moved into the new entries go back into the old ones first;
        // an old entry that is back in place already holds its kept paths (a repeated
        // rollback)
        for entry in kept.iter().rev() {
            let at_old = join_rel(&old_dir, &entry.path);
            let at_root = join_rel(server_root, &entry.path);
            let (old_there, root_there) = match (present(&at_old), present(&at_root)) {
                (Ok(old_there), Ok(root_there)) => (old_there, root_there),
                (Err(e), _) | (_, Err(e)) => {
                    problems.push(format!("{}: {e}", entry.path));
                    continue;
                }
            };
            if old_there || !root_there || !owner_in_old(entry) {
                continue;
            }
            if !no_links_above(server_root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
                problems.push(format!("{}: a folder on the way is a link", entry.path));
                continue;
            }
            move_one(&at_root, &at_old, &entry.path, &mut problems);
        }
    }
    for entry in owned.iter().rev() {
        let at_old = join_rel(&old_dir, &entry.path);
        let at_root = join_rel(server_root, &entry.path);
        if !no_links_above(server_root, &entry.path) || !no_links_above(&old_dir, &entry.path) {
            problems.push(format!("{}: a folder on the way is a link", entry.path));
            continue;
        }
        let (old_there, root_there) = match (present(&at_old), present(&at_root)) {
            (Ok(old_there), Ok(root_there)) => (old_there, root_there),
            (Err(e), _) | (_, Err(e)) => {
                problems.push(format!("{}: {e}", entry.path));
                continue;
            }
        };
        if old_there {
            if root_there {
                move_aside(&at_root, &entry.path, &mut problems);
            }
            move_one(&at_old, &at_root, &entry.path, &mut problems);
        } else if !entry.had_old && journal.phase == Phase::MovingNew && root_there {
            move_aside(&at_root, &entry.path, &mut problems);
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// What [`recover`] found.
#[derive(Debug, Default)]
pub struct Recovery {
    /// Servers whose interrupted restore was undone (or finished, if it was done).
    pub repaired: Vec<InstanceUuid>,
    /// Servers whose restore couldn't be undone; they must not start until it is.
    pub broken: Vec<InstanceUuid>,
}

/// At startup, before the servers are loaded: undoes interrupted restores, and deletes
/// leftovers (restore folders no journal names, half-made new servers, save-off markers).
/// The image store's own temporary files are swept by the image service.
pub fn recover(store: &Store, instances_dir: &Path) -> Recovery {
    recover_with(store, instances_dir, SWAP_RENAME_BUDGET)
}

fn recover_with(store: &Store, instances_dir: &Path, budget: Duration) -> Recovery {
    let mut recovery = Recovery::default();
    let (servers, all_read) = server_folders(instances_dir);
    // server folders whose journal couldn't be carried out: their restore folders may hold
    // the server's files, so none of them is deleted
    let mut protected: HashSet<PathBuf> = HashSet::new();
    // false when a journal may exist that couldn't be seen: then no leftover restore folder
    // is deleted, since it may hold a server's original files
    let mut journals_seen = true;

    let uuids = match store.instances() {
        Ok(uuids) => uuids,
        Err(e) => {
            warn!("Keeping every restore folder: the backups folder can't be read: {e}");
            journals_seen = false;
            Vec::new()
        }
    };
    for uuid in uuids {
        let Ok(journal_path) = store.journal_path(&uuid) else {
            continue;
        };
        match fs::symlink_metadata(&journal_path) {
            Ok(meta) if meta.is_file() => {}
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                warn!("The restore journal of {uuid} can't be checked, so the server stays locked: {e}");
                match servers.get(&uuid) {
                    Some(root) => {
                        protected.insert(root.clone());
                        recovery.broken.push(uuid);
                    }
                    None => journals_seen = false,
                }
                continue;
            }
        }
        let Some(root) = servers.get(&uuid) else {
            if all_read {
                warn!("Deleting the restore journal of {uuid}, whose server is gone");
                remove_journal(&journal_path);
            } else {
                warn!("Keeping the restore journal of {uuid}: its server wasn't found, but not every server folder could be read");
            }
            continue;
        };
        let journal: Journal = match fs::read(&journal_path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
        {
            Ok(journal) => journal,
            Err(e) => {
                warn!(
                    "The restore journal of {uuid} can't be read, so the server stays locked: {e}"
                );
                protected.insert(root.clone());
                recovery.broken.push(uuid);
                continue;
            }
        };
        match rollback(root, &journal, budget) {
            Ok(()) => {
                if journal.phase == Phase::Done {
                    info!("Finishing the restore of {uuid}");
                } else {
                    warn!("Undid the interrupted restore of {uuid}");
                }
                remove_journal(&journal_path);
                for dir in [
                    root.join(format!("{STAGING_PREFIX}{}", journal.staging)),
                    root.join(format!("{OLD_PREFIX}{}", journal.old)),
                ] {
                    if is_real_dir(&dir) {
                        if let Err(e) = fs::remove_dir_all(&dir) {
                            warn!("Failed to delete {}: {e}", dir.display());
                        }
                    }
                }
                recovery.repaired.push(uuid);
            }
            Err(problems) => {
                warn!(
                    "The interrupted restore of {uuid} couldn't be undone, so the server stays locked: {}",
                    problems.join("; ")
                );
                protected.insert(root.clone());
                recovery.broken.push(uuid);
            }
        }
    }

    // leftovers no journal names
    for root in servers.values() {
        if !journals_seen || protected.contains(root) {
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_restore_dir = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(".lodestone-restore"));
            if is_restore_dir && is_real_dir(&path) {
                warn!("Deleting the leftover restore folder {}", path.display());
                if let Err(e) = fs::remove_dir_all(&path) {
                    warn!("Failed to delete {}: {e}", path.display());
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir(instances_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_new = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(NEW_PREFIX));
            if is_new && is_real_dir(&path) {
                warn!("Deleting the unfinished restored server {}", path.display());
                if let Err(e) = fs::remove_dir_all(&path) {
                    warn!("Failed to delete {}: {e}", path.display());
                }
            }
        }
    }
    super::live::remove_leftover_markers(store);
    recovery
}

/// The server folders in `instances_dir`, by the uuid in their `.lodestone_config`, and
/// whether every folder could be read (otherwise a server that wasn't found may still be
/// there). A folder may be a link, as the core loads servers through links too; the paths
/// go through it.
fn server_folders(instances_dir: &Path) -> (HashMap<InstanceUuid, PathBuf>, bool) {
    let mut servers = HashMap::new();
    let entries = match fs::read_dir(instances_dir) {
        Ok(entries) => entries,
        Err(e) => return (servers, e.kind() == io::ErrorKind::NotFound),
    };
    let mut all_read = true;
    for entry in entries {
        let Ok(entry) = entry else {
            all_read = false;
            continue;
        };
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => continue,
            Err(_) => {
                all_read = false;
                continue;
            }
        }
        let bytes = match fs::read(path.join(".lodestone_config")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                all_read = false;
                continue;
            }
        };
        match serde_json::from_slice::<DotLodestoneConfig>(&bytes) {
            Ok(config) => {
                servers.insert(config.uuid().clone(), path);
            }
            Err(_) => all_read = false,
        }
    }
    (servers, all_read)
}

// ---------------------------------------------------------------------------------------
// Restore as a new server

/// A new `<instances>/.lodestone-restore-new-<16>` path for [`stage_all`].
pub fn new_server_dir(instances_dir: &Path) -> PathBuf {
    instances_dir.join(format!("{NEW_PREFIX}{}", random_suffix()))
}

/// Every file of `image`, where `layout` ([`image_layout`]) places it, sorted by disk path.
pub fn all_files(image: &Image, layout: &DiskLayout) -> Result<Vec<Target>, Error> {
    let mut files = Vec::with_capacity(image.entries.len());
    for entry in &image.entries {
        let disk = layout.to_disk(&entry.path).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!(
                    "The backup's {} can't be placed on a server: {e}",
                    entry.path
                ),
            )
        })?;
        files.push(Target {
            disk,
            entry: entry.clone(),
        });
    }
    files.sort_by(|a, b| a.disk.cmp(&b.disk));
    Ok(files)
}

/// Writes `files` into `dest`, a new folder. On failure `dest` is deleted. The caller then
/// pins the settings ([`pin_settings`]), writes `.lodestone_config` last and renames the
/// folder.
pub fn stage_all(
    store: &ImageStore,
    files: &[Target],
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancel: Cancel,
) -> Result<(), Error> {
    fs::create_dir(dest).map_err(|e| io_error(e, "Failed to create the new server's folder"))?;
    let result = write_files(store, files, dest, progress, cancel).and_then(|()| sync_tree(dest));
    if result.is_err() {
        if let Err(e) = fs::remove_dir_all(dest) {
            warn!("Failed to delete {}: {e}", dest.display());
        }
    }
    result
}

#[cfg(test)]
mod tests;
