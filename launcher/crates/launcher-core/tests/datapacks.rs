//! A world's data packs: listing them with their state from `level.dat`, adding zips and
//! folders, turning them on and off, deleting them, and installing them from Modrinth
//! (against a loopback mock).

mod modrinth;

use std::{fs, io::Write as _, path::Path};

use futures::executor::block_on;
use launcher_core::{
    content::{InstanceDirs, Modrinth},
    datapacks::{self, Installed, PackState},
    game::download::{CancelToken, Downloader, Progress, recording_sleep},
    http,
    worlds::{self, WorldError},
};
use lodestone_minecraft::level::{self, DataPackLists};
use modrinth::{FakeVersion, ModrinthMock};

const LEVEL_DAT: &[u8] =
    include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");
const OLD_LEVEL_DAT: &[u8] =
    include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.8.9.dat");

const MCMETA: &[u8] = br#"{"pack": {"pack_format": 48, "description": "A test pack"}}"#;

fn instance() -> (tempfile::TempDir, InstanceDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(temp.path().join("instance"));
    fs::create_dir_all(&dirs.game).unwrap();
    (temp, dirs)
}

fn make_world(dirs: &InstanceDirs, folder: &str, level_dat: &[u8]) {
    let dir = worlds::saves_dir(dirs).join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("level.dat"), level_dat).unwrap();
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn lists(dirs: &InstanceDirs, world: &str) -> DataPackLists {
    let bytes = fs::read(worlds::saves_dir(dirs).join(world).join("level.dat")).unwrap();
    level::data_packs(&level::read_level(&bytes).unwrap()).unwrap()
}

#[test]
fn add_list_toggle_and_delete() {
    let (temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    assert_eq!(
        datapacks::list(&dirs, "World").unwrap().packs,
        Vec::new(),
        "no datapacks folder yet"
    );

    // a zip and a folder
    let zip = temp.path().join("Tools.zip");
    write_zip(
        &zip,
        &[
            ("pack.mcmeta", MCMETA),
            ("data/test/function/hi.mcfunction", b"say hi"),
        ],
    );
    let folder = temp.path().join("Folder Pack");
    fs::create_dir_all(folder.join("data/test")).unwrap();
    fs::write(folder.join("pack.mcmeta"), MCMETA).unwrap();
    fs::write(folder.join("data/test/x.json"), b"{}").unwrap();
    assert_eq!(datapacks::add(&dirs, "World", &zip).unwrap(), "Tools.zip");
    assert_eq!(
        datapacks::add(&dirs, "World", &folder).unwrap(),
        "Folder Pack"
    );
    assert!(matches!(
        datapacks::add(&dirs, "World", &zip),
        Err(WorldError::Conflict(_))
    ));

    let listed = datapacks::list(&dirs, "World").unwrap();
    assert!(listed.switchable);
    let names: Vec<&str> = listed.packs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Folder Pack", "Tools.zip"]);
    for pack in &listed.packs {
        assert_eq!(pack.state, PackState::New, "{}", pack.name);
        assert_eq!(pack.meta.description.as_deref(), Some("A test pack"));
        assert_eq!(pack.meta.problem, None);
        assert_eq!(
            datapacks::compatibility(pack.meta.formats.as_ref(), "1.21.1"),
            launcher_core::content::packs::Compatibility::Compatible
        );
    }

    // off, then on again (last, so it loads over the others)
    datapacks::set_enabled(&dirs, "World", "Tools.zip", false).unwrap();
    assert!(
        lists(&dirs, "World")
            .disabled
            .contains(&"file/Tools.zip".to_string())
    );
    let tools = |dirs: &InstanceDirs| {
        datapacks::list(dirs, "World")
            .unwrap()
            .packs
            .into_iter()
            .find(|p| p.name == "Tools.zip")
            .unwrap()
    };
    assert_eq!(tools(&dirs).state, PackState::Disabled);
    datapacks::set_enabled(&dirs, "World", "Tools.zip", true).unwrap();
    let after = lists(&dirs, "World");
    assert_eq!(
        after.enabled.last().map(String::as_str),
        Some("file/Tools.zip")
    );
    assert!(!after.disabled.contains(&"file/Tools.zip".to_string()));
    assert_eq!(tools(&dirs).state, PackState::Enabled);
    // the rest of level.dat is as it was
    let summary = |bytes: &[u8]| level::summarize(bytes).unwrap();
    let written = fs::read(worlds::saves_dir(&dirs).join("World/level.dat")).unwrap();
    assert_eq!(summary(&written), summary(LEVEL_DAT));

    assert!(matches!(
        datapacks::set_enabled(&dirs, "World", "Missing.zip", true),
        Err(WorldError::NotFound(_))
    ));

    // deleting drops it from the lists too
    datapacks::delete_with(&dirs, "World", "Tools.zip", |path| {
        fs::remove_file(path).map_err(|e| e.to_string())
    })
    .unwrap();
    assert!(
        !lists(&dirs, "World")
            .enabled
            .contains(&"file/Tools.zip".to_string())
    );
    let names: Vec<String> = datapacks::list(&dirs, "World")
        .unwrap()
        .packs
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Folder Pack"]);
}

#[test]
fn refuses_what_isnt_a_data_pack() {
    let (temp, dirs) = instance();
    make_world(&dirs, "World", LEVEL_DAT);
    let jar = temp.path().join("mod.jar");
    write_zip(&jar, &[("fabric.mod.json", b"{}")]);
    assert!(
        datapacks::check_addition(&jar)
            .unwrap_err()
            .contains("Mods page")
    );
    let resource_pack = temp.path().join("textures.zip");
    write_zip(
        &resource_pack,
        &[("pack.mcmeta", MCMETA), ("assets/minecraft/x.png", b"x")],
    );
    assert!(
        datapacks::check_addition(&resource_pack)
            .unwrap_err()
            .contains("no data folder")
    );
    let no_meta = temp.path().join("bare.zip");
    write_zip(&no_meta, &[("data/x/y.json", b"{}")]);
    assert!(datapacks::check_addition(&no_meta).is_err());
    let text = temp.path().join("notes.txt");
    fs::write(&text, b"hi").unwrap();
    assert!(datapacks::check_addition(&text).is_err());
    // unknown worlds and names that leave the folder
    assert!(matches!(
        datapacks::list(&dirs, "../World"),
        Err(WorldError::NotFound(_))
    ));
    assert!(datapacks::set_enabled(&dirs, "World", "../level.dat", false).is_err());
}

#[test]
fn old_worlds_have_no_lists() {
    let (temp, dirs) = instance();
    make_world(&dirs, "Old", OLD_LEVEL_DAT);
    let zip = temp.path().join("Pack.zip");
    write_zip(&zip, &[("pack.mcmeta", MCMETA), ("data/a/b.json", b"{}")]);
    datapacks::add(&dirs, "Old", &zip).unwrap();
    let listed = datapacks::list(&dirs, "Old").unwrap();
    assert!(!listed.switchable);
    assert_eq!(listed.packs[0].state, PackState::New);
    let before = fs::read(worlds::saves_dir(&dirs).join("Old/level.dat")).unwrap();
    assert!(matches!(
        datapacks::set_enabled(&dirs, "Old", "Pack.zip", false),
        Err(WorldError::Invalid(_))
    ));
    let after = fs::read(worlds::saves_dir(&dirs).join("Old/level.dat")).unwrap();
    assert_eq!(before, after, "level.dat is left alone");
}

// ---- Modrinth installs (LS-24) ----

fn zip_bytes(temp: &Path, marker: &str) -> Vec<u8> {
    let path = temp.join(format!("{marker}.zip"));
    write_zip(
        &path,
        &[
            ("pack.mcmeta", MCMETA),
            ("data/a/b.json", marker.as_bytes()),
        ],
    );
    fs::read(path).unwrap()
}

/// Project "DPK" with versions 1 and 2, and "OTHER" whose only file is named like
/// DPK 1's.
fn data_pack_mock(temp: &Path) -> ModrinthMock {
    let mock = ModrinthMock::start();
    let version = |id: &str, project: &str, number: &str, file: &str, date: &str| {
        FakeVersion::new(id, project, number, &["datapack"], &["1.21.11"], date)
            .file(file, &zip_bytes(temp, id))
    };
    mock.project("DPK", "dpk", "datapack")
        .project("OTHER", "other", "datapack")
        .version(version(
            "d1",
            "DPK",
            "1",
            "dpk-1.zip",
            "2024-01-01T00:00:00Z",
        ))
        .version(version(
            "d2",
            "DPK",
            "2",
            "dpk-2.zip",
            "2024-02-01T00:00:00Z",
        ))
        .version(version(
            "o1",
            "OTHER",
            "1",
            "dpk-1.zip",
            "2024-01-01T00:00:00Z",
        ));
    mock
}

fn install(
    api: &Modrinth,
    dirs: &InstanceDirs,
    world: &str,
    id: &str,
) -> Result<Installed, WorldError> {
    let downloader = Downloader::new(http::download_client().unwrap(), recording_sleep().0);
    let version = block_on(api.version(id)).unwrap();
    block_on(datapacks::install_with(
        api,
        &downloader,
        dirs,
        world,
        &version,
        &Progress::default(),
        &CancelToken::new(),
        |path| fs::remove_file(path).map_err(|e| e.to_string()),
    ))
}

fn pack_names(dirs: &InstanceDirs, world: &str) -> Vec<(String, Option<String>)> {
    datapacks::list(dirs, world)
        .unwrap()
        .packs
        .into_iter()
        .map(|p| (p.name, p.modrinth_project))
        .collect()
}

fn named(name: &str, project: Option<&str>) -> (String, Option<String>) {
    (name.to_string(), project.map(String::from))
}

#[test]
fn modrinth_packs_follow_their_world_when_it_is_renamed() {
    let (temp, dirs) = instance();
    let mock = data_pack_mock(temp.path());
    let api = mock.client();
    make_world(&dirs, "New World", LEVEL_DAT);
    install(&api, &dirs, "New World", "d1").unwrap();
    datapacks::set_enabled(&dirs, "New World", "dpk-1.zip", false).unwrap();

    let folder = worlds::rename(&dirs, "New World", "Survival").unwrap();
    assert_eq!(folder, "Survival");
    assert_eq!(
        pack_names(&dirs, &folder),
        [named("dpk-1.zip", Some("DPK"))]
    );
    // the newer version replaces the old file and keeps it off
    let installed = install(&api, &dirs, &folder, "d2").unwrap();
    assert_eq!(installed.replaced.as_deref(), Some("dpk-1.zip"));
    assert_eq!(
        pack_names(&dirs, &folder),
        [named("dpk-2.zip", Some("DPK"))]
    );
    assert!(
        lists(&dirs, &folder)
            .disabled
            .contains(&"file/dpk-2.zip".to_string())
    );
}

#[test]
fn a_deleted_world_takes_its_modrinth_records_along() {
    let (temp, dirs) = instance();
    let mock = data_pack_mock(temp.path());
    let api = mock.client();
    make_world(&dirs, "World", LEVEL_DAT);
    install(&api, &dirs, "World", "d1").unwrap();
    worlds::delete_with(&dirs, "World", |path| {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    })
    .unwrap();
    // a new world in the same folder, with the user's own pack of the same name
    make_world(&dirs, "World", LEVEL_DAT);
    let own = temp.path().join("dpk-1.zip");
    write_zip(&own, &[("pack.mcmeta", MCMETA), ("data/mine.json", b"{}")]);
    fs::create_dir_all(datapacks::dir(&dirs, "World").unwrap()).unwrap();
    fs::copy(
        &own,
        datapacks::dir(&dirs, "World").unwrap().join("dpk-1.zip"),
    )
    .unwrap();
    assert_eq!(pack_names(&dirs, "World"), [named("dpk-1.zip", None)]);
    let installed = install(&api, &dirs, "World", "d2").unwrap();
    assert_eq!(installed.replaced, None);
    assert_eq!(
        fs::read(datapacks::dir(&dirs, "World").unwrap().join("dpk-1.zip")).unwrap(),
        fs::read(&own).unwrap(),
        "the user's pack is left alone"
    );
}

#[test]
fn stale_records_never_take_a_users_file() {
    let (temp, dirs) = instance();
    let mock = data_pack_mock(temp.path());
    let api = mock.client();
    make_world(&dirs, "World", LEVEL_DAT);
    let folder = datapacks::dir(&dirs, "World").unwrap();
    let own = temp.path().join("own.zip");
    write_zip(&own, &[("pack.mcmeta", MCMETA), ("data/mine.json", b"{}")]);

    // Adding a file under a name a gone Modrinth install had drops its record.
    install(&api, &dirs, "World", "d1").unwrap();
    fs::remove_file(folder.join("dpk-1.zip")).unwrap();
    let added = temp.path().join("add").join("dpk-1.zip");
    fs::create_dir_all(added.parent().unwrap()).unwrap();
    fs::copy(&own, &added).unwrap();
    datapacks::add(&dirs, "World", &added).unwrap();
    assert_eq!(pack_names(&dirs, "World"), [named("dpk-1.zip", None)]);
    assert_eq!(install(&api, &dirs, "World", "d2").unwrap().replaced, None);
    assert_eq!(
        fs::read(folder.join("dpk-1.zip")).unwrap(),
        fs::read(&own).unwrap()
    );
    fs::remove_file(folder.join("dpk-2.zip")).unwrap();
    fs::remove_file(folder.join("dpk-1.zip")).unwrap();

    // A file swapped in outside the app under the installed name isn't the installed
    // version: a newer version goes beside it, and a version with that same name is
    // refused rather than overwriting it.
    install(&api, &dirs, "World", "d1").unwrap();
    fs::copy(&own, folder.join("dpk-1.zip")).unwrap();
    assert_eq!(install(&api, &dirs, "World", "d2").unwrap().replaced, None);
    assert_eq!(
        fs::read(folder.join("dpk-1.zip")).unwrap(),
        fs::read(&own).unwrap()
    );
    fs::remove_file(folder.join("dpk-2.zip")).unwrap();
    assert!(matches!(
        install(&api, &dirs, "World", "o1"),
        Err(WorldError::Conflict(_))
    ));
    assert_eq!(
        fs::read(folder.join("dpk-1.zip")).unwrap(),
        fs::read(&own).unwrap()
    );
}

#[test]
fn records_without_a_hash_are_checked_with_modrinth() {
    let (temp, dirs) = instance();
    let mock = data_pack_mock(temp.path());
    let api = mock.client();
    make_world(&dirs, "World", LEVEL_DAT);
    let folder = datapacks::dir(&dirs, "World").unwrap();
    fs::create_dir_all(&folder).unwrap();
    let record = |file: &str| {
        let json = serde_json::json!({ "worlds": { "World": {
            file: { "project_id": "DPK", "version_id": "d1" },
        }}});
        fs::write(dirs.instance.join("datapacks.json"), json.to_string()).unwrap();
    };

    // the user's file under an old record: Modrinth doesn't know it, so it stays
    let own = temp.path().join("own.zip");
    write_zip(&own, &[("pack.mcmeta", MCMETA), ("data/mine.json", b"{}")]);
    fs::copy(&own, folder.join("dpk-1.zip")).unwrap();
    record("dpk-1.zip");
    assert_eq!(install(&api, &dirs, "World", "d2").unwrap().replaced, None);
    assert_eq!(
        fs::read(folder.join("dpk-1.zip")).unwrap(),
        fs::read(&own).unwrap()
    );
    fs::remove_file(folder.join("dpk-2.zip")).unwrap();

    // DPK 1 itself under an old record: replaced as before
    let d1 = mock
        .state()
        .versions
        .iter()
        .find(|v| v.id == "d1")
        .unwrap()
        .bytes
        .clone();
    fs::write(folder.join("dpk-1.zip"), d1).unwrap();
    record("dpk-1.zip");
    assert_eq!(
        install(&api, &dirs, "World", "d2")
            .unwrap()
            .replaced
            .as_deref(),
        Some("dpk-1.zip")
    );
    assert_eq!(
        pack_names(&dirs, "World"),
        [named("dpk-2.zip", Some("DPK"))]
    );
}
