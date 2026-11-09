//! The core client and the client-pack import (through the modpack import and update)
//! against a mock core on a loopback port, with a Modrinth mock for the CDN and the hash
//! lookups. Each test starts its own mocks.

mod modrinth;

use std::{
    io::{Cursor, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    content::{InstanceDirs, install::InstallProgress, provenance::Provenance},
    cores::{
        CoreClient, CoreError, PackFetch, ServerState,
        link::{self, ServerLink},
        parse_address,
        sync::{self, PackChanges},
        trust,
    },
    game::{
        download::{CancelToken, Downloader, recording_sleep},
        loaders::{LoaderChoice, LoaderKind},
    },
    http,
    instances::InstanceStore,
    modpack::{
        self, ModpackRecord,
        import::{self, ImportOptions},
        update,
    },
};
use modrinth::{FakeVersion, ModrinthMock};
use serde_json::{Value, json};

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
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "name": "SMP client mods",
            "versionId": "SMP (Lodestone server)",
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

/// The index entry of a Modrinth version's file, as the core writes it.
fn index_file(version: &FakeVersion) -> Value {
    json!({
        "path": format!("mods/{}", version.filename),
        "hashes": { "sha1": version.sha1(), "sha512": version.sha512() },
        "env": { "client": "required", "server": "unsupported" },
        "downloads": [version.cdn_url()],
        "fileSize": version.bytes.len(),
    })
}

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1)
}

fn sodium(number: &str) -> FakeVersion {
    FakeVersion::new(
        &format!("sodium-{number}"),
        "AANobbMI",
        number,
        &["fabric"],
        &["1.20.1"],
        "2026-01-01T00:00:00Z",
    )
    .file("sodium.jar", format!("sodium {number}").as_bytes())
}

/// Fetches the server's pack and reads it, as "Create matching instance" does.
fn fetch_pack(
    client: &CoreClient,
    server: &launcher_core::cores::ServerInstance,
    dir: &std::path::Path,
) -> (std::path::PathBuf, modpack::PackInfo, Option<String>) {
    let PackFetch::Pack { bytes, etag } = block_on(client.client_pack(None, server, None)).unwrap()
    else {
        panic!("expected a pack");
    };
    let path = sync::write_pack(dir, &bytes).unwrap();
    let pack = modpack::read_pack(&path).unwrap();
    (path, pack, etag)
}

#[test]
fn imports_and_keeps_a_servers_client_pack_in_step() {
    let core = MockCore::start();
    let client = core.client();
    let modrinth = ModrinthMock::start();
    modrinth.project("AANobbMI", "sodium", "mod");
    modrinth.version(sodium("1")).version(sodium("2"));
    let api = modrinth.client();
    let root = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(root.path().join("instances"));
    std::fs::create_dir_all(store.root()).unwrap();

    let first = make_pack(
        json!([index_file(&sodium("1"))]),
        &[
            ("overrides/mods/uploaded.jar", b"uploaded 1"),
            ("overrides/config/smp.toml", b"pvp = false"),
            ("client-overrides/kubejs/startup_scripts/hello.js", b"// hi"),
        ],
    );
    core.state.lock().unwrap().pack = Some((first, "\"e1\"".into()));
    let server = block_on(client.public_servers()).unwrap().instances[0].clone();
    let (pack_path, pack, etag) = fetch_pack(&client, &server, store.root());
    assert_eq!(etag.as_deref(), Some("\"e1\""));
    let loader = sync::loader_for(Some(&pack), &server).unwrap();
    assert_eq!(
        loader,
        Some(LoaderChoice {
            kind: LoaderKind::Fabric,
            version: "0.16.14".into()
        })
    );

    // the advisory: Modrinth knows sodium by its hash, not the uploaded jar
    let advisory = block_on(trust::advisory(Some(&api), &sync::pack_pulls(&pack)));
    assert_eq!(advisory.on_modrinth.len(), 1);
    assert_eq!(advisory.on_modrinth[0].path, "mods/sodium.jar");
    assert_eq!(advisory.on_modrinth[0].project, "sodium title");
    assert_eq!(advisory.uploaded, ["mods/uploaded.jar"]);
    assert_eq!((advisory.configs, advisory.scripts), (1, 1));
    assert!(!advisory.unchecked);

    let instance = block_on(http::in_http_runtime(import::import(
        &store,
        Some(&api),
        &downloader(),
        &pack_path,
        &pack,
        ImportOptions {
            name: server.name.clone(),
            loader: loader.clone(),
            record: sync::record(&pack),
            icon: None,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    std::fs::remove_file(&pack_path).unwrap();
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.14"));
    let dirs = InstanceDirs::new(store.dir(&instance.id));
    let game = dirs.game.clone();
    assert_eq!(
        std::fs::read(game.join("mods/sodium.jar")).unwrap(),
        b"sodium 1"
    );
    assert_eq!(
        std::fs::read(game.join("mods/uploaded.jar")).unwrap(),
        b"uploaded 1"
    );
    // configs and scripts too, and the modpack records
    assert_eq!(
        std::fs::read(game.join("config/smp.toml")).unwrap(),
        b"pvp = false"
    );
    assert!(game.join("kubejs/startup_scripts/hello.js").is_file());
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert_eq!(record.title, "SMP client mods");
    assert!(record.files.contains_key("mods/uploaded.jar"));
    let provenance = Provenance::load(&dirs.instance);
    assert!(provenance.files.keys().any(|k| k.contains("sodium.jar")));
    // the server link
    let link = ServerLink {
        core_uuid: "core-1".into(),
        core_url: core.base.to_string(),
        server_uuid: server.uuid.clone(),
        server_name: server.name.clone(),
        pack_etag: etag,
    };
    link::save_link(&dirs.instance, &link).unwrap();
    assert_eq!(
        link::linked_instances(store.root()),
        vec![(instance.id.clone(), link.clone())]
    );
    assert!(sync::mods_on_disk(&dirs));

    // the same pack again: not modified
    assert!(matches!(
        block_on(client.client_pack(None, &server, link.pack_etag.as_deref())).unwrap(),
        PackFetch::NotModified
    ));

    // the user adds a mod, changes a server mod and a config
    std::fs::write(game.join("mods/mine.jar"), b"the user's own mod").unwrap();
    std::fs::write(game.join("mods/uploaded.jar"), b"tampered").unwrap();
    std::fs::write(game.join("config/smp.toml"), b"pvp = true").unwrap();
    assert!(!sync::mods_on_disk(&dirs));

    // the server's mods change: sodium updated, the script dropped
    let second = make_pack(
        json!([index_file(&sodium("2"))]),
        &[
            ("overrides/mods/uploaded.jar", b"uploaded 1"),
            ("overrides/config/smp.toml", b"pvp = false"),
        ],
    );
    core.state.lock().unwrap().pack = Some((second, "\"e2\"".into()));
    let (pack_path, pack, etag) = fetch_pack(&client, &server, &dirs.instance);
    assert_eq!(etag.as_deref(), Some("\"e2\""));
    let plan = sync::plan(&dirs, &pack, loader.clone());
    assert!(plan.changes_files());
    let changes = PackChanges::from_plan(&plan);
    // the changed server mod is replaced, the changed config kept
    assert_eq!(changes.updated, ["mods/sodium.jar", "mods/uploaded.jar"]);
    assert!(changes.added.is_empty() && changes.removed.is_empty());
    assert_eq!(plan.overrides_kept, ["config/smp.toml"]);
    let advisory = block_on(trust::advisory(Some(&api), &sync::plan_pulls(&plan)));
    assert_eq!(advisory.on_modrinth[0].version, "2");
    assert_eq!(advisory.uploaded, ["mods/uploaded.jar"]);

    block_on(http::in_http_runtime(update::apply(
        Some(&api),
        &downloader(),
        &dirs,
        &plan,
        &pack_path,
        false,
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    std::fs::remove_file(&pack_path).unwrap();
    assert_eq!(
        std::fs::read(game.join("mods/sodium.jar")).unwrap(),
        b"sodium 2"
    );
    assert_eq!(
        std::fs::read(game.join("mods/uploaded.jar")).unwrap(),
        b"uploaded 1"
    );
    // the user's own mod and config are never touched
    assert_eq!(
        std::fs::read(game.join("mods/mine.jar")).unwrap(),
        b"the user's own mod"
    );
    assert_eq!(
        std::fs::read(game.join("config/smp.toml")).unwrap(),
        b"pvp = true"
    );
    assert!(sync::mods_on_disk(&dirs));
    // in step now
    assert!(!sync::plan(&dirs, &pack, loader).changes_files());

    // a damaged download leaves no instance behind
    let broken = {
        let mut file = index_file(&sodium("1"));
        file["hashes"]["sha512"] = json!("0".repeat(128));
        make_pack(json!([file]), &[])
    };
    core.state.lock().unwrap().pack = Some((broken, "\"e3\"".into()));
    let (pack_path, pack, _) = fetch_pack(&client, &server, store.root());
    let err = block_on(http::in_http_runtime(import::import(
        &store,
        Some(&api),
        &downloader(),
        &pack_path,
        &pack,
        ImportOptions {
            name: "Broken".into(),
            ..ImportOptions::default()
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap_err();
    std::fs::remove_file(&pack_path).unwrap();
    assert!(err.to_string().contains("SHA-512"), "{err}");
    assert_eq!(store.list().0.len(), 1);
    assert!(
        std::fs::read_dir(store.root())
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with(".lodestone")),
        "nothing is left behind"
    );

    // a server without client mods
    core.state.lock().unwrap().pack = None;
    assert!(matches!(
        block_on(client.client_pack(Some(&Secret::new(KEY)), &server, None)).unwrap(),
        PackFetch::NoMods(message) if message.contains("don't need any mods")
    ));
}
