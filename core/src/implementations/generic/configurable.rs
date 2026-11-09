use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
use indexmap::IndexMap;
use tracing::warn;

use super::GenericInstance;
use crate::error::{Error, ErrorKind};
use crate::implementations::generic::bridge::procedure_call::{
    ProcedureCallInner, ProcedureCallKind, ProcedureCallResultInner,
};
use crate::traits::t_configurable::manifest::{ConfigurableManifest, ConfigurableValue};
use crate::traits::t_configurable::GameType;
use crate::traits::t_configurable::{Game, TConfigurable};
use crate::InstanceUuid;

impl GenericInstance {
    /// Asks the atom for a value, falling back to `default` (and logging why) if it fails.
    async fn get_or<T>(&self, call: ProcedureCallInner, default: T) -> T
    where
        T: TryFrom<ProcedureCallResultInner, Error = Error>,
    {
        let kind = ProcedureCallKind::from(&call);
        match self.procedure_bridge.call(call).await {
            // the atom does not implement this getter
            Ok(ProcedureCallResultInner::Void) => default,
            Ok(result) => result.try_into().unwrap_or_else(|e: Error| {
                warn!("Atom returned an unexpected value for {kind:?}: {e}");
                default
            }),
            Err(e) => {
                warn!("Atom failed to answer {kind:?}: {e}");
                default
            }
        }
    }
}

#[async_trait]
impl TConfigurable for GenericInstance {
    async fn uuid(&self) -> InstanceUuid {
        self.dot_lodestone_config.uuid().clone()
    }
    async fn name(&self) -> String {
        self.get_or(ProcedureCallInner::GetName, "Unknown".to_string())
            .await
    }

    async fn game_type(&self) -> Game {
        self.get_or(
            ProcedureCallInner::GetGame,
            Game::Generic {
                game_name: GameType::Generic,
                game_display_name: "Unknown".to_string(),
            },
        )
        .await
    }
    async fn version(&self) -> String {
        self.get_or(ProcedureCallInner::GetVersion, "Unknown".to_string())
            .await
    }
    async fn description(&self) -> String {
        self.get_or(ProcedureCallInner::GetDescription, "Unknown".to_string())
            .await
    }
    async fn port(&self) -> u32 {
        self.get_or(ProcedureCallInner::GetPort, 0).await
    }
    async fn creation_time(&self) -> i64 {
        self.dot_lodestone_config.creation_time()
    }
    async fn path(&self) -> PathBuf {
        self.path.clone()
    }
    /// does start when lodestone starts
    async fn auto_start(&self) -> bool {
        self.get_or(ProcedureCallInner::GetAutoStart, false).await
    }
    async fn restart_on_crash(&self) -> bool {
        self.get_or(ProcedureCallInner::GetRestartOnCrash, false)
            .await
    }
    // setters
    async fn set_name(&self, name: String) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetName { new_name: name })
            .await?;
        Ok(())
    }
    async fn set_description(&self, description: String) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetDescription {
                new_description: description,
            })
            .await?;
        Ok(())
    }
    async fn set_port(&self, port: u32) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetPort { new_port: port })
            .await?;
        Ok(())
    }
    async fn set_auto_start(&self, auto_start: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetAutoStart {
                new_auto_start: auto_start,
            })
            .await?;
        Ok(())
    }
    async fn set_restart_on_crash(&self, restart_on_crash: bool) -> Result<(), Error> {
        self.procedure_bridge
            .call(ProcedureCallInner::SetRestartOnCrash {
                new_restart_on_crash: restart_on_crash,
            })
            .await?;
        Ok(())
    }
    async fn set_backup_period(&self, _backup_period: Option<u32>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This server does not support setting backup period"),
        })
    }

    async fn change_version(&self, _version: String) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This server does not support changing version"),
        })
    }

    async fn configurable_manifest(&self) -> ConfigurableManifest {
        self.get_or(
            ProcedureCallInner::GetConfigurableManifest,
            ConfigurableManifest::new(false, false, IndexMap::new()),
        )
        .await
    }

    async fn update_configurable(
        &self,
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
