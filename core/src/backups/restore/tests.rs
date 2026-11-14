//! Restores from data images, on temporary server folders and stores: what is replaced and
//! kept, the Paper layout, the journaled swap and its recovery after a crash at every step.

use std::collections::BTreeMap;

use lodestone_image::store::snapshot::{self as snap, SnapshotOptions};
use lodestone_image::store::{Encoding, NEVER};
use lodestone_image::{Kind, Producer};

use super::*;
use crate::backups::take::tests::{paper, request, take_image, uuid, write};
use crate::images::layout::game_of;
use crate::images::scope::content_filter;
use crate::implementations::minecraft::Flavour;
use crate::traits::t_configurable::GameType;

struct Env {
    _dir: tempfile::TempDir,
    images: ImageStore,
    store: Store,
    instances: PathBuf,
    root: PathBuf,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let images = ImageStore::new(dir.path().join("backups"), Encoding::Zstd);
    let store = Store::new(dir.path().join("backups"));
    let instances = dir.path().join("instances");
    let root = instances.join("Test-12345678");
    fs::create_dir_all(&root).unwrap();
    let config = DotLodestoneConfig::new(uuid(), GameType::MinecraftJava);
    fs::write(
        root.join(".lodestone_config"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    Env {
        _dir: dir,
        images,
        store,
        instances,
        root,
    }
}

impl Env {
    fn ctx(&self, flavour: Flavour, policy_exclude: &[&str]) -> InstanceContext {
        InstanceContext {
            uuid: uuid(),
            root: self.root.clone(),
            game_version: "1.21.1".to_string(),
            flavour,
            level_name: "world".to_string(),
            policy_exclude: policy_exclude.iter().map(|s| s.to_string()).collect(),
            running: false,
        }
    }

    /// A backup of the server now: its image and level.
    fn backup(&self, flavour: Flavour, policy_exclude: &[&str]) -> (Image, Option<String>) {
        let (prepared, _, image) =
            take_image(&self.images, request(&self.root, flavour, policy_exclude));
        let level = prepared.level_in(&image);
        (image, level)
    }

    /// The server's content now as a content image, saved.
    fn content_image(&self, flavour: Flavour) -> Image {
        let (layout, _) = server_layout(&self.root, "world", &flavour).unwrap();
        let options = SnapshotOptions {
            filter: Some(&content_filter),
            threads: 1,
            ..SnapshotOptions::default()
        };
        let snapshot = snap::snapshot(&self.images, &self.root, &layout, &options).unwrap();
        let mut image = Image::new(Kind::Content, game_of("1.21.1", &flavour));
        snapshot.fill(&mut image);
        image.worlds.clear();
        image.producer = Producer::default();
        image.normalize();
        image.seal();
        self.images.save_manifest(uuid().as_ref(), &image).unwrap();
        image
    }

    fn plan(&self, source: &Source, scope: RestoreScope, ctx: &InstanceContext) -> RestorePlan {
        plan(&self.images, ctx, source, scope).unwrap()
    }

    fn stage(&self, plan: &RestorePlan, pin: &Pin) -> Staged {
        stage(&self.images, &self.root, plan, pin, &mut |_, _| {}, NEVER).unwrap()
    }

    fn restore(&self, source: &Source, scope: RestoreScope, ctx: &InstanceContext, pin: &Pin) {
        let plan = self.plan(source, scope, ctx);
        self.stage(&plan, pin)
            .swap(&self.store.journal_path(&uuid()).unwrap())
            .unwrap();
    }

    fn journal(&self) -> PathBuf {
        self.store.journal_path(&uuid()).unwrap()
    }

    fn recover(&self) -> Recovery {
        recover_with(&self.store, &self.instances, Duration::ZERO)
    }
}

/// A data image as a source, without a content image.
fn everything<'a>(image: &'a Image, level: Option<&'a str>) -> Source<'a> {
    Source {
        image,
        level,
        content: None,
    }
}

/// Every file and folder under `root`, with file contents.
fn snapshot(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
        let entry = entry.unwrap();
        let rel = relative_to(root, entry.path()).unwrap();
        let contents = if entry.file_type().is_file() {
            Some(fs::read(entry.path()).unwrap())
        } else if entry.file_type().is_symlink() {
            Some(b"<link>".to_vec())
        } else {
            None
        };
        out.insert(rel, contents);
    }
    out
}

fn read(root: &Path, path: &str) -> Option<String> {
    fs::read_to_string(join_rel(root, path)).ok()
}

/// Links `link` to the folder `target`; false where links can't be made.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        false
    }
}

fn no_restore_folders(root: &Path) -> bool {
    !fs::read_dir(root).unwrap().flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .starts_with(".lodestone-restore")
    })
}

fn base_server(root: &Path) {
    write(root, "server.properties", b"motd=hi\nserver-port=25565\n");
    write(
        root,
        CONFIG_FILE,
        br#"{"name":"Old name","port":25565,"java_cmd":null,"cmd_args":[],"version":"1.20"}"#,
    );
    write(root, "server.jar", b"jar v1");
    write(root, "world/level.dat", b"level v1");
    write(root, "world/region/r.0.0.mca", b"region v1");
    write(root, "world_nether/DIM-1/r.mca", b"nether v1");
    write(root, "plugins/dynmap/config.txt", b"dynmap config v1");
}

#[test]
fn restore_everything() {
    let env = env();
    let root = &env.root;
    base_server(root);
    let exclude = ["plugins/dynmap/web/tiles/", "/old-worlds/"];
    let (image, level) = env.backup(Flavour::Vanilla, &exclude);

    // the server changes after the backup
    write(root, "server.jar", b"jar v2");
    write(root, "world/level.dat", b"level v2");
    write(root, "world/new-file.dat", b"new");
    fs::remove_file(root.join("world_nether/DIM-1/r.mca")).unwrap();
    write(root, "world/session.lock", b"lock");
    write(root, "added-later.txt", b"added");
    write(root, "logs/latest.log", b"log");
    write(root, "old-worlds/w/level.dat", b"excluded top level");
    write(root, "plugins/dynmap/web/tiles/0.png", b"tile");
    write(root, "plugins/dynmap/config.txt", b"dynmap config v2");
    write(root, CONFIG_FILE, br#"{"name":"New name","port":25570,"java_cmd":"/opt/java","cmd_args":["-Dx"],"version":"1.21","future_field":1}"#);
    write(
        root,
        "server.properties",
        b"motd=changed\nserver-port=25570\n",
    );
    let config_before = fs::read(root.join(".lodestone_config")).unwrap();
    let outside = env.instances.parent().unwrap().join("outside");
    write(&outside, "data.txt", b"outside");
    let linked = link_dir(&outside, &root.join("linked-top"))
        && link_dir(&outside, &root.join("world/linked-inside"));

    let pin = Pin {
        name: Some("New name".to_string()),
        port: Some(25570),
        java_cmd: Some(Some("/opt/java".to_string())),
        cmd_args: Some(vec!["-Dx".to_string()]),
    };
    let ctx = env.ctx(Flavour::Vanilla, &exclude);
    env.restore(
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
        &ctx,
        &pin,
    );

    assert_eq!(read(root, "server.jar").unwrap(), "jar v1");
    assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
    assert_eq!(read(root, "world/new-file.dat"), None);
    assert_eq!(read(root, "added-later.txt"), None);
    assert_eq!(read(root, "world_nether/DIM-1/r.mca").unwrap(), "nether v1");
    assert_eq!(
        read(root, "plugins/dynmap/config.txt").unwrap(),
        "dynmap config v1"
    );
    // never touched
    assert_eq!(read(root, "world/session.lock").unwrap(), "lock");
    assert_eq!(read(root, "logs/latest.log").unwrap(), "log");
    assert_eq!(
        read(root, "old-worlds/w/level.dat").unwrap(),
        "excluded top level"
    );
    assert_eq!(
        read(root, "plugins/dynmap/web/tiles/0.png").unwrap(),
        "tile"
    );
    assert_eq!(
        fs::read(root.join(".lodestone_config")).unwrap(),
        config_before
    );
    if linked {
        assert!(fs::symlink_metadata(root.join("linked-top"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(fs::symlink_metadata(root.join("world/linked-inside"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(read(&outside, "data.txt").unwrap(), "outside");
    }
    // pinned settings, other fields from the backup
    let config: serde_json::Value =
        serde_json::from_str(&read(root, CONFIG_FILE).unwrap()).unwrap();
    assert_eq!(config["name"], "New name");
    assert_eq!(config["port"], 25570);
    assert_eq!(config["java_cmd"], "/opt/java");
    assert_eq!(config["cmd_args"], serde_json::json!(["-Dx"]));
    assert_eq!(config["version"], "1.20");
    assert_eq!(
        read(root, "server.properties").unwrap(),
        "motd=hi\nserver-port=25570\n"
    );
    // restored modified times
    let restored = fs::metadata(root.join("server.jar"))
        .unwrap()
        .modified()
        .unwrap();
    let entry = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "server.jar")
        .unwrap();
    let restored_ns = restored
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;
    assert_eq!(restored_ns / 1_000_000, entry.modified.unwrap() / 1_000_000);
    // nothing left behind
    assert!(no_restore_folders(root));
    assert!(!env.journal().exists());
}

#[test]
fn a_link_where_the_backup_has_a_folder_is_refused() {
    let env = env();
    write(&env.root, "plugins/a.jar", b"a");
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    fs::remove_dir_all(env.root.join("plugins")).unwrap();
    let outside = env.instances.parent().unwrap().join("shared-plugins");
    fs::create_dir_all(&outside).unwrap();
    if !link_dir(&outside, &env.root.join("plugins")) {
        return;
    }
    let error = plan(
        &env.images,
        &env.ctx(Flavour::Vanilla, &[]),
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
    )
    .unwrap_err();
    assert_eq!(
        error.source.to_string(),
        "plugins is a link; restore can't replace it"
    );
}

/// A Paper server's level is split on disk and one world in the image: a world restore
/// puts back the level and its `_nether` and `_the_end` folders, and nothing else.
#[test]
fn a_paper_world_is_restored_into_its_split_folders() {
    let env = env();
    let root = &env.root;
    write(root, "server.properties", b"level-name=world\n");
    write(root, "server.jar", b"jar v1");
    write(root, "world/level.dat", b"level v1");
    write(root, "world_nether/level.dat", b"nether level v1");
    write(root, "world_nether/DIM-1/r.mca", b"nether v1");
    write(root, "creative/level.dat", b"other world v1");
    let (image, level) = env.backup(paper(), &[]);
    assert_eq!(level.as_deref(), Some("world"));
    assert!(image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "saves/world/DIM-1/r.mca"));

    write(root, "world/level.dat", b"level v2");
    write(root, "world/extra.dat", b"extra");
    write(root, "world/session.lock", b"lock");
    write(root, "world_nether/DIM-1/r.mca", b"nether v2");
    write(root, "world_the_end/DIM1/r.mca", b"end v2");
    write(root, "creative/level.dat", b"other world v2");
    write(root, "server.jar", b"jar v2");

    let ctx = env.ctx(paper(), &[]);
    let source = everything(&image, level.as_deref());
    let plan_ = env.plan(&source, RestoreScope::World, &ctx);
    let owned: Vec<&str> = plan_.owned.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(owned, ["world", "world_nether", "world_the_end"]);
    env.restore(&source, RestoreScope::World, &ctx, &Pin::default());

    assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
    assert_eq!(read(root, "world/extra.dat"), None);
    assert_eq!(read(root, "world/session.lock").unwrap(), "lock");
    assert_eq!(read(root, "world_nether/DIM-1/r.mca").unwrap(), "nether v1");
    assert_eq!(
        read(root, "world_nether/level.dat").unwrap(),
        "nether level v1"
    );
    // the backup had no End: it is removed
    assert!(!root.join("world_the_end").exists());
    // not part of the world
    assert_eq!(read(root, "creative/level.dat").unwrap(), "other world v2");
    assert_eq!(read(root, "server.jar").unwrap(), "jar v2");
    assert!(no_restore_folders(root));
}

/// Restoring everything of a Paper server's backup lays its level out the Paper way again,
/// and leaves alone the files no image can hold (a vanilla `DIM-1` left in the level).
#[test]
fn everything_of_a_paper_server_goes_back_to_its_layout() {
    let env = env();
    let root = &env.root;
    write(root, "world/level.dat", b"level v1");
    write(root, "world_nether/DIM-1/r.mca", b"nether v1");
    write(root, "world_the_end/DIM1/r.mca", b"end v1");
    let (image, level) = env.backup(paper(), &[]);
    fs::remove_dir_all(root.join("world_nether")).unwrap();
    fs::remove_dir_all(root.join("world_the_end")).unwrap();
    write(
        root,
        "world/DIM-1/stale.mca",
        b"a vanilla nether, no image holds it",
    );
    let ctx = env.ctx(paper(), &[]);
    env.restore(
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
        &ctx,
        &Pin::default(),
    );
    assert_eq!(read(root, "world_nether/DIM-1/r.mca").unwrap(), "nether v1");
    assert_eq!(read(root, "world_the_end/DIM1/r.mca").unwrap(), "end v1");
    assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
    assert_eq!(
        read(root, "world/DIM-1/stale.mca").unwrap(),
        "a vanilla nether, no image holds it"
    );
}

#[test]
fn a_world_restore_needs_a_world() {
    let env = env();
    write(
        &env.root,
        "server.properties",
        b"level-name=worlds/survival\n",
    );
    write(&env.root, "worlds/survival/level.dat", b"x");
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    // a nested level isn't a world images can name: only everything can be restored
    assert_eq!(level, None);
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    for scope in [RestoreScope::World, RestoreScope::WorldAndMods] {
        let error = plan(&env.images, &ctx, &everything(&image, None), scope).unwrap_err();
        assert_eq!(error.source.to_string(), "This backup has no world");
    }
    // a level the image doesn't have
    let error = plan(
        &env.images,
        &ctx,
        &everything(&image, Some("nothere")),
        RestoreScope::World,
    )
    .unwrap_err();
    assert_eq!(error.source.to_string(), "This backup has no world");
    env.plan(&everything(&image, None), RestoreScope::Everything, &ctx);
}

/// "World and its mods" puts back the level and the content the backup ran with, and
/// nothing else: other files, and what plugins keep in their folders, stay.
#[test]
fn the_world_and_its_mods_are_restored_together() {
    let env = env();
    let root = &env.root;
    base_server(root);
    write(root, "mods/a.jar", b"mod a v1");
    write(root, "config/a.toml", b"a = 1");
    write(root, "plugins/x.jar", b"plugin v1");
    write(root, "plugins/x/data.db", b"plugin data v1");
    let content = env.content_image(Flavour::Vanilla);
    let (image, level) = env.backup(Flavour::Vanilla, &[]);

    write(root, "mods/a.jar", b"mod a v2");
    write(root, "mods/b.jar", b"mod b");
    write(root, "config/a.toml", b"a = 2");
    write(root, "plugins/x.jar", b"plugin v2");
    write(root, "plugins/x/data.db", b"plugin data v2");
    write(root, "world/level.dat", b"level v2");
    write(root, "notes.txt", b"notes");
    let source = Source {
        image: &image,
        level: level.as_deref(),
        content: Some(&content),
    };
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    let plan_ = env.plan(&source, RestoreScope::WorldAndMods, &ctx);
    assert!(plan_.replaces("mods/b.jar"));
    assert!(!plan_.replaces("plugins/x/data.db"));
    assert!(!plan_.replaces("notes.txt"));
    env.restore(&source, RestoreScope::WorldAndMods, &ctx, &Pin::default());
    assert_eq!(read(root, "mods/a.jar").unwrap(), "mod a v1");
    assert_eq!(read(root, "mods/b.jar"), None);
    assert_eq!(read(root, "config/a.toml").unwrap(), "a = 1");
    assert_eq!(read(root, "plugins/x.jar").unwrap(), "plugin v1");
    assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
    assert_eq!(read(root, "server.jar").unwrap(), "jar v1");
    // not the world, not content
    assert_eq!(read(root, "plugins/x/data.db").unwrap(), "plugin data v2");
    assert_eq!(read(root, "notes.txt").unwrap(), "notes");
    assert!(no_restore_folders(root));

    // without the content image, it is refused
    let error = plan(
        &env.images,
        &ctx,
        &Source {
            content: None,
            ..source
        },
        RestoreScope::WorldAndMods,
    )
    .unwrap_err();
    assert!(error.source.to_string().contains("mods can't be restored"));
}

#[test]
fn diff_lists_changes_by_disk_path() {
    let env = env();
    let root = &env.root;
    base_server(root);
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    write(root, "server.jar", b"jar v2");
    write(root, "mods/new.jar", b"new mod");
    fs::remove_dir_all(root.join("plugins")).unwrap();
    write(root, CONFIG_FILE, b"{}");
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    let source = everything(&image, level.as_deref());
    let plan_ = env.plan(&source, RestoreScope::Everything, &ctx);
    let hashes = hash_current(root, &plan_, &StatCache::new(0), NEVER).unwrap();
    let d = diff(&plan_, &hashes);
    assert_eq!(d.added, vec!["plugins/dynmap/config.txt"]);
    assert_eq!(d.changed, vec!["server.jar"]);
    assert_eq!(d.removed, vec!["mods/new.jar"]);
    assert_eq!(d.created_dirs, vec!["plugins", "plugins/dynmap"]);
    assert_eq!(d.removed_dirs, vec!["mods"]);
    // the same from a safety backup's image
    let (current, _) = env.backup(Flavour::Vanilla, &[]);
    let from_image: HashMap<String, String> = current
        .entries
        .iter()
        .map(|e| (e.path.to_string(), e.sha256.clone()))
        .collect();
    assert_eq!(diff(&plan_, &from_image), d);
    // a file with no hash counts as changed
    assert_eq!(
        diff(&plan_, &HashMap::new()).changed,
        vec![
            "server.jar",
            "server.properties",
            "world/level.dat",
            "world/region/r.0.0.mca",
            "world_nether/DIM-1/r.mca"
        ]
    );

    // a world restore doesn't touch the jar
    let plan_ = env.plan(&source, RestoreScope::World, &ctx);
    let hashes = hash_current(root, &plan_, &StatCache::new(0), NEVER).unwrap();
    assert!(diff(&plan_, &hashes).is_empty());
}

/// What the backup couldn't carry stays as it is on disk: the backup has no copy of it, so
/// a restore neither replaces nor removes it.
#[cfg(unix)]
#[test]
fn files_the_backup_couldnt_carry_are_kept() {
    use std::os::unix::fs::PermissionsExt;

    let env = env();
    let root = &env.root;
    base_server(root);
    write(root, "world/secret.dat", b"secret v1");
    let secret = root.join("world/secret.dat");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).unwrap();
    if File::open(&secret).is_ok() {
        // running as root, which reads every file
        return;
    }
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(image
        .withheld
        .iter()
        .any(|w| w.path == "saves/world/secret.dat"));
    write(root, "world/secret.dat", b"secret v2");
    write(root, "world/level.dat", b"level v2");
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    let source = everything(&image, level.as_deref());
    let plan_ = env.plan(&source, RestoreScope::Everything, &ctx);
    assert!(!plan_.replaces("saves/world/secret.dat"));
    env.restore(&source, RestoreScope::Everything, &ctx, &Pin::default());
    assert_eq!(read(root, "world/secret.dat").unwrap(), "secret v2");
    assert_eq!(read(root, "world/level.dat").unwrap(), "level v1");
}

/// A file in the way of what the restore places, that a backup wouldn't take (so the
/// safety backup has no copy), refuses the restore.
#[test]
fn a_file_in_the_way_is_refused() {
    let env = env();
    let root = &env.root;
    base_server(root);
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    fs::remove_dir_all(root.join("world")).unwrap();
    // a file named like the level: no image can hold it
    write(root, "world", b"in the way");
    let error = plan(
        &env.images,
        &env.ctx(Flavour::Vanilla, &[]),
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
    )
    .unwrap_err();
    assert!(
        error.source.to_string().contains("in the way"),
        "{}",
        error.source
    );
}

#[test]
fn an_image_without_a_level_keeps_every_path() {
    let env = env();
    base_server(&env.root);
    let (image, _) = env.backup(Flavour::Vanilla, &[]);
    let DiskLayout::Server(layout) = image_layout(&image, None, true).unwrap() else {
        panic!("a server layout");
    };
    // the image's own world, and a level no file uses
    assert_ne!(layout.level_name(), "world");
    assert!(layout.worlds().any(|w| w == "world"));
    let files = all_files(&image, &DiskLayout::Server(layout)).unwrap();
    assert!(files.iter().any(|f| f.disk == "world/level.dat"));
    assert!(files.iter().any(|f| f.disk == "world_nether/DIM-1/r.mca"));
}

#[test]
fn pin_config_keeps_unknown_fields() {
    let text = r#"{"name":"Backup","port":1,"java_cmd":"/bad/java","cmd_args":["-evil"],"modpack":{"x":1}}"#;
    let pinned = pin_config_json(
        text,
        &Pin {
            name: Some("Current".to_string()),
            port: Some(25566),
            java_cmd: Some(None),
            cmd_args: Some(vec![]),
        },
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&pinned).unwrap();
    assert_eq!(value["name"], "Current");
    assert_eq!(value["port"], 25566);
    assert!(value["java_cmd"].is_null());
    assert_eq!(value["cmd_args"], serde_json::json!([]));
    assert_eq!(value["modpack"]["x"], 1);
    // nothing pinned: unchanged values
    let same = pin_config_json(text, &Pin::default()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&same).unwrap();
    assert_eq!(value["java_cmd"], "/bad/java");
    assert!(pin_config_json("not json", &Pin::default()).is_err());
}

#[test]
fn pin_server_port_keeps_other_lines() {
    let table: &[(&str, &str)] = &[
        (
            "motd=a\nserver-port=1\npvp=true\n",
            "motd=a\nserver-port=7\npvp=true\n",
        ),
        (
            "motd=a\r\nserver-port = 1\r\n",
            "motd=a\r\nserver-port=7\r\n",
        ),
        (
            "#server-port=1\nmotd=a",
            "#server-port=1\nmotd=a\nserver-port=7\n",
        ),
        ("server-port=1", "server-port=7\n"),
        ("server-portal=1\n", "server-portal=1\nserver-port=7\n"),
        ("", "server-port=7\n"),
    ];
    for (input, expected) in table {
        let out = pin_server_port(input.as_bytes(), 7);
        assert_eq!(String::from_utf8(out).unwrap(), *expected, "{input:?}");
    }
}

/// Sets up a server, backs it up, changes it, and stages the backup.
fn staged_restore(env: &Env) -> (Staged, BTreeMap<String, Option<Vec<u8>>>) {
    let root = &env.root;
    base_server(root);
    let exclude = ["plugins/dynmap/web/tiles/"];
    let (image, level) = env.backup(Flavour::Vanilla, &exclude);
    write(root, "world/level.dat", b"level v2");
    write(root, "world/session.lock", b"lock");
    write(root, "added.txt", b"added");
    write(root, "plugins/dynmap/web/tiles/0.png", b"tile");
    fs::remove_dir_all(root.join("world_nether")).unwrap();
    let before = snapshot(root);
    let plan_ = env.plan(
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
        &env.ctx(Flavour::Vanilla, &exclude),
    );
    (env.stage(&plan_, &Pin::default()), before)
}

fn count_renames(env: &Env) -> usize {
    let (mut staged, _) = staged_restore(env);
    staged.faults.crash_after = Some(usize::MAX);
    let owned_old = staged.owned.iter().filter(|e| e.had_old).count();
    let owned_new = staged.owned.iter().filter(|e| e.had_new).count();
    let carried = staged.carried.len();
    staged.discard();
    // each kept path whose staged copy exists takes two renames; here none has one
    owned_old + owned_new + carried
}

#[test]
fn recovery_after_a_crash_in_each_phase() {
    let total = count_renames(&env());
    assert!(total >= 8, "{total}");
    for crash_after in 0..=total {
        let env = env();
        let (mut staged, before) = staged_restore(&env);
        staged.faults.crash_after = Some(crash_after);
        assert!(matches!(
            staged.swap(&env.journal()),
            Err(SwapError::Broken(_))
        ));
        assert!(env.journal().exists());
        let recovery = env.recover();
        assert_eq!(recovery.repaired, vec![uuid()], "crash after {crash_after}");
        assert!(recovery.broken.is_empty());
        assert!(!env.journal().exists());
        assert_eq!(
            snapshot(&env.root),
            before,
            "crash after {crash_after} renames"
        );
    }
}

#[test]
fn recovery_when_done_keeps_the_restore() {
    let env = env();
    let (staged, _) = staged_restore(&env);
    let staging = staged.staging_dir();
    staged.swap(&env.journal()).unwrap();
    let after = snapshot(&env.root);
    // as if the core died after writing "done" but before cleaning up
    fs::create_dir_all(staging.join("leftover")).unwrap();
    let suffix = staging
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .trim_start_matches(STAGING_PREFIX)
        .to_string();
    write_journal(
        &env.journal(),
        &Journal {
            phase: Phase::Done,
            staging: suffix,
            old: "0123456789abcdef".to_string(),
            entries: vec![JournalEntry {
                path: "world".to_string(),
                had_old: true,
                had_new: true,
            }],
        },
    )
    .unwrap();
    let recovery = env.recover();
    assert_eq!(recovery.repaired, vec![uuid()]);
    assert_eq!(snapshot(&env.root), after);
    assert!(!staging.exists());
}

#[test]
fn a_failed_swap_is_rolled_back() {
    for fail_after in [0, 2, 5, 9] {
        let env = env();
        let (mut staged, before) = staged_restore(&env);
        staged.faults.fail_after = Some(fail_after);
        assert!(matches!(
            staged.swap(&env.journal()),
            Err(SwapError::RolledBack(_))
        ));
        assert!(!env.journal().exists());
        assert_eq!(snapshot(&env.root), before, "fail after {fail_after}");
    }
}

#[test]
fn a_rollback_that_fails_keeps_the_journal() {
    let env = env();
    let (mut staged, _) = staged_restore(&env);
    staged.faults.fail_after = Some(3);
    staged.faults.break_rollback = true;
    match staged.swap(&env.journal()) {
        Err(SwapError::Broken(e)) => assert_eq!(
            e.source.to_string(),
            "The restore could not be undone; restart Lodestone to repair it"
        ),
        other => panic!("{other:?}"),
    }
    assert!(env.journal().exists());
    // the next start repairs it
    assert_eq!(env.recover().repaired, vec![uuid()]);
}

#[test]
fn recovery_refuses_a_tampered_journal() {
    let env = env();
    write(&env.root, "world/level.dat", b"x");
    for journal in [
        Journal {
            phase: Phase::MovingNew,
            staging: "../../stores".to_string(),
            old: "0123456789abcdef".to_string(),
            entries: vec![],
        },
        Journal {
            phase: Phase::MovingNew,
            staging: "0123456789abcdef".to_string(),
            old: "0123456789abcdef".to_string(),
            entries: vec![JournalEntry {
                path: "../../stores/users.json".to_string(),
                had_old: true,
                had_new: false,
            }],
        },
    ] {
        write_journal(&env.journal(), &journal).unwrap();
        assert_eq!(env.recover().broken, vec![uuid()]);
        assert!(env.journal().exists());
    }
    assert_eq!(read(&env.root, "world/level.dat").unwrap(), "x");
}

#[test]
fn recovery_deletes_leftovers() {
    let env = env();
    write(&env.root, "world/level.dat", b"x");
    write(
        &env.root,
        ".lodestone-restore-staging-0123456789abcdef/world/level.dat",
        b"staged",
    );
    write(
        &env.root,
        ".lodestone-restore-old-0123456789abcdef/world/level.dat",
        b"old",
    );
    write(
        &env.instances,
        ".lodestone-restore-new-0123456789abcdef/server.jar",
        b"half",
    );
    let marker = env.store.marker_path(&uuid()).unwrap();
    fs::create_dir_all(marker.parent().unwrap()).unwrap();
    fs::write(&marker, b"").unwrap();
    let recovery = env.recover();
    assert!(recovery.repaired.is_empty() && recovery.broken.is_empty());
    assert!(no_restore_folders(&env.root));
    assert!(!env
        .instances
        .join(".lodestone-restore-new-0123456789abcdef")
        .exists());
    assert!(!marker.exists());
    assert_eq!(read(&env.root, "world/level.dat").unwrap(), "x");
}

#[test]
fn stage_all_and_damaged_backups() {
    let env = env();
    base_server(&env.root);
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    let layout = image_layout(&image, level.as_deref(), false).unwrap();
    let files = all_files(&image, &layout).unwrap();
    let dest = new_server_dir(&env.instances);
    stage_all(&env.images, &files, &dest, &mut |_, _| {}, NEVER).unwrap();
    assert_eq!(read(&dest, "world/level.dat").unwrap(), "level v1");
    assert!(!dest.join(".lodestone_config").exists());
    pin_settings(
        &dest,
        &Pin {
            name: Some("Copy".to_string()),
            port: Some(25566),
            java_cmd: Some(None),
            cmd_args: Some(vec![]),
        },
    )
    .unwrap();
    assert_eq!(
        read(&dest, "server.properties").unwrap(),
        "motd=hi\nserver-port=25566\n"
    );

    // a damaged object fails staging and leaves nothing behind
    let entry = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "saves/world/level.dat")
        .unwrap();
    let object = env.images.object_path(&entry.sha256).unwrap();
    let mut permissions = fs::metadata(&object).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&object, permissions).unwrap();
    fs::write(&object, b"garbage").unwrap();
    let dest = new_server_dir(&env.instances);
    let error = stage_all(&env.images, &files, &dest, &mut |_, _| {}, NEVER).unwrap_err();
    assert_eq!(
        error.source.to_string(),
        "The backup's copy of world/level.dat is missing or damaged"
    );
    assert!(!dest.exists());
    let plan_ = env.plan(
        &everything(&image, level.as_deref()),
        RestoreScope::Everything,
        &env.ctx(Flavour::Vanilla, &[]),
    );
    assert!(stage(
        &env.images,
        &env.root,
        &plan_,
        &Pin::default(),
        &mut |_, _| {},
        NEVER
    )
    .is_err());
    assert!(no_restore_folders(&env.root));
}

#[test]
fn a_journal_that_cant_be_carried_out_keeps_the_restore_folders() {
    let tampered = serde_json::to_vec(&Journal {
        phase: Phase::MovingNew,
        staging: "../../stores".to_string(),
        old: "0123456789abcdef".to_string(),
        entries: vec![],
    })
    .unwrap();
    // damaged
    let unreadable = br#"{"phase":"verifying","staging":"0123456789abcdef"}"#.to_vec();
    for text in [unreadable, tampered] {
        let env = env();
        write(&env.root, "world/region/r.0.0.mca", b"staged");
        write(
            &env.root,
            ".lodestone-restore-old-0123456789abcdef/world/level.dat",
            b"original",
        );
        write(
            &env.root,
            ".lodestone-restore-staging-0123456789abcdef/plugins/a.jar",
            b"staged",
        );
        let journal_path = env.journal();
        fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
        fs::write(&journal_path, &text).unwrap();
        let before = snapshot(&env.root);
        assert_eq!(env.recover().broken, vec![uuid()]);
        assert!(journal_path.exists());
        assert_eq!(snapshot(&env.root), before);
    }
}

/// A server whose folder is a link (or junction) is backed up and restored through it:
/// the link stays, and its target gets the backup's files; a crash is undone through it.
#[test]
fn backup_restore_and_recovery_through_a_linked_server_folder() {
    let env = env();
    let root = &env.root;
    base_server(root);
    let elsewhere = env.instances.parent().unwrap().join("elsewhere");
    fs::rename(root, &elsewhere).unwrap();
    if !link_dir(&elsewhere, root) {
        return;
    }
    let (image, level) = env.backup(Flavour::Vanilla, &[]);
    assert!(image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "saves/world/level.dat"));
    write(root, "world/level.dat", b"level v2");
    write(root, "world/session.lock", b"lock");
    write(root, "added.txt", b"added");
    let before = snapshot(&elsewhere);
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    let source = everything(&image, level.as_deref());
    let mut staged = env.stage(
        &env.plan(&source, RestoreScope::Everything, &ctx),
        &Pin::default(),
    );
    staged.faults.crash_after = Some(4);
    assert!(matches!(
        staged.swap(&env.journal()),
        Err(SwapError::Broken(_))
    ));
    assert_eq!(env.recover().repaired, vec![uuid()]);
    assert!(!env.journal().exists());
    assert_eq!(snapshot(&elsewhere), before);

    env.restore(&source, RestoreScope::Everything, &ctx, &Pin::default());
    assert_eq!(read(&elsewhere, "world/level.dat").unwrap(), "level v1");
    assert_eq!(read(&elsewhere, "added.txt"), None);
    assert!(fs::symlink_metadata(root).unwrap().file_type().is_symlink());
    assert!(no_restore_folders(&elsewhere));
}

#[test]
fn a_journal_of_a_server_not_found_stays_when_a_folder_cant_be_read() {
    let env = env();
    write_journal(
        &env.journal(),
        &Journal {
            phase: Phase::MovingNew,
            staging: "0123456789abcdef".to_string(),
            old: "0123456789abcdef".to_string(),
            entries: vec![],
        },
    )
    .unwrap();
    // the server's settings can't be read, so it may be the journal's server
    fs::write(env.root.join(".lodestone_config"), b"damaged").unwrap();
    let recovery = env.recover();
    assert!(recovery.repaired.is_empty() && recovery.broken.is_empty());
    assert!(env.journal().exists());
    // the server is gone for sure
    fs::remove_dir_all(&env.root).unwrap();
    env.recover();
    assert!(!env.journal().exists());
}

#[test]
fn a_backup_without_the_settings_keeps_the_current_ones() {
    let env = env();
    let root = &env.root;
    base_server(root);
    let (image, level) = env.backup(Flavour::Vanilla, &["*.json"]);
    assert!(!image.entries.iter().any(|e| e.path.as_str() == CONFIG_FILE));
    let settings = fs::read(root.join(CONFIG_FILE)).unwrap();
    write(root, "server.jar", b"jar v2");

    // the exclude is gone by the time of the restore
    let ctx = env.ctx(Flavour::Vanilla, &[]);
    let source = everything(&image, level.as_deref());
    let plan_ = env.plan(&source, RestoreScope::Everything, &ctx);
    assert!(!plan_.replaces(CONFIG_FILE));
    env.restore(&source, RestoreScope::Everything, &ctx, &Pin::default());
    assert_eq!(fs::read(root.join(CONFIG_FILE)).unwrap(), settings);
    assert_eq!(read(root, "server.jar").unwrap(), "jar v1");
}

#[test]
fn a_journal_that_cant_be_finished_undoes_the_swap() {
    let env = env();
    let (mut staged, before) = staged_restore(&env);
    staged.faults.stuck_journal = true;
    assert!(matches!(
        staged.swap(&env.journal()),
        Err(SwapError::RolledBack(_))
    ));
    assert_eq!(snapshot(&env.root), before);
    // the journal stayed, saying the swap is under way; the next start changes nothing
    assert!(env.journal().exists());
    assert_eq!(env.recover().repaired, vec![uuid()]);
    assert!(!env.journal().exists());
    assert_eq!(snapshot(&env.root), before);
}

#[test]
fn a_finished_restore_with_a_stale_journal_is_kept() {
    let env = env();
    let (staged, _) = staged_restore(&env);
    let stale = Journal {
        phase: Phase::MovingNew,
        staging: staged.staging.clone(),
        old: staged.old.clone(),
        entries: staged
            .owned
            .iter()
            .cloned()
            .chain(staged.carried.iter().map(|path| JournalEntry {
                path: path.clone(),
                had_old: true,
                had_new: false,
            }))
            .collect(),
    };
    staged.swap(&env.journal()).unwrap();
    let after = snapshot(&env.root);
    // as if the journal couldn't be marked done or deleted, and the folders were deleted
    write_journal(&env.journal(), &stale).unwrap();
    assert_eq!(env.recover().repaired, vec![uuid()]);
    assert_eq!(snapshot(&env.root), after);
    assert_eq!(read(&env.root, "world/session.lock").unwrap(), "lock");
    assert!(!env.journal().exists());
}

#[test]
fn a_repeated_rollback_changes_nothing() {
    let total = count_renames(&env());
    let env = env();
    let (mut staged, before) = staged_restore(&env);
    staged.faults.crash_after = Some(total);
    assert!(matches!(
        staged.swap(&env.journal()),
        Err(SwapError::Broken(_))
    ));
    let journal: Journal = serde_json::from_slice(&fs::read(env.journal()).unwrap()).unwrap();
    assert_eq!(journal.phase, Phase::MovingNew);
    rollback(&env.root, &journal, Duration::ZERO).unwrap();
    let once = snapshot(&env.root);
    rollback(&env.root, &journal, Duration::ZERO).unwrap();
    assert_eq!(snapshot(&env.root), once);
    assert_eq!(env.recover().repaired, vec![uuid()]);
    assert_eq!(snapshot(&env.root), before);
}

/// An old folder (or an entry in it) that can't be read isn't taken for a missing one:
/// the rollback fails, moving nothing, so the journal stays and the server is protected.
#[test]
fn a_rollback_never_takes_an_unreadable_old_folder_for_a_missing_one() {
    let env = env();
    let (mut staged, _) = staged_restore(&env);
    staged.faults.crash_after = Some(4);
    assert!(matches!(
        staged.swap(&env.journal()),
        Err(SwapError::Broken(_))
    ));
    let journal: Journal = serde_json::from_slice(&fs::read(env.journal()).unwrap()).unwrap();
    let old_dir = env.root.join(format!("{OLD_PREFIX}{}", journal.old));
    assert!(old_dir.is_dir());
    let half_swapped = snapshot(&env.root);
    let denied = || io::Error::from(io::ErrorKind::PermissionDenied);

    let unreadable_folder = |path: &Path| {
        if path == old_dir {
            Err(denied())
        } else {
            present(path)
        }
    };
    let problems =
        rollback_with(&env.root, &journal, Duration::ZERO, &unreadable_folder).unwrap_err();
    assert!(problems[0].contains("couldn't be read"), "{problems:?}");
    assert_eq!(snapshot(&env.root), half_swapped);

    let unreadable_entries = |path: &Path| {
        if path != old_dir && path.starts_with(&old_dir) {
            Err(denied())
        } else {
            present(path)
        }
    };
    assert!(rollback_with(&env.root, &journal, Duration::ZERO, &unreadable_entries).is_err());
    assert_eq!(snapshot(&env.root), half_swapped);

    // once it can be read, the next start undoes the swap
    assert!(env.journal().exists());
    assert_eq!(env.recover().repaired, vec![uuid()]);
    assert!(!env.journal().exists());
}

#[test]
fn under_matches_whole_parts() {
    assert!(under("world", "world"));
    assert!(under("world/level.dat", "world"));
    assert!(!under("world2/level.dat", "world"));
    assert!(!under("worl", "world"));
}
