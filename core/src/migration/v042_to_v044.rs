use std::path::Path;

use color_eyre::eyre::Context;
use serde_json::{json, Value};

use crate::{
    error::Error, implementations::minecraft::RestoreConfig, persist::write_atomic_blocking,
};

use super::RestoreConfigV042;

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
            java_cmd: None,
        }
    }
}

/// Splits a v0.4.2 `.lodestone_config` into the current `.lodestone_config` and
/// `.lodestone_minecraft_config.json`.
pub(super) fn migrate_v042_instance_to_v044(path_to_instance: &Path) -> Result<(), Error> {
    let mut old_dot_lodestone_config: Value = serde_json::from_reader(
        std::fs::File::open(path_to_instance.join(".lodestone_config")).context(format!(
            "Failed to read config file at {}",
            path_to_instance.join(".lodestone_config").display()
        ))?,
    )
    .context("Failed to deserialize old config file. This is likely a bug in Lodestone.")?;
    if let Some("fabric") = old_dot_lodestone_config["flavour"].as_str() {
        old_dot_lodestone_config["flavour"] =
            json!({ "fabric": { "loader_version": null, "installer_version": null } });
    } else if let Some("paper") = old_dot_lodestone_config["flavour"].as_str() {
        old_dot_lodestone_config["flavour"] = json!({ "paper": { "build_version": null } });
    }

    let path_to_dot_lodestone_config = path_to_instance.join(".lodestone_config");
    let path_to_dot_lodestone_minecraft_config =
        path_to_instance.join(".lodestone_minecraft_config.json");
    let dot_lodestone_config: RestoreConfigV042 = serde_json::from_value(old_dot_lodestone_config)
        .context("Failed to deserialize old config file. This is a bug in Lodestone.")?;

    let dot_lodestone_config_new: crate::types::DotLodestoneConfig =
        dot_lodestone_config.clone().try_into()?;
    let dot_lodestone_config_new = serde_json::to_string_pretty(&dot_lodestone_config_new)
        .context("Failed to serialize config file")?;
    let dot_lodestone_minecraft_config: RestoreConfig = dot_lodestone_config.into();
    let dot_lodestone_minecraft_config =
        serde_json::to_string_pretty(&dot_lodestone_minecraft_config)
            .context("Failed to serialize Minecraft config file")?;

    // Write the Minecraft config first: until `.lodestone_config` is rewritten the
    // instance is still detected as v0.4.2, so an interrupted migration is retried.
    write_atomic_blocking(
        &path_to_dot_lodestone_minecraft_config,
        dot_lodestone_minecraft_config.as_bytes(),
        None,
    )
    .context(format!(
        "Failed to write config file at {}",
        path_to_dot_lodestone_minecraft_config.display()
    ))?;
    write_atomic_blocking(
        &path_to_dot_lodestone_config,
        dot_lodestone_config_new.as_bytes(),
        None,
    )
    .context(format!(
        "Failed to write config file at {}",
        path_to_dot_lodestone_config.display()
    ))?;
    Ok(())
}
