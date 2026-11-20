//! The Discord app's local RPC connection: a named pipe on Windows
//! (`\\.\pipe\discord-ipc-N`) and a Unix socket elsewhere (`$XDG_RUNTIME_DIR` or the
//! temporary folder, `discord-ipc-N`), N from 0 to 9.
//!
//! Each frame is an opcode and a length, both little-endian `u32`, then that many
//! bytes of JSON. The client says hello (`HANDSHAKE` with its application id), Discord
//! answers `READY`, and from then on every command gets one answer with the same
//! `nonce`. Only `SET_ACTIVITY` is used. Closing the connection clears the status.
//!
//! Everything blocks: the app calls this from its own worker thread.

use std::{
    io::{self, Read, Write},
    path::PathBuf,
};

use serde::Serialize;
use serde_json::{Value, json};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

/// Discord's answers are small; anything bigger isn't Discord.
const MAX_FRAME: usize = 64 * 1024;

/// How long a read or write waits before the connection counts as lost. Windows'
/// pipes, opened as files, have no timeout; Discord answers at once.
#[cfg(unix)]
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The longest text Discord takes in an activity's fields, in characters.
pub const MAX_TEXT: usize = 128;
/// The shortest: a field with less is refused, so it is left out instead.
const MIN_TEXT: usize = 2;

/// What the status shows. Discord draws `details` and `state` as two lines under the
/// application's name, the time since `start`, and the images with their tooltips.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Activity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    #[serde(skip_serializing_if = "Assets::is_empty")]
    pub assets: Assets,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<ActivityButton>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Timestamps {
    /// Unix milliseconds: Discord shows the time since.
    pub start: u64,
}

/// The images: an art asset's key in the Discord application, or an `https://` URL.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Assets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

impl Assets {
    fn is_empty(&self) -> bool {
        self.large_image.is_none() && self.small_image.is_none()
    }
}

/// A link under the status; Discord takes two at most.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActivityButton {
    pub label: String,
    pub url: String,
}

/// `text` as Discord takes it: cut to [`MAX_TEXT`] characters (with an ellipsis), or
/// `None` when shorter than Discord allows.
pub fn field(text: impl AsRef<str>) -> Option<String> {
    let text = text.as_ref().trim();
    if text.chars().count() < MIN_TEXT {
        return None;
    }
    if text.chars().count() <= MAX_TEXT {
        return Some(text.to_owned());
    }
    let mut cut: String = text.chars().take(MAX_TEXT - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    Some(cut)
}

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// The Discord app isn't running (or doesn't take RPC connections).
    #[error("Discord isn't running")]
    NotRunning,
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Discord closed the connection or refused a command.
    #[error("Discord: {0}")]
    Refused(String),
}

#[cfg(windows)]
type Stream = std::fs::File;
#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;

/// A connection to the Discord app.
pub struct Connection {
    stream: Stream,
    nonce: u64,
}

impl Connection {
    /// Connects to the first Discord app that answers, and says hello as `client_id`.
    pub fn open(client_id: &str) -> Result<Self, IpcError> {
        for path in candidates() {
            let Some(stream) = open_stream(&path) else {
                continue;
            };
            let mut connection = Connection { stream, nonce: 0 };
            connection.handshake(client_id)?;
            return Ok(connection);
        }
        Err(IpcError::NotRunning)
    }

    fn handshake(&mut self, client_id: &str) -> Result<(), IpcError> {
        self.write(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))?;
        loop {
            let (op, body) = self.read()?;
            match op {
                OP_FRAME if body["evt"] == "READY" => return Ok(()),
                OP_PING => self.write(OP_PONG, &body)?,
                OP_CLOSE => return Err(IpcError::Refused(message(&body))),
                _ if body["evt"] == "ERROR" => return Err(IpcError::Refused(message(&body))),
                _ => {}
            }
        }
    }

    /// Shows `activity`, or clears the status with `None`. `pid` is the process the
    /// status belongs to: Discord clears it when that process exits.
    pub fn set_activity(&mut self, pid: u32, activity: Option<&Activity>) -> Result<(), IpcError> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        self.write(
            OP_FRAME,
            &json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": pid, "activity": activity },
                "nonce": nonce,
            }),
        )?;
        loop {
            let (op, body) = self.read()?;
            match op {
                OP_FRAME if body["nonce"] == nonce.as_str() => {
                    return if body["evt"] == "ERROR" {
                        Err(IpcError::Refused(message(&body)))
                    } else {
                        Ok(())
                    };
                }
                OP_PING => self.write(OP_PONG, &body)?,
                OP_CLOSE => return Err(IpcError::Refused(message(&body))),
                // Events and answers to earlier commands.
                _ => {}
            }
        }
    }

    fn write(&mut self, op: u32, body: &Value) -> io::Result<()> {
        let body = serde_json::to_vec(body)?;
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&body);
        self.stream.write_all(&frame)?;
        self.stream.flush()
    }

    fn read(&mut self) -> Result<(u32, Value), IpcError> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if len > MAX_FRAME {
            return Err(IpcError::Refused(format!("a {len}-byte answer")));
        }
        let mut body = vec![0u8; len];
        self.stream.read_exact(&mut body)?;
        let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
        Ok((op, body))
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Says goodbye; closing the connection clears the status either way.
        let _ = self.write(OP_CLOSE, &json!({}));
    }
}

/// The message of an `ERROR` event or a `CLOSE` frame.
fn message(body: &Value) -> String {
    body["data"]["message"]
        .as_str()
        .or_else(|| body["message"].as_str())
        .unwrap_or("closed the connection")
        .to_owned()
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    (0..10)
        .map(|n| PathBuf::from(format!(r"\\.\pipe\discord-ipc-{n}")))
        .collect()
}

#[cfg(windows)]
fn open_stream(path: &std::path::Path) -> Option<Stream> {
    // A pipe that no Discord serves fails at once, without touching a disk.
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()
}

#[cfg(unix)]
fn candidates() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    roots.push(PathBuf::from("/tmp"));
    roots.dedup();
    // Flatpak and Snap builds of Discord, and Vesktop, keep the socket a folder down.
    let subfolders = [
        "",
        "app/com.discordapp.Discord",
        "app/com.discordapp.DiscordCanary",
        "snap.discord",
        "snap.discord-canary",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
    ];
    let mut paths = Vec::new();
    for root in &roots {
        for sub in subfolders {
            let dir = if sub.is_empty() {
                root.clone()
            } else {
                root.join(sub)
            };
            for n in 0..10 {
                paths.push(dir.join(format!("discord-ipc-{n}")));
            }
        }
    }
    paths
}

#[cfg(unix)]
fn open_stream(path: &std::path::Path) -> Option<Stream> {
    let stream = std::os::unix::net::UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(TIMEOUT)).ok()?;
    Some(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_cut_to_what_discord_takes() {
        assert_eq!(field("a"), None);
        assert_eq!(field("  "), None);
        assert_eq!(field("ab"), Some("ab".into()));
        let long = "é".repeat(200);
        let cut = field(&long).unwrap();
        assert_eq!(cut.chars().count(), MAX_TEXT);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn an_activity_leaves_out_what_it_lacks() {
        let activity = Activity {
            details: Some("All the Mods 9".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&activity).unwrap(),
            json!({ "details": "All the Mods 9" })
        );
        let activity = Activity {
            state: Some("Overworld".into()),
            timestamps: Some(Timestamps { start: 5 }),
            assets: Assets {
                large_image: Some("lodestone".into()),
                ..Default::default()
            },
            buttons: vec![ActivityButton {
                label: "View modpack".into(),
                url: "https://modrinth.com/modpack/x".into(),
            }],
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&activity).unwrap(),
            json!({
                "state": "Overworld",
                "timestamps": { "start": 5 },
                "assets": { "large_image": "lodestone" },
                "buttons": [{ "label": "View modpack", "url": "https://modrinth.com/modpack/x" }],
            })
        );
    }

    /// A fake Discord on a Unix socket: answers the handshake and one command.
    #[cfg(unix)]
    #[test]
    fn talks_to_discord() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("discord-ipc-0");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut fake = Connection { stream, nonce: 0 };
            let (op, hello) = fake.read().unwrap();
            assert_eq!(op, OP_HANDSHAKE);
            assert_eq!(hello["client_id"], "42");
            fake.write(OP_FRAME, &json!({ "evt": "READY", "cmd": "DISPATCH" }))
                .unwrap();
            let (_, command) = fake.read().unwrap();
            assert_eq!(command["cmd"], "SET_ACTIVITY");
            assert_eq!(command["args"]["activity"]["details"], "Hello");
            fake.write(
                OP_FRAME,
                &json!({ "cmd": "SET_ACTIVITY", "nonce": command["nonce"], "data": {} }),
            )
            .unwrap();
        });
        let stream = open_stream(&path).unwrap();
        let mut connection = Connection { stream, nonce: 0 };
        connection.handshake("42").unwrap();
        let activity = Activity {
            details: Some("Hello".into()),
            ..Default::default()
        };
        connection.set_activity(1, Some(&activity)).unwrap();
        server.join().unwrap();
    }
}
