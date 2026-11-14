//! SHA-256, from ring, which uses the CPU's SHA-256 instructions where it has them (ARMv8's
//! on Apple silicon and ARM servers, SHA-NI on recent x86) and fast assembly elsewhere.
//! Every hash the crate takes goes through here: object and chunk keys, image ids, and
//! the checks of what's read back.

use ring::digest::{Context, SHA256};

/// A SHA-256 being taken, a piece at a time.
#[derive(Clone)]
pub(crate) struct Sha256(Context);

impl Sha256 {
    pub(crate) fn new() -> Self {
        Self(Context::new(&SHA256))
    }

    pub(crate) fn update(&mut self, bytes: impl AsRef<[u8]>) {
        self.0.update(bytes.as_ref());
    }

    /// The hash of everything given to [`Sha256::update`].
    pub(crate) fn finalize(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(self.0.finish().as_ref());
        out
    }

    /// The hash of `bytes`.
    pub(crate) fn digest(bytes: impl AsRef<[u8]>) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(bytes);
        hasher.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex::lower_hex;

    /// FIPS 180-2's test vectors, and one cut into uneven pieces (ring buffers partial
    /// blocks, so a piece boundary inside a block must not change the hash).
    #[test]
    fn known_hashes() {
        assert_eq!(
            lower_hex(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            lower_hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(
            lower_hex(&Sha256::digest(long)),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let mut pieces = Sha256::new();
        for piece in long.chunks(7) {
            pieces.update(piece);
        }
        assert_eq!(pieces.finalize(), Sha256::digest(long));
    }

    /// A million `a`s, FIPS 180-2's long vector, given in pieces larger than a block.
    #[test]
    fn million_a() {
        let mut hasher = Sha256::new();
        let piece = [b'a'; 1000];
        for _ in 0..1000 {
            hasher.update(piece);
        }
        assert_eq!(
            lower_hex(&hasher.finalize()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
