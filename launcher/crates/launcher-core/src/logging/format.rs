//! One record as the log file's lines:
//! `2026-11-16T08:30:12.345Z WARN  launcher_core::cores::live: the stream ended`.
//!
//! The time is UTC (RFC 3339, milliseconds). A message's later lines are indented two
//! spaces, so `grep` and `tail` stay line-based and a record's start is easy to find.
//! Messages come already scrubbed of secrets (the logger and the panic hook run
//! [`lodestone_trace::scrub_line`] once, before the text goes anywhere), and control
//! characters come out escaped ([`escape_controls`]): a core's error message can
//! carry terminal escapes, which `logs tail` would otherwise run.

use chrono::{DateTime, SecondsFormat, Utc};
use log::Level;

/// The longest message kept for an ordinary record; the rest is cut and counted.
pub const MAX_MESSAGE: usize = 16 * 1024;

/// The record's lines, ending in a newline. `message` is already scrubbed. `limit`
/// cuts a longer message (`None` keeps all of it: a panic's report); `cut_before`
/// counts what the caller cut before scrubbing.
pub fn record(
    at: DateTime<Utc>,
    level: Level,
    target: &str,
    message: &str,
    limit: Option<usize>,
    cut_before: usize,
) -> String {
    let mut message = message;
    let mut cut = cut_before;
    if let Some(limit) = limit
        && message.len() > limit
    {
        let kept = truncate(message, limit);
        cut += message.len() - kept.len();
        message = kept;
    }
    let mut out = format!(
        "{} {:<5} {target}: ",
        at.to_rfc3339_opts(SecondsFormat::Millis, true),
        level.as_str()
    );
    let mut lines = message.trim_end().split('\n');
    out.push_str(&escape_controls(
        lines.next().unwrap_or_default().trim_end_matches('\r'),
    ));
    for line in lines {
        out.push_str("\n  ");
        out.push_str(&escape_controls(line.trim_end_matches('\r')));
    }
    if cut > 0 {
        out.push_str(&format!(" …[cut {cut} bytes]"));
    }
    out.push('\n');
    out
}

/// `text` cut to at most `max` bytes, at a character's boundary.
pub fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// One line with its control characters (but tabs) and Unicode's direction controls
/// written as escapes (`\u{1b}`), so printing it can't move the cursor, change the
/// terminal or the clipboard, or make it read as another line.
pub fn escape_controls(line: &str) -> std::borrow::Cow<'_, str> {
    let hidden = |c: char| {
        (c.is_control() && c != '\t')
            || matches!(c, '\u{200e}' | '\u{200f}' | '\u{061c}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    };
    if !line.contains(hidden) {
        return std::borrow::Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len() + 16);
    for c in line.chars() {
        if hidden(c) {
            out.push_str(&format!("\\u{{{:x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    std::borrow::Cow::Owned(out)
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
            0,
        );
        assert_eq!(
            line,
            "2026-11-16T08:30:12.345Z WARN  launcher_core::cores::live: the stream ended\n"
        );
        assert_eq!(
            launcher_control::logs::severity_of(&line),
            Some(Level::Warn as usize)
        );
        let line = record(at(), Level::Error, "x", "first\r\nsecond\n", None, 0);
        assert_eq!(line, "2026-11-16T08:30:12.345Z ERROR x: first\n  second\n");
        assert_eq!(launcher_control::logs::severity_of("  second"), None);
    }

    #[test]
    fn control_characters_are_escaped() {
        let message =
            "\u{1b}]52;c;ZWNobyBoaQ==\u{7}\u{1b}[2K\rINFO  launcher_core: all good\u{202e}\tend";
        let line = record(at(), Level::Warn, "x", message, Some(MAX_MESSAGE), 0);
        assert_eq!(
            line,
            "2026-11-16T08:30:12.345Z WARN  x: \\u{1b}]52;c;ZWNobyBoaQ==\\u{7}\\u{1b}[2K\\u{d}INFO  launcher_core: all good\\u{202e}\tend\n"
        );
        assert_eq!(line.matches('\n').count(), 1);
        assert!(matches!(
            escape_controls("plain\ttext"),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn long_messages_are_cut() {
        let message = "é".repeat(MAX_MESSAGE);
        let line = record(at(), Level::Info, "x", &message, Some(MAX_MESSAGE), 0);
        assert!(line.len() < MAX_MESSAGE + 100);
        assert!(line.ends_with(&format!(" …[cut {} bytes]\n", MAX_MESSAGE)));
        let line = record(at(), Level::Info, "x", "short", Some(MAX_MESSAGE), 7);
        assert!(line.ends_with("short …[cut 7 bytes]\n"), "{line}");
        assert_eq!(truncate("éé", 3), "é");
    }
}
