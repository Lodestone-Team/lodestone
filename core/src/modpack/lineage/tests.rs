//! The modpack lineage on temporary server folders and image stores: genuine `history.json`
//! and `record.json` written by the core before images (the shared crate's fixtures), and
//! updates and restores through the HTTP handlers with a fake Modrinth. No server is started
//! and nothing is downloaded from the network.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use lodestone_image::legacy::core_modpack;
use lodestone_image::{Base, Env, EnvFrom, EnvValue, Image, Kind, Source, WithheldReason};
use serde_json::json;
use tokio::sync::RwLock;

use super::*;
use crate::auth::user::User;
use crate::handlers::modpack_update::{
    modpack_history, update_modpack, ModpackUpdateRequest, UpdateSource,
};
use crate::handlers::util::AuthedUser;
use crate::implementations::minecraft::{FabricLoaderVersion, MinecraftInstance};
use crate::modpack::history::{ChangeKind, HISTORY_FILE};
use crate::modpack::tests::{file_entry, index, pack, Entry as ZipEntry, TestPack};
use crate::modpack::update::{
    pack_files, place_files, plan_files, remove_work_dirs, stage_files_from, PackRecord,
    RECORD_FILE,
};
use crate::modpack::{install_pack_contents, read_pack};
use crate::modrinth::fake::{sha1_hex, sha512_hex, FakeModrinth, FakeVersion};

const UUID: &str = "INSTANCE_aaaaaaaa-2222-3333-4444-555555555555";

fn uuid() -> InstanceUuid {
    InstanceUuid::from(UUID.to_string())
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn read(root: &Path, relative: &str) -> Option<Vec<u8>> {
    std::fs::read(root.join(relative)).ok()
}

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../crates/lodestone-image/tests/fixtures/legacy/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn fabric() -> Flavour {
    Flavour::Fabric {
        loader_version: Some(FabricLoaderVersion::new("0.15.11".into())),
        installer_version: None,
    }
}

fn context(root: &Path, flavour: Flavour) -> InstanceContext {
    InstanceContext {
        uuid: uuid(),
        root: root.to_path_buf(),
        game_version: "1.20.1".into(),
        flavour,
        level_name: "world".into(),
        policy_exclude: Vec::new(),
        running: false,
    }
}

/// An image service on a temporary store, and a server folder beside it.
struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    images: Arc<ImageService>,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("server");
    std::fs::create_dir_all(&root).unwrap();
    let images = Arc::new(ImageService::new(
        dir.path().join("backups"),
        Arc::new(RwLock::new(())),
    ));
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
}

/// The fixtures' server as the core left it: version 1 of the test pack installed, updated
/// to version 2 with `config/edited.toml` changed by the user, then restored to version 1.
/// The history's store has both configs it replaced.
fn fixture_server(root: &Path) {
    write(root, RECORD_FILE, &fixture("record.json"));
    write(root, HISTORY_FILE, &fixture("history.json"));
    write(root, "server.properties", b"level-name=world\n");
    write(root, "config/edited.toml", b"mine");
    write(root, "config/pack.toml", b"v1");
    write(root, "config/only-v2.toml", b"only in v2");
    write(root, "mods/a-1.jar", b"a1");
    write(root, "mods/dropped.jar", b"dropped");
    write(root, "mods/keep.jar", b"keep");
    write(root, "world/level.dat", b"level");
    for content in [&b"v1"[..], b"v2"] {
        let sha1 = sha1_hex(content);
        write(
            root,
            &format!(".lodestone_modpack/objects/{}/{sha1}", &sha1[..2]),
            content,
        );
    }
}

/// The History API's JSON of the server in `root`, as the handler builds it.
fn api_json(root: &Path, images: &ImageService) -> serde_json::Value {
    let history: History = serde_json::from_slice(&read(root, HISTORY_FILE).unwrap()).unwrap();
    let lookup = ImageLookup::new(images.store(), &uuid());
    serde_json::to_value(history.info_with(root, None, &lookup)).unwrap()
}

/// `value` without the `image` fields of its entries.
fn without_images(mut value: serde_json::Value) -> serde_json::Value {
    for entry in value["entries"].as_array_mut().unwrap() {
        entry.as_object_mut().unwrap().remove("image");
    }
    value
}

#[tokio::test]
async fn backfill_links_old_entries_and_changes_nothing_else() {
    let s = setup();
    fixture_server(&s.root);
    let before = api_json(&s.root, &s.images);
    assert!(before["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e.get("image").is_none()));

    let links = backfill(&s.images, &context(&s.root, fabric()), None).await;
    assert_eq!(links.keys().copied().collect::<Vec<_>>(), [1, 2, 3]);
    // Going back to entry 1 gives entry 1's image again: two manifests for three entries.
    assert_eq!(links[&1], links[&3]);
    assert_ne!(links[&1], links[&2]);
    let manifests = s.manifests();
    assert_eq!(manifests.len(), 2);
    for image in &manifests {
        assert_eq!(image.kind, Kind::Content);
        // Converted: what the pack wrote, env unknown.
        assert!(image.entries.iter().all(|e| e.env == Env::UNKNOWN));
        // Version 2's mods aren't on the server or in the history's store any more (they
        // can be downloaded again): withheld, never guessed.
        let withheld: Vec<&str> = image.withheld.iter().map(|w| w.path.as_str()).collect();
        if image.id == links[&2] {
            assert_eq!(withheld, ["mods/a-2.jar", "mods/new.jar"]);
            assert!(image
                .withheld
                .iter()
                .all(|w| w.reason == WithheldReason::Unreadable));
            // Its config the history's store kept is there.
            assert!(image
                .entries
                .iter()
                .any(|e| e.path.as_str() == "config/pack.toml"
                    && e.sha1.as_deref() == Some(sha1_hex(b"v2").as_str())));
        } else {
            assert!(withheld.is_empty(), "{withheld:?}");
        }
    }
    // Converted images never become the current content image.
    assert_eq!(s.images.current_content(&uuid()).await, None);

    // The API answers the same, but for the links.
    let after = api_json(&s.root, &s.images);
    assert_eq!(without_images(after.clone()), before);
    let images: Vec<&str> = after["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["image"].as_str().unwrap())
        .collect();
    assert_eq!(
        images,
        [links[&3].as_str(), links[&2].as_str(), links[&1].as_str()]
    );

    // A core from before images reads the new file, and writes back the original bytes.
    let new_file = read(&s.root, HISTORY_FILE).unwrap();
    let old_core = core_modpack::History::from_json(&new_file).unwrap();
    assert_eq!(old_core.to_json(), fixture("history.json"));
    // The record, the history's store and the files are untouched.
    assert_eq!(read(&s.root, RECORD_FILE).unwrap(), fixture("record.json"));
    assert_eq!(read(&s.root, "config/pack.toml").unwrap(), b"v1");
    assert!(object_path(&s.root, &sha1_hex(b"v2")).unwrap().is_file());

    // Once every entry has an image there is nothing more to do.
    assert!(backfill(&s.images, &context(&s.root, fabric()), None)
        .await
        .is_empty());
}

#[tokio::test]
async fn backfill_withholds_what_is_gone_and_still_saves_the_image() {
    let s = setup();
    fixture_server(&s.root);
    // The dropped mod was taken off the server since, and nothing kept it.
    std::fs::remove_file(s.root.join("mods/dropped.jar")).unwrap();
    let links = backfill(&s.images, &context(&s.root, fabric()), None).await;
    assert_eq!(links.len(), 3);
    let first = s.images.store().load_manifest(UUID, &links[&1]).unwrap();
    assert!(first
        .withheld
        .iter()
        .any(|w| w.path == "mods/dropped.jar" && w.reason == WithheldReason::Unreadable));
    assert!(first
        .entries
        .iter()
        .any(|e| e.path.as_str() == "mods/keep.jar"));
    // The history keeps the file, so nothing about it is lost.
    let history = History::load(&s.root).await;
    assert!(history.entries[0].files.contains_key("mods/dropped.jar"));
}

#[tokio::test]
async fn backfill_of_a_paper_server_withholds_world_overrides() {
    let s = setup();
    let paper: Flavour = serde_json::from_value(json!({"paper": {"build_version": 100}})).unwrap();
    // A pack override the core put in Paper's Nether folder: a world's file.
    write(&s.root, "world/level.dat", b"level");
    write(&s.root, "world_nether/level.dat", b"nether");
    write(&s.root, "world_nether/datapacks/x.zip", b"datapack");
    write(&s.root, "plugins/p.jar", b"plugin");
    let mut history = History::default();
    let record = PackRecord {
        files: BTreeMap::from([
            (
                "world_nether/datapacks/x.zip".to_string(),
                crate::modpack::update::RecordedFile {
                    sha1: sha1_hex(b"datapack"),
                    from_overrides: true,
                    project_id: None,
                },
            ),
            (
                "plugins/p.jar".to_string(),
                crate::modpack::update::RecordedFile {
                    sha1: sha1_hex(b"plugin"),
                    from_overrides: true,
                    project_id: None,
                },
            ),
        ]),
    };
    let entry = crate::modpack::history::entry_as_is(
        HistoryKind::Install,
        origin("1"),
        "1.21.1".into(),
        paper.clone(),
        &record,
    );
    history.push(&s.root, entry);
    history.save(&s.root).await.unwrap();

    let mut ctx = context(&s.root, paper);
    ctx.game_version = "1.21.1".into();
    let links = backfill(&s.images, &ctx, None).await;
    let image = s.images.store().load_manifest(UUID, &links[&1]).unwrap();
    assert!(image
        .withheld
        .iter()
        .any(|w| w.path == "world_nether/datapacks/x.zip" && w.reason == WithheldReason::Excluded));
    assert!(image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "plugins/p.jar"));
    assert!(image.entries.iter().all(|e| e.world.is_none()));
    // Not lost: the history still lists it, and the old engine restores it from there.
    let history = History::load(&s.root).await;
    assert!(history.entries[0]
        .files
        .contains_key("world_nether/datapacks/x.zip"));
    assert_eq!(
        history.entries[0].image.as_deref(),
        Some(links[&1].as_str())
    );
}

#[tokio::test]
async fn a_file_only_the_image_store_has_is_restored_and_checked() {
    let s = setup();
    fixture_server(&s.root);
    let fake = FakeModrinth::default();
    for (url, content) in [
        (
            "https://cdn.modrinth.com/data/a/versions/v2/a-2.jar",
            &b"a2"[..],
        ),
        (
            "https://cdn.modrinth.com/data/n/versions/v2/new.jar",
            b"new",
        ),
    ] {
        fake.add_file(url, content.to_vec());
    }
    backfill(&s.images, &context(&s.root, fabric()), None).await;
    // The history's stored copy of version 2's config is lost since.
    std::fs::remove_file(object_path(&s.root, &sha1_hex(b"v2")).unwrap()).unwrap();
    let history = History::load(&s.root).await;
    let entry = history.entry(2).unwrap().clone();

    // Without images, version 2 can't be restored; with them it can.
    assert!(history.info(&s.root, None).entries[1].missing > 0);
    let lookup = ImageLookup::new(s.images.store(), &uuid());
    assert_eq!(
        history.info_with(&s.root, None, &lookup).entries[1].missing,
        0
    );

    let old = PackRecord::load(&s.root).await.unwrap();
    let mut files = history.files_of(&s.root, 2, false).unwrap();
    let plan = plan_files(&s.root, &old, &files);
    fill_from_image(&lookup, &entry, &plan, &mut files);
    let config = &files["config/pack.toml"];
    assert!(config.stored.is_none() && config.download.is_none());
    assert!(config.image.is_some());
    stage_files_from(
        &fake,
        None,
        &files,
        &plan,
        &s.root,
        false,
        Some(s.images.store()),
        &|_| {},
    )
    .await
    .unwrap();
    place_files(&s.root, &plan).unwrap();
    remove_work_dirs(&s.root).await;
    assert_eq!(read(&s.root, "config/pack.toml").unwrap(), b"v2");
    assert_eq!(read(&s.root, "mods/a-2.jar").unwrap(), b"a2");

    // An object that isn't the file the history names is never staged.
    let mut wrong = files.clone();
    wrong.get_mut("config/pack.toml").unwrap().sha1 = sha1_hex(b"else");
    let elsewhere = tempfile::tempdir().unwrap();
    let error = stage_files_from(
        &fake,
        None,
        &wrong,
        &plan,
        elsewhere.path(),
        false,
        Some(s.images.store()),
        &|_| {},
    )
    .await;
    assert!(error.is_err());
    assert!(!elsewhere
        .path()
        .join(".lodestone_modpack/staging/config/pack.toml")
        .exists());
    // And a lookup by the wrong SHA-1 finds nothing.
    assert!(lookup
        .file(&entry, "config/pack.toml", &sha1_hex(b"else"))
        .is_none());
}

#[tokio::test]
async fn retention_keeps_the_images_the_history_names() {
    let s = setup();
    s.images
        .register_refs(Arc::new(LineageRefs::new(Arc::new(DashMap::new()))));
    let _creating = creating(&uuid(), &s.root);
    let store = s.images.store();
    let mut ids = Vec::new();
    for n in 0..40 {
        let mut image = Image::new(
            Kind::Content,
            lodestone_image::Game {
                minecraft: format!("1.{n}"),
                loader: lodestone_image::Loader::Vanilla,
                loader_version: None,
            },
        );
        image.created = 1_000 + n;
        image.seal();
        store.save_manifest(UUID, &image).unwrap();
        ids.push(image.id);
    }
    // The history names the oldest image.
    let mut history = History::default();
    let mut entry = crate::modpack::history::entry_as_is(
        HistoryKind::Install,
        origin("1"),
        "1.20.1".into(),
        fabric(),
        &PackRecord::default(),
    );
    entry.image = Some(ids[0].clone());
    history.push(&s.root, entry);
    history.save(&s.root).await.unwrap();

    let deleted = s.images.prune(&uuid()).await.unwrap();
    assert_eq!(deleted, 9);
    let kept: Vec<String> = s.manifests().into_iter().map(|i| i.id).collect();
    assert!(kept.contains(&ids[0]), "the history's image is kept");
    assert!(!kept.contains(&ids[1]));
    assert!(kept.contains(&ids[39]));

    // A history that can't be read keeps everything.
    write(&s.root, HISTORY_FILE, b"{damaged");
    assert_eq!(s.images.prune(&uuid()).await.unwrap(), 0);
}

#[test]
fn entries_without_images_are_written_as_before() {
    let entry = crate::modpack::history::entry_as_is(
        HistoryKind::Install,
        origin("1"),
        "1.20.1".into(),
        fabric(),
        &PackRecord::default(),
    );
    let json = serde_json::to_value(&entry).unwrap();
    assert!(json.get("image").is_none());
    let mut linked = entry;
    linked.image = Some("a".repeat(64));
    let json = serde_json::to_vec(&linked).unwrap();
    assert!(String::from_utf8_lossy(&json).contains(&"a".repeat(64)));
    // The old core's entry type reads it.
    let history = format!(
        "{{\"entries\":[{}],\"files\":{{}}}}",
        String::from_utf8(json).unwrap()
    );
    assert!(core_modpack::History::from_json(history.as_bytes()).is_ok());
}

#[test]
fn labels() {
    assert_eq!(
        label(&HistoryKind::Update, &origin("2")),
        "Updated Test Pack to 2"
    );
    assert_eq!(
        label(&HistoryKind::Restore { to: 1 }, &origin("1")),
        "Restored Test Pack to 1"
    );
    assert_eq!(
        label(&HistoryKind::Install, &origin("1")),
        "Installed Test Pack 1"
    );
    let mut unnumbered = origin("1");
    unnumbered.version_number = None;
    assert_eq!(
        label(&HistoryKind::Update, &unnumbered),
        "Updated Test Pack"
    );
}

fn origin(version: &str) -> ModpackOrigin {
    ModpackOrigin {
        project_id: "pack".into(),
        version_id: format!("v{version}"),
        name: "Test Pack".into(),
        version_number: Some(version.into()),
        icon_url: None,
    }
}

// ---- through the handlers ----

const CDN: &str = "https://cdn.modrinth.com/data";

/// Version `version` of a pack: mods from the CDN (version 2's `new.jar` is for servers only,
/// says the index) and configs in its overrides (version 2 has one in `server-overrides/`).
fn test_pack(version: u8, fake: &FakeModrinth) -> TestPack {
    let mut files = vec![];
    let mut add = |path: &str, content: &[u8], project: &str, env: Option<serde_json::Value>| {
        let url = format!(
            "{CDN}/{project}/versions/v{version}/{}",
            path.rsplit('/').next().unwrap()
        );
        let mut entry = file_entry(path, content, None, &url);
        if let Some(env) = env {
            entry["env"] = env;
        }
        files.push(entry);
        fake.add_file(&url, content.to_vec());
    };
    add("mods/keep.jar", b"keep", "k", None);
    let config: &[u8] = match version {
        1 => {
            add("mods/a-1.jar", b"a1", "a", None);
            add("mods/dropped.jar", b"dropped", "d", None);
            b"v1"
        }
        2 => {
            add("mods/a-2.jar", b"a2", "a", None);
            add(
                "mods/new.jar",
                b"new",
                "n",
                Some(json!({"client": "unsupported", "server": "required"})),
            );
            b"v2"
        }
        _ => {
            // a file no one can download: the update fails
            files.push(file_entry(
                "mods/gone.jar",
                b"gone",
                None,
                &format!("{CDN}/g/versions/v3/gone.jar"),
            ));
            b"v3"
        }
    };
    let mut entries = vec![
        ZipEntry::File("overrides/config/pack.toml", config),
        ZipEntry::File("overrides/config/edited.toml", config),
    ];
    if version == 2 {
        entries.push(ZipEntry::File(
            "overrides/config/only-v2.toml",
            b"only in v2",
        ));
        entries.push(ZipEntry::File(
            "server-overrides/config/server-only.toml",
            b"server only",
        ));
    }
    pack(
        index(
            files,
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &entries,
    )
}

/// A core with a fake Modrinth and one stopped Fabric server made from version 1 of the test
/// pack by a core from before images (its history has no image links).
struct Core {
    _dir: tempfile::TempDir,
    state: crate::AppState,
    fake: Arc<FakeModrinth>,
    uuid: InstanceUuid,
    root: PathBuf,
    _packs: Vec<TestPack>,
}

fn owner() -> AuthedUser {
    AuthedUser(User::new(
        "owner".to_string(),
        "pw",
        true,
        true,
        Default::default(),
    ))
}

impl Core {
    async fn new() -> Core {
        static LODESTONE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let lodestone = LODESTONE.get_or_init(|| tempfile::tempdir().unwrap());
        crate::prelude::try_init_paths(lodestone.path().to_owned()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let fake = Arc::new(FakeModrinth::default());
        state.mods = Arc::new(crate::mods::ModsState::new(
            fake.clone(),
            crate::modpack::store::PackStore::new(dir.path().join("modpacks")),
        ));
        let mut packs = Vec::new();
        for version in 1..=3u8 {
            let id = format!("v{version}");
            fake.add(FakeVersion::new("pack", &id, "pack.mrpack"));
            let test_pack = test_pack(version, &fake);
            let temp = state.mods.packs.new_file().unwrap();
            std::fs::copy(&test_pack.path, &temp).unwrap();
            let info = read_pack(&test_pack.path, false).unwrap().info;
            state.mods.packs.insert(temp, info, Some(id), None);
            packs.push(test_pack);
        }
        let project_title = crate::modrinth::ModrinthApi::project(fake.as_ref(), "pack")
            .await
            .unwrap()
            .title;
        let origin = |version: u8| ModpackOrigin {
            project_id: "pack".into(),
            version_id: format!("v{version}"),
            name: project_title.clone(),
            version_number: Some(format!("v{version}-number")),
            icon_url: None,
        };

        let root = dir.path().join("instance");
        std::fs::create_dir_all(&root).unwrap();
        // version 1 as a core from before images installed it
        let v1 = read_pack(&packs[0].path, false).unwrap();
        let report =
            install_pack_contents(fake.as_ref(), &v1, &packs[0].path, &root, false, &|_, _| {})
                .await
                .unwrap();
        let mut history = History::default();
        history.remember_downloads(&report.files);
        history.push(
            &root,
            crate::modpack::history::entry_as_is(
                HistoryKind::Install,
                origin(1),
                "1.20.1".into(),
                fabric(),
                &crate::modpack::update::record_of(&report.files),
            ),
        );
        history.save(&root).await.unwrap();
        std::fs::write(
            root.join(".lodestone_minecraft_config.json"),
            json!({
                "name": "Packed", "version": "1.20.1",
                "flavour": {"fabric": {"loader_version": "0.15.11", "installer_version": null}},
                "description": "", "cmd_args": [], "java_cmd": root.join("no-java-here"),
                "port": 25599, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
                "restart_on_crash": false, "backup_period": null, "jre_major_version": 17,
                "has_started": false, "modpack": origin(1),
            })
            .to_string(),
        )
        .unwrap();
        write(&root, "server.properties", b"server-port=25599\n");
        let uuid = InstanceUuid::default();
        let instance = MinecraftInstance::restore(
            root.clone(),
            crate::types::DotLodestoneConfig::new(
                uuid.clone(),
                crate::traits::t_configurable::GameType::MinecraftJava,
            ),
            state.event_broadcaster.clone(),
            crate::global_settings::CodeExecutionSwitch::new(false),
        )
        .await
        .unwrap();
        state
            .instances
            .insert(uuid.clone(), GameInstance::MinecraftInstance(instance));
        state
            .images()
            .register_refs(Arc::new(LineageRefs::new(state.instances.clone())));
        Core {
            _dir: dir,
            state,
            fake,
            uuid,
            root,
            _packs: packs,
        }
    }

    async fn change(&self, source: UpdateSource) -> Result<(), Error> {
        update_modpack(
            State(self.state.clone()),
            AxumPath(self.uuid.clone()),
            owner(),
            Json(ModpackUpdateRequest { source }),
        )
        .await
        .map(|_| ())
    }

    async fn history(&self) -> crate::modpack::history::ModpackHistory {
        modpack_history(
            State(self.state.clone()),
            AxumPath(self.uuid.clone()),
            owner(),
        )
        .await
        .unwrap()
        .0
    }

    fn images(&self) -> Vec<Image> {
        self.state
            .images()
            .store()
            .list_manifests(self.uuid.as_ref())
            .unwrap()
            .images
    }

    fn image(&self, id: &str) -> Image {
        self.state
            .images()
            .store()
            .load_manifest(self.uuid.as_ref(), id)
            .unwrap()
    }
}

fn entry<'a>(image: &'a Image, path: &str) -> &'a lodestone_image::Entry {
    image
        .entries
        .iter()
        .find(|e| e.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} in {:?}", image.label))
}

#[tokio::test(flavor = "multi_thread")]
async fn updates_and_restores_record_the_lineage() {
    let core = Core::new().await;
    let title = crate::modrinth::ModrinthApi::project(core.fake.as_ref(), "pack")
        .await
        .unwrap()
        .title;

    // ---- an update records one image of the whole server, on the pack ----
    core.change(UpdateSource::Modrinth {
        version_id: "v2".into(),
    })
    .await
    .unwrap();
    let history = History::load(&core.root).await;
    assert_eq!(history.entries.len(), 2);
    let installed = history.entries[0].image.clone().expect("converted");
    let updated = history.entries[1].image.clone().expect("recorded");
    assert_ne!(installed, updated);
    let current = core.state.images().current_content(&core.uuid).await;
    assert_eq!(current.as_deref(), Some(updated.as_str()));
    let labels: Vec<String> = core
        .images()
        .iter()
        .filter_map(|i| i.label.clone())
        .collect();
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.starts_with("Updated"))
            .collect::<Vec<_>>(),
        [&format!("Updated {title} to v2-number")]
    );

    let image = core.image(&updated);
    let pack_sha512 = sha512_hex(&std::fs::read(&core._packs[1].path).unwrap());
    assert_eq!(
        image.base,
        Some(Base::ModrinthPack {
            project: "pack".into(),
            version: "v2".into(),
            sha512: pack_sha512,
        })
    );
    let config = entry(&image, "config/pack.toml");
    assert_eq!(config.source, Source::Base {});
    assert_eq!(config.env, Env::new(EnvValue::Required, EnvValue::Required));
    assert_eq!(config.env_from, [EnvFrom::BasePack]);
    let server_only = entry(&image, "config/server-only.toml");
    assert_eq!(server_only.source, Source::Base {});
    assert_eq!(server_only.env.client, EnvValue::Unsupported);
    let new = entry(&image, "mods/new.jar");
    assert!(matches!(&new.source, Source::Modrinth { project, .. } if project == "n"));
    assert_eq!(new.env, Env::new(EnvValue::Unsupported, EnvValue::Required));
    // The server's own files are in the image too.
    assert!(image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "server.properties"));

    // ---- a failed update records nothing and links nothing ----
    let before = History::load(&core.root).await;
    let images_before = core.images().len();
    assert!(core
        .change(UpdateSource::Modrinth {
            version_id: "v3".into(),
        })
        .await
        .is_err());
    assert_eq!(History::load(&core.root).await, before);
    assert_eq!(core.images().len(), images_before);
    assert_eq!(
        core.state.images().current_content(&core.uuid).await,
        current
    );

    // ---- restoring an old entry (converted from the history) ----
    core.change(UpdateSource::History { entry_id: 1 })
        .await
        .unwrap();
    assert_eq!(read(&core.root, "config/pack.toml").unwrap(), b"v1");
    assert_eq!(read(&core.root, "mods/a-1.jar").unwrap(), b"a1");
    assert!(read(&core.root, "mods/new.jar").is_none());
    let history = History::load(&core.root).await;
    let restored = history.latest().unwrap();
    assert_eq!(restored.kind, HistoryKind::Restore { to: 1 });
    let restored_image = core.image(restored.image.as_deref().unwrap());
    assert_eq!(
        restored_image.label.as_deref(),
        Some(format!("Restored {title} to v1-number").as_str())
    );
    // The restored version's pack is the base again, with its env.
    assert!(matches!(
        &restored_image.base,
        Some(Base::ModrinthPack { version, .. }) if version == "v1"
    ));
    assert_eq!(restored_image.parent.as_deref(), Some(updated.as_str()));

    // ---- restoring a new entry, with a file only its image still has ----
    std::fs::remove_file(object_path(&core.root, &sha1_hex(b"v2")).unwrap()).unwrap();
    let api = core.history().await;
    let update_info = api.entries.iter().find(|e| e.id == 2).unwrap();
    assert_eq!(update_info.missing, 0, "the image has version 2's configs");
    assert_eq!(update_info.image.as_deref(), Some(updated.as_str()));
    // Without the image store the history alone couldn't.
    assert!(
        History::load(&core.root)
            .await
            .info(&core.root, None)
            .entries
            .iter()
            .find(|e| e.id == 2)
            .unwrap()
            .missing
            > 0
    );
    core.change(UpdateSource::History { entry_id: 2 })
        .await
        .unwrap();
    assert_eq!(read(&core.root, "config/pack.toml").unwrap(), b"v2");
    assert_eq!(read(&core.root, "mods/new.jar").unwrap(), b"new");
    // Back at version 2's files: version 2's image again.
    let history = History::load(&core.root).await;
    assert_eq!(
        history.latest().unwrap().image.as_deref(),
        Some(updated.as_str())
    );

    // ---- the History API keeps its shape, newest first, old and new entries ----
    let api = core.history().await;
    let ids: Vec<u32> = api.entries.iter().map(|e| e.id).collect();
    assert_eq!(ids, [4, 3, 2, 1]);
    assert!(api.entries.iter().all(|e| e.image.is_some()));
    assert!(api.entries[0].current);
    assert!(api.entries[1]
        .changes
        .iter()
        .any(|c| c.kind == ChangeKind::Updated));
    let json = serde_json::to_value(&api).unwrap();
    for key in [
        "id",
        "time",
        "kind",
        "name",
        "version_number",
        "game_version",
        "loader",
        "loader_version",
        "changes",
        "kept",
        "file_count",
        "current",
        "missing",
        "changelog",
    ] {
        assert!(json["entries"][0].get(key).is_some(), "{key}");
    }

    // ---- retention keeps what the history names ----
    assert_eq!(core.state.images().prune(&core.uuid).await.unwrap(), 0);
    let kept: Vec<String> = core.images().into_iter().map(|i| i.id).collect();
    for entry in &history.entries {
        assert!(kept.contains(entry.image.as_ref().unwrap()));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn base_info_reads_the_packs_env() {
    let fake = FakeModrinth::default();
    let v2 = test_pack(2, &fake);
    let files = pack_files(&read_pack(&v2.path, false).unwrap(), &v2.path, false).unwrap();
    let info = base_info(&origin("2"), Some(&v2.path), &files, None);
    assert!(matches!(info.base, Some(Base::ModrinthPack { .. })));
    let new = &info.files["mods/new.jar"];
    assert_eq!(
        new.env,
        Some(Env::new(EnvValue::Unsupported, EnvValue::Required))
    );
    assert!(new.modrinth_url.as_deref().unwrap().starts_with(CDN));
    // a disabled pack mod keeps what the pack says
    assert_eq!(info.files["mods/new.jar.disabled"], *new);
    assert_eq!(info.files["mods/keep.jar"].env, Some(Env::UNKNOWN));
    assert_eq!(
        info.files["config/server-only.toml"].env,
        Some(Env::new(EnvValue::Unsupported, EnvValue::Required))
    );
    assert_eq!(
        info.files["config/pack.toml"].env,
        Some(Env::new(EnvValue::Required, EnvValue::Required))
    );
    // An uploaded pack is no base; without the pack nothing is known of its env.
    let mut uploaded = origin("2");
    uploaded.project_id.clear();
    uploaded.version_id.clear();
    let info = base_info(&uploaded, None, &files, None);
    assert_eq!(info.base, None);
    assert_eq!(info.files["mods/new.jar"].env, None);
    // A restored entry's base stands in for a pack Lodestone no longer has, if it is
    // the same version.
    let base = Base::ModrinthPack {
        project: "pack".into(),
        version: "v2".into(),
        sha512: "1".repeat(128),
    };
    assert_eq!(
        base_info(&origin("2"), None, &files, Some(base.clone())).base,
        Some(base.clone())
    );
    assert_eq!(base_info(&origin("1"), None, &files, Some(base)).base, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_server_records_its_first_image() {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
    // what pruning reads while the server isn't listed yet
    state
        .images()
        .register_refs(Arc::new(LineageRefs::new(state.instances.clone())));
    let fake = FakeModrinth::default();
    let v2 = test_pack(2, &fake);
    let pack = read_pack(&v2.path, false).unwrap();
    let root = dir.path().join("new-server");
    std::fs::create_dir_all(&root).unwrap();
    let report = install_pack_contents(&fake, &pack, &v2.path, &root, false, &|_, _| {})
        .await
        .unwrap();
    let uuid = InstanceUuid::default();
    let image = record_install(
        &state,
        &uuid,
        &root,
        &origin("2"),
        &v2.path,
        &report.files,
        "1.20.1",
        &fabric(),
    )
    .await
    .expect("recorded");
    assert_eq!(
        state.images().current_content(&uuid).await.as_deref(),
        Some(image.as_str())
    );
    let manifest = state
        .images()
        .store()
        .load_manifest(uuid.as_ref(), &image)
        .unwrap();
    assert_eq!(manifest.label.as_deref(), Some("Installed Test Pack 2"));
    assert!(matches!(
        &manifest.base,
        Some(Base::ModrinthPack { version, .. }) if version == "v2"
    ));
    assert_eq!(
        entry(&manifest, "mods/new.jar").env,
        Env::new(EnvValue::Unsupported, EnvValue::Required)
    );
    assert_eq!(manifest.parent, None);
    // The guard that let pruning find the history is gone again.
    assert!(LineageRefs::new(state.instances.clone())
        .refs(&uuid)
        .await
        .is_err());
}
