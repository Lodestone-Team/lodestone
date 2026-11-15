//! Milestone 2 end to end: the core's content images, backups as data images, the modpack
//! history as the content lineage, Paper's world layout, and retention with collections,
//! through the HTTP API where a client would use it. Every core lives in a temporary
//! folder with a fake Modrinth; no server is started, nothing is downloaded and no EULA is
//! accepted.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lodestone_image::hex::sha256_hex;
use lodestone_image::store::gc::GcOptions;
use lodestone_image::{Base, Kind};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use crate::auth::user::User;
use crate::backups::types::BackupList;
use crate::events::CausedBy;
use crate::handlers::instance::content_images_tests::{self as content, paths, write};
use crate::handlers::modpack_update::UpdateSource;
use crate::images::{AppInstances, KEEP_CONTENT};
use crate::modpack::lineage::tests as lineage;
use crate::types::InstanceUuid;
use crate::AppState;

/// An HTTP client of a core, signed in as its owner.
struct Client {
    base: String,
    token: String,
    uuid: InstanceUuid,
    http: reqwest::Client,
}

impl Client {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    fn backups(&self) -> String {
        format!("/instance/{}/backups", self.uuid)
    }

    async fn list(&self) -> BackupList {
        let (status, body) = self.call(Method::GET, &self.backups(), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body).unwrap()
    }

    async fn wait_idle(&self) {
        crate::test_util::wait_until("the backup job ends", || async {
            self.list().await.running.is_none()
        })
        .await;
    }

    /// Takes a named backup and waits for it. Returns its id.
    async fn backup(&self, name: &str) -> String {
        let (status, body) = self
            .call(Method::POST, &self.backups(), Some(json!({ "name": name })))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        self.wait_idle().await;
        let list = self.list().await;
        assert_eq!(list.backups[0].name.as_deref(), Some(name));
        list.backups[0].id.clone()
    }

    /// Restores a backup without a safety backup and waits for it.
    async fn restore(&self, id: &str, scope: &str) {
        let (status, body) = self
            .call(
                Method::POST,
                &format!("{}/{id}/restore", self.backups()),
                Some(json!({ "scope": scope, "safety_backup": false })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        self.wait_idle().await;
    }
}

/// A client for the content-images harness' core.
fn client_of(core: &content::Core) -> Client {
    Client {
        base: core.base.clone(),
        token: core.owner.clone(),
        uuid: core.uuid.clone(),
        http: reqwest::Client::new(),
    }
}

/// Serves `state` and signs an owner in.
async fn serve(state: &AppState, uuid: &InstanceUuid) -> Client {
    let user = User::new("owner".to_string(), "pw", true, false, Default::default());
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    Client {
        base: crate::test_util::serve_api(state.clone()).await,
        token: user.create_jwt().unwrap().to_string(),
        uuid: uuid.clone(),
        http: reqwest::Client::new(),
    }
}

fn read(root: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(root.join(relative)).ok()
}

/// Fails unless the core's backup folder under `dir` is the one image store layout
/// (`docs/instance-images.md`): the store's folders, and in each server's image folder only
/// its manifests (every one a schema-2 image), stat caches and the core's own files beside
/// them.
fn assert_one_layout(dir: &Path) {
    const CORE_FILES: [&str; 6] = [
        "content-state.json",
        "pack.json",
        "backups.json",
        "backup-policy.json",
        "restore-journal.json",
        "save-off.marker",
    ];
    let names = |dir: &Path| -> Vec<(String, bool)> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| {
                (
                    e.file_name().to_string_lossy().to_string(),
                    e.file_type().unwrap().is_dir(),
                )
            })
            .collect()
    };
    let store = dir.join("backups");
    for (name, _) in names(&store) {
        assert!(
            ["objects", "lists", "chunks", "images"].contains(&name.as_str()),
            "backups/{name} isn't part of the image store"
        );
    }
    for (instance, is_dir) in names(&store.join("images")) {
        if !is_dir {
            assert_eq!(instance, "collection.lock");
            continue;
        }
        for (name, _) in names(&store.join("images").join(&instance)) {
            let path = store.join("images").join(&instance).join(&name);
            let is_manifest = name.len() == 69
                && name.ends_with(".json")
                && lodestone_image::hex::is_sha256(&name[..64]);
            if is_manifest {
                let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                assert_eq!(json["schema"], 2, "{}", path.display());
            } else {
                assert!(
                    CORE_FILES.contains(&name.as_str())
                        || (name.starts_with("stat-cache") && name.ends_with(".json")),
                    "{} isn't part of the image store",
                    path.display()
                );
            }
        }
    }
}

/// Sets every object, list and chunk of the store two hours back, past a collection's grace.
pub(crate) fn age_store(root: &Path) {
    for area in ["objects", "lists", "chunks"] {
        for entry in walkdir::WalkDir::new(root.join(area))
            .min_depth(2)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                let mut permissions = entry.metadata().unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                std::fs::set_permissions(entry.path(), permissions).unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(entry.path())
                    .unwrap()
                    .set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))
                    .unwrap();
            }
        }
    }
}

/// (a) A Mods-tab install records one content image, a backup records a data image linked
/// to it, a hand edit is caught by the check before a start, and restoring "the world and
/// its mods" puts back both.
#[tokio::test]
async fn mods_backups_hand_edits_and_restoring_the_world_and_its_mods() {
    let core = content::core(content::Kind::Fabric).await;
    let client = client_of(&core);
    // with the port its settings have, which a restore writes into server.properties
    write(
        &core.root,
        "server.properties",
        b"motd=Hello\nlevel-name=world\nserver-port=25570\n",
    );
    core.record_initial().await;

    // ---- the Mods tab: one content image ----
    core.modrinth.add(crate::modrinth::fake::FakeVersion::new(
        "jei",
        "v1",
        "jei-1.jar",
    ));
    let before = core.manifests().len();
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/instance/{}/mods/install", core.uuid),
            &core.owner,
            Some(json!({"version_id": "v1"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Added Title of jei");
    assert_eq!(core.manifests().len(), before + 1);
    let installed = core.current().await;
    assert!(paths(&installed).contains(&"mods/jei-1.jar"));

    // ---- a backup: a data image linked to that content image ----
    let id = client.backup("Base").await;
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
    assert_eq!(backup.image.kind, Kind::Data);
    assert_eq!(backup.image.content.as_deref(), Some(installed.id.as_str()));
    assert_eq!(backup.content.as_ref().map(|c| &c.id), Some(&installed.id));
    let data_paths = paths(&backup.image);
    assert!(data_paths.contains(&"saves/world/level.dat"));
    assert!(data_paths.contains(&"mods/jei-1.jar"));
    assert!(!data_paths.iter().any(|p| p.starts_with("logs/")));
    // the backup itself recorded no content image: nothing had changed
    assert_eq!(core.current().await.id, installed.id);

    // ---- a hand edit, caught before the next start ----
    write(&core.root, "mods/by-hand.jar", b"copied over SFTP");
    core.instance()
        .check_content_before_start(&core.state.images)
        .await;
    let outside = core.current().await;
    assert_eq!(outside.label.as_deref(), Some("Changed outside Lodestone"));
    assert!(paths(&outside).contains(&"mods/by-hand.jar"));

    // ---- more changes: the world, and a mod removed in the Mods tab ----
    write(&core.root, "world/level.dat", b"overworld level, later");
    let (status, body) = core
        .call(
            Method::DELETE,
            &format!("/instance/{}/mods/jei-1.jar", core.uuid),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Removed jei-1.jar");

    // ---- restoring the world and its mods puts back both ----
    client.restore(&id, "world_and_mods").await;
    assert_eq!(
        read(&core.root, "world/level.dat").as_deref(),
        Some("overworld level")
    );
    assert!(read(&core.root, "mods/jei-1.jar").is_some());
    // a mod the backup's content didn't have goes; its bytes stay in its image
    assert!(read(&core.root, "mods/by-hand.jar").is_none());
    let store = core.state.images.store();
    let hand = outside
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/by-hand.jar")
        .unwrap();
    assert!(store.has_object(&hand.sha256, hand.size));
    // the restore is one content image, of the backup's content
    let restored = core.current().await;
    assert_eq!(restored.id, installed.id);
    assert_eq!(
        restored.label.as_deref(),
        Some("Restored backup \u{201c}Base\u{201d}")
    );
    assert!(!core.pending());
    assert_one_layout(core._dir.path());
}

/// (b) A modpack install, update and history restore are the lineage, and the History API
/// lists it. A backup taken on the update links to its image, and restoring the backup's
/// world and mods brings the update back, with the pack it is built on.
#[tokio::test(flavor = "multi_thread")]
async fn the_modpack_lineage_and_backups() {
    let core = lineage::Core::new().await;
    core.state.images.set_instances(Arc::new(AppInstances {
        instances: core.state.instances.clone(),
        root: core.state.images.root().to_path_buf(),
    }));
    let client = serve(&core.state, &core.uuid).await;
    let title = core.title.clone();
    lineage::write(&core.root, "world/level.dat", b"level");
    let installed = core.current().await;

    core.change(UpdateSource::Modrinth {
        version_id: "v2".into(),
    })
    .await
    .unwrap();
    let updated = core.current().await;
    assert_ne!(updated, installed);
    let v2_base = core.image(&updated).base.clone();
    assert!(matches!(&v2_base, Some(Base::ModrinthPack { version, .. }) if version == "v2"));

    // ---- the History API, over HTTP, is the lineage newest first ----
    let (status, history) = client
        .call(
            Method::GET,
            &format!("/instance/{}/modpack/history", core.uuid),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    let entries = history["entries"].as_array().unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, [updated.as_str(), installed.as_str()]);
    assert_eq!(entries[0]["label"], format!("Updated {title} to v2-number"));
    assert_eq!(entries[0]["current"], true);
    assert_eq!(entries[1]["label"], format!("Installed {title} v1-number"));

    // ---- a backup on version 2 links to the update's image ----
    let backup_id = client.backup("On v2").await;
    let backup = core
        .state
        .backups
        .load(&core.uuid, &backup_id)
        .await
        .unwrap();
    assert_eq!(backup.image.content.as_deref(), Some(updated.as_str()));

    // ---- restoring the install from the history ----
    core.restore(&installed).await.unwrap();
    assert_eq!(lineage::read(&core.root, "mods/a-1.jar").unwrap(), b"a1");
    assert!(lineage::read(&core.root, "mods/new.jar").is_none());
    assert_eq!(core.origin().await.version_id, "v1");
    assert!(matches!(
        &core.pack_json().base,
        Some(Base::ModrinthPack { version, .. }) if version == "v1"
    ));
    let history = core.history().await;
    assert_eq!(
        history.entries[0].label,
        format!("Restored \u{201c}Installed {title} v1-number\u{201d}")
    );

    // ---- restoring the backup's world and mods brings version 2 back ----
    client.restore(&backup_id, "world_and_mods").await;
    assert_eq!(lineage::read(&core.root, "mods/a-2.jar").unwrap(), b"a2");
    assert_eq!(lineage::read(&core.root, "mods/new.jar").unwrap(), b"new");
    assert!(lineage::read(&core.root, "mods/a-1.jar").is_none());
    // the server says it is on version 2 again: its origin, and the pack its images name
    assert_eq!(core.origin().await.version_id, "v2");
    assert_eq!(core.pack_json().base, v2_base);
    let restored = core.image(&core.current().await);
    assert_eq!(restored.base, v2_base);
    let history = core.history().await;
    assert_eq!(
        history.entries[0].label,
        "Restored backup \u{201c}On v2\u{201d}"
    );
    assert!(history.entries[0].current);
    // every image so far is still listed: none was pruned
    let listed: HashSet<&str> = history.entries.iter().map(|e| e.id.as_str()).collect();
    assert!(listed.contains(installed.as_str()));
    assert!(listed.contains(updated.as_str()));

    // ---- a server restored as new from the backup is built on version 2 too ----
    std::fs::create_dir_all(core._dir.path().join("instances")).unwrap();
    let (status, body) = client
        .call(
            Method::POST,
            &format!("{}/{backup_id}/restore_as_new", client.backups()),
            Some(json!({ "name": "Copy" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let images = core.state.images.clone();
    let new = || {
        core.state
            .instances
            .iter()
            .map(|e| e.key().clone())
            .find(|uuid| *uuid != core.uuid)
    };
    crate::test_util::wait_until("the copy records its first image", || async {
        match new() {
            Some(uuid) => images.current_content(&uuid).await.is_some(),
            None => false,
        }
    })
    .await;
    let copy = new().unwrap();
    let pack = crate::images::base::load_base(images.root(), &copy).unwrap();
    assert_eq!(pack.base, v2_base);
    assert!(pack.files.is_empty());
    let first = images.current_content(&copy).await.unwrap();
    let first = images.store().load_manifest(copy.as_ref(), &first).unwrap();
    assert_eq!(first.label.as_deref(), Some("Created Copy"));
    assert_eq!(first.base, v2_base);
    assert_one_layout(core._dir.path());
}

/// (c) A Paper server's split worlds back up as one canonical world and restore to Paper's
/// layout.
#[tokio::test]
async fn a_paper_server_backs_up_to_canonical_paths_and_restores_to_its_layout() {
    let core = content::core(content::Kind::Paper).await;
    let client = client_of(&core);
    core.record_initial().await;
    let id = client.backup("Split").await;
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
    let data_paths = paths(&backup.image);
    for path in [
        "saves/world/level.dat",
        "saves/world/region/r.0.0.mca",
        "saves/world/DIM-1/region/r.0.0.mca",
        "saves/world/DIM1/region/r.0.0.mca",
        "plugins/WorldEdit.jar",
    ] {
        assert!(data_paths.contains(&path), "{path} in {data_paths:?}");
    }
    assert!(
        !data_paths.iter().any(|p| p.starts_with("world_")),
        "{data_paths:?}"
    );
    // the worlds are never content
    assert!(!paths(&core.current().await)
        .iter()
        .any(|p| p.starts_with("saves/") || p.starts_with("world")));

    std::fs::remove_file(core.root.join("world_nether/DIM-1/region/r.0.0.mca")).unwrap();
    write(
        &core.root,
        "world_the_end/DIM1/region/r.0.0.mca",
        b"end region, later",
    );
    write(&core.root, "world/level.dat", b"overworld level, later");
    client.restore(&id, "world").await;
    assert_eq!(
        read(&core.root, "world_nether/DIM-1/region/r.0.0.mca").as_deref(),
        Some("nether region")
    );
    assert_eq!(
        read(&core.root, "world_the_end/DIM1/region/r.0.0.mca").as_deref(),
        Some("end region")
    );
    assert_eq!(
        read(&core.root, "world/level.dat").as_deref(),
        Some("overworld level")
    );
    assert_eq!(
        read(&core.root, "world_nether/level.dat").as_deref(),
        Some("nether level")
    );
    // nothing is put back in the vanilla layout
    assert!(!core.root.join("world/DIM-1").exists());
    assert!(!core.root.join("world/DIM1").exists());
    assert_one_layout(core._dir.path());
}

/// (d) Retention keeps every content image a backup links to and the newest
/// [`KEEP_CONTENT`] content images and deletes the rest; a collection then deletes only
/// the objects no kept image names. Deleting the backup lets its content image go too.
#[tokio::test]
async fn retention_and_collections_keep_what_backups_link_to() {
    let core = content::core(content::Kind::Fabric).await;
    let client = client_of(&core);
    let images = core.state.images.clone();
    let initial = core.record_initial().await.id;

    // Each change writes a counter of its own, so each image has one object no other has.
    let counter = |n: usize| format!("count = {n}");
    let change = |n: usize| {
        let (images, core) = (images.clone(), &core);
        async move {
            let op = images.begin(&core.uuid, format!("Change {n}")).await;
            write(&core.root, "config/counter.toml", counter(n).as_bytes());
            let ctx = crate::images::context_of(&core.state, &core.uuid)
                .await
                .unwrap();
            op.succeed_and_record(ctx).await.unwrap().unwrap().id
        }
    };
    let linked = change(0).await;
    let backup_id = client.backup("Linked").await;
    let backup = core
        .state
        .backups
        .load(&core.uuid, &backup_id)
        .await
        .unwrap();
    assert_eq!(backup.image.content.as_deref(), Some(linked.as_str()));
    let mut changes = vec![linked.clone()];
    for n in 1..=KEEP_CONTENT + 4 {
        changes.push(change(n).await);
    }
    images.prune(&core.uuid).await.unwrap();

    let content_ids = |core: &content::Core| -> HashSet<String> {
        core.manifests()
            .into_iter()
            .filter(|i| i.kind == Kind::Content)
            .map(|i| i.id)
            .collect()
    };
    let kept = content_ids(&core);
    // the newest 30 (changes 5 to 34), and the one the backup links to
    let newest = &changes[changes.len() - KEEP_CONTENT..];
    assert!(newest.iter().all(|id| kept.contains(id)));
    assert!(kept.contains(&linked));
    assert_eq!(kept.len(), KEEP_CONTENT + 1);
    for gone in std::iter::once(&initial).chain(&changes[1..5]) {
        assert!(!kept.contains(gone), "{gone} should be pruned");
    }
    // the backup's data image itself is never pruned
    assert!(core.manifests().iter().any(|i| i.id == backup.image.id));

    // ---- a collection deletes only what no kept image names ----
    let store = images.store();
    let object_of = |n: usize| {
        let bytes = counter(n);
        (sha256_hex(bytes.as_bytes()), bytes.len() as u64)
    };
    age_store(store.root());
    let collect = || {
        let store = store.clone();
        {
            tokio::task::spawn_blocking(move || {
                crate::images::gc::collect(
                    &store,
                    &GcOptions {
                        grace: lodestone_image::store::gc::MIN_GRACE,
                        ..Default::default()
                    },
                )
                .unwrap()
            })
        }
    };
    collect().await.unwrap();
    for n in 1..5 {
        let (sha, size) = object_of(n);
        assert!(
            !store.has_object(&sha, size),
            "change {n}'s object is collected"
        );
    }
    for n in std::iter::once(0).chain(5..=KEEP_CONTENT + 4) {
        let (sha, size) = object_of(n);
        assert!(store.has_object(&sha, size), "change {n}'s object is kept");
    }
    for entry in &backup.image.entries {
        assert!(
            store.has_object(&entry.sha256, entry.size),
            "{} of the backup",
            entry.path
        );
    }

    // ---- deleting the backup lets its image and the content it linked to go ----
    let (status, body) = client
        .call(
            Method::DELETE,
            &format!("{}/{backup_id}", client.backups()),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!core.manifests().iter().any(|i| i.id == backup.image.id));
    let kept = content_ids(&core);
    assert!(!kept.contains(&linked));
    assert_eq!(kept.len(), KEEP_CONTENT);
    age_store(store.root());
    collect().await.unwrap();
    let (sha, size) = object_of(0);
    assert!(!store.has_object(&sha, size));
    assert_one_layout(core._dir.path());
}
