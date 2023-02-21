use axum::routing::{delete, get, post};
use axum::Router;
use axum::{extract::Path, Json};
use axum_auth::AuthBearer;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::info;

use ts_rs::TS;

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{
    CausedBy, Event, EventInner, ProgressionEndValue, ProgressionEvent, ProgressionEventInner,
    ProgressionStartValue,
};

use crate::implementations::minecraft::{Flavour, SetupConfig};
use crate::prelude::PATH_TO_INSTANCES;
use crate::traits::{t_configurable::TConfigurable, t_server::TServer, InstanceInfo, TInstance};

use crate::types::{InstanceUuid, Snowflake};
use crate::{implementations::minecraft, traits::t_server::State, AppState};

pub async fn get_instance_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<InstanceInfo>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    let mut list_of_configs: Vec<InstanceInfo> = Vec::new();

    let instances = state.instances.lock().await;
    for instance in instances.values() {
        if requester.can_perform_action(&UserAction::ViewInstance(instance.uuid().await)) {
            list_of_configs.push(instance.get_instance_info().await);
        }
    }

    list_of_configs.sort_by(|a, b| a.creation_time.cmp(&b.creation_time));

    Ok(Json(list_of_configs))
}

pub async fn get_instance_info(
    Path(uuid): Path<InstanceUuid>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<InstanceInfo>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    let instances = state.instances.lock().await;

    let instance = instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    requester.try_action(&UserAction::ViewInstance(uuid.clone()))?;
    Ok(Json(instance.get_instance_info().await))
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct MinecraftSetupConfigPrimitive {
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub port: u32,
    pub cmd_args: Option<Vec<String>>,
    pub description: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: Option<bool>,
    pub backup_period: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct GenericSetupConfigPrimitive {
    pub name: String,
    pub description: Option<String>,
    pub port: u32,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: Option<bool>,
}

impl From<MinecraftSetupConfigPrimitive> for SetupConfig {
    fn from(config: MinecraftSetupConfigPrimitive) -> Self {
        let uuid = InstanceUuid::default();
        SetupConfig {
            name: config.name.clone(),
            version: config.version,
            flavour: config.flavour,
            port: config.port,
            cmd_args: config.cmd_args,
            description: config.description,
            min_ram: config.min_ram,
            max_ram: config.max_ram,
            auto_start: config.auto_start,
            restart_on_crash: config.restart_on_crash,
            timeout_last_left: config.timeout_last_left,
            timeout_no_activity: config.timeout_no_activity,
            start_on_connection: config.start_on_connection,
            backup_period: config.backup_period,
        }
    }
}
pub async fn create_minecraft_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(mut primitive_setup_config): Json<MinecraftSetupConfigPrimitive>,
) -> Result<Json<InstanceUuid>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::CreateInstance)?;
    primitive_setup_config.name = sanitize_filename::sanitize(&primitive_setup_config.name);
    let mut setup_config: SetupConfig = primitive_setup_config.into();
    let name = setup_config.name.clone();
    if name.is_empty() {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name must not be empty"),
        });
    }
    if name.len() > 100 {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name must not be longer than 100 characters"),
        });
    }
    // for (_, instance) in state.instances.lock().await.iter() {
    //     let path = instance.path().await;
    //     if path == setup_config.path {
    //         while path == setup_config.path {
    //             info!("You just hit the lottery");
    //             setup_config.uuid = InstanceUuid::default();
    //             let name_with_uuid = format!("{}-{}", name, &setup_config.uuid.no_prefix()[0..5]);
    //             setup_config.path = PATH_TO_INSTANCES.with(|path| {
    //                 path.join(format!(
    //                     "{}-{}",
    //                     name_with_uuid,
    //                     &setup_config.uuid.no_prefix()[0..5]
    //                 ))
    //             });
    //         }
    //     }
    // }

    // let uuid = setup_config.uuid.clone();
    // tokio::task::spawn({
    //     let uuid = uuid.clone();
    //     let instance_name = setup_config.name.clone();
    //     let event_broadcaster = state.event_broadcaster.clone();
    //     let port = setup_config.port;
    //     let flavour = setup_config.flavour.clone();
    //     let caused_by = CausedBy::User {
    //         user_id: requester.uid.clone(),
    //         user_name: requester.username.clone(),
    //     };
    //     async move {
    //         let progression_event_id = Snowflake::default();
    //         let _ = event_broadcaster.send(Event {
    //             event_inner: EventInner::ProgressionEvent(ProgressionEvent {
    //                 event_id: progression_event_id,
    //                 progression_event_inner: ProgressionEventInner::ProgressionStart {
    //                     progression_name: format!("Setting up Minecraft server {}", name),
    //                     producer_id: Some(uuid.clone()),
    //                     total: Some(10.0),
    //                     inner: Some(ProgressionStartValue::InstanceCreation {
    //                         instance_uuid: uuid.clone(),
    //                         instance_name: instance_name.clone(),
    //                         port,
    //                         flavour: flavour.to_string(),
    //                         game_type: "minecraft".to_string(),
    //                     }),
    //                 },
    //             }),
    //             details: "".to_string(),
    //             snowflake: Snowflake::default(),
    //             caused_by: caused_by.clone(),
    //         });
    //         let minecraft_instance = match minecraft::MinecraftInstance::new(
    //             setup_config.clone(),
    //             progression_event_id,
    //             state.event_broadcaster.clone(),
    //             state.macro_executor.clone(),
    //         )
    //         .await
    //         {
    //             Ok(v) => {
    //                 let _ = event_broadcaster.send(Event {
    //                     event_inner: EventInner::ProgressionEvent(ProgressionEvent {
    //                         event_id: progression_event_id,
    //                         progression_event_inner: ProgressionEventInner::ProgressionEnd {
    //                             success: true,
    //                             message: Some("Instance creation success".to_string()),
    //                             inner: Some(ProgressionEndValue::InstanceCreation(
    //                                 v.get_instance_info().await,
    //                             )),
    //                         },
    //                     }),
    //                     details: "".to_string(),
    //                     snowflake: Snowflake::default(),
    //                     caused_by: caused_by.clone(),
    //                 });
    //                 v
    //             }
    //             Err(e) => {
    //                 let _ = event_broadcaster.send(Event {
    //                     event_inner: EventInner::ProgressionEvent(ProgressionEvent {
    //                         event_id: progression_event_id,
    //                         progression_event_inner: ProgressionEventInner::ProgressionEnd {
    //                             success: false,
    //                             message: Some(format!("Instance creation failed: {:?}", e)),
    //                             inner: None,
    //                         },
    //                     }),
    //                     details: "".to_string(),
    //                     snowflake: Snowflake::default(),
    //                     caused_by: caused_by.clone(),
    //                 });
    //                 crate::util::fs::remove_dir_all(setup_config.path)
    //                     .await
    //                     .context("Failed to remove directory after instance creation failed")
    //                     .unwrap();
    //                 return;
    //             }
    //         };
    //         let mut port_manager = state.port_manager.lock().await;
    //         port_manager.add_port(setup_config.port);
    //         state
    //             .instances
    //             .lock()
    //             .await
    //             .insert(uuid.clone(), minecraft_instance.into());
    //     }
    // });
    // Ok(Json(uuid))
    todo!()
}

pub async fn delete_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::DeleteInstance)?;
    let mut instances = state.instances.lock().await;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    if let Some(instance) = instances.get(&uuid) {
        if !(instance.state().await == State::Stopped) {
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Instance must be stopped before deletion"),
            })
        } else {
            let progression_id = Snowflake::default();
            let event_broadcaster = state.event_broadcaster.clone();
            let _ = event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: progression_id,
                    progression_event_inner: ProgressionEventInner::ProgressionStart {
                        progression_name: format!("Deleting instance {}", instance.name().await),
                        producer_id: Some(uuid.clone()),
                        total: Some(10.0),
                        inner: None,
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: caused_by.clone(),
            });
            tokio::fs::remove_file(instance.path().await.join(".lodestone_config"))
                .await
                .map_err(|e| {
                    let _ = event_broadcaster.send(Event {
                        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                            event_id: Snowflake::default(),
                            progression_event_inner: ProgressionEventInner::ProgressionEnd {
                                success: false,
                                message: Some(
                                    "Failed to delete .lodestone_config. Instance not deleted"
                                        .to_string(),
                                ),
                                inner: None,
                            },
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by: caused_by.clone(),
                    });
                    Err::<(), std::io::Error>(e)
                        .context("Failed to delete .lodestone_config file. Instance not deleted")
                        .unwrap_err()
                })?;
            state
                .port_manager
                .lock()
                .await
                .deallocate(instance.port().await);
            let instance_path = instance.path().await;
            instances.remove(&uuid);
            drop(instances);
            let res = crate::util::fs::remove_dir_all(instance_path).await;

            if res.is_ok() {
                let _ = event_broadcaster.send(Event {
                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                        event_id: progression_id,
                        progression_event_inner: ProgressionEventInner::ProgressionEnd {
                            success: true,
                            message: Some("Deleted instance".to_string()),
                            inner: Some(ProgressionEndValue::InstanceDelete {
                                instance_uuid: uuid.clone(),
                            }),
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: caused_by.clone(),
                });
            } else {
                let _ = event_broadcaster.send(Event {
                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                        event_id: progression_id,
                        progression_event_inner: ProgressionEventInner::ProgressionEnd {
                            success: false,
                            message: Some(
                                "Could not delete some or all of instance's files".to_string(),
                            ),
                            inner: None,
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: caused_by.clone(),
                });
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
        .route("/instance/minecraft", post(create_minecraft_instance))
        .route("/instance/:uuid", delete(delete_instance))
        .route("/instance/:uuid/info", get(get_instance_info))
        .with_state(state)
}
