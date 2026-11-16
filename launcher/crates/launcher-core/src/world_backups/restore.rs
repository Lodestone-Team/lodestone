//! Restoring a world backup: in place, or beside the world as a new one.
//!
//! **In place** ([`RestoreTo::InPlace`]) runs as a snapshot's restore does
//! ([`crate::images::restore`]): under the instance's record lock, never while its game runs
//! or any of its worlds is open, an unfinished restore undone first. The zip is checked
//! whole before anything is written ([`worlds::zip::check`]), then what the swap would
//! refuse (from the list of what the safety backup will zip), then the disks' room (the
//! unpacked world and the safety backup together when they share a disk), then the
//! **safety world backup** of the world as it is ("Before restoring “…”"; a world no
//! longer there needs none), refused when that fails or is cancelled. A restore that then
//! ends with the world unchanged (refused, cancelled while unpacking, or rolled back)
//! deletes that safety backup again: each is a full-size zip. The zip is unpacked
//! into `<instance>/.lodestone-restore-staging-<16>/minecraft/saves/<world>/` and swapped
//! in with M5's journaled swap ([`crate::images::swap`]): the world's folder is owned, what
//! the safety backup zipped is replaced, and everything else in it is carried back
//! (`session.lock`, links, names no zip holds); a folder the zip lists (an empty
//! `datapacks/` too) is the zip's. Just before the swap, open worlds and a game session
//! are checked again. A crash is undone at the next start or by Play, as for snapshots.
//! Only that world changes.
//!
//! In place is refused when `saves` or a folder above it is a link: the swap could rename
//! through it, but its rollback refuses to, so a failure could never be undone.
//!
//! **As a new world** ([`RestoreTo::NewWorld`]) unpacks it into
//! `saves/.lodestone-restore-staging-<instance>-<16>/` (one disk with `saves`, even a
//! linked one), then renames it beside the others as "`<world>` (restored `<date>`)", in a
//! folder free in `saves/` and the backups' folder, replacing nothing.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use lodestone_image::layout::CLIENT_GAME_FOLDER;

use super::{
    MAX_NAME_CHARS, Making, WorldBackup, WorldBackupError, WorldBackupKind, WorldBackups, same_disk,
};
use crate::content::InstanceDirs;
use crate::game::download::Progress;
use crate::images::swap::{self, STAGING_PREFIX, Staged, SwapError};
use crate::images::{Cancel, labels};
use crate::worlds::{self, Shape};

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
            let folder = record.folder.clone();
            // Exactly what making a backup takes: never `saves/aux` from a planted folder.
            worlds::zip::check_world_folder(&folder)?;
            let dirs = InstanceDirs::new(&locked.dir);
            let saves = worlds::saves_dir(&dirs);
            // A game this process didn't start has its worlds open; any open world refuses,
            // as a snapshot's restore does.
            if worlds::open_in_game(&saves, None).is_some() {
                return Err(WorldBackupError::Running);
            }
            let checked = worlds::zip::check(&found.zip, Shape::OneWorldFolder, self.limits())?;
            let world = saves.join(&folder);

            let owned_path = format!("{CLIENT_GAME_FOLDER}/saves/{folder}");
            // A linked `saves` (shared saves, or on another drive): the swap could rename
            // through it but never undo that, so a failure or a crash would leave the
            // instance unplayable. As a new world needs no undo.
            if !swap::no_links_above(&locked.dir, &owned_path) {
                return Err(WorldBackupError::Invalid(
                    "The saves folder, or a folder above it, is a link, so the world can't be restored in place. Restore it as a new world, or put the real folder back in place of the link."
                        .into(),
                ));
            }
            let in_world = |rel: &str| format!("{owned_path}/{rel}");
            let placed: Vec<String> = checked.files().map(in_world).collect();
            let folders: Vec<String> = checked.folders().map(in_world).collect();
            let owned = [owned_path.clone()];
            // What the safety backup will zip (a link or a file in the world's place has
            // nothing to zip, and the swap refuses it): every file the restore replaces.
            let plan = if fs::symlink_metadata(&world).is_ok_and(|m| m.is_dir()) {
                Some(worlds::zip::plan(&world, &folder)?)
            } else {
                None
            };
            // A world too large for a world backup can't have its safety backup made (its
            // zip would be one no restore takes), so it isn't restored over: nothing has
            // changed yet, and restoring as a new world needs no safety backup.
            if let Some(over) = plan.as_ref().and_then(|p| p.within(self.limits()).err()) {
                return Err(WorldBackupError::Invalid(format!(
                    "The world as it is now is too large for the world backup Lodestone makes before restoring over it: it has {over}. Restore as a new world instead."
                )));
            }
            let planned: HashSet<String> =
                plan.iter().flat_map(|p| p.files()).map(in_world).collect();
            // What the swap refuses, before the safety backup is made or anything unpacked;
            // again once it is.
            let suffix = swap::random_suffix();
            Staged::with_folders(
                &locked.dir,
                suffix.clone(),
                &owned,
                &planned,
                &placed,
                &folders,
            )?;
            // Room for the unpacked world and the safety backup: together when both are on
            // one disk, as they are unless the data folder is split across drives.
            let safety_bytes = plan.as_ref().map_or(0, |p| p.bytes());
            let backups_dir = self.folder_dir(instance, &folder)?;
            if plan.is_some() && same_disk(&locked.dir, &backups_dir) {
                self.check_room(
                    &locked.dir,
                    checked.declared_bytes.saturating_add(safety_bytes),
                )?;
            } else {
                self.check_room(&locked.dir, checked.declared_bytes)?;
                if plan.is_some() {
                    self.check_room(&backups_dir, safety_bytes)?;
                }
            }

            // The safety backup, named after the backup restored (cut so the whole name
            // fits a backup's name).
            let restored_name: String = record
                .name
                .chars()
                .take(MAX_NAME_CHARS - labels::before_restoring("").chars().count())
                .collect();
            let safety = match plan {
                Some(plan) => {
                    step(WorldStep::SavingCurrent);
                    let (safety, files) = self.create_locked(
                        &locked.slot,
                        instance,
                        &folder,
                        Making {
                            name: labels::before_restoring(&restored_name),
                            kind: WorldBackupKind::BeforeRestore,
                            restored: Some(restored_name),
                        },
                        Some(plan),
                        progress,
                        cancel,
                    )?;
                    self.changed(instance);
                    Some((safety, files))
                }
                None => None,
            };
            let replaced: HashSet<String> = safety
                .iter()
                .flat_map(|(_, files)| files.iter())
                .map(|f| in_world(f))
                .collect();

            // Whether the world changed is what decides whether the safety backup stays: a
            // restore that ends with the world as it was deletes the copy it just made (a
            // full-size zip, never pruned) so a retry doesn't add another.
            let swapped = (|| -> Result<Option<PathBuf>, (WorldBackupError, bool)> {
                let unchanged = |e: WorldBackupError| (e, false);
                let staging = locked.dir.join(format!("{STAGING_PREFIX}{suffix}"));
                let staged_saves = staging.join(CLIENT_GAME_FOLDER).join("saves");
                fs::create_dir_all(&staged_saves)
                    .map_err(WorldBackupError::io(
                        "Failed to create the restore's staging folder",
                    ))
                    .map_err(unchanged)?;
                step(WorldStep::Unpacking);
                let staged = (|| -> Result<Staged, WorldBackupError> {
                    worlds::zip::extract_into(
                        &found.zip,
                        &checked,
                        &staged_saves.join(&folder),
                        progress,
                        cancel,
                    )?;
                    Ok(Staged::with_folders(
                        &locked.dir,
                        suffix.clone(),
                        &owned,
                        &replaced,
                        &placed,
                        &folders,
                    )?)
                })();
                let staged = match staged {
                    Ok(staged) => staged,
                    Err(e) => {
                        let _ = fs::remove_dir_all(&staging);
                        return Err(unchanged(e));
                    }
                };
                #[cfg(test)]
                let staged = {
                    let mut staged = staged;
                    if let Some(faults) =
                        self.faults.lock().unwrap_or_else(|e| e.into_inner()).take()
                    {
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
                // Unpacking a large world takes minutes: a game this Launcher didn't start
                // may have opened a world meanwhile.
                if worlds::open_in_game(&saves, None).is_some() || locked.slot.in_session() {
                    staged.discard();
                    return Err(unchanged(WorldBackupError::Running));
                }
                match staged.swap(&locked.journal) {
                    Ok(kept) => Ok(kept),
                    Err(SwapError::RolledBack(e)) => Err((e.into(), false)),
                    Err(SwapError::Broken(e)) => Err((e.into(), true)),
                }
            })();
            match swapped {
                Ok(kept_aside) => Ok(WorldRestored {
                    folder,
                    safety: safety.map(|(safety, _)| safety),
                    kept_aside,
                }),
                Err((e, changed)) => {
                    if let (false, Some((safety, _))) = (changed, &safety) {
                        self.remove_made(instance, safety);
                    }
                    Err(e)
                }
            }
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
            worlds::zip::check_world_folder(&record.folder)?;
            let checked = worlds::zip::check(&found.zip, Shape::OneWorldFolder, self.limits())?;
            let saves = worlds::saves_dir(&InstanceDirs::new(&locked.dir));
            fs::create_dir_all(&saves)
                .map_err(WorldBackupError::io("Failed to create the saves folder"))?;
            self.check_room(&saves, checked.declared_bytes)?;
            // Staged in `saves/` itself, so placing it is a rename on one disk even when
            // `saves` is a link to another drive. The name is hidden from world lists and
            // snapshots, and what a crash leaves the startup sweep deletes
            // ([`WorldBackups::sweep`]).
            let staging = saves.join(super::staging_name(instance));
            fs::create_dir(&staging).map_err(WorldBackupError::io(
                "Failed to create the restore's staging folder",
            ))?;
            step(WorldStep::Unpacking);
            let placed = (|| -> Result<String, WorldBackupError> {
                let unpacked = staging.join("world");
                worlds::zip::extract_into(&found.zip, &checked, &unpacked, progress, cancel)?;
                let today = worlds::date(std::time::SystemTime::now());
                // Free among the world backups' folders too: a deleted world's backups
                // stay under its folder, and a new world given that folder would take
                // them over.
                let folder = worlds::unused_in(
                    &[&saves, &self.root.join(instance)],
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
