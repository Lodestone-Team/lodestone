//! Offline accounts: a username and the UUID an offline-mode server gives it.

use md5::{Digest as _, Md5};
use uuid::Uuid;

/// Why a name cannot be used for an offline account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UsernameError {
    #[error("Usernames are at least 3 characters long.")]
    TooShort,
    #[error("Usernames are at most 16 characters long.")]
    TooLong,
    #[error("Usernames can only use letters, numbers and underscores.")]
    InvalidCharacter,
}

/// Checks a Minecraft username: 3 to 16 characters, each `A-Z`, `a-z`, `0-9` or `_`.
pub fn validate_username(name: &str) -> Result<(), UsernameError> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(UsernameError::InvalidCharacter);
    }
    match name.len() {
        0..3 => Err(UsernameError::TooShort),
        3..=16 => Ok(()),
        _ => Err(UsernameError::TooLong),
    }
}

/// The UUID an offline-mode server assigns to `name`.
///
/// This is Java's `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`,
/// as the vanilla server computes it: the MD5 of the bytes with no namespace, with
/// the version set to 3 and the IETF variant. It is case-sensitive.
pub fn offline_uuid(name: &str) -> Uuid {
    let digest: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    uuid::Builder::from_md5_bytes(digest).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_offline_uuids() {
        // Notch's offline UUID is widely published; the others were computed with
        // Python's hashlib, an independent MD5, using Java's algorithm.
        for (name, expected) in [
            ("Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f"),
            ("jeb_", "a762f560-4fce-3236-812a-b80efff0b62b"),
            ("Steve", "5627dd98-e6be-3c21-b8a8-e92344183641"),
            ("Alex", "36532b5e-c442-3dbb-a24c-c7e55d0f979a"),
            ("a_b_C_123", "cc8d4480-6cb6-3543-b7c6-a7d2f01cad03"),
        ] {
            let uuid = offline_uuid(name);
            assert_eq!(uuid.to_string(), expected, "{name}");
            assert_eq!(uuid.get_version_num(), 3);
            assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
        }
    }

    #[test]
    fn is_case_sensitive() {
        assert_ne!(offline_uuid("notch"), offline_uuid("Notch"));
    }

    #[test]
    fn validates_usernames() {
        for ok in [
            "abc",
            "Notch",
            "jeb_",
            "a_b_C_123",
            "sixteen_chars_16",
            "___",
        ] {
            assert_eq!(validate_username(ok), Ok(()), "{ok}");
        }
        assert_eq!(validate_username(""), Err(UsernameError::TooShort));
        assert_eq!(validate_username("ab"), Err(UsernameError::TooShort));
        assert_eq!(
            validate_username("seventeen_chars17"),
            Err(UsernameError::TooLong)
        );
        for bad in [
            "with space",
            "dash-name",
            "dot.name",
            "ümlaut",
            "emoji😀",
            "tab\t",
        ] {
            assert_eq!(
                validate_username(bad),
                Err(UsernameError::InvalidCharacter),
                "{bad}"
            );
        }
    }
}
