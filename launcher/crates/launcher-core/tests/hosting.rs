//! "Host this" against a mock core on a loopback port (`cores::hosting`): the instance is
//! written as a `.lodeimg` and uploaded, the server made from it and its creation
//! followed to its end. Each test starts its own mock and data folder. Nothing is ever
//! started: the mock records every request, and the tests check that no power route is
//! called and that the EULA is accepted only when the person ticked it.

use std::{
    fs,
    io::Write as _,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::PathBuf,
    str::FromStr as _,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::executor::block_on;
use launcher_core::content::provenance::{self, ModrinthSource, Provenance};
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        admin::AdminClient,
        hosting::{self, HostAuth, HostChoices, HostError},
    },
    game::download::{CancelToken, Sleep},
    game::loaders::{LoaderChoice, LoaderKind},
    http,
    images::Images,
    instances::{Instance, InstanceStore},
    paths::DataDir,
};
use lodestone_image::container::{self, Limits};
use lodestone_image::{EnvValue, Side};
use serde_json::{Value, json};
use sha1::Digest as _;

const PASSWORD: &str = "hunter2-PASSWORD";
const SESSION: &str = "session-jwt-SECRET";
/// The owner key "Connect as owner" gives.
const OWNER_KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";
const UPLOAD_ID: &str = "upload-1";
const SERVER: &str = "new-server-uuid";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Vec<u8>,
}

/// How the mock core behaves.
#[derive(Default)]
struct State {
    requests: Vec<Recorded>,
    /// A core from before milestone 6: no image routes.
    too_old: bool,
    /// The user may not create servers.
    forbidden: bool,
    /// Answer an upload this long after it arrived.
    upload_delay: Option<Duration>,
    /// What `GET /events/all/buffer` answers.
    events: Vec<Value>,
    /// What `GET /instance/list` answers.
    listed: Vec<Value>,
    /// The signed-in user may not change global files, and the core refuses a creation
    /// whose file needs it.
    no_global_files: bool,
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

/// The file in a `multipart/form-data` body with one field, or `None` when it has none.
fn multipart_file(body: &[u8]) -> Option<Vec<u8>> {
    let start = body.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let end = body.windows(4).rposition(|w| w == b"\r\n--")?;
    (end > start).then(|| body[start..end].to_vec())
}

fn summary() -> Value {
    json!({
        "image": "image-1", "file_name": "Survival.lodeimg", "label": "Survival",
        "made_by": "launcher", "producer_version": "1.0.0", "minecraft": "1.21.1",
        "loader": "fabric", "loader_version": "0.16.10", "server": "Fabric",
        "server_choices": ["Fabric"],
        "client_only": {"names": ["zoom.jar"], "count": 1},
        "worlds": [{"folder": "A", "size": 20}], "default_world": "A"
    })
}

fn info(uuid: &str) -> Value {
    json!({
        "uuid": uuid, "name": "Survival",
        "game_type": {"type": "MinecraftJava", "variant": {"type": "Fabric"}},
        "version": "1.21.1", "port": 25565, "creation_time": 1, "path": "/s",
        "auto_start": false, "restart_on_crash": false, "state": "Stopped"
    })
}

fn progression(id: i64, snowflake: i64, inner: Value) -> Value {
    json!({
        "event_inner": {"type": "ProgressionEvent", "event_id": id.to_string(),
                        "progression_event_inner": inner},
        "details": "", "snowflake": snowflake.to_string(), "level": "Info",
        "caused_by": {"type": "System"}
    })
}

/// A creation's events, as the core buffers them: its start, an update and its end, with
/// what the server didn't get.
fn creation_events() -> Vec<Value> {
    let mut end = info(SERVER);
    end["type"] = json!("InstanceCreation");
    end["outcome"] = json!({
        "not_downloaded": {"names": ["mods/gone.jar"], "count": 1},
        "secrets": {"names": ["forwarding.secret"], "count": 1}
    });
    vec![
        progression(
            9,
            9,
            json!({"type": "ProgressionStart", "progression_name": "Setting up", "total": 10.0,
                   "inner": {"type": "InstanceCreation", "instance_uuid": SERVER}}),
        ),
        progression(
            9,
            10,
            json!({"type": "ProgressionUpdate", "progress_message": "Placing", "progress": 5.0}),
        ),
        progression(
            9,
            11,
            json!({"type": "ProgressionEnd", "success": true,
                   "message": "Server created successfully. 1 file(s) couldn't be downloaded and were left out: mods/gone.jar",
                   "inner": end}),
        ),
    ]
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<u8>) {
    let reply = |status: u16, value: Value| (status, value.to_string().into_bytes());
    let bearer = |token: &str| request.authorization.as_deref() == Some(&format!("Bearer {token}"));
    let allowed = bearer(SESSION) || bearer(OWNER_KEY);
    let path = request.path.split('?').next().unwrap_or_default();
    if path == "/api/v1/user/login" && request.method == "POST" {
        let expected = format!(
            "Basic {}",
            base64_encode(format!("steve:{PASSWORD}").as_bytes())
        );
        return if request.authorization.as_deref() == Some(expected.as_str()) {
            reply(
                200,
                json!({ "token": SESSION, "user": {"uid": "u1", "username": "steve"} }),
            )
        } else {
            reply(401, json!({ "causes": ["Credential mismatch"] }))
        };
    }
    if !allowed {
        return reply(401, json!({ "causes": ["Unauthorized"] }));
    }
    if let Some(port) = path.strip_prefix("/api/v1/check/port/") {
        let taken = port == "25565";
        return reply(200, json!({ "is_in_use": taken, "is_allocated": false }));
    }
    let image_route =
        path.starts_with("/api/v1/instance/image/") || path == "/api/v1/instance/create_from_image";
    if image_route && state.too_old {
        // what a core answers for a route it doesn't have: nothing
        return (404, Vec::new());
    }
    if image_route && state.forbidden {
        return reply(
            403,
            json!({ "kind": "PermissionDenied", "causes": ["Not authorized to create instances"] }),
        );
    }
    match (request.method.as_str(), path) {
        ("POST", "/api/v1/instance/image/upload") => match multipart_file(&request.body) {
            None => reply(
                400,
                json!({ "kind": "BadRequest", "causes": ["No file was uploaded"] }),
            ),
            Some(_) => {
                if let Some(delay) = state.upload_delay {
                    std::thread::sleep(delay);
                }
                reply(200, json!({ "upload_id": UPLOAD_ID, "summary": summary() }))
            }
        },
        ("DELETE", p) if p == format!("/api/v1/instance/image/upload/{UPLOAD_ID}") => {
            reply(200, json!(null))
        }
        ("GET", "/api/v1/user/info") => reply(
            200,
            json!({ "uid": "u1", "username": "steve", "is_owner": bearer(OWNER_KEY),
                    "permissions": {"can_create_instance": true,
                                    "can_write_global_file": !state.no_global_files} }),
        ),
        ("POST", "/api/v1/instance/create_from_image") if state.no_global_files => reply(
            403,
            json!({ "kind": "PermissionDenied",
                    "causes": ["This file has protected files: making a server from it needs permission to change global files"] }),
        ),
        ("POST", "/api/v1/instance/create_from_image") => {
            state.events = creation_events();
            reply(200, json!(SERVER))
        }
        ("GET", "/api/v1/events/all/buffer") => reply(200, json!(state.events)),
        ("GET", "/api/v1/instance/list") => reply(200, json!(state.listed)),
        _ => (404, Vec::new()),
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
                    let state = state.clone();
                    // a thread per request: an upload the mock delays mustn't hold the rest
                    std::thread::spawn(move || {
                        let mut body = Vec::new();
                        let _ = request.as_reader().read_to_end(&mut body);
                        let authorization = request
                            .headers()
                            .iter()
                            .find(|h| h.field.equiv("Authorization"))
                            .map(|h| h.value.to_string());
                        let recorded = Recorded {
                            method: request.method().to_string(),
                            path: request.url().to_string(),
                            authorization,
                            body,
                        };
                        state.lock().unwrap().requests.push(recorded.clone());
                        let delay = state.lock().unwrap().upload_delay;
                        let (status, bytes) = {
                            let mut guard = state.lock().unwrap();
                            if let Some(delay) =
                                delay.filter(|_| recorded.path.contains("/image/upload"))
                            {
                                // answered outside the lock, after the delay
                                guard.upload_delay = None;
                                drop(guard);
                                std::thread::sleep(delay);
                                let mut guard = state.lock().unwrap();
                                route(&mut guard, &recorded)
                            } else {
                                route(&mut guard, &recorded)
                            }
                        };
                        let response = tiny_http::Response::from_data(bytes)
                            .with_status_code(status)
                            .with_header(
                                tiny_http::Header::from_str("Content-Type: application/json")
                                    .unwrap(),
                            );
                        let _ = request.respond(response);
                    });
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

    fn owner(&self) -> AdminClient {
        AdminClient::new(self.client(), Secret::new(OWNER_KEY))
    }

    fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }

    fn uploads_with_a_file(&self) -> Vec<Vec<u8>> {
        self.requests()
            .iter()
            .filter(|r| r.method == "POST" && r.path.ends_with("/instance/image/upload"))
            .filter_map(|r| multipart_file(&r.body))
            .collect()
    }

    fn has(&self, method: &str, path: &str) -> bool {
        self.requests()
            .iter()
            .any(|r| r.method == method && r.path.split('?').next() == Some(path))
    }

    /// No request ever started, stopped or otherwise powered a server.
    fn assert_nothing_started(&self) {
        for request in self.requests() {
            for word in ["/start", "/stop", "/restart", "/kill", "/power"] {
                assert!(!request.path.contains(word), "{request:?}");
            }
        }
    }
}

/// A Fabric instance with a Modrinth mod content.json knows, a mod whose jar says it runs
/// only on clients, two worlds (`A` played last), and a player's private files.
struct Fixture {
    _temp: tempfile::TempDir,
    data: DataDir,
    images: Arc<Images>,
    instance: Instance,
    store: InstanceStore,
}

impl Fixture {
    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.store.game_dir(&self.instance.id).join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn temp_dir(&self) -> PathBuf {
        self.data.cache_dir().join(hosting::UPLOADS_FOLDER)
    }

    fn temp_files(&self) -> Vec<String> {
        fs::read_dir(self.temp_dir())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }
}

/// A jar whose `fabric.mod.json` says it runs on `environment`.
fn fabric_jar(id: &str, environment: &str) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    out.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    out.write_all(
        format!(r#"{{"schemaVersion":1,"id":"{id}","version":"1","environment":"{environment}"}}"#)
            .as_bytes(),
    )
    .unwrap();
    out.finish().unwrap().into_inner()
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path().join("data"));
    let store = InstanceStore::new(data.instances_dir());
    let instance = store
        .create(
            "Survival",
            "1.21.1",
            Some(&LoaderChoice {
                kind: LoaderKind::Fabric,
                version: "0.16.10".into(),
            }),
        )
        .unwrap();
    let f = Fixture {
        images: Images::open(&data),
        data,
        store,
        instance,
        _temp: temp,
    };
    f.write("mods/zoom.jar", &fabric_jar("zoom", "client"));
    f.write("mods/lithium.jar", &fabric_jar("lithium", "*"));
    f.write("config/lithium.properties", b"mixin.ai=false\n");
    f.write("saves/A/level.dat", b"level A");
    f.write("saves/A/region/r.0.0.mca", b"region A");
    f.write("saves/B/level.dat", b"level B");
    f.write("options.txt", b"fov:90");
    f.write("servers.dat", b"server list");
    f.write("logs/latest.log", b"[CHAT] <Steve> hello");
    f.write("logs/2026-01-01-1.log.gz", b"old log");
    f.write("crash-reports/crash-2026-01-01.txt", b"crash");
    f.write("screenshots/2026-01-01.png", b"png");
    f.write("usercache.json", b"[]");
    f.write("XaeroWaypoints/Multiplayer_example/waypoints.txt", b"home");
    f.write("command_history.txt", b"/login hunter2");
    f.write("launcher_accounts.json", b"{\"accessToken\":\"official\"}");
    f.write(
        "hs_err_pid4242.log",
        b"java ... --accessToken eyJsecret ...",
    );
    // content.json knows both mods from Modrinth, with their sides: Zoom runs only on
    // clients, so the core's server view leaves it out
    let mut known = Provenance::default();
    for (name, project, client, server) in [
        ("zoom.jar", "ZOOMzoom", "required", "unsupported"),
        ("lithium.jar", "gvQqBUqZ", "optional", "required"),
    ] {
        let path = f.store.game_dir(&f.instance.id).join("mods").join(name);
        let bytes = fs::read(&path).unwrap();
        let meta = fs::metadata(&path).unwrap();
        known.files.insert(
            format!("mods/{name}"),
            provenance::Entry {
                size: meta.len(),
                modified: meta.modified().ok().and_then(provenance::nanos),
                sha1: format!("{:x}", sha1::Sha1::digest(&bytes)),
                sha512: format!("{:x}", sha2::Sha512::digest(&bytes)),
                modrinth: Some(ModrinthSource {
                    project_id: project.into(),
                    version_id: "v1".into(),
                    url: Some(format!(
                        "https://cdn.modrinth.com/data/{project}/versions/v1/{name}"
                    )),
                    client_side: client.into(),
                    server_side: server.into(),
                    ..ModrinthSource::default()
                }),
                ..provenance::Entry::default()
            },
        );
    }
    known.save(&f.store.dir(&f.instance.id)).unwrap();
    f.images
        .record_now(&f.instance.id, "Created Survival")
        .unwrap();
    f
}

fn upload(
    f: &Fixture,
    admin: &AdminClient,
    worlds: &[String],
) -> Result<hosting::Uploaded, HostError> {
    block_on(hosting::upload(
        admin,
        &f.images,
        &f.instance,
        worlds,
        &f.temp_dir(),
        &hosting::HostProgress::default(),
        &CancelToken::new(),
    ))
}

/// A sleep that returns at once.
fn no_sleep() -> Sleep {
    Arc::new(|_| Box::pin(async {}))
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_core_is_asked_first_whether_it_makes_servers_from_files() {
    let core = MockCore::start();
    block_on(hosting::probe(&core.owner())).unwrap();
    // the probe sent no file
    assert!(core.uploads_with_a_file().is_empty());

    core.state.lock().unwrap().forbidden = true;
    let refused = block_on(hosting::probe(&core.owner()));
    assert!(
        matches!(refused, Err(HostError::Forbidden(_))),
        "{refused:?}"
    );

    let old = MockCore::start();
    old.state.lock().unwrap().too_old = true;
    let refused = block_on(hosting::probe(&old.owner()));
    assert!(
        matches!(refused, Err(HostError::NeedsUpdate)),
        "{refused:?}"
    );
    assert!(
        refused
            .unwrap_err()
            .to_string()
            .contains("too old to make servers from files")
    );
}

#[test]
fn a_password_sign_in_makes_a_session_for_the_calls() {
    let core = MockCore::start();
    let admin = block_on(
        HostAuth::Password {
            username: "steve".into(),
            password: Secret::new(PASSWORD),
        }
        .admin(core.client()),
    )
    .unwrap();
    block_on(hosting::probe(&admin)).unwrap();
    assert_eq!(
        block_on(admin.free_port(hosting::FIRST_PORT, &CancelToken::new())).unwrap(),
        25566
    );
    let wrong = block_on(
        HostAuth::Password {
            username: "steve".into(),
            password: Secret::new("wrong"),
        }
        .admin(core.client()),
    );
    assert!(wrong.is_err());
}

/// The upload is a `.lodeimg` the shared container opens: the chosen world only, jars and
/// configs as the instance has them with their sides (so the core's server view drops the
/// client-only mod), never a player's private files; named after the instance.
#[test]
fn the_upload_is_the_instance_with_the_chosen_world_and_nothing_private() {
    let f = fixture();
    let core = MockCore::start();
    let admin = core.owner();
    let mut uploaded = upload(&f, &admin, &["A".to_string()]).unwrap();
    assert_eq!(uploaded.upload.upload_id, UPLOAD_ID);
    // the core has it: the temporary file is gone already, before the summary is read
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());
    assert!(
        uploaded.may_write_global,
        "the owner may change global files"
    );
    assert_eq!(uploaded.upload.summary.client_only.names, ["zoom.jar"]);
    let request = core
        .requests()
        .into_iter()
        .find(|r| r.path.ends_with("/instance/image/upload") && multipart_file(&r.body).is_some())
        .unwrap();
    let header = String::from_utf8_lossy(&request.body[..200.min(request.body.len())]).into_owned();
    assert!(header.contains("filename=\"Survival.lodeimg\""), "{header}");
    let file = f.data.root().join("sent.lodeimg");
    fs::write(&file, multipart_file(&request.body).unwrap()).unwrap();
    let opened = container::open(&file, &Limits::default()).unwrap();
    let placed = lodestone_image::share::placed_entries(opened.image(), opened.linked());
    let paths: Vec<&str> = placed.iter().map(|e| e.path.as_str()).collect();
    for present in [
        "mods/zoom.jar",
        "mods/lithium.jar",
        "config/lithium.properties",
        "saves/A/level.dat",
        "saves/A/region/r.0.0.mca",
    ] {
        assert!(paths.contains(&present), "{present} in {paths:?}");
    }
    // only the content and the picked world: none of the player's other files
    for absent in [
        "saves/B/level.dat",
        "command_history.txt",
        "launcher_accounts.json",
        "hs_err_pid4242.log",
        "options.txt",
        "servers.dat",
        "logs/latest.log",
        "logs/2026-01-01-1.log.gz",
        "crash-reports/crash-2026-01-01.txt",
        "screenshots/2026-01-01.png",
        "usercache.json",
        "XaeroWaypoints/Multiplayer_example/waypoints.txt",
    ] {
        assert!(!paths.contains(&absent), "{absent} in {paths:?}");
    }
    assert_eq!(opened.image().label.as_deref(), Some("Survival"));
    let worlds: Vec<&str> = opened
        .image()
        .worlds
        .iter()
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(worlds, ["A"]);
    // the core's server view leaves the client-only mod out
    let zoom = placed
        .iter()
        .find(|e| e.path.as_str() == "mods/zoom.jar")
        .unwrap();
    assert_eq!(zoom.env.server, EnvValue::Unsupported);
    let mut whole = opened.linked()[0].clone();
    whole.entries = placed.clone();
    whole.worlds = opened.image().worlds.clone();
    whole.kind = lodestone_image::Kind::Full;
    whole.content = None;
    whole.seal();
    let server_view = lodestone_image::view::view(&whole, Side::Server, whole.rules).unwrap();
    assert!(
        !server_view
            .entries
            .iter()
            .any(|e| e.path.as_str() == "mods/zoom.jar")
    );
    assert!(
        server_view
            .entries
            .iter()
            .any(|e| e.path.as_str() == "mods/lithium.jar")
    );
    // the instance itself kept nothing of it: no snapshot
    assert!(
        f.images
            .snapshots(&f.instance.id)
            .unwrap()
            .records
            .is_empty()
    );

    // the server: the EULA only when ticked, the world picked, never started
    let choices = HostChoices {
        name: "Survival".into(),
        port: 25566,
        world: Some("A".into()),
        flavour: None,
        min_ram: None,
        max_ram: Some(4096),
        accept_eula: false,
    };
    let uuid = block_on(hosting::create(&admin, &mut uploaded, &choices)).unwrap();
    assert_eq!(uuid, SERVER);
    let create = core
        .requests()
        .into_iter()
        .find(|r| r.path.ends_with("/instance/create_from_image"))
        .unwrap();
    let body: Value = serde_json::from_slice(&create.body).unwrap();
    assert_eq!(body["accept_eula"], false);
    assert_eq!(body["world"], "A");
    assert_eq!(body["upload_id"], UPLOAD_ID);
    assert_eq!(body["image"], "image-1");
    assert_eq!(body["keep_operators"], false);
    assert!(body.get("auto_start").is_none());
    // made: the upload is the core's now, so dropping it lets nothing go
    drop(uploaded);
    std::thread::sleep(Duration::from_millis(200));
    assert!(!core.has(
        "DELETE",
        &format!("/api/v1/instance/image/upload/{UPLOAD_ID}")
    ));
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());

    // the creation's end, with what the server didn't get
    let seen = Mutex::new(Vec::new());
    let end = block_on(hosting::wait_created(
        &admin,
        &uuid,
        &|fraction| seen.lock().unwrap().push(fraction),
        &CancelToken::new(),
        &no_sleep(),
    ))
    .unwrap();
    assert!(end.success);
    let outcome = end.outcome.unwrap();
    assert_eq!(outcome.not_downloaded.names, ["mods/gone.jar"]);
    assert_eq!(outcome.secrets.names, ["forwarding.secret"]);
    core.assert_nothing_started();

    // ticked: accepted
    let mut again = upload(&f, &admin, &[]).unwrap();
    block_on(hosting::create(
        &admin,
        &mut again,
        &HostChoices {
            accept_eula: true,
            world: None,
            ..choices
        },
    ))
    .unwrap();
    let create = core
        .requests()
        .into_iter()
        .rfind(|r| r.path.ends_with("/instance/create_from_image"))
        .unwrap();
    let body: Value = serde_json::from_slice(&create.body).unwrap();
    assert_eq!(body["accept_eula"], true);
    assert!(body.get("world").is_none());
    core.assert_nothing_started();
}

/// The paths a mock core received in its upload.
fn sent_paths(f: &Fixture, core: &MockCore) -> (lodestone_image::Kind, Vec<String>) {
    let request = core
        .requests()
        .into_iter()
        .rfind(|r| r.path.ends_with("/instance/image/upload") && multipart_file(&r.body).is_some())
        .unwrap();
    let file = f.data.root().join("sent-again.lodeimg");
    fs::write(&file, multipart_file(&request.body).unwrap()).unwrap();
    let opened = container::open(&file, &Limits::default()).unwrap();
    let placed = lodestone_image::share::placed_entries(opened.image(), opened.linked());
    (
        opened.image().kind,
        placed.iter().map(|e| e.path.to_string()).collect(),
    )
}

#[test]
fn content_that_cant_be_recorded_still_sends_only_the_content_and_the_worlds() {
    let f = fixture();
    // A saved "Find it for me" search keeps the content from being recorded, so the
    // upload is one full image; it still holds nothing of the player's but the world.
    f.write("mods/new.jar", &fabric_jar("new", "*"));
    fs::write(
        f.store
            .dir(&f.instance.id)
            .join(launcher_core::content::mod_finder::FILE_NAME),
        b"{}",
    )
    .unwrap();
    let core = MockCore::start();
    drop(upload(&f, &core.owner(), &["A".to_string()]).unwrap());
    let (kind, paths) = sent_paths(&f, &core);
    assert_eq!(kind, lodestone_image::Kind::Full);
    for present in ["mods/new.jar", "mods/lithium.jar", "saves/A/level.dat"] {
        assert!(paths.iter().any(|p| p == present), "{present} in {paths:?}");
    }
    for absent in [
        "saves/B/level.dat",
        "options.txt",
        "servers.dat",
        "logs/latest.log",
        "screenshots/2026-01-01.png",
        "crash-reports/crash-2026-01-01.txt",
        "command_history.txt",
        "XaeroWaypoints/Multiplayer_example/waypoints.txt",
    ] {
        assert!(!paths.iter().any(|p| p == absent), "{absent} in {paths:?}");
    }
}

#[test]
fn an_upload_dropped_before_a_server_is_made_is_let_go_of() {
    let f = fixture();
    let core = MockCore::start();
    let uploaded = upload(&f, &core.owner(), &["A".to_string()]).unwrap();
    drop(uploaded);
    wait_until("the upload is let go of", || {
        core.has(
            "DELETE",
            &format!("/api/v1/instance/image/upload/{UPLOAD_ID}"),
        )
    });
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());
    assert!(!core.has("POST", "/api/v1/instance/create_from_image"));
}

#[test]
fn cancel_leaves_no_file_and_makes_no_server() {
    let f = fixture();
    let core = MockCore::start();
    // cancelled before the export: nothing is written or sent
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled = block_on(hosting::upload(
        &core.owner(),
        &f.images,
        &f.instance,
        &["A".to_string()],
        &f.temp_dir(),
        &hosting::HostProgress::default(),
        &cancel,
    ));
    assert!(
        matches!(cancelled, Err(HostError::Cancelled)),
        "{cancelled:?}"
    );
    assert!(core.uploads_with_a_file().is_empty());
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());

    // cancelled while the core reads the file: no server, no file left here
    core.state.lock().unwrap().upload_delay = Some(Duration::from_secs(2));
    let cancel = CancelToken::new();
    let progress = Arc::new(hosting::HostProgress::default());
    {
        let (cancel, progress) = (cancel.clone(), progress.clone());
        std::thread::spawn(move || {
            // sending, or (every byte sent) the core reading it
            while progress.stage() == hosting::HostStage::Exporting {
                std::thread::sleep(Duration::from_millis(5));
            }
            std::thread::sleep(Duration::from_millis(300));
            cancel.cancel();
        });
    }
    let cancelled = block_on(hosting::upload(
        &core.owner(),
        &f.images,
        &f.instance,
        &["A".to_string()],
        &f.temp_dir(),
        &progress,
        &cancel,
    ));
    assert!(
        matches!(cancelled, Err(HostError::Cancelled)),
        "{cancelled:?}"
    );
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());
    assert!(!core.has("POST", "/api/v1/instance/create_from_image"));
    core.assert_nothing_started();
}

#[test]
fn an_older_core_is_too_old_and_nothing_is_left() {
    let f = fixture();
    let core = MockCore::start();
    core.state.lock().unwrap().too_old = true;
    let refused = upload(&f, &core.owner(), &["A".to_string()]);
    assert!(
        matches!(refused, Err(HostError::NeedsUpdate)),
        "{refused:?}"
    );
    assert!(f.temp_files().is_empty(), "{:?}", f.temp_files());
}

#[test]
fn a_quilt_instance_is_refused_before_anything_is_written() {
    let mut f = fixture();
    f.instance.loader = launcher_core::instances::Loader::Quilt;
    let core = MockCore::start();
    let refused = upload(&f, &core.owner(), &[]);
    assert!(matches!(refused, Err(HostError::Quilt)), "{refused:?}");
    assert!(core.requests().is_empty());
    assert!(f.temp_files().is_empty());
}

/// A busy core's buffer can lose a creation: the server being listed then counts as made,
/// without an outcome.
#[test]
fn a_creation_the_buffer_lost_counts_as_made_once_listed() {
    let core = MockCore::start();
    core.state.lock().unwrap().listed = vec![info(SERVER)];
    let end = block_on(hosting::wait_created(
        &core.owner(),
        SERVER,
        &|_| {},
        &CancelToken::new(),
        &no_sleep(),
    ))
    .unwrap();
    assert!(end.success);
    assert_eq!(end.outcome, None);
    // Cancel stops only the waiting
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled = block_on(hosting::wait_created(
        &core.owner(),
        "another",
        &|_| {},
        &cancel,
        &no_sleep(),
    ));
    assert!(
        matches!(cancelled, Err(HostError::Cancelled)),
        "{cancelled:?}"
    );
}

/// The instance is linked to its new server only once the creation ended well: a failed
/// creation, or one the Launcher stopped waiting for, leaves the earlier link as it was.
#[test]
fn the_link_is_saved_only_when_the_server_was_made() {
    let f = fixture();
    let dir = f.store.dir(&f.instance.id);
    let old = hosting::host_link("core-0", "http://old", "old-server", "Old SMP");
    launcher_core::cores::link::save_link(&dir, &old).unwrap();
    let link = hosting::host_link("core-1", "http://core", SERVER, "Survival");
    let core = MockCore::start();
    let follow = |core: &MockCore| {
        block_on(hosting::wait_created(
            &core.owner(),
            SERVER,
            &|_| {},
            &CancelToken::new(),
            &no_sleep(),
        ))
    };

    // the core's end says it failed
    let mut events = creation_events();
    events[2]["event_inner"]["progression_event_inner"]["success"] = json!(false);
    events[2]["event_inner"]["progression_event_inner"]["message"] = json!("No Java 21");
    core.state.lock().unwrap().events = events;
    let linked = hosting::link_when_made(&dir, follow(&core), &link).unwrap();
    assert_eq!(linked, hosting::Linked::Failed(Some("No Java 21".into())));
    assert_eq!(
        launcher_core::cores::link::load_link(&dir),
        Some(old.clone())
    );

    // the wait stopped before the end
    let stopped = hosting::link_when_made(&dir, Err(HostError::StillCreating), &link);
    assert!(
        matches!(stopped, Err(HostError::StillCreating)),
        "{stopped:?}"
    );
    assert_eq!(launcher_core::cores::link::load_link(&dir), Some(old));

    // made: linked, naming the server it was linked to before
    core.state.lock().unwrap().events = creation_events();
    let linked = hosting::link_when_made(&dir, follow(&core), &link).unwrap();
    let hosting::Linked::Made { end, previous } = linked else {
        panic!("{linked:?}");
    };
    assert!(end.success);
    assert_eq!(previous.as_deref(), Some("Old SMP"));
    assert_eq!(launcher_core::cores::link::load_link(&dir), Some(link));
}

/// An instance deleted while its server was being made isn't brought back by its link:
/// the server was made, nothing is saved.
#[test]
fn a_deleted_instance_isnt_brought_back_by_its_link() {
    let temp = tempfile::tempdir().unwrap();
    let gone = temp.path().join("gone-instance");
    let link = hosting::host_link("core-1", "http://core", SERVER, "Survival");
    let core = MockCore::start();
    core.state.lock().unwrap().events = creation_events();
    let ended = block_on(hosting::wait_created(
        &core.owner(),
        SERVER,
        &|_| {},
        &CancelToken::new(),
        &no_sleep(),
    ));
    let linked = hosting::link_when_made(&gone, ended, &link).unwrap();
    let hosting::Linked::InstanceGone { end } = linked else {
        panic!("{linked:?}");
    };
    assert!(end.success);
    assert!(!gone.exists(), "the folder isn't made again");
}

/// A password user who may not change global files is known to before Create (the
/// summary warns then), and a refusal at creation says the core's own reason.
#[test]
fn a_user_who_cant_change_global_files_is_told_why() {
    let f = fixture();
    let core = MockCore::start();
    core.state.lock().unwrap().no_global_files = true;
    let admin = block_on(
        HostAuth::Password {
            username: "steve".into(),
            password: Secret::new(PASSWORD),
        }
        .admin(core.client()),
    )
    .unwrap();
    let mut uploaded = upload(&f, &admin, &["A".to_string()]).unwrap();
    assert!(!uploaded.may_write_global);
    let refused = block_on(hosting::create(
        &admin,
        &mut uploaded,
        &hosting::HostChoices {
            name: "Survival".into(),
            port: 25566,
            world: Some("A".into()),
            flavour: None,
            min_ram: None,
            max_ram: None,
            accept_eula: true,
        },
    ));
    match refused {
        Err(HostError::Refused(message)) => {
            assert!(message.contains("change global files"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    core.assert_nothing_started();
}
