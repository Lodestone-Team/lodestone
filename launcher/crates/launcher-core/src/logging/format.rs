//! One record as the log file's lines:
//! `2026-11-16T08:30:12.345Z WARN  launcher_core::cores::live: the stream ended`.
//!
//! The time is UTC (RFC 3339, milliseconds). A message's later lines are indented two
//! spaces, so `grep` and `tail` stay line-based and a record's start is easy to find.
//! Every message goes through [`lodestone_trace::scrub_line`] first.

use chrono::{DateTime, SecondsFormat, Utc};
use log::Level;

/// The longest message kept for an ordinary record; the rest is cut and counted.
pub const MAX_MESSAGE: usize = 16 * 1024;

/// The record's lines, ending in a newline. `limit` cuts a longer message (`None`
/// keeps all of it: a panic's backtrace).
pub fn record(
    at: DateTime<Utc>,
    level: Level,
    target: &str,
    message: &str,
    limit: Option<usize>,
) -> String {
    let message = lodestone_trace::scrub_line(message);
    let mut message: &str = &message;
    let mut cut = 0;
    if let Some(limit) = limit
        && message.len() > limit
    {
        let mut end = limit;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        cut = message.len() - end;
        message = &message[..end];
    }
    let mut out = format!(
        "{} {:<5} {target}: ",
        at.to_rfc3339_opts(SecondsFormat::Millis, true),
        level.as_str()
    );
    let mut lines = message.trim_end().split('\n');
    out.push_str(lines.next().unwrap_or_default().trim_end_matches('\r'));
    for line in lines {
        out.push_str("\n  ");
        out.push_str(line.trim_end_matches('\r'));
    }
    if cut > 0 {
        out.push_str(&format!(" …[cut {cut} bytes]"));
    }
    out.push('\n');
    out
}

/// The level of a line [`record`] wrote, or `None` for a continuation line or
/// anything else.
pub fn level_of(line: &str) -> Option<Level> {
    if line.starts_with(' ') {
        return None;
    }
    let mut words = line.split_whitespace();
    words.next()?;
    words.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 16, 8, 30, 12).unwrap() + chrono::Duration::milliseconds(345)
    }

    #[test]
    fn records_have_time_level_target_and_message() {
        let line = record(
            at(),
            Level::Warn,
            "launcher_core::cores::live",
            "the stream ended",
            Some(MAX_MESSAGE),
        );
        assert_eq!(
            line,
            "2026-11-16T08:30:12.345Z WARN  launcher_core::cores::live: the stream ended\n"
        );
        assert_eq!(level_of(&line), Some(Level::Warn));
        let line = record(at(), Level::Error, "x", "first\r\nsecond\n", None);
        assert_eq!(line, "2026-11-16T08:30:12.345Z ERROR x: first\n  second\n");
        assert_eq!(level_of("  second"), None);
    }

    #[test]
    fn long_messages_are_cut() {
        let message = "é".repeat(MAX_MESSAGE);
        let line = record(at(), Level::Info, "x", &message, Some(MAX_MESSAGE));
        assert!(line.len() < MAX_MESSAGE + 100);
        assert!(line.ends_with(&format!(" …[cut {} bytes]\n", MAX_MESSAGE)));
    }

    #[test]
    fn secrets_never_reach_a_line() {
        let line = record(
            at(),
            Level::Warn,
            "lodestone_launcher::accounts",
            "refresh failed: {\"access_token\":\"secret-one\"} password=secret-two\nAuthorization: Bearer secret-three",
            Some(MAX_MESSAGE),
        );
        for secret in ["secret-one", "secret-two", "secret-three"] {
            assert!(!line.contains(secret), "{line}");
        }
    }
}
