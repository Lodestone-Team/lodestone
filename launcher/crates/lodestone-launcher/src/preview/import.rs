//! The import dialog with made-up launchers and instances, for the offscreen screenshots.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use gpui::{AppContext as _, Context, RenderImage};
use launcher_core::{
    game::loaders::LoaderKind,
    import::{Found, Launcher, Source, WantedLoader, WantedVersion},
    instances::Memory,
};

use crate::{
    instances::import::{ImportDialog, QueueState, Size},
    shell::LauncherShell,
};

fn found(
    source: Source,
    name: &str,
    version: WantedVersion,
    loader: Option<(LoaderKind, &str)>,
    notes: &[&str],
) -> Found {
    let game_dir = PathBuf::from(format!("/Users/you/Instances/{name}"));
    Found {
        source,
        key: format!("{source:?}:{name}"),
        name: name.into(),
        game_dir,
        game_version: version,
        loader: loader.map(|(kind, v)| WantedLoader::new(kind, Some(v))),
        memory: Some(Memory {
            min: 512,
            max: 4096,
        }),
        jvm_args: None,
        icon: None,
        notes: notes.iter().map(|n| n.to_string()).collect(),
    }
}

fn launcher(source: Source, root: &str, instances: Vec<Found>) -> Launcher {
    Launcher {
        source,
        root: PathBuf::from(root),
        instances,
        problems: Vec::new(),
    }
}

/// A square icon in `rgb`.
fn icon(rgb: [u8; 3]) -> Arc<RenderImage> {
    let image = image::RgbaImage::from_fn(64, 64, |x, y| {
        let shade = if (x / 16 + y / 16) % 2 == 0 { 0 } else { 30 };
        image::Rgba([
            rgb[0].saturating_sub(shade),
            rgb[1].saturating_sub(shade),
            rgb[2].saturating_sub(shade),
            255,
        ])
    });
    crate::images::render_image(image)
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let exact = |v: &str| WantedVersion::Exact(v.into());
    let launchers = vec![
        launcher(
            Source::Vanilla,
            "/Users/you/Library/Application Support/minecraft",
            vec![
                found(
                    Source::Vanilla,
                    "Latest release",
                    WantedVersion::LatestRelease,
                    None,
                    &[],
                ),
                found(
                    Source::Vanilla,
                    "Fabric 1.20.1",
                    exact("1.20.1"),
                    Some((LoaderKind::Fabric, "0.16.14")),
                    &[],
                ),
            ],
        ),
        launcher(
            Source::Prism,
            "/Users/you/Library/Application Support/PrismLauncher",
            vec![
                found(
                    Source::Prism,
                    "Create: Above and Beyond",
                    exact("1.18.2"),
                    Some((LoaderKind::Forge, "40.2.4")),
                    &[],
                ),
                found(
                    Source::Prism,
                    "Fabulously Optimized",
                    exact("1.21.1"),
                    Some((LoaderKind::Fabric, "0.16.5")),
                    &[],
                ),
                found(
                    Source::Prism,
                    "NeoForge 1.20.1",
                    exact("1.20.1"),
                    Some((LoaderKind::Forge, "47.1.106")),
                    &["NeoForge for Minecraft 1.20.1 runs as Forge 47.1 in Lodestone Launcher."],
                ),
            ],
        ),
        launcher(
            Source::ModrinthApp,
            "/Users/you/Library/Application Support/ModrinthApp",
            vec![found(
                Source::ModrinthApp,
                "Cobblemon",
                exact("1.21.1"),
                Some((LoaderKind::Fabric, "0.16.9")),
                &[],
            )],
        ),
    ];
    let sizes: HashMap<String, Size> = [
        ("Vanilla:Latest release", 412_000_000, 380_000_000),
        ("Vanilla:Fabric 1.20.1", 96_000_000, 12_000_000),
        (
            "Prism:Create: Above and Beyond",
            2_400_000_000,
            1_900_000_000,
        ),
        ("Prism:Fabulously Optimized", 188_000_000, 0),
        ("Prism:NeoForge 1.20.1", 640_000_000, 210_000_000),
        ("ModrinthApp:Cobblemon", 1_100_000_000, 720_000_000),
    ]
    .into_iter()
    .map(|(key, total, worlds)| (key.to_string(), Size { total, worlds }))
    .collect();
    let icons: HashMap<String, Arc<RenderImage>> = [
        ("Prism:Create: Above and Beyond", [214, 160, 70]),
        ("Prism:Fabulously Optimized", [90, 160, 230]),
        ("ModrinthApp:Cobblemon", [60, 200, 120]),
        ("Vanilla:Fabric 1.20.1", [200, 190, 160]),
    ]
    .into_iter()
    .map(|(key, rgb)| (key.to_string(), icon(rgb)))
    .collect();
    let queue = if scene != "import" {
        vec![
            (
                "Vanilla:Fabric 1.20.1".to_string(),
                QueueState::Done {
                    id: "fabric-1-20-1".into(),
                    notes: Vec::new(),
                },
                None,
            ),
            (
                "Prism:Create: Above and Beyond".to_string(),
                QueueState::Importing,
                Some((1_020_000_000, 2_400_000_000)),
            ),
            (
                "ModrinthApp:Cobblemon".to_string(),
                QueueState::Waiting,
                None,
            ),
            (
                "Prism:NeoForge 1.20.1".to_string(),
                QueueState::Failed(
                    "Forge's version list couldn't be loaded: you're offline.".into(),
                ),
                None,
            ),
        ]
    } else {
        Vec::new()
    };
    let instances = shell.instances().clone();
    let dialog = cx.new(|cx| ImportDialog::preview(instances, launchers, sizes, icons, queue, cx));
    if scene == "import-stop" {
        // closed while importing: "Stop importing?"
        dialog.update(cx, |dialog, cx| dialog.request_close(cx));
    }
    shell.show_import(dialog, cx);
}
