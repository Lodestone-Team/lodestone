use axum::{
    extract::Path,
    routing::{get, put},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    traits::{t_configurable::TConfigurable, Error, ErrorInner},
    types::InstanceUuid,
    AppState,
};

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum InstanceSetting {
    Uuid,
    Name,
    Flavour,
    ServerInstance,
    CmdArgs,
    Description,
    Port,
    MinRam,
    MaxRam,
    CreationTime,
    Path,
    AutoStart,
    RestartOnCrash,
    BackupPeriod,
}

pub async fn get_instance_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, InstanceSetting)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessSetting(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to get instance setting".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    Ok(Json(match key {
        InstanceSetting::Uuid => json!(instance.uuid().await),
        InstanceSetting::Name => json!(instance.name().await),
        InstanceSetting::Flavour => json!(instance.flavour().await),
        InstanceSetting::ServerInstance => json!(instance.game_type().await),
        InstanceSetting::CmdArgs => json!(instance.cmd_args().await),
        InstanceSetting::Description => json!(instance.description().await),
        InstanceSetting::Port => json!(instance.port().await),
        InstanceSetting::MinRam => json!(instance.min_ram().await),
        InstanceSetting::MaxRam => json!(instance.max_ram().await),
        InstanceSetting::CreationTime => json!(instance.creation_time().await),
        InstanceSetting::Path => json!(instance.path().await.display().to_string()),
        InstanceSetting::AutoStart => json!(instance.auto_start().await),
        InstanceSetting::RestartOnCrash => json!(instance.restart_on_crash().await),
        InstanceSetting::BackupPeriod => json!(instance.backup_period().await),
    }))
}

pub async fn set_instance_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, InstanceSetting)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<Value>,
) -> Result<Json<()>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessSetting(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change instance setting".to_string(),
        });
    }
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;

    match value {
        Value::Null => match key {
            InstanceSetting::BackupPeriod => instance.set_backup_period(None).await,
            _ => Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            }),
        },
        Value::Number(n) => {
            let number = n.as_u64().ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })? as u32;

            match key {
                InstanceSetting::BackupPeriod => instance.set_backup_period(Some(number)).await,
                InstanceSetting::MaxRam => instance.set_max_ram(number).await,
                InstanceSetting::MinRam => instance.set_min_ram(number).await,
                InstanceSetting::Port => instance.set_port(number).await,
                _ => Err(Error {
                    inner: ErrorInner::MalformedRequest,
                    detail: "".to_string(),
                }),
            }
        }
        Value::Bool(b) => match key {
            InstanceSetting::AutoStart => instance.set_auto_start(b).await,
            InstanceSetting::RestartOnCrash => instance.set_restart_on_crash(b).await,
            _ => Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            }),
        },
        Value::String(s) => match key {
            InstanceSetting::Name => instance.set_name(s).await,
            InstanceSetting::Description => instance.set_description(s).await,
            _ => Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            }),
        },
        Value::Array(a) => match key {
            InstanceSetting::CmdArgs => {
                instance
                    .set_cmd_args(
                        a.into_iter()
                            .map(|v| {
                                v.as_str()
                                    .ok_or(Error {
                                        inner: ErrorInner::MalformedRequest,
                                        detail: "".to_string(),
                                    })
                                    .map(|s| s.to_string())
                            })
                            .collect::<Result<Vec<String>, Error>>()?,
                    )
                    .await
            }
            _ => Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            }),
        },
        _ => Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "".to_string(),
        }),
    }?;

    Ok(Json(()))
}

pub async fn get_game_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<String>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessSetting(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to get game setting".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    Ok(Json(instance.get_field(&key).await?))
}

pub async fn set_game_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessSetting(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change game setting".to_string(),
        });
    }
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .set_field(&key, value)
        .await?;
    Ok(Json(()))
}

pub async fn change_version(
    Extension(state): Extension<AppState>,
    Path((uuid, new_version)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessSetting(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change game setting".to_string(),
        });
    }
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .change_version(new_version)
        .await?;
    Ok(Json(()))
}

pub fn get_instance_config_routes() -> Router {
    Router::new()
        .route(
            "/instance/:uuid/:key",
            get(get_instance_setting).put(set_instance_setting),
        )
        .route(
            "/instance/:uuid/game/:key",
            get(get_game_setting).put(set_game_setting),
        )
        .route("/instance/:uuid/version/:new_version", put(change_version))
}
