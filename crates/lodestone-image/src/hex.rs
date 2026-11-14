//! Hashes as lowercase hex, the way manifests, Modrinth and `.mrpack` indexes write them.

use crate::sha256::Sha256;

/// The SHA-256 of `bytes`, as 64 lowercase hex characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

/// `bytes` as lowercase hex, two characters each.
pub(crate) fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(byte & 0xf)]));
    }
    out
}

/// A SHA-256 written as 64 lowercase hex characters, as its 32 bytes.
pub(crate) fn sha256_bytes(text: &str) -> Option<[u8; 32]> {
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }
    let (pairs, []) = text.as_bytes().as_chunks::<2>() else {
        return None;
    };
    if pairs.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    for (byte, [high, low]) in out.iter_mut().zip(pairs) {
        *byte = nibble(*high)? << 4 | nibble(*low)?;
    }
    Some(out)
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
