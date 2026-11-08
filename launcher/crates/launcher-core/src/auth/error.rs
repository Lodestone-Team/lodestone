//! Sign-in errors, with the messages the UI shows for them.
//!
//! No variant carries a token or a raw response body, so an error can be shown or
//! logged as it is.

use std::fmt;

/// A step of the sign-in chain, for errors that are not otherwise explained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    MicrosoftToken,
    DeviceCode,
    XboxLive,
    Xsts,
    MinecraftLogin,
    Entitlements,
    Profile,
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Step::MicrosoftToken => "Microsoft sign-in",
            Step::DeviceCode => "Microsoft device code sign-in",
            Step::XboxLive => "Xbox Live sign-in",
            Step::Xsts => "Xbox Live authorization",
            Step::MinecraftLogin => "Minecraft sign-in",
            Step::Entitlements => "the Minecraft ownership check",
            Step::Profile => "loading the Minecraft profile",
        })
    }
}

/// Why XSTS refused the account, from its `XErr` code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XboxError {
    /// 2148916233: the Microsoft account has no Xbox profile yet.
    NoXboxAccount,
    /// 2148916235: Xbox Live is not available in the account's country or region.
    Region,
    /// 2148916236 and 2148916237: the account needs adult verification (South Korea).
    AdultVerification,
    /// 2148916238: a child account that is not in a Microsoft family group.
    ChildAccount,
    /// Any other code.
    Other(u64),
}

impl XboxError {
    pub fn from_code(code: u64) -> Self {
        match code {
            2148916233 => Self::NoXboxAccount,
            2148916235 => Self::Region,
            2148916236 | 2148916237 => Self::AdultVerification,
            2148916238 => Self::ChildAccount,
            other => Self::Other(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("sign-in was cancelled")]
    Cancelled,
    #[error("the browser sign-in timed out")]
    TimedOut,
    #[error("the browser returned a sign-in for a different request")]
    StateMismatch,
    #[error("the sign-in was declined")]
    Declined,
    #[error("Microsoft refused the sign-in ({code})")]
    Microsoft { code: String },
    #[error("the device code expired")]
    DeviceCodeExpired,
    #[error("the saved sign-in has expired")]
    SignInExpired,
    #[error("there is no saved sign-in for this account")]
    NoSavedSignIn,
    #[error("Xbox Live refused the account ({0:?})")]
    Xbox(XboxError),
    #[error("Minecraft has not approved this app registration yet")]
    AppNotApproved,
    #[error("the account does not own Minecraft: Java Edition")]
    NotOwned,
    #[error("the account has no Minecraft profile")]
    NoProfile,
    #[error("too many requests")]
    RateLimited,
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response from {step} (HTTP {status})")]
    UnexpectedResponse { step: Step, status: u16 },
    #[error("could not start the sign-in listener: {0}")]
    Listener(String),
    #[error("could not use the keychain: {0}")]
    Keychain(String),
}

impl AuthError {
    /// A short heading for the error.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Cancelled => "Sign-in cancelled",
            Self::TimedOut => "Sign-in timed out",
            Self::StateMismatch => "Sign-in didn't match",
            Self::Declined => "Sign-in declined",
            Self::Microsoft { .. } => "Microsoft sign-in failed",
            Self::DeviceCodeExpired => "The code expired",
            Self::SignInExpired | Self::NoSavedSignIn => "Sign in again",
            Self::Xbox(XboxError::NoXboxAccount) => "No Xbox profile",
            Self::Xbox(XboxError::Region) => "Xbox Live isn't available",
            Self::Xbox(XboxError::AdultVerification) => "Adult verification needed",
            Self::Xbox(XboxError::ChildAccount) => "Child account",
            Self::Xbox(XboxError::Other(_)) => "Xbox Live sign-in failed",
            Self::AppNotApproved => "Not approved by Minecraft yet",
            Self::NotOwned => "Minecraft not found",
            Self::NoProfile => "No Minecraft profile",
            Self::RateLimited => "Too many attempts",
            Self::Network(_) => "Couldn't connect",
            Self::UnexpectedResponse { .. } => "Sign-in failed",
            Self::Listener(_) => "Couldn't start sign-in",
            Self::Keychain(_) => "Couldn't save the sign-in",
        }
    }

    /// What happened and what to do, in a sentence or two.
    pub fn message(&self) -> String {
        match self {
            Self::Cancelled => "Sign-in was cancelled.".into(),
            Self::TimedOut => {
                "The browser sign-in wasn't finished in time. Try again, or use a code instead."
                    .into()
            }
            Self::StateMismatch => {
                "The browser came back with a different sign-in request, so Lodestone Launcher ignored it. Try again."
                    .into()
            }
            Self::Declined => {
                "The sign-in was declined in the browser. Try again and accept the permissions to continue."
                    .into()
            }
            Self::Microsoft { code } => {
                format!("Microsoft couldn't sign you in ({code}). Try again in a moment.")
            }
            Self::DeviceCodeExpired => "The code expired before sign-in finished. Get a new code and try again.".into(),
            Self::SignInExpired | Self::NoSavedSignIn => {
                "Your saved Microsoft sign-in has expired or was revoked. Sign in again to keep using this account."
                    .into()
            }
            Self::Xbox(XboxError::NoXboxAccount) => {
                "This Microsoft account doesn't have an Xbox profile yet. Sign in once at xbox.com to create one, then try again."
                    .into()
            }
            Self::Xbox(XboxError::Region) => {
                "Xbox Live isn't available in this account's country or region, so it can't sign in to Minecraft."
                    .into()
            }
            Self::Xbox(XboxError::AdultVerification) => {
                "This account needs adult verification on the Xbox website before it can play. This is required in South Korea."
                    .into()
            }
            Self::Xbox(XboxError::ChildAccount) => {
                "This is a child account. An adult needs to add it to a Microsoft family group before it can sign in."
                    .into()
            }
            Self::Xbox(XboxError::Other(code)) => {
                format!("Xbox Live refused the sign-in (error {code}). Try again later.")
            }
            Self::AppNotApproved => {
                "Microsoft sign-in works, but Minecraft hasn't approved Lodestone Launcher yet. Use offline mode for now."
                    .into()
            }
            Self::NotOwned => {
                "This Microsoft account doesn't own Minecraft: Java Edition. If you play through Xbox Game Pass, start Minecraft once in the official launcher, then try again."
                    .into()
            }
            Self::NoProfile => {
                "This account has no Minecraft profile yet. If you play through Xbox Game Pass, start Minecraft once in the official launcher to pick a name, then try again."
                    .into()
            }
            Self::RateLimited => "Too many sign-in attempts. Wait a few minutes and try again.".into(),
            Self::Network(_) => "Lodestone Launcher couldn't reach the sign-in servers. Check your internet connection and try again.".into(),
            Self::UnexpectedResponse { step, status } => {
                format!("Something went wrong during {step} (HTTP {status}). Try again in a moment.")
            }
            Self::Listener(_) => "Lodestone Launcher couldn't open the local page the browser returns to. Use a code instead.".into(),
            Self::Keychain(_) => "Lodestone Launcher couldn't save the sign-in to your keychain. Check that the keychain is unlocked and try again.".into(),
        }
    }

    /// The account's saved sign-in is no longer usable, and the user has to sign
    /// in again.
    pub fn needs_sign_in(&self) -> bool {
        matches!(self, Self::SignInExpired | Self::NoSavedSignIn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_xerr_codes() {
        assert_eq!(XboxError::from_code(2148916233), XboxError::NoXboxAccount);
        assert_eq!(XboxError::from_code(2148916235), XboxError::Region);
        assert_eq!(
            XboxError::from_code(2148916236),
            XboxError::AdultVerification
        );
        assert_eq!(
            XboxError::from_code(2148916237),
            XboxError::AdultVerification
        );
        assert_eq!(XboxError::from_code(2148916238), XboxError::ChildAccount);
        assert_eq!(XboxError::from_code(1), XboxError::Other(1));
    }

    #[test]
    fn the_not_approved_message_says_what_to_do() {
        let message = AuthError::AppNotApproved.message();
        assert!(message.contains("Microsoft sign-in works"), "{message}");
        assert!(
            message.contains("hasn't approved Lodestone Launcher"),
            "{message}"
        );
        assert!(message.contains("offline mode"), "{message}");
        assert!(!AuthError::AppNotApproved.needs_sign_in());
        assert!(AuthError::SignInExpired.needs_sign_in());
    }
}
