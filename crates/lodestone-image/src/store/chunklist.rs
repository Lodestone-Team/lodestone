//! Chunk lists: how a file over [`CHUNKED_ABOVE`] bytes is stored.
//!
//! A large file's chunks are stored under their own SHA-256 in `chunks/`, and the list of
//! them under the file's SHA-256 in `lists/` (never in `objects/`, which only ever holds a
//! file's own bytes: released Launchers share that folder and read any file there as the
//! file it names). A list is plain text, one line per chunk, and has exactly one spelling:
//!
//! ```text
//! lodestone-chunks 1
//! size 10000000
//! <sha256 of chunk 1> 81234
//! <sha256 of chunk 2> 65536
//! …
//! ```
//!
//! In an export, which has one `objects/` folder, a reader tells a list from a whole file
//! by length: an object whose plain bytes are as long as the entry's `size` is the file
//! itself; otherwise it must be a list for that size. A list is always far shorter than
//! its file (at most 72 bytes per chunk of at least 16 KiB), so the two can't be confused.

use std::io::{BufRead, Read};

use crate::hex::is_sha256;

use super::cdc::{MAX_CHUNK, MIN_CHUNK};

/// Files larger than this (1 MiB) are stored as chunk lists.
pub const CHUNKED_ABOVE: u64 = 1 << 20;
/// The longest chunk list read (256 MiB): a list that long stands for a file of tens of
/// terabytes.
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
        let mut chunks = Vec::new();
        let size = ChunkList::visit(&mut &bytes[..], &mut |sha256, size| {
            chunks.push(ChunkRef {
                sha256: sha256.to_string(),
                size,
            });
            Ok(())
        })?;
        let list = ChunkList { size, chunks };
        debug_assert!(list.encode() == bytes, "a list read in another spelling");
        Ok(list)
    }

    /// Reads a list from `reader` with the rules of [`ChunkList::parse`], giving each chunk
    /// (its hash and size) to `chunk` in order instead of keeping them, and returns the
    /// file's size. Memory stays a line's worth however long the list is, so a list from
    /// elsewhere (an export's, which can name one chunk millions of times) can be checked
    /// without building it. The rules leave one spelling: exactly one space and one newline
    /// per line, lowercase hashes, decimals without leading zeros. An error from `chunk`
    /// stops the read and is returned.
    pub fn visit(
        reader: &mut dyn BufRead,
        chunk: &mut dyn FnMut(&str, u32) -> Result<(), String>,
    ) -> Result<u64, String> {
        let mut line = Vec::with_capacity(MAX_LINE as usize + 1);
        let magic = next_line(reader, &mut line, MAGIC.len() as u64)?
            .ok_or("the chunk list's header is wrong")?;
        if magic != MAGIC.as_bytes() {
            return Err("the chunk list's header is wrong".into());
        }
        let size_line =
            next_line(reader, &mut line, 5 + 20 + 1)?.ok_or("the chunk list has no size")?;
        let size = std::str::from_utf8(size_line)
            .ok()
            .and_then(|l| l.strip_suffix('\n'))
            .and_then(|l| l.strip_prefix("size "))
            .and_then(parse_decimal)
            .ok_or("the chunk list's size is wrong")?;
        if size <= CHUNKED_ABOVE {
            return Err("a chunk list is only for files over 1 MiB".into());
        }
        let max_chunks = max_chunks(size);
        let mut count: u64 = 0;
        let mut total: u64 = 0;
        // Whether the last chunk seen was shorter than chunking makes all but the last.
        let mut short_seen = false;
        while let Some(text) = next_line(reader, &mut line, MAX_LINE)? {
            let text = std::str::from_utf8(text)
                .ok()
                .and_then(|l| l.strip_suffix('\n'))
                .ok_or("a chunk line is wrong")?;
            let (hash, len) = text.split_once(' ').ok_or("a chunk line is wrong")?;
            if !is_sha256(hash) {
                return Err("a chunk's hash isn't a SHA-256".into());
            }
            let len = parse_decimal(len).ok_or("a chunk's size is wrong")?;
            if len == 0 || len > MAX_CHUNK as u64 {
                return Err("a chunk's size is out of range".into());
            }
            if count >= max_chunks {
                return Err("the chunk list has more chunks than its size allows".into());
            }
            if short_seen {
                return Err("a chunk other than the last is shorter than chunking allows".into());
            }
            short_seen = len < MIN_CHUNK as u64;
            count += 1;
            total = total.saturating_add(len);
            // At most MAX_CHUNK, by the check above.
            chunk(
                hash,
                u32::try_from(len).map_err(|_| "a chunk's size is out of range")?,
            )?;
        }
        if total != size {
            return Err("the chunks don't add up to the file's size".into());
        }
        Ok(size)
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

/// The next line of `reader` into `buf`, newline included, if it is at most `max` bytes;
/// `None` at the end. A last line without its newline is an error, and so is a longer one
/// (read no further than `max` + 1 bytes, so a line never costs more memory than that).
fn next_line<'b>(
    reader: &mut dyn BufRead,
    buf: &'b mut Vec<u8>,
    max: u64,
) -> Result<Option<&'b [u8]>, String> {
    buf.clear();
    (&mut *reader)
        .take(max.saturating_add(1))
        .read_until(b'\n', buf)
        .map_err(|e| format!("the chunk list can't be read ({e})"))?;
    if buf.is_empty() {
        return Ok(None);
    }
    if buf.len() as u64 > max {
        return Err("a chunk list line is too long".into());
    }
    if buf.last() != Some(&b'\n') {
        return Err("the chunk list's last line is cut short".into());
    }
    Ok(Some(buf))
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
            text.push_str(&format!("{i:064x} 16384\n"));
        }
        assert!(ChunkList::parse(text.as_bytes())
            .unwrap_err()
            .contains("more chunks"));
    }

    #[test]
    fn visiting_streams_and_stops_at_the_first_error() {
        let good = list(&[262_144, 262_144, 262_144, 262_144, 5]);
        let bytes = good.encode();
        let mut seen = Vec::new();
        let size = ChunkList::visit(&mut &bytes[..], &mut |sha256, size| {
            seen.push((sha256.to_string(), size));
            Ok(())
        })
        .unwrap();
        assert_eq!(size, good.size);
        assert_eq!(seen.len(), 5);
        let mut calls = 0;
        let stopped = ChunkList::visit(&mut &bytes[..], &mut |_, _| {
            calls += 1;
            Err("no".into())
        });
        assert_eq!(stopped, Err("no".to_string()));
        assert_eq!(calls, 1);
        // A line far longer than any list's is refused without reading it all.
        let mut long = b"lodestone-chunks 1\nsize 2000000\n".to_vec();
        long.extend(std::iter::repeat_n(b'a', 10 << 20));
        assert!(ChunkList::parse(&long).unwrap_err().contains("too long"));
    }
}
