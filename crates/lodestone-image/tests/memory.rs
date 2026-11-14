//! Hostile input from another computer never costs much more memory than its own bytes.
//!
//! This test binary counts every allocation (its own global allocator) and checks the peak
//! while a manifest or an export that compresses a thousand to one is read. Everything is
//! in one test so no other test's allocations land in the count. Hashing an image, which
//! every reader does right after, is checked the same way.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use lodestone_image::container::{self, Limits};
use lodestone_image::hex::sha256_hex;
use lodestone_image::manifest::{Entry, Game, Image, Kind, Loader, Source};
use lodestone_image::paths::CanonicalPath;

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grew(by: usize) {
    let now = CURRENT.fetch_add(by, Ordering::Relaxed) + by;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

// SAFETY: every call goes to the system allocator unchanged; only sizes are counted.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() {
            grew(new_size - layout.size());
        } else {
            CURRENT.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The most bytes allocated at once beyond what was allocated before, while `f` ran.
fn peak_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let base = CURRENT.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let result = f();
    (result, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

const MIB: usize = 1 << 20;

#[test]
fn hostile_input_costs_little_more_than_its_size() {
    // A manifest whose schema is an array of four million zeros (8 MB of JSON, a few KB
    // deflated). Built as JSON values it would take about 300 MB before anything refused
    // it; probed, it takes nothing.
    let mut manifest = String::with_capacity(8 * MIB + 16);
    manifest.push_str("{\"schema\":[0");
    for _ in 1..4_000_000 {
        manifest.push_str(",0");
    }
    manifest.push_str("]}");
    let (result, peak) = peak_during(|| Image::from_json(manifest.as_bytes()));
    assert!(result.is_err());
    assert!(peak < MIB, "reading the schema took {peak} bytes");
    drop(manifest);

    // An export whose image names an 8 GiB file, carried as a chunk list that names one
    // 16 KiB chunk 524,288 times (38 MB of list, deflated to almost nothing). Checking it
    // streams the list: no copy of it, no list of its chunks.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("export.zip");
    let chunk = vec![0u8; 16 << 10];
    let chunk_sha = sha256_hex(&chunk);
    let count: u64 = 524_288;
    let size = count * chunk.len() as u64;
    let file_sha = sha256_hex(b"whatever the file hashes to: open doesn't read it");
    let mut image = Image::new(
        Kind::Content,
        Game {
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
        },
    );
    image.entries.push(Entry::new(
        CanonicalPath::new("mods/big.jar").unwrap(),
        size,
        file_sha.clone(),
        Source::Object {},
    ));
    image.seal();
    let header = format!("lodestone-chunks 1\nsize {size}\n");
    let line = format!("{chunk_sha} {}\n", chunk.len());
    {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        zip.start_file(container::IMAGE_FILE, options).unwrap();
        zip.write_all(&image.to_json_pretty()).unwrap();
        zip.start_file(format!("objects/{file_sha}"), options)
            .unwrap();
        zip.write_all(header.as_bytes()).unwrap();
        let lines = line.repeat(4096);
        for _ in 0..count / 4096 {
            zip.write_all(lines.as_bytes()).unwrap();
        }
        zip.start_file(format!("chunks/{chunk_sha}"), options)
            .unwrap();
        zip.write_all(&chunk).unwrap();
        zip.finish().unwrap();
    }
    let (opened, peak) = peak_during(|| container::open(&path, &Limits::default()));
    let opened = opened.unwrap();
    let list_len = header.len() as u64 + count * line.len() as u64;
    assert_eq!(opened.unpacked_bytes(), list_len + chunk.len() as u64);
    assert!(peak < 8 * MIB, "opening the export took {peak} bytes");

    drop(opened);

    // Hashing an image (seal, verify, compute_id) writes its canonical form straight into
    // the hasher, an entry at a time: a 100,000-entry image (about 40 MB as a manifest)
    // costs memory for an entry, not for the image.
    let mut big = Image::new(
        Kind::Content,
        Game {
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
        },
    );
    for i in 0..100_000 {
        let name = format!("config/folder{}/file{i}.toml", i % 100);
        big.entries.push(Entry::new(
            CanonicalPath::new(&name).unwrap(),
            i,
            sha256_hex(name.as_bytes()),
            Source::Object {},
        ));
    }
    big.seal();
    let (verified, peak) = peak_during(|| big.verify());
    verified.unwrap();
    assert!(peak < MIB, "verifying a 100k-entry image took {peak} bytes");
    let (_, peak) = peak_during(|| big.seal());
    assert!(peak < MIB, "sealing a 100k-entry image took {peak} bytes");
}
