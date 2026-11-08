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
    /// [`CUSTOM_ICON`] for the instance's own `icon.png`; otherwise the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The sidebar group the instance is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
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
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: None,
            group: None,
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
        }
    }
}

impl Document for Instance {
    const VERSION: u32 = 2;
    /// Version 0 is a file without a `"version"` key; its fields carry over. Version
    /// 2 makes the settings overrides of the global defaults.
    const MIGRATIONS: &'static [Migration] = &[Ok, migrate_v1];
}

/// Version 1 stored every setting. Values that were the defaults M3 filled in become
/// "use the global setting"; the others stay as the instance's own.
fn migrate_v1(mut fields: Map<String, Value>) -> anyhow::Result<Map<String, Value>> {
    let min = fields.remove("min_ram").and_then(|v| v.as_u64());
    let max = fields.remove("max_ram").and_then(|v| v.as_u64());
    if let (Some(min), Some(max)) = (min, max) {
        let old_default_max = {
            let total_mb =
                crate::game::platform::total_memory().map_or(8192, |bytes| bytes / (1024 * 1024));
            (total_mb / 4).clamp(2048, 4096)
        };
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

/// The instances by sidebar group: those without a group first, then each group in
/// name order (ignoring case), each keeping the list's order.
pub fn grouped(instances: &[Instance]) -> Vec<(Option<String>, Vec<&Instance>)> {
    let mut groups: Vec<(Option<String>, Vec<&Instance>)> = Vec::new();
    let ungrouped: Vec<&Instance> = instances
        .iter()
        .filter(|i| group_name(i).is_none())
        .collect();
    if !ungrouped.is_empty() {
        groups.push((None, ungrouped));
    }
    let mut names: Vec<&str> = instances.iter().filter_map(group_name).collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    for name in names {
        groups.push((
            Some(name.to_string()),
            instances
                .iter()
                .filter(|i| group_name(i) == Some(name))
                .collect(),
        ));
    }
    groups
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
    #[error("couldn't move {path} to the Trash: {reason}")]
    Trash { path: PathBuf, reason: String },
    #[error("couldn't use {path} as the icon: {reason}")]
    Icon { path: PathBuf, reason: String },
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
        instances.sort_by(|a, b| {
            b.last_played
                .unwrap_or(0)
                .cmp(&a.last_played.unwrap_or(0))
                .then(b.created.cmp(&a.created))
                .then(a.name.cmp(&b.name))
        });
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
        let name = clean_name(name)?;
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
        let name = clean_name(&name)?;
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
            fs::rename(&staging, self.dir(&new_id)).map_err(io_error(&staging))?;
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
        instance.name = clean_name(&instance.name)?;
        instance.id = self.unused_id(&instance.name);
        storage::save(&staging.join("instance.json"), &instance)?;
        let dir = self.dir(&instance.id);
        fs::rename(staging, &dir).map_err(io_error(staging))?;
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
        let mut instance = self.get(id)?;
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
        instance.icon = Some(CUSTOM_ICON.to_string());
        self.save(&instance)?;
        Ok(instance)
    }

    /// Goes back to the default icon.
    pub fn clear_icon(&self, id: &str) -> Result<Instance, InstanceError> {
        let mut instance = self.get(id)?;
        let path = self.dir(id).join(ICON_FILE);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(InstanceError::Io { path, source }),
        }
        instance.icon = None;
        self.save(&instance)?;
        Ok(instance)
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
    use std::cell::RefCell;

    use serde_json::{Value, json};

    use super::*;

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
        let old_default_max = {
            let total_mb =
                crate::game::platform::total_memory().map_or(8192, |b| b / (1024 * 1024));
            (total_mb / 4).clamp(2048, 4096)
        };
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
        let groups: Vec<(Option<String>, Vec<&str>)> = grouped(&list)
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
