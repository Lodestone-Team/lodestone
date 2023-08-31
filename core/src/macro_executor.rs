use std::{
    fmt::{Debug, Display},
    iter::zip,
    path::PathBuf,
    rc::Rc,
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
use tokio::{sync::mpsc, task::LocalSet};
use tracing::{debug, error, log::warn};
use ts_rs::TS;

use crate::{
    deno_ops::{
        events::register_all_event_ops, instance_control::register_instance_control_ops,
        prelude::register_prelude_ops,
    },
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

use crate::traits::t_configurable::manifest::{
    ConfigurableValue, ConfigurableValueType, SettingManifest,
};
use crate::util::fs;
use futures::FutureExt;
use indexmap::IndexMap;

pub trait WorkerOptionGenerator: Send + Sync {
    fn generate(&self) -> deno_runtime::worker::WorkerOptions;
}

pub struct DefaultWorkerOptionGenerator;

impl WorkerOptionGenerator for DefaultWorkerOptionGenerator {
    fn generate(&self) -> deno_runtime::worker::WorkerOptions {
        deno_runtime::worker::WorkerOptions {
            module_loader: Rc::new(TypescriptModuleLoader::default()),
            ..Default::default()
        }
    }
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
    rt: tokio::runtime::Handle,
}

pub struct SpawnResult {
    pub macro_pid: MacroPID,
    pub detach_future: Pin<Box<dyn Future<Output = ()> + Send>>,
    pub exit_future: Pin<Box<dyn Future<Output = Result<ExitStatus, Error>> + Send>>,
}

impl MacroExecutor {
    pub fn new(event_broadcaster: EventBroadcaster, rt: tokio::runtime::Handle) -> MacroExecutor {
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
            rt,
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
    ) -> Result<SpawnResult, Error> {
        let pid = MacroPID(self.next_process_id.fetch_add(1, Ordering::SeqCst));
        let exit_future = Box::pin({
            let __self = self.clone();
            async move { __self.wait_with_timeout(pid).await }
        });
        let detach_future = Box::pin({
            let __self = self.clone();
            async move {
                __self.wait_for_detach(pid).await;
            }
        });
        let main_module = deno_core::resolve_path(
            ".",
            &std::env::current_dir().context("Failed to get current directory")?,
        )
        .context("Failed to resolve path")?;
        std::thread::spawn({
            let process_table = self.macro_process_table.clone();
            let event_broadcaster = self.event_broadcaster.clone();
            let rt = self.rt.clone();
            move || {
                let _guard = rt.enter();
                let local = LocalSet::new();
                local.spawn_local({
                    let event_broadcaster = event_broadcaster.clone();
                    let instance_uuid = instance_uuid.clone();
                    async move {
                        let mut worker_option = worker_options_generator.generate();
                        worker_option.get_error_class_fn = Some(&deno_errors::get_error_class_name);
                        register_prelude_ops(&mut worker_option);
                        register_all_event_ops(&mut worker_option, event_broadcaster.clone());
                        register_instance_control_ops(&mut worker_option);

                        let mut main_worker = deno_runtime::worker::MainWorker::from_options(
                            main_module,
                            deno_runtime::permissions::PermissionsContainer::new(
                                permissions.unwrap_or_else(Permissions::allow_all),
                            ),
                            worker_option,
                        );
                        main_worker.bootstrap(&deno_runtime::BootstrapOptions {
                            args,
                            ..Default::default()
                        });
                        main_worker
                            .execute_script(
                                "deps_inject",
                                deno_core::FastString::Owned(
                                    format!(
                                        "const __macro_pid = {}; const __instance_uuid = \"{}\";",
                                        pid.0,
                                        instance_uuid
                                            .clone()
                                            .map(|uuid| uuid.to_string())
                                            .unwrap_or_else(|| "null".to_string())
                                    )
                                    .into_boxed_str(),
                                ),
                            )
                            .unwrap();

                        let isolate_handle =
                            main_worker.js_runtime.v8_isolate().thread_safe_handle();

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

                        debug!("Macro event loop exited");

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
                    }
                });

                // This will return once all senders are dropped and all
                // spawned tasks have returned.
                rt.block_on(local);
                debug!("MacroExecutor thread exited");
                event_broadcaster.send(
                    MacroEvent {
                        macro_pid: pid,
                        macro_event_inner: MacroEventInner::Stopped {
                            exit_status: ExitStatus::Error {
                                time: chrono::Utc::now().timestamp(),
                                error_msg: "Macro executor thread unexpectedly panicked"
                                    .to_string(),
                            },
                        },
                        instance_uuid: instance_uuid.clone(),
                    }
                    .into(),
                );
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
            detach_future,
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

    pub async fn wait_for_detach(&self, target_macro_pid: MacroPID) {
        let mut rx = self.event_broadcaster.subscribe();
        loop {
            let event = rx.recv().await.unwrap();
            if let EventInner::MacroEvent(MacroEvent {
                macro_pid,
                macro_event_inner,
                ..
            }) = event.event_inner
            {
                if target_macro_pid == macro_pid {
                    if let MacroEventInner::Detach = macro_event_inner {
                        return;
                    }
                }
            }
        }
    }

    /// wait for a macro to finish
    async fn wait_with_timeout(&self, taget_macro_pid: MacroPID) -> Result<ExitStatus, Error> {
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
                    if let MacroEventInner::Stopped { exit_status } = macro_event_inner {
                        break Ok(exit_status);
                    }
                }
            }
        }
    }

    pub async fn get_macro_status(&self, pid: MacroPID) -> Option<ExitStatus> {
        self.exit_status_table.get(&pid).map(|v| v.clone())
    }

    pub async fn get_config_manifest(
        path: &PathBuf,
    ) -> Result<IndexMap<String, SettingManifest>, Error> {
        match extract_config_code(&fs::read_to_string(path).await?) {
            Ok(optional_code) => match optional_code {
                Some((var_name, definition)) => get_config_from_code(&var_name, &definition),
                None => Ok(IndexMap::<String, SettingManifest>::new()),
            },
            Err(e) => Err(e),
        }
    }
}

///
/// extract the class definition and the name of the declared config instance
/// from the typescript code
///
/// *returns (instance_name, class_definition)*
///
fn extract_config_code(code: &str) -> Result<Option<(String, String)>, Error> {
    let config_indices: Vec<_> = code.match_indices("LodestoneConfig").collect();
    if config_indices.is_empty() {
        return Ok(None);
    }
    if config_indices.len() < 2 {
        return Err(Error::ts_syntax_error(
            "Class definition or config declaration is missing",
        ));
    }

    // first occurrence of LodeStoneConfig must be the class declaration
    let config_code = &code[(config_indices[0].0)..];
    // end_index is for extracting the class definition
    let end_index = {
        let mut open_count = 0;
        let mut close_count = 0;
        let mut i = 0;
        // match for brackets to determine where the class definition ends
        for &char_item in config_code.to_string().as_bytes().iter() {
            if char_item == b'{' {
                open_count += 1;
            }
            if char_item == b'}' {
                close_count += 1;
            }
            i += 1;

            if open_count == close_count && open_count > 0 {
                break;
            }
        }

        if i == 0 || open_count != close_count {
            return Err(Error::ts_syntax_error("config"));
        }

        i
    };

    // second occurrence of LodeStoneConfig must be the config variable declaration
    // idea: slice from the end of class definition to 'let/const/var (name): LodestoneConfig'
    let config_var_code = {
        let second_occur_index =
            config_indices[1].0 - config_indices[0].0 + "LodestoneConfig".len();
        &config_code[end_index..second_occur_index]
    };

    // parse whether the keyword 'var', 'let', or 'const' is used
    let decl_keyword = {
        let mut config_var_tokens: Vec<_> = config_var_code.split(' ').collect();
        config_var_tokens.reverse();

        let keywords = ["let", "const", "var"];
        let keyword_found = config_var_tokens.iter().find(
            |&kw| keywords.contains(kw)
        );
        match keyword_found {
            Some(&kw) => kw,
            None => return Err(Error::ts_syntax_error(
                "Class definition detected but cannot find config declaration",
            ))
        }
    };

    let config_var_code = config_var_code.replace(' ', "");
    let config_var_name = {
        // now check for the last occurrence of the keyword to find the starting index of
        // 'let/const/var (name): LodestoneConfig'
        let decl_keyword_index = match config_var_code
            .match_indices(decl_keyword)
            .collect::<Vec<_>>()
            .last()
        {
            Some(val) => val.0,
            None => {
                return Err(Error::ts_syntax_error(
                    "Class definition detected but cannot find config declaration",
                ));
            }
        };

        // slice from the keyword to the end to isolate 'let/const/var (name): LodestoneConfig'
        let decl_var_statement = &config_var_code[decl_keyword_index..];
        // since spaces are removed, ':' is the separator between name and 'LodestoneConfig'
        let var_name_end_index = match decl_var_statement.find(':') {
            Some(index) => index,
            None => {
                return Err(Error::ts_syntax_error(
                    "Class definition detected but cannot find config declaration",
                ));
            }
        };

        // the name is in between the keyword and ':'
        &decl_var_statement[decl_keyword.len()..var_name_end_index]
    };

    // last sanity check: class definition must start with a '{'
    match config_code.find('{') {
        Some(start_index) => Ok(Some((
            config_var_name.to_string(),
            // we no longer need the declaration, so slice after the '{'
            config_code[start_index..end_index].to_string(),
        ))),
        None => Err(Error::ts_syntax_error("config")),
    }
}

fn get_config_from_code(
    config_var_name: &str,
    config_definition: &str,
) -> Result<IndexMap<String, SettingManifest>, Error> {
    // remove the open and close brackets
    let str_length = config_definition.len();
    let config_params_str = &config_definition[1..str_length - 1].to_string();

    let config_params_str: Vec<_> = config_params_str.split('\n').collect();

    // parse config code into a collection of description and definition
    let mut comment_lines: Vec<String> = vec![];
    let mut code_lines: Vec<String> = vec![];
    let mut comments: Vec<String> = vec![];
    let mut codes: Vec<String> = vec![];
    let mut comment_block_count = 0;
    for line in config_params_str {
        let line = line.replace('\r', "");
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        // comments within a comment block
        if comment_block_count > 0 {
            // closing the comment block
            if line.starts_with("*/") {
                comment_block_count -= 1;
                continue;
            }

            // comments within a comment block
            let comment_str = {
                if let Some(line) = line.strip_prefix('*') {
                    line.trim()
                } else {
                    line
                }
            };
            // do not push empty comment at the beginning of the comment block
            if !comment_str.is_empty() || !comment_lines.is_empty()  {
                comment_lines.push(comment_str.to_string());
            }
            continue;
        }

        // single line comment & opening of a comment block
        if line.starts_with("//") {
            if let Some(comment_str) = cleanup_comment_line(line, "//") {
                comment_lines.push(comment_str.to_string());
            }
        } else if line.starts_with("/**") {
            if let Some(comment_str) = cleanup_comment_line(line, "/**") {
                comment_lines.push(comment_str.to_string());
            }
            comment_block_count += 1;
        } else if line.starts_with("/*") {
            if let Some(comment_str) = cleanup_comment_line(line, "/*") {
                comment_lines.push(comment_str.to_string());
            }
            comment_block_count += 1;
        } else {
            // if non of those are satisfied, it must be a line of actual code instead of a comment
            let code_line = line.replace([' ', '\t'], "").trim().to_string();
            code_lines.push(code_line.clone());
            if code_line.ends_with(';') {
                comments.push(comment_lines.join(" "));
                comment_lines.clear();
                codes.push(code_lines.join("").strip_suffix(';').unwrap().to_string());
                code_lines.clear();
            }
        }
    }

    let mut configs: IndexMap<String, SettingManifest> = IndexMap::new();
    for (definition, desc) in zip(codes, comments) {
        let config = parse_config_single(&definition, &desc, config_var_name)?;
        configs.insert(config.get_identifier().clone(), config);
    }

    Ok(configs)
}

fn cleanup_comment_line(comment_line: &str, comment_prefix: &str) -> Option<String> {
    let result_str = comment_line.strip_prefix(comment_prefix).unwrap().trim();
    if result_str.is_empty() {
        None
    } else {
        Some(result_str.to_string())
    }
}

fn parse_config_single(
    single_config_definition: &str,
    config_description: &str,
    setting_id_prefix: &str,
) -> Result<SettingManifest, Error> {
    let entry = single_config_definition.trim().to_string();

    // compute indices to isolate class field names and types
    let (name_end_index, type_start_index) = match entry.find('?') {
        Some(index) => (index, index + 2),
        None => match entry.find(':') {
            Some(index) => (index, index + 1),
            None => {
                return Err(Error::ts_syntax_error("config"));
            }
        },
    };
    let var_name = &entry[..name_end_index];
    // if the field name is 2 char from the type, it must be optional ('?:')
    let is_optional = name_end_index + 2 == type_start_index;

    let default_value_index = match entry.find('=') {
        Some(index) => index,
        None => entry.len(),
    };
    if type_start_index >= default_value_index {
        return Err(Error::ts_syntax_error("config"));
    }

    let var_type = &entry[type_start_index..default_value_index];
    let config_type = get_config_value_type(var_type)?;
    let has_default = default_value_index != entry.len();

    // TODO: remove this. We will handle this in validation instead
    if !is_optional && !has_default {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("{var_name} is not optional and thus must have a default value"),
        });
    }

    let default_val = if has_default {
        // default value as string
        let val_str = entry[default_value_index + 1..].to_string();
        let val_str_len = val_str.len();
        Some(match config_type {
            ConfigurableValueType::String { .. } => {
                ConfigurableValue::String(val_str[1..val_str_len - 1].to_string())
            }
            ConfigurableValueType::Boolean => {
                let value = match val_str.parse::<bool>() {
                    Ok(val) => val,
                    Err(_) => {
                        return Err(Error {
                            kind: ErrorKind::Internal,
                            source: eyre!("Cannot parse \"{val_str}\" to a bool"),
                        });
                    }
                };
                ConfigurableValue::Boolean(value)
            }
            ConfigurableValueType::Float { .. } => {
                let value = match val_str.parse::<f32>() {
                    Ok(val) => val,
                    Err(_) => {
                        return Err(Error {
                            kind: ErrorKind::Internal,
                            source: eyre!("Cannot parse \"{val_str}\" to a number"),
                        });
                    }
                };
                ConfigurableValue::Float(value)
            }
            ConfigurableValueType::Enum { .. } => {
                ConfigurableValue::Enum(val_str[1..val_str_len - 1].to_string())
            }
            _ => panic!("TS config parsing error: invalid type not caught by the type parser"),
        })
    } else {
        None
    };

    let mut settings_id = setting_id_prefix.to_string();
    settings_id.push('|');
    settings_id.push_str(var_name);
    Ok(SettingManifest::new_value_with_type(
        settings_id,
        var_name.to_string(),
        config_description.to_string(),
        default_val.clone(),
        config_type,
        default_val,
        false,
        true,
    ))
}

fn get_config_value_type(type_str: &str) -> Result<ConfigurableValueType, Error> {
    let result = match type_str {
        "string" => ConfigurableValueType::String { regex: None },
        "boolean" => ConfigurableValueType::Boolean,
        "number" => ConfigurableValueType::Float {
            max: None,
            min: None,
        },
        _ => {
            // try to parse it into an enum
            let enum_options: Vec<_> = type_str.split('|').collect();
            let mut options: Vec<String> = Vec::new();

            for option in enum_options {
                // verify the enum options are strings
                let first_quote_index = {
                    if let Some(i) = option.find('\'') {
                        i
                    } else if let Some(i) = option.find('"') {
                        i
                    } else {
                        return Err(Error {
                            kind: ErrorKind::Internal,
                            source: eyre!("cannot parse type \"{}\"", type_str),
                        });
                    }
                };

                if first_quote_index == 0 {
                    let str_len = option.len();
                    options.push(option[1..str_len - 1].to_string());
                } else {
                    return Err(Error {
                        kind: ErrorKind::Internal,
                        source: eyre!("cannot parse type \"{}\"", type_str),
                    });
                }
            }

            ConfigurableValueType::Enum { options }
        }
    };
    Ok(result)
}

#[cfg(test)]
mod tests {

    use std::rc::Rc;

    use deno_core::op;

    use super::{TypescriptModuleLoader, WorkerOptionGenerator};

    use crate::event_broadcaster::EventBroadcaster;
    use crate::events::CausedBy;
    use crate::macro_executor::{extract_config_code, get_config_from_code, parse_config_single, SpawnResult};
    use crate::traits::t_configurable::manifest::ConfigurableValue;

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
        tracing_subscriber::fmt::try_init();
        let (event_broadcaster, _rx) = EventBroadcaster::new(10);
        // construct a macro executor
        let executor =
            super::MacroExecutor::new(event_broadcaster, tokio::runtime::Handle::current());

        // create a temp directory
        let temp_dir = tempdir::TempDir::new("macro_test").unwrap().into_path();

        // create a macro file

        let path_to_macro = temp_dir.join("test.ts");

        std::fs::write(
            &path_to_macro,
            r#"
            const core = Deno[Deno.internal].core;
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
            )
            .await
            .unwrap();
        exit_future.await.unwrap();
    }

    #[tokio::test]
    async fn test_http_url() {
        tracing_subscriber::fmt::try_init();

        let (event_broadcaster, _rx) = EventBroadcaster::new(10);
        // construct a macro executor
        let executor =
            super::MacroExecutor::new(event_broadcaster, tokio::runtime::Handle::current());

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
            )
            .await
            .unwrap();
        exit_future.await.unwrap();
    }

    #[test]
    fn test_macro_config_extraction() {
        // should return None if no there is no config definition
        let result = extract_config_code(
            r#"
            console.log("hello world");
            const message = "hello macro";
            console.debug(message);
            "#
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);

        // should return an error if the instance declaration is missing
        let result = extract_config_code(
            r#"
            class LodestoneConfig {
                id: string = 'defaultId';
            }
            "#
        );
        assert!(result.is_err());

        // should return an error if the class definition is missing
        let result = extract_config_code(
            r#"
            declare let config: LodestoneConfig;
            console.debug(config);
            "#
        );
        assert!(result.is_err());

        // should extract the correct instance name and class definition
        let result = extract_config_code(
            r#"
            class LodestoneConfig {
                id: string = 'defaultId';
            }
            declare let config: LodestoneConfig;
            console.debug(config);
            "#
        );
        assert!(result.is_ok());
        let (name, code) = result.unwrap().unwrap();
        assert_eq!(
            &code,
            r#"{
                id: string = 'defaultId';
            }"#
        );
        assert_eq!(&name, "config");
    }

    #[test]
    fn test_macro_config_single_parsing() {
        // should return an error if a non-option variable does not have default value
        let result = parse_config_single(
            "id:string",
            "",
            "",
        );
        assert!(result.is_err());

        // should return an error if the value and type does not match
        let result = parse_config_single(
            "id:number='defaultId'",
            "",
            "prefix",
        );
        assert!(result.is_err());

        // should properly parse the optional variable
        let result = parse_config_single(
            "id?:string",
            "",
            "prefix",
        );
        let config = result.unwrap();
        assert!(config.get_value().is_none());
        assert_eq!(config.get_identifier(), "prefix|id");

        // should properly parse the non-optional variable
        let result = parse_config_single(
            "id:string='defaultId'",
            "",
            "prefix",
        );
        let config = result.unwrap();
        let value = config.get_value().unwrap();
        match value {
            ConfigurableValue::String(val) => assert_eq!(val, "defaultId"),
            _ => panic!("incorrect value")
        }
        assert_eq!(config.get_identifier(), "prefix|id");
    }

    #[test]
    fn test_macro_config_multi_parsing() {
        let result = get_config_from_code(
            "config",
            r#"{
                id: string = 'defaultId';
                interval?: number;
            }"#
        ).unwrap();
        let identifiers = ["config|id", "config|interval"];
        let configs: Vec<_> = result.iter().collect();
        for (_, settings) in configs {
            assert_ne!(identifiers.iter().find(|&val| val == settings.get_identifier()), None);
        }
    }
}

mod deno_errors {
    // Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

    //! There are many types of errors in Deno:
    //! - AnyError: a generic wrapper that can encapsulate any type of error.
    //! - JsError: a container for the error message and stack trace for exceptions
    //!   thrown in JavaScript code. We use this to pretty-print stack traces.
    //! - Diagnostic: these are errors that originate in TypeScript's compiler.
    //!   They're similar to JsError, in that they have line numbers. But
    //!   Diagnostics are compile-time type errors, whereas JsErrors are runtime
    //!   exceptions.

    use deno_ast::Diagnostic;
    use deno_core::error::AnyError;
    use deno_graph::ModuleError;
    use deno_graph::ModuleGraphError;
    use deno_graph::ResolutionError;
    use import_map::ImportMapError;

    fn get_import_map_error_class(_: &ImportMapError) -> &'static str {
        "URIError"
    }

    fn get_diagnostic_class(_: &Diagnostic) -> &'static str {
        "SyntaxError"
    }

    fn get_module_graph_error_class(err: &ModuleGraphError) -> &'static str {
        match err {
            ModuleGraphError::ModuleError(err) => match err {
                ModuleError::LoadingErr(_, _, err) => get_error_class_name(err.as_ref()),
                ModuleError::InvalidTypeAssertion { .. } => "SyntaxError",
                ModuleError::ParseErr(_, diagnostic) => get_diagnostic_class(diagnostic),
                ModuleError::UnsupportedMediaType { .. }
                | ModuleError::UnsupportedImportAssertionType { .. } => "TypeError",
                ModuleError::Missing(_, _) | ModuleError::MissingDynamic(_, _) => "NotFound",
            },
            ModuleGraphError::ResolutionError(err) => get_resolution_error_class(err),
        }
    }

    fn get_resolution_error_class(err: &ResolutionError) -> &'static str {
        match err {
            ResolutionError::ResolverError { error, .. } => get_error_class_name(error.as_ref()),
            _ => "TypeError",
        }
    }

    pub fn get_error_class_name(e: &AnyError) -> &'static str {
        deno_runtime::errors::get_error_class_name(e)
            .or_else(|| {
                e.downcast_ref::<ImportMapError>()
                    .map(get_import_map_error_class)
            })
            .or_else(|| e.downcast_ref::<Diagnostic>().map(get_diagnostic_class))
            .or_else(|| {
                e.downcast_ref::<ModuleGraphError>()
                    .map(get_module_graph_error_class)
            })
            .or_else(|| {
                e.downcast_ref::<ResolutionError>()
                    .map(get_resolution_error_class)
            })
            .unwrap_or_else(|| "Error")
    }
}
