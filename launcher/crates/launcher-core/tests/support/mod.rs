//! A mock of the Microsoft, Xbox Live and Minecraft services on a loopback port
//! the OS picks. Each test starts its own, so tests share no state.
//!
//! Every token it hands out contains `SECRET`, so a test can search files for that
//! word to check that no token was written to disk.

#![allow(dead_code)]

use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{Arc, Mutex, MutexGuard},
    thread::JoinHandle,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use launcher_core::{
    auth::{
        Endpoints, MINECRAFT_LAUNCHER_CLIENT_ID, MINECRAFT_LAUNCHER_SCOPE, MsaConfig,
        live::REDIRECT_URI,
    },
    skin,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use url::Url;

pub const CLIENT_ID: &str = "test-client-id";
pub const PROFILE_ID: &str = "069a79f444e94726a5befca90e38aaf5";
pub const SKIN_HASH: &str = "abc123def456";
/// A profile on the session server with no skin of its own.
pub const STEVE_ID: &str = "8667ba71b85a4004af54457a9734eed7";
pub const AUTH_CODE: &str = "auth-code-SECRET";
pub const MS_ACCESS: &str = "ms-access-SECRET";
pub const XBL_TOKEN: &str = "xbl-token-SECRET";
pub const XSTS_TOKEN: &str = "xsts-token-SECRET";
pub const MC_TOKEN: &str = "mc-token-SECRET";
pub const USER_HASH: &str = "user-hash-1";
pub const DEVICE_CODE: &str = "device-code-SECRET";
/// What login.live.com hands the Minecraft Launcher's client.
pub const LIVE_ACCESS: &str = "live-access-SECRET";
pub const LIVE_AUTH_CODE: &str = "M.C-live-code-SECRET";
pub const LIVE_DEVICE_CODE: &str = "live-device-code-SECRET";

/// How the login endpoint answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Login {
    Ok,
    /// 403 "Invalid app registration", before Mojang approves the app.
    NotApproved,
}

/// What a device code poll answers, in order. The last answer repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poll {
    Pending,
    SlowDown,
    Declined,
    Expired,
    Granted,
}

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub user_agent: Option<String>,
    pub body: String,
}

pub struct State {
    // Behaviour, set by the test.
    pub xerr: Option<u64>,
    pub login: Login,
    pub entitlements: Vec<&'static str>,
    pub has_profile: bool,
    /// Refresh tokens the token endpoint accepts.
    pub valid_refresh_tokens: HashSet<String>,
    /// Refresh tokens login.live.com accepts for the Minecraft Launcher's client.
    /// Each client only accepts its own.
    pub valid_live_refresh_tokens: HashSet<String>,
    /// Whether a refresh hands out a new refresh token and revokes the old one.
    pub rotate: bool,
    /// The S256 challenge the authorization code was issued for.
    pub expected_challenge: Option<String>,
    pub expected_redirect_uri: Option<String>,
    pub polls: Vec<Poll>,
    // Records.
    pub requests: Vec<Recorded>,
    refresh_counter: u32,
    poll_index: usize,
}

pub struct MockServer {
    pub base: Url,
    state: Arc<Mutex<State>>,
    server: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let state = Arc::new(Mutex::new(State {
            xerr: None,
            login: Login::Ok,
            entitlements: vec!["product_minecraft", "game_minecraft"],
            has_profile: true,
            valid_refresh_tokens: HashSet::new(),
            valid_live_refresh_tokens: HashSet::new(),
            rotate: true,
            expected_challenge: None,
            expected_redirect_uri: None,
            polls: vec![Poll::Granted],
            requests: Vec::new(),
            refresh_counter: 0,
            poll_index: 0,
        }));
        let thread = {
            let server = server.clone();
            let state = state.clone();
            std::thread::spawn(move || {
                // `recv` fails once `unblock` is called on drop.
                while let Ok(mut request) = server.recv() {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);
                    let (status, response) = route(&mut state.lock().unwrap(), &request, &body);
                    let response = match response {
                        Response::Json(value) => {
                            tiny_http::Response::from_string(value.to_string()).with_header(
                                tiny_http::Header::from_str("Content-Type: application/json")
                                    .unwrap(),
                            )
                        }
                        Response::Bytes(bytes) => tiny_http::Response::from_data(bytes),
                    };
                    let _ = request.respond(response.with_status_code(status));
                }
            })
        };
        Self {
            base: Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            state,
            server,
            thread: Some(thread),
        }
    }

    pub fn config(&self) -> MsaConfig {
        MsaConfig {
            client_id: CLIENT_ID.into(),
            endpoints: Endpoints::all_at(&self.base),
        }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.state().requests.clone()
    }

    pub fn paths(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.path).collect()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum Response {
    Json(Value),
    Bytes(Vec<u8>),
}

fn form(body: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn oauth_error(error: &str) -> (u16, Response) {
    (
        400,
        Response::Json(json!({"error": error, "error_description": "mock"})),
    )
}

fn header(request: &tiny_http::Request, name: &'static str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str().to_string())
}

fn route(state: &mut State, request: &tiny_http::Request, body: &str) -> (u16, Response) {
    let path = request.url().split('?').next().unwrap_or("").to_string();
    let method = request.method().as_str().to_string();
    state.requests.push(Recorded {
        method: method.clone(),
        path: path.clone(),
        user_agent: header(request, "User-Agent"),
        body: body.to_string(),
    });
    let bearer = header(request, "Authorization");
    let json_body: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let not_found = (404, Response::Json(json!({"error": "no route"})));

    match (method.as_str(), path.as_str()) {
        ("POST", "/login/token") => token(state, &form(body)),
        ("POST", "/live/oauth20_token.srf") => live_token(state, &form(body)),
        ("POST", "/live/oauth20_connect.srf") => {
            let fields = form(body);
            if field(&fields, "client_id") != Some(MINECRAFT_LAUNCHER_CLIENT_ID) {
                return oauth_error("unauthorized_client");
            }
            if field(&fields, "scope") != Some(MINECRAFT_LAUNCHER_SCOPE)
                || field(&fields, "response_type") != Some("device_code")
            {
                return oauth_error("invalid_request");
            }
            (
                200,
                Response::Json(json!({
                    "user_code": "LIVE1234",
                    "device_code": LIVE_DEVICE_CODE,
                    "verification_uri": "https://www.microsoft.com/link",
                    "interval": 5,
                    "expires_in": 900,
                })),
            )
        }
        ("POST", "/login/devicecode") => {
            let fields = form(body);
            if field(&fields, "client_id") != Some(CLIENT_ID) {
                return oauth_error("invalid_client");
            }
            assert_eq!(
                field(&fields, "scope"),
                Some("XboxLive.signin offline_access")
            );
            (
                200,
                Response::Json(json!({
                    "device_code": DEVICE_CODE,
                    "user_code": "ABCD-EFGH",
                    // Not a Microsoft host, so the launcher shows its own URL.
                    "verification_uri": "https://evil.example/link",
                    "expires_in": 900,
                    "interval": 5,
                    "message": "mock",
                })),
            )
        }
        ("POST", "/xbl/user/authenticate") => {
            // Xbox Live takes an Azure app's token as `d=` and a title client's
            // token as `t=`, and nothing else.
            let ticket = json_body["Properties"]["RpsTicket"].as_str();
            let lodestone = format!("d={MS_ACCESS}");
            let launcher = format!("t={LIVE_ACCESS}");
            if ticket != Some(lodestone.as_str()) && ticket != Some(launcher.as_str()) {
                return (401, Response::Json(json!({})));
            }
            assert_eq!(json_body["RelyingParty"], "http://auth.xboxlive.com");
            (
                200,
                Response::Json(json!({
                    "Token": XBL_TOKEN,
                    "DisplayClaims": {"xui": [{"uhs": USER_HASH}]},
                })),
            )
        }
        ("POST", "/xsts/xsts/authorize") => {
            assert_eq!(json_body["RelyingParty"], "rp://api.minecraftservices.com/");
            if json_body["Properties"]["UserTokens"][0] != XBL_TOKEN {
                return (401, Response::Json(json!({})));
            }
            if let Some(code) = state.xerr {
                return (
                    401,
                    Response::Json(json!({
                        "Identity": "0",
                        "XErr": code,
                        "Message": "",
                        "Redirect": "https://start.ui.xboxlive.com/CreateAccount",
                    })),
                );
            }
            (
                200,
                Response::Json(json!({
                    "Token": XSTS_TOKEN,
                    "DisplayClaims": {"xui": [{"uhs": USER_HASH}]},
                })),
            )
        }
        ("POST", "/mc/authentication/login_with_xbox") => {
            if json_body["identityToken"] != format!("XBL3.0 x={USER_HASH};{XSTS_TOKEN}") {
                return (401, Response::Json(json!({})));
            }
            match state.login {
                Login::NotApproved => (
                    403,
                    Response::Json(json!({
                        "path": "/authentication/login_with_xbox",
                        "errorMessage": "Invalid app registration, see https://aka.ms/AppRegInfo for more information",
                    })),
                ),
                Login::Ok => (
                    200,
                    Response::Json(json!({
                        "username": "some-uuid",
                        "roles": [],
                        "access_token": MC_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 86400,
                    })),
                ),
            }
        }
        ("GET", "/mc/entitlements/mcstore") => {
            if bearer.as_deref() != Some(&format!("Bearer {MC_TOKEN}")) {
                return (401, Response::Json(json!({})));
            }
            let items: Vec<Value> = state
                .entitlements
                .iter()
                .map(|name| json!({"name": name, "signature": "sig"}))
                .collect();
            (
                200,
                Response::Json(json!({"items": items, "signature": "sig", "keyId": "1"})),
            )
        }
        ("GET", "/mc/minecraft/profile") => {
            if bearer.as_deref() != Some(&format!("Bearer {MC_TOKEN}")) {
                return (401, Response::Json(json!({})));
            }
            if !state.has_profile {
                return (
                    404,
                    Response::Json(json!({
                        "path": "/minecraft/profile",
                        "errorType": "NOT_FOUND",
                        "error": "NOT_FOUND",
                        "errorMessage": "The server has not found anything matching the request URI",
                    })),
                );
            }
            (
                200,
                Response::Json(json!({
                    "id": PROFILE_ID,
                    "name": "Notch",
                    "skins": [
                        {"id": "old", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/0000", "variant": "CLASSIC"},
                        {"id": "s", "state": "ACTIVE", "url": format!("http://textures.minecraft.net/texture/{SKIN_HASH}"), "variant": "CLASSIC"},
                    ],
                    "capes": [],
                })),
            )
        }
        ("GET", path) if path == format!("/textures/texture/{SKIN_HASH}") => (
            200,
            Response::Bytes(skin::encode_png(&skin::sample_skin(true)).unwrap()),
        ),
        // the public session server: anyone's profile by UUID, its textures in base64
        ("GET", path) if path == format!("/sessions/session/minecraft/profile/{PROFILE_ID}") => {
            let textures = json!({
                "timestamp": 1,
                "profileId": PROFILE_ID,
                "profileName": "Notch",
                "textures": {
                    "SKIN": {"url": format!("http://textures.minecraft.net/texture/{SKIN_HASH}")},
                },
            });
            let value = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&textures).unwrap());
            (
                200,
                Response::Json(json!({
                    "id": PROFILE_ID,
                    "name": "Notch",
                    "properties": [{"name": "textures", "value": value}],
                })),
            )
        }
        // a profile with the default skin
        ("GET", path) if path == format!("/sessions/session/minecraft/profile/{STEVE_ID}") => (
            200,
            Response::Json(json!({
                "id": STEVE_ID,
                "name": "Steve",
                "properties": [{"name": "textures", "value": base64::engine::general_purpose::STANDARD.encode(br#"{"textures":{}}"#)}],
            })),
        ),
        // no such profile
        ("GET", path) if path.starts_with("/sessions/session/minecraft/profile/") => {
            (204, Response::Bytes(Vec::new()))
        }
        _ => not_found,
    }
}

fn token(state: &mut State, fields: &[(String, String)]) -> (u16, Response) {
    if field(fields, "client_id") != Some(CLIENT_ID) {
        return oauth_error("invalid_client");
    }
    let granted = |refresh: String| {
        (
            200,
            Response::Json(json!({
                "token_type": "Bearer",
                "scope": "XboxLive.signin offline_access",
                "expires_in": 3600,
                "access_token": MS_ACCESS,
                "refresh_token": refresh,
            })),
        )
    };
    match field(fields, "grant_type") {
        Some("authorization_code") => {
            if field(fields, "code") != Some(AUTH_CODE) {
                return oauth_error("invalid_grant");
            }
            // PKCE: the verifier must hash to the challenge from the browser.
            let verifier = field(fields, "code_verifier").unwrap_or_default();
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            if state.expected_challenge.as_deref() != Some(challenge.as_str()) {
                return oauth_error("invalid_grant");
            }
            if field(fields, "redirect_uri") != state.expected_redirect_uri.as_deref() {
                return oauth_error("invalid_grant");
            }
            let refresh = "refresh-SECRET-0".to_string();
            state.valid_refresh_tokens.insert(refresh.clone());
            granted(refresh)
        }
        Some("refresh_token") => {
            let presented = field(fields, "refresh_token")
                .unwrap_or_default()
                .to_string();
            if !state.valid_refresh_tokens.contains(&presented) {
                return oauth_error("invalid_grant");
            }
            if state.rotate {
                state.valid_refresh_tokens.remove(&presented);
                state.refresh_counter += 1;
                let next = format!("refresh-SECRET-{}", state.refresh_counter);
                state.valid_refresh_tokens.insert(next.clone());
                granted(next)
            } else {
                granted(presented)
            }
        }
        Some("urn:ietf:params:oauth:grant-type:device_code") => {
            if field(fields, "device_code") != Some(DEVICE_CODE) {
                return oauth_error("bad_verification_code");
            }
            let poll = state.polls[state.poll_index.min(state.polls.len() - 1)];
            state.poll_index += 1;
            match poll {
                Poll::Pending => oauth_error("authorization_pending"),
                Poll::SlowDown => oauth_error("slow_down"),
                Poll::Declined => oauth_error("authorization_declined"),
                Poll::Expired => oauth_error("expired_token"),
                Poll::Granted => {
                    let refresh = "refresh-SECRET-device".to_string();
                    state.valid_refresh_tokens.insert(refresh.clone());
                    granted(refresh)
                }
            }
        }
        _ => oauth_error("unsupported_grant_type"),
    }
}

/// login.live.com's token endpoint, for the Minecraft Launcher's client only.
fn live_token(state: &mut State, fields: &[(String, String)]) -> (u16, Response) {
    if field(fields, "client_id") != Some(MINECRAFT_LAUNCHER_CLIENT_ID) {
        return oauth_error("unauthorized_client");
    }
    let granted = |refresh: String| {
        (
            200,
            Response::Json(json!({
                "token_type": "bearer",
                "expires_in": 86400,
                "scope": MINECRAFT_LAUNCHER_SCOPE,
                "access_token": LIVE_ACCESS,
                "refresh_token": refresh,
                "user_id": "abc",
            })),
        )
    };
    let scope_ok = field(fields, "scope") == Some(MINECRAFT_LAUNCHER_SCOPE);
    match field(fields, "grant_type") {
        Some("authorization_code") => {
            if !scope_ok || field(fields, "redirect_uri") != Some(REDIRECT_URI) {
                return oauth_error("invalid_request");
            }
            if field(fields, "code") != Some(LIVE_AUTH_CODE) {
                return oauth_error("invalid_grant");
            }
            let refresh = "live-refresh-SECRET-0".to_string();
            state.valid_live_refresh_tokens.insert(refresh.clone());
            granted(refresh)
        }
        Some("refresh_token") => {
            if !scope_ok {
                return oauth_error("invalid_scope");
            }
            let presented = field(fields, "refresh_token")
                .unwrap_or_default()
                .to_string();
            if !state.valid_live_refresh_tokens.remove(&presented) {
                return oauth_error("invalid_grant");
            }
            state.refresh_counter += 1;
            let next = format!("live-refresh-SECRET-{}", state.refresh_counter);
            state.valid_live_refresh_tokens.insert(next.clone());
            granted(next)
        }
        Some("urn:ietf:params:oauth:grant-type:device_code") => {
            if field(fields, "device_code") != Some(LIVE_DEVICE_CODE) {
                return oauth_error("invalid_grant");
            }
            let poll = state.polls[state.poll_index.min(state.polls.len() - 1)];
            state.poll_index += 1;
            match poll {
                Poll::Pending => oauth_error("authorization_pending"),
                Poll::SlowDown => oauth_error("slow_down"),
                Poll::Declined => oauth_error("access_denied"),
                Poll::Expired => oauth_error("expired_token"),
                Poll::Granted => {
                    let refresh = "live-refresh-SECRET-device".to_string();
                    state.valid_live_refresh_tokens.insert(refresh.clone());
                    granted(refresh)
                }
            }
        }
        _ => oauth_error("unsupported_grant_type"),
    }
}
