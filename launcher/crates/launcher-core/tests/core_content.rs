//! "Host this modpack" against a mock core on a loopback port. Each test starts its
//! own mock.

use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{Arc, Mutex},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        hosting::{self, HostSettings},
    },
    export,
    game::loaders::{LoaderChoice, LoaderKind},
    http,
    instances::InstanceStore,
};
use serde_json::{Value, json};

const PASSWORD: &str = "hunter2-PASSWORD";
const SESSION: &str = "session-jwt-SECRET";
/// The owner key "Connect as owner" gives, which may create servers too.
const OWNER_KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";

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
    let bearer = |token: &str| request.authorization.as_deref() == Some(&format!("Bearer {token}"));
    let allowed = bearer(SESSION) || bearer(OWNER_KEY);
    let path = request.path.as_str();
    if let Some(port) = path.strip_prefix("/api/v1/check/port/") {
        if !allowed {
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
        ("POST", "/api/v1/modpack/upload") if allowed => json_reply(
            200,
            json!({ "name": "My Pack", "version_number": null, "summary": null, "game_version": "1.20.1",
                    "loader": "fabric", "loader_version": "0.16.14", "server_file_count": 1,
                    "server_download_size": 10, "unsupported_reason": null, "upload_id": "upload-1" }),
        ),
        ("POST", "/api/v1/instance/create_modpack") if allowed => {
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

/// On a core claimed on this computer, the owner key makes the server: no sign-in.
#[test]
fn hosting_with_the_owner_key_needs_no_password() {
    let mock = MockCore::start();
    let client = mock.client();
    let key = Secret::new(OWNER_KEY);
    let uuid = block_on(async {
        let port = client.free_port(&key, hosting::FIRST_PORT).await.unwrap();
        let uploaded = client
            .upload_pack(&key, "Pack.mrpack", b"PK")
            .await
            .unwrap();
        client
            .create_server(
                &key,
                uploaded.upload_id.as_deref().unwrap(),
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
    assert!(requests.iter().all(|r| r.path != "/api/v1/user/login"));
    assert!(
        requests.iter().all(|r| {
            r.authorization.as_deref() == Some(format!("Bearer {OWNER_KEY}").as_str())
        })
    );
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
