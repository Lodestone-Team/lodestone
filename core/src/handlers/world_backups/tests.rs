//! The world backup routes, against a core on a loopback port with a real (never started)
//! Minecraft instance restored from files in a temporary folder.

use std::collections::HashSet;
use std::path::PathBuf;

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::GlobalSettingsData,
    test_util::{serve_api, test_app_state, wait_until},
    types::DotLodestoneConfig,
};

fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

struct Core {
    dir: tempfile::TempDir,
    state: AppState,
    root: PathBuf,
    base: String,
    client: reqwest::Client,
    uuid: InstanceUuid,
}

/// A core with one stopped vanilla server with a world.
async fn core() -> Core {
    init_paths();
    let dir = tempfile::tempdir().unwrap();
    let state = test_app_state(
        dir.path(),
        GlobalSettingsData {
            code_execution_enabled: false,
            safe_mode: false,
            ..Default::default()
        },
    )
    .await;
    let instances = dir.path().join("instances");
    let root = instances.join("vanilla");
    std::fs::create_dir_all(root.join("world/region")).unwrap();
    let uuid = InstanceUuid::default();
    std::fs::write(
        root.join(".lodestone_config"),
        serde_json::to_string(&DotLodestoneConfig::new(
            uuid.clone(),
            crate::traits::t_configurable::GameType::MinecraftJava,
        ))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join(".lodestone_minecraft_config.json"),
        json!({
            "name": "Survival", "version": "1.21.1", "flavour": "vanilla",
            "description": "", "cmd_args": [], "java_cmd": "java",
            "port": 25572, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 21,
            "has_started": false
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("server.properties"),
        "server-port=25572\nlevel-name=world\n",
    )
    .unwrap();
    std::fs::write(root.join("eula.txt"), "eula=false").unwrap();
    std::fs::write(root.join("server.jar"), "jar v1").unwrap();
    std::fs::write(root.join("world/level.dat"), "level v1").unwrap();
    std::fs::write(root.join("world/region/r.0.0.mca"), "region v1").unwrap();
    let executor = crate::global_settings::CodeExecutionSwitch::new(false);
    let restored = crate::restore_instances(&instances, state.event_broadcaster.clone(), executor)
        .await
        .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    let base = serve_api(state.clone()).await;
    Core {
        dir,
        state,
        root,
        base,
        client: reqwest::Client::new(),
        uuid,
    }
}

impl Core {
    async fn user(&self, name: &str, is_owner: bool, permissions: UserPermission) -> String {
        let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
        self.state
            .users_manager
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user.create_jwt().unwrap().to_string()
    }

    async fn owner(&self) -> String {
        self.user("owner", true, Default::default()).await
    }

    /// A user with these per-server permissions on the core's server (and viewing it).
    async fn member(&self, name: &str, set: impl Fn(&mut UserPermission)) -> String {
        let mut permissions = UserPermission {
            can_view_instance: HashSet::from([self.uuid.clone()]),
            ..Default::default()
        };
        set(&mut permissions);
        self.user(name, false, permissions).await
    }

    /// `path` is relative to `/instance/{uuid}/world-backups`.
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let url = format!("{}/instance/{}/world-backups{path}", self.base, self.uuid);
        self.call_url(method, &url, token, body).await
    }

    async fn call_url(
        &self,
        method: Method,
        url: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self.client.request(method, url).bearer_auth(token);
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

    async fn list(&self, token: &str) -> WorldBackupList {
        let (status, body) = self.call(Method::GET, "", token, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body).unwrap()
    }

    async fn wait_idle(&self, token: &str) {
        wait_until("the job ends", || async {
            self.list(token).await.running.is_none()
        })
        .await;
    }

    /// Makes a world backup and waits for it. Returns its id.
    async fn backup(&self, token: &str, name: Option<&str>) -> String {
        let before: HashSet<String> = self
            .list(token)
            .await
            .backups
            .into_iter()
            .map(|b| b.id)
            .collect();
        let (status, body) = self
            .call(Method::POST, "", token, Some(json!({ "name": name })))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["task_id"].as_str().is_some_and(|id| !id.is_empty()));
        self.wait_idle(token).await;
        // the new one: two made in the same second sort by their ids' random ends
        let mut new: Vec<String> = self
            .list(token)
            .await
            .backups
            .into_iter()
            .map(|b| b.id)
            .filter(|id| !before.contains(id))
            .collect();
        assert_eq!(new.len(), 1, "one new world backup");
        new.remove(0)
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap()
    }

    /// The server's world backups folder.
    fn folder(&self) -> PathBuf {
        self.dir
            .path()
            .join("world-backups")
            .join(self.uuid.as_ref())
    }
}

#[tokio::test]
async fn world_backups_are_made_renamed_and_deleted() {
    let core = core().await;
    let owner = core.owner().await;
    assert!(core.list(&owner).await.backups.is_empty());
    let id = core.backup(&owner, Some("Spawn")).await;
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 1);
    let backup = &list.backups[0];
    assert_eq!(backup.name.as_deref(), Some("Spawn"));
    assert_eq!(backup.level, "world");
    assert_eq!(backup.game_version, "1.21.1");
    assert_eq!(backup.file_count, 2);
    assert!(!backup.live);
    assert!(core.folder().join(format!("{id}.zip")).is_file());
    assert_eq!(list.usage.total_bytes, backup.size);

    let (status, body) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &owner,
            Some(json!({ "name": "Renamed" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "Renamed");
    let (status, body) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &owner,
            Some(json!({ "name": null })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], Value::Null);
    let (status, _) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &owner,
            Some(json!({ "name": "x".repeat(101) })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = core
        .call(Method::DELETE, &format!("/{id}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(core.list(&owner).await.backups.is_empty());
    let (status, _) = core
        .call(Method::DELETE, &format!("/{id}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for bad in ["../x", "20260101-000000-AAAA", "x.json"] {
        let (status, _) = core
            .call(Method::DELETE, &format!("/{bad}"), &owner, None)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{bad}");
    }
    let (status, _) = core
        .call_url(
            Method::GET,
            &format!("{}/instance/not-a-server/world-backups", core.base),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The snapshot routes' permissions: reading lists and downloads, Settings makes and
/// renames, changing files deletes and restores.
#[tokio::test]
async fn each_route_needs_its_permission() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let viewer = core.member("viewer", |_| {}).await;
    let reader = core
        .member("reader", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let settings = core
        .member("settings", |p| {
            p.can_access_instance_setting.insert(core.uuid.clone());
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let writer = core
        .member("writer", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
            p.can_write_instance_file.insert(core.uuid.clone());
        })
        .await;

    let (status, _) = core.call(Method::GET, "", &viewer, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    core.list(&reader).await;
    let (status, _) = core.call(Method::POST, "", &reader, Some(json!({}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(Method::POST, &format!("/{id}/download"), &reader, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(Method::POST, &format!("/{id}/download"), &viewer, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &reader,
            Some(json!({ "name": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for who in [&reader, &settings] {
        let (status, _) = core
            .call(Method::DELETE, &format!("/{id}"), who, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = core
            .call(Method::POST, &format!("/{id}/restore"), who, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, body) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &settings,
            Some(json!({ "name": "x" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = core.backup(&settings, Some("By settings")).await;
    assert_ne!(second, id);

    // a plain world restores with WriteInstanceFile alone
    let (status, body) = core
        .call(Method::POST, &format!("/{id}/restore"), &writer, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;

    // a jar inside the world is a protected file
    core.write("world/datapacks/tool.jar", "jar");
    let with_jar = core.backup(&owner, Some("With a jar")).await;
    let (status, _) = core
        .call(Method::POST, &format!("/{with_jar}/restore"), &writer, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // and so is replacing one that is there now
    let (status, _) = core
        .call(Method::POST, &format!("/{id}/restore"), &writer, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    std::fs::remove_file(core.root.join("world/datapacks/tool.jar")).unwrap();

    // a script needs the owner with code execution on (off here, so nobody)
    core.write("world/datapacks/run.js", "code");
    let with_script = core.backup(&owner, Some("With a script")).await;
    std::fs::remove_file(core.root.join("world/datapacks/run.js")).unwrap();
    let (status, _) = core
        .call(
            Method::POST,
            &format!("/{with_script}/restore"),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!core.root.join("world/datapacks/run.js").exists());
    core.wait_idle(&owner).await;
    let (status, _) = core
        .call(Method::DELETE, &format!("/{id}"), &writer, None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_restore_puts_the_world_back_after_a_safety_backup() {
    let core = core().await;
    let owner = core.owner().await;
    let mut events = core.state.event_broadcaster.subscribe();
    let id = core.backup(&owner, Some("First")).await;
    core.write("world/level.dat", "level v2");
    core.write("world/region/r.1.0.mca", "new");
    core.write("server.jar", "jar v2");
    let (status, body) = core
        .call(Method::POST, &format!("/{id}/restore"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v1");
    assert!(!core.root.join("world/region/r.1.0.mca").exists());
    // only the world changed
    assert_eq!(core.read("server.jar"), "jar v2");
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 2);
    let safety = list.backups.iter().find(|b| b.id != id).unwrap();
    assert_eq!(
        safety.kind,
        crate::backups::world::WorldBackupKind::BeforeRestore
    );
    assert_eq!(safety.name.as_deref(), Some("Before restoring “First”"));
    assert_eq!(safety.before_restoring.as_deref(), Some("First"));

    let mut ends = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd {
                inner: Some(value), ..
            } = p.progression_event_inner()
            {
                ends.push(value.clone());
            }
        }
    }
    assert!(ends.iter().any(|end| matches!(
        end,
        crate::events::ProgressionEndValue::WorldBackupCompleted { success: true, world_backup_id: Some(got), .. } if *got == id
    )));
    assert!(ends.iter().any(|end| matches!(
        end,
        crate::events::ProgressionEndValue::WorldRestoreCompleted { success: true, world_backup_id: Some(got), safety_id: Some(safety_id), .. }
            if *got == id && *safety_id == safety.id
    )));

    // never into a running server
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    instance.set_state_for_test(InstanceState::Running).await;
    let (status, _) = core
        .call(Method::POST, &format!("/{id}/restore"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    instance.set_state_for_test(InstanceState::Stopped).await;
}

/// World jobs and snapshots share one lock per server.
#[tokio::test]
async fn one_job_at_a_time() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let held = core.state.backups.try_lock_job(&core.uuid).unwrap();
    let (status, body) = core.call(Method::POST, "", &owner, Some(json!({}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, _) = core
        .call(Method::POST, &format!("/{id}/restore"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = core
        .call_url(
            Method::POST,
            &format!("{}/instance/{}/backups", core.base, core.uuid),
            &owner,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    drop(held);
    // a task that isn't running can't be cancelled
    let (status, _) = core
        .call(Method::POST, "/tasks/12345/cancel", &owner, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_download_serves_the_stored_zip() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let reader = core
        .member("reader", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let (status, body) = core
        .call(Method::POST, &format!("/{id}/download"), &reader, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let download: WorldBackupDownload = serde_json::from_value(body).unwrap();
    assert_eq!(download.file_name, format!("Survival-world-{id}.zip"));
    let stored = std::fs::read(core.folder().join(format!("{id}.zip"))).unwrap();
    assert_eq!(download.size, stored.len() as u64);
    let url = format!("{}/file/{}", core.base, download.key);
    let response = core.client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-disposition"],
        format!("attachment; filename=\"Survival-world-{id}.zip\"").as_str()
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), stored.as_slice());
    // letting go of the link keeps the backup
    let (status, _) = core.call_url(Method::DELETE, &url, &reader, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(core.folder().join(format!("{id}.zip")).is_file());
    assert_eq!(core.list(&owner).await.backups.len(), 1);

    // a link stops working once its owner may no longer read the server's files
    let (_, body) = core
        .call(Method::POST, &format!("/{id}/download"), &reader, None)
        .await;
    let download: WorldBackupDownload = serde_json::from_value(body).unwrap();
    let reader_id = core
        .state
        .users_manager
        .read()
        .await
        .get_user_by_username("reader")
        .unwrap()
        .uid;
    core.state
        .users_manager
        .write()
        .await
        .modify_permissions(
            &reader_id,
            |p| {
                p.can_read_instance_file.clear();
            },
            CausedBy::System,
        )
        .await
        .unwrap();
    let response = core
        .client
        .get(format!("{}/file/{}", core.base, download.key))
        .send()
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn deleting_the_server_deletes_its_world_backups() {
    let core = core().await;
    let owner = core.owner().await;
    core.backup(&owner, None).await;
    assert!(core.folder().is_dir());
    let (status, body) = core
        .call_url(
            Method::DELETE,
            &format!("{}/instance/{}", core.base, core.uuid),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the world backups are gone", || async {
        !core.folder().exists()
    })
    .await;
}
