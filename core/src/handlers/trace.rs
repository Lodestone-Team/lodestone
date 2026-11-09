//! `/trace/*`: the core's debug traces (see [`crate::trace`]), for its owner only.
//!
//! - `GET /trace/topics`: the topics, what each says, and how many events each had;
//! - `GET /trace/snapshot`: what the core is doing now;
//! - `GET /trace/stream` (WebSocket): subscriptions. The client sends
//!   `{"op": "subscribe", "topics": [...], "backlog": N, "since": SEQ}` and
//!   `{"op": "unsubscribe", "subscription": ID}`; the core answers
//!   `{"op": "subscribed", "subscription": ID}` and sends
//!   `{"op": "event", "subscription": ID, "event": {seq, at_ms, topic, data}}`.
//!   `?topics=a,b&backlog=N` subscribes at once. The token is the `Authorization`
//!   header, or `?token=Bearer%20…` for browsers.
//!
//! Events never leave the machine except to the owner's own connection.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    http::HeaderMap,
    response::Response,
    routing::get,
    Json, Router,
};
use color_eyre::eyre::eyre;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::util::{authenticate, parse_bearer_token, AuthedUser};
use crate::{
    auth::user::User,
    error::{Error, ErrorKind},
    trace::{self, Filter, TraceEvent},
    traits::t_configurable::TConfigurable,
    traits::t_server::TServer,
    AppState,
};

/// The topics the core emits, with what each says.
pub const CATALOG: &[(&str, &str)] = &[
    (
        "log.error",
        "An error the core logged (target, message, fields, spans).",
    ),
    ("log.warn", "A warning."),
    ("log.info", "An info record."),
    ("log.debug", "A debug record of the core."),
    ("log.trace", "A trace record of the core."),
    (
        "http.request",
        "An API request (method, route, status, ms).",
    ),
    ("http.error", "An API request answered with 400 or more."),
    ("instance.state", "A server started, stopped or failed."),
    ("instance.warning", "A server warning."),
    ("instance.error", "A server error."),
    ("instance.system", "A system message about a server."),
    ("instance.players", "Players joined or left."),
    ("instance.console", "A console line."),
    ("instance.input", "A console command sent."),
    ("instance.chat", "A player's chat message."),
    ("macro.started", "A macro started."),
    ("macro.detach", "A macro detached."),
    ("macro.stopped", "A macro stopped."),
    ("task.start", "A long task started (install, download, …)."),
    ("task.update", "A long task's progress."),
    ("task.end", "A long task ended."),
    (
        "user.user_logged_in",
        "A user signed in (user events are user.*).",
    ),
    ("fs.operation", "A file operation through the API."),
    ("settings.global", "A global setting changed."),
];

fn owner_only(user: &User) -> Result<(), Error> {
    if user.is_owner {
        Ok(())
    } else {
        Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Only the owner can read the core's debug traces"),
        })
    }
}

async fn topics(requester: AuthedUser) -> Result<Json<Value>, Error> {
    owner_only(&requester)?;
    let seen = trace::bus().topics();
    let count = |topic: &str| seen.iter().find(|(t, _)| t == topic).map_or(0, |(_, n)| *n);
    let live = |topic: &str| trace::LIVE_ONLY.iter().any(|p| topic.starts_with(p));
    let mut list: Vec<Value> = CATALOG
        .iter()
        .map(|(topic, about)| {
            json!({ "topic": topic, "about": about, "live_only": live(topic), "events": count(topic) })
        })
        .collect();
    for (topic, events) in &seen {
        if !CATALOG.iter().any(|(t, _)| t == topic) {
            list.push(
                json!({ "topic": topic, "about": "", "live_only": live(topic), "events": events }),
            );
        }
    }
    Ok(Json(json!({
        "topics": list,
        "backlog": trace::DEFAULT_BACKLOG,
        "last_seq": trace::bus().last_seq(),
        "patterns": "log or log.* takes everything under log; * is one segment (*.error); * alone takes everything",
    })))
}

async fn snapshot(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<Value>, Error> {
    owner_only(&requester)?;
    let instances: Vec<_> = state
        .instances
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().clone()))
        .collect();
    let mut list = Vec::new();
    for (uuid, instance) in instances {
        list.push(json!({
            "uuid": uuid,
            "name": instance.name().await,
            "state": instance.state().await,
        }));
    }
    let settings = state.global_settings.lock().await;
    Ok(Json(json!({
        "version": crate::VERSION.with(|v| v.to_string()),
        "uptime_secs": chrono::Utc::now().timestamp() - state.up_since,
        "instances": list,
        "safe_mode": state.safe_mode(),
        "code_execution": settings.code_execution_enabled(),
        "trace": {
            "last_seq": trace::bus().last_seq(),
            "subscribers": trace::bus().subscribers(),
        },
    })))
}

#[derive(Deserialize, Default)]
struct StreamQuery {
    /// `Bearer <token>`, for clients that can't set headers.
    #[serde(default)]
    token: Option<String>,
    /// Subscribe at once to these patterns (comma-separated).
    #[serde(default)]
    topics: Option<String>,
    #[serde(default)]
    backlog: Option<usize>,
    #[serde(default)]
    since: Option<u64>,
}

/// The session or key of a stream: the `Authorization` header, else `?token=`.
fn stream_token(headers: &HeaderMap, query: &StreamQuery) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer_token)
        .or_else(|| query.token.as_deref().and_then(parse_bearer_token))
}

async fn stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StreamQuery>,
) -> Result<Response, Error> {
    let token = stream_token(&headers, &query).ok_or(Error {
        kind: ErrorKind::Unauthorized,
        source: eyre!("Missing token"),
    })?;
    let user = authenticate(&state, &token).await?;
    owner_only(&user)?;
    Ok(ws.on_upgrade(move |socket| serve(socket, state, token, query)))
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum ClientMessage {
    Subscribe {
        #[serde(default)]
        topics: Vec<String>,
        #[serde(default)]
        backlog: Option<usize>,
        #[serde(default)]
        since: Option<u64>,
    },
    Unsubscribe {
        subscription: u64,
    },
}

/// Lines waiting for a slow client before events are dropped.
const QUEUE: usize = 10_000;

async fn serve(socket: WebSocket, state: AppState, token: String, query: StreamQuery) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<String>(QUEUE);
    let dropped = Arc::new(AtomicU64::new(0));
    let mut next_local = 1u64;
    // local subscription number → the bus's id
    let mut subscriptions: HashMap<u64, u64> = HashMap::new();

    let mut subscribe = |topics: Vec<String>,
                         backlog: Option<usize>,
                         since: Option<u64>,
                         subscriptions: &mut HashMap<u64, u64>| {
        let local = next_local;
        next_local += 1;
        // the answer goes first, so the backlog follows it
        let _ = tx.try_send(
            json!({ "op": "subscribed", "subscription": local, "topics": topics }).to_string(),
        );
        let (tx, dropped) = (tx.clone(), dropped.clone());
        let sink = Box::new(move |event: &TraceEvent| {
            let line = json!({ "op": "event", "subscription": local, "event": event.to_json() })
                .to_string();
            match tx.try_send(line) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    dropped.fetch_add(1, Ordering::Relaxed);
                    true
                }
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            }
        });
        let id = trace::bus().subscribe(
            Filter::new(topics.iter()),
            backlog.unwrap_or(0),
            since,
            sink,
        );
        subscriptions.insert(local, id);
    };

    if let Some(topics) = &query.topics {
        let topics = topics.split(',').map(str::to_string).collect();
        subscribe(topics, query.backlog, query.since, &mut subscriptions);
    }

    // the session or key is checked again now and then: a stream ends when it is
    // revoked, expires, or the user is no longer the owner
    let mut recheck = tokio::time::interval(Duration::from_secs(30));
    recheck.tick().await;
    loop {
        tokio::select! {
            line = rx.recv() => {
                let Some(line) = line else { break };
                if sender.send(Message::Text(line.into())).await.is_err() {
                    break;
                }
                let lost = dropped.swap(0, Ordering::Relaxed);
                if lost > 0 {
                    let note = json!({ "op": "lagged", "dropped": lost }).to_string();
                    if sender.send(Message::Text(note.into())).await.is_err() {
                        break;
                    }
                }
            }
            message = receiver.next() => {
                let Some(Ok(message)) = message else { break };
                let text = match message {
                    Message::Text(text) => text.to_string(),
                    Message::Close(_) => break,
                    _ => continue,
                };
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Subscribe { topics, backlog, since }) => {
                        subscribe(topics, backlog, since, &mut subscriptions)
                    }
                    Ok(ClientMessage::Unsubscribe { subscription }) => {
                        let ended = subscriptions
                            .remove(&subscription)
                            .is_some_and(|id| trace::bus().unsubscribe(id));
                        let _ = tx.try_send(json!({ "op": "unsubscribed", "subscription": subscription, "ended": ended }).to_string());
                    }
                    Err(err) => {
                        let _ = tx.try_send(json!({ "op": "error", "message": format!("Bad message: {err}") }).to_string());
                    }
                }
            }
            _ = recheck.tick() => {
                let still_owner = authenticate(&state, &token).await.is_ok_and(|user| user.is_owner);
                if !still_owner {
                    break;
                }
            }
        }
    }
    for id in subscriptions.into_values() {
        trace::bus().unsubscribe(id);
    }
}

pub fn get_trace_routes(state: AppState) -> Router {
    Router::new()
        .route("/trace/topics", get(topics))
        .route("/trace/snapshot", get(snapshot))
        .route("/trace/stream", get(stream))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

    use super::*;
    use crate::{
        auth::{permission::UserPermission, user::User},
        events::CausedBy,
        global_settings::GlobalSettingsData,
        test_util::{serve_api, test_app_state},
    };

    async fn add_user(state: &AppState, name: &str, is_owner: bool) -> String {
        let user = User::new(
            name.to_string(),
            "pw",
            is_owner,
            false,
            UserPermission::default(),
        );
        state
            .users_manager
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user.create_jwt().unwrap().to_string()
    }

    type Socket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// The next frame as JSON; fails instead of hanging.
    async fn next(socket: &mut Socket) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .expect("a frame")
            .unwrap()
            .unwrap();
        serde_json::from_str(frame.to_text().unwrap()).unwrap()
    }

    #[tokio::test]
    async fn only_the_owner_reads_traces() {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let state = test_app_state(dir.path(), GlobalSettingsData::default()).await;
        let owner = add_user(&state, "owner", true).await;
        let member = add_user(&state, "member", false).await;
        let base = serve_api(state.clone()).await;
        let client = reqwest::Client::new();
        for path in ["/trace/topics", "/trace/snapshot"] {
            let ok = client
                .get(format!("{base}{path}"))
                .bearer_auth(&owner)
                .send()
                .await
                .unwrap();
            assert_eq!(ok.status(), StatusCode::OK, "{path}");
            let refused = client
                .get(format!("{base}{path}"))
                .bearer_auth(&member)
                .send()
                .await
                .unwrap();
            assert_eq!(refused.status(), StatusCode::FORBIDDEN, "{path}");
            let anonymous = client.get(format!("{base}{path}")).send().await.unwrap();
            // the core's bearer extractor answers a missing token with 400
            assert!(
                matches!(anonymous.status().as_u16(), 400 | 401),
                "{path}: {}",
                anonymous.status()
            );
        }
        let topics: Value = client
            .get(format!("{base}/trace/topics"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(topics["topics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["topic"] == "instance.state"));

        // the stream: a member is refused, the owner gets the backlog and live events
        let ws = base.replacen("http://", "ws://", 1);
        let mut request = format!("{ws}/trace/stream").into_client_request().unwrap();
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {member}").parse().unwrap());
        match tokio_tungstenite::connect_async(request).await {
            Err(tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::FORBIDDEN)
            }
            other => panic!("a member got a stream: {other:?}"),
        }
        trace::emit("test.before", || json!({ "n": 1, "password": "hunter2" }));
        let mut request = format!("{ws}/trace/stream?topics=test&backlog=5")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {owner}").parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(next(&mut socket).await["op"], "subscribed");
        let before = next(&mut socket).await;
        assert_eq!(before["event"]["topic"], "test.before");
        assert_eq!(
            before["event"]["data"]["password"],
            lodestone_trace::REDACTED
        );
        trace::emit("other.topic", || json!({}));
        trace::emit("test.after", || json!({ "n": 2 }));
        let after = next(&mut socket).await;
        assert_eq!(
            after["event"]["topic"], "test.after",
            "other topics are filtered out"
        );
    }
}
