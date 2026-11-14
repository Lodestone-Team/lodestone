//! The CLI binary against a fake launcher on a real socket: its JSON output, exit
//! codes, confirmations, progress and streams.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
};

use launcher_control::{
    ErrorObject, Message, Request, codes,
    server::{Backend, Outbox, Server, ServerHandle},
};
use serde_json::{Value, json};

/// Answers like the launcher, and remembers what it was asked.
#[derive(Default)]
struct Fake {
    requests: Mutex<Vec<Request>>,
    hellos: Mutex<Vec<Value>>,
}

impl Backend for Fake {
    fn request(&self, _connection: u64, request: Request, outbox: Outbox) {
        if request.method == "meta.hello" {
            self.hellos.lock().unwrap().push(request.params.clone());
            outbox.respond(request.id, Ok(json!({ "access": { "level": "full" } })));
            return;
        }
        self.requests.lock().unwrap().push(request.clone());
        let id = request.id.clone();
        let p = &request.params;
        let result = match request.method.as_str() {
            "meta.ping" => Ok(json!({ "app": "Lodestone Launcher", "protocol": 1 })),
            "instances.list" => Ok(
                json!({ "instances": [{ "id": "a1", "name": "Survival", "effective_group": "Fabric (modded)" }] }),
            ),
            "instances.delete" if p["confirm"] != true => Err(ErrorObject::new(
                codes::CONFIRMATION_REQUIRED,
                "This moves the instance \"Survival\" to the Trash. Ask the user first, then pass --yes.",
            )),
            "instances.delete" => Ok(json!({ "deleted": "a1", "trash": true })),
            "instances.show" => Err(ErrorObject::new(
                codes::NOT_FOUND,
                "No instance is called \"x\".",
            )),
            "content.install" if p["wait"] == true => {
                for done in 1..=2 {
                    outbox.send(&Message::Progress {
                        id: id.clone(),
                        progress: json!({ "id": 1, "state": "running", "progress": { "label": "Downloading", "files_done": done, "files_total": 2 } }),
                    });
                }
                Ok(json!({ "installed": [{ "title": "Sodium", "file": "sodium.jar" }], "job": 1 }))
            }
            "content.install" => Ok(json!({ "job": { "id": 1, "state": "running" } })),
            "play.logs" => Ok(
                json!({ "instance": "a1", "state": "running", "rows": [{ "instance": "a1", "text": "old line" }] }),
            ),
            "events.subscribe" if p.get("instance").is_none() => {
                // `events`: a notice every few milliseconds, until the client goes away
                outbox.respond(id, Ok(json!({ "subscription": 5 })));
                std::thread::spawn(move || {
                    for n in 0..2000 {
                        let event = Message::Event {
                            subscription: 5,
                            event: "notice".into(),
                            data: json!({ "seq": n }),
                        };
                        if !outbox.send(&event) {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                });
                return;
            }
            "events.subscribe" => {
                outbox.respond(id, Ok(json!({ "subscription": 4 })));
                for text in ["Loading", "Sound engine started", "after"] {
                    outbox.send(&Message::Event {
                        subscription: 4,
                        event: "log".into(),
                        data: json!({ "instance": "a1", "text": text }),
                    });
                }
                return;
            }
            "cores.add" => {
                Ok(json!({ "core": { "id": "c1" }, "got_password": p["password"] == "s3cret" }))
            }
            method => Err(ErrorObject::new(
                codes::METHOD_NOT_FOUND,
                format!("No method {method}."),
            )),
        };
        outbox.respond(id, result);
    }
}

struct Launcher {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    fake: Arc<Fake>,
    _handle: ServerHandle,
}

fn launcher() -> Launcher {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("control.sock");
    let fake = Arc::new(Fake::default());
    let handle = Server::bind(&socket).unwrap().spawn(fake.clone()).unwrap();
    Launcher {
        _dir: dir,
        socket,
        fake,
        _handle: handle,
    }
}

fn cli(socket: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lodestone-launcher-cli"));
    command
        .arg("--socket")
        .arg(socket)
        .args(args)
        .env_remove("LODESTONE_CORE_PASSWORD")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = command.spawn().unwrap();
    if let Some(text) = stdin {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "stdout isn't one JSON document ({err}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn stderr_error(output: &Output) -> ErrorObject {
    let line = String::from_utf8_lossy(&output.stderr);
    let value: Value = serde_json::from_str(line.lines().last().unwrap_or_default())
        .unwrap_or_else(|_| panic!("stderr isn't a JSON error: {line}"));
    serde_json::from_value(value["error"].clone()).unwrap()
}

#[test]
fn prints_json_when_piped() {
    let launcher = launcher();
    let output = cli(&launcher.socket, &["instances", "list"], None);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout_json(&output)["instances"][0]["effective_group"],
        "Fabric (modded)"
    );
    let output = cli(&launcher.socket, &["--human", "instances", "list"], None);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("name: Survival"), "{text}");
    // each run says which program it is
    let hellos = launcher.fake.hellos.lock().unwrap();
    assert_eq!(hellos.len(), 2);
    let client = hellos[0]["client"].as_str().unwrap();
    assert!(client.starts_with("lodestone-launcher-cli "), "{client}");
    assert!(hellos[0]["pid"].as_u64().is_some());
}

#[test]
fn destructive_commands_need_yes() {
    let launcher = launcher();
    let output = cli(&launcher.socket, &["instances", "delete", "Survival"], None);
    assert_eq!(output.status.code(), Some(5));
    assert_eq!(stderr_error(&output).code, codes::CONFIRMATION_REQUIRED);
    assert!(output.stdout.is_empty());

    let output = cli(
        &launcher.socket,
        &["instances", "delete", "Survival", "--yes"],
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_json(&output)["trash"], true);
    let requests = launcher.fake.requests.lock().unwrap();
    assert_eq!(requests.last().unwrap().params["confirm"], true);
}

#[test]
fn errors_have_codes_and_exit_codes() {
    let launcher = launcher();
    let output = cli(&launcher.socket, &["instances", "show", "x"], None);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(stderr_error(&output).code, codes::NOT_FOUND);

    let dir = tempfile::tempdir().unwrap();
    let output = cli(&dir.path().join("nothing.sock"), &["status"], None);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stderr_error(&output).code, codes::NOT_RUNNING);

    let output = cli(&launcher.socket, &["instances", "frobnicate"], None);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_error(&output).code, codes::USAGE);
}

#[test]
fn long_commands_stream_progress_then_one_result() {
    let launcher = launcher();
    let output = cli(
        &launcher.socket,
        &["content", "install", "Survival", "sodium"],
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_json(&output)["installed"][0]["title"], "Sodium");
    let progress: Vec<Value> = String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect();
    assert_eq!(progress.len(), 2);
    assert_eq!(progress[1]["progress"]["progress"]["files_done"], 2);

    let output = cli(
        &launcher.socket,
        &["content", "install", "Survival", "sodium", "--no-wait"],
        None,
    );
    assert_eq!(stdout_json(&output)["job"]["state"], "running");
    let requests = launcher.fake.requests.lock().unwrap();
    assert_eq!(requests[0].params["wait"], true);
    assert_eq!(requests[1].params["wait"], false);
}

#[test]
fn follow_stops_at_the_line_asked_for() {
    let launcher = launcher();
    let output = cli(
        &launcher.socket,
        &[
            "play",
            "logs",
            "Survival",
            "--follow",
            "--until",
            "Sound engine",
            "--timeout",
            "10",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    let texts: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["text"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(texts, ["old line", "Loading", "Sound engine started"]);
}

/// LS-68: `events | head -n 1` never ended, because writes to the closed pipe failed
/// without anything noticing.
#[test]
fn streams_end_when_stdout_closes() {
    use std::io::BufRead as _;

    let launcher = launcher();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lodestone-launcher-cli"))
        .arg("--socket")
        .arg(&launcher.socket)
        .arg("events")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // read one line, like `head -n 1`, then close the pipe
    let mut first = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&first).unwrap()["event"],
        "notice"
    );
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            panic!("the stream kept going after its stdout closed");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
}

#[test]
fn passwords_are_read_from_stdin() {
    let launcher = launcher();
    let output = cli(
        &launcher.socket,
        &[
            "cores",
            "add",
            "10.0.0.5",
            "--username",
            "alice",
            "--password-stdin",
        ],
        Some("s3cret\n"),
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_json(&output)["got_password"], true);
    // and never echoed
    assert!(!String::from_utf8_lossy(&output.stdout).contains("s3cret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("s3cret"));
}

#[test]
fn schema_needs_no_launcher() {
    let dir = tempfile::tempdir().unwrap();
    let output = cli(&dir.path().join("nothing.sock"), &["schema"], None);
    assert_eq!(output.status.code(), Some(0));
    let schema = stdout_json(&output);
    assert!(schema["commands"].as_array().unwrap().len() > 80);
    assert!(
        schema["exit_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == 5)
    );
}
