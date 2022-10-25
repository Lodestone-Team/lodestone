use std::{collections::HashMap, sync::Arc};

use log::{debug, error};
use mlua::Lua;
use tokio::{
    runtime::Builder,
    sync::{
        broadcast,
        mpsc::{self, UnboundedSender},
        Mutex,
    },
    task::{JoinHandle, LocalSet},
};

use crate::{
    events::{MacroEvent, MacroEventInner},
    traits::{Error, ErrorInner}, util::rand_macro_uuid,
};
#[derive(Debug)]
pub struct LuaExecutionInstruction {
    pub lua: Option<Lua>,
    pub content: String,
    pub args: Vec<String>,
    pub executor: Option<String>,
}
#[derive(Debug)]
pub enum Task {
    Spawn(LuaExecutionInstruction),
    Abort(String),
}

#[derive(Clone)]
pub struct MacroExecutor {
    macro_process_table: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    sender: mpsc::UnboundedSender<(Task, String)>,
    event_sender: broadcast::Sender<MacroEvent>,
    /// since the Lua struct isn't clone, we store a Fn closure that produces a Lua struct
    get_lua: Arc<Mutex<Arc<dyn Fn() -> Lua + Send + Sync>>>,
}

impl MacroExecutor {
    pub fn new(get_lua: Arc<Mutex<Arc<dyn Fn() -> Lua + Send + Sync>>>) -> MacroExecutor {
        let (tx, mut rx): (
            mpsc::UnboundedSender<(Task, String)>,
            mpsc::UnboundedReceiver<(Task, String)>,
        ) = mpsc::unbounded_channel();
        let (event_sender, _) = broadcast::channel(16);
        let process_table = Arc::new(Mutex::new(HashMap::new()));
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        std::thread::spawn({
            let process_table = process_table.clone();
            let get_lua = get_lua.clone();
            let event_sender = event_sender.clone();
            move || {
                let local = LocalSet::new();
                local.spawn_local(async move {
                    while let Some((new_task, uuid)) = rx.recv().await {
                        match new_task {
                            Task::Spawn(exec_instruction) => {
                                let handle = tokio::task::spawn_local({
                                    let get_lua = get_lua.clone();
                                    let event_sender = event_sender.clone();
                                    let uuid = uuid.clone();
                                    async move {
                                        let LuaExecutionInstruction {
                                            lua,
                                            content,
                                            args,
                                            executor,
                                        } = exec_instruction;
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
                                        // debug!("Executing lua: {}", content);

                                        let _ = lua.load(&content).exec_async().await.map_err({
                                            |e| {
                                                error!("Failed to execute lua: {}", e);
                                                let _ = event_sender.send(MacroEvent {
                                                    macro_uuid: uuid.clone(),
                                                    macro_event_inner:
                                                        MacroEventInner::MacroErrored {
                                                            error_msg: e.to_string(),
                                                        },
                                                    instance_uuid: "".to_string(),
                                                });
                                            }
                                        });
                                        let _ = event_sender.send(MacroEvent {
                                            macro_uuid: uuid.clone(),
                                            macro_event_inner: MacroEventInner::MacroStopped,
                                            instance_uuid: "".to_string(),
                                        });
                                    }
                                });
                                process_table.lock().await.insert(uuid, handle);
                            }
                            Task::Abort(uuid) => {
                                process_table.lock().await.get(&uuid).unwrap().abort();
                            }
                        }
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
            event_sender,
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

    pub fn event_stream(&self) -> broadcast::Receiver<MacroEvent> {
        self.event_sender.subscribe()
    }

    /// set the lua execution context
    pub async fn set_lua(&self, get_lua: Arc<dyn Fn() -> Lua + Sync + Send>) {
        *self.get_lua.lock().await = get_lua;
    }

    pub fn spawn(&self, exec_instruction: LuaExecutionInstruction) -> String {
        let uuid = rand_macro_uuid();
        self.sender
            .send((Task::Spawn(exec_instruction), uuid.clone()))
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
    pub fn get_sender(&self) -> UnboundedSender<(Task, String)> {
        self.sender.clone()
    }
}
