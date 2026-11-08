//! The [`MacroExecutor`] of a core built without the `deno` feature: the same API as the
//! Deno one, but no macro ever starts. [`MacroExecutor::spawn`] and the config parser fail
//! with [`Capability::Macros`]' 501 error, and since nothing runs, no macro is ever running
//! or has an exit status.

use std::path::PathBuf;

use indexmap::IndexMap;

use super::{MacroPID, SpawnResult};
use crate::{
    capabilities::Capability,
    error::{Error, ErrorKind},
    event_broadcaster::EventBroadcaster,
    events::CausedBy,
    global_settings::CodeExecutionSwitch,
    traits::{t_configurable::manifest::SettingManifest, t_macro::ExitStatus},
    types::InstanceUuid,
};

/// Stands in for the Deno worker options; there is nothing to configure.
pub trait WorkerOptionGenerator: Send + Sync {}

pub struct DefaultWorkerOptionGenerator;

impl WorkerOptionGenerator for DefaultWorkerOptionGenerator {}

/// Stands in for Deno's permission options. It has no values: without Deno there is
/// nothing to grant permissions to.
pub enum PermissionsOptions {}

#[derive(Clone, Debug)]
pub struct MacroExecutor {
    code_execution: CodeExecutionSwitch,
}

impl MacroExecutor {
    pub fn new(
        _event_broadcaster: EventBroadcaster,
        _rt: tokio::runtime::Handle,
        code_execution: CodeExecutionSwitch,
    ) -> MacroExecutor {
        MacroExecutor { code_execution }
    }

    /// The code execution setting, which also covers custom Java commands, so it is
    /// still kept without Deno.
    pub fn code_execution_enabled(&self) -> bool {
        self.code_execution.is_enabled()
    }

    /// Fails with 403 while code execution is turned off.
    pub fn require_code_execution(&self) -> Result<(), Error> {
        self.code_execution.check()
    }

    /// Always fails: this core cannot run macros.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        &self,
        _path_to_main_module: PathBuf,
        _args: Vec<String>,
        _caused_by: CausedBy,
        _worker_options_generator: Box<dyn WorkerOptionGenerator>,
        _pre_injection_code: Option<String>,
        _permissions: Option<PermissionsOptions>,
        _instance_uuid: Option<InstanceUuid>,
    ) -> Result<SpawnResult, Error> {
        Err(Capability::Macros.unavailable())
    }

    pub fn abort_macro(&self, pid: MacroPID) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::NotFound,
            source: color_eyre::eyre::eyre!("No running macro with pid {}", pid),
        })
    }

    pub async fn get_macro_status(&self, _pid: MacroPID) -> Option<ExitStatus> {
        None
    }

    pub fn is_running(&self, _pid: MacroPID) -> bool {
        false
    }

    /// Always fails: reading a macro's config needs the TypeScript parser.
    pub async fn get_config_manifest(
        _path: &PathBuf,
    ) -> Result<IndexMap<String, SettingManifest>, Error> {
        Err(Capability::Macros.unavailable())
    }

    pub fn shutdown_all(&self) {}
}
