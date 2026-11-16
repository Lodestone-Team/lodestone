//! Restoring a world backup: in place, or beside the world as a new one.
//!
//! **In place** ([`RestoreTo::InPlace`]) runs as a snapshot's restore does
//! ([`crate::images::restore`]): under the instance's record lock, never while its game runs
//! or any of its worlds is open, an unfinished restore undone first. The zip is checked
//! whole before anything is written ([`worlds::zip::check`]), then the disks' room, then
//! the **safety world backup** of the world as it is ("Before restoring “…”"; a world no
//! longer there needs none), refused when that fails or is cancelled. The zip is unpacked
//! into `<instance>/.lodestone-restore-staging-<16>/minecraft/saves/<world>/` and swapped
//! in with M5's journaled swap ([`crate::images::swap`]): the world's folder is owned, what
//! the safety backup zipped is replaced, and everything else in it is carried back
//! (`session.lock`, links, names no zip holds). A crash is undone at the next start or by
//! Play, as for snapshots. Only that world changes.
//!
//! **As a new world** ([`RestoreTo::NewWorld`]) unpacks it beside the others as
//! "`<world>` (restored `<date>`)", replacing nothing.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use lodestone_image::layout::CLIENT_GAME_FOLDER;

use super::{Making, WorldBackup, WorldBackupError, WorldBackupKind, WorldBackups};
use crate::content::InstanceDirs;
use crate::game::download::Progress;
use crate::images::swap::{self, STAGING_PREFIX, Staged, SwapError};
use crate::images::{Cancel, labels};
use crate::worlds::{self, Limits, Shape};

/// Where a restore puts the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreTo {
    /// Over the world in its folder, after a safety world backup.
    InPlace,
    /// Beside the others, as a new world.
    NewWorld,
}

/// What a restore is doing, for its progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldStep {
    /// Making the safety world backup.
    SavingCurrent,
    /// Unpacking the backup.
    Unpacking,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldRestored {
    /// The world's folder in `saves/`.
    pub folder: String,
    /// The safety world backup made first (in place, when the world was there).
    pub safety: Option<WorldBackup>,
    /// Where files written meanwhile into the world were kept
    /// (`<instance>/restore-leftovers-<16>/`), when there were any.
    pub kept_aside: Option<PathBuf>,
}

impl WorldBackups {
    /// Puts the backup `id` of `instance` back (see the module). `step` says what it is
    /// doing; `progress` counts that step's bytes. Blocking.
    pub fn restore(
        &self,
        instance: &str,
        id: &str,
        to: RestoreTo,
        step: &(dyn Fn(WorldStep) + Sync),
        progress: &Progress,
        cancel: Cancel,
    ) -> Result<WorldRestored, WorldBackupError> {
        match to {
            RestoreTo::InPlace => self.restore_in_place(instance, id, step, progress, cancel),
            RestoreTo::NewWorld => self.restore_new(instance, id, step, progress, cancel),
        }
    }

    fn restore_in_place(
        &self,
        instance: &str,
        id: &str,
        step: &(dyn Fn(WorldStep) + Sync),
        progress: &Progress,
        cancel: Cancel,
    ) -> Result<WorldRestored, WorldBackupError> {
        self.images.with_restore_locked(instance, |locked| {
            let found = self.find(instance, id)?;
            let record = found.backup.clone();
            let dirs = InstanceDirs::new(&locked.dir);
            let saves = worlds::saves_dir(&dirs);
            // A game this process didn't start has its worlds open; any open world refuses,
            // as a snapshot's restore does.
            if worlds::open_in_game(&saves, None).is_some() {
                return Err(WorldBackupError::Running);
            }
            let checked = worlds::zip::check(&found.zip, Shape::OneWorldFolder, Limits::default())?;
            let folder = record.folder.clone();
            let world = saves.join(&folder);
            let exists = fs::symlink_metadata(&world).is_ok();
            self.check_room(&locked.dir, checked.declared_bytes)?;
            if exists {
                self.check_room(
                    &self.folder_dir(instance, &folder)?,
                    worlds::folder_size(&world),
                )?;
            }

            let owned_path = format!("{CLIENT_GAME_FOLDER}/saves/{folder}");
            let in_world = |rel: &str| format!("{owned_path}/{rel}");
            // The safety backup: every file the restore replaces.
            let (safety, replaced) = if exists {
                step(WorldStep::SavingCurrent);
                let (safety, files) = self.create_locked(
                    &locked.slot,
                    instance,
                    &folder,
                    Making {
                        name: labels::before_restoring(&record.name),
                        kind: WorldBackupKind::BeforeRestore,
                        restored: Some(record.name.clone()),
                    },
                    progress,
                    cancel,
                )?;
                self.changed(instance);
                let replaced: HashSet<String> = files.iter().map(|f| in_world(f)).collect();
                (Some(safety), replaced)
            } else {
                (None, HashSet::new())
            };
            let placed: Vec<String> = checked.files().map(in_world).collect();
            let owned = [owned_path.clone()];

            // What the swap refuses, before anything is unpacked; again once it is.
            let suffix = swap::random_suffix();
            Staged::new(&locked.dir, suffix.clone(), &owned, &replaced, &placed)?;
            let staging = locked.dir.join(format!("{STAGING_PREFIX}{suffix}"));
            let staged_saves = staging.join(CLIENT_GAME_FOLDER).join("saves");
            fs::create_dir_all(&staged_saves).map_err(WorldBackupError::io(
                "Failed to create the restore's staging folder",
            ))?;
            step(WorldStep::Unpacking);
            let staged = (|| -> Result<Staged, WorldBackupError> {
                worlds::zip::extract_into(
                    &found.zip,
                    &checked,
                    &staged_saves.join(&folder),
                    progress,
                    cancel,
                )?;
                Ok(Staged::new(
                    &locked.dir,
                    suffix,
                    &owned,
                    &replaced,
                    &placed,
                )?)
            })();
            let staged = match staged {
                Ok(staged) => staged,
                Err(e) => {
                    let _ = fs::remove_dir_all(&staging);
                    return Err(e);
                }
            };
            #[cfg(test)]
            let staged = {
                let mut staged = staged;
                if let Some(faults) = self.faults.lock().unwrap_or_else(|e| e.into_inner()).take() {
                    staged.faults = faults;
                }
                staged
            };
            #[cfg(test)]
            if let Some(meanwhile) = self
                .before_swap
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                meanwhile();
            }
            let kept_aside = match staged.swap(&locked.journal) {
                Ok(kept) => kept,
                Err(SwapError::RolledBack(e)) | Err(SwapError::Broken(e)) => return Err(e.into()),
            };
            Ok(WorldRestored {
                folder,
                safety,
                kept_aside,
            })
        })
    }

    fn restore_new(
        &self,
        instance: &str,
        id: &str,
        step: &(dyn Fn(WorldStep) + Sync),
        progress: &Progress,
        cancel: Cancel,
    ) -> Result<WorldRestored, WorldBackupError> {
        self.images.with_instance_locked(instance, |locked| {
            let found = self.find(instance, id)?;
            let record = &found.backup;
            let checked = worlds::zip::check(&found.zip, Shape::OneWorldFolder, Limits::default())?;
            let saves = worlds::saves_dir(&InstanceDirs::new(&locked.dir));
            fs::create_dir_all(&saves)
                .map_err(WorldBackupError::io("Failed to create the saves folder"))?;
            self.check_room(&saves, checked.declared_bytes)?;
            // Staged where startup's clean-up finds what a crash left.
            let staging = locked
                .dir
                .join(format!("{STAGING_PREFIX}{}", swap::random_suffix()));
            fs::create_dir(&staging).map_err(WorldBackupError::io(
                "Failed to create the restore's staging folder",
            ))?;
            step(WorldStep::Unpacking);
            let placed = (|| -> Result<String, WorldBackupError> {
                let unpacked = staging.join("world");
                worlds::zip::extract_into(&found.zip, &checked, &unpacked, progress, cancel)?;
                let today = worlds::date(std::time::SystemTime::now());
                let folder = worlds::unused(
                    &saves,
                    &worlds::folder_name(&format!("{} (restored {today})", record.folder)),
                );
                crate::fsx::rename(&unpacked, saves.join(&folder))
                    .map_err(WorldBackupError::io("Failed to place the restored world"))?;
                let level = checked
                    .summary
                    .name
                    .clone()
                    .unwrap_or_else(|| record.world_name().to_string());
                worlds::set_level_name(
                    &saves.join(&folder),
                    &format!("{level} (restored {today})"),
                );
                Ok(folder)
            })();
            let _ = fs::remove_dir_all(&staging);
            Ok(WorldRestored {
                folder: placed?,
                safety: None,
                kept_aside: None,
            })
        })
    }
}
