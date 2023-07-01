use std::rc::Rc;

use async_trait::async_trait;
use color_eyre::eyre::eyre;

use crate::error::{Error, ErrorKind};
use crate::events::CausedBy;
use crate::macro_executor::{self, WorkerOptionGenerator};
use crate::traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry};

use super::bridge::procedure_call::{
    emit_result, next_procedure, proc_bridge_ready, ProcedureBridge,
};
use super::GenericInstance;

pub struct GenericMainWorkerGenerator {
    bridge: ProcedureBridge,
}

impl GenericMainWorkerGenerator {
    pub fn new(bridge: ProcedureBridge) -> Self {
        Self { bridge }
    }
}

impl WorkerOptionGenerator for GenericMainWorkerGenerator {
    fn generate(&self) -> deno_runtime::worker::WorkerOptions {
        let ext = deno_core::Extension::builder("generic_deno_extension_builder")
            .ops(vec![
                next_procedure::decl(),
                emit_result::decl(),
                proc_bridge_ready::decl(),
            ])
            .state({
                let brige = self.bridge.clone();
                move |state| {
                    state.put(brige);
                }
            })
            .build();
        deno_runtime::worker::WorkerOptions {
            extensions: vec![ext],
            module_loader: Rc::new(macro_executor::TypescriptModuleLoader::default()),
            ..Default::default()
        }
    }
}

#[async_trait]
impl TMacro for GenericInstance {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error> {
        Ok(Vec::new())
    }
    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error> {
        Ok(Vec::new())
    }
    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error> {
        Ok(Vec::new())
    }
    async fn delete_macro(&self, _name: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn create_macro(&self, _name: &str, _content: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn run_macro(
        &self,
        _name: &str,
        _args: Vec<String>,
        _caused_by: CausedBy,
    ) -> Result<TaskEntry, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Generic macro is not supported"),
        })
    }
}
