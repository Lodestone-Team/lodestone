use crate::{error::Error, persist::write_atomic_blocking, types::DotLodestoneConfig};
use color_eyre::eyre::Context;
use std::path::Path;

use super::DotLodestoneConfigV043;

/// Rewrites a v0.4.3 `.lodestone_config` (which has a `lodestone_version` field) in the
/// current format.
pub(super) fn migrate_v043_instance_to_v044(path_to_instance: &Path) -> Result<(), Error> {
    let config_path = path_to_instance.join(".lodestone_config");
    let dot_lodestone_file = std::fs::File::open(&config_path).context(format!(
        "Failed to read config file at {}",
        config_path.display()
    ))?;
    let dot_lodestone_config: DotLodestoneConfigV043 = serde_json::from_reader(&dot_lodestone_file)
        .context(format!(
            "Failed to parse config file at {}",
            config_path.display()
        ))?;

    let new_dot_lodestone_config: DotLodestoneConfig = dot_lodestone_config.into();

    let string = serde_json::to_string_pretty(&new_dot_lodestone_config)
        .context("Failed to serialize config file")?;
    write_atomic_blocking(&config_path, string.as_bytes(), None).context(format!(
        "Failed to write config file at {}",
        config_path.display()
    ))?;
    Ok(())
}
