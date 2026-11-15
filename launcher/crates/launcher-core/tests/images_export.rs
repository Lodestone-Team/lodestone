//! Image exports of snapshots: what travels, what stays home, and importing one as a new
//! instance against a loopback Modrinth mock (never the network), including hostile files.

mod modrinth;

use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::executor::block_on;
use launcher_core::content::provenance::{self, ModrinthSource, Provenance};
use launcher_core::game::download::{CancelToken, Downloader, Progress, recording_sleep};
use launcher_core::game::loaders::{LoaderChoice, LoaderKind};
use launcher_core::http;
use launcher_core::images::export::{self, PRIVATE_FILES, Sources};
use launcher_core::images::restore::RestoreScope;
use launcher_core::images::{Images, SnapshotKind, Take};
use launcher_core::instances::{Instance, InstanceStore};
use launcher_core::paths::DataDir;
use lodestone_image::store::NEVER;
use modrinth::ModrinthMock;
use sha1::Digest as _;

const SODIUM: &[u8] = b"sodium jar bytes, from Modrinth";
const SODIUM_URL: &str = "https://cdn.modrinth.com/data/AANobbMI/versions/v1/sodium.jar";

struct Fixture {
    _temp: tempfile::TempDir,
    data: DataDir,
    images: Arc<Images>,
    store: InstanceStore,
    instance: Instance,
    mock: ModrinthMock,
}

impl Fixture {
    fn game(&self, relative: &str) -> PathBuf {
        self.store.game_dir(&self.instance.id).join(relative)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.game(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

fn hex_sha1(bytes: &[u8]) -> String {
    format!("{:x}", sha1::Sha1::digest(bytes))
}

fn hex_sha512(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha512::digest(bytes))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}

/// An instance with a Modrinth mod content.json knows, a config, a world and the files
/// that must never leave the computer, recorded and snapshotted.
fn fixture() -> (Fixture, launcher_core::images::SnapshotRecord) {
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path().join("data"));
    let store = InstanceStore::new(data.instances_dir());
    let instance = store
        .create(
            "Survival",
            "1.21.1",
            Some(&LoaderChoice {
                kind: LoaderKind::Fabric,
                version: "0.16.10".into(),
            }),
        )
        .unwrap();
    let f = Fixture {
        images: Images::open(&data),
        data,
        store,
        instance,
        mock: ModrinthMock::start(),
        _temp: temp,
    };
    f.write("mods/sodium.jar", SODIUM);
    f.write("config/sodium.json", b"{\"fps\":true}");
    f.write("saves/World/level.dat", b"a level");
    f.write("saves/World/region/r.0.0.mca", b"region bytes");
    f.write("command_history.txt", b"/login hunter2");
    f.write("hotbar.nbt", b"hotbars");
    f.write("config/ias.json", b"{\"accounts\":\"token\"}");
    // The official launcher's account files, as a pasted `.minecraft` folder brings them.
    f.write("launcher_accounts.json", b"{\"accessToken\":\"official\"}");
    f.write("Launcher_Profiles.json", b"{\"authenticationDatabase\":{}}");
    // A JVM crash log, which holds the whole command line, access token included.
    f.write(
        "hs_err_pid4242.log",
        b"java ... --accessToken eyJsecret ...",
    );
    let meta = fs::metadata(f.game("mods/sodium.jar")).unwrap();
    let dir = f.store.dir(&f.instance.id);
    let mut known = Provenance::default();
    known.files.insert(
        "mods/sodium.jar".into(),
        provenance::Entry {
            size: meta.len(),
            modified: meta.modified().ok().and_then(provenance::nanos),
            sha1: hex_sha1(SODIUM),
            sha512: hex_sha512(SODIUM),
            modrinth: Some(ModrinthSource {
                project_id: "AANobbMI".into(),
                version_id: "v1".into(),
                url: Some(SODIUM_URL.into()),
                ..ModrinthSource::default()
            }),
            ..provenance::Entry::default()
        },
    );
    known.save(&dir).unwrap();
    f.images
        .record_now(&f.instance.id, "Created Survival")
        .unwrap();
    let record = f
        .images
        .take_snapshot(
            &f.instance.id,
            Take::new(SnapshotKind::Manual, "Before the end"),
        )
        .unwrap()
        .unwrap();
    (f, record)
}

/// Another Launcher: a fresh data folder, with nothing in its image store.
fn elsewhere() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path().join("data"));
    let store = InstanceStore::new(data.instances_dir());
    fs::create_dir_all(store.root()).unwrap();
    Fixture {
        images: Images::open(&data),
        data,
        store,
        instance: Instance::default(),
        mock: ModrinthMock::start(),
        _temp: temp,
    }
}

fn zip_names(path: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
        .collect()
}

fn zip_read(path: &Path, name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

/// A copy of the zip at `from` at `to`, with `change` deciding each entry's new bytes
/// (`None` drops it), and `extra` entries added.
fn rewrite_zip(
    from: &Path,
    to: &Path,
    change: impl Fn(&str, Vec<u8>) -> Option<Vec<u8>>,
    extra: &[(&str, &[u8])],
) {
    let mut archive = zip::ZipArchive::new(fs::File::open(from).unwrap()).unwrap();
    let mut out = zip::ZipWriter::new(fs::File::create(to).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        if entry.is_dir() {
            out.add_directory(name, options).unwrap();
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if let Some(bytes) = change(&name, bytes) {
            out.start_file(name, options).unwrap();
            out.write_all(&bytes).unwrap();
        }
    }
    for (name, bytes) in extra {
        out.start_file(*name, options).unwrap();
        out.write_all(bytes).unwrap();
    }
    out.finish().unwrap();
}

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1)
}

fn import(
    f: &Fixture,
    path: &Path,
    room: bool,
) -> Result<export::Imported, launcher_core::images::ImageError> {
    let modrinth = f.mock.client();
    let downloader = downloader();
    let check = |bytes: u64| {
        if room {
            Ok(())
        } else {
            Err(launcher_core::images::ImageError::NoRoom {
                needed: bytes,
                free: 0,
            })
        }
    };
    block_on(export::import_file(
        &f.images,
        Sources {
            downloader: &downloader,
            modrinth: &modrinth,
            curseforge: None,
        },
        path,
        None,
        None,
        &check,
        &Progress::default(),
        &CancelToken::new(),
    ))
}

fn instance_count(f: &Fixture) -> usize {
    f.store.list().0.len()
}

#[test]
fn an_export_carries_references_not_jars_and_never_private_files() {
    let (f, record) = fixture();
    let file = f
        .data
        .root()
        .join("out")
        .join(format!("Survival.{}", export::EXTENSION));
    let report = f
        .images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    assert!(file.is_file());
    assert!(!f.data.root().join("out/Survival.lodeimg.part").exists());
    assert!(report.references >= 1);
    let mut left_out = report.left_out.clone();
    left_out.sort();
    assert_eq!(
        left_out,
        [
            "Launcher_Profiles.json",
            "command_history.txt",
            "config/ias.json",
            "hotbar.nbt",
            "launcher_accounts.json"
        ]
    );
    let names = zip_names(&file);
    let objects: Vec<&String> = names.iter().filter(|n| n.starts_with("objects/")).collect();
    assert!(
        !names.contains(&format!("objects/{}", hex_sha256(SODIUM))),
        "the Modrinth jar travels as a reference"
    );
    for private in [
        &b"/login hunter2"[..],
        b"hotbars",
        b"{\"accounts\":\"token\"}",
        b"java ... --accessToken eyJsecret ...",
        b"{\"accessToken\":\"official\"}",
        b"{\"authenticationDatabase\":{}}",
    ] {
        assert!(!names.contains(&format!("objects/{}", hex_sha256(private))));
    }
    assert!(names.contains(&format!("objects/{}", hex_sha256(b"region bytes"))));
    assert!(!objects.is_empty());
    let image = lodestone_image::Image::from_json(&zip_read(&file, "image.json")).unwrap();
    assert!(
        !image
            .entries
            .iter()
            .any(|e| e.path.as_str().starts_with("hs_err")),
        "no snapshot takes the JVM's crash log"
    );
    for pattern in PRIVATE_FILES {
        assert!(image.excludes.iter().any(|e| e == pattern), "{pattern}");
    }
    assert_eq!(image.label.as_deref(), Some("Before the end"));
    // The preview reads it without unpacking.
    let preview = export::inspect(&file).unwrap();
    assert_eq!(preview.minecraft, "1.21.1");
    assert_eq!(preview.loader, "fabric");
    assert_eq!(preview.mods, ["sodium.jar"]);
    assert!(
        preview.carried_mods.is_empty(),
        "Sodium is a Modrinth download"
    );
    assert!(preview.needed_bytes > preview.files_bytes + preview.download_bytes);
    assert_eq!(preview.worlds, ["World"]);
    assert_eq!(preview.download_bytes, SODIUM.len() as u64);
    assert_eq!(preview.name_shown, "Before the end");
}

#[test]
fn an_import_makes_a_new_instance_and_downloads_the_references() {
    let (f, record) = fixture();
    let file = f.data.root().join("Survival.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    let other = elsewhere();
    other.mock.serve(SODIUM_URL, SODIUM);
    let before = instance_count(&other);
    let imported = import(&other, &file, true).unwrap();
    assert_eq!(instance_count(&other), before + 1);
    assert!(imported.left_out.is_empty());
    let made = &imported.instance;
    assert_eq!(made.name, "Before the end");
    assert_eq!(made.game_version, "1.21.1");
    assert_eq!(made.loader_version.as_deref(), Some("0.16.10"));
    assert_eq!(made.java_override, None);
    let game = other.store.game_dir(&made.id);
    assert_eq!(fs::read(game.join("mods/sodium.jar")).unwrap(), SODIUM);
    assert_eq!(
        fs::read(game.join("config/sodium.json")).unwrap(),
        b"{\"fps\":true}"
    );
    assert_eq!(
        fs::read(game.join("saves/World/region/r.0.0.mca")).unwrap(),
        b"region bytes"
    );
    assert!(!game.join("command_history.txt").exists());
    assert!(!game.join("hotbar.nbt").exists());
    assert!(!game.join("config/ias.json").exists());
    assert!(!game.join("launcher_accounts.json").exists());
    // content.json knows the Modrinth file.
    let known = Provenance::load(&other.store.dir(&made.id));
    assert_eq!(
        known.files["mods/sodium.jar"]
            .modrinth
            .as_ref()
            .unwrap()
            .project_id,
        "AANobbMI"
    );
    // Recorded, and the imported image is a snapshot to go back to.
    let history = other.images.history(&made.id).unwrap();
    assert_eq!(
        history.images[0].label.as_deref(),
        Some("Made from a snapshot")
    );
    assert_eq!(
        history.images.len(),
        1,
        "the sender's content image, kept for the snapshot, isn't this instance's history"
    );
    let snapshots = other.images.snapshots(&made.id).unwrap();
    assert_eq!(snapshots.records.len(), 1);
    assert_eq!(snapshots.records[0].kind, SnapshotKind::Imported);
    // The download came from the mock's CDN mirror.
    assert!(
        other
            .mock
            .requests()
            .iter()
            .any(|r| r.path.ends_with("/sodium.jar"))
    );
}

#[test]
fn a_download_that_doesnt_match_is_left_out_and_a_full_disk_is_refused() {
    let (f, record) = fixture();
    let file = f.data.root().join("Survival.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    let other = elsewhere();
    let before = instance_count(&other);
    // Bytes that aren't the ones named: Sodium is left out, the rest is imported.
    other.mock.serve(SODIUM_URL, b"something else entirely");
    let imported = import(&other, &file, true).unwrap();
    assert_eq!(imported.left_out, ["mods/sodium.jar"]);
    let game = other.store.game_dir(&imported.instance.id);
    assert!(!game.join("mods/sodium.jar").exists());
    assert!(game.join("saves/World/level.dat").is_file());
    let before = before + 1;
    assert_eq!(instance_count(&other), before);
    // No room: refused before anything is unpacked.
    other.mock.serve(SODIUM_URL, SODIUM);
    let objects = other.data.image_store_dir().join("objects");
    let count = || walk_count(&objects);
    let had = count();
    assert!(import(&other, &file, false).is_err());
    assert_eq!(count(), had);
    assert_eq!(instance_count(&other), before);
    // Nothing is left in the instances folder.
    let stray: Vec<String> = fs::read_dir(other.data.instances_dir())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".lodestone"))
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
}

fn walk_count(dir: &Path) -> usize {
    let mut count = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                stack.push(entry.path());
            } else {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn hostile_exports_are_refused_before_anything_is_placed() {
    let (f, record) = fixture();
    let good = f.data.root().join("good.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &good, NEVER)
        .unwrap();
    let other = elsewhere();
    other.mock.serve(SODIUM_URL, SODIUM);
    let before = instance_count(&other);
    let region = format!("objects/{}", hex_sha256(b"region bytes"));
    let bad = other.data.root().join("bad.lodeimg");
    // Bytes that don't match their hash (same size).
    rewrite_zip(
        &good,
        &bad,
        |name, bytes| {
            Some(if name == region {
                b"region BYTES".to_vec()
            } else {
                bytes
            })
        },
        &[],
    );
    assert!(import(&other, &bad, true).is_err());
    // An object nothing names.
    rewrite_zip(
        &good,
        &bad,
        |_, b| Some(b),
        &[(&format!("objects/{}", hex_sha256(b"x")), b"x")],
    );
    assert!(export::inspect(&bad).is_err());
    assert!(import(&other, &bad, true).is_err());
    // A name outside the layout.
    rewrite_zip(&good, &bad, |_, b| Some(b), &[("../escape.txt", b"x")]);
    assert!(import(&other, &bad, true).is_err());
    // A missing object.
    rewrite_zip(&good, &bad, |name, b| (name != region).then_some(b), &[]);
    assert!(import(&other, &bad, true).is_err());
    // Not a zip at all.
    fs::write(&bad, b"not a zip").unwrap();
    assert!(import(&other, &bad, true).is_err());
    assert_eq!(instance_count(&other), before);

    // An image naming one of Lodestone's own files.
    let store_dir = other.data.root().join("elsewhere");
    let store =
        lodestone_image::store::Store::new(&store_dir, lodestone_image::store::Encoding::Raw);
    let stored = store.put_bytes(b"sneaky").unwrap();
    let mut image = lodestone_image::Image::new(
        lodestone_image::Kind::Full,
        lodestone_image::Game {
            minecraft: "1.21.1".into(),
            loader: lodestone_image::Loader::Vanilla,
            loader_version: None,
        },
    );
    image.entries.push(lodestone_image::Entry::new(
        lodestone_image::CanonicalPath::new("config/.lodestone_marks.json").unwrap(),
        6,
        stored.sha256.clone(),
        lodestone_image::Source::Object {},
    ));
    image.seal();
    let sneaky = other.data.root().join("sneaky.lodeimg");
    lodestone_image::container::export(&store, &image, &[], &sneaky, NEVER).unwrap();
    assert!(export::inspect(&sneaky).is_err());
    assert!(import(&other, &sneaky, true).is_err());
    assert_eq!(instance_count(&other), before);
}

/// A full image of `entries` (path, bytes, source) for Minecraft 1.21.1 with `loader`,
/// exported from a store of its own to `to`.
fn hostile_export(
    dir: &Path,
    loader: lodestone_image::Loader,
    entries: &[(&str, &[u8], lodestone_image::Source, Option<String>)],
    to: &Path,
) {
    let store = lodestone_image::store::Store::new(
        dir.join("store"),
        lodestone_image::store::Encoding::Raw,
    );
    let mut image = lodestone_image::Image::new(
        lodestone_image::Kind::Full,
        lodestone_image::Game {
            minecraft: "1.21.1".into(),
            loader,
            loader_version: Some("0.16.10".into()),
        },
    );
    for (path, bytes, source, sha512) in entries {
        let stored = store.put_bytes(bytes).unwrap();
        let mut entry = lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new(*path).unwrap(),
            bytes.len() as u64,
            stored.sha256.clone(),
            source.clone(),
        );
        entry.sha512 = sha512.clone();
        image.entries.push(entry);
    }
    image.normalize();
    image.seal();
    lodestone_image::container::export(&store, &image, &[], to, NEVER).unwrap();
}

#[test]
fn a_download_whose_bytes_the_export_carries_is_downloaded_and_checked_anyway() {
    let other = elsewhere();
    other.mock.serve(SODIUM_URL, SODIUM);
    let evil: &[u8] = b"not sodium at all, but the same length?";
    let file = other.data.root().join("evil.lodeimg");
    // A "Modrinth" Sodium whose bytes are the sender's, with the SHA-512 and SHA-256 of
    // those bytes: the export carries the same bytes under another path, so the store has
    // them, and the file's own hashes agree with them.
    hostile_export(
        &other.data.root().join("sender"),
        lodestone_image::Loader::Fabric,
        &[
            (
                "mods/sodium.jar",
                evil,
                lodestone_image::Source::Modrinth {
                    project: "AANobbMI".into(),
                    version: "v1".into(),
                    url: SODIUM_URL.into(),
                },
                Some(hex_sha512(evil)),
            ),
            (
                "config/blob.bin",
                evil,
                lodestone_image::Source::Object {},
                None,
            ),
        ],
        &file,
    );
    // The preview names the carried bytes as the sender's.
    assert_eq!(export::inspect(&file).unwrap().carried_mods, ["sodium.jar"]);
    // Modrinth's file isn't those bytes, so the download fails its check: the carried bytes
    // are placed, as a plain file that nothing records as Sodium.
    let imported = import(&other, &file, true).unwrap();
    assert!(imported.left_out.is_empty());
    let game = other.store.game_dir(&imported.instance.id);
    assert_eq!(fs::read(game.join("mods/sodium.jar")).unwrap(), evil);
    let known = Provenance::load(&other.store.dir(&imported.instance.id));
    assert!(
        known
            .files
            .get("mods/sodium.jar")
            .is_none_or(|e| e.modrinth.is_none()),
        "{:?}",
        known.files.get("mods/sodium.jar")
    );
    assert!(
        other
            .mock
            .requests()
            .iter()
            .any(|r| r.path.ends_with("/sodium.jar")),
        "Sodium was downloaded from Modrinth although the store had the claimed bytes"
    );
}

#[test]
fn a_download_that_fails_leaves_the_file_out_and_the_import_goes_on() {
    let (f, record) = fixture();
    let file = f.data.root().join("Survival.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    // Modrinth answers 404: the author deleted the version.
    let other = elsewhere();
    let before = instance_count(&other);
    let imported = import(&other, &file, true).unwrap();
    assert_eq!(instance_count(&other), before + 1);
    assert_eq!(imported.left_out, ["mods/sodium.jar"]);
    let game = other.store.game_dir(&imported.instance.id);
    assert!(!game.join("mods/sodium.jar").exists());
    assert_eq!(
        fs::read(game.join("saves/World/region/r.0.0.mca")).unwrap(),
        b"region bytes"
    );
    assert_eq!(
        fs::read(game.join("config/sodium.json")).unwrap(),
        b"{\"fps\":true}"
    );
    // The snapshot of what they were sent is kept, the file it lacks withheld as not
    // downloaded: never an object the store can't back.
    let id = imported.instance.id.clone();
    let snapshots = other.images.snapshots(&id).unwrap();
    assert_eq!(snapshots.records.len(), 1);
    let kept = &snapshots.records[0];
    assert_eq!(kept.kind, SnapshotKind::Imported);
    let content = other
        .images
        .store()
        .load_manifest(&id, kept.content.as_deref().unwrap())
        .unwrap();
    assert!(
        content
            .entries
            .iter()
            .all(|e| e.path.as_str() != "mods/sodium.jar")
    );
    assert_eq!(
        content.withheld,
        [lodestone_image::Withheld {
            path: "mods/sodium.jar".into(),
            reason: lodestone_image::WithheldReason::Other(export::NOT_DOWNLOADED.into()),
        }]
    );
    // Going back to it puts the rest back and leaves the file it never had as it is.
    fs::create_dir_all(game.join("mods")).unwrap();
    fs::write(game.join("mods/sodium.jar"), b"found it myself").unwrap();
    fs::write(game.join("config/sodium.json"), b"changed").unwrap();
    other
        .images
        .restore(
            &id,
            &kept.id,
            &RestoreScope::WorldsAndContent(vec!["World".into()]),
            NEVER,
        )
        .unwrap();
    assert_eq!(
        fs::read(game.join("mods/sodium.jar")).unwrap(),
        b"found it myself"
    );
    assert_eq!(
        fs::read(game.join("config/sodium.json")).unwrap(),
        b"{\"fps\":true}"
    );
    // A store that has the bytes already places them, as a plain file.
    let third = elsewhere();
    third.images.store().put_bytes(SODIUM).unwrap();
    let imported = import(&third, &file, true).unwrap();
    assert!(imported.left_out.is_empty());
    let game = third.store.game_dir(&imported.instance.id);
    assert_eq!(fs::read(game.join("mods/sodium.jar")).unwrap(), SODIUM);
    let known = Provenance::load(&third.store.dir(&imported.instance.id));
    assert!(
        known
            .files
            .get("mods/sodium.jar")
            .is_none_or(|e| e.modrinth.is_none())
    );
}

#[test]
fn downloads_that_get_no_answer_stop_being_tried() {
    // A host that takes every connection and drops it at once, counting them: as a
    // network that blocks the CDN looks to the downloader.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = url::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let accepted = accepted.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                accepted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                drop(stream);
            }
        });
    }
    let other = elsewhere();
    let file = other.data.root().join("many.lodeimg");
    const MODS: usize = 60;
    let jars: Vec<(String, Vec<u8>, lodestone_image::Source, Option<String>)> = (0..MODS)
        .map(|n| {
            let bytes = format!("mod number {n}").into_bytes();
            let source = lodestone_image::Source::Modrinth {
                project: format!("Proj{n}"),
                version: format!("v{n}"),
                url: format!("https://cdn.modrinth.com/data/Proj{n}/versions/v{n}/m{n}.jar"),
            };
            let sha512 = hex_sha512(&bytes);
            (format!("mods/m{n}.jar"), bytes, source, Some(sha512))
        })
        .collect();
    let entries: Vec<(&str, &[u8], lodestone_image::Source, Option<String>)> = jars
        .iter()
        .map(|(path, bytes, source, sha)| {
            (path.as_str(), bytes.as_slice(), source.clone(), sha.clone())
        })
        .collect();
    hostile_export(
        &other.data.root().join("sender"),
        lodestone_image::Loader::Fabric,
        &entries,
        &file,
    );
    let modrinth = launcher_core::content::Modrinth::with_urls(
        http::default_client().unwrap(),
        base.join("v2").unwrap(),
        Some(base),
    );
    // Every try, as the Launcher makes them.
    let downloader = Downloader::new(http::download_client().unwrap(), recording_sleep().0);
    let imported = block_on(export::import_file(
        &other.images,
        Sources {
            downloader: &downloader,
            modrinth: &modrinth,
            curseforge: None,
        },
        &file,
        None,
        None,
        &|_| Ok(()),
        &Progress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(imported.left_out.len(), MODS, "nothing could be had");
    // A round of downloads that got no answer (and those already under way), each with its
    // tries; never every file's.
    let tries = accepted.load(std::sync::atomic::Ordering::SeqCst);
    let attempts = launcher_core::game::download::DEFAULT_ATTEMPTS as usize;
    assert!(tries > 0);
    assert!(
        tries <= 2 * 8 * attempts,
        "{tries} connections for {MODS} files"
    );
    // The snapshot of what they were sent is kept, every file withheld.
    let snapshots = other.images.snapshots(&imported.instance.id).unwrap();
    assert_eq!(snapshots.records.len(), 1);
}

#[test]
fn a_file_of_another_project_never_passes_for_the_one_named() {
    let other = elsewhere();
    let evil: &[u8] = b"another project's jar";
    let evil_url = "https://cdn.modrinth.com/data/EVILPROJ/versions/x1/evil.jar";
    other.mock.serve(evil_url, evil);
    let file = other.data.root().join("spoof.lodeimg");
    // Sodium's project and version, with another project's URL and that file's hashes; the
    // bytes are carried too, under another path. The label tries to show as another name.
    hostile_export(
        &other.data.root().join("sender"),
        lodestone_image::Loader::Fabric,
        &[
            (
                "mods/sodium.jar",
                evil,
                lodestone_image::Source::Modrinth {
                    project: "AANobbMI".into(),
                    version: "v1".into(),
                    url: evil_url.into(),
                },
                Some(hex_sha512(evil)),
            ),
            (
                "config/blob.bin",
                evil,
                lodestone_image::Source::Object {},
                None,
            ),
            (
                "mods/other.jar",
                b"a curseforge jar",
                lodestone_image::Source::Curseforge {
                    project: 12345,
                    file: 678,
                    downloadable: true,
                },
                None,
            ),
            (
                "config/other.bin",
                b"a curseforge jar",
                lodestone_image::Source::Object {},
                None,
            ),
        ],
        &file,
    );
    let mut image = lodestone_image::Image::from_json(&zip_read(&file, "image.json")).unwrap();
    image.label = Some("Survival\u{202E}lacitnedi".into());
    image.seal();
    let labelled = other.data.root().join("labelled.lodeimg");
    rewrite_zip(
        &file,
        &labelled,
        |name, bytes| {
            Some(if name == "image.json" {
                image.to_json_pretty()
            } else {
                bytes
            })
        },
        &[],
    );
    // The preview names both as the sender's files.
    let preview = export::inspect(&labelled).unwrap();
    assert_eq!(preview.carried_mods, ["other.jar", "sodium.jar"]);
    let imported = import(&other, &labelled, true).unwrap();
    let game = other.store.game_dir(&imported.instance.id);
    // Placed as the sender's files, which nothing records as a project's.
    assert_eq!(fs::read(game.join("mods/sodium.jar")).unwrap(), evil);
    assert_eq!(
        fs::read(game.join("mods/other.jar")).unwrap(),
        b"a curseforge jar"
    );
    let known = Provenance::load(&other.store.dir(&imported.instance.id));
    for name in ["mods/sodium.jar", "mods/other.jar"] {
        assert!(
            known
                .files
                .get(name)
                .is_none_or(|e| e.modrinth.is_none() && e.curseforge.is_none()),
            "{name}: {:?}",
            known.files.get(name)
        );
    }
    assert!(
        other
            .mock
            .requests()
            .iter()
            .all(|r| !r.path.ends_with("/evil.jar")),
        "a URL that isn't the named project's is never downloaded"
    );
    // The snapshot kept of what was sent claims no source either.
    let snapshots = other.images.snapshots(&imported.instance.id).unwrap();
    let kept = other
        .images
        .store()
        .load_manifest(&imported.instance.id, &snapshots.records[0].image)
        .unwrap();
    assert!(
        kept.entries
            .iter()
            .all(|e| e.source == lodestone_image::Source::Object {}),
        "{:?}",
        kept.entries
    );
    // The label's direction override is gone from the instance's name.
    assert_eq!(imported.instance.name, "Survivallacitnedi");
}

#[test]
fn a_url_that_resolves_to_another_project_never_passes_for_the_one_named() {
    let evil: &[u8] = b"another project's jar";
    let evil_url = "https://cdn.modrinth.com/data/EVILPROJ/versions/x1/evil.jar";
    for spoof in [
        "https://cdn.modrinth.com/data/AANobbMI/versions/v1/../../../EVILPROJ/versions/x1/evil.jar",
        "https://cdn.modrinth.com/data/AANobbMI/versions/v1/%2e%2e/%2E%2E/%2e%2e/EVILPROJ/versions/x1/evil.jar",
        "https://cdn.modrinth.com/data/AANobbMI/versions/v1/..\\..\\..\\EVILPROJ/versions/x1/evil.jar",
        "https://cdn.modrinth.com/data/AANobbMI/versions/v1/evil.jar?x=../../EVILPROJ",
    ] {
        let other = elsewhere();
        other.mock.serve(evil_url, evil);
        let file = other.data.root().join("spoof.lodeimg");
        // The bytes are carried too, under another path.
        hostile_export(
            &other.data.root().join("sender"),
            lodestone_image::Loader::Fabric,
            &[
                (
                    "mods/sodium.jar",
                    evil,
                    lodestone_image::Source::Modrinth {
                        project: "AANobbMI".into(),
                        version: "v1".into(),
                        url: spoof.into(),
                    },
                    Some(hex_sha512(evil)),
                ),
                (
                    "config/blob.bin",
                    evil,
                    lodestone_image::Source::Object {},
                    None,
                ),
            ],
            &file,
        );
        // Carried, so the preview names it as the sender's file.
        assert_eq!(export::inspect(&file).unwrap().carried_mods, ["sodium.jar"]);
        let imported = import(&other, &file, true).unwrap();
        let known = Provenance::load(&other.store.dir(&imported.instance.id));
        assert!(
            known
                .files
                .get("mods/sodium.jar")
                .is_none_or(|e| e.modrinth.is_none()),
            "{spoof}: {:?}",
            known.files.get("mods/sodium.jar")
        );
        assert!(
            other
                .mock
                .requests()
                .iter()
                .all(|r| !r.path.ends_with("/evil.jar")),
            "{spoof} was downloaded"
        );
    }
}

#[test]
fn an_export_listing_many_files_it_leaves_out_imports_in_linear_time() {
    let other = elsewhere();
    let store = lodestone_image::store::Store::new(
        other.data.root().join("sender"),
        lodestone_image::store::Encoding::Raw,
    );
    let mut image = lodestone_image::Image::new(
        lodestone_image::Kind::Full,
        lodestone_image::Game {
            minecraft: "1.21.1".into(),
            loader: lodestone_image::Loader::Fabric,
            loader_version: Some("0.16.10".into()),
        },
    );
    let count = 50_000u64;
    for n in 0..count {
        image.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new(format!("mods/m{n}.jar")).unwrap(),
            10,
            format!("{n:064x}"),
            lodestone_image::Source::Curseforge {
                project: n,
                file: n,
                downloadable: false,
            },
        ));
    }
    image.normalize();
    image.seal();
    let file = other.data.root().join("many.lodeimg");
    lodestone_image::container::export(&store, &image, &[], &file, NEVER).unwrap();
    let started = std::time::Instant::now();
    let imported = import(&other, &file, true).unwrap();
    assert_eq!(imported.left_out.len(), count as usize);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn mods_below_the_mods_folder_count_and_carried_ones_are_named() {
    let other = elsewhere();
    let file = other.data.root().join("quilt.lodeimg");
    hostile_export(
        &other.data.root().join("sender"),
        lodestone_image::Loader::Quilt,
        &[
            (
                "mods/1.21.1/evil.jar",
                b"a jar from the sender",
                lodestone_image::Source::Object {},
                None,
            ),
            (
                "coremods/old.jar",
                b"an old forge coremod",
                lodestone_image::Source::Object {},
                None,
            ),
            (
                "mods/sodium.jar",
                SODIUM,
                lodestone_image::Source::Modrinth {
                    project: "AANobbMI".into(),
                    version: "v1".into(),
                    url: SODIUM_URL.into(),
                },
                Some(hex_sha512(SODIUM)),
            ),
        ],
        &file,
    );
    let preview = export::inspect(&file).unwrap();
    let mut mods = preview.mods.clone();
    mods.sort();
    assert_eq!(mods, ["1.21.1/evil.jar", "coremods/old.jar", "sodium.jar"]);
    let mut carried = preview.carried_mods.clone();
    carried.sort();
    assert_eq!(carried, ["1.21.1/evil.jar", "coremods/old.jar"]);
}

#[test]
fn a_mods_folder_spelled_another_way_is_still_a_mods_folder() {
    let other = elsewhere();
    // Upper case: the same folder on most players' disks, and counted as mods.
    let upper = other.data.root().join("upper.lodeimg");
    hostile_export(
        &other.data.root().join("sender-upper"),
        lodestone_image::Loader::Fabric,
        &[(
            "MODS/x.jar",
            b"a jar from the sender",
            lodestone_image::Source::Object {},
            None,
        )],
        &upper,
    );
    let preview = export::inspect(&upper).unwrap();
    assert_eq!(preview.mods, ["x.jar"]);
    assert_eq!(preview.carried_mods, ["x.jar"]);
    // A long s: `modſ/` is `mods/` on APFS, where a loader would load it, yet no letter
    // comparison sees it. Refused outright.
    let long_s = other.data.root().join("long-s.lodeimg");
    hostile_export(
        &other.data.root().join("sender-long-s"),
        lodestone_image::Loader::Fabric,
        &[(
            "mod\u{17f}/evil.jar",
            b"a jar from the sender",
            lodestone_image::Source::Object {},
            None,
        )],
        &long_s,
    );
    let before = instance_count(&other);
    assert!(export::inspect(&long_s).is_err());
    assert!(import(&other, &long_s, true).is_err());
    assert_eq!(instance_count(&other), before);
}

#[test]
fn an_import_refuses_a_file_that_changed_since_its_preview() {
    let (f, record) = fixture();
    let file = f.data.root().join("Survival.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    let other = elsewhere();
    other.mock.serve(SODIUM_URL, SODIUM);
    let modrinth = other.mock.client();
    let downloader = downloader();
    let refused = block_on(export::import_file(
        &other.images,
        Sources {
            downloader: &downloader,
            modrinth: &modrinth,
            curseforge: None,
        },
        &file,
        None,
        Some(&"0".repeat(64)),
        &|_| Ok(()),
        &Progress::default(),
        &CancelToken::new(),
    ));
    assert!(refused.is_err());
    assert_eq!(instance_count(&other), 0);
}
