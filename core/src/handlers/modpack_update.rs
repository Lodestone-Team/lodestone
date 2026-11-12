//! Updating a server to another version of its modpack:
//! `/instance/{uuid}/modpack/update/plan` shows what an update will do, and
//! `/instance/{uuid}/modpack/update` does it. See [`crate::modpack::update`].
//!
//! Both need `WriteInstanceFile`. A pack from Modrinth must be a version of the modpack
//! the server came from; an uploaded pack (`/modpack/upload`) can be any pack, and needs
//! `WriteGlobalFile` too, as for creating a server from one.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use super::modpack::{modrinth_pack, stored_pack, ModpackSource};
use super::util::{get_instance, AuthedUser};
use crate::auth::user::UserAction;
use crate::error::Error;
use crate::events::{CausedBy, Event};
use crate::implementations::minecraft::{Flavour, MinecraftInstance};
use crate::modpack::store::StoredPack;
use crate::modpack::update::{
    pack_files, place_files, plan_files, remove_work_dirs, stage_files, undo_placed_files,
    FileAction, FilePlan, ModpackUpdatePlan, ModpackUpdateResult, PackFile, PackRecord,
};
use crate::modpack::{
    describe_skipped, read_pack_async, server_flavour, ModpackInfo, ModpackOrigin, Pack,
};
use crate::mods::bad_request;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::types::InstanceUuid;
use crate::AppState;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ModpackUpdateRequest {
    pub source: ModpackSource,
}

fn minecraft_instance(instance: GameInstance) -> MinecraftInstance {
    match instance {
        GameInstance::MinecraftInstance(instance) => instance,
    }
}

async fn spawn_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| eyre!("The modpack task failed: {e}"))?
}

/// Whether `flavour` already runs the loader version a pack names. A Forge pack names
/// `47.2.0` for the build `1.20.1-47.2.0`.
fn runs_loader_version(flavour: &Flavour, game_version: &str, info: &ModpackInfo) -> bool {
    let (Some(current), Some(wanted)) = (flavour.loader_version(), info.loader_version.as_ref())
    else {
        return false;
    };
    match flavour {
        Flavour::Forge { .. } => [
            wanted.clone(),
            format!("{game_version}-{wanted}"),
            format!("{game_version}-{wanted}-{game_version}"),
        ]
        .contains(&current),
        _ => current == *wanted,
    }
}

/// Everything an update needs, worked out before anything changes.
struct Prepared {
    instance: MinecraftInstance,
    /// Keeps the pack file while the update runs.
    stored: StoredPack,
    pack: Pack,
    origin: ModpackOrigin,
    new_files: Arc<BTreeMap<String, PackFile>>,
    plan: FilePlan,
    summary: ModpackUpdatePlan,
    allow_scripts: bool,
}

/// The files the server's current pack wrote: its record, else the files of the current
/// pack version when it is on Modrinth. `None` with a note when neither is there.
async fn current_record(
    state: &AppState,
    root: &std::path::Path,
    origin: Option<&ModpackOrigin>,
    allow_scripts: bool,
) -> (PackRecord, Option<String>) {
    if let Some(record) = PackRecord::load(root).await {
        return (record, None);
    }
    let no_record = "This server has no record of the files its modpack wrote, so files the \
                     new version drops are not removed."
        .to_string();
    let Some(version_id) = origin
        .map(|o| o.version_id.clone())
        .filter(|v| !v.is_empty())
    else {
        return (PackRecord::default(), Some(no_record));
    };
    let old = async {
        let stored = modrinth_pack(state, &version_id).await?;
        let pack = read_pack_async(stored.path(), allow_scripts).await?;
        let files =
            spawn_blocking(move || pack_files(&pack, &stored.path(), allow_scripts)).await?;
        Ok::<_, Error>(crate::modpack::update::record_of(&files))
    }
    .await;
    match old {
        Ok(record) => (record, None),
        Err(e) => {
            warn!("Could not read the server's current modpack version {version_id}: {e}");
            (PackRecord::default(), Some(no_record))
        }
    }
}

async fn prepare(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
    source: &ModpackSource,
) -> Result<Prepared, Error> {
    let instance = minecraft_instance(get_instance(state, uuid)?);
    let current_origin = instance.modpack_origin().await;
    let stored = stored_pack(state, requester, source).await?;
    // scripts in a pack are only copied for the owner, while code execution is on
    let allow_scripts = requester.is_owner && state.code_execution_enabled();
    let pack = read_pack_async(stored.path(), allow_scripts).await?;
    let origin = match &stored.version_id {
        Some(version_id) => {
            ModpackOrigin::from_modrinth(state.mods.modrinth.as_ref(), version_id).await?
        }
        None => ModpackOrigin::from_upload(&pack.info),
    };
    if let Some(current) = current_origin.as_ref().filter(|c| !c.project_id.is_empty()) {
        if !origin.project_id.is_empty() && origin.project_id != current.project_id {
            return Err(bad_request(format!(
                "{} is not a version of {}, the modpack this server came from",
                origin.name, current.name
            )));
        }
    }

    let root = instance.path().await;
    let (old_record, note) =
        current_record(state, &root, current_origin.as_ref(), allow_scripts).await;
    let new_files = {
        let (pack, path) = (pack.clone(), stored.path());
        Arc::new(spawn_blocking(move || pack_files(&pack, &path, allow_scripts)).await?)
    };
    let plan = {
        let (root, new_files) = (root.clone(), new_files.clone());
        spawn_blocking(move || Ok(plan_files(&root, &old_record, &new_files))).await?
    };

    let (current_version, flavour) = instance.server_software().await;
    let current_loader = flavour.to_string();
    let mut target = pack.info.clone();
    target.upload_id = None;
    if target.unsupported_reason.is_none() && current_loader != target.loader {
        target.unsupported_reason = Some(format!(
            "This server runs {current_loader}, and this version of the modpack needs {}",
            target.loader
        ));
    }
    let reinstalls_server = current_version != target.game_version
        || !runs_loader_version(&flavour, &target.game_version, &target);
    let summary = ModpackUpdatePlan {
        current_game_version: current_version,
        current_loader,
        current_loader_version: flavour.loader_version(),
        reinstalls_server,
        added: plan.paths(FileAction::Add),
        replaced: plan.paths(FileAction::Replace),
        removed: plan.paths(FileAction::Remove),
        kept: plan.kept.clone(),
        skipped: pack
            .skipped
            .iter()
            .map(|s| format!("{} ({})", s.path, s.reason))
            .collect(),
        note,
        target,
    };
    Ok(Prepared {
        instance,
        stored,
        pack,
        origin,
        new_files,
        plan,
        summary,
        allow_scripts,
    })
}

pub(super) async fn plan_update(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ModpackUpdateRequest>,
) -> Result<Json<ModpackUpdatePlan>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let prepared = prepare(&state, &requester, &uuid, &request.source).await?;
    Ok(Json(prepared.summary))
}

/// Stages, places and records the files of `prepared`, and installs the server software
/// again when the Minecraft or loader version changes. The caller holds the instance's
/// change lock.
async fn apply(
    state: &AppState,
    prepared: &Prepared,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<ModpackUpdateResult, Error> {
    let Prepared {
        instance,
        stored,
        pack,
        origin,
        new_files,
        plan,
        summary,
        allow_scripts,
    } = prepared;
    let root = instance.path().await;
    let flavour = if summary.reinstalls_server {
        Some(server_flavour(&pack.info).await?)
    } else {
        None
    };
    stage_files(
        state.mods.modrinth.as_ref(),
        &stored.path(),
        new_files,
        plan,
        &root,
        *allow_scripts,
        progress,
    )
    .await?;
    progress("Replacing the modpack's files".to_string());
    {
        let (root, plan) = (root.clone(), plan.clone());
        spawn_blocking(move || place_files(&root, &plan)).await?;
    }
    if let Some(flavour) = flavour {
        if let Err(e) = instance
            .install_server_software(pack.info.game_version.clone(), flavour, progress)
            .await
        {
            let (root, plan) = (root.clone(), plan.clone());
            spawn_blocking(move || {
                undo_placed_files(&root, &plan);
                Ok(())
            })
            .await?;
            return Err(e);
        }
    }
    plan.record.save(&root).await?;
    instance.set_modpack_origin(Some(origin.clone())).await?;
    Ok(ModpackUpdateResult {
        added: summary.added.len() as u32,
        replaced: summary.replaced.len() as u32,
        removed: summary.removed.len() as u32,
        kept: summary.kept.len() as u32,
        skipped: describe_skipped(&pack.skipped),
    })
}

pub(super) async fn update_modpack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ModpackUpdateRequest>,
) -> Result<Json<ModpackUpdateResult>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft_instance(get_instance(&state, &uuid)?);
    // start() refuses to run while the guard is held, so the server stays stopped until
    // the update is done
    let guard = instance.lock_stopped("updating its modpack").await?;
    let prepared = prepare(&state, &requester, &uuid, &request.source).await?;
    if let Some(reason) = &prepared.summary.target.unsupported_reason {
        return Err(bad_request(reason.clone()));
    }
    let caused_by: CausedBy = requester.caused_by();
    // the update runs on its own task, so a dropped request can't stop it half way
    let task = tokio::spawn(async move {
        let _guard = guard;
        let name = prepared.instance.name().await;
        let title = format!(
            "Updating {name} to {} {}",
            prepared.origin.name,
            prepared.origin.version_number.clone().unwrap_or_default()
        );
        let broadcaster = state.event_broadcaster.clone();
        let (start, event_id) =
            Event::new_progression_event_start(title.trim_end(), None, None, caused_by);
        broadcaster.send(start);
        let progress = |message: String| {
            broadcaster.send(Event::new_progression_event_update(&event_id, message, 0.0));
        };
        let result = apply(&state, &prepared, &progress).await;
        remove_work_dirs(&prepared.instance.path().await).await;
        let message = match &result {
            Ok(done) => {
                let mut message = format!(
                    "Updated {name}: {} added, {} replaced, {} removed",
                    done.added, done.replaced, done.removed
                );
                if done.kept > 0 {
                    message.push_str(&format!(", {} changed file(s) kept", done.kept));
                }
                if let Some(skipped) = &done.skipped {
                    message.push_str(&format!(". {skipped}"));
                }
                message
            }
            Err(e) => format!("Updating {name}'s modpack failed: {e}"),
        };
        broadcaster.send(Event::new_progression_event_end(
            event_id,
            result.is_ok(),
            Some(message),
            None,
        ));
        result
    });
    let result = task
        .await
        .map_err(|e| eyre!("The modpack update task failed: {e}"))??;
    Ok(Json(result))
}
