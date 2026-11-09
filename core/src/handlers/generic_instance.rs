//! Creating atom (generic TypeScript) instances. Only with the `deno` feature; without it
//! `POST /instance/create_generic` answers 501, see [`crate::capabilities`].

use axum::extract::State;
use axum::Json;
use color_eyre::eyre::Context;
use serde::Deserialize;
use tracing::{error, info};

use crate::events::{Event, ProgressionEndValue, ProgressionStartValue};
use crate::implementations::generic;
use crate::prelude::path_to_instances;
use crate::traits::t_configurable::{manifest::SetupValue, GameType};
use crate::traits::TInstance;
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::{error::Error, AppState};

use super::instance::{instance_setup_path, remove_failed_instance_dir};
use super::util::{check_create_atom, AuthedUser};

#[derive(Debug, Clone, Deserialize)]
pub struct GenericSetupConfig {
    url: String,
    setup_value: SetupValue,
}

pub async fn create_generic_instance(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(setup_config): Json<GenericSetupConfig>,
) -> Result<Json<InstanceUuid>, Error> {
    // an atom is code that runs on the core
    check_create_atom(&requester, &state.code_execution)?;
    let caused_by = requester.caused_by();
    let instance_uuid = InstanceUuid::default();

    let setup_path = instance_setup_path(
        path_to_instances(),
        &setup_config.setup_value.name,
        &instance_uuid,
    );

    tokio::fs::create_dir_all(&setup_path)
        .await
        .context("Failed to create server directory")?;

    let dot_lodestone_config = DotLodestoneConfig::new(instance_uuid.clone(), GameType::Generic);
    let event_broadcaster = state.event_broadcaster.clone();
    let created_uuid = instance_uuid.clone();
    tokio::task::spawn(async move {
        let (progression_start_event, event_id) = Event::new_progression_event_start(
            format!("Setting up server {}", setup_config.setup_value.name),
            Some(100.0),
            Some(ProgressionStartValue::InstanceCreation {
                instance_uuid: instance_uuid.clone(),
            }),
            caused_by,
        );
        event_broadcaster.send(progression_start_event);
        let instance = match generic::GenericInstance::new(
            setup_config.url.into(),
            setup_path.clone(),
            dot_lodestone_config.clone(),
            setup_config.setup_value,
            &event_id,
            state.event_broadcaster.clone(),
            state.macro_executor.clone(),
        )
        .await
        {
            Ok(v) => {
                info!("Atom created successfully");
                event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    true,
                    Some("Server created successfully"),
                    Some(ProgressionEndValue::InstanceCreation(
                        v.get_instance_info().await,
                    )),
                ));
                v
            }
            Err(e) => {
                error!("Atom creation failed: {:?}", e);
                event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    false,
                    Some(&format!("Server creation failed: {e}")),
                    None,
                ));
                remove_failed_instance_dir(setup_path).await;
                return;
            }
        };

        // write dot lodestone config
        let written = match serde_json::to_string_pretty(&dot_lodestone_config) {
            Ok(config) => tokio::fs::write(setup_path.join(".lodestone_config"), config)
                .await
                .context("Failed to write .lodestone_config file"),
            Err(e) => Err(e).context("Failed to serialize .lodestone_config"),
        };
        if let Err(e) = written {
            // without the config file the instance would be lost on the next start
            error!(
                "Failed to save instance {}: {:#}. The instance was not added",
                instance_uuid, e
            );
            return;
        }

        if let Err(e) = state
            .instance_folders
            .record([(instance_uuid.clone(), setup_path)])
            .await
        {
            error!("Failed to record the folder of instance {instance_uuid}: {e}");
        }
        state
            .instances
            .insert(instance_uuid.clone(), instance.into());
    });

    Ok(Json(created_uuid))
}
