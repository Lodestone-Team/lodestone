use std::path::PathBuf;

use color_eyre::eyre::Context;
use url::Url;

use self::r#macro::GenericMainWorkerGenerator;
use crate::{
    error::Error,
    events::{CausedBy, Event},
    macro_executor::MacroExecutor,
    traits::t_configurable::{manifest::ConfigurableManifest, TConfigurable},
    types::DotLodestoneConfig,
};

mod bridge;
pub mod configurable;
mod r#macro;
pub mod player;
pub mod server;

#[derive(Clone)]
pub struct GenericInstance {
    dot_lodestone_config: DotLodestoneConfig,
    global_event_broadcaster: tokio::sync::broadcast::Sender<Event>,
    procedure_bridge: bridge::procedure_call::ProcedureBridge,
    core_macro_executor: MacroExecutor,
    path: PathBuf,
}

impl GenericInstance {
    pub async fn new(
        link_to_source: String,
        path: PathBuf,
        dot_lodestone_config: DotLodestoneConfig,
        global_event_broadcaster: tokio::sync::broadcast::Sender<Event>,
        core_macro_executor: MacroExecutor,
    ) -> Result<(Self, ConfigurableManifest), Error> {
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
            global_event_broadcaster,
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
            )
            .await
            .unwrap();
        let configurable_manifest = __self.configurable_manifest().await;
        Ok((__self, configurable_manifest))
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_create_generic_instance() {
        // let (event_tx, mut rx) = tokio::sync::broadcast::channel(100);
        // let core_macro_executor = MacroExecutor::new(event_tx.clone());
        // let mut instance = GenericInstance::new(
        //     SetupConfig::from(GenericSetupConfigPrimitive {
        //         name: "test".to_string(),
        //         description: None,
        //         port: 25565,
        //         auto_start: None,
        //         restart_on_crash: None,
        //         timeout_last_left: None,
        //         timeout_no_activity: None,
        //         start_on_connection: None,
        //     }),
        //     event_tx,
        //     core_macro_executor,
        //     PathBuf::from("./InstanceTest/instances/generic_instance_test"),
        // )
        // .await
        // .unwrap();

        // tokio::spawn(async move {
        //     loop {
        //         let event = rx.recv().await.unwrap();
        //         println!("Event on its way to WS: {:#?}", event);
        //     }
        // });

        // instance.start(CausedBy::Unknown, false).await.unwrap();

        // tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // instance.kill(CausedBy::Unknown).await.unwrap();

        // tokio::time::sleep(std::time::Duration::from_secs(100)).await;
    }
}
