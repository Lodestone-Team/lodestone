//! The whole sign-in chain against a loopback mock server: the browser and device
//! code flows, every mapped error, refreshing, and what ends up on disk.

mod support;

use std::{
    cell::RefCell,
    io::{Read as _, Write as _},
    path::Path,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use futures::{FutureExt as _, executor::block_on};
use launcher_core::{
    MemorySecretStore, Secret, SecretStore as _, USER_AGENT,
    accounts::{Account, AccountStore},
    auth::{AuthError, Authenticator, DEVICE_LOGIN_URL, MsTokens, SignedIn, XboxError},
    http, settings, skin, storage,
};
use support::{Login, MockServer, Poll};
use uuid::Uuid;

fn authenticator(mock: &MockServer) -> Authenticator {
    Authenticator::new(http::default_client().unwrap(), mock.config())
}

/// Plays the browser: follows the authorize URL's redirect back to the loopback
/// listener with the code (or `code`) and the given `state`.
fn browser_redirect(redirect_uri: &str, query: &str) -> String {
    let port: u16 = redirect_uri.rsplit(':').next().unwrap().parse().unwrap();
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "GET /?{query} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// Signs in through the browser flow, with the mock checking PKCE.
fn browser_sign_in(mock: &MockServer, auth: &Authenticator) -> Result<MsTokens, AuthError> {
    let sign_in = auth.start_browser_sign_in(Duration::from_secs(60)).unwrap();
    let url = sign_in.url().clone();
    let query = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
            .unwrap()
    };
    assert_eq!(query("client_id"), support::CLIENT_ID);
    assert_eq!(query("code_challenge_method"), "S256");
    assert_eq!(query("redirect_uri"), sign_in.redirect_uri());
    {
        let mut state = mock.state();
        state.expected_challenge = Some(query("code_challenge"));
        state.expected_redirect_uri = Some(sign_in.redirect_uri().to_string());
    }
    let page = browser_redirect(
        sign_in.redirect_uri(),
        &format!("code={}&state={}", support::AUTH_CODE, query("state")),
    );
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("You can close this tab"));
    block_on(auth.complete_browser_sign_in(sign_in))
}

fn sign_in_fully(mock: &MockServer, secrets: &MemorySecretStore) -> Result<SignedIn, AuthError> {
    let auth = authenticator(mock);
    let tokens = browser_sign_in(mock, &auth)?;
    block_on(auth.finish_sign_in(tokens, secrets))
}

fn notch() -> Uuid {
    Uuid::parse_str(support::PROFILE_ID).unwrap()
}

/// Every file under `dir`, recursively.
fn all_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(all_files(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn browser_sign_in_runs_the_whole_chain() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let signed_in = sign_in_fully(&mock, &secrets).unwrap();

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
    );
    assert_eq!(signed_in.session.access_token.expose(), support::MC_TOKEN);
    assert!(!signed_in.session.is_expired(std::time::SystemTime::now()));
    // The refresh token is in the secret store, under the account's key.
    let key = signed_in.account.secret_key().unwrap();
    assert_eq!(secrets.keys(), std::slice::from_ref(&key));
    assert_eq!(secrets.get(&key).unwrap().expose(), "refresh-SECRET-0");

    assert_eq!(
        mock.paths(),
        [
            "/login/token",
            "/xbl/user/authenticate",
            "/xsts/xsts/authorize",
            "/mc/authentication/login_with_xbox",
            "/mc/entitlements/mcstore",
            "/mc/minecraft/profile",
        ]
    );
    // Every request identifies the app, and only the app.
    for request in mock.requests() {
        assert_eq!(
            request.user_agent.as_deref(),
            Some(USER_AGENT),
            "{}",
            request.path
        );
    }
    // The token request carries the verifier and the scopes.
    let token_request = &mock.requests()[0].body;
    assert!(token_request.contains("code_verifier="));
    assert!(token_request.contains("scope=XboxLive.signin+offline_access"));
}

#[test]
fn browser_sign_in_rejects_a_state_mismatch() {
    let mock = MockServer::start();
    let auth = authenticator(&mock);
    let sign_in = auth.start_browser_sign_in(Duration::from_secs(60)).unwrap();
    let page = browser_redirect(
        sign_in.redirect_uri(),
        &format!("code={}&state=forged", support::AUTH_CODE),
    );
    assert!(page.starts_with("HTTP/1.1 400"), "{page}");
    assert_eq!(
        block_on(auth.complete_browser_sign_in(sign_in)).unwrap_err(),
        AuthError::StateMismatch
    );
    // The code was never exchanged.
    assert!(mock.requests().is_empty());
}

#[test]
fn browser_sign_in_can_be_cancelled_and_times_out() {
    let mock = MockServer::start();
    let auth = authenticator(&mock);

    let sign_in = auth.start_browser_sign_in(Duration::from_secs(60)).unwrap();
    sign_in.cancel_handle().cancel();
    assert_eq!(
        block_on(auth.complete_browser_sign_in(sign_in)).unwrap_err(),
        AuthError::Cancelled
    );

    let sign_in = auth
        .start_browser_sign_in(Duration::from_millis(10))
        .unwrap();
    assert_eq!(
        block_on(auth.complete_browser_sign_in(sign_in)).unwrap_err(),
        AuthError::TimedOut
    );
    assert!(mock.requests().is_empty());
}

#[test]
fn maps_each_xerr_code() {
    for (code, expected) in [
        (2148916233, XboxError::NoXboxAccount),
        (2148916235, XboxError::Region),
        (2148916236, XboxError::AdultVerification),
        (2148916237, XboxError::AdultVerification),
        (2148916238, XboxError::ChildAccount),
        (2148916227, XboxError::Other(2148916227)),
    ] {
        let mock = MockServer::start();
        mock.state().xerr = Some(code);
        let secrets = MemorySecretStore::new();
        let err = sign_in_fully(&mock, &secrets).unwrap_err();
        assert_eq!(err, AuthError::Xbox(expected), "{code}");
        assert!(!err.message().is_empty());
        // Nothing is saved for a failed sign-in.
        assert!(secrets.keys().is_empty());
    }
}

#[test]
fn reports_the_app_is_not_approved_yet() {
    let mock = MockServer::start();
    mock.state().login = Login::NotApproved;
    let secrets = MemorySecretStore::new();
    let err = sign_in_fully(&mock, &secrets).unwrap_err();
    assert_eq!(err, AuthError::AppNotApproved);
    assert!(
        err.message()
            .contains("hasn't approved Lodestone Launcher yet")
    );
    assert!(!err.needs_sign_in());
    assert!(secrets.keys().is_empty());
}

#[test]
fn reports_missing_ownership() {
    let mock = MockServer::start();
    {
        let mut state = mock.state();
        state.entitlements.clear();
        state.has_profile = false;
    }
    let err = sign_in_fully(&mock, &MemorySecretStore::new()).unwrap_err();
    assert_eq!(err, AuthError::NotOwned);
}

#[test]
fn reports_a_game_pass_account_without_a_profile() {
    let mock = MockServer::start();
    mock.state().has_profile = false;
    let err = sign_in_fully(&mock, &MemorySecretStore::new()).unwrap_err();
    assert_eq!(err, AuthError::NoProfile);
    assert!(err.message().contains("Game Pass"));
}

#[test]
fn accepts_a_profile_without_a_store_entitlement() {
    // Game Pass accounts can have a profile but no `mcstore` entitlement.
    let mock = MockServer::start();
    mock.state().entitlements.clear();
    let signed_in = sign_in_fully(&mock, &MemorySecretStore::new()).unwrap();
    assert_eq!(signed_in.account.username, "Notch");
}

#[test]
fn refresh_rotates_the_refresh_token() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let account = sign_in_fully(&mock, &secrets).unwrap().account;
    let key = account.secret_key().unwrap();
    let auth = authenticator(&mock);

    let refreshed = block_on(auth.refresh(&account, &secrets)).unwrap();
    assert_eq!(refreshed.account, account);
    assert_eq!(refreshed.session.access_token.expose(), support::MC_TOKEN);
    // The rotated token replaced the old one, which the server revoked.
    assert_eq!(secrets.get(&key).unwrap().expose(), "refresh-SECRET-1");
    assert!(
        !mock
            .state()
            .valid_refresh_tokens
            .contains("refresh-SECRET-0")
    );

    // The next refresh uses the new token.
    block_on(auth.refresh(&account, &secrets)).unwrap();
    assert_eq!(secrets.get(&key).unwrap().expose(), "refresh-SECRET-2");
    let refresh_requests: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.body.contains("grant_type=refresh_token"))
        .map(|r| r.body)
        .collect();
    assert_eq!(refresh_requests.len(), 2);
    assert!(refresh_requests[0].contains("refresh_token=refresh-SECRET-0"));
    assert!(refresh_requests[1].contains("refresh_token=refresh-SECRET-1"));
}

#[test]
fn keeps_the_refresh_token_when_it_is_not_rotated() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let account = sign_in_fully(&mock, &secrets).unwrap().account;
    mock.state().rotate = false;
    block_on(authenticator(&mock).refresh(&account, &secrets)).unwrap();
    assert_eq!(
        secrets
            .get(&account.secret_key().unwrap())
            .unwrap()
            .expose(),
        "refresh-SECRET-0"
    );
}

#[test]
fn an_expired_refresh_token_asks_to_sign_in_again() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let account = Account::microsoft(notch(), "Notch", None);
    let auth = authenticator(&mock);

    // No saved token at all.
    let err = block_on(auth.refresh(&account, &secrets)).unwrap_err();
    assert_eq!(err, AuthError::NoSavedSignIn);
    assert!(err.needs_sign_in());

    // A token the server no longer accepts.
    block_on(secrets.write(
        &account.secret_key().unwrap(),
        &Secret::new("refresh-SECRET-revoked"),
    ))
    .unwrap();
    let err = block_on(auth.refresh(&account, &secrets)).unwrap_err();
    assert_eq!(err, AuthError::SignInExpired);
    assert!(err.needs_sign_in());
    // Only the token endpoint was asked.
    assert_eq!(mock.paths(), ["/login/token"]);

    // Signing in again recovers the account.
    let signed_in = sign_in_fully(&mock, &secrets).unwrap();
    assert_eq!(signed_in.account.id, account.id);
    block_on(auth.refresh(&signed_in.account, &secrets)).unwrap();
}

#[test]
fn a_refresh_for_another_profile_asks_to_sign_in_again() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let signed_in = sign_in_fully(&mock, &secrets).unwrap();
    // A different account whose keychain entry holds Notch's token.
    let other = Account::microsoft(Uuid::from_u128(7), "Other", None);
    let token = secrets
        .get(&signed_in.account.secret_key().unwrap())
        .unwrap();
    block_on(secrets.write(&other.secret_key().unwrap(), &token)).unwrap();
    let err = block_on(authenticator(&mock).refresh(&other, &secrets)).unwrap_err();
    assert_eq!(err, AuthError::SignInExpired);
}

/// A sleep that returns at once and records how long it was asked to wait.
fn recording_sleep() -> (
    Rc<RefCell<Vec<u64>>>,
    impl Fn(Duration) -> futures::future::LocalBoxFuture<'static, ()>,
) {
    let slept = Rc::new(RefCell::new(Vec::new()));
    let record = slept.clone();
    (slept, move |duration: Duration| {
        record.borrow_mut().push(duration.as_secs());
        futures::future::ready(()).boxed_local()
    })
}

#[test]
fn device_code_flow_waits_and_slows_down() {
    let mock = MockServer::start();
    mock.state().polls = vec![Poll::Pending, Poll::SlowDown, Poll::Pending, Poll::Granted];
    let auth = authenticator(&mock);
    let code = block_on(auth.start_device_code()).unwrap();
    assert_eq!(code.user_code, "ABCD-EFGH");
    // The mock suggests a URL on a host the launcher does not trust.
    assert_eq!(code.verification_uri, DEVICE_LOGIN_URL);
    assert_eq!(code.expires_in, Duration::from_secs(900));
    assert!(!format!("{code:?}").contains(support::DEVICE_CODE));

    let (slept, sleep) = recording_sleep();
    let tokens = block_on(auth.complete_device_code(&code, &sleep)).unwrap();
    assert_eq!(*slept.borrow(), [5, 5, 10, 10]);
    let secrets = MemorySecretStore::new();
    let signed_in = block_on(auth.finish_sign_in(tokens, &secrets)).unwrap();
    assert_eq!(signed_in.account.username, "Notch");
    assert_eq!(
        secrets
            .get(&signed_in.account.secret_key().unwrap())
            .unwrap()
            .expose(),
        "refresh-SECRET-device"
    );
}

#[test]
fn device_code_flow_reports_declines_and_expiry() {
    for (polls, expected) in [
        (vec![Poll::Pending, Poll::Declined], AuthError::Declined),
        (vec![Poll::Expired], AuthError::DeviceCodeExpired),
        // The server never answers; the code runs out after 900 s of 5 s waits.
        (vec![Poll::Pending], AuthError::DeviceCodeExpired),
    ] {
        let mock = MockServer::start();
        mock.state().polls = polls;
        let auth = authenticator(&mock);
        let code = block_on(auth.start_device_code()).unwrap();
        let (slept, sleep) = recording_sleep();
        let err = block_on(auth.complete_device_code(&code, &sleep)).unwrap_err();
        assert_eq!(err, expected);
        assert!(slept.borrow().iter().sum::<u64>() <= 900);
    }
}

#[test]
fn reports_network_errors() {
    // A port with nothing listening on it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let base = url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    let config = launcher_core::auth::MsaConfig {
        client_id: "x".into(),
        endpoints: launcher_core::auth::Endpoints::all_at(&base),
    };
    let auth = Authenticator::new(http::default_client().unwrap(), config);
    let err = block_on(auth.start_device_code()).unwrap_err();
    assert!(matches!(err, AuthError::Network(_)), "{err:?}");
}

#[test]
fn secrets_never_reach_the_data_directory() {
    let mock = MockServer::start();
    let secrets = MemorySecretStore::new();
    let dir = tempfile::tempdir().unwrap();
    let data = launcher_core::paths::DataDir::at(dir.path());

    let signed_in = sign_in_fully(&mock, &secrets).unwrap();
    let (mut store, _) = AccountStore::open(data.accounts_file());
    store
        .upsert_and_activate(signed_in.account.clone())
        .unwrap();
    store
        .upsert_and_activate(Account::offline("Dev_1").unwrap())
        .unwrap();
    storage::save(&data.settings_file(), &settings::Settings::default()).unwrap();
    let refreshed = block_on(authenticator(&mock).refresh(&signed_in.account, &secrets)).unwrap();
    store
        .update_profile(
            &refreshed.account.id,
            &refreshed.account.username,
            refreshed.account.skin_url.clone(),
        )
        .unwrap();
    // The skin head is cached in the data directory too.
    let head = block_on(skin::cached_head(
        &*http::default_client().unwrap(),
        &mock.config().endpoints.textures,
        &data.avatar_cache_dir(),
        signed_in.account.skin_url.as_deref().unwrap(),
    ))
    .unwrap();
    assert_eq!(
        head,
        data.avatar_cache_dir()
            .join(format!("{}.png", support::SKIN_HASH))
    );

    let accounts_json = std::fs::read_to_string(data.accounts_file()).unwrap();
    assert!(accounts_json.contains("Notch"), "{accounts_json}");
    assert!(accounts_json.contains("Dev_1"));
    let files = all_files(dir.path());
    assert_eq!(files.len(), 3, "{files:?}");
    for file in files {
        let contents = std::fs::read(&file).unwrap();
        let text = String::from_utf8_lossy(&contents);
        // Every token the mock hands out contains "SECRET".
        assert!(
            !text.contains("SECRET"),
            "{} holds a secret",
            file.display()
        );
        for token in [
            support::MS_ACCESS,
            support::XBL_TOKEN,
            support::XSTS_TOKEN,
            support::MC_TOKEN,
            "refresh-SECRET",
        ] {
            assert!(!text.contains(token), "{} holds {token}", file.display());
        }
    }
    // The secrets are all in the secret store.
    assert_eq!(secrets.keys(), [signed_in.account.secret_key().unwrap()]);
}

#[test]
fn caches_skin_heads_and_refuses_other_hosts() {
    let mock = MockServer::start();
    let client: Arc<dyn http_client::HttpClient> = http::default_client().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let textures = mock.config().endpoints.textures;
    let url = format!(
        "http://textures.minecraft.net/texture/{}",
        support::SKIN_HASH
    );

    let path = block_on(skin::cached_head(&*client, &textures, dir.path(), &url)).unwrap();
    let head = image::open(&path).unwrap().to_rgba8();
    assert_eq!(
        head.dimensions(),
        (8 * skin::HEAD_SCALE, 8 * skin::HEAD_SCALE)
    );
    // A second call uses the cache.
    block_on(skin::cached_head(&*client, &textures, dir.path(), &url)).unwrap();
    assert_eq!(
        mock.paths(),
        [format!("/textures/texture/{}", support::SKIN_HASH)]
    );

    let err = block_on(skin::cached_head(
        &*client,
        &textures,
        dir.path(),
        "https://evil.example/texture/abc",
    ))
    .unwrap_err();
    assert!(matches!(err, skin::SkinError::UntrustedUrl));
    assert_eq!(mock.paths().len(), 1);
}
