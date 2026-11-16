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
//! - **Crashes**: the panic hook keeps the last one in `last-crash.txt`, and the next
//!   start notices it ([`session`], [`Started::previous_crash`]).
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
    io::{IsTerminal as _, Write as _},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::json;

pub use file::{KEEP, MAX_BYTES};
pub use filter::Filter;
pub use session::{CRASH_FILE, Crash, SESSION_FILE};
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
    /// [`ENV`] sets the filter, so the setting does nothing.
    pub env_override: bool,
}

/// What [`install`] set up, for the rest of the app.
struct Installed {
    app: AppKind,
    dir: Option<PathBuf>,
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
    let started = Started {
        previous_crash,
        env_override,
    };
    let _ = INSTALLED.set(Installed {
        app: config.app,
        dir: config.dir,
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

/// The app's log file, once [`install`] set one.
pub fn log_file() -> Option<PathBuf> {
    let installed = INSTALLED.get()?;
    Some(installed.dir.as_ref()?.join(installed.app.file_name()))
}

/// Whether [`ENV`] sets the filter.
pub fn env_override() -> bool {
    INSTALLED.get().is_some_and(|i| i.env_override)
}

/// What [`install`] found, if it ran.
pub fn started() -> Option<Started> {
    INSTALLED.get().map(|i| i.started.clone())
}

/// Writes what is buffered.
pub fn flush() {
    if let Some(logger) = LOGGER.get() {
        log::Log::flush(logger);
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
        session::end(dir);
    }
}

/// A serde error about untrusted input, without the values it quotes: serde writes a
/// value it couldn't use in double quotes (`invalid type: string "steve"`), which can be
/// a player's name or a console line. Names from the format itself (variants and
/// fields, in backticks) and the line and column stay, which is what a fix needs.
pub fn serde_error(err: &serde_json::Error) -> String {
    let text = err.to_string();
    let mut out = String::with_capacity(text.len());
    // serde writes the value with `{:?}`, so a quote inside it is escaped (`\"`): the
    // character after a backslash never ends the quoted part.
    let mut quoted = false;
    let mut escaped = false;
    for c in text.chars() {
        match (c, quoted) {
            (_, true) if escaped => escaped = false,
            ('\\', true) => escaped = true,
            ('"', false) => {
                quoted = true;
                out.push_str("\"…");
            }
            ('"', true) => {
                quoted = false;
                out.push('"');
            }
            (_, true) => {}
            (c, false) => out.push(c),
        }
    }
    out
}

/// The log files in `dir` for `app`, newest first.
pub fn files(dir: &Path, app: AppKind) -> Vec<PathBuf> {
    RotatingFile::files(dir, app.file_name())
}

/// The last `lines` records of `path` (and of `path.1` when `path` has fewer), at
/// `min_level` or more severe when given; a record's continuation lines count as
/// lines of their own and follow its level. Reads at most 1 MiB of each file.
pub fn tail(
    path: &Path,
    lines: usize,
    min_level: Option<log::Level>,
) -> std::io::Result<Vec<String>> {
    const MAX_READ: u64 = 1024 * 1024;
    let mut text = file::read_tail(path, MAX_READ)?;
    let rotated = {
        let mut name = path.as_os_str().to_owned();
        name.push(".1");
        PathBuf::from(name)
    };
    if text.lines().count() < lines
        && let Ok(older) = file::read_tail(&rotated, MAX_READ)
    {
        text = older + &text;
    }
    let mut keep = true;
    let mut out: Vec<String> = text
        .lines()
        .filter(|line| {
            if let Some(level) = format::level_of(line) {
                keep = min_level.is_none_or(|min| level <= min);
            }
            keep
        })
        .map(str::to_string)
        .collect();
    let skip = out.len().saturating_sub(lines);
    out.drain(..skip);
    Ok(out)
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
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        let Some(file) = file.as_mut() else {
            return;
        };
        file.write(text);
        if flush {
            file.flush();
        } else if !self.dirty.swap(true, Ordering::Relaxed)
            && let Some(wake) = self.wake.get()
        {
            let _ = wake.lock().unwrap_or_else(|p| p.into_inner()).send(());
        }
    }

    /// Writes a panic's report and flushes it, waiting at most 250 ms for the file
    /// (another thread may be writing; this one may have panicked while writing).
    fn write_report(&self, report: &str) {
        let line = format::record(chrono::Utc::now(), log::Level::Error, "panic", report, None);
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match self.file.try_lock() {
                Ok(mut file) => return Self::write_now(&mut file, &line),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    return Self::write_now(&mut poisoned.into_inner(), &line);
                }
                Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(std::sync::TryLockError::WouldBlock) => return,
            }
        }
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
        let message = record.args().to_string();
        if to_file {
            let line = format::record(
                chrono::Utc::now(),
                level,
                target,
                &message,
                Some(format::MAX_MESSAGE),
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
        if let Some(file) = self.file.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
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
            "bus check: Bearer abc",
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
        assert_eq!(event.data["message"], "bus check: Bearer [redacted]");
    }

    #[test]
    fn tails_read_the_last_records_across_a_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.log");
        std::fs::write(
            dir.path().join("app.log.1"),
            "2026-11-16T08:00:00.000Z INFO  a: one\n2026-11-16T08:00:01.000Z WARN  a: two\n  more of two\n",
        )
        .unwrap();
        std::fs::write(&path, "2026-11-16T08:00:02.000Z INFO  a: three\n").unwrap();
        let all = tail(&path, 10, None).unwrap();
        assert_eq!(all.len(), 4);
        assert!(all[0].ends_with("one") && all[3].ends_with("three"));
        assert_eq!(tail(&path, 1, None).unwrap().len(), 1);
        let warnings = tail(&path, 10, Some(log::Level::Warn)).unwrap();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].ends_with("two") && warnings[1] == "  more of two");
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
