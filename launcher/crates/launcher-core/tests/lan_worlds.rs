//! Sharing an open LAN world with a friend (launcher/docs/friends.md, "LAN worlds"):
//! two friend lists, in temp folders, become friends through an introduction service;
//! Ana turns on sharing and opens a (fake) world to LAN; Ben sees her launcher in her
//! state, sees the world, joins it through a tunnel and exchanges bytes with it. Then
//! hiding from Ben cuts him off, and closing the world takes Ana's launcher offline.
//!
//! It runs against Lodestone Connect's in-process stand-in for the service, or against
//! the Worker at CONNECT_TEST_SERVICE (`npm run dev` in services/connect, or the dev
//! Worker). Both sides are on this computer, so they offer only loopback addresses.

// formatted by the root workspace (edition 2021), not this one
#[rustfmt::skip]
#[allow(dead_code)]
#[path = "../../../../crates/lodestone-connect/tests/support/fake_service.rs"]
mod fake_service;

use std::{
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};

use launcher_core::{
    MemorySecretStore,
    cores::lan_open::{OpenWorld, OpenWorlds, Status},
    friend_list::{
        FriendStatus, Friends,
        lan::{LAN_SOURCE, LanHost, LanHostConfig, LanHostState, lan_friends},
    },
    paths::DataDir,
};
use lodestone_connect::{
    GatherOptions, Service,
    friend::join::{is_online, join_friend},
    join::JoinOptions,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

const LOOPBACK: GatherOptions = GatherOptions {
    portmap: false,
    loopback_only: true,
};

fn options() -> JoinOptions {
    JoinOptions {
        gather: LOOPBACK,
        probe_for: Duration::from_secs(10),
        connect_for: Duration::from_secs(10),
    }
}

/// Stands in for the world's game port: echoes what arrives.
async fn fake_game_port() -> u16 {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = tcp.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}

/// Waits until `check` says yes, for up to `within`.
async fn eventually(within: Duration, mut check: impl AsyncFnMut() -> bool) -> bool {
    let until = Instant::now() + within;
    while Instant::now() < until {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lan_world_is_shared_with_a_friend_through_a_tunnel() {
    let fake = fake_service::FakeService::start().await;
    let real = std::env::var("CONNECT_TEST_SERVICE").ok();
    let service = Service::parse(real.as_deref().unwrap_or(&fake.url())).unwrap();
    let (ana_dir, ben_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (ana_keys, ben_keys) = (MemorySecretStore::new(), MemorySecretStore::new());
    let mut ana = Friends::create(&DataDir::at(ana_dir.path()), &ana_keys, service.clone())
        .await
        .unwrap();
    let mut ben = Friends::create(&DataDir::at(ben_dir.path()), &ben_keys, service.clone())
        .await
        .unwrap();
    ana.set_name("Ana").unwrap();
    ben.set_name("Ben").unwrap();

    // --- they become friends
    let link = ana.create_link().await.unwrap();
    let ana_key = ben.answer_link(&link).await.unwrap();
    let replies = ana.link_replies().await.unwrap();
    assert_eq!(replies.len(), 1);
    ana.accept_reply(&replies[0].code, &replies[0].key)
        .await
        .unwrap();
    ben.refresh().await.unwrap();
    let ben_key = ben.me().to_hex();
    ana.refresh().await.unwrap();
    assert_eq!(
        ana.book.friend(&ben_key).unwrap().status,
        FriendStatus::Active
    );

    // --- Ana turns on sharing: her launcher is listed in her state, as a launcher
    let (identity, changed) = ana.start_lan_sharing().unwrap();
    assert!(changed);
    let fingerprint = identity.fingerprint();
    assert_eq!(
        ana.lan_launcher().unwrap().source.as_deref(),
        Some(LAN_SOURCE)
    );
    // the same certificate next time
    assert_eq!(ana.lan_identity(false).unwrap().fingerprint(), fingerprint);
    assert!(
        !ana.start_lan_sharing().unwrap().1,
        "nothing new to publish"
    );
    assert!(ana.publish_states().await.unwrap().is_empty());
    ben.refresh().await.unwrap();
    let friend = ben.book.friend(&ana_key).unwrap().clone();
    assert!(friend.is_launcher(&fingerprint));
    assert!(
        friend
            .joinable_cores(launcher_core::friend_list::now())
            .iter()
            .any(|c| c.fingerprint == fingerprint)
    );
    let target = ben.friend_core(&friend, &fingerprint).unwrap();
    // no world open: nothing publishes, so Ben sees her launcher offline
    assert!(!is_online(&service, &target).await.unwrap());

    // --- Ana opens a world to LAN: its port answers pings (here, the tracker is told)
    let port = fake_game_port().await;
    let mut worlds = OpenWorlds::default();
    let now = Instant::now();
    worlds.announced(port, Some("Ana - New World".into()), now);
    worlds.checked(
        port,
        Some(Status {
            version: Some("1.21.4".into()),
            motd: Some("Ana - New World".into()),
            online: Some(1),
            max: Some(8),
            ..Status::default()
        }),
        now,
    );
    let open: Vec<OpenWorld> = worlds.open();
    assert_eq!(open.len(), 1);
    let access = lan_friends(&ana.book, &ana.me(), &fingerprint);
    assert_eq!(access.len(), 1, "Ben, who accepted and isn't hidden");
    let host = LanHost::start(
        identity,
        access,
        open.clone(),
        ana.book.name.clone(),
        LanHostConfig {
            service: service.clone(),
            gather: LOOPBACK,
            knocks: Default::default(),
        },
    )
    .unwrap();
    assert!(
        eventually(Duration::from_secs(20), async || {
            is_online(&service, &target).await.unwrap_or(false)
        })
        .await,
        "Ben sees the world open"
    );
    assert!(matches!(host.state(), LanHostState::Sharing { .. }));

    // --- Ben lists the world and joins it
    let me = lodestone_connect::friend::PersonKey::from_pkcs8(ben.person_key().pkcs8()).unwrap();
    let joined = join_friend(&service, &me, &target, options())
        .await
        .unwrap();
    let servers = joined.servers().await.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].instance_uuid, open[0].id);
    assert_eq!(servers[0].name, "Ana - New World");
    assert_eq!(servers[0].game_version, "1.21.4");
    assert_eq!(servers[0].core_name, "Ana");
    let mut game = joined.open_game(&open[0].id).await.unwrap();
    game.write_all(b"hello, world").await.unwrap();
    let mut echoed = [0u8; 12];
    tokio::time::timeout(Duration::from_secs(10), game.read_exact(&mut echoed))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&echoed, b"hello, world");
    // a forwarded port, as Minecraft uses it (Magic localhost)
    let forward = joined
        .forward(
            Some(&open[0].id),
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        )
        .await
        .unwrap();
    let mut tcp = tokio::net::TcpStream::connect(forward.local_addr())
        .await
        .unwrap();
    tcp.write_all(b"ping").await.unwrap();
    let mut back = [0u8; 4];
    tokio::time::timeout(Duration::from_secs(10), tcp.read_exact(&mut back))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&back, b"ping");
    // a world that isn't open gets nothing
    let mut other = joined.open_game("lan-000000000000").await.unwrap();
    let _ = other.write_all(b"x").await;
    let mut nothing = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), other.read(&mut nothing)).await;
    assert!(matches!(read, Ok(Ok(0)) | Ok(Err(_)) | Err(_)));
    drop((tcp, forward, game, other));

    // --- Ana hides from Ben: he's cut off at once
    ana.set_hidden(&ben_key, true).unwrap();
    let access = lan_friends(&ana.book, &ana.me(), &fingerprint);
    assert!(access.is_empty());
    host.set_friends(access);
    assert!(
        eventually(Duration::from_secs(10), async || joined.is_closed()).await,
        "Ben's tunnel closes"
    );
    assert!(
        join_friend(&service, &me, &target, options())
            .await
            .is_err()
    );
    ana.set_hidden(&ben_key, false).unwrap();
    host.set_friends(lan_friends(&ana.book, &ana.me(), &fingerprint));

    // --- the world closes: the host stops, and the launcher is offline again
    drop(host);
    assert!(
        eventually(Duration::from_secs(20), async || {
            !is_online(&service, &target).await.unwrap_or(true)
        })
        .await,
        "Ben sees no world open"
    );

    // --- Ana turns sharing off: her state no longer lists her launcher
    assert!(ana.stop_lan_sharing().unwrap());
    assert!(ana.lan_launcher().is_none());
    ana.publish_states().await.unwrap();
    let refreshed = ben.refresh().await.unwrap();
    assert!(
        refreshed
            .dropped_cores
            .contains(&launcher_core::friend_list::hex(&fingerprint))
    );
    let friend = ben.book.friend(&ana_key).unwrap();
    assert!(!friend.is_launcher(&fingerprint));
    // and turning it on again lists the same certificate
    let (again, _) = ana.start_lan_sharing().unwrap();
    assert_eq!(again.fingerprint(), fingerprint);
}
