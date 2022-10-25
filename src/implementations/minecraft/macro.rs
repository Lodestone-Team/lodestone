use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use log::error;
use mlua::Lua;
use tokio::{io::AsyncWriteExt};


use crate::{
    macro_executor::LuaExecutionInstruction,
    traits::{t_macro::TMacro, Error, ErrorInner},
    util::{list_dir, rand_macro_uuid},
};

use super::Instance;

impl Instance {
    pub fn macro_std(&self) -> Arc<dyn Fn() -> Lua + Sync + Send> {
        Arc::new({
            let stdin = self.stdin.clone();
            let path = self.config.path.clone().to_str().unwrap().to_string();
            let uuid = self.config.uuid.clone();
            let event_broadcaster = self.event_broadcaster.clone();
            let macro_sender = self.macro_executor.get_sender();
            move || {
                let lua = Lua::new();
                lua.globals()
                    .set(
                        "sleep_and_yield",
                        lua.create_async_function(|_, (secs,): (u64,)| async move {
                            tokio::time::sleep(Duration::from_secs(secs)).await;
                            Ok(())
                        })
                        .unwrap(),
                    )
                    .unwrap();
                lua.globals()
                    .set(
                        "send_stdin",
                        lua.create_async_function({
                            let stdin = stdin.clone();
                            move |_, cmd: String| {
                                let stdin = stdin.clone();
                                async move {
                                    let mut stdin = stdin.lock().await;
                                    match stdin.as_mut() {
                                        Some(stdin) => {
                                            stdin
                                                .write_all(format!("{}\n", cmd).as_bytes())
                                                .await
                                                .unwrap();
                                        }
                                        None => {
                                            error!("Failed to send stdin, stdin is not available")
                                        }
                                    }
                                    Ok(())
                                }
                            }
                        })
                        .unwrap(),
                    )
                    .unwrap();
                lua.globals()
                    .set(
                        "await_event",
                        lua.create_async_function({
                            let event_broadcaster = event_broadcaster.clone();
                            let uuid = uuid.clone();
                            move |_, event_str: String| {
                                let event_broadcaster = event_broadcaster.clone();
                                let instance_uuid = uuid.clone();
                                async move {
                                    let mut event_receiver = event_broadcaster.subscribe();
                                    while let Ok(event) = event_receiver.recv().await {
                                        match event_str.as_str() {
                                            "player_msg" => {
                                                if let Some(event_uuid) = event.get_instance_uuid()
                                                {
                                                    if instance_uuid != event_uuid {
                                                        continue;
                                                    }
                                                    if let Some((player, msg)) =
                                                        event.try_player_message()
                                                    {
                                                        return Ok((player, msg));
                                                    }
                                                }
                                            }
                                            _ => todo!(),
                                        }
                                    }
                                    panic!("Event receiver closed");
                                }
                            }
                        })
                        .unwrap(),
                    )
                    .unwrap();
                lua.globals()
                    .set(
                        "spawn",
                        lua.create_async_function({
                            let path = path.clone();
                            let macro_sender = macro_sender.clone();
                            move |_, (macro_name, args): (String, Vec<String>)| {
                                let path = path.clone();
                                let macro_sender = macro_sender.clone();
                                async move {
                                    let macro_path = format!("{}/macros/{}.lua", path, macro_name);
                                    if let Ok(macro_code) =
                                        tokio::fs::read_to_string(macro_path).await
                                    {
                                        let exec_instruction = LuaExecutionInstruction {
                                            lua: None,
                                            content: macro_code,
                                            args,
                                            executor: None,
                                        };
                                        let uuid = rand_macro_uuid();
                                        macro_sender
                                            .send((exec_instruction, uuid.clone()))
                                            .unwrap();
                                        Ok(uuid)
                                    } else {
                                        panic!("Failed to load macro");
                                    }
                                }
                            }
                        })
                        .unwrap(),
                    )
                    .unwrap();
                lua.globals().set("INSTANCE_PATH", path.clone()).unwrap();

                lua
            }
        })
    }
}

#[async_trait]
impl TMacro for Instance {
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
    ) -> crate::traits::MaybeUnsupported<Result<String, crate::traits::Error>> {
        let path = self.path_to_macros.join(name).with_extension("lua");
        let content = tokio::fs::read_to_string(&path)
            .await
            .expect("Failed to read macro");

        let exec_instruction = LuaExecutionInstruction {
            lua: None,
            content,
            args,
            executor: executor.map(|s| s.to_string()),
        };

        self.macro_executor.spawn(exec_instruction);

        // lua.load(&content).exec_async().unwrap();

        Some(Ok("".to_string()))
    }
}
