use std::collections::HashMap;
pub use std::path::PathBuf;

use async_trait::async_trait;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
use serde_json::Value;

use crate::Error;
use crate::traits::GameInstance;
use crate::traits::MinecraftInstance;

use super::ErrorInner;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TConfigurable{
    // getters
    async fn uuid(&self) -> String;
    async fn name(&self) -> String;
    async fn flavour(&self) -> String;
    async fn game_type(&self) -> String;
    async fn cmd_args(&self) -> Vec<String>;
    async fn description(&self) -> String;
    async fn port(&self) -> u32;
    async fn min_ram(&self) -> Result<u32, Error>;
    async fn max_ram(&self) -> Result<u32, Error>;
    async fn creation_time(&self) -> i64;
    async fn path(&self) -> PathBuf;
    /// does start when lodestone starts
    async fn auto_start(&self) -> bool;
    async fn restart_on_crash(&self) -> bool;
    async fn backup_period(&self) -> Result<Option<u32>, Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support backing up".to_string(),
        })
    }
    async fn get_info(&self) -> Value;

    // setters
    async fn set_name(&mut self, name: String) -> Result<(), crate::traits::Error>;
    async fn set_description(&mut self, description: String) -> Result<(), crate::traits::Error>;
    async fn set_port(&mut self, _port: u32) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support setting port".to_string(),
        })
    }
    async fn set_cmd_args(
        &mut self,
        _cmd_args: Vec<String>,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support setting command line arguments".to_string(),
        })
    }
    async fn set_min_ram(
        &mut self,
        _min_ram: u32,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support setting ram".to_string(),
        })
    }
    async fn set_max_ram(
        &mut self,
        _max_ram: u32,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support setting ram".to_string(),
        })
    }
    async fn set_auto_start(
        &mut self,
        _auto_start: bool,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support auto start".to_string(),
        })
    }
    async fn set_restart_on_crash(
        &mut self,
        _restart_on_crash: bool,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support restarting on crash".to_string(),
        })
    }
    async fn set_backup_period(
        &mut self,
        _backup_period: Option<u32>,
    ) -> Result<(), crate::traits::Error> {
        Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "This instance does not support backup".to_string(),
        })
    }

    // server config files (server.properties)
    async fn set_field(&mut self, field: &str, value: String) -> Result<(), super::Error>;
    async fn get_field(&self, field: &str) -> Result<String, super::Error>;

    async fn change_version(&mut self, _version: String) -> Result<(), super::Error> {
        Err(super::Error {
            detail: "Not supported".to_string(),
            inner: super::ErrorInner::UnsupportedOperation,
        })
    }

    async fn settings(&self) -> Result<HashMap<String, String>, super::Error>;
}
