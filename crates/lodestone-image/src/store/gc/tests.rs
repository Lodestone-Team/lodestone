use super::*;
use crate::hex::sha256_hex;
use crate::manifest::tests::{fabric, path};
use crate::manifest::{Entry, Image, Kind, Source};
use crate::settings::{ClientSettings, StoredFile};
use crate::store::cdc::tests::region_like;
use crate::store::tests::{age, count, encodings, store, stored_image};
use crate::store::{Encoding, ObjectForm, StoreError};
use std::fs::File;

/// Collection without the grace period, for tests about what is used.
fn now_options() -> GcOptions {
    allow_short_grace();
    GcOptions {
        grace: Duration::ZERO,
        ..GcOptions::default()
    }
}

/// Ages every object and chunk, as if written long ago.
fn age_all(store: &Store) {
    for area in Area::ALL {
        for entry in walkdir::WalkDir::new(store.area_dir(area))
            .min_depth(2)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                age(entry.path(), 7200);
            }
        }
    }
}

#[test]
fn unused_objects_and_chunks_go_and_used_ones_stay() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let shared = region_like(1, 2_000_000);
        let only_old = region_like(2, 2_000_000);
        let kept = stored_image(&store, &[("saves/w/a.mca", &shared), ("mods/a.jar", b"a")]);
        let dropped = stored_image(
            &store,
            &[("saves/w/b.mca", &only_old), ("mods/b.jar", b"b")],
        );
        let fix_worlds = |mut image: Image| {
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
            image
        };
        let kept = fix_worlds(kept);
        let dropped = fix_worlds(dropped);
        store.save_manifest("one", &kept).unwrap();
        store.save_manifest("two", &dropped).unwrap();
        age_all(&store);

        let report = store.gc(&now_options()).unwrap();
        assert_eq!(report.manifests, 2);
        assert_eq!((report.objects_removed, report.chunks_removed), (0, 0));

        store.delete_manifest("two", &dropped.id).unwrap();
        let chunks_before = count(&store, Area::Chunks);
        let report = store.gc(&now_options()).unwrap();
        assert_eq!(report.objects_removed, 2, "{report:?}");
        assert!(report.chunks_removed > 0);
        assert!(report.bytes_removed > 1_000_000);
        assert_eq!(
            count(&store, Area::Chunks),
            chunks_before - report.chunks_removed as usize
        );
        assert!(report.chunks_skipped.is_none());
        // What the kept image uses reads back whole.
        for entry in &kept.entries {
            store
                .verify_object(&entry.sha256, entry.size, crate::store::NEVER)
                .unwrap();
        }
        assert!(!store.has_object(&sha256_hex(b"b"), 1));
        assert!(!store.has_object(&sha256_hex(&only_old), only_old.len() as u64));
    }
}

#[test]
fn an_unreadable_or_newer_manifest_stops_everything() {
    let (_dir, store) = store(Encoding::Raw);
    let image = stored_image(&store, &[("mods/a.jar", b"a")]);
    store.save_manifest("one", &image).unwrap();
    store.put_bytes(b"garbage nobody uses").unwrap();
    age_all(&store);
    let dir = store.instance_dir("one").unwrap();

    let mut newer: serde_json::Value = serde_json::from_slice(&image.to_json_pretty()).unwrap();
    newer["schema"] = 3.into();
    let mut older = newer.clone();
    older["schema"] = 1.into();
    let mut no_schema = newer.clone();
    no_schema.as_object_mut().unwrap().remove("schema");
    let mut bad_entry =
        serde_json::from_slice::<serde_json::Value>(&image.to_json_pretty()).unwrap();
    bad_entry["entries"][0]["size"] = "big".into();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("newer", serde_json::to_vec(&newer).unwrap()),
        ("older", serde_json::to_vec(&older).unwrap()),
        ("no schema", serde_json::to_vec(&no_schema).unwrap()),
        ("bad entry", serde_json::to_vec(&bad_entry).unwrap()),
        ("cut short", image.to_json_pretty()[..40].to_vec()),
        ("not json", b"\xff\xfe".to_vec()),
    ];
    for (what, bytes) in cases {
        let name = dir.join(format!("{}.json", sha256_hex(what.as_bytes())));
        fs::write(&name, bytes).unwrap();
        let error = store.gc(&now_options()).unwrap_err();
        assert!(matches!(error, GcError::Refused { .. }), "{what}: {error}");
        if what == "newer" {
            assert!(error.to_string().contains("newer"));
        }
        assert!(
            store.has_object(&sha256_hex(b"garbage nobody uses"), 19),
            "{what}"
        );
        fs::remove_file(&name).unwrap();
    }
    // Files that aren't manifests by name don't count.
    fs::write(dir.join("notes.json"), b"not a manifest").unwrap();
    fs::write(dir.join("stat-cache.json"), b"{}").unwrap();
    let report = store.gc(&now_options()).unwrap();
    assert_eq!(report.objects_removed, 1);
    assert!(store.has_object(&sha256_hex(b"a"), 1));
}

#[test]
fn new_files_are_kept_for_writers_in_progress() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        // A snapshot in progress: objects stored, manifest not written yet.
        let big = region_like(3, 3_000_000);
        let pending_big = store.put_bytes(&big).unwrap();
        let pending_small = store.put_bytes(b"small").unwrap();
        let report = store.gc(&GcOptions::default()).unwrap();
        assert_eq!((report.objects_removed, report.chunks_removed), (0, 0));
        assert!(report.kept_new > 0);
        assert!(store
            .verify_object(&pending_big.sha256, big.len() as u64, crate::store::NEVER)
            .is_ok());

        // An old object nobody uses is reused by a new snapshot: storing it touches it, so
        // it is kept until the manifest is written.
        let old = store.put_bytes(b"old but reused").unwrap();
        age_all(&store);
        store.put_bytes(b"old but reused").unwrap();
        let report = store.gc(&GcOptions::default()).unwrap();
        assert!(store.has_object(&old.sha256, 14), "{report:?}");
        // The big file's object and chunks were aged and nothing names them: gone.
        assert!(!store.has_object(&pending_big.sha256, big.len() as u64));
        assert!(!store.has_object(&pending_small.sha256, 5));
    }
}

#[test]
fn a_new_chunk_list_keeps_its_old_chunks() {
    let (_dir, store) = store(Encoding::Raw);
    let big = region_like(4, 2_500_000);
    let stored = store.put_bytes(&big).unwrap();
    age_all(&store);
    // Its chunks are old; the list itself is new again (reused by a writer).
    let list = store.list_path(&stored.sha256).unwrap();
    crate::store::fsx::touch(&list).unwrap();
    let report = store.gc(&GcOptions::default()).unwrap();
    assert_eq!(
        (report.objects_removed, report.chunks_removed),
        (0, 0),
        "{report:?}"
    );
    assert_eq!(
        store
            .read_object_bytes(&stored.sha256, big.len() as u64)
            .unwrap(),
        big
    );
}

#[test]
fn a_damaged_live_list_keeps_every_chunk() {
    let (_dir, store) = store(Encoding::Raw);
    let big = region_like(5, 2_000_000);
    let image = stored_image(&store, &[("mods/big.jar", &big)]);
    store.save_manifest("one", &image).unwrap();
    let orphan = region_like(6, 2_000_000);
    store.put_bytes(&orphan).unwrap();
    let orphan_object = store.object_path(&sha256_hex(&orphan)).unwrap();
    age_all(&store);
    // The list of the live file loses its end.
    let list = store.list_path(&image.entries[0].sha256).unwrap();
    let bytes = fs::read(&list).unwrap();
    crate::store::fsx::remove_file_forced(&list).unwrap();
    fs::write(&list, &bytes[..bytes.len() - 10]).unwrap();
    age(&list, 7200);
    let chunks_before = count(&store, Area::Chunks);
    let report = store.gc(&now_options()).unwrap();
    assert!(report.chunks_skipped.is_some());
    assert_eq!(report.chunks_removed, 0);
    assert_eq!(count(&store, Area::Chunks), chunks_before);
    // Unused objects still go.
    assert_eq!(report.objects_removed, 1);
    assert!(!orphan_object.exists());
}

#[test]
fn settings_files_and_older_formats_are_live() {
    let (_dir, store) = store(Encoding::Raw);
    let icon = store.put_bytes(b"png").unwrap();
    let legacy = store.put_bytes(b"used by a schema 1 backup").unwrap();
    let mut image = Image::new(Kind::Content, fabric());
    image.settings.client = Some(ClientSettings {
        icon: Some(StoredFile {
            sha256: icon.sha256.clone(),
            size: 3,
        }),
        ..ClientSettings::default()
    });
    image.entries.push(Entry::new(
        path("mods/a.jar"),
        1,
        store.put_bytes(b"a").unwrap().sha256,
        // Not carried in exports, but the store has its bytes: still live here.
        Source::Curseforge {
            project: 1,
            file: 2,
            downloadable: true,
        },
    ));
    image.seal();
    store.save_manifest("one", &image).unwrap();
    age_all(&store);
    let mut options = now_options();
    options.also_live.insert(legacy.sha256.clone());
    let report = store.gc(&options).unwrap();
    assert_eq!(report.objects_removed, 0, "{report:?}");
    options.also_live.clear();
    let report = store.gc(&options).unwrap();
    assert_eq!(report.objects_removed, 1);
    assert!(store.has_object(&icon.sha256, 3));
    assert!(!store.has_object(&legacy.sha256, 25));
}

#[test]
fn only_store_names_are_ever_deleted() {
    let (_dir, store) = store(Encoding::Raw);
    store.put_bytes(b"x").unwrap();
    let objects = store.area_dir(Area::Objects);
    let stray = [
        objects.join("README"),
        objects.join("ab").join("not-a-hash"),
        objects.join("zz").join(sha256_hex(b"wrong folder")),
    ];
    for file in &stray {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"keep me").unwrap();
        age(file, 7200);
    }
    age_all(&store);
    let report = store.gc(&now_options()).unwrap();
    assert_eq!(report.objects_removed, 1);
    for file in &stray {
        assert!(file.exists(), "{}", file.display());
    }
}

#[test]
fn old_temp_files_go_young_ones_stay() {
    let (_dir, store) = store(Encoding::Raw);
    store.put_bytes(b"x").unwrap();
    let objects = store.area_dir(Area::Objects);
    let (_, old) = crate::store::fsx::create_temp(&objects).unwrap();
    let (_, young) = crate::store::fsx::create_temp(&objects).unwrap();
    age(&old, 7200);
    let report = store.gc(&GcOptions::default()).unwrap();
    assert_eq!(report.temp_removed, 1);
    assert!(!old.exists());
    assert!(young.exists());
}

#[test]
fn an_object_whose_form_changed_is_still_marked_by_size() {
    // A large file the core stored whole: its manifest names it with its size, and the
    // object is whole, so no chunk list is looked for.
    let (_dir, store) = store(Encoding::Raw);
    let big = region_like(7, 1_500_000);
    let sha256 = sha256_hex(&big);
    let path_on_disk = store.object_path(&sha256).unwrap();
    fs::create_dir_all(path_on_disk.parent().unwrap()).unwrap();
    fs::write(&path_on_disk, &big).unwrap();
    let mut image = Image::new(Kind::Data, fabric());
    image.entries.push(Entry::new(
        path("mods/big.jar"),
        big.len() as u64,
        sha256.clone(),
        Source::Object {},
    ));
    image.seal();
    store.save_manifest("one", &image).unwrap();
    age_all(&store);
    let report = store.gc(&now_options()).unwrap();
    assert_eq!(report.objects_removed, 0);
    assert!(report.chunks_skipped.is_none());
    assert_eq!(
        store.object_form(&sha256, big.len() as u64).unwrap(),
        ObjectForm::Whole
    );
    assert!(!matches!(
        store.read_object_bytes(&sha256, big.len() as u64),
        Err(StoreError::Damaged(_))
    ));
}

/// An image that names a large file's object with the wrong size, as a manifest received
/// from elsewhere may (any size validates, and a Modrinth or CurseForge entry needs no
/// object of its own).
fn misnamed(sha256: &str, size: u64) -> Image {
    let mut image = Image::new(Kind::Content, fabric());
    image.entries.push(Entry::new(
        path("mods/same.jar"),
        size,
        sha256.to_string(),
        Source::Curseforge {
            project: 1,
            file: 2,
            downloadable: true,
        },
    ));
    image.seal();
    image
}

/// Whether an object is a chunk list is decided from the object, never from whichever
/// manifest is read first: a manifest naming a list with the list's own length (which
/// anyone with the same file can work out) must not make its chunks garbage, in either
/// order of the instance folders.
#[test]
fn a_list_named_with_its_own_length_keeps_its_chunks() {
    for (good_instance, bad_instance) in [("aa", "zz"), ("zz", "aa")] {
        let (_dir, store) = store(Encoding::Raw);
        let big = region_like(8, 2_500_000);
        let image = stored_image(&store, &[("mods/big.jar", &big)]);
        store.save_manifest(good_instance, &image).unwrap();
        let sha256 = image.entries[0].sha256.clone();
        let list_len = fs::metadata(store.list_path(&sha256).unwrap())
            .unwrap()
            .len();
        store
            .save_manifest(bad_instance, &misnamed(&sha256, list_len))
            .unwrap();
        age_all(&store);
        let chunks = count(&store, Area::Chunks);
        let report = store.gc(&now_options()).unwrap();
        assert_eq!(report.chunks_removed, 0, "{good_instance}: {report:?}");
        assert_eq!(count(&store, Area::Chunks), chunks);
        assert_eq!(report.wrong_sizes, 1);
        assert_eq!(
            store.read_object_bytes(&sha256, big.len() as u64).unwrap(),
            big
        );
    }
}

/// A manifest naming a live list with a merely wrong size blocks nothing: the list's
/// chunks stay, and other garbage chunks still go.
#[test]
fn a_wrong_size_doesnt_block_chunk_collection() {
    let (_dir, store) = store(Encoding::Raw);
    let big = region_like(9, 2_000_000);
    let image = stored_image(&store, &[("mods/big.jar", &big)]);
    store.save_manifest("one", &image).unwrap();
    let sha256 = image.entries[0].sha256.clone();
    store
        .save_manifest("two", &misnamed(&sha256, 5_000_000))
        .unwrap();
    // A whole file named with a wrong size, too.
    let small = store.put_bytes(b"small").unwrap();
    store
        .save_manifest("three", &misnamed(&small.sha256, 9_000_000))
        .unwrap();
    let garbage = region_like(10, 2_000_000);
    store.put_bytes(&garbage).unwrap();
    age_all(&store);
    let report = store.gc(&now_options()).unwrap();
    assert!(report.chunks_skipped.is_none(), "{report:?}");
    assert!(report.chunks_removed > 0);
    assert_eq!(report.wrong_sizes, 2);
    assert_eq!(
        store.read_object_bytes(&sha256, big.len() as u64).unwrap(),
        big
    );
    assert!(store.has_object(&small.sha256, 5));
}

/// An object only an older format names (no size known) that the store keeps as a chunk
/// list (stored again after its whole copy was damaged and removed) keeps its chunks.
#[test]
fn an_also_live_chunk_list_keeps_its_chunks() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let big = region_like(11, 3 << 20);
        let stored = store.put_bytes(&big).unwrap();
        assert!(matches!(
            store.object_form(&stored.sha256, stored.size).unwrap(),
            ObjectForm::Chunked(_)
        ));
        // A whole file that starts like a list is just a whole file.
        let lookalike = store
            .put_bytes(b"lodestone-chunks 1\nsize 2000000\nnot really")
            .unwrap();
        age_all(&store);
        let mut options = now_options();
        options.also_live.insert(stored.sha256.clone());
        options.also_live.insert(lookalike.sha256.clone());
        let report = store.gc(&options).unwrap();
        assert_eq!(
            (report.objects_removed, report.chunks_removed),
            (0, 0),
            "{report:?}"
        );
        assert!(report.chunks_skipped.is_none());
        assert_eq!(
            store
                .read_object_bytes(&stored.sha256, stored.size)
                .unwrap(),
            big
        );
    }
}

#[test]
fn a_file_touched_while_it_is_collected_is_put_back() {
    let (_dir, store) = store(Encoding::Raw);
    let old = store.put_bytes(b"old and unused").unwrap();
    let kept = store.put_bytes(b"claimed meanwhile").unwrap();
    age_all(&store);
    let old_path = store.object_path(&old.sha256).unwrap();
    let kept_path = store.object_path(&kept.sha256).unwrap();
    let options = GcOptions::default();
    let sweep = Sweep {
        started: SystemTime::now(),
        options: &options,
    };
    // Still old once renamed away: deleted.
    assert_eq!(collect(&old_path, &sweep).unwrap(), Some(14));
    assert!(!old_path.exists());
    // A claim touched it after the collection found it old: put back, whole.
    fsx::touch(&kept_path).unwrap();
    assert_eq!(collect(&kept_path, &sweep).unwrap(), None);
    assert_eq!(
        store.read_object_bytes(&kept.sha256, kept.size).unwrap(),
        b"claimed meanwhile"
    );
    assert!(crate::store::tests::temp_files(store.root()).is_empty());
}

#[test]
fn a_claim_fails_once_a_collection_has_taken_the_file_away() {
    let (_dir, store) = store(Encoding::Raw);
    let stored = store.put_bytes(b"about to be named").unwrap();
    age_all(&store);
    let path = store.object_path(&stored.sha256).unwrap();
    // The collection renamed it away (and hasn't decided yet): a claim now fails, so
    // the manifest that would name it isn't saved.
    let aside = fsx::temp_path(path.parent().unwrap());
    fs::rename(&path, &aside).unwrap();
    assert!(!store.claim_object(&stored.sha256, stored.size));
    fs::rename(&aside, &path).unwrap();
    // Back in place: the claim touches it and holds.
    assert!(store.claim_object(&stored.sha256, stored.size));
    let age = fsx::age(&fs::metadata(&path).unwrap(), SystemTime::now()).unwrap();
    assert!(age < Duration::from_secs(60));
}

#[test]
fn a_short_grace_is_raised_to_the_minimum() {
    // On a thread of its own: tests elsewhere lower the minimum on theirs.
    std::thread::spawn(|| {
        let (_dir, store) = store(Encoding::Raw);
        let recent = store.put_bytes(b"stored fifteen minutes ago").unwrap();
        let old = store.put_bytes(b"stored two hours ago").unwrap();
        age(&store.object_path(&recent.sha256).unwrap(), 900);
        age(&store.object_path(&old.sha256).unwrap(), 7200);
        let report = store
            .gc(&GcOptions {
                grace: Duration::ZERO,
                ..GcOptions::default()
            })
            .unwrap();
        // Fifteen minutes is old enough for a claim to touch it, so a collection with
        // no grace could take what a snapshot is about to name: the minimum keeps it.
        assert_eq!(report.objects_removed, 1);
        assert!(store.has_object(&recent.sha256, recent.size));
        assert!(!store.has_object(&old.sha256, old.size));
    })
    .join()
    .unwrap();
}

#[test]
fn manifests_with_null_lists_mark_and_oversized_ones_refuse() {
    let (_dir, store) = store(Encoding::Raw);
    store.put_bytes(b"garbage").unwrap();
    age_all(&store);
    let dir = store.instance_dir("one").unwrap();
    fs::create_dir_all(&dir).unwrap();
    let name = dir.join(format!("{}.json", sha256_hex(b"null entries")));
    fs::write(
        &name,
        br#"{"schema": 2, "entries": null, "settings": null}"#,
    )
    .unwrap();
    let report = store.gc(&now_options()).unwrap();
    assert_eq!(report.manifests, 1);
    assert_eq!(report.objects_removed, 1);
    // One past the manifest limit isn't read into memory: refused.
    let file = File::create(&name).unwrap();
    file.set_len(MAX_MANIFEST_BYTES + 1).unwrap();
    let error = store.gc(&now_options()).unwrap_err();
    assert!(error.to_string().contains("larger than"), "{error}");
}

/// A manifest saved after marking began isn't marked; its objects are safe because age is
/// judged against when the collection started, not when the sweep reaches them. A save
/// that found an object eight minutes old didn't touch it, and a slow sweep reaching it
/// half an hour later must still keep it.
#[test]
fn age_is_judged_against_the_start_of_the_collection() {
    let (_dir, store) = store(Encoding::Raw);
    let saved = store
        .put_bytes(b"named by a manifest saved during the sweep")
        .unwrap();
    let path = store.object_path(&saved.sha256).unwrap();
    let options = GcOptions {
        grace: MIN_GRACE,
        ..GcOptions::default()
    };
    // Started 30 minutes ago; the object was written 8 minutes before that, so 38 minutes
    // old now: old by the clock at sweep time, new by the start.
    let sweep = Sweep {
        started: SystemTime::now() - Duration::from_secs(30 * 60),
        options: &options,
    };
    age(&path, 38 * 60);
    assert!(sweep.is_new(&fs::metadata(&path).unwrap()));
    assert_eq!(collect(&path, &sweep).unwrap(), None);
    assert!(store.has_object(&saved.sha256, saved.size));
    // Older than the grace before the start: collected.
    age(&path, 51 * 60);
    assert!(!sweep.is_new(&fs::metadata(&path).unwrap()));
    assert!(collect(&path, &sweep).unwrap().is_some());
}

/// Marking keeps each object once, however many manifests name it: memory follows the
/// distinct objects, as backups of one server share most of theirs.
#[test]
fn marking_keeps_each_object_once() {
    let (_dir, store) = store(Encoding::Raw);
    let files: Vec<Vec<u8>> = (0..40).map(|i| format!("file {i}").into_bytes()).collect();
    let names: Vec<String> = (0..40).map(|i| format!("mods/{i}.jar")).collect();
    let pairs: Vec<(&str, &[u8])> = names
        .iter()
        .zip(&files)
        .map(|(name, file)| (name.as_str(), file.as_slice()))
        .collect();
    for label in 0..25 {
        let mut image = stored_image(&store, &pairs);
        // A different file each time, so every manifest is its own.
        let own = store.put_bytes(format!("own {label}").as_bytes()).unwrap();
        image.entries.push(Entry::new(
            path("config/own.txt"),
            own.size,
            own.sha256,
            Source::Object {},
        ));
        image.seal();
        store.save_manifest("one", &image).unwrap();
    }
    let mut sizes = HashMap::new();
    let mut report = GcReport::default();
    store.mark_manifests(&mut sizes, &mut report).unwrap();
    assert_eq!(report.manifests, 25);
    assert_eq!(sizes.len(), 40 + 25);
    assert!(sizes.values().all(|named| named.len() == 1));
}
