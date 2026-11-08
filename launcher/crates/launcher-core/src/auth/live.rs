//! The Minecraft Launcher's client on login.live.com, for the developer option
//! ([`ClientIdentity::MinecraftLauncher`]).
//!
//! The device code flow and refreshing go through [`microsoft`](super::microsoft)
//! with that identity. This module is the other way in, for when a code is not an
//! option: the browser signs in at `oauth20_authorize.srf` and ends on
//! `https://login.live.com/oauth20_desktop.srf?code=…`, a blank page only Microsoft
//! serves. The user copies that address back into the launcher, and
//! [`parse_redirect`] checks it strictly before the code is used.
//!
//! The pasted address holds a sign-in code, so it is never logged; the code becomes
//! a [`Secret`] as soon as it is read.

use http_client::HttpClient;
use url::Url;

use super::{
    AuthError, ClientIdentity, MsaConfig,
    error::LinkProblem,
    microsoft::{self, MsTokens, OAuthClient},
    pkce::{self, constant_time_eq},
};
use crate::{Secret, http};

/// login.live.com, where the Minecraft Launcher's client signs in.
pub const LIVE_BASE: &str = "https://login.live.com";

/// The page login.live.com sends the browser to with the code. The address the
/// user pastes must be exactly this page.
pub const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";

const REDIRECT_HOST: &str = "login.live.com";
const REDIRECT_PATH: &str = "/oauth20_desktop.srf";

/// The longest error description from a pasted address that is shown.
const MAX_DESCRIPTION: usize = 300;

/// A sign-in by pasted link in progress: open [`LinkSignIn::url`] in the browser,
/// then pass what the user pastes to
/// [`Authenticator::complete_link_sign_in`](super::Authenticator::complete_link_sign_in).
#[derive(Debug)]
pub struct LinkSignIn {
    url: Url,
    state: String,
}

impl LinkSignIn {
    pub(crate) fn new(config: &MsaConfig) -> Self {
        let state = pkce::random_state();
        let oauth = OAuthClient::new(config, ClientIdentity::MinecraftLauncher);
        let mut url = Url::parse(&http::join(&config.endpoints.live, "oauth20_authorize.srf"))
            .expect("the live endpoint is a valid URL");
        url.query_pairs_mut()
            .append_pair("client_id", &oauth.client_id)
            .append_pair("response_type", "code")
            .append_pair("scope", oauth.scope)
            .append_pair("redirect_uri", REDIRECT_URI)
            .append_pair("response_mode", "query")
            .append_pair("state", &state);
        Self { url, state }
    }

    /// The authorization URL to open in the browser.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Checks a pasted address with [`parse_redirect`] against this sign-in's
    /// `state`, and returns its code.
    pub fn check(&self, pasted: &str) -> Result<Secret, AuthError> {
        parse_redirect(pasted, &self.state)
    }
}

/// Reads the code from the address the user pasted, after checking that it is
/// login.live.com's `oauth20_desktop.srf` page and carries no error.
///
/// A `state` in the address must be the one this sign-in sent. An address without
/// one is accepted, as login.live.com does not promise to echo it on this page.
pub fn parse_redirect(pasted: &str, expected_state: &str) -> Result<Secret, AuthError> {
    let url =
        Url::parse(pasted.trim()).map_err(|_| AuthError::InvalidLink(LinkProblem::NotAUrl))?;
    let exact_page = url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str() == Some(REDIRECT_HOST)
        && url.port().is_none()
        && url.path() == REDIRECT_PATH;
    if !exact_page {
        return Err(AuthError::InvalidLink(LinkProblem::WrongPage));
    }

    let (mut codes, mut states, mut error, mut description) = (Vec::new(), Vec::new(), None, None);
    for (key, value) in url.query_pairs() {
        match &*key {
            "code" => codes.push(value.into_owned()),
            "state" => states.push(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => description = Some(value.into_owned()),
            _ => {}
        }
    }
    if let Some(code) = error {
        return Err(match code.as_str() {
            "access_denied" => AuthError::Declined,
            _ => AuthError::LinkError {
                code: clean(&code, 64),
                description: description
                    .map(|text| clean(&text, MAX_DESCRIPTION))
                    .filter(|text| !text.is_empty()),
            },
        });
    }
    match states.as_slice() {
        [] => {}
        [state] if constant_time_eq(state, expected_state) => {}
        _ => return Err(AuthError::StateMismatch),
    }
    match codes.as_slice() {
        [code] if !code.is_empty() => Ok(Secret::new(code.clone())),
        _ => Err(AuthError::InvalidLink(LinkProblem::NoCode)),
    }
}

/// Text from a pasted address, made safe to show: no control characters, and at
/// most `max` characters.
fn clean(text: &str, max: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    match trimmed.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &trimmed[..end]),
        None => trimmed.to_string(),
    }
}

/// Checks the pasted address and exchanges its code for tokens.
pub async fn complete(
    client: &dyn HttpClient,
    config: &MsaConfig,
    sign_in: &LinkSignIn,
    pasted: &str,
) -> Result<MsTokens, AuthError> {
    let code = sign_in.check(pasted)?;
    exchange_code(client, config, &code).await
}

/// Exchanges a code from `oauth20_desktop.srf` for tokens.
pub async fn exchange_code(
    client: &dyn HttpClient,
    config: &MsaConfig,
    code: &Secret,
) -> Result<MsTokens, AuthError> {
    let oauth = OAuthClient::new(config, ClientIdentity::MinecraftLauncher);
    let request = http::form(
        &oauth.token_url,
        &[
            ("client_id", &oauth.client_id),
            ("scope", oauth.scope),
            ("grant_type", "authorization_code"),
            ("code", code.expose()),
            ("redirect_uri", REDIRECT_URI),
        ],
    )
    .map_err(|err| AuthError::Network(err.to_string()))?;
    let response = http::send(client, request)
        .await
        .map_err(microsoft::network_error)?;
    microsoft::token_response(&response, false, ClientIdentity::MinecraftLauncher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Endpoints, MINECRAFT_LAUNCHER_CLIENT_ID, MINECRAFT_LAUNCHER_SCOPE};

    const STATE: &str = "the-state";

    fn parse(url: &str) -> Result<String, AuthError> {
        parse_redirect(url, STATE).map(|code| code.expose().to_string())
    }

    #[test]
    fn builds_the_authorize_url() {
        let config = MsaConfig {
            client_id: "lodestone-client".into(),
            endpoints: Endpoints::production(),
        };
        let sign_in = LinkSignIn::new(&config);
        let url = sign_in.url();
        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            "https://login.live.com/oauth20_authorize.srf"
        );
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        // The Minecraft Launcher's client, never Lodestone's.
        assert_eq!(get("client_id"), Some(MINECRAFT_LAUNCHER_CLIENT_ID));
        assert_eq!(get("scope"), Some(MINECRAFT_LAUNCHER_SCOPE));
        assert_eq!(get("response_type"), Some("code"));
        assert_eq!(get("response_mode"), Some("query"));
        assert_eq!(get("redirect_uri"), Some(REDIRECT_URI));
        assert_eq!(get("state"), Some(sign_in.state.as_str()));
        assert!(!url.as_str().contains("lodestone-client"));
    }

    #[test]
    fn accepts_the_desktop_page_with_a_code() {
        assert_eq!(
            parse("https://login.live.com/oauth20_desktop.srf?code=M.C123_abc&lc=1033").unwrap(),
            "M.C123_abc"
        );
        // Whitespace around a paste, the default port, an upper-case host and the
        // matching state are all fine.
        assert_eq!(
            parse("  https://LOGIN.live.com:443/oauth20_desktop.srf?code=abc&state=the-state\n")
                .unwrap(),
            "abc"
        );
        // Percent-encoding in the code is decoded.
        assert_eq!(
            parse("https://login.live.com/oauth20_desktop.srf?code=a%2Bb").unwrap(),
            "a+b"
        );
    }

    #[test]
    fn rejects_other_pages_and_hosts() {
        let wrong_page = Err(AuthError::InvalidLink(LinkProblem::WrongPage));
        for url in [
            // Not HTTPS.
            "http://login.live.com/oauth20_desktop.srf?code=abc",
            // Lookalike hosts.
            "https://login.live.com.evil.example/oauth20_desktop.srf?code=abc",
            "https://evil-login.live.com/oauth20_desktop.srf?code=abc",
            "https://login-live.com/oauth20_desktop.srf?code=abc",
            "https://login.live.co/oauth20_desktop.srf?code=abc",
            "https://login.live.com./oauth20_desktop.srf?code=abc",
            "https://live.com/oauth20_desktop.srf?code=abc",
            "https://login.microsoftonline.com/oauth20_desktop.srf?code=abc",
            // A Cyrillic "о" in "login" becomes punycode.
            "https://l\u{043e}gin.live.com/oauth20_desktop.srf?code=abc",
            // `@` tricks: the real host is after the `@`, and credentials are refused.
            "https://login.live.com@evil.example/oauth20_desktop.srf?code=abc",
            "https://login.live.com:443@evil.example/oauth20_desktop.srf?code=abc",
            "https://user@login.live.com/oauth20_desktop.srf?code=abc",
            "https://user:pass@login.live.com/oauth20_desktop.srf?code=abc",
            "https://login.live.com\\@evil.example/oauth20_desktop.srf?code=abc",
            // Another port.
            "https://login.live.com:8443/oauth20_desktop.srf?code=abc",
            // Other paths.
            "https://login.live.com/oauth20_authorize.srf?code=abc",
            "https://login.live.com/oauth20_desktop.srf/extra?code=abc",
            "https://login.live.com/oauth20_desktop.srfx?code=abc",
            "https://login.live.com/OAUTH20_DESKTOP.SRF?code=abc",
            "https://login.live.com/x/oauth20_desktop.srf?code=abc",
            "https://login.live.com/oauth20%5Fdesktop.srf?code=abc",
            "https://login.live.com/?next=/oauth20_desktop.srf&code=abc",
            // Other schemes.
            "javascript:alert(1)//login.live.com/oauth20_desktop.srf?code=abc",
            "file:///oauth20_desktop.srf?code=abc",
        ] {
            assert_eq!(parse(url), wrong_page, "{url}");
        }
        for not_a_url in [
            "",
            "   ",
            "code=abc",
            "login.live.com/oauth20_desktop.srf?code=abc",
        ] {
            assert_eq!(
                parse(not_a_url),
                Err(AuthError::InvalidLink(LinkProblem::NotAUrl)),
                "{not_a_url}"
            );
        }
    }

    #[test]
    fn needs_exactly_one_code() {
        let no_code = Err(AuthError::InvalidLink(LinkProblem::NoCode));
        for url in [
            "https://login.live.com/oauth20_desktop.srf",
            "https://login.live.com/oauth20_desktop.srf?lc=1033",
            "https://login.live.com/oauth20_desktop.srf?code=",
            "https://login.live.com/oauth20_desktop.srf?code=a&code=b",
            // A code in the fragment is not read.
            "https://login.live.com/oauth20_desktop.srf#code=abc",
        ] {
            assert_eq!(parse(url), no_code, "{url}");
        }
    }

    #[test]
    fn rejects_a_different_state() {
        for url in [
            "https://login.live.com/oauth20_desktop.srf?code=abc&state=other",
            "https://login.live.com/oauth20_desktop.srf?code=abc&state=",
            "https://login.live.com/oauth20_desktop.srf?code=abc&state=the-state&state=x",
        ] {
            assert_eq!(parse(url), Err(AuthError::StateMismatch), "{url}");
        }
    }

    #[test]
    fn surfaces_errors_in_the_address() {
        let err = parse(
            "https://login.live.com/oauth20_desktop.srf?error=invalid_scope&error_description=The%20provided%20value%20for%20the%20input%20parameter%20'scope'%20is%20not%20valid.",
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthError::LinkError {
                code: "invalid_scope".into(),
                description: Some(
                    "The provided value for the input parameter 'scope' is not valid.".into()
                ),
            }
        );
        assert!(err.message().contains("is not valid."), "{}", err.message());

        // An error wins over a code in the same address.
        assert_eq!(
            parse("https://login.live.com/oauth20_desktop.srf?code=abc&error=server_error"),
            Err(AuthError::LinkError {
                code: "server_error".into(),
                description: None,
            })
        );
        assert_eq!(
            parse(
                "https://login.live.com/oauth20_desktop.srf?error=access_denied&error_description=x"
            ),
            Err(AuthError::Declined)
        );

        // Descriptions are cleaned and shortened before they are shown.
        let long = "a".repeat(1000);
        let Err(AuthError::LinkError { description, .. }) = parse(&format!(
            "https://login.live.com/oauth20_desktop.srf?error=x&error_description=line%0Abreak{long}"
        )) else {
            panic!("expected a link error");
        };
        let description = description.unwrap();
        assert!(description.starts_with("line break"), "{description}");
        assert_eq!(description.chars().count(), MAX_DESCRIPTION + 1);
        assert!(description.ends_with('…'));
    }

    #[test]
    fn link_errors_never_show_the_code() {
        // Nothing the parser returns for a bad address contains the code.
        for url in [
            "https://evil.example/oauth20_desktop.srf?code=SECRET-CODE",
            "https://login.live.com/oauth20_desktop.srf?code=SECRET-CODE&state=other",
            "https://login.live.com/oauth20_desktop.srf?code=SECRET-CODE&error=e&error_description=d",
        ] {
            let err = parse(url).unwrap_err();
            assert!(!format!("{err} {err:?} {}", err.message()).contains("SECRET-CODE"));
        }
        let code = parse_redirect(
            "https://login.live.com/oauth20_desktop.srf?code=SECRET-CODE",
            STATE,
        )
        .unwrap();
        assert!(!format!("{code:?} {code}").contains("SECRET-CODE"));
    }
}
