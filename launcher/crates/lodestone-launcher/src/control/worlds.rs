//! The Worlds tab: worlds and their backups. Each change is the tab's
//! `launcher_core::worlds` call, with the tab's checks (nothing while the game runs, one
//! job at a time), and the tab shows it when it shows that instance.

use std::{sync::Arc, time::UNIX_EPOCH};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    content::InstanceDirs,
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
    tri!(can_change(&target, cx));
    let progress = Arc::new(Downloads::default());
    let cancel = CancelToken::new();
    let dirs = target.dirs.clone();
    let (job_progress, job_cancel) = (progress.clone(), cancel.clone());
    let background = cx.background_spawn(async move { work(&dirs, &progress, &cancel) });
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
