//! The Modrinth App (modrinth/code, `packages/app-lib`).
//!
//! - Data folder `~/Library/Application Support/ModrinthApp` (`src/state/dirs.rs`,
//!   `apps/app/tauri.conf.json` identifier). Its `app.db` is SQLite in WAL mode
//!   (`src/state/db.rs`), read with [`super::sqlite`]. Instances are folders in
//!   `<settings.custom_dir, or the data folder>/profiles/<path>/`, which are the game
//!   folders themselves.
//! - Up to 0.14: the `profiles` table (`migrations/20240711194701_init.sql` and later
//!   `ALTER`s): `path`, `name`, `icon_path`, `game_version`, `mod_loader`
//!   (`vanilla|forge|fabric|quilt|neoforge`), `mod_loader_version`, `install_stage`,
//!   `override_java_path`, `override_extra_launch_args` (JSONB array),
//!   `override_mc_memory_max` (MB).
//! - From 0.15 (`20260619120000_finish-instance-profile-migration.sql` drops `profiles`):
//!   `instances(id, path, applied_content_set_id, name, icon_path, install_stage, …)`,
//!   `instance_content_sets(id, instance_id, game_version, loader, loader_version, …)` and
//!   `instance_launch_overrides(instance_id, overrides JSONB)` with `java_path`,
//!   `extra_launch_args`, `memory.maximum` (`20260611120000_instances-content-foundation.sql`,
//!   `src/state/instances/model/launch.rs`).
//! - Before SQLite (≤ 0.7, `~/Library/Application Support/com.modrinth.theseus`):
//!   `profiles/<path>/profile.json` with `metadata.{name, icon, game_version, loader,
//!   loader_version.id}`, `memory.maximum`, `java.extra_arguments`
//!   (`src/state/legacy_converter.rs`).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::Value as Json;

use super::{
    Found, IconSource, Launcher, Source, WantedLoader, WantedVersion, file_inside, folder_name,
    read_json,
    sqlite::{Database, Row, Value},
    subfolders,
};
use crate::game::loaders::LoaderKind;

pub const DATABASE: &str = "app.db";

/// Reads the Modrinth App's data folder `root` (current or pre-SQLite).
pub fn read(root: &Path) -> Launcher {
    let mut launcher = Launcher::new(Source::ModrinthApp, root);
    if root.join(DATABASE).is_file() {
        match read_database(root) {
            Ok(found) => launcher.instances = found,
            Err(err) => launcher
                .problems
                .push(format!("{}: {err}", root.join(DATABASE).display())),
        }
        return launcher;
    }
    // The JSON format: `settings.json` may move the folder with the profiles.
    let config_dir = read_json(&root.join("settings.json"), 4 * 1024 * 1024)
        .ok()
        .and_then(|s| {
            s.get("loaded_config_dir")
                .and_then(Json::as_str)
                .map(PathBuf::from)
        })
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| root.to_path_buf());
    for dir in subfolders(&config_dir.join("profiles")) {
        if dir.join("profile.json").is_file() {
            match read_profile_json(&dir, &[root, &config_dir]) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

fn text(row: &Row, column: &str) -> Option<String> {
    row.get(column)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn read_database(root: &Path) -> Result<Vec<Found>, String> {
    let db = Database::open(&root.join(DATABASE)).map_err(|err| err.to_string())?;
    let rows = |table: &str| db.rows(table).map_err(|err| err.to_string());
    let config_dir = rows("settings")?
        .and_then(|rows| rows.into_iter().next())
        .and_then(|row| text(&row, "custom_dir"))
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(|| root.to_path_buf());
    let profiles = config_dir.join("profiles");
    let icon_roots = [root, config_dir.as_path()];
    let mut found = Vec::new();
    if let Some(rows) = rows("profiles")? {
        for row in rows {
            let Some(path) = text(&row, "path") else {
                continue;
            };
            let overrides = Overrides {
                java_path: text(&row, "override_java_path"),
                extra_args: row
                    .get("override_extra_launch_args")
                    .and_then(Value::as_json),
                memory_max: row.get("override_mc_memory_max").and_then(Value::as_i64),
            };
            if let Some(f) = to_found(
                &profiles,
                &path,
                &row,
                &row,
                "mod_loader",
                "mod_loader_version",
                &overrides,
                &icon_roots,
            ) {
                found.push(f);
            }
        }
        return Ok(found);
    }
    let Some(instances) = rows("instances")? else {
        return Err("it has neither a profiles nor an instances table".into());
    };
    let sets: HashMap<String, Row> = rows("instance_content_sets")?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| Some((text(&row, "id")?, row)))
        .collect();
    let launch: HashMap<String, Json> = rows("instance_launch_overrides")?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| Some((text(&row, "instance_id")?, row.get("overrides")?.as_json()?)))
        .collect();
    for row in instances {
        let (Some(id), Some(path)) = (text(&row, "id"), text(&row, "path")) else {
            continue;
        };
        let Some(set) = text(&row, "applied_content_set_id").and_then(|s| sets.get(&s)) else {
            continue;
        };
        let json = launch.get(&id);
        let overrides = Overrides {
            java_path: json
                .and_then(|j| j.get("java_path"))
                .and_then(Json::as_str)
                .map(str::to_string),
            extra_args: json.and_then(|j| j.get("extra_launch_args")).cloned(),
            memory_max: json
                .and_then(|j| j.pointer("/memory/maximum"))
                .and_then(Json::as_i64),
        };
        if let Some(f) = to_found(
            &profiles,
            &path,
            &row,
            set,
            "loader",
            "loader_version",
            &overrides,
            &icon_roots,
        ) {
            found.push(f);
        }
    }
    Ok(found)
}

struct Overrides {
    java_path: Option<String>,
    extra_args: Option<Json>,
    memory_max: Option<i64>,
}

#[allow(clippy::too_many_arguments)]
fn to_found(
    profiles: &Path,
    path: &str,
    row: &Row,
    set: &Row,
    loader_column: &str,
    loader_version_column: &str,
    overrides: &Overrides,
    icon_roots: &[&Path],
) -> Option<Found> {
    // `path` is one folder name in `profiles/`.
    if path.contains(['/', '\\']) || path.starts_with('.') {
        return None;
    }
    let game_dir = profiles.join(path);
    let game_version = text(set, "game_version")?;
    let name = text(row, "name").unwrap_or_else(|| path.to_string());
    let mut found = Found::new(
        Source::ModrinthApp,
        &name,
        game_dir,
        WantedVersion::Exact(game_version),
    );
    found.loader = loader(
        text(set, loader_column).as_deref(),
        text(set, loader_version_column).as_deref(),
    );
    apply(&mut found, overrides, text(row, "icon_path"), icon_roots);
    if text(row, "install_stage").is_some_and(|s| s != "installed") {
        found.note("The Modrinth App hadn't finished installing this instance.");
    }
    Some(found)
}

fn apply(found: &mut Found, overrides: &Overrides, icon: Option<String>, icon_roots: &[&Path]) {
    if let Some(max) = overrides.memory_max {
        found.set_memory(None, u64::try_from(max).ok());
    }
    if let Some(Json::Array(args)) = &overrides.extra_args {
        let args: Vec<&str> = args.iter().filter_map(Json::as_str).collect();
        found.set_jvm_args(Some(&args.join(" ")));
    }
    if overrides.java_path.is_some() {
        found.note(
            "The instance's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    if let Some(icon) = icon {
        let path = PathBuf::from(icon);
        found.icon = icon_roots
            .iter()
            .find_map(|root| file_inside(root, &root.join(&path)))
            .map(IconSource::File);
    }
}

fn loader(name: Option<&str>, version: Option<&str>) -> Option<WantedLoader> {
    let kind = match name?.to_ascii_lowercase().as_str() {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        _ => return None,
    };
    Some(WantedLoader::new(kind, version))
}

/// Reads a pre-SQLite `profile.json` instance folder.
pub fn read_profile_json(dir: &Path, icon_roots: &[&Path]) -> Result<Found, String> {
    let json = read_json(&dir.join("profile.json"), 16 * 1024 * 1024)?;
    let meta = json.get("metadata").ok_or("profile.json has no metadata")?;
    let get = |value: &Json, pointer: &str| {
        value
            .pointer(pointer)
            .and_then(Json::as_str)
            .map(str::to_string)
    };
    let game_version = get(meta, "/game_version").ok_or("profile.json has no game version")?;
    let name = get(meta, "/name").unwrap_or_else(|| folder_name(dir));
    let mut found = Found::new(
        Source::ModrinthApp,
        &name,
        dir.to_path_buf(),
        WantedVersion::Exact(game_version),
    );
    found.loader = loader(
        get(meta, "/loader").as_deref(),
        get(meta, "/loader_version/id").as_deref(),
    );
    let overrides = Overrides {
        java_path: get(&json, "/java/override_version/path"),
        extra_args: json.pointer("/java/extra_arguments").cloned(),
        memory_max: json.pointer("/memory/maximum").and_then(Json::as_i64),
    };
    let mut roots: Vec<&Path> = icon_roots.to_vec();
    roots.push(dir);
    apply(&mut found, &overrides, get(meta, "/icon"), &roots);
    Ok(found)
}
