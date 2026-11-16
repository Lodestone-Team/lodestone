//! Keeps a failure that repeats (an event shape that changed, a core polled while it
//! refuses, a download retried) from filling the log and rotating the useful lines
//! away: a few lines per key a minute. How many were left out is said by the next line
//! with that key once the minute is over; when the failure stops, nothing says it (the
//! last line kept is at most a minute older than the last one left out).

use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Lines per key in each window.
const PER_WINDOW: u32 = 10;
/// The window.
const WINDOW: Duration = Duration::from_secs(60);
/// Keys kept before the oldest windows are forgotten, so memory stays bounded.
const MAX_KEYS: usize = 256;

/// Whether a line may be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    Yes,
    /// Yes, and this many lines with its key were left out in the last window.
    YesAfterSuppressing(u32),
    No,
}

struct Window {
    started: Instant,
    count: u32,
    suppressed: u32,
}

/// Counts lines by key. Usually a `static`.
pub struct Throttle {
    windows: Mutex<BTreeMap<String, Window>>,
}

impl Default for Throttle {
    fn default() -> Self {
        Self::new()
    }
}

impl Throttle {
    pub const fn new() -> Self {
        Self {
            windows: Mutex::new(BTreeMap::new()),
        }
    }

    /// Whether a line with `key` may be written now.
    pub fn allow(&self, key: &str) -> Allowed {
        self.allow_at(key, Instant::now())
    }

    fn allow_at(&self, key: &str, now: Instant) -> Allowed {
        let mut windows = self.windows.lock().unwrap_or_else(|p| p.into_inner());
        if !windows.contains_key(key) && windows.len() >= MAX_KEYS {
            windows.retain(|_, w| now.duration_since(w.started) < WINDOW);
            if windows.len() >= MAX_KEYS {
                windows.clear();
            }
        }
        let window = windows.entry(key.to_string()).or_insert(Window {
            started: now,
            count: 0,
            suppressed: 0,
        });
        if now.duration_since(window.started) >= WINDOW {
            let suppressed = std::mem::take(&mut window.suppressed);
            window.started = now;
            window.count = 1;
            return match suppressed {
                0 => Allowed::Yes,
                n => Allowed::YesAfterSuppressing(n),
            };
        }
        if window.count < PER_WINDOW {
            window.count += 1;
            Allowed::Yes
        } else {
            window.suppressed += 1;
            Allowed::No
        }
    }

    /// Logs `message` at `level` from `target` unless `key` has had its lines this
    /// minute; the first line after a quiet spell says how many were left out.
    pub fn log(&self, key: &str, level: log::Level, target: &str, message: std::fmt::Arguments) {
        if !log::log_enabled!(target: target, level) {
            return;
        }
        match self.allow(key) {
            Allowed::Yes => log::log!(target: target, level, "{message}"),
            Allowed::YesAfterSuppressing(n) => {
                log::log!(target: target, level, "{message} (and {n} more like it before)")
            }
            Allowed::No => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeating_line_is_summarised() {
        let throttle = Throttle::new();
        let start = Instant::now();
        for _ in 0..PER_WINDOW {
            assert_eq!(throttle.allow_at("events", start), Allowed::Yes);
        }
        for _ in 0..5 {
            assert_eq!(throttle.allow_at("events", start), Allowed::No);
        }
        // other keys have their own count
        assert_eq!(throttle.allow_at("downloads", start), Allowed::Yes);
        let later = start + WINDOW;
        assert_eq!(
            throttle.allow_at("events", later),
            Allowed::YesAfterSuppressing(5)
        );
        assert_eq!(throttle.allow_at("events", later), Allowed::Yes);
    }

    #[test]
    fn keys_are_bounded() {
        let throttle = Throttle::new();
        let now = Instant::now();
        for n in 0..(MAX_KEYS * 3) {
            throttle.allow_at(&format!("key {n}"), now);
        }
        assert!(throttle.windows.lock().unwrap().len() <= MAX_KEYS);
    }
}
