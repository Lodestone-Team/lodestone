//! Xbox Live, XSTS and the Minecraft services.

use std::time::Duration;

use http_client::HttpClient;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{AuthError, Endpoints, Step, XboxError, microsoft::network_error};
use crate::{
    Secret,
    http::{self, Response},
};

/// An Xbox Live or XSTS token and the user hash that goes with it.
#[derive(Debug)]
pub struct XboxToken {
    pub token: Secret,
    pub user_hash: String,
}

/// A Minecraft access token, valid for about a day.
#[derive(Debug)]
pub struct MinecraftToken {
    pub access_token: Secret,
    pub expires_in: Duration,
}

/// A Minecraft: Java Edition profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    /// The active skin's texture URL.
    pub skin_url: Option<String>,
}

fn unexpected(step: Step, response: &Response) -> AuthError {
    if response.status == 429 {
        AuthError::RateLimited
    } else {
        AuthError::UnexpectedResponse {
            step,
            status: response.status,
        }
    }
}

fn request_error(err: anyhow::Error) -> AuthError {
    AuthError::Network(err.to_string())
}

/// Not `Debug`, as it holds a token.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<UserInfo>,
}

#[derive(Deserialize)]
struct UserInfo {
    uhs: String,
}

fn xbox_token(step: Step, response: &Response) -> Result<XboxToken, AuthError> {
    let body: XboxResponse = response.json().map_err(|_| unexpected(step, response))?;
    let user_hash = body
        .display_claims
        .xui
        .into_iter()
        .next()
        .map(|info| info.uhs)
        .ok_or_else(|| unexpected(step, response))?;
    Ok(XboxToken {
        token: Secret::new(body.token),
        user_hash,
    })
}

/// Signs in to Xbox Live with a Microsoft access token.
pub async fn xbox_live(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    ms_access_token: &Secret,
) -> Result<XboxToken, AuthError> {
    let request = http::json(
        &http::join(&endpoints.xbox_user, "user/authenticate"),
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                // `d=` marks a token from an app's own Azure registration.
                "RpsTicket": format!("d={}", ms_access_token.expose()),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
    )
    .map_err(request_error)?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if !response.is_success() {
        return Err(unexpected(Step::XboxLive, &response));
    }
    xbox_token(Step::XboxLive, &response)
}

/// Gets an XSTS token for the Minecraft services, mapping `XErr` codes.
pub async fn xsts(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    xbox: &XboxToken,
) -> Result<XboxToken, AuthError> {
    let request = http::json(
        &http::join(&endpoints.xsts, "xsts/authorize"),
        &json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox.token.expose()],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
    )
    .map_err(request_error)?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if response.is_success() {
        return xbox_token(Step::Xsts, &response);
    }
    let xerr = response.json::<Value>().ok().and_then(|body| {
        let xerr = body.get("XErr")?;
        xerr.as_u64()
            .or_else(|| xerr.as_str().and_then(|code| code.parse().ok()))
    });
    Err(match xerr {
        Some(code) => AuthError::Xbox(XboxError::from_code(code)),
        None => unexpected(Step::Xsts, &response),
    })
}

/// Not `Debug`, as it holds a token.
#[derive(Deserialize)]
struct LoginResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Signs in to the Minecraft services with an XSTS token.
///
/// Until Mojang approves an app registration, this answers 403 with "Invalid app
/// registration", which is [`AuthError::AppNotApproved`].
pub async fn login(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    xsts: &XboxToken,
) -> Result<MinecraftToken, AuthError> {
    let request = http::json(
        &http::join(&endpoints.minecraft, "authentication/login_with_xbox"),
        &json!({
            "identityToken": format!("XBL3.0 x={};{}", xsts.user_hash, xsts.token.expose()),
        }),
    )
    .map_err(request_error)?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if response.status == 403 && is_invalid_app_registration(&response) {
        return Err(AuthError::AppNotApproved);
    }
    if !response.is_success() {
        return Err(unexpected(Step::MinecraftLogin, &response));
    }
    let body: LoginResponse = response
        .json()
        .map_err(|_| unexpected(Step::MinecraftLogin, &response))?;
    Ok(MinecraftToken {
        access_token: Secret::new(body.access_token),
        expires_in: Duration::from_secs(body.expires_in.unwrap_or(86_400)),
    })
}

/// The Xbox user id (`xuid`) in a Minecraft access token, which is a JWT. The game
/// gets it as `--xuid`. The token's signature is not checked: it came from Mojang
/// over TLS, and the value is only passed back to the game.
pub fn xuid_from_token(token: &Secret) -> Option<String> {
    use base64::Engine as _;
    let payload = token.expose().split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    let xuid = claims.get("xuid")?;
    xuid.as_str()
        .map(str::to_string)
        .or_else(|| xuid.as_u64().map(|n| n.to_string()))
        .filter(|xuid| !xuid.is_empty() && xuid.bytes().all(|b| b.is_ascii_digit()))
}

fn is_invalid_app_registration(response: &Response) -> bool {
    response.json::<Value>().ok().is_some_and(|body| {
        ["errorMessage", "error", "message"].iter().any(|key| {
            body.get(key).and_then(Value::as_str).is_some_and(|text| {
                text.to_ascii_lowercase()
                    .contains("invalid app registration")
            })
        })
    })
}

#[derive(Deserialize)]
struct Entitlements {
    #[serde(default)]
    items: Vec<Entitlement>,
}

#[derive(Deserialize)]
struct Entitlement {
    name: String,
}

/// Whether the account has a Minecraft: Java Edition entitlement.
pub async fn owns_minecraft(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &MinecraftToken,
) -> Result<bool, AuthError> {
    let request = http::get(
        &http::join(&endpoints.minecraft, "entitlements/mcstore"),
        Some(&token.access_token),
    )
    .map_err(request_error)?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if !response.is_success() {
        return Err(unexpected(Step::Entitlements, &response));
    }
    let body: Entitlements = response
        .json()
        .map_err(|_| unexpected(Step::Entitlements, &response))?;
    Ok(body
        .items
        .iter()
        .any(|item| matches!(item.name.as_str(), "product_minecraft" | "game_minecraft")))
}

#[derive(Deserialize)]
struct ProfileResponse {
    id: String,
    name: String,
    #[serde(default)]
    skins: Vec<Skin>,
}

#[derive(Deserialize)]
struct Skin {
    #[serde(default)]
    state: String,
    url: String,
}

/// The account's profile, or `None` if it has none yet (`404 NOT_FOUND`), as with
/// a Game Pass account that has never started the official launcher.
pub async fn profile(
    client: &dyn HttpClient,
    endpoints: &Endpoints,
    token: &MinecraftToken,
) -> Result<Option<Profile>, AuthError> {
    let request = http::get(
        &http::join(&endpoints.minecraft, "minecraft/profile"),
        Some(&token.access_token),
    )
    .map_err(request_error)?;
    let response = http::send(client, request).await.map_err(network_error)?;
    if response.status == 404 {
        return Ok(None);
    }
    if !response.is_success() {
        return Err(unexpected(Step::Profile, &response));
    }
    let body: ProfileResponse = response
        .json()
        .map_err(|_| unexpected(Step::Profile, &response))?;
    let id = Uuid::try_parse(&body.id).map_err(|_| unexpected(Step::Profile, &response))?;
    let skin_url = body
        .skins
        .iter()
        .find(|skin| skin.state == "ACTIVE")
        .or(body.skins.first())
        .map(|skin| skin.url.clone());
    Ok(Some(Profile {
        id,
        name: body.name,
        skin_url,
    }))
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;

    use super::*;

    #[test]
    fn reads_the_xuid_from_the_token() {
        let encode = |json: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
        let token = format!(
            "{}.{}.signature",
            encode(r#"{"alg":"HS256"}"#),
            encode(r#"{"xuid":"2535412345678901","pfd":[]}"#)
        );
        assert_eq!(
            xuid_from_token(&Secret::new(token)).as_deref(),
            Some("2535412345678901")
        );
        let numeric = format!("h.{}.s", encode(r#"{"xuid":2535412345678901}"#));
        assert_eq!(
            xuid_from_token(&Secret::new(numeric)).as_deref(),
            Some("2535412345678901")
        );
        for bad in [
            "not-a-jwt",
            "a.!!!.c",
            &format!("a.{}.c", encode(r#"{"xuid":"x; rm"}"#)),
        ] {
            assert_eq!(xuid_from_token(&Secret::new(bad)), None, "{bad}");
        }
    }
}
