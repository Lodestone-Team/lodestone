use axum::{
    extract::Path,
    routing::{get, post, put},
    Json, Router,
};

use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::util::require_code_execution;
use crate::traits::t_configurable::manifest::SettingManifest;
use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    macro_executor::MacroPID,
    prelude::GameInstance,
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
    types::InstanceUuid,
    AppState,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GetConfigResponse {
    pub config: IndexMap<String, SettingManifest>,
    pub message: Option<String>,
    pub error: Option<ErrorKind>,
}

/// Looks up an instance, cloned out so the map entry is not locked while the handler awaits.
fn get_instance(state: &AppState, uuid: &InstanceUuid) -> Result<GameInstance, Error> {
    state
        .instances
        .get(uuid)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Server not found"),
        })
}

pub async fn get_instance_task_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<TaskEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::AccessMacro(Some(uuid.clone())),
        state.safe_mode(),
    )?;
    let instance = get_instance(&state, &uuid)?;
    let tasks = instance.get_task_list().await?;
    Ok(Json(tasks))
}

pub async fn get_instance_macro_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<MacroEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::AccessMacro(Some(uuid.clone())),
        state.safe_mode(),
    )?;
    let instance = get_instance(&state, &uuid)?;
    let macros = instance.get_macro_list().await?;
    Ok(Json(macros))
}

pub async fn get_instance_history_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<HistoryEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::AccessMacro(Some(uuid.clone())),
        state.safe_mode(),
    )?;
    let instance = get_instance(&state, &uuid)?;
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
    requester.try_action(
        &UserAction::AccessMacro(Some(uuid.clone())),
        state.safe_mode(),
    )?;
    require_code_execution(&state)?;
    let instance = get_instance(&state, &uuid)?;

    let valid_config = instance
        .validate_local_config(&macro_name, None)
        .await
        .map_err(|e| Error {
            kind: match e.kind {
                // the macro needs a config that has not been saved yet
                ErrorKind::NotFound => ErrorKind::BadRequest,
                kind => kind,
            },
            source: e
                .source
                .wrap_err(format!("The config of macro {macro_name} is not valid")),
        })?;
    let valid_config = if valid_config.is_empty() {
        None
    } else {
        Some(valid_config)
    };

    instance
        .run_macro(&macro_name, args, valid_config, requester.caused_by())
        .await?;

    Ok(Json(()))
}

pub async fn kill_macro(
    Path((uuid, pid)): Path<(InstanceUuid, MacroPID)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(
        &UserAction::AccessMacro(Some(uuid.clone())),
        state.safe_mode(),
    )?;
    let instance = get_instance(&state, &uuid)?;
    instance.kill_macro(pid).await?;
    Ok(Json(()))
}

pub async fn get_macro_configs(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<GetConfigResponse>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    let safe_mode = state.safe_mode();
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())), safe_mode)?;

    let instance = get_instance(&state, &uuid)?;

    let config = instance.get_macro_config(&macro_name).await?;

    match instance
        .validate_local_config(&macro_name, Some(&config))
        .await
    {
        Ok(local_value) => {
            // applied to a copy, so a failure returns the macro's defaults untouched
            let mut with_local_values = config.clone();
            for (setting_id, local_cache) in &local_value {
                let applied = match with_local_values.get_mut(setting_id) {
                    Some(setting) => setting.set_optional_value(local_cache.get_value().clone()),
                    None => Err(Error {
                        kind: ErrorKind::NotFound,
                        source: eyre!("Setting {setting_id} is not declared by the macro"),
                    }),
                };
                if let Err(e) = applied {
                    return Ok(Json(GetConfigResponse {
                        config,
                        message: Some(format!(
                            "There is a mismatch between a config type and its locally-stored value: {e}"
                        )),
                        error: Some(ErrorKind::Internal),
                    }));
                }
            }
            Ok(Json(GetConfigResponse {
                config: with_local_values,
                message: None,
                error: None,
            }))
        }
        Err(e) => match e.kind {
            ErrorKind::NotFound => Ok(Json(GetConfigResponse {
                config,
                message: Some("Local config cache not found".to_string()),
                error: Some(ErrorKind::NotFound),
            })),
            _ => Ok(Json(GetConfigResponse {
                config,
                message: Some(
                    "There is a mismatch between a config type and its locally-stored value"
                        .to_string(),
                ),
                error: Some(ErrorKind::Internal),
            })),
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
    let safe_mode = state.safe_mode();

    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())), safe_mode)?;
    require_code_execution(&state)?;

    let instance = get_instance(&state, &uuid)?;

    instance
        .store_macro_config_to_local(&macro_name, &config_to_store)
        .await?;
    Ok(())
}

pub fn get_instance_macro_routes(state: AppState) -> Router {
    let router = Router::new()
        .route("/instance/{uuid}/macro/run/{macro_name}", put(run_macro))
        .route("/instance/{uuid}/macro/kill/{pid}", put(kill_macro))
        .route("/instance/{uuid}/macro/list", get(get_instance_macro_list))
        .route(
            "/instance/{uuid}/macro/config/get/{macro_name}",
            get(get_macro_configs),
        )
        .route(
            "/instance/{uuid}/macro/config/store/{macro_name}",
            post(store_config_to_local),
        )
        .route("/instance/{uuid}/task/list", get(get_instance_task_list))
        .route(
            "/instance/{uuid}/history/list",
            get(get_instance_history_list),
        );
    #[cfg(not(feature = "deno"))]
    let router =
        crate::capabilities::unavailable_routes(router, crate::capabilities::Capability::Macros);
    router.with_state(state)
}
