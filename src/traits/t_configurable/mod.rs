pub mod manifest;

use std::collections::BTreeMap;
use std::collections::HashMap;
pub use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
use ts_rs::TS;

use crate::error::Error;
use crate::error::ErrorKind;
use crate::traits::GameInstance;
use crate::traits::MinecraftInstance;
use crate::types::InstanceUuid;

use self::manifest::ConfigurableManifest;
use self::manifest::ConfigurableValue;

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TConfigurable {
    // getters
    async fn uuid(&self) -> InstanceUuid;
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
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support backup period"),
        })
    }
    // setters
    async fn set_name(&mut self, name: String) -> Result<(), Error>;
    async fn set_description(&mut self, description: String) -> Result<(), Error>;
    async fn set_port(&mut self, _port: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting port"),
        })
    }
    async fn set_cmd_args(&mut self, _cmd_args: Vec<String>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting cmd args"),
        })
    }
    async fn set_min_ram(&mut self, _min_ram: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting ram"),
        })
    }
    async fn set_max_ram(&mut self, _max_ram: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting ram"),
        })
    }
    async fn set_auto_start(&mut self, _auto_start: bool) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting auto start"),
        })
    }
    async fn set_restart_on_crash(&mut self, _restart_on_crash: bool) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting restart on crash"),
        })
    }
    async fn set_backup_period(&mut self, _backup_period: Option<u32>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting backup period"),
        })
    }

    // server config files (server.properties)
    async fn set_field(&mut self, field: &str, value: String) -> Result<(), Error>;
    async fn get_field(&self, field: &str) -> Result<String, Error>;

    async fn change_version(&mut self, _version: String) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support changing version"),
        })
    }

    async fn settings(&self) -> Result<HashMap<String, String>, Error>;

    async fn configurable_manifest(&self) -> ConfigurableManifest;

    async fn update_configurable(
        &mut self,
        section_id: &str,
        setting_id: &str,
        value: ConfigurableValue,
    ) -> Result<(), Error>;
}
