use std::{cell::RefCell, path::PathBuf, rc::Rc};

use async_trait::async_trait;
use deno_core::{
    anyhow::{self},
    op, OpState,
};

use crate::{
    events::{CausedBy, EventInner},
    macro_executor::{self, ExecutionInstruction},
    traits::{t_macro::TMacro, t_server::TServer, Error, ErrorInner},
    util::list_dir,
};

use super::MinecraftInstance;

#[op]
async fn send_stdin(state: Rc<RefCell<OpState>>, cmd: String) -> Result<(), anyhow::Error> {
    let instance = state.borrow().borrow::<MinecraftInstance>().clone();
    instance.send_command(&cmd, CausedBy::Unknown).await?;
    Ok(())
}

#[op]
async fn send_rcon(state: Rc<RefCell<OpState>>, cmd: String) -> Result<String, anyhow::Error> {
    let instance = state.borrow().borrow::<MinecraftInstance>().clone();
    let ret = instance.send_rcon(&cmd).await?;
    Ok(ret)
}

#[op]
fn config(state: Rc<RefCell<OpState>>) -> Result<String, anyhow::Error> {
    let instance = state.borrow().borrow::<MinecraftInstance>().clone();
    Ok(serde_json::to_string(&instance.config)?)
}

#[op]
async fn on_event(
    state: Rc<RefCell<OpState>>,
    event: String,
) -> Result<Option<String>, anyhow::Error> {
    let instance = state.borrow().borrow::<MinecraftInstance>().clone();
    let mut event_rx = instance.event_broadcaster.subscribe();
    if event == "playerMessage" {
        while let Ok(event) = event_rx.recv().await {
            if let EventInner::InstanceEvent(inner) = event.event_inner {
                if let crate::events::InstanceEventInner::PlayerMessage {
                    player,
                    player_message,
                } = inner.instance_event_inner
                {
                    return Ok(Some(
                        serde_json::json!(
                         {
                             "player": player,
                             "message": player_message
                         }
                        )
                        .to_string(),
                    ));
                }
            }
        }
    } else if event == "playersJoined" {
        while let Ok(event) = event_rx.recv().await {
            if let EventInner::InstanceEvent(inner) = event.event_inner {
                if let crate::events::InstanceEventInner::PlayerChange { players_joined, .. } =
                    inner.instance_event_inner
                {
                    if !players_joined.is_empty() {
                        return Ok(Some(
                            serde_json::json!({ "players": players_joined }).to_string(),
                        ));
                    }
                    continue;
                }
            }
        }
    } else if event == "playersLeft" {
        while let Ok(event) = event_rx.recv().await {
            if let EventInner::InstanceEvent(inner) = event.event_inner {
                if let crate::events::InstanceEventInner::PlayerChange { players_left, .. } =
                    inner.instance_event_inner
                {
                    if !players_left.is_empty() {
                        return Ok(Some(
                            serde_json::json!(
                             {
                                 "playersLeft": players_left,
                             }
                            )
                            .to_string(),
                        ));
                    } else {
                        continue;
                    }
                }
            }
        }
    } else if event == "playersChanged" {
        while let Ok(event) = event_rx.recv().await {
            if let EventInner::InstanceEvent(inner) = event.event_inner {
                if let crate::events::InstanceEventInner::PlayerChange { player_list, .. } =
                    inner.instance_event_inner
                {
                    return Ok(Some(
                        serde_json::json!(
                         {
                             "players": player_list,
                         }
                        )
                        .to_string(),
                    ));
                }
            }
        }
    }
    todo!()
}

impl MinecraftInstance {
    pub fn macro_std(
        &self,
    ) -> Box<
        dyn Fn(
                String,
                String,
                Vec<String>,
                bool,
            ) -> Result<(deno_runtime::worker::MainWorker, PathBuf), Error>
            + Send,
    > {
        Box::new({
            let path_to_macros = self.path_to_macros.clone();
            let instance = self.clone();
            move |macro_name: String,
                  executor: String,
                  args: Vec<String>,
                  is_in_game: bool|
                  -> Result<(deno_runtime::worker::MainWorker, PathBuf), Error> {
                let path_to_main_module = macro_executor::resolve_macro_invocation(
                    &path_to_macros,
                    &macro_name,
                    is_in_game,
                )
                .expect("Failed to resolve macro invocation");

                let bootstrap_options = deno_runtime::BootstrapOptions {
                    args,
                    ..Default::default()
                };

                let mut worker_options = deno_runtime::worker::WorkerOptions {
                    bootstrap: bootstrap_options,
                    ..Default::default()
                };

                let ext = deno_core::Extension::builder()
                    .ops(vec![
                        send_stdin::decl(),
                        send_rcon::decl(),
                        config::decl(),
                        on_event::decl(),
                    ])
                    .state({
                        let instance = instance.clone();
                        move |state| {
                            state.put(instance.clone());
                            Ok(())
                        }
                    })
                    .build();
                worker_options.extensions.push(ext);
                worker_options.module_loader = Rc::new(macro_executor::TypescriptModuleLoader);
                let main_module = deno_core::resolve_path(&path_to_main_module.to_string_lossy())
                    .map_err(|e| Error {
                    inner: ErrorInner::IOError,
                    detail: format!("Failed to resolve path: {}", e),
                })?;
                // todo(CheatCod3) : limit the permissions
                let permissions = deno_runtime::permissions::Permissions::allow_all();
                let mut worker = deno_runtime::worker::MainWorker::bootstrap_from_options(
                    main_module,
                    permissions,
                    worker_options,
                );
                let js = include_str!("js_macro/runtime.js");
                worker
                    .execute_script("[lodestone:runtime.js]", js)
                    .map_err(|e| Error {
                        inner: ErrorInner::FailedToRun,
                        detail: format!("Failed to execute runtime.js: {}", e),
                    })?;
                worker
                    .execute_script(
                        "[dep_inject]",
                        format!("const executor = \"{}\";", executor).as_str(),
                    )
                    .expect("Failed to inject executor");
                Ok((worker, path_to_main_module))
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
        is_in_game: bool,
    ) -> Result<(), crate::traits::Error> {
        let exec_instruction = ExecutionInstruction {
            name: name.to_string(),
            args,
            executor: executor.map(|s| s.to_string()),
            runtime: self.macro_std(),
            is_in_game,
        };

        self.macro_executor.spawn(exec_instruction);

        Ok(())
    }
}
