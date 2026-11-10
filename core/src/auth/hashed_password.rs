use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct HashedPassword(String);

impl HashedPassword {
    /// No password: matches nothing. Lodestone Host's owner has none until they set one
    /// to share the core (see `crate::remote_access`); until then they sign in only
    /// through the local claim.
    pub fn none() -> Self {
        Self(String::new())
    }

    /// Whether there is a password (an empty hash is none).
    pub fn is_set(&self) -> bool {
        !self.0.is_empty()
    }
}

impl PartialEq<str> for HashedPassword {
    fn eq(&self, other: &str) -> bool {
        // a malformed stored hash matches no password
        PasswordHash::new(&self.0).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(other.as_bytes(), &hash)
                .is_ok()
        })
    }
}

impl ToString for HashedPassword {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl From<HashedPassword> for String {
    fn from(hashed_password: HashedPassword) -> Self {
        hashed_password.to_string()
    }
}

impl AsRef<str> for HashedPassword {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub fn hash_password(password: impl AsRef<str>) -> HashedPassword {
    HashedPassword(
        // generates a random 16-byte salt from the OS RNG, like the old `SaltString::generate`
        Argon2::default()
            .hash_password(password.as_ref().as_bytes())
            .unwrap()
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hash in the format the old argon2 0.4 `Argon2::default()` + `SaltString::generate`
    /// produced (Argon2id v19, m=4096 KiB, t=3, p=1, 16-byte salt, 32-byte output), for the
    /// password "correct horse battery staple". Generated with an independent RFC 9106
    /// implementation that reproduces the RFC's Argon2id test vector.
    const ARGON2_0_4_HASH: &str = "$argon2id$v=19$m=4096,t=3,p=1$tR1UtONgqTiSCvurjnU8IQ$ESHbhSjb/lTyT1o9eMQ939wNN1rz7WWHA9Z3duTY4Cg";

    #[test]
    fn verifies_hashes_from_argon2_0_4() {
        let hashed = HashedPassword(ARGON2_0_4_HASH.to_string());
        assert!(hashed == *"correct horse battery staple");
        assert!(hashed != *"correct horse battery stapler");
        // the same path `UsersManager` uses for login and password changes
        assert!(Argon2::default()
            .verify_password(
                b"correct horse battery staple",
                &PasswordHash::new(ARGON2_0_4_HASH).unwrap(),
            )
            .is_ok());
    }

    #[test]
    fn no_password_matches_nothing() {
        let none = HashedPassword::none();
        assert!(!none.is_set());
        assert!(none != *"");
        assert!(none != *"anything at all");
        assert!(hash_password("hunter2").is_set());
    }

    #[test]
    fn hash_password_round_trip() {
        let hashed = hash_password("hunter2");
        assert!(hashed.as_ref().starts_with("$argon2id$v=19$"));
        assert!(hashed == *"hunter2");
        assert!(hashed != *"hunter3");
        assert_ne!(hash_password("hunter2").as_ref(), hashed.as_ref());
    }
}
