//! Instances: a Minecraft version with its own game folder and settings.
//!
//! Each instance is a folder under `<data>/instances/` with `instance.json` (this
//! module's [`Instance`], a versioned document) and `minecraft/`, the game's working
//! directory, where saves, mods and options live. The folder name is the instance's
//! id and never changes; renaming changes only the name.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    game::{
        launch::{GameSettings, split_args},
        loaders::{LoaderChoice, LoaderKind},
    },
    settings::{self, InstanceDefaults},
    storage::{self, Document, Migration, StorageError},
};

/// The mod loader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[default]
    None,
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

impl Loader {
    pub fn kind(self) -> Option<LoaderKind> {
        match self {
            Loader::None => None,
            Loader::Fabric => Some(LoaderKind::Fabric),
            Loader::Quilt => Some(LoaderKind::Quilt),
            Loader::Forge => Some(LoaderKind::Forge),
            Loader::NeoForge => Some(LoaderKind::NeoForge),
        }
    }

    pub fn from_kind(kind: Option<LoaderKind>) -> Self {
        match kind {
            None => Loader::None,
            Some(LoaderKind::Fabric) => Loader::Fabric,
            Some(LoaderKind::Quilt) => Loader::Quilt,
            Some(LoaderKind::Forge) => Loader::Forge,
            Some(LoaderKind::NeoForge) => Loader::NeoForge,
        }
    }

    pub fn label(self) -> &'static str {
        self.kind().map_or("Vanilla", LoaderKind::label)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

/// An instance's own memory, when it overrides the global one. MiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub min: u32,
    pub max: u32,
}

/// The game window: a size (or the game's default) and fullscreen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub fullscreen: bool,
}

/// What `icon` is set to when the instance has its own `icon.png`.
pub const CUSTOM_ICON: &str = "custom";
/// The prefix of an icon from the launcher's library: `builtin:grass`.
pub const BUILTIN_ICON: &str = "builtin:";
/// The prefix of an emoji icon: `emoji:<tile colour as RRGGBB>:<emoji>`.
pub const EMOJI_ICON: &str = "emoji:";
/// What `background` is set to when the instance has its own `background.jpg`.
pub const CUSTOM_BACKGROUND: &str = "custom";
/// The prefix of `background` for a skin's banner: `skin:<skin id>/<banner id>`.
pub const SKIN_BANNER: &str = "skin:";
/// The instance's own Play background, in its folder.
pub const BACKGROUND_FILE: &str = "background.jpg";
/// Backgrounds are stored at most this size, keeping their aspect ratio.
pub const BACKGROUND_MAX: (u32, u32) = (2560, 1600);
/// The folder of the images the user added to the instance's slideshow.
pub const SLIDESHOW_DIR: &str = "backgrounds";

/// Where the Play tab's slideshow takes its images from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlideshowSource {
    /// The instance's own screenshots.
    Instance,
    /// The screenshots of every instance.
    AllInstances,
    /// The images added to the instance's slideshow, in its `backgrounds` folder.
    Custom,
}

/// What the Play tab shows behind the instance's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundMode {
    /// The launcher's artwork.
    Default,
    /// The instance's own `background.jpg`.
    Image,
    /// A banner from a skin ([`Instance::skin_banner`]).
    Banner,
    /// Images in turn, crossfading.
    Slideshow(SlideshowSource),
}
/// The instance's own icon, in its folder.
pub const ICON_FILE: &str = "icon.png";
/// The size custom icons are stored at.
pub const ICON_SIZE: u32 = 128;

/// `instance.json`.
///
/// The settings in the `Option`s override the launcher's global defaults
/// ([`InstanceDefaults`]) when set; `None` follows them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// [`CUSTOM_ICON`] for the instance's own `icon.png`, an icon from the library
    /// ([`BUILTIN_ICON`]) or an emoji ([`EMOJI_ICON`]); `None` for the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// [`CUSTOM_BACKGROUND`] for the instance's own Play background; `None` for the
    /// launcher's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Where the Play tab's slideshow takes its images from, when it shows one; it
    /// wins over `background`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slideshow: Option<SlideshowSource>,
    /// The Play button's colour for this instance, over the skin's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_color: Option<crate::ui_skin::Rgb>,
    /// The accent colour while this instance is selected, over the skin's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<crate::ui_skin::Rgb>,
    /// The sidebar group the instance is in. Without one the sidebar puts it in an
    /// automatic group ([`AutoGroup`]) when that setting is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The launcher the instance was imported from, by name ("Prism Launcher"), when
    /// it was imported from another launcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<String>,
    pub game_version: String,
    #[serde(default)]
    pub loader: Loader,
    /// See [`LoaderChoice::version`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    /// Unix seconds.
    pub created: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_played: Option<u64>,
    /// Seconds played in total.
    #[serde(default)]
    pub play_time: u64,
    /// A `java` binary to use instead of Mojang's runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_override: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
    /// Extra JVM arguments, as typed; they replace the global ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
    /// Extra game arguments, as typed; they replace the global ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_args: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowSettings>,
    /// Environment variables for the game; they replace the global ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    /// "Beta and alpha updates": Modrinth content may update to pre-release
    /// versions. Off, a file only updates to one at least as stable as it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub prerelease_updates: bool,
    /// The memory the instance's modpack recommends, MiB (a CurseForge manifest's
    /// `recommendedRam`), which automatic setup gives it at least, as far as the
    /// computer can spare. Updated with the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_memory: Option<u32>,
    /// What automatic setup ([`crate::configurator`]) chose for the instance; `None`
    /// until it has set it up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_config: Option<AutoConfig>,
    /// Whether the instance runs under Rosetta 2 on this Apple silicon Mac, and whether
    /// the player was told; `None` until the game was first prepared on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rosetta: Option<RosettaState>,
}

/// Rosetta 2 and an instance, on an Apple silicon Mac. Versions before 1.13 run in an
/// Intel Java under Rosetta, because Mojang made no Apple silicon build of their
/// graphics library (LWJGL 2); the launcher never does that without saying so.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosettaState {
    /// The Minecraft version the game was last prepared to run under Rosetta: the
    /// instance says it runs under Rosetta while its version is still this one. `None`
    /// when it last ran natively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether the player was told before the instance's first game under Rosetta.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub told: bool,
}

/// What automatic setup chose for an instance. While a setting still has the value it
/// chose, it keeps choosing it (memory follows the mods that are added); once the user
/// changes it, it is theirs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoConfig {
    /// The memory it gave the instance last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
    /// The JVM arguments it gave the instance last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jvm_args: Option<String>,
    /// The game's options were set up, or the instance already had its own: they are
    /// never touched again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub options: bool,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: None,
            background: None,
            slideshow: None,
            play_color: None,
            accent: None,
            group: None,
            imported_from: None,
            game_version: String::new(),
            loader: Loader::None,
            loader_version: None,
            created: 0,
            last_played: None,
            play_time: 0,
            java_override: None,
            memory: None,
            jvm_args: None,
            game_args: None,
            window: None,
            env: None,
            prerelease_updates: false,
            recommended_memory: None,
            auto_config: None,
            rosetta: None,
        }
    }
}

impl Instance {
    /// Whether the instance's version was last prepared to run under Rosetta 2.
    pub fn runs_under_rosetta(&self) -> bool {
        self.rosetta
            .as_ref()
            .and_then(|rosetta| rosetta.version.as_deref())
            == Some(self.game_version.as_str())
    }
}

impl Document for Instance {
    const VERSION: u32 = 2;
    /// Version 0 is a file without a `"version"` key; its fields carry over. Version
    /// 2 makes the settings overrides of the global defaults.
    const MIGRATIONS: &'static [Migration] = &[Ok, migrate_v1];
}

/// The maximum memory M3 gave new instances: a quarter of the memory, from 2 to 4 GB.
/// M3 read the memory size on macOS only and assumed 8 GB elsewhere, so this does too:
/// the real size on Windows or Linux would turn M3's default into a custom setting.
fn m3_default_max_ram_mb() -> u64 {
    let total = if cfg!(target_os = "macos") {
        crate::game::platform::total_memory()
    } else {
        None
    };
    let total_mb = total.map_or(8192, |bytes| bytes / (1024 * 1024));
    (total_mb / 4).clamp(2048, 4096)
}

/// Version 1 stored every setting. Values that were the defaults M3 filled in become
/// "use the global setting"; the others stay as the instance's own.
fn migrate_v1(mut fields: Map<String, Value>) -> anyhow::Result<Map<String, Value>> {
    let min = fields.remove("min_ram").and_then(|v| v.as_u64());
    let max = fields.remove("max_ram").and_then(|v| v.as_u64());
    if let (Some(min), Some(max)) = (min, max) {
        let old_default_max = m3_default_max_ram_mb();
        if min != u64::from(settings::DEFAULT_MIN_RAM_MB) || max != old_default_max {
            fields.insert("memory".into(), serde_json::json!({"min": min, "max": max}));
        }
    }
    for key in ["jvm_args", "game_args"] {
        if fields
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|text| text.trim().is_empty())
        {
            fields.remove(key);
        }
    }
    let resolution = fields.remove("resolution").filter(|v| !v.is_null());
    let fullscreen = fields
        .remove("fullscreen")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if resolution.is_some() || fullscreen {
        let mut window = Map::new();
        if let Some(resolution) = resolution {
            window.insert("resolution".into(), resolution);
        }
        window.insert("fullscreen".into(), fullscreen.into());
        fields.insert("window".into(), Value::Object(window));
    }
    Ok(fields)
}

impl Instance {
    /// What the Play tab shows behind the name. The slideshow wins over the image.
    pub fn background_mode(&self) -> BackgroundMode {
        match (self.slideshow, self.background.as_deref()) {
            (Some(source), _) => BackgroundMode::Slideshow(source),
            (None, Some(CUSTOM_BACKGROUND)) => BackgroundMode::Image,
            (None, Some(other)) if other.starts_with(SKIN_BANNER) => BackgroundMode::Banner,
            (None, _) => BackgroundMode::Default,
        }
    }

    /// The skin and banner the instance's Play background is, when it is a skin's
    /// banner.
    pub fn skin_banner(&self) -> Option<(&str, &str)> {
        self.background
            .as_deref()?
            .strip_prefix(SKIN_BANNER)?
            .split_once('/')
    }

    /// The instance's loader and its version, if it has a loader.
    pub fn loader_choice(&self) -> Option<LoaderChoice> {
        Some(LoaderChoice {
            kind: self.loader.kind()?,
            version: self.loader_version.clone()?,
        })
    }

    /// "Fabric 0.16.14", "Forge 47.4.28" or "Vanilla".
    pub fn loader_label(&self) -> String {
        match (self.loader.kind(), &self.loader_version) {
            (Some(LoaderKind::Forge), Some(version)) => format!(
                "Forge {}",
                crate::game::loaders::forge::forge_display(version, &self.game_version)
            ),
            (Some(kind), Some(version)) => format!("{} {version}", kind.label()),
            (Some(kind), None) => kind.label().to_string(),
            (None, _) => "Vanilla".to_string(),
        }
    }

    /// The memory the game gets: the instance's, or the global one.
    pub fn memory(&self, defaults: &InstanceDefaults, total_memory: Option<u64>) -> (u32, u32) {
        match self.memory {
            Some(memory) => (memory.min.min(memory.max), memory.max),
            None => defaults.memory(total_memory),
        }
    }

    /// The settings the launch command uses: each of the instance's overrides, or the
    /// global default.
    pub fn game_settings(
        &self,
        defaults: &InstanceDefaults,
        total_memory: Option<u64>,
    ) -> GameSettings {
        let (min, max) = self.memory(defaults, total_memory);
        let window = self.window.unwrap_or_else(|| defaults.window());
        GameSettings {
            min_ram_mb: min,
            max_ram_mb: max,
            jvm_args: split_args(self.jvm_args.as_deref().unwrap_or(&defaults.jvm_args)),
            game_args: split_args(self.game_args.as_deref().unwrap_or(&defaults.game_args)),
            resolution: window.resolution.map(|r| (r.width, r.height)),
            fullscreen: window.fullscreen,
            env: self
                .env
                .as_ref()
                .unwrap_or(&defaults.env)
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        }
    }
}

/// Parses environment variables typed as `NAME=value`, separated by spaces or new
/// lines; a value with spaces is quoted (`NAME="a b"`). Words starting with `#` begin
/// a comment that runs to the end of the line.
pub fn parse_env(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut env = BTreeMap::new();
    for line in text.lines() {
        let line = match line.trim_start().find('#') {
            Some(0) => continue,
            _ => line,
        };
        for word in split_args(line) {
            let (name, value) = word
                .split_once('=')
                .ok_or_else(|| format!("{word:?}: write variables as NAME=value."))?;
            let valid = name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                return Err(format!(
                    "{name:?} is not a valid variable name (letters, digits and _)."
                ));
            }
            env.insert(name.to_string(), value.to_string());
        }
    }
    Ok(env)
}

/// Environment variables as [`parse_env`] reads them, on one line.
pub fn format_env(env: &BTreeMap<String, String>) -> String {
    env.iter()
        .map(|(name, value)| {
            if value.is_empty()
                || value.contains(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            {
                format!("{name}=\"{}\"", value.replace('"', "'"))
            } else {
                format!("{name}={value}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The groups the sidebar puts instances without a group of their own in, in the
/// order they are listed: Servers before the groups the user named, the others
/// after them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AutoGroup {
    /// Linked to a server: one on a Lodestone core, or a friend's shared server
    /// ([`crate::cores::link`]), whatever its loader.
    Servers,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    Vanilla,
    /// Imported from another launcher, whatever its loader.
    Imported,
}

impl AutoGroup {
    pub const ALL: [AutoGroup; 7] = [
        AutoGroup::Servers,
        AutoGroup::Fabric,
        AutoGroup::Forge,
        AutoGroup::NeoForge,
        AutoGroup::Quilt,
        AutoGroup::Vanilla,
        AutoGroup::Imported,
    ];

    /// The instance's automatic group: Servers for one `linked` to a server, then
    /// Imported for one imported from another launcher, else its loader's.
    pub fn of(instance: &Instance, linked: bool) -> Self {
        if linked {
            return AutoGroup::Servers;
        }
        if instance.imported_from.is_some() {
            return AutoGroup::Imported;
        }
        match instance.loader {
            Loader::None => AutoGroup::Vanilla,
            Loader::Fabric => AutoGroup::Fabric,
            Loader::Forge => AutoGroup::Forge,
            Loader::NeoForge => AutoGroup::NeoForge,
            Loader::Quilt => AutoGroup::Quilt,
        }
    }

    /// The group's name, which is also its heading.
    pub fn name(self) -> &'static str {
        match self {
            AutoGroup::Servers => "Servers",
            AutoGroup::Fabric => "Fabric (modded)",
            AutoGroup::Forge => "Forge (modded)",
            AutoGroup::NeoForge => "NeoForge (modded)",
            AutoGroup::Quilt => "Quilt (modded)",
            AutoGroup::Vanilla => "Vanilla",
            AutoGroup::Imported => "Imported",
        }
    }
}

/// The instances by sidebar group, each group keeping the list's order: those
/// without a group first (none when `automatic`), then the groups the user named in
/// name order (ignoring case), then, when `automatic`, the [`AutoGroup`]s in their
/// order for the instances without a group of their own. The Servers group, of the
/// instances `linked` to a server, comes before the named groups.
///
/// A group the user named like an automatic group ("vanilla") is that group: the
/// instances it would get join it.
pub fn grouped(
    instances: &[Instance],
    automatic: bool,
    linked: impl Fn(&Instance) -> bool,
) -> Vec<(Option<String>, Vec<&Instance>)> {
    let mut groups: Vec<(Option<String>, Vec<&Instance>)> = Vec::new();
    if !automatic {
        let ungrouped: Vec<&Instance> = instances
            .iter()
            .filter(|i| group_name(i).is_none())
            .collect();
        if !ungrouped.is_empty() {
            groups.push((None, ungrouped));
        }
    }
    let mut names: Vec<&str> = instances.iter().filter_map(group_name).collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    let named = |name: &str| names.iter().copied().find(|n| n.eq_ignore_ascii_case(name));
    let auto_of = |i: &Instance| AutoGroup::of(i, linked(i));
    // An automatic group, unless a group the user named takes its instances.
    let auto_group = |auto: AutoGroup| -> Option<(Option<String>, Vec<&Instance>)> {
        if named(auto.name()).is_some() {
            return None;
        }
        let members: Vec<&Instance> = instances
            .iter()
            .filter(|i| group_name(i).is_none() && auto_of(i) == auto)
            .collect();
        (!members.is_empty()).then(|| (Some(auto.name().to_string()), members))
    };
    if automatic {
        groups.extend(auto_group(AutoGroup::Servers));
    }
    for name in &names {
        groups.push((
            Some(name.to_string()),
            instances
                .iter()
                .filter(|i| match group_name(i) {
                    Some(group) => group == *name,
                    None => automatic && named(auto_of(i).name()) == Some(*name),
                })
                .collect(),
        ));
    }
    if automatic {
        for auto in AutoGroup::ALL {
            if auto != AutoGroup::Servers {
                groups.extend(auto_group(auto));
            }
        }
    }
    groups
}

/// The group the sidebar lists the instance in: its own, or its automatic one when
/// `automatic` (Servers when it is `linked` to a server).
pub fn group_of(instance: &Instance, automatic: bool, linked: bool) -> Option<String> {
    match group_name(instance) {
        Some(name) => Some(name.to_string()),
        None => automatic.then(|| AutoGroup::of(instance, linked).name().to_string()),
    }
}

/// The instance played most recently, if any has been played.
pub fn last_played(instances: &[Instance]) -> Option<&Instance> {
    instances
        .iter()
        .filter(|i| i.last_played.is_some())
        .reduce(|best, i| {
            if i.last_played > best.last_played {
                i
            } else {
                best
            }
        })
}

fn group_name(instance: &Instance) -> Option<&str> {
    instance
        .group
        .as_deref()
        .map(str::trim)
        .filter(|g| !g.is_empty())
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("give the instance a name")]
    EmptyName,
    #[error("there is no instance {0}")]
    NotFound(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("couldn't move {path} to {trash}: {reason}", trash = crate::wording::TRASH)]
    Trash { path: PathBuf, reason: String },
    #[error("couldn't use {path} as the icon: {reason}")]
    Icon { path: PathBuf, reason: String },
    #[error("couldn't use {path} as the background: {reason}")]
    Background { path: PathBuf, reason: String },
    #[error("{0:?} isn't an icon the launcher knows")]
    IconName(String),
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> InstanceError + '_ {
    move |source| InstanceError::Io {
        path: path.to_owned(),
        source,
    }
}

/// A problem with one instance folder, found while listing.
#[derive(Debug)]
pub struct BrokenInstance {
    pub dir: PathBuf,
    pub error: StorageError,
}

/// The instances in a data directory.
#[derive(Clone, Debug)]
pub struct InstanceStore {
    root: PathBuf,
}

/// The longest name kept; longer names are cut.
const MAX_NAME_CHARS: usize = 64;

/// Puts instances in the order [`InstanceStore::list`] returns them: most recently
/// played first, then newest first. For a list changed in place after one instance
/// was saved.
pub fn sort_listed(instances: &mut [Instance]) {
    instances.sort_by(|a, b| {
        b.last_played
            .unwrap_or(0)
            .cmp(&a.last_played.unwrap_or(0))
            .then(b.created.cmp(&a.created))
            .then(a.name.cmp(&b.name))
    });
}

impl InstanceStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// The game's working directory.
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.dir(id).join("minecraft")
    }

    /// Where old-style natives are extracted for each launch.
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.dir(id).join("natives")
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir(id).join("instance.json")
    }

    /// Every instance, most recently played first, then newest first. Folders whose
    /// `instance.json` can't be read are returned separately.
    pub fn list(&self) -> (Vec<Instance>, Vec<BrokenInstance>) {
        let mut instances = Vec::new();
        let mut broken = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return (instances, broken);
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let Some(id) = dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if id.starts_with('.') || !dir.join("instance.json").is_file() {
                continue;
            }
            match storage::load::<Instance>(&dir.join("instance.json")) {
                Ok(loaded) => {
                    let mut instance = loaded.document;
                    // The folder name is the id, whatever the file says.
                    instance.id = id.to_string();
                    instances.push(instance);
                }
                Err(error) => broken.push(BrokenInstance { dir, error }),
            }
        }
        sort_listed(&mut instances);
        (instances, broken)
    }

    pub fn get(&self, id: &str) -> Result<Instance, InstanceError> {
        check_id(id)?;
        let path = self.file(id);
        if !path.is_file() {
            return Err(InstanceError::NotFound(id.to_string()));
        }
        let mut instance = storage::load::<Instance>(&path)?.document;
        instance.id = id.to_string();
        Ok(instance)
    }

    /// Creates an instance of `game_version` (with `loader`, if any) named `name`,
    /// following the global settings.
    pub fn create(
        &self,
        name: &str,
        game_version: &str,
        loader: Option<&LoaderChoice>,
    ) -> Result<Instance, InstanceError> {
        let name = self.unique_name(&clean_name(name)?);
        let id = self.unused_id(&name);
        let instance = Instance {
            id: id.clone(),
            name,
            game_version: game_version.to_string(),
            loader: Loader::from_kind(loader.map(|l| l.kind)),
            loader_version: loader.map(|l| l.version.clone()),
            created: now(),
            ..Instance::default()
        };
        let game_dir = self.game_dir(&id);
        fs::create_dir_all(&game_dir).map_err(io_error(&game_dir))?;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Copies an instance: its settings and its game folder (worlds, mods, options),
    /// under a new name. The copy starts with no play time. The files are copied
    /// into a hidden folder first and renamed into place, so a failed copy leaves
    /// nothing behind.
    pub fn duplicate(&self, id: &str) -> Result<Instance, InstanceError> {
        let original = self.get(id)?;
        let mut name: String = format!("{} (copy)", original.name);
        if name.chars().count() > MAX_NAME_CHARS {
            name = name.chars().take(MAX_NAME_CHARS).collect();
        }
        let name = self.unique_name(&clean_name(&name)?);
        let new_id = self.unused_id(&name);
        let staging = self.root.join(format!(".{new_id}.copying"));
        let _ = fs::remove_dir_all(&staging);
        let result = (|| {
            copy_tree(&self.dir(id), &staging, &|relative| {
                // Natives are extracted again at launch.
                relative == Path::new("natives")
            })?;
            fs::create_dir_all(staging.join("minecraft")).map_err(io_error(&staging))?;
            let copy = Instance {
                id: new_id.clone(),
                name,
                created: now(),
                last_played: None,
                play_time: 0,
                ..original.clone()
            };
            storage::save(&staging.join("instance.json"), &copy)?;
            crate::fsx::rename(&staging, self.dir(&new_id)).map_err(io_error(&staging))?;
            Ok(copy)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    /// A new hidden folder next to the instances (with an empty game folder) to build an
    /// instance in; [`InstanceStore::commit_staged`] makes it one. The list skips it.
    pub fn new_staging(&self) -> Result<PathBuf, InstanceError> {
        let staging = self.root.join(format!(
            ".lodestone-staging-{:016x}",
            u64::from_le_bytes(crate::random_bytes())
        ));
        let game = staging.join("minecraft");
        fs::create_dir_all(&game).map_err(io_error(&game))?;
        Ok(staging)
    }

    /// Makes the folder from [`InstanceStore::new_staging`] the instance `instance`, under
    /// an unused id from its name, by renaming it into place.
    pub fn commit_staged(
        &self,
        staging: &Path,
        mut instance: Instance,
    ) -> Result<Instance, InstanceError> {
        instance.name = self.unique_name(&clean_name(&instance.name)?);
        instance.id = self.unused_id(&instance.name);
        storage::save(&staging.join("instance.json"), &instance)?;
        let dir = self.dir(&instance.id);
        crate::fsx::rename(staging, &dir).map_err(io_error(staging))?;
        Ok(instance)
    }

    /// The instance's own icon, if it has one.
    pub fn icon_path(&self, instance: &Instance) -> Option<PathBuf> {
        let path = self.dir(&instance.id).join(ICON_FILE);
        (instance.icon.as_deref() == Some(CUSTOM_ICON) && path.is_file()).then_some(path)
    }

    /// Makes the image at `source` (PNG or JPEG) the instance's icon: cropped to a
    /// square, scaled to [`ICON_SIZE`] and saved as the instance's `icon.png`.
    pub fn set_icon(&self, id: &str, source: &Path) -> Result<Instance, InstanceError> {
        self.write_icon(id, source)?;
        self.use_custom_icon(id)
    }

    /// The slow half of [`InstanceStore::set_icon`]: reads, crops and scales the image
    /// and writes the instance's `icon.png`, without changing its `instance.json`.
    /// [`InstanceStore::use_custom_icon`] then makes it the icon.
    pub fn write_icon(&self, id: &str, source: &Path) -> Result<(), InstanceError> {
        check_id(id)?;
        if !self.file(id).is_file() {
            return Err(InstanceError::NotFound(id.to_string()));
        }
        let icon_error = |reason: String| InstanceError::Icon {
            path: source.to_owned(),
            reason,
        };
        let bytes = fs::read(source).map_err(|err| icon_error(err.to_string()))?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(icon_error("the image is larger than 32 MB".into()));
        }
        let png = icon_png(&bytes).map_err(icon_error)?;
        let path = self.dir(id).join(ICON_FILE);
        storage::write_atomic(&path, &png).map_err(io_error(&path))?;
        Ok(())
    }

    /// Makes the `icon.png` that [`InstanceStore::write_icon`] wrote the instance's icon.
    pub fn use_custom_icon(&self, id: &str) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.icon = Some(CUSTOM_ICON.to_string());
        self.save(&instance)?;
        Ok(instance)
    }

    /// Goes back to the default icon.
    pub fn clear_icon(&self, id: &str) -> Result<Instance, InstanceError> {
        self.set_icon_name(id, None)
    }

    /// Uses an icon from the library or an emoji (see [`Instance::icon`]), or the
    /// default for `None`, and removes the instance's own `icon.png`.
    pub fn set_icon_name(&self, id: &str, icon: Option<&str>) -> Result<Instance, InstanceError> {
        if let Some(icon) = icon
            && !valid_icon_name(icon)
        {
            return Err(InstanceError::IconName(icon.to_string()));
        }
        let mut instance = self.get(id)?;
        remove_if_present(&self.dir(id).join(ICON_FILE))?;
        instance.icon = icon.map(str::to_string);
        self.save(&instance)?;
        Ok(instance)
    }

    /// The instance's own Play background, if it has one.
    pub fn background_path(&self, instance: &Instance) -> Option<PathBuf> {
        let path = self.dir(&instance.id).join(BACKGROUND_FILE);
        (instance.background.as_deref() == Some(CUSTOM_BACKGROUND) && path.is_file())
            .then_some(path)
    }

    /// Makes the image at `source` (PNG, JPEG, WebP or GIF) the instance's Play
    /// background: scaled down to fit [`BACKGROUND_MAX`] and saved as its
    /// `background.jpg`.
    pub fn set_background(&self, id: &str, source: &Path) -> Result<Instance, InstanceError> {
        self.write_background(id, source)?;
        self.use_custom_background(id)
    }

    /// The slow half of [`InstanceStore::set_background`]: reads and scales the image
    /// and writes the instance's `background.jpg`, without changing its
    /// `instance.json`. [`InstanceStore::use_custom_background`] then shows it.
    pub fn write_background(&self, id: &str, source: &Path) -> Result<(), InstanceError> {
        check_id(id)?;
        if !self.file(id).is_file() {
            return Err(InstanceError::NotFound(id.to_string()));
        }
        let error = |reason: String| InstanceError::Background {
            path: source.to_owned(),
            reason,
        };
        let bytes = fs::read(source).map_err(|err| error(err.to_string()))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(error("the image is larger than 64 MB".into()));
        }
        let jpeg = background_jpeg(&bytes).map_err(error)?;
        let path = self.dir(id).join(BACKGROUND_FILE);
        storage::write_atomic(&path, &jpeg).map_err(io_error(&path))?;
        Ok(())
    }

    /// Makes the `background.jpg` that [`InstanceStore::write_background`] wrote the
    /// instance's Play background.
    pub fn use_custom_background(&self, id: &str) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.background = Some(CUSTOM_BACKGROUND.to_string());
        self.save(&instance)?;
        Ok(instance)
    }

    /// Makes banner `banner` of skin `skin` the instance's Play background. The
    /// banner is kept by id, so it follows the skin's updates.
    pub fn set_skin_banner(
        &self,
        id: &str,
        skin: &str,
        banner: &str,
    ) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        remove_if_present(&self.dir(id).join(BACKGROUND_FILE))?;
        instance.background = Some(format!("{SKIN_BANNER}{skin}/{banner}"));
        instance.slideshow = None;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Goes back to the launcher's Play background. The slideshow's own images stay,
    /// for when it is chosen again.
    pub fn clear_background(&self, id: &str) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        remove_if_present(&self.dir(id).join(BACKGROUND_FILE))?;
        instance.background = None;
        instance.slideshow = None;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Shows a slideshow from `source` behind the name, or, for `None`, the
    /// instance's own image again (or the launcher's, without one).
    pub fn set_slideshow(
        &self,
        id: &str,
        source: Option<SlideshowSource>,
    ) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.slideshow = source;
        self.save(&instance)?;
        Ok(instance)
    }

    /// The images added to the instance's slideshow, by name.
    pub fn slideshow_images(&self, id: &str) -> Vec<PathBuf> {
        let dir = self.dir(id).join(SLIDESHOW_DIR);
        let mut images: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "jpg"))
            .collect();
        images.sort();
        images
    }

    /// Adds the image at `source` (PNG, JPEG, WebP or GIF) to the instance's slideshow,
    /// scaled down like a background. Returns where it was saved.
    pub fn add_slideshow_image(&self, id: &str, source: &Path) -> Result<PathBuf, InstanceError> {
        check_id(id)?;
        let error = |reason: String| InstanceError::Background {
            path: source.to_owned(),
            reason,
        };
        let bytes = fs::read(source).map_err(|err| error(err.to_string()))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(error("the image is larger than 64 MB".into()));
        }
        let jpeg = background_jpeg(&bytes).map_err(error)?;
        let dir = self.dir(id).join(SLIDESHOW_DIR);
        fs::create_dir_all(&dir).map_err(io_error(&dir))?;
        // Named by when it was added, so the set keeps that order.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let mut path = dir.join(format!("{stamp}.jpg"));
        let mut n = 1;
        while path.exists() {
            path = dir.join(format!("{stamp}-{n}.jpg"));
            n += 1;
        }
        storage::write_atomic(&path, &jpeg).map_err(io_error(&path))?;
        Ok(path)
    }

    /// Removes one of the slideshow's own images; `name` is its file name.
    pub fn remove_slideshow_image(&self, id: &str, name: &str) -> Result<(), InstanceError> {
        check_id(id)?;
        if name.contains(['/', '\\']) || name.starts_with('.') || !name.ends_with(".jpg") {
            return Err(InstanceError::Background {
                path: PathBuf::from(name),
                reason: "not one of the slideshow's images".into(),
            });
        }
        remove_if_present(&self.dir(id).join(SLIDESHOW_DIR).join(name))
    }

    /// Writes the instance's `instance.json`.
    pub fn save(&self, instance: &Instance) -> Result<(), InstanceError> {
        check_id(&instance.id)?;
        storage::save(&self.file(&instance.id), instance)?;
        Ok(())
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.name = clean_name(name)?;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Records that a game started at `started`, so it counts as played even if it
    /// crashes or the launcher quits before it exits.
    pub fn record_start(&self, id: &str, started: u64) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.last_played = Some(started);
        self.save(&instance)?;
        Ok(instance)
    }

    /// Records whether the game was just prepared to run under Rosetta 2 (on an Apple
    /// silicon Mac). Returns the instance, and whether this is its first game under
    /// Rosetta, which the player is to be told about; it counts as told from now on.
    pub fn record_rosetta(
        &self,
        id: &str,
        under_rosetta: bool,
    ) -> Result<(Instance, bool), InstanceError> {
        let mut instance = self.get(id)?;
        let mut state = instance.rosetta.clone().unwrap_or_default();
        state.version = under_rosetta.then(|| instance.game_version.clone());
        let tell = under_rosetta && !state.told;
        state.told |= under_rosetta;
        if instance.rosetta.as_ref() != Some(&state) {
            instance.rosetta = Some(state);
            self.save(&instance)?;
        }
        Ok((instance, tell))
    }

    /// Records a finished game: when it started and how long it ran.
    pub fn record_play(
        &self,
        id: &str,
        started: u64,
        seconds: u64,
    ) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        instance.last_played = Some(started);
        instance.play_time = instance.play_time.saturating_add(seconds);
        self.save(&instance)?;
        Ok(instance)
    }

    /// Moves the instance's folder to the Trash, so it can be restored.
    pub fn delete(&self, id: &str) -> Result<(), InstanceError> {
        self.delete_with(id, |path| {
            trash::delete(path).map_err(|err| err.to_string())
        })
    }

    /// Like [`InstanceStore::delete`], with `remove` doing the removal. Tests pass
    /// one that does not touch the user's Trash.
    pub fn delete_with(
        &self,
        id: &str,
        remove: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), InstanceError> {
        check_id(id)?;
        let dir = self.dir(id);
        if !dir.join("instance.json").is_file() {
            return Err(InstanceError::NotFound(id.to_string()));
        }
        remove(&dir).map_err(|reason| InstanceError::Trash { path: dir, reason })
    }

    /// `name`, or `name (2)`, `name (3)`, … when another instance already has that
    /// name (ignoring case), so a new instance can always be told apart from the others
    /// in the sidebar and in pickers. Renaming an instance yourself is not changed.
    fn unique_name(&self, name: &str) -> String {
        let taken: Vec<String> = self
            .list()
            .0
            .into_iter()
            .map(|instance| instance.name.to_lowercase())
            .collect();
        (1..)
            .map(|n| {
                if n == 1 {
                    return name.to_string();
                }
                let suffix = format!(" ({n})");
                let room = MAX_NAME_CHARS.saturating_sub(suffix.chars().count());
                let base: String = name.chars().take(room).collect();
                format!("{}{suffix}", base.trim_end())
            })
            .find(|candidate| !taken.contains(&candidate.to_lowercase()))
            .expect("an unused name")
    }

    /// A folder name from `name`: lowercase letters, digits and dashes, with a
    /// number added if it is taken.
    fn unused_id(&self, name: &str) -> String {
        let mut base: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        while base.contains("--") {
            base = base.replace("--", "-");
        }
        let base = base.trim_matches('-');
        let base = if base.is_empty() { "instance" } else { base };
        let base: String = base.chars().take(40).collect();
        (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|id| !self.dir(id).exists())
            .expect("an unused id")
    }
}

/// `bytes` (a PNG or JPEG) as a square PNG of [`ICON_SIZE`] pixels.
/// Whether `icon` is a library icon or an emoji in the form [`Instance::icon`] takes.
/// The launcher decides which library names it draws; this keeps out what isn't one.
pub fn valid_icon_name(icon: &str) -> bool {
    if icon.len() > 64 || icon.chars().any(char::is_control) {
        return false;
    }
    if let Some(name) = icon.strip_prefix(BUILTIN_ICON) {
        return !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    }
    if let Some(rest) = icon.strip_prefix(EMOJI_ICON) {
        let Some((color, emoji)) = rest.split_once(':') else {
            return false;
        };
        return color.len() == 6
            && color.chars().all(|c| c.is_ascii_hexdigit())
            && !emoji.is_empty()
            && emoji.chars().count() <= 8;
    }
    false
}

fn remove_if_present(path: &Path) -> Result<(), InstanceError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(InstanceError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

/// The image in `bytes` as a JPEG no larger than [`BACKGROUND_MAX`].
pub fn background_jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let not_an_image =
        |err: image::ImageError| format!("it is not a PNG, JPEG, WebP or GIF image ({err})");
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    // Refuses images that would take too much memory to decode.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(not_an_image)?;
    if image.width() == 0 || image.height() == 0 {
        return Err("the image is empty".into());
    }
    let (max_w, max_h) = BACKGROUND_MAX;
    let image = if image.width() > max_w || image.height() > max_h {
        image.resize(max_w, max_h, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let rgb = image::DynamicImage::ImageRgb8(image.to_rgb8());
    let mut jpeg = std::io::Cursor::new(Vec::new());
    rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut jpeg, 88,
    ))
    .map_err(|err| err.to_string())?;
    Ok(jpeg.into_inner())
}

pub fn icon_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| format!("it is not a PNG or JPEG image ({err})"))?;
    let side = image.width().min(image.height());
    if side == 0 {
        return Err("the image is empty".into());
    }
    let square = image.crop_imm(
        (image.width() - side) / 2,
        (image.height() - side) / 2,
        side,
        side,
    );
    let icon = square.resize_exact(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3);
    let mut png = std::io::Cursor::new(Vec::new());
    icon.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|err| err.to_string())?;
    Ok(png.into_inner())
}

/// Copies the folder `from` to `to` (which must not exist), skipping what `skip`
/// says (by path relative to `from`) and symbolic links.
fn copy_tree(from: &Path, to: &Path, skip: &dyn Fn(&Path) -> bool) -> Result<(), InstanceError> {
    fn walk(
        root: &Path,
        dir: &Path,
        to: &Path,
        skip: &dyn Fn(&Path) -> bool,
    ) -> Result<(), InstanceError> {
        let target = to.join(dir.strip_prefix(root).unwrap_or(dir));
        fs::create_dir_all(&target).map_err(io_error(&target))?;
        for entry in fs::read_dir(dir).map_err(io_error(dir))? {
            let entry = entry.map_err(io_error(dir))?;
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if skip(relative) {
                continue;
            }
            let kind = entry.file_type().map_err(io_error(&path))?;
            if kind.is_dir() {
                walk(root, &path, to, skip)?;
            } else if kind.is_file() {
                let dest = to.join(relative);
                fs::copy(&path, &dest).map_err(io_error(&dest))?;
            }
        }
        Ok(())
    }
    walk(from, from, to, skip)
}

/// Ids are folder names this store created; refuse anything else.
fn check_id(id: &str) -> Result<(), InstanceError> {
    let valid = !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if valid {
        Ok(())
    } else {
        Err(InstanceError::NotFound(id.to_string()))
    }
}

fn clean_name(name: &str) -> Result<String, InstanceError> {
    let name: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    if name.is_empty() {
        Err(InstanceError::EmptyName)
    } else {
        Ok(name)
    }
}

/// The current time in Unix seconds.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {

    #[test]
    fn icon_names_are_library_icons_or_emoji() {
        assert!(valid_icon_name("builtin:grass"));
        assert!(valid_icon_name("builtin:diamond-ore"));
        assert!(valid_icon_name("emoji:3DAE5E:🌲"));
        assert!(valid_icon_name("emoji:1d8eb2:🧑‍🚀"));
        assert!(!valid_icon_name("builtin:"));
        assert!(!valid_icon_name("builtin:../icon"));
        assert!(!valid_icon_name("emoji:green:🌲"));
        assert!(!valid_icon_name("emoji:3DAE5E:"));
        assert!(!valid_icon_name("emoji:3DAE5E:a\nb"));
        assert!(!valid_icon_name("custom"));
        assert!(!valid_icon_name("https://example.com/icon.png"));
    }

    #[test]
    fn named_icons_and_backgrounds_are_saved() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        let instance = store.create("Pixel", "1.20.1", None).unwrap();
        let source = dir.path().join("photo.png");
        image::RgbImage::from_pixel(3000, 1000, image::Rgb([200, 210, 220]))
            .save(&source)
            .unwrap();

        store.set_icon(&instance.id, &source).unwrap();
        let named = store
            .set_icon_name(&instance.id, Some("builtin:grass"))
            .unwrap();
        assert_eq!(named.icon.as_deref(), Some("builtin:grass"));
        assert!(!store.dir(&instance.id).join(ICON_FILE).exists());
        assert!(
            store
                .set_icon_name(&instance.id, Some("builtin:../x"))
                .is_err()
        );

        let with_background = store.set_background(&instance.id, &source).unwrap();
        let path = store.background_path(&with_background).unwrap();
        let saved = image::open(&path).unwrap();
        assert_eq!((saved.width(), saved.height()), (2560, 853));
        let cleared = store.clear_background(&instance.id).unwrap();
        assert_eq!(cleared.background, None);
        assert!(!path.exists());
    }

    /// The player is told once per instance before its first game under Rosetta, and
    /// the instance says it runs under Rosetta only while its version is the one that
    /// did.
    #[test]
    fn rosetta_is_told_once_and_follows_the_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        let instance = store.create("Old", "1.12.2", None).unwrap();
        assert!(!instance.runs_under_rosetta());
        let (native, tell) = store.record_rosetta(&instance.id, false).unwrap();
        assert!(!tell);
        assert!(!native.runs_under_rosetta());
        let (first, tell) = store.record_rosetta(&instance.id, true).unwrap();
        assert!(tell);
        assert!(first.runs_under_rosetta());
        let (again, tell) = store.record_rosetta(&instance.id, true).unwrap();
        assert!(!tell);
        assert!(again.runs_under_rosetta());
        // Saved, and only for the version that ran under Rosetta.
        let mut changed = store.get(&instance.id).unwrap();
        assert!(changed.runs_under_rosetta());
        changed.game_version = "1.16.5".into();
        assert!(!changed.runs_under_rosetta());
        store.save(&changed).unwrap();
        let (native, tell) = store.record_rosetta(&instance.id, false).unwrap();
        assert!(!tell);
        assert_eq!(
            native.rosetta,
            Some(RosettaState {
                version: None,
                told: true
            })
        );
    }

    #[test]
    fn a_skin_banner_is_kept_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        let instance = store.create("Banner", "1.20.1", None).unwrap();
        let with = store
            .set_skin_banner(&instance.id, "grassblock", "sunset")
            .unwrap();
        assert_eq!(with.background_mode(), BackgroundMode::Banner);
        assert_eq!(with.skin_banner(), Some(("grassblock", "sunset")));
        assert!(store.background_path(&with).is_none());
        let cleared = store.clear_background(&instance.id).unwrap();
        assert_eq!(cleared.skin_banner(), None);
        assert_eq!(cleared.background_mode(), BackgroundMode::Default);
    }

    #[test]
    fn the_slideshow_keeps_its_own_images_and_wins_over_the_image() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        let instance = store.create("Shows", "1.20.1", None).unwrap();
        assert_eq!(instance.background_mode(), BackgroundMode::Default);
        let source = dir.path().join("photo.png");
        image::RgbImage::from_pixel(4000, 2000, image::Rgb([90, 120, 150]))
            .save(&source)
            .unwrap();

        let image = store.set_background(&instance.id, &source).unwrap();
        assert_eq!(image.background_mode(), BackgroundMode::Image);
        let show = store
            .set_slideshow(&instance.id, Some(SlideshowSource::Custom))
            .unwrap();
        assert_eq!(
            show.background_mode(),
            BackgroundMode::Slideshow(SlideshowSource::Custom)
        );
        // turning the slideshow off shows the image again
        let back = store.set_slideshow(&instance.id, None).unwrap();
        assert_eq!(back.background_mode(), BackgroundMode::Image);

        let first = store.add_slideshow_image(&instance.id, &source).unwrap();
        let second = store.add_slideshow_image(&instance.id, &source).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            store.slideshow_images(&instance.id),
            vec![first.clone(), second.clone()]
        );
        let saved = image::open(&first).unwrap();
        assert_eq!((saved.width(), saved.height()), (2560, 1280));

        let name = second.file_name().unwrap().to_str().unwrap();
        store.remove_slideshow_image(&instance.id, name).unwrap();
        assert_eq!(store.slideshow_images(&instance.id), vec![first]);
        assert!(
            store
                .remove_slideshow_image(&instance.id, "../instance.json")
                .is_err()
        );

        // the default clears both, and keeps the set for later
        let cleared = store.clear_background(&instance.id).unwrap();
        assert_eq!(cleared.background_mode(), BackgroundMode::Default);
        assert_eq!(store.slideshow_images(&instance.id).len(), 1);
    }

    #[test]
    fn the_slideshow_source_is_stored_by_name_and_left_out_when_off() {
        let mut instance = Instance {
            id: "a".into(),
            name: "A".into(),
            ..Instance::default()
        };
        let json = serde_json::to_value(&instance).unwrap();
        assert!(json.get("slideshow").is_none());
        instance.slideshow = Some(SlideshowSource::AllInstances);
        let json = serde_json::to_value(&instance).unwrap();
        assert_eq!(json["slideshow"], "all_instances");
        let back: Instance = serde_json::from_value(json).unwrap();
        assert_eq!(back.slideshow, Some(SlideshowSource::AllInstances));
    }

    use std::cell::RefCell;

    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn new_instances_get_a_name_no_other_instance_has() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        let first = store.create("Survival", "1.20.1", None).unwrap();
        let second = store.create("Survival", "1.20.1", None).unwrap();
        let third = store.create("survival", "1.20.1", None).unwrap();
        assert_eq!(first.name, "Survival");
        assert_eq!(second.name, "Survival (2)");
        // names are compared without case
        assert_eq!(third.name, "survival (3)");
        // a copy of a copy stays unique too
        let copy = store.duplicate(&first.id).unwrap();
        let copy_again = store.duplicate(&first.id).unwrap();
        assert_eq!(copy.name, "Survival (copy)");
        assert_eq!(copy_again.name, "Survival (copy) (2)");
        // a long name keeps within the limit
        let long = "x".repeat(MAX_NAME_CHARS);
        store.create(&long, "1.20.1", None).unwrap();
        let long_again = store.create(&long, "1.20.1", None).unwrap();
        assert!(long_again.name.ends_with(" (2)"));
        assert_eq!(long_again.name.chars().count(), MAX_NAME_CHARS);
        // and staged instances (imports, modpacks, matching instances) as well
        let staging = store.new_staging().unwrap();
        let staged = store
            .commit_staged(
                &staging,
                Instance {
                    name: "Survival".into(),
                    game_version: "1.20.1".into(),
                    ..Instance::default()
                },
            )
            .unwrap();
        assert_eq!(staged.name, "Survival (4)");
    }

    fn fabric() -> LoaderChoice {
        LoaderChoice {
            kind: LoaderKind::Fabric,
            version: "0.16.14".into(),
        }
    }

    #[test]
    fn creates_lists_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        assert!(store.list().0.is_empty());

        let survival = store
            .create("  My Survival World ", "1.20.1", None)
            .unwrap();
        assert_eq!(survival.id, "my-survival-world");
        assert_eq!(survival.name, "My Survival World");
        assert_eq!(survival.memory, None, "follows the global memory");
        assert!(store.game_dir(&survival.id).is_dir());
        let again = store
            .create("My survival world!", "1.20.1", Some(&fabric()))
            .unwrap();
        assert_eq!(again.id, "my-survival-world-2");
        assert_eq!(again.loader, Loader::Fabric);
        assert_eq!(again.loader_choice(), Some(fabric()));
        assert_eq!(again.loader_label(), "Fabric 0.16.14");
        let odd = store.create("✨✨", "1.8.9", None).unwrap();
        assert_eq!(odd.id, "instance");

        let raw: Value =
            serde_json::from_slice(&fs::read(store.dir(&again.id).join("instance.json")).unwrap())
                .unwrap();
        assert_eq!(raw["version"], json!(2));
        assert_eq!(raw["game_version"], json!("1.20.1"));
        assert_eq!(raw["loader"], json!("fabric"));
        assert_eq!(raw["loader_version"], json!("0.16.14"));
        assert!(raw.get("memory").is_none());

        let (listed, broken) = store.list();
        assert!(broken.is_empty());
        assert_eq!(listed.len(), 3);
        assert_eq!(store.get(&survival.id).unwrap(), survival);
    }

    #[test]
    fn loader_labels() {
        let forge = Instance {
            game_version: "1.20.1".into(),
            loader: Loader::Forge,
            loader_version: Some("1.20.1-47.4.28".into()),
            ..Instance::default()
        };
        assert_eq!(forge.loader_label(), "Forge 47.4.28");
        assert_eq!(Instance::default().loader_label(), "Vanilla");
        for (loader, text) in [
            (Loader::NeoForge, "\"neoforge\""),
            (Loader::Quilt, "\"quilt\""),
            (Loader::None, "\"none\""),
        ] {
            assert_eq!(serde_json::to_string(&loader).unwrap(), text);
            assert_eq!(serde_json::from_str::<Loader>(text).unwrap(), loader);
        }
    }

    /// An M3 file: default values become "use the global setting", custom ones stay.
    #[test]
    fn migrates_version_1() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let old_default_max = m3_default_max_ram_mb();
        if !cfg!(target_os = "macos") {
            // A quarter of the 8 GB M3 assumed, whatever this computer has.
            assert_eq!(old_default_max, 2048);
        }
        for (id, body) in [
            (
                "plain",
                json!({"version": 1, "id": "plain", "name": "Plain", "game_version": "1.20.1",
                       "loader": "none", "created": 1, "play_time": 3600, "last_played": 9,
                       "min_ram": 512, "max_ram": old_default_max, "jvm_args": "", "game_args": "",
                       "fullscreen": false}),
            ),
            (
                "custom",
                json!({"version": 1, "id": "custom", "name": "Custom", "game_version": "1.8.9",
                       "loader": "none", "created": 1, "play_time": 5,
                       "min_ram": 1024, "max_ram": 6144, "jvm_args": "-XX:+UseG1GC", "game_args": "",
                       "resolution": {"width": 1280, "height": 720}, "fullscreen": true}),
            ),
        ] {
            fs::create_dir_all(dir.path().join(id)).unwrap();
            fs::write(dir.path().join(id).join("instance.json"), body.to_string()).unwrap();
        }
        let plain = store.get("plain").unwrap();
        assert_eq!(plain.memory, None);
        assert_eq!(plain.jvm_args, None);
        assert_eq!(plain.window, None);
        assert_eq!(plain.play_time, 3600, "play time survives");
        assert_eq!(plain.last_played, Some(9));
        let custom = store.get("custom").unwrap();
        assert_eq!(
            custom.memory,
            Some(Memory {
                min: 1024,
                max: 6144
            })
        );
        assert_eq!(custom.jvm_args.as_deref(), Some("-XX:+UseG1GC"));
        assert_eq!(custom.game_args, None);
        assert_eq!(
            custom.window,
            Some(WindowSettings {
                resolution: Some(Resolution {
                    width: 1280,
                    height: 720
                }),
                fullscreen: true,
            })
        );
        // Saving writes version 2.
        store.save(&custom).unwrap();
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("custom/instance.json")).unwrap())
                .unwrap();
        assert_eq!(raw["version"], json!(2));
        assert!(raw.get("min_ram").is_none());
    }

    #[test]
    fn rejects_empty_names_and_bad_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        assert!(matches!(
            store.create("   ", "1.20.1", None),
            Err(InstanceError::EmptyName)
        ));
        for bad in ["../x", "", ".hidden", "a/b"] {
            assert!(
                matches!(store.get(bad), Err(InstanceError::NotFound(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn renames_and_records_play_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let instance = store.create("Old", "1.20.1", None).unwrap();
        let renamed = store.rename(&instance.id, "New name").unwrap();
        assert_eq!(renamed.id, "old", "the folder keeps its name");
        assert_eq!(store.get("old").unwrap().name, "New name");
        store.record_play("old", 1_700_000_000, 90).unwrap();
        let played = store.record_play("old", 1_700_000_500, 30).unwrap();
        assert_eq!(played.play_time, 120);
        assert_eq!(played.last_played, Some(1_700_000_500));
    }

    #[test]
    fn lists_recently_played_first_and_reports_broken_folders() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let a = store.create("A", "1.20.1", None).unwrap();
        let b = store.create("B", "1.20.1", None).unwrap();
        store.record_play(&a.id, 2_000_000_000, 1).unwrap();
        fs::create_dir_all(dir.path().join("broken")).unwrap();
        fs::write(dir.path().join("broken/instance.json"), "{ nope").unwrap();
        fs::create_dir_all(dir.path().join("not-an-instance")).unwrap();
        let (listed, broken) = store.list();
        assert_eq!(
            listed.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            [a.id.as_str(), b.id.as_str()]
        );
        assert_eq!(broken.len(), 1);
    }

    #[test]
    fn delete_moves_the_folder_with_the_given_remover() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let instance = store.create("Doomed", "1.20.1", None).unwrap();
        let removed = RefCell::new(None);
        store
            .delete_with(&instance.id, |path| {
                *removed.borrow_mut() = Some(path.to_owned());
                fs::remove_dir_all(path).map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(removed.into_inner(), Some(store.dir(&instance.id)));
        assert!(store.list().0.is_empty());
        assert!(matches!(
            store.delete_with("doomed", |_| Ok(())),
            Err(InstanceError::NotFound(_))
        ));
        let other = store.create("Other", "1.20.1", None).unwrap();
        let err = store
            .delete_with(&other.id, |_| Err("Trash is full".into()))
            .unwrap_err();
        assert!(err.to_string().contains("Trash is full"));
    }

    #[test]
    fn duplicates_with_the_game_folder_and_no_play_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let mut original = store.create("Modded", "1.20.1", Some(&fabric())).unwrap();
        original.group = Some("Survival".into());
        original.jvm_args = Some("-XX:+UseZGC".into());
        store.save(&original).unwrap();
        store.record_play(&original.id, 1_700_000_000, 600).unwrap();
        let game = store.game_dir(&original.id);
        fs::create_dir_all(game.join("saves/World")).unwrap();
        fs::write(game.join("saves/World/level.dat"), b"level").unwrap();
        fs::write(game.join("options.txt"), b"fov:90").unwrap();
        fs::create_dir_all(store.natives_dir(&original.id)).unwrap();
        fs::write(store.natives_dir(&original.id).join("lib.dylib"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", game.join("link")).unwrap();

        let copy = store.duplicate(&original.id).unwrap();
        assert_eq!(copy.id, "modded-copy");
        assert_eq!(copy.name, "Modded (copy)");
        assert_eq!(copy.play_time, 0);
        assert_eq!(copy.last_played, None);
        assert_eq!(copy.loader_choice(), Some(fabric()));
        assert_eq!(copy.group.as_deref(), Some("Survival"));
        assert_eq!(copy.jvm_args.as_deref(), Some("-XX:+UseZGC"));
        let copied = store.game_dir(&copy.id);
        assert_eq!(
            fs::read(copied.join("saves/World/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(fs::read(copied.join("options.txt")).unwrap(), b"fov:90");
        assert!(!store.natives_dir(&copy.id).exists());
        assert!(!copied.join("link").exists());
        assert_eq!(store.get(&copy.id).unwrap(), copy);
        // The original keeps its play time; no staging folder is left.
        assert_eq!(store.get(&original.id).unwrap().play_time, 600);
        assert_eq!(store.list().0.len(), 2);
        assert_eq!(store.duplicate(&original.id).unwrap().id, "modded-copy-2");
        assert!(
            fs::read_dir(dir.path()).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.'))
        );
    }

    #[test]
    fn custom_icons() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let instance = store.create("Pretty", "1.20.1", None).unwrap();
        assert_eq!(store.icon_path(&instance), None);
        let source = dir.path().join("wide.png");
        image::RgbaImage::from_pixel(300, 200, image::Rgba([10, 200, 30, 255]))
            .save(&source)
            .unwrap();
        let updated = store.set_icon(&instance.id, &source).unwrap();
        assert_eq!(updated.icon.as_deref(), Some(CUSTOM_ICON));
        let icon = store.icon_path(&updated).unwrap();
        let decoded = image::open(&icon).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (ICON_SIZE, ICON_SIZE));
        // JPEG photos work too.
        let photo = dir.path().join("photo.jpg");
        image::RgbImage::from_pixel(64, 96, image::Rgb([200, 10, 10]))
            .save(&photo)
            .unwrap();
        store.set_icon(&instance.id, &photo).unwrap();
        let decoded = image::open(&icon).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (ICON_SIZE, ICON_SIZE));
        let not_image = dir.path().join("notes.png");
        fs::write(&not_image, b"not an image").unwrap();
        assert!(matches!(
            store.set_icon(&instance.id, &not_image),
            Err(InstanceError::Icon { .. })
        ));
        let cleared = store.clear_icon(&instance.id).unwrap();
        assert_eq!(cleared.icon, None);
        assert!(!icon.exists());
    }

    #[test]
    fn groups() {
        let instance = |id: &str, group: Option<&str>| Instance {
            id: id.into(),
            group: group.map(str::to_string),
            ..Instance::default()
        };
        let list = [
            instance("a", Some("survival")),
            instance("b", None),
            instance("c", Some("Modded")),
            instance("d", Some("survival")),
            instance("e", Some("  ")),
        ];
        let groups: Vec<(Option<String>, Vec<&str>)> = grouped(&list, false, |_| false)
            .into_iter()
            .map(|(name, items)| (name, items.iter().map(|i| i.id.as_str()).collect()))
            .collect();
        assert_eq!(
            groups,
            [
                (None, vec!["b", "e"]),
                (Some("Modded".into()), vec!["c"]),
                (Some("survival".into()), vec!["a", "d"]),
            ]
        );
    }

    fn ids(groups: Vec<(Option<String>, Vec<&Instance>)>) -> Vec<(Option<String>, Vec<&str>)> {
        groups
            .into_iter()
            .map(|(name, items)| (name, items.iter().map(|i| i.id.as_str()).collect()))
            .collect()
    }

    fn auto_instance(id: &str, loader: Loader, group: Option<&str>, imported: bool) -> Instance {
        Instance {
            id: id.into(),
            loader,
            group: group.map(str::to_string),
            imported_from: imported.then(|| "Prism Launcher".to_string()),
            ..Instance::default()
        }
    }

    #[test]
    fn automatic_groups_follow_the_loader_or_the_import() {
        let list = [
            auto_instance("vanilla", Loader::None, None, false),
            auto_instance("fabric", Loader::Fabric, None, false),
            auto_instance("forge", Loader::Forge, None, false),
            auto_instance("neo", Loader::NeoForge, None, false),
            auto_instance("quilt", Loader::Quilt, None, false),
            auto_instance("prism", Loader::Fabric, None, true),
            auto_instance("named", Loader::Fabric, Some("Survival"), true),
        ];
        assert_eq!(AutoGroup::of(&list[0], false), AutoGroup::Vanilla);
        assert_eq!(AutoGroup::of(&list[5], false), AutoGroup::Imported);
        // A group of its own wins over the automatic one.
        assert_eq!(group_of(&list[6], true, false).as_deref(), Some("Survival"));
        assert_eq!(
            group_of(&list[1], true, false).as_deref(),
            Some("Fabric (modded)")
        );
        assert_eq!(group_of(&list[1], false, false), None);
        assert_eq!(
            ids(grouped(&list, true, |_| false)),
            [
                (Some("Survival".into()), vec!["named"]),
                (Some("Fabric (modded)".into()), vec!["fabric"]),
                (Some("Forge (modded)".into()), vec!["forge"]),
                (Some("NeoForge (modded)".into()), vec!["neo"]),
                (Some("Quilt (modded)".into()), vec!["quilt"]),
                (Some("Vanilla".into()), vec!["vanilla"]),
                (Some("Imported".into()), vec!["prism"]),
            ]
        );
        // Off, the instances without a group are listed loose, as before.
        assert_eq!(
            ids(grouped(&list, false, |_| false)),
            [
                (
                    None,
                    vec!["vanilla", "fabric", "forge", "neo", "quilt", "prism"]
                ),
                (Some("Survival".into()), vec!["named"]),
            ]
        );
    }

    #[test]
    fn automatic_groups_are_ordered_and_skip_empty_ones() {
        let list = [
            auto_instance("old-import", Loader::None, None, true),
            auto_instance("v", Loader::None, None, false),
            auto_instance("b", Loader::Forge, Some("beta"), false),
            auto_instance("a", Loader::Forge, Some("Alpha"), false),
            auto_instance("q", Loader::Quilt, None, false),
        ];
        assert_eq!(
            ids(grouped(&list, true, |_| false)),
            [
                (Some("Alpha".into()), vec!["a"]),
                (Some("beta".into()), vec!["b"]),
                (Some("Quilt (modded)".into()), vec!["q"]),
                (Some("Vanilla".into()), vec!["v"]),
                (Some("Imported".into()), vec!["old-import"]),
            ]
        );
    }

    #[test]
    fn a_named_group_called_like_an_automatic_one_takes_its_instances() {
        let list = [
            auto_instance("mine", Loader::Forge, Some("vanilla"), false),
            auto_instance("plain", Loader::None, None, false),
            auto_instance("fabric", Loader::Fabric, None, false),
        ];
        assert_eq!(
            ids(grouped(&list, true, |_| false)),
            [
                (Some("vanilla".into()), vec!["mine", "plain"]),
                (Some("Fabric (modded)".into()), vec!["fabric"]),
            ]
        );
    }

    #[test]
    fn linked_instances_are_grouped_first() {
        let list = [
            auto_instance("plain", Loader::None, None, false),
            auto_instance("smp", Loader::Fabric, None, false),
            auto_instance("fabric", Loader::Fabric, None, false),
            auto_instance("pack", Loader::Forge, None, true),
            auto_instance("named", Loader::Fabric, Some("Alpha"), false),
        ];
        let linked = |i: &Instance| ["smp", "pack", "named"].contains(&i.id.as_str());
        // Linked instances leave their loader's (or Imported's) group for Servers,
        // which comes before the groups the user named; a group of its own still wins.
        assert_eq!(AutoGroup::of(&list[3], true), AutoGroup::Servers);
        assert_eq!(group_of(&list[1], true, true).as_deref(), Some("Servers"));
        assert_eq!(group_of(&list[4], true, true).as_deref(), Some("Alpha"));
        assert_eq!(
            ids(grouped(&list, true, linked)),
            [
                (Some("Servers".into()), vec!["smp", "pack"]),
                (Some("Alpha".into()), vec!["named"]),
                (Some("Fabric (modded)".into()), vec!["fabric"]),
                (Some("Vanilla".into()), vec!["plain"]),
            ]
        );
        // Without linked instances, nothing changes.
        assert_eq!(
            ids(grouped(&list, true, |_| false)),
            [
                (Some("Alpha".into()), vec!["named"]),
                (Some("Fabric (modded)".into()), vec!["smp", "fabric"]),
                (Some("Vanilla".into()), vec!["plain"]),
                (Some("Imported".into()), vec!["pack"]),
            ]
        );
        // Off, nothing moves: the links make no group.
        assert_eq!(group_of(&list[1], false, true), None);
        assert_eq!(
            ids(grouped(&list, false, linked)),
            ids(grouped(&list, false, |_| false))
        );
        // A group the user named "servers" takes them, in its name's place.
        let mut list = list.to_vec();
        list[2].group = Some("servers".into());
        assert_eq!(
            ids(grouped(&list, true, linked)),
            [
                (Some("Alpha".into()), vec!["named"]),
                (Some("servers".into()), vec!["smp", "fabric", "pack"]),
                (Some("Vanilla".into()), vec!["plain"]),
            ]
        );
    }

    /// The links on disk say which instances are linked: to a server on a core, or
    /// to a friend's shared server.
    #[test]
    fn instances_linked_on_disk_and_to_friends_are_grouped_as_servers() {
        use crate::cores::{friends, link};
        let dir = tempfile::tempdir().unwrap();
        let list = [
            auto_instance("core-linked", Loader::Fabric, None, false),
            auto_instance("friend-linked", Loader::None, None, false),
            auto_instance("loose", Loader::None, None, false),
        ];
        let save = |id: &str, core: &str| {
            let instance_dir = dir.path().join(id);
            fs::create_dir_all(&instance_dir).unwrap();
            let server = link::ServerLink {
                core_uuid: core.into(),
                core_url: "https://example.org:16662".into(),
                server_uuid: "smp".into(),
                server_name: "Survival SMP".into(),
                pack_etag: None,
            };
            link::save_link(&instance_dir, &server).unwrap();
        };
        save("core-linked", "6f1c-core-uuid");
        let friend = format!("{}connect.lodestone.cc/ABCDEFGH23", friends::ID_PREFIX);
        assert!(friends::is_friend_id(&friend));
        save("friend-linked", &friend);
        fs::create_dir_all(dir.path().join("loose")).unwrap();
        let linked: std::collections::HashSet<String> = link::linked_instances(dir.path())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            ids(grouped(&list, true, |i| linked.contains(&i.id))),
            [
                (Some("Servers".into()), vec!["core-linked", "friend-linked"]),
                (Some("Vanilla".into()), vec!["loose"]),
            ]
        );
    }

    #[test]
    fn the_last_played_instance() {
        let played = |id: &str, at: Option<u64>| Instance {
            id: id.into(),
            last_played: at,
            ..Instance::default()
        };
        assert!(last_played(&[played("a", None)]).is_none());
        let list = [
            played("a", Some(5)),
            played("b", None),
            played("c", Some(9)),
        ];
        assert_eq!(last_played(&list).map(|i| i.id.as_str()), Some("c"));
    }

    #[test]
    fn imported_from_is_kept_and_optional() {
        let old: Instance = serde_json::from_value(serde_json::json!({
            "id": "a", "name": "A", "game_version": "1.21.4", "created": 1
        }))
        .unwrap();
        assert_eq!(old.imported_from, None);
        let value = serde_json::to_value(&old).unwrap();
        assert!(value.get("imported_from").is_none(), "left out when unset");
        let imported = Instance {
            imported_from: Some("Prism Launcher".into()),
            ..old
        };
        let back: Instance =
            serde_json::from_value(serde_json::to_value(&imported).unwrap()).unwrap();
        assert_eq!(back.imported_from.as_deref(), Some("Prism Launcher"));
    }

    #[test]
    fn records_the_start_before_the_play_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path());
        let instance = store.create("Started", "1.21.4", None).unwrap();
        let started = store.record_start(&instance.id, 1_700_000_000).unwrap();
        assert_eq!(started.last_played, Some(1_700_000_000));
        assert_eq!(started.play_time, 0);
        let finished = store.record_play(&instance.id, 1_700_000_000, 60).unwrap();
        assert_eq!(finished.last_played, Some(1_700_000_000));
        assert_eq!(finished.play_time, 60);
    }

    #[test]
    fn environment_variables() {
        let env = parse_env(
            "# comment\nFOO=bar MESA_GL_VERSION_OVERRIDE=4.5\n\n  SPACED=\"a b\" EMPTY=\n",
        )
        .unwrap();
        assert_eq!(env["FOO"], "bar");
        assert_eq!(env["MESA_GL_VERSION_OVERRIDE"], "4.5");
        assert_eq!(env["SPACED"], "a b");
        assert_eq!(env["EMPTY"], "");
        assert_eq!(env.len(), 4);
        assert_eq!(parse_env(&format_env(&env)).unwrap(), env);
        assert_eq!(
            format_env(&env),
            "EMPTY=\"\" FOO=bar MESA_GL_VERSION_OVERRIDE=4.5 SPACED=\"a b\""
        );
        assert!(parse_env("NOEQUALS").unwrap_err().contains("NAME=value"));
        assert!(parse_env("OK=1 1BAD=x").is_err());
        assert!(parse_env("A-B=x").is_err());
        assert!(parse_env("").unwrap().is_empty());
    }

    /// The instance follows the global settings until it overrides them.
    #[test]
    fn game_settings_inherit_or_override() {
        let defaults = InstanceDefaults {
            min_ram: Some(1024),
            max_ram: Some(6144),
            jvm_args: "-XX:+UseG1GC".into(),
            game_args: "--demo".into(),
            resolution: Some(Resolution {
                width: 1280,
                height: 720,
            }),
            fullscreen: false,
            env: [("A".to_string(), "1".to_string())].into(),
            ..InstanceDefaults::default()
        };
        let inherited = Instance::default().game_settings(&defaults, Some(16 << 30));
        assert_eq!((inherited.min_ram_mb, inherited.max_ram_mb), (1024, 6144));
        assert_eq!(inherited.jvm_args, ["-XX:+UseG1GC"]);
        assert_eq!(inherited.game_args, ["--demo"]);
        assert_eq!(inherited.resolution, Some((1280, 720)));
        assert_eq!(inherited.env, [("A".to_string(), "1".to_string())]);
        // With no global memory set, the Mac's default: half its memory, at most 4 GB.
        let plain = Instance::default().game_settings(&InstanceDefaults::default(), Some(6 << 30));
        assert_eq!((plain.min_ram_mb, plain.max_ram_mb), (512, 3072));

        let instance = Instance {
            memory: Some(Memory {
                min: 2048,
                max: 3072,
            }),
            jvm_args: Some("-Dfoo=\"a b\"".into()),
            game_args: Some(String::new()),
            window: Some(WindowSettings {
                resolution: None,
                fullscreen: true,
            }),
            env: Some(BTreeMap::new()),
            ..Instance::default()
        };
        let overridden = instance.game_settings(&defaults, Some(16 << 30));
        assert_eq!((overridden.min_ram_mb, overridden.max_ram_mb), (2048, 3072));
        assert_eq!(overridden.jvm_args, ["-Dfoo=a b"]);
        assert!(
            overridden.game_args.is_empty(),
            "an empty override is no arguments"
        );
        assert_eq!(overridden.resolution, None);
        assert!(overridden.fullscreen);
        assert!(overridden.env.is_empty());
    }
}
