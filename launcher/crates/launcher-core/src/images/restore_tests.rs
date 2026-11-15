//! Tests of going back to snapshots: each scope, the safety snapshot, what is carried, the
//! journaled swap's crash points and recovery, on real instance folders in temporary data
//! folders.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use lodestone_image::Base;
use lodestone_image::store::NEVER;

use super::restore::RestoreScope;
use super::snapshots::{SnapshotKind, Take};
use super::swap::{self, Staged, SwapError};
use super::tests::Setup;
use super::*;
use crate::modpack::{FileSource, ModpackRecord, PackFile};

/// Every file under `dir`, by path relative to it, with its bytes.
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

impl Setup {
    fn read(&self, relative: &str) -> Option<Vec<u8>> {
        std::fs::read(self.game(relative)).ok()
    }

    fn restore(&self, id: &str, scope: RestoreScope) -> Result<restore::Restored, ImageError> {
        self.images.restore(self.id(), id, &scope, NEVER)
    }
}

/// An instance with two worlds, a mod, a config and options, recorded and snapshotted.
fn played() -> (Setup, SnapshotRecord) {
    let s = Setup::new();
    s.write("mods/a.jar", b"mod a v1");
    s.write("config/a.toml", b"config v1");
    s.write("options.txt", b"fov:70");
    s.world("World", b"world v1");
    s.world("Other", b"other v1");
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let record = s.snapshot("Before");
    (s, record)
}

/// Changes everything the snapshot holds, and adds what it doesn't.
fn change_everything(s: &Setup) {
    s.write("mods/a.jar", b"mod a v2");
    s.write("mods/b.jar", b"mod b");
    s.write("config/a.toml", b"config v2");
    s.write("options.txt", b"fov:110");
    s.write("saves/World/region/r.0.0.mca", b"world v2");
    s.write("saves/World/region/r.1.0.mca", b"new region");
    s.write("saves/Other/region/r.0.0.mca", b"other v2");
    s.world("Added", b"added since");
    s.write("logs/latest.log", b"a log");
    // What no snapshot takes, inside a world the restore replaces.
    s.write("saves/World/session.lock", b"lock");
}

#[test]
fn only_the_worlds_puts_back_those_worlds_alone() {
    let (s, record) = played();
    change_everything(&s);
    let restored = s
        .restore(&record.id, RestoreScope::Worlds(vec!["World".into()]))
        .unwrap();
    let safety = restored.safety.unwrap();
    assert_eq!(safety.kind, SnapshotKind::BeforeRestore);
    assert_eq!(safety.name, "Before going back to \u{201c}Before\u{201d}");
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v1");
    assert_eq!(s.read("saves/World/region/r.1.0.mca"), None);
    assert_eq!(s.read("saves/World/session.lock").unwrap(), b"lock");
    // Nothing else moved.
    assert_eq!(s.read("saves/Other/region/r.0.0.mca").unwrap(), b"other v2");
    assert_eq!(s.read("mods/a.jar").unwrap(), b"mod a v2");
    assert_eq!(s.read("options.txt").unwrap(), b"fov:110");
    assert!(s.read("saves/Added/level.dat").is_some());
    // No work folders are left.
    let leftovers: Vec<String> = std::fs::read_dir(s.store.dir(s.id()))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".lodestone-restore"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // The safety snapshot holds what was replaced: going back to it undoes the restore.
    s.restore(&safety.id, RestoreScope::Worlds(vec!["World".into()]))
        .unwrap();
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v2");
    assert_eq!(
        s.read("saves/World/region/r.1.0.mca").unwrap(),
        b"new region"
    );
}

#[test]
fn the_worlds_and_their_mods_put_back_the_linked_content() {
    let (s, record) = played();
    change_everything(&s);
    let preview = s.images.preview_restore(s.id(), &record.id).unwrap();
    assert!(preview.can_restore_content);
    assert!(!preview.other_game);
    assert!(preview.missing.is_empty());
    // The content image is still the one the snapshot links to (the changes weren't
    // recorded yet), so the preview sees none; recording them shows the difference.
    s.images.check_now(s.id()).unwrap();
    let preview = s.images.preview_restore(s.id(), &record.id).unwrap();
    let diff = preview.content.expect("the mods differ");
    assert_eq!(diff.removed, ["mods/b.jar"]);
    assert!(diff.updated.contains(&"mods/a.jar".to_string()));
    assert!(diff.updated.contains(&"config/a.toml".to_string()));
    let names: Vec<&str> = preview.worlds.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["Other", "World"]);

    let restored = s
        .restore(
            &record.id,
            RestoreScope::WorldsAndContent(vec!["World".into()]),
        )
        .unwrap();
    assert_eq!(s.read("mods/a.jar").unwrap(), b"mod a v1");
    assert_eq!(s.read("mods/b.jar"), None);
    assert_eq!(s.read("config/a.toml").unwrap(), b"config v1");
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v1");
    // Not the other world, nor what isn't content.
    assert_eq!(s.read("saves/Other/region/r.0.0.mca").unwrap(), b"other v2");
    assert_eq!(s.read("options.txt").unwrap(), b"fov:110");
    // Recorded, as an image with the snapshot's content.
    let recorded = restored.recorded.expect("the content changed");
    assert_eq!(Some(recorded.id.clone()), record.content);
    assert_eq!(s.labels()[0], labels::reverted("Before"));
    // The safety snapshot links the content as it was before.
    let safety = restored.safety.unwrap();
    let before = s.image(safety.content.as_deref().unwrap());
    assert!(
        before
            .entries
            .iter()
            .any(|e| e.path.as_str() == "mods/b.jar")
    );
}

#[test]
fn everything_puts_back_the_whole_game_folder() {
    let (s, record) = played();
    change_everything(&s);
    s.restore(&record.id, RestoreScope::Everything).unwrap();
    assert_eq!(s.read("mods/a.jar").unwrap(), b"mod a v1");
    assert_eq!(s.read("mods/b.jar"), None);
    assert_eq!(s.read("options.txt").unwrap(), b"fov:70");
    assert_eq!(s.read("saves/Other/region/r.0.0.mca").unwrap(), b"other v1");
    assert_eq!(
        s.read("saves/Added/level.dat"),
        None,
        "a world added since goes"
    );
    // What no snapshot takes stays.
    assert_eq!(s.read("logs/latest.log").unwrap(), b"a log");
    assert_eq!(s.read("saves/World/session.lock").unwrap(), b"lock");
}

#[test]
fn a_new_instance_from_a_snapshot() {
    let (s, record) = played();
    change_everything(&s);
    let made = s
        .images
        .restore_as_new(s.id(), &record.id, "Pack again", NEVER)
        .unwrap();
    assert_ne!(made.id, s.id());
    assert_eq!(made.name, "Pack again");
    assert_eq!(made.game_version, "1.21.1");
    let game = s.store.game_dir(&made.id);
    assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"mod a v1");
    assert_eq!(
        std::fs::read(game.join("saves/World/region/r.0.0.mca")).unwrap(),
        b"world v1"
    );
    assert!(!game.join("mods/b.jar").exists());
    // Its first image is recorded.
    let history = s.images.history(&made.id).unwrap();
    assert_eq!(
        history.images[0].label.as_deref(),
        Some(labels::FROM_SNAPSHOT)
    );
    // The original is untouched.
    assert_eq!(s.read("mods/a.jar").unwrap(), b"mod a v2");
}

#[test]
fn a_world_put_back_as_a_copy_replaces_nothing() {
    let (s, record) = played();
    change_everything(&s);
    let folder = s
        .images
        .restore_world_copy(s.id(), &record.id, "World", NEVER)
        .unwrap();
    assert!(folder.starts_with("World (restored "), "{folder}");
    assert_eq!(
        s.read(&format!("saves/{folder}/region/r.0.0.mca")).unwrap(),
        b"world v1"
    );
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v2");
    // Again: another free name.
    let again = s
        .images
        .restore_world_copy(s.id(), &record.id, "World", NEVER)
        .unwrap();
    assert_ne!(again, folder);
}

#[test]
fn restores_are_refused_when_they_would_be_unsafe() {
    let (s, record) = played();
    // While the game runs.
    let _ = s.images.before_launch(s.id());
    assert!(matches!(
        s.restore(&record.id, RestoreScope::Everything),
        Err(ImageError::Running)
    ));
    s.images.end_session(s.id());
    // A world the snapshot doesn't have.
    assert!(
        s.restore(&record.id, RestoreScope::Worlds(vec!["Nope".into()]))
            .is_err()
    );
    // Content while a Find it for me search is saved.
    let finder = s
        .store
        .dir(s.id())
        .join(crate::content::mod_finder::FILE_NAME);
    std::fs::write(&finder, b"{}").unwrap();
    assert!(s.restore(&record.id, RestoreScope::Everything).is_err());
    // Worlds alone are fine then.
    s.restore(&record.id, RestoreScope::Worlds(vec!["World".into()]))
        .unwrap();
    std::fs::remove_file(&finder).unwrap();
    // A snapshot of another Minecraft version only makes a new instance.
    let mut instance = s.store.get(s.id()).unwrap();
    instance.game_version = "1.20.1".into();
    s.store.save(&instance).unwrap();
    let preview = s.images.preview_restore(s.id(), &record.id).unwrap();
    assert!(preview.other_game);
    assert!(
        s.restore(
            &record.id,
            RestoreScope::WorldsAndContent(vec!["World".into()])
        )
        .is_err()
    );
    s.restore(&record.id, RestoreScope::Worlds(vec!["World".into()]))
        .unwrap();
}

#[cfg(unix)]
#[test]
fn a_file_the_safety_snapshot_couldnt_read_is_never_replaced() {
    use std::os::unix::fs::PermissionsExt as _;
    let (s, record) = played();
    s.write("saves/World/region/r.0.0.mca", b"world v2");
    let file = s.game("saves/World/region/r.0.0.mca");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let refused = s.restore(&record.id, RestoreScope::Worlds(vec!["World".into()]));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v2");
}

#[test]
fn names_that_clash_on_a_case_insensitive_disk_are_refused() {
    let (s, record) = played();
    // Does this disk ignore letter case?
    let probe = s.game("CaseProbe");
    std::fs::write(&probe, b"x").unwrap();
    let ignores_case = s.game("caseprobe").exists();
    std::fs::remove_file(&probe).unwrap();
    if !ignores_case {
        return;
    }
    // A snapshot made elsewhere could hold two names this disk takes for one.
    let store = s.images.store();
    let mut image = store.load_manifest(s.id(), &record.image).unwrap();
    let mut twin = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "saves/World/level.dat")
        .unwrap()
        .clone();
    twin.path = lodestone_image::CanonicalPath::new("saves/World/LEVEL.DAT").unwrap();
    image.entries.push(twin);
    image.normalize();
    image.seal();
    store.save_manifest(s.id(), &image).unwrap();
    let clash = s
        .records()
        .into_iter()
        .find(|r| r.image == image.id)
        .expect("listed again as a manual snapshot");
    s.write("saves/World/region/r.0.0.mca", b"world v2");
    assert!(
        s.restore(&clash.id, RestoreScope::Worlds(vec!["World".into()]))
            .is_err()
    );
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"world v2");
}

/// An instance folder with owned paths, staged replacements, and a carried file, for the
/// swap's crash tests.
fn swap_fixture(root: &Path) -> (Staged, BTreeMap<String, Vec<u8>>) {
    let write = |rel: &str, bytes: &[u8]| {
        let path = swap::join_rel(root, rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    };
    write("minecraft/saves/World/level.dat", b"old level");
    write("minecraft/saves/World/region/r.0.0.mca", b"old region");
    write("minecraft/saves/World/session.lock", b"lock");
    write("minecraft/mods/a.jar", b"old mod");
    write("minecraft/options.txt", b"old options");
    let before = files(root);
    let suffix = swap::random_suffix();
    let staging = format!("{}{suffix}", swap::STAGING_PREFIX);
    write(
        &format!("{staging}/minecraft/saves/World/level.dat"),
        b"new level",
    );
    write(
        &format!("{staging}/minecraft/saves/World/region/r.0.0.mca"),
        b"new region",
    );
    write(&format!("{staging}/minecraft/mods/b.jar"), b"new mod");
    let owned = vec![
        "minecraft/mods".to_string(),
        "minecraft/saves/World".to_string(),
        "minecraft/shaderpacks".to_string(),
    ];
    let replaced: HashSet<String> = [
        "minecraft/saves/World/level.dat",
        "minecraft/saves/World/region/r.0.0.mca",
        "minecraft/mods/a.jar",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    let placed = vec![
        "minecraft/saves/World/level.dat".to_string(),
        "minecraft/saves/World/region/r.0.0.mca".to_string(),
        "minecraft/mods/b.jar".to_string(),
    ];
    let staged = Staged::new(root, suffix, &owned, &replaced, &placed).unwrap();
    (staged, before)
}

#[test]
fn a_swap_replaces_what_it_owns_and_carries_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("instance");
    let (staged, _) = swap_fixture(&root);
    let journal = dir.path().join("journal.json");
    staged.swap(&journal).unwrap();
    assert!(!journal.exists());
    let now = files(&root);
    let expected: BTreeMap<String, Vec<u8>> = [
        ("minecraft/saves/World/level.dat", &b"new level"[..]),
        ("minecraft/saves/World/region/r.0.0.mca", b"new region"),
        ("minecraft/saves/World/session.lock", b"lock"),
        ("minecraft/mods/b.jar", b"new mod"),
        ("minecraft/options.txt", b"old options"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_vec()))
    .collect();
    assert_eq!(now, expected);
}

#[test]
fn a_crash_at_any_rename_is_undone_at_the_next_start() {
    for crash_after in 0..12 {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("instance");
        let (mut staged, before) = swap_fixture(&root);
        staged.faults.crash_after = Some(crash_after);
        let journal = dir.path().join("journal.json");
        let swapped = staged.swap(&journal);
        if swapped.is_ok() {
            // Fewer renames than that: it finished.
            break;
        }
        assert!(matches!(swapped, Err(SwapError::Broken(_))));
        assert!(journal.exists(), "the journal stays after a crash");
        let phase = swap::recover_one(&root, &journal).unwrap();
        assert!(phase.is_some());
        assert!(!journal.exists());
        swap::remove_leftovers(&root);
        assert_eq!(
            files(&root),
            before,
            "after a crash after {crash_after} renames"
        );
        // Again: nothing to do.
        assert_eq!(swap::recover_one(&root, &journal).unwrap(), None);
    }
}

#[test]
fn a_failed_rename_rolls_back_and_a_broken_rollback_waits_for_the_next_start() {
    for fail_after in 0..8 {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("instance");
        let (mut staged, before) = swap_fixture(&root);
        staged.faults.fail_after = Some(fail_after);
        let journal = dir.path().join("journal.json");
        match staged.swap(&journal) {
            Ok(()) => break,
            Err(SwapError::RolledBack(_)) => {}
            Err(SwapError::Broken(e)) => panic!("not rolled back: {e}"),
        }
        assert!(!journal.exists());
        assert_eq!(files(&root), before, "after failing at rename {fail_after}");
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("instance");
    let (mut staged, before) = swap_fixture(&root);
    staged.faults.fail_after = Some(3);
    staged.faults.break_rollback = true;
    let journal = dir.path().join("journal.json");
    assert!(matches!(staged.swap(&journal), Err(SwapError::Broken(_))));
    assert!(journal.exists());
    swap::recover_one(&root, &journal).unwrap();
    swap::remove_leftovers(&root);
    assert_eq!(files(&root), before);
}

#[test]
fn an_interrupted_restore_is_recovered_when_the_launcher_starts() {
    let (s, _) = played();
    let root = s.store.dir(s.id());
    let before = files(&root);
    // A swap that crashed part way, journaled where a restore puts it.
    let suffix = swap::random_suffix();
    let staging = root.join(format!("{}{suffix}", swap::STAGING_PREFIX));
    std::fs::create_dir_all(staging.join("minecraft/saves/World")).unwrap();
    std::fs::write(staging.join("minecraft/saves/World/level.dat"), b"new").unwrap();
    let replaced: HashSet<String> = ["minecraft/saves/World/level.dat".to_string()].into();
    let mut staged = Staged::new(
        &root,
        suffix,
        &["minecraft/saves/World".to_string()],
        &replaced,
        &["minecraft/saves/World/level.dat".to_string()],
    )
    .unwrap();
    staged.faults.crash_after = Some(1);
    let journal = s
        .images
        .store()
        .instance_dir(s.id())
        .unwrap()
        .join(swap::JOURNAL_FILE);
    assert!(staged.swap(&journal).is_err());
    assert_ne!(files(&root), before);
    let recovery = s.images.recover_restores();
    assert_eq!(recovery.repaired, [s.id().to_string()]);
    assert!(recovery.broken.is_empty());
    assert_eq!(files(&root), before);
    // A staging folder a crash left before any journal is deleted too.
    std::fs::create_dir_all(root.join(format!(
        "{}{}",
        swap::STAGING_PREFIX,
        swap::random_suffix()
    )))
    .unwrap();
    s.images.recover_restores();
    assert_eq!(files(&root), before);
}

#[test]
fn a_restored_modpack_record_follows_the_snapshots_pack_version() {
    let (s, _) = played();
    let dir = s.store.dir(s.id());
    s.write("scripts/pack.zs", b"pack script v1");
    let record = ModpackRecord {
        project_id: "P".into(),
        version_id: "V2".into(),
        version_number: "2.0".into(),
        pack_sha512: Some("b".repeat(128)),
        files: [
            ("mods/a.jar", "old"),
            ("mods/gone.jar", "old"),
            ("scripts/pack.zs", "old"),
        ]
        .into_iter()
        .map(|(path, sha1)| {
            (
                path.to_string(),
                PackFile {
                    sha1: sha1.into(),
                    source: FileSource::Override,
                    project: None,
                },
            )
        })
        .collect(),
        ..ModpackRecord::default()
    };
    crate::storage::save(&ModpackRecord::path(&dir), &record).unwrap();
    // The snapshot is of the pack's version 1: an image whose base says so.
    let older = Base::ModrinthPack {
        project: "P".into(),
        version: "V1".into(),
        sha512: "a".repeat(128),
    };
    let restored: Vec<lodestone_image::Entry> = ["mods/a.jar", "scripts/pack.zs"]
        .iter()
        .map(|p| {
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(*p).unwrap(),
                1,
                "0".repeat(64),
                lodestone_image::Source::Object {},
            )
        })
        .collect();
    restore::follow_base_for_tests(&dir, Some(&older), &restored).unwrap();
    let after = ModpackRecord::load(&dir).unwrap();
    assert_eq!(after.version_id, "V1");
    assert_eq!(after.pack_sha512.as_deref(), Some("a".repeat(128).as_str()));
    assert_eq!(after.version_number, "");
    let paths: Vec<&str> = after.files.keys().map(String::as_str).collect();
    assert_eq!(paths, ["mods/a.jar", "scripts/pack.zs"]);
    let sha1 = crate::game::download::sha1_of(&s.game("scripts/pack.zs")).unwrap();
    assert_eq!(after.files["scripts/pack.zs"].sha1, sha1);
    // The same version changes nothing.
    restore::follow_base_for_tests(&dir, Some(&older), &[]).unwrap();
    assert_eq!(ModpackRecord::load(&dir).unwrap(), after);
}

#[test]
fn a_full_snapshot_restores_everything_without_a_link() {
    let s = Setup::new();
    s.write("mods/a.jar", b"mod v1");
    s.world("World", b"w1");
    s.images.record_now(s.id(), "Created").unwrap();
    let full = s
        .images
        .take_snapshot(
            s.id(),
            Take {
                full: true,
                ..Take::new(SnapshotKind::Manual, "All")
            },
        )
        .unwrap()
        .unwrap();
    s.write("mods/a.jar", b"mod v2");
    s.write("saves/World/region/r.0.0.mca", b"w2");
    s.restore(
        &full.id,
        RestoreScope::WorldsAndContent(vec!["World".into()]),
    )
    .unwrap();
    assert_eq!(s.read("mods/a.jar").unwrap(), b"mod v1");
    assert_eq!(s.read("saves/World/region/r.0.0.mca").unwrap(), b"w1");
}
