use axum::{
    extract::Path,
    routing::{get, put, post},
    Json, Router,
};

use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    events::CausedBy,
    macro_executor::MacroPID,
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
    types::InstanceUuid,
    AppState,
};
use crate::traits::t_configurable::manifest::SettingManifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigResponse {
    config: IndexMap<String, SettingManifest>,
    message: Option<String>,
    error: Option<ErrorKind>,
}

pub async fn get_instance_task_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<TaskEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let tasks = instance.get_task_list().await?;
    Ok(Json(tasks))
}

pub async fn get_instance_macro_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<MacroEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let macros = instance.get_macro_list().await?;
    Ok(Json(macros))
}

pub async fn get_instance_history_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<HistoryEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let history = instance.get_history_list().await?;
    Ok(Json(history))
}

pub async fn run_macro(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(args): Json<Vec<String>>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    if let Ok(valid_config) = instance.validate_local_config(&macro_name, None).await {
        let valid_config = if valid_config.is_empty() {
            None
        } else {
            Some(valid_config)
        };

        instance
            .run_macro(
                &macro_name,
                args,
                valid_config,
                CausedBy::User {
                    user_id: requester.uid,
                    user_name: requester.username,
                },
            )
            .await?;

        Ok(Json(()))
    } else {
        Err(Error {
            kind: ErrorKind::Internal,
            source: eyre!("Config error"),
        })
    }
}

pub async fn kill_macro(
    Path((uuid, pid)): Path<(InstanceUuid, MacroPID)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    instance.kill_macro(pid).await?;
    Ok(Json(()))
}

pub async fn get_macro_configs(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<GetConfigResponse>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;

    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    let mut config = instance.get_macro_config(&macro_name).await?;

    match instance.validate_local_config(&macro_name, Some(&config)).await {
        Ok(local_value) => {
            local_value.iter().for_each(|(setting_id, local_cache)| {
                config[setting_id].set_optional_value(local_cache.get_value().clone()).unwrap();
            });
            Ok(Json(GetConfigResponse{ config, message: None, error: None }))
        },
        Err(e) => {
            match e.kind {
                ErrorKind::NotFound => {
                    Ok(Json(GetConfigResponse { config, message: Some("Local config cache not found".to_string()), error: Some(ErrorKind::NotFound) }))
                },
                _ => {
                    Ok(Json(GetConfigResponse { config, message: Some("There is a mismatch between a config type and its locally-stored value".to_string()), error: Some(ErrorKind::Internal) }))
                }
            }
        },
    }
}

pub async fn store_config_to_local(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(config_to_store): Json<IndexMap<String, SettingManifest>>,
) -> Result<(), Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;

    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    instance.store_macro_config_to_local(&macro_name, &config_to_store).await?;
    Ok(())
}

pub fn get_instance_macro_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/macro/run/:macro_name", put(run_macro))
        .route("/instance/:uuid/macro/kill/:pid", put(kill_macro))
        .route("/instance/:uuid/macro/list", get(get_instance_macro_list))
        .route("/instance/:uuid/macro/config/get/:macro_name", get(get_macro_configs))
        .route("/instance/:uuid/macro/config/store/:macro_name", post(store_config_to_local))
        .route("/instance/:uuid/task/list", get(get_instance_task_list))
        .route(
            "/instance/:uuid/history/list",
            get(get_instance_history_list),
        )
        .with_state(state)
}
