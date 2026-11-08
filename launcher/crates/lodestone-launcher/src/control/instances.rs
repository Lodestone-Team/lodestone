//! Instances and their games: what the sidebar, the New instance dialog, the Settings
//! tab's maintenance buttons and the Play button do.

use std::{path::PathBuf, rc::Rc, time::Duration};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    export,
    game::{
        launch::JoinServer,
        loaders::{LoaderChoice, LoaderKind},
    },
    instances::{self as core_instances, Instance},
};
use lodestone_minecraft::versions::mojang::MojangVersionType;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    events::row_json,
    jobs::{Cancel, JobSpec, Progress},
    poll_until, ready, servers,
};
use crate::{
    instances::{Instances, appearance::IconChoice},
    servers::Servers,
};

macro_rules! params {
    ($call:expr) => {
        match $call.params() {
            Ok(params) => params,
            Err(err) => return $crate::control::ready(Err(err)),
        }
    };
}
pub(super) use params;

macro_rules! tri {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(err) => return $crate::control::ready(Err(err)),
        }
    };
}
pub(super) use tri;

/// A `Result<_, String>` from the app, as a `failed` error.
pub fn failed<T>(result: Result<T, String>) -> Result<T, ErrorObject> {
    result.map_err(|message| api_error(codes::FAILED, message))
}

/// `"fabric"`, `"forge"`, …; `"vanilla"` or `"none"` for no loader.
pub fn parse_loader(text: &str) -> Result<Option<LoaderKind>, ErrorObject> {
    match text.trim().to_ascii_lowercase().as_str() {
        "vanilla" | "none" | "" => Ok(None),
        "fabric" => Ok(Some(LoaderKind::Fabric)),
        "quilt" => Ok(Some(LoaderKind::Quilt)),
        "forge" => Ok(Some(LoaderKind::Forge)),
        "neoforge" | "neo-forge" => Ok(Some(LoaderKind::NeoForge)),
        other => Err(api_error(
            codes::INVALID_PARAMS,
            format!("No loader \"{other}\". Loaders: vanilla, fabric, quilt, forge, neoforge."),
        )),
    }
}

/// An instance as the API shows it: what `instance.json` holds, and what the window
/// shows about it.
pub fn instance_json(instance: &Instance, instances: &Instances, servers: &Servers) -> Value {
    let mut value = serde_json::to_value(instance).unwrap_or_default();
    let status = instances.run_status(&instance.id);
    let link = servers.instance_link(&instance.id);
    let extra = json!({
        "loader_label": instance.loader_label(),
        "effective_group": core_instances::group_of(instance, instances.auto_groups()),
        "selected": instances.selected().is_some_and(|s| s.id == instance.id),
        "state": status.state,
        "server": link.map(|link| json!({
            "server_name": link.server_name,
            "core_name": link.core_name,
            "core_known": link.core_known,
        })),
        "modpack": instances.modpack(&instance.id).map(|record| json!({
            "title": record.title,
            "project_id": record.project_id,
            "version_id": record.version_id,
            "version_number": record.version_number,
        })),
        "dir": instances.content_dirs(&instance.id).instance,
    });
    if let (Value::Object(object), Value::Object(extra)) = (&mut value, extra) {
        object.extend(extra);
    }
    value
}

/// Each instance with its state, for `meta.status`.
pub fn summaries(control: &Control, cx: &App) -> Value {
    let instances = control.instances.read(cx);
    Value::Array(
        instances
            .list()
            .iter()
            .map(|i| {
                let status = instances.run_status(&i.id);
                json!({
                    "id": i.id,
                    "name": i.name,
                    "game_version": i.game_version,
                    "loader": i.loader_label(),
                    "state": status.state,
                })
            })
            .collect(),
    )
}

pub fn list(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let instances = control.instances.read(cx);
    let servers = control.servers.read(cx);
    let groups: Vec<Value> = instances
        .grouped()
        .into_iter()
        .map(|(name, members)| {
            json!({
                "group": name,
                "instances": members.iter().map(|i| &i.id).collect::<Vec<_>>(),
            })
        })
        .collect();
    let list: Vec<Value> = instances
        .list()
        .iter()
        .map(|i| instance_json(i, instances, servers))
        .collect();
    ready(Ok(json!({
        "instances": list,
        "groups": groups,
        "auto_groups": instances.auto_groups(),
        "last_played": core_instances::last_played(instances.list()).map(|i| &i.id),
        "notice": instances.notice().map(|n| n.to_string()),
    })))
}

pub fn show(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let instance = tri!(control.instance_param(&call, cx));
    let instances = control.instances.read(cx);
    let mut value = instance_json(&instance, instances, control.servers.read(cx));
    let status = instances.run_status(&instance.id);
    value["run"] = run_json(&status);
    value["effective"] = settings_effective(&instance, instances);
    ready(Ok(json!({ "instance": value })))
}

pub fn run_json(status: &crate::instances::control::RunStatus) -> Value {
    json!({
        "state": status.state,
        "pid": status.pid,
        "running_secs": status.running_secs,
        "progress": status.progress.map(|(stage, snapshot)| super::jobs::progress_json(stage.label(), snapshot)),
    })
}

/// What the game gets: the instance's settings over the defaults.
pub fn settings_effective(instance: &Instance, instances: &Instances) -> Value {
    let (min, max) = instance.memory(instances.defaults(), instances.total_memory());
    let settings = instance.game_settings(instances.defaults(), instances.total_memory());
    json!({
        "memory": { "min": min, "max": max },
        "jvm_args": settings.jvm_args,
        "game_args": settings.game_args,
        "resolution": settings.resolution.map(|(width, height)| json!({ "width": width, "height": height })),
        "fullscreen": settings.fullscreen,
        "env": settings.env.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "java": instance.java_override,
    })
}

pub fn select(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    call.control
        .instances
        .update(cx, |instances, cx| instances.select(&instance.id, cx));
    ready(Ok(json!({ "selected": instance.id })))
}

#[derive(Deserialize)]
struct VersionsParams {
    #[serde(default)]
    snapshots: bool,
    #[serde(default)]
    old: bool,
    #[serde(default)]
    limit: Option<usize>,
}

pub fn versions(call: Call, cx: &mut App) -> Reply {
    let params: VersionsParams = params!(call);
    let installer = call.control.instances.read(cx).installer();
    cx.spawn(async move |cx| {
        let manifest = cx
            .background_spawn(async move { installer.version_manifest().await })
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let latest_release = manifest
            .versions
            .iter()
            .find(|v| v.kind == MojangVersionType::Release)
            .map(|v| v.id.clone());
        let list: Vec<Value> = manifest
            .versions
            .iter()
            .filter(|v| match v.kind {
                MojangVersionType::Release => true,
                MojangVersionType::Snapshot => params.snapshots,
                _ => params.old,
            })
            .take(params.limit.unwrap_or(usize::MAX))
            .map(|v| {
                json!({
                    "id": v.id,
                    "type": format!("{:?}", v.kind).to_lowercase(),
                    "released": v.release_time,
                })
            })
            .collect();
        Ok(json!({ "latest_release": latest_release, "versions": list }))
    })
}

#[derive(Deserialize)]
struct LoaderVersionsParams {
    loader: String,
    game_version: String,
}

pub fn loader_versions(call: Call, cx: &mut App) -> Reply {
    let params: LoaderVersionsParams = params!(call);
    let Some(kind) = tri!(parse_loader(&params.loader)) else {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Vanilla has no loader versions.",
        )));
    };
    let installer = call.control.instances.read(cx).installer();
    let game_version = params.game_version.clone();
    cx.spawn(async move |cx| {
        let list = cx
            .background_spawn(async move { installer.loader_versions(kind, &game_version).await })
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        Ok(json!({
            "loader": kind.label(),
            "game_version": params.game_version,
            "default": default_loader_version(&list),
            "versions": list.iter().map(|v| json!({
                "id": v.id,
                "display": v.display,
                "stable": v.stable,
                "recommended": v.recommended,
            })).collect::<Vec<_>>(),
        }))
    })
}

/// The version the loader picker starts at: the recommended one, else the first.
fn default_loader_version(list: &[launcher_core::game::loaders::LoaderVersion]) -> Option<String> {
    list.iter()
        .find(|v| v.recommended)
        .or_else(|| list.first())
        .map(|v| v.id.clone())
}

#[derive(Deserialize)]
struct CreateParams {
    #[serde(default)]
    name: Option<String>,
    /// A Minecraft version, or "latest" (the newest release).
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    loader: Option<String>,
    #[serde(default)]
    loader_version: Option<String>,
    #[serde(default)]
    group: Option<String>,
}

pub fn create(call: Call, cx: &mut App) -> Reply {
    let params: CreateParams = params!(call);
    let kind = match &params.loader {
        Some(loader) => tri!(parse_loader(loader)),
        None => None,
    };
    let control = call.control.clone();
    let installer = control.instances.read(cx).installer();
    cx.spawn(async move |cx| {
        let wanted = params
            .version
            .clone()
            .filter(|v| !v.trim().is_empty() && v != "latest");
        let check_installer = installer.clone();
        let manifest = cx
            .background_spawn(async move { check_installer.version_manifest().await })
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let version = match wanted {
            Some(version) => {
                if !manifest.versions.iter().any(|v| v.id == version) {
                    return Err(api_error(
                        codes::NOT_FOUND,
                        format!("Minecraft has no version \"{version}\". List them with `instances versions`."),
                    ));
                }
                version
            }
            None => manifest
                .versions
                .iter()
                .find(|v| v.kind == MojangVersionType::Release)
                .map(|v| v.id.clone())
                .ok_or_else(|| api_error(codes::UNAVAILABLE, "Mojang's list has no release."))?,
        };
        let loader = match kind {
            None => None,
            Some(kind) => {
                let game_version = version.clone();
                let list = cx
                    .background_spawn(
                        async move { installer.loader_versions(kind, &game_version).await },
                    )
                    .await
                    .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
                let id = match &params.loader_version {
                    Some(wanted) => list
                        .iter()
                        .find(|v| &v.id == wanted || &v.display == wanted)
                        .map(|v| v.id.clone())
                        .ok_or_else(|| {
                            api_error(
                                codes::NOT_FOUND,
                                format!("{} has no version \"{wanted}\" for Minecraft {version}.", kind.label()),
                            )
                        })?,
                    None => default_loader_version(&list).ok_or_else(|| {
                        api_error(
                            codes::UNAVAILABLE,
                            format!("{} has no build for Minecraft {version}.", kind.label()),
                        )
                    })?,
                };
                Some(LoaderChoice { kind, version: id })
            }
        };
        // the dialog's name when none is given
        let name = params
            .name
            .clone()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| match &loader {
                Some(loader) => format!("{version} {}", loader.kind.label()),
                None => version.clone(),
            });
        let created = control.instances.update(cx, |instances, cx| {
            instances.create(&name, &version, loader.as_ref(), cx)
        });
        let mut instance = failed(created)?;
        if let Some(group) = params.group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()) {
            instance.group = Some(group);
            let saved = control
                .instances
                .update(cx, |instances, cx| instances.save(&instance, cx));
            failed(saved)?;
        }
        Ok(cx.update(|cx| {
            json!({ "instance": instance_json(&instance, control.instances.read(cx), control.servers.read(cx)) })
        }))
    })
}

/// Saves a changed copy of the instance, as the Settings tab's Save does.
pub fn save_changed(
    control: &Control,
    instance: Instance,
    cx: &mut App,
) -> Result<Value, ErrorObject> {
    let saved = control
        .instances
        .update(cx, |instances, cx| instances.save(&instance, cx));
    failed(saved)?;
    let current = control.instance(&instance.id, cx)?;
    Ok(json!({
        "instance": instance_json(&current, control.instances.read(cx), control.servers.read(cx))
    }))
}

#[derive(Deserialize)]
struct RenameParams {
    name: String,
}

pub fn rename(call: Call, cx: &mut App) -> Reply {
    let params: RenameParams = params!(call);
    let mut instance = tri!(call.control.instance_param(&call, cx));
    let name = params.name.trim();
    if name.is_empty() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give the instance a name.",
        )));
    }
    instance.name = name.to_string();
    ready(save_changed(&call.control, instance, cx))
}

#[derive(Deserialize)]
struct GroupParams {
    #[serde(default)]
    group: Option<String>,
}

pub fn set_group(call: Call, cx: &mut App) -> Reply {
    let params: GroupParams = params!(call);
    let mut instance = tri!(call.control.instance_param(&call, cx));
    instance.group = params
        .group
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty());
    ready(save_changed(&call.control, instance, cx))
}

#[derive(Deserialize)]
struct IconParams {
    /// An image file (PNG, JPEG, WebP, GIF), absolute.
    #[serde(default)]
    image: Option<PathBuf>,
    /// A library icon's name.
    #[serde(default)]
    library: Option<String>,
    #[serde(default)]
    emoji: Option<String>,
    /// The emoji tile's colour, `RRGGBB`.
    #[serde(default)]
    tile: Option<String>,
    #[serde(default)]
    clear: bool,
}

pub fn set_icon(call: Call, cx: &mut App) -> Reply {
    let params: IconParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    let chosen = [
        params.image.is_some(),
        params.library.is_some(),
        params.emoji.is_some(),
        params.clear,
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if chosen != 1 {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give one of: an image, a library icon, an emoji, or clear.",
        )));
    }
    let id = instance.id.clone();
    let result = if let Some(image) = params.image {
        tri!(absolute(&image));
        call.control
            .instances
            .update(cx, |instances, cx| instances.set_icon(&id, &image, cx))
    } else if let Some(name) = params.library {
        let known = crate::instances::appearance::LIBRARY
            .iter()
            .any(|(n, _)| *n == name);
        if !known {
            let names: Vec<&str> = crate::instances::appearance::LIBRARY
                .iter()
                .map(|(n, _)| *n)
                .collect();
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No library icon \"{name}\". Icons: {}.", names.join(", ")),
            )));
        }
        let icon = IconChoice::library(&name);
        call.control.instances.update(cx, |instances, cx| {
            instances.set_icon_name(&id, Some(&icon), cx)
        })
    } else if let Some(emoji) = params.emoji {
        let tile = match params
            .tile
            .as_deref()
            .map(|t| u32::from_str_radix(t.trim_start_matches('#'), 16))
        {
            None => 0x3A3A4A,
            Some(Ok(tile)) if tile <= 0xFF_FFFF => tile,
            Some(_) => {
                return ready(Err(api_error(
                    codes::INVALID_PARAMS,
                    "The tile colour is six hex digits, like 3A6EA5.",
                )));
            }
        };
        let icon = IconChoice::emoji(tile, emoji.trim());
        if !core_instances::valid_icon_name(&icon) {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "That emoji can't be an icon.",
            )));
        }
        call.control.instances.update(cx, |instances, cx| {
            instances.set_icon_name(&id, Some(&icon), cx)
        })
    } else if instance.icon.as_deref() == Some(core_instances::CUSTOM_ICON) {
        call.control
            .instances
            .update(cx, |instances, cx| instances.clear_icon(&id, cx))
    } else {
        call.control
            .instances
            .update(cx, |instances, cx| instances.set_icon_name(&id, None, cx))
    };
    tri!(failed(result));
    let current = tri!(call.control.instance(&id, cx));
    ready(Ok(
        json!({ "instance": { "id": id, "icon": current.icon } }),
    ))
}

#[derive(Deserialize)]
struct BackgroundParams {
    #[serde(default)]
    image: Option<PathBuf>,
    #[serde(default)]
    clear: bool,
}

pub fn set_background(call: Call, cx: &mut App) -> Reply {
    let params: BackgroundParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    let id = instance.id.clone();
    let result = match (params.image, params.clear) {
        (Some(image), false) => {
            tri!(absolute(&image));
            call.control.instances.update(cx, |instances, cx| {
                instances.set_background(&id, &image, cx)
            })
        }
        (None, true) => call
            .control
            .instances
            .update(cx, |instances, cx| instances.clear_background(&id, cx)),
        _ => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "Give an image, or clear.",
            )));
        }
    };
    tri!(failed(result));
    let current = tri!(call.control.instance(&id, cx));
    ready(Ok(
        json!({ "instance": { "id": id, "background": current.background } }),
    ))
}

/// Paths come from another process with another working directory.
pub fn absolute(path: &std::path::Path) -> Result<(), ErrorObject> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(api_error(
            codes::INVALID_PARAMS,
            format!("{} isn't an absolute path.", path.display()),
        ))
    }
}

pub fn duplicate(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    let copy = call
        .control
        .instances
        .update(cx, |instances, cx| instances.duplicate(&instance.id, cx));
    let copy = tri!(failed(copy));
    let control = &call.control;
    ready(Ok(json!({
        "instance": instance_json(&copy, control.instances.read(cx), control.servers.read(cx))
    })))
}

pub fn delete(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(call.confirm(format!(
        "moves the instance \"{}\" (with its worlds and mods) to the Trash",
        instance.name
    )));
    tri!(call.control.idle(&instance, cx));
    let result = call
        .control
        .instances
        .update(cx, |instances, cx| instances.delete(&instance.id, cx));
    tri!(failed(result));
    ready(Ok(
        json!({ "deleted": instance.id, "name": instance.name, "trash": true }),
    ))
}

#[derive(Deserialize)]
struct ExportParams {
    path: PathBuf,
}

pub fn export(call: Call, cx: &mut App) -> Reply {
    let params: ExportParams = params!(call);
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(absolute(&params.path));
    if params.path.extension().and_then(|e| e.to_str()) != Some("mrpack") {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "A Modrinth modpack's file name ends in .mrpack.",
        )));
    }
    let store = call.control.instances.read(cx).store();
    let game_dir = store.game_dir(&instance.id);
    let instance_dir = store.dir(&instance.id);
    let out = params.path.clone();
    let exported = instance.clone();
    // the steps of `Instances::export`, with the outcome returned
    let work = cx.background_spawn(async move {
        let on_modrinth = export::modrinth_files(&instance_dir);
        let plan = export::plan(&exported, &game_dir, &on_modrinth)
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        export::write(&plan, &out).map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        Ok(json!({
            "path": out,
            "files_listed": plan.index.files.len(),
            "files_copied": plan.overrides.len(),
            "skipped": plan.skipped.iter().map(|(file, why)| json!({ "file": file, "reason": why.to_string() })).collect::<Vec<_>>(),
        }))
    });
    let control = call.control.clone();
    let job = control.start_job(
        JobSpec::new("export", format!("Exporting {}", instance.name)).instance(&instance),
        work,
        cx,
    );
    control.reply_job(&call, job, cx)
}

pub fn verify(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(call.control.idle(&instance, cx));
    let control = call.control.clone();
    let seq = control.notices.borrow().last_seq();
    let id = instance.id.clone();
    control
        .instances
        .update(cx, |instances, cx| instances.verify(&id, cx));
    let work = follow_run(control.clone(), id.clone(), seq, false, cx);
    let job = control.start_job(
        JobSpec::new("verify", format!("Verifying {}", instance.name))
            .instance(&instance)
            .progress(Progress::Run(id.clone()))
            .cancel(Cancel::Instance(id)),
        work,
        cx,
    );
    control.reply_job(&call, job, cx)
}

/// Follows an instance's run until it is running (`until_running`) or idle again, and
/// ends with what the window said about it.
fn follow_run(
    control: Rc<Control>,
    id: String,
    seq: u64,
    until_running: bool,
    cx: &mut App,
) -> gpui::Task<super::ApiResult> {
    cx.spawn(async move |cx| {
        // a job goes on when its client goes away
        let status = poll_until(
            cx,
            None,
            Duration::from_millis(250),
            Duration::from_secs(6 * 3600),
            |cx| {
                let status = control.instances.read(cx).run_status(&id);
                match status.state {
                    "running" if until_running => Some(status),
                    "idle" => Some(status),
                    _ => None,
                }
            },
        )
        .await?;
        if status.state == "running" {
            return Ok(json!({ "state": "running", "pid": status.pid }));
        }
        let notice = control.notices.borrow().find_since(seq, |n| {
            matches!(n.kind, "notice" | "launch_failed" | "crashed")
        });
        match notice {
            Some(notice) if notice.is_failure() => Err(api_error(
                codes::FAILED,
                format!("{}: {}", notice.title, notice.message),
            )
            .with_data(notice.to_json())),
            Some(notice) => {
                Ok(json!({ "state": "idle", "title": notice.title, "message": notice.message }))
            }
            None => Err(api_error(codes::CANCELLED, "Cancelled.")),
        }
    })
}

#[derive(Deserialize)]
struct LaunchParams {
    /// `host` or `host:port` to join once the game starts.
    #[serde(default)]
    join: Option<String>,
    /// Don't join the server the instance is linked to.
    #[serde(default)]
    no_join: bool,
    #[serde(default)]
    update_mods: Option<bool>,
    #[serde(default)]
    accept_advisory: bool,
}

/// Reads `host`, `host:port` or `[v6]:port`.
pub fn parse_join(text: &str) -> Result<JoinServer, ErrorObject> {
    let text = text.trim();
    let bad = || {
        api_error(
            codes::INVALID_PARAMS,
            format!("\"{text}\" isn't a server address: give host or host:port."),
        )
    };
    let (host, port) = if let Some(rest) = text.strip_prefix('[') {
        let (host, rest) = rest.split_once(']').ok_or_else(bad)?;
        let port = match rest.strip_prefix(':') {
            Some(port) => port.parse().map_err(|_| bad())?,
            None if rest.is_empty() => 25565,
            None => return Err(bad()),
        };
        (host.to_string(), port)
    } else {
        match text.rsplit_once(':') {
            Some((host, port)) if !host.contains(':') => {
                (host.to_string(), port.parse().map_err(|_| bad())?)
            }
            Some(_) => (text.to_string(), 25565),
            None => (text.to_string(), 25565),
        }
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err(bad());
    }
    Ok(JoinServer { host, port })
}

pub fn launch(call: Call, cx: &mut App) -> Reply {
    let params: LaunchParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    if control.instances.read(cx).is_active(&instance.id) {
        let status = control.instances.read(cx).run_status(&instance.id);
        return ready(Err(api_error(
            codes::BUSY,
            format!("{} is already {}.", instance.name, status.state),
        )
        .with_data(json!({ "state": status.state, "pid": status.pid }))));
    }
    let linked = control
        .servers
        .read(cx)
        .instance_link(&instance.id)
        .is_some();
    if params.join.is_none() && linked && !params.no_join {
        // the Play button of a linked instance joins its server
        return servers::join_instance(
            call,
            instance,
            servers::JoinOptions {
                update_mods: params.update_mods,
                accept_advisory: params.accept_advisory,
            },
            cx,
        );
    }
    let join = match &params.join {
        Some(address) => Some(tri!(parse_join(address))),
        None => None,
    };
    let seq = control.notices.borrow().last_seq();
    let id = instance.id.clone();
    let joining = join.as_ref().map(|j| j.address());
    control.instances.update(cx, |instances, cx| match join {
        Some(join) => instances.play_joining(&id, Some(join), cx),
        None => instances.play(&id, cx),
    });
    start_launch_job(&call, &instance, seq, joining, cx)
}

/// The job that follows a launch until the game runs.
pub fn start_launch_job(
    call: &Call,
    instance: &Instance,
    seq: u64,
    joining: Option<String>,
    cx: &mut App,
) -> Reply {
    let control = call.control.clone();
    let id = instance.id.clone();
    let work = follow_run(control.clone(), id.clone(), seq, true, cx);
    let label = match &joining {
        Some(address) => format!("Starting {} and joining {address}", instance.name),
        None => format!("Starting {}", instance.name),
    };
    let job = control.start_job(
        JobSpec::new("launch", label)
            .instance(instance)
            .progress(Progress::Run(id.clone()))
            .cancel(Cancel::Instance(id)),
        work,
        cx,
    );
    control.reply_job(call, job, cx)
}

pub fn stop(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let status = control.instances.read(cx).run_status(&instance.id);
    if status.state == "idle" {
        return ready(Err(api_error(
            codes::CONFLICT,
            format!("{} isn't running.", instance.name),
        )));
    }
    let id = instance.id.clone();
    control
        .instances
        .update(cx, |instances, cx| instances.stop(&id, cx));
    if !call.wait() {
        return ready(Ok(json!({ "instance": id, "stopping": true })));
    }
    let outbox = call.outbox.clone();
    cx.spawn(async move |cx| {
        poll_until(
            cx,
            Some(&outbox),
            Duration::from_millis(200),
            Duration::from_secs(60),
            |cx| (control.instances.read(cx).run_status(&id).state == "idle").then_some(()),
        )
        .await?;
        Ok(json!({ "instance": id, "state": "idle" }))
    })
}

#[derive(Deserialize)]
struct StatusParams {
    #[serde(default)]
    instance: Option<String>,
}

pub fn status(call: Call, cx: &mut App) -> Reply {
    let params: StatusParams = params!(call);
    let control = &call.control;
    let only = match &params.instance {
        Some(reference) => Some(tri!(control.instance(reference, cx)).id),
        None => None,
    };
    let instances = control.instances.read(cx);
    let servers = control.servers.read(cx);
    let list: Vec<Value> = instances
        .list()
        .iter()
        .filter(|i| only.as_ref().is_none_or(|id| id == &i.id))
        .map(|i| {
            let status = instances.run_status(&i.id);
            let mut value = run_json(&status);
            value["instance"] = json!(i.id);
            value["name"] = json!(i.name);
            value["server_busy"] = json!(servers.instance_busy(&i.id).map(|b| b.label()));
            value
        })
        .collect();
    ready(Ok(json!({ "instances": list })))
}

#[derive(Deserialize)]
struct LogsParams {
    #[serde(default = "default_lines")]
    lines: usize,
}

fn default_lines() -> usize {
    100
}

pub fn logs(call: Call, cx: &mut App) -> Reply {
    let params: LogsParams = params!(call);
    let control = &call.control;
    let instance = tri!(control.instance_param(&call, cx));
    let instances = control.instances.read(cx);
    let (rows, cursor) = instances.log_tail(&instance.id, params.lines);
    ready(Ok(json!({
        "instance": instance.id,
        "rows": rows.iter().map(|r| row_json(&instance.id, r)).collect::<Vec<_>>(),
        "cursor": cursor,
        "state": instances.run_status(&instance.id).state,
    })))
}

pub fn clear_logs(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    tri!(call.confirm("clears the instance's console"));
    call.control
        .instances
        .update(cx, |instances, cx| instances.clear_log(&instance.id, cx));
    ready(Ok(json!({ "instance": instance.id, "cleared": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_server_addresses() {
        let join = |text: &str| parse_join(text).map(|j| (j.host, j.port));
        assert_eq!(
            join("mc.example.com").unwrap(),
            ("mc.example.com".into(), 25565)
        );
        assert_eq!(join("10.0.0.5:25570").unwrap(), ("10.0.0.5".into(), 25570));
        assert_eq!(join("[::1]:25566").unwrap(), ("::1".into(), 25566));
        assert_eq!(join("[::1]").unwrap(), ("::1".into(), 25565));
        assert_eq!(join("::1").unwrap(), ("::1".into(), 25565));
        assert!(join("host:port").is_err());
        assert!(join("").is_err());
        assert!(join("a b:1").is_err());
    }

    #[test]
    fn reads_loaders() {
        assert_eq!(parse_loader("Fabric").unwrap(), Some(LoaderKind::Fabric));
        assert_eq!(
            parse_loader("neoforge").unwrap(),
            Some(LoaderKind::NeoForge)
        );
        assert_eq!(parse_loader("vanilla").unwrap(), None);
        assert_eq!(
            parse_loader("rift").unwrap_err().code,
            codes::INVALID_PARAMS
        );
    }
}
