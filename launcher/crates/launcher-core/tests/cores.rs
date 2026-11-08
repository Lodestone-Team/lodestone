//! The core client and the client-pack import against a mock core on a loopback port,
//! which also mirrors Modrinth's CDN. Each test starts its own mock.

use std::{
    collections::HashMap,
    io::{Cursor, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    content::Modrinth,
    cores::{
        CoreClient, CoreError, PackFetch, ServerState,
        pack::{self, LinkedFile, ServerLink},
        parse_address,
    },
    game::download::{CancelToken, Downloader, Progress, recording_sleep, sha1_hex},
    http,
};
use serde_json::{Value, json};
use sha2::Digest as _;

const PASSWORD: &str = "hunter2-PASSWORD";
const SESSION: &str = "session-jwt-SECRET";
const KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    authorization: Option<String>,
    if_none_match: Option<String>,
    body: String,
}

#[derive(Default)]
struct State {
    requests: Vec<Recorded>,
    /// Whether the key still works.
    key_revoked: bool,
    /// The client pack, and its ETag.
    pack: Option<(Vec<u8>, String)>,
    /// CDN files by path.
    cdn: HashMap<String, Vec<u8>>,
    running: bool,
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

fn instance(state: &str) -> Value {
    json!({
        "uuid": "inst-1", "name": "SMP", "game_version": "1.20.1", "loader": "fabric",
        "loader_version": "0.16.14", "port": 25570, "state": state, "player_count": 1,
        "max_player_count": 20, "has_client_pack": true, "playit_address": null, "listed": true
    })
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let json_reply = |status: u16, value: Value| (status, vec![], value.to_string().into_bytes());
    let bearer_ok =
        request.authorization.as_deref() == Some(&format!("Bearer {KEY}")) && !state.key_revoked;
    let path = request.path.as_str();
    if let Some(cdn) = path.strip_prefix("/cdn.modrinth.com/") {
        return match state.cdn.get(cdn) {
            Some(data) => (200, vec![], data.clone()),
            None => (404, vec![], vec![]),
        };
    }
    match (request.method.as_str(), path) {
        ("GET", "/api/v1/info") => json_reply(
            200,
            json!({ "version": "0.5.1", "uuid": "core-1", "core_name": "Home core", "is_setup": true, "tls": {"mode": "none"} }),
        ),
        ("GET", "/api/v1/public/instances") => json_reply(
            200,
            json!({ "core_name": "Home core", "core_uuid": "core-1", "listing_enabled": true, "instances": [instance("Stopped")] }),
        ),
        ("GET", "/api/v1/launcher/instances") if bearer_ok => {
            let s = if state.running { "Running" } else { "Starting" };
            json_reply(
                200,
                json!({ "core_name": "Home core", "core_uuid": "core-1", "listing_enabled": false, "instances": [instance(s)] }),
            )
        }
        ("POST", "/api/v1/user/login") => {
            let expected = format!(
                "Basic {}",
                base64_encode(format!("steve:{PASSWORD}").as_bytes())
            );
            if request.authorization.as_deref() == Some(expected.as_str()) {
                json_reply(200, json!({ "token": SESSION, "user": {} }))
            } else {
                json_reply(
                    401,
                    json!({ "kind": "Unauthorized", "causes": ["Credential mismatch"] }),
                )
            }
        }
        ("POST", "/api/v1/user/api_keys")
            if request.authorization.as_deref() == Some(&format!("Bearer {SESSION}")) =>
        {
            json_reply(
                200,
                json!({ "key": KEY, "info": { "id": "abcdefghijkl", "name": "x", "expires_at": null } }),
            )
        }
        ("PUT", "/api/v1/instance/inst-1/start") if bearer_ok => {
            state.running = true;
            json_reply(200, json!(null))
        }
        ("PUT", "/api/v1/instance/inst-1/stop") if bearer_ok => json_reply(
            403,
            json!({ "kind": "PermissionDenied", "causes": ["You don't have permission to stop this instance"] }),
        ),
        ("GET", "/api/v1/public/instances/inst-1/client-pack")
        | ("GET", "/api/v1/launcher/instances/inst-1/client-pack") => match &state.pack {
            Some((_, etag)) if request.if_none_match.as_deref() == Some(etag) => {
                (304, vec![("ETag".into(), etag.clone())], vec![])
            }
            Some((bytes, etag)) => (200, vec![("ETag".into(), etag.clone())], bytes.clone()),
            None => json_reply(
                400,
                json!({ "kind": "BadRequest", "causes": ["Players don't need any mods to join this server"] }),
            ),
        },
        _ if request.authorization.is_some() => json_reply(
            401,
            json!({ "kind": "Unauthorized", "causes": ["Unauthorized"] }),
        ),
        _ => json_reply(404, json!({ "kind": "NotFound", "causes": ["Not found"] })),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
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
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
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
                        if_none_match: header("If-None-Match"),
                        body,
                    };
                    let (status, headers, bytes) = {
                        let mut state = state.lock().unwrap();
                        state.requests.push(recorded.clone());
                        route(&mut state, &recorded)
                    };
                    let mut response =
                        tiny_http::Response::from_data(bytes).with_status_code(status);
                    for (name, value) in headers {
                        response = response.with_header(
                            tiny_http::Header::from_str(&format!("{name}: {value}")).unwrap(),
                        );
                    }
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

    fn modrinth(&self) -> Modrinth {
        Modrinth::with_urls(
            http::default_client().unwrap(),
            self.base.join("v2").unwrap(),
            Some(self.base.clone()),
        )
    }

    fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }
}

#[test]
fn lists_servers_with_and_without_signing_in() {
    let core = MockCore::start();
    let client = core.client();
    let info = block_on(client.info()).unwrap();
    assert_eq!(info.uuid, "core-1");
    assert_eq!(info.core_name, "Home core");
    let public = block_on(client.public_servers()).unwrap();
    assert_eq!(public.instances[0].name, "SMP");
    assert_eq!(public.instances[0].state, ServerState::Stopped);
    // anonymous requests carry no credentials
    assert!(core.requests().iter().all(|r| r.authorization.is_none()));

    let (key, info) = block_on(client.sign_in(
        "steve",
        &Secret::new(PASSWORD),
        "Lodestone Launcher on Mac",
        None,
    ))
    .unwrap();
    assert_eq!(key.expose(), KEY);
    assert_eq!(info.id, "abcdefghijkl");
    let requests = core.requests();
    let create = requests
        .iter()
        .find(|r| r.path == "/api/v1/user/api_keys")
        .unwrap();
    let body: Value = serde_json::from_str(&create.body).unwrap();
    assert_eq!(body["scope"], json!({ "type": "launcher" }));
    assert_eq!(body["name"], "Lodestone Launcher on Mac");
    assert_eq!(body["expires_at"], Value::Null);
    // the password went to the login only, the session token to the key creation only
    let with_password: Vec<&Recorded> = requests
        .iter()
        .filter(|r| {
            r.authorization
                .as_deref()
                .is_some_and(|a| a.starts_with("Basic "))
        })
        .collect();
    assert_eq!(with_password.len(), 1);
    assert_eq!(with_password[0].path, "/api/v1/user/login");

    let signed_in = block_on(client.servers(&key)).unwrap();
    assert_eq!(signed_in.instances[0].state, ServerState::Starting);
}

#[test]
fn reports_bad_passwords_revoked_keys_and_refusals() {
    let core = MockCore::start();
    let client = core.client();
    let err = block_on(client.sign_in("steve", &Secret::new("wrong"), "x", None)).err();
    assert_eq!(err, Some(CoreError::BadCredentials));

    let key = Secret::new(KEY);
    block_on(client.start(&key, "inst-1")).unwrap();
    assert_eq!(
        block_on(client.servers(&key)).unwrap().instances[0].state,
        ServerState::Running
    );
    let err = block_on(client.stop(&key, "inst-1")).unwrap_err();
    assert_eq!(
        err,
        CoreError::Forbidden("You don't have permission to stop this instance".into())
    );
    core.state.lock().unwrap().key_revoked = true;
    assert_eq!(
        block_on(client.servers(&key)).unwrap_err(),
        CoreError::SignedOut
    );
    // never the key in an error
    assert!(!format!("{err:?} {err}").contains("SECRET"));
    assert!(block_on(client.start(&key, "../x")).is_err());

    // nothing listens on this port
    let gone = CoreClient::new(
        http::default_client().unwrap(),
        parse_address("http://127.0.0.1:9").unwrap(),
    );
    let err = block_on(gone.info()).unwrap_err();
    assert!(err.is_offline(), "{err:?}");
}

fn make_pack(files: Value, overrides: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::FileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "name": "SMP client mods",
            "dependencies": { "minecraft": "1.20.1", "fabric-loader": "0.16.14" },
            "files": files,
        });
        zip.write_all(index.to_string().as_bytes()).unwrap();
        for (path, data) in overrides {
            zip.start_file(*path, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn cdn_file(core: &MockCore, name: &str, data: &[u8]) -> Value {
    let path = format!("data/proj/versions/v1/{name}");
    core.state
        .lock()
        .unwrap()
        .cdn
        .insert(path.clone(), data.to_vec());
    json!({
        "path": format!("mods/{name}"),
        "hashes": { "sha1": sha1_hex(data), "sha512": hex(&sha2::Sha512::digest(data)) },
        "env": { "client": "required", "server": "unsupported" },
        "downloads": [format!("https://cdn.modrinth.com/{path}")],
        "fileSize": data.len(),
    })
}

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0)
}

#[test]
fn imports_and_updates_a_servers_client_pack() {
    let core = MockCore::start();
    let client = core.client();
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("minecraft");
    let mods = game.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("mine.jar"), b"the user's own mod").unwrap();

    let first = make_pack(
        json!([
            cdn_file(&core, "sodium.jar", b"sodium 1"),
            cdn_file(&core, "lib.jar", b"lib 1"),
        ]),
        &[("overrides/mods/uploaded.jar", b"uploaded 1")],
    );
    core.state.lock().unwrap().pack = Some((first, "\"e1\"".into()));
    let server = block_on(client.public_servers()).unwrap().instances[0].clone();
    let PackFetch::Pack { bytes, etag } =
        block_on(client.client_pack(None, &server, None)).unwrap()
    else {
        panic!("expected a pack");
    };
    assert_eq!(etag.as_deref(), Some("\"e1\""));
    let parsed = pack::read_pack(&bytes).unwrap();
    let files = block_on(launcher_core::http::in_http_runtime(async {
        pack::apply(
            &parsed,
            &game,
            None,
            &core.modrinth(),
            &downloader(),
            &Progress::default(),
            &CancelToken::new(),
        )
        .await
    }))
    .unwrap();
    assert_eq!(std::fs::read(mods.join("sodium.jar")).unwrap(), b"sodium 1");
    assert_eq!(
        std::fs::read(mods.join("uploaded.jar")).unwrap(),
        b"uploaded 1"
    );
    assert!(!game.join(".lodestone-server-pack").exists());
    let link = ServerLink {
        core_uuid: "core-1".into(),
        core_url: core.base.to_string(),
        server_uuid: server.uuid.clone(),
        server_name: server.name.clone(),
        pack_etag: etag,
        files,
    };
    assert!(pack::changed_on_disk(&link, &mods).is_empty());

    // the same pack again: not modified
    assert!(matches!(
        block_on(client.client_pack(None, &server, link.pack_etag.as_deref())).unwrap(),
        PackFetch::NotModified
    ));

    // the server's mods change: lib removed, sodium updated
    let second = make_pack(
        json!([cdn_file(&core, "sodium.jar", b"sodium 2")]),
        &[("overrides/mods/uploaded.jar", b"uploaded 1")],
    );
    core.state.lock().unwrap().pack = Some((second, "\"e2\"".into()));
    let PackFetch::Pack { bytes, .. } =
        block_on(client.client_pack(None, &server, link.pack_etag.as_deref())).unwrap()
    else {
        panic!("expected a new pack");
    };
    let parsed = pack::read_pack(&bytes).unwrap();
    let changes = pack::changes(Some(&link), &parsed);
    assert_eq!(changes.updated, ["sodium.jar"]);
    assert_eq!(changes.removed, ["lib.jar"]);
    assert!(changes.added.is_empty());
    let files = block_on(launcher_core::http::in_http_runtime(async {
        pack::apply(
            &parsed,
            &game,
            Some(&link),
            &core.modrinth(),
            &downloader(),
            &Progress::default(),
            &CancelToken::new(),
        )
        .await
    }))
    .unwrap();
    assert_eq!(std::fs::read(mods.join("sodium.jar")).unwrap(), b"sodium 2");
    assert!(!mods.join("lib.jar").exists());
    // the user's own mod is never touched
    assert_eq!(
        std::fs::read(mods.join("mine.jar")).unwrap(),
        b"the user's own mod"
    );
    assert_eq!(
        files,
        vec![
            LinkedFile {
                name: "sodium.jar".into(),
                sha1: sha1_hex(b"sodium 2")
            },
            LinkedFile {
                name: "uploaded.jar".into(),
                sha1: sha1_hex(b"uploaded 1")
            },
        ]
    );

    // a damaged download changes nothing
    let broken = {
        let mut file = cdn_file(&core, "new.jar", b"new");
        file["hashes"]["sha512"] = json!("0".repeat(128));
        make_pack(json!([file]), &[])
    };
    let parsed = pack::read_pack(&broken).unwrap();
    let err = block_on(launcher_core::http::in_http_runtime(async {
        pack::apply(
            &parsed,
            &game,
            None,
            &core.modrinth(),
            &downloader(),
            &Progress::default(),
            &CancelToken::new(),
        )
        .await
    }))
    .unwrap_err();
    assert!(err.to_string().contains("SHA-512"), "{err}");
    assert!(!mods.join("new.jar").exists());
    assert!(mods.join("sodium.jar").exists());

    // a server without client mods
    core.state.lock().unwrap().pack = None;
    assert!(matches!(
        block_on(client.client_pack(Some(&Secret::new(KEY)), &server, None)).unwrap(),
        PackFetch::NoMods(message) if message.contains("don't need any mods")
    ));
}
