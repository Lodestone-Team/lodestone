//! Shared game settings: one set of `options.txt` values (mouse sensitivity, key
//! bindings, video, sound, accessibility, language) that every instance gets.
//!
//! - The global set is `<data>/shared_options.json` ([`Profile`]), in one canonical
//!   format ([`format`]); it is converted to each instance's version when merged in.
//! - Each instance has `<instance>/shared_options.json` ([`InstanceSharing`]): whether it
//!   uses sharing, the keys it overrides (they keep the instance's own value and are
//!   never written or read back), and what the last launch left in its `options.txt`.
//! - Before a launch, [`before_launch`] merges the global values into the instance's
//!   `options.txt`, keeping every other line and the order.
//! - After the game exits, [`after_exit`] compares the file with what was merged in. The
//!   *preferred* instance's in-game changes go into the global set; any other
//!   instance's are only recorded ([`InstanceSharing::changed`]), so the launcher can
//!   offer to keep them as overrides before the next launch replaces them. With no
//!   preferred instance nothing is read back.
//! - Every change reads, changes and saves under a lock file ([`LOCK_FILE`]) next to the
//!   documents, so two launchers on one data directory don't lose each other's changes.
//!
//! `resourcePacks`, `incompatibleResourcePacks` and `lastServer` ([`keys::EXCLUDED`]) are
//! never shared, nor is any key not in [`keys::SPECS`] apart from mod key bindings the
//! instance's file already has. Shader configs are never opened.
//!
//! TODO(M5b): share `servers.dat` once the NBT reader lands.

pub mod format;
pub mod keycodes;
pub mod keys;

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use keys::{Category, Kind, Spec};
use keys::{Codec, find, is_other_binding, legacy_data_version, release_data_version};

use crate::{
    content::options::{self, OptionsError, OptionsFile},
    paths::DataDir,
    storage::{self, Document, Migration, StorageError},
};

/// The global set, in the data directory.
pub const PROFILE_FILE: &str = "shared_options.json";
/// An instance's sharing state, in the instance's folder.
pub const INSTANCE_FILE: &str = "shared_options.json";

/// `shared_options.json` in the data directory.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Categories that are not shared.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled: BTreeSet<Category>,
    /// Key → value, in the canonical format.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// The instance whose in-game changes update the global set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred: Option<String>,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Document for Profile {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

impl Profile {
    pub fn shares(&self, category: Category) -> bool {
        !self.disabled.contains(&category)
    }
}

/// `shared_options.json` in an instance's folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceSharing {
    /// "Use shared settings".
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Keys that keep the instance's own value.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub overrides: BTreeSet<String>,
    /// The canonical value of each shared key the last launch left in `options.txt`;
    /// `None` when no game is waiting to be read back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<BTreeMap<String, String>>,
    /// Shared keys changed in-game in this (non-preferred) instance, with their
    /// in-game values. The next launch replaces them unless they become overrides.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub changed: BTreeMap<String, String>,
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

fn yes() -> bool {
    true
}

impl Default for InstanceSharing {
    fn default() -> Self {
        Self {
            enabled: true,
            overrides: BTreeSet::new(),
            session: None,
            changed: BTreeMap::new(),
            other: Map::new(),
        }
    }
}

impl Document for InstanceSharing {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

#[derive(Debug, thiserror::Error)]
pub enum SharedError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Options(#[from] OptionsError),
    #[error("{0}")]
    Invalid(String),
}

pub fn profile_path(data: &DataDir) -> PathBuf {
    data.root().join(PROFILE_FILE)
}

pub fn instance_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(INSTANCE_FILE)
}

/// The vanilla client jar of `game_version`, which has its data version.
pub fn client_jar(data: &DataDir, game_version: &str) -> PathBuf {
    data.versions_dir()
        .join(game_version)
        .join(format!("{game_version}.jar"))
}

/// The lock file next to the global set and in each instance's folder.
pub const LOCK_FILE: &str = "shared_options.lock";

/// Held while documents are read, changed and saved, so another launcher on the same
/// data directory (or another thread) waits instead of saving over a change made in
/// between. An instance's lock is always taken before the global set's. Dropping it
/// unlocks.
struct Lock {
    _file: std::fs::File,
}

/// Locks the documents in `dir` (the data directory or an instance's folder), waiting
/// for whoever holds them.
fn lock(dir: &Path) -> Result<Lock, SharedError> {
    let path = dir.join(LOCK_FILE);
    let file = std::fs::create_dir_all(dir)
        .and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)
        })
        .and_then(|file| file.lock().map(|()| file))
        .map_err(|source| StorageError::Io { path, source })?;
    Ok(Lock { _file: file })
}

/// Loads the global set. A preferred instance that no longer exists (deleted) is
/// cleared, and the file saved.
pub fn load_profile(data: &DataDir) -> Result<Profile, SharedError> {
    let _lock = lock(data.root())?;
    read_profile(data)
}

/// [`load_profile`] for a caller that holds the lock.
fn read_profile(data: &DataDir) -> Result<Profile, SharedError> {
    let path = profile_path(data);
    let mut profile = storage::load::<Profile>(&path)?.document;
    let gone = profile.preferred.as_deref().is_some_and(|id| {
        id.is_empty()
            || id.contains(['/', '\\'])
            || id.starts_with('.')
            || !data
                .instances_dir()
                .join(id)
                .join("instance.json")
                .is_file()
    });
    if gone {
        profile.preferred = None;
        storage::save(&path, &profile)?;
    }
    Ok(profile)
}

/// Changes the global set on disk. `change` runs under the lock, so it mustn't call back
/// into this module.
pub fn update_profile(
    data: &DataDir,
    change: impl FnOnce(&mut Profile),
) -> Result<Profile, SharedError> {
    let _lock = lock(data.root())?;
    change_profile(data, change)
}

/// [`update_profile`] for a caller that holds the lock.
fn change_profile(
    data: &DataDir,
    change: impl FnOnce(&mut Profile),
) -> Result<Profile, SharedError> {
    let mut profile = read_profile(data)?;
    let before = profile.clone();
    change(&mut profile);
    storage::save(&profile_path(data), &profile)?;
    crate::trace::settings_changed("shared", &before, &profile, serde_json::Value::Null);
    Ok(profile)
}

pub fn load_instance(instance_dir: &Path) -> Result<InstanceSharing, SharedError> {
    Ok(storage::load::<InstanceSharing>(&instance_path(instance_dir))?.document)
}

/// Changes an instance's sharing state on disk.
pub fn update_instance(
    instance_dir: &Path,
    change: impl FnOnce(&mut InstanceSharing),
) -> Result<InstanceSharing, SharedError> {
    let _lock = lock(instance_dir)?;
    let mut sharing = load_instance(instance_dir)?;
    change(&mut sharing);
    storage::save(&instance_path(instance_dir), &sharing)?;
    Ok(sharing)
}

/// Makes `id` the preferred instance (or none), and turns sharing on for it.
pub fn set_preferred(
    data: &DataDir,
    id: Option<&str>,
    instance_dir: Option<&Path>,
) -> Result<Profile, SharedError> {
    if let Some(dir) = instance_dir {
        update_instance(dir, |sharing| sharing.enabled = true)?;
    }
    update_profile(data, |profile| profile.preferred = id.map(str::to_string))
}

/// The data version of a client jar, from its `version.json` (18w47b and later).
pub fn jar_data_version(jar: &Path) -> Option<u32> {
    let file = std::fs::File::open(jar).ok()?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    let mut entry = archive.by_name("version.json").ok()?;
    if entry.size() > 64 * 1024 {
        return None;
    }
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;
    json.get("world_version")?.as_u64()?.try_into().ok()
}

/// The format an instance's `options.txt` is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The data version whose format values are written in.
    pub dv: u32,
    /// For a file without a `version` line on 1.10+: the line to add, so the game
    /// doesn't run its fixers over values that are already current.
    pub add_version_line: Option<u32>,
}

/// The format of `file`: its own `version` line; for a file without one, the data
/// version of the game (from its client jar, or the table for 1.10–1.13.2), or the
/// made-up one of a release before 1.10. `None` when it can't be told.
pub fn target(file: &OptionsFile, game_version: &str, jar: Option<&Path>) -> Option<Target> {
    if let Some(dv) = file
        .get("version")
        .and_then(|v| v.trim().parse::<u32>().ok())
    {
        return Some(Target {
            dv,
            add_version_line: None,
        });
    }
    if let Some(dv) = legacy_data_version(game_version) {
        return Some(Target {
            dv,
            add_version_line: None,
        });
    }
    let dv = jar
        .and_then(jar_data_version)
        .or_else(|| release_data_version(game_version))?;
    Some(Target {
        dv,
        add_version_line: Some(dv),
    })
}

/// A shared setting as one instance's file has it.
struct Setting {
    key: String,
    category: Category,
    /// (key in the file, codec, whether the key exists at the file's data version).
    forms: Vec<(String, Codec, bool)>,
}

impl Setting {
    fn from_spec(spec: &Spec, dv: u32) -> Self {
        Self {
            key: spec.key.to_string(),
            category: spec.category,
            forms: spec
                .forms()
                .map(|form| (form.key.to_string(), form.codec, form.exists_at(dv)))
                .collect(),
        }
    }

    /// A mod's key binding: only written where the file has it.
    fn other_binding(key: &str) -> Self {
        Self {
            key: key.to_string(),
            category: Category::Controls,
            forms: vec![(key.to_string(), Codec::KeyBinding, false)],
        }
    }

    /// The keys to write the setting under in `file`: those it has, and those the
    /// version has.
    fn applicable<'a>(
        &'a self,
        file: &'a OptionsFile,
    ) -> impl Iterator<Item = &'a (String, Codec, bool)> + 'a {
        self.forms
            .iter()
            .filter(|(key, _, exists)| *exists || file.get(key).is_some())
    }

    /// The canonical value the file has: from the key the version uses if present,
    /// else from any older key it still has.
    fn current(&self, file: &OptionsFile, dv: u32) -> Option<String> {
        let read = |(key, codec, _): &(String, Codec, bool)| {
            file.get(key).and_then(|raw| format::read(*codec, dv, raw))
        };
        self.forms
            .iter()
            .filter(|form| form.2)
            .find_map(read)
            .or_else(|| self.forms.iter().find_map(read))
    }

    /// Writes `value` into `file`; returns whether a line changed.
    fn write(&self, file: &mut OptionsFile, dv: u32, value: &str) -> bool {
        let mut changed = false;
        let forms: Vec<(String, Codec)> = self
            .applicable(file)
            .map(|(key, codec, _)| (key.clone(), *codec))
            .collect();
        for (key, codec) in forms {
            let Some(raw) = format::write(codec, dv, value) else {
                continue;
            };
            let same = file
                .get(&key)
                .and_then(|current| format::read(codec, dv, current))
                .is_some_and(|current| format::same(&current, value));
            if !same {
                file.set(&key, &raw);
                changed = true;
            }
        }
        changed
    }
}

/// Every shared setting that applies to a file at `dv`: the table's, plus mod key
/// bindings from `extra` (the global set) and from the file.
fn settings<'a>(
    dv: u32,
    extra: impl IntoIterator<Item = &'a String>,
    file: &OptionsFile,
) -> Vec<Setting> {
    let mut out: Vec<Setting> = keys::SPECS
        .iter()
        .map(|spec| Setting::from_spec(spec, dv))
        .collect();
    let mut others: BTreeSet<String> = extra
        .into_iter()
        .filter(|key| is_other_binding(key))
        .cloned()
        .collect();
    others.extend(
        file.keys()
            .filter(|key| is_other_binding(key))
            .map(str::to_string),
    );
    out.extend(others.iter().map(|key| Setting::other_binding(key)));
    out
}

fn setting_for(key: &str, dv: u32) -> Option<Setting> {
    match find(key) {
        Some(spec) => Some(Setting::from_spec(spec, dv)),
        None if is_other_binding(key) => Some(Setting::other_binding(key)),
        None => None,
    }
}

/// What [`before_launch`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchOutcome {
    /// Lines written.
    pub written: usize,
    /// Whether `options.txt` was created.
    pub created: bool,
    /// Why nothing was merged, if so.
    pub skipped: Option<String>,
    /// What a pending read-back (from a game the launcher didn't see exit) did.
    pub recovered: Option<ExitOutcome>,
}

impl LaunchOutcome {
    /// A line for the instance's console, if there is something to say.
    pub fn note(&self) -> Option<String> {
        if let Some(reason) = &self.skipped {
            return Some(format!("Shared game settings: {reason}"));
        }
        (self.written > 0).then(|| {
            format!(
                "Shared game settings: {} value{} written to options.txt{}",
                self.written,
                if self.written == 1 { "" } else { "s" },
                if self.created { " (new file)" } else { "" }
            )
        })
    }
}

/// What [`after_exit`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ExitOutcome {
    /// Nothing to read back (sharing off, no launch recorded, or nothing changed).
    #[default]
    Nothing,
    /// The preferred instance: these keys went into the global set.
    Emitted(Vec<String>),
    /// Another instance: these keys were changed in-game and will be replaced next
    /// launch unless kept as overrides.
    Changed(Vec<String>),
}

/// Merges the global values into the instance's `options.txt` before the game starts:
/// every shared category, except the instance's overrides. Creates the file if needed;
/// keeps every other line. Then records what the file holds, for [`after_exit`].
pub fn before_launch(
    data: &DataDir,
    instance_id: &str,
    instance_dir: &Path,
    game_version: &str,
    jar: Option<&Path>,
) -> Result<LaunchOutcome, SharedError> {
    let _instance = lock(instance_dir)?;
    let _profile = lock(data.root())?;
    let mut outcome = LaunchOutcome::default();
    // A game whose exit the launcher didn't see (it quit or crashed) is read back first.
    if load_instance(instance_dir)?.session.is_some() {
        outcome.recovered = Some(read_back(data, instance_id, instance_dir, game_version)?);
    }
    let mut sharing = load_instance(instance_dir)?;
    // Whatever the last game changed is about to be replaced.
    sharing.changed.clear();
    if !sharing.enabled {
        sharing.session = None;
        storage::save(&instance_path(instance_dir), &sharing)?;
        return Ok(outcome);
    }
    let profile = read_profile(data)?;
    let path = instance_dir.join("minecraft").join(options::FILE);
    let exists = path.is_file();
    let mut file = OptionsFile::read(&path)?;
    let Some(target) = target(&file, game_version, jar) else {
        outcome.skipped = Some(format!(
            "couldn't tell which options format Minecraft {game_version} uses, so nothing was changed"
        ));
        storage::save(&instance_path(instance_dir), &sharing)?;
        return Ok(outcome);
    };
    let dv = target.dv;
    let all = settings(dv, profile.values.keys(), &file);
    let active = |setting: &&Setting| {
        profile.shares(setting.category) && !sharing.overrides.contains(&setting.key)
    };
    for setting in all.iter().filter(active) {
        if let Some(value) = profile.values.get(&setting.key)
            && setting.write(&mut file, dv, value)
        {
            outcome.written += 1;
        }
    }
    if outcome.written > 0 {
        if let Some(version) = target.add_version_line {
            file.set_first("version", &version.to_string());
        }
        file.write(&path)?;
        outcome.created = !exists;
    }
    sharing.session = Some(
        all.iter()
            .filter(active)
            .filter_map(|setting| Some((setting.key.clone(), setting.current(&file, dv)?)))
            .collect(),
    );
    storage::save(&instance_path(instance_dir), &sharing)?;
    Ok(outcome)
}

/// Reads the instance's `options.txt` after the game exited and compares it with what
/// [`before_launch`] left. The preferred instance's changes (and shared keys the global
/// set doesn't have yet) go into the global set; another instance's changes are
/// recorded in [`InstanceSharing::changed`].
pub fn after_exit(
    data: &DataDir,
    instance_id: &str,
    instance_dir: &Path,
    game_version: &str,
) -> Result<ExitOutcome, SharedError> {
    let _instance = lock(instance_dir)?;
    let _profile = lock(data.root())?;
    read_back(data, instance_id, instance_dir, game_version)
}

/// [`after_exit`] for a caller that holds the instance's lock and the global set's.
fn read_back(
    data: &DataDir,
    instance_id: &str,
    instance_dir: &Path,
    game_version: &str,
) -> Result<ExitOutcome, SharedError> {
    let mut sharing = load_instance(instance_dir)?;
    let Some(session) = sharing.session.take() else {
        return Ok(ExitOutcome::Nothing);
    };
    let save = |sharing: &InstanceSharing| storage::save(&instance_path(instance_dir), sharing);
    if !sharing.enabled {
        save(&sharing)?;
        return Ok(ExitOutcome::Nothing);
    }
    let file = OptionsFile::read(&instance_dir.join("minecraft").join(options::FILE))?;
    // The game rewrote the file with its own version line.
    let Some(Target { dv, .. }) = target(&file, game_version, None) else {
        save(&sharing)?;
        return Ok(ExitOutcome::Nothing);
    };
    let profile = read_profile(data)?;
    let preferred = profile.preferred.as_deref() == Some(instance_id);
    let mut changed = BTreeMap::new();
    let mut new = BTreeMap::new();
    for (key, before) in &session {
        let Some(setting) = setting_for(key, dv) else {
            continue;
        };
        if !profile.shares(setting.category) || sharing.overrides.contains(key) {
            continue;
        }
        let Some(now) = setting.current(&file, dv) else {
            continue;
        };
        if !format::same(&now, before) {
            changed.insert(key.clone(), now);
        } else if !profile.values.contains_key(key) {
            new.insert(key.clone(), now);
        }
    }
    if preferred {
        // Shared keys the file didn't have at launch: a new instance's first game writes
        // the whole file, and the global set takes what it doesn't have yet.
        for setting in settings(dv, std::iter::empty(), &file) {
            if session.contains_key(&setting.key)
                || profile.values.contains_key(&setting.key)
                || !profile.shares(setting.category)
                || sharing.overrides.contains(&setting.key)
            {
                continue;
            }
            if let Some(now) = setting.current(&file, dv) {
                new.insert(setting.key, now);
            }
        }
    }
    let outcome = if preferred {
        let mut keys: Vec<String> = changed.keys().cloned().collect();
        if !changed.is_empty() || !new.is_empty() {
            change_profile(data, |profile| {
                profile.values.extend(new.clone());
                profile.values.extend(changed.clone());
            })?;
        }
        keys.extend(new.into_keys());
        keys.sort();
        if keys.is_empty() {
            ExitOutcome::Nothing
        } else {
            ExitOutcome::Emitted(keys)
        }
    } else if changed.is_empty() {
        ExitOutcome::Nothing
    } else {
        sharing.changed = changed;
        ExitOutcome::Changed(sharing.changed.keys().cloned().collect())
    };
    save(&sharing)?;
    Ok(outcome)
}

/// "Keep them as overrides": the keys changed in-game keep the instance's values.
pub fn keep_changes_as_overrides(instance_dir: &Path) -> Result<InstanceSharing, SharedError> {
    update_instance(instance_dir, |sharing| {
        let changed = std::mem::take(&mut sharing.changed);
        sharing.overrides.extend(changed.into_keys());
    })
}

/// "Dismiss": the next launch replaces the keys changed in-game.
pub fn dismiss_changes(instance_dir: &Path) -> Result<InstanceSharing, SharedError> {
    update_instance(instance_dir, |sharing| sharing.changed.clear())
}

/// Turns the override of `keys` on or off.
pub fn set_overrides<'a>(
    instance_dir: &Path,
    keys: impl IntoIterator<Item = &'a str>,
    overridden: bool,
) -> Result<InstanceSharing, SharedError> {
    update_instance(instance_dir, |sharing| {
        for key in keys {
            if overridden {
                sharing.overrides.insert(key.to_string());
            } else {
                sharing.overrides.remove(key);
            }
            sharing.changed.remove(key);
        }
    })
}

/// The instance's `options.txt` with its format, for showing and copying its values.
pub fn read_instance_options(
    instance_dir: &Path,
    game_version: &str,
    jar: Option<&Path>,
) -> Result<(OptionsFile, Option<Target>), SharedError> {
    let file = OptionsFile::read(&instance_dir.join("minecraft").join(options::FILE))?;
    let target = target(&file, game_version, jar);
    Ok((file, target))
}

/// The canonical values of the shared settings in an instance's `options.txt`.
pub fn instance_values(file: &OptionsFile, dv: u32) -> BTreeMap<String, String> {
    settings(dv, std::iter::empty(), file)
        .iter()
        .filter_map(|setting| Some((setting.key.clone(), setting.current(file, dv)?)))
        .collect()
}

/// "Copy from instance…": the global set takes every shared value the instance's
/// `options.txt` has. Returns how many.
pub fn copy_from_instance(
    data: &DataDir,
    instance_dir: &Path,
    game_version: &str,
    jar: Option<&Path>,
) -> Result<usize, SharedError> {
    let (file, target) = read_instance_options(instance_dir, game_version, jar)?;
    let Some(target) = target else {
        return Err(SharedError::Invalid(
            "That instance has no settings yet: play it once first.".into(),
        ));
    };
    let values = instance_values(&file, target.dv);
    if values.is_empty() {
        return Err(SharedError::Invalid(
            "That instance has no settings yet: play it once first.".into(),
        ));
    }
    let count = values.len();
    update_profile(data, |profile| profile.values.extend(values))?;
    Ok(count)
}

/// "Reset": forgets every global value. Instances keep what they have.
pub fn reset(data: &DataDir) -> Result<Profile, SharedError> {
    update_profile(data, |profile| profile.values.clear())
}

/// Sets one global value, typed in the launcher.
pub fn set_value(data: &DataDir, key: &str, text: &str) -> Result<Profile, SharedError> {
    let value = parse_value(key, text)?;
    update_profile(data, |profile| {
        profile.values.insert(key.to_string(), value);
    })
}

/// The keys "Keep the narrator off" shares: the narrator off, and the game's first-start
/// accessibility screen (which the narrator reads aloud) skipped.
pub const NARRATOR_OFF: [(&str, &str); 2] = [("narrator", "0"), ("onboardAccessibility", "false")];

/// Whether the global set keeps the narrator off in every instance.
pub fn narrator_off(profile: &Profile) -> bool {
    NARRATOR_OFF
        .iter()
        .all(|(key, value)| profile.values.get(*key).map(String::as_str) == Some(*value))
}

/// Keeps the narrator off in every instance, new ones included, or stops sharing those
/// values so each instance keeps its own.
pub fn set_narrator_off(data: &DataDir, off: bool) -> Result<Profile, SharedError> {
    update_profile(data, |profile| {
        for (key, value) in NARRATOR_OFF {
            if off {
                profile.values.insert(key.to_string(), value.to_string());
            } else {
                profile.values.remove(key);
            }
        }
    })
}

fn parse_value(key: &str, text: &str) -> Result<String, SharedError> {
    match find(key) {
        Some(spec) => format::parse_input(spec, text).map_err(SharedError::Invalid),
        None if is_other_binding(key) => {
            format::parse_input(find("key_key.forward").expect("a binding"), text)
                .map_err(SharedError::Invalid)
        }
        None => Err(SharedError::Invalid(format!(
            "{key} isn't a shared setting"
        ))),
    }
}

/// Sets an instance's own value of an overridden key, typed in the launcher.
pub fn set_instance_value(
    instance_dir: &Path,
    game_version: &str,
    jar: Option<&Path>,
    key: &str,
    text: &str,
) -> Result<(), SharedError> {
    let value = parse_value(key, text)?;
    let _lock = lock(instance_dir)?;
    let path = instance_dir.join("minecraft").join(options::FILE);
    let mut file = OptionsFile::read(&path)?;
    let Some(target) = target(&file, game_version, jar) else {
        return Err(SharedError::Invalid(
            "Play the instance once first, so it has its settings file.".into(),
        ));
    };
    let setting = setting_for(key, target.dv)
        .ok_or_else(|| SharedError::Invalid(format!("{key} isn't a shared setting")))?;
    if setting.applicable(&file).next().is_none() {
        return Err(SharedError::Invalid(format!(
            "Minecraft {game_version} doesn't have this setting."
        )));
    }
    if setting.write(&mut file, target.dv, &value) {
        if let Some(version) = target.add_version_line {
            file.set_first("version", &version.to_string());
        }
        file.write(&path)?;
    }
    Ok(())
}

/// Whether `key` can be set in a file at data version `dv`: a key of the table that
/// exists there (under any of its names). Mod key bindings depend on the file.
pub fn exists_at(key: &str, dv: u32) -> bool {
    find(key).is_some_and(|spec| spec.forms().any(|form| form.exists_at(dv)))
}

/// A row of the shared settings lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub label: String,
    pub category: Category,
    /// The global value, canonical.
    pub shared: Option<String>,
    /// The instance's value, canonical.
    pub local: Option<String>,
    pub overridden: bool,
}

impl Row {
    /// The value as the launcher shows it.
    pub fn show(&self, value: &str) -> String {
        match find(&self.key) {
            Some(spec) => format::display(spec, value),
            None => keycodes::label(value),
        }
    }

    /// How the launcher lets the user change the value. A mod's key binding is typed.
    pub fn control(&self) -> format::Control {
        find(&self.key).map_or(format::Control::Text, format::control)
    }
}

/// The rows to list: for the launcher's Settings (`instance` `None`), the settings the
/// global set has; for an instance, also those its file has.
pub fn rows(
    profile: &Profile,
    instance: Option<(&InstanceSharing, &BTreeMap<String, String>)>,
) -> Vec<Row> {
    let local = instance.map(|(_, values)| values);
    let mut out: Vec<Row> = keys::SPECS
        .iter()
        .filter_map(|spec| {
            let shared = profile.values.get(spec.key).cloned();
            let local = local.and_then(|values| values.get(spec.key).cloned());
            (shared.is_some() || local.is_some()).then(|| Row {
                key: spec.key.to_string(),
                label: spec.label.to_string(),
                category: spec.category,
                shared,
                local,
                overridden: instance
                    .is_some_and(|(sharing, _)| sharing.overrides.contains(spec.key)),
            })
        })
        .collect();
    let mut others: BTreeSet<&String> = profile
        .values
        .keys()
        .filter(|key| is_other_binding(key))
        .collect();
    if let Some(values) = local {
        others.extend(values.keys().filter(|key| is_other_binding(key)));
    }
    for key in others {
        out.push(Row {
            key: key.clone(),
            label: key.strip_prefix("key_").unwrap_or(key).to_string(),
            category: Category::Controls,
            shared: profile.values.get(key).cloned(),
            local: local.and_then(|values| values.get(key).cloned()),
            overridden: instance.is_some_and(|(sharing, _)| sharing.overrides.contains(key)),
        });
    }
    out
}

#[cfg(test)]
mod tests;
