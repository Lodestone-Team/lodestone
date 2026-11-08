//! Downloading a resource pack URL to compute its SHA-1, without letting the URL reach
//! the core's own machine or network.
//!
//! - Only `http` and `https`, without a username or password in the URL.
//! - The host is resolved once per request and every address must be public (unless
//!   `allow_private`, for the owner and admins). The connection goes to the checked
//!   addresses only, so a second DNS answer can't point it elsewhere. No proxy is used.
//! - Redirects are followed by hand, at most [`MAX_REDIRECTS`], each one checked again.
//! - At most `max_bytes` are read; only the hash and the size are returned, never the
//!   body.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use color_eyre::eyre::eyre;
use futures::StreamExt;
use sha1::{Digest, Sha1};

use super::ResourcePackHash;
use crate::error::{Error, ErrorKind};

/// Minecraft refuses server packs larger than 250 MB.
pub const MAX_PACK_BYTES: u64 = 250 * 1024 * 1024;
pub const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(180);

fn refused(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

fn external(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::External,
        source: eyre!(message.into()),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        // shared address space (carrier-grade NAT)
        || (a == 100 && (64..128).contains(&b))
        // IETF protocol assignments
        || (a == 192 && b == 0 && c == 0)
        // benchmarking
        || (a == 198 && (b == 18 || b == 19))
        // reserved
        || a >= 240)
}

/// Whether `ip` is an address on the public internet.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let segments = ip.segments();
            // NAT64 and IPv4-compatible addresses carry an IPv4 address
            if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] || segments[..6] == [0; 6] {
                let v4 = Ipv4Addr::new(
                    (segments[6] >> 8) as u8,
                    segments[6] as u8,
                    (segments[7] >> 8) as u8,
                    segments[7] as u8,
                );
                return !ip.is_loopback() && !ip.is_unspecified() && is_public_v4(v4);
            }
            !(ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                // unique local fc00::/7
                || (segments[0] & 0xfe00) == 0xfc00
                // link-local fe80::/10, and the old site-local fec0::/10
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] & 0xffc0) == 0xfec0
                // documentation 2001:db8::/32
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

/// Checks `url` and resolves its host. Every address must be allowed.
async fn resolve(url: &url::Url, allow_private: bool) -> Result<Vec<SocketAddr>, Error> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(refused("Only http:// and https:// URLs can be downloaded"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(refused("Leave the username and password out of the URL"));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| refused("The URL has no port"))?;
    let addrs: Vec<SocketAddr> = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Some(url::Host::Domain(domain)) => tokio::net::lookup_host((domain, port))
            .await
            .map_err(|_| external(format!("Couldn't find {domain}")))?
            .collect(),
        None => return Err(refused("The URL has no host")),
    };
    if addrs.is_empty() {
        return Err(external("The URL's host has no address"));
    }
    if !allow_private && addrs.iter().any(|a| !is_public(a.ip())) {
        return Err(refused(
            "The URL points at this machine or a private network. Only the owner and admins can use such URLs",
        ));
    }
    Ok(addrs)
}

/// Downloads `url` (following checked redirects) and returns its SHA-1 and size.
pub async fn hash_url(
    url: &str,
    allow_private: bool,
    max_bytes: u64,
) -> Result<ResourcePackHash, Error> {
    let mut url = url::Url::parse(url).map_err(|_| refused("The URL is not valid"))?;
    for _ in 0..=MAX_REDIRECTS {
        let addrs = resolve(&url, allow_private).await?;
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .user_agent(concat!("Lodestone Core/", env!("CARGO_PKG_VERSION")));
        if let Some(url::Host::Domain(domain)) = url.host() {
            builder = builder.resolve_to_addrs(domain, &addrs);
        }
        let client = builder
            .build()
            .map_err(|e| external(format!("Couldn't start the download: {e}")))?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| external(format!("Couldn't download the resource pack: {e}")))?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|l| l.to_str().ok())
                .ok_or_else(|| external("The server sent a redirect without a location"))?;
            url = url
                .join(location)
                .map_err(|_| external("The server sent an invalid redirect"))?;
            continue;
        }
        if !status.is_success() {
            return Err(external(format!("The resource pack URL answered {status}")));
        }
        let too_large = || {
            refused(format!(
                "The resource pack is larger than {} MB",
                max_bytes / (1024 * 1024)
            ))
        };
        if response.content_length().is_some_and(|len| len > max_bytes) {
            return Err(too_large());
        }
        let mut hasher = Sha1::new();
        let mut size = 0u64;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| external(format!("The download stopped: {e}")))?;
            size += chunk.len() as u64;
            if size > max_bytes {
                return Err(too_large());
            }
            hasher.update(&chunk);
        }
        return Ok(ResourcePackHash {
            sha1: hex::encode(hasher.finalize()),
            size,
        });
    }
    Err(external("The resource pack URL redirects too often"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_are_public() {
        let public = |s: &str| is_public(s.parse().unwrap());
        for ok in [
            "1.1.1.1",
            "8.8.8.8",
            "2606:4700:4700::1111",
            "::ffff:1.1.1.1",
        ] {
            assert!(public(ok), "{ok}");
        }
        for bad in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.5.4",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "198.18.0.1",
            "255.255.255.255",
            "224.0.2.60",
            "240.0.0.1",
            "::1",
            "::",
            "fe80::1",
            "fd00::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
            "64:ff9b::a00:1",
            "::7f00:1",
            "2001:db8::1",
        ] {
            assert!(!public(bad), "{bad}");
        }
    }

    /// Serves `body` at `/pack.zip`, and a redirect to it at `/go`.
    async fn serve(body: Vec<u8>) -> String {
        use axum::routing::get;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = axum::Router::new()
            .route("/pack.zip", get(move || async move { body.clone() }))
            .route(
                "/go",
                get(|| async { axum::response::Redirect::temporary("/pack.zip") }),
            )
            .route(
                "/loop",
                get(|| async { axum::response::Redirect::temporary("/loop") }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await });
        base
    }

    #[tokio::test]
    async fn hashes_a_pack_and_follows_redirects() {
        let base = serve(b"pack bytes".to_vec()).await;
        let expected = hex::encode(Sha1::digest(b"pack bytes"));
        for path in ["/pack.zip", "/go"] {
            let hash = hash_url(&format!("{base}{path}"), true, 1024)
                .await
                .unwrap();
            assert_eq!(hash.sha1, expected);
            assert_eq!(hash.size, 10);
        }
        let err = hash_url(&format!("{base}/loop"), true, 1024)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("redirects too often"), "{err}");
    }

    #[tokio::test]
    async fn private_addresses_need_the_owner() {
        let base = serve(b"x".to_vec()).await;
        let err = hash_url(&format!("{base}/pack.zip"), false, 1024)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert!(err.to_string().contains("private network"), "{err}");
        // a name that resolves to loopback is refused too
        let port = base.rsplit(':').next().unwrap();
        let err = hash_url(&format!("http://localhost:{port}/pack.zip"), false, 1024)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("private network"), "{err}");
    }

    #[tokio::test]
    async fn refuses_large_packs_and_other_schemes() {
        let base = serve(vec![7u8; 4096]).await;
        let err = hash_url(&format!("{base}/pack.zip"), true, 1024)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("larger than"), "{err}");
        for bad in [
            "file:///etc/passwd",
            "ftp://example.com/x",
            "http://u:p@example.com/x",
        ] {
            assert!(hash_url(bad, true, 1024).await.is_err(), "{bad}");
        }
    }
}
