use std::collections::HashMap;
pub use std::path::PathBuf;

use async_trait::async_trait;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
use serde_json::Value;

use crate::traits::{MaybeUnsupported, Unsupported};

#[async_trait]
pub trait TConfigurable : Sync + Send {
    // getters
    async fn uuid(&self) -> String;
    async fn name(&self) -> String;
    async fn flavour(&self) -> String;
    async fn game_type(&self) -> String;
    async fn cmd_args(&self) -> Vec<String>;
    async fn description(&self) -> String;
    async fn port(&self) -> u32;
    async fn min_ram(&self) -> MaybeUnsupported<u32>;
    async fn max_ram(&self) -> MaybeUnsupported<u32>;
    async fn creation_time(&self) -> i64;
    async fn path(&self) -> PathBuf;
    /// does start when lodestone starts
    async fn auto_start(&self) -> bool;
    async fn restart_on_crash(&self) -> MaybeUnsupported<bool>   {
        Unsupported
    }
    async fn timeout_last_left(&self) -> MaybeUnsupported<Option<u32>>  {
        Unsupported
    }
    async fn timeout_no_activity(&self) -> MaybeUnsupported<Option<u32>>  {
        Unsupported
    }
    async fn start_on_connection(&self) -> MaybeUnsupported<bool>  {
        Unsupported
    }
    async fn backup_period(&self) -> MaybeUnsupported<Option<u32>>  {
        Unsupported
    }
    async fn get_info(&self) -> Value;

    // setters
    async fn set_name(&mut self, name: String) -> Result<(), crate::traits::Error>;
    async fn set_description(&mut self, description: String) -> Result<(), crate::traits::Error> ;
    async fn set_port(&mut self, _port: u32) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_cmd_argss(
        &mut self,
        _cmd_argss: Vec<String>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_min_ram(&mut self, _min_ram: u32) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_max_ram(&mut self, _max_ram: u32) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_auto_start(
        &mut self,
        _auto_start: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_restart_on_crash(
        &mut self,
        _restart_on_crash: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_timeout_last_left(
        &mut self,
        _timeout_last_left: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_timeout_no_activity(
        &mut self,
        _timeout_no_activity: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_start_on_connection(
        &mut self,
        _start_on_connection: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }
    async fn set_backup_period(
        &mut self,
        _backup_period: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>>  {
        Unsupported
    }

    // server config files (server.properties)
    async fn set_field(&mut self, field: &str, value: String) -> Result<(), super::Error>;
    async fn get_field(&self, field: &str) -> Result<String, super::Error>;

    async fn settings(&self) -> Result<HashMap<String, String>, super::Error>;
}
