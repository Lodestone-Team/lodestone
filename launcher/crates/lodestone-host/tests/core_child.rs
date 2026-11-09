//! The Host's core as a child process, end to end, against the real `lodestone_core`
//! program: start it in a temporary data folder on a free port, wait until it answers,
//! set up its owner and sign in through the local claim, list the instances, and stop
//! it by closing its stdin.
//!
//! Needs the core's program: build it with `cargo build -p lodestone_core
//! --no-default-features` in the repository's root (or set `LODESTONE_HOST_CORE`).
//! Without it the test says so and passes. It never starts a game server.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{CoreClient, CoreError, admin::instances::Power, local_claim::OwnerCredentials},
    game::download::Sleep,
};
use lodestone_host::{
    core_process::{self, CoreConfig, CoreProcess},
    session::{self, SignInError},
};

fn sleep() -> Sleep {
    Arc::new(|duration| Box::pin(async move { std::thread::sleep(duration) }))
}

fn free_port() -> u16 {
    (17_100..17_400)
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port")
}

#[test]
fn the_host_runs_and_signs_in_to_its_own_core() {
    let program = match core_process::find_core() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };
    let data = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let config = CoreConfig {
        program,
        data_dir: data.path().join("core"),
        port: free_port(),
        log_file: logs.path().join("core.log"),
    };
    let mut process = CoreProcess::spawn(&config).expect("the core starts");
    let http = launcher_core::http::default_client().unwrap();
    let local = CoreClient::new(http.clone(), config.local_url());

    // it answers
    let started = Instant::now();
    let info = loop {
        if let Some(status) = process.try_wait() {
            panic!(
                "the core exited ({status}): {:?}",
                core_process::log_tail(&config.log_file, 20)
            );
        }
        if let Ok(info) = block_on(local.info()) {
            break info;
        }
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "the core didn't answer: {:?}",
            core_process::log_tail(&config.log_file, 20)
        );
        std::thread::sleep(Duration::from_millis(250));
    };
    assert!(!info.is_setup, "a new core has no owner");
    assert_eq!(
        info.launched_by,
        launcher_core::cores::LaunchedBy::Host,
        "--is-host"
    );

    // a core without an owner asks for one
    let first = block_on(session::claim_owner(
        http.clone(),
        config.local_url(),
        &config.data_dir,
        None,
        sleep(),
    ));
    assert_eq!(first.err(), Some(SignInError::NeedsOwner));

    // the first run makes the owner, and signs in as them
    let owner = OwnerCredentials {
        username: "host_owner".into(),
        password: Secret::new(String::from("a long test password 123")),
    };
    let session = block_on(session::claim_owner(
        http.clone(),
        config.local_url(),
        &config.data_dir,
        Some(owner),
        sleep(),
    ))
    .expect("the claim signs in");
    assert!(session.me.is_owner);
    assert_eq!(session.me.username, "host_owner");
    assert!(session.info.is_setup);
    assert!(
        !session.info.capabilities.macros,
        "the Host's core is built without Deno"
    );
    assert!(
        session.admin.core().is_strict(),
        "the token only goes over HTTPS"
    );
    assert!(block_on(session.admin.instances()).unwrap().is_empty());
    // nothing to start: an unknown instance is refused, and no server starts
    assert!(matches!(
        block_on(session.admin.power("no-such-instance", Power::Start)),
        Err(CoreError::NotFound(_) | CoreError::Server { .. } | CoreError::Forbidden(_))
    ));
    // the claim's files are gone: nothing secret is left in the data folder
    let claims = config.data_dir.join("local_claim");
    let left: Vec<_> = std::fs::read_dir(&claims)
        .map(|dir| dir.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "claim folders left: {left:?}");

    // later starts sign in without a password
    let again = block_on(session::claim_owner(
        http.clone(),
        config.local_url(),
        &config.data_dir,
        None,
        sleep(),
    ))
    .expect("the owner signs in again");
    assert_eq!(again.me.uid, session.me.uid);

    // and with the password, as the web dashboard does
    let with_password = block_on(session::sign_in_with_password(
        http,
        config.local_url(),
        "host_owner",
        &Secret::new(String::from("a long test password 123")),
    ))
    .expect("the password signs in");
    assert!(with_password.me.is_owner);

    // closing stdin stops the core
    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}
