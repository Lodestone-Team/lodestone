use crate::{
    error::Error,
    events::CausedBy,
    traits::t_server::{State, TServer, MonitorReport},
};

use super::{
    bridge::procedure_call::{ProcedureCallInner, ProcedureCallResultInner},
    GenericInstance,
};

#[async_trait::async_trait]
impl TServer for GenericInstance {
    async fn start(&mut self, caused_by: CausedBy, _block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::StartInstance { caused_by })
            .await?;
        Ok(())
    }
    async fn stop(&mut self, caused_by: CausedBy, _block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::StopInstance { caused_by })
            .await?;
        Ok(())
    }
    async fn restart(&mut self, caused_by: CausedBy, _block: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::RestartInstance { caused_by })
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
        if let ProcedureCallResultInner::State(state) = self
            .procedure_bridge
            .call(ProcedureCallInner::GetState)
            .await
            .unwrap()
        {
            state
        } else {
            unreachable!()
        }
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
        if let ProcedureCallResultInner::Monitor(report) = self
            .procedure_bridge
            .call(ProcedureCallInner::Monitor)
            .await
            .unwrap()
        {
            report
        } else {
            unreachable!()
        }
        
    }
}
