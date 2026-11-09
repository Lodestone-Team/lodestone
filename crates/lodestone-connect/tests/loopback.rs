//! A host and a friend on this computer, through an in-process introduction service:
//! the game stream reaches a TCP echo server, the info stream an HTTP server, and the
//! link keeps working after the invite ends and after the host restarts.

mod support {
    pub mod fake_service;
}

use std::{net::SocketAddr, time::Duration};

use lodestone_connect::{
    host::{Forward, Host, HostConfig, HostIdentity, HostState, MAX_PEERS, MAX_SPRAYS},
    join::{join, JoinOptions},
    ConnectionKind, Error, GatherOptions, Invite, Service, SharedServer, StreamKind,
};
use support::fake_service::FakeService;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
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

/// Answers every line with the same line.
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

fn server() -> SharedServer {
    SharedServer {
        core_name: "Test core".into(),
        core_uuid: "core".into(),
        instance_uuid: "instance".into(),
        name: "SMP".into(),
        game_version: "1.21.4".into(),
        loader: "fabric".into(),
        loader_version: Some("0.16.10".into()),
        modpack: true,
        state: Some("Running".into()),
        player_count: Some(1),
        max_player_count: Some(20),
    }
}

/// Answers any HTTP request with the server's info.
async fn info_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let mut got = Vec::new();
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = tcp.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    got.extend_from_slice(&buf[..n]);
                }
                let body = serde_json::to_string(&server()).unwrap();
                let _ = tcp
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            });
        }
    });
    addr
}

async fn echo_through(
    stream: &mut (impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin),
    text: &str,
) {
    stream.write_all(text.as_bytes()).await.unwrap();
    let mut answer = vec![0u8; text.len()];
    tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut answer))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(String::from_utf8(answer).unwrap(), text);
}

async fn wait_for(host: &Host, what: impl Fn(&lodestone_connect::host::HostStatus) -> bool) {
    for _ in 0..100 {
        if what(&host.status()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out; status: {:?}", host.status());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_friend_joins_plays_and_keeps_the_link() {
    let service = FakeService::start().await;
    let handler = Forward {
        game: echo_server().await,
        info: Some(info_server().await),
    };
    let config = HostConfig {
        service: Service::parse(&service.url()).unwrap(),
        gather: LOOPBACK,
        keep: true,
    };
    let host = Host::start(
        HostIdentity::generate().unwrap(),
        config.clone(),
        handler.clone(),
    )
    .await
    .unwrap();
    let invite = host.invite();
    let link = invite.link();
    assert!(link.starts_with(&format!("{}/j/", service.url())), "{link}");
    let status = host.status();
    assert_eq!(status.state, HostState::Sharing);
    assert!(status.stable_link);

    // Join with the link as people share it, and as the launcher's deep link has it.
    let joined = join(&Invite::parse(&invite.deep_link()).unwrap(), options())
        .await
        .unwrap();
    assert_eq!(joined.kind(), ConnectionKind::SameComputer);
    assert_eq!(joined.server_info().await.unwrap(), server());

    // A game stream reaches the echo server, directly and through a forwarded port.
    let mut game = joined.open(StreamKind::Game).await.unwrap();
    echo_through(&mut game, "hello\n").await;
    let forward = joined
        .forward(StreamKind::Game, "127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let mut tcp = TcpStream::connect(forward.local_addr()).await.unwrap();
    echo_through(&mut tcp, "through a local port\n").await;

    // A given port (the launcher keeps one per friend's server) works the same; while
    // it is open, a second forward can't take it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let fixed: SocketAddr = ([127, 0, 0, 1], port).into();
    let stable = joined.forward(StreamKind::Game, fixed).await.unwrap();
    assert_eq!(stable.local_addr(), fixed);
    let mut tcp = TcpStream::connect(fixed).await.unwrap();
    echo_through(&mut tcp, "through the usual port\n").await;
    assert!(joined.forward(StreamKind::Game, fixed).await.is_err());

    // An info port answers HTTP, as a core client would ask.
    let info = joined
        .forward(StreamKind::Info, "127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let mut http = TcpStream::connect(info.local_addr()).await.unwrap();
    http.write_all(b"GET /api/v1/connect/info HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut answer = Vec::new();
    http.read_to_end(&mut answer).await.unwrap();
    assert!(String::from_utf8_lossy(&answer).contains("\"name\":\"SMP\""));

    wait_for(&host, |s| s.joins == 1 && s.friends.len() == 1).await;
    assert_eq!(host.status().friends[0].kind, ConnectionKind::SameComputer);
    drop(game);
    joined.close().await;
    wait_for(&host, |s| s.friends.is_empty()).await;

    // The invite ends (its host went quiet): the host opens it again, same link.
    let code = invite.code.clone();
    service.end_quietly(&code);
    tokio::time::sleep(Duration::from_millis(300)).await;
    wait_for(&host, |s| s.state == HostState::Sharing).await;
    assert_eq!(host.invite().link(), link);
    let joined = join(&invite, options()).await.unwrap();
    echo_through(&mut joined.open(StreamKind::Game).await.unwrap(), "again\n").await;
    joined.close().await;

    // The host restarts with its saved identity: still the same link.
    let saved = HostIdentity::from_json(&host.identity().to_json()).unwrap();
    host.stop(false).await;
    let host = Host::start(saved, config.clone(), handler.clone())
        .await
        .unwrap();
    assert_eq!(host.invite().link(), link);
    let joined = join(&invite, options()).await.unwrap();
    echo_through(
        &mut joined.open(StreamKind::Game).await.unwrap(),
        "restarted\n",
    )
    .await;
    joined.close().await;

    // A link with the wrong token gets nowhere (the host's probes are tagged with
    // the token), one with the wrong certificate is refused by the friend.
    let mut wrong_token = invite.clone();
    wrong_token.token[0] ^= 1;
    let options_short = JoinOptions {
        probe_for: Duration::from_secs(2),
        ..options()
    };
    assert!(matches!(
        join(&wrong_token, options_short).await,
        Err(Error::NoPath(_))
    ));
    let mut wrong_cert = invite.clone();
    wrong_cert.fingerprint[0] ^= 1;
    assert!(matches!(
        join(&wrong_cert, options()).await,
        Err(Error::WrongHost)
    ));

    // Stopping for good ends the link.
    host.stop(true).await;
    let ended = join(&invite, options()).await.err().unwrap();
    assert!(ended.is_not_found(), "{ended}");
    assert!(service.joins(&code) == 0);
}

/// Anyone with the code can join and make the host probe the addresses they give. The
/// host probes only a few addresses for a few joins at once, however many there are.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn strangers_cannot_make_the_host_probe_many_addresses() {
    let service = FakeService::start().await;
    let handler = Forward {
        game: echo_server().await,
        info: None,
    };
    let config = HostConfig {
        service: Service::parse(&service.url()).unwrap(),
        gather: LOOPBACK,
        keep: false,
    };
    let host = Host::start(HostIdentity::generate().unwrap(), config, handler)
        .await
        .unwrap();
    let invite = host.invite();

    // 12 joins of 10 addresses each, none of them a friend.
    let mut targets = Vec::new();
    for _ in 0..12 {
        let sockets: Vec<std::net::UdpSocket> = (0..10)
            .map(|_| std::net::UdpSocket::bind("127.0.0.1:0").unwrap())
            .collect();
        let addrs: Vec<SocketAddr> = sockets.iter().map(|s| s.local_addr().unwrap()).collect();
        invite.service.join(&invite.code, &addrs).await.unwrap();
        targets.extend(sockets);
    }
    wait_for(&host, |s| s.joins == 12).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Which addresses get probes now?
    let mut buf = [0u8; 64];
    for socket in &targets {
        socket.set_nonblocking(true).unwrap();
        while socket.recv_from(&mut buf).is_ok() {}
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    let probed = targets
        .iter()
        .filter(|socket| socket.recv_from(&mut buf).is_ok())
        .count();
    assert!(probed > 0, "the host should probe the latest joins");
    assert!(
        probed <= MAX_SPRAYS * MAX_PEERS,
        "the host probes {probed} addresses"
    );
    host.stop(true).await;
}
