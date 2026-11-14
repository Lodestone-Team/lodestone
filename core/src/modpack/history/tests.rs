use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use super::*;
use crate::implementations::minecraft::FabricLoaderVersion;
use crate::modpack::tests::{file_entry, index, pack, Entry, TestPack};
use crate::modpack::update::{
    pack_files, place_files, plan_files, record_of, remove_work_dirs, stage_files,
};
use crate::modpack::{install_pack_contents, read_pack};
use crate::modrinth::fake::{sha1_hex, FakeModrinth};

const CDN: &str = "https://cdn.modrinth.com/data";

fn origin(version: &str) -> ModpackOrigin {
    ModpackOrigin {
        project_id: "pack".into(),
        version_id: format!("v{version}"),
        name: "Test Pack".into(),
        version_number: Some(version.into()),
        icon_url: None,
    }
}

fn fabric() -> Flavour {
    Flavour::Fabric {
        loader_version: Some(FabricLoaderVersion::new("0.15.11".into())),
        installer_version: None,
    }
}

fn read_file(root: &Path, path: &str) -> Option<Vec<u8>> {
    std::fs::read(root.join(path)).ok()
}

fn write(root: &Path, path: &str, content: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

/// Version 1 or 2 of a pack: mods from the CDN and configs in its overrides.
fn test_pack(version: u8, fake: &FakeModrinth) -> TestPack {
    let mut files = vec![];
    let mut add = |path: &str, content: Vec<u8>, project: &str| {
        let url = format!(
            "{CDN}/{project}/versions/v{version}/{}",
            path.rsplit('/').next().unwrap()
        );
        files.push(file_entry(path, &content, None, &url));
        fake.add_file(&url, content);
    };
    add("mods/keep.jar", b"keep".to_vec(), "k");
    if version == 1 {
        add("mods/a-1.jar", b"a1".to_vec(), "a");
        add("mods/dropped.jar", b"dropped".to_vec(), "d");
    } else {
        add("mods/a-2.jar", b"a2".to_vec(), "a");
        add("mods/new.jar", b"new".to_vec(), "n");
    }
    let config: &[u8] = if version == 1 { b"v1" } else { b"v2" };
    pack(
        index(
            files,
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &if version == 1 {
            vec![
                Entry::File("overrides/config/pack.toml", config),
                Entry::File("overrides/config/edited.toml", config),
            ]
        } else {
            // a config only version 2 has: going back leaves it, as dropped configs stay
            vec![
                Entry::File("overrides/config/pack.toml", config),
                Entry::File("overrides/config/edited.toml", config),
                Entry::File("overrides/config/only-v2.toml", b"only in v2"),
            ]
        },
    )
}

/// Installs version 1 the way a new server gets it, with its first history entry.
async fn install(root: &Path, fake: &FakeModrinth) -> History {
    let v1 = test_pack(1, fake);
    let parsed = read_pack(&v1.path, false).unwrap();
    let report = install_pack_contents(fake, &parsed, &v1.path, root, false, &|_, _| {})
        .await
        .unwrap();
    let mut history = History::default();
    history.remember_downloads(&report.files);
    let entry = entry_as_is(
        HistoryKind::Install,
        origin("1"),
        "1.20.1".into(),
        fabric(),
        &record_of(&report.files),
    );
    history.push(root, entry);
    history.save(root).await.unwrap();
    history
}

/// Goes from what the server has to `new` (a pack's or an entry's files) and records it.
async fn change_to(
    root: &Path,
    fake: &FakeModrinth,
    history: &mut History,
    new: &BTreeMap<String, PackFile>,
    pack_path: Option<&Path>,
    kind: HistoryKind,
    version: &str,
) -> u32 {
    let old = PackRecord::load(root).await.unwrap();
    let plan = plan_files(root, &old, new);
    stage_files(fake, pack_path, new, &plan, root, false, &|_| {})
        .await
        .unwrap();
    place_files(root, &plan).unwrap();
    plan.record.save(root).await.unwrap();
    let entry = entry_for(
        kind,
        origin(version),
        "1.20.1".into(),
        fabric(),
        &old,
        &plan,
    );
    let id = history.record_change(root, &plan, new, entry);
    history.save(root).await.unwrap();
    remove_work_dirs(root).await;
    id
}

#[tokio::test]
async fn updates_are_undone_from_a_small_store() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let fake = FakeModrinth::default();
    let mut history = install(root, &fake).await;
    assert_eq!(history.entries.len(), 1);
    // nothing is stored yet: every file is on the server
    assert_eq!(history.stored_bytes(), 0);

    // the user changes a config, then updates to version 2
    write(root, "config/edited.toml", b"mine");
    let v2 = test_pack(2, &fake);
    let new = pack_files(&read_pack(&v2.path, false).unwrap(), &v2.path, false).unwrap();
    let update = change_to(
        root,
        &fake,
        &mut history,
        &new,
        Some(&v2.path),
        HistoryKind::Update,
        "2",
    )
    .await;
    assert_eq!(
        read_file(root, "config/pack.toml").as_deref(),
        Some(&b"v2"[..])
    );

    // only the replaced config was stored: the mods it removed can be downloaded again
    let stored: Vec<&String> = history
        .files
        .iter()
        .filter(|(_, known)| known.stored)
        .map(|(sha1, _)| sha1)
        .collect();
    assert_eq!(stored, [&sha1_hex(b"v1")]);
    assert_eq!(history.stored_bytes(), 2);
    assert!(object_path(root, &sha1_hex(b"v1")).unwrap().is_file());

    // blame: the kept mod came with the install, the new ones with the update
    let blame = history.blame();
    assert_eq!(blame["mods/keep.jar"], 1);
    assert_eq!(blame["mods/new.jar"], update);
    assert_eq!(blame["config/pack.toml"], update);

    // the history reads back the same, and shows the update as changes
    let loaded = History::load(root).await;
    assert_eq!(loaded, history);
    let info = loaded.info(root, None);
    assert_eq!(info.entries[0].id, update);
    assert!(info.entries[0].current);
    let kinds: Vec<(ChangeKind, &str, Option<&str>)> = info.entries[0]
        .changes
        .iter()
        .map(|c| (c.kind, c.path.as_str(), c.old_path.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ChangeKind::Added, "config/only-v2.toml", None),
            (ChangeKind::Added, "mods/new.jar", None),
            (ChangeKind::Updated, "mods/a-2.jar", Some("mods/a-1.jar")),
            (ChangeKind::Replaced, "config/pack.toml", None),
            (ChangeKind::Removed, "mods/dropped.jar", None),
        ]
    );
    assert_eq!(info.entries[0].kept, ["config/edited.toml"]);
    assert_eq!(info.entries[1].missing, 0, "version 1 can be restored");
    // nor does version 2 lack anything: the config the user edited is never stored,
    // but a restore keeps the user's edit anyway
    assert_eq!(info.entries[0].missing, 0);

    // undo: back to version 1, from the store and the downloads
    let back = history.files_of(root, 1, false).unwrap();
    assert!(back["config/pack.toml"].stored.is_some());
    assert!(back["mods/a-1.jar"].download.is_some());
    let restore = change_to(
        root,
        &fake,
        &mut history,
        &back,
        None,
        HistoryKind::Restore { to: 1 },
        "1",
    )
    .await;
    assert_eq!(
        read_file(root, "config/pack.toml").as_deref(),
        Some(&b"v1"[..])
    );
    assert_eq!(read_file(root, "mods/a-1.jar").as_deref(), Some(&b"a1"[..]));
    assert_eq!(
        read_file(root, "mods/dropped.jar").as_deref(),
        Some(&b"dropped"[..])
    );
    assert_eq!(read_file(root, "mods/new.jar"), None);
    // the user's change stays through both
    assert_eq!(
        read_file(root, "config/edited.toml").as_deref(),
        Some(&b"mine"[..])
    );
    // the files are the install's again, so blame goes back to it
    assert_eq!(history.blame()["mods/a-1.jar"], restore);
    assert_eq!(history.blame()["mods/keep.jar"], 1);
    // and version 2 can be restored too: its replaced config was stored on the way
    // back, and the config only it has is still on the server
    assert_eq!(
        read_file(root, "config/only-v2.toml").as_deref(),
        Some(&b"only in v2"[..])
    );
    assert_eq!(history.info(root, None).entries[1].missing, 0);
}

#[tokio::test]
async fn a_version_with_lost_files_cannot_be_restored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let fake = FakeModrinth::default();
    let mut history = install(root, &fake).await;
    let v2 = test_pack(2, &fake);
    let new = pack_files(&read_pack(&v2.path, false).unwrap(), &v2.path, false).unwrap();
    change_to(
        root,
        &fake,
        &mut history,
        &new,
        Some(&v2.path),
        HistoryKind::Update,
        "2",
    )
    .await;
    // both version 1 configs have the same bytes, so one stored copy served both
    assert_eq!(
        history.files.values().filter(|known| known.stored).count(),
        1
    );
    std::fs::remove_file(object_path(root, &sha1_hex(b"v1")).unwrap()).unwrap();
    let info = history.info(root, None);
    assert_eq!(info.entries[1].missing, 2);
    let back = history.files_of(root, 1, false).unwrap();
    let config = &back["config/pack.toml"];
    assert!(config.stored.is_none() && config.download.is_none());
}

#[test]
fn edited_histories_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let file = |sha1: &str| RecordedFile {
        sha1: sha1.into(),
        from_overrides: false,
        project_id: None,
    };
    let jar = sha1_hex(b"jar");
    let mut history = History::default();
    history.entries.push(HistoryEntry {
        id: 1,
        time: 0,
        kind: HistoryKind::Install,
        origin: origin("1"),
        game_version: "1.20.1".into(),
        flavour: fabric(),
        files: BTreeMap::from([
            ("mods/a.jar".into(), file(&jar)),
            ("../outside.jar".into(), file(&jar)),
            ("eula.txt".into(), file(&jar)),
            (".lodestone_config".into(), file(&jar)),
            ("kubejs/x.js".into(), file(&jar)),
        ]),
        changes: vec![],
        kept: vec![],
        changelog: None,
    });
    // a download from a host packs may not use is ignored
    history.files.insert(
        jar.clone(),
        FileRef {
            size: 3,
            sha512: Some("a".repeat(128)),
            urls: vec!["https://example.com/a.jar".into()],
            stored: false,
        },
    );
    let files = history.files_of(root, 1, false).unwrap();
    assert_eq!(files.keys().collect::<Vec<_>>(), ["mods/a.jar"]);
    assert!(files["mods/a.jar"].download.is_none());
    // an object name that isn't a SHA-1 has no path
    assert!(object_path(root, "../../x").is_none());
}

#[test]
fn old_entries_and_their_stored_files_go() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut history = History::default();
    for n in 0..(MAX_ENTRIES + 3) {
        let content = format!("config {n}");
        let sha1 = sha1_hex(content.as_bytes());
        let object = object_path(root, &sha1).unwrap();
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, &content).unwrap();
        history.files.insert(
            sha1.clone(),
            FileRef {
                size: content.len() as u64,
                stored: true,
                ..Default::default()
            },
        );
        let record = PackRecord {
            files: BTreeMap::from([(
                "config/a.toml".into(),
                RecordedFile {
                    sha1,
                    from_overrides: true,
                    project_id: None,
                },
            )]),
        };
        history.push(
            root,
            entry_as_is(
                HistoryKind::Update,
                origin("1"),
                "1.20.1".into(),
                fabric(),
                &record,
            ),
        );
    }
    assert_eq!(history.entries.len(), MAX_ENTRIES);
    assert_eq!(history.entries[0].id, 4);
    assert_eq!(history.files.len(), MAX_ENTRIES);
    // the first three entries' stored configs were deleted
    let first = object_path(root, &sha1_hex(b"config 0")).unwrap();
    assert!(!first.exists());
    let kept = object_path(root, &sha1_hex(b"config 3")).unwrap();
    assert!(kept.is_file());
}

/// Restores entry `id` as the update handler does: the entry's files, and the user's own
/// files that later versions replaced, which stay out of the record.
async fn restore(root: &Path, fake: &FakeModrinth, history: &mut History, id: u32) -> u32 {
    let mut files = history.files_of(root, id, false).unwrap();
    let theirs = history.user_files_after(root, id, false);
    for (path, file) in &theirs {
        files.entry(path.clone()).or_insert_with(|| file.clone());
    }
    let old = PackRecord::load(root).await.unwrap();
    let mut plan = plan_files(root, &old, &files);
    for path in theirs.keys() {
        plan.record.files.remove(path);
    }
    stage_files(fake, None, &files, &plan, root, false, &|_| {})
        .await
        .unwrap();
    place_files(root, &plan).unwrap();
    plan.record.save(root).await.unwrap();
    let entry = entry_for(
        HistoryKind::Restore { to: id },
        origin("1"),
        "1.20.1".into(),
        fabric(),
        &old,
        &plan,
    );
    let restored = history.record_change(root, &plan, &files, entry);
    history.save(root).await.unwrap();
    remove_work_dirs(root).await;
    restored
}

/// LS-98: an update that ships a mod at the path of one of the user's own replaces it,
/// and going back to the version before puts the user's mod back.
#[tokio::test]
async fn restoring_puts_back_the_users_file_an_update_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let fake = FakeModrinth::default();
    let mut history = install(root, &fake).await;
    write(root, "mods/sodium.jar", b"mine");

    let url = format!("{CDN}/s/versions/v2/sodium.jar");
    fake.add_file(&url, b"the pack's".to_vec());
    let keep_url = format!("{CDN}/k/versions/v1/keep.jar");
    let v2 = pack(
        index(
            vec![
                file_entry("mods/keep.jar", b"keep", None, &keep_url),
                file_entry("mods/sodium.jar", b"the pack's", None, &url),
            ],
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &[],
    );
    let new = pack_files(&read_pack(&v2.path, false).unwrap(), &v2.path, false).unwrap();
    let update = change_to(
        root,
        &fake,
        &mut history,
        &new,
        Some(&v2.path),
        HistoryKind::Update,
        "2",
    )
    .await;
    assert_eq!(
        read_file(root, "mods/sodium.jar").as_deref(),
        Some(&b"the pack's"[..])
    );
    // the user's mod can't be downloaded, so it was stored
    assert!(object_path(root, &sha1_hex(b"mine")).unwrap().is_file());

    // back to version 1: the user's mod comes back, and stays theirs
    let back = restore(root, &fake, &mut history, 1).await;
    assert_eq!(
        read_file(root, "mods/sodium.jar").as_deref(),
        Some(&b"mine"[..])
    );
    assert_eq!(read_file(root, "mods/a-1.jar").as_deref(), Some(&b"a1"[..]));
    let record = PackRecord::load(root).await.unwrap();
    assert!(!record.files.contains_key("mods/sodium.jar"));
    assert!(!history
        .entry(back)
        .unwrap()
        .files
        .contains_key("mods/sodium.jar"));

    // forward to version 2 again: the pack's mod replaces the user's once more
    restore(root, &fake, &mut history, update).await;
    assert_eq!(
        read_file(root, "mods/sodium.jar").as_deref(),
        Some(&b"the pack's"[..])
    );
    // and back: still the user's
    restore(root, &fake, &mut history, 1).await;
    assert_eq!(
        read_file(root, "mods/sodium.jar").as_deref(),
        Some(&b"mine"[..])
    );
}

/// LS-98: a file the user put at that path since stays: only the pack's file, or
/// nothing, is replaced by the user's old one.
#[tokio::test]
async fn restoring_leaves_a_users_newer_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let fake = FakeModrinth::default();
    let mut history = install(root, &fake).await;
    write(root, "mods/sodium.jar", b"mine");
    let url = format!("{CDN}/s/versions/v2/sodium.jar");
    fake.add_file(&url, b"the pack's".to_vec());
    let v2 = pack(
        index(
            vec![file_entry("mods/sodium.jar", b"the pack's", None, &url)],
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &[],
    );
    let new = pack_files(&read_pack(&v2.path, false).unwrap(), &v2.path, false).unwrap();
    change_to(
        root,
        &fake,
        &mut history,
        &new,
        Some(&v2.path),
        HistoryKind::Update,
        "2",
    )
    .await;
    // the user replaces the pack's mod with another of their own
    write(root, "mods/sodium.jar", b"newer");
    restore(root, &fake, &mut history, 1).await;
    assert_eq!(
        read_file(root, "mods/sodium.jar").as_deref(),
        Some(&b"newer"[..])
    );
}

/// LS-99: a baseline without files (a server that had no record of them) can't be
/// restored: restoring it would remove every file of the pack.
#[test]
fn a_baseline_without_files_cannot_be_restored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut history = History::default();
    history.push(
        root,
        entry_as_is(
            HistoryKind::Baseline,
            origin("1"),
            "1.20.1".into(),
            fabric(),
            &PackRecord::default(),
        ),
    );
    let record = PackRecord {
        files: BTreeMap::from([(
            "mods/a.jar".into(),
            RecordedFile {
                sha1: sha1_hex(b"a"),
                from_overrides: false,
                project_id: None,
            },
        )]),
    };
    history.push(
        root,
        entry_as_is(
            HistoryKind::Update,
            origin("2"),
            "1.20.1".into(),
            fabric(),
            &record,
        ),
    );
    assert!(history.entry(1).unwrap().files_unknown());
    assert!(!history.entry(2).unwrap().files_unknown());
    let info = history.info(root, None);
    assert_eq!(info.entries[1].id, 1);
    assert_eq!(info.entries[1].missing, 1);

    // and a server without a record gets no baseline at all
    let mut history = History::default();
    history.begin_with_baseline(&origin("1"), "1.20.1", &fabric(), &PackRecord::default());
    assert!(history.entries.is_empty());
    history.begin_with_baseline(&origin("1"), "1.20.1", &fabric(), &record);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].id, 1);
    assert_eq!(history.entries[0].kind, HistoryKind::Baseline);
    // only an empty history gets one
    history.begin_with_baseline(&origin("1"), "1.20.1", &fabric(), &record);
    assert_eq!(history.entries.len(), 1);
}
