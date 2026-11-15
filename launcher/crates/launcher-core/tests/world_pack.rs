//! "Download this world" of a server built on a Modrinth modpack: the instance gets the
//! pack's client share (its client-only index files and `client-overrides/`) as the
//! server's players get it through the base (M4b), less what the server removed, with the
//! server's own files winning; when the pack can't be had, the instance is made without
//! those files and the import says so. Against a loopback Modrinth mock, never the
//! network; the core's file is built here as a core writes one.

mod modrinth;

use std::fs;
use std::io::Write as _;
use std::path::Path;

use futures::executor::block_on;
use launcher_core::content::provenance::Provenance;
use launcher_core::game::download::{CancelToken, Downloader, Progress, recording_sleep};
use launcher_core::http;
use launcher_core::images::Images;
use launcher_core::images::export::{self, Sources};
use launcher_core::images::from_server::{PackPlatform, PackShare};
use launcher_core::instances::InstanceStore;
use launcher_core::paths::DataDir;
use lodestone_image::hex::sha256_hex;
use lodestone_image::manifest::{App, Producer};
use lodestone_image::store::{Encoding, NEVER, Store};
use lodestone_image::{
    Base, CanonicalPath, Entry, EnvValue, Game, Image, Kind, Loader, Source, World,
};
use modrinth::{FakeVersion, ModrinthMock};
use serde_json::{Value, json};
use sha1::Digest as _;

const PACK_PROJECT: &str = "PACKproj";
const PACK_VERSION: &str = "pv1";
const CLIENT_ONLY: &[u8] = b"a mod only the pack's players run";
const CLIENT_ONLY_URL: &str = "https://cdn.modrinth.com/data/CLNTonly/versions/c1/client-only.jar";
const SERVER_BOTH: &[u8] = b"the server's own copy of both.jar";

fn sha1(bytes: &[u8]) -> String {
    format!("{:x}", sha1::Sha1::digest(bytes))
}

fn sha512(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha512::digest(bytes))
}

fn index_file(path: &str, url: &str, bytes: &[u8], client: &str, server: &str) -> Value {
    json!({
        "path": path,
        "hashes": { "sha1": sha1(bytes), "sha512": sha512(bytes) },
        "env": { "client": client, "server": server },
        "downloads": [url],
        "fileSize": bytes.len(),
    })
}

/// The pack's `.mrpack`: a client-only mod, a mod both run (the server has its own copy),
/// one the server removed, a shared config, a client-only config and a world in
/// `client-overrides/`.
fn pack() -> Vec<u8> {
    let removed = b"a mod the server took out of the pack";
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0",
        "name": "Test Pack",
        "files": [
            index_file("mods/client-only.jar", CLIENT_ONLY_URL, CLIENT_ONLY, "required", "unsupported"),
            index_file(
                "mods/both.jar",
                "https://cdn.modrinth.com/data/BOTHboth/versions/b1/both.jar",
                b"the pack's copy of both.jar",
                "required",
                "required",
            ),
            index_file(
                "mods/removed.jar",
                "https://cdn.modrinth.com/data/REMOVEDx/versions/r1/removed.jar",
                removed,
                "required",
                "unsupported",
            ),
        ],
        "dependencies": { "minecraft": "1.20.1", "fabric-loader": "0.16.14" },
    });
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default();
        for (name, bytes) in [
            ("modrinth.index.json", index.to_string().into_bytes()),
            ("overrides/config/pack.toml", b"shared = true".to_vec()),
            ("client-overrides/config/client.toml", b"zoom = 3".to_vec()),
            (
                "client-overrides/saves/Tutorial/level.dat",
                b"a tutorial world".to_vec(),
            ),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

fn entry(store: &Store, path: &str, bytes: &[u8]) -> Entry {
    store.put_bytes(bytes).unwrap();
    Entry::new(
        CanonicalPath::new(path).unwrap(),
        bytes.len() as u64,
        sha256_hex(bytes),
        Source::Object {},
    )
}

/// A core's export of a server built on the pack: its content image names the pack as its
/// base and the pack's `mods/removed.jar` as removed, and holds the server's own
/// `mods/both.jar`; the backup holds the world.
fn core_export(dir: &Path, pack_sha512: &str) -> std::path::PathBuf {
    let store = Store::new(dir.join("core-store"), Encoding::Raw);
    let core = Producer {
        app: App::Core,
        version: "0.6.0".into(),
    };
    let game = Game {
        minecraft: "1.20.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.14".into()),
    };
    let mut both = entry(&store, "mods/both.jar", SERVER_BOTH);
    both.env.client = EnvValue::Required;
    both.env.server = EnvValue::Required;
    let mut content = Image::new(Kind::Content, game.clone());
    content.producer = core.clone();
    content.entries = vec![both];
    content.base = Some(Base::ModrinthPack {
        project: PACK_PROJECT.into(),
        version: PACK_VERSION.into(),
        sha512: pack_sha512.into(),
    });
    content.removed = vec![CanonicalPath::new("mods/removed.jar").unwrap()];
    content.normalize();
    content.seal();
    let mut level = entry(&store, "saves/world/level.dat", b"the server's world");
    level.world = Some("world".into());
    let mut data = Image::new(Kind::Data, game);
    data.producer = core;
    data.label = Some("Pack SMP – Tonight".into());
    data.worlds = vec![World {
        name: "world".into(),
        level_name: None,
        data_version: None,
        game_version: None,
        size: None,
        last_played: None,
        quiesced: Some(true),
    }];
    data.entries = vec![level];
    data.content = Some(content.id.clone());
    data.normalize();
    data.seal();
    let file = dir.join("Pack SMP - Tonight.lodeimg");
    lodestone_image::container::export(&store, &data, &[content], &file, NEVER).unwrap();
    file
}

struct Launcher {
    _temp: tempfile::TempDir,
    data: DataDir,
    images: std::sync::Arc<Images>,
    store: InstanceStore,
    mock: ModrinthMock,
}

fn launcher() -> Launcher {
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path().join("data"));
    let store = InstanceStore::new(data.instances_dir());
    fs::create_dir_all(store.root()).unwrap();
    Launcher {
        images: Images::open(&data),
        data,
        store,
        mock: ModrinthMock::start(),
        _temp: temp,
    }
}

fn import(l: &Launcher, file: &Path) -> export::Imported {
    let downloader =
        Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1);
    block_on(export::import_file(
        &l.images,
        Sources {
            downloader: &downloader,
            modrinth: &l.mock.client(),
            curseforge: None,
        },
        export::ImportRequest {
            server: Some("Pack SMP"),
            ..export::ImportRequest::new(file)
        },
        &|_| Ok(()),
        &Progress::default(),
        &CancelToken::new(),
    ))
    .unwrap()
}

#[test]
fn the_packs_client_share_comes_with_the_world() {
    let l = launcher();
    let pack = pack();
    let file = core_export(l.data.root(), &sha512(&pack));
    l.mock.version(
        FakeVersion::new(
            PACK_VERSION,
            PACK_PROJECT,
            "1.0",
            &["fabric"],
            &["1.20.1"],
            "2026-01-01",
        )
        .file("Test Pack.mrpack", &pack),
    );
    l.mock.serve(CLIENT_ONLY_URL, CLIENT_ONLY);

    let preview = export::inspect(&file).unwrap();
    let from = preview.from_server.unwrap();
    assert_eq!(from.modpack, Some(PackPlatform::Modrinth));

    let imported = import(&l, &file);
    assert_eq!(imported.server_pack, Some(PackShare::Placed { files: 3 }));
    let id = &imported.instance.id;
    let game = l.store.game_dir(id);
    // the pack's client share: its client-only mod, checked, and its configs
    assert_eq!(
        fs::read(game.join("mods/client-only.jar")).unwrap(),
        CLIENT_ONLY
    );
    assert_eq!(
        fs::read(game.join("config/client.toml")).unwrap(),
        b"zoom = 3"
    );
    assert_eq!(
        fs::read(game.join("config/pack.toml")).unwrap(),
        b"shared = true"
    );
    // the server's own copy wins over the pack's; what it removed stays out
    assert_eq!(fs::read(game.join("mods/both.jar")).unwrap(), SERVER_BOTH);
    assert!(!game.join("mods/removed.jar").exists());
    // the server's world is the one that comes, never the pack's
    assert!(game.join("saves/world/level.dat").is_file());
    assert!(!game.join("saves/Tutorial").exists());
    // nothing of the staging is left, and the instance isn't linked to the pack
    assert!(!l.store.dir(id).join(".lodestone-pack-share").exists());
    assert!(!l.store.dir(id).join("modpack.json").exists());
    // content.json knows the pack's download
    let known = Provenance::load(&l.store.dir(id));
    assert_eq!(
        known.files["mods/client-only.jar"]
            .modrinth
            .as_ref()
            .unwrap()
            .project_id,
        "CLNTonly"
    );
    // the snapshot of what was sent links to the content as placed, pack files and all
    let snapshots = l.images.snapshots(id).unwrap();
    let store = l.images.store();
    let kept = store
        .load_manifest(id, &snapshots.records[0].image)
        .unwrap();
    let content = store
        .load_manifest(id, kept.content.as_deref().unwrap())
        .unwrap();
    let paths: Vec<&str> = content.entries.iter().map(|e| e.path.as_str()).collect();
    for present in [
        "mods/client-only.jar",
        "mods/both.jar",
        "config/client.toml",
    ] {
        assert!(paths.contains(&present), "{present} in {paths:?}");
    }
    assert_eq!(content.label.as_deref(), Some("Downloaded from Pack SMP"));
}

#[test]
fn a_pack_that_cant_be_had_leaves_its_files_out_and_says_so() {
    let l = launcher();
    let pack = pack();
    let file = core_export(l.data.root(), &sha512(&pack));
    // the mock has no such version: Modrinth answers 404
    let imported = import(&l, &file);
    let Some(PackShare::Skipped { why, reason }) = &imported.server_pack else {
        panic!("{:?}", imported.server_pack);
    };
    assert_eq!(*why, launcher_core::images::from_server::SkipReason::Gone);
    assert!(reason.contains("no longer on Modrinth"), "{reason}");
    let game = l.store.game_dir(&imported.instance.id);
    assert_eq!(fs::read(game.join("mods/both.jar")).unwrap(), SERVER_BOTH);
    assert!(game.join("saves/world/level.dat").is_file());
    for absent in [
        "mods/client-only.jar",
        "config/client.toml",
        "config/pack.toml",
    ] {
        assert!(!game.join(absent).exists(), "{absent}");
    }

    // a pack file whose download fails: nothing of the pack is placed
    let l = launcher();
    let file = core_export(l.data.root(), &sha512(&pack));
    l.mock.version(
        FakeVersion::new(
            PACK_VERSION,
            PACK_PROJECT,
            "1.0",
            &["fabric"],
            &["1.20.1"],
            "2026-01-01",
        )
        .file("Test Pack.mrpack", &pack),
    );
    let imported = import(&l, &file);
    assert!(
        matches!(imported.server_pack, Some(PackShare::Skipped { .. })),
        "{:?}",
        imported.server_pack
    );
    let game = l.store.game_dir(&imported.instance.id);
    for absent in [
        "mods/client-only.jar",
        "config/client.toml",
        "config/pack.toml",
    ] {
        assert!(!game.join(absent).exists(), "{absent}");
    }
    assert!(
        !l.store
            .dir(&imported.instance.id)
            .join(".lodestone-pack-share")
            .exists()
    );
}
