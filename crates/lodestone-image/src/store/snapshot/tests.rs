use super::*;
use crate::layout::ServerLayout;
use crate::manifest::tests::fabric;
use crate::manifest::Kind;
use crate::store::cdc::tests::region_like;
use crate::store::tests::{age, encodings, store, temp_files, write};
use crate::store::Encoding;
use std::fs::File;

fn paths(snapshot: &Snapshot) -> Vec<&str> {
    snapshot.entries.iter().map(|e| e.path.as_str()).collect()
}

fn withheld(snapshot: &Snapshot) -> Vec<(&str, &str)> {
    snapshot
        .withheld
        .iter()
        .map(|w| (w.path.as_str(), w.reason.as_str()))
        .collect()
}

/// A client instance with a bit of everything.
fn client_instance(root: &Path) {
    write(root, "instance.json", b"{}");
    write(root, "minecraft/mods/sodium.jar", b"sodium");
    write(root, "minecraft/config/sodium.json", b"{}");
    write(root, "minecraft/options.txt", b"fov:90");
    write(root, "minecraft/saves/New World/level.dat", b"level");
    write(
        root,
        "minecraft/saves/New World/region/r.0.0.mca",
        &region_like(1, 1_500_000),
    );
    write(root, "minecraft/saves/New World/session.lock", b"lock");
    write(root, "minecraft/logs/latest.log", b"log");
    write(root, "minecraft/natives/lib.dylib", b"lib");
    write(root, "minecraft/screenshots/a.png", b"png");
}

#[test]
fn a_client_instance_becomes_entries() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let root = dir.path().join("instance");
        client_instance(&root);
        let snapshot = snapshot(
            &store,
            &root,
            &DiskLayout::Client,
            &SnapshotOptions::default(),
        )
        .unwrap();
        assert_eq!(
            paths(&snapshot),
            vec![
                "config/sodium.json",
                "mods/sodium.jar",
                "options.txt",
                "saves/New World/level.dat",
                "saves/New World/region/r.0.0.mca",
                "screenshots/a.png",
            ]
        );
        assert!(snapshot.withheld.is_empty(), "{:?}", snapshot.withheld);
        assert_eq!(snapshot.worlds, BTreeSet::from(["New World".to_string()]));
        assert_eq!(snapshot.files_read, 6);
        let region = &snapshot.entries[4];
        assert_eq!(region.world.as_deref(), Some("New World"));
        assert!(region.modified.is_some());
        assert_eq!(region.source, Source::Object {});
        for entry in &snapshot.entries {
            assert!(
                store.has_object(&entry.sha256, entry.size),
                "{}",
                entry.path
            );
        }
        // It makes a valid image.
        let mut image = Image::new(Kind::Full, fabric());
        snapshot.fill(&mut image);
        image.seal();
        image.validate().unwrap();
        store.save_manifest("client", &image).unwrap();
        assert!(temp_files(store.root()).is_empty());
    }
}

#[test]
fn a_split_server_maps_its_worlds() {
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("server");
    write(&root, "server.properties", b"level-name=world");
    write(&root, "eula.txt", b"eula=true");
    write(&root, "plugins/x.jar", b"plugin");
    write(&root, "world/level.dat", b"overworld");
    write(&root, "world/region/r.0.0.mca", b"region");
    write(&root, "world_nether/level.dat", b"nether level");
    write(&root, "world_nether/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"end");
    write(&root, "world/session.lock", b"lock");
    write(&root, "logs/latest.log", b"log");
    write(&root, ".lodestone_config", b"{}");
    // Can't be told apart from a world in canonical form: withheld.
    write(&root, "saves/oops.txt", b"x");
    // A vanilla dimension left in a split level would come back in the split folder.
    write(&root, "world/DIM-1/region/r.0.0.mca", b"stale");
    let layout = DiskLayout::Server(ServerLayout::new("world", [], true).unwrap());
    let snapshot = snapshot(&store, &root, &layout, &SnapshotOptions::default()).unwrap();
    assert_eq!(
        paths(&snapshot),
        vec![
            "eula.txt",
            "plugins/x.jar",
            "saves/world/.bukkit_nether/level.dat",
            "saves/world/DIM-1/region/r.0.0.mca",
            "saves/world/DIM1/region/r.0.0.mca",
            "saves/world/level.dat",
            "saves/world/region/r.0.0.mca",
            "server.properties",
        ]
    );
    assert_eq!(
        withheld(&snapshot),
        vec![
            ("saves/oops.txt", "bad_name"),
            ("world/DIM-1/region/r.0.0.mca", "bad_name"),
        ]
    );
    // And back: every entry maps to the disk path it came from.
    for entry in &snapshot.entries {
        let disk = layout.to_disk(&entry.path).unwrap();
        assert!(join_disk_exists(&root, &disk), "{disk}");
    }
}

fn join_disk_exists(root: &Path, disk: &str) -> bool {
    crate::layout::join_disk(root, disk).is_file()
}

#[test]
fn excludes_leave_files_out_quietly() {
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("server");
    write(&root, "dynmap/web/tiles/a/b.png", b"tile");
    write(&root, "dynmap/config.txt", b"config");
    write(&root, "world/level.dat", b"level");
    write(&root, "world/DIM1/region/r.mca", b"end");
    write(&root, "world/old.bak", b"backup");
    let layout = DiskLayout::Server(ServerLayout::new("world", [], false).unwrap());
    let excludes = vec![
        "dynmap/web/tiles/".to_string(),
        "*.bak".to_string(),
        "saves/*/DIM1/".to_string(),
    ];
    let options = SnapshotOptions {
        excludes: &excludes,
        ..SnapshotOptions::default()
    };
    let snapshot = snapshot(&store, &root, &layout, &options).unwrap();
    assert_eq!(
        paths(&snapshot),
        vec!["dynmap/config.txt", "saves/world/level.dat"]
    );
    assert!(snapshot.withheld.is_empty());
}

#[cfg(unix)]
#[test]
fn links_bad_names_and_unreadable_files_are_withheld() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("instance");
    write(&root, "minecraft/mods/ok.jar", b"ok");
    write(&root, "minecraft/mods/target.jar", b"target");
    std::os::unix::fs::symlink(
        root.join("minecraft/mods/target.jar"),
        root.join("minecraft/mods/link.jar"),
    )
    .unwrap();
    std::os::unix::fs::symlink(dir.path(), root.join("minecraft/linked-folder")).unwrap();
    write(&root, "minecraft/config/a:b.toml", b"colon");
    write(&root, "minecraft/config/trailing.", b"dot");
    write(&root, "minecraft/bad:folder/inner/x.txt", b"x");
    write(&root, "minecraft/config/locked.toml", b"locked");
    // Outside the game folder, nothing is reported.
    write(&root, "odd:name", b"x");
    std::os::unix::fs::symlink(dir.path(), root.join("outside-link")).unwrap();
    let locked = root.join("minecraft/config/locked.toml");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let readable_anyway = File::open(&locked).is_ok(); // running as root
    let fifo = root.join("minecraft/config/pipe");
    let has_fifo = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|s| s.success());

    let snapshot = snapshot(
        &store,
        &root,
        &DiskLayout::Client,
        &SnapshotOptions::default(),
    )
    .unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    let mut expected = vec![
        ("bad:folder", "bad_name"),
        ("config/a:b.toml", "bad_name"),
        ("config/trailing.", "bad_name"),
        ("linked-folder", "symlink"),
        ("mods/link.jar", "symlink"),
    ];
    if !readable_anyway {
        expected.push(("config/locked.toml", "unreadable"));
    }
    if has_fifo {
        expected.push(("config/pipe", "unreadable"));
    }
    expected.sort();
    assert_eq!(withheld(&snapshot), expected);
    let mut want = vec!["mods/ok.jar", "mods/target.jar"];
    if readable_anyway {
        want.insert(0, "config/locked.toml");
    }
    assert_eq!(paths(&snapshot), want);
}

#[test]
fn the_stat_cache_skips_unchanged_files() {
    for encoding in encodings() {
        let (dir, store) = store(encoding);
        let root = dir.path().join("instance");
        client_instance(&root);
        for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
            if entry.file_type().is_file() {
                age(entry.path(), 3600);
            }
        }
        let first = snapshot(
            &store,
            &root,
            &DiskLayout::Client,
            &SnapshotOptions::default(),
        )
        .unwrap();
        assert_eq!(first.files_read, 6);
        store.save_stat_cache("i", &first.cache).unwrap();

        let cache = store.load_stat_cache("i");
        let options = SnapshotOptions {
            cache: Some(&cache),
            ..SnapshotOptions::default()
        };
        let second = snapshot(&store, &root, &DiskLayout::Client, &options).unwrap();
        assert_eq!(second.files_read, 0, "an unchanged instance reads nothing");
        assert_eq!(second.entries, first.entries);

        // A changed file is read again, the same size or not.
        write(&root, "minecraft/options.txt", b"fov:70");
        let plan = plan(&store, &root, &DiskLayout::Client, &options).unwrap();
        assert_eq!(plan.to_read.len(), 1);
        assert_eq!(plan.to_read[0].path.as_str(), "options.txt");
        assert_eq!(plan.bytes_to_read(), 6);
        let third = run(&store, plan, &options, &|_, _| {}).unwrap();
        assert_eq!(third.files_read, 1);
        let options_entry = third
            .entries
            .iter()
            .find(|e| e.path.as_str() == "options.txt")
            .unwrap();
        assert_eq!(options_entry.sha256, crate::hex::sha256_hex(b"fov:70"));

        // A file changed within 2 s of the scan that cached it isn't trusted, even though
        // its size and time match.
        let racy_cache = third.cache.clone();
        let options = SnapshotOptions {
            cache: Some(&racy_cache),
            ..SnapshotOptions::default()
        };
        let fourth = plan_paths(&store, &root, &options);
        assert_eq!(fourth, vec!["options.txt"]);

        // A missing object means the cached hash is no good.
        let sodium = first
            .entries
            .iter()
            .find(|e| e.path.as_str() == "mods/sodium.jar")
            .unwrap();
        let object = store.object_path(&sodium.sha256).unwrap();
        super::fsx::remove_file_forced(&object).unwrap();
        let options = SnapshotOptions {
            cache: Some(&cache),
            ..SnapshotOptions::default()
        };
        assert!(plan_paths(&store, &root, &options).contains(&"mods/sodium.jar".to_string()));
    }
}

fn plan_paths(store: &Store, root: &Path, options: &SnapshotOptions) -> Vec<String> {
    plan(store, root, &DiskLayout::Client, options)
        .unwrap()
        .to_read
        .iter()
        .map(|f| f.path.to_string())
        .collect()
}

#[test]
fn threads_give_the_same_snapshot() {
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("instance");
    for i in 0..60 {
        write(
            &root,
            &format!("minecraft/saves/w/region/r.{i}.0.mca"),
            &region_like(i, 20_000 + i as usize * 1000),
        );
    }
    write(
        &root,
        "minecraft/saves/w/big.mca",
        &region_like(99, 2_200_000),
    );
    let one = snapshot(
        &store,
        &root,
        &DiskLayout::Client,
        &SnapshotOptions {
            threads: 1,
            ..SnapshotOptions::default()
        },
    )
    .unwrap();
    let progress = std::sync::Mutex::new(Vec::new());
    let options = SnapshotOptions {
        threads: 8,
        ..SnapshotOptions::default()
    };
    let plan = plan(&store, &root, &DiskLayout::Client, &options).unwrap();
    let total = plan.bytes_to_read();
    let many = run(&store, plan, &options, &|done, all| {
        progress.lock().unwrap().push((done, all));
    })
    .unwrap();
    assert_eq!(one.entries, many.entries);
    let progress = progress.into_inner().unwrap();
    assert_eq!(progress.len(), 61);
    assert!(progress.iter().all(|&(_, all)| all == total));
    assert_eq!(progress.iter().map(|&(done, _)| done).max(), Some(total));
}

#[test]
fn a_file_that_keeps_changing_is_withheld() {
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("instance");
    write(&root, "minecraft/mods/steady.jar", b"steady");
    let busy = root.join("minecraft/saves/w/busy.dat");
    write(&root, "minecraft/saves/w/busy.dat", &vec![0u8; 3_000_000]);
    let stop = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut file = File::options().append(true).open(&busy).unwrap();
            while !stop.load(Ordering::Relaxed) {
                io::Write::write_all(&mut file, &[1u8; 512]).unwrap();
            }
        });
        // Start once the writer has.
        while fs::metadata(&busy).unwrap().len() == 3_000_000 {
            std::thread::yield_now();
        }
        let result = snapshot(
            &store,
            &root,
            &DiskLayout::Client,
            &SnapshotOptions {
                tries: 2,
                ..SnapshotOptions::default()
            },
        );
        stop.store(true, Ordering::Relaxed);
        result
    })
    .unwrap();
    assert_eq!(paths(&result), vec!["mods/steady.jar"]);
    assert_eq!(
        withheld(&result),
        vec![("saves/w/busy.dat", "kept_changing")]
    );
    // Its world isn't listed: no entry is in it.
    assert!(result.worlds.is_empty());
}

#[test]
fn cancelling_stops_the_snapshot() {
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("instance");
    write(&root, "minecraft/mods/a.jar", b"a");
    let options = SnapshotOptions {
        cancel: &|| true,
        ..SnapshotOptions::default()
    };
    assert!(matches!(
        snapshot(&store, &root, &DiskLayout::Client, &options),
        Err(SnapshotError::Cancelled)
    ));
    assert!(matches!(
        snapshot(
            &store,
            &dir.path().join("missing"),
            &DiskLayout::Client,
            &SnapshotOptions::default()
        ),
        Err(SnapshotError::RootMissing(_))
    ));
}

#[cfg(unix)]
#[test]
fn modes_are_recorded() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, store) = store(Encoding::Raw);
    let root = dir.path().join("server");
    write(&root, "start.sh", b"#!/bin/sh");
    fs::set_permissions(root.join("start.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    write(&root, "server.properties", b"x");
    fs::set_permissions(
        root.join("server.properties"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let layout = DiskLayout::Server(ServerLayout::new("world", [], false).unwrap());
    let snapshot = snapshot(&store, &root, &layout, &SnapshotOptions::default()).unwrap();
    let modes: Vec<(&str, u32)> = snapshot
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.mode))
        .collect();
    assert_eq!(
        modes,
        vec![("server.properties", 0o600), ("start.sh", 0o755)]
    );
}
