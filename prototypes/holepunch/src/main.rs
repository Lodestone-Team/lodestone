//! Prototype: a direct tunnel for a Minecraft server between two computers behind NAT,
//! with no relay carrying the game.
//!
//! 1. Each side binds one UDP socket and asks public STUN servers what address the
//!    internet sees for it ([`stun`]). Two servers are asked: if they see different
//!    ports, the NAT is "symmetric" and punching will most likely fail.
//! 2. The two sides swap short tickets by hand (chat, paste). The host's ticket carries
//!    its public address, the SHA-256 of its self-signed certificate (so the friend
//!    knows it's talking to the right host) and a random token (so the host knows it's
//!    talking to the right friend). The friend's ticket carries its public address.
//! 3. Both send small UDP packets to each other's public address for a minute. Each
//!    side's router sees outgoing packets to the other and lets the other's packets in.
//! 4. Over the same socket, the friend opens a QUIC connection (encrypted, reliable)
//!    to the host. Each TCP connection Minecraft makes to the friend's local port
//!    becomes a QUIC stream, which the host connects to its Minecraft server.
//!
//! Usage:
//!   holepunch host [--target 127.0.0.1:25565] [--local]
//!   holepunch join <host ticket> [--listen 127.0.0.1:25566] [--local]
//!
//! `--local` skips STUN and uses 127.0.0.1, to try the tunnel on one computer.

use std::{
    io::{self, BufRead as _, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
    time::{Duration, Instant},
};

use quinn::{Endpoint, EndpointConfig, TransportConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use sha2::{Digest as _, Sha256};

/// How long each side keeps punching and the friend keeps trying to connect.
const PUNCH_FOR: Duration = Duration::from_secs(90);
const PUNCH_EVERY: Duration = Duration::from_millis(250);
const ALPN: &[u8] = b"lodestone-holepunch-1";
const SERVER_NAME: &str = "lodestone-holepunch";
const STUN_SERVERS: [&str; 2] = ["stun.l.google.com:19302", "stun.cloudflare.com:3478"];
const TOKEN_LEN: usize = 16;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("host") => host(&args[1..]).await,
        Some("join") => join(&args[1..]).await,
        _ => {
            eprintln!(
                "usage:\n  holepunch host [--target 127.0.0.1:25565] [--local]\n  holepunch join <host ticket> [--listen 127.0.0.1:25566] [--local]"
            );
            std::process::exit(2);
        }
    };
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
// The public address

/// Binds the UDP socket both STUN and QUIC use, and finds its public address.
fn bind_and_discover(local: bool) -> Result<(UdpSocket, SocketAddr), Error> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    let port = socket.local_addr()?.port();
    if local {
        return Ok((
            socket,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        ));
    }
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut seen = Vec::new();
    for server in STUN_SERVERS {
        match stun::mapped_address(&socket, server) {
            Ok(addr) => {
                println!("  {server} sees us as {addr}");
                seen.push(addr);
            }
            Err(err) => println!("  {server}: {err}"),
        }
    }
    socket.set_read_timeout(None)?;
    let Some(first) = seen.first().copied() else {
        return Err("no STUN server answered: is UDP blocked on this network?".into());
    };
    if seen.iter().any(|addr| addr.port() != first.port()) {
        println!(
            "  warning: the STUN servers see different ports, so this network's NAT is \"symmetric\".\n  \
             Punching through it usually fails unless the other side's NAT is friendly."
        );
    } else if first.port() == port {
        println!(
            "  the public port matches the local one: this NAT keeps ports (good for punching)"
        );
    }
    Ok((socket, first))
}

mod stun {
    //! The smallest STUN client (RFC 8489): one Binding request, and the
    //! XOR-MAPPED-ADDRESS from the answer.

    use super::Error;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};

    const COOKIE: u32 = 0x2112_A442;

    pub fn mapped_address(socket: &UdpSocket, server: &str) -> Result<SocketAddr, Error> {
        let server = server
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or("no IPv4 address")?;
        let mut txid = [0u8; 12];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut txid)
            .map_err(|_| "no randomness")?;
        let mut request = Vec::with_capacity(20);
        request.extend_from_slice(&0x0001u16.to_be_bytes()); // Binding request
        request.extend_from_slice(&0u16.to_be_bytes()); // no attributes
        request.extend_from_slice(&COOKIE.to_be_bytes());
        request.extend_from_slice(&txid);
        let mut buf = [0u8; 512];
        for _ in 0..3 {
            socket.send_to(&request, server)?;
            let Ok((len, from)) = socket.recv_from(&mut buf) else {
                continue;
            };
            if from != server || len < 20 || buf[8..20] != txid {
                continue;
            }
            if let Some(addr) = parse(&buf[..len], &txid) {
                return Ok(addr);
            }
        }
        Err("no answer".into())
    }

    fn parse(msg: &[u8], txid: &[u8; 12]) -> Option<SocketAddr> {
        if u16::from_be_bytes([msg[0], msg[1]]) != 0x0101 {
            return None; // not a Binding success response
        }
        let mut at = 20;
        while at + 4 <= msg.len() {
            let kind = u16::from_be_bytes([msg[at], msg[at + 1]]);
            let len = u16::from_be_bytes([msg[at + 2], msg[at + 3]]) as usize;
            let value = msg.get(at + 4..at + 4 + len)?;
            if (kind == 0x0020 || kind == 0x0001) && value.len() >= 8 {
                let xor = kind == 0x0020;
                let port = u16::from_be_bytes([value[2], value[3]])
                    ^ if xor { (COOKIE >> 16) as u16 } else { 0 };
                let ip = match value[1] {
                    0x01 => {
                        let raw = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
                        IpAddr::V4(Ipv4Addr::from(if xor { raw ^ COOKIE } else { raw }))
                    }
                    0x02 if value.len() >= 20 => {
                        let mut raw = [0u8; 16];
                        raw.copy_from_slice(&value[4..20]);
                        if xor {
                            let mut key = [0u8; 16];
                            key[..4].copy_from_slice(&COOKIE.to_be_bytes());
                            key[4..].copy_from_slice(txid);
                            for (byte, k) in raw.iter_mut().zip(key) {
                                *byte ^= k;
                            }
                        }
                        IpAddr::V6(Ipv6Addr::from(raw))
                    }
                    _ => return None,
                };
                return Some(SocketAddr::new(ip, port));
            }
            at += 4 + len.div_ceil(4) * 4;
        }
        None
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_an_xor_mapped_ipv4_address() {
            let txid = [7u8; 12];
            // 203.0.113.5:54321
            let port = 54321u16 ^ (COOKIE >> 16) as u16;
            let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ COOKIE;
            let mut msg = vec![0x01, 0x01, 0, 12];
            msg.extend_from_slice(&COOKIE.to_be_bytes());
            msg.extend_from_slice(&txid);
            msg.extend_from_slice(&[0x00, 0x20, 0, 8, 0, 0x01]);
            msg.extend_from_slice(&port.to_be_bytes());
            msg.extend_from_slice(&ip.to_be_bytes());
            assert_eq!(
                parse(&msg, &txid),
                Some("203.0.113.5:54321".parse().unwrap())
            );
        }
    }
}

/// Sends small packets to `peer` for [`PUNCH_FOR`], from a clone of the QUIC socket,
/// so this side's router lets the peer's packets in. QUIC ignores them on arrival.
fn punch(socket: &UdpSocket, peer: SocketAddr) -> io::Result<()> {
    let socket = socket.try_clone()?;
    std::thread::spawn(move || {
        let until = Instant::now() + PUNCH_FOR;
        while Instant::now() < until {
            let _ = socket.send_to(b"lodestone-punch", peer);
            std::thread::sleep(PUNCH_EVERY);
        }
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Tickets

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

struct HostTicket {
    addr: SocketAddr,
    fingerprint: [u8; 32],
    token: [u8; TOKEN_LEN],
}

impl HostTicket {
    fn encode(&self) -> String {
        format!(
            "lhp1|host|{}|{}|{}",
            self.addr,
            hex(&self.fingerprint),
            hex(&self.token)
        )
    }

    fn decode(text: &str) -> Option<Self> {
        let mut parts = text.trim().split('|');
        if (parts.next()?, parts.next()?) != ("lhp1", "host") {
            return None;
        }
        Some(Self {
            addr: parts.next()?.parse().ok()?,
            fingerprint: unhex(parts.next()?)?.try_into().ok()?,
            token: unhex(parts.next()?)?.try_into().ok()?,
        })
    }
}

fn join_ticket(addr: SocketAddr) -> String {
    format!("lhp1|join|{addr}")
}

fn parse_join_ticket(text: &str) -> Option<SocketAddr> {
    text.trim().strip_prefix("lhp1|join|")?.parse().ok()
}

fn transport() -> Arc<TransportConfig> {
    let mut transport = TransportConfig::default();
    // Keeps the routers' mappings open while nobody is playing.
    transport.keep_alive_interval(Some(Duration::from_secs(5)));
    transport.max_idle_timeout(Some(Duration::from_secs(30).try_into().expect("small")));
    Arc::new(transport)
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

// ---------------------------------------------------------------------------
// Host: the computer with the Minecraft server

async fn host(args: &[String]) -> Result<(), Error> {
    let target: SocketAddr = flag(args, "--target")
        .unwrap_or_else(|| "127.0.0.1:25565".into())
        .parse()?;
    let local = has(args, "--local");

    println!("Finding this computer's public address...");
    let (socket, public) = bind_and_discover(local)?;

    let certified = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()])?;
    let cert = CertificateDer::from(certified.cert.der().to_vec());
    let fingerprint: [u8; 32] = Sha256::digest(&cert).into();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));
    let mut token = [0u8; TOKEN_LEN];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut token)
        .map_err(|_| "no randomness")?;

    let mut crypto = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut server = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(crypto)?,
    ));
    server.transport_config(transport());

    let ticket = HostTicket {
        addr: public,
        fingerprint,
        token,
    }
    .encode();
    println!("\nSend this ticket to your friend:\n\n  {ticket}\n");
    let peer = loop {
        let line = read_line("Paste your friend's ticket: ")?;
        match parse_join_ticket(&line) {
            Some(addr) => break addr,
            None => println!("  that isn't a join ticket (it starts with lhp1|join|)"),
        }
    };

    socket.set_nonblocking(true)?;
    punch(&socket, peer)?;
    let endpoint = Endpoint::new(
        EndpointConfig::default(),
        Some(server),
        socket,
        Arc::new(quinn::TokioRuntime),
    )?;
    println!("Punching towards {peer}; waiting for your friend to connect...");

    while let Some(incoming) = endpoint.accept().await {
        let token = token;
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(connection) => connection,
                Err(err) => return println!("  a connection attempt failed: {err}"),
            };
            println!(
                "Connected directly to {} (round trip {} ms). Game traffic goes straight between you.",
                connection.remote_address(),
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Join: the friend's computer

async fn join(args: &[String]) -> Result<(), Error> {
    let ticket = args
        .first()
        .and_then(|text| HostTicket::decode(text))
        .ok_or("pass the host's ticket (it starts with lhp1|host|), in quotes")?;
    let listen: SocketAddr = flag(args, "--listen")
        .unwrap_or_else(|| "127.0.0.1:25566".into())
        .parse()?;
    let local = has(args, "--local");

    println!("Finding this computer's public address...");
    let (socket, public) = bind_and_discover(local)?;
    println!(
        "\nSend this ticket back to the host:\n\n  {}\n",
        join_ticket(public)
    );

    let mut crypto = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCert {
            fingerprint: ticket.fingerprint,
            provider: provider(),
        }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut client = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
    ));
    client.transport_config(transport());

    socket.set_nonblocking(true)?;
    punch(&socket, ticket.addr)?;
    let mut endpoint = Endpoint::new(
        EndpointConfig::default(),
        None,
        socket,
        Arc::new(quinn::TokioRuntime),
    )?;
    endpoint.set_default_client_config(client);

    println!(
        "Punching towards {} and connecting (up to {} s)...",
        ticket.addr,
        PUNCH_FOR.as_secs()
    );
    let until = Instant::now() + PUNCH_FOR;
    let connection = loop {
        let attempt = endpoint.connect(ticket.addr, SERVER_NAME)?;
        match tokio::time::timeout(Duration::from_secs(5), attempt).await {
            Ok(Ok(connection)) => break connection,
            // The certificate isn't the one in the ticket: retrying can't help.
            Ok(Err(err)) if err.to_string().contains("not the host from the ticket") => {
                return Err("the host's certificate doesn't match the ticket. Ask for a fresh ticket; if it still fails, someone may be in the way".into());
            }
            Ok(Err(err)) => println!("  not yet: {err}"),
            Err(_) => println!("  not yet: no answer"),
        }
        if Instant::now() >= until {
            return Err("couldn't get through. One side's NAT is probably symmetric; this pair needs a relay".into());
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
        let token = ticket.token;
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
            addr: "203.0.113.5:54321".parse().unwrap(),
            fingerprint: [0xab; 32],
            token: [0x01; TOKEN_LEN],
        };
        let back = HostTicket::decode(&host.encode()).unwrap();
        assert_eq!(
            (back.addr, back.fingerprint, back.token),
            (host.addr, host.fingerprint, host.token)
        );
        assert_eq!(
            parse_join_ticket(&join_ticket("[2001:db8::1]:9".parse().unwrap())),
            Some("[2001:db8::1]:9".parse().unwrap())
        );
        assert!(HostTicket::decode("lhp1|join|1.2.3.4:5").is_none());
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(unhex(&hex(&[0, 15, 255])), Some(vec![0, 15, 255]));
        assert_eq!(unhex("zz"), None);
    }
}
