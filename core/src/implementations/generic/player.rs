use std::collections::HashSet;

use async_trait::async_trait;

pub use crate::traits::t_player::GenericPlayer;
use crate::{
    error::Error,
    traits::t_player::{Player, TPlayerManagement},
};

use super::{bridge::procedure_call::ProcedureCallInner, GenericInstance};

#[async_trait]
impl TPlayerManagement for GenericInstance {
    async fn get_player_count(&self) -> Result<u32, Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::GetPlayerCount)
            .await?
            .try_into()
    }
    async fn get_max_player_count(&self) -> Result<u32, Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::GetMaxPlayerCount)
            .await?
            .try_into()
    }
    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::GetPlayerList)
            .await?
            .try_into()
    }
}
