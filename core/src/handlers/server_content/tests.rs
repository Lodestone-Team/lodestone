//! World downloads, the resource pack lines and modpack uploads, against a core on a
//! loopback port with real (never started) Minecraft instances restored from files in a
//! temporary folder.

use std::io::{Read as _, Write as _};

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::{CodeExecutionSwitch, GlobalSettingsData},
    test_util::{serve_api, test_app_state},
    types::DotLodestoneConfig,
};

fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

struct Core {
    _dir: tempfile::TempDir,
    state: AppState,
    root: PathBuf,
    base: String,
    client: reqwest::Client,
    owner_jwt: String,
    uuid: InstanceUuid,
}

const PROPERTIES: &str = "#Minecraft server properties\n# kept by hand\nmotd=A \\u00A7aGreen server\nserver-port=25570\nresource-pack=\nlevel-name=world\n";

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
    let root = instances.join("paper");
    std::fs::create_dir_all(&root).unwrap();
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
            "name": "Paper SMP", "version": "1.20.1", "flavour": {"paper": {"build_version": 196}},
            "description": "", "cmd_args": [], "java_cmd": "java",
            "port": 25570, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 17,
            "has_started": true
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(root.join("server.properties"), PROPERTIES).unwrap();
    // a Bukkit-style world: the overworld, and the other dimensions beside it
    for (path, content) in [
        ("world/level.dat", "overworld level"),
        ("world/region/r.0.0.mca", "overworld region"),
        ("world/session.lock", "lock"),
        ("world_nether/level.dat", "nether level"),
        ("world_nether/DIM-1/region/r.0.0.mca", "nether region"),
        ("world_the_end/DIM1/region/r.0.0.mca", "end region"),
        ("world_other/level.dat", "not this world"),
    ] {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let executor = CodeExecutionSwitch::new(false);
    let restored = crate::restore_instances(&instances, state.event_broadcaster.clone(), executor)
        .await
        .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    assert_eq!(state.instances.len(), 1);
    let owner = add_user(&state, "owner", true, Default::default()).await;
    let owner_jwt = owner.create_jwt().unwrap().to_string();
    let base = serve_api(state.clone()).await;
    Core {
        _dir: dir,
        state,
        root,
        base,
        client: reqwest::Client::new(),
        owner_jwt,
        uuid,
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

    async fn member(&self, name: &str, permissions: UserPermission) -> String {
        add_user(&self.state, name, false, permissions)
            .await
            .create_jwt()
            .unwrap()
            .to_string()
    }

    async fn launcher_key(&self) -> String {
        let (status, body) = self
            .call(
                Method::POST,
                "/user/api_keys",
                Some(&self.owner_jwt),
                Some(json!({ "name": "Lodestone Launcher on Test Mac", "expires_at": null, "scope": { "type": "launcher" } })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["key"].as_str().unwrap().to_string()
    }

    async fn set_state(&self, state: InstanceState) {
        let instance = get_instance(&self.state, &self.uuid).unwrap();
        minecraft(&instance)
            .unwrap()
            .set_state_for_test(state)
            .await;
    }

    fn world_path(&self) -> String {
        format!("/instance/{}/world/download", self.uuid)
    }

    fn properties(&self) -> String {
        std::fs::read_to_string(self.root.join("server.properties")).unwrap()
    }
}

/// The names and contents of the files in a zip.
fn zip_entries(bytes: &[u8]) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        if file.is_dir() {
            continue;
        }
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        entries.push((file.name().unwrap().to_string(), content));
    }
    entries.sort();
    entries
}

#[test]
fn world_folder_names_must_be_plain() {
    assert_eq!(world_folder("world").unwrap(), PathBuf::from("world"));
    assert_eq!(
        world_folder("worlds/survival").unwrap(),
        PathBuf::from("worlds").join("survival")
    );
    assert_eq!(
        world_folder("My World 2").unwrap(),
        PathBuf::from("My World 2")
    );
    for bad in [
        "",
        "..",
        "../world",
        "world/../../etc",
        "/etc",
        "/",
        "./world",
        ".lodestone",
        "world/",
        "a//b",
        "C:\\world",
        "C:world",
        "..\\world",
        "world\0",
        " world",
        // Windows short names of a restore's folders
        "LODEST~1",
        "lodest~2",
        "worlds/LO1F2A~1",
    ] {
        assert!(world_folder(bad).is_err(), "{bad:?}");
    }
    assert!(world_folder("world~1").is_ok());
}

#[tokio::test]
async fn downloads_the_world_with_its_dimensions() {
    let core = core().await;
    let (status, body) = core
        .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let download: WorldDownload = serde_json::from_value(body).unwrap();
    assert_eq!(download.folder, "world");
    assert_eq!(
        download.dimension_folders,
        vec!["world_nether".to_string(), "world_the_end".to_string()]
    );
    assert_eq!(download.file_name, "Paper SMP-world.zip");
    let response = core
        .client
        .get(format!("{}/file/{}", core.base, download.key))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes.len() as u64, download.size);
    let names: Vec<String> = zip_entries(&bytes).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        [
            "world/level.dat",
            "world/region/r.0.0.mca",
            "world/session.lock",
            "world_nether/DIM-1/region/r.0.0.mca",
            "world_nether/level.dat",
            "world_the_end/DIM1/region/r.0.0.mca",
        ]
    );
}

#[tokio::test]
async fn world_downloads_need_file_read_access() {
    let core = core().await;
    let mut view_only = UserPermission::new();
    view_only.can_view_instance.insert(core.uuid.clone());
    let viewer = core.member("viewer", view_only.clone()).await;
    let (status, _) = core
        .call(Method::GET, &core.world_path(), Some(&viewer), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let mut reader = view_only;
    reader.can_read_instance_file.insert(core.uuid.clone());
    let reader = core.member("reader", reader).await;
    let (status, body) = core
        .call(Method::GET, &core.world_path(), Some(&reader), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // a launcher key views, starts and stops only, even the owner's
    let key = core.launcher_key().await;
    let (status, _) = core
        .call(Method::GET, &core.world_path(), Some(&key), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core.call(Method::GET, &core.world_path(), None, None).await;
    assert!(
        status.is_client_error() && status != StatusCode::NOT_FOUND,
        "{status}"
    );
}

#[tokio::test]
async fn world_downloads_are_refused_while_the_server_runs() {
    let core = core().await;
    for state in [
        InstanceState::Running,
        InstanceState::Starting,
        InstanceState::Stopping,
    ] {
        core.set_state(state).await;
        let (status, body) = core
            .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{state:?}");
        assert!(body.to_string().contains("Stop the server"), "{body}");
    }
    core.set_state(InstanceState::Stopped).await;
    let (status, _) = core
        .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn unsafe_or_missing_worlds_are_refused() {
    let core = core().await;
    let set_level = |name: &str| {
        std::fs::write(
            core.root.join("server.properties"),
            format!("level-name={name}\n"),
        )
        .unwrap()
    };
    for bad in ["../paper", "/etc", ".lodestone", "world/../../x"] {
        set_level(bad);
        let (status, body) = core
            .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}: {body}");
    }
    set_level("missing");
    let (status, _) = core
        .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // a world folder that is a link out of the instance
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("level.dat"), "secret").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), core.root.join("linked")).unwrap();
        set_level("linked");
        let (status, body) = core
            .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }

    // another level name is used as it is, without the Bukkit folders of `world`
    set_level("world_other");
    let (status, body) = core
        .call(Method::GET, &core.world_path(), Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["folder"], "world_other");
    assert_eq!(body["dimension_folders"], json!([]));
}

#[tokio::test]
async fn resource_pack_lines_are_written_and_the_rest_kept() {
    let core = core().await;
    let path = format!("/instance/{}/resource_pack", core.uuid);
    let (status, body) = core
        .call(Method::GET, &path, Some(&core.owner_jwt), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"url": "", "sha1": "", "required": false, "prompt": ""})
    );

    let sha1 = "A".repeat(40);
    let (status, body) = core
        .call(
            Method::PUT,
            &path,
            Some(&core.owner_jwt),
            Some(json!({"url": " https://example.com/pack.zip?v=2 ", "sha1": sha1, "required": true, "prompt": "Please accept: it's \"tiny\""})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"url": "https://example.com/pack.zip?v=2", "sha1": "a".repeat(40), "required": true, "prompt": "Please accept: it's \"tiny\""})
    );
    let text = core.properties();
    // the other lines, comments and escapes are as they were
    assert!(text.starts_with("#Minecraft server properties\n# kept by hand\nmotd=A \\u00A7aGreen server\nserver-port=25570\n"), "{text}");
    assert!(text.contains("level-name=world\n"), "{text}");
    assert!(
        text.contains("resource-pack=https\\://example.com/pack.zip?v\\=2\n"),
        "{text}"
    );
    assert!(text.contains(&format!("resource-pack-sha1={}", "a".repeat(40))));
    assert!(text.contains("require-resource-pack=true"));
    // the prompt is a JSON text component
    let (doc, _) = lodestone_minecraft::properties::PropertiesDocument::parse(&text);
    assert_eq!(
        doc.get("resource-pack-prompt").unwrap(),
        r#""Please accept: it's \"tiny\"""#
    );
    assert_eq!(
        text.lines()
            .filter(|l| l.starts_with("resource-pack="))
            .count(),
        1
    );

    // a JSON component is kept as it is
    let component = r#"{"text":"Hi","color":"gold"}"#;
    let (status, body) = core
        .call(
            Method::PUT,
            &path,
            Some(&core.owner_jwt),
            Some(json!({"url": "http://example.com/p.zip", "sha1": "", "required": false, "prompt": component})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["prompt"], component);

    // an empty URL clears every line
    let (status, body) = core
        .call(
            Method::PUT,
            &path,
            Some(&core.owner_jwt),
            Some(json!({"url": "", "sha1": "a".repeat(40), "required": true, "prompt": "x"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"url": "", "sha1": "", "required": false, "prompt": ""})
    );
    assert!(core.properties().contains("motd=A \\u00A7aGreen server\n"));

    for bad in [
        json!({"url": "ftp://example.com/p.zip", "sha1": "", "required": false, "prompt": ""}),
        json!({"url": "https://example.com/a b.zip", "sha1": "", "required": false, "prompt": ""}),
        json!({"url": "https://example.com/p.zip", "sha1": "xyz", "required": false, "prompt": ""}),
        json!({"url": "https://example.com/p.zip", "sha1": "g".repeat(40), "required": false, "prompt": ""}),
    ] {
        let (status, _) = core
            .call(Method::PUT, &path, Some(&core.owner_jwt), Some(bad.clone()))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
}

#[tokio::test]
async fn resource_pack_permissions() {
    let core = core().await;
    let path = format!("/instance/{}/resource_pack", core.uuid);
    let body =
        json!({"url": "https://example.com/p.zip", "sha1": "", "required": false, "prompt": ""});
    let mut view_only = UserPermission::new();
    view_only.can_view_instance.insert(core.uuid.clone());
    let viewer = core.member("viewer", view_only.clone()).await;
    let (status, _) = core.call(Method::GET, &path, Some(&viewer), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(Method::PUT, &path, Some(&viewer), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core
        .call(
            Method::POST,
            &format!("{path}/hash"),
            Some(&viewer),
            Some(json!({"url": "https://example.com/p.zip"})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // a launcher key may read the pack, to show it in the launcher
    let key = core.launcher_key().await;
    let (status, _) = core.call(Method::GET, &path, Some(&key), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core
        .call(Method::PUT, &path, Some(&key), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let stranger = core.member("stranger", UserPermission::new()).await;
    let (status, _) = core.call(Method::GET, &path, Some(&stranger), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let mut settings = view_only;
    settings
        .can_access_instance_setting
        .insert(core.uuid.clone());
    let editor = core.member("editor", settings).await;
    let (status, _) = core
        .call(Method::PUT, &path, Some(&editor), Some(body))
        .await;
    assert_eq!(status, StatusCode::OK);

    // a pack on this machine: the owner may hash it, other users may not
    let pack_server = {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let url = format!("http://{}/pack.zip", listener.local_addr().unwrap());
        let app = axum::Router::new().route(
            "/pack.zip",
            axum::routing::get(|| async { b"resource pack".to_vec() }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        url
    };
    let (status, body) = core
        .call(
            Method::POST,
            &format!("{path}/hash"),
            Some(&editor),
            Some(json!({ "url": pack_server })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = core
        .call(
            Method::POST,
            &format!("{path}/hash"),
            Some(&core.owner_jwt),
            Some(json!({ "url": pack_server })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["sha1"],
        crate::modrinth::fake::sha1_hex(b"resource pack")
    );
    assert_eq!(body["size"], 13);
}

/// A `.mrpack` as Lodestone Launcher exports one: a Modrinth mod for both sides, a
/// client-only one (`env.server: unsupported`), an uploaded jar and a config override.
fn launcher_pack(dependencies: Value) -> Vec<u8> {
    let entry = |name: &str, server: &str| {
        json!({
            "path": format!("mods/{name}"),
            "hashes": {"sha1": crate::modrinth::fake::sha1_hex(name.as_bytes()), "sha512": crate::modrinth::fake::sha512_hex(name.as_bytes())},
            "env": {"client": "required", "server": server},
            "downloads": [format!("https://cdn.modrinth.com/data/AAAAAAAA/versions/BBBBBBBB/{name}")],
            "fileSize": name.len(),
        })
    };
    let index = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0.0", "name": "My Instance",
        "files": [entry("lithium.jar", "required"), entry("sodium.jar", "unsupported")],
        "dependencies": dependencies,
    });
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut buffer);
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file("modrinth.index.json", options).unwrap();
    writer.write_all(index.to_string().as_bytes()).unwrap();
    writer
        .start_file("overrides/mods/uploaded.jar", options)
        .unwrap();
    writer.write_all(b"jar").unwrap();
    writer
        .start_file("overrides/config/thing.toml", options)
        .unwrap();
    writer.write_all(b"a = 1").unwrap();
    writer
        .start_file("client-overrides/options.txt", options)
        .unwrap();
    writer.write_all(b"fov:1").unwrap();
    writer.start_file("overrides/eula.txt", options).unwrap();
    writer.write_all(b"eula=true").unwrap();
    writer.finish().unwrap();
    buffer.into_inner()
}

impl Core {
    /// Uploads a pack as the dashboard and the launcher do: one multipart `file` field.
    async fn upload(&self, token: &str, file_name: &str, bytes: Vec<u8>) -> (StatusCode, Value) {
        let boundary = "lodestone-test-boundary";
        let mut body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/x-modrinth-modpack+zip\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(&bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let response = self
            .client
            .post(format!("{}/modpack/upload", self.base))
            .bearer_auth(token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
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

#[tokio::test]
async fn an_uploaded_launcher_pack_creates_a_server_instance() {
    let core = core().await;
    let fabric = json!({"minecraft": "1.20.1", "fabric-loader": "0.16.14"});
    let (status, info) = core
        .upload(
            &core.owner_jwt,
            "My Instance.mrpack",
            launcher_pack(fabric.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["loader"], "fabric");
    assert_eq!(info["loader_version"], "0.16.14");
    assert_eq!(info["game_version"], "1.20.1");
    assert_eq!(info["unsupported_reason"], Value::Null);
    // the client-only mod is not downloaded for the server
    assert_eq!(info["server_file_count"], 1);
    let upload_id = info["upload_id"].as_str().unwrap().to_string();

    // the pack's server files: the shared mod, the uploaded jar and the config, never
    // the client-only mod, the client overrides or the EULA
    let stored = core.state.mods.packs.get(&upload_id).unwrap();
    let pack = crate::modpack::read_pack(&stored.path(), false).unwrap();
    let downloads: Vec<String> = pack
        .downloads
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    assert_eq!(downloads, ["mods/lithium.jar"]);
    let skipped: Vec<String> = pack.skipped.iter().map(|s| s.path.clone()).collect();
    assert!(
        skipped.iter().any(|p| p.ends_with("eula.txt")),
        "{skipped:?}"
    );

    // creating it checks the settings before anything is set up
    let create = |source: Value, port: u32| json!({"source": source, "name": "My Instance", "port": port, "min_ram": 1024, "max_ram": 4096, "auto_start": false});
    let (status, _) = core
        .call(
            Method::POST,
            "/instance/create_modpack",
            Some(&core.owner_jwt),
            Some(create(json!({"type": "upload", "upload_id": upload_id}), 0)),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // only the uploader may use the pack
    let mut creator = UserPermission::new();
    creator.can_create_instance = true;
    creator.can_write_global_file = true;
    let other = core.member("other", creator.clone()).await;
    let (status, _) = core
        .call(
            Method::POST,
            "/instance/create_modpack",
            Some(&other),
            Some(create(
                json!({"type": "upload", "upload_id": upload_id}),
                25600,
            )),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // uploading needs the permission to add jars, and a launcher key can't
    let mut no_jars = creator;
    no_jars.can_write_global_file = false;
    let no_jars = core.member("nojars", no_jars).await;
    let (status, _) = core
        .upload(&no_jars, "p.mrpack", launcher_pack(fabric.clone()))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let key = core.launcher_key().await;
    let (status, _) = core.upload(&key, "p.mrpack", launcher_pack(fabric)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // a pack without Fabric can't be hosted, and says why
    let (status, info) = core
        .upload(
            &core.owner_jwt,
            "Vanilla.mrpack",
            launcher_pack(json!({"minecraft": "1.20.1"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert!(info["unsupported_reason"]
        .as_str()
        .unwrap()
        .contains("Only Fabric"));
    assert_eq!(info["upload_id"], Value::Null);
}
