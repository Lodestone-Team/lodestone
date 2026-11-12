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
        stored: None,
    }
}

fn override_file(content: &[u8]) -> PackFile {
    PackFile {
        sha1: sha1_hex(content),
        download: None,
        from_overrides: true,
        project_id: None,
        stored: None,
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
    ] {
        assert!(record_path(bad).is_none(), "{bad}");
    }
    assert!(record_path("mods/a.jar").is_some());

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

    // version 1 as a new server: its files are recorded
    let (v1, fake1) = test_pack(1);
    let parsed = read_pack(&v1.path, false).unwrap();
    install_pack_contents(&fake1, &parsed, &v1.path, root, false, &|_, _| {})
        .await
        .unwrap();
    let record = PackRecord::load(root).await.unwrap();
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
    stage_files(&fake2, Some(&v2.path), &new, &plan, root, false, &|_| {})
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
    stage_files(&fake2, Some(&v2.path), &new, &plan, root, false, &|_| {})
        .await
        .unwrap();
    place_files(root, &plan).unwrap();
    plan.record.save(root).await.unwrap();
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
    let record = PackRecord::load(root).await.unwrap();
    assert!(record.files.contains_key("mods/new.jar"));
    let (v1_again, fake1_again) = test_pack(1);
    let back = files_of(&v1_again.path).await;
    let plan = plan_files(root, &record, &back);
    stage_files(
        &fake1_again,
        Some(&v1_again.path),
        &back,
        &plan,
        root,
        false,
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
