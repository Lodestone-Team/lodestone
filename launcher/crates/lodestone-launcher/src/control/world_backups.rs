//! World backups, as Settings › Snapshots and the Worlds tab have them: list, make,
//! restore (in place after a safety world backup, or as a new world) and delete. A world
//! backup is a zip of one world (`launcher_core::world_backups`); making and restoring one
//! are jobs of kind `world_backups`, which `jobs.cancel` stops.

use std::sync::Arc;

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    content::InstanceDirs,
    game::download::{CancelToken, Progress as Downloads},
    instances::Instance,
    world_backups::{RestoreTo, WorldBackup, WorldBackupError, WorldBackupKind, WorldBackups},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{params, tri},
    jobs::{Cancel, JobSpec, Progress},
    ready,
};

fn backup_error(err: WorldBackupError) -> ErrorObject {
    let code = match &err {
        WorldBackupError::NotFound(_) => codes::NOT_FOUND,
        WorldBackupError::Running | WorldBackupError::Suspended => codes::BUSY,
        WorldBackupError::Cancelled => codes::CANCELLED,
        WorldBackupError::Invalid(_) | WorldBackupError::World(_) => codes::INVALID_PARAMS,
        _ => codes::FAILED,
    };
    api_error(code, err.to_string())
}

fn kind_name(kind: WorldBackupKind) -> &'static str {
    match kind {
        WorldBackupKind::Manual => "manual",
        WorldBackupKind::BeforeRestore => "before_restore",
    }
}

fn record_json(backups: &WorldBackups, instance: &str, backup: &WorldBackup) -> Value {
    json!({
        "id": backup.id,
        "name": backup.name,
        "kind": kind_name(backup.kind),
        "restored": backup.restored,
        "world": backup.folder,
        "level_name": backup.level_name,
        "game_version": backup.game_version,
        "created": backup.created,
        "bytes": backup.bytes,
        "world_bytes": backup.world_bytes,
        "files": backup.files,
        "left_out": backup.left_out,
        "left_out_count": backup.left_out_count,
        "path": backups.zip_path(instance, backup),
    })
}

/// The backup `reference` names among `list`: its id, its name in any case, or `latest`
/// (the newest).
fn find(
    list: &[WorldBackup],
    instance: &Instance,
    reference: &str,
) -> Result<WorldBackup, ErrorObject> {
    if reference == "latest" {
        return list.first().cloned().ok_or_else(|| {
            api_error(
                codes::NOT_FOUND,
                format!("{} has no world backups of that world.", instance.name),
            )
        });
    }
    if let Some(found) = list.iter().find(|b| b.id == reference) {
        return Ok(found.clone());
    }
    let named: Vec<&WorldBackup> = list
        .iter()
        .filter(|b| b.name.eq_ignore_ascii_case(reference))
        .collect();
    match named.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("{} has no world backup \"{reference}\".", instance.name),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("Several world backups are named \"{reference}\": use an id."),
        )),
    }
}

/// Runs `work` in the background as a `world_backups` job of `instance`, with the bytes
/// it counts as the job's progress and `jobs.cancel` stopping it; then `after` on the
/// window's thread when it succeeded.
fn job(
    call: &Call,
    label: String,
    instance: &Instance,
    work: impl FnOnce(&Downloads, &CancelToken) -> Result<Value, ErrorObject> + Send + 'static,
    after: impl FnOnce(&Control, &mut App) + 'static,
    cx: &mut App,
) -> Reply {
    let progress = Arc::new(Downloads::default());
    let cancel = CancelToken::new();
    let (report, job_cancel) = (progress.clone(), cancel.clone());
    let background = cx.background_spawn(async move { work(&report, &cancel) });
    let control = call.control.clone();
    let task = cx.spawn(async move |cx| {
        let result = background.await;
        cx.update(|cx| after(&control, cx));
        result
    });
    let spec = JobSpec::new("world_backups", label)
        .instance(instance)
        .progress(Progress::Files(progress))
        .cancel(Cancel::Token(job_cancel));
    let control = call.control.clone();
    let id = control.start_job(spec, task, cx);
    control.reply_job(call, id, cx)
}

/// Shows the change in the window: the lists read again, the Worlds tab too.
fn refresh(control: &Control, instance: &str, cx: &mut App) {
    control.instances.update(cx, |instances, cx| {
        instances.load_world_backups(instance, cx);
        cx.notify();
    });
    super::worlds::refresh_view(control, instance, cx);
}

/// The world folder `reference` names for the instance's world backups `list`: a folder
/// they are kept under, a world on disk ([`super::worlds::find_world`]), or, for a world no
/// longer in the instance (deleted or renamed outside the Launcher), a folder or world
/// name of its backups in any case. Blocking: it reads the worlds' `level.dat`s.
fn world_folder(
    list: &[WorldBackup],
    dirs: &InstanceDirs,
    reference: &str,
) -> Result<String, ErrorObject> {
    if list.iter().any(|b| b.folder == reference) {
        return Ok(reference.to_string());
    }
    let on_disk = super::worlds::find_world(dirs, reference);
    if let Ok(world) = &on_disk {
        return Ok(world.folder.clone());
    }
    let mut folders: Vec<&str> = list
        .iter()
        .filter(|b| {
            b.folder.eq_ignore_ascii_case(reference)
                || b.level_name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(reference))
        })
        .map(|b| b.folder.as_str())
        .collect();
    folders.sort_unstable();
    folders.dedup();
    match folders.as_slice() {
        [one] => Ok(one.to_string()),
        [] => Err(on_disk.err().unwrap_or_else(|| {
            api_error(
                codes::NOT_FOUND,
                format!("No world is called \"{reference}\"."),
            )
        })),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!(
                "World backups of more than one world are called \"{reference}\"; use its folder."
            ),
        )
        .with_data(json!({ "matches": folders }))),
    }
}

/// The instance's world backups, only the world `world`'s when given. Blocking.
fn backups_of(
    backups: &WorldBackups,
    dirs: &InstanceDirs,
    instance: &Instance,
    world: Option<&str>,
) -> Result<Vec<WorldBackup>, ErrorObject> {
    let list = backups.list(&instance.id).map_err(backup_error)?;
    Ok(match world {
        Some(reference) => {
            let folder = world_folder(&list, dirs, reference)?;
            list.into_iter().filter(|b| b.folder == folder).collect()
        }
        None => list,
    })
}

/// Finds the backup `reference` (with `world`, among that world's; `latest` needs it) in
/// the background, then runs `then` with it on the window's thread.
fn with_backup(
    call: Call,
    world: Option<String>,
    reference: String,
    then: impl FnOnce(Call, Instance, WorldBackup, &mut App) -> Reply + 'static,
    cx: &mut App,
) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    if world.is_none() && reference == "latest" {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "latest needs the world (world: its folder or name).",
        )));
    }
    let backups = call.control.instances.read(cx).world_backups();
    let dirs = call.control.instances.read(cx).content_dirs(&instance.id);
    let found = cx.background_spawn({
        let instance = instance.clone();
        async move {
            let list = backups_of(&backups, &dirs, &instance, world.as_deref())?;
            find(&list, &instance, &reference)
        }
    });
    cx.spawn(async move |cx| {
        let backup = found.await?;
        cx.update(|cx| then(call, instance, backup, cx)).await
    })
}

#[derive(Deserialize)]
struct ListParams {
    #[serde(default)]
    deleted: bool,
    /// Only this world's: its folder or name.
    #[serde(default)]
    world: Option<String>,
}

/// `world_backups.list`: an instance's world backups, newest first (one world's, given
/// `world`); or, with `deleted`, every deleted instance's.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let params: ListParams = params!(call);
    let backups = call.control.instances.read(cx).world_backups();
    if params.deleted {
        return cx.background_spawn(async move {
            let deleted = backups.list_deleted();
            Ok(json!({
                "deleted": deleted.iter().map(|d| json!({
                    "instance": d.instance_id,
                    "name": d.instance_name,
                    "backups": d.backups.iter().map(|b| record_json(&backups, &d.instance_id, b)).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }))
        });
    }
    let instance = tri!(call.control.instance_param(&call, cx));
    let dirs = call.control.instances.read(cx).content_dirs(&instance.id);
    cx.background_spawn(async move {
        let list = backups_of(&backups, &dirs, &instance, params.world.as_deref())?;
        Ok(json!({
            "instance": instance.id,
            "world_backups": list
                .iter()
                .map(|b| record_json(&backups, &instance.id, b))
                .collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct CreateParams {
    #[serde(default)]
    name: Option<String>,
}

/// `world_backups.create`: zips a world. The world is found in the background (it reads
/// every world's `level.dat`), then the job starts on the window's thread.
pub fn create(call: Call, cx: &mut App) -> Reply {
    let params: CreateParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    let reference = tri!(
        call.params
            .get("world")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| api_error(
                codes::INVALID_PARAMS,
                "Name the world: its folder or its name.",
            ))
    );
    let dirs = call.control.instances.read(cx).content_dirs(&instance.id);
    let found = cx.background_spawn(async move { super::worlds::find_world(&dirs, &reference) });
    cx.spawn(async move |cx| {
        let world = found.await?;
        cx.update(|cx| create_found(call, instance, world, params, cx))
            .await
    })
}

/// [`create`] once the world is found.
fn create_found(
    call: Call,
    instance: Instance,
    world: launcher_core::worlds::World,
    params: CreateParams,
    cx: &mut App,
) -> Reply {
    tri!(super::snapshots::idle(&call.control, &instance, cx));
    let instances = call.control.instances.read(cx);
    tri!(files_idle(instances, &instance));
    let backups = instances.world_backups();
    // Play waits until it is done.
    let busy = instances.mark_files_busy(&instance.id);
    let label = format!("Backing up {}", world.name());
    let (id, folder) = (instance.id.clone(), world.folder.clone());
    let refreshed = id.clone();
    job(
        &call,
        label,
        &instance,
        move |progress, cancel| {
            let _busy = busy;
            let made = backups
                .create(&id, &folder, params.name.as_deref(), progress, &|| {
                    cancel.is_cancelled()
                })
                .map_err(backup_error)?;
            Ok(record_json(&backups, &id, &made))
        },
        move |control, cx| refresh(control, &refreshed, cx),
        cx,
    )
}

/// Refuses while another job writes the instance's files (an install, a modpack update, a
/// job of the window): a backup could zip a file mid-write and a restore's swap would lose
/// what it writes, and either would wait for the instance's lock meanwhile, with
/// `jobs.cancel` doing nothing until it got it.
fn files_idle(
    instances: &crate::instances::Instances,
    instance: &Instance,
) -> Result<(), ErrorObject> {
    if instances.files_or_pack_busy(&instance.id) {
        return Err(api_error(
            codes::BUSY,
            "Another job is changing the instance's files; try again when it is done.",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RestoreParams {
    /// The backup: its id, its name, or `latest` (with `world`: the newest of that world).
    backup: String,
    /// Only this world's backups are looked at: its folder or name.
    #[serde(default)]
    world: Option<String>,
    /// `new` (a new world beside it, the default) or `replace` (in place, after a safety
    /// world backup), as `worlds.restore_snapshot` has them.
    #[serde(default = "new_world")]
    mode: String,
}

fn new_world() -> String {
    "new".into()
}

/// `world_backups.restore`: puts a world backup back in place (with confirm) or as a new
/// world.
pub fn restore(call: Call, cx: &mut App) -> Reply {
    let params: RestoreParams = params!(call);
    let to = match params.mode.as_str() {
        "replace" => RestoreTo::InPlace,
        "new" => RestoreTo::NewWorld,
        other => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No restore mode \"{other}\": new or replace."),
            )));
        }
    };
    with_backup(
        call,
        params.world,
        params.backup,
        move |call, instance, backup, cx| restore_found(call, instance, backup, to, cx),
        cx,
    )
}

/// [`restore`] once the backup is found.
fn restore_found(
    call: Call,
    instance: Instance,
    backup: WorldBackup,
    to: RestoreTo,
    cx: &mut App,
) -> Reply {
    if to == RestoreTo::InPlace {
        tri!(call.confirm(format!(
            "puts the world \"{}\" back as it was in the world backup \"{}\" (the world as it is now is saved first as a world backup)",
            backup.world_name(),
            backup.name
        )));
    }
    tri!(super::snapshots::idle(&call.control, &instance, cx));
    let instances = call.control.instances.read(cx);
    tri!(files_idle(instances, &instance));
    let backups = instances.world_backups();
    let busy = instances.mark_files_busy(&instance.id);
    let label = format!("Restoring {} from {}", backup.world_name(), backup.name);
    let id = instance.id.clone();
    let instance_id = instance.id.clone();
    job(
        &call,
        label,
        &instance,
        move |progress, cancel| {
            let _busy = busy;
            let restored = backups
                .restore(&instance_id, &backup.id, to, &|_| {}, progress, &|| {
                    cancel.is_cancelled()
                })
                .map_err(backup_error)?;
            Ok(json!({
                "world": restored.folder,
                "from": backup.id,
                "safety_world_backup": restored.safety.as_ref().map(|b| record_json(&backups, &instance_id, b)),
                "kept_aside": restored.kept_aside,
            }))
        },
        move |control, cx| refresh(control, &id, cx),
        cx,
    )
}

#[derive(Deserialize)]
struct DeleteParams {
    /// The backup: its id, its name, or `latest` (with `world`: the newest of that world).
    backup: String,
    /// Only this world's backups are looked at: its folder or name.
    #[serde(default)]
    world: Option<String>,
}

/// `world_backups.delete`: moves a world backup's zip to the Trash.
pub fn delete(call: Call, cx: &mut App) -> Reply {
    let params: DeleteParams = params!(call);
    with_backup(call, params.world, params.backup, delete_found, cx)
}

/// [`delete`] once the backup is found.
fn delete_found(call: Call, instance: Instance, backup: WorldBackup, cx: &mut App) -> Reply {
    tri!(call.confirm(format!(
        "moves the world backup \"{}\" of \"{}\" to the Trash",
        backup.name,
        backup.world_name()
    )));
    tri!(super::snapshots::idle(&call.control, &instance, cx));
    let backups = call.control.instances.read(cx).world_backups();
    let control = call.control.clone();
    let id = instance.id.clone();
    let work =
        cx.background_spawn(async move { backups.delete(&id, &backup.id).map(|()| backup.id) });
    cx.spawn(async move |cx| {
        let deleted = work.await.map_err(backup_error)?;
        cx.update(|cx| refresh(&control, &instance.id, cx));
        Ok(json!({ "deleted": deleted, "trash": true }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::world_backups::WorldBackupKind;

    fn backup(id: &str, folder: &str, level_name: Option<&str>) -> WorldBackup {
        WorldBackup {
            version: 1,
            id: id.into(),
            instance_created: 1,
            instance_name: "Survival".into(),
            folder: folder.into(),
            name: "World backup".into(),
            kind: WorldBackupKind::Manual,
            restored: None,
            created: 0,
            level_name: level_name.map(str::to_string),
            game_version: None,
            data_version: None,
            bytes: 0,
            world_bytes: 0,
            files: 0,
            left_out: Vec::new(),
            left_out_count: 0,
        }
    }

    /// A world deleted (or renamed outside the Launcher) is still found by its folder or
    /// name among its backups, so `latest` and `--world` work for the world one most
    /// wants back.
    #[test]
    fn a_world_no_longer_in_the_instance_is_found_among_its_backups() {
        // An instance with no worlds on disk: they were all deleted.
        let dirs = InstanceDirs::new(std::env::temp_dir().join("lodestone-no-such-instance"));
        let list = [
            backup("20261115-120000-0001", "New World", Some("Hardcore run")),
            backup("20261115-120000-0002", "Other", Some("Islands")),
            backup("20261115-120000-0003", "Twin", Some("Same")),
            backup("20261115-120000-0004", "Twin 2", Some("Same")),
        ];
        for (reference, folder) in [
            ("New World", "New World"),
            ("new world", "New World"),
            ("Hardcore run", "New World"),
            ("HARDCORE RUN", "New World"),
            ("Islands", "Other"),
        ] {
            assert_eq!(
                world_folder(&list, &dirs, reference).unwrap(),
                folder,
                "{reference}"
            );
        }
        assert_eq!(
            world_folder(&list, &dirs, "Nothing").unwrap_err().code,
            codes::NOT_FOUND
        );
        assert_eq!(
            world_folder(&list, &dirs, "Same").unwrap_err().code,
            codes::AMBIGUOUS
        );
    }
}
