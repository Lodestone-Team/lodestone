//! Printing: JSON for programs, text for people.

use std::io::{self, Write};

use launcher_control::ErrorObject;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Json,
    Human,
}

/// A command's result, on stdout.
pub fn result(mode: Mode, value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = match mode {
        Mode::Json => writeln!(out, "{value}"),
        Mode::Human => {
            let mut text = String::new();
            human(value, 0, &mut text);
            write!(out, "{text}")
        }
    };
}

/// `logs tail` for people: the log's lines as they are in the file, with control
/// characters written visibly in case a file holds some (the Launcher escapes them
/// when it writes; an older or edited file may not).
pub fn lines(value: &Value) {
    let mut out = std::io::stdout().lock();
    for line in value["lines"].as_array().into_iter().flatten() {
        let _ = writeln!(out, "{}", visible(line.as_str().unwrap_or_default()));
    }
}

/// `line` with control characters (but tabs) and Unicode's direction controls as
/// escapes (`\u{1b}`), so printing it can't drive the terminal.
fn visible(line: &str) -> String {
    line.chars()
        .map(|c| {
            let hidden = (c.is_control() && c != '\t')
                || matches!(c, '\u{200e}' | '\u{200f}' | '\u{061c}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            if hidden {
                format!("\\u{{{:x}}}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// `content.history` for people: one line per image, newest first.
pub fn history(value: &Value) {
    let _ = write!(std::io::stdout().lock(), "{}", history_text(value));
}

/// See [`history`]: `2026-11-14 10:02  Added JEI  (42 files, fabric 0.16.10 on 1.21.1)
/// current`. Labels come from images other computers may have made: the Launcher sends
/// them with control and invisible characters written visibly (`label_shown`,
/// `game_shown`).
pub fn history_text(value: &Value) -> String {
    let images = value["images"].as_array().cloned().unwrap_or_default();
    if images.is_empty() {
        return "No content images yet.\n".into();
    }
    let mut text = String::new();
    for image in &images {
        let s = |key: &str| image[key].as_str().unwrap_or_default();
        let label = s("label_shown").to_string();
        let label = if label.is_empty() {
            "(no label)".into()
        } else {
            label
        };
        let files = image["files"].as_u64().unwrap_or(0);
        text.push_str(&format!(
            "{}  {label}  ({files} {}, {})",
            utc_minute(image["created"].as_i64().unwrap_or(0)),
            if files == 1 { "file" } else { "files" },
            s("game_shown"),
        ));
        if image["current"].as_bool() == Some(true) {
            text.push_str("  current");
        }
        text.push('\n');
    }
    text
}

/// Unix seconds as `YYYY-MM-DD HH:MM` in UTC.
fn utc_minute(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let minute = seconds.rem_euclid(86_400) / 60;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        minute / 60,
        minute % 60
    )
}

/// One line of a stream (log rows, events), on stdout. Fails once stdout is gone,
/// such as when `head` read what it wanted and exited: Rust ignores SIGPIPE, so the
/// stream has to stop on the error or it would never end.
pub fn line(mode: Mode, value: &Value) -> io::Result<()> {
    match mode {
        Mode::Json => text_line(&value.to_string()),
        Mode::Human => text_line(&human_line(value)),
    }
}

/// A line of text of a stream, on stdout; fails like [`line`].
pub fn text_line(text: &str) -> io::Result<()> {
    write_line(&mut std::io::stdout().lock(), text)
}

fn write_line(out: &mut impl Write, text: &str) -> io::Result<()> {
    writeln!(out, "{text}")?;
    out.flush()
}

/// An error, on stderr.
pub fn error(mode: Mode, error: &ErrorObject) {
    let mut err = std::io::stderr().lock();
    let _ = match mode {
        Mode::Json => writeln!(err, "{}", json!({ "error": error })),
        Mode::Human => {
            let mut text = format!("error: {}\n", error.message);
            if let Some(data) = &error.data
                && !data.is_null()
            {
                human(data, 1, &mut text);
            }
            write!(err, "{text}")
        }
    };
}

/// Progress of a long command, on stderr.
pub fn progress(mode: Mode, value: &Value) {
    let mut err = std::io::stderr().lock();
    let _ = match mode {
        Mode::Json => writeln!(err, "{}", json!({ "progress": value })),
        Mode::Human => writeln!(err, "{}", progress_text(value)),
    };
}

/// "Downloading 12/40 files, 3.1/20.0 MB (15%)", or the message of a sign-in code.
pub fn progress_text(value: &Value) -> String {
    if let Some(message) = value.get("message").and_then(Value::as_str)
        && value.get("user_code").is_some()
    {
        return message.to_string();
    }
    let progress = value.get("progress").unwrap_or(value);
    let label = value
        .get("stage")
        .and_then(Value::as_str)
        .or_else(|| progress.get("label").and_then(Value::as_str))
        .or_else(|| value.get("label").and_then(Value::as_str))
        .unwrap_or("Working");
    let mut text = label.to_string();
    let n = |key: &str| progress.get(key).and_then(Value::as_u64).unwrap_or(0);
    if n("files_total") > 0 {
        text.push_str(&format!(" {}/{} files", n("files_done"), n("files_total")));
    }
    if n("bytes_total") > 0 {
        let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
        text.push_str(&format!(
            ", {:.1}/{:.1} MB",
            mb(n("bytes_done")),
            mb(n("bytes_total"))
        ));
    }
    if let Some(fraction) = progress.get("fraction").and_then(Value::as_f64) {
        text.push_str(&format!(" ({:.0}%)", fraction * 100.0));
    }
    text
}

/// A log row or event as one line of text.
pub fn human_line(value: &Value) -> String {
    let s = |key: &str| value.get(key).and_then(Value::as_str);
    if let Some(text) = s("text") {
        let mut line = String::new();
        if let Some(time) = s("time") {
            line.push_str(&format!("[{time}] "));
        }
        if let Some(thread) = s("thread") {
            line.push_str(&format!("[{thread}/{}] ", s("level").unwrap_or("INFO")));
        }
        line.push_str(text);
        return line;
    }
    if let Some(event) = s("event") {
        let data = value.get("data").cloned().unwrap_or(Value::Null);
        if event == "log" {
            return human_line(&data);
        }
        return format!("{event}: {}", compact(&data));
    }
    compact(value)
}

fn compact(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| format!("{k}={}", compact(v)))
            .collect::<Vec<_>>()
            .join(" "),
        Value::Array(items) => items.iter().map(compact).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}

/// An indented, YAML-like rendering, without nulls and empty lists.
pub fn human(value: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::Null => {}
                    Value::Array(items) if items.is_empty() => {}
                    Value::Object(inner) if inner.is_empty() => {}
                    Value::Object(_) => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        human(value, depth + 1, out);
                    }
                    Value::Array(items)
                        if items.iter().all(|i| !i.is_object() && !i.is_array()) =>
                    {
                        out.push_str(&format!("{pad}{key}: {}\n", compact(value)));
                        let _ = items;
                    }
                    Value::Array(items) => {
                        out.push_str(&format!("{pad}{key}:\n"));
                        for item in items {
                            item_block(item, depth + 1, out);
                        }
                    }
                    scalar => out.push_str(&format!("{pad}{key}: {}\n", compact(scalar))),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                item_block(item, depth, out);
            }
        }
        Value::Null => {}
        scalar => out.push_str(&format!("{pad}{}\n", compact(scalar))),
    }
}

fn item_block(item: &Value, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match item {
        Value::Object(_) => {
            let mut inner = String::new();
            human(item, depth + 1, &mut inner);
            let trimmed = inner.trim_start();
            out.push_str(&format!("{pad}- {trimmed}"));
        }
        other => out.push_str(&format!("{pad}- {}\n", compact(other))),
    }
}

/// A trace event for people: `14:02:07.120  ui.click  {"control":"button",…}`, the time
/// in UTC.
pub fn trace_line(event: &Value) -> String {
    let ms = event["at_ms"].as_u64().unwrap_or(0);
    let day = ms % 86_400_000;
    let time = format!(
        "{:02}:{:02}:{:02}.{:03}",
        day / 3_600_000,
        day / 60_000 % 60,
        day / 1000 % 60,
        day % 1000
    );
    let topic = event["topic"].as_str().unwrap_or("?");
    format!("{time}  {topic:<18} {}", event["data"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_print_their_controls_visibly() {
        assert_eq!(
            visible("a\u{1b}]52;c;x\u{7}\rb\u{202e}\tc"),
            "a\\u{1b}]52;c;x\\u{7}\\u{d}b\\u{202e}\tc"
        );
    }

    /// A pipe whose reader went away.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn stream_lines_say_when_stdout_is_gone() {
        let mut out = Vec::new();
        write_line(&mut out, "one").unwrap();
        assert_eq!(out, b"one\n");
        let err = write_line(&mut Closed, "two").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn trace_lines_read_well() {
        let event = json!({ "seq": 4, "at_ms": 1_700_000_527_120u64, "topic": "ui.click", "data": { "id": "save" } });
        assert_eq!(
            trace_line(&event),
            "22:22:07.120  ui.click           {\"id\":\"save\"}"
        );
    }

    #[test]
    fn people_get_readable_text() {
        let mut text = String::new();
        human(
            &json!({
                "instances": [{ "name": "Survival", "state": "idle", "group": null }],
                "auto_groups": true,
                "tags": ["a", "b"],
                "none": [],
            }),
            0,
            &mut text,
        );
        assert_eq!(
            text,
            "instances:\n  - name: Survival\n    state: idle\nauto_groups: true\ntags: a, b\n"
        );
    }

    #[test]
    fn log_rows_read_like_the_console() {
        let row =
            json!({ "time": "12:00:01", "thread": "Render thread", "level": "INFO", "text": "Hi" });
        assert_eq!(human_line(&row), "[12:00:01] [Render thread/INFO] Hi");
        let event = json!({ "event": "state", "data": { "instance": "a", "state": "running" } });
        assert_eq!(human_line(&event), "state: instance=a state=running");
    }

    #[test]
    fn progress_reads_as_a_line() {
        let value = json!({
            "label": "Installing",
            "progress": { "label": "Downloading", "files_done": 2, "files_total": 4, "bytes_total": 0, "fraction": 0.5 },
        });
        assert_eq!(progress_text(&value), "Downloading 2/4 files (50%)");
        let code = json!({ "user_code": "ABCD", "message": "Open https://microsoft.com/link and enter ABCD." });
        assert_eq!(
            progress_text(&code),
            "Open https://microsoft.com/link and enter ABCD."
        );
    }
}
