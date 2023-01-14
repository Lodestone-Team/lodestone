use async_trait::async_trait;
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
            (State::Starting, StateAction::UserStart) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot start an instance that is already starting".to_string(),
            }),
            (State::Starting, StateAction::UserStop) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot stop an instance that is starting".to_string(),
            }),
            (_, StateAction::InstanceStart) => Ok(State::Running),
            (_, StateAction::InstanceStop) => Ok(State::Stopped),
            (State::Running, StateAction::UserStart) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot start an instance that is already running".to_string(),
            }),
            (State::Running, StateAction::UserStop) => Ok(State::Stopping),
            (State::Stopping, StateAction::UserStart) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot start an instance that is stopping".to_string(),
            }),
            (State::Stopping, StateAction::UserStop) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot stop an instance that is already stopping".to_string(),
            }),
            (State::Stopped, StateAction::UserStart) => Ok(State::Starting),
            (State::Stopped, StateAction::UserStop) => Err(Error {
                inner: ErrorInner::InvalidInstanceState,
                detail: "Cannot stop an instance that is already stopped".to_string(),
            }),
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

use super::ErrorInner;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TServer {
    async fn start(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn stop(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn kill(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn state(&self) -> State;
    async fn send_command(&self, command: &str, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn monitor(&self) -> MonitorReport;
}
