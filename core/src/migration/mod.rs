mod v042_to_v044;
pub mod v043_to_v044;

use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};

use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::{
    error::Error,
    implementations::minecraft::Flavour,
    persist::write_atomic_blocking,
    prelude::VERSION,
    traits::t_configurable::GameType,
    types::{DotLodestoneConfig, InstanceUuid, LodestoneMetadata},
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DotLodestoneConfigV043 {
    pub game_type: GameType,
    pub uuid: InstanceUuid,
    pub creation_time: i64,
    pub lodestone_version: String,
}

/// The format of an instance's `.lodestone_config`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceConfigVersion {
    /// v0.4.2: the whole Minecraft config, with `"game_type": "minecraft"`.
    V042,
    /// v0.4.3: the current fields plus `lodestone_version`.
    V043,
    /// v0.4.4 and later.
    Current,
}

/// Works out the version from the instance's own `.lodestone_config`, instead of guessing
/// from which other files exist in the instances folder. Returns `None` when the folder
/// has no readable `.lodestone_config` (a stray file, an unrelated folder, ...).
fn detect_instance_config_version(path_to_instance: &Path) -> Option<InstanceConfigVersion> {
    let contents = std::fs::read(path_to_instance.join(".lodestone_config")).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&contents).ok()?;
    let object = value.as_object()?;
    if object.contains_key("lodestone_version") {
        return Some(InstanceConfigVersion::V043);
    }
    if serde_json::from_value::<DotLodestoneConfig>(value.clone()).is_ok() {
        return Some(InstanceConfigVersion::Current);
    }
    if object.get("game_type").and_then(|v| v.as_str()) == Some("minecraft")
        && object.contains_key("flavour")
    {
        return Some(InstanceConfigVersion::V042);
    }
    None
}

/// Older version of Lodestone Core (v0.4.3 and below) does not store the version of Lodestone Core explicitly in version file.
///
/// More specifically, anything below v0.4.2 does not store version anywhere at all
///
/// While 0.4.3 only stores it via the `lodestone_version` field in instances' `.lodestone_config` file
///
/// From 0.4.4 onwards, the version stored under LODESTONE_PATH/version.json
///
/// The high-level migration process is as follows:
///
/// First check if the version file exists. If it does, then we can assume that the instance is at least v0.4.4
///
/// If the version file does not exist, then check if the `instances` directory has at least one instance with a `.lodestone_config` that contains the `lodestone_version` field
///
/// If it is, then we are at v0.4.3 and thus migrate to 0.4.4 by creating the version file
/// and rewrite all the `.lodestone_config` files to remove the `lodestone_version` field
/// Each instance is checked on its own, so a mix of formats (or a stray file in the
/// instances folder) is handled. If any instance fails to migrate, the version file is not
/// written, so the failed instances are retried on the next start.
pub fn migrate(lodestone_path: &Path) -> Result<(), Error> {
    let version_path = lodestone_path.join(".lodestone_metadata.json");
    // if the metadata exists, then it's not a legacy version
    if version_path.is_file() {
        info!("No migration needed");
        return Ok(());
    }
    let instances_path = lodestone_path.join("instances");
    let mut failed = 0;
    if instances_path.is_dir() {
        for entry in std::fs::read_dir(&instances_path)
            .context("Failed to read instances directory")?
            .filter_map(|entry| entry.ok())
        {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let version = detect_instance_config_version(&path);
            debug!(
                "Instance {} has config version {:?}",
                path.display(),
                version
            );
            let result = match version {
                Some(InstanceConfigVersion::V042) => {
                    info!("Migrating instance {} from v0.4.2", path.display());
                    v042_to_v044::migrate_v042_instance_to_v044(&path)
                }
                Some(InstanceConfigVersion::V043) => {
                    info!("Migrating instance {} from v0.4.3", path.display());
                    v043_to_v044::migrate_v043_instance_to_v044(&path)
                }
                Some(InstanceConfigVersion::Current) => Ok(()),
                None => {
                    warn!(
                        "Skipping {} during migration: no recognisable .lodestone_config",
                        path.display()
                    );
                    Ok(())
                }
            };
            if let Err(e) = result {
                error!("Failed to migrate instance at {}: {}", path.display(), e);
                failed += 1;
            }
        }
    }
    if failed > 0 {
        return Err(eyre!("{failed} instance(s) failed to migrate").into());
    }
    let metadata = serde_json::to_vec_pretty(&LodestoneMetadata {
        semver: VERSION.with(|v| v.clone()),
    })
    .context("Failed to serialize version file")?;
    write_atomic_blocking(&version_path, &metadata, None)
        .context("Failed to write version file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(dir: &Path, name: &str, config: serde_json::Value) -> PathBuf {
        let path = dir.join("instances").join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join(".lodestone_config"), config.to_string()).unwrap();
        path
    }

    fn v042_config() -> serde_json::Value {
        serde_json::json!({
            "game_type": "minecraft",
            "uuid": "INSTANCE_old",
            "name": "old",
            "version": "1.19.2",
            "flavour": "vanilla",
            "description": "",
            "cmd_args": [],
            "path": "/tmp/old",
            "port": 25565,
            "min_ram": 1024,
            "max_ram": 2048,
            "creation_time": 1,
            "auto_start": false,
            "restart_on_crash": false,
            "backup_period": null,
            "jre_major_version": 17,
            "has_started": false
        })
    }

    #[test]
    fn detects_each_format() {
        let dir = tempfile::tempdir().unwrap();
        let v042 = write_config(dir.path(), "a", v042_config());
        let v043 = write_config(
            dir.path(),
            "b",
            serde_json::json!({
                "game_type": "MinecraftJava",
                "uuid": "INSTANCE_b",
                "creation_time": 1,
                "lodestone_version": "0.4.3"
            }),
        );
        let current = write_config(
            dir.path(),
            "c",
            serde_json::json!({
                "game_type": "Generic",
                "uuid": "INSTANCE_c",
                "creation_time": 1
            }),
        );
        let stray = dir.path().join("instances").join("d");
        std::fs::create_dir_all(&stray).unwrap();

        use InstanceConfigVersion::*;
        assert_eq!(detect_instance_config_version(&v042), Some(V042));
        assert_eq!(detect_instance_config_version(&v043), Some(V043));
        assert_eq!(detect_instance_config_version(&current), Some(Current));
        assert_eq!(detect_instance_config_version(&stray), None);
    }

    #[test]
    fn current_instances_without_metadata_are_left_alone() {
        // Used to be treated as v0.4.2 (or v0.4.3), and that migration then failed.
        let dir = tempfile::tempdir().unwrap();
        let config = serde_json::json!({
            "game_type": "MinecraftJava",
            "uuid": "INSTANCE_c",
            "creation_time": 1
        });
        let current = write_config(dir.path(), "c", config.clone());
        std::fs::write(current.join(".lodestone_minecraft_config.json"), "{}").unwrap();
        std::fs::write(dir.path().join("instances").join("stray.txt"), "x").unwrap();

        migrate(dir.path()).unwrap();
        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(current.join(".lodestone_config")).unwrap())
                .unwrap();
        assert_eq!(after, config);
        assert!(dir.path().join(".lodestone_metadata.json").is_file());
    }

    #[test]
    fn migrates_v042_instance() {
        let dir = tempfile::tempdir().unwrap();
        let old = write_config(dir.path(), "a", v042_config());
        migrate(dir.path()).unwrap();
        assert_eq!(
            detect_instance_config_version(&old),
            Some(InstanceConfigVersion::Current)
        );
        assert!(old.join(".lodestone_minecraft_config.json").is_file());
    }

    #[test]
    fn unknown_v042_game_type_is_an_error_not_a_panic() {
        let mut config = v042_config();
        config["flavour"] = serde_json::json!("vanilla");
        let config: RestoreConfigV042 = serde_json::from_value(config).unwrap();
        let config = RestoreConfigV042 {
            game_type: "terraria".to_string(),
            ..config
        };
        assert!(DotLodestoneConfig::try_from(config).is_err());
    }
}
