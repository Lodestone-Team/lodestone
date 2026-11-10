use std::{fs, io::Write, path::Path};

use super::*;

fn node(file: &str, provides: &[&str], requires: &[&str]) -> ModNode {
    ModNode {
        file_name: file.into(),
        name: file.trim_end_matches(".jar").into(),
        provides: provides.iter().map(|s| s.to_string()).collect(),
        requires: requires.iter().map(|s| s.to_string()).collect(),
    }
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Plays a whole search where `culprit` crashes the game whenever it is on.
fn run(mods: &Mods, files: &[String], suspects: &[String], culprit: &str) -> (Step, usize) {
    let mut search = Search::new(files.to_vec(), suspects, true);
    let mut tries = 0;
    loop {
        match search.next(mods) {
            Step::Test(off) => {
                tries += 1;
                assert!(tries < 40, "the search doesn't end");
                search.record(!off.contains(culprit));
            }
            done => return (done, tries),
        }
    }
}

#[test]
fn turning_a_library_off_turns_off_what_needs_it() {
    let mods = Mods::new(vec![
        node(
            "api.jar",
            &["fabric-api", "fabric-api-base"],
            &["fabricloader"],
        ),
        node("lib.jar", &["cloth-config"], &["fabric-api-base"]),
        node("a.jar", &["a"], &["cloth-config", "minecraft"]),
        node("b.jar", &["b"], &["a"]),
        node("c.jar", &["c"], &["minecraft"]),
    ]);
    assert_eq!(
        mods.closure(&set(&["lib.jar"])),
        set(&["lib.jar", "a.jar", "b.jar"])
    );
    assert_eq!(
        mods.closure(&set(&["api.jar"])),
        set(&["api.jar", "lib.jar", "a.jar", "b.jar"])
    );
    assert_eq!(mods.closure(&set(&["c.jar"])), set(&["c.jar"]));
}

#[test]
fn an_id_two_mods_give_needs_both_off() {
    let mods = Mods::new(vec![
        node("old.jar", &["lib"], &[]),
        node("new.jar", &["lib"], &[]),
        node("user.jar", &["user"], &["lib"]),
    ]);
    assert_eq!(mods.closure(&set(&["old.jar"])), set(&["old.jar"]));
    assert_eq!(
        mods.closure(&set(&["old.jar", "new.jar"])),
        set(&["old.jar", "new.jar", "user.jar"])
    );
}

/// 120 mods: a few libraries, mods that need them, and mods that need those.
fn pack() -> (Mods, Vec<String>) {
    let mut nodes = vec![
        node("fabric-api.jar", &["fabric-api"], &[]),
        node("cloth.jar", &["cloth-config"], &["fabric-api"]),
        node("owo.jar", &["owo"], &[]),
    ];
    for i in 0..117 {
        let needs: &[&str] = match i % 4 {
            0 => &["fabric-api"],
            1 => &["cloth-config"],
            2 => &["owo", "fabric-api"],
            _ => &[],
        };
        nodes.push(node(&format!("mod{i:03}.jar"), &[], needs));
    }
    // A mod that needs another mod (not a library).
    nodes.push(node("addon.jar", &[], &["mod005"]));
    nodes[8].provides = vec!["mod005".into()];
    let files = nodes.iter().map(|n| n.file_name.clone()).collect();
    (Mods::new(nodes), files)
}

#[test]
fn finds_any_culprit_in_a_few_tries() {
    let (mods, files) = pack();
    for culprit in &files {
        let (step, tries) = run(&mods, &files, &[], culprit);
        assert_eq!(
            step,
            Step::Found(vec![culprit.clone()]),
            "looking for {culprit}"
        );
        assert!(tries <= 9, "{culprit} took {tries} tries");
    }
}

#[test]
fn the_suspect_first_saves_tries() {
    let (mods, files) = pack();
    let culprit = "mod042.jar".to_string();
    let (step, tries) = run(&mods, &files, std::slice::from_ref(&culprit), &culprit);
    assert_eq!(step, Step::Found(vec![culprit]));
    assert_eq!(tries, 1);
    // A wrong suspect costs one try.
    let (step, tries) = run(&mods, &files, &["mod007.jar".into()], "mod100.jar");
    assert_eq!(step, Step::Found(vec!["mod100.jar".into()]));
    assert!(tries <= 9);
}

#[test]
fn a_crash_no_mod_causes_is_not_found() {
    let (mods, files) = pack();
    let (step, _) = run(&mods, &files, &[], "not-a-mod");
    assert_eq!(step, Step::NotFound);
}

#[test]
fn mods_that_need_each_other_are_found_together() {
    let mods = Mods::new(vec![
        node("a.jar", &["a"], &["b"]),
        node("b.jar", &["b"], &["a"]),
    ]);
    let files = vec!["a.jar".to_string(), "b.jar".to_string()];
    // One try, with both off, to be sure they are the cause.
    assert_eq!(
        run(&mods, &files, &[], "a.jar"),
        (Step::Found(vec!["a.jar".into(), "b.jar".into()]), 1)
    );
}

#[test]
fn the_last_mod_is_only_blamed_once_turning_it_off_helps() {
    let files: Vec<String> = (0..8).map(|i| format!("m{i}.jar")).collect();
    let mods = Mods::new(files.iter().map(|f| node(f, &[], &[])).collect());
    // Every try crashes until the last, which turns the culprit off alone.
    let culprit = {
        let mut search = Search::new(files.clone(), &[], true);
        while search.candidates.len() > 1 {
            assert!(matches!(search.next(&mods), Step::Test(_)));
            search.record(true);
        }
        search.candidates[0].clone()
    };
    let (step, tries) = run(&mods, &files, &[], &culprit);
    assert_eq!(step, Step::Found(vec![culprit]));
    assert_eq!(tries, 4);
}

#[test]
fn counts_the_tries_left() {
    let files: Vec<String> = (0..200).map(|i| format!("m{i}.jar")).collect();
    let mut search = Search::new(files, &[], true);
    assert_eq!(search.tries_left(), 8);
    search.candidates.truncate(2);
    assert_eq!(search.tries_left(), 1);
    search.candidates.truncate(1);
    assert_eq!(search.tries_left(), 0);
}

fn jar(path: &Path, fabric_mod_json: &str) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(fabric_mod_json.as_bytes()).unwrap();
    zip.finish().unwrap();
}

#[test]
fn dependents_are_the_enabled_mods_that_need_it() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(dir.path());
    let mods_dir = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods_dir).unwrap();
    jar(
        &mods_dir.join("api.jar"),
        r#"{"id": "fabric-api", "name": "Fabric API"}"#,
    );
    jar(
        &mods_dir.join("sodium.jar"),
        r#"{"id": "sodium", "name": "Sodium", "depends": {"fabric-api": "*", "minecraft": "*"}}"#,
    );
    // Needs Sodium, so it goes too, through it.
    jar(
        &mods_dir.join("extra.jar"),
        r#"{"id": "sodium-extra", "name": "Sodium Extra", "depends": {"sodium": "*"}}"#,
    );
    jar(
        &mods_dir.join("culling.jar"),
        r#"{"id": "entityculling", "name": "Entity Culling", "depends": {"fabric-api": "*"}}"#,
    );
    jar(&mods_dir.join("alone.jar"), r#"{"id": "alone"}"#);
    // Already off: not asked about.
    jar(
        &mods_dir.join("off.jar.disabled"),
        r#"{"id": "off", "depends": {"fabric-api": "*"}}"#,
    );

    let names = |list: Vec<Dependent>| -> Vec<(String, String)> {
        list.into_iter().map(|d| (d.file_name, d.name)).collect()
    };
    assert_eq!(
        names(dependents(&dirs, "api.jar")),
        [
            ("culling.jar".to_string(), "Entity Culling".to_string()),
            ("sodium.jar".to_string(), "Sodium".to_string()),
            ("extra.jar".to_string(), "Sodium Extra".to_string()),
        ]
    );
    assert_eq!(
        names(dependents(&dirs, "sodium.jar")),
        [("extra.jar".to_string(), "Sodium Extra".to_string())]
    );
    assert!(dependents(&dirs, "alone.jar").is_empty());
    // A disabled mod has no enabled mods relying on it.
    assert!(dependents(&dirs, "off.jar.disabled").is_empty());

    // Another mod that also gives the id stands in for it.
    jar(
        &mods_dir.join("api-copy.jar"),
        r#"{"id": "other", "provides": ["fabric-api"]}"#,
    );
    assert!(dependents(&dirs, "api.jar").is_empty());
}

#[test]
fn applies_tries_and_restores_the_mods() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(dir.path());
    let mods_dir = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods_dir).unwrap();
    jar(
        &mods_dir.join("lib.jar"),
        r#"{"id": "lib", "name": "Library"}"#,
    );
    jar(
        &mods_dir.join("user.jar"),
        r#"{"id": "user", "depends": {"lib": "*", "minecraft": "1.21"}}"#,
    );
    jar(&mods_dir.join("other.jar"), r#"{"id": "other"}"#);
    // Off before the search: never touched.
    jar(&mods_dir.join("off.jar.disabled"), r#"{"id": "off"}"#);

    let mut search = Search::start(&dirs, &[], true);
    assert_eq!(search.mods, ["lib.jar", "other.jar", "user.jar"]);
    let mods = Mods::read(&dirs, &search.mods);
    assert_eq!(mods.name("lib.jar"), "Library");
    assert_eq!(
        mods.closure(&set(&["lib.jar"])),
        set(&["lib.jar", "user.jar"])
    );

    apply(&dirs, &mut search, &set(&["lib.jar", "user.jar"])).unwrap();
    assert!(mods_dir.join("lib.jar.disabled").exists());
    assert!(mods_dir.join("user.jar.disabled").exists());
    assert_eq!(load(&dirs).unwrap(), search);

    apply(&dirs, &mut search, &set(&["other.jar"])).unwrap();
    assert!(mods_dir.join("lib.jar").exists());
    assert!(mods_dir.join("user.jar").exists());
    assert!(mods_dir.join("other.jar.disabled").exists());

    // The user deleted a mod meanwhile: fine.
    fs::remove_file(mods_dir.join("other.jar.disabled")).unwrap();
    restore(&dirs, &search, &set(&["user.jar"])).unwrap();
    assert!(mods_dir.join("lib.jar").exists());
    assert!(mods_dir.join("user.jar.disabled").exists());
    assert!(mods_dir.join("off.jar.disabled").exists());
    assert!(load(&dirs).is_none());
}
