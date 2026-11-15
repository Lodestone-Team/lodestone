use std::collections::BTreeMap;
use std::path::Path;

use crate::modpack::tests::{file_entry, index, pack, Entry};
use crate::modpack::{install_pack_contents, read_pack};
use crate::modrinth::fake::{sha1_hex, FakeModrinth};

use super::*;

const CDN: &str = "https://cdn.modrinth.com/data";

fn write(root: &Path, path: &str, content: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

fn read_file(root: &Path, path: &str) -> Option<Vec<u8>> {
    std::fs::read(root.join(path)).ok()
}

fn recorded(content: &[u8], from_overrides: bool, project: Option<&str>) -> RecordedFile {
    RecordedFile {
        sha1: sha1_hex(content),
        from_overrides,
        project_id: project.map(String::from),
    }
}

fn download(path: &str, content: &[u8], project: &str) -> PackFile {
    let url = format!(
        "{CDN}/{project}/versions/v/{}",
        path.rsplit('/').next().unwrap()
    );
    PackFile {
        sha1: sha1_hex(content),
        download: Some(PackDownload {
            path: path.into(),
            urls: vec![url::Url::parse(&url).unwrap()],
            sha1: sha1_hex(content),
            sha512: String::new(),
            size: content.len() as u64,
        }),
        from_overrides: false,
        project_id: Some(project.to_string()),
    }
}

fn override_file(content: &[u8]) -> PackFile {
    PackFile {
        sha1: sha1_hex(content),
        download: None,
        from_overrides: true,
        project_id: None,
    }
}

fn change(plan: &FilePlan, target: &str) -> Option<FileAction> {
    plan.changes
        .iter()
        .find(|c| c.target == target)
        .map(|c| c.action)
}

#[test]
fn plans_downloads() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "mods/same.jar", b"same");
    write(root, "mods/old.jar", b"old");
    write(root, "mods/gone.jar", b"gone");
    write(root, "mods/changed-gone.jar", b"user changed it");
    write(root, "mods/changed.jar", b"user changed it");
    write(root, "mods/by-hand.jar", b"the user's");
    write(root, "mods/mine.jar", b"the user's own mod");
    let old = PackRecord {
        files: BTreeMap::from([
            ("mods/same.jar".into(), recorded(b"same", false, Some("s"))),
            ("mods/old.jar".into(), recorded(b"old", false, Some("o"))),
            ("mods/gone.jar".into(), recorded(b"gone", false, Some("g"))),
            (
                "mods/changed-gone.jar".into(),
                recorded(b"pack", false, Some("cg")),
            ),
            (
                "mods/changed.jar".into(),
                recorded(b"pack", false, Some("c")),
            ),
            ("mods/missing.jar".into(), recorded(b"m", false, Some("m"))),
        ]),
    };
    let new = BTreeMap::from([
        (
            "mods/same.jar".into(),
            download("mods/same.jar", b"same", "s"),
        ),
        ("mods/old.jar".into(), download("mods/old.jar", b"new", "o")),
        (
            "mods/changed.jar".into(),
            download("mods/changed.jar", b"new", "c"),
        ),
        (
            "mods/by-hand.jar".into(),
            download("mods/by-hand.jar", b"pack", "h"),
        ),
        (
            "mods/added.jar".into(),
            download("mods/added.jar", b"added", "a"),
        ),
    ]);
    let plan = plan_files(root, &old, &new);
    assert_eq!(change(&plan, "mods/same.jar"), None);
    assert_eq!(change(&plan, "mods/old.jar"), Some(FileAction::Replace));
    assert_eq!(change(&plan, "mods/gone.jar"), Some(FileAction::Remove));
    assert_eq!(change(&plan, "mods/added.jar"), Some(FileAction::Add));
    // a mod at the pack's path that the old pack didn't write is replaced
    assert_eq!(change(&plan, "mods/by-hand.jar"), Some(FileAction::Replace));
    // the user's changes and own mods stay
    assert_eq!(change(&plan, "mods/changed.jar"), None);
    assert_eq!(change(&plan, "mods/changed-gone.jar"), None);
    assert_eq!(change(&plan, "mods/mine.jar"), None);
    assert_eq!(change(&plan, "mods/missing.jar"), None);
    assert_eq!(plan.kept, ["mods/changed-gone.jar", "mods/changed.jar"]);
    // the new record lists the new pack's files
    assert_eq!(
        plan.record.files.keys().collect::<Vec<_>>(),
        new.keys().collect::<Vec<_>>()
    );
}

#[test]
fn plans_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "config/a.toml", b"pack a");
    write(root, "config/edited.toml", b"user edit");
    write(root, "config/own.toml", b"user's own");
    write(root, "config/dropped.toml", b"dropped");
    write(root, "mods/bundled.jar", b"bundled");
    let old = PackRecord {
        files: BTreeMap::from([
            ("config/a.toml".into(), recorded(b"pack a", true, None)),
            ("config/edited.toml".into(), recorded(b"pack", true, None)),
            (
                "config/dropped.toml".into(),
                recorded(b"dropped", true, None),
            ),
            ("mods/bundled.jar".into(), recorded(b"bundled", true, None)),
        ]),
    };
    let new = BTreeMap::from([
        ("config/a.toml".into(), override_file(b"pack a 2")),
        ("config/edited.toml".into(), override_file(b"pack 2")),
        ("config/own.toml".into(), override_file(b"pack's")),
        ("config/new.toml".into(), override_file(b"new")),
    ]);
    let plan = plan_files(root, &old, &new);
    assert_eq!(change(&plan, "config/a.toml"), Some(FileAction::Replace));
    assert_eq!(change(&plan, "config/new.toml"), Some(FileAction::Add));
    // the user's configs stay
    assert_eq!(change(&plan, "config/edited.toml"), None);
    assert_eq!(change(&plan, "config/own.toml"), None);
    // a dropped config stays; a dropped bundled mod goes
    assert_eq!(change(&plan, "config/dropped.toml"), None);
    assert_eq!(change(&plan, "mods/bundled.jar"), Some(FileAction::Remove));
    assert_eq!(plan.kept, ["config/edited.toml", "config/own.toml"]);
}

#[test]
fn disabled_mods_stay_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "mods/a.jar.disabled", b"a1");
    write(root, "mods/b-1.0.jar.disabled", b"b1");
    let old = PackRecord {
        files: BTreeMap::from([
            ("mods/a.jar".into(), recorded(b"a1", false, Some("a"))),
            ("mods/b-1.0.jar".into(), recorded(b"b1", false, Some("b"))),
        ]),
    };
    let new = BTreeMap::from([
        ("mods/a.jar".into(), download("mods/a.jar", b"a2", "a")),
        // a new file name for the same project
        (
            "mods/b-2.0.jar".into(),
            download("mods/b-2.0.jar", b"b2", "b"),
        ),
    ]);
    let plan = plan_files(root, &old, &new);
    assert_eq!(
        change(&plan, "mods/a.jar.disabled"),
        Some(FileAction::Replace)
    );
    assert_eq!(
        change(&plan, "mods/b-1.0.jar.disabled"),
        Some(FileAction::Remove)
    );
    assert_eq!(
        change(&plan, "mods/b-2.0.jar.disabled"),
        Some(FileAction::Add)
    );
}

#[test]
fn record_paths_stay_in_the_server() {
    for bad in [
        "../outside.jar",
        "mods/../../outside.jar",
        "/etc/passwd",
        ".lodestone_config",
        "x/.lodestone_minecraft_config.json",
        "",
        // files no pack may write (LS-02)
        "server.jar",
        "Server.JAR",
        "server.jar.",
        "server.jar::$DATA",
        "eula.txt",
        "user_jvm_args.txt",
        "USER_J~1.TXT",
        ".Lodestone_Modpack/notes.json",
    ] {
        assert!(record_path(bad).is_none(), "{bad}");
    }
    assert!(record_path("mods/a.jar").is_some());
    assert!(record_path("mods/server.jar").is_some());

    // a record edited to list server.jar doesn't remove it
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "server.jar", b"vanilla");
    let old = PackRecord {
        files: BTreeMap::from([("server.jar".into(), recorded(b"vanilla", false, None))]),
    };
    let plan = plan_files(dir.path(), &old, &BTreeMap::new());
    assert!(plan.changes.is_empty());

    // a record edited to point outside the server removes nothing there
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    std::fs::create_dir(&root).unwrap();
    write(dir.path(), "outside.jar", b"x");
    let old = PackRecord {
        files: BTreeMap::from([("../outside.jar".into(), recorded(b"x", false, None))]),
    };
    let plan = plan_files(&root, &old, &BTreeMap::new());
    assert!(plan.changes.is_empty());
}

/// A mod updated in the Mods tab (saved under the new version's file name) is replaced by
/// the pack's next version of it, not kept next to it (LS-47).
#[tokio::test]
async fn another_copy_of_an_added_mod_is_replaced() {
    use crate::modrinth::fake::FakeVersion;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let api = FakeModrinth::default();
    let updated = FakeVersion::new("sodium", "s2", "sodium-2.jar");
    write(root, "mods/sodium-2.jar", &updated.content);
    api.add(updated);
    let other = FakeVersion::new("lithium", "l1", "lithium.jar");
    write(root, "mods/lithium.jar", &other.content);
    api.add(other);
    write(root, "mods/not-on-modrinth.jar", b"the user's");
    // the pack's first version wrote sodium-1.jar, which the Mods tab removed
    let old = PackRecord {
        files: BTreeMap::from([(
            "mods/sodium-1.jar".into(),
            recorded(b"sodium 1", false, Some("sodium")),
        )]),
    };
    let new = std::sync::Arc::new(BTreeMap::from([(
        "mods/sodium-3.jar".to_string(),
        download("mods/sodium-3.jar", b"sodium 3", "sodium"),
    )]));
    let mut plan = plan_files(root, &old, &new);
    let others = other_copies(&api, root, &old, &new, &plan).await;
    assert_eq!(
        others,
        [OtherCopy {
            target: "mods/sodium-2.jar".into(),
            project_id: "sodium".into(),
            sha512: crate::modrinth::fake::sha512_hex(
                &std::fs::read(root.join("mods/sodium-2.jar")).unwrap()
            ),
        }]
    );
    remove_other_copies(&mut plan, &new, &others);
    assert_eq!(change(&plan, "mods/sodium-3.jar"), Some(FileAction::Add));
    assert_eq!(change(&plan, "mods/sodium-2.jar"), Some(FileAction::Remove));
    // the copy it removes is checked by its SHA-512 before placement
    check_as_planned(root, &plan).unwrap();
    write(root, "mods/sodium-2.jar", b"changed since");
    assert!(check_as_planned(root, &plan).is_err());
    write(
        root,
        "mods/sodium-2.jar",
        &FakeVersion::new("sodium", "s2", "sodium-2.jar").content,
    );
    check_as_planned(root, &plan).unwrap();
    // other projects and the user's own mods stay
    assert_eq!(change(&plan, "mods/lithium.jar"), None);
    assert_eq!(change(&plan, "mods/not-on-modrinth.jar"), None);

    // a disabled copy keeps the new version disabled
    std::fs::rename(
        root.join("mods/sodium-2.jar"),
        root.join("mods/sodium-2.jar.disabled"),
    )
    .unwrap();
    let mut plan = plan_files(root, &old, &new);
    let others = other_copies(&api, root, &old, &new, &plan).await;
    remove_other_copies(&mut plan, &new, &others);
    assert_eq!(
        change(&plan, "mods/sodium-3.jar.disabled"),
        Some(FileAction::Add)
    );
    assert_eq!(
        change(&plan, "mods/sodium-2.jar.disabled"),
        Some(FileAction::Remove)
    );
}

/// Scripts an update leaves out (no permission to add them) stay on the server, so they
/// stay in the record as the pack's files (LS-47).
#[test]
fn scripts_left_out_stay_in_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "kubejs/server_scripts/x.js", b"v1");
    let old = PackRecord {
        files: BTreeMap::from([
            (
                "kubejs/server_scripts/x.js".into(),
                recorded(b"v1", true, None),
            ),
            ("config/a.toml".into(), recorded(b"a", true, None)),
        ]),
    };
    // the new version, without its scripts
    let new = BTreeMap::from([("config/b.toml".into(), override_file(b"b"))]);
    let mut plan = plan_files(root, &old, &new);
    keep_unwritten_scripts(&mut plan, &old, &new);
    assert_eq!(
        plan.record.files.get("kubejs/server_scripts/x.js"),
        old.files.get("kubejs/server_scripts/x.js")
    );
    // a dropped config isn't the pack's any more
    assert!(!plan.record.files.contains_key("config/a.toml"));

    // so the owner's next update, with the scripts, replaces the unchanged script
    let next = BTreeMap::from([("kubejs/server_scripts/x.js".into(), override_file(b"v3"))]);
    let plan = plan_files(root, &plan.record, &next);
    assert_eq!(
        change(&plan, "kubejs/server_scripts/x.js"),
        Some(FileAction::Replace)
    );
    assert!(plan.kept.is_empty());
}

/// A pack path whose case changes names the same file on a disk that ignores case: it is
/// kept or replaced, not removed (LS-47). On a disk that minds case, the old file goes.
#[test]
fn a_path_that_only_changes_case() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "mods/Foo.jar", b"foo");
    let ignores_case = root.join("mods/foo.jar").exists();
    let old = PackRecord {
        files: BTreeMap::from([("mods/Foo.jar".into(), recorded(b"foo", false, Some("f")))]),
    };

    // the same content
    let same = BTreeMap::from([("mods/foo.jar".into(), download("mods/foo.jar", b"foo", "f"))]);
    let plan = plan_files(root, &old, &same);
    if ignores_case {
        assert!(plan.changes.is_empty(), "{:?}", plan.changes);
    } else {
        assert_eq!(change(&plan, "mods/Foo.jar"), Some(FileAction::Remove));
        assert_eq!(change(&plan, "mods/foo.jar"), Some(FileAction::Add));
    }

    // new content: replaced, and placing it works
    let changed = BTreeMap::from([(
        "mods/foo.jar".into(),
        download("mods/foo.jar", b"foo 2", "f"),
    )]);
    let plan = plan_files(root, &old, &changed);
    if ignores_case {
        assert_eq!(plan.changes.len(), 1, "{:?}", plan.changes);
        assert_eq!(change(&plan, "mods/foo.jar"), Some(FileAction::Replace));
    }
    write(root, &format!("{STAGING_DIR}/mods/foo.jar"), b"foo 2");
    place_files(root, &plan).unwrap();
    assert_eq!(
        read_file(root, "mods/foo.jar").as_deref(),
        Some(&b"foo 2"[..])
    );
    if !ignores_case {
        assert_eq!(read_file(root, "mods/Foo.jar"), None);
    }
}

/// The server's files and plan for an interrupted-update test: the old pack wrote
/// `mods/old.jar` (replaced) and `mods/gone.jar` (removed), and the user added
/// `mods/mine.jar` at a path the new pack uses (replaced, and the only copy of it).
fn interrupted_setup(root: &Path) -> (PackRecord, FilePlan) {
    write(root, "mods/old.jar", b"old 1");
    write(root, "mods/gone.jar", b"gone");
    write(root, "mods/mine.jar", b"the user's own");
    let old = PackRecord {
        files: BTreeMap::from([
            ("mods/old.jar".into(), recorded(b"old 1", false, Some("o"))),
            ("mods/gone.jar".into(), recorded(b"gone", false, Some("g"))),
        ]),
    };
    let new = BTreeMap::from([
        (
            "mods/old.jar".into(),
            download("mods/old.jar", b"old 2", "o"),
        ),
        (
            "mods/mine.jar".into(),
            download("mods/mine.jar", b"pack's", "m"),
        ),
        (
            "mods/added.jar".into(),
            download("mods/added.jar", b"added", "a"),
        ),
    ]);
    let plan = plan_files(root, &old, &new);
    assert_eq!(plan.changes.len(), 4, "{:?}", plan.changes);
    for (path, content) in [
        ("mods/old.jar", &b"old 2"[..]),
        ("mods/mine.jar", b"pack's"),
        ("mods/added.jar", b"added"),
    ] {
        write(root, &format!("{STAGING_DIR}/{path}"), content);
    }
    (old, plan)
}

fn assert_as_before(root: &Path) {
    assert_eq!(
        read_file(root, "mods/old.jar").as_deref(),
        Some(&b"old 1"[..])
    );
    assert_eq!(
        read_file(root, "mods/gone.jar").as_deref(),
        Some(&b"gone"[..])
    );
    assert_eq!(
        read_file(root, "mods/mine.jar").as_deref(),
        Some(&b"the user's own"[..])
    );
    assert_eq!(read_file(root, "mods/added.jar"), None);
}

/// An update the core stopped in the middle of (during the server software's reinstall,
/// after its files were placed but before it committed) is undone on the next start, and
/// the next update doesn't delete the snapshot with the only copies of the old files
/// meanwhile (LS-19).
#[tokio::test]
async fn an_interrupted_update_is_undone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (old, plan) = interrupted_setup(root);
    begin_update(root, &plan).unwrap();
    place_files(root, &plan).unwrap();
    // the core stops here; the update task's cleanup, or the next update's staging,
    // keeps the snapshot
    remove_work_dirs(root).await;
    assert_eq!(
        read_file(root, &format!("{SNAPSHOT_DIR}/mods/mine.jar")).as_deref(),
        Some(&b"the user's own"[..])
    );

    assert!(recover_interrupted(root));
    assert_as_before(root);
    assert!(!root.join(JOURNAL_FILE).exists());
    assert!(!root.join(SNAPSHOT_DIR).exists());
    assert!(!root.join(STAGING_DIR).exists());
    // and only once
    assert!(!recover_interrupted(root));
    assert_as_before(root);

    // so the next plan sees the old pack's files as they were, not as user changes
    let again = plan_files(
        root,
        &old,
        &BTreeMap::from([(
            "mods/old.jar".to_string(),
            download("mods/old.jar", b"old 2", "o"),
        )]),
    );
    assert!(again.kept.is_empty(), "{:?}", again.kept);
}

/// The core can stop at any point while the files are moved: a file is removed only when
/// it is the one the update placed, and put back only from the snapshot (LS-19).
#[tokio::test]
async fn an_update_stopped_while_moving_files_is_undone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (_, plan) = interrupted_setup(root);
    begin_update(root, &plan).unwrap();
    // only the first file was moved into the snapshot
    let first = plan
        .changes
        .iter()
        .find(|c| c.action != FileAction::Add)
        .unwrap();
    write(
        root,
        &format!("{SNAPSHOT_DIR}/{}", first.target),
        &read_file(root, &first.target).unwrap(),
    );
    std::fs::remove_file(root.join(&first.target)).unwrap();

    assert!(recover_interrupted(root));
    assert_as_before(root);
    assert!(!root.join(SNAPSHOT_DIR).exists());
}

/// An update stopped after it committed keeps its files, and what is left in its
/// snapshot is kept rather than deleted (LS-19).
#[tokio::test]
async fn an_update_stopped_after_it_committed_keeps_its_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (_, plan) = interrupted_setup(root);
    begin_update(root, &plan).unwrap();
    place_files(root, &plan).unwrap();
    commit_update(root).unwrap();

    assert!(recover_interrupted(root));
    assert_eq!(
        read_file(root, "mods/old.jar").as_deref(),
        Some(&b"old 2"[..])
    );
    assert_eq!(
        read_file(root, "mods/mine.jar").as_deref(),
        Some(&b"pack's"[..])
    );
    assert_eq!(read_file(root, "mods/gone.jar"), None);
    assert!(!root.join(JOURNAL_FILE).exists());
    assert!(!root.join(SNAPSHOT_DIR).exists());
    let kept: Vec<_> = std::fs::read_dir(root.join(MODPACK_DIR))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("interrupted-"))
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(
        std::fs::read(kept[0].path().join("mods/mine.jar")).unwrap(),
        b"the user's own"
    );
}

/// A finished update leaves no journal, and its snapshot goes as before.
#[tokio::test]
async fn a_finished_update_leaves_nothing_to_undo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let (_, plan) = interrupted_setup(root);
    begin_update(root, &plan).unwrap();
    place_files(root, &plan).unwrap();
    commit_update(root).unwrap();
    end_update(root).unwrap();
    remove_work_dirs(root).await;
    assert!(!root.join(SNAPSHOT_DIR).exists());
    assert!(!recover_interrupted(root));
    assert_eq!(
        read_file(root, "mods/old.jar").as_deref(),
        Some(&b"old 2"[..])
    );
}

#[test]
fn project_ids_come_from_cdn_urls() {
    let file = download("mods/a.jar", b"a", "AANobbMI");
    assert_eq!(
        project_of(file.download.as_ref().unwrap()).as_deref(),
        Some("AANobbMI")
    );
    let mut other = file.download.unwrap();
    other.urls = vec![url::Url::parse("https://github.com/data/x/versions/y/a.jar").unwrap()];
    assert_eq!(project_of(&other), None);
}

/// A pack with mods and configs, as version 1 or 2.
fn test_pack(version: u8) -> (crate::modpack::tests::TestPack, FakeModrinth) {
    let fake = FakeModrinth::default();
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
    let p = pack(
        index(
            files,
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &[
            Entry::File("overrides/config/pack.toml", config),
            Entry::File("overrides/config/edited.toml", config),
        ],
    );
    (p, fake)
}

use serde_json::json;

async fn files_of(path: &Path) -> BTreeMap<String, PackFile> {
    let pack = read_pack(path, false).unwrap();
    pack_files(&pack, path, false).unwrap()
}

#[tokio::test]
async fn updates_a_server_and_undoes_a_failed_update() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // version 1 as a new server: its files are what pack.json keeps
    let (v1, fake1) = test_pack(1);
    let parsed = read_pack(&v1.path, false).unwrap();
    let report = install_pack_contents(&fake1, &parsed, &v1.path, root, false, &|_, _| {})
        .await
        .unwrap();
    let record = record_of(&report.files);
    assert!(record.files.contains_key("mods/a-1.jar"));
    assert!(record.files["config/pack.toml"].from_overrides);

    // the user disables a mod, edits a config and adds a mod of their own
    std::fs::rename(
        root.join("mods/a-1.jar"),
        root.join("mods/a-1.jar.disabled"),
    )
    .unwrap();
    write(root, "config/edited.toml", b"mine");
    write(root, "mods/own.jar", b"own");

    let (v2, fake2) = test_pack(2);
    let new = files_of(&v2.path).await;
    let plan = plan_files(root, &record, &new);
    assert_eq!(plan.kept, ["config/edited.toml"]);

    // a failed placement (a staged file is missing) changes nothing
    let sources = sources_of(&new);
    stage_files(
        &fake2,
        Some(&v2.path),
        &sources,
        &plan,
        root,
        false,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    std::fs::remove_file(root.join(STAGING_DIR).join("mods/new.jar")).unwrap();
    assert!(place_files(root, &plan).is_err());
    assert_eq!(
        read_file(root, "mods/dropped.jar").as_deref(),
        Some(&b"dropped"[..])
    );
    assert_eq!(
        read_file(root, "config/pack.toml").as_deref(),
        Some(&b"v1"[..])
    );
    assert_eq!(read_file(root, "mods/a-2.jar.disabled"), None);
    assert!(root.join("mods/a-1.jar.disabled").is_file());
    remove_work_dirs(root).await;

    // the update
    stage_files(
        &fake2,
        Some(&v2.path),
        &sources,
        &plan,
        root,
        false,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    place_files(root, &plan).unwrap();
    remove_work_dirs(root).await;

    assert_eq!(
        read_file(root, "mods/keep.jar").as_deref(),
        Some(&b"keep"[..])
    );
    assert_eq!(
        read_file(root, "mods/new.jar").as_deref(),
        Some(&b"new"[..])
    );
    assert_eq!(read_file(root, "mods/dropped.jar"), None);
    assert_eq!(read_file(root, "mods/a-1.jar.disabled"), None);
    assert_eq!(
        read_file(root, "mods/a-2.jar.disabled").as_deref(),
        Some(&b"a2"[..])
    );
    assert_eq!(
        read_file(root, "config/pack.toml").as_deref(),
        Some(&b"v2"[..])
    );
    assert_eq!(
        read_file(root, "config/edited.toml").as_deref(),
        Some(&b"mine"[..])
    );
    assert_eq!(
        read_file(root, "mods/own.jar").as_deref(),
        Some(&b"own"[..])
    );
    assert!(!root.join(STAGING_DIR).exists());
    assert!(!root.join(SNAPSHOT_DIR).exists());

    // later, undoing placed files puts the old ones back
    let record = plan.record.clone();
    assert!(record.files.contains_key("mods/new.jar"));
    let (v1_again, fake1_again) = test_pack(1);
    let back = files_of(&v1_again.path).await;
    let plan = plan_files(root, &record, &back);
    stage_files(
        &fake1_again,
        Some(&v1_again.path),
        &sources_of(&back),
        &plan,
        root,
        false,
        None,
        &|_| {},
    )
    .await
    .unwrap();
    place_files(root, &plan).unwrap();
    assert_eq!(read_file(root, "mods/new.jar"), None);
    undo_placed_files(root, &plan);
    assert_eq!(
        read_file(root, "mods/new.jar").as_deref(),
        Some(&b"new"[..])
    );
    assert_eq!(
        read_file(root, "config/pack.toml").as_deref(),
        Some(&b"v2"[..])
    );
    assert_eq!(read_file(root, "mods/dropped.jar"), None);
}

#[cfg(unix)]
#[test]
fn files_behind_a_linked_folder_are_not_moved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    write(&outside, "a.jar", b"a");
    std::os::unix::fs::symlink(&outside, root.join("mods")).unwrap();
    let old = PackRecord {
        files: BTreeMap::from([("mods/a.jar".into(), recorded(b"a", false, None))]),
    };
    let plan = plan_files(&root, &old, &BTreeMap::new());
    assert_eq!(change(&plan, "mods/a.jar"), Some(FileAction::Remove));
    assert!(place_files(&root, &plan).is_err());
    assert!(outside.join("a.jar").is_file());
}

/// What an update planned is checked again right before placement: a file it replaces or
/// removes that changed since, or a file where it adds one, refuses it; and placing a new
/// file never replaces one that appeared after the check.
#[test]
fn changes_since_the_plan_refuse_the_placement() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "mods/a.jar", b"a1");
    write(root, "mods/gone.jar", b"gone");
    let old = PackRecord {
        files: BTreeMap::from([
            ("mods/a.jar".into(), recorded(b"a1", false, Some("a"))),
            ("mods/gone.jar".into(), recorded(b"gone", false, Some("g"))),
        ]),
    };
    let new = BTreeMap::from([
        ("mods/a.jar".to_string(), download("mods/a.jar", b"a2", "a")),
        (
            "mods/new.jar".to_string(),
            download("mods/new.jar", b"new", "n"),
        ),
    ]);
    let plan = plan_files(root, &old, &new);
    assert_eq!(change(&plan, "mods/a.jar"), Some(FileAction::Replace));
    assert_eq!(change(&plan, "mods/gone.jar"), Some(FileAction::Remove));
    assert_eq!(change(&plan, "mods/new.jar"), Some(FileAction::Add));
    check_as_planned(root, &plan).unwrap();

    for (path, bytes) in [
        ("mods/a.jar", &b"edited"[..]),
        ("mods/gone.jar", b"edited"),
        ("mods/new.jar", b"uploaded meanwhile"),
    ] {
        let before = std::fs::read(root.join(path)).ok();
        write(root, path, bytes);
        let error = check_as_planned(root, &plan).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Conflict), "{path}");
        assert!(error.source.to_string().contains(path), "{}", error.source);
        match before {
            Some(before) => write(root, path, &before),
            None => std::fs::remove_file(root.join(path)).unwrap(),
        }
    }
    check_as_planned(root, &plan).unwrap();

    // a file that appears after the check is never replaced by the placement
    write(root, &format!("{STAGING_DIR}/mods/new.jar"), b"new");
    write(root, &format!("{STAGING_DIR}/mods/a.jar"), b"a2");
    write(root, "mods/new.jar", b"uploaded after the check");
    let error = place_files(root, &plan).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Conflict));
    assert_eq!(
        std::fs::read(root.join("mods/new.jar")).unwrap(),
        b"uploaded after the check"
    );
    // and everything it moved is back
    assert_eq!(std::fs::read(root.join("mods/a.jar")).unwrap(), b"a1");
    assert_eq!(std::fs::read(root.join("mods/gone.jar")).unwrap(), b"gone");
}

/// Undoing a placement never replaces a file that appeared where an old one goes back (a
/// copy or unzip job, which no operation keeps out): that file stays, and the old copy is
/// kept in an `interrupted-*` folder rather than deleted with the snapshot.
#[test]
fn undoing_never_replaces_what_appeared_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, &format!("{SNAPSHOT_DIR}/mods/a.jar"), b"a1");
    write(root, &format!("{SNAPSHOT_DIR}/mods/b.jar"), b"b1");
    write(root, "mods/a.jar", b"written by a job meanwhile");
    Moves {
        saved: vec!["mods/a.jar".into(), "mods/b.jar".into()],
        placed: Vec::new(),
    }
    .undo(root);
    assert_eq!(
        read_file(root, "mods/a.jar").as_deref(),
        Some(&b"written by a job meanwhile"[..])
    );
    assert_eq!(read_file(root, "mods/b.jar").as_deref(), Some(&b"b1"[..]));
    assert!(!root.join(SNAPSHOT_DIR).exists());
    let kept: Vec<PathBuf> = std::fs::read_dir(root.join(MODPACK_DIR))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("interrupted-"))
        })
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(std::fs::read(kept[0].join("mods/a.jar")).unwrap(), b"a1");
}
