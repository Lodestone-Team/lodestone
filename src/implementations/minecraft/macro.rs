use std::{
    borrow::BorrowMut, cell::RefCell, path::PathBuf, rc::Rc, sync::Arc, thread, time::Duration,
};

use async_trait::async_trait;
use deno_core::{op, JsRuntime, OpState, Resource};
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
async fn send_stdin(state: Rc<RefCell<OpState>>, cmd: String) {
    let mut instance = state.borrow().resource_table.get::<MinecraftInstance>(0).unwrap();
    instance
        .stdin
        .lock()
        .await
        .as_mut()
        .unwrap()
        .write_all(cmd.as_bytes())
        .await
        .unwrap();
}

struct RuntimeState {
    pub stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    pub path: PathBuf,
    pub uuid: String,
    pub rcon_conn: Arc<Mutex<Option<rcon::Connection<tokio::net::TcpStream>>>>,
}

impl MinecraftInstance {
    pub fn macro_std(&self) -> Arc<dyn Fn() -> JsRuntime> {
        Arc::new({
            let a = self.clone();
            move || {
                let ext = deno_core::Extension::builder()
                    .ops(vec![
                        // An op for summing an array of numbers
                        // The op-layer automatically deserializes inputs
                        // and serializes the returned Result & value
                        send_stdin::decl(),
                    ])
                    .build();
                let mut op_state = OpState::new(32);
                op_state.resource_table.add(a.clone());
                ext.init_state(&mut op_state).unwrap();
                let runtime = JsRuntime::new(deno_core::RuntimeOptions {
                    extensions: vec![ext],
                    ..Default::default()
                });
                runtime
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
        let path = self.path_to_macros.join(name).with_extension("lua");
        let content = tokio::fs::read_to_string(&path)
            .await
            .expect("Failed to read macro");

        let exec_instruction = ExecutionInstruction {
            content,
            args,
            executor: executor.map(|s| s.to_string()),
            runtime: self.macro_std(),
        };

        self.macro_executor.spawn(exec_instruction);

        // lua.load(&content).exec_async().unwrap();

        Ok(())
    }
}
