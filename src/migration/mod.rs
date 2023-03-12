mod v042_to_v043;

use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;

use tracing::debug;

use crate::{error::Error, implementations::minecraft::Flavour, types::InstanceUuid};

#[derive(serde::Deserialize, Clone)]
pub struct RestoreConfigV042 {
    pub game_type: String,
    pub uuid: InstanceUuid,
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub description: String,
    pub cmd_args: Vec<String>,
    pub path: PathBuf,
    pub port: u32,
    pub min_ram: u32,
    pub max_ram: u32,
    pub creation_time: i64,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub backup_period: Option<u32>,
    pub jre_major_version: u64,
    pub has_started: bool,
}

pub async fn migrate(path_to_instance: &Path) -> Result<(), Error> {
    let path_to_dot_lodestone_config = path_to_instance.join(".lodestone_config");
    let dot_lodestone_config: serde_json::Value = serde_json::from_reader(
        std::fs::File::open(&path_to_dot_lodestone_config).context(format!(
            "Failed to open config file at {}",
            &path_to_dot_lodestone_config.display()
        ))?,
    )
    .context("Failed to deserialize config from string. Was the config file modified manually?")?;

    match dot_lodestone_config.get("lodestone_version") {
        None => {
            // Version 0.4.2 did not have a version field in the config
            debug!(
                "Migrating {} from v0.4.2 to v0.4.3",
                path_to_instance.display()
            );
            v042_to_v043::migrate_v042_to_v043(dot_lodestone_config, path_to_instance).await?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use serde_json::json;

    use crate::{
        implementations::minecraft::{Flavour, RestoreConfig},
        traits::t_configurable::GameType,
        types::DotLodestoneConfig,
    };

    use super::migrate;

    #[tokio::test]
    async fn test_migrate_v042_to_v043() {
        // setup temp dir
        let temp_dir = tempdir::TempDir::new("test_migrate_v042_to_v043").unwrap();
        let path_to_instance = temp_dir.path().join("test_instance");
        std::fs::create_dir(&path_to_instance).unwrap();

        // write old config
        let old_dot_lodestone_config = json!({
            "game_type": "minecraft",
            "uuid": "a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6",
            "name": "test_instance",
            "version": "1.1.1",
            "flavour": "fabric",
            "description": "test instance",
            "cmd_args": [],
            "path": "/home/user/.minecraft",
            "port": 25565,
            "min_ram": 1024,
            "max_ram": 1024,
            "creation_time": 1,
            "auto_start": false,
            "restart_on_crash": false,
            "backup_period": null,
            "jre_major_version": 11,
            "has_started": false
        });

        let path_to_dot_lodestone_config = path_to_instance.join(".lodestone_config");
        let path_to_dot_lodestone_minecraft_config =
            path_to_instance.join(".lodestone_minecraft_config.json");

        std::fs::write(
            &path_to_dot_lodestone_config,
            old_dot_lodestone_config.to_string(),
        )
        .unwrap();

        // migrate

        migrate(path_to_instance.as_path()).await.unwrap();

        // check new config

        let new_dot_lodestone_config: DotLodestoneConfig =
            serde_json::from_reader(std::fs::File::open(&path_to_dot_lodestone_config).unwrap())
                .unwrap();

        let new_dot_lodestone_minecraft_config: RestoreConfig = serde_json::from_reader(
            std::fs::File::open(&path_to_dot_lodestone_minecraft_config).unwrap(),
        )
        .unwrap();

        assert_eq!(
            new_dot_lodestone_config.uuid().as_ref(),
            "a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6"
        );
        assert_eq!(new_dot_lodestone_config.creation_time() as u64, 1);
        assert_eq!(new_dot_lodestone_config.lodestone_version(), "0.4.3");
        assert_eq!(
            new_dot_lodestone_config.game_type(),
            &GameType::MinecraftJava
        );

        assert_eq!(
            new_dot_lodestone_minecraft_config.flavour,
            Flavour::Fabric {
                loader_version: None,
                installer_version: None,
            }
        );
    }
}
