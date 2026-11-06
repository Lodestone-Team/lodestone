//! An in-memory limit on failed logins per username.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use color_eyre::eyre::eyre;

use crate::error::{Error, ErrorKind};

/// Failed logins allowed for one username within [`WINDOW`].
pub const MAX_FAILURES: u32 = 10;
/// How long failed logins are counted for, from the first one.
pub const WINDOW: Duration = Duration::from_secs(5 * 60);
/// The most usernames tracked at once. Usernames are chosen by the client, so this keeps
/// the table bounded even when someone tries many different names.
const MAX_ENTRIES: usize = 10_000;

struct FailureWindow {
    started: Instant,
    failures: u32,
}

impl FailureWindow {
    fn expired(&self, now: Instant, window: Duration) -> bool {
        now.saturating_duration_since(self.started) >= window
    }
}

/// A login refused because its username has too many recent failures.
///
/// Its response is a 429 with the usual JSON error body and a `Retry-After` header.
#[derive(Debug)]
pub struct LoginLimited {
    /// Whole seconds until the username may try again (at least 1).
    pub retry_after_secs: u64,
}

impl LoginLimited {
    fn new(wait: Duration) -> Self {
        // rounded up, so a client that waits this long is not refused again
        let secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
        Self {
            retry_after_secs: secs.max(1),
        }
    }

    pub fn to_error(&self) -> Error {
        Error {
            kind: ErrorKind::TooManyRequests,
            source: eyre!(
                "Too many failed login attempts. Try again in {} seconds",
                self.retry_after_secs
            ),
        }
    }
}

impl IntoResponse for LoginLimited {
    fn into_response(self) -> Response {
        let mut response = self.to_error().into_response();
        response.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from(self.retry_after_secs),
        );
        response
    }
}

/// Counts failed logins per username and refuses further attempts once a username has
/// [`MAX_FAILURES`] failures within [`WINDOW`].
///
/// Every attempt counts as a failure until it is reported as a success, so parallel
/// requests cannot get past the limit while their passwords are still being checked.
/// Usernames are stored as keyed 64-bit hashes, so long names do not use extra memory.
pub struct LoginRateLimiter {
    max_failures: u32,
    window: Duration,
    max_entries: usize,
    hasher: RandomState,
    entries: Mutex<HashMap<u64, FailureWindow>>,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::with_limits(MAX_FAILURES, WINDOW, MAX_ENTRIES)
    }
}

impl LoginRateLimiter {
    fn with_limits(max_failures: u32, window: Duration, max_entries: usize) -> Self {
        Self {
            max_failures,
            window,
            max_entries: max_entries.max(1),
            hasher: RandomState::new(),
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, HashMap<u64, FailureWindow>> {
        // the table only holds counters, so it is still usable after a panic elsewhere
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Call before checking a password. Counts the attempt as a failure, or refuses it if
    /// `username` has used up its failed attempts.
    pub fn begin_attempt(&self, username: &str) -> Result<(), LoginLimited> {
        self.begin_attempt_at(username, Instant::now())
    }

    /// Call after a successful login: clears the failures counted for `username`.
    pub fn record_success(&self, username: &str) {
        let key = self.hasher.hash_one(username);
        self.entries().remove(&key);
    }

    fn begin_attempt_at(&self, username: &str, now: Instant) -> Result<(), LoginLimited> {
        let key = self.hasher.hash_one(username);
        let mut entries = self.entries();
        if let Some(entry) = entries.get_mut(&key) {
            if !entry.expired(now, self.window) {
                if entry.failures >= self.max_failures {
                    return Err(LoginLimited::new(
                        self.window
                            .saturating_sub(now.saturating_duration_since(entry.started)),
                    ));
                }
                entry.failures += 1;
                return Ok(());
            }
            entries.remove(&key);
        }
        if entries.len() >= self.max_entries {
            self.make_room(&mut entries, now);
        }
        entries.insert(
            key,
            FailureWindow {
                started: now,
                failures: 1,
            },
        );
        Ok(())
    }

    /// Drops expired entries. If the table is still full, drops the oldest entry, which is
    /// the one closest to expiring anyway.
    fn make_room(&self, entries: &mut HashMap<u64, FailureWindow>, now: Instant) {
        entries.retain(|_, entry| !entry.expired(now, self.window));
        if entries.len() >= self.max_entries {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.started)
                .map(|(key, _)| *key)
            {
                entries.remove(&oldest);
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_limited(result: Result<(), LoginLimited>) -> bool {
        result.is_err()
    }

    #[test]
    fn allows_ten_failures_then_refuses_until_the_window_passes() {
        let limiter = LoginRateLimiter::default();
        let start = Instant::now();
        for _ in 0..MAX_FAILURES {
            assert!(limiter.begin_attempt_at("alice", start).is_ok());
        }
        assert!(is_limited(limiter.begin_attempt_at("alice", start)));
        assert!(is_limited(limiter.begin_attempt_at(
            "alice",
            start + WINDOW - Duration::from_secs(1)
        )));
        // other usernames are not affected
        assert!(limiter.begin_attempt_at("bob", start).is_ok());
        // once the window has passed, attempts are allowed again
        assert!(limiter.begin_attempt_at("alice", start + WINDOW).is_ok());
    }

    #[test]
    fn success_resets_the_count() {
        let limiter = LoginRateLimiter::default();
        let now = Instant::now();
        for _ in 0..MAX_FAILURES {
            limiter.begin_attempt_at("alice", now).unwrap();
        }
        limiter.record_success("alice");
        for _ in 0..MAX_FAILURES {
            assert!(limiter.begin_attempt_at("alice", now).is_ok());
        }
        assert!(is_limited(limiter.begin_attempt_at("alice", now)));
        assert_eq!(limiter.len(), 1);
    }

    #[test]
    fn error_says_how_long_to_wait() {
        let limiter = LoginRateLimiter::with_limits(1, Duration::from_secs(300), 10);
        let now = Instant::now();
        limiter.begin_attempt_at("alice", now).unwrap();
        let limited = limiter
            .begin_attempt_at("alice", now + Duration::from_secs(100))
            .unwrap_err();
        assert_eq!(limited.retry_after_secs, 200);
        let error = limited.to_error();
        assert!(matches!(error.kind, ErrorKind::TooManyRequests));
        assert!(
            error.source.to_string().contains("200 seconds"),
            "{}",
            error.source
        );
        // part of a second left is rounded up
        let limited = limiter
            .begin_attempt_at("alice", now + Duration::from_millis(299_500))
            .unwrap_err();
        assert_eq!(limited.retry_after_secs, 1);
    }

    #[test]
    fn response_is_429_with_retry_after() {
        let response = LoginLimited {
            retry_after_secs: 42,
        }
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "42");
    }

    #[test]
    fn table_stays_bounded() {
        let limiter = LoginRateLimiter::with_limits(3, Duration::from_secs(60), 100);
        let start = Instant::now();
        for i in 0..1000 {
            limiter
                .begin_attempt_at(&format!("user{i}"), start + Duration::from_millis(i))
                .unwrap();
        }
        assert_eq!(limiter.len(), 100);
        // expired entries are dropped first when room is needed
        limiter
            .begin_attempt_at("late", start + Duration::from_secs(120))
            .unwrap();
        assert_eq!(limiter.len(), 1);
    }

    #[test]
    fn a_full_table_keeps_the_newest_entries() {
        let limiter = LoginRateLimiter::with_limits(1, Duration::from_secs(60), 2);
        let start = Instant::now();
        limiter.begin_attempt_at("a", start).unwrap();
        limiter
            .begin_attempt_at("b", start + Duration::from_secs(1))
            .unwrap();
        limiter
            .begin_attempt_at("c", start + Duration::from_secs(2))
            .unwrap();
        assert_eq!(limiter.len(), 2);
        let now = start + Duration::from_secs(3);
        // "a" was the oldest, so it was dropped; "b" and "c" are still limited
        assert!(is_limited(limiter.begin_attempt_at("b", now)));
        assert!(is_limited(limiter.begin_attempt_at("c", now)));
    }
}
