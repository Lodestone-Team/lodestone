//! Prototype: a direct tunnel for a Minecraft server between two computers behind NAT,
//! with no relay carrying the game. Version 2 tries several ways in, as Tailscale does:
//!
//! 1. One UDP socket for IPv4 and IPv6. Each side collects its candidate addresses:
//!    its address on the local network, its IPv6 address, the public address public
//!    STUN servers see ([`stun`]), and a port its router forwards on request (UPnP or
//!    NAT-PMP, [`portmap`]).
//! 2. The two sides swap tickets by hand. The host's ticket carries its candidates, the
//!    SHA-256 of its certificate and a random token; the friend's carries its candidates.
//! 3. Probing ([`probe`]): both send small tagged probes to every candidate of the other
//!    and answer the probes they get. A probe from an address that wasn't a candidate
//!    (a router that picks a new port per destination) becomes a candidate too. The
//!    first address the other side answers from is the path.
//! 4. Over the same socket, the friend opens a QUIC connection (encrypted, reliable) to
//!    the host on that path. Each TCP connection Minecraft makes to the friend's local
//!    port becomes a QUIC stream, which the host connects to its Minecraft server.
//!
//! Usage:
//!   holepunch host [--target 127.0.0.1:25565] [--via <https://service or ip:port>] [--no-portmap] [--local]
//!   holepunch join <invite or host ticket> [--listen 127.0.0.1:25566] [--no-portmap] [--local]
//!   holepunch rendezvous [--listen 0.0.0.0:8080]   (the introduction service, [`rendezvous`])
//!
//! With `--via`, the host registers with the introduction service and prints an invite
//! link; friends join with it and no tickets are swapped by hand.

mod portmap;
mod probe;
mod rendezvous;
mod stun;

use std::{
    io::{self, BufRead as _, Write as _},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use quinn::{Endpoint, EndpointConfig, TransportConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use sha2::{Digest as _, Sha256};

/// How long each side probes, and the friend tries to connect.
const WINDOW: Duration = Duration::from_secs(120);
const ALPN: &[u8] = b"lodestone-holepunch-2";
const SERVER_NAME: &str = "lodestone-holepunch";
const STUN_V4: [&str; 2] = ["stun.l.google.com:19302", "stun.cloudflare.com:3478"];
const STUN_V6: &str = "stun.l.google.com:19302";
const TOKEN_LEN: usize = 16;

type Error = Box<dyn std::error::Error + Send + Sync>;

/// The router mapping to remove when the program ends, however it ends.
static MAPPING: Mutex<Option<portmap::Mapping>> = Mutex::new(None);

fn remove_mapping() {
    if let Some(mapping) = MAPPING.lock().unwrap().take() {
        mapping.remove();
        println!("Removed the router's port forwarding.");
    }
}

/// The invite a host has open (service, code, secret), ended on Ctrl-C.
static INVITE: Mutex<Option<(rendezvous::Service, String, String)>> = Mutex::new(None);

/// Ends the open invite, if any, so friends don't try a host that has gone. Best effort.
async fn end_invite() {
    let open = INVITE.lock().unwrap().take();
    if let Some((service, code, secret)) = open {
        let path = format!("/v1/invites/{code}");
        let ended = tokio::time::timeout(
            Duration::from_secs(3),
            rendezvous::call(&service, "DELETE", &path, Some(&secret), None),
        )
        .await;
        if matches!(ended, Ok(Ok(_))) {
            println!("Ended the invite.");
        }
    }
}

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            remove_mapping();
            end_invite().await;
            std::process::exit(130);
        }
    });
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("host") => host(&args[1..]).await,
        Some("join") => join(&args[1..]).await,
        Some("rendezvous") => {
            let listen = flag(&args[1..], "--listen").unwrap_or_else(|| "0.0.0.0:8080".into());
            match listen.parse() {
                Ok(listen) => rendezvous::serve(listen).await,
                Err(err) => Err(err.into()),
            }
        }
        _ => {
            eprintln!(
                "usage:\n  holepunch host [--target 127.0.0.1:25565] [--via <https://service or ip:port>] [--no-portmap] [--local]\n  \
                 holepunch join <invite or host ticket> [--listen 127.0.0.1:25566] [--no-portmap] [--local]\n  \
                 holepunch rendezvous [--listen 0.0.0.0:8080]"
            );
            std::process::exit(2);
        }
    };
    remove_mapping();
    end_invite().await;
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn read_line(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

// ---------------------------------------------------------------------------
// Addresses

/// IPv4 addresses as themselves, not as `::ffff:a.b.c.d` (how a dual-stack socket
/// reports them).
pub fn canonical(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(addr.ip().to_canonical(), addr.port())
}

/// The form a dual-stack socket sends to: IPv4 as `::ffff:a.b.c.d`.
pub fn to_socket(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V4(v4) => {
            SocketAddr::V6(SocketAddrV6::new(v4.ip().to_ipv6_mapped(), v4.port(), 0, 0))
        }
        v6 => v6,
    }
}

fn dual_stack_socket() -> io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_only_v6(false)?;
    socket.bind(&SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)).into())?;
    Ok(socket.into())
}

/// The address this computer uses to reach `probe` (no packet is sent).
fn route_to(bind: SocketAddr, probe: SocketAddr) -> Option<IpAddr> {
    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(probe).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// This side's socket and the addresses the other side can try.
struct Side {
    socket: UdpSocket,
    candidates: Vec<SocketAddr>,
}

fn gather(local: bool, portmap: bool) -> Result<Side, Error> {
    let socket = dual_stack_socket()?;
    let port = socket.local_addr()?.port();
    if local {
        return Ok(Side {
            socket,
            candidates: vec![SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port)],
        });
    }
    let mut candidates = Vec::new();
    let mut add = |addr: SocketAddr, what: &str| {
        if !candidates.contains(&addr) {
            println!("  {what}: {addr}");
            candidates.push(addr);
        }
    };

    let lan_v4 = route_to("0.0.0.0:0".parse().unwrap(), "8.8.8.8:53".parse().unwrap());
    if let Some(ip) = lan_v4 {
        add(SocketAddr::new(ip, port), "local network");
    }
    let v6 = route_to(
        "[::]:0".parse().unwrap(),
        "[2001:4860:4860::8888]:53".parse().unwrap(),
    )
    .filter(|ip| matches!(ip, IpAddr::V6(v6) if !v6.is_unicast_link_local() && !v6.is_loopback()));
    if let Some(ip) = v6 {
        add(SocketAddr::new(ip, port), "IPv6");
    }

    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut seen = Vec::new();
    for server in STUN_V4 {
        match stun::mapped_address(&socket, server, false) {
            Ok(addr) => {
                println!("  {server} sees us as {addr}");
                seen.push(addr);
            }
            Err(err) => println!("  {server}: {err}"),
        }
    }
    if v6.is_some()
        && let Ok(addr) = stun::mapped_address(&socket, STUN_V6, true)
    {
        add(addr, "IPv6 as the internet sees it");
    }
    socket.set_read_timeout(None)?;
    if let Some(first) = seen.first().copied() {
        add(first, "public (STUN)");
        if seen.iter().any(|addr| addr.port() != first.port()) {
            println!(
                "  note: the STUN servers see different ports: this router picks a new port per\n  \
                 destination (\"symmetric NAT\"). The other side learns the real one from our probes."
            );
        }
    }

    if portmap && let Some(IpAddr::V4(local_ip)) = lan_v4 {
        match portmap::map(port, local_ip) {
            Ok(mapping)
                if portmap::is_private(match mapping.external.ip() {
                    IpAddr::V4(ip) => ip,
                    IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
                }) =>
            {
                println!(
                    "  router forwarding ({}): the router's own address {} is private, so it's behind\n  \
                     another router or the ISP's NAT (CGNAT); forwarding there doesn't help",
                    mapping.how,
                    mapping.external.ip()
                );
                mapping.remove();
            }
            Ok(mapping) => {
                add(
                    mapping.external,
                    &format!("router forwarding ({})", mapping.how),
                );
                *MAPPING.lock().unwrap() = Some(mapping);
            }
            Err(err) => println!("  router forwarding: not available ({err})"),
        }
    }
    if candidates.is_empty() {
        return Err("found no address to offer: is this computer online?".into());
    }
    Ok(Side { socket, candidates })
}

// ---------------------------------------------------------------------------
// Tickets: lhp2|host|<addr,addr>|<fingerprint>|<token>  and  lhp2|join|<addr,addr>

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn addrs(list: &[SocketAddr]) -> String {
    list.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_addrs(text: &str) -> Option<Vec<SocketAddr>> {
    let list: Option<Vec<SocketAddr>> = text.split(',').map(|a| a.parse().ok()).collect();
    list.filter(|list| !list.is_empty())
}

struct HostTicket {
    candidates: Vec<SocketAddr>,
    fingerprint: [u8; 32],
    token: [u8; TOKEN_LEN],
}

impl HostTicket {
    fn encode(&self) -> String {
        format!(
            "lhp2|host|{}|{}|{}",
            addrs(&self.candidates),
            hex(&self.fingerprint),
            hex(&self.token)
        )
    }

    fn decode(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('|');
        if (parts.next()?, parts.next()?) != ("lhp2", "host") {
            return None;
        }
        Some(Self {
            candidates: parse_addrs(parts.next()?)?,
            fingerprint: unhex(parts.next()?)?.try_into().ok()?,
            token: unhex(parts.next()?)?.try_into().ok()?,
        })
    }
}

fn join_ticket(candidates: &[SocketAddr]) -> String {
    format!("lhp2|join|{}", addrs(candidates))
}

fn parse_join_ticket(text: &str) -> Option<Vec<SocketAddr>> {
    parse_addrs(text.trim().strip_prefix("lhp2|join|")?)
}

fn transport() -> Arc<TransportConfig> {
    let mut transport = TransportConfig::default();
    // Keeps the routers' mappings open while nobody is playing.
    transport.keep_alive_interval(Some(Duration::from_secs(5)));
    transport.max_idle_timeout(Some(Duration::from_secs(30).try_into().expect("small")));
    Arc::new(transport)
}

pub fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// Keeps sending probes along the chosen path while QUIC takes over the socket, so the
/// routers keep it open and a peer still probing gets its answer.
fn keep_open(socket: &UdpSocket, peer: SocketAddr, tag: [u8; 8]) -> io::Result<()> {
    let socket = socket.try_clone()?;
    std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(30);
        while Instant::now() < until {
            let _ = socket.send_to(&probe::message(tag, probe::ACK), to_socket(peer));
            std::thread::sleep(Duration::from_millis(250));
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Host: the computer with the Minecraft server

async fn host(args: &[String]) -> Result<(), Error> {
    let target: SocketAddr = flag(args, "--target")
        .unwrap_or_else(|| "127.0.0.1:25565".into())
        .parse()?;
    let via = match flag(args, "--via") {
        Some(text) => Some(
            rendezvous::Service::parse(&text)
                .ok_or("--via takes the service: https://name or ip:port")?,
        ),
        None => None,
    };
    println!("Finding the ways in to this computer...");
    let side = gather(has(args, "--local"), !has(args, "--no-portmap"))?;

    let certified = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()])?;
    let cert = CertificateDer::from(certified.cert.der().to_vec());
    let fingerprint: [u8; 32] = Sha256::digest(&cert).into();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));
    let mut token = [0u8; TOKEN_LEN];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut token)
        .map_err(|_| "no randomness")?;
    let tag: [u8; 8] = token[..8].try_into().expect("16 bytes");

    let mut crypto = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut server = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
    ));
    server.transport_config(transport());

    let Some(service) = via else {
        // Tickets swapped by hand.
        let ticket = HostTicket {
            candidates: side.candidates.clone(),
            fingerprint,
            token,
        }
        .encode();
        println!("\nSend this ticket to your friend:\n\n  {ticket}\n");
        let peers = loop {
            let line = read_line("Paste your friend's ticket: ")?;
            match parse_join_ticket(&line) {
                Some(peers) => break peers,
                None => println!("  that isn't a join ticket (it starts with lhp2|join|)"),
            }
        };
        println!("Probing your friend's addresses: {}", addrs(&peers));
        let path = probe::run(&side.socket, &peers, tag, WINDOW).ok_or(
            "no probe got through in either direction. This pair of networks needs a relay",
        )?;
        println!("Path found: {path}. Waiting for your friend's connection...");
        keep_open(&side.socket, path, tag)?;
        side.socket.set_nonblocking(true)?;
        let endpoint = Endpoint::new(
            EndpointConfig::default(),
            Some(server),
            side.socket,
            Arc::new(quinn::TokioRuntime),
        )?;
        accept_loop(endpoint, token, target).await;
        return Ok(());
    };

    // Through the introduction service: an invite link, and any number of friends.
    let list: Vec<String> = side.candidates.iter().map(ToString::to_string).collect();
    let created = rendezvous::call(
        &service,
        "POST",
        "/v1/invites",
        None,
        Some(serde_json::json!({ "candidates": list })),
    )
    .await?;
    let code = created["code"]
        .as_str()
        .ok_or("no code from the service")?
        .to_string();
    let secret = created["secret"]
        .as_str()
        .ok_or("no secret from the service")?
        .to_string();
    *INVITE.lock().unwrap() = Some((service.clone(), code.clone(), secret.clone()));
    println!(
        "\nSend this invite to your friends (the part after # never goes to the service):\n\n  \
         lhp2://{}/{code}#{}.{}\n",
        service.link(),
        hex(&fingerprint),
        hex(&token)
    );
    let sprayer = side.socket.try_clone()?;
    side.socket.set_nonblocking(true)?;
    let endpoint = Endpoint::new(
        EndpointConfig::default(),
        Some(server),
        side.socket,
        Arc::new(quinn::TokioRuntime),
    )?;
    tokio::spawn(accept_loop(endpoint, token, target));

    println!("Waiting for friends to join...");
    let mut seen = 0usize;
    loop {
        let mut watch = match rendezvous::watch(&service, &code, &secret, seen).await {
            Ok(Some(watch)) => watch,
            Ok(None) => return Err("the invite has ended".into()),
            Err(err) => {
                println!("  {err}; retrying");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        loop {
            match watch.next().await {
                Ok(rendezvous::Event::Joins(joins)) => {
                    for (id, peers) in joins {
                        if id < seen {
                            continue;
                        }
                        seen = id + 1;
                        println!("A friend joined; punching towards {}", addrs(&peers));
                        probe::spray(&sprayer, peers, tag, Duration::from_secs(60))?;
                    }
                }
                Ok(rendezvous::Event::Ended) => {
                    if INVITE.lock().unwrap().take().is_none() {
                        // Ended by Ctrl-C, which is finishing up and exits.
                        std::future::pending::<()>().await;
                    }
                    return Err("the invite has ended".into());
                }
                Err(err) => {
                    println!("  lost the introduction service ({err}); reconnecting");
                    break;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn accept_loop(endpoint: Endpoint, token: [u8; TOKEN_LEN], target: SocketAddr) {
    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(connection) => connection,
                Err(err) => return println!("  a connection attempt failed: {err}"),
            };
            println!(
                "Connected directly to {} (round trip {} ms). Game traffic goes straight between you.",
                canonical(connection.remote_address()),
                connection.rtt().as_millis()
            );
            loop {
                let (send, mut recv) = match connection.accept_bi().await {
                    Ok(streams) => streams,
                    Err(err) => return println!("  connection closed: {err}"),
                };
                tokio::spawn(async move {
                    // Each stream starts with the token from the ticket.
                    let mut presented = [0u8; TOKEN_LEN];
                    if recv.read_exact(&mut presented).await.is_err()
                        || !bool::from(subtle::ConstantTimeEq::ct_eq(&presented[..], &token[..]))
                    {
                        return println!("  refused a stream without the right token");
                    }
                    match tokio::net::TcpStream::connect(target).await {
                        Ok(mut tcp) => {
                            let mut quic = tokio::io::join(recv, send);
                            let _ = tokio::io::copy_bidirectional(&mut tcp, &mut quic).await;
                        }
                        Err(err) => println!("  couldn't reach the server at {target}: {err}"),
                    }
                });
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Join: the friend's computer

/// `lhp2://<service>/<code>#<fingerprint>.<token>`
struct Invite {
    service: rendezvous::Service,
    code: String,
    fingerprint: [u8; 32],
    token: [u8; TOKEN_LEN],
}

impl Invite {
    fn parse(text: &str) -> Option<Self> {
        let rest = text.trim().strip_prefix("lhp2://")?;
        let (place, secret) = rest.split_once('#')?;
        let (service, code) = place.split_once('/')?;
        let (fingerprint, token) = secret.split_once('.')?;
        let code = code.trim_end_matches('/');
        if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        Some(Self {
            service: rendezvous::Service::from_link(service)?,
            code: code.to_string(),
            fingerprint: unhex(fingerprint)?.try_into().ok()?,
            token: unhex(token)?.try_into().ok()?,
        })
    }
}

async fn join(args: &[String]) -> Result<(), Error> {
    let first = args
        .first()
        .ok_or("pass the host's invite (lhp2://…) or ticket (lhp2|host|…), in quotes")?;
    let listen: SocketAddr = flag(args, "--listen")
        .unwrap_or_else(|| "127.0.0.1:25566".into())
        .parse()?;
    let invite = Invite::parse(first);
    let ticket = HostTicket::decode(first);
    let (fingerprint, token) = match (&invite, &ticket) {
        (Some(invite), _) => (invite.fingerprint, invite.token),
        (None, Some(ticket)) => (ticket.fingerprint, ticket.token),
        (None, None) => {
            return Err("that isn't an invite (lhp2://…) or a host ticket (lhp2|host|…)".into());
        }
    };
    let tag: [u8; 8] = token[..8].try_into().expect("16 bytes");

    println!("Finding the ways in to this computer...");
    let side = gather(has(args, "--local"), !has(args, "--no-portmap"))?;
    let host_candidates = match &invite {
        Some(invite) => {
            let list: Vec<String> = side.candidates.iter().map(ToString::to_string).collect();
            let answer = rendezvous::call(
                &invite.service,
                "POST",
                &format!("/v1/invites/{}/joins", invite.code),
                None,
                Some(serde_json::json!({ "candidates": list })),
            )
            .await?;
            rendezvous::addrs_of(&answer["host"])
        }
        None => {
            println!(
                "\nSend this ticket back to the host:\n\n  {}\n",
                join_ticket(&side.candidates)
            );
            ticket.map(|t| t.candidates).unwrap_or_default()
        }
    };

    let mut crypto = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCert {
            fingerprint,
            provider: provider(),
        }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut client = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
    ));
    client.transport_config(transport());

    println!(
        "Probing the host's addresses (up to {} s): {}",
        WINDOW.as_secs(),
        addrs(&host_candidates)
    );
    // With an invite, the host's side is already a QUIC server and only sends probes,
    // so hearing one is the answer.
    let path = if invite.is_some() {
        probe::run_until_heard(&side.socket, &host_candidates, tag, WINDOW)
    } else {
        probe::run(&side.socket, &host_candidates, tag, WINDOW)
    }
    .ok_or("no probe got through in either direction. This pair of networks needs a relay")?;
    println!("Path found: {path}. Connecting...");
    keep_open(&side.socket, path, tag)?;
    side.socket.set_nonblocking(true)?;
    let mut endpoint = Endpoint::new(
        EndpointConfig::default(),
        None,
        side.socket,
        Arc::new(quinn::TokioRuntime),
    )?;
    endpoint.set_default_client_config(client);

    let until = Instant::now() + Duration::from_secs(30);
    let connection = loop {
        let attempt = endpoint.connect(path, SERVER_NAME)?;
        match tokio::time::timeout(Duration::from_secs(5), attempt).await {
            Ok(Ok(connection)) => break connection,
            Ok(Err(err)) if err.to_string().contains("not the host from the ticket") => {
                return Err("the host's certificate doesn't match the invite. Ask for a fresh one; if it still fails, someone may be in the way".into());
            }
            Ok(Err(err)) => println!("  not yet: {err}"),
            Err(_) => println!("  not yet: no answer"),
        }
        if Instant::now() >= until {
            return Err("probes got through but the encrypted connection didn't".into());
        }
    };
    println!(
        "Connected directly to the host (round trip {} ms).\n\nIn Minecraft, add the server {listen} and join it.",
        connection.rtt().as_millis()
    );

    let listener = tokio::net::TcpListener::bind(listen).await?;
    loop {
        let (mut tcp, _) = listener.accept().await?;
        let connection = connection.clone();
        tokio::spawn(async move {
            let Ok((mut send, recv)) = connection.open_bi().await else {
                return println!("  the tunnel closed");
            };
            if send.write_all(&token).await.is_err() {
                return;
            }
            let mut quic = tokio::io::join(recv, send);
            let _ = tokio::io::copy_bidirectional(&mut tcp, &mut quic).await;
        });
    }
}

/// Accepts exactly the host's certificate, whose SHA-256 came in the ticket, and checks
/// the handshake's signature with it as usual.
#[derive(Debug)]
struct PinnedCert {
    fingerprint: [u8; 32],
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::client::danger::ServerCertVerifier for PinnedCert {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        let actual: [u8; 32] = Sha256::digest(end_entity).into();
        if bool::from(subtle::ConstantTimeEq::ct_eq(
            &actual[..],
            &self.fingerprint[..],
        )) {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(
                "not the host from the ticket".into(),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("TLS 1.2 isn't used".into()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_round_trip() {
        let host = HostTicket {
            candidates: vec![
                "203.0.113.5:54321".parse().unwrap(),
                "[2001:db8::1]:9".parse().unwrap(),
            ],
            fingerprint: [0xab; 32],
            token: [0x01; TOKEN_LEN],
        };
        let back = HostTicket::decode(&host.encode()).unwrap();
        assert_eq!(
            (back.candidates, back.fingerprint, back.token),
            (host.candidates.clone(), host.fingerprint, host.token)
        );
        let join = join_ticket(&host.candidates);
        assert_eq!(parse_join_ticket(&join), Some(host.candidates));
        assert!(HostTicket::decode(&join).is_none());
    }

    #[test]
    fn invites_parse() {
        let text = format!(
            "lhp2://16.146.26.94:8080/ABCDEFGH23#{}.{}",
            "ab".repeat(32),
            "01".repeat(16)
        );
        let invite = Invite::parse(&text).unwrap();
        assert_eq!(invite.service.link(), "16.146.26.94:8080");
        assert_eq!(invite.code, "ABCDEFGH23");
        assert_eq!(invite.fingerprint, [0xab; 32]);
        assert!(Invite::parse("lhp2://16.146.26.94:8080/AB/../x#00.00").is_none());
    }

    #[test]
    fn addresses_map_to_and_from_the_dual_stack_form() {
        let v4: SocketAddr = "203.0.113.5:7".parse().unwrap();
        assert!(to_socket(v4).is_ipv6());
        assert_eq!(canonical(to_socket(v4)), v4);
    }
}
