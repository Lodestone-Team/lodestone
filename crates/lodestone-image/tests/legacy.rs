//! The legacy readers against genuine files from Lodestone's own code.
//!
//! The fixtures in `tests/fixtures/legacy/` were written by the core and the Launcher
//! themselves (their own writers, run by throwaway tests against temporary folders at
//! commit f5ce27ad), not by hand:
//!
//! - `backup-fabric.json`, `backup-paper.json`: `core/src/backups/store.rs` scanning,
//!   copying and saving backups of a Fabric server (two worlds, a script, a link, an
//!   unreadable file, a folder named `saves`) and a Paper server (split level).
//! - `record.json`, `history.json`: `core/src/modpack/history/tests.rs`' install, update and
//!   restore of a test pack; `history-baseline.json`: a baseline of an uploaded pack.
//! - `snapshot.json`, `snapshot-worlds.json`, `survival.lodesnap`: the Launcher's snapshot
//!   store taking a full and a worlds-only snapshot, and exporting the full one.
//! - `modpack.json`, `content.json`: `ModpackRecord::save` and `Provenance::save`.
//! - `world-backup.zip`: `worlds::backup` of a world with a Nether.
//!
//! The file contents those programs wrote are the short strings below, so the tests know
//! every file's hashes.

use std::io::Cursor;

use lodestone_image::legacy::{
    core_backup, core_modpack, launcher_modpack, lodesnap, world_zip, KnownFiles, Note,
};
use lodestone_image::{Base, Game, Kind, Loader, ServerLayout, Source, WithheldReason};
use sha1::Digest as _;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/legacy/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn sha1_hex(bytes: &[u8]) -> String {
    sha1::Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    lodestone_image::hex::sha256_hex(bytes)
}

/// What a caller that hashed these files would know.
fn known(contents: &[&[u8]]) -> KnownFiles {
    let mut known = KnownFiles::new();
    for bytes in contents {
        known.add_file(&sha1_hex(bytes), &sha256_hex(bytes), bytes.len() as u64);
    }
    known
}

fn withheld(image: &lodestone_image::Image) -> Vec<(&str, &str)> {
    image
        .withheld
        .iter()
        .map(|w| (w.path.as_str(), w.reason.as_str()))
        .collect()
}

fn paths(image: &lodestone_image::Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

// ---- core backups ----

#[test]
fn fabric_backup_becomes_a_data_image_and_back_byte_for_byte() {
    let bytes = fixture("backup-fabric.json");
    let converted = core_backup::read(&bytes).unwrap();
    let image = &converted.image;
    image.verify().unwrap();
    assert_eq!(image.kind, Kind::Data);
    assert_eq!(image.label.as_deref(), Some("Before 1.21.2"));
    assert_eq!(image.game.minecraft, "1.21.1");
    assert_eq!(image.game.loader, Loader::Fabric);
    assert_eq!(image.producer.version, "0.5.1");
    let server = image.settings.server.as_ref().unwrap();
    assert_eq!(server.flavour.as_ref().unwrap().as_str(), "fabric");
    assert_eq!(server.properties["level-name"], "world");
    assert_eq!(
        paths(image),
        [
            ".lodestone_modpack/history.json",
            "config/fabric.toml",
            "eula.txt",
            "mods/fabric-api.jar",
            "saves/creative/level.dat",
            "saves/creative/region/r.0.0.mca",
            "saves/world/DIM-1/region/r.0.0.mca",
            "saves/world/level.dat",
            "saves/world/region/r.0.0.mca",
            "server.properties",
            "start.sh",
        ]
    );
    let worlds: Vec<(&str, Option<bool>)> = image
        .worlds
        .iter()
        .map(|w| (w.name.as_str(), w.quiesced))
        .collect();
    assert_eq!(worlds, [("creative", Some(true)), ("world", Some(true))]);
    let script = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "start.sh")
        .unwrap();
    assert_eq!(script.mode, 0o755);
    assert!(image.entries.iter().all(|e| e.source == Source::Object {}));
    // The server's folder named saves can't be told apart from worlds; the link and the
    // locked file were skipped.
    assert_eq!(
        withheld(image),
        [
            ("linked-mods", "symlink"),
            ("saves/strange.txt", "bad_name"),
            ("secret.db", "unreadable"),
        ]
    );
    assert!(converted.notes.contains(&Note::ExcludesNotRecorded));
    assert!(converted.rest.pinned);

    let back = core_backup::from_image(image, &converted.rest).unwrap();
    assert_eq!(back, core_backup::Manifest::from_json(&bytes).unwrap());
    assert_eq!(back.to_json(), bytes);
}

#[test]
fn paper_backup_folds_the_split_level_into_one_world() {
    let bytes = fixture("backup-paper.json");
    let converted = core_backup::read(&bytes).unwrap();
    let image = &converted.image;
    assert_eq!(image.game.loader, Loader::Vanilla);
    assert_eq!(
        paths(image),
        [
            "paper.jar",
            "plugins/LuckPerms.jar",
            "saves/world/.bukkit_nether/level.dat",
            "saves/world/.bukkit_the_end/level.dat",
            "saves/world/DIM-1/region/r.0.0.mca",
            "saves/world/DIM1/region/r.0.0.mca",
            "saves/world/level.dat",
            "saves/world/region/r.0.0.mca",
        ]
    );
    assert_eq!(image.worlds.len(), 1);
    assert_eq!(image.worlds[0].quiesced, Some(false));
    assert!(image.withheld.is_empty());
    let back = core_backup::from_image(image, &converted.rest).unwrap();
    assert_eq!(back.to_json(), bytes);
}

// ---- Launcher snapshots ----

#[test]
fn snapshot_becomes_a_full_image_without_local_settings() {
    let bytes = fixture("snapshot.json");
    let converted = lodesnap::read(&bytes).unwrap();
    let image = &converted.image;
    image.verify().unwrap();
    assert_eq!(image.kind, Kind::Full);
    assert_eq!(image.label.as_deref(), Some("Before the update"));
    assert_eq!(image.game.loader, Loader::Fabric);
    assert_eq!(image.game.loader_version.as_deref(), Some("0.16.10"));
    let client = image.settings.client.as_ref().unwrap();
    assert_eq!(client.recommended_memory_mb, Some(6144));
    assert_eq!(client.icon.as_ref().unwrap().sha256, sha256_hex(b"png"));
    assert_eq!(
        client.background.as_ref().unwrap().sha256,
        sha256_hex(b"jpg")
    );
    assert!(paths(image).contains(&"saves/World/DIM-1/region/r.0.0.mca"));
    assert!(paths(image).contains(&"mods/iris.jar.disabled"));
    assert!(paths(image).contains(&"options.txt"));
    let run = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "run.sh")
        .unwrap();
    assert_eq!(run.mode, 0o755);
    assert_eq!(run.modified, Some(1_794_660_764_000_000_000));
    let worlds: Vec<&str> = image.worlds.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(worlds, ["Other", "World"]);
    assert_eq!(
        withheld(image),
        [
            ("minecraft/cache/thing.bin", "excluded"),
            ("minecraft/saves/World/session.lock", "excluded"),
            ("minecraft/server.properties", "bad_name"),
        ]
    );
    // Launch settings stay out of the image, and are reported.
    let json = String::from_utf8(image.to_json_pretty()).unwrap();
    for local in [
        "/opt/java",
        "UseG1GC",
        "--demo",
        "MESA",
        "1280",
        "36000",
        "Friends",
    ] {
        assert!(!json.contains(local), "{local} is in the image");
    }
    let local = converted
        .notes
        .iter()
        .find_map(|n| match n {
            Note::LocalSettings(fields) => Some(fields.clone()),
            _ => None,
        })
        .unwrap();
    for field in [
        "java_override",
        "jvm_args",
        "game_args",
        "memory",
        "window",
        "env",
        "play_time",
    ] {
        assert!(local.iter().any(|f| f == field), "{field}");
    }
    assert!(!local.iter().any(|f| f == "game_version"));

    let back = lodesnap::from_image(image, &converted.rest).unwrap();
    assert_eq!(back, lodesnap::Snapshot::from_json(&bytes).unwrap());
    assert_eq!(back.to_json(), bytes);
}

#[test]
fn worlds_only_snapshot_is_a_data_image() {
    let bytes = fixture("snapshot-worlds.json");
    let converted = lodesnap::read(&bytes).unwrap();
    assert_eq!(converted.image.kind, Kind::Data);
    assert_eq!(converted.image.game.loader, Loader::Vanilla);
    assert!(converted.image.settings.client.is_none());
    let back = lodesnap::from_image(&converted.image, &converted.rest).unwrap();
    assert_eq!(back.to_json(), bytes);
}

#[test]
fn lodesnap_file_reads_its_manifest_with_the_launchers_checks() {
    let snapshot = lodesnap::read_lodesnap(Cursor::new(fixture("survival.lodesnap"))).unwrap();
    assert_eq!(
        snapshot,
        lodesnap::Snapshot::from_json(&fixture("snapshot.json")).unwrap()
    );
    assert!(lodesnap::read_lodesnap(Cursor::new(fixture("world-backup.zip"))).is_err());
    assert!(lodesnap::read_lodesnap(Cursor::new(b"not a zip".to_vec())).is_err());
}

// ---- core modpack record and history ----

const PACK_CONTENTS: [&[u8]; 9] = [
    b"keep",
    b"a1",
    b"a2",
    b"dropped",
    b"new",
    b"v1",
    b"v2",
    b"only in v2",
    b"mine",
];

/// The fixtures' server: a Fabric server with the level `world`.
fn server() -> ServerLayout {
    ServerLayout::new("world", [], false).unwrap()
}

#[test]
fn history_becomes_a_lineage_and_back_byte_for_byte() {
    let record = fixture("record.json");
    let history = fixture("history.json");
    let mut known = known(&PACK_CONTENTS);
    known.add_pack("pack", "v1", &"1".repeat(128));
    known.add_pack("pack", "v2", &"2".repeat(128));
    let lineage = core_modpack::read(Some(&record), Some(&history), &server(), &known).unwrap();
    assert_eq!(lineage.images.len(), 3);
    for (i, image) in lineage.images.iter().enumerate() {
        image.verify().unwrap();
        assert_eq!(image.kind, Kind::Content);
        assert_eq!(image.game.loader, Loader::Fabric);
        assert_eq!(image.game.loader_version.as_deref(), Some("0.15.11"));
        assert!(image.withheld.is_empty());
        let parent = i.checked_sub(1).map(|p| lineage.images[p].id.clone());
        assert_eq!(image.parent, parent);
    }
    let labels: Vec<&str> = lineage
        .images
        .iter()
        .map(|i| i.label.as_deref().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "Installed Test Pack 1",
            "Updated to Test Pack 2",
            "Went back to entry 1 (Test Pack 1)"
        ]
    );
    let update = &lineage.images[1];
    assert!(matches!(
        &update.base,
        Some(Base::ModrinthPack { project, version, .. }) if project == "pack" && version == "v2"
    ));
    let new = update
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/new.jar")
        .unwrap();
    assert_eq!(
        new.source,
        Source::Modrinth {
            project: "n".into(),
            version: "v2".into(),
            url: "https://cdn.modrinth.com/data/n/versions/v2/new.jar".into(),
        }
    );
    assert_eq!(new.sha256, sha256_hex(b"new"));
    let config = update
        .entries
        .iter()
        .find(|e| e.path.as_str() == "config/pack.toml")
        .unwrap();
    assert_eq!(config.source, Source::Base {});
    // Going back to entry 1 gives entry 1's image again, and the record is that too.
    assert_eq!(lineage.images[2].id, lineage.images[0].id);
    assert_eq!(lineage.record.as_ref().unwrap().id, lineage.images[2].id);

    let (record_back, history_back) =
        core_modpack::from_images(&lineage.images, lineage.record.as_ref(), &lineage.rest).unwrap();
    assert_eq!(record_back.unwrap().to_json(), record);
    assert_eq!(history_back.to_json(), history);

    // No image is its own parent: the record, with the same files as the newest entry,
    // has the image before that as its parent.
    let record_image = lineage.record.as_ref().unwrap();
    for image in lineage.images.iter().chain([record_image]) {
        assert_ne!(image.parent.as_deref(), Some(image.id.as_str()));
        image.validate().unwrap();
    }
    assert_eq!(
        record_image.parent.as_deref(),
        Some(lineage.images[1].id.as_str())
    );

    // A store keeps one manifest per id (entries 1 and 3 share one, and the record too),
    // and the rest says which image each entry had: the files come back from that alone.
    let mut by_id = std::collections::HashMap::new();
    for image in lineage.images.iter().chain([record_image]) {
        by_id.insert(image.id.clone(), image.clone());
    }
    assert_eq!(by_id.len(), 2);
    let images: Vec<_> = lineage
        .rest
        .entries
        .iter()
        .map(|entry| by_id[&entry.image].clone())
        .collect();
    let record_from_store = &by_id[lineage.rest.record_image.as_ref().unwrap()];
    // The rest can be kept beside the images.
    let rest: core_modpack::Rest =
        serde_json::from_slice(&serde_json::to_vec(&lineage.rest).unwrap()).unwrap();
    let (record_back, history_back) =
        core_modpack::from_images(&images, Some(record_from_store), &rest).unwrap();
    assert_eq!(record_back.unwrap().to_json(), record);
    assert_eq!(history_back.to_json(), history);
}

#[test]
fn a_disabled_file_is_listed_as_it_is_on_disk() {
    let record = fixture("record.json");
    let history = fixture("history.json");
    let mut known = known(&PACK_CONTENTS);
    known.add_pack("pack", "v1", &"1".repeat(128));
    let plain = core_modpack::read(Some(&record), Some(&history), &server(), &known).unwrap();
    let a_record_path = plain.record.as_ref().unwrap().entries[0].path.to_string();
    known.add_disabled(&a_record_path);
    let lineage = core_modpack::read(Some(&record), Some(&history), &server(), &known).unwrap();
    let record_image = lineage.record.as_ref().unwrap();
    let disabled = format!("{a_record_path}.disabled");
    assert!(record_image
        .entries
        .iter()
        .any(|e| e.path.as_str() == disabled));
    // History entries keep the names they recorded.
    for image in &lineage.images {
        assert!(image
            .entries
            .iter()
            .all(|e| !e.path.as_str().ends_with(".disabled")));
    }
    let (record_back, history_back) =
        core_modpack::from_images(&lineage.images, Some(record_image), &lineage.rest).unwrap();
    assert_eq!(record_back.unwrap().to_json(), record);
    assert_eq!(history_back.to_json(), history);

    // The Launcher's content.json the same way.
    let modpack = fixture("modpack.json");
    let content = fixture("content.json");
    let mut known = known_launcher();
    known.add_disabled("mods/sodium.jar");
    let converted =
        launcher_modpack::read(Some(&modpack), Some(&content), launcher_game(), &known).unwrap();
    let paths = paths(&converted.image);
    assert!(paths.contains(&"mods/sodium.jar.disabled"), "{paths:?}");
    assert!(!paths.contains(&"mods/sodium.jar"));
    // The rest can be kept beside the image.
    let rest: launcher_modpack::Rest =
        serde_json::from_slice(&serde_json::to_vec(&converted.rest).unwrap()).unwrap();
    assert_eq!(rest, converted.rest);
    let (modpack_back, content_back) =
        launcher_modpack::from_image(&converted.image, &converted.rest).unwrap();
    assert_eq!(modpack_back.unwrap().to_json(), modpack);
    assert_eq!(content_back.unwrap().to_json(), content);
}

/// Every image a fixture converts to loses nothing between its two views and what stays
/// local.
#[test]
fn every_converted_image_is_lossless() {
    let mut images = Vec::new();
    for name in ["backup-fabric.json", "backup-paper.json"] {
        images.push(core_backup::read(&fixture(name)).unwrap().image);
    }
    for name in [
        "snapshot.json",
        "snapshot-worlds.json",
        "snapshot-modeless.json",
    ] {
        images.push(lodesnap::read(&fixture(name)).unwrap().image);
    }
    let mut known_pack = known(&PACK_CONTENTS);
    known_pack.add_pack("pack", "v1", &"1".repeat(128));
    known_pack.add_pack("pack", "v2", &"2".repeat(128));
    let lineage = core_modpack::read(
        Some(&fixture("record.json")),
        Some(&fixture("history.json")),
        &server(),
        &known_pack,
    )
    .unwrap();
    images.extend(lineage.images);
    images.extend(lineage.record);
    images.push(
        launcher_modpack::read(
            Some(&fixture("modpack.json")),
            Some(&fixture("content.json")),
            launcher_game(),
            &known_launcher(),
        )
        .unwrap()
        .image,
    );
    images.push(
        world_zip::read(
            Cursor::new(fixture("world-backup.zip")),
            "World",
            launcher_game(),
        )
        .unwrap()
        .image,
    );
    for image in &images {
        lodestone_image::view::check_lossless(image)
            .unwrap_or_else(|e| panic!("{:?}: {e}", image.label));
    }
}

#[test]
fn history_without_hashes_withholds_every_file_and_still_goes_back() {
    let record = fixture("record.json");
    let history = fixture("history.json");
    let lineage = core_modpack::read(Some(&record), Some(&history), &server(), &()).unwrap();
    for image in &lineage.images {
        assert!(image.entries.is_empty());
        assert!(image.base.is_none());
        assert!(image
            .withheld
            .iter()
            .all(|w| w.reason == WithheldReason::Unreadable));
    }
    assert!(lineage.notes.contains(&Note::PackNotReferenced {
        name: "Test Pack".into()
    }));
    let (record_back, history_back) =
        core_modpack::from_images(&lineage.images, lineage.record.as_ref(), &lineage.rest).unwrap();
    assert_eq!(record_back.unwrap().to_json(), record);
    assert_eq!(history_back.to_json(), history);
}

#[test]
fn baseline_of_an_uploaded_pack() {
    let history = fixture("history-baseline.json");
    let lineage = core_modpack::read(None, Some(&history), &server(), &()).unwrap();
    assert_eq!(lineage.images.len(), 1);
    let image = &lineage.images[0];
    assert_eq!(image.base, None);
    assert_eq!(image.game.loader, Loader::Forge);
    assert_eq!(image.game.loader_version.as_deref(), Some("47.3.0"));
    assert!(lineage.record.is_none());
    let (record, back) = core_modpack::from_images(&lineage.images, None, &lineage.rest).unwrap();
    assert!(record.is_none());
    assert_eq!(back.to_json(), history);
}

// ---- Launcher modpack.json and content.json ----

const LAUNCHER_CONTENTS: [&[u8]; 6] = [
    b"sodium",
    b"iris",
    b"pack mod",
    b"faithful",
    b"from the pack",
    b"old sodium",
];

fn known_launcher() -> KnownFiles {
    let mut known = known(&LAUNCHER_CONTENTS);
    known.add_pack("1KVo5zza", "Xk3Lp9qA", &"a".repeat(128));
    known
}

fn launcher_game() -> Game {
    Game {
        minecraft: "1.21.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.10".into()),
    }
}

#[test]
fn launcher_records_become_a_content_image_and_back_byte_for_byte() {
    let modpack = fixture("modpack.json");
    let content = fixture("content.json");
    let mut known = known(&LAUNCHER_CONTENTS);
    known.add_pack("1KVo5zza", "Xk3Lp9qA", &"a".repeat(128));
    let converted =
        launcher_modpack::read(Some(&modpack), Some(&content), launcher_game(), &known).unwrap();
    let image = &converted.image;
    image.verify().unwrap();
    assert!(
        matches!(&image.base, Some(Base::ModrinthPack { project, .. }) if project == "1KVo5zza")
    );
    let source = |path: &str| {
        image
            .entries
            .iter()
            .find(|e| e.path.as_str() == path)
            .unwrap_or_else(|| panic!("{path}"))
    };
    assert_eq!(source("config/pack.toml").source, Source::Base {});
    assert!(matches!(
        source("mods/pack-mod.jar").source,
        Source::Modrinth { .. }
    ));
    // The user's own sodium replaced the pack's: its source is its own Modrinth version.
    let sodium = source("mods/sodium.jar");
    assert_eq!(sodium.sha256, sha256_hex(b"sodium"));
    assert!(matches!(&sodium.source, Source::Modrinth { version, .. } if version == "u1OtTp2l"));
    assert_eq!(sodium.env.server, lodestone_image::EnvValue::Unsupported);
    assert_eq!(sodium.env_from.len(), 1);
    assert_eq!(sodium.modified, Some(1_794_645_624_123_456_789));
    assert_eq!(
        source("mods/iris.jar").source,
        Source::Curseforge {
            project: 455508,
            file: 5700001,
            downloadable: false
        }
    );
    assert_eq!(
        source("resourcepacks/faithful.zip").source,
        Source::Object {}
    );

    let (modpack_back, content_back) =
        launcher_modpack::from_image(image, &converted.rest).unwrap();
    assert_eq!(modpack_back.unwrap().to_json(), modpack);
    assert_eq!(content_back.unwrap().to_json(), content);
}

#[test]
fn a_plain_instance_has_the_loader_as_its_base() {
    let content = fixture("content.json");
    let converted = launcher_modpack::read(
        None,
        Some(&content),
        launcher_game(),
        &known(&LAUNCHER_CONTENTS),
    )
    .unwrap();
    assert_eq!(converted.image.base, Some(Base::Loader {}));
    let (modpack, content_back) =
        launcher_modpack::from_image(&converted.image, &converted.rest).unwrap();
    assert!(modpack.is_none());
    assert_eq!(content_back.unwrap().to_json(), content);
}

// ---- Launcher world backups ----

#[test]
fn world_backup_zip_becomes_a_data_image() {
    let converted = world_zip::read(
        Cursor::new(fixture("world-backup.zip")),
        "World",
        launcher_game(),
    )
    .unwrap();
    let image = &converted.image;
    image.verify().unwrap();
    assert_eq!(image.kind, Kind::Data);
    let listed: Vec<(&str, String, u64)> = image
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.sha256.clone(), e.size))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "saves/World/DIM-1/region/r.0.0.mca",
                sha256_hex(b"nether"),
                6
            ),
            ("saves/World/level.dat", sha256_hex(b"world"), 5),
            ("saves/World/region/r.0.0.mca", sha256_hex(b"region"), 6),
        ]
    );
    assert_eq!(image.worlds[0].name, "World");
    assert_eq!(image.worlds[0].size, Some(17));
    assert!(image.withheld.is_empty());
}
