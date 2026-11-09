//! The snapshot screenshots: an instance with mods and worlds, two snapshots of it and
//! one taken before going back, and a `.lodesnap` file to import.

use std::path::PathBuf;

use gpui::Context;
use launcher_core::{
    instances::{Instance, InstanceStore},
    paths::DataDir,
    snapshots::{Selection, Store},
};

use crate::{
    i18n::{InstanceSettingsMsg as M, t},
    instances::snapshots::SnapshotRequest,
    shell::{InstanceTab, LauncherShell},
};

pub fn is_scene(scene: &str) -> bool {
    scene == "snapshots" || scene.starts_with("snapshot-")
}

fn none(_: u64, _: u64) {}

/// The file the import scene opens.
fn import_file(data_dir: &DataDir) -> PathBuf {
    data_dir
        .root()
        .join("Survival 1.20.1 - Before 1.21.lodesnap")
}

/// Fills `instance` and takes its snapshots.
pub fn fill(data_dir: &DataDir, store: &InstanceStore, instance: &Instance) {
    let dir = store.dir(&instance.id);
    super::content::fill(&dir, "content-mods");
    super::worlds::fill(&dir);
    let snapshots = Store::new(data_dir);
    let Ok(first) = snapshots.create(
        instance,
        &dir,
        "Before 1.21",
        None,
        &Selection::all(),
        false,
        &none,
    ) else {
        return;
    };
    let _ = snapshots.export(&first, &import_file(data_dir), &none);
    // A new mod, then a second snapshot: only the mod is new.
    let _ = std::fs::write(
        dir.join("minecraft/mods/new-mod-1.0.jar"),
        vec![7u8; 180_000],
    );
    let _ = snapshots.create(
        instance,
        &dir,
        "Added a minimap",
        None,
        &Selection::all(),
        false,
        &none,
    );
    // Going back to the first one leaves a snapshot of what was there.
    let mods_only = Selection {
        categories: [launcher_core::snapshots::Category::Mods].into(),
        worlds: Default::default(),
    };
    let _ = snapshots.revert(&first, instance, &dir, &mods_only, &none);
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let instances = shell.instances().clone();
    let (id, data_dir) = {
        let instances = instances.read(cx);
        let id = instances
            .list()
            .iter()
            .find(|i| i.name == "Survival 1.20.1")
            .map(|i| i.id.clone())
            .unwrap_or_default();
        (id, instances.data_dir().clone())
    };
    instances.update(cx, |instances, cx| instances.select(&id, cx));
    shell.select_tab(InstanceTab::Settings, cx);
    let snapshots = Store::new(&data_dir).list(&id);
    // Newest first: the automatic one, "Added a minimap", "Before 1.21".
    let named = |name: &str| {
        snapshots
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
        "snapshot-revert" => SnapshotRequest::Revert(id, named("Before 1.21")),
        "snapshot-share" => SnapshotRequest::Share(id, named("Added a minimap")),
        "snapshot-delete" => SnapshotRequest::Delete(id, named("Added a minimap")),
        _ => SnapshotRequest::Import(import_file(&data_dir)),
    };
    shell.open_snapshot(request, cx);
}
