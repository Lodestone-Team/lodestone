//! Signing in to the Host's own core as its owner.
//!
//! The Tauri Host ran the core in its own process and asked it for an owner token
//! directly (`tauri_export::get_owner_jwt`). This Host runs the core as a child, so it
//! uses the core's local claim instead (`core/src/local_claim.rs`): it proves it can
//! write the core's data folder, which only this OS user can, and the core writes an
//! owner sign-in token into that folder ([`local_claim::claim_session`]). The token is
//! tied to the core's HTTPS certificate and only ever sent to it. Nothing is saved:
//! the Host claims again each time it starts.
//!
//! - A core without an owner yet (first run): the claim makes the owner without a
//!   password, named after this computer's user ([`default_owner_name`]), so the first
//!   run asks nothing. The core then serves only this computer. When the owner wants to
//!   use it from other devices, the Host asks for a username and password first
//!   (`pages::account_dialogs`), which lets other devices in.
//! - A core whose owner turned off "Let apps on this computer claim owner access": the
//!   claim is refused and the user signs in with a username and password, as on the
//!   dashboard's sign-in page ([`sign_in_with_password`]).

use std::{path::Path, sync::Arc};

use http_client::HttpClient;
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        admin::{AdminClient, FullCoreInfo, PublicUser},
        local_claim::{self, ClaimError, OwnerCredentials},
    },
    game::download::Sleep,
};

/// The name the core logs the claim under.
pub const CLAIM_NAME: &str = "Lodestone Host";

/// Signed in to a core.
#[derive(Clone, Debug)]
pub struct Session {
    pub admin: AdminClient,
    pub info: FullCoreInfo,
    pub me: PublicUser,
}

/// Why signing in didn't work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInError {
    /// The core refused the claim (its owner turned it off): sign in with a password.
    Refused,
    /// Anything else, for people.
    Failed(String),
}

impl From<ClaimError> for SignInError {
    fn from(err: ClaimError) -> Self {
        match err {
            ClaimError::Refused => SignInError::Refused,
            err => SignInError::Failed(err.to_string()),
        }
    }
}

impl From<CoreError> for SignInError {
    fn from(err: CoreError) -> Self {
        SignInError::Failed(err.to_string())
    }
}

/// Signs in to the core at `local_url` (its plain-HTTP address on this computer),
/// whose data folder is `data_dir`, as its owner. A core without an owner gets `owner`,
/// or else (the Host's first run) one without a password, named [`default_owner_name`].
pub async fn claim_owner(
    http: Arc<dyn HttpClient>,
    local_url: url::Url,
    data_dir: &Path,
    owner: Option<OwnerCredentials>,
    sleep: Sleep,
) -> Result<Session, SignInError> {
    let local = CoreClient::new(http, local_url);
    let info = local.info().await?;
    let owner = match owner {
        _ if info.is_setup => None,
        Some(owner) => Some(owner),
        None => Some(OwnerCredentials::without_password(default_owner_name())),
    };
    let session =
        local_claim::claim_session(&local, data_dir, owner.as_ref(), CLAIM_NAME, sleep).await?;
    let admin = AdminClient::new(session.client()?, session.token.clone());
    finish(admin).await
}

/// Signs in with a username and password (`POST /user/login`), over the core's
/// plain-HTTP address on this computer.
pub async fn sign_in_with_password(
    http: Arc<dyn HttpClient>,
    local_url: url::Url,
    username: &str,
    password: &Secret,
) -> Result<Session, SignInError> {
    let local = CoreClient::new(http, local_url);
    let reply = launcher_core::cores::admin::login(&local, username, password).await?;
    finish(AdminClient::new(local, Secret::new(reply.token))).await
}

/// Signs in to another core (`client`, with the certificate the user trusted) with a
/// username and password (`POST /user/login`), for Settings' other cores.
pub async fn sign_in_to(
    client: CoreClient,
    username: &str,
    password: &Secret,
) -> Result<Session, SignInError> {
    let reply = launcher_core::cores::admin::login(&client, username, password).await?;
    finish(AdminClient::new(client, Secret::new(reply.token))).await
}

/// What the shell needs once signed in: the core's details and who is signed in.
async fn finish(admin: AdminClient) -> Result<Session, SignInError> {
    let info = admin.full_info().await?;
    let me = admin.me().await?;
    Ok(Session { admin, info, me })
}

/// When a sign-in token (a JWT) expires, in Unix seconds: its `exp` claim, read without
/// checking the signature (the core checks it). `None` for anything else, such as an
/// API key.
pub fn token_expiry(token: &str) -> Option<i64> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("exp")?.as_i64()
}

/// The owner's username on the first run: this computer's user name (`USER`, or
/// `USERNAME` on Windows), cleaned up, else "owner". It can be changed in Settings ›
/// Profile.
pub fn default_owner_name() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok();
    owner_name_from(user.as_deref())
}

/// `user` as a username: letters, digits, `-`, `_` and `.` (spaces become `_`), at most
/// 32 characters; "owner" when nothing is left.
pub fn owner_name_from(user: Option<&str>) -> String {
    let name: String = user
        .unwrap_or_default()
        .trim()
        .chars()
        .filter_map(|c| match c {
            c if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') => Some(c),
            ' ' => Some('_'),
            _ => None,
        })
        .take(32)
        .collect();
    let name = name.trim_matches(|c| matches!(c, '_' | '.' | '-'));
    if name.is_empty() {
        "owner".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_expiry_is_read_from_a_jwt() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let part = |json: &str| URL_SAFE_NO_PAD.encode(json);
        let jwt = format!(
            "{}.{}.signature",
            part(r#"{"alg":"HS512"}"#),
            part(r#"{"uid":"abc","exp":1794244636}"#)
        );
        assert_eq!(token_expiry(&jwt), Some(1794244636));
        assert_eq!(token_expiry("lsk_abc_def"), None);
        assert_eq!(token_expiry("a.not base64.c"), None);
        let no_exp = format!("{}.{}.s", part("{}"), part(r#"{"uid":"abc"}"#));
        assert_eq!(token_expiry(&no_exp), None);
    }

    #[test]
    fn the_first_owner_is_named_after_this_computers_user() {
        assert_eq!(owner_name_from(Some("peter")), "peter");
        assert_eq!(owner_name_from(Some(" Peter Jiang ")), "Peter_Jiang");
        assert_eq!(owner_name_from(Some("DOMAIN\\alex")), "DOMAINalex");
        assert_eq!(owner_name_from(Some("élodie.m")), "élodie.m");
        assert_eq!(owner_name_from(Some("$#@")), "owner");
        assert_eq!(owner_name_from(Some("")), "owner");
        assert_eq!(owner_name_from(None), "owner");
        assert_eq!(owner_name_from(Some(&"a".repeat(80))).len(), 32);
        assert!(!default_owner_name().is_empty());
    }
}
