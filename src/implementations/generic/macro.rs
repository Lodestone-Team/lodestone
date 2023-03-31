use std::rc::Rc;

use async_trait::async_trait;

use crate::error::Error;
use crate::events::CausedBy;
use crate::macro_executor::{self, MainWorkerGenerator};
use crate::traits::t_macro::{TMacro, MacroEntry, TaskEntry, HistoryEntry};

use super::bridge::procedure_call::{
    emit_console_out, emit_result, on_procedure, proc_bridge_ready, ProcedureBridge,
};
use super::GenericInstance;

pub struct GenericMainWorkerGenerator {
    bridge: ProcedureBridge,
    instance: GenericInstance,
}

impl GenericMainWorkerGenerator {
    pub fn new(bridge: ProcedureBridge, instance: GenericInstance) -> Self {
        Self { bridge, instance }
    }
}

impl MainWorkerGenerator for GenericMainWorkerGenerator {
    fn generate(
        &self,
        args: Vec<String>,
        _caused_by: CausedBy,
    ) -> deno_runtime::worker::MainWorker {
        let bootstrap_options = deno_runtime::BootstrapOptions {
            args,
            ..Default::default()
        };

        let mut worker_options = deno_runtime::worker::WorkerOptions {
            bootstrap: bootstrap_options,
            ..Default::default()
        };

        let ext = deno_core::Extension::builder("generic_deno_extension_builder")
            .ops(vec![
                on_procedure::decl(),
                emit_result::decl(),
                proc_bridge_ready::decl(),
                emit_console_out::decl(),
            ])
            .state({
                let brige = self.bridge.clone();
                let instance = self.instance.clone();
                move |state| {
                    state.put(brige.clone());
                    state.put(instance);
                }
            })
            .force_op_registration()
            .build();
        worker_options.extensions.push(ext);
        worker_options.module_loader = Rc::new(macro_executor::TypescriptModuleLoader::default());

        let main_module = deno_core::resolve_path(".", &std::env::current_dir().unwrap())
            .expect("Failed to resolve path");
        // todo(CheatCod3) : limit the permissions
        let permissions = deno_runtime::permissions::Permissions::allow_all();
        deno_runtime::worker::MainWorker::bootstrap_from_options(
            main_module,
            deno_runtime::permissions::PermissionsContainer::new(permissions),
            worker_options,
        )
    }
}

#[async_trait]
impl TMacro for GenericInstance {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error> {
        unimplemented!()
    }
    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error> {
        unimplemented!()
    }
    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error> {
        unimplemented!()
    }
    async fn delete_macro(&mut self, _name: &str) -> Result<(), Error> {
        unimplemented!()
    }
    async fn create_macro(&mut self, _name: &str, _content: &str) -> Result<(), Error> {
        unimplemented!()
    }
    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _caused_by: CausedBy,
        _is_in_game: bool,
    ) -> Result<TaskEntry, Error> {
        unimplemented!()
    }
}
