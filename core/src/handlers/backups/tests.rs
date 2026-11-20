//! The backup routes, against a core on a loopback port with a real (never started)
//! Minecraft instance restored from files in a temporary folder.

use std::collections::HashSet;
use std::path::PathBuf;

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::backups::RestoreScope;
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

    /// The server's image folder, which holds its backups' records and policy.
    fn server_backups(&self) -> PathBuf {
        self.dir
            .path()
            .join("backups/images")
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

/// Of what an export left out, a player is told only what a player would miss: never the
/// server software, the EULA, Lodestone's files or what a client view never carries
/// (plugins' data, the server's settings), however many there are.
#[test]
fn players_are_told_only_what_they_would_miss() {
    for kept in [
        "config/FabricProxy-Lite.toml",
        "mods/kept/a.jar",
        "saves/world/data/secrets.dat",
        "kubejs/server_scripts/token.js",
    ] {
        assert!(missed_by_players(kept), "{kept}");
    }
    for software in [
        ".lodestone_config",
        "config/.lodestone-marks.json",
        "eula.txt",
        "EULA.TXT",
        ".console_history",
        "fabric-server-launch.jar",
        "run.sh",
        "user_jvm_args.txt",
        "hs_err_pid123.log",
        "libraries/net/fabricmc/x.jar",
        "Versions/1.21.1/server.jar",
        // what a player's instance never takes: the server's settings, operators and
        // plugins' data
        "ops.json",
        "server.properties",
        "Banned-IPs.json",
        "forwarding.secret",
        "plugins/LuckPerms/config.yml",
        "PLUGINS/Essentials/userdata/0f0e.yml",
        "plugins/CoreProtect/database.db",
        "bukkit.yml",
        "usercache.json",
        "mods/old.jar.disabled",
    ] {
        assert!(!missed_by_players(software), "{software}");
    }
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
    assert!(core.server_backups().join("backup-policy.json").is_file());
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
    for uuid in ["..%2F..%2Fstores", "%2Ftmp%2Fx", "C:%5Cx"] {
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
    let store = core.state.backups.store().clone();
    core.state
        .backups
        .scheduled_backup(&core.state, instance.clone())
        .await
        .unwrap();
    assert_eq!(
        store.load_records(&core.uuid).unwrap().scheduled_since_full,
        0
    );
    // the 10th scheduled backup since the full scan is due to be one
    let mut records = store.load_records(&core.uuid).unwrap();
    records.scheduled_since_full = 9;
    store.save_records(&core.uuid, &records).unwrap();
    core.state
        .backups
        .scheduled_backup(&core.state, instance)
        .await
        .unwrap();
    assert_eq!(core.list(&owner).await.backups.len(), 1, "nothing changed");
    assert_eq!(
        store.load_records(&core.uuid).unwrap().scheduled_since_full,
        0
    );
}

/// The handler checks the stop and start permissions only for a server it sees running;
/// one that starts before the restore locks it is refused rather than stopped unchecked.
#[tokio::test]
async fn a_server_that_started_meanwhile_is_not_stopped() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
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
            backup,
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
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
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
            backup,
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
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
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
            backup,
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
        .load(&core.uuid, &safety.id)
        .await
        .unwrap();
    assert!(safety
        .image
        .entries
        .iter()
        .any(|e| e.path.as_str() == "plugins/new.jar"));
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
    drop(Preparing {
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
    let zipping = Preparing {
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

/// A backup named with the full 100 characters is restored with a safety backup: the safety
/// backup's name ("Before restoring “…”") is shortened to fit instead of being refused
/// after every file was copied, and so is the next one when that is restored in turn.
#[tokio::test]
async fn a_long_name_doesnt_stop_the_safety_backup() {
    let core = core().await;
    let owner = core.owner().await;
    let long = "n".repeat(100);
    let id = core.backup(&owner, Some(&long)).await;

    core.write("world/level.dat", "level v2");
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v1");
    let list = core.list(&owner).await;
    assert_eq!(list.backups.len(), 2, "with the safety backup");
    // both may be from the same second, so the safety backup isn't necessarily listed first
    let safety = list
        .backups
        .iter()
        .find(|b| b.kind == crate::backups::types::BackupKind::BeforeRestore)
        .unwrap();
    let name = safety.name.clone().unwrap();
    assert!(name.starts_with("Before restoring “nnn"), "{name}");
    assert!(name.ends_with("…”"), "{name}");
    assert_eq!(name.chars().count(), 100);

    // undo it: the safety backup of the safety backup fits too
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{}/restore", safety.id),
            &owner,
            Some(json!({ "scope": "world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("world/level.dat"), "level v2");
    assert_eq!(core.list(&owner).await.backups.len(), 3);
}

/// A file the restore would replace that the safety backup couldn't read would be deleted
/// for good, with no copy to undo it from: the restore is refused and the file stays.
#[cfg(unix)]
#[tokio::test]
async fn a_file_the_safety_backup_couldnt_read_stops_the_restore() {
    use std::os::unix::fs::PermissionsExt;

    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    core.write("world/level.dat", "level v2");
    core.write("world/data/secret.dat", "only copy");
    let secret = core.root.join("world/data/secret.dat");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&secret).is_ok() {
        // running as root, which reads every file
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(core.read("world/data/secret.dat"), "only copy");
    assert_eq!(
        core.read("world/level.dat"),
        "level v2",
        "nothing was restored"
    );
}

/// LS-97: the same when an older backup has the file: the safety backup only carries that
/// older copy over, so the file as it is now would still be lost.
#[cfg(unix)]
#[tokio::test]
async fn a_file_the_safety_backup_only_carried_over_stops_the_restore() {
    use std::os::unix::fs::PermissionsExt;

    let core = core().await;
    let owner = core.owner().await;
    core.write("world/data/x.dat", "old copy");
    let id = core.backup(&owner, None).await;
    core.write("world/level.dat", "level v2");
    core.write("world/data/x.dat", "the newer copy");
    let file = core.root.join("world/data/x.dat");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&file).is_ok() {
        // running as root, which reads every file
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(core.read("world/data/x.dat"), "the newer copy");
    assert_eq!(
        core.read("world/level.dat"),
        "level v2",
        "nothing was restored"
    );
}

/// Without a safety backup, a protected file the core can't read still counts in the
/// permission check: the restore would delete it, so someone who may not change protected
/// files is refused, as for one it can read.
#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_protected_file_still_needs_global_file_permission() {
    use std::os::unix::fs::PermissionsExt;

    let core = core().await;
    let owner = core.owner().await;
    let writer = core
        .member("writer", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
            p.can_write_instance_file.insert(core.uuid.clone());
        })
        .await;
    let id = core.backup(&owner, None).await;
    core.write("world/pack.jar", "jar");
    let jar = core.root.join("world/pack.jar");
    std::fs::set_permissions(&jar, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&jar).is_ok() {
        // running as root, which reads every file
        std::fs::set_permissions(&jar, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &writer,
            Some(json!({ "scope": "world", "safety_backup": false })),
        )
        .await;
    core.wait_idle(&owner).await;
    std::fs::set_permissions(&jar, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.to_string().contains("protected"), "{body}");
    assert_eq!(core.read("world/pack.jar"), "jar");
}

/// Every object, list and chunk of the store made old enough for a collection to take.
fn age_store(root: &std::path::Path) {
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
                    .set_modified(
                        std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 3600),
                    )
                    .unwrap();
            }
        }
    }
}

/// A backup is a data image linked to the server's content image (recorded first when the
/// server had none); "world and its mods" restores the level and that content, and the
/// restored content is recorded as a content image of its own.
#[tokio::test]
async fn the_world_and_its_mods_are_restored_and_recorded() {
    let core = core().await;
    let owner = core.owner().await;
    core.write("mods/a.jar", "mod a v1");
    let id = core.backup(&owner, Some("Base")).await;
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
    let current = core.state.images.current_content(&core.uuid).await;
    assert!(current.is_some());
    assert_eq!(backup.image.content, current);
    assert_eq!(backup.image.kind, lodestone_image::Kind::Data);
    let content = backup.content.clone().unwrap();
    assert!(content
        .entries
        .iter()
        .any(|e| e.path.as_str() == "mods/a.jar"));

    core.write("mods/a.jar", "mod a v2");
    core.write("mods/b.jar", "mod b");
    core.write("world/level.dat", "level v2");
    core.write("notes.txt", "notes");
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world_and_mods", "safety_backup": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.wait_idle(&owner).await;
    assert_eq!(core.read("mods/a.jar"), "mod a v1");
    assert!(!core.root.join("mods/b.jar").exists());
    assert_eq!(core.read("world/level.dat"), "level v1");
    assert_eq!(core.read("notes.txt"), "notes");
    let restored = core.state.images.current_content(&core.uuid).await.unwrap();
    let image = core
        .state
        .images
        .store()
        .load_manifest(core.uuid.as_ref(), &restored)
        .unwrap();
    assert!(image
        .label
        .as_deref()
        .is_some_and(|label| label.contains("Restored snapshot “Base”")));
    // the content is the backup's again, so it is the same image
    assert_eq!(image.id, content.id);

    // a backup with no world is refused for this scope before anything starts
    let mut records = core.state.backups.store().load_records(&core.uuid).unwrap();
    records.backups[0].level = None;
    core.state
        .backups
        .store()
        .save_records(&core.uuid, &records)
        .unwrap();
    let (status, _) = core
        .call(
            Method::POST,
            &format!("/{id}/restore"),
            &owner,
            Some(json!({ "scope": "world_and_mods" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Backups of a server that didn't change between them are one image; it goes with the
/// last of them.
#[tokio::test]
async fn backups_of_an_unchanged_server_share_one_image() {
    let core = core().await;
    let owner = core.owner().await;
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    core.state
        .backups
        .scheduled_backup(&core.state, instance)
        .await
        .unwrap();
    let named = core.backup(&owner, Some("Named")).await;
    let records = core.state.backups.store().load_records(&core.uuid).unwrap();
    assert_eq!(records.backups.len(), 2);
    let image = records.backups[0].image.clone();
    assert_eq!(records.backups[1].image, image);
    let store = core.state.images.store();
    let manifest = store.manifest_path(core.uuid.as_ref(), &image).unwrap();

    let (status, _) = core
        .call(Method::DELETE, &format!("/{named}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(manifest.is_file());
    let last = core.list(&owner).await.backups[0].id.clone();
    let (status, _) = core
        .call(Method::DELETE, &format!("/{last}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!manifest.is_file());
    // the content image the backups linked to is still the server's current one
    let current = core.state.images.current_content(&core.uuid).await.unwrap();
    assert!(store
        .manifest_path(core.uuid.as_ref(), &current)
        .unwrap()
        .is_file());
}

/// Collections running all through a backup never take a file it names: the backup holds
/// them off until its image is saved, and then its image names them.
#[tokio::test]
async fn collections_during_a_backup_never_take_its_files() {
    let core = core().await;
    let owner = core.owner().await;
    let first = core.backup(&owner, None).await;
    let store = core.state.images.store();
    age_store(store.root());
    core.write("world/level.dat", "level v2");
    // the first backup is deleted, so its files are free for a collection to take
    let (status, _) = core
        .call(Method::DELETE, &format!("/{first}"), &owner, None)
        .await;
    assert_eq!(status, StatusCode::OK);

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let collector = {
        let (stop, images) = (stop.clone(), core.state.images.clone());
        std::thread::spawn(move || {
            let mut runs = 0;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                images.collect_blocking().unwrap();
                runs += 1;
            }
            runs
        })
    };
    let instance = minecraft(&core.state, &core.uuid).unwrap();
    core.state
        .backups
        .scheduled_backup(&core.state, instance)
        .await
        .unwrap();
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(collector.join().unwrap() > 0);

    let id = core.list(&owner).await.backups[0].id.clone();
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
    assert!(lodestone_image::store::materialize::missing_objects(
        &store,
        &backup.image,
        lodestone_image::store::materialize::Selection::All
    )
    .is_empty());
    for entry in &backup.image.entries {
        store
            .verify_object(&entry.sha256, entry.size, lodestone_image::store::NEVER)
            .unwrap();
    }
}

/// An export whose request is dropped before the file is written (the client went away)
/// ends its progression through the route's own guard, and leaves no file behind.
#[tokio::test]
async fn a_dropped_export_ends_its_progression() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner, None).await;
    let user = User::new("exporter".into(), "pw", true, false, Default::default());
    core.state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    let mut events = core.state.event_broadcaster.subscribe();
    let mut export = Box::pin(export_backup(
        State(core.state.clone()),
        Path((core.uuid.clone(), id.clone())),
        AuthedUser(user),
        None,
    ));
    // the progression starts, then the export waits for the backup store
    assert!(futures::poll!(export.as_mut()).is_pending());
    drop(export);
    let mut started = None;
    let mut ended = Vec::new();
    while let Ok(event) = events.try_recv() {
        let crate::events::EventInner::ProgressionEvent(p) = event.event_inner else {
            continue;
        };
        match p.progression_event_inner() {
            crate::events::ProgressionEventInner::ProgressionStart { .. } => {
                started = Some(p.event_id())
            }
            crate::events::ProgressionEventInner::ProgressionEnd { success, .. } => {
                ended.push((p.event_id(), *success))
            }
            _ => {}
        }
    }
    let started = started.expect("the export's progression started");
    assert_eq!(ended, vec![(started, false)]);
    // the dropped request's export never writes its file
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let suffix = format!("{id}.{}", crate::images::share::EXTENSION);
    let written: Vec<_> = walkdir::WalkDir::new(path_to_tmp())
        .into_iter()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(&suffix))
        .collect();
    assert!(written.is_empty(), "{written:?}");
}
