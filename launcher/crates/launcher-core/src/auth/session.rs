//! The whole sign-in, from a Microsoft token to a Minecraft session.

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use http_client::HttpClient;
use url::Url;

use super::{
    AuthError, MsaConfig,
    loopback::{CancelHandle, Loopback},
    microsoft::{self, DeviceCode, MsTokens, Sleep},
    minecraft::{self, Profile},
    pkce::{self, Pkce},
};
use crate::{Secret, SecretStore, accounts::Account};

/// Refresh a Minecraft token this long before it expires.
const EXPIRY_MARGIN: Duration = Duration::from_secs(5 * 60);

/// A signed-in Minecraft session. It is kept in memory only.
#[derive(Debug)]
pub struct MinecraftSession {
    pub access_token: Secret,
    pub expires_at: SystemTime,
    pub profile: Profile,
}

impl MinecraftSession {
    /// Whether the token has expired, or will within a few minutes.
    pub fn is_expired(&self, now: SystemTime) -> bool {
        now + EXPIRY_MARGIN >= self.expires_at
    }
}

/// An account and its session, after a sign-in or a refresh.
#[derive(Debug)]
pub struct SignedIn {
    pub account: Account,
    pub session: MinecraftSession,
}

/// A browser sign-in in progress: open [`BrowserSignIn::url`], then pass this to
/// [`Authenticator::complete_browser_sign_in`].
pub struct BrowserSignIn {
    url: Url,
    pkce: Pkce,
    loopback: Loopback,
}

impl BrowserSignIn {
    /// The authorization URL to open in the browser.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Stops waiting for the browser.
    pub fn cancel_handle(&self) -> CancelHandle {
        self.loopback.cancel_handle()
    }

    pub fn redirect_uri(&self) -> &str {
        self.loopback.redirect_uri()
    }
}

/// Signs accounts in and refreshes them.
#[derive(Clone)]
pub struct Authenticator {
    http: Arc<dyn HttpClient>,
    config: MsaConfig,
}

impl Authenticator {
    pub fn new(http: Arc<dyn HttpClient>, config: MsaConfig) -> Self {
        Self { http, config }
    }

    pub fn config(&self) -> &MsaConfig {
        &self.config
    }

    /// Starts the browser sign-in: listens on loopback and builds the URL to open.
    pub fn start_browser_sign_in(&self, timeout: Duration) -> Result<BrowserSignIn, AuthError> {
        let state = pkce::random_state();
        let pkce = Pkce::generate();
        let loopback = Loopback::start(state.clone(), timeout)?;
        let url = microsoft::authorize_url(&self.config, loopback.redirect_uri(), &pkce, &state);
        Ok(BrowserSignIn {
            url,
            pkce,
            loopback,
        })
    }

    /// Waits for the browser to come back, then exchanges the code for tokens.
    pub async fn complete_browser_sign_in(
        &self,
        sign_in: BrowserSignIn,
    ) -> Result<MsTokens, AuthError> {
        let BrowserSignIn { pkce, loopback, .. } = sign_in;
        let redirect_uri = loopback.redirect_uri().to_string();
        let code = loopback.wait().await?;
        microsoft::exchange_code(&*self.http, &self.config, &code, &redirect_uri, &pkce).await
    }

    pub async fn start_device_code(&self) -> Result<DeviceCode, AuthError> {
        microsoft::start_device_code(&*self.http, &self.config).await
    }

    pub async fn complete_device_code(
        &self,
        code: &DeviceCode,
        sleep: Sleep<'_>,
    ) -> Result<MsTokens, AuthError> {
        microsoft::poll_device_code(&*self.http, &self.config, code, sleep).await
    }

    /// Runs the Xbox Live, XSTS and Minecraft steps with a Microsoft access token.
    pub async fn minecraft_session(
        &self,
        ms_access_token: &Secret,
    ) -> Result<MinecraftSession, AuthError> {
        let http = &*self.http;
        let endpoints = &self.config.endpoints;
        let xbox = minecraft::xbox_live(http, endpoints, ms_access_token).await?;
        let xsts = minecraft::xsts(http, endpoints, &xbox).await?;
        let token = minecraft::login(http, endpoints, &xsts).await?;
        let owned = minecraft::owns_minecraft(http, endpoints, &token).await?;
        // Game Pass accounts can have no store entitlement but still a profile, so
        // the profile decides; the entitlement only tells the two failures apart.
        let profile = match minecraft::profile(http, endpoints, &token).await? {
            Some(profile) => profile,
            None if owned => return Err(AuthError::NoProfile),
            None => return Err(AuthError::NotOwned),
        };
        Ok(MinecraftSession {
            access_token: token.access_token,
            expires_at: SystemTime::now() + token.expires_in,
            profile,
        })
    }

    /// Finishes a sign-in: runs the chain and saves the refresh token in `secrets`.
    /// The caller adds the returned account to the [`AccountStore`](crate::accounts::AccountStore).
    pub async fn finish_sign_in(
        &self,
        tokens: MsTokens,
        secrets: &dyn SecretStore,
    ) -> Result<SignedIn, AuthError> {
        let session = self.minecraft_session(&tokens.access_token).await?;
        let account = account_for(&session.profile);
        let refresh_token = tokens.refresh_token.ok_or(AuthError::Microsoft {
            code: "no_refresh_token".into(),
        })?;
        let key = account.secret_key().expect("a Microsoft account");
        secrets
            .write(&key, &refresh_token)
            .await
            .map_err(|err| AuthError::Keychain(format!("{err:#}")))?;
        Ok(SignedIn { account, session })
    }

    /// Signs a saved account in again with its refresh token, without the browser.
    ///
    /// Microsoft rotates refresh tokens: the new one is saved before the rest of
    /// the chain runs, so it is kept even if a later step fails. If the refresh
    /// token is missing or refused, the error is one where
    /// [`AuthError::needs_sign_in`] is true.
    pub async fn refresh(
        &self,
        account: &Account,
        secrets: &dyn SecretStore,
    ) -> Result<SignedIn, AuthError> {
        let key = account.secret_key().ok_or(AuthError::NoSavedSignIn)?;
        let refresh_token = secrets
            .read(&key)
            .await
            .map_err(|err| AuthError::Keychain(format!("{err:#}")))?
            .ok_or(AuthError::NoSavedSignIn)?;
        let tokens = microsoft::refresh(&*self.http, &self.config, &refresh_token).await?;
        if let Some(rotated) = &tokens.refresh_token
            && *rotated != refresh_token
        {
            secrets
                .write(&key, rotated)
                .await
                .map_err(|err| AuthError::Keychain(format!("{err:#}")))?;
        }
        let session = self.minecraft_session(&tokens.access_token).await?;
        if session.profile.id != account.uuid {
            // The saved sign-in belongs to another Minecraft profile.
            return Err(AuthError::SignInExpired);
        }
        Ok(SignedIn {
            account: account_for(&session.profile),
            session,
        })
    }
}

fn account_for(profile: &Profile) -> Account {
    Account::microsoft(profile.id, profile.name.clone(), profile.skin_url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_expire_early() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let session = |expires_in: u64| MinecraftSession {
            access_token: Secret::new("t"),
            expires_at: now + Duration::from_secs(expires_in),
            profile: Profile {
                id: uuid::Uuid::nil(),
                name: "n".into(),
                skin_url: None,
            },
        };
        assert!(!session(3600).is_expired(now));
        assert!(session(60).is_expired(now));
        assert!(session(0).is_expired(now));
    }
}
