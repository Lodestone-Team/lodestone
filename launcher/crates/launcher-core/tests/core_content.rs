//! A server's world, "Host this modpack" and the server resource pack, against a mock
//! core on a loopback port. Each test starts its own mock.

use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    content::InstanceDirs,
    cores::{
        CoreClient, CoreError,
        hosting::{self, HostSettings},
        resource_pack::{self, ServerResourcePack},
        worlds::{self as server_worlds, WorldDownload},
    },
    export,
    game::{
        download::{CancelToken, Progress, sha1_hex},
        loaders::{LoaderChoice, LoaderKind},
    },
    http,
    instances::InstanceStore,
    worlds,
};
use lodestone_minecraft::level;
use serde_json::{Value, json};

const LEVEL_DAT: &[u8] =
    include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");
const PASSWORD: &str = "hunter2-PASSWORD";
const SESSION: &str = "session-jwt-SECRET";
const KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";
const DOWNLOAD_KEY: &str = "AbCdEf0123456789AbCdEf0123456789";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    authorization: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

#[derive(Default)]
struct State {
    requests: Vec<Recorded>,
    running: bool,
    world_zip: Vec<u8>,
    /// Files served at `/files/<name>`.
    files: HashMap<String, Vec<u8>>,
    pack: Option<Value>,
    /// Ports in use and allocated.
    in_use: Vec<u16>,
    allocated: Vec<u16>,
}

struct MockCore {
    base: url::Url,
    state: Arc<Mutex<State>>,
    server: Arc<tiny_http::Server>,
}

impl Drop for MockCore {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<u8>) {
    let json_reply = |status: u16, value: Value| (status, value.to_string().into_bytes());
    let session = request.authorization.as_deref() == Some(&format!("Bearer {SESSION}"));
    let key = request.authorization.as_deref() == Some(&format!("Bearer {KEY}"));
    let path = request.path.as_str();
    if let Some(name) = path.strip_prefix("/files/") {
        return match state.files.get(name) {
            Some(data) => (200, data.clone()),
            None => (404, vec![]),
        };
    }
    if let Some(port) = path.strip_prefix("/api/v1/check/port/") {
        if !session {
            return json_reply(401, json!({ "causes": ["Unauthorized"] }));
        }
        let port: u16 = port.parse().unwrap();
        return json_reply(
            200,
            json!({ "is_in_use": state.in_use.contains(&port), "is_allocated": state.allocated.contains(&port) }),
        );
    }
    match (request.method.as_str(), path) {
        ("POST", "/api/v1/user/login") => {
            let expected = format!(
                "Basic {}",
                base64_encode(format!("steve:{PASSWORD}").as_bytes())
            );
            if request.authorization.as_deref() == Some(expected.as_str()) {
                json_reply(200, json!({ "token": SESSION, "user": {} }))
            } else {
                json_reply(401, json!({ "causes": ["Credential mismatch"] }))
            }
        }
        ("GET", "/api/v1/instance/inst-1/world/download") if session => {
            if state.running {
                return json_reply(
                    409,
                    json!({ "kind": "Conflict", "causes": ["Stop the server before downloading its world"] }),
                );
            }
            json_reply(
                200,
                json!({ "key": DOWNLOAD_KEY, "file_name": "SMP-world.zip", "folder": "world",
                        "dimension_folders": ["world_nether", "world_the_end"], "size": state.world_zip.len() }),
            )
        }
        // the device key can't read files
        ("GET", "/api/v1/instance/inst-1/world/download") if key => json_reply(
            403,
            json!({ "kind": "PermissionDenied", "causes": ["You don't have permission to read this server's files"] }),
        ),
        ("GET", p) if p == format!("/api/v1/file/{DOWNLOAD_KEY}") => (200, state.world_zip.clone()),
        ("GET", "/api/v1/instance/inst-1/resource_pack") if key => match &state.pack {
            Some(pack) => json_reply(200, pack.clone()),
            None => json_reply(
                200,
                json!({"url": "", "sha1": "", "required": false, "prompt": ""}),
            ),
        },
        ("POST", "/api/v1/modpack/upload") if session => json_reply(
            200,
            json!({ "name": "My Pack", "version_number": null, "summary": null, "game_version": "1.20.1",
                    "loader": "fabric", "loader_version": "0.16.14", "server_file_count": 1,
                    "server_download_size": 10, "unsupported_reason": null, "upload_id": "upload-1" }),
        ),
        ("POST", "/api/v1/instance/create_modpack") if session => {
            json_reply(200, json!("new-server-uuid"))
        }
        _ if request.authorization.is_some() => {
            json_reply(401, json!({ "causes": ["Unauthorized"] }))
        }
        _ => json_reply(404, json!({ "causes": ["Not found"] })),
    }
}

impl MockCore {
    fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        {
            let server = server.clone();
            let state = state.clone();
            std::thread::spawn(move || {
                while let Ok(mut request) = server.recv() {
                    let mut body = Vec::new();
                    let _ = request.as_reader().read_to_end(&mut body);
                    let header = |name: &'static str| {
                        request
                            .headers()
                            .iter()
                            .find(|h| h.field.equiv(name))
                            .map(|h| h.value.to_string())
                    };
                    let recorded = Recorded {
                        method: request.method().to_string(),
                        path: request.url().to_string(),
                        authorization: header("Authorization"),
                        content_type: header("Content-Type"),
                        body,
                    };
                    let (status, bytes) = {
                        let mut state = state.lock().unwrap();
                        state.requests.push(recorded.clone());
                        route(&mut state, &recorded)
                    };
                    let response = tiny_http::Response::from_data(bytes)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_str("Content-Type: application/json").unwrap(),
                        );
                    let _ = request.respond(response);
                }
            });
        }
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}")).unwrap(),
            state,
            server,
        }
    }

    fn client(&self) -> CoreClient {
        CoreClient::new(http::default_client().unwrap(), self.base.clone())
    }

    fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::FileOptions::default();
    for (name, content) in entries {
        if name.ends_with('/') {
            writer.add_directory(*name, options).unwrap();
        } else {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

/// A Bukkit-style server world, as the core zips it.
fn server_world_zip() -> Vec<u8> {
    zip(&[
        ("world/", b""),
        ("world/level.dat", LEVEL_DAT),
        ("world/region/r.0.0.mca", b"overworld"),
        ("world/playerdata/p.dat", b"player"),
        ("world/session.lock", b"lock"),
        ("world_nether/", b""),
        ("world_nether/level.dat", LEVEL_DAT),
        ("world_nether/uid.dat", b"uid"),
        ("world_nether/DIM-1/region/r.0.0.mca", b"nether"),
        ("world_the_end/level.dat", LEVEL_DAT),
        ("world_the_end/DIM1/region/r.0.0.mca", b"end"),
    ])
}

fn tree(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn instance_dirs() -> (tempfile::TempDir, InstanceDirs) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = InstanceDirs::new(temp.path().join("instance"));
    fs::create_dir_all(&dirs.game).unwrap();
    (temp, dirs)
}

#[test]
fn a_servers_world_becomes_a_singleplayer_world() {
    let mock = MockCore::start();
    mock.state.lock().unwrap().world_zip = server_world_zip();
    let client = mock.client();
    let (temp, dirs) = instance_dirs();

    let (download, zip_path) = block_on(async {
        // the device key can't download worlds; a session can
        let refused = client
            .world_download(&Secret::new(KEY), "inst-1")
            .await
            .unwrap_err();
        assert!(matches!(refused, CoreError::Forbidden(_)), "{refused:?}");
        let session = client
            .session("steve", &Secret::new(PASSWORD))
            .await
            .unwrap();
        let download = client.world_download(&session, "inst-1").await.unwrap();
        let out = temp.path().join("world.zip");
        let size = client
            .download_file(
                &download.key,
                &out,
                1 << 30,
                &Progress::default(),
                &CancelToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(size, download.size);
        (download, out)
    });
    assert_eq!(download.folder, "world");

    // a world called "SMP" exists already: the import gets its own folder
    fs::create_dir_all(worlds::saves_dir(&dirs).join("SMP")).unwrap();
    let folder = server_worlds::import(
        &dirs,
        &zip_path,
        &download,
        "SMP",
        &Progress::default(),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(folder, "SMP (2)");
    let dir = worlds::saves_dir(&dirs).join(&folder);
    assert_eq!(
        tree(&dir),
        [
            "DIM-1/region/r.0.0.mca",
            "DIM1/region/r.0.0.mca",
            "level.dat",
            "playerdata/p.dat",
            "region/r.0.0.mca",
            "session.lock",
        ]
    );
    assert_eq!(
        fs::read(dir.join("DIM-1/region/r.0.0.mca")).unwrap(),
        b"nether"
    );
    // it shows the server's name in singleplayer
    let summary = level::summarize(&fs::read(dir.join("level.dat")).unwrap()).unwrap();
    assert_eq!(summary.name.as_deref(), Some("SMP"));
    // nothing is left behind in saves/
    let names: Vec<String> = fs::read_dir(worlds::saves_dir(&dirs))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");

    // the download itself never sent a token
    let file_request = mock
        .requests()
        .into_iter()
        .find(|r| r.path.starts_with("/api/v1/file/"))
        .unwrap();
    assert_eq!(file_request.authorization, None);
}

#[test]
fn world_downloads_are_refused_while_running() {
    let mock = MockCore::start();
    mock.state.lock().unwrap().running = true;
    let client = mock.client();
    let err = block_on(async {
        let session = client
            .session("steve", &Secret::new(PASSWORD))
            .await
            .unwrap();
        client.world_download(&session, "inst-1").await.unwrap_err()
    });
    match err {
        CoreError::Server { status, message } => {
            assert_eq!(status, 409);
            assert!(message.contains("Stop the server"), "{message}");
        }
        err => panic!("{err:?}"),
    }
    // a wrong password is not a revoked key
    let err = block_on(client.session("steve", &Secret::new("nope"))).unwrap_err();
    assert_eq!(err, CoreError::BadCredentials);
}

#[test]
fn unsafe_world_zips_are_refused() {
    let (temp, dirs) = instance_dirs();
    let download = |folder: &str| WorldDownload {
        key: "k".into(),
        file_name: "w.zip".into(),
        folder: folder.into(),
        dimension_folders: vec![format!("{folder}_nether")],
        size: 0,
    };
    let import = |bytes: Vec<u8>, folder: &str| {
        let path = temp.path().join("w.zip");
        fs::write(&path, bytes).unwrap();
        server_worlds::import(
            &dirs,
            &path,
            &download(folder),
            "World",
            &Progress::default(),
            &CancelToken::new(),
        )
    };
    // the core names a folder that isn't plain
    for folder in ["../world", "", ".hidden", "a/b"] {
        let err = import(server_world_zip(), folder).unwrap_err();
        assert!(err.to_string().contains("plain name"), "{folder}: {err}");
    }
    // no level.dat in the named folder
    let err = import(zip(&[("other/level.dat", LEVEL_DAT)]), "world").unwrap_err();
    assert!(err.to_string().contains("no world/level.dat"), "{err}");
    // a path that climbs out of the world
    let err = import(
        zip(&[
            ("world/level.dat", LEVEL_DAT),
            ("world/../../escape.txt", b"x"),
        ]),
        "world",
    )
    .unwrap_err();
    assert!(err.to_string().contains("unsafe"), "{err}");
    assert!(!temp.path().join("escape.txt").exists());
    // not a zip
    assert!(import(b"not a zip".to_vec(), "world").is_err());
    // nothing was imported, and no staging folder is left
    let saves = worlds::saves_dir(&dirs);
    assert!(
        !saves.exists() || fs::read_dir(&saves).unwrap().next().is_none(),
        "{:?}",
        tree(&saves)
    );

    // a world that has its nether inside keeps it, and the sibling is left out
    let folder = import(
        zip(&[
            ("world/level.dat", LEVEL_DAT),
            ("world/DIM-1/region/r.0.0.mca", b"inside"),
            ("world_nether/DIM-1/region/r.0.0.mca", b"sibling"),
        ]),
        "world",
    )
    .unwrap();
    assert_eq!(
        fs::read(saves.join(folder).join("DIM-1/region/r.0.0.mca")).unwrap(),
        b"inside"
    );
}

#[test]
fn hosting_finds_a_port_uploads_and_creates_a_stopped_server() {
    let mock = MockCore::start();
    {
        let mut state = mock.state.lock().unwrap();
        state.in_use = vec![25565];
        state.allocated = vec![25566];
    }
    let client = mock.client();
    let pack = b"PK fake mrpack bytes".to_vec();
    let uuid = block_on(async {
        let session = client
            .session("steve", &Secret::new(PASSWORD))
            .await
            .unwrap();
        let port = client
            .free_port(&session, hosting::FIRST_PORT)
            .await
            .unwrap();
        assert_eq!(port, 25567);
        let uploaded = client
            .upload_pack(&session, "My \"Pack\".mrpack", &pack)
            .await
            .unwrap();
        assert_eq!(uploaded.upload_id.as_deref(), Some("upload-1"));
        assert_eq!(uploaded.unsupported_reason, None);
        client
            .create_server(
                &session,
                "upload-1",
                &HostSettings {
                    name: "My Pack".into(),
                    port,
                    min_ram: 1024,
                    max_ram: 4096,
                    accept_eula: true,
                },
            )
            .await
            .unwrap()
    });
    assert_eq!(uuid, "new-server-uuid");

    let requests = mock.requests();
    let upload = requests
        .iter()
        .find(|r| r.path == "/api/v1/modpack/upload")
        .unwrap();
    let content_type = upload.content_type.clone().unwrap();
    let boundary = content_type
        .strip_prefix("multipart/form-data; boundary=")
        .unwrap();
    let body = String::from_utf8_lossy(&upload.body);
    assert!(body.starts_with(&format!("--{boundary}\r\n")), "{body}");
    // the quote in the name can't end the header
    assert!(
        body.contains("name=\"file\"; filename=\"My _Pack_.mrpack\""),
        "{body}"
    );
    assert!(body.contains("PK fake mrpack bytes"));
    assert!(body.ends_with(&format!("\r\n--{boundary}--\r\n")));

    let create = requests
        .iter()
        .find(|r| r.path == "/api/v1/instance/create_modpack")
        .unwrap();
    let create: Value = serde_json::from_slice(&create.body).unwrap();
    assert_eq!(
        create,
        json!({
            "source": {"type": "upload", "upload_id": "upload-1"},
            "name": "My Pack", "port": 25567, "min_ram": 1024, "max_ram": 4096,
            "auto_start": false, "restart_on_crash": false,
            // the user ticked "I agree to the Minecraft EULA"
            "accept_eula": true,
        })
    );
    // the password went only to the login, and the session nowhere but this core
    for request in &requests {
        if request.path != "/api/v1/user/login" {
            assert_eq!(
                request.authorization.as_deref(),
                Some(format!("Bearer {SESSION}").as_str())
            );
        }
        assert!(!String::from_utf8_lossy(&request.body).contains(PASSWORD));
    }
    // nothing asked the core to start anything
    assert!(requests.iter().all(|r| !r.path.ends_with("/start")));
}

#[test]
fn the_hosted_instance_is_linked_with_its_server_mods() {
    let temp = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(temp.path().join("instances"));
    let instance = store
        .create(
            "My Pack",
            "1.20.1",
            Some(&LoaderChoice {
                kind: LoaderKind::Fabric,
                version: "0.16.14".into(),
            }),
        )
        .unwrap();
    assert!(hosting::check_hostable(&instance).is_ok());
    let vanilla = store.create("Vanilla", "1.20.1", None).unwrap();
    assert!(hosting::check_hostable(&vanilla).is_err());

    let game = store.game_dir(&instance.id);
    fs::create_dir_all(game.join("mods")).unwrap();
    fs::write(game.join("mods/uploaded.jar"), b"jar bytes").unwrap();
    fs::create_dir_all(game.join("config")).unwrap();
    fs::write(game.join("config/a.toml"), b"a = 1").unwrap();
    let plan = export::plan(&instance, &game, &Default::default()).unwrap();
    let out = temp.path().join("pack.mrpack");
    export::write(&plan, &out).unwrap();
    let bytes = hosting::read_pack_file(&out).unwrap();
    assert!(bytes.starts_with(b"PK"));

    let link = hosting::host_link("core-1", "http://127.0.0.1:16662", "new-server", "My Pack");
    assert_eq!(link.server_uuid, "new-server");
    assert_eq!(link.pack_etag, None);
    let dir = store.dir(&instance.id);
    launcher_core::cores::link::save_link(&dir, &link).unwrap();
    assert_eq!(launcher_core::cores::link::load_link(&dir), Some(link));
}

fn resource_pack_zip(marker: &[u8]) -> Vec<u8> {
    zip(&[
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":15,"description":"x"}}"#,
        ),
        ("assets/minecraft/x.txt", marker),
    ])
}

#[test]
fn the_server_resource_pack_downloads_into_the_instance() {
    let mock = MockCore::start();
    let pack_bytes = resource_pack_zip(b"one");
    let sha1 = sha1_hex(&pack_bytes);
    {
        let mut state = mock.state.lock().unwrap();
        state.files.insert("pack.zip".into(), pack_bytes.clone());
        state
            .files
            .insert("other.zip".into(), resource_pack_zip(b"two"));
        state
            .files
            .insert("notapack.zip".into(), zip(&[("x.txt", b"x")]));
        state.pack = Some(json!({
            "url": format!("{}files/pack.zip", mock.base), "sha1": sha1, "required": true, "prompt": "Please"
        }));
    }
    let client = mock.client();
    let pack: ServerResourcePack = block_on(client.resource_pack(&Secret::new(KEY), "inst-1"))
        .unwrap()
        .unwrap();
    assert!(pack.required);
    assert_eq!(pack.label(), "127.0.0.1 · pack.zip");

    let temp = tempfile::tempdir().unwrap();
    let packs = temp.path().join("resourcepacks");
    let http = http::default_client().unwrap();
    let saved = block_on(resource_pack::download(
        http.clone(),
        &pack,
        "My/SMP",
        &packs,
    ))
    .unwrap();
    assert_eq!(saved.file_name().unwrap(), "My_SMP (server).zip");
    assert_eq!(fs::read(&saved).unwrap(), pack_bytes);
    // again: the file is kept, nothing is downloaded
    let before = mock.requests().len();
    block_on(resource_pack::download(
        http.clone(),
        &pack,
        "My/SMP",
        &packs,
    ))
    .unwrap();
    assert_eq!(mock.requests().len(), before);

    // a pack whose hash doesn't match isn't saved
    let mut wrong = pack.clone();
    wrong.url = format!("{}files/other.zip", mock.base);
    let err = block_on(resource_pack::download(
        http.clone(),
        &wrong,
        "Other",
        &packs,
    ))
    .unwrap_err();
    assert!(err.contains("doesn't match"), "{err}");
    // a zip that isn't a resource pack isn't saved
    let not_pack = ServerResourcePack {
        url: format!("{}files/notapack.zip", mock.base),
        ..Default::default()
    };
    let err = block_on(resource_pack::download(
        http.clone(),
        &not_pack,
        "Other",
        &packs,
    ))
    .unwrap_err();
    assert!(err.contains("pack.mcmeta"), "{err}");
    for url in [
        "file:///etc/passwd",
        "ftp://example.com/p.zip",
        "http://u:p@example.com/p.zip",
    ] {
        let bad = ServerResourcePack {
            url: url.into(),
            ..Default::default()
        };
        assert!(
            block_on(resource_pack::download(http.clone(), &bad, "X", &packs)).is_err(),
            "{url}"
        );
    }
    // only the saved pack is left
    let names: Vec<String> = fs::read_dir(&packs)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["My_SMP (server).zip"]);

    // a server without a pack
    mock.state.lock().unwrap().pack = None;
    assert_eq!(
        block_on(client.resource_pack(&Secret::new(KEY), "inst-1")).unwrap(),
        None
    );
}
