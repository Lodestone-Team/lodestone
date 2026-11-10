//! The sidebar's automatic groups and "Last played": instances of each loader, one
//! imported from another launcher, one in a group of its own, and a folded group.
//! The `sidebar-play-*` scenes show Play over a row's icon, as when the pointer is
//! on the row.

use gpui::Context;
use launcher_core::{
    game::{
        install::{InstallProgress, Stage},
        loaders::{LoaderChoice, LoaderKind},
    },
    instances::{InstanceStore, now},
    paths::DataDir,
    settings,
};

use crate::shell::LauncherShell;

pub const SCENES: &[&str] = &[
    "sidebar-groups",
    "sidebar-groups-off",
    "sidebar-groups-settings",
    // the pointer on a row with a white icon, on the dark icon under "Last played",
    // and on an instance that is getting ready
    "sidebar-play-hover",
    "sidebar-play-hover-dark",
    "sidebar-play-preparing",
];

pub fn is_scene(scene: &str) -> bool {
    SCENES.contains(&scene)
}

/// The instance the scenes open: the one played last.
pub const SELECTED: &str = "Creative build";

pub fn fill(data_dir: &DataDir, store: &InstanceStore, scene: &str) {
    let loader = |kind, version: &str| {
        Some(LoaderChoice {
            kind,
            version: version.to_string(),
        })
    };
    let now = now();
    let hour = 3600;
    // (name, version, loader, own group, imported from, played this long ago)
    for (name, version, choice, group, imported, ago) in [
        (SELECTED, "1.21.4", None, None, None, Some(2 * hour)),
        (
            "Fabulously Optimized",
            "1.21.4",
            loader(LoaderKind::Fabric, "0.16.14"),
            None,
            None,
            Some(26 * hour),
        ),
        (
            "All the mods",
            "1.20.1",
            loader(LoaderKind::Forge, "1.20.1-47.4.28"),
            None,
            None,
            None,
        ),
        (
            "Create pack",
            "1.21.1",
            loader(LoaderKind::NeoForge, "21.1.172"),
            None,
            None,
            Some(4 * 24 * hour),
        ),
        (
            "Quilt test",
            "1.20.1",
            loader(LoaderKind::Quilt, "0.29.1"),
            None,
            None,
            None,
        ),
        (
            "Skyblock",
            "1.20.1",
            loader(LoaderKind::Fabric, "0.16.14"),
            None,
            Some("Prism Launcher"),
            Some(30 * 24 * hour),
        ),
        ("Hardcore", "1.21.4", None, Some("Survival"), None, None),
    ] {
        let Ok(mut instance) = store.create(name, version, choice.as_ref()) else {
            continue;
        };
        instance.group = group.map(str::to_string);
        instance.imported_from = imported.map(str::to_string);
        let _ = store.save(&instance);
        if let Some(ago) = ago {
            let _ = store.record_play(&instance.id, now - ago, 3 * hour + 10 * 60);
        }
        if scene.starts_with("sidebar-play-") {
            match name {
                // a white logo, like a modpack's, for the scrim to darken
                "Fabulously Optimized" => {
                    let file = data_dir.root().join("white-icon.png");
                    if white_icon().save(&file).is_ok() {
                        let _ = store.set_icon(&instance.id, &file);
                    }
                }
                SELECTED => {
                    let _ = store.set_icon_name(&instance.id, Some("builtin:obsidian"));
                }
                _ => {}
            }
        }
    }
    let _ = settings::update(&data_dir.settings_file(), |settings| {
        settings.collapsed_groups.insert("Quilt (modded)".into());
        settings.auto_groups = scene != "sidebar-groups-off";
    });
}

/// A white logo with a pale grey mark: the hardest icon for a white Play to read on.
fn white_icon() -> image::RgbaImage {
    image::RgbaImage::from_fn(128, 128, |x, y| {
        let (dx, dy) = (x as f32 - 64.0, y as f32 - 64.0);
        let ring = (dx * dx + dy * dy).sqrt();
        let grey = if (34.0..44.0).contains(&ring) {
            210
        } else {
            252
        };
        image::Rgba([grey, grey, grey, 255])
    })
}

/// The row the pointer is on, in the `sidebar-play-*` scenes: by its element id.
pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let (name, place) = match scene {
        "sidebar-play-hover" => ("Fabulously Optimized", "instance"),
        "sidebar-play-hover-dark" => (SELECTED, "last-played"),
        "sidebar-play-preparing" => ("Create pack", "instance"),
        _ => return,
    };
    let instances = shell.instances().clone();
    let Some(id) = instances
        .read(cx)
        .list()
        .iter()
        .find(|instance| instance.name == name)
        .map(|instance| instance.id.clone())
    else {
        return;
    };
    if scene == "sidebar-play-preparing" {
        let progress = std::sync::Arc::new(InstallProgress::default());
        progress.set_stage(Stage::Libraries);
        instances.update(cx, |instances, _| {
            instances.preview_preparing(&id, progress)
        });
    }
    shell.preview_sidebar_hover(format!("{place}-{id}").into(), cx);
}
