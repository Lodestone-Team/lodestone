//! The developer option "Sign in as the Minecraft Launcher" against the loopback
//! mock: the login.live.com device code and pasted-link flows with the official
//! launcher's client, the `t=` ticket, and refreshing each account with the client
//! that signed it in.

mod support;

use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

use futures::{FutureExt as _, executor::block_on};
use launcher_core::{
    MemorySecretStore, Secret, SecretStore as _,
    accounts::{Account, AccountStore},
    auth::{
        AuthError, Authenticator, ClientIdentity, LinkProblem, MINECRAFT_LAUNCHER_CLIENT_ID,
        MINECRAFT_LAUNCHER_SCOPE, SignedIn, live::REDIRECT_URI,
    },
    http,
};
use support::{MockServer, Poll, Recorded};
use uuid::Uuid;

fn authenticator(mock: &MockServer) -> Authenticator {
    Authenticator::new(http::default_client().unwrap(), mock.config())
}

fn form(body: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

/// The form fields of a recorded request, sorted, for exact comparison.
fn fields(request: &Recorded) -> Vec<(String, String)> {
    let mut fields = form(&request.body);
    fields.sort();
    fields
}

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut pairs: Vec<_> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    pairs.sort();
    pairs
}

fn requests_to(mock: &MockServer, path: &str) -> Vec<Recorded> {
    mock.requests()
        .into_iter()
        .filter(|r| r.path == path)
        .collect()
}

/// The `RpsTicket` of every Xbox Live user sign-in.
fn rps_tickets(mock: &MockServer) -> Vec<String> {
    requests_to(mock, "/xbl/user/authenticate")
        .iter()
        .map(|r| {
            let body: serde_json::Value = serde_json::from_str(&r.body).unwrap();
            body["Properties"]["RpsTicket"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

fn no_sleep() -> impl Fn(Duration) -> futures::future::LocalBoxFuture<'static, ()> {
    |_| futures::future::ready(()).boxed_local()
}

fn device_code_sign_in(
    mock: &MockServer,
    secrets: &MemorySecretStore,
) -> Result<SignedIn, AuthError> {
    let auth = authenticator(mock);
    let code = block_on(auth.start_device_code(ClientIdentity::MinecraftLauncher))?;
    let tokens = block_on(auth.complete_device_code(&code, &no_sleep()))?;
    block_on(auth.finish_sign_in(tokens, secrets))
}

fn notch() -> Uuid {
    Uuid::parse_str(support::PROFILE_ID).unwrap()
}

#[test]
fn device_code_sign_in_uses_the_minecraft_launcher_client() {
    let mock = MockServer::start();
    mock.state().polls = vec![Poll::Pending, Poll::Granted];
    let secrets = MemorySecretStore::new();
    let auth = authenticator(&mock);

    let code = block_on(auth.start_device_code(ClientIdentity::MinecraftLauncher)).unwrap();
    assert_eq!(code.identity(), ClientIdentity::MinecraftLauncher);
    assert_eq!(code.user_code, "LIVE1234");
    assert_eq!(code.verification_uri, "https://www.microsoft.com/link");
    assert!(!format!("{code:?}").contains(support::LIVE_DEVICE_CODE));
    let tokens = block_on(auth.complete_device_code(&code, &no_sleep())).unwrap();
    assert_eq!(tokens.identity, ClientIdentity::MinecraftLauncher);
    let signed_in = block_on(auth.finish_sign_in(tokens, &secrets)).unwrap();

    // The whole chain ran, with login.live.com in place of the Entra endpoints.
    assert_eq!(
        mock.paths(),
        [
            "/live/oauth20_connect.srf",
            "/live/oauth20_token.srf",
            "/live/oauth20_token.srf",
            "/xbl/user/authenticate",
            "/xsts/xsts/authorize",
            "/mc/authentication/login_with_xbox",
            "/mc/entitlements/mcstore",
            "/mc/minecraft/profile",
        ]
    );
    let requests = mock.requests();
    assert_eq!(
        fields(&requests[0]),
        pairs(&[
            ("client_id", MINECRAFT_LAUNCHER_CLIENT_ID),
            ("scope", MINECRAFT_LAUNCHER_SCOPE),
            ("response_type", "device_code"),
        ])
    );
    for poll in &requests[1..3] {
        assert_eq!(
            fields(poll),
            pairs(&[
                ("client_id", MINECRAFT_LAUNCHER_CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", support::LIVE_DEVICE_CODE),
            ])
        );
    }
    // Lodestone's own client ID is never sent.
    assert!(
        mock.requests()
            .iter()
            .all(|r| !r.body.contains(support::CLIENT_ID))
    );
    // A title client's token goes to Xbox Live as `t=<token>`.
    assert_eq!(rps_tickets(&mock), [format!("t={}", support::LIVE_ACCESS)]);

    // The account records the identity; the refresh token is in the secret store.
    assert_eq!(
        signed_in.account,
        Account::microsoft(
            notch(),
            "Notch",
            Some(format!(
                "http://textures.minecraft.net/texture/{}",
                support::SKIN_HASH
            )),
        )
        .with_client(ClientIdentity::MinecraftLauncher)
    );
    assert!(signed_in.account.uses_launcher_identity());
    assert_eq!(signed_in.session.access_token.expose(), support::MC_TOKEN);
    let key = signed_in.account.secret_key().unwrap();
    assert_eq!(secrets.keys(), std::slice::from_ref(&key));
    assert_eq!(
        secrets.get(&key).unwrap().expose(),
        "live-refresh-SECRET-device"
    );
}

#[test]
fn lodestones_app_still_sends_a_d_ticket() {
    let mock = MockServer::start();
    let auth = authenticator(&mock);
    let code = block_on(auth.start_device_code(ClientIdentity::Lodestone)).unwrap();
    let tokens = block_on(auth.complete_device_code(&code, &no_sleep())).unwrap();
    assert_eq!(tokens.identity, ClientIdentity::Lodestone);
    let signed_in = block_on(auth.finish_sign_in(tokens, &MemorySecretStore::new())).unwrap();
    assert_eq!(signed_in.account.client, ClientIdentity::Lodestone);
    assert!(!signed_in.account.uses_launcher_identity());
    assert_eq!(rps_tickets(&mock), [format!("d={}", support::MS_ACCESS)]);
    assert!(
        mock.paths().iter().all(|path| !path.starts_with("/live/")),
        "{:?}",
        mock.paths()
    );
}

#[test]
fn device_code_declines_map_like_the_entra_flow() {
    for (polls, expected) in [
        (vec![Poll::Pending, Poll::Declined], AuthError::Declined),
        (vec![Poll::Expired], AuthError::DeviceCodeExpired),
    ] {
        let mock = MockServer::start();
        mock.state().polls = polls;
        let err = device_code_sign_in(&mock, &MemorySecretStore::new()).unwrap_err();
        assert_eq!(err, expected);
    }
}

/// What the browser's address bar shows after signing in at the authorize URL.
fn desktop_page(state: Option<&str>) -> String {
    let mut url = url::Url::parse(REDIRECT_URI).unwrap();
    url.query_pairs_mut()
        .append_pair("code", support::LIVE_AUTH_CODE)
        .append_pair("lc", "1033");
    if let Some(state) = state {
        url.query_pairs_mut().append_pair("state", state);
    }
    url.into()
}

fn state_of(url: &url::Url) -> String {
    url.query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .unwrap()
}

#[test]
fn link_sign_in_exchanges_the_pasted_code() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let auth = authenticator(&mock);
    let sign_in = auth.start_link_sign_in();
    let url = sign_in.url().clone();
    assert_eq!(url.path(), "/live/oauth20_authorize.srf");
    let state = state_of(&url);

    // With the state echoed back, and without it.
    for pasted in [desktop_page(Some(&state)), desktop_page(None)] {
        let tokens =
            block_on(auth.complete_link_sign_in(&sign_in, &format!(" {pasted}\n"))).unwrap();
        assert_eq!(tokens.identity, ClientIdentity::MinecraftLauncher);
        let signed_in = block_on(auth.finish_sign_in(tokens, &secrets)).unwrap();
        assert_eq!(signed_in.account.client, ClientIdentity::MinecraftLauncher);
    }

    let token_requests = requests_to(&mock, "/live/oauth20_token.srf");
    assert_eq!(token_requests.len(), 2);
    for request in &token_requests {
        assert_eq!(
            fields(request),
            pairs(&[
                ("client_id", MINECRAFT_LAUNCHER_CLIENT_ID),
                ("scope", MINECRAFT_LAUNCHER_SCOPE),
                ("grant_type", "authorization_code"),
                ("code", support::LIVE_AUTH_CODE),
                ("redirect_uri", REDIRECT_URI),
            ])
        );
    }
    assert_eq!(
        rps_tickets(&mock),
        [
            format!("t={}", support::LIVE_ACCESS),
            format!("t={}", support::LIVE_ACCESS)
        ]
    );
}

#[test]
fn a_bad_pasted_link_sends_nothing() {
    let mock = MockServer::start();
    let auth = authenticator(&mock);
    let sign_in = auth.start_link_sign_in();
    for (pasted, expected) in [
        (
            format!(
                "https://login.live.com.evil.example/oauth20_desktop.srf?code={}",
                support::LIVE_AUTH_CODE
            ),
            AuthError::InvalidLink(LinkProblem::WrongPage),
        ),
        (
            format!(
                "https://login.live.com@evil.example/oauth20_desktop.srf?code={}",
                support::LIVE_AUTH_CODE
            ),
            AuthError::InvalidLink(LinkProblem::WrongPage),
        ),
        (
            // The mock's own address is not login.live.com either.
            mock.base
                .join(&format!(
                    "live/oauth20_desktop.srf?code={}",
                    support::LIVE_AUTH_CODE
                ))
                .unwrap()
                .to_string(),
            AuthError::InvalidLink(LinkProblem::WrongPage),
        ),
        (
            "https://login.live.com/oauth20_desktop.srf?lc=1033".to_string(),
            AuthError::InvalidLink(LinkProblem::NoCode),
        ),
        (
            desktop_page(Some("forged")),
            AuthError::StateMismatch,
        ),
        (
            "https://login.live.com/oauth20_desktop.srf?error=invalid_request&error_description=The%20request%20is%20not%20valid."
                .to_string(),
            AuthError::LinkError {
                code: "invalid_request".into(),
                description: Some("The request is not valid.".into()),
            },
        ),
    ] {
        let err = block_on(auth.complete_link_sign_in(&sign_in, &pasted)).unwrap_err();
        assert_eq!(err, expected, "{pasted}");
    }
    assert!(mock.requests().is_empty(), "{:?}", mock.paths());
}

#[test]
fn refresh_goes_to_the_client_that_signed_the_account_in() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let auth = authenticator(&mock);

    // An account signed in as the Minecraft Launcher refreshes at login.live.com.
    let launcher = device_code_sign_in(&mock, &secrets).unwrap().account;
    let key = launcher.secret_key().unwrap();
    let before = mock.requests().len();
    let refreshed = block_on(auth.refresh(&launcher, &secrets)).unwrap();
    assert_eq!(refreshed.account, launcher);
    assert_eq!(refreshed.account.client, ClientIdentity::MinecraftLauncher);
    let after: Vec<Recorded> = mock.requests().split_off(before);
    assert_eq!(after[0].path, "/live/oauth20_token.srf");
    assert_eq!(
        fields(&after[0]),
        pairs(&[
            ("client_id", MINECRAFT_LAUNCHER_CLIENT_ID),
            ("scope", MINECRAFT_LAUNCHER_SCOPE),
            ("grant_type", "refresh_token"),
            ("refresh_token", "live-refresh-SECRET-device"),
        ])
    );
    assert_eq!(after[1].path, "/xbl/user/authenticate");
    assert!(
        after[1]
            .body
            .contains(&format!("t={}", support::LIVE_ACCESS))
    );
    // login.live.com rotated the token, and the new one was saved.
    assert_eq!(secrets.get(&key).unwrap().expose(), "live-refresh-SECRET-1");
    assert!(after.iter().all(|r| r.path != "/login/token"));

    // A Lodestone account (here, the same profile signed in again with Lodestone's
    // app) refreshes at the Entra endpoint.
    let lodestone = {
        let code = block_on(auth.start_device_code(ClientIdentity::Lodestone)).unwrap();
        let tokens = block_on(auth.complete_device_code(&code, &no_sleep())).unwrap();
        block_on(auth.finish_sign_in(tokens, &secrets))
            .unwrap()
            .account
    };
    assert_eq!(lodestone.client, ClientIdentity::Lodestone);
    let before = mock.requests().len();
    let refreshed = block_on(auth.refresh(&lodestone, &secrets)).unwrap();
    assert_eq!(refreshed.account.client, ClientIdentity::Lodestone);
    let after: Vec<Recorded> = mock.requests().split_off(before);
    assert_eq!(after[0].path, "/login/token");
    assert!(after.iter().all(|r| !r.path.starts_with("/live/")));
}

#[test]
fn a_refresh_token_only_works_with_its_own_client() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let auth = authenticator(&mock);
    let launcher = device_code_sign_in(&mock, &secrets).unwrap().account;

    // Recorded as Lodestone's but holding a login.live.com token: Entra refuses it.
    let mislabelled = launcher.clone().with_client(ClientIdentity::Lodestone);
    let err = block_on(auth.refresh(&mislabelled, &secrets)).unwrap_err();
    assert_eq!(err, AuthError::SignInExpired);

    // And the other way round.
    block_on(secrets.write(
        &launcher.secret_key().unwrap(),
        &Secret::new("refresh-SECRET-0"),
    ))
    .unwrap();
    mock.state()
        .valid_refresh_tokens
        .insert("refresh-SECRET-0".into());
    let err = block_on(auth.refresh(&launcher, &secrets)).unwrap_err();
    assert_eq!(err, AuthError::SignInExpired);
    assert!(err.needs_sign_in());
}

#[test]
fn the_identity_survives_the_account_file_and_no_token_does() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accounts.json");
    let launcher = device_code_sign_in(&mock, &secrets).unwrap().account;
    {
        let (mut store, _) = AccountStore::open(&path);
        store.upsert_and_activate(launcher.clone()).unwrap();
    }
    let (store, _) = AccountStore::open(&path);
    let reopened = store.get(&launcher.id).unwrap().clone();
    assert_eq!(reopened.client, ClientIdentity::MinecraftLauncher);
    // Refreshing the reopened account still goes to login.live.com.
    block_on(authenticator(&mock).refresh(&reopened, &secrets)).unwrap();
    assert_eq!(
        mock.requests().last().map(|r| r.path.clone()).as_deref(),
        Some("/mc/minecraft/profile")
    );
    assert_eq!(
        requests_to(&mock, "/live/oauth20_token.srf").len(),
        2,
        "one poll and one refresh"
    );

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"minecraft_launcher\""), "{text}");
    assert!(!text.contains("SECRET"), "{text}");
    assert_only_secret_free_files(dir.path());
}

fn assert_only_secret_free_files(dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        assert!(
            !text.contains("SECRET"),
            "{} holds a secret",
            path.display()
        );
    }
}

/// Recorded sleeps, to check the live.com poll follows the interval.
#[test]
fn polls_at_the_interval_login_live_com_asks_for() {
    let mock = MockServer::start();
    mock.state().polls = vec![Poll::Pending, Poll::SlowDown, Poll::Granted];
    let auth = authenticator(&mock);
    let code = block_on(auth.start_device_code(ClientIdentity::MinecraftLauncher)).unwrap();
    let slept = Rc::new(RefCell::new(Vec::new()));
    let record = slept.clone();
    let sleep = move |duration: Duration| {
        record.borrow_mut().push(duration.as_secs());
        futures::future::ready(()).boxed_local()
    };
    block_on(auth.complete_device_code(&code, &sleep)).unwrap();
    assert_eq!(*slept.borrow(), [5, 5, 10]);
}
