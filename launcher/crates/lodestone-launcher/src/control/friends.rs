//! Friends' servers (Lodestone Connect): joining a server a friend shares with an invite
//! link, as the join dialog does.
//!
//! `friends.join` opens the dialog for a link (or a server joined before), waits for the
//! tunnel, and answers with the server's card; with `play` it goes on as Play does,
//! keeping the mod advisory (`accept_advisory`) and the mods-changed question
//! (`update_mods`) as `servers.join` keeps them. `friends.open` only opens the dialog: a
//! second launcher on Windows hands the link it was started for to the running one
//! with it. Invite links are secrets (anyone with one can join): no method returns one.

use std::{rc::Rc, time::Duration};

use gpui::App;
use launcher_control::{ErrorObject, codes};
use launcher_core::cores::{
    connect::{ConnectionKind, Invite, JoinError},
    friends::InstanceMatch,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ApiResult, Call, Reply, api_error,
    instances::{params, start_launch_job, tri},
    poll_until, ready,
};
use crate::servers::{
    AdvisoryAnswer, Servers,
    friends::{FriendEntry, FriendStatus, JoinStage, join_error_text},
};

fn kind_name(kind: ConnectionKind) -> &'static str {
    match kind {
        ConnectionKind::SameComputer => "same_computer",
        ConnectionKind::LocalNetwork => "local_network",
        ConnectionKind::Ipv6 => "ipv6",
        ConnectionKind::Internet => "internet",
    }
}

/// A join error as an API error, with what to do in its message.
pub fn join_error(error: &JoinError) -> ErrorObject {
    let (reason, code) = match error {
        JoinError::Ended => ("link_ended", codes::NOT_FOUND),
        JoinError::HostOffline => ("host_offline", codes::UNAVAILABLE),
        JoinError::NoPath => ("no_path", codes::UNAVAILABLE),
        JoinError::WrongHost => ("wrong_host", codes::UNTRUSTED_CERTIFICATE),
        JoinError::Offline => ("offline", codes::UNAVAILABLE),
        JoinError::Unreachable(_) => ("service_unreachable", codes::UNAVAILABLE),
        JoinError::Failed(_) => ("failed", codes::FAILED),
    };
    let (title, body) = join_error_text(error);
    api_error(code, format!("{title}. {body}")).with_data(json!({ "reason": reason }))
}

fn friend_json(entry: &FriendEntry, servers: &Servers) -> Value {
    let saved = &entry.saved;
    let playing = servers
        .friends
        .sessions
        .get(&saved.id)
        .and_then(|s| s.playing.clone());
    let status = match &entry.status {
        FriendStatus::Unknown => json!({ "state": "unknown" }),
        FriendStatus::Checking => json!({ "state": "checking" }),
        FriendStatus::Online { server, kind, rtt } => json!({
            "state": "online",
            "server_state": server.state,
            "players": server.player_count,
            "max_players": server.max_player_count,
            "connection": kind_name(*kind),
            "rtt_ms": rtt.as_millis() as u64,
        }),
        FriendStatus::Offline(error) => json!({
            "state": "offline",
            "error": join_error(error),
        }),
    };
    json!({
        "id": saved.id,
        "name": saved.name,
        "core_name": saved.core_name,
        "link": saved.link,
        "game_version": saved.game_version,
        "loader": saved.loader,
        "loader_version": saved.loader_version,
        "modpack": saved.modpack,
        "trusted": saved.is_trusted(),
        "instance": saved.instance_id,
        "added_at": saved.added_at,
        "last_joined": saved.last_joined,
        "status": status,
        "playing": playing,
    })
}

/// `friends.list`.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let servers = call.control.servers.read(cx);
    ready(Ok(json!({
        "friends": servers
            .friends
            .list
            .iter()
            .map(|entry| friend_json(entry, servers))
            .collect::<Vec<_>>(),
    })))
}

/// A saved friend's server by id or name.
fn find_friend(servers: &Servers, reference: &str) -> Result<String, ErrorObject> {
    let list = &servers.friends.list;
    if let Some(entry) = list.iter().find(|f| f.saved.id == reference) {
        return Ok(entry.saved.id.clone());
    }
    let named: Vec<&FriendEntry> = list
        .iter()
        .filter(|f| f.saved.name.eq_ignore_ascii_case(reference))
        .collect();
    match named.as_slice() {
        [entry] => Ok(entry.saved.id.clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("No friend's server is called \"{reference}\". See `friends list`."),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("Several friends' servers are called \"{reference}\". Use its id."),
        )),
    }
}

#[derive(Deserialize)]
struct OpenParams {
    link: String,
}

/// `friends.open`: the join dialog for an invite link, or the Add friend dialog for a
/// friend link, as opening the link does.
pub fn open(call: Call, cx: &mut App) -> Reply {
    let params: OpenParams = params!(call);
    // a friend link (`lodestone://friend?link=…`) opens the Add friend dialog
    if launcher_core::cores::friends::FriendLink::parse(&params.link).is_ok() {
        let servers = call.control.servers.clone();
        return ready(super::friend_list::open_link(params.link, cx, &servers));
    }
    if Invite::parse(&params.link).is_err() {
        return ready(Err(not_an_invite()));
    }
    call.control
        .servers
        .update(cx, |servers, cx| servers.open_join(Some(params.link), cx));
    ready(Ok(json!({ "opened": true })))
}

fn not_an_invite() -> ErrorObject {
    api_error(
        codes::INVALID_PARAMS,
        "That isn't a Lodestone invite link: it should start with https://connect.lodestone.cc/j/ (or lodestone://join?invite=).",
    )
}

#[derive(Deserialize)]
struct JoinParams {
    #[serde(default)]
    link: Option<String>,
    /// A friend's server joined before, by id or name.
    #[serde(default)]
    friend: Option<String>,
    #[serde(default = "yes")]
    play: bool,
    #[serde(default)]
    accept_advisory: bool,
    #[serde(default)]
    update_mods: Option<bool>,
}

fn yes() -> bool {
    true
}

/// The server's card, as the dialog shows it.
fn card_json(servers: &Servers, id: &str, cx: &App) -> Option<Value> {
    let session = servers.friends.sessions.get(id)?;
    let shared = session.shared();
    let instance = servers.friend_instance(id, cx).map(|found| {
        let (how, instance) = match &found {
            InstanceMatch::Linked(id) => ("linked", Some(id.clone())),
            InstanceMatch::Matching(id) => ("matching", Some(id.clone())),
            InstanceMatch::Create => ("create", None),
        };
        json!({ "match": how, "id": instance })
    });
    Some(json!({
        "id": id,
        "name": shared.name,
        "core_name": shared.core_name,
        "game_version": shared.game_version,
        "loader": shared.loader,
        "loader_version": shared.loader_version,
        "modpack": shared.modpack,
        "state": shared.state,
        "players": shared.player_count,
        "max_players": shared.max_player_count,
        "connection": kind_name(session.kind()),
        "rtt_ms": session.rtt().as_millis() as u64,
        "instance": instance,
    }))
}

/// What the dialog says now: still connecting (`None`), the server's card, or an error.
fn dialog_state(servers: &Servers) -> Option<Result<String, ErrorObject>> {
    let Some(prompt) = servers.friends.join.current() else {
        return Some(Err(api_error(
            codes::CANCELLED,
            "The join dialog was closed in the window.",
        )));
    };
    match &prompt.stage {
        JoinStage::Connecting { .. } => None,
        JoinStage::Ready { id, .. } => Some(Ok(id.clone())),
        JoinStage::Failed { error, .. } => Some(Err(join_error(error))),
        JoinStage::Paste { error, .. } => Some(Err(api_error(
            codes::INVALID_PARAMS,
            error
                .as_ref()
                .map_or_else(|| "No invite link.".to_string(), |e| e.to_string()),
        ))),
        JoinStage::Preparing { id } => Some(Ok(id.clone())),
    }
}

/// `friends.join`.
pub fn join(call: Call, cx: &mut App) -> Reply {
    let params: JoinParams = params!(call);
    let control = call.control.clone();
    match (&params.link, &params.friend) {
        (Some(link), None) => {
            let Ok(invite) = Invite::parse(link) else {
                return ready(Err(not_an_invite()));
            };
            control
                .servers
                .update(cx, |servers, cx| servers.connect_invite(invite, false, cx));
        }
        (None, Some(friend)) => {
            let id = tri!(find_friend(control.servers.read(cx), friend));
            control
                .servers
                .update(cx, |servers, cx| servers.open_friend(&id, false, cx));
        }
        _ => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "Give either \"link\" (an invite link) or \"friend\" (a server joined before).",
            )));
        }
    }
    let outbox = call.outbox.clone();
    let call = Rc::new(call);
    cx.spawn(async move |cx| -> ApiResult {
        // the tunnel takes a few seconds; the dialog says when it is open
        let id = poll_until(
            cx,
            Some(&outbox),
            Duration::from_millis(200),
            Duration::from_secs(120),
            |cx| dialog_state(control.servers.read(cx)),
        )
        .await??;
        let card = cx
            .update(|cx| card_json(control.servers.read(cx), &id, cx))
            .ok_or_else(|| api_error(codes::FAILED, "The tunnel closed."))?;
        if !params.play {
            return Ok(json!({ "server": card }));
        }
        let seq = control.notices.borrow().last_seq();
        let server_name = card["name"].as_str().unwrap_or_default().to_string();
        cx.update(|cx| {
            control
                .servers
                .update(cx, |servers, cx| servers.friend_play(cx))
        });
        // until the game starts, or the dialog stops with a question or an error
        let instance = poll_until(
            cx,
            Some(&outbox),
            Duration::from_millis(200),
            Duration::from_secs(20 * 60),
            |cx| {
                let playing = control
                    .servers
                    .read(cx)
                    .friends
                    .sessions
                    .get(&id)
                    .and_then(|s| s.playing.clone());
                if let Some(instance) = playing {
                    return Some(Ok(instance));
                }
                let answered = control.servers.update(cx, |servers, cx| {
                    answer_questions(servers, &server_name, &params, cx)
                });
                if let Some(Err(err)) = answered {
                    return Some(Err(err));
                }
                let servers = control.servers.read(cx);
                match servers.friends.join.current().map(|p| &p.stage) {
                    Some(JoinStage::Ready {
                        error: Some((title, body)),
                        ..
                    }) => Some(Err(api_error(codes::FAILED, format!("{title}: {body}")))),
                    Some(JoinStage::Failed { error, .. }) => Some(Err(join_error(error))),
                    None if !servers.advisory.is_open() && !servers.prompt.is_open() => {
                        Some(Err(api_error(
                            codes::CANCELLED,
                            "The join dialog was closed in the window.",
                        )))
                    }
                    _ => None,
                }
            },
        )
        .await??;
        let reply = cx.update(|cx| {
            let instance = control.instance(&instance, cx)?;
            Ok::<_, ErrorObject>(start_launch_job(
                &call,
                &instance,
                seq,
                Some(server_name.clone()),
                cx,
            ))
        })?;
        let mut result = reply.await?;
        if let Some(object) = result.as_object_mut() {
            object.insert("server".into(), card);
        }
        Ok(result)
    })
}

/// The join dialog's error, while it shows one: for Play on an instance made for a
/// friend's server (`servers.join`, `play.launch`).
pub fn dialog_error(servers: &Servers) -> Option<ErrorObject> {
    match &servers.friends.join.current()?.stage {
        JoinStage::Failed { error, .. } => Some(join_error(error)),
        JoinStage::Ready {
            error: Some((title, body)),
            ..
        } => Some(api_error(codes::FAILED, format!("{title}: {body}"))),
        _ => None,
    }
}

/// Answers the advisory and the mods-changed question for the friend's server, as the
/// call's flags say.
fn answer_questions(
    servers: &mut Servers,
    server_name: &str,
    params: &JoinParams,
    cx: &mut gpui::Context<Servers>,
) -> Option<Result<(), ErrorObject>> {
    if let Some(prompt) = servers.prompt.current()
        && prompt.server.name == server_name
    {
        return Some(match params.update_mods {
            Some(update) => {
                servers.answer_prompt(update, cx);
                Ok(())
            }
            None => {
                servers.cancel_prompt(cx);
                Err(api_error(
                    codes::MODS_CHANGED,
                    "The server's mods changed since this instance was made. Ask the user: pass --update-mods to bring the instance in step, or --skip-update to play as it is.",
                ))
            }
        });
    }
    let prompt = servers.advisory.current()?;
    if prompt.server_name != server_name {
        return None;
    }
    if params.accept_advisory {
        servers.answer_advisory(AdvisoryAnswer::InstallOnce, cx);
        return Some(Ok(()));
    }
    let core = prompt.core_name.clone();
    let data = json!({
        "core_name": prompt.core_name,
        "server_name": prompt.server_name,
        "on_modrinth": prompt.advisory.on_modrinth.iter().map(|m| json!({ "path": m.path, "project": m.project, "version": m.version })).collect::<Vec<_>>(),
        "uploaded": prompt.advisory.uploaded,
        "scripts": prompt.advisory.scripts,
        "configs": prompt.advisory.configs,
        "other": prompt.advisory.other,
        "unchecked": prompt.advisory.unchecked,
    });
    servers.answer_advisory(AdvisoryAnswer::Cancel, cx);
    Some(Err(api_error(
        codes::UNTRUSTED_CORE,
        format!(
            "{server_name} (shared by {core}) has mods, which run with your account's access. Nothing was installed. Show the user the advisory; then pass --accept-advisory to install them this once."
        ),
    )
    .with_data(json!({ "advisory": data }))))
}

#[derive(Deserialize)]
struct RemoveParams {
    friend: String,
}

/// `friends.remove`.
pub fn remove(call: Call, cx: &mut App) -> Reply {
    let params: RemoveParams = params!(call);
    let control = &call.control;
    let id = tri!(find_friend(control.servers.read(cx), &params.friend));
    let name = control
        .servers
        .read(cx)
        .friends
        .get(&id)
        .map(|f| f.saved.name.clone())
        .unwrap_or_default();
    tri!(call.confirm(format!(
        "removes {name} from your friends' servers and forgets its invite link (instances made for it stay)"
    )));
    control
        .servers
        .update(cx, |servers, cx| servers.remove_friend(&id, cx));
    ready(Ok(json!({ "removed": id, "name": name })))
}
