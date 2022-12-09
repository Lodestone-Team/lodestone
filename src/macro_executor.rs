use std::{
    collections::HashMap,
    fmt::Debug,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use log::{debug, error, info};
use tokio::{
    runtime::Builder,
    sync::{
        broadcast,
        mpsc,
        oneshot, Mutex,
    },
    task::{JoinHandle, LocalSet},
};

use crate::{
    events::{MacroEvent, MacroEventInner},
    traits::{Error, ErrorInner},
};

use std::pin::Pin;

use anyhow::bail;
use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::anyhow;
use deno_core::anyhow::anyhow;
use deno_core::resolve_import;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSourceFuture;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use futures::FutureExt;
pub struct TypescriptModuleLoader;

pub fn resolve_macro_invocation(
    path_to_macro: &Path,
    macro_name: &str,
    is_in_game: bool,
) -> Option<PathBuf> {
    let path_to_macro = if is_in_game {
        path_to_macro.join("in_game")
    } else {
        path_to_macro.to_owned()
    };

    let ts_macro = path_to_macro.join(macro_name).with_extension("ts");
    let js_macro = path_to_macro.join(macro_name).with_extension("js");

    let macro_folder = path_to_macro.join(macro_name);

    if ts_macro.is_file() {
        return Some(ts_macro);
    } else if js_macro.is_file() {
        return Some(js_macro);
    } else if macro_folder.is_dir() {
        // check if index.ts exists
        let index_ts = macro_folder.join("index.ts");
        let index_js = macro_folder.join("index.js");
        if index_ts.exists() {
            return Some(index_ts);
        } else if index_js.exists() {
            return Some(index_js);
        }
    } else if !is_in_game {
        return resolve_macro_invocation(&path_to_macro.join("in_game"), macro_name, false);
    };
    None
}

impl ModuleLoader for TypescriptModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _is_main: bool,
    ) -> Result<ModuleSpecifier, anyhow::Error> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
        _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
        let module_specifier = module_specifier.clone();
        async move {
            let path = module_specifier
                .to_file_path()
                .map_err(|_| anyhow!("Only file: URLs are supported."))?;

            let path = if path.extension().is_none() && path.with_extension("ts").exists() {
                path.with_extension("ts")
            } else if path.with_extension("js").exists() {
                path.with_extension("js")
            } else {
                path
            };

            let path = if path.is_dir() {
                // check if index.ts exists
                let index_ts = path.join("index.ts");
                if index_ts.exists() {
                    index_ts
                } else {
                    path.join("index.js")
                }
            } else {
                path
            };

            let media_type = MediaType::from(&path);
            let (module_type, should_transpile) = match MediaType::from(&path) {
                MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                    (ModuleType::JavaScript, false)
                }
                MediaType::Jsx => (ModuleType::JavaScript, true),
                MediaType::TypeScript
                | MediaType::Mts
                | MediaType::Cts
                | MediaType::Dts
                | MediaType::Dmts
                | MediaType::Dcts
                | MediaType::Tsx => (ModuleType::JavaScript, true),
                MediaType::Json => (ModuleType::Json, false),
                _ => bail!("Unknown extension {:?}", path.extension()),
            };

            let code = std::fs::read_to_string(&path)?;
            let code = if should_transpile {
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.to_string(),
                    text_info: SourceTextInfo::from_string(code),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })?;
                parsed.transpile(&Default::default())?.text
            } else {
                code
            };
            let module = ModuleSource {
                code: code.into_bytes().into_boxed_slice(),
                module_type,
                module_url_specified: module_specifier.to_string(),
                module_url_found: module_specifier.to_string(),
            };
            Ok(module)
        }
        .boxed_local()
    }
}

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
    Abort(usize),
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
    macro_process_table: Arc<Mutex<HashMap<usize, JoinHandle<()>>>>,
    sender: mpsc::UnboundedSender<(Task, usize)>,
    event_broadcaster: broadcast::Sender<MacroEvent>,
    next_process_id: Arc<AtomicUsize>,
}

impl MacroExecutor {
    pub fn new() -> MacroExecutor {
        let (tx, mut rx): (
            mpsc::UnboundedSender<(Task, usize)>,
            mpsc::UnboundedReceiver<(Task, usize)>,
        ) = mpsc::unbounded_channel();
        let (event_broadcaster, _) = broadcast::channel(16);
        let process_table = Arc::new(Mutex::new(HashMap::new()));
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let process_id = Arc::new(AtomicUsize::new(0));
        std::thread::spawn({
            let process_table = process_table.clone();
            let process_id = process_id.clone();
            let event_broadcaster = event_broadcaster.clone();
            move || {
                let local = LocalSet::new();
                local.spawn_local(async move {
                    while let Some((new_task, pid)) = rx.recv().await {
                        match new_task {
                            Task::Spawn(exec_instruction) => {
                                let handle = tokio::task::spawn_local({
                                    let event_broadcaster = event_broadcaster.clone();
                                    let process_id = process_id.clone();
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
                                            macro_pid: process_id.load(Ordering::SeqCst),
                                            macro_event_inner: MacroEventInner::MacroStopped,
                                            instance_uuid: "".to_string(),
                                        });
                                    }
                                });
                                process_table.lock().await.insert(pid, handle);
                            }
                            Task::Abort(pid) => {
                                process_table.lock().await.get(&pid).unwrap().abort();
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
            next_process_id: process_id.clone(),
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

    pub fn spawn(&self, exec_instruction: ExecutionInstruction) -> usize {
        let pid = self.next_process_id.fetch_add(1, Ordering::SeqCst);
        self.sender
            .send((Task::Spawn(exec_instruction), pid))
            .expect("Thread with LocalSet has shut down.");
        info!("Spawned macro with pid {}", pid);
        pid
    }

    /// abort a macro execution
    ///
    /// Note that if a macro is blocking the executor, it will not be aborted
    pub async fn abort_macro(&self, pid: &usize) -> Result<(), Error> {
        self.macro_process_table
            .lock()
            .await
            .get(&pid)
            .ok_or_else(|| Error {
                inner: ErrorInner::MacroNotFound,
                detail: "Macro not found".to_owned(),
            })?
            .abort();
        Ok(())
    }
    pub async fn wait_with_timeout(
        &self,
        macro_pid: usize,
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
                    if event.macro_pid == macro_pid {
                        break;
                    }
                }
            }} => {
                Ok(())
            }
        }
    }

    pub async fn get_macro_status(&self, pid: usize) -> Result<bool, Error> {
        let table = self.macro_process_table.lock().await;
        let handle = table.get(&pid).ok_or_else(|| Error {
            inner: ErrorInner::MacroNotFound,
            detail: "Macro not found".to_owned(),
        })?;
        Ok(handle.is_finished())
    }
}
