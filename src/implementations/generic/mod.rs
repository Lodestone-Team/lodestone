use std::path::PathBuf;

use color_eyre::eyre::Context;
use ts_rs::TS;

use self::r#macro::GenericMainWorkerGenerator;
use crate::{
    error::Error,
    events::{CausedBy, Event},
    macro_executor::MacroExecutor,
    types::InstanceUuid,
};

mod bridge;
mod r#macro;
pub mod player;
pub mod server;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[serde(rename = "GenericSetupConfig")]
#[ts(export)]
pub struct SetupConfig {
    pub game_type: String,
    pub uuid: InstanceUuid,
    pub name: String,
    pub description: String,
    pub port: u32,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub started_count: u32,
}

// impl From<GenericSetupConfigPrimitive> for SetupConfig {
//     fn from(val: GenericSetupConfigPrimitive) -> Self {
//         SetupConfig {
//             game_type: "generic".to_string(),
//             uuid: InstanceUuid::default(),
//             name: val.name,
//             description: val.description.unwrap_or_default(),
//             port: val.port,
//             auto_start: val.auto_start.unwrap_or(false),
//             restart_on_crash: val.restart_on_crash.unwrap_or(false),
//             started_count: 0,
//         }
//     }
// }

#[derive(Clone)]
pub struct GenericInstance {
    config: SetupConfig,
    global_event_broadcaster: tokio::sync::broadcast::Sender<Event>,
    procedure_bridge: bridge::procedure_call::ProcedureBridge,
    core_macro_executor: MacroExecutor,
    path: PathBuf,
}

impl GenericInstance {
    pub async fn new(
        config: SetupConfig,
        global_event_broadcaster: tokio::sync::broadcast::Sender<Event>,
        core_macro_executor: MacroExecutor,
        path: PathBuf,
    ) -> Result<Self, Error> {
        let path_to_config = path.join(".lodestone_config");
        let path_to_boostrap = path.join("run.ts");
        std::fs::write(
            &path_to_config,
            serde_json::to_string_pretty(&config).context(
                "Failed to serialize config to string. This is a bug, please report it.",
            )?,
        )
        .context(format!(
            "Failed to write config to {}",
            &path_to_config.display()
        ))?;

        let procedure_bridge =
            bridge::procedure_call::ProcedureBridge::new(global_event_broadcaster.clone());

        let __self = GenericInstance {
            procedure_bridge: procedure_bridge.clone(),
            config: config.clone(),
            global_event_broadcaster,
            core_macro_executor: core_macro_executor.clone(),
            path: path.clone(),
        };

        core_macro_executor
            .spawn(
                path_to_boostrap,
                Vec::new(),
                CausedBy::System,
                Box::new(GenericMainWorkerGenerator::new(
                    procedure_bridge.clone(),
                    __self.clone(),
                )),
                Some(config.uuid.clone()),
            )
            .await
            .unwrap();

        match procedure_bridge
            .call(bridge::procedure_call::ProcedureCallInner::SetupInstance {
                config: config.clone(),
                path: path.clone(),
            })
            .await
        {
            Ok(_) => {
                println!("[RUST] set up successfully")
            }
            Err(e) => {
                println!("[RUST] set up failed: {}", e);
            }
        };

        Ok(__self)
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
