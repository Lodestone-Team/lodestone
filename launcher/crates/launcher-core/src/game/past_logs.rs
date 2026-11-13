//! The logs and crash reports a game left in its folder, for the Logs tab to show
//! after the game has quit: `logs/latest.log` and the older `logs/*.log.gz`, the
//! `crash-reports`, and the JVM's own `hs_err_pid*.log` files.
//!
//! [`list`] finds them, newest first. [`read`] turns one into lines with the time,
//! level and thread the game wrote, as the live console has them.

use std::{
    io::Read as _,
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::log4j::Level;

/// The most lines of a file that are shown: its last ones.
pub const MAX_LINES: usize = 50_000;

/// The most of a file that is read, once unpacked.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// What a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `logs/latest.log`, `logs/debug.log` and the packed logs of earlier sessions.
    Log,
    /// `crash-reports/crash-….txt`, written by the game.
    CrashReport,
    /// `hs_err_pid….log`, written by Java when it crashes itself.
    JvmCrash,
}

/// A log or crash report on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFile {
    pub path: PathBuf,
    /// The file's name, which says when it is from (`2026-11-12-1.log.gz`).
    pub name: String,
    pub kind: Kind,
    pub modified: Option<SystemTime>,
    pub size: u64,
}

/// One line of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// `12:00:00`, on the lines that start an event.
    pub time: Option<String>,
    pub level: Level,
    /// The thread, on the lines that start an event.
    pub source: Option<String>,
    pub text: String,
}

/// A file's lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    pub lines: Vec<Line>,
    /// How many earlier lines were left out (see [`MAX_LINES`]).
    pub skipped: usize,
}

/// The files of `dir` that `keep` picks, as `kind`.
fn files_in(dir: &Path, kind: Kind, keep: impl Fn(&str) -> bool, found: &mut Vec<LogFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if meta.is_file() && keep(&name.to_lowercase()) {
            found.push(LogFile {
                path: entry.path(),
                name,
                kind,
                modified: meta.modified().ok(),
                size: meta.len(),
            });
        }
    }
}

/// The logs and crash reports in a game folder, newest first.
pub fn list(game: &Path) -> Vec<LogFile> {
    let mut found = Vec::new();
    files_in(
        &game.join("logs"),
        Kind::Log,
        |name| name.ends_with(".log") || name.ends_with(".log.gz"),
        &mut found,
    );
    files_in(
        &game.join("crash-reports"),
        Kind::CrashReport,
        |name| name.ends_with(".txt"),
        &mut found,
    );
    files_in(
        game,
        Kind::JvmCrash,
        |name| name.starts_with("hs_err_pid") && name.ends_with(".log"),
        &mut found,
    );
    found.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
    found
}

/// `12:00:00` from what a log line has in its first brackets: `12:00:00`, or Forge's
/// `12Nov2026 12:00:00.123`. `None` if that isn't a time.
fn time_of(stamp: &str) -> Option<String> {
    let time = stamp.rsplit(' ').next()?;
    let time = time.split('.').next()?;
    let mut parts = time.split(':');
    let plain = |part: Option<&str>| {
        part.is_some_and(|p| (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
    };
    (stamp.len() <= 32
        && plain(parts.next())
        && plain(parts.next())
        && plain(parts.next())
        && parts.next().is_none())
    .then(|| time.to_string())
}

/// A line that starts an event: `[12:00:00] [Server thread/INFO]: Done`, or with a
/// logger after the thread, as Forge writes: `[…] [main/INFO] [forge/CORE]: Done`.
fn event(line: &str) -> Option<Line> {
    let (stamp, rest) = line.strip_prefix('[')?.split_once(']')?;
    let time = time_of(stamp)?;
    let (who, mut rest) = rest.trim_start().strip_prefix('[')?.split_once(']')?;
    let (thread, level) = who.rsplit_once('/')?;
    let level = Level::parse(level)?;
    // loggers and markers, each in brackets, up to the colon
    while let Some((_, after)) = rest
        .trim_start()
        .strip_prefix('[')
        .and_then(|inner| inner.split_once(']'))
    {
        rest = after;
    }
    let text = rest.strip_prefix(':').unwrap_or(rest);
    Some(Line {
        time: Some(time),
        level,
        source: Some(thread.to_string()),
        text: text.strip_prefix(' ').unwrap_or(text).to_string(),
    })
}

/// Splits a file's text into lines. In a log, a line that doesn't start an event (a
/// stack trace, the rest of a message) keeps the level of the event before it. A
/// crash report has no events: its lines are shown as they are.
pub fn parse(text: &str, kind: Kind) -> Contents {
    let mut level = Level::Info;
    let mut lines: Vec<Line> = text
        .lines()
        .map(|line| {
            let line = line.trim_end_matches('\r');
            if kind == Kind::Log
                && let Some(event) = event(line)
            {
                level = event.level;
                return event;
            }
            Line {
                time: None,
                level,
                source: None,
                text: line.to_string(),
            }
        })
        .collect();
    let skipped = lines.len().saturating_sub(MAX_LINES);
    lines.drain(..skipped);
    Contents { lines, skipped }
}

/// Reads a file, unpacking a `.gz`. Bytes that aren't UTF-8 are replaced, not refused:
/// old logs are in the system's encoding.
pub fn read(file: &LogFile) -> std::io::Result<Contents> {
    let raw = std::fs::File::open(&file.path)?;
    let mut bytes = Vec::new();
    if file.name.to_lowercase().ends_with(".gz") {
        flate2::read::MultiGzDecoder::new(raw)
            .take(MAX_BYTES)
            .read_to_end(&mut bytes)?;
    } else {
        raw.take(MAX_BYTES).read_to_end(&mut bytes)?;
    }
    Ok(parse(&String::from_utf8_lossy(&bytes), file.kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn line(time: &str, level: Level, source: &str, text: &str) -> Line {
        Line {
            time: Some(time.into()),
            level,
            source: Some(source.into()),
            text: text.into(),
        }
    }

    fn rest(level: Level, text: &str) -> Line {
        Line {
            time: None,
            level,
            source: None,
            text: text.into(),
        }
    }

    #[test]
    fn log_lines_keep_their_time_level_and_thread() {
        let text = "[12:00:01] [Render thread/INFO]: Setting user: Sam\r\n\
[12:00:02] [Worker-Main-3/WARN]: Missing texture\n\
[12:00:03] [Render thread/ERROR]: Couldn't load: a/b\n\
java.lang.IllegalStateException: boom\n\
\tat net.minecraft.Main.run(Main.java:10)\n\
[12Nov2026 12:00:04.512] [main/INFO] [net.minecraftforge.fml.loading/CORE]: Loading 3 mods\n\
[12:00:05] [main/FATAL] [mixin/]: Mixin failed\n\
[not a time] and so on\n";
        assert_eq!(
            parse(text, Kind::Log).lines,
            vec![
                line(
                    "12:00:01",
                    Level::Info,
                    "Render thread",
                    "Setting user: Sam"
                ),
                line("12:00:02", Level::Warn, "Worker-Main-3", "Missing texture"),
                line(
                    "12:00:03",
                    Level::Error,
                    "Render thread",
                    "Couldn't load: a/b"
                ),
                // the stack trace is part of the error
                rest(Level::Error, "java.lang.IllegalStateException: boom"),
                rest(Level::Error, "\tat net.minecraft.Main.run(Main.java:10)"),
                line("12:00:04", Level::Info, "main", "Loading 3 mods"),
                line("12:00:05", Level::Fatal, "main", "Mixin failed"),
                rest(Level::Fatal, "[not a time] and so on"),
            ]
        );
    }

    #[test]
    fn a_crash_report_is_shown_as_it_is() {
        let text =
            "---- Minecraft Crash Report ----\n// Oops.\n\n[12:00:00] [main/ERROR]: quoted\n";
        assert_eq!(
            parse(text, Kind::CrashReport).lines,
            vec![
                rest(Level::Info, "---- Minecraft Crash Report ----"),
                rest(Level::Info, "// Oops."),
                rest(Level::Info, ""),
                rest(Level::Info, "[12:00:00] [main/ERROR]: quoted"),
            ]
        );
    }

    #[test]
    fn only_the_end_of_a_very_long_log_is_kept() {
        let text: String = (0..MAX_LINES + 7).map(|n| format!("line {n}\n")).collect();
        let contents = parse(&text, Kind::Log);
        assert_eq!(contents.skipped, 7);
        assert_eq!(contents.lines.len(), MAX_LINES);
        assert_eq!(contents.lines[0].text, "line 7");
    }

    #[test]
    fn the_files_of_a_game_folder_are_found_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::create_dir_all(game.join("crash-reports")).unwrap();
        let write = |relative: &str, age_days: u64, bytes: &[u8]| {
            let path = game.join(relative);
            std::fs::write(&path, bytes).unwrap();
            let when = SystemTime::now() - std::time::Duration::from_secs(age_days * 86_400);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        write("logs/latest.log", 0, b"[12:00:00] [main/INFO]: now\n");
        let mut packed = flate2::write::GzEncoder::new(Vec::new(), Default::default());
        packed
            .write_all(b"[09:00:00] [main/WARN]: earlier\n")
            .unwrap();
        write("logs/2026-11-10-1.log.gz", 3, &packed.finish().unwrap());
        write(
            "crash-reports/crash-2026-11-11_10.00.00-client.txt",
            2,
            b"---- Minecraft Crash Report ----\n",
        );
        write("hs_err_pid1234.log", 1, b"# A fatal error\n");
        // not logs
        write("logs/telemetry.json", 0, b"{}");
        write("options.txt", 0, b"");
        write("crash-reports/notes.md", 0, b"");

        let files = list(game);
        let found: Vec<(&str, Kind)> = files.iter().map(|f| (f.name.as_str(), f.kind)).collect();
        assert_eq!(
            found,
            vec![
                ("latest.log", Kind::Log),
                ("hs_err_pid1234.log", Kind::JvmCrash),
                ("crash-2026-11-11_10.00.00-client.txt", Kind::CrashReport),
                ("2026-11-10-1.log.gz", Kind::Log),
            ]
        );
        // a packed log is unpacked
        assert_eq!(
            read(&files[3]).unwrap().lines,
            vec![line("09:00:00", Level::Warn, "main", "earlier")]
        );
        assert_eq!(read(&files[1]).unwrap().lines[0].text, "# A fatal error");
        // a game that never ran has none
        assert!(list(&game.join("missing")).is_empty());
    }
}
