//! The image service end to end, on temporary server folders and stores. No server is
//! started and nothing is downloaded.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use lodestone_image::store::Store;
use lodestone_image::{Entry, Game, Image, Kind, Loader, Source};
use tokio::sync::watch;

use super::*;
use crate::implementations::minecraft::{FabricLoaderVersion, Flavour};
use crate::test_util::wait_until;

const UUID: &str = "INSTANCE_11111111-2222-3333-4444-555555555555";

fn uuid() -> InstanceUuid {
    InstanceUuid::from(UUID.to_string())
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = lodestone_image::layout::join_disk(root, relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn fabric() -> Flavour {
    Flavour::Fabric {
        loader_version: Some(FabricLoaderVersion::new("0.16.10".into())),
        installer_version: None,
    }
}

fn paper() -> Flavour {
    serde_json::from_value(serde_json::json!({"paper": {"build_version": 100}})).unwrap()
}

fn context(root: &Path, flavour: Flavour) -> InstanceContext {
    InstanceContext {
        uuid: uuid(),
        root: root.to_path_buf(),
        game_version: "1.21.1".into(),
        flavour,
        level_name: "world".into(),
        policy_exclude: Vec::new(),
        running: false,
    }
}

/// A Fabric server with a bit of everything.
fn fabric_server(root: &Path) {
    write(root, "server.properties", b"level-name=world\n");
    write(root, "eula.txt", b"eula=false\n");
    write(root, "mods/a.jar", b"mod a");
    write(
        root,
        "mods/.lodestone_client_mods.json",
        br#"{"required":["a.jar"]}"#,
    );
    write(root, "config/a.toml", b"a = 1");
    write(root, "world/level.dat", b"level");
    write(root, "world/region/r.0.0.mca", b"region");
    write(root, "logs/latest.log", b"log");
    write(root, "usercache.json", b"[]");
    write(root, ".lodestone_minecraft_config.json", b"{}");
}

struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    images: Arc<ImageService>,
}

fn setup() -> Setup {
    setup_with(Arc::new(SystemClock))
}

fn setup_with(clock: Arc<dyn Clock>) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    let images = Arc::new(ImageService::with_clock(dir.path().join("backups"), clock));
    Setup {
        _dir: dir,
        root,
        images,
    }
}

impl Setup {
    fn manifests(&self) -> Vec<Image> {
        self.images.store().list_manifests(UUID).unwrap().images
    }

    fn state(&self) -> state::ContentState {
        state::load(self.images.root(), &uuid())
    }
}

fn paths(image: &Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

/// A clock tests move by hand.
struct ManualClock {
    base: Instant,
    offset: watch::Sender<Duration>,
}

impl ManualClock {
    fn new() -> Arc<ManualClock> {
        Arc::new(ManualClock {
            base: Instant::now(),
            offset: watch::channel(Duration::ZERO).0,
        })
    }

    fn advance(&self, by: Duration) {
        self.offset.send_modify(|offset| *offset += by);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + *self.offset.borrow()
    }

    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        let mut offset = self.offset.subscribe();
        let base = self.base;
        Box::pin(async move {
            loop {
                if base + *offset.borrow_and_update() >= deadline {
                    return;
                }
                if offset.changed().await.is_err() {
                    return;
                }
            }
        })
    }
}

/// Servers by uuid, for merged records.
struct Fixed(StdMutex<HashMap<InstanceUuid, InstanceContext>>);

#[async_trait]
impl InstanceSource for Fixed {
    async fn context(&self, uuid: &InstanceUuid) -> Option<InstanceContext> {
        self.0.lock().unwrap().get(uuid).cloned()
    }
}

fn fixed(ctx: &InstanceContext) -> Arc<Fixed> {
    Arc::new(Fixed(StdMutex::new(HashMap::from([(
        ctx.uuid.clone(),
        ctx.clone(),
    )]))))
}

#[tokio::test]
async fn a_record_takes_only_content_and_saves_the_pointer_last() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    let recorded = s
        .images
        .record_now(ctx.clone(), "Added A".into())
        .await
        .unwrap()
        .unwrap();
    assert!(recorded.changed);
    assert_eq!(recorded.parent, None);
    let manifests = s.manifests();
    assert_eq!(manifests.len(), 1);
    let image = &manifests[0];
    assert_eq!(image.id, recorded.id);
    assert_eq!(image.kind, Kind::Content);
    assert_eq!(image.label.as_deref(), Some("Added A"));
    assert_eq!(
        paths(image),
        vec!["config/a.toml", "mods/a.jar", "server.properties"]
    );
    assert_eq!(image.game.loader, Loader::Fabric);
    assert_eq!(image.game.loader_version.as_deref(), Some("0.16.10"));
    assert!(image.worlds.is_empty());
    // The "For players" mark is the jar's client env, not a file of the image.
    let jar = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/a.jar")
        .unwrap();
    assert_eq!(jar.env.client, lodestone_image::EnvValue::Required);
    assert_eq!(jar.env_from, vec![lodestone_image::EnvFrom::OwnerMark]);
    // The pointer and baseline name what was saved.
    let state = s.state();
    assert_eq!(state.current.as_deref(), Some(recorded.id.as_str()));
    assert!(state.has_baseline());
    assert_eq!(
        state.baseline.keys().collect::<Vec<_>>(),
        vec!["config/a.toml", "mods/a.jar", "server.properties"]
    );
    assert_eq!(
        s.images.current_content(&uuid()).await,
        Some(recorded.id.clone())
    );
    // Its own stat cache, apart from the backups'.
    let dir = s.images.store().instance_dir(UUID).unwrap();
    assert!(dir.join("stat-cache.content.json").is_file());
    assert!(!dir.join("stat-cache.json").exists());

    // Recording again with nothing changed writes no image.
    let again = s
        .images
        .record_now(ctx.clone(), "Nothing".into())
        .await
        .unwrap()
        .unwrap();
    assert!(!again.changed);
    assert_eq!(again.id, recorded.id);
    assert_eq!(s.manifests().len(), 1);

    // A change gives a new image whose parent is the last.
    write(&s.root, "mods/b.jar", b"mod b");
    let next = s
        .images
        .record_now(ctx, "Added B".into())
        .await
        .unwrap()
        .unwrap();
    assert!(next.changed);
    assert_eq!(next.parent.as_deref(), Some(recorded.id.as_str()));
    assert_eq!(s.manifests().len(), 2);
}

#[tokio::test]
async fn a_failed_operation_records_nothing() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    let operation = s.images.begin(&uuid(), "Adding B").await;
    write(&s.root, "mods/b.jar", b"half");
    drop(operation);
    assert_eq!(s.images.flush(ctx).await.unwrap(), None);
    assert!(s.manifests().is_empty());
    assert_eq!(s.state().current, None);
}

#[tokio::test]
async fn instances_without_images_are_ignored() {
    let s = setup();
    for id in ["DOCKER-abc", "", "a/b", "CON"] {
        let mut ctx = context(&s.root, fabric());
        ctx.uuid = InstanceUuid::from(id.to_string());
        assert!(!ImageService::accepts(&ctx.uuid));
        let op = s.images.begin(&ctx.uuid, "x").await;
        op.succeed();
        assert_eq!(
            s.images.record_now(ctx.clone(), "x".into()).await.unwrap(),
            None
        );
        assert_eq!(s.images.before_start(ctx).await.unwrap(), None);
    }
    assert!(!s.images.root().join("images").exists());
}

#[tokio::test]
async fn successes_within_the_window_are_one_image() {
    let clock = ManualClock::new();
    let s = setup_with(clock.clone());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.set_instances(fixed(&ctx));
    for (n, label) in ["Added B", "Added C", "Removed A"].iter().enumerate() {
        let operation = s.images.begin(&uuid(), *label).await;
        write(&s.root, &format!("mods/{n}.jar"), label.as_bytes());
        operation.succeed();
        clock.advance(Duration::from_secs(1));
    }
    // A failed operation in between adds nothing.
    drop(s.images.begin(&uuid(), "Failed").await);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(s.manifests().is_empty(), "nothing before the window closes");
    clock.advance(MERGE_WINDOW);
    wait_until("the merged image is saved", || async {
        s.manifests().len() == 1
    })
    .await;
    let image = &s.manifests()[0];
    assert_eq!(image.label.as_deref(), Some("Added B; Added C; Removed A"));
    assert!(paths(image).contains(&"mods/2.jar"));
    wait_until("the worker ends", || async {
        !s.images.slot(&uuid()).pending.lock().unwrap().worker
    })
    .await;
}

#[tokio::test]
async fn a_busy_instance_records_by_the_deferral_cap() {
    let clock = ManualClock::new();
    let s = setup_with(clock.clone());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.set_instances(fixed(&ctx));
    // One success every two seconds never leaves the window quiet.
    for n in 0..15 {
        let operation = s.images.begin(&uuid(), format!("Edit {n}")).await;
        write(&s.root, "config/a.toml", format!("a = {n}").as_bytes());
        operation.succeed();
        clock.advance(Duration::from_secs(2));
        tokio::task::yield_now().await;
    }
    // Now 30 s after the first: due by the cap, though the last came 2 s ago.
    wait_until("the capped image is saved", || async {
        s.manifests().len() == 1
    })
    .await;
    let label = s.manifests()[0].label.clone().unwrap();
    assert!(label.starts_with("Edit 0; Edit 1"), "{label}");
    assert!(label.ends_with("Edit 14"), "{label}");
}

#[tokio::test]
async fn an_operation_that_starts_during_a_record_waits() {
    let s = setup();
    fabric_server(&s.root);
    let slot = s.images.slot(&uuid());
    // A record holds the gate alone.
    let record = slot.gate.clone().write_owned().await;
    let images = s.images.clone();
    let begun = tokio::spawn(async move { images.begin(&uuid(), "x").await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!begun.is_finished());
    drop(record);
    drop(begun.await.unwrap());

    // And a record waits for an operation in progress, then takes its label.
    let operation = s.images.begin(&uuid(), "Added A").await;
    let images = s.images.clone();
    let ctx = context(&s.root, fabric());
    let recording = tokio::spawn(async move { images.record_now(ctx, "Restored".into()).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!recording.is_finished());
    operation.succeed();
    let recorded = recording.await.unwrap().unwrap().unwrap();
    assert!(recorded.changed);
    assert_eq!(s.manifests()[0].label.as_deref(), Some("Added A"));
}

#[tokio::test]
async fn succeed_and_record_returns_the_image() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    let operation = s.images.begin(&uuid(), "Updated the pack").await;
    write(&s.root, "mods/pack.jar", b"pack mod");
    let recorded = operation.succeed_and_record(ctx).await.unwrap().unwrap();
    assert_eq!(s.state().current, Some(recorded.id));
    assert_eq!(s.manifests()[0].label.as_deref(), Some("Updated the pack"));
}

#[tokio::test]
async fn a_paper_content_image_skips_its_worlds() {
    let s = setup();
    let root = &s.root;
    write(root, "server.properties", b"level-name=world\n");
    write(root, "paper.jar", b"paper");
    write(root, "bukkit.yml", b"settings: {}");
    write(root, "plugins/WorldEdit.jar", b"we");
    write(root, "plugins/Essentials/config.yml", b"x: 1");
    write(root, "plugins/Essentials/userdata/player.yml", b"money: 5");
    write(root, "world/level.dat", b"overworld");
    write(root, "world/DIM-1/region/r.0.0.mca", b"stale");
    write(root, "world_nether/level.dat", b"nether");
    write(
        root,
        "world_nether/DIM-1/region/r.0.0.mca",
        b"nether region",
    );
    write(root, "world_the_end/DIM1/region/r.0.0.mca", b"end region");
    write(root, "creative/level.dat", b"creative");
    let ctx = context(root, paper());
    s.images
        .record_now(ctx, INITIAL.into())
        .await
        .unwrap()
        .unwrap();
    let image = &s.manifests()[0];
    assert_eq!(
        paths(image),
        vec![
            "bukkit.yml",
            "paper.jar",
            "plugins/Essentials/config.yml",
            "plugins/WorldEdit.jar",
            "server.properties",
        ]
    );
    assert!(image.withheld.is_empty(), "{:?}", image.withheld);
    assert_eq!(image.game.loader, Loader::Vanilla);
    let flavour = image
        .settings
        .server
        .as_ref()
        .and_then(|s| s.flavour.clone());
    assert_eq!(
        flavour.map(|f| f.as_str().to_string()),
        Some("paper".into())
    );
}

const INITIAL: &str = recorder::INITIAL_LABEL;

fn set_mtime(path: &Path, when: SystemTime) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(when).unwrap();
}

#[tokio::test]
async fn the_quick_check_sees_changes_and_absorbs_what_a_server_writes() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    // No image yet: the check skips (the first image is taken elsewhere).
    assert_eq!(s.images.before_start(ctx.clone()).await.unwrap(), None);
    assert!(s.manifests().is_empty());
    s.images
        .record_now(ctx.clone(), INITIAL.into())
        .await
        .unwrap();
    let store = s.images.store();
    let changed = |store: &Store| {
        let (layout, excludes) = recorder::content_scope(&ctx).unwrap();
        check::quick_changed(store, &s.root, &layout, &excludes, &s.state().baseline).unwrap()
    };
    assert!(!changed(&store));
    // Worlds, logs and Lodestone's own files aren't content.
    write(&s.root, "world/region/r.0.0.mca", b"more region");
    write(&s.root, "logs/latest.log", b"more log");
    write(&s.root, "mods/.lodestone_client_mods.json", b"{}");
    write(&s.root, "usercache.json", b"[1]");
    assert!(!changed(&store));
    // A touched file (same size), an added, a resized and a removed one are seen.
    let a = s.root.join("mods/a.jar");
    set_mtime(&a, SystemTime::now() - Duration::from_secs(3600));
    assert!(changed(&store));
    s.images.server_stopped(ctx.clone()).await.unwrap();
    assert!(!changed(&store));
    write(&s.root, "mods/new.jar", b"new");
    assert!(changed(&store));
    std::fs::remove_file(s.root.join("mods/new.jar")).unwrap();
    assert!(!changed(&store));
    write(&s.root, "config/a.toml", b"a = 1000");
    assert!(changed(&store));
    std::fs::remove_file(s.root.join("config/a.toml")).unwrap();
    assert!(changed(&store));
    write(&s.root, "config/a.toml", b"a = 1");

    // A start refused before the state left Stopped leaves nothing noted, and a second
    // note of a run already noted isn't the start's to take back.
    assert!(s.images.server_started(&uuid()).await);
    assert!(!s.images.server_started(&uuid()).await);
    s.images.start_refused(&uuid()).await;
    assert_eq!(s.state().running_since, None);

    // What a running server writes joins the baseline when it stops, without an image.
    let before = s.manifests().len();
    assert!(s.images.server_started(&uuid()).await);
    assert!(s.state().running_since.is_some());
    write(&s.root, "config/a.toml", b"a = 2 # rewritten by the mod");
    s.images.server_stopped(ctx.clone()).await.unwrap();
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.manifests().len(), before);
    assert_eq!(s.images.before_start(ctx.clone()).await.unwrap(), None);

    // A mod dropped in by hand is recorded before the start.
    write(&s.root, "mods/by-hand.jar", b"sftp");
    let recorded = s.images.before_start(ctx.clone()).await.unwrap().unwrap();
    assert!(recorded.changed);
    let newest = s
        .manifests()
        .into_iter()
        .find(|image| image.id == recorded.id)
        .unwrap();
    assert_eq!(newest.label.as_deref(), Some(recorder::OUTSIDE_LABEL));
    assert!(paths(&newest).contains(&"mods/by-hand.jar"));
    // And then the check is quiet again.
    assert_eq!(s.images.before_start(ctx).await.unwrap(), None);
}

#[tokio::test]
async fn new_backup_excludes_are_their_own_change_not_one_outside_lodestone() {
    let s = setup();
    fabric_server(&s.root);
    write(&s.root, "config/dynmap/a.txt", b"map");
    let ctx = context(&s.root, fabric());
    s.images
        .record_now(ctx.clone(), "one".into())
        .await
        .unwrap();
    assert_eq!(s.images.before_start(ctx.clone()).await.unwrap(), None);

    // The policy now leaves config/dynmap/ out; the server ran and stopped meanwhile, which
    // takes the baseline again, under the excludes of the current image.
    let excluding = InstanceContext {
        policy_exclude: vec!["config/dynmap/".into()],
        ..ctx.clone()
    };
    s.images.server_stopped(excluding.clone()).await.unwrap();
    let recorded = s
        .images
        .before_start(excluding.clone())
        .await
        .unwrap()
        .unwrap();
    let image = s
        .manifests()
        .into_iter()
        .find(|i| i.id == recorded.id)
        .unwrap();
    assert_eq!(image.label.as_deref(), Some(recorder::EXCLUDES_LABEL));
    assert!(!paths(&image).contains(&"config/dynmap/a.txt"));
    assert_eq!(s.state().excludes, image.excludes);
    assert_eq!(
        s.images.before_start(excluding.clone()).await.unwrap(),
        None
    );

    // A file changed by hand as well is a change outside Lodestone.
    write(&s.root, "mods/by-hand.jar", b"sftp");
    let recorded = s.images.before_start(ctx).await.unwrap().unwrap();
    let image = s
        .manifests()
        .into_iter()
        .find(|i| i.id == recorded.id)
        .unwrap();
    assert_eq!(image.label.as_deref(), Some(recorder::OUTSIDE_LABEL));
}

#[tokio::test]
async fn a_job_whose_files_another_record_took_adds_its_label_to_that_image() {
    let s = setup_with(ManualClock::new());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images
        .record_now(ctx.clone(), "one".into())
        .await
        .unwrap();

    // An install (a job, holding no gate) has placed its jar but not yet succeeded when a
    // record of an edit runs and takes the jar in.
    let install = s.images.begin_job(&uuid(), "Added JEI");
    write(&s.root, "mods/jei.jar", b"jei");
    write(&s.root, "config/a.toml", b"a = 2");
    s.images
        .begin(&uuid(), "Edited config/a.toml")
        .await
        .succeed();
    let edit = s.images.flush(ctx.clone()).await.unwrap().unwrap();
    assert!(edit.changed);

    // The install's own record finds nothing changed and labels that image instead.
    install.succeed();
    let again = s.images.flush(ctx.clone()).await.unwrap().unwrap();
    assert!(!again.changed);
    assert_eq!(again.id, edit.id);
    let label = |id: &str| {
        s.manifests()
            .into_iter()
            .find(|i| i.id == id)
            .unwrap()
            .label
    };
    assert_eq!(
        label(&edit.id).as_deref(),
        Some("Edited config/a.toml; Added JEI")
    );
    s.images.store().load_manifest(UUID, &edit.id).unwrap();

    // An operation that holds the gate and changed nothing labels nothing.
    s.images.begin(&uuid(), "Saved nothing").await.succeed();
    s.images.flush(ctx).await.unwrap();
    assert_eq!(
        label(&edit.id).as_deref(),
        Some("Edited config/a.toml; Added JEI")
    );
}

#[tokio::test]
async fn a_job_that_changed_no_content_labels_nothing() {
    let s = setup_with(ManualClock::new());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    let created = s
        .images
        .record_now(ctx.clone(), "Created".into())
        .await
        .unwrap()
        .unwrap();
    let label = |id: &str| {
        s.manifests()
            .into_iter()
            .find(|i| i.id == id)
            .unwrap()
            .label
    };

    // An unzip that wrote only world files: no new image was recorded since it began, so
    // its record finds nothing changed and drops its label.
    let unzip = s.images.begin_job(&uuid(), "Unzipped world.zip");
    write(&s.root, "world/region/r.0.0.mca", b"unzipped region");
    unzip.succeed();
    let again = s.images.flush(ctx.clone()).await.unwrap().unwrap();
    assert!(!again.changed);
    assert_eq!(again.id, created.id);
    assert_eq!(label(&created.id).as_deref(), Some("Created"));

    // A copy into a folder the policy excludes changes no content either.
    let mut excluding = ctx.clone();
    excluding.policy_exclude = vec!["config/dynmap/".into()];
    let excluded = s
        .images
        .record_now(excluding.clone(), "Excluded dynmap".into())
        .await
        .unwrap()
        .unwrap();
    assert!(excluded.changed);
    let copy = s.images.begin_job(&uuid(), "Copied tiles to config/dynmap");
    write(&s.root, "config/dynmap/tiles/a.png", b"tile");
    copy.succeed();
    let again = s.images.flush(excluding).await.unwrap().unwrap();
    assert!(!again.changed);
    assert_eq!(label(&excluded.id).as_deref(), Some("Excluded dynmap"));
}

#[tokio::test]
async fn a_job_that_changed_no_content_labels_nothing_while_other_changes_record() {
    let s = setup_with(ManualClock::new());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images
        .record_now(ctx.clone(), "Created".into())
        .await
        .unwrap();
    let label = |id: &str| {
        s.manifests()
            .into_iter()
            .find(|i| i.id == id)
            .unwrap()
            .label
    };

    // An unzip into the server folder runs while an edit of server.properties records an
    // image. The unzip placed only world files: its label is dropped, not added to the
    // edit's image.
    let mut unzip = s.images.begin_job(&uuid(), "Unzipped world.zip");
    write(&s.root, "server.properties", b"level-name=world\nmotd=hi\n");
    let edit = s
        .images
        .begin(&uuid(), "Edited server.properties")
        .await
        .succeed_and_record(ctx.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(edit.changed);
    write(&s.root, "world/region/r.0.0.mca", b"unzipped region");
    unzip.wrote(vec!["world".into()]);
    unzip.succeed();
    let again = s.images.flush(ctx.clone()).await.unwrap().unwrap();
    assert!(!again.changed);
    assert_eq!(again.id, edit.id);
    assert_eq!(label(&edit.id).as_deref(), Some("Edited server.properties"));

    // An install that only wrote the bytes the jar already had changed nothing either.
    let mut install = s.images.begin_job(&uuid(), "Updated A");
    install.wrote(vec!["mods".into()]);
    write(&s.root, "config/a.toml", b"a = 3");
    let config = s
        .images
        .begin(&uuid(), "Edited config/a.toml")
        .await
        .succeed_and_record(ctx.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(config.changed);
    write(&s.root, "mods/a.jar", b"mod a");
    install.succeed();
    let again = s.images.flush(ctx.clone()).await.unwrap().unwrap();
    assert_eq!(again.id, config.id);
    assert_eq!(label(&config.id).as_deref(), Some("Edited config/a.toml"));

    // One whose files that record took in still adds its label.
    let mut install = s.images.begin_job(&uuid(), "Added JEI");
    install.wrote(vec!["mods".into()]);
    write(&s.root, "mods/jei.jar", b"jei");
    write(&s.root, "config/a.toml", b"a = 4");
    let config = s
        .images
        .begin(&uuid(), "Edited config/a.toml")
        .await
        .succeed_and_record(ctx.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(config.changed);
    install.succeed();
    let again = s.images.flush(ctx).await.unwrap().unwrap();
    assert_eq!(again.id, config.id);
    assert_eq!(
        label(&config.id).as_deref(),
        Some("Edited config/a.toml; Added JEI")
    );
}

#[test]
fn excludes_that_reach_no_content_never_push_out_one_that_does() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    fabric_server(&root);
    let mut ctx = context(&root, fabric());
    // More world patterns than an image's search budget holds, then a content pattern.
    let mut policy: Vec<String> = (0..64).map(|n| format!("world/*tmp{n}*/")).collect();
    policy.push("config/*secret*".into());
    ctx.policy_exclude = policy.clone();
    let (_, before) = recorder::content_scope(&ctx).unwrap();
    assert_eq!(before, vec!["config/*secret*".to_string()]);
    // Removing a world pattern changes nothing.
    policy.remove(0);
    ctx.policy_exclude = policy;
    let (_, after) = recorder::content_scope(&ctx).unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn excludes_that_reach_no_content_change_no_content_image() {
    let s = setup();
    fabric_server(&s.root);
    let mut ctx = context(&s.root, fabric());
    let first = s
        .images
        .record_now(ctx.clone(), INITIAL.into())
        .await
        .unwrap()
        .unwrap();

    // World and log excludes can't match a content file: no image, the same id.
    ctx.policy_exclude = vec!["world/playerdata/".into(), "/logs/".into()];
    assert_eq!(s.images.before_start(ctx.clone()).await.unwrap(), None);
    assert_eq!(s.manifests().len(), 1);
    assert!(s.state().excludes.is_empty());
    let again = s
        .images
        .record_now(ctx.clone(), "Again".into())
        .await
        .unwrap()
        .unwrap();
    assert!(!again.changed);
    assert_eq!(again.id, first.id);
    assert!(s
        .images
        .store()
        .load_manifest(UUID, &first.id)
        .unwrap()
        .excludes
        .is_empty());

    // One that can is recorded, and only it is kept.
    ctx.policy_exclude.push("config/*.txt".into());
    let recorded = s.images.before_start(ctx.clone()).await.unwrap().unwrap();
    assert!(recorded.changed);
    let image = s.images.store().load_manifest(UUID, &recorded.id).unwrap();
    assert_eq!(image.label.as_deref(), Some(recorder::EXCLUDES_LABEL));
    assert_eq!(image.excludes, ["config/*.txt"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shutdown_waits_for_a_record_already_running() {
    // The clock never moves, so only the flush below records.
    let s = setup_with(ManualClock::new());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.set_instances(fixed(&ctx));
    s.images
        .record_now(ctx.clone(), "one".into())
        .await
        .unwrap();
    // Enough files that the record takes a while.
    for i in 0..2000 {
        write(
            &s.root,
            &format!("mods/m{i}.jar"),
            format!("mod {i}").as_bytes(),
        );
    }
    s.images.begin(&uuid(), "Added many").await.succeed();
    let record = tokio::spawn({
        let images = s.images.clone();
        let ctx = ctx.clone();
        async move { images.flush(ctx).await }
    });
    // The record has taken the labels off the queue (it may be running still).
    wait_until("the record takes the labels", || async {
        s.images.slot(&uuid()).pending.lock().unwrap().is_empty()
    })
    .await;
    s.images.settle(vec![uuid()], Duration::from_secs(60)).await;
    // The shutdown waited for it: its image is the current one.
    let current = s.state().current.unwrap();
    let image = s.images.store().load_manifest(UUID, &current).unwrap();
    assert_eq!(image.label.as_deref(), Some("Added many"));
    assert!(record.await.unwrap().unwrap().unwrap().changed);
}

#[tokio::test]
async fn the_shutdown_waits_for_stop_work_and_records_what_is_queued() {
    // The clock never moves, so the merge window never closes on its own.
    let s = setup_with(ManualClock::new());
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.set_instances(fixed(&ctx));
    s.images
        .record_now(ctx.clone(), "one".into())
        .await
        .unwrap();

    // The server ran and is stopping: its stop's work runs in the background, late.
    assert!(s.images.server_started(&uuid()).await);
    write(&s.root, "config/a.toml", b"rewritten while running");
    let pending = s.images.begin_stop(&uuid());
    let (images, stop_ctx) = (s.images.clone(), ctx.clone());
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        images.server_stopped(stop_ctx).await.unwrap();
        drop(pending);
    });
    // An edit made just before the shutdown waits in the merge window.
    write(&s.root, "mods/b.jar", b"b");
    s.images.begin(&uuid(), "Added B").await.succeed();

    s.images
        .clone()
        .settle(vec![uuid()], Duration::from_secs(10))
        .await;
    assert_eq!(s.state().running_since, None);
    let current = s.state().current.unwrap();
    let image = s.manifests().into_iter().find(|i| i.id == current).unwrap();
    assert_eq!(image.label.as_deref(), Some("Added B"));
    assert!(paths(&image).contains(&"mods/b.jar"));

    // A change made while Lodestone was down is recorded before the next start, not
    // folded into the baseline.
    write(&s.root, "mods/by-hand.jar", b"sftp");
    let recorded = s.images.before_start(ctx).await.unwrap().unwrap();
    let newest = s
        .manifests()
        .into_iter()
        .find(|i| i.id == recorded.id)
        .unwrap();
    assert_eq!(newest.label.as_deref(), Some(recorder::OUTSIDE_LABEL));
}

#[tokio::test]
async fn a_server_without_an_image_gets_its_first_when_it_stops_or_at_startup() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.server_stopped(ctx.clone()).await.unwrap();
    let manifests = s.manifests();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].label.as_deref(), Some(INITIAL));

    // A core that died while the server ran retakes the baseline at its next start.
    s.images.server_started(&uuid()).await;
    write(&s.root, "config/a.toml", b"rewritten while running");
    s.images.set_instances(fixed(&ctx));
    s.images.clone().startup_sweep(vec![uuid()]).await;
    assert_eq!(s.state().running_since, None);
    assert_eq!(s.manifests().len(), 1);
    assert_eq!(s.images.before_start(ctx).await.unwrap(), None);

    // A stopped server without an image gets one at startup.
    let other = setup();
    fabric_server(&other.root);
    other
        .images
        .set_instances(fixed(&context(&other.root, fabric())));
    other.images.clone().startup_sweep(vec![uuid()]).await;
    assert_eq!(other.manifests().len(), 1);
}

#[tokio::test]
async fn the_pointer_survives_a_crash_and_damage() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    let first = s
        .images
        .record_now(ctx.clone(), "one".into())
        .await
        .unwrap()
        .unwrap();
    write(&s.root, "mods/b.jar", b"b");
    let second = s
        .images
        .record_now(ctx.clone(), "two".into())
        .await
        .unwrap()
        .unwrap();
    // A crash after the manifest was saved but before the pointer moved.
    let mut stale = s.state();
    stale.current = Some(first.id.clone());
    state::save(s.images.root(), &uuid(), &stale).unwrap();
    let again = s
        .images
        .record_now(ctx.clone(), "three".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.id, second.id);
    assert!(again.changed);
    assert_eq!(again.parent.as_deref(), Some(first.id.as_str()));
    assert_eq!(s.state().current.as_deref(), Some(second.id.as_str()));
    assert_eq!(s.manifests().len(), 2);

    // A damaged state is no state: recording starts a new pointer.
    std::fs::write(state::path(s.images.root(), &uuid()).unwrap(), b"{garbage").unwrap();
    let after = s
        .images
        .record_now(ctx.clone(), "four".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.id, second.id);
    assert_eq!(s.state().current.as_deref(), Some(second.id.as_str()));

    // A pointer whose manifest went missing is saved again.
    let path = s.images.store().manifest_path(UUID, &second.id).unwrap();
    std::fs::remove_file(path).unwrap();
    let saved = s
        .images
        .record_now(ctx, "five".into())
        .await
        .unwrap()
        .unwrap();
    assert!(saved.changed);
    assert!(s.manifests().iter().any(|image| image.id == second.id));
}

fn content_image(n: i64) -> Image {
    let mut image = Image::new(
        Kind::Content,
        Game {
            minecraft: format!("1.21.{n}"),
            loader: Loader::Fabric,
            loader_version: None,
        },
    );
    image.created = n;
    image.seal();
    image
}

#[tokio::test]
async fn prune_keeps_backups_and_what_is_named_linked_current_and_newest() {
    let s = setup();
    let store = s.images.store();
    let images: Vec<Image> = (0..35).map(content_image).collect();
    for image in &images {
        store.save_manifest(UUID, image).unwrap();
    }
    // Backups (data images), linked to the oldest content image and the fourth: pruning
    // never deletes a backup, whatever names it.
    let mut data = Image::new(Kind::Data, images[0].game.clone());
    data.content = Some(images[0].id.clone());
    data.created = 100;
    data.seal();
    store.save_manifest(UUID, &data).unwrap();
    let mut other = Image::new(Kind::Data, images[0].game.clone());
    other.content = Some(images[3].id.clone());
    other.label = None;
    other.created = 101;
    other.worlds.push(lodestone_image::World {
        name: "w".into(),
        level_name: None,
        data_version: None,
        game_version: None,
        size: None,
        last_played: None,
        quiesced: None,
    });
    other.seal();
    store.save_manifest(UUID, &other).unwrap();
    state::save(
        s.images.root(),
        &uuid(),
        &state::ContentState {
            current: Some(images[2].id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let restoring = s.images.keep_while_restoring(&uuid(), &images[1].id);
    assert_eq!(s.images.prune(&uuid()).await.unwrap(), 1);
    drop(restoring);
    let kept: HashSet<String> = s.manifests().into_iter().map(|i| i.id).collect();
    for n in [0, 1, 2, 3] {
        assert!(kept.contains(&images[n].id), "image {n}");
    }
    for (n, image) in images.iter().enumerate().skip(5) {
        assert!(kept.contains(&image.id), "image {n}");
    }
    assert!(kept.contains(&data.id));
    assert!(kept.contains(&other.id));
    assert!(!kept.contains(&images[4].id));

    // While a manifest can't be read (it may be a backup linking to anything), nothing is
    // deleted.
    let extra = content_image(-1);
    store.save_manifest(UUID, &extra).unwrap();
    let damaged = store.manifest_path(UUID, &"ab".repeat(32)).unwrap();
    std::fs::write(&damaged, b"{").unwrap();
    assert_eq!(s.images.prune(&uuid()).await.unwrap(), 0);
    assert!(s.manifests().iter().any(|i| i.id == extra.id));
    std::fs::remove_file(&damaged).unwrap();
    // the image the restore went back to is no longer kept either
    assert_eq!(s.images.prune(&uuid()).await.unwrap(), 2);
    let kept: HashSet<String> = s.manifests().into_iter().map(|i| i.id).collect();
    assert!(!kept.contains(&extra.id));
    assert!(!kept.contains(&images[1].id));
}

/// The current image's parent is kept whatever the clocks said: after a clock that ran
/// ahead went back, the image recorded before a modpack restore ranks below 30 images
/// dated in the future, and it holds the only copy of what the restore took off.
#[tokio::test]
async fn prune_keeps_the_current_images_parent_after_the_clock_went_back() {
    let s = setup();
    let store = s.images.store();
    // recorded while the clock was a day ahead
    for n in 0..KEEP_CONTENT as i64 {
        let mut image = content_image(n);
        image.created = 100_000 + n;
        image.seal();
        store.save_manifest(UUID, &image).unwrap();
    }
    let before = content_image(1000);
    store.save_manifest(UUID, &before).unwrap();
    let mut restored = content_image(1001);
    restored.parent = Some(before.id.clone());
    restored.seal();
    store.save_manifest(UUID, &restored).unwrap();
    state::save(
        s.images.root(),
        &uuid(),
        &state::ContentState {
            current: Some(restored.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.images.prune(&uuid()).await.unwrap(), 0);
    let kept: HashSet<String> = s.manifests().into_iter().map(|i| i.id).collect();
    assert!(kept.contains(&before.id));
    assert!(kept.contains(&restored.id));
}

/// Images recorded in the same second are ordered by lineage, and the current image comes
/// first even when going back to earlier content made the parents go round in a circle.
#[test]
fn newest_content_first_orders_one_second_by_lineage_and_current() {
    let with_parent = |n: i64, parent: Option<&Image>| {
        let mut image = content_image(n);
        image.created = 10;
        image.parent = parent.map(|p| p.id.clone());
        image
    };
    let a = with_parent(1, None);
    let b = with_parent(2, Some(&a));
    let c = with_parent(3, Some(&b));
    let mut older = content_image(4);
    older.created = 9;
    let ids = |images: &[Image], current: Option<&str>| -> Vec<String> {
        let headers: Vec<lodestone_image::store::ManifestHeader> = images
            .iter()
            .map(|i| lodestone_image::store::ManifestHeader {
                id: i.id.clone(),
                kind: i.kind,
                parent: i.parent.clone(),
                created: i.created,
                label: i.label.clone(),
                content: i.content.clone(),
            })
            .collect();
        retention::newest_content_first(&headers, current)
            .into_iter()
            .map(|i| i.id.clone())
            .collect()
    };
    let images = vec![older.clone(), a.clone(), c.clone(), b.clone()];
    assert_eq!(
        ids(&images, None),
        [c.id.clone(), b.id.clone(), a.id.clone(), older.id.clone()]
    );
    // Going back to `a` saved it again with `b` as its parent: a circle.
    let mut a_again = a.clone();
    a_again.parent = Some(b.id.clone());
    let images = vec![a_again.clone(), b.clone()];
    assert_eq!(ids(&images, Some(&a.id)), [a.id.clone(), b.id.clone()]);
    assert_eq!(ids(&images, Some(&b.id)), [b.id.clone(), a.id.clone()]);
    // A restore within one second: set `a`, then `b`, two records after it, then back to
    // `a`, saved again after the second record (a circle of four), then one more change.
    let r1 = with_parent(5, Some(&b));
    let r2 = with_parent(6, Some(&r1));
    let mut a_back = a.clone();
    a_back.parent = Some(r2.id.clone());
    let after = with_parent(7, Some(&a_back));
    let circle = vec![r1.clone(), a_back.clone(), b.clone(), r2.clone()];
    let newest_first = [a.id.clone(), r2.id.clone(), r1.id.clone(), b.id.clone()];
    for _ in 0..8 {
        assert_eq!(ids(&circle, Some(&a.id)), newest_first);
    }
    let mut all = circle.clone();
    all.push(after.clone());
    let mut expected = vec![after.id.clone()];
    expected.extend(newest_first.iter().cloned());
    assert_eq!(ids(&all, Some(&after.id)), expected);
    // An older second stays older, whatever is current.
    let images = vec![older.clone(), b.clone()];
    assert_eq!(
        ids(&images, Some(&older.id)),
        [b.id.clone(), older.id.clone()]
    );
}

/// Sets an object's modification time two hours back, for a collection's grace.
fn age(path: &Path) {
    let readonly = std::fs::metadata(path).unwrap().permissions().readonly();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    set_mtime(path, SystemTime::now() - Duration::from_secs(2 * 3600));
    #[cfg(unix)]
    if readonly {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444)).unwrap();
    }
    let _ = readonly;
}

fn age_objects(store: &Store) {
    for entry in walkdir::WalkDir::new(store.root().join("objects"))
        .min_depth(2)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            age(entry.path());
        }
    }
}

#[tokio::test]
async fn the_collector_keeps_what_images_name_and_deletes_the_rest() {
    let s = setup();
    let store = s.images.store();
    let unused = store.put_bytes(b"named by nothing").unwrap();
    let in_image = store.put_bytes(b"named by an image").unwrap();
    let mut image = content_image(1);
    image.entries.push(Entry::new(
        lodestone_image::CanonicalPath::new("mods/a.jar").unwrap(),
        in_image.size,
        in_image.sha256.clone(),
        Source::Object {},
    ));
    image.seal();
    store.save_manifest(UUID, &image).unwrap();
    age_objects(&store);

    let images = s.images.clone();
    let report = tokio::task::spawn_blocking(move || images.collect_blocking())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report.objects_removed, 1);
    assert!(store.has_object(&in_image.sha256, in_image.size));
    assert!(!store.has_object(&unused.sha256, unused.size));

    // A manifest it can't read stops it deleting anything.
    let unused = store.put_bytes(b"also named by nothing").unwrap();
    age_objects(&store);
    let unreadable = store.manifest_path(UUID, &"0".repeat(64)).unwrap();
    std::fs::write(unreadable, b"{garbage").unwrap();
    let images = s.images.clone();
    assert!(
        tokio::task::spawn_blocking(move || images.collect_blocking())
            .await
            .unwrap()
            .is_err()
    );
    assert!(store.has_object(&unused.sha256, unused.size));
}

/// Collections asked for while one runs make it run once more, never two at once, and
/// the last request is never lost.
#[tokio::test]
async fn requested_collections_run_in_the_background() {
    let s = setup();
    let store = s.images.store();
    let image = content_image(1);
    store.save_manifest(UUID, &image).unwrap();
    let unused = store.put_bytes(b"named by nothing").unwrap();
    age_objects(&store);
    for _ in 0..5 {
        s.images.request_gc();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.has_object(&unused.sha256, unused.size) {
        assert!(Instant::now() < deadline, "the collection never ran");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Asked for again once that one ended: another runs and takes what is unused now.
    while s
        .images
        .gc
        .running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let later = store.put_bytes(b"unused later").unwrap();
    age_objects(&store);
    s.images.request_gc();
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.has_object(&later.sha256, later.size) {
        assert!(Instant::now() < deadline, "the second collection never ran");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_collection_during_records_loses_nothing() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images
        .record_now(ctx.clone(), INITIAL.into())
        .await
        .unwrap();
    let store = s.images.store();
    for round in 0..5 {
        // Prune went too far: nothing names the objects the stat cache still knows, and
        // they are old enough to collect while the record relies on them.
        for image in s.manifests() {
            store.delete_manifest(UUID, &image.id).unwrap();
        }
        age_objects(&store);
        write(
            &s.root,
            &format!("mods/round-{round}.jar"),
            &[round as u8; 70_000],
        );
        let images = s.images.clone();
        let collecting = tokio::task::spawn_blocking(move || images.collect_blocking());
        let recorded = s
            .images
            .record_now(ctx.clone(), format!("round {round}"))
            .await
            .unwrap()
            .unwrap();
        collecting.await.unwrap().unwrap();
        let image = store.load_manifest(UUID, &recorded.id).unwrap();
        for entry in &image.entries {
            assert!(
                store.object_complete(&entry.sha256, entry.size),
                "round {round}: {} lost",
                entry.path
            );
        }
    }
}

#[tokio::test]
async fn deleting_an_instance_forgets_its_images() {
    let s = setup();
    fabric_server(&s.root);
    let ctx = context(&s.root, fabric());
    s.images.record_now(ctx, INITIAL.into()).await.unwrap();
    let dir = s.images.store().instance_dir(UUID).unwrap();
    assert!(dir.is_dir());
    s.images.delete_instance(&uuid()).await.unwrap();
    assert!(!dir.exists());
    assert_eq!(s.images.current_content(&uuid()).await, None);
    // Again is fine.
    s.images.delete_instance(&uuid()).await.unwrap();
}

#[tokio::test]
async fn pack_files_get_their_sources_and_env() {
    let s = setup();
    fabric_server(&s.root);
    write(&s.root, "mods/pack-mod.jar", b"from the pack");
    write(&s.root, "config/pack.toml", b"pack override");
    write(&s.root, "config/changed.toml", b"changed by the owner");
    let sha1 = |bytes: &[u8]| {
        use sha1::Digest;
        sha1::Sha1::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let sha512 = "e".repeat(128);
    let info = base::BaseInfo {
        base: Some(lodestone_image::Base::ModrinthPack {
            project: "pack".into(),
            version: "v1".into(),
            sha512: "f".repeat(128),
        }),
        files: BTreeMap::from([
            (
                "mods/pack-mod.jar".to_string(),
                base::BaseFile {
                    sha1: sha1(b"from the pack"),
                    env: Some(lodestone_image::Env::new(
                        lodestone_image::EnvValue::Unsupported,
                        lodestone_image::EnvValue::Required,
                    )),
                    modrinth_url: Some(
                        "https://cdn.modrinth.com/data/AAAA/versions/BBBB/pack-mod.jar".into(),
                    ),
                    sha512: Some(sha512.clone()),
                    ..Default::default()
                },
            ),
            (
                "config/pack.toml".to_string(),
                base::BaseFile {
                    sha1: sha1(b"pack override"),
                    ..Default::default()
                },
            ),
            (
                "config/changed.toml".to_string(),
                base::BaseFile {
                    sha1: sha1(b"the pack's version"),
                    ..Default::default()
                },
            ),
        ]),
    };
    base::save_base(s.images.root(), &uuid(), &info).unwrap();
    s.images
        .record_now(context(&s.root, fabric()), "Installed".into())
        .await
        .unwrap();
    let image = &s.manifests()[0];
    assert_eq!(image.base, info.base);
    let entry = |path: &str| {
        image
            .entries
            .iter()
            .find(|e| e.path.as_str() == path)
            .unwrap()
            .clone()
    };
    let pack_mod = entry("mods/pack-mod.jar");
    assert!(matches!(pack_mod.source, Source::Modrinth { .. }));
    assert_eq!(pack_mod.sha512.as_deref(), Some(sha512.as_str()));
    assert_eq!(pack_mod.env.client, lodestone_image::EnvValue::Unsupported);
    assert_eq!(pack_mod.env_from, vec![lodestone_image::EnvFrom::BasePack]);
    assert_eq!(entry("config/pack.toml").source, Source::Base {});
    assert_eq!(entry("config/changed.toml").source, Source::Object {});
    assert_eq!(entry("mods/a.jar").source, Source::Object {});
}

#[tokio::test]
async fn modrinth_hints_give_sources_and_env_and_outlive_restarts() {
    let s = setup();
    fabric_server(&s.root);
    let sha512 = crate::mods::scan::sha512_of(&s.root.join("mods/a.jar")).unwrap();
    s.images.note_modrinth(
        &sha512,
        ModrinthHint {
            url: Some("https://cdn.modrinth.com/data/AAAA/versions/BBBB/a.jar".into()),
            environment: Some("client_and_server".into()),
            client_side: None,
            server_side: None,
        },
    );
    let ctx = context(&s.root, fabric());
    let first = s
        .images
        .record_now(ctx.clone(), "Installed A".into())
        .await
        .unwrap()
        .unwrap();
    let image = &s.manifests()[0];
    let jar = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/a.jar")
        .unwrap();
    assert!(matches!(jar.source, Source::Modrinth { .. }));
    assert_eq!(jar.env.server, lodestone_image::EnvValue::Required);
    // A new core (no hints in memory) gives the same image from the saved state.
    let restarted = ImageService::new(s.images.root().to_path_buf());
    let again = Arc::new(restarted)
        .record_now(ctx, "Nothing".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.id, first.id);
    assert!(!again.changed);
}
