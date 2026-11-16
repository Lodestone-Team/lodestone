//! The panic hook: a panic's message, place, thread and backtrace go to the log file,
//! flushed before the process carries on unwinding or aborts, and to
//! `logs/last-crash.txt`, which the next start reads ([`super::session`]).
//!
//! It runs for every thread's panics, GPUI's executor and dispatch threads too: a
//! panic there aborts the app ("panic in a function that cannot unwind"), and before
//! this its message only went to stderr, lost when the app was opened from Finder.
//! The default hook still runs afterwards, so a terminal still shows it.

use std::{
    backtrace::Backtrace,
    cell::Cell,
    fs::OpenOptions,
    io::Write as _,
    panic::PanicHookInfo,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use super::session::{self, CRASH_FILE};

/// The crash file stops growing past this (later panics of a run are appended).
const MAX_CRASH_FILE: u64 = 64 * 1024;

/// What the hook needs to know about the app.
pub struct Context {
    /// The logs folder; `None` writes no crash file.
    pub dir: Option<PathBuf>,
    pub app: &'static str,
    pub version: String,
    pub run_id: String,
}

/// Installs the hook in front of the one there now (usually the default).
pub fn install(context: Context) {
    let previous = std::panic::take_hook();
    let first = AtomicBool::new(true);
    std::panic::set_hook(Box::new(move |info| {
        thread_local! {
            static IN_HOOK: Cell<bool> = const { Cell::new(false) };
        }
        // A panic inside the hook itself (or with the thread's storage gone): only
        // the default hook, so it can't loop.
        if IN_HOOK.try_with(|flag| flag.replace(true)).unwrap_or(true) {
            previous(info);
            return;
        }
        let report = report(info, &context);
        let report = lodestone_trace::scrub_line(&report);
        if let Some(logger) = super::LOGGER.get() {
            logger.write_report(&report);
        }
        if let Some(dir) = &context.dir {
            write_crash(dir, &context, &report, first.swap(false, Ordering::SeqCst));
        }
        let _ = IN_HOOK.try_with(|flag| flag.set(false));
        previous(info);
    }));
}

/// The report: where, which thread, the message, the app, and a backtrace.
pub fn report(info: &PanicHookInfo, context: &Context) -> String {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".into());
    let place = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "an unknown place".into());
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    format!(
        "thread '{name}' ({:?}) panicked at {place}:\n{message}\n{} {}, run {}\nbacktrace:\n{}",
        thread.id(),
        context.app,
        context.version,
        context.run_id,
        Backtrace::force_capture()
    )
}

/// The run's first panic replaces the crash file; later ones are appended.
fn write_crash(dir: &std::path::Path, context: &Context, report: &str, first: bool) {
    let path = dir.join(CRASH_FILE);
    if first {
        let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let header = session::crash_header(&context.run_id, &at, context.app, &context.version);
        let _ = session::write_atomically(&path, format!("{header}\n{report}\n").as_bytes());
        return;
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_CRASH_FILE) {
        return;
    }
    if let Ok(mut file) = OpenOptions::new().append(true).open(&path) {
        let _ = write!(file, "\n{report}\n");
    }
}
