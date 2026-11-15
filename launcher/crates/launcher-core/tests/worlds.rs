//! Worlds: the list from `level.dat`, renaming, deleting, and world zips (export, import,
//! and the checks). Backups of worlds are snapshots (`launcher_core::images`).

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use launcher_core::{
    content::InstanceDirs,
    game::download::{CancelToken, Progress},
    worlds::{self, WorldError},
};
use lodestone_minecraft::level;

const LEVEL_DAT: &[u8] =
    include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");
const OLD_LEVEL_DAT: &[u8] =
    include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.8.9.dat");

fn instance() -> (tempfile::TempDir, InstanceDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(temp.path().join("instance"));
    fs::create_dir_all(&dirs.game).unwrap();
    (temp, dirs)
}

fn make_world(dirs: &InstanceDirs, folder: &str, level_dat: &[u8]) -> PathBuf {
    let dir = worlds::saves_dir(dirs).join(folder);
    fs::create_dir_all(dir.join("region")).unwrap();
    fs::create_dir_all(dir.join("playerdata")).unwrap();
    fs::write(dir.join("level.dat"), level_dat).unwrap();
    fs::write(dir.join("region/r.0.0.mca"), vec![7u8; 10_000]).unwrap();
    fs::write(dir.join("playerdata/p.dat"), b"player").unwrap();
    fs::write(dir.join("session.lock"), b"\xe2\x98\x83").unwrap();
    let icon = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 128, 0, 255]));
    icon.save(dir.join("icon.png")).unwrap();
    dir
}

fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if path.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
        } else {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
    }
    zip.finish().unwrap();
}

fn progress() -> (Progress, CancelToken) {
    (Progress::default(), CancelToken::new())
}

#[test]
fn world_zips_with_colliding_names_are_rejected() {
    let (temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    let (progress, cancel) = progress();

    let check = |name: &str, entries: &[(&str, &[u8])], expected: &str| {
        let path = temp.path().join(name);
        write_zip(&path, entries);
        let before = tree(&worlds::saves_dir(&dirs));
        let err = worlds::import_zip(&dirs, &path, &progress, &cancel).unwrap_err();
        assert!(matches!(err, WorldError::Conflict(_)), "{name}: {err:?}");
        assert!(err.to_string().contains(expected), "{name}: {err}");
        assert_eq!(tree(&worlds::saves_dir(&dirs)), before, "{name} left files");
    };
    // the case from the Windows report: only one region file survived, silently
    check(
        "Collide World.zip",
        &[
            ("Collide World/level.dat", LEVEL_DAT),
            ("Collide World/region/r.0.0.mca", b"AAAA"),
            ("Collide World/REGION/r.0.0.mca", b"BBBB"),
        ],
        "Collide World/region/r.0.0.mca and Collide World/REGION/r.0.0.mca",
    );
    check(
        "file case.zip",
        &[
            ("level.dat", LEVEL_DAT),
            ("data/raids.dat", b"a"),
            ("data/Raids.dat", b"b"),
        ],
        "differ only in case",
    );
    check(
        "level case.zip",
        &[("level.dat", LEVEL_DAT), ("LEVEL.DAT", b"not nbt")],
        "differ only in case",
    );
    check(
        "unicode case.zip",
        &[
            ("level.dat", LEVEL_DAT),
            ("datapacks/Ünï/pack.mcmeta", b"a"),
            ("datapacks/üNÏ/pack.mcmeta", b"b"),
        ],
        "differ only in case",
    );
    check(
        "file and folder.zip",
        &[
            ("level.dat", LEVEL_DAT),
            ("stats", b"a file"),
            ("STATS/p.json", b"{}"),
        ],
        "differ only in case",
    );
    check(
        "same file and folder.zip",
        &[
            ("level.dat", LEVEL_DAT),
            ("stats", b"a file"),
            ("stats/p.json", b"{}"),
        ],
        "more than once",
    );

    // Folders may repeat, and names that differ by more than case are fine.
    let fine = temp.path().join("Fine.zip");
    write_zip(
        &fine,
        &[
            ("Fine/", b""),
            ("Fine/level.dat", LEVEL_DAT),
            ("Fine/region/", b""),
            ("Fine/region/r.0.0.mca", b"AAAA"),
            ("Fine/region/r.0.1.mca", b"BBBB"),
            ("Fine/DIM-1/region/r.0.0.mca", b"CCCC"),
        ],
    );
    assert_eq!(
        worlds::import_zip(&dirs, &fine, &progress, &cancel).unwrap(),
        "Fine"
    );
    let dir = worlds::saves_dir(&dirs).join("Fine");
    assert_eq!(fs::read(dir.join("region/r.0.0.mca")).unwrap(), b"AAAA");
    assert_eq!(fs::read(dir.join("region/r.0.1.mca")).unwrap(), b"BBBB");
    assert_eq!(
        fs::read(dir.join("DIM-1/region/r.0.0.mca")).unwrap(),
        b"CCCC"
    );
}

#[test]
fn lists_worlds_from_level_dat() {
    let (_temp, dirs) = instance();
    assert!(worlds::list(&dirs).is_empty());
    make_world(&dirs, "New World", LEVEL_DAT);
    make_world(&dirs, "Old", OLD_LEVEL_DAT);
    fs::create_dir_all(worlds::saves_dir(&dirs).join("not a world")).unwrap();
    fs::create_dir_all(worlds::saves_dir(&dirs).join(".hidden")).unwrap();
    fs::write(
        worlds::saves_dir(&dirs).join(".hidden/level.dat"),
        LEVEL_DAT,
    )
    .unwrap();
    let broken = worlds::saves_dir(&dirs).join("Broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(broken.join("level.dat"), b"garbage").unwrap();

    let list = worlds::list(&dirs);
    let folders: Vec<&str> = list.iter().map(|w| w.folder.as_str()).collect();
    // Most recently played first; the broken world's file is newest by its mtime.
    assert_eq!(folders.len(), 3);
    let modern = list.iter().find(|w| w.folder == "New World").unwrap();
    assert_eq!(modern.name(), "Fixture World");
    let summary = modern.summary.as_ref().unwrap();
    assert_eq!(summary.version_name.as_deref(), Some("1.21.11"));
    assert_eq!(summary.game_mode.map(|m| m.label()), Some("Survival"));
    assert!(modern.icon.is_some());
    assert!(modern.size > 10_000);
    let old = list.iter().find(|w| w.folder == "Old").unwrap();
    assert!(old.summary.as_ref().unwrap().hardcore);
    assert!(modern.last_played() > old.last_played());
    let broken = list.iter().find(|w| w.folder == "Broken").unwrap();
    assert!(broken.summary.is_none());
    assert!(broken.problem.as_deref().unwrap().contains("level.dat"));
    assert_eq!(broken.name(), "Broken");
}

#[test]
fn rename_and_delete() {
    let (_temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    make_world(&dirs, "Taken", LEVEL_DAT);

    let folder = worlds::rename(&dirs, "World", "My: Base").unwrap();
    assert_eq!(folder, "My_ Base");
    let dir = worlds::saves_dir(&dirs).join(&folder);
    let summary = level::summarize(&fs::read(dir.join("level.dat")).unwrap()).unwrap();
    assert_eq!(summary.name.as_deref(), Some("My: Base"));
    // A taken folder name gets a number.
    assert_eq!(
        worlds::rename(&dirs, "My_ Base", "Taken").unwrap(),
        "Taken (2)"
    );
    // Only the name inside changes when the folder name stays.
    assert_eq!(
        worlds::rename(&dirs, "Taken (2)", "Taken (2)").unwrap(),
        "Taken (2)"
    );
    assert!(worlds::rename(&dirs, "Taken (2)", "  ").is_err());
    assert!(worlds::rename(&dirs, "../Taken", "x").is_err());

    let mut removed = Vec::new();
    worlds::delete_with(&dirs, "Taken", |path| {
        removed.push(path.to_path_buf());
        Ok(())
    })
    .unwrap();
    assert_eq!(removed, [worlds::saves_dir(&dirs).join("Taken")]);
    assert!(worlds::delete_with(&dirs, "Missing", |_| Ok(())).is_err());
    assert!(worlds::delete_with(&dirs, "..", |_| Ok(())).is_err());
}

#[test]
fn a_rename_that_cant_write_level_dat_moves_the_world_back() {
    let (_temp, dirs) = instance();
    let world = make_world(&dirs, "World", LEVEL_DAT);
    // level.dat can't be replaced: a folder is where its temporary file goes.
    let mut moves = Vec::new();
    let renamed = worlds::rename_with(&dirs, "World", "Base", |from, to| {
        moves.push((from.to_path_buf(), to.to_path_buf()));
        if moves.len() == 1 {
            fs::rename(from, to)?;
            // Make writing level.dat fail.
            let level = to.join("level.dat");
            fs::remove_file(&level)?;
            fs::create_dir(&level)?;
            Ok(())
        } else {
            fs::remove_dir(to.join("level.dat")).ok();
            fs::rename(from, to)
        }
    });
    assert!(renamed.is_err());
    assert_eq!(moves.len(), 2, "moved there and back");
    assert!(world.is_dir());
    assert!(!worlds::saves_dir(&dirs).join("Base").exists());
}

#[test]
fn world_zips_export_import_and_checks() {
    let (temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    let (progress, cancel) = progress();

    let out = temp.path().join("exported.zip");
    worlds::export(&dirs, "World", &out, &progress, &cancel).unwrap();
    let checked = worlds::inspect_zip(&out).unwrap();
    assert_eq!(checked.prefix, "World/");
    assert_eq!(checked.folder_hint, "World");
    assert_eq!(checked.summary.name.as_deref(), Some("Fixture World"));
    let imported = worlds::import_zip(&dirs, &out, &progress, &cancel).unwrap();
    assert_eq!(imported, "World (2)");
    assert!(
        worlds::saves_dir(&dirs)
            .join("World (2)/region/r.0.0.mca")
            .is_file()
    );

    // level.dat at the root: the folder is named after the zip.
    let root = temp.path().join("Flat Root.zip");
    write_zip(
        &root,
        &[
            ("level.dat", LEVEL_DAT),
            ("region/", b""),
            ("region/r.0.0.mca", b"r"),
            ("__MACOSX/._level.dat", b"junk"),
        ],
    );
    let checked = worlds::inspect_zip(&root).unwrap();
    assert_eq!(checked.prefix, "");
    assert_eq!(
        worlds::import_zip(&dirs, &root, &progress, &cancel).unwrap(),
        "Flat Root"
    );
    let flat = worlds::saves_dir(&dirs).join("Flat Root");
    assert!(flat.join("region/r.0.0.mca").is_file());
    assert!(!flat.join("__MACOSX").exists());

    let check = |name: &str, entries: &[(&str, &[u8])], expected: &str| {
        let path = temp.path().join(name);
        write_zip(&path, entries);
        let before = tree(&worlds::saves_dir(&dirs));
        let err = worlds::import_zip(&dirs, &path, &progress, &cancel).unwrap_err();
        assert!(err.to_string().contains(expected), "{name}: {err}");
        assert_eq!(tree(&worlds::saves_dir(&dirs)), before, "{name} left files");
    };
    check("none.zip", &[("readme.txt", b"hi")], "no level.dat");
    check("deep.zip", &[("a/b/level.dat", LEVEL_DAT)], "no level.dat");
    check(
        "two.zip",
        &[("A/level.dat", LEVEL_DAT), ("B/level.dat", LEVEL_DAT)],
        "several worlds",
    );
    check("garbage.zip", &[("level.dat", b"not nbt")], "can't be read");
    check(
        "escape.zip",
        &[("W/level.dat", LEVEL_DAT), ("W/../../escape.txt", b"x")],
        "unsafe",
    );
    check(
        "absolute.zip",
        &[("level.dat", LEVEL_DAT), ("/etc/evil", b"x")],
        "unsafe",
    );
    assert!(!temp.path().join("escape.txt").exists());
    let not_zip = temp.path().join("not.zip");
    fs::write(&not_zip, b"plain text").unwrap();
    assert!(worlds::inspect_zip(&not_zip).is_err());
}

/// Replaces `from` with `to` (the same length) in the file at `path`: the first match
/// only, or every match with `all`. Makes zips that zip's writer refuses to write.
fn patch_zip(path: &Path, from: &str, to: &str, all: bool) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut found = 0;
    let mut at = 0;
    while let Some(i) = bytes[at..]
        .windows(from.len())
        .position(|w| w == from.as_bytes())
    {
        let start = at + i;
        bytes[start..start + to.len()].copy_from_slice(to.as_bytes());
        found += 1;
        at = start + to.len();
        if !all {
            break;
        }
    }
    assert!(found > 0, "{from} isn't in the zip");
    fs::write(path, bytes).unwrap();
}

#[test]
fn crafted_world_zips_never_write_outside_the_world() {
    let (temp, dirs) = instance();
    let (progress, cancel) = progress();
    let saves = worlds::saves_dir(&dirs);

    // A symbolic link entry is left out.
    let path = temp.path().join("Link.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("Link/level.dat", options).unwrap();
    zip.write_all(LEVEL_DAT).unwrap();
    zip.add_symlink("Link/region", "../../../..", options)
        .unwrap();
    zip.finish().unwrap();
    let folder = worlds::import_zip(&dirs, &path, &progress, &cancel).unwrap();
    assert!(fs::symlink_metadata(saves.join(&folder).join("region")).is_err());

    // The local header names a file outside the world, the central directory one
    // inside ("zip cloaking"). Only the central directory's name is used.
    let path = temp.path().join("Cloak.zip");
    write_zip(
        &path,
        &[
            ("Cloak/level.dat", LEVEL_DAT),
            ("Cloak/data/aaaaaaaaaaaaaaaa.dat", b"data"),
        ],
    );
    patch_zip(
        &path,
        "Cloak/data/aaaaaaaaaaaaaaaa.dat",
        "../../../../../../escape123.dat",
        false,
    );
    let folder = worlds::import_zip(&dirs, &path, &progress, &cancel).unwrap();
    assert_eq!(
        fs::read(saves.join(folder).join("data/aaaaaaaaaaaaaaaa.dat")).unwrap(),
        b"data"
    );
    assert!(!temp.path().join("escape123.dat").exists());
    assert!(!dirs.game.join("escape123.dat").exists());

    // A name listed twice: zip reads it as one entry, so one file is imported.
    let path = temp.path().join("Twice.zip");
    write_zip(
        &path,
        &[
            ("Twice/level.dat", LEVEL_DAT),
            ("Twice/region/r.0.0.mca", b"AAAA"),
            ("Twice/region/r.0.0.mcb", b"BBBB"),
        ],
    );
    patch_zip(&path, "r.0.0.mcb", "r.0.0.mca", true);
    let folder = worlds::import_zip(&dirs, &path, &progress, &cancel).unwrap();
    let region = saves.join(folder).join("region");
    assert_eq!(fs::read_dir(&region).unwrap().count(), 1);
    let bytes = fs::read(region.join("r.0.0.mca")).unwrap();
    assert!(bytes == b"AAAA" || bytes == b"BBBB");
}
