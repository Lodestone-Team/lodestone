//! Deleting what is damaged only on proof, for callers that may meet a passing failure to
//! read: a server answering its players, which runs out of file descriptors under load, or
//! meets an antivirus holding a file on Windows.
//!
//! The store's plain reads ([`Store::read_chunk`], [`Store::object_form`],
//! [`Store::read_object`]) report any failure to open or read an object as
//! [`StoreError::Damaged`], and [`Store::remove_damaged`] reads it again the same way, so a
//! failure that has nothing to do with the bytes (too many open files, permission denied, a
//! sharing violation) deletes a healthy object that backups and other images share. The
//! functions here tell the two apart: damage is an object that was opened and read and
//! whose bytes are wrong (a wrong length or hash, or a zstd frame that doesn't decode),
//! and a missing one is one the drive says isn't there; any other failure to open or read
//! is [`StoreError::Io`], and deletes nothing.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::hex::sha256_hex;
use crate::sha256::Sha256;

use super::chunklist::{ChunkList, ChunkRef, CHUNKED_ABOVE, MAX_LIST_BYTES};
use super::{fsx, hex_digest, io_err, remove_if_there, Encoding, Presence, Store, StoreError};

/// What reading an object file all the way through proved.
enum Proven {
    /// The drive says there is no such file.
    Missing,
    /// It was read and isn't what it should be: not a file, too long, or not a frame that
    /// decodes.
    Damaged,
    /// Its plain bytes, for the caller to check against their hash.
    Bytes(Vec<u8>),
}

/// A file that remembers whether reading it failed, so a decoder's error over it can be
/// told from the drive's.
struct Watched {
    file: File,
    failed: Arc<AtomicBool>,
}

impl Read for Watched {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf).inspect_err(|e| {
            if e.kind() != io::ErrorKind::Interrupted {
                self.failed.store(true, Ordering::Relaxed);
            }
        })
    }
}

/// Reads all plain bytes of the object file at `path`, at most `limit`. An error is a
/// failure to open or read the file itself (not a missing one); anything wrong with what
/// was read is [`Proven::Damaged`].
fn read_proven(encoding: Encoding, path: &Path, limit: u64) -> io::Result<Proven> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Proven::Missing),
        Err(e) => return Err(e),
    };
    if !file.metadata()?.is_file() {
        return Ok(Proven::Damaged);
    }
    let failed = Arc::new(AtomicBool::new(false));
    let watched = Watched {
        file,
        failed: failed.clone(),
    };
    let reader: Box<dyn Read> = match encoding {
        Encoding::Raw => Box::new(watched),
        #[cfg(feature = "zstd")]
        Encoding::Zstd => Box::new(zstd::stream::read::Decoder::new(watched)?.single_frame()),
    };
    let mut bytes = Vec::new();
    match reader.take(limit.saturating_add(1)).read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(e) if failed.load(Ordering::Relaxed) => return Err(e),
        // The drive gave every byte asked for: the decoder refused them.
        Err(_) => return Ok(Proven::Damaged),
    }
    if bytes.len() as u64 > limit {
        return Ok(Proven::Damaged);
    }
    Ok(Proven::Bytes(bytes))
}

/// How many plain bytes the object file at `path` holds, from its length (raw) or its
/// frame header (zstd): `None` when it is missing or its header doesn't say (damaged). An
/// error is a failure to open or read it.
fn proven_len(encoding: Encoding, path: &Path) -> io::Result<Option<u64>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Ok(None);
    }
    match encoding {
        Encoding::Raw => Ok(Some(meta.len())),
        #[cfg(feature = "zstd")]
        Encoding::Zstd => {
            // A frame header is at most 18 bytes.
            let mut header = Vec::with_capacity(18);
            file.take(18).read_to_end(&mut header)?;
            Ok(zstd::zstd_safe::get_frame_content_size(&header)
                .ok()
                .flatten())
        }
    }
}

impl Store {
    /// [`Store::read_chunk`], except that a failure to open or read the chunk's file (other
    /// than its being missing) is [`StoreError::Io`], not [`StoreError::Damaged`]: damaged
    /// means its bytes were read and are wrong.
    pub fn read_chunk_checked(&self, chunk: &ChunkRef) -> Result<Vec<u8>, StoreError> {
        let path = self.chunk_path(&chunk.sha256)?;
        let damaged = || StoreError::Damaged(chunk.sha256.clone());
        match read_proven(self.encoding, &path, u64::from(chunk.size))
            .map_err(io_err("Failed to read a chunk"))?
        {
            Proven::Missing => Err(StoreError::Missing(chunk.sha256.clone())),
            Proven::Damaged => Err(damaged()),
            Proven::Bytes(bytes)
                if bytes.len() == chunk.size as usize && sha256_hex(&bytes) == chunk.sha256 =>
            {
                Ok(bytes)
            }
            Proven::Bytes(_) => Err(damaged()),
        }
    }

    /// [`Store::remove_damaged`], deleting only on proof (see the module): the whole object
    /// when it was read and doesn't hash to its name, the chunk list when it was read and
    /// isn't a list or its chunks, all read, don't hash to its name. A failure to open or
    /// read the object, the list or a chunk is [`StoreError::Io`] and deletes nothing; a
    /// missing or damaged chunk is that chunk's error (see
    /// [`Store::remove_damaged_chunk_checked`]). Returns whether it deleted anything.
    pub fn remove_damaged_checked(&self, sha256: &str) -> Result<bool, StoreError> {
        self.remove_damaged_proven(sha256, true)
    }

    /// [`Store::remove_damaged_checked`] without reading a chunk list's chunks: the whole
    /// object is deleted when it was read and doesn't hash to its name, the chunk list when
    /// it was read and isn't a list. A list that parses is left whatever its chunks hold, so
    /// this costs at most the list's read, where a full check of a file of a few GiB reads,
    /// decompresses and hashes all of it; whoever reads the chunks later checks each and
    /// deletes on proof ([`Store::remove_damaged_chunk_checked`],
    /// [`Store::remove_incomplete_checked`]). Returns whether it deleted anything.
    pub fn remove_damaged_shallow_checked(&self, sha256: &str) -> Result<bool, StoreError> {
        self.remove_damaged_proven(sha256, false)
    }

    fn remove_damaged_proven(&self, sha256: &str, chunks: bool) -> Result<bool, StoreError> {
        let mut removed = false;
        let path = self.object_path(sha256)?;
        // The store keeps only files of up to 1 MiB whole: a longer one isn't its file.
        match read_proven(self.encoding, &path, CHUNKED_ABOVE)
            .map_err(io_err("Failed to read a stored object"))?
        {
            Proven::Missing => {}
            Proven::Bytes(bytes) if sha256_hex(&bytes) == sha256 => {}
            Proven::Damaged | Proven::Bytes(_) => removed |= remove_if_there(&path)?,
        }
        let path = self.list_path(sha256)?;
        let list = match read_proven(self.encoding, &path, MAX_LIST_BYTES)
            .map_err(io_err("Failed to read a chunk list"))?
        {
            Proven::Missing => return Ok(removed),
            Proven::Damaged => None,
            Proven::Bytes(bytes) => ChunkList::parse(&bytes).ok(),
        };
        let healthy = match list {
            Some(_) if !chunks => true,
            Some(list) => {
                let mut hasher = Sha256::new();
                for chunk in &list.chunks {
                    hasher.update(&self.read_chunk_checked(chunk)?);
                }
                hex_digest(hasher) == sha256
            }
            None => false,
        };
        if !healthy {
            removed |= remove_if_there(&path)?;
        }
        Ok(removed)
    }

    /// [`Store::remove_damaged_chunk`], deleting only on proof (see the module): the chunk
    /// was read and its bytes don't match its hash and size. A failure to open or read it
    /// is [`StoreError::Io`]. Returns whether it deleted the chunk.
    pub fn remove_damaged_chunk_checked(&self, chunk: &ChunkRef) -> Result<bool, StoreError> {
        match self.read_chunk_checked(chunk) {
            Err(StoreError::Damaged(_)) => {}
            Err(e @ StoreError::Io { .. }) => return Err(e),
            _ => return Ok(false),
        }
        let path = self.chunk_path(&chunk.sha256)?;
        match fsx::remove_file_forced(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err("Failed to delete a damaged chunk")(e)),
        }
    }

    /// Deletes the chunk list of a large file of `size` bytes when the file can't be read
    /// whole from it, deleting only on proof (see the module): when one of its chunks is
    /// missing (one deleted by [`Store::remove_damaged_chunk_checked`], say) or its length
    /// (or zstd frame header) shows it isn't the chunk. [`Store::has_object`] and
    /// [`Store::object_form`] look only at the list, so while it is there they still find
    /// the object, and whoever checks only those would never store the file again; without
    /// the list the object is plainly missing, so the next write of the file stores it (and
    /// writes the lost chunk back). The chunks are left: other files may share them. A small
    /// file or a list for another size is left as it is. A failure to open or read a
    /// chunk is [`StoreError::Io`] and deletes nothing; a list that can't be read now is left
    /// as it is. Returns whether it deleted the list.
    pub fn remove_incomplete_checked(&self, sha256: &str, size: u64) -> Result<bool, StoreError> {
        let Some(Presence::Chunked(path, list)) = self.presence(sha256, size) else {
            return Ok(false);
        };
        for chunk in &list.chunks {
            let chunk_path = self.chunk_path(&chunk.sha256)?;
            let len = proven_len(self.encoding, &chunk_path)
                .map_err(io_err("Failed to look at a chunk"))?;
            if len != Some(u64::from(chunk.size)) {
                return remove_if_there(&path);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path) -> Store {
        #[cfg(feature = "zstd")]
        let encoding = Encoding::Zstd;
        #[cfg(not(feature = "zstd"))]
        let encoding = Encoding::Raw;
        Store::new(dir, encoding)
    }

    /// Bytes that don't repeat, so the chunker cuts a large file into several chunks.
    fn noise(len: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    fn overwrite(path: &Path, bytes: &[u8]) {
        fsx::remove_file_forced(path).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn damage_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let small = noise(50_000, 1);
        let small_hash = store.put_bytes(&small).unwrap().sha256;
        let large = noise(3 << 20, 2);
        let large_hash = store.put_bytes(&large).unwrap().sha256;
        let Ok(super::super::ObjectForm::Chunked(list)) =
            store.object_form(&large_hash, large.len() as u64)
        else {
            panic!("a chunk list");
        };
        assert!(list.chunks.len() > 1);

        // healthy: nothing is deleted
        assert!(!store.remove_damaged_checked(&small_hash).unwrap());
        assert!(!store.remove_damaged_checked(&large_hash).unwrap());
        assert!(!store.remove_damaged_chunk_checked(&list.chunks[0]).unwrap());
        assert_eq!(store.read_chunk_checked(&list.chunks[0]).unwrap().len(), {
            list.chunks[0].size as usize
        });
        assert!(!store
            .remove_incomplete_checked(&large_hash, large.len() as u64)
            .unwrap());

        // a damaged whole object: other bytes, or not a frame at all
        let path = store.object_path(&small_hash).unwrap();
        overwrite(&path, b"not the object");
        assert!(store.remove_damaged_checked(&small_hash).unwrap());
        assert!(!path.exists());
        assert!(!store.remove_damaged_checked(&small_hash).unwrap());

        // a damaged chunk is deleted, then the list that lost it
        let chunk = &list.chunks[1];
        let chunk_path = store.chunk_path(&chunk.sha256).unwrap();
        overwrite(&chunk_path, b"not the chunk");
        assert!(matches!(
            store.read_chunk_checked(chunk),
            Err(StoreError::Damaged(_))
        ));
        assert!(matches!(
            store.remove_damaged_checked(&large_hash),
            Err(StoreError::Damaged(_))
        ));
        // the shallow check reads only the list, which is fine, and leaves it
        assert!(!store.remove_damaged_shallow_checked(&large_hash).unwrap());
        assert!(store.list_path(&large_hash).unwrap().exists());
        assert!(store.remove_damaged_chunk_checked(chunk).unwrap());
        assert!(matches!(
            store.read_chunk_checked(chunk),
            Err(StoreError::Missing(_))
        ));
        assert!(!store
            .remove_incomplete_checked(&large_hash, large.len() as u64 + 1)
            .unwrap());
        assert!(store
            .remove_incomplete_checked(&large_hash, large.len() as u64)
            .unwrap());
        assert!(!store.has_object(&large_hash, large.len() as u64));
        // the chunks other files may share are left
        assert!(store.chunk_path(&list.chunks[0].sha256).unwrap().exists());

        // a damaged chunk list
        store.put_bytes(&large).unwrap();
        let list_path = store.list_path(&large_hash).unwrap();
        overwrite(&list_path, b"not a list");
        assert!(store.remove_damaged_checked(&large_hash).unwrap());
        assert!(!list_path.exists());
        // ...which the shallow check deletes too, and a whole object that doesn't hash
        store.put_bytes(&large).unwrap();
        overwrite(&list_path, b"not a list");
        assert!(store.remove_damaged_shallow_checked(&large_hash).unwrap());
        assert!(!list_path.exists());
        assert!(!store.remove_damaged_shallow_checked(&large_hash).unwrap());
        store.put_bytes(&small).unwrap();
        overwrite(&path, b"not the object");
        assert!(store.remove_damaged_shallow_checked(&small_hash).unwrap());
        assert!(!path.exists());
    }

    /// A file that can't be opened (as when the process is out of file descriptors, or an
    /// antivirus holds it) is no proof of damage: nothing is deleted.
    #[cfg(unix)]
    #[test]
    fn a_file_that_cant_be_read_is_kept() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let small = noise(50_000, 3);
        let small_hash = store.put_bytes(&small).unwrap().sha256;
        let large = noise(3 << 20, 4);
        let large_hash = store.put_bytes(&large).unwrap().sha256;
        let Ok(super::super::ObjectForm::Chunked(list)) =
            store.object_form(&large_hash, large.len() as u64)
        else {
            panic!("a chunk list");
        };
        let chunk = &list.chunks[1];
        let object = store.object_path(&small_hash).unwrap();
        let chunk_path = store.chunk_path(&chunk.sha256).unwrap();
        let lock = |path: &Path, mode: u32| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap()
        };
        lock(&object, 0o000);
        lock(&chunk_path, 0o000);
        if File::open(&object).is_ok() {
            // Running as root, which reads anything: nothing to test.
            lock(&object, 0o444);
            lock(&chunk_path, 0o444);
            return;
        }
        // the plain reads take it for damage...
        assert!(matches!(
            store.read_chunk(chunk),
            Err(StoreError::Damaged(_))
        ));
        // ...the checked ones don't, and delete nothing
        assert!(matches!(
            store.read_chunk_checked(chunk),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            store.remove_damaged_checked(&small_hash),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            store.remove_damaged_chunk_checked(chunk),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            store.remove_damaged_checked(&large_hash),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            store.remove_damaged_shallow_checked(&small_hash),
            Err(StoreError::Io { .. })
        ));
        assert!(matches!(
            store.remove_incomplete_checked(&large_hash, large.len() as u64),
            Err(StoreError::Io { .. })
        ));
        lock(&object, 0o444);
        lock(&chunk_path, 0o444);
        assert_eq!(store.read_object_bytes(&small_hash, 50_000).unwrap(), small);
        assert!(store.object_complete(&large_hash, large.len() as u64));
        assert_eq!(store.read_chunk_checked(chunk).unwrap().len(), {
            chunk.size as usize
        });
    }
}
