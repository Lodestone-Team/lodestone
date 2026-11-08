//! Instances: a Minecraft version with its own game folder and settings.
//!
//! Each instance is a folder under `<data>/instances/` with `instance.json` (this
//! module's [`Instance`], a versioned document) and `minecraft/`, the game's working
//! directory, where saves, mods and options live. The folder name is the instance's
//! id and never changes; renaming changes only the name.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    game::launch::{GameSettings, split_args},
    storage::{self, Document, Migration, StorageError},
};

/// The mod loader. Fabric, Quilt, Forge and NeoForge come later.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

/// `instance.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// The name of one of the launcher's icons, if one was picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub game_version: String,
    #[serde(default)]
    pub loader: Loader,
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
    /// Memory in MiB.
    pub min_ram: u32,
    pub max_ram: u32,
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
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: None,
            game_version: String::new(),
            loader: Loader::None,
            created: 0,
            last_played: None,
            play_time: 0,
            java_override: None,
            min_ram: DEFAULT_MIN_RAM_MB,
            max_ram: default_max_ram_mb(None),
            jvm_args: String::new(),
            game_args: String::new(),
            resolution: None,
            fullscreen: false,
        }
    }
}

impl Document for Instance {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

/// The default minimum memory.
pub const DEFAULT_MIN_RAM_MB: u32 = 512;

/// The default maximum memory: a quarter of the machine's memory, between 2 and
/// 4 GiB.
pub fn default_max_ram_mb(total_bytes: Option<u64>) -> u32 {
    let total_mb = total_bytes.map_or(8192, |bytes| bytes / (1024 * 1024));
    (total_mb / 4).clamp(2048, 4096) as u32
}

impl Instance {
    /// The settings the launch command uses.
    pub fn game_settings(&self) -> GameSettings {
        GameSettings {
            min_ram_mb: self.min_ram,
            max_ram_mb: self.max_ram,
            jvm_args: split_args(&self.jvm_args),
            game_args: split_args(&self.game_args),
            resolution: self.resolution.map(|r| (r.width, r.height)),
            fullscreen: self.fullscreen,
        }
    }
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

    /// Creates an instance of `game_version` named `name`, with default settings.
    pub fn create(
        &self,
        name: &str,
        game_version: &str,
        total_memory: Option<u64>,
    ) -> Result<Instance, InstanceError> {
        let name = clean_name(name)?;
        let id = self.unused_id(&name);
        let instance = Instance {
            id: id.clone(),
            name,
            game_version: game_version.to_string(),
            created: now(),
            max_ram: default_max_ram_mb(total_memory),
            ..Instance::default()
        };
        let game_dir = self.game_dir(&id);
        fs::create_dir_all(&game_dir).map_err(|source| InstanceError::Io {
            path: game_dir,
            source,
        })?;
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

    #[test]
    fn creates_lists_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("instances"));
        assert!(store.list().0.is_empty());

        let survival = store
            .create("  My Survival World ", "1.20.1", Some(16 << 30))
            .unwrap();
        assert_eq!(survival.id, "my-survival-world");
        assert_eq!(survival.name, "My Survival World");
        assert_eq!(survival.max_ram, 4096);
        assert_eq!(survival.min_ram, DEFAULT_MIN_RAM_MB);
        assert!(store.game_dir(&survival.id).is_dir());
        let again = store.create("My survival world!", "26.3", None).unwrap();
        assert_eq!(again.id, "my-survival-world-2");
        let odd = store.create("✨✨", "1.8.9", None).unwrap();
        assert_eq!(odd.id, "instance");

        let raw: Value = serde_json::from_slice(
            &fs::read(store.dir(&survival.id).join("instance.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["version"], json!(1));
        assert_eq!(raw["game_version"], json!("1.20.1"));
        assert_eq!(raw["loader"], json!("none"));

        let (listed, broken) = store.list();
        assert!(broken.is_empty());
        assert_eq!(listed.len(), 3);
        assert_eq!(store.get(&survival.id).unwrap(), survival);
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
    fn game_settings() {
        let instance = Instance {
            min_ram: 1024,
            max_ram: 3072,
            jvm_args: "-XX:+UseG1GC -Dfoo=\"a b\"".into(),
            game_args: "--demo".into(),
            resolution: Some(Resolution {
                width: 1280,
                height: 720,
            }),
            ..Instance::default()
        };
        let settings = instance.game_settings();
        assert_eq!(settings.jvm_args, ["-XX:+UseG1GC", "-Dfoo=a b"]);
        assert_eq!(settings.game_args, ["--demo"]);
        assert_eq!(settings.resolution, Some((1280, 720)));
        assert_eq!(default_max_ram_mb(Some(4 << 30)), 2048);
        assert_eq!(default_max_ram_mb(Some(12 << 30)), 3072);
        assert_eq!(default_max_ram_mb(Some(64 << 30)), 4096);
    }
}
