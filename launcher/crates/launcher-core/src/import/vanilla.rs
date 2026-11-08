//! The official Minecraft launcher: `launcher_profiles.json` in its folder
//! (`~/Library/Application Support/minecraft`).
//!
//! Each profile has `name`, `type` (`custom`, `latest-release`, `latest-snapshot`),
//! `lastVersionId`, and optionally `gameDir` (else the launcher's folder), `javaArgs`,
//! `javaDir` and `icon` (a built-in name, or a `data:image/png;base64,` URL as the Fabric
//! and Quilt installers write). See minecraft.wiki's "launcher_profiles.json", and
//! FabricMC/fabric-installer `src/main/java/net/fabricmc/installer/util/Utils.java`,
//! QuiltMC/quilt-installer `LauncherProfiles.java`. A modded `lastVersionId` is read by
//! [`super::mapping::parse_version_id`] with its version JSON from `versions/<id>/`.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::Value;

use super::{
    Found, IconSource, Launcher, Source, WantedVersion, mapping, read_json, split_memory_args,
};
use crate::game::libraries::safe_relative_path;

pub const PROFILES_FILE: &str = "launcher_profiles.json";

/// Reads the profiles in `root`, the official launcher's folder.
pub fn read(root: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::Vanilla, root);
    let profiles = match read_json(&root.join(PROFILES_FILE), 16 * 1024 * 1024) {
        Ok(value) => value,
        Err(err) => {
            launcher.problems.push(err);
            return launcher;
        }
    };
    let Some(profiles) = profiles.get("profiles").and_then(Value::as_object) else {
        launcher
            .problems
            .push(format!("{PROFILES_FILE} has no profiles"));
        return launcher;
    };
    let mut found: Vec<(String, Found)> = profiles
        .iter()
        .filter_map(|(key, profile)| {
            Some((last_used(profile), profile_to_found(root, key, profile)?))
        })
        .collect();
    // Most recently used first, as the launcher lists them.
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.name.cmp(&b.1.name)));
    launcher.instances = found.into_iter().map(|(_, f)| f).collect();
    launcher
}

fn last_used(profile: &Value) -> String {
    profile
        .get("lastUsed")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn profile_to_found(root: &Path, key: &str, profile: &Value) -> Option<Found> {
    let text = |name: &str| {
        profile
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let kind = text("type").unwrap_or("custom");
    let id = text("lastVersionId").unwrap_or(match kind {
        "latest-snapshot" => "latest-snapshot",
        _ => "latest-release",
    });
    let game_dir = match text("gameDir") {
        Some(dir) if Path::new(dir).is_absolute() => PathBuf::from(dir),
        Some(dir) => root.join(dir),
        None => root.to_path_buf(),
    };
    let (version, loader, notes) = match id {
        "latest-release" => (WantedVersion::LatestRelease, None, Vec::new()),
        "latest-snapshot" => (WantedVersion::LatestSnapshot, None, Vec::new()),
        id => {
            let json = safe_relative_path(id)
                .ok()
                .filter(|p| p.components().count() == 1)
                .and_then(|_| {
                    read_json(
                        &root.join("versions").join(id).join(format!("{id}.json")),
                        8 * 1024 * 1024,
                    )
                    .ok()
                });
            let parsed = mapping::parse_version_id(id, json.as_ref());
            (parsed.game_version, parsed.loader, parsed.notes)
        }
    };
    let name = match text("name") {
        Some(name) => name.to_string(),
        None => match &version {
            WantedVersion::LatestRelease => "Latest release".into(),
            WantedVersion::LatestSnapshot => "Latest snapshot".into(),
            WantedVersion::Exact(v) => v.clone(),
        },
    };
    let mut found = Found::new(Source::Vanilla, &name, game_dir, version);
    found.key = format!("Vanilla:{}:{key}", root.display());
    found.loader = loader;
    found.notes = notes;
    if let Some(args) = text("javaArgs") {
        let (min, max, rest) = split_memory_args(args);
        found.set_memory(min, max);
        found.set_jvm_args(Some(&rest));
    }
    if text("javaDir").is_some() {
        found.note(
            "The profile's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    if let Some(data) = text("icon").and_then(|icon| icon.strip_prefix("data:image/"))
        && let Some((_, encoded)) = data.split_once(";base64,")
        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded.trim())
    {
        found.icon = Some(IconSource::Bytes(bytes));
    }
    if found.game_dir == root {
        found.note(
            "This profile uses the launcher's own folder, which other profiles share; its worlds, mods and options are copied.",
        );
    }
    Some(found)
}
