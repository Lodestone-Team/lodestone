//! Modpacks against a loopback Modrinth mock (API, CDN and the other allowed hosts):
//! importing with client/server filtering and both override folders, refusing unsafe
//! packs, leaving nothing behind on a failure, and updating while keeping the user's files.

mod modrinth;

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use futures::executor::block_on;
use launcher_core::{
    content::{ContentKind, InstanceDirs, install::InstallProgress, provenance::Provenance},
    export,
    game::download::{CancelToken, Downloader, recording_sleep},
    game::loaders::{LoaderChoice, LoaderKind},
    http,
    instances::{InstanceStore, Loader},
    modpack::{
        self, ModpackError, ModpackRecord, PackInfo,
        import::{self, ImportOptions},
        update,
    },
    worlds,
};
use modrinth::{FakeVersion, ModrinthMock};
use serde_json::{Value, json};
use sha1::Digest as _;

const SODIUM: &str = "AANobbMI";

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1)
}

fn sha1(bytes: &[u8]) -> String {
    format!("{:x}", sha1::Sha1::digest(bytes))
}

fn sha512(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha512::digest(bytes))
}

/// An index entry for `bytes` at `path`, downloaded from `url`.
fn file(path: &str, url: &str, bytes: &[u8], client: &str, server: &str) -> Value {
    json!({
        "path": path,
        "hashes": { "sha1": sha1(bytes), "sha512": sha512(bytes) },
        "env": { "client": client, "server": server },
        "downloads": [url],
        "fileSize": bytes.len(),
    })
}

fn write_pack(path: &Path, index: Value, entries: &[(&str, &[u8])]) {
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

fn index(name: &str, version: &str, files: Vec<Value>) -> Value {
    json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": version,
        "name": name,
        "summary": "A test pack",
        "files": files,
        "dependencies": { "minecraft": "1.20.1", "fabric-loader": "0.16.14" },
    })
}

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        200,
        100,
        image::Rgba([1, 2, 3, 255]),
    ))
    .write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    bytes
}

/// Every entry of `dir`, recursively, relative and sorted.
fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            // with `/` between the parts on every system
            let parts: Vec<_> = path
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
            if path.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
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

fn import_pack(
    fixture: &Fixture,
    pack_path: &Path,
    record: ModpackRecord,
    cancel: &CancelToken,
) -> Result<launcher_core::instances::Instance, ModpackError> {
    let pack = modpack::read_pack(pack_path)?;
    let api = fixture.mock.client();
    let loader = pack.loader.as_ref().map(|(kind, version)| {
        modpack::loader_choice(*kind, version, &pack.game_version, None).unwrap()
    });
    block_on(import::import(
        &fixture.store,
        Some(&api),
        &downloader(),
        pack_path,
        &pack,
        ImportOptions {
            name: pack.name.clone(),
            loader,
            record,
            icon: Some(png()),
        },
        &InstallProgress::default(),
        cancel,
    ))
}

const SODIUM_JAR: &[u8] = b"sodium 0.5.13 jar";
const GITHUB_JAR: &[u8] = b"a mod from github";
const SERVER_JAR: &[u8] = b"server only";
const PACK_ZIP: &[u8] = b"resource pack zip";

/// A Fabric pack with a Modrinth mod, a GitHub mod, a server-only mod, a resource pack
/// on the CDN that Modrinth doesn't know by hash, and overrides of every kind.
fn sample_pack(fixture: &Fixture) -> PathBuf {
    let sodium = FakeVersion::new(
        "sod0513",
        SODIUM,
        "0.5.13",
        &["fabric"],
        &["1.20.1"],
        "2024-10-01T00:00:00Z",
    )
    .file("sodium.jar", SODIUM_JAR);
    let sodium_url = sodium.cdn_url();
    fixture
        .mock
        .project(SODIUM, "sodium", "mod")
        .version(sodium)
        .serve(
            "https://github.com/x/y/releases/download/1/gh.jar",
            GITHUB_JAR,
        )
        .serve(
            "https://cdn.modrinth.com/data/PACKPROJ/versions/v1/pack.zip",
            PACK_ZIP,
        );
    let path = fixture.packs.join("sample.mrpack");
    write_pack(
        &path,
        index(
            "Sample Pack",
            "1.0.0",
            vec![
                file(
                    "mods/sodium.jar",
                    &sodium_url,
                    SODIUM_JAR,
                    "required",
                    "required",
                ),
                file(
                    "mods/gh.jar",
                    "https://github.com/x/y/releases/download/1/gh.jar",
                    GITHUB_JAR,
                    "optional",
                    "unsupported",
                ),
                file(
                    "mods/server.jar",
                    "https://cdn.modrinth.com/data/SRV/versions/s/server.jar",
                    SERVER_JAR,
                    "unsupported",
                    "required",
                ),
                file(
                    "resourcepacks/pack.zip",
                    "https://cdn.modrinth.com/data/PACKPROJ/versions/v1/pack.zip",
                    PACK_ZIP,
                    "required",
                    "unsupported",
                ),
            ],
        ),
        &[
            ("overrides/config/shared.toml", b"from overrides"),
            ("overrides/config/only-common.toml", b"common"),
            ("overrides/options.txt", b"lang:en_us\n"),
            ("overrides/.lodestone-mine", b"no"),
            (
                "client-overrides/config/shared.toml",
                b"from client-overrides",
            ),
            ("server-overrides/config/server.toml", b"server"),
        ],
    );
    path
}

#[test]
fn imports_a_pack_for_the_client() {
    let fixture = fixture();
    let path = sample_pack(&fixture);
    let pack = modpack::read_pack(&path).unwrap();
    assert_eq!(pack.name, "Sample Pack");
    assert_eq!(pack.game_version, "1.20.1");
    assert_eq!(pack.loader, Some((LoaderKind::Fabric, "0.16.14".into())));
    assert_eq!(pack.downloads.len(), 3, "the server-only mod is left out");
    assert!(pack.skipped.iter().any(|s| s.path == ".lodestone-mine"));

    let record = ModpackRecord {
        project_id: "PACKPROJ".into(),
        version_id: "v1".into(),
        title: "Sample Pack".into(),
        version_number: "1.0.0".into(),
        ..ModpackRecord::default()
    };
    let instance = import_pack(&fixture, &path, record, &CancelToken::new()).unwrap();
    assert_eq!(instance.name, "Sample Pack");
    assert_eq!(instance.game_version, "1.20.1");
    assert_eq!(instance.loader, Loader::Fabric);
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.14"));
    assert_eq!(instance.icon.as_deref(), Some("custom"));
    let listed = fixture.store.list().0;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, instance.id);

    let dir = fixture.store.dir(&instance.id);
    let game = dir.join("minecraft");
    assert_eq!(fs::read(game.join("mods/sodium.jar")).unwrap(), SODIUM_JAR);
    assert_eq!(fs::read(game.join("mods/gh.jar")).unwrap(), GITHUB_JAR);
    assert!(!game.join("mods/server.jar").exists());
    assert_eq!(
        fs::read_to_string(game.join("config/shared.toml")).unwrap(),
        "from client-overrides"
    );
    assert!(game.join("config/only-common.toml").is_file());
    assert!(!game.join("config/server.toml").exists());
    assert!(!game.join(".lodestone-mine").exists());
    assert!(image::open(dir.join("icon.png")).unwrap().width() == 128);
    // Nothing was fetched from the server-only mod's URL.
    assert!(
        fixture
            .mock
            .requests()
            .iter()
            .all(|r| !r.path.contains("server.jar"))
    );
    // No staging folder is left.
    assert!(fs::read_dir(fixture.store.root()).unwrap().count() == 1);

    // The Content tab knows the files, and the export lists the Modrinth ones by URL.
    let provenance = Provenance::load(&dir);
    let sodium = provenance.get(ContentKind::Mod, "sodium.jar").unwrap();
    let source = sodium.modrinth.as_ref().unwrap();
    assert_eq!(source.project_id, SODIUM);
    assert_eq!(source.title, "sodium title");
    let pack_zip = provenance
        .get(ContentKind::ResourcePack, "pack.zip")
        .unwrap();
    assert_eq!(
        pack_zip.modrinth.as_ref().unwrap().url.as_deref(),
        Some("https://cdn.modrinth.com/data/PACKPROJ/versions/v1/pack.zip")
    );
    let gh = provenance.get(ContentKind::Mod, "gh.jar").unwrap();
    assert!(gh.modrinth.is_none() && gh.looked_up);
    let plan = export::plan(&instance, &game, &export::modrinth_files(&dir)).unwrap();
    let listed: Vec<&str> = plan.index.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(listed, ["mods/sodium.jar", "resourcepacks/pack.zip"]);
    assert!(
        plan.overrides
            .iter()
            .any(|(p, _)| p == "overrides/mods/gh.jar")
    );

    let record = ModpackRecord::load(&dir).unwrap();
    assert_eq!(record.version_id, "v1");
    let paths: Vec<&str> = record.files.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "config/only-common.toml",
            "config/shared.toml",
            "mods/gh.jar",
            "mods/sodium.jar",
            "options.txt",
            "resourcepacks/pack.zip"
        ]
    );
    assert_eq!(
        record.files["config/shared.toml"].sha1,
        sha1(b"from client-overrides")
    );
    assert_eq!(
        record.files["mods/sodium.jar"].project.as_deref(),
        Some(SODIUM)
    );
}

fn assert_refused(index: Value, entries: &[(&str, &[u8])], expected: &str) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.mrpack");
    write_pack(&path, index, entries);
    let err = modpack::read_pack(&path).unwrap_err().to_string();
    assert!(
        err.contains(expected),
        "{err:?} should contain {expected:?}"
    );
}

#[test]
fn refuses_unsafe_and_unsupported_packs() {
    let ok = |path: &str, url: &str| file(path, url, b"x", "required", "required");
    let cdn = "https://cdn.modrinth.com/data/a/versions/b/x.jar";
    for bad_path in [
        "../escape.jar",
        "mods/../../escape.jar",
        "/etc/passwd",
        "C:/Windows/x.jar",
        "mods\\x.jar",
        "mods/con.jar",
    ] {
        assert_refused(index("p", "1", vec![ok(bad_path, cdn)]), &[], "unsafe");
    }
    for bad_url in [
        "https://evil.example/x.jar",
        "http://cdn.modrinth.com/data/a/versions/b/x.jar",
        "https://cdn.modrinth.com:8443/x.jar",
        "https://user@github.com/x.jar",
        "https://cdn.modrinth.com.evil.example/x.jar",
        "file:///etc/passwd",
    ] {
        assert_refused(
            index("p", "1", vec![ok("mods/x.jar", bad_url)]),
            &[],
            "allowed host",
        );
    }
    let mut no_hash = ok("mods/x.jar", cdn);
    no_hash["hashes"] = json!({ "sha1": sha1(b"x") });
    assert_refused(index("p", "1", vec![no_hash]), &[], "sha512");
    for bad_override in [
        "overrides/../escape.txt",
        "client-overrides//abs",
        "overrides/a/./b",
    ] {
        assert_refused(index("p", "1", vec![]), &[(bad_override, b"x")], "unsafe");
    }
    let mut two = index("p", "1", vec![]);
    two["dependencies"]["forge"] = json!("47.4.28");
    assert_refused(two, &[], "two loaders");
    let mut other = index("p", "1", vec![]);
    other["dependencies"]["liteloader"] = json!("1");
    assert_refused(other, &[], "liteloader");
    let mut format = index("p", "1", vec![]);
    format["formatVersion"] = json!(2);
    assert_refused(format, &[], "format version 2");
    let mut no_game = index("p", "1", vec![]);
    no_game["dependencies"] = json!({ "fabric-loader": "0.16.14" });
    assert_refused(no_game, &[], "Minecraft version");

    let temp = tempfile::tempdir().unwrap();
    let not_zip = temp.path().join("x.mrpack");
    fs::write(&not_zip, b"not a zip").unwrap();
    assert!(modpack::read_pack(&not_zip).is_err());
}

#[test]
fn symbolic_links_in_overrides_are_skipped() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("links.mrpack");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("modrinth.index.json", options).unwrap();
    zip.write_all(index("p", "1", vec![]).to_string().as_bytes())
        .unwrap();
    zip.add_symlink("overrides/config", "../../..", options)
        .unwrap();
    zip.add_symlink("overrides/options.txt", "/etc/passwd", options)
        .unwrap();
    zip.start_file("overrides/config.toml", options).unwrap();
    zip.write_all(b"ok").unwrap();
    zip.finish().unwrap();

    let pack = modpack::read_pack(&path).unwrap();
    let written: Vec<&str> = pack.overrides.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(written, ["config.toml"]);
    for link in ["config", "options.txt"] {
        assert!(
            pack.skipped
                .iter()
                .any(|s| s.path == link && s.reason.contains("symbolic link")),
            "{link}: {:?}",
            pack.skipped
        );
    }
}

#[test]
fn a_failed_or_cancelled_import_leaves_nothing() {
    let fixture = fixture();
    // The CDN sends other bytes than the hashes say.
    fixture.mock.serve(
        "https://cdn.modrinth.com/data/a/versions/b/x.jar",
        b"tampered",
    );
    fixture.mock.serve("https://github.com/ok.jar", b"fine");
    let damaged = fixture.packs.join("damaged.mrpack");
    write_pack(
        &damaged,
        index(
            "Damaged",
            "1",
            vec![
                file(
                    "mods/ok.jar",
                    "https://github.com/ok.jar",
                    b"fine",
                    "required",
                    "required",
                ),
                file(
                    "mods/x.jar",
                    "https://cdn.modrinth.com/data/a/versions/b/x.jar",
                    b"original",
                    "required",
                    "required",
                ),
            ],
        ),
        &[("overrides/config/a.toml", b"a")],
    );
    let err = import_pack(
        &fixture,
        &damaged,
        ModpackRecord::default(),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("mods/x.jar"), "{err}");
    assert_eq!(tree(fixture.store.root()), Vec::<String>::new());

    // Right SHA-1, wrong SHA-512.
    let mut entry = file(
        "mods/ok.jar",
        "https://github.com/ok.jar",
        b"fine",
        "required",
        "required",
    );
    entry["hashes"]["sha512"] = json!("0".repeat(128));
    let wrong_sha512 = fixture.packs.join("sha512.mrpack");
    write_pack(&wrong_sha512, index("Wrong", "1", vec![entry]), &[]);
    let err = import_pack(
        &fixture,
        &wrong_sha512,
        ModpackRecord::default(),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("SHA-512"), "{err}");
    assert_eq!(tree(fixture.store.root()), Vec::<String>::new());

    let good = sample_pack(&fixture);
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = import_pack(&fixture, &good, ModpackRecord::default(), &cancel).unwrap_err();
    assert!(matches!(err, ModpackError::Cancelled), "{err}");
    assert_eq!(tree(fixture.store.root()), Vec::<String>::new());
}

#[test]
fn forge_versions_become_maven_versions() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("forge.mrpack");
    let mut forge = index("Forge pack", "1", vec![]);
    forge["dependencies"] = json!({ "minecraft": "1.20.1", "forge": "47.4.28" });
    write_pack(&path, forge, &[]);
    let pack = modpack::read_pack(&path).unwrap();
    let (kind, version) = pack.loader.clone().unwrap();
    assert_eq!(
        modpack::loader_choice(kind, &version, &pack.game_version, None).unwrap(),
        LoaderChoice {
            kind: LoaderKind::Forge,
            version: "1.20.1-47.4.28".into()
        }
    );
}

/// Version 1 and 2 of a pack: `a` and `b` get new versions (new file names), `c` is
/// dropped and `d` added; `x.toml` and `y.toml` change and `z.toml` is new.
struct Versions {
    v1: PathBuf,
    v2: PathBuf,
}

fn versions(fixture: &Fixture) -> Versions {
    let url = |project: &str, version: &str, name: &str| {
        format!("https://cdn.modrinth.com/data/{project}/versions/{version}/{name}")
    };
    let mods = [
        ("AAAA", "a1", "a-1.jar", b"a one".as_slice()),
        ("AAAA", "a2", "a-2.jar", b"a two"),
        ("BBBB", "b1", "b-1.jar", b"b one"),
        ("BBBB", "b2", "b-2.jar", b"b two"),
        ("CCCC", "c1", "c.jar", b"c one"),
        ("DDDD", "d1", "d.jar", b"d one"),
        ("EEEE", "e1", "e.jar", b"e one"),
    ];
    for (project, version, name, bytes) in mods {
        fixture.mock.serve(&url(project, version, name), bytes);
    }
    let entry = |i: usize| {
        let (project, version, name, bytes) = mods[i];
        file(
            &format!("mods/{name}"),
            &url(project, version, name),
            bytes,
            "required",
            "required",
        )
    };
    let v1 = fixture.packs.join("v1.mrpack");
    write_pack(
        &v1,
        index(
            "Updatable",
            "1.0",
            vec![entry(0), entry(2), entry(4), entry(6)],
        ),
        &[
            ("overrides/config/x.toml", b"x v1"),
            ("overrides/config/y.toml", b"y v1"),
            ("overrides/mods/bundled.jar", b"bundled v1"),
        ],
    );
    let v2 = fixture.packs.join("v2.mrpack");
    write_pack(
        &v2,
        index(
            "Updatable",
            "2.0",
            vec![entry(1), entry(3), entry(5), entry(6)],
        ),
        &[
            ("overrides/config/x.toml", b"x v2"),
            ("overrides/config/y.toml", b"y v2"),
            ("overrides/config/z.toml", b"z v2"),
            ("overrides/config/mine.toml", b"pack's version"),
        ],
    );
    Versions { v1, v2 }
}

#[test]
fn an_update_keeps_the_users_files() {
    let fixture = fixture();
    let versions = versions(&fixture);
    let record = |version: &str| ModpackRecord {
        project_id: "PACK".into(),
        version_id: version.into(),
        title: "Updatable".into(),
        version_number: version.into(),
        ..ModpackRecord::default()
    };
    let instance = import_pack(&fixture, &versions.v1, record("v1"), &CancelToken::new()).unwrap();
    let dirs = InstanceDirs::new(fixture.store.dir(&instance.id));
    let game = &dirs.game;
    // What the user did since.
    fs::write(game.join("mods/mine.jar"), b"my own mod").unwrap();
    fs::rename(
        game.join("mods/b-1.jar"),
        game.join("mods/b-1.jar.disabled"),
    )
    .unwrap();
    fs::write(game.join("config/x.toml"), b"x as I like it").unwrap();
    fs::write(game.join("config/mine.toml"), b"my config").unwrap();
    fs::write(game.join("mods/e.jar"), b"e patched by me").unwrap();
    fs::write(game.join("options.txt"), b"fov:90\n").unwrap();
    fs::create_dir_all(game.join("saves/World")).unwrap();
    fs::write(
        game.join("saves/World/level.dat"),
        include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat"),
    )
    .unwrap();

    let current = ModpackRecord::load(&dirs.instance).unwrap();
    let pack: PackInfo = modpack::read_pack(&versions.v2).unwrap();
    let plan = update::plan(&dirs, &current, &pack, record("v2"), None);
    assert_eq!(plan.added, ["mods/d.jar"]);
    assert_eq!(
        plan.updated,
        [
            ("mods/a-1.jar".to_string(), "mods/a-2.jar".to_string()),
            (
                "mods/b-1.jar.disabled".to_string(),
                "mods/b-2.jar.disabled".to_string()
            ),
        ]
    );
    assert_eq!(plan.removed, ["mods/bundled.jar", "mods/c.jar"]);
    assert_eq!(plan.kept_changed, ["mods/e.jar"]);
    assert_eq!(plan.overrides_written, ["config/y.toml", "config/z.toml"]);
    assert_eq!(plan.overrides_kept, ["config/mine.toml", "config/x.toml"]);
    assert!(plan.replaced_own.is_empty());

    let api = fixture.mock.client();
    let outcome = block_on(update::apply(
        Some(&api),
        &downloader(),
        &dirs,
        &plan,
        &versions.v2,
        true,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();

    let read = |p: &str| fs::read_to_string(game.join(p)).unwrap();
    assert_eq!(read("mods/a-2.jar"), "a two");
    assert!(!game.join("mods/a-1.jar").exists());
    assert_eq!(read("mods/b-2.jar.disabled"), "b two");
    assert!(!game.join("mods/b-1.jar.disabled").exists());
    assert!(!game.join("mods/c.jar").exists());
    assert!(!game.join("mods/bundled.jar").exists());
    assert_eq!(read("mods/d.jar"), "d one");
    assert_eq!(read("mods/e.jar"), "e patched by me");
    assert_eq!(read("mods/mine.jar"), "my own mod");
    assert_eq!(read("config/x.toml"), "x as I like it");
    assert_eq!(read("config/y.toml"), "y v2");
    assert_eq!(read("config/z.toml"), "z v2");
    assert_eq!(read("config/mine.toml"), "my config");
    assert_eq!(read("options.txt"), "fov:90\n");
    assert!(game.join("saves/World/level.dat").is_file());

    // The snapshot has what was replaced or removed, and the records.
    let snapshot = tree(&outcome.snapshot);
    for expected in [
        "content.json",
        "instance.json",
        "modpack.json",
        "minecraft/mods/a-1.jar",
        "minecraft/mods/b-1.jar.disabled",
        "minecraft/mods/c.jar",
        "minecraft/config/y.toml",
    ] {
        assert!(
            snapshot.contains(&expected.to_string()),
            "{expected} in {snapshot:?}"
        );
    }
    assert!(!snapshot.contains(&"minecraft/mods/mine.jar".to_string()));
    assert_eq!(outcome.world_backups.len(), 1);
    assert_eq!(worlds::backups(&dirs, "World").len(), 1);

    let provenance = Provenance::load(&dirs.instance);
    assert!(provenance.get(ContentKind::Mod, "a-1.jar").is_none());
    assert!(provenance.get(ContentKind::Mod, "c.jar").is_none());
    assert_eq!(
        provenance
            .get(ContentKind::Mod, "b-2.jar.disabled")
            .and_then(|e| e.modrinth.as_ref())
            .map(|m| m.version_id.as_str()),
        Some("b2")
    );
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert_eq!(record.version_id, "v2");
    assert!(record.files.contains_key("config/z.toml"));
    assert!(!record.files.contains_key("mods/c.jar"));
    // No staging folder is left in the instance.
    assert!(
        fs::read_dir(&dirs.instance)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with(".lodestone"))
    );

    // Updating again to the same version changes nothing.
    let again = update::plan(&dirs, &record, &pack, record.clone(), None);
    assert!(!again.changes_files(), "{again:?}");
}

#[test]
fn an_update_that_stops_part_way_is_undone() {
    let fixture = fixture();
    let versions = versions(&fixture);
    let record = |version: &str| ModpackRecord {
        project_id: "PACK".into(),
        version_id: version.into(),
        title: "Updatable".into(),
        version_number: version.into(),
        ..ModpackRecord::default()
    };
    let instance = import_pack(&fixture, &versions.v1, record("v1"), &CancelToken::new()).unwrap();
    let dirs = InstanceDirs::new(fixture.store.dir(&instance.id));
    let game = &dirs.game;
    let current = ModpackRecord::load(&dirs.instance).unwrap();
    let pack: PackInfo = modpack::read_pack(&versions.v2).unwrap();
    let plan = update::plan(&dirs, &current, &pack, record("v2"), None);
    assert_eq!(
        plan.overrides_written,
        [
            "config/mine.toml",
            "config/x.toml",
            "config/y.toml",
            "config/z.toml"
        ]
    );
    let provenance_before = fs::read(dirs.instance.join("content.json")).unwrap();

    // The last file can't be moved in: a folder is in its place since the plan was made
    // (as when another program holds a file). Mods were removed and replaced by then.
    fs::create_dir_all(game.join("config/z.toml/inside")).unwrap();
    let api = fixture.mock.client();
    let err = block_on(update::apply(
        Some(&api),
        &downloader(),
        &dirs,
        &plan,
        &versions.v2,
        false,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(err.to_string().contains("were put back"), "{err}");

    let read = |p: &str| fs::read_to_string(game.join(p)).unwrap();
    assert_eq!(read("mods/a-1.jar"), "a one");
    assert_eq!(read("mods/b-1.jar"), "b one");
    assert_eq!(read("mods/c.jar"), "c one");
    assert_eq!(read("mods/bundled.jar"), "bundled v1");
    assert_eq!(read("config/x.toml"), "x v1");
    assert_eq!(read("config/y.toml"), "y v1");
    for added in [
        "mods/a-2.jar",
        "mods/b-2.jar",
        "mods/d.jar",
        "config/mine.toml",
    ] {
        assert!(!game.join(added).exists(), "{added} is left");
    }
    assert!(game.join("config/z.toml/inside").is_dir());
    assert_eq!(
        ModpackRecord::load(&dirs.instance).unwrap().version_id,
        "v1"
    );
    assert_eq!(
        fs::read(dirs.instance.join("content.json")).unwrap(),
        provenance_before
    );
}

#[test]
fn downloads_a_versions_pack_from_the_cdn_only() {
    let fixture = fixture();
    let source = sample_pack(&fixture);
    let bytes = fs::read(&source).unwrap();
    let good = FakeVersion::new("pv1", "PACK", "1.0", &["fabric"], &["1.20.1"], "2025-01-01")
        .file("Sample.mrpack", &bytes);
    let mut damaged =
        FakeVersion::new("pv2", "PACK", "1.1", &["fabric"], &["1.20.1"], "2025-02-01")
            .file("Sample.mrpack", &bytes);
    damaged.wrong_sha512 = true;
    let mut elsewhere =
        FakeVersion::new("pv3", "PACK", "1.2", &["fabric"], &["1.20.1"], "2025-03-01")
            .file("Sample.mrpack", &bytes);
    elsewhere.url_override = Some("https://github.com/x/Sample.mrpack".into());
    fixture
        .mock
        .project("PACK", "sample", "modpack")
        .version(good)
        .version(damaged)
        .version(elsewhere);
    let api = fixture.mock.client();
    let download = |id: &str| {
        let version = block_on(api.version(id)).unwrap();
        block_on(import::download_pack(
            &api,
            &downloader(),
            &version,
            &fixture.packs.join("downloads"),
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
    };
    let path = download("pv1").unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(modpack::read_pack(&path).is_ok());
    fs::remove_file(&path).unwrap();
    assert!(download("pv2").unwrap_err().to_string().contains("SHA-512"));
    assert!(
        download("pv3")
            .unwrap_err()
            .to_string()
            .contains("cdn.modrinth.com")
    );
    assert_eq!(tree(&fixture.packs.join("downloads")), Vec::<String>::new());
}

/// Makes an instance from version `pv1` of the modpack `PACK1`, the way a share code's
/// recipient does: the pack checked and downloaded from Modrinth, then imported.
fn import_shared_pack(
    fixture: &Fixture,
    pack: &launcher_core::content::share::SharedPack,
    name: &str,
) -> InstanceDirs {
    let api = fixture.mock.client();
    let (project, version) =
        block_on(launcher_core::content::share::pack_version(&api, pack)).unwrap();
    let path = block_on(import::download_pack(
        &api,
        &downloader(),
        &version,
        &fixture.packs.join("downloads"),
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let info = modpack::read_pack(&path).unwrap();
    let loader = info.loader.as_ref().map(|(kind, version)| {
        modpack::loader_choice(*kind, version, &info.game_version, None).unwrap()
    });
    let instance = block_on(import::import(
        &fixture.store,
        Some(&api),
        &downloader(),
        &path,
        &info,
        ImportOptions {
            name: name.into(),
            loader,
            record: ModpackRecord::from_modrinth(&project, &version),
            icon: None,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    fs::remove_file(&path).unwrap();
    InstanceDirs::new(fixture.store.dir(&instance.id))
}

#[test]
fn a_share_code_based_on_the_pack_recreates_the_instance() {
    use launcher_core::content::{
        Target, files,
        install::{self as content_install, InstallRequest},
        options::PackOptions,
        packs::Compatibility,
        share::{self, LeftOutReason, ShareCode, SharedPack},
    };
    let fixture = fixture();
    // A pack with Sodium from Modrinth, and bundled mods and resource packs (a folder
    // and a zip, the zip turned on by the pack's options.txt).
    let sodium = FakeVersion::new(
        "sod0513",
        SODIUM,
        "0.5.13",
        &["fabric"],
        &["1.20.1"],
        "2024-10-01T00:00:00Z",
    )
    .file("sodium.jar", SODIUM_JAR);
    let sodium_url = sodium.cdn_url();
    let pack_path = fixture.packs.join("source.mrpack");
    write_pack(
        &pack_path,
        index(
            "Test Pack",
            "1.0",
            vec![file(
                "mods/sodium.jar",
                &sodium_url,
                SODIUM_JAR,
                "required",
                "required",
            )],
        ),
        &[
            ("overrides/mods/bundled.jar", b"bundled mod"),
            ("overrides/mods/tweaked.jar", b"bundled, to be changed"),
            ("overrides/mods/gone.jar", b"bundled, to be removed"),
            (
                "overrides/resourcepacks/Folder Pack/pack.mcmeta",
                b"{\"pack\":{\"pack_format\":15,\"description\":\"x\"}}",
            ),
            ("overrides/resourcepacks/Folder Pack/assets/a.txt", b"a"),
            ("overrides/resourcepacks/Zipped.zip", b"zipped pack"),
            (
                "overrides/options.txt",
                b"resourcePacks:[\"vanilla\",\"file/Zipped.zip\"]\n",
            ),
            ("overrides/config/x.toml", b"config"),
        ],
    );
    let pack_bytes = fs::read(&pack_path).unwrap();
    fixture
        .mock
        .project(SODIUM, "sodium", "mod")
        .project("EXTRAMOD", "extra", "mod")
        .project("FAITHFUL", "faithful", "resourcepack")
        .project("PACK1", "testpack", "modpack")
        .version(sodium)
        .version(
            FakeVersion::new(
                "ext100",
                "EXTRAMOD",
                "1.0.0",
                &["fabric"],
                &["1.20.1"],
                "2024-10-01T00:00:00Z",
            )
            .file("extra.jar", b"extra mod"),
        )
        .version(
            FakeVersion::new(
                "fai100",
                "FAITHFUL",
                "1.0",
                &["minecraft"],
                &["1.20.1"],
                "2024-10-01T00:00:00Z",
            )
            .file("Faithful.zip", b"faithful pack"),
        )
        .version(
            FakeVersion::new(
                "pv1",
                "PACK1",
                "1.0",
                &["fabric"],
                &["1.20.1"],
                "2025-01-01T00:00:00Z",
            )
            .file("Test.mrpack", &pack_bytes),
        );
    let api = fixture.mock.client();
    let target = Target::new("1.20.1", Loader::Fabric);
    let shared = SharedPack {
        project: "PACK1".into(),
        version: "pv1".into(),
        title: String::new(),
        version_number: String::new(),
    };

    // The instance to share: Sodium turned off, a bundled mod removed and another
    // changed, a mod added by hand, a Modrinth mod and resource pack added, and the
    // folder pack and the added pack turned on above the pack's zip.
    let from = import_shared_pack(&fixture, &shared, "Mine");
    let mods = from.content_dir(ContentKind::Mod);
    files::set_mod_enabled(&from, "sodium.jar", false).unwrap();
    fs::remove_file(mods.join("gone.jar")).unwrap();
    fs::write(mods.join("tweaked.jar"), b"changed by the user").unwrap();
    fs::write(mods.join("homemade.jar"), b"by hand").unwrap();
    for (kind, id) in [
        (ContentKind::Mod, "ext100"),
        (ContentKind::ResourcePack, "fai100"),
    ] {
        block_on(async {
            let version = api.version(id).await.unwrap();
            let plan = content_install::plan(
                &api,
                &from,
                &target,
                InstallRequest {
                    kind,
                    version,
                    project: None,
                    install_dependencies: false,
                    force: false,
                },
            )
            .await
            .unwrap();
            content_install::execute(
                &api,
                &downloader(),
                &from,
                &target,
                plan,
                &InstallProgress::default(),
                &CancelToken::new(),
            )
            .await
            .unwrap();
        });
    }
    for name in ["Folder Pack", "Faithful.zip"] {
        files::set_pack_enabled(&from, &target, name, true, Compatibility::Compatible).unwrap();
    }
    let enabled = |dirs: &InstanceDirs| {
        PackOptions::read(&dirs.game)
            .unwrap()
            .enabled_files(&target.game_version)
    };
    assert_eq!(
        enabled(&from),
        ["Faithful.zip", "Folder Pack", "Zipped.zip"]
    );

    let plan = block_on(share::collect(
        Some(&api),
        &from,
        &target,
        Some("Mine".into()),
        Loader::Fabric,
        Some("0.16.14".into()),
    ))
    .unwrap();
    // Only what really won't arrive is listed: not the pack's unchanged files.
    let mut left_out: Vec<(&str, LeftOutReason)> = plan
        .left_out
        .iter()
        .map(|l| (l.file_name.as_str(), l.reason))
        .collect();
    left_out.sort_by_key(|(name, _)| *name);
    assert_eq!(
        left_out,
        [
            ("homemade.jar", LeftOutReason::AddedByHand),
            ("tweaked.jar", LeftOutReason::ChangedSincePack),
        ]
    );
    let code = &plan.code;
    assert_eq!(code.pack.as_ref().unwrap().project, "PACK1");
    assert_eq!(code.pack.as_ref().unwrap().version, "pv1");
    assert_eq!(code.pack.as_ref().unwrap().title, "testpack title");
    assert_eq!(code.items.len(), 2);
    assert_eq!(code.changes.removed, ["mods/gone.jar"]);
    assert_eq!(code.changes.turned_off, ["mods/sodium.jar"]);
    let counts = plan.changes.clone().unwrap();
    assert_eq!(
        (counts.added, counts.removed, counts.changed, counts.toggled),
        (2, 1, 0, 1)
    );
    // sodium, bundled, extra (tweaked arrives as the pack's); both zips and the folder
    assert_eq!(plan.shared(ContentKind::Mod), 3);
    assert_eq!(plan.shared(ContentKind::ResourcePack), 3);
    let text = code.encode();
    assert!(text.starts_with(share::PACK_PREFIX), "{text}");
    assert!(text.len() < 400, "{} chars", text.len());
    let decoded = ShareCode::decode(&text).unwrap();
    assert_eq!(&decoded, code);

    // The recipient: the pack, then the changes.
    let to = import_shared_pack(&fixture, decoded.pack.as_ref().unwrap(), "Theirs");
    let outcome = block_on(share::install(
        &api,
        &downloader(),
        &to,
        &target,
        &decoded,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    assert_eq!(outcome.installed.len(), 2);
    let mut expected = tree(&from.game);
    expected.retain(|p| {
        (p.starts_with("mods") || p.starts_with("resourcepacks")) && !p.ends_with("homemade.jar")
    });
    let mut got = tree(&to.game);
    got.retain(|p| p.starts_with("mods") || p.starts_with("resourcepacks"));
    assert_eq!(got, expected);
    assert!(
        to.content_dir(ContentKind::Mod)
            .join("sodium.jar.disabled")
            .is_file()
    );
    assert_eq!(
        fs::read(to.content_dir(ContentKind::Mod).join("tweaked.jar")).unwrap(),
        b"bundled, to be changed"
    );
    assert_eq!(enabled(&to), enabled(&from));

    // A code for another pack isn't applied to this instance.
    let mut other = decoded.clone();
    other.pack.as_mut().unwrap().version = "pv9".into();
    assert!(
        block_on(share::install(
            &api,
            &downloader(),
            &to,
            &target,
            &other,
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
        .is_err()
    );
}
