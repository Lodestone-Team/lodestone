use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{delete, get, post};
use axum::Router;
use axum::{extract::Path, Json};

use color_eyre::eyre::{eyre, Context};
use futures::future::BoxFuture;
use tracing::{error, warn};

use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{
    CausedBy, CreatedInstance, Event, ProgressionEndValue, ProgressionEventID,
    ProgressionStartValue,
};
use crate::images::share::outcome::CreationOutcome;

use crate::traits::t_configurable::GameType;

use crate::implementations::minecraft::{MinecraftInstance, SetupConfig};
use crate::prelude::{path_to_instances, GameInstance};
use crate::traits::t_configurable::manifest::SetupValue;
use crate::traits::{t_configurable::TConfigurable, InstanceInfo, TInstance};
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::{implementations::minecraft, AppState};

use super::instance_setup_configs::HandlerGameType;
use super::util::{all_instances, get_instance, require_code_execution, AuthedUser};

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
pub(crate) fn instance_setup_path(
    instances_dir: &FsPath,
    name: &str,
    uuid: &InstanceUuid,
) -> PathBuf {
    let uuid = uuid.no_prefix();
    let short_uuid = uuid.get(0..8).unwrap_or(&uuid);
    instances_dir.join(format!(
        "{}-{}",
        sanitize_instance_dir_name(name),
        short_uuid
    ))
}

/// Removes the folder of an instance whose creation failed, logging any error.
pub(crate) async fn remove_failed_instance_dir(setup_path: PathBuf) {
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
/// as installing a modpack. It gets the instance folder, the creation's progression event
/// and the new instance's uuid, and returns what the end of the creation says.
pub(super) type PostSetupStep = Box<
    dyn FnOnce(
            PathBuf,
            ProgressionEventID,
            InstanceUuid,
        ) -> BoxFuture<'static, Result<SetupDone, Error>>
        + Send,
>;

/// What a [`PostSetupStep`] tells the end of the creation.
#[derive(Debug, Default)]
pub(super) struct SetupDone {
    /// An extra English sentence after "Server created successfully.".
    pub message: Option<String>,
    /// For a server made from a file, what it didn't get, as data for clients that
    /// translate it (the end value's `outcome`).
    pub outcome: Option<CreationOutcome>,
}

impl SetupDone {
    /// Only an extra message (or none), as every creation but one from a file ends.
    pub fn message(message: Option<String>) -> SetupDone {
        SetupDone {
            message,
            outcome: None,
        }
    }
}

/// The progress of recording a new server's first content image, on top of the creation's
/// `total_progress`.
const RECORD_PROGRESS: f64 = 1.0;

/// The file in a server folder whose creation hasn't finished, holding the new server's
/// uuid. Its `.lodestone_config` is written only once the creation's last step is done,
/// so the core never loads a half-made server; at startup a folder with this file and no
/// `.lodestone_config` is deleted ([`remove_unfinished_creations`]).
pub(crate) const CREATING_MARKER: &str = ".lodestone_creating";

/// Creates the folder of a new Minecraft instance and sets it up in the background, with
/// progression events (`total_progress` for setting it up; recording its first content
/// image adds [`RECORD_PROGRESS`]), like `POST /instance/create/{game_type}`. On failure the folder is
/// removed. Returns the new instance's UUID right away.
///
/// The folder holds [`CREATING_MARKER`] until the setup and `post_setup` are done, and
/// gets its `.lodestone_config` only then: a core stopped in the middle (while a file's
/// world is unpacked, say) leaves a folder that startup deletes, not a server with half
/// its files.
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
        .context("Failed to create server directory")?;

    let dot_lodestone_config =
        DotLodestoneConfig::new(instance_uuid.clone(), GameType::MinecraftJava);
    let dot_lodestone_json = serde_json::to_string_pretty(&dot_lodestone_config)
        .context("Failed to serialize .lodestone_config")?;
    if let Err(e) = crate::persist::write_atomic(
        &setup_path.join(CREATING_MARKER),
        instance_uuid.to_string(),
        None,
    )
    .await
    {
        // without its marker startup would never remove the folder
        remove_failed_instance_dir(setup_path).await;
        return Err(e);
    }

    tokio::task::spawn({
        let uuid = instance_uuid.clone();
        let event_broadcaster = state.event_broadcaster.clone();
        async move {
            let (progression_start_event, event_id) = Event::new_progression_event_start(
                title,
                Some(total_progress + RECORD_PROGRESS),
                Some(ProgressionStartValue::InstanceCreation {
                    instance_uuid: uuid.clone(),
                }),
                caused_by,
            );
            event_broadcaster.send(progression_start_event);
            let created = async {
                let instance = state
                    .server_setup
                    .install(
                        setup_config.clone(),
                        dot_lodestone_config.clone(),
                        setup_path.clone(),
                        &event_id,
                        state.event_broadcaster.clone(),
                        state.code_execution.clone(),
                    )
                    .await?;
                let (instance, done) = match post_setup {
                    None => (instance, SetupDone::default()),
                    Some(post_setup) => {
                        let done =
                            post_setup(setup_path.clone(), event_id.clone(), uuid.clone()).await?;
                        // read the instance again: the step may have changed its files
                        drop(instance);
                        let instance = minecraft::MinecraftInstance::restore(
                            setup_path.clone(),
                            dot_lodestone_config,
                            state.event_broadcaster.clone(),
                            state.code_execution.clone(),
                        )
                        .await?;
                        (instance, done)
                    }
                };
                // finished: from now on the core loads it
                crate::persist::write_atomic(
                    &setup_path.join(".lodestone_config"),
                    dot_lodestone_json,
                    None,
                )
                .await?;
                if let Err(e) = tokio::fs::remove_file(setup_path.join(CREATING_MARKER)).await {
                    warn!("Failed to remove the new server's creation marker: {e}");
                }
                Ok::<_, Error>((instance, done))
            }
            .await;
            let (minecraft_instance, done) = match created {
                Ok(created) => created,
                Err(e) => {
                    event_broadcaster.send(Event::new_progression_event_end(
                        event_id,
                        false,
                        Some(&format!("Server creation failed: {e}")),
                        None,
                    ));
                    // the images first (what a modpack's setup or a file's import recorded
                    // before the failure), then the folder, moved aside with its marker in
                    // one step: startup's order, so a core stopped meanwhile, or a delete
                    // that fails, leaves a folder the next start still recognises
                    if discard_unfinished(setup_path, Some(&uuid), &state.images).await {
                        state.images.request_gc();
                        let instances = path_to_instances().clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            if let Some(aside) = unfinished_dir(&instances) {
                                crate::images::share::clear_leftovers(&aside);
                            }
                        })
                        .await;
                    }
                    // and the objects a file's import unpacked or downloaded, which no
                    // image names but the store's grace keeps for an hour
                    state
                        .images
                        .request_gc_after(crate::images::gc::AFTER_GRACE);
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
            // before the server is listed, so nothing starts or changes it first
            record_created(&state, &uuid, &minecraft_instance, &event_id).await;
            let info = minecraft_instance.get_instance_info().await;
            state
                .instances
                .insert(uuid.clone(), minecraft_instance.into());
            let message = match done.message {
                Some(extra) => format!("Server created successfully. {extra}"),
                None => "Server created successfully".to_string(),
            };
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                true,
                Some(&message),
                Some(ProgressionEndValue::InstanceCreation(CreatedInstance {
                    info,
                    outcome: done.outcome.map(Box::new),
                })),
            ));
        }
    });
    Ok(instance_uuid)
}

/// Deletes the server folders in `instances_dir` whose creation a stopped core didn't
/// finish ([`CREATING_MARKER`] and no `.lodestone_config`), and their images (a file's
/// import may have recorded some), before the servers are read. A folder with both is a
/// finished server whose marker wasn't removed yet: it only loses the marker.
///
/// A folder's images are deleted first, while its marker still names them: a core stopped
/// in between finds the folder, and its marker, at the next start. A half-made folder can
/// hold a half-unpacked world of many gigabytes, so it is then only moved aside, into
/// `<instances>.unfinished` beside the servers' folder, which is emptied in the background
/// (`images::share::clear_leftovers`, read-only files too).
///
/// It always asks for a collection once the store's grace is past: the objects a write cut
/// off in the previous run stored (a creation from a file that failed, its timer lost with
/// the run) are named by nothing and go then.
pub(crate) async fn remove_unfinished_creations(
    instances_dir: &FsPath,
    images: &Arc<crate::images::ImageService>,
) {
    let dir = instances_dir.to_path_buf();
    let unfinished = tokio::task::spawn_blocking(move || {
        let mut unfinished = Vec::new();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return unfinished;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let marker = path.join(CREATING_MARKER);
            let Ok(uuid) = std::fs::read_to_string(&marker) else {
                continue;
            };
            if path.join(".lodestone_config").exists() {
                let _ = std::fs::remove_file(&marker);
                continue;
            }
            unfinished.push((path, uuid.trim().to_string()));
        }
        unfinished
    })
    .await
    .unwrap_or_default();
    for (path, uuid) in unfinished {
        // only a uuid the core makes names an images folder
        let uuid = uuid
            .strip_prefix("INSTANCE_")
            .is_some_and(|rest| uuid::Uuid::parse_str(rest).is_ok())
            .then(|| InstanceUuid::from(uuid));
        discard_unfinished(path, uuid.as_ref(), images).await;
    }
    let aside = unfinished_dir(instances_dir);
    if let Some(aside) = aside {
        let _ = tokio::task::spawn_blocking(move || crate::images::share::clear_leftovers(&aside))
            .await;
    }
    images.request_gc_after(crate::images::gc::AFTER_GRACE);
}

/// Discards the folder `path` of a server whose creation didn't finish, and the images
/// of `uuid`, in an order a stop or a failure at any point can't spoil: the images first,
/// while the folder's marker still names them (if they can't be deleted the folder and
/// its marker stay, for the next start to try again, and this answers false); then the
/// folder is renamed into `<instances>.unfinished` in one step, for the caller to empty
/// in the background (`images::share::clear_leftovers`). A deletion in place, which a
/// stop can cut off after the marker went, is only the fallback when the rename fails.
async fn discard_unfinished(
    path: PathBuf,
    uuid: Option<&InstanceUuid>,
    images: &Arc<crate::images::ImageService>,
) -> bool {
    if let Some(uuid) = uuid {
        if let Err(e) = images.delete_instance(uuid).await {
            error!(
                "Failed to delete the images of the unfinished server {uuid}: {}",
                e.source
            );
            return false;
        }
    }
    warn!(
        "Deleting {}, a server whose creation didn't finish",
        path.display()
    );
    let aside = path.parent().and_then(unfinished_dir);
    let _ = tokio::task::spawn_blocking(move || {
        let moved = aside.as_ref().is_some_and(|aside| {
            let to = aside.join(format!(
                "{}-{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                crate::util::rand_alphanumeric(8)
            ));
            std::fs::create_dir_all(aside).is_ok() && std::fs::rename(&path, to).is_ok()
        });
        if !moved {
            if let Err(e) = crate::images::remove_tree(&path) {
                error!("Failed to delete {}: {e}", path.display());
            }
        }
    })
    .await;
    true
}

/// Where startup moves the half-made server folders of `instances_dir` before deleting
/// them: `<instances>.unfinished`, beside it, so the servers' folder never lists them.
fn unfinished_dir(instances_dir: &FsPath) -> Option<PathBuf> {
    let name = instances_dir.file_name()?.to_str()?;
    Some(instances_dir.with_file_name(format!("{name}.unfinished")))
}

/// The first content image of a server just created, before it is listed: "Created
/// <name>", unless its setup already recorded one (a modpack's "Installed <pack>"). It
/// reads and stores every content file (all of an imported server's), so it first says so
/// on the creation's progression event `event_id` ([`RECORD_PROGRESS`]). A failure is only
/// logged: its first content image is then recorded when it stops, or at the next startup
/// (see `ImageService::server_stopped` and `startup_sweep`).
async fn record_created(
    state: &AppState,
    uuid: &InstanceUuid,
    instance: &minecraft::MinecraftInstance,
    event_id: &ProgressionEventID,
) {
    state
        .event_broadcaster
        .send(Event::new_progression_event_update(
            event_id,
            "Recording the server's content",
            RECORD_PROGRESS,
        ));
    if state.images.current_content(uuid).await.is_some() {
        return;
    }
    let ctx = crate::images::context_with_policy(state.images.root(), instance).await;
    let name = instance.name().await;
    if let Err(e) = state
        .images
        .record_now(ctx, format!("Created {name}"))
        .await
    {
        tracing::warn!(
            "Failed to record the content of the new server {uuid}: {}",
            e.source
        );
    }
}

pub async fn delete_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::DeleteInstance)
        .await?;
    let caused_by = requester.caused_by();
    let GameInstance::MinecraftInstance(minecraft) = get_instance(&state, &uuid)?;
    // no backup or restore may be using the folder, and the server can't start until it is
    // gone; both are held to the end
    let _job = state.backups.try_lock_job(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::Conflict,
        source: eyre!("A backup or restore of this server is running"),
    })?;
    let _stopped = minecraft.lock_stopped("deleting it").await?;
    let Some((_, instance)) = state.instances.remove(&uuid) else {
        return Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Server not found"),
        });
    };
    let (progression_event_start, event_id) = Event::new_progression_event_start(
        format!("Deleting server {}", instance.name().await),
        Some(10.0),
        Some(ProgressionStartValue::InstanceDelete {
            instance_uuid: uuid.clone(),
        }),
        caused_by,
    );
    let event_broadcaster = state.event_broadcaster.clone();
    event_broadcaster.send(progression_event_start);
    if let Err(e) = tokio::fs::remove_file(instance.path().await.join(".lodestone_config")).await {
        event_broadcaster.send(Event::new_progression_event_end(
            event_id,
            false,
            Some("Failed to delete .lodestone_config. Server not deleted"),
            None,
        ));
        state.instances.insert(uuid.clone(), instance);
        return Err::<Json<()>, std::io::Error>(e)
            .context("Failed to delete .lodestone_config file. Server not deleted")
            .map_err(Into::into);
    }

    // the link stops working with the server, and friends lose it
    state.connect.stop(&uuid, true).await;
    state.connect.friends.refresh(&state).await;
    state
        .port_manager
        .lock()
        .await
        .deallocate(instance.port().await);
    let instance_path = instance.path().await;
    state.public_listing.forget(&uuid).await;
    let res = crate::util::fs::remove_dir_all(instance_path).await;
    // the server's backups go with it (the Host's delete confirmation says so)
    if let Err(e) = state.backups.delete_server(&uuid).await {
        error!("Failed to delete the backups of {uuid}: {}", e.source);
    }
    match &res {
        Ok(_) => event_broadcaster.send(Event::new_progression_event_end(
            event_id,
            true,
            Some("Server deleted successfully"),
            Some(ProgressionEndValue::InstanceDelete {
                instance_uuid: uuid.clone(),
            }),
        )),
        Err(e) => {
            event_broadcaster.send(Event::new_progression_event_end(
                event_id,
                false,
                Some(&format!(
                    "Failed to delete some or all of the server's files: {e}"
                )),
                None,
            ));
        }
    }
    res.map(|_| Json(()))
}

pub fn get_instance_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/list", get(get_instance_list))
        .route(
            "/instance/create/{game_type}",
            post(create_minecraft_instance),
        )
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

#[cfg(test)]
pub(crate) mod content_images_tests;
