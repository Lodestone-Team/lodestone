use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;

use super::util::{get_instance, require_code_execution, require_owner_for_atom};
use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
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
    matches!(setting_id, "java_cmd" | "cmd_args" | "custom_cmd")
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
    require_owner_for_atom(&requester, &instance)?;

    instance
        .update_configurable(&section_id, &setting_id, value)
        .await?;

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
    require_owner_for_atom(&requester, &instance)?;
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
    require_owner_for_atom(&requester, &instance)?;
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
    require_owner_for_atom(&requester, &instance)?;
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
    require_owner_for_atom(&requester, &instance)?;
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
    require_owner_for_atom(&requester, &instance)?;
    instance.change_version(new_version).await?;
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
    use reqwest::StatusCode;
    use serde_json::{json, Value};

    use super::setting_controls_command_line;
    use crate::{
        auth::{permission::UserPermission, user::User},
        events::CausedBy,
        global_settings::{GlobalSettingsData, CODE_EXECUTION_DISABLED_MESSAGE},
        test_util::{serve_api, test_app_state},
        types::InstanceUuid,
    };

    #[test]
    fn command_line_settings_need_owner_permission() {
        assert!(setting_controls_command_line("java_cmd"));
        assert!(setting_controls_command_line("cmd_args"));
        assert!(setting_controls_command_line("custom_cmd"));
        assert!(!setting_controls_command_line("min_ram"));
        assert!(!setting_controls_command_line("max_ram"));
        assert!(!setting_controls_command_line("motd"));
    }

    const INSTANCE: &str = "DOCKER-custom-cmd-test";

    struct Core {
        _dir: tempfile::TempDir,
        base: String,
        owner: String,
        editor: String,
    }

    /// A core with an owner, and a user who may change the settings of the (missing)
    /// instance, and may also write global files if `editor_writes_global_files`.
    async fn core(code_execution: bool, editor_writes_global_files: bool) -> Core {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let state = test_app_state(
            dir.path(),
            GlobalSettingsData {
                safe_mode: false,
                code_execution_enabled: code_execution,
                ..Default::default()
            },
        )
        .await;
        let mut permissions = UserPermission::new();
        permissions
            .can_access_instance_setting
            .insert(InstanceUuid::from(INSTANCE.to_string()));
        permissions.can_write_global_file = editor_writes_global_files;
        let mut jwts = Vec::new();
        for (name, is_owner, permissions) in [
            ("owner", true, UserPermission::new()),
            ("editor", false, permissions),
        ] {
            let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
            state
                .users_manager
                .write()
                .await
                .add_user(user.clone(), CausedBy::System)
                .await
                .unwrap();
            jwts.push(user.create_jwt().unwrap().to_string());
        }
        let base = serve_api(state).await;
        Core {
            _dir: dir,
            base,
            owner: jwts[0].clone(),
            editor: jwts[1].clone(),
        }
    }

    impl Core {
        async fn set(&self, token: &str, setting: &str, value: &str) -> (StatusCode, Value) {
            let response = reqwest::Client::new()
                .put(format!(
                    "{}/instance/{INSTANCE}/settings/cmd_args_section/{setting}",
                    self.base
                ))
                .bearer_auth(token)
                .json(&json!({ "type": "String", "value": value }))
                .send()
                .await
                .unwrap();
            let status = response.status();
            let text = response.text().await.unwrap();
            (
                status,
                serde_json::from_str(&text).unwrap_or(Value::String(text)),
            )
        }
    }

    #[tokio::test]
    async fn custom_cmd_edit_is_rejected_without_the_global_file_permission() {
        let core = core(true, false).await;
        for setting in ["custom_cmd", "java_cmd", "cmd_args"] {
            let (status, body) = core.set(&core.editor, setting, "./run.sh").await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{setting}: {body}");
        }
        // a setting that does not run anything gets past the permission checks, and only
        // fails because the instance does not exist
        let (status, _) = core.set(&core.editor, "max_ram", "1").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn custom_cmd_edit_is_rejected_while_code_execution_is_disabled() {
        let core = core(false, true).await;
        for token in [&core.owner, &core.editor] {
            for setting in ["custom_cmd", "java_cmd", "cmd_args"] {
                let (status, body) = core.set(token, setting, "./run.sh").await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{setting}: {body}");
                assert!(
                    body.to_string().contains(CODE_EXECUTION_DISABLED_MESSAGE),
                    "{body}"
                );
            }
        }
    }

    #[tokio::test]
    async fn custom_cmd_edit_is_allowed_for_those_with_the_permission() {
        let core = core(true, true).await;
        // past every check; only the instance is missing
        for token in [&core.owner, &core.editor] {
            let (status, _) = core.set(token, "custom_cmd", "./run.sh").await;
            assert_eq!(status, StatusCode::NOT_FOUND);
        }
    }
}
