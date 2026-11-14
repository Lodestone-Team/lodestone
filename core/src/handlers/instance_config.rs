use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;

use super::util::{get_instance, require_code_execution};
use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    implementations::minecraft::server_properties,
    traits::t_configurable::{
        manifest::{ConfigurableManifest, ConfigurableValue},
        TConfigurable,
    },
    types::InstanceUuid,
    AppState,
};

pub async fn get_instance_configurable_manifest(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<ConfigurableManifest>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    Ok(Json(instance.configurable_manifest().await))
}

pub async fn get_instance_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<ConfigurableManifest>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    if uuid.to_string().starts_with("DOCKER-") {
        return Ok(Json(ConfigurableManifest::default()));
    }
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    })?;
    Ok(Json(instance.configurable_manifest().await))
}

/// Settings that decide which program the instance runs, or with which arguments. Changing
/// them is as powerful as running any command as the Lodestone user, so it needs the
/// owner-level `WriteGlobalFile` permission (which is also blocked in safe mode), and is
/// turned off with the rest of code execution.
fn setting_controls_command_line(setting_id: &str) -> bool {
    matches!(setting_id, "java_cmd" | "cmd_args")
}

pub async fn set_instance_setting(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, section_id, setting_id)): Path<(InstanceUuid, String, String)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<ConfigurableValue>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    let safe_mode = state.safe_mode();
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), safe_mode)?;
    if setting_controls_command_line(&setting_id) {
        requester.try_action(&UserAction::WriteGlobalFile, safe_mode)?;
        require_code_execution(&state)?;
    }
    let instance = get_instance(&state, &uuid)?;

    // `server.properties` is content; the command line, JVM and Java settings are this
    // core's own (`.lodestone_minecraft_config.json`) and never in an image
    let op = if section_id == server_properties::SECTION_ID {
        Some(
            state
                .images
                .begin(&uuid, format!("Changed {setting_id}"))
                .await,
        )
    } else {
        None
    };
    instance
        .update_configurable(&section_id, &setting_id, value)
        .await?;
    if let Some(op) = op {
        op.succeed();
    }

    Ok(Json(()))
}

pub async fn set_instance_name(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(new_name): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = get_instance(&state, &uuid)?;
    instance.set_name(new_name).await?;
    Ok(Json(()))
}

pub async fn set_instance_description(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(new_description): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = get_instance(&state, &uuid)?;
    instance.set_description(new_description).await?;
    Ok(Json(()))
}

pub async fn set_instance_auto_start(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(auto_start): Json<bool>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = get_instance(&state, &uuid)?;
    instance.set_auto_start(auto_start).await?;
    Ok(Json(()))
}

pub async fn set_instance_restart_on_crash(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(restart_on_crash): Json<bool>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = get_instance(&state, &uuid)?;
    instance.set_restart_on_crash(restart_on_crash).await?;
    Ok(Json(()))
}

pub async fn change_version(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, new_version)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()), state.safe_mode())?;
    let instance = get_instance(&state, &uuid)?;
    instance.change_version(new_version.clone()).await?;
    // Recorded at once rather than merged: the image's game version is the new one. The
    // change itself succeeded, so a failed record is only logged (the check before the
    // next start records it then).
    if let Some(ctx) = crate::images::context_of(&state, &uuid).await {
        if let Err(e) = state
            .images
            .record_now(
                ctx,
                format!("Changed Minecraft version to {new_version}"),
            )
            .await
        {
            tracing::warn!(
                "Failed to record the content of {uuid} after its version changed: {}",
                e.source
            );
        }
    }
    Ok(Json(()))
}

pub fn get_instance_config_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/configurable_manifest",
            get(get_instance_configurable_manifest),
        )
        .route(
            "/instance/{uuid}/version/{new_version}",
            put(change_version),
        )
        .route("/instance/{uuid}/settings", get(get_instance_settings))
        .route(
            "/instance/{uuid}/settings/{section_id}/{setting_id}",
            put(set_instance_setting),
        )
        .route("/instance/{uuid}/name", put(set_instance_name))
        .route(
            "/instance/{uuid}/description",
            put(set_instance_description),
        )
        .route("/instance/{uuid}/auto_start", put(set_instance_auto_start))
        .route(
            "/instance/{uuid}/restart_on_crash",
            put(set_instance_restart_on_crash),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::setting_controls_command_line;

    #[test]
    fn command_line_settings_need_owner_permission() {
        assert!(setting_controls_command_line("java_cmd"));
        assert!(setting_controls_command_line("cmd_args"));
        assert!(!setting_controls_command_line("min_ram"));
        assert!(!setting_controls_command_line("max_ram"));
        assert!(!setting_controls_command_line("motd"));
    }
}
