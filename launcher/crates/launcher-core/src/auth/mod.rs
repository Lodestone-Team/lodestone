//! Microsoft account sign-in for Minecraft: Java Edition.
//!
//! The chain is: a Microsoft token (from the browser flow or the device code
//! flow) → an Xbox Live user token → an XSTS token for
//! `rp://api.minecraftservices.com/` → `login_with_xbox` (the Minecraft access
//! token) → `entitlements/mcstore` (ownership) → `minecraft/profile`.
//!
//! [`Authenticator`] runs it. Every URL comes from [`Endpoints`], so tests point the
//! whole chain at a loopback mock server.

mod error;
pub mod loopback;
pub mod microsoft;
pub mod minecraft;
pub mod pkce;
mod session;

use url::Url;

pub use error::{AuthError, Step, XboxError};
pub use microsoft::{DeviceCode, MsTokens};
pub use minecraft::Profile;
pub use session::{Authenticator, BrowserSignIn, MinecraftSession, SignedIn};

/// The Lodestone Launcher app registration in Microsoft Entra. It is a public
/// client ID, not a secret.
pub const CLIENT_ID: &str = "00f524c5-eac7-4ea6-bd06-a96380edeb91";

/// Replaces [`CLIENT_ID`], for development against another app registration.
pub const CLIENT_ID_ENV: &str = "LODESTONE_LAUNCHER_MSA_CLIENT_ID";

/// Personal Microsoft accounts only.
pub const TENANT: &str = "consumers";

/// Xbox Live sign-in, plus a refresh token.
pub const SCOPES: &str = "XboxLive.signin offline_access";

/// Where the user enters a device code. Shown and opened by the device code flow.
pub const DEVICE_LOGIN_URL: &str = "https://www.microsoft.com/link";

/// The base URLs of every service the sign-in talks to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoints {
    /// The Microsoft identity platform's OAuth 2.0 endpoints for [`TENANT`].
    pub login: Url,
    pub xbox_user: Url,
    pub xsts: Url,
    pub minecraft: Url,
    /// Skin textures. A profile's skin URL is only fetched if it is on this host.
    pub textures: Url,
}

impl Endpoints {
    pub fn production() -> Self {
        let url = |url: &str| Url::parse(url).expect("a valid built-in URL");
        Self {
            login: url(&format!(
                "https://login.microsoftonline.com/{TENANT}/oauth2/v2.0"
            )),
            xbox_user: url("https://user.auth.xboxlive.com"),
            xsts: url("https://xsts.auth.xboxlive.com"),
            minecraft: url("https://api.minecraftservices.com"),
            textures: url("https://textures.minecraft.net"),
        }
    }

    /// Every service under one base URL, at `/login`, `/xbl`, `/xsts`, `/mc` and
    /// `/textures`. For tests against a mock server.
    pub fn all_at(base: &Url) -> Self {
        let at = |path: &str| base.join(path).expect("a valid path");
        Self {
            login: at("login"),
            xbox_user: at("xbl"),
            xsts: at("xsts"),
            minecraft: at("mc"),
            textures: at("textures"),
        }
    }
}

/// The app registration and endpoints to sign in with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsaConfig {
    pub client_id: String,
    pub endpoints: Endpoints,
}

impl MsaConfig {
    /// The production endpoints, with [`CLIENT_ID`] or the [`CLIENT_ID_ENV`] override.
    pub fn from_env() -> Self {
        Self::with_override(std::env::var(CLIENT_ID_ENV).ok())
    }

    fn with_override(client_id: Option<String>) -> Self {
        let client_id = client_id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| CLIENT_ID.to_string());
        Self {
            client_id,
            endpoints: Endpoints::production(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_and_override() {
        assert_eq!(MsaConfig::with_override(None).client_id, CLIENT_ID);
        assert_eq!(
            MsaConfig::with_override(Some("  ".into())).client_id,
            CLIENT_ID
        );
        assert_eq!(
            MsaConfig::with_override(Some(" dev-id ".into())).client_id,
            "dev-id"
        );
    }

    #[test]
    fn production_endpoints() {
        let endpoints = Endpoints::production();
        assert_eq!(
            crate::http::join(&endpoints.login, "token"),
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/token"
        );
        assert_eq!(
            endpoints.textures.host_str(),
            Some("textures.minecraft.net")
        );
    }

    #[test]
    fn mock_endpoints_share_a_base() {
        let base = Url::parse("http://127.0.0.1:4000/").unwrap();
        let endpoints = Endpoints::all_at(&base);
        assert_eq!(
            crate::http::join(&endpoints.minecraft, "minecraft/profile"),
            "http://127.0.0.1:4000/mc/minecraft/profile"
        );
    }
}
