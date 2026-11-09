//! The Console and Files tabs (work package B) against the real `lodestone_core`
//! program: the routes as the Host calls them.
//!
//! Needs the core's program, as `core_child.rs` does; without it the tests say so and
//! pass. `LODESTONE_HOST_NETWORK_TESTS=1` also makes a vanilla instance (the core
//! downloads its server) and works with its console and files. No server is ever
//! started, and the Minecraft EULA is never accepted.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        admin::{
            AdminClient, InstanceState,
            files::{UnzipOption, join, separator},
        },
        local_claim::OwnerCredentials,
    },
    game::download::Sleep,
};
use lodestone_host::{
    core_process::{self, CoreConfig, CoreProcess},
    session,
};

fn sleep() -> Sleep {
    Arc::new(|duration| Box::pin(async move { std::thread::sleep(duration) }))
}

/// A core in a temporary folder, signed in as its owner. Stops when dropped.
struct TestCore {
    process: CoreProcess,
    admin: AdminClient,
    data: tempfile::TempDir,
}

impl Drop for TestCore {
    fn drop(&mut self) {
        self.process.stop(Duration::from_secs(30));
    }
}

fn start_core(mut ports: std::ops::Range<u16>) -> Option<TestCore> {
    let program = match core_process::find_core() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let data = tempfile::tempdir().unwrap();
    let port = ports
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port");
    let config = CoreConfig {
        program,
        data_dir: data.path().join("core"),
        port,
        log_file: data.path().join("core.log"),
    };
    let mut process = CoreProcess::spawn(&config).expect("the core starts");
    let http = launcher_core::http::default_client().unwrap();
    let local = CoreClient::new(http.clone(), config.local_url());
    let started = Instant::now();
    while block_on(local.info()).is_err() {
        assert!(process.try_wait().is_none(), "the core exited");
        assert!(started.elapsed() < Duration::from_secs(90), "no answer");
        std::thread::sleep(Duration::from_millis(250));
    }
    let owner = OwnerCredentials {
        username: "files_owner".into(),
        password: Secret::new(String::from("a long test password 789")),
    };
    let session = block_on(session::claim_owner(
        http,
        config.local_url(),
        &config.data_dir,
        Some(owner),
        sleep(),
    ))
    .expect("signed in");
    Some(TestCore {
        process,
        admin: session.admin,
        data,
    })
}

#[test]
fn console_and_files_refuse_an_unknown_instance() {
    let Some(core) = start_core(17_600..17_700) else {
        return;
    };
    let admin = &core.admin;
    let refused = |result: Result<(), CoreError>| {
        assert!(
            matches!(
                result,
                Err(CoreError::NotFound(_) | CoreError::Server { .. } | CoreError::Invalid(_))
            ),
            "{result:?}"
        )
    };
    // the console's history of an unknown instance is empty
    assert!(
        block_on(admin.console_buffer("nope"))
            .unwrap_or_default()
            .is_empty()
    );
    refused(block_on(admin.send_console("nope", "list")));
    refused(block_on(admin.list_files("nope", ".")).map(drop));
    refused(block_on(admin.read_file("nope", "server.properties")).map(drop));
    refused(block_on(admin.new_folder("nope", "./x")));
}

/// Waits until `check` holds, for work the core does in the background.
fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(started.elapsed() < Duration::from_secs(60), "{what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[test]
fn a_stopped_instances_console_and_files() {
    if std::env::var("LODESTONE_HOST_NETWORK_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set LODESTONE_HOST_NETWORK_TESTS=1 to download a vanilla server");
        return;
    }
    let Some(core) = start_core(17_700..17_800) else {
        return;
    };
    let admin = &core.admin;

    // a vanilla instance with the manifest's defaults, the EULA not accepted
    let manifest: serde_json::Value =
        block_on(admin.get("setup_manifest/MinecraftJavaVanilla")).unwrap();
    let mut sections = serde_json::Map::new();
    for (id, section) in manifest["setting_sections"].as_object().unwrap() {
        let mut settings = serde_json::Map::new();
        for (key, setting) in section["settings"].as_object().unwrap() {
            let value = if setting["value"].is_null() {
                setting["default_value"].clone()
            } else {
                setting["value"].clone()
            };
            settings.insert(key.clone(), serde_json::json!({ "value": value }));
        }
        sections.insert(id.clone(), serde_json::json!({ "settings": settings }));
    }
    let setup = serde_json::json!({
        "name": "Files test", "description": "", "auto_start": false,
        "restart_on_crash": false, "accept_eula": false, "setting_sections": sections,
    });
    block_on(admin.call(
        http_client::Method::POST,
        "instance/create/MinecraftJavaVanilla",
        Some(setup),
    ))
    .expect("the instance is made");
    let started = Instant::now();
    let instance = loop {
        if let Some(instance) = block_on(admin.instances()).unwrap().pop() {
            break instance;
        }
        assert!(started.elapsed() < Duration::from_secs(600), "not made");
        std::thread::sleep(Duration::from_secs(1));
    };
    let uuid = instance.uuid.as_str();
    let sep = separator(&instance.path);

    // the console: its history, and a command is refused while it is stopped
    assert!(block_on(admin.console_buffer(uuid)).is_ok());
    assert!(block_on(admin.send_console(uuid, "list")).is_err());

    // listing, once the core has set the instance up
    let started = Instant::now();
    let top = loop {
        let top = block_on(admin.list_files(uuid, ".")).unwrap_or_default();
        if top.iter().any(|f| f.name == "server.properties")
            && top.iter().any(|f| f.name == "eula.txt")
        {
            break top;
        }
        assert!(started.elapsed() < Duration::from_secs(600), "not set up");
        std::thread::sleep(Duration::from_secs(1));
    };
    assert!(top.iter().any(|f| f.name == "server.properties"));
    let eula = block_on(admin.read_file(uuid, "eula.txt")).unwrap();
    assert!(eula.contains("eula=false"));

    // new folder and file, write, read, rename, copy
    let dir = join(".", "b_test", sep);
    block_on(admin.new_folder(uuid, &dir)).unwrap();
    let a = join(&dir, "a.txt", sep);
    block_on(admin.new_file(uuid, &a)).unwrap();
    block_on(admin.write_file(uuid, &a, "one\r\ntwo ü\n")).unwrap();
    assert_eq!(
        block_on(admin.read_file(uuid, &a)).unwrap(),
        "one\r\ntwo ü\n"
    );
    let b = join(&dir, "b.txt", sep);
    block_on(admin.move_path(uuid, &a, &b)).unwrap();
    block_on(admin.copy_paths(uuid, std::slice::from_ref(&b), ".")).unwrap();
    let names = |path: &str| -> Vec<String> {
        block_on(admin.list_files(uuid, path))
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect()
    };
    assert!(names(".").contains(&"b.txt".to_string()));
    assert_eq!(names(&dir), vec!["b.txt".to_string()]);
    assert_eq!(
        names(".")[0..3].iter().filter(|n| *n == "b_test").count(),
        1
    );

    // upload into the folder
    block_on(admin.upload_files(
        uuid,
        &dir,
        &[
            ("up 1.txt".into(), b"first".to_vec()),
            ("up2.json".into(), b"{\"a\": 1}".to_vec()),
        ],
    ))
    .unwrap();
    assert_eq!(names(&dir), vec!["b.txt", "up 1.txt", "up2.json"]);

    // zip and unzip, done in the background
    let zip = join(".", "b_test.zip", sep);
    block_on(admin.zip_paths(uuid, std::slice::from_ref(&dir), &zip)).unwrap();
    // done once its size stops changing (the core writes it in place)
    let zip_size = || {
        block_on(admin.list_files(uuid, "."))
            .unwrap()
            .into_iter()
            .find(|f| f.name == "b_test.zip")
            .and_then(|f| f.size)
    };
    let mut last = None;
    eventually("the zip is made", || {
        std::thread::sleep(Duration::from_millis(500));
        let size = zip_size();
        let done = size.is_some_and(|s| s > 0) && size == last;
        last = size;
        done
    });
    let unzip_into = join(".", "unzipped", sep);
    block_on(admin.new_folder(uuid, &unzip_into)).unwrap();
    let zipped_copy = join(&unzip_into, "b_test.zip", sep);
    block_on(admin.copy_paths(uuid, std::slice::from_ref(&zip), &unzip_into)).unwrap();
    block_on(admin.unzip(uuid, &zipped_copy, &UnzipOption::ToDirectoryWithFileName)).unwrap();
    eventually("the zip is unzipped", || {
        names(&unzip_into)
            .iter()
            .any(|n| n == "b_test" || n == "b_test(1)")
    });

    // download a file and a folder (zipped by the core)
    let out = core.data.path().join("server.properties");
    let properties = top.iter().find(|f| f.name == "server.properties").unwrap();
    let size = block_on(admin.download_to(uuid, properties, &out)).unwrap();
    assert_eq!(size, std::fs::metadata(&out).unwrap().len());
    assert!(std::fs::read_to_string(&out).unwrap().contains('='));
    let folder = block_on(admin.list_files(uuid, "."))
        .unwrap()
        .into_iter()
        .find(|f| f.name == "b_test")
        .unwrap();
    let out = core.data.path().join("b_test.zip");
    assert!(block_on(admin.download_to(uuid, &folder, &out)).unwrap() > 0);
    assert_eq!(&std::fs::read(&out).unwrap()[..2], b"PK");

    // delete a file and a folder
    block_on(admin.delete_path(uuid, &join(".", "b.txt", sep), false)).unwrap();
    block_on(admin.delete_path(uuid, &dir, true)).unwrap();
    let left = names(".");
    assert!(!left.contains(&"b.txt".into()) && !left.contains(&"b_test".into()));

    // nothing started the server, and the EULA is as it was
    let instance = block_on(admin.instances()).unwrap().pop().unwrap();
    assert_eq!(instance.state, InstanceState::Stopped);
    assert!(
        block_on(admin.read_file(uuid, "eula.txt"))
            .unwrap()
            .contains("eula=false")
    );
}
