//! The shared game settings scenes: real `options.txt` files (from the tests) in the
//! made-up instances, a global set copied from one of them, and an instance with
//! overrides and settings changed in-game.

use launcher_core::{instances::InstanceStore, paths::DataDir, shared_options};

const OPTIONS_1_20_1: &str = include_str!("../../../launcher-core/testdata/options/1.20.1.txt");
const OPTIONS_1_21_11: &str = include_str!("../../../launcher-core/testdata/options/1.21.11.txt");
const OPTIONS_1_8_9: &str = include_str!("../../../launcher-core/testdata/options/1.8.9.txt");

pub fn is_scene(scene: &str) -> bool {
    matches!(
        scene,
        "shared-settings" | "instance-shared-settings" | "focus-popover" | "focus-escape"
    )
}

/// Writes the files after the made-up instances exist.
pub fn fill(data: &DataDir, store: &InstanceStore) {
    let (list, _) = store.list();
    let mut source = None;
    let mut survival = None;
    for instance in &list {
        let options = match instance.name.as_str() {
            "Survival 1.20.1" => {
                survival = Some(instance.id.clone());
                // As the game leaves it after these were changed in-game.
                OPTIONS_1_20_1
                    .replace("mouseSensitivity:0.5\n", "mouseSensitivity:0.7\n")
                    .replace("renderDistance:12\n", "renderDistance:20\n")
                    .replace(
                        "key_key.drop:key.keyboard.q\n",
                        "key_key.drop:key.keyboard.z\n",
                    )
            }
            "Latest release" => {
                source = Some(instance.id.clone());
                OPTIONS_1_21_11.to_string()
            }
            "Old school" => OPTIONS_1_8_9.to_string(),
            _ => continue,
        };
        let _ = std::fs::write(store.game_dir(&instance.id).join("options.txt"), options);
    }
    if let Some(id) = &source {
        let dir = store.dir(id);
        let _ = shared_options::copy_from_instance(data, &dir, "26.3", None);
        let _ = shared_options::set_preferred(data, Some(id), Some(&dir));
    }
    if let Some(id) = survival {
        let _ = shared_options::update_instance(&store.dir(&id), |sharing| {
            sharing
                .overrides
                .extend(["fov".to_string(), "guiScale".to_string()]);
            sharing.changed.extend([
                ("mouseSensitivity".to_string(), "0.7".to_string()),
                ("key_key.drop".to_string(), "key.keyboard.z".to_string()),
                ("renderDistance".to_string(), "20".to_string()),
            ]);
        });
    }
}

/// The window height the scene needs to show the whole section.
/// The window width of scenes narrower than the usual 1200 px.
pub fn window_width(scene: &str) -> Option<f32> {
    scene.contains("-narrow").then_some(720.)
}

pub fn window_height(scene: &str) -> Option<f32> {
    match scene {
        "shared-settings" => Some(1700.),
        "instance-shared-settings" => Some(2100.),
        "sidebar-groups" | "sidebar-groups-off" => Some(1000.),
        // Tall enough for the Modrinth section under six mods.
        "content-mods" => Some(1100.),
        // A world's data packs, then Modrinth's results.
        "worlds-datapacks" => Some(1050.),
        // The installed mods, then Modrinth's results to browse (and its filters).
        "content-browse-idle" => Some(1500.),
        "content-description" | "content-gallery" | "content-cf-project" => Some(1400.),
        "content-filters" | "content-filters-packs" | "content-filters-shaders" => Some(1700.),
        // Down to "Other cores", under the friends sections.
        "servers-local" => Some(2500.),
        "servers"
        | "servers-core-menu"
        | "servers-core-menu-other"
        | "servers-local-none"
        | "servers-local-unclaimed"
        | "servers-shared-local"
        | "servers-shared-local-gone" => Some(2000.),
        _ => None,
    }
}
