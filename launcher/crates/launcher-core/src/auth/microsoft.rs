//! The Microsoft identity platform: the authorization code flow with PKCE, the
//! device code flow, and refreshing.

use std::time::Duration;

use futures::future::LocalBoxFuture;
use http_client::HttpClient;
use serde::Deserialize;
use url::Url;

use super::{AuthError, DEVICE_LOGIN_URL, MsaConfig, SCOPES, Step, pkce::Pkce};
use crate::{
    Secret,
    http::{self, RequestError, Response},
};

/// The tokens from the Microsoft token endpoint.
#[derive(Debug)]
pub struct MsTokens {
    pub access_token: Secret,
    /// Present when `offline_access` was granted. Microsoft rotates it on refresh.
    pub refresh_token: Option<Secret>,
}

/// The token endpoint's answer. Not `Debug`, as it holds tokens.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
}

/// An OAuth 2.0 error response.
#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
}

/// The authorization request URL to open in the browser.
pub fn authorize_url(config: &MsaConfig, redirect_uri: &str, pkce: &Pkce, state: &str) -> Url {
    let mut url = Url::parse(&http::join(&config.endpoints.login, "authorize"))
        .expect("the login endpoint is a valid URL");
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        // Let the user pick which account to add.
        .append_pair("prompt", "select_account");
    url
}

pub(crate) fn network_error(err: RequestError) -> AuthError {
    AuthError::Network(err.to_string())
}

/// Exchanges an authorization code for tokens.
pub async fn exchange_code(
    client: &dyn HttpClient,
    config: &MsaConfig,
    code: &Secret,
    redirect_uri: &str,
    pkce: &Pkce,
) -> Result<MsTokens, AuthError> {
    let request = http::form(
        &http::join(&config.endpoints.login, "token"),
        &[
            ("client_id", &config.client_id),
            ("grant_type", "authorization_code"),
            ("code", code.expose()),
            ("redirect_uri", redirect_uri),
            ("code_verifier", pkce.verifier.expose()),
            ("scope", SCOPES),
        ],
    )
    .map_err(|err| AuthError::Network(err.to_string()))?;
    let response = http::send(client, request).await.map_err(network_error)?;
    token_response(&response, false)
}

/// Gets new tokens with a refresh token. A refused refresh token is
/// [`AuthError::SignInExpired`], which means the user has to sign in again.
pub async fn refresh(
    client: &dyn HttpClient,
    config: &MsaConfig,
    refresh_token: &Secret,
) -> Result<MsTokens, AuthError> {
    let request = http::form(
        &http::join(&config.endpoints.login, "token"),
        &[
            ("client_id", &config.client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.expose()),
            ("scope", SCOPES),
        ],
    )
    .map_err(|err| AuthError::Network(err.to_string()))?;
    let response = http::send(client, request).await.map_err(network_error)?;
    token_response(&response, true)
}

fn token_response(response: &Response, refreshing: bool) -> Result<MsTokens, AuthError> {
    if response.is_success() {
        let tokens: TokenResponse = response.json().map_err(|_| AuthError::UnexpectedResponse {
            step: Step::MicrosoftToken,
            status: response.status,
        })?;
        return Ok(MsTokens {
            access_token: Secret::new(tokens.access_token),
            refresh_token: tokens.refresh_token.map(Secret::new),
        });
    }
    if response.status == 429 {
        return Err(AuthError::RateLimited);
    }
    match response.json::<OAuthError>() {
        Ok(OAuthError { error }) => Err(match error.as_str() {
            "invalid_grant" | "interaction_required" | "consent_required" | "login_required"
                if refreshing =>
            {
                AuthError::SignInExpired
            }
            "access_denied" => AuthError::Declined,
            _ => AuthError::Microsoft { code: error },
        }),
        Err(_) => Err(AuthError::UnexpectedResponse {
            step: Step::MicrosoftToken,
            status: response.status,
        }),
    }
}

/// A device code to show the user.
#[derive(Debug)]
pub struct DeviceCode {
    /// The code the user types at [`DeviceCode::verification_uri`].
    pub user_code: String,
    /// Where to enter the code: Microsoft's, or [`DEVICE_LOGIN_URL`] if the server
    /// suggested a URL on a host we do not recognise.
    pub verification_uri: String,
    pub expires_in: Duration,
    device_code: Secret,
    interval: Duration,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[serde(default)]
    verification_uri: Option<String>,
    expires_in: u64,
    #[serde(default)]
    interval: Option<u64>,
}

/// The hosts whose verification URIs the launcher opens as given.
const VERIFICATION_HOSTS: &[&str] = &[
    "www.microsoft.com",
    "microsoft.com",
    "login.microsoft.com",
    "login.live.com",
];

fn verification_uri(suggested: Option<&str>) -> String {
    suggested
        .and_then(|uri| Url::parse(uri).ok())
        .filter(|url| {
            url.scheme() == "https"
                && url
                    .host_str()
                    .is_some_and(|host| VERIFICATION_HOSTS.contains(&host))
        })
        .map(String::from)
        .unwrap_or_else(|| DEVICE_LOGIN_URL.to_string())
}

/// Asks for a device code.
pub async fn start_device_code(
    client: &dyn HttpClient,
    config: &MsaConfig,
) -> Result<DeviceCode, AuthError> {
    let request = http::form(
        &http::join(&config.endpoints.login, "devicecode"),
        &[("client_id", &config.client_id), ("scope", SCOPES)],
    )
    .map_err(|err| AuthError::Network(err.to_string()))?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if !response.is_success() {
        return Err(match response.json::<OAuthError>() {
            Ok(OAuthError { error }) => AuthError::Microsoft { code: error },
            Err(_) => AuthError::UnexpectedResponse {
                step: Step::DeviceCode,
                status: response.status,
            },
        });
    }
    let code: DeviceCodeResponse = response.json().map_err(|_| AuthError::UnexpectedResponse {
        step: Step::DeviceCode,
        status: response.status,
    })?;
    Ok(DeviceCode {
        user_code: code.user_code,
        verification_uri: verification_uri(code.verification_uri.as_deref()),
        expires_in: Duration::from_secs(code.expires_in),
        device_code: Secret::new(code.device_code),
        interval: Duration::from_secs(code.interval.unwrap_or(5)),
    })
}

/// Waits for a duration. The app passes its executor's timer; tests pass one that
/// returns at once and records the durations.
pub type Sleep<'a> = &'a dyn Fn(Duration) -> LocalBoxFuture<'static, ()>;

/// Polls the token endpoint until the user has entered the code, at the interval
/// the server asks for (longer after `slow_down`).
///
/// Drop the future to cancel. The time spent waiting is counted from the
/// intervals, so the code's expiry is checked without reading the clock.
pub async fn poll_device_code(
    client: &dyn HttpClient,
    config: &MsaConfig,
    code: &DeviceCode,
    sleep: Sleep<'_>,
) -> Result<MsTokens, AuthError> {
    let token_url = http::join(&config.endpoints.login, "token");
    let mut interval = code.interval;
    let mut waited = Duration::ZERO;
    loop {
        if waited >= code.expires_in {
            return Err(AuthError::DeviceCodeExpired);
        }
        sleep(interval).await;
        waited += interval;

        let request = http::form(
            &token_url,
            &[
                ("client_id", &config.client_id),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", code.device_code.expose()),
            ],
        )
        .map_err(|err| AuthError::Network(err.to_string()))?;
        let response = http::send(client, request).await.map_err(network_error)?;
        if response.is_success() {
            return token_response(&response, false);
        }
        let Ok(OAuthError { error }) = response.json::<OAuthError>() else {
            return Err(AuthError::UnexpectedResponse {
                step: Step::DeviceCode,
                status: response.status,
            });
        };
        match error.as_str() {
            "authorization_pending" => {}
            // RFC 8628 section 3.5: add 5 seconds to the interval.
            "slow_down" => interval += Duration::from_secs(5),
            "authorization_declined" | "access_denied" => return Err(AuthError::Declined),
            "expired_token" | "code_expired" => return Err(AuthError::DeviceCodeExpired),
            _ => return Err(AuthError::Microsoft { code: error }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Endpoints;

    #[test]
    fn builds_the_authorize_url() {
        let config = MsaConfig {
            client_id: "client".into(),
            endpoints: Endpoints::production(),
        };
        let pkce = Pkce::from_verifier("verifier".into());
        let url = authorize_url(&config, "http://localhost:5555", &pkce, "the-state");
        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize"
        );
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("client_id"), Some("client"));
        assert_eq!(get("response_type"), Some("code"));
        assert_eq!(get("redirect_uri"), Some("http://localhost:5555"));
        assert_eq!(get("scope"), Some("XboxLive.signin offline_access"));
        assert_eq!(get("state"), Some("the-state"));
        assert_eq!(get("code_challenge"), Some(pkce.challenge.as_str()));
        assert_eq!(get("code_challenge_method"), Some("S256"));
        // The verifier itself never goes to the browser.
        assert!(!url.as_str().contains("verifier"));
    }

    #[test]
    fn only_opens_known_verification_hosts() {
        assert_eq!(
            verification_uri(Some("https://www.microsoft.com/link")),
            "https://www.microsoft.com/link"
        );
        assert_eq!(
            verification_uri(Some("https://login.live.com/oauth20_remoteconnect.srf")),
            "https://login.live.com/oauth20_remoteconnect.srf"
        );
        for other in [
            Some("http://www.microsoft.com/link"),
            Some("https://evil.example/link"),
            Some("https://microsoft.com.evil.example/"),
            Some("not a url"),
            None,
        ] {
            assert_eq!(verification_uri(other), DEVICE_LOGIN_URL, "{other:?}");
        }
    }
}
