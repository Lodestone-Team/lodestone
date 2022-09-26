pub use std::path::PathBuf;

pub use serde_json;
pub use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::traits::{MaybeUnsupported, Unsupported};

pub trait TConfigurable {
    // getters
    fn uuid(&self) -> String;
    fn name(&self) -> String;
    fn flavour(&self) -> String;
    fn game_type(&self) -> String;
    fn cmd_args(&self) -> Vec<String>;
    fn description(&self) -> String;
    fn port(&self) -> u32;
    fn min_ram(&self) -> MaybeUnsupported<u32>;
    fn max_ram(&self) -> MaybeUnsupported<u32>;
    fn creation_time(&self) -> i64;
    fn path(&self) -> PathBuf;
    /// does start when lodestone starts
    fn auto_start(&self) -> bool;
    fn restart_on_crash(&self) -> MaybeUnsupported<bool> {
        Unsupported
    }
    fn timeout_last_left(&self) -> MaybeUnsupported<Option<u32>> {
        Unsupported
    }
    fn timeout_no_activity(&self) -> MaybeUnsupported<Option<u32>> {
        Unsupported
    }
    fn start_on_connection(&self) -> MaybeUnsupported<bool> {
        Unsupported
    }
    fn backup_period(&self) -> MaybeUnsupported<Option<u32>> {
        Unsupported
    }
    fn get_flavours(&self) -> Vec<String> {
        vec![]
    }
    fn get_info(&self) -> Value;

    // setters
    fn set_name(&mut self, name: String) -> Result<(), crate::traits::Error>;
    fn set_description(&mut self, description: String) -> Result<(), crate::traits::Error>;
    fn set_jvm_args(
        &mut self,
        _jvm_args: Vec<String>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_min_ram(&mut self, _min_ram: u32) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_max_ram(&mut self, _max_ram: u32) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_auto_start(
        &mut self,
        _auto_start: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_restart_on_crash(
        &mut self,
        _restart_on_crash: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_timeout_last_left(
        &mut self,
        _timeout_last_left: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_timeout_no_activity(
        &mut self,
        _timeout_no_activity: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_start_on_connection(
        &mut self,
        _start_on_connection: bool,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }
    fn set_backup_period(
        &mut self,
        _backup_period: Option<u32>,
    ) -> MaybeUnsupported<Result<(), crate::traits::Error>> {
        Unsupported
    }

    // server config files (server.properties)
    fn set_field(&mut self, field: &str, value: String) -> Result<(), super::Error>;
    fn get_field(&self, field: &str) -> Result<String, super::Error>;

    fn setup_params(&self) -> serde_json::Value;
}
