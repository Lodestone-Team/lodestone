//! The legacy reader and the store together: a backup the core made (schema 1, in its own
//! zstd object store) read as an image and restored by the image store, in place, without
//! rewriting an object.
//!
//! Each test builds a small backup store the way `core/src/backups/store.rs` writes one at
//! commit f5ce27ad, independently of this crate's store: one zstd frame per object (level
//! 3, with a checksum, the content size and the pledged size, fed in 1 MiB pieces), named
//! `objects/ab/<sha256>`, read-only, and a compact schema-1 manifest. Then
//! `legacy::core_backup` converts the manifest and `store::materialize` places its files.

#![cfg(feature = "zstd")]

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use lodestone_image::hex::sha256_hex;
use lodestone_image::legacy::core_backup;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::materialize::{
    materialize, missing_objects, MaterializeError, MaterializeOptions, Selection,
};
use lodestone_image::store::snapshot::{snapshot, SnapshotOptions};
use lodestone_image::store::{Encoding, ObjectForm, Store};
use lodestone_image::{Kind, WithheldReason};
use serde_json::{json, Value};

/// The core's zstd level and piece size (`ZSTD_LEVEL`, `CHUNK`).
const CORE_ZSTD_LEVEL: i32 = 3;
const CORE_PIECE: usize = 1 << 20;

/// One file of the server the backup was taken of.
struct ServerFile {
    path: &'static str,
    bytes: Vec<u8>,
    executable: bool,
    modified_ns: i128,
}

fn file(path: &'static str, bytes: impl Into<Vec<u8>>) -> ServerFile {
    ServerFile {
        path,
        bytes: bytes.into(),
        executable: false,
        // Well before the scan began, as most of a server's files are.
        modified_ns: 1_794_660_000_000_000_000,
    }
}

/// Bytes that don't compress, so a large file's object is a large frame.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// Writes `bytes` into the core-style store at `root` exactly as the core's
/// `Store::store_object` does, and returns its SHA-256.
fn core_store_object(root: &Path, bytes: &[u8]) -> String {
    let sha256 = sha256_hex(bytes);
    let path = core_object_path(root, &sha256);
    if path.exists() {
        return sha256;
    }
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_core_frame(&path, bytes);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    sha256
}

/// One zstd frame of `bytes` at `path`, with the core's settings.
fn write_core_frame(path: &Path, bytes: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut encoder =
        zstd::stream::write::Encoder::new(BufWriter::new(file), CORE_ZSTD_LEVEL).unwrap();
    encoder.include_checksum(true).unwrap();
    encoder.include_contentsize(true).unwrap();
    encoder
        .set_pledged_src_size(Some(bytes.len() as u64))
        .unwrap();
    for piece in bytes.chunks(CORE_PIECE) {
        encoder.write_all(piece).unwrap();
    }
    let file = encoder.finish().unwrap().into_inner().unwrap();
    file.sync_all().unwrap();
}

fn core_object_path(root: &Path, sha256: &str) -> PathBuf {
    root.join("objects").join(&sha256[..2]).join(sha256)
}

/// A backup of `files` in a core-style store at `root`, and its schema-1 manifest as the
/// core writes it (compact, fields in the core's order).
fn core_backup(
    root: &Path,
    flavour: &str,
    level_name: &str,
    files: &[ServerFile],
    skipped: Value,
) -> Vec<u8> {
    let mut sorted: Vec<&ServerFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(b.path));
    let entries: Vec<Value> = sorted
        .iter()
        .map(|f| {
            let sha256 = core_store_object(root, &f.bytes);
            json!({
                "path": f.path,
                "size": f.bytes.len(),
                "sha256": sha256,
                "modified_ns": f.modified_ns,
                "executable": f.executable,
            })
        })
        .collect();
    let total: usize = files.iter().map(|f| f.bytes.len()).sum();
    let manifest = json!({
        "schema": 1,
        "id": "20261114-101500-ab12",
        "instance_uuid": "INSTANCE_test-1234",
        "instance_name": "Survival",
        "game_version": "1.21.1",
        "flavour": flavour,
        "name": "Before the update",
        "kind": "manual",
        "created": 1_794_645_624,
        "scan_started_ns": 1_794_660_827_542_808_000i128,
        "pinned": false,
        "live": false,
        "quiesced": true,
        "level_name": level_name,
        "full_scan": true,
        "files": entries,
        "skipped": skipped,
        "total_bytes": total,
        "new_bytes": total,
        "stored_bytes": total,
        "duration_ms": 12,
        "core_version": "0.5.1",
    });
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let dir = root.join("servers").join("INSTANCE_test-1234");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("20261114-101500-ab12.json"), &bytes).unwrap();
    bytes
}

fn fabric_server() -> Vec<ServerFile> {
    let mut start = file("start.sh", "#!/bin/sh\njava -jar server.jar\n");
    start.executable = true;
    start.modified_ns = 1_700_000_000_123_456_789;
    vec![
        file("server.properties", "level-name=world\nmotd=hi\n"),
        file("mods/sodium.jar", "a mod"),
        file("config/fabric.toml", "x = 1"),
        start,
        file("world/level.dat", "level 1"),
        // Over the store's chunking threshold: the core stored it whole, and the image
        // store reads that object in place.
        file("world/region/r.0.0.mca", noise(1_536 * 1024 + 7, 42)),
        file("creative/level.dat", "level 2"),
        file("creative/region/r.0.0.mca", "creative region"),
        // The same bytes twice: one object.
        file("world/data/raids.dat", "same"),
        file("creative/data/raids.dat", "same"),
    ]
}

/// Every regular file under `root`, by `/`-separated path relative to it.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in fs::read_dir(&dir).unwrap() {
            let item = item.unwrap();
            let path = item.path();
            if item.file_type().unwrap().is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).unwrap();
                let rel: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.insert(rel.join("/"), fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn expected(files: &[ServerFile]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .map(|f| (f.path.to_string(), f.bytes.clone()))
        .collect()
}

#[test]
fn a_core_backup_restores_through_the_image_store_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let files = fabric_server();
    let manifest = core_backup(
        &root,
        "fabric",
        "world",
        &files,
        json!([{"path": "mods/broken.jar", "reason": "unreadable", "carried_over": false}]),
    );
    let objects_before = tree(&root.join("objects"));

    let converted = core_backup::read(&manifest).unwrap();
    let image = &converted.image;
    image.verify().unwrap();
    assert_eq!(image.kind, Kind::Data);
    assert_eq!(image.entries.len(), files.len());
    assert_eq!(image.withheld.len(), 1);
    assert_eq!(image.withheld[0].reason, WithheldReason::Unreadable);
    let mut worlds: Vec<&str> = image.worlds.iter().map(|w| w.name.as_str()).collect();
    worlds.sort();
    assert_eq!(worlds, ["creative", "world"]);

    let store = Store::new(&root, Encoding::Zstd);
    assert!(missing_objects(&store, image, Selection::All).is_empty());
    let layout = core_backup::disk_layout(image, &converted.rest).unwrap();
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let placed = materialize(
        &store,
        image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    assert_eq!(placed.files, files.len());
    assert_eq!(
        placed.bytes,
        files.iter().map(|f| f.bytes.len() as u64).sum::<u64>()
    );
    assert!(placed.not_placed.is_empty());

    // Every file is back at the path the backup had, with its bytes.
    assert_eq!(tree(&staging), expected(&files));
    // ...with its time and, on unix, its execute bit.
    let start = fs::metadata(staging.join("start.sh")).unwrap();
    let modified = start
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    assert_eq!(modified.as_secs(), 1_700_000_000);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(start.permissions().mode() & 0o777, 0o755);
        let props = fs::metadata(staging.join("server.properties")).unwrap();
        assert_eq!(props.permissions().mode() & 0o777, 0o644);
    }

    // Reading touched no object: the core's store is as the core left it.
    assert_eq!(tree(&root.join("objects")), objects_before);
}

#[test]
fn a_restored_core_backup_snapshots_to_the_same_entries_without_new_objects() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let files = fabric_server();
    let manifest = core_backup(&root, "fabric", "world", &files, json!([]));
    let converted = core_backup::read(&manifest).unwrap();
    let store = Store::new(&root, Encoding::Zstd);
    let layout = core_backup::disk_layout(&converted.image, &converted.rest).unwrap();
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    materialize(
        &store,
        &converted.image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    let summary = |entries: &[lodestone_image::Entry]| -> Vec<(String, String, u64, u32)> {
        entries
            .iter()
            .map(|e| (e.path.to_string(), e.sha256.clone(), e.size, e.mode))
            .collect()
    };
    let objects_before = tree(&root.join("objects"));

    // The first image backup after the conversion, seeded with the old backup as its stat
    // cache (as the core uses its last backup), reads nothing and stores nothing: the
    // restore gave every file its old time back.
    let cache = core_backup::stat_cache(&core_backup::Manifest::from_json(&manifest).unwrap());
    let options = SnapshotOptions {
        cache: Some(&cache),
        ..SnapshotOptions::default()
    };
    let snap = snapshot(&store, &staging, &layout, &options).unwrap();
    assert!(snap.withheld.is_empty(), "{:?}", snap.withheld);
    assert_eq!((snap.files_read, snap.bytes_added), (0, 0));
    assert_eq!(summary(&snap.entries), summary(&converted.image.entries));
    assert_eq!(
        snap.worlds.iter().map(String::as_str).collect::<Vec<_>>(),
        ["creative", "world"]
    );

    // A full scan reads everything and finds the same entries. Small files are the core's
    // objects as they are; the large one is cut into chunks as any large file is, but the
    // core's whole object is kept as its object (old objects are never rewritten), so
    // those chunks are unused until the file changes, and GC frees them.
    let full = snapshot(&store, &staging, &layout, &SnapshotOptions::default()).unwrap();
    assert_eq!(summary(&full.entries), summary(&converted.image.entries));
    assert_eq!(full.files_read, files.len());
    let large = &files[5];
    assert!(large.bytes.len() > 1 << 20);
    assert!(full.bytes_added > 0 && full.bytes_added <= large.bytes.len() as u64 + 4096);
    assert_eq!(
        store
            .object_form(&sha256_hex(&large.bytes), large.bytes.len() as u64)
            .unwrap(),
        ObjectForm::Whole
    );
    assert_eq!(tree(&root.join("objects")), objects_before);
    store.save_manifest("survival", &converted.image).unwrap();
    let report = store
        .gc(&GcOptions {
            grace: Duration::ZERO,
            ..GcOptions::default()
        })
        .unwrap();
    assert!(report.chunks_removed > 0);
    assert_eq!(report.objects_removed, 0);
    assert!(!root.join("chunks").exists() || tree(&root.join("chunks")).is_empty());
}

#[test]
fn a_paper_backup_folds_its_split_level_and_restores_it_split() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let files = vec![
        file("server.properties", "level-name=survival\n"),
        file("plugins/essentials.jar", "plugin"),
        file("survival/level.dat", "overworld"),
        file("survival/region/r.0.0.mca", "overworld region"),
        file("survival_nether/DIM-1/region/r.0.0.mca", "nether region"),
        file("survival_the_end/DIM1/region/r.0.0.mca", "end region"),
    ];
    let manifest = core_backup(&root, "paper", "survival", &files, json!([]));
    let converted = core_backup::read(&manifest).unwrap();
    let image = &converted.image;
    // One world in the image...
    assert_eq!(
        image
            .worlds
            .iter()
            .map(|w| w.name.as_str())
            .collect::<Vec<_>>(),
        ["survival"]
    );
    assert!(image
        .entries
        .iter()
        .filter(|e| e.path.as_str().contains("region"))
        .all(|e| e.world.as_deref() == Some("survival")));

    // ...and three folders again on the Paper server's disk.
    let store = Store::new(&root, Encoding::Zstd);
    let layout = core_backup::disk_layout(image, &converted.rest).unwrap();
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    materialize(
        &store,
        image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    assert_eq!(tree(&staging), expected(&files));

    // Only the worlds, as a world restore would.
    let worlds_only = dir.path().join("worlds-only");
    fs::create_dir(&worlds_only).unwrap();
    let names = ["survival".to_string()];
    let options = MaterializeOptions {
        selection: Selection::Worlds(&names),
        ..MaterializeOptions::default()
    };
    materialize(&store, image, &layout, &worlds_only, &options).unwrap();
    let got: Vec<String> = tree(&worlds_only).into_keys().collect();
    assert_eq!(
        got,
        [
            "survival/level.dat",
            "survival/region/r.0.0.mca",
            "survival_nether/DIM-1/region/r.0.0.mca",
            "survival_the_end/DIM1/region/r.0.0.mca",
        ]
    );
}

#[test]
fn a_missing_or_damaged_core_object_fails_the_restore() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let files = fabric_server();
    let manifest = core_backup(&root, "fabric", "world", &files, json!([]));
    let converted = core_backup::read(&manifest).unwrap();
    let image = &converted.image;
    let store = Store::new(&root, Encoding::Zstd);
    let layout = core_backup::disk_layout(image, &converted.rest).unwrap();

    // Damaged: a valid frame of other bytes of the same length under the mod's name.
    let mod_bytes = b"a mod";
    let mod_object = core_object_path(&root, &sha256_hex(mod_bytes));
    let mut permissions = fs::metadata(&mod_object).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&mod_object, permissions).unwrap();
    write_core_frame(&mod_object, b"b mod");
    let staging = dir.path().join("damaged");
    fs::create_dir(&staging).unwrap();
    let error = materialize(
        &store,
        image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, MaterializeError::Damaged { path } if path.as_str() == "mods/sodium.jar"),
        "{error:?}"
    );

    // Missing: nothing is placed, and the missing file is named.
    fs::remove_file(&mod_object).unwrap();
    let missing: Vec<&str> = missing_objects(&store, image, Selection::All)
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(missing, ["mods/sodium.jar"]);
    let staging = dir.path().join("missing");
    fs::create_dir(&staging).unwrap();
    let error = materialize(
        &store,
        image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, MaterializeError::Missing(paths) if paths.len() == 1),
        "{error:?}"
    );
    assert!(tree(&staging).is_empty());
}

#[test]
fn gc_keeps_what_converted_images_and_schema_1_manifests_use() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    // An older backup only schema 1 lists, and a newer one converted to an image.
    let old_files = vec![
        file("server.properties", "old"),
        file("world/level.dat", "old level"),
    ];
    let old_manifest = core_backup(&root, "fabric", "world", &old_files, json!([]));
    let files = fabric_server();
    let manifest = core_backup(&root, "fabric", "world", &files, json!([]));
    let garbage = core_store_object(&root, b"no backup uses this");

    let converted = core_backup::read(&manifest).unwrap();
    let store = Store::new(&root, Encoding::Zstd);
    store.save_manifest("survival", &converted.image).unwrap();

    let old = core_backup::Manifest::from_json(&old_manifest).unwrap();
    let options = GcOptions {
        grace: Duration::ZERO,
        also_live: old.objects().map(str::to_string).collect::<HashSet<_>>(),
        ..GcOptions::default()
    };
    let report = store.gc(&options).unwrap();
    assert_eq!(report.manifests, 1);
    assert_eq!(report.objects_removed, 1);
    assert!(!core_object_path(&root, &garbage).exists());
    for f in old_files.iter().chain(&files) {
        let sha256 = sha256_hex(&f.bytes);
        assert!(
            store.has_object(&sha256, f.bytes.len() as u64),
            "{}",
            f.path
        );
    }

    // Both backups still restore.
    for (bytes, files) in [(&old_manifest, &old_files), (&manifest, &files)] {
        let converted = core_backup::read(bytes).unwrap();
        let layout = core_backup::disk_layout(&converted.image, &converted.rest).unwrap();
        let staging = tempfile::tempdir_in(dir.path()).unwrap();
        materialize(
            &store,
            &converted.image,
            &layout,
            staging.path(),
            &MaterializeOptions::default(),
        )
        .unwrap();
        assert_eq!(tree(staging.path()), expected(files));
    }
}
