//! Taking backups as data images, on temporary server folders and stores.

use std::path::Path;
use std::time::{Duration, SystemTime};

use lodestone_image::store::{Encoding, Store as ImageStore, NEVER};
use lodestone_image::{Kind, Loader, WithheldReason};

use super::*;
use crate::implementations::minecraft::Flavour;

pub(crate) fn uuid() -> InstanceUuid {
    InstanceUuid::from("INSTANCE_take".to_string())
}

pub(crate) fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = lodestone_image::layout::join_disk(root, relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Makes a file look written a while ago, so the stat cache trusts its hash.
pub(crate) fn age(root: &Path, relative: &str) {
    let file = std::fs::File::options()
        .write(true)
        .open(lodestone_image::layout::join_disk(root, relative))
        .unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
}

pub(crate) fn paper() -> Flavour {
    Flavour::Paper {
        build_version: None,
    }
}

pub(crate) fn request(root: &Path, flavour: Flavour, policy_exclude: &[&str]) -> Request {
    Request {
        uuid: uuid(),
        root: root.to_path_buf(),
        game_version: "1.21.1".to_string(),
        flavour,
        level_name: "world".to_string(),
        policy_exclude: policy_exclude.iter().map(|s| s.to_string()).collect(),
        full_scan: false,
        threads: 1,
    }
}

pub(crate) fn meta(content: Option<&str>) -> Meta {
    Meta {
        label: "Backup".to_string(),
        created: 1_800_000_000,
        quiesced: true,
        content: content.map(str::to_string),
        parent: None,
    }
}

/// Takes and builds a backup of `request`, saving its image and stat cache.
pub(crate) fn take_image(store: &ImageStore, request: Request) -> (Prepared, Taken, Image) {
    let prepared = Prepared::new(store, request).unwrap();
    let taken = prepared.take(store, &|_| {}, &|_, _| {}, NEVER).unwrap();
    let image = build(&prepared, &taken, meta(None)).unwrap();
    store.save_manifest(uuid().as_ref(), &image).unwrap();
    store
        .save_stat_cache(uuid().as_ref(), &taken.snapshot.cache)
        .unwrap();
    (prepared, taken, image)
}

fn paths(image: &Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

fn setup() -> (tempfile::TempDir, ImageStore, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let store = ImageStore::new(dir.path().join("backups"), Encoding::Zstd);
    let root = dir.path().join("server");
    std::fs::create_dir_all(&root).unwrap();
    (dir, store, root)
}

fn vanilla_server(root: &Path) {
    write(root, "server.properties", b"level-name=world\n");
    write(root, "server.jar", b"jar");
    write(root, ".lodestone_config", b"{}");
    write(root, ".lodestone_minecraft_config.json", b"{}");
    write(root, "world/level.dat", b"level");
    write(root, "world/region/r.0.0.mca", b"region");
    write(root, "world/session.lock", b"lock");
    write(root, "creative/level.dat", b"creative");
    write(root, "logs/latest.log", b"log");
    write(root, "plugins/dynmap/web/tiles/0.png", b"tile");
}

#[test]
fn a_backup_is_a_data_image_of_the_server() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    let prepared = Prepared::new(&store, request(&root, Flavour::Vanilla, &[])).unwrap();
    let taken = prepared.take(&store, &|_| {}, &|_, _| {}, NEVER).unwrap();
    let content = "cc".repeat(32);
    let image = build(&prepared, &taken, meta(Some(&content))).unwrap();
    assert_eq!(image.kind, Kind::Data);
    assert_eq!(image.content.as_deref(), Some(content.as_str()));
    assert_eq!(image.game.loader, Loader::Vanilla);
    assert_eq!(image.label.as_deref(), Some("Backup"));
    image.validate().unwrap();
    // worlds are canonical; what images never record is left out
    assert_eq!(
        paths(&image),
        [
            ".lodestone_minecraft_config.json",
            "plugins/dynmap/web/tiles/0.png",
            "saves/creative/level.dat",
            "saves/world/level.dat",
            "saves/world/region/r.0.0.mca",
            "server.jar",
            "server.properties",
        ]
    );
    let worlds: Vec<(&str, Option<u64>, Option<bool>)> = image
        .worlds
        .iter()
        .map(|w| (w.name.as_str(), w.size, w.quiesced))
        .collect();
    assert_eq!(
        worlds,
        [
            ("creative", Some(8), Some(true)),
            ("world", Some(11), Some(true))
        ]
    );
    assert_eq!(prepared.level_in(&image), Some("world".to_string()));
    assert!(image.withheld.is_empty(), "{:?}", image.withheld);
    // the image names only what the store has
    store.save_manifest(uuid().as_ref(), &image).unwrap();
}

#[test]
fn the_same_server_is_the_same_image() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    let (_, _, first) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    let prepared = Prepared::new(&store, request(&root, Flavour::Vanilla, &[])).unwrap();
    let taken = prepared.take(&store, &|_| {}, &|_, _| {}, NEVER).unwrap();
    let again = build(
        &prepared,
        &taken,
        Meta {
            label: "Another name".to_string(),
            created: 1_900_000_000,
            parent: Some(first.id.clone()),
            ..meta(None)
        },
    )
    .unwrap();
    assert_eq!(again.id, first.id);
    // never its own parent
    assert_eq!(again.parent, None);
    // a quiesced copy isn't the same image as one that may not be
    let not_quiesced = build(
        &prepared,
        &taken,
        Meta {
            quiesced: false,
            ..meta(None)
        },
    )
    .unwrap();
    assert_ne!(not_quiesced.id, first.id);
    // nor one linked to other content
    let linked = build(&prepared, &taken, meta(Some(&"dd".repeat(32)))).unwrap();
    assert_ne!(linked.id, first.id);
}

#[test]
fn a_paper_level_is_one_canonical_world() {
    let (_dir, store, root) = setup();
    write(&root, "world/level.dat", b"level");
    write(&root, "world_nether/level.dat", b"nether level");
    write(&root, "world_nether/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"end");
    write(&root, "world_the_end/uid.dat", b"uid");
    // a vanilla Nether left inside a split level reads back elsewhere: withheld
    write(&root, "world/DIM-1/region/r.0.0.mca", b"stale");
    let (prepared, _, image) = take_image(&store, request(&root, paper(), &[]));
    assert_eq!(
        paths(&image),
        [
            "saves/world/.bukkit_nether/level.dat",
            "saves/world/.bukkit_the_end/uid.dat",
            "saves/world/DIM-1/region/r.0.0.mca",
            "saves/world/DIM1/region/r.0.0.mca",
            "saves/world/level.dat",
        ]
    );
    assert_eq!(image.worlds.len(), 1);
    assert_eq!(prepared.level_in(&image), Some("world".to_string()));
    assert!(image
        .withheld
        .iter()
        .any(|w| w.reason == WithheldReason::BadName && w.path.starts_with("world/DIM-1")));
    assert_eq!(
        image
            .settings
            .server
            .as_ref()
            .and_then(|s| s.flavour.as_ref())
            .map(|f| f.as_str().to_string()),
        Some("paper".to_string())
    );
}

#[test]
fn policy_excludes_become_canonical_excludes() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    write(&root, "world/cache/x.bin", b"cache");
    let (_, _, image) = take_image(
        &store,
        request(
            &root,
            Flavour::Vanilla,
            &["plugins/dynmap/web/tiles/", "world/cache/"],
        ),
    );
    assert_eq!(
        image.excludes,
        ["plugins/dynmap/web/tiles/", "saves/world/cache/"]
    );
    let listed = paths(&image);
    assert!(!listed.iter().any(|p| p.contains("tiles")), "{listed:?}");
    assert!(!listed.iter().any(|p| p.contains("cache")), "{listed:?}");
}

#[test]
fn only_what_changed_is_read_unless_it_is_a_full_scan() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    for path in ["server.jar", "world/level.dat", "world/region/r.0.0.mca"] {
        age(&root, path);
    }
    let (_, first, _) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    assert_eq!(first.snapshot.files_read, 7);
    write(&root, "world/level.dat", b"level 2");
    age(&root, "world/level.dat");
    let (_, second, image) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    // the files written just now aren't trusted from the cache; the aged ones are
    assert!(second.snapshot.files_read < first.snapshot.files_read);
    assert!(image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "saves/world/level.dat" && e.size == 7));
    let full = Request {
        full_scan: true,
        ..request(&root, Flavour::Vanilla, &[])
    };
    let (_, third, _) = take_image(&store, full);
    assert_eq!(third.snapshot.files_read, 7);
}

#[test]
fn a_prefetch_leaves_only_what_changed_since_to_read() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    for path in ["server.jar", "world/level.dat", "world/region/r.0.0.mca"] {
        age(&root, path);
    }
    let mut prepared = Prepared::new(&store, request(&root, Flavour::Vanilla, &[])).unwrap();
    // nothing to read ahead at the real threshold
    assert!(!prepared.prefetch(&store, &|_, _| {}, NEVER).unwrap());
    prepared.prefetch_min = 0;
    assert!(prepared.prefetch(&store, &|_, _| {}, NEVER).unwrap());
    // the world changes between the read-ahead and the final pass
    write(&root, "world/region/r.0.0.mca", b"region 2");
    age(&root, "world/region/r.0.0.mca");
    let taken = prepared.take(&store, &|_| {}, &|_, _| {}, NEVER).unwrap();
    assert!(
        taken.snapshot.files_read < 7,
        "{}",
        taken.snapshot.files_read
    );
    let image = build(&prepared, &taken, meta(None)).unwrap();
    let region = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "saves/world/region/r.0.0.mca")
        .unwrap();
    assert_eq!(region.size, 8);
    store.save_manifest(uuid().as_ref(), &image).unwrap();
}

#[cfg(unix)]
#[test]
fn files_it_cant_carry_are_withheld_with_why() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, store, root) = setup();
    vanilla_server(&root);
    write(&root, "world/secret.dat", b"secret");
    let secret = root.join("world/secret.dat");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    let outside = root.parent().unwrap().join("outside");
    write(&outside, "x", b"x");
    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
    std::fs::write(root.join("bad\\name.txt"), b"x").unwrap();
    let (_, _, image) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
    let withheld: Vec<(&str, WithheldReason)> = image
        .withheld
        .iter()
        .map(|w| (w.path.as_str(), w.reason.clone()))
        .collect();
    assert!(
        withheld.contains(&("linked", WithheldReason::Symlink)),
        "{withheld:?}"
    );
    assert!(
        withheld.contains(&("bad\\name.txt", WithheldReason::BadName)),
        "{withheld:?}"
    );
    // withheld as unreadable, or (running as root, which reads every file) read
    let secret_withheld =
        withheld.contains(&("saves/world/secret.dat", WithheldReason::Unreadable));
    let secret_read = image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "saves/world/secret.dat");
    assert!(secret_withheld != secret_read, "{withheld:?}");
}

#[test]
fn full_scans_come_first_and_every_tenth_scheduled_backup() {
    assert!(needs_full_scan(false, true, 0));
    assert!(needs_full_scan(true, true, 0));
    assert!(!needs_full_scan(true, false, 0));
    assert!(!needs_full_scan(true, false, 8));
    assert!(needs_full_scan(true, false, 9));
    assert!(!needs_full_scan(false, false, 9));
}

#[test]
fn new_bytes_are_what_the_previous_backup_didnt_have() {
    let (_dir, store, root) = setup();
    vanilla_server(&root);
    let (_, _, first) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    let total: u64 = first.entries.iter().map(|e| e.size).sum();
    assert_eq!(new_bytes(&first, None), total);
    assert_eq!(new_bytes(&first, Some(&first)), 0);
    write(&root, "world/level.dat", b"level 22");
    let (_, _, second) = take_image(&store, request(&root, Flavour::Vanilla, &[]));
    assert_eq!(new_bytes(&second, Some(&first)), 8);
}
