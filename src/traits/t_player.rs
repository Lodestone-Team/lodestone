use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ErrorInner;
use crate::minecraft::player::MinecraftPlayer;
use crate::{traits::GameInstance, Error};

#[enum_dispatch::enum_dispatch]
pub trait TPlayer {
    fn get_id(&self) -> String;
    fn get_name(&self) -> String;
}

#[enum_dispatch::enum_dispatch(TPlayer)]
#[derive(Serialize, Deserialize, Debug, Eq, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum Player {
    MinecraftPlayer,
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.get_id() == other.get_id()
    }
}
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
impl Hash for Player {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get_id().hash(state);
    }
}

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TPlayerManagement {
    async fn get_player_count(&self) -> Result<u32, Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "Getting player count is unsupported for this instance".to_string(),
        })
    }
    async fn get_max_player_count(&self) -> Result<u32, Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "Getting max player count is unsupported for this instance".to_string(),
        })
    }
    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "Getting player list is unsupported for this instance".to_string(),
        })
    }

    async fn set_max_player_count(&mut self, _max_player_count: u32) -> Result<(), Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "Setting max player count is unsupported for this instance".to_string(),
        })
    }
}
