use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ts_rs::TS;

use crate::events::CausedBy;

use super::MaybeUnsupported;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename = "InstanceState")]
#[ts(export)]
pub enum State {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
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

use crate::traits::GameInstance;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TServer {
    async fn start(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn stop(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn kill(&mut self, caused_by: CausedBy) -> Result<(), super::Error>;
    async fn state(&self) -> State;
    async fn send_command(
        &mut self,
        command: &str,
        caused_by: CausedBy,
    ) -> MaybeUnsupported<Result<(), super::Error>>;
    async fn monitor(&self) -> MonitorReport;
}
