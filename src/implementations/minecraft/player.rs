use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::traits::t_player::{TPlayer, TPlayerManagement};
use crate::traits::ErrorInner;
use crate::traits::{t_configurable::TConfigurable, t_player::Player};
use crate::Error;

use super::MinecraftInstance;

#[derive(Eq, Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MinecraftPlayer {
    pub name: String,
    pub uuid: String,
}

impl MinecraftPlayer {
    pub fn new(name: String, uuid: String) -> Self {
        Self { name, uuid }
    }
}

impl PartialEq for MinecraftPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
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
        self.uuid.clone()
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
        self.get_field("max-players")
            .await
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .map_err(|_| Error {
                inner: ErrorInner::MalformedFile,
                detail: "Invalid value for max-players".to_string(),
            })
    }

    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        Ok(self.players_manager.lock().await.clone().into())
    }

    async fn set_max_player_count(&mut self, max_player_count: u32) -> Result<(), Error> {
        self.set_field("max-players", max_player_count.to_string())
            .await
    }
}
