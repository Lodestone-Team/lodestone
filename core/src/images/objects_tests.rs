//! Streaming objects out of a temporary store: whole and in ranges, small and chunked,
//! damaged, and while a collection runs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::StreamExt;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::{Encoding, ObjectForm, Store};
use lodestone_image::{CanonicalPath, Entry, Game, Image, Kind, Loader, Source};

use super::*;

/// Bytes FastCDC cuts into many chunks.
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

async fn collect(
    store: &Arc<Store>,
    sha256: &str,
    size: u64,
    range: Option<ByteRange>,
) -> Result<Vec<u8>, std::io::Error> {
    let permit = Downloads::default()
        .try_start(&InstanceUuid::default())
        .unwrap();
    let stream = object_stream(
        store.clone(),
        sha256.to_string(),
        size,
        range,
        permit,
        IDLE_TIMEOUT,
    );
    let mut stream = std::pin::pin!(stream);
    let mut out = Vec::new();
    while let Some(bytes) = stream.next().await {
        out.extend_from_slice(&bytes?);
    }
    Ok(out)
}

fn store() -> (tempfile::TempDir, Arc<Store>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::new(dir.path(), Encoding::Zstd));
    (dir, store)
}

fn range(start: u64, end: u64) -> Option<ByteRange> {
    Some(ByteRange { start, end })
}

#[tokio::test]
async fn a_small_object_whole_and_in_part() {
    let (_dir, store) = store();
    let bytes = b"a small config file".to_vec();
    let stored = store.put_bytes(&bytes).unwrap();
    let size = stored.size;
    assert_eq!(
        collect(&store, &stored.sha256, size, None).await.unwrap(),
        bytes
    );
    assert_eq!(
        collect(&store, &stored.sha256, size, range(2, 6))
            .await
            .unwrap(),
        &bytes[2..=6]
    );
    assert_eq!(
        collect(&store, &stored.sha256, size, range(size - 1, size - 1))
            .await
            .unwrap(),
        &bytes[bytes.len() - 1..]
    );
    // an empty file
    let empty = store.put_bytes(b"").unwrap();
    assert!(collect(&store, &empty.sha256, 0, None)
        .await
        .unwrap()
        .is_empty());
    // a missing object ends in an error
    assert!(collect(&store, &"a".repeat(64), 3, None).await.is_err());
}

#[tokio::test]
async fn a_large_object_is_put_together_from_its_chunks() {
    let (_dir, store) = store();
    let bytes = noise(3 << 20, 7);
    let stored = store.put_bytes(&bytes).unwrap();
    let size = stored.size;
    let ObjectForm::Chunked(list) = store.object_form(&stored.sha256, size).unwrap() else {
        panic!("a chunk list");
    };
    assert!(list.chunks.len() > 10);
    assert_eq!(
        collect(&store, &stored.sha256, size, None).await.unwrap(),
        bytes
    );
    let first = u64::from(list.chunks[0].size);
    let second = first + u64::from(list.chunks[1].size);
    for (start, end) in [
        // inside one chunk
        (10, 20),
        (first, first),
        // across chunk boundaries
        (first - 5, first + 5),
        (first - 1, second + 1),
        // more than one read's worth
        (1, size - 2),
        // a suffix
        (size - 100, size - 1),
        (0, 0),
    ] {
        assert_eq!(
            collect(&store, &stored.sha256, size, range(start, end))
                .await
                .unwrap(),
            &bytes[start as usize..=end as usize],
            "{start}-{end}"
        );
    }
}

#[tokio::test]
async fn a_damaged_chunk_ends_the_stream_and_is_deleted() {
    let (_dir, store) = store();
    let bytes = noise(2 << 20, 11);
    let stored = store.put_bytes(&bytes).unwrap();
    let ObjectForm::Chunked(list) = store.object_form(&stored.sha256, stored.size).unwrap() else {
        panic!("a chunk list");
    };
    let chunk = &list.chunks[list.chunks.len() / 2];
    let path = store.chunk_path(&chunk.sha256).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    std::fs::write(&path, b"not this chunk").unwrap();

    assert!(collect(&store, &stored.sha256, stored.size, None)
        .await
        .is_err());
    assert!(!path.exists(), "the damaged chunk is deleted");
    // a range before the damage still reads
    assert_eq!(
        collect(&store, &stored.sha256, stored.size, range(0, 99))
            .await
            .unwrap(),
        &bytes[..100]
    );
    // storing the file again brings the chunk back
    store.put_bytes(&bytes).unwrap();
    assert_eq!(
        collect(&store, &stored.sha256, stored.size, None)
            .await
            .unwrap(),
        bytes
    );
}

#[tokio::test]
async fn a_damaged_small_object_ends_the_stream_and_is_deleted() {
    let (_dir, store) = store();
    let stored = store.put_bytes(b"a small object").unwrap();
    let path = store.object_path(&stored.sha256).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    std::fs::write(&path, b"garbage").unwrap();
    assert!(collect(&store, &stored.sha256, stored.size, None)
        .await
        .is_err());
    assert!(!path.exists(), "the damaged object is deleted");
}

/// Marks every object, list and chunk two hours old, so a collection takes whatever no
/// manifest names.
fn age(root: &std::path::Path) {
    for area in ["objects", "lists", "chunks"] {
        for entry in walkdir::WalkDir::new(root.join(area))
            .min_depth(2)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                let mut permissions = entry.metadata().unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                std::fs::set_permissions(entry.path(), permissions).unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(entry.path())
                    .unwrap()
                    .set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))
                    .unwrap();
            }
        }
    }
}

#[tokio::test]
async fn collections_never_break_a_stream_of_a_live_object() {
    let (dir, store) = store();
    let bytes = noise(3 << 20, 13);
    let stored = store.put_bytes(&bytes).unwrap();
    // an object nothing names, which every collection takes
    let unused = store.put_bytes(&noise(2 << 20, 17)).unwrap();
    let mut image = Image::new(
        Kind::Content,
        Game {
            minecraft: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
        },
    );
    image.entries = vec![Entry::new(
        CanonicalPath::new("mods/big.jar").unwrap(),
        stored.size,
        stored.sha256.clone(),
        Source::Object {},
    )];
    image.seal();
    store.save_manifest("INSTANCE_a", &image).unwrap();
    age(dir.path());

    let stop = Arc::new(AtomicBool::new(false));
    let collector = {
        let store = store.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut runs = 0;
            while !stop.load(Ordering::SeqCst) || runs == 0 {
                store.gc(&GcOptions::default()).unwrap();
                runs += 1;
            }
            runs
        })
    };
    for round in 0..5 {
        assert_eq!(
            collect(&store, &stored.sha256, stored.size, None)
                .await
                .unwrap(),
            bytes,
            "round {round}"
        );
    }
    stop.store(true, Ordering::SeqCst);
    assert!(collector.join().unwrap() > 0);
    assert!(store.object_complete(&stored.sha256, stored.size));
    assert!(!store.object_complete(&unused.sha256, unused.size));
}

#[test]
fn downloads_are_limited_per_server_and_in_all() {
    let downloads = Downloads::default();
    let servers: Vec<InstanceUuid> = (0..5)
        .map(|n| InstanceUuid::from(format!("INSTANCE_{n}")))
        .collect();
    let mut held = Vec::new();
    for _ in 0..MAX_OBJECT_STREAMS_PER_SERVER {
        held.push(downloads.try_start(&servers[0]).unwrap());
    }
    // one server's limit leaves the others theirs
    assert!(downloads.try_start(&servers[0]).is_none());
    for server in &servers[1..] {
        while held.len() < MAX_OBJECT_STREAMS {
            let Some(permit) = downloads.try_start(server) else {
                break;
            };
            held.push(permit);
        }
    }
    assert_eq!(downloads.running(), MAX_OBJECT_STREAMS);
    // all of them: nobody starts, not even a server with none running
    assert!(downloads
        .try_start(&InstanceUuid::from("INSTANCE_new".to_string()))
        .is_none());
    held.pop();
    assert!(downloads.try_start(&servers[0]).is_none());
    held.clear();
    assert_eq!(downloads.running(), 0);
    assert!(downloads.try_start(&servers[0]).is_some());
}

/// A player that stops reading: the download ends after the idle time and gives its place
/// back, though the connection still holds the stream.
#[tokio::test]
async fn a_download_nobody_reads_ends_and_frees_its_place() {
    let (_dir, store) = store();
    let bytes = noise(3 << 20, 19);
    let stored = store.put_bytes(&bytes).unwrap();
    let downloads = Downloads::default();
    let uuid = InstanceUuid::default();
    let stream = object_stream(
        store.clone(),
        stored.sha256.clone(),
        stored.size,
        None,
        downloads.try_start(&uuid).unwrap(),
        Duration::from_millis(200),
    );
    let mut stream = std::pin::pin!(stream);
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(&first[..], &bytes[..first.len()]);
    assert_eq!(downloads.running(), 1);
    // the player takes nothing more
    crate::test_util::wait_until("the download gives up", || async {
        downloads.running() == 0
    })
    .await;
    // what it already read ahead may still come, then the body ends short
    let mut rest = first.len();
    while let Some(piece) = stream.next().await {
        rest += piece.unwrap().len();
    }
    assert!(rest < bytes.len(), "the body is cut short");

    // a player that reads at its own pace gets everything
    let stream = object_stream(
        store.clone(),
        stored.sha256.clone(),
        stored.size,
        None,
        downloads.try_start(&uuid).unwrap(),
        Duration::from_millis(200),
    );
    let mut stream = std::pin::pin!(stream);
    let mut out = Vec::new();
    while let Some(piece) = stream.next().await {
        out.extend_from_slice(&piece.unwrap());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(out, bytes);
    assert_eq!(downloads.running(), 0);
}
