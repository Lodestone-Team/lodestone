use super::*;
use crate::layout::DiskLayout;
use crate::manifest::tests::{fabric, path};
use crate::manifest::{Entry, Kind, World};
use crate::settings::{ClientSettings, StoredFile};
use crate::store::cdc::tests::region_like;
use crate::store::materialize::{materialize, MaterializeOptions};
use crate::store::tests::{encodings, store};
use crate::store::{Encoding, NEVER};

/// A content image and a data image linked to it, with every kind of entry.
struct Fixture {
    content: Image,
    data: Image,
    big: Vec<u8>,
}

fn world(name: &str) -> World {
    World {
        name: name.into(),
        level_name: None,
        data_version: None,
        game_version: None,
        size: None,
        last_played: None,
        quiesced: None,
    }
}

fn fixture(store: &Store) -> Fixture {
    let object = |name: &str, bytes: &[u8]| {
        let stored = store.put_bytes(bytes).unwrap();
        Entry::new(
            path(name),
            bytes.len() as u64,
            stored.sha256,
            Source::Object {},
        )
    };
    let mut content = Image::new(Kind::Content, fabric());
    let mut jei = Entry::new(
        path("mods/jei.jar"),
        4,
        sha256_hex(b"jeii"),
        Source::Modrinth {
            project: "u6dRKJwZ".into(),
            version: "v1".into(),
            url: "https://cdn.modrinth.com/data/u6dRKJwZ/versions/v1/jei.jar".into(),
        },
    );
    jei.sha512 = Some("b".repeat(128));
    content.entries = vec![jei, object("config/jei.toml", b"jei config")];
    let icon = store.put_bytes(b"icon png").unwrap();
    content.settings.client = Some(ClientSettings {
        icon: Some(StoredFile {
            sha256: icon.sha256,
            size: 8,
        }),
        ..ClientSettings::default()
    });
    content.seal();

    let big = region_like(1, 2_500_000);
    let mut data = Image::new(Kind::Data, fabric());
    data.content = Some(content.id.clone());
    data.worlds = vec![world("world")];
    data.entries = vec![
        object("saves/world/level.dat", b"level"),
        object("saves/world/region/r.0.0.mca", &big),
        // The same bytes twice: carried once.
        object("saves/world/copy.dat", b"level"),
    ];
    data.seal();
    Fixture { content, data, big }
}

fn names(path: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| {
            archive
                .by_index_raw(i)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn no_check(_: u64) -> Result<(), String> {
    Ok(())
}

#[test]
fn an_export_round_trips_between_encodings() {
    for from in encodings() {
        for to in encodings() {
            let (dir, source) = store(from);
            let fixture = fixture(&source);
            let file = dir.path().join("world.lodeimage");
            let report = export(
                &source,
                &fixture.data,
                std::slice::from_ref(&fixture.content),
                &file,
                NEVER,
            )
            .unwrap();
            assert!(!part_path(&file).exists());
            // Objects: level.dat, the region list, the config and the icon; the Modrinth jar
            // travels as a reference.
            assert_eq!(report.objects, 4);
            assert!(report.chunks > 5);
            let all = names(&file);
            assert!(all.contains(&IMAGE_FILE.to_string()));
            assert!(all.contains(&format!("images/{}.json", fixture.content.id)));
            assert!(!all.iter().any(|n| n.ends_with(&sha256_hex(b"jeii"))));
            assert_eq!(all.iter().filter(|n| n.starts_with("objects/")).count(), 4);

            let (_dir2, target) = store(to);
            let mut container = open(&file, &Limits::default()).unwrap();
            assert_eq!(container.image(), &fixture.data);
            assert_eq!(container.linked(), std::slice::from_ref(&fixture.content));
            assert!(container.unpacked_bytes() >= fixture.big.len() as u64);
            let imported = container.import_objects(&target, &no_check, NEVER).unwrap();
            assert_eq!(imported.objects, 4);
            // Importing again adds nothing.
            let again = container.import_objects(&target, &no_check, NEVER).unwrap();
            assert_eq!(again.objects, 0);

            target.save_manifest("copy", container.image()).unwrap();
            let staging = dir.path().join(format!("staging-{from:?}-{to:?}"));
            fs::create_dir(&staging).unwrap();
            materialize(
                &target,
                container.image(),
                &DiskLayout::Client,
                &staging,
                &MaterializeOptions::default(),
            )
            .unwrap();
            assert_eq!(
                fs::read(staging.join("minecraft/saves/world/region/r.0.0.mca")).unwrap(),
                fixture.big
            );
            assert!(target.has_object(
                &fixture
                    .content
                    .settings
                    .client
                    .as_ref()
                    .unwrap()
                    .icon
                    .as_ref()
                    .unwrap()
                    .sha256,
                8
            ));
        }
    }
}

#[test]
fn exports_are_deterministic() {
    let (dir, store) = store(Encoding::Raw);
    let fixture = fixture(&store);
    let a = dir.path().join("a.zip");
    let b = dir.path().join("b.zip");
    export(
        &store,
        &fixture.data,
        std::slice::from_ref(&fixture.content),
        &a,
        NEVER,
    )
    .unwrap();
    export(
        &store,
        &fixture.data,
        std::slice::from_ref(&fixture.content),
        &b,
        NEVER,
    )
    .unwrap();
    assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
}

#[test]
fn a_failed_export_leaves_nothing() {
    let (dir, store) = store(Encoding::Raw);
    let fixture = fixture(&store);
    // The store lost an object the image carries.
    let level = &fixture.data.entries[1];
    assert_eq!(level.path.as_str(), "saves/world/level.dat");
    crate::store::fsx::remove_file_forced(&store.object_path(&level.sha256).unwrap()).unwrap();
    let file = dir.path().join("out.zip");
    assert!(export(&store, &fixture.data, &[], &file, NEVER).is_err());
    assert!(!file.exists());
    assert!(!part_path(&file).exists());
    // A linked image the image doesn't link to is refused before writing.
    let (_d, other_store) = crate::store::tests::store(Encoding::Raw);
    let other = fixture_store_image(&other_store);
    assert!(matches!(
        export(&store, &fixture.content, &[other], &file, NEVER),
        Err(ContainerError::Invalid(_))
    ));
    // So is an unsealed image.
    let mut unsealed = fixture.content.clone();
    unsealed.id.clear();
    assert!(export(&store, &unsealed, &[], &file, NEVER).is_err());
}

fn fixture_store_image(store: &Store) -> Image {
    crate::store::tests::stored_image(store, &[("mods/x.jar", b"x")])
}

/// Writes a zip of these entries as they are.
fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, options).unwrap();
        } else {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
    }
    zip.finish().unwrap();
}

/// Replaces every `from` in the file with `to` (same length): names zip writers refuse.
fn patch(path: &Path, from: &str, to: &str) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut found = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from.as_bytes() {
            bytes[i..i + from.len()].copy_from_slice(to.as_bytes());
            found += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert!(found > 0);
    fs::write(path, bytes).unwrap();
}

/// A minimal valid export's entries: an image with two small objects.
fn small_export() -> (Image, Vec<(String, Vec<u8>)>) {
    let mut image = Image::new(Kind::Content, fabric());
    for (name, bytes) in [
        ("config/a.toml", &b"aaaa"[..]),
        ("config/b.toml", b"bbbbbb"),
    ] {
        image.entries.push(Entry::new(
            path(name),
            bytes.len() as u64,
            sha256_hex(bytes),
            Source::Object {},
        ));
    }
    image.seal();
    let entries = vec![
        (IMAGE_FILE.to_string(), image.to_json_pretty()),
        (format!("objects/{}", sha256_hex(b"aaaa")), b"aaaa".to_vec()),
        (
            format!("objects/{}", sha256_hex(b"bbbbbb")),
            b"bbbbbb".to_vec(),
        ),
    ];
    (image, entries)
}

fn write_entries(path: &Path, entries: &[(String, Vec<u8>)]) {
    let borrowed: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    zip_with(path, &borrowed);
}

fn open_error(path: &Path) -> String {
    match open(path, &Limits::default()) {
        Ok(_) => panic!("{} opened", path.display()),
        Err(e) => e.to_string(),
    }
}

#[test]
fn the_minimal_export_is_fine() {
    let dir = tempfile::tempdir().unwrap();
    let (image, entries) = small_export();
    let file = dir.path().join("ok.zip");
    write_entries(&file, &entries);
    let mut container = open(&file, &Limits::default()).unwrap();
    assert_eq!(container.image(), &image);
    let (_d, store) = store(Encoding::Raw);
    container.import_objects(&store, &no_check, NEVER).unwrap();
    assert_eq!(
        store.read_object_bytes(&sha256_hex(b"bbbbbb"), 6).unwrap(),
        b"bbbbbb"
    );
}

#[test]
fn unexpected_names_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_, entries) = small_export();
    // Each placeholder is patched over with the hostile name of the same length, which
    // zip writers might refuse to write.
    let hostile = [
        ("xx/evil", "../evil"),
        ("xetc/passwd", "/etc/passwd"),
        ("objects/XX/image.json", "objects/../image.json"),
        ("image.jsonX", "image.json/"),
        ("CXXwindows", "C:\\windows"),
        ("mods/x.jar", "mods/x.jar"),
        ("IMAGE.JSON", "IMAGE.JSON"),
    ];
    for (i, (placeholder, name)) in hostile.iter().enumerate() {
        let file = dir.path().join(format!("names-{i}.zip"));
        let mut with = entries.clone();
        with.push((placeholder.to_string(), b"x".to_vec()));
        write_entries(&file, &with);
        if placeholder != name {
            patch(&file, placeholder, name);
        }
        let error = open_error(&file);
        assert!(
            error.contains("unexpected") || error.contains("inconsistent"),
            "{name}: {error}"
        );
    }
    // Folders other than the store's.
    let file = dir.path().join("folder.zip");
    let mut with = entries.clone();
    with.push(("mods/".to_string(), Vec::new()));
    write_entries(&file, &with);
    assert!(open_error(&file).contains("unexpected"));
    // An object name that isn't a hash.
    let file = dir.path().join("short.zip");
    let mut with = entries.clone();
    with.push(("objects/abc".to_string(), b"x".to_vec()));
    write_entries(&file, &with);
    assert!(open_error(&file).contains("unexpected"));
}

#[test]
fn duplicate_names_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut entries) = small_export();
    // A second copy of an object under a name one letter off, then patched to the same
    // name: some tools would read one copy and some the other.
    let real = sha256_hex(b"aaaa");
    let mut twin = real.clone();
    twin.replace_range(63..64, if real.ends_with('0') { "1" } else { "0" });
    entries.push((format!("objects/{twin}"), b"AAAA".to_vec()));
    let file = dir.path().join("dup.zip");
    write_entries(&file, &entries);
    patch(&file, &twin, &real);
    let error = open_error(&file);
    assert!(
        error.contains("twice") || error.contains("inconsistent"),
        "{error}"
    );
}

#[test]
fn objects_must_be_exactly_what_the_image_needs() {
    let dir = tempfile::tempdir().unwrap();
    let (_, entries) = small_export();
    // Missing.
    let file = dir.path().join("missing.zip");
    write_entries(&file, &entries[..2]);
    assert!(open_error(&file).contains("missing"));
    // An extra object nobody uses.
    let file = dir.path().join("extra.zip");
    let mut with = entries.clone();
    with.push((
        format!("objects/{}", sha256_hex(b"extra")),
        b"extra".to_vec(),
    ));
    write_entries(&file, &with);
    assert!(open_error(&file).contains("isn't used"));
    // A chunk nobody uses.
    let file = dir.path().join("chunk.zip");
    let mut with = entries.clone();
    with.push((format!("chunks/{}", sha256_hex(b"c")), b"c".to_vec()));
    write_entries(&file, &with);
    assert!(open_error(&file).contains("isn't used"));
    // An object of another size than the image says.
    let file = dir.path().join("size.zip");
    let mut with = entries.clone();
    with[1].1 = b"aaaaa".to_vec();
    write_entries(&file, &with);
    assert!(open_error(&file).contains("size"));
    // No image at all.
    let file = dir.path().join("noimage.zip");
    write_entries(&file, &entries[1..]);
    assert!(open_error(&file).contains("no image.json"));
    // Not a zip.
    let file = dir.path().join("garbage.zip");
    fs::write(&file, b"PK\x03\x04 not really").unwrap();
    assert!(open_error(&file).contains("isn't a zip"));
}

#[test]
fn wrong_bytes_never_reach_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut entries) = small_export();
    // Right size, wrong contents.
    entries[1].1 = b"AAAA".to_vec();
    let file = dir.path().join("wrong.zip");
    write_entries(&file, &entries);
    let mut container = open(&file, &Limits::default()).unwrap();
    let (_d, store) = store(Encoding::Raw);
    let error = container
        .import_objects(&store, &no_check, NEVER)
        .unwrap_err();
    assert!(error.to_string().contains("doesn't match"), "{error}");
    assert!(!store.has_object(&sha256_hex(b"aaaa"), 4));
    assert!(!store.has_object(&sha256_hex(b"AAAA"), 4));
}

#[test]
fn tampered_manifests_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (image, mut entries) = small_export();
    let mut json: serde_json::Value = serde_json::from_slice(&entries[0].1).unwrap();
    json["entries"][0]["mode"] = 0o755.into();
    entries[0].1 = serde_json::to_vec(&json).unwrap();
    let file = dir.path().join("tampered.zip");
    write_entries(&file, &entries);
    assert!(matches!(
        open(&file, &Limits::default()),
        Err(ContainerError::Identity(_))
    ));
    // A linked manifest the image doesn't link to.
    let (_, mut entries) = small_export();
    let mut other = image.clone();
    other.label = Some("x".into());
    other.entries.pop();
    other.seal();
    entries.push((format!("images/{}.json", other.id), other.to_json_pretty()));
    let file = dir.path().join("unlinked.zip");
    write_entries(&file, &entries);
    assert!(open_error(&file).contains("doesn't link"));
}

#[test]
fn limits_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let (_, entries) = small_export();
    let file = dir.path().join("ok.zip");
    write_entries(&file, &entries);
    let image_len = entries[0].1.len() as u64;
    let refused = [
        Limits {
            max_entries: 2,
            ..Limits::default()
        },
        Limits {
            max_total_bytes: image_len + 5,
            ..Limits::default()
        },
        Limits {
            max_manifest_bytes: image_len - 1,
            ..Limits::default()
        },
    ];
    for limits in refused {
        assert!(open(&file, &limits).is_err(), "{limits:?}");
    }
    assert!(open(
        &file,
        &Limits {
            max_entries: 3,
            ..Limits::default()
        }
    )
    .is_ok());
}

#[test]
fn chunked_objects_are_checked_chunk_by_chunk() {
    let (dir, store) = store(Encoding::Raw);
    let fixture = fixture(&store);
    let file = dir.path().join("world.zip");
    export(
        &store,
        &fixture.data,
        std::slice::from_ref(&fixture.content),
        &file,
        NEVER,
    )
    .unwrap();
    // Read every entry back to rebuild altered copies.
    let mut archive = zip::ZipArchive::new(File::open(&file).unwrap()).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.push((entry.name().unwrap().into_owned(), bytes));
    }
    let first_chunk = entries
        .iter()
        .position(|(n, _)| n.starts_with("chunks/"))
        .unwrap();

    // A chunk with other bytes of the same size.
    let mut bad = entries.clone();
    bad[first_chunk].1[0] ^= 1;
    let altered = dir.path().join("bad-chunk.zip");
    write_entries(&altered, &bad);
    let mut container = open(&altered, &Limits::default()).unwrap();
    let (_d, target) = crate::store::tests::store(Encoding::Raw);
    let error = container
        .import_objects(&target, &no_check, NEVER)
        .unwrap_err();
    assert!(error.to_string().contains("doesn't match"), "{error}");
    let region = &fixture.data.entries[2];
    assert_eq!(region.path.as_str(), "saves/world/region/r.0.0.mca");
    assert!(!target.has_object(&region.sha256, region.size));

    // A chunk missing.
    let mut missing = entries.clone();
    missing.remove(first_chunk);
    let altered = dir.path().join("missing-chunk.zip");
    write_entries(&altered, &missing);
    assert!(open_error(&altered).contains("missing"));

    // A chunk of another size than its list says.
    let mut resized = entries.clone();
    resized[first_chunk].1.push(0);
    let altered = dir.path().join("resized-chunk.zip");
    write_entries(&altered, &resized);
    assert!(open_error(&altered).contains("size"));

    // A list that isn't one.
    let list = entries
        .iter()
        .position(|(n, _)| *n == format!("objects/{}", region.sha256))
        .unwrap();
    let mut broken = entries.clone();
    let shorter = broken[list].1.len() - 3;
    broken[list].1.truncate(shorter);
    let altered = dir.path().join("broken-list.zip");
    write_entries(&altered, &broken);
    assert!(open_error(&altered).contains(&region.sha256));
}

#[test]
fn the_free_space_hook_can_refuse() {
    let (dir, store) = store(Encoding::Raw);
    let fixture = fixture(&store);
    let file = dir.path().join("world.zip");
    export(
        &store,
        &fixture.data,
        std::slice::from_ref(&fixture.content),
        &file,
        NEVER,
    )
    .unwrap();
    let mut container = open(&file, &Limits::default()).unwrap();
    let asked = std::sync::Mutex::new(0);
    let (_d, target) = crate::store::tests::store(Encoding::Raw);
    let error = container
        .import_objects(
            &target,
            &|bytes| {
                *asked.lock().unwrap() = bytes;
                Err("only 1 MB free".to_string())
            },
            NEVER,
        )
        .unwrap_err();
    assert!(matches!(error, ContainerError::NoRoom(_)));
    assert_eq!(*asked.lock().unwrap(), container.unpacked_bytes());
    assert!(!target.root().join("objects").exists());
}

/// A large file the core stored whole travels as its chunk list, cut as any store cuts
/// it, so the same image gives the same export from a store that keeps it whole and one
/// that keeps it chunked; and the export imports and places as usual.
#[test]
fn a_whole_large_object_exports_as_its_chunk_list() {
    let big = region_like(31, 5 << 20);
    let sha256 = sha256_hex(&big);
    let mut image = Image::new(Kind::Content, fabric());
    image.entries.push(Entry::new(
        path("mods/big.jar"),
        big.len() as u64,
        sha256.clone(),
        Source::Object {},
    ));
    image.seal();

    let (dir, chunked) = store(Encoding::Raw);
    chunked.put_bytes(&big).unwrap();
    let (_whole_dir, whole) = store(Encoding::Raw);
    // As the core's backups wrote it (raw here, for a store without zstd).
    let object = whole.object_path(&sha256).unwrap();
    fs::create_dir_all(object.parent().unwrap()).unwrap();
    fs::write(&object, &big).unwrap();
    assert_eq!(
        whole.object_form(&sha256, big.len() as u64).unwrap(),
        ObjectForm::Whole
    );

    let from_chunked = dir.path().join("chunked.zip");
    let from_whole = dir.path().join("whole.zip");
    let a = export(&chunked, &image, &[], &from_chunked, NEVER).unwrap();
    let b = export(&whole, &image, &[], &from_whole, NEVER).unwrap();
    assert_eq!(a, b);
    assert!(b.chunks > 1);
    assert_eq!(
        fs::read(&from_chunked).unwrap(),
        fs::read(&from_whole).unwrap()
    );

    let mut container = open(&from_whole, &Limits::default()).unwrap();
    let (_t, target) = store(Encoding::Raw);
    container.import_objects(&target, &no_check, NEVER).unwrap();
    assert_eq!(
        target.read_object_bytes(&sha256, big.len() as u64).unwrap(),
        big
    );
}

/// Entries can share one object, and a list can repeat one chunk, so a tiny export can
/// name an image whose files add up to far more than the zip holds. Such an image is
/// refused when the export is opened, by its files' sizes.
#[test]
fn an_image_whose_files_add_up_to_too_much_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let chunk = vec![7u8; 256 * 1024];
    let chunk_sha = sha256_hex(&chunk);
    let repeats = 64u64;
    let size = repeats * chunk.len() as u64;
    let list = ChunkList {
        size,
        chunks: (0..repeats)
            .map(|_| ChunkRef {
                sha256: chunk_sha.clone(),
                size: chunk.len() as u32,
            })
            .collect(),
    };
    let file_sha = {
        let mut hasher = Sha256::new();
        for _ in 0..repeats {
            hasher.update(&chunk);
        }
        hex_digest(hasher)
    };
    let mut image = Image::new(Kind::Data, fabric());
    for i in 0..100 {
        image.entries.push(Entry::new(
            path(&format!("data/copy-{i:03}.bin")),
            size,
            file_sha.clone(),
            Source::Object {},
        ));
    }
    image.seal();
    assert_eq!(files_size(&image), 100 * size);
    let entries = vec![
        (IMAGE_FILE.to_string(), image.to_json_pretty()),
        (format!("objects/{file_sha}"), list.encode()),
        (format!("chunks/{chunk_sha}"), chunk.clone()),
    ];
    let file = dir.path().join("bomb.zip");
    write_entries(&file, &entries);
    assert!(fs::metadata(&file).unwrap().len() < 100_000);
    let limits = Limits {
        max_total_bytes: 50 * size,
        ..Limits::default()
    };
    let error = open(&file, &limits).unwrap_err().to_string();
    assert!(error.contains("adding up"), "{error}");
    // Under a limit it fits, it opens, and says what placing it writes.
    let container = open(&file, &Limits::default()).unwrap();
    assert_eq!(container.image_bytes(), 100 * size);
    assert!(container.unpacked_bytes() < size);
}
