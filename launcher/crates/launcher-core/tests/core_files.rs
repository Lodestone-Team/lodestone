//! The Files tab's transfers (`cores::admin::files`) against a bare HTTP server on a
//! loopback port: an upload is streamed from disk with its length, and a download that
//! fails never harms a file already where it was to be saved.

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        admin::{
            AdminClient,
            files::{ClientFile, FileType},
        },
    },
    http,
};

/// A request as the server read it.
#[derive(Clone, Debug, Default)]
struct Recorded {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// How the server answers a request: the whole reply, written as it is, after which
/// the connection is closed.
type Answer = Box<dyn Fn(&Recorded) -> Vec<u8> + Send + Sync>;

struct Server {
    base: url::Url,
    requests: Arc<Mutex<Vec<Recorded>>>,
    thread: Option<JoinHandle<()>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    port: u16,
}

impl Server {
    fn start(answer: Answer) -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread = {
            let (requests, stop) = (requests.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else { continue };
                    let Some(request) = read_request(&mut stream) else {
                        continue;
                    };
                    let reply = answer(&request);
                    requests.lock().unwrap().push(request);
                    stream.write_all(&reply).ok();
                    stream.shutdown(std::net::Shutdown::Both).ok();
                }
            })
        };
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            requests,
            thread: Some(thread),
            stop,
            port,
        }
    }

    fn admin(&self) -> AdminClient {
        AdminClient::new(
            CoreClient::new(http::default_client().unwrap(), self.base.clone()),
            Secret::new("session-SECRET"),
        )
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        // wakes the accept loop
        TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<Recorded> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    let (method, path) = (words.next()?.to_string(), words.next()?.to_string());
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let mut request = Recorded {
        method,
        path,
        headers,
        body: Vec::new(),
    };
    if let Some(length) = request.header("content-length") {
        let mut body = vec![0u8; length.parse().ok()?];
        reader.read_exact(&mut body).ok()?;
        request.body = body;
    } else if request
        .header("transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"))
    {
        // not expected here: the test checks the length was sent
        request.body = b"<chunked>".to_vec();
    }
    Some(request)
}

fn reply(status: &str, headers: &[(&str, String)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    let mut out = out.into_bytes();
    out.extend_from_slice(body);
    out
}

fn json(body: &str) -> Vec<u8> {
    reply(
        "200 OK",
        &[
            ("Content-Type", "application/json".into()),
            ("Content-Length", body.len().to_string()),
        ],
        body.as_bytes(),
    )
}

fn file(name: &str, file_type: FileType) -> ClientFile {
    ClientFile {
        name: name.into(),
        file_stem: name.into(),
        extension: None,
        path: name.into(),
        size: None,
        creation_time: None,
        modification_time: None,
        file_type,
    }
}

#[test]
fn an_upload_is_streamed_from_disk_with_its_length() {
    let server = Server::start(Box::new(|_| json("null")));
    let dir = tempfile::tempdir().unwrap();
    let world = dir.path().join("world.zip");
    let bytes: Vec<u8> = (0..1_000_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&world, &bytes).unwrap();
    let notes = dir.path().join("notes.txt");
    std::fs::write(&notes, b"hello").unwrap();

    block_on(server.admin().upload_paths("inst-1", ".", &[world, notes])).unwrap();

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "PUT");
    assert!(request.path.starts_with("/api/v1/instance/inst-1/fs/"));
    assert!(request.path.ends_with("/upload"));
    // the core reports the upload's progress against its length
    assert_eq!(
        request.header("content-length"),
        Some(request.body.len().to_string().as_str())
    );
    let boundary = request
        .header("content-type")
        .and_then(|v| v.split("boundary=").nth(1))
        .unwrap()
        .to_string();
    let mut expected = Vec::new();
    for (name, contents) in [("world.zip", &bytes[..]), ("notes.txt", b"hello")] {
        expected.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        expected.extend_from_slice(contents);
        expected.extend_from_slice(b"\r\n");
    }
    expected.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    assert!(request.body == expected, "the body isn't the files' fields");
}

#[test]
fn a_download_that_stops_early_leaves_the_file_there_as_it_was() {
    // the key is handed out, then the file stops halfway
    let server = Server::start(Box::new(|request| {
        if request.path.ends_with("/url") {
            json("\"abc123\"")
        } else {
            reply(
                "200 OK",
                &[("Content-Length", "1000".into())],
                b"only the first bytes",
            )
        }
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("world.zip");
    std::fs::write(&out, b"the zip the user had").unwrap();

    let result = block_on(server.admin().download_to(
        "inst-1",
        &file("world", FileType::Directory),
        &out,
    ));
    assert!(result.is_err());
    assert_eq!(std::fs::read(&out).unwrap(), b"the zip the user had");
    // and no partial file is left beside it
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_download_refused_before_it_starts_leaves_the_file_there_as_it_was() {
    let server = Server::start(Box::new(|_| {
        let body = r#"{"kind":"Internal","causes":["zipping took too long"]}"#;
        reply(
            "500 Internal Server Error",
            &[
                ("Content-Type", "application/json".into()),
                ("Content-Length", body.len().to_string()),
            ],
            body.as_bytes(),
        )
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("server.properties");
    std::fs::write(&out, b"motd=mine").unwrap();

    let result = block_on(server.admin().download_to(
        "inst-1",
        &file("server.properties", FileType::File),
        &out,
    ));
    assert!(result.is_err());
    assert_eq!(std::fs::read(&out).unwrap(), b"motd=mine");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_download_replaces_the_file_once_complete() {
    let server = Server::start(Box::new(|request| {
        if request.path.ends_with("/url") {
            json("\"abc123\"")
        } else {
            reply("200 OK", &[("Content-Length", "9".into())], b"motd=new!")
        }
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("server.properties");
    std::fs::write(&out, b"motd=old").unwrap();

    let saved = block_on(server.admin().download_to(
        "inst-1",
        &file("server.properties", FileType::File),
        &out,
    ))
    .unwrap();
    assert_eq!(saved, 9);
    assert_eq!(std::fs::read(&out).unwrap(), b"motd=new!");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
