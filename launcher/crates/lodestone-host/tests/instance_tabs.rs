//! An instance's tabs (work package C) against the real `lodestone_core` program: the
//! Settings tab's routes, as the Host calls them.
//!
//! Needs the core's program, as `core_child.rs` does; without it the tests say so and
//! pass. `LODESTONE_HOST_NETWORK_TESTS=1` also makes a Fabric instance (the core
//! downloads its server files) and changes its settings. No server is ever started,
//! and the Minecraft EULA is never accepted.

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
            AdminClient,
            instance_settings::{ConfigurableValue, ServerResourcePack},
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
    _data: tempfile::TempDir,
}

impl Drop for TestCore {
    fn drop(&mut self) {
        self.process.stop(Duration::from_secs(30));
    }
}

/// `ports`: a range of its own for each test, as tests run at the same time.
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
        username: "tabs_owner".into(),
        password: Secret::new(String::from("a long test password 456")),
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
        _data: data,
    })
}

#[test]
fn settings_routes_refuse_an_unknown_instance() {
    let Some(core) = start_core(17_400..17_500) else {
        return;
    };
    let admin = &core.admin;
    let unknown = |result: Result<(), CoreError>| {
        assert!(
            matches!(
                result,
                Err(CoreError::NotFound(_) | CoreError::Server { .. } | CoreError::Invalid(_))
            ),
            "{result:?}"
        )
    };
    unknown(block_on(admin.instance_settings("nope")).map(drop));
    unknown(block_on(admin.eula("nope")).map(drop));
    unknown(block_on(admin.resource_pack("nope")).map(drop));
    unknown(block_on(admin.set_auto_start("nope", true)));
    unknown(block_on(admin.delete_instance("nope")));
}

#[test]
fn a_fabric_instances_settings() {
    if std::env::var("LODESTONE_HOST_NETWORK_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set LODESTONE_HOST_NETWORK_TESTS=1 to download a Fabric server");
        return;
    }
    let Some(core) = start_core(17_500..17_600) else {
        return;
    };
    let admin = &core.admin;
    // made with the EULA not accepted, as the user hasn't agreed to it here
    let setup = serde_json::json!({
        "name": "Tabs test", "description": "", "auto_start": false,
        "restart_on_crash": false, "accept_eula": false,
        "setting_sections": {
            "section_1": { "settings": {
                "version": { "value": { "type": "Enum", "value": "1.21.1" } },
                "port": { "value": { "type": "UnsignedInteger", "value": 25590 } } } },
            "section_2": { "settings": {
                "min_ram": { "value": { "type": "UnsignedInteger", "value": 1024 } },
                "max_ram": { "value": { "type": "UnsignedInteger", "value": 2048 } },
                "cmd_args": { "value": null } } }
        }
    });
    block_on(admin.call(
        http_client::Method::POST,
        "instance/create/MinecraftFabric",
        Some(setup),
    ))
    .expect("created");
    let started = Instant::now();
    let uuid = loop {
        if let Some(i) = block_on(admin.instances()).unwrap().first() {
            break i.uuid.clone();
        }
        assert!(started.elapsed() < Duration::from_secs(300), "not made");
        std::thread::sleep(Duration::from_secs(1));
    };

    let manifest = block_on(admin.instance_settings(&uuid)).unwrap();
    assert!(!manifest.setting_sections.is_empty());
    let (section, max_ram) = manifest
        .setting_sections
        .iter()
        .find_map(|(id, s)| {
            s.settings
                .iter()
                .find(|(k, _)| k == "max_ram")
                .map(|(_, v)| (id.clone(), v.clone()))
        })
        .expect("a max_ram setting");
    assert_eq!(
        max_ram.value,
        Some(ConfigurableValue::UnsignedInteger(2048))
    );
    block_on(admin.set_instance_setting(
        &uuid,
        &section,
        "max_ram",
        &ConfigurableValue::UnsignedInteger(3072),
    ))
    .unwrap();
    let again = block_on(admin.instance_settings(&uuid)).unwrap();
    let saved = again
        .setting_sections
        .iter()
        .flat_map(|(_, s)| s.settings.iter())
        .find(|(k, _)| k == "max_ram")
        .and_then(|(_, v)| v.value.clone());
    assert_eq!(saved, Some(ConfigurableValue::UnsignedInteger(3072)));

    block_on(admin.set_auto_start(&uuid, true)).unwrap();
    block_on(admin.set_restart_on_crash(&uuid, true)).unwrap();
    let info = block_on(admin.instances()).unwrap().remove(0);
    assert!(info.auto_start && info.restart_on_crash);
    block_on(admin.set_auto_start(&uuid, false)).unwrap();

    // the EULA isn't accepted, and the test never accepts it
    assert!(!block_on(admin.eula(&uuid)).unwrap().accepted);

    let pack = ServerResourcePack {
        url: "https://example.com/pack.zip".into(),
        sha1: "A".repeat(40),
        required: true,
        prompt: "Please".into(),
    };
    let written = block_on(admin.set_resource_pack(&uuid, &pack)).unwrap();
    assert_eq!(written.sha1, "a".repeat(40), "lowercased");
    assert_eq!(block_on(admin.resource_pack(&uuid)).unwrap(), written);
    let removed = block_on(admin.set_resource_pack(&uuid, &ServerResourcePack::default())).unwrap();
    assert!(removed.url.is_empty());

    block_on(admin.delete_instance(&uuid)).unwrap();
}
