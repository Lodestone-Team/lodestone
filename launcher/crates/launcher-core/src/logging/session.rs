//! Noticing, at start, that the last run ended in a crash.
//!
//! `logs/session.json` is written when the app starts and removed when it quits
//! cleanly ([`super::end_session`]). The panic hook writes `logs/last-crash.txt`, whose
//! first line names the run (`run <id> <time> <app> <version>`). So at the next start,
//! a marker still there whose run the crash file names means the app crashed and the
//! person may not have seen why: [`begin`] returns the crash, once, and the new run's
//! marker replaces the old one.
//!
//! A panic the app survived (a background thread's, say) followed by a clean quit
//! shows nothing, and neither does a forced quit or a power cut without a panic. A
//! forced quit after a panic the app survived is reported as a crash: the crash file
//! names the run, and its marker is still there.
//!
//! Two processes on one logs folder (a dev build beside the installed app) each
//! remove only their own marker.

use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The marker of a running app.
pub const SESSION_FILE: &str = "session.json";
/// The last crash's report.
pub const CRASH_FILE: &str = launcher_control::logs::CRASH_FILE;
/// The report before it: a run's first panic moves [`CRASH_FILE`] here.
pub const PREVIOUS_CRASH_FILE: &str = launcher_control::logs::PREVIOUS_CRASH_FILE;
/// How much of the crash file is read back.
const MAX_CRASH_READ: u64 = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    run_id: String,
    pid: u32,
    started: String,
    version: String,
}

/// The crash the last run ended with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crash {
    /// When, as the report says (RFC 3339).
    pub at: String,
    /// The panic message's first line.
    pub summary: String,
    /// The report.
    pub file: PathBuf,
}

/// Starts run `run_id`: returns the crash the last run ended with, if it ended in
/// one, and marks this run as running.
pub fn begin(dir: &Path, run_id: &str, version: &str) -> Option<Crash> {
    let previous = fs::read(dir.join(SESSION_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok());
    let crash = previous.and_then(|previous| crash_of(dir, &previous.run_id));
    let marker = Marker {
        run_id: run_id.to_string(),
        pid: std::process::id(),
        started: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        version: version.to_string(),
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&marker) {
        let _ = write_atomically(&dir.join(SESSION_FILE), &bytes);
    }
    crash
}

/// Ends run `run_id`: a clean quit. The marker is removed only when it is this run's,
/// so another process's stays.
pub fn end(dir: &Path, run_id: &str) {
    let path = dir.join(SESSION_FILE);
    let ours = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok())
        .is_some_and(|marker| marker.run_id == run_id);
    if ours {
        let _ = fs::remove_file(path);
    }
}

/// The crash file's first line for a run.
pub fn crash_header(run_id: &str, at: &str, app: &str, version: &str) -> String {
    format!("run {run_id} {at} {app} {version}")
}

/// The crash `dir`'s crash file describes, if it is run `run_id`'s.
fn crash_of(dir: &Path, run_id: &str) -> Option<Crash> {
    let file = dir.join(CRASH_FILE);
    let mut text = String::new();
    fs::File::open(&file)
        .ok()?
        .take(MAX_CRASH_READ)
        .read_to_string(&mut text)
        .ok()?;
    let mut lines = text.lines();
    let mut header = lines.next()?.split(' ');
    if header.next()? != "run" || header.next()? != run_id {
        return None;
    }
    let at = header.next().unwrap_or_default().to_string();
    // the report: "thread 'main' panicked at src/x.rs:1:2:", then the message
    let summary = lines
        .skip_while(|line| !line.contains(" panicked at "))
        .nth(1)
        .unwrap_or_default()
        .chars()
        .take(200)
        .collect();
    Some(Crash { at, summary, file })
}

/// Writes `bytes` to a temporary file beside `path`, then renames it into place. On
/// Unix the file is the user's alone, like the logs.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(dir) = path.parent() {
        super::file::private_dir(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(".{}.tmp", std::process::id()));
    let temp = PathBuf::from(temp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&temp)?.write_all(bytes)?;
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crash(dir: &Path, run_id: &str) {
        let text = format!(
            "{}\nthread 'main' panicked at src/main.rs:3:5:\nsomething broke\nsecond line\n",
            crash_header(
                run_id,
                "2026-11-16T08:30:12Z",
                "Lodestone Launcher",
                "1.0.0"
            )
        );
        fs::write(dir.join(CRASH_FILE), text).unwrap();
    }

    #[test]
    fn a_crash_is_noticed_once() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(begin(dir.path(), "a", "1.0.0"), None, "a first start");
        crash(dir.path(), "a");
        let noticed = begin(dir.path(), "b", "1.0.0").expect("the crash");
        assert_eq!(noticed.summary, "something broke");
        assert_eq!(noticed.at, "2026-11-16T08:30:12Z");
        assert_eq!(noticed.file, dir.path().join(CRASH_FILE));
        // run b didn't crash: its marker is still there, but the crash file is a's
        assert_eq!(begin(dir.path(), "c", "1.0.0"), None);
    }

    #[test]
    fn a_clean_quit_shows_nothing() {
        let dir = tempfile::tempdir().unwrap();
        begin(dir.path(), "a", "1.0.0");
        // a panic the app survived, then a clean quit
        crash(dir.path(), "a");
        end(dir.path(), "a");
        assert!(!dir.path().join(SESSION_FILE).exists());
        assert_eq!(begin(dir.path(), "b", "1.0.0"), None);
    }

    #[test]
    fn a_quit_leaves_another_runs_marker() {
        let dir = tempfile::tempdir().unwrap();
        begin(dir.path(), "a", "1.0.0");
        // run b started beside a (or after a restart that a was slow to finish)
        begin(dir.path(), "b", "1.0.0");
        end(dir.path(), "a");
        assert!(dir.path().join(SESSION_FILE).exists(), "b's marker stays");
        crash(dir.path(), "b");
        assert!(
            begin(dir.path(), "c", "1.0.0").is_some(),
            "b's crash is noticed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn markers_are_the_users_alone() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        begin(&logs, "a", "1.0.0");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&logs.join(SESSION_FILE)), 0o600);
        assert_eq!(mode(&logs), 0o700);
    }

    #[test]
    fn a_marker_without_a_crash_shows_nothing() {
        let dir = tempfile::tempdir().unwrap();
        begin(dir.path(), "a", "1.0.0");
        // a force quit: the marker stays, but nothing panicked
        assert_eq!(begin(dir.path(), "b", "1.0.0"), None);
        fs::write(dir.path().join(SESSION_FILE), "not json").unwrap();
        assert_eq!(begin(dir.path(), "c", "1.0.0"), None);
    }
}
