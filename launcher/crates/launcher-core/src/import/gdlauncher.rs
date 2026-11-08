//! GDLauncher: the old Electron app and the newer Carbon.
//!
//! - **Legacy** (gorilla-devs/GDLauncher, v1.1.30): data in
//!   `~/Library/Application Support/gdlauncher_next`, moved to the path written in its
//!   `override.data` (`public/electron.js`). `instances/<name>/config.json` (the name is
//!   the folder's) with `loader.{loaderType vanilla|forge|fabric, loaderVersion (Forge's
//!   full `1.16.5-36.2.39`), mcVersion}`, `javaArgs`, `javaMemory` (MB), `customJavaPath`,
//!   `background` (an image in the folder) (`src/common/reducers/actions.js`,
//!   `src/common/utils/index.js`). The game runs in the instance folder.
//! - **Carbon** (gorilla-devs/GDLauncher-Carbon): data in
//!   `~/Library/Application Support/gdlauncher_carbon`, runtime folder `data/` unless
//!   `runtime_path_override` names another; `instances/<folder>/instance.json` (`_version`
//!   `"1"`, `name`, `icon`, `game_configuration.{version.{release, modloaders[{type
//!   Forge|Neoforge|Fabric|Quilt, version}]}, memory.{min_mb, max_mb}, extra_java_args,
//!   java_override}`) with the game in `instance/`
//!   (`crates/carbon_app/src/managers/instance/schema/v1.rs`).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{
    Found, IconSource, Launcher, Source, WantedLoader, WantedVersion, file_inside, folder_name,
    read_json, read_text, subfolders,
};
use crate::game::loaders::LoaderKind;

/// The folder the legacy app keeps its data in: `root`, or where `override.data` says.
fn legacy_data(root: &Path) -> PathBuf {
    read_text(&root.join("override.data"), 64 * 1024)
        .ok()
        .map(|text| PathBuf::from(text.trim()))
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| root.to_path_buf())
}

pub fn read_legacy(root: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::GdLauncher, root);
    for dir in subfolders(&legacy_data(root).join("instances")) {
        if dir.join("config.json").is_file() {
            match read_legacy_instance(&dir) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn kind(name: &str) -> Option<LoaderKind> {
    match name.to_ascii_lowercase().as_str() {
        "fabric" => Some(LoaderKind::Fabric),
        "quilt" => Some(LoaderKind::Quilt),
        "forge" => Some(LoaderKind::Forge),
        "neoforge" => Some(LoaderKind::NeoForge),
        _ => None,
    }
}

/// Whether `dir` is a legacy GDLauncher instance: a `config.json` with `loader.mcVersion`.
pub fn is_legacy_instance(dir: &Path) -> bool {
    read_json(&dir.join("config.json"), 16 * 1024 * 1024)
        .is_ok_and(|json| json.pointer("/loader/mcVersion").is_some())
}

pub fn read_legacy_instance(dir: &Path) -> Result<Found, String> {
    let json = read_json(&dir.join("config.json"), 16 * 1024 * 1024)?;
    let loader = json
        .get("loader")
        .ok_or_else(|| format!("{}: not a GDLauncher instance", dir.display()))?;
    let game_version = text(loader, "mcVersion")
        .ok_or_else(|| format!("{}: no Minecraft version", dir.display()))?;
    let mut found = Found::new(
        Source::GdLauncher,
        &folder_name(dir),
        dir.to_path_buf(),
        WantedVersion::Exact(game_version),
    );
    if let Some(name) = text(loader, "loaderType") {
        match kind(&name) {
            Some(kind) => {
                found.loader = Some(WantedLoader::new(
                    kind,
                    text(loader, "loaderVersion").as_deref(),
                ))
            }
            None if name != "vanilla" => found.note(format!(
                "{name} isn't a loader Lodestone Launcher installs; the instance is vanilla."
            )),
            None => {}
        }
    }
    found.set_memory(None, json.get("javaMemory").and_then(Value::as_u64));
    found.set_jvm_args(text(&json, "javaArgs").as_deref());
    if text(&json, "customJavaPath").is_some() {
        found.note(
            "The instance's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    if let Some(background) = text(&json, "background") {
        found.icon = file_inside(dir, &dir.join(background)).map(IconSource::File);
    }
    Ok(found)
}

/// The Carbon runtime folder: `root/data`, or where `runtime_path_override` says.
fn carbon_runtime(root: &Path) -> PathBuf {
    read_text(&root.join("runtime_path_override"), 64 * 1024)
        .ok()
        .map(|text| PathBuf::from(text.trim()))
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| root.join("data"))
}

pub fn read_carbon(root: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::GdLauncherCarbon, root);
    for dir in subfolders(&carbon_runtime(root).join("instances")) {
        if dir.join("instance.json").is_file() && dir.join("instance").is_dir() {
            match read_carbon_instance(&dir) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

/// Whether `dir` is a Carbon instance: `instance.json` with `game_configuration`.
pub fn is_carbon_instance(dir: &Path) -> bool {
    read_json(&dir.join("instance.json"), 16 * 1024 * 1024)
        .is_ok_and(|json| json.get("game_configuration").is_some())
}

pub fn read_carbon_instance(dir: &Path) -> Result<Found, String> {
    let json = read_json(&dir.join("instance.json"), 16 * 1024 * 1024)?;
    let config = json
        .get("game_configuration")
        .ok_or_else(|| format!("{}: not a GDLauncher instance", dir.display()))?;
    let mut version = config.get("version").cloned().unwrap_or(Value::Null);
    // An externally tagged enum, as serde writes one: {"Standard": {...}}.
    if let Some(inner) = version.get("Standard").cloned() {
        version = inner;
    }
    let mut notes = Vec::new();
    let (game_version, loader) = match &version {
        Value::Object(_) => {
            let release = text(&version, "release")
                .ok_or_else(|| format!("{}: no Minecraft version", dir.display()))?;
            let loader = version
                .get("modloaders")
                .and_then(Value::as_array)
                .and_then(|list| list.first())
                .and_then(|l| {
                    let name = text(l, "type")?;
                    match kind(&name) {
                        Some(kind) => Some(WantedLoader::new(kind, text(l, "version").as_deref())),
                        None => {
                            notes.push(format!("{name} isn't a loader Lodestone Launcher installs; the instance is vanilla."));
                            None
                        }
                    }
                });
            (release, loader)
        }
        Value::String(custom) => {
            let parsed = super::mapping::parse_version_id(custom, None);
            notes.extend(parsed.notes);
            match parsed.game_version {
                WantedVersion::Exact(v) => (v, parsed.loader),
                _ => return Err(format!("{}: no Minecraft version", dir.display())),
            }
        }
        _ => return Err(format!("{}: no Minecraft version", dir.display())),
    };
    let name = text(&json, "name").unwrap_or_else(|| folder_name(dir));
    let mut found = Found::new(
        Source::GdLauncherCarbon,
        &name,
        dir.join("instance"),
        WantedVersion::Exact(game_version),
    );
    found.loader = loader;
    found.notes = notes;
    let mb = |key: &str| {
        config
            .pointer(&format!("/memory/{key}"))
            .and_then(Value::as_u64)
    };
    found.set_memory(mb("min_mb"), mb("max_mb"));
    found.set_jvm_args(text(config, "extra_java_args").as_deref());
    if config.get("java_override").is_some_and(|j| !j.is_null()) {
        found.note(
            "The instance's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    let icon = text(&json, "icon")
        .map(|icon| dir.join(icon))
        .into_iter()
        .chain(["icon.png", "icon.webp", "icon.jpg"].map(|n| dir.join(n)))
        .find_map(|path| file_inside(dir, &path));
    found.icon = icon.map(IconSource::File);
    Ok(found)
}
