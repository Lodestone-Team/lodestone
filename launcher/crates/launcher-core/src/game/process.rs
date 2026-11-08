//! Running the game: the Java process, its output as [`LogLine`]s, and stopping it.

use std::{
    io::{self, BufRead as _, BufReader, Read},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use futures::channel::mpsc;

use super::{
    launch::LaunchCommand,
    log4j::{LogLine, LogParser, Redactor, Stream},
};

/// Keeps a console program, such as a `java.exe` picked as the Java override, from
/// opening a console window of its own on Windows. The launcher's release build has
/// no console for it to share, and its output is piped either way.
pub(crate) trait NoConsoleWindow {
    fn no_console_window(&mut self) -> &mut Self;
}

impl NoConsoleWindow for Command {
    fn no_console_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

/// What a running game reports.
#[derive(Debug)]
pub enum GameEvent {
    Line(LogLine),
    /// The process exited. Always the last event.
    Exited {
        /// The exit code, or `None` if a signal ended it.
        code: Option<i32>,
        /// Whether [`GameProcess::stop`] ended it.
        stopped: bool,
    },
}

/// A running game.
pub struct GameProcess {
    pid: u32,
    child: Arc<Mutex<Child>>,
    stopped: Arc<AtomicBool>,
}

impl GameProcess {
    /// Starts the command, with stdin closed and stdout and stderr captured. Every
    /// line passes through `redactor` before it is sent.
    pub fn spawn(
        command: &LaunchCommand,
        redactor: Redactor,
    ) -> io::Result<(Self, mpsc::UnboundedReceiver<GameEvent>)> {
        let mut child = command
            .to_command()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let pid = child.id();
        let (tx, rx) = mpsc::unbounded();
        let readers = [
            child
                .stdout
                .take()
                .map(|out| read_lines(out, Stream::Stdout, redactor.clone(), tx.clone())),
            child
                .stderr
                .take()
                .map(|err| read_lines(err, Stream::Stderr, redactor, tx.clone())),
        ];
        let child = Arc::new(Mutex::new(child));
        let stopped = Arc::new(AtomicBool::new(false));
        {
            let child = child.clone();
            let stopped = stopped.clone();
            std::thread::Builder::new()
                .name("game-waiter".into())
                .spawn(move || {
                    // The output ends when the game exits; send every line first.
                    for reader in readers.into_iter().flatten() {
                        let _ = reader.join();
                    }
                    let status = wait(&child);
                    let _ = tx.unbounded_send(GameEvent::Exited {
                        code: status.ok().and_then(|status| status.code()),
                        stopped: stopped.load(Ordering::SeqCst),
                    });
                })?;
        }
        Ok((
            Self {
                pid,
                child,
                stopped,
            },
            rx,
        ))
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Kills the game. Safe to call more than once and after it exited.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let mut child = self.child.lock().unwrap_or_else(|p| p.into_inner());
        // An error means it has already exited.
        let _ = child.kill();
    }
}

/// Waits for the child without holding the lock, so `stop` never blocks behind it.
fn wait(child: &Mutex<Child>) -> io::Result<ExitStatus> {
    loop {
        {
            let mut child = child.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
        }
        // Normally reached only when the game closed its output but kept running.
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_lines(
    pipe: impl Read + Send + 'static,
    stream: Stream,
    redactor: Redactor,
    tx: mpsc::UnboundedSender<GameEvent>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut parser = LogParser::new(stream);
        let mut reader = BufReader::new(pipe);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buffer);
                    let text = text.trim_end_matches(['\n', '\r']);
                    for line in parser.push(text) {
                        let _ = tx.unbounded_send(GameEvent::Line(redactor.redact_line(line)));
                    }
                }
            }
        }
        for line in parser.finish() {
            let _ = tx.unbounded_send(GameEvent::Line(redactor.redact_line(line)));
        }
    })
}
