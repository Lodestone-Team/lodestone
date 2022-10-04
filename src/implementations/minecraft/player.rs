use async_trait::async_trait;
use serde_json::json;


use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_player::TPlayerManagement;
use crate::traits::Supported;

use super::Instance;

#[async_trait]
impl TPlayerManagement for Instance {
    async fn get_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        Supported(self.players.lock().await.get_ref().len() as u32)
    }

    async fn get_max_player_count(&self) -> crate::traits::MaybeUnsupported<u32> {
        Supported(
            self.get_field("max-players")
                .await
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .expect("Failed to parse max-players"),
        )
    }

    async fn get_player_list(&self) -> crate::traits::MaybeUnsupported<Vec<serde_json::Value>> {
        Supported(
            self.players
                .lock()
                .await
                .get_ref()
                .iter()
                .map(|name| json!({ "name": name }))
                .collect(),
        )
    }

    async fn set_max_player_count(
        &mut self,
        _max_player_count: u32,
    ) -> crate::traits::MaybeUnsupported<()> {
        todo!()
    }
}
