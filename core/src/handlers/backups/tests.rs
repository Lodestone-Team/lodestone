//! The backup routes, against a core on a loopback port with a real (never started)
//! Minecraft instance restored from files in a temporary folder.

use std::collections::HashSet;
use std::path::PathBuf;

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::{CodeExecutionSwitch, GlobalSettingsData},
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

/// A core with one stopped vanilla server, with a jar, a world and a plugin folder.
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
            "port": 25571, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 21,
            "has_started": false
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("server.properties"),
        "server-port=25571\nlevel-name=world\n",
    )
    .unwrap();
    std::fs::write(root.join("eula.txt"), "eula=false").unwrap();
    std::fs::write(root.join("server.jar"), "jar v1").unwrap();
    std::fs::write(root.join("world/level.dat"), "level v1").unwrap();
    std::fs::write(root.join("world/region/r.0.0.mca"), "region v1").unwrap();
    let executor = CodeExecutionSwitch::new(false);
    let restored = crate::restore_instances(&instances, state.event_broadcaster.clone(), executor)
        .await
        .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    assert_eq!(state.instances.len(), 1);
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

    /// `path` is relative to `/instance/{uuid}/backups`.
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let url = format!("{}/instance/{}/backups{path}", self.base, self.uuid);
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

    async fn list(&self, token: &str) -> BackupList {
        let (status, body) = self.call(Method::GET, "", token, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        serde_json::from_value(body).unwrap()
    }

    /// Waits until no backup or restore of the server runs.
    async fn wait_idle(&self, token: &str) {
        wait_until("the job ends", || async {
            self.list(token).await.running.is_none()
        })
        .await;
    }

    /// Takes a manual backup and waits for it. Returns its id.
    async fn backup(&self, token: &str, name: Option<&str>) -> String {
        let (status, body) = self
            .call(Method::POST, "", token, Some(json!({ "name": name })))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["task_id"].as_str().is_some_and(|id| !id.is_empty()));
        self.wait_idle(token).await;
        self.list(token).await.backups[0].id.clone()
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap()
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn server_backups(&self) -> PathBuf {
        self.dir
            .path()
            .join("backups/servers")
            .join(self.uuid.as_ref())
    }
}

/// The bodies the Host sends (`launcher-core`'s `cores/admin/backups.rs`): an empty object
/// to back up or download, `"name": null` to restore as new with the default name, and
/// only the fields that change in a PATCH.
#[test]
fn the_hosts_bodies_are_read() {
    let create: CreateBackupRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(create.name, None);
    let new: RestoreAsNewRequest = serde_json::from_value(json!({ "name": null })).unwrap();
    assert_eq!(new.name, None);
    let update: UpdateBackupRequest =
        serde_json::from_value(json!({ "name": null, "pinned": true })).unwrap();
    assert_eq!(update.name, Some(None));
    assert_eq!(update.pinned, Some(true));
    let update: UpdateBackupRequest = serde_json::from_value(json!({ "pinned": false })).unwrap();
    assert_eq!(update.name, None);
    let restore: RestoreRequest = serde_json::from_value(json!({
        "scope": "world", "stop": true, "start_after": false, "safety_backup": true
    }))
    .unwrap();
    assert_eq!(restore.scope, RestoreScope::World);
    assert!(!restore.start_after);
    let task = serde_json::to_value(BackupTask {
        task_id: "7380000000000000001".to_string(),
    })
    .unwrap();
    assert_eq!(task, json!({ "task_id": "7380000000000000001" }));
}

#[tokio::test]
async fn backups_are_listed_renamed_pinned_and_deleted() {
    let core = core().await;
    let owner = core.owner().await;
    let empty = core.list(&owner).await;
    assert!(empty.backups.is_empty());
    assert!(empty.running.is_none());
    assert!(!empty.policy.enabled);

    let id = core.backup(&owner, Some("First")).await;
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 1);
    let backup = &list.backups[0];
    assert_eq!(backup.name.as_deref(), Some("First"));
    assert_eq!(backup.kind, crate::backups::BackupKind::Manual);
    assert!(!backup.live);
    assert!(backup.has_world);
    // server.jar, eula.txt, server.properties, the config and two world files; never
    // .lodestone_config
    assert_eq!(backup.file_count, 6);
    assert!(list.usage.total_bytes > 0 && list.usage.stored_bytes > 0);

    // a second manual backup within a minute is refused
    let (status, _) = core.call(Method::POST, "", &owner, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

    let (status, body) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &owner,
            Some(json!({ "name": "Renamed", "pinned": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "Renamed");
    assert_eq!(body["pinned"], true);
    // null clears the name; leaving it out keeps it
    let (_, body) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &owner,
            Some(json!({ "name": null })),
        )
        .await;
    assert_eq!(body["name"], Value::Null);
    assert_eq!(body["pinned"], true);
    let (_, body) = core
        .call(Method::PATCH, &format!("/{id}"), &owner, Some(json!({})))
        .await;
    assert_eq!(body["pinned"], true);
    let listed = core.list(&owner).await;
    assert!(listed.backups[0].pinned);
    assert_eq!(listed.backups[0].name, None);

    let (status, body) = core
        .call(Method::DELETE, &format!("/{id}"), &owner, None)
        .await;
    assert_eq!((status, body), (StatusCode::OK, json!({})));
    assert!(core.list(&owner).await.backups.is_empty());
    let (status, _) = core
        .call(Method::DELETE, &format!("/{id}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_policy_is_read_validated_and_saved() {
    let core = core().await;
    let owner = core.owner().await;
    let (status, policy) = core.call(Method::GET, "/policy", &owner, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(policy["interval_minutes"], 360);

    let mut changed = policy.clone();
    changed["enabled"] = json!(true);
    changed["interval_minutes"] = json!(60);
    let (status, body) = core
        .call(Method::PUT, "/policy", &owner, Some(changed.clone()))
        .await;
    assert_eq!((status, body), (StatusCode::OK, changed.clone()));
    assert_eq!(
        core.call(Method::GET, "/policy", &owner, None).await.1,
        changed
    );
    let list = core.list(&owner).await;
    assert!(list.policy.enabled);
    // the schedule is on and nothing ran yet, so the next backup is due now
    assert!(list.next_scheduled.is_some());

    let mut invalid = changed.clone();
    invalid["interval_minutes"] = json!(5);
    let (status, body) = core
        .call(Method::PUT, "/policy", &owner, Some(invalid))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.to_string().contains("interval_minutes"), "{body}");

    // `policy` isn't taken for a backup id
    let (status, _) = core.call(Method::DELETE, "/policy", &owner, None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(core.server_backups().join("policy.json").is_file());
}

#[tokio::test]
async fn permissions_are_checked() {
    let core = core().await;
    let owner = core.owner().await;
    let viewer = core.member("viewer", |_| {}).await;
    let reader = core
        .member("reader", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let setter = core
        .member("setter", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
            p.can_access_instance_setting.insert(core.uuid.clone());
        })
        .await;
    let id = core.backup(&owner, None).await;

    // viewing the server isn't enough to see or make backups
    assert_eq!(
        core.call(Method::GET, "", &viewer, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        core.call(Method::POST, "", &viewer, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        core.call(Method::POST, "", &reader, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(core.list(&reader).await.backups.len(), 1);

    // without WriteInstanceFile: no restore, delete, unpin or policy change
    let restore = json!({ "scope": "everything" });
    for (method, path, body) in [
        (Method::POST, format!("/{id}/restore"), Some(restore)),
        (Method::DELETE, format!("/{id}"), None),
        (
            Method::PATCH,
            format!("/{id}"),
            Some(json!({ "pinned": false })),
        ),
        (
            Method::PUT,
            "/policy".to_string(),
            Some(serde_json::to_value(BackupPolicy::default()).unwrap()),
        ),
    ] {
        let (status, body) = core.call(method.clone(), &path, &setter, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
    }
    // pinning and renaming only need the settings permission
    let (status, _) = core
        .call(
            Method::PATCH,
            &format!("/{id}"),
            &setter,
            Some(json!({ "pinned": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    // restoring as a new server needs CreateInstance
    let (status, _) = core
        .call(
            Method::POST,
            &format!("/{id}/restore_as_new"),
            &setter,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(core.list(&owner).await.backups.len(), 1);
}

#[tokio::test]
async fn restoring_a_jar_needs_global_file_permission_but_the_world_does_not() {
    let core = core().await;
    let owner = core.owner().await;
    let writer = core
        .member("writer", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
            p.can_write_instance_file.insert(core.uuid.clone());
        })
        .await;
    let id = core.backup(&owner, None).await;
    core.write("server.jar", "jar v2");
    core.write("world/level.dat", "level v2");

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &writer,
            Some(json!({ "scope": "everything" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.to_string().contains("protected"), "{body}");
    assert_eq!(core.read("server.jar"), "jar v2");
    assert!(core.list(&owner).await.running.is_none());

    // the world has no protected files
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &writer,
            Some(json!({ "scope": "world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v1");
    assert_eq!(core.read("server.jar"), "jar v2");
    // the safety backup has the world as it was
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 2);
    let safety = list
        .backups
        .iter()
        .find(|b| b.kind == crate::backups::BackupKind::BeforeRestore)
        .unwrap();
    assert!(safety
        .name
        .as_deref()
        .unwrap()
        .starts_with("Before restoring “"));
}

#[tokio::test]
async fn everything_and_the_world_are_restored() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, Some("Base")).await;
    let mut events = core.state.event_broadcaster.subscribe();

    // changes after the backup
    core.write("server.jar", "jar v2");
    core.write("world/level.dat", "level v2");
    core.write("world/region/r.1.0.mca", "new region");
    core.write("notes.txt", "added later");
    core.write("logs/latest.log", "log");

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world", "safety_backup": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v1");
    assert!(!core.root.join("world/region/r.1.0.mca").exists());
    // outside the world nothing changed
    assert_eq!(core.read("server.jar"), "jar v2");
    assert_eq!(core.read("notes.txt"), "added later");
    assert_eq!(core.list(&owner).await.backups.len(), 1);

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "everything" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("server.jar"), "jar v1");
    assert!(!core.root.join("notes.txt").exists());
    // never backed up, so never touched
    assert_eq!(core.read("logs/latest.log"), "log");
    assert!(core.root.join(".lodestone_config").is_file());
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 2, "with the safety backup");
    // nothing is left in the server folder
    let leftovers: Vec<_> = std::fs::read_dir(&core.root)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".lodestone-restore")
        })
        .collect();
    assert!(leftovers.is_empty());
    // the restore ended successfully, naming the backup
    let mut ends = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd {
                inner:
                    Some(crate::events::ProgressionEndValue::RestoreCompleted {
                        success,
                        backup_id,
                        ..
                    }),
                ..
            } = p.progression_event_inner()
            {
                ends.push((*success, backup_id.clone()));
            }
        }
    }
    assert_eq!(ends, vec![(true, Some(id.clone())), (true, Some(id))]);

    // a running server isn't restored without stopping it
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    instance.set_state_for_test(InstanceState::Running).await;
    let (status, _) = core
        .call(
            Method::POST,
            &format!("/{}/restore", list.backups[0].id),
            &owner,
            Some(json!({ "scope": "everything", "stop": false })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn a_backup_is_restored_as_a_new_server() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore_as_new"),
            &owner,
            Some(json!({ "name": "Copy" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the new server is added", || async {
        core.state.instances.len() == 2
    })
    .await;
    let new = core
        .state
        .instances
        .iter()
        .map(|e| e.key().clone())
        .find(|uuid| *uuid != core.uuid)
        .unwrap();
    let instance = minecraft(&core.state, &new).unwrap();
    assert_eq!(instance.name().await, "Copy");
    let port = instance.port().await;
    assert_ne!(port, 25571);
    let root = instance.path().await;
    assert_ne!(root, core.root);
    assert_eq!(
        std::fs::read_to_string(root.join("server.jar")).unwrap(),
        "jar v1"
    );
    assert!(std::fs::read_to_string(root.join("server.properties"))
        .unwrap()
        .contains(&format!("server-port={port}")));
    // code execution is off, so the backup's Java command isn't kept
    assert_eq!(instance.config_snapshot().await.java_cmd, None);
    let config: DotLodestoneConfig =
        serde_json::from_slice(&std::fs::read(root.join(".lodestone_config")).unwrap()).unwrap();
    assert_eq!(config.uuid(), &new);
    assert!(
        core.state
            .port_manager
            .lock()
            .await
            .port_status(port)
            .is_allocated
    );
}

#[tokio::test]
async fn a_backup_is_downloaded_as_a_zip() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let (status, body) = core
        .call(Method::POST, &format!("/{id}/download"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let download: BackupDownload = serde_json::from_value(body).unwrap();
    assert!(download.file_name.ends_with(&format!("{id}.zip")));
    assert!(download.size > 0);
    let response = core
        .client
        .get(format!("{}/file/{}", core.base, download.key))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.bytes().await.unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().to_string())
        .collect();
    assert!(names.contains(&"world/level.dat".to_string()), "{names:?}");
    let mut level = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("world/level.dat").unwrap(), &mut level)
        .unwrap();
    assert_eq!(level, "level v1");

    let (status, _) = core
        .call(Method::POST, "/20200101-000000-0000/download", &owner, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn uuids_that_leave_the_store_are_refused() {
    let core = core().await;
    let owner = core.owner().await;
    for uuid in ["..%2F..%2Fstores", "%2Ftmp%2Fx", "C:%5Cx", "DOCKER-abc"] {
        for (method, path) in [
            (Method::GET, "/backups".to_string()),
            (Method::GET, "/backups/policy".to_string()),
            (Method::DELETE, "/backups/users".to_string()),
        ] {
            let url = format!("{}/instance/{uuid}{path}", core.base);
            let (status, body) = core.call_url(method.clone(), &url, &owner, None).await;
            assert!(
                status == StatusCode::NOT_FOUND || status == StatusCode::BAD_REQUEST,
                "{method} {url}: {status} {body}"
            );
        }
    }
    assert!(!core.dir.path().join("stores/users.json").exists());
}

#[tokio::test]
async fn deleting_the_server_deletes_its_backups() {
    let core = core().await;
    let owner = core.owner().await;
    core.backup(&owner, None).await;
    assert!(core.server_backups().is_dir());
    let (status, body) = core
        .call_url(
            Method::DELETE,
            &format!("{}/instance/{}", core.base, core.uuid),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!core.server_backups().exists());
    assert!(!core.root.exists());
}

#[tokio::test]
async fn the_file_api_never_touches_a_restores_folders() {
    let core = core().await;
    let owner = core.owner().await;
    core.write(
        ".lodestone-restore-old-0123456789abcdef/world/level.dat",
        "old",
    );
    let encode = |path: &str| {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path)
    };
    for path in [
        ".lodestone-restore-old-0123456789abcdef/world/level.dat",
        ".LODESTONE-RESTORE-old-0123456789abcdef/world/level.dat",
    ] {
        let url = format!(
            "{}/instance/{}/fs/{}/read",
            core.base,
            core.uuid,
            encode(path)
        );
        let (status, body) = core.call_url(Method::GET, &url, &owner, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {body}");
    }
    let url = format!(
        "{}/instance/{}/fs/{}/rm",
        core.base,
        core.uuid,
        encode(".lodestone-restore-old-0123456789abcdef/world/level.dat")
    );
    let (status, _) = core.call_url(Method::DELETE, &url, &owner, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(core
        .root
        .join(".lodestone-restore-old-0123456789abcdef/world/level.dat")
        .is_file());
    // nor lists them
    let url = format!("{}/instance/{}/fs/{}/ls", core.base, core.uuid, encode("."));
    let (status, body) = core.call_url(Method::GET, &url, &owner, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.to_string().contains(".lodestone-restore"), "{body}");
    assert!(body.to_string().contains("server.jar"), "{body}");

    // nor by the short name Windows gives the folder, where the disk makes them
    let short = core.root.join("LODEST~1");
    if short.is_dir() {
        for (method, route) in [(Method::GET, "read"), (Method::DELETE, "rm")] {
            let url = format!(
                "{}/instance/{}/fs/{}/{route}",
                core.base,
                core.uuid,
                encode("LODEST~1/world/level.dat")
            );
            let (status, body) = core.call_url(method, &url, &owner, None).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{route}: {body}");
        }
        assert!(short.join("world/level.dat").is_file());
    }
}

/// A full scan that finds nothing changed saves no backup, but is still recorded, so the
/// next scheduled backup doesn't hash everything again.
#[tokio::test]
async fn a_full_scan_that_found_nothing_is_recorded() {
    let core = core().await;
    let owner = core.owner().await;
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    let count = core.server_backups().join("full-scan.count");
    core.state
        .backups
        .scheduled_backup(&core.state, instance.clone())
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&count).unwrap(), "0");
    // the 10th scheduled backup since the full scan is due to be one
    std::fs::write(&count, "9").unwrap();
    core.state
        .backups
        .scheduled_backup(&core.state, instance)
        .await
        .unwrap();
    assert_eq!(core.list(&owner).await.backups.len(), 1, "nothing changed");
    assert_eq!(std::fs::read_to_string(&count).unwrap(), "0");
}

/// The handler checks the stop and start permissions only for a server it sees running;
/// one that starts before the restore locks it is refused rather than stopped unchecked.
#[tokio::test]
async fn a_server_that_started_meanwhile_is_not_stopped() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let manifest = core
        .state
        .backups
        .load_manifest(&core.uuid, &id)
        .await
        .unwrap();
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    instance.set_state_for_test(InstanceState::Running).await;
    let request = RestoreRequest {
        scope: RestoreScope::Everything,
        stop: true,
        start_after: true,
        safety_backup: false,
    };
    let error = core
        .state
        .backups
        .start_restore(
            &core.state,
            instance.clone(),
            manifest,
            request,
            false,
            None,
            CausedBy::System,
        )
        .await
        .err()
        .unwrap();
    assert!(matches!(error.kind, ErrorKind::Conflict));
    assert_eq!(instance.state().await, InstanceState::Running);
    assert!(core.list(&owner).await.running.is_none());
}

/// The restore uses the policy its permission check used, even if the policy changed while
/// the check hashed the server.
#[tokio::test]
async fn a_restore_keeps_the_policy_it_was_checked_with() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    core.write("server.jar", "jar v2");
    core.write("world/level.dat", "level v2");
    let store = core.state.backups.store().clone();
    let excluding_jars = BackupPolicy {
        exclude: vec!["*.jar".to_string()],
        ..Default::default()
    };
    store.save_policy(&core.uuid, &excluding_jars).unwrap();
    let manifest = core
        .state
        .backups
        .load_manifest(&core.uuid, &id)
        .await
        .unwrap();
    let uuid = core.uuid.clone();
    let check: DiffCheck = Arc::new(move |diff: &RestoreDiff| {
        assert!(!diff.changed.iter().any(|path| path == "server.jar"));
        // the policy changes while the check runs
        store.save_policy(&uuid, &BackupPolicy::default()).unwrap();
        Ok(())
    });
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    core.state
        .backups
        .start_restore(
            &core.state,
            instance,
            manifest,
            RestoreRequest {
                scope: RestoreScope::Everything,
                stop: false,
                start_after: false,
                safety_backup: false,
            },
            false,
            Some(check),
            CausedBy::System,
        )
        .await
        .unwrap();
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v1");
    assert_eq!(core.read("server.jar"), "jar v2", "excluded when checked");
}

/// The check after stopping compares against the safety backup's files, which are taken
/// with the exclude the first check used: a file the old policy owns, written before the
/// stop, is seen by the second check even if the policy came to exclude it.
#[tokio::test]
async fn the_check_after_stopping_uses_the_exclude_it_was_checked_with() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    core.write("plugins/new.jar", "written after the backup");
    let manifest = core
        .state
        .backups
        .load_manifest(&core.uuid, &id)
        .await
        .unwrap();
    let store = core.state.backups.store().clone();
    let uuid = core.uuid.clone();
    let removed: Arc<std::sync::Mutex<Vec<Vec<String>>>> = Default::default();
    let seen = removed.clone();
    let check: DiffCheck = Arc::new(move |diff: &RestoreDiff| {
        let mut seen = seen.lock().unwrap();
        if seen.is_empty() {
            // the policy changes between the first check and the safety backup
            let excluding_jars = BackupPolicy {
                exclude: vec!["*.jar".to_string()],
                ..Default::default()
            };
            store.save_policy(&uuid, &excluding_jars).unwrap();
        }
        seen.push(diff.removed.clone());
        Ok(())
    });
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    core.state
        .backups
        .start_restore(
            &core.state,
            instance,
            manifest,
            RestoreRequest {
                scope: RestoreScope::Everything,
                stop: false,
                start_after: false,
                safety_backup: true,
            },
            false,
            Some(check),
            CausedBy::System,
        )
        .await
        .unwrap();
    core.wait_idle(&owner).await;
    let removed = removed.lock().unwrap().clone();
    assert_eq!(removed.len(), 2, "checked before and after stopping");
    for diff in &removed {
        assert!(diff.iter().any(|p| p == "plugins/new.jar"), "{removed:?}");
    }
    assert!(!core.root.join("plugins/new.jar").exists());
    // the safety backup has it, so it can be restored
    let safety = core
        .list(&owner)
        .await
        .backups
        .into_iter()
        .find(|b| b.kind == crate::backups::BackupKind::BeforeRestore)
        .unwrap();
    let safety = core
        .state
        .backups
        .load_manifest(&core.uuid, &safety.id)
        .await
        .unwrap();
    assert!(safety.files.iter().any(|f| f.path == "plugins/new.jar"));
}

/// A server that stopped moments ago (it may have crashed, and restart in 5 seconds) isn't
/// backed up on schedule while stopped, even when it was stopped at the last look too; a
/// minute later it is.
#[tokio::test]
async fn a_server_that_just_stopped_is_not_backed_up_on_schedule() {
    let core = core().await;
    let owner = core.owner().await;
    core.state
        .backups
        .store()
        .save_policy(
            &core.uuid,
            &BackupPolicy {
                enabled: true,
                only_when_active: false,
                ..Default::default()
            },
        )
        .unwrap();
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    // it started and crashed since the last look, which saw it stopped
    instance.set_state_for_test(InstanceState::Running).await;
    instance.set_state_for_test(InstanceState::Stopped).await;
    assert!(instance
        .stopped_ago()
        .is_some_and(|ago| ago < std::time::Duration::from_secs(60)));
    let seen = std::collections::HashMap::from([(core.uuid.clone(), InstanceState::Stopped)]);
    crate::backups::scheduler::tick(&core.state, &seen).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let list = core.list(&owner).await;
    assert!(list.running.is_none() && list.backups.is_empty());
    // nothing holds off its restart
    let guard = instance.lock_stopped("testing").await.unwrap();
    drop(guard);

    instance.set_stopped_ago_for_test(std::time::Duration::from_secs(61));
    crate::backups::scheduler::tick(&core.state, &seen).await;
    wait_until("the scheduled backup", || async {
        let list = core.list(&owner).await;
        list.running.is_none() && list.backups.len() == 1
    })
    .await;
}

/// While no server has scheduled backups on, the scheduler says so (and waits); saving a
/// schedule wakes it.
#[tokio::test]
async fn the_scheduler_waits_while_no_schedule_is_on() {
    let core = core().await;
    let seen = std::collections::HashMap::new();
    let (_, any_scheduled) = crate::backups::scheduler::tick(&core.state, &seen).await;
    assert!(!any_scheduled, "scheduled backups are off by default");

    core.state
        .backups
        .set_policy(
            &core.uuid,
            BackupPolicy {
                enabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        core.state.backups.schedule_changed(),
    )
    .await
    .expect("saving a schedule wakes the scheduler");
    let (_, any_scheduled) = crate::backups::scheduler::tick(&core.state, &seen).await;
    assert!(any_scheduled);
}

/// A download whose request is dropped (the client went away) ends its progression and
/// stops its zip.
#[tokio::test]
async fn a_dropped_download_ends_its_progression() {
    let core = core().await;
    let mut events = core.state.event_broadcaster.subscribe();
    let (_, event_id) = Event::new_progression_event_start("Zipping", None, None, CausedBy::System);
    let cancel = core.state.backups.cancel_token().child_token();
    drop(Zipping {
        broadcaster: core.state.event_broadcaster.clone(),
        event_id: Some(event_id.clone()),
        cancel: cancel.clone(),
    });
    assert!(cancel.is_cancelled());
    assert!(!core.state.backups.cancel_token().is_cancelled());
    let mut ended = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd { success, .. } =
                p.progression_event_inner()
            {
                ended.push((p.event_id(), *success));
            }
        }
    }
    assert_eq!(ended, vec![(event_id.inner(), false)]);

    // one that finished sends its own End, once
    let zipping = Zipping {
        broadcaster: core.state.event_broadcaster.clone(),
        event_id: Some(event_id.clone()),
        cancel: core.state.backups.cancel_token().child_token(),
    };
    zipping.end(true, "Zipping complete");
    let mut ends = 0;
    while let Ok(event) = events.try_recv() {
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd { success, .. } =
                p.progression_event_inner()
            {
                assert!(*success);
                ends += 1;
            }
        }
    }
    assert_eq!(ends, 1);
}
