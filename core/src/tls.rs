//! HTTPS for the web server.
//!
//! The core serves TLS and plain HTTP on the same port (see [`DualProtocolAcceptor`]). Plain
//! HTTP is only accepted from this machine unless the owner allows it on the LAN, so passwords
//! and login tokens never cross the network unencrypted by default.
//!
//! The certificate is the user's own (`tls/cert.pem` and `tls/key.pem`) when both files
//! exist, and otherwise a self-signed one that the core creates and renews itself in
//! `tls/self-signed/` (see [`setup`]).

use std::{
    future::Future,
    io,
    net::IpAddr,
    path::Path,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};

use axum::{http::header, response::IntoResponse};
use axum_server::{
    accept::Accept,
    tls_rustls::{RustlsAcceptor, RustlsConfig},
};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, KeyPair,
    KeyUsagePurpose, SanType,
};
use rustls::{
    client::{danger::ServerCertVerifier, WebPkiServerVerifier},
    crypto::CryptoProvider,
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName, UnixTime},
    CertificateError, RootCertStore, ServerConfig,
};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::either::Either;
use tracing::{debug, error, info, warn};
use ts_rs::TS;

use crate::persist::write_atomic;

/// Where the TLS certificate in use comes from.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TlsMode {
    /// Created by the core in `tls/self-signed/`.
    SelfSigned,
    /// `tls/cert.pem` and `tls/key.pem`, supplied by the user.
    UserProvided,
    /// No certificate could be loaded or created: the core serves plain HTTP only.
    None,
}

/// The TLS state reported by `/info`. The fingerprint is public: every TLS handshake sends
/// the certificate.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct TlsInfo {
    pub mode: TlsMode,
    /// SHA-256 of the certificate (DER), as colon-separated uppercase hex.
    pub fingerprint_sha256: Option<String>,
}

/// The certificate the web server uses, see [`setup`].
pub struct TlsSetup {
    pub info: TlsInfo,
    /// `None` when [`TlsMode::None`].
    pub config: Option<RustlsConfig>,
}

/// Total validity of a self-signed certificate. 825 days is the most Apple platforms accept.
const SELF_SIGNED_VALIDITY: Duration = Duration::from_secs(825 * 24 * 60 * 60);
/// A self-signed certificate is backdated by this much, so a device whose clock is a little
/// behind still accepts it.
const SELF_SIGNED_BACKDATE: Duration = Duration::from_secs(60 * 60);
/// A self-signed certificate is replaced at startup once fewer than this many days remain.
const RENEW_BEFORE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const SELF_SIGNED_COMMON_NAME: &str = "Lodestone Core";

fn crypto_provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

/// SHA-256 through rustls' aws-lc-rs provider (the hash of a TLS 1.3 SHA-256 suite), so no
/// separate hashing crate is needed.
fn sha256(data: &[u8]) -> Vec<u8> {
    rustls::crypto::aws_lc_rs::cipher_suite::TLS13_AES_128_GCM_SHA256
        .tls13()
        .expect("TLS13_AES_128_GCM_SHA256 is a TLS 1.3 suite")
        .common
        .hash_provider
        .hash(data)
        .as_ref()
        .to_vec()
}

/// The SHA-256 fingerprint of a DER certificate, as browsers show it: `AB:CD:...`.
pub fn sha256_fingerprint(cert_der: &[u8]) -> String {
    sha256(cert_der)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// A rustls server config for `certs` and `key`, with the same ALPN protocols as
/// axum-server's own `RustlsConfig` (HTTP/2, then HTTP/1.1).
fn server_config(
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
) -> Result<ServerConfig, rustls::Error> {
    let mut config = ServerConfig::builder_with_provider(crypto_provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

/// A certificate chain and its private key, parsed from PEM.
struct PemPair {
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl PemPair {
    fn parse(cert_pem: &[u8], key_pem: &[u8]) -> Result<Self, String> {
        let certs = CertificateDer::pem_slice_iter(cert_pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("the certificate is not valid PEM ({e})"))?;
        if certs.is_empty() {
            return Err("the certificate file contains no certificate".to_string());
        }
        let key = PrivateKeyDer::from_pem_slice(key_pem)
            .map_err(|e| format!("the private key could not be read ({e})"))?;
        Ok(Self { certs, key })
    }

    fn fingerprint(&self) -> String {
        sha256_fingerprint(self.certs[0].as_ref())
    }

    /// Fails when the key does not belong to the certificate.
    fn into_config(self) -> Result<ServerConfig, String> {
        server_config(self.certs, self.key).map_err(|e| e.to_string())
    }
}

async fn read_pem_pair(cert_path: &Path, key_path: &Path) -> Result<PemPair, String> {
    let cert = tokio::fs::read(cert_path)
        .await
        .map_err(|e| format!("could not read {} ({e})", cert_path.display()))?;
    let key = tokio::fs::read(key_path)
        .await
        .map_err(|e| format!("could not read {} ({e})", key_path.display()))?;
    PemPair::parse(&cert, &key)
}

/// The names a self-signed certificate is made for: `localhost`, the loopback addresses,
/// the machine's host name (with and without `.local`) and its network interface addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateIdentity {
    dns_names: Vec<String>,
    ips: Vec<IpAddr>,
}

impl CertificateIdentity {
    /// The identity of this machine right now.
    pub fn current() -> Self {
        let interface_ips = match local_ip_address::list_afinet_netifas() {
            Ok(interfaces) => interfaces.into_iter().map(|(_, ip)| ip).collect(),
            Err(e) => {
                warn!("Could not list the network interfaces for the TLS certificate: {e}");
                Vec::new()
            }
        };
        Self::new(sysinfo::System::host_name(), interface_ips)
    }

    pub fn new(host_name: Option<String>, interface_ips: impl IntoIterator<Item = IpAddr>) -> Self {
        let mut dns_names = vec!["localhost".to_string()];
        if let Some(host_name) = host_name {
            let host_name = host_name.trim().trim_end_matches('.').to_ascii_lowercase();
            let short_name = host_name.strip_suffix(".local").unwrap_or(&host_name);
            if is_valid_dns_name(short_name) {
                for name in [short_name.to_string(), format!("{short_name}.local")] {
                    if is_valid_dns_name(&name) && !dns_names.contains(&name) {
                        dns_names.push(name);
                    }
                }
            } else {
                debug!("Leaving the host name {host_name:?} out of the TLS certificate: it is not a valid DNS name");
            }
        }
        let mut ips: Vec<IpAddr> = vec![
            IpAddr::from([127, 0, 0, 1]),
            IpAddr::from([0, 0, 0, 0, 0, 0, 0, 1]),
        ];
        let mut interface_ips: Vec<IpAddr> = interface_ips
            .into_iter()
            .map(|ip| ip.to_canonical())
            .filter(|ip| is_reachable_interface_ip(*ip))
            .collect();
        interface_ips.sort();
        for ip in interface_ips {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
        Self { dns_names, ips }
    }

    fn subject_alt_names(&self) -> Result<Vec<SanType>, rcgen::Error> {
        let mut sans = Vec::with_capacity(self.dns_names.len() + self.ips.len());
        for name in &self.dns_names {
            sans.push(SanType::DnsName(name.clone().try_into()?));
        }
        sans.extend(self.ips.iter().map(|ip| SanType::IpAddress(*ip)));
        Ok(sans)
    }

    /// The names an existing certificate must cover to be kept. IPv6 interface addresses are
    /// in new certificates but not required: temporary IPv6 addresses change daily, and a new
    /// certificate each time would make every browser ask to trust it again.
    fn required_names(&self) -> Vec<(String, ServerName<'static>)> {
        let mut names: Vec<(String, ServerName<'static>)> = Vec::new();
        for name in &self.dns_names {
            if let Ok(server_name) = ServerName::try_from(name.clone()) {
                names.push((name.clone(), server_name));
            }
        }
        for ip in &self.ips {
            if ip.is_ipv4() || ip.is_loopback() {
                names.push((ip.to_string(), ServerName::IpAddress((*ip).into())));
            }
        }
        names
    }
}

/// A host name label is 1 to 63 letters, digits or hyphens, and does not start or end with
/// a hyphen. Anything else is left out of the certificate.
fn is_valid_dns_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        // a name made only of digits and dots would be read as an IP address
        && !name.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Whether another device could reach this machine at `ip`: not loopback, unspecified,
/// multicast or link-local (link-local addresses need an interface scope that a URL rarely
/// carries, and change on every boot on some systems).
fn is_reachable_interface_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_link_local())
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

fn unix_time(time: SystemTime) -> UnixTime {
    UnixTime::since_unix_epoch(
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default(),
    )
}

/// Why an existing self-signed certificate cannot be kept, or `None` when it can: it must
/// parse, be valid now and for at least [`RENEW_BEFORE`], and cover every required name of
/// `identity`.
fn self_signed_problem(
    cert: &CertificateDer<'static>,
    identity: &CertificateIdentity,
    now: SystemTime,
) -> Option<String> {
    // The certificate is its own issuer, so trusting it alone lets webpki check its validity
    // period and names exactly like a TLS client does.
    let mut roots = RootCertStore::empty();
    if let Err(e) = roots.add(cert.clone()) {
        return Some(format!("it could not be parsed ({e})"));
    }
    let verifier =
        match WebPkiServerVerifier::builder_with_provider(Arc::new(roots), crypto_provider())
            .build()
        {
            Ok(verifier) => verifier,
            Err(e) => return Some(format!("it could not be checked ({e})")),
        };
    let check = |name: &ServerName<'static>, time: SystemTime| {
        verifier.verify_server_cert(cert, &[], name, &[], unix_time(time))
    };
    let names = identity.required_names();
    for (label, name) in &names {
        match check(name, now) {
            Ok(_) => {}
            Err(rustls::Error::InvalidCertificate(
                CertificateError::Expired | CertificateError::ExpiredContext { .. },
            )) => return Some("it has expired".to_string()),
            Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => return Some(format!("it does not cover {label}")),
            Err(e) => return Some(format!("it is not valid ({e})")),
        }
    }
    if let Some((_, name)) = names.first() {
        match check(name, now + RENEW_BEFORE) {
            Ok(_) => {}
            Err(rustls::Error::InvalidCertificate(
                CertificateError::Expired | CertificateError::ExpiredContext { .. },
            )) => return Some("it expires within 30 days".to_string()),
            Err(e) => return Some(format!("it is not valid ({e})")),
        }
    }
    None
}

/// A new self-signed certificate (PEM) and its ECDSA P-256 private key (PKCS#8 PEM).
fn generate_self_signed(
    identity: &CertificateIdentity,
    now: SystemTime,
) -> Result<(String, String), rcgen::Error> {
    let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let mut params = CertificateParams::default();
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, SELF_SIGNED_COMMON_NAME);
    params.distinguished_name = distinguished_name;
    params.subject_alt_names = identity.subject_alt_names()?;
    let not_before = now - SELF_SIGNED_BACKDATE;
    params.not_before = not_before.into();
    params.not_after = (not_before + SELF_SIGNED_VALIDITY).into();
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    // Apple platforms refuse TLS server certificates without this.
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

/// Why a new self-signed certificate was made.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RenewReason {
    /// There was none yet.
    Missing,
    /// The files exist but could not be used.
    Unusable(String),
    /// The certificate is fine but expires soon or misses a current name.
    Outdated(String),
}

/// The self-signed certificate in use.
struct SelfSigned {
    config: ServerConfig,
    fingerprint: String,
    /// Why it was just made, or `None` when the existing one was kept.
    renewed: Option<RenewReason>,
}

/// Loads the self-signed certificate in `dir`, or makes a new one when there is none, it is
/// broken, expires within [`RENEW_BEFORE`], or misses a name of `identity`.
async fn load_or_create_self_signed(
    dir: &Path,
    identity: &CertificateIdentity,
    now: SystemTime,
) -> Result<SelfSigned, String> {
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    let renew_reason = if !cert_path.exists() && !key_path.exists() {
        RenewReason::Missing
    } else {
        match read_pem_pair(&cert_path, &key_path).await {
            Err(e) => RenewReason::Unusable(e),
            Ok(pair) => match self_signed_problem(&pair.certs[0], identity, now) {
                Some(problem) => RenewReason::Outdated(problem),
                None => {
                    let fingerprint = pair.fingerprint();
                    match pair.into_config() {
                        Ok(config) => {
                            return Ok(SelfSigned {
                                config,
                                fingerprint,
                                renewed: None,
                            })
                        }
                        Err(e) => RenewReason::Unusable(e),
                    }
                }
            },
        }
    };

    let (cert_pem, key_pem) = generate_self_signed(identity, now)
        .map_err(|e| format!("could not create a self-signed certificate ({e})"))?;
    let pair = PemPair::parse(cert_pem.as_bytes(), key_pem.as_bytes())?;
    let fingerprint = pair.fingerprint();
    let config = pair.into_config()?;
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("could not create {} ({e})", dir.display()))?;
    // The key first: a crash in between leaves a pair that does not match, which the next
    // start replaces.
    write_atomic(&key_path, key_pem, Some(0o600))
        .await
        .map_err(|e| format!("could not save the private key ({})", e.source))?;
    write_atomic(&cert_path, cert_pem, Some(0o644))
        .await
        .map_err(|e| format!("could not save the certificate ({})", e.source))?;
    Ok(SelfSigned {
        config,
        fingerprint,
        renewed: Some(renew_reason),
    })
}

/// Picks the certificate for the web server and logs its fingerprint. Never fails: a broken
/// certificate is replaced by a self-signed one, and if that cannot be made either, the core
/// serves plain HTTP only.
pub async fn setup(tls_dir: &Path) -> TlsSetup {
    setup_with_identity(tls_dir, CertificateIdentity::current, SystemTime::now()).await
}

async fn setup_with_identity(
    tls_dir: &Path,
    identity: impl FnOnce() -> CertificateIdentity + Send + 'static,
    now: SystemTime,
) -> TlsSetup {
    let user_cert = tls_dir.join("cert.pem");
    let user_key = tls_dir.join("key.pem");
    match (user_cert.exists(), user_key.exists()) {
        (true, true) => match read_pem_pair(&user_cert, &user_key).await.and_then(|pair| {
            let fingerprint = pair.fingerprint();
            pair.into_config().map(|config| (config, fingerprint))
        }) {
            Ok((config, fingerprint)) => {
                info!(
                    "Using the TLS certificate in {} (SHA-256 fingerprint {fingerprint})",
                    tls_dir.display()
                );
                return TlsSetup {
                    info: TlsInfo {
                        mode: TlsMode::UserProvided,
                        fingerprint_sha256: Some(fingerprint),
                    },
                    config: Some(RustlsConfig::from_config(Arc::new(config))),
                };
            }
            Err(e) => error!(
                "Cannot use the TLS certificate in {}: {e}. Using a self-signed certificate instead",
                tls_dir.display()
            ),
        },
        (true, false) | (false, true) => warn!(
            "{} needs both cert.pem and key.pem to use your own TLS certificate. Using a self-signed certificate instead",
            tls_dir.display()
        ),
        (false, false) => {}
    }

    let identity = match tokio::task::spawn_blocking(identity).await {
        Ok(identity) => identity,
        Err(e) => {
            warn!("Could not read the host name and addresses for the TLS certificate: {e}");
            CertificateIdentity::new(None, [])
        }
    };
    let self_signed_dir = tls_dir.join("self-signed");
    match load_or_create_self_signed(&self_signed_dir, &identity, now).await {
        Ok(self_signed) => {
            match &self_signed.renewed {
                None => {}
                Some(RenewReason::Missing) => info!(
                    "Created a self-signed TLS certificate in {}. Browsers will ask you to trust it once",
                    self_signed_dir.display()
                ),
                Some(RenewReason::Unusable(e)) => error!(
                    "The self-signed TLS certificate in {} is unusable ({e}). Created a new one: browsers will ask you to trust it again",
                    self_signed_dir.display()
                ),
                Some(RenewReason::Outdated(why)) => warn!(
                    "Replaced the self-signed TLS certificate because {why}. Browsers will ask you to trust the new one"
                ),
            }
            info!(
                "Self-signed TLS certificate SHA-256 fingerprint: {}",
                self_signed.fingerprint
            );
            TlsSetup {
                info: TlsInfo {
                    mode: TlsMode::SelfSigned,
                    fingerprint_sha256: Some(self_signed.fingerprint),
                },
                config: Some(RustlsConfig::from_config(Arc::new(self_signed.config))),
            }
        }
        Err(e) => {
            error!("Could not set up HTTPS: {e}. Lodestone Core will serve plain HTTP only");
            TlsSetup {
                info: TlsInfo {
                    mode: TlsMode::None,
                    fingerprint_sha256: None,
                },
                config: None,
            }
        }
    }
}

/// The first byte of every TLS connection (a handshake record).
const TLS_HANDSHAKE_RECORD: u8 = 0x16;
/// How long a new connection may stay silent before it is closed.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(30);

/// The response to plain HTTP from another device while that is not allowed.
pub const PLAIN_HTTP_REFUSED_BODY: &str = "This Lodestone core only accepts HTTPS from other devices. Connect with https:// instead of http://.";

fn plain_http_refused_response() -> String {
    format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        PLAIN_HTTP_REFUSED_BODY.len(),
        PLAIN_HTTP_REFUSED_BODY
    )
}

/// Whether `ip` is this machine: 127.0.0.0/8, `::1`, or an IPv4-mapped loopback address
/// (what a dual-stack socket reports for IPv4 clients).
pub fn is_loopback(ip: IpAddr) -> bool {
    ip.to_canonical().is_loopback()
}

/// What to do with a new connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDecision {
    /// Serve it over TLS.
    Tls,
    /// Serve it as plain HTTP.
    PlainHttp,
    /// Answer 403 and close: plain HTTP from another device.
    RefusePlainHttp,
    /// Close it: a TLS handshake while the core has no certificate.
    Close,
}

/// Decides how to serve a connection from its first byte and the peer's address. Without a
/// certificate, plain HTTP is served to everyone, as before HTTPS existed.
pub fn decide(
    first_byte: u8,
    peer: IpAddr,
    tls_available: bool,
    allow_lan_http: bool,
) -> ConnectionDecision {
    if first_byte == TLS_HANDSHAKE_RECORD {
        return if tls_available {
            ConnectionDecision::Tls
        } else {
            ConnectionDecision::Close
        };
    }
    if !tls_available || allow_lan_http || is_loopback(peer) {
        ConnectionDecision::PlainHttp
    } else {
        ConnectionDecision::RefusePlainHttp
    }
}

/// The TLS stream type produced by axum-server's rustls acceptor.
type TlsStream<S> = <RustlsAcceptor as Accept<TcpStream, S>>::Stream;

/// Serves TLS and plain HTTP on one port. It peeks the first byte of each connection without
/// consuming it: a TLS handshake goes through axum-server's [`RustlsAcceptor`], anything else
/// is plain HTTP if [`decide`] allows it. Both kinds of stream go to the same hyper connection
/// handler, so HTTP upgrades (WebSockets) work on both.
#[derive(Clone)]
pub struct DualProtocolAcceptor {
    tls: Option<RustlsAcceptor>,
    allow_lan_http: Arc<AtomicBool>,
}

impl DualProtocolAcceptor {
    /// `allow_lan_http` is read on every connection, so changing the setting takes effect
    /// for new connections at once.
    pub fn new(tls: Option<RustlsConfig>, allow_lan_http: Arc<AtomicBool>) -> Self {
        Self {
            tls: tls.map(RustlsAcceptor::new),
            allow_lan_http,
        }
    }

    /// Accepts `stream` as if it came from `peer`.
    pub async fn accept_from<S: Send + 'static>(
        &self,
        stream: TcpStream,
        peer: IpAddr,
        service: S,
    ) -> io::Result<(Either<TlsStream<S>, TcpStream>, S)> {
        let mut first_byte = [0_u8; 1];
        let read = tokio::time::timeout(FIRST_BYTE_TIMEOUT, stream.peek(&mut first_byte))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no data from the client"))??;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let decision = decide(
            first_byte[0],
            peer,
            self.tls.is_some(),
            self.allow_lan_http.load(Ordering::SeqCst),
        );
        match (decision, &self.tls) {
            (ConnectionDecision::Tls, Some(tls)) => {
                let (stream, service) = tls.accept(stream, service).await?;
                Ok((Either::Left(stream), service))
            }
            (ConnectionDecision::PlainHttp, _) => Ok((Either::Right(stream), service)),
            (ConnectionDecision::RefusePlainHttp, _) => {
                debug!("Refused plain HTTP from {peer}");
                refuse_plain_http(stream).await;
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "plain HTTP from another device",
                ))
            }
            (ConnectionDecision::Tls | ConnectionDecision::Close, _) => Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "TLS is not available",
            )),
        }
    }
}

impl<S: Send + 'static> Accept<TcpStream, S> for DualProtocolAcceptor {
    type Stream = Either<TlsStream<S>, TcpStream>;
    type Service = S;
    type Future = Pin<Box<dyn Future<Output = io::Result<(Self::Stream, S)>> + Send>>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let acceptor = self.clone();
        Box::pin(async move {
            let peer = stream.peer_addr()?.ip();
            acceptor.accept_from(stream, peer, service).await
        })
    }
}

/// Answers a refused plain HTTP connection with 403 and closes it. The request is read
/// first, and whatever follows is drained after the response: closing a socket with unread
/// data makes the OS reset the connection, and the client may then never see the response.
async fn refuse_plain_http(mut stream: TcpStream) {
    const MAX_DRAIN: usize = 64 * 1024;
    let mut buffer = [0_u8; 4096];
    let mut head: Vec<u8> = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while head.len() < MAX_DRAIN && !head.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(n) => head.extend_from_slice(&buffer[..n]),
            }
        }
    })
    .await;
    let _ = tokio::time::timeout(
        Duration::from_secs(2),
        stream.write_all(plain_http_refused_response().as_bytes()),
    )
    .await;
    let _ = stream.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        let mut drained = 0;
        while drained < MAX_DRAIN {
            match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(n) => drained += n,
            }
        }
    })
    .await;
}

/// The page at `/`. Opening `https://<core>:<port>/` in a browser and accepting the
/// certificate warning makes the browser trust the core's self-signed certificate.
pub const ROOT_PAGE_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Lodestone Core</title>
<style>
body { font-family: system-ui, sans-serif; max-width: 36rem; margin: 4rem auto; padding: 0 1rem; line-height: 1.5; color: #e5e7eb; background: #1f2023; }
h1 { font-size: 1.5rem; }
code { background: #2e2f33; padding: 0 0.25rem; border-radius: 0.25rem; }
</style>
</head>
<body>
<h1>This is a Lodestone Core</h1>
<p>If you opened this page with <code>https://</code> and accepted the certificate warning, your browser now trusts this core's certificate.</p>
<p>Go back to the Lodestone dashboard and connect to this core with <code>https://</code>.</p>
</body>
</html>
"#;

/// Serves [`ROOT_PAGE_HTML`].
pub async fn root_page() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'",
            ),
        ],
        ROOT_PAGE_HTML,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, Ipv6Addr};

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn test_identity() -> CertificateIdentity {
        CertificateIdentity::new(
            Some("TestHost".to_string()),
            [
                ip("192.168.1.20"),
                ip("2001:db8::5"),
                ip("fe80::1"),
                ip("169.254.3.4"),
                ip("127.0.0.1"),
            ],
        )
    }

    fn cert_der(pem: &str) -> CertificateDer<'static> {
        CertificateDer::from_pem_slice(pem.as_bytes()).unwrap()
    }

    /// Whether a client trusting only `cert` accepts it for `name` at `time`.
    fn accepts(
        cert: &CertificateDer<'static>,
        name: &str,
        time: SystemTime,
    ) -> Result<(), rustls::Error> {
        let mut roots = RootCertStore::empty();
        roots.add(cert.clone()).unwrap();
        let verifier =
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), crypto_provider())
                .build()
                .unwrap();
        verifier
            .verify_server_cert(
                cert,
                &[],
                &ServerName::try_from(name.to_string()).unwrap(),
                &[],
                unix_time(time),
            )
            .map(|_| ())
    }

    #[test]
    fn loopback_check() {
        for addr in [
            "127.0.0.1",
            "127.8.9.10",
            "::1",
            "::ffff:127.0.0.1",
            "::ffff:127.1.2.3",
        ] {
            assert!(is_loopback(ip(addr)), "{addr}");
        }
        for addr in [
            "192.168.1.2",
            "10.0.0.1",
            "::ffff:192.168.1.2",
            "fe80::1",
            "2001:db8::1",
            "0.0.0.0",
            "::",
        ] {
            assert!(!is_loopback(ip(addr)), "{addr}");
        }
    }

    #[test]
    fn decision() {
        use ConnectionDecision::*;
        let lan = ip("192.168.1.2");
        let local = ip("::ffff:127.0.0.1");
        // TLS from anywhere
        assert_eq!(decide(0x16, lan, true, false), Tls);
        assert_eq!(decide(0x16, local, true, false), Tls);
        assert_eq!(decide(0x16, lan, false, false), Close);
        // plain HTTP only from this machine, unless allowed
        assert_eq!(decide(b'G', local, true, false), PlainHttp);
        assert_eq!(decide(b'G', ip("::1"), true, false), PlainHttp);
        assert_eq!(decide(b'G', lan, true, false), RefusePlainHttp);
        assert_eq!(decide(b'P', lan, true, false), RefusePlainHttp);
        assert_eq!(decide(b'G', lan, true, true), PlainHttp);
        // without a certificate, plain HTTP is served to everyone
        assert_eq!(decide(b'G', lan, false, false), PlainHttp);
    }

    #[test]
    fn fingerprint_format() {
        assert_eq!(
            sha256_fingerprint(b"abc"),
            "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
        );
    }

    #[test]
    fn identity_names() {
        let identity = test_identity();
        assert_eq!(
            identity.dns_names,
            ["localhost", "testhost", "testhost.local"]
        );
        assert_eq!(
            identity.ips,
            [
                ip("127.0.0.1"),
                ip("::1"),
                ip("192.168.1.20"),
                ip("2001:db8::5")
            ]
        );
        // a host name that already ends in .local, and IPv4-mapped addresses
        let identity = CertificateIdentity::new(
            Some("Box.local.".to_string()),
            [ip("::ffff:10.0.0.2"), ip("10.0.0.2")],
        );
        assert_eq!(identity.dns_names, ["localhost", "box", "box.local"]);
        assert_eq!(identity.ips, [ip("127.0.0.1"), ip("::1"), ip("10.0.0.2")]);
        // invalid host names are left out
        for host in ["my host", "under_score", "-dash", "", "123.45", "é"] {
            let identity = CertificateIdentity::new(Some(host.to_string()), []);
            assert_eq!(identity.dns_names, ["localhost"], "{host:?}");
        }
        // IPv6 interface addresses are in the certificate but not required
        let required: Vec<String> = test_identity()
            .required_names()
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        assert_eq!(
            required,
            [
                "localhost",
                "testhost",
                "testhost.local",
                "127.0.0.1",
                "::1",
                "192.168.1.20"
            ]
        );
    }

    #[test]
    fn generated_certificate_names_and_validity() {
        let now = SystemTime::now();
        let identity = test_identity();
        let (cert_pem, key_pem) = generate_self_signed(&identity, now).unwrap();
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));
        let cert = cert_der(&cert_pem);

        for name in [
            "localhost",
            "testhost",
            "testhost.local",
            "127.0.0.1",
            "::1",
            "192.168.1.20",
            "2001:db8::5",
        ] {
            accepts(&cert, name, now).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for name in ["otherhost", "192.168.1.21", "fe80::1", "169.254.3.4"] {
            assert!(
                matches!(
                    accepts(&cert, name, now),
                    Err(rustls::Error::InvalidCertificate(
                        CertificateError::NotValidForName
                            | CertificateError::NotValidForNameContext { .. }
                    ))
                ),
                "{name}"
            );
        }

        // valid for exactly 825 days, starting an hour ago
        let not_before = now - SELF_SIGNED_BACKDATE;
        let minute = Duration::from_secs(60);
        assert!(accepts(&cert, "localhost", not_before - minute).is_err());
        accepts(&cert, "localhost", not_before + minute).unwrap();
        accepts(&cert, "localhost", not_before + 825 * DAY - minute).unwrap();
        assert!(matches!(
            accepts(&cert, "localhost", not_before + 825 * DAY + minute),
            Err(rustls::Error::InvalidCertificate(
                CertificateError::Expired | CertificateError::ExpiredContext { .. }
            ))
        ));

        // the key belongs to the certificate, and rustls can serve it
        PemPair::parse(cert_pem.as_bytes(), key_pem.as_bytes())
            .unwrap()
            .into_config()
            .unwrap();
    }

    #[test]
    fn renewal_conditions() {
        let now = SystemTime::now();
        let identity = test_identity();
        let (cert_pem, _) = generate_self_signed(&identity, now).unwrap();
        let cert = cert_der(&cert_pem);

        assert_eq!(self_signed_problem(&cert, &identity, now), None);
        // a new IPv6 address alone does not renew it
        let new_ipv6 = CertificateIdentity::new(
            Some("testhost".to_string()),
            [ip("192.168.1.20"), ip("2001:db8::77")],
        );
        assert_eq!(self_signed_problem(&cert, &new_ipv6, now), None);
        // a new IPv4 address does
        let new_ipv4 = CertificateIdentity::new(
            Some("testhost".to_string()),
            [ip("192.168.1.20"), ip("10.1.2.3")],
        );
        assert_eq!(
            self_signed_problem(&cert, &new_ipv4, now).as_deref(),
            Some("it does not cover 10.1.2.3")
        );
        // so does a new host name
        let renamed = CertificateIdentity::new(Some("newname".to_string()), [ip("192.168.1.20")]);
        assert_eq!(
            self_signed_problem(&cert, &renamed, now).as_deref(),
            Some("it does not cover newname")
        );
        // fewer than 30 days left
        assert_eq!(self_signed_problem(&cert, &identity, now + 790 * DAY), None);
        assert_eq!(
            self_signed_problem(&cert, &identity, now + 796 * DAY).as_deref(),
            Some("it expires within 30 days")
        );
        assert_eq!(
            self_signed_problem(&cert, &identity, now + 900 * DAY).as_deref(),
            Some("it has expired")
        );
        // not a certificate at all
        assert!(
            self_signed_problem(&CertificateDer::from(vec![1, 2, 3]), &identity, now)
                .unwrap()
                .starts_with("it could not be parsed")
        );
    }

    #[tokio::test]
    async fn self_signed_files_are_created_kept_and_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let self_signed_dir = dir.path().join("tls").join("self-signed");
        let identity = test_identity();
        let now = SystemTime::now();

        let first = load_or_create_self_signed(&self_signed_dir, &identity, now)
            .await
            .unwrap();
        assert_eq!(first.renewed, Some(RenewReason::Missing));
        let cert_path = self_signed_dir.join("cert.pem");
        let key_path = self_signed_dir.join("key.pem");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&key_path), 0o600);
            assert_eq!(mode(&cert_path), 0o644);
        }
        // the logged fingerprint is the certificate's
        let cert_pem = std::fs::read_to_string(&cert_path).unwrap();
        assert_eq!(
            first.fingerprint,
            sha256_fingerprint(cert_der(&cert_pem).as_ref())
        );
        assert_eq!(first.fingerprint.len(), 32 * 3 - 1);

        // kept on the next start
        let second = load_or_create_self_signed(&self_signed_dir, &identity, now)
            .await
            .unwrap();
        assert_eq!(second.renewed, None);
        assert_eq!(second.fingerprint, first.fingerprint);

        // replaced when it misses an address
        let moved = CertificateIdentity::new(Some("testhost".to_string()), [ip("10.9.8.7")]);
        let third = load_or_create_self_signed(&self_signed_dir, &moved, now)
            .await
            .unwrap();
        assert_eq!(
            third.renewed,
            Some(RenewReason::Outdated(
                "it does not cover 10.9.8.7".to_string()
            ))
        );
        assert_ne!(third.fingerprint, first.fingerprint);

        // replaced when the key does not belong to the certificate
        let (_, other_key) = generate_self_signed(&moved, now).unwrap();
        std::fs::write(&key_path, other_key).unwrap();
        let mismatched = load_or_create_self_signed(&self_signed_dir, &moved, now)
            .await
            .unwrap();
        assert!(matches!(mismatched.renewed, Some(RenewReason::Unusable(_))));

        // replaced when the certificate is garbage
        std::fs::write(&cert_path, "not a certificate").unwrap();
        let garbage = load_or_create_self_signed(&self_signed_dir, &moved, now)
            .await
            .unwrap();
        assert!(matches!(garbage.renewed, Some(RenewReason::Unusable(_))));
        assert_eq!(
            load_or_create_self_signed(&self_signed_dir, &moved, now)
                .await
                .unwrap()
                .renewed,
            None
        );

        // replaced when it expires soon
        let expiring = load_or_create_self_signed(&self_signed_dir, &moved, now + 800 * DAY)
            .await
            .unwrap();
        assert_eq!(
            expiring.renewed,
            Some(RenewReason::Outdated(
                "it expires within 30 days".to_string()
            ))
        );
    }

    #[tokio::test]
    async fn user_provided_certificate_wins_and_broken_one_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let tls_dir = dir.path().join("tls");
        std::fs::create_dir_all(&tls_dir).unwrap();
        let now = SystemTime::now();

        // no files: self-signed
        let setup = setup_with_identity(&tls_dir, test_identity, now).await;
        assert_eq!(setup.info.mode, TlsMode::SelfSigned);
        assert!(setup.config.is_some());

        // the user's own certificate wins
        let (cert_pem, key_pem) = generate_self_signed(&test_identity(), now).unwrap();
        std::fs::write(tls_dir.join("cert.pem"), &cert_pem).unwrap();
        std::fs::write(tls_dir.join("key.pem"), &key_pem).unwrap();
        let setup = setup_with_identity(&tls_dir, test_identity, now).await;
        assert_eq!(setup.info.mode, TlsMode::UserProvided);
        assert_eq!(
            setup.info.fingerprint_sha256,
            Some(sha256_fingerprint(cert_der(&cert_pem).as_ref()))
        );

        // a broken one never stops the core
        std::fs::write(tls_dir.join("key.pem"), "broken").unwrap();
        let setup = setup_with_identity(&tls_dir, test_identity, now).await;
        assert_eq!(setup.info.mode, TlsMode::SelfSigned);
        assert!(setup.config.is_some());

        // and nor does a missing key
        std::fs::remove_file(tls_dir.join("key.pem")).unwrap();
        let setup = setup_with_identity(&tls_dir, test_identity, now).await;
        assert_eq!(setup.info.mode, TlsMode::SelfSigned);
    }

    #[test]
    fn info_serialization() {
        let info = TlsInfo {
            mode: TlsMode::SelfSigned,
            fingerprint_sha256: Some("AB:CD".to_string()),
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            serde_json::json!({"mode": "self_signed", "fingerprint_sha256": "AB:CD"})
        );
        let none = TlsInfo {
            mode: TlsMode::None,
            fingerprint_sha256: None,
        };
        assert_eq!(
            serde_json::to_value(&none).unwrap(),
            serde_json::json!({"mode": "none", "fingerprint_sha256": null})
        );
        assert_eq!(
            serde_json::to_value(TlsMode::UserProvided).unwrap(),
            "user_provided"
        );
    }

    // End-to-end tests on a real socket on 127.0.0.1, port 0, so they never clash with a
    // running core.

    const WS_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
    const WS_ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

    fn test_router() -> axum::Router {
        use axum::extract::ws::{Message, WebSocketUpgrade};
        axum::Router::new()
            .route("/", axum::routing::get(root_page))
            .route("/api/v1/hello", axum::routing::get(|| async { "hello" }))
            .route(
                "/api/v1/ws",
                axum::routing::get(|ws: WebSocketUpgrade| async move {
                    ws.on_upgrade(|mut socket| async move {
                        let _ = socket.send(Message::Text("hi".into())).await;
                        // wait for the client to go away
                        while let Some(Ok(_)) = socket.recv().await {}
                    })
                }),
            )
    }

    fn get_request(path: &str) -> String {
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
    }

    fn websocket_request() -> String {
        format!(
            "GET /api/v1/ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\
             Sec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {WS_KEY}\r\n\r\n"
        )
    }

    /// The first WebSocket frame the test route sends: an unmasked text frame "hi".
    const WS_HI_FRAME: &[u8] = &[0x81, 0x02, b'h', b'i'];

    /// Reads a response. For a WebSocket upgrade, reads the headers and the first frame
    /// (the connection stays open); otherwise reads until the server closes.
    fn read_response(stream: &mut impl Read, upgrade: bool) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            if upgrade {
                if let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") {
                    if out.len() >= end + 4 + WS_HI_FRAME.len() {
                        break;
                    }
                }
            }
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => out.extend_from_slice(&buffer[..n]),
            }
        }
        out
    }

    fn assert_upgraded(response: &[u8]) {
        let text = String::from_utf8_lossy(response);
        assert!(text.starts_with("HTTP/1.1 101"), "{text}");
        assert!(text.contains(WS_ACCEPT), "{text}");
        assert!(response.ends_with(WS_HI_FRAME), "{response:?}");
    }

    fn plain_request(port: u16, request: String, upgrade: bool) -> Vec<u8> {
        let mut stream = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        read_response(&mut stream, upgrade)
    }

    fn tls_request(
        port: u16,
        trusted: CertificateDer<'static>,
        server_name: &str,
        request: String,
        upgrade: bool,
    ) -> Result<Vec<u8>, String> {
        let mut roots = RootCertStore::empty();
        roots.add(trusted).unwrap();
        let config = rustls::ClientConfig::builder_with_provider(crypto_provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connection = rustls::ClientConnection::new(
            Arc::new(config),
            ServerName::try_from(server_name.to_string()).unwrap(),
        )
        .unwrap();
        let socket = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut stream = rustls::StreamOwned::new(connection, socket);
        // writing drives the handshake, so a rejected certificate shows up here
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;
        Ok(read_response(&mut stream, upgrade))
    }

    fn body_of(response: &[u8]) -> String {
        let text = String::from_utf8_lossy(response).to_string();
        text.split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn serves_tls_and_local_plain_http_on_one_port() {
        let dir = tempfile::tempdir().unwrap();
        let identity = CertificateIdentity::new(Some("testhost".to_string()), []);
        let self_signed = load_or_create_self_signed(dir.path(), &identity, SystemTime::now())
            .await
            .unwrap();
        let cert_pem = std::fs::read_to_string(dir.path().join("cert.pem")).unwrap();
        let cert = cert_der(&cert_pem);
        let acceptor = DualProtocolAcceptor::new(
            Some(RustlsConfig::from_config(Arc::new(self_signed.config))),
            Arc::new(AtomicBool::new(false)),
        );

        let handle = axum_server::Handle::<std::net::SocketAddr>::new();
        let server = tokio::spawn(
            axum_server::bind(std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
                .acceptor(acceptor)
                .handle(handle.clone())
                .serve(test_router().into_make_service()),
        );
        let port = handle.listening().await.unwrap().port();

        let blocking = |f: Box<dyn FnOnce() -> Vec<u8> + Send>| tokio::task::spawn_blocking(f);

        // TLS, for both the host name and the loopback address in the certificate
        for server_name in ["localhost", "127.0.0.1"] {
            let cert = cert.clone();
            let response = tokio::task::spawn_blocking(move || {
                tls_request(port, cert, server_name, get_request("/api/v1/hello"), false)
            })
            .await
            .unwrap()
            .unwrap();
            assert!(
                String::from_utf8_lossy(&response).starts_with("HTTP/1.1 200"),
                "{server_name}"
            );
            assert_eq!(body_of(&response), "hello");
        }

        // the root page, over TLS
        let page = {
            let cert = cert.clone();
            tokio::task::spawn_blocking(move || {
                tls_request(port, cert, "localhost", get_request("/"), false)
            })
            .await
            .unwrap()
            .unwrap()
        };
        let page_text = String::from_utf8_lossy(&page);
        assert!(page_text.starts_with("HTTP/1.1 200"));
        assert!(page_text.contains("content-type: text/html"));
        assert!(page_text.contains("This is a Lodestone Core"));

        // a client that does not trust the certificate fails the handshake
        let (other_cert, _) = generate_self_signed(&identity, SystemTime::now()).unwrap();
        let other_cert = cert_der(&other_cert);
        let untrusted = tokio::task::spawn_blocking(move || {
            tls_request(port, other_cert, "localhost", get_request("/"), false)
        })
        .await
        .unwrap();
        assert!(untrusted.is_err());

        // plain HTTP from this machine
        let response = blocking(Box::new(move || {
            plain_request(port, get_request("/api/v1/hello"), false)
        }))
        .await
        .unwrap();
        assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 200"));
        assert_eq!(body_of(&response), "hello");

        // WebSocket upgrades work on both protocols
        let response = blocking(Box::new(move || {
            plain_request(port, websocket_request(), true)
        }))
        .await
        .unwrap();
        assert_upgraded(&response);
        let response = {
            let cert = cert.clone();
            tokio::task::spawn_blocking(move || {
                tls_request(port, cert, "localhost", websocket_request(), true)
            })
            .await
            .unwrap()
            .unwrap()
        };
        assert_upgraded(&response);

        // graceful shutdown still ends the server
        handle.graceful_shutdown(Some(Duration::from_secs(2)));
        tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .expect("server did not shut down")
            .unwrap()
            .unwrap();
    }

    /// Accepts one connection on a 127.0.0.1 socket through `acceptor`, as if it came from
    /// `peer`, while a client sends a plain HTTP request. Returns the acceptor's result and
    /// what the client received.
    async fn plain_http_from(
        acceptor: DualProtocolAcceptor,
        peer: IpAddr,
    ) -> (io::Result<bool>, Vec<u8>) {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let client = tokio::task::spawn_blocking(move || {
            let mut stream = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream
                .write_all(get_request("/api/v1/hello").as_bytes())
                .unwrap();
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response);
            response
        });
        let (server_stream, _) = listener.accept().await.unwrap();
        let accepted = acceptor
            .accept_from(server_stream, peer, ())
            .await
            .map(|(stream, ())| matches!(stream, Either::Right(_)));
        // the client only finishes reading once the accepted stream is gone
        let response = client.await.unwrap();
        (accepted, response)
    }

    #[tokio::test]
    async fn refuses_plain_http_from_other_devices() {
        let (cert_pem, key_pem) =
            generate_self_signed(&test_identity(), SystemTime::now()).unwrap();
        let config = PemPair::parse(cert_pem.as_bytes(), key_pem.as_bytes())
            .unwrap()
            .into_config()
            .unwrap();
        let allow_lan_http = Arc::new(AtomicBool::new(false));
        let acceptor = DualProtocolAcceptor::new(
            Some(RustlsConfig::from_config(Arc::new(config))),
            allow_lan_http.clone(),
        );
        let lan_peer = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50));

        let (accepted, response) = plain_http_from(acceptor.clone(), lan_peer).await;
        assert_eq!(
            accepted.unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        let response = String::from_utf8(response).unwrap();
        assert_eq!(
            response,
            format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                PLAIN_HTTP_REFUSED_BODY.len(),
                PLAIN_HTTP_REFUSED_BODY
            )
        );

        // an IPv4-mapped loopback peer is this machine
        let mapped_loopback = IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped());
        let (accepted, _) = plain_http_from(acceptor.clone(), mapped_loopback).await;
        assert!(accepted.unwrap());
        let (accepted, _) =
            plain_http_from(acceptor.clone(), IpAddr::V6(Ipv6Addr::LOCALHOST)).await;
        assert!(accepted.unwrap());

        // the setting takes effect for the next connection
        allow_lan_http.store(true, Ordering::SeqCst);
        let (accepted, _) = plain_http_from(acceptor, lan_peer).await;
        assert!(accepted.unwrap());
    }
}
