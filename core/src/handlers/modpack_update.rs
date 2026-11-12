//! Updating a server's modpack, and going back to an earlier version of it:
//!
//! - `POST /instance/{uuid}/modpack/update/plan` shows what an update or a restore will
//!   do, and `POST /instance/{uuid}/modpack/update` does it. See
//!   [`crate::modpack::update`].
//! - `GET /instance/{uuid}/modpack/history` lists what was installed, updated and
//!   restored, and which entry each current file came with. See
//!   [`crate::modpack::history`].
//! - `POST /instance/{uuid}/modpack/check` looks for a newer version on Modrinth now
//!   (the core also checks on its own, see [`crate::modpack::updates`]).
//!
//! Changes need `WriteInstanceFile`, reading needs `ReadInstanceFile`. A pack from
//! Modrinth must be a version of the modpack the server came from; an uploaded pack
//! (`/modpack/upload`) can be any pack, and needs `WriteGlobalFile` too, as for creating a
//! server from one. A restore needs only `WriteInstanceFile`: it puts back files the
//! server had, from Lodestone's store or from the hosts packs download from.

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
use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event};
use crate::implementations::minecraft::{Flavour, MinecraftInstance};
use crate::modpack::history::{entry_as_is, entry_for, History, HistoryKind, ModpackHistory};
use crate::modpack::store::StoredPack;
use crate::modpack::update::{
    pack_files, place_files, plan_files, record_of, remove_work_dirs, stage_files,
    undo_placed_files, FileAction, FilePlan, ModpackUpdatePlan, ModpackUpdateResult, PackFile,
    PackRecord,
};
use crate::modpack::updates::{self, AvailableUpdate};
use crate::modpack::{
    describe_skipped, read_pack_async, server_flavour, ModpackInfo, ModpackOrigin, Pack,
};
use crate::mods::bad_request;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::types::InstanceUuid;
use crate::AppState;

/// What to update the server to.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateSource {
    Modrinth {
        version_id: String,
    },
    Upload {
        upload_id: String,
    },
    /// An entry of the server's modpack history: going back to it.
    History {
        entry_id: u32,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub struct ModpackUpdateRequest {
    pub source: UpdateSource,
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

/// What the server has before the change.
struct Before {
    origin: Option<ModpackOrigin>,
    game_version: String,
    flavour: Flavour,
    record: PackRecord,
    /// The files of the server's pack version, when they were read from Modrinth because
    /// there was no record: where they can be downloaded again.
    pack_files: Option<BTreeMap<String, PackFile>>,
}

/// Everything an update or restore needs, worked out before anything changes.
struct Prepared {
    instance: MinecraftInstance,
    kind: HistoryKind,
    before: Before,
    history: History,
    origin: ModpackOrigin,
    /// The pack, for an update from one. Keeps the pack file while the update runs.
    pack: Option<(StoredPack, Pack)>,
    /// The server software a restore goes back to; an update works it out from the pack.
    flavour: Option<Flavour>,
    new_files: Arc<BTreeMap<String, PackFile>>,
    plan: FilePlan,
    summary: ModpackUpdatePlan,
    allow_scripts: bool,
    /// The changelog of the version, for the history.
    changelog: Option<String>,
}

/// The files the server's current pack wrote: its record, else the files of the current
/// pack version when it is on Modrinth. With a note when neither is there.
async fn current_record(
    state: &AppState,
    root: &std::path::Path,
    origin: Option<&ModpackOrigin>,
    allow_scripts: bool,
) -> (
    PackRecord,
    Option<BTreeMap<String, PackFile>>,
    Option<String>,
) {
    if let Some(record) = PackRecord::load(root).await {
        return (record, None, None);
    }
    let no_record = "This server has no record of the files its modpack wrote, so files the \
                     new version drops are not removed."
        .to_string();
    let Some(version_id) = origin
        .map(|o| o.version_id.clone())
        .filter(|v| !v.is_empty())
    else {
        return (PackRecord::default(), None, Some(no_record));
    };
    let old = async {
        let stored = modrinth_pack(state, &version_id).await?;
        let pack = read_pack_async(stored.path(), allow_scripts).await?;
        spawn_blocking(move || pack_files(&pack, &stored.path(), allow_scripts)).await
    }
    .await;
    match old {
        Ok(files) => (record_of(&files), Some(files), None),
        Err(e) => {
            warn!("Could not read the server's current modpack version {version_id}: {e}");
            (PackRecord::default(), None, Some(no_record))
        }
    }
}

/// The plan of going to `new_files`, from what the server has.
async fn plan_against(
    root: &std::path::Path,
    before: &Before,
    new_files: &Arc<BTreeMap<String, PackFile>>,
) -> Result<FilePlan, Error> {
    let (root, old, new_files) = (root.to_path_buf(), before.record.clone(), new_files.clone());
    spawn_blocking(move || Ok(plan_files(&root, &old, &new_files))).await
}

async fn prepare(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
    source: &UpdateSource,
) -> Result<Prepared, Error> {
    let instance = minecraft_instance(get_instance(state, uuid)?);
    let root = instance.path().await;
    // scripts in a pack are only copied for the owner, while code execution is on
    let allow_scripts = requester.is_owner && state.code_execution_enabled();
    let origin_before = instance.modpack_origin().await;
    let (game_version, flavour) = instance.server_software().await;
    let (record, pack_files_before, note) =
        current_record(state, &root, origin_before.as_ref(), allow_scripts).await;
    let before = Before {
        origin: origin_before,
        game_version,
        flavour,
        record,
        pack_files: pack_files_before,
    };
    let history = History::load(&root).await;
    let current_loader = before.flavour.to_string();

    let (kind, origin, pack, flavour, new_files, plan, mut target, skipped) = match source {
        UpdateSource::Modrinth { version_id } => {
            let source = ModpackSource::Modrinth {
                version_id: version_id.clone(),
            };
            from_pack(state, requester, &root, &before, &source, allow_scripts).await?
        }
        UpdateSource::Upload { upload_id } => {
            let source = ModpackSource::Upload {
                upload_id: upload_id.clone(),
            };
            from_pack(state, requester, &root, &before, &source, allow_scripts).await?
        }
        UpdateSource::History { entry_id } => {
            let entry = history.entry(*entry_id).ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("That version is no longer in the server's modpack history"),
            })?;
            let new_files = Arc::new(
                history
                    .files_of(&root, *entry_id, allow_scripts)
                    .unwrap_or_default(),
            );
            let plan = plan_against(&root, &before, &new_files).await?;
            // files to put back that Lodestone can't get
            let missing: Vec<&str> = plan
                .changes
                .iter()
                .filter(|c| c.action != FileAction::Remove)
                .filter(|c| {
                    new_files
                        .get(&c.path)
                        .is_some_and(|f| f.stored.is_none() && f.download.is_none())
                })
                .map(|c| c.target.as_str())
                .collect();
            let unsupported_reason = if history.latest().is_some_and(|l| l.id == *entry_id) {
                Some("The server is already at this version".to_string())
            } else if missing.is_empty() {
                None
            } else {
                Some(format!(
                    "Lodestone no longer has {} file(s) of this version: {}",
                    missing.len(),
                    missing
                        .iter()
                        .take(5)
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            };
            let download_size = plan
                .changes
                .iter()
                .filter(|c| c.action != FileAction::Remove)
                .filter_map(|c| {
                    let file = new_files.get(&c.path)?;
                    file.stored
                        .is_none()
                        .then(|| file.download.as_ref().map(|d| d.size))
                        .flatten()
                })
                .sum();
            let target = ModpackInfo {
                name: entry.origin.name.clone(),
                version_number: entry.origin.version_number.clone(),
                summary: None,
                game_version: entry.game_version.clone(),
                loader: entry.flavour.to_string(),
                loader_version: entry.flavour.loader_version(),
                server_file_count: new_files.len() as u32,
                server_download_size: download_size,
                unsupported_reason,
                upload_id: None,
            };
            (
                HistoryKind::Restore { to: *entry_id },
                entry.origin.clone(),
                None,
                Some(entry.flavour.clone()),
                new_files,
                plan,
                target,
                Vec::new(),
            )
        }
    };

    target.upload_id = None;
    if flavour.is_none() && target.unsupported_reason.is_none() && current_loader != target.loader {
        target.unsupported_reason = Some(format!(
            "This server runs {current_loader}, and this version of the modpack needs {}",
            target.loader
        ));
    }
    let reinstalls_server = before.game_version != target.game_version
        || match &flavour {
            Some(flavour) => {
                flavour.to_string() != current_loader
                    || flavour.loader_version() != before.flavour.loader_version()
            }
            None => !runs_loader_version(&before.flavour, &target.game_version, &target),
        };
    let summary = ModpackUpdatePlan {
        current_game_version: before.game_version.clone(),
        current_loader,
        current_loader_version: before.flavour.loader_version(),
        reinstalls_server,
        added: plan.paths(FileAction::Add),
        replaced: plan.paths(FileAction::Replace),
        removed: plan.paths(FileAction::Remove),
        kept: plan.kept.clone(),
        skipped,
        note,
        target,
    };
    // a restore goes back to a version whose changelog the history has
    let changelog = match source {
        UpdateSource::History { entry_id } => {
            history.entry(*entry_id).and_then(|e| e.changelog.clone())
        }
        _ if !origin.version_id.is_empty() => {
            crate::modpack::modrinth_changelog(state.mods.modrinth.as_ref(), &origin.version_id)
                .await
        }
        _ => None,
    };
    Ok(Prepared {
        instance,
        kind,
        before,
        history,
        origin,
        pack,
        flavour,
        new_files,
        plan,
        summary,
        allow_scripts,
        changelog,
    })
}

type FromPack = (
    HistoryKind,
    ModpackOrigin,
    Option<(StoredPack, Pack)>,
    Option<Flavour>,
    Arc<BTreeMap<String, PackFile>>,
    FilePlan,
    ModpackInfo,
    Vec<String>,
);

/// An update to the pack `source` names.
async fn from_pack(
    state: &AppState,
    requester: &AuthedUser,
    root: &std::path::Path,
    before: &Before,
    source: &ModpackSource,
    allow_scripts: bool,
) -> Result<FromPack, Error> {
    let stored = stored_pack(state, requester, source).await?;
    let pack = read_pack_async(stored.path(), allow_scripts).await?;
    let origin = match &stored.version_id {
        Some(version_id) => {
            ModpackOrigin::from_modrinth(state.mods.modrinth.as_ref(), version_id).await?
        }
        None => ModpackOrigin::from_upload(&pack.info),
    };
    if let Some(current) = before.origin.as_ref().filter(|c| !c.project_id.is_empty()) {
        if !origin.project_id.is_empty() && origin.project_id != current.project_id {
            return Err(bad_request(format!(
                "{} is not a version of {}, the modpack this server came from",
                origin.name, current.name
            )));
        }
    }
    let new_files = {
        let (pack, path) = (pack.clone(), stored.path());
        Arc::new(spawn_blocking(move || pack_files(&pack, &path, allow_scripts)).await?)
    };
    let plan = plan_against(root, before, &new_files).await?;
    let target = pack.info.clone();
    let skipped = pack
        .skipped
        .iter()
        .map(|s| format!("{} ({})", s.path, s.reason))
        .collect();
    Ok((
        HistoryKind::Update,
        origin,
        Some((stored, pack)),
        None,
        new_files,
        plan,
        target,
        skipped,
    ))
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

/// Adds the change to the server's history: keeps the files it took off the server that
/// can't be downloaded again, and remembers where the new ones come from. A server
/// without a history gets an entry for what it had first.
async fn record_history(prepared: &Prepared, root: &std::path::Path) -> Result<(), Error> {
    let mut history = prepared.history.clone();
    let before = &prepared.before;
    if history.entries.is_empty() {
        if let Some(origin) = &before.origin {
            let baseline = entry_as_is(
                HistoryKind::Baseline,
                origin.clone(),
                before.game_version.clone(),
                before.flavour.clone(),
                &before.record,
            );
            history.entries.push(baseline);
            history.entries[0].id = 1;
        }
    }
    if let Some(files) = &before.pack_files {
        history.remember_downloads(files);
    }
    let (game_version, flavour) = prepared.instance.server_software().await;
    let mut entry = entry_for(
        prepared.kind.clone(),
        prepared.origin.clone(),
        game_version,
        flavour,
        &before.record,
        &prepared.plan,
    );
    entry.changelog = prepared.changelog.clone();
    let (root, plan, new_files) = (
        root.to_path_buf(),
        prepared.plan.clone(),
        prepared.new_files.clone(),
    );
    let history = spawn_blocking(move || {
        history.record_change(&root, &plan, &new_files, entry);
        Ok(history)
    })
    .await?;
    history.save(&prepared.instance.path().await).await
}

/// Stages, places and records the files of `prepared`, and installs the server software
/// again when the Minecraft or loader version changes. The caller holds the instance's
/// change lock.
async fn apply(
    state: &AppState,
    prepared: &Prepared,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<ModpackUpdateResult, Error> {
    let instance = &prepared.instance;
    let summary = &prepared.summary;
    let plan = &prepared.plan;
    let root = instance.path().await;
    let flavour = match (&prepared.flavour, summary.reinstalls_server) {
        (_, false) => None,
        (Some(flavour), true) => Some(flavour.clone()),
        (None, true) => Some(server_flavour(&summary.target).await?),
    };
    let pack_path = prepared.pack.as_ref().map(|(stored, _)| stored.path());
    stage_files(
        state.mods.modrinth.as_ref(),
        pack_path.as_deref(),
        &prepared.new_files,
        plan,
        &root,
        prepared.allow_scripts,
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
            .install_server_software(summary.target.game_version.clone(), flavour, progress)
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
    // the files are in place: a history that can't be written only loses the undo
    if let Err(e) = record_history(prepared, &root).await {
        warn!("Could not add the modpack change to the history: {e}");
    }
    instance
        .set_modpack_origin(Some(prepared.origin.clone()))
        .await?;
    Ok(ModpackUpdateResult {
        added: summary.added.len() as u32,
        replaced: summary.replaced.len() as u32,
        removed: summary.removed.len() as u32,
        kept: summary.kept.len() as u32,
        skipped: prepared
            .pack
            .as_ref()
            .and_then(|(_, pack)| describe_skipped(&pack.skipped)),
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
    // the update is done; the mods lock keeps the Mods tab's changes out of the way
    let guard = instance.lock_stopped("changing its modpack").await?;
    let mods_guard = state.mods.lock_for(&uuid).lock_owned().await;
    let prepared = prepare(&state, &requester, &uuid, &request.source).await?;
    if let Some(reason) = &prepared.summary.target.unsupported_reason {
        return Err(bad_request(reason.clone()));
    }
    let caused_by: CausedBy = requester.caused_by();
    // the update runs on its own task, so a dropped request can't stop it half way
    let task = tokio::spawn(async move {
        let _guards = (guard, mods_guard);
        let name = prepared.instance.name().await;
        let version = format!(
            "{} {}",
            prepared.origin.name,
            prepared.origin.version_number.clone().unwrap_or_default()
        );
        let version = version.trim_end();
        let restore = matches!(prepared.kind, HistoryKind::Restore { .. });
        let title = if restore {
            format!("Restoring {name} to {version}")
        } else {
            format!("Updating {name} to {version}")
        };
        let broadcaster = state.event_broadcaster.clone();
        let (start, event_id) = Event::new_progression_event_start(title, None, None, caused_by);
        broadcaster.send(start);
        let progress = |message: String| {
            broadcaster.send(Event::new_progression_event_update(&event_id, message, 0.0));
        };
        let result = apply(&state, &prepared, &progress).await;
        remove_work_dirs(&prepared.instance.path().await).await;
        let message = match &result {
            Ok(done) => {
                let verb = if restore { "Restored" } else { "Updated" };
                let mut message = format!(
                    "{verb} {name} to {version}: {} added, {} replaced, {} removed",
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
            Err(e) if restore => format!("Restoring {name}'s modpack failed: {e}"),
            Err(e) => format!("Updating {name}'s modpack failed: {e}"),
        };
        broadcaster.send(Event::new_progression_event_end(
            event_id,
            result.is_ok(),
            Some(message),
            None,
        ));
        if result.is_ok() {
            updates::forget(&state, &prepared.instance.uuid().await);
        }
        result
    });
    let result = task
        .await
        .map_err(|e| eyre!("The modpack update task failed: {e}"))??;
    Ok(Json(result))
}

pub(super) async fn modpack_history(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<ModpackHistory>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft_instance(get_instance(&state, &uuid)?);
    let root = instance.path().await;
    let history = History::load(&root).await;
    let update = state.mods.updates.get(&uuid);
    let info = spawn_blocking(move || Ok(history.info(&root, update))).await?;
    Ok(Json(info))
}

pub(super) async fn check_modpack(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<Option<AvailableUpdate>>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    let instance = minecraft_instance(get_instance(&state, &uuid)?);
    Ok(Json(updates::check_instance(&state, &instance).await?))
}
