use crate::{
    error::Error,
    events::CausedBy,
    traits::t_server::{MonitorReport, State, TServer},
};

use super::{bridge::procedure_call::ProcedureCallInner, GenericInstance};

#[async_trait::async_trait]
impl TServer for GenericInstance {
    async fn start(&mut self, caused_by: CausedBy, block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::StartInstance { caused_by, block })
            .await?;
        Ok(())
    }
    async fn stop(&mut self, caused_by: CausedBy, block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::StopInstance { caused_by, block })
            .await?;
        Ok(())
    }
    async fn restart(&mut self, caused_by: CausedBy, block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::RestartInstance { caused_by, block })
            .await?;
        Ok(())
    }
    async fn kill(&mut self, caused_by: CausedBy) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::KillInstance { caused_by })
            .await?;
        Ok(())
    }
    async fn state(&self) -> State {
        self.procedure_bridge
            .call(ProcedureCallInner::GetState)
            .await
            .map_or(State::Stopped, |r| r.try_into().unwrap_or(State::Stopped))
    }
    async fn send_command(&self, command: &str, caused_by: CausedBy) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SendCommand {
                command: command.to_string(),
                caused_by,
            })
            .await?;
        Ok(())
    }
    async fn monitor(&self) -> MonitorReport {
        self.procedure_bridge
            .call(ProcedureCallInner::Monitor)
            .await
            .map_or(MonitorReport::default(), |r| {
                r.try_into().unwrap_or_default()
            })
    }
}
