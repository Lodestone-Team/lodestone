use async_trait::async_trait;
use serde_json::json;

use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_player::TPlayerManagement;
use crate::traits::ErrorInner;
use crate::Error;

use super::MinecraftInstance;

#[async_trait]
impl TPlayerManagement for MinecraftInstance {
    async fn get_player_count(&self) -> Result<u32, Error> {
        Ok(self.players.lock().await.get_ref().len() as u32)
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

    async fn get_player_list(&self) -> Result<Vec<serde_json::Value>, Error> {
        Ok(self
            .players
            .lock()
            .await
            .get_ref()
            .iter()
            .map(|name| json!({ "name": name }))
            .collect())
    }

    async fn set_max_player_count(&mut self, max_player_count: u32) -> Result<(), Error> {
        self.set_field("max-players", max_player_count.to_string())
            .await
    }
}
