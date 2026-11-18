//! CurseForge for a server's mods (`cores::admin::curseforge`), as Lodestone Host uses
//! it: against the CurseForge mock, and a bare HTTP server standing in for the core,
//! which keeps the jars it is sent. The core never talks to CurseForge: the files are
//! downloaded here and uploaded to it. And a CurseForge modpack turned into the pack a
//! core makes a server from (`cores::admin::curseforge_pack`).

mod curseforge;
mod modrinth;

use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use curseforge::{CurseForgeMock, FakeFile, KEY, fabric_jar};
use futures::executor::block_on;
use launcher_core::{
    Secret,
    content::curseforge::{CLASS_BUKKIT_PLUGINS, CLASS_MODS, CfFile, fingerprint},
    cores::{
        CoreClient,
        admin::{
            AdminClient,
            curseforge::{
                self as server, CfInstalled, SearchFilter, ServerCfError, ServerInstallPlan,
                ServerTarget,
            },
            curseforge_pack::{self, PackStep},
            models::CurseForgeOrigin,
            mods::{
                InstalledMod, InstalledModKind, InstalledModLoader, ModrinthMatch,
                ModrinthSearchIndex,
            },
        },
    },
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
};
use serde_json::json;

const CREATE: u64 = 328_085;
const FABRIC_API: u64 = 306_612;
const LITHIUM: u64 = 360_438;
const ESSENTIALS: u64 = 93_271;

const NEW: &str = "2025-02-01T00:00:00Z";
const OLD: &str = "2024-02-01T00:00:00Z";
const UUID: &str = "server-1";

/// A core that only has a mods folder: uploads land in it, deletes take from it, and
/// disabling renames. Every request is recorded as "METHOD path".
struct FakeCore {
    base: url::Url,
    jars: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    port: u16,
    thread: Option<JoinHandle<()>>,
}

fn installed(name: &str, bytes: &[u8]) -> InstalledMod {
    serde_json::from_value(json!({
        "file_name": name,
        "enabled": !name.ends_with(".disabled"),
        "size": bytes.len(),
        "sha512": "",
        "curseforge_fingerprint": fingerprint(bytes),
    }))
    .unwrap()
}

impl FakeCore {
    fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let jars: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread = {
            let (jars, requests, stop) = (jars.clone(), requests.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else { continue };
                    let Some((method, path, content_type, body)) = read_request(&mut stream) else {
                        continue;
                    };
                    requests.lock().unwrap().push(format!("{method} {path}"));
                    let reply = answer(&jars, &method, &path, &content_type, &body);
                    stream.write_all(&reply).ok();
                    stream.shutdown(std::net::Shutdown::Both).ok();
                }
            })
        };
        Self {
            base: url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            jars,
            requests,
            stop,
            port,
            thread: Some(thread),
        }
    }

    fn admin(&self) -> AdminClient {
        AdminClient::new(
            CoreClient::new(http::default_client().unwrap(), self.base.clone()),
            Secret::new("session-SECRET"),
        )
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        self.jars
            .lock()
            .unwrap()
            .insert(name.into(), bytes.to_vec());
    }

    /// The server's list, as the core would send it.
    fn mods(&self) -> Vec<InstalledMod> {
        let mut mods: Vec<InstalledMod> = self
            .jars
            .lock()
            .unwrap()
            .iter()
            .map(|(name, bytes)| installed(name, bytes))
            .collect();
        mods.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        mods
    }

    fn names(&self) -> Vec<String> {
        self.mods().into_iter().map(|m| m.file_name).collect()
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeCore {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String, String, Vec<u8>)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    let (method, path) = (words.next()?.to_string(), words.next()?.to_string());
    let (mut length, mut content_type) = (0usize, String::new());
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().ok()?;
        } else if name.eq_ignore_ascii_case("content-type") {
            content_type = value.trim().to_string();
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some((method, path, content_type, body))
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

/// The files of a multipart body: (file name, bytes).
fn multipart(content_type: &str, body: &[u8]) -> Vec<(String, Vec<u8>)> {
    let boundary = content_type.split("boundary=").nth(1).unwrap_or_default();
    let mark = format!("--{boundary}").into_bytes();
    let mut files = Vec::new();
    let mut at = 0;
    while let Some(start) = find(body, &mark, at) {
        let head = start + mark.len();
        let Some(blank) = find(body, b"\r\n\r\n", head) else {
            break;
        };
        let header = String::from_utf8_lossy(&body[head..blank]).into_owned();
        let Some(end) = find(body, &mark, blank) else {
            break;
        };
        if let Some(name) = header.split("filename=\"").nth(1) {
            let name = name.split('"').next().unwrap_or_default().to_string();
            // the part ends with a line break before the next boundary
            files.push((name, body[blank + 4..end - 2].to_vec()));
        }
        at = end;
    }
    files
}

fn reply(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn answer(
    jars: &Mutex<HashMap<String, Vec<u8>>>,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Vec<u8> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let prefix = format!("/api/v1/instance/{UUID}/mods");
    let Some(rest) = path.strip_prefix(&prefix) else {
        return reply("404 Not Found", "{}");
    };
    let mut jars = jars.lock().unwrap();
    let segments: Vec<String> = rest
        .trim_start_matches('/')
        .split('/')
        .map(|s| {
            url::form_urlencoded::parse(format!("x={s}").as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        })
        .collect();
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    match (method, segments.as_slice()) {
        ("POST", ["upload"]) => {
            let files = multipart(content_type, body);
            let overwrite = query.contains("overwrite=true");
            if !overwrite && files.iter().any(|(name, _)| jars.contains_key(name)) {
                return reply(
                    "409 Conflict",
                    r#"{"kind":"Conflict","causes":["These files already exist"]}"#,
                );
            }
            let list: Vec<serde_json::Value> = files
                .iter()
                .map(|(name, bytes)| {
                    json!({ "file_name": name, "enabled": true, "size": bytes.len(),
                        "curseforge_fingerprint": fingerprint(bytes) })
                })
                .collect();
            jars.extend(files);
            reply("200 OK", &json!(list).to_string())
        }
        ("DELETE", [name]) => match jars.remove(*name) {
            Some(_) => reply("200 OK", "null"),
            None => reply("404 Not Found", "{}"),
        },
        ("PUT", [name, "disable"]) => match jars.remove(*name) {
            Some(bytes) => {
                let disabled = format!("{name}.disabled");
                let body = json!({ "file_name": disabled, "enabled": false, "size": bytes.len() });
                jars.insert(disabled, bytes);
                reply("200 OK", &body.to_string())
            }
            None => reply("404 Not Found", "{}"),
        },
        _ => reply("404 Not Found", "{}"),
    }
}

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1)
}

fn fabric() -> ServerTarget {
    ServerTarget {
        kind: InstalledModKind::Mod,
        loader: InstalledModLoader::Fabric,
        game_version: "1.20.1".into(),
    }
}

const FABRIC: [&str; 3] = ["1.20.1", "Fabric", "Server"];

/// Create (which requires Fabric API), Fabric API and Lithium, each with an old and a
/// new file for Fabric 1.20.1.
fn projects(mock: &CurseForgeMock) {
    mock.project(CREATE, "create-fabric", CLASS_MODS)
        .project(FABRIC_API, "fabric-api", CLASS_MODS)
        .project(LITHIUM, "lithium", CLASS_MODS);
    mock.file(
        FakeFile::new(
            5001,
            CREATE,
            "create-0.5.1.jar",
            &FABRIC,
            NEW,
            fabric_jar("create", "0.5.1"),
        )
        .requires(FABRIC_API),
    )
    .file(FakeFile::new(
        4001,
        FABRIC_API,
        "fabric-api-0.90.jar",
        &FABRIC,
        OLD,
        fabric_jar("fabric-api", "0.90"),
    ))
    .file(FakeFile::new(
        4002,
        FABRIC_API,
        "fabric-api-0.92.jar",
        &FABRIC,
        NEW,
        fabric_jar("fabric-api", "0.92"),
    ))
    .file(FakeFile::new(
        6001,
        LITHIUM,
        "lithium-0.11.jar",
        &FABRIC,
        OLD,
        fabric_jar("lithium", "0.11"),
    ))
    .file(
        FakeFile::new(
            6002,
            LITHIUM,
            "lithium-0.12-beta.jar",
            &FABRIC,
            NEW,
            fabric_jar("lithium", "0.12-beta"),
        )
        .beta(),
    );
}

fn file_of(mock: &CurseForgeMock, id: u64) -> CfFile {
    let project = mock
        .state()
        .files
        .iter()
        .find(|f| f.id == id)
        .unwrap()
        .mod_id;
    block_on(mock.client().file(project, id)).unwrap()
}

fn bytes_of(mock: &CurseForgeMock, id: u64) -> Vec<u8> {
    mock.state()
        .files
        .iter()
        .find(|f| f.id == id)
        .unwrap()
        .bytes
        .clone()
}

fn identify(mock: &CurseForgeMock, core: &FakeCore) -> HashMap<String, CfInstalled> {
    block_on(server::identify(&mock.client(), &core.mods())).unwrap()
}

fn plan(
    mock: &CurseForgeMock,
    core: &FakeCore,
    file: u64,
    dependencies: bool,
) -> Result<ServerInstallPlan, ServerCfError> {
    let known = identify(mock, core);
    block_on(server::plan_install(
        &mock.client(),
        &fabric(),
        &known,
        file_of(mock, file),
        None,
        dependencies,
        false,
    ))
}

fn run(
    mock: &CurseForgeMock,
    core: &FakeCore,
    plan: ServerInstallPlan,
) -> Result<server::ServerInstallOutcome, ServerCfError> {
    let staging = tempfile::tempdir().unwrap();
    block_on(http::in_http_runtime(server::install(
        &mock.client(),
        &downloader(),
        &core.admin(),
        UUID,
        &core.mods(),
        plan,
        staging.path(),
        &CancelToken::new(),
    )))
}

#[test]
fn installed_jars_are_recognised_by_the_fingerprint_the_core_reports() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let core = FakeCore::start();
    core.put("fabric-api-0.90.jar", &bytes_of(&mock, 4001));
    core.put("my-own.jar", &fabric_jar("mine", "1"));

    let known = identify(&mock, &core);
    assert_eq!(known.len(), 1);
    let api = &known["fabric-api-0.90.jar"];
    assert_eq!((api.project_id, api.file_id), (FABRIC_API, 4001));
    assert_eq!(api.title, "fabric-api title");
    assert_eq!(api.version, "fabric-api-0.90");
    assert_eq!(
        api.page_url,
        "https://www.curseforge.com/minecraft/mc-mods/fabric-api"
    );
    assert!(
        api.icon_url
            .as_deref()
            .unwrap()
            .contains("media.forgecdn.net")
    );
    assert_eq!(api.update, None);

    // the same fingerprint on a file of another size isn't that file
    let mut other = core.mods();
    other.iter_mut().for_each(|m| m.size += 1);
    assert!(
        block_on(server::identify(&mock.client(), &other))
            .unwrap()
            .is_empty()
    );
    // a core too old to report fingerprints: nothing is asked
    let before = mock.requests().len();
    let mut old = core.mods();
    old.iter_mut().for_each(|m| m.curseforge_fingerprint = None);
    assert!(
        block_on(server::identify(&mock.client(), &old))
            .unwrap()
            .is_empty()
    );
    assert_eq!(mock.requests().len(), before);
    // the key goes to the API
    assert!(
        mock.requests()
            .iter()
            .all(|r| r.api_key.as_deref() == Some(KEY))
    );
}

#[test]
fn updates_are_the_newest_fitting_file_at_least_as_stable() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let core = FakeCore::start();
    core.put("fabric-api-0.90.jar", &bytes_of(&mock, 4001));
    core.put("lithium-0.11.jar", &bytes_of(&mock, 6001));
    let mods = core.mods();
    let mut known = identify(&mock, &core);

    block_on(server::find_updates(
        &mock.client(),
        &fabric(),
        &mods,
        &mut known,
    ))
    .unwrap();
    assert_eq!(
        known["fabric-api-0.90.jar"].update.as_ref().map(|f| f.id),
        Some(4002)
    );
    // a release isn't updated to a beta
    assert_eq!(known["lithium-0.11.jar"].update, None);

    // Modrinth knows this jar too: the core's own check covers it
    let mut on_modrinth = mods.clone();
    on_modrinth
        .iter_mut()
        .find(|m| m.file_name == "fabric-api-0.90.jar")
        .unwrap()
        .modrinth = Some(
        serde_json::from_value::<ModrinthMatch>(
            json!({ "project_id": "P7dR8mSH", "version_id": "v" }),
        )
        .unwrap(),
    );
    let before = mock.requests().len();
    block_on(server::find_updates(
        &mock.client(),
        &fabric(),
        &on_modrinth,
        &mut known,
    ))
    .unwrap();
    assert_eq!(known["fabric-api-0.90.jar"].update, None);
    // only Lithium's files were asked for
    assert_eq!(mock.requests().len(), before + 1);
}

#[test]
fn an_install_downloads_here_and_uploads_to_the_core_with_dependencies() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let core = FakeCore::start();

    let plan = plan(&mock, &core, 5001, true).unwrap();
    let names: Vec<&str> = plan
        .files
        .iter()
        .map(|p| p.file.file_name.as_str())
        .collect();
    assert_eq!(names, ["create-0.5.1.jar", "fabric-api-0.92.jar"]);
    assert_eq!(plan.files[0].title(), "create-fabric title");
    assert_eq!(plan.files[1].depth, 1);
    assert!(plan.blocked.is_empty() && plan.skipped.is_empty());

    let outcome = run(&mock, &core, plan).unwrap();
    assert_eq!(core.names(), ["create-0.5.1.jar", "fabric-api-0.92.jar"]);
    assert_eq!(outcome.result.installed.len(), 2);
    assert!(outcome.result.removed.is_empty());
    // the very bytes CurseForge has, sent in one upload
    assert_eq!(
        core.jars.lock().unwrap()["create-0.5.1.jar"],
        bytes_of(&mock, 5001)
    );
    assert_eq!(
        core.requests(),
        [format!("POST /api/v1/instance/{UUID}/mods/upload")]
    );
    // no key went to the CDN
    assert!(
        mock.requests()
            .iter()
            .filter(|r| r.path.starts_with("/edge.forgecdn.net"))
            .all(|r| r.api_key.is_none())
    );

    // installed already: Create alone, and nothing of Fabric API
    let again = self::plan(&mock, &core, 5001, true).unwrap();
    assert_eq!(again.files.len(), 1);
    assert!(again.files[0].installed);
    let before = core.requests().len();
    let outcome = run(&mock, &core, again).unwrap();
    assert!(outcome.result.installed.is_empty());
    assert_eq!(core.requests().len(), before);

    // without dependencies
    let core = FakeCore::start();
    let alone = self::plan(&mock, &core, 5001, false).unwrap();
    assert_eq!(alone.files.len(), 1);
}

#[test]
fn an_update_replaces_the_older_file_and_stays_disabled() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let core = FakeCore::start();
    core.put("fabric-api-0.90.jar.disabled", &bytes_of(&mock, 4001));

    let plan = plan(&mock, &core, 4002, true).unwrap();
    assert_eq!(plan.files[0].replaces, ["fabric-api-0.90.jar.disabled"]);
    let outcome = run(&mock, &core, plan).unwrap();
    assert_eq!(core.names(), ["fabric-api-0.92.jar.disabled"]);
    assert_eq!(outcome.result.removed, ["fabric-api-0.90.jar.disabled"]);
    assert_eq!(outcome.result.installed.len(), 1);
    assert!(!outcome.result.installed[0].enabled);
    assert_eq!(
        core.requests(),
        [
            format!("POST /api/v1/instance/{UUID}/mods/upload"),
            format!("DELETE /api/v1/instance/{UUID}/mods/fabric-api-0.90.jar.disabled"),
            format!("PUT /api/v1/instance/{UUID}/mods/fabric-api-0.92.jar/disable"),
        ]
    );
}

#[test]
fn a_file_never_takes_the_place_of_another_projects() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let core = FakeCore::start();
    // not Fabric API's file, whatever its name
    core.put("fabric-api-0.92.jar", &fabric_jar("something-else", "1"));

    let plan = plan(&mock, &core, 4002, true).unwrap();
    let err = run(&mock, &core, plan).unwrap_err();
    assert!(
        matches!(&err, ServerCfError::Invalid(why) if why.contains("already on the server")),
        "{err}"
    );
    assert!(core.requests().is_empty());
    assert_eq!(
        core.jars.lock().unwrap()["fabric-api-0.92.jar"],
        fabric_jar("something-else", "1")
    );
}

#[test]
fn files_that_dont_fit_or_cant_be_downloaded() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    mock.file(FakeFile::new(
        4003,
        FABRIC_API,
        "fabric-api-forge.jar",
        &["1.20.1", "Forge"],
        NEW,
        fabric_jar("fabric-api", "forge"),
    ))
    .file(
        FakeFile::new(
            6003,
            LITHIUM,
            "lithium-site-only.jar",
            &FABRIC,
            NEW,
            fabric_jar("lithium", "site"),
        )
        .blocked(),
    );
    let core = FakeCore::start();

    // for another loader: refused, unless forced
    let err = plan(&mock, &core, 4003, true).unwrap_err();
    assert!(
        matches!(&err, ServerCfError::Invalid(why) if why.contains("Forge")),
        "{err}"
    );
    let forced = block_on(server::plan_install(
        &mock.client(),
        &fabric(),
        &HashMap::new(),
        file_of(&mock, 4003),
        None,
        true,
        true,
    ))
    .unwrap();
    assert_eq!(forced.files.len(), 1);

    // kept on CurseForge's website: listed for the user, never fetched
    let plan = plan(&mock, &core, 6003, true).unwrap();
    assert!(plan.files.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(
        plan.blocked[0].file_page(),
        "https://www.curseforge.com/minecraft/mc-mods/lithium/download/6003"
    );
    let outcome = run(&mock, &core, plan).unwrap();
    assert_eq!(outcome.blocked.len(), 1);
    assert!(core.requests().is_empty());
    assert!(
        !mock
            .requests()
            .iter()
            .any(|r| r.path.starts_with("/edge.forgecdn.net"))
    );
}

#[test]
fn searches_are_shown_as_the_mods_tabs_results() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    mock.project(ESSENTIALS, "essentialsx", CLASS_BUKKIT_PLUGINS);
    let cf = mock.client();

    let found = block_on(server::search(
        &cf,
        fabric().class_id(),
        "lith",
        &SearchFilter::for_server(&fabric()),
        ModrinthSearchIndex::Downloads,
        0,
        20,
    ))
    .unwrap();
    assert_eq!(found.total_hits, 1);
    let hit = &found.hits[0];
    assert_eq!(hit.project_id, LITHIUM.to_string());
    assert_eq!(
        (hit.title.as_str(), hit.slug.as_str()),
        ("lithium title", "lithium")
    );
    assert_eq!(hit.project_type, "mod");
    assert_eq!(hit.author, "someone");
    assert_eq!(hit.downloads, 1000);
    assert_eq!(hit.client_side, "unknown");
    let asked = mock.requests().pop().unwrap();
    assert_eq!(asked.query["classId"], "6");
    assert_eq!(asked.query["gameVersion"], "1.20.1");
    assert_eq!(asked.query["sortField"], "6");
    // Fabric is loader type 4
    assert_eq!(asked.query["modLoaderTypes"], "[4]");

    // plugins: their own class, and no loader
    let paper = ServerTarget {
        kind: InstalledModKind::Plugin,
        loader: InstalledModLoader::Paper,
        game_version: "1.20.1".into(),
    };
    let found = block_on(server::search(
        &cf,
        paper.class_id(),
        "",
        &SearchFilter::for_server(&paper),
        ModrinthSearchIndex::Relevance,
        0,
        20,
    ))
    .unwrap();
    assert_eq!(found.hits.len(), 1);
    assert_eq!(found.hits[0].project_type, "plugin");
    let asked = mock.requests().pop().unwrap();
    assert_eq!(asked.query["classId"], "5");
    assert!(!asked.query.contains_key("modLoaderTypes"));

    // a project's files for the server, then all of them
    mock.file(FakeFile::new(
        6004,
        LITHIUM,
        "lithium-1.19.jar",
        &["1.19.2", "Fabric"],
        OLD,
        fabric_jar("lithium", "old"),
    ));
    let fitting = block_on(server::files(&cf, LITHIUM, &fabric(), false)).unwrap();
    assert_eq!(fitting.len(), 2);
    let all = block_on(server::files(&cf, LITHIUM, &fabric(), true)).unwrap();
    assert_eq!(all.len(), 3);
    let shown = server::version(&all[0]);
    assert_eq!(shown.project_id, LITHIUM.to_string());
    assert_eq!(shown.files[0].filename, all[0].file_name);
}

// ---------------------------------------------------------------------------
// A CurseForge modpack as a server pack

const PACK: u64 = 900_001;
const SODIUM: u64 = 394_468;
const SECRET_MOD: u64 = 555_555;
const EXTRAS: u64 = 600_001;
const FAITHFUL: u64 = 700_001;
const MINIMAP: u64 = 800_001;

/// A CurseForge pack zip for Fabric 0.15.11 on 1.20.1 listing `files` (project, file,
/// required), with one override.
fn pack_zip(files: &[(u64, u64, bool)]) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        let manifest = json!({
            "minecraft": { "version": "1.20.1", "modLoaders": [{ "id": "fabric-0.15.11", "primary": true }] },
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "Manifest Name",
            "version": "2.5",
            "author": "someone",
            "files": files.iter().map(|(p, f, r)| json!({
                "projectID": p, "fileID": f, "required": r
            })).collect::<Vec<_>>(),
            "overrides": "overrides",
        });
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file("overrides/config/create.toml", options)
            .unwrap();
        zip.write_all(b"setting = true").unwrap();
        zip.start_file("modlist.html", options).unwrap();
        zip.write_all(b"<ul></ul>").unwrap();
        zip.finish().unwrap();
    }
    bytes.into_inner()
}

fn sha512(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!("{:x}", sha2::Sha512::digest(bytes))
}

/// The entries of a zip: name to bytes.
fn unzip(path: &std::path::Path) -> HashMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            (entry.name().unwrap().into_owned(), bytes)
        })
        .collect()
}

#[test]
fn a_curseforge_modpack_becomes_a_pack_the_core_reads() {
    let mock = CurseForgeMock::start();
    projects(&mock);
    let sodium_jar = fabric_jar("sodium", "0.5.13");
    mock.project(PACK, "a-pack", 4471)
        .project(SODIUM, "sodium", CLASS_MODS)
        .project(SECRET_MOD, "secret-mod", CLASS_MODS)
        .project(EXTRAS, "optional-extras", CLASS_MODS)
        .project(FAITHFUL, "faithful-32x", 12)
        .project(MINIMAP, "minimap", CLASS_MODS)
        // Modrinth says servers don't run it
        .file(FakeFile::new(
            7_001,
            SODIUM,
            "sodium-0.5.13.jar",
            &["1.20.1", "Fabric"],
            NEW,
            sodium_jar.clone(),
        ))
        // only on CurseForge's website
        .file(
            FakeFile::new(
                7_002,
                SECRET_MOD,
                "secret-mod-1.0.jar",
                &FABRIC,
                NEW,
                fabric_jar("secret", "1.0"),
            )
            .blocked(),
        )
        .file(FakeFile::new(
            7_003,
            EXTRAS,
            "optional-extras-1.0.jar",
            &FABRIC,
            NEW,
            fabric_jar("extras", "1.0"),
        ))
        .file(FakeFile::new(
            7_004,
            FAITHFUL,
            "faithful.zip",
            &["1.20.1"],
            NEW,
            b"a resource pack".to_vec(),
        ))
        // its file's tags say players only
        .file(FakeFile::new(
            7_005,
            MINIMAP,
            "minimap-1.0.jar",
            &["1.20.1", "Fabric", "Client"],
            NEW,
            fabric_jar("minimap", "1.0"),
        ))
        .file(FakeFile::new(
            9_001,
            PACK,
            "a-pack-2.5.zip",
            &["1.20.1"],
            NEW,
            pack_zip(&[
                (CREATE, 5001, true),
                (FABRIC_API, 4002, true),
                (SODIUM, 7_001, true),
                (SECRET_MOD, 7_002, true),
                (EXTRAS, 7_003, false),
                (FAITHFUL, 7_004, true),
                (MINIMAP, 7_005, true),
                (LITHIUM, 123_456, true),
            ]),
        ));
    // Modrinth has Fabric API's very jar, and Sodium's
    let modrinth = modrinth::ModrinthMock::start();
    modrinth.project("P7dR8mSH", "fabric-api", "mod");
    modrinth.project("AANobbMI", "sodium", "mod");
    modrinth
        .state()
        .projects
        .iter_mut()
        .find(|p| p.id == "AANobbMI")
        .unwrap()
        .server_side = "unsupported".into();
    let api_on_modrinth =
        modrinth::FakeVersion::new("apiV", "P7dR8mSH", "0.92", &["fabric"], &["1.20.1"], NEW)
            .file("fabric-api-0.92.jar", &bytes_of(&mock, 4002));
    let api_url = api_on_modrinth.cdn_url();
    modrinth.version(api_on_modrinth).version(
        modrinth::FakeVersion::new(
            "sodiumV",
            "AANobbMI",
            "0.5.13",
            &["fabric"],
            &["1.20.1"],
            NEW,
        )
        .file("sodium-0.5.13.jar", &sodium_jar),
    );

    let cf = mock.client();
    let project = block_on(cf.project(PACK)).unwrap();
    let file = block_on(cf.file(PACK, 9_001)).unwrap();
    let staging = tempfile::tempdir().unwrap();
    let steps: Arc<Mutex<Vec<PackStep>>> = Arc::default();
    let seen = steps.clone();
    let pack = block_on(http::in_http_runtime(curseforge_pack::build(
        &cf,
        Some(&modrinth.client()),
        &downloader(),
        &project,
        &file,
        staging.path(),
        &move |step| seen.lock().unwrap().push(step),
        &CancelToken::new(),
    )))
    .unwrap();

    assert_eq!(pack.file_name, "a-pack-title-9001.mrpack");
    let entries = unzip(&pack.path);
    // the overrides as they were, and nothing else of the CurseForge zip
    let mut names: Vec<&str> = entries.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(
        names,
        ["modrinth.index.json", "overrides/config/create.toml"]
    );
    assert_eq!(entries["overrides/config/create.toml"], b"setting = true");

    let index: serde_json::Value = serde_json::from_slice(&entries["modrinth.index.json"]).unwrap();
    assert_eq!(index["formatVersion"], 1);
    assert_eq!(index["name"], "a-pack title");
    assert_eq!(index["versionId"], "2.5");
    assert_eq!(
        index["dependencies"],
        json!({ "minecraft": "1.20.1", "fabric-loader": "0.15.11" })
    );
    assert_eq!(
        index["lodestone"]["curseforge"],
        json!({
            "project": PACK,
            "file": 9_001,
            "pageUrl": "https://www.curseforge.com/minecraft/mc-mods/a-pack",
            "iconUrl": format!("https://media.forgecdn.net/avatars/{PACK}.png"),
        })
    );
    let files: HashMap<String, serde_json::Value> = index["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap().to_string(), f.clone()))
        .collect();
    let mut paths: Vec<&str> = files.keys().map(String::as_str).collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "mods/create-0.5.1.jar",
            "mods/fabric-api-0.92.jar",
            // optional in the pack: there, but off
            "mods/optional-extras-1.0.jar.disabled",
        ]
    );
    // only CurseForge has Create: from its CDN, with the SHA-512 worked out here
    let create = &files["mods/create-0.5.1.jar"];
    let create_bytes = bytes_of(&mock, 5001);
    assert_eq!(
        create["downloads"],
        json!(["https://edge.forgecdn.net/files/5/1/create-0.5.1.jar"])
    );
    assert_eq!(create["hashes"]["sha512"], sha512(&create_bytes));
    assert_eq!(create["fileSize"], create_bytes.len());
    // Modrinth has Fabric API's jar: from Modrinth's CDN, with Modrinth's SHA-512
    let api = &files["mods/fabric-api-0.92.jar"];
    assert_eq!(api["downloads"], json!([api_url]));
    assert_eq!(api["hashes"]["sha512"], sha512(&bytes_of(&mock, 4002)));

    // the core's reader takes it, with CurseForge's hosts asked for
    let parsed: lodestone_minecraft::mrpack::Index = serde_json::from_value(index.clone()).unwrap();
    let hosts: Vec<&str> = lodestone_minecraft::mrpack::DOWNLOAD_HOSTS
        .into_iter()
        .chain(lodestone_minecraft::mrpack::CURSEFORGE_DOWNLOAD_HOSTS)
        .collect();
    let (downloads, _) = lodestone_minecraft::mrpack::check_files_from(
        &parsed,
        lodestone_minecraft::mrpack::Side::Server,
        &lodestone_minecraft::mrpack::WriteAll,
        &hosts,
    )
    .unwrap();
    assert_eq!(downloads.len(), 3);

    // kept on CurseForge's website and nowhere else: for the user to fetch
    assert_eq!(pack.by_hand.len(), 1);
    assert_eq!(pack.by_hand[0].title, "secret-mod title");
    assert_eq!(
        pack.by_hand[0].page_url,
        "https://www.curseforge.com/minecraft/mc-mods/secret-mod/download/7002"
    );
    // and what a server has no use for, each with its reason
    let why: HashMap<&str, &str> = pack
        .skipped
        .iter()
        .map(|s| (s.path.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(why["mods/sodium-0.5.13.jar"], "it is for players only");
    assert_eq!(why["minimap-1.0.jar"], "it is for players only");
    assert_eq!(why["faithful.zip"], "a server doesn't use it");
    assert_eq!(why["CurseForge file 123456"], "CurseForge no longer has it");

    // the files only CurseForge has were fetched once each, the others not at all
    let fetched: Vec<String> = mock
        .requests()
        .iter()
        .filter(|r| r.path.starts_with("/edge.forgecdn.net"))
        .map(|r| r.path.rsplit('/').next().unwrap().to_string())
        .collect();
    let count = |name: &str| fetched.iter().filter(|f| f.as_str() == name).count();
    assert_eq!(count("a-pack-2.5.zip"), 1);
    assert_eq!(count("create-0.5.1.jar"), 1);
    assert_eq!(count("optional-extras-1.0.jar"), 1);
    assert_eq!(count("fabric-api-0.92.jar"), 0);
    assert_eq!(count("sodium-0.5.13.jar"), 0);
    // nothing is left in the staging folder but the pack
    let left: Vec<String> = std::fs::read_dir(staging.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, ["curseforge-pack-9001.mrpack"]);
    let steps = steps.lock().unwrap();
    assert_eq!(steps.first(), Some(&PackStep::Downloading));
    assert!(steps.contains(&PackStep::Hashing { done: 2, total: 2 }));
    assert_eq!(steps.last(), Some(&PackStep::Writing));
}

#[test]
fn a_pack_kept_on_curseforges_website_cant_be_fetched() {
    let mock = CurseForgeMock::start();
    mock.project(PACK, "a-pack", 4471)
        .file(FakeFile::new(9_001, PACK, "a-pack.zip", &["1.20.1"], NEW, pack_zip(&[])).blocked());
    let cf = mock.client();
    let project = block_on(cf.project(PACK)).unwrap();
    let file = block_on(cf.file(PACK, 9_001)).unwrap();
    let staging = tempfile::tempdir().unwrap();
    let err = block_on(http::in_http_runtime(curseforge_pack::build(
        &cf,
        None,
        &downloader(),
        &project,
        &file,
        staging.path(),
        &|_| {},
        &CancelToken::new(),
    )))
    .unwrap_err();
    assert!(err.to_string().contains("CurseForge's website"), "{err}");
}

#[test]
fn a_servers_curseforge_pack_has_a_newer_file() {
    let mock = CurseForgeMock::start();
    mock.project(PACK, "a-pack", 4471)
        .file(FakeFile::new(
            9_001,
            PACK,
            "a-pack-1.zip",
            &["1.20.1"],
            OLD,
            pack_zip(&[]),
        ))
        .file(FakeFile::new(
            9_002,
            PACK,
            "a-pack-2.zip",
            &["1.20.1"],
            NEW,
            pack_zip(&[]),
        ))
        .file(
            FakeFile::new(
                9_003,
                PACK,
                "a-pack-3-beta.zip",
                &["1.20.1"],
                "2025-03-01T00:00:00Z",
                pack_zip(&[]),
            )
            .beta(),
        );
    let cf = mock.client();
    let origin = |file| CurseForgeOrigin {
        project: PACK,
        file,
        ..Default::default()
    };
    // a release moves to the newest release, not to a beta
    let newer = block_on(curseforge_pack::newer_file(&cf, &origin(9_001), "1.20.1")).unwrap();
    assert_eq!(newer.map(|f| f.id), Some(9_002));
    assert_eq!(
        block_on(curseforge_pack::newer_file(&cf, &origin(9_002), "1.20.1")).unwrap(),
        None
    );
    // a server already on a newer Minecraft version isn't offered the older line
    // the pack still publishes, and one on an older version is offered the newer
    assert_eq!(
        block_on(curseforge_pack::newer_file(&cf, &origin(9_001), "1.21.1")).unwrap(),
        None
    );
    let newer = block_on(curseforge_pack::newer_file(&cf, &origin(9_001), "1.19.2")).unwrap();
    assert_eq!(newer.map(|f| f.id), Some(9_002));
    // from a beta, nothing older
    assert_eq!(
        block_on(curseforge_pack::newer_file(&cf, &origin(9_003), "1.20.1")).unwrap(),
        None
    );
}
