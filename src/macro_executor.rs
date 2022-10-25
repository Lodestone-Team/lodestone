use std::{collections::HashMap, sync::Arc};

use log::debug;
use mlua::Lua;
use tokio::{
    runtime::Builder,
    sync::{mpsc::UnboundedSender, Mutex},
    task::{JoinHandle, LocalSet},
};

use crate::traits::{Error, ErrorInner};
#[derive(Debug)]
pub struct LuaExecutionInstruction {
    pub lua: Option<Lua>,
    pub content: String,
    pub args: Vec<String>,
    pub executor: Option<String>,
}

#[derive(Clone)]
pub struct MacroExecutor {
    macro_process_table: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    sender: UnboundedSender<(LuaExecutionInstruction, String)>,
    /// since the Lua struct isn't clone, we store a Fn closure that produces a Lua struct
    get_lua: Arc<Mutex<Arc<dyn Fn() -> Lua + Send + Sync>>>,
}

impl MacroExecutor {
    pub fn new(get_lua: Arc<Mutex<Arc<dyn Fn() -> Lua + Send + Sync>>>) -> MacroExecutor {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let process_table = Arc::new(Mutex::new(HashMap::new()));
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        std::thread::spawn({
            let process_table = process_table.clone();
            let get_lua = get_lua.clone();
            move || {
                let local = LocalSet::new();
                local.spawn_local(async move {
                    while let Some((new_task, uuid)) = rx.recv().await {
                        let handle = tokio::task::spawn_local({
                            let get_lua = get_lua.clone();
                            async move {
                                let LuaExecutionInstruction {
                                    lua,
                                    content,
                                    args,
                                    executor,
                                } = new_task;
                                let executor = executor.unwrap_or_default();
                                let lua = match lua {
                                    Some(lua) => lua,
                                    None => (get_lua.lock().await)(),
                                };
                                lua.globals().set("EXECUTOR", executor).unwrap();
                                lua.globals().set("ARG0", args.len()).unwrap();
                                for (i, arg) in args.iter().enumerate() {
                                    lua.globals()
                                        .set(format!("ARG{}", i + 1), arg.to_owned())
                                        .unwrap();
                                }
                                debug!("Executing lua: {}", content);
                                lua.load(&content).exec_async().await.unwrap();
                            }
                        });
                        process_table.lock().await.insert(uuid, handle);
                    }
                    // If the while loop returns, then all the LocalSpawner
                    // objects have been dropped.
                });

                // This will return once all senders are dropped and all
                // spawned tasks have returned.
                rt.block_on(local);
            }
        });
        MacroExecutor {
            macro_process_table: process_table,
            sender: tx,
            get_lua,
        }
    }
    /// modify the lua execution context while choosing preserving the old context by adding a new layer
    pub async fn add_lua_chain(&self, get_lua: Arc<dyn Fn(Lua) -> Lua + Sync + Send>) {
        // add the function to the lua chain
        let mut lock = self.get_lua.lock().await;
        let old = lock.clone();
        let new = Arc::new(move || {
            let lua = old();
            get_lua(lua)
        });
        *lock = new;
    }

    /// set the lua execution context
    pub async fn set_lua(&self, get_lua: Arc<dyn Fn() -> Lua + Sync + Send>) {
        *self.get_lua.lock().await = get_lua;
    }

    pub fn spawn(&self, task: LuaExecutionInstruction) -> String {
        let uuid = uuid::Uuid::new_v4().to_string();
        self.sender
            .send((task, uuid.clone()))
            .expect("Thread with LocalSet has shut down.");
        uuid
    }

    /// abort a macro execution
    /// 
    /// Note that if a macro is blocking the executor, it will not be aborted
    pub async fn abort_macro(&self, uuid: &str) -> Result<(), Error> {
        self.macro_process_table
            .lock()
            .await
            .get(uuid)
            .ok_or_else(|| Error {
                inner: ErrorInner::MacroNotFound,
                detail: "Macro not found".to_owned(),
            })?
            .abort();
            Ok(())
    }
    /// return the sender
    ///
    /// note that the caller is responsible for generating the uuid
    ///
    /// I sure hope you know what you're doing...
    pub fn get_sender(&self) -> UnboundedSender<(LuaExecutionInstruction, String)> {
        self.sender.clone()
    }
}
