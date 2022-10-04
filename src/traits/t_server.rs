use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::MaybeUnsupported;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum State {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl ToString for State {
    fn to_string(&self) -> String {
        match self {
            State::Starting => "Starting".to_string(),
            State::Running => "Running".to_string(),
            State::Stopping => "Stopping".to_string(),
            State::Stopped => "Stopped".to_string(),
            State::Error => "Error".to_string(),
        }
    }
}

#[async_trait]
pub trait TServer {
    async fn start(&mut self) -> Result<(), super::Error>;
    async fn stop(&mut self) -> Result<(), super::Error>;
    async fn kill(&mut self) -> Result<(), super::Error>;
    async fn state(&self) -> State;
    async fn send_command(&mut self, command: &str) -> MaybeUnsupported<Result<(), super::Error>>;
}
