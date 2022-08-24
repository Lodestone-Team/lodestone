use serde::{Deserialize, Serialize};

use super::MaybeUnsupported;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

pub trait TServer {
    fn start(&mut self) -> Result<(), super::Error>;
    fn stop(&mut self) -> Result<(), super::Error>;
    fn kill(&mut self) -> Result<(), super::Error>;
    fn state(&self) -> State;
    fn send_command(&mut self, command: &str) -> MaybeUnsupported<Result<(), super::Error>>;
}
