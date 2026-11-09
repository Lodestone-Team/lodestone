//! A mock of the CurseForge API and CDN on a loopback port the OS picks, like
//! `modrinth/`: a small HTTP/1.1 server on `std::net`, one thread per connection, closed
//! after one response. Projects and files are added by the test; every request is
//! recorded with its API key header.

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

use launcher_core::content::{CurseForge, curseforge::fingerprint};
use serde_json::{Value, json};
use sha1::Digest as _;
use url::Url;

pub const KEY: &str = "test-key-not-real";

/// A jar with a `fabric.mod.json` naming mod `id`.
pub fn fabric_jar(id: &str, version: &str) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            json!({ "schemaVersion": 1, "id": id, "version": version, "name": id })
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        zip.finish().unwrap();
    }
    bytes.into_inner()
}

#[derive(Clone, Debug)]
pub struct FakeFile {
    pub id: u64,
    pub mod_id: u64,
    pub file_name: String,
    pub display_name: String,
    pub release_type: u8,
    pub date: String,
    pub game_versions: Vec<String>,
    /// (project, relation type)
    pub dependencies: Vec<(u64, u8)>,
    pub bytes: Vec<u8>,
    /// No `downloadUrl`: the author doesn't allow third-party downloads.
    pub blocked: bool,
    /// Sent instead of the real fingerprint, to test damaged downloads.
    pub wrong_fingerprint: bool,
}

impl FakeFile {
    pub fn new(
        id: u64,
        mod_id: u64,
        name: &str,
        tags: &[&str],
        date: &str,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            id,
            mod_id,
            file_name: name.into(),
            display_name: name.trim_end_matches(".jar").into(),
            release_type: 1,
            date: date.into(),
            game_versions: tags.iter().map(|s| s.to_string()).collect(),
            dependencies: Vec::new(),
            bytes,
            blocked: false,
            wrong_fingerprint: false,
        }
    }

    pub fn requires(mut self, project: u64) -> Self {
        self.dependencies.push((project, 3));
        self
    }

    pub fn blocked(mut self) -> Self {
        self.blocked = true;
        self
    }

    pub fn beta(mut self) -> Self {
        self.release_type = 2;
        self
    }

    pub fn sha1(&self) -> String {
        format!("{:x}", sha1::Sha1::digest(&self.bytes))
    }

    pub fn url(&self) -> String {
        format!(
            "https://edge.forgecdn.net/files/{}/{}/{}",
            self.id / 1000,
            self.id % 1000,
            self.file_name
        )
    }

    fn json(&self) -> Value {
        let print = fingerprint(&self.bytes) as u64;
        json!({
            "id": self.id,
            "modId": self.mod_id,
            "displayName": self.display_name,
            "fileName": self.file_name,
            "releaseType": self.release_type,
            "fileDate": self.date,
            "downloadUrl": if self.blocked { Value::Null } else { json!(self.url()) },
            "fileLength": self.bytes.len(),
            "hashes": [
                { "value": self.sha1(), "algo": 1 },
                { "value": "00000000000000000000000000000000", "algo": 2 }
            ],
            "gameVersions": self.game_versions,
            "dependencies": self.dependencies.iter()
                .map(|(m, r)| json!({ "modId": m, "relationType": r }))
                .collect::<Vec<_>>(),
            "fileFingerprint": if self.wrong_fingerprint { print + 1 } else { print },
            "isAvailable": true,
            "isServerPack": false,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub body: String,
    pub api_key: Option<String>,
}

#[derive(Default)]
pub struct State {
    pub projects: Vec<(u64, String, String, u32)>,
    pub files: Vec<FakeFile>,
    pub requests: Vec<Recorded>,
    /// Answers the next request with this status.
    pub next_status: Option<u16>,
}

pub struct CurseForgeMock {
    pub base: Url,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl CurseForgeMock {
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

    pub fn client(&self) -> CurseForge {
        CurseForge::with_urls(
            launcher_core::http::default_client().unwrap(),
            self.base.join("v1").unwrap(),
            KEY.into(),
            Some(self.base.clone()),
        )
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// A project of class `class` (6 mods, 12 resource packs, 6552 shaders).
    pub fn project(&self, id: u64, slug: &str, class: u32) -> &Self {
        self.state()
            .projects
            .push((id, slug.into(), format!("{slug} title"), class));
        self
    }

    pub fn file(&self, file: FakeFile) -> &Self {
        self.state().files.push(file);
        self
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.state().requests.clone()
    }
}

impl Drop for CurseForgeMock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn project_json(state: &State, id: u64) -> Option<Value> {
    let (id, slug, name, class) = state.projects.iter().find(|p| p.0 == id)?.clone();
    Some(json!({
        "id": id,
        "name": name,
        "slug": slug,
        "summary": format!("About {name}"),
        "links": { "websiteUrl": format!("https://www.curseforge.com/minecraft/mc-mods/{slug}") },
        "logo": { "thumbnailUrl": format!("https://media.forgecdn.net/avatars/{id}.png") },
        "authors": [{ "name": "someone" }],
        "classId": class,
        "downloadCount": 1000.0,
        "dateModified": "2024-01-01T00:00:00Z",
        "allowModDistribution": true,
    }))
}

fn loader_name(kind: &str) -> &'static str {
    match kind {
        "1" => "Forge",
        "4" => "Fabric",
        "5" => "Quilt",
        "6" => "NeoForge",
        _ => "",
    }
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<u8>) {
    let ok = |value: Value| (200, value.to_string().into_bytes());
    let not_found = (404, b"{}".to_vec());
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    let body: Value = serde_json::from_str(&request.body).unwrap_or(Value::Null);
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["edge.forgecdn.net", "files", ..]) => {
            match state.files.iter().find(|f| {
                !f.blocked
                    && Url::parse(&f.url()).unwrap().path()
                        == &request.path["/edge.forgecdn.net".len()..]
            }) {
                Some(file) => (200, file.bytes.clone()),
                None => not_found,
            }
        }
        ("GET", ["v1", "mods", "search"]) => {
            let class: u32 = request
                .query
                .get("classId")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0);
            let text = request
                .query
                .get("searchFilter")
                .cloned()
                .unwrap_or_default()
                .to_lowercase();
            let hits: Vec<Value> = state
                .projects
                .iter()
                .filter(|p| p.3 == class && p.1.contains(&text))
                .filter_map(|p| project_json(state, p.0))
                .collect();
            ok(
                json!({ "data": hits, "pagination": { "index": 0, "pageSize": hits.len(), "resultCount": hits.len(), "totalCount": hits.len() } }),
            )
        }
        ("POST", ["v1", "mods"]) => {
            let ids: Vec<u64> = serde_json::from_value(body["modIds"].clone()).unwrap_or_default();
            ok(
                json!({ "data": ids.iter().filter_map(|id| project_json(state, *id)).collect::<Vec<_>>() }),
            )
        }
        ("GET", ["v1", "mods", id]) => match id.parse().ok().and_then(|id| project_json(state, id))
        {
            Some(project) => ok(json!({ "data": project })),
            None => not_found,
        },
        ("GET", ["v1", "mods", id, "files"]) => {
            let id: u64 = id.parse().unwrap_or(0);
            if !state.projects.iter().any(|p| p.0 == id) {
                return not_found;
            }
            let version = request.query.get("gameVersion");
            let loader = request.query.get("modLoaderType").map(|k| loader_name(k));
            let mut files: Vec<&FakeFile> = state
                .files
                .iter()
                .filter(|f| f.mod_id == id)
                .filter(|f| version.is_none_or(|v| f.game_versions.contains(v)))
                .filter(|f| loader.is_none_or(|l| f.game_versions.iter().any(|t| t == l)))
                .collect();
            files.sort_by(|a, b| b.date.cmp(&a.date));
            let index: usize = request
                .query
                .get("index")
                .and_then(|i| i.parse().ok())
                .unwrap_or(0);
            let size: usize = request
                .query
                .get("pageSize")
                .and_then(|i| i.parse().ok())
                .unwrap_or(50)
                .min(50);
            let page: Vec<Value> = files
                .iter()
                .skip(index)
                .take(size)
                .map(|f| f.json())
                .collect();
            ok(
                json!({ "data": page, "pagination": { "index": index, "pageSize": size, "resultCount": page.len(), "totalCount": files.len() } }),
            )
        }
        ("GET", ["v1", "mods", _, "files", file]) => {
            match state.files.iter().find(|f| file.parse() == Ok(f.id)) {
                Some(file) => ok(json!({ "data": file.json() })),
                None => not_found,
            }
        }
        ("POST", ["v1", "mods", "files"]) => {
            let ids: Vec<u64> = serde_json::from_value(body["fileIds"].clone()).unwrap_or_default();
            ok(
                json!({ "data": state.files.iter().filter(|f| ids.contains(&f.id)).map(|f| f.json()).collect::<Vec<_>>() }),
            )
        }
        ("POST", ["v1", "fingerprints", "432"]) => {
            let prints: Vec<u64> =
                serde_json::from_value(body["fingerprints"].clone()).unwrap_or_default();
            let matches: Vec<Value> = state
                .files
                .iter()
                .filter(|f| prints.contains(&(fingerprint(&f.bytes) as u64)))
                .map(|f| json!({ "id": f.mod_id, "file": f.json(), "latestFiles": [] }))
                .collect();
            ok(
                json!({ "data": { "isCacheBuilt": true, "exactMatches": matches, "exactFingerprints": [] } }),
            )
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
    let mut key = None;
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("x-api-key") {
                key = Some(value.trim().to_string());
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
        key,
        String::from_utf8_lossy(&body).into_owned(),
    ))
}

fn answer(state: &Mutex<State>, mut stream: TcpStream) {
    let Some((method, target, api_key, body)) = read_request(&stream) else {
        return;
    };
    let url = Url::parse(&format!("http://mock{target}")).unwrap();
    let request = Recorded {
        method,
        path: url.path().to_string(),
        query: url.query_pairs().into_owned().collect(),
        body,
        api_key,
    };
    let (status, body) = {
        let mut state = state.lock().unwrap();
        state.requests.push(request.clone());
        match state.next_status.take() {
            Some(status) => (status, b"{}".to_vec()),
            None => route(&mut state, &request),
        }
    };
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}
