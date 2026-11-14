//! Minecraft's NBT format (Java Edition): a reader and writer for whole files, such as a
//! world's `level.dat`.
//!
//! The tree keeps everything as it was read, in order, with strings as the raw bytes
//! of Java's "modified UTF-8", so writing a tree that was read gives back the same bytes.
//! Lengths are checked against the input before anything is allocated, the tags built
//! may take at most [`TAG_MEMORY_PER_BYTE`] times the input's size in memory, and nesting
//! is limited to [`MAX_DEPTH`], as in the game.
//!
//! See <https://minecraft.wiki/w/NBT_format>.

use std::io::{Read, Write};

/// How deep compounds and lists may nest.
pub const MAX_DEPTH: usize = 512;

/// How much memory the tags read may take, per byte of input. A tag takes far more
/// memory than its smallest encoding (an empty compound is one byte read but a whole
/// [`Tag`] in memory), so a file of a few tiny tags repeated could otherwise cost 32
/// times its size. Real files take about 3.
pub const TAG_MEMORY_PER_BYTE: usize = 8;

/// The tag memory any input may take, however small.
const MIN_TAG_MEMORY: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NbtError {
    #[error("the NBT data ends early")]
    UnexpectedEnd,
    #[error("unknown NBT tag type {0}")]
    UnknownTag(u8),
    #[error("a negative length in NBT data")]
    NegativeLength,
    #[error("NBT data nests more than {MAX_DEPTH} levels deep")]
    TooDeep,
    #[error("the root of NBT data must be a compound")]
    RootNotCompound,
    #[error("{0} bytes are left after the NBT data")]
    TrailingBytes(usize),
    #[error("the file isn't gzip or NBT data")]
    NotNbt,
    #[error("the file is larger than {0} bytes unpacked")]
    TooLarge(u64),
    #[error("the gzip data is damaged: {0}")]
    Gzip(String),
    #[error("the NBT data holds more tags than its size allows")]
    TooManyTags,
}

/// A string as stored: Java's modified UTF-8.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct NbtString(pub Vec<u8>);

impl NbtString {
    /// Decodes modified UTF-8, replacing what isn't valid.
    pub fn to_string_lossy(&self) -> String {
        decode_mutf8(&self.0)
    }

    pub fn is(&self, value: &str) -> bool {
        self.0 == encode_mutf8(value)
    }
}

impl From<&str> for NbtString {
    fn from(value: &str) -> Self {
        NbtString(encode_mutf8(value))
    }
}

impl std::fmt::Debug for NbtString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.to_string_lossy())
    }
}

/// A tag's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(NbtString),
    /// The element type, and the elements (all of that type).
    List(u8, Vec<Tag>),
    Compound(Compound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    /// The value of a whole-number tag (byte, short, int or long).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Tag::Byte(v) => Some(i64::from(*v)),
            Tag::Short(v) => Some(i64::from(*v)),
            Tag::Int(v) => Some(i64::from(*v)),
            Tag::Long(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<String> {
        match self {
            Tag::String(s) => Some(s.to_string_lossy()),
            _ => None,
        }
    }

    pub fn as_compound(&self) -> Option<&Compound> {
        match self {
            Tag::Compound(c) => Some(c),
            _ => None,
        }
    }

    pub fn as_compound_mut(&mut self) -> Option<&mut Compound> {
        match self {
            Tag::Compound(c) => Some(c),
            _ => None,
        }
    }
}

/// Named tags, in the order they were read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Compound(pub Vec<(NbtString, Tag)>);

impl Compound {
    pub fn get(&self, name: &str) -> Option<&Tag> {
        let key = encode_mutf8(name);
        self.0.iter().find(|(k, _)| k.0 == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Tag> {
        let key = encode_mutf8(name);
        self.0.iter_mut().find(|(k, _)| k.0 == key).map(|(_, v)| v)
    }

    pub fn compound(&self, name: &str) -> Option<&Compound> {
        self.get(name)?.as_compound()
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.get(name)?.as_i64()
    }

    pub fn string(&self, name: &str) -> Option<String> {
        self.get(name)?.as_str()
    }

    /// Replaces the value of `name`, or adds it at the end.
    pub fn set(&mut self, name: &str, value: Tag) {
        match self.get_mut(name) {
            Some(slot) => *slot = value,
            None => self.0.push((NbtString::from(name), value)),
        }
    }

    /// Removes `name`, returning its value.
    pub fn remove(&mut self, name: &str) -> Option<Tag> {
        let key = encode_mutf8(name);
        let index = self.0.iter().position(|(k, _)| k.0 == key)?;
        Some(self.0.remove(index).1)
    }
}

/// A whole NBT document: a named root compound.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Root {
    pub name: NbtString,
    pub value: Compound,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// The bytes of tag memory still allowed.
    budget: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader {
            bytes,
            pos: 0,
            budget: bytes
                .len()
                .saturating_mul(TAG_MEMORY_PER_BYTE)
                .max(MIN_TAG_MEMORY),
        }
    }

    /// Takes `count` items of type `T` from the budget, before they are allocated.
    fn charge<T>(&mut self, count: usize) -> Result<(), NbtError> {
        let size = count.saturating_mul(std::mem::size_of::<T>());
        self.budget = self.budget.checked_sub(size).ok_or(NbtError::TooManyTags)?;
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], NbtError> {
        if self.remaining() < n {
            return Err(NbtError::UnexpectedEnd);
        }
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], NbtError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, NbtError> {
        Ok(self.take(1)?[0])
    }

    /// A length, checked to fit in what is left if each element takes `size` bytes.
    fn length(&mut self, size: usize) -> Result<usize, NbtError> {
        let len = i32::from_be_bytes(self.array()?);
        if len < 0 {
            return Err(NbtError::NegativeLength);
        }
        let len = len as usize;
        if len.saturating_mul(size.max(1)) > self.remaining() {
            return Err(NbtError::UnexpectedEnd);
        }
        Ok(len)
    }

    fn string(&mut self) -> Result<NbtString, NbtError> {
        let len = u16::from_be_bytes(self.array()?) as usize;
        Ok(NbtString(self.take(len)?.to_vec()))
    }

    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag, NbtError> {
        if depth > MAX_DEPTH {
            return Err(NbtError::TooDeep);
        }
        Ok(match id {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(f32::from_bits(u32::from_be_bytes(self.array()?))),
            6 => Tag::Double(f64::from_bits(u64::from_be_bytes(self.array()?))),
            7 => {
                let len = self.length(1)?;
                Tag::ByteArray(self.take(len)?.to_vec())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let element = self.u8()?;
                if element > 12 {
                    return Err(NbtError::UnknownTag(element));
                }
                // Every element takes at least one byte, except empty compounds' End
                // byte, which is still one.
                let len = self.length(element_size(element))?;
                if element == 0 && len > 0 {
                    return Err(NbtError::UnknownTag(0));
                }
                self.charge::<Tag>(len)?;
                let mut items = Vec::with_capacity(len);
                for _ in 0..len {
                    items.push(self.payload(element, depth + 1)?);
                }
                Tag::List(element, items)
            }
            10 => Tag::Compound(self.compound(depth + 1)?),
            11 => {
                let len = self.length(4)?;
                Tag::IntArray(
                    (0..len)
                        .map(|_| self.array().map(i32::from_be_bytes))
                        .collect::<Result<_, _>>()?,
                )
            }
            12 => {
                let len = self.length(8)?;
                Tag::LongArray(
                    (0..len)
                        .map(|_| self.array().map(i64::from_be_bytes))
                        .collect::<Result<_, _>>()?,
                )
            }
            other => return Err(NbtError::UnknownTag(other)),
        })
    }

    fn compound(&mut self, depth: usize) -> Result<Compound, NbtError> {
        if depth > MAX_DEPTH {
            return Err(NbtError::TooDeep);
        }
        let mut entries = Vec::new();
        loop {
            let id = self.u8()?;
            if id == 0 {
                return Ok(Compound(entries));
            }
            self.charge::<(NbtString, Tag)>(1)?;
            let name = self.string()?;
            let value = self.payload(id, depth)?;
            entries.push((name, value));
        }
    }
}

/// The smallest size of one list element of type `id`.
fn element_size(id: u8) -> usize {
    match id {
        2 => 2,
        3 | 5 => 4,
        4 | 6 => 8,
        7 | 11 | 12 => 4,
        8 => 2,
        _ => 1,
    }
}

/// Reads an uncompressed NBT document whose root is a compound.
pub fn read(bytes: &[u8]) -> Result<Root, NbtError> {
    let mut reader = Reader::new(bytes);
    let id = reader.u8()?;
    if id != 10 {
        return Err(NbtError::RootNotCompound);
    }
    let name = reader.string()?;
    let value = reader.compound(1)?;
    if reader.remaining() > 0 {
        return Err(NbtError::TrailingBytes(reader.remaining()));
    }
    Ok(Root { name, value })
}

fn write_string(out: &mut Vec<u8>, value: &NbtString) {
    // Longer strings can't be stored; they are cut, as Java would refuse them.
    let bytes = &value.0[..value.0.len().min(u16::MAX as usize)];
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn write_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len.min(i32::MAX as usize) as i32).to_be_bytes());
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
        Tag::ByteArray(v) => {
            write_len(out, v.len());
            out.extend_from_slice(v);
        }
        Tag::String(s) => write_string(out, s),
        Tag::List(element, items) => {
            out.push(*element);
            write_len(out, items.len());
            for item in items {
                write_payload(out, item);
            }
        }
        Tag::Compound(c) => write_compound(out, c),
        Tag::IntArray(v) => {
            write_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            write_len(out, v.len());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

fn write_compound(out: &mut Vec<u8>, compound: &Compound) {
    for (name, tag) in &compound.0 {
        out.push(tag.id());
        write_string(out, name);
        write_payload(out, tag);
    }
    out.push(0);
}

/// Writes an uncompressed NBT document.
pub fn write(root: &Root) -> Vec<u8> {
    let mut out = vec![10];
    write_string(&mut out, &root.name);
    write_compound(&mut out, &root.value);
    out
}

/// The uncompressed bytes of `bytes`: gzip data is unpacked (at most `limit` bytes), and
/// uncompressed NBT is returned as it is.
pub fn unpack(bytes: &[u8], limit: u64) -> Result<Vec<u8>, NbtError> {
    match bytes {
        [0x1f, 0x8b, ..] => {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .take(limit + 1)
                .read_to_end(&mut out)
                .map_err(|err| NbtError::Gzip(err.to_string()))?;
            if out.len() as u64 > limit {
                return Err(NbtError::TooLarge(limit));
            }
            Ok(out)
        }
        [10, ..] => Ok(bytes.to_vec()),
        _ => Err(NbtError::NotNbt),
    }
}

/// Gzips `bytes`, as the game stores `level.dat`.
pub fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    // Writing to a Vec can't fail.
    let _ = encoder.write_all(bytes);
    encoder.finish().unwrap_or_default()
}

/// Encodes `value` as Java's modified UTF-8: NUL as two bytes, and characters outside
/// the Basic Multilingual Plane as two three-byte surrogates.
pub fn encode_mutf8(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len());
    for c in value.chars() {
        match c as u32 {
            0 => out.extend_from_slice(&[0xC0, 0x80]),
            0x10000.. => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let u = *unit as u32;
                    out.push((0xE0 | (u >> 12)) as u8);
                    out.push((0x80 | ((u >> 6) & 0x3F)) as u8);
                    out.push((0x80 | (u & 0x3F)) as u8);
                }
            }
            _ => {
                let mut buffer = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out
}

/// Decodes Java's modified UTF-8, replacing what isn't valid.
pub fn decode_mutf8(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        if !text.contains('\u{0}') && !bytes.windows(2).any(|w| w[0] == 0xED && w[1] >= 0xA0) {
            return text.to_string();
        }
    }
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let (unit, len) = match b {
            0x00..=0x7F => (u16::from(b), 1),
            0xC0..=0xDF if i + 1 < bytes.len() => (
                ((u16::from(b) & 0x1F) << 6) | (u16::from(bytes[i + 1]) & 0x3F),
                2,
            ),
            0xE0..=0xEF if i + 2 < bytes.len() => (
                ((u16::from(b) & 0x0F) << 12)
                    | ((u16::from(bytes[i + 1]) & 0x3F) << 6)
                    | (u16::from(bytes[i + 2]) & 0x3F),
                3,
            ),
            _ => (0xFFFD, 1),
        };
        units.push(unit);
        i += len;
    }
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Root {
        let mut inner = Compound::default();
        inner.set("Name", Tag::String("1.20.1".into()));
        inner.set("Id", Tag::Int(3465));
        let mut data = Compound::default();
        data.set("LevelName", Tag::String("My World \u{1F600}".into()));
        data.set("hardcore", Tag::Byte(1));
        data.set("Short", Tag::Short(-2));
        data.set("LastPlayed", Tag::Long(1_762_500_000_000));
        data.set("Float", Tag::Float(0.5));
        data.set("Double", Tag::Double(-1.25));
        data.set("Bytes", Tag::ByteArray(vec![1, 2, 255]));
        data.set("Ints", Tag::IntArray(vec![1, -1]));
        data.set("Longs", Tag::LongArray(vec![i64::MAX]));
        data.set("Version", Tag::Compound(inner));
        data.set(
            "List",
            Tag::List(8, vec![Tag::String("a".into()), Tag::String("".into())]),
        );
        data.set("Empty", Tag::List(0, vec![]));
        let mut root = Compound::default();
        root.set("Data", Tag::Compound(data));
        Root {
            name: NbtString::default(),
            value: root,
        }
    }

    #[test]
    fn round_trips() {
        let root = sample();
        let bytes = write(&root);
        let read_back = read(&bytes).unwrap();
        assert_eq!(read_back, root);
        assert_eq!(write(&read_back), bytes);
        let data = read_back.value.compound("Data").unwrap();
        assert_eq!(data.string("LevelName").unwrap(), "My World \u{1F600}");
        assert_eq!(data.int("hardcore"), Some(1));
        assert_eq!(data.compound("Version").unwrap().int("Id"), Some(3465));
        let packed = gzip(&bytes);
        assert_eq!(unpack(&packed, 1 << 20).unwrap(), bytes);
        assert_eq!(unpack(&bytes, 1 << 20).unwrap(), bytes);
        assert!(matches!(unpack(&packed, 10), Err(NbtError::TooLarge(10))));
        assert!(matches!(unpack(b"nope", 10), Err(NbtError::NotNbt)));
    }

    #[test]
    fn modified_utf8() {
        let text = "a\u{0}é\u{1F600}";
        let bytes = encode_mutf8(text);
        assert!(!bytes.contains(&0));
        assert_eq!(bytes[1..3], [0xC0, 0x80]);
        // a surrogate pair, not a four-byte sequence
        assert_eq!(bytes.len(), 1 + 2 + 2 + 6);
        assert_eq!(decode_mutf8(&bytes), text);
        assert_eq!(decode_mutf8(b"plain"), "plain");
        assert_eq!(decode_mutf8(&[0xFF, b'a']), "\u{FFFD}a");
    }

    #[test]
    fn refuses_bad_data() {
        let bytes = write(&sample());
        for cut in [0, 1, 3, bytes.len() / 2, bytes.len() - 1] {
            assert!(read(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(read(&trailing), Err(NbtError::TrailingBytes(1)));
        // a byte array claiming 2 GB
        let huge = [10, 0, 0, 7, 0, 1, b'x', 0x7F, 0xFF, 0xFF, 0xFF, 0];
        assert_eq!(read(&huge), Err(NbtError::UnexpectedEnd));
        // a list of a million ints in a few bytes
        let list = [10, 0, 0, 9, 0, 1, b'x', 3, 0, 0x0F, 0x42, 0x40, 0];
        assert_eq!(read(&list), Err(NbtError::UnexpectedEnd));
        let negative = [10, 0, 0, 11, 0, 1, b'x', 0xFF, 0xFF, 0xFF, 0xFF, 0];
        assert_eq!(read(&negative), Err(NbtError::NegativeLength));
        assert_eq!(read(&[8, 0, 0]), Err(NbtError::RootNotCompound));
        assert_eq!(read(&[10, 0, 0, 13, 0, 0]), Err(NbtError::UnknownTag(13)));
        // compounds nested deeper than the game allows
        let mut deep = vec![10, 0, 0];
        for _ in 0..(MAX_DEPTH + 1) {
            deep.extend_from_slice(&[10, 0, 0]);
        }
        deep.extend(std::iter::repeat_n(0, MAX_DEPTH + 2));
        assert_eq!(read(&deep), Err(NbtError::TooDeep));
    }

    /// LS-88: a list of empty compounds is one byte each to read but a whole `Tag` each in
    /// memory, so 32 MB of them must not cost a gigabyte.
    #[test]
    fn refuses_tags_far_larger_than_the_input() {
        let len: usize = 33_554_000;
        let mut bytes = vec![10, 0, 0, 9, 0, 1, b'x', 10];
        bytes.extend_from_slice(&(len as i32).to_be_bytes());
        bytes.resize(bytes.len() + len, 0);
        bytes.push(0);
        assert_eq!(read(&bytes), Err(NbtError::TooManyTags));
        let packed = gzip(&bytes);
        assert!(packed.len() < 64 * 1024);
        assert!(matches!(
            crate::level::summarize(&packed),
            Err(NbtError::TooManyTags)
        ));

        // The same in compounds of byte tags.
        let mut bytes = vec![10, 0, 0];
        for _ in 0..200_000 {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
        }
        bytes.push(0);
        assert_eq!(read(&bytes), Err(NbtError::TooManyTags));

        // Small files may still hold lists of tiny tags.
        let mut bytes = vec![10, 0, 0, 9, 0, 1, b'x', 1];
        bytes.extend_from_slice(&10_000i32.to_be_bytes());
        bytes.resize(bytes.len() + 10_000, 7);
        bytes.push(0);
        let root = read(&bytes).unwrap();
        assert!(matches!(root.value.get("x"), Some(Tag::List(1, items)) if items.len() == 10_000));
    }
}
