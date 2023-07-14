use crate::{error::Error, types::DotLodestoneConfig};
use color_eyre::eyre::Context;
use std::path::Path;
use tracing::error;

use super::DotLodestoneConfigV043;

pub fn migrate_v043_to_v044(path_to_instances: &Path) -> Result<(), Error> {
    for instance in path_to_instances
        .read_dir()
        .context(format!(
            "Failed to read instances directory at {}",
            path_to_instances.display()
        ))?
        .filter_map(|entry| entry.ok())
    {
        if !instance.path().join(".lodestone_config").is_file() {
            continue;
        }
        migrate_v043_instance_to_v044(&instance.path()).map_err(|e| {
            error!(
                "Failed to migrate instance at {}: {}",
                instance.path().display(),
                e
            );
            e
        })?;
    }
    Ok(())
}

fn migrate_v043_instance_to_v044(path_to_instance: &Path) -> Result<(), Error> {
    let dot_lodestone_file = std::fs::File::open(path_to_instance.join(".lodestone_config"))
        .context(format!(
            "Failed to read config file at {}",
            &path_to_instance.join(".lodestone_config").display()
        ))?;
    let dot_lodestone_config: DotLodestoneConfigV043 = serde_json::from_reader(&dot_lodestone_file)
        .context(format!(
            "Failed to parse config file at {}",
            &path_to_instance.join(".lodestone_config").display()
        ))?;

    let new_dot_lodestone_config: DotLodestoneConfig = dot_lodestone_config.into();

    let string = serde_json::to_string_pretty(&new_dot_lodestone_config).unwrap();
    std::fs::write(path_to_instance.join(".lodestone_config"), string).context(format!(
        "Failed to write config file at {}",
        &path_to_instance.join(".lodestone_config").display()
    ))?;
    Ok(())
}
