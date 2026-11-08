//! A server's live view and linking Minecraft accounts, against a mock core on loopback
//! ports: its REST routes on one, its WebSocket streams on another. Each test starts its
//! own mock. Nothing here prints a key or token: comparisons that involve one use
//! `assert!` without the values.

// tungstenite's handshake callback returns its own (large) error response
#![allow(clippy::result_large_err)]

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::{channel::mpsc, executor::block_on};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError, ServerState,
        live::{
            ConsoleLine, LauncherAccess, LauncherScope, LiveMessage, LiveStream, StreamKind,
            chat_command,
        },
    },
    http,
};
use serde_json::{Value, json};
use tungstenite::{
    Message,
    handshake::server::{ErrorResponse, Request, Response},
};

const KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";
const SESSION: &str = "session-jwt-SECRET";
const MC_TOKEN: &str = "minecraft-access-TOKEN-SECRET";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

struct MockRest {
    base: url::Url,
    requests: Arc<Mutex<Vec<Recorded>>>,
    server: Arc<tiny_http::Server>,
}

impl Drop for MockRest {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

fn console_event(snowflake: i64, message: &str) -> Value {
    json!({
        "event_inner": { "type": "InstanceEvent", "instance_uuid": "inst-1", "instance_name": "SMP",
                         "instance_event_inner": { "type": "InstanceOutput", "message": message } },
        "details": "", "snowflake": snowflake.to_string(), "level": "Info",
        "caused_by": { "type": "System" }
    })
}

fn route(request: &Recorded) -> (u16, Value) {
    let keyed = request.authorization.as_deref() == Some(&format!("Bearer {KEY}"));
    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/api/v1/user/login") => (200, json!({ "token": SESSION })),
        ("POST", "/api/v1/user/api_keys")
            if request.authorization.as_deref() == Some(&format!("Bearer {SESSION}")) =>
        {
            let body: Value = serde_json::from_str(&request.body).unwrap();
            (
                200,
                json!({ "key": KEY, "info": { "id": "abcdefghijkl", "name": "x", "expires_at": null,
                                               "scope": body["scope"] } }),
            )
        }
        _ if !keyed => (
            401,
            json!({ "kind": "Unauthorized", "causes": ["Unauthorized"] }),
        ),
        ("GET", "/api/v1/launcher/instances/inst-1/access") => (
            200,
            json!({ "view": true, "start": true, "stop": false, "console_read": true, "console_write": false }),
        ),
        ("GET", "/api/v1/instance/inst-1/console/buffer") => (
            200,
            // out of order, with a repeat and another server's line
            json!([console_event(3, "third"), console_event(1, "first"), console_event(3, "third"),
                   { "event_inner": { "type": "InstanceEvent", "instance_uuid": "other",
                                      "instance_event_inner": { "type": "InstanceOutput", "message": "x" } },
                     "snowflake": "2" }]),
        ),
        ("GET", "/api/v1/instance/inst-1/players") => (
            200,
            json!([{ "type": "MinecraftPlayer", "name": "steve", "uuid": null },
                   { "type": "MinecraftPlayer", "name": "Alex", "uuid": "853c80ef3c3749fdaa49938b674adae6" }]),
        ),
        ("POST", "/api/v1/instance/inst-1/console") => (200, Value::Null),
        ("POST", "/api/v1/instance/inst-2/console") => (
            403,
            json!({ "kind": "PermissionDenied", "causes": ["You don't have permission to access this instance's console"] }),
        ),
        ("POST", "/api/v1/user/minecraft_links") => {
            let body: Value = serde_json::from_str(&request.body).unwrap();
            if body["access_token"] == MC_TOKEN {
                (
                    200,
                    json!({ "uid": "u1", "minecraft_uuid": body["minecraft_uuid"], "minecraft_name": "Steve",
                            "verified": true, "linked_at": 1 }),
                )
            } else {
                (
                    400,
                    json!({ "kind": "BadRequest", "causes": ["Minecraft's services refused the account's token."] }),
                )
            }
        }
        _ => (404, json!({ "kind": "NotFound", "causes": ["Not found"] })),
    }
}

impl MockRest {
    fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        {
            let server = server.clone();
            let requests = requests.clone();
            std::thread::spawn(move || {
                while let Ok(mut request) = server.recv() {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
                    let recorded = Recorded {
                        method: request.method().to_string(),
                        path: request.url().to_string(),
                        authorization: request
                            .headers()
                            .iter()
                            .find(|h| h.field.equiv("Authorization"))
                            .map(|h| h.value.to_string()),
                        body,
                    };
                    requests.lock().unwrap().push(recorded.clone());
                    let (status, value) = route(&recorded);
                    let _ = request.respond(
                        tiny_http::Response::from_data(value.to_string().into_bytes())
                            .with_status_code(status),
                    );
                }
            });
        }
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}")).unwrap(),
            requests,
            server,
        }
    }

    fn client(&self) -> CoreClient {
        CoreClient::new(http::default_client().unwrap(), self.base.clone())
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

#[test]
fn signing_in_asks_for_the_chosen_console_access() {
    let core = MockRest::start();
    let (_, info) = block_on(core.client().sign_in_with_scope(
        "steve",
        &Secret::new("pw"),
        "Lodestone Launcher on Mac",
        None,
        LauncherScope {
            console_read: true,
            console_write: true,
        },
    ))
    .unwrap();
    let create = core
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/v1/user/api_keys")
        .unwrap();
    let body: Value = serde_json::from_str(&create.body).unwrap();
    assert_eq!(
        body["scope"],
        json!({ "type": "launcher", "console_read": true, "console_write": true })
    );
    assert_eq!(
        info.scope,
        Some(launcher_core::cores::live::KeyScope::Launcher {
            console_read: true,
            console_write: true
        })
    );
}

#[test]
fn access_console_and_players_over_rest() {
    let core = MockRest::start();
    let client = core.client();
    let key = Secret::new(KEY);
    assert_eq!(
        block_on(client.access(&key, "inst-1")).unwrap(),
        Some(LauncherAccess {
            view: true,
            start: true,
            stop: false,
            console_read: true,
            console_write: false
        })
    );
    // an older core has no access route
    assert_eq!(block_on(client.access(&key, "inst-9")).unwrap(), None);
    assert_eq!(
        block_on(client.console_buffer(&key, "inst-1")).unwrap(),
        vec![
            ConsoleLine {
                snowflake: 1,
                message: "first".into()
            },
            ConsoleLine {
                snowflake: 3,
                message: "third".into()
            },
        ]
    );
    let players = block_on(client.players(&key, "inst-1")).unwrap();
    assert_eq!(
        players.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["Alex", "steve"]
    );

    let command = chat_command("hello").unwrap();
    block_on(client.send_command(&key, "inst-1", &command)).unwrap();
    let sent = core
        .requests()
        .into_iter()
        .find(|r| r.path == "/api/v1/instance/inst-1/console")
        .unwrap();
    assert_eq!(sent.body, "\"say hello\"");
    assert!(matches!(
        block_on(client.send_command(&key, "inst-2", "say hi")),
        Err(CoreError::Forbidden(message)) if message.contains("console")
    ));
    assert!(matches!(
        block_on(client.console_buffer(&Secret::new("lsk_revoked_x"), "inst-1")),
        Err(CoreError::SignedOut)
    ));
}

#[test]
fn linking_sends_the_minecraft_token_once_to_the_link_route_only() {
    let core = MockRest::start();
    let client = core.client();
    let key = Secret::new(KEY);
    let link = block_on(client.link_minecraft_account(
        &key,
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        &Secret::new(MC_TOKEN),
    ))
    .unwrap();
    assert_eq!(link.minecraft_name, "Steve");
    assert!(link.verified);
    let with_token: Vec<Recorded> = core
        .requests()
        .into_iter()
        .filter(|r| r.body.contains(MC_TOKEN) || r.path.contains(MC_TOKEN))
        .collect();
    assert!(with_token.len() == 1 && with_token[0].path == "/api/v1/user/minecraft_links");
    // a refusal is the core's message, never the token
    let err = block_on(client.link_minecraft_account(
        &key,
        "069a79f4-44e9-4726-a5be-fca90e38aaf5",
        "Steve",
        &Secret::new("expired"),
    ))
    .unwrap_err();
    assert!(err.to_string().contains("refused"));
}

// ---- WebSocket streams ----

/// What the mock does with each connection, in turn.
#[derive(Clone)]
enum Connection {
    /// Accepts, sends these frames, then closes.
    Send(Vec<Value>),
    /// Accepts, sends these frames, then waits for the client to go.
    SendAndWait(Vec<Value>),
    /// Refuses the handshake with this status.
    Refuse(u16),
}

struct MockStreams {
    base: url::Url,
    /// The request paths, with the key replaced by `<key>`.
    paths: Arc<Mutex<Vec<String>>>,
    /// Whether every request carried the right key.
    keys_ok: Arc<Mutex<bool>>,
}

impl MockStreams {
    fn start(script: Vec<Connection>) -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let paths = Arc::new(Mutex::new(Vec::new()));
        let keys_ok = Arc::new(Mutex::new(true));
        {
            let paths = paths.clone();
            let keys_ok = keys_ok.clone();
            std::thread::spawn(move || {
                for (stream, step) in listener.incoming().zip(script) {
                    let Ok(stream) = stream else { return };
                    let paths = paths.clone();
                    let keys_ok = keys_ok.clone();
                    std::thread::spawn(move || {
                        let callback = |request: &Request, response: Response| {
                            let uri = request.uri().to_string();
                            let decoded: String = url::form_urlencoded::parse(uri.as_bytes())
                                .map(|(k, v)| format!("{k}={v}"))
                                .collect::<Vec<_>>()
                                .join("&");
                            if !decoded.contains(KEY) {
                                *keys_ok.lock().unwrap() = false;
                            }
                            paths.lock().unwrap().push(decoded.replace(KEY, "<key>"));
                            match step {
                                Connection::Refuse(status) => {
                                    let mut refusal = ErrorResponse::new(Some(
                                        json!({ "kind": "x", "causes": ["Refused by the mock"] })
                                            .to_string(),
                                    ));
                                    *refusal.status_mut() =
                                        tungstenite::http::StatusCode::from_u16(status).unwrap();
                                    Err(refusal)
                                }
                                _ => Ok(response),
                            }
                        };
                        let Ok(mut socket) = tungstenite::accept_hdr(stream, callback) else {
                            return;
                        };
                        let (frames, wait) = match &step {
                            Connection::Send(frames) => (frames.clone(), false),
                            Connection::SendAndWait(frames) => (frames.clone(), true),
                            Connection::Refuse(_) => return,
                        };
                        for frame in frames {
                            if socket
                                .send(Message::Text(frame.to_string().into()))
                                .is_err()
                            {
                                return;
                            }
                        }
                        if wait {
                            // until the client closes or goes away
                            while socket.read().is_ok() {}
                        } else {
                            let _ = socket.close(None);
                            while socket.read().is_ok() {}
                        }
                    });
                }
            });
        }
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}")).unwrap(),
            paths,
            keys_ok,
        }
    }
}

/// The next message, failing the test after a deadline instead of hanging.
fn next(rx: &mut mpsc::UnboundedReceiver<LiveMessage>) -> LiveMessage {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match rx.try_recv() {
            Ok(message) => return message,
            Err(mpsc::TryRecvError::Closed) => panic!("the stream ended"),
            Err(mpsc::TryRecvError::Empty) => {
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for the stream"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

fn instance_event(instance: &str, inner: Value) -> Value {
    json!({
        "event_inner": { "type": "InstanceEvent", "instance_uuid": instance, "instance_name": "SMP",
                         "instance_event_inner": inner },
        "details": "", "snowflake": "7", "level": "Info", "caused_by": { "type": "System" }
    })
}

#[test]
fn the_event_stream_reports_state_and_players_and_reconnects() {
    let mock = MockStreams::start(vec![
        Connection::Send(vec![
            instance_event(
                "other",
                json!({ "type": "StateTransition", "to": "Stopped" }),
            ),
            instance_event(
                "inst-1",
                json!({ "type": "StateTransition", "to": "Starting" }),
            ),
            // console lines aren't for the event stream
            instance_event(
                "inst-1",
                json!({ "type": "InstanceOutput", "message": "x" }),
            ),
            instance_event(
                "inst-1",
                json!({ "type": "PlayerChange", "player_list": [{ "type": "MinecraftPlayer", "name": "Steve", "uuid": null }],
                        "players_joined": [], "players_left": [] }),
            ),
        ]),
        Connection::SendAndWait(vec![instance_event(
            "inst-1",
            json!({ "type": "StateTransition", "to": "Running" }),
        )]),
    ]);
    let (tx, mut rx) = mpsc::unbounded();
    let stream = LiveStream::open(
        &mock.base,
        Secret::new(KEY),
        "inst-1",
        StreamKind::Events,
        tx,
    );
    assert_eq!(next(&mut rx), LiveMessage::Connected);
    assert_eq!(next(&mut rx), LiveMessage::State(ServerState::Starting));
    assert!(matches!(next(&mut rx), LiveMessage::Players(players) if players[0].name == "Steve"));
    assert!(matches!(next(&mut rx), LiveMessage::Disconnected(_)));
    assert_eq!(next(&mut rx), LiveMessage::Connected);
    assert_eq!(next(&mut rx), LiveMessage::State(ServerState::Running));
    drop(stream);
    assert!(*mock.keys_ok.lock().unwrap());
    let paths = mock.paths.lock().unwrap().clone();
    assert!(paths[0].starts_with("/api/v1/events/all/stream?filter="));
    assert!(paths[0].contains(r#""event_instance_ids":["inst-1"]"#));
}

#[test]
fn the_console_stream_follows_output_and_stops_when_refused() {
    let mock = MockStreams::start(vec![
        Connection::Send(vec![
            instance_event(
                "inst-1",
                json!({ "type": "InstanceOutput", "message": "§aDone (3.2s)!" }),
            ),
            instance_event(
                "inst-1",
                json!({ "type": "StateTransition", "to": "Running" }),
            ),
        ]),
        // the key was revoked meanwhile
        Connection::Refuse(401),
    ]);
    let (tx, mut rx) = mpsc::unbounded();
    let _stream = LiveStream::open(
        &mock.base,
        Secret::new(KEY),
        "inst-1",
        StreamKind::Console,
        tx,
    );
    assert_eq!(next(&mut rx), LiveMessage::Connected);
    assert_eq!(
        next(&mut rx),
        LiveMessage::Output(ConsoleLine {
            snowflake: 7,
            message: "§aDone (3.2s)!".into()
        })
    );
    assert!(matches!(next(&mut rx), LiveMessage::Disconnected(_)));
    assert_eq!(next(&mut rx), LiveMessage::Ended(CoreError::SignedOut));
    // nothing more: the thread has stopped
    let deadline = Instant::now() + Duration::from_secs(30);
    while !rx.is_terminated_or_closed() {
        assert!(Instant::now() < deadline, "the stream kept running");
        std::thread::sleep(Duration::from_millis(5));
    }
    let paths = mock.paths.lock().unwrap().clone();
    assert_eq!(
        paths[0],
        "/api/v1/instance/inst-1/console/stream?token=Bearer <key>"
    );
}

#[test]
fn a_forbidden_stream_says_why() {
    let mock = MockStreams::start(vec![Connection::Refuse(403)]);
    let (tx, mut rx) = mpsc::unbounded();
    let _stream = LiveStream::open(
        &mock.base,
        Secret::new(KEY),
        "inst-1",
        StreamKind::Console,
        tx,
    );
    assert_eq!(
        next(&mut rx),
        LiveMessage::Ended(CoreError::Forbidden("Refused by the mock".into()))
    );
}

trait Closed {
    fn is_terminated_or_closed(&mut self) -> bool;
}

impl Closed for mpsc::UnboundedReceiver<LiveMessage> {
    /// Whether the sender is gone and nothing is left.
    fn is_terminated_or_closed(&mut self) -> bool {
        matches!(self.try_recv(), Err(mpsc::TryRecvError::Closed))
    }
}
