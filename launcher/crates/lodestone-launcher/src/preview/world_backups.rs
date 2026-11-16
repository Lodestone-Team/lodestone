//! The world backup screenshots: the made-up worlds of the Worlds tab's scenes, with
//! world backups made through `WorldBackups` in the scene's own data folder (one restored,
//! which leaves a safety backup; one of a world deleted since), and a snapshot.

use gpui::Context;
use launcher_core::{
    game::download::Progress,
    images::{Images, NEVER, SnapshotKind, Take},
    instances::{Instance, InstanceStore},
    paths::DataDir,
    world_backups::{RestoreTo, WorldBackups},
};

use crate::{
    i18n::{InstanceSettingsMsg as M, t, tr},
    instances::world_backups::WorldBackupRequest,
    shell::{InstanceTab, LauncherShell},
};

pub fn is_scene(scene: &str) -> bool {
    scene == "world-backups" || scene.starts_with("world-backup-")
}

/// Fills `instance` with worlds and backs them up.
pub fn fill(data_dir: &DataDir, store: &InstanceStore, instance: &Instance) {
    let dir = store.dir(&instance.id);
    super::worlds::fill(&dir);
    let images = Images::open(data_dir);
    let backups = WorldBackups::open(data_dir, images.clone());
    let id = &instance.id;
    let _ = images.record_now(id, "Created Survival 1.20.1");
    let back_up = |folder: &str, name: Option<&str>| {
        backups
            .create(id, folder, name, &Progress::default(), NEVER)
            .ok()
    };
    let Some(first) = back_up("New World", None) else {
        return;
    };
    back_up("New World", Some("Before the dragon fight"));
    back_up("Castle", Some("Finished the towers"));
    // Going back to the first one leaves a safety backup of the world as it was.
    let _ = backups.restore(
        id,
        &first.id,
        RestoreTo::InPlace,
        &|_| {},
        &Progress::default(),
        NEVER,
    );
    // A world deleted since: its backup stays.
    back_up("Hardcore run", None);
    let _ = std::fs::remove_dir_all(dir.join("minecraft/saves/Hardcore run"));
}

/// The worlds scene's world backups and a snapshot: two backups of its first world, and a
/// snapshot holding every world.
pub fn fill_worlds(data_dir: &DataDir, instance: &Instance) {
    let images = Images::open(data_dir);
    let backups = WorldBackups::open(data_dir, images.clone());
    let id = &instance.id;
    let _ = images.record_now(id, "Created Survival 1.20.1");
    for name in [None, Some("Before the dragon fight")] {
        let _ = backups.create(id, "New World", name, &Progress::default(), NEVER);
    }
    let _ = images.take_snapshot(id, Take::new(SnapshotKind::Manual, "Before 1.21"));
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let instances = shell.instances().clone();
    let (id, backups) = {
        let instances = instances.read(cx);
        let id = instances
            .list()
            .iter()
            .find(|i| i.name == "Survival 1.20.1")
            .map(|i| i.id.clone())
            .unwrap_or_default();
        (id, instances.world_backups())
    };
    instances.update(cx, |instances, cx| instances.select(&id, cx));
    shell.select_tab(InstanceTab::Settings, cx);
    let list = backups.list(&id).unwrap_or_default();
    let named = |name: &str| list.iter().find(|b| b.name == name).cloned();
    let request = match scene {
        "world-backups" => {
            // The Settings tab is built again for the instance chosen above: wait for it.
            cx.spawn(async move |shell, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(300))
                    .await;
                shell
                    .update(cx, |shell, cx| {
                        if let Some(settings) = shell.settings_view().cloned() {
                            settings.update(cx, |settings, cx| {
                                settings.preview_section(t(M::InstanceSnapshots));
                                cx.notify();
                            });
                        }
                    })
                    .ok();
            })
            .detach();
            return;
        }
        "world-backup-create" => WorldBackupRequest::BackUp {
            instance: id,
            world: None,
        },
        "world-backup-restore" => {
            let Some(backup) = named("Before the dragon fight") else {
                return;
            };
            WorldBackupRequest::Restore {
                instance: id,
                backup,
                to: RestoreTo::InPlace,
            }
        }
        "world-backup-restore-new" => {
            let Some(backup) = named("Finished the towers") else {
                return;
            };
            // As it ends: the restore runs in the scene's data folder, and the dialog
            // shows how it went.
            shell.open_world_backup(
                WorldBackupRequest::Restore {
                    instance: id,
                    backup,
                    to: RestoreTo::NewWorld,
                },
                cx,
            );
            if let Some(dialog) = shell.world_backup_dialog() {
                dialog.update(cx, |dialog, cx| {
                    dialog.preview_done(tr!(
                        M::WorldRestoredAsNew,
                        folder = "Castle (restored 2026-11-15)"
                    ));
                    cx.notify();
                });
            }
            return;
        }
        // An in-place restore as it ends: where the world as it was went.
        "world-backup-restored" => {
            let Some(backup) = named("Before the dragon fight") else {
                return;
            };
            let world = backup.world_name().to_string();
            shell.open_world_backup(
                WorldBackupRequest::Restore {
                    instance: id,
                    backup,
                    to: RestoreTo::InPlace,
                },
                cx,
            );
            if let Some(dialog) = shell.world_backup_dialog() {
                dialog.update(cx, |dialog, cx| {
                    let safety = tr!(
                        M::WorldBackupBeforeRestoring,
                        name = "Before the dragon fight"
                    );
                    dialog.preview_done(format!(
                        "{} {}",
                        tr!(M::WorldRestored, world = world),
                        tr!(M::WorldRestoredSafety, name = safety)
                    ));
                    cx.notify();
                });
            }
            return;
        }
        _ => return,
    };
    shell.open_world_backup(request, cx);
}
