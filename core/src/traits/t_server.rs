use async_trait::async_trait;
use bollard::models::ContainerState;
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
            // newer Docker engines report this while a container shuts down
            "stopping" => State::Stopping,
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
                Err(eyre!("Cannot start a server that is already starting"))
            }
            // a server that never finishes starting must still be stoppable
            (State::Starting, StateAction::UserStop) => Ok(State::Stopping),
            (_, StateAction::InstanceStart) => Ok(State::Running),
            (_, StateAction::InstanceStop) => Ok(State::Stopped),
            (State::Running, StateAction::UserStart) => {
                Err(eyre!("Cannot start a server that is already running"))
            }
            (State::Running, StateAction::UserStop) => Ok(State::Stopping),
            (State::Stopping, StateAction::UserStart) => {
                Err(eyre!("Cannot start a server that is stopping"))
            }
            (State::Stopping, StateAction::UserStop) => {
                Err(eyre!("Cannot stop a server that is already stopping"))
            }
            (State::Stopped, StateAction::UserStart) => Ok(State::Starting),
            (State::Stopped, StateAction::UserStop) => {
                Err(eyre!("Cannot stop a server that is already stopped"))
            }
            // let the user retry after an error
            (State::Error, StateAction::UserStart) => Ok(State::Starting),
            (State::Error, StateAction::UserStop) => Err(eyre!(
                "Cannot stop a server that is in an error state, kill it instead"
            )),
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

#[cfg(test)]
mod tests {
    use super::{State, StateAction};

    fn next(state: State, action: StateAction) -> Option<State> {
        state.try_new_state(action, None).ok()
    }

    #[test]
    fn state_transitions() {
        use State::*;
        use StateAction::*;
        assert_eq!(next(Stopped, UserStart), Some(Starting));
        assert_eq!(next(Starting, UserStart), None);
        assert_eq!(next(Starting, UserStop), Some(Stopping));
        assert_eq!(next(Running, UserStop), Some(Stopping));
        assert_eq!(next(Running, UserStart), None);
        assert_eq!(next(Stopping, UserStart), None);
        assert_eq!(next(Stopping, UserStop), None);
        assert_eq!(next(Stopped, UserStop), None);
        assert_eq!(next(Error, UserStart), Some(Starting));
        assert_eq!(next(Error, UserStop), None);
        for state in [Starting, Running, Stopping, Stopped, Error] {
            assert_eq!(next(state, InstanceStart), Some(Running));
            assert_eq!(next(state, InstanceStop), Some(Stopped));
        }
    }

    #[test]
    fn on_transit_runs_only_on_success() {
        let called = std::cell::Cell::new(None);
        let record = |s: State| called.set(Some(s));
        let mut state = State::Stopped;
        assert!(state
            .try_transition(StateAction::UserStop, Some(&record))
            .is_err());
        assert_eq!(called.get(), None);
        state
            .try_transition(StateAction::UserStart, Some(&record))
            .unwrap();
        assert_eq!(state, State::Starting);
        assert_eq!(called.get(), Some(State::Starting));
    }
}
