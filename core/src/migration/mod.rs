mod v042_to_v044;
pub mod v043_to_v044;

use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;

use serde::Deserialize;
use tracing::{debug, info};

use crate::{
    error::Error,
    implementations::minecraft::Flavour,
    prelude::VERSION,
    traits::t_configurable::GameType,
    types::{InstanceUuid, LodestoneMetadata},
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

/// Versions without a lodestone_metadata.json file
#[derive(Debug, Clone, PartialEq, Eq)]
enum LegacyVersion {
    V042,
    V043,
}

fn determine_legacy_version(lodestone_path: &Path) -> Result<Option<LegacyVersion>, Error> {
    let metadata_path = lodestone_path.join(".lodestone_metadata.json");
    // if the metadata exists, then it's not a legacy version
    if metadata_path.is_file() {
        Ok(None)
    } else {
        // check if there is at least one instance with a .lodestone_config file
        let instances_path = lodestone_path.join("instances");
        if !instances_path.is_dir() {
            return Ok(None);
        }
        let mut at_least_one_instance = false;
        for entry in std::fs::read_dir(instances_path)
            .context("Failed to read instances directory")?
            .filter_map(|entry| entry.ok())
        {
            at_least_one_instance = true;
            let path = entry.path();
            if path.is_dir() && path.join(".lodestone_minecraft_config.json").is_file() {
                return Ok(Some(LegacyVersion::V043));
            }
        }
        if at_least_one_instance {
            Ok(Some(LegacyVersion::V042))
        } else {
            Ok(None)
        }
    }
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
///
///

pub fn migrate(lodestone_path: &Path) -> Result<(), Error> {
    let legacy_version = determine_legacy_version(lodestone_path)?;
    debug!("Legacy version: {:?}", legacy_version);
    match legacy_version {
        Some(LegacyVersion::V042) => {
            info!("Migrating from v0.4.2 to v0.4.3");
            v042_to_v044::migrate_v042_to_v044(&lodestone_path.join("instances"))?;
        }
        Some(LegacyVersion::V043) => {
            info!("Migrating from v0.4.3 to v0.4.4");
            v043_to_v044::migrate_v043_to_v044(&lodestone_path.join("instances"))?;
        }
        None => {
            info!("No migration needed");
        }
    }
    let version_path = lodestone_path.join(".lodestone_metadata.json");
    let version_file =
        std::fs::File::create(version_path).context("Failed to create version file")?;
    serde_json::to_writer_pretty(
        version_file,
        &LodestoneMetadata {
            semver: VERSION.with(|v| v.clone()),
        },
    )
    .context("Failed to write version file")?;
    Ok(())
}

