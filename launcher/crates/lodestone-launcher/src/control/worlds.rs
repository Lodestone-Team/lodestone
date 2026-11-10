//! The Worlds tab: worlds, their backups and their data packs. Each change is the tab's
//! `launcher_core::worlds` or `launcher_core::datapacks` call, with the tab's checks
//! (nothing while the game runs, one job at a time), and the tab shows it when it shows
//! that instance.

use std::{sync::Arc, time::UNIX_EPOCH};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    content::InstanceDirs,
    datapacks::{self, PackState},
    game::download::{CancelToken, Progress as Downloads},
    instances::Instance,
    worlds::{self, Backup, RestoreMode, World, WorldError},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{params, tri},
    jobs::{Cancel, JobSpec, Progress},
    ready,
};

fn unix(time: Option<std::time::SystemTime>) -> Option<u64> {
    time.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

fn world_json(world: &World, dirs: &InstanceDirs) -> Value {
    let summary = world.summary.as_ref();
    json!({
        "folder": world.folder,
        "name": world.name(),
        "last_played": unix(world.last_played()),
        "game_mode": summary.and_then(|s| s.game_mode).map(|m| format!("{m:?}").to_lowercase()),
        "hardcore": summary.map(|s| s.hardcore),
        "version": summary.and_then(|s| s.version_name.clone()),
        "size": world.size,
        "problem": world.problem,
        "backups": worlds::backups(dirs, &world.folder).len(),
    })
}

fn backup_json(backup: &Backup) -> Value {
    json!({
        "file": backup.file_name,
        "path": backup.path,
        "size": backup.size,
        "created": unix(backup.created),
    })
}

fn world_error(err: WorldError) -> ErrorObject {
    let code = match &err {
        WorldError::NotFound(_) => codes::NOT_FOUND,
        WorldError::Conflict(_) => codes::CONFLICT,
        WorldError::Invalid(_) => codes::INVALID_PARAMS,
        WorldError::Cancelled => codes::CANCELLED,
        _ => codes::FAILED,
    };
    api_error(code, err.to_string())
}

/// The world named by `reference`: its folder, or its name in any case.
fn find_world(dirs: &InstanceDirs, reference: &str) -> Result<World, ErrorObject> {
    let list = worlds::list(dirs);
    if let Some(world) = list.iter().find(|w| w.folder == reference) {
        return Ok(world.clone());
    }
    let matches: Vec<&World> = list
        .iter()
        .filter(|w| {
            w.name().eq_ignore_ascii_case(reference) || w.folder.eq_ignore_ascii_case(reference)
        })
        .collect();
    match matches.as_slice() {
        [world] => Ok((*world).clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("No world is called \"{reference}\". List them with `worlds list`."),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("More than one world is called \"{reference}\"; use its folder."),
        )
        .with_data(json!({ "matches": matches.iter().map(|w| &w.folder).collect::<Vec<_>>() }))),
    }
}

/// The backup named by `reference` (its file name, or `latest`) of `world`.
fn find_backup(dirs: &InstanceDirs, world: &World, reference: &str) -> Result<Backup, ErrorObject> {
    let list = worlds::backups(dirs, &world.folder);
    let found = if reference == "latest" {
        list.into_iter().next()
    } else {
        list.into_iter().find(|b| b.file_name == reference)
    };
    found.ok_or_else(|| {
        api_error(
            codes::NOT_FOUND,
            format!(
                "{} has no backup \"{reference}\". List them with `worlds backups`.",
                world.name()
            ),
        )
    })
}

struct Target {
    control: std::rc::Rc<Control>,
    instance: Instance,
    dirs: InstanceDirs,
    world: World,
}

fn target(call: &Call, cx: &App) -> Result<Target, ErrorObject> {
    let control = call.control.clone();
    let instance = control.instance_param(call, cx)?;
    let dirs = control.instances.read(cx).content_dirs(&instance.id);
    let reference = call
        .params
        .get("world")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                "Name the world: its folder or its name.",
            )
        })?;
    let world = find_world(&dirs, reference)?;
    Ok(Target {
        control,
        instance,
        dirs,
        world,
    })
}

/// The Worlds tab's lock.
fn can_change(target: &Target, cx: &App) -> Result<(), ErrorObject> {
    target.control.idle(&target.instance, cx)?;
    if let Some(job) = target
        .control
        .jobs
        .borrow()
        .running_for("worlds", &target.instance.id)
    {
        return Err(api_error(
            codes::BUSY,
            format!(
                "Job {job} is working on {}'s worlds; wait for it.",
                target.instance.name
            ),
        ));
    }
    Ok(())
}

/// Shows the change in the window, if the instance is the one shown.
fn refresh_view(control: &Control, instance: &str, cx: &mut App) {
    let selected = control
        .instances
        .read(cx)
        .selected()
        .is_some_and(|s| s.id == instance);
    if let (true, Some(view)) = (selected, control.worlds.clone()) {
        view.update(cx, |view, cx| view.reload(cx));
    }
}

pub fn list(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let dirs = call.control.instances.read(cx).content_dirs(&instance.id);
    cx.background_spawn(async move {
        let list = worlds::list(&dirs);
        Ok(json!({
            "instance": instance.id,
            "saves": worlds::saves_dir(&dirs),
            "worlds": list.iter().map(|w| world_json(w, &dirs)).collect::<Vec<_>>(),
        }))
    })
}

pub fn backups(call: Call, cx: &mut App) -> Reply {
    let target = tri!(target(&call, cx));
    let list = worlds::backups(&target.dirs, &target.world.folder);
    ready(Ok(json!({
        "instance": target.instance.id,
        "world": target.world.folder,
        "backups": list.iter().map(backup_json).collect::<Vec<_>>(),
    })))
}

/// Runs `work` in the background as a worlds job of the instance.
fn run(
    call: &Call,
    target: Target,
    kind_label: String,
    work: impl FnOnce(&InstanceDirs, &Downloads, &CancelToken) -> Result<Value, WorldError>
    + Send
    + 'static,
    cx: &mut App,
) -> Reply {
    run_async(
        call,
        target,
        kind_label,
        move |dirs, progress, cancel| async move { work(&dirs, &progress, &cancel) },
        cx,
    )
}

/// Like [`run`], for work that waits on the network.
fn run_async<F>(
    call: &Call,
    target: Target,
    kind_label: String,
    work: impl FnOnce(InstanceDirs, Arc<Downloads>, CancelToken) -> F,
    cx: &mut App,
) -> Reply
where
    F: std::future::Future<Output = Result<Value, WorldError>> + Send + 'static,
{
    tri!(can_change(&target, cx));
    let progress = Arc::new(Downloads::default());
    let cancel = CancelToken::new();
    let dirs = target.dirs.clone();
    let (job_progress, job_cancel) = (progress.clone(), cancel.clone());
    let background = cx.background_spawn(work(dirs, progress, cancel));
    let control = target.control.clone();
    let id = target.instance.id.clone();
    let task = cx.spawn(async move |cx| {
        let result = background.await.map_err(world_error);
        cx.update(|cx| refresh_view(&control, &id, cx));
        result
    });
    let control = target.control.clone();
    let job = control.start_job(
        JobSpec::new("worlds", kind_label)
            .instance(&target.instance)
            .progress(Progress::Files(job_progress))
            .cancel(Cancel::Token(job_cancel)),
        task,
        cx,
    );
    control.reply_job(call, job, cx)
}

pub fn backup(call: Call, cx: &mut App) -> Reply {
    let target = tri!(target(&call, cx));
    let folder = target.world.folder.clone();
    let label = format!("Backing up {}", target.world.name());
    run(
        &call,
        target,
        label,
        move |dirs, progress, cancel| {
            let backup = worlds::backup(dirs, &folder, progress, cancel)?;
            Ok(json!({ "world": folder, "backup": backup_json(&backup) }))
        },
        cx,
    )
}

#[derive(Deserialize)]
struct RestoreParams {
    /// The backup's file name, or `latest`.
    #[serde(default = "latest")]
    backup: String,
    /// `copy` (a new world beside it) or `replace` (the world goes to the Trash, after
    /// a safety backup).
    #[serde(default = "copy")]
    mode: String,
}

fn latest() -> String {
    "latest".into()
}

fn copy() -> String {
    "copy".into()
}

pub fn restore(call: Call, cx: &mut App) -> Reply {
    let params: RestoreParams = params!(call);
    let target = tri!(target(&call, cx));
    let backup = tri!(find_backup(&target.dirs, &target.world, &params.backup));
    let mode = match params.mode.as_str() {
        "copy" => RestoreMode::Copy,
        "replace" => {
            tri!(call.confirm(format!(
                "replaces the world \"{}\" with the backup {} (the world is backed up first, then moved to the Trash)",
                target.world.name(),
                backup.file_name
            )));
            RestoreMode::Replace
        }
        other => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No restore mode \"{other}\": copy or replace."),
            )));
        }
    };
    let folder = target.world.folder.clone();
    let label = format!(
        "Restoring {} from {}",
        target.world.name(),
        backup.file_name
    );
    run(
        &call,
        target,
        label,
        move |dirs, progress, cancel| {
            let restored = match mode {
                // a copy removes nothing
                RestoreMode::Copy => worlds::restore_with(
                    dirs,
                    &folder,
                    &backup.path,
                    mode,
                    |_| Ok(()),
                    progress,
                    cancel,
                )?,
                RestoreMode::Replace => {
                    worlds::restore(dirs, &folder, &backup.path, mode, progress, cancel)?
                }
            };
            Ok(json!({
                "world": restored.folder,
                "from": backup.file_name,
                "safety_backup": restored.safety_backup.as_ref().map(backup_json),
            }))
        },
        cx,
    )
}

pub fn delete(call: Call, cx: &mut App) -> Reply {
    let target = tri!(target(&call, cx));
    tri!(call.confirm(format!(
        "moves the world \"{}\" to the Trash",
        target.world.name()
    )));
    tri!(can_change(&target, cx));
    tri!(worlds::delete(&target.dirs, &target.world.folder).map_err(world_error));
    refresh_view(&target.control, &target.instance.id, cx);
    ready(Ok(json!({ "deleted": target.world.folder, "trash": true })))
}

#[derive(Deserialize)]
struct DeleteBackupParams {
    backup: String,
}

// ---- data packs ----

fn datapack_json(pack: &datapacks::DataPack, game_version: &str) -> Value {
    let compatibility = datapacks::compatibility(pack.meta.formats.as_ref(), game_version);
    json!({
        "name": pack.name,
        "folder": pack.is_dir,
        "size": pack.size,
        "state": match pack.state {
            PackState::Enabled => "enabled",
            PackState::Disabled => "disabled",
            PackState::New => "new",
        },
        "description": pack.meta.description.as_deref().map(launcher_core::mc_text::strip),
        // A bare major number as the maximum means any minor of it.
        "formats": pack.meta.formats.map(|f| json!({
            "min": f.min.to_string(),
            "max": if f.max.minor == u32::MAX { f.max.major.to_string() } else { f.max.to_string() },
        })),
        "warning": compatibility.warning(),
        "problem": pack.meta.problem,
        "modrinth_project": pack.modrinth_project,
    })
}

fn game_version(target: &Target) -> String {
    target.instance.game_version.clone()
}

/// A world's data packs, with whether each is on.
pub fn datapacks(call: Call, cx: &mut App) -> Reply {
    let target = tri!(target(&call, cx));
    let version = game_version(&target);
    cx.background_spawn(async move {
        let listed = datapacks::list(&target.dirs, &target.world.folder).map_err(world_error)?;
        Ok(json!({
            "instance": target.instance.id,
            "world": target.world.folder,
            "folder": datapacks::dir(&target.dirs, &target.world.folder).map_err(world_error)?,
            "game_version": version,
            "switchable": listed.switchable,
            "datapacks": listed.packs.iter().map(|p| datapack_json(p, &version)).collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct PackParams {
    pack: String,
}

/// The pack named by `reference`: its file name, or the name without `.zip`, in any case.
fn find_pack(target: &Target, reference: &str) -> Result<String, ErrorObject> {
    let listed = datapacks::list(&target.dirs, &target.world.folder).map_err(world_error)?;
    if let Some(pack) = listed.packs.iter().find(|p| p.name == reference) {
        return Ok(pack.name.clone());
    }
    let stem = |name: &str| {
        let lower = name.to_lowercase();
        lower
            .strip_suffix(".zip")
            .map(str::to_string)
            .unwrap_or(lower)
    };
    let matches: Vec<&str> = listed
        .packs
        .iter()
        .filter(|p| stem(&p.name) == stem(reference))
        .map(|p| p.name.as_str())
        .collect();
    match matches.as_slice() {
        [name] => Ok(name.to_string()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!(
                "{} has no data pack \"{reference}\". List them with `worlds datapacks`.",
                target.world.name()
            ),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("More than one data pack is called \"{reference}\"; use its file name."),
        )
        .with_data(json!({ "matches": matches }))),
    }
}

fn set_pack(call: Call, enabled: bool, cx: &mut App) -> Reply {
    let params: PackParams = params!(call);
    let target = tri!(target(&call, cx));
    let pack = tri!(find_pack(&target, &params.pack));
    tri!(can_change(&target, cx));
    tri!(
        datapacks::set_enabled(&target.dirs, &target.world.folder, &pack, enabled)
            .map_err(world_error)
    );
    refresh_view(&target.control, &target.instance.id, cx);
    ready(Ok(json!({
        "world": target.world.folder,
        "pack": pack,
        "state": if enabled { "enabled" } else { "disabled" },
        "applies": "the next time the world loads",
    })))
}

pub fn datapack_enable(call: Call, cx: &mut App) -> Reply {
    set_pack(call, true, cx)
}

pub fn datapack_disable(call: Call, cx: &mut App) -> Reply {
    set_pack(call, false, cx)
}

#[derive(Deserialize)]
struct AddParams {
    /// An absolute path to a .zip or a folder.
    path: std::path::PathBuf,
}

pub fn datapack_add(call: Call, cx: &mut App) -> Reply {
    let params: AddParams = params!(call);
    if !params.path.is_absolute() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give the data pack's absolute path.",
        )));
    }
    let target = tri!(target(&call, cx));
    tri!(can_change(&target, cx));
    let name =
        tri!(datapacks::add(&target.dirs, &target.world.folder, &params.path).map_err(world_error));
    refresh_view(&target.control, &target.instance.id, cx);
    ready(Ok(json!({ "world": target.world.folder, "added": name })))
}

pub fn datapack_remove(call: Call, cx: &mut App) -> Reply {
    let params: PackParams = params!(call);
    let target = tri!(target(&call, cx));
    let pack = tri!(find_pack(&target, &params.pack));
    tri!(call.confirm(format!(
        "moves the data pack {pack} of \"{}\" to the Trash",
        target.world.name()
    )));
    tri!(can_change(&target, cx));
    tri!(datapacks::delete(&target.dirs, &target.world.folder, &pack).map_err(world_error));
    refresh_view(&target.control, &target.instance.id, cx);
    ready(Ok(json!({ "deleted": pack, "trash": true })))
}

fn modrinth(
    control: &Control,
    cx: &App,
) -> Result<Arc<launcher_core::content::Modrinth>, ErrorObject> {
    control.instances.read(cx).modrinth().ok_or_else(|| {
        api_error(
            codes::UNAVAILABLE,
            "Modrinth isn't available (the launcher is offline).",
        )
    })
}

#[derive(Deserialize)]
struct SearchParams {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

/// Data packs on Modrinth for the instance's Minecraft version.
pub fn datapack_search(call: Call, cx: &mut App) -> Reply {
    let params: SearchParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let api = tri!(modrinth(&control, cx));
    let search = tri!(
        lodestone_minecraft::modrinth::ClientSearchParams::new(
            params.query.filter(|q| !q.trim().is_empty()),
            lodestone_minecraft::modrinth::ClientProjectType::DataPack,
            Some(instance.game_version.clone()),
            Vec::new(),
            None,
            Some(0),
            params.limit.or(Some(10)),
        )
        .map_err(|err| api_error(codes::INVALID_PARAMS, err.to_string()))
    );
    cx.background_spawn(async move {
        let results = api
            .search(&search)
            .await
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        Ok(json!({
            "game_version": instance.game_version,
            "total": results.total_hits,
            "hits": results.hits.iter().map(|h| json!({
                "project": h.project_id,
                "slug": h.slug,
                "title": h.title,
                "author": h.author,
                "description": h.description,
                "downloads": h.downloads,
            })).collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct InstallParams {
    /// A Modrinth project id or slug.
    project: String,
    /// A version id or number; the newest that fits by default.
    #[serde(default)]
    version: Option<String>,
}

/// Installs a Modrinth data pack into a world.
pub fn datapack_install(call: Call, cx: &mut App) -> Reply {
    let params: InstallParams = params!(call);
    let target = tri!(target(&call, cx));
    let api = tri!(modrinth(&target.control, cx));
    let downloader = target.control.instances.read(cx).content_downloader();
    let version = game_version(&target);
    let folder = target.world.folder.clone();
    let label = format!("Installing {} in {}", params.project, target.world.name());
    run_async(
        &call,
        target,
        label,
        move |dirs, progress, cancel| async move {
            let invalid =
                |err: launcher_core::content::ModrinthError| WorldError::Invalid(err.to_string());
            let picked = match &params.version {
                Some(v) => {
                    let picked = api
                        .project_version(&params.project, v)
                        .await
                        .map_err(invalid)?;
                    if let Some(problem) = datapacks::incompatibility(&picked, &version) {
                        return Err(WorldError::Invalid(problem));
                    }
                    picked
                }
                None => {
                    let versions = api
                        .versions(
                            &params.project,
                            &[datapacks::MODRINTH_LOADER.to_string()],
                            std::slice::from_ref(&version),
                        )
                        .await
                        .map_err(invalid)?;
                    datapacks::pick_version(&version, versions).ok_or_else(|| {
                        WorldError::Invalid(format!(
                            "{} has no data pack for Minecraft {version}.",
                            params.project
                        ))
                    })?
                }
            };
            let installed = datapacks::install(
                &api,
                &downloader,
                &dirs,
                &folder,
                &picked,
                &progress,
                &cancel,
            )
            .await?;
            Ok(json!({
                "world": folder,
                "installed": installed.name,
                "version": picked.version_number,
                "replaced": installed.replaced,
            }))
        },
        cx,
    )
}

pub fn delete_backup(call: Call, cx: &mut App) -> Reply {
    let params: DeleteBackupParams = params!(call);
    let target = tri!(target(&call, cx));
    let backup = tri!(find_backup(&target.dirs, &target.world, &params.backup));
    tri!(call.confirm(format!(
        "moves the backup {} of \"{}\" to the Trash",
        backup.file_name,
        target.world.name()
    )));
    tri!(can_change(&target, cx));
    tri!(worlds::delete_backup(&target.dirs, &backup.path).map_err(world_error));
    refresh_view(&target.control, &target.instance.id, cx);
    ready(Ok(json!({ "deleted": backup.file_name, "trash": true })))
}
