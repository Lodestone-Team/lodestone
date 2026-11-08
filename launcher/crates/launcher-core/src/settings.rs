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
///
/// Fields are added with serde defaults, so an older file loads unchanged. Keys
/// this build does not know are kept, so a file edited by a newer build at the same
/// version survives a round trip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Developer options, off by default. Left out of the file while untouched.
    #[serde(default, skip_serializing_if = "is_default")]
    pub developer: DeveloperSettings,
    /// What every instance uses unless its own Settings tab overrides it. Left out
    /// of the file while untouched.
    #[serde(default, skip_serializing_if = "is_default")]
    pub instance_defaults: InstanceDefaults,
    /// Sidebar groups that are folded away.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed_groups: BTreeSet<String>,
    /// "Group instances automatically": the sidebar lists instances without a group
    /// of their own by loader, or under Imported. On by default; left out of the file
    /// while on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_groups: bool,
    /// Settings > General > Language. Follows the system's preferred languages by
    /// default; left out of the file while it does.
    #[serde(default, skip_serializing_if = "is_default")]
    pub language: Language,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            developer: DeveloperSettings::default(),
            instance_defaults: InstanceDefaults::default(),
            collapsed_groups: BTreeSet::new(),
            auto_groups: true,
            language: Language::System,
            other: Map::new(),
        }
    }
}

/// The language the launcher shows, from Settings > General.
///
/// A value this build does not know (from a newer build with more languages) reads as
/// [`Language::System`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "zh-Hans")]
    SimplifiedChinese,
    #[serde(rename = "zh-Hant")]
    TraditionalChinese,
    /// The first of the system's preferred languages the launcher has, else English.
    /// Last, as serde wants of the variant that reads unknown values.
    #[default]
    #[serde(rename = "system", other)]
    System,
}

impl Language {
    /// The choices, in the order Settings lists them.
    pub const ALL: [Language; 5] = [
        Language::System,
        Language::English,
        Language::French,
        Language::SimplifiedChinese,
        Language::TraditionalChinese,
    ];
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Settings > Developer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeveloperSettings {
    /// "Sign in as the Minecraft Launcher": new Microsoft sign-ins use the official
    /// launcher's client ([`ClientIdentity::MinecraftLauncher`]) instead of
    /// Lodestone's app.
    ///
    /// [`ClientIdentity::MinecraftLauncher`]: crate::auth::ClientIdentity::MinecraftLauncher
    #[serde(default)]
    pub sign_in_as_minecraft_launcher: bool,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Loads the settings, or the defaults if the file is missing or unreadable.
pub fn load_or_default(path: &Path) -> Settings {
    storage::load::<Settings>(path)
        .map(|loaded| loaded.document)
        .unwrap_or_default()
}

/// Changes the settings on disk: reads the file as it is now, applies `change` and
/// writes it back, so changes saved by other parts of the app are kept.
///
/// A file this build cannot read (a newer version, or not valid JSON) is left alone
/// and the error returned, rather than replaced with defaults.
pub fn update(path: &Path, change: impl FnOnce(&mut Settings)) -> Result<Settings, StorageError> {
    let mut settings = storage::load::<Settings>(path)?.document;
    change(&mut settings);
    storage::save(path, &settings)?;
    Ok(settings)
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

/// Loads the settings, or the defaults if there is no file. Changes go through
/// [`update`], so the parts of the app that each own a section keep each other's.
pub fn load(path: &Path) -> Result<Settings, StorageError> {
    Ok(storage::load::<Settings>(path)?.document)
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
    fn developer_options_are_off_by_default() {
        assert!(!Settings::default().developer.sign_in_as_minecraft_launcher);
        let dir = tempfile::tempdir().unwrap();
        // No file, an empty file at version 1, and a file from before the field.
        let missing = dir.path().join("missing.json");
        assert!(
            !load_or_default(&missing)
                .developer
                .sign_in_as_minecraft_launcher
        );
        for raw in [
            r#"{"version": 1}"#,
            r#"{"version": 1, "developer": {}}"#,
            "{}",
        ] {
            let path = dir.path().join("settings.json");
            std::fs::write(&path, raw).unwrap();
            let loaded = load_or_default(&path);
            assert!(!loaded.developer.sign_in_as_minecraft_launcher, "{raw}");
        }
    }

    #[test]
    fn update_keeps_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"version": 1, "memory": {"max_mb": 4096}}"#).unwrap();
        let settings = update(&path, |settings| {
            settings.developer.sign_in_as_minecraft_launcher = true;
        })
        .unwrap();
        assert!(settings.developer.sign_in_as_minecraft_launcher);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw,
            json!({
                "version": 1,
                "memory": {"max_mb": 4096},
                "developer": {"sign_in_as_minecraft_launcher": true},
            })
        );
        assert!(
            load_or_default(&path)
                .developer
                .sign_in_as_minecraft_launcher
        );

        // A file from a newer build is not overwritten.
        std::fs::write(&path, r#"{"version": 99}"#).unwrap();
        assert!(update(&path, |_| {}).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"version": 99}"#
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

    /// The Developer section (saved by the accounts) and the instance defaults (saved
    /// by the instances) each change only their own part of the file.
    #[test]
    fn sections_saved_separately_keep_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        update(&path, |s| s.developer.sign_in_as_minecraft_launcher = true).unwrap();
        update(&path, |s| {
            s.instance_defaults.max_ram = Some(6144);
            s.collapsed_groups.insert("Modded".into());
        })
        .unwrap();
        update(&path, |s| s.developer.sign_in_as_minecraft_launcher = false).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.instance_defaults.max_ram, Some(6144));
        assert!(loaded.collapsed_groups.contains("Modded"));
        assert!(!loaded.developer.sign_in_as_minecraft_launcher);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw,
            json!({
                "version": 1,
                "instance_defaults": {"max_ram": 6144, "jvm_args": "", "game_args": "", "fullscreen": false},
                "collapsed_groups": ["Modded"],
            })
        );
    }

    #[test]
    fn automatic_groups_are_on_unless_turned_off() {
        assert!(Settings::default().auto_groups);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"version": 1}"#).unwrap();
        assert!(
            load(&path).unwrap().auto_groups,
            "a file without it has it on"
        );
        update(&path, |s| s.auto_groups = false).unwrap();
        assert!(!load(&path).unwrap().auto_groups);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw, json!({"version": 1, "auto_groups": false}));
        update(&path, |s| s.auto_groups = true).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw, json!({"version": 1}), "left out while on");
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

    #[test]
    fn language_round_trips_and_unknown_reads_as_system() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let raw = |path: &Path| -> Value {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        };
        // Left out of the file while it follows the system.
        storage::save(&path, &Settings::default()).unwrap();
        assert!(raw(&path).get("language").is_none());
        update(&path, |settings| {
            settings.language = Language::TraditionalChinese
        })
        .unwrap();
        assert_eq!(raw(&path)["language"], "zh-Hant");
        assert_eq!(
            load_or_default(&path).language,
            Language::TraditionalChinese
        );
        std::fs::write(&path, r#"{"version": 1, "language": "eo"}"#).unwrap();
        assert_eq!(load_or_default(&path).language, Language::System);
        for (language, code) in [
            (Language::English, "en"),
            (Language::French, "fr"),
            (Language::SimplifiedChinese, "zh-Hans"),
        ] {
            assert_eq!(serde_json::to_value(language).unwrap(), code);
        }
    }
}
