//! The modpack screenshots: the Modpack page of "New instance", an import in progress,
//! the Settings tab of an instance made from a pack, and the update's confirmation.

use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

use gpui::Context;
use launcher_core::{
    content::{InstanceDirs, install::InstallProgress},
    game::{
        download::ProgressSnapshot,
        loaders::{LoaderChoice, LoaderKind},
    },
    modpack::{FileSource, ModpackRecord, OverrideFile, PackFile, PackInfo, update},
};
use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthProject, ModrinthSearchResponse, RawProject, RawVersion},
    mrpack::PackDownload,
};
use serde_json::json;

use super::content::{icon, icon_url};
use crate::{
    instances::{Instances, modpacks::UpdatePrompt, new_instance::Mode},
    shell::{InstanceTab, LauncherShell},
};

const PACK: &str = "1KVo5zza";

fn record() -> ModpackRecord {
    ModpackRecord {
        project_id: PACK.into(),
        version_id: "fo640".into(),
        slug: "fabulously-optimized".into(),
        title: "Fabulously Optimized".into(),
        version_number: "6.4.0".into(),
        date_published: "2025-05-01T00:00:00Z".into(),
        icon_url: Some(icon_url(PACK)),
        files: BTreeMap::new(),
        curseforge: None,
    }
}

fn pack_version(
    id: &str,
    number: &str,
    kind: &str,
    game: &str,
    date: &str,
    size: u64,
) -> RawVersion {
    serde_json::from_value(json!({
        "id": id, "project_id": PACK, "name": number, "version_number": number,
        "version_type": kind, "loaders": ["fabric"], "game_versions": [game],
        "date_published": date, "downloads": 1,
        "files": [{"hashes": {"sha1": "0".repeat(40), "sha512": "0".repeat(128)},
            "url": format!("https://cdn.modrinth.com/data/{PACK}/versions/{id}/pack.mrpack"),
            "filename": format!("Fabulously Optimized {number}.mrpack"), "primary": true, "size": size}],
        "dependencies": [],
    }))
    .expect("a valid version")
}

fn versions() -> Vec<RawVersion> {
    vec![
        pack_version(
            "fo650",
            "6.5.0",
            "release",
            "1.20.1",
            "2025-09-12T00:00:00Z",
            74_000,
        ),
        pack_version(
            "fo650b",
            "6.5.0-beta.2",
            "beta",
            "1.20.1",
            "2025-08-30T00:00:00Z",
            73_000,
        ),
        pack_version(
            "fo640",
            "6.4.0",
            "release",
            "1.20.1",
            "2025-05-01T00:00:00Z",
            71_000,
        ),
        pack_version(
            "fo632",
            "6.3.2",
            "release",
            "1.20.1",
            "2025-02-11T00:00:00Z",
            70_000,
        ),
        pack_version(
            "fo5x",
            "5.12.1",
            "release",
            "1.19.4",
            "2024-06-20T00:00:00Z",
            64_000,
        ),
    ]
}

fn project() -> ModrinthProject {
    let raw: RawProject = serde_json::from_value(json!({
        "id": PACK, "slug": "fabulously-optimized", "project_type": "modpack",
        "title": "Fabulously Optimized",
        "description": "Improved performance, better graphics and familiar features, in one simple pack.",
        "body": "## What's inside\n\nA **performance** pack with *familiar* options:\n\n- Sodium and Lithium\n- Iris for shaders\n- Zoom, dynamic lights and more\n\nSee the [wiki](https://example.com/wiki) for the full list.",
        "icon_url": icon_url(PACK), "categories": ["optimization"], "loaders": ["fabric"],
        "downloads": 9_800_000, "followers": 4_200, "client_side": "required",
        "server_side": "unsupported",
    }))
    .expect("a valid project");
    raw.into()
}

fn results() -> ModrinthSearchResponse {
    let hits = [
        (PACK, "Fabulously Optimized", "robotkoer", 9_800_000_u64),
        ("mOgUt4GM", "Simply Optimized", "Bernasss12", 2_100_000),
        (
            "gvQqBUqZ",
            "Cobblemon Official Modpack",
            "Cobblemon",
            1_900_000,
        ),
        ("HVnmMxH1", "Better MC [FABRIC]", "SharkieTV", 1_400_000),
        ("Q1vvjJYV", "Additive", "Pinkspread", 980_000),
        ("NNAgCjsB", "Adrenaline", "Kaniee", 640_000),
    ];
    serde_json::from_value(json!({
        "hits": hits.iter().map(|(id, title, author, downloads)| json!({
            "project_id": id, "slug": id, "project_type": "modpack", "title": title,
            "description": "", "author": author, "icon_url": icon_url(id),
            "downloads": downloads, "follows": downloads / 1000, "categories": [],
            "versions": ["1.20.1"], "date_modified": "", "client_side": "required",
            "server_side": "optional",
        })).collect::<Vec<_>>(),
        "offset": 0, "limit": 20, "total_hits": hits.len(),
    }))
    .expect("valid results")
}

fn icons() -> std::collections::HashMap<String, Arc<gpui::RenderImage>> {
    [
        (PACK, [90, 150, 230]),
        ("mOgUt4GM", [140, 100, 200]),
        ("gvQqBUqZ", [90, 190, 120]),
        ("HVnmMxH1", [230, 150, 60]),
        ("Q1vvjJYV", [60, 120, 200]),
        ("NNAgCjsB", [200, 90, 90]),
    ]
    .into_iter()
    .map(|(id, rgb)| (icon_url(id), crate::images::render_image(icon(rgb))))
    .collect()
}

/// The files the old version wrote, written into the instance, with what the user did
/// since: a changed config and a mod of their own.
pub fn fill(instance_dir: &Path) {
    let dirs = InstanceDirs::new(instance_dir);
    let mods = dirs.game.join("mods");
    let config = dirs.game.join("config");
    let _ = fs::create_dir_all(&mods);
    let _ = fs::create_dir_all(&config);
    let mut files = BTreeMap::new();
    for (name, project) in [
        ("sodium-fabric-0.5.13+mc1.20.1.jar", "AANobbMI"),
        ("lithium-fabric-mc1.20.1-0.11.2.jar", "gvQqBUqZ"),
        ("entityculling-fabric-1.6.2-mc1.20.1.jar", "NNAgCjsB"),
        ("cull-less-leaves-1.0.6.jar", "HVnmMxH1"),
    ] {
        let path = mods.join(name);
        super::content::fabric_mod(&path, project, name, "1", None);
        files.insert(
            format!("mods/{name}"),
            PackFile {
                sha1: hash::sha1_of(&path).unwrap_or_default(),
                source: FileSource::Index,
                project: Some(project.into()),
            },
        );
    }
    for (name, text) in [
        ("sodium-options.json", "{\"quality\":{}}"),
        ("zoomify.json", "{}"),
    ] {
        let path = config.join(name);
        let _ = fs::write(&path, text);
        files.insert(
            format!("config/{name}"),
            PackFile {
                sha1: hash::sha1_of(&path).unwrap_or_default(),
                source: FileSource::Override,
                project: None,
            },
        );
    }
    // The user's changes since.
    let _ = fs::write(
        config.join("sodium-options.json"),
        "{\"quality\":{\"weather\":\"FAST\"}}",
    );
    super::content::fabric_mod(&mods.join("my-own-mod.jar"), "mine", "Mine", "1", None);
    let _ = ModpackRecord { files, ..record() }.save(&dirs.instance);
}

fn download(path: &str, project: &str) -> PackDownload {
    let bytes = path.as_bytes();
    PackDownload {
        path: path.into(),
        urls: vec![
            url::Url::parse(&format!(
                "https://cdn.modrinth.com/data/{project}/versions/x/{}",
                path.rsplit('/').next().unwrap_or(path)
            ))
            .expect("a URL"),
        ],
        sha1: launcher_core::game::download::sha1_hex(bytes),
        sha512: "0".repeat(128),
        size: 400_000 + bytes.len() as u64 * 1000,
    }
}

/// The plan for updating the made-up instance to 6.5.0.
fn prompt(instances: &Instances, id: &str) -> UpdatePrompt {
    let dirs = instances.content_dirs(id);
    let current = ModpackRecord::load(&dirs.instance).unwrap_or_else(record);
    let override_file = |path: &str, sha1: String| OverrideFile {
        path: path.into(),
        index: 0,
        size: 120,
        sha1,
    };
    let pack = PackInfo {
        name: "Fabulously Optimized".into(),
        version_number: Some("6.5.0".into()),
        summary: None,
        game_version: "1.20.1".into(),
        loader: Some((LoaderKind::Fabric, "0.16.14".into())),
        downloads: vec![
            download("mods/sodium-fabric-0.5.14+mc1.20.1.jar", "AANobbMI"),
            download("mods/lithium-fabric-mc1.20.1-0.11.3.jar", "gvQqBUqZ"),
            download("mods/entityculling-fabric-1.6.2-mc1.20.1.jar", "NNAgCjsB"),
            download("mods/zoomify-2.11.2.jar", "w7ThoJFB"),
            download("mods/moreculling-fabric-1.20.4-0.22.1.jar", "51shyZVL"),
        ],
        overrides: vec![
            override_file("config/sodium-options.json", "1".repeat(40)),
            override_file("config/zoomify.json", "2".repeat(40)),
            override_file("config/moreculling.toml", "3".repeat(40)),
        ],
        skipped: Vec::new(),
    };
    // entityculling is the same file as before.
    let mut pack = pack;
    if let Some(entry) = current
        .files
        .get("mods/entityculling-fabric-1.6.2-mc1.20.1.jar")
    {
        pack.downloads[2].sha1 = entry.sha1.clone();
    }
    let new_record = ModpackRecord {
        version_id: "fo650".into(),
        version_number: "6.5.0".into(),
        date_published: "2025-09-12T00:00:00Z".into(),
        ..record()
    };
    let plan = update::plan(
        &dirs,
        &current,
        &pack,
        new_record,
        Some(LoaderChoice {
            kind: LoaderKind::Fabric,
            version: "0.16.14".into(),
        }),
    );
    UpdatePrompt {
        instance_id: id.to_string(),
        instance_name: "Survival 1.20.1".into(),
        from_version: "6.4.0".into(),
        plan: Arc::new(plan),
        pack_path: dirs.instance.join(".lodestone-preview-pack.mrpack"),
        pack: Arc::new(pack),
    }
}

/// Shows an import in the sidebar.
pub fn importing(instances: &mut Instances, cx: &mut Context<Instances>) {
    let progress = Arc::new(InstallProgress::default());
    progress.set_label("Downloading 74 files");
    progress.downloads.set(ProgressSnapshot {
        files_total: 74,
        files_done: 31,
        bytes_total: 96 * 1024 * 1024,
        bytes_done: 41 * 1024 * 1024,
    });
    instances.preview_import("Fabulously Optimized", progress, cx);
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    match scene {
        "new-instance-modpack" => {
            shell.open_new_instance_with(Some(("", "")), None, cx);
            if let Some(dialog) = shell.new_instance_dialog() {
                dialog.update(cx, |dialog, cx| {
                    dialog.set_mode(Mode::Modpack, cx);
                    dialog.modpack_browser().clone().update(cx, |browser, cx| {
                        browser.preview("", results(), project(), versions(), icons(), cx)
                    });
                });
            }
        }
        "modpack-settings" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view() {
                settings.update(cx, |settings, _| {
                    settings.preview_modpack(ModpackRecord { ..record() }, versions())
                });
            }
        }
        "modpack-update" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view() {
                settings.update(cx, |settings, _| {
                    settings.preview_modpack(record(), versions())
                });
            }
            let id = shell.instances().read(cx).selected().map(|i| i.id.clone());
            if let Some(id) = id {
                let prompt = prompt(shell.instances().read(cx), &id);
                shell.show_modpack_update(prompt, cx);
            }
        }
        _ => {}
    }
}
