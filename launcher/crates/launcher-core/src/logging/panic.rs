//! The panic hook: a panic's message, place, thread and backtrace go to the log file,
//! flushed before the process carries on unwinding or aborts, and to
//! `logs/last-crash.txt`, which the next start reads ([`super::session`]).
//!
//! It runs for every thread's panics, GPUI's executor and dispatch threads too: a
//! panic there aborts the app ("panic in a function that cannot unwind"), and before
//! this its message only went to stderr, lost when the app was opened from Finder.
//! The default hook still runs afterwards, so a terminal still shows it.
//!
//! A panic someone catches (`catch_unwind` around a library a LAN device can upset)
//! is reported too: the hook can't tell. So a report is bounded: the message is cut
//! at [`MAX_MESSAGE`], and only a run's first [`FULL_BACKTRACES`] panics carry a
//! backtrace.

use std::{
    backtrace::Backtrace,
    cell::Cell,
    fs::OpenOptions,
    io::Write as _,
    panic::PanicHookInfo,
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::session::{self, CRASH_FILE, PREVIOUS_CRASH_FILE};

/// The crash file stops growing past this: later panics of a run are appended until
/// then, and past it the file keeps the run's first report and its newest.
const MAX_CRASH_FILE: u64 = 64 * 1024;
/// The longest panic message kept (an `unwrap()` on an error with a body in it).
pub const MAX_MESSAGE: usize = 16 * 1024;
/// Panics per run whose report has a backtrace.
pub const FULL_BACKTRACES: usize = 3;

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
    let panics = AtomicUsize::new(0);
    // the run's first report, as the crash file starts with it
    let first = OnceLock::new();
    std::panic::set_hook(Box::new(move |info| {
        thread_local! {
            // `const` with no destructor: readable even while the thread ends.
            static IN_HOOK: Cell<bool> = const { Cell::new(false) };
        }
        // A panic inside the hook itself: only the default hook, so it can't loop.
        if IN_HOOK.with(|flag| flag.replace(true)) {
            previous(info);
            return;
        }
        let nth = panics.fetch_add(1, Ordering::SeqCst);
        let report = report(info, &context, nth < FULL_BACKTRACES);
        // Scrubbed once, here; the log writes it as it is.
        let report = lodestone_trace::scrub_line(&report);
        if let Some(logger) = super::LOGGER.get() {
            logger.write_report(&report);
        }
        if let Some(dir) = &context.dir {
            write_crash(dir, &context, &report, nth == 0, &first);
        }
        IN_HOOK.with(|flag| flag.set(false));
        previous(info);
    }));
}

/// The report: where, which thread, the message (cut at [`MAX_MESSAGE`]), the app,
/// and a backtrace when `backtrace` is set.
pub fn report(info: &PanicHookInfo, context: &Context, backtrace: bool) -> String {
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(no message)");
    let mut message = super::format::truncate(payload, MAX_MESSAGE).to_string();
    if message.len() < payload.len() {
        message.push_str(&format!(" …[cut {} bytes]", payload.len() - message.len()));
    }
    let place = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "an unknown place".into());
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("<unnamed>");
    let backtrace = if backtrace {
        Backtrace::force_capture().to_string()
    } else {
        format!("(left out: this run's first {FULL_BACKTRACES} panics have one)")
    };
    format!(
        "thread '{name}' ({:?}) panicked at {place}:\n{message}\n{} {}, run {}\nbacktrace:\n{backtrace}",
        thread.id(),
        context.app,
        context.version,
        context.run_id,
    )
}

/// The run's first panic replaces the crash file, keeping the one it had as
/// [`PREVIOUS_CRASH_FILE`] (a crash the start just reported, which a panic this run
/// survived mustn't hide); later ones are appended. Once the file is past
/// [`MAX_CRASH_FILE`] (panics the run survived), it is rewritten as the first report
/// and the newest, so the panic that ends the run is always there.
fn write_crash(
    dir: &std::path::Path,
    context: &Context,
    report: &str,
    is_first: bool,
    first: &OnceLock<String>,
) {
    let path = dir.join(CRASH_FILE);
    if is_first {
        if path.is_file() {
            let _ = std::fs::rename(&path, dir.join(PREVIOUS_CRASH_FILE));
        }
        let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let header = session::crash_header(&context.run_id, &at, context.app, &context.version);
        let text = format!("{header}\n{report}\n");
        let _ = session::write_atomically(&path, text.as_bytes());
        let _ = first.set(text);
        return;
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_CRASH_FILE) {
        if let Some(first) = first.get() {
            let text = format!("{first}\n(later panics left out; the newest:)\n\n{report}\n");
            let _ = session::write_atomically(&path, text.as_bytes());
        }
        return;
    }
    if let Ok(mut file) = OpenOptions::new().append(true).open(&path) {
        let _ = write!(file, "\n{report}\n");
    }
}
