use async_trait::async_trait;
use color_eyre::eyre::eyre;
use std::path::PathBuf;

use crate::{
    error::{Error, ErrorKind},
    events::CausedBy,
    macro_executor::MacroPID,
    traits::GameInstance,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MacroEntry {
    pub name: String,
    pub last_run: Option<i64>,
    // relative path to instance root
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskEntry {
    pub name: String,
    pub creation_time: i64,
    pub pid: MacroPID,
}

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TMacro {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error>;
    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error>;
    async fn delete_macro(&mut self, name: &str) -> Result<(), Error>;
    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), Error>;
    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _caused_by: CausedBy,
        _is_in_game: bool,
    ) -> Result<MacroPID, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support running macro"),
        })
    }
}
