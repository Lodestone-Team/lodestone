use super::*;
use crate::layout::ServerLayout;
use crate::manifest::tests::{fabric, path};
use crate::manifest::{Kind, Source};
use crate::store::cdc::tests::region_like;
use crate::store::snapshot::{snapshot, SnapshotOptions};
use crate::store::tests::{encodings, store, stored_image, write};

/// Snapshots a small client instance into a sealed image.
fn client_image(store: &Store, root: &Path) -> Image {
    write(root, "minecraft/mods/sodium.jar", b"sodium");
    write(root, "minecraft/options.txt", b"fov:90");
    write(root, "minecraft/saves/World/level.dat", b"level");
    write(
        root,
        "minecraft/saves/World/region/r.0.0.mca",
        &region_like(1, 2_000_000),
    );
    write(root, "minecraft/saves/Other/level.dat", b"other");
    let snapshot = snapshot(
        store,
        root,
        &DiskLayout::Client,
        &SnapshotOptions::default(),
    )
    .unwrap();
    let mut image = Image::new(Kind::Full, fabric());
    snapshot.fill(&mut image);
    image.seal();
    image
}

fn files_under(root: &Path) -> Vec<String> {
    let mut files: Vec<String> = walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    files.sort();
    files
}

#[test]
fn an_image_comes_back_byte_for_byte() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let source = dir.path().join("instance");
        let image = client_image(&store, &source);
        let staging = dir.path().join("staging");
        fs::create_dir(&staging).unwrap();
        let report = materialize(
            &store,
            &image,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions::default(),
        )
        .unwrap();
        assert_eq!(report.files, 5);
        assert!(report.not_placed.is_empty());
        assert_eq!(files_under(&staging), files_under(&source));
        for name in files_under(&source) {
            let from = source.join(&name);
            let to = staging.join(&name);
            assert_eq!(fs::read(&from).unwrap(), fs::read(&to).unwrap(), "{name}");
            let (a, b) = (fs::metadata(&from).unwrap(), fs::metadata(&to).unwrap());
            assert_eq!(a.modified().unwrap(), b.modified().unwrap(), "{name}");
            assert_eq!(fsx::mode_of(&a), fsx::mode_of(&b), "{name}");
            assert!(
                !b.permissions().readonly(),
                "placed files are writable like their source"
            );
        }
    }
}

#[test]
fn a_selection_places_some_files() {
    let (dir, store) = store(Encoding::Raw);
    let image = client_image(&store, &dir.path().join("instance"));
    let worlds = vec!["World".to_string()];
    let cases: Vec<(Selection, Vec<&str>)> = vec![
        (
            Selection::Worlds(&worlds),
            vec![
                "minecraft/saves/World/level.dat",
                "minecraft/saves/World/region/r.0.0.mca",
            ],
        ),
        (
            Selection::Content,
            vec!["minecraft/mods/sodium.jar", "minecraft/options.txt"],
        ),
    ];
    for (i, (selection, expected)) in cases.into_iter().enumerate() {
        let staging = dir.path().join(format!("staging-{i}"));
        fs::create_dir(&staging).unwrap();
        let options = MaterializeOptions {
            selection,
            ..MaterializeOptions::default()
        };
        materialize(&store, &image, &DiskLayout::Client, &staging, &options).unwrap();
        assert_eq!(files_under(&staging), expected);
    }
    let only_mods = |e: &Entry| e.path.first() == "mods";
    let staging = dir.path().join("staging-mods");
    fs::create_dir(&staging).unwrap();
    let options = MaterializeOptions {
        selection: Selection::Matching(&only_mods),
        durable: false,
        ..MaterializeOptions::default()
    };
    materialize(&store, &image, &DiskLayout::Client, &staging, &options).unwrap();
    assert_eq!(files_under(&staging), vec!["minecraft/mods/sodium.jar"]);
}

#[test]
fn a_server_gets_its_layout_and_skips_client_files() {
    let (dir, store) = store(Encoding::Raw);
    let image = client_image(&store, &dir.path().join("instance"));
    let layout =
        DiskLayout::Server(ServerLayout::new("World", ["Other".to_string()], true).unwrap());
    let staging = dir.path().join("server");
    fs::create_dir(&staging).unwrap();
    let report = materialize(
        &store,
        &image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    assert_eq!(report.not_placed, vec![path("options.txt")]);
    assert_eq!(
        files_under(&staging),
        vec![
            "Other/level.dat",
            "World/level.dat",
            "World/region/r.0.0.mca",
            "mods/sodium.jar",
        ]
    );
    // A server that doesn't know a world can't place it.
    let narrow = DiskLayout::Server(ServerLayout::new("World", [], false).unwrap());
    let staging = dir.path().join("narrow");
    fs::create_dir(&staging).unwrap();
    assert!(matches!(
        materialize(
            &store,
            &image,
            &narrow,
            &staging,
            &MaterializeOptions::default()
        ),
        Err(MaterializeError::Layout { .. })
    ));
}

#[test]
fn a_file_named_like_a_world_is_refused_before_anything_is_placed() {
    // `world` the file and `saves/world/…` the world are one name on this server's disk.
    let (dir, store) = store(Encoding::Raw);
    let mut image = stored_image(
        &store,
        &[
            ("mods/a.jar", b"a"),
            ("world", b"file"),
            ("saves/world/level.dat", b"level"),
        ],
    );
    image.worlds = vec![crate::manifest::World {
        name: "world".into(),
        level_name: None,
        data_version: None,
        game_version: None,
        size: None,
        last_played: None,
        quiesced: None,
    }];
    image.seal();
    image.validate().unwrap();
    let layout = DiskLayout::Server(ServerLayout::new("world", [], false).unwrap());
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let error = materialize(
        &store,
        &image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, MaterializeError::Layout { path: p, error: PathError::LayoutConflict(_) } if p.as_str() == "world"),
        "{error}"
    );
    assert!(files_under(&staging).is_empty());
}

#[test]
fn missing_objects_place_nothing() {
    let (dir, store) = store(Encoding::Raw);
    let mut image = stored_image(&store, &[("mods/a.jar", b"a")]);
    image.entries.push(Entry::new(
        path("mods/b.jar"),
        1,
        crate::hex::sha256_hex(b"b"),
        Source::Object {},
    ));
    image.seal();
    assert_eq!(
        missing_objects(&store, &image, Selection::All)
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        vec!["mods/b.jar"]
    );
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let error = materialize(
        &store,
        &image,
        &DiskLayout::Client,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&error, MaterializeError::Missing(paths) if paths == &vec![path("mods/b.jar")])
    );
    assert!(files_under(&staging).is_empty());
}

#[test]
fn damaged_objects_fail_and_leave_no_partial_file() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let big = region_like(2, 1_500_000);
        let image = stored_image(&store, &[("mods/a.jar", b"aaaa"), ("saves/w/r.mca", &big)]);
        let mut image = image;
        image.worlds.push(crate::manifest::World {
            name: "w".into(),
            level_name: None,
            data_version: None,
            game_version: None,
            size: None,
            last_played: None,
            quiesced: None,
        });
        image.seal();
        // Damage one chunk of the large file.
        let ObjectForm::Chunked(list) = store
            .object_form(&image.entries[1].sha256, big.len() as u64)
            .unwrap()
        else {
            panic!("chunked");
        };
        let chunk = store.chunk_path(&list.chunks[2].sha256).unwrap();
        let mut bytes = store.read_chunk(&list.chunks[2]).unwrap();
        bytes[10] ^= 1;
        fsx::remove_file_forced(&chunk).unwrap();
        let file = File::create(&chunk).unwrap();
        crate::store::encoding::encode_into(encoding, file, &bytes).unwrap();
        let staging = dir.path().join("staging");
        fs::create_dir(&staging).unwrap();
        let error = materialize(
            &store,
            &image,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions::default(),
        )
        .unwrap_err();
        assert!(
            matches!(&error, MaterializeError::Damaged { path: p } if p.as_str() == "saves/w/r.mca"),
            "{error}"
        );
        assert_eq!(files_under(&staging), vec!["minecraft/mods/a.jar"]);

        // A whole object with the right length and wrong bytes, too (raw clones are
        // checked like copies).
        let object = store.object_path(&image.entries[0].sha256).unwrap();
        fsx::remove_file_forced(&object).unwrap();
        let file = File::create(&object).unwrap();
        crate::store::encoding::encode_into(encoding, file, b"bbbb").unwrap();
        let staging = dir.path().join("staging-2");
        fs::create_dir(&staging).unwrap();
        let only_a = |e: &Entry| e.path.as_str() == "mods/a.jar";
        let options = MaterializeOptions {
            selection: Selection::Matching(&only_a),
            ..MaterializeOptions::default()
        };
        assert!(matches!(
            materialize(&store, &image, &DiskLayout::Client, &staging, &options),
            Err(MaterializeError::Damaged { .. })
        ));
        assert!(files_under(&staging).is_empty());
    }
}

#[test]
fn files_already_in_staging_are_never_overwritten() {
    let (dir, store) = store(Encoding::Raw);
    let image = stored_image(&store, &[("mods/a.jar", b"new")]);
    let staging = dir.path().join("staging");
    write(&staging, "minecraft/mods/a.jar", b"mine");
    assert!(matches!(
        materialize(
            &store,
            &image,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions::default()
        ),
        Err(MaterializeError::Exists(_))
    ));
    assert_eq!(
        fs::read(staging.join("minecraft/mods/a.jar")).unwrap(),
        b"mine"
    );
    assert!(matches!(
        materialize(
            &store,
            &image,
            &DiskLayout::Client,
            &dir.path().join("none"),
            &MaterializeOptions::default()
        ),
        Err(MaterializeError::NoStaging(_))
    ));
}

#[test]
fn names_differing_in_case_are_refused_where_the_disk_ignores_case() {
    let (dir, store) = store(Encoding::Raw);
    let image = stored_image(
        &store,
        &[("mods/A.jar", b"upper"), ("mods/a.jar", b"lower")],
    );
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let result = materialize(
        &store,
        &image,
        &DiskLayout::Client,
        &staging,
        &MaterializeOptions::default(),
    );
    if ignores_case(&staging).unwrap() {
        assert!(matches!(result, Err(MaterializeError::CaseClash(..))));
        assert!(files_under(&staging).is_empty());
    } else {
        result.unwrap();
        assert_eq!(files_under(&staging).len(), 2);
    }
    // The probe leaves nothing behind.
    assert!(fs::read_dir(&staging)
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().contains("case-probe")));
}

#[test]
fn deep_names_differing_in_case_are_found_too() {
    // On a client's disk these paths are longer than a canonical path may be: they are
    // still checked (they used to be left out of the check).
    let (dir, store) = store(Encoding::Raw);
    let folder = "f".repeat(250);
    let deep = format!(
        "mods/{}/{}",
        vec![folder.as_str(); 16].join("/"),
        "g".repeat(70)
    );
    assert!(deep.len() + 2 <= 4096 && deep.len() + 2 + "minecraft/".len() > 4096);
    let upper = format!("{deep}/A");
    let lower = format!("{deep}/a");
    let image = stored_image(
        &store,
        &[(upper.as_str(), b"upper"), (lower.as_str(), b"lower")],
    );
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    if ignores_case(&staging).unwrap() {
        let result = materialize(
            &store,
            &image,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions::default(),
        );
        assert!(
            matches!(&result, Err(MaterializeError::CaseClash(a, b)) if a.as_str() == upper && b.as_str() == lower),
            "{result:?}"
        );
        assert!(files_under(&staging).is_empty());
    }
}

#[cfg(unix)]
#[test]
fn modes_are_applied() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store) = store(Encoding::Raw);
    let mut image = stored_image(&store, &[("start.sh", b"#!/bin/sh"), ("ro.txt", b"ro")]);
    image.entries[0].mode = 0o444;
    image.entries[1].mode = 0o4755;
    image.entries.push({
        let mut entry = image.entries[0].clone();
        entry.path = CanonicalPath::new("unreadable.txt").unwrap();
        entry.mode = 0;
        entry
    });
    image.seal();
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let layout = DiskLayout::Server(ServerLayout::new("world", [], false).unwrap());
    materialize(
        &store,
        &image,
        &layout,
        &staging,
        &MaterializeOptions::default(),
    )
    .unwrap();
    let mode = |name: &str| {
        fs::metadata(staging.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    };
    // Entries are sorted: ro.txt, start.sh. A file recorded read-only comes back
    // writable by its owner, as both old restores did.
    assert_eq!(mode("ro.txt"), 0o644);
    // Setuid never comes from an image.
    assert_eq!(mode("start.sh"), 0o755);
    // Even mode 0 is owner-writable (and the v1 snapshot reader never gives 0).
    assert_eq!(mode("unreadable.txt"), 0o200);
    assert_eq!(placed_mode(0o4755), 0o755);
    assert_eq!(placed_mode(0o444), 0o644);
}
