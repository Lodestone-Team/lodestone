use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use serde_json::json;

use crate::{
    error::Error,
    implementations::minecraft::{Flavour, RestoreConfig},
    types::InstanceUuid,
};

pub mod v042_to_v043 {
    use std::path::{Path, PathBuf};

    use color_eyre::eyre::Context;
    use serde_json::json;

    use crate::{
        error::Error,
        implementations::minecraft::{Flavour, RestoreConfig},
        types::InstanceUuid,
    };

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

    impl From<RestoreConfigV042> for crate::types::DotLodestoneConfig {
        fn from(config: RestoreConfigV042) -> Self {
            Self {
                uuid: config.uuid,
                creation_time: config.creation_time,
                lodestone_version: "0.4.3".to_string(),
            }
        }
    }
    impl From<RestoreConfigV042> for RestoreConfig {
        fn from(config: RestoreConfigV042) -> Self {
            Self {
                name: config.name,
                version: config.version,
                flavour: config.flavour,
                description: config.description,
                cmd_args: config.cmd_args,
                port: config.port,
                min_ram: config.min_ram,
                max_ram: config.max_ram,
                auto_start: config.auto_start,
                restart_on_crash: config.restart_on_crash,
                backup_period: config.backup_period,
                jre_major_version: config.jre_major_version,
                has_started: config.has_started,
            }
        }
    }

    pub async fn migrate_v042_to_v043(
        mut old_dot_lodestone_config: serde_json::Value,
        path_to_instance: &Path,
    ) -> Result<(), Error> {
        if let Some("fabric") = old_dot_lodestone_config["flavour"].as_str() {
            old_dot_lodestone_config["flavour"] =
                json!({ "fabric": { "loader_version": null, "installer_version": null } });
        } else if let Some("paper") = old_dot_lodestone_config["flavour"].as_str() {
            old_dot_lodestone_config["flavour"] = json!({ "paper": { "build_version": null } });
        }

        let path_to_dot_lodestone_config = path_to_instance.join(".lodestone_config");
        let path_to_dot_lodestone_minecraft_config =
            path_to_instance.join(".lodestone_minecraft_config.json");
        let dot_lodestone_config: RestoreConfigV042 = serde_json::from_value(
            old_dot_lodestone_config,
        )
        .context("Failed to deserialize old config file. This is likely a bug in Lodestone.")?;

        let dot_lodestone_config_new: crate::types::DotLodestoneConfig =
            dot_lodestone_config.clone().into();
        let dot_lodestone_config_new =
            serde_json::to_string_pretty(&dot_lodestone_config_new).unwrap();
        tokio::fs::write(&path_to_dot_lodestone_config, dot_lodestone_config_new)
            .await
            .context(format!(
                "Failed to write config file at {}",
                &path_to_dot_lodestone_config.display()
            ))?;

        let dot_lodestone_minecraft_config: RestoreConfig = dot_lodestone_config.into();
        let dot_lodestone_minecraft_config =
            serde_json::to_string_pretty(&dot_lodestone_minecraft_config).unwrap();
        tokio::fs::write(
            &path_to_dot_lodestone_minecraft_config,
            dot_lodestone_minecraft_config,
        )
        .await
        .context(format!(
            "Failed to write config file at {}",
            &path_to_dot_lodestone_minecraft_config.display()
        ))?;
        Ok(())
    }
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
            v042_to_v043::migrate_v042_to_v043(dot_lodestone_config, path_to_instance).await?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use color_eyre::eyre::Context;
    use serde_json::json;

    use crate::{
        implementations::minecraft::{Flavour, RestoreConfig},
        types::{DotLodestoneConfig, InstanceUuid},
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
            new_dot_lodestone_config.uuid.as_ref(),
            "a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6"
        );
        assert_eq!(new_dot_lodestone_config.creation_time as u64, 1);
        assert_eq!(new_dot_lodestone_config.lodestone_version, "0.4.3");

        assert_eq!(
            new_dot_lodestone_minecraft_config.flavour,
            Flavour::Fabric {
                loader_version: None,
                installer_version: None,
            }
        );
    }
}
