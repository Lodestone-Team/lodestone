//! The Console tab: an instance's console history and sending it commands, and which
//! players are linked to Lodestone users (for the badges on console lines). The live
//! console is `LiveStream::open(.., StreamKind::Console, ..)` (`cores::live`), whose
//! lines merge into the history with [`merge_console`].
//!
//! Work package B (see `launcher/docs/host-parity.md`, §4).

use std::collections::HashMap;

use http_client::Method;
use serde::Deserialize;
use serde_json::Value;

use super::{AdminClient, segment};
use crate::cores::CoreError;
pub use crate::cores::live::{ConsoleLine, MAX_CONSOLE_LINES, merge_console};

/// The most commands the console's history keeps, as the dashboard's
/// `CommandHistoryContext`.
pub const MAX_COMMAND_HISTORY: usize = 50;

impl AdminClient {
    /// `GET /instance/{uuid}/console/buffer`: the console lines the core keeps,
    /// oldest first.
    pub async fn console_buffer(&self, uuid: &str) -> Result<Vec<ConsoleLine>, CoreError> {
        let events: Vec<Value> = self
            .get(&format!("instance/{}/console/buffer", segment(uuid)))
            .await?;
        Ok(crate::cores::live::parse_buffer(events, uuid))
    }

    /// `POST /instance/{uuid}/console`: runs `command` on the server's console, as
    /// typed (the dashboard sends it as it is, without a leading `/` added or taken).
    pub async fn send_console(&self, uuid: &str, command: &str) -> Result<(), CoreError> {
        self.call(
            Method::POST,
            &format!("instance/{}/console", segment(uuid)),
            Some(command),
        )
        .await
    }

    /// The Lodestone users of linked Minecraft accounts: every link for the owner
    /// (`GET /minecraft_links`), the user's own for everyone else
    /// (`GET /user/minecraft_links`), as the dashboard's `useLinkedPlayers`. An older
    /// core without links gives none.
    pub async fn linked_players(
        &self,
        is_owner: bool,
        username: &str,
    ) -> Result<LinkedPlayers, CoreError> {
        let path = if is_owner {
            "minecraft_links"
        } else {
            "user/minecraft_links"
        };
        let links: Vec<Link> = match self.get(path).await {
            Ok(links) => links,
            Err(CoreError::NotFound(_)) => Vec::new(),
            Err(err) => return Err(err),
        };
        Ok(LinkedPlayers::new(links.into_iter().map(|link| {
            let user = link.username.unwrap_or_else(|| username.to_string());
            (link.minecraft_uuid, link.minecraft_name, user)
        })))
    }
}

#[derive(Deserialize)]
struct Link {
    minecraft_uuid: String,
    minecraft_name: String,
    /// Only in the owner's list of every link.
    #[serde(default)]
    username: Option<String>,
}

/// Lodestone usernames of linked Minecraft players, by name and UUID
/// (`utils/linkedPlayers.ts`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkedPlayers {
    /// By lowercase Minecraft name.
    by_name: HashMap<String, String>,
    /// By Minecraft UUID, lowercase with dashes.
    by_uuid: HashMap<String, String>,
}

impl LinkedPlayers {
    /// From `(minecraft_uuid, minecraft_name, lodestone_username)`.
    pub fn new(links: impl IntoIterator<Item = (String, String, String)>) -> Self {
        let mut this = Self::default();
        for (uuid, name, user) in links {
            this.by_name.insert(name.to_lowercase(), user.clone());
            this.by_uuid.insert(normalize_uuid(&uuid), user);
        }
        this
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// The Lodestone user of a player in the player list.
    pub fn user_of(&self, name: &str, uuid: Option<&str>) -> Option<&str> {
        uuid.and_then(|uuid| self.by_uuid.get(&normalize_uuid(uuid)))
            .or_else(|| self.by_name.get(&name.to_lowercase()))
            .map(String::as_str)
    }

    /// The linked player a console line is about, and their Lodestone user: the
    /// player who chats, joins or leaves (`linkedPlayerInLine`). Only the message
    /// after the log prefix (`[12:00:00] [Server thread/INFO]: `) is read, so a name
    /// elsewhere in the line doesn't count.
    pub fn in_line<'a>(&'a self, plain: &'a str) -> Option<(&'a str, &'a str)> {
        if self.by_name.is_empty() {
            return None;
        }
        let message = plain.find("]: ").map_or(plain, |at| &plain[at + 3..]);
        let player = player_in_message(message)?;
        let user = self.by_name.get(&player.to_lowercase())?;
        Some((player, user))
    }
}

/// A Minecraft UUID with or without dashes, lowercase with dashes.
pub fn normalize_uuid(uuid: &str) -> String {
    let hex: String = uuid.chars().filter(|c| *c != '-').collect::<String>();
    let hex = hex.to_lowercase();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return uuid.to_lowercase();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A Minecraft name at the start of `text` (1 to 16 letters, digits or `_`), and
/// what follows it.
fn name_at(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !is_name_char(c)).unwrap_or(text.len());
    (1..=16)
        .contains(&end)
        .then(|| (&text[..end], &text[end..]))
}

/// The player a server message starts with: chat (`<Steve> hi`, `[Not Secure] <Steve>
/// hi`), joining and leaving (`Steve joined the game`, `Steve[/127.0.0.1:5000] logged
/// in …`) and `/me` (`* Steve waves`). The dashboard's `PLAYER_IN_MESSAGE`.
fn player_in_message(message: &str) -> Option<&str> {
    let message = message.strip_prefix("[Not Secure] ").unwrap_or(message);
    if let Some(rest) = message.strip_prefix('<') {
        let (name, rest) = name_at(rest)?;
        return rest.starts_with('>').then_some(name);
    }
    if let Some(rest) = message.strip_prefix("* ") {
        let (name, rest) = name_at(rest)?;
        return rest.starts_with(' ').then_some(name);
    }
    let (name, rest) = name_at(message)?;
    if rest.starts_with(" joined the game") || rest.starts_with(" left the game") {
        return Some(name);
    }
    let address = rest.strip_prefix("[/")?;
    let close = address.find(']')?;
    address[close + 1..]
        .starts_with(" logged in")
        .then_some(name)
}

/// A console's sent commands, oldest first, and where ↑ and ↓ are in them (the
/// dashboard's `CommandHistoryContext` and `GameConsole`'s `commandNav`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandHistory {
    commands: Vec<String>,
    /// `commands.len()` when not browsing.
    at: usize,
}

impl CommandHistory {
    /// Remembers a sent command, keeping the last [`MAX_COMMAND_HISTORY`], and stops
    /// browsing.
    pub fn push(&mut self, command: String) {
        self.commands.push(command);
        if self.commands.len() > MAX_COMMAND_HISTORY {
            self.commands.remove(0);
        }
        self.at = self.commands.len();
    }

    /// ↑: the command before the one shown, if any.
    pub fn older(&mut self) -> Option<&str> {
        if self.commands.is_empty() {
            return None;
        }
        self.at = self.at.saturating_sub(1);
        self.commands.get(self.at).map(String::as_str)
    }

    /// ↓: the command after the one shown; at the newest, it stays there.
    pub fn newer(&mut self) -> Option<&str> {
        if self.commands.is_empty() {
            return None;
        }
        self.at = (self.at + 1).min(self.commands.len() - 1);
        self.commands.get(self.at).map(String::as_str)
    }

    /// Typing something else stops browsing.
    pub fn reset(&mut self) {
        self.at = self.commands.len();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linked() -> LinkedPlayers {
        LinkedPlayers::new([(
            "0123456789ABCDEF0123456789abcdef".to_string(),
            "Steve".to_string(),
            "peter".to_string(),
        )])
    }

    #[test]
    fn finds_the_player_a_line_is_about() {
        let linked = linked();
        let line = |text: &str| linked.in_line(text).map(|(_, user)| user.to_string());
        let prefix = "[12:00:00] [Server thread/INFO]: ";
        for message in [
            "<Steve> hello",
            "[Not Secure] <steve> hello",
            "Steve joined the game",
            "Steve left the game",
            "Steve[/127.0.0.1:50000] logged in with entity id 1",
            "* Steve waves",
        ] {
            assert_eq!(
                line(&format!("{prefix}{message}")).as_deref(),
                Some("peter"),
                "{message}"
            );
        }
        for message in [
            "<Alex> Steve is here",
            "Alex joined the game",
            "Steve has made the advancement",
            "<Steve hello",
            "<ThisNameIsFarTooLong> hi",
        ] {
            assert_eq!(line(&format!("{prefix}{message}")), None, "{message}");
        }
        assert_eq!(LinkedPlayers::default().in_line("<Steve> hi"), None);
    }

    #[test]
    fn players_are_found_by_uuid_then_name() {
        let linked = linked();
        assert_eq!(
            linked.user_of("Other", Some("01234567-89ab-cdef-0123-456789abcdef")),
            Some("peter")
        );
        assert_eq!(linked.user_of("STEVE", None), Some("peter"));
        assert_eq!(linked.user_of("Alex", Some("nope")), None);
        assert_eq!(normalize_uuid("not-a-uuid"), "not-a-uuid");
    }

    #[test]
    fn history_goes_up_and_down() {
        let mut history = CommandHistory::default();
        assert_eq!(history.older(), None);
        for command in ["list", "time set day", "say hi"] {
            history.push(command.into());
        }
        assert_eq!(history.older(), Some("say hi"));
        assert_eq!(history.older(), Some("time set day"));
        assert_eq!(history.older(), Some("list"));
        assert_eq!(history.older(), Some("list"), "stays at the oldest");
        assert_eq!(history.newer(), Some("time set day"));
        assert_eq!(history.newer(), Some("say hi"));
        assert_eq!(history.newer(), Some("say hi"), "stays at the newest");
        history.reset();
        assert_eq!(history.older(), Some("say hi"));
        for i in 0..100 {
            history.push(i.to_string());
        }
        assert_eq!(history.len(), MAX_COMMAND_HISTORY);
    }
}
