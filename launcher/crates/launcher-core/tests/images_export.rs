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
            Err(format!("no room for {bytes} bytes"))
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
        ["command_history.txt", "config/ias.json", "hotbar.nbt"]
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
    ] {
        assert!(!names.contains(&format!("objects/{}", hex_sha256(private))));
    }
    assert!(names.contains(&format!("objects/{}", hex_sha256(b"region bytes"))));
    assert!(!objects.is_empty());
    let image = lodestone_image::Image::from_json(&zip_read(&file, "image.json")).unwrap();
    for pattern in PRIVATE_FILES {
        assert!(image.excludes.iter().any(|e| e == pattern), "{pattern}");
    }
    assert_eq!(image.label.as_deref(), Some("Before the end"));
    // The preview reads it without unpacking.
    let preview = export::inspect(&file).unwrap();
    assert_eq!(preview.minecraft, "1.21.1");
    assert_eq!(preview.loader, "fabric");
    assert_eq!(preview.mods, ["sodium.jar"]);
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
fn a_download_that_doesnt_match_and_a_full_disk_are_refused() {
    let (f, record) = fixture();
    let file = f.data.root().join("Survival.lodeimg");
    f.images
        .export_snapshot(&f.instance.id, &record.id, &file, NEVER)
        .unwrap();
    let other = elsewhere();
    let before = instance_count(&other);
    other.mock.serve(SODIUM_URL, b"something else entirely");
    assert!(import(&other, &file, true).is_err());
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
