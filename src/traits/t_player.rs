use async_trait::async_trait;
use serde_json;


use crate::{traits::GameInstance, Error};

use super::ErrorInner;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TPlayerManagement: Sync + Send {
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
    async fn get_player_list(&self) -> Result<Vec<serde_json::Value>, Error> {
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
