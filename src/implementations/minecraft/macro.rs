use std::{sync::Arc, thread, time::Duration};

use async_trait::async_trait;
use log::error;
use mlua::Lua;

use tokio::{io::AsyncWriteExt, task::yield_now};

use crate::{
    macro_executor::LuaExecutionInstruction,
    traits::{t_macro::TMacro, Error, ErrorInner},
    util::{list_dir, scoped_join_win_safe},
};

use super::Instance;

impl Instance {
    pub fn macro_std(&self) -> Arc<dyn Fn() -> Lua + Sync + Send> {
        Arc::new({
            let stdin = self.stdin.clone();
            let path = self.config.path.clone().to_str().unwrap().to_string();
            let uuid = self.config.uuid.clone();
            let event_broadcaster = self.event_broadcaster.clone();
s            // dont use the macro executor, use the sender and the event broadcaster
            let macro_executor = self.macro_executor.clone();
            move || {
                let lua = Lua::new();
                let await_event = lua
                    .create_async_function({
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
                                            if let Some(event_uuid) = event.get_instance_uuid() {
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
                    .unwrap();
                let sleep_and_yield = lua
                    .create_async_function(|_, (secs,): (f64,)| async move {
                        tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                        Ok(())
                    })
                    .unwrap();
                let yield_now = lua
                    .create_async_function(|_, ()| async move {
                        yield_now().await;
                        Ok(())
                    })
                    .unwrap();
                let sleep_and_block = lua
                    .create_async_function(|_, (secs,): (f64,)| async move {
                        thread::sleep(Duration::from_secs_f64(secs));
                        Ok(())
                    })
                    .unwrap();
                let send_stdin = lua
                    .create_async_function({
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
                    .unwrap();
                let log_info = lua.create_async_function({
                    let stdin = stdin.clone();
                    move |_, msg: String| {
                        let stdin = stdin.clone();
                        async move {
                            let mut stdin = stdin.lock().await;
                            match stdin.as_mut() {
                                Some(stdin) => {
                                    stdin
                                        .write_all(format!(
                                            "tellraw @a [\"\",{{\"text\":\"[Info] \",\"color\":\"green\"}},{{\"text\":\"{}\"}}]\n",
                                            msg
                                        ).as_bytes())
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
                }).unwrap();
                let log_warn = lua.create_async_function({
                    let stdin = stdin.clone();
                    move |_, msg: String| {
                        let stdin = stdin.clone();
                        async move {
                            let mut stdin = stdin.lock().await;
                            match stdin.as_mut() {
                                Some(stdin) => {
                                    stdin
                                        .write_all( format!(
                                            "tellraw @a [\"\",{{\"text\":\"[Warn] \",\"color\":\"yellow\"}},{{\"text\":\"{}\"}}]\n",
                                            msg
                                        ).as_bytes())
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
                .unwrap();
                let log_err = lua.create_async_function({
                    let stdin = stdin.clone();
                    move |_, msg: String| {
                        let stdin = stdin.clone();
                        async move {
                            let mut stdin = stdin.lock().await;
                            match stdin.as_mut() {
                                Some(stdin) => {
                                    stdin
                                        .write_all(format!(
                                            "tellraw @a [\"\",{{\"text\":\"[Error] \",\"color\":\"red\"}},{{\"text\":\"{}\"}}]\n",
                                            msg
                                        )
                                        .as_bytes())
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
                .unwrap();
                let spawn_task = lua
                    .create_async_function({
                        let path = path.clone();
                        let macro_executor = macro_executor.clone();
                        move |_, (macro_name, args): (String, Vec<String>)| {
                            let path = path.clone();
                            let macro_executor = macro_executor.clone();
                            async move {
                                let macro_path =
                                    scoped_join_win_safe(path, format!("macros/{}", macro_name))
                                        .unwrap()
                                        .with_extension("lua");
                                if let Ok(macro_code) = tokio::fs::read_to_string(macro_path).await
                                {
                                    let exec_instruction = LuaExecutionInstruction {
                                        lua: None,
                                        content: macro_code,
                                        args,
                                        // todo: figure out how to inherit the executor
                                        executor: None,
                                    };
                                    Ok(macro_executor.spawn(exec_instruction))
                                } else {
                                    panic!("Failed to load macro");
                                }
                            }
                        }
                    })
                    .unwrap();
                let abort_task = lua
                    .create_async_function({
                        let macro_executor = macro_executor.clone();
                        move |_, uuid: String| {
                            let macro_executor = macro_executor.clone();
                            async move { Ok(macro_executor.abort_macro(&uuid).await.is_ok()) }
                        }
                    })
                    .unwrap();
                let await_task = lua
                    .create_async_function({
                        let macro_executor = macro_executor.clone();
                        move |_, (macro_uuid, timeout): (String, Option<f64>)| {
                            let macro_executor = macro_executor.clone();
                            async move {
                                Ok(macro_executor
                                    .wait_with_timeout(macro_uuid, timeout)
                                    .await
                                    .is_ok())
                            }
                        }
                    })
                    .unwrap();
                lua.globals()
                    .set("sleep_and_yield", sleep_and_yield)
                    .unwrap();
                lua.globals().set("yield_now", yield_now).unwrap();
                lua.globals()
                    .set("sleep_and_block", sleep_and_block)
                    .unwrap();
                lua.globals().set("send_stdin", send_stdin).unwrap();
                lua.globals().set("log_info", log_info).unwrap();
                lua.globals().set("log_warn", log_warn).unwrap();
                lua.globals().set("log_err", log_err).unwrap();
                lua.globals().set("await_event", await_event).unwrap();
                lua.globals().set("spawn_task", spawn_task).unwrap();
                lua.globals().set("abort_task", abort_task).unwrap();
                lua.globals().set("await_task", await_task).unwrap();
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
