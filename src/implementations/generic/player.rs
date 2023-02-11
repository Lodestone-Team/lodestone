use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::traits::t_player::TPlayer;

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
