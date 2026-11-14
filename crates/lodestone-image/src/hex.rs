//! Hashes as lowercase hex, the way manifests, Modrinth and `.mrpack` indexes write them.

use sha2::{Digest, Sha256};

/// The SHA-256 of `bytes`, as 64 lowercase hex characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(byte & 0xf)]));
    }
    out
}

/// Whether `text` is exactly `len` lowercase hex characters. Uppercase is refused so a hash
/// has one spelling, and ids and store keys compare as plain strings.
pub fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A SHA-256 (an object's store key, an image id): 64 lowercase hex characters.
pub fn is_sha256(text: &str) -> bool {
    is_lower_hex(text, 64)
}

/// A SHA-1: 40 lowercase hex characters.
pub fn is_sha1(text: &str) -> bool {
    is_lower_hex(text, 40)
}

/// A SHA-512: 128 lowercase hex characters.
pub fn is_sha512(text: &str) -> bool {
    is_lower_hex(text, 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_known_input() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hex_checks() {
        assert!(is_sha256(&sha256_hex(b"x")));
        assert!(!is_sha256(&sha256_hex(b"x").to_uppercase()));
        assert!(!is_sha256("abc"));
        assert!(!is_sha256(&"g".repeat(64)));
        assert!(is_sha1(&"0".repeat(40)));
        assert!(!is_sha1(&"0".repeat(41)));
        assert!(is_sha512(&"f".repeat(128)));
        // Multi-byte characters never pass, whatever their byte length.
        assert!(!is_lower_hex("é", 2));
    }
}
