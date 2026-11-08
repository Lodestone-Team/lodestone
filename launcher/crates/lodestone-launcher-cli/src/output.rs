//! Printing: JSON for programs, text for people.

use std::io::Write as _;

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

/// One line of a stream (log rows, events), on stdout.
pub fn line(mode: Mode, value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = match mode {
        Mode::Json => writeln!(out, "{value}"),
        Mode::Human => writeln!(out, "{}", human_line(value)),
    };
    let _ = out.flush();
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

#[cfg(test)]
mod tests {
    use super::*;

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
