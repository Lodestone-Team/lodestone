//! What a Lodestone core sends: the parts of `/info`, the instance lists and the replies
//! of signing in. Unknown fields are ignored and unknown values map to `Other`, so a
//! newer core still parses.

use serde::Deserialize;

/// The parts of `GET /info` the launcher uses.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct CoreInfo {
    pub version: String,
    pub uuid: String,
    pub core_name: String,
    #[serde(default)]
    pub is_setup: bool,
}

/// The mod loader of a server.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerLoader {
    Vanilla,
    Fabric,
    Forge,
    Paper,
    Spigot,
    #[serde(other)]
    Other,
}

impl ServerLoader {
    pub fn label(self) -> &'static str {
        match self {
            ServerLoader::Vanilla => "Vanilla",
            ServerLoader::Fabric => "Fabric",
            ServerLoader::Forge => "Forge",
            ServerLoader::Paper => "Paper",
            ServerLoader::Spigot => "Spigot",
            ServerLoader::Other => "Other",
        }
    }
}

/// Whether a server runs. `Unknown` also when the owner doesn't show the status
/// publicly.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum ServerState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
    #[serde(other)]
    #[default]
    Unknown,
}

impl ServerState {
    pub fn label(self) -> &'static str {
        match self {
            ServerState::Starting => "Starting",
            ServerState::Running => "Running",
            ServerState::Stopping => "Stopping",
            ServerState::Stopped => "Stopped",
            ServerState::Error => "Crashed",
            ServerState::Unknown => "Unknown",
        }
    }
}

/// A server (a core's instance) as `GET /public/instances` and
/// `GET /launcher/instances` describe it. The owner can leave the status, the address
/// (port and playit.gg address) and the client pack out of the public list; those fields
/// are then missing.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServerInstance {
    pub uuid: String,
    pub name: String,
    pub game_version: String,
    pub loader: ServerLoader,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub port: Option<u32>,
    #[serde(default)]
    pub state: ServerState,
    #[serde(default)]
    pub player_count: Option<u32>,
    #[serde(default)]
    pub max_player_count: Option<u32>,
    #[serde(default)]
    pub has_client_pack: bool,
    #[serde(default)]
    pub playit_address: Option<String>,
    #[serde(default)]
    pub listed: bool,
}

impl ServerInstance {
    /// "3 / 20 players", or `None` when the core doesn't know.
    pub fn players_label(&self) -> Option<String> {
        let count = self.player_count?;
        Some(match self.max_player_count {
            Some(max) => format!("{count} / {max} players"),
            None => format!("{count} players"),
        })
    }
}

/// The list replies.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ServerList {
    pub core_name: String,
    pub core_uuid: String,
    pub listing_enabled: bool,
    pub instances: Vec<ServerInstance>,
}

/// `POST /user/login`. Only the token is read; it is wrapped in a `Secret` at once.
#[derive(Deserialize)]
pub(crate) struct LoginReply {
    pub token: String,
}

/// The `info` of `POST /user/api_keys`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct KeyInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

/// `POST /user/api_keys`. No `Debug`: it holds the key.
#[derive(Deserialize)]
pub(crate) struct CreatedKeyReply {
    pub key: String,
    pub info: KeyInfo,
}

/// The JSON error body of the core: `{ "kind": …, "causes": ["message", …] }`.
#[derive(Deserialize)]
pub(crate) struct ErrorBody {
    #[serde(default)]
    pub causes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_list_and_tolerates_new_values() {
        let list: ServerList = serde_json::from_value(serde_json::json!({
            "core_name": "Home", "core_uuid": "c1", "listing_enabled": true, "extra": 1,
            "instances": [{
                "uuid": "i1", "name": "SMP", "game_version": "1.20.1", "loader": "fabric",
                "loader_version": "0.16.14", "port": 25565, "state": "Running",
                "player_count": 2, "max_player_count": 20, "has_client_pack": true,
                "playit_address": null, "listed": true
            }, {
                "uuid": "i2", "name": "New", "game_version": "26.3", "loader": "quilt",
                "loader_version": null, "port": 25566, "state": "Hibernating"
            }, {
                "uuid": "i3", "name": "Scoped", "game_version": "1.21.1", "loader": "vanilla",
                "loader_version": null, "listed": true
            }]
        }))
        .unwrap();
        assert_eq!(list.instances[0].loader, ServerLoader::Fabric);
        assert_eq!(
            list.instances[0].players_label().as_deref(),
            Some("2 / 20 players")
        );
        assert_eq!(list.instances[1].loader, ServerLoader::Other);
        assert_eq!(list.instances[1].state, ServerState::Unknown);
        assert_eq!(list.instances[1].players_label(), None);
        // the owner hid the status, address and client pack
        assert_eq!(list.instances[2].state, ServerState::Unknown);
        assert_eq!(list.instances[2].port, None);
        assert!(!list.instances[2].has_client_pack);
    }
}
