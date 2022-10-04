use async_trait::async_trait;
use serde_json;

use super::{MaybeUnsupported, Unsupported};

#[async_trait]
pub trait TPlayerManagement : Sync + Send {
    async fn get_player_count(&self) -> MaybeUnsupported<u32>
    
    {
        Unsupported
    }
    async fn get_max_player_count(&self) -> MaybeUnsupported<u32>
    
    {
        Unsupported
    }
    async fn get_player_list(&self) -> MaybeUnsupported<Vec<serde_json::Value>>
    
    {
        Unsupported
    }

    async fn set_max_player_count(&mut self, _max_player_count: u32) -> MaybeUnsupported<()>
    
    {
        Unsupported
    }
}
