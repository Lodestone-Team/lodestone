//! Lodestone Core as a child process: where its program and data are, which port it
//! serves on, starting it and stopping it.
//!
//! The Host can't link the core: the core's dependencies would break the launcher
//! workspace's rule that every crate comes from Zed's lockfile. So it runs the core's
//! own program, `lodestone_core`, built from the repository's root workspace without
//! the `deno` feature and shipped next to the Host's executable.
//!
//! The core is started with `--is-host` (so `/info` says Lodestone Host started it),
//! `--lodestone-path` (its data folder), `--port` (the Host picks the port, so it knows
//! where to reach the core) and `--stop-with-stdin`. The Host holds the core's stdin:
//! closing it stops the core cleanly (the servers stop first), and if the Host dies the
//! system closes it, so the core never outlives the Host by more than its shutdown.
//! [`crate::supervisor`] restarts a core that exits on its own.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead as _, BufReader, Seek as _, SeekFrom},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// The core's program name.
pub const CORE_PROGRAM: &str = if cfg!(windows) {
    "lodestone_core.exe"
} else {
    "lodestone_core"
};

/// The port every core serves on unless told otherwise, which dashboards and
/// launchers look for.
pub const DEFAULT_PORT: u16 = 16662;

/// How long a stopping core may take (its servers stop first) before it is killed.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(20);

/// The core's log files are rotated once they pass this size.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Environment variables a developer (or a test) sets.
pub mod env {
    /// The core's data folder, as for the core itself.
    pub const DATA: &str = "LODESTONE_PATH";
    /// The core program to run.
    pub const CORE: &str = "LODESTONE_HOST_CORE";
    /// The port the core serves on.
    pub const PORT: &str = "LODESTONE_HOST_CORE_PORT";
    /// The Host's own folder (logs, the single-instance socket).
    pub const HOST_DATA: &str = "LODESTONE_HOST_DATA_DIR";
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.trim().is_empty())
}

/// The core's data folder: `LODESTONE_PATH`, else `~/.lodestone` (the Tauri Host's
/// too). Debug builds use `~/.lodestone-host-dev` instead, so a development build never
/// takes over the real core's data, or fails on its lock.
pub fn data_dir() -> PathBuf {
    if let Some(path) = var(env::DATA) {
        return PathBuf::from(path);
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    if cfg!(debug_assertions) {
        home.join(".lodestone-host-dev")
    } else {
        home.join(".lodestone")
    }
}

/// The Host's own folder, for its logs and the single-instance socket:
/// `LODESTONE_HOST_DATA_DIR`, else the system's app data folder.
pub fn host_dir() -> PathBuf {
    if let Some(path) = var(env::HOST_DATA) {
        return PathBuf::from(path);
    }
    let name = if cfg!(debug_assertions) {
        "Lodestone Host Dev"
    } else {
        "Lodestone Host"
    };
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(name)
}

/// Where the core's program is: `LODESTONE_HOST_CORE`, next to the Host's executable
/// (as bundled), or, in a debug build, the repository's `target/debug` or
/// `target/release` (`cargo build -p lodestone_core --no-default-features`).
pub fn find_core() -> Result<PathBuf, String> {
    if let Some(path) = var(env::CORE) {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{} is set to {}, which isn't a file.",
                env::CORE,
                path.display()
            ))
        };
    }
    let exe = std::env::current_exe().ok();
    let beside = exe
        .as_deref()
        .and_then(Path::parent)
        .map(|dir| dir.join(CORE_PROGRAM));
    if let Some(path) = beside.filter(|path| path.is_file()) {
        return Ok(path);
    }
    if cfg!(debug_assertions) {
        // launcher/crates/lodestone-host -> the repository's root workspace
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let built = ["debug", "release"]
            .iter()
            .map(|profile| repo.join("target").join(profile).join(CORE_PROGRAM))
            .filter(|path| path.is_file())
            .max_by_key(|path| fs::metadata(path).and_then(|m| m.modified()).ok());
        if let Some(path) = built {
            return Ok(path);
        }
    }
    Err(format!(
        "Lodestone Core ({CORE_PROGRAM}) wasn't found next to Lodestone Host. Reinstall Lodestone Host.{}",
        if cfg!(debug_assertions) {
            " In development, build it with `cargo build -p lodestone_core --no-default-features` in the repository's root, or set LODESTONE_HOST_CORE."
        } else {
            ""
        }
    ))
}

/// Whether nothing listens on `port`, on IPv4 or IPv6, as the core checks.
pub fn port_is_free(port: u16) -> bool {
    let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).is_ok();
    // a system without IPv6 still has a free port
    let v6 = match TcpListener::bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))) {
        Ok(_) => true,
        Err(err) => err.kind() != io::ErrorKind::AddrInUse,
    };
    v4 && v6
}

/// The port for the core: `LODESTONE_HOST_CORE_PORT`, else 16662. A release build
/// insists on 16662, which the dashboard and the launcher look for, and says so when it
/// is taken; a debug build takes the next free port after it.
pub fn pick_port(is_free: impl Fn(u16) -> bool) -> Result<u16, String> {
    if let Some(text) = var(env::PORT) {
        return text
            .trim()
            .parse()
            .map_err(|_| format!("{} isn't a port number: {text}", env::PORT));
    }
    choose_port(cfg!(debug_assertions), is_free)
}

fn choose_port(scan: bool, is_free: impl Fn(u16) -> bool) -> Result<u16, String> {
    if is_free(DEFAULT_PORT) {
        return Ok(DEFAULT_PORT);
    }
    if scan {
        if let Some(port) = (DEFAULT_PORT + 1..DEFAULT_PORT + 100).find(|&port| is_free(port)) {
            return Ok(port);
        }
    }
    Err(format!(
        "Port {DEFAULT_PORT} is in use. Is another Lodestone Core running on this computer? Stop it, then try again."
    ))
}

/// How to start the core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreConfig {
    pub program: PathBuf,
    pub data_dir: PathBuf,
    pub port: u16,
    /// Where the core's output goes.
    pub log_file: PathBuf,
}

impl CoreConfig {
    /// The core's arguments.
    pub fn args(&self) -> Vec<std::ffi::OsString> {
        vec![
            "--is-host".into(),
            "--lodestone-path".into(),
            self.data_dir.clone().into_os_string(),
            "--port".into(),
            self.port.to_string().into(),
            "--stop-with-stdin".into(),
        ]
    }

    /// The core's address on this computer, over plain HTTP (which the core allows
    /// from this computer only).
    pub fn local_url(&self) -> url::Url {
        url::Url::parse(&format!("http://127.0.0.1:{}", self.port)).expect("a valid URL")
    }
}

/// A running core.
pub struct CoreProcess {
    child: Child,
    /// Closing it stops the core.
    stdin: Option<ChildStdin>,
    pub started: Instant,
}

impl CoreProcess {
    /// Starts the core, its output appended to `config.log_file`.
    pub fn spawn(config: &CoreConfig) -> io::Result<Self> {
        fs::create_dir_all(&config.data_dir)?;
        let log = open_log(&config.log_file)?;
        let mut command = Command::new(&config.program);
        command
            .args(config.args())
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            // the folder is given on the command line; don't let ours confuse it
            .env_remove(env::DATA);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            // no console window for the core
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn()?;
        let stdin = child.stdin.take();
        Ok(Self {
            child,
            stdin,
            started: Instant::now(),
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// The exit status, once the core has exited.
    pub fn try_wait(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Asks the core to stop (closes its stdin): it stops its servers, then exits.
    pub fn request_stop(&mut self) {
        drop(self.stdin.take());
    }

    /// Stops the core: asks, waits up to `timeout`, then kills it.
    pub fn stop(&mut self, timeout: Duration) -> Option<ExitStatus> {
        self.request_stop();
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(status) = self.try_wait() {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = self.child.kill();
        self.child.wait().ok()
    }
}

impl Drop for CoreProcess {
    /// Asks the core to stop when the Host lets go of it. (The system closes the pipe
    /// when the Host exits, so the core stops even when this doesn't run.)
    fn drop(&mut self) {
        self.request_stop();
    }
}

/// The log file, rotated (to `<name>.1`) when it has grown too large.
fn open_log(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = fs::rename(path, path.with_extension("log.1"));
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// The last `lines` lines of the log, for the error page, without the terminal's
/// colour codes.
pub fn log_tail(path: &Path, lines: usize) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(len.saturating_sub(64 * 1024)));
    let mut tail: Vec<String> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .map(|line| strip_ansi(&line))
        .filter(|line| !line.trim().is_empty())
        .collect();
    let skip = tail.len().saturating_sub(lines);
    tail.drain(..skip);
    tail
}

/// `text` without ANSI escape sequences (`ESC [ … letter`).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_port_comes_first() {
        assert_eq!(choose_port(false, |_| true), Ok(DEFAULT_PORT));
        assert_eq!(choose_port(true, |_| true), Ok(DEFAULT_PORT));
    }

    #[test]
    fn a_release_build_insists_on_the_default_port() {
        let taken = |port| port != DEFAULT_PORT;
        assert!(choose_port(false, taken).unwrap_err().contains("16662"));
        assert_eq!(choose_port(true, taken), Ok(DEFAULT_PORT + 1));
        assert!(choose_port(true, |_| false).is_err());
    }

    #[test]
    fn the_core_is_started_as_the_hosts_child() {
        let config = CoreConfig {
            program: "core".into(),
            data_dir: "/data".into(),
            port: 17000,
            log_file: "/log".into(),
        };
        let args: Vec<String> = config
            .args()
            .into_iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--is-host",
                "--lodestone-path",
                "/data",
                "--port",
                "17000",
                "--stop-with-stdin"
            ]
        );
        assert_eq!(config.local_url().as_str(), "http://127.0.0.1:17000/");
    }

    #[test]
    fn colour_codes_are_stripped_from_the_log() {
        assert_eq!(
            strip_ansi("\u{1b}[2m2026\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m up"),
            "2026  INFO up"
        );
    }

    #[test]
    fn the_log_tail_keeps_the_last_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core.log");
        fs::write(&path, "one\n\ntwo\nthree\n").unwrap();
        assert_eq!(log_tail(&path, 2), ["two", "three"]);
        assert!(log_tail(&dir.path().join("missing"), 2).is_empty());
    }
}
