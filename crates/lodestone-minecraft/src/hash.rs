//! File hashes in the form Modrinth and `.mrpack` indexes use (lowercase hex), and
//! CurseForge's fingerprint of a file.

use std::io::Read;
use std::path::Path;

fn hash_file<D: sha2::Digest>(path: &Path) -> std::io::Result<String>
where
    sha2::digest::Output<D>: std::fmt::LowerHex,
{
    let mut file = std::fs::File::open(path)?;
    let mut hasher = D::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// The SHA-1 of a file, as lowercase hex.
pub fn sha1_of(path: &Path) -> std::io::Result<String> {
    hash_file::<sha1::Sha1>(path)
}

/// The SHA-512 of a file, as lowercase hex.
pub fn sha512_of(path: &Path) -> std::io::Result<String> {
    hash_file::<sha2::Sha512>(path)
}

/// CurseForge's fingerprint of a file's bytes: 32-bit MurmurHash2 with seed 1 over the
/// bytes left after removing tabs, line feeds, carriage returns and spaces.
pub fn curseforge_fingerprint(bytes: &[u8]) -> u32 {
    let mut hasher = Murmur2::seeded(
        1,
        bytes.iter().filter(|b| !is_whitespace(**b)).count() as u32,
    );
    hasher.update(bytes);
    hasher.finish()
}

/// [`curseforge_fingerprint`] of a file, read twice in chunks (the length of the filtered bytes is
/// part of the hash's seed) rather than into memory at once.
pub fn curseforge_fingerprint_of(path: &Path) -> std::io::Result<u32> {
    let mut length = 0u32;
    read_chunks(path, |chunk| {
        length = length.wrapping_add(chunk.iter().filter(|b| !is_whitespace(**b)).count() as u32)
    })?;
    let mut hasher = Murmur2::seeded(1, length);
    read_chunks(path, |chunk| hasher.update(chunk))?;
    Ok(hasher.finish())
}

fn read_chunks(path: &Path, mut each: impl FnMut(&[u8])) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        each(&buffer[..read]);
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x0a | 0x0d | 0x20)
}

const M: u32 = 0x5bd1_e995;

/// MurmurHash2 over whitespace-filtered bytes fed in any number of pieces.
struct Murmur2 {
    h: u32,
    pending: [u8; 4],
    filled: usize,
}

impl Murmur2 {
    /// CurseForge's seed is 1; other seeds are for checking against known answers.
    fn seeded(seed: u32, length: u32) -> Self {
        Self {
            h: seed ^ length,
            pending: [0; 4],
            filled: 0,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if is_whitespace(byte) {
                continue;
            }
            self.pending[self.filled] = byte;
            self.filled += 1;
            if self.filled == 4 {
                let mut k = u32::from_le_bytes(self.pending);
                k = k.wrapping_mul(M);
                k ^= k >> 24;
                k = k.wrapping_mul(M);
                self.h = self.h.wrapping_mul(M) ^ k;
                self.filled = 0;
            }
        }
    }

    fn finish(mut self) -> u32 {
        let tail = &self.pending[..self.filled];
        if tail.len() >= 3 {
            self.h ^= u32::from(tail[2]) << 16;
        }
        if tail.len() >= 2 {
            self.h ^= u32::from(tail[1]) << 8;
        }
        if !tail.is_empty() {
            self.h ^= u32::from(tail[0]);
            self.h = self.h.wrapping_mul(M);
        }
        self.h ^= self.h >> 13;
        self.h = self.h.wrapping_mul(M);
        self.h ^= self.h >> 15;
        self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straightforward MurmurHash2 (seed 1) over already-filtered bytes, to check the
    /// streaming one against.
    fn reference(data: &[u8]) -> u32 {
        let mut h = 1u32 ^ data.len() as u32;
        let (chunks, tail) = data.as_chunks::<4>();
        for chunk in chunks {
            let mut k = u32::from_le_bytes(*chunk);
            k = k.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            h = h.wrapping_mul(M) ^ k;
        }
        match tail.len() {
            3 => {
                h ^= u32::from(tail[2]) << 16;
                h ^= u32::from(tail[1]) << 8;
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            2 => {
                h ^= u32::from(tail[1]) << 8;
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            1 => {
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            _ => {}
        }
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        h
    }

    /// Known answers for MurmurHash2 from github.com/aviddiviner/go-murmur's tests,
    /// the implementation packwiz uses for CurseForge fingerprints.
    #[test]
    fn murmur2_matches_published_values() {
        let hash = |data: &[u8], seed: u32| {
            let mut hasher = Murmur2::seeded(seed, data.len() as u32);
            hasher.update(data);
            hasher.finish()
        };
        assert_eq!(hash(b"foo", 123), 1_412_061_192);
        assert_eq!(hash(b"zztop", 123), 1_878_194_508);
        assert_eq!(hash(b"foobarbaz", 234), 1_777_016_281);
        assert_eq!(hash(b"blam", 777), 1_668_928_339);
    }

    #[test]
    fn fingerprints_ignore_whitespace() {
        assert_eq!(
            curseforge_fingerprint(b"a b\tc\r\nd"),
            curseforge_fingerprint(b"abcd")
        );
        assert_ne!(
            curseforge_fingerprint(b"abcd"),
            curseforge_fingerprint(b"abce")
        );
        // Only those four bytes count as whitespace.
        assert_ne!(
            curseforge_fingerprint(b"ab\x0bcd"),
            curseforge_fingerprint(b"abcd")
        );
    }

    #[test]
    fn fingerprints_match_murmur2_with_seed_1() {
        // The empty input: h = 1 ^ 0, then the final mix.
        let mut h = 1u32;
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        assert_eq!(curseforge_fingerprint(b""), h);
        for data in [
            &b"x"[..],
            b"xy",
            b"xyz",
            b"wxyz",
            b"hello, world!",
            &[0xffu8; 37],
        ] {
            let filtered: Vec<u8> = data
                .iter()
                .copied()
                .filter(|b| !is_whitespace(*b))
                .collect();
            assert_eq!(
                curseforge_fingerprint(data),
                reference(&filtered),
                "{data:?}"
            );
        }
    }

    #[test]
    fn a_files_fingerprint_is_read_in_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.jar");
        let bytes: Vec<u8> = (0..3_000_001u32).map(|i| (i * 7 % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            curseforge_fingerprint_of(&path).unwrap(),
            curseforge_fingerprint(&bytes)
        );
    }
}
