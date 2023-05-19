use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::traits::t_player::Player;
use crate::traits::t_player::{TPlayer, TPlayerManagement};
use crate::Error;

use super::configurable::ServerPropertySetting;
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

impl PartialEq for MinecraftPlayer {
    fn eq(&self, other: &Self) -> bool {
        // if uuid is not set, compare by name
        if self.uuid.is_none() || other.uuid.is_none() {
            self.name == other.name
        } else {
            self.uuid == other.uuid
        }
    }
}
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
impl Hash for MinecraftPlayer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl TPlayer for MinecraftPlayer {
    fn get_id(&self) -> String {
        self.uuid.clone().unwrap_or_else(|| self.name.clone())
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[async_trait]
impl TPlayerManagement for MinecraftInstance {
    async fn get_player_count(&self) -> Result<u32, Error> {
        Ok(self.players_manager.lock().await.count())
    }

    async fn get_max_player_count(&self) -> Result<u32, Error> {
        self.configurable_manifest
            .lock()
            .await
            .get_unique_setting_key(&ServerPropertySetting::MaxPlayers(0).get_identifier())
            .and_then(|v| v.get_value().map(|v| v.try_as_unsigned_integer()))
            .unwrap_or(Ok(20))
    }

    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        Ok(self.players_manager.lock().await.clone().into())
    }
}
