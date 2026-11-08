//! Talking to a Lodestone core over its REST API (`/api/v1`).
//!
//! Signing in sends the username and password once, to `POST /user/login`. The session
//! token it returns is used for one request, `POST /user/api_keys`, which mints this
//! device's own API key with the core's fixed `launcher` scope (view, start and stop
//! instances), and is then dropped. Only the key is kept, in the Keychain.

use std::{sync::Arc, time::Duration};

use base64::Engine as _;
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use url::Url;

use super::models::{
    CoreInfo, CreatedKeyReply, ErrorBody, KeyInfo, LoginReply, ServerInstance, ServerList,
};
use crate::{Secret, USER_AGENT};

/// The core's default port.
pub const DEFAULT_PORT: u16 = 16662;
/// How long an API request may take.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a client pack download may take.
pub const PACK_TIMEOUT: Duration = Duration::from_secs(180);
/// JSON replies larger than this are refused.
pub const MAX_JSON_BYTES: u64 = 4 * 1024 * 1024;
/// Client packs larger than this are refused.
pub const MAX_PACK_BYTES: u64 = 512 * 1024 * 1024;

/// Why a request to a core failed. The messages are for people, and never contain a
/// password, token or key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    InvalidAddress(String),
    #[error("Couldn't reach the core. Check the address and that the core is running. ({0})")]
    Unreachable(String),
    #[error(
        "The core's HTTPS certificate isn't trusted by this Mac. Lodestone Launcher can't use a core's built-in self-signed certificate yet: connect over http:// on the core's own machine, or give the core a trusted certificate."
    )]
    Untrusted,
    #[error("Signed out: this device's key was revoked or has expired. Sign in again.")]
    SignedOut,
    #[error("Wrong username or password.")]
    BadCredentials,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(
        "This core is too old for Lodestone Launcher. Update Lodestone Core to a version with server listing."
    )]
    TooOld,
    #[error("{0}")]
    TooManyRequests(String),
    #[error("The core answered {status}: {message}")]
    Server { status: u16, message: String },
    #[error("The core sent a reply Lodestone Launcher doesn't understand: {0}")]
    Invalid(String),
}

impl CoreError {
    /// Errors that mean "not reachable right now", as opposed to a refusal.
    pub fn is_offline(&self) -> bool {
        matches!(self, CoreError::Unreachable(_) | CoreError::Untrusted)
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Reads a core address as people type it: `192.168.1.5`, `localhost:16662`,
/// `https://core.example.com`. Without a scheme, loopback addresses use `http` and
/// every other address `https`, as the dashboard does. Without a port, the core's
/// default port.
pub fn parse_address(input: &str) -> Result<Url, CoreError> {
    let input = input.trim();
    let invalid = |why: &str| CoreError::InvalidAddress(format!("{why}: {input:?}"));
    if input.is_empty() {
        return Err(CoreError::InvalidAddress("Enter the core's address".into()));
    }
    let (scheme, rest) = match input.split_once("://") {
        Some((_, rest)) if rest.trim_end_matches('/').is_empty() => {
            return Err(invalid("That isn't an address"));
        }
        Some((scheme, rest)) => (
            scheme.to_ascii_lowercase(),
            rest.trim_end_matches('/').to_string(),
        ),
        None => {
            let host = input
                .split('/')
                .next()
                .unwrap_or_default()
                .rsplit_once(':')
                .filter(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
                .map_or(input.split('/').next().unwrap_or_default(), |(host, _)| {
                    host
                });
            let scheme = if is_loopback_host(host) {
                "http"
            } else {
                "https"
            };
            (scheme.to_string(), input.trim_end_matches('/').to_string())
        }
    };
    if scheme != "http" && scheme != "https" {
        return Err(invalid("The address must start with http:// or https://"));
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let explicit_port = authority.rsplit_once(':').is_some_and(|(host, port)| {
        !port.is_empty()
            && port.chars().all(|c| c.is_ascii_digit())
            && (!host.contains(':') || host.ends_with(']'))
    });
    let mut url =
        Url::parse(&format!("{scheme}://{rest}")).map_err(|_| invalid("That isn't an address"))?;
    if url.host_str().is_none_or(str::is_empty) {
        return Err(invalid("That isn't an address"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid(
            "Leave the username and password out of the address",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid("That isn't a core address"));
    }
    if !explicit_port && url.port().is_none() {
        url.set_port(Some(DEFAULT_PORT))
            .map_err(|()| invalid("That isn't an address"))?;
    }
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&path);
    Ok(url)
}

/// The address as shown: `http://localhost:16662`.
pub fn display_address(url: &Url) -> String {
    url.as_str().trim_end_matches('/').to_string()
}

/// A client pack request's answer.
pub enum PackFetch {
    /// The pack is the one with the ETag that was sent.
    NotModified,
    Pack {
        bytes: Vec<u8>,
        etag: Option<String>,
    },
    /// The server needs no client mods (or is not a Fabric server).
    NoMods(String),
}

struct RawResponse {
    status: u16,
    etag: Option<String>,
    body: Vec<u8>,
}

/// A core's API at one address.
#[derive(Clone)]
pub struct CoreClient {
    http: Arc<dyn HttpClient>,
    base: Url,
}

impl CoreClient {
    /// `base` is the core's address from [`parse_address`].
    pub fn new(http: Arc<dyn HttpClient>, base: Url) -> Self {
        Self { http, base }
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/api/v1/{}",
            self.base.as_str().trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    #[allow(clippy::too_many_arguments)]
    async fn send(
        &self,
        method: Method,
        path: &str,
        auth: Auth<'_>,
        body: Option<Vec<u8>>,
        if_none_match: Option<&str>,
        limit: u64,
        timeout: Duration,
    ) -> Result<RawResponse, CoreError> {
        let mut builder = Request::builder()
            .method(method)
            .uri(self.url(path))
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .follow_redirects(RedirectPolicy::NoFollow)
            .timeout(timeout);
        match auth {
            Auth::None => {}
            Auth::Bearer(token) => {
                builder = builder.header("Authorization", format!("Bearer {}", token.expose()));
            }
            Auth::Basic(username, password) => {
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{}", password.expose()));
                builder = builder.header("Authorization", format!("Basic {encoded}"));
            }
        }
        if let Some(etag) = if_none_match {
            builder = builder.header("If-None-Match", etag);
        }
        let request = match body {
            Some(body) => builder
                .header("Content-Type", "application/json")
                .body(AsyncBody::from(body)),
            None => builder.body(AsyncBody::empty()),
        }
        .map_err(|err| CoreError::InvalidAddress(err.to_string()))?;
        let response = self
            .http
            .send(request)
            .await
            .map_err(|err| transport_error(&err))?;
        let status = response.status().as_u16();
        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let mut body = Vec::new();
        response
            .into_body()
            .take(limit + 1)
            .read_to_end(&mut body)
            .await
            .map_err(|err| CoreError::Unreachable(err.to_string()))?;
        if body.len() as u64 > limit {
            return Err(CoreError::Invalid(format!(
                "the reply is larger than {} MB",
                limit / (1024 * 1024)
            )));
        }
        Ok(RawResponse { status, etag, body })
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        auth: Auth<'_>,
        body: Option<Vec<u8>>,
    ) -> Result<T, CoreError> {
        let response = self
            .send(
                method,
                path,
                auth,
                body,
                None,
                MAX_JSON_BYTES,
                REQUEST_TIMEOUT,
            )
            .await?;
        check(&response, auth)?;
        serde_json::from_slice(&response.body).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `GET /info`: the core's name, id and version. Needs no sign-in.
    pub async fn info(&self) -> Result<CoreInfo, CoreError> {
        self.json(Method::GET, "info", Auth::None, None).await
    }

    /// The instances anyone may see. An empty list with `listing_enabled: false` while
    /// the owner hasn't turned listing on.
    pub async fn public_servers(&self) -> Result<ServerList, CoreError> {
        self.json(Method::GET, "public/instances", Auth::None, None)
            .await
            .map_err(|err| match err {
                CoreError::NotFound(_) => CoreError::TooOld,
                err => err,
            })
    }

    /// The instances `key` can view.
    pub async fn servers(&self, key: &Secret) -> Result<ServerList, CoreError> {
        self.json(Method::GET, "launcher/instances", Auth::Bearer(key), None)
            .await
            .map_err(|err| match err {
                CoreError::NotFound(_) => CoreError::TooOld,
                err => err,
            })
    }

    /// Signs in with a username and password and mints this device's key, named
    /// `key_name`, expiring at `expires_at` (unix seconds) or never. The session token
    /// is dropped once the key exists.
    pub async fn sign_in(
        &self,
        username: &str,
        password: &Secret,
        key_name: &str,
        expires_at: Option<i64>,
    ) -> Result<(Secret, KeyInfo), CoreError> {
        let login: LoginReply = self
            .json(
                Method::POST,
                "user/login",
                Auth::Basic(username, password),
                None,
            )
            .await?;
        let session = Secret::new(login.token);
        let body = serde_json::json!({
            "name": key_name,
            "expires_at": expires_at,
            "scope": { "type": "launcher" },
        });
        let created: CreatedKeyReply = self
            .json(
                Method::POST,
                "user/api_keys",
                Auth::Bearer(&session),
                Some(body.to_string().into_bytes()),
            )
            .await
            .map_err(|err| match err {
                // a session token that stops working at once is not this device's key
                CoreError::SignedOut => CoreError::BadCredentials,
                err => err,
            })?;
        Ok((Secret::new(created.key), created.info))
    }

    async fn put(&self, key: &Secret, path: &str) -> Result<(), CoreError> {
        let response = self
            .send(
                Method::PUT,
                path,
                Auth::Bearer(key),
                None,
                None,
                MAX_JSON_BYTES,
                REQUEST_TIMEOUT,
            )
            .await?;
        check(&response, Auth::Bearer(key))
    }

    /// Starts a server. It runs once the list says `Running`.
    pub async fn start(&self, key: &Secret, uuid: &str) -> Result<(), CoreError> {
        self.put(key, &format!("instance/{}/start", segment(uuid)?))
            .await
    }

    pub async fn stop(&self, key: &Secret, uuid: &str) -> Result<(), CoreError> {
        self.put(key, &format!("instance/{}/stop", segment(uuid)?))
            .await
    }

    /// The server's client modpack: with `key` through the signed-in route (any
    /// instance the key can view), else through the public one (listed instances).
    /// `etag` is the one of the pack the instance has.
    pub async fn client_pack(
        &self,
        key: Option<&Secret>,
        server: &ServerInstance,
        etag: Option<&str>,
    ) -> Result<PackFetch, CoreError> {
        let uuid = segment(&server.uuid)?;
        let (path, auth) = match key {
            Some(key) => (
                format!("launcher/instances/{uuid}/client-pack"),
                Auth::Bearer(key),
            ),
            None => (format!("public/instances/{uuid}/client-pack"), Auth::None),
        };
        let response = self
            .send(
                Method::GET,
                &path,
                auth,
                None,
                etag,
                MAX_PACK_BYTES,
                PACK_TIMEOUT,
            )
            .await?;
        match response.status {
            304 => Ok(PackFetch::NotModified),
            400 => Ok(PackFetch::NoMods(message(&response))),
            _ => {
                check(&response, auth)?;
                Ok(PackFetch::Pack {
                    bytes: response.body,
                    etag: response.etag,
                })
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Auth<'a> {
    None,
    Bearer(&'a Secret),
    Basic(&'a str, &'a Secret),
}

/// An instance UUID as one path segment.
fn segment(uuid: &str) -> Result<&str, CoreError> {
    if !uuid.is_empty()
        && uuid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(uuid)
    } else {
        Err(CoreError::Invalid(format!("{uuid:?} isn't an instance id")))
    }
}

fn message(response: &RawResponse) -> String {
    serde_json::from_slice::<ErrorBody>(&response.body)
        .ok()
        .and_then(|body| body.causes.into_iter().next())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            String::from_utf8_lossy(&response.body)
                .chars()
                .take(300)
                .collect()
        })
}

fn check(response: &RawResponse, auth: Auth<'_>) -> Result<(), CoreError> {
    let status = response.status;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let message = message(response);
    Err(match status {
        401 => match auth {
            Auth::Bearer(_) => CoreError::SignedOut,
            Auth::Basic(..) => CoreError::BadCredentials,
            Auth::None => CoreError::Server { status, message },
        },
        403 => CoreError::Forbidden(if message.is_empty() {
            "The core refused: you don't have permission to do that.".into()
        } else {
            message
        }),
        404 => CoreError::NotFound(if message.is_empty() {
            "Not found".into()
        } else {
            message
        }),
        429 => CoreError::TooManyRequests(if message.is_empty() {
            "Too many attempts. Try again in a few minutes.".into()
        } else {
            message
        }),
        _ => CoreError::Server { status, message },
    })
}

fn transport_error(err: &anyhow::Error) -> CoreError {
    let text = format!("{err:#}");
    let lower = text.to_ascii_lowercase();
    if lower.contains("certificate") || lower.contains("unknownissuer") {
        CoreError::Untrusted
    } else {
        CoreError::Unreachable(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_addresses_as_people_type_them() {
        let ok = |input: &str| parse_address(input).unwrap().to_string();
        assert_eq!(ok("localhost"), "http://localhost:16662/");
        assert_eq!(ok("127.0.0.1:16700"), "http://127.0.0.1:16700/");
        assert_eq!(ok("[::1]"), "http://[::1]:16662/");
        assert_eq!(ok("192.168.1.5"), "https://192.168.1.5:16662/");
        assert_eq!(ok("core.example.com/"), "https://core.example.com:16662/");
        assert_eq!(ok("http://192.168.1.5"), "http://192.168.1.5:16662/");
        // an explicit default port is kept
        assert_eq!(
            ok("https://core.example.com:443"),
            "https://core.example.com/"
        );
        assert_eq!(
            ok("https://example.com/lodestone/"),
            "https://example.com:16662/lodestone"
        );
        for bad in [
            "",
            "ftp://x",
            "http://user:pw@host",
            "http://h/?a=1",
            "http://",
        ] {
            assert!(parse_address(bad).is_err(), "{bad}");
        }
        assert_eq!(
            display_address(&parse_address("localhost").unwrap()),
            "http://localhost:16662"
        );
    }

    #[test]
    fn instance_ids_are_one_segment() {
        assert!(segment("a1-B2_c3").is_ok());
        assert!(segment("../x").is_err());
        assert!(segment("").is_err());
    }

    #[test]
    fn certificate_errors_are_untrusted() {
        assert_eq!(
            transport_error(&anyhow::anyhow!("invalid peer certificate: UnknownIssuer")),
            CoreError::Untrusted
        );
        assert!(matches!(
            transport_error(&anyhow::anyhow!("connection refused")),
            CoreError::Unreachable(_)
        ));
    }
}
