use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::traits::t_player::Player;
use crate::traits::t_player::{TPlayer, TPlayerManagement};
use crate::Error;

use super::server_properties;
use super::MinecraftInstance;

#[derive(Eq, Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MinecraftPlayer {
    pub name: String,
    pub uuid: Option<String>,
}

impl MinecraftPlayer {
    pub fn new(name: String, uuid: Option<String>) -> Self {
        Self { name, uuid }
    }
}

// Players are identified by name, which is unique on a server at any time. The UUID is
// only known when the Mojang lookup succeeded, so it can't be used for equality, and Hash
// must agree with Eq.
impl PartialEq for MinecraftPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
impl Hash for MinecraftPlayer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl TPlayer for MinecraftPlayer {
    fn get_id(&self) -> String {
        self.uuid.clone().unwrap_or_else(|| self.name.clone())
    }
}

#[async_trait]
impl TPlayerManagement for MinecraftInstance {
    async fn get_player_count(&self) -> Result<u32, Error> {
        Ok(self.players_manager.lock().await.count())
    }

    async fn get_max_player_count(&self) -> Result<u32, Error> {
        // 20 is Minecraft's default; also used when the value is not a valid number
        Ok(self
            .configurable_manifest
            .lock()
            .await
            .get_setting(server_properties::SECTION_ID, "max-players")
            .and_then(|v| v.get_value())
            .and_then(|v| v.try_as_unsigned_integer().ok())
            .unwrap_or(20))
    }

    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        Ok(self.players_manager.lock().await.players())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_are_identified_by_name() {
        let with_uuid = MinecraftPlayer::new("Steve".to_string(), Some("abc".to_string()));
        let without_uuid = MinecraftPlayer::new("Steve".to_string(), None);
        let other = MinecraftPlayer::new("Alex".to_string(), Some("abc".to_string()));
        assert_eq!(with_uuid, without_uuid);
        assert_ne!(with_uuid, other);
        let set: HashSet<_> = [with_uuid, without_uuid, other].into_iter().collect();
        assert_eq!(set.len(), 2);
    }
}
