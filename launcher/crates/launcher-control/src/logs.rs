//! Reading the launcher's log files, shared by the launcher (`logs.tail`, `logs.path`)
//! and `lodestone-launcher-cli`, which reads them itself when no launcher answers (one
//! that crashes at every start is when the log is wanted most). The launcher writes
//! them (`launcher_core::logging`): `launcher.log`, rotated to `.1` … `.5`, the last
//! crash's report and the one before it, in [`crate::paths::logs_dir`].

use std::{
    fs::File,
    io::{self, Read as _, Seek as _, SeekFrom},
    path::{Path, PathBuf},
};

/// The launcher's log file.
pub const LOG_FILE: &str = "launcher.log";
/// Rotated files kept: `.1` (the newest) to `.5`.
pub const KEEP: usize = 5;
/// The last crash's report.
pub const CRASH_FILE: &str = "last-crash.txt";
/// The report before it.
pub const PREVIOUS_CRASH_FILE: &str = "previous-crash.txt";
/// How much of each file [`tail`] reads.
const MAX_READ: u64 = 1024 * 1024;

/// The files a logs folder lists for the log named `name` (`launcher.log`,
/// `host.log`): the log and its rotations, newest first, then the crash reports, those
/// that exist. Not the running app's marker or half-written files.
pub fn files(dir: &Path, name: &str) -> Vec<PathBuf> {
    std::iter::once(dir.join(name))
        .chain((1..=KEEP).map(|n| dir.join(format!("{name}.{n}"))))
        .chain([dir.join(CRASH_FILE), dir.join(PREVIOUS_CRASH_FILE)])
        .filter(|path| path.is_file())
        .collect()
}

/// Reads the end of a file: at most `max` bytes, from the start of a line.
pub fn read_tail(path: &Path, max: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0
        && let Some(newline) = text.find('\n')
    {
        text.drain(..=newline);
    }
    Ok(text)
}

/// A record's severity, as the log writes it: 1 for `ERROR` to 5 for `TRACE` (the
/// `log` crate's order). `None` for a continuation line or anything else.
pub fn severity_of(line: &str) -> Option<usize> {
    if line.starts_with(' ') {
        return None;
    }
    let mut words = line.split_whitespace();
    words.next()?;
    severity(words.next()?)
}

/// `error`, `warn`, `info`, `debug` or `trace` (any case) as 1 to 5.
pub fn severity(level: &str) -> Option<usize> {
    ["error", "warn", "info", "debug", "trace"]
        .iter()
        .position(|name| name.eq_ignore_ascii_case(level))
        .map(|n| n + 1)
}

/// The last `lines` records of `path` (and of `path.1` when `path` has fewer), of
/// severity `max_severity` or more severe when given; a record's continuation lines
/// count as lines of their own and follow its severity. Reads at most 1 MiB of each.
pub fn tail(path: &Path, lines: usize, max_severity: Option<usize>) -> io::Result<Vec<String>> {
    let mut text = read_tail(path, MAX_READ)?;
    let rotated = {
        let mut name = path.as_os_str().to_owned();
        name.push(".1");
        PathBuf::from(name)
    };
    if text.lines().count() < lines
        && let Ok(older) = read_tail(&rotated, MAX_READ)
    {
        text = older + &text;
    }
    let mut keep = true;
    let mut out: Vec<String> = text
        .lines()
        .filter(|line| {
            if let Some(severity) = severity_of(line) {
                keep = max_severity.is_none_or(|max| severity <= max);
            }
            keep
        })
        .map(str::to_string)
        .collect();
    let skip = out.len().saturating_sub(lines);
    out.drain(..skip);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tail_starts_at_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(read_tail(&path, 9).unwrap(), "three\n");
        assert_eq!(read_tail(&path, 100).unwrap(), "one\ntwo\nthree\n");
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
        let warnings = tail(&path, 10, severity("warn")).unwrap();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].ends_with("two") && warnings[1] == "  more of two");
        assert_eq!(severity_of("  more of two"), None);
        assert_eq!(severity("ERROR"), Some(1));
        assert_eq!(severity("verbose"), None);
    }

    #[test]
    fn only_log_files_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "launcher.log",
            "launcher.log.1",
            "launcher.log.rotating",
            "session.json",
            "last-crash.txt",
            "last-crash.txt.123.tmp",
            "previous-crash.txt",
        ] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        let names: Vec<String> = files(dir.path(), LOG_FILE)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "launcher.log",
                "launcher.log.1",
                "last-crash.txt",
                "previous-crash.txt"
            ]
        );
    }
}
