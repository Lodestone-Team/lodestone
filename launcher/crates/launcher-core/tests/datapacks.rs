//! A world's data packs: listing them with their state from `level.dat`, adding zips and
//! folders, turning them on and off, and deleting them.

use std::{fs, io::Write as _, path::Path};

use launcher_core::{
    content::InstanceDirs,
    datapacks::{self, PackState},
    worlds::{self, WorldError},
};
use lodestone_minecraft::level::{self, DataPackLists};

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
