//! "Download this world" against a mock core on a loopback port
//! (`cores::world_download`): a server's backup is exported and downloaded as a
//! `.lodeimg`, the core's file let go of however it ends, and the file becomes a new
//! instance through the snapshot import. The file the mock serves is one a real core
//! exported (`tests/fixtures/core_images`).

mod modrinth;

use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    str::FromStr as _,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        admin::AdminClient,
        world_download::{self, DownloadProgress, WhichBackup, WorldDownloadError},
    },
    game::download::{CancelToken, Downloader, Progress, Sleep, recording_sleep},
    http,
    images::{Images, export},
    instances::InstanceStore,
    paths::DataDir,
};
use modrinth::ModrinthMock;
use serde_json::{Value, json};

const OWNER_KEY: &str = "lsk_abcdefghijkl_KEY-SECRET";
const SERVER: &str = "server-1";
const KEY: &str = "k1";

#[derive(Clone, Debug)]
struct Recorded {
    method: String,
    path: String,
}

#[derive(Default)]
struct State {
    requests: Vec<Recorded>,
    too_old: bool,
    forbidden: bool,
    /// The backup asked for is gone (pruned, deleted by another admin).
    backup_gone: bool,
    /// The export says it is this big.
    size: Option<u64>,
    /// Backing up is refused, reading the backups isn't.
    backup_forbidden: bool,
    /// The export asked for, by backup id.
    exported: Vec<String>,
    /// Wait this long before sending the file.
    file_delay: Option<Duration>,
    /// Send the file's first bytes, then nothing more: a link gone dead mid-download.
    stall_body: bool,
    events: Vec<Value>,
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

fn fixture_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core_images/fabric-backup.lodeimg")
}

fn backup(id: &str, created: i64) -> Value {
    json!({ "id": id, "created": created, "kind": "manual", "live": true, "quiesced": false,
            "total_bytes": 100 })
}

fn route(state: &mut State, request: &Recorded) -> (u16, Vec<u8>) {
    let reply = |status: u16, value: Value| (status, value.to_string().into_bytes());
    let path = request
        .path
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let backups = format!("/api/v1/instance/{SERVER}/backups");
    if state.forbidden && path.starts_with(&backups) {
        return reply(
            403,
            json!({ "kind": "PermissionDenied", "causes": ["Not allowed"] }),
        );
    }
    match (request.method.as_str(), path.as_str()) {
        ("GET", p) if p == backups => reply(
            200,
            json!({ "backups": [backup("b-old", 100), backup("b-new", 200)],
                    "policy": {}, "running": null, "usage": {} }),
        ),
        ("POST", p) if p == backups => {
            if state.backup_forbidden {
                return reply(403, json!({ "kind": "PermissionDenied", "causes": ["No"] }));
            }
            state.events = vec![json!({
                "event_inner": {"type": "ProgressionEvent", "event_id": "77",
                    "progression_event_inner": {"type": "ProgressionEnd", "success": true,
                        "message": "Backup done",
                        "inner": {"type": "BackupCompleted", "instance_uuid": SERVER,
                                  "backup_id": "b-taken", "success": true, "message": "Backup done"}}},
                "details": "", "snowflake": "78", "level": "Info", "caused_by": {"type": "System"}
            })];
            reply(200, json!({ "task_id": 77 }))
        }
        ("GET", "/api/v1/events/all/buffer") => reply(200, json!(state.events)),
        ("POST", p) if p.starts_with(&backups) && p.ends_with("/export") => {
            if state.too_old {
                // what a core answers for a route it doesn't have: nothing
                return (404, Vec::new());
            }
            if state.backup_gone {
                return reply(
                    404,
                    json!({ "kind": "NotFound", "causes": ["Backup not found"] }),
                );
            }
            let id = p
                .trim_start_matches(&format!("{backups}/"))
                .trim_end_matches("/export")
                .to_string();
            state.exported.push(id.clone());
            let size = state
                .size
                .unwrap_or_else(|| fs::metadata(fixture_file()).unwrap().len());
            reply(
                200,
                json!({ "key": KEY, "file_name": format!("Survival - {id}.lodeimg"), "size": size,
                        "left_out": {"names": ["eula.txt"], "count": 1},
                        "left_out_for_players": {"names": [], "count": 0} }),
            )
        }
        ("GET", p) if p == format!("/api/v1/file/{KEY}") => {
            (200, fs::read(fixture_file()).unwrap())
        }
        ("DELETE", p) if p == format!("/api/v1/file/{KEY}") => reply(200, json!(null)),
        _ => (404, Vec::new()),
    }
}

/// A body that sends 10 bytes, then waits without closing, as a half-open connection does.
struct Stalling {
    sent: bool,
}

impl std::io::Read for Stalling {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if !self.sent {
            self.sent = true;
            let n = buffer.len().min(10);
            buffer[..n].fill(7);
            return Ok(n);
        }
        std::thread::sleep(Duration::from_secs(20));
        Err(std::io::Error::other("the link went dead"))
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
                    std::thread::spawn(move || {
                        let mut body = Vec::new();
                        let _ = request.as_reader().read_to_end(&mut body);
                        let recorded = Recorded {
                            method: request.method().to_string(),
                            path: request.url().to_string(),
                        };
                        let delay = {
                            let mut state = state.lock().unwrap();
                            state.requests.push(recorded.clone());
                            state.file_delay.filter(|_| {
                                recorded.method == "GET" && recorded.path.contains("/file/")
                            })
                        };
                        if let Some(delay) = delay {
                            std::thread::sleep(delay);
                        }
                        let stall = state.lock().unwrap().stall_body
                            && recorded.method == "GET"
                            && recorded.path.contains("/file/");
                        if stall {
                            let size = fs::metadata(fixture_file()).unwrap().len() as usize;
                            let response = tiny_http::Response::new(
                                tiny_http::StatusCode(200),
                                Vec::new(),
                                Stalling { sent: false },
                                Some(size),
                                None,
                            );
                            let _ = request.respond(response);
                            return;
                        }
                        let (status, bytes) = route(&mut state.lock().unwrap(), &recorded);
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

    fn admin(&self) -> AdminClient {
        AdminClient::new(
            CoreClient::new(http::default_client().unwrap(), self.base.clone()),
            Secret::new(OWNER_KEY),
        )
    }

    fn released(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.method == "DELETE" && r.path == format!("/api/v1/file/{KEY}"))
    }
}

fn no_sleep() -> Sleep {
    Arc::new(|_| Box::pin(async {}))
}

fn files_in(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn download(
    core: &MockCore,
    which: WhichBackup,
    dir: &Path,
    cancel: &CancelToken,
) -> Result<world_download::Downloaded, WorldDownloadError> {
    block_on(world_download::download(
        &core.admin(),
        SERVER,
        which,
        dir,
        &DownloadProgress::default(),
        cancel,
        &no_sleep(),
    ))
}

#[test]
fn the_backups_are_listed_newest_first() {
    let core = MockCore::start();
    let list = block_on(world_download::backups(&core.admin(), SERVER)).unwrap();
    let ids: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["b-new", "b-old"]);
    assert!(list[0].may_be_inconsistent());
}

#[test]
fn the_newest_backup_comes_with_progress_and_the_cores_file_is_let_go_of() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    let progress = DownloadProgress::default();
    let downloaded = block_on(world_download::download(
        &core.admin(),
        SERVER,
        WhichBackup::Newest,
        temp.path(),
        &progress,
        &CancelToken::new(),
        &no_sleep(),
    ))
    .unwrap();
    assert_eq!(downloaded.backup, "b-new");
    assert_eq!(core.state.lock().unwrap().exported, ["b-new"]);
    assert_eq!(
        fs::read(downloaded.path()).unwrap(),
        fs::read(fixture_file()).unwrap()
    );
    let bytes = progress.bytes.snapshot();
    assert_eq!(bytes.bytes_done, bytes.bytes_total);
    assert!(bytes.bytes_total > 0);
    assert_eq!(downloaded.exported.left_out.names, ["eula.txt"]);
    assert!(core.released());
    // the file goes with it, deleted away from the dropping thread
    let path = downloaded.path().to_path_buf();
    drop(downloaded);
    wait_until("the file is deleted", || !path.exists());
    assert!(files_in(temp.path()).is_empty());
}

#[test]
fn a_new_backup_is_taken_and_waited_for() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    let progress = DownloadProgress::default();
    let downloaded = block_on(world_download::download(
        &core.admin(),
        SERVER,
        WhichBackup::New,
        temp.path(),
        &progress,
        &CancelToken::new(),
        &no_sleep(),
    ))
    .unwrap();
    assert_eq!(downloaded.backup, "b-taken");
    // said, so a download that stops later can take this one instead of another
    assert_eq!(progress.backup_made().as_deref(), Some("b-taken"));
    assert_eq!(core.state.lock().unwrap().exported, ["b-taken"]);
}

#[test]
fn an_older_core_or_a_user_who_cant_read_the_files_is_told_plainly() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    core.state.lock().unwrap().too_old = true;
    let refused = download(
        &core,
        WhichBackup::Id("b-old".into()),
        temp.path(),
        &CancelToken::new(),
    );
    assert!(
        matches!(refused, Err(WorldDownloadError::NeedsUpdate)),
        "{refused:?}"
    );
    let core = MockCore::start();
    core.state.lock().unwrap().forbidden = true;
    let refused = download(&core, WhichBackup::Newest, temp.path(), &CancelToken::new());
    assert!(
        matches!(refused, Err(WorldDownloadError::Forbidden)),
        "{refused:?}"
    );
    assert!(files_in(temp.path()).is_empty());
}

/// A 404 the core explains is its own refusal, not a core too old for the route.
#[test]
fn a_backup_gone_is_the_cores_message_not_an_update() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    core.state.lock().unwrap().backup_gone = true;
    let refused = download(
        &core,
        WhichBackup::Id("b-old".into()),
        temp.path(),
        &CancelToken::new(),
    );
    match refused {
        Err(WorldDownloadError::Core(err)) => {
            assert!(err.to_string().contains("Backup not found"), "{err}")
        }
        other => panic!("{other:?}"),
    }
    assert!(files_in(temp.path()).is_empty());
}

/// The core says the file's size first: one that won't fit is never downloaded, and the
/// core's copy goes.
#[test]
fn a_world_too_big_for_the_disk_is_refused_before_it_comes() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    core.state.lock().unwrap().size = Some(u64::MAX / 2);
    let refused = download(&core, WhichBackup::Newest, temp.path(), &CancelToken::new());
    assert!(
        matches!(refused, Err(WorldDownloadError::NoRoom { .. })),
        "{refused:?}"
    );
    assert!(core.released(), "the core's file is let go of");
    assert!(
        !core
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.method == "GET" && r.path.contains("/file/")),
        "nothing was downloaded"
    );
    assert!(files_in(temp.path()).is_empty());
}

/// The download is held to the size the core stated, which the free-space check counted:
/// a core sending more (or less) than it said is refused, and nothing is left.
#[test]
fn a_core_sending_another_size_than_it_stated_is_refused() {
    let len = fs::metadata(fixture_file()).unwrap().len();
    for stated in [10, len - 1, len + 1] {
        let core = MockCore::start();
        let temp = tempfile::tempdir().unwrap();
        core.state.lock().unwrap().size = Some(stated);
        let refused = download(&core, WhichBackup::Newest, temp.path(), &CancelToken::new());
        assert!(
            matches!(refused, Err(WorldDownloadError::Core(_))),
            "{stated}: {refused:?}"
        );
        wait_until("the core's file is let go of", || core.released());
        assert!(files_in(temp.path()).is_empty(), "{stated}");
    }
}

/// A user who may read the files but not back the server up is told so, not that they
/// can't read its files.
#[test]
fn a_user_who_cant_back_up_is_told_to_pick_a_backup() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    core.state.lock().unwrap().backup_forbidden = true;
    let refused = download(&core, WhichBackup::New, temp.path(), &CancelToken::new());
    assert!(
        matches!(refused, Err(WorldDownloadError::BackupForbidden)),
        "{refused:?}"
    );
}

#[test]
fn cancel_mid_download_leaves_nothing_and_lets_the_cores_file_go() {
    let core = MockCore::start();
    core.state.lock().unwrap().file_delay = Some(Duration::from_secs(2));
    let temp = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    {
        let cancel = cancel.clone();
        let state = core.state.clone();
        std::thread::spawn(move || {
            loop {
                let asked = state
                    .lock()
                    .unwrap()
                    .requests
                    .iter()
                    .any(|r| r.method == "GET" && r.path.contains("/file/"));
                if asked {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            cancel.cancel();
        });
    }
    let cancelled = download(&core, WhichBackup::Newest, temp.path(), &cancel);
    assert!(
        matches!(cancelled, Err(WorldDownloadError::Cancelled)),
        "{cancelled:?}"
    );
    wait_until("the core's file is let go of", || core.released());
    assert!(
        files_in(temp.path()).is_empty(),
        "{:?}",
        files_in(temp.path())
    );
}

/// Cancel stops a download whose connection went quiet mid-file (Wi-Fi dropped, a lid
/// closed) at once, rather than when a timeout runs out, and still leaves nothing.
#[test]
fn cancel_stops_a_stalled_download_at_once() {
    let core = MockCore::start();
    core.state.lock().unwrap().stall_body = true;
    let temp = tempfile::tempdir().unwrap();
    let cancel = CancelToken::new();
    {
        let cancel = cancel.clone();
        let state = core.state.clone();
        std::thread::spawn(move || {
            while !state
                .lock()
                .unwrap()
                .requests
                .iter()
                .any(|r| r.method == "GET" && r.path.contains("/file/"))
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            // past the first bytes, into the stall
            std::thread::sleep(Duration::from_millis(500));
            cancel.cancel();
        });
    }
    let started = std::time::Instant::now();
    let cancelled = download(&core, WhichBackup::Newest, temp.path(), &cancel);
    assert!(
        matches!(cancelled, Err(WorldDownloadError::Cancelled)),
        "{cancelled:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "took {:?}",
        started.elapsed()
    );
    wait_until("the core's file is let go of", || core.released());
    assert!(
        files_in(temp.path()).is_empty(),
        "{:?}",
        files_in(temp.path())
    );
}

/// A download whose future is dropped mid-way (the dialog closed, the Launcher's task
/// gone) leaves nothing in the folder and still lets the core's file go.
#[test]
fn a_download_dropped_mid_way_leaves_nothing_and_lets_the_cores_file_go() {
    let core = MockCore::start();
    core.state.lock().unwrap().file_delay = Some(Duration::from_secs(2));
    let temp = tempfile::tempdir().unwrap();
    let admin = core.admin();
    let progress = DownloadProgress::default();
    let cancel = CancelToken::new();
    let sleep = no_sleep();
    let download = Box::pin(world_download::download(
        &admin,
        SERVER,
        WhichBackup::Newest,
        temp.path(),
        &progress,
        &cancel,
        &sleep,
    ));
    // polled until the file is asked for, then dropped with it in flight
    let asked = Box::pin(futures::future::poll_fn(|cx| {
        let asked = core
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| r.method == "GET" && r.path.contains("/file/"));
        if asked {
            std::task::Poll::Ready(())
        } else {
            std::thread::sleep(Duration::from_millis(5));
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }));
    match block_on(futures::future::select(download, asked)) {
        futures::future::Either::Left((done, _)) => panic!("finished first: {done:?}"),
        futures::future::Either::Right(((), download)) => drop(download),
    }
    wait_until("the core's file is let go of", || core.released());
    wait_until("nothing is left in the folder", || {
        files_in(temp.path()).is_empty()
    });
}

/// End to end: the downloaded file becomes a new instance named after the server.
#[test]
fn a_downloaded_world_becomes_an_instance() {
    let core = MockCore::start();
    let temp = tempfile::tempdir().unwrap();
    let data = DataDir::at(temp.path().join("data"));
    let store = InstanceStore::new(data.instances_dir());
    fs::create_dir_all(store.root()).unwrap();
    let images = Images::open(&data);
    let downloaded = download(
        &core,
        WhichBackup::Newest,
        &data.cache_dir().join(world_download::DOWNLOADS_FOLDER),
        &CancelToken::new(),
    )
    .unwrap();
    let mock = ModrinthMock::start();
    let downloader =
        Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1);
    let imported = block_on(export::import_file(
        &images,
        export::Sources {
            downloader: &downloader,
            modrinth: &mock.client(),
            curseforge: None,
        },
        export::ImportRequest {
            server: Some("Survival SMP"),
            ..export::ImportRequest::new(downloaded.path())
        },
        &|_| Ok(()),
        &Progress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(imported.instance.name, "Survival SMP");
    let game = store.game_dir(&imported.instance.id);
    assert!(game.join("saves/world/level.dat").is_file());
    assert!(!game.join("server.properties").exists());
    // nothing was served, so both Modrinth mods are listed as left out
    assert_eq!(imported.left_out, ["mods/lithium.jar", "mods/sodium.jar"]);
    // the file is deleted away from the dropping thread, so wait for it
    let path = downloaded.path().to_path_buf();
    drop(downloaded);
    wait_until("the file is deleted", || !path.exists());
    assert!(files_in(&data.cache_dir().join(world_download::DOWNLOADS_FOLDER)).is_empty());
}
