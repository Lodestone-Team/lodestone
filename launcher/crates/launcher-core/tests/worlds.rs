//! Worlds: the list from `level.dat`, backups and restoring them (as a copy and over the
//! world), renaming, deleting, and world zips (export, import, and the checks).

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use launcher_core::{
    content::InstanceDirs,
    game::download::{CancelToken, Progress},
    worlds::{self, RestoreMode, WorldError},
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

fn zip_names(path: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    names.sort();
    names
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        if name.ends_with('/') {
            zip.add_directory(*name, Default::default()).unwrap();
        } else {
            zip.start_file(*name, Default::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
    }
    zip.finish().unwrap();
}

fn progress() -> (Progress, CancelToken) {
    (Progress::default(), CancelToken::new())
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
fn backups_restore_as_a_copy_and_over_the_world() {
    let (_temp, dirs) = instance();
    let world = make_world(&dirs, "World", LEVEL_DAT);
    let (progress, cancel) = progress();

    let backup = worlds::backup(&dirs, "World", &progress, &cancel).unwrap();
    assert!(
        backup
            .path
            .starts_with(worlds::backups_dir(&dirs).join("World"))
    );
    assert!(backup.file_name.ends_with(".zip"));
    assert!(backup.size > 0);
    let names = zip_names(&backup.path);
    assert!(names.contains(&"World/level.dat".to_string()));
    assert!(names.contains(&"World/region/r.0.0.mca".to_string()));
    assert!(!names.iter().any(|n| n.ends_with("session.lock")));
    assert_eq!(
        progress.snapshot().files_done,
        progress.snapshot().files_total
    );
    let listed = worlds::backups(&dirs, "World");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, backup.path);
    // A second backup in the same second gets its own name.
    let second = worlds::backup(&dirs, "World", &progress, &cancel).unwrap();
    assert_ne!(second.path, backup.path);
    assert_eq!(worlds::backups(&dirs, "World").len(), 2);

    // As a copy.
    let restored = worlds::restore_with(
        &dirs,
        "World",
        &backup.path,
        RestoreMode::Copy,
        |_| panic!("nothing is removed"),
        &progress,
        &cancel,
    )
    .unwrap();
    let today = worlds::date(std::time::SystemTime::now());
    assert_eq!(restored.folder, format!("World (restored {today})"));
    assert!(restored.safety_backup.is_none());
    let copy = worlds::saves_dir(&dirs).join(&restored.folder);
    assert_eq!(
        fs::read(copy.join("region/r.0.0.mca")).unwrap(),
        fs::read(world.join("region/r.0.0.mca")).unwrap()
    );
    let name = level::summarize(&fs::read(copy.join("level.dat")).unwrap())
        .unwrap()
        .name
        .unwrap();
    assert_eq!(name, format!("Fixture World (restored {today})"));
    // The original is untouched.
    assert_eq!(fs::read(world.join("level.dat")).unwrap(), LEVEL_DAT);

    // Over the world: it is backed up first and the old one goes to the Trash.
    fs::write(world.join("region/r.0.0.mca"), b"griefed").unwrap();
    fs::write(world.join("new-file"), b"added after the backup").unwrap();
    let mut trashed = Vec::new();
    let replaced = worlds::restore_with(
        &dirs,
        "World",
        &backup.path,
        RestoreMode::Replace,
        |path| {
            trashed.push(fs::read(path.join("region/r.0.0.mca")).unwrap());
            fs::remove_dir_all(path).map_err(|e| e.to_string())
        },
        &progress,
        &cancel,
    )
    .unwrap();
    assert_eq!(replaced.folder, "World");
    assert_eq!(trashed, [b"griefed".to_vec()]);
    assert_eq!(
        fs::read(world.join("region/r.0.0.mca")).unwrap(),
        vec![7u8; 10_000]
    );
    assert!(!world.join("new-file").exists());
    let safety = replaced.safety_backup.unwrap();
    assert!(zip_names(&safety.path).contains(&"World/new-file".to_string()));
    assert_eq!(worlds::backups(&dirs, "World").len(), 3);
    // Nothing hidden is left in saves/.
    assert!(
        tree(&worlds::saves_dir(&dirs))
            .iter()
            .all(|p| !p.starts_with(".lodestone"))
    );

    // When the Trash fails, the old world is kept under another name.
    let kept = worlds::restore_with(
        &dirs,
        "World",
        &backup.path,
        RestoreMode::Replace,
        |_| Err("no Trash".into()),
        &progress,
        &cancel,
    )
    .unwrap();
    assert_eq!(kept.folder, "World");
    assert!(
        worlds::saves_dir(&dirs)
            .join(format!("World (replaced {today})"))
            .is_dir()
    );

    // Only the instance's own backups are restored.
    let elsewhere = dirs.instance.join("elsewhere.zip");
    fs::copy(&backup.path, &elsewhere).unwrap();
    let err = worlds::restore_with(
        &dirs,
        "World",
        &elsewhere,
        RestoreMode::Copy,
        |_| Ok(()),
        &progress,
        &cancel,
    )
    .unwrap_err();
    assert!(matches!(err, WorldError::NotFound(_)), "{err}");
}

#[test]
fn rename_and_delete() {
    let (_temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    make_world(&dirs, "Taken", LEVEL_DAT);
    let (progress, cancel) = progress();
    worlds::backup(&dirs, "World", &progress, &cancel).unwrap();

    let folder = worlds::rename(&dirs, "World", "My: Base").unwrap();
    assert_eq!(folder, "My_ Base");
    let dir = worlds::saves_dir(&dirs).join(&folder);
    let summary = level::summarize(&fs::read(dir.join("level.dat")).unwrap()).unwrap();
    assert_eq!(summary.name.as_deref(), Some("My: Base"));
    assert_eq!(worlds::backups(&dirs, "My_ Base").len(), 1);
    assert!(worlds::backups(&dirs, "World").is_empty());
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
fn renaming_keeps_the_backups_with_their_world() {
    let (_temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    let (progress, cancel) = progress();
    worlds::backup(&dirs, "World", &progress, &cancel).unwrap();
    let level_name = |folder: &str| {
        let bytes = fs::read(worlds::saves_dir(&dirs).join(folder).join("level.dat")).unwrap();
        level::summarize(&bytes).unwrap().name
    };

    // Backups of a deleted world called Base are still there: the renamed world gets a
    // folder whose backups folder is free, and takes its own backups along.
    let stale = worlds::backups_dir(&dirs).join("Base");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("old.zip"), b"not ours").unwrap();
    assert_eq!(worlds::rename(&dirs, "World", "Base").unwrap(), "Base (2)");
    assert_eq!(worlds::backups(&dirs, "Base (2)").len(), 1);
    assert!(!worlds::backups_dir(&dirs).join("World").exists());
    assert_eq!(
        fs::read_dir(&stale).unwrap().count(),
        1,
        "the old backups are left alone"
    );

    // When the backups can't be moved (Windows refuses while a backup is open), the
    // world is moved back and the error shown.
    let backups = worlds::backups_dir(&dirs).join("Base (2)");
    let err = worlds::rename_with(&dirs, "Base (2)", "Elsewhere", |from, to| {
        if from == backups {
            Err(std::io::Error::other("the folder is in use"))
        } else {
            fs::rename(from, to)
        }
    })
    .unwrap_err();
    assert!(err.to_string().contains("left as it was"), "{err}");
    assert!(err.to_string().contains("the folder is in use"), "{err}");
    assert!(!worlds::saves_dir(&dirs).join("Elsewhere").exists());
    assert_eq!(level_name("Base (2)").as_deref(), Some("Base"));
    assert_eq!(worlds::backups(&dirs, "Base (2)").len(), 1);

    // If even moving the world back fails, the error says where everything is.
    let world = worlds::saves_dir(&dirs).join("Elsewhere");
    let err = worlds::rename_with(&dirs, "Base (2)", "Elsewhere", |from, to| {
        if from == backups || from == world {
            Err(std::io::Error::other("the folder is in use"))
        } else {
            fs::rename(from, to)
        }
    })
    .unwrap_err();
    assert!(err.to_string().contains("couldn't put it back"), "{err}");
    assert!(
        err.to_string().contains(&world.display().to_string()),
        "{err}"
    );
    assert!(world.is_dir());
    assert_eq!(worlds::backups(&dirs, "Base (2)").len(), 1);
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

    // A cancelled backup leaves no file.
    let cancelled = CancelToken::new();
    cancelled.cancel();
    assert!(matches!(
        worlds::backup(&dirs, "World", &progress, &cancelled),
        Err(WorldError::Cancelled)
    ));
    assert!(worlds::backups(&dirs, "World").is_empty());
    assert!(
        fs::read_dir(worlds::backups_dir(&dirs).join("World"))
            .map(|d| d.count())
            .unwrap_or(0)
            == 0
    );
}
