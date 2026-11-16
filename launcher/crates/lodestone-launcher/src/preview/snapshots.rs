//! The snapshot screenshots: an instance with mods and worlds, snapshots of it (one the
//! user took, one taken before an update, one before a restore), and an image file to
//! import. They are taken through the image store in the scene's own data folder.

use std::path::PathBuf;

use gpui::Context;
use launcher_core::{
    images::{Images, RestoreScope, RiskyChange, SnapshotKind, Take},
    instances::{Instance, InstanceStore},
    paths::DataDir,
};

use crate::{
    i18n::{InstanceSettingsMsg as M, t},
    instances::snapshots::{Choice, SnapshotRequest},
    shell::{InstanceTab, LauncherShell},
};

pub fn is_scene(scene: &str) -> bool {
    scene == "snapshots" || scene.starts_with("snapshot-")
}

/// A Fabric server's backup a real core exported (launcher-core's test fixture).
const SERVER_BACKUP: &[u8] =
    include_bytes!("../../../launcher-core/tests/fixtures/core_images/fabric-backup.lodeimg");

/// The file the import scene opens.
fn import_file(data_dir: &DataDir) -> PathBuf {
    data_dir
        .root()
        .join("Survival 1.20.1 - Before 1.21.lodeimg")
}

/// Fills `instance` and takes its snapshots.
pub fn fill(data_dir: &DataDir, store: &InstanceStore, instance: &Instance) {
    let dir = store.dir(&instance.id);
    super::content::fill(&dir, "content-mods");
    super::worlds::fill(&dir);
    let images = Images::open(data_dir);
    let id = &instance.id;
    let _ = images.record_now(id, "Created Survival 1.20.1");
    let Ok(Some(first)) = images.take_snapshot(id, Take::new(SnapshotKind::Manual, "Before 1.21"))
    else {
        return;
    };
    let _ = images.export_snapshot(
        id,
        &first.id,
        &import_file(data_dir),
        launcher_core::images::NEVER,
    );
    // A new mod, and an update: a snapshot before it.
    let _ = std::fs::write(
        dir.join("minecraft/mods/new-mod-1.0.jar"),
        vec![7u8; 180_000],
    );
    let _ = std::fs::write(
        dir.join("minecraft/saves/New World/region/r.1.0.mca"),
        vec![3u8; 90_000],
    );
    let _ = images.before_risky_change(
        id,
        RiskyChange::ContentUpdate {
            titles: &["Sodium".to_string()],
        },
    );
    // Going back to the first one's world leaves a safety snapshot of what was there.
    let _ = images.restore(
        id,
        &first.id,
        &RestoreScope::Worlds(vec!["New World".into()]),
        launcher_core::images::NEVER,
    );
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let instances = shell.instances().clone();
    let (id, data_dir, images) = {
        let instances = instances.read(cx);
        let id = instances
            .list()
            .iter()
            .find(|i| i.name == "Survival 1.20.1")
            .map(|i| i.id.clone())
            .unwrap_or_default();
        (id, instances.data_dir().clone(), instances.images())
    };
    instances.update(cx, |instances, cx| instances.select(&id, cx));
    shell.select_tab(InstanceTab::Settings, cx);
    let records = images
        .snapshots(&id)
        .map(|found| found.records)
        .unwrap_or_default();
    let named = |name: &str| {
        records
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.id.clone())
            .unwrap_or_default()
    };
    let request = match scene {
        "snapshots" => {
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
        "snapshot-take" => SnapshotRequest::Take(id),
        "snapshot-restore" => SnapshotRequest::Restore(id, named("Before 1.21")),
        "snapshot-restore-mods" => {
            // "The worlds and their mods": what putting the mods back changes.
            shell.open_snapshot(SnapshotRequest::Restore(id, named("Before 1.21")), cx);
            if let Some(dialog) = shell.snapshot_dialog() {
                dialog.update(cx, |dialog, _| dialog.preset_choice(Choice::WorldsAndMods));
            }
            return;
        }
        "snapshot-restore-new" => {
            // "As a new instance": the name of the copy.
            shell.open_snapshot(SnapshotRequest::Restore(id, named("Before 1.21")), cx);
            if let Some(dialog) = shell.snapshot_dialog() {
                dialog.update(cx, |dialog, _| dialog.preset_choice(Choice::AsNew));
            }
            return;
        }
        "snapshot-export" => SnapshotRequest::Export(id, named("Before 1.21")),
        "snapshot-delete" => SnapshotRequest::Delete(id, named("Before updating Sodium")),
        "snapshot-import-server" => {
            // a server's backup a core exported, as "Download this world" gets it
            let file = data_dir
                .root()
                .join("Survival - 20261112-141000-a1b2.lodeimg");
            let _ = std::fs::write(&file, SERVER_BACKUP);
            SnapshotRequest::Import(file)
        }
        "snapshot-import-server-downloaded" => {
            // the same backup through "Download this world", with what the host took out
            let file = data_dir.root().join("downloaded-world.lodeimg");
            let _ = std::fs::write(&file, SERVER_BACKUP);
            let exported = serde_json::from_value(serde_json::json!({
                "left_out": {"names": ["config/simplevoice.toml", "eula.txt", "plugins/LuckPerms/config.yml"], "count": 3},
                "left_out_for_players": {"names": ["config/simplevoice.toml"], "count": 1},
                "blanked": {"names": ["server.properties: rcon.password"], "count": 1}
            }))
            .unwrap_or_default();
            SnapshotRequest::ImportDownloaded {
                file: std::sync::Arc::new(
                    launcher_core::cores::world_download::Downloaded::preview(file, exported),
                ),
                server: "Survival SMP".into(),
            }
        }
        "snapshot-import-pack-skipped" => {
            // the import made, without the files only the server's modpack's players use
            instances.update(cx, |_, cx| {
                cx.emit(crate::instances::InstancesEvent::Notice {
                    title: t(M::ImportPackSkippedTitle).into(),
                    message: crate::instances::snapshots::pack_skipped(
                        launcher_core::images::from_server::SkipReason::Gone,
                    )
                    .into(),
                    about: crate::instances::About::Instance(id.clone()),
                })
            });
            return;
        }
        "snapshot-import-server-done" => {
            // what the import says once made, when some of the server's mods couldn't
            // be downloaded
            instances.update(cx, |_, cx| {
                cx.emit(crate::instances::InstancesEvent::Notice {
                    title: t(M::ImportLeftOutTitle).into(),
                    message: crate::i18n::tr!(
                        M::ImportLeftOut,
                        files = "mods/lithium.jar, mods/sodium.jar"
                    )
                    .into(),
                    about: crate::instances::About::Instance(id.clone()),
                })
            });
            return;
        }
        _ => SnapshotRequest::Import(import_file(&data_dir)),
    };
    shell.open_snapshot(request, cx);
}
