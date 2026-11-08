//! PKCE (RFC 7636) with the S256 method, and the OAuth `state` value.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest as _, Sha256};

use crate::Secret;

/// A PKCE code verifier and its S256 challenge.
#[derive(Debug)]
pub struct Pkce {
    /// Sent only with the token request.
    pub verifier: Secret,
    /// Sent with the authorization request.
    pub challenge: String,
}

impl Pkce {
    /// A fresh verifier: 32 random bytes, base64url-encoded to 43 characters.
    pub fn generate() -> Self {
        Self::from_verifier(URL_SAFE_NO_PAD.encode(crate::random_bytes::<32>()))
    }

    pub fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier: Secret::new(verifier),
            challenge,
        }
    }
}

/// A random `state` value for an authorization request.
pub fn random_state() -> String {
    URL_SAFE_NO_PAD.encode(crate::random_bytes::<32>())
}

/// Compares two strings in time that depends only on their lengths.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_7636_example() {
        // RFC 7636, Appendix B.
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_verifiers_are_valid_and_unique() {
        let a = Pkce::generate();
        let b = Pkce::generate();
        assert_ne!(a.verifier, b.verifier);
        for pkce in [&a, &b] {
            let verifier = pkce.verifier.expose();
            // RFC 7636 section 4.1: 43 to 128 unreserved characters.
            assert_eq!(verifier.len(), 43);
            assert!(
                verifier
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c))
            );
            assert_eq!(
                pkce.challenge,
                Pkce::from_verifier(verifier.to_string()).challenge
            );
            assert_eq!(pkce.challenge.len(), 43);
        }
        assert!(!format!("{a:?}").contains(a.verifier.expose()));
    }

    #[test]
    fn states_are_random() {
        let a = random_state();
        assert_eq!(a.len(), 43);
        assert_ne!(a, random_state());
    }

    #[test]
    fn compares_states() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(constant_time_eq("", ""));
    }
}
