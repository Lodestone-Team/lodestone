//! A mock of the Modrinth API and CDN on a loopback port the OS picks. Each test starts
//! its own. Projects and versions are added by the test; the mock answers the endpoints
//! the launcher uses from them, and records every request.
//!
//! Like `files/`, it is a small HTTP/1.1 server on `std::net`, one thread per connection,
//! closed after one response.

#![allow(dead_code)]

use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use launcher_core::content::Modrinth;
use serde_json::{Value, json};
use sha1::Digest as _;
use url::Url;

#[derive(Clone, Debug)]
pub struct FakeProject {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub project_type: String,
    pub client_side: String,
    pub server_side: String,
}

#[derive(Clone, Debug)]
pub struct FakeVersion {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub date: String,
    /// (project_id, version_id, dependency_type)
    pub dependencies: Vec<(Option<String>, Option<String>, String)>,
    pub filename: String,
    pub bytes: Vec<u8>,
    /// Sent instead of the real SHA-512, to test damaged downloads.
    pub wrong_sha512: bool,
    /// Sent instead of the CDN URL.
    pub url_override: Option<String>,
}

impl FakeVersion {
    pub fn new(
        id: &str,
        project_id: &str,
        number: &str,
        loaders: &[&str],
        game_versions: &[&str],
        date: &str,
    ) -> Self {
        Self {
            id: id.into(),
            project_id: project_id.into(),
            version_number: number.into(),
            version_type: "release".into(),
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            date: date.into(),
            dependencies: Vec::new(),
            filename: format!("{project_id}-{number}.jar"),
            bytes: format!("{project_id} {number}").into_bytes(),
            wrong_sha512: false,
            url_override: None,
        }
    }

    pub fn file(mut self, filename: &str, bytes: &[u8]) -> Self {
        self.filename = filename.into();
        self.bytes = bytes.to_vec();
        self
    }

    pub fn kind(mut self, version_type: &str) -> Self {
        self.version_type = version_type.into();
        self
    }

    pub fn requires(mut self, project: Option<&str>, version: Option<&str>) -> Self {
        self.dependencies.push((
            project.map(String::from),
            version.map(String::from),
            "required".into(),
        ));
        self
    }

    pub fn depends(mut self, project: &str, dependency_type: &str) -> Self {
        self.dependencies
            .push((Some(project.into()), None, dependency_type.into()));
        self
    }

    pub fn sha1(&self) -> String {
        format!("{:x}", sha1::Sha1::digest(&self.bytes))
    }

    pub fn sha512(&self) -> String {
        format!("{:x}", sha2::Sha512::digest(&self.bytes))
    }

    pub fn cdn_url(&self) -> String {
        format!(
            "https://cdn.modrinth.com/data/{}/versions/{}/{}",
            self.project_id, self.id, self.filename
        )
    }

    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "project_id": self.project_id,
            "name": format!("{} {}", self.project_id, self.version_number),
            "version_number": self.version_number,
            "version_type": self.version_type,
            "loaders": self.loaders,
            "game_versions": self.game_versions,
            "date_published": self.date,
            "downloads": 1,
            "files": [{
                "hashes": {
                    "sha1": self.sha1(),
                    "sha512": if self.wrong_sha512 { "0".repeat(128) } else { self.sha512() },
                },
                "url": self.url_override.clone().unwrap_or_else(|| self.cdn_url()),
                "filename": self.filename,
                "primary": true,
                "size": self.bytes.len(),
            }],
            "dependencies": self.dependencies.iter().map(|(p, v, t)| json!({
                "project_id": p, "version_id": v, "file_name": null, "dependency_type": t,
            })).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub body: String,
    pub user_agent: Option<String>,
}

#[derive(Default)]
pub struct State {
    pub projects: Vec<FakeProject>,
    pub versions: Vec<FakeVersion>,
    pub requests: Vec<Recorded>,
    /// Answers the next requests with this status and headers instead.
    pub next_status: Option<(u16, Vec<(String, String)>)>,
    /// Headers added to every answer.
    pub headers: Vec<(String, String)>,
    /// Other files, by path (`/<host>/<path>`), such as a modpack's downloads.
    pub files: HashMap<String, Vec<u8>>,
}

pub struct ModrinthMock {
    pub base: Url,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl ModrinthMock {
    pub fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let addr = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = state.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let state = state.clone();
                    std::thread::spawn(move || answer(&state, stream));
                }
            })
        };
        Self {
            base: Url::parse(&format!("http://{addr}/")).unwrap(),
            state,
            stop,
            addr,
            thread: Some(thread),
        }
    }

    /// A client that uses this mock for the API and the CDN.
    pub fn client(&self) -> Modrinth {
        Modrinth::with_urls(
            launcher_core::http::default_client().unwrap(),
            self.base.join("v2").unwrap(),
            Some(self.base.clone()),
        )
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    pub fn project(&self, id: &str, slug: &str, project_type: &str) -> &Self {
        self.state().projects.push(FakeProject {
            id: id.into(),
            slug: slug.into(),
            title: format!("{slug} title"),
            project_type: project_type.into(),
            client_side: "required".into(),
            server_side: "optional".into(),
        });
        self
    }

    pub fn version(&self, version: FakeVersion) -> &Self {
        self.state().versions.push(version);
        self
    }

    /// Serves `bytes` for the URL `url` (on any host), through the mirror.
    pub fn serve(&self, url: &str, bytes: &[u8]) -> &Self {
        let url = Url::parse(url).unwrap();
        let path = format!("/{}{}", url.host_str().unwrap(), url.path());
        self.state().files.insert(path, bytes.to_vec());
        self
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.state().requests.clone()
    }

    pub fn api_requests(&self, method: &str, path: &str) -> Vec<Recorded> {
        self.requests()
            .into_iter()
            .filter(|r| r.method == method && r.path == path)
            .collect()
    }
}

impl Drop for ModrinthMock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn project_json(project: &FakeProject, versions: &[FakeVersion]) -> Value {
    let mine: Vec<&FakeVersion> = versions
        .iter()
        .filter(|v| v.project_id == project.id)
        .collect();
    let mut loaders: Vec<&String> = mine.iter().flat_map(|v| &v.loaders).collect();
    loaders.sort();
    loaders.dedup();
    json!({
        "id": project.id,
        "slug": project.slug,
        "project_type": project.project_type,
        "title": project.title,
        "description": format!("About {}", project.title),
        "body": "# Body",
        "icon_url": format!("https://cdn.modrinth.com/data/{}/icon.png", project.id),
        "categories": [],
        "loaders": loaders,
        "game_versions": [],
        "downloads": 1000,
        "followers": 10,
        "client_side": project.client_side,
        "server_side": project.server_side,
        "gallery": [],
        "published": "2024-01-01T00:00:00Z",
        "updated": "2024-01-01T00:00:00Z",
    })
}

fn list_param(query: &HashMap<String, String>, key: &str) -> Option<Vec<String>> {
    query.get(key).and_then(|v| serde_json::from_str(v).ok())
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<u8>) {
    let ok = |value: Value| (200, value.to_string().into_bytes());
    let not_found = (404, br#"{"error":"not_found"}"#.to_vec());
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    let find_project = |state: &State, id: &str| {
        state
            .projects
            .iter()
            .find(|p| p.id == id || p.slug == id)
            .cloned()
    };
    if request.method == "GET"
        && let Some(bytes) = state.files.get(&request.path)
    {
        return (200, bytes.clone());
    }
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["cdn.modrinth.com", "data", project, "versions", id, _]) => {
            match state
                .versions
                .iter()
                .find(|v| v.id == *id && v.project_id == *project)
            {
                Some(version) => (200, version.bytes.clone()),
                None => not_found,
            }
        }
        ("GET", ["cdn.modrinth.com", "data", _, "icon.png"]) => {
            (200, b"\x89PNG not really".to_vec())
        }
        ("GET", ["v2", "search"]) => ok(json!({
            "hits": state.projects.iter().map(|p| json!({
                "project_id": p.id, "slug": p.slug, "project_type": p.project_type,
                "title": p.title, "description": "", "author": "someone", "icon_url": "",
                "downloads": 5, "follows": 1, "categories": [], "versions": [],
                "date_modified": "", "client_side": p.client_side, "server_side": p.server_side,
            })).collect::<Vec<_>>(),
            "offset": 0, "limit": 20, "total_hits": state.projects.len(),
        })),
        ("GET", ["v2", "project", id]) => match find_project(state, id) {
            Some(project) => ok(project_json(&project, &state.versions)),
            None => not_found,
        },
        ("GET", ["v2", "projects"]) => {
            let ids = list_param(&request.query, "ids").unwrap_or_default();
            ok(Value::Array(
                state
                    .projects
                    .iter()
                    .filter(|p| ids.contains(&p.id))
                    .map(|p| project_json(p, &state.versions))
                    .collect(),
            ))
        }
        ("GET", ["v2", "project", id, "version"]) => {
            let Some(project) = find_project(state, id) else {
                return not_found;
            };
            let loaders = list_param(&request.query, "loaders");
            let game_versions = list_param(&request.query, "game_versions");
            ok(Value::Array(
                state
                    .versions
                    .iter()
                    .filter(|v| v.project_id == project.id)
                    .filter(|v| {
                        loaders
                            .as_ref()
                            .is_none_or(|l| v.loaders.iter().any(|x| l.contains(x)))
                    })
                    .filter(|v| {
                        game_versions
                            .as_ref()
                            .is_none_or(|g| v.game_versions.iter().any(|x| g.contains(x)))
                    })
                    .map(FakeVersion::json)
                    .collect(),
            ))
        }
        ("GET", ["v2", "project", id, "version", version]) => {
            let Some(project) = find_project(state, id) else {
                return not_found;
            };
            match state.versions.iter().find(|v| {
                v.project_id == project.id && (v.id == *version || v.version_number == *version)
            }) {
                Some(v) => ok(v.json()),
                None => not_found,
            }
        }
        ("GET", ["v2", "version", id]) => match state.versions.iter().find(|v| v.id == *id) {
            Some(v) => ok(v.json()),
            None => not_found,
        },
        ("POST", ["v2", "version_files"]) => {
            let body: Value = serde_json::from_str(&request.body).unwrap_or_default();
            assert_eq!(body["algorithm"], "sha1");
            let hashes: Vec<String> =
                serde_json::from_value(body["hashes"].clone()).unwrap_or_default();
            let mut map = serde_json::Map::new();
            for version in &state.versions {
                if hashes.contains(&version.sha1()) {
                    map.insert(version.sha1(), version.json());
                }
            }
            ok(Value::Object(map))
        }
        ("POST", ["v2", "version_files", "update"]) => {
            let body: Value = serde_json::from_str(&request.body).unwrap_or_default();
            let hashes: Vec<String> =
                serde_json::from_value(body["hashes"].clone()).unwrap_or_default();
            let loaders: Vec<String> =
                serde_json::from_value(body["loaders"].clone()).unwrap_or_default();
            let game_versions: Vec<String> =
                serde_json::from_value(body["game_versions"].clone()).unwrap_or_default();
            let mut map = serde_json::Map::new();
            for installed in state.versions.iter().filter(|v| hashes.contains(&v.sha1())) {
                let newest = state
                    .versions
                    .iter()
                    .filter(|v| v.project_id == installed.project_id)
                    .filter(|v| v.loaders.iter().any(|l| loaders.contains(l)))
                    .filter(|v| v.game_versions.iter().any(|g| game_versions.contains(g)))
                    .max_by(|a, b| a.date.cmp(&b.date));
                if let Some(newest) = newest {
                    map.insert(installed.sha1(), newest.json());
                }
            }
            ok(Value::Object(map))
        }
        _ => not_found,
    }
}

fn read_request(stream: &TcpStream) -> Option<(String, String, Option<String>, String)> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut user_agent = None;
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("user-agent") {
                user_agent = Some(value.trim().to_string());
            } else if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some((
        method,
        target,
        user_agent,
        String::from_utf8_lossy(&body).into_owned(),
    ))
}

fn answer(state: &Mutex<State>, mut stream: TcpStream) {
    let Some((method, target, user_agent, body)) = read_request(&stream) else {
        return;
    };
    let url = Url::parse(&format!("http://mock{target}")).unwrap();
    let request = Recorded {
        method,
        path: url.path().to_string(),
        query: url.query_pairs().into_owned().collect(),
        body,
        user_agent,
    };
    let (status, body, headers) = {
        let mut state = state.lock().unwrap();
        state.requests.push(request.clone());
        let mut headers = state.headers.clone();
        let (status, body) = match state.next_status.take() {
            Some((status, extra)) => {
                headers.extend(extra);
                (status, b"{}".to_vec())
            }
            None => route(&mut state, &request),
        };
        (status, body, headers)
    };
    let mut head = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}
