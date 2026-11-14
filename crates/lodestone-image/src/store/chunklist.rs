//! Chunk lists: the object a file over [`CHUNKED_ABOVE`] bytes is stored as.
//!
//! A large file's object (named, like every object, by the SHA-256 of the file) holds the
//! list of its chunks instead of its bytes; the chunks are stored under their own SHA-256
//! in `chunks/`. A list is plain text, one line per chunk, and has exactly one spelling:
//!
//! ```text
//! lodestone-chunks 1
//! size 10000000
//! <sha256 of chunk 1> 81234
//! <sha256 of chunk 2> 65536
//! …
//! ```
//!
//! A reader tells a list from a whole file by length: an object whose plain bytes are as
//! long as the entry's `size` is the file itself (every file up to 1 MiB, and large files
//! the core stored whole before chunking existed); otherwise it must be a list for that
//! size. A list is always far shorter than its file (at most 72 bytes per chunk of at least
//! 16 KiB), so the two can't be confused.

use crate::hex::is_sha256;

use super::cdc::{MAX_CHUNK, MIN_CHUNK};

/// Files larger than this (1 MiB) are stored as chunk lists.
pub const CHUNKED_ABOVE: u64 = 1 << 20;
/// The longest object read whole to find out whether it is a chunk list without knowing
/// the file's size (256 MiB): a list that long stands for a file of tens of terabytes, so
/// anything longer is a whole file.
pub const MAX_LIST_BYTES: u64 = 256 << 20;

const MAGIC: &str = "lodestone-chunks 1\n";
/// The longest line: a SHA-256, a space, a length of up to 6 digits, a newline.
const MAX_LINE: u64 = 64 + 1 + 6 + 1;

/// One chunk of a large file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRef {
    pub sha256: String,
    pub size: u32,
}

/// The chunks of a large file, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkList {
    /// The file's size: the sum of the chunks' sizes.
    pub size: u64,
    pub chunks: Vec<ChunkRef>,
}

impl ChunkList {
    /// The list's one spelling.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::with_capacity(MAGIC.len() + 32 + self.chunks.len() * 72);
        out.push_str(MAGIC);
        out.push_str(&format!("size {}\n", self.size));
        for chunk in &self.chunks {
            out.push_str(&chunk.sha256);
            out.push(' ');
            out.push_str(&chunk.size.to_string());
            out.push('\n');
        }
        out.into_bytes()
    }

    /// Reads a list strictly: the exact header, a size over [`CHUNKED_ABOVE`], chunks of 1
    /// to [`MAX_CHUNK`] bytes that add up to it and are no more than chunking could make
    /// (all but the last at least [`MIN_CHUNK`]), lowercase hashes, and the one spelling
    /// [`ChunkList::encode`] writes. Anything else is damage or forgery.
    pub fn parse(bytes: &[u8]) -> Result<ChunkList, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "the chunk list isn't text")?;
        let rest = text
            .strip_prefix(MAGIC)
            .ok_or("the chunk list's header is wrong")?;
        let (size_line, mut rest) = rest.split_once('\n').ok_or("the chunk list has no size")?;
        let size = size_line
            .strip_prefix("size ")
            .and_then(parse_decimal)
            .ok_or("the chunk list's size is wrong")?;
        if size <= CHUNKED_ABOVE {
            return Err("a chunk list is only for files over 1 MiB".into());
        }
        let max_chunks = max_chunks(size);
        let mut chunks = Vec::new();
        let mut total: u64 = 0;
        while !rest.is_empty() {
            let (line, next) = rest
                .split_once('\n')
                .ok_or("the chunk list's last line is cut short")?;
            rest = next;
            let (hash, len) = line.split_once(' ').ok_or("a chunk line is wrong")?;
            if !is_sha256(hash) {
                return Err("a chunk's hash isn't a SHA-256".into());
            }
            let len = parse_decimal(len).ok_or("a chunk's size is wrong")?;
            if len == 0 || len > MAX_CHUNK as u64 {
                return Err("a chunk's size is out of range".into());
            }
            if chunks.len() as u64 >= max_chunks {
                return Err("the chunk list has more chunks than its size allows".into());
            }
            total = total.saturating_add(len);
            chunks.push(ChunkRef {
                sha256: hash.to_string(),
                size: u32::try_from(len).map_err(|_| "a chunk's size is out of range")?,
            });
        }
        if total != size {
            return Err("the chunks don't add up to the file's size".into());
        }
        if let Some((_, rest)) = chunks.split_last() {
            if rest.iter().any(|c| (c.size as usize) < MIN_CHUNK) {
                return Err("a chunk other than the last is shorter than chunking allows".into());
            }
        }
        let list = ChunkList { size, chunks };
        if list.encode() != bytes {
            return Err("the chunk list isn't in its one spelling".into());
        }
        Ok(list)
    }

    /// Whether `bytes` (an object's start) begin like a chunk list.
    pub fn looks_like(bytes: &[u8]) -> bool {
        bytes.starts_with(MAGIC.as_bytes())
    }

    /// Whether `bytes`, the first bytes read of an object so far, could still be the start
    /// of a chunk list.
    pub fn could_start(bytes: &[u8]) -> bool {
        let n = bytes.len().min(MAGIC.len());
        bytes[..n] == MAGIC.as_bytes()[..n]
    }

    /// How many bytes to read of an object to find the size its list header gives
    /// ([`ChunkList::header_size`]): the magic, `size `, 20 digits and a newline.
    pub const HEADER_LEN: usize = MAGIC.len() + 5 + 20 + 1;

    /// The file size a chunk list's header gives, from an object's first bytes (at least
    /// [`ChunkList::HEADER_LEN`] of them, or all of a shorter object); `None` when they
    /// don't start like a list. Lets a reader skip reading a whole object to find it is
    /// a list for another size, or no list.
    pub fn header_size(bytes: &[u8]) -> Option<u64> {
        let rest = bytes.strip_prefix(MAGIC.as_bytes())?;
        let rest = rest.strip_prefix(b"size ")?;
        let end = rest.iter().position(|&b| b == b'\n')?;
        let digits = std::str::from_utf8(&rest[..end]).ok()?;
        parse_decimal(digits)
    }

    /// How many bytes to read of an object to tell whether it is a list ([`looks_like`]).
    ///
    /// [`looks_like`]: ChunkList::looks_like
    pub const MAGIC_LEN: usize = MAGIC.len();

    /// The longest a list for a file of `size` bytes can be: the bound to read an object
    /// with before parsing it as one.
    pub fn max_encoded_len(size: u64) -> u64 {
        MAGIC.len() as u64 + 26 + max_chunks(size).saturating_mul(MAX_LINE)
    }
}

/// The most chunks a file of `size` bytes can have: all but the last at least
/// [`MIN_CHUNK`].
fn max_chunks(size: u64) -> u64 {
    size / MIN_CHUNK as u64 + 1
}

/// A decimal number without sign, leading zeros (other than `0` itself) or spaces.
fn parse_decimal(text: &str) -> Option<u64> {
    if text.is_empty()
        || text.len() > 20
        || !text.bytes().all(|b| b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(sizes: &[u32]) -> ChunkList {
        ChunkList {
            size: sizes.iter().map(|&s| u64::from(s)).sum(),
            chunks: sizes
                .iter()
                .enumerate()
                .map(|(i, &size)| ChunkRef {
                    sha256: format!("{i:064x}"),
                    size,
                })
                .collect(),
        }
    }

    #[test]
    fn round_trip() {
        let original = list(&[262_144, 100_000, 300_000 - 37_856, 262_144, 262_144, 70_000]);
        assert!(original.size > CHUNKED_ABOVE);
        let bytes = original.encode();
        assert!(ChunkList::looks_like(&bytes));
        assert!(bytes.len() as u64 <= ChunkList::max_encoded_len(original.size));
        assert_eq!(ChunkList::parse(&bytes).unwrap(), original);
        assert!(bytes.starts_with(b"lodestone-chunks 1\nsize 1218576\n0000"));
    }

    #[test]
    fn the_header_gives_the_size_without_the_rest() {
        let original = list(&[MAX_CHUNK as u32, MAX_CHUNK as u32, 100]);
        let bytes = original.encode();
        let head = &bytes[..ChunkList::HEADER_LEN.min(bytes.len())];
        assert_eq!(ChunkList::header_size(head), Some(original.size));
        let biggest = format!("{MAGIC}size {}\n", u64::MAX);
        assert_eq!(biggest.len(), ChunkList::HEADER_LEN);
        assert_eq!(ChunkList::header_size(biggest.as_bytes()), Some(u64::MAX));
        for bad in [
            &b"PK\x03\x04 a jar"[..],
            b"lodestone-chunks 1\nsize 012\n",
            b"lodestone-chunks 1\nsize 12",
            b"lodestone-chunks 1\nsize x\n",
            b"lodestone-chunks 2\nsize 12\n",
            b"",
        ] {
            assert_eq!(ChunkList::header_size(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_list_is_always_much_shorter_than_its_file() {
        for size in [CHUNKED_ABOVE + 1, 10 << 20, 1 << 40] {
            assert!(ChunkList::max_encoded_len(size) * 100 < size);
        }
    }

    #[test]
    fn hostile_lists_are_refused() {
        let good = list(&[262_144, 262_144, 262_144, 262_144, 5]);
        let text = String::from_utf8(good.encode()).unwrap();
        let bad: Vec<String> = vec![
            text.replace("lodestone-chunks 1", "lodestone-chunks 2"),
            text.replace("size 1048581", "size 01048581"),
            text.replace("size 1048581", "size 1048582"),
            text.replace(" 5\n", " 05\n"),
            text.replace(" 5\n", " 0\n"),
            text.replace(" 5\n", " 5"),
            text.replace(" 5\n", "  5\n"),
            text.replace(&format!("{:064x}", 0), &format!("{:064X}", 0xabc)),
            text.replace(&format!("{:064x}", 0), "abc"),
            format!("{text}\n"),
            text.replace("\n", "\r\n"),
            // Over the maximum chunk size.
            String::from_utf8(list(&[262_145, 262_144, 262_144, 262_144, 4]).encode()).unwrap(),
            // A small chunk in the middle: chunking never makes one.
            String::from_utf8(list(&[262_144, 100, 262_144, 262_144, 262_144]).encode()).unwrap(),
            // A list for a file that is stored whole.
            String::from_utf8(list(&[262_144, 262_144]).encode()).unwrap(),
        ];
        for (i, text) in bad.iter().enumerate() {
            assert!(ChunkList::parse(text.as_bytes()).is_err(), "case {i}");
        }
        assert!(ChunkList::parse(&[0xff, 0xfe]).is_err());
        assert!(ChunkList::parse(b"").is_err());
    }

    #[test]
    fn a_huge_count_is_refused_before_it_is_built() {
        // A 1 MiB + 1 file can have at most 65 chunks.
        let mut text = String::from("lodestone-chunks 1\nsize 1048577\n");
        for i in 0..200 {
            text.push_str(&format!("{i:064x} 5243\n"));
        }
        assert!(ChunkList::parse(text.as_bytes())
            .unwrap_err()
            .contains("more chunks"));
    }
}
