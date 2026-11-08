//! The CLI's side of the socket: a blocking client.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::Value;

use crate::protocol::{ErrorObject, MAX_LINE_BYTES, Message, Request, codes};

/// Why the client couldn't talk to the launcher.
#[derive(Debug)]
pub enum ClientError {
    /// Nothing answers on the socket: the launcher isn't running, or uses another data
    /// directory.
    NotRunning { path: PathBuf, reason: String },
    /// The connection broke, or the launcher sent something unreadable.
    Io(String),
    /// No message arrived before the deadline.
    Timeout,
    /// The socket belongs to another user: nothing is sent to it.
    NotYours { path: PathBuf, uid: u32 },
}

impl ClientError {
    /// As an error object, for the CLI's output.
    pub fn to_error(&self) -> ErrorObject {
        match self {
            ClientError::NotRunning { path, reason } => ErrorObject::new(
                codes::NOT_RUNNING,
                format!(
                    "Lodestone Launcher isn't running, or it uses another data directory: nothing answers on {} ({reason}). Open Lodestone Launcher, or pass --socket (or set LODESTONE_LAUNCHER_DATA_DIR as for the launcher).",
                    path.display()
                ),
            )
            .with_data(serde_json::json!({ "socket": path })),
            ClientError::Io(message) => ErrorObject::new(
                codes::FAILED,
                format!("The connection to Lodestone Launcher broke: {message}"),
            ),
            ClientError::NotYours { path, uid } => ErrorObject::new(
                codes::PERMISSION_DENIED,
                format!(
                    "The socket at {} is served by another user (uid {uid}), not by your Lodestone Launcher. Nothing was sent to it.",
                    path.display()
                ),
            )
            .with_data(serde_json::json!({ "socket": path, "uid": uid })),
            ClientError::Timeout => ErrorObject::new(
                codes::TIMEOUT,
                "Timed out waiting for Lodestone Launcher. Whatever it was doing goes on; check with `jobs list`.",
            ),
        }
    }
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_error().message)
    }
}

impl std::error::Error for ClientError {}

/// A connection to a running launcher.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: u64,
}

impl Client {
    pub fn connect(path: &Path) -> Result<Client, ClientError> {
        let stream = UnixStream::connect(path).map_err(|err| ClientError::NotRunning {
            path: path.to_path_buf(),
            reason: err.to_string(),
        })?;
        // Only talk to a launcher run by this user. Under the shared /tmp fallback,
        // another user could have put a socket there first, and requests may carry
        // core passwords.
        let uid = crate::peer_uid(&stream).map_err(|err| ClientError::Io(err.to_string()))?;
        if uid != crate::current_uid() {
            return Err(ClientError::NotYours {
                path: path.to_path_buf(),
                uid,
            });
        }
        let writer = stream
            .try_clone()
            .map_err(|err| ClientError::Io(err.to_string()))?;
        Ok(Client {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
        })
    }

    /// Sends a request and returns its id.
    pub fn send(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = Value::from(self.next_id);
        self.next_id += 1;
        let line = Request::new(id.clone(), method, params).to_line();
        self.writer
            .write_all(line.as_bytes())
            .map_err(|err| ClientError::Io(err.to_string()))?;
        Ok(id)
    }

    /// The next line from the launcher, waiting at most `timeout` (forever with `None`).
    pub fn next_message(&mut self, timeout: Option<Duration>) -> Result<Message, ClientError> {
        self.reader
            .get_ref()
            .set_read_timeout(timeout)
            .map_err(|err| ClientError::Io(err.to_string()))?;
        let mut line = Vec::new();
        let read = (&mut self.reader)
            .take(MAX_LINE_BYTES as u64 * 4)
            .read_until(b'\n', &mut line);
        match read {
            Ok(0) => Err(ClientError::Io("the launcher closed the connection".into())),
            Ok(_) => {
                let text = String::from_utf8_lossy(&line);
                Message::parse(text.trim()).map_err(ClientError::Io)
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Err(ClientError::Timeout)
            }
            Err(err) => Err(ClientError::Io(err.to_string())),
        }
    }

    /// Calls `method` and waits for its response. Progress lines for it go to
    /// `on_progress`; events and other lines are handed to `on_other`.
    pub fn call_with(
        &mut self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        mut on_progress: impl FnMut(&Value),
        mut on_other: impl FnMut(Message),
    ) -> Result<Result<Value, ErrorObject>, ClientError> {
        let id = match self.send(method, params) {
            Ok(id) => id,
            // a launcher that refuses the connection says why before closing it
            Err(err) => {
                return match self.next_message(Some(Duration::from_secs(1))) {
                    Ok(Message::Response { id, result }) if id.is_null() => Ok(result),
                    _ => Err(err),
                };
            }
        };
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        loop {
            let left = match deadline {
                Some(deadline) => {
                    let left = deadline.saturating_duration_since(std::time::Instant::now());
                    if left.is_zero() {
                        return Err(ClientError::Timeout);
                    }
                    Some(left)
                }
                None => None,
            };
            match self.next_message(left)? {
                Message::Response { id: got, result } if got == id || got.is_null() => {
                    return Ok(result);
                }
                Message::Progress { id: got, progress } if got == id => on_progress(&progress),
                other => on_other(other),
            }
        }
    }

    /// Calls `method` and waits for its response, ignoring progress.
    pub fn call(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, ErrorObject>, ClientError> {
        self.call_with(method, params, None, |_| {}, |_| {})
    }
}
