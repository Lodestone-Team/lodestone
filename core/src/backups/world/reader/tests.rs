//! Restore plans and extraction from zips built in temporary folders: the layout mapping
//! both ways, and every hostile zip refused before anything is written.

use std::collections::BTreeMap;
use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

fn vanilla(level: &str) -> ServerLayout {
    ServerLayout::new(level, [], false).unwrap()
}

fn paper(level: &str) -> ServerLayout {
    ServerLayout::new(level, [], true).unwrap()
}

/// A zip with these entries (a trailing `/` makes a folder).
fn zip_of(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
    zip_with(dir, entries, SimpleFileOptions::default())
}

fn zip_with(
    dir: &Path,
    entries: &[(&str, &[u8])],
    options: SimpleFileOptions,
) -> std::path::PathBuf {
    let path = dir.join(format!("{}.zip", crate::util::random_suffix()));
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    for (name, bytes) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, options).unwrap();
        } else {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
    }
    zip.finish().unwrap();
    path
}

fn placed(plan: &RestoreZipPlan) -> BTreeMap<String, u64> {
    plan.files
        .iter()
        .map(|f| (f.disk.clone(), f.size))
        .collect()
}

fn refuses(dir: &Path, entries: &[(&str, &[u8])]) -> String {
    let zip = zip_of(dir, entries);
    let error = plan_restore(&zip, "world", &vanilla("world")).unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::BadRequest),
        "{}",
        error.source
    );
    error.source.to_string()
}

/// Replaces every `from` in the file with `to` (same length): names and fields in both
/// the local headers and the central directory.
fn patch(path: &Path, from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut found = 0;
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            found += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    assert!(found > 0, "nothing to patch");
    fs::write(path, bytes).unwrap();
}

/// Sets a field of every local header (`local_offset` past its signature) and central
/// directory header (`central_offset`).
fn patch_headers(path: &Path, local_offset: usize, central_offset: usize, value: &[u8]) {
    let mut bytes = fs::read(path).unwrap();
    for (signature, offset) in [
        ([0x50u8, 0x4b, 0x03, 0x04], local_offset),
        ([0x50u8, 0x4b, 0x01, 0x02], central_offset),
    ] {
        let mut i = 0;
        while i + 4 <= bytes.len() {
            if bytes[i..i + 4] == signature {
                bytes[i + offset..i + offset + value.len()].copy_from_slice(value);
            }
            i += 1;
        }
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn a_vanilla_world_restores_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let zip = zip_of(
        dir.path(),
        &[
            ("world/", b""),
            ("world/level.dat", b"level"),
            ("world/region/r.0.0.mca", b"region"),
            ("world/DIM-1/region/r.0.0.mca", b"nether"),
            ("world/session.lock", b"lock"),
            ("world/empty/", b""),
        ],
    );
    let plan = plan_restore(&zip, "world", &vanilla("world")).unwrap();
    assert_eq!(
        placed(&plan),
        BTreeMap::from([
            ("world/DIM-1/region/r.0.0.mca".to_string(), 6),
            ("world/level.dat".to_string(), 5),
            ("world/region/r.0.0.mca".to_string(), 6),
        ])
    );
    assert_eq!(plan.folders, ["world", "world/empty"]);
    assert_eq!(plan.total_bytes, 17 + 4);
    assert_eq!(plan.tops, ["world"]);

    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    extract_into(&zip, &plan, &staging, &|_, _| {}, &|| false).unwrap();
    assert_eq!(fs::read(staging.join("world/level.dat")).unwrap(), b"level");
    assert_eq!(
        fs::read(staging.join("world/DIM-1/region/r.0.0.mca")).unwrap(),
        b"nether"
    );
    assert!(staging.join("world/empty").is_dir());
    assert!(!staging.join("world/session.lock").exists());
}

#[test]
fn worlds_map_between_layouts_and_levels() {
    let dir = tempfile::tempdir().unwrap();
    let vanilla_zip = zip_of(
        dir.path(),
        &[
            ("world/level.dat", b"level"),
            ("world/DIM-1/region/r.0.0.mca", b"nether"),
            ("world/DIM1/region/r.0.0.mca", b"end"),
        ],
    );
    // into Paper: the dimensions go to its split folders
    let plan = plan_restore(&vanilla_zip, "world", &paper("world")).unwrap();
    assert_eq!(
        placed(&plan).into_keys().collect::<Vec<_>>(),
        [
            "world/level.dat",
            "world_nether/DIM-1/region/r.0.0.mca",
            "world_the_end/DIM1/region/r.0.0.mca",
        ]
    );
    assert_eq!(plan.tops, ["world", "world_nether", "world_the_end"]);

    let paper_zip = zip_of(
        dir.path(),
        &[
            ("world/level.dat", b"level"),
            ("world_nether/level.dat", b"nether level"),
            ("world_nether/DIM-1/region/r.0.0.mca", b"nether"),
            ("world_the_end/DIM1/region/r.0.0.mca", b"end"),
        ],
    );
    // into vanilla: the split folders' own files are kept inside the world
    let plan = plan_restore(&paper_zip, "world", &vanilla("world")).unwrap();
    assert_eq!(
        placed(&plan).into_keys().collect::<Vec<_>>(),
        [
            "world/.bukkit_nether/level.dat",
            "world/DIM-1/region/r.0.0.mca",
            "world/DIM1/region/r.0.0.mca",
            "world/level.dat",
        ]
    );
    // and back into Paper as it was
    let plan = plan_restore(&paper_zip, "world", &paper("world")).unwrap();
    assert!(placed(&plan).contains_key("world_nether/level.dat"));

    // into a server whose level is named otherwise
    let plan = plan_restore(&paper_zip, "world", &paper("survival")).unwrap();
    assert_eq!(
        placed(&plan).into_keys().collect::<Vec<_>>(),
        [
            "survival/level.dat",
            "survival_nether/DIM-1/region/r.0.0.mca",
            "survival_nether/level.dat",
            "survival_the_end/DIM1/region/r.0.0.mca",
        ]
    );
    assert_eq!(
        plan.tops,
        ["survival", "survival_nether", "survival_the_end"]
    );
}

#[test]
fn paths_that_lead_elsewhere_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "../x",
        "world/../../x",
        "/abs",
        "world\\x",
        "world/./x",
        "world//x",
        "./world/x",
    ] {
        refuses(dir.path(), &[("world/level.dat", b"x"), (name, b"x")]);
    }
}

#[test]
fn names_no_disk_can_hold_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "world/CON",
        "world/aux.txt",
        "world/x.",
        "world/x ",
        // decomposed é
        "world/e\u{301}.txt",
        // a direction override
        "world/a\u{202e}txt.exe",
        "world/a:b",
    ] {
        refuses(dir.path(), &[("world/level.dat", b"x"), (name, b"x")]);
    }
}

#[test]
fn only_the_worlds_folders_and_one_spelling_of_each_path() {
    let dir = tempfile::tempdir().unwrap();
    // outside the world
    refuses(
        dir.path(),
        &[("world/level.dat", b"x"), ("mods/a.jar", b"x")],
    );
    refuses(
        dir.path(),
        &[("world/level.dat", b"x"), ("server.properties", b"x")],
    );
    refuses(dir.path(), &[("World/level.dat", b"x")]);
    refuses(dir.path(), &[("world2/level.dat", b"x")]);
    // one file on a disk that ignores case
    refuses(
        dir.path(),
        &[
            ("world/level.dat", b"x"),
            ("world/A.txt", b"x"),
            ("world/a.txt", b"y"),
        ],
    );
    refuses(
        dir.path(),
        &[("world/Region/a.mca", b"x"), ("world/region/b.mca", b"y")],
    );
    // a file where a folder goes
    refuses(dir.path(), &[("world/a", b"x"), ("world/a/b", b"y")]);
    // a vanilla dimension inside a split level
    let zip = zip_of(
        dir.path(),
        &[
            ("world/level.dat", b"x"),
            ("world/DIM-1/r.mca", b"x"),
            ("world_nether/DIM-1/r.mca", b"y"),
        ],
    );
    assert!(plan_restore(&zip, "world", &vanilla("world")).is_err());
}

#[test]
fn a_name_twice_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let zip = zip_of(dir.path(), &[("world/aa", b"x"), ("world/ab", b"y")]);
    patch(&zip, b"world/ab", b"world/aa");
    assert!(plan_restore(&zip, "world", &vanilla("world")).is_err());
    // a local header naming another path than the directory
    let zip = zip_of(dir.path(), &[("world/aa", b"x")]);
    let mut bytes = fs::read(&zip).unwrap();
    let local = bytes.windows(8).position(|w| w == b"world/aa").unwrap();
    bytes[local + 7] = b'z';
    fs::write(&zip, bytes).unwrap();
    assert!(plan_restore(&zip, "world", &vanilla("world")).is_err());
}

#[test]
fn links_encryption_and_other_methods_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("link.zip");
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    zip.start_file("world/level.dat", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"x").unwrap();
    zip.add_symlink("world/link", "/etc", SimpleFileOptions::default())
        .unwrap();
    zip.finish().unwrap();
    assert!(plan_restore(&path, "world", &vanilla("world")).is_err());

    for method in [CompressionMethod::Bzip2, CompressionMethod::Zstd] {
        let zip = zip_with(
            dir.path(),
            &[("world/level.dat", b"x")],
            SimpleFileOptions::default().compression_method(method),
        );
        assert!(
            plan_restore(&zip, "world", &vanilla("world")).is_err(),
            "{method:?}"
        );
    }

    // the encrypted flag, in the local header (offset 6) and the directory (offset 8)
    let zip = zip_with(
        dir.path(),
        &[("world/level.dat", b"x")],
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    );
    patch_headers(&zip, 6, 8, &[1, 0]);
    assert!(plan_restore(&zip, "world", &vanilla("world")).is_err());
}

#[test]
fn bombs_and_damage_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let zip = zip_of(
        dir.path(),
        &[
            ("world/level.dat", b"x"),
            ("world/a", b"y"),
            ("world/b", b"z"),
        ],
    );
    let tight = |max_entries, max_bytes| ZipLimits {
        max_entries,
        max_bytes,
    };
    assert!(plan_restore_with(&zip, "world", &vanilla("world"), tight(2, 100)).is_err());
    plan_restore_with(&zip, "world", &vanilla("world"), tight(3, 100)).unwrap();
    // declared total over the bound
    assert!(plan_restore_with(&zip, "world", &vanilla("world"), tight(10, 2)).is_err());
    let big = zip_of(dir.path(), &[("world/level.dat", &[0u8; 1000])]);
    assert!(plan_restore_with(&big, "world", &vanilla("world"), tight(10, 999)).is_err());

    // more bytes than declared: the uncompressed size (local offset 22, directory 24)
    // says 2 for a stored entry of 6
    let lying = zip_with(
        dir.path(),
        &[("world/level.dat", b"abcdef")],
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    );
    patch_headers(&lying, 22, 24, &2u32.to_le_bytes());
    let plan = plan_restore(&lying, "world", &vanilla("world")).unwrap();
    assert_eq!(plan.total_bytes, 2);
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let error = extract_into(&lying, &plan, &staging, &|_, _| {}, &|| false).unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::BadRequest),
        "{}",
        error.source
    );

    // a truncated zip has no directory
    let cut = zip_of(dir.path(), &[("world/level.dat", &[1u8; 5000])]);
    let bytes = fs::read(&cut).unwrap();
    fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    assert!(plan_restore(&cut, "world", &vanilla("world")).is_err());
}

#[test]
fn a_cancelled_extraction_stops() {
    let dir = tempfile::tempdir().unwrap();
    let zip = zip_of(
        dir.path(),
        &[
            ("world/level.dat", b"x"),
            ("world/r.mca", &vec![3u8; 4 * CHUNK]),
        ],
    );
    let plan = plan_restore(&zip, "world", &vanilla("world")).unwrap();
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).unwrap();
    let error = extract_into(&zip, &plan, &staging, &|_, _| {}, &|| true).unwrap_err();
    assert_eq!(error.source.to_string(), "Cancelled");
    // a file already there is never written over
    let plan = plan_restore(&zip, "world", &vanilla("world")).unwrap();
    let staging = dir.path().join("staging2");
    fs::create_dir_all(staging.join("world")).unwrap();
    fs::write(staging.join("world/level.dat"), b"mine").unwrap();
    assert!(extract_into(&zip, &plan, &staging, &|_, _| {}, &|| false).is_err());
    assert_eq!(fs::read(staging.join("world/level.dat")).unwrap(), b"mine");
}
