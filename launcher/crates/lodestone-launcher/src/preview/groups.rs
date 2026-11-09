//! The sidebar's automatic groups and "Last played": instances of each loader, one
//! imported from another launcher, one in a group of its own, and a folded group.

use launcher_core::{
    game::loaders::{LoaderChoice, LoaderKind},
    instances::{InstanceStore, now},
    paths::DataDir,
    settings,
};

pub const SCENES: &[&str] = &[
    "sidebar-groups",
    "sidebar-groups-off",
    "sidebar-groups-settings",
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
    }
    let _ = settings::update(&data_dir.settings_file(), |settings| {
        settings.collapsed_groups.insert("Quilt (modded)".into());
        settings.auto_groups = scene != "sidebar-groups-off";
    });
}
