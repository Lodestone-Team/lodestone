//! The CurseForge app's Minecraft instances (not pack downloads).
//!
//! Instances are folders in `~/Documents/curseforge/minecraft/Instances` (also
//! `~/curseforge/minecraft/Instances`; Modrinth App `packages/app-lib/src/api/pack/import/mod.rs`
//! checks both) with a `minecraftinstance.json`; the game runs in the folder itself. The
//! app is closed source; the fields are from the importers that read them: Modrinth App
//! `import/curseforge.rs`, packwiz `curseforge/packinterop/minecraftinstance.go`, XMCL
//! `packages/instance/parsers/curseforge_parser.ts`. `name`, `gameVersion`,
//! `baseModLoader.{name ("forge-47.4.0", "fabric-0.16.14-1.20.1", "neoforge-…"),
//! forgeVersion, minecraftVersion, type (CurseForge API ModLoaderType: 1 Forge, 4 Fabric,
//! 5 Quilt, 6 NeoForge)}`, `allocatedMemory` (MB) with `isMemoryOverride`,
//! `javaArgsOverride`, `profileImagePath`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{
    Found, IconSource, Launcher, Source, WantedLoader, WantedVersion, folder_name, read_json,
    subfolders,
};
use crate::game::loaders::LoaderKind;

pub const INSTANCE_FILE: &str = "minecraftinstance.json";

/// Reads the instances in `instances_dir` (CurseForge's `minecraft/Instances`).
pub fn read(instances_dir: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::CurseForge, instances_dir);
    for dir in subfolders(instances_dir) {
        if dir.join(INSTANCE_FILE).is_file() {
            match read_instance(&dir) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

pub fn read_instance(dir: &Path) -> Result<Found, String> {
    let json = read_json(&dir.join(INSTANCE_FILE), 64 * 1024 * 1024)?;
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let base = json.get("baseModLoader").filter(|b| b.is_object());
    let game_version = text(&json, "gameVersion")
        .or_else(|| base.and_then(|b| text(b, "minecraftVersion")))
        .ok_or_else(|| format!("{}: no Minecraft version", dir.display()))?;
    let name = text(&json, "name").unwrap_or_else(|| folder_name(dir));
    let mut found = Found::new(
        Source::CurseForge,
        &name,
        dir.to_path_buf(),
        WantedVersion::Exact(game_version.clone()),
    );
    if let Some(base) = base {
        let loader_name = text(base, "name").unwrap_or_default();
        let lower = loader_name.to_ascii_lowercase();
        let kind = match base.get("type").and_then(Value::as_u64) {
            Some(1) => Some(LoaderKind::Forge),
            Some(4) => Some(LoaderKind::Fabric),
            Some(5) => Some(LoaderKind::Quilt),
            Some(6) => Some(LoaderKind::NeoForge),
            _ if lower.starts_with("neoforge") => Some(LoaderKind::NeoForge),
            _ if lower.starts_with("forge") => Some(LoaderKind::Forge),
            _ if lower.starts_with("fabric") => Some(LoaderKind::Fabric),
            _ if lower.starts_with("quilt") => Some(LoaderKind::Quilt),
            _ => None,
        };
        match kind {
            Some(mut kind) => {
                let version = if loader_name.is_empty() {
                    text(base, "forgeVersion")
                } else {
                    Some(loader_name.clone())
                };
                if kind == LoaderKind::NeoForge && game_version == "1.20.1" {
                    kind = LoaderKind::Forge;
                    found.note(
                        "NeoForge for Minecraft 1.20.1 runs as Forge 47.1 in Lodestone Launcher.",
                    );
                }
                found.loader = Some(WantedLoader::new(kind, version.as_deref()));
            }
            None if !loader_name.is_empty() => found.note(format!(
                "{loader_name} isn't a loader Lodestone Launcher installs; the instance is vanilla."
            )),
            None => {}
        }
    }
    if json.get("isMemoryOverride").and_then(Value::as_bool) == Some(true) {
        found.set_memory(None, json.get("allocatedMemory").and_then(Value::as_u64));
    }
    found.set_jvm_args(text(&json, "javaArgsOverride").as_deref());
    if let Some(image) = text(&json, "profileImagePath").map(PathBuf::from) {
        let image = if image.is_absolute() {
            image
        } else {
            dir.join(image)
        };
        if image.is_file() {
            found.icon = Some(IconSource::File(image));
        }
    }
    Ok(found)
}
