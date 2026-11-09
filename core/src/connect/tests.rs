//! Sharing with friends, without the network: the info routes over in-memory streams,
//! the share endpoints against a service nothing answers on, and the saved identities.

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::{CodeExecutionSwitch, GlobalSettingsData},
    macro_executor::MacroExecutor,
    test_util::{serve_api, test_app_state},
    types::DotLodestoneConfig,
};

/// Restoring a Minecraft instance looks up the Lodestone paths (for the Java runtimes).
fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

fn write_instance(
    root: &Path,
    folder: &str,
    name: &str,
    flavour: Value,
    port: u32,
) -> InstanceUuid {
    let dir = root.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let uuid = InstanceUuid::default();
    std::fs::write(
        dir.join(".lodestone_config"),
        serde_json::to_string(&DotLodestoneConfig::new(
            uuid.clone(),
            crate::traits::t_configurable::GameType::MinecraftJava,
        ))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join(".lodestone_minecraft_config.json"),
        json!({
            "name": name, "version": "1.20.1", "flavour": flavour,
            "description": "", "cmd_args": [], "java_cmd": "java",
            "port": port, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 17,
            "has_started": false
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        dir.join("server.properties"),
        format!("server-port={port}\n"),
    )
    .unwrap();
    uuid
}

struct Core {
    dir: tempfile::TempDir,
    state: AppState,
    base: String,
    owner_jwt: String,
    fabric: InstanceUuid,
    vanilla: InstanceUuid,
}

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
    let fabric = write_instance(
        &instances,
        "fabric",
        "Fabric server",
        json!({"fabric": {"loader_version": "0.16.14", "installer_version": "1.0.1"}}),
        25580,
    );
    let vanilla = write_instance(
        &instances,
        "vanilla",
        "Vanilla server",
        json!("vanilla"),
        25581,
    );
    let mods = instances.join("fabric").join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    crate::mods::metadata::tests::make_zip(
        &mods.join("client-thing.jar"),
        &[(
            "fabric.mod.json",
            r#"{"id":"thing","environment":"client"}"#,
        )],
    );
    let executor = MacroExecutor::new(
        state.event_broadcaster.clone(),
        tokio::runtime::Handle::current(),
        CodeExecutionSwitch::new(false),
    );
    let restored = crate::restore_instances(&instances, state.event_broadcaster.clone(), executor)
        .await
        .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    let owner = add_user(&state, "owner", true, UserPermission::default()).await;
    let owner_jwt = owner.create_jwt().unwrap().to_string();
    let base = serve_api(state.clone()).await;
    Core {
        dir,
        state,
        base,
        owner_jwt,
        fabric,
        vanilla,
    }
}

async fn add_user(
    state: &AppState,
    name: &str,
    is_owner: bool,
    permissions: UserPermission,
) -> User {
    let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    user
}

impl Core {
    async fn call(&self, method: Method, path: &str, token: &str) -> (StatusCode, Value) {
        let response = reqwest::Client::new()
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }
}

/// `GET path` through an info stream of the shared `uuid`.
async fn info_get(tx: &mpsc::Sender<tokio::io::DuplexStream>, path: &str) -> (u16, Vec<u8>) {
    let (mut friend, host) = tokio::io::duplex(1 << 20);
    tx.send(host).await.unwrap();
    friend
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), friend.read_to_end(&mut answer))
        .await
        .unwrap()
        .unwrap();
    lodestone_connect::info::parse_response(&answer).unwrap()
}

#[tokio::test]
async fn info_streams_reach_only_the_shared_instances_routes() {
    let core = core().await;
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(serve_info(core.state.clone(), core.fabric.clone(), rx));

    let (status, body) = info_get(&tx, "/api/v1/connect/info").await;
    assert_eq!(status, 200);
    let server: SharedServer = serde_json::from_slice(&body).unwrap();
    assert_eq!(server.instance_uuid, core.fabric.to_string());
    assert_eq!(server.name, "Fabric server");
    assert_eq!(server.game_version, "1.20.1");
    assert_eq!(server.loader, "fabric");
    assert_eq!(server.loader_version.as_deref(), Some("0.16.14"));
    assert!(
        server.modpack,
        "a Fabric server with a mod has a client pack"
    );
    assert_eq!(server.state.as_deref(), Some("Stopped"));
    assert_eq!(server.core_uuid, core.state.uuid);

    // the instance alone, without its address
    let (status, body) = info_get(&tx, "/api/v1/public/instances").await;
    assert_eq!(status, 200);
    let list: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list["instances"].as_array().unwrap().len(), 1);
    assert_eq!(list["instances"][0]["uuid"], core.fabric.to_string());
    assert!(list["instances"][0].get("port").is_none());

    // its client pack, as the launcher fetches it
    let (status, _) = info_get(
        &tx,
        &format!("/api/v1/public/instances/{}/client-pack", core.fabric),
    )
    .await;
    assert_eq!(status, 200);

    // nothing about the other instance, and nothing else of the API
    for path in [
        format!("/api/v1/public/instances/{}/client-pack", core.vanilla),
        "/api/v1/info".to_string(),
        "/api/v1/user/info".to_string(),
        "/api/v1/instance/list".to_string(),
        format!("/api/v1/instance/{}/info", core.fabric),
        format!("/api/v1/launcher/instances/{}/client-pack", core.fabric),
        "/api/v1/connect/info/../../user/info".to_string(),
    ] {
        let (status, _) = info_get(&tx, &path).await;
        assert_eq!(status, 404, "{path}");
    }
}

#[tokio::test]
async fn sharing_needs_the_settings_permission() {
    let core = core().await;
    let path = format!("/instance/{}/share", core.fabric);
    let (status, body) = core.call(Method::GET, &path, &core.owner_jwt).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sharing"], false);
    assert_eq!(body["link"], Value::Null);

    let mut can_view = UserPermission::default();
    can_view.can_view_instance.insert(core.fabric.clone());
    let viewer = add_user(&core.state, "viewer", false, can_view).await;
    let viewer = viewer.create_jwt().unwrap().to_string();
    for method in [Method::GET, Method::POST, Method::DELETE] {
        let (status, _) = core.call(method.clone(), &path, &viewer).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method}");
    }
    let (status, _) = core
        .call(Method::GET, "/instance/nope/share", &core.owner_jwt)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_share_that_cannot_reach_the_service_fails_and_saves_nothing() {
    let core = core().await;
    let path = format!("/instance/{}/share", core.fabric);
    let (status, body) = core.call(Method::POST, &path, &core.owner_jwt).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    assert!(
        body.to_string().contains("Couldn't start sharing"),
        "{body}"
    );
    let (_, body) = core.call(Method::GET, &path, &core.owner_jwt).await;
    assert_eq!(body["sharing"], false);
    assert!(!core
        .dir
        .path()
        .join("connect")
        .join(format!("{}.json", core.fabric))
        .exists());

    // stopping a share that isn't running is fine
    let (status, body) = core.call(Method::DELETE, &path, &core.owner_jwt).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sharing"], false);
}

#[tokio::test]
async fn identities_are_saved_for_the_core_user_only_and_resumed_for_existing_instances() {
    let core = core().await;
    let shares = &core.state.connect;
    let identity = HostIdentity::generate().unwrap();
    shares.save(&core.fabric, &identity).await.unwrap();
    let file = core
        .dir
        .path()
        .join("connect")
        .join(format!("{}.json", core.fabric));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(file.parent().unwrap()), 0o700);
    }
    let loaded = shares.load(&core.fabric).await.unwrap();
    assert_eq!(loaded.fingerprint(), identity.fingerprint());

    // a saved identity of an instance that is gone is deleted at startup
    let gone = InstanceUuid::default();
    shares.save(&gone, &identity).await.unwrap();
    shares.resume_all(&core.state).await;
    assert!(!core
        .dir
        .path()
        .join("connect")
        .join(format!("{gone}.json"))
        .exists());
    assert!(
        file.exists(),
        "kept for an instance that exists (resuming it retries)"
    );

    // shutting down keeps the identity, to resume with the same link
    shares.stop_all().await;
    assert!(file.exists(), "kept when the core shuts down");
    assert!(shares.slots.lock().await.is_empty());

    shares.stop(&core.fabric, true).await;
    assert!(!file.exists(), "stopping for good deletes the identity");
}

#[test]
fn game_streams_reach_the_port_only_while_the_server_runs() {
    // Tests run in debug builds, where LODESTONE_CONNECT_ANY_STATE would open the port.
    assert!(std::env::var_os(ANY_STATE_ENV).is_none());
    assert!(game_port_open(ServerState::Running));
    for state in [
        ServerState::Starting,
        ServerState::Stopping,
        ServerState::Stopped,
        ServerState::Error,
    ] {
        assert!(!game_port_open(state), "{state:?}");
    }
}
