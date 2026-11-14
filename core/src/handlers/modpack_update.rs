//! Updating a server's modpack, and going back to an earlier version of its content:
//!
//! - `POST /instance/{uuid}/modpack/update/plan` shows what an update or a restore will
//!   do, and `POST /instance/{uuid}/modpack/update` does it. See
//!   [`crate::modpack::update`] and [`crate::modpack::lineage`].
//! - `GET /instance/{uuid}/modpack/history` lists the server's content images newest
//!   first, what each changed, and which one each current file came with. See
//!   [`crate::modpack::lineage`].
//! - `POST /instance/{uuid}/modpack/check` looks for a newer version on Modrinth now
//!   (the core also checks on its own, see [`crate::modpack::updates`]).
//!
//! Changes need `WriteInstanceFile`, reading needs `ReadInstanceFile`. A pack from
//! Modrinth must be a version of the modpack the server came from; an uploaded pack
//! (`/modpack/upload`) can be any pack, and needs `WriteGlobalFile` too, as for creating a
//! server from one. A restore needs only `WriteInstanceFile`: it puts back files the
//! server had, from the image store or from Modrinth. A protected file it puts back (a
//! jar, see [`is_protected`]) must be one Modrinth knows, as for adding a mod, or the
//! restore needs `WriteGlobalFile`: an image holds whatever was on the server, also a jar
//! someone who could write global files put there and took away again.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use color_eyre::eyre::eyre;
use lodestone_image::store::Store;
use lodestone_image::Base;
use serde::{Deserialize, Serialize};
use tracing::warn;
use ts_rs::TS;

use super::instance_fs::is_protected;
use super::modpack::{modrinth_pack, stored_pack, ModpackSource};
use super::util::{get_instance, AuthedUser};
use crate::auth::user::UserAction;
use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event};
use crate::images::base::BaseInfo;
use crate::implementations::minecraft::{Flavour, MinecraftInstance};
use crate::modpack::lineage::{self, ModpackHistory, RestoreTarget};
use crate::modpack::pack_info;
use crate::modpack::store::StoredPack;
use crate::modpack::update::{
    begin_update, check_as_planned, commit_update, end_update, keep_unwritten_scripts,
    other_copies, pack_files, place_files, plan_files, record_of, recover_interrupted,
    remove_other_copies, remove_work_dirs, sources_of, stage_files, undo_placed_files, FileAction,
    FilePlan, FileSource, ModpackUpdatePlan, ModpackUpdateResult, PackFile, PackRecord,
};
use crate::modpack::updates::{self, AvailableUpdate};
use crate::modpack::{
    describe_skipped, read_pack_async, server_flavour, ModpackInfo, ModpackOrigin, Pack,
};
use crate::modrinth::ModrinthApi;
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
    /// An entry of the server's history (a content image, by id): going back to it.
    History {
        entry_id: String,
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
}

/// An update to another version of the pack.
struct PackUpdate {
    origin: ModpackOrigin,
    /// Keeps the pack file while the update runs.
    stored: StoredPack,
    pack: Pack,
    /// The pack's files.
    new_files: Arc<BTreeMap<String, PackFile>>,
}

/// What a change is.
enum Change {
    Update(Box<PackUpdate>),
    /// Back to a content image of the server's history.
    Restore(Box<RestoreTarget>),
}

impl Change {
    fn is_restore(&self) -> bool {
        matches!(self, Change::Restore(_))
    }

    /// The label of the content image the change records.
    fn label(&self) -> String {
        match self {
            Change::Update(update) => lineage::update_label(&update.origin),
            Change::Restore(target) => lineage::restore_label(target.label.as_deref()),
        }
    }

    /// What the server goes to, as messages name it.
    fn what(&self) -> String {
        match self {
            Change::Update(update) => {
                let origin = &update.origin;
                let version = format!(
                    "{} {}",
                    origin.name,
                    origin.version_number.clone().unwrap_or_default()
                );
                version.trim_end().to_string()
            }
            Change::Restore(target) => match target.label.as_deref().map(str::trim) {
                Some(label) if !label.is_empty() => format!("\u{201c}{label}\u{201d}"),
                _ => "an earlier version".to_string(),
            },
        }
    }
}

/// A change worked out against the server, before the summary.
struct Planned {
    change: Change,
    /// The server software a restore goes back to; an update works it out from the pack.
    flavour: Option<Flavour>,
    sources: BTreeMap<String, FileSource>,
    plan: FilePlan,
    target: ModpackInfo,
    skipped: Vec<String>,
    note: Option<String>,
}

/// Everything an update or restore needs, worked out before anything changes.
struct Prepared {
    instance: MinecraftInstance,
    change: Change,
    flavour: Option<Flavour>,
    sources: Arc<BTreeMap<String, FileSource>>,
    plan: FilePlan,
    summary: ModpackUpdatePlan,
    allow_scripts: bool,
}

/// The files the server's current pack wrote: from its `pack.json`, else the files of the
/// current pack version when it is on Modrinth. With a note when neither is there.
async fn current_record(
    state: &AppState,
    uuid: &InstanceUuid,
    origin: Option<&ModpackOrigin>,
    allow_scripts: bool,
) -> (PackRecord, Option<String>) {
    let saved = {
        let (images, uuid) = (state.images().clone(), uuid.clone());
        spawn_blocking(move || Ok(pack_info::load(&images, &uuid)))
            .await
            .ok()
            .flatten()
    };
    if let Some(record) = saved
        .map(|info| pack_info::record_of_base(&info))
        .filter(|record| !record.files.is_empty())
    {
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
        spawn_blocking(move || pack_files(&pack, &stored.path(), allow_scripts)).await
    }
    .await;
    match old {
        Ok(files) => (record_of(&files), None),
        Err(e) => {
            warn!("Could not read the server's current modpack version {version_id}: {e}");
            (PackRecord::default(), Some(no_record))
        }
    }
}

/// The plan of going from `record` to `new_files`, from what the server has.
async fn plan_against(
    state: &AppState,
    root: &std::path::Path,
    record: &PackRecord,
    new_files: &Arc<BTreeMap<String, PackFile>>,
    allow_scripts: bool,
) -> Result<FilePlan, Error> {
    let mut plan = {
        let (root, old, new_files) = (root.to_path_buf(), record.clone(), new_files.clone());
        spawn_blocking(move || Ok(plan_files(&root, &old, &new_files))).await?
    };
    if !allow_scripts {
        keep_unwritten_scripts(&mut plan, record, new_files);
    }
    // another version of a mod the plan adds, such as one updated in the Mods tab
    let others = other_copies(state.mods.modrinth.as_ref(), root, record, new_files, &plan).await;
    remove_other_copies(&mut plan, new_files, &others);
    Ok(plan)
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
    let (game_version, flavour) = instance.server_software().await;
    let before = Before {
        origin: instance.modpack_origin().await,
        game_version,
        flavour,
    };
    let current_loader = before.flavour.to_string();
    let planned = match source {
        UpdateSource::Modrinth { version_id } => {
            let source = ModpackSource::Modrinth {
                version_id: version_id.clone(),
            };
            from_pack(
                state,
                requester,
                uuid,
                &root,
                &before,
                &source,
                allow_scripts,
            )
            .await?
        }
        UpdateSource::Upload { upload_id } => {
            let source = ModpackSource::Upload {
                upload_id: upload_id.clone(),
            };
            from_pack(
                state,
                requester,
                uuid,
                &root,
                &before,
                &source,
                allow_scripts,
            )
            .await?
        }
        UpdateSource::History { entry_id } => {
            from_history(state, uuid, &before, entry_id, allow_scripts).await?
        }
    };
    let Planned {
        change,
        flavour,
        sources,
        plan,
        mut target,
        skipped,
        note,
    } = planned;

    target.upload_id = None;
    if !change.is_restore()
        && target.unsupported_reason.is_none()
        && current_loader != target.loader
    {
        target.unsupported_reason = Some(format!(
            "This server runs {current_loader}, and this version of the modpack needs {}",
            target.loader
        ));
    }
    let reinstalls_server = match (&change, &flavour) {
        (Change::Restore(_), flavour) => flavour.is_some(),
        (Change::Update(_), _) => {
            before.game_version != target.game_version
                || !runs_loader_version(&before.flavour, &target.game_version, &target)
        }
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
    Ok(Prepared {
        instance,
        change,
        flavour,
        sources: Arc::new(sources),
        plan,
        summary,
        allow_scripts,
    })
}

/// An update to the pack `source` names.
async fn from_pack(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
    root: &std::path::Path,
    before: &Before,
    source: &ModpackSource,
    allow_scripts: bool,
) -> Result<Planned, Error> {
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
    let (record, note) = current_record(state, uuid, before.origin.as_ref(), allow_scripts).await;
    let new_files = {
        let (pack, path) = (pack.clone(), stored.path());
        Arc::new(spawn_blocking(move || pack_files(&pack, &path, allow_scripts)).await?)
    };
    let plan = plan_against(state, root, &record, &new_files, allow_scripts).await?;
    let target = pack.info.clone();
    let skipped = pack
        .skipped
        .iter()
        .map(|s| format!("{} ({})", s.path, s.reason))
        .collect();
    Ok(Planned {
        sources: sources_of(&new_files),
        change: Change::Update(Box::new(PackUpdate {
            origin,
            stored,
            pack,
            new_files,
        })),
        flavour: None,
        plan,
        target,
        skipped,
        note,
    })
}

/// `paths` for a message: the first five, comma-separated.
fn first_five(paths: &[String]) -> String {
    paths
        .iter()
        .take(5)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A restore of the content image `entry_id` ([`lineage::plan_restore`]).
async fn from_history(
    state: &AppState,
    uuid: &InstanceUuid,
    before: &Before,
    entry_id: &str,
    allow_scripts: bool,
) -> Result<Planned, Error> {
    let not_found = || Error {
        kind: ErrorKind::NotFound,
        source: eyre!("That version is no longer in the server's history"),
    };
    let images = state.images().clone();
    let current = images.current_content(uuid).await.ok_or_else(not_found)?;
    let ctx = crate::images::context_of(state, uuid)
        .await
        .ok_or_else(not_found)?;
    let restore = {
        let (store, uuid, entry_id) = (images.store(), uuid.clone(), entry_id.to_string());
        let current = current.clone();
        spawn_blocking(move || {
            let (layout, _) = crate::images::recorder::content_scope(&ctx)?;
            lineage::plan_restore(
                &store,
                &uuid,
                &ctx.root,
                &layout,
                &current,
                &entry_id,
                allow_scripts,
            )
        })
        .await?
    };
    let target = restore.target.clone();
    let now = crate::images::layout::game_of(&before.game_version, &before.flavour);
    let flavour = if target.game == now {
        None
    } else {
        lineage::flavour_of_game(&target.game)
    };
    let unsupported_reason = if entry_id == current {
        Some("The server is already at this version".to_string())
    } else if target.game != now && flavour.is_none() {
        Some(format!(
            "This version ran Minecraft {} with {}, which a restore can't install",
            target.game.minecraft,
            target.game.loader.as_str()
        ))
    } else if !restore.unrecorded.is_empty() {
        Some(format!(
            "Lodestone has no copy of {} file(s) this version would replace, so the restore couldn't be undone: {}",
            restore.unrecorded.len(),
            first_five(&restore.unrecorded)
        ))
    } else if !restore.blocked.is_empty() {
        Some(format!(
            "Something other than a file is where this version has {} file(s): {}",
            restore.blocked.len(),
            first_five(&restore.blocked)
        ))
    } else if !restore.missing.is_empty() {
        Some(format!(
            "Lodestone no longer has {} file(s) of this version: {}",
            restore.missing.len(),
            first_five(&restore.missing)
        ))
    } else {
        None
    };
    let info = ModpackInfo {
        name: target.label.clone().unwrap_or_default(),
        version_number: None,
        summary: None,
        game_version: target.game.minecraft.clone(),
        loader: target.game.loader.as_str().to_string(),
        loader_version: target.game.loader_version.clone(),
        server_file_count: restore.file_count,
        server_download_size: restore.download_size,
        unsupported_reason,
        upload_id: None,
    };
    Ok(Planned {
        change: Change::Restore(Box::new(target)),
        flavour,
        sources: restore.sources,
        plan: restore.plan,
        target: info,
        skipped: Vec::new(),
        note: None,
    })
}

/// The protected files (see [`is_protected`]) that `plan` adds or replaces, with the
/// SHA-512 of the bytes it would put there: the download's, which it is checked against,
/// else the image store copy's. `None` when that can't be had. Reads the image store, so
/// call it on a blocking thread.
fn protected_new_files(
    plan: &FilePlan,
    sources: &BTreeMap<String, FileSource>,
    images: Option<&Store>,
) -> Vec<(String, Option<String>)> {
    plan.changes
        .iter()
        .filter(|c| c.action != FileAction::Remove)
        .filter(|c| {
            is_protected(std::path::Path::new(&c.path), false)
                || is_protected(std::path::Path::new(&c.target), false)
        })
        .map(|c| {
            let sha512 = match sources.get(&c.path) {
                Some(FileSource::Download(download)) => Some(download.sha512.clone()),
                Some(FileSource::Image(file)) => {
                    images.and_then(|s| lineage::image_sha512(s, file))
                }
                Some(FileSource::Override) | None => None,
            };
            (c.target.clone(), sha512.map(|h| h.to_lowercase()))
        })
        .collect()
}

/// The protected files (see [`is_protected`]) a restore's `plan` removes, which only a user
/// who can write global files may delete, as through the Files API. Mod and plugin jars
/// (`mods/*.jar`, `plugins/*.jar`, enabled or disabled) are the exception: the Mods tab lets
/// any user who may change the server's files remove those.
fn protected_removals(plan: &FilePlan) -> Vec<String> {
    let removable_jar = |path: &str| {
        let lower = path.to_ascii_lowercase();
        let name = lower.strip_suffix(".disabled").unwrap_or(&lower);
        let mut parts = name.split('/');
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some("mods" | "plugins"), Some(jar), None) if jar.ends_with(".jar")
        )
    };
    plan.changes
        .iter()
        .filter(|c| c.action == FileAction::Remove)
        .filter(|c| is_protected(std::path::Path::new(&c.target), false))
        .filter(|c| !removable_jar(&c.target))
        .map(|c| c.target.clone())
        .collect()
}

/// For a restore by a user without `WriteGlobalFile`: fails unless Modrinth knows every
/// protected file it puts back. An image holds whatever was on the server, so without this
/// a restore could put back a jar that only a user who can write global files could add
/// (and maybe took away again). Uploading or enabling a mod has the same rule.
async fn require_known_restored_files(
    api: &dyn ModrinthApi,
    plan: &FilePlan,
    sources: &Arc<BTreeMap<String, FileSource>>,
    images: Option<Arc<Store>>,
) -> Result<(), Error> {
    let files = {
        let (plan, sources) = (plan.clone(), sources.clone());
        spawn_blocking(move || Ok(protected_new_files(&plan, &sources, images.as_deref()))).await?
    };
    if files.is_empty() {
        return Ok(());
    }
    let hashes: Vec<String> = files.iter().filter_map(|(_, h)| h.clone()).collect();
    let known = if hashes.is_empty() {
        Default::default()
    } else {
        api.versions_by_hash(&hashes).await?
    };
    let unknown: Vec<String> = files
        .iter()
        .filter(|(_, hash)| !hash.as_ref().is_some_and(|h| known.contains_key(h)))
        .map(|(target, _)| target.clone())
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!(
            "This version puts back {} file(s) that are not on Modrinth ({}). Only users who can write global files can restore it",
            unknown.len(),
            first_five(&unknown)
        ),
    })
}

/// [`require_known_restored_files`] when `prepared` is a restore and `requester` can't
/// write global files.
async fn check_restore(
    state: &AppState,
    requester: &AuthedUser,
    prepared: &Prepared,
) -> Result<(), Error> {
    if !prepared.change.is_restore() || requester.can_perform_action(&UserAction::WriteGlobalFile) {
        return Ok(());
    }
    let removed = protected_removals(&prepared.plan);
    if !removed.is_empty() {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "This version removes {} protected file(s) ({}). Only users who can write global files can restore it",
                removed.len(),
                first_five(&removed)
            ),
        });
    }
    require_known_restored_files(
        state.mods.modrinth.as_ref(),
        &prepared.plan,
        &prepared.sources,
        Some(state.images().store()),
    )
    .await
}

pub(crate) async fn plan_update(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(request): Json<ModpackUpdateRequest>,
) -> Result<Json<ModpackUpdatePlan>, Error> {
    requester
        .require(&state, UserAction::WriteInstanceFile(uuid.clone()))
        .await?;
    let mut prepared = prepare(&state, &requester, &uuid, &request.source).await?;
    if prepared.summary.target.unsupported_reason.is_none() {
        if let Err(e) = check_restore(&state, &requester, &prepared).await {
            prepared.summary.target.unsupported_reason = Some(e.source.to_string());
        }
    }
    Ok(Json(prepared.summary))
}

/// How long a restore waits for Modrinth (the restored version's `.mrpack` and its name)
/// before it goes on without.
const MODRINTH_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// What the server is built on after the change: its `pack.json` and modpack origin, each
/// `None` when it stays as it is.
struct After {
    pack: Option<BaseInfo>,
    origin: Option<ModpackOrigin>,
}

/// [`After`] for `prepared`, worked out before anything changes (a restore may download
/// the pack it goes back to).
async fn after_change(state: &AppState, prepared: &Prepared, uuid: &InstanceUuid) -> After {
    match &prepared.change {
        Change::Update(update) => {
            let origin = &update.origin;
            let (origin_for_pack, path, record, files) = (
                origin.clone(),
                update.stored.path(),
                prepared.plan.record.clone(),
                update.new_files.clone(),
            );
            let pack = spawn_blocking(move || {
                Ok(pack_info::base_info(
                    &origin_for_pack,
                    Some(&path),
                    &record,
                    &files,
                ))
            })
            .await
            .ok();
            After {
                pack,
                origin: Some(origin.clone()),
            }
        }
        Change::Restore(target) => restored_pack(state, prepared, uuid, target).await,
    }
}

/// [`After`] for a restore of `target`. The same base as now changes nothing. Another
/// Modrinth pack version becomes the origin, and its files the `pack.json`, read from its
/// `.mrpack` when Lodestone can get it and it is the one the image names (by SHA-512);
/// otherwise the `pack.json` has the base without files, so the next update falls back to
/// the pack itself. An image without a pack keeps the origin (the server may still be made
/// from an uploaded pack) and a `pack.json` without base or files.
async fn restored_pack(
    state: &AppState,
    prepared: &Prepared,
    uuid: &InstanceUuid,
    target: &RestoreTarget,
) -> After {
    let current = {
        let (images, uuid) = (state.images().clone(), uuid.clone());
        spawn_blocking(move || Ok(pack_info::load(&images, &uuid)))
            .await
            .ok()
            .flatten()
    };
    if current.as_ref().and_then(|info| info.base.as_ref()) == target.base.as_ref() {
        return After {
            pack: None,
            origin: None,
        };
    }
    let without_files = BaseInfo {
        base: target.base.clone(),
        files: BTreeMap::new(),
    };
    let Some(Base::ModrinthPack {
        project,
        version,
        sha512,
    }) = &target.base
    else {
        return After {
            pack: Some(without_files),
            origin: None,
        };
    };
    let origin = match tokio::time::timeout(
        MODRINTH_WAIT,
        ModpackOrigin::from_modrinth(state.mods.modrinth.as_ref(), version),
    )
    .await
    {
        Ok(Ok(origin)) if origin.project_id == *project => origin,
        _ => {
            warn!("Could not look up the modpack version {version}; keeping the pack's name");
            let mut origin =
                prepared
                    .instance
                    .modpack_origin()
                    .await
                    .unwrap_or_else(|| ModpackOrigin {
                        project_id: String::new(),
                        version_id: String::new(),
                        name: "Modpack".into(),
                        version_number: None,
                        icon_url: None,
                    });
            origin.project_id = project.clone();
            origin.version_id = version.clone();
            origin.version_number = None;
            origin
        }
    };
    let pack = match tokio::time::timeout(MODRINTH_WAIT, modrinth_pack(state, version)).await {
        Ok(Ok(stored)) => {
            let (origin, allow_scripts, sha512) =
                (origin.clone(), prepared.allow_scripts, sha512.clone());
            spawn_blocking(move || {
                let path = stored.path();
                if pack_info::pack_sha512(&path).as_deref() != Some(sha512.as_str()) {
                    return Err(eyre!("it isn't the pack the image was built on").into());
                }
                let pack = crate::modpack::read_pack(&path, allow_scripts)?;
                let files = pack_files(&pack, &path, allow_scripts)?;
                Ok(pack_info::base_info(
                    &origin,
                    Some(&path),
                    &record_of(&files),
                    &files,
                ))
            })
            .await
            .map_err(|e| warn!("Could not read the modpack version {version}: {e}"))
            .ok()
        }
        Ok(Err(e)) => {
            warn!("Could not get the modpack version {version}: {e}");
            None
        }
        Err(_) => {
            warn!("Gave up getting the modpack version {version}");
            None
        }
    };
    After {
        pack: Some(pack.unwrap_or(without_files)),
        origin: Some(origin),
    }
}

/// Stages, places and records the files of `prepared`, and installs the server software
/// again when the Minecraft or loader version changes. The caller holds the instance's
/// change lock and the mods lock.
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
    let uuid = instance.uuid().await;
    let images = state.images().clone();
    let after = after_change(state, prepared, &uuid).await;
    let (pack_path, skipped) = match &prepared.change {
        Change::Update(update) => (
            Some(update.stored.path()),
            describe_skipped(&update.pack.skipped),
        ),
        Change::Restore(_) => (None, None),
    };
    {
        // A restore copies objects out of the store: no collection deletes them meanwhile.
        let _hold = match &prepared.change {
            Change::Restore(_) => {
                let store = images.store();
                Some(
                    spawn_blocking(move || {
                        store.hold_off_collection().map_err(|e| eyre!("{e}").into())
                    })
                    .await?,
                )
            }
            Change::Update(_) => None,
        };
        stage_files(
            state.mods.modrinth.as_ref(),
            pack_path.as_deref(),
            &prepared.sources,
            plan,
            &root,
            prepared.allow_scripts,
            Some(images.store()),
            progress,
        )
        .await?;
    }
    // Staging wrote only Lodestone's own folder. From here to the record the change has the
    // server's content to itself: no Files API operation runs between the check that every
    // file it changes is as planned (nothing new where it adds one) and their placement,
    // and the mods lock keeps the Mods tab out. The copy and unzip jobs hold no gate, so one
    // may still write meanwhile: placing, and undoing a placement, never replaces a file,
    // so such a write fails the change rather than being lost. Dropped on failure, which
    // records nothing.
    let op = images.begin_exclusive(&uuid, prepared.change.label()).await;
    progress("Replacing the server's files".to_string());
    {
        // the journal lets the next start put the files back if the core stops before
        // the change is committed (LS-19)
        let (root, plan) = (root.clone(), plan.clone());
        spawn_blocking(move || {
            check_as_planned(&root, &plan)?;
            begin_update(&root, &plan)?;
            let placed = place_files(&root, &plan);
            if placed.is_err() {
                // place_files put the files back
                end_update(&root)?;
            }
            placed
        })
        .await?;
    }
    let undo = || {
        let (root, plan) = (root.clone(), plan.clone());
        spawn_blocking(move || {
            undo_placed_files(&root, &plan);
            end_update(&root)
        })
    };
    if let Some(flavour) = flavour {
        if let Err(e) = instance
            .install_server_software(summary.target.game_version.clone(), flavour, progress)
            .await
        {
            undo().await?;
            return Err(e);
        }
    }
    {
        let root = root.clone();
        if let Err(e) = spawn_blocking(move || commit_update(&root)).await {
            undo().await?;
            return Err(e);
        }
    }
    // the change is complete: what the server is built on now, then its content image
    if let Some(pack) = after.pack {
        pack_info::save(&images, &uuid, pack).await;
    }
    let origin = match after.origin {
        Some(origin) => instance.set_modpack_origin(Some(origin)).await,
        None => Ok(()),
    };
    {
        let root = root.clone();
        if let Err(e) = spawn_blocking(move || end_update(&root)).await {
            warn!("{}", e.source);
        }
    }
    match crate::images::context_of(state, &uuid).await {
        Some(ctx) => {
            if let Err(e) = op.succeed_and_record(ctx).await {
                warn!(
                    "Could not record the content image of {uuid} after its modpack changed: {}",
                    e.source
                );
            }
        }
        None => drop(op),
    }
    origin?;
    Ok(ModpackUpdateResult {
        added: summary.added.len() as u32,
        replaced: summary.replaced.len() as u32,
        removed: summary.removed.len() as u32,
        kept: summary.kept.len() as u32,
        skipped,
    })
}

pub(crate) async fn update_modpack(
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
    // with both held no other update runs, so an unfinished one was interrupted
    {
        let root = instance.path().await;
        spawn_blocking(move || Ok(recover_interrupted(&root))).await?;
    }
    let restoring = match &request.source {
        UpdateSource::History { entry_id } => {
            // refused before anything is recorded, when the version isn't in the history
            let (store, id, entry) = (state.images().store(), uuid.clone(), entry_id.clone());
            spawn_blocking(move || lineage::check_in_history(&store, &id, &entry)).await?;
            Some(state.images().keep_while_restoring(&uuid, entry_id))
        }
        _ => None,
    };
    // the content as it is now, so the change is planned from it and what it takes off the
    // server is in an image. A change refused after this (a target the server can't run,
    // a file the user may not replace) leaves that image: it holds what was on disk.
    if let Some(ctx) = crate::images::context_of(&state, &uuid).await {
        if let Err(e) = lineage::before_change(state.images(), ctx, restoring.is_some()).await {
            if restoring.is_some() {
                return Err(Error {
                    kind: e.kind,
                    source: e
                        .source
                        .wrap_err("Could not record the server's content before restoring"),
                });
            }
            warn!(
                "Could not record the content of {uuid} before its modpack changed: {}",
                e.source
            );
        }
    }
    let prepared = prepare(&state, &requester, &uuid, &request.source).await?;
    if let Some(reason) = &prepared.summary.target.unsupported_reason {
        return Err(bad_request(reason.clone()));
    }
    check_restore(&state, &requester, &prepared).await?;
    let caused_by: CausedBy = requester.caused_by();
    // the update runs on its own task, so a dropped request can't stop it half way
    let task = tokio::spawn(async move {
        let _guards = (guard, mods_guard, restoring);
        let name = prepared.instance.name().await;
        let what = prepared.change.what();
        let restore = prepared.change.is_restore();
        let title = if restore {
            format!("Restoring {name} to {what}")
        } else {
            format!("Updating {name} to {what}")
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
                    "{verb} {name} to {what}: {} added, {} replaced, {} removed",
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
            Err(e) if restore => format!("Restoring {name} failed: {e}"),
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

pub(crate) async fn modpack_history(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<ModpackHistory>, Error> {
    requester
        .require(&state, UserAction::ReadInstanceFile(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?;
    let update = state.mods.updates.get(&uuid);
    let images = state.images().clone();
    let history = spawn_blocking(move || Ok(lineage::history(&images, &uuid, update))).await?;
    Ok(Json(history))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modpack::update::{Expected, FileChange, ImageFile};
    use crate::modpack::PackDownload;
    use crate::modrinth::fake::{sha1_hex, sha512_hex, FakeModrinth, FakeVersion};

    fn download(path: &str, content: &[u8], url: &str) -> FileSource {
        FileSource::Download(PackDownload {
            path: path.into(),
            urls: vec![url::Url::parse(url).unwrap()],
            sha1: sha1_hex(content),
            sha512: sha512_hex(content),
            size: content.len() as u64,
        })
    }

    fn adds(paths: &[&str]) -> FilePlan {
        FilePlan {
            changes: paths
                .iter()
                .map(|path| FileChange {
                    path: path.to_string(),
                    target: path.to_string(),
                    action: FileAction::Add,
                    expected: Expected::Nothing,
                })
                .collect(),
            ..Default::default()
        }
    }

    /// A restore removes protected files only for a user who may delete them through the
    /// Files API, but mod and plugin jars, which the Mods tab lets anyone remove.
    #[test]
    fn protected_removals_spare_mod_and_plugin_jars() {
        let plan = FilePlan {
            changes: [
                "mods/a.jar",
                "mods/b.jar.disabled",
                "plugins/x.jar",
                "config/start-hook.sh",
                "config/native.dll",
                "config/somefile",
                "config/a.toml",
                "mods/sub/c.jar",
            ]
            .iter()
            .map(|path| FileChange {
                path: path.to_string(),
                target: path.to_string(),
                action: FileAction::Remove,
                expected: Expected::Nothing,
            })
            .collect(),
            ..Default::default()
        };
        assert_eq!(
            protected_removals(&plan),
            [
                "config/start-hook.sh",
                "config/native.dll",
                "config/somefile",
                "mods/sub/c.jar"
            ]
        );
        // adding is the other check's
        assert!(protected_removals(&adds(&["config/native.dll"])).is_empty());
    }

    /// A restore by a user who can't write global files puts a jar into `mods/` only when
    /// Modrinth knows its bytes (LS-02): a download by the hash it is checked against, a
    /// copy in the image store by its own bytes.
    #[tokio::test]
    async fn a_restore_only_puts_back_jars_modrinth_knows() {
        let api = FakeModrinth::default();
        let known = FakeVersion::new("sodium", "v1", "sodium.jar");
        let known_content = known.content.clone();
        api.add(known);
        let evil = b"not on modrinth";
        let sources = Arc::new(BTreeMap::from([
            (
                "mods/sodium.jar".to_string(),
                download(
                    "mods/sodium.jar",
                    &known_content,
                    "https://cdn.modrinth.com/data/sodium/versions/v1/file.jar",
                ),
            ),
            (
                "mods/evil.jar".to_string(),
                download(
                    "mods/evil.jar",
                    evil,
                    "https://raw.githubusercontent.com/a/b/main/evil.jar",
                ),
            ),
            ("config/a.toml".to_string(), FileSource::Override),
        ]));

        let plan = adds(&["mods/sodium.jar", "config/a.toml"]);
        assert!(require_known_restored_files(&api, &plan, &sources, None)
            .await
            .is_ok());

        let plan = adds(&["mods/sodium.jar", "mods/evil.jar", "config/a.toml"]);
        let error = require_known_restored_files(&api, &plan, &sources, None)
            .await
            .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::PermissionDenied));
        assert!(error.source.to_string().contains("mods/evil.jar"));

        // a copy in the image store is checked by its own bytes
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(
            dir.path().join("images"),
            lodestone_image::store::Encoding::Zstd,
        ));
        let from_image = |content: &[u8]| {
            let stored = store.put_bytes(content).unwrap();
            Arc::new(BTreeMap::from([(
                "mods/sodium.jar".to_string(),
                FileSource::Image(ImageFile {
                    sha256: stored.sha256,
                    size: stored.size,
                }),
            )]))
        };
        let plan = adds(&["mods/sodium.jar"]);
        let disguised = from_image(evil);
        assert!(
            require_known_restored_files(&api, &plan, &disguised, Some(store.clone()))
                .await
                .is_err()
        );
        let genuine = from_image(&known_content);
        assert!(
            require_known_restored_files(&api, &plan, &genuine, Some(store.clone()))
                .await
                .is_ok()
        );
        // without the store to read it from, it isn't known
        assert!(require_known_restored_files(&api, &plan, &genuine, None)
            .await
            .is_err());
    }

    /// A file that appears where a restore adds one, or a pack file edited where an update
    /// replaces it, while the change stages its files (a Files API upload, say) refuses the
    /// placement: the user's file is never overwritten, and nothing is recorded.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_change_never_overwrites_a_file_written_while_it_stages() {
        use crate::modpack::lineage::tests::{owner, Core};
        let core = Core::new().await;
        let installed = core.current().await;

        // an update: config/pack.toml is replaced, as the pack wrote it
        let update = UpdateSource::Modrinth {
            version_id: "v2".into(),
        };
        let prepared = prepare(&core.state, &owner(), &core.uuid, &update)
            .await
            .unwrap();
        assert!(prepared
            .summary
            .replaced
            .contains(&"config/pack.toml".to_string()));
        std::fs::write(core.root.join("config/pack.toml"), b"EDITED WHILE STAGING").unwrap();
        let error = apply(&core.state, &prepared, &|_| {}).await.unwrap_err();
        remove_work_dirs(&core.root).await;
        assert!(
            matches!(error.kind, ErrorKind::Conflict),
            "{}",
            error.source
        );
        assert_eq!(
            std::fs::read(core.root.join("config/pack.toml")).unwrap(),
            b"EDITED WHILE STAGING"
        );
        assert!(std::fs::read(core.root.join("mods/new.jar")).is_err());
        assert_eq!(core.current().await, installed);

        // a restore: mods/dropped.jar, gone since the update, is added back
        core.change(update).await.unwrap();
        let updated = core.current().await;
        let restore = UpdateSource::History {
            entry_id: installed.clone(),
        };
        let ctx = crate::images::context_of(&core.state, &core.uuid)
            .await
            .unwrap();
        lineage::before_change(core.state.images(), ctx, true)
            .await
            .unwrap();
        let prepared = prepare(&core.state, &owner(), &core.uuid, &restore)
            .await
            .unwrap();
        assert!(prepared
            .summary
            .added
            .contains(&"mods/dropped.jar".to_string()));
        std::fs::write(core.root.join("mods/dropped.jar"), b"USER UPLOAD").unwrap();
        let error = apply(&core.state, &prepared, &|_| {}).await.unwrap_err();
        remove_work_dirs(&core.root).await;
        assert!(
            matches!(error.kind, ErrorKind::Conflict),
            "{}",
            error.source
        );
        assert!(error.source.to_string().contains("mods/dropped.jar"));
        assert_eq!(
            std::fs::read(core.root.join("mods/dropped.jar")).unwrap(),
            b"USER UPLOAD"
        );
        assert_eq!(
            std::fs::read(core.root.join("mods/new.jar")).unwrap(),
            b"new"
        );
        assert_eq!(core.current().await, updated);
    }
}
