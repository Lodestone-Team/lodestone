use std::{collections::HashMap, fmt::Debug, path::PathBuf, sync::Arc, time::Duration};

use log::{debug, error, info};
use tokio::{
    runtime::Builder,
    sync::{
        broadcast,
        mpsc::{self, UnboundedSender},
        oneshot, Mutex,
    },
    task::{JoinHandle, LocalSet},
};

use crate::{
    events::{MacroEvent, MacroEventInner},
    traits::{Error, ErrorInner},
    util::rand_macro_uuid,
};

pub struct ExecutionInstruction {
    pub runtime: Box<
        dyn Fn(
                String,
                String,
                Vec<String>,
                bool,
            ) -> Result<(deno_runtime::worker::MainWorker, PathBuf), Error>
            + Send,
    >,
    pub name: String,
    pub executor: Option<String>,
    pub args: Vec<String>,
    pub is_in_game: bool,
}

pub enum Task {
    Spawn(ExecutionInstruction),
    Abort(String),
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::Spawn(exec_instruction) => f
                .debug_struct("Task::Spawn")
                .field("name", &exec_instruction.name)
                .field("args", &exec_instruction.args)
                .field("executor", &exec_instruction.executor)
                .finish(),
            Task::Abort(uuid) => {
                write!(f, "Abort {{ uuid: {} }}", uuid)
            }
        }
    }
}

#[derive(Clone)]
pub struct MacroExecutor {
    macro_process_table: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    sender: mpsc::UnboundedSender<(Task, String)>,
    event_broadcaster: broadcast::Sender<MacroEvent>,
}

impl MacroExecutor {
    pub fn new() -> MacroExecutor {
        let (tx, mut rx): (
            mpsc::UnboundedSender<(Task, String)>,
            mpsc::UnboundedReceiver<(Task, String)>,
        ) = mpsc::unbounded_channel();
        let (event_broadcaster, _) = broadcast::channel(16);
        let process_table = Arc::new(Mutex::new(HashMap::new()));
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        std::thread::spawn({
            let process_table = process_table.clone();
            let event_broadcaster = event_broadcaster.clone();
            move || {
                let local = LocalSet::new();
                local.spawn_local(async move {
                    while let Some((new_task, uuid)) = rx.recv().await {
                        match new_task {
                            Task::Spawn(exec_instruction) => {
                                let handle = tokio::task::spawn_local({
                                    let event_broadcaster = event_broadcaster.clone();
                                    let uuid = uuid.clone();
                                    async move {
                                        let ExecutionInstruction {
                                            runtime,
                                            name,
                                            args,
                                            executor,
                                            is_in_game,
                                        } = exec_instruction;
                                        let executor = executor.unwrap_or_default();
                                        // inject exectuor into the js runtime
                                        let (mut runtime, path_to_main_module) =
                                            runtime(name, executor, args, is_in_game)
                                                .expect("Failed to create runtime");
                                        let main_module = deno_core::resolve_path(
                                            &path_to_main_module.to_string_lossy(),
                                        )
                                        .unwrap();

                                        let _ = runtime
                                            .execute_main_module(&main_module)
                                            .await
                                            .map_err(|e| {
                                                error!("Error executing main module: {}", e);
                                                e
                                            });

                                        let _ = runtime.run_event_loop(false).await.map_err(|e| {
                                            error!("Error while running event loop: {}", e);
                                        });

                                        let _ = event_broadcaster.send(MacroEvent {
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
                debug!("MacroExecutor thread exited");
            }
        });
        MacroExecutor {
            macro_process_table: process_table,
            sender: tx,
            event_broadcaster,
        }
    }
    /// modify the lua execution context while choosing preserving the old context by adding a new layer
    // pub async fn add_lua_chain(&self, get_lua: Arc<dyn Fn(Lua) -> Lua + Sync + Send>) {
    //     // add the function to the lua chain
    //     let mut lock = self.get_lua.lock().await;
    //     let old = lock.clone();
    //     let new = Arc::new(move || {
    //         let lua = old();
    //         get_lua(lua)
    //     });
    //     *lock = new;
    // }

    pub fn event_receiver(&self) -> broadcast::Receiver<MacroEvent> {
        self.event_broadcaster.subscribe()
    }

    pub fn spawn(&self, exec_instruction: ExecutionInstruction) -> String {
        let uuid = rand_macro_uuid();
        self.sender
            .send((Task::Spawn(exec_instruction), uuid.clone()))
            .expect("Thread with LocalSet has shut down.");
        info!("Spawned macro with uuid {}", uuid);
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
    pub async fn wait_with_timeout(
        &self,
        macro_uuid: String,
        timeout: Option<f64>,
    ) -> Result<(), ()> {
        let mut rx = self.event_broadcaster.subscribe();
        tokio::select! {
            _ = async {
                if let Some(timeout) = timeout {
                    tokio::time::sleep(Duration::from_secs_f64(timeout)).await;
                } else {
                    // create a future that never resolves
                    let (_tx, rx) = oneshot::channel::<()>();
                    let _ = rx.await;

                }
            } => {
                Err(())
            }
            _ = {
                async {loop {
                    let event = rx.recv().await.unwrap();
                    if event.macro_uuid == macro_uuid {
                        break;
                    }
                }
            }} => {
                Ok(())
            }
        }
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
