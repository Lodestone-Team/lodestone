//! Adding a modpack to an instance made for a server, against a loopback Modrinth mock:
//! the server's pack wins, nothing the user has is replaced, a server update that brings
//! a mod the added pack had removes the added copy, and removing the added pack removes
//! only its unchanged files.

mod modrinth;

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use futures::executor::block_on;
use launcher_core::{
    content::{
        ContentKind, InstanceDirs, Platforms,
        install::InstallProgress,
        provenance::{Entry, ModrinthSource, Provenance},
    },
    cores::sync,
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
    instances::{InstanceStore, Loader},
    modpack::{
        self, ModpackRecord,
        addon::{self, AddedPacks},
        import::{self, ImportOptions},
        update,
    },
};
use modrinth::ModrinthMock;
use serde_json::{Value, json};
use sha1::Digest as _;

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1)
}

fn sha1(bytes: &[u8]) -> String {
    format!("{:x}", sha1::Sha1::digest(bytes))
}

fn sha512(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha512::digest(bytes))
}

fn cdn(project: &str, file: &str) -> String {
    format!("https://cdn.modrinth.com/data/{project}/versions/v1/{file}")
}

/// An index entry for `bytes` at `path`, from Modrinth's CDN as `project`, served by the
/// mock.
fn file(mock: &ModrinthMock, path: &str, project: &str, bytes: &[u8]) -> Value {
    let name = path.rsplit('/').next().unwrap();
    let url = cdn(project, name);
    mock.serve(&url, bytes);
    json!({
        "path": path,
        "hashes": { "sha1": sha1(bytes), "sha512": sha512(bytes) },
        "env": { "client": "required", "server": "optional" },
        "downloads": [url],
        "fileSize": bytes.len(),
    })
}

fn write_pack(
    path: &Path,
    name: &str,
    dependencies: Value,
    files: Vec<Value>,
    entries: &[(&str, &[u8])],
) {
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": name,
        "files": files,
        "dependencies": dependencies,
    });
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("modrinth.index.json", options).unwrap();
    zip.write_all(index.to_string().as_bytes()).unwrap();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn fabric() -> Value {
    json!({ "minecraft": "1.20.1", "fabric-loader": "0.16.14" })
}

struct Fixture {
    _temp: tempfile::TempDir,
    mock: ModrinthMock,
    store: InstanceStore,
    packs: PathBuf,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(temp.path().join("instances"));
    fs::create_dir_all(store.root()).unwrap();
    let packs = temp.path().join("packs");
    fs::create_dir_all(&packs).unwrap();
    Fixture {
        mock: ModrinthMock::start(),
        store,
        packs,
        _temp: temp,
    }
}

const LITHIUM_SERVER: &[u8] = b"lithium as the server has it";
const CONTENT_MOD: &[u8] = b"the server's content mod";
const SODIUM: &[u8] = b"sodium from the optimization pack";
const LITHIUM_PACK: &[u8] = b"another lithium build";
const IRIS_PACK: &[u8] = b"iris from the pack";
const MENU: &[u8] = b"a menu mod";

/// The server's client pack, version 1: Lithium and a content mod, and a config.
fn server_pack(fixture: &Fixture, name: &str, extra: Vec<Value>) -> PathBuf {
    let mock = &fixture.mock;
    let mut files = vec![
        file(mock, "mods/lithium.jar", "LITH", LITHIUM_SERVER),
        file(mock, "mods/content.jar", "CONT", CONTENT_MOD),
    ];
    files.extend(extra);
    let path = fixture.packs.join(format!("{name}.mrpack"));
    write_pack(
        &path,
        "SMP",
        fabric(),
        files,
        &[("overrides/config/server.toml", b"the server's config")],
    );
    path
}

/// The optimization pack the user adds: Sodium, another Lithium build, Iris (which the
/// user has installed already), a mod at a path the user has a file at, and configs.
fn optimization_pack(fixture: &Fixture) -> PathBuf {
    let mock = &fixture.mock;
    let path = fixture.packs.join("optimized.mrpack");
    write_pack(
        &path,
        "Optimized",
        json!({ "minecraft": "1.20.1", "fabric-loader": "0.16.10" }),
        vec![
            file(mock, "mods/sodium.jar", "SOD", SODIUM),
            file(mock, "mods/lithium-fabric.jar", "LITH", LITHIUM_PACK),
            file(mock, "mods/iris.jar", "IRIS", IRIS_PACK),
            file(mock, "mods/menu.jar", "MENU", MENU),
        ],
        &[
            ("overrides/config/sodium.toml", b"sodium settings"),
            ("overrides/config/server.toml", b"the pack's take on it"),
            ("overrides/options.txt", b"the pack's options"),
        ],
    );
    path
}

fn record(title: &str, project: &str) -> ModpackRecord {
    ModpackRecord {
        project_id: project.into(),
        version_id: "v1".into(),
        title: title.into(),
        version_number: "1.0.0".into(),
        ..ModpackRecord::default()
    }
}

#[test]
fn an_added_pack_never_overrides_the_servers_mods() {
    let fixture = fixture();
    let api = fixture.mock.client();
    let platforms = Platforms::new(Some(&api), None);

    // The instance made for the server, as "Create matching instance" makes it.
    let v1 = server_pack(&fixture, "smp-1", Vec::new());
    let server = modpack::read_pack(&v1).unwrap();
    let instance = block_on(import::import(
        &fixture.store,
        Some(&api),
        &downloader(),
        &v1,
        &server,
        ImportOptions {
            name: "SMP".into(),
            loader: None,
            record: sync::record(&server),
            icon: None,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let mut instance = instance;
    instance.loader = Loader::Fabric;
    instance.loader_version = Some("0.16.14".into());
    let dirs = InstanceDirs::new(fixture.store.dir(&instance.id));
    let game = &dirs.game;

    // What the user did since: their own Iris build, a file at a path the pack uses,
    // and their options.
    fs::write(game.join("mods/iris-1.7.jar"), b"my iris").unwrap();
    Provenance::update(&dirs.instance, |p| {
        p.insert(
            ContentKind::Mod,
            "iris-1.7.jar",
            Entry {
                sha1: sha1(b"my iris"),
                modrinth: Some(ModrinthSource {
                    project_id: "IRIS".into(),
                    version_id: "iris17".into(),
                    ..ModrinthSource::default()
                }),
                ..Entry::default()
            },
        );
    })
    .unwrap();
    fs::write(game.join("mods/menu.jar"), b"my menu build").unwrap();
    fs::write(game.join("options.txt"), b"fov:90\n").unwrap();

    // Only a pack for the same Minecraft version and loader can be added.
    let added = modpack::read_pack(&optimization_pack(&fixture)).unwrap();
    assert_eq!(addon::incompatibility(&instance, &added), None);
    let mut other = added.clone();
    other.game_version = "1.21.1".into();
    assert!(
        addon::incompatibility(&instance, &other)
            .unwrap()
            .contains("1.21.1")
    );
    let mut forge = added.clone();
    forge.loader = Some((
        launcher_core::game::loaders::LoaderKind::Forge,
        "47.4.0".into(),
    ));
    assert!(
        addon::incompatibility(&instance, &forge)
            .unwrap()
            .contains("Forge")
    );

    let plan = addon::plan(&dirs, &added, record("Optimized", "OPT"));
    assert_eq!(plan.added, ["mods/sodium.jar"]);
    assert_eq!(plan.configs, ["config/sodium.toml"]);
    assert_eq!(
        plan.kept_instance_pack,
        ["mods/lithium-fabric.jar", "config/server.toml"]
    );
    assert_eq!(
        plan.already_there,
        ["mods/iris.jar", "mods/menu.jar", "options.txt"]
    );
    assert_eq!(plan.download_size(), SODIUM.len() as u64);

    let optimized = optimization_pack(&fixture);
    let pack = block_on(addon::apply(
        platforms,
        &downloader(),
        &dirs,
        &plan,
        &optimized,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let read = |p: &str| fs::read(game.join(p)).unwrap();
    assert_eq!(read("mods/sodium.jar"), SODIUM);
    assert_eq!(read("config/sodium.toml"), b"sodium settings");
    assert_eq!(read("mods/lithium.jar"), LITHIUM_SERVER);
    assert!(!game.join("mods/lithium-fabric.jar").exists());
    assert!(!game.join("mods/iris.jar").exists());
    assert_eq!(read("mods/menu.jar"), b"my menu build");
    assert_eq!(read("config/server.toml"), b"the server's config");
    assert_eq!(read("options.txt"), b"fov:90\n");
    // The server's record is untouched, and the added pack has its own.
    let own = ModpackRecord::load(&dirs.instance).unwrap();
    assert_eq!(own.title, "SMP");
    assert!(!own.files.contains_key("mods/sodium.jar"));
    let packs = AddedPacks::load(&dirs.instance);
    assert_eq!(packs.packs, std::slice::from_ref(&pack));
    assert_eq!(
        pack.record.files.keys().collect::<Vec<_>>(),
        ["config/sodium.toml", "mods/sodium.jar"]
    );
    assert_eq!(
        pack.record.files["mods/sodium.jar"].project.as_deref(),
        Some("SOD")
    );
    assert_eq!(
        Provenance::load(&dirs.instance)
            .get(ContentKind::Mod, "sodium.jar")
            .and_then(|e| e.modrinth.as_ref())
            .map(|m| m.project_id.as_str()),
        Some("SOD")
    );
    assert!(packs.same_project(&record("Optimized 2", "OPT")).is_some());
    // No staging folder is left.
    assert!(
        fs::read_dir(&dirs.instance)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with(".lodestone"))
    );
    // Adding it again writes nothing.
    let again = addon::plan(&dirs, &added, record("Optimized", "OPT"));
    assert!(!again.changes_files(), "{again:?}");

    // The server's mods stay in step: its pack is unchanged, so nothing changes.
    assert!(sync::mods_on_disk(&dirs));
    assert!(!sync::plan(&dirs, &server, None).changes_files());

    // The server adds Sodium itself, under another name: the added copy goes, so the
    // game doesn't load two Sodiums.
    const SODIUM_SERVER: &[u8] = b"sodium as the server has it";
    let v2 = server_pack(
        &fixture,
        "smp-2",
        vec![file(
            &fixture.mock,
            "mods/sodium-fabric.jar",
            "SOD",
            SODIUM_SERVER,
        )],
    );
    let server2 = modpack::read_pack(&v2).unwrap();
    let plan2 = sync::plan(&dirs, &server2, None);
    assert_eq!(plan2.added, ["mods/sodium-fabric.jar"]);
    assert_eq!(plan2.removed, ["mods/sodium.jar"]);
    let outcome = block_on(update::apply(
        Some(&api),
        &downloader(),
        &dirs,
        &plan2,
        &v2,
        false,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert!(outcome.snapshot.join("minecraft/mods/sodium.jar").is_file());
    assert!(outcome.snapshot.join(addon::ADDED_FILE).is_file());
    assert!(!game.join("mods/sodium.jar").exists());
    assert_eq!(read("mods/sodium-fabric.jar"), SODIUM_SERVER);
    assert_eq!(read("mods/menu.jar"), b"my menu build");
    let pack = AddedPacks::load(&dirs.instance)
        .get(&pack.id)
        .cloned()
        .unwrap();
    assert_eq!(
        pack.record.files.keys().collect::<Vec<_>>(),
        ["config/sodium.toml"]
    );

    // Removing the added pack removes its unchanged files only, and its record.
    fs::write(game.join("config/sodium.toml"), b"tuned by me").unwrap();
    let removed = addon::remove(&dirs, &pack.id).unwrap();
    assert!(removed.removed.is_empty());
    assert_eq!(removed.kept_changed, ["config/sodium.toml"]);
    assert!(!AddedPacks::path(&dirs.instance).exists());
    assert_eq!(read("mods/sodium-fabric.jar"), SODIUM_SERVER);
}

#[test]
fn removing_an_added_pack_keeps_the_users_changes() {
    let fixture = fixture();
    let api = fixture.mock.client();
    // A plain Fabric instance, with no pack of its own.
    let instance = fixture
        .store
        .create(
            "Plain",
            "1.20.1",
            Some(&launcher_core::game::loaders::LoaderChoice {
                kind: launcher_core::game::loaders::LoaderKind::Fabric,
                version: "0.16.14".into(),
            }),
        )
        .unwrap();
    let dirs = InstanceDirs::new(fixture.store.dir(&instance.id));
    let path = optimization_pack(&fixture);
    let pack = modpack::read_pack(&path).unwrap();
    assert_eq!(addon::incompatibility(&instance, &pack), None);
    let plan = addon::plan(&dirs, &pack, record("Optimized", "OPT"));
    assert_eq!(
        plan.added,
        [
            "mods/sodium.jar",
            "mods/lithium-fabric.jar",
            "mods/iris.jar",
            "mods/menu.jar"
        ]
    );
    assert!(plan.kept_instance_pack.is_empty());
    let added = block_on(addon::apply(
        Platforms::new(Some(&api), None),
        &downloader(),
        &dirs,
        &plan,
        &path,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let game = &dirs.game;
    // The user disables one mod and edits another, and a config.
    fs::rename(
        game.join("mods/iris.jar"),
        game.join("mods/iris.jar.disabled"),
    )
    .unwrap();
    fs::write(game.join("mods/menu.jar"), b"patched").unwrap();
    fs::write(game.join("options.txt"), b"fov:70\n").unwrap();

    let outcome = addon::remove(&dirs, &added.id).unwrap();
    assert_eq!(
        outcome.removed,
        [
            "config/server.toml",
            "config/sodium.toml",
            "mods/iris.jar.disabled",
            "mods/lithium-fabric.jar",
            "mods/sodium.jar"
        ]
    );
    assert_eq!(outcome.kept_changed, ["mods/menu.jar", "options.txt"]);
    assert_eq!(fs::read(game.join("mods/menu.jar")).unwrap(), b"patched");
    let provenance = Provenance::load(&dirs.instance);
    assert!(provenance.get(ContentKind::Mod, "sodium.jar").is_none());
    assert!(provenance.get(ContentKind::Mod, "iris.jar").is_none());
    assert!(AddedPacks::load(&dirs.instance).packs.is_empty());
    // An unknown id changes nothing.
    assert_eq!(addon::remove(&dirs, "nope").unwrap(), Default::default());
}
