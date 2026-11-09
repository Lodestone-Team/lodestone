//! Certificate pinning for HTTPS cores whose certificate this Mac doesn't trust, such as
//! the self-signed certificate a Lodestone core makes for itself.
//!
//! Requests to a core go through Zed's client with the platform verifier first, so a
//! certificate macOS trusts keeps working as it always did and is never pinned. When
//! macOS refuses the certificate, the user is shown its SHA-256 fingerprint and can
//! compare it with the one the core shows (Core Settings → Information in the dashboard,
//! or the core's log at startup). Once they trust it, the fingerprint and the address's
//! host are kept with the core in `servers.json` ([`CertificatePin`]).
//!
//! A pinned core is reached through its own client ([`PinnedHttp`]), whose verifier
//! ([`PinVerifier`]) accepts exactly the pinned certificate, and only when it is
//! presented for the pinned host. It doesn't look at the certificate's names, issuer or
//! dates: the user decided to trust this certificate. The handshake signatures are still
//! checked, so the server must hold the certificate's private key. Any other certificate
//! is refused, and the verifier remembers what was presented so the launcher can show
//! both fingerprints. Verification is never turned off in general.

use std::sync::{Arc, Mutex};

use http_client::HttpClient;
use rustls::{
    CertificateError, DigitallySignedStruct, Error as TlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::USER_AGENT;

/// A certificate the user trusted for a core: kept in `servers.json`. Fingerprints are
/// public (every handshake sends the certificate); no private key is ever involved.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CertificatePin {
    /// The host the certificate is accepted for: the host of the core's address
    /// (`192.168.1.5`, `core.local`, `::1`).
    pub host: String,
    /// SHA-256 of the certificate (DER), as colon-separated uppercase hex, the way the
    /// core and browsers show it.
    pub sha256: String,
}

impl CertificatePin {
    pub fn new(host: &str, sha256: &str) -> Self {
        Self {
            host: normalize_host(host),
            sha256: format_fingerprint(sha256),
        }
    }

    /// Whether this pin accepts `sha256` presented for `host`.
    pub fn accepts(&self, host: &str, sha256: &str) -> bool {
        normalize_host(&self.host) == normalize_host(host) && same_fingerprint(&self.sha256, sha256)
    }
}

/// The SHA-256 fingerprint of a DER certificate: `AB:CD:…` (95 characters).
pub fn fingerprint(der: &[u8]) -> String {
    sha2::Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn fingerprint_hex(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A fingerprint in the core's format, whatever separators it was written with.
pub fn format_fingerprint(value: &str) -> String {
    let hex = fingerprint_hex(value);
    hex.as_bytes()
        .chunks(2)
        .map(|pair| String::from_utf8_lossy(pair).into_owned())
        .collect::<Vec<_>>()
        .join(":")
}

/// Compares two fingerprints, ignoring case and separators. An empty one never matches.
pub fn same_fingerprint(a: &str, b: &str) -> bool {
    let (a, b) = (fingerprint_hex(a), fingerprint_hex(b));
    a.len() == 64 && a == b
}

/// `[::1]` and `::1`, `Core.Local` and `core.local` are the same host.
pub fn normalize_host(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// The certificate a server presented to a [`PinVerifier`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presented {
    pub host: String,
    pub sha256: String,
}

/// A rustls verifier that accepts only one pinned certificate, for one host (or nothing,
/// without a pin: then it only records what the server presented).
#[derive(Debug)]
pub struct PinVerifier {
    pin: Option<CertificatePin>,
    provider: Arc<CryptoProvider>,
    presented: Mutex<Option<Presented>>,
}

impl PinVerifier {
    pub fn new(pin: Option<CertificatePin>, provider: Arc<CryptoProvider>) -> Self {
        Self {
            pin,
            provider,
            presented: Mutex::new(None),
        }
    }

    /// The last certificate a server presented, taken.
    pub fn take_presented(&self) -> Option<Presented> {
        self.presented
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
    }
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let presented = Presented {
            host: normalize_host(&server_name.to_str()),
            sha256: fingerprint(end_entity.as_ref()),
        };
        let accepted = self
            .pin
            .as_ref()
            .is_some_and(|pin| pin.accepts(&presented.host, &presented.sha256));
        *self.presented.lock().unwrap_or_else(|p| p.into_inner()) = Some(presented);
        if accepted {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// An HTTP client for one core that trusts only its pinned certificate (or, without a
/// pin, nothing: a probe that learns which certificate a server presents).
pub struct PinnedHttp {
    http: Arc<dyn HttpClient>,
    verifier: Arc<PinVerifier>,
}

impl PinnedHttp {
    pub fn new(pin: Option<CertificatePin>) -> anyhow::Result<Self> {
        // the provider Zed's own TLS configuration uses
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let verifier = Arc::new(PinVerifier::new(pin, provider.clone()));
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(verifier.clone())
            .with_no_client_auth();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(USER_AGENT),
        );
        // the connection settings of Zed's `ReqwestClient`
        let client = reqwest::Client::builder()
            .use_preconfigured_tls(config)
            .default_headers(headers)
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(30))
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http: Arc::new(reqwest_client::ReqwestClient::from(client)),
            verifier,
        })
    }

    pub fn pin(&self) -> Option<&CertificatePin> {
        self.verifier.pin.as_ref()
    }

    pub fn http(&self) -> Arc<dyn HttpClient> {
        self.http.clone()
    }

    pub fn take_presented(&self) -> Option<Presented> {
        self.verifier.take_presented()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_compare_in_any_format() {
        let fp = fingerprint(b"abc");
        assert_eq!(fp.len(), 32 * 3 - 1);
        assert_eq!(&fp[..8], "BA:78:16");
        assert!(same_fingerprint(&fp, &fp.replace(':', "").to_lowercase()));
        assert_eq!(format_fingerprint(&fp.replace(':', " ")), fp);
        assert!(!same_fingerprint("", ""));
        assert!(!same_fingerprint(&fp, &fingerprint(b"abd")));
        let pin = CertificatePin::new("[::1]", &fp);
        assert_eq!(pin.host, "::1");
        assert!(pin.accepts("::1", &fp));
        assert!(pin.accepts("[::1]", &fp.to_lowercase()));
        assert!(!pin.accepts("127.0.0.1", &fp));
        assert!(CertificatePin::new("Core.Local.", &fp).accepts("core.local", &fp));
    }
}
