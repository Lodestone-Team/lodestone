//! Content-defined chunking (FastCDC), so large files that change in place share most of
//! their bytes between images.
//!
//! A region file is 1 to 10 MB, and Minecraft rewrites one chunk of it in place or moves
//! only that chunk; cutting at fixed offsets would still share most pieces, but an
//! insertion would shift every later piece. Content-defined cut points depend only on the
//! bytes around them, so after a change the cuts fall where they fell before and every
//! piece away from the change is stored once.
//!
//! This is FastCDC (Xia et al., "FastCDC: a Fast and Efficient Content-Defined Chunking
//! Approach for Data Deduplication", USENIX ATC 2016) with its cut-point skipping and
//! normalized chunking at level 2:
//!
//! - A rolling "gear" fingerprint, `fp = (fp << 1) + GEAR[byte]`, so bit `k` of `fp`
//!   depends on the last `k + 1` bytes only.
//! - No cut before [`MIN_CHUNK`] bytes (those bytes aren't even hashed), and a cut at
//!   [`MAX_CHUNK`] when none came sooner.
//! - Before [`AVG_CHUNK`] a cut needs [`MASK_S`]'s 18 bits of `fp` to be zero (harder),
//!   after it [`MASK_L`]'s 14 bits (easier), which keeps chunks near the average.
//!
//! Everything is fixed and written out here, so the same bytes give the same chunks on
//! every computer and in every version that reads `lodestone-chunks 1` lists: the gear
//! table comes from SplitMix64 with a fixed seed ([`GEAR_SEED`]), the masks spread their
//! bits over the fingerprint's top 48 bits, and the test `boundaries_are_pinned` fails if
//! any of it changes. Changing them would only cost deduplication against older chunks,
//! never correctness: chunk lists say where their chunks are.

/// No chunk is shorter than this, except a file's last.
pub const MIN_CHUNK: usize = 16 * 1024;
/// Where the cut rule switches from the strict mask to the loose one.
pub const AVG_CHUNK: usize = 64 * 1024;
/// No chunk is longer than this.
pub const MAX_CHUNK: usize = 256 * 1024;

/// The seed of the gear table: "Lodeston" in ASCII.
pub const GEAR_SEED: u64 = 0x4c6f_6465_7374_6f6e;

/// The cut rule before [`AVG_CHUNK`]: 18 bits, so a cut is 4 times less likely per byte
/// than at the average.
pub const MASK_S: u64 = spread_mask(18);
/// The cut rule after [`AVG_CHUNK`]: 14 bits, 4 times more likely.
pub const MASK_L: u64 = spread_mask(14);

/// One random 64-bit value per byte value, from SplitMix64 seeded with [`GEAR_SEED`]:
/// `GEAR[i]` is the `i + 1`th output.
pub static GEAR: [u64; 256] = gear_table();

/// SplitMix64's step: the next state and its output.
const fn splitmix64(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (state, z ^ (z >> 31))
}

const fn gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut state = GEAR_SEED;
    let mut i = 0;
    while i < 256 {
        let (next, value) = splitmix64(state);
        state = next;
        table[i] = value;
        i += 1;
    }
    table
}

/// A mask of `bits` one bits spread evenly from bit 63 down (every `48 / bits`th bit), so
/// each one depends on a window of at least 25 bytes.
const fn spread_mask(bits: u32) -> u64 {
    let step = 48 / bits;
    let mut mask = 0u64;
    let mut i = 0;
    while i < bits {
        mask |= 1u64 << (63 - i * step);
        i += 1;
    }
    mask
}

/// The length of the first chunk of `data`, when `data` is everything left of the file or
/// at least [`MAX_CHUNK`] bytes of it. (With less than that, and more to come, a cut found
/// later could be the right one: [`Chunker`] only asks with enough bytes.)
pub fn cut_point(data: &[u8]) -> usize {
    let len = data.len();
    if len <= MIN_CHUNK {
        return len;
    }
    let end = len.min(MAX_CHUNK);
    let normal = AVG_CHUNK.min(end);
    let mut fp: u64 = 0;
    let mut i = MIN_CHUNK;
    while i < normal {
        fp = (fp << 1).wrapping_add(GEAR[usize::from(data[i])]);
        if fp & MASK_S == 0 {
            return i + 1;
        }
        i += 1;
    }
    while i < end {
        fp = (fp << 1).wrapping_add(GEAR[usize::from(data[i])]);
        if fp & MASK_L == 0 {
            return i + 1;
        }
        i += 1;
    }
    end
}

/// Cuts a stream into chunks as bytes arrive, whatever sizes they arrive in: the chunks
/// are the same as cutting the whole file at once.
#[derive(Debug, Default)]
pub struct Chunker {
    buffer: Vec<u8>,
    start: usize,
}

impl Chunker {
    pub fn new() -> Chunker {
        Chunker::default()
    }

    /// Adds `data`, calling `emit` with each chunk it completes.
    pub fn push<E>(
        &mut self,
        data: &[u8],
        emit: &mut dyn FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        // Drop what was emitted once it is most of the buffer, so the buffer stays near
        // MAX_CHUNK plus one read without moving bytes for every chunk.
        if self.start > 0 && self.start >= self.buffer.len() / 2 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
        self.buffer.extend_from_slice(data);
        while self.buffer.len() - self.start >= MAX_CHUNK {
            let rest = &self.buffer[self.start..];
            let cut = cut_point(rest);
            emit(&rest[..cut])?;
            self.start += cut;
        }
        Ok(())
    }

    /// Emits the chunks left at the end of the file.
    pub fn finish<E>(mut self, emit: &mut dyn FnMut(&[u8]) -> Result<(), E>) -> Result<(), E> {
        while self.start < self.buffer.len() {
            let rest = &self.buffer[self.start..];
            let cut = cut_point(rest);
            emit(&rest[..cut])?;
            self.start += cut;
        }
        Ok(())
    }
}

/// The chunk lengths of `data`, cut as a whole.
pub fn chunk_lengths(data: &[u8]) -> Vec<usize> {
    let mut lengths = Vec::new();
    let mut rest = data;
    while !rest.is_empty() {
        let cut = cut_point(rest);
        lengths.push(cut);
        rest = &rest[cut..];
    }
    lengths
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::hex::sha256_hex;

    /// A small seeded generator for test data (xorshift64*), so tests need no crate and
    /// give the same bytes everywhere.
    pub(crate) struct TestRng(u64);

    impl TestRng {
        pub(crate) fn new(seed: u64) -> TestRng {
            TestRng(seed.max(1))
        }

        pub(crate) fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        pub(crate) fn below(&mut self, n: u64) -> u64 {
            self.next_u64() % n.max(1)
        }

        pub(crate) fn bytes(&mut self, len: usize) -> Vec<u8> {
            let mut out = Vec::with_capacity(len + 8);
            while out.len() < len {
                out.extend_from_slice(&self.next_u64().to_le_bytes());
            }
            out.truncate(len);
            out
        }
    }

    /// Bytes shaped like a region file: 4 KiB sectors, each chunk's data compressed (so
    /// random-looking), with runs of zero padding between them.
    pub(crate) fn region_like(seed: u64, len: usize) -> Vec<u8> {
        let mut rng = TestRng::new(seed);
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let sectors = 1 + rng.below(6) as usize;
            let used = sectors * 4096 - rng.below(4000) as usize;
            out.extend(rng.bytes(used));
            out.resize(out.len() + (sectors * 4096 - used), 0);
        }
        out.truncate(len);
        out
    }

    #[test]
    fn constants_are_what_the_docs_say() {
        assert_eq!(MASK_S.count_ones(), 18);
        assert_eq!(MASK_L.count_ones(), 14);
        assert_eq!(MASK_S, 0xaaaa_aaaa_a000_0000);
        assert_eq!(MASK_L, 0x9249_2492_4900_0000);
        // Pins the table: its first and last values.
        assert_eq!(GEAR[0], PINNED_GEAR_FIRST, "{:#018x}", GEAR[0]);
        assert_eq!(GEAR[255], PINNED_GEAR_LAST, "{:#018x}", GEAR[255]);
        let distinct: std::collections::HashSet<u64> = GEAR.iter().copied().collect();
        assert_eq!(distinct.len(), 256);
    }

    #[test]
    fn short_inputs_are_one_chunk() {
        assert_eq!(chunk_lengths(&[]), Vec::<usize>::new());
        assert_eq!(chunk_lengths(&[1, 2, 3]), vec![3]);
        assert_eq!(chunk_lengths(&vec![7; MIN_CHUNK]), vec![MIN_CHUNK]);
    }

    #[test]
    fn chunks_respect_the_bounds() {
        let data = TestRng::new(1).bytes(20 << 20);
        let lengths = chunk_lengths(&data);
        assert_eq!(lengths.iter().sum::<usize>(), data.len());
        let (last, rest) = lengths.split_last().unwrap();
        assert!(rest.iter().all(|&l| (MIN_CHUNK..=MAX_CHUNK).contains(&l)));
        assert!(*last <= MAX_CHUNK);
        let mean = data.len() / lengths.len();
        assert!(
            (32 * 1024..=96 * 1024).contains(&mean),
            "mean chunk {mean} bytes"
        );
        // A run of zeros settles the fingerprint on one value, so cuts come at a fixed
        // spacing or at the maximum: either way chunks stay bounded.
        let zeros = vec![0u8; 3 << 20];
        let lengths = chunk_lengths(&zeros);
        assert!(lengths.iter().all(|&l| l <= MAX_CHUNK));
        assert_eq!(lengths.iter().sum::<usize>(), zeros.len());
    }

    #[test]
    fn streaming_gives_the_same_chunks_whatever_the_read_sizes() {
        let data = region_like(2, 5 << 20);
        let whole = chunk_lengths(&data);
        for seed in 1..6u64 {
            let mut rng = TestRng::new(seed);
            let mut chunker = Chunker::new();
            let mut got = Vec::new();
            let mut emit = |chunk: &[u8]| -> Result<(), ()> {
                got.push(chunk.len());
                Ok(())
            };
            let mut at = 0;
            while at < data.len() {
                let n = (1 + rng.below(700_000) as usize).min(data.len() - at);
                chunker.push(&data[at..at + n], &mut emit).unwrap();
                at += n;
            }
            chunker.finish(&mut emit).unwrap();
            assert_eq!(got, whole, "read sizes from seed {seed}");
        }
    }

    /// The chunk hashes of `data`.
    fn chunk_hashes(data: &[u8]) -> Vec<String> {
        let mut at = 0;
        chunk_lengths(data)
            .into_iter()
            .map(|len| {
                let hash = sha256_hex(&data[at..at + len]);
                at += len;
                hash
            })
            .collect()
    }

    #[test]
    fn a_rewritten_block_changes_only_the_chunks_near_it() {
        let original = region_like(3, 10_000_000);
        let before = chunk_hashes(&original);
        for (seed, block) in [(10u64, 37usize), (11, 1200), (12, 2400)] {
            let mut changed = original.clone();
            let at = block * 4096;
            changed[at..at + 4096].copy_from_slice(&TestRng::new(seed).bytes(4096));
            let after = chunk_hashes(&changed);
            let old: std::collections::HashSet<&String> = before.iter().collect();
            let new: Vec<&String> = after.iter().filter(|h| !old.contains(h)).collect();
            assert!(
                (1..=2).contains(&new.len()),
                "{} new chunks for one rewritten 4 KiB block",
                new.len()
            );
            assert!(after.len().abs_diff(before.len()) <= 1);
        }
    }

    #[test]
    fn an_insertion_resynchronizes() {
        let original = region_like(4, 10_000_000);
        let before: std::collections::HashSet<String> =
            chunk_hashes(&original).into_iter().collect();
        let mut changed = original.clone();
        // A chunk moved: 8 KiB inserted in the middle of the file shifts everything after.
        let insert = TestRng::new(5).bytes(8192);
        changed.splice(5_000_000..5_000_000, insert);
        let after = chunk_hashes(&changed);
        let new = after.iter().filter(|h| !before.contains(*h)).count();
        assert!(new <= 3, "{new} new chunks after an 8 KiB insertion");
        let shared_bytes: usize = {
            let mut at = 0;
            chunk_lengths(&changed)
                .into_iter()
                .filter(|&len| {
                    let shared = before.contains(&sha256_hex(&changed[at..at + len]));
                    at += len;
                    shared
                })
                .sum()
        };
        assert!(shared_bytes * 100 / changed.len() >= 95);
    }

    #[test]
    fn boundaries_are_pinned() {
        // The chunk lengths of a fixed input. If this changes, the gear table, the masks or
        // the cut rule changed, and new chunks no longer line up with stored ones.
        let data = TestRng::new(42).bytes(2 << 20);
        let lengths = chunk_lengths(&data);
        let joined: Vec<String> = lengths.iter().map(usize::to_string).collect();
        assert_eq!(
            sha256_hex(joined.join(",").as_bytes()),
            PINNED_BOUNDARIES,
            "chunk lengths now: {joined:?}"
        );
    }

    // Worked out by an independent implementation (a few lines of Python following the
    // module documentation), not by this code.
    const PINNED_BOUNDARIES: &str =
        "1f6ea1455608cefa6aa3c41edb8386a45189f8da7fba9c57fc71cfa966e08f3f";
    const PINNED_GEAR_FIRST: u64 = 0xe846_d404_4312_2dfe;
    const PINNED_GEAR_LAST: u64 = 0xf626_e08d_cad9_2c61;
}
