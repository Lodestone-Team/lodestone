//! The launcher's global hooks for the debug traces (see [`launcher_core::trace`]):
//! log records, and the shortcuts typed and the actions they ran.

use gpui::{App, Keystroke};
use launcher_core::trace;
use serde_json::json;

/// Sends `log` records (the launcher's and GPUI's) to the traces as `log.{level}`.
/// Nothing is printed or written: without a subscriber, warnings and errors only go
/// to the in-memory backlog, and debug and trace records are dropped.
struct TraceLogger;

/// Whether a record is forwarded: info and above from anywhere; debug and trace only
/// from the launcher's own crates and GPUI, since the network libraries' detailed
/// records (TLS, HTTP framing) are noise and could describe requests in detail.
fn forwarded(metadata: &log::Metadata) -> bool {
    metadata.level() <= log::Level::Info
        || ["lodestone", "launcher", "gpui"]
            .iter()
            .any(|own| metadata.target().starts_with(own))
}

impl log::Log for TraceLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        forwarded(metadata) && trace::bus().wants(level_topic(metadata.level()))
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let topic = level_topic(record.level());
        trace::emit(topic, || {
            json!({
                "target": record.target(),
                "message": record.args().to_string(),
                "file": record.file(),
                "line": record.line(),
            })
        });
    }

    fn flush(&self) {}
}

fn level_topic(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "log.error",
        log::Level::Warn => "log.warn",
        log::Level::Info => "log.info",
        log::Level::Debug => "log.debug",
        log::Level::Trace => "log.trace",
    }
}

/// "cmd-shift-z": the modifiers, then the key.
fn keys(keystroke: &Keystroke) -> String {
    let m = &keystroke.modifiers;
    let mut out = String::new();
    for (on, name) in [
        (m.control, "ctrl-"),
        (m.alt, "alt-"),
        (m.shift, "shift-"),
        (m.platform, "cmd-"),
        (m.function, "fn-"),
    ] {
        if on {
            out.push_str(name);
        }
    }
    out.push_str(&keystroke.key);
    out
}

/// Installs the logger and the keystroke observer. Call once, at start.
pub fn install(cx: &mut App) {
    // Only the control socket reads the traces, and it is Unix only for now (see
    // `crate::control`). Elsewhere, recording them would format every log record,
    // keystroke and frame's state for no reader, so the bus stays off.
    #[cfg(not(unix))]
    trace::bus().set_enabled(false);
    if log::set_logger(&TraceLogger).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
    cx.observe_keystrokes(|event, _, _| {
        let keystroke = &event.keystroke;
        let modified = keystroke.modifiers.control
            || keystroke.modifiers.platform
            || keystroke.modifiers.alt
            || keystroke.modifiers.function;
        match &event.action {
            // A key that ran an action: the action's name and the keys, never text.
            Some(action) => {
                let name = action.name();
                trace::emit(
                    "ui.action",
                    || json!({ "action": name, "keys": keys(keystroke) }),
                );
            }
            // A shortcut nothing bound: live only. Plain typing isn't traced.
            None if modified => {
                trace::emit("ui.key", || json!({ "keys": keys(keystroke) }));
            }
            None => {}
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(level: log::Level, target: &str) -> bool {
        forwarded(&log::Metadata::builder().level(level).target(target).build())
    }

    #[test]
    fn only_own_debug_records_are_forwarded() {
        assert!(meta(log::Level::Warn, "rustls::client"));
        assert!(meta(log::Level::Info, "hyper"));
        assert!(!meta(log::Level::Debug, "rustls::client::hs"));
        assert!(!meta(log::Level::Trace, "zed_reqwest::connect"));
        assert!(meta(log::Level::Debug, "lodestone_launcher::shell"));
        assert!(meta(log::Level::Trace, "gpui::window"));
    }
}
