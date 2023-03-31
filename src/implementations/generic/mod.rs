use std::{path::PathBuf, rc::Rc};

use async_trait::async_trait;
use color_eyre::eyre::Context;
use url::Url;

use self::{
    bridge::procedure_call::{emit_result, on_procedure, proc_bridge_ready, ProcedureCallInner},
    r#macro::GenericMainWorkerGenerator,
};
use crate::{
    error::Error,
    event_broadcaster::EventBroadcaster,
    events::CausedBy,
    macro_executor::{self, MacroExecutor, MainWorkerGenerator},
    traits::{
        t_configurable::{
            manifest::{ManifestValue, SetupManifest},
            TConfigurable,
        },
        t_player::TPlayerManagement,
        t_server::TServer,
        InstanceInfo, TInstance,
    },
    types::DotLodestoneConfig,
};
use std::io::Write;

mod bridge;
pub mod configurable;
mod r#macro;
pub mod player;
pub mod resource;
pub mod server;

#[derive(Clone)]
pub struct GenericInstance {
    dot_lodestone_config: DotLodestoneConfig,
    event_broadcaster: EventBroadcaster,
    procedure_bridge: bridge::procedure_call::ProcedureBridge,
    core_macro_executor: MacroExecutor,
    path: PathBuf,
}

struct InitWorkerGenerator {
    pub bridge: bridge::procedure_call::ProcedureBridge,
}

impl MainWorkerGenerator for InitWorkerGenerator {
    fn generate(
        &self,
        args: Vec<String>,
        _caused_by: CausedBy,
    ) -> deno_runtime::worker::MainWorker {
        let bootstrap_options = deno_runtime::BootstrapOptions {
            args,
            ..Default::default()
        };

        let mut worker_options = deno_runtime::worker::WorkerOptions {
            bootstrap: bootstrap_options,
            ..Default::default()
        };

        let ext = deno_core::Extension::builder("generic_deno_extension_builder")
            .ops(vec![
                on_procedure::decl(),
                emit_result::decl(),
                proc_bridge_ready::decl(),
            ])
            .state({
                let brige = self.bridge.clone();
                move |state| {
                    state.put(brige);
                }
            })
            .force_op_registration()
            .build();
        worker_options.extensions.push(ext);
        worker_options.module_loader = Rc::new(macro_executor::TypescriptModuleLoader::default());

        let main_module = deno_core::resolve_path(".", &std::env::current_dir().unwrap())
            .expect("Failed to resolve path");
        // todo(CheatCod3) : limit the permissions
        let permissions = deno_runtime::permissions::Permissions::allow_all();
        deno_runtime::worker::MainWorker::bootstrap_from_options(
            main_module,
            deno_runtime::permissions::PermissionsContainer::new(permissions),
            worker_options,
        )
    }
}

impl GenericInstance {
    pub async fn new(
        link_to_source: String,
        path: PathBuf,
        dot_lodestone_config: DotLodestoneConfig,
        setup_value: ManifestValue,
        event_broadcaster: EventBroadcaster,
        core_macro_executor: MacroExecutor,
    ) -> Result<Self, Error> {
        tokio::fs::create_dir_all(&path).await.context(format!(
            "Failed to create directory for instance at {}",
            &path.display()
        ))?;
        let path_to_config = path.join(".lodestone_config");
        let run_ts_content = format!(
            r#"import {{ run }} from "{}";
                run();
            "#,
            Url::parse(&link_to_source)
                .context("Invalid URL")?
                .join("mod.ts")
                .context("Invalid URL")?
                .as_str()
        );

        let path_to_bootstrap = path.join("run.ts");
        tokio::fs::write(&path_to_bootstrap, run_ts_content)
            .await
            .context(format!(
                "Failed to write bootstrap to {}",
                &path_to_bootstrap.display()
            ))?;
        std::fs::write(
            &path_to_config,
            serde_json::to_string_pretty(&dot_lodestone_config).context(
                "Failed to serialize config to string. This is a bug, please report it.",
            )?,
        )
        .context(format!(
            "Failed to write config to {}",
            &path_to_config.display()
        ))?;

        let procedure_bridge = bridge::procedure_call::ProcedureBridge::new();

        let __self = GenericInstance {
            dot_lodestone_config: dot_lodestone_config.clone(),
            procedure_bridge: procedure_bridge.clone(),
            event_broadcaster,
            core_macro_executor: core_macro_executor.clone(),
            path: path.clone(),
        };

        core_macro_executor
            .spawn(
                path_to_bootstrap,
                Vec::new(),
                CausedBy::System,
                Box::new(GenericMainWorkerGenerator::new(
                    procedure_bridge.clone(),
                    __self.clone(),
                )),
                Some(dot_lodestone_config.uuid().clone()),
                None,
            )
            .await?;
        procedure_bridge
            .call(ProcedureCallInner::SetupInstance {
                dot_lodestone_config,
                setup_value,
                path,
            })
            .await?;
        Ok(__self)
    }

    pub async fn setup_manifest(
        link_to_source: &str,
        macro_executor: MacroExecutor,
    ) -> Result<SetupManifest, Error> {
        // create a tempfile
        let mut temp_file = tempfile::NamedTempFile::new().context("Failed to create temp file")?;
        let run_ts_content = format!(
            r#"import {{ run }} from "{}";
                run();
            "#,
            Url::parse(link_to_source)
                .context("Invalid URL")?
                .join("mod.ts")
                .context("Invalid URL")?
                .as_str()
        );
        writeln!(temp_file, "{}", run_ts_content).context("Failed to write to temp file")?;
        let procedure_bridge = bridge::procedure_call::ProcedureBridge::new();
        macro_executor
            .spawn(
                temp_file.path().to_owned(),
                Vec::new(),
                CausedBy::System,
                Box::new(InitWorkerGenerator {
                    bridge: procedure_bridge.clone(),
                }),
                None,
                None,
            )
            .await?;

        procedure_bridge
            .call(ProcedureCallInner::GetSetupManifest)
            .await?
            .try_into()
    }
}

#[async_trait]
impl TInstance for GenericInstance {
    async fn get_instance_info(&self) -> InstanceInfo {
        InstanceInfo {
            uuid: self.uuid().await,
            name: self.name().await,
            game_type: self.game_type().await,
            description: self.description().await,
            version: self.version().await,
            port: self.port().await,
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
            auto_start: self.auto_start().await,
            restart_on_crash: self.restart_on_crash().await,
            state: self.state().await,
            player_count: self.get_player_count().await.ok(),
            max_player_count: self.get_max_player_count().await.ok(),
            player_list: self.get_player_list().await.ok(),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use std::path::PathBuf;

//     use crate::{
//         event_broadcaster::EventBroadcaster,
//         events::CausedBy,
//         implementations::generic::GenericInstance,
//         macro_executor::MacroExecutor,
//         types::{DotLodestoneConfig, InstanceUuid},
//     };

//     use crate::traits::t_configurable::GameType;

//     #[tokio::test]
//     async fn test_create_generic_instance() {
//         let _ = tracing_subscriber::fmt::try_init();
//         let (event_tx, mut rx) = EventBroadcaster::new(100);
//         let core_macro_executor = MacroExecutor::new(event_tx.clone());
//         let mut instance = GenericInstance::new(
//             "file://home/peter/dev/backend/src/implementations/generic/js/main/".to_string(),
//             PathBuf::from("./InstanceTest/instances/generic_instance_test"),
//             DotLodestoneConfig::new(InstanceUuid::default(), GameType::Generic),
//             event_tx,
//             core_macro_executor,
//         )
//         .await
//         .unwrap();

//         tokio::spawn(async move {
//             loop {
//                 let event = rx.recv().await.unwrap();
//                 println!("Event on its way to WS: {:#?}", event);
//             }
//         });

//         tokio::time::sleep(std::time::Duration::from_secs(100)).await;
//     }
// }
