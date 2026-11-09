//! The socket server and client against a fake backend.
#![cfg(unix)]

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use launcher_control::{
    ErrorObject, Message,
    client::{Client, ClientError},
    codes, current_uid,
    server::{Backend, BindError, Outbox, Server},
};
use serde_json::{Value, json};

/// Answers a few methods the way the launcher's backend does.
#[derive(Default)]
struct Fake {
    connected: Mutex<Vec<u64>>,
    disconnected: Mutex<Vec<u64>>,
}

impl Backend for Fake {
    fn request(&self, _connection: u64, request: launcher_control::Request, outbox: Outbox) {
        let id = request.id.clone();
        match request.method.as_str() {
            "echo" => {
                outbox.respond(id, Ok(request.params));
            }
            "slow" => {
                // answered later, from another thread, as the GPUI main thread does
                std::thread::spawn(move || {
                    for step in 1..=3 {
                        outbox.send(&Message::Progress {
                            id: id.clone(),
                            progress: json!({ "step": step }),
                        });
                    }
                    outbox.respond(id, Ok(json!({ "done": true })));
                });
            }
            "subscribe" => {
                outbox.respond(id, Ok(json!({ "subscription": 9 })));
                for n in 0..2 {
                    outbox.send(&Message::Event {
                        subscription: 9,
                        event: "log".into(),
                        data: json!({ "n": n }),
                    });
                }
            }
            "delete" if request.params.get("confirm") != Some(&json!(true)) => {
                outbox.respond(
                    id,
                    Err(ErrorObject::new(
                        codes::CONFIRMATION_REQUIRED,
                        "Pass confirm: true.",
                    )),
                );
            }
            method => {
                outbox.respond(
                    id,
                    Err(ErrorObject::new(
                        codes::METHOD_NOT_FOUND,
                        format!("No method {method}."),
                    )),
                );
            }
        }
    }

    fn connected(&self, connection: u64) {
        self.connected.lock().unwrap().push(connection);
    }

    fn disconnected(&self, connection: u64) {
        self.disconnected.lock().unwrap().push(connection);
    }
}

fn serve() -> (tempfile::TempDir, std::path::PathBuf, Arc<Fake>, impl Drop) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    let fake = Arc::new(Fake::default());
    let handle = Server::bind(&path).unwrap().spawn(fake.clone()).unwrap();
    (dir, path, fake, handle)
}

#[test]
fn requests_get_their_responses() {
    let (_dir, path, _fake, _handle) = serve();
    let mut client = Client::connect(&path).unwrap();
    let result = client.call("echo", json!({ "a": 1 })).unwrap();
    assert_eq!(result, Ok(json!({ "a": 1 })));

    let mut progress = Vec::new();
    let result = client
        .call_with(
            "slow",
            json!({}),
            Some(Duration::from_secs(5)),
            |p| progress.push(p.clone()),
            |_| {},
        )
        .unwrap();
    assert_eq!(result, Ok(json!({ "done": true })));
    assert_eq!(
        progress,
        [
            json!({ "step": 1 }),
            json!({ "step": 2 }),
            json!({ "step": 3 })
        ]
    );

    let error = client.call("nope", json!({})).unwrap().unwrap_err();
    assert_eq!(error.code, codes::METHOD_NOT_FOUND);
    assert_eq!(error.exit_code(), 2);
    let error = client.call("delete", json!({})).unwrap().unwrap_err();
    assert_eq!(error.code, codes::CONFIRMATION_REQUIRED);
    assert_eq!(
        client.call("delete", json!({ "confirm": true })).unwrap(),
        Err(ErrorObject::new(
            codes::METHOD_NOT_FOUND,
            "No method delete."
        ))
    );
}

#[test]
fn subscriptions_stream_events() {
    let (_dir, path, _fake, _handle) = serve();
    let mut client = Client::connect(&path).unwrap();
    let mut events = Vec::new();
    let result = client
        .call_with("subscribe", json!({}), None, |_| {}, |m| events.push(m))
        .unwrap();
    assert_eq!(result, Ok(json!({ "subscription": 9 })));
    for _ in 0..2 {
        events.push(client.next_message(Some(Duration::from_secs(5))).unwrap());
    }
    let ns: Vec<Value> = events
        .into_iter()
        .map(|m| match m {
            Message::Event {
                subscription: 9,
                event,
                data,
            } if event == "log" => data["n"].clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(ns, [json!(0), json!(1)]);
    // nothing more comes
    assert!(matches!(
        client.next_message(Some(Duration::from_millis(100))),
        Err(ClientError::Timeout)
    ));
}

#[test]
fn bad_lines_are_answered_with_codes() {
    let (_dir, path, fake, _handle) = serve();
    let stream = UnixStream::connect(&path).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut ask = |line: &str| {
        writer.write_all(line.as_bytes()).unwrap();
        writer.write_all(b"\n").unwrap();
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        Message::parse(&answer).unwrap()
    };
    let error = |message: Message| match message {
        Message::Response {
            id,
            result: Err(error),
        } => (id, error.code),
        other => panic!("not an error: {other:?}"),
    };
    assert_eq!(
        error(ask("{oops")),
        (Value::Null, codes::PARSE_ERROR.into())
    );
    assert_eq!(
        error(ask(r#"{"v":2,"id":5,"method":"echo"}"#)),
        (json!(5), codes::PROTOCOL_VERSION.into())
    );
    assert_eq!(
        error(ask(r#"{"v":1,"id":6}"#)),
        (json!(6), codes::INVALID_REQUEST.into())
    );
    // the connection still works after bad lines
    assert_eq!(
        ask(r#"{"v":1,"id":7,"method":"echo","params":{"x":true}}"#),
        Message::result(json!(7), Ok(json!({ "x": true })))
    );
    drop(ask);
    drop(reader);
    drop(writer);
    // the backend hears that the client went away
    for _ in 0..50 {
        if !fake.disconnected.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fake.disconnected.lock().unwrap().len(), 1);
}

#[test]
fn the_socket_is_private() {
    let (_dir, path, _fake, _handle) = serve();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn other_users_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    // pretend the launcher runs as someone else: this test's user is then the stranger
    let _handle = Server::bind(&path)
        .unwrap()
        .with_allowed_uid(current_uid().wrapping_add(1))
        .spawn(Arc::new(Fake::default()))
        .unwrap();
    let mut client = Client::connect(&path).unwrap();
    let result = client.call("echo", json!({})).unwrap();
    assert_eq!(result.unwrap_err().code, codes::PERMISSION_DENIED);
}

#[test]
fn a_stale_socket_is_replaced_but_a_live_one_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    {
        // a launcher that quit without cleaning up leaves its socket file
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(listener);
    }
    assert!(path.exists());
    let handle = Server::bind(&path)
        .unwrap()
        .spawn(Arc::new(Fake::default()))
        .unwrap();
    assert!(matches!(Server::bind(&path), Err(BindError::InUse(_))));
    // and a file that isn't a socket is never removed
    let other = dir.path().join("not-a-socket");
    std::fs::write(&other, "keep me").unwrap();
    assert!(matches!(Server::bind(&other), Err(BindError::Io(_))));
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "keep me");
    drop(handle);
    assert!(!path.exists(), "shutting down removes the socket");
}

#[test]
fn a_missing_launcher_is_not_running() {
    let dir = tempfile::tempdir().unwrap();
    let err = match Client::connect(&dir.path().join("control.sock")) {
        Err(err) => err,
        Ok(_) => panic!("connected to nothing"),
    };
    let error = err.to_error();
    assert_eq!(error.code, codes::NOT_RUNNING);
    assert_eq!(error.exit_code(), 3);
}

#[test]
fn slow_readers_lose_events_not_responses() {
    let (tx, rx) = mpsc::channel();
    let outbox = Outbox::for_test(tx);
    for n in 0..launcher_control::server::MAX_QUEUED_EVENTS + 5 {
        outbox.send(&Message::Event {
            subscription: 1,
            event: "log".into(),
            data: json!(n),
        });
    }
    assert!(outbox.respond(json!(1), Ok(json!(null))));
    assert_eq!(outbox.take_dropped(), 5);
    assert_eq!(
        rx.try_iter().count(),
        launcher_control::server::MAX_QUEUED_EVENTS + 1
    );
}

fn wait_until(mut done: impl FnMut() -> bool) {
    let started = std::time::Instant::now();
    while !done() {
        assert!(started.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_server_can_drop_a_client_or_all_of_them() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.sock");
    let fake = Arc::new(Fake::default());
    let mut handle = Server::bind(&path).unwrap().spawn(fake.clone()).unwrap();

    let mut first = Client::connect(&path).unwrap();
    assert_eq!(
        first.call("echo", json!({ "n": 1 })).unwrap(),
        Ok(json!({ "n": 1 }))
    );
    wait_until(|| fake.connected.lock().unwrap().len() == 1);
    let id = fake.connected.lock().unwrap()[0];
    let mut second = Client::connect(&path).unwrap();
    assert_eq!(
        second.call("echo", json!({ "n": 2 })).unwrap(),
        Ok(json!({ "n": 2 }))
    );
    wait_until(|| fake.connected.lock().unwrap().len() == 2);
    assert_eq!(handle.connections(), 2);

    assert!(handle.disconnect(id));
    assert!(!handle.disconnect(id));
    wait_until(|| fake.disconnected.lock().unwrap().contains(&id));
    assert!(first.call("echo", json!({ "n": 3 })).is_err());
    assert_eq!(
        second.call("echo", json!({ "n": 4 })).unwrap(),
        Ok(json!({ "n": 4 }))
    );

    handle.shutdown();
    assert!(!path.exists());
    wait_until(|| fake.disconnected.lock().unwrap().len() == 2);
    assert!(second.call("echo", json!({ "n": 5 })).is_err());
    assert!(UnixStream::connect(&path).is_err());
}
