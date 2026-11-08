//! ATLauncher (ATLauncher/ATLauncher).
//!
//! The data folder is the app's working directory, `ATLauncher.app/Contents/Java` on
//! macOS (`build.gradle` sets `user.dir`; `FileSystem.java`) and by default
//! `%APPDATA%\ATLauncher` on Windows (`packaging/windows-setup/installer.iss`), with
//! `instances/<folder>/instance.json` read by Gson into `data/Instance.java`: `id` is the
//! Minecraft version; `launcher` (`data/InstanceLauncher.java`) has `name`,
//! `loaderVersion` (`data/minecraft/loaders/LoaderVersion.java`: `type` Fabric, Forge,
//! LegacyFabric, NeoForge, Quilt, Paper, Purpur; `version`; `rawVersion`, Forge's
//! `<mc>-<build>`, `loaders/forge/ForgeLoader.java`), `maximumMemory` (MB),
//! `javaArguments` and `javaPath`. The game runs in the instance folder itself
//! (`mclauncher/MCLauncher.java`), and the icon is its `instance.png`.

use std::path::Path;

use serde_json::Value;

use super::{
    Found, IconSource, Launcher, Source, WantedLoader, WantedVersion, folder_name, read_json,
    subfolders,
};
use crate::game::loaders::LoaderKind;

pub const INSTANCE_FILE: &str = "instance.json";

pub fn read(root: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::AtLauncher, root);
    for dir in subfolders(&root.join("instances")) {
        if dir.join(INSTANCE_FILE).is_file() {
            match read_instance(&dir) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

/// Whether `dir` looks like an ATLauncher instance (its `instance.json` has `launcher`).
pub fn is_instance(dir: &Path) -> bool {
    read_json(&dir.join(INSTANCE_FILE), 16 * 1024 * 1024)
        .is_ok_and(|json| json.get("launcher").is_some_and(Value::is_object))
}

pub fn read_instance(dir: &Path) -> Result<Found, String> {
    let json = read_json(&dir.join(INSTANCE_FILE), 16 * 1024 * 1024)?;
    let launcher = json
        .get("launcher")
        .ok_or_else(|| format!("{}: not an ATLauncher instance", dir.display()))?;
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let game_version =
        text(&json, "id").ok_or_else(|| format!("{}: no Minecraft version", dir.display()))?;
    let name = text(launcher, "name").unwrap_or_else(|| folder_name(dir));
    let mut found = Found::new(
        Source::AtLauncher,
        &name,
        dir.to_path_buf(),
        WantedVersion::Exact(game_version),
    );
    if let Some(loader) = launcher.get("loaderVersion").filter(|l| l.is_object()) {
        let kind = match text(loader, "type")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "fabric" => Some(LoaderKind::Fabric),
            "quilt" => Some(LoaderKind::Quilt),
            "forge" => Some(LoaderKind::Forge),
            "neoforge" => Some(LoaderKind::NeoForge),
            other => {
                found.note(format!(
                    "ATLauncher's {other} loader isn't one Lodestone Launcher installs; the instance is vanilla."
                ));
                None
            }
        };
        if let Some(kind) = kind {
            let version = text(loader, "rawVersion").or_else(|| text(loader, "version"));
            found.loader = Some(WantedLoader::new(kind, version.as_deref()));
        }
    }
    found.set_memory(None, launcher.get("maximumMemory").and_then(Value::as_u64));
    found.set_jvm_args(text(launcher, "javaArguments").as_deref());
    if text(launcher, "javaPath").is_some() {
        found.note(
            "The instance's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    let icon = dir.join("instance.png");
    if let Some(path) = super::file_inside(dir, &icon) {
        found.icon = Some(IconSource::File(path));
    }
    Ok(found)
}
