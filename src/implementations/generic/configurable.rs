use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
use indexmap::IndexMap;

use super::GenericInstance;
use crate::error::{Error, ErrorKind};
use crate::implementations::generic::bridge::procedure_call::{
    ProcedureCallInner, ProcedureCallResultInner,
};
use crate::traits::t_configurable::manifest::{ConfigurableManifest, ConfigurableValue};
use crate::traits::t_configurable::GameType;
use crate::traits::t_configurable::{Game, TConfigurable};
use crate::InstanceUuid;

#[async_trait]

impl TConfigurable for GenericInstance {
    async fn uuid(&self) -> InstanceUuid {
        self.dot_lodestone_config.uuid().clone()
    }
    async fn name(&self) -> String {
        self.procedure_bridge
            .call(ProcedureCallInner::GetName)
            .await
            .unwrap_or(ProcedureCallResultInner::String("Unknown".to_string()))
            .try_into()
            .unwrap_or("Unknown".to_string())
    }

    async fn game_type(&self) -> Game {
        self.procedure_bridge
            .call(ProcedureCallInner::GetGame)
            .await
            .map_or(
                Game::Generic {
                    game_name: GameType::Generic,
                    game_display_name: "Unknown".to_string(),
                },
                |r| {
                    r.try_into().unwrap_or(Game::Generic {
                        game_name: GameType::Generic,
                        game_display_name: "Unknown".to_string(),
                    })
                },
            )
    }
    async fn version(&self) -> String {
        self.procedure_bridge
            .call(ProcedureCallInner::GetVersion)
            .await
            .unwrap_or(ProcedureCallResultInner::String("Unknown".to_string()))
            .try_into()
            .unwrap_or("Unknown".to_string())
    }
    async fn description(&self) -> String {
        self.procedure_bridge
            .call(ProcedureCallInner::GetDescription)
            .await
            .unwrap_or(ProcedureCallResultInner::String("Unknown".to_string()))
            .try_into()
            .unwrap_or("Unknown".to_string())
    }
    async fn port(&self) -> u32 {
        self.procedure_bridge
            .call(ProcedureCallInner::GetPort)
            .await
            .unwrap_or(ProcedureCallResultInner::Num(0))
            .try_into()
            .unwrap_or(0)
    }
    async fn creation_time(&self) -> i64 {
        self.dot_lodestone_config.creation_time()
    }
    async fn path(&self) -> PathBuf {
        self.path.clone()
    }
    /// does start when lodestone starts
    async fn auto_start(&self) -> bool {
        self.procedure_bridge
            .call(ProcedureCallInner::GetAutoStart)
            .await
            .unwrap_or(ProcedureCallResultInner::Bool(false))
            .try_into()
            .unwrap_or(false)
    }
    async fn restart_on_crash(&self) -> bool {
        self.procedure_bridge
            .call(ProcedureCallInner::GetRestartOnCrash)
            .await
            .unwrap_or(ProcedureCallResultInner::Bool(false))
            .try_into()
            .unwrap_or(false)
    }
    // setters
    async fn set_name(&mut self, name: String) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetName { new_name: name })
            .await?;
        Ok(())
    }
    async fn set_description(&mut self, description: String) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetDescription {
                new_description: description,
            })
            .await?;
        Ok(())
    }
    async fn set_port(&mut self, port: u32) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetPort { new_port: port })
            .await?;
        Ok(())
    }
    async fn set_auto_start(&mut self, auto_start: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetAutoStart {
                new_auto_start: auto_start,
            })
            .await?;
        Ok(())
    }
    async fn set_restart_on_crash(&mut self, restart_on_crash: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetRestartOnCrash {
                new_restart_on_crash: restart_on_crash,
            })
            .await?;
        Ok(())
    }
    async fn set_backup_period(&mut self, _backup_period: Option<u32>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting backup period"),
        })
    }

    async fn change_version(&mut self, _version: String) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support changing version"),
        })
    }

    async fn configurable_manifest(&mut self) -> ConfigurableManifest {
        self.procedure_bridge
            .call(ProcedureCallInner::GetConfigurableManifest)
            .await
            .map_or(
                ConfigurableManifest::new(false, false, IndexMap::new()),
                |r| {
                    r.try_into()
                        .unwrap_or(ConfigurableManifest::new(false, false, IndexMap::new()))
                },
            )
    }

    async fn update_configurable(
        &mut self,
        section_id: &str,
        setting_id: &str,
        value: ConfigurableValue,
    ) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::UpdateConfigurable {
                section_id: section_id.to_string(),
                setting_id: setting_id.to_string(),
                new_value: value,
            })
            .await?;
        Ok(())
    }
}
