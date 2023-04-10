use async_trait::async_trait;
use color_eyre::eyre::eyre;
use std::path::PathBuf;
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    events::CausedBy,
    macro_executor::MacroPID,
    traits::GameInstance,
};

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct MacroEntry {
    pub name: String,
    pub last_run: Option<i64>,
    // relative path to instance root
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct TaskEntry {
    pub name: String,
    pub creation_time: i64,
    pub pid: MacroPID,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct HistoryEntry {
    pub task: TaskEntry,
    pub exit_status: ExitStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ExitStatus {
    Success { time: i64 },
    Killed { time: i64 },
    Error { time: i64, error_msg: String },
}

impl ExitStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, ExitStatus::Success { .. })
    }
}

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TMacro {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error>;
    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error>;
    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error>;
    async fn delete_macro(&mut self, name: &str) -> Result<(), Error>;
    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), Error>;
    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _caused_by: CausedBy,
        _is_in_game: bool,
    ) -> Result<TaskEntry, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support running macro"),
        })
    }
    async fn kill_macro(&mut self, _pid: MacroPID) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support killing macro"),
        })
    }
}
