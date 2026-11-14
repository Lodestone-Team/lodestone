//! The legacy `.lodesnap` reader and the store together, as the Launcher (raw objects, no
//! zstd) will use them: a snapshot file the Launcher exported is read with the Launcher's
//! checks, its objects go into an image store (each checked against its name), and the
//! converted image is placed into a staging folder through the client layout.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use lodestone_image::hex::sha256_hex;
use lodestone_image::legacy::lodesnap;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::materialize::{
    materialize, missing_objects, MaterializeOptions, Selection,
};
use lodestone_image::store::{Encoding, Store, StoreError, NEVER};
use lodestone_image::DiskLayout;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/legacy/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Imports every object of a `.lodesnap` into `store`, each only if its bytes hash to its
/// name. Returns the hashes imported.
fn import_objects(store: &Store, lodesnap: &[u8]) -> Result<BTreeSet<String>, StoreError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(lodesnap)).unwrap();
    let mut imported = BTreeSet::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        let Some(hash) = name.strip_prefix("objects/").filter(|h| !h.is_empty()) else {
            continue;
        };
        let hash = hash.to_string();
        let size = entry.size();
        store.put_reader(&mut entry, size, Some(&hash), NEVER)?;
        imported.insert(hash);
    }
    Ok(imported)
}

/// Every regular file under `root`, by `/`-separated path, with its bytes' SHA-256.
fn tree(root: &Path) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in fs::read_dir(&dir).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let mut bytes = Vec::new();
                fs::File::open(&path)
                    .unwrap()
                    .read_to_end(&mut bytes)
                    .unwrap();
                let rel: Vec<String> = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.insert((rel.join("/"), sha256_hex(&bytes)));
            }
        }
    }
    out
}

#[test]
fn a_lodesnap_imports_into_a_raw_store_and_materializes_its_game_folder() {
    let bytes = fixture("survival.lodesnap");
    let snapshot = lodesnap::read_lodesnap(Cursor::new(&bytes)).unwrap();
    let converted = lodesnap::to_image(&snapshot).unwrap();
    let image = &converted.image;
    image.verify().unwrap();

    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("store"), Encoding::Raw);
    let imported = import_objects(&store, &bytes).unwrap();
    assert!(missing_objects(&store, image, Selection::All).is_empty());
    // The icon and background the instance had are settings objects, imported too.
    let client = image.settings.client.as_ref();
    assert!(client.is_some_and(|c| c.icon.is_some() && c.background.is_some()));
    for object in [
        client.and_then(|c| c.icon.as_ref()),
        client.and_then(|c| c.background.as_ref()),
    ]
    .into_iter()
    .flatten()
    {
        assert!(imported.contains(&object.sha256));
        assert!(store.has_object(&object.sha256, object.size));
    }

    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let placed = materialize(
        &store,
        image,
        &DiskLayout::Client,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    assert_eq!(placed.files, image.entries.len());
    assert!(placed.not_placed.is_empty());

    // Exactly the snapshot's game-folder files, where the Launcher had them; files beside
    // the game folder stay in the rest and are not placed.
    let want: BTreeSet<(String, String)> = snapshot
        .files
        .iter()
        .filter(|f| f.path.starts_with("minecraft/"))
        .filter(|f| {
            !converted
                .rest
                .unmapped
                .iter()
                .any(|(u, _)| u.path == f.path)
        })
        .map(|f| (f.path.clone(), f.sha256.clone()))
        .collect();
    assert!(!want.is_empty());
    assert_eq!(tree(&staging), want);
    for file in &converted.rest.instance_files {
        assert!(!staging.join(&file.path).exists(), "{}", file.path);
    }

    // Saved as an image, GC keeps every object it uses, settings objects included.
    store.save_manifest("survival", image).unwrap();
    let report = store
        .gc(&GcOptions {
            grace: std::time::Duration::ZERO,
            ..GcOptions::default()
        })
        .unwrap();
    let used: BTreeSet<&str> = image
        .entries
        .iter()
        .map(|e| e.sha256.as_str())
        .chain(
            [
                client.and_then(|c| c.icon.as_ref()),
                client.and_then(|c| c.background.as_ref()),
            ]
            .into_iter()
            .flatten()
            .map(|o| o.sha256.as_str()),
        )
        .collect();
    assert_eq!(
        report.objects_removed as usize,
        imported
            .iter()
            .filter(|h| !used.contains(h.as_str()))
            .count()
    );
    assert!(missing_objects(&store, image, Selection::All).is_empty());
}

#[test]
fn a_lodesnap_object_that_doesnt_match_its_name_is_refused() {
    let bytes = fixture("survival.lodesnap");
    // Rewrite one object's bytes, keeping its name.
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut tampered = None;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if tampered.is_none() && name.starts_with("objects/") && !data.is_empty() {
            data[0] ^= 1;
            tampered = Some(name.clone());
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    let bad = out.finish().unwrap().into_inner();
    let tampered = tampered.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path(), Encoding::Raw);
    let error = import_objects(&store, &bad).unwrap_err();
    assert!(
        matches!(error, StoreError::HashMismatch { .. }),
        "{error:?}"
    );
    let hash = tampered.strip_prefix("objects/").unwrap();
    assert!(!store.object_path(hash).unwrap().exists());
}
