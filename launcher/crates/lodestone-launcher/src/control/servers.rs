//! The Servers page: Lodestone cores, their servers, and the questions the page asks.
//!
//! Trust is kept as the page keeps it. Installing a server's mods from a core that
//! isn't trusted for mods shows the advisory: the API answers it only when the caller
//! said so (`accept_advisory`), and otherwise cancels it and fails with
//! `untrusted_core` and the advisory. Trusting a core is its own method,
//! `cores.trust`, which needs `confirm`. A certificate this Mac doesn't trust is
//! pinned only with the fingerprint the user compared.

use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use gpui::{App, SharedString};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    Secret,
    cores::{ServerInstance, client::display_address, live::LauncherScope, store::SavedCore},
    game::download::{CancelToken, Progress as Downloads},
    instances::Instance,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ApiResult, Call, Control, Reply, api_error, core_error,
    instances::{failed, params, start_launch_job, tri},
    jobs::{Cancel, JobSpec, Progress},
    poll_until, ready,
};
use crate::servers::{
    AdvisoryAnswer, CoreEntry, CoreStatus, Servers,
    add_dialog::KeyExpiry,
    control::{AddCore, HostCore},
    host::{EULA_URL, MEMORY_CHOICES},
    trust_explanation,
};

fn lower(value: impl std::fmt::Debug) -> String {
    format!("{value:?}").to_lowercase()
}

fn server_json(core: &CoreEntry, server: &ServerInstance, servers: &Servers) -> Value {
    let row = (core.saved.uuid.clone(), server.uuid.clone());
    json!({
        "uuid": server.uuid,
        "name": server.name,
        "core": core.saved.uuid,
        "core_name": core.saved.name,
        "game_version": server.game_version,
        "loader": lower(server.loader),
        "loader_version": server.loader_version,
        "state": lower(server.state),
        "players": server.player_count,
        "max_players": server.max_player_count,
        "port": server.port,
        "address_shown": server.address_shown(),
        "playit_address": server.playit_address,
        "has_client_pack": server.has_client_pack,
        "listed": server.listed,
        "linked_instance": servers.linked_instance(&core.saved.uuid, &server.uuid),
        "busy": servers.busy.get(&row).map(|b| b.label()),
    })
}

fn core_json(core: &CoreEntry, servers: &Servers, with_servers: bool) -> Value {
    let (status, error) = match &core.status {
        CoreStatus::Loading => ("loading", None),
        CoreStatus::Online(_) => ("online", None),
        CoreStatus::Failed(err) => ("failed", Some(core_error(err.clone()))),
    };
    let saved: &SavedCore = &core.saved;
    let mut value = json!({
        "id": saved.uuid,
        "name": saved.name,
        "address": display_address(&core.url),
        "status": status,
        "error": error,
        "signed_in": core.signed_in(),
        "username": saved.username,
        // the key's id only, as the window shows it ("lsk_<id>_…"); never the key
        "key_id": saved.key_id,
        "key_expires_at": saved.key_expires_at,
        "trusted_for_mods": saved.is_trusted(),
        "certificate_pinned": saved.certificate.is_some(),
        "product": saved.product.label(),
        "launched_by": (!saved.launched_by.is_unknown()).then_some(saved.launched_by),
    });
    if with_servers {
        value["servers"] = Value::Array(
            core.servers()
                .iter()
                .map(|s| server_json(core, s, servers))
                .collect(),
        );
    }
    value
}

/// The cores, for `meta.status`.
pub fn cores_summary(control: &Control, cx: &App) -> Value {
    let servers = control.servers.read(cx);
    Value::Array(
        servers
            .cores
            .iter()
            .map(|c| core_json(c, servers, false))
            .collect(),
    )
}

/// Fetches every core's list (as the page does while shown), and waits for them.
fn fresh(
    control: &Rc<Control>,
    call: &Call,
    cached: bool,
    cx: &mut App,
) -> gpui::Task<Result<(), ErrorObject>> {
    if cached {
        return gpui::Task::ready(Ok(()));
    }
    control
        .servers
        .update(cx, |servers, cx| servers.refresh_all(cx));
    let control = control.clone();
    let outbox = call.outbox.clone();
    cx.spawn(async move |cx| {
        // a core that doesn't answer in time is listed with what it said last
        let _ = poll_until(
            cx,
            Some(&outbox),
            Duration::from_millis(150),
            Duration::from_secs(25),
            |cx| (!control.servers.read(cx).control_refreshing()).then_some(()),
        )
        .await;
        Ok(())
    })
}

#[derive(Deserialize, Default)]
struct ListParams {
    /// Don't ask the cores again; list what the launcher has.
    #[serde(default)]
    cached: bool,
}

pub fn cores_list(call: Call, cx: &mut App) -> Reply {
    let params: ListParams = params!(call);
    let control = call.control.clone();
    let refresh = fresh(&control, &call, params.cached, cx);
    cx.spawn(async move |cx| {
        refresh.await?;
        Ok(cx.update(|cx| {
            let servers = control.servers.read(cx);
            json!({
                "cores": servers.cores.iter().map(|c| core_json(c, servers, true)).collect::<Vec<_>>(),
                "notice": servers.notice.as_ref().map(|notice| {
                    let (title, message) = notice.text();
                    json!({ "title": title, "message": message })
                }),
            })
        }))
    })
}

pub fn servers_list(call: Call, cx: &mut App) -> Reply {
    let params: ListParams = params!(call);
    let control = call.control.clone();
    let only = call
        .params
        .get("core")
        .and_then(Value::as_str)
        .map(str::to_string);
    let refresh = fresh(&control, &call, params.cached, cx);
    cx.spawn(async move |cx| {
        refresh.await?;
        cx.update(|cx| {
            let servers = control.servers.read(cx);
            let only = match &only {
                Some(reference) => Some(servers.control_core(reference)?),
                None => None,
            };
            let list: Vec<Value> = servers
                .cores
                .iter()
                .enumerate()
                .filter(|(i, _)| only.is_none_or(|o| o == *i))
                .flat_map(|(_, c)| c.servers().iter().map(move |s| (c, s)))
                .map(|(c, s)| server_json(c, s, servers))
                .collect();
            Ok(json!({ "servers": list }))
        })
    })
}

fn core_param(call: &Call, servers: &Servers) -> Result<usize, ErrorObject> {
    let reference = call
        .params
        .get("core")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                "Name the core: its id, name or address.",
            )
        })?;
    servers.control_core(reference)
}

/// The server named by the `server` parameter (its uuid or name), on the `core`
/// parameter's core or any core.
fn find_server(call: &Call, servers: &Servers) -> Result<(String, ServerInstance), ErrorObject> {
    let reference = call
        .params
        .get("server")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                "Name the server: its id or its name.",
            )
        })?;
    let only = match call.params.get("core").and_then(Value::as_str) {
        Some(core) => Some(servers.control_core(core)?),
        None => None,
    };
    let candidates: Vec<(String, &ServerInstance)> = servers
        .cores
        .iter()
        .enumerate()
        .filter(|(i, _)| only.is_none_or(|o| o == *i))
        .flat_map(|(_, c)| c.servers().iter().map(|s| (c.saved.uuid.clone(), s)))
        .collect();
    if let Some((core, server)) = candidates.iter().find(|(_, s)| s.uuid == reference) {
        return Ok((core.clone(), (*server).clone()));
    }
    let matches: Vec<&(String, &ServerInstance)> = candidates
        .iter()
        .filter(|(_, s)| s.name.eq_ignore_ascii_case(reference))
        .collect();
    match matches.as_slice() {
        [(core, server)] => Ok((core.clone(), (*server).clone())),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("No server is called \"{reference}\". List them with `servers list` (signed-in cores show more)."),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("More than one server is called \"{reference}\"; pass --core or use its id."),
        )
        .with_data(json!({ "matches": matches.iter().map(|(c, s)| json!({ "core": c, "server": s.uuid })).collect::<Vec<_>>() }))),
    }
}

#[derive(Deserialize)]
struct AddParams {
    address: String,
    #[serde(default)]
    username: Option<String>,
    /// Read from stdin by the CLI; dropped once the key is made.
    #[serde(default)]
    password: Option<String>,
    /// `90d`, `1y` or `never`.
    #[serde(default)]
    expiry: Option<String>,
    #[serde(default)]
    console_read: bool,
    #[serde(default)]
    console_write: bool,
    #[serde(default)]
    fingerprint: Option<String>,
}

pub fn cores_add(call: Call, cx: &mut App) -> Reply {
    let params: AddParams = params!(call);
    let expiry = match params.expiry.as_deref() {
        None | Some("1y") | Some("year") => KeyExpiry::Year,
        Some("90d") => KeyExpiry::Days90,
        Some("never") => KeyExpiry::Never,
        Some(other) => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No key expiry \"{other}\": 90d, 1y or never."),
            )));
        }
    };
    let now = launcher_core::instances::now() as i64;
    let request = AddCore {
        address: params.address.clone(),
        username: params.username.filter(|u| !u.trim().is_empty()),
        password: params.password.map(Secret::new),
        expires_at: expiry.expires_at(now),
        console: LauncherScope {
            console_read: params.console_read,
            console_write: params.console_write,
        },
        fingerprint: params.fingerprint,
    };
    let task = call
        .control
        .servers
        .update(cx, |servers, cx| servers.control_add(request, cx));
    cx.spawn(async move |_| {
        let added = task.await?;
        Ok(json!({
            "core": {
                "id": added.uuid,
                "name": added.name,
                "new": added.new,
                "signed_in": added.signed_in,
                "trusted_for_mods": added.trusted,
            },
            "trust": if added.trusted { Value::Null } else { json!(format!("Not trusted for mods: installing a server's mods asks first. {} To trust it, the user decides and runs `cores trust <core> --yes`.", trust_explanation())) },
        }))
    })
}

#[derive(Deserialize)]
struct ClaimParams {
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    core: Option<String>,
}

/// "Connect as owner" for the core on this computer. The key and any owner account the
/// launcher made stay in the launcher: the reply names the file they are kept in.
pub fn cores_claim_local(call: Call, cx: &mut App) -> Reply {
    let params: ClaimParams = params!(call);
    let task = call.control.servers.update(cx, |servers, cx| {
        servers.claim_local(
            params.core.as_deref().unwrap_or_default(),
            params.folder.map(std::path::PathBuf::from),
            cx,
        )
    });
    cx.spawn(async move |_| {
        let claimed = task.await.map_err(|err| {
            use launcher_core::cores::local_claim::ClaimError;
            let code = match err {
                ClaimError::NoCore | ClaimError::FolderNotFound => codes::NOT_FOUND,
                ClaimError::Refused => codes::PERMISSION_DENIED,
                ClaimError::Core(ref err) => return core_error(err.clone()),
                _ => codes::FAILED,
            };
            api_error(code, crate::servers::local::claim_error(&err))
        })?;
        Ok(json!({
            "core": {
                "id": claimed.uuid,
                "name": claimed.name,
                "product": claimed.product.label(),
                "signed_in": true,
            },
            "owner_username": claimed.owner_username,
            "owner_created": claimed.owner_created,
            "credentials_file": claimed.credentials_file,
        }))
    })
}

pub fn cores_remove(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let index = tri!(core_param(&call, control.servers.read(cx)));
    let core = control.servers.read(cx).cores[index].saved.clone();
    tri!(call.confirm(format!(
        "removes the core {} and forgets this device's key for it",
        core.name
    )));
    control
        .servers
        .update(cx, |servers, cx| servers.remove(index, cx));
    ready(Ok(json!({ "removed": core.uuid, "name": core.name })))
}

pub fn cores_trust(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let index = tri!(core_param(&call, control.servers.read(cx)));
    let name = control.servers.read(cx).cores[index].saved.name.clone();
    tri!(call.confirm(format!(
        "trusts {name} for mods, so its servers' mods install without the advisory. {}",
        trust_explanation()
    )));
    control
        .servers
        .update(cx, |servers, _| servers.control_set_trusted(index, true));
    let servers = control.servers.read(cx);
    ready(Ok(
        json!({ "core": core_json(&servers.cores[index], servers, false) }),
    ))
}

pub fn cores_untrust(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let index = tri!(core_param(&call, control.servers.read(cx)));
    control
        .servers
        .update(cx, |servers, cx| servers.set_trusted(index, false, cx));
    let servers = control.servers.read(cx);
    ready(Ok(
        json!({ "core": core_json(&servers.cores[index], servers, false) }),
    ))
}

#[derive(Deserialize)]
struct CertificateParams {
    fingerprint: String,
}

pub fn cores_trust_certificate(call: Call, cx: &mut App) -> Reply {
    let params: CertificateParams = params!(call);
    let control = &call.control;
    let index = tri!(core_param(&call, control.servers.read(cx)));
    let untrusted = tri!(control.servers.update(cx, |servers, cx| {
        servers.control_trust_certificate(index, &params.fingerprint, cx)
    }));
    let servers = control.servers.read(cx);
    ready(Ok(json!({
        "core": core_json(&servers.cores[index], servers, false),
        "no_longer_trusted_for_mods": untrusted,
    })))
}

pub fn cores_sign_out(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let index = tri!(core_param(&call, control.servers.read(cx)));
    let key_id = control.servers.read(cx).cores[index].saved.key_id.clone();
    control
        .servers
        .update(cx, |servers, cx| servers.sign_out(index, cx));
    let servers = control.servers.read(cx);
    ready(Ok(json!({
        "core": core_json(&servers.cores[index], servers, false),
        "note": key_id.map(|id| format!("The key for this device (lsk_{id}_…) stays valid on the core until it expires. Revoke it under Settings → API keys in the dashboard to end it now.")),
    })))
}

/// The page's notice, to tell a new one from the old.
fn notice_of(servers: &Servers) -> Option<(SharedString, SharedString)> {
    servers.notice.as_ref().map(|notice| {
        let (title, message) = notice.text();
        (title.into(), message.into())
    })
}

fn notice_error(notice: &(SharedString, SharedString)) -> ErrorObject {
    api_error(codes::FAILED, format!("{}: {}", notice.0, notice.1))
}

/// Starts or stops a server and waits for the core's answer.
fn start_or_stop(call: Call, start: bool, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let index = tri!(control.servers.read(cx).control_core(&core));
    if !control.servers.read(cx).cores[index].signed_in() {
        return ready(Err(api_error(
            codes::NOT_SIGNED_IN,
            "Starting and stopping need this device signed in to the core: `cores add <address> --username <name>` (the password from stdin).",
        )));
    }
    let before = notice_of(control.servers.read(cx));
    let row = (core.clone(), server.uuid.clone());
    let name = server.name.clone();
    control.servers.update(cx, |servers, cx| {
        if start {
            servers.start(&core, server, false, cx)
        } else {
            servers.stop(&core, server, cx)
        }
    });
    let outbox = call.outbox.clone();
    cx.spawn(async move |cx| {
        poll_until(cx, Some(&outbox), Duration::from_millis(200), Duration::from_secs(60), |cx| {
            (!control.servers.read(cx).busy.contains_key(&row)).then_some(())
        })
        .await?;
        let after = cx.update(|cx| notice_of(control.servers.read(cx)));
        match after {
            Some(notice) if Some(&notice) != before.as_ref() => Err(notice_error(&notice)),
            _ => Ok(json!({ "server": row.1, "name": name, "core": row.0, if start { "started" } else { "stopping" }: true })),
        }
    })
}

pub fn start(call: Call, cx: &mut App) -> Reply {
    start_or_stop(call, true, cx)
}

pub fn stop(call: Call, cx: &mut App) -> Reply {
    start_or_stop(call, false, cx)
}

/// How to answer the page's questions while joining.
#[derive(Clone, Copy, Default)]
pub struct JoinOptions {
    /// The server's mods changed: update (true), play as is (false), or ask (none).
    pub update_mods: Option<bool>,
    /// Install from a core that isn't trusted for mods, this once.
    pub accept_advisory: bool,
}

#[derive(Deserialize)]
struct JoinParams {
    #[serde(default)]
    update_mods: Option<bool>,
    #[serde(default)]
    accept_advisory: bool,
}

fn advisory_json(prompt: &crate::servers::AdvisoryPrompt) -> Value {
    let advisory = &prompt.advisory;
    json!({
        "core_name": prompt.core_name,
        "server_name": prompt.server_name,
        "update": prompt.update,
        "on_modrinth": advisory.on_modrinth.iter().map(|m| json!({ "path": m.path, "project": m.project, "version": m.version })).collect::<Vec<_>>(),
        "uploaded": advisory.uploaded,
        "scripts": advisory.scripts,
        "configs": advisory.configs,
        "other": advisory.other,
        "unchecked": advisory.unchecked,
        "explanation": trust_explanation(),
    })
}

fn changes_json(prompt: &crate::servers::UpdatePrompt) -> Value {
    json!({
        "server": prompt.server.name,
        "instance": prompt.instance_id,
        "added": prompt.changes.added,
        "updated": prompt.changes.updated,
        "removed": prompt.changes.removed,
        "other": prompt.changes.other,
    })
}

/// Answers the advisory for `server_name` as `accept` says; the error when it isn't
/// accepted.
fn handle_advisory(
    servers: &mut Servers,
    server_name: &str,
    accept: bool,
    cx: &mut gpui::Context<Servers>,
) -> Option<Result<(), ErrorObject>> {
    let prompt = servers.advisory.current()?;
    if prompt.server_name != server_name {
        return None;
    }
    if accept {
        servers.answer_advisory(AdvisoryAnswer::InstallOnce, cx);
        return Some(Ok(()));
    }
    let data = advisory_json(prompt);
    servers.answer_advisory(AdvisoryAnswer::Cancel, cx);
    Some(Err(api_error(
        codes::UNTRUSTED_CORE,
        format!(
            "{} isn't trusted for mods, so its mods need the user's decision. Nothing was installed. Show the user the advisory; then either pass --accept-advisory to install them this once, or trust the core with `cores trust <core> --yes`.",
            data["core_name"].as_str().unwrap_or_default()
        ),
    )
    .with_data(json!({ "advisory": data }))))
}

/// Play on an instance linked to a server: what its Play button does. Follows the page
/// until the game starts, answering its questions as `options` says.
pub fn join_instance(call: Call, instance: Instance, options: JoinOptions, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let Some(link) = control.servers.read(cx).instance_link(&instance.id) else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!("{} isn't linked to a server.", instance.name),
        )));
    };
    if !link.core_known {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!(
                "The core {} was made for isn't in Lodestone Launcher any more. Add it with `cores add`, or play without joining (`play launch --no-join`).",
                instance.name
            ),
        )));
    }
    let seq = control.notices.borrow().last_seq();
    let before = notice_of(control.servers.read(cx));
    let id = instance.id.clone();
    control
        .servers
        .update(cx, |servers, cx| servers.play_instance(&id, cx));
    let outbox = call.outbox.clone();
    let server_name = link.server_name.clone();
    let call = Rc::new(call);
    cx.spawn(async move |cx| {
        // until the game starts preparing, or the page stops with a question or error
        poll_until(cx, Some(&outbox), Duration::from_millis(200), Duration::from_secs(20 * 60), |cx| {
            if control.instances.read(cx).is_active(&id) {
                return Some(Ok(()));
            }
            let answered = control.servers.update(cx, |servers, cx| {
                if let Some(prompt) = servers.prompt.current()
                    && prompt.instance_id == id
                {
                    return Some(match options.update_mods {
                        Some(update) => {
                            servers.answer_prompt(update, cx);
                            Ok(())
                        }
                        None => {
                            let changes = changes_json(prompt);
                            servers.cancel_prompt(cx);
                            Err(api_error(
                                codes::MODS_CHANGED,
                                "The server's mods changed since this instance was made. Ask the user: pass --update-mods to bring the instance in step, or --skip-update to play as it is.",
                            )
                            .with_data(json!({ "changes": changes })))
                        }
                    });
                }
                handle_advisory(servers, &server_name, options.accept_advisory, cx)
            });
            match answered {
                Some(Err(err)) => return Some(Err(err)),
                Some(Ok(())) => return None,
                None => {}
            }
            let servers = control.servers.read(cx);
            let busy = servers.instance_busy(&id).is_some();
            match notice_of(servers) {
                Some(notice) if !busy && Some(&notice) != before.as_ref() => Some(Err(notice_error(&notice))),
                _ => None,
            }
        })
        .await??;
        let reply = cx.update(|cx| {
            let instance = control.instance(&id, cx)?;
            Ok::<_, ErrorObject>(start_launch_job(&call, &instance, seq, Some(server_name.clone()), cx))
        })?;
        reply.await
    })
}

pub fn join(call: Call, cx: &mut App) -> Reply {
    let params: JoinParams = params!(call);
    let control = call.control.clone();
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let Some(instance_id) = control
        .servers
        .read(cx)
        .linked_instance(&core, &server.uuid)
        .cloned()
    else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!(
                "No instance is linked to {}. Make one with `servers create-instance`, or link one with `servers link`.",
                server.name
            ),
        )));
    };
    let instance = tri!(control.instance(&instance_id, cx));
    if control.instances.read(cx).is_active(&instance.id) {
        return ready(Err(api_error(
            codes::BUSY,
            format!("{} is already running.", instance.name),
        )));
    }
    join_instance(
        call,
        instance,
        JoinOptions {
            update_mods: params.update_mods,
            accept_advisory: params.accept_advisory,
        },
        cx,
    )
}

#[derive(Deserialize)]
struct CreateParams {
    #[serde(default)]
    accept_advisory: bool,
}

pub fn create_instance(call: Call, cx: &mut App) -> Reply {
    let params: CreateParams = params!(call);
    let control = call.control.clone();
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let before_link = control
        .servers
        .read(cx)
        .linked_instance(&core, &server.uuid)
        .cloned();
    let before = notice_of(control.servers.read(cx));
    let row = (core.clone(), server.uuid.clone());
    let name = server.name.clone();
    let label = format!("Creating an instance for {name}");
    let core_id = core.clone();
    let server_id = server.uuid.clone();
    control
        .servers
        .update(cx, |servers, cx| servers.create_instance(&core, server, cx));
    let watch = control.clone();
    let work = cx.spawn(async move |cx| -> ApiResult {
        let outcome = poll_until(
            cx,
            None,
            Duration::from_millis(250),
            Duration::from_secs(6 * 3600),
            |cx| {
                if let Some(answer) = watch.servers.update(cx, |servers, cx| {
                    handle_advisory(servers, &name, params.accept_advisory, cx)
                }) {
                    if let Err(err) = answer {
                        return Some(Err(err));
                    }
                    return None;
                }
                let servers = watch.servers.read(cx);
                let link = servers.linked_instance(&core_id, &server_id).cloned();
                let busy = servers.busy.contains_key(&row) || servers.advisory.is_open();
                if let Some(id) = link
                    && Some(&id) != before_link.as_ref()
                    && !busy
                {
                    return Some(Ok(id));
                }
                match notice_of(servers) {
                    Some(notice) if !busy && Some(&notice) != before.as_ref() => {
                        Some(Err(notice_error(&notice)))
                    }
                    _ => None,
                }
            },
        )
        .await??;
        Ok(cx.update(|cx| {
            let servers = watch.servers.read(cx);
            json!({
                "instance": outcome,
                "notice": servers.notice.as_ref().map(|notice| {
                    let (title, message) = notice.text();
                    json!({ "title": title, "message": message })
                }),
            })
        }))
    });
    let job = control.start_job(JobSpec::new("create_instance", label), work, cx);
    control.reply_job(&call, job, cx)
}

pub fn link(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let id = instance.id.clone();
    let name = server.name.clone();
    control.servers.update(cx, |servers, cx| {
        servers.control_link(&core, &server, &id, cx)
    });
    let linked = control.servers.read(cx).instance_link(&id);
    match linked {
        Some(link) => ready(Ok(
            json!({ "instance": id, "server": name, "core_name": link.core_name }),
        )),
        None => ready(Err(api_error(
            codes::FAILED,
            notice_of(control.servers.read(cx))
                .map(|n| n.1.to_string())
                .unwrap_or_else(|| "Couldn't link the instance.".into()),
        ))),
    }
}

pub fn unlink(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let instance = tri!(control.instance_param(&call, cx));
    if control
        .servers
        .read(cx)
        .instance_link(&instance.id)
        .is_none()
    {
        return ready(Err(api_error(
            codes::CONFLICT,
            format!("{} isn't linked to a server.", instance.name),
        )));
    }
    let result = control
        .servers
        .update(cx, |servers, cx| servers.unlink_instance(&instance.id, cx));
    tri!(failed(result));
    ready(Ok(json!({ "instance": instance.id, "unlinked": true })))
}

#[derive(Deserialize)]
struct HostParams {
    username: String,
    password: String,
    #[serde(default)]
    name: Option<String>,
    /// MB: 2048, 4096, 6144 or 8192.
    #[serde(default)]
    max_ram: Option<u32>,
    /// Starting the server accepts Minecraft's EULA; the dialog has a box for it.
    #[serde(default)]
    accept_eula: bool,
}

pub fn host(call: Call, cx: &mut App) -> Reply {
    let params: HostParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let index = tri!(core_param(&call, control.servers.read(cx)));
    if !params.accept_eula {
        return ready(Err(api_error(
            codes::EULA_REQUIRED,
            format!("Hosting a Minecraft server accepts Minecraft's EULA ({EULA_URL}). Only the user can agree to it: ask them, then pass --accept-eula."),
        )
        .with_data(json!({ "eula": EULA_URL }))));
    }
    let max_ram = params
        .max_ram
        .unwrap_or(launcher_core::cores::hosting::DEFAULT_MAX_RAM);
    if !MEMORY_CHOICES.contains(&max_ram) {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            format!("The server's memory is one of {MEMORY_CHOICES:?} MB."),
        )));
    }
    let name = params
        .name
        .clone()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| instance.name.clone());
    let core_uuid = control.servers.read(cx).cores[index].saved.uuid.clone();
    let stage = Arc::new(Mutex::new(String::new()));
    let request = HostCore {
        core_uuid,
        username: params.username.clone(),
        password: Secret::new(params.password),
        name: name.clone(),
        max_ram,
    };
    let job_stage = stage.clone();
    let id = instance.id.clone();
    let work = control.servers.update(cx, |servers, cx| {
        servers.control_host(&id, request, stage, cx)
    });
    let job = control.start_job(
        JobSpec::new("host", format!("Hosting {} as {name}", instance.name))
            .instance(&instance)
            .stage(job_stage),
        work,
        cx,
    );
    control.reply_job(&call, job, cx)
}

pub fn resource_pack(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&instance, cx));
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let index = tri!(control.servers.read(cx).control_core(&core));
    let id = instance.id.clone();
    let task = control.servers.update(cx, |servers, cx| {
        servers.control_download_pack(index, server, &id, cx)
    });
    cx.spawn(async move |_| {
        let file = task.await?;
        Ok(json!({ "instance": id, "file": file, "note": "Saved in the instance's resource packs; turn it on with `content enable`." }))
    })
}

#[derive(Deserialize)]
struct WorldParams {
    username: String,
    password: String,
}

pub fn download_world(call: Call, cx: &mut App) -> Reply {
    let params: WorldParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&instance, cx));
    let (core, server) = tri!(find_server(&call, control.servers.read(cx)));
    let index = tri!(control.servers.read(cx).control_core(&core));
    let progress = Arc::new(Downloads::default());
    let stage = Arc::new(Mutex::new(String::new()));
    let cancel = CancelToken::new();
    let id = instance.id.clone();
    let server_name = server.name.clone();
    let task = control.servers.update(cx, |servers, cx| {
        servers.control_download_world(
            index,
            server,
            &id,
            params.username.clone(),
            Secret::new(params.password),
            progress.clone(),
            stage.clone(),
            cancel.clone(),
            cx,
        )
    });
    let refresh = control.clone();
    let instance_name = instance.name.clone();
    let work = cx.spawn(async move |cx| {
        let folder = task.await?;
        cx.update(|cx| {
            if let Some(view) = refresh.worlds.clone()
                && refresh.instances.read(cx).selected().is_some_and(|s| s.id == id)
            {
                view.update(cx, |view, cx| view.reload(cx));
            }
        });
        Ok(json!({
            "instance": id,
            "world": folder,
            "message": format!("Imported as the world \"{folder}\" in {instance_name}. It's a copy: changes don't go back to the server."),
        }))
    });
    let job = control.start_job(
        JobSpec::new(
            "world_download",
            format!("Downloading {server_name}'s world"),
        )
        .instance(&instance)
        .progress(Progress::Files(progress))
        .stage(stage)
        .cancel(Cancel::Token(cancel)),
        work,
        cx,
    );
    control.reply_job(&call, job, cx)
}

pub fn prompts_list(call: Call, cx: &mut App) -> Reply {
    let servers = call.control.servers.read(cx);
    let mut prompts = Vec::new();
    if let Some(prompt) = servers.prompt.current() {
        prompts.push(json!({ "prompt": "update", "answers": ["update", "play", "cancel"], "details": changes_json(prompt) }));
    }
    if let Some(prompt) = servers.advisory.current() {
        prompts.push(json!({ "prompt": "advisory", "answers": ["install_once", "trust", "cancel"], "details": advisory_json(prompt) }));
    }
    if let Some(prompt) = servers.cert_prompt.current() {
        prompts.push(json!({
            "prompt": "certificate",
            "answers": ["trust", "reject"],
            "details": {
                "address": prompt.address,
                "host": prompt.host,
                "name": prompt.name,
                "fingerprint": prompt.presented,
                "previous": prompt.previous,
                "help": crate::servers::FINGERPRINT_HELP,
            },
        }));
    }
    if let Some(prompt) = servers.trust_prompt.current() {
        prompts.push(json!({
            "prompt": "trust",
            "answers": ["trust", "no"],
            "details": { "core": prompt.core, "name": prompt.name, "address": prompt.address, "explanation": trust_explanation() },
        }));
    }
    ready(Ok(json!({ "prompts": prompts })))
}

#[derive(Deserialize)]
struct AnswerParams {
    prompt: String,
    answer: String,
    /// For a certificate: the fingerprint the user compared.
    #[serde(default)]
    fingerprint: Option<String>,
}

pub fn prompts_answer(call: Call, cx: &mut App) -> Reply {
    let params: AnswerParams = params!(call);
    let control = call.control.clone();
    let servers = control.servers.read(cx);
    let open = match params.prompt.as_str() {
        "update" => servers.prompt.is_open(),
        "advisory" => servers.advisory.is_open(),
        "certificate" => servers.cert_prompt.is_open(),
        "trust" => servers.trust_prompt.is_open(),
        other => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("No prompt \"{other}\": update, advisory, certificate or trust."),
            )));
        }
    };
    if !open {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!("The window isn't asking a {} question.", params.prompt),
        )));
    }
    let bad = || {
        api_error(
            codes::INVALID_PARAMS,
            format!(
                "\"{}\" doesn't answer the {} prompt; `prompts list` gives the answers.",
                params.answer, params.prompt
            ),
        )
    };
    match (params.prompt.as_str(), params.answer.as_str()) {
        ("update", answer @ ("update" | "play")) => {
            let update = answer == "update";
            control
                .servers
                .update(cx, |s, cx| s.answer_prompt(update, cx));
        }
        ("update", "cancel") => control.servers.update(cx, |s, cx| s.cancel_prompt(cx)),
        ("advisory", "install_once") => control.servers.update(cx, |s, cx| {
            s.answer_advisory(AdvisoryAnswer::InstallOnce, cx)
        }),
        ("advisory", "trust") => {
            tri!(call.confirm(format!(
                "trusts the core for mods from now on. {}",
                trust_explanation()
            )));
            control.servers.update(cx, |s, cx| {
                s.answer_advisory(AdvisoryAnswer::TrustAndInstall, cx)
            })
        }
        ("advisory", "cancel") => control
            .servers
            .update(cx, |s, cx| s.answer_advisory(AdvisoryAnswer::Cancel, cx)),
        ("certificate", "trust") => {
            let presented = servers
                .cert_prompt
                .current()
                .map(|p| p.presented.clone())
                .unwrap_or_default();
            let given = params.fingerprint.as_deref().unwrap_or_default();
            if !crate::servers::control::same_fingerprint(given, &presented) {
                return ready(Err(api_error(
                    codes::UNTRUSTED_CERTIFICATE,
                    "Trusting a certificate needs --fingerprint with the fingerprint the user compared, and it must match the one presented.",
                )
                .with_data(json!({ "fingerprint": presented }))));
            }
            control
                .servers
                .update(cx, |s, cx| s.answer_certificate(true, cx))
        }
        ("certificate", "reject") => control
            .servers
            .update(cx, |s, cx| s.answer_certificate(false, cx)),
        ("trust", "trust") => {
            tri!(call.confirm(format!("trusts the core for mods. {}", trust_explanation())));
            control.servers.update(cx, |s, cx| s.answer_trust(true, cx))
        }
        ("trust", "no") => control
            .servers
            .update(cx, |s, cx| s.answer_trust(false, cx)),
        _ => return ready(Err(bad())),
    }
    ready(Ok(
        json!({ "prompt": params.prompt, "answer": params.answer }),
    ))
}
