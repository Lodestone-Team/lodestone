//! A server as images see it: its folder, game, flavour and disk layout, read once per
//! record.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lodestone_image::settings::{ServerFlavour, ServerSettings};
use lodestone_image::{DiskLayout, Game, Loader, ServerLayout, Settings};

use crate::error::Error;
use crate::implementations::minecraft::{Flavour, MinecraftInstance};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::{State, TServer};
use crate::types::InstanceUuid;

/// The level `level-name` names when `server.properties` has none.
pub const DEFAULT_LEVEL_NAME: &str = "world";

/// What a record needs to know about a server, read from it once.
#[derive(Debug, Clone)]
pub struct InstanceContext {
    pub uuid: InstanceUuid,
    /// The server folder.
    pub root: PathBuf,
    pub game_version: String,
    pub flavour: Flavour,
    /// `level-name`, or [`DEFAULT_LEVEL_NAME`].
    pub level_name: String,
    /// The backup policy's exclude patterns (server disk patterns).
    pub policy_exclude: Vec<String>,
    /// Whether the server is running (or starting or stopping): its files may change while
    /// they are read, so a record reads them on one thread.
    pub running: bool,
}

impl InstanceContext {
    /// The context of `instance` now, with its backup policy's `policy_exclude`.
    pub async fn of(instance: &MinecraftInstance, policy_exclude: Vec<String>) -> InstanceContext {
        let (game_version, flavour) = instance.server_software().await;
        let level_name = instance
            .server_properties()
            .await
            .ok()
            .and_then(|p| p.get("level-name").map(str::to_string))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| DEFAULT_LEVEL_NAME.to_string());
        InstanceContext {
            uuid: instance.uuid().await,
            root: instance.path().await,
            game_version,
            flavour,
            level_name,
            policy_exclude,
            running: !matches!(instance.state().await, State::Stopped | State::Error),
        }
    }
}

/// The server flavour as an image's settings name it.
pub fn server_flavour(flavour: &Flavour) -> ServerFlavour {
    ServerFlavour::parse(&flavour.to_string())
}

/// The server folder's disk layout, read from what is in it ([`ServerLayout::discover`]):
/// the level, every other top-level folder with a `level.dat`, split the Paper way for the
/// Bukkit family. Returns it with whether the level is a world images can name. Blocking.
pub fn server_layout(
    root: &Path,
    level_name: &str,
    flavour: &Flavour,
) -> Result<(DiskLayout, bool), Error> {
    let entries = std::fs::read_dir(root).map_err(|e| {
        color_eyre::eyre::eyre!("Failed to list the server folder {}: {e}", root.display())
    })?;
    let mut tops = BTreeSet::new();
    let mut with_level_dat = BTreeSet::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if entry.file_type().is_ok_and(|t| t.is_dir()) && entry.path().join("level.dat").is_file() {
            with_level_dat.insert(name.clone());
        }
        tops.insert(name);
    }
    let (layout, level_is_world) = ServerLayout::discover(
        Some(level_name),
        &tops,
        &with_level_dat,
        server_flavour(flavour).splits_worlds(),
    )
    .map_err(|e| color_eyre::eyre::eyre!("The server's worlds can't be mapped: {e}"))?;
    Ok((DiskLayout::Server(layout), level_is_world))
}

/// The game players run with this server: a Paper or Spigot server's players run vanilla
/// (the flavour is a server setting); a modded server's run its loader, at its version.
pub fn game_of(version: &str, flavour: &Flavour) -> Game {
    let (loader, loader_version) = match flavour {
        Flavour::Vanilla | Flavour::Paper { .. } | Flavour::Spigot => (Loader::Vanilla, None),
        Flavour::Fabric { .. } => (Loader::Fabric, flavour.loader_version()),
        Flavour::Forge { .. } => (Loader::Forge, flavour.loader_version()),
        Flavour::NeoForge { .. } => (Loader::NeoForge, flavour.loader_version()),
    };
    Game {
        minecraft: version.to_string(),
        loader,
        loader_version: loader_version.filter(|v| !v.is_empty()),
    }
}

/// The settings a server's images carry. For now only the flavour; the typed
/// `server.properties` values, ops and whitelist come with a later milestone (the files
/// themselves are content entries already).
pub fn server_settings(flavour: &Flavour) -> Settings {
    let mut settings = Settings {
        server: Some(ServerSettings {
            flavour: Some(server_flavour(flavour)),
            ..ServerSettings::default()
        }),
        client: None,
    };
    settings.normalize();
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::minecraft::FabricLoaderVersion;

    #[test]
    fn games_and_flavours() {
        let fabric = Flavour::Fabric {
            loader_version: Some(FabricLoaderVersion::new("0.16.10".into())),
            installer_version: None,
        };
        assert_eq!(
            game_of("1.21.1", &fabric),
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            }
        );
        let paper: Flavour =
            serde_json::from_value(serde_json::json!({"paper": {"build_version": 100}})).unwrap();
        assert_eq!(game_of("1.21.1", &paper).loader, Loader::Vanilla);
        assert_eq!(game_of("1.21.1", &paper).loader_version, None);
        assert!(server_flavour(&paper).splits_worlds());
        assert!(server_flavour(&Flavour::Spigot).splits_worlds());
        assert!(!server_flavour(&fabric).splits_worlds());
        assert_eq!(
            server_settings(&paper)
                .server
                .and_then(|s| s.flavour)
                .map(|f| f.as_str().to_string()),
            Some("paper".to_string())
        );
    }

    #[test]
    fn layouts_come_from_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for path in [
            "world/level.dat",
            "world_nether/level.dat",
            "creative/level.dat",
        ] {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"x").unwrap();
        }
        std::fs::create_dir_all(root.join("mods")).unwrap();
        let paper = Flavour::Paper {
            build_version: None,
        };
        let (layout, level_is_world) = server_layout(root, "world", &paper).unwrap();
        assert!(level_is_world);
        let DiskLayout::Server(server) = layout else {
            panic!("a server layout");
        };
        assert!(server.is_split());
        assert_eq!(
            server.worlds().collect::<Vec<_>>(),
            vec!["creative", "world"]
        );
        let (layout, _) = server_layout(root, "world", &Flavour::Vanilla).unwrap();
        let DiskLayout::Server(server) = layout else {
            panic!("a server layout");
        };
        assert_eq!(
            server.worlds().collect::<Vec<_>>(),
            vec!["creative", "world", "world_nether"]
        );
        assert!(server_layout(&root.join("missing"), "world", &paper).is_err());
    }
}
