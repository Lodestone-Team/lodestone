//! Parses Minecraft server console lines.
//!
//! A log line starts with one or more bracketed groups and ": ", for example
//! `[12:34:56] [Server thread/INFO]: `, `[12:34:56 INFO]: ` (Paper) or
//! `[06Nov2025 12:34:56.789] [Server thread/INFO] [net.minecraft.server.MinecraftServer/]: `
//! (Forge). The patterns are anchored to that prefix so that chat text such as
//! `<Steve> [x]: Notch joined the game` can't fake a system message.

use std::sync::LazyLock;

use fancy_regex::Regex;

pub struct PlayerMessage {
    pub player: String,
    pub message: String,
}

/// The log prefix, then the message
static LOG_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\[[^\]\r\n]*\][ \t]*)+: (.*)$").unwrap());

/// Chat: `<name> message`, optionally marked `[Not Secure]` by 1.19+ servers
static CHAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\[Not Secure\] )?<([^<>\s]{1,32})> (.*)$").unwrap());

/// Player names are 3-16 word characters; Bedrock players (Geyser/Floodgate) get a '.' or
/// '*' prefix. Renamed players are shown as `Name (formerly known as Old)`.
static JOINED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([.*]?\w{1,16})(?: \(formerly known as [.*]?\w{1,16}\))? joined the game$")
        .unwrap()
});
static LEFT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([.*]?\w{1,16}) left the game$").unwrap());

static DONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Done \([^)]*\)!").unwrap());

fn log_message(line: &str) -> Option<&str> {
    let line = line.trim_end_matches(['\r', '\n']);
    let caps = LOG_LINE.captures(line).ok()??;
    Some(caps.get(1)?.as_str())
}

/// The message of a server (non-chat) log line.
pub fn parse_system_msg(line: &str) -> Option<String> {
    let message = log_message(line)?;
    if CHAT.is_match(message).ok()? {
        None
    } else {
        Some(message.to_string())
    }
}

pub fn parse_player_msg(line: &str) -> Option<PlayerMessage> {
    let caps = CHAT.captures(log_message(line)?).ok()??;
    Some(PlayerMessage {
        player: caps.get(1)?.as_str().to_string(),
        message: caps.get(2)?.as_str().to_string(),
    })
}

/// The player name from a system message such as `Steve joined the game`.
pub fn parse_player_joined(system_msg: &str) -> Option<String> {
    let caps = JOINED.captures(system_msg.trim_end()).ok()??;
    Some(caps.get(1)?.as_str().to_string())
}

pub fn parse_player_left(system_msg: &str) -> Option<String> {
    let caps = LEFT.captures(system_msg.trim_end()).ok()??;
    Some(caps.get(1)?.as_str().to_string())
}

/// True for the "Done (12.3s)! For help, type "help"" line.
///
/// Not anchored to the log prefix, because very old versions use a different log format,
/// but chat lines never count.
pub fn parse_server_started(line: &str) -> bool {
    parse_player_msg(line).is_none() && DONE.is_match(line).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_messages() {
        assert_eq!(
            parse_system_msg("[12:00:00] [Server thread/INFO]: Steve joined the game\n").as_deref(),
            Some("Steve joined the game")
        );
        assert_eq!(
            parse_system_msg("[12:00:00 INFO]: Steve left the game").as_deref(),
            Some("Steve left the game")
        );
        assert_eq!(
            parse_system_msg(
                "[06Nov2025 12:00:00.000] [Server thread/INFO] [net.minecraft.server.MinecraftServer/]: Steve joined the game"
            )
            .as_deref(),
            Some("Steve joined the game")
        );
        assert!(parse_system_msg("[12:00:00] [Server thread/INFO]: <Steve> hello").is_none());
        assert!(parse_system_msg("no prefix at all").is_none());
    }

    #[test]
    fn chat_cannot_fake_joins_or_leaves() {
        for line in [
            "[12:00:00] [Server thread/INFO]: <Steve> [x]: Notch joined the game",
            "[12:00:00] [Server thread/INFO]: [Not Secure] <Steve> [x]: Notch left the game",
            "[12:00:00] [Async Chat Thread - #0/INFO]: <Steve> Notch joined the game",
        ] {
            assert!(parse_system_msg(line).is_none(), "{line}");
            assert!(parse_player_msg(line).is_some(), "{line}");
        }
        // /say and /me output is a system message, but not a join
        let say = parse_system_msg("[12:00:00] [Server thread/INFO]: [Server] Bob joined the game")
            .unwrap();
        assert_eq!(parse_player_joined(&say), None);
        let me =
            parse_system_msg("[12:00:00] [Server thread/INFO]: * Steve joined the game").unwrap();
        assert_eq!(parse_player_joined(&me), None);
    }

    #[test]
    fn joins_and_leaves() {
        assert_eq!(
            parse_player_joined("Steve_123 joined the game").as_deref(),
            Some("Steve_123")
        );
        assert_eq!(
            parse_player_joined("Steve (formerly known as Alex) joined the game").as_deref(),
            Some("Steve")
        );
        assert_eq!(
            parse_player_joined(".BedrockGuy joined the game").as_deref(),
            Some(".BedrockGuy")
        );
        assert_eq!(
            parse_player_left("Steve left the game").as_deref(),
            Some("Steve")
        );
        assert_eq!(parse_player_left("Steve left the game again"), None);
    }

    #[test]
    fn player_messages() {
        let msg =
            parse_player_msg("[12:00:00] [Server thread/INFO]: <Steve> hi <there>\n").unwrap();
        assert_eq!(msg.player, "Steve");
        assert_eq!(msg.message, "hi <there>");
        let msg = parse_player_msg("[12:00:00 INFO]: [Not Secure] <Steve> hello world").unwrap();
        assert_eq!(msg.player, "Steve");
        assert_eq!(msg.message, "hello world");
    }

    #[test]
    fn server_started() {
        assert!(parse_server_started(
            "[12:00:00] [Server thread/INFO]: Done (3.456s)! For help, type \"help\""
        ));
        assert!(parse_server_started(
            "2013-07-01 12:00:00 [INFO] Done (1.2s)! For help, type \"help\" or \"?\""
        ));
        assert!(!parse_server_started(
            "[12:00:00] [Server thread/INFO]: <Steve> Done (1s)!"
        ));
        assert!(!parse_server_started(
            "[12:00:00] [Server thread/INFO]: Starting"
        ));
    }
}
