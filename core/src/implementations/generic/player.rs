use std::collections::HashSet;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    error::Error,
    traits::t_player::{Player, TPlayer, TPlayerManagement},
};

use super::{bridge::procedure_call::ProcedureCallInner, GenericInstance};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, TS, Clone, Hash)]
#[ts(export)]
pub struct GenericPlayer {
    pub id: String,
    pub name: String,
}

impl TPlayer for GenericPlayer {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

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
