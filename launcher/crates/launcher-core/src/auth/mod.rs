//! Microsoft account sign-in for Minecraft: Java Edition.
//!
//! The chain is: a Microsoft token (from the browser flow or the device code
//! flow) → an Xbox Live user token → an XSTS token for
//! `rp://api.minecraftservices.com/` → `login_with_xbox` (the Minecraft access
//! token) → `entitlements/mcstore` (ownership) → `minecraft/profile`.
//!
//! [`Authenticator`] runs it. Every URL comes from [`Endpoints`], so tests point the
//! whole chain at a loopback mock server.
//!
//! The Microsoft token comes from one of two app identities ([`ClientIdentity`]):
//! Lodestone's own Azure app, or, as a developer option, the official Minecraft
//! Launcher's public client on login.live.com (see [`live`]).

mod error;
pub mod live;
pub mod loopback;
pub mod microsoft;
pub mod minecraft;
pub mod pkce;
mod session;

use serde::{Deserialize, Serialize};
use url::Url;

pub use error::{AuthError, LinkProblem, Step, XboxError};
pub use live::LinkSignIn;
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

/// The official Minecraft Launcher's public client ID on login.live.com (the Java
/// Edition "title ID"). Only used when the developer option is on.
pub const MINECRAFT_LAUNCHER_CLIENT_ID: &str = "00000000402b5328";

/// The scope the Minecraft Launcher's client asks for: an Xbox Live user ticket.
pub const MINECRAFT_LAUNCHER_SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

/// Which app the launcher says it is when it asks Microsoft for a token.
///
/// A refresh token only works with the client that issued it, so every Microsoft
/// account records the identity it was signed in with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientIdentity {
    /// Lodestone Launcher's own Azure app registration, [`CLIENT_ID`].
    #[default]
    Lodestone,
    /// The official Minecraft Launcher's client, [`MINECRAFT_LAUNCHER_CLIENT_ID`].
    /// A developer option until Mojang approves Lodestone's app.
    MinecraftLauncher,
}

impl ClientIdentity {
    pub fn is_lodestone(&self) -> bool {
        *self == Self::Lodestone
    }

    /// The prefix of the Microsoft token in the Xbox Live `RpsTicket`: `d=` for a
    /// token from an Azure app, `t=` for one from a title's live.com client.
    pub fn rps_ticket_prefix(self) -> &'static str {
        match self {
            Self::Lodestone => "d=",
            Self::MinecraftLauncher => "t=",
        }
    }
}

/// The base URLs of every service the sign-in talks to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoints {
    /// The Microsoft identity platform's OAuth 2.0 endpoints for [`TENANT`].
    pub login: Url,
    /// login.live.com, for the [`ClientIdentity::MinecraftLauncher`] client.
    pub live: Url,
    pub xbox_user: Url,
    pub xsts: Url,
    pub minecraft: Url,
    /// Skin textures. A profile's skin URL is only fetched if it is on this host.
    pub textures: Url,
    /// Mojang's public session server, for another player's profile (a friend's
    /// skin), by UUID and without signing in.
    pub sessions: Url,
}

impl Endpoints {
    pub fn production() -> Self {
        let url = |url: &str| Url::parse(url).expect("a valid built-in URL");
        Self {
            login: url(&format!(
                "https://login.microsoftonline.com/{TENANT}/oauth2/v2.0"
            )),
            live: url(live::LIVE_BASE),
            xbox_user: url("https://user.auth.xboxlive.com"),
            xsts: url("https://xsts.auth.xboxlive.com"),
            minecraft: url("https://api.minecraftservices.com"),
            textures: url("https://textures.minecraft.net"),
            sessions: url("https://sessionserver.mojang.com"),
        }
    }

    /// Every service under one base URL, at `/login`, `/live`, `/xbl`, `/xsts`, `/mc`,
    /// `/textures` and `/sessions`. For tests against a mock server.
    pub fn all_at(base: &Url) -> Self {
        let at = |path: &str| base.join(path).expect("a valid path");
        Self {
            login: at("login"),
            live: at("live"),
            xbox_user: at("xbl"),
            xsts: at("xsts"),
            minecraft: at("mc"),
            textures: at("textures"),
            sessions: at("sessions"),
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
        assert_eq!(
            crate::http::join(&endpoints.live, "oauth20_token.srf"),
            "https://login.live.com/oauth20_token.srf"
        );
    }

    #[test]
    fn identities_and_ticket_prefixes() {
        assert_eq!(ClientIdentity::default(), ClientIdentity::Lodestone);
        assert_eq!(ClientIdentity::Lodestone.rps_ticket_prefix(), "d=");
        assert_eq!(ClientIdentity::MinecraftLauncher.rps_ticket_prefix(), "t=");
        assert_eq!(
            serde_json::to_value(ClientIdentity::MinecraftLauncher).unwrap(),
            "minecraft_launcher"
        );
        assert_eq!(
            serde_json::to_value(ClientIdentity::Lodestone).unwrap(),
            "lodestone"
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
