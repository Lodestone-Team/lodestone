//! The log file Lodestone Launcher and Lodestone Host write, so a failure can be
//! looked into after the fact: `<logs>/launcher.log` in the Launcher's default data
//! folder, `<Host folder>/logs/host.log` beside the core's `core.log`.
//!
//! - **Lines** are `2026-11-16T08:30:12.345Z WARN  target: message` ([`format`]),
//!   scrubbed of secrets ([`lodestone_trace::scrub_line`]).
//! - **Levels**: the apps' own crates at info (or debug, Settings), everything else at
//!   warnings; [`ENV`] (`LODESTONE_LOG`, written like `RUST_LOG`) replaces both.
//! - **Rotation**: at 5 MB the file becomes `.1`, up to `.5` ([`file`]).
//! - **Writes** are buffered: warnings and errors are flushed at once, the rest within
//!   a second by a thread that sleeps while nothing is written, and a panic's report
//!   synchronously ([`panic`]).
//! - **Crashes**: the panic hook keeps the last one in `last-crash.txt` (and the one
//!   before in `previous-crash.txt`), and the next start notices it ([`session`],
//!   [`Started::previous_crash`]).
//!
//! Records also go to the trace bus as `log.{level}`, as they did before there was a
//! file; failures traced on the bus come back the other way ([`crate::trace::emit`]).

mod file;
mod filter;
mod format;
mod panic;
mod session;
mod throttle;

use std::{
    cell::Cell,
    io::{IsTerminal as _, Write as _},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::json;

pub use file::{KEEP, MAX_BYTES};
pub use filter::Filter;
pub use format::escape_controls;
pub use session::{CRASH_FILE, Crash, PREVIOUS_CRASH_FILE, SESSION_FILE};
pub use throttle::{Allowed, Throttle};

use file::RotatingFile;

/// The variable that sets the filter, written like `RUST_LOG`
/// (`info,launcher_core::cores=debug`). It wins over the setting.
pub const ENV: &str = "LODESTONE_LOG";

/// The target of failures logged from the trace bus ([`crate::trace::emit`]); not
/// sent back to it.
pub const TRACE_TARGET: &str = "lodestone::trace";

/// The logs folder's name, in the Launcher's default data folder and the Host's folder.
pub const LOGS_DIR: &str = launcher_control::paths::LOGS_DIR;

/// Settings › Developer › Log detail (the Launcher), Settings › Updates › Log detail
/// (the Host): how much the apps' own crates log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    /// Last, as serde wants of the variant that reads unknown values.
    #[default]
    #[serde(other)]
    Info,
}

/// Which app is logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppKind {
    Launcher,
    Host,
}

impl AppKind {
    pub fn name(self) -> &'static str {
        match self {
            AppKind::Launcher => "Lodestone Launcher",
            AppKind::Host => "Lodestone Host",
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            AppKind::Launcher => "launcher.log",
            AppKind::Host => "host.log",
        }
    }
}

/// How [`install`] sets logging up.
pub struct Config {
    pub app: AppKind,
    /// The logs folder. `None` writes no files (the offscreen scenes without a data
    /// folder of their own): records only go to stderr, in a terminal.
    pub dir: Option<PathBuf>,
    /// The setting.
    pub level: LogLevel,
    /// [`ENV`]'s value.
    pub env_filter: Option<String>,
    pub version: String,
}

/// What [`install`] found.
#[derive(Clone, Debug, Default)]
pub struct Started {
    /// The last run ended in this crash: the app shows a notice, once.
    pub previous_crash: Option<Crash>,
}

/// What [`install`] set up, for the rest of the app.
struct Installed {
    app: AppKind,
    dir: Option<PathBuf>,
    run_id: String,
    env_override: bool,
    started: Started,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();
/// The setting's level is Debug (see [`level`]).
static DEBUG: AtomicBool = AtomicBool::new(false);
static INSTALLED: OnceLock<Installed> = OnceLock::new();

/// Sets logging up for the process: the logger, the panic hook, the run's marker and
/// the flushing thread. Call once, early; later calls change nothing and return what
/// the first one found.
pub fn install(config: Config) -> Started {
    if let Some(installed) = INSTALLED.get() {
        return installed.started.clone();
    }
    let (env, bad_env) = match config.env_filter.as_deref().map(str::trim) {
        Some(spec) if !spec.is_empty() => match Filter::parse(spec) {
            Ok(filter) => (Some(filter), None),
            Err(err) => (None, Some(err)),
        },
        _ => (None, None),
    };
    let env_override = env.is_some();
    DEBUG.store(config.level == LogLevel::Debug, Ordering::Relaxed);
    let logger = LOGGER.get_or_init(|| {
        Logger::new(
            config.dir.as_deref(),
            config.app.file_name(),
            env.unwrap_or_else(|| Filter::defaults(config.level)),
            std::io::stderr().is_terminal(),
        )
    });
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Trace);
    }
    let run_id = format!("{:016x}", rand::random::<u64>());
    let previous_crash = config
        .dir
        .as_deref()
        .and_then(|dir| session::begin(dir, &run_id, &config.version));
    panic::install(panic::Context {
        dir: config.dir.clone(),
        app: config.app.name(),
        version: config.version.clone(),
        run_id: run_id.clone(),
    });
    logger.start_flusher();
    let started = Started { previous_crash };
    let _ = INSTALLED.set(Installed {
        app: config.app,
        dir: config.dir,
        run_id: run_id.clone(),
        env_override,
        started: started.clone(),
    });
    log::info!(
        "{} {} started (pid {}, run {run_id}, {} {})",
        config.app.name(),
        config.version,
        std::process::id(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    if let Some(err) = bad_env {
        log::warn!("{ENV} was ignored: {err}");
    }
    if let Some(crash) = &started.previous_crash {
        log::warn!(
            "the last run crashed at {}: {} (see {})",
            crash.at,
            crash.summary,
            CRASH_FILE
        );
    }
    started
}

/// Changes how much the apps' own crates log (the setting). Does nothing while [`ENV`]
/// sets the filter.
pub fn set_level(level: LogLevel) {
    let Some(installed) = INSTALLED.get() else {
        return;
    };
    if installed.env_override {
        return;
    }
    DEBUG.store(level == LogLevel::Debug, Ordering::Relaxed);
    if let Some(logger) = LOGGER.get() {
        logger.set_filter(Filter::defaults(level));
        log::info!("log detail is now {level:?}");
    }
}

/// The setting's level, as last given ([`ENV`] may set the filter instead).
pub fn level() -> LogLevel {
    if DEBUG.load(Ordering::Relaxed) {
        LogLevel::Debug
    } else {
        LogLevel::Info
    }
}

/// The logs folder, once [`install`] set one.
pub fn logs_dir() -> Option<PathBuf> {
    INSTALLED.get()?.dir.clone()
}

/// Whether [`ENV`] sets the filter.
pub fn env_override() -> bool {
    INSTALLED.get().is_some_and(|i| i.env_override)
}

/// What [`install`] found, if it ran.
pub fn started() -> Option<Started> {
    INSTALLED.get().map(|i| i.started.clone())
}

/// Writes what is buffered, after making sure the file is still where it should be
/// (someone may have moved it away to start afresh).
pub fn flush() {
    if let Some(logger) = LOGGER.get() {
        log::Log::flush(logger);
    }
}

/// Closes the log file until the guard is dropped, so its folder can be renamed:
/// Windows refuses to rename a folder while a file in it is open. Lines logged
/// meanwhile wait in memory (a little at most) and are written when it reopens.
#[must_use = "the file reopens when the guard is dropped"]
pub fn release_file() -> FileReleased {
    if let Some(logger) = LOGGER.get()
        && let Some(file) = logger.lock_file().as_mut()
    {
        file.release();
    }
    FileReleased(())
}

/// Reopens the log file when dropped ([`release_file`]).
pub struct FileReleased(());

impl Drop for FileReleased {
    fn drop(&mut self) {
        if let Some(logger) = LOGGER.get()
            && let Some(file) = logger.lock_file().as_mut()
        {
            file.resume();
        }
    }
}

/// The app is quitting cleanly: flushes and removes the run's marker, so the next
/// start shows no crash notice.
pub fn end_session() {
    let Some(installed) = INSTALLED.get() else {
        return;
    };
    log::info!("{} quit", installed.app.name());
    flush();
    if let Some(dir) = &installed.dir {
        session::end(dir, &installed.run_id);
    }
}

/// The version a log names: "1.2.0 (build 812, 1a2b3c4)", or "(development build)"
/// for one the publish script didn't make.
pub fn version_label(version: &str, build: Option<crate::update::Build>) -> String {
    match build {
        Some(build) => match build.commit {
            Some(commit) => format!(
                "{version} (build {}, {})",
                build.number,
                crate::update::short_commit(commit)
            ),
            None => format!("{version} (build {})", build.number),
        },
        None => format!("{version} (development build)"),
    }
}

/// A serde error about untrusted input, without the values it quotes: serde writes a
/// value it couldn't use in double quotes (`invalid type: string "steve"`), which can be
/// a player's name or a console line. Names in backticks stay when they look like
/// names (letters, digits and `_`, at most 64): the format's own variants and fields,
/// which is what a fix needs; an unknown variant or field is the input's own text, so
/// anything else in backticks goes too. The line and column stay.
pub fn serde_error(err: &serde_json::Error) -> String {
    const MAX_NAME: usize = 64;
    let plain = |name: &str| {
        !name.is_empty()
            && name.len() <= MAX_NAME
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    let mut text = err.to_string();
    // serde writes the input's unknown name with `{}`, backticks unescaped, so it runs
    // to serde's own text after it, not to the next backtick.
    for prefix in ["unknown variant `", "unknown field `"] {
        let Some(start) = text.find(prefix).map(|n| n + prefix.len()) else {
            continue;
        };
        let end = ["`, expected", "`, there are no"]
            .iter()
            .filter_map(|after| text[start..].rfind(after))
            .max()
            .or_else(|| text[start..].rfind('`'))
            .map(|n| start + n);
        if let Some(end) = end
            && !plain(&text[start..end])
        {
            text.replace_range(start..end, "…");
        }
    }
    let mut out = String::with_capacity(text.len());
    // serde writes a value with `{:?}`, so a quote inside it is escaped (`\"`): the
    // character after a backslash never ends the quoted part.
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut escaped = false;
                for c in chars.by_ref() {
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '"' => break,
                        _ => {}
                    }
                }
                out.push_str("\"…\"");
            }
            '`' => {
                let mut name = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '`' {
                        closed = true;
                        break;
                    }
                    name.push(c);
                }
                if closed && plain(&name) {
                    out.push('`');
                    out.push_str(&name);
                    out.push('`');
                } else {
                    out.push_str("`…`");
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Makes the logs folder if needed, the user's alone on Unix (as logging makes it).
pub fn make_dir(dir: &Path) -> std::io::Result<()> {
    file::private_dir(dir)
}

/// The files in the logs folder `dir` for `app`: its log, newest first, then the crash
/// reports.
pub fn files(dir: &Path, app: AppKind) -> Vec<PathBuf> {
    launcher_control::logs::files(dir, app.file_name())
}

/// The last `lines` records of `path` (and of `path.1` when `path` has fewer), at
/// `min_level` or more severe when given ([`launcher_control::logs::tail`]).
pub fn tail(
    path: &Path,
    lines: usize,
    min_level: Option<log::Level>,
) -> std::io::Result<Vec<String>> {
    launcher_control::logs::tail(path, lines, min_level.map(|level| level as usize))
}

/// The logger: the file (when there is one), stderr in a terminal, and the trace bus.
pub struct Logger {
    filter: RwLock<Filter>,
    file: Mutex<Option<RotatingFile>>,
    /// Records also go to stderr: the app was started from a terminal.
    echo: bool,
    /// Something was written since the last flush.
    dirty: AtomicBool,
    /// Wakes the flushing thread.
    wake: OnceLock<Mutex<mpsc::Sender<()>>>,
}

impl Logger {
    /// A logger writing `dir/file_name` (nothing when `dir` is `None`).
    pub fn new(dir: Option<&Path>, file_name: &str, filter: Filter, echo: bool) -> Self {
        Self {
            filter: RwLock::new(filter),
            file: Mutex::new(dir.map(|dir| RotatingFile::open(dir, file_name))),
            echo,
            dirty: AtomicBool::new(false),
            wake: OnceLock::new(),
        }
    }

    pub fn set_filter(&self, filter: Filter) {
        *self.filter.write().unwrap_or_else(|p| p.into_inner()) = filter;
    }

    fn file_enabled(&self, level: log::Level, target: &str) -> bool {
        self.filter
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .enabled(level, target)
    }

    fn write(&self, text: &str, flush: bool) {
        if self.echo {
            let _ = std::io::stderr().write_all(text.as_bytes());
        }
        let wake = {
            let mut file = self.lock_file();
            let Some(file) = file.as_mut() else {
                return;
            };
            file.write(text);
            if flush {
                file.flush();
            }
            !flush && !self.dirty.swap(true, Ordering::Relaxed)
        };
        if wake && let Some(wake) = self.wake.get() {
            let _ = wake.lock().unwrap_or_else(|p| p.into_inner()).send(());
        }
    }

    /// The file, locked, with [`HOLDS_FILE`] set while the guard lives.
    fn lock_file(&self) -> FileGuard<'_> {
        let guard = self.file.lock().unwrap_or_else(|p| p.into_inner());
        HOLDS_FILE.with(|holds| holds.set(true));
        FileGuard(guard)
    }

    /// Writes a panic's report (already scrubbed) and flushes it, waiting for the file
    /// as long as another thread writes. When this thread holds the file (it panicked
    /// while writing), the log can't have it; the crash file still does.
    fn write_report(&self, report: &str) {
        if HOLDS_FILE.with(Cell::get) {
            return;
        }
        let line = format::record(
            chrono::Utc::now(),
            log::Level::Error,
            "panic",
            report,
            None,
            0,
        );
        Self::write_now(&mut self.lock_file(), &line);
    }

    fn write_now(file: &mut Option<RotatingFile>, line: &str) {
        if let Some(file) = file {
            file.write(line);
            file.flush();
        }
    }

    /// Starts the thread that flushes buffered lines a second after they were
    /// written. It sleeps while nothing is.
    fn start_flusher(&'static self) {
        let (tx, rx) = mpsc::channel::<()>();
        if self.wake.set(Mutex::new(tx)).is_err() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("log-flusher".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    std::thread::sleep(Duration::from_secs(1));
                    log::Log::flush(self);
                }
            });
    }
}

thread_local! {
    /// This thread holds the logger's file lock: a panic while writing mustn't wait
    /// for it. A `const` `Cell` has no destructor, so it can be read even while the
    /// thread ends.
    static HOLDS_FILE: Cell<bool> = const { Cell::new(false) };
}

/// The logger's file lock, which clears [`HOLDS_FILE`] when dropped.
struct FileGuard<'a>(std::sync::MutexGuard<'a, Option<RotatingFile>>);

impl std::ops::Deref for FileGuard<'_> {
    type Target = Option<RotatingFile>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for FileGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for FileGuard<'_> {
    fn drop(&mut self) {
        HOLDS_FILE.with(|holds| holds.set(false));
    }
}

/// Whether a record goes to the trace bus: info and above from anywhere; debug and
/// trace only from Lodestone's own crates and GPUI, since the network libraries'
/// detailed records (TLS, HTTP framing) are noise and could describe requests in
/// detail. Never the failures the bus itself sent here.
fn forwarded(metadata: &log::Metadata) -> bool {
    metadata.target() != TRACE_TARGET
        && (metadata.level() <= log::Level::Info
            || ["lodestone", "launcher", "gpui"]
                .iter()
                .any(|own| metadata.target().starts_with(own)))
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

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.file_enabled(metadata.level(), metadata.target())
            || (forwarded(metadata) && crate::trace::bus().wants(level_topic(metadata.level())))
    }

    fn log(&self, record: &log::Record) {
        let (level, target) = (record.level(), record.target());
        let to_file = self.file_enabled(level, target);
        let to_bus = forwarded(record.metadata()) && crate::trace::bus().wants(level_topic(level));
        if !to_file && !to_bus {
            return;
        }
        // A caller's Display that fails mustn't panic the thread that logged.
        let mut message = String::new();
        if std::fmt::write(&mut message, *record.args()).is_err() {
            message.push_str(" …[format error]");
        }
        // Scrubbed once, for the file and the bus alike. A huge message (a body in an
        // error) is cut first, so scrubbing costs little on the thread that logged;
        // twice what the file keeps, so a secret at the file's cut is still seen.
        let whole = message.len();
        let message = format::truncate(&message, 2 * format::MAX_MESSAGE);
        let cut = whole - message.len();
        let message = lodestone_trace::scrub_line(message).into_owned();
        if to_file {
            let line = format::record(
                chrono::Utc::now(),
                level,
                target,
                &message,
                Some(format::MAX_MESSAGE),
                cut,
            );
            self.write(&line, level <= log::Level::Warn);
        }
        // The bus is called with no lock of ours held, and never calls back in.
        if to_bus {
            crate::trace::bus().emit(level_topic(level), || {
                json!({
                    "target": target,
                    "message": message,
                    "file": record.file(),
                    "line": record.line(),
                })
            });
        }
    }

    fn flush(&self) {
        self.dirty.store(false, Ordering::Relaxed);
        if let Some(file) = self.lock_file().as_mut() {
            file.reattach();
            file.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log as _;

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
        assert!(
            !meta(log::Level::Warn, TRACE_TARGET),
            "it came from the bus"
        );
    }

    fn record(logger: &Logger, level: log::Level, target: &str, message: &str) {
        logger.log(
            &log::Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{message}"))
                .build(),
        );
    }

    #[test]
    fn the_file_gets_what_the_filter_lets_through() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(
            Some(dir.path()),
            "test.log",
            Filter::defaults(LogLevel::Info),
            false,
        );
        record(
            &logger,
            log::Level::Info,
            "lodestone_launcher::shell",
            "own info",
        );
        record(
            &logger,
            log::Level::Debug,
            "lodestone_launcher::shell",
            "own debug",
        );
        record(&logger, log::Level::Info, "rustls::client", "their info");
        record(&logger, log::Level::Warn, "rustls::client", "their warning");
        record(
            &logger,
            log::Level::Warn,
            "launcher_core::accounts",
            "token=hunter2 refused",
        );
        // a warning is flushed at once
        let text = std::fs::read_to_string(dir.path().join("test.log")).unwrap();
        assert!(
            text.contains("INFO  lodestone_launcher::shell: own info"),
            "{text}"
        );
        assert!(
            text.contains("WARN  rustls::client: their warning"),
            "{text}"
        );
        assert!(
            !text.contains("own debug") && !text.contains("their info"),
            "{text}"
        );
        assert!(text.contains("token=[redacted] refused") && !text.contains("hunter2"));

        logger.set_filter(Filter::defaults(LogLevel::Debug));
        record(
            &logger,
            log::Level::Debug,
            "lodestone_launcher::shell",
            "own debug",
        );
        logger.flush();
        let text = std::fs::read_to_string(dir.path().join("test.log")).unwrap();
        assert!(text.contains("own debug"));
    }

    #[test]
    fn a_logger_without_a_folder_writes_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(None, "test.log", Filter::defaults(LogLevel::Debug), false);
        record(&logger, log::Level::Error, "lodestone_host", "nowhere");
        logger.flush();
        logger.write_report("a report");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn records_reach_the_bus_without_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(
            Some(dir.path()),
            "test.log",
            Filter::defaults(LogLevel::Info),
            false,
        );
        let (tx, rx) = mpsc::channel();
        let id = crate::trace::bus().subscribe(
            lodestone_trace::Filter::new(["log.error"]),
            0,
            None,
            Box::new(move |e| tx.send(e.clone()).is_ok()),
        );
        record(
            &logger,
            log::Level::Error,
            "lodestone_ui::toast",
            "bus check: Bearer abc password=hunter2",
        );
        crate::trace::bus().unsubscribe(id);
        let event = rx
            .try_iter()
            .find(|e| {
                e.data["message"]
                    .as_str()
                    .is_some_and(|m| m.starts_with("bus check"))
            })
            .expect("the record on the bus");
        assert_eq!(
            event.data["message"],
            "bus check: Bearer [redacted] password=[redacted]"
        );
    }

    #[test]
    fn a_line_is_scrubbed_once() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(
            Some(dir.path()),
            "test.log",
            Filter::defaults(LogLevel::Info),
            false,
        );
        record(
            &logger,
            log::Level::Warn,
            "lodestone_launcher",
            "request failed: Bearer abc",
        );
        logger.write_report("thread 'x' panicked: Bearer [redacted]");
        let text = std::fs::read_to_string(dir.path().join("test.log")).unwrap();
        assert!(
            text.contains("request failed: Bearer [redacted]\n"),
            "{text}"
        );
        assert!(text.contains("panicked: Bearer [redacted]\n"), "{text}");
        assert!(!text.contains("]]"), "{text}");
    }

    #[test]
    fn a_huge_message_is_cut_before_it_is_scrubbed() {
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(
            Some(dir.path()),
            "test.log",
            Filter::defaults(LogLevel::Info),
            false,
        );
        let body = format!("password=hunter2 {}", "x ".repeat(2 * 1024 * 1024));
        record(&logger, log::Level::Warn, "lodestone_launcher", &body);
        let text = std::fs::read_to_string(dir.path().join("test.log")).unwrap();
        assert!(text.len() < format::MAX_MESSAGE + 200, "{}", text.len());
        assert!(!text.contains("hunter2"));
        // what was cut before scrubbing counts too (scrubbing changed a few bytes)
        let cut: usize = text
            .trim_end()
            .rsplit("…[cut ")
            .next()
            .and_then(|rest| rest.strip_suffix(" bytes]"))
            .and_then(|n| n.parse().ok())
            .expect("the cut is said");
        assert!(cut.abs_diff(body.len() - format::MAX_MESSAGE) < 16, "{cut}");
    }

    #[test]
    fn a_failing_display_never_panics_the_logger() {
        struct Failing;
        impl std::fmt::Display for Failing {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let logger = Logger::new(
            Some(dir.path()),
            "test.log",
            Filter::defaults(LogLevel::Info),
            false,
        );
        logger.log(
            &log::Record::builder()
                .level(log::Level::Warn)
                .target("lodestone_launcher")
                .args(format_args!("before {} after", Failing))
                .build(),
        );
        let text = std::fs::read_to_string(dir.path().join("test.log")).unwrap();
        assert!(text.contains("[format error]"), "{text}");
    }

    #[test]
    fn version_labels_name_the_build() {
        assert_eq!(version_label("1.0.0", None), "1.0.0 (development build)");
        let build = crate::update::Build {
            number: 812,
            commit: Some("1a2b3c4d5e6f"),
            dev: false,
        };
        assert!(version_label("1.0.0", Some(build)).starts_with("1.0.0 (build 812, 1a2b3c4"));
    }

    #[test]
    fn tails_filter_by_the_logs_levels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        std::fs::write(
            &path,
            "2026-11-16T08:00:00.000Z INFO  a: one\n2026-11-16T08:00:01.000Z WARN  a: two\n",
        )
        .unwrap();
        let warnings = tail(&path, 10, Some(log::Level::Warn)).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].ends_with("two"));
        assert_eq!(tail(&path, 10, Some(log::Level::Debug)).unwrap().len(), 2);
    }

    #[test]
    fn serde_errors_lose_the_values_they_quote() {
        let err = serde_json::from_str::<u64>(r#""steve""#).unwrap_err();
        let text = serde_error(&err);
        assert!(!text.contains("steve"), "{text}");
        assert!(text.contains("invalid type: string \"…\""), "{text}");
        // A quote inside the value is escaped, and doesn't end the hidden part.
        let err = serde_json::from_str::<u64>(r#""he said \"secret\" ok""#).unwrap_err();
        let text = serde_error(&err);
        assert!(!text.contains("secret") && !text.contains("ok"), "{text}");
        assert!(
            text.contains("invalid type: string \"…\", expected"),
            "{text}"
        );
        #[derive(Debug, serde::Deserialize)]
        enum Kind {
            Known,
        }
        let err = serde_json::from_str::<Kind>(r#""Other""#).unwrap_err();
        assert!(serde_error(&err).contains("`Other`"), "variant names stay");
        assert!(
            serde_error(&err).contains("`Known`"),
            "{}",
            serde_error(&err)
        );
        // an unknown variant is the input's own text: only a name-like one stays
        for hostile in ["steve joined the game", "a`b", &"x".repeat(100)] {
            let json = serde_json::to_string(hostile).unwrap();
            let err = serde_json::from_str::<Kind>(&json).unwrap_err();
            let text = serde_error(&err);
            assert!(!text.contains("steve") && !text.contains("xxxx"), "{text}");
            assert!(text.contains("expected `Known`"), "{text}");
        }
    }

    #[test]
    fn levels_read_unknown_values_as_info() {
        assert_eq!(
            serde_json::from_str::<LogLevel>("\"debug\"").unwrap(),
            LogLevel::Debug
        );
        assert_eq!(
            serde_json::from_str::<LogLevel>("\"trace\"").unwrap(),
            LogLevel::Info
        );
        assert_eq!(serde_json::to_string(&LogLevel::Info).unwrap(), "\"info\"");
    }
}
