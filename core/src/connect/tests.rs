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
    let executor = CodeExecutionSwitch::new(false);
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
    // nothing shared; the resume may still be trying, and stops once it is done
    assert!(shares
        .slots
        .lock()
        .await
        .values()
        .all(|slot| matches!(slot, Slot::Abandoned)));

    shares.stop(&core.fabric, true).await;
    assert!(!file.exists(), "stopping for good deletes the identity");
}

#[tokio::test]
async fn nothing_starts_sharing_once_the_core_is_shutting_down() {
    let core = core().await;
    let shares = &core.state.connect;
    shares
        .save(&core.fabric, &HostIdentity::generate().unwrap())
        .await
        .unwrap();
    let file = core
        .dir
        .path()
        .join("connect")
        .join(format!("{}.json", core.fabric));
    // as if a share were starting: the shutdown marks it, for the start to stop it once
    // it has its host, keeping the link
    shares
        .slots
        .lock()
        .await
        .insert(core.vanilla.clone(), Slot::Starting);
    shares.stop_all().await;
    assert!(matches!(
        shares.slots.lock().await.remove(&core.vanilla),
        Some(Slot::Abandoned)
    ));

    let error = shares.start(&core.state, &core.fabric).await.unwrap_err();
    assert!(
        error.source.to_string().contains("shutting down"),
        "{error}"
    );
    assert!(shares.slots.lock().await.is_empty());
    // a resume after the shutdown began starts nothing and keeps the link
    shares.resume_all(&core.state).await;
    tokio::task::yield_now().await;
    assert!(shares.slots.lock().await.is_empty());
    assert!(file.exists(), "the link is kept for the next start");
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

#[test]
fn the_service_override_is_checked_like_the_apps_setting() {
    assert_eq!(service_from(None), Service::default());
    assert_eq!(service_from(Some("  ")), Service::default());
    assert_eq!(
        service_from(Some("https://connect-dev.lodestone.cc/")).base_url(),
        "https://connect-dev.lodestone.cc"
    );
    assert_eq!(
        service_from(Some("http://127.0.0.1:8787")).base_url(),
        "http://127.0.0.1:8787"
    );
    // plain HTTP to another computer, a path, or no scheme: the default
    for bad in [
        "http://connect-dev.lodestone.cc",
        "https://connect-dev.lodestone.cc/v1",
        "connect-dev.lodestone.cc",
        "https://user@connect-dev.lodestone.cc",
    ] {
        assert_eq!(service_from(Some(bad)), Service::default(), "{bad}");
    }
}

// ---------------------------------------------------------------------------
// The friend list

impl Core {
    async fn call_json(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = reqwest::Client::new()
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token)
            .json(&body)
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

fn friend_key() -> String {
    lodestone_connect::friend::PersonKey::generate()
        .unwrap()
        .public()
        .to_hex()
}

#[tokio::test]
async fn the_friend_list_is_the_owners_and_only_moves_forward() {
    let core = core().await;
    let (status, body) = core
        .call(Method::GET, "/connect/friends", &core.owner_jwt)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["fingerprint"].as_str().unwrap().len(), 64);
    assert_eq!(body["version"], 0);
    assert_eq!(body["friends"], json!([]));
    assert_eq!(body["host"], Value::Null);

    let ben = friend_key();
    let secret = lodestone_connect::friend::Secret::random()
        .unwrap()
        .to_hex();
    let list = json!({
        "version": 1,
        "friends": [{"key": ben, "name": "Ben\u{202E}", "secret": secret, "shared": [core.fabric.to_string()]}]
    });
    let (status, body) = core
        .call_json(
            Method::PUT,
            "/connect/friends",
            &core.owner_jwt,
            list.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["version"], 1);
    assert_eq!(body["friends"][0]["key"], ben);
    assert_eq!(body["friends"][0]["name"], "Ben", "names are cleaned");
    assert_eq!(
        body["friends"][0]["shared"],
        json!([core.fabric.to_string()])
    );
    assert!(
        body["host"].is_object(),
        "something is shared: the friend host runs"
    );
    assert!(
        !body.to_string().contains(&secret),
        "secrets never come back"
    );

    // the same version again, or an older one
    let (status, _) = core
        .call_json(
            Method::PUT,
            "/connect/friends",
            &core.owner_jwt,
            list.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // someone who isn't the owner, even with every permission on the instance
    let mut lots = UserPermission::default();
    lots.can_view_instance.insert(core.fabric.clone());
    lots.can_access_instance_setting.insert(core.fabric.clone());
    let admin = add_user(&core.state, "helper", false, lots).await;
    let admin = admin.create_jwt().unwrap().to_string();
    let (status, _) = core.call(Method::GET, "/connect/friends", &admin).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let mut newer = list.clone();
    newer["version"] = json!(9);
    let (status, _) = core
        .call_json(Method::PUT, "/connect/friends", &admin, newer.clone())
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // bad entries
    for bad in [
        json!({"version": 5, "friends": [{"key": "zz", "name": "x", "secret": secret, "shared": []}]}),
        json!({"version": 5, "friends": [{"key": ben, "name": "x", "secret": "00", "shared": []}]}),
        json!({"version": 5, "friends": [{"key": ben, "name": "\u{200B}", "secret": secret, "shared": []}]}),
        json!({"version": 5, "friends": [
            {"key": ben, "name": "a", "secret": secret, "shared": []},
            {"key": ben, "name": "b", "secret": secret, "shared": []}
        ]}),
    ] {
        let (status, body) = core
            .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, bad)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    // saved for the core's user only, and read again after a restart
    let file = core
        .dir
        .path()
        .join("connect")
        .join("friends")
        .join("list.json");
    assert!(file.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&file.parent().unwrap().join("core.json")), 0o600);
    }
    let again = friends::FriendShares::new(
        core.dir.path().join("connect").join("friends"),
        lodestone_connect::Service::parse("http://127.0.0.1:9").unwrap(),
        lodestone_connect::GatherOptions {
            portmap: false,
            loopback_only: true,
        },
    );
    let view = again.view().await.unwrap();
    assert_eq!(view.version, 1);
    assert_eq!(view.friends.len(), 1);
    assert_eq!(view.fingerprint, body_fingerprint(&core).await);

    // nothing shared with anyone: the friend host stops
    let info = core
        .state
        .connect
        .friends
        .info_for_tests()
        .expect("started");
    let none = json!({"version": 2, "friends": [{"key": ben, "name": "Ben", "secret": secret, "shared": []}]});
    let (status, body) = core
        .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, none)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["host"], Value::Null);

    // sharing again starts a new friend host, but the same info server serves it
    let mut again = list.clone();
    again["version"] = json!(3);
    let (status, body) = core
        .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, again)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["host"].is_object());
    let info_again = core.state.connect.friends.info_for_tests().unwrap();
    assert!(info.same_channel(&info_again), "no second info server");
    core.state.connect.stop_all().await;
}

async fn body_fingerprint(core: &Core) -> String {
    let (_, body) = core
        .call(Method::GET, "/connect/friends", &core.owner_jwt)
        .await;
    body["fingerprint"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn api_keys_cannot_reach_the_friend_list() {
    let core = core().await;
    let (status, body) = core
        .call_json(
            Method::POST,
            "/user/api_keys",
            &core.owner_jwt,
            json!({ "name": "script", "expires_at": null, "scope": { "type": "full" } }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let key = body["key"].as_str().unwrap().to_string();
    let (status, _) = core.call(Method::GET, "/connect/friends", &key).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// `GET path` through `friend`'s info stream.
async fn friend_get(
    tx: &mpsc::Sender<(
        tokio::io::DuplexStream,
        lodestone_connect::friend::PublicKey,
    )>,
    friend: lodestone_connect::friend::PublicKey,
    path: &str,
) -> (u16, Vec<u8>) {
    let (mut near, far) = tokio::io::duplex(1 << 20);
    tx.send((far, friend)).await.unwrap();
    near.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").as_bytes(),
    )
    .await
    .unwrap();
    let mut answer = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), near.read_to_end(&mut answer))
        .await
        .unwrap()
        .unwrap();
    lodestone_connect::info::parse_response(&answer).unwrap()
}

#[tokio::test]
async fn friends_reach_only_what_is_shared_with_them() {
    let core = core().await;
    let ben = lodestone_connect::friend::PublicKey([1; 32]);
    let carl = lodestone_connect::friend::PublicKey([2; 32]);
    let access: friends::Access = Default::default();
    access
        .write()
        .unwrap()
        .insert(ben, [core.fabric.clone()].into_iter().collect());
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(friends::serve_friend_info(
        core.state.clone(),
        access.clone(),
        rx,
    ));

    let (status, body) = friend_get(&tx, ben, "/api/v1/friend/servers").await;
    assert_eq!(status, 200);
    let servers: Vec<SharedServer> = serde_json::from_slice(&body).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].instance_uuid, core.fabric.to_string());

    let (status, body) = friend_get(&tx, ben, "/api/v1/public/instances").await;
    assert_eq!(status, 200);
    let list: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(list["instances"].as_array().unwrap().len(), 1);
    assert!(list["instances"][0].get("port").is_none());

    let pack = format!("/api/v1/public/instances/{}/client-pack", core.fabric);
    assert_eq!(friend_get(&tx, ben, &pack).await.0, 200);

    // Carl has nothing shared with him
    let (status, body) = friend_get(&tx, carl, "/api/v1/friend/servers").await;
    assert_eq!(status, 200);
    assert_eq!(body, b"[]");
    assert_eq!(friend_get(&tx, carl, &pack).await.0, 404);

    // nothing unshared, and nothing else of the API
    for path in [
        format!("/api/v1/public/instances/{}/client-pack", core.vanilla),
        "/api/v1/connect/info".to_string(),
        "/api/v1/info".to_string(),
        "/api/v1/connect/friends".to_string(),
        format!("/api/v1/instance/{}/info", core.fabric),
    ] {
        assert_eq!(friend_get(&tx, ben, &path).await.0, 404, "{path}");
    }

    // unsharing takes effect on the next request
    access.write().unwrap().clear();
    assert_eq!(friend_get(&tx, ben, &pack).await.0, 404);
}

#[tokio::test]
async fn the_friend_list_refuses_forwarded_requests_and_too_many_sharing_friends() {
    let core = core().await;
    // a reverse proxy or tunnel on this computer may have passed it on from anywhere
    for header in [
        "x-forwarded-for",
        "forwarded",
        "x-real-ip",
        "cf-connecting-ip",
        "true-client-ip",
    ] {
        let response = reqwest::Client::new()
            .get(format!("{}/connect/friends", core.base))
            .bearer_auth(&core.owner_jwt)
            .header(header, "203.0.113.9")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{header}");
    }
    // without them, loopback is let in
    let (status, _) = core
        .call(Method::GET, "/connect/friends", &core.owner_jwt)
        .await;
    assert_eq!(status, StatusCode::OK);

    let secret = lodestone_connect::friend::Secret::random()
        .unwrap()
        .to_hex();
    let friends: Vec<Value> = (0..=friends::MAX_SHARING_FRIENDS)
        .map(|_| json!({"key": friend_key(), "name": "x", "secret": secret, "shared": [core.fabric.to_string()]}))
        .collect();
    let (status, body) = core
        .call_json(
            Method::PUT,
            "/connect/friends",
            &core.owner_jwt,
            json!({"version": 1, "friends": friends}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[test]
fn friend_entries_never_print_their_secret() {
    let entry = friends::FriendEntry {
        key: "ab".repeat(32),
        name: "Ben".into(),
        secret: "cd".repeat(32),
        shared: Vec::new(),
        hidden: false,
    };
    assert!(!format!("{entry:?}").contains(&"cd".repeat(32)));
}

#[tokio::test]
async fn hidden_friends_keep_their_shares_but_get_nothing() {
    let core = core().await;
    let secret = lodestone_connect::friend::Secret::random()
        .unwrap()
        .to_hex();
    let fabric = core.fabric.to_string();
    let (ben, carl) = (friend_key(), friend_key());
    let list = json!({"version": 1, "friends": [
        {"key": ben, "name": "Ben", "secret": secret, "shared": [fabric]},
        {"key": carl, "name": "Carl", "secret": secret, "shared": [fabric], "hidden": true}
    ]});
    let (status, body) = core
        .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, list)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["friends"][0]["hidden"], false);
    assert_eq!(body["friends"][1]["hidden"], true);
    assert_eq!(
        body["friends"][1]["shared"],
        json!([fabric]),
        "shares are kept"
    );
    assert!(body["host"].is_object(), "Ben has something shared");
    let carl_key = lodestone_connect::friend::PublicKey::from_hex(&carl).unwrap();
    let ben_key = lodestone_connect::friend::PublicKey::from_hex(&ben).unwrap();
    {
        let access = core.state.connect.friends.access_for_tests();
        let access = access.read().unwrap();
        assert!(access.contains_key(&ben_key));
        assert!(
            !access.contains_key(&carl_key),
            "no access, so no records or connections"
        );
    }

    // the info routes serve Carl nothing
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(friends::serve_friend_info(
        core.state.clone(),
        core.state.connect.friends.access_for_tests(),
        rx,
    ));
    let (status, body) = friend_get(&tx, carl_key, "/api/v1/friend/servers").await;
    assert_eq!(status, 200);
    assert_eq!(body, b"[]");
    let pack = format!("/api/v1/public/instances/{fabric}/client-pack");
    assert_eq!(friend_get(&tx, carl_key, &pack).await.0, 404);
    assert_eq!(friend_get(&tx, ben_key, &pack).await.0, 200);

    // only a hidden friend has anything shared: the friend host stops, the shares stay
    let list = json!({"version": 2, "friends": [
        {"key": ben, "name": "Ben", "secret": secret, "shared": []},
        {"key": carl, "name": "Carl", "secret": secret, "shared": [fabric], "hidden": true}
    ]});
    let (status, body) = core
        .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, list)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["host"], Value::Null);
    assert_eq!(body["friends"][1]["shared"], json!([fabric]));

    // kept across a restart
    let again = offline_shares(&core.dir.path().join("connect").join("friends"));
    let view = again.view().await.unwrap();
    assert!(view.friends[1].hidden);
    assert_eq!(view.friends[1].shared, vec![core.fabric.clone()]);

    // hidden friends don't count towards the friends a core shares with
    let mut many: Vec<Value> = (0..friends::MAX_SHARING_FRIENDS)
        .map(|_| json!({"key": friend_key(), "name": "x", "secret": secret, "shared": [fabric]}))
        .collect();
    many.push(
        json!({"key": carl, "name": "Carl", "secret": secret, "shared": [fabric], "hidden": true}),
    );
    let (status, body) = core
        .call_json(
            Method::PUT,
            "/connect/friends",
            &core.owner_jwt,
            json!({"version": 3, "friends": many}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    core.state.connect.stop_all().await;
}

#[tokio::test]
async fn only_the_owner_replaces_the_friend_certificate_and_the_host_serves_the_new_one() {
    let core = core().await;
    let secret = lodestone_connect::friend::Secret::random()
        .unwrap()
        .to_hex();
    let ben = friend_key();
    let list = json!({"version": 1, "friends": [
        {"key": ben, "name": "Ben", "secret": secret, "shared": [core.fabric.to_string()]}
    ]});
    let (status, body) = core
        .call_json(Method::PUT, "/connect/friends", &core.owner_jwt, list)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let old = body["fingerprint"].as_str().unwrap().to_string();
    let hex = |bytes: [u8; 32]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let host = core.state.connect.friends.host_fingerprint_for_tests();
    assert_eq!(hex(host.await.unwrap()), old);

    // someone who isn't the owner, or a request a local proxy passed on: refused
    let mut lots = UserPermission::default();
    lots.can_view_instance.insert(core.fabric.clone());
    lots.can_access_instance_setting.insert(core.fabric.clone());
    let helper = add_user(&core.state, "helper", false, lots).await;
    let helper = helper.create_jwt().unwrap().to_string();
    let path = "/connect/friends/certificate";
    let (status, _) = core.call(Method::POST, path, &helper).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let forwarded = reqwest::Client::new()
        .post(format!("{}{path}", core.base))
        .bearer_auth(&core.owner_jwt)
        .header("x-forwarded-for", "203.0.113.9")
        .send()
        .await
        .unwrap();
    assert_eq!(forwarded.status(), StatusCode::FORBIDDEN);
    assert_eq!(body_fingerprint(&core).await, old, "unchanged");

    // the owner: a new certificate, the list and shares kept, the host restarted with it
    let (status, body) = core.call(Method::POST, path, &core.owner_jwt).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let new = body["fingerprint"].as_str().unwrap().to_string();
    assert_eq!(new.len(), 64);
    assert_ne!(new, old);
    assert_eq!(body["version"], 1);
    assert_eq!(body["friends"][0]["key"], ben);
    assert_eq!(
        body["friends"][0]["shared"],
        json!([core.fabric.to_string()])
    );
    assert!(body["host"].is_object());
    let host = core.state.connect.friends.host_fingerprint_for_tests();
    assert_eq!(
        hex(host.await.unwrap()),
        new,
        "the old certificate is no longer served"
    );
    assert_eq!(body_fingerprint(&core).await, new);

    // the old one is kept aside, and the new one is what a restart reads
    let dir = core.dir.path().join("connect").join("friends");
    let retired: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("core.json.retired-")
        })
        .collect();
    assert_eq!(retired.len(), 1);
    let kept = std::fs::read_to_string(&retired[0]).unwrap();
    let kept = lodestone_connect::friend::host::CoreIdentity::from_json(&kept).unwrap();
    assert_eq!(hex(kept.fingerprint()), old);
    let view = offline_shares(&dir).view().await.unwrap();
    assert_eq!(view.fingerprint, new);

    // again: each old one is kept
    let (status, body) = core.call(Method::POST, path, &core.owner_jwt).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_ne!(body["fingerprint"].as_str().unwrap(), new);
    let count = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("core.json.retired-")
        })
        .count();
    assert_eq!(count, 2);
    core.state.connect.stop_all().await;
}

fn offline_shares(dir: &Path) -> friends::FriendShares {
    friends::FriendShares::new(
        dir.to_path_buf(),
        lodestone_connect::Service::parse("http://127.0.0.1:9").unwrap(),
        lodestone_connect::GatherOptions {
            portmap: false,
            loopback_only: true,
        },
    )
}

#[tokio::test]
async fn a_damaged_friend_certificate_is_kept_aside_and_an_unreadable_one_is_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let friends_dir = dir.path().join("friends");
    std::fs::create_dir_all(&friends_dir).unwrap();

    // damaged: moved aside, and a new one made
    std::fs::write(friends_dir.join("core.json"), "{not json").unwrap();
    let made = offline_shares(&friends_dir).identity().await.unwrap();
    let backups: Vec<String> = std::fs::read_dir(&friends_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with("core.json.damaged-"))
        .collect();
    assert_eq!(backups.len(), 1, "{backups:?}");
    assert_eq!(
        std::fs::read_to_string(friends_dir.join(&backups[0])).unwrap(),
        "{not json"
    );
    // the new one is saved, and read back as it is
    let again = offline_shares(&friends_dir).identity().await.unwrap();
    assert_eq!(again.fingerprint(), made.fingerprint());

    // unreadable (here, a folder in its place): an error, and nothing replaced
    let other = dir.path().join("other");
    std::fs::create_dir_all(other.join("core.json")).unwrap();
    let Err(err) = offline_shares(&other).identity().await else {
        panic!("an error");
    };
    assert!(
        format!("{err:?}").contains("Couldn't read the friend certificate"),
        "{err:?}"
    );
    assert!(other.join("core.json").is_dir(), "left as it was");
}

#[tokio::test]
async fn a_friend_host_that_cannot_start_is_saved_reported_and_tried_again() {
    let core = core().await;
    let dir = tempfile::tempdir().unwrap();
    // the first two starts fail, as while offline
    let shares = Arc::new(
        offline_shares(&dir.path().join("friends"))
            .failing_for_tests(2, Duration::from_millis(200)),
    );
    let secret = lodestone_connect::friend::Secret::random()
        .unwrap()
        .to_hex();
    let list: friends::FriendList = serde_json::from_value(json!({
        "version": 1,
        "friends": [{"key": friend_key(), "name": "Ben", "secret": secret, "shared": [core.fabric.to_string()]}]
    }))
    .unwrap();

    // saved, and the problem shows in the host status
    let view = shares.replace(&core.state, list).await.unwrap();
    assert_eq!(view.version, 1);
    let host = view.host.expect("a status while it's tried again");
    assert_eq!(host.state, lodestone_connect::host::HostState::Reconnecting);
    assert!(host.problem.unwrap().contains("Couldn't start sharing"));
    assert!(dir.path().join("friends").join("list.json").exists());

    // tried again in the background until it starts
    let started = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let view = shares.view().await.unwrap();
            // the running host's own status (it can't reach the service here either)
            let problem = view.host.expect("a status").problem.unwrap_or_default();
            if !problem.contains("Couldn't start sharing") {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(started.is_ok(), "the friend host started on a retry");
    shares.stop().await;
}
