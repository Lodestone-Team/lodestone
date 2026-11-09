//! The public and launcher instance routes, against a core on a loopback port with real
//! (never started) Minecraft instances restored from files in a temporary folder.

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::permission::UserPermission,
    events::CausedBy,
    global_settings::{CodeExecutionSwitch, GlobalSettingsData},
    macro_executor::MacroExecutor,
    test_util::{serve_api, test_app_state},
    types::DotLodestoneConfig,
};

/// Restoring a Minecraft instance looks up the Lodestone paths (for the Java runtimes).
/// They are set once for the test run, to a folder that is never dropped, as in
/// `atoms_and_macros_on_disk_do_not_stop_startup`.
fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

struct Core {
    _dir: tempfile::TempDir,
    state: AppState,
    base: String,
    client: reqwest::Client,
    owner_jwt: String,
    fabric: InstanceUuid,
    vanilla: InstanceUuid,
}

/// Writes an instance's config files. `flavour` is the `flavour` JSON value.
fn write_instance(
    root: &std::path::Path,
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
            "description": "a private description", "cmd_args": [], "java_cmd": "java",
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
        25570,
    );
    let vanilla = write_instance(
        &instances,
        "vanilla",
        "Vanilla server",
        json!("vanilla"),
        25571,
    );
    // a client mod that is not on Modrinth, so it goes into the pack's overrides
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
    let restored = crate::restore_instances(
        &instances,
        &state.instance_folders,
        state.event_broadcaster.clone(),
        executor,
    )
    .await
    .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    assert_eq!(state.instances.len(), 2);
    let owner = add_user(&state, "owner", true, false, Default::default()).await;
    let owner_jwt = owner.create_jwt().unwrap().to_string();
    let base = serve_api(state.clone()).await;
    Core {
        _dir: dir,
        state,
        base,
        client: reqwest::Client::new(),
        owner_jwt,
        fabric,
        vanilla,
    }
}

async fn add_user(
    state: &AppState,
    name: &str,
    is_owner: bool,
    is_admin: bool,
    permissions: UserPermission,
) -> User {
    let user = User::new(name.to_string(), "pw", is_owner, is_admin, permissions);
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
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self.client.request(method, format!("{}{path}", self.base));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
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

    async fn public_list(&self) -> Value {
        let (status, body) = self
            .call(Method::GET, "/public/instances", None, None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn set_switch(&self, on: bool) {
        let (status, body) = self
            .call(
                Method::PUT,
                "/global_settings/list_instances_publicly",
                Some(&self.owner_jwt),
                Some(json!(on)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    async fn set_listed(&self, uuid: &InstanceUuid, listed: bool) -> (StatusCode, Value) {
        self.call(
            Method::PUT,
            &format!("/instance/{uuid}/public_listing"),
            Some(&self.owner_jwt),
            Some(json!(listed)),
        )
        .await
    }

    async fn set_scope(&self, uuid: &InstanceUuid, change: Value) -> (StatusCode, Value) {
        self.call(
            Method::PUT,
            &format!("/instance/{uuid}/public_listing"),
            Some(&self.owner_jwt),
            Some(change),
        )
        .await
    }

    /// The public entry of `uuid`, which must be listed.
    async fn public_entry(&self, uuid: &InstanceUuid) -> Value {
        let list = self.public_list().await;
        list["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["uuid"] == uuid.to_string())
            .unwrap_or_else(|| panic!("{uuid} is not in {list}"))
            .clone()
    }

    async fn create_key(&self, token: &str, scope: Value) -> String {
        let (status, body) = self
            .call(
                Method::POST,
                "/user/api_keys",
                Some(token),
                Some(json!({ "name": "Lodestone Launcher on Test Mac", "expires_at": null, "scope": scope })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["key"].as_str().unwrap().to_string()
    }

    fn uuids(list: &Value) -> Vec<String> {
        list["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["uuid"].as_str().unwrap().to_string())
            .collect()
    }
}

#[tokio::test]
async fn nothing_is_public_by_default() {
    let core = core().await;
    let list = core.public_list().await;
    assert_eq!(list["listing_enabled"], false);
    assert_eq!(list["instances"], json!([]));
    assert!(list["core_name"].is_string());

    // listing an instance needs the switch
    let (status, body) = core.set_listed(&core.fabric, true).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.to_string().contains("turned off in Core Settings"));

    // the pack is not public either, with the same reply as for a missing instance
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/public/instances/{}/client-pack", core.fabric),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = core
        .call(
            Method::GET,
            "/public/instances/nope/client-pack",
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // the switch on, but nothing listed yet
    core.set_switch(true).await;
    let list = core.public_list().await;
    assert_eq!(list["listing_enabled"], true);
    assert_eq!(list["instances"], json!([]));
}

#[tokio::test]
async fn only_listed_instances_are_shown_while_the_switch_is_on() {
    let core = core().await;
    core.set_switch(true).await;
    let (status, body) = core.set_listed(&core.fabric, true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = core
        .call(
            Method::GET,
            &format!("/instance/{}/public_listing", core.fabric),
            Some(&core.owner_jwt),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    // listed with the defaults: status and address, not the client pack
    assert_eq!(
        body,
        json!({
            "listed": true, "status": true, "address": true, "client_pack": false,
            "list_instances_publicly": true
        })
    );

    let list = core.public_list().await;
    assert_eq!(Core::uuids(&list), vec![core.fabric.to_string()]);
    let entry = &list["instances"][0];
    assert_eq!(entry["name"], "Fabric server");
    assert_eq!(entry["game_version"], "1.20.1");
    assert_eq!(entry["loader"], "fabric");
    assert_eq!(entry["loader_version"], "0.16.14");
    assert_eq!(entry["port"], 25570);
    assert_eq!(entry["state"], "Stopped");
    assert_eq!(entry["playit_address"], Value::Null);
    assert_eq!(entry["listed"], true);

    // only the fields of the contract: no paths, description, owner or settings
    assert_eq!(
        keys_of(entry),
        [
            "game_version",
            "listed",
            "loader",
            "loader_version",
            "max_player_count",
            "name",
            "player_count",
            "playit_address",
            "port",
            "state",
            "uuid"
        ]
    );
    let text = list.to_string();
    assert!(!text.contains("private description"));
    assert!(!text.contains(core._dir.path().to_str().unwrap()));
    assert!(!text.contains("owner"));

    // turning the switch off hides it at once, though it stays listed
    core.set_switch(false).await;
    let list = core.public_list().await;
    assert_eq!(list["instances"], json!([]));
    core.set_switch(true).await;
    assert_eq!(
        Core::uuids(&core.public_list().await),
        vec![core.fabric.to_string()]
    );

    // unlisting works with or without the switch
    let (status, _) = core.set_listed(&core.fabric, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.public_list().await["instances"], json!([]));
}

/// The keys of a JSON object, sorted.
fn keys_of(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

const MINIMUM: [&str; 6] = [
    "game_version",
    "listed",
    "loader",
    "loader_version",
    "name",
    "uuid",
];

#[tokio::test]
async fn each_scope_flag_shows_exactly_its_fields() {
    let core = core().await;
    core.set_switch(true).await;
    let (status, body) = core
        .set_scope(
            &core.fabric,
            json!({ "listed": true, "status": false, "address": false, "client_pack": false }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "listed": true, "status": false, "address": false, "client_pack": false })
    );
    let entry = core.public_entry(&core.fabric).await;
    assert_eq!(keys_of(&entry), MINIMUM);

    for (flag, fields) in [
        ("status", vec!["max_player_count", "player_count", "state"]),
        ("address", vec!["playit_address", "port"]),
        ("client_pack", vec!["has_client_pack"]),
    ] {
        let (status, body) = core.set_scope(&core.fabric, json!({ flag: true })).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let entry = core.public_entry(&core.fabric).await;
        let mut expected: Vec<String> = MINIMUM.iter().map(|k| k.to_string()).collect();
        expected.extend(fields.iter().map(|k| k.to_string()));
        expected.sort();
        assert_eq!(keys_of(&entry), expected, "{flag}");
        core.set_scope(&core.fabric, json!({ flag: false })).await;
    }

    // the values when shown: counts and the playit address are null when unknown
    core.set_scope(
        &core.fabric,
        json!({ "status": true, "address": true, "client_pack": true }),
    )
    .await;
    let entry = core.public_entry(&core.fabric).await;
    assert_eq!(entry["state"], "Stopped");
    assert_eq!(entry["port"], 25570);
    assert_eq!(entry["playit_address"], Value::Null);
    assert_eq!(entry["has_client_pack"], true);
    assert!(entry.as_object().unwrap().contains_key("player_count"));

    // a launcher key sees everything, whatever the public scope
    core.set_scope(
        &core.fabric,
        json!({ "status": false, "address": false, "client_pack": false }),
    )
    .await;
    let key = core
        .create_key(&core.owner_jwt, json!({ "type": "launcher" }))
        .await;
    let (_, list) = core
        .call(Method::GET, "/launcher/instances", Some(&key), None)
        .await;
    for entry in list["instances"].as_array().unwrap() {
        assert_eq!(
            keys_of(entry),
            [
                "game_version",
                "has_client_pack",
                "listed",
                "loader",
                "loader_version",
                "max_player_count",
                "name",
                "player_count",
                "playit_address",
                "port",
                "state",
                "uuid"
            ]
        );
    }

    // unlisting keeps the scope for the next listing
    core.set_listed(&core.fabric, false).await;
    assert_eq!(core.public_list().await["instances"], json!([]));
    core.set_listed(&core.fabric, true).await;
    assert_eq!(keys_of(&core.public_entry(&core.fabric).await), MINIMUM);

    // the owner's overview: every Minecraft instance and its scope
    let (status, overview) = core
        .call(Method::GET, "/public_listing", Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["list_instances_publicly"], true);
    assert_eq!(
        overview["instances"],
        json!([
            {
                "uuid": core.fabric.to_string(), "name": "Fabric server",
                "listed": true, "status": false, "address": false, "client_pack": false
            },
            {
                "uuid": core.vanilla.to_string(), "name": "Vanilla server",
                "listed": false, "status": true, "address": true, "client_pack": false
            }
        ])
    );

    // unknown flags are refused
    let (status, _) = core
        .set_scope(&core.fabric, json!({ "players": true }))
        .await;
    assert!(status.is_client_error(), "{status}");
}

#[tokio::test]
async fn only_the_owner_changes_what_is_public() {
    let core = core().await;
    core.set_switch(true).await;
    let all: std::collections::HashSet<InstanceUuid> =
        [core.fabric.clone(), core.vanilla.clone()].into();
    let admin = add_user(&core.state, "admin", false, true, Default::default()).await;
    let settings_member = add_user(
        &core.state,
        "settings",
        false,
        false,
        UserPermission {
            can_view_instance: all.clone(),
            can_access_instance_setting: all.clone(),
            ..Default::default()
        },
    )
    .await;
    let viewer = add_user(
        &core.state,
        "viewer",
        false,
        false,
        UserPermission {
            can_view_instance: [core.fabric.clone()].into(),
            ..Default::default()
        },
    )
    .await;
    let owner_key = core
        .create_key(&core.owner_jwt, json!({ "type": "launcher" }))
        .await;
    let mut tokens = vec![("owner's launcher key", owner_key)];
    for (who, user) in [
        ("admin", &admin),
        ("member with settings access", &settings_member),
        ("viewer", &viewer),
    ] {
        tokens.push((who, user.create_jwt().unwrap().to_string()));
    }
    for (who, token) in &tokens {
        for body in [
            json!(true),
            json!(false),
            json!({ "listed": true }),
            json!({ "client_pack": true }),
        ] {
            let (status, reply) = core
                .call(
                    Method::PUT,
                    &format!("/instance/{}/public_listing", core.fabric),
                    Some(token),
                    Some(body.clone()),
                )
                .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{who} {body}: {reply}");
        }
        // nor the overview, nor the global switch
        let (status, _) = core
            .call(Method::GET, "/public_listing", Some(token), None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}");
        let (status, _) = core
            .call(
                Method::PUT,
                "/global_settings/list_instances_publicly",
                Some(token),
                Some(json!(false)),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{who}");
    }
    assert_eq!(
        core.state.public_listing.scope(&core.fabric),
        PublicScope::default()
    );
    assert!(core
        .state
        .global_settings
        .lock()
        .await
        .list_instances_publicly());

    // anyone who sees the instance sees its scope, read-only
    let viewer_jwt = &tokens[3].1;
    core.set_listed(&core.fabric, true).await;
    let (status, body) = core
        .call(
            Method::GET,
            &format!("/instance/{}/public_listing", core.fabric),
            Some(viewer_jwt),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["listed"], true);
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/instance/{}/public_listing", core.vanilla),
            Some(viewer_jwt),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // the owner may unlist with the switch off, but not list
    core.set_switch(false).await;
    let (status, _) = core.set_listed(&core.vanilla, true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .set_scope(&core.vanilla, json!({ "status": false }))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core.set_listed(&core.fabric, false).await;
    assert_eq!(status, StatusCode::OK);
}

/// Parses a client pack and returns its index.
fn read_pack(bytes: &[u8]) -> (Value, Vec<String>) {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut names = Vec::new();
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().unwrap().to_string());
    }
    let mut index = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("modrinth.index.json").unwrap(), &mut index)
        .unwrap();
    (serde_json::from_str(&index).unwrap(), names)
}

#[tokio::test]
async fn the_client_pack_is_public_only_when_listed() {
    let core = core().await;
    core.set_switch(true).await;
    let url = format!("{}/public/instances/{}/client-pack", core.base, core.fabric);
    assert_eq!(
        core.client.get(&url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    core.set_listed(&core.fabric, true).await;
    // listed, but the client pack is not shared by default: the same 404
    let (status, body) = core
        .call(
            Method::GET,
            &format!("/public/instances/{}/client-pack", core.fabric),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        core.call(
            Method::GET,
            "/public/instances/nope/client-pack",
            None,
            None
        )
        .await
        .1
    );
    assert!(!core
        .public_entry(&core.fabric)
        .await
        .as_object()
        .unwrap()
        .contains_key("has_client_pack"));
    core.set_scope(&core.fabric, json!({ "client_pack": true }))
        .await;
    let response = core.client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[CONTENT_TYPE],
        "application/x-modrinth-modpack+zip"
    );
    assert_eq!(response.headers()["x-lodestone-included-jars"], "1");
    let etag = response.headers()[ETAG].to_str().unwrap().to_string();
    let bytes = response.bytes().await.unwrap();
    let (index, names) = read_pack(&bytes);
    assert_eq!(index["dependencies"]["fabric-loader"], "0.16.14");
    assert_eq!(index["dependencies"]["minecraft"], "1.20.1");
    assert!(names.contains(&"overrides/mods/client-thing.jar".to_string()));
    // nothing but the index and the jars
    assert!(names
        .iter()
        .all(|n| n == "modrinth.index.json" || n.starts_with("overrides/mods/")));

    // the same pack again: 304 with the ETag, and the same ETag without it
    let again = core
        .client
        .get(&url)
        .header(IF_NONE_MATCH, &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    let again = core.client.get(&url).send().await.unwrap();
    assert_eq!(again.headers()[ETAG].to_str().unwrap(), etag);

    // a changed mods folder makes a new pack
    let mods = core._dir.path().join("instances/fabric/mods");
    crate::mods::metadata::tests::make_zip(
        &mods.join("second.jar"),
        &[(
            "fabric.mod.json",
            r#"{"id":"second","environment":"client"}"#,
        )],
    );
    let changed = core
        .client
        .get(&url)
        .header(IF_NONE_MATCH, &etag)
        .send()
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);
    assert_ne!(changed.headers()[ETAG].to_str().unwrap(), etag);
    assert_eq!(changed.headers()["x-lodestone-included-jars"], "2");

    // a vanilla server has no pack
    core.set_scope(
        &core.vanilla,
        json!({ "listed": true, "client_pack": true }),
    )
    .await;
    let (status, body) = core
        .call(
            Method::GET,
            &format!("/public/instances/{}/client-pack", core.vanilla),
            None,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.to_string().contains("only available for Fabric"));

    // the client pack unshared: gone
    core.set_scope(&core.fabric, json!({ "client_pack": false }))
        .await;
    assert_eq!(
        core.client.get(&url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    // and unlisted with it shared: gone too
    core.set_scope(&core.fabric, json!({ "client_pack": true }))
        .await;
    core.set_listed(&core.fabric, false).await;
    assert_eq!(
        core.client.get(&url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    // and with the switch off
    core.set_listed(&core.fabric, true).await;
    core.set_switch(false).await;
    assert_eq!(
        core.client.get(&url).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn launcher_keys_view_start_and_stop_only() {
    let core = core().await;
    let key = core
        .create_key(&core.owner_jwt, json!({ "type": "launcher" }))
        .await;

    // the owner's launcher key sees every instance, listed or not
    let (status, list) = core
        .call(Method::GET, "/launcher/instances", Some(&key), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let mut uuids = Core::uuids(&list);
    uuids.sort();
    let mut expected = vec![core.fabric.to_string(), core.vanilla.to_string()];
    expected.sort();
    assert_eq!(uuids, expected);
    assert_eq!(list["listing_enabled"], false);
    assert!(list["instances"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["listed"] == false));

    // the pack of an unlisted instance, with the key
    let response = core
        .client
        .get(format!(
            "{}/launcher/instances/{}/client-pack",
            core.base, core.fabric
        ))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // views the instance
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/instance/{}/info", core.fabric),
            Some(&key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // refused: settings, listing, console, files, users, global settings, keys
    for (method, path, body) in [
        (
            Method::PUT,
            format!("/instance/{}/name", core.fabric),
            Some(json!("renamed")),
        ),
        (
            Method::PUT,
            format!("/instance/{}/public_listing", core.fabric),
            Some(json!(true)),
        ),
        (
            Method::GET,
            format!("/instance/{}/settings", core.fabric),
            None,
        ),
        (
            Method::POST,
            format!("/instance/{}/console", core.fabric),
            Some(json!("op someone")),
        ),
        (Method::GET, format!("/instance/{}/mods", core.fabric), None),
        (Method::GET, "/user/list".to_string(), None),
        (
            Method::POST,
            "/user".to_string(),
            Some(json!({ "username": "x", "password": "longpassword" })),
        ),
        (
            Method::PUT,
            "/global_settings/list_instances_publicly".to_string(),
            Some(json!(true)),
        ),
        (Method::GET, "/user/api_keys".to_string(), None),
        (Method::DELETE, format!("/instance/{}", core.vanilla), None),
    ] {
        let (status, body) = core.call(method.clone(), &path, Some(&key), body).await;
        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED,
            "{method} {path}: {status} {body}"
        );
    }
    assert_eq!(core.state.instances.len(), 2);
    assert!(!core
        .state
        .global_settings
        .lock()
        .await
        .list_instances_publicly());

    // the effective user can start and stop, nothing else
    let record_user = core
        .state
        .users_manager
        .read()
        .await
        .try_auth(&key)
        .unwrap();
    assert!(!record_user.is_owner && !record_user.is_admin);
    assert!(record_user.can_perform_action(&UserAction::StartInstance(core.fabric.clone())));
    assert!(record_user.can_perform_action(&UserAction::StopInstance(core.fabric.clone())));
    assert!(
        record_user.can_perform_action(&UserAction::ViewInstance(InstanceUuid::from(
            "created-later".to_string()
        )))
    );
    for (what, action) in [
        ("settings", UserAction::AccessSetting(core.fabric.clone())),
        ("console", UserAction::AccessConsole(core.fabric.clone())),
        (
            "read files",
            UserAction::ReadInstanceFile(core.fabric.clone()),
        ),
        (
            "write files",
            UserAction::WriteInstanceFile(core.fabric.clone()),
        ),
        ("resources", UserAction::ReadResource(core.fabric.clone())),
        ("create", UserAction::CreateInstance),
        ("delete", UserAction::DeleteInstance),
        ("users", UserAction::ManageUser),
        ("permissions", UserAction::ManagePermission),
        ("global files", UserAction::ReadGlobalFile),
    ] {
        assert!(!record_user.can_perform_action(&action), "{what}");
    }
    match record_user.caused_by() {
        CausedBy::User { user_name, .. } => {
            assert_eq!(user_name, "owner (API key: Lodestone Launcher on Test Mac)")
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_members_launcher_key_follows_their_permissions() {
    let core = core().await;
    let member = add_user(
        &core.state,
        "member",
        false,
        false,
        UserPermission {
            can_view_instance: [core.fabric.clone()].into(),
            can_start_instance: [core.fabric.clone()].into(),
            can_access_instance_setting: [core.fabric.clone()].into(),
            ..Default::default()
        },
    )
    .await;
    let jwt = member.create_jwt().unwrap().to_string();
    let key = core.create_key(&jwt, json!({ "type": "launcher" })).await;
    let (status, list) = core
        .call(Method::GET, "/launcher/instances", Some(&key), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(Core::uuids(&list), vec![core.fabric.to_string()]);
    let user = core
        .state
        .users_manager
        .read()
        .await
        .try_auth(&key)
        .unwrap();
    assert!(user.can_perform_action(&UserAction::StartInstance(core.fabric.clone())));
    // never granted to the member, so not to the key
    assert!(!user.can_perform_action(&UserAction::StopInstance(core.fabric.clone())));
    // the member has it, but the launcher scope does not
    assert!(!user.can_perform_action(&UserAction::AccessSetting(core.fabric.clone())));
    assert!(!user.can_perform_action(&UserAction::ViewInstance(core.vanilla.clone())));
    let (status, _) = core
        .call(
            Method::GET,
            &format!("/launcher/instances/{}/client-pack", core.vanilla),
            Some(&key),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // losing a permission takes it from the key at once
    let mut perms = member.permissions.clone();
    perms.can_view_instance.clear();
    core.state
        .users_manager
        .write()
        .await
        .update_permissions(&member.uid, perms, CausedBy::System)
        .await
        .unwrap();
    let (_, list) = core
        .call(Method::GET, "/launcher/instances", Some(&key), None)
        .await;
    assert_eq!(list["instances"], json!([]));

    // and revoking the key signs the launcher out
    let id = key[4..16].to_string();
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/user/api_keys/{id}"),
            Some(&jwt),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(Method::GET, "/launcher/instances", Some(&key), None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_launcher_key_lists_in_the_dashboard() {
    let core = core().await;
    core.create_key(&core.owner_jwt, json!({ "type": "launcher" }))
        .await;
    let (status, keys) = core
        .call(Method::GET, "/user/api_keys", Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys[0]["scope"], json!({ "type": "launcher" }));
    assert_eq!(keys[0]["name"], "Lodestone Launcher on Test Mac");
}

#[tokio::test]
async fn deleting_an_instance_unlists_it() {
    let core = core().await;
    core.set_switch(true).await;
    core.set_listed(&core.vanilla, true).await;
    assert!(core.state.public_listing.is_listed(&core.vanilla));
    let (status, body) = core
        .call(
            Method::DELETE,
            &format!("/instance/{}", core.vanilla),
            Some(&core.owner_jwt),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!core.state.public_listing.is_listed(&core.vanilla));
}

#[test]
fn etags_match_with_or_without_weak_prefix() {
    let mut headers = HeaderMap::new();
    headers.insert(IF_NONE_MATCH, "W/\"abc\", \"def\"".parse().unwrap());
    assert!(matches_etag(&headers, "\"abc\""));
    assert!(matches_etag(&headers, "\"def\""));
    assert!(!matches_etag(&headers, "\"xyz\""));
    assert!(!matches_etag(&HeaderMap::new(), "\"abc\""));
}

#[test]
fn listing_rule() {
    let owner = User::new("o".into(), "pw", true, false, Default::default());
    let admin = User::new("a".into(), "pw", false, true, Default::default());
    let list = PublicScopeChange {
        listed: Some(true),
        ..Default::default()
    };
    let unlist = PublicScopeChange {
        listed: Some(false),
        ..Default::default()
    };
    let scope_only = PublicScopeChange {
        status: Some(false),
        ..Default::default()
    };
    let unlisted = PublicScope::default();
    let listed = PublicScope {
        listed: true,
        ..Default::default()
    };
    assert!(check_change(&owner, unlisted, &list, true).is_ok());
    assert!(check_change(&owner, unlisted, &list, false).is_err());
    // already listed: no switch needed to keep it so
    assert!(check_change(&owner, listed, &list, false).is_ok());
    assert!(check_change(&owner, listed, &unlist, false).is_ok());
    assert!(check_change(&owner, unlisted, &scope_only, false).is_ok());
    for change in [list, unlist, scope_only] {
        let err = check_change(&admin, listed, &change, true).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
    }
}

#[test]
fn event_details_name_what_is_shown() {
    assert_eq!(
        describe(PublicScope {
            listed: true,
            ..Default::default()
        }),
        "listed publicly (status, address)"
    );
    assert_eq!(
        describe(PublicScope {
            listed: true,
            status: false,
            address: false,
            client_pack: false
        }),
        "listed publicly (name and version only)"
    );
    assert_eq!(
        describe(PublicScope::default()),
        "no longer listed publicly"
    );
}
