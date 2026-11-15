//! Settings › General, Users and Profile against the real `lodestone_core` program (work
//! package D): the core's settings and public listing, users, permissions, passwords,
//! API keys and Minecraft links, through the calls the pages make.
//!
//! Needs the core's program, as `core_child.rs` does; without it the test says so and
//! passes. It never starts a game server, and never opens a port on the router.

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
            UserPermission,
            core_settings::CoreSwitch,
            users::{ApiKeyScope, CreateApiKeyRequest, Permission},
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

fn free_port() -> u16 {
    (17_400..17_700)
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port")
}

#[test]
fn the_settings_pages_calls_work_on_a_real_core() {
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
        connect_service: CoreConfig::connect_service_now(),
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
        username: "owner".into(),
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
    let admin = session.admin.clone();

    // the setup key never reaches the core's output
    let log = std::fs::read_to_string(&config.log_file).unwrap_or_default();
    assert!(!log.contains("First time setup key"), "the key was logged");

    // Settings › General
    block_on(admin.set_core_name("Test core")).unwrap();
    block_on(admin.set_domain("play.example.com")).unwrap();
    block_on(admin.set_core_switch(CoreSwitch::ListInstancesPublicly, true)).unwrap();
    block_on(admin.set_core_switch(CoreSwitch::AllowLocalClaim, false)).unwrap();
    block_on(admin.set_core_switch(CoreSwitch::SafeMode, true)).unwrap();
    let settings = block_on(admin.global_settings()).unwrap();
    assert_eq!(settings.core_name, "Test core");
    assert_eq!(settings.domain.as_deref(), Some("play.example.com"));
    assert!(settings.list_instances_publicly && settings.safe_mode);
    assert!(!settings.allow_local_claim);
    let raw = block_on(admin.global_settings_raw()).unwrap();
    assert!(
        raw["allow_lan_http"].is_boolean(),
        "this core has the setting"
    );
    block_on(admin.set_core_switch(CoreSwitch::AllowLocalClaim, true)).unwrap();
    block_on(admin.set_core_switch(CoreSwitch::SafeMode, false)).unwrap();
    let overview = block_on(admin.public_listing_overview()).unwrap();
    assert!(overview.list_instances_publicly && overview.instances.is_empty());

    // Settings › Users
    let alice = block_on(admin.create_user("alice", "another test password 456")).unwrap();
    assert!(!alice.is_owner);
    let users = block_on(admin.users()).unwrap();
    assert_eq!(users.len(), 2);
    let mut permissions = UserPermission::default();
    Permission::CreateInstance.set_flag(&mut permissions, true);
    Permission::ViewInstance.set_instance(&mut permissions, "some-instance", true);
    block_on(admin.set_permissions(&alice.uid, &permissions)).unwrap();
    let alice_now = block_on(admin.users())
        .unwrap()
        .into_iter()
        .find(|u| u.uid == alice.uid)
        .unwrap();
    assert!(alice_now.permissions.can_create_instance);
    block_on(admin.rename_user(&alice.uid, "alice2")).unwrap();
    block_on(admin.change_password(&alice.uid, None, "a third test password 789")).unwrap();
    let alice_session = block_on(session::sign_in_with_password(
        http.clone(),
        config.local_url(),
        "alice2",
        &Secret::new(String::from("a third test password 789")),
    ))
    .expect("alice signs in with her new password");
    // her own password needs the old one
    assert!(
        block_on(
            alice_session
                .admin
                .change_password(&alice.uid, Some("wrong"), "x y z 1 2 3 4")
        )
        .is_err()
    );
    // only the owner lists users
    assert!(block_on(alice_session.admin.users()).is_err());

    // API keys
    let created = block_on(admin.create_api_key(&CreateApiKeyRequest {
        name: "CI".into(),
        expires_at: None,
        scope: ApiKeyScope::Full,
    }))
    .unwrap();
    assert!(created.key.starts_with("lsk_"));
    assert_eq!(created.info.scope, ApiKeyScope::Full);
    let mut scope = UserPermission::default();
    Permission::CreateInstance.set_flag(&mut scope, true);
    let alice_key = block_on(alice_session.admin.create_api_key(&CreateApiKeyRequest {
        name: "Bot".into(),
        expires_at: Some(chrono_now() + 86_400),
        scope: ApiKeyScope::Custom { permissions: scope },
    }))
    .unwrap();
    let own = block_on(admin.own_api_keys()).unwrap();
    assert_eq!(own.len(), 1);
    let hers = block_on(admin.user_api_keys(&alice.uid)).unwrap();
    assert_eq!(hers[0].id, alice_key.info.id);
    block_on(admin.revoke_api_key(Some(&alice.uid), &alice_key.info.id)).unwrap();
    block_on(admin.revoke_api_key(None, &created.info.id)).unwrap();
    assert!(block_on(admin.own_api_keys()).unwrap().is_empty());
    assert!(
        block_on(admin.user_api_keys(&alice.uid))
            .unwrap()
            .is_empty()
    );

    // Minecraft links: none yet, and removing an unknown one fails
    assert!(block_on(admin.own_minecraft_links()).unwrap().is_empty());
    assert!(block_on(admin.all_minecraft_links()).unwrap().is_empty());
    assert!(matches!(
        block_on(admin.unlink_minecraft("00000000-0000-0000-0000-000000000000", true)),
        Err(CoreError::NotFound(_) | CoreError::Server { .. } | CoreError::Forbidden(_))
    ));

    // another core, as the Cores dialog connects: over HTTPS, the core's own certificate
    // isn't trusted until the user trusts its fingerprint, then a password signs in
    let https = url::Url::parse(&format!("https://127.0.0.1:{}", config.port)).unwrap();
    let fingerprint = match block_on(CoreClient::new(http.clone(), https.clone()).info()) {
        Err(CoreError::Untrusted {
            fingerprint: Some(fingerprint),
            ..
        }) => fingerprint,
        other => panic!(
            "expected an untrusted certificate, got {:?}",
            other.map(|_| ())
        ),
    };
    let pin = launcher_core::cores::tls::CertificatePin::new("127.0.0.1", &fingerprint);
    let pinned = Arc::new(launcher_core::cores::tls::PinnedHttp::new(Some(pin)).unwrap());
    let remote = CoreClient::new(http.clone(), https).with_pinned(pinned);
    assert_eq!(block_on(remote.info()).unwrap().core_name, "Test core");
    let remote_session = block_on(session::sign_in_to(
        remote,
        "owner",
        &Secret::new(String::from("a long test password 123")),
    ))
    .expect("the owner signs in to the pinned core");
    assert!(remote_session.me.is_owner);

    // deleting a user
    block_on(admin.delete_user(&alice.uid)).unwrap();
    assert_eq!(block_on(admin.users()).unwrap().len(), 1);

    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
