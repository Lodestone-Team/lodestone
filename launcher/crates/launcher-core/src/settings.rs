//! `settings.json`: the launcher's settings. It holds no secrets.
//!
//! Every field has a serde default and keys this build does not know are kept, so
//! sections added by other builds (or later milestones) survive a round trip, and a
//! file missing a section reads as that section's defaults.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    instances::{Resolution, WindowSettings},
    storage::{self, Document, Migration, StorageError},
};

/// The launcher's settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// What every instance uses unless its own Settings tab overrides it.
    #[serde(default)]
    pub instance_defaults: InstanceDefaults,
    /// Sidebar groups that are folded away.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed_groups: BTreeSet<String>,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Document for Settings {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

/// The global defaults of the instance settings. `None` memory means the launcher's
/// own default for this Mac.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceDefaults {
    /// MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_ram: Option<u32>,
    /// MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ram: Option<u32>,
    /// Extra JVM arguments, as typed.
    #[serde(default)]
    pub jvm_args: String,
    /// Extra game arguments, as typed.
    #[serde(default)]
    pub game_args: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

/// The default minimum memory.
pub const DEFAULT_MIN_RAM_MB: u32 = 512;

/// The default maximum memory: half the machine's memory, at most 4 GiB.
pub fn default_max_ram_mb(total_bytes: Option<u64>) -> u32 {
    let total_mb = total_bytes.map_or(8192, |bytes| bytes / (1024 * 1024));
    (total_mb / 2).clamp(u64::from(DEFAULT_MIN_RAM_MB), 4096) as u32
}

/// Whether `max_mb` is more than about three quarters of the machine's memory, which
/// leaves too little for macOS and other apps.
pub fn max_ram_is_high(max_mb: u32, total_bytes: Option<u64>) -> bool {
    total_bytes.is_some_and(|total| u64::from(max_mb) * 1024 * 1024 * 4 > total * 3)
}

impl InstanceDefaults {
    /// The memory instances get unless they override it: (min, max) in MiB.
    pub fn memory(&self, total_bytes: Option<u64>) -> (u32, u32) {
        let max = self
            .max_ram
            .unwrap_or_else(|| default_max_ram_mb(total_bytes));
        let min = self.min_ram.unwrap_or(DEFAULT_MIN_RAM_MB).min(max);
        (min, max)
    }

    pub fn window(&self) -> WindowSettings {
        WindowSettings {
            resolution: self.resolution,
            fullscreen: self.fullscreen,
        }
    }
}

/// Loads the settings, or the defaults if there is no file.
pub fn load(path: &Path) -> Result<Settings, StorageError> {
    Ok(storage::load::<Settings>(path)?.document)
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), StorageError> {
    storage::save(path, settings)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn round_trips_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "future_key": [1, 2], "instance_defaults": {"max_ram": 3072, "later": true}}"#,
        )
        .unwrap();
        let loaded = storage::load::<Settings>(&path).unwrap();
        assert_eq!(loaded.document.instance_defaults.max_ram, Some(3072));
        storage::save(&path, &loaded.document).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw,
            json!({
                "version": 1,
                "future_key": [1, 2],
                "instance_defaults": {"max_ram": 3072, "later": true, "jvm_args": "", "game_args": "", "fullscreen": false},
            })
        );
    }

    #[test]
    fn migrates_an_unversioned_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"kept": true}"#).unwrap();
        let loaded = storage::load::<Settings>(&path).unwrap();
        assert!(loaded.migrated(Settings::VERSION));
        assert_eq!(loaded.document.other.get("kept"), Some(&json!(true)));
        assert_eq!(
            loaded.document.instance_defaults,
            InstanceDefaults::default()
        );
    }

    #[test]
    fn default_memory() {
        let gib = |n: u64| Some(n << 30);
        assert_eq!(default_max_ram_mb(gib(4)), 2048);
        assert_eq!(default_max_ram_mb(gib(6)), 3072);
        assert_eq!(default_max_ram_mb(gib(16)), 4096);
        assert_eq!(default_max_ram_mb(gib(64)), 4096);
        assert_eq!(default_max_ram_mb(Some(512 << 20)), 512);
        assert_eq!(default_max_ram_mb(None), 4096);
        let defaults = InstanceDefaults::default();
        assert_eq!(defaults.memory(gib(16)), (512, 4096));
        let custom = InstanceDefaults {
            min_ram: Some(8192),
            max_ram: Some(6144),
            ..InstanceDefaults::default()
        };
        assert_eq!(custom.memory(gib(16)), (6144, 6144), "min never above max");
        assert!(max_ram_is_high(12_800, gib(16)));
        assert!(!max_ram_is_high(12_000, gib(16)));
        assert!(!max_ram_is_high(12_800, None));
    }
}
