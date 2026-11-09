//! Sign-in errors as the launcher shows them, in the current language: the same
//! titles and messages as `launcher-core`'s [`AuthError::title`] and
//! [`AuthError::message`], which stay English for logs.

use launcher_core::auth::{AuthError, LinkProblem, Step, XboxError};

use crate::i18n::{CommonMsg, ShellMsg as M, t, tr};

/// The start of the address the sign-in-by-link flow accepts.
pub const LINK_PAGE: &str = "https://login.live.com/oauth20_desktop.srf";

/// A short heading for the error.
pub fn title(err: &AuthError) -> &'static str {
    t(match err {
        AuthError::Cancelled => M::AuthCancelled,
        AuthError::TimedOut => M::AuthTimedOut,
        AuthError::StateMismatch => M::AuthStateMismatch,
        AuthError::Declined => M::AuthDeclined,
        AuthError::Microsoft { .. } | AuthError::LinkError { .. } => M::AuthMicrosoft,
        AuthError::DeviceCodeExpired => M::AuthCodeExpired,
        AuthError::SignInExpired | AuthError::NoSavedSignIn => M::SignInAgain,
        AuthError::Xbox(XboxError::NoXboxAccount) => M::AuthNoXbox,
        AuthError::Xbox(XboxError::Region) => M::AuthRegion,
        AuthError::Xbox(XboxError::AdultVerification) => M::AuthAdult,
        AuthError::Xbox(XboxError::ChildAccount) => M::AuthChild,
        AuthError::Xbox(XboxError::Other(_)) => M::AuthXbox,
        AuthError::AppNotApproved => M::AuthNotApproved,
        AuthError::NotOwned => M::AuthNotOwned,
        AuthError::NoProfile => M::AuthNoProfile,
        AuthError::RateLimited => M::AuthRateLimited,
        AuthError::Network(_) => M::AuthNetwork,
        AuthError::UnexpectedResponse { .. } => M::AuthUnexpected,
        AuthError::Listener(_) => M::AuthListener,
        AuthError::Keychain(_) => M::AuthKeychain,
        AuthError::InvalidLink(_) => M::AuthInvalidLink,
    })
}

/// What happened and what to do, in a sentence or two.
pub fn message(err: &AuthError) -> String {
    let plain = |message: M| t(message).to_string();
    match err {
        AuthError::Cancelled => plain(M::AuthCancelledBody),
        AuthError::TimedOut => plain(M::AuthTimedOutBody),
        AuthError::StateMismatch => plain(M::AuthStateMismatchBody),
        AuthError::Declined => plain(M::AuthDeclinedBody),
        AuthError::Microsoft { code }
        | AuthError::LinkError {
            code,
            description: None,
        } => tr!(M::AuthMicrosoftBody, code = code),
        AuthError::LinkError {
            code,
            description: Some(description),
        } => tr!(M::AuthMicrosoftSaid, description = description, code = code),
        AuthError::DeviceCodeExpired => plain(M::AuthCodeExpiredBody),
        AuthError::SignInExpired | AuthError::NoSavedSignIn => plain(M::AuthSignInExpiredBody),
        AuthError::Xbox(XboxError::NoXboxAccount) => plain(M::AuthNoXboxBody),
        AuthError::Xbox(XboxError::Region) => plain(M::AuthRegionBody),
        AuthError::Xbox(XboxError::AdultVerification) => plain(M::AuthAdultBody),
        AuthError::Xbox(XboxError::ChildAccount) => plain(M::AuthChildBody),
        AuthError::Xbox(XboxError::Other(code)) => tr!(M::AuthXboxBody, code = code),
        AuthError::AppNotApproved => plain(M::AuthNotApprovedBody),
        AuthError::NotOwned => plain(M::AuthNotOwnedBody),
        AuthError::NoProfile => plain(M::AuthNoProfileBody),
        AuthError::RateLimited => plain(M::AuthRateLimitedBody),
        AuthError::Network(_) => plain(M::AuthNetworkBody),
        AuthError::UnexpectedResponse { step, status } => tr!(
            M::AuthUnexpectedBody,
            step = step_name(*step),
            status = status
        ),
        AuthError::Listener(_) => plain(M::AuthListenerBody),
        AuthError::Keychain(_) => tr!(M::AuthKeychainBody, keychain = t(CommonMsg::Keychain)),
        AuthError::InvalidLink(LinkProblem::NotAUrl) => plain(M::AuthNotAUrlBody),
        AuthError::InvalidLink(LinkProblem::WrongPage) => {
            tr!(M::AuthWrongPageBody, url = LINK_PAGE)
        }
        AuthError::InvalidLink(LinkProblem::NoCode) => plain(M::AuthNoCodeBody),
    }
}

/// A step of signing in, in a sentence: "during {step}".
fn step_name(step: Step) -> &'static str {
    t(match step {
        Step::MicrosoftToken => M::StepMicrosoftToken,
        Step::DeviceCode => M::StepDeviceCode,
        Step::XboxLive => M::StepXboxLive,
        Step::Xsts => M::StepXsts,
        Step::MinecraftLogin => M::StepMinecraftLogin,
        Step::Entitlements => M::StepEntitlements,
        Step::Profile => M::StepProfile,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In English the launcher says what `launcher-core` says, apart from the
    /// keychain's wording.
    #[test]
    fn english_matches_the_core() {
        let errors = [
            AuthError::Cancelled,
            AuthError::TimedOut,
            AuthError::StateMismatch,
            AuthError::Declined,
            AuthError::Microsoft { code: "x".into() },
            AuthError::DeviceCodeExpired,
            AuthError::SignInExpired,
            AuthError::NoSavedSignIn,
            AuthError::Xbox(XboxError::NoXboxAccount),
            AuthError::Xbox(XboxError::Region),
            AuthError::Xbox(XboxError::AdultVerification),
            AuthError::Xbox(XboxError::ChildAccount),
            AuthError::Xbox(XboxError::Other(7)),
            AuthError::AppNotApproved,
            AuthError::NotOwned,
            AuthError::NoProfile,
            AuthError::RateLimited,
            AuthError::Network("down".into()),
            AuthError::UnexpectedResponse {
                step: Step::Xsts,
                status: 500,
            },
            AuthError::Listener("busy".into()),
            AuthError::InvalidLink(LinkProblem::NotAUrl),
            AuthError::InvalidLink(LinkProblem::WrongPage),
            AuthError::InvalidLink(LinkProblem::NoCode),
            AuthError::LinkError {
                code: "access_denied".into(),
                description: Some("The user said no".into()),
            },
            AuthError::LinkError {
                code: "access_denied".into(),
                description: None,
            },
        ];
        for err in errors {
            assert_eq!(title(&err), err.title(), "{err:?}");
            assert_eq!(message(&err), err.message(), "{err:?}");
        }
        assert_eq!(
            title(&AuthError::Keychain("locked".into())),
            AuthError::Keychain("locked".into()).title()
        );
    }
}
