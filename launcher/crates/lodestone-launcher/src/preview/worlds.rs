//! The Worlds tab's screenshots: made-up worlds (from the shared crate's `level.dat`
//! fixtures, renamed), backups and screenshots, drawn as blocky landscapes.

use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

use gpui::Context;
use launcher_core::{
    content::InstanceDirs,
    game::download::{CancelToken, Progress},
    worlds,
};
use lodestone_minecraft::nbt::{self, Tag};

use crate::instances::worlds::{Section, WorldsView};

const MODERN: &[u8] =
    include_bytes!("../../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");
const LEGACY: &[u8] =
    include_bytes!("../../../../../crates/lodestone-minecraft/testdata/level/1.8.9.dat");

/// A landscape: a sky gradient, hills of grass over dirt and stone, water below the sea
/// line, and a square sun, in blocks of `block` pixels.
pub fn landscape(width: u32, height: u32, palette: usize, seed: f32) -> image::RgbaImage {
    let palettes: [([u8; 3], [u8; 3], [u8; 3]); 4] = [
        ([96, 156, 236], [176, 214, 255], [92, 168, 64]), // day
        ([234, 120, 80], [252, 196, 120], [96, 140, 60]), // sunset
        ([18, 22, 48], [52, 64, 110], [44, 86, 52]),      // night
        ([120, 190, 240], [220, 236, 250], [222, 206, 150]), // desert
    ];
    let (top, horizon, ground) = palettes[palette % palettes.len()];
    let block = (width / 80).max(2);
    let mix = |a: [u8; 3], b: [u8; 3], t: f32| {
        [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
    };
    let sea = height as f32 * 0.68;
    image::RgbaImage::from_fn(width, height, |x, y| {
        let bx = (x / block) as f32;
        let by = (y / block * block) as f32;
        let hill = height as f32 * 0.55
            + (bx * 0.09 + seed).sin() * height as f32 * 0.07
            + (bx * 0.023 + seed * 2.0).sin() * height as f32 * 0.12;
        let hill = (hill / block as f32).floor() * block as f32;
        let sun = {
            let (sx, sy) = (width as f32 * 0.78, height as f32 * 0.2);
            (x as f32 - sx).abs() < width as f32 * 0.035
                && (y as f32 - sy).abs() < width as f32 * 0.035
        };
        let pixel = if by < hill {
            if by > sea - 1.0 && palette != 3 {
                [52, 96, 200]
            } else if sun && palette != 2 {
                [255, 244, 190]
            } else if sun {
                [230, 230, 240]
            } else {
                mix(top, horizon, (by / sea).min(1.0))
            }
        } else if by < hill + block as f32 {
            ground
        } else if by < hill + block as f32 * 4.0 {
            mix(ground, [134, 96, 67], 0.7)
        } else {
            let noise = ((bx * 12.9898 + by * 78.233).sin() * 43758.545)
                .fract()
                .abs();
            if noise > 0.93 {
                [70, 70, 76]
            } else {
                [128, 128, 132]
            }
        };
        // A darker edge on every block, like the game's textures.
        let edge = x % block == 0 || y % block == 0;
        let shade = if edge { 0.88 } else { 1.0 };
        image::Rgba([
            (pixel[0] as f32 * shade) as u8,
            (pixel[1] as f32 * shade) as u8,
            (pixel[2] as f32 * shade) as u8,
            255,
        ])
    })
}

fn save_png(image: &image::RgbaImage, path: &Path) {
    let _ = image.save(path);
}

fn set_time(path: &Path, time: SystemTime) {
    if let Ok(file) = fs::File::options().write(true).open(path) {
        let _ = file.set_modified(time);
    }
}

/// `level.dat` from a fixture with another name, mode and version.
fn level_dat(
    fixture: &[u8],
    name: &str,
    game_type: i32,
    hardcore: bool,
    cheats: bool,
    version: Option<&str>,
    last_played: SystemTime,
) -> Vec<u8> {
    let Ok(mut root) = lodestone_minecraft::level::read_level(fixture) else {
        return fixture.to_vec();
    };
    if let Some(data) = root.value.get_mut("Data").and_then(Tag::as_compound_mut) {
        data.set("LevelName", Tag::String(name.into()));
        data.set("GameType", Tag::Int(game_type));
        data.set("hardcore", Tag::Byte(i8::from(hardcore)));
        data.set("allowCommands", Tag::Byte(i8::from(cheats)));
        let ms = last_played
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        data.set("LastPlayed", Tag::Long(ms));
        if let (Some(version), Some(Tag::Compound(v))) = (version, data.get_mut("Version")) {
            v.set("Name", Tag::String(version.into()));
        }
    }
    nbt::gzip(&nbt::write(&root))
}

/// Folder, name, `level.dat` fixture, game mode, hardcore, cheats, version, last played,
/// icon palette and size in MB.
type SampleWorld<'a> = (
    &'a str,
    &'a str,
    &'a [u8],
    i32,
    bool,
    bool,
    Option<&'a str>,
    SystemTime,
    usize,
    u64,
);

/// Writes the sample worlds, backups and screenshots into the instance.
pub fn fill(instance_dir: &Path) {
    let dirs = InstanceDirs::new(instance_dir);
    let saves = worlds::saves_dir(&dirs);
    let now = SystemTime::now();
    let hours = |h: u64| now - Duration::from_secs(h * 3600);
    let worlds: [SampleWorld; 4] = [
        (
            "New World",
            "Survival Island",
            MODERN,
            0,
            false,
            false,
            None,
            hours(1),
            0,
            38,
        ),
        (
            "Castle",
            "Castle Build",
            MODERN,
            1,
            false,
            true,
            None,
            hours(30),
            3,
            112,
        ),
        (
            "Hardcore run",
            "One Life",
            MODERN,
            0,
            true,
            false,
            Some("1.20.1"),
            hours(24 * 9),
            1,
            21,
        ),
        (
            "Old Hardcore",
            "Old Hardcore",
            LEGACY,
            0,
            true,
            true,
            None,
            hours(24 * 400),
            2,
            6,
        ),
    ];
    for (folder, name, fixture, mode, hardcore, cheats, version, played, palette, megabytes) in
        worlds
    {
        let dir = saves.join(folder);
        let _ = fs::create_dir_all(dir.join("region"));
        let _ = fs::write(
            dir.join("level.dat"),
            level_dat(fixture, name, mode, hardcore, cheats, version, played),
        );
        // A few region files, so the sizes look like a world's.
        for i in 0..4 {
            if let Ok(file) = fs::File::create(dir.join(format!("region/r.{i}.0.mca"))) {
                let _ = file.set_len(megabytes * 1024 * 1024 / 4);
            }
        }
        if folder != "Old Hardcore" {
            save_png(
                &landscape(64, 64, palette, folder.len() as f32),
                &dir.join("icon.png"),
            );
        }
    }
    // Two backups of the first world, a day apart.
    let progress = Progress::default();
    let cancel = CancelToken::new();
    for age in [26, 2] {
        if let Ok(backup) = worlds::backup(&dirs, "New World", &progress, &cancel) {
            set_time(&backup.path, hours(age));
        }
    }

    let shots = dirs.game.join("screenshots");
    let _ = fs::create_dir_all(&shots);
    for (index, (name, palette, age)) in [
        ("2026-11-08_14.03.12.png", 0, 2),
        ("2026-11-08_13.41.55.png", 1, 3),
        ("2026-11-07_22.10.03.png", 2, 18),
        ("2026-11-05_16.20.47.png", 3, 70),
        ("2026-11-02_11.08.31.png", 0, 140),
        ("2026-10-28_19.55.02.png", 1, 260),
    ]
    .into_iter()
    .enumerate()
    {
        let path = shots.join(name);
        save_png(&landscape(1280, 720, palette, index as f32 * 1.7), &path);
        set_time(&path, hours(age));
    }
}

pub fn apply(scene: &str, view: &mut WorldsView, cx: &mut Context<WorldsView>) {
    view.preview_load(cx);
    match scene {
        "worlds" => view.preview(Section::Worlds, None, None, cx),
        "worlds-backups" => view.preview(Section::Worlds, Some("New World"), None, cx),
        "screenshots" => view.preview(Section::Screenshots, None, None, cx),
        "screenshot-view" => {
            view.preview(Section::Screenshots, None, None, cx);
            view.preview_view(0, cx);
        }
        _ => {}
    }
}
