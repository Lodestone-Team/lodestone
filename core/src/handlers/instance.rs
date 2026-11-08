use std::path::{Path as FsPath, PathBuf};

use axum::extract::State;
use axum::routing::{delete, get, post};
use axum::Router;
use axum::{extract::Path, Json};

use color_eyre::eyre::{eyre, Context};
use futures::future::BoxFuture;
use serde::Deserialize;
use tracing::{error, info};

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{
    CausedBy, Event, ProgressionEndValue, ProgressionEventID, ProgressionStartValue,
};

use crate::implementations::generic;
use crate::traits::t_configurable::GameType;

use crate::implementations::minecraft::{MinecraftInstance, SetupConfig};
use crate::prelude::{path_to_instances, GameInstance};
use crate::traits::t_configurable::manifest::SetupValue;
use crate::traits::{t_configurable::TConfigurable, t_server::TServer, InstanceInfo, TInstance};
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::{implementations::minecraft, traits::t_server::State as InstanceState, AppState};

use super::instance_setup_configs::HandlerGameType;
use super::util::{
    all_instances, check_create_atom, get_instance, require_code_execution, require_owner_if_atom,
    AuthedUser,
};

/// The longest instance name, in characters, that is used in the folder name.
const MAX_DIR_NAME_CHARS: usize = 64;

/// Turns an instance name into a safe folder name.
///
/// Only letters, digits, spaces, `-` and `_` are kept (anything else becomes `_`), so the
/// result can never contain a path separator, `..`, a drive letter or a leading dot. It is
/// also trimmed and shortened, and never empty.
pub fn sanitize_instance_dir_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(MAX_DIR_NAME_CHARS)
        .collect();
    let sanitized = sanitized.trim();
    if sanitized.is_empty() {
        "instance".to_string()
    } else {
        sanitized.to_string()
    }
}

/// The folder for a new instance: `<instances_dir>/<sanitized name>-<first 8 uuid chars>`.
fn instance_setup_path(instances_dir: &FsPath, name: &str, uuid: &InstanceUuid) -> PathBuf {
    let uuid = uuid.no_prefix();
    let short_uuid = uuid.get(0..8).unwrap_or(&uuid);
    instances_dir.join(format!(
        "{}-{}",
        sanitize_instance_dir_name(name),
        short_uuid
    ))
}

/// Removes the folder of an instance whose creation failed, logging any error.
async fn remove_failed_instance_dir(setup_path: PathBuf) {
    if let Err(e) = crate::util::fs::remove_dir_all(&setup_path).await {
        error!(
            "Failed to remove directory {} after instance creation failed: {}",
            setup_path.display(),
            e
        );
    }
}

pub async fn get_instance_list(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<Vec<InstanceInfo>>, Error> {
    let mut list_of_configs: Vec<InstanceInfo> = Vec::new();

    for instance in all_instances(&state) {
        if requester.can_perform_action(&UserAction::ViewInstance(instance.uuid().await)) {
            list_of_configs.push(instance.get_instance_info().await);
        }
    }
    let containers = state
        .docker_bridge
        .list_containers()
        .await
        .unwrap_or_default();
    list_of_configs.extend(
        containers.into_iter().filter(|info| {
            requester.can_perform_action(&UserAction::ViewInstance(info.uuid.clone()))
        }),
    );

    list_of_configs.sort_by_key(|a| a.creation_time);

    Ok(Json(list_of_configs))
}

pub async fn get_instance_info(
    Path(uuid): Path<InstanceUuid>,
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<InstanceInfo>, Error> {
    // check the permission first, so the reply does not show whether the instance exists
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    Ok(Json(instance.get_instance_info().await))
}

pub async fn create_minecraft_instance(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(game_type): Path<HandlerGameType>,
    Json(manifest_value): Json<SetupValue>,
) -> Result<Json<InstanceUuid>, Error> {
    requester
        .require(&state, UserAction::CreateInstance)
        .await?;

    let flavour = game_type.try_into()?;

    let setup_config = MinecraftInstance::construct_setup_config(manifest_value, flavour).await?;
    // Extra command line arguments can make Java run any code, so they need the same
    // permission as changing them later (see `instance_config::set_instance_setting`).
    // Creation does not accept a custom `java_cmd`.
    // They are also turned off with the rest of code execution.
    if !setup_config.cmd_args.is_empty() {
        requester
            .require(&state, UserAction::WriteGlobalFile)
            .await?;
        require_code_execution(&state)?;
    }

    let title = format!("Setting up Minecraft server {}", setup_config.name);
    let uuid =
        spawn_minecraft_instance_creation(state, &requester, setup_config, title, 10.0, None)
            .await?;
    Ok(Json(uuid))
}

/// A step that runs after a new Minecraft instance is set up and before it is added, such
/// as installing a modpack. It gets the instance folder and the creation's progression
/// event, and returns an extra message for the end of the creation.
pub(super) type PostSetupStep = Box<
    dyn FnOnce(PathBuf, ProgressionEventID) -> BoxFuture<'static, Result<Option<String>, Error>>
        + Send,
>;

/// Creates the folder of a new Minecraft instance and sets it up in the background, with
/// progression events, like `POST /instance/create/{game_type}`. On failure the folder is
/// removed. Returns the new instance's UUID right away.
pub(super) async fn spawn_minecraft_instance_creation(
    state: AppState,
    requester: &AuthedUser,
    setup_config: SetupConfig,
    title: String,
    total_progress: f64,
    post_setup: Option<PostSetupStep>,
) -> Result<InstanceUuid, Error> {
    let caused_by = requester.caused_by();
    let requester_uid = requester.uid.clone();
    let instance_uuid = InstanceUuid::default();

    let setup_path = instance_setup_path(path_to_instances(), &setup_config.name, &instance_uuid);

    tokio::fs::create_dir_all(&setup_path)
        .await
        .context("Failed to create instance directory")?;

    let dot_lodestone_config =
        DotLodestoneConfig::new(instance_uuid.clone(), GameType::MinecraftJava);

    // write dot lodestone config

    tokio::fs::write(
        setup_path.join(".lodestone_config"),
        serde_json::to_string_pretty(&dot_lodestone_config)
            .context("Failed to serialize .lodestone_config")?,
    )
    .await
    .context("Failed to write .lodestone_config file")?;

    tokio::task::spawn({
        let uuid = instance_uuid.clone();
        let event_broadcaster = state.event_broadcaster.clone();
        async move {
            let (progression_start_event, event_id) = Event::new_progression_event_start(
                title,
                Some(total_progress),
                Some(ProgressionStartValue::InstanceCreation {
                    instance_uuid: uuid.clone(),
                }),
                caused_by,
            );
            event_broadcaster.send(progression_start_event);
            let created = async {
                let instance = minecraft::MinecraftInstance::new(
                    setup_config.clone(),
                    dot_lodestone_config.clone(),
                    setup_path.clone(),
                    &event_id,
                    state.event_broadcaster.clone(),
                    state.macro_executor.clone(),
                )
                .await?;
                let Some(post_setup) = post_setup else {
                    return Ok((instance, None));
                };
                let message = post_setup(setup_path.clone(), event_id.clone()).await?;
                // read the instance again: the step may have changed its files
                drop(instance);
                let instance = minecraft::MinecraftInstance::restore(
                    setup_path.clone(),
                    dot_lodestone_config,
                    state.event_broadcaster.clone(),
                    state.macro_executor.clone(),
                )
                .await?;
                Ok::<_, Error>((instance, message))
            }
            .await;
            let minecraft_instance = match created {
                Ok((v, message)) => {
                    let message = match message {
                        Some(extra) => format!("Instance created successfully. {extra}"),
                        None => "Instance created successfully".to_string(),
                    };
                    event_broadcaster.send(Event::new_progression_event_end(
                        event_id,
                        true,
                        Some(&message),
                        Some(ProgressionEndValue::InstanceCreation(
                            v.get_instance_info().await,
                        )),
                    ));
                    v
                }
                Err(e) => {
                    event_broadcaster.send(Event::new_progression_event_end(
                        event_id,
                        false,
                        Some(&format!("Instance creation failed: {e}")),
                        None,
                    ));
                    remove_failed_instance_dir(setup_path).await;
                    return;
                }
            };
            state.port_manager.lock().await.add_port(setup_config.port);
            // Add the new instance to the creator's current permissions. Re-reading them under
            // the write lock keeps changes made while the instance was being set up.
            if let Err(e) = state
                .users_manager
                .write()
                .await
                .modify_permissions(
                    &requester_uid,
                    |perm| {
                        perm.can_start_instance.insert(uuid.clone());
                        perm.can_stop_instance.insert(uuid.clone());
                        perm.can_view_instance.insert(uuid.clone());
                        perm.can_read_instance_file.insert(uuid.clone());
                        perm.can_write_instance_file.insert(uuid.clone());
                    },
                    CausedBy::System,
                )
                .await
            {
                // the instance is still usable by the owner and admins
                error!("Failed to update permissions: {:?}", e);
            }
            state
                .instances
                .insert(uuid.clone(), minecraft_instance.into());
        }
    });
    Ok(instance_uuid)
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenericSetupConfig {
    url: String,
    setup_value: SetupValue,
}

pub async fn create_generic_instance(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(setup_config): Json<GenericSetupConfig>,
) -> Result<Json<()>, Error> {
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
        .context("Failed to create instance directory")?;

    let dot_lodestone_config = DotLodestoneConfig::new(instance_uuid.clone(), GameType::Generic);
    let event_broadcaster = state.event_broadcaster.clone();
    tokio::task::spawn(async move {
        let (progression_start_event, event_id) = Event::new_progression_event_start(
            format!("Setting up instance {}", setup_config.setup_value.name),
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
                    Some("Instance created successfully"),
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
                    Some(&format!("Instance creation failed: {e}")),
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

        state
            .instances
            .insert(instance_uuid.clone(), instance.into());
    });

    Ok(Json(()))
}

pub async fn delete_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::DeleteInstance)
        .await?;
    require_owner_if_atom(&state, &requester, &uuid)?;
    let caused_by = requester.caused_by();
    if let Some((_, instance)) = state.instances.remove(&uuid) {
        if !(instance.state().await == InstanceState::Stopped) {
            state.instances.insert(uuid.clone(), instance);
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Instance must be stopped before deletion"),
            })
        } else {
            let (progression_event_start, event_id) = Event::new_progression_event_start(
                format!("Deleting instance {}", instance.name().await),
                Some(10.0),
                Some(ProgressionStartValue::InstanceDelete {
                    instance_uuid: uuid.clone(),
                }),
                caused_by,
            );
            let event_broadcaster = state.event_broadcaster.clone();
            event_broadcaster.send(progression_event_start);
            if let Err(e) =
                tokio::fs::remove_file(instance.path().await.join(".lodestone_config")).await
            {
                event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    false,
                    Some("Failed to delete .lodestone_config. Instance not deleted"),
                    None,
                ));
                state.instances.insert(uuid.clone(), instance);
                return Err::<Json<()>, std::io::Error>(e)
                    .context("Failed to delete .lodestone_config file. Instance not deleted")
                    .map_err(Into::into);
            }

            state
                .port_manager
                .lock()
                .await
                .deallocate(instance.port().await);
            let instance_path = instance.path().await;
            // if instance is generic
            if let GameInstance::GenericInstance(i) = instance {
                i.destruct().await;
            };
            let res = crate::util::fs::remove_dir_all(instance_path).await;
            match &res {
                Ok(_) => event_broadcaster.send(Event::new_progression_event_end(
                    event_id,
                    true,
                    Some("Instance deleted successfully"),
                    Some(ProgressionEndValue::InstanceDelete {
                        instance_uuid: uuid.clone(),
                    }),
                )),
                Err(e) => {
                    event_broadcaster.send(Event::new_progression_event_end(
                        event_id,
                        false,
                        Some(&format!(
                            "Failed to delete some or all of the instance's files: {e}"
                        )),
                        None,
                    ));
                }
            }
            res.map(|_| Json(()))
        }
    } else {
        Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })
    }
}

pub fn get_instance_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/list", get(get_instance_list))
        .route(
            "/instance/create/{game_type}",
            post(create_minecraft_instance),
        )
        .route("/instance/create_generic", post(create_generic_instance))
        .route("/instance/{uuid}", delete(delete_instance))
        .route("/instance/{uuid}/info", get(get_instance_info))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_normal_names() {
        assert_eq!(sanitize_instance_dir_name("My Server"), "My Server");
        assert_eq!(
            sanitize_instance_dir_name("survival_2-test"),
            "survival_2-test"
        );
        assert_eq!(sanitize_instance_dir_name("服务器"), "服务器");
    }

    #[test]
    fn sanitize_removes_path_parts() {
        for name in [
            "../../x",
            "/tmp/x",
            "..\\..\\x",
            "C:\\Windows",
            "a/b",
            "..",
            ".hidden",
            "x\0y",
        ] {
            let sanitized = sanitize_instance_dir_name(name);
            assert!(!sanitized.contains('/'), "{name:?} -> {sanitized:?}");
            assert!(!sanitized.contains('\\'), "{name:?} -> {sanitized:?}");
            assert!(!sanitized.contains(':'), "{name:?} -> {sanitized:?}");
            assert!(!sanitized.contains('.'), "{name:?} -> {sanitized:?}");
            assert!(!sanitized.contains('\0'), "{name:?} -> {sanitized:?}");
        }
        assert_eq!(sanitize_instance_dir_name("../../x"), "______x");
    }

    #[test]
    fn sanitize_is_never_empty_and_is_bounded() {
        assert_eq!(sanitize_instance_dir_name(""), "instance");
        assert_eq!(sanitize_instance_dir_name("   "), "instance");
        assert_eq!(
            sanitize_instance_dir_name(&"a".repeat(1000))
                .chars()
                .count(),
            MAX_DIR_NAME_CHARS
        );
    }

    #[test]
    fn setup_path_stays_in_instances_folder() {
        let uuid = InstanceUuid::default();
        let instances_dir = FsPath::new("/lodestone/instances");
        for name in ["../../x", "/tmp/x", "..", "a/../../b", "C:\\x"] {
            let path = instance_setup_path(instances_dir, name, &uuid);
            assert_eq!(
                path.parent(),
                Some(instances_dir),
                "{name:?} -> {}",
                path.display()
            );
        }
    }
}
