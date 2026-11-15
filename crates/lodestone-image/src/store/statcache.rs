//! The stat cache: hashes from the last snapshot, reused for files whose size and
//! modification time haven't changed, so a snapshot of an unchanged world reads nothing.
//!
//! A file's size and modification time can stay the same across a change made in the same
//! tick of the file system's clock as the last read (a "racy" file, in git's words). So a
//! cached hash is only trusted for a file modified at least [`RACY_NS`] before the scan
//! that cached it started: a later change would have moved its time past that. The core's
//! backups have worked this way since they began.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hex::is_sha256;

/// Files modified less than this (2 s) before the scan that cached them are hashed again.
pub const RACY_NS: i64 = 2_000_000_000;

const VERSION: u32 = 1;

/// One file as the last scan found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedFile {
    pub size: u64,
    /// Unix nanoseconds.
    pub modified_ns: i64,
    pub sha256: String,
}

/// What the last snapshot of an instance read, by path relative to the folder it walked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatCache {
    pub version: u32,
    /// When that scan started, unix nanoseconds.
    pub scan_started_ns: i64,
    pub files: BTreeMap<String, CachedFile>,
}

impl StatCache {
    /// An empty cache for a scan that started at `scan_started_ns`.
    pub fn new(scan_started_ns: i64) -> StatCache {
        StatCache {
            version: VERSION,
            scan_started_ns,
            files: BTreeMap::new(),
        }
    }

    /// The cached hash of the file at `path`, if it has the same size and modification
    /// time as then and was modified safely before that scan started.
    pub fn lookup(&self, path: &str, size: u64, modified_ns: i64) -> Option<&str> {
        let cached = self.files.get(path)?;
        let trusted_before = self.scan_started_ns.saturating_sub(RACY_NS);
        (cached.size == size && cached.modified_ns == modified_ns && modified_ns < trusted_before)
            .then_some(cached.sha256.as_str())
    }

    pub fn insert(&mut self, path: String, size: u64, modified_ns: i64, sha256: String) {
        self.files.insert(
            path,
            CachedFile {
                size,
                modified_ns,
                sha256,
            },
        );
    }

    /// Reads a saved cache; `None` when it isn't one this version wrote, or holds a hash
    /// that isn't one. A cache is only ever an optimization, so anything odd means "none".
    pub fn from_json(bytes: &[u8]) -> Option<StatCache> {
        let cache: StatCache = serde_json::from_slice(bytes).ok()?;
        (cache.version == VERSION && cache.files.values().all(|f| is_sha256(&f.sha256)))
            .then_some(cache)
    }

    pub fn to_json(&self) -> Vec<u8> {
        // Maps with string keys and plain values: serializing can't fail.
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(c: char) -> String {
        c.to_string().repeat(64)
    }

    #[test]
    fn hits_need_the_same_size_and_time_before_the_racy_window() {
        let started = 100 * RACY_NS;
        let mut cache = StatCache::new(started);
        cache.insert("a".into(), 5, started - RACY_NS - 1, hash('a'));
        cache.insert("racy".into(), 5, started - RACY_NS, hash('b'));
        cache.insert("after".into(), 5, started + 1, hash('c'));
        assert_eq!(
            cache.lookup("a", 5, started - RACY_NS - 1),
            Some(hash('a').as_str())
        );
        assert_eq!(cache.lookup("a", 6, started - RACY_NS - 1), None);
        assert_eq!(cache.lookup("a", 5, started - RACY_NS - 2), None);
        assert_eq!(cache.lookup("racy", 5, started - RACY_NS), None);
        assert_eq!(cache.lookup("after", 5, started + 1), None);
        assert_eq!(cache.lookup("missing", 5, 0), None);
    }

    #[test]
    fn round_trip_and_odd_caches_are_none() {
        let mut cache = StatCache::new(7);
        cache.insert("minecraft/mods/a.jar".into(), 1, 2, hash('d'));
        assert_eq!(StatCache::from_json(&cache.to_json()), Some(cache.clone()));
        let mut newer = cache.clone();
        newer.version = 2;
        assert_eq!(StatCache::from_json(&newer.to_json()), None);
        let mut bad = cache;
        bad.insert("x".into(), 1, 2, "nope".into());
        assert_eq!(StatCache::from_json(&bad.to_json()), None);
        assert_eq!(StatCache::from_json(b"not json"), None);
        // An empty cache trusts nothing.
        assert_eq!(StatCache::default().lookup("x", 0, i64::MIN), None);
    }
}
