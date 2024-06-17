use async_trait::async_trait;
use bollard::secret::ContainerState;
use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};

use ts_rs::TS;

use crate::events::CausedBy;
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, Copy)]
#[serde(rename = "InstanceState")]
#[ts(export)]
pub enum State {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl State {
    pub fn from_docker_state_string(state: &str) -> Self {
        match state {
            "running" => State::Running,
            "exited" => State::Stopped,
            "paused" => State::Stopped,
            "restarting" => State::Starting,
            "dead" => State::Error,
            _ => State::Error,
        }
    }
    pub fn from_container_state(state: &ContainerState) -> Self {
        state.status.map_or(State::Error, |status| {
            State::from_docker_state_string(status.as_ref())
        })
    }
}

pub enum StateAction {
    UserStart,
    UserStop,
    InstanceStart,
    InstanceStop,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DiskUsage {
    pub total_written_bytes: u64,
    pub written_bytes: u64,
    pub total_read_bytes: u64,
    pub read_bytes: u64,
}

impl From<sysinfo::DiskUsage> for DiskUsage {
    fn from(du: sysinfo::DiskUsage) -> Self {
        Self {
            total_written_bytes: du.total_written_bytes,
            written_bytes: du.written_bytes,
            total_read_bytes: du.total_read_bytes,
            read_bytes: du.read_bytes,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename = "PerformanceReport")]
#[ts(export)]
pub struct MonitorReport {
    pub memory_usage: Option<u64>,
    pub disk_usage: Option<DiskUsage>,
    pub cpu_usage: Option<f32>,
    pub start_time: Option<u64>,
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

impl State {
    pub fn try_new_state(
        &self,
        action: StateAction,
        on_transit: Option<&dyn Fn(State)>,
    ) -> Result<State, Error> {
        let state = match (*self, action) {
            (State::Starting, StateAction::UserStart) => {
                Err(eyre!("Cannot start an instance that is already starting"))
            }
            (State::Starting, StateAction::UserStop) => {
                Err(eyre!("Cannot stop an instance that is starting"))
            }
            (_, StateAction::InstanceStart) => Ok(State::Running),
            (_, StateAction::InstanceStop) => Ok(State::Stopped),
            (State::Running, StateAction::UserStart) => {
                Err(eyre!("Cannot start an instance that is already running"))
            }
            (State::Running, StateAction::UserStop) => Ok(State::Stopping),
            (State::Stopping, StateAction::UserStart) => {
                Err(eyre!("Cannot start an instance that is stopping"))
            }
            (State::Stopping, StateAction::UserStop) => {
                Err(eyre!("Cannot stop an instance that is already stopping"))
            }
            (State::Stopped, StateAction::UserStart) => Ok(State::Starting),
            (State::Stopped, StateAction::UserStop) => {
                Err(eyre!("Cannot stop an instance that is already stopped"))
            }
            (State::Error, StateAction::UserStart) => todo!(),
            (State::Error, StateAction::UserStop) => todo!(),
        }?;
        if let Some(on_transit) = on_transit {
            on_transit(state);
        }
        Ok(state)
    }

    pub fn try_transition(
        &mut self,
        action: StateAction,
        on_transit: Option<&dyn Fn(State)>,
    ) -> Result<(), Error> {
        let new_state = self.try_new_state(action, on_transit)?;
        *self = new_state;
        Ok(())
    }
}

use crate::traits::GameInstance;

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TServer {
    async fn start(&self, caused_by: CausedBy, block: bool) -> Result<(), Error>;
    async fn stop(&self, caused_by: CausedBy, block: bool) -> Result<(), Error>;
    async fn restart(&self, caused_by: CausedBy, block: bool) -> Result<(), Error>;
    async fn kill(&self, caused_by: CausedBy) -> Result<(), Error>;
    async fn state(&self) -> State;
    async fn send_command(&self, command: &str, caused_by: CausedBy) -> Result<(), Error>;
    async fn monitor(&self) -> MonitorReport;
}
