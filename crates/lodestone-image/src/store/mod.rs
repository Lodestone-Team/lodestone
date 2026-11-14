//! The store: content-addressed objects shared by every instance on one machine, and the
//! manifests of their images.
//!
//! ```text
//! <root>/
//!   objects/ab/<sha256>              whole files (up to 1 MiB, and large files the core
//!                                    stored whole), or chunk lists of larger files
//!   chunks/ab/<sha256>               the chunks of large files (see `cdc`, `chunklist`)
//!   images/<instance>/<id>.json      one manifest per image
//!   images/<instance>/stat-cache.json   what the last snapshot read (`statcache`)
//!   objects/.incoming-<16 hex>.tmp   writes in flight (also in chunks/ and images/…/),
//!                                    swept once an hour old
//! ```
//!
//! Objects are named by the SHA-256 of the file they stand for; how their bytes sit on
//! disk is the store's [`Encoding`]. Every write goes to a temporary file that is flushed,
//! made read-only and renamed into place, and a manifest is written only after the objects
//! it uses (whose folders are flushed first), so a crash never leaves a manifest pointing
//! at missing bytes or a name holding part of a file. Every read checks the SHA-256 (and
//! size) of what comes out, so damage on disk is an error, never wrong bytes.
//!
//! The parts:
//! - [`cdc`] and [`chunklist`]: how large files are cut into chunks and listed.
//! - [`statcache`]: skipping files that haven't changed since the last snapshot.
//! - [`snapshot`]: walking a folder into an image's entries.
//! - [`materialize`]: placing an image's files into a staging folder.
//! - [`gc`]: deleting what no manifest uses.
//! - [`excludes`]: an image's exclude patterns.
//!
//! Unlike the rest of the crate, this module reads and writes files. Everything here
//! blocks; callers run it on their own threads.

pub mod cdc;
pub mod chunklist;
pub mod encoding;
pub mod excludes;
pub(crate) mod fsx;
pub mod gc;
pub mod materialize;
pub mod snapshot;
pub mod statcache;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::hex::{is_sha256, sha256_hex};
use crate::identity::IdentityError;
use crate::manifest::{Image, ManifestError};
use crate::paths::is_reserved_windows_name;

use cdc::Chunker;
use chunklist::{ChunkList, ChunkRef, CHUNKED_ABOVE};
pub use encoding::Encoding;
use encoding::{buffer_len, decoded_len, decoder, encode_into, read_limited};
pub use statcache::StatCache;

/// Temporary files older than this were left by a crash and are swept.
pub const TEMP_MAX_AGE: Duration = Duration::from_secs(3600);
/// A manifest is at most this long (a million entries fit).
pub const MAX_MANIFEST_BYTES: u64 = 512 << 20;
/// Instance ids are at most this long.
pub const MAX_INSTANCE_ID: usize = 100;

/// An object reused by a write is touched when it is older than this, so a garbage
/// collection keeps it for the rest of its hour of grace ([`gc`]).
const TOUCH_AFTER: Duration = Duration::from_secs(600);
const STAT_CACHE_FILE: &str = "stat-cache.json";

/// Asked between pieces of work; true stops it with [`StoreError::Cancelled`].
pub type Cancel<'a> = &'a (dyn Fn() -> bool + Sync);
/// A [`Cancel`] that never cancels.
pub const NEVER: Cancel<'static> = &|| false;

/// Why a store operation failed.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Reading or writing the store failed.
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
    /// The file being stored couldn't be opened or read (locked, gone, permission denied).
    #[error("the file can't be read: {0}")]
    Source(#[source] io::Error),
    /// The file being stored changed while it was read.
    #[error("the file changed while it was read")]
    Changed,
    /// There is no object (or chunk) with this hash.
    #[error("the store has no object {0}")]
    Missing(String),
    /// The object is there but its bytes aren't what its name says.
    #[error("the stored object {0} is damaged")]
    Damaged(String),
    #[error("{0:?} isn't a SHA-256")]
    BadHash(String),
    #[error("{0:?} isn't a valid instance id")]
    BadInstance(String),
    /// Bytes given to store didn't hash to what the caller expected.
    #[error("the contents hash to {found}, not {expected}")]
    HashMismatch { expected: String, found: String },
    /// Bytes given to store were more or fewer than the caller said.
    #[error("the contents have {found} bytes, not {expected}")]
    SizeMismatch { expected: u64, found: u64 },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("cancelled")]
    Cancelled,
}

fn io_err(what: impl Into<String>) -> impl FnOnce(io::Error) -> StoreError {
    let what = what.into();
    move |source| StoreError::Io { what, source }
}

/// The two folders of content-addressed files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Area {
    Objects,
    Chunks,
}

impl Area {
    fn folder(self) -> &'static str {
        match self {
            Area::Objects => "objects",
            Area::Chunks => "chunks",
        }
    }
}

/// An object stored by [`Store::put_bytes`], [`Store::put_reader`] or
/// [`ObjectWriter::finish`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub sha256: String,
    pub size: u64,
    /// Bytes this write added to the store (encoded): 0 when everything was there.
    pub added_bytes: u64,
}

/// A file stored by [`Store::put_file`], with what its metadata said when it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    pub sha256: String,
    pub size: u64,
    /// Unix nanoseconds.
    pub modified_ns: i64,
    /// The permission bits an entry records ([`crate::Entry::mode`]).
    pub mode: u32,
    pub added_bytes: u64,
}

/// How an object holds its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectForm {
    /// The file's bytes.
    Whole,
    /// The list of the file's chunks.
    Chunked(ChunkList),
}

/// What [`Store::list_manifests`] found.
#[derive(Debug, Default)]
pub struct ManifestListing {
    /// Newest first (by `created`, then id).
    pub images: Vec<Image>,
    /// Manifests that can't be read, by id, with why.
    pub unreadable: Vec<(String, String)>,
}

/// A store at a folder, with one encoding. Cheap to make; nothing is read or created until
/// used. Shared between threads freely: writes of the same object at once are safe.
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    encoding: Encoding,
    /// Folders this value wrote objects into since the last manifest, flushed (on unix)
    /// before the next manifest is written.
    unsynced: Mutex<BTreeSet<(Area, String)>>,
}

/// Whether `id` can name an instance's folder: 1 to 100 ASCII letters, digits, `-` and
/// `_`, and not a Windows device name (`CON`, `NUL`, `COM1`…).
pub fn valid_instance_id(id: &str) -> bool {
    (1..=MAX_INSTANCE_ID).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && !is_reserved_windows_name(id)
}

impl Store {
    pub fn new(root: impl Into<PathBuf>, encoding: Encoding) -> Store {
        Store {
            root: root.into(),
            encoding,
            unsynced: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    fn area_dir(&self, area: Area) -> PathBuf {
        self.root.join(area.folder())
    }

    fn area_path(&self, area: Area, sha256: &str) -> Result<PathBuf, StoreError> {
        if !is_sha256(sha256) {
            return Err(StoreError::BadHash(sha256.to_string()));
        }
        Ok(self.area_dir(area).join(&sha256[..2]).join(sha256))
    }

    /// Where the object for `sha256` is (whether or not it is there).
    pub fn object_path(&self, sha256: &str) -> Result<PathBuf, StoreError> {
        self.area_path(Area::Objects, sha256)
    }

    /// Where the chunk `sha256` is (whether or not it is there).
    pub fn chunk_path(&self, sha256: &str) -> Result<PathBuf, StoreError> {
        self.area_path(Area::Chunks, sha256)
    }

    /// The folder of every instance's manifests.
    pub fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    /// The folder of one instance's manifests.
    pub fn instance_dir(&self, instance: &str) -> Result<PathBuf, StoreError> {
        if !valid_instance_id(instance) {
            return Err(StoreError::BadInstance(instance.to_string()));
        }
        Ok(self.images_dir().join(instance))
    }

    pub fn manifest_path(&self, instance: &str, id: &str) -> Result<PathBuf, StoreError> {
        if !is_sha256(id) {
            return Err(StoreError::BadHash(id.to_string()));
        }
        Ok(self.instance_dir(instance)?.join(format!("{id}.json")))
    }

    // -----------------------------------------------------------------------------------
    // Objects

    /// Whether the object for a file of `size` bytes with this hash looks present: there,
    /// and as long as the file (whole) or as a chunk list for it can be. Reading it all is
    /// left to reads, which check every byte.
    pub fn has_object(&self, sha256: &str, size: u64) -> bool {
        let Ok(path) = self.object_path(sha256) else {
            return false;
        };
        match decoded_len(self.encoding, &path) {
            Some(len) if len == size => true,
            Some(len) => size > CHUNKED_ABOVE && len > 0 && len <= ChunkList::max_encoded_len(size),
            None => false,
        }
    }

    /// How the object for a file of `size` bytes holds it. Reads and checks the chunk list
    /// of a large file, not its chunks.
    pub fn object_form(&self, sha256: &str, size: u64) -> Result<ObjectForm, StoreError> {
        let path = self.object_path(sha256)?;
        let damaged = || StoreError::Damaged(sha256.to_string());
        let Some(len) = decoded_len(self.encoding, &path) else {
            return Err(if fs::symlink_metadata(&path).is_ok() {
                damaged()
            } else {
                StoreError::Missing(sha256.to_string())
            });
        };
        if len == size {
            return Ok(ObjectForm::Whole);
        }
        if size <= CHUNKED_ABOVE || len > ChunkList::max_encoded_len(size) {
            return Err(damaged());
        }
        let bytes = read_limited(self.encoding, &path, len).map_err(|_| damaged())?;
        let list = ChunkList::parse(&bytes).map_err(|_| damaged())?;
        if list.size != size {
            return Err(damaged());
        }
        Ok(ObjectForm::Chunked(list))
    }

    /// Starts storing a file of `size` bytes given in pieces. When `expected` is given, the
    /// object is only kept if the bytes hash to it.
    pub fn writer(
        &self,
        size: u64,
        expected: Option<&str>,
    ) -> Result<ObjectWriter<'_>, StoreError> {
        if let Some(hash) = expected {
            if !is_sha256(hash) {
                return Err(StoreError::BadHash(hash.to_string()));
            }
        }
        let state = if size > CHUNKED_ABOVE {
            WriterState::Chunked {
                chunker: Chunker::new(),
                chunks: Vec::new(),
            }
        } else {
            // At most 1 MiB, by the check above.
            WriterState::Whole(Vec::with_capacity(usize::try_from(size).unwrap_or(0)))
        };
        Ok(ObjectWriter {
            store: self,
            size,
            expected: expected.map(str::to_string),
            received: 0,
            hasher: Sha256::new(),
            state,
            added: 0,
        })
    }

    /// Stores `bytes`.
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<Stored, StoreError> {
        let mut writer = self.writer(bytes.len() as u64, None)?;
        writer.push(bytes)?;
        writer.finish()
    }

    /// Stores what `reader` gives, which must be exactly `size` bytes (and hash to
    /// `expected`, if given). Errors reading are [`StoreError::Source`].
    pub fn put_reader(
        &self,
        reader: &mut dyn Read,
        size: u64,
        expected: Option<&str>,
        cancel: Cancel,
    ) -> Result<Stored, StoreError> {
        let mut writer = self.writer(size, expected)?;
        let mut buffer = vec![0u8; buffer_len(size)];
        loop {
            if cancel() {
                return Err(StoreError::Cancelled);
            }
            let n = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(StoreError::Source(e)),
            };
            writer.push(&buffer[..n])?;
        }
        writer.finish()
    }

    /// Stores the regular file at `path`, which may be in use: if its size or modification
    /// time changes while it is read, this fails with [`StoreError::Changed`] (callers try
    /// again). On macOS a raw store clones small files from `path` (APFS shares their
    /// blocks) and checks the clone's hash before keeping it.
    pub fn put_file(&self, path: &Path, cancel: Cancel) -> Result<StoredFile, StoreError> {
        let mut file = File::open(path).map_err(StoreError::Source)?;
        let before = file.metadata().map_err(StoreError::Source)?;
        if !before.is_file() {
            return Err(StoreError::Source(io::Error::other("not a regular file")));
        }
        let size = before.len();
        let (sha256, added_bytes) = if size <= CHUNKED_ABOVE {
            let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
            (&mut file)
                .take(size + 1)
                .read_to_end(&mut bytes)
                .map_err(StoreError::Source)?;
            if bytes.len() as u64 != size {
                return Err(StoreError::Changed);
            }
            let sha256 = sha256_hex(&bytes);
            unchanged_since(path, &before)?;
            let added = self.write_whole(&sha256, &bytes, Some(path))?;
            (sha256, added)
        } else {
            let stored = match self.put_reader(&mut file, size, None, cancel) {
                Err(StoreError::SizeMismatch { .. }) => return Err(StoreError::Changed),
                other => other?,
            };
            unchanged_since(path, &before)?;
            (stored.sha256, stored.added_bytes)
        };
        Ok(StoredFile {
            sha256,
            size,
            modified_ns: fsx::modified_ns(&before),
            mode: fsx::mode_of(&before),
            added_bytes,
        })
    }

    /// Stores a whole file's bytes under `sha256` (their hash), cloning them from `source`
    /// where that is possible and checks out.
    fn write_whole(
        &self,
        sha256: &str,
        bytes: &[u8],
        source: Option<&Path>,
    ) -> Result<u64, StoreError> {
        let path = self.area_path(Area::Objects, sha256)?;
        if self.reuse(&path, bytes.len() as u64) {
            return Ok(0);
        }
        if cfg!(target_os = "macos") && self.encoding == Encoding::Raw {
            if let Some(source) = source {
                if let Some(added) = self.try_clone_in(source, sha256, bytes.len() as u64)? {
                    return Ok(added);
                }
            }
        }
        self.write_area(Area::Objects, sha256, bytes)
    }

    /// Clones `source` into a temporary file and installs it as the object `sha256` if the
    /// clone still has those bytes (the source may have changed since it was read). `None`
    /// when cloning didn't work out; the caller writes the bytes it has instead.
    fn try_clone_in(
        &self,
        source: &Path,
        sha256: &str,
        size: u64,
    ) -> Result<Option<u64>, StoreError> {
        let dir = self.area_dir(Area::Objects);
        fs::create_dir_all(&dir).map_err(io_err("Failed to create the store"))?;
        let temp = fsx::temp_path(&dir);
        if fsx::clone_or_copy(source, &temp).is_err() {
            let _ = fsx::remove_file_forced(&temp);
            return Ok(None);
        }
        let guard = fsx::TempGuard::new(temp.clone());
        let checked = (|| -> io::Result<bool> {
            // The clone has the source's permissions and times: make it writable to set
            // its time to now (so a running garbage collection sees it as new).
            fsx::set_mode(&temp, 0o644)?;
            let mut file = File::options().read(true).write(true).open(&temp)?;
            file.set_modified(SystemTime::now())?;
            let mut hasher = Sha256::new();
            let mut total = 0u64;
            let mut buffer = vec![0u8; buffer_len(size)];
            loop {
                let n = match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                total += n as u64;
                hasher.update(&buffer[..n]);
            }
            file.sync_all()?;
            Ok(total == size && hex_digest(hasher) == sha256)
        })();
        match checked {
            Ok(true) => self.install(guard, Area::Objects, sha256, size).map(Some),
            _ => Ok(None),
        }
    }

    /// Whether the content-addressed file at `path` is there with `len` plain bytes; if
    /// so, touches it when old, so a garbage collection keeps it for the manifest about to
    /// use it.
    fn reuse(&self, path: &Path, len: u64) -> bool {
        if decoded_len(self.encoding, path) != Some(len) {
            return false;
        }
        let old = fs::metadata(path)
            .ok()
            .and_then(|meta| fsx::age(&meta, SystemTime::now()))
            .is_some_and(|age| age >= TOUCH_AFTER);
        if old {
            fsx::touch(path);
        }
        true
    }

    /// Writes `bytes` (encoded) as `name` in `area`, unless it is there. Returns the bytes
    /// added.
    fn write_area(&self, area: Area, name: &str, bytes: &[u8]) -> Result<u64, StoreError> {
        let path = self.area_path(area, name)?;
        if self.reuse(&path, bytes.len() as u64) {
            return Ok(0);
        }
        let dir = self.area_dir(area);
        fs::create_dir_all(&dir).map_err(io_err("Failed to create the store"))?;
        let (file, temp) =
            fsx::create_temp(&dir).map_err(io_err("Failed to write to the store"))?;
        let guard = fsx::TempGuard::new(temp);
        let file = encode_into(self.encoding, file, bytes)
            .map_err(io_err("Failed to write to the store"))?;
        file.sync_all()
            .map_err(io_err("Failed to flush the store"))?;
        drop(file);
        self.install(guard, area, name, bytes.len() as u64)
    }

    /// Makes the flushed temporary file read-only and renames it to `name` in `area`.
    /// Another writer may have put the same bytes there first, which is as good. A
    /// damaged file there is replaced by the rename, never deleted first, so the name is
    /// never empty. Returns the bytes added.
    fn install(
        &self,
        mut guard: fsx::TempGuard,
        area: Area,
        name: &str,
        len: u64,
    ) -> Result<u64, StoreError> {
        let Some(temp) = guard.path().map(Path::to_path_buf) else {
            return Ok(0);
        };
        let path = self.area_path(area, name)?;
        fsx::set_readonly(&temp).map_err(io_err("Failed to write to the store"))?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(io_err("Failed to create the store"))?;
        }
        if fs::symlink_metadata(&path).is_ok() {
            if decoded_len(self.encoding, &path) == Some(len) {
                return Ok(0);
            }
            fsx::clear_readonly(&path);
        }
        match fsx::rename_with_retry(&temp, &path) {
            Ok(()) => guard.disarm(),
            Err(_) if decoded_len(self.encoding, &path) == Some(len) => return Ok(0),
            Err(e) => return Err(io_err("Failed to write to the store")(e)),
        }
        if let Ok(mut unsynced) = self.unsynced.lock() {
            unsynced.insert((area, name[..2].to_string()));
        }
        Ok(fs::metadata(&path).map(|m| m.len()).unwrap_or(0))
    }

    /// Flushes the folders objects were renamed into since the last call (unix), so a
    /// manifest written next never points at objects a power cut could lose.
    pub fn sync(&self) -> Result<(), StoreError> {
        let dirs = match self.unsynced.lock() {
            Ok(mut unsynced) => std::mem::take(&mut *unsynced),
            Err(_) => BTreeSet::new(),
        };
        for (area, prefix) in &dirs {
            let dir = self.area_dir(*area).join(prefix);
            if let Err(e) = fsx::sync_dir(&dir) {
                // Keep them for the next try.
                if let Ok(mut unsynced) = self.unsynced.lock() {
                    unsynced.extend(dirs.iter().cloned());
                }
                return Err(io_err("Failed to flush the store")(e));
            }
        }
        Ok(())
    }

    /// Writes the file the object for `sha256` (`size` bytes) stands for into `out`,
    /// checking every chunk and the whole file's size and SHA-256. On an error, what was
    /// written to `out` must be thrown away.
    pub fn read_object(
        &self,
        sha256: &str,
        size: u64,
        out: &mut dyn Write,
        cancel: Cancel,
    ) -> Result<(), StoreError> {
        match self.object_form(sha256, size)? {
            ObjectForm::Whole => {
                let path = self.object_path(sha256)?;
                self.stream_whole(&path, sha256, size, out, cancel)
            }
            ObjectForm::Chunked(list) => {
                let mut hasher = Sha256::new();
                for chunk in &list.chunks {
                    if cancel() {
                        return Err(StoreError::Cancelled);
                    }
                    let bytes = self.read_chunk(chunk)?;
                    hasher.update(&bytes);
                    out.write_all(&bytes)
                        .map_err(io_err("Failed to write the file out"))?;
                }
                if hex_digest(hasher) != sha256 {
                    return Err(StoreError::Damaged(sha256.to_string()));
                }
                Ok(())
            }
        }
    }

    /// Reads a whole object, checking its size and hash as it goes.
    fn stream_whole(
        &self,
        path: &Path,
        sha256: &str,
        size: u64,
        out: &mut dyn Write,
        cancel: Cancel,
    ) -> Result<(), StoreError> {
        let damaged = || StoreError::Damaged(sha256.to_string());
        let file = File::open(path).map_err(|_| damaged())?;
        let mut reader = decoder(self.encoding, file).map_err(|_| damaged())?;
        let mut buffer = vec![0u8; buffer_len(size)];
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        loop {
            if cancel() {
                return Err(StoreError::Cancelled);
            }
            let n = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(damaged()),
            };
            total += n as u64;
            if total > size {
                return Err(damaged());
            }
            hasher.update(&buffer[..n]);
            out.write_all(&buffer[..n])
                .map_err(io_err("Failed to write the file out"))?;
        }
        if total != size || hex_digest(hasher) != sha256 {
            return Err(damaged());
        }
        Ok(())
    }

    /// The bytes of one chunk, checked against its size and hash.
    pub fn read_chunk(&self, chunk: &ChunkRef) -> Result<Vec<u8>, StoreError> {
        let path = self.chunk_path(&chunk.sha256)?;
        let bytes = match read_limited(self.encoding, &path, u64::from(chunk.size)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(StoreError::Missing(chunk.sha256.clone()))
            }
            Err(_) => return Err(StoreError::Damaged(chunk.sha256.clone())),
        };
        if bytes.len() != chunk.size as usize || sha256_hex(&bytes) != chunk.sha256 {
            return Err(StoreError::Damaged(chunk.sha256.clone()));
        }
        Ok(bytes)
    }

    /// The whole file of a small object in memory, checked.
    pub fn read_object_bytes(&self, sha256: &str, size: u64) -> Result<Vec<u8>, StoreError> {
        let mut out = Vec::with_capacity(usize::try_from(size.min(64 << 20)).unwrap_or(0));
        self.read_object(sha256, size, &mut out, NEVER)?;
        Ok(out)
    }

    /// Reads the object all the way through, checking every byte.
    pub fn verify_object(&self, sha256: &str, size: u64, cancel: Cancel) -> Result<(), StoreError> {
        self.read_object(sha256, size, &mut io::sink(), cancel)
    }

    /// Deletes the object for `sha256` if reading it all finds it damaged, so the next
    /// write of those bytes stores them again ([`Store::has_object`] only looks at its
    /// length or frame header, which survive most damage). Chunks shared with other files
    /// are left: a damaged chunk is found and deleted with [`Store::remove_damaged_chunk`].
    /// Returns whether it deleted the object.
    pub fn remove_damaged(&self, sha256: &str, size: u64) -> Result<bool, StoreError> {
        match self.verify_object(sha256, size, NEVER) {
            Err(StoreError::Damaged(name)) if name == sha256 => {}
            Err(StoreError::Damaged(chunk)) => {
                return Err(StoreError::Damaged(chunk));
            }
            Ok(()) | Err(StoreError::Missing(_)) => return Ok(false),
            Err(e) => return Err(e),
        }
        let path = self.object_path(sha256)?;
        match fsx::remove_file_forced(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err("Failed to delete a damaged object")(e)),
        }
    }

    /// Deletes a chunk whose bytes don't match its hash (and size). Returns whether it did.
    pub fn remove_damaged_chunk(&self, chunk: &ChunkRef) -> Result<bool, StoreError> {
        match self.read_chunk(chunk) {
            Err(StoreError::Damaged(_)) => {}
            _ => return Ok(false),
        }
        let path = self.chunk_path(&chunk.sha256)?;
        match fsx::remove_file_forced(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err("Failed to delete a damaged chunk")(e)),
        }
    }

    // -----------------------------------------------------------------------------------
    // Manifests

    /// Writes `image` (valid and sealed) as its instance's manifest `<id>.json`, after
    /// flushing the folders of the objects this store wrote. Writing the same image again
    /// replaces the file, keeping the newer informational fields (label, time).
    pub fn save_manifest(&self, instance: &str, image: &Image) -> Result<PathBuf, StoreError> {
        image
            .validate()
            .map_err(|e| StoreError::Manifest(ManifestError::Invalid(e)))?;
        image.verify()?;
        let path = self.manifest_path(instance, &image.id)?;
        self.sync()?;
        let dir = self.instance_dir(instance)?;
        let created = !dir.exists();
        fs::create_dir_all(&dir).map_err(io_err("Failed to create the image folder"))?;
        write_atomic(&dir, &path, &image.to_json_pretty())?;
        if created {
            fsx::sync_dir(&self.images_dir())
                .map_err(io_err("Failed to flush the image folder"))?;
        }
        Ok(path)
    }

    /// Reads one manifest and checks that it is valid and its contents hash to its id.
    pub fn load_manifest(&self, instance: &str, id: &str) -> Result<Image, StoreError> {
        let path = self.manifest_path(instance, id)?;
        let bytes = read_capped(&path, MAX_MANIFEST_BYTES)?;
        let image = Image::from_json(&bytes)?;
        if image.id != id {
            return Err(StoreError::Identity(IdentityError::Mismatch {
                claimed: id.to_string(),
                actual: image.id,
            }));
        }
        image.verify()?;
        Ok(image)
    }

    /// Every manifest of an instance that can be read, and those that can't.
    pub fn list_manifests(&self, instance: &str) -> Result<ManifestListing, StoreError> {
        let dir = self.instance_dir(instance)?;
        let mut listing = ManifestListing::default();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(listing),
            Err(e) => return Err(io_err("Failed to list the images")(e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(manifest_id) else {
                continue;
            };
            match self.load_manifest(instance, id) {
                Ok(image) => listing.images.push(image),
                Err(e) => listing.unreadable.push((id.to_string(), e.to_string())),
            }
        }
        listing
            .images
            .sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.id.cmp(&b.id)));
        listing.unreadable.sort();
        Ok(listing)
    }

    /// Deletes one manifest. Its objects stay until a garbage collection. Returns whether
    /// there was one.
    pub fn delete_manifest(&self, instance: &str, id: &str) -> Result<bool, StoreError> {
        let path = self.manifest_path(instance, id)?;
        match fsx::remove_file_forced(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err("Failed to delete the image")(e)),
        }
    }

    /// The instances with a manifest folder, sorted.
    pub fn list_instances(&self) -> Result<Vec<String>, StoreError> {
        let entries = match fs::read_dir(self.images_dir()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_err("Failed to list the images")(e)),
        };
        let mut instances: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|name| valid_instance_id(name))
            .collect();
        instances.sort();
        Ok(instances)
    }

    /// The instance's stat cache; empty when there is none or it can't be read (then every
    /// file is hashed again, which is only slower).
    pub fn load_stat_cache(&self, instance: &str) -> StatCache {
        self.instance_dir(instance)
            .ok()
            .and_then(|dir| read_capped(&dir.join(STAT_CACHE_FILE), MAX_MANIFEST_BYTES).ok())
            .and_then(|bytes| StatCache::from_json(&bytes))
            .unwrap_or_default()
    }

    /// Saves the instance's stat cache, replacing the last one.
    pub fn save_stat_cache(&self, instance: &str, cache: &StatCache) -> Result<(), StoreError> {
        let dir = self.instance_dir(instance)?;
        fs::create_dir_all(&dir).map_err(io_err("Failed to create the image folder"))?;
        write_atomic(&dir, &dir.join(STAT_CACHE_FILE), &cache.to_json())
    }

    /// Deletes temporary files older than `max_age` (left by a crash) in `objects/`,
    /// `chunks/` and every instance's folder. Returns how many.
    pub fn sweep_temp(&self, max_age: Duration) -> u64 {
        let mut dirs = Vec::new();
        for top in [
            self.area_dir(Area::Objects),
            self.area_dir(Area::Chunks),
            self.images_dir(),
        ] {
            if let Ok(entries) = fs::read_dir(&top) {
                dirs.extend(
                    entries
                        .flatten()
                        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                        .map(|e| e.path()),
                );
            }
            dirs.push(top);
        }
        let now = SystemTime::now();
        let mut removed = 0;
        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let is_temp = entry.file_name().to_str().is_some_and(fsx::is_temp_name);
                let old = entry
                    .metadata()
                    .ok()
                    .and_then(|m| fsx::age(&m, now))
                    .is_some_and(|age| age >= max_age);
                if is_temp && old && fsx::remove_file_forced(&entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

/// The id in a manifest's file name `<id>.json`.
pub(crate) fn manifest_id(name: &str) -> Option<&str> {
    name.strip_suffix(".json").filter(|id| is_sha256(id))
}

/// A finished SHA-256 as lowercase hex.
pub(crate) fn hex_digest(hasher: Sha256) -> String {
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(byte & 0xf)]));
    }
    out
}

/// Fails with [`StoreError::Changed`] unless the file at `path` still has the size and
/// modification time it had (`before`) when it was opened.
fn unchanged_since(path: &Path, before: &fs::Metadata) -> Result<(), StoreError> {
    let after = fs::symlink_metadata(path).map_err(StoreError::Source)?;
    if !after.is_file()
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err(StoreError::Changed);
    }
    Ok(())
}

/// Reads a file of at most `limit` bytes.
fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>, StoreError> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            StoreError::Missing(path.display().to_string())
        } else {
            io_err(format!("Failed to read {}", path.display()))(e)
        }
    })?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_err(format!("Failed to read {}", path.display())))?;
    if bytes.len() as u64 > limit {
        return Err(StoreError::Manifest(ManifestError::Invalid(format!(
            "{} is larger than {limit} bytes",
            path.display()
        ))));
    }
    Ok(bytes)
}

/// Writes `bytes` to `path` through a flushed temporary file in `dir`, then flushes `dir`.
fn write_atomic(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let what = || format!("Failed to write {}", path.display());
    let (mut file, temp) = fsx::create_temp(dir).map_err(io_err(what()))?;
    let mut guard = fsx::TempGuard::new(temp.clone());
    file.write_all(bytes).map_err(io_err(what()))?;
    file.sync_all().map_err(io_err(what()))?;
    drop(file);
    fsx::clear_readonly(path);
    fsx::rename_with_retry(&temp, path).map_err(io_err(what()))?;
    guard.disarm();
    fsx::sync_dir(dir).map_err(io_err(what()))
}

/// Stores a file given in pieces ([`Store::writer`]). Small files are kept in memory until
/// [`ObjectWriter::finish`]; large ones are cut into chunks as they come, and each chunk is
/// stored at once, so memory stays bounded whatever the size.
pub struct ObjectWriter<'s> {
    store: &'s Store,
    size: u64,
    expected: Option<String>,
    received: u64,
    hasher: Sha256,
    state: WriterState,
    added: u64,
}

enum WriterState {
    Whole(Vec<u8>),
    Chunked {
        chunker: Chunker,
        chunks: Vec<ChunkRef>,
    },
}

impl ObjectWriter<'_> {
    /// Adds the next bytes. More bytes than the size given is an error at once.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), StoreError> {
        self.received += bytes.len() as u64;
        if self.received > self.size {
            return Err(StoreError::SizeMismatch {
                expected: self.size,
                found: self.received,
            });
        }
        self.hasher.update(bytes);
        let store = self.store;
        let added = &mut self.added;
        match &mut self.state {
            WriterState::Whole(buffer) => buffer.extend_from_slice(bytes),
            WriterState::Chunked { chunker, chunks } => {
                chunker.push(bytes, &mut |chunk: &[u8]| -> Result<(), StoreError> {
                    let sha256 = sha256_hex(chunk);
                    *added += store.write_area(Area::Chunks, &sha256, chunk)?;
                    chunks.push(ChunkRef {
                        sha256,
                        // Chunks are at most 256 KiB.
                        size: u32::try_from(chunk.len()).unwrap_or(u32::MAX),
                    });
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    /// Checks the size and hash and stores the object: the bytes, or the chunk list after
    /// the last chunks. Nothing is named by the file's hash unless the bytes matched.
    pub fn finish(self) -> Result<Stored, StoreError> {
        let ObjectWriter {
            store,
            size,
            expected,
            received,
            hasher,
            state,
            mut added,
        } = self;
        if received != size {
            return Err(StoreError::SizeMismatch {
                expected: size,
                found: received,
            });
        }
        let sha256 = hex_digest(hasher);
        if let Some(expected) = expected {
            if expected != sha256 {
                return Err(StoreError::HashMismatch {
                    expected,
                    found: sha256,
                });
            }
        }
        match state {
            WriterState::Whole(bytes) => {
                added += store.write_whole(&sha256, &bytes, None)?;
            }
            WriterState::Chunked {
                chunker,
                mut chunks,
            } => {
                chunker.finish(&mut |chunk: &[u8]| -> Result<(), StoreError> {
                    let chunk_sha = sha256_hex(chunk);
                    added += store.write_area(Area::Chunks, &chunk_sha, chunk)?;
                    chunks.push(ChunkRef {
                        sha256: chunk_sha,
                        size: u32::try_from(chunk.len()).unwrap_or(u32::MAX),
                    });
                    Ok(())
                })?;
                let object = store.area_path(Area::Objects, &sha256)?;
                // A whole copy (the core's, from before chunking) is as good as a list.
                if !store.reuse(&object, size) {
                    let list = ChunkList { size, chunks }.encode();
                    added += store.write_area(Area::Objects, &sha256, &list)?;
                }
            }
        }
        Ok(Stored {
            sha256,
            size,
            added_bytes: added,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
