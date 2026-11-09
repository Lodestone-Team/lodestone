//! Snapshots, as an instance's Settings tab has them: list, take, go back to, write to a
//! `.lodesnap` file, delete, and make a new instance from a file.

use std::{path::PathBuf, sync::Arc};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    game::download::{Progress as Downloads, ProgressSnapshot},
    instances::Instance,
    snapshots::{self, Category, Selection, Snapshot, SnapshotError, Store},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{params, tri},
    jobs::{JobSpec, Progress},
    ready,
};
use crate::instances::snapshots::display_name;

fn snapshot_error(err: SnapshotError) -> ErrorObject {
    match err {
        SnapshotError::NotFound(id) => {
            api_error(codes::NOT_FOUND, format!("No snapshot \"{id}\"."))
        }
        SnapshotError::Invalid(_) | SnapshotError::TooNew => {
            api_error(codes::INVALID_PARAMS, err.to_string())
        }
        other => api_error(codes::FAILED, other.to_string()),
    }
}

fn snapshot_json(snapshot: &Snapshot) -> Value {
    json!({
        "id": snapshot.id,
        "name": display_name(&snapshot.name, snapshot.note.as_deref(), snapshot.automatic),
        "created": snapshot.created,
        "automatic": snapshot.automatic,
        "launcher_version": snapshot.launcher_version,
        "minecraft": snapshot.instance.game_version,
        "total_bytes": snapshot.total_bytes,
        "new_bytes": snapshot.new_bytes,
        "files": snapshot.files.len(),
        "parts": snapshot.summary(),
        "settings": snapshot.selection.categories.contains(&Category::Settings),
    })
}

/// The parts to take or put back: `include` (category ids) and `worlds` (folder names;
/// none: all). Without `include`, `default`.
fn selection(params: &Value, default: Selection) -> Result<Selection, ErrorObject> {
    let worlds: Vec<String> = params
        .get("worlds")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let Some(include) = params.get("include").and_then(Value::as_array) else {
        return Ok(Selection {
            worlds: worlds.into_iter().collect(),
            ..default
        });
    };
    let mut categories = std::collections::BTreeSet::new();
    for id in include {
        let id = id.as_str().unwrap_or_default();
        let category = Category::from_id(id).ok_or_else(|| {
            let ids: Vec<&str> = Category::ALL.iter().map(|c| c.id()).collect();
            api_error(
                codes::INVALID_PARAMS,
                format!("No part \"{id}\": use {}.", ids.join(", ")),
            )
        })?;
        categories.insert(category);
    }
    if !worlds.is_empty() {
        categories.insert(Category::Worlds);
    }
    if categories.is_empty() {
        return Err(api_error(
            codes::INVALID_PARAMS,
            "Include at least one part.",
        ));
    }
    Ok(Selection {
        categories,
        worlds: worlds.into_iter().collect(),
    })
}

/// What "Take snapshot…" chooses at first: everything but screenshots and other files.
fn default_take() -> Selection {
    let mut selection = Selection::all();
    selection.categories.remove(&Category::Screenshots);
    selection.categories.remove(&Category::Other);
    selection
}

fn store(control: &Control, cx: &App) -> Store {
    control.instances.read(cx).snapshot_store()
}

/// The snapshot `params.snapshot` names: its id, its name, or `latest`.
fn find(store: &Store, instance: &Instance, reference: &str) -> Result<Snapshot, ErrorObject> {
    let list = store.list(&instance.id);
    if reference == "latest" {
        return list.into_iter().next().ok_or_else(|| {
            api_error(
                codes::NOT_FOUND,
                format!("{} has no snapshots.", instance.name),
            )
        });
    }
    if let Some(found) = list.iter().find(|s| s.id == reference) {
        return Ok(found.clone());
    }
    let named: Vec<&Snapshot> = list
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case(reference))
        .collect();
    match named.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("{} has no snapshot \"{reference}\".", instance.name),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("Several snapshots are named \"{reference}\": use an id."),
        )),
    }
}

fn snapshot_param(call: &Call) -> Result<String, ErrorObject> {
    call.params
        .get("snapshot")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                "Name the snapshot: its id, its name, or latest.",
            )
        })
}

fn path_param(call: &Call) -> Result<PathBuf, ErrorObject> {
    let path = call
        .params
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| api_error(codes::INVALID_PARAMS, "Give the file's path."))?;
    if !path.is_absolute() {
        return Err(api_error(
            codes::INVALID_PARAMS,
            "The path must be absolute.",
        ));
    }
    Ok(path)
}

/// Reads the instance's snapshots again in the window.
fn refresh(control: &Control, instance: &str, cx: &mut App) {
    let id = instance.to_string();
    control
        .instances
        .update(cx, |instances, cx| instances.load_snapshots(&id, cx));
}

/// Runs `work` as a snapshots job, with byte progress.
fn job(
    call: &Call,
    label: String,
    instance: Option<&Instance>,
    work: impl FnOnce(&(dyn Fn(u64, u64) + Sync)) -> Result<Value, ErrorObject> + Send + 'static,
    after: impl FnOnce(&Control, &Value, &mut App) + 'static,
    cx: &mut App,
) -> Reply {
    let progress = Arc::new(Downloads::default());
    let report = progress.clone();
    let background = cx.background_spawn(async move {
        work(&move |done, total| {
            report.set(ProgressSnapshot {
                bytes_done: done,
                bytes_total: total,
                ..ProgressSnapshot::default()
            })
        })
    });
    let control = call.control.clone();
    let task = cx.spawn(async move |cx| {
        let result = background.await;
        if let Ok(value) = &result {
            cx.update(|cx| after(&control, value, cx));
        }
        result
    });
    let mut spec = JobSpec::new("snapshots", label).progress(Progress::Files(progress));
    if let Some(instance) = instance {
        spec = spec.instance(instance);
    }
    let control = call.control.clone();
    let id = control.start_job(spec, task, cx);
    control.reply_job(call, id, cx)
}

/// One snapshot job per instance at a time.
fn idle(control: &Control, instance: &Instance, cx: &App) -> Result<(), ErrorObject> {
    control.idle(instance, cx)?;
    if let Some(job) = control.jobs.borrow().running_for("snapshots", &instance.id) {
        return Err(api_error(
            codes::BUSY,
            format!(
                "Job {job} is working on {}'s snapshots; wait for it.",
                instance.name
            ),
        ));
    }
    Ok(())
}

/// `snapshots.list`: an instance's snapshots, newest first.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let store = store(&call.control, cx);
    cx.background_spawn(async move {
        let list = store.list(&instance.id);
        Ok(json!({
            "instance": instance.id,
            "snapshots": list.iter().map(snapshot_json).collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct CreateParams {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// `snapshots.create`: takes a snapshot.
pub fn create(call: Call, cx: &mut App) -> Reply {
    let params: CreateParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(idle(&call.control, &instance, cx));
    let selection = tri!(selection(&call.params, default_take()));
    let store = store(&call.control, cx);
    let dir = call.control.instances.read(cx).store().dir(&instance.id);
    let name = params
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            crate::i18n::format::short_date(chrono::Local::now().date_naive(), true)
        });
    launcher_core::trace::emit("snapshot.create", || json!({ "instance": instance.id }));
    let label = format!("Taking a snapshot of {}", instance.name);
    let id = instance.id.clone();
    let taken = instance.clone();
    job(
        &call,
        label,
        Some(&instance),
        move |progress| {
            store
                .create(
                    &taken,
                    &dir,
                    &name,
                    params.note,
                    &selection,
                    false,
                    progress,
                )
                .map(|s| snapshot_json(&s))
                .map_err(snapshot_error)
        },
        move |control, _, cx| refresh(control, &id, cx),
        cx,
    )
}

/// `snapshots.revert`: puts back the chosen parts of a snapshot, after an automatic
/// snapshot of them.
pub fn revert(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let store = store(&call.control, cx);
    let snapshot = tri!(find(&store, &instance, &tri!(snapshot_param(&call))));
    let selection = tri!(selection(&call.params, snapshot.selection.clone()));
    let parts: Vec<&str> = selection.categories.iter().map(|c| c.id()).collect();
    tri!(call.confirm(format!(
        "replaces {} of \"{}\" with the snapshot \"{}\" (an automatic snapshot of them is taken first)",
        parts.join(", "),
        instance.name,
        snapshot.name
    )));
    tri!(idle(&call.control, &instance, cx));
    let dir = call.control.instances.read(cx).store().dir(&instance.id);
    launcher_core::trace::emit(
        "snapshot.revert",
        || json!({ "instance": instance.id, "snapshot": snapshot.id }),
    );
    let label = format!("Going back to {} in {}", snapshot.name, instance.name);
    let current = instance.clone();
    let id = instance.id.clone();
    job(
        &call,
        label,
        Some(&instance),
        move |progress| {
            let (settings, safety) = store
                .revert(&snapshot, &current, &dir, &selection, progress)
                .map_err(snapshot_error)?;
            Ok(json!({
                "reverted_to": snapshot.id,
                "safety_snapshot": snapshot_json(&safety),
                "settings": settings.map(|s| serde_json::to_value(s).unwrap_or(Value::Null)),
            }))
        },
        move |control, value, cx| {
            let settings = value
                .get("settings")
                .filter(|s| !s.is_null())
                .and_then(|s| serde_json::from_value::<Instance>(s.clone()).ok());
            control.instances.update(cx, |instances, cx| {
                if let Some(settings) = settings {
                    let _ = instances.apply_reverted_settings(settings, cx);
                } else {
                    instances.reload(cx);
                }
            });
            refresh(control, &id, cx);
        },
        cx,
    )
}

/// `snapshots.export`: writes a snapshot to a `.lodesnap` file.
pub fn export(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let store = store(&call.control, cx);
    let snapshot = tri!(find(&store, &instance, &tri!(snapshot_param(&call))));
    let mut path = tri!(path_param(&call));
    if path.is_dir() {
        path = path.join(format!(
            "{} - {}.{}",
            instance.name.replace(['/', '\\', ':'], "_"),
            snapshot.name.replace(['/', '\\', ':'], "_"),
            snapshots::EXTENSION
        ));
    }
    if path.exists() && !super::confirmed(&call.params) {
        return ready(Err(api_error(
            codes::CONFIRMATION_REQUIRED,
            format!(
                "{} exists. Ask the user first, then pass --yes (\"confirm\": true) to replace it.",
                path.display()
            ),
        )));
    }
    launcher_core::trace::emit("snapshot.export", || json!({ "snapshot": snapshot.id }));
    let label = format!("Writing {}", snapshot.name);
    job(
        &call,
        label,
        Some(&instance),
        move |progress| {
            store
                .export(&snapshot, &path, progress)
                .map_err(snapshot_error)?;
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Ok(json!({ "path": path, "bytes": bytes, "snapshot": snapshot.id }))
        },
        |_, _, _| {},
        cx,
    )
}

/// `snapshots.import`: what a `.lodesnap` file holds (`preview`), or a new instance
/// from it. One with mods needs confirm.
pub fn import(call: Call, cx: &mut App) -> Reply {
    let path = tri!(path_param(&call));
    // Reads the zip's directory and manifest only; the files are checked as they unpack.
    let snapshot = tri!(snapshots::inspect(&path).map_err(snapshot_error));
    let mut preview = snapshot_json(&snapshot);
    preview["instance_name"] = json!(snapshot.instance.name);
    preview["loader"] = json!(snapshot.instance.loader);
    preview["mods"] = json!(snapshot.mods());
    if call.params.get("preview").and_then(Value::as_bool) == Some(true) {
        return ready(Ok(preview));
    }
    let selection = tri!(selection(&call.params, snapshot.selection.clone()));
    let mods = if selection.categories.contains(&Category::Mods) {
        snapshot.mods()
    } else {
        Vec::new()
    };
    if !mods.is_empty() {
        tri!(call.confirm(format!(
            "makes an instance with {} mods ({}): programs that run when the game starts. Only use snapshots from people the user trusts",
            mods.len(),
            mods.join(", ")
        )));
    }
    let instances = call.control.instances.read(cx);
    let store = instances.snapshot_store();
    let instance_store = instances.store();
    launcher_core::trace::emit("snapshot.import", || json!({ "mods": mods.len() }));
    let label = format!("Making an instance from {}", snapshot.instance.name);
    job(
        &call,
        label,
        None,
        move |progress| {
            let instance = store
                .import_as_instance(&instance_store, &path, &selection, progress)
                .map_err(snapshot_error)?;
            Ok(json!({ "instance": instance.id, "name": instance.name }))
        },
        |control, _, cx| {
            control
                .instances
                .update(cx, |instances, cx| instances.reload(cx))
        },
        cx,
    )
}

/// `snapshots.delete`: deletes a snapshot, and the files no other one uses.
pub fn delete(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let store = store(&call.control, cx);
    let snapshot = tri!(find(&store, &instance, &tri!(snapshot_param(&call))));
    tri!(call.confirm(format!(
        "deletes the snapshot \"{}\" of \"{}\", for good",
        snapshot.name, instance.name
    )));
    launcher_core::trace::emit("snapshot.delete", || json!({ "snapshot": snapshot.id }));
    let freed = tri!(
        store
            .delete(&instance.id, &snapshot.id)
            .map_err(snapshot_error)
    );
    refresh(&call.control, &instance.id, cx);
    ready(Ok(json!({ "deleted": snapshot.id, "freed_bytes": freed })))
}
