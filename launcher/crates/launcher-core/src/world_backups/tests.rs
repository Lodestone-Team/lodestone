//! World backups on real instance folders in temporary data folders, with fake worlds
//! (a real `level.dat` and region files). The game never runs.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::game::loaders::{LoaderChoice, LoaderKind};
use crate::images::swap::Faults;
use crate::images::{NEVER, SnapshotKind, Take};

const LEVEL_DAT: &[u8] =
    include_bytes!("../../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");

struct Setup {
    _temp: tempfile::TempDir,
    data: DataDir,
    images: Arc<Images>,
    backups: Arc<WorldBackups>,
    store: InstanceStore,
    id: String,
}

impl Setup {
    fn new() -> Setup {
        let temp = tempfile::tempdir().unwrap();
        let data = DataDir::at(temp.path().join("data"));
        let store = InstanceStore::new(data.instances_dir());
        let instance = store
            .create(
                "Pack",
                "1.21.1",
                Some(&LoaderChoice {
                    kind: LoaderKind::Fabric,
                    version: "0.16.10".into(),
                }),
            )
            .unwrap();
        let images = Images::open(&data);
        let backups = WorldBackups::open(&data, images.clone());
        let s = Setup {
            _temp: temp,
            data,
            images,
            backups,
            store,
            id: instance.id,
        };
        s.world("World", b"region v1");
        s.world("Other", b"other v1");
        s.write("mods/a.jar", b"a mod");
        s
    }

    fn game(&self, rel: &str) -> PathBuf {
        self.store.game_dir(&self.id).join(rel)
    }

    fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.game(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn world(&self, folder: &str, region: &[u8]) {
        self.write(&format!("saves/{folder}/level.dat"), LEVEL_DAT);
        self.write(&format!("saves/{folder}/region/r.0.0.mca"), region);
    }

    fn back_up(&self, folder: &str, name: Option<&str>) -> WorldBackup {
        self.backups
            .create(&self.id, folder, name, &Progress::default(), NEVER)
            .unwrap()
    }

    fn restore(&self, id: &str, to: RestoreTo) -> Result<WorldRestored, WorldBackupError> {
        self.backups
            .restore(&self.id, id, to, &|_| {}, &Progress::default(), NEVER)
    }

    fn list(&self) -> Vec<WorldBackup> {
        self.backups.list(&self.id).unwrap()
    }

    fn world_files(&self, folder: &str) -> BTreeMap<String, Vec<u8>> {
        files(&self.game(&format!("saves/{folder}")))
    }

    fn backup_dir(&self) -> PathBuf {
        self.data.world_backups_dir().join(&self.id)
    }

    /// Changes the instance's `created`, as a new instance reusing its id would have.
    fn set_created(&self, created: u64) {
        let path = self.store.dir(&self.id).join("instance.json");
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        json["created"] = created.into();
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    }
}

/// Every file under `dir`, by `/`-separated relative path, with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                found.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    found
}

fn zip_names(path: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().into_owned())
        .collect();
    names.sort();
    names
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn a_world_backup_is_a_plain_zip_of_the_world_listed_newest_first() {
    let s = Setup::new();
    s.write("saves/World/session.lock", b"lock");
    let first = s.back_up("World", None);
    let second = s.back_up("World", Some("  Before the raid  "));
    let other = s.back_up("Other", Some(&"x".repeat(500)));
    assert_eq!(first.name, labels::WORLD_BACKUP);
    assert_eq!(second.name, "Before the raid");
    assert_eq!(other.name.chars().count(), MAX_NAME_CHARS);
    assert_eq!(first.kind, WorldBackupKind::Manual);
    assert_eq!(first.folder, "World");
    assert_eq!(first.level_name.as_deref(), Some("Fixture World"));
    assert_eq!(first.game_version.as_deref(), Some("1.21.11"));
    assert_eq!(first.files, 2);
    assert_eq!(first.left_out_count, 0);
    let instance = s.store.get(&s.id).unwrap();
    assert_eq!(first.instance_created, instance.created);
    assert_eq!(first.instance_name, "Pack");

    let listed = s.list();
    assert_eq!(listed.len(), 3);
    // newest first (one second apart at most: ids break ties)
    assert!(
        listed
            .windows(2)
            .all(|w| (w[0].created, &w[0].id) >= (w[1].created, &w[1].id))
    );
    assert!(listed.contains(&first) && listed.contains(&second));

    let zip = s.backups.zip_path(&s.id, &first);
    assert_eq!(
        zip,
        s.backup_dir()
            .join("World")
            .join(format!("{}.zip", first.id))
    );
    assert_eq!(
        zip_names(&zip),
        [
            "World/",
            "World/level.dat",
            "World/region/",
            "World/region/r.0.0.mca"
        ]
    );
    assert_eq!(first.bytes, std::fs::metadata(&zip).unwrap().len());
    // Outside the instance folder, so snapshots never take them.
    assert!(!zip.starts_with(s.store.dir(&s.id)));
    let snapshot = s
        .images
        .take_snapshot(&s.id, Take::new(SnapshotKind::Manual, "All"))
        .unwrap()
        .unwrap();
    let image = s
        .images
        .store()
        .load_manifest(&s.id, &snapshot.image)
        .unwrap();
    assert!(
        !image
            .entries
            .iter()
            .any(|e| e.path.as_str().ends_with(".zip"))
    );
}

#[test]
fn a_cancelled_or_failed_backup_leaves_nothing() {
    let s = Setup::new();
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), &|| true)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Cancelled), "{err:?}");
    assert!(s.list().is_empty());
    assert!(files(&s.backup_dir()).is_empty(), "no .part, no zip");

    let err = s
        .backups
        .create(&s.id, "Missing", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::NotFound(_)), "{err:?}");
    let err = s
        .backups
        .create(&s.id, "../World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::NotFound(_)), "{err:?}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let region = s.game("saves/World/region/r.0.0.mca");
        std::fs::set_permissions(&region, std::fs::Permissions::from_mode(0o000)).unwrap();
        let failed = s
            .backups
            .create(&s.id, "World", None, &Progress::default(), NEVER);
        std::fs::set_permissions(&region, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(failed.is_err());
        assert!(s.list().is_empty());
        assert!(files(&s.backup_dir()).is_empty());
    }
}

#[test]
fn a_backup_is_refused_while_the_game_runs_without_room_or_during_a_data_move() {
    let s = Setup::new();
    let slot = s.images.slot(&s.id);
    slot.session.fetch_add(1, Ordering::SeqCst);
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Running), "{err:?}");
    slot.session_over();

    *s.backups.fake_free.lock().unwrap() = Some(1000);
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::NoRoom { .. }), "{err:?}");
    *s.backups.fake_free.lock().unwrap() = None;

    let suspension = s.images.suspend();
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Suspended), "{err:?}");
    drop(suspension);
    assert!(s.list().is_empty());
    s.back_up("World", None);
}

/// Holds `lock` locked as the game does, from another process, until dropped.
#[cfg(unix)]
struct HeldLock(
    std::process::Child,
    // Kept open until the child ends, so its own output never meets a closed pipe.
    #[allow(dead_code)] std::io::BufReader<std::process::ChildStdout>,
);

#[cfg(unix)]
impl HeldLock {
    fn new(lock: &Path) -> HeldLock {
        use std::io::BufRead as _;
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worlds::tests::hold_session_lock_helper",
                "--nocapture",
            ])
            .env("LODESTONE_TEST_HOLD_LOCK", lock)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut out = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while !line.contains("LOCKED") {
            line.clear();
            assert!(out.read_line(&mut line).unwrap() > 0, "the helper ended");
        }
        HeldLock(child, out)
    }
}

#[cfg(unix)]
impl Drop for HeldLock {
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        let _ = self.0.wait();
    }
}

#[cfg(unix)]
#[test]
fn a_world_the_game_has_open_is_neither_backed_up_nor_restored() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/session.lock", b"lock");
    s.write("saves/Other/session.lock", b"lock");
    let held = HeldLock::new(&s.game("saves/World/session.lock"));
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Running), "{err:?}");
    // Another world can be backed up, but nothing restored while any world is open.
    s.back_up("Other", None);
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(matches!(err, WorldBackupError::Running), "{err:?}");
    drop(held);
    s.restore(&backup.id, RestoreTo::InPlace).unwrap();
}

#[test]
fn backups_follow_the_instance_by_id_and_created_time() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let created = s.store.get(&s.id).unwrap().created;
    // A new instance that reuses the id sees nothing, and the old ones are a deleted
    // instance's.
    s.set_created(created + 1000);
    assert!(s.list().is_empty());
    let deleted = s.backups.list_deleted();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].instance_id, s.id);
    assert_eq!(deleted[0].instance_created, created);
    assert_eq!(deleted[0].backups, std::slice::from_ref(&backup));
    assert!(matches!(
        s.backups.delete_with(&s.id, &backup.id, |_| Ok(())),
        Err(WorldBackupError::NotFound(_))
    ));
    s.set_created(created);

    // In the Trash and back: the same id and created time.
    let aside = s._temp.path().join("trash-copy");
    std::fs::rename(s.store.dir(&s.id), &aside).unwrap();
    assert_eq!(s.backups.list_deleted().len(), 1);
    assert_eq!(WorldBackups::usage(&s.data, &s.store.list().0).deleted, 1);
    std::fs::rename(&aside, s.store.dir(&s.id)).unwrap();
    assert!(s.backups.list_deleted().is_empty());
    assert_eq!(s.list(), std::slice::from_ref(&backup));
    s.write("saves/World/region/r.0.0.mca", b"changed");
    s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    assert_eq!(
        std::fs::read(s.game("saves/World/region/r.0.0.mca")).unwrap(),
        b"region v1"
    );
}

#[test]
fn cleaning_deleted_instances_backups_leaves_live_and_broken_ones() {
    let s = Setup::new();
    let live = s.back_up("World", None);
    let other = s.store.create("Gone", "1.21.1", None).unwrap();
    let broken = s.store.create("Broken", "1.21.1", None).unwrap();
    for id in [&other.id, &broken.id] {
        let world = s.store.game_dir(id).join("saves/W");
        std::fs::create_dir_all(world.join("region")).unwrap();
        std::fs::write(world.join("level.dat"), LEVEL_DAT).unwrap();
        s.backups
            .create(id, "W", None, &Progress::default(), NEVER)
            .unwrap();
    }
    std::fs::remove_dir_all(s.store.dir(&other.id)).unwrap();
    std::fs::write(s.store.dir(&broken.id).join("instance.json"), b"not json").unwrap();

    let deleted = s.backups.list_deleted();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].instance_id, other.id);
    assert_eq!(deleted[0].instance_name, "Gone");
    let usage = WorldBackups::usage(&s.data, &s.store.list().0);
    assert_eq!((usage.backups, usage.deleted), (1, 1));
    assert!(usage.deleted_bytes > 0 && usage.bytes > usage.deleted_bytes);

    let cleaned = s.backups.clean_deleted().unwrap();
    assert_eq!(cleaned.backups, 1);
    assert!(cleaned.bytes > 0);
    assert!(!s.data.world_backups_dir().join(&other.id).exists());
    assert_eq!(s.list(), [live]);
    assert!(
        s.data
            .world_backups_dir()
            .join(&broken.id)
            .join("W")
            .is_dir()
    );
    assert_eq!(
        s.backups.clean_deleted().unwrap(),
        CleanedWorldBackups::default()
    );
}

#[test]
fn deleting_moves_the_zip_away_then_forgets_it() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let zip = s.backups.zip_path(&s.id, &backup);
    let err = s
        .backups
        .delete_with(&s.id, &backup.id, |_| Err("no Trash here".into()))
        .unwrap_err();
    assert!(err.to_string().contains("no Trash here"));
    assert_eq!(s.list(), std::slice::from_ref(&backup));
    let mut removed = Vec::new();
    s.backups
        .delete_with(&s.id, &backup.id, |path| {
            removed.push(path.to_path_buf());
            std::fs::remove_file(path).map_err(|e| e.to_string())
        })
        .unwrap();
    assert_eq!(removed, [zip]);
    assert!(s.list().is_empty());
    assert!(!s.backup_dir().exists(), "empty folders go too");
    assert!(matches!(
        s.backups.delete_with(&s.id, "../x", |_| Ok(())),
        Err(WorldBackupError::NotFound(_))
    ));
}

#[test]
fn the_sweep_clears_crash_leftovers_and_keeps_orphan_zips() {
    let s = Setup::new();
    let backup = s.back_up("World", Some("Mine"));
    let dir = s.backup_dir().join("World");
    let zip = dir.join(format!("{}.zip", backup.id));
    // a zip whose .json was never written: the crash came between them
    let orphan = dir.join("20260101-000000-abcd.zip");
    std::fs::copy(&zip, &orphan).unwrap();
    // a .json whose zip is gone: a crash during a delete
    std::fs::write(dir.join("20260101-000000-0000.json"), b"{}").unwrap();
    // .part files: one old (a crash), one new (another Launcher's? never touched)
    let old_part = dir.join(".x.zip.0000000000000000.part");
    let new_part = dir.join(".y.zip.0000000000000000.part");
    std::fs::write(&old_part, b"half").unwrap();
    std::fs::write(&new_part, b"half").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&old_part)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(2 * 60 * 60))
        .unwrap();
    let mut changes = s.backups.watch();
    s.backups.sweep();
    assert!(!old_part.exists());
    assert!(new_part.exists());
    assert!(!dir.join("20260101-000000-0000.json").exists());
    let listed = s.list();
    assert_eq!(listed.len(), 2);
    let adopted = listed
        .iter()
        .find(|b| b.id == "20260101-000000-abcd")
        .unwrap();
    assert_eq!(adopted.name, labels::WORLD_BACKUP);
    assert_eq!(adopted.level_name.as_deref(), Some("Fixture World"));
    assert_eq!(adopted.files, 2);
    assert_eq!(changes.try_recv().ok().as_deref(), Some(s.id.as_str()));
}

#[test]
fn hostile_or_damaged_details_are_ignored() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let dir = s.backup_dir().join("World");
    let zip = dir.join(format!("{}.zip", backup.id));
    let sidecar = std::fs::read(dir.join(format!("{}.json", backup.id))).unwrap();
    // too large
    let big = dir.join("20260101-000000-0001");
    std::fs::copy(&zip, big.with_extension("zip")).unwrap();
    std::fs::write(big.with_extension("json"), vec![b' '; 70_000]).unwrap();
    // an id that isn't one, or names another backup
    std::fs::copy(&zip, dir.join("evil.zip")).unwrap();
    std::fs::write(dir.join("evil.json"), &sidecar).unwrap();
    std::fs::copy(&zip, dir.join("20260101-000000-0002.zip")).unwrap();
    std::fs::write(dir.join("20260101-000000-0002.json"), &sidecar).unwrap();
    // a folder that isn't a world's
    let hidden = s.backup_dir().join(".hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::copy(&zip, hidden.join(format!("{}.zip", backup.id))).unwrap();
    std::fs::write(hidden.join(format!("{}.json", backup.id)), &sidecar).unwrap();
    // The folder it is in wins over the folder it names.
    let moved = s.backup_dir().join("Other");
    std::fs::create_dir_all(&moved).unwrap();
    std::fs::copy(&zip, moved.join("20260101-000000-0003.zip")).unwrap();
    let mut other: WorldBackup = serde_json::from_slice(&sidecar).unwrap();
    other.id = "20260101-000000-0003".into();
    other.folder = "../../escape".into();
    std::fs::write(
        moved.join("20260101-000000-0003.json"),
        serde_json::to_vec(&other).unwrap(),
    )
    .unwrap();
    let listed = s.list();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(
        listed.iter().find(|b| b.id == other.id).unwrap().folder,
        "Other"
    );
}

#[test]
fn restoring_in_place_saves_the_world_first_and_changes_only_that_world() {
    let s = Setup::new();
    s.write("saves/World/data/raids.dat", b"raids v1");
    let backup = s.back_up("World", Some("Base"));
    let before = s.world_files("World");
    let other_before = s.world_files("Other");
    // Changed since: a file changed, one added, one removed, and the lock the game keeps.
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    s.write("saves/World/region/r.1.0.mca", b"added since");
    std::fs::remove_file(s.game("saves/World/data/raids.dat")).unwrap();
    s.write("saves/World/session.lock", b"lock");
    s.write("saves/Other/region/r.0.0.mca", b"other v2");
    s.write("mods/b.jar", b"another mod");
    let mut changes = s.backups.watch();

    let restored = s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    assert_eq!(restored.folder, "World");
    assert_eq!(restored.kept_aside, None);
    let mut expected = before.clone();
    expected.insert("session.lock".into(), b"lock".to_vec());
    assert_eq!(s.world_files("World"), expected);
    assert_ne!(
        s.world_files("Other"),
        other_before,
        "other worlds untouched"
    );
    assert_eq!(
        std::fs::read(s.game("saves/Other/region/r.0.0.mca")).unwrap(),
        b"other v2"
    );
    assert!(s.game("mods/b.jar").is_file());

    let safety = restored.safety.unwrap();
    assert_eq!(safety.kind, WorldBackupKind::BeforeRestore);
    assert_eq!(safety.name, labels::before_restoring("Base"));
    assert_eq!(safety.restored.as_deref(), Some("Base"));
    assert_eq!(
        zip_names(&s.backups.zip_path(&s.id, &safety)),
        [
            "World/",
            "World/data/",
            "World/level.dat",
            "World/region/",
            "World/region/r.0.0.mca",
            "World/region/r.1.0.mca"
        ]
    );
    assert_eq!(s.list().len(), 2);
    assert_eq!(changes.try_recv().ok().as_deref(), Some(s.id.as_str()));
    // Nothing of the restore's work is left.
    let leftovers: Vec<_> = std::fs::read_dir(s.store.dir(&s.id))
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".lodestone-restore")
        })
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // Back to how it was before the restore, from the safety backup.
    s.restore(&safety.id, RestoreTo::InPlace).unwrap();
    assert_eq!(
        std::fs::read(s.game("saves/World/region/r.1.0.mca")).unwrap(),
        b"added since"
    );
}

#[test]
fn what_is_written_into_the_world_during_a_restore_is_kept_aside() {
    // A file that appears in the world after the safety backup listed what the restore
    // replaces, and after the restore checked what it carries back, is neither replaced nor
    // carried: the swap keeps it in `restore-leftovers-<16>/` rather than deleting it.
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let late = s.game("saves/World/data/late.dat");
    *s.backups.before_swap.lock().unwrap() = Some(Box::new(move || {
        std::fs::create_dir_all(late.parent().unwrap()).unwrap();
        std::fs::write(&late, b"written meanwhile").unwrap();
    }));

    let restored = s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    let kept = restored.kept_aside.expect("kept aside");
    assert_eq!(kept.parent(), Some(s.store.dir(&s.id).as_path()));
    assert!(
        kept.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(crate::images::swap::LEFTOVERS_PREFIX),
        "{kept:?}"
    );
    assert_eq!(
        std::fs::read(kept.join("minecraft/saves/World/data/late.dat")).unwrap(),
        b"written meanwhile"
    );
    // The world is the backup's; the safety backup has what it replaced.
    assert!(!s.game("saves/World/data/late.dat").exists());
    assert_eq!(s.world_files("World")["region/r.0.0.mca"], b"region v1");
    let safety = restored.safety.unwrap();
    assert!(
        !zip_names(&s.backups.zip_path(&s.id, &safety)).contains(&"World/data/late.dat".into())
    );
}

#[test]
fn every_folder_a_backup_takes_restores_and_the_rest_are_refused_first() {
    let s = Setup::new();
    // Folder names `is_plain_name` finds but no zip restore takes (on Windows the disk
    // can't hold the dotted ones, and `world_dir` refuses them).
    let mut refused = vec!["aux", "com1", "nul.txt", "CON"];
    if !cfg!(windows) {
        refused.extend(["World.", "World "]);
    }
    for folder in refused {
        s.world(folder, b"region");
        let err = s
            .backups
            .create(&s.id, folder, None, &Progress::default(), NEVER)
            .unwrap_err();
        assert!(
            err.to_string().contains("can't be restored from a zip"),
            "{folder:?}: {err}"
        );
    }
    assert!(s.list().is_empty(), "nothing listed");
    assert!(
        files(&s.backup_dir()).is_empty(),
        "nothing written: {:?}",
        files(&s.backup_dir())
    );

    // Names inside a world the zip check refuses are left out, so the zip still restores.
    let long = "d".repeat(250);
    s.world("Odd name (1) é", b"region v1");
    s.write("saves/Odd name (1) é/data/a..b.dat", b"two dots");
    s.write(
        &format!("saves/Odd name (1) é/{long}/{long}/x.dat"),
        b"too long a zip name",
    );
    s.write("saves/Odd name (1) é/data/kept.dat", b"kept");
    let backup = s.back_up("Odd name (1) é", None);
    assert_eq!(backup.left_out_count, 2, "{:?}", backup.left_out);
    let zip = s.backups.zip_path(&s.id, &backup);
    worlds::zip::check(&zip, Shape::OneWorldFolder, worlds::Limits::default()).unwrap();
    s.write("saves/Odd name (1) é/region/r.0.0.mca", b"region v2");
    s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    let world = s.world_files("Odd name (1) é");
    assert_eq!(world["region/r.0.0.mca"], b"region v1");
    assert_eq!(world["data/kept.dat"], b"kept");
    let copy = s.restore(&backup.id, RestoreTo::NewWorld).unwrap();
    assert_eq!(s.world_files(&copy.folder)["data/kept.dat"], b"kept");
}

#[test]
fn a_world_deleted_since_comes_back_without_a_safety_backup() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let before = s.world_files("World");
    std::fs::remove_dir_all(s.game("saves/World")).unwrap();
    let restored = s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    assert_eq!(restored.safety, None);
    assert_eq!(s.world_files("World"), before);
    assert_eq!(s.list().len(), 1);
}

#[test]
fn a_restore_is_refused_before_anything_changes() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let now = s.world_files("World");

    let slot = s.images.slot(&s.id);
    slot.session.fetch_add(1, Ordering::SeqCst);
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(matches!(err, WorldBackupError::Running), "{err:?}");
    slot.session_over();

    let zip = s.backups.zip_path(&s.id, &backup);
    let good = std::fs::read(&zip).unwrap();
    type Entries<'a> = &'a [(&'a str, &'a [u8])];
    let hostile: &[(&str, Entries)] = &[
        (
            "escape",
            &[("World/level.dat", LEVEL_DAT), ("World/../../x", b"x")],
        ),
        (
            "absolute",
            &[("World/level.dat", LEVEL_DAT), ("/etc/x", b"x")],
        ),
        (
            "two worlds",
            &[
                ("World/level.dat", LEVEL_DAT),
                ("Else/level.dat", LEVEL_DAT),
            ],
        ),
        (
            "case",
            &[
                ("World/level.dat", LEVEL_DAT),
                ("World/region/a.mca", b"a"),
                ("World/REGION/b.mca", b"b"),
            ],
        ),
        ("not a world", &[("World/region/r.0.0.mca", b"x")]),
    ];
    for (what, entries) in hostile {
        write_zip(&zip, entries);
        assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err(), "{what}");
        assert!(
            s.restore(&backup.id, RestoreTo::NewWorld).is_err(),
            "{what}"
        );
        assert_eq!(s.world_files("World"), now, "{what}");
        assert_eq!(s.list().len(), 1, "{what}: no safety backup was made");
    }
    std::fs::write(&zip, b"not a zip").unwrap();
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    std::fs::write(&zip, &good).unwrap();

    *s.backups.fake_free.lock().unwrap() = Some(10);
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(matches!(err, WorldBackupError::NoRoom { .. }), "{err:?}");
    *s.backups.fake_free.lock().unwrap() = None;
    assert_eq!(s.world_files("World"), now);
    assert!(matches!(
        s.restore("20260101-000000-ffff", RestoreTo::InPlace),
        Err(WorldBackupError::NotFound(_))
    ));
}

#[test]
fn cancel_while_saving_or_unpacking_changes_nothing() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let now = s.world_files("World");
    let err = s
        .backups
        .restore(
            &s.id,
            &backup.id,
            RestoreTo::InPlace,
            &|_| {},
            &Progress::default(),
            &|| true,
        )
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Cancelled), "{err:?}");
    assert_eq!(s.list().len(), 1, "no half safety backup");
    assert!(!files(&s.backup_dir()).keys().any(|k| k.ends_with(".part")));
    assert_eq!(s.world_files("World"), now);

    let unpacking = AtomicBool::new(false);
    let err = s
        .backups
        .restore(
            &s.id,
            &backup.id,
            RestoreTo::InPlace,
            &|step| unpacking.store(step == WorldStep::Unpacking, Ordering::SeqCst),
            &Progress::default(),
            &|| unpacking.load(Ordering::SeqCst),
        )
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Cancelled), "{err:?}");
    assert_eq!(s.world_files("World"), now);
    assert!(
        !std::fs::read_dir(s.store.dir(&s.id))
            .unwrap()
            .flatten()
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".lodestone-restore"))
    );
}

#[test]
fn a_crash_during_the_swap_is_undone_at_the_next_start() {
    let mut crashed = 0;
    for crash_after in 0..6 {
        let s = Setup::new();
        let backup = s.back_up("World", None);
        s.write("saves/World/region/r.0.0.mca", b"region v2");
        s.write("saves/World/region/r.1.0.mca", b"added");
        let instance_before = files(&s.store.dir(&s.id));
        *s.backups.faults.lock().unwrap() = Some(Faults {
            crash_after: Some(crash_after),
            ..Faults::default()
        });
        if s.restore(&backup.id, RestoreTo::InPlace).is_ok() {
            // fewer renames than that: it finished
            break;
        }
        crashed += 1;
        assert!(s.images.restore_unfinished(&s.id));
        let recovery = s.images.recover_restores();
        assert_eq!(recovery.repaired, std::slice::from_ref(&s.id));
        s.images.empty_restore_trash(Some(&s.id));
        assert!(!s.images.restore_unfinished(&s.id));
        assert_eq!(
            files(&s.store.dir(&s.id)),
            instance_before,
            "after a crash after {crash_after} renames"
        );
    }
    assert!(crashed >= 3, "{crashed}");

    // A rollback that fails waits for Play's retry; a crash once done finishes.
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let at_backup = s.world_files("World");
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let now = s.world_files("World");
    *s.backups.faults.lock().unwrap() = Some(Faults {
        fail_after: Some(1),
        break_rollback: true,
        ..Faults::default()
    });
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    assert!(s.images.restore_unfinished(&s.id));
    s.images
        .retry_unfinished_restore(&s.id, Duration::from_secs(1))
        .unwrap();
    assert!(!s.images.restore_unfinished(&s.id));
    assert_eq!(s.world_files("World"), now);

    *s.backups.faults.lock().unwrap() = Some(Faults {
        crash_when_done: true,
        ..Faults::default()
    });
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    s.images.recover_restores();
    assert_eq!(s.world_files("World"), at_backup);
}

#[test]
fn restoring_as_a_new_world_places_a_copy_beside_it() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let now = s.world_files("World");
    let restored = s.restore(&backup.id, RestoreTo::NewWorld).unwrap();
    let today = worlds::date(SystemTime::now());
    assert_eq!(restored.folder, format!("World (restored {today})"));
    assert_eq!(restored.safety, None);
    assert_eq!(s.world_files("World"), now);
    let copy = s.world_files(&restored.folder);
    assert_eq!(copy["region/r.0.0.mca"], b"region v1");
    let world = worlds::read_world(&s.game(&format!("saves/{}", restored.folder))).unwrap();
    assert_eq!(world.name(), format!("Fixture World (restored {today})"));
    // again: another folder
    let again = s.restore(&backup.id, RestoreTo::NewWorld).unwrap();
    assert_eq!(again.folder, format!("World (restored {today}) (2)"));
    assert_eq!(s.list().len(), 1);
}

/// Minecraft makes an empty `datapacks/` in every world: a folder the zip lists, empty, is
/// the zip's whether the world on disk has it or not; one only on disk stays.
#[test]
fn empty_folders_restore_whether_in_the_backup_on_disk_or_both() {
    let s = Setup::new();
    for empty in ["datapacks", "DIM1/data", "poi"] {
        std::fs::create_dir_all(s.game(&format!("saves/World/{empty}"))).unwrap();
    }
    let backup = s.back_up("World", None);
    let at_backup = s.world_files("World");
    // Only in the backup now, and only on disk.
    std::fs::remove_dir_all(s.game("saves/World/DIM1")).unwrap();
    std::fs::create_dir_all(s.game("saves/World/entities")).unwrap();
    s.write("saves/World/region/r.0.0.mca", b"region v2");

    let restored = s.restore(&backup.id, RestoreTo::InPlace).unwrap();
    assert_eq!(s.world_files("World"), at_backup);
    for folder in ["datapacks", "DIM1/data", "poi", "entities"] {
        assert!(
            s.game(&format!("saves/World/{folder}")).is_dir(),
            "{folder}"
        );
    }
    // The safety backup, empty folders and all, restores too.
    let safety = restored.safety.unwrap();
    s.restore(&safety.id, RestoreTo::InPlace).unwrap();
    assert_eq!(
        s.world_files("World")["region/r.0.0.mca"],
        b"region v2".to_vec()
    );
}

/// A restore that ends with the world as it was deletes the safety backup it made: each
/// is a full-size zip, and world backups are never pruned. One that changed the world (or
/// may have: a swap that couldn't be undone) keeps it.
#[test]
fn a_restore_that_leaves_the_world_as_it_was_deletes_its_safety_backup() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", b"region v2");
    let now = s.world_files("World");
    let mut changes = s.backups.watch();

    // Cancelled while unpacking.
    let unpacking = AtomicBool::new(false);
    let err = s
        .backups
        .restore(
            &s.id,
            &backup.id,
            RestoreTo::InPlace,
            &|step| unpacking.store(step == WorldStep::Unpacking, Ordering::SeqCst),
            &Progress::default(),
            &|| unpacking.load(Ordering::SeqCst),
        )
        .unwrap_err();
    assert!(matches!(err, WorldBackupError::Cancelled), "{err:?}");
    assert_eq!(s.list(), [backup.clone()]);
    assert_eq!(
        files(&s.backup_dir()).len(),
        2,
        "only the backup's zip and .json"
    );
    // The list followed: made, then deleted.
    assert_eq!(changes.try_recv().ok().as_deref(), Some(s.id.as_str()));
    assert_eq!(changes.try_recv().ok().as_deref(), Some(s.id.as_str()));

    // Rolled back.
    *s.backups.faults.lock().unwrap() = Some(Faults {
        fail_after: Some(1),
        ..Faults::default()
    });
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    assert_eq!(s.list(), [backup.clone()]);
    assert_eq!(s.world_files("World"), now);

    // A game session that began while it unpacked.
    let slot = s.images.slot(&s.id);
    let started = slot.clone();
    *s.backups.before_swap.lock().unwrap() = Some(Box::new(move || {
        started.session.fetch_add(1, Ordering::SeqCst);
    }));
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(matches!(err, WorldBackupError::Running), "{err:?}");
    slot.session_over();
    assert_eq!(s.list(), [backup.clone()]);
    assert_eq!(s.world_files("World"), now);
    assert!(
        !std::fs::read_dir(s.store.dir(&s.id))
            .unwrap()
            .flatten()
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".lodestone-restore")),
        "the staging folder is gone"
    );

    // A swap that couldn't be undone keeps it: the world may be half restored.
    *s.backups.faults.lock().unwrap() = Some(Faults {
        fail_after: Some(1),
        break_rollback: true,
        ..Faults::default()
    });
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    assert_eq!(s.list().len(), 2);
}

/// A link where the backup places a folder is refused before the safety backup is made
/// or anything is unpacked, and nothing is written through it.
#[cfg(unix)]
#[test]
fn a_link_in_the_way_is_refused_before_the_safety_backup() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let elsewhere = s
        .data
        .world_backups_dir()
        .parent()
        .unwrap()
        .join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::remove_dir_all(s.game("saves/World/region")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, s.game("saves/World/region")).unwrap();
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(err.to_string().contains("region"), "{err}");
    assert_eq!(s.list(), [backup], "no safety backup");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
}

/// Renaming a world in the Launcher takes its world backups along; a world later given
/// the old folder name has none of them. The new folder is free in both places.
#[test]
fn renaming_a_world_moves_its_backups() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let mut changes = s.backups.watch();
    let folder = s.backups.rename_world(&s.id, "World", "Base").unwrap();
    assert_eq!(folder, "Base");
    assert!(s.game("saves/Base/level.dat").is_file());
    assert_eq!(changes.try_recv().ok().as_deref(), Some(s.id.as_str()));
    let listed = s.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, backup.id);
    assert_eq!(listed[0].folder, "Base");
    assert!(!s.backup_dir().join("World").exists());

    // A new world given the old name has no backups.
    s.world("World", b"new world");
    assert!(s.list().iter().all(|b| b.folder != "World"));

    // A name only the backups' folder has is taken too.
    std::fs::create_dir_all(s.backup_dir().join("Taken")).unwrap();
    assert_eq!(
        s.backups.rename_world(&s.id, "Base", "Taken").unwrap(),
        "Taken (2)"
    );
    assert_eq!(s.list()[0].folder, "Taken (2)");
    // Refused while the game runs.
    let slot = s.images.slot(&s.id);
    slot.session.fetch_add(1, Ordering::SeqCst);
    assert!(matches!(
        s.backups.rename_world(&s.id, "Taken (2)", "Again"),
        Err(WorldBackupError::Running)
    ));
    slot.session_over();
}

/// A zip found without its `.json` that is older than the instance with that id was made
/// by a deleted instance whose id was reused: it isn't given to the new one.
#[test]
fn the_sweep_never_gives_an_older_instances_zip_to_a_new_one() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let dir = s.backup_dir().join("World");
    let orphan = dir.join("20200101-000000-abcd.zip");
    std::fs::copy(dir.join(format!("{}.zip", backup.id)), &orphan).unwrap();
    let created = s.store.get(&s.id).unwrap().created;
    std::fs::File::options()
        .write(true)
        .open(&orphan)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(created - 1000))
        .unwrap();
    s.backups.sweep();
    assert_eq!(s.list(), [backup]);
    assert!(orphan.exists(), "left alone");
}

#[test]
fn a_folder_without_level_dat_is_not_backed_up_by_hand() {
    let s = Setup::new();
    std::fs::remove_file(s.game("saves/World/level.dat")).unwrap();
    let err = s
        .backups
        .create(&s.id, "World", None, &Progress::default(), NEVER)
        .unwrap_err();
    assert!(err.to_string().contains("level.dat"), "{err}");
    assert!(s.list().is_empty());
}

/// On one disk, a restore needs room for the unpacked world and the safety backup at
/// once, checked before the safety backup is made.
#[test]
fn a_restore_needs_room_for_both_copies_on_one_disk() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    s.write("saves/World/region/r.0.0.mca", &vec![7u8; 100_000]);
    let world = worlds::zip::plan(&s.game("saves/World"), "World")
        .unwrap()
        .bytes();
    let both = worlds::SPARE_BYTES + backup.world_bytes + world;
    *s.backups.fake_free.lock().unwrap() = Some(both - 1);
    let err = s.restore(&backup.id, RestoreTo::InPlace).unwrap_err();
    assert!(matches!(err, WorldBackupError::NoRoom { .. }), "{err:?}");
    assert_eq!(s.list().len(), 1, "refused before the safety backup");
    *s.backups.fake_free.lock().unwrap() = Some(both);
    s.restore(&backup.id, RestoreTo::InPlace).unwrap();
}

/// A world backup's folder whose name no backup would be made under (planted in the data
/// folder) is never restored.
#[test]
fn a_backup_under_a_folder_no_backup_takes_is_never_restored() {
    let s = Setup::new();
    let backup = s.back_up("World", None);
    let planted = s.backup_dir().join("aux");
    std::fs::create_dir_all(&planted).unwrap();
    for ext in ["zip", "json"] {
        std::fs::rename(
            s.backup_dir()
                .join("World")
                .join(format!("{}.{ext}", backup.id)),
            planted.join(format!("{}.{ext}", backup.id)),
        )
        .unwrap();
    }
    assert!(s.restore(&backup.id, RestoreTo::InPlace).is_err());
    assert!(s.restore(&backup.id, RestoreTo::NewWorld).is_err());
    assert!(!s.game("saves/aux").exists());
}
