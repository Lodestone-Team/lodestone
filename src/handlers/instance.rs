use std::{env, sync::Arc};

use axum::{extract::Path, Extension, Json};
use axum_auth::AuthBearer;
use futures::future::join_all;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::util::{is_authorized, try_auth};
use crate::json_store::permission::Permission::{self};
use crate::{
    implementations::minecraft,
    traits::{t_server::State, Error, ErrorInner},
    AppState,
};

pub async fn list_instance(Extension(state): Extension<AppState>) -> Result<Json<Value>, Error> {
    let mut list_of_configs = join_all(
        state
            .instances
            .lock()
            .await
            .iter()
            .map(|(_, instance)| async move { instance.lock().await.get_info() }),
    )
    .await
    .into_iter()
    .collect::<Vec<Value>>();

    list_of_configs.sort_by(|a, b| {
        a["creation_time"]
            .as_u64()
            .unwrap()
            .cmp(&b["creation_time"].as_u64().unwrap())
    });
    Ok(Json(json!(list_of_configs)))
}
pub async fn create_instance(
    Extension(state): Extension<AppState>,
    Path(idempotency): Path<String>,
    Json(config): Json<Value>,
) -> Result<Json<Value>, Error> {
    let game_type = config
        .get("type")
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Json must contain type".to_string(),
        })?
        .as_str()
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Type must be string".to_string(),
        })?
        .to_string();

    let name = sanitize_filename::sanitize(
        config
            .get("name")
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Json must contain name".to_string(),
            })?
            .as_str()
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Name must be string".to_string(),
            })?,
    );
    if name.is_empty() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Name must not be empty".to_string(),
        });
    }
    let port = config
        .get("port")
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Json must contain port".to_string(),
        })?
        .as_u64()
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Port must be integer".to_string(),
        })? as u32;
    for v in (*state.instances.lock().await).values() {
        if v.lock()
            .await
            .get_info()
            .get("name")
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Name does not exist for instance".to_string(),
            })?
            .as_str()
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Name is not a string".to_string(),
            })?
            == name
        {
            return Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: format!("Instance with name {} already exists", name),
            });
        }
        if v.lock()
            .await
            .get_info()
            .get("port")
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Port does not exist for instance".to_string(),
            })?
            .as_u64()
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Port is not a integer".to_string(),
            })? as u32
            == port
        {
            return Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: format!("Instance with port {} already exists", port),
            });
        }
    }

    let uuid = uuid::Uuid::new_v4().to_string();

    match game_type.to_ascii_lowercase().as_str() {
        "minecraft" => {
            let mc_config = minecraft::Config {
                r#type: "minecraft".to_string(),
                uuid: uuid.clone(),
                name: name.clone(),
                version: config
                    .get("version")
                    .ok_or(Error {
                        inner: ErrorInner::MalformedRequest,
                        detail: "Json must contain version".to_string(),
                    })?
                    .as_str()
                    .ok_or(Error {
                        inner: ErrorInner::MalformedRequest,
                        detail: "Version must be string".to_string(),
                    })?
                    .to_string(),
                fabric_loader_version: config
                    .get("fabric_loader_version")
                    .map(|v| v.as_str().unwrap().to_string()),
                fabric_installer_version: config
                    .get("fabric_installer_version")
                    .map(|v| v.as_str().unwrap().to_string()),
                flavour: {
                    let flavour = config
                        .get("flavour")
                        .ok_or(Error {
                            inner: ErrorInner::MalformedRequest,
                            detail: "Json must contain flavour".to_string(),
                        })?
                        .to_owned();
                    serde_json::from_value(flavour.clone()).map_err(|_| Error {
                        inner: ErrorInner::MalformedRequest,
                        detail: format!("Flavour {} is not one of the valid options", flavour),
                    })?
                },
                description: config
                    .get("description")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| "Pizza time".to_string()),
                jvm_args: vec![],
                path: env::current_dir().unwrap().join("instances").join(&name),
                port,
                min_ram: config
                    .get("min_ram")
                    .map(|v| v.as_u64().unwrap_or(1024) as u32)
                    .unwrap_or(1024),
                max_ram: config
                    .get("max_ram")
                    .map(|v| v.as_u64().unwrap_or(2048) as u32)
                    .unwrap_or(2048),
                creation_time: chrono::Utc::now().timestamp(),
                auto_start: config
                    .get("auto_start")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false),
                restart_on_crash: config
                    .get("restart_on_crash")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false),
                timeout_last_left: config
                    .get("timeout_last_left")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                timeout_no_activity: config
                    .get("timeout_no_activity")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                start_on_connection: config
                    .get("start_on_connection")
                    .map(|v| v.as_bool().unwrap_or(false))
                    .unwrap_or(false),
                backup_period: config
                    .get("backup_period")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                jre_major_version: None,
            };
            state.instances.lock().await.insert(
                mc_config.uuid.clone(),
                Arc::new(Mutex::new(
                    minecraft::Instance::new(
                        mc_config,
                        state.event_broadcaster.clone(),
                        Some(idempotency),
                    )
                    .await?,
                )),
            );
        }
        _ => todo!(),
    }

    Ok(Json(json!(uuid)))
}

pub async fn remove_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
    let mut instances = state.instances.lock().await;
    if let Some(instance) = instances.get(&uuid) {
        if !(instance.lock().await.state() == State::Stopped) {
            Err(Error {
                inner: ErrorInner::InstanceStarted,
                detail: "Instance is running, cannot remove".to_string(),
            })
        } else {
            tokio::fs::remove_dir_all(instance.lock().await.path())
                .await
                .map_err(|e| Error {
                    inner: ErrorInner::FailedToRemoveFileOrDir,
                    detail: format!("Could not remove instance: {}", e),
                })?;
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

pub async fn start_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "".to_string(),
    })?;
    if !is_authorized(&requester, &uuid, Permission::CanStartInstance) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to start instance".to_string(),
        });
    }
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
        .start()?;
    Ok(Json(json!("ok")))
}

pub async fn stop_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
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
        .stop()?;
    Ok(Json(json!("ok")))
}

pub async fn kill_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
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
        .kill()?;
    Ok(Json(json!("ok")))
}

pub async fn send_command(
    Extension(state): Extension<AppState>,
    Path((uuid, cmd)): Path<(String, String)>,
) -> Result<Json<Value>, Error> {
    match state
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
        .send_command(&cmd)
    {
        crate::traits::MaybeUnsupported::Supported(v) => v.map(|_| Json(json!("ok"))),
        crate::traits::MaybeUnsupported::Unsupported => Err(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        }),
    }
}

pub async fn get_instance_state(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(json!(state
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
        .state())))
}
