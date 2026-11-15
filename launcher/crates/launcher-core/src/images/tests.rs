//! Tests of recording, checking and keeping images on real instance folders, in temporary
//! data folders (never the Launcher's own).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};

use lodestone_image::{Kind, Loader as ImageLoader, Source, WithheldReason};

use super::labels::{INITIAL_LABEL, OUTSIDE_LABEL, WHILE_RUNNING_LABEL};
use super::*;
use crate::game::loaders::{LoaderChoice, LoaderKind};
use crate::instances::{Instance, InstanceStore};

struct Setup {
    _temp: tempfile::TempDir,
    data: DataDir,
    images: Arc<Images>,
    store: InstanceStore,
    instance: Instance,
}

impl Setup {
    fn new() -> Setup {
        Self::with_timing(Timing {
            // Long enough that only an explicit flush records, unless a test says.
            merge: Duration::from_secs(600),
            max_defer: Duration::from_secs(600),
            ..Timing::default()
        })
    }

    fn with_timing(timing: Timing) -> Setup {
        let temp = tempfile::tempdir().unwrap();
        let data = DataDir::at(temp.path().join("data"));
        let store = InstanceStore::new(data.instances_dir());
        let instance = store
            .create(
                "Pack",
                "1.21.1",
                Some(&LoaderChoice {
                    kind: LoaderKind::Fabric,
                    version: "0.16.10".into(),
                }),
            )
            .unwrap();
        Setup {
            images: Images::with_timing(&data, timing),
            _temp: temp,
            data,
            store,
            instance,
        }
    }

    fn id(&self) -> &str {
        &self.instance.id
    }

    fn game(&self, relative: &str) -> PathBuf {
        self.store.game_dir(self.id()).join(relative)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.game(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn remove(&self, relative: &str) {
        std::fs::remove_file(self.game(relative)).unwrap();
    }

    /// An operation that writes a file, then records at once.
    fn change(&self, relative: &str, bytes: &[u8], label: &str) -> Recorded {
        let op = self.images.begin(self.id(), label);
        self.write(relative, bytes);
        op.succeed();
        self.images.flush(self.id()).unwrap().unwrap()
    }

    fn history(&self) -> History {
        self.images.history(self.id()).unwrap()
    }

    fn labels(&self) -> Vec<String> {
        self.history()
            .images
            .into_iter()
            .map(|i| i.label.unwrap_or_default())
            .collect()
    }

    fn image(&self, id: &str) -> lodestone_image::Image {
        self.images.store().load_manifest(self.id(), id).unwrap()
    }

    fn paths(&self, id: &str) -> Vec<String> {
        self.image(id)
            .entries
            .iter()
            .map(|e| e.path.to_string())
            .collect()
    }

    fn state(&self) -> state::ContentState {
        state::load(self.images.store(), self.id())
    }
}

#[test]
fn operations_within_the_window_are_one_image() {
    let s = Setup::new();
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    assert!(first.changed);
    let a = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    a.succeed();
    let b = s.images.begin(s.id(), "Added b.jar");
    s.write("mods/b.jar", b"b");
    b.succeed();
    let merged = s.images.flush(s.id()).unwrap().unwrap();
    assert!(merged.changed);
    assert_eq!(merged.parent.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.labels(), ["Added a.jar; Added b.jar", "Created Pack"]);
    assert_eq!(s.paths(&merged.id), ["mods/a.jar", "mods/b.jar"]);
    let image = s.image(&merged.id);
    assert_eq!(image.kind, Kind::Content);
    assert_eq!(image.producer.app, lodestone_image::manifest::App::Launcher);
    assert_eq!(image.base, Some(lodestone_image::Base::Loader {}));
    assert_eq!(image.game.loader, ImageLoader::Fabric);
    assert_eq!(s.state().current.as_deref(), Some(merged.id.as_str()));
    // Nothing queued: nothing to record.
    assert_eq!(s.images.flush(s.id()).unwrap(), None);
}

#[test]
fn a_record_now_keeps_what_is_queued_and_its_own_label() {
    let s = Setup::new();
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    assert_eq!(s.labels(), ["Added a.jar; Created Pack"]);
    // Its own label never joins an image that holds the same contents already.
    let same = s.images.record_now(s.id(), "Again").unwrap().unwrap();
    assert!(!same.changed);
    assert_eq!(s.labels(), ["Added a.jar; Created Pack"]);
}

#[test]
fn only_success_records_and_a_change_of_nothing_is_dropped() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // A failed operation: dropped without succeeding.
    let failed = s.images.begin(s.id(), "Added broken.jar");
    s.write("mods/broken.jar", b"half");
    drop(failed);
    assert_eq!(s.images.flush(s.id()).unwrap(), None);
    let err: Result<(), &str> = Err("download failed");
    s.images.begin(s.id(), "Added x").finish(&err);
    assert_eq!(s.images.flush(s.id()).unwrap(), None);
    s.remove("mods/broken.jar");
    // An operation that changed nothing writes nothing, and its label goes.
    let noop = s.images.begin(s.id(), "Enabled nothing");
    noop.succeed();
    let recorded = s.images.flush(s.id()).unwrap().unwrap();
    assert!(!recorded.changed);
    assert_eq!(s.labels(), ["Created Pack"]);
}

#[test]
fn a_label_joins_the_image_another_record_took_its_files_in() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // A long install writes its file; another operation's record takes it in before the
    // install succeeds.
    let install = s.images.begin(s.id(), "Added Sodium");
    s.write("mods/sodium.jar", b"sodium");
    let other = s.change("config/a.toml", b"a", "Edited a.toml");
    assert!(other.changed);
    install.succeed();
    let recorded = s.images.flush(s.id()).unwrap().unwrap();
    assert!(!recorded.changed);
    assert_eq!(recorded.id, other.id);
    assert_eq!(s.labels(), ["Edited a.toml; Added Sodium", "Created Pack"]);
    // The id is the contents: still the same image.
    assert_eq!(s.image(&other.id).id, other.id);
}

#[test]
fn game_written_and_other_files_are_never_content() {
    let s = Setup::new();
    for (path, bytes) in [
        ("mods/a.jar", &b"a"[..]),
        ("mods/b.jar.disabled", b"b"),
        ("config/x.json", b"{}"),
        ("defaultconfigs/d.toml", b"d"),
        ("kubejs/startup_scripts/s.js", b"s"),
        ("resourcepacks/r.zip", b"r"),
        ("shaderpacks/s.zip", b"s"),
        ("saves/world/level.dat", b"w"),
        ("logs/latest.log", b"l"),
        ("crash-reports/c.txt", b"c"),
        ("screenshots/s.png", b"p"),
        ("options.txt", b"resourcePacks:[\"vanilla\"]\n"),
        ("servers.dat", b"s"),
        ("mods/.lodestone-abc.download", b"partial"),
        ("config/.lodestone_x", b"x"),
    ] {
        s.write(path, bytes);
    }
    std::fs::write(s.store.dir(s.id()).join("notes.txt"), b"outside").unwrap();
    let recorded = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    assert_eq!(
        s.paths(&recorded.id),
        [
            "config/x.json",
            "defaultconfigs/d.toml",
            "kubejs/startup_scripts/s.js",
            "mods/a.jar",
            "mods/b.jar.disabled",
            "resourcepacks/r.zip",
            "shaderpacks/s.zip",
        ]
    );
    // options.txt isn't a file of the image, but its resource packs are a setting.
    let client = s.image(&recorded.id).settings.client.unwrap();
    assert_eq!(client.resource_packs, ["vanilla"]);
}

#[test]
fn what_the_game_writes_joins_the_next_operation() {
    let s = Setup::new();
    s.write("config/sodium.json", b"{}");
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    assert!(s.state().running_since.is_some());
    // The game rewrites a config while it runs; an operation queued meanwhile waits.
    s.write("config/sodium.json", b"{\"rewritten\":true}");
    s.write("saves/world/level.dat", b"w");
    s.images.end_session(s.id());
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.state().current.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.labels(), ["Created Pack"], "the session made no image");
    // The next launch finds nothing changed by hand.
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
    // The next operation's image has the rewritten config.
    let next = s.change("mods/a.jar", b"a", "Added a.jar");
    assert!(next.changed);
    let image = s.image(&next.id);
    let config = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "config/sodium.json")
        .unwrap();
    assert_eq!(config.size, b"{\"rewritten\":true}".len() as u64);
}

#[test]
fn the_check_before_a_launch() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    // No image yet: the check leaves the first to the session's end, off the launch.
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
    assert!(s.history().images.is_empty());
    s.images.end_session(s.id());
    assert_eq!(s.labels(), [INITIAL_LABEL]);
    let first = s.history().images[0].id.clone();
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());

    // A mod dropped in by hand.
    s.write("mods/by-hand.jar", b"hand");
    let Checked::Recorded(by_hand) = s.images.before_launch(s.id()) else {
        panic!("no change found")
    };
    assert_eq!(by_hand.parent.as_deref(), Some(first.as_str()));
    assert_eq!(s.labels()[0], OUTSIDE_LABEL);
    s.images.end_session(s.id());

    // Only the modification time changed: the check reads no file, so it records, and
    // the contents give the same image.
    let file = std::fs::File::options()
        .write(true)
        .open(s.game("mods/a.jar"))
        .unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
    drop(file);
    let Checked::Recorded(same) = s.images.before_launch(s.id()) else {
        panic!("a touched file is a change to look at")
    };
    assert!(!same.changed);
    s.images.end_session(s.id());
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);

    // The Launcher quit after the launch (or died while the game ran): the next launch
    // can't tell what the game wrote from a change by hand, and records both.
    s.write("config/rewritten.json", b"by the game");
    assert!(s.state().running_since.is_some());
    let images = Images::with_timing(&s.data, s.images.timing());
    let Checked::Recorded(after) = images.before_launch(s.id()) else {
        panic!("a change after a session no end was seen of")
    };
    assert!(after.changed);
    assert_eq!(s.labels()[0], WHILE_RUNNING_LABEL);
    images.end_session(s.id());
    assert_eq!(s.state().running_since, None);
    assert_eq!(images.before_launch(s.id()), Checked::Unchanged);
    images.end_session(s.id());
    // The Launcher that quit is gone with its session.
    s.images.slot(s.id()).session_over();

    // A loader version edited into instance.json by hand.
    let mut instance = s.store.get(s.id()).unwrap();
    instance.loader_version = Some("0.16.11".into());
    s.store.save(&instance).unwrap();
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    s.images.end_session(s.id());

    // Find it for me renames mods on purpose: no check while its search is saved.
    std::fs::write(
        s.store
            .dir(s.id())
            .join(crate::content::mod_finder::FILE_NAME),
        b"{}",
    )
    .unwrap();
    s.remove("mods/by-hand.jar");
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
}

#[test]
fn queued_labels_are_recorded_before_the_check() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    assert_eq!(s.labels()[0], "Added a.jar");
}

#[test]
fn a_loader_change_and_a_resource_pack_toggle_are_new_images() {
    let s = Setup::new();
    s.write("resourcepacks/r.zip", b"r");
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();

    let op = s.images.begin(s.id(), "Changed Fabric to 0.16.11");
    let mut instance = s.store.get(s.id()).unwrap();
    instance.loader_version = Some("0.16.11".into());
    s.store.save(&instance).unwrap();
    op.succeed();
    let loader = s.images.flush(s.id()).unwrap().unwrap();
    assert!(loader.changed);
    assert_eq!(
        s.image(&loader.id).game.loader_version.as_deref(),
        Some("0.16.11")
    );
    assert_eq!(s.paths(&loader.id), s.paths(&first.id));

    let op = s.images.begin(s.id(), "Changed the resource packs");
    s.write(
        "options.txt",
        b"resourcePacks:[\"vanilla\",\"file/r.zip\"]\n",
    );
    op.succeed();
    let packs = s.images.flush(s.id()).unwrap().unwrap();
    assert!(packs.changed);
    assert_eq!(
        s.image(&packs.id).settings.client.unwrap().resource_packs,
        ["vanilla", "file/r.zip"]
    );

    // Going back to earlier content saves that image again, with its new parent.
    let op = s.images.begin(s.id(), "Changed the resource packs");
    std::fs::remove_file(s.game("options.txt")).unwrap();
    op.succeed();
    let back = s.images.flush(s.id()).unwrap().unwrap();
    assert!(back.changed);
    assert_eq!(back.id, loader.id);
    assert_eq!(s.image(&back.id).parent.as_deref(), Some(packs.id.as_str()));
    assert_eq!(s.history().images.len(), 3);
    assert_eq!(s.history().images[0].id, loader.id);
    assert!(s.history().images[0].current);
}

#[test]
fn record_before_change_names_what_is_about_to_change() {
    let s = Setup::new();
    // No image: the first.
    let first = s
        .images
        .record_before_change(s.id(), labels::BEFORE_MODPACK_UPDATE)
        .unwrap()
        .unwrap();
    assert_eq!(s.labels(), [INITIAL_LABEL]);
    // Nothing changed: nothing new.
    let same = s
        .images
        .record_before_change(s.id(), labels::BEFORE_MODPACK_UPDATE)
        .unwrap()
        .unwrap();
    assert!(!same.changed);
    assert_eq!(same.id, first.id);
    // What the game wrote during a session (folded into the baseline) is taken in under
    // the change's label.
    s.images.before_launch(s.id());
    s.write("config/rewritten.toml", b"x");
    s.images.end_session(s.id());
    let before = s
        .images
        .record_before_change(s.id(), labels::BEFORE_SERVER_UPDATE)
        .unwrap()
        .unwrap();
    assert!(before.changed);
    assert_eq!(s.labels()[0], labels::BEFORE_SERVER_UPDATE);
    // A change by hand is called one.
    s.write("mods/hand.jar", b"h");
    s.images
        .record_before_change(s.id(), labels::BEFORE_MODPACK_UPDATE)
        .unwrap();
    assert_eq!(s.labels()[0], OUTSIDE_LABEL);
}

#[test]
fn retention_keeps_thirty_the_current_and_what_data_images_link() {
    let s = Setup::new();
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    // A data image (milestone 5's snapshots) linking to the first content image.
    let mut data = lodestone_image::Image::new(Kind::Data, s.image(&first.id).game);
    data.content = Some(first.id.clone());
    data.seal();
    s.images.store().save_manifest(s.id(), &data).unwrap();
    let mut last = first.clone();
    for n in 0..35 {
        last = s.change(
            &format!("mods/{n}.jar"),
            format!("{n}").as_bytes(),
            &format!("{n}"),
        );
    }
    let history = s.history();
    // The newest 30, and the first, which the data image links to.
    assert_eq!(history.images.len(), KEEP_CONTENT + 1);
    assert_eq!(history.images[0].id, last.id);
    assert!(history.images.iter().any(|i| i.id == first.id));
    assert!(
        s.images
            .store()
            .manifest_path(s.id(), &data.id)
            .unwrap()
            .is_file()
    );
    assert!(s.images.gc_requests() >= 1, "pruning asks for a collection");

    // A manifest that can't be read: nothing is deleted.
    let bad = s
        .images
        .store()
        .manifest_path(s.id(), &"f".repeat(64))
        .unwrap();
    std::fs::write(&bad, b"not json").unwrap();
    let before = s.images.store().manifest_ids(s.id()).unwrap().len();
    s.change("mods/extra.jar", b"extra", "Added extra");
    assert_eq!(
        s.images.store().manifest_ids(s.id()).unwrap().len(),
        before + 1
    );
    std::fs::remove_file(bad).unwrap();

    // An image held with keep_while survives pruning.
    let held = s.history().images[KEEP_CONTENT - 1].id.clone();
    let _keep = s.images.keep_while(s.id(), &held);
    for n in 0..3 {
        s.change(&format!("mods/more{n}.jar"), b"m", "More");
    }
    let ids: Vec<String> = s.history().images.into_iter().map(|i| i.id).collect();
    assert!(ids.contains(&held));
    assert!(ids.contains(&first.id));
}

#[test]
fn the_current_image_and_its_parent_are_kept_whatever_the_clock_says() {
    let s = Setup::new();
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    let second = s.change("mods/a.jar", b"a", "Added a");
    // A clock far ahead made 31 images that rank newest.
    let state = s.state();
    for n in 0..31 {
        let mut image = s.image(&second.id);
        image.created += 1_000_000;
        image.label = Some(format!("future {n}"));
        image.settings.client = Some(lodestone_image::settings::ClientSettings {
            recommended_memory_mb: Some(1000 + n),
            ..Default::default()
        });
        image.seal();
        s.images.store().save_manifest(s.id(), &image).unwrap();
    }
    state::save(s.images.store(), s.id(), &state).unwrap();
    let pruned = {
        let slot = s.images.slot(s.id());
        let _record = slot.lock_record();
        s.images.prune_locked(s.id(), Some(&second.id)).unwrap()
    };
    assert_eq!(pruned, 1);
    let ids = s.images.store().manifest_ids(s.id()).unwrap();
    assert!(ids.contains(&second.id), "the current image");
    assert!(ids.contains(&first.id), "its parent");
}

#[test]
fn a_reused_id_starts_a_new_history() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // The instance goes to the Trash without the Launcher noticing (another copy of
    // it, say), and a new one takes its name and id.
    std::fs::remove_dir_all(s.store.dir(s.id())).unwrap();
    let mut again = s.store.create("Pack", "1.20.1", None).unwrap();
    assert_eq!(again.id, s.id());
    again.created += 1;
    s.store.save(&again).unwrap();
    assert!(
        s.history().images.is_empty(),
        "not the old instance's history"
    );
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let history = s.history();
    assert_eq!(history.images.len(), 1);
    assert_eq!(history.images[0].game.minecraft, "1.20.1");
    assert_eq!(history.images[0].parent, None);

    // Deleting the instance forgets its images at once.
    s.images.delete_instance(s.id()).unwrap();
    assert!(!s.images.store().instance_dir(s.id()).unwrap().exists());
    assert!(s.history().images.is_empty());
}

#[test]
fn a_crash_after_the_manifest_leaves_the_old_pointer() {
    let s = Setup::new();
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    let state_path = state::path(s.images.store(), s.id()).unwrap();
    let saved = std::fs::read(&state_path).unwrap();
    // The state file can't be replaced (a folder is in the way).
    std::fs::remove_file(&state_path).unwrap();
    std::fs::create_dir(&state_path).unwrap();
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    assert!(s.images.flush(s.id()).is_err());
    // The manifest was saved; the pointer wasn't.
    std::fs::remove_dir(&state_path).unwrap();
    std::fs::write(&state_path, saved).unwrap();
    assert_eq!(s.state().current.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.images.store().manifest_ids(s.id()).unwrap().len(), 2);
    // The label stayed queued; the next record moves the pointer.
    let moved = s.images.flush(s.id()).unwrap().unwrap();
    assert!(moved.changed);
    assert_eq!(moved.parent.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.state().current.as_deref(), Some(moved.id.as_str()));
    assert_eq!(s.labels(), ["Added a.jar", "Created Pack"]);
}

#[test]
fn operations_records_and_collections_at_once_lose_nothing() {
    let s = Setup::new();
    for n in 0..20 {
        s.write(
            &format!("mods/old-{n}.jar"),
            format!("old {n} ").repeat(50).as_bytes(),
        );
    }
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // Every object becomes old garbage the records below find again through the stat
    // cache: collections delete what a record hasn't claimed yet, and a record whose
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
            let mut removed = 0;
            while !stop.load(Ordering::SeqCst) {
                removed += store
                    .gc(&lodestone_image::store::gc::GcOptions::default())
                    .unwrap()
                    .objects_removed;
            }
            removed
        })
    };
    let game = s.store.game_dir(s.id());
    let workers: Vec<_> = (0..2)
        .map(|t| {
            let images = s.images.clone();
            let id = s.id().to_string();
            let game = game.clone();
            std::thread::spawn(move || {
                for n in 0..15 {
                    let op = images.begin(&id, format!("{t}-{n}"));
                    let path = game.join(format!("mods/{t}-{n}.jar"));
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, format!("{t}-{n}")).unwrap();
                    op.succeed();
                    if n % 3 == 0 {
                        images.flush(&id).unwrap();
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    s.images.flush(s.id()).unwrap();
    stop.store(true, Ordering::SeqCst);
    let removed = collector.join().unwrap();
    eprintln!("the collections removed {removed} objects");
    let ids = store.manifest_ids(s.id()).unwrap();
    assert!(ids.len() > 2);
    for id in ids {
        let image = store.load_manifest(s.id(), &id).unwrap();
        for entry in &image.entries {
            assert!(
                store.object_complete(&entry.sha256, entry.size),
                "{}",
                entry.path
            );
        }
    }
    let current = s.state().current.unwrap();
    assert_eq!(s.paths(&current).len(), 50);
}

#[test]
fn records_under_way_share_the_free_space() {
    let s = Setup::new();
    s.write("mods/a.jar", &[7; 1000]);
    *s.images.fake_free.lock().unwrap() = Some(recorder::SPARE_BYTES + 5000);
    // Another instance's record under way holds most of the room: alone this record would
    // fit, but the two together would not.
    let other = s.images.reserve_space(4500).unwrap();
    let err = s.images.record_now(s.id(), "Created").unwrap_err();
    assert!(err.to_string().contains("other records under way"), "{err}");
    assert!(s.images.reserve_space(4500).is_err());
    assert_eq!(*s.images.reserved.lock().unwrap(), 4500);
    drop(other);
    assert_eq!(*s.images.reserved.lock().unwrap(), 0);
    s.images.record_now(s.id(), "Created").unwrap().unwrap();
    assert_eq!(
        *s.images.reserved.lock().unwrap(),
        0,
        "a record gives its room back"
    );
}

/// Every object file under `dir`, made an hour and a half old.
fn age_objects(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let file = std::fs::File::open(&path).unwrap();
                file.set_modified(SystemTime::now() - Duration::from_secs(5400))
                    .unwrap();
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn the_image_store_and_the_snapshot_store_never_collect_each_others_objects() {
    let s = Setup::new();
    s.write("mods/a.jar", b"the same mod in both stores");
    s.write("saves/world/level.dat", b"a world only snapshots take");
    let recorded = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    let snapshots = crate::snapshots::Store::new(&s.data);
    let instance = s.store.get(s.id()).unwrap();
    snapshots
        .create(
            &instance,
            &s.store.dir(s.id()),
            "Before",
            None,
            &crate::snapshots::Selection::all(),
            false,
            &|_, _| {},
        )
        .unwrap();
    let image_objects = age_objects(&s.data.image_store_dir().join("objects"));
    let snapshot_objects = age_objects(&s.data.root().join("snapshots").join("objects"));
    assert!(!image_objects.is_empty());
    assert!(snapshot_objects.len() >= 2);

    // Each collector keeps what its own manifests name, and never looks at the other.
    snapshots.collect_garbage();
    s.images
        .store()
        .gc(&lodestone_image::store::gc::GcOptions::default())
        .unwrap();
    assert!(image_objects.iter().all(|p| p.exists()));
    assert!(snapshot_objects.iter().all(|p| p.exists()));

    // With the content image gone, the image store's collector takes its object; the
    // snapshot's copy of the same bytes stays. (Deleting asks for a collection in the
    // background too; this one waits for it.)
    s.images.delete_instance(s.id()).unwrap();
    s.images
        .store()
        .gc(&lodestone_image::store::gc::GcOptions::default())
        .unwrap();
    assert!(image_objects.iter().all(|p| !p.exists()));
    assert!(snapshot_objects.iter().all(|p| p.exists()));
    // And the other way round.
    for snapshot in snapshots.list(s.id()) {
        snapshots.delete(s.id(), &snapshot.id).unwrap();
    }
    let _ = recorded;
    snapshots.collect_garbage();
    assert!(snapshot_objects.iter().all(|p| !p.exists()));
}

#[cfg(unix)]
#[test]
fn links_and_bad_names_are_withheld_not_followed() {
    let s = Setup::new();
    let outside = s._temp.path().join("secret.txt");
    std::fs::write(&outside, b"not the instance's").unwrap();
    std::fs::create_dir_all(s.game("mods")).unwrap();
    std::os::unix::fs::symlink(&outside, s.game("mods/link.jar")).unwrap();
    std::os::unix::fs::symlink(s._temp.path(), s.game("config")).unwrap();
    s.write("mods/a:b.jar", b"colon");
    s.write("mods/CON.jar", b"device");
    s.write("mods/ok.jar", b"ok");
    let recorded = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    let image = s.image(&recorded.id);
    assert_eq!(s.paths(&recorded.id), ["mods/ok.jar"]);
    let withheld: Vec<(String, WithheldReason)> = image
        .withheld
        .iter()
        .map(|w| (w.path.clone(), w.reason.clone()))
        .collect();
    assert!(withheld.contains(&("mods/link.jar".into(), WithheldReason::Symlink)));
    assert!(withheld.contains(&("config".into(), WithheldReason::Symlink)));
    assert!(withheld.contains(&("mods/a:b.jar".into(), WithheldReason::BadName)));
    assert!(withheld.contains(&("mods/CON.jar".into(), WithheldReason::BadName)));
    assert_eq!(s.history().images[0].withheld, withheld.len());
}

#[test]
fn ids_the_store_cant_name_record_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path());
    let store = InstanceStore::new(data.instances_dir());
    // `con` is a Windows device name; the store refuses it.
    let instance = store.create("con", "1.21.1", None).unwrap();
    assert_eq!(instance.id, "con");
    let images = Images::open(&data);
    assert_eq!(images.record_now("con", "Created con").unwrap(), None);
    let op = images.begin("con", "Added x");
    op.succeed();
    assert_eq!(images.flush("con").unwrap(), None);
    assert!(matches!(images.before_launch("con"), Checked::Skipped(_)));
    assert_eq!(images.history("con").unwrap(), History::default());
    assert!(!data.image_store_dir().exists());
    assert!(matches!(images.history("nope"), Err(ImageError::Gone(_))));
}

#[test]
fn a_record_of_a_deleted_instance_drops_its_labels() {
    let s = Setup::new();
    let op = s.images.begin(s.id(), "Added a");
    op.succeed();
    std::fs::remove_dir_all(s.store.dir(s.id())).unwrap();
    assert!(matches!(s.images.flush(s.id()), Err(ImageError::Gone(_))));
    assert!(s.images.slot(s.id()).pending().is_empty());
}

#[test]
fn modpack_bases_and_sources_reach_the_image() {
    let s = Setup::new();
    let url = "https://cdn.modrinth.com/data/AANobbMI/versions/v1/sodium.jar";
    s.write("mods/sodium.jar", b"sodium bytes");
    let meta = std::fs::metadata(s.game("mods/sodium.jar")).unwrap();
    let dir = s.store.dir(s.id());
    crate::content::provenance::Provenance::update(&dir, |p| {
        p.files.insert(
            "mods/sodium.jar".into(),
            crate::content::provenance::Entry {
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(crate::content::provenance::nanos),
                sha1: "1".repeat(40),
                sha512: "5".repeat(128),
                modrinth: Some(crate::content::provenance::ModrinthSource {
                    url: Some(url.into()),
                    client_side: "required".into(),
                    server_side: "optional".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    })
    .unwrap();
    crate::modpack::ModpackRecord {
        project_id: "1KVo5zza".into(),
        version_id: "fo640".into(),
        pack_sha512: Some("a".repeat(128)),
        ..Default::default()
    }
    .save(&dir)
    .unwrap();
    let recorded = s
        .images
        .record_now(s.id(), "Installed FO 6.4.0")
        .unwrap()
        .unwrap();
    let image = s.image(&recorded.id);
    assert_eq!(
        image.base,
        Some(lodestone_image::Base::ModrinthPack {
            project: "1KVo5zza".into(),
            version: "fo640".into(),
            sha512: "a".repeat(128),
        })
    );
    assert!(matches!(image.entries[0].source, Source::Modrinth { .. }));
    assert_eq!(
        image.entries[0].env.server,
        lodestone_image::EnvValue::Optional
    );
}

#[test]
fn merged_records_run_on_their_own_and_wait_for_the_session() {
    let s = Setup::with_timing(Timing {
        merge: Duration::from_millis(50),
        max_defer: Duration::from_millis(400),
        ..Timing::default()
    });
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let wait_for = |n: usize| {
        let until = Instant::now() + Duration::from_secs(10);
        while s.history().images.len() < n && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        s.history().images.len()
    };
    let a = s.images.begin(s.id(), "a");
    s.write("mods/a.jar", b"a");
    a.succeed();
    let b = s.images.begin(s.id(), "b");
    s.write("mods/b.jar", b"b");
    b.succeed();
    assert_eq!(wait_for(2), 2);
    assert_eq!(s.labels()[0], "a; b");

    // During a session nothing is recorded; once it ends, the queued label is.
    s.images.before_launch(s.id());
    let c = s.images.begin(s.id(), "c");
    s.write("mods/c.jar", b"c");
    c.succeed();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(s.history().images.len(), 2);
    s.images.session_ended(s.id());
    assert_eq!(wait_for(3), 3);
    assert_eq!(s.labels()[0], "c");

    // So does a Find it for me search, until it ends.
    let finder = s
        .store
        .dir(s.id())
        .join(crate::content::mod_finder::FILE_NAME);
    std::fs::write(&finder, b"{}").unwrap();
    let d = s.images.begin(s.id(), "d");
    s.write("mods/d.jar", b"d");
    d.succeed();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(s.history().images.len(), 3);
    std::fs::remove_file(&finder).unwrap();
    assert_eq!(wait_for(4), 4);

    // Settling records what is queued at once, whatever the window.
    let slow = Setup::new();
    slow.images.record_now(slow.id(), "Created Pack").unwrap();
    let e = slow.images.begin(slow.id(), "e");
    slow.write("mods/e.jar", b"e");
    e.succeed();
    slow.images.settle(Duration::from_secs(10));
    assert_eq!(slow.labels()[0], "e");
}

#[test]
fn a_crash_after_the_first_manifest_keeps_its_image() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    let first = s
        .images
        .record_now(s.id(), "Created Pack")
        .unwrap()
        .unwrap();
    // A crash after the manifest and before the pointer: what the first record wrote
    // before its manifest is a state naming only the instance.
    state::save(
        s.images.store(),
        s.id(),
        &state::ContentState {
            instance_created: Some(s.instance.created),
            ..state::ContentState::default()
        },
    )
    .unwrap();
    // The next record (here the session's end after a check that left the first image to
    // it) takes the image up, label and all, rather than taking it for an earlier
    // instance's.
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
    s.images.end_session(s.id());
    assert_eq!(s.state().current.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.labels(), ["Created Pack"]);

    // The first record writes that state before anything else: here its objects can't be
    // written (a file is where their folder goes), and the state is there all the same.
    let fresh = Setup::new();
    fresh.write("mods/b.jar", b"b");
    let blocked = fresh.images.store().root().join("objects");
    std::fs::create_dir_all(blocked.parent().unwrap()).unwrap();
    std::fs::write(&blocked, b"in the way").unwrap();
    assert!(fresh.images.record_now(fresh.id(), "Created Pack").is_err());
    let written = fresh.state();
    assert_eq!(written.instance_created, Some(fresh.instance.created));
    assert_eq!(written.current, None);
    std::fs::remove_file(&blocked).unwrap();
    let recorded = fresh
        .images
        .record_now(fresh.id(), "Created Pack")
        .unwrap()
        .unwrap();
    assert!(recorded.changed);
    assert_eq!(fresh.labels(), ["Created Pack"]);
}

#[test]
fn a_check_that_couldnt_run_still_holds_merged_records_and_runs_at_the_end() {
    let s = Setup::with_timing(Timing {
        merge: Duration::from_millis(50),
        max_defer: Duration::from_millis(400),
        gate: Duration::from_millis(100),
    });
    s.write("mods/a.jar", b"a");
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // A mod dropped in by hand, then Play while the store is suspended (a data move).
    s.write("mods/by-hand.jar", b"hand");
    let slot = s.images.slot(s.id());
    let suspension = s.images.suspend();
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
    drop(suspension);
    // The session is marked all the same: nothing is recorded while the game writes.
    assert!(slot.in_session());
    s.write("config/written-by-the-game.json", b"{}");
    let op = s.images.begin(s.id(), "Added queued.jar");
    s.write("mods/queued.jar", b"q");
    op.succeed();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(s.history().images.len(), 1);
    // The session's end runs the check that couldn't run: what is queued, then what
    // changed by hand, is recorded instead of folded into the baseline.
    s.images.end_session(s.id());
    assert!(!slot.in_session());
    assert_eq!(s.labels(), ["Added queued.jar", "Created Pack"]);
    let newest = s.history().images[0].id.clone();
    assert!(s.paths(&newest).contains(&"mods/by-hand.jar".to_string()));
    // Without anything queued, a change by hand is "Changed by hand".
    s.write("mods/by-hand-2.jar", b"hand");
    let suspension = s.images.suspend();
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
    drop(suspension);
    s.images.end_session(s.id());
    assert_eq!(s.labels()[0], OUTSIDE_LABEL);
    // A check that ran leaves the session's end to take the baseline again.
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.write("config/written-again.json", b"{}");
    s.images.end_session(s.id());
    assert_eq!(s.labels().len(), 3);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
}

#[test]
fn a_record_holding_the_instance_at_launch_leaves_the_games_writes_out() {
    let s = Setup::with_timing(Timing {
        gate: Duration::from_millis(50),
        ..Timing::default()
    });
    s.write("mods/a.jar", b"a");
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // A pack installed, its record still running when Play is pressed.
    let slot = s.images.slot(s.id());
    let held = slot.lock_record();
    s.write("mods/pack.jar", b"pack");
    assert_eq!(
        s.images.before_launch(s.id()),
        Checked::Skipped("a record of the instance is still running")
    );
    s.images
        .record_held(&slot, s.id(), Some("Installed Pack 1.0"))
        .unwrap();
    drop(held);
    // The game rewrites a config at startup: it joins the baseline, not an image.
    s.write("config/written-by-the-game.json", b"{}");
    s.images.end_session(s.id());
    assert_eq!(s.labels(), ["Installed Pack 1.0", "Created Pack"]);
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
}

#[test]
fn play_never_waits_for_a_first_image() {
    // The default wait: were the check to wait, this test would take ten seconds.
    let s = Setup::with_timing(Timing::default());
    s.write("mods/a.jar", b"a");
    let slot = s.images.slot(s.id());
    // A new pack's first image is being recorded (the startup pass, or an import).
    let held = slot.lock_record();
    let started = Instant::now();
    assert_eq!(
        s.images.before_launch(s.id()),
        Checked::Skipped("the instance's first image is being recorded")
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    s.images
        .record_held(&slot, s.id(), Some("Installed Pack 1.0"))
        .unwrap();
    // Recorded during the session, so a Launcher that quits now knows the game ran.
    assert!(s.state().running_since.is_some());
    drop(held);
    s.write("config/written-by-the-game.json", b"{}");
    s.images.end_session(s.id());
    assert_eq!(s.labels(), ["Installed Pack 1.0"]);
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
}

#[test]
fn a_change_of_which_files_are_content_records_without_a_label() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    s.write("scripts/a.zs", b"a script");
    s.images.record_now(s.id(), "Created").unwrap();
    // A rebuilt server pack with the same files lists one more already on disk.
    let op = s.images.begin_rescope(s.id());
    let files = ["scripts/a.zs", "mods/a.jar"]
        .into_iter()
        .map(|path| (path.to_string(), crate::modpack::PackFile::default()))
        .collect();
    let saved = crate::modpack::ModpackRecord {
        files,
        ..Default::default()
    }
    .save(&s.store.dir(s.id()));
    op.finish(&saved);
    let recorded = s.images.flush(s.id()).unwrap().unwrap();
    assert!(recorded.changed);
    assert_eq!(s.image(&recorded.id).label, None);
    assert!(s.paths(&recorded.id).contains(&"scripts/a.zs".to_string()));
    // Not "Changed by hand" at the next launch.
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
    assert_eq!(s.labels(), ["", "Created"]);
    // When nothing new is in, it only takes the baseline again.
    s.images.begin_rescope(s.id()).succeed();
    let again = s.images.flush(s.id()).unwrap().unwrap();
    assert!(!again.changed);
    assert_eq!(s.labels().len(), 2);
}

#[test]
fn merged_records_never_wait_on_another_instances_record() {
    let s = Setup::with_timing(Timing {
        merge: Duration::from_millis(30),
        max_defer: Duration::from_millis(200),
        ..Timing::default()
    });
    let other = s.store.create("Other", "1.21.1", None).unwrap();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    s.images.record_now(&other.id, "Created Other").unwrap();
    // The first instance's record runs (a big pack's first image) when both change.
    let slot = s.images.slot(s.id());
    let held = slot.lock_record();
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    let op = s.images.begin(&other.id, "Added b.jar");
    std::fs::create_dir_all(s.store.game_dir(&other.id).join("mods")).unwrap();
    std::fs::write(s.store.game_dir(&other.id).join("mods/b.jar"), b"b").unwrap();
    op.succeed();
    let other_labels = || s.images.history(&other.id).unwrap().images.len();
    let until = Instant::now() + Duration::from_secs(10);
    while other_labels() < 2 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        other_labels(),
        2,
        "the other instance is recorded meanwhile"
    );
    drop(held);
    while s.labels().len() < 2 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(s.labels(), ["Added a.jar", "Created Pack"]);
}

#[test]
fn a_suspended_store_writes_nothing_until_it_resumes() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let images_dir = s.images.store().instance_dir(s.id()).unwrap();
    let listing = |dir: &Path| {
        let mut all = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let meta = entry.metadata().unwrap();
                if meta.is_dir() {
                    stack.push(entry.path());
                } else {
                    all.push((entry.path(), meta.len(), meta.modified().unwrap()));
                }
            }
        }
        all.sort();
        all
    };

    // Suspending waits for a write that is running.
    let writing = s.images.enter().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let images = s.images.clone();
    let suspender = std::thread::spawn(move || {
        let suspended = images.suspend();
        tx.send(()).unwrap();
        suspended
    });
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    drop(writing);
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let suspended = suspender.join().unwrap();
    assert!(s.images.suspended());

    let before = listing(s.images.store().root());
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    assert!(matches!(s.images.flush(s.id()), Err(ImageError::Suspended)));
    assert!(matches!(
        s.images.record_now(s.id(), "x"),
        Err(ImageError::Suspended)
    ));
    assert!(matches!(
        s.images.record_before_change(s.id(), "x"),
        Err(ImageError::Suspended)
    ));
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Skipped(_)
    ));
    s.images.end_session(s.id());
    assert!(matches!(
        s.images.delete_instance(s.id()),
        Err(ImageError::Suspended)
    ));
    assert!(matches!(
        s.images.forget_deleted(),
        Err(ImageError::Suspended)
    ));
    let requests = s.images.gc_requests();
    s.images.request_gc();
    assert_eq!(s.images.gc_requests(), requests + 1);
    assert!(!s.images.gc_running(), "no collection while suspended");
    s.images.settle(Duration::from_secs(5));
    assert_eq!(listing(s.images.store().root()), before, "nothing written");
    assert!(images_dir.is_dir());

    // Resumed, the queued label is recorded, and the collection that waited runs.
    drop(suspended);
    assert!(!s.images.suspended());
    s.images.flush(s.id()).unwrap().unwrap();
    assert_eq!(s.labels()[0], "Added a.jar");

    // Kept, the store stays suspended for good (a data move that succeeded).
    s.images.suspend().keep();
    assert!(s.images.suspended());
    assert!(matches!(
        s.images.record_now(s.id(), "x"),
        Err(ImageError::Suspended)
    ));
}

#[test]
fn instances_deleted_outside_the_launcher_are_forgotten() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    let other = s.store.create("Other", "1.21.1", None).unwrap();
    s.images.record_now(&other.id, "Created Other").unwrap();
    // Nothing is gone yet.
    assert_eq!(s.images.forget_deleted().unwrap(), 0);
    // Deleted in Finder: its folder goes, its images stay behind.
    std::fs::remove_dir_all(s.store.dir(&other.id)).unwrap();
    let theirs = s.images.store().instance_dir(&other.id).unwrap();
    assert!(theirs.is_dir());
    assert_eq!(s.images.forget_deleted().unwrap(), 1);
    assert!(!theirs.exists());
    assert_eq!(
        s.history().images.len(),
        1,
        "the other instance keeps its images"
    );

    // A collection forgets them too, before it collects.
    let third = s.store.create("Third", "1.21.1", None).unwrap();
    s.images.record_now(&third.id, "Created Third").unwrap();
    std::fs::remove_dir_all(s.store.dir(&third.id)).unwrap();
    let theirs = s.images.store().instance_dir(&third.id).unwrap();
    s.images.request_gc();
    let until = Instant::now() + Duration::from_secs(10);
    while (theirs.exists() || s.images.gc_running()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!theirs.exists());

    // Nothing is forgotten while the instances folder itself is missing (a drive that
    // isn't there).
    let instances = s.data.instances_dir();
    let away = s.data.root().join("instances-away");
    std::fs::rename(&instances, &away).unwrap();
    assert_eq!(s.images.forget_deleted().unwrap(), 0);
    std::fs::rename(&away, &instances).unwrap();
    assert_eq!(s.history().images.len(), 1);
}

#[test]
fn files_a_pack_wrote_outside_the_content_folders_are_content() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    s.write("scripts/recipes.zs", b"recipes");
    s.write("scripts/mine.zs", b"by hand");
    s.write("options.txt", b"from the pack");
    let files = ["scripts/recipes.zs", "options.txt", "mods/a.jar"]
        .into_iter()
        .map(|path| (path.to_string(), crate::modpack::PackFile::default()))
        .collect();
    crate::modpack::ModpackRecord {
        files,
        ..Default::default()
    }
    .save(&s.store.dir(s.id()))
    .unwrap();
    let recorded = s
        .images
        .record_now(s.id(), "Installed Pack")
        .unwrap()
        .unwrap();
    assert_eq!(
        s.paths(&recorded.id),
        ["mods/a.jar", "scripts/recipes.zs"],
        "the pack's script, not the game's options or a script added by hand"
    );
    // The quick check looks at them too.
    s.write("scripts/recipes.zs", b"edited by hand");
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    assert_eq!(s.labels()[0], OUTSIDE_LABEL);
}

#[test]
fn a_session_no_end_was_seen_of_is_over_once_a_record_runs() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    // The Launcher quit after the launch: no session end.
    s.images.before_launch(s.id());
    assert!(s.state().running_since.is_some());
    let images = Images::with_timing(&s.data, s.images.timing());
    let op = images.begin(s.id(), "Added b.jar");
    s.write("mods/b.jar", b"b");
    op.succeed();
    images.flush(s.id()).unwrap();
    // The record took the content again, so the next check is an ordinary one.
    assert_eq!(s.state().running_since, None);
    s.write("mods/hand.jar", b"hand");
    assert!(matches!(
        images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    assert_eq!(s.labels(), [OUTSIDE_LABEL, "Added b.jar", "Created Pack"]);
    images.end_session(s.id());
}

#[test]
fn a_game_changed_during_a_session_is_still_found() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created Pack").unwrap();
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    // The loader version edited into instance.json while the game ran: the session's end
    // takes the files again, not the game.
    let mut instance = s.store.get(s.id()).unwrap();
    instance.loader_version = Some("0.16.99".into());
    s.store.save(&instance).unwrap();
    s.images.end_session(s.id());
    assert_eq!(
        s.state()
            .game
            .and_then(|game| game.loader_version)
            .as_deref(),
        Some("0.16.10")
    );
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    assert_eq!(s.labels()[0], OUTSIDE_LABEL);
    let current = s.history().images[0].id.clone();
    assert_eq!(
        s.image(&current).game.loader_version.as_deref(),
        Some("0.16.99")
    );
    s.images.end_session(s.id());
}

/// Makes `dir` and every folder in it read-only (or writable again).
#[cfg(unix)]
fn set_folders_writable(dir: &Path, writable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if writable { 0o755 } else { 0o555 };
    if writable {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            set_folders_writable(&entry.path(), writable);
        }
    }
    if !writable {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn a_check_that_failed_to_record_runs_again_at_the_sessions_end() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    s.images.record_now(s.id(), "Created").unwrap();
    s.write("mods/by-hand.jar", b"a mod dropped in by hand");
    let objects = s.images.store().root().join("objects");
    set_folders_writable(&objects, false);
    let checked = s.images.before_launch(s.id());
    set_folders_writable(&objects, true);
    assert!(matches!(checked, Checked::Failed(_)), "{checked:?}");
    // Not folded into the baseline: the session's end records it.
    s.images.end_session(s.id());
    assert_eq!(s.labels(), [OUTSIDE_LABEL, "Created"]);
    let newest = s.history().images[0].id.clone();
    assert!(s.paths(&newest).contains(&"mods/by-hand.jar".to_string()));
    assert_eq!(s.state().running_since, None);
}

#[cfg(unix)]
#[test]
fn the_check_before_a_launch_reads_no_file() {
    use std::os::unix::fs::PermissionsExt;
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    s.images.record_now(s.id(), "Created").unwrap();
    // A file nobody may read: the check only looks at sizes and times.
    let path = s.game("mods/a.jar");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let checked = s.images.before_launch(s.id());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(checked, Checked::Unchanged);
    s.images.end_session(s.id());
}

#[test]
fn an_old_sessions_end_leaves_a_new_session_alone() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    let slot = s.images.slot(s.id());
    // Play pressed again just as the last run's end is handled.
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
    assert!(slot.in_session(), "the new run still plays");
    assert!(s.state().running_since.is_some());
    // What the new game writes waits for its own end.
    s.write("config/written-by-the-game.json", b"{}");
    s.images.end_session(s.id());
    assert!(!slot.in_session());
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.images.before_launch(s.id()), Checked::Unchanged);
    s.images.end_session(s.id());
    assert_eq!(s.labels(), ["Created"]);
}

#[test]
fn settling_skips_a_saved_search() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    let op = s.images.begin(s.id(), "Added a.jar");
    s.write("mods/a.jar", b"a");
    op.succeed();
    let finder = s
        .store
        .dir(s.id())
        .join(crate::content::mod_finder::FILE_NAME);
    std::fs::write(&finder, b"{}").unwrap();
    s.images.settle(Duration::from_secs(10));
    assert_eq!(s.labels(), ["Created"]);
    std::fs::remove_file(&finder).unwrap();
    s.images.settle(Duration::from_secs(10));
    assert_eq!(s.labels(), ["Added a.jar", "Created"]);
}

#[test]
fn first_images_of_instances_from_before_recording() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    let other = s.store.create("Other", "1.21.1", None).unwrap();
    let playing = s.store.create("Playing", "1.21.1", None).unwrap();
    s.images.record_now(&other.id, "Created Other").unwrap();
    // A game session runs: its end records that instance's first image instead.
    s.images
        .slot(&playing.id)
        .session
        .fetch_add(1, Ordering::SeqCst);
    assert_eq!(s.images.record_first_images(), 1);
    assert_eq!(s.labels(), [INITIAL_LABEL]);
    assert_eq!(
        s.images.history(&other.id).unwrap().images.len(),
        1,
        "an instance with images keeps them"
    );
    assert!(s.images.history(&playing.id).unwrap().images.is_empty());
    // The first operation on it then has a parent holding what it replaces.
    let first = s.history().images[0].id.clone();
    let updated = s.change("mods/a.jar", b"a, updated", "Updated a");
    assert_eq!(updated.parent.as_deref(), Some(first.as_str()));
    assert_eq!(s.images.record_first_images(), 0);
}

#[cfg(unix)]
#[test]
fn a_first_image_that_failed_is_tried_again_at_the_next_start() {
    let s = Setup::new();
    s.write("mods/a.jar", b"a");
    // The disk is full (or the free-space check refuses a big pack).
    let objects = s.images.store().root().join("objects");
    std::fs::create_dir_all(&objects).unwrap();
    set_folders_writable(&objects, false);
    let first = s.images.record_first_images();
    set_folders_writable(&objects, true);
    assert_eq!(first, 0);
    assert!(state::path(s.images.store(), s.id()).unwrap().exists());
    assert_eq!(s.state().current, None);
    // The state file a failed first record leaves is no reason to skip the instance.
    assert_eq!(s.images.record_first_images(), 1);
    assert_eq!(s.labels(), [INITIAL_LABEL]);
    let initial = s.history().images[0].id.clone();
    assert!(s.paths(&initial).contains(&"mods/a.jar".to_string()));
    let updated = s.change("mods/a.jar", b"a, updated", "Updated a");
    assert_eq!(updated.parent.as_deref(), Some(initial.as_str()));
}

#[test]
fn a_job_is_one_operation_with_each_steps_label() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    let mut job = s.images.begin(s.id(), "");
    s.write("mods/a.jar", b"a");
    job.note("Updated A");
    s.write("mods/b.jar", b"b");
    job.note("Updated B");
    job.succeed();
    s.images.flush(s.id()).unwrap();
    assert_eq!(s.labels(), ["Updated A; Updated B", "Created"]);
    // An action that failed part way records what it did change, said so.
    let partly = s.images.begin(s.id(), "Updated Pack to 2.0");
    s.write("mods/c.jar", b"c");
    partly.finish_partly(&Err::<(), ()>(()));
    s.images.flush(s.id()).unwrap();
    assert_eq!(s.labels()[0], "Updated Pack to 2.0 (stopped part way)");
    // A job that changed nothing records nothing.
    let mut idle = s.images.begin(s.id(), "");
    idle.note("Partly installed C");
    idle.succeed();
    let recorded = s.images.flush(s.id()).unwrap().unwrap();
    assert!(!recorded.changed);
    assert_eq!(s.labels().len(), 3);
}

#[test]
fn a_suspension_waiting_for_a_write_can_be_cancelled() {
    let s = Setup::new();
    let writing = s.images.enter().unwrap();
    assert!(s.images.suspend_unless(&|| true).is_none());
    assert!(!s.images.suspended());
    drop(writing);
    let suspension = s
        .images
        .suspend_unless(&|| false)
        .expect("nothing to wait for");
    assert!(s.images.suspended());
    drop(suspension);
    assert!(!s.images.suspended());
    // Kept after a data move, the store stays suspended whatever suspension comes next
    // (another move, cancelled in a Launcher whose restart failed).
    s.images.suspend().keep();
    assert!(s.images.suspended());
    drop(s.images.suspend());
    assert!(s.images.suspended());
    assert!(s.images.suspend_unless(&|| false).is_some());
    assert!(s.images.suspended());
}

#[cfg(unix)]
#[test]
fn linked_instances_on_a_missing_drive_are_not_forgotten() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    // The instance kept on another drive, linked into the instances folder.
    let drive = s._temp.path().join("drive");
    std::fs::create_dir_all(&drive).unwrap();
    let dir = s.store.dir(s.id());
    std::fs::rename(&dir, drive.join("pack")).unwrap();
    std::os::unix::fs::symlink(drive.join("pack"), &dir).unwrap();
    assert_eq!(s.history().images.len(), 1);
    // The drive unplugged: the link dangles, and the images stay.
    let away = s._temp.path().join("away");
    std::fs::rename(&drive, &away).unwrap();
    assert_eq!(s.images.forget_deleted().unwrap(), 0);
    std::fs::rename(&away, &drive).unwrap();
    assert_eq!(s.history().images.len(), 1);
    // A real folder without instance.json, or nothing at all, is a deletion.
    std::fs::remove_file(&dir).unwrap();
    assert_eq!(s.images.forget_deleted().unwrap(), 1);
}

#[cfg(unix)]
#[test]
fn the_store_folder_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    let mode = std::fs::metadata(s.images.store().root())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn an_empty_label_queued_still_lets_the_check_look() {
    let s = Setup::new();
    s.images.record_now(s.id(), "Created").unwrap();
    // An operation that succeeded saying nothing (an install that installed nothing).
    s.images.begin(s.id(), "").succeed();
    s.write("mods/by-hand.jar", b"hand");
    assert!(matches!(
        s.images.before_launch(s.id()),
        Checked::Recorded(Recorded { changed: true, .. })
    ));
    assert_eq!(s.labels(), [OUTSIDE_LABEL, "Created"]);
    s.images.end_session(s.id());
}

#[test]
fn the_check_before_a_search_records_a_change_made_by_hand() {
    let s = Setup::new();
    s.write("mods/b.jar", b"b");
    s.images.record_now(s.id(), "Created").unwrap();
    assert_eq!(s.images.check_now(s.id()).unwrap(), None);
    s.remove("mods/b.jar");
    let finder = s
        .store
        .dir(s.id())
        .join(crate::content::mod_finder::FILE_NAME);
    // While a search is saved it renames mods on purpose: nothing is checked.
    std::fs::write(&finder, b"{}").unwrap();
    assert_eq!(s.images.check_now(s.id()).unwrap(), None);
    std::fs::remove_file(&finder).unwrap();
    let recorded = s.images.check_now(s.id()).unwrap().unwrap();
    assert!(recorded.changed);
    assert_eq!(s.labels(), [OUTSIDE_LABEL, "Created"]);
}
