//! Live backups: pausing the server's saves while its files are copied.
//!
//! `save-off`, then `save-all flush`, then wait for the server to say it saved, then copy,
//! then `save-on`, which is always sent (and awaited) however the copy ends. While saving is
//! off, `servers/<uuid>/save-off.marker` exists, so a core that died in the middle can say so
//! at the next start.
//!
//! Written against two small traits ([`LiveServer`], [`ConsoleLines`]) so it can be tested
//! without a server.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use color_eyre::eyre::Context;
use tokio::sync::broadcast;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::error::{Error, ErrorKind};
use crate::events::{Event, EventInner, InstanceEventInner};
use crate::implementations::minecraft::line_parser::parse_system_msg;
use crate::traits::t_server::State;
use crate::types::InstanceUuid;

use super::store::{cancelled, err, valid_server_id, Store};

/// How long to wait for "Saved the game" after `save-all flush`.
pub const SAVE_WAIT: Duration = Duration::from_secs(60);
/// The longest saving may stay off. Past this, saving is turned back on and the backup fails,
/// so a stuck backup can't lose the players' progress.
pub const SAVE_OFF_CAP: Duration = Duration::from_secs(15 * 60);

const MARKER_FILE: &str = "save-off.marker";

/// The running server a live backup talks to.
#[async_trait]
pub trait LiveServer: Send + Sync {
    /// Sends a console command.
    async fn send_command(&self, command: &str) -> Result<(), Error>;
    /// Whether the server is still running.
    async fn is_running(&self) -> bool;
}

/// What the server printed, as far as the save wait cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleLine {
    /// A console line. `system` is true for the server's own (non-chat) log lines.
    Line { system: bool, text: String },
    /// Some lines were missed; keep waiting.
    Lagged,
    /// The server left the Running state.
    Stopped,
    /// No more lines will come.
    Closed,
}

#[async_trait]
pub trait ConsoleLines: Send {
    async fn next_line(&mut self) -> ConsoleLine;
}

/// The console lines of one server, from the event broadcaster. Subscribe before sending
/// `save-all flush` so the save line can't be missed.
pub struct InstanceLines {
    events: broadcast::Receiver<Event>,
    uuid: InstanceUuid,
}

impl InstanceLines {
    pub fn new(events: broadcast::Receiver<Event>, uuid: InstanceUuid) -> InstanceLines {
        InstanceLines { events, uuid }
    }
}

#[async_trait]
impl ConsoleLines for InstanceLines {
    async fn next_line(&mut self) -> ConsoleLine {
        loop {
            let event = match self.events.recv().await {
                Ok(event) => event,
                Err(broadcast::error::RecvError::Lagged(_)) => return ConsoleLine::Lagged,
                Err(broadcast::error::RecvError::Closed) => return ConsoleLine::Closed,
            };
            let EventInner::InstanceEvent(event) = event.event_inner else {
                continue;
            };
            if event.instance_uuid != self.uuid {
                continue;
            }
            match event.instance_event_inner {
                InstanceEventInner::SystemMessage { message } => {
                    return ConsoleLine::Line {
                        system: true,
                        text: message,
                    }
                }
                InstanceEventInner::InstanceOutput { message } => {
                    return ConsoleLine::Line {
                        system: false,
                        text: message,
                    }
                }
                InstanceEventInner::StateTransition { to } if to != State::Running => {
                    return ConsoleLine::Stopped
                }
                _ => continue,
            }
        }
    }
}

/// Whether a server log line says the world was saved: "Saved the game" (1.13+) or "Saved
/// the world" (older), ignoring case, surrounding spaces and a final period. The log prefix
/// must be there and chat never counts, so a player can't fake it; Lodestone's own system
/// messages have no prefix either.
pub fn is_save_line(line: &str) -> bool {
    let Some(message) = parse_system_msg(line) else {
        return false;
    };
    let message = message.trim();
    let message = message.strip_suffix('.').unwrap_or(message).trim_end();
    message.eq_ignore_ascii_case("Saved the game")
        || message.eq_ignore_ascii_case("Saved the world")
}

/// How the wait for the save line ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveWait {
    Saved,
    TimedOut,
    NotRunning,
    Cancelled,
}

/// Waits up to `timeout` for the save line. Only the server's own lines count; missed lines
/// mean keep waiting. Stops when the server stops running.
pub async fn wait_for_save(
    server: &dyn LiveServer,
    lines: &mut dyn ConsoleLines,
    timeout: Duration,
    cancel: &CancellationToken,
) -> SaveWait {
    let deadline = Instant::now() + timeout;
    let mut check_running = tokio::time::interval(Duration::from_secs(1));
    check_running.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => return SaveWait::TimedOut,
            _ = cancel.cancelled() => return SaveWait::Cancelled,
            _ = check_running.tick() => {
                if !server.is_running().await {
                    return SaveWait::NotRunning;
                }
            }
            line = lines.next_line() => match line {
                ConsoleLine::Line { system: true, text } if is_save_line(&text) => return SaveWait::Saved,
                ConsoleLine::Line { .. } | ConsoleLine::Lagged => {}
                ConsoleLine::Stopped => return SaveWait::NotRunning,
                // nothing more will come; don't spin
                ConsoleLine::Closed => return SaveWait::TimedOut,
            },
        }
    }
}

/// Watches the server while `work` runs: returns the work's result and whether the server
/// stopped running meanwhile (a stop or restart saves and rewrites the world while it is
/// copied, so the copy isn't consistent). The server is asked every `check_every`, besides
/// noticing its state change in `lines`.
pub async fn watch_while<T>(
    server: &dyn LiveServer,
    lines: &mut dyn ConsoleLines,
    check_every: Duration,
    work: impl Future<Output = Result<T, Error>>,
) -> Result<(T, bool), Error> {
    tokio::pin!(work);
    let mut stopped = false;
    let mut lines_open = true;
    let mut check_running = tokio::time::interval(check_every);
    check_running.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            result = &mut work => return result.map(|value| (value, stopped)),
            _ = check_running.tick(), if !stopped => {
                if !server.is_running().await {
                    stopped = true;
                }
            }
            line = lines.next_line(), if !stopped && lines_open => match line {
                ConsoleLine::Stopped => stopped = true,
                ConsoleLine::Closed => lines_open = false,
                ConsoleLine::Line { .. } | ConsoleLine::Lagged => {}
            },
        }
    }
}

/// The time limits of a live backup; [`Limits::default`] in production.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub save_wait: Duration,
    pub save_off_cap: Duration,
    /// How often the copy checks that the server still runs.
    pub check_running: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            save_wait: SAVE_WAIT,
            save_off_cap: SAVE_OFF_CAP,
            check_running: Duration::from_secs(1),
        }
    }
}

/// Runs `work` with the server's saving off.
///
/// `work` gets whether the server confirmed it saved (false when the wait timed out: the copy
/// may be inconsistent) and a token that is cancelled when `cancel` is, or when saving has been
/// off for `limits.save_off_cap`. Blocking work must check that token, since the future is
/// dropped at the cap while a blocking thread keeps running until it looks.
///
/// `save-on` is sent and awaited on every way out, then the marker is deleted. Returns the
/// work's result and whether the copy is consistent: the server confirmed it saved, and
/// didn't stop (or restart) while `work` ran. That replaces the work's own idea of it, from
/// its argument.
pub async fn with_saving_off<T, F, Fut>(
    server: &dyn LiveServer,
    lines: &mut dyn ConsoleLines,
    marker: &Path,
    limits: Limits,
    cancel: &CancellationToken,
    work: F,
) -> Result<(T, bool), Error>
where
    F: FnOnce(bool, CancellationToken) -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    write_marker(marker).await?;
    let work_cancel = cancel.child_token();
    let paused = async {
        server.send_command("save-off").await?;
        server.send_command("save-all flush").await?;
        let quiesced = match wait_for_save(server, lines, limits.save_wait, cancel).await {
            SaveWait::Saved => true,
            SaveWait::TimedOut => false,
            SaveWait::NotRunning => {
                return Err(err(
                    ErrorKind::Conflict,
                    "The server stopped while it was being backed up",
                ))
            }
            SaveWait::Cancelled => return Err(cancelled()),
        };
        let work = work(quiesced, work_cancel.clone());
        let (value, stopped) = watch_while(server, lines, limits.check_running, work).await?;
        if stopped {
            warn!(
                "The server stopped while it was being backed up; the backup may be inconsistent"
            );
        }
        Ok((value, quiesced && !stopped))
    };
    let result = match tokio::time::timeout(limits.save_off_cap, paused).await {
        Ok(result) => result,
        Err(_) => {
            work_cancel.cancel();
            Err(err(
                ErrorKind::Internal,
                format!(
                    "The backup was stopped because saving was paused for more than {} minutes",
                    limits.save_off_cap.as_secs() / 60
                ),
            ))
        }
    };
    if let Err(e) = server.send_command("save-on").await {
        warn!("Failed to turn saving back on after a backup: {}", e.source);
    }
    if let Err(e) = tokio::fs::remove_file(marker).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("Failed to delete {}: {e}", marker.display());
        }
    }
    result
}

async fn write_marker(marker: &Path) -> Result<(), Error> {
    if let Some(parent) = marker.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create the server's backup folder")?;
    }
    let now = chrono::Utc::now().timestamp().to_string();
    tokio::fs::write(marker, now)
        .await
        .context("Failed to write the save-off marker")?;
    Ok(())
}

/// At startup: deletes the save-off markers a crashed core left, with a warning, and returns
/// whose they were.
pub fn remove_leftover_markers(store: &Store) -> Vec<InstanceUuid> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(store.servers_dir()) else {
        return found;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        if !valid_server_id(&name) {
            continue;
        }
        let marker = entry.path().join(MARKER_FILE);
        if !marker.is_file() {
            continue;
        }
        warn!(
            "A backup of {name} was interrupted while the server's saving was off. If that server is still running outside Lodestone, run save-on in its console"
        );
        if let Err(e) = std::fs::remove_file(&marker) {
            warn!("Failed to delete {}: {e}", marker.display());
        }
        found.push(InstanceUuid::from(name));
    }
    found
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use super::*;
    use crate::events::{CausedBy, InstanceEvent};
    use crate::types::Snowflake;

    const SAVED: &str = "[12:34:56] [Server thread/INFO]: Saved the game";

    struct FakeServer {
        commands: Mutex<Vec<String>>,
        running: AtomicBool,
        fail: Option<&'static str>,
    }

    impl FakeServer {
        fn new() -> FakeServer {
            FakeServer {
                commands: Mutex::new(Vec::new()),
                running: AtomicBool::new(true),
                fail: None,
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl LiveServer for FakeServer {
        async fn send_command(&self, command: &str) -> Result<(), Error> {
            self.commands.lock().unwrap().push(command.to_string());
            if self.fail == Some(command) {
                return Err(err(ErrorKind::Internal, "command failed"));
            }
            Ok(())
        }

        async fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }
    }

    /// Gives the lines it was made with, then nothing (the server is quiet).
    struct FakeLines(std::collections::VecDeque<ConsoleLine>);

    impl FakeLines {
        fn new(lines: Vec<ConsoleLine>) -> FakeLines {
            FakeLines(lines.into())
        }
    }

    #[async_trait]
    impl ConsoleLines for FakeLines {
        async fn next_line(&mut self) -> ConsoleLine {
            match self.0.pop_front() {
                Some(line) => line,
                None => std::future::pending().await,
            }
        }
    }

    fn system(text: &str) -> ConsoleLine {
        ConsoleLine::Line {
            system: true,
            text: text.to_string(),
        }
    }

    fn short() -> Limits {
        Limits {
            save_wait: Duration::from_millis(200),
            save_off_cap: Duration::from_secs(10),
            check_running: Duration::from_millis(20),
        }
    }

    async fn run(
        server: &FakeServer,
        lines: Vec<ConsoleLine>,
        marker: &Path,
        limits: Limits,
    ) -> Result<(u32, bool), Error> {
        let mut lines = FakeLines::new(lines);
        with_saving_off(
            server,
            &mut lines,
            marker,
            limits,
            &CancellationToken::new(),
            |_, _| async {
                assert!(marker.exists(), "the marker exists while saving is off");
                Ok(7)
            },
        )
        .await
    }

    #[test]
    fn save_lines() {
        for line in [
            SAVED,
            "[12:34:56] [Server thread/INFO]: Saved the world",
            "[12:34:56 INFO]: Saved the world.",
            "[12:34:56] [Server thread/INFO]: saved the game ",
        ] {
            assert!(is_save_line(line), "{line:?}");
        }
        for line in [
            "[12:34:56] [Server thread/INFO]: <Steve> Saved the game",
            "[12:34:56] [Server thread/INFO]: [Not Secure] <Steve> Saved the game",
            "Saved the game",
            "[12:34:56] [Server thread/INFO]: Saving the game (this may take a moment!)",
            "[12:34:56] [Server thread/INFO]: Saved the games",
        ] {
            assert!(!is_save_line(line), "{line:?}");
        }
    }

    #[tokio::test]
    async fn save_line_means_quiesced() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("servers/x/save-off.marker");
        let server = FakeServer::new();
        let lines = vec![
            system("[12:34:56] [Server thread/INFO]: Automatic saving is now disabled"),
            system("[12:34:56] [Server thread/INFO]: Saving the game (this may take a moment!)"),
            system(SAVED),
        ];
        let (value, quiesced) = run(&server, lines, &marker, Limits::default())
            .await
            .unwrap();
        assert_eq!(value, 7);
        assert!(quiesced);
        assert_eq!(
            server.commands(),
            vec!["save-off", "save-all flush", "save-on"]
        );
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn chat_and_output_lines_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let lines = vec![
            // a player typing it
            system("[12:34:56] [Server thread/INFO]: <Steve> Saved the game"),
            // the raw output line of a real save, not a system message
            ConsoleLine::Line {
                system: false,
                text: SAVED.to_string(),
            },
            ConsoleLine::Lagged,
        ];
        let (_, quiesced) = run(&server, lines, &marker, short()).await.unwrap();
        assert!(!quiesced);
        assert_eq!(server.commands().last().unwrap(), "save-on");
    }

    #[tokio::test]
    async fn timeout_is_not_quiesced_and_lagged_keeps_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let (_, quiesced) = run(&server, vec![], &marker, short()).await.unwrap();
        assert!(!quiesced);
        let (_, quiesced) = run(
            &server,
            vec![ConsoleLine::Lagged, system(SAVED)],
            &marker,
            short(),
        )
        .await
        .unwrap();
        assert!(quiesced);
        let (_, quiesced) = run(
            &server,
            vec![system("[01:02:03] [Server thread/INFO]: Saved the world")],
            &marker,
            short(),
        )
        .await
        .unwrap();
        assert!(quiesced);
    }

    #[tokio::test]
    async fn save_on_is_sent_when_the_work_fails() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let mut lines = FakeLines::new(vec![system(SAVED)]);
        let result: Result<((), bool), Error> = with_saving_off(
            &server,
            &mut lines,
            &marker,
            Limits::default(),
            &CancellationToken::new(),
            |_, _| async { Err(err(ErrorKind::Internal, "disk full")) },
        )
        .await;
        assert_eq!(result.unwrap_err().source.to_string(), "disk full");
        assert_eq!(
            server.commands(),
            vec!["save-off", "save-all flush", "save-on"]
        );
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn save_on_is_sent_when_a_command_fails() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let mut server = FakeServer::new();
        server.fail = Some("save-all flush");
        let ran = AtomicBool::new(false);
        let mut lines = FakeLines::new(vec![]);
        let result = with_saving_off(
            &server,
            &mut lines,
            &marker,
            short(),
            &CancellationToken::new(),
            |_, _| async {
                ran.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .await;
        assert!(result.is_err());
        assert!(!ran.load(Ordering::SeqCst));
        assert_eq!(
            server.commands(),
            vec!["save-off", "save-all flush", "save-on"]
        );
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn a_stopping_server_fails_the_backup() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let result = run(
            &server,
            vec![ConsoleLine::Stopped],
            &marker,
            Limits::default(),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(server.commands().last().unwrap(), "save-on");
        let server = FakeServer::new();
        server.running.store(false, Ordering::SeqCst);
        let result = run(&server, vec![], &marker, Limits::default()).await;
        assert_eq!(
            result.unwrap_err().source.to_string(),
            "The server stopped while it was being backed up"
        );
        assert_eq!(server.commands().last().unwrap(), "save-on");
    }

    #[tokio::test]
    async fn a_server_stopping_during_the_copy_makes_it_inconsistent() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");

        // the state change is seen in the console lines
        let server = FakeServer::new();
        let mut lines = FakeLines::new(vec![system(SAVED), ConsoleLine::Stopped]);
        let (value, quiesced) = with_saving_off(
            &server,
            &mut lines,
            &marker,
            short(),
            &CancellationToken::new(),
            |quiesced, _| async move {
                assert!(quiesced, "the save was confirmed before the copy");
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(7)
            },
        )
        .await
        .unwrap();
        assert_eq!(value, 7);
        assert!(!quiesced);
        assert_eq!(server.commands().last().unwrap(), "save-on");
        assert!(!marker.exists());

        // the server is found not running, with the state change missed, and is running
        // again (restarted) by the end of the copy
        let server = FakeServer::new();
        let mut lines = FakeLines::new(vec![system(SAVED), ConsoleLine::Lagged]);
        let (_, quiesced) = with_saving_off(
            &server,
            &mut lines,
            &marker,
            short(),
            &CancellationToken::new(),
            |_, _| async {
                server.running.store(false, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await;
                server.running.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap();
        assert!(!quiesced);

        // a server that keeps running stays consistent, even when its lines end
        let server = FakeServer::new();
        let mut lines = FakeLines::new(vec![system(SAVED), ConsoleLine::Closed]);
        let (_, quiesced) = with_saving_off(
            &server,
            &mut lines,
            &marker,
            short(),
            &CancellationToken::new(),
            |_, _| async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(())
            },
        )
        .await
        .unwrap();
        assert!(quiesced);
    }

    #[tokio::test]
    async fn the_cap_stops_the_work_and_turns_saving_on() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let mut lines = FakeLines::new(vec![system(SAVED)]);
        let limits = Limits {
            save_wait: Duration::from_secs(60),
            save_off_cap: Duration::from_millis(200),
            ..Limits::default()
        };
        let token_seen = std::sync::Arc::new(Mutex::new(None));
        let seen = token_seen.clone();
        let result: Result<((), bool), Error> = with_saving_off(
            &server,
            &mut lines,
            &marker,
            limits,
            &CancellationToken::new(),
            |_, token| async move {
                *seen.lock().unwrap() = Some(token.clone());
                std::future::pending::<()>().await;
                Ok(())
            },
        )
        .await;
        assert!(result.is_err());
        assert!(token_seen.lock().unwrap().as_ref().unwrap().is_cancelled());
        assert_eq!(server.commands().last().unwrap(), "save-on");
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn cancellation_stops_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("save-off.marker");
        let server = FakeServer::new();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let mut lines = FakeLines::new(vec![]);
        let result = with_saving_off(
            &server,
            &mut lines,
            &marker,
            Limits::default(),
            &cancel,
            |_, _| async { Ok(()) },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(server.commands().last().unwrap(), "save-on");
    }

    fn instance_event(uuid: &str, inner: InstanceEventInner) -> Event {
        Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: InstanceUuid::from(uuid.to_string()),
                instance_name: "Test".to_string(),
                instance_event_inner: inner,
            }),
            details: String::new(),
            snowflake: Snowflake::default(),
            caused_by: CausedBy::System,
        }
    }

    #[tokio::test]
    async fn instance_lines_filter_by_server() {
        let (tx, rx) = broadcast::channel(16);
        let mut lines = InstanceLines::new(rx, InstanceUuid::from("INSTANCE_a".to_string()));
        tx.send(instance_event(
            "INSTANCE_b",
            InstanceEventInner::SystemMessage {
                message: SAVED.to_string(),
            },
        ))
        .unwrap();
        tx.send(instance_event(
            "INSTANCE_a",
            InstanceEventInner::InstanceOutput {
                message: "out".to_string(),
            },
        ))
        .unwrap();
        tx.send(instance_event(
            "INSTANCE_a",
            InstanceEventInner::SystemMessage {
                message: SAVED.to_string(),
            },
        ))
        .unwrap();
        tx.send(instance_event(
            "INSTANCE_a",
            InstanceEventInner::StateTransition { to: State::Running },
        ))
        .unwrap();
        tx.send(instance_event(
            "INSTANCE_a",
            InstanceEventInner::StateTransition {
                to: State::Stopping,
            },
        ))
        .unwrap();
        assert_eq!(
            lines.next_line().await,
            ConsoleLine::Line {
                system: false,
                text: "out".to_string()
            }
        );
        assert_eq!(lines.next_line().await, system(SAVED));
        assert_eq!(lines.next_line().await, ConsoleLine::Stopped);
        drop(tx);
        assert_eq!(lines.next_line().await, ConsoleLine::Closed);

        // a full channel reports lagging
        let (tx, rx) = broadcast::channel(1);
        let mut lines = InstanceLines::new(rx, InstanceUuid::from("INSTANCE_a".to_string()));
        for _ in 0..3 {
            tx.send(instance_event(
                "INSTANCE_a",
                InstanceEventInner::InstanceOutput {
                    message: "x".to_string(),
                },
            ))
            .unwrap();
        }
        assert_eq!(lines.next_line().await, ConsoleLine::Lagged);
    }

    #[test]
    fn leftover_markers_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let uuid = InstanceUuid::from("INSTANCE_a".to_string());
        let marker = store.marker_path(&uuid).unwrap();
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, "1").unwrap();
        assert_eq!(remove_leftover_markers(&store), vec![uuid]);
        assert!(!marker.exists());
        assert!(remove_leftover_markers(&store).is_empty());
    }
}
