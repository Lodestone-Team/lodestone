//! The info stream: what a friend's launcher needs to know to join a shared server.
//!
//! An info stream ([`crate::StreamKind::Info`]) carries plain HTTP/1.1 to a small part of
//! the host core's API, for the shared instance only:
//!
//! - `GET /api/v1/connect/info`: [`SharedServer`].
//! - `GET /api/v1/info`: the core's name, version and id (as the core's own `/info`).
//! - `GET /api/v1/public/instances`: the shared instance alone, as the core's public
//!   instance list shows it (no address: the friend plays through the tunnel).
//! - `GET /api/v1/public/instances/<uuid>/client-pack`: its client modpack (Fabric
//!   servers with mods), with `ETag` and `If-None-Match` as the core does it.
//!
//! Nothing else answers. So a launcher can point its core client at a local port that
//! forwards info streams (`Joined::forward`) and fetch the client pack as it does from a
//! core whose servers are listed publicly.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::{Error, Result, TunnelStream};

/// The path of [`SharedServer`] on an info stream.
pub const INFO_PATH: &str = "/api/v1/connect/info";
/// The most of an answer [`get`] reads.
const MAX_ANSWER: u64 = 256 * 1024;

/// The shared server, as a friend's launcher sees it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SharedServer {
    /// The host core's name.
    pub core_name: String,
    /// The host core's id.
    pub core_uuid: String,
    /// The shared instance's id (for the client pack path).
    pub instance_uuid: String,
    /// The server's name.
    pub name: String,
    /// The Minecraft version, such as `1.21.4`.
    pub game_version: String,
    /// `vanilla`, `fabric`, `forge`, `paper`, `spigot` or `other`.
    pub loader: String,
    /// The Fabric loader version, the full Forge version or the Paper build, when known.
    #[serde(default)]
    pub loader_version: Option<String>,
    /// The server has mods the friend needs too: fetch its client pack from
    /// `/api/v1/public/instances/<instance_uuid>/client-pack`.
    #[serde(default)]
    pub modpack: bool,
    /// `Starting`, `Running`, `Stopping`, `Stopped` or `Error`, when known.
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub player_count: Option<u32>,
    #[serde(default)]
    pub max_player_count: Option<u32>,
}

/// An HTTP/1.1 answer's status and body (de-chunked). `None` if it isn't one.
pub fn parse_response(response: &[u8]) -> Option<(u16, Vec<u8>)> {
    let end = response.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&response[..end]);
    let rest = &response[end + 4..];
    let mut lines = head.lines();
    let status: u16 = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let chunked = lines.any(|line| {
        let line = line.to_ascii_lowercase();
        line.starts_with("transfer-encoding:") && line.contains("chunked")
    });
    let body = if chunked {
        dechunk(rest)
    } else {
        rest.to_vec()
    };
    Some((status, body))
}

fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = body.windows(2).position(|w| w == b"\r\n") {
        let size = std::str::from_utf8(&body[..line_end])
            .ok()
            .and_then(|s| usize::from_str_radix(s.split(';').next()?.trim(), 16).ok());
        let Some(size) = size else { break };
        let rest = &body[line_end + 2..];
        if size == 0 || rest.len() < size {
            break;
        }
        out.extend_from_slice(&rest[..size]);
        body = rest[size..].strip_prefix(b"\r\n").unwrap_or(&rest[size..]);
    }
    out
}

/// `GET path` over an info stream; returns the status and body.
pub async fn get(mut stream: TunnelStream, path: &str) -> Result<(u16, Vec<u8>)> {
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: lodestone-connect\r\nConnection: close\r\n\r\n");
    let failed = |err: std::io::Error| Error::Tunnel(err.to_string());
    stream.write_all(request.as_bytes()).await.map_err(failed)?;
    stream.flush().await.map_err(failed)?;
    let mut response = Vec::new();
    (&mut stream)
        .take(MAX_ANSWER)
        .read_to_end(&mut response)
        .await
        .map_err(failed)?;
    if response.is_empty() {
        return Err(Error::Tunnel("the host shares no server info".into()));
    }
    parse_response(&response).ok_or_else(|| Error::Tunnel("the host's answer was garbled".into()))
}

/// Reads [`SharedServer`] from an info stream.
pub async fn shared_server(stream: TunnelStream) -> Result<SharedServer> {
    let (status, body) = get(stream, INFO_PATH).await?;
    if status != 200 {
        return Err(Error::Tunnel(format!(
            "the host answered {status} when asked about its server"
        )));
    }
    serde_json::from_slice(&body)
        .map_err(|err| Error::Tunnel(format!("the host's server info was garbled: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_and_chunked_answers() {
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nhi"),
            Some((200, b"hi".to_vec()))
        );
        assert_eq!(
            parse_response(
                b"HTTP/1.1 404 Not Found\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
            ),
            Some((404, b"hello world".to_vec()))
        );
        assert_eq!(parse_response(b"garbage"), None);
    }

    #[test]
    fn shared_server_tolerates_missing_and_new_fields() {
        let server: SharedServer = serde_json::from_str(
            r#"{"core_name":"Home","core_uuid":"c","instance_uuid":"i","name":"SMP",
                "game_version":"1.21.4","loader":"fabric","something_new":1}"#,
        )
        .unwrap();
        assert_eq!(server.loader, "fabric");
        assert!(!server.modpack);
    }
}
