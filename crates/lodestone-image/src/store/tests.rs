//! Tests of the store's objects and manifests, shared helpers for the other store tests.

use super::*;
use crate::manifest::tests::{fabric, path};
use crate::manifest::{Entry, Kind, Source};
use crate::store::cdc::tests::{region_like, TestRng};
use std::time::Duration;

/// Every encoding this build has.
pub(crate) fn encodings() -> Vec<Encoding> {
    vec![
        Encoding::Raw,
        #[cfg(feature = "zstd")]
        Encoding::Zstd,
    ]
}

/// A store in a new temporary folder.
pub(crate) fn store(encoding: Encoding) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("store"), encoding);
    (dir, store)
}

/// Writes `bytes` at `relative` under `root`, making folders.
pub(crate) fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = crate::layout::join_disk(root, relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Sets `path`'s modification time `seconds` ago (clearing read-only for the moment).
pub(crate) fn age(path: &Path, seconds: u64) {
    let readonly = fs::metadata(path).unwrap().permissions().readonly();
    fsx::set_mode(path, 0o644).unwrap();
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(seconds))
        .unwrap();
    drop(file);
    if readonly {
        fsx::set_readonly(path).unwrap();
    }
}

/// Every file under `root` whose name says it is temporary.
pub(crate) fn temp_files(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(fsx::is_temp_name))
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// How many files `area` holds.
pub(crate) fn count(store: &Store, area: Area) -> usize {
    walkdir::WalkDir::new(store.area_dir(area))
        .min_depth(2)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .count()
}

/// A sealed data image of objects with these contents, stored in `store`.
pub(crate) fn stored_image(store: &Store, files: &[(&str, &[u8])]) -> Image {
    let mut image = Image::new(Kind::Data, fabric());
    for (name, bytes) in files {
        let stored = store.put_bytes(bytes).unwrap();
        image.entries.push(Entry::new(
            path(name),
            bytes.len() as u64,
            stored.sha256,
            Source::Object {},
        ));
    }
    image.seal();
    image
}

#[test]
fn small_objects_round_trip_and_dedupe() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        for data in [
            &b""[..],
            b"x",
            &TestRng::new(1).bytes(100_000),
            &vec![7u8; 1 << 20],
        ] {
            let first = store.put_bytes(data).unwrap();
            assert_eq!(first.sha256, sha256_hex(data));
            assert!(store.has_object(&first.sha256, data.len() as u64));
            assert!(!store.has_object(&first.sha256, data.len() as u64 + 1));
            assert_eq!(
                store.object_form(&first.sha256, data.len() as u64).unwrap(),
                ObjectForm::Whole
            );
            assert_eq!(
                store
                    .read_object_bytes(&first.sha256, data.len() as u64)
                    .unwrap(),
                data
            );
            let again = store.put_bytes(data).unwrap();
            assert_eq!(again.added_bytes, 0);
            let path = store.object_path(&first.sha256).unwrap();
            assert!(fs::metadata(&path).unwrap().permissions().readonly());
        }
        assert_eq!(count(&store, Area::Objects), 4);
        assert_eq!(count(&store, Area::Chunks), 0);
        assert!(temp_files(store.root()).is_empty());
    }
}

#[test]
fn large_files_are_chunk_lists() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let data = region_like(7, 3_000_000);
        let stored = store.put_bytes(&data).unwrap();
        assert_eq!(stored.sha256, sha256_hex(&data));
        let ObjectForm::Chunked(list) = store
            .object_form(&stored.sha256, data.len() as u64)
            .unwrap()
        else {
            panic!("a 3 MB file is a chunk list");
        };
        assert_eq!(list.size, data.len() as u64);
        assert_eq!(list.chunks.len(), count(&store, Area::Chunks));
        assert!(store.has_object(&stored.sha256, data.len() as u64));
        assert_eq!(
            store
                .read_object_bytes(&stored.sha256, data.len() as u64)
                .unwrap(),
            data
        );
        // Just over the threshold is chunked; exactly 1 MiB isn't.
        let over = TestRng::new(8).bytes((1 << 20) + 1);
        let stored_over = store.put_bytes(&over).unwrap();
        assert!(matches!(
            store
                .object_form(&stored_over.sha256, over.len() as u64)
                .unwrap(),
            ObjectForm::Chunked(_)
        ));
        assert_eq!(
            store
                .read_object_bytes(&stored_over.sha256, over.len() as u64)
                .unwrap(),
            over
        );
    }
}

#[test]
fn a_changed_block_stores_only_nearby_chunks() {
    let (_dir, store) = store(Encoding::Raw);
    let original = region_like(9, 10_000_000);
    let first = store.put_bytes(&original).unwrap();
    let chunks_before = count(&store, Area::Chunks);
    let mut changed = original.clone();
    let at = 1500 * 4096;
    changed[at..at + 4096].copy_from_slice(&TestRng::new(10).bytes(4096));
    let second = store.put_bytes(&changed).unwrap();
    let new_chunks = count(&store, Area::Chunks) - chunks_before;
    assert!((1..=2).contains(&new_chunks), "{new_chunks} new chunks");
    // The second copy cost a few chunks and its list, not another 10 MB.
    assert!(
        second.added_bytes < 600_000,
        "{} bytes added",
        second.added_bytes
    );
    assert!(first.added_bytes > 10_000_000);
    let ratio = (first.added_bytes + second.added_bytes) as f64 / (2.0 * original.len() as f64);
    assert!(ratio < 0.53, "two versions take {ratio:.3} of their size");
}

#[cfg(feature = "zstd")]
mod core_compatibility {
    use super::*;
    use std::io::BufWriter;

    /// An object written exactly the way `core/src/backups/store.rs` writes one (its
    /// `store_object` and `digest`): a stream encoder at level 3 with a checksum, the
    /// content size and the pledged size, fed in pieces of `(size + 1)` clamped to 8 KiB
    /// to 1 MiB.
    fn core_object(path: &Path, data: &[u8]) {
        let file = File::create(path).unwrap();
        let mut encoder = zstd::stream::write::Encoder::new(BufWriter::new(file), 3).unwrap();
        encoder.include_checksum(true).unwrap();
        encoder.include_contentsize(true).unwrap();
        encoder
            .set_pledged_src_size(Some(data.len() as u64))
            .unwrap();
        let size = data.len() as u64;
        let mut buffer = vec![0u8; (size.saturating_add(1)).clamp(8 * 1024, 1 << 20) as usize];
        let mut reader = io::Cursor::new(data);
        loop {
            let n = reader.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            encoder.write_all(&buffer[..n]).unwrap();
        }
        encoder
            .finish()
            .unwrap()
            .into_inner()
            .unwrap()
            .sync_all()
            .unwrap();
    }

    #[test]
    fn small_objects_are_the_cores_byte_for_byte() {
        let (dir, store) = store(Encoding::Zstd);
        for data in [
            Vec::new(),
            b"x".to_vec(),
            TestRng::new(2).bytes(5000),
            region_like(3, 300_000),
            region_like(4, 1 << 20),
        ] {
            let stored = store.put_bytes(&data).unwrap();
            let core = dir.path().join("core-object");
            core_object(&core, &data);
            assert_eq!(
                fs::read(store.object_path(&stored.sha256).unwrap()).unwrap(),
                fs::read(&core).unwrap(),
                "{} bytes",
                data.len()
            );
        }
    }

    #[test]
    fn the_cores_large_whole_objects_read_in_place() {
        let (_dir, store) = store(Encoding::Zstd);
        let data = region_like(5, 3_000_000);
        let sha256 = sha256_hex(&data);
        let path = store.object_path(&sha256).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        core_object(&path, &data);
        assert!(store.has_object(&sha256, data.len() as u64));
        assert_eq!(
            store.object_form(&sha256, data.len() as u64).unwrap(),
            ObjectForm::Whole
        );
        assert_eq!(
            store.read_object_bytes(&sha256, data.len() as u64).unwrap(),
            data
        );
        // Storing the same file again keeps the whole object rather than adding a list.
        let again = store.put_bytes(&data).unwrap();
        assert_eq!(
            store.object_form(&sha256, data.len() as u64).unwrap(),
            ObjectForm::Whole
        );
        assert!(
            again.added_bytes > 0,
            "its chunks are stored, the object isn't"
        );
        assert_eq!(
            store.read_object_bytes(&sha256, data.len() as u64).unwrap(),
            data
        );
    }
}

#[test]
fn reads_check_every_byte() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let small = TestRng::new(11).bytes(50_000);
        let small_hash = store.put_bytes(&small).unwrap().sha256;
        let path = store.object_path(&small_hash).unwrap();
        // Same length, other bytes (bit rot, or someone edited it).
        let mut bad = small.clone();
        bad[100] ^= 1;
        fsx::remove_file_forced(&path).unwrap();
        fs::write(&path, encode(encoding, &bad)).unwrap();
        assert!(matches!(
            store.read_object_bytes(&small_hash, small.len() as u64),
            Err(StoreError::Damaged(_))
        ));
        assert!(store
            .verify_object(&small_hash, small.len() as u64, NEVER)
            .is_err());
        // Missing is told apart from damaged.
        fsx::remove_file_forced(&path).unwrap();
        assert!(matches!(
            store.read_object_bytes(&small_hash, small.len() as u64),
            Err(StoreError::Missing(_))
        ));
        // A damaged chunk spoils the large file it is in.
        let large = region_like(12, 2_000_000);
        let large_hash = store.put_bytes(&large).unwrap().sha256;
        let ObjectForm::Chunked(list) = store.object_form(&large_hash, large.len() as u64).unwrap()
        else {
            panic!("chunked");
        };
        let chunk = store.chunk_path(&list.chunks[1].sha256).unwrap();
        let mut bytes = store.read_chunk(&list.chunks[1]).unwrap();
        bytes[0] ^= 0xff;
        fsx::remove_file_forced(&chunk).unwrap();
        fs::write(&chunk, encode(encoding, &bytes)).unwrap();
        assert!(matches!(
            store.read_object_bytes(&large_hash, large.len() as u64),
            Err(StoreError::Damaged(_))
        ));
        // The list is fine; the chunk is what to delete.
        assert!(store.remove_damaged(&large_hash).is_err());
        assert!(store.remove_damaged_chunk(&list.chunks[1]).unwrap());
        assert!(!store.remove_damaged_chunk(&list.chunks[0]).unwrap());
        store.put_bytes(&large).unwrap();
        assert_eq!(
            store
                .read_object_bytes(&large_hash, large.len() as u64)
                .unwrap(),
            large
        );
        fsx::remove_file_forced(&chunk).unwrap();
        assert!(matches!(
            store.read_object_bytes(&large_hash, large.len() as u64),
            Err(StoreError::Missing(_))
        ));
        // A healthy list asked about with another size is the asker's mistake.
        assert!(matches!(
            store.read_object_bytes(&large_hash, large.len() as u64 + 5),
            Err(StoreError::WrongSize { .. })
        ));
        assert!(matches!(
            store.object_form("nothex", 1),
            Err(StoreError::BadHash(_))
        ));
    }
}

/// An image from elsewhere can name a healthy object with any size. That is the image's
/// mistake: reads say so, nothing calls the object present for that size, and
/// `remove_damaged` never deletes it (every other image that names it would lose it).
#[test]
fn a_wrong_size_never_costs_a_healthy_object() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let large = region_like(21, 3 << 20);
        let large_hash = store.put_bytes(&large).unwrap().sha256;
        let small = b"ten bytes!".to_vec();
        let small_hash = store.put_bytes(&small).unwrap().sha256;
        for (hash, wrong) in [
            (&large_hash, 999_999_999u64),
            (&large_hash, (3 << 20) + 1),
            (&large_hash, 3),
            (&small_hash, 3),
            (&small_hash, 5 << 20),
        ] {
            assert!(!store.has_object(hash, wrong), "{wrong}");
            assert!(!store.claim_object(hash, wrong), "{wrong}");
            let read = store.read_object_bytes(hash, wrong);
            assert!(
                matches!(read, Err(StoreError::WrongSize { size, .. }) if size == wrong),
                "{wrong}: {read:?}"
            );
            assert!(!store.remove_damaged(hash).unwrap(), "{wrong}");
        }
        assert_eq!(
            store
                .read_object_bytes(&large_hash, large.len() as u64)
                .unwrap(),
            large
        );
        assert_eq!(store.read_object_bytes(&small_hash, 10).unwrap(), small);
    }
}

/// The chunk list of one file, named with the list's own length as if it were a whole
/// file of that size, reads as neither damaged nor that file.
#[test]
fn a_list_named_with_its_own_length_is_a_wrong_size() {
    let (_dir, store) = store(Encoding::Raw);
    let large = region_like(22, 3 << 20);
    let hash = store.put_bytes(&large).unwrap().sha256;
    // Lists live in `lists/`, never in `objects/`.
    assert!(!store.object_path(&hash).unwrap().exists());
    let list_len = fs::metadata(store.list_path(&hash).unwrap()).unwrap().len();
    assert!(!store.has_object(&hash, list_len));
    assert!(matches!(
        store.read_object_bytes(&hash, list_len),
        Err(StoreError::WrongSize { .. })
    ));
    assert!(!store.remove_damaged(&hash).unwrap());
    assert_eq!(
        store.read_object_bytes(&hash, large.len() as u64).unwrap(),
        large
    );
}

/// Claiming an object touches it (and a list's chunks) when old, so a garbage collection
/// running now keeps it; a list missing a chunk isn't claimed.
#[test]
fn claiming_touches_old_objects_and_chunks() {
    let (_dir, store) = store(Encoding::Raw);
    let backdate = |path: &Path| age(path, 7200);
    let elapsed = |path: &Path| {
        SystemTime::now()
            .duration_since(fs::metadata(path).unwrap().modified().unwrap())
            .unwrap_or_default()
    };
    let small = store.put_bytes(b"small").unwrap();
    let small_path = store.object_path(&small.sha256).unwrap();
    backdate(&small_path);
    assert!(store.claim_object(&small.sha256, 5));
    assert!(elapsed(&small_path) < Duration::from_secs(600));

    let large = region_like(23, 2 << 20);
    let stored = store.put_bytes(&large).unwrap();
    let ObjectForm::Chunked(list) = store.object_form(&stored.sha256, stored.size).unwrap() else {
        panic!("chunked");
    };
    let list_path = store.list_path(&stored.sha256).unwrap();
    let chunk_paths: Vec<PathBuf> = list
        .chunks
        .iter()
        .map(|c| store.chunk_path(&c.sha256).unwrap())
        .collect();
    backdate(&list_path);
    for path in &chunk_paths {
        backdate(path);
    }
    assert!(store.claim_object(&stored.sha256, stored.size));
    assert!(elapsed(&list_path) < Duration::from_secs(600));
    for path in &chunk_paths {
        assert!(elapsed(path) < Duration::from_secs(600));
    }
    fsx::remove_file_forced(&chunk_paths[0]).unwrap();
    assert!(store.has_object(&stored.sha256, stored.size));
    assert!(!store.claim_object(&stored.sha256, stored.size));
    assert!(!store.claim_object(&sha256_hex(b"absent"), 6));
}

/// A manifest is never saved naming an object only this store has that it lacks.
#[test]
fn saving_a_manifest_checks_its_objects() {
    let (_dir, store) = store(Encoding::Raw);
    let mut image = Image::new(Kind::Data, fabric());
    let present = store.put_bytes(b"present").unwrap();
    image.entries.push(Entry::new(
        path("a.txt"),
        present.size,
        present.sha256.clone(),
        Source::Object {},
    ));
    image.seal();
    store.save_manifest("inst", &image).unwrap();

    let mut lacking = image.clone();
    lacking.entries.push(Entry::new(
        path("b.txt"),
        6,
        sha256_hex(b"absent"),
        Source::Object {},
    ));
    lacking.seal();
    assert!(matches!(
        store.save_manifest("inst", &lacking),
        Err(StoreError::Missing(hash)) if hash == sha256_hex(b"absent")
    ));
    assert!(!store.manifest_path("inst", &lacking.id).unwrap().exists());

    // A downloadable file's bytes needn't be in the store.
    let mut downloadable = image.clone();
    let mut jar = Entry::new(
        path("mods/a.jar"),
        6,
        sha256_hex(b"absent"),
        Source::Curseforge {
            project: 1,
            file: 2,
            downloadable: true,
        },
    );
    jar.env = crate::env::Env::UNKNOWN;
    downloadable.entries.push(jar);
    downloadable.seal();
    store.save_manifest("inst", &downloadable).unwrap();

    // Nor may an icon be missing.
    let mut icon = image.clone();
    icon.settings.client = Some(crate::settings::ClientSettings {
        icon: Some(crate::settings::StoredFile {
            sha256: sha256_hex(b"no icon"),
            size: 7,
        }),
        ..Default::default()
    });
    icon.seal();
    assert!(matches!(
        store.save_manifest("inst", &icon),
        Err(StoreError::Missing(_))
    ));
}

fn encode(encoding: Encoding, bytes: &[u8]) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x");
    encoding::encode_into(encoding, File::create(&path).unwrap(), bytes).unwrap();
    fs::read(&path).unwrap()
}

#[test]
fn a_cut_short_object_is_replaced_by_the_next_write() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let data = TestRng::new(13).bytes(200_000);
        let sha256 = store.put_bytes(&data).unwrap().sha256;
        let path = store.object_path(&sha256).unwrap();
        // A crash in a copy by hand, or a disk that lost the end of the file.
        let encoded = fs::read(&path).unwrap();
        fsx::remove_file_forced(&path).unwrap();
        fs::write(&path, &encoded[..encoded.len() / 2]).unwrap();
        // A raw object's length shows it; a zstd frame's header still looks whole (as in
        // the core), and only reading finds it.
        assert_eq!(
            store.has_object(&sha256, data.len() as u64),
            encoding != Encoding::Raw
        );
        assert!(store
            .verify_object(&sha256, data.len() as u64, NEVER)
            .is_err());
        assert!(store.remove_damaged(&sha256).unwrap());
        assert!(!path.exists());
        assert!(!store.remove_damaged(&sha256).unwrap());
        let again = store.put_bytes(&data).unwrap();
        assert!(again.added_bytes > 0);
        assert_eq!(
            store.read_object_bytes(&sha256, data.len() as u64).unwrap(),
            data
        );
    }
}

#[test]
fn failed_writes_leave_nothing_named() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        // Fewer bytes than promised, small and large.
        for size in [10_000usize, 3_000_000] {
            let data = TestRng::new(14).bytes(size);
            let mut writer = store.writer(size as u64 + 1, None).unwrap();
            writer.push(&data).unwrap();
            assert!(matches!(
                writer.finish(),
                Err(StoreError::SizeMismatch { .. })
            ));
            assert!(!store.has_object(&sha256_hex(&data), size as u64));
            // More bytes than promised fail at once.
            let mut writer = store.writer(size as u64 - 1, None).unwrap();
            assert!(matches!(
                writer.push(&data),
                Err(StoreError::SizeMismatch { .. })
            ));
            // A writer dropped part way (a crash in the caller).
            let mut writer = store.writer(size as u64, None).unwrap();
            writer.push(&data[..size / 2]).unwrap();
            drop(writer);
            assert!(!store.has_object(&sha256_hex(&data), size as u64));
            // Bytes that don't hash to what the caller expected.
            let wrong = sha256_hex(b"something else");
            let mut reader = io::Cursor::new(&data);
            let result = store.put_reader(&mut reader, size as u64, Some(&wrong), NEVER);
            assert!(matches!(result, Err(StoreError::HashMismatch { .. })));
            assert!(!store.has_object(&wrong, size as u64));
            assert!(!store.has_object(&sha256_hex(&data), size as u64));
        }
        // A reader that fails part way.
        struct Failing(usize);
        impl Read for Failing {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0 == 0 {
                    return Err(io::Error::other("disk gone"));
                }
                let n = buf.len().min(self.0);
                buf[..n].fill(1);
                self.0 -= n;
                Ok(n)
            }
        }
        let result = store.put_reader(&mut Failing(500_000), 1_000_000, None, NEVER);
        assert!(matches!(result, Err(StoreError::Source(_))));
        let cancelled = store.put_reader(&mut Failing(500_000), 1_000_000, None, &|| true);
        assert!(matches!(cancelled, Err(StoreError::Cancelled)));
        assert!(
            temp_files(store.root()).is_empty(),
            "{:?}",
            temp_files(store.root())
        );
    }
}

#[test]
fn temp_files_left_by_a_crash_are_swept_after_an_hour() {
    let (_dir, store) = store(Encoding::Raw);
    store.put_bytes(b"x").unwrap();
    let image = stored_image(&store, &[("mods/a.jar", b"a")]);
    store.save_manifest("inst", &image).unwrap();
    let objects = store.area_dir(Area::Objects);
    let chunks = store.area_dir(Area::Chunks);
    fs::create_dir_all(&chunks).unwrap();
    let instance = store.instance_dir("inst").unwrap();
    let mut left = Vec::new();
    for dir in [&objects, &chunks, &instance] {
        let (_, old) = fsx::create_temp(dir).unwrap();
        let (_, new) = fsx::create_temp(dir).unwrap();
        age(&old, 7200);
        left.push((old, new));
    }
    assert_eq!(store.sweep_temp(TEMP_MAX_AGE), 3);
    for (old, new) in &left {
        assert!(!old.exists());
        assert!(new.exists(), "a write in progress is left alone");
    }
    // Nothing else was touched.
    assert!(store.has_object(&sha256_hex(b"x"), 1));
    assert_eq!(store.list_manifests("inst").unwrap().images.len(), 1);
}

#[test]
fn concurrent_writers_of_the_same_bytes_agree() {
    for encoding in encodings() {
        let (_dir, store) = store(encoding);
        let small = TestRng::new(15).bytes(300_000);
        let large = region_like(16, 2_500_000);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    store.put_bytes(&small).unwrap();
                    store.put_bytes(&large).unwrap();
                });
            }
        });
        assert_eq!(
            store
                .read_object_bytes(&sha256_hex(&small), small.len() as u64)
                .unwrap(),
            small
        );
        assert_eq!(
            store
                .read_object_bytes(&sha256_hex(&large), large.len() as u64)
                .unwrap(),
            large
        );
        assert!(temp_files(store.root()).is_empty());
    }
}

#[test]
fn files_are_stored_with_their_metadata() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let small = dir.path().join("small.txt");
        fs::write(&small, b"hello").unwrap();
        let stored = store.put_file(&small, NEVER).unwrap();
        assert_eq!(stored.sha256, sha256_hex(b"hello"));
        assert_eq!(stored.size, 5);
        assert_eq!(
            stored.modified_ns,
            fsx::modified_ns(&fs::metadata(&small).unwrap())
        );
        assert_eq!(
            store.read_object_bytes(&stored.sha256, 5).unwrap(),
            b"hello"
        );
        // The object is its own file: changing the source (even through a clone) leaves
        // it as it was.
        fs::write(&small, b"HELLO").unwrap();
        assert_eq!(
            store.read_object_bytes(&stored.sha256, 5).unwrap(),
            b"hello"
        );
        let object = store.object_path(&stored.sha256).unwrap();
        assert!(fs::metadata(&object).unwrap().permissions().readonly());
        // A clone keeps the source's time; the object must look new to a running GC.
        age(&small, 7200);
        fs::write(dir.path().join("other.txt"), b"other").unwrap();
        age(&dir.path().join("other.txt"), 7200);
        let other = store
            .put_file(&dir.path().join("other.txt"), NEVER)
            .unwrap();
        let meta = fs::metadata(store.object_path(&other.sha256).unwrap()).unwrap();
        assert!(fsx::age(&meta, SystemTime::now()).unwrap() < Duration::from_secs(600));

        let large = dir.path().join("large.mca");
        let data = region_like(17, 2_000_000);
        fs::write(&large, &data).unwrap();
        let stored = store.put_file(&large, NEVER).unwrap();
        assert_eq!(stored.sha256, sha256_hex(&data));
        assert_eq!(
            store
                .read_object_bytes(&stored.sha256, data.len() as u64)
                .unwrap(),
            data
        );

        assert!(matches!(
            store.put_file(&dir.path().join("missing"), NEVER),
            Err(StoreError::Source(e)) if e.kind() == io::ErrorKind::NotFound
        ));
        assert!(matches!(
            store.put_file(dir.path(), NEVER),
            Err(StoreError::Source(_))
        ));
    }
}

#[test]
fn a_file_growing_while_read_is_reported_as_changed() {
    let (dir, store) = store(Encoding::Raw);
    let path = dir.path().join("growing.log");
    fs::write(&path, vec![1u8; 3_000_000]).unwrap();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let mut changed = 0;
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut file = File::options().append(true).open(&path).unwrap();
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                file.write_all(&[2u8; 4096]).unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        for _ in 0..5 {
            if matches!(store.put_file(&path, NEVER), Err(StoreError::Changed)) {
                changed += 1;
            }
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    assert!(changed >= 3, "only {changed} of 5 reads saw the change");
}

#[test]
fn manifests_round_trip_and_are_checked() {
    let (_dir, store) = store(Encoding::Raw);
    let mut image = stored_image(&store, &[("mods/a.jar", b"a"), ("config/b.toml", b"b")]);
    image.label = Some("first".into());
    image.created = 10;
    image.seal();
    let path = store.save_manifest("server-1", &image).unwrap();
    assert!(path.ends_with(format!("server-1/{}.json", image.id)));
    assert_eq!(store.load_manifest("server-1", &image.id).unwrap(), image);
    // Saving again with another label keeps one file, with the newer label.
    let mut relabelled = image.clone();
    relabelled.label = Some("second".into());
    relabelled.created = 20;
    store.save_manifest("server-1", &relabelled).unwrap();
    let listing = store.list_manifests("server-1").unwrap();
    assert_eq!(listing.images.len(), 1);
    assert_eq!(listing.images[0].label.as_deref(), Some("second"));

    // Unsealed and tampered images are refused.
    let mut unsealed = image.clone();
    unsealed.id.clear();
    assert!(matches!(
        store.save_manifest("server-1", &unsealed),
        Err(StoreError::Identity(IdentityError::Unsealed))
    ));
    let mut tampered = image.clone();
    tampered.entries[0].size += 1;
    assert!(matches!(
        store.save_manifest("server-1", &tampered),
        Err(StoreError::Identity(_))
    ));

    // A manifest under another id's name, a newer schema, and garbage are listed as
    // unreadable, not silently taken.
    let dir = store.instance_dir("server-1").unwrap();
    let wrong_id = sha256_hex(b"wrong");
    fs::copy(&path, dir.join(format!("{wrong_id}.json"))).unwrap();
    let newer = sha256_hex(b"newer");
    let mut json: serde_json::Value = serde_json::from_slice(&image.to_json_pretty()).unwrap();
    json["schema"] = 3.into();
    fs::write(
        dir.join(format!("{newer}.json")),
        serde_json::to_vec(&json).unwrap(),
    )
    .unwrap();
    let garbage = sha256_hex(b"garbage");
    fs::write(dir.join(format!("{garbage}.json")), b"{").unwrap();
    // Other files are not manifests at all.
    fs::write(dir.join("notes.json"), b"{}").unwrap();
    let listing = store.list_manifests("server-1").unwrap();
    assert_eq!(listing.images.len(), 1);
    let unreadable: Vec<&str> = listing
        .unreadable
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    let mut expected = vec![wrong_id.as_str(), newer.as_str(), garbage.as_str()];
    expected.sort();
    assert_eq!(unreadable, expected);
    assert!(listing
        .unreadable
        .iter()
        .any(|(id, why)| id == &newer && why.contains("newer")));

    assert!(store.delete_manifest("server-1", &image.id).unwrap());
    assert!(!store.delete_manifest("server-1", &image.id).unwrap());
    assert!(matches!(
        store.load_manifest("server-1", &image.id),
        Err(StoreError::Missing(_))
    ));
    assert_eq!(
        store.list_instances().unwrap(),
        vec!["server-1".to_string()]
    );
    assert!(store.list_manifests("nobody").unwrap().images.is_empty());
}

#[test]
fn instance_ids_name_only_their_folder() {
    let (_dir, store) = store(Encoding::Raw);
    for bad in [
        "",
        "..",
        "a/b",
        "a\\b",
        "CON",
        "nul",
        "com1",
        "x y",
        &"a".repeat(101),
    ] {
        assert!(!valid_instance_id(bad), "{bad:?}");
        assert!(matches!(
            store.instance_dir(bad),
            Err(StoreError::BadInstance(_))
        ));
    }
    for good in [
        "a",
        "server-1",
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "My_Pack",
    ] {
        assert!(valid_instance_id(good));
    }
    assert!(matches!(
        store.manifest_path("a", "../x"),
        Err(StoreError::BadHash(_))
    ));
}

#[test]
fn stat_caches_persist_per_instance() {
    let (_dir, store) = store(Encoding::Raw);
    assert_eq!(store.load_stat_cache("a"), StatCache::default());
    let mut cache = StatCache::new(42);
    cache.insert("world/level.dat".into(), 3, 1, sha256_hex(b"abc"));
    store.save_stat_cache("a", &cache).unwrap();
    assert_eq!(store.load_stat_cache("a"), cache);
    assert_eq!(store.load_stat_cache("b"), StatCache::default());
    // A damaged cache is no cache.
    fs::write(
        store.instance_dir("a").unwrap().join(STAT_CACHE_FILE),
        b"{oops",
    )
    .unwrap();
    assert_eq!(store.load_stat_cache("a"), StatCache::default());
    // It isn't a manifest.
    assert!(store.list_manifests("a").unwrap().unreadable.is_empty());
}

/// How fast a large new file goes into the store, flushes and all: run by hand
/// (`cargo test --release -p lodestone_image store_throughput -- --ignored --nocapture`)
/// to see that durability stays batched (one full flush per manifest, not per chunk).
#[test]
#[ignore]
fn store_throughput() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let mut rng = TestRng::new(7);
        let bytes: Vec<u8> = (0..256 << 20).map(|_| rng.next_u64() as u8).collect();
        let source = dir.path().join("big.bin");
        fs::write(&source, &bytes).unwrap();
        let started = std::time::Instant::now();
        let stored = store.put_file(&source, NEVER).unwrap();
        store.sync().unwrap();
        let elapsed = started.elapsed();
        let mb = stored.size as f64 / (1 << 20) as f64;
        println!(
            "{encoding:?}: {mb:.0} MiB, {} chunks' worth, in {elapsed:.2?}: {:.0} MiB/s",
            stored.size / (64 << 10),
            mb / elapsed.as_secs_f64()
        );

        // Placing many small files durably: one flush of the drive at the end, not one per
        // file.
        let mut image = Image::new(Kind::Data, fabric());
        for i in 0..3000u32 {
            let bytes = format!("file {i} ").repeat(400).into_bytes();
            let stored = store.put_bytes(&bytes).unwrap();
            image.entries.push(Entry::new(
                path(&format!("config/{}/{i}.txt", i % 30)),
                stored.size,
                stored.sha256,
                Source::Object {},
            ));
        }
        image.seal();
        for durable in [false, true] {
            let staging = dir.path().join(format!("staging-{durable}"));
            fs::create_dir_all(&staging).unwrap();
            let started = std::time::Instant::now();
            materialize::materialize(
                &store,
                &image,
                &crate::DiskLayout::Client,
                &staging,
                &materialize::MaterializeOptions {
                    durable,
                    ..Default::default()
                },
            )
            .unwrap();
            println!(
                "{encoding:?}: placed 3000 files (durable: {durable}) in {:.2?}",
                started.elapsed()
            );
        }
    }
}

#[test]
fn the_downgrade_guard_is_written_before_anything_else() {
    let (_dir, store) = store(Encoding::Raw);
    assert!(!store.root().exists());
    store.put_bytes(b"first object").unwrap();
    for (path, contents) in guard::guard_files(store.root()) {
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }
}
