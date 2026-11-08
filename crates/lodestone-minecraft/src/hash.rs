//! File hashes in the form Modrinth and `.mrpack` indexes use: lowercase hex.

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
