//! The Content tab's screenshots: sample mods, resource packs and shader packs written
//! into the made-up instance, their provenance, and prepared Modrinth answers.

use std::{collections::HashMap, fs, io::Write as _, path::Path};

use gpui::Context;
use launcher_core::content::{
    ContentKind, InstanceDirs,
    provenance::{Entry, ModrinthSource, Provenance},
    shaders::{ShaderLoader, ShaderModOffer},
};
use lodestone_minecraft::modrinth::{
    ModrinthProject, ModrinthSearchResponse, RawProject, RawVersion,
};
use serde_json::json;

use crate::instances::content::{ContentView, ListView};

/// A square icon in two shades of `rgb`, with a lighter block in the middle.
pub(super) fn icon(rgb: [u8; 3]) -> image::RgbaImage {
    image::RgbaImage::from_fn(64, 64, |x, y| {
        let inner = (16..48).contains(&x) && (16..48).contains(&y);
        let shade = if inner {
            1.25
        } else if (x / 8 + y / 8) % 2 == 0 {
            1.0
        } else {
            0.85
        };
        let c = |v: u8| ((v as f32 * shade).min(255.0)) as u8;
        image::Rgba([c(rgb[0]), c(rgb[1]), c(rgb[2]), 255])
    })
}

pub(super) fn png(rgb: [u8; 3]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(icon(rgb))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("a PNG encodes");
    bytes
}

fn write_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let _ = fs::create_dir_all(path.parent().expect("a folder"));
    let Ok(file) = fs::File::create(path) else {
        return;
    };
    let mut zip = zip::ZipWriter::new(file);
    for (name, bytes) in entries {
        if zip
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .is_ok()
        {
            let _ = zip.write_all(bytes);
        }
    }
    let _ = zip.finish();
}

pub(super) fn fabric_mod(path: &Path, id: &str, name: &str, version: &str, color: Option<[u8; 3]>) {
    let mut json = json!({"schemaVersion": 1, "id": id, "name": name, "version": version});
    let mut entries = Vec::new();
    if let Some(color) = color {
        json["icon"] = json!(format!("assets/{id}/icon.png"));
        entries.push((format!("assets/{id}/icon.png"), png(color)));
    }
    let mut all: Vec<(&str, Vec<u8>)> = vec![("fabric.mod.json", json.to_string().into_bytes())];
    all.extend(entries.iter().map(|(n, b)| (n.as_str(), b.clone())));
    write_zip(path, &all);
}

fn resource_pack(path: &Path, description: &str, format: serde_json::Value, color: [u8; 3]) {
    let mut pack = json!({"description": description});
    if let Some(object) = format.as_object() {
        for (k, v) in object {
            pack[k] = v.clone();
        }
    }
    write_zip(
        path,
        &[
            (
                "pack.mcmeta",
                json!({"pack": pack}).to_string().into_bytes(),
            ),
            ("pack.png", png(color)),
        ],
    );
}

fn source(project: &str, slug: &str, title: &str, version: &str, icon: bool) -> ModrinthSource {
    ModrinthSource {
        project_id: project.into(),
        version_id: format!("{project}-{version}"),
        slug: slug.into(),
        title: title.into(),
        icon_url: icon.then(|| icon_url(project)),
        version_number: version.into(),
        date_published: "2025-01-01T00:00:00Z".into(),
        url: Some(format!(
            "https://cdn.modrinth.com/data/{project}/versions/x/{slug}.jar"
        )),
        client_side: "required".into(),
        server_side: "optional".into(),
        version_type: None,
    }
}

/// A made-up Modrinth result: id, slug, title, author, description, downloads.
type Hit = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u64,
);

pub(super) fn icon_url(project: &str) -> String {
    format!("https://cdn.modrinth.com/data/{project}/icon.png")
}

/// Records `file` as coming from Modrinth, with its real size and time so the listing
/// trusts the entry without asking Modrinth.
fn remember(
    provenance: &mut Provenance,
    dirs: &InstanceDirs,
    kind: ContentKind,
    file: &str,
    source: ModrinthSource,
) {
    let path = dirs.content_dir(kind).join(file);
    let Ok(metadata) = fs::metadata(&path) else {
        return;
    };
    provenance.insert(
        kind,
        file,
        Entry {
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(launcher_core::content::provenance::nanos),
            sha1: lodestone_minecraft::hash::sha1_of(&path).unwrap_or_default(),
            sha512: lodestone_minecraft::hash::sha512_of(&path).unwrap_or_default(),
            modrinth: Some(source),
            looked_up: false,
            ..Default::default()
        },
    );
}

/// Writes the sample content of `scene` into the instance at `instance_dir`.
pub fn fill(instance_dir: &Path, scene: &str) {
    let dirs = InstanceDirs::new(instance_dir);
    let mods = dirs.content_dir(ContentKind::Mod);
    let packs = dirs.content_dir(ContentKind::ResourcePack);
    let shaders = dirs.content_dir(ContentKind::ShaderPack);
    let mut provenance = Provenance::default();

    // `--mods N`: N more mods, for timing long lists (`--bench-scroll`).
    let extra = std::env::args()
        .skip_while(|arg| arg != "--mods")
        .nth(1)
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0);
    for i in 0..extra {
        let shade = (i * 37 % 200) as u8;
        fabric_mod(
            &mods.join(format!("extra-mod-{i:04}-1.0.{i}.jar")),
            &format!("extra{i}"),
            &format!("Extra mod {i}"),
            &format!("1.0.{i}"),
            Some([40 + shade / 2, 90 + shade / 3, 160 - shade / 2]),
        );
    }

    fabric_mod(
        &mods.join("sodium-fabric-0.5.13+mc1.20.1.jar"),
        "sodium",
        "Sodium",
        "0.5.13+mc1.20.1",
        None,
    );
    fabric_mod(
        &mods.join("fabric-api-0.92.2+1.20.1.jar"),
        "fabric-api",
        "Fabric API",
        "0.92.2+1.20.1",
        None,
    );
    fabric_mod(
        &mods.join("modmenu-7.2.2.jar.disabled"),
        "modmenu",
        "Mod Menu",
        "7.2.2",
        None,
    );
    fabric_mod(
        &mods.join("lithium-fabric-0.11.2.jar"),
        "lithium",
        "Lithium",
        "0.11.2",
        None,
    );
    fabric_mod(
        &mods.join("homemade-tweaks.jar"),
        "homemade",
        "Homemade Tweaks",
        "1.0.0",
        Some([196, 120, 60]),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::Mod,
        "sodium-fabric-0.5.13+mc1.20.1.jar",
        source("AANobbMI", "sodium", "Sodium", "mc1.20.1-0.5.13", true),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::Mod,
        "fabric-api-0.92.2+1.20.1.jar",
        source(
            "P7dR8mSH",
            "fabric-api",
            "Fabric API",
            "0.92.2+1.20.1",
            true,
        ),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::Mod,
        "modmenu-7.2.2.jar.disabled",
        source("mOgUt4GM", "modmenu", "Mod Menu", "7.2.2", true),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::Mod,
        "lithium-fabric-0.11.2.jar",
        source("gvQqBUqZ", "lithium", "Lithium", "mc1.20.1-0.11.2", true),
    );
    if matches!(
        scene,
        "content-mods"
            | "content-shaders"
            | "content-browse"
            | "content-sort"
            | "content-browse-idle"
            | "content-filters"
            | "content-project"
            | "content-description"
            | "content-gallery"
            | "content-page-scrolled"
            | "content-project-narrow"
            | "content-cf-project"
            | "content-formatted-shaders"
    ) {
        fabric_mod(
            &mods.join("iris-1.7.6+mc1.20.1.jar"),
            "iris",
            "Iris",
            "1.7.6+mc1.20.1",
            None,
        );
        remember(
            &mut provenance,
            &dirs,
            ContentKind::Mod,
            "iris-1.7.6+mc1.20.1.jar",
            source("YL57xq9U", "iris", "Iris Shaders", "1.7.6+1.20.1", true),
        );
    }

    resource_pack(
        &packs.join("Faithful 32x - 1.20.1.zip"),
        "§7The §6Faithful§7 look, at 32x",
        json!({"pack_format": 15}),
        [90, 150, 70],
    );
    resource_pack(
        &packs.join("FreshAnimations_v1.9.zip"),
        "Breathing new life into mobs",
        json!({"pack_format": 15, "supported_formats": [15, 34]}),
        [200, 140, 70],
    );
    resource_pack(
        &packs.join("Old Classic Textures.zip"),
        "Classic textures from 1.12",
        json!({"pack_format": 3}),
        [120, 120, 160],
    );
    resource_pack(
        &packs.join("Stay True 1.21.zip"),
        "Made for 1.21",
        json!({"pack_format": 34}),
        [70, 130, 170],
    );
    write_zip(
        &packs.join("Nested Download.zip"),
        &[(
            "Nested Download/pack.mcmeta",
            b"{\"pack\": {\"pack_format\": 15, \"description\": \"\"}}".to_vec(),
        )],
    );
    let _ = fs::create_dir_all(packs.join("My Edits"));
    let _ = fs::write(
        packs.join("My Edits/pack.mcmeta"),
        json!({"pack": {"pack_format": 15, "description": "Textures I made"}}).to_string(),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::ResourcePack,
        "Faithful 32x - 1.20.1.zip",
        source("PB8Tee5S", "faithful-32x", "Faithful 32x", "1.20.1", false),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::ResourcePack,
        "FreshAnimations_v1.9.zip",
        source(
            "50dA9Sha",
            "fresh-animations",
            "Fresh Animations",
            "1.9",
            false,
        ),
    );
    let _ = fs::write(
        dirs.game.join("options.txt"),
        "version:3465\nautoJump:false\nresourcePacks:[\"vanilla\",\"fabric\",\"file/Old Classic Textures.zip\",\"file/FreshAnimations_v1.9.zip\",\"file/Faithful 32x - 1.20.1.zip\"]\nincompatibleResourcePacks:[\"file/Old Classic Textures.zip\"]\nlang:en_us\n",
    );

    write_zip(
        &shaders.join("ComplementaryReimagined_r5.3.zip"),
        &[("shaders/composite.fsh", b"x".to_vec())],
    );
    write_zip(
        &shaders.join("BSL_v8.2.09.zip"),
        &[("shaders/final.fsh", b"x".to_vec())],
    );
    write_zip(
        &shaders.join("MakeUp-UltraFast-9.2.zip"),
        &[("MakeUp-UltraFast/shaders/final.fsh", b"x".to_vec())],
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::ShaderPack,
        "ComplementaryReimagined_r5.3.zip",
        source(
            "HVnmMxH1",
            "complementary-reimagined",
            "Complementary Shaders - Reimagined",
            "r5.3",
            true,
        ),
    );
    remember(
        &mut provenance,
        &dirs,
        ContentKind::ShaderPack,
        "BSL_v8.2.09.zip",
        source("Q1vvjJYV", "bsl-shaders", "BSL Shaders", "v8.2.09", true),
    );
    let _ = fs::create_dir_all(dirs.game.join("config"));
    let _ = fs::write(
        dirs.game.join("config/iris.properties"),
        "#This file stores configuration options for Iris\nenableShaders=true\nshaderPack=ComplementaryReimagined_r5.3.zip\n",
    );
    if scene.starts_with("content-formatted") {
        formatted(&packs, &shaders, &dirs.game);
    }
    let _ = provenance.save(&dirs.instance);
    if scene == "loader-change" {
        for world in ["New World", "Creative build"] {
            let dir = dirs.game.join("saves").join(world);
            let _ = fs::create_dir_all(&dir);
            let _ = fs::write(dir.join("level.dat"), b"not a real level");
        }
    }
}

/// Packs whose names and descriptions use Minecraft's formatting: `§` codes in file
/// names, a JSON text component, and the dark colours the launcher lightens.
fn formatted(packs: &Path, shaders: &Path, game: &Path) {
    let description = json!([
        {"text": "A ", "color": "gray"},
        {"text": "golden", "color": "gold", "bold": true},
        {"text": " look, ", "color": "gray"},
        {"text": "secret", "obfuscated": true},
        " and ",
        {"text": "pink", "color": "#FF77AA", "italic": true},
        {"text": " · ", "color": "dark_gray"},
        {"text": "struck", "strikethrough": true},
        " ",
        {"text": "underlined", "underlined": true, "color": "aqua"}
    ]);
    write_zip(
        &packs.join("§6§lGold§fCraft §7[§a1.20§7].zip"),
        &[
            (
                "pack.mcmeta",
                json!({"pack": {"pack_format": 15, "description": description}})
                    .to_string()
                    .into_bytes(),
            ),
            ("pack.png", png([200, 160, 40])),
        ],
    );
    resource_pack(
        &packs.join("§0Dark §1Blue §8Grey.zip"),
        "§0black §1dark blue §8dark grey §4dark red§r, lightened to read",
        json!({"pack_format": 15}),
        [40, 40, 90],
    );
    // Enabled, so they are at the top of the list.
    let _ = fs::write(
        game.join("options.txt"),
        "version:3465\nresourcePacks:[\"vanilla\",\"file/FreshAnimations_v1.9.zip\",\"file/§0Dark §1Blue §8Grey.zip\",\"file/§6§lGold§fCraft §7[§a1.20§7].zip\"]\nlang:en_us\n",
    );
    let astra = "§r§lAstra§4§lLex§r§l_By_LexBoosT_§4§lV93.0§r§l.zip";
    write_zip(
        &shaders.join(astra),
        &[("shaders/final.fsh", b"x".to_vec())],
    );
    let _ = fs::write(
        game.join("config/iris.properties"),
        format!("enableShaders=true\nshaderPack={astra}\n"),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn version(
    id: &str,
    project: &str,
    number: &str,
    kind: &str,
    loaders: &[&str],
    games: &[&str],
    date: &str,
    size: u64,
) -> RawVersion {
    serde_json::from_value(json!({
        "id": id, "project_id": project, "name": number, "version_number": number,
        "version_type": kind, "loaders": loaders, "game_versions": games,
        "date_published": date, "downloads": 1,
        "files": [{"hashes": {"sha1": "0".repeat(40), "sha512": "0".repeat(128)},
            "url": format!("https://cdn.modrinth.com/data/{project}/versions/{id}/file.jar"),
            "filename": "file.jar", "primary": true, "size": size}],
        "dependencies": [],
    }))
    .expect("a valid version")
}

fn project(
    id: &str,
    slug: &str,
    title: &str,
    description: &str,
    downloads: u64,
) -> ModrinthProject {
    let raw: RawProject = serde_json::from_value(json!({
        "id": id, "slug": slug, "project_type": "mod", "title": title, "description": description,
        "icon_url": icon_url(id), "categories": ["optimization", "fabric", "quilt"],
        "loaders": ["fabric", "quilt"], "downloads": downloads, "followers": downloads / 1500,
        "client_side": "required", "server_side": "optional",
    }))
    .expect("a valid project");
    raw.into()
}

fn icons() -> HashMap<String, std::sync::Arc<gpui::RenderImage>> {
    [
        ("AANobbMI", [70, 160, 210]),
        ("P7dR8mSH", [210, 180, 120]),
        ("mOgUt4GM", [140, 100, 200]),
        ("gvQqBUqZ", [90, 190, 120]),
        ("YL57xq9U", [110, 80, 210]),
        ("HVnmMxH1", [230, 150, 60]),
        ("Q1vvjJYV", [60, 120, 200]),
        ("NNAgCjsB", [200, 90, 90]),
        ("Bh37bMuy", [80, 170, 170]),
        ("rpFrMove", [210, 110, 50]),
        ("rpFrExt0", [220, 160, 50]),
        ("rpFrFood", [120, 160, 60]),
        ("rpFrWatr", [50, 130, 210]),
        ("rpSkyDra", [70, 90, 200]),
        ("rpSkyBox", [120, 70, 170]),
        ("rpClouds", [150, 170, 210]),
    ]
    .into_iter()
    .map(|(id, rgb)| (icon_url(id), crate::images::render_image(icon(rgb))))
    .collect()
}

pub fn apply(scene: &str, view: &mut ContentView, cx: &mut Context<ContentView>) {
    match scene {
        "content-mods" => view.preview_state(
            ContentKind::Mod,
            None,
            HashMap::from([(
                "fabric-api-0.92.2+1.20.1.jar".to_string(),
                version(
                    "fapi",
                    "P7dR8mSH",
                    "0.92.6+1.20.1",
                    "release",
                    &["fabric"],
                    &["1.20.1"],
                    "2025-06-01T00:00:00Z",
                    2_000_000,
                ),
            )]),
            icons(),
            cx,
        ),
        "content-resourcepacks" | "content-formatted" => {
            view.preview_state(ContentKind::ResourcePack, None, HashMap::new(), icons(), cx)
        }
        "content-menu" => {
            view.preview_state(ContentKind::Mod, None, HashMap::new(), icons(), cx);
            view.menu_open = true;
        }
        "content-shaders" | "content-formatted-shaders" => {
            view.preview_state(ContentKind::ShaderPack, None, HashMap::new(), icons(), cx)
        }
        "content-shaders-missing" => view.preview_state(
            ContentKind::ShaderPack,
            Some(ShaderModOffer::Install {
                loader: ShaderLoader::Iris,
                version: Box::new(version(
                    "iris176",
                    "YL57xq9U",
                    "1.7.6+1.20.1",
                    "release",
                    &["fabric", "quilt"],
                    &["1.20.1"],
                    "2025-01-02T00:00:00Z",
                    2_500_000,
                )),
            }),
            HashMap::new(),
            icons(),
            cx,
        ),
        "content-filters-packs" | "content-filters-shaders" => {
            let kind = if scene == "content-filters-packs" {
                ContentKind::ResourcePack
            } else {
                ContentKind::ShaderPack
            };
            view.preview_state(kind, None, HashMap::new(), icons(), cx);
            let browse = &mut view.section_mut(kind).browse;
            browse.filters_open = true;
            browse
                .filters
                .toggle_category(if kind == ContentKind::ShaderPack {
                    "shadows"
                } else {
                    "audio"
                });
            browse.filters.open_source = true;
            view.set_view(kind, ListView::Modrinth, cx);
        }
        "content-shaders-vanilla" => {
            view.preview_state(ContentKind::ShaderPack, None, HashMap::new(), icons(), cx)
        }
        "content-search" | "content-search-modrinth-only" => {
            view.preview_state(ContentKind::ResourcePack, None, HashMap::new(), icons(), cx);
            let (text, hits, total): (&str, &[Hit], u64) = if scene == "content-search" {
                (
                    "fresh",
                    &[
                        (
                            "rpFrMove",
                            "fresh-moves",
                            "Fresh Moves",
                            "FreshTeam",
                            "New animations for the player",
                            3_200_000,
                        ),
                        (
                            "rpFrExt0",
                            "fresh-animations-extensions",
                            "Fresh Animations: Extensions",
                            "FreshTeam",
                            "More mobs, same style",
                            1_100_000,
                        ),
                        (
                            "rpFrFood",
                            "fresh-food",
                            "Fresh Food",
                            "Cookie",
                            "Food that looks good enough to eat",
                            640_000,
                        ),
                        (
                            "rpFrWatr",
                            "fresher-water",
                            "Fresher Water",
                            "Wave",
                            "Clearer, livelier water",
                            410_000,
                        ),
                    ],
                    214,
                )
            } else {
                (
                    "sky",
                    &[
                        (
                            "rpSkyDra",
                            "dramatic-skys",
                            "Dramatic Skys",
                            "Ezzenix",
                            "Hand-painted skies for every time of day",
                            4_800_000,
                        ),
                        (
                            "rpSkyBox",
                            "sky-boxes-plus",
                            "Sky Boxes Plus",
                            "Nimbus",
                            "Sunsets and night skies",
                            820_000,
                        ),
                        (
                            "rpClouds",
                            "better-clouds-pack",
                            "Better Clouds Pack",
                            "Cirrus",
                            "Softer, fuller clouds",
                            350_000,
                        ),
                    ],
                    87,
                )
            };
            let response: ModrinthSearchResponse = serde_json::from_value(json!({
                "hits": hits.iter().map(|(id, slug, title, author, description, downloads)| json!({
                    "project_id": id, "slug": slug, "project_type": "resourcepack", "title": title,
                    "description": description, "author": author, "icon_url": icon_url(id),
                    "downloads": downloads, "follows": downloads / 1400, "categories": [], "versions": [],
                    "date_modified": "", "client_side": "required", "server_side": "unsupported",
                })).collect::<Vec<_>>(),
                "offset": 0, "limit": 20, "total_hits": total,
            }))
            .expect("a valid response");
            view.preview_results(ContentKind::ResourcePack, text, response.normalize(), cx);
            // Typed in the installed list: its matches, then the way to Modrinth's.
            if scene == "content-search-modrinth-only" {
                view.set_view(ContentKind::ResourcePack, ListView::Modrinth, cx);
            }
        }
        "content-browse" | "content-sort" | "content-browse-idle" | "content-filters" => {
            view.preview_state(ContentKind::Mod, None, HashMap::new(), icons(), cx);
            let hits = [
                (
                    "AANobbMI",
                    "sodium",
                    "Sodium",
                    "JellySquid",
                    "The fastest and most compatible rendering optimization mod for Minecraft.",
                    98_400_000_u64,
                ),
                (
                    "P7dR8mSH",
                    "fabric-api",
                    "Fabric API",
                    "modmuss50",
                    "Lightweight and modular API providing common hooks and intercompatibility measures utilized by mods using the Fabric toolchain.",
                    120_300_000,
                ),
                (
                    "YL57xq9U",
                    "iris",
                    "Iris Shaders",
                    "coderbot",
                    "A modern shader pack loader for Minecraft intended to be compatible with existing OptiFine shader packs.",
                    70_100_000,
                ),
                (
                    "gvQqBUqZ",
                    "lithium",
                    "Lithium",
                    "JellySquid",
                    "No-compromises game logic optimization mod.",
                    41_000_000,
                ),
                (
                    "NNAgCjsB",
                    "entityculling",
                    "Entity Culling",
                    "tr7zw",
                    "Using async path-tracing to hide Block-/Entities that are not visible.",
                    38_200_000,
                ),
                (
                    "Bh37bMuy",
                    "reeses-sodium-options",
                    "Reese's Sodium Options",
                    "FlashyReese",
                    "Alternative Options Menu for Sodium.",
                    21_700_000,
                ),
            ];
            let response: ModrinthSearchResponse = serde_json::from_value(json!({
                "hits": hits.iter().map(|(id, slug, title, author, description, downloads)| json!({
                    "project_id": id, "slug": slug, "project_type": "mod", "title": title,
                    "description": description, "author": author, "icon_url": icon_url(id),
                    "downloads": downloads, "follows": downloads / 1400, "categories": [], "versions": [],
                    "date_modified": "", "client_side": if *slug == "entityculling" { "optional" } else { "required" },
                    "server_side": "optional",
                })).collect::<Vec<_>>(),
                "offset": 0, "limit": 20, "total_hits": 2481,
            }))
            .expect("a valid response");
            // Typed "so", or browsing with nothing typed (with the filter panel open).
            let text = if scene == "content-browse" { "so" } else { "" };
            // The sort list open, under its button in the toolbar.
            view.sort_open = scene == "content-sort";
            view.preview_results(ContentKind::Mod, text, response.normalize(), cx);
            view.set_view(ContentKind::Mod, ListView::Modrinth, cx);
            if scene == "content-filters" {
                let browse = &mut view.section_mut(ContentKind::Mod).browse;
                browse.filters_open = true;
                browse.filters.toggle_category("optimization");
                browse.filters.toggle_category("utility");
                browse.filters.environment =
                    lodestone_minecraft::modrinth::ClientEnvironment::ClientOnly;
            }
        }
        "content-cf-project" => {
            view.preview_state(ContentKind::Mod, None, HashMap::new(), icons(), cx);
            let project: launcher_core::content::curseforge::CfMod =
                serde_json::from_value(json!({
                    "id": 238222, "name": "Just Enough Items (JEI)", "slug": "jei",
                    "summary": "View Items and Recipes", "downloadCount": 412_000_000.0,
                    "authors": [{ "name": "mezz" }],
                    "screenshots": [{ "title": "Recipes", "url": CF_SHOT_URL }],
                }))
                .expect("a valid project");
            view.preview_cf_project(ContentKind::Mod, project, CF_DESCRIPTION.into(), vec![], cx);
            view.preview_description_image(
                CF_SHOT_URL,
                crate::images::render_image(super::worlds::landscape(1280, 720, 1, 0.7)),
            );
        }
        "content-project"
        | "content-description"
        | "content-gallery"
        | "content-page-scrolled"
        | "content-project-narrow" => {
            view.preview_state(ContentKind::Mod, None, HashMap::new(), icons(), cx);
            let versions = vec![
                version(
                    "AANobbMI-mc1.20.1-0.5.13",
                    "AANobbMI",
                    "mc1.20.1-0.5.13",
                    "release",
                    &["fabric", "quilt"],
                    &["1.20.1"],
                    "2025-03-10T00:00:00Z",
                    1_140_000,
                ),
                version(
                    "s6",
                    "AANobbMI",
                    "mc1.20.1-0.5.11",
                    "release",
                    &["fabric", "quilt"],
                    &["1.20.1"],
                    "2024-07-01T00:00:00Z",
                    1_120_000,
                ),
                version(
                    "s5",
                    "AANobbMI",
                    "mc1.20.1-0.5.10-beta.2",
                    "beta",
                    &["fabric"],
                    &["1.20.1"],
                    "2024-05-01T00:00:00Z",
                    1_100_000,
                ),
                version(
                    "s4",
                    "AANobbMI",
                    "mc1.20.1-0.5.8",
                    "release",
                    &["fabric", "quilt"],
                    &["1.20", "1.20.1"],
                    "2024-02-01T00:00:00Z",
                    1_050_000,
                ),
                version(
                    "s9",
                    "AANobbMI",
                    "mc1.21.1-0.6.13-fabric",
                    "release",
                    &["fabric"],
                    &["1.21.1"],
                    "2025-04-01T00:00:00Z",
                    1_300_000,
                ),
            ];
            let mut sodium = project(
                "AANobbMI",
                "sodium",
                "Sodium",
                "The fastest and most compatible rendering optimization mod for Minecraft. Now available for both NeoForge and Fabric!",
                98_400_000,
            );
            sodium.body = DESCRIPTION.into();
            sodium.gallery = GALLERY
                .iter()
                .enumerate()
                .map(
                    |(index, (url, title))| lodestone_minecraft::modrinth::ModrinthGalleryImage {
                        url: url.to_string(),
                        title: Some(title.to_string()),
                        description: None,
                        featured: index == 0,
                    },
                )
                .collect();
            view.preview_project(ContentKind::Mod, sodium, versions, cx);
            view.preview_description_image(
                BANNER_URL,
                crate::images::render_image(super::worlds::landscape(900, 140, 1, 0.4)),
            );
            for (index, (url, _)) in GALLERY.iter().enumerate() {
                view.preview_description_image(
                    url,
                    crate::images::render_image(super::worlds::landscape(
                        1280,
                        720,
                        index + 2,
                        0.5,
                    )),
                );
            }
            let tab = match scene {
                "content-project" => crate::instances::content::ProjectTab::Versions,
                "content-gallery" => crate::instances::content::ProjectTab::Gallery,
                _ => crate::instances::content::ProjectTab::Description,
            };
            view.set_project_tab(ContentKind::Mod, tab, cx);
            // The description read part of the way down: "Back to top" shows.
            if scene == "content-page-scrolled" {
                view.page_scroll
                    .set_offset(gpui::point(gpui::px(0.), gpui::px(-480.)));
            }
        }
        _ => {}
    }
}

const BANNER_URL: &str = "https://cdn.modrinth.com/data/AANobbMI/images/banner.png";

const CF_SHOT_URL: &str = "https://media.forgecdn.net/attachments/1/2/recipes.png";

/// A made-up CurseForge description: HTML with headings, styles, links and an image.
const CF_DESCRIPTION: &str = r#"<h2>About</h2>
<p><strong>JEI</strong> is an item and recipe viewing mod, built from the ground up for <em>stability</em> and performance.</p>
<p><a href="https://example.com/wiki"><img src="https://media.forgecdn.net/attachments/1/2/recipes.png" alt="Recipes"></a></p>
<h3>Features</h3>
<ul>
<li>Search items by name, mod or tag</li>
<li>See every recipe that makes or uses an item</li>
<li>Read the <a href="https://example.com/wiki">wiki</a> for more</li>
</ul>"#;

/// Made-up gallery images, by URL and title.
const GALLERY: &[(&str, &str)] = &[
    (
        "https://cdn.modrinth.com/data/AANobbMI/images/far.png",
        "A 32-chunk render distance",
    ),
    (
        "https://cdn.modrinth.com/data/AANobbMI/images/night.png",
        "Night in a dense forest",
    ),
];

/// A made-up description with every block the renderer draws, and HTML that is shown
/// as text.
const DESCRIPTION: &str = r#"<div align="center">
<img src="https://cdn.modrinth.com/data/AANobbMI/images/banner.png" alt="Sodium">
</div>

## A faster renderer

Sodium replaces the game's chunk renderer with a **much faster** one, and fixes *many* graphical issues along the way. It works with `Fabric` and NeoForge.

- Higher and steadier frame rates
- Less stutter while chunks load
  1. smaller memory use
  2. faster chunk meshing
- Works with [Iris](https://modrinth.com/mod/iris) for shaders

> Sodium is not a magic button: very slow graphics drivers stay slow.

### Settings

| Option | Default |
|---|---|
| Chunk builder threads | Auto |
| Use block face culling | On |

```
-Dsodium.checks.issue2561=false
```

---

Report problems on [GitHub](https://github.com/CaffeineMC/sodium/issues). Links like [this one](javascript:alert(1)) are shown as plain text. <script>alert("never runs")</script>
"#;
