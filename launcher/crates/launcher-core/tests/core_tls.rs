//! Certificate pinning against a loopback HTTPS core with a self-signed certificate.
//!
//! `testdata/tls/` holds two self-signed certificates for `localhost` and `127.0.0.1`
//! (ECDSA P-256, made with `openssl req -x509`) and their keys, which exist only for these
//! tests.

use std::{
    io::{Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpListener},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::executor::block_on;
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        tls::{CertificatePin, PinnedHttp, fingerprint},
    },
    http,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

const CERT_A: &[u8] = include_bytes!("../testdata/tls/cert-a.der");
const KEY_A: &[u8] = include_bytes!("../testdata/tls/key-a.der");
const CERT_B: &[u8] = include_bytes!("../testdata/tls/cert-b.der");
const KEY_B: &[u8] = include_bytes!("../testdata/tls/key-b.der");

/// A core that answers `/api/v1/info` over HTTPS with one certificate, and counts the
/// requests that got through the handshake.
struct TlsCore {
    port: u16,
    requests: Arc<AtomicUsize>,
}

impl TlsCore {
    fn start(cert: &[u8], key: &[u8]) -> Self {
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec())),
        )
        .unwrap();
        let config = Arc::new(config);
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(AtomicUsize::new(0));
        {
            let requests = requests.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let config = config.clone();
                    let requests = requests.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                        let Ok(connection) = rustls::ServerConnection::new(config) else {
                            return;
                        };
                        let mut tls = rustls::StreamOwned::new(connection, stream);
                        let mut request = Vec::new();
                        let mut buf = [0u8; 4096];
                        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                            match tls.read(&mut buf) {
                                Ok(0) | Err(_) => return,
                                Ok(n) => request.extend_from_slice(&buf[..n]),
                            }
                        }
                        requests.fetch_add(1, Ordering::SeqCst);
                        let body = r#"{"version":"0.5.1","uuid":"core-1","core_name":"TLS core","is_setup":true}"#;
                        let _ = write!(
                            tls,
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = tls.flush();
                        tls.conn.send_close_notify();
                        let _ = tls.flush();
                    });
                }
            });
        }
        Self { port, requests }
    }

    fn client(&self, host: &str, pin: Option<CertificatePin>) -> CoreClient {
        let client = CoreClient::new(
            http::default_client().unwrap(),
            url::Url::parse(&format!("https://{host}:{}", self.port)).unwrap(),
        );
        match pin {
            Some(pin) => client.with_pinned(Arc::new(PinnedHttp::new(Some(pin)).unwrap())),
            None => client,
        }
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

#[test]
fn an_unpinned_self_signed_certificate_is_refused_with_its_fingerprint() {
    let core = TlsCore::start(CERT_A, KEY_A);
    let client = core.client("127.0.0.1", None);
    assert_eq!(
        block_on(client.info()).unwrap_err(),
        CoreError::Untrusted {
            host: "127.0.0.1".into(),
            fingerprint: Some(fingerprint(CERT_A)),
        }
    );
    // signing in sends nothing either
    let err = block_on(client.sign_in("steve", &Secret::new("hunter2"), "x", None)).unwrap_err();
    assert!(matches!(err, CoreError::Untrusted { .. }), "{err:?}");
    assert!(err.is_certificate());
    assert_eq!(
        core.requests(),
        0,
        "no request may cross an untrusted connection"
    );
    // the probe on its own
    assert_eq!(
        block_on(client.probe_fingerprint()),
        Some(fingerprint(CERT_A))
    );
    assert_eq!(core.requests(), 0);
}

#[test]
fn the_pinned_certificate_is_accepted() {
    let core = TlsCore::start(CERT_A, KEY_A);
    let pin = CertificatePin::new("127.0.0.1", &fingerprint(CERT_A));
    let client = core.client("127.0.0.1", Some(pin));
    let info = block_on(client.info()).unwrap();
    assert_eq!(info.core_name, "TLS core");
    assert_eq!(core.requests(), 1);
    // and again, on the same client
    assert_eq!(block_on(client.info()).unwrap().uuid, "core-1");
}

#[test]
fn a_changed_certificate_is_refused_with_both_fingerprints() {
    let core = TlsCore::start(CERT_B, KEY_B);
    let pin = CertificatePin::new("127.0.0.1", &fingerprint(CERT_A));
    let client = core.client("127.0.0.1", Some(pin));
    let err = block_on(client.info()).unwrap_err();
    assert_eq!(
        err,
        CoreError::CertificateChanged {
            host: "127.0.0.1".into(),
            pinned: fingerprint(CERT_A),
            presented: fingerprint(CERT_B),
        }
    );
    assert!(err.to_string().contains("changed"));
    assert_eq!(core.requests(), 0);
}

#[test]
fn a_pinned_certificate_is_not_accepted_for_another_host() {
    let core = TlsCore::start(CERT_A, KEY_A);
    // pinned for 127.0.0.1, presented for localhost (the certificate names both)
    let pin = CertificatePin::new("127.0.0.1", &fingerprint(CERT_A));
    let client = core.client("localhost", Some(pin));
    assert_eq!(
        block_on(client.info()).unwrap_err(),
        CoreError::Untrusted {
            host: "localhost".into(),
            fingerprint: Some(fingerprint(CERT_A)),
        }
    );
    assert_eq!(core.requests(), 0);
    // pinned for localhost, it works there
    let pin = CertificatePin::new("localhost", &fingerprint(CERT_A));
    assert!(block_on(core.client("localhost", Some(pin)).info()).is_ok());
}
