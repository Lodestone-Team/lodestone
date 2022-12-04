use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    thread,
    time::Duration,
};

use async_trait::async_trait;
use deno_core::{
    anyhow::{self, anyhow},
    op, JsRuntime, OpState, Resource,
};
use deno_runtime::worker::MainWorker;
use log::error;
use mlua::Lua;

use tokio::{io::AsyncWriteExt, sync::Mutex, task::yield_now};

use crate::{
    macro_executor::ExecutionInstruction,
    traits::{t_macro::TMacro, t_server::TServer, Error, ErrorInner},
    util::{list_dir, scoped_join_win_safe},
};

use super::MinecraftInstance;

impl Resource for MinecraftInstance {}

#[op]
async fn send_stdin(state: Rc<RefCell<OpState>>, cmd: String) -> Result<(), anyhow::Error> {
    let state = state.borrow();
    let instance = state.borrow::<MinecraftInstance>().clone();
    instance
        .stdin
        .lock()
        .await
        .as_mut()
        .ok_or_else(|| anyhow!("Failed to lock stdin"))?
        .write_all(format!("{}\n", cmd).as_bytes())
        .await?;
    Ok(())
}

#[op]
async fn send_rcon(state: Rc<RefCell<OpState>>, cmd: String) -> Result<(), anyhow::Error> {
    let state = state.borrow();
    let instance = state.borrow::<MinecraftInstance>().clone();
    instance
        .rcon_conn
        .lock()
        .await
        .as_mut()
        .ok_or_else(|| anyhow!("Failed to lock stdin"))?
        .cmd(&cmd)
        .await?;
    Ok(())
}

impl MinecraftInstance {
    pub fn macro_std(&self) -> Box<dyn Fn(&Path) -> deno_runtime::worker::MainWorker + Send> {
        Box::new({
            let a = self.clone();
            move |js_path| {
                let a = a.clone();
                let mut options = deno_runtime::worker::WorkerOptions::default();
                let ext = deno_core::Extension::builder()
                    .ops(vec![send_stdin::decl(), send_rcon::decl()])
                    .state(move |state| {
                        state.put(a.clone());
                        Ok(())
                    })
                    .build();
                options.extensions.push(ext);
                let main_module = deno_core::resolve_path(&js_path.to_string_lossy()).unwrap();
                let permissions = deno_runtime::permissions::Permissions::allow_all();
                let worker = deno_runtime::worker::MainWorker::bootstrap_from_options(
                    main_module.clone(),
                    permissions,
                    options,
                );

                worker
            }
        })
    }
}

#[async_trait]
impl TMacro for MinecraftInstance {
    async fn get_macro_list(&self) -> Vec<String> {
        list_dir(&self.path_to_macros, Some(true))
            .await
            .expect("Failed to list macros")
            .iter()
            .map(|s| s.file_name().unwrap().to_str().unwrap().to_string())
            .collect()
    }

    async fn delete_macro(&mut self, name: &str) -> Result<(), crate::traits::Error> {
        tokio::fs::remove_file(self.path_to_macros.join(name))
            .await
            .map_err(|e| Error {
                inner: ErrorInner::FailedToRemoveFileOrDir,
                detail: format!("Failed to delete macro {}, {}", name, e),
            })?;
        Ok(())
    }

    async fn create_macro(
        &mut self,
        name: &str,
        content: &str,
    ) -> Result<(), crate::traits::Error> {
        // if macro already exists, return error
        if self.get_macro_list().await.contains(&name.to_string()) {
            return Err(Error {
                inner: ErrorInner::FiledOrDirAlreadyExists,
                detail: format!("Macro {} already exists", name),
            });
        }
        tokio::fs::write(self.path_to_macros.join(name), content)
            .await
            .map_err(|e| Error {
                inner: ErrorInner::FailedToUpload,
                detail: format!("Failed to create macro {}, {}", name, e),
            })?;
        Ok(())
    }

    async fn run_macro(
        &mut self,
        name: &str,
        args: Vec<String>,
        executor: Option<&str>,
    ) -> Result<(), crate::traits::Error> {
        let path_to_main_module = self.path_to_macros.join(name).with_extension("js");

        let exec_instruction = ExecutionInstruction {
            path_to_main_module,
            args,
            executor: executor.map(|s| s.to_string()),
            runtime: self.macro_std(),
        };

        self.macro_executor.spawn(exec_instruction);

        // lua.load(&content).exec_async().unwrap();

        Ok(())
    }
}
