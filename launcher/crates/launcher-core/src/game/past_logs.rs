//! The logs and crash reports a game left in its folder, for the Logs tab to show
//! after the game has quit: `logs/latest.log` and the older `logs/*.log.gz`, the
//! `crash-reports`, and the JVM's own `hs_err_pid*.log` files.
//!
//! [`list`] finds them, newest first. [`read`] turns one into lines with the time,
//! level and thread the game wrote, as the live console has them.

use std::{
    borrow::Cow,
    collections::VecDeque,
    io::{BufRead, BufReader, Read as _},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::log4j::Level;

/// The most lines of a file that are shown: its last ones.
pub const MAX_LINES: usize = 50_000;

/// The most text of a file's last lines that is kept, once unpacked.
const MAX_BYTES: u64 = 256 * 1024 * 1024;

/// The longest line read in one piece. A longer one is split, so one huge line can't
/// fill the memory.
const MAX_LINE_BYTES: u64 = 1024 * 1024;

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

/// The access tokens a line may hold, replaced by `<redacted>` as the live console
/// and the launch command it shows have them: the session ID that old versions log
/// (`(Session ID is token:<access token>:<uuid>)`, at INFO in 1.7 and 1.8), and an
/// `--accessToken` argument, which a JVM crash file has in its command line. Older
/// files hold older tokens, which no signed-in account's would match, so these are
/// found by their shape.
pub fn redact(line: &str) -> Cow<'_, str> {
    const REDACTED: &str = "<redacted>";
    if !line.contains("token:") && !line.contains("--accessToken") {
        return Cow::Borrowed(line);
    }
    let mut out = String::new();
    let mut done = 0;
    let mut at = 0;
    while at < line.len() {
        let rest = &line[at..];
        // `token:<value>:`, the value as long as `Redactor` wants a secret to be
        if let Some(after) = rest.strip_prefix("token:")
            && let Some(len) = after.find(|c: char| c == ':' || c.is_whitespace())
            && len >= 8
            && after[len..].starts_with(':')
        {
            let value = at + "token:".len();
            out.push_str(&line[done..value]);
            out.push_str(REDACTED);
            done = value + len;
            at = done;
            continue;
        }
        // `--accessToken <value>` or `--accessToken=<value>`
        if let Some(after) = rest.strip_prefix("--accessToken")
            && let Some(value) = after.strip_prefix('=').or_else(|| {
                let value = after.trim_start();
                (value.len() < after.len()).then_some(value)
            })
            && !value.starts_with("--")
        {
            let len = value.find(char::is_whitespace).unwrap_or(value.len());
            if len > 0 {
                let start = line.len() - value.len();
                out.push_str(&line[done..start]);
                out.push_str(REDACTED);
                done = start + len;
                at = done;
                continue;
            }
        }
        at += rest.chars().next().map_or(1, char::len_utf8);
    }
    if done == 0 {
        Cow::Borrowed(line)
    } else {
        out.push_str(&line[done..]);
        Cow::Owned(out)
    }
}

/// Turns a file's lines into [`Line`]s, one at a time, keeping the last
/// [`MAX_LINES`]. In a log, a line that doesn't start an event (a stack trace, the
/// rest of a message) keeps the level of the event before it. A crash report has no
/// events: its lines are shown as they are. Every line is [`redact`]ed.
struct Parser {
    kind: Kind,
    level: Level,
    lines: VecDeque<Line>,
    /// The bytes of `lines`' texts, kept under [`MAX_BYTES`].
    bytes: usize,
    skipped: usize,
}

impl Parser {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            level: Level::Info,
            lines: VecDeque::new(),
            bytes: 0,
            skipped: 0,
        }
    }

    fn push(&mut self, line: &str) {
        let line = redact(line.trim_end_matches('\r'));
        let line = match self.kind {
            Kind::Log => event(&line),
            _ => None,
        }
        .inspect(|event| self.level = event.level)
        .unwrap_or_else(|| Line {
            time: None,
            level: self.level,
            source: None,
            text: line.into_owned(),
        });
        self.bytes += line.text.len();
        self.lines.push_back(line);
        while self.lines.len() > MAX_LINES || self.bytes as u64 > MAX_BYTES {
            let Some(dropped) = self.lines.pop_front() else {
                break;
            };
            self.bytes -= dropped.text.len();
            self.skipped += 1;
        }
    }

    fn finish(self) -> Contents {
        Contents {
            lines: self.lines.into(),
            skipped: self.skipped,
        }
    }
}

/// Splits a file's text into lines (see [`Parser`]).
pub fn parse(text: &str, kind: Kind) -> Contents {
    let mut parser = Parser::new(kind);
    for line in text.lines() {
        parser.push(line);
    }
    parser.finish()
}

/// Reads a file to its end, unpacking a `.gz`, and keeps its last lines: the end is
/// where a session stopped or crashed, however long the log is. Bytes that aren't
/// UTF-8 are replaced, not refused: old logs are in the system's encoding.
pub fn read(file: &LogFile) -> std::io::Result<Contents> {
    let raw = std::fs::File::open(&file.path)?;
    if file.name.to_lowercase().ends_with(".gz") {
        read_lines(
            BufReader::new(flate2::read::MultiGzDecoder::new(raw)),
            file.kind,
        )
    } else {
        read_lines(BufReader::new(raw), file.kind)
    }
}

/// The lines of `reader`. A line longer than [`MAX_LINE_BYTES`] is split.
fn read_lines(mut reader: impl BufRead, kind: Kind) -> std::io::Result<Contents> {
    let mut parser = Parser::new(kind);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        if (&mut reader)
            .take(MAX_LINE_BYTES)
            .read_until(b'\n', &mut bytes)?
            == 0
        {
            break;
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        parser.push(&String::from_utf8_lossy(&bytes));
    }
    Ok(parser.finish())
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
    fn access_tokens_are_redacted() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJ4dWlkIjoiMjUzNSJ9.c2lnbmF0dXJl";
        let text = format!(
            "[12:00:00] [Client thread/INFO]: Setting user: Steve\n\
[12:00:00] [Client thread/INFO]: (Session ID is token:{jwt}:0123456789abcdef0123456789abcdef)\n\
Unexpected token: here, a token:short: one\n"
        );
        assert_eq!(
            parse(&text, Kind::Log).lines,
            vec![
                line(
                    "12:00:00",
                    Level::Info,
                    "Client thread",
                    "Setting user: Steve"
                ),
                line(
                    "12:00:00",
                    Level::Info,
                    "Client thread",
                    "(Session ID is token:<redacted>:0123456789abcdef0123456789abcdef)"
                ),
                // not tokens
                rest(Level::Info, "Unexpected token: here, a token:short: one"),
            ]
        );
        // a JVM crash file has the command line
        let text = format!(
            "java_command: net.minecraft.client.main.Main --username Steve --accessToken {jwt} --version 1.8.9\n\
--accessToken={jwt}\n\
--accessToken --userType msa\n"
        );
        let shown: Vec<String> = parse(&text, Kind::JvmCrash)
            .lines
            .into_iter()
            .map(|line| line.text)
            .collect();
        assert_eq!(
            shown,
            vec![
                "java_command: net.minecraft.client.main.Main --username Steve --accessToken <redacted> --version 1.8.9",
                "--accessToken=<redacted>",
                "--accessToken --userType msa",
            ]
        );
    }

    /// Lines of `width` bytes, `count` of them, then `the end`, without holding them
    /// all.
    struct LongLog {
        line: Vec<u8>,
        count: usize,
        at: usize,
        end: &'static [u8],
    }

    impl std::io::Read for LongLog {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.count == 0 {
                let n = self.end.len().min(buf.len());
                buf[..n].copy_from_slice(&self.end[..n]);
                self.end = &self.end[n..];
                return Ok(n);
            }
            let n = (self.line.len() - self.at).min(buf.len());
            buf[..n].copy_from_slice(&self.line[self.at..self.at + n]);
            self.at += n;
            if self.at == self.line.len() {
                self.at = 0;
                self.count -= 1;
            }
            Ok(n)
        }
    }

    #[test]
    fn the_end_of_a_log_bigger_than_what_is_kept_is_read() {
        let mut line = vec![b'x'; 1023];
        line.push(b'\n');
        let count = (MAX_BYTES as usize) / line.len() + 1000;
        let log = LongLog {
            line,
            count,
            at: 0,
            end: b"[12:00:00] [main/INFO]: Stopping!\nthe end",
        };
        let contents = read_lines(std::io::BufReader::new(log), Kind::Log).unwrap();
        assert_eq!(contents.lines.len(), MAX_LINES);
        assert_eq!(contents.skipped, count + 2 - MAX_LINES);
        let last: Vec<&str> = contents.lines[MAX_LINES - 2..]
            .iter()
            .map(|line| line.text.as_str())
            .collect();
        assert_eq!(last, ["Stopping!", "the end"]);
    }

    #[test]
    fn a_huge_line_is_split() {
        let mut text = vec![b'y'; MAX_LINE_BYTES as usize + 10];
        text.extend_from_slice(b"\nnext\n");
        let contents = read_lines(&text[..], Kind::CrashReport).unwrap();
        let lengths: Vec<usize> = contents.lines.iter().map(|l| l.text.len()).collect();
        assert_eq!(lengths, [MAX_LINE_BYTES as usize, 10, 4]);
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
