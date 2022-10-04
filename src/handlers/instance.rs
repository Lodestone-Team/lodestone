use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;
use axum::{extract::Path, Extension, Json};

use futures::future::join_all;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::implementations::minecraft::{Flavour, SetupConfig};
use crate::prelude::PATH_TO_INSTANCES;
use crate::traits::InstanceInfo;

use crate::{
    implementations::minecraft,
    traits::{t_server::State, Error, ErrorInner},
    AppState,
};

pub async fn get_instance_list(
    Extension(state): Extension<AppState>,
) -> Result<Json<Vec<InstanceInfo>>, Error> {
    let mut list_of_configs: Vec<InstanceInfo> = join_all(state.instances.lock().await.iter().map(
        |(_, instance)| async move {
            // want id, name, playercount, maxplayer count, port, state and type
            let instance = instance.lock().await;
            instance.get_instance_info().await
        },
    ))
    .await
    .into_iter()
    .collect();

    list_of_configs.sort_by(|a, b| a.creation_time.cmp(&b.creation_time));

    Ok(Json(list_of_configs))
}

pub async fn get_instance_info(
    Path(uuid): Path<String>,
    Extension(state): Extension<AppState>,
) -> Result<Json<InstanceInfo>, Error> {
    Ok(Json(
        state
            .instances
            .lock()
            .await
            .get(&uuid)
            .ok_or(Error {
                inner: ErrorInner::InstanceNotFound,
                detail: "".to_string(),
            })?
            .lock()
            .await
            .get_instance_info()
            .await,
    ))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MinecraftSetupConfigPrimitive {
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub port: u32,
    pub cmd_args: Option<Vec<String>>,
    pub description: Option<String>,
    pub fabric_loader_version: Option<String>,
    pub fabric_installer_version: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: Option<bool>,
    pub backup_period: Option<u32>,
}

impl From<MinecraftSetupConfigPrimitive> for SetupConfig {
    fn from(config: MinecraftSetupConfigPrimitive) -> Self {
        let uuid = uuid::Uuid::new_v4().to_string();
        SetupConfig {
            name: config.name.clone(),
            version: config.version,
            flavour: config.flavour,
            port: config.port,
            cmd_args: config.cmd_args,
            description: config.description,
            fabric_loader_version: config.fabric_loader_version,
            fabric_installer_version: config.fabric_installer_version,
            min_ram: config.min_ram,
            max_ram: config.max_ram,
            auto_start: config.auto_start,
            restart_on_crash: config.restart_on_crash,
            timeout_last_left: config.timeout_last_left,
            timeout_no_activity: config.timeout_no_activity,
            start_on_connection: config.start_on_connection,
            backup_period: config.backup_period,
            game_type: "minecraft".to_string(),
            uuid: uuid.clone(),
            path: PATH_TO_INSTANCES
                .with(|path| path.join(format!("{}-{}", config.name, uuid[0..8].to_string()))),
        }
    }
}
pub async fn create_minecraft_instance(
    Extension(state): Extension<AppState>,
    Json(mut primitive_setup_config): Json<MinecraftSetupConfigPrimitive>,
) -> Result<Json<String>, Error> {
    println!("Creating minecraft instance");
    primitive_setup_config.name = sanitize_filename::sanitize(&primitive_setup_config.name);
    let mut setup_config: SetupConfig = primitive_setup_config.into();
    let mut name = setup_config.name.clone();
    if name.is_empty() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Name must not be empty".to_string(),
        });
    }
    if name.len() > 100 {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Name must not be longer than 100 characters".to_string(),
        });
    }
    name = format!("{}-{}", name, setup_config.uuid[0..5].to_string());
    for (_, instance) in state.instances.lock().await.iter() {
        let path = instance.lock().await.path().await;
        if path == setup_config.path {
            while path == setup_config.path {
                info!("You just hit the lottery");
                setup_config.uuid = uuid::Uuid::new_v4().to_string();
                name = format!("{}-{}", name, setup_config.uuid[0..5].to_string());
                setup_config.name = name.clone();
            }
        }
    }

    let uuid = setup_config.uuid.clone();
    tokio::task::spawn({
        let uuid = uuid.clone();
        async move {
            let minecraft_instance = match minecraft::Instance::new(
                setup_config.clone(),
                state.event_broadcaster.clone(),
            )
            .await
            {
                Ok(v) => v,
                Err(_) => {
                    tokio::fs::remove_dir_all(setup_config.path)
                        .await
                        .map_err(|e| Error {
                            inner: ErrorInner::FailedToRemoveFileOrDir,
                            detail: format!(
                            "Instance creation failed. Failed to clean up instance directory: {}",
                            e
                        ),
                        });
                    return;
                }
            };
            let mut port_allocator = state.port_allocator.lock().await;
            port_allocator.add_port(setup_config.port);
            state
                .instances
                .lock()
                .await
                .insert(uuid.clone(), Arc::new(Mutex::new(minecraft_instance)));
        }
    });
    Ok(Json(uuid))
}

pub async fn delete_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
    let mut instances = state.instances.lock().await;
    if let Some(instance) = instances.get(&uuid) {
        let instance_lock = instance.lock().await;
        if !(instance_lock.state().await == State::Stopped) {
            Err(Error {
                inner: ErrorInner::InstanceStarted,
                detail: "Instance is running, cannot remove".to_string(),
            })
        } else {
            tokio::fs::remove_dir_all(instance_lock.path().await)
                .await
                .map_err(|e| Error {
                    inner: ErrorInner::FailedToRemoveFileOrDir,
                    detail: format!("Could not remove instance: {}", e),
                })?;

            state
                .port_allocator
                .lock()
                .await
                .deallocate(instance_lock.port().await);
            drop(instance_lock);
            instances.remove(&uuid);
            Ok(Json(json!("OK")))
        }
    } else {
        Err(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: format!("Instance with uuid {} does not exist", uuid),
        })
    }
}

pub fn get_instance_routes() -> Router {
    Router::new()
        .route("/instance/list", get(get_instance_list))
        .route("/instance/minecraft", post(create_minecraft_instance))
        .route("/instance/:uuid", delete(delete_instance))
        .route("/instance/:uuid/info", get(get_instance_info))
}
