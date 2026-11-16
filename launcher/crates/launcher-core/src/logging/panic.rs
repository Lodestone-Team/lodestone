//! The panic hook: a panic's message, place, thread and backtrace go to the log file,
//! flushed before the process carries on unwinding or aborts, and to
//! `logs/last-crash.txt`, which the next start reads ([`super::session`]).
//!
//! It runs for every thread's panics, GPUI's executor and dispatch threads too: a
//! panic there aborts the app ("panic in a function that cannot unwind"), and before
//! this its message only went to stderr, lost when the app was opened from Finder.
//! The default hook still runs afterwards, so a terminal still shows it.
//!
//! A panic someone catches (`catch_unwind` around a library a LAN device can upset,
//! Tokio around a task) is reported too: the hook can't tell. So a report is bounded:
//! the message is cut at [`MAX_MESSAGE`], only a run's first [`FULL_BACKTRACES`]
//! panics carry a backtrace, and the log takes [`LOGGED_PER_MINUTE`] reports a minute
//! from each place (a task that panics on every connection would otherwise rotate the
//! first report away). The crash file takes every one, at the cost of one report each
//! once it is full, so the panic that ends the run is always in it.

use std::{
    backtrace::Backtrace,
    cell::Cell,
    fs::OpenOptions,
    io::{Seek as _, SeekFrom, Write as _},
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{
    session::{self, CRASH_FILE, PREVIOUS_CRASH_FILE},
    throttle::{Allowed, Throttle},
};

/// The crash file stops growing past this: later panics of a run are appended until
/// then, and past it the file keeps the run's first report and its newest.
const MAX_CRASH_FILE: u64 = 64 * 1024;
/// The longest panic message kept (an `unwrap()` on an error with a body in it).
pub const MAX_MESSAGE: usize = 16 * 1024;
/// Panics per run whose report has a backtrace.
pub const FULL_BACKTRACES: usize = 3;
/// Reports a minute the log takes from one place in the code.
pub const LOGGED_PER_MINUTE: u32 = 5;
/// What separates the first report from the newest in a full crash file.
const NEWEST: &str = "(later panics left out; the newest:)";

/// What the hook needs to know about the app.
pub struct Context {
    /// The logs folder; `None` writes no crash file.
    pub dir: Option<PathBuf>,
    pub app: &'static str,
    pub version: String,
    pub run_id: String,
}

/// What this run wrote to the crash file.
#[derive(Default)]
struct CrashFile {
    /// The run's first report with its header, as the file starts with it.
    first: Option<String>,
    /// The file is full: it is the first report and the newest, and each later panic
    /// replaces the newest.
    full: bool,
}

/// Installs the hook in front of the one there now (usually the default).
pub fn install(context: Context) {
    let previous = std::panic::take_hook();
    let panics = AtomicUsize::new(0);
    let logged = Throttle::with_limit(LOGGED_PER_MINUTE);
    // Held while the crash file is written, so two threads panicking at once write
    // their reports in one order: the first replaces the file, the other is added.
    let crash = Mutex::new(CrashFile::default());
    std::panic::set_hook(Box::new(move |info| {
        thread_local! {
            // `const` with no destructor: readable even while the thread ends.
            static IN_HOOK: Cell<bool> = const { Cell::new(false) };
        }
        // A panic inside the hook itself: only the default hook, so it can't loop (or
        // take the crash file's lock twice).
        if IN_HOOK.with(|flag| flag.replace(true)) {
            previous(info);
            return;
        }
        let nth = panics.fetch_add(1, Ordering::SeqCst);
        let report = report(info, &context, nth < FULL_BACKTRACES);
        // Scrubbed once, here; the log writes it as it is.
        let report = lodestone_trace::scrub_line(&report).into_owned();
        let place = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let logged_report = match logged.allow(&place) {
            Allowed::Yes => Some(report.clone()),
            Allowed::YesAfterSuppressing(left_out) => Some(format!(
                "{report}\n({left_out} more panics here in the last minute weren't logged; \
                 the crash file has the newest)"
            )),
            Allowed::No => None,
        };
        if let Some(logged_report) = logged_report
            && let Some(logger) = super::LOGGER.get()
        {
            logger.write_report(&logged_report);
        }
        if let Some(dir) = &context.dir {
            let mut crash = crash.lock().unwrap_or_else(|p| p.into_inner());
            write_crash(dir, &context, &escape_lines(&report), &mut crash);
        }
        IN_HOOK.with(|flag| flag.set(false));
        previous(info);
    }));
}

/// The report with each line's control characters escaped, as the log writes them: the
/// crash file is sent and printed like a log ([`super::format::escape_controls`]).
fn escape_lines(report: &str) -> String {
    report
        .lines()
        .map(super::format::escape_controls)
        .collect::<Vec<_>>()
        .join("\n")
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
/// [`MAX_CRASH_FILE`] (panics the run survived), it is the first report and the
/// newest, so the panic that ends the run is always there: written whole once, then
/// each newer report replaces the last by cutting the file back to the first.
fn write_crash(dir: &Path, context: &Context, report: &str, crash: &mut CrashFile) {
    let path = dir.join(CRASH_FILE);
    let Some(first) = &crash.first else {
        if path.is_file() {
            let _ = std::fs::rename(&path, dir.join(PREVIOUS_CRASH_FILE));
        }
        let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let header = session::crash_header(&context.run_id, &at, context.app, &context.version);
        let text = format!("{header}\n{report}\n");
        let _ = session::write_atomically(&path, text.as_bytes());
        crash.first = Some(text);
        return;
    };
    let kept = format!("{first}\n{NEWEST}\n\n");
    if crash.full && replace_newest(&path, kept.len() as u64, report).is_ok() {
        return;
    }
    if crash.full || std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_CRASH_FILE) {
        let _ = session::write_atomically(&path, format!("{kept}{report}\n").as_bytes());
        crash.full = true;
        return;
    }
    if let Ok(mut file) = OpenOptions::new().append(true).open(&path) {
        let _ = write!(file, "\n{report}\n");
    }
}

/// Cuts a full crash file back to its first `kept` bytes (the first report) and adds
/// `report` as the newest: one report's writing, where rewriting it whole would be the
/// file's. A file shorter than that isn't the one this run wrote: an error, so the
/// caller writes it whole.
fn replace_newest(path: &Path, kept: u64, report: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(path)?;
    if file.metadata()?.len() < kept {
        return Err(std::io::Error::other("not this run's crash file"));
    }
    file.set_len(kept)?;
    file.seek(SeekFrom::End(0))?;
    writeln!(file, "{report}")
}
