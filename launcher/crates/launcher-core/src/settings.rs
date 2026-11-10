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
    /// Developer options. Left out of the file while untouched.
    #[serde(default, skip_serializing_if = "is_default")]
    pub developer: DeveloperSettings,
    /// Settings > Developer > Command line: what the control socket allows. Changed
    /// only in the launcher's window, never over the socket. Left out of the file
    /// while untouched.
    #[serde(default, skip_serializing_if = "is_default")]
    pub cli: CliSettings,
    /// What every instance uses unless its own Settings tab overrides it. Left out
    /// of the file while untouched.
    #[serde(default, skip_serializing_if = "is_default")]
    pub instance_defaults: InstanceDefaults,
    /// Sidebar groups that are folded away.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed_groups: BTreeSet<String>,
    /// "Group instances automatically": the sidebar lists instances without a group
    /// of their own under Servers (linked to a server), by loader, or under Imported.
    /// On by default; left out of the file while on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_groups: bool,
    /// Settings > Instance defaults > "Set up new instances automatically": a new
    /// instance's memory, Java arguments and game options are chosen for this computer
    /// and its mods ([`crate::configurator`]). On by default; left out of the file
    /// while on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_configure: bool,
    /// Settings > General > Language. Follows the system's preferred languages by
    /// default; left out of the file while it does.
    #[serde(default, skip_serializing_if = "is_default")]
    pub language: Language,
    /// Settings > General > "Update automatically": new builds are downloaded and
    /// installed in the background, and run when the launcher restarts. On by
    /// default; left out of the file while on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_update: bool,
    /// Settings > General > "Beta builds": which release the launcher updates from
    /// ([`crate::update`]). Stable by default; left out of the file while stable.
    #[serde(default, skip_serializing_if = "is_default")]
    pub update_channel: UpdateChannel,
    /// Settings > General > "When a game starts": what the launcher's window does
    /// once the game's window opens. Minimize by default; left out of the file then.
    #[serde(default, skip_serializing_if = "is_default")]
    pub after_launch: AfterLaunch,
    /// The launcher has offered to make a new instance the main one for shared game
    /// settings, which it does once (Settings > Developer can ask again). Left out of
    /// the file until then.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main_instance_offered: bool,
    /// The notification at start that offers to keep the narrator off in every
    /// instance was accepted or dismissed, so it doesn't come back (Settings >
    /// Developer can ask again). Left out of the file until then.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub narrator_offered: bool,
    /// The Lodestone Host page's offer to download Lodestone Host, shown while no
    /// Host or Core runs on this computer, was dismissed (Settings > Developer can
    /// show it again). Left out of the file until then.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub host_offer_dismissed: bool,
    /// The Friends section's "How to add a friend" steps were hidden ("Show steps"
    /// brings them back). Left out of the file until then.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub friend_steps_hidden: bool,
    /// The interface's zoom, from Ctrl/⌘ + and −: 1.0 is 100%. Left out of the file
    /// at 100%.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub zoom: f32,
    /// Settings > Launcher > Skin: the id of the launcher's skin
    /// ([`crate::ui_skin`]). Left out of the file for the default look.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<String>,
    /// The sidebar's width, dragged from its edge, in logical pixels at 100% zoom.
    /// Left out of the file at the default width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_width: Option<f32>,
    /// The sidebar is folded into a rail of icons (dragged narrower than it goes).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sidebar_collapsed: bool,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            developer: DeveloperSettings::default(),
            cli: CliSettings::default(),
            instance_defaults: InstanceDefaults::default(),
            collapsed_groups: BTreeSet::new(),
            auto_groups: true,
            auto_configure: true,
            language: Language::System,
            auto_update: true,
            update_channel: UpdateChannel::Stable,
            after_launch: AfterLaunch::Minimize,
            main_instance_offered: false,
            narrator_offered: false,
            host_offer_dismissed: false,
            friend_steps_hidden: false,
            zoom: 1.0,
            skin: None,
            sidebar_width: None,
            sidebar_collapsed: false,
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

/// Which builds the launcher updates to, from Settings > General > "Beta builds"
/// (and "Developer builds", which only developers see).
///
/// A value this build does not know reads as [`UpdateChannel::Stable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpdateChannel {
    /// Developers' builds (`launcher-dev-<build>-<commit>`, for developers only),
    /// betas and stable releases, whichever has the newest build. Settings only
    /// offers it in a developer build, or once it is on; a launcher from before it
    /// reads it as stable.
    #[serde(rename = "dev")]
    Dev,
    /// Pre-releases (`launcher-vX.Y.Z-beta.N`) and stable releases, whichever has
    /// the newer build.
    #[serde(rename = "beta")]
    Beta,
    /// Stable releases (`launcher-vX.Y.Z`) only. Last, as serde wants of the
    /// variant that reads unknown values.
    #[default]
    #[serde(rename = "stable", other)]
    Stable,
}

/// What the launcher does once a game's window opens, from Settings > General >
/// "When a game starts".
///
/// A value this build does not know reads as [`AfterLaunch::Minimize`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AfterLaunch {
    /// The window stays as it is.
    #[serde(rename = "stay")]
    Stay,
    /// The launcher quits. The game keeps running, but its log, crash report and
    /// play time are no longer recorded.
    #[serde(rename = "quit")]
    Quit,
    /// The window is minimized, and comes back when the game exits. Last, as serde
    /// wants of the variant that reads unknown values.
    #[default]
    #[serde(rename = "minimize", other)]
    Minimize,
}

impl AfterLaunch {
    /// The choices, in the order Settings lists them.
    pub const ALL: [AfterLaunch; 3] = [AfterLaunch::Stay, AfterLaunch::Minimize, AfterLaunch::Quit];
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

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Settings > Developer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeveloperSettings {
    /// "Sign in as the Minecraft Launcher": new Microsoft sign-ins use the official
    /// launcher's client ([`ClientIdentity::MinecraftLauncher`]) instead of
    /// Lodestone's app. On by default until Lodestone's own app is approved; left out
    /// of the file while on.
    ///
    /// [`ClientIdentity::MinecraftLauncher`]: crate::auth::ClientIdentity::MinecraftLauncher
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub sign_in_as_minecraft_launcher: bool,
    /// "Offline accounts": "Add offline account" in the account menu and Settings >
    /// Accounts. Always on in debug builds and with `LODESTONE_LAUNCHER_OFFLINE=1`
    /// ([`crate::offline_mode_enabled`]); off by default otherwise, and left out of the
    /// file while off.
    #[serde(default, skip_serializing_if = "is_false")]
    pub offline_accounts: bool,
    /// "Connect service (relay)": the Lodestone Connect introduction service the
    /// friend list uses, as a URL (`https://connect-dev.lodestone.cc`), for testing.
    /// `LODESTONE_CONNECT_SERVICE` wins over it ([`crate::cores::connect_service`]).
    /// Left out of the file for connect.lodestone.cc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_service: Option<String>,
    /// Developer options this build doesn't know, kept so that a newer build's
    /// options survive a save by this one (as [`Settings::other`] does for the whole
    /// file).
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            sign_in_as_minecraft_launcher: true,
            offline_accounts: false,
            connect_service: None,
            other: Map::new(),
        }
    }
}

/// Settings > Developer > Command line: what `lodestone-launcher-cli` and other
/// local clients of the control socket may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliSettings {
    /// "Allow command-line connections": the control socket is open. On by default.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// "Access": full control, or only methods that change nothing.
    #[serde(default, skip_serializing_if = "is_default")]
    pub access: CliAccess,
    /// "Debug traces": the trace bus records events and `trace.*` answers. On by
    /// default.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub traces: bool,
    /// "Game logs": clients may read game output (`play.logs`, the `game.log`
    /// trace topic). On by default.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub game_logs: bool,
}

impl Default for CliSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            access: CliAccess::Full,
            traces: true,
            game_logs: true,
        }
    }
}

/// How much a control-socket client may do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliAccess {
    /// Every method.
    #[default]
    Full,
    /// Only methods that read; the rest answer `permission_denied`.
    ReadOnly,
}

impl CliAccess {
    /// The name the socket reports: `full` or `read_only`.
    pub fn as_str(self) -> &'static str {
        match self {
            CliAccess::Full => "full",
            CliAccess::ReadOnly => "read_only",
        }
    }
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
    fn update_channel_defaults_to_stable_and_stays_out_of_the_file() {
        let settings: Settings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings.update_channel, UpdateChannel::Stable);
        let raw = serde_json::to_value(&settings).unwrap();
        assert!(raw.get("update_channel").is_none());

        let beta: Settings = serde_json::from_value(json!({"update_channel": "beta"})).unwrap();
        assert_eq!(beta.update_channel, UpdateChannel::Beta);
        assert_eq!(
            serde_json::to_value(&beta).unwrap()["update_channel"],
            json!("beta")
        );

        let unknown: Settings =
            serde_json::from_value(json!({"update_channel": "nightly"})).unwrap();
        assert_eq!(unknown.update_channel, UpdateChannel::Stable);
    }

    #[test]
    fn after_launch_defaults_to_minimize_and_stays_out_of_the_file() {
        let settings: Settings = serde_json::from_value(json!({})).unwrap();
        assert_eq!(settings.after_launch, AfterLaunch::Minimize);
        assert!(
            serde_json::to_value(&settings)
                .unwrap()
                .get("after_launch")
                .is_none()
        );

        for (value, name) in [(AfterLaunch::Stay, "stay"), (AfterLaunch::Quit, "quit")] {
            let settings: Settings = serde_json::from_value(json!({"after_launch": name})).unwrap();
            assert_eq!(settings.after_launch, value);
            assert_eq!(
                serde_json::to_value(&settings).unwrap()["after_launch"],
                json!(name)
            );
        }

        let unknown: Settings = serde_json::from_value(json!({"after_launch": "hide"})).unwrap();
        assert_eq!(unknown.after_launch, AfterLaunch::Minimize);
    }

    #[test]
    fn cli_settings_default_to_open_and_stay_out_of_the_file() {
        let defaults = Settings::default();
        assert_eq!(defaults.cli, CliSettings::default());
        assert!(defaults.cli.enabled && defaults.cli.traces && defaults.cli.game_logs);
        assert_eq!(defaults.cli.access, CliAccess::Full);
        let text = serde_json::to_string(&defaults).unwrap();
        assert!(!text.contains("cli"), "{text}");

        let mut changed = Settings::default();
        changed.cli.access = CliAccess::ReadOnly;
        changed.cli.traces = false;
        let value = serde_json::to_value(&changed).unwrap();
        assert_eq!(
            value["cli"],
            serde_json::json!({ "access": "read_only", "traces": false })
        );
        let back: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(back.cli, changed.cli);
        let partial: Settings =
            serde_json::from_str(r#"{"version": 1, "cli": {"enabled": false}}"#).unwrap();
        assert!(!partial.cli.enabled);
        assert!(partial.cli.traces && partial.cli.game_logs);
    }

    #[test]
    fn signing_in_as_the_launcher_is_on_by_default() {
        assert!(Settings::default().developer.sign_in_as_minecraft_launcher);
        let dir = tempfile::tempdir().unwrap();
        // No file, an empty file at version 1, and a file from before the field.
        let missing = dir.path().join("missing.json");
        assert!(
            load_or_default(&missing)
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
            assert!(loaded.developer.sign_in_as_minecraft_launcher, "{raw}");
        }
        // Left out of the file while on.
        let raw = serde_json::to_value(Settings::default()).unwrap();
        assert!(raw.get("developer").is_none(), "{raw}");
    }

    #[test]
    fn offline_accounts_are_off_by_default_and_saved_only_while_on() {
        assert!(!Settings::default().developer.offline_accounts);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        update(&path, |s| s.developer.offline_accounts = true).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["developer"], json!({ "offline_accounts": true }));
        assert!(load_or_default(&path).developer.offline_accounts);
        update(&path, |s| s.developer.offline_accounts = false).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(raw.get("developer").is_none(), "{raw}");
    }

    #[test]
    fn the_connect_service_is_saved_only_while_set() {
        assert_eq!(Settings::default().developer.connect_service, None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let dev = "https://connect-dev.lodestone.cc";
        update(&path, |s| s.developer.connect_service = Some(dev.into())).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["developer"], json!({ "connect_service": dev }));
        assert_eq!(
            load_or_default(&path).developer.connect_service.as_deref(),
            Some(dev)
        );
        update(&path, |s| s.developer.connect_service = None).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(raw.get("developer").is_none(), "{raw}");
    }

    #[test]
    fn developer_options_from_a_newer_build_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"version":1,"developer":{"offline_accounts":true,"from_a_newer_build":{"a":1}}}"#,
        )
        .unwrap();
        update(&path, |s| s.zoom = 1.25).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw["developer"],
            json!({ "offline_accounts": true, "from_a_newer_build": { "a": 1 } })
        );
        // and with every known option at its default, the unknown one still is
        update(&path, |s| s.developer.offline_accounts = false).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw["developer"],
            json!({ "from_a_newer_build": { "a": 1 } })
        );
    }

    #[test]
    fn update_keeps_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"version": 1, "memory": {"max_mb": 4096}}"#).unwrap();
        let settings = update(&path, |settings| {
            settings.developer.sign_in_as_minecraft_launcher = false;
        })
        .unwrap();
        assert!(!settings.developer.sign_in_as_minecraft_launcher);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            raw,
            json!({
                "version": 1,
                "memory": {"max_mb": 4096},
                "developer": {"sign_in_as_minecraft_launcher": false},
            })
        );
        assert!(
            !load_or_default(&path)
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
        update(&path, |s| s.developer.sign_in_as_minecraft_launcher = false).unwrap();
        update(&path, |s| {
            s.instance_defaults.max_ram = Some(6144);
            s.collapsed_groups.insert("Modded".into());
        })
        .unwrap();
        update(&path, |s| s.developer.sign_in_as_minecraft_launcher = true).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.instance_defaults.max_ram, Some(6144));
        assert!(loaded.collapsed_groups.contains("Modded"));
        assert!(loaded.developer.sign_in_as_minecraft_launcher);
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
    fn automatic_setup_is_on_unless_turned_off() {
        assert!(Settings::default().auto_configure);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"version": 1}"#).unwrap();
        assert!(load(&path).unwrap().auto_configure);
        update(&path, |s| s.auto_configure = false).unwrap();
        assert!(!load(&path).unwrap().auto_configure);
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw, json!({"version": 1, "auto_configure": false}));
        update(&path, |s| s.auto_configure = true).unwrap();
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
