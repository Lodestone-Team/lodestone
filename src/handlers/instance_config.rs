use std::iter::Map;

use axum::{extract::Path, routing::get, Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{
    traits::{Error, ErrorInner},
    AppState,
};

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum InstanceSetting {
    Uuid,
    Name,
    Flavour,
    GameType,
    CmdArgs,
    Description,
    Port,
    MinRam,
    MaxRam,
    CreationTime,
    Path,
    AutoStart,
    RestartOnCrash,
    TimeoutLastLeft,
    TimeoutNoActivity,
    StartOnConnection,
    BackupPeriod,
}

pub async fn get_instance_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(String, InstanceSetting)>,
) -> Result<Json<Value>, Error> {
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    Ok(Json(match key {
        InstanceSetting::Uuid => json!(instance.uuid().await),
        InstanceSetting::Name => json!(instance.name().await),
        InstanceSetting::Flavour => json!(instance.flavour().await),
        InstanceSetting::GameType => json!(instance.game_type().await),
        InstanceSetting::CmdArgs => json!(instance.cmd_args().await),
        InstanceSetting::Description => json!(instance.description().await),
        InstanceSetting::Port => json!(instance.port().await),
        InstanceSetting::MinRam => json!(instance.min_ram().await),
        InstanceSetting::MaxRam => json!(instance.max_ram().await),
        InstanceSetting::CreationTime => json!(instance.creation_time().await),
        InstanceSetting::Path => json!(instance.path().await.display().to_string()),
        InstanceSetting::AutoStart => json!(instance.auto_start().await),
        InstanceSetting::RestartOnCrash => json!(instance.restart_on_crash().await),
        InstanceSetting::TimeoutLastLeft => json!(instance.timeout_last_left().await),
        InstanceSetting::TimeoutNoActivity => json!(instance.timeout_no_activity().await),
        InstanceSetting::StartOnConnection => json!(instance.start_on_connection().await),
        InstanceSetting::BackupPeriod => json!(instance.backup_period().await),
    }))
}

pub async fn set_instance_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(String, InstanceSetting)>,
    Json(value): Json<Value>,
) -> Result<Json<String>, Error> {
    let instances = state.instances.lock().await;
    let mut instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;

    match value {
        Value::Null => match key {
            InstanceSetting::TimeoutLastLeft => instance.set_timeout_last_left(None).await,
            InstanceSetting::TimeoutNoActivity => instance.set_timeout_no_activity(None).await,
            InstanceSetting::BackupPeriod => instance.set_backup_period(None).await,
            _ => Some(Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })),
        },
        Value::Number(n) => {
            let number = n.as_u64().ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })? as u32;

            match key {
                InstanceSetting::TimeoutLastLeft => {
                    instance.set_timeout_last_left(Some(number)).await
                }
                InstanceSetting::TimeoutNoActivity => {
                    instance.set_timeout_no_activity(Some(number)).await
                }
                InstanceSetting::BackupPeriod => instance.set_backup_period(Some(number)).await,
                InstanceSetting::MaxRam => instance.set_max_ram(number).await,
                InstanceSetting::MinRam => instance.set_min_ram(number).await,
                InstanceSetting::Port => instance.set_port(number).await,
                _ => Some(Err(Error {
                    inner: ErrorInner::MalformedRequest,
                    detail: "".to_string(),
                })),
            }
        }
        Value::Bool(b) => match key {
            InstanceSetting::AutoStart => instance.set_auto_start(b).await,
            InstanceSetting::RestartOnCrash => instance.set_restart_on_crash(b).await,
            InstanceSetting::StartOnConnection => instance.set_start_on_connection(b).await,
            _ => Some(Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })),
        },
        Value::String(s) => match key {
            InstanceSetting::Name => Some(instance.set_name(s).await),
            InstanceSetting::Description => Some(instance.set_description(s).await),
            _ => Some(Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })),
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
            _ => Some(Err(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })),
        },
        _ => Some(Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "".to_string(),
        })),
    }.ok_or(Error {
        inner: ErrorInner::UnsupportedOperation,
        detail: "".to_string(),
    })??;

    Ok(Json("ok".to_string()))
}

pub async fn get_game_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(String, String)>,
) -> Result<Json<String>, Error> {
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    Ok(Json(instance.get_field(&key).await?))
}

pub async fn set_game_setting(
    Extension(state): Extension<AppState>,
    Path((uuid, key)): Path<(String, String)>,
    Json(value): Json<String>,
) -> Result<Json<String>, Error> {
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
        .set_field(&key, value)
        .await?;
    Ok(Json("ok".to_string()))
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
}
