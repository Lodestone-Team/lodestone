//! Importing an existing Minecraft server: a folder on the core's machine becomes a new
//! instance.
//!
//! `POST /instance/import/inspect` reads a folder and says what it found (the Minecraft
//! version, the server software, the port, the size). `POST /instance/import` then makes
//! the instance the way `POST /instance/create/{game_type}` does, with Lodestone's own
//! Java and server software for the chosen version, and copies the folder's files into
//! it. The folder itself is never changed.
//!
//! Both need `CreateInstance` and `ReadGlobalFile`: they read any folder the core can.

use std::path::{Path as FsPath, PathBuf};

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::Event;
use crate::implementations::minecraft::properties::PropertiesDocument;
use crate::implementations::minecraft::util::get_server_jar_url;
use crate::implementations::minecraft::{
    eula, validate_new_instance_name, FabricLoaderVersion, Flavour, FlavourKind, ForgeBuildVersion,
    NeoForgeBuildVersion, RestoreConfig, SetupConfig,
};
use crate::modpack::write_port;
use crate::prelude::path_to_instances;
use crate::types::InstanceUuid;
use crate::AppState;

use super::global_fs::canonicalize_global_read;
use super::instance::{spawn_minecraft_instance_creation, PostSetupStep};
use super::util::{is_script_path, AuthedUser};

/// Lodestone's own files in an instance folder. They belong to the instance they were
/// written for, so they are never copied.
const LODESTONE_FILE_PREFIX: &str = ".lodestone";

/// Folders at the top of a server folder that the server software makes again: its
/// libraries, the unpacked game, and caches of patched jars.
const SOFTWARE_DIRS: [&str; 4] = ["libraries", "versions", ".fabric", "cache"];

/// The share of the creation's progress that copying the files takes.
const COPY_PROGRESS: f64 = 10.0;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct InspectServerFolderRequest {
    /// A folder on the core's machine.
    pub path: String,
}

/// What `POST /instance/import/inspect` found in a folder.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ServerFolderInfo {
    /// The folder, with symbolic links resolved.
    pub path: String,
    /// A name for the new server: the old one's when Lodestone made it, else the folder's.
    pub name: String,
    /// The Minecraft version the server last ran, when it could be told.
    pub version: Option<String>,
    /// The server software Lodestone would run it with, when it could be told.
    pub flavour: Option<FlavourKind>,
    /// The version of the mod loader (Fabric, Forge, NeoForge) the server has.
    pub loader_version: Option<String>,
    /// The server's own software when Lodestone doesn't offer it ("Purpur", "Spigot",
    /// "Quilt"). `flavour` is then the closest one Lodestone has, if any.
    pub other_software: Option<String>,
    /// `server-port` from `server.properties`.
    pub port: Option<u32>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    /// Whether `eula.txt` accepts the Minecraft EULA. It is copied as it is.
    pub eula_accepted: bool,
    /// Whether the folder has a world (`level.dat` in the `level-name` folder).
    pub has_world: bool,
    /// How much would be copied.
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub file_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ImportServerRequest {
    pub path: String,
    pub name: String,
    pub version: String,
    pub flavour: FlavourKind,
    /// The mod loader version to install. Left out, or when that version can't be
    /// downloaded any more, the newest for `version` is used.
    #[serde(default)]
    #[ts(optional)]
    pub loader_version: Option<String>,
    pub port: u32,
    #[serde(default)]
    #[ts(optional)]
    pub min_ram: Option<u32>,
    #[serde(default)]
    #[ts(optional)]
    pub max_ram: Option<u32>,
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// The folder to import from: an existing folder that is not Lodestone's own data and
/// not one of this core's servers.
async fn source_folder(path: &str) -> Result<PathBuf, Error> {
    if path.trim().is_empty() {
        return Err(bad_request("Choose the server's folder"));
    }
    // a folder holding Lodestone's data would also copy the new server into itself
    let source = canonicalize_global_read(FsPath::new(path), true).await?;
    if !tokio::fs::metadata(&source)
        .await
        .is_ok_and(|meta| meta.is_dir())
    {
        return Err(bad_request(format!("{} is not a folder", source.display())));
    }
    let instances = tokio::fs::canonicalize(path_to_instances())
        .await
        .unwrap_or_else(|_| path_to_instances().clone());
    if source.starts_with(&instances) || instances.starts_with(&source) {
        return Err(bad_request(
            "This folder is already part of Lodestone's servers",
        ));
    }
    Ok(source)
}

/// What the server software in a folder is.
#[derive(Debug, Default, PartialEq)]
struct Software {
    version: Option<String>,
    flavour: Option<FlavourKind>,
    loader_version: Option<String>,
    other_software: Option<String>,
}

/// The names of the folders in `dir`, newest version first.
fn version_dirs(dir: &FsPath) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort_by(|a, b| lodestone_minecraft::versions::compare_versions(b, a));
    names
}

/// A Minecraft version looks like `1.20.1`, `26.1` or a snapshot such as `24w14a`: short,
/// starting with a digit, and safe to put in a URL.
fn looks_like_version(text: &str) -> bool {
    (1..=32).contains(&text.len())
        && text.starts_with(|c: char| c.is_ascii_digit())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// The text after `marker` in `text`, up to the first character a version can't have.
fn version_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = &text[text.find(marker)? + marker.len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .unwrap_or(rest.len());
    let version = rest[..end].trim_end_matches('.');
    looks_like_version(version).then_some(version)
}

/// The version the server said it was starting, in the start of `logs/latest.log`.
fn version_from_log(root: &FsPath) -> Option<String> {
    use std::io::Read;
    let mut start = Vec::new();
    std::fs::File::open(root.join("logs").join("latest.log"))
        .ok()?
        .take(64 * 1024)
        .read_to_end(&mut start)
        .ok()?;
    let start = String::from_utf8_lossy(&start);
    version_after(&start, "Starting minecraft server version ").map(str::to_owned)
}

/// The version Paper last ran: `"currentVersion": "git-Paper-196 (MC: 1.20.1)"`.
fn version_from_paper_history(root: &FsPath) -> Option<String> {
    let text = std::fs::read_to_string(root.join("version_history.json")).ok()?;
    let history: serde_json::Value = serde_json::from_str(&text).ok()?;
    version_after(history.get("currentVersion")?.as_str()?, "(MC: ").map(str::to_owned)
}

/// The `id` in the `version.json` of a server jar at the top of the folder (vanilla
/// since 1.14, and Paper's jars).
fn version_from_jars(root: &FsPath) -> Option<String> {
    use std::io::Read;
    let jars = std::fs::read_dir(root).ok()?.flatten().filter(|entry| {
        entry.path().extension().is_some_and(|ext| ext == "jar")
            && entry.file_type().is_ok_and(|kind| kind.is_file())
    });
    for jar in jars {
        let Ok(file) = std::fs::File::open(jar.path()) else {
            continue;
        };
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            continue;
        };
        let Ok(entry) = archive.by_name("version.json") else {
            continue;
        };
        let mut text = String::new();
        if entry.take(64 * 1024).read_to_string(&mut text).is_err() {
            continue;
        }
        let id = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|json| json.get("id")?.as_str().map(str::to_owned));
        if let Some(id) = id.filter(|id| looks_like_version(id)) {
            return Some(id);
        }
    }
    None
}

/// The Minecraft version of a server that isn't Forge or NeoForge, from the most to the
/// least telling place.
fn game_version(root: &FsPath) -> Option<String> {
    version_from_log(root)
        .or_else(|| version_from_paper_history(root))
        .or_else(|| version_from_jars(root))
        .or_else(|| {
            version_dirs(&root.join("versions"))
                .into_iter()
                .find(|name| looks_like_version(name))
        })
}

/// Whether `dir` holds at least one `.jar` file.
fn has_jars(dir: &FsPath) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "jar"))
}

/// Works out what a server folder runs from the files its software leaves behind.
fn detect_software(root: &FsPath) -> Software {
    let libraries = root.join("libraries");
    let exists = |relative: &str| root.join(relative).exists();

    // NeoForge and Forge name their version, and with it the game's, in their library
    if let Some(build) = version_dirs(&libraries.join("net/neoforged/neoforge"))
        .into_iter()
        .next()
    {
        return Software {
            version: lodestone_minecraft::versions::neoforge::neoforge_game_version(&build)
                .or_else(|| game_version(root)),
            flavour: Some(FlavourKind::NeoForge),
            loader_version: Some(build),
            other_software: None,
        };
    }
    // `1.20.1-47.2.0`: the game version, then Forge's
    if let Some(build) = version_dirs(&libraries.join("net/minecraftforge/forge"))
        .into_iter()
        .next()
    {
        let version = build
            .split_once('-')
            .map(|(game, _)| game.to_owned())
            .filter(|game| looks_like_version(game));
        return Software {
            version: version.or_else(|| game_version(root)),
            flavour: Some(FlavourKind::Forge),
            loader_version: Some(build),
            other_software: None,
        };
    }
    let version = game_version(root);
    if libraries.join("org/quiltmc/quilt-loader").exists() || exists(".quilt") {
        return Software {
            version,
            flavour: None,
            loader_version: None,
            other_software: Some("Quilt".to_owned()),
        };
    }
    let fabric_loader = version_dirs(&libraries.join("net/fabricmc/fabric-loader"))
        .into_iter()
        .next();
    if fabric_loader.is_some() || exists(".fabric") || exists("fabric-server-launcher.properties") {
        return Software {
            version,
            flavour: Some(FlavourKind::Fabric),
            loader_version: fabric_loader,
            other_software: None,
        };
    }
    // Paper and the servers its plugins also run on, the most particular first
    let paper_like = [
        ("purpur.yml", Some("Purpur")),
        ("pufferfish.yml", Some("Pufferfish")),
        ("config/paper-global.yml", None),
        ("paper.yml", None),
        ("version_history.json", None),
        ("spigot.yml", Some("Spigot")),
        ("bukkit.yml", Some("CraftBukkit")),
    ];
    if let Some((_, other)) = paper_like.iter().find(|(file, _)| exists(file)) {
        return Software {
            version,
            flavour: Some(FlavourKind::Paper),
            loader_version: None,
            other_software: other.map(str::to_owned),
        };
    }
    // mods with no loader to be found: not vanilla, and not something to guess
    let flavour = (!has_jars(&root.join("mods"))).then_some(FlavourKind::Vanilla);
    Software {
        version,
        flavour,
        loader_version: None,
        other_software: None,
    }
}

/// Whether the entry at `relative` (inside the server folder) is left out of the copy
/// whoever imports: Lodestone's own files, the server software, which is installed
/// anew, and the lock of a running world.
fn is_left_out(relative: &FsPath, is_dir: bool) -> bool {
    let mut parts = relative.components();
    let (Some(first), rest) = (parts.next(), parts.next()) else {
        return false;
    };
    let top_level = rest.is_none();
    let name = |part: std::path::Component| part.as_os_str().to_string_lossy().to_lowercase();
    if top_level {
        let first = name(first);
        if first.starts_with(LODESTONE_FILE_PREFIX) {
            return true;
        }
        if is_dir {
            return SOFTWARE_DIRS.contains(&first.as_str());
        }
        if first.ends_with(".jar") {
            return true;
        }
    }
    !is_dir
        && relative
            .file_name()
            .is_some_and(|file| file.eq_ignore_ascii_case("session.lock"))
}

/// A file to copy, and where it goes inside the new folder.
struct CopyEntry {
    relative: PathBuf,
    size: u64,
}

/// What a copy of a server folder holds.
#[derive(Default)]
struct CopyPlan {
    dirs: Vec<PathBuf>,
    files: Vec<CopyEntry>,
    size_bytes: u64,
    /// Script files left out because the importer may not add scripts.
    skipped_scripts: u64,
    /// Symbolic links, which are never followed.
    skipped_links: u64,
}

/// Lists what to copy from `root`. Symbolic links are not followed, so nothing outside
/// the folder is read.
fn plan_copy(root: &FsPath, allow_scripts: bool) -> Result<CopyPlan, Error> {
    let mut plan = CopyPlan::default();
    let mut walk = walkdir::WalkDir::new(root).min_depth(1).into_iter();
    while let Some(entry) = walk.next() {
        let entry = entry.context("Failed to read the server's folder")?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .context("Failed to read the server's folder")?
            .to_path_buf();
        let kind = entry.file_type();
        if kind.is_symlink() {
            plan.skipped_links += 1;
            continue;
        }
        if is_left_out(&relative, kind.is_dir()) {
            if kind.is_dir() {
                walk.skip_current_dir();
            }
            continue;
        }
        if kind.is_dir() {
            plan.dirs.push(relative);
        } else if kind.is_file() {
            if !allow_scripts && is_script_path(&relative) {
                plan.skipped_scripts += 1;
                continue;
            }
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            plan.size_bytes += size;
            plan.files.push(CopyEntry { relative, size });
        }
    }
    Ok(plan)
}

/// Copies what `plan` lists from `source` into `target`, replacing files already there.
/// `progress` gets the bytes copied by each file.
fn copy_plan(
    plan: &CopyPlan,
    source: &FsPath,
    target: &FsPath,
    progress: &dyn Fn(u64),
) -> Result<(), Error> {
    for dir in &plan.dirs {
        std::fs::create_dir_all(target.join(dir))
            .with_context(|| format!("Failed to create the folder {}", dir.display()))?;
    }
    for file in &plan.files {
        let to = target.join(&file.relative);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create the folder {}", parent.display()))?;
        }
        std::fs::copy(source.join(&file.relative), &to)
            .with_context(|| format!("Failed to copy {}", file.relative.display()))?;
        progress(file.size);
    }
    Ok(())
}

/// Reads a server folder: its software, settings and size.
fn inspect_folder(root: &FsPath, allow_scripts: bool) -> Result<ServerFolderInfo, Error> {
    let plan = plan_copy(root, allow_scripts)?;
    // a server Lodestone made says exactly what it runs
    let lodestone: Option<RestoreConfig> =
        std::fs::read(root.join(".lodestone_minecraft_config.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let software = match &lodestone {
        Some(config) => Software {
            version: Some(config.version.clone()),
            flavour: Some(FlavourKind::from(&config.flavour)),
            loader_version: config.flavour.loader_version(),
            other_software: None,
        },
        None => detect_software(root),
    };
    let properties = std::fs::read(root.join("server.properties")).ok();
    let document = properties
        .as_ref()
        .map(|bytes| PropertiesDocument::from_bytes(bytes).0);
    let property = |key: &str| document.as_ref().and_then(|doc| doc.get(key));
    let level_name = property("level-name")
        .filter(|name| !name.is_empty())
        .unwrap_or("world");
    // `level-name` is a folder name; anything else isn't looked for outside the folder
    let has_world = FsPath::new(level_name).components().count() == 1
        && root.join(level_name).join("level.dat").is_file();
    let has_server_jar = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "jar"));
    if properties.is_none() && !has_world && !has_server_jar && lodestone.is_none() {
        return Err(bad_request(
            "This folder doesn't look like a Minecraft server: it has no server.properties, world or server jar",
        ));
    }
    let folder_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(ServerFolderInfo {
        path: root.to_string_lossy().into_owned(),
        name: lodestone
            .as_ref()
            .map(|config| config.name.clone())
            .unwrap_or(folder_name),
        version: software.version,
        flavour: software.flavour,
        loader_version: software.loader_version,
        other_software: software.other_software,
        port: property("server-port")
            .and_then(|port| port.trim().parse().ok())
            .filter(|port| (1..=65535).contains(port)),
        min_ram: lodestone.as_ref().map(|config| config.min_ram),
        max_ram: lodestone.as_ref().map(|config| config.max_ram),
        eula_accepted: std::fs::read_to_string(root.join(eula::EULA_FILE))
            .is_ok_and(|text| eula::eula_text_accepts(&text)),
        has_world,
        size_bytes: plan.size_bytes,
        file_count: plan.files.len() as u64,
    })
}

/// Importing reads a folder anywhere on the core's machine and makes a server of it.
async fn require_import(state: &AppState, requester: &AuthedUser) -> Result<(), Error> {
    requester.require(state, UserAction::CreateInstance).await?;
    requester.try_action(&UserAction::ReadGlobalFile, state.safe_mode())
}

/// Scripts are only copied for the owner, while code execution is on (the rule for
/// creating script files, see `util::check_code_write`).
fn may_import_scripts(state: &AppState, requester: &AuthedUser) -> bool {
    requester.is_owner && state.code_execution_enabled()
}

async fn inspect_server_folder(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<InspectServerFolderRequest>,
) -> Result<Json<ServerFolderInfo>, Error> {
    require_import(&state, &requester).await?;
    let source = source_folder(&request.path).await?;
    let allow_scripts = may_import_scripts(&state, &requester);
    let info = tokio::task::spawn_blocking(move || inspect_folder(&source, allow_scripts))
        .await
        .context("Failed to read the server's folder")??;
    Ok(Json(info))
}

/// `kind` with the loader version to install, when one is asked for.
fn flavour_with_loader(kind: FlavourKind, loader_version: Option<&str>) -> Flavour {
    let loader = loader_version.map(str::to_owned);
    match kind {
        FlavourKind::Fabric => Flavour::Fabric {
            loader_version: loader.map(FabricLoaderVersion::new),
            installer_version: None,
        },
        FlavourKind::Forge => Flavour::Forge {
            build_version: loader.map(ForgeBuildVersion::new),
        },
        FlavourKind::NeoForge => Flavour::NeoForge {
            build_version: loader.map(NeoForgeBuildVersion::new),
        },
        other => other.into(),
    }
}

fn check_request(request: &ImportServerRequest) -> Result<(), Error> {
    validate_new_instance_name(&request.name)?;
    if !looks_like_version(&request.version) {
        return Err(bad_request("Choose the server's Minecraft version"));
    }
    if !(1..=65535).contains(&request.port) {
        return Err(bad_request("The port must be between 1 and 65535"));
    }
    if request.min_ram == Some(0) || request.max_ram == Some(0) {
        return Err(bad_request("RAM must be at least 1 MB"));
    }
    if let (Some(min), Some(max)) = (request.min_ram, request.max_ram) {
        if min > max {
            return Err(bad_request("Minimum RAM cannot be larger than maximum RAM"));
        }
    }
    Ok(())
}

/// The software to install: the server's own loader version when it can still be
/// downloaded, else the newest for the Minecraft version. Fails before anything is
/// copied when Lodestone has no such server at all.
async fn resolve_flavour(request: &ImportServerRequest) -> Result<Flavour, Error> {
    let newest: Flavour = request.flavour.into();
    if let Some(loader) = request.loader_version.as_deref().filter(|v| !v.is_empty()) {
        let asked = flavour_with_loader(request.flavour, Some(loader));
        if asked != newest && get_server_jar_url(&request.version, &asked).await.is_ok() {
            return Ok(asked);
        }
    }
    get_server_jar_url(&request.version, &newest)
        .await
        .map_err(|e| {
            bad_request(format!(
                "Lodestone can't set up a {} server for Minecraft {}: {}",
                newest.to_string(),
                request.version,
                e.source
            ))
        })?;
    Ok(newest)
}

/// What the end of the import says about files that were not copied.
fn describe_skipped(plan: &CopyPlan) -> Option<String> {
    let mut parts = Vec::new();
    if plan.skipped_scripts > 0 {
        parts.push(format!(
            "{} script file(s) were not copied: only the owner can import scripts, while code execution is on",
            plan.skipped_scripts
        ));
    }
    if plan.skipped_links > 0 {
        parts.push(format!(
            "{} symbolic link(s) were not copied",
            plan.skipped_links
        ));
    }
    (!parts.is_empty()).then(|| parts.join(". "))
}

async fn import_server(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<ImportServerRequest>,
) -> Result<Json<InstanceUuid>, Error> {
    require_import(&state, &requester).await?;
    check_request(&request)?;
    let source = source_folder(&request.path).await?;
    let allow_scripts = may_import_scripts(&state, &requester);
    // read it first: a folder that isn't a server fails here, before anything is made
    {
        let source = source.clone();
        tokio::task::spawn_blocking(move || inspect_folder(&source, allow_scripts))
            .await
            .context("Failed to read the server's folder")??;
    }
    let flavour = resolve_flavour(&request).await?;
    let port = request.port;
    let setup_config = SetupConfig {
        name: request.name.clone(),
        version: request.version.clone(),
        flavour,
        port,
        cmd_args: Vec::new(),
        description: None,
        min_ram: request.min_ram,
        max_ram: request.max_ram,
        auto_start: None,
        restart_on_crash: None,
        backup_period: None,
        modpack: None,
        // the server's own eula.txt is copied over this
        accept_eula: false,
    };
    let title = format!("Importing Minecraft server {}", request.name);
    let event_broadcaster = state.event_broadcaster.clone();
    let post_setup: PostSetupStep = Box::new(move |root, event_id| {
        Box::pin(async move {
            let message = tokio::task::spawn_blocking({
                let root = root.clone();
                move || {
                    // listed again: the folder may have changed since it was read
                    let plan = plan_copy(&source, allow_scripts)?;
                    let total = plan.size_bytes.max(1) as f64;
                    event_broadcaster.send(Event::new_progression_event_update(
                        &event_id,
                        format!("Copying {} files", plan.files.len()),
                        0.0,
                    ));
                    // one update for each 1% copied, not one for each file
                    let pending = std::cell::Cell::new(0u64);
                    let step = (plan.size_bytes / 100).max(1);
                    copy_plan(&plan, &source, &root, &|bytes| {
                        pending.set(pending.get() + bytes);
                        if pending.get() >= step {
                            event_broadcaster.send(Event::new_progression_event_update(
                                &event_id,
                                "Copying the server's files",
                                pending.get() as f64 / total * COPY_PROGRESS,
                            ));
                            pending.set(0);
                        }
                    })?;
                    Ok::<_, Error>(describe_skipped(&plan))
                }
            })
            .await
            .context("Failed to copy the server's files")??;
            // the copied server.properties has the old server's port
            write_port(&root, port).await?;
            Ok(message)
        })
    });
    let uuid = spawn_minecraft_instance_creation(
        state,
        &requester,
        setup_config,
        title,
        10.0 + COPY_PROGRESS,
        Some(post_setup),
    )
    .await?;
    Ok(Json(uuid))
}

pub fn get_instance_import_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/import/inspect", post(inspect_server_folder))
        .route("/instance/import", post(import_server))
        .with_state(state)
}

#[cfg(test)]
mod tests;
