use std::{
    fmt::{Debug, Display},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use color_eyre::eyre::Context;
use dashmap::DashMap;
use deno_runtime::permissions::Permissions;
use futures_util::Future;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{runtime::Builder, sync::mpsc, task::LocalSet};
use tracing::{debug, error, log::warn};
use ts_rs::TS;

use crate::{
    deno_ops::events::register_all_event_ops,
    error::{Error, ErrorKind},
    event_broadcaster::EventBroadcaster,
    events::{CausedBy, EventInner, MacroEvent, MacroEventInner},
    traits::t_macro::ExitStatus,
    types::InstanceUuid,
};

use color_eyre::eyre::eyre;

use std::pin::Pin;

use anyhow::bail;
use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSourceFuture;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use deno_core::{anyhow, error::generic_error};
use deno_core::{resolve_import, ModuleCode};

use futures::FutureExt;

pub trait WorkerOptionGenerator: Send + Sync {
    fn generate(&self) -> deno_runtime::worker::WorkerOptions;
}
pub struct TypescriptModuleLoader {
    http: reqwest::Client,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct MacroPID(pub usize); // todo remove pub

impl From<MacroPID> for usize {
    fn from(uid: MacroPID) -> Self {
        uid.0
    }
}

impl From<&MacroPID> for usize {
    fn from(uid: &MacroPID) -> Self {
        uid.0
    }
}

impl AsRef<usize> for MacroPID {
    fn as_ref(&self) -> &usize {
        &self.0
    }
}

impl Display for MacroPID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MacroPID({})", self.0)
    }
}

impl Default for TypescriptModuleLoader {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

impl ModuleLoader for TypescriptModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, anyhow::Error> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
    ) -> Pin<Box<ModuleSourceFuture>> {
        let module_specifier = module_specifier.clone();
        let http = self.http.clone();
        async move {
            let (code, module_type, media_type, should_transpile) = match module_specifier
                .to_file_path()
            {
                Ok(path) => {
                    let media_type = MediaType::from_path(&path);
                    let (module_type, should_transpile) = match media_type {
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

                    (
                        tokio::fs::read_to_string(&path).await?,
                        module_type,
                        media_type,
                        should_transpile,
                    )
                }
                Err(_) => {
                    if module_specifier.scheme() == "http" || module_specifier.scheme() == "https" {
                        let http_res = http.get(module_specifier.to_string()).send().await?;
                        if !http_res.status().is_success() {
                            bail!("Failed to fetch module: {module_specifier}");
                        }
                        let content_type = http_res
                            .headers()
                            .get("content-type")
                            .and_then(|ct| ct.to_str().ok())
                            .ok_or_else(|| generic_error("No content-type header"))?;
                        let media_type =
                            MediaType::from_content_type(&module_specifier, content_type);
                        let (module_type, should_transpile) = match media_type {
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
                            _ => bail!("Unknown content-type {:?}", content_type),
                        };
                        let code = http_res.text().await?;
                        (code, module_type, media_type, should_transpile)
                    } else {
                        bail!("Unsupported module specifier: {}", module_specifier);
                    }
                }
            };
            let code = if should_transpile {
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.to_string(),
                    text_info: SourceTextInfo::from_string(code),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })?;
                parsed.transpile(&Default::default())?.text.into_boxed_str()
            } else {
                code.into_boxed_str()
            };

            let module = ModuleSource::new(module_type, ModuleCode::Owned(code), &module_specifier);
            Ok(module)
        }
        .boxed_local()
    }
}

#[derive(Clone, Debug)]
pub struct MacroExecutor {
    macro_process_table: Arc<DashMap<MacroPID, deno_core::v8::IsolateHandle>>,
    exit_status_table: Arc<DashMap<MacroPID, ExitStatus>>,
    channel_table:
        Arc<DashMap<MacroPID, (mpsc::UnboundedSender<Value>, mpsc::UnboundedSender<Value>)>>,
    event_broadcaster: EventBroadcaster,
    next_process_id: Arc<AtomicUsize>,
}

pub struct SpawnResult {
    pub macro_pid: MacroPID,
    pub main_module_future: Pin<Box<dyn Future<Output = ()> + Send>>,
    pub exit_future: Pin<Box<dyn Future<Output = Result<ExitStatus, Error>> + Send>>,
}

impl MacroExecutor {
    pub fn new(event_broadcaster: EventBroadcaster) -> MacroExecutor {
        let process_table = Arc::new(DashMap::new());
        let process_id = Arc::new(AtomicUsize::new(0));
        let exit_status_table = Arc::new(DashMap::new());

        // spawn a task to listen for exit events and update the exit status table
        tokio::task::spawn({
            let exit_status_table = exit_status_table.clone();
            let mut rx = event_broadcaster.subscribe();
            async move {
                loop {
                    if let Ok(event) = rx.recv().await {
                        if let Some(MacroEvent {
                            macro_pid,
                            macro_event_inner: MacroEventInner::Stopped { exit_status },
                            ..
                        }) = event.try_macro_event()
                        {
                            exit_status_table.insert(*macro_pid, exit_status.clone());
                        }
                    }
                }
            }
        });

        MacroExecutor {
            macro_process_table: process_table,
            event_broadcaster,
            channel_table: Arc::new(DashMap::new()),
            exit_status_table,
            next_process_id: process_id,
        }
    }

    /// For timeout:
    ///
    /// If `None`, the handle will never timeout.
    ///
    /// If `Some(Duration)`, the handle will timeout after the duration.
    ///
    /// Note that this does not terminate the process, it just stops the handle from waiting for it.
    ///
    /// It is up to the caller to terminate the process if it is still running.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        &self,
        path_to_main_module: PathBuf,
        args: Vec<String>,
        _caused_by: CausedBy,
        worker_options_generator: Box<dyn WorkerOptionGenerator>,
        permissions: Option<Permissions>,
        instance_uuid: Option<InstanceUuid>,
        timeout: Option<Duration>,
    ) -> Result<SpawnResult, Error> {
        let pid = MacroPID(self.next_process_id.fetch_add(1, Ordering::SeqCst));
        let exit_future = Box::pin({
            let __self = self.clone();
            async move { __self.wait_with_timeout(pid, timeout).await }
        });
        let main_module_future = Box::pin({
            let __self = self.clone();
            async move {
                __self.wait_for_main_module_executed(pid).await;
            }
        });
        let main_module = deno_core::resolve_path(
            ".",
            &std::env::current_dir().context("Failed to get current directory")?,
        )
        .context("Failed to resolve path")?;
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        std::thread::spawn({
            let process_table = self.macro_process_table.clone();
            let event_broadcaster = self.event_broadcaster.clone();
            move || {
                let local = LocalSet::new();
                local.spawn_local(async move {
                    let mut worker_option = worker_options_generator.generate();
                    register_all_event_ops(&mut worker_option, event_broadcaster.clone());
                    worker_option.bootstrap.args = args;

                    let mut main_worker = deno_runtime::worker::MainWorker::from_options(
                        main_module,
                        deno_runtime::permissions::PermissionsContainer::new(
                            permissions.unwrap_or_else(Permissions::allow_all),
                        ),
                        worker_option,
                    );

                    let isolate_handle = main_worker.js_runtime.v8_isolate().thread_safe_handle();

                    process_table.insert(pid, isolate_handle);

                    let main_module = match deno_core::resolve_path(
                        &path_to_main_module.to_string_lossy(),
                        &std::env::current_dir().unwrap(),
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Error resolving main module: {}", e);
                            return;
                        }
                    };

                    event_broadcaster.send(
                        MacroEvent {
                            macro_pid: pid,
                            macro_event_inner: MacroEventInner::Started,
                            instance_uuid: instance_uuid.clone(),
                        }
                        .into(),
                    );

                    if let Err(e) = main_worker.execute_main_module(&main_module).await {
                        if e.to_string() == "Uncaught Error: execution terminated" {
                            warn!("User terminated macro execution");
                            event_broadcaster.send(
                                MacroEvent {
                                    macro_pid: pid,
                                    macro_event_inner: MacroEventInner::Stopped {
                                        exit_status: ExitStatus::Killed {
                                            time: chrono::Utc::now().timestamp(),
                                        },
                                    },
                                    instance_uuid,
                                }
                                .into(),
                            );
                        } else {
                            error!("Error executing main module {main_module}: {}", e);
                            event_broadcaster.send(
                                MacroEvent {
                                    macro_pid: pid,
                                    macro_event_inner: MacroEventInner::Stopped {
                                        exit_status: ExitStatus::Error {
                                            error_msg: e.to_string(),
                                            time: chrono::Utc::now().timestamp(),
                                        },
                                    },
                                    instance_uuid,
                                }
                                .into(),
                            );
                        }
                        return;
                    }

                    event_broadcaster.send(
                        MacroEvent {
                            macro_pid: pid,
                            macro_event_inner: MacroEventInner::MainModuleExecuted,
                            instance_uuid: instance_uuid.clone(),
                        }
                        .into(),
                    );

                    if let Err(e) = main_worker.run_event_loop(false).await {
                        if e.to_string() == "Uncaught Error: execution terminated" {
                            warn!("User terminated macro execution");
                            event_broadcaster.send(
                                MacroEvent {
                                    macro_pid: pid,
                                    macro_event_inner: MacroEventInner::Stopped {
                                        exit_status: ExitStatus::Killed {
                                            time: chrono::Utc::now().timestamp(),
                                        },
                                    },
                                    instance_uuid: instance_uuid.clone(),
                                }
                                .into(),
                            );
                        } else {
                            error!("Error running event loops: {}", e);
                            event_broadcaster.send(
                                MacroEvent {
                                    macro_pid: pid,
                                    macro_event_inner: MacroEventInner::Stopped {
                                        exit_status: ExitStatus::Error {
                                            error_msg: e.to_string(),
                                            time: chrono::Utc::now().timestamp(),
                                        },
                                    },
                                    instance_uuid: instance_uuid.clone(),
                                }
                                .into(),
                            );
                        }
                    }

                    event_broadcaster.send(
                        MacroEvent {
                            macro_pid: pid,
                            macro_event_inner: MacroEventInner::Stopped {
                                exit_status: ExitStatus::Success {
                                    time: chrono::Utc::now().timestamp(),
                                },
                            },
                            instance_uuid,
                        }
                        .into(),
                    );

                    // If the while loop returns, then all the LocalSpawner
                    // objects have been dropped.
                });

                // This will return once all senders are dropped and all
                // spawned tasks have returned.
                rt.block_on(local);
                debug!("MacroExecutor thread exited");
            }
        });

        // listen to event broadcaster for macro started event
        // and return the pid

        let rx = self.event_broadcaster.subscribe();

        let fut = async move {
            let mut rx = rx;
            loop {
                if let Ok(event) = rx.recv().await {
                    if let EventInner::MacroEvent(MacroEvent {
                        macro_pid,
                        macro_event_inner: MacroEventInner::Started,
                        ..
                    }) = event.event_inner
                    {
                        if macro_pid == pid {
                            return Ok(macro_pid);
                        }
                    }
                } else {
                    break Err(eyre!("Failed to receive macro started event"));
                }
            }
        };

        tokio::time::timeout(Duration::from_secs(1), fut)
            .await
            .context("Failed to spawn macro")??;
        Ok(SpawnResult {
            macro_pid: pid,
            main_module_future,
            exit_future,
        })
    }

    /// abort a macro execution
    pub fn abort_macro(&self, pid: MacroPID) -> Result<(), Error> {
        self.macro_process_table
            .get(&pid)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Macro with pid {} not found", pid),
            })?
            .terminate_execution();
        Ok(())
    }

    async fn wait_for_main_module_executed(&self, taget_macro_pid: MacroPID) {
        let mut rx = self.event_broadcaster.subscribe();
        loop {
            let event = rx.recv().await.unwrap();
            if let EventInner::MacroEvent(MacroEvent {
                macro_pid,
                macro_event_inner,
                ..
            }) = event.event_inner
            {
                if taget_macro_pid == macro_pid {
                    if let MacroEventInner::MainModuleExecuted = macro_event_inner {
                        return;
                    }
                }
            }
        }
    }

    /// wait for a macro to finish
    async fn wait_with_timeout(
        &self,
        taget_macro_pid: MacroPID,
        timeout: Option<Duration>,
    ) -> Result<ExitStatus, Error> {
        let mut rx = self.event_broadcaster.subscribe();
        let fut = async {
            loop {
                let event = rx.recv().await.unwrap();
                if let EventInner::MacroEvent(MacroEvent {
                    macro_pid,
                    macro_event_inner,
                    ..
                }) = event.event_inner
                {
                    if taget_macro_pid == macro_pid {
                        if let MacroEventInner::Stopped { exit_status } = macro_event_inner {
                            break Ok(exit_status);
                        }
                    }
                }
            }
        };
        if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, fut)
                .await
                .context("Macro execution timed out")?
        } else {
            fut.await
        }
    }

    pub async fn get_macro_status(&self, pid: MacroPID) -> Option<ExitStatus> {
        self.exit_status_table.get(&pid).map(|v| v.clone())
    }
}

#[cfg(test)]
mod tests {

    use std::rc::Rc;

    use deno_core::op;

    use super::{TypescriptModuleLoader, WorkerOptionGenerator};

    use crate::event_broadcaster::EventBroadcaster;
    use crate::events::CausedBy;
    use crate::macro_executor::SpawnResult;

    struct BasicMainWorkerGenerator;

    #[op]
    fn hello_world() -> String {
        "Hello World".to_string()
    }

    #[op]
    async fn async_hello_world() -> String {
        "async Hello World".to_string()
    }

    impl WorkerOptionGenerator for BasicMainWorkerGenerator {
        fn generate(&self) -> deno_runtime::worker::WorkerOptions {
            let ext = deno_core::Extension::builder("generic_deno_extension_builder")
                .ops(vec![hello_world::decl(), async_hello_world::decl()])
                .force_op_registration()
                .build();
            deno_runtime::worker::WorkerOptions {
                module_loader: Rc::new(TypescriptModuleLoader::default()),
                extensions: vec![ext],
                ..Default::default()
            }
        }
    }
    #[tokio::test]
    async fn basic_execution() {
        // init tracing
        tracing_subscriber::fmt::init();
        let (event_broadcaster, _) = EventBroadcaster::new(10);
        // construct a macro executor
        let executor = super::MacroExecutor::new(event_broadcaster);

        // create a temp directory
        let temp_dir = tempdir::TempDir::new("macro_test").unwrap().into_path();

        // create a macro file

        let path_to_macro = temp_dir.join("test.ts");

        std::fs::write(
            &path_to_macro,
            r#"
            const { core } = Deno;
            const { ops } = core;
            console.log(ops.hello_world())
            console.log(await core.opAsync("async_hello_world"))
            "#,
        )
        .unwrap();

        let basic_worker_generator = BasicMainWorkerGenerator;

        let SpawnResult { exit_future, .. } = executor
            .spawn(
                path_to_macro,
                Vec::new(),
                CausedBy::Unknown,
                Box::new(basic_worker_generator),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        exit_future.await.unwrap();
    }

    #[tokio::test]
    async fn test_http_url() {
        let (event_broadcaster, _) = EventBroadcaster::new(10);
        // construct a macro executor
        let executor = super::MacroExecutor::new(event_broadcaster);

        // create a temp directory
        let temp_dir = tempdir::TempDir::new("macro_test").unwrap().into_path();

        // create a macro file

        let path_to_macro = temp_dir.join("test.ts");

        std::fs::write(
            &path_to_macro,
            r#"
            import { readLines } from "https://deno.land/std@0.104.0/io/mod.ts";
            console.log(readLines);
            "#,
        )
        .unwrap();

        let basic_worker_generator = BasicMainWorkerGenerator;

        let SpawnResult { exit_future, .. } = executor
            .spawn(
                path_to_macro,
                Vec::new(),
                CausedBy::Unknown,
                Box::new(basic_worker_generator),
                None,
                None,
                None,
            )
            .await
            .unwrap();
        exit_future.await.unwrap();
    }
}
