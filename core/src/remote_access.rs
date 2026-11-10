//! Whether other devices may use the core: the "Allow access from other devices"
//! setting (`allow_remote_access` in the global settings).
//!
//! Lodestone Host makes its core's owner without a password on the first run, so the
//! person using it never sets one up (see `crate::local_claim`). Such a core serves only
//! this computer: the Host, Lodestone Launcher's "Connect as owner" and Lodestone CLI. When
//! the owner wants to use the core from other devices (the web dashboard, other users, API
//! keys for remote tools), the Host asks them for a username and password first
//! (`PUT /user/owner_account`), which turns remote access on. The setting is saved, so
//! it never depends on guessing from the users file:
//!
//! - Files written before it existed have no field and read as on: every existing
//!   install keeps serving other devices.
//! - A new core run by Lodestone Host (`--is-host`) without an owner starts with it off,
//!   and a claim that makes an owner without a password keeps it off. An owner made with
//!   a password (the dashboard's setup page, the Tauri app, a claim with a password)
//!   turns it on, as before.
//! - It can only be turned on while the owner has a password, and only turned off from
//!   this computer (a remote owner would lock themselves out).
//!
//! While it is off, [`guard`] answers every API request from another device with 403 and
//! [`ErrorKind::LocalOnly`]. It runs inside the CORS layer, so a web dashboard on another
//! origin can read the reason, and before every route, WebSocket upgrades and the public
//! listing included. Plain HTTP from other devices is still refused before that, by
//! `crate::tls::DualProtocolAcceptor`. Connections from other devices opened while it was
//! on fail at their next read or write once it is turned off ([`GatedStream`]), so an
//! open event stream stops too.
//!
//! Lodestone Connect is not affected: friends reach a shared instance's game port and
//! its few info routes through the Connect tunnel, never through this listener (see
//! `crate::connect`).

use std::{
    io,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use color_eyre::eyre::eyre;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tracing::{debug, info, warn};

use crate::{
    error::{Error, ErrorKind},
    events::{CausedBy, Event, EventInner},
    tls::is_loopback,
    types::Snowflake,
    AppState,
};

/// What another device is told while the core serves only this computer.
pub const LOCAL_ONLY_MESSAGE: &str = "This Lodestone core only allows apps on its own \
    computer. To use it from another device, open Lodestone Host on that computer and set \
    up an owner account in Settings > General > Allow access from other devices";

/// The setting, readable without a lock, and a count of the times it was turned off.
#[derive(Debug)]
pub struct RemoteAccess {
    allowed: AtomicBool,
    /// Goes up each time remote access is turned off: a connection from another device
    /// opened before that fails ([`GatedStream`]).
    closed: AtomicU64,
}

impl RemoteAccess {
    pub fn new(allowed: bool) -> Arc<Self> {
        Arc::new(Self {
            allowed: AtomicBool::new(allowed),
            closed: AtomicU64::new(0),
        })
    }

    /// Whether other devices may use the core.
    pub fn allowed(&self) -> bool {
        self.allowed.load(Ordering::SeqCst)
    }

    /// Only [`crate::global_settings::GlobalSettings`] changes it outside of tests.
    pub(crate) fn set(&self, allowed: bool) {
        let was = self.allowed.swap(allowed, Ordering::SeqCst);
        if was && !allowed {
            self.closed.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn generation(&self) -> u64 {
        self.closed.load(Ordering::SeqCst)
    }

    /// Whether a request from `peer` may go on: always from this computer, from anywhere
    /// else only while remote access is on. An unknown address counts as another device.
    pub fn allows(&self, peer: Option<IpAddr>) -> bool {
        peer.is_some_and(is_loopback) || self.allowed()
    }
}

pub fn local_only_error() -> Error {
    Error {
        kind: ErrorKind::LocalOnly,
        source: eyre!(LOCAL_ONLY_MESSAGE),
    }
}

/// Refuses requests from other devices while remote access is off (403,
/// [`ErrorKind::LocalOnly`]). Answered here, without the warning every refused request
/// would otherwise log.
pub async fn guard(
    State(remote): State<Arc<RemoteAccess>>,
    request: Request,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    if remote.allows(peer) {
        return next.run(request).await;
    }
    debug!(
        "Refused {} {} from another device: the core only allows this computer",
        request.method(),
        request.uri().path()
    );
    (StatusCode::FORBIDDEN, Json(local_only_error())).into_response()
}

/// A connection, which fails at its next read or write if it comes from another device
/// and remote access was turned off after it was opened.
pub struct GatedStream<S> {
    inner: S,
    /// For another device: the setting and how many times it had been turned off.
    gate: Option<(Arc<RemoteAccess>, u64)>,
}

impl<S> GatedStream<S> {
    pub fn new(inner: S, peer: IpAddr, remote: &Arc<RemoteAccess>) -> Self {
        let gate = (!is_loopback(peer)).then(|| (remote.clone(), remote.generation()));
        Self { inner, gate }
    }

    fn check(&self) -> io::Result<()> {
        match &self.gate {
            Some((remote, opened)) if remote.generation() != *opened => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "access from other devices was turned off",
            )),
            _ => Ok(()),
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for GatedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Err(e) = self.check() {
            return Poll::Ready(Err(e));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for GatedStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(e) = self.check() {
            return Poll::Ready(Err(e));
        }
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if let Err(e) = self.check() {
            return Poll::Ready(Err(e));
        }
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(e) = self.check() {
            return Poll::Ready(Err(e));
        }
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The owner's username, trimmed: 1 to 64 characters, none of them control characters.
/// The error is for people.
pub fn owner_username(username: &str) -> Result<String, &'static str> {
    let username = username.trim();
    let chars = username.chars().count();
    if chars == 0 || chars > 64 || username.chars().any(char::is_control) {
        return Err("The owner's username must have 1 to 64 characters");
    }
    Ok(username.to_string())
}

/// Checks the owner's new password: 12 to 256 characters. The error is for people.
pub fn owner_password(password: &str) -> Result<(), &'static str> {
    if password.chars().count() < 12 || password.len() > 256 {
        return Err("The owner's password must have 12 to 256 characters");
    }
    Ok(())
}

/// Whether the core's owner can sign in with a password. `false` without an owner.
pub async fn owner_has_password(app: &AppState) -> bool {
    app.users_manager
        .read()
        .await
        .as_ref()
        .values()
        .any(|user| user.is_owner && user.has_password())
}

/// Turns remote access on or off, saves it and tells clients.
pub(crate) async fn set(app: &AppState, allowed: bool, caused_by: CausedBy) -> Result<(), Error> {
    app.global_settings
        .lock()
        .await
        .set_allow_remote_access(allowed)
        .await?;
    let details = if allowed {
        "Access from other devices turned on"
    } else {
        "Access from other devices turned off: only apps on this computer can use the core"
    };
    info!("{details}");
    app.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details: details.to_string(),
        snowflake: Snowflake::default(),
        caused_by,
    });
    Ok(())
}

/// After an owner was made: one with a password may be used from other devices, as
/// always; one without (Lodestone Host's first run) only from this computer.
pub(crate) async fn owner_created(app: &AppState, has_password: bool) {
    let current = app.global_settings.lock().await.allow_remote_access();
    if current == has_password {
        return;
    }
    if let Err(e) = set(app, has_password, CausedBy::System).await {
        // with a password: still local-only, which is safe, and turning it on in the
        // Host's settings tries again. Without: the setting couldn't be saved, but the
        // owner has no password, so nobody can sign in from another device anyway.
        warn!(
            "Failed to save whether other devices may use the core: {}",
            e.source
        );
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use reqwest::{Method, StatusCode};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

    use super::*;
    use crate::{
        auth::{api_key::ApiKeyScope, user::User},
        global_settings::{GlobalSettings, GlobalSettingsData},
        test_util::{serve_api, serve_api_from, test_app_state},
    };

    /// Another device on the local network.
    const LAN: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    const PASSWORD: &str = "a long enough password";

    fn local_only() -> GlobalSettingsData {
        GlobalSettingsData {
            allow_remote_access: false,
            ..Default::default()
        }
    }

    /// Adds `user` and returns a session token for them.
    async fn add(state: &AppState, user: User) -> String {
        state
            .users_manager
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user.create_jwt().unwrap().to_string()
    }

    async fn send(
        method: Method,
        url: String,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value, reqwest::header::HeaderMap) {
        let mut request = reqwest::Client::new()
            .request(method, url)
            .header("Origin", "https://dashboard.example");
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::Null),
            headers,
        )
    }

    fn assert_local_only(status: StatusCode, body: &Value, what: &str) {
        assert_eq!(status, StatusCode::FORBIDDEN, "{what}: {body}");
        assert_eq!(body["kind"], "LocalOnly", "{what}: {body}");
        assert_eq!(body["causes"][0], LOCAL_ONLY_MESSAGE, "{what}");
    }

    #[tokio::test]
    async fn other_devices_are_refused_while_local_only() {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let state = test_app_state(dir.path(), local_only()).await;
        let owner = add(&state, User::owner_without_password("alex".into())).await;
        let lan = serve_api_from(state.clone(), LAN).await;

        // every kind of route: public, sign-in, signed in, the claim, the listing
        for (method, path, token) in [
            (Method::GET, "/info", None),
            (Method::GET, "/public/instances", None),
            (Method::POST, "/user/login", None),
            (Method::POST, "/local_claim/start", None),
            (Method::GET, "/user/info", Some(owner.as_str())),
            (Method::GET, "/global_settings", Some(owner.as_str())),
            (Method::GET, "/instance/list", Some(owner.as_str())),
        ] {
            let (status, body, headers) =
                send(method, format!("{lan}{path}"), token, Some(json!({}))).await;
            assert_local_only(status, &body, path);
            // a dashboard on another origin can read why
            assert_eq!(
                headers
                    .get("access-control-allow-origin")
                    .map(|v| v.to_str().unwrap()),
                Some("*"),
                "{path}"
            );
        }

        // a CORS preflight is answered, so the browser goes on to the refused request
        let preflight = reqwest::Client::new()
            .request(Method::OPTIONS, format!("{lan}/user/info"))
            .header("Origin", "https://dashboard.example")
            .header("Access-Control-Request-Method", "GET")
            .header("Access-Control-Request-Headers", "authorization")
            .send()
            .await
            .unwrap();
        assert!(preflight.status().is_success());

        // a WebSocket is refused before it is upgraded
        let ws = lan.replacen("http://", "ws://", 1);
        let mut request = format!("{ws}/trace/stream").into_client_request().unwrap();
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {owner}").parse().unwrap());
        match tokio_tungstenite::connect_async(request).await {
            Err(tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                let body: Value =
                    serde_json::from_slice(response.body().as_deref().unwrap_or_default()).unwrap();
                assert_eq!(body["kind"], "LocalOnly");
            }
            other => panic!("another device got a stream: {other:?}"),
        }

        // this computer, over IPv4 (as a dual-stack socket reports it) and IPv6
        for peer in [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped()),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
        ] {
            let local = serve_api_from(state.clone(), peer).await;
            let (status, info, _) = send(Method::GET, format!("{local}/info"), None, None).await;
            assert_eq!(status, StatusCode::OK, "{peer}");
            assert_eq!(info["remote_access"], false);
            let (status, _, _) = send(
                Method::GET,
                format!("{local}/user/info"),
                Some(&owner),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{peer}");
        }

        // the setting applies at once
        state.remote_access.set(true);
        let (status, info, _) = send(Method::GET, format!("{lan}/info"), None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(info["remote_access"], true);
    }

    #[tokio::test]
    async fn an_unknown_address_counts_as_another_device() {
        let remote = RemoteAccess::new(false);
        assert!(!remote.allows(None));
        assert!(!remote.allows(Some(LAN)));
        assert!(remote.allows(Some(IpAddr::V6(Ipv6Addr::LOCALHOST))));
        remote.set(true);
        assert!(remote.allows(None));
        assert!(remote.allows(Some(LAN)));
    }

    #[tokio::test]
    async fn setting_up_the_owner_account_turns_remote_access_on() {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let state = test_app_state(dir.path(), local_only()).await;
        let owner = add(&state, User::owner_without_password("alex".into())).await;
        let local = serve_api(state.clone()).await;
        let lan = serve_api_from(state.clone(), LAN).await;
        let put = |path: &str, token: &str, body: Value| {
            let (url, token) = (format!("{local}{path}"), token.to_string());
            async move { send(Method::PUT, url, Some(&token), Some(body)).await }
        };

        // no password: nobody signs in with one, and nothing can leave this computer
        let login = reqwest::Client::new()
            .post(format!("{local}/user/login"))
            .basic_auth("alex", Some(PASSWORD))
            .send()
            .await
            .unwrap();
        assert_eq!(login.status(), StatusCode::UNAUTHORIZED);
        let (_, me, _) = send(
            Method::GET,
            format!("{local}/user/info"),
            Some(&owner),
            None,
        )
        .await;
        assert_eq!(me["has_password"], false);
        let (status, body, _) =
            put("/global_settings/allow_remote_access", &owner, json!(true)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        // the claim is the only way in, so it stays on
        let (status, _, _) = put("/global_settings/allow_local_claim", &owner, json!(false)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!state.remote_access.allowed());

        // an API key can't do it, and the rules of the owner's password hold
        let uid = state
            .users_manager
            .read()
            .await
            .get_user_by_username("alex")
            .unwrap()
            .uid;
        let key = state
            .users_manager
            .write()
            .await
            .create_api_key(
                &uid,
                "script".into(),
                None,
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await
            .unwrap()
            .key;
        let account = json!({ "username": " alex_admin ", "password": PASSWORD });
        let (status, _, _) = put("/user/owner_account", &key, account.clone()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _, _) = put(
            "/user/owner_account",
            &owner,
            json!({ "username": "alex", "password": "short" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _, _) = put(
            "/user/owner_account",
            &owner,
            json!({ "username": " ", "password": PASSWORD }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!state.remote_access.allowed());

        let (status, user, _) = put("/user/owner_account", &owner, account.clone()).await;
        assert_eq!(status, StatusCode::OK, "{user}");
        assert_eq!(user["username"], "alex_admin");
        assert_eq!(user["has_password"], true);
        assert!(state.remote_access.allowed());
        let stored: Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("global_settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(stored["allow_remote_access"], true);

        // another device signs in with it now; the Host's session goes on
        let login = reqwest::Client::new()
            .post(format!("{lan}/user/login"))
            .basic_auth("alex_admin", Some(PASSWORD))
            .send()
            .await
            .unwrap();
        assert_eq!(login.status(), StatusCode::OK);
        let (status, _, _) = send(
            Method::GET,
            format!("{local}/user/info"),
            Some(&owner),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // only once: then the password is changed with the old one
        let (status, _, _) = put("/user/owner_account", &owner, account).await;
        assert_eq!(status, StatusCode::CONFLICT);

        // turning it off: not from another device (it would lock them out), from here yes
        let (status, body, _) = send(
            Method::PUT,
            format!("{lan}/global_settings/allow_remote_access"),
            Some(&owner),
            Some(json!(false)),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["kind"], "PermissionDenied");
        let (status, _, _) =
            put("/global_settings/allow_remote_access", &owner, json!(false)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, body, _) = send(Method::GET, format!("{lan}/info"), None, None).await;
        assert_local_only(status, &body, "/info after turning it off");
        // and on again, with the password set up
        let (status, _, _) = put("/global_settings/allow_remote_access", &owner, json!(true)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = send(Method::GET, format!("{lan}/info"), None, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_dashboard_setup_with_a_password_lets_other_devices_in() {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        // a new Lodestone Host core, before its owner exists
        let state = test_app_state(dir.path(), local_only()).await;
        *state.first_time_setup_key.lock().await = Some("setupkey".into());
        let local = serve_api(state.clone()).await;
        let (status, _, _) = send(
            Method::POST,
            format!("{local}/setup/setupkey"),
            None,
            Some(json!({ "username": "boss", "password": PASSWORD })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(state.remote_access.allowed());
    }

    /// Every install from before the setting keeps serving other devices.
    #[tokio::test]
    async fn existing_installs_keep_remote_access() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global_settings.json");
        std::fs::write(
            &path,
            r#"{"core_name":"Home","safe_mode":true,"domain":null,"allow_lan_http":false}"#,
        )
        .unwrap();
        let (tx, _) = crate::event_broadcaster::EventBroadcaster::new(4);
        let mut settings = GlobalSettings::new(path.clone(), tx, GlobalSettingsData::default());
        settings.load_from_file().await.unwrap();
        assert!(settings.allow_remote_access());
        assert!(settings.remote_access().allowed());
        // saved, so it no longer depends on the file's age
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["allow_remote_access"], true);

        // a stored "off" stays off after a restart
        settings.set_allow_remote_access(false).await.unwrap();
        let (tx, _) = crate::event_broadcaster::EventBroadcaster::new(4);
        let mut again = GlobalSettings::new(path, tx, GlobalSettingsData::default());
        again.load_from_file().await.unwrap();
        assert!(!again.remote_access().allowed());
    }
}
