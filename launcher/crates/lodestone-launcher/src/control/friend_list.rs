//! The friend list (launcher/docs/friends.md): this person's name and safety code,
//! friend links and their answers, friends, and the servers they share.
//!
//! Every method goes through the same `Servers` operations as the window, so the window
//! shows each change at once. Adding someone always needs confirmation: answering a
//! link and accepting an answer need `"confirm": true` (after showing the user the name
//! and safety code, from `friend_list.preview_link` or `friend_list.replies`), and
//! `answer_link` can also check the safety code it expects. Ticking "Codes match"
//! (`verify`) needs it too, and only the user can say the codes matched.
//!
//! The read-only methods (`list`, `links`) open the friend list without making
//! anything, and answer "no friend list yet" when there's none; they start no refresh
//! and reach no one. `me` and the methods that start a friendship make the person key
//! on first use.
//!
//! A friend link is returned by `create_link` so it can be sent: it lets its holder ask
//! to be this person's friend, nothing more (the person still accepts the answer). No
//! method returns a key's secret part or a pair secret.

use gpui::{App, AsyncApp, Entity, Task};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    cores::connect::SharedServer,
    friend_list::{Friend, now, ops::LinkReply},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ApiResult, Call, Reply, api_error,
    instances::{params, tri},
};
use crate::servers::{
    Servers,
    friend_list::{FindError, RefreshReport, Snapshot, find_friend, hex, status_name},
};

fn failed(message: String) -> ErrorObject {
    api_error(codes::FAILED, message)
}

fn friend_json(friend: &Friend, servers: &Servers) -> Value {
    let list = &servers.friend_list;
    let cores: Vec<Value> = friend
        .joinable_cores(now())
        .iter()
        .map(|core| {
            let fingerprint = hex(&core.fingerprint);
            let online = list
                .presence
                .get(&(friend.key.clone(), fingerprint.clone()))
                .copied();
            json!({ "fingerprint": fingerprint, "name": core.name, "online": online })
        })
        .collect();
    json!({
        "key": friend.key,
        "name": friend.name,
        "nickname": friend.nickname,
        "display_name": friend.display_name(),
        "status": status_name(friend.status),
        "verified": friend.verified,
        "hidden": friend.hidden,
        "safety_code": friend.safety_code(),
        "added_at": friend.added_at,
        "online": list.online(friend),
        "cores": cores,
        "same_name_as": list.snapshot.name_clashes.get(&friend.key).cloned().unwrap_or_default(),
    })
}

/// What a read-only method answers when there's no friend list.
const NO_LIST_NOTE: &str = "No friend list yet: choose a name first (`friend-list set-name`).";

fn no_list() -> ErrorObject {
    api_error(codes::NOT_FOUND, NO_LIST_NOTE)
}

fn reply_json(reply: &LinkReply) -> Value {
    json!({
        "code": reply.code,
        "key": reply.key,
        "name": reply.name,
        "safety_code": reply.safety_code,
        "answers": reply.answers,
        "someone_else_may_have_the_link": reply.answers > 1,
        "same_name_as": reply.same_name_as,
    })
}

fn me_json(snapshot: &Snapshot) -> Value {
    json!({
        "name": snapshot.book.name,
        "safety_code": snapshot.safety_code,
        "key": snapshot.key,
        "friends": snapshot.book.friends.len(),
        "links": snapshot.book.links.len(),
        "cores": snapshot
            .book
            .my_cores
            .iter()
            .map(|c| json!({ "fingerprint": c.fingerprint, "name": c.name }))
            .collect::<Vec<_>>(),
    })
}

/// Runs an operation of `Servers` and answers with `answer` of its result.
fn run<T: 'static>(
    call: &Call,
    cx: &mut App,
    op: impl FnOnce(&mut Servers, &mut gpui::Context<Servers>) -> Task<Result<T, String>>,
    answer: impl FnOnce(T, &mut AsyncApp, &Entity<Servers>) -> ApiResult + 'static,
) -> Reply {
    let servers = call.control.servers.clone();
    let task = servers.update(cx, op);
    cx.spawn(async move |cx| {
        let value = task.await.map_err(failed)?;
        answer(value, cx, &servers)
    })
}

/// Opens the list, finds the friend `reference`, then runs `op` for their key.
fn with_friend<T: 'static>(
    call: &Call,
    cx: &mut App,
    reference: String,
    op: impl FnOnce(&mut Servers, String, &mut gpui::Context<Servers>) -> Task<Result<T, String>>
    + 'static,
    answer: impl FnOnce(T, String, &mut AsyncApp, &Entity<Servers>) -> ApiResult + 'static,
) -> Reply {
    let servers = call.control.servers.clone();
    let opening = servers.update(cx, |servers, cx| servers.friend_peek(cx));
    cx.spawn(async move |cx| {
        let snapshot = opening.await.map_err(failed)?.ok_or_else(no_list)?;
        let key = find_friend(&snapshot.book, &reference).map_err(|err| match err {
            FindError::NotFound => api_error(
                codes::NOT_FOUND,
                format!("No friend is called \"{reference}\". See `friend-list list`."),
            ),
            FindError::Ambiguous => api_error(
                codes::AMBIGUOUS,
                format!("Several friends match \"{reference}\". Use their key."),
            ),
        })?;
        let task = cx.update(|cx| servers.update(cx, |servers, cx| op(servers, key.clone(), cx)));
        let value = task.await.map_err(failed)?;
        answer(value, key, cx, &servers)
    })
}

fn friend_answer(key: &str, cx: &mut AsyncApp, servers: &Entity<Servers>) -> ApiResult {
    cx.update(|cx| {
        let servers = servers.read(cx);
        servers
            .friend_list
            .friend(key)
            .map(|friend| json!({ "friend": friend_json(friend, servers) }))
            .ok_or_else(|| api_error(codes::NOT_FOUND, "That friend is gone."))
    })
}

/// `friend_list.me`: this person's name and safety code (opening the list, which
/// makes the person key on first use).
pub fn me(call: Call, cx: &mut App) -> Reply {
    run(
        &call,
        cx,
        |servers, cx| servers.friend_me(cx),
        |snapshot, _, _| Ok(me_json(&snapshot)),
    )
}

#[derive(Deserialize)]
struct NameParams {
    name: String,
}

/// `friend_list.set_name`.
pub fn set_name(call: Call, cx: &mut App) -> Reply {
    let params: NameParams = params!(call);
    run(
        &call,
        cx,
        move |servers, cx| servers.friend_set_name(params.name, cx),
        |(), cx, servers| Ok(cx.update(|cx| me_json(&servers.read(cx).friend_list.snapshot))),
    )
}

/// `friend_list.create_link`: a friend link to send.
pub fn create_link(call: Call, cx: &mut App) -> Reply {
    run(
        &call,
        cx,
        |servers, cx| servers.friend_create_link(cx),
        |made, _, _| {
            Ok(json!({
                "link": made.link,
                "expires_at": made.expires,
                "note": "Works once, for 24 hours. Send it only to the friend: anyone with it can ask to be your friend.",
            }))
        },
    )
}

/// `friend_list.links`: this person's open friend links (their mailbox codes). Makes
/// nothing.
pub fn links(call: Call, cx: &mut App) -> Reply {
    let servers = call.control.servers.clone();
    let opening = servers.update(cx, |servers, cx| servers.friend_peek(cx));
    cx.spawn(async move |_| {
        let Some(snapshot) = opening.await.map_err(failed)? else {
            return Ok(json!({ "links": [], "note": NO_LIST_NOTE }));
        };
        Ok(json!({
            "links": snapshot
                .book
                .links
                .iter()
                .map(|l| json!({ "code": l.code, "expires_at": l.expires }))
                .collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct CodeParams {
    code: String,
}

/// `friend_list.cancel_link`.
pub fn cancel_link(call: Call, cx: &mut App) -> Reply {
    let params: CodeParams = params!(call);
    let code = params.code.trim().to_ascii_uppercase();
    run(
        &call,
        cx,
        move |servers, cx| servers.friend_cancel_link(code, cx),
        |(), _, _| Ok(json!({ "cancelled": true })),
    )
}

#[derive(Deserialize)]
struct LinkParams {
    link: String,
    /// `answer_link`: the safety code the user saw and expects.
    #[serde(default)]
    safety_code: Option<String>,
}

/// `friend_list.preview_link`: who a friend link is from, without answering it. It asks
/// the service (so it isn't read-only), but makes no key.
pub fn preview_link(call: Call, cx: &mut App) -> Reply {
    let params: LinkParams = params!(call);
    run(
        &call,
        cx,
        move |servers, cx| servers.friend_preview(params.link, cx),
        |preview, _, _| {
            Ok(json!({
                "name": preview.name,
                "safety_code": preview.safety_code,
                "key": preview.key,
                "already_friends": preview.already_friends,
                "same_name_as": preview.same_name_as,
            }))
        },
    )
}

fn same_code(a: &str, b: &str) -> bool {
    let clean = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_uppercase()
    };
    clean(a) == clean(b)
}

/// `friend_list.answer_link`: adds the link's sender as a pending friend, after the
/// user saw their name and safety code (`"confirm": true`).
pub fn answer_link(call: Call, cx: &mut App) -> Reply {
    let params: LinkParams = params!(call);
    tri!(call.confirm(
        "adds the link's sender as your friend. Show the user their name and safety code first (`friend-list preview`)"
    ));
    let servers = call.control.servers.clone();
    let preview = servers.update(cx, |servers, cx| {
        servers.friend_preview(params.link.clone(), cx)
    });
    cx.spawn(async move |cx| {
        let preview = preview.await.map_err(failed)?;
        if let Some(expected) = &params.safety_code
            && !same_code(expected, &preview.safety_code)
        {
            return Err(api_error(
                codes::CONFLICT,
                format!(
                    "The link's safety code is {}, not {expected}. Nothing was sent.",
                    preview.safety_code
                ),
            ));
        }
        let task = cx.update(|cx| {
            servers.update(cx, |servers, cx| servers.friend_answer(params.link, cx))
        });
        let key = task.await.map_err(failed)?;
        let mut answer = friend_answer(&key, cx, &servers)?;
        answer["note"] = json!("Pending until they accept your answer. Compare safety codes by voice or in person, then `friend-list verify`.");
        Ok(answer)
    })
}

/// `friend_list.replies`: answers to this person's open links.
pub fn replies(call: Call, cx: &mut App) -> Reply {
    run(
        &call,
        cx,
        |servers, cx| servers.friend_replies(cx),
        |replies, _, _| {
            Ok(json!({ "replies": replies.iter().map(reply_json).collect::<Vec<_>>() }))
        },
    )
}

#[derive(Deserialize)]
struct AcceptParams {
    code: String,
    key: String,
}

/// `friend_list.accept`: accepts an answer (`"confirm": true`).
pub fn accept(call: Call, cx: &mut App) -> Reply {
    let params: AcceptParams = params!(call);
    tri!(call.confirm(
        "adds the person who answered your link as your friend. Show the user their name and safety code first (`friend-list replies`)"
    ));
    let code = params.code.trim().to_ascii_uppercase();
    let key = params.key.trim().to_ascii_lowercase();
    let answer_key = key.clone();
    run(
        &call,
        cx,
        move |servers, cx| servers.friend_accept(code, key, cx),
        move |(), cx, servers| friend_answer(&answer_key, cx, servers),
    )
}

/// `friend_list.list`: friends, as last checked (`friend_list.refresh` checks again).
/// Makes nothing and reaches no one.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let servers = call.control.servers.clone();
    let opening = servers.update(cx, |servers, cx| servers.friend_peek(cx));
    cx.spawn(async move |cx| {
        if opening.await.map_err(failed)?.is_none() {
            return Ok(json!({ "me": null, "friends": [], "note": NO_LIST_NOTE }));
        }
        Ok(cx.update(|cx| {
            let servers = servers.read(cx);
            let list = &servers.friend_list;
            json!({
                "me": me_json(&list.snapshot),
                "friends": list.sorted().into_iter().map(|f| friend_json(f, servers)).collect::<Vec<_>>(),
                "checked_seconds_ago": list.refreshed_at.map(|at| at.elapsed().as_secs()),
            })
        }))
    })
}

fn report_json(report: &RefreshReport) -> Value {
    json!({
        "confirmed": report.refreshed.confirmed,
        "retired": report.refreshed.retired,
        "dropped_cores": report.refreshed.dropped_cores,
        "same_name": report
            .refreshed
            .same_name
            .iter()
            .map(|(key, names)| json!({ "friend": key, "same_name_as": names }))
            .collect::<Vec<_>>(),
        "problems": report
            .refreshed
            .problems
            .iter()
            .map(|(key, why)| match key.as_str() {
                // not about one friend (the service asked to slow down)
                "" => json!({ "error": why }),
                key => json!({ "friend": key, "error": why }),
            })
            .chain(report.problems.iter().map(|why| json!({ "error": why })))
            .collect::<Vec<_>>(),
        "presence": report
            .presence
            .iter()
            .map(|p| json!({ "friend": p.friend, "core": hex(&p.core.fingerprint), "core_name": p.core.name, "online": p.online }))
            .collect::<Vec<_>>(),
        "replies": report.replies.iter().map(reply_json).collect::<Vec<_>>(),
    })
}

/// `friend_list.refresh`: reads friends' states, checks presence and link answers.
pub fn refresh(call: Call, cx: &mut App) -> Reply {
    run(
        &call,
        cx,
        |servers, cx| servers.friend_refresh(cx),
        |report, _, _| Ok(report_json(&report)),
    )
}

#[derive(Deserialize)]
struct FriendParams {
    friend: String,
    #[serde(default)]
    value: Option<bool>,
}

/// Ticking "Codes match" needs `"confirm": true`; unticking doesn't.
fn verify_confirmation(verified: bool, params: &Value) -> Result<(), ErrorObject> {
    if !verified || super::confirmed(params) {
        return Ok(());
    }
    Err(api_error(
        codes::CONFIRMATION_REQUIRED,
        "This marks the friend as checked: that their safety code is the one the user's friend read out by voice or in person. Only the user can say so. Show them both codes (`friend-list list`, `friend-list me`), ask them to compare the codes with their friend aloud, and pass --yes (\"confirm\": true) only if they say the codes match.",
    ))
}

/// `friend_list.verify`: ticks (or with `value: false` unticks) "Codes match". Ticking
/// needs `"confirm": true`, on the user's word only: it's what says this friend is who
/// they claim to be.
pub fn verify(call: Call, cx: &mut App) -> Reply {
    let params: FriendParams = params!(call);
    let verified = params.value.unwrap_or(true);
    tri!(verify_confirmation(verified, &call.params));
    with_friend(
        &call,
        cx,
        params.friend,
        move |servers, key, cx| servers.friend_set_verified(key, verified, cx),
        |(), key, cx, servers| friend_answer(&key, cx, servers),
    )
}

/// `friend_list.hide`: hides this person's cores from a friend (`value: false`
/// shows them again).
pub fn hide(call: Call, cx: &mut App) -> Reply {
    let params: FriendParams = params!(call);
    let hidden = params.value.unwrap_or(true);
    with_friend(
        &call,
        cx,
        params.friend,
        move |servers, key, cx| servers.friend_set_hidden(key, hidden, cx),
        |(), key, cx, servers| friend_answer(&key, cx, servers),
    )
}

#[derive(Deserialize)]
struct NicknameParams {
    friend: String,
    #[serde(default)]
    nickname: Option<String>,
}

/// `friend_list.nickname`: the name only this person sees (none: their own name).
pub fn nickname(call: Call, cx: &mut App) -> Reply {
    let params: NicknameParams = params!(call);
    let nickname = params.nickname.filter(|n| !n.trim().is_empty());
    with_friend(
        &call,
        cx,
        params.friend,
        move |servers, key, cx| servers.friend_set_nickname(key, nickname, cx),
        |(), key, cx, servers| friend_answer(&key, cx, servers),
    )
}

#[derive(Deserialize)]
struct RemoveParams {
    friend: String,
}

/// `friend_list.remove`.
pub fn remove(call: Call, cx: &mut App) -> Reply {
    let params: RemoveParams = params!(call);
    tri!(call.confirm(
        "removes the friend: you stop seeing each other's servers, and becoming friends again needs a new friend link"
    ));
    with_friend(
        &call,
        cx,
        params.friend,
        |servers, key, cx| servers.friend_remove(key, cx),
        |(), key, _, _| Ok(json!({ "removed": key })),
    )
}

#[derive(Deserialize)]
struct ServersParams {
    friend: String,
    /// A core's fingerprint (or its first hex digits), or its name; all by default.
    #[serde(default)]
    core: Option<String>,
}

fn server_json(server: &SharedServer) -> Value {
    json!({
        "instance": server.instance_uuid,
        "name": server.name,
        "core_name": server.core_name,
        "game_version": server.game_version,
        "loader": server.loader,
        "loader_version": server.loader_version,
        "modpack": server.modpack,
        "state": server.state,
        "players": server.player_count,
        "max_players": server.max_player_count,
    })
}

/// `friend_list.servers`: what a friend's cores share with this person.
pub fn servers(call: Call, cx: &mut App) -> Reply {
    let params: ServersParams = params!(call);
    let servers = call.control.servers.clone();
    let opening = servers.update(cx, |servers, cx| servers.friend_peek(cx));
    cx.spawn(async move |cx| {
        let snapshot = opening.await.map_err(failed)?.ok_or_else(no_list)?;
        let key = find_friend(&snapshot.book, &params.friend).map_err(|_| {
            api_error(
                codes::NOT_FOUND,
                format!("No single friend matches \"{}\".", params.friend),
            )
        })?;
        let friend = snapshot.book.friend(&key).cloned().expect("found above");
        let cores: Vec<(String, String)> = friend
            .joinable_cores(now())
            .iter()
            .map(|c| (hex(&c.fingerprint), c.name.clone()))
            .filter(|(fp, name)| match &params.core {
                Some(core) => {
                    fp.starts_with(&core.to_ascii_lowercase()) || name.eq_ignore_ascii_case(core)
                }
                None => true,
            })
            .collect();
        if cores.is_empty() {
            return Err(api_error(
                codes::NOT_FOUND,
                format!(
                    "{} has no core to join{}.",
                    friend.display_name(),
                    if friend.status == launcher_core::friend_list::FriendStatus::Pending {
                        " yet: they haven't confirmed"
                    } else {
                        ""
                    }
                ),
            ));
        }
        let mut out = Vec::new();
        for (fingerprint, name) in cores {
            let task = cx.update(|cx| {
                servers.update(cx, |servers, cx| {
                    servers.friend_servers(key.clone(), fingerprint.clone(), cx)
                })
            });
            out.push(match task.await {
                Ok(list) => json!({
                    "core": fingerprint,
                    "core_name": name,
                    "servers": list.iter().map(server_json).collect::<Vec<_>>(),
                }),
                Err(error) => json!({ "core": fingerprint, "core_name": name, "error": error }),
            });
        }
        Ok(json!({ "friend": key, "cores": out }))
    })
}

/// `friend_list.retire`: retires this person's friend identity.
pub fn retire(call: Call, cx: &mut App) -> Reply {
    tri!(call.confirm(
        "retires your friend identity: every friend is told and drops you, then your key and friend list are deleted. It can't be undone"
    ));
    run(
        &call,
        cx,
        |servers, cx| servers.friend_retire(cx),
        |unreached, _, _| {
            Ok(json!({
                "retired": true,
                "unreached": unreached
                    .iter()
                    .map(|(key, why)| json!({ "friend": key, "error": why }))
                    .collect::<Vec<_>>(),
            }))
        },
    )
}

#[derive(Deserialize)]
struct CoreParams {
    /// One of this person's cores: its fingerprint (or its first 8+ hex digits), or
    /// its name.
    core: String,
}

/// `friend_list.retire_core`: retires one of this person's cores (`"confirm": true`):
/// friends drop it, and it's never listed again.
pub fn retire_core(call: Call, cx: &mut App) -> Reply {
    let params: CoreParams = params!(call);
    tri!(call.confirm(
        "retires one of your cores: friends drop it and stop trusting its servers' mods, and it's never shown to them again. It can't be undone"
    ));
    let servers = call.control.servers.clone();
    let opening = servers.update(cx, |servers, cx| servers.friend_peek(cx));
    cx.spawn(async move |cx| {
        let snapshot = opening.await.map_err(failed)?.ok_or_else(no_list)?;
        let reference = params.core.trim().to_string();
        let lower = reference.to_ascii_lowercase();
        let found: Vec<_> = snapshot
            .book
            .my_cores
            .iter()
            .filter(|c| {
                c.fingerprint == lower
                    || (lower.len() >= 8 && c.fingerprint.starts_with(&lower))
                    || c.name.eq_ignore_ascii_case(&reference)
            })
            .collect();
        let core = match found.as_slice() {
            [core] => (*core).clone(),
            [] => {
                return Err(api_error(
                    codes::NOT_FOUND,
                    format!(
                        "You don't share from a core called \"{reference}\". See `friend-list me`."
                    ),
                ));
            }
            _ => {
                return Err(api_error(
                    codes::AMBIGUOUS,
                    format!("Several of your cores match \"{reference}\". Use its fingerprint."),
                ));
            }
        };
        let task = cx.update(|cx| {
            servers.update(cx, |servers, cx| {
                servers.friend_retire_core(core.fingerprint.clone(), cx)
            })
        });
        let unreached = task.await.map_err(failed)?;
        Ok(json!({
            "retired": core.fingerprint,
            "name": core.name,
            "unreached": unreached
                .iter()
                .map(|(key, why)| json!({ "friend": key, "error": why }))
                .collect::<Vec<_>>(),
        }))
    })
}

/// Opens the Add friend dialog for a friend link (`friends.open` with a friend link,
/// as opening `lodestone://friend?link=…` does). Nothing is added.
pub fn open_link(link: String, cx: &mut App, servers: &Entity<Servers>) -> ApiResult {
    servers.update(cx, |servers, cx| servers.open_add_friend(Some(link), cx));
    Ok(json!({ "opened": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_codes_compare_without_case_or_dashes() {
        assert!(same_code("K7Q2-M9XD-4TRW-H3BE", "k7q2m9xd4trwh3be"));
        assert!(same_code(" K7Q2 M9XD 4TRW H3BE ", "K7Q2-M9XD-4TRW-H3BE"));
        assert!(!same_code("K7Q2-M9XD-4TRW-H3BE", "K7Q2-M9XD-4TRW-H3BF"));
    }

    #[test]
    fn codes_match_needs_the_user_s_confirmation() {
        let refused = verify_confirmation(true, &json!({ "friend": "Alex" })).unwrap_err();
        assert_eq!(refused.code, codes::CONFIRMATION_REQUIRED);
        assert!(refused.message.contains("aloud"), "{}", refused.message);
        assert!(verify_confirmation(true, &json!({ "friend": "Alex", "confirm": true })).is_ok());
        // unticking needs nothing
        assert!(verify_confirmation(false, &json!({ "friend": "Alex" })).is_ok());
    }
}
