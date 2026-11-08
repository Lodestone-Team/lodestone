use async_trait::async_trait;
use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::minecraft::player::MinecraftPlayer;
use crate::traits::GameInstance;
#[enum_dispatch::enum_dispatch]
pub trait TPlayer {
    fn get_id(&self) -> String;
    fn get_name(&self) -> String;
}

// The player of an atom (generic) instance. Here rather than with the atom implementation,
// which needs the `deno` feature, because `Player` is part of the API either way.
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

#[enum_dispatch::enum_dispatch(TPlayer)]
#[derive(Serialize, Deserialize, Debug, Eq, TS, Clone)]
#[serde(tag = "type")]
#[ts(export)]
pub enum Player {
    MinecraftPlayer,
    GenericPlayer,
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
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Getting player count is unsupported for this instance"),
        })
    }
    async fn get_max_player_count(&self) -> Result<u32, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Getting max player count is unsupported for this instance"),
        })
    }
    async fn get_player_list(&self) -> Result<HashSet<Player>, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Getting player list is unsupported for this instance"),
        })
    }

    async fn set_max_player_count(&self, _max_player_count: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Setting max player count is unsupported for this instance"),
        })
    }
}
