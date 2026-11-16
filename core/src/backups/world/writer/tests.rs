//! Writing world zips from temporary server folders: what is taken, skipped and left out,
//! the Paper split, compression, and that nothing is left when a write stops.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

pub fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn vanilla() -> ServerLayout {
    ServerLayout::new("world", [], false).unwrap()
}

fn paper() -> ServerLayout {
    ServerLayout::new("world", [], true).unwrap()
}

fn never() -> bool {
    false
}

/// The zip's entries: name to (contents, compression); folders end with `/`.
pub fn read_zip(path: &Path) -> BTreeMap<String, (Vec<u8>, CompressionMethod)> {
    let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut entries = BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        entries.insert(
            file.name().unwrap().to_string(),
            (bytes, file.compression()),
        );
    }
    entries
}

fn zip_world(
    root: &Path,
    layout: &ServerLayout,
    out: &Path,
) -> Result<(WorldPlan, Written), Error> {
    let plan = plan_world(root, layout, true, ZipLimits::default())?;
    let written = write_world_zip(&plan, out, &|_, _| {}, &never)?;
    Ok((plan, written))
}

#[test]
fn a_vanilla_world_is_a_plain_zip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world/level.dat", b"level");
    write(&root, "world/region/r.0.0.mca", b"region");
    write(&root, "world/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world/stats/a.json", b"{}");
    write(&root, "world/session.lock", b"lock");
    write(&root, "world/.lodestone-restore-staging-x/a", b"x");
    // not the world
    write(&root, "server.properties", b"level-name=world");
    write(&root, "mods/a.jar", b"jar");
    write(&root, "world_nether/level.dat", b"not split on vanilla");
    let out = dir.path().join("out/.a.zip.part");
    let (plan, written) = zip_world(&root, &vanilla(), &out).unwrap();
    assert_eq!(plan.folders, ["world"]);
    assert_eq!(plan.file_count, 4);
    assert_eq!(plan.total_bytes, 5 + 6 + 6 + 2);
    assert_eq!(written.file_count, 4);
    assert_eq!(written.left_out, 0);
    assert_eq!(written.size, fs::metadata(&out).unwrap().len());
    let entries = read_zip(&out);
    let names: Vec<&str> = entries.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "world/",
            "world/DIM-1/",
            "world/DIM-1/region/",
            "world/DIM-1/region/r.0.0.mca",
            "world/level.dat",
            "world/region/",
            "world/region/r.0.0.mca",
            "world/stats/",
            "world/stats/a.json",
        ]
    );
    assert_eq!(entries["world/region/r.0.0.mca"].0, b"region");
    assert_eq!(
        entries["world/region/r.0.0.mca"].1,
        CompressionMethod::Stored
    );
    assert_eq!(entries["world/level.dat"].1, CompressionMethod::Stored);
    assert_eq!(entries["world/stats/a.json"].0, b"{}");
    assert_eq!(entries["world/stats/a.json"].1, CompressionMethod::Deflated);
}

#[test]
fn a_paper_world_keeps_its_split_folders() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world/level.dat", b"level");
    write(&root, "world/region/r.0.0.mca", b"overworld");
    write(&root, "world_nether/level.dat", b"nether level");
    write(&root, "world_nether/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"end");
    // a stale vanilla Nether in a split level: a restore couldn't place it back
    write(&root, "world/DIM-1/region/r.0.0.mca", b"stale");
    let out = dir.path().join(".a.zip.part");
    let (plan, written) = zip_world(&root, &paper(), &out).unwrap();
    assert_eq!(plan.folders, ["world", "world_nether", "world_the_end"]);
    assert_eq!(written.left_out, 1);
    let entries = read_zip(&out);
    assert_eq!(entries["world_nether/DIM-1/region/r.0.0.mca"].0, b"nether");
    assert_eq!(entries["world_nether/level.dat"].0, b"nether level");
    assert_eq!(entries["world_the_end/DIM1/region/r.0.0.mca"].0, b"end");
    assert!(!entries.contains_key("world/DIM-1/region/r.0.0.mca"));
    assert!(!entries.contains_key("world/DIM-1/"));
    // without its own folders, a split level is still backed up
    fs::remove_dir_all(root.join("world_nether")).unwrap();
    fs::remove_dir_all(root.join("world_the_end")).unwrap();
    let plan = plan_world(&root, &paper(), true, ZipLimits::default()).unwrap();
    assert_eq!(plan.folders, ["world"]);
}

#[test]
fn no_world_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let error = plan_world(root, &vanilla(), true, ZipLimits::default()).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotFound));
    fs::create_dir_all(root.join("world/region")).unwrap();
    assert!(matches!(
        plan_world(root, &vanilla(), true, ZipLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    ));
    // a restore's safety backup takes a level without a level.dat too
    let plan = plan_world(root, &vanilla(), false, ZipLimits::default()).unwrap();
    assert_eq!(plan.folders, ["world"]);
    assert!(check_world(root, &vanilla(), ZipLimits::default()).is_err());
    write(root, "world/level.dat", b"x");
    plan_world(root, &vanilla(), true, ZipLimits::default()).unwrap();
    check_world(root, &vanilla(), ZipLimits::default()).unwrap();
}

#[cfg(unix)]
#[test]
fn links_are_left_out_and_a_linked_level_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    let outside = dir.path().join("outside");
    write(&outside, "secret.txt", b"secret");
    write(&root, "world/level.dat", b"level");
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("world/link.txt")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("world/linked")).unwrap();
    let out = dir.path().join(".a.zip.part");
    let (_, written) = zip_world(&root, &vanilla(), &out).unwrap();
    assert_eq!(written.left_out, 2);
    assert_eq!(written.left_out_sample, ["world/link.txt", "world/linked"]);
    let entries = read_zip(&out);
    assert_eq!(
        entries.keys().collect::<Vec<_>>(),
        ["world/", "world/level.dat"]
    );

    let linked = dir.path().join("linked-server");
    fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(root.join("world"), linked.join("world")).unwrap();
    let error = plan_world(&linked, &vanilla(), true, ZipLimits::default()).unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::BadRequest),
        "{}",
        error.source
    );
    let split = dir.path().join("split-server");
    write(&split, "world/level.dat", b"x");
    std::os::unix::fs::symlink(&outside, split.join("world_nether")).unwrap();
    assert!(matches!(
        plan_world(&split, &paper(), true, ZipLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    ));
}

#[test]
fn a_cancelled_write_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world/level.dat", b"level");
    write(&root, "world/region/r.0.0.mca", &vec![7u8; 3 * CHUNK]);
    let plan = plan_world(&root, &vanilla(), true, ZipLimits::default()).unwrap();
    let out = dir.path().join(".a.zip.part");
    let seen = AtomicU64::new(0);
    // cancelled in the middle of the region file
    let cancel = || seen.load(Ordering::SeqCst) >= CHUNK as u64;
    let error = write_world_zip(
        &plan,
        &out,
        &|done, total| {
            assert_eq!(total, plan.total_bytes);
            seen.store(done, Ordering::SeqCst);
        },
        &cancel,
    )
    .unwrap_err();
    assert_eq!(error.source.to_string(), "Cancelled");
    assert!(!out.exists());
    // an existing file is never written over (nor deleted)
    fs::write(&out, b"someone else's").unwrap();
    assert!(write_world_zip(&plan, &out, &|_, _| {}, &never).is_err());
    assert_eq!(fs::read(&out).unwrap(), b"someone else's");
}

#[test]
fn compression_and_large_files() {
    for name in [
        "r.0.0.mca",
        "level.dat",
        "level.dat_old",
        "a.NBT",
        "icon.png",
        "x.gz",
    ] {
        assert!(stored(name), "{name}");
    }
    for name in ["a.json", "mca", "readme.txt", "data.dat.json"] {
        assert!(!stored(name), "{name}");
    }
    assert!(!needs_large_file(u64::from(u32::MAX) - 1));
    assert!(needs_large_file(u64::from(u32::MAX)));
    assert!(needs_large_file(5 << 30));
    // times before 1980 have no zip form; the entry is still written
    assert!(zip_time(std::time::UNIX_EPOCH).is_none());
    assert!(zip_time(SystemTime::now()).is_some());
}

/// A backup is only made within the bounds a restore reads a zip in: where the writer
/// takes a world, the reader takes its zip, and one entry or byte fewer refuses both.
#[test]
fn the_writer_keeps_within_what_the_reader_restores() {
    use crate::backups::world::reader::plan_restore_with;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world/level.dat", b"level");
    write(&root, "world/region/r.0.0.mca", b"region");
    write(&root, "world/stats/a.json", b"{}");
    let tight = |max_entries, max_bytes| ZipLimits {
        max_entries,
        max_bytes,
    };
    let plan = plan_world(&root, &vanilla(), true, ZipLimits::default()).unwrap();
    // world/, region/, stats/ and three files
    assert_eq!(plan.entry_count, 6);
    assert_eq!(plan.total_bytes, 13);
    let exact = tight(plan.entry_count, plan.total_bytes);
    let plan = plan_world(&root, &vanilla(), true, exact).unwrap();
    let out = dir.path().join(".a.zip.part");
    write_world_zip(&plan, &out, &|_, _| {}, &never).unwrap();
    plan_restore_with(&out, "world", &vanilla(), exact).unwrap();
    for smaller in [tight(5, 13), tight(6, 12)] {
        let error = plan_world(&root, &vanilla(), true, smaller).unwrap_err();
        assert!(
            matches!(error.kind, ErrorKind::BadRequest),
            "{}",
            error.source
        );
        assert!(error.source.to_string().contains("Take a snapshot instead"));
        assert!(check_world(&root, &vanilla(), smaller).is_err());
        assert!(plan_restore_with(&out, "world", &vanilla(), smaller).is_err());
    }

    // a world that grows past them while it is copied is stopped, and leaves nothing
    write(&root, "world/region/r.1.0.mca", b"new");
    let out = dir.path().join(".b.zip.part");
    let error = write_world_zip(&plan, &out, &|_, _| {}, &never).unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::BadRequest),
        "{}",
        error.source
    );
    assert!(!out.exists());
}

/// What is left out is counted and its first paths named, for the restore's refusal.
#[test]
fn left_out_paths_are_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world/level.dat", b"level");
    for i in 0..7 {
        write(&root, &format!("world/DIM-1/region/r.{i}.0.mca"), b"stale");
    }
    let plan = plan_world(&root, &paper(), true, ZipLimits::default()).unwrap();
    assert_eq!(plan.left_out, 7);
    assert_eq!(plan.left_out_sample.len(), LEFT_OUT_SAMPLE);
    assert_eq!(plan.left_out_sample[0], "world/DIM-1/region/r.0.0.mca");
    let out = dir.path().join(".a.zip.part");
    let written = write_world_zip(&plan, &out, &|_, _| {}, &never).unwrap();
    assert_eq!(written.left_out, 7);
    assert_eq!(written.left_out_sample, plan.left_out_sample);
}

/// A restore's safety backup takes a split server's `_nether` and `_the_end` folders when
/// their level is gone; a backup made by hand needs the level.
#[test]
fn split_folders_without_their_level() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    write(&root, "world_nether/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"end");
    assert!(matches!(
        plan_world(&root, &paper(), true, ZipLimits::default())
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    ));
    let plan = plan_world(&root, &paper(), false, ZipLimits::default()).unwrap();
    assert_eq!(plan.folders, ["world_nether", "world_the_end"]);
    let out = dir.path().join(".a.zip.part");
    write_world_zip(&plan, &out, &|_, _| {}, &never).unwrap();
    let entries = read_zip(&out);
    assert_eq!(entries["world_nether/DIM-1/region/r.0.0.mca"].0, b"nether");
    assert!(entries.keys().all(|name| !name.starts_with("world/")));
    // on vanilla those folders aren't the world's
    assert!(plan_world(&root, &vanilla(), false, ZipLimits::default()).is_err());
}
