//! Work package A's routes against the real `lodestone_core` program (built with
//! `cargo build -p lodestone_core --no-default-features`): the games, a setup
//! manifest, sharing, renaming, linked players and the monitor stream. Routes that need
//! the internet (Mojang's version list, Modrinth) are checked only when it answers.
//!
//! It creates no instance and never starts a game server. Without the core's program
//! it says so and passes.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{StreamExt as _, executor::block_on};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        admin::{
            instances::HandlerGameType,
            mods::{ModrinthProjectType, ModrinthSearch, ModrinthSearchIndex},
        },
        live::{LiveMessage, LiveStream, StreamKind},
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

#[test]
fn package_a_routes_against_a_real_core() {
    let program = match core_process::find_core() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let data = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let port = (17_400..17_700)
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port");
    let config = CoreConfig {
        program,
        data_dir: data.path().join("core"),
        port,
        log_file: logs.path().join("core.log"),
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
        username: "owner_a".into(),
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
    let admin = &session.admin;

    // the games the core offers, in its order
    let games = block_on(admin.games()).unwrap();
    assert!(games.contains(&HandlerGameType::MinecraftFabric));
    assert!(games.iter().all(|g| g.route().is_some()));

    // a setup manifest (Mojang's version list: needs the internet)
    match block_on(admin.setup_manifest(&HandlerGameType::MinecraftJavaVanilla)) {
        Ok(manifest) => {
            assert!(!manifest.setting_sections.is_empty());
            let ids: Vec<_> = manifest
                .setting_sections
                .iter()
                .flat_map(|s| s.settings.iter().map(|s| s.setting_id.as_str()))
                .collect();
            assert!(ids.contains(&"version"), "{ids:?}");
        }
        Err(err) => eprintln!("setup manifest not checked (offline?): {err}"),
    }

    // an instance that doesn't exist: refused, never created
    let missing = "no-such-instance";
    assert!(matches!(
        block_on(admin.share(missing)),
        Err(CoreError::NotFound(_) | CoreError::Forbidden(_) | CoreError::Server { .. })
    ));
    assert!(block_on(admin.set_instance_name(missing, "x")).is_err());
    assert!(block_on(admin.set_instance_description(missing, "x")).is_err());
    assert!(block_on(admin.instances()).unwrap().is_empty());

    // the owner sees every link: none yet
    assert!(
        block_on(admin.linked_players(true, "owner_a"))
            .unwrap()
            .is_empty()
    );

    // the monitor stream of an unknown instance ends without frames
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let stream = LiveStream::open(
        admin.core(),
        admin.token().clone(),
        missing,
        StreamKind::Monitor,
        tx,
    );
    let first = block_on(async {
        futures::select! {
            message = rx.next().fuse() => message,
            _ = futures::FutureExt::fuse(futures_timer(Duration::from_secs(15))) => None,
        }
    });
    assert!(
        !matches!(first, Some(LiveMessage::Frame(_))),
        "no reports for an unknown instance: {first:?}"
    );
    drop(stream);

    // Modrinth, through the core (needs the internet)
    let search = ModrinthSearch {
        query: "fabulously".into(),
        project_type: ModrinthProjectType::Modpack,
        game_version: None,
        loader: Some("fabric".into()),
        index: ModrinthSearchIndex::Relevance,
        offset: 0,
        limit: 5,
    };
    match block_on(admin.modrinth_search(&search)) {
        Ok(results) => assert!(results.hits.len() <= 5),
        Err(err) => eprintln!("Modrinth not checked (offline?): {err}"),
    }

    // done: stop the core
    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}

/// A timer without a runtime: a thread that sleeps.
fn futures_timer(duration: Duration) -> futures::channel::oneshot::Receiver<()> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(duration);
        let _ = tx.send(());
    });
    rx
}

use futures::FutureExt as _;

/// Not a test: makes a core data folder for screenshots, at `LODESTONE_FIXTURE_DIR`
/// (a temporary folder you choose), with an owner and one vanilla server made through
/// the API with the EULA not accepted. It downloads the server from Mojang and never
/// starts it. Run with `cargo test -p lodestone-host --test instances_api -- --ignored`.
#[test]
#[ignore]
fn make_a_screenshot_fixture() {
    let Some(dir) = std::env::var_os("LODESTONE_FIXTURE_DIR") else {
        eprintln!("set LODESTONE_FIXTURE_DIR");
        return;
    };
    let program = core_process::find_core().expect("the core's program");
    let logs = tempfile::tempdir().unwrap();
    let port = (17_700..18_000)
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port");
    let config = CoreConfig {
        program,
        data_dir: std::path::PathBuf::from(dir),
        port,
        log_file: logs.path().join("core.log"),
    };
    let mut process = CoreProcess::spawn(&config).expect("the core starts");
    let http = launcher_core::http::default_client().unwrap();
    let local = CoreClient::new(http.clone(), config.local_url());
    while block_on(local.info()).is_err() {
        std::thread::sleep(Duration::from_millis(250));
    }
    let owner = OwnerCredentials {
        username: "screenshots".into(),
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
    let admin = &session.admin;
    if block_on(admin.instances()).unwrap().is_empty() {
        use launcher_core::cores::admin::instances::SetupValue;
        let game = HandlerGameType::MinecraftJavaVanilla;
        let manifest = block_on(admin.setup_manifest(&game)).expect("the form");
        let value = SetupValue {
            name: "Survival SMP".into(),
            description: Some("A world for friends".into()),
            auto_start: false,
            restart_on_crash: true,
            setting_sections: SetupValue::sections(&manifest, |_, s| {
                s.default_value.clone().or_else(|| s.value.clone())
            }),
            accept_eula: false,
        };
        block_on(admin.create_instance(&game, &value)).expect("created");
        let started = Instant::now();
        while block_on(admin.instances()).unwrap().is_empty() {
            assert!(started.elapsed() < Duration::from_secs(600), "not made");
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}
