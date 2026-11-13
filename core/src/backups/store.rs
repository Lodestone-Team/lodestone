//! The backup store: content-addressed objects shared by every server, one manifest per
//! backup, walking a server folder with a stat cache, and garbage collection.
//!
//! ```text
//! <root>/
//!   objects/ab/<sha256 hex>        one zstd frame, read-only
//!   objects/.incoming-<16>.tmp     in-flight writes (swept when older than an hour)
//!   servers/<uuid>/
//!     policy.json                  BackupPolicy
//!     <backup id>.json             Manifest
//!     restore-journal.json         only while an in-place restore swaps
//!     save-off.marker              only while a live backup has saving off
//!     full-scan.count              scheduled backups saved since every file was hashed
//! ```
//!
//! Everything here is blocking and knows nothing of the app state; callers run it with
//! `spawn_blocking` and hold the service's locks (jobs share the store, GC has it alone).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use color_eyre::eyre::{eyre, Context};
use rand::RngExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::error::{Error, ErrorKind};
use crate::types::InstanceUuid;

use super::pattern::{always_excluded, always_excluded_path, Matcher};
use super::types::{
    BackupKind, BackupPolicy, BackupUsage, FileEntry, Manifest, SkipReason, SkippedFile,
    MANIFEST_SCHEMA, MAX_NAME_LEN,
};

/// Free space always left on a disk after a backup, restore or download.
pub const FREE_SPACE_MARGIN: u64 = 1 << 30;
/// A cached hash isn't trusted for a file modified this close before the scan that hashed it.
const RACY_NS: i128 = 2_000_000_000;
/// Every this many scheduled backups, every file is hashed again.
const FULL_SCAN_EVERY: usize = 10;
/// A live backup with more than this to read reads it first with saving on
/// ([`Store::prefetch`]), so saving stays off only while reading what changed since.
pub const PREFETCH_MIN_BYTES: u64 = 256 << 20;
/// A file that changes while it is read is tried this many more times.
const READ_RETRIES: usize = 3;
/// Read in chunks of this size, checking for cancellation between them.
const CHUNK: usize = 1 << 20;
const ZSTD_LEVEL: i32 = 3;
/// Temporary object files older than this are left over from a crash.
pub const TEMP_MAX_AGE: Duration = Duration::from_secs(3600);
/// How long to keep retrying a rename that Windows refuses because something has the file
/// open (antivirus, the search indexer, Explorer).
#[cfg(windows)]
const OBJECT_RENAME_BUDGET: Duration = Duration::from_secs(2);
#[cfg(not(windows))]
const OBJECT_RENAME_BUDGET: Duration = Duration::ZERO;

const POLICY_FILE: &str = "policy.json";
const JOURNAL_FILE: &str = "restore-journal.json";
const MARKER_FILE: &str = "save-off.marker";
/// Not `.json`, so neither listing the backups nor GC takes it for a manifest.
const FULL_SCAN_COUNT_FILE: &str = "full-scan.count";

/// Checked between files and chunks; true means stop.
pub type Cancel<'a> = &'a dyn Fn() -> bool;

pub fn err(kind: ErrorKind, message: impl Into<String>) -> Error {
    Error {
        kind,
        source: eyre!(message.into()),
    }
}

/// The error of work stopped by its cancellation check.
pub fn cancelled() -> Error {
    err(ErrorKind::Internal, "Cancelled")
}

// ---------------------------------------------------------------------------------------
// Names, ids and paths

/// Server folders in the store are named by the instance uuid, which is an unvalidated
/// string (and URL parameters are percent-decoded), so only these characters are accepted.
pub fn valid_server_id(id: &str) -> bool {
    (1..=100).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Backup ids: `YYYYMMDD-HHMMSS-xxxx` when made here; any `[A-Za-z0-9-]{1,64}` is accepted,
/// except names that could reach another file than `<id>.json`.
///
/// Case is ignored when comparing, because the store's disk may ignore it (Windows, macOS):
/// there `POLICY` would name `policy.json`. Windows' device names (`CON`, `NUL`, `COM1`…)
/// are refused too, since `CON.json` opens the console on older Windows.
pub fn valid_backup_id(id: &str) -> bool {
    const RESERVED: [&str; 6] = ["policy", "restore-journal", "con", "prn", "aux", "nul"];
    let device_with_number = |prefix: &str| {
        id.len() == 4 && id[..3].eq_ignore_ascii_case(prefix) && id.as_bytes()[3].is_ascii_digit()
    };
    (1..=64).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !RESERVED.iter().any(|name| id.eq_ignore_ascii_case(name))
        && !device_with_number("com")
        && !device_with_number("lpt")
}

/// A new backup id: the UTC time and 4 random hex digits.
pub fn new_backup_id(now: DateTime<Utc>) -> String {
    format!(
        "{}-{:04x}",
        now.format("%Y%m%d-%H%M%S"),
        rand::rng().random::<u16>()
    )
}

/// 16 random hex digits, for temporary and restore folder names.
pub fn random_suffix() -> String {
    format!("{:016x}", rand::rng().random::<u64>())
}

pub fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `path` can be stored in a manifest and joined onto a folder safely: `/`-separated
/// plain names, without `\`, `:`, control characters, `.`, `..` or empty parts.
pub fn valid_rel_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part
                    .chars()
                    .any(|c| c.is_control() || c == '\\' || c == ':')
        })
}

/// `path` (valid per [`valid_rel_path`]) under `base`.
pub fn join_rel(base: &Path, path: &str) -> PathBuf {
    let mut joined = base.to_path_buf();
    for part in path.split('/') {
        joined.push(part);
    }
    joined
}

/// A backup name from the user: trimmed, `None` when empty.
pub fn check_backup_name(name: &str) -> Result<Option<String>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(format!(
            "Backup names can be at most {MAX_NAME_LEN} characters long"
        ));
    }
    if name.chars().any(char::is_control) {
        return Err("Backup names can't have control characters".to_string());
    }
    Ok(Some(name.to_string()))
}

/// `level-name` when it is a world folder Lodestone accepts, for [`Manifest::level_name`].
pub fn checked_level_name(level_name: &str) -> Option<String> {
    crate::handlers::server_content::world_folder(level_name)
        .ok()
        .map(|_| level_name.to_string())
}

pub fn unix_ns(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_nanos() as i128,
        Err(before) => -(before.duration().as_nanos() as i128),
    }
}

/// Sizes for messages: "1.2 GB".
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Checks a manifest loaded from `servers/<uuid>/<id>.json`.
pub fn validate_manifest(m: &Manifest, uuid: &str, id: &str) -> Result<(), String> {
    if m.schema > MANIFEST_SCHEMA {
        return Err(format!(
            "it was made by a newer Lodestone (schema {})",
            m.schema
        ));
    }
    if m.id != id {
        return Err("its id doesn't match its file name".to_string());
    }
    if m.instance_uuid != uuid {
        return Err("its server doesn't match its folder".to_string());
    }
    if m.name
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_NAME_LEN)
    {
        return Err("its name is too long".to_string());
    }
    let mut seen = HashSet::new();
    let mut seen_folded = HashSet::new();
    for file in &m.files {
        let path = file.path.as_str();
        if !valid_rel_path(path) {
            return Err(format!("it has an invalid path {path:?}"));
        }
        if path == ".lodestone_config"
            || path.starts_with(".lodestone-restore")
            || path
                .split('/')
                .any(|part| part.starts_with(".lodestone-restore"))
        {
            return Err(format!("it has a path it can't restore: {path:?}"));
        }
        if !valid_sha256(&file.sha256) {
            return Err(format!("it has an invalid hash for {path:?}"));
        }
        if !seen.insert(path) {
            return Err(format!("it has {path:?} twice"));
        }
        if cfg!(windows) && !seen_folded.insert(path.to_lowercase()) {
            return Err(format!(
                "it has two paths that differ only in case, like {path:?}"
            ));
        }
    }
    Ok(())
}

/// Whether two sorted file lists have the same paths, sizes and contents.
pub fn same_files(a: &[FileEntry], b: &[FileEntry]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.path == y.path && x.size == y.size && x.sha256 == y.sha256)
}

// ---------------------------------------------------------------------------------------
// Disks

/// Free space on the disk holding `path` (or its nearest existing parent): the disk whose
/// mount point is the longest prefix of the resolved path.
pub fn free_space(path: &Path) -> Option<u64> {
    let mut existing = path;
    let resolved = loop {
        match existing.canonicalize() {
            Ok(resolved) => break resolved,
            Err(_) => existing = existing.parent()?,
        }
    };
    let resolved = strip_verbatim(resolved);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path_starts_with(&resolved, disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
}

/// `canonicalize` gives `\\?\C:\...` on Windows; mount points are `C:\`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    if cfg!(windows) {
        let path = PathBuf::from(path.to_string_lossy().to_lowercase());
        let prefix = PathBuf::from(prefix.to_string_lossy().to_lowercase());
        path.starts_with(prefix)
    } else {
        path.starts_with(prefix)
    }
}

/// Fails unless the disk holding `path` has `bytes` plus [`FREE_SPACE_MARGIN`] free.
/// `what` completes "Not enough free disk space …", for example "for a backup" or
/// "to restore". Passes when the free space can't be found out.
pub fn require_free_space(path: &Path, bytes: u64, what: &str) -> Result<(), Error> {
    let needed = bytes.saturating_add(FREE_SPACE_MARGIN);
    match free_space(path) {
        Some(free) if free < needed => Err(err(
            ErrorKind::Conflict,
            format!(
                "Not enough free disk space {what}: {} needed, {} free",
                format_bytes(needed),
                format_bytes(free)
            ),
        )),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------
// File system helpers

/// Renames, retrying for up to `budget` while Windows reports the file in use. Elsewhere,
/// and for other errors, fails at once.
pub fn rename_with_retry(from: &Path, to: &Path, budget: Duration) -> io::Result<()> {
    let started = Instant::now();
    let mut wait = Duration::from_millis(20);
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if is_in_use(&e) && started.elapsed() + wait <= budget => {
                std::thread::sleep(wait);
                wait = (wait * 2).min(Duration::from_millis(500));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Access denied, sharing violation or lock violation: on Windows, something has the file
/// (or a file in the folder) open.
fn is_in_use(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// Deletes a file even when it is read-only (Windows refuses to delete those).
pub fn remove_file_forced(path: &Path) -> io::Result<()> {
    clear_readonly(path);
    fs::remove_file(path)
}

/// Clears a file's read-only flag on Windows, which refuses to delete or rename over a
/// read-only file. Elsewhere the flag doesn't matter for either, and this does nothing.
fn clear_readonly(path: &Path) {
    #[cfg(windows)]
    if let Ok(meta) = fs::symlink_metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
    #[cfg(not(windows))]
    let _ = path;
}

/// Flushes a folder's entries (new and renamed files) to disk. Only needed on unix.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn is_executable(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o100 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

fn modified_ns(meta: &fs::Metadata) -> i128 {
    meta.modified().map(unix_ns).unwrap_or(0)
}

#[cfg(test)]
thread_local! {
    static FILE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Run by [`Store::store_object`] right after it found the object missing or damaged:
    /// lets a test store it from "another job" at that moment.
    static AFTER_PRESENCE_CHECK: std::cell::RefCell<Option<Box<dyn FnMut(&Path)>>> =
        const { std::cell::RefCell::new(None) };
}

/// How many server files this thread has opened for reading (tests only).
#[cfg(test)]
pub(crate) fn file_reads() -> usize {
    FILE_READS.with(|reads| reads.get())
}

// ---------------------------------------------------------------------------------------
// Objects

/// Why storing an object failed.
#[derive(Debug)]
pub enum StoreError {
    /// Reading the source failed (locked, in use, permission denied).
    Source(io::Error),
    /// The source had more or fewer bytes than expected: it changed while being read.
    SizeMismatch,
    Cancelled,
    /// Writing to the store failed; the backup can't go on.
    Store(Error),
}

/// Why reading an object failed.
#[derive(Debug)]
pub enum ReadError {
    /// Missing, corrupt, or its contents don't match its hash or size.
    Damaged,
    Cancelled,
    /// Writing the contents out failed.
    Io(io::Error),
}

impl ReadError {
    /// The error for the user, naming the backup and the file.
    pub fn into_error(self, backup_id: &str, path: &str) -> Error {
        match self {
            ReadError::Damaged => err(
                ErrorKind::Internal,
                format!("Backup {backup_id} is damaged: {path}"),
            ),
            ReadError::Cancelled => cancelled(),
            ReadError::Io(e) => Error {
                kind: ErrorKind::Internal,
                source: eyre!(e).wrap_err(format!("Failed to write {path}")),
            },
        }
    }
}

/// An object stored (or found already stored) by [`Store::store_object`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObject {
    pub sha256: String,
    /// The object file was written now, rather than already there.
    pub added: bool,
    /// Its compressed size.
    pub stored_size: u64,
}

/// Deletes a temporary file when dropped, unless disarmed.
struct TempFile(Option<PathBuf>);

impl TempFile {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = remove_file_forced(&path);
        }
    }
}

/// Feeds `reader` (expected to have exactly `size` bytes) to `sink` in chunks while hashing
/// it. Returns the hex SHA-256.
fn digest(
    reader: &mut dyn Read,
    size: u64,
    mut sink: impl FnMut(&[u8]) -> Result<(), StoreError>,
    cancel: Cancel,
) -> Result<String, StoreError> {
    let mut buffer = vec![0u8; (size.saturating_add(1)).clamp(8 * 1024, CHUNK as u64) as usize];
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
            Err(e) => return Err(StoreError::Source(e)),
        };
        total += n as u64;
        if total > size {
            return Err(StoreError::SizeMismatch);
        }
        hasher.update(&buffer[..n]);
        sink(&buffer[..n])?;
    }
    if total != size {
        return Err(StoreError::SizeMismatch);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// The policy, manifests and objects under one folder.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Store {
        Store { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn objects_dir(&self) -> PathBuf {
        self.root.join("objects")
    }

    /// `objects/ab/<sha256>`. `sha256` must be valid ([`valid_sha256`]).
    pub fn object_path(&self, sha256: &str) -> PathBuf {
        self.objects_dir().join(&sha256[..2]).join(sha256)
    }

    pub fn servers_dir(&self) -> PathBuf {
        self.root.join("servers")
    }

    /// `servers/<uuid>`. Every per-server path goes through here, so a uuid from a URL
    /// can't reach outside the store.
    pub fn server_dir(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        if !valid_server_id(uuid.as_ref()) {
            return Err(err(ErrorKind::BadRequest, "Invalid server id"));
        }
        Ok(self.servers_dir().join(uuid.as_ref()))
    }

    pub fn manifest_path(&self, uuid: &InstanceUuid, id: &str) -> Result<PathBuf, Error> {
        if !valid_backup_id(id) {
            return Err(err(ErrorKind::BadRequest, "Invalid backup id"));
        }
        Ok(self.server_dir(uuid)?.join(format!("{id}.json")))
    }

    pub fn policy_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.server_dir(uuid)?.join(POLICY_FILE))
    }

    /// Where an in-place restore keeps its journal: outside the server folder, so the
    /// server's users can't write it.
    pub fn journal_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.server_dir(uuid)?.join(JOURNAL_FILE))
    }

    pub fn marker_path(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        Ok(self.server_dir(uuid)?.join(MARKER_FILE))
    }

    /// How many scheduled backups were saved since the last backup or check that hashed
    /// every file. `None` when it isn't recorded (or can't be read): then it is counted
    /// from the manifests that are left.
    fn scheduled_since_full_scan(&self, uuid: &InstanceUuid) -> Option<usize> {
        let path = self.server_dir(uuid).ok()?.join(FULL_SCAN_COUNT_FILE);
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    /// Records [`Scan::scheduled_since_full_after`]. Kept in a file of its own, because
    /// retention may delete the backup that was a full scan, and a full scan that found
    /// nothing changed saves no backup at all.
    pub fn record_scheduled_since_full_scan(
        &self,
        uuid: &InstanceUuid,
        count: usize,
    ) -> Result<(), Error> {
        fs::create_dir_all(self.server_dir(uuid)?)
            .context("Failed to create the server's backup folder")?;
        let path = self.server_dir(uuid)?.join(FULL_SCAN_COUNT_FILE);
        crate::persist::write_atomic_blocking(&path, count.to_string().as_bytes(), None)
    }

    /// The server's policy; the default when it has none (or it can't be read).
    pub fn load_policy(&self, uuid: &InstanceUuid) -> Result<BackupPolicy, Error> {
        let path = self.policy_path(uuid)?;
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(policy) => Ok(policy),
                Err(e) => {
                    warn!(
                        "The backup policy {} can't be read, using the default: {e}",
                        path.display()
                    );
                    Ok(BackupPolicy::default())
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BackupPolicy::default()),
            Err(e) => Err(Error::from(
                eyre!(e).wrap_err("Failed to read the backup policy"),
            )),
        }
    }

    pub fn save_policy(&self, uuid: &InstanceUuid, policy: &BackupPolicy) -> Result<(), Error> {
        policy
            .validate()
            .map_err(|e| err(ErrorKind::BadRequest, e))?;
        let path = self.policy_path(uuid)?;
        fs::create_dir_all(self.server_dir(uuid)?)
            .context("Failed to create the server's backup folder")?;
        let json = serde_json::to_vec_pretty(policy).context("Failed to write the policy")?;
        crate::persist::write_atomic_blocking(&path, &json, None)
    }

    /// Whether the object for `sha256` is there and looks whole: not empty, and its zstd
    /// frame header says it holds `size` bytes. Reading it all is left to restores.
    pub fn object_present(&self, sha256: &str, size: u64) -> bool {
        valid_sha256(sha256) && object_present_at(&self.object_path(sha256), size)
    }

    /// The object's compressed size, if it is there.
    pub fn object_stored_size(&self, sha256: &str) -> Option<u64> {
        if !valid_sha256(sha256) {
            return None;
        }
        fs::metadata(self.object_path(sha256)).ok().map(|m| m.len())
    }

    /// Stores `reader`'s contents (exactly `size` bytes) unless an object with the same
    /// hash is present.
    ///
    /// The contents are compressed into a temporary file while hashed, flushed to disk, and
    /// renamed into place read-only. On unix the caller must [`Store::sync_object_dirs`]
    /// before writing a manifest that uses the object.
    pub fn store_object(
        &self,
        reader: &mut dyn Read,
        size: u64,
        cancel: Cancel,
    ) -> Result<StoredObject, StoreError> {
        let store_err = |e: io::Error, what: &str| {
            StoreError::Store(Error::from(eyre!(e).wrap_err(what.to_string())))
        };
        let objects = self.objects_dir();
        fs::create_dir_all(&objects)
            .map_err(|e| store_err(e, "Failed to create the backup store"))?;
        let temp_path = objects.join(format!(".incoming-{}.tmp", random_suffix()));
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|e| store_err(e, "Failed to write to the backup store"))?;
        let mut temp = TempFile(Some(temp_path.clone()));

        let mut encoder = zstd::stream::write::Encoder::new(BufWriter::new(file), ZSTD_LEVEL)
            .map_err(|e| store_err(e, "Failed to start compressing"))?;
        let setup = encoder
            .include_checksum(true)
            .and_then(|_| encoder.include_contentsize(true))
            .and_then(|_| encoder.set_pledged_src_size(Some(size)));
        setup.map_err(|e| store_err(e, "Failed to start compressing"))?;
        let sha256 = digest(
            reader,
            size,
            |chunk| {
                encoder
                    .write_all(chunk)
                    .map_err(|e| store_err(e, "Failed to write to the backup store"))
            },
            cancel,
        )?;
        let file = encoder
            .finish()
            .map_err(|e| store_err(e, "Failed to write to the backup store"))?
            .into_inner()
            .map_err(|e| store_err(e.into_error(), "Failed to write to the backup store"))?;
        file.sync_all()
            .map_err(|e| store_err(e, "Failed to flush the backup store"))?;
        drop(file);

        let final_path = self.object_path(&sha256);
        let already_there = |final_path: &Path| StoredObject {
            sha256: sha256.clone(),
            added: false,
            stored_size: fs::metadata(final_path).map(|m| m.len()).unwrap_or(0),
        };
        if object_present_at(&final_path, size) {
            return Ok(already_there(&final_path));
        }
        #[cfg(test)]
        AFTER_PRESENCE_CHECK.with(|hook| {
            if let Some(hook) = hook.borrow_mut().as_mut() {
                hook(&final_path);
            }
        });
        let dir = final_path.parent().expect("objects have a folder");
        fs::create_dir_all(dir).map_err(|e| store_err(e, "Failed to create the backup store"))?;
        let mut permissions = fs::metadata(&temp_path)
            .map_err(|e| store_err(e, "Failed to write to the backup store"))?
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&temp_path, permissions)
            .map_err(|e| store_err(e, "Failed to write to the backup store"))?;
        if fs::symlink_metadata(&final_path).is_ok() {
            // Another job may have stored it since the check above; that object is whole and
            // a manifest may already use it.
            if object_present_at(&final_path, size) {
                return Ok(already_there(&final_path));
            }
            // There but damaged. It is replaced by the rename below, never deleted first,
            // so a failed rename (or a crash) can't leave the name empty: if the object
            // turns out to have been whole after all, the rename puts the same contents in
            // its place. Windows refuses to replace a read-only file.
            warn!("Replacing the damaged backup object {sha256}");
            clear_readonly(&final_path);
        }
        match rename_with_retry(&temp_path, &final_path, OBJECT_RENAME_BUDGET) {
            Ok(()) => temp.disarm(),
            // another writer stored the same contents first
            Err(_) if object_present_at(&final_path, size) => {
                if let Ok(meta) = fs::metadata(&final_path) {
                    let mut permissions = meta.permissions();
                    permissions.set_readonly(true);
                    let _ = fs::set_permissions(&final_path, permissions);
                }
                return Ok(already_there(&final_path));
            }
            Err(e) => return Err(store_err(e, "Failed to write to the backup store")),
        }
        let stored_size = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
        Ok(StoredObject {
            sha256,
            added: true,
            stored_size,
        })
    }

    /// Flushes the folders of these objects to disk (unix), so a manifest written next never
    /// points at objects a power cut could lose.
    pub fn sync_object_dirs<'a>(
        &self,
        hashes: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), Error> {
        if !cfg!(unix) {
            return Ok(());
        }
        let dirs: HashSet<&str> = hashes
            .into_iter()
            .filter(|h| h.len() >= 2)
            .map(|h| &h[..2])
            .collect();
        let objects = self.objects_dir();
        for dir in dirs {
            sync_dir(&objects.join(dir)).context("Failed to flush the backup store")?;
        }
        Ok(())
    }

    /// Decompresses the object into `out`, checking its size and SHA-256.
    pub fn read_object(
        &self,
        sha256: &str,
        size: u64,
        out: &mut dyn Write,
        cancel: Cancel,
    ) -> Result<(), ReadError> {
        if !valid_sha256(sha256) {
            return Err(ReadError::Damaged);
        }
        let file = File::open(self.object_path(sha256)).map_err(|_| ReadError::Damaged)?;
        let mut decoder = zstd::stream::read::Decoder::new(file)
            .map_err(|_| ReadError::Damaged)?
            .single_frame();
        let mut buffer = vec![0u8; (size.saturating_add(1)).clamp(8 * 1024, CHUNK as u64) as usize];
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        loop {
            if cancel() {
                return Err(ReadError::Cancelled);
            }
            let n = match decoder.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(ReadError::Damaged),
            };
            total += n as u64;
            if total > size {
                return Err(ReadError::Damaged);
            }
            hasher.update(&buffer[..n]);
            out.write_all(&buffer[..n]).map_err(ReadError::Io)?;
        }
        if total != size || hex::encode(hasher.finalize()) != sha256 {
            return Err(ReadError::Damaged);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------------------
    // Manifests

    /// Loads and checks a manifest. NotFound when there is no such backup.
    pub fn load_manifest(&self, uuid: &InstanceUuid, id: &str) -> Result<Manifest, Error> {
        let path = self.manifest_path(uuid, id)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(err(ErrorKind::NotFound, "Backup not found"));
            }
            Err(e) => {
                return Err(Error::from(
                    eyre!(e).wrap_err(format!("Failed to read backup {id}")),
                ))
            }
        };
        parse_manifest(&bytes, uuid.as_ref(), id).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("Backup {id} can't be used: {e}"),
            )
        })
    }

    /// Writes a manifest atomically. Its objects must already be durable.
    pub fn save_manifest(&self, manifest: &Manifest) -> Result<(), Error> {
        validate_manifest(manifest, manifest.instance_uuid.as_ref(), &manifest.id)
            .map_err(|e| err(ErrorKind::Internal, format!("Invalid backup: {e}")))?;
        let path = self.manifest_path(&manifest.instance_uuid, &manifest.id)?;
        fs::create_dir_all(self.server_dir(&manifest.instance_uuid)?)
            .context("Failed to create the server's backup folder")?;
        let json = serde_json::to_vec(manifest).context("Failed to write the backup")?;
        crate::persist::write_atomic_blocking(&path, &json, None)
    }

    /// Deletes a backup's manifest; its objects go at the next GC. NotFound when it is gone.
    pub fn delete_manifest(&self, uuid: &InstanceUuid, id: &str) -> Result<(), Error> {
        let path = self.manifest_path(uuid, id)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                Err(err(ErrorKind::NotFound, "Backup not found"))
            }
            Err(e) => Err(Error::from(
                eyre!(e).wrap_err(format!("Failed to delete backup {id}")),
            )),
        }
    }

    /// Deletes all of a server's backups and its policy (when the server is deleted).
    pub fn delete_server(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let dir = self.server_dir(uuid)?;
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::from(
                eyre!(e).wrap_err("Failed to delete the server's backups"),
            )),
        }
    }

    /// The server's valid manifests, newest first. Invalid ones are skipped with a log line.
    pub fn list_manifests(&self, uuid: &InstanceUuid) -> Result<Vec<Manifest>, Error> {
        let dir = self.server_dir(uuid)?;
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Error::from(
                    eyre!(e).wrap_err("Failed to list the server's backups"),
                ))
            }
        };
        let mut manifests = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".json")) else {
                continue;
            };
            if !valid_backup_id(id) {
                continue;
            }
            match self.load_manifest(uuid, id) {
                Ok(manifest) => manifests.push(manifest),
                Err(e) => warn!("Skipping backup {}: {}", entry.path().display(), e.source),
            }
        }
        sort_newest_first(&mut manifests);
        Ok(manifests)
    }

    // -----------------------------------------------------------------------------------
    // Taking a backup

    /// Walks the server folder and decides which files must be read; see [`Scan`].
    pub fn scan(&self, request: ScanRequest, cancel: Cancel) -> Result<Scan, Error> {
        self.scan_with(request, None, cancel)
    }

    /// [`Store::scan`], also taking hashes from `prefetched` (read earlier in the same
    /// backup), which are trusted even in a full scan.
    pub fn scan_with(
        &self,
        request: ScanRequest,
        prefetched: Option<&Prefetched>,
        cancel: Cancel,
    ) -> Result<Scan, Error> {
        let scan_started_ns = unix_ns(SystemTime::now());
        let manifests = self.list_manifests(request.uuid)?;
        let scheduled_since_full = self
            .scheduled_since_full_scan(request.uuid)
            .unwrap_or_else(|| scheduled_since_full_manifest(&manifests));
        let full_scan = needs_full_scan(request.kind, &manifests, scheduled_since_full);
        let mut earlier_hashes = HashSet::new();
        for m in &manifests {
            earlier_hashes.extend(m.files.iter().map(|f| f.sha256.clone()));
        }
        let previous = manifests.into_iter().next();

        let Walked {
            files: found,
            mut skipped,
            unlistable,
        } = walk(request.server_root, request.exclude, cancel)?;
        let carried = {
            let found_paths: HashSet<&str> = found.iter().map(|f| f.path.as_str()).collect();
            carry_over_unlistable(
                previous.as_ref(),
                &unlistable,
                &found_paths,
                request.exclude,
                |f| self.object_present(&f.sha256, f.size),
                &mut skipped,
            )
        };
        // (cache, files modified at or after this time aren't trusted from it)
        let mut caches: Vec<(HashMap<&str, &FileEntry>, i128)> = Vec::new();
        if let Some(p) = prefetched {
            caches.push((
                p.files.iter().map(|f| (f.path.as_str(), f)).collect(),
                p.scan_started_ns - RACY_NS,
            ));
        }
        // a full scan doesn't take hashes from the previous backup, but still uses it to
        // tell how much it will store: what it holds unchanged is read, not stored again
        let previous_cache = previous.as_ref().map(|m| {
            (
                m.files
                    .iter()
                    .map(|f| (f.path.as_str(), f))
                    .collect::<HashMap<&str, &FileEntry>>(),
                m.scan_started_ns - RACY_NS,
            )
        });
        let full_scan_cache = if full_scan {
            previous_cache
        } else {
            caches.extend(previous_cache);
            None
        };
        fn cache_hit<'a>(
            store: &Store,
            (cache, racy_from): &(HashMap<&str, &'a FileEntry>, i128),
            file: &FoundFile,
        ) -> Option<&'a FileEntry> {
            cache.get(file.path.as_str()).copied().filter(|entry| {
                entry.size == file.size
                    && entry.modified_ns == file.modified_ns
                    && file.modified_ns < *racy_from
                    && store.object_present(&entry.sha256, entry.size)
            })
        }
        let mut cached = Vec::new();
        let mut to_read = Vec::new();
        let mut bytes_to_store = 0;
        for file in found {
            let reuse = caches
                .iter()
                .find_map(|cache| cache_hit(self, cache, &file));
            let stored_before = full_scan_cache
                .as_ref()
                .is_some_and(|cache| cache_hit(self, cache, &file).is_some());
            if reuse.is_none() && !stored_before {
                bytes_to_store += file.size;
            }
            match reuse {
                Some(entry) => cached.push(FileEntry {
                    path: file.path,
                    size: file.size,
                    sha256: entry.sha256.clone(),
                    modified_ns: file.modified_ns,
                    executable: file.executable,
                }),
                None => to_read.push(file),
            }
        }
        Ok(Scan {
            server_root: request.server_root.to_path_buf(),
            scan_started_ns,
            kind: request.kind,
            full_scan,
            scheduled_since_full,
            bytes_to_store,
            previous,
            earlier_hashes,
            cached,
            carried,
            to_read,
            skipped,
        })
    }

    /// Reads and stores the files `scan` found to read, without making a backup of them:
    /// a live backup does this while the server still saves when there is a lot to read
    /// ([`Scan::worth_prefetching`]), so that the pass with saving off only reads what
    /// changed since. Files that can't be read are left for that pass.
    pub fn prefetch(
        &self,
        scan: &Scan,
        progress: &mut dyn FnMut(u64, u64),
        cancel: Cancel,
    ) -> Result<Prefetched, Error> {
        require_free_space(&self.root, scan.bytes_to_store(), "for a backup")?;
        let total = scan.bytes_to_read();
        let mut done = 0;
        let mut files = Vec::new();
        for file in &scan.to_read {
            if cancel() {
                return Err(cancelled());
            }
            let absolute = join_rel(&scan.server_root, &file.path);
            if let ReadOutcome::Done(entry) = self.read_file(&absolute, true, cancel)? {
                files.push(FileEntry {
                    path: file.path.clone(),
                    ..entry
                });
            }
            done += file.size;
            progress(done, total);
        }
        // the backup's manifest may use these objects without reading them again
        self.sync_object_dirs(files.iter().map(|f| f.sha256.as_str()))?;
        Ok(Prefetched {
            scan_started_ns: scan.scan_started_ns,
            files,
        })
    }

    /// Reads and stores the files the scan couldn't take from the cache. `progress` gets
    /// (bytes done, bytes to read) after each file.
    ///
    /// A file that can't be read, or keeps changing, doesn't fail the backup: the previous
    /// backup's copy is kept if it has one, and the file is listed in `skipped`.
    pub fn copy(
        &self,
        scan: &Scan,
        progress: &mut dyn FnMut(u64, u64),
        cancel: Cancel,
    ) -> Result<Copied, Error> {
        let total = scan.bytes_to_read();
        let mut done = 0;
        let mut files = scan.cached.clone();
        files.extend(scan.carried.iter().cloned());
        let mut skipped = scan.skipped.clone();
        let mut new_hashes: Vec<String> = Vec::new();
        for file in &scan.to_read {
            if cancel() {
                return Err(cancelled());
            }
            let absolute = join_rel(&scan.server_root, &file.path);
            match self.read_file(&absolute, true, cancel)? {
                ReadOutcome::Done(entry) => {
                    new_hashes.push(entry.sha256.clone());
                    files.push(FileEntry {
                        path: file.path.clone(),
                        ..entry
                    });
                }
                ReadOutcome::Vanished => {}
                ReadOutcome::Failed(reason) => {
                    let carried = scan.previous.as_ref().and_then(|m| {
                        m.files
                            .iter()
                            .find(|f| f.path == file.path)
                            .filter(|f| self.object_present(&f.sha256, f.size))
                    });
                    if let Some(entry) = carried {
                        files.push(entry.clone());
                    }
                    skipped.push(SkippedFile {
                        path: file.path.clone(),
                        reason,
                        carried_over: carried.is_some(),
                    });
                }
            }
            done += file.size;
            progress(done, total);
        }
        self.sync_object_dirs(new_hashes.iter().map(|h| h.as_str()))?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        skipped.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Copied { files, skipped })
    }

    /// The current files' hashes, as a backup would record them, without storing anything:
    /// for a restore's permission check when it takes no safety backup. Files that can't be
    /// read are left out.
    pub fn hash_current(
        &self,
        request: ScanRequest,
        cancel: Cancel,
    ) -> Result<Vec<FileEntry>, Error> {
        let request = ScanRequest {
            kind: BackupKind::Manual,
            ..request
        };
        let scan = self.scan(request, cancel)?;
        let mut files = scan.cached;
        for file in scan.to_read {
            let absolute = join_rel(&scan.server_root, &file.path);
            if let ReadOutcome::Done(entry) = self.read_file(&absolute, false, cancel)? {
                files.push(FileEntry {
                    path: file.path,
                    ..entry
                });
            }
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Reads one file, storing it as an object when `store`, retrying while it changes.
    /// The returned entry's path is empty.
    fn read_file(&self, path: &Path, store: bool, cancel: Cancel) -> Result<ReadOutcome, Error> {
        for _ in 0..=READ_RETRIES {
            let before = match fs::symlink_metadata(path) {
                Ok(meta) if meta.is_file() => meta,
                Ok(_) => return Ok(ReadOutcome::Failed(SkipReason::Unreadable)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ReadOutcome::Vanished),
                Err(_) => return Ok(ReadOutcome::Failed(SkipReason::Unreadable)),
            };
            let mut file = match File::open(path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ReadOutcome::Vanished),
                Err(_) => return Ok(ReadOutcome::Failed(SkipReason::Unreadable)),
            };
            #[cfg(test)]
            FILE_READS.with(|reads| reads.set(reads.get() + 1));
            let size = before.len();
            let result = if store {
                self.store_object(&mut file, size, cancel).map(|o| o.sha256)
            } else {
                digest(&mut file, size, |_| Ok(()), cancel)
            };
            drop(file);
            let sha256 = match result {
                Ok(sha256) => sha256,
                Err(StoreError::SizeMismatch) => continue,
                Err(StoreError::Source(_)) => {
                    return Ok(ReadOutcome::Failed(SkipReason::Unreadable))
                }
                Err(StoreError::Cancelled) => return Err(cancelled()),
                Err(StoreError::Store(e)) => return Err(e),
            };
            let after = match fs::symlink_metadata(path) {
                Ok(meta) => meta,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(ReadOutcome::Vanished),
                Err(_) => return Ok(ReadOutcome::Failed(SkipReason::Unreadable)),
            };
            if after.len() != size || modified_ns(&after) != modified_ns(&before) {
                continue;
            }
            return Ok(ReadOutcome::Done(FileEntry {
                path: String::new(),
                size,
                sha256,
                modified_ns: modified_ns(&before),
                executable: is_executable(&before),
            }));
        }
        Ok(ReadOutcome::Failed(SkipReason::KeptChanging))
    }

    /// The manifest of a finished copy. `new_bytes` and `stored_bytes` count the objects
    /// none of this server's earlier backups use (never other servers', which would tell
    /// whether another server has a file).
    pub fn finish(
        &self,
        meta: BackupMeta,
        scan: &Scan,
        copied: Copied,
        duration_ms: u64,
    ) -> Manifest {
        let total_bytes = copied.files.iter().map(|f| f.size).sum();
        let mut counted = HashSet::new();
        let mut new_bytes = 0;
        let mut stored_bytes = 0;
        for file in &copied.files {
            if !scan.earlier_hashes.contains(&file.sha256) && counted.insert(file.sha256.as_str()) {
                new_bytes += file.size;
                stored_bytes += self.object_stored_size(&file.sha256).unwrap_or(0);
            }
        }
        Manifest {
            schema: MANIFEST_SCHEMA,
            id: meta.id,
            instance_uuid: meta.instance_uuid,
            instance_name: meta.instance_name,
            game_version: meta.game_version,
            flavour: meta.flavour,
            name: meta.name,
            kind: meta.kind,
            created: meta.created,
            scan_started_ns: scan.scan_started_ns,
            pinned: false,
            live: meta.live,
            quiesced: meta.quiesced,
            level_name: meta.level_name,
            full_scan: scan.full_scan,
            files: copied.files,
            skipped: copied.skipped,
            total_bytes,
            new_bytes,
            stored_bytes,
            duration_ms,
            core_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// The space a server's backups take. `manifests` are that server's.
    pub fn usage(&self, manifests: &[Manifest]) -> BackupUsage {
        let mut hashes = HashSet::new();
        for m in manifests {
            hashes.extend(m.files.iter().map(|f| f.sha256.as_str()));
        }
        BackupUsage {
            total_bytes: manifests.iter().map(|m| m.total_bytes).sum(),
            stored_bytes: hashes
                .into_iter()
                .filter_map(|h| self.object_stored_size(h))
                .sum(),
            disk_free: free_space(&self.root),
        }
    }

    // -----------------------------------------------------------------------------------
    // Garbage collection

    /// Deletes the objects no manifest of any server uses, and old temporary files.
    ///
    /// Refuses (deleting nothing) when any manifest can't be parsed or was made by a newer
    /// Lodestone: its objects can't be told apart. Run only with the store to itself.
    pub fn gc(&self) -> Result<GcReport, Error> {
        let mut used: HashSet<String> = HashSet::new();
        let servers = match fs::read_dir(self.servers_dir()) {
            Ok(entries) => entries.flatten().collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(Error::from(eyre!(e).wrap_err("Failed to list the backups"))),
        };
        for server in servers {
            if !server.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let entries = fs::read_dir(server.path()).context("Failed to list the backups")?;
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if !name.ends_with(".json") || name == POLICY_FILE || name == JOURNAL_FILE {
                    continue;
                }
                let refuse = |why: String| {
                    let message = format!(
                        "Not deleting unused backup files: {} {why}",
                        entry.path().display()
                    );
                    warn!("{message}");
                    err(ErrorKind::Internal, message)
                };
                let bytes =
                    fs::read(entry.path()).map_err(|e| refuse(format!("can't be read ({e})")))?;
                let hashes: GcManifest = serde_json::from_slice(&bytes)
                    .map_err(|e| refuse(format!("can't be parsed ({e})")))?;
                if hashes.schema > MANIFEST_SCHEMA {
                    return Err(refuse("was made by a newer Lodestone".to_string()));
                }
                used.extend(hashes.files.into_iter().map(|f| f.sha256));
            }
        }

        let mut report = GcReport::default();
        report.temp_removed += self.sweep_temp(TEMP_MAX_AGE);
        let Ok(dirs) = fs::read_dir(self.objects_dir()) else {
            return Ok(report);
        };
        for dir in dirs.flatten() {
            if !dir.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let Ok(files) = fs::read_dir(dir.path()) else {
                continue;
            };
            for file in files.flatten() {
                let name = file.file_name();
                let Some(name) = name.to_str() else { continue };
                if !valid_sha256(name) || used.contains(name) {
                    continue;
                }
                let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                match remove_file_forced(&file.path()) {
                    Ok(()) => {
                        report.objects_removed += 1;
                        report.bytes_removed += size;
                    }
                    Err(e) => warn!("Failed to delete the unused backup object {name}: {e}"),
                }
            }
        }
        if report.objects_removed > 0 {
            info!(
                "Deleted {} unused backup objects ({})",
                report.objects_removed,
                format_bytes(report.bytes_removed)
            );
        }
        Ok(report)
    }

    /// Deletes temporary object files older than `max_age` (left by a crash). Returns how
    /// many were deleted.
    pub fn sweep_temp(&self, max_age: Duration) -> u64 {
        let objects = self.objects_dir();
        let mut dirs = vec![objects.clone()];
        if let Ok(entries) = fs::read_dir(&objects) {
            dirs.extend(
                entries
                    .flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.path()),
            );
        }
        let now = SystemTime::now();
        let mut removed = 0;
        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let is_temp = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.ends_with(".tmp"));
                let old = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| now.duration_since(t).ok())
                    .is_some_and(|age| age >= max_age);
                if is_temp && old && remove_file_forced(&entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

fn object_present_at(path: &Path, size: u64) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|m| m.is_file() && m.len() > 0) {
        return false;
    }
    // a frame header is at most 18 bytes
    let mut header = [0u8; 18];
    let mut filled = 0;
    while filled < header.len() {
        match file.read(&mut header[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
    matches!(
        zstd::zstd_safe::get_frame_content_size(&header[..filled]),
        Ok(Some(content)) if content == size
    )
}

fn parse_manifest(bytes: &[u8], uuid: &str, id: &str) -> Result<Manifest, String> {
    #[derive(Deserialize)]
    struct Schema {
        schema: u32,
    }
    let schema: Schema = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if schema.schema > MANIFEST_SCHEMA {
        return Err(format!(
            "it was made by a newer Lodestone (schema {})",
            schema.schema
        ));
    }
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    validate_manifest(&manifest, uuid, id)?;
    Ok(manifest)
}

pub fn sort_newest_first(manifests: &mut [Manifest]) {
    manifests.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
}

/// Every 10th scheduled backup since the last full scan hashes everything, and so does the
/// first backup. `scheduled_since_full` is the count [`Store::scan`] found.
fn needs_full_scan(
    kind: BackupKind,
    newest_first: &[Manifest],
    scheduled_since_full: usize,
) -> bool {
    if newest_first.is_empty() {
        return true;
    }
    kind == BackupKind::Scheduled && scheduled_since_full + 1 >= FULL_SCAN_EVERY
}

/// The scheduled backups since the newest full scan, counted from the manifests left: for
/// a server whose count isn't recorded. Retention may have deleted the full scan, so this
/// counts every scheduled backup then.
fn scheduled_since_full_manifest(newest_first: &[Manifest]) -> usize {
    newest_first
        .iter()
        .take_while(|m| !m.full_scan)
        .filter(|m| m.kind == BackupKind::Scheduled)
        .count()
}

/// What GC reads of a manifest: as little as possible, so that it refuses only when it
/// can't tell which objects are used.
#[derive(Deserialize)]
struct GcManifest {
    schema: u32,
    #[serde(default)]
    files: Vec<GcFile>,
}

#[derive(Deserialize)]
struct GcFile {
    sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GcReport {
    pub objects_removed: u64,
    pub bytes_removed: u64,
    pub temp_removed: u64,
}

// ---------------------------------------------------------------------------------------
// Walking

/// What to back up.
#[derive(Clone, Copy)]
pub struct ScanRequest<'a> {
    pub server_root: &'a Path,
    pub uuid: &'a InstanceUuid,
    pub kind: BackupKind,
    /// The policy's exclude patterns.
    pub exclude: &'a Matcher,
}

/// A file the walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    pub path: String,
    pub size: u64,
    pub modified_ns: i128,
    pub executable: bool,
}

/// The result of walking a server folder: files whose hash the previous backup already
/// has, and files to read.
#[derive(Debug)]
pub struct Scan {
    pub server_root: PathBuf,
    pub scan_started_ns: i128,
    pub kind: BackupKind,
    /// Every file is read (no stat cache).
    pub full_scan: bool,
    /// Scheduled backups saved since the last full scan, before this one.
    scheduled_since_full: usize,
    /// The bytes of the files to read that the previous backup doesn't hold unchanged: what
    /// may be stored as new objects. In a full scan, much less than [`Scan::bytes_to_read`].
    bytes_to_store: u64,
    /// The newest backup of the server, used as the stat cache and for carrying over files
    /// that can't be read.
    pub previous: Option<Manifest>,
    /// Every object this server's backups use.
    earlier_hashes: HashSet<String>,
    pub cached: Vec<FileEntry>,
    /// The previous backup's files inside folders that can't be listed, kept as they were.
    /// Listed in `skipped` as carried over.
    pub carried: Vec<FileEntry>,
    pub to_read: Vec<FoundFile>,
    /// Links, names that can't be stored, and what is in folders that can't be listed.
    pub skipped: Vec<SkippedFile>,
}

/// Hashes of files read earlier in the same backup, by [`Store::prefetch`].
#[derive(Debug, Clone)]
pub struct Prefetched {
    /// When the walk those files were found by began: like a manifest's, a hash of a file
    /// modified within 2 s before it isn't trusted.
    pub scan_started_ns: i128,
    pub files: Vec<FileEntry>,
}

impl Scan {
    /// The bytes [`Store::copy`] will read.
    pub fn bytes_to_read(&self) -> u64 {
        self.to_read.iter().map(|f| f.size).sum()
    }

    /// The bytes [`Store::copy`] may store as new objects: what the free-space check needs.
    /// A full scan reads every file, but what is unchanged since the previous backup is
    /// already stored, so it isn't counted (or a full scan of a server bigger than the free
    /// space would always fail).
    pub fn bytes_to_store(&self) -> u64 {
        self.bytes_to_store
    }

    /// What [`Store::record_scheduled_since_full_scan`] should record once this backup is
    /// done (`saved` when its manifest was saved): 0 after a full scan, saved or not (it
    /// hashed every file even when nothing had changed), one more after a scheduled backup
    /// that was saved, and `None` when the count doesn't change.
    pub fn scheduled_since_full_after(&self, saved: bool) -> Option<usize> {
        if self.full_scan {
            Some(0)
        } else if saved && self.kind == BackupKind::Scheduled {
            Some(self.scheduled_since_full + 1)
        } else {
            None
        }
    }

    /// Whether a live backup should [`Store::prefetch`] before turning saving off.
    pub fn worth_prefetching(&self) -> bool {
        self.bytes_to_read() > PREFETCH_MIN_BYTES
    }

    /// False when the server certainly hasn't changed since the previous backup: every file
    /// came from the stat cache and the same paths are there. A scheduled backup sends no
    /// events until this is true.
    pub fn maybe_changed(&self) -> bool {
        let Some(previous) = &self.previous else {
            return true;
        };
        if !self.to_read.is_empty() {
            return true;
        }
        let mut cached: Vec<&FileEntry> = self.cached.iter().chain(&self.carried).collect();
        cached.sort_by(|a, b| a.path.cmp(&b.path));
        cached.len() != previous.files.len()
            || cached
                .iter()
                .zip(&previous.files)
                .any(|(a, b)| a.path != b.path || a.size != b.size || a.sha256 != b.sha256)
    }
}

/// The files of a backup, once read and stored.
#[derive(Debug, Clone)]
pub struct Copied {
    /// Sorted by path.
    pub files: Vec<FileEntry>,
    pub skipped: Vec<SkippedFile>,
}

impl Copied {
    /// Same paths, sizes and contents as `previous`: "Nothing changed since the last backup".
    pub fn unchanged_since(&self, previous: Option<&Manifest>) -> bool {
        previous.is_some_and(|m| same_files(&self.files, &m.files))
    }

    /// Files that couldn't be read ("N files couldn't be read").
    pub fn unreadable_count(&self) -> usize {
        self.skipped
            .iter()
            .filter(|s| matches!(s.reason, SkipReason::Unreadable | SkipReason::KeptChanging))
            .count()
    }
}

/// What a backup's manifest says beyond its files.
#[derive(Debug, Clone)]
pub struct BackupMeta {
    pub id: String,
    pub instance_uuid: InstanceUuid,
    pub instance_name: String,
    pub game_version: String,
    pub flavour: String,
    pub name: Option<String>,
    pub kind: BackupKind,
    pub created: i64,
    pub live: bool,
    pub quiesced: bool,
    pub level_name: Option<String>,
}

enum ReadOutcome {
    Done(FileEntry),
    /// Deleted since the walk.
    Vanished,
    Failed(SkipReason),
}

/// What [`walk`] found.
struct Walked {
    files: Vec<FoundFile>,
    /// Links and unstorable names, and the paths in `unlistable`.
    skipped: Vec<SkippedFile>,
    /// Folders (or entries) that couldn't be listed: what is inside them is unknown.
    unlistable: Vec<String>,
}

/// Walks `root` without following links below it. Returns the regular files to back up,
/// and the links, unstorable names and folders that can't be listed it skipped.
///
/// `root` itself may be a link or a junction to the server's folder (one moved to another
/// disk, say): it is followed, as every path a restore joins onto it is.
fn walk(root: &Path, exclude: &Matcher, cancel: Cancel) -> Result<Walked, Error> {
    if !fs::metadata(root).is_ok_and(|m| m.is_dir()) {
        return Err(err(ErrorKind::NotFound, "The server folder is missing"));
    }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut unlistable = Vec::new();
    #[cfg(windows)]
    let mut seen_folded: HashSet<String> = HashSet::new();
    let mut entries = walkdir::WalkDir::new(root)
        .follow_root_links(true)
        .follow_links(false)
        .min_depth(1)
        .into_iter();
    while let Some(entry) = entries.next() {
        if cancel() {
            return Err(cancelled());
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                // a folder that can't be listed
                if let Some(path) = e.path().and_then(|p| relative(root, p)) {
                    skipped.push(SkippedFile {
                        path: path.clone(),
                        reason: SkipReason::Unreadable,
                        carried_over: false,
                    });
                    unlistable.push(path);
                }
                continue;
            }
        };
        let file_type = entry.file_type();
        let is_dir = file_type.is_dir();
        let path = match relative(root, entry.path()) {
            Some(path) if valid_rel_path(&path) => path,
            _ => {
                let lossy = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                skipped.push(SkippedFile {
                    path: lossy,
                    reason: SkipReason::BadName,
                    carried_over: false,
                });
                if is_dir {
                    entries.skip_current_dir();
                }
                continue;
            }
        };
        if always_excluded(&path, is_dir) || exclude.is_excluded(&path, is_dir) {
            if is_dir {
                entries.skip_current_dir();
            }
            continue;
        }
        #[cfg(windows)]
        if !seen_folded.insert(path.to_lowercase()) {
            skipped.push(SkippedFile {
                path,
                reason: SkipReason::BadName,
                carried_over: false,
            });
            if is_dir {
                entries.skip_current_dir();
            }
            continue;
        }
        if file_type.is_symlink() {
            skipped.push(SkippedFile {
                path,
                reason: SkipReason::Symlink,
                carried_over: false,
            });
            continue;
        }
        if is_dir {
            continue;
        }
        // the listing can be stale for open files on Windows; ask the file itself
        let meta = match fs::symlink_metadata(entry.path()) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                skipped.push(SkippedFile {
                    path,
                    reason: SkipReason::Unreadable,
                    carried_over: false,
                });
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            skipped.push(SkippedFile {
                path,
                reason: SkipReason::Symlink,
                carried_over: false,
            });
            continue;
        }
        if !meta.is_file() {
            // fifos, sockets and devices: opening some of them would block
            skipped.push(SkippedFile {
                path,
                reason: SkipReason::Unreadable,
                carried_over: false,
            });
            continue;
        }
        files.push(FoundFile {
            path,
            size: meta.len(),
            modified_ns: modified_ns(&meta),
            executable: is_executable(&meta),
        });
    }
    Ok(Walked {
        files,
        skipped,
        unlistable,
    })
}

/// Keeps the previous backup's files inside folders the walk couldn't list, as a file that
/// can't be read is kept: those whose object is present, that the walk didn't find anyway
/// and that aren't excluded now. Returns them; in `skipped`, such a folder's entry is
/// replaced by one carried-over entry per kept file.
fn carry_over_unlistable(
    previous: Option<&Manifest>,
    unlistable: &[String],
    found: &HashSet<&str>,
    exclude: &Matcher,
    present: impl Fn(&FileEntry) -> bool,
    skipped: &mut Vec<SkippedFile>,
) -> Vec<FileEntry> {
    let mut carried: Vec<FileEntry> = Vec::new();
    let Some(previous) = previous else {
        return carried;
    };
    for folder in unlistable {
        let inside = |path: &str| {
            path == folder
                || path
                    .strip_prefix(folder.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        let kept: Vec<&FileEntry> = previous
            .files
            .iter()
            .filter(|f| {
                inside(&f.path)
                    && !found.contains(f.path.as_str())
                    && !carried.iter().any(|c| c.path == f.path)
                    && !always_excluded_path(&f.path, false)
                    && !exclude.is_excluded_path(&f.path, false)
                    && present(f)
            })
            .collect();
        if kept.is_empty() {
            continue;
        }
        skipped.retain(|s| s.path != *folder);
        for entry in kept {
            skipped.push(SkippedFile {
                path: entry.path.clone(),
                reason: SkipReason::Unreadable,
                carried_over: true,
            });
            carried.push(entry.clone());
        }
    }
    carried
}

/// `path` relative to `root`, `/`-separated, if it is valid UTF-8.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const NO_CANCEL: &dyn Fn() -> bool = &|| false;

    pub(crate) fn uuid() -> InstanceUuid {
        InstanceUuid::from("INSTANCE_test-1234".to_string())
    }

    pub(crate) fn write(root: &Path, path: &str, contents: &[u8]) {
        let full = join_rel(root, path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, contents).unwrap();
    }

    /// Sets a file's modified time well in the past, so the stat cache trusts it.
    pub(crate) fn age(root: &Path, path: &str) {
        let full = join_rel(root, path);
        let file = File::options().write(true).open(full).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
    }

    pub(crate) fn meta(id: &str, kind: BackupKind) -> BackupMeta {
        BackupMeta {
            id: id.to_string(),
            instance_uuid: uuid(),
            instance_name: "Test".to_string(),
            game_version: "1.21".to_string(),
            flavour: "vanilla".to_string(),
            name: None,
            kind,
            created: Utc::now().timestamp(),
            live: false,
            quiesced: true,
            level_name: Some("world".to_string()),
        }
    }

    /// Takes a backup of `root` the way the service does, and saves its manifest.
    pub(crate) fn backup(
        store: &Store,
        root: &Path,
        id: &str,
        kind: BackupKind,
        exclude: &Matcher,
    ) -> Manifest {
        let uuid = uuid();
        let scan = store
            .scan(
                ScanRequest {
                    server_root: root,
                    uuid: &uuid,
                    kind,
                    exclude,
                },
                NO_CANCEL,
            )
            .unwrap();
        let copied = store.copy(&scan, &mut |_, _| {}, NO_CANCEL).unwrap();
        let manifest = store.finish(meta(id, kind), &scan, copied, 1);
        store.save_manifest(&manifest).unwrap();
        if let Some(count) = scan.scheduled_since_full_after(true) {
            store
                .record_scheduled_since_full_scan(&uuid, count)
                .unwrap();
        }
        manifest
    }

    pub(crate) fn setup() -> (tempfile::TempDir, Store, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("backups"));
        let root = dir.path().join("server");
        fs::create_dir_all(&root).unwrap();
        (dir, store, root)
    }

    fn paths(m: &Manifest) -> Vec<&str> {
        m.files.iter().map(|f| f.path.as_str()).collect()
    }

    /// Creates a link to a folder: a symlink, or a junction on Windows (which needs no
    /// special rights). False when this system can't.
    pub(crate) fn link_dir(target: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(target, link).is_ok() {
                return true;
            }
            std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
        }
    }

    #[test]
    fn ids_and_names() {
        assert!(valid_server_id(
            "INSTANCE_6f1c2e7a-1234-4abc-9def-0123456789ab"
        ));
        for bad in [
            "",
            "..",
            "../x",
            "a/b",
            "a\\b",
            "C:",
            "a b",
            &"x".repeat(101),
        ] {
            assert!(!valid_server_id(bad), "{bad:?}");
        }
        let id = new_backup_id(Utc::now());
        assert!(valid_backup_id(&id), "{id}");
        assert_eq!(id.len(), "20261112-153000-abcd".len());
        for reserved in [
            "policy",
            "POLICY",
            "Policy",
            "restore-journal",
            "Restore-Journal",
            "RESTORE-JOURNAL",
            "con",
            "NUL",
            "Aux",
            "prn",
            "COM1",
            "lpt9",
        ] {
            assert!(!valid_backup_id(reserved), "{reserved:?}");
        }
        for fine in ["policy2", "console", "com10", "lpt", "nul-1", "a"] {
            assert!(valid_backup_id(fine), "{fine:?}");
        }
        assert!(!valid_backup_id("../x"));
        assert!(!valid_backup_id("a_b"));
        assert_eq!(check_backup_name("  "), Ok(None));
        assert_eq!(check_backup_name(" x "), Ok(Some("x".to_string())));
        assert!(check_backup_name(&"x".repeat(101)).is_err());
        assert_eq!(
            checked_level_name("worlds/survival").as_deref(),
            Some("worlds/survival")
        );
        assert_eq!(checked_level_name("../other"), None);
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(1_250_000_000), "1.2 GB");
    }

    #[test]
    fn rel_paths() {
        for good in [
            "a",
            "a/b.txt",
            "world/region/r.0.0.mca",
            ".lodestone_modpack/record.json",
        ] {
            assert!(valid_rel_path(good), "{good:?}");
        }
        for bad in [
            "", "/a", "a/", "a//b", ".", "a/./b", "..", "a/../b", "a\\b", "C:/x", "a\u{1}b",
        ] {
            assert!(!valid_rel_path(bad), "{bad:?}");
        }
    }

    fn manifest_with(paths: &[&str]) -> Manifest {
        Manifest {
            schema: 1,
            id: "b1".to_string(),
            instance_uuid: uuid(),
            instance_name: String::new(),
            game_version: String::new(),
            flavour: String::new(),
            name: None,
            kind: BackupKind::Manual,
            created: 0,
            scan_started_ns: 0,
            pinned: false,
            live: false,
            quiesced: true,
            level_name: None,
            full_scan: true,
            files: paths
                .iter()
                .map(|p| FileEntry {
                    path: p.to_string(),
                    size: 0,
                    sha256: "0".repeat(64),
                    modified_ns: 0,
                    executable: false,
                })
                .collect(),
            skipped: Vec::new(),
            total_bytes: 0,
            new_bytes: 0,
            stored_bytes: 0,
            duration_ms: 0,
            core_version: String::new(),
        }
    }

    #[test]
    fn manifest_validation() {
        let uuid = uuid();
        let ok = manifest_with(&["a", "b/c"]);
        validate_manifest(&ok, uuid.as_ref(), "b1").unwrap();
        assert!(validate_manifest(&ok, uuid.as_ref(), "b2").is_err());
        assert!(validate_manifest(&ok, "INSTANCE_other", "b1").is_err());
        for bad in [
            &["../../stores/users.json"][..],
            &["/etc/passwd"],
            &["a\\..\\b"],
            &[".lodestone_config"],
            &[".lodestone-restore-old-0123456789abcdef/x"],
            &["world/.lodestone-restore-x"],
            &["a", "a"],
            &["C:/x"],
        ] {
            let m = manifest_with(bad);
            assert!(
                validate_manifest(&m, uuid.as_ref(), "b1").is_err(),
                "{bad:?}"
            );
        }
        let mut newer = manifest_with(&["a"]);
        newer.schema = 2;
        assert!(validate_manifest(&newer, uuid.as_ref(), "b1").is_err());
        let mut bad_hash = manifest_with(&["a"]);
        bad_hash.files[0].sha256 = "ABC".to_string();
        assert!(validate_manifest(&bad_hash, uuid.as_ref(), "b1").is_err());
        if cfg!(windows) {
            assert!(validate_manifest(&manifest_with(&["A", "a"]), uuid.as_ref(), "b1").is_err());
        }
    }

    #[test]
    fn objects_round_trip_and_dedupe() {
        let (_dir, store, _root) = setup();
        let data = b"hello world".repeat(1000);
        let first = store
            .store_object(&mut &data[..], data.len() as u64, NO_CANCEL)
            .unwrap();
        assert!(first.added);
        assert!(store.object_present(&first.sha256, data.len() as u64));
        assert!(!store.object_present(&first.sha256, data.len() as u64 + 1));
        assert!(first.stored_size < data.len() as u64);
        // read-only on disk
        let meta = fs::metadata(store.object_path(&first.sha256)).unwrap();
        assert!(meta.permissions().readonly());
        let second = store
            .store_object(&mut &data[..], data.len() as u64, NO_CANCEL)
            .unwrap();
        assert!(!second.added);
        assert_eq!(first.sha256, second.sha256);
        let mut out = Vec::new();
        store
            .read_object(&first.sha256, data.len() as u64, &mut out, NO_CANCEL)
            .unwrap();
        assert_eq!(out, data);
        // empty files work too
        let empty = store.store_object(&mut &b""[..], 0, NO_CANCEL).unwrap();
        assert!(store.object_present(&empty.sha256, 0));
        let mut out = Vec::new();
        store
            .read_object(&empty.sha256, 0, &mut out, NO_CANCEL)
            .unwrap();
        assert!(out.is_empty());
        // no temporary files left
        let leftovers: Vec<_> = fs::read_dir(store.objects_dir())
            .unwrap()
            .flatten()
            .filter(|e| e.file_type().unwrap().is_file())
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn size_mismatch_is_reported() {
        let (_dir, store, _root) = setup();
        let data = b"12345";
        assert!(matches!(
            store.store_object(&mut &data[..], 4, NO_CANCEL),
            Err(StoreError::SizeMismatch)
        ));
        assert!(matches!(
            store.store_object(&mut &data[..], 6, NO_CANCEL),
            Err(StoreError::SizeMismatch)
        ));
        assert!(matches!(
            store.store_object(&mut &data[..], 5, &|| true),
            Err(StoreError::Cancelled)
        ));
    }

    #[test]
    fn damaged_objects_are_detected_and_replaced() {
        let (_dir, store, _root) = setup();
        let data = b"some world data".repeat(100);
        let size = data.len() as u64;
        let stored = store.store_object(&mut &data[..], size, NO_CANCEL).unwrap();
        let path = store.object_path(&stored.sha256);

        // flip a byte in the middle: the header still looks right, reading finds it
        let mut bytes = fs::read(&path).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0xff;
        remove_file_forced(&path).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut out = Vec::new();
        let error = store
            .read_object(&stored.sha256, size, &mut out, NO_CANCEL)
            .unwrap_err();
        assert!(matches!(error, ReadError::Damaged));
        assert_eq!(
            error.into_error("b1", "world/level.dat").source.to_string(),
            "Backup b1 is damaged: world/level.dat"
        );

        // garbage: not present, so storing the same contents replaces it
        remove_file_forced(&path).unwrap();
        fs::write(&path, b"garbage").unwrap();
        assert!(!store.object_present(&stored.sha256, size));
        let again = store.store_object(&mut &data[..], size, NO_CANCEL).unwrap();
        assert!(again.added);
        let mut out = Vec::new();
        store
            .read_object(&stored.sha256, size, &mut out, NO_CANCEL)
            .unwrap();
        assert_eq!(out, data);

        // missing
        remove_file_forced(&path).unwrap();
        assert!(matches!(
            store.read_object(&stored.sha256, size, &mut Vec::new(), NO_CANCEL),
            Err(ReadError::Damaged)
        ));
    }

    #[test]
    fn first_backup_stores_everything_and_second_reads_nothing() {
        let (_dir, store, root) = setup();
        write(&root, "server.properties", b"server-port=25565\n");
        write(&root, "world/level.dat", b"level");
        write(&root, "world/region/r.0.0.mca", &vec![7u8; 100_000]);
        for path in [
            "server.properties",
            "world/level.dat",
            "world/region/r.0.0.mca",
        ] {
            age(&root, path);
        }
        let none = Matcher::default();

        let reads = file_reads();
        let first = backup(&store, &root, "b1", BackupKind::Manual, &none);
        assert_eq!(file_reads() - reads, 3);
        assert!(first.full_scan);
        assert_eq!(
            paths(&first),
            vec![
                "server.properties",
                "world/level.dat",
                "world/region/r.0.0.mca"
            ]
        );
        assert_eq!(first.total_bytes, 18 + 5 + 100_000);
        assert_eq!(first.new_bytes, first.total_bytes);
        assert!(first.stored_bytes > 0 && first.stored_bytes < first.total_bytes);

        let reads = file_reads();
        let uuid = uuid();
        let scan = store
            .scan(
                ScanRequest {
                    server_root: &root,
                    uuid: &uuid,
                    kind: BackupKind::Manual,
                    exclude: &none,
                },
                NO_CANCEL,
            )
            .unwrap();
        assert!(!scan.maybe_changed());
        assert_eq!(scan.bytes_to_read(), 0);
        let copied = store.copy(&scan, &mut |_, _| {}, NO_CANCEL).unwrap();
        assert!(copied.unchanged_since(scan.previous.as_ref()));
        let second = store.finish(meta("b2", BackupKind::Manual), &scan, copied, 1);
        assert_eq!(file_reads() - reads, 0);
        assert!(!second.full_scan);
        assert_eq!(second.new_bytes, 0);
        assert_eq!(second.stored_bytes, 0);
        assert!(same_files(&first.files, &second.files));
    }

    #[test]
    fn a_changed_file_stores_one_object() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"one");
        write(&root, "b.txt", b"two");
        age(&root, "a.txt");
        age(&root, "b.txt");
        let none = Matcher::default();
        let first = backup(&store, &root, "b1", BackupKind::Manual, &none);
        write(&root, "b.txt", b"three");
        age(&root, "b.txt");
        let objects_before = count_objects(&store);
        let reads = file_reads();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        assert_eq!(file_reads() - reads, 1);
        assert_eq!(count_objects(&store), objects_before + 1);
        assert_eq!(second.new_bytes, 5);
        assert_ne!(first.files[1].sha256, second.files[1].sha256);
        assert_eq!(first.files[0].sha256, second.files[0].sha256);
    }

    fn count_objects(store: &Store) -> usize {
        walkdir::WalkDir::new(store.objects_dir())
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file() && valid_sha256(&e.file_name().to_string_lossy()))
            .count()
    }

    #[test]
    fn racy_entries_are_hashed_again() {
        let (_dir, store, root) = setup();
        // modified just now: within 2 s of the first scan
        write(&root, "plugin.db", b"aaaa");
        let none = Matcher::default();
        backup(&store, &root, "b1", BackupKind::Manual, &none);
        // same size and modified time, different contents
        let full = root.join("plugin.db");
        let mtime = fs::metadata(&full).unwrap().modified().unwrap();
        fs::write(&full, b"bbbb").unwrap();
        File::options()
            .write(true)
            .open(&full)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        let reads = file_reads();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        assert_eq!(file_reads() - reads, 1);
        let mut out = Vec::new();
        store
            .read_object(&second.files[0].sha256, 4, &mut out, NO_CANCEL)
            .unwrap();
        assert_eq!(out, b"bbbb");
    }

    #[test]
    fn every_tenth_scheduled_backup_is_a_full_scan() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        age(&root, "a.txt");
        let none = Matcher::default();
        let mut full = Vec::new();
        for i in 0..21 {
            let m = backup(
                &store,
                &root,
                &format!("b{i:02}"),
                BackupKind::Scheduled,
                &none,
            );
            full.push(m.full_scan);
            // keep `created` increasing so newest-first is well defined
            let mut m = m;
            m.created += i;
            store.save_manifest(&m).unwrap();
        }
        let expected: Vec<bool> = (0..21).map(|i| i % 10 == 0 || i == 0).collect();
        assert_eq!(full, expected);
        // manual backups never force one
        let m = backup(&store, &root, "manual", BackupKind::Manual, &none);
        assert!(!m.full_scan);
    }

    #[test]
    fn full_scans_go_on_when_retention_deleted_the_last_one() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        age(&root, "a.txt");
        let none = Matcher::default();
        let mut full = Vec::new();
        for i in 0..25 {
            let mut m = backup(
                &store,
                &root,
                &format!("b{i:02}"),
                BackupKind::Scheduled,
                &none,
            );
            full.push(m.full_scan);
            m.created += i;
            store.save_manifest(&m).unwrap();
            // keep_last 3, nothing else: the full scan is soon deleted
            for old in store.list_manifests(&uuid()).unwrap().iter().skip(3) {
                store.delete_manifest(&uuid(), &old.id).unwrap();
            }
        }
        let expected: Vec<bool> = (0..25).map(|i| i % 10 == 0).collect();
        assert_eq!(full, expected);
    }

    #[test]
    fn a_full_scan_that_finds_nothing_changed_still_counts() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        age(&root, "a.txt");
        let none = Matcher::default();
        let uuid = uuid();
        backup(&store, &root, "b00", BackupKind::Manual, &none);
        store.record_scheduled_since_full_scan(&uuid, 9).unwrap();
        let request = ScanRequest {
            server_root: &root,
            uuid: &uuid,
            kind: BackupKind::Scheduled,
            exclude: &none,
        };
        let scan = store.scan(request, NO_CANCEL).unwrap();
        assert!(scan.full_scan);
        let copied = store.copy(&scan, &mut |_, _| {}, NO_CANCEL).unwrap();
        // nothing changed, so the service saves no backup, but records the full scan
        assert!(copied.unchanged_since(scan.previous.as_ref()));
        assert_eq!(scan.scheduled_since_full_after(false), Some(0));
        store.record_scheduled_since_full_scan(&uuid, 0).unwrap();
        let scan = store.scan(request, NO_CANCEL).unwrap();
        assert!(!scan.full_scan);
        // an incremental backup that saved nothing doesn't count
        assert_eq!(scan.scheduled_since_full_after(false), None);
        assert_eq!(scan.scheduled_since_full_after(true), Some(1));
    }

    #[test]
    fn the_count_of_scheduled_backups_falls_back_to_the_manifests() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        age(&root, "a.txt");
        let none = Matcher::default();
        for i in 0..9 {
            let mut m = backup(
                &store,
                &root,
                &format!("b{i:02}"),
                BackupKind::Scheduled,
                &none,
            );
            m.created += i;
            store.save_manifest(&m).unwrap();
        }
        // a count file that can't be read
        let count = store
            .server_dir(&uuid())
            .unwrap()
            .join(FULL_SCAN_COUNT_FILE);
        fs::write(&count, "garbage").unwrap();
        let m = backup(&store, &root, "b09", BackupKind::Scheduled, &none);
        assert!(
            !m.full_scan,
            "8 scheduled backups since the full scan, and this one"
        );
        fs::remove_file(&count).unwrap();
        let mut m = m;
        m.created += 9;
        store.save_manifest(&m).unwrap();
        let m = backup(&store, &root, "b10", BackupKind::Scheduled, &none);
        assert!(m.full_scan);
        // neither listing nor GC takes the count for a backup
        assert_eq!(fs::read_to_string(&count).unwrap(), "0");
        assert_eq!(store.list_manifests(&uuid()).unwrap().len(), 11);
        store.gc().unwrap();
    }

    #[test]
    fn a_full_scan_needs_free_space_only_for_what_changed() {
        let (_dir, store, root) = setup();
        write(&root, "big.dat", &vec![7u8; 100_000]);
        write(&root, "small.txt", b"one");
        age(&root, "big.dat");
        age(&root, "small.txt");
        let none = Matcher::default();
        let uuid = uuid();
        let first = backup(&store, &root, "b00", BackupKind::Scheduled, &none);
        assert!(first.full_scan);
        store.record_scheduled_since_full_scan(&uuid, 9).unwrap();
        write(&root, "small.txt", b"two!");
        age(&root, "small.txt");
        let scan = store
            .scan(
                ScanRequest {
                    server_root: &root,
                    uuid: &uuid,
                    kind: BackupKind::Scheduled,
                    exclude: &none,
                },
                NO_CANCEL,
            )
            .unwrap();
        assert!(scan.full_scan);
        assert_eq!(scan.bytes_to_read(), 100_004);
        assert_eq!(scan.bytes_to_store(), 4);
    }

    #[test]
    fn exclusions_are_left_out() {
        let (_dir, store, root) = setup();
        for path in [
            ".lodestone_config",
            ".lodestone_minecraft_config.json",
            "logs/latest.log",
            "crash-reports/x.txt",
            "cache/x",
            ".lodestone_modpack/objects/ab/x",
            ".lodestone_modpack/record.json",
            ".lodestone-restore-old-0123456789abcdef/world/level.dat",
            "world/session.lock",
            "world/level.dat",
            "plugins/dynmap/web/tiles/0/0.png",
            "plugins/dynmap/config.txt",
            "old.zip",
        ] {
            write(&root, path, b"x");
        }
        let exclude = Matcher::with_case(
            &["plugins/dynmap/web/tiles/".to_string(), "*.zip".to_string()],
            false,
        );
        let m = backup(&store, &root, "b1", BackupKind::Manual, &exclude);
        assert_eq!(
            paths(&m),
            vec![
                ".lodestone_minecraft_config.json",
                ".lodestone_modpack/record.json",
                "plugins/dynmap/config.txt",
                "world/level.dat",
            ]
        );
    }

    #[test]
    fn symlinks_are_skipped_and_recorded() {
        let (dir, store, root) = setup();
        let outside = dir.path().join("outside");
        write(&outside, "secret.txt", b"secret");
        write(&root, "world/level.dat", b"x");
        if !link_dir(&outside, &root.join("linked")) {
            eprintln!("can't create links here; skipping");
            return;
        }
        let m = backup(&store, &root, "b1", BackupKind::Manual, &Matcher::default());
        assert_eq!(paths(&m), vec!["world/level.dat"]);
        assert_eq!(
            m.skipped,
            vec![SkippedFile {
                path: "linked".to_string(),
                reason: SkipReason::Symlink,
                carried_over: false
            }]
        );
    }

    /// A server folder that is itself a link or a junction is backed up through it; links
    /// below it are still skipped.
    #[test]
    fn a_linked_server_folder_is_followed_but_links_inside_it_are_not() {
        let (dir, store, root) = setup();
        let elsewhere = dir.path().join("elsewhere");
        write(&elsewhere, "world/level.dat", b"level");
        write(&elsewhere, "server.jar", b"jar");
        let outside = dir.path().join("outside");
        write(&outside, "secret.txt", b"secret");
        fs::remove_dir(&root).unwrap();
        if !link_dir(&elsewhere, &root) || !link_dir(&outside, &elsewhere.join("linked")) {
            eprintln!("can't create links here; skipping");
            return;
        }
        let m = backup(&store, &root, "b1", BackupKind::Manual, &Matcher::default());
        assert_eq!(paths(&m), vec!["server.jar", "world/level.dat"]);
        assert_eq!(
            m.skipped,
            vec![SkippedFile {
                path: "linked".to_string(),
                reason: SkipReason::Symlink,
                carried_over: false
            }]
        );
        let current = store
            .hash_current(
                ScanRequest {
                    server_root: &root,
                    uuid: &uuid(),
                    kind: BackupKind::Manual,
                    exclude: &Matcher::default(),
                },
                NO_CANCEL,
            )
            .unwrap();
        assert_eq!(current.len(), 2);

        // a link to a file is no server folder
        fs::write(dir.path().join("file"), b"x").unwrap();
        let to_file = dir.path().join("to-file");
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(dir.path().join("file"), &to_file).is_ok();
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(dir.path().join("file"), &to_file).is_ok();
        if made {
            assert!(walk(&to_file, &Matcher::default(), NO_CANCEL).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_is_carried_over_or_skipped() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            // root reads everything
            return;
        }
        let (_dir, store, root) = setup();
        write(&root, "plugins/LuckPerms/luckperms-h2.mv.db", b"perms");
        write(&root, "new.db", b"new");
        age(&root, "plugins/LuckPerms/luckperms-h2.mv.db");
        let none = Matcher::default();
        // the first backup reads it; new.db is made unreadable before it exists in a backup
        fs::set_permissions(root.join("new.db"), fs::Permissions::from_mode(0o000)).unwrap();
        let first = backup(&store, &root, "b1", BackupKind::Manual, &none);
        assert_eq!(paths(&first), vec!["plugins/LuckPerms/luckperms-h2.mv.db"]);
        assert_eq!(first.skipped[0].path, "new.db");
        assert!(!first.skipped[0].carried_over);
        // now the database changes and becomes unreadable
        let db = root.join("plugins/LuckPerms/luckperms-h2.mv.db");
        fs::write(&db, b"perms2").unwrap();
        fs::set_permissions(&db, fs::Permissions::from_mode(0o000)).unwrap();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        assert_eq!(second.files, first.files);
        let carried = second
            .skipped
            .iter()
            .find(|s| s.path.ends_with("mv.db"))
            .unwrap();
        assert!(carried.carried_over);
        assert_eq!(carried.reason, SkipReason::Unreadable);
        fs::set_permissions(&db, fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(root.join("new.db"), fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn locked_file_is_carried_over_or_skipped() {
        use std::os::windows::fs::OpenOptionsExt;
        let (_dir, store, root) = setup();
        write(&root, "plugins/LuckPerms/luckperms-h2.mv.db", b"perms");
        write(&root, "new.db", b"new");
        age(&root, "plugins/LuckPerms/luckperms-h2.mv.db");
        let none = Matcher::default();
        // share mode 0: nobody else may open it, like a database holding its file
        let held = File::options()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(root.join("new.db"))
            .unwrap();
        let first = backup(&store, &root, "b1", BackupKind::Manual, &none);
        assert_eq!(paths(&first), vec!["plugins/LuckPerms/luckperms-h2.mv.db"]);
        assert_eq!(first.skipped.len(), 1);
        assert_eq!(first.skipped[0].path, "new.db");
        assert!(!first.skipped[0].carried_over);
        drop(held);
        let db = root.join("plugins/LuckPerms/luckperms-h2.mv.db");
        fs::write(&db, b"perms2").unwrap();
        let held = File::options()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&db)
            .unwrap();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        drop(held);
        let entry = second
            .files
            .iter()
            .find(|f| f.path.ends_with("mv.db"))
            .unwrap();
        assert_eq!(entry, &first.files[0]);
        let carried = second
            .skipped
            .iter()
            .find(|s| s.path.ends_with("mv.db"))
            .unwrap();
        assert!(carried.carried_over);
        assert_eq!(carried.reason, SkipReason::Unreadable);
    }

    #[test]
    fn new_bytes_count_only_this_server() {
        let (dir, store, root) = setup();
        write(&root, "same.txt", b"shared contents");
        backup(&store, &root, "b1", BackupKind::Manual, &Matcher::default());
        // another server with the same file: its first backup counts it as new
        let other_root = dir.path().join("other");
        write(&other_root, "same.txt", b"shared contents");
        let other = InstanceUuid::from("INSTANCE_other".to_string());
        let scan = store
            .scan(
                ScanRequest {
                    server_root: &other_root,
                    uuid: &other,
                    kind: BackupKind::Manual,
                    exclude: &Matcher::default(),
                },
                NO_CANCEL,
            )
            .unwrap();
        let copied = store.copy(&scan, &mut |_, _| {}, NO_CANCEL).unwrap();
        let mut m = meta("b1", BackupKind::Manual);
        m.instance_uuid = other.clone();
        let manifest = store.finish(m, &scan, copied, 1);
        assert_eq!(manifest.new_bytes, 15);
        assert!(manifest.stored_bytes > 0);
        store.save_manifest(&manifest).unwrap();
        let usage = store.usage(&store.list_manifests(&other).unwrap());
        assert_eq!(usage.total_bytes, 15);
        assert_eq!(usage.stored_bytes, manifest.stored_bytes);
    }

    #[test]
    fn list_skips_invalid_manifests_newest_first() {
        let (_dir, store, root) = setup();
        write(&root, "a", b"a");
        let mut old = backup(&store, &root, "b1", BackupKind::Manual, &Matcher::default());
        old.created -= 100;
        store.save_manifest(&old).unwrap();
        backup(&store, &root, "b2", BackupKind::Manual, &Matcher::default());
        let dir = store.server_dir(&uuid()).unwrap();
        fs::write(dir.join("broken.json"), b"{not json").unwrap();
        // a manifest copied under another name doesn't match its id
        fs::copy(dir.join("b1.json"), dir.join("b3.json")).unwrap();
        store
            .save_policy(&uuid(), &BackupPolicy::default())
            .unwrap();
        let ids: Vec<String> = store
            .list_manifests(&uuid())
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, vec!["b2", "b1"]);
        assert_eq!(
            store
                .load_manifest(&uuid(), "nope")
                .unwrap_err()
                .kind
                .to_string(),
            "Not Found"
        );
        store.delete_manifest(&uuid(), "b2").unwrap();
        assert!(store.delete_manifest(&uuid(), "b2").is_err());
        assert!(store
            .server_dir(&InstanceUuid::from("../../stores".to_string()))
            .is_err());
    }

    #[test]
    fn policy_round_trip() {
        let (_dir, store, _root) = setup();
        assert_eq!(store.load_policy(&uuid()).unwrap(), BackupPolicy::default());
        let policy = BackupPolicy {
            enabled: true,
            keep_last: 3,
            ..BackupPolicy::default()
        };
        store.save_policy(&uuid(), &policy).unwrap();
        assert_eq!(store.load_policy(&uuid()).unwrap(), policy);
        let bad = BackupPolicy {
            keep_last: 0,
            ..BackupPolicy::default()
        };
        assert!(store.save_policy(&uuid(), &bad).is_err());
    }

    #[test]
    fn reserved_names_in_any_case_are_not_backup_ids() {
        let (_dir, store, _root) = setup();
        store
            .save_policy(&uuid(), &BackupPolicy::default())
            .unwrap();
        let journal = store.journal_path(&uuid()).unwrap();
        fs::write(&journal, b"{}").unwrap();
        for id in ["POLICY", "Policy", "Restore-Journal", "RESTORE-JOURNAL"] {
            let error = store.delete_manifest(&uuid(), id).unwrap_err();
            assert!(matches!(error.kind, ErrorKind::BadRequest), "{id}");
            let error = store.load_manifest(&uuid(), id).unwrap_err();
            assert!(matches!(error.kind, ErrorKind::BadRequest), "{id}");
        }
        assert!(store.policy_path(&uuid()).unwrap().is_file());
        assert!(journal.is_file());
    }

    /// Makes the object for `data` garbage, read-only like a real object.
    fn damage(store: &Store, sha256: &str) -> PathBuf {
        let path = store.object_path(sha256);
        remove_file_forced(&path).unwrap();
        fs::write(&path, b"garbage").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn a_read_only_damaged_object_is_replaced_in_place() {
        let (_dir, store, _root) = setup();
        let data = b"some world data".repeat(100);
        let size = data.len() as u64;
        let stored = store.store_object(&mut &data[..], size, NO_CANCEL).unwrap();
        let path = damage(&store, &stored.sha256);
        let again = store.store_object(&mut &data[..], size, NO_CANCEL).unwrap();
        assert!(again.added);
        assert!(store.object_present(&stored.sha256, size));
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        let mut out = Vec::new();
        store
            .read_object(&stored.sha256, size, &mut out, NO_CANCEL)
            .unwrap();
        assert_eq!(out, data);
        // nothing is left behind
        let objects = store.root().join("objects");
        let temps = fs::read_dir(&objects)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temps, 0);
    }

    #[test]
    fn an_object_another_job_stores_meanwhile_is_kept() {
        let (_dir, store, _root) = setup();
        let data = b"paper.jar".repeat(1000);
        let size = data.len() as u64;
        let stored = store.store_object(&mut &data[..], size, NO_CANCEL).unwrap();
        let path = store.object_path(&stored.sha256);
        let good = fs::read(&path).unwrap();
        damage(&store, &stored.sha256);

        // right after this job finds the object damaged, another job stores it
        let replaced_by_other = std::rc::Rc::new(std::cell::Cell::new(None));
        let seen = replaced_by_other.clone();
        AFTER_PRESENCE_CHECK.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move |final_path: &Path| {
                let temp = final_path.with_file_name("other-job.tmp");
                fs::write(&temp, &good).unwrap();
                clear_readonly(final_path);
                fs::rename(&temp, final_path).unwrap();
                seen.set(Some(fs::metadata(final_path).unwrap().modified().unwrap()));
            }));
        });
        let again = store.store_object(&mut &data[..], size, NO_CANCEL);
        AFTER_PRESENCE_CHECK.with(|hook| *hook.borrow_mut() = None);
        let again = again.unwrap();
        assert!(!again.added, "the other job's object is used as it is");
        assert_eq!(
            Some(fs::metadata(&path).unwrap().modified().unwrap()),
            replaced_by_other.get()
        );
        let mut out = Vec::new();
        store
            .read_object(&stored.sha256, size, &mut out, NO_CANCEL)
            .unwrap();
        assert_eq!(out, data);
    }

    fn entry(path: &str, sha256: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            size: 1,
            sha256: sha256.to_string(),
            modified_ns: 0,
            executable: false,
        }
    }

    #[test]
    fn files_in_folders_that_cant_be_listed_are_carried_over() {
        let mut previous = manifest_with(&[]);
        previous.files = vec![
            entry("plugins/Foo/data/a.yml", "a"),
            entry("plugins/Foo/data/sub/b.yml", "b"),
            entry("plugins/Foo/data/gone.yml", "gone"),
            entry("plugins/Foo/data/found.yml", "found"),
            entry("plugins/Foo/data/cache/c.bin", "c"),
            entry("plugins/Foo/database.db", "outside"),
            entry("plugins/Bar/x.yml", "x"),
            entry("locked.dat", "locked"),
        ];
        let unlistable = vec![
            "plugins/Foo/data".to_string(),
            "plugins/Baz".to_string(),
            "locked.dat".to_string(),
        ];
        let found: HashSet<&str> = ["plugins/Foo/data/found.yml"].into_iter().collect();
        let exclude = Matcher::new(&["plugins/Foo/data/cache/".to_string()]);
        let mut skipped: Vec<SkippedFile> = unlistable
            .iter()
            .map(|path| SkippedFile {
                path: path.clone(),
                reason: SkipReason::Unreadable,
                carried_over: false,
            })
            .collect();
        let carried = carry_over_unlistable(
            Some(&previous),
            &unlistable,
            &found,
            &exclude,
            |f| f.sha256 != "gone",
            &mut skipped,
        );
        let carried: Vec<&str> = carried.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            carried,
            vec![
                "plugins/Foo/data/a.yml",
                "plugins/Foo/data/sub/b.yml",
                "locked.dat"
            ]
        );
        skipped.sort_by(|a, b| a.path.cmp(&b.path));
        let skipped: Vec<(&str, bool)> = skipped
            .iter()
            .map(|s| (s.path.as_str(), s.carried_over))
            .collect();
        assert_eq!(
            skipped,
            vec![
                ("locked.dat", true),
                ("plugins/Baz", false),
                ("plugins/Foo/data/a.yml", true),
                ("plugins/Foo/data/sub/b.yml", true),
            ]
        );
        // nothing to carry over without a previous backup
        let mut skipped = Vec::new();
        assert!(
            carry_over_unlistable(None, &unlistable, &found, &exclude, |_| true, &mut skipped)
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cant_be_listed_keeps_its_files() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            // root lists everything
            return;
        }
        let (_dir, store, root) = setup();
        write(&root, "plugins/Foo/data/a.yml", b"a");
        write(&root, "plugins/Foo/data/sub/b.yml", b"b");
        write(&root, "server.properties", b"x");
        let none = Matcher::default();
        let first = backup(&store, &root, "b1", BackupKind::Manual, &none);
        let folder = root.join("plugins/Foo/data");
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o000)).unwrap();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(same_files(&second.files, &first.files));
        let skipped: Vec<(&str, bool)> = second
            .skipped
            .iter()
            .map(|s| (s.path.as_str(), s.carried_over))
            .collect();
        assert_eq!(
            skipped,
            vec![
                ("plugins/Foo/data/a.yml", true),
                ("plugins/Foo/data/sub/b.yml", true)
            ]
        );
    }

    #[test]
    fn prefetched_files_are_not_read_again() {
        let (_dir, store, root) = setup();
        write(&root, "world/region/r.0.0.mca", &vec![1u8; 50_000]);
        write(&root, "world/region/r.0.1.mca", &vec![2u8; 50_000]);
        write(&root, "world/level.dat", b"level");
        for path in [
            "world/region/r.0.0.mca",
            "world/region/r.0.1.mca",
            "world/level.dat",
        ] {
            age(&root, path);
        }
        let none = Matcher::default();
        let uuid = uuid();
        let request = ScanRequest {
            server_root: &root,
            uuid: &uuid,
            kind: BackupKind::Scheduled,
            exclude: &none,
        };
        // the first backup is a full scan: everything is to read
        let scan = store.scan(request, NO_CANCEL).unwrap();
        assert!(scan.full_scan);
        assert_eq!(scan.to_read.len(), 3);
        assert!(!scan.worth_prefetching());
        let reads = file_reads();
        let mut reported = 0;
        let prefetched = store
            .prefetch(&scan, &mut |done, _| reported = done, NO_CANCEL)
            .unwrap();
        assert_eq!(file_reads() - reads, 3);
        assert_eq!(reported, scan.bytes_to_read());
        assert_eq!(prefetched.files.len(), 3);

        // the server writes one file before saving is turned off
        write(&root, "world/level.dat", b"level 2");
        age(&root, "world/level.dat");
        let reads = file_reads();
        let scan = store
            .scan_with(request, Some(&prefetched), NO_CANCEL)
            .unwrap();
        assert!(scan.full_scan);
        let to_read: Vec<&str> = scan.to_read.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(to_read, vec!["world/level.dat"]);
        let copied = store.copy(&scan, &mut |_, _| {}, NO_CANCEL).unwrap();
        assert_eq!(file_reads() - reads, 1);
        let manifest = store.finish(meta("b1", BackupKind::Scheduled), &scan, copied, 1);
        assert_eq!(manifest.files.len(), 3);
        for file in &manifest.files {
            let mut out = Vec::new();
            store
                .read_object(&file.sha256, file.size, &mut out, NO_CANCEL)
                .unwrap();
            assert_eq!(out, fs::read(join_rel(&root, &file.path)).unwrap());
        }

        // a hash isn't trusted for a file modified just before the prefetch's walk
        let racy = Prefetched {
            scan_started_ns: unix_ns(SystemTime::now()),
            files: prefetched.files.clone(),
        };
        let fresh = root.join("world/region/r.0.0.mca");
        File::options()
            .write(true)
            .open(&fresh)
            .unwrap()
            .set_modified(SystemTime::now())
            .unwrap();
        let mut racy_files = racy.files.clone();
        for f in &mut racy_files {
            if f.path == "world/region/r.0.0.mca" {
                f.modified_ns = modified_ns(&fs::metadata(&fresh).unwrap());
            }
        }
        let racy = Prefetched {
            files: racy_files,
            ..racy
        };
        let scan = store.scan_with(request, Some(&racy), NO_CANCEL).unwrap();
        let to_read: Vec<&str> = scan.to_read.iter().map(|f| f.path.as_str()).collect();
        assert!(to_read.contains(&"world/region/r.0.0.mca"), "{to_read:?}");
    }

    #[test]
    fn gc_removes_only_unreferenced_objects() {
        let (_dir, store, root) = setup();
        write(&root, "keep.txt", b"keep");
        write(&root, "gone.txt", b"gone");
        let none = Matcher::default();
        backup(&store, &root, "b1", BackupKind::Manual, &none);
        fs::remove_file(root.join("gone.txt")).unwrap();
        let second = backup(&store, &root, "b2", BackupKind::Manual, &none);
        assert_eq!(count_objects(&store), 2);
        // nothing to delete while b1 exists
        assert_eq!(store.gc().unwrap().objects_removed, 0);
        store.delete_manifest(&uuid(), "b1").unwrap();
        // an old temp file goes, a fresh one stays
        let objects = store.objects_dir();
        let old_temp = objects.join(".incoming-old.tmp");
        fs::write(&old_temp, b"x").unwrap();
        File::options()
            .write(true)
            .open(&old_temp)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(7200))
            .unwrap();
        let new_temp = objects.join(".incoming-new.tmp");
        fs::write(&new_temp, b"x").unwrap();
        let report = store.gc().unwrap();
        assert_eq!(report.objects_removed, 1);
        assert_eq!(report.temp_removed, 1);
        assert!(!old_temp.exists());
        assert!(new_temp.exists());
        assert_eq!(count_objects(&store), 1);
        assert!(store.object_present(&second.files[0].sha256, 4));
    }

    #[test]
    fn gc_refuses_with_a_corrupt_or_newer_manifest() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        let none = Matcher::default();
        backup(&store, &root, "b1", BackupKind::Manual, &none);
        store.delete_manifest(&uuid(), "b1").unwrap();
        let dir = store.server_dir(&uuid()).unwrap();
        fs::write(dir.join("b9.json"), b"{broken").unwrap();
        assert!(store.gc().is_err());
        assert_eq!(count_objects(&store), 1);
        fs::write(dir.join("b9.json"), br#"{"schema": 2, "files": []}"#).unwrap();
        assert!(store.gc().is_err());
        assert_eq!(count_objects(&store), 1);
        fs::remove_file(dir.join("b9.json")).unwrap();
        assert_eq!(store.gc().unwrap().objects_removed, 1);
    }

    #[test]
    fn cancellation_stops_the_walk_and_copy() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        let uuid = uuid();
        let none = Matcher::default();
        let request = ScanRequest {
            server_root: &root,
            uuid: &uuid,
            kind: BackupKind::Manual,
            exclude: &none,
        };
        assert!(store.scan(request, &|| true).is_err());
        let scan = store.scan(request, NO_CANCEL).unwrap();
        assert!(store.copy(&scan, &mut |_, _| {}, &|| true).is_err());
    }

    #[test]
    fn hash_current_matches_a_backup() {
        let (_dir, store, root) = setup();
        write(&root, "a.txt", b"a");
        write(&root, "b/c.txt", b"c");
        let uuid = uuid();
        let none = Matcher::default();
        let files = store
            .hash_current(
                ScanRequest {
                    server_root: &root,
                    uuid: &uuid,
                    kind: BackupKind::Manual,
                    exclude: &none,
                },
                NO_CANCEL,
            )
            .unwrap();
        assert_eq!(count_objects(&store), 0);
        let m = backup(&store, &root, "b1", BackupKind::Manual, &none);
        assert!(same_files(&files, &m.files));
    }

    #[test]
    fn free_space_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let free = free_space(&dir.path().join("does/not/exist/yet"));
        assert!(free.is_some_and(|f| f > 0), "{free:?}");
        assert!(require_free_space(dir.path(), 0, "for a backup").is_ok());
        let error = require_free_space(dir.path(), u64::MAX / 2, "for a backup").unwrap_err();
        assert!(error
            .source
            .to_string()
            .starts_with("Not enough free disk space for a backup: "));
    }
}
