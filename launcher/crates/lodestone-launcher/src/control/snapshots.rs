//! Snapshots, as an instance's Settings tab has them: list, take, restore (or preview what a
//! restore does), export as an image file, make a new instance from one, delete, pin, and
//! the instance's snapshot settings. Snapshots are images in the image store
//! (`launcher_core::images`).

use std::{path::PathBuf, sync::Arc};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    game::download::{CancelToken, Progress as Downloads, ProgressSnapshot},
    images::{
        ImageError, Images, RestoreScope, SnapshotKind, SnapshotPolicy, SnapshotRecord, Take,
        export,
    },
    instances::{Instance, SnapshotSettings},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{params, tri},
    jobs::{JobSpec, Progress},
    ready,
};

pub(super) fn image_error(err: ImageError) -> ErrorObject {
    match err {
        ImageError::NoSnapshot(_) | ImageError::Gone(_) => {
            api_error(codes::NOT_FOUND, err.to_string())
        }
        ImageError::Running => api_error(codes::BUSY, err.to_string()),
        ImageError::Cancelled => api_error(codes::CANCELLED, err.to_string()),
        ImageError::Invalid(_) => api_error(codes::INVALID_PARAMS, err.to_string()),
        other => api_error(codes::FAILED, other.to_string()),
    }
}

fn kind_name(kind: SnapshotKind) -> &'static str {
    match kind {
        SnapshotKind::Manual => "manual",
        SnapshotKind::BeforeChange => "before_change",
        SnapshotKind::AfterPlay => "after_play",
        SnapshotKind::BeforeRestore => "before_restore",
        SnapshotKind::Imported => "imported",
    }
}

pub(super) fn record_json(record: &SnapshotRecord) -> Value {
    json!({
        "id": record.id,
        "image": record.image,
        "name": record.name,
        "name_shown": record.name_shown(),
        "kind": kind_name(record.kind),
        "created": record.created,
        "pinned": record.pinned,
        "full": record.full,
        "worlds": record.worlds,
        "content": record.content,
        "files": record.files,
        "withheld": record.withheld,
        "bytes": record.bytes,
        "new_bytes": record.new_bytes,
    })
}

fn policy_json(policy: &SnapshotPolicy, after_play: bool) -> Value {
    json!({
        "after_play": after_play,
        "keep_last": policy.keep_last,
        "keep_daily": policy.keep_daily,
        "keep_weekly": policy.keep_weekly,
    })
}

/// The snapshot `reference` names among `records` (newest first): its id, its name, or
/// `latest`.
pub(super) fn find(
    records: &[SnapshotRecord],
    instance: &Instance,
    reference: &str,
) -> Result<SnapshotRecord, ErrorObject> {
    if reference == "latest" {
        return records.first().cloned().ok_or_else(|| {
            api_error(
                codes::NOT_FOUND,
                format!("{} has no snapshots.", instance.name),
            )
        });
    }
    if let Some(found) = records.iter().find(|s| s.id == reference) {
        return Ok(found.clone());
    }
    let named: Vec<&SnapshotRecord> = records
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

/// Changes the snapshot settings `params` names (see [`policy`]); whether any was given.
fn apply_policy(settings: &mut SnapshotSettings, params: &Value) -> Result<bool, ErrorObject> {
    let mut changed = false;
    if let Some(value) = params.get("after_play") {
        // true or false; null or "default" follows the default
        settings.after_play = value.as_bool();
        changed = true;
    }
    for (key, field) in [
        ("keep_last", &mut settings.keep_last),
        ("keep_daily", &mut settings.keep_daily),
        ("keep_weekly", &mut settings.keep_weekly),
    ] {
        let Some(value) = params.get(key) else {
            continue;
        };
        *field = match value {
            Value::Null => None,
            Value::String(text) if text == "default" => None,
            other => match other.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(n) if n <= SnapshotSettings::MAX_KEEP => Some(n),
                _ => {
                    return Err(api_error(
                        codes::INVALID_PARAMS,
                        format!(
                            "{key} is a number from 0 to {}, or default.",
                            SnapshotSettings::MAX_KEEP
                        ),
                    ));
                }
            },
        };
        changed = true;
    }
    Ok(changed)
}

/// The instance's snapshot `reference`, read now (blocking: the records file).
pub(super) fn find_now(
    images: &Images,
    instance: &Instance,
    reference: &str,
) -> Result<SnapshotRecord, ErrorObject> {
    let records = images.snapshots(&instance.id).map_err(image_error)?.records;
    find(&records, instance, reference)
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
pub(super) fn refresh(control: &Control, instance: &str, cx: &mut App) {
    let id = instance.to_string();
    control
        .instances
        .update(cx, |instances, cx| instances.load_snapshots(&id, cx));
}

/// Runs `work` as a snapshots job, with byte progress.
pub(super) fn job(
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

/// One snapshot job per instance at a time, and none while its game runs.
pub(super) fn idle(control: &Control, instance: &Instance, cx: &App) -> Result<(), ErrorObject> {
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

#[derive(Deserialize)]
struct ListParams {
    /// The deleted instances' snapshots instead.
    #[serde(default)]
    deleted: bool,
}

/// `snapshots.list`: an instance's snapshots, newest first, its images that can't be read
/// and its snapshot settings; or, with `deleted`, every deleted instance's snapshots.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let params: ListParams = params!(call);
    let images = call.control.instances.read(cx).images();
    if params.deleted {
        return cx.background_spawn(async move {
            let retired = images.deleted_snapshots().map_err(image_error)?;
            Ok(json!({
                "deleted": retired.iter().map(|r| json!({
                    "folder": r.folder,
                    "former_id": r.marker.former_id,
                    "name": r.marker.name,
                    "deleted": r.marker.deleted,
                    "snapshots": r.records.iter().map(record_json).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }))
        });
    }
    let instance = tri!(call.control.instance_param(&call, cx));
    let after_play = call
        .control
        .instances
        .read(cx)
        .snapshot_after_play(&instance.id);
    let policy = SnapshotPolicy::of(instance.snapshots.as_ref());
    cx.background_spawn(async move {
        let found = images.snapshots(&instance.id).map_err(image_error)?;
        Ok(json!({
            "instance": instance.id,
            "snapshots": found.records.iter().map(record_json).collect::<Vec<_>>(),
            "unreadable": found.unreadable.iter().map(|(image, why)| json!({ "image": image, "error": why })).collect::<Vec<_>>(),
            "policy": policy_json(&policy, after_play),
        }))
    })
}

#[derive(Deserialize)]
struct CreateParams {
    #[serde(default)]
    name: Option<String>,
    /// A full image: mods and configs too, not only what the content image links.
    #[serde(default)]
    everything: bool,
}

/// `snapshots.create`: takes a snapshot.
pub fn create(call: Call, cx: &mut App) -> Reply {
    let params: CreateParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(idle(&call.control, &instance, cx));
    let instances = call.control.instances.read(cx);
    let images = instances.images();
    let busy = instances.mark_files_busy(&instance.id);
    launcher_core::trace::emit("snapshot.create", || json!({ "instance": instance.id }));
    let label = format!("Taking a snapshot of {}", instance.name);
    let id = instance.id.clone();
    let taken = instance.id.clone();
    job(
        &call,
        label,
        Some(&instance),
        move |progress| {
            let _busy = busy;
            let record = images
                .take_snapshot(
                    &taken,
                    Take {
                        kind: SnapshotKind::Manual,
                        name: params.name,
                        full: params.everything,
                        progress,
                        cancel: launcher_core::images::NEVER,
                    },
                )
                .map_err(image_error)?
                .ok_or_else(|| api_error(codes::FAILED, "No snapshot was taken."))?;
            Ok(record_json(&record))
        },
        move |control, _, cx| refresh(control, &id, cx),
        cx,
    )
}

#[derive(Deserialize)]
struct RestoreParams {
    /// `worlds`, `worlds_and_mods`, `everything` or `new`.
    scope: String,
    /// The worlds (folders) for `worlds` and `worlds_and_mods`; all of the snapshot's
    /// without it.
    #[serde(default)]
    worlds: Vec<String>,
    /// The new instance's name, for `new`.
    #[serde(default)]
    name: Option<String>,
    /// Only say what the restore would do.
    #[serde(default)]
    preview: bool,
}

/// `snapshots.restore`: what restoring a snapshot does (`preview`), or the restore: its
/// worlds, the worlds and their mods, everything, or as a new instance. In place it takes
/// a safety snapshot first and needs confirm.
pub fn restore(call: Call, cx: &mut App) -> Reply {
    let params: RestoreParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    let images = call.control.instances.read(cx).images();
    let snapshot = tri!(find_now(&images, &instance, &tri!(snapshot_param(&call))));
    let preview = tri!(
        images
            .preview_restore(&instance.id, &snapshot.id)
            .map_err(image_error)
    );
    if params.preview {
        return ready(Ok(json!({
            "snapshot": record_json(&snapshot),
            "preview": serde_json::to_value(&preview).unwrap_or(Value::Null),
        })));
    }
    let worlds = if params.worlds.is_empty() {
        preview.worlds.iter().map(|w| w.name.clone()).collect()
    } else {
        params.worlds.clone()
    };
    let scope = match params.scope.as_str() {
        "worlds" => Some(RestoreScope::Worlds(worlds)),
        "worlds_and_mods" => Some(RestoreScope::WorldsAndContent(worlds)),
        "everything" => Some(RestoreScope::Everything),
        "new" => None,
        other => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No scope \"{other}\": worlds, worlds_and_mods, everything or new."),
            )));
        }
    };
    tri!(idle(&call.control, &instance, cx));
    let id = instance.id.clone();
    match scope {
        Some(scope) => {
            let what = match &scope {
                RestoreScope::Worlds(w) => format!("the worlds {}", w.join(", ")),
                RestoreScope::WorldsAndContent(w) => {
                    format!("the worlds {} and the mods and configs", w.join(", "))
                }
                RestoreScope::Everything => "the whole game folder".to_string(),
            };
            tri!(call.confirm(format!(
                "replaces {what} of \"{}\" with the snapshot \"{}\" (a safety snapshot of them is taken first)",
                instance.name,
                snapshot.name_shown()
            )));
            let busy = call
                .control
                .instances
                .read(cx)
                .mark_files_busy(&instance.id);
            launcher_core::trace::emit(
                "snapshot.restore",
                || json!({ "instance": instance.id, "snapshot": snapshot.id }),
            );
            let label = format!("Restoring {} in {}", snapshot.name_shown(), instance.name);
            let target = instance.id.clone();
            job(
                &call,
                label,
                Some(&instance),
                move |_| {
                    let _busy = busy;
                    let restored = images
                        .restore(&target, &snapshot.id, &scope, launcher_core::images::NEVER)
                        .map_err(image_error)?;
                    Ok(json!({
                        "restored": snapshot.id,
                        "safety_snapshot": restored.safety.as_ref().map(record_json),
                        "loader_version": restored.instance.as_ref().and_then(|i| i.loader_version.clone()),
                        "recorded": restored.recorded.as_ref().map(|r| r.id.clone()),
                    }))
                },
                move |control, value, cx| {
                    let loader = value.get("loader_version").is_some_and(|v| !v.is_null());
                    control.instances.update(cx, |instances, cx| {
                        instances.reload(cx);
                        if loader {
                            cx.emit(crate::instances::InstancesEvent::LoaderChanged(id.clone()));
                        }
                    });
                    refresh(control, &id, cx);
                },
                cx,
            )
        }
        None => {
            let name = params
                .name
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("{} ({})", instance.name, snapshot.name));
            let label = format!("Making an instance from {}", snapshot.name_shown());
            let source = instance.id.clone();
            job(
                &call,
                label,
                None,
                move |_| {
                    let made = images
                        .restore_as_new(&source, &snapshot.id, &name, launcher_core::images::NEVER)
                        .map_err(image_error)?;
                    Ok(json!({ "instance": made.id, "name": made.name }))
                },
                |control, _, cx| {
                    control
                        .instances
                        .update(cx, |instances, cx| instances.reload(cx))
                },
                cx,
            )
        }
    }
}

/// `snapshots.export`: writes a snapshot as an image file (`.lodeimg`).
pub fn export(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let images = call.control.instances.read(cx).images();
    let snapshot = tri!(find_now(&images, &instance, &tri!(snapshot_param(&call))));
    let mut path = tri!(path_param(&call));
    if path.is_dir() {
        path = path.join(format!(
            "{} - {}.{}",
            instance.name.replace(['/', '\\', ':'], "_"),
            snapshot.name.replace(['/', '\\', ':'], "_"),
            export::EXTENSION
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
    let label = format!("Writing {}", snapshot.name_shown());
    let id = instance.id.clone();
    job(
        &call,
        label,
        Some(&instance),
        move |_| {
            let report = images
                .export_snapshot(&id, &snapshot.id, &path, launcher_core::images::NEVER)
                .map_err(image_error)?;
            Ok(json!({
                "path": report.file,
                "bytes": report.bytes,
                "snapshot": snapshot.id,
                "objects": report.objects,
                "references": report.references,
                "left_out": report.left_out,
            }))
        },
        |_, _, _| {},
        cx,
    )
}

#[derive(Deserialize)]
struct ImportParams {
    #[serde(default)]
    preview: bool,
    #[serde(default)]
    name: Option<String>,
}

/// `snapshots.import`: what an image file holds (`preview`), or a new instance from it.
/// One with mods needs confirm.
pub fn import(call: Call, cx: &mut App) -> Reply {
    let params: ImportParams = params!(call);
    let path = tri!(path_param(&call));
    // Reads the zip's directory and manifests only; the files are checked as they unpack.
    let preview = tri!(export::inspect(&path).map_err(image_error));
    if params.preview {
        return ready(Ok(serde_json::to_value(&preview).unwrap_or(Value::Null)));
    }
    if !preview.mods.is_empty() {
        tri!(call.confirm(format!(
            "makes an instance with {} mods ({}): programs that run when the game starts. Only use files from people the user trusts",
            preview.mods.len(),
            preview.mods.join(", ")
        )));
    }
    let instances = call.control.instances.read(cx);
    let images = instances.images();
    let downloader = instances.content_downloader();
    let modrinth = tri!(instances.modrinth().ok_or_else(|| api_error(
        codes::UNAVAILABLE,
        "Modrinth can't be reached, so the file's mods can't be downloaded."
    )));
    let curseforge = instances.curseforge();
    let room = export::room_on(instances.store().root().to_path_buf());
    launcher_core::trace::emit("snapshot.import", || json!({ "mods": preview.mods.len() }));
    let label = format!("Making an instance from {}", preview.name_shown);
    let name = params.name;
    job(
        &call,
        label,
        None,
        move |_| {
            let progress = Downloads::default();
            let cancel = CancelToken::new();
            let imported = futures::executor::block_on(launcher_core::http::in_http_runtime(
                export::import_file(
                    &images,
                    export::Sources {
                        downloader: &downloader,
                        modrinth: &modrinth,
                        curseforge: curseforge.as_deref(),
                    },
                    &path,
                    name.as_deref(),
                    &room,
                    &progress,
                    &cancel,
                ),
            ))
            .map_err(image_error)?;
            Ok(json!({
                "instance": imported.instance.id,
                "name": imported.instance.name,
                "left_out": imported.left_out,
            }))
        },
        |control, _, cx| {
            control
                .instances
                .update(cx, |instances, cx| instances.reload(cx))
        },
        cx,
    )
}

/// `snapshots.delete`: deletes a snapshot (its image too, when no other snapshot uses it).
pub fn delete(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let images = call.control.instances.read(cx).images();
    let snapshot = tri!(find_now(&images, &instance, &tri!(snapshot_param(&call))));
    tri!(call.confirm(format!(
        "deletes the snapshot \"{}\" of \"{}\", for good",
        snapshot.name_shown(),
        instance.name
    )));
    launcher_core::trace::emit("snapshot.delete", || json!({ "snapshot": snapshot.id }));
    let deleting = cx.background_spawn({
        let instance = instance.id.clone();
        let id = snapshot.id.clone();
        async move { images.delete_snapshot(&instance, &id) }
    });
    let control = call.control.clone();
    cx.spawn(async move |cx| {
        deleting.await.map_err(image_error)?;
        cx.update(|cx| refresh(&control, &instance.id, cx));
        Ok(json!({ "deleted": snapshot.id }))
    })
}

#[derive(Deserialize)]
struct PinParams {
    #[serde(default = "yes")]
    pinned: bool,
}

fn yes() -> bool {
    true
}

/// `snapshots.pin`: pins (or with `pinned: false` unpins) a snapshot, which retention and
/// Storage's clean-up then never delete.
pub fn pin(call: Call, cx: &mut App) -> Reply {
    let params: PinParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    let images = call.control.instances.read(cx).images();
    let snapshot = tri!(find_now(&images, &instance, &tri!(snapshot_param(&call))));
    let pinning = cx.background_spawn({
        let instance = instance.id.clone();
        let id = snapshot.id.clone();
        async move { images.set_pinned(&instance, &id, params.pinned) }
    });
    let control = call.control.clone();
    cx.spawn(async move |cx| {
        pinning.await.map_err(image_error)?;
        cx.update(|cx| refresh(&control, &instance.id, cx));
        Ok(json!({ "snapshot": snapshot.id, "pinned": params.pinned }))
    })
}

/// `snapshots.policy`: the instance's snapshot settings, changing those given: `after_play`
/// (`null` or `"default"` follows Settings › Instances) and how many automatic snapshots
/// are kept (`keep_last`, `keep_daily`, `keep_weekly`; at most 1000, `null` or
/// `"default"` for the default).
pub fn policy(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let mut settings = instance.snapshots.unwrap_or_default();
    let changed = tri!(apply_policy(&mut settings, &call.params));
    let control = call.control.clone();
    if changed {
        let mut updated = instance.clone();
        updated.snapshots = (!settings.is_empty()).then_some(settings);
        let saved = control
            .instances
            .update(cx, |instances, cx| instances.save(&updated, cx));
        tri!(saved.map_err(|err| api_error(codes::FAILED, err)));
    }
    let after_play = control.instances.read(cx).snapshot_after_play(&instance.id);
    let policy = SnapshotPolicy::of(Some(&settings));
    ready(Ok(json!({
        "instance": instance.id,
        "policy": policy_json(&policy, after_play),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, name: &str) -> SnapshotRecord {
        serde_json::from_value(json!({
            "id": id,
            "image": "a".repeat(64),
            "kind": "manual",
            "name": name,
            "created": 0,
        }))
        .unwrap()
    }

    fn instance() -> Instance {
        Instance {
            name: "Survival".into(),
            ..Instance::default()
        }
    }

    #[test]
    fn finds_the_latest_an_id_or_one_name() {
        let records = [
            record("20261108-143000-aaaa", "Before 1.21"),
            record("20261107-120000-bbbb", "Twice"),
            record("20261106-120000-cccc", "twice"),
        ];
        let found = |reference: &str| find(&records, &instance(), reference);
        assert_eq!(found("latest").unwrap().id, "20261108-143000-aaaa");
        assert_eq!(found("20261107-120000-bbbb").unwrap().name, "Twice");
        assert_eq!(found("before 1.21").unwrap().id, "20261108-143000-aaaa");
        assert_eq!(found("Twice").unwrap_err().code, codes::AMBIGUOUS);
        assert_eq!(found("Nothing").unwrap_err().code, codes::NOT_FOUND);
        assert_eq!(
            find(&[], &instance(), "latest").unwrap_err().code,
            codes::NOT_FOUND
        );
    }

    #[test]
    fn a_policy_changes_only_what_is_given() {
        let mut settings = SnapshotSettings::default();
        assert!(!apply_policy(&mut settings, &json!({})).unwrap());
        assert_eq!(settings, SnapshotSettings::default());
        let given = json!({ "after_play": true, "keep_last": 10, "keep_weekly": 1000 });
        assert!(apply_policy(&mut settings, &given).unwrap());
        assert_eq!(settings.after_play, Some(true));
        assert_eq!(settings.keep_last, Some(10));
        assert_eq!(settings.keep_daily, None);
        assert_eq!(settings.keep_weekly, Some(1000));
        // null and "default" follow the defaults again
        let reset = json!({ "after_play": "default", "keep_last": null, "keep_weekly": "default" });
        assert!(apply_policy(&mut settings, &reset).unwrap());
        assert_eq!(settings, SnapshotSettings::default());
        assert!(settings.is_empty());
        apply_policy(&mut settings, &json!({ "after_play": false })).unwrap();
        assert_eq!(settings.after_play, Some(false));
    }

    #[test]
    fn a_policy_refuses_counts_out_of_bounds() {
        for bad in [
            json!(1001),
            json!(-1),
            json!(2.5),
            json!("ten"),
            json!(true),
        ] {
            let mut settings = SnapshotSettings::default();
            let refused = apply_policy(&mut settings, &json!({ "keep_daily": bad })).unwrap_err();
            assert_eq!(refused.code, codes::INVALID_PARAMS, "{bad}");
        }
    }
}
