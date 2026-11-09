//! The QUIC tunnel: the host's certificate, the friend's pinned check of it, and the
//! header each stream starts with.
//!
//! Each bidirectional QUIC stream the friend opens starts with the invite's 16-byte
//! token and one byte for its [`StreamKind`]. The host closes any stream without the
//! right token or with an unknown kind.

use std::{net::UdpSocket, sync::Arc, time::Duration};

use quinn::{Endpoint, EndpointConfig, TransportConfig};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

/// The QUIC application protocol. Both sides must speak the same version.
pub const ALPN: &[u8] = b"lodestone-connect-1";
/// The name in the host's certificate (checked by fingerprint, not by name).
pub const SERVER_NAME: &str = "lodestone-connect";
/// The invite token's length in bytes.
pub const TOKEN_LEN: usize = 16;
/// How long a tunnel with no packets lasts. Keep-alives go every 5 seconds.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// What a stream carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamKind {
    /// Minecraft's own TCP connection, passed to the server's game port.
    Game = 0,
    /// HTTP/1.1 to a few read-only routes about the shared server ([`crate::info`]).
    Info = 1,
    /// The host answers one byte ([`HELLO_ANSWER`]) and closes it. A friend opens one
    /// right after connecting, to prove it has the token; the host closes connections
    /// that haven't done so within [`PROVE_WITHIN`].
    Hello = 2,
}

/// The host's answer on a [`StreamKind::Hello`] stream.
pub const HELLO_ANSWER: u8 = 0x4c; // 'L'
/// How long a new connection has to open a stream with the right token.
pub const PROVE_WITHIN: Duration = Duration::from_secs(20);

impl StreamKind {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(StreamKind::Game),
            1 => Some(StreamKind::Info),
            2 => Some(StreamKind::Hello),
            _ => None,
        }
    }
}

/// One stream through the tunnel, readable and writable like a TCP connection.
pub type TunnelStream = tokio::io::Join<quinn::RecvStream, quinn::SendStream>;

/// The SHA-256 of a DER certificate, as invite links carry it.
pub fn fingerprint(cert_der: &[u8]) -> [u8; 32] {
    Sha256::digest(cert_der).into()
}

pub(crate) fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub(crate) fn transport() -> Arc<TransportConfig> {
    let mut transport = TransportConfig::default();
    // Keeps the routers' mappings open while nobody is playing.
    transport.keep_alive_interval(Some(Duration::from_secs(5)));
    transport.max_idle_timeout(Some(IDLE_TIMEOUT.try_into().expect("small")));
    Arc::new(transport)
}

/// A QUIC endpoint on the socket the candidates were gathered on.
pub(crate) fn endpoint(socket: UdpSocket, server: Option<quinn::ServerConfig>) -> Result<Endpoint> {
    socket.set_nonblocking(true)?;
    Ok(Endpoint::new(
        EndpointConfig::default(),
        server,
        socket,
        Arc::new(quinn::TokioRuntime),
    )?)
}

/// The friend's QUIC settings: TLS 1.3, accepting only the certificate whose SHA-256
/// is `fingerprint`.
pub(crate) fn client_config(fingerprint: [u8; 32]) -> Result<quinn::ClientConfig> {
    let tls_error = |err: rustls::Error| Error::Other(err.to_string());
    let mut crypto = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedCert {
            fingerprint,
            provider: provider(),
        }))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut client = quinn::ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|err| Error::Other(err.to_string()))?,
    ));
    client.transport_config(transport());
    Ok(client)
}

/// The host's QUIC settings, with its certificate and PKCS#8 key.
#[cfg(feature = "host")]
pub(crate) fn server_config(cert_der: &[u8], key_der: &[u8]) -> Result<quinn::ServerConfig> {
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    let tls_error = |err: rustls::Error| Error::Other(err.to_string());
    let mut crypto = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(tls_error)?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert_der.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der.to_vec())),
        )
        .map_err(tls_error)?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let mut server = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(crypto)
            .map_err(|err| Error::Other(err.to_string()))?,
    ));
    server.transport_config(transport());
    Ok(server)
}

/// What [`PinnedCert`] says about a certificate that isn't the host's.
pub(crate) const WRONG_HOST: &str = "not the host from the invite";

/// Accepts exactly the host's certificate, whose SHA-256 came in the invite, and checks
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
        let actual = fingerprint(end_entity);
        if bool::from(subtle::ConstantTimeEq::ct_eq(
            &actual[..],
            &self.fingerprint[..],
        )) {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(WRONG_HOST.into()))
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

/// The header a friend writes at the start of a stream.
pub(crate) fn header(token: &[u8; TOKEN_LEN], kind: StreamKind) -> [u8; TOKEN_LEN + 1] {
    let mut header = [0u8; TOKEN_LEN + 1];
    header[..TOKEN_LEN].copy_from_slice(token);
    header[TOKEN_LEN] = kind as u8;
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_kinds_are_one_byte() {
        for kind in [StreamKind::Game, StreamKind::Info, StreamKind::Hello] {
            assert_eq!(StreamKind::from_byte(kind as u8), Some(kind));
        }
        assert_eq!(StreamKind::from_byte(7), None);
        let header = header(&[3; TOKEN_LEN], StreamKind::Info);
        assert_eq!(header[TOKEN_LEN], 1);
    }
}
