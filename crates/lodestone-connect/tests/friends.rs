//! Two friends on this computer, through the in-process introduction service: they pair
//! with a friend link, swap signed states through their slots, and Ben joins the server
//! Ana's core shares with him. Then the checks: a stolen core secret without Ben's key
//! gets nowhere, strangers get silence, and a removed friend is cut off.

#[allow(dead_code)]
mod support {
    pub mod fake_service;
}

use std::{
    net::{SocketAddr, UdpSocket},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lodestone_connect::{
    friend::{
        crypto::{core_secret, PersonKey, PublicKey},
        host::{CoreIdentity, FriendAccess, FriendHandler, FriendHost, FriendHostConfig},
        join::{is_online, join_friend, FriendCore},
        pair::{answer, open_card, FriendLink, Invitation},
        state::{
            open_state, seal_state, slot_id, slot_proof, slot_proof_hash, State, StateCore,
            STATE_LIFETIME_SECS,
        },
    },
    host::pipe,
    join::JoinOptions,
    Error, GatherOptions, Service, SharedServer, TunnelStream,
};
use support::fake_service::FakeService;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
};

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

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn echo_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let (mut read, mut write) = tcp.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    addr
}

/// Passes game streams for "smp" (shared with `friend` only) to an echo server, and
/// answers info streams with the server list.
struct Streams {
    friend: PublicKey,
    game: SocketAddr,
}

impl FriendHandler for Streams {
    fn game(&self, stream: TunnelStream, friend: PublicKey, instance: String) {
        if friend == self.friend && instance == "smp" {
            tokio::spawn(pipe(stream, self.game));
        }
    }

    fn info(&self, mut stream: TunnelStream, _friend: PublicKey) {
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let mut got = Vec::new();
            while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                got.extend_from_slice(&buf[..n]);
            }
            let body = serde_json::to_string(&vec![SharedServer {
                core_name: "Ana's PC".into(),
                core_uuid: "core".into(),
                instance_uuid: "smp".into(),
                name: "SMP".into(),
                game_version: "1.21.4".into(),
                loader: "fabric".into(),
                loader_version: None,
                modpack: false,
                state: Some("Running".into()),
                player_count: Some(0),
                max_player_count: Some(20),
            }])
            .unwrap();
            let _ = stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
            let _ = stream.shutdown().await;
        });
    }
}

async fn echo_through(joined: &lodestone_connect::friend::join::FriendJoined) -> Vec<u8> {
    let mut game = joined.open_game("smp").await.unwrap();
    game.write_all(b"hello\n").await.unwrap();
    let mut back = [0u8; 6];
    tokio::time::timeout(Duration::from_secs(10), game.read_exact(&mut back))
        .await
        .expect("echo in time")
        .unwrap();
    back.to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn friends_pair_and_join_without_links() {
    // CONNECT_TEST_SERVICE=http://127.0.0.1:8787 runs this against the real Worker
    // (`npm run dev` in services/connect) instead of the in-process stand-in.
    let fake = FakeService::start().await;
    let real = std::env::var("CONNECT_TEST_SERVICE").ok();
    let service = Service::parse(real.as_deref().unwrap_or(&fake.url())).unwrap();
    let ana = PersonKey::generate().unwrap();
    let ben = PersonKey::generate().unwrap();

    // --- pairing through the mailbox
    let (mut invitation, card) = Invitation::create(&ana, "Ana", service.clone(), now()).unwrap();
    let link = invitation.link.clone();
    invitation.mailbox_secret = Some(
        service
            .pair_open(&link.code, &link.access_hash(), &card)
            .await
            .unwrap(),
    );

    let opened = FriendLink::parse(&link.link()).unwrap();
    let sealed = service
        .pair_card(&opened.code, &opened.access())
        .await
        .unwrap()
        .expect("a card");
    let card = open_card(&opened, &sealed, now()).unwrap();
    assert_eq!(card.key, ana.public());
    let reply = answer(&ben, "Ben", &opened, &card).unwrap();
    assert_eq!(
        service
            .pair_reply(&opened.code, &opened.access(), &reply.sealed)
            .await
            .unwrap(),
        1
    );
    // the wrong access is refused
    assert!(service
        .pair_card(&opened.code, &"0".repeat(64))
        .await
        .is_err());

    let secret = invitation.mailbox_secret.clone().unwrap();
    let replies = service
        .pair_replies(&link.code, &secret)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replies.len(), 1);
    let checked = invitation.open_reply(&ana.public(), &replies[0]).unwrap();
    let pair = invitation.accept(&checked).unwrap();
    assert_eq!(pair, reply.pair);
    service.pair_close(&link.code, &secret).await.unwrap();
    assert!(service
        .pair_card(&opened.code, &opened.access())
        .await
        .unwrap()
        .is_none());

    // --- Ana's core, and her state naming it, through her slot for Ben
    let identity = CoreIdentity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let state = State {
        seq: 1,
        issued: now(),
        expires: now() + STATE_LIFETIME_SECS,
        name: "Ana".into(),
        cores: vec![StateCore {
            fingerprint,
            name: "Ana's PC".into(),
        }],
        retired: false,
    };
    let id = slot_id(&pair, &ana.public(), &ben.public());
    let proof = slot_proof(&pair, &ana.public(), &ben.public());
    assert_eq!(slot_proof_hash(&proof).len(), 64);
    service
        .slot_write(
            &id,
            &proof,
            &seal_state(&ana, &ben.public(), &pair, &state).unwrap(),
        )
        .await
        .unwrap();
    // Ben can't overwrite Ana's slot with a proof of his own
    let bens_proof = slot_proof(&pair, &ben.public(), &ana.public());
    assert!(service.slot_write(&id, &bens_proof, b"junk").await.is_err());
    let read = service.slot_read(&id).await.unwrap().unwrap();
    let got = open_state(&ana.public(), &ben.public(), &pair, &read).unwrap();
    assert_eq!(got.cores[0].fingerprint, fingerprint);

    // --- Ana's core shares with Ben
    let secret_for_ben = core_secret(&pair, &ana.public(), &ben.public(), &fingerprint);
    let host = FriendHost::start(
        &identity,
        vec![FriendAccess {
            key: ben.public(),
            secret: secret_for_ben.clone(),
        }],
        FriendHostConfig {
            service: service.clone(),
            gather: LOOPBACK,
        },
        Streams {
            friend: ben.public(),
            game: echo_server().await,
        },
    )
    .await
    .unwrap();
    let core = FriendCore {
        fingerprint: got.cores[0].fingerprint,
        secret: core_secret(&pair, &ana.public(), &ben.public(), &fingerprint),
    };
    for _ in 0..50 {
        if is_online(&service, &core).await.unwrap() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        is_online(&service, &core).await.unwrap(),
        "Ana's core shows as online"
    );

    // Strangers get silence: a QUIC handshake to the core before any knock gets no answer.
    let core_addr = host.status().candidates[0].addr;
    let stranger = quinn_attempt(core_addr, fingerprint).await;
    assert!(stranger.is_err(), "the core answered a stranger");
    // Not even QUIC's own answers: a version negotiation or a stateless reset would show
    // a server is here.
    assert_silent(core_addr);

    // --- Ben joins
    let joined = join_friend(&service, &ben, &core, options()).await.unwrap();
    let servers = joined.servers().await.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "SMP");
    assert_eq!(echo_through(&joined).await, b"hello\n");
    assert_eq!(host.status().connected.len(), 1);

    // --- Carl stole the core secret Ana's core shares with Ben, but not Ben's key: his
    // knocks get through, the hello doesn't.
    let carl = PersonKey::generate().unwrap();
    match join_friend(&service, &carl, &core, options()).await {
        Err(Error::Tunnel(message)) => assert!(message.contains("didn't accept"), "{message}"),
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("Carl got in"),
    }

    // --- Ana removes Ben: his connection closes, and his core shows offline
    host.set_friends(Vec::new());
    tokio::time::timeout(Duration::from_secs(10), joined.closed())
        .await
        .expect("Ben's connection closes");
    for _ in 0..50 {
        if !is_online(&service, &core).await.unwrap() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!is_online(&service, &core).await.unwrap());
    match join_friend(&service, &ben, &core, options()).await {
        Err(Error::Service { code, .. }) => assert_eq!(code, "host_offline"),
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("a removed friend got in"),
    }
    host.stop().await;
    if real.is_none() {
        assert!(fake.sessions().is_empty(), "stopping ends the session");
    }
}

/// When the service ends the core's session (it went quiet, or a day passed), the core
/// opens another and its friends can find it again.
#[tokio::test(flavor = "multi_thread")]
async fn a_core_publishes_again_after_its_session_ends() {
    let fake = FakeService::start().await;
    let service = Service::parse(&fake.url()).unwrap();
    let ana = PersonKey::generate().unwrap();
    let ben = PersonKey::generate().unwrap();
    let pair = lodestone_connect::friend::Secret::random().unwrap();
    let identity = CoreIdentity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let secret = core_secret(&pair, &ana.public(), &ben.public(), &fingerprint);
    let host = FriendHost::start(
        &identity,
        vec![FriendAccess {
            key: ben.public(),
            secret: secret.clone(),
        }],
        FriendHostConfig {
            service: service.clone(),
            gather: LOOPBACK,
        },
        Streams {
            friend: ben.public(),
            game: echo_server().await,
        },
    )
    .await
    .unwrap();
    let core = FriendCore {
        fingerprint,
        secret,
    };
    let online = |want: bool| {
        let service = service.clone();
        let core = core.clone();
        async move {
            for _ in 0..100 {
                if is_online(&service, &core).await.unwrap() == want {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            false
        }
    };
    assert!(online(true).await);
    let first = fake.sessions();
    assert_eq!(first.len(), 1);
    fake.end_session(&first[0]);
    assert!(online(true).await, "the core published again");
    let second = fake.sessions();
    assert_eq!(second.len(), 1);
    assert_ne!(second, first);
    let joined = join_friend(&service, &ben, &core, options()).await.unwrap();
    assert_eq!(
        echo_through(&joined).await,
        b"hello
"
    );
    host.stop().await;
}

/// Sends `addr` datagrams any QUIC server answers (an Initial with a version it doesn't
/// know, which gets a version negotiation; a short-header packet for no connection,
/// which gets a stateless reset) and checks nothing comes back.
fn assert_silent(addr: SocketAddr) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(1500)))
        .unwrap();
    // long header, version 0x1a2a3a4a, 8-byte connection ids, padded to 1200 bytes
    let mut unknown_version = vec![0xc0, 0x1a, 0x2a, 0x3a, 0x4a, 8];
    unknown_version.extend_from_slice(&[7; 8]);
    unknown_version.push(8);
    unknown_version.extend_from_slice(&[9; 8]);
    unknown_version.resize(1200, 0);
    // short header for a connection id the server never gave out
    let mut short = vec![0x40];
    short.extend(std::iter::repeat_n(0x5a, 99));
    for datagram in [&unknown_version, &short] {
        socket.send_to(datagram, addr).unwrap();
        let mut buf = [0u8; 2048];
        match socket.recv_from(&mut buf) {
            Ok((len, _)) => panic!("the core answered a stranger's datagram ({len} bytes)"),
            Err(err) => assert!(
                matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ),
                "{err}"
            ),
        }
    }
}

/// A bare QUIC handshake to `addr` pinning `fingerprint`, with a short timeout.
async fn quinn_attempt(addr: SocketAddr, fingerprint: [u8; 32]) -> Result<(), String> {
    let _ = fingerprint;
    raw_connect(addr).await.map(|_| ())
}

/// A bare QUIC connection to `addr` on the friend protocol, accepting any certificate,
/// that never says hello.
async fn raw_connect(addr: SocketAddr) -> Result<(quinn::Endpoint, quinn::Connection), String> {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut endpoint = quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        None,
        socket,
        Arc::new(quinn::TokioRuntime),
    )
    .unwrap();
    let crypto = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(AnyCert))
    .with_no_client_auth();
    let mut crypto = crypto;
    crypto.alpn_protocols = vec![b"lodestone-friend-1".to_vec()];
    endpoint.set_default_client_config(quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto).unwrap(),
    )));
    let attempt = endpoint.connect(addr, "lodestone-connect").unwrap();
    match tokio::time::timeout(Duration::from_secs(3), attempt).await {
        Ok(Ok(connection)) => Ok((endpoint, connection)),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("no answer".into()),
    }
}

#[derive(Debug)]
struct AnyCert;

impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Two friends share Ana's core. Removing one cuts off only them; the core refuses
/// invite streams and a second hello on a friend connection; unsharing disconnects; and
/// one friend can't take every connection.
#[tokio::test(flavor = "multi_thread")]
async fn one_friends_removal_and_misbehaviour_leave_the_others_alone() {
    let fake = FakeService::start().await;
    let service = Service::parse(&fake.url()).unwrap();
    let ana = PersonKey::generate().unwrap();
    let ben = PersonKey::generate().unwrap();
    let dee = PersonKey::generate().unwrap();
    let identity = CoreIdentity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let pair_ben = lodestone_connect::friend::Secret::random().unwrap();
    let pair_dee = lodestone_connect::friend::Secret::random().unwrap();
    let for_ben = core_secret(&pair_ben, &ana.public(), &ben.public(), &fingerprint);
    let for_dee = core_secret(&pair_dee, &ana.public(), &dee.public(), &fingerprint);
    let access = |key: &PersonKey, secret: &lodestone_connect::friend::Secret| FriendAccess {
        key: key.public(),
        secret: secret.clone(),
    };
    let host = FriendHost::start(
        &identity,
        vec![access(&ben, &for_ben), access(&dee, &for_dee)],
        FriendHostConfig {
            service: service.clone(),
            gather: LOOPBACK,
        },
        Streams {
            friend: dee.public(),
            game: echo_server().await,
        },
    )
    .await
    .unwrap();
    let ben_core = FriendCore {
        fingerprint,
        secret: for_ben.clone(),
    };
    let dee_core = FriendCore {
        fingerprint,
        secret: for_dee.clone(),
    };
    for _ in 0..50 {
        if is_online(&service, &dee_core).await.unwrap() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // an invite stream (kind 0) on a friend connection closes it
    let joined = join_friend(&service, &dee, &dee_core, options())
        .await
        .unwrap();
    let mut invite_kind = joined.open_raw(&[0u8; 17]).await.unwrap();
    let _ = invite_kind.write_all(b"x").await;
    tokio::time::timeout(Duration::from_secs(10), joined.closed())
        .await
        .expect("closed");

    // so does a second hello
    let joined = join_friend(&service, &dee, &dee_core, options())
        .await
        .unwrap();
    let mut again = joined.open_raw(&[3u8; 97]).await.unwrap();
    let _ = again.write_all(b"x").await;
    tokio::time::timeout(Duration::from_secs(10), joined.closed())
        .await
        .expect("closed");

    // one friend gets at most MAX_CONNECTIONS_PER_FRIEND connections
    let mut held = Vec::new();
    for _ in 0..lodestone_connect::friend::host::MAX_CONNECTIONS_PER_FRIEND {
        held.push(
            join_friend(&service, &dee, &dee_core, options())
                .await
                .unwrap(),
        );
    }
    assert!(join_friend(&service, &dee, &dee_core, options())
        .await
        .is_err());
    // ... and Ben still gets in
    let ben_joined = join_friend(&service, &ben, &ben_core, options())
        .await
        .unwrap();
    drop(held);

    // removing Ben closes his connection and stops his record; Dee is unaffected
    host.set_friends(vec![access(&dee, &for_dee)]);
    tokio::time::timeout(Duration::from_secs(10), ben_joined.closed())
        .await
        .expect("Ben is cut off");
    for _ in 0..50 {
        if !is_online(&service, &ben_core).await.unwrap() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!is_online(&service, &ben_core).await.unwrap());
    assert!(is_online(&service, &dee_core).await.unwrap());
    let dee_joined = join_friend(&service, &dee, &dee_core, options())
        .await
        .unwrap();
    assert_eq!(echo_through(&dee_joined).await, b"hello\n");

    // unsharing with Dee closes her connections at once
    host.disconnect(&dee.public());
    tokio::time::timeout(Duration::from_secs(10), dee_joined.closed())
        .await
        .expect("Dee is disconnected");
    host.stop().await;
}

/// Anyone at an address a friend's knock names can start a connection. Those that never
/// say hello count against that friend, so they can't take the places of the core's
/// other friends.
#[tokio::test(flavor = "multi_thread")]
async fn connections_without_a_hello_count_against_the_friend_who_knocked() {
    let fake = FakeService::start().await;
    let service = Service::parse(&fake.url()).unwrap();
    let ana = PersonKey::generate().unwrap();
    let ben = PersonKey::generate().unwrap();
    let dee = PersonKey::generate().unwrap();
    let identity = CoreIdentity::generate().unwrap();
    let fingerprint = identity.fingerprint();
    let pair = lodestone_connect::friend::Secret::random().unwrap();
    let for_ben = core_secret(&pair, &ana.public(), &ben.public(), &fingerprint);
    let for_dee = core_secret(&pair, &ana.public(), &dee.public(), &fingerprint);
    let host = FriendHost::start(
        &identity,
        vec![
            FriendAccess {
                key: ben.public(),
                secret: for_ben.clone(),
            },
            FriendAccess {
                key: dee.public(),
                secret: for_dee.clone(),
            },
        ],
        FriendHostConfig {
            service: service.clone(),
            gather: LOOPBACK,
        },
        Streams {
            friend: ben.public(),
            game: echo_server().await,
        },
    )
    .await
    .unwrap();
    let ben_core = FriendCore {
        fingerprint,
        secret: for_ben,
    };
    let dee_core = FriendCore {
        fingerprint,
        secret: for_dee,
    };
    for _ in 0..50 {
        if is_online(&service, &ben_core).await.unwrap() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Dee knocks and connects: her address may now connect
    let _dee_joined = join_friend(&service, &dee, &dee_core, options())
        .await
        .unwrap();
    // a crowd at her address opens connections that never say hello
    let core_addr = host.status().candidates[0].addr;
    let mut crowd = Vec::new();
    for _ in 0..lodestone_connect::host::MAX_UNPROVEN {
        if let Ok(held) = raw_connect(core_addr).await {
            crowd.push(held);
        }
    }
    assert_eq!(
        crowd.len(),
        lodestone_connect::friend::host::MAX_CONNECTIONS_PER_FRIEND - 1,
        "the crowd took more than Dee's places"
    );
    // Ben still gets in, and plays
    let ben_joined = join_friend(&service, &ben, &ben_core, options())
        .await
        .unwrap();
    assert_eq!(echo_through(&ben_joined).await, b"hello\n");
    assert_eq!(host.status().connected.len(), 2);
    host.stop().await;
}
