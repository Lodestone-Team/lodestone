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

use super::{
    models::{
        CoreInfo, CreatedKeyReply, ErrorBody, KeyInfo, LoginReply, ServerInstance, ServerList,
    },
    tls::{PinnedHttp, normalize_host, same_fingerprint},
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
    /// macOS doesn't trust the certificate, and no pin accepts it. `fingerprint` is the
    /// presented certificate's, when it could be read.
    #[error(
        "The HTTPS certificate of {host} isn't trusted by this {computer}. Check its fingerprint, then trust it to connect.",
        computer = crate::wording::COMPUTER
    )]
    Untrusted {
        host: String,
        fingerprint: Option<String>,
    },
    /// The core presents another certificate than the one the user trusted.
    #[error(
        "The HTTPS certificate of {host} changed since you trusted it, so {app} refuses to connect. Check the new fingerprint before you trust it.",
        app = crate::wording::app_name()
    )]
    CertificateChanged {
        host: String,
        pinned: String,
        presented: String,
    },
    #[error("Signed out: this device's key was revoked or has expired. Sign in again.")]
    SignedOut,
    #[error("Wrong username or password.")]
    BadCredentials,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error(
        "This core is too old for {app}. Update Lodestone Core to a version with server listing.",
        app = crate::wording::app_name()
    )]
    TooOld,
    #[error("{0}")]
    TooManyRequests(String),
    #[error("The core answered {status}: {message}")]
    Server { status: u16, message: String },
    #[error(
        "The core sent a reply {app} doesn't understand: {0}",
        app = crate::wording::app_name()
    )]
    Invalid(String),
}

impl CoreError {
    /// Errors that mean "not reachable right now", as opposed to a refusal.
    pub fn is_offline(&self) -> bool {
        matches!(self, CoreError::Unreachable(_))
    }

    /// A certificate that needs the user's decision.
    pub fn is_certificate(&self) -> bool {
        matches!(
            self,
            CoreError::Untrusted { .. } | CoreError::CertificateChanged { .. }
        )
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

pub(super) struct RawResponse {
    pub(super) status: u16,
    pub(super) etag: Option<String>,
    pub(super) body: Vec<u8>,
}

/// A core's API at one address.
#[derive(Clone)]
pub struct CoreClient {
    http: Arc<dyn HttpClient>,
    base: Url,
    /// The client that accepts the core's pinned certificate, tried first.
    pinned: Option<Arc<PinnedHttp>>,
    /// Only the pinned certificate: no fallback to the platform's trust, and nothing
    /// without HTTPS. For keys that must reach only that core (a local claim's owner
    /// key, see [`super::local_claim`]).
    strict: bool,
}

impl CoreClient {
    /// `base` is the core's address from [`parse_address`]. `http` verifies certificates
    /// as macOS does.
    pub fn new(http: Arc<dyn HttpClient>, base: Url) -> Self {
        Self {
            http,
            base,
            pinned: None,
            strict: false,
        }
    }

    /// Talks only to the core that presents the pinned certificate, over HTTPS: a
    /// request without a pin, over plain HTTP, or to another certificate fails before
    /// anything (a key included) is sent.
    pub fn strict(mut self) -> Self {
        self.strict = true;
        self
    }

    pub fn is_strict(&self) -> bool {
        self.strict
    }

    /// The certificate the user trusted for this core, if any: the live streams use it
    /// too (see [`super::live`]).
    pub fn pin(&self) -> Option<&super::tls::CertificatePin> {
        self.pinned.as_ref()?.pin()
    }

    /// Also accepts the certificate `pinned` was made for (see [`super::tls`]).
    pub fn with_pinned(mut self, pinned: Arc<PinnedHttp>) -> Self {
        self.pinned = Some(pinned);
        self
    }

    fn host(&self) -> String {
        normalize_host(self.base.host_str().unwrap_or_default())
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    pub(super) fn url(&self, path: &str) -> String {
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
        let body = body.map(|body| (body, "application/json".to_string()));
        self.send_typed(method, path, auth, body, if_none_match, limit, timeout)
            .await
    }

    /// Like `send`, with the body's content type.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn send_typed(
        &self,
        method: Method,
        path: &str,
        auth: Auth<'_>,
        body: Option<(Vec<u8>, String)>,
        if_none_match: Option<&str>,
        limit: u64,
        timeout: Duration,
    ) -> Result<RawResponse, CoreError> {
        let request = || {
            let mut builder = Request::builder()
                .method(method.clone())
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
            match body.clone() {
                Some((body, content_type)) => builder
                    .header("Content-Type", content_type)
                    .body(AsyncBody::from(body)),
                None => builder.body(AsyncBody::empty()),
            }
            .map_err(|err| CoreError::InvalidAddress(err.to_string()))
        };
        let started = std::time::Instant::now();
        let traced_path = path.split('?').next().unwrap_or_default().to_string();
        let response = match self.send_checked(request).await {
            Ok(response) => response,
            Err(err) => {
                let ms = started.elapsed().as_millis() as u64;
                crate::trace::emit("core_http.error", || {
                    serde_json::json!({
                        "method": method.to_string(),
                        "host": self.host(),
                        "path": traced_path,
                        "error": err.to_string(),
                        "ms": ms,
                    })
                });
                return Err(err);
            }
        };
        let status = response.status().as_u16();
        let ms = started.elapsed().as_millis() as u64;
        crate::trace::emit("core_http.response", || {
            serde_json::json!({
                "method": method.to_string(),
                "host": self.host(),
                "path": traced_path,
                "status": status,
                "ms": ms,
            })
        });
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

    /// Sends a request (made by `request`, which may be called more than once) the way
    /// the core's certificate allows:
    ///
    /// - with a pin, through the pinned client first; when that refuses the certificate,
    ///   through the platform verifier (a certificate macOS trusts now still works), and
    ///   otherwise [`CoreError::CertificateChanged`] (or [`CoreError::Untrusted`] when the
    ///   pinned certificate is presented for another host);
    /// - without one, through the platform verifier; when that refuses the certificate,
    ///   a probe connection reads the presented fingerprint for
    ///   [`CoreError::Untrusted`]. The probe accepts nothing, so no request is answered.
    pub(super) async fn send_checked(
        &self,
        request: impl Fn() -> Result<Request<AsyncBody>, CoreError>,
    ) -> Result<http_client::Response<AsyncBody>, CoreError> {
        let untrusted = |fingerprint: Option<String>| CoreError::Untrusted {
            host: self.host(),
            fingerprint,
        };
        if self.strict {
            return self.send_strict(request).await;
        }
        if let Some(pinned) = &self.pinned {
            pinned.take_presented();
            let err = match pinned.http().send(request()?).await {
                Ok(response) => return Ok(response),
                Err(err) => err,
            };
            let Some(presented) = pinned.take_presented() else {
                // the handshake never got to the certificate
                return Err(CoreError::Unreachable(format!("{err:#}")));
            };
            match self.http.send(request()?).await {
                Ok(response) => return Ok(response),
                Err(err) if !is_certificate_error(&err) => {
                    return Err(CoreError::Unreachable(format!("{err:#}")));
                }
                Err(_) => {}
            }
            return Err(match pinned.pin() {
                Some(pin) if !same_fingerprint(&pin.sha256, &presented.sha256) => {
                    CoreError::CertificateChanged {
                        host: presented.host,
                        pinned: pin.sha256.clone(),
                        presented: presented.sha256,
                    }
                }
                _ => untrusted(Some(presented.sha256)),
            });
        }
        match self.http.send(request()?).await {
            Ok(response) => Ok(response),
            Err(err) if is_certificate_error(&err) => {
                Err(untrusted(self.probe_fingerprint().await))
            }
            Err(err) => Err(CoreError::Unreachable(format!("{err:#}"))),
        }
    }

    /// [`Self::send_checked`] for a strict client: the pinned certificate over HTTPS, or
    /// nothing.
    async fn send_strict(
        &self,
        request: impl Fn() -> Result<Request<AsyncBody>, CoreError>,
    ) -> Result<http_client::Response<AsyncBody>, CoreError> {
        let pinned = match &self.pinned {
            Some(pinned) if pinned.pin().is_some() && self.base.scheme() == "https" => pinned,
            _ => {
                return Err(CoreError::Untrusted {
                    host: self.host(),
                    fingerprint: None,
                });
            }
        };
        pinned.take_presented();
        match pinned.http().send(request()?).await {
            Ok(response) => Ok(response),
            Err(err) => match (pinned.take_presented(), pinned.pin()) {
                (Some(presented), Some(pin))
                    if !same_fingerprint(&pin.sha256, &presented.sha256) =>
                {
                    Err(CoreError::CertificateChanged {
                        host: presented.host,
                        pinned: pin.sha256.clone(),
                        presented: presented.sha256,
                    })
                }
                _ => Err(CoreError::Unreachable(format!("{err:#}"))),
            },
        }
    }

    /// The fingerprint of the certificate the core presents, read by a connection that
    /// accepts no certificate: its handshake fails before anything is sent.
    pub async fn probe_fingerprint(&self) -> Option<String> {
        let probe = PinnedHttp::new(None).ok()?;
        let request = Request::builder()
            .method(Method::GET)
            .uri(self.url("info"))
            .follow_redirects(RedirectPolicy::NoFollow)
            .timeout(REQUEST_TIMEOUT)
            .body(AsyncBody::empty())
            .ok()?;
        let _ = probe.http().send(request).await;
        probe.take_presented().map(|p| p.sha256)
    }

    pub(super) async fn json<T: serde::de::DeserializeOwned>(
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
        self.sign_in_with_scope(
            username,
            password,
            key_name,
            expires_at,
            super::live::LauncherScope::default(),
        )
        .await
    }

    /// [`CoreClient::sign_in`], with the console access chosen at sign-in. The key's
    /// `info.scope` says what the core granted: an older core ignores the opt-ins.
    pub async fn sign_in_with_scope(
        &self,
        username: &str,
        password: &Secret,
        key_name: &str,
        expires_at: Option<i64>,
        scope: super::live::LauncherScope,
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
            "scope": scope.to_json(),
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

    /// `POST /local_claim/start`: begins a claim of owner access by a launcher on the
    /// core's computer (see [`super::local_claim`]). Needs no sign-in.
    pub async fn local_claim_start(&self) -> Result<super::local_claim::ClaimStart, CoreError> {
        self.json(
            Method::POST,
            "local_claim/start",
            Auth::None,
            Some(b"{}".to_vec()),
        )
        .await
    }

    /// `POST /local_claim/finish`: the core checks the files the launcher wrote and
    /// writes the key into the claim's folder. Nothing secret is in the reply.
    pub async fn local_claim_finish(&self, id: &str, key_name: &str) -> Result<(), CoreError> {
        self.local_claim_finish_with(id, key_name, false).await
    }

    /// [`Self::local_claim_finish`], asking for an owner sign-in token instead of a key
    /// when `session` is set (see [`super::local_claim::claim_session`]).
    pub async fn local_claim_finish_with(
        &self,
        id: &str,
        key_name: &str,
        session: bool,
    ) -> Result<(), CoreError> {
        let mut body = serde_json::json!({ "id": id, "name": key_name });
        if session {
            body["session"] = serde_json::json!(true);
        }
        let response = self
            .send(
                Method::POST,
                "local_claim/finish",
                Auth::None,
                Some(body.to_string().into_bytes()),
                None,
                MAX_JSON_BYTES,
                REQUEST_TIMEOUT,
            )
            .await?;
        check(&response, Auth::None)
    }

    /// Revokes the API key `id` with the key itself. Cores that only let a sign-in
    /// session revoke keys refuse it; [`Self::revoke_key_signed_in`] works there.
    pub async fn revoke_key(&self, key: &Secret, id: &str) -> Result<(), CoreError> {
        let response = self
            .send(
                Method::DELETE,
                &format!("user/api_keys/{}", segment(id)?),
                Auth::Bearer(key),
                None,
                None,
                MAX_JSON_BYTES,
                REQUEST_TIMEOUT,
            )
            .await?;
        check(&response, Auth::Bearer(key))
    }

    /// Signs in with a username and password and revokes the API key `id` with that
    /// session, which is then dropped.
    pub async fn revoke_key_signed_in(
        &self,
        username: &str,
        password: &Secret,
        id: &str,
    ) -> Result<(), CoreError> {
        let login: LoginReply = self
            .json(
                Method::POST,
                "user/login",
                Auth::Basic(username, password),
                None,
            )
            .await?;
        let session = Secret::new(login.token);
        let response = self
            .send(
                Method::DELETE,
                &format!("user/api_keys/{}", segment(id)?),
                Auth::Bearer(&session),
                None,
                None,
                MAX_JSON_BYTES,
                REQUEST_TIMEOUT,
            )
            .await?;
        check(&response, Auth::Bearer(&session))
    }

    /// A JSON request with this device's key, for the routes in [`super::live`].
    pub(crate) async fn json_with_key<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        key: &Secret,
        body: Option<Vec<u8>>,
    ) -> Result<T, CoreError> {
        self.json(method, path, Auth::Bearer(key), body).await
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
pub(super) enum Auth<'a> {
    None,
    Bearer(&'a Secret),
    Basic(&'a str, &'a Secret),
}

/// An instance UUID as one path segment.
pub(crate) fn segment(uuid: &str) -> Result<&str, CoreError> {
    if !uuid.is_empty()
        && uuid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(uuid)
    } else {
        Err(CoreError::Invalid(format!("{uuid:?} isn't a server id")))
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

pub(super) fn check(response: &RawResponse, auth: Auth<'_>) -> Result<(), CoreError> {
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

/// Whether a request failed on the server's certificate (rustls' and reqwest's
/// messages say so).
fn is_certificate_error(err: &anyhow::Error) -> bool {
    let lower = format!("{err:#}").to_ascii_lowercase();
    lower.contains("certificate") || lower.contains("unknownissuer")
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
    fn certificate_errors_are_recognised() {
        assert!(is_certificate_error(&anyhow::anyhow!(
            "invalid peer certificate: UnknownIssuer"
        )));
        assert!(!is_certificate_error(&anyhow::anyhow!(
            "connection refused"
        )));
    }

    /// An HTTP client that fails the test if anything is sent through it.
    struct NothingSent;

    impl HttpClient for NothingSent {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&Url> {
            None
        }

        fn send(
            &self,
            req: Request<AsyncBody>,
        ) -> futures::future::BoxFuture<'static, anyhow::Result<http_client::Response<AsyncBody>>>
        {
            panic!("a strict client sent {}", req.uri());
        }
    }

    #[test]
    fn a_strict_client_sends_nothing_without_a_pinned_https_connection() {
        let key = Secret::new("lsk_k_secret");
        for address in ["http://127.0.0.1:16662", "https://127.0.0.1:16662"] {
            let client =
                CoreClient::new(Arc::new(NothingSent), Url::parse(address).unwrap()).strict();
            let result = futures::executor::block_on(client.servers(&key));
            assert!(
                matches!(result, Err(CoreError::Untrusted { .. })),
                "{address}: {result:?}"
            );
        }
    }
}
