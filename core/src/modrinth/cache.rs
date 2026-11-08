//! A small in-memory cache with a time to live per entry.

use std::time::{Duration, Instant};

use axum::body::Bytes;
use indexmap::IndexMap;

struct Entry {
    expires: Instant,
    value: Bytes,
}

/// Maps keys to response bodies until they expire. It is bounded by an entry count and a
/// total size; when either is exceeded, the oldest entries are evicted first.
pub struct TtlCache {
    entries: IndexMap<String, Entry>,
    max_entries: usize,
    max_bytes: usize,
    total_bytes: usize,
}

impl TtlCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: IndexMap::new(),
            max_entries,
            max_bytes,
            total_bytes: 0,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<Bytes> {
        self.get_at(key, Instant::now())
    }

    fn get_at(&mut self, key: &str, now: Instant) -> Option<Bytes> {
        let expired = self.entries.get(key)?.expires <= now;
        if expired {
            self.remove(key);
            return None;
        }
        self.entries.get(key).map(|e| e.value.clone())
    }

    pub fn insert(&mut self, key: String, value: Bytes, ttl: Duration) {
        self.insert_at(key, value, ttl, Instant::now());
    }

    fn insert_at(&mut self, key: String, value: Bytes, ttl: Duration, now: Instant) {
        if value.len() > self.max_bytes {
            return;
        }
        // a new value moves the key to the end, as the newest entry
        self.remove(&key);
        self.total_bytes += value.len();
        self.entries.insert(
            key,
            Entry {
                expires: now + ttl,
                value,
            },
        );
        while self.entries.len() > self.max_entries || self.total_bytes > self.max_bytes {
            match self.entries.shift_remove_index(0) {
                Some((_, entry)) => self.total_bytes -= entry.value.len(),
                None => break,
            }
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.entries.shift_remove(key) {
            self.total_bytes -= entry.value.len();
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expires_entries() {
        let mut cache = TtlCache::new(10, 1000);
        let now = Instant::now();
        cache.insert_at("a".into(), Bytes::from("1"), Duration::from_secs(5), now);
        assert_eq!(
            cache.get_at("a", now + Duration::from_secs(4)),
            Some(Bytes::from("1"))
        );
        assert_eq!(cache.get_at("a", now + Duration::from_secs(5)), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn evicts_oldest_first() {
        let mut cache = TtlCache::new(2, 1000);
        let ttl = Duration::from_secs(60);
        cache.insert("a".into(), Bytes::from("1"), ttl);
        cache.insert("b".into(), Bytes::from("2"), ttl);
        cache.insert("c".into(), Bytes::from("3"), ttl);
        assert_eq!(cache.get("a"), None);
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
        // replacing a key makes it the newest
        cache.insert("b".into(), Bytes::from("22"), ttl);
        cache.insert("d".into(), Bytes::from("4"), ttl);
        assert_eq!(cache.get("c"), None);
        assert_eq!(cache.get("b"), Some(Bytes::from("22")));
    }

    #[test]
    fn bounded_by_size() {
        let mut cache = TtlCache::new(100, 10);
        let ttl = Duration::from_secs(60);
        cache.insert("a".into(), Bytes::from("123456"), ttl);
        cache.insert("b".into(), Bytes::from("123456"), ttl);
        assert_eq!(cache.get("a"), None);
        assert!(cache.get("b").is_some());
        // a value larger than the whole cache is not stored
        cache.insert("c".into(), Bytes::from("12345678901"), ttl);
        assert_eq!(cache.get("c"), None);
        assert!(cache.get("b").is_some());
    }
}
