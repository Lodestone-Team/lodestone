//! World backups (`docs/instance-images.md`, "World backups"): a plain `.zip` of one world
//! folder, made by hand (and one made on its own before a world backup is restored), kept
//! until the user deletes it. Unlike a snapshot (an image of the whole instance folder in
//! the image store, [`crate::images`]), it holds one world, shares nothing with other
//! backups, and opens with any zip tool.
//!
//! **Where.** `<data>/world-backups/<instance id>/<world folder>/<id>.zip`, outside the
//! instance folder so snapshots, exports and "Host this" never take them, beside its
//! `<id>.json` ([`WorldBackup`]). The id has a snapshot record id's shape
//! (`YYYYMMDD-HHMMSS-xxxx`, UTC); the zip's one top folder is the world's folder, so it
//! unzips into any `saves` folder. The zip is written as a `.part` file, flushed and
//! renamed, and its `.json` last ([`crate::storage::write_atomic`]): a backup is listed
//! only once both are there, never half a zip.
//!
//! **Whose.** A backup belongs to the instance whose id its folder names and whose
//! `created` time its `.json` holds. An instance put back from the Trash has both, so it
//! has its backups again; a new instance that only reuses the id has another `created`
//! and never sees them. A backup no live instance owns is a deleted instance's, which
//! Settings › Storage lists and deletes for good; an id whose instance folder is there but
//! can't be read (a broken `instance.json`, a drive unplugged under a link) is never taken
//! for a deleted one.
//!
//! **Locks.** Making, restoring, deleting and the startup sweep run inside the instance's
//! record lock and a counted write of the image store ([`Images::with_instance_locked`]),
//! so they never overlap a snapshot, a restore or one another, and a data move waits for
//! them or refuses them. Listing only reads files written whole.
//!
//! [`restore`] puts a backup back: in place through M5's journaled swap, after a safety
//! world backup of the world as it is, or beside it as a new world.

pub mod restore;
#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde::{Deserialize, Serialize};

pub use restore::{RestoreTo, WorldRestored, WorldStep};

use crate::{
    content::{InstanceDirs, is_plain_name},
    game::download::Progress,
    images::{Cancel, ImageError, Images, labels, snapshots},
    instances::{Instance, InstanceStore},
    paths::DataDir,
    storage,
    worlds::{self, Limits, Shape, WorldError},
};

/// The `.json` beside each zip is at most this large; a larger one is ignored.
pub const MAX_SIDECAR_BYTES: u64 = 64 * 1024;
/// A name is cut to this many characters.
pub const MAX_NAME_CHARS: usize = 200;
/// A `.part` older than this is a crash's, and the sweep deletes it.
const STALE_PART: Duration = Duration::from_secs(60 * 60);
const VERSION: u32 = 1;

/// How a world backup was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldBackupKind {
    /// By the user (Back up world, the Worlds tab's Back up, the CLI), or a zip found
    /// without its `.json` and kept.
    Manual,
    /// The safety backup made before another world backup of the world was restored.
    BeforeRestore,
}

/// One world backup: its `<id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldBackup {
    pub version: u32,
    /// `YYYYMMDD-HHMMSS-xxxx`: the zip is `<id>.zip`.
    pub id: String,
    /// The owning instance's `created` time (its id is the folder's name).
    pub instance_created: u64,
    /// The instance's name when it was made, for the deleted instances' list.
    pub instance_name: String,
    /// The world's folder in `saves/` (the folder the backup is kept in wins).
    pub folder: String,
    /// The user's name, or a label ([`labels::WORLD_BACKUP`],
    /// [`labels::before_restoring`]). Plain text: show it through `display_text`.
    pub name: String,
    pub kind: WorldBackupKind,
    /// For a safety backup: the name of the backup whose restore it came before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored: Option<String>,
    /// Unix seconds.
    pub created: i64,
    /// What the world's `level.dat` said.
    #[serde(default)]
    pub level_name: Option<String>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub data_version: Option<i64>,
    /// The zip's size.
    pub bytes: u64,
    /// The bytes of the files zipped.
    pub world_bytes: u64,
    /// How many files it holds.
    pub files: u64,
    /// Names the zip left out (links, names no disk keeps everywhere), at most
    /// [`worlds::zip::MAX_LEFT_OUT_NAMES`].
    #[serde(default)]
    pub left_out: Vec<String>,
    #[serde(default)]
    pub left_out_count: u64,
}

impl WorldBackup {
    /// The world's own name, else its folder.
    pub fn world_name(&self) -> &str {
        self.level_name.as_deref().unwrap_or(&self.folder)
    }
}

/// A deleted instance's world backups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeletedWorldBackups {
    pub instance_id: String,
    pub instance_name: String,
    pub instance_created: u64,
    pub backups: Vec<WorldBackup>,
}

/// What [`WorldBackups::clean_deleted`] deleted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CleanedWorldBackups {
    pub backups: usize,
    pub bytes: u64,
}

/// The world backups' share of the data folder, for Settings › Storage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct WorldBackupsUsage {
    /// Everything in the world backups folder.
    pub bytes: u64,
    /// Live instances' backups.
    pub backups: usize,
    /// Deleted instances' backups, and their zips' bytes.
    pub deleted: usize,
    pub deleted_bytes: u64,
}

/// Why making, restoring or deleting a world backup failed.
#[derive(Debug, thiserror::Error)]
pub enum WorldBackupError {
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("the game is running; close it first")]
    Running,
    #[error("cancelled")]
    Cancelled,
    #[error("It needs {} MB and the disk has {} MB free", .needed.div_ceil(1_000_000), .free / 1_000_000)]
    NoRoom { needed: u64, free: u64 },
    #[error("world backups are paused while the data folder moves")]
    Suspended,
    #[error(transparent)]
    World(WorldError),
    #[error(transparent)]
    Image(ImageError),
}

impl WorldBackupError {
    fn io(what: impl Into<String>) -> impl FnOnce(io::Error) -> WorldBackupError {
        let what = what.into();
        move |source| WorldBackupError::Io { what, source }
    }
}

impl From<WorldError> for WorldBackupError {
    fn from(e: WorldError) -> Self {
        match e {
            WorldError::Cancelled => WorldBackupError::Cancelled,
            WorldError::NoRoom { needed, free } => WorldBackupError::NoRoom { needed, free },
            WorldError::NotFound(why) => WorldBackupError::NotFound(why),
            other => WorldBackupError::World(other),
        }
    }
}

impl From<ImageError> for WorldBackupError {
    fn from(e: ImageError) -> Self {
        match e {
            ImageError::Running => WorldBackupError::Running,
            ImageError::Cancelled => WorldBackupError::Cancelled,
            ImageError::Suspended => WorldBackupError::Suspended,
            ImageError::NoRoom { needed, free } => WorldBackupError::NoRoom { needed, free },
            other => WorldBackupError::Image(other),
        }
    }
}

/// The Launcher's world backups: see the module.
pub struct WorldBackups {
    root: PathBuf,
    images: Arc<Images>,
    watchers: Mutex<Vec<UnboundedSender<String>>>,
    /// The free space tests pretend the disks have.
    #[cfg(test)]
    pub(crate) fake_free: Mutex<Option<u64>>,
    /// Failures the tests make the next in-place restore's swap simulate.
    #[cfg(test)]
    pub(crate) faults: Mutex<Option<crate::images::swap::Faults>>,
    /// What the tests do just before the next in-place restore's swap, once it is staged
    /// and checked: another job writing into the world meanwhile.
    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(crate) before_swap: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

/// A backup's two files.
struct Found {
    backup: WorldBackup,
    zip: PathBuf,
    sidecar: PathBuf,
}

/// Which instances own backups now: live ones by (id, `created`), and ids whose folder is
/// there but couldn't be read, which own whatever they have.
struct Owners {
    live: HashMap<String, (u64, String)>,
    unknown: HashSet<String>,
    /// The instances folder itself couldn't be read (a drive that isn't there): nothing
    /// counts as deleted.
    blind: bool,
}

impl Owners {
    fn of(instances: &InstanceStore) -> Owners {
        Owners::listed(instances.root(), &instances.list().0)
    }

    /// The owner of the one instance id `id`, reading only its record.
    fn one(instances: &InstanceStore, id: &str) -> Owners {
        let root = instances.root();
        let mut owners = Owners {
            live: HashMap::new(),
            unknown: HashSet::new(),
            blind: !root.is_dir(),
        };
        match instances.get(id) {
            Ok(instance) => {
                owners
                    .live
                    .insert(id.to_string(), (instance.created, instance.name));
            }
            Err(_) if fs::symlink_metadata(root.join(id)).is_ok() => {
                owners.unknown.insert(id.to_string());
            }
            Err(_) => {}
        }
        owners
    }

    /// The owners given the instances folder `root` and the instances read from it.
    fn listed(root: &Path, list: &[Instance]) -> Owners {
        let blind = !root.is_dir();
        let live: HashMap<String, (u64, String)> = list
            .iter()
            .map(|i| (i.id.clone(), (i.created, i.name.clone())))
            .collect();
        // Any entry in the instances folder that isn't a live instance: broken, a link to
        // a drive that isn't there, half made. Never taken for a deleted instance.
        let unknown = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|id| !live.contains_key(id))
            .collect();
        Owners {
            live,
            unknown,
            blind,
        }
    }

    /// Whether a backup of `id` made by the instance created at `created` is a deleted
    /// instance's.
    fn deleted(&self, id: &str, created: u64) -> bool {
        !self.blind
            && !self.unknown.contains(id)
            && self.live.get(id).is_none_or(|(c, _)| *c != created)
    }
}

impl WorldBackups {
    /// The world backups of the Launcher whose data folder is `data`. Nothing is read or
    /// written until used.
    pub fn open(data: &DataDir, images: Arc<Images>) -> Arc<WorldBackups> {
        Arc::new(WorldBackups {
            root: data.world_backups_dir(),
            images,
            watchers: Mutex::default(),
            #[cfg(test)]
            fake_free: Mutex::default(),
            #[cfg(test)]
            faults: Mutex::default(),
            #[cfg(test)]
            before_swap: Mutex::default(),
        })
    }

    fn instances(&self) -> &InstanceStore {
        self.images.instances()
    }

    /// The ids of the instances whose world backups change from now on, one per change
    /// (made, restored with a safety backup, deleted, kept by the sweep). Stops when the
    /// receiver is dropped.
    pub fn watch(&self) -> UnboundedReceiver<String> {
        let (sender, receiver) = unbounded();
        self.watchers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(sender);
        receiver
    }

    fn changed(&self, instance: &str) {
        self.watchers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|w| w.unbounded_send(instance.to_string()).is_ok());
    }

    /// The free space on the disk holding `dir`, or what a test pretends.
    fn free(&self, dir: &Path) -> Option<u64> {
        #[cfg(test)]
        if let Some(free) = *self.fake_free.lock().unwrap_or_else(|e| e.into_inner()) {
            return Some(free);
        }
        dir.ancestors()
            .find(|d| d.is_dir())
            .and_then(crate::disk::free_space)
    }

    fn check_room(&self, dir: &Path, bytes: u64) -> Result<(), WorldBackupError> {
        Ok(worlds::check_room(dir, bytes, self.free(dir))?)
    }

    /// `<root>/<instance>/<folder>/`, for an id the store can name and a plain folder.
    fn folder_dir(&self, instance: &str, folder: &str) -> Result<PathBuf, WorldBackupError> {
        if !lodestone_image::store::valid_instance_id(instance) || !is_plain_name(folder) {
            return Err(WorldBackupError::NotFound(format!(
                "There is no world {folder:?}."
            )));
        }
        Ok(self.root.join(instance).join(folder))
    }

    /// Where the backup's zip is, for Show in Finder.
    pub fn zip_path(&self, instance: &str, backup: &WorldBackup) -> PathBuf {
        self.root
            .join(instance)
            .join(&backup.folder)
            .join(format!("{}.zip", backup.id))
    }

    /// The instance's world backups, newest first. A pure read.
    pub fn list(&self, instance: &str) -> Result<Vec<WorldBackup>, WorldBackupError> {
        let created = self.instance(instance)?.created;
        Ok(self.owned(instance, created).map(|f| f.backup).collect())
    }

    fn owned(&self, instance: &str, created: u64) -> impl Iterator<Item = Found> {
        found_in(&self.root, instance)
            .into_iter()
            .filter(move |f| f.backup.instance_created == created)
    }

    fn instance(&self, instance: &str) -> Result<Instance, WorldBackupError> {
        self.instances()
            .get(instance)
            .map_err(|_| WorldBackupError::NotFound(format!("There is no instance {instance:?}.")))
    }

    /// One backup of the instance, by id.
    fn find(&self, instance: &str, id: &str) -> Result<Found, WorldBackupError> {
        let created = self.instance(instance)?.created;
        if !snapshots::valid_record_id(id) {
            return Err(WorldBackupError::NotFound(format!(
                "There is no world backup {id:?}."
            )));
        }
        self.owned(instance, created)
            .find(|f| f.backup.id == id)
            .ok_or_else(|| WorldBackupError::NotFound(format!("There is no world backup {id:?}.")))
    }

    /// The world backups of deleted instances (see the module), by instance.
    pub fn list_deleted(&self) -> Vec<DeletedWorldBackups> {
        let owners = Owners::of(self.instances());
        let mut deleted: Vec<DeletedWorldBackups> = Vec::new();
        for id in read_names(&self.root) {
            for found in found_in(&self.root, &id) {
                let b = found.backup;
                if !owners.deleted(&id, b.instance_created) {
                    continue;
                }
                match deleted
                    .iter_mut()
                    .find(|d| d.instance_id == id && d.instance_created == b.instance_created)
                {
                    Some(group) => group.backups.push(b),
                    None => deleted.push(DeletedWorldBackups {
                        instance_id: id.clone(),
                        instance_name: b.instance_name.clone(),
                        instance_created: b.instance_created,
                        backups: vec![b],
                    }),
                }
            }
        }
        deleted
    }

    /// Makes a world backup of the world `folder` of `instance`, named `name` (blank is
    /// [`labels::WORLD_BACKUP`]). Refused while the game runs or has the world open, and
    /// without room for a copy of the world and 1 GiB; stopped when Cancel is pressed, a
    /// game session begins or the world opens, leaving nothing. Blocking.
    pub fn create(
        &self,
        instance: &str,
        folder: &str,
        name: Option<&str>,
        progress: &Progress,
        cancel: Cancel,
    ) -> Result<WorldBackup, WorldBackupError> {
        self.images.with_instance_locked(instance, |locked| {
            let made = self.create_locked(
                &locked.slot,
                instance,
                folder,
                Making {
                    name: backup_name(name),
                    kind: WorldBackupKind::Manual,
                    restored: None,
                },
                None,
                progress,
                cancel,
            )?;
            self.changed(instance);
            Ok(made.0)
        })
    }

    /// [`Self::create`] for a caller holding the record lock inside a counted write, with
    /// what [`worlds::zip::plan`] listed when the caller has it. Returns the files zipped
    /// too.
    ///
    /// A manual backup of a folder without `level.dat` is refused: no restore could take
    /// it. The safety backup before a restore is made anyway (the world may be broken, the
    /// very reason for restoring), keeping the files the restore replaces; such a zip opens
    /// with any zip tool.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_locked(
        &self,
        slot: &crate::images::Slot,
        instance: &str,
        folder: &str,
        making: Making,
        plan: Option<worlds::zip::ZipPlan>,
        progress: &Progress,
        cancel: Cancel,
    ) -> Result<(WorldBackup, Vec<String>), WorldBackupError> {
        if slot.in_session() {
            return Err(WorldBackupError::Running);
        }
        let owner = self.instance(instance)?;
        let dirs = InstanceDirs::new(self.instances().dir(instance));
        let world = worlds::world_dir(&dirs, folder)?;
        worlds::zip::check_world_folder(folder)?;
        if making.kind == WorldBackupKind::Manual && !world.join("level.dat").is_file() {
            return Err(WorldBackupError::Invalid(
                "That world's folder has no level.dat, so a backup of it couldn't be restored."
                    .into(),
            ));
        }
        let saves = worlds::saves_dir(&dirs);
        let only = [folder.to_string()];
        if worlds::open_in_game(&saves, Some(&only)).is_some() {
            return Err(WorldBackupError::Running);
        }
        let dir = self.folder_dir(instance, folder)?;
        let plan = match plan {
            Some(plan) => plan,
            None => worlds::zip::plan(&world, folder)?,
        };
        self.check_room(&dir, plan.bytes())?;
        fs::create_dir_all(&dir).map_err(WorldBackupError::io(format!(
            "Failed to create {}",
            dir.display()
        )))?;
        let created = unix_now();
        let id = loop {
            let id = snapshots::new_record_id(created, &[]);
            if !dir.join(format!("{id}.zip")).exists() && !dir.join(format!("{id}.json")).exists() {
                break id;
            }
        };
        let zip = dir.join(format!("{id}.zip"));
        // A session that begins, or the game opening the world (one this Launcher didn't
        // start), stops the zip at the next file: it would read the world mid-write.
        let stop = || {
            if cancel() {
                Err(WorldError::Cancelled)
            } else if slot.in_session() || worlds::open_in_game(&saves, Some(&only)).is_some() {
                Err(WorldError::Invalid(RUNNING.into()))
            } else {
                Ok(())
            }
        };
        let written = worlds::zip::write_planned(&world, folder, plan, &zip, progress, &stop)
            .map_err(|e| match e {
                WorldError::Invalid(why) if why == RUNNING => WorldBackupError::Running,
                other => other.into(),
            })?;
        let summary = worlds::read_world(&world).and_then(|w| w.summary);
        let backup = WorldBackup {
            version: VERSION,
            id: id.clone(),
            instance_created: owner.created,
            instance_name: owner.name,
            folder: folder.to_string(),
            name: making.name,
            kind: making.kind,
            restored: making.restored,
            created,
            level_name: summary.as_ref().and_then(|s| s.name.clone()),
            game_version: summary.as_ref().and_then(|s| s.version_name.clone()),
            data_version: summary.as_ref().and_then(|s| s.data_version),
            bytes: written.bytes,
            world_bytes: written.world_bytes,
            files: written.files.len() as u64,
            left_out: written.left_out,
            left_out_count: written.left_out_count,
        };
        if let Err(e) = write_sidecar(&dir.join(format!("{id}.json")), &backup) {
            let _ = fs::remove_file(&zip);
            return Err(e);
        }
        Ok((backup, written.files))
    }

    /// Deletes a backup this process just made, for good (a restore that left the world as
    /// it was deletes its safety backup). The caller holds the record lock.
    fn remove_made(&self, instance: &str, backup: &WorldBackup) {
        let zip = self.zip_path(instance, backup);
        let _ = fs::remove_file(&zip);
        let _ = fs::remove_file(zip.with_extension("json"));
        prune(zip.parent());
        self.changed(instance);
    }

    /// Renames the world `folder` of `instance` to `name` ([`worlds::rename`]) and moves
    /// its world backups with it, under the instance's record lock: its backups stay its
    /// own, and a world later given the old folder name never takes them. The new folder is
    /// a name free in both `saves/` and the world backups' folder. If the backups can't be
    /// moved, the world is put back as it was. Refused while a game session runs. Returns
    /// the world's folder now. Blocking.
    pub fn rename_world(
        &self,
        instance: &str,
        folder: &str,
        name: &str,
    ) -> Result<String, WorldBackupError> {
        self.images.with_instance_locked(instance, |locked| {
            if locked.slot.in_session() {
                return Err(WorldBackupError::Running);
            }
            let dirs = InstanceDirs::new(&locked.dir);
            let world = worlds::world_dir(&dirs, folder)?;
            let backups = self.root.join(instance);
            let from_backups = backups.join(folder);
            let has_backups = fs::symlink_metadata(&from_backups).is_ok_and(|m| m.is_dir());
            let renamed = worlds::rename_in(&dirs, folder, name, &[&backups], |from, to| {
                if !has_backups {
                    return crate::fsx::rename(from, to);
                }
                let to_name = to.file_name().map(PathBuf::from).unwrap_or_default();
                let from_name = from.file_name().map(PathBuf::from).unwrap_or_default();
                if from == world {
                    // Forward: the world, then its backups; the world back if they can't.
                    crate::fsx::rename(from, to)?;
                    if let Err(e) = crate::fsx::rename(&from_backups, backups.join(&to_name)) {
                        let _ = crate::fsx::rename(to, from);
                        return Err(e);
                    }
                    Ok(())
                } else {
                    // Putting it back (its level.dat couldn't be renamed): both again.
                    crate::fsx::rename(backups.join(&from_name), backups.join(&to_name))?;
                    crate::fsx::rename(from, to)
                }
            })?;
            if renamed != folder && has_backups {
                self.changed(instance);
            }
            Ok(renamed)
        })
    }

    /// Moves the backup's zip to the Trash, then deletes its `.json`. Blocking.
    pub fn delete(&self, instance: &str, id: &str) -> Result<(), WorldBackupError> {
        self.delete_with(instance, id, |path| {
            trash::delete(path).map_err(|e| e.to_string())
        })
    }

    /// [`Self::delete`] with `remove` doing the removal (tests don't use the Trash). If it
    /// fails, nothing changes; a crash before the `.json` goes leaves it without its zip,
    /// which nothing lists and the sweep deletes.
    pub(crate) fn delete_with(
        &self,
        instance: &str,
        id: &str,
        remove: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), WorldBackupError> {
        self.images.with_instance_locked(instance, |_| {
            let found = self.find(instance, id)?;
            remove(&found.zip).map_err(|reason| {
                WorldBackupError::Invalid(format!(
                    "Couldn't move {} to the {}: {reason}",
                    found.zip.display(),
                    crate::wording::TRASH
                ))
            })?;
            let _ = fs::remove_file(&found.sidecar);
            prune(found.sidecar.parent());
            self.changed(instance);
            Ok(())
        })
    }

    /// Deletes for good every deleted instance's world backups (see the module), each
    /// instance's under its record lock. Blocking.
    pub fn clean_deleted(&self) -> Result<CleanedWorldBackups, WorldBackupError> {
        let mut cleaned = CleanedWorldBackups::default();
        for id in read_names(&self.root) {
            if !self.images.accepts(&id) {
                continue;
            }
            self.images
                .with_instance_locked(&id, |_| -> Result<(), WorldBackupError> {
                    // Looked at again under the lock: an instance put back meanwhile keeps them.
                    let owners = Owners::one(self.instances(), &id);
                    for found in found_in(&self.root, &id) {
                        if !owners.deleted(&id, found.backup.instance_created) {
                            continue;
                        }
                        let bytes = fs::metadata(&found.zip).map_or(0, |m| m.len());
                        fs::remove_file(&found.zip).map_err(WorldBackupError::io(format!(
                            "Failed to delete {}",
                            found.zip.display()
                        )))?;
                        let _ = fs::remove_file(&found.sidecar);
                        prune(found.sidecar.parent());
                        cleaned.backups += 1;
                        cleaned.bytes += bytes;
                    }
                    Ok(())
                })?;
        }
        Ok(cleaned)
    }

    /// Tidies up after crashes, each instance id under its record lock: deletes `.part`
    /// files older than an hour and `.json` files without their zip, and keeps a zip found
    /// without its `.json` (the crash came between them; it is the user's copy) as a
    /// manual backup of its live instance. Run in the background when the Launcher
    /// starts. Errors are logged.
    pub fn sweep(&self) {
        let owners = Owners::of(self.instances());
        for id in read_names(&self.root) {
            if !self.images.accepts(&id) {
                continue;
            }
            let swept =
                self.images
                    .with_instance_locked(&id, |_| -> Result<bool, WorldBackupError> {
                        Ok(self.sweep_locked(&id, owners.live.get(&id)))
                    });
            match swept {
                Ok(true) => self.changed(&id),
                Ok(false) => {}
                Err(e) => crate::images::warn("world_backups.sweep", &id, &e.to_string()),
            }
        }
    }

    /// [`Self::sweep`] of one instance id; returns whether a backup was kept.
    fn sweep_locked(&self, id: &str, live: Option<&(u64, String)>) -> bool {
        let mut kept = false;
        let dir = self.root.join(id);
        for folder in read_names(&dir).into_iter().filter(|n| is_plain_name(n)) {
            let folder_dir = dir.join(&folder);
            for name in read_names(&folder_dir) {
                let path = folder_dir.join(&name);
                if name.starts_with('.') && name.ends_with(".part") {
                    let old = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > STALE_PART));
                    if old {
                        let _ = fs::remove_file(&path);
                    }
                } else if let Some(stem) = name.strip_suffix(".json") {
                    if snapshots::valid_record_id(stem)
                        && !folder_dir.join(format!("{stem}.zip")).exists()
                    {
                        let _ = fs::remove_file(&path);
                    }
                } else if let Some(stem) = name.strip_suffix(".zip")
                    && snapshots::valid_record_id(stem)
                    && !folder_dir.join(format!("{stem}.json")).exists()
                    && let Some((created, instance_name)) = live
                    // Written by this instance, not by a deleted one whose id it reuses.
                    && written_at(&path).is_some_and(|at| at >= *created as i64)
                {
                    match adopt(&path, stem, &folder, *created, instance_name) {
                        Ok(backup) => {
                            if write_sidecar(&folder_dir.join(format!("{stem}.json")), &backup)
                                .is_ok()
                            {
                                kept = true;
                            }
                        }
                        Err(e) => crate::images::warn(
                            "world_backups.sweep",
                            id,
                            &format!("{} isn't a world backup: {e}", path.display()),
                        ),
                    }
                }
            }
            prune(Some(&folder_dir));
        }
        kept
    }

    /// The world backups' share of the data folder `data` (Settings › Storage), whose
    /// instances are `instances`, read without locks.
    pub fn usage(data: &DataDir, instances: &[Instance]) -> WorldBackupsUsage {
        let root = data.world_backups_dir();
        let owners = Owners::listed(&data.instances_dir(), instances);
        let mut usage = WorldBackupsUsage {
            bytes: worlds::folder_size(&root),
            ..WorldBackupsUsage::default()
        };
        for id in read_names(&root) {
            for found in found_in(&root, &id) {
                if owners.deleted(&id, found.backup.instance_created) {
                    usage.deleted += 1;
                    usage.deleted_bytes += fs::metadata(&found.zip).map_or(0, |m| m.len());
                } else if owners
                    .live
                    .get(&id)
                    .is_some_and(|(c, _)| *c == found.backup.instance_created)
                {
                    usage.backups += 1;
                }
            }
        }
        usage
    }
}

/// The backups in the instance id's folder under `root`, by world folder, whatever
/// instance made them, newest first. Only those with both files, a `.json` that reads, and
/// plain names.
fn found_in(root: &Path, instance: &str) -> Vec<Found> {
    let mut found = Vec::new();
    if !lodestone_image::store::valid_instance_id(instance) {
        return found;
    }
    let dir = root.join(instance);
    for folder in read_names(&dir).into_iter().filter(|n| is_plain_name(n)) {
        let folder_dir = dir.join(&folder);
        if !fs::symlink_metadata(&folder_dir).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        for name in read_names(&folder_dir) {
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if !snapshots::valid_record_id(id) {
                continue;
            }
            let zip = folder_dir.join(format!("{id}.zip"));
            if !fs::symlink_metadata(&zip).is_ok_and(|m| m.is_file()) {
                continue;
            }
            let sidecar = folder_dir.join(&name);
            if let Some(backup) = read_sidecar(&sidecar, id, &folder) {
                found.push(Found {
                    backup,
                    zip,
                    sidecar,
                });
            }
        }
    }
    found.sort_by(|a, b| {
        b.backup
            .created
            .cmp(&a.backup.created)
            .then_with(|| b.backup.id.cmp(&a.backup.id))
    });
    found
}

/// The message a zip stopped by the game carries until it is turned into
/// [`WorldBackupError::Running`].
const RUNNING: &str = "the game opened the world";

/// What a backup being made is called and why.
pub(crate) struct Making {
    pub(crate) name: String,
    pub(crate) kind: WorldBackupKind,
    pub(crate) restored: Option<String>,
}

/// A backup's name: the user's, trimmed and cut, else [`labels::WORLD_BACKUP`].
fn backup_name(name: Option<&str>) -> String {
    match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.chars().take(MAX_NAME_CHARS).collect(),
        None => labels::WORLD_BACKUP.to_string(),
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The names in `dir` (UTF-8 only), sorted; none when it can't be read.
fn read_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

/// Reads a backup's `.json`: bounded, with its id and folder matching where it is (the
/// folder it is in wins). `None` when it isn't one.
fn read_sidecar(path: &Path, id: &str, folder: &str) -> Option<WorldBackup> {
    use std::io::Read as _;
    let file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_SIDECAR_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_SIDECAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_SIDECAR_BYTES {
        return None;
    }
    let mut backup: WorldBackup = serde_json::from_slice(&bytes).ok()?;
    if backup.version != VERSION || backup.id != id {
        return None;
    }
    backup.folder = folder.to_string();
    backup.name = backup.name.chars().take(MAX_NAME_CHARS).collect();
    backup.left_out.truncate(worlds::zip::MAX_LEFT_OUT_NAMES);
    Some(backup)
}

fn write_sidecar(path: &Path, backup: &WorldBackup) -> Result<(), WorldBackupError> {
    let bytes = serde_json::to_vec_pretty(backup).map_err(|e| {
        WorldBackupError::Invalid(format!("Couldn't write the backup's details: {e}"))
    })?;
    storage::write_atomic(path, &bytes).map_err(WorldBackupError::io(format!(
        "Failed to write {}",
        path.display()
    )))
}

/// A `.json` for a zip found without one: a manual backup named [`labels::WORLD_BACKUP`],
/// what its world's `level.dat` says, made when the zip was written.
fn adopt(
    zip: &Path,
    id: &str,
    folder: &str,
    instance_created: u64,
    instance_name: &str,
) -> Result<WorldBackup, WorldError> {
    let checked = worlds::zip::check(zip, Shape::OneWorldFolder, Limits::default())?;
    let created = written_at(zip).unwrap_or(0);
    Ok(WorldBackup {
        version: VERSION,
        id: id.to_string(),
        instance_created,
        instance_name: instance_name.to_string(),
        folder: folder.to_string(),
        name: labels::WORLD_BACKUP.to_string(),
        kind: WorldBackupKind::Manual,
        restored: None,
        created,
        level_name: checked.summary.name.clone(),
        game_version: checked.summary.version_name.clone(),
        data_version: checked.summary.data_version,
        bytes: fs::metadata(zip).map_or(0, |m| m.len()),
        world_bytes: checked.declared_bytes,
        files: checked.files().count() as u64,
        left_out: Vec::new(),
        left_out_count: 0,
    })
}

/// When the file at `path` was last written, in Unix seconds.
fn written_at(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Whether `a` and `b` (or the nearest folders above them that exist) are on one disk.
/// When that can't be told, they are taken to be: room is then checked for both at once.
pub(crate) fn same_disk(a: &Path, b: &Path) -> bool {
    let near = |p: &Path| p.ancestors().find(|d| d.is_dir()).map(Path::to_path_buf);
    let (Some(a), Some(b)) = (near(a), near(b)) else {
        return true;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (fs::metadata(&a), fs::metadata(&b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev(),
            _ => true,
        }
    }
    #[cfg(not(unix))]
    {
        let volume = |p: &Path| {
            fs::canonicalize(p)
                .ok()
                .and_then(|p| p.components().next().map(|c| c.as_os_str().to_owned()))
        };
        match (volume(&a), volume(&b)) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

/// Removes a world's backup folder when it is empty now, and then its instance's.
fn prune(folder: Option<&Path>) {
    let mut dir = folder;
    for _ in 0..2 {
        let Some(d) = dir else { return };
        if fs::remove_dir(d).is_err() {
            return;
        }
        dir = d.parent();
    }
}
