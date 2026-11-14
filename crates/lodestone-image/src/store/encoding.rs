//! How a store keeps an object's bytes on disk.
//!
//! Encoding belongs to the store, not to the image: an object is named by the SHA-256 of
//! its plain bytes whatever the encoding, and moving an object between stores re-encodes
//! it. The Launcher stores plain files ([`Encoding::Raw`]), which APFS can clone into an
//! instance folder; the core stores zstd frames (`Encoding::Zstd`, behind the `zstd`
//! feature), written exactly as its backup store has always written them, so the core's
//! existing objects are readable in place.

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

/// How a store keeps objects on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// Plain files, as they are.
    Raw,
    /// One zstd frame per object: level 3, with a checksum and the content size in the
    /// frame header, the settings of the core's backup store.
    #[cfg(feature = "zstd")]
    Zstd,
}

/// zstd's level, as the core's backups have always used.
#[cfg(feature = "zstd")]
pub(crate) const ZSTD_LEVEL: i32 = 3;

/// Bytes are moved in pieces of at most this size.
pub(crate) const IO_CHUNK: usize = 1 << 20;

/// The size of the buffer to move `size` bytes with: big enough to need one read for small
/// files, never more than [`IO_CHUNK`]. The core's backups use the same sizes, which
/// matters for zstd: feeding the encoder the same pieces gives the same frames.
pub(crate) fn buffer_len(size: u64) -> usize {
    usize::try_from(size.saturating_add(1).clamp(8 * 1024, IO_CHUNK as u64)).unwrap_or(IO_CHUNK)
}

/// Writes an object's bytes, encoded, into a file.
pub(crate) enum Encoder {
    Raw(BufWriter<File>),
    #[cfg(feature = "zstd")]
    Zstd(zstd::stream::write::Encoder<'static, BufWriter<File>>),
}

impl Encoder {
    /// Starts encoding `size` bytes into `file`.
    pub(crate) fn new(encoding: Encoding, file: File, size: u64) -> io::Result<Encoder> {
        match encoding {
            Encoding::Raw => {
                let _ = size;
                Ok(Encoder::Raw(BufWriter::new(file)))
            }
            #[cfg(feature = "zstd")]
            Encoding::Zstd => {
                let mut encoder =
                    zstd::stream::write::Encoder::new(BufWriter::new(file), ZSTD_LEVEL)?;
                encoder.include_checksum(true)?;
                encoder.include_contentsize(true)?;
                encoder.set_pledged_src_size(Some(size))?;
                Ok(Encoder::Zstd(encoder))
            }
        }
    }

    pub(crate) fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Encoder::Raw(writer) => writer.write_all(bytes),
            #[cfg(feature = "zstd")]
            Encoder::Zstd(encoder) => encoder.write_all(bytes),
        }
    }

    /// Ends the encoding and returns the file, not yet flushed to disk.
    // Without the `zstd` feature the match has one arm.
    #[allow(clippy::infallible_destructuring_match)]
    pub(crate) fn finish(self) -> io::Result<File> {
        let writer = match self {
            Encoder::Raw(writer) => writer,
            #[cfg(feature = "zstd")]
            Encoder::Zstd(encoder) => encoder.finish()?,
        };
        writer.into_inner().map_err(|e| e.into_error())
    }
}

/// Encodes all of `bytes` into `file`, in the pieces the core's backups feed zstd.
pub(crate) fn encode_into(encoding: Encoding, file: File, bytes: &[u8]) -> io::Result<File> {
    let mut encoder = Encoder::new(encoding, file, bytes.len() as u64)?;
    for piece in bytes.chunks(buffer_len(bytes.len() as u64)) {
        encoder.write_all(piece)?;
    }
    encoder.finish()
}

/// Opens an object file for reading its plain bytes. A zstd object is read as one frame,
/// as the core reads it; callers check the size and SHA-256 of what comes out.
pub(crate) fn decoder(encoding: Encoding, file: File) -> io::Result<Box<dyn Read + Send>> {
    match encoding {
        Encoding::Raw => Ok(Box::new(file)),
        #[cfg(feature = "zstd")]
        Encoding::Zstd => Ok(Box::new(
            zstd::stream::read::Decoder::new(file)?.single_frame(),
        )),
    }
}

/// How many plain bytes the object file at `path` holds, from its length (raw) or its frame
/// header (zstd), without reading it all. `None` when it is missing, not a file, or its
/// header doesn't say (every frame this crate and the core write says).
pub(crate) fn decoded_len(encoding: Encoding, path: &Path) -> Option<u64> {
    let file = File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    match encoding {
        Encoding::Raw => Some(meta.len()),
        #[cfg(feature = "zstd")]
        Encoding::Zstd => {
            let mut file = file;
            // A frame header is at most 18 bytes.
            let mut header = [0u8; 18];
            let mut filled = 0;
            while filled < header.len() {
                match file.read(&mut header[filled..]) {
                    Ok(0) => break,
                    Ok(n) => filled += n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => return None,
                }
            }
            match zstd::zstd_safe::get_frame_content_size(&header[..filled]) {
                Ok(Some(size)) => Some(size),
                _ => None,
            }
        }
    }
}

/// Reads the first `n` plain bytes of the object at `path` (fewer when it is shorter),
/// without decoding the rest.
pub(crate) fn read_prefix(encoding: Encoding, path: &Path, n: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut bytes = Vec::with_capacity(n);
    decoder(encoding, file)?
        .take(n as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Reads all plain bytes of the object at `path`, refusing more than `limit`.
pub(crate) fn read_limited(encoding: Encoding, path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut bytes = Vec::new();
    decoder(encoding, file)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the object is larger than expected",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_sizes_match_the_cores() {
        assert_eq!(buffer_len(0), 8 * 1024);
        assert_eq!(buffer_len(100_000), 100_001);
        assert_eq!(buffer_len(1 << 20), 1 << 20);
        assert_eq!(buffer_len(u64::MAX), 1 << 20);
    }

    #[test]
    fn raw_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        let file = File::create(&path).unwrap();
        encode_into(Encoding::Raw, file, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        assert_eq!(decoded_len(Encoding::Raw, &path), Some(5));
        assert_eq!(read_limited(Encoding::Raw, &path, 5).unwrap(), b"hello");
        assert!(read_limited(Encoding::Raw, &path, 4).is_err());
        assert_eq!(
            decoded_len(Encoding::Raw, &dir.path().join("missing")),
            None
        );
        assert_eq!(decoded_len(Encoding::Raw, dir.path()), None);
    }

    #[cfg(feature = "zstd")]
    #[test]
    fn zstd_round_trip_and_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        encode_into(Encoding::Zstd, File::create(&path).unwrap(), &data).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() < data.len() as u64);
        assert_eq!(decoded_len(Encoding::Zstd, &path), Some(data.len() as u64));
        assert_eq!(read_limited(Encoding::Zstd, &path, 300_000).unwrap(), data);
        // A frame cut short (a crash mid-copy by hand) is an error, not fewer bytes.
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        assert!(read_limited(Encoding::Zstd, &path, 300_000).is_err());
        // Not a frame at all.
        std::fs::write(&path, b"plain").unwrap();
        assert_eq!(decoded_len(Encoding::Zstd, &path), None);
    }
}
