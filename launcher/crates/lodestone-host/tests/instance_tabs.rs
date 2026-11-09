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
            mods::ModInstallRequest,
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

    // its history has no events of an unknown instance
    assert!(block_on(admin.event_history()).is_ok());
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
        .find_map(|s| {
            s.settings
                .iter()
                .find(|v| v.setting_id == "max_ram")
                .map(|v| (s.section_id.clone(), v.clone()))
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
        .flat_map(|s| s.settings.iter())
        .find(|v| v.setting_id == "max_ram")
        .and_then(|v| v.value.clone());
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

    // the Mods tab: install from Modrinth, turn off and on, the client pack, delete
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    assert!(list.mods.is_empty());
    assert_eq!(list.directory, "mods");
    let versions =
        block_on(admin.modrinth_versions("fabric-api", Some("1.21.1"), Some("fabric"))).unwrap();
    let oldest = versions.last().expect("a Fabric API version for 1.21.1");
    let result = block_on(admin.install_mod(
        &uuid,
        &ModInstallRequest {
            version_id: oldest.id.clone(),
            install_dependencies: true,
            force: false,
        },
    ))
    .unwrap();
    assert_eq!(result.installed.len(), 1);
    let file = result.installed[0].file_name.clone();
    let checked = block_on(admin.installed_mods(&uuid, true)).unwrap();
    assert!(
        checked.mods[0]
            .modrinth
            .as_ref()
            .is_some_and(|m| m.update.is_some()),
        "the oldest version has an update"
    );
    let disabled = block_on(admin.set_mod_enabled(&uuid, &file, false)).unwrap();
    assert!(!disabled.enabled);
    let enabled = block_on(admin.set_mod_enabled(&uuid, &disabled.file_name, true)).unwrap();
    assert!(enabled.enabled);
    // the same file again is refused unless replaced
    let jar = std::fs::read(
        std::path::Path::new(&info.path)
            .join("mods")
            .join(&enabled.file_name),
    )
    .unwrap();
    let again =
        block_on(admin.upload_mods(&uuid, &[(enabled.file_name.clone(), jar.clone())], false));
    assert!(
        matches!(again, Err(CoreError::Server { status: 409, .. })),
        "{again:?}"
    );
    let replaced =
        block_on(admin.upload_mods(&uuid, &[(enabled.file_name.clone(), jar)], true)).unwrap();
    assert_eq!(replaced.len(), 1);
    // players need nothing yet; with Sodium (client side) they do
    assert!(matches!(
        block_on(admin.client_pack(&uuid)),
        Err(CoreError::Server { status: 400, .. })
    ));
    let sodium =
        block_on(admin.modrinth_versions("sodium", Some("1.21.1"), Some("fabric"))).unwrap();
    let sodium = block_on(admin.install_mod(
        &uuid,
        &ModInstallRequest {
            version_id: sodium.last().expect("a Sodium version").id.clone(),
            install_dependencies: true,
            force: false,
        },
    ))
    .unwrap();
    let pack = block_on(admin.client_pack(&uuid)).unwrap();
    assert!(pack.file_name.ends_with(".mrpack"));
    assert!(
        !block_on(admin.download_file(&pack.key, 64 << 20))
            .unwrap()
            .is_empty()
    );
    block_on(admin.delete_mod(&uuid, &enabled.file_name)).unwrap();
    block_on(admin.delete_mod(&uuid, &sodium.installed[0].file_name)).unwrap();
    assert!(
        block_on(admin.installed_mods(&uuid, false))
            .unwrap()
            .mods
            .is_empty()
    );

    block_on(admin.delete_instance(&uuid)).unwrap();
}
