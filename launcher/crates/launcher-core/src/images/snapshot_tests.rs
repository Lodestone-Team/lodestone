//! Tests of snapshots in the image store: taking them, their records, retention and what
//! they keep alive, on real instance folders in temporary data folders.

use std::sync::Arc;

use lodestone_image::Kind;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::store::materialize::{Selection, missing_objects};

use super::snapshots::{RECORDS_FILE, RiskyChange, SnapshotKind, Take};
use super::tests::{Setup, age_objects};
use super::*;
use crate::instances::SnapshotSettings;

impl Setup {
    /// A manual snapshot named `name`.
    pub(super) fn snapshot(&self, name: &str) -> SnapshotRecord {
        self.images
            .take_snapshot(self.id(), Take::new(SnapshotKind::Manual, name))
            .unwrap()
            .expect("a manual snapshot always records")
    }

    pub(super) fn records(&self) -> Vec<SnapshotRecord> {
        self.images.snapshots(self.id()).unwrap().records
    }

    fn records_path(&self) -> std::path::PathBuf {
        self.images
            .store()
            .instance_dir(self.id())
            .unwrap()
            .join(RECORDS_FILE)
    }

    /// A world with a level.dat and a region file.
    pub(super) fn world(&self, name: &str, region: &[u8]) {
        self.write(&format!("saves/{name}/level.dat"), b"not nbt, but a file");
        self.write(&format!("saves/{name}/region/r.0.0.mca"), region);
    }

    /// Every object a manifest of the instance names is in the store, whole.
    pub(super) fn nothing_missing(&self) {
        let store = self.images.store();
        for id in store.manifest_ids(self.id()).unwrap() {
            let image = store.load_manifest(self.id(), &id).unwrap();
            let missing = missing_objects(store, &image, Selection::All);
            assert!(missing.is_empty(), "{id} lacks {:?}", missing.first());
        }
    }
}

#[test]
fn a_snapshot_is_a_data_image_linked_to_the_current_content() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a mod");
    s.write("config/a.toml", b"a config");
    s.write("options.txt", b"fov:90");
    s.world("World", b"region bytes");
    s.write("screenshots/a.png", b"png");
    let created = s.images.record_now(s.id(), "Created Pack").unwrap().unwrap();
    let record = s.snapshot("Before 1.21");
    assert_eq!(record.kind, SnapshotKind::Manual);
    assert_eq!(record.name, "Before 1.21");
    assert!(!record.full);
    assert_eq!(record.worlds, ["World"]);
    assert_eq!(record.content.as_deref(), Some(created.id.as_str()));
    let image = s.image(&record.image);
    assert_eq!(image.kind, Kind::Data);
    assert_eq!(image.content.as_deref(), Some(created.id.as_str()));
    assert_eq!(image.label.as_deref(), Some("Before 1.21"));
    assert_eq!(
        s.paths(&record.image),
        [
            "options.txt",
            "saves/World/level.dat",
            "saves/World/region/r.0.0.mca",
            "screenshots/a.png"
        ]
    );
    let world = &image.worlds[0];
    assert_eq!(world.name, "World");
    assert_eq!(world.quiesced, Some(true));
    assert_eq!(
        world.size,
        Some((b"not nbt, but a file".len() + b"region bytes".len()) as u64)
    );
    assert_eq!(record.files, 4);
    assert!(record.new_bytes > 0);
    // Listed, newest first, with nothing unreadable.
    let listed = s.images.snapshots(s.id()).unwrap();
    assert_eq!(listed.records, [record.clone()]);
    assert!(listed.unreadable.is_empty());
    // The snapshot records a change made by hand first, so the link is what is on disk.
    s.write("mods/b.jar", b"dropped in by hand");
    let second = s.snapshot("Later");
    let current = s.state().current.unwrap();
    assert_ne!(current, created.id);
    assert_eq!(second.content.as_deref(), Some(current.as_str()));
    assert_eq!(s.labels()[0], labels::OUTSIDE_LABEL);
    // The same data with another content link is another image.
    assert_ne!(second.image, record.image);
    assert_eq!(s.records().len(), 2);
}

#[test]
fn everything_is_a_full_image_and_so_is_one_without_content() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a mod");
    s.world("World", b"r");
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let full = s
        .images
        .take_snapshot(
            s.id(),
            Take {
                full: true,
                ..Take::new(SnapshotKind::Manual, "All of it")
            },
        )
        .unwrap()
        .unwrap();
    assert!(full.full);
    assert_eq!(full.content, None);
    let image = s.image(&full.image);
    assert_eq!(image.kind, Kind::Full);
    assert_eq!(image.base, Some(lodestone_image::Base::Loader {}));
    assert!(s.paths(&full.image).contains(&"mods/a.jar".to_string()));

    // A "Find it for me" search has the mods renamed on purpose, so the current image
    // isn't what is on disk: the snapshot holds everything instead of linking it.
    let finder = Setup::new();
    finder.write("mods/a.jar", b"a mod");
    finder.images.record_now(finder.id(), "Created").unwrap();
    std::fs::write(
        finder
            .store
            .dir(finder.id())
            .join(crate::content::mod_finder::FILE_NAME),
        b"{}",
    )
    .unwrap();
    assert!(finder.images.finder_running(finder.id()));
    let record = finder.snapshot("During a search");
    assert!(record.full);
    assert_eq!(finder.image(&record.image).kind, Kind::Full);
}

#[test]
fn an_unchanged_automatic_snapshot_writes_nothing() {
    let s = Setup::new();
    s.world("World", b"r");
    let first = s
        .images
        .before_risky_change(
            s.id(),
            RiskyChange::ContentUpdate {
                titles: &["Sodium".to_string()],
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(first.kind, SnapshotKind::BeforeChange);
    assert_eq!(first.name, "Before updating Sodium");
    // The instance had no image: the snapshot recorded its first.
    assert!(first.content.is_some());
    let again = s
        .images
        .before_risky_change(
            s.id(),
            RiskyChange::LoaderChange {
                loader: crate::instances::Loader::Fabric,
                version: "0.17.0",
            },
        )
        .unwrap();
    assert_eq!(again, None);
    assert_eq!(s.records().len(), 1);
    // A manual one records all the same, sharing the image.
    let manual = s.snapshot("Mine");
    assert_eq!(manual.image, first.image);
    assert_eq!(s.records().len(), 2);
}

#[test]
fn a_snapshot_before_a_pack_update_holds_what_the_game_wrote() {
    let s = Setup::new();
    s.write("config/a.toml", b"as installed");
    s.images.record_now(s.id(), "Installed FO 6.3").unwrap();
    // The game rewrites a config while it runs; its session's end takes the baseline
    // again without an image.
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Unchanged
    ));
    s.write("config/a.toml", b"rewritten by the game");
    s.images.end_session(s.id());
    let before = s.state().current.unwrap();
    let record = s
        .images
        .before_risky_change(
            s.id(),
            RiskyChange::ModpackUpdate {
                title: "FO",
                version: "6.4",
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(record.name, "Before updating FO to 6.4");
    let linked = record.content.unwrap();
    assert_ne!(linked, before, "the game's writes are a content image now");
    assert_eq!(s.labels()[0], labels::BEFORE_MODPACK_UPDATE);
    // So is a server's update.
    s.write("config/a.toml", b"rewritten again");
    let server = s
        .images
        .before_risky_change(s.id(), RiskyChange::ServerUpdate { server: "SMP" })
        .unwrap()
        .unwrap();
    assert_eq!(server.name, "Before updating from SMP");
    assert_eq!(s.labels()[0], labels::OUTSIDE_LABEL);
}

#[test]
fn snapshots_are_refused_while_the_game_runs() {
    let s = Setup::new();
    s.world("World", b"r");
    s.images.record_now(s.id(), "Created").unwrap();
    let _ = s.images.before_launch(s.id());
    assert!(matches!(
        s.images
            .take_snapshot(s.id(), Take::new(SnapshotKind::Manual, "x")),
        Err(ImageError::Running)
    ));
    s.images.end_session(s.id());
    s.snapshot("Now");
}

#[test]
fn after_play_snapshots_only_when_asked() {
    let s = Setup::new();
    s.world("World", b"r");
    s.images.record_now(s.id(), "Created").unwrap();
    let _ = s.images.before_launch(s.id());
    s.write("saves/World/region/r.0.0.mca", b"played");
    s.images.end_session(s.id());
    assert!(s.records().is_empty(), "off by default");

    let _ = s.images.before_launch(s.id());
    s.write("saves/World/region/r.0.0.mca", b"played again");
    s.images.end_session(s.id());
    s.images.after_play(s.id());
    let records = s.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, SnapshotKind::AfterPlay);
    assert_eq!(records[0].name, labels::AFTER_PLAYING);

    // Through the session's end on its own thread.
    let _ = s.images.before_launch(s.id());
    s.write("saves/World/region/r.0.0.mca", b"and again");
    s.images.session_ended_with(s.id(), true);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while s.records().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(s.records().len(), 2);
}

#[test]
fn records_follow_the_manifests() {
    let s = Setup::new();
    s.world("World", b"one");
    s.images.record_now(s.id(), "Created").unwrap();
    let first = s.snapshot("First");
    s.write("saves/World/region/r.0.0.mca", b"two");
    let second = s.snapshot("Second");

    // A crash between a manifest and its record: the image comes back as a manual one,
    // named by its label.
    let records_file = s.records_path();
    std::fs::remove_file(&records_file).unwrap();
    let listed = s.records();
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|r| r.kind == SnapshotKind::Manual));
    let names: Vec<&str> = listed.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"First") && names.contains(&"Second"));
    assert!(listed.iter().any(|r| r.image == first.image && r.files == 2));

    // A record whose image is gone is dropped.
    s.images
        .store()
        .delete_manifest(s.id(), &second.image)
        .unwrap();
    let listed = s.records();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].image, first.image);

    // A damaged list is set aside and rebuilt: nothing is lost, every snapshot is manual.
    std::fs::write(&records_file, b"{not json").unwrap();
    let listed = s.records();
    assert_eq!(listed.len(), 1);
    let folder = records_file.parent().unwrap();
    let aside: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("snapshots.damaged-"))
        .collect();
    assert_eq!(aside.len(), 1, "{aside:?}");
    // So is one naming something that isn't an image.
    std::fs::write(
        &records_file,
        br#"{"version":1,"snapshots":[{"id":"x","image":"../../etc","kind":"manual","name":"n","created":1}]}"#,
    )
    .unwrap();
    assert_eq!(s.records().len(), 1);
    assert_eq!(s.records()[0].image, first.image);
}

#[test]
fn pins_deletes_and_shared_images() {
    let s = Setup::new();
    s.world("World", b"r");
    s.images.record_now(s.id(), "Created").unwrap();
    let a = s.snapshot("A");
    let b = s.snapshot("B");
    assert_eq!(a.image, b.image);
    s.images.set_pinned(s.id(), &a.id, true).unwrap();
    assert!(s.records().iter().any(|r| r.id == a.id && r.pinned));
    // Deleting one keeps the image the other names.
    s.images.delete_snapshot(s.id(), &b.id).unwrap();
    assert!(
        s.images
            .store()
            .manifest_path(s.id(), &a.image)
            .unwrap()
            .is_file()
    );
    // Deleting the last deletes the image.
    s.images.delete_snapshot(s.id(), &a.id).unwrap();
    assert!(
        !s.images
            .store()
            .manifest_path(s.id(), &a.image)
            .unwrap()
            .exists()
    );
    assert!(s.records().is_empty());
    assert!(matches!(
        s.images.delete_snapshot(s.id(), &a.id),
        Err(ImageError::NoSnapshot(_))
    ));
    assert!(matches!(
        s.images.set_pinned(s.id(), "20990101-000000-abcd", true),
        Err(ImageError::NoSnapshot(_))
    ));
}

#[test]
fn retention_thins_automatic_snapshots_and_keeps_the_rest() {
    let s = Setup::new();
    let mut instance = s.store.get(s.id()).unwrap();
    instance.snapshots = Some(SnapshotSettings {
        keep_last: Some(2),
        keep_daily: Some(0),
        keep_weekly: Some(0),
        ..SnapshotSettings::default()
    });
    s.store.save(&instance).unwrap();
    s.world("World", b"0");
    s.images.record_now(s.id(), "Created").unwrap();
    let manual = s.snapshot("Mine");
    for i in 0..5 {
        s.write("saves/World/region/r.0.0.mca", format!("change {i}").as_bytes());
        s.images
            .before_risky_change(
                s.id(),
                RiskyChange::ContentUpdate {
                    titles: &[format!("Mod {i}")],
                },
            )
            .unwrap()
            .unwrap();
    }
    let records = s.records();
    let automatic: Vec<&str> = records
        .iter()
        .filter(|r| r.kind == SnapshotKind::BeforeChange)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(automatic, ["Before updating Mod 4", "Before updating Mod 3"]);
    assert!(records.iter().any(|r| r.id == manual.id));
    // Each record left names an image that is there; the deleted ones' images went.
    let images: Vec<String> = s.images.store().manifest_ids(s.id()).unwrap();
    for record in &records {
        assert!(images.contains(&record.image));
    }
    let data_images = images
        .iter()
        .filter(|id| s.image(id).kind != Kind::Content)
        .count();
    assert_eq!(data_images, 3);
}

#[test]
fn data_images_keep_their_content_alive() {
    let s = Setup::new();
    s.write("mods/a.jar", b"the first mod");
    s.world("World", b"r");
    s.images.record_now(s.id(), "Created").unwrap();
    let record = s.snapshot("Keep me");
    let linked = record.content.clone().unwrap();
    // Past the content lineage's 30.
    for i in 0..(KEEP_CONTENT + 3) {
        s.change("mods/a.jar", format!("version {i}").as_bytes(), "Updated a");
    }
    assert!(
        s.images
            .store()
            .manifest_path(s.id(), &linked)
            .unwrap()
            .is_file(),
        "the content image a snapshot links to is kept"
    );
    // And its objects survive a collection of old objects.
    age_objects(&s.data.image_store_dir().join("objects"));
    s.images.store().gc(&GcOptions::default()).unwrap();
    s.nothing_missing();
    let content = s.image(&linked);
    assert!(
        missing_objects(s.images.store(), &content, Selection::All).is_empty(),
        "the linked mod's bytes are kept"
    );
}

#[test]
fn snapshots_and_collections_at_once_lose_nothing() {
    let s = Setup::new();
    s.write("mods/a.jar", b"mod");
    for n in 0..10 {
        s.write(
            &format!("saves/World/region/r.{n}.mca"),
            format!("old {n} ").repeat(50).as_bytes(),
        );
    }
    s.images.record_now(s.id(), "Created").unwrap();
    s.snapshot("First");
    // Every object becomes old garbage the snapshots below find again through the stat
    // cache: collections delete what a snapshot hasn't claimed yet, and a snapshot whose
    // object went reads the file again.
    let store = s.images.store().clone();
    assert!(!age_objects(&store.root().join("objects")).is_empty());
    for id in store.manifest_ids(s.id()).unwrap() {
        std::fs::remove_file(store.manifest_path(s.id(), &id).unwrap()).unwrap();
    }
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let collector = {
        let store = store.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                store.gc(&GcOptions::default()).unwrap();
            }
        })
    };
    for i in 0..8 {
        s.write(&format!("screenshots/{i}.png"), b"png");
        let _ = s.snapshot(&format!("S{i}"));
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    collector.join().unwrap();
    s.nothing_missing();
    assert_eq!(s.records().len(), 8);
}
