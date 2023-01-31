use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    traits::t_configurable::TConfigurable,
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
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, InstanceSetting)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
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
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, InstanceSetting)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<Value>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or(Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    match value {
        Value::Null => match key {
            InstanceSetting::BackupPeriod => instance.set_backup_period(None).await,
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot set null to this setting"),
            }),
        },
        Value::Number(n) => {
            let number = n.as_u64().ok_or_else(|| Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot convert number to u64"),
            })? as u32;

            match key {
                InstanceSetting::BackupPeriod => instance.set_backup_period(Some(number)).await,
                InstanceSetting::MaxRam => instance.set_max_ram(number).await,
                InstanceSetting::MinRam => instance.set_min_ram(number).await,
                InstanceSetting::Port => instance.set_port(number).await,
                _ => Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("Cannot set number to this setting"),
                }),
            }
        }
        Value::Bool(b) => match key {
            InstanceSetting::AutoStart => instance.set_auto_start(b).await,
            InstanceSetting::RestartOnCrash => instance.set_restart_on_crash(b).await,
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot set bool to this setting"),
            }),
        },
        Value::String(s) => match key {
            InstanceSetting::Name => instance.set_name(s).await,
            InstanceSetting::Description => instance.set_description(s).await,
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot set string to this setting"),
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
                                        kind: ErrorKind::BadRequest,
                                        source: eyre!("Cannot convert value to string"),
                                    })
                                    .map(|s| s.to_string())
                            })
                            .collect::<Result<Vec<String>, Error>>()?,
                    )
                    .await
            }
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot set array to this setting"),
            }),
        },
        _ => Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Cannot set value to this setting"),
        }),
    }?;

    Ok(Json(()))
}

pub async fn get_game_setting(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<String>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;

    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    Ok(Json(instance.get_field(&key).await?))
}

pub async fn set_game_setting(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, key)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .set_field(&key, value)
        .await?;
    Ok(Json(()))
}

pub async fn change_version(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, new_version)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .change_version(new_version)
        .await?;
    Ok(Json(()))
}

pub fn get_instance_config_routes(state: AppState) -> Router {
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
        .with_state(state)
}
