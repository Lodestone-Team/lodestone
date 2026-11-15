//! Self-signed certificates for hosts (an invite host's, a core's for friends, a
//! launcher's for its LAN worlds): an ECDSA P-256 key from `ring` and a small X.509 v3
//! certificate written here, so hosting needs no certificate crate. Friends pin the
//! certificate's SHA-256 ([`crate::tunnel::fingerprint`]); nothing checks its names or
//! dates, but TLS still parses it and checks the handshake's signature with its key.

use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair as _, ECDSA_P256_SHA256_ASN1_SIGNING},
};

use crate::{Error, Result};

/// A certificate (DER) and its PKCS#8 key (DER).
pub(crate) struct SelfSigned {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

/// ecdsa-with-SHA256 (1.2.840.10045.4.3.2)
const ECDSA_SHA256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
/// id-ecPublicKey (1.2.840.10045.2.1)
const EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
/// prime256v1 (1.2.840.10045.3.1.7)
const P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
/// commonName (2.5.4.3)
const COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];
/// subjectAltName (2.5.29.17)
const SUBJECT_ALT_NAME: &[u8] = &[0x55, 0x1d, 0x11];

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes: Vec<u8> = len
            .to_be_bytes()
            .into_iter()
            .skip_while(|b| *b == 0)
            .collect();
        out.push(0x80 | bytes.len() as u8);
        out.extend_from_slice(&bytes);
    }
    out.extend_from_slice(content);
    out
}

fn seq(parts: &[&[u8]]) -> Vec<u8> {
    der(0x30, &parts.concat())
}

fn oid(id: &[u8]) -> Vec<u8> {
    der(0x06, id)
}

fn bit_string(bytes: &[u8]) -> Vec<u8> {
    let mut content = vec![0u8]; // no unused bits
    content.extend_from_slice(bytes);
    der(0x03, &content)
}

/// A name with one common name.
fn name(common_name: &str) -> Vec<u8> {
    let attribute = seq(&[&oid(COMMON_NAME), &der(0x0c, common_name.as_bytes())]);
    seq(&[&der(0x31, &attribute)])
}

/// A new key and a certificate for `dns_name`, valid from 1975 to the end of 9999 (as
/// rcgen made them): nothing checks the dates, only the fingerprint.
pub(crate) fn self_signed(dns_name: &str) -> Result<SelfSigned> {
    let failed = || Error::Other("Couldn't make a certificate".into());
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
        .map_err(|_| failed())?;
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
        .map_err(|_| failed())?;

    // a positive serial number: 16 random bytes, the first below 0x80
    let mut serial: [u8; 16] = crate::random()?;
    serial[0] = (serial[0] & 0x7f) | 0x01;
    let algorithm = seq(&[&oid(ECDSA_SHA256)]);
    let validity = seq(&[
        &der(0x17, b"750101000000Z"),   // UTCTime
        &der(0x18, b"99991231235959Z"), // GeneralizedTime
    ]);
    let public_key = seq(&[
        &seq(&[&oid(EC_PUBLIC_KEY), &oid(P256)]),
        &bit_string(key.public_key().as_ref()),
    ]);
    let alt_names = seq(&[&der(0x82, dns_name.as_bytes())]); // [2] dNSName
    let extensions = der(
        0xa3, // [3] EXPLICIT
        &seq(&[&seq(&[&oid(SUBJECT_ALT_NAME), &der(0x04, &alt_names)])]),
    );
    let subject = name(dns_name);
    let tbs = seq(&[
        &der(0xa0, &der(0x02, &[2])), // [0] EXPLICIT version: v3
        &der(0x02, &serial),
        &algorithm,
        &subject, // issuer: itself
        &validity,
        &subject,
        &public_key,
        &extensions,
    ]);
    let signature = key.sign(&rng, &tbs).map_err(|_| failed())?;
    let cert_der = seq(&[&tbs, &algorithm, &bit_string(signature.as_ref())]);
    Ok(SelfSigned {
        cert_der,
        key_der: pkcs8.as_ref().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_are_encoded_as_der_wants() {
        assert_eq!(der(0x04, &[1, 2]), vec![0x04, 2, 1, 2]);
        let long = der(0x04, &[0; 200]);
        assert_eq!(&long[..3], &[0x04, 0x81, 200]);
        let longer = der(0x04, &[0; 300]);
        assert_eq!(&longer[..4], &[0x04, 0x82, 0x01, 0x2c]);
    }

    #[test]
    fn certificates_are_new_each_time_and_differ() {
        let a = self_signed("lodestone-connect").unwrap();
        let b = self_signed("lodestone-connect").unwrap();
        assert_ne!(a.cert_der, b.cert_der);
        assert_ne!(a.key_der, b.key_der);
        // a sequence that holds the whole certificate
        assert_eq!(a.cert_der[0], 0x30);
    }

    /// A whole QUIC handshake on loopback: the server loads the key and certificate,
    /// and the client (pinning its fingerprint) parses it and checks the handshake's
    /// signature with its key.
    #[tokio::test]
    async fn quic_handshakes_with_the_certificate() {
        use crate::tunnel;
        let made = self_signed(tunnel::SERVER_NAME).unwrap();
        let server =
            tunnel::server_config_for(&made.cert_der, &made.key_der, tunnel::ALPN).unwrap();
        let server = tunnel::endpoint(
            std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
            Some(server),
        )
        .unwrap();
        let at = server.local_addr().unwrap();
        let accept = tokio::spawn(async move {
            let connection = server.accept().await.unwrap().await.unwrap();
            connection.closed().await;
        });
        let mut client =
            tunnel::endpoint(std::net::UdpSocket::bind("127.0.0.1:0").unwrap(), None).unwrap();
        client.set_default_client_config(
            tunnel::client_config_for(tunnel::fingerprint(&made.cert_der), tunnel::ALPN).unwrap(),
        );
        let connection = client
            .connect(at, tunnel::SERVER_NAME)
            .unwrap()
            .await
            .expect("the handshake works with the certificate");
        connection.close(0u32.into(), b"");
        accept.await.unwrap();

        // another certificate isn't taken in its place
        let other = self_signed(tunnel::SERVER_NAME).unwrap();
        client.set_default_client_config(
            tunnel::client_config_for(tunnel::fingerprint(&other.cert_der), tunnel::ALPN).unwrap(),
        );
        let server2 = tunnel::endpoint(
            std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
            Some(tunnel::server_config_for(&made.cert_der, &made.key_der, tunnel::ALPN).unwrap()),
        )
        .unwrap();
        let at2 = server2.local_addr().unwrap();
        tokio::spawn(async move {
            if let Some(incoming) = server2.accept().await {
                let _ = incoming.await;
            }
        });
        assert!(client
            .connect(at2, tunnel::SERVER_NAME)
            .unwrap()
            .await
            .is_err());
    }
}
