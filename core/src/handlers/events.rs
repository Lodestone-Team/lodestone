use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Json, Router,
};

use color_eyre::eyre::eyre;
use futures::{SinkExt, StreamExt};
use ringbuffer::RingBuffer;
use tracing::{debug, error, warn};

use crate::output_types::ClientEvent;
use crate::types::InstanceUuid;
use crate::{
    auth::{
        user::{User, UsersManager},
        user_id::UserId,
    },
    db::read::search_events,
    error::{Error, ErrorKind},
    events::EventQuery,
};

use crate::{
    events::{Event, EventInner, UserEventInner},
    AppState,
};
use serde::Deserialize;
use tokio::sync::{broadcast::error::RecvError, broadcast::Receiver, RwLock};
use ts_rs::TS;

use super::util::{authenticate, authenticate_query_token, AuthedUser};

#[derive(Deserialize, Clone, Debug, TS)]
pub struct EventQueryWrapper {
    filter: String,
}

fn parse_event_query(query: &EventQueryWrapper) -> Result<EventQuery, Error> {
    serde_json::from_str(&query.filter).map_err(|e| {
        error!("Error deserializing event query: {}", e);
        Error {
            kind: ErrorKind::BadRequest,
            source: e.into(),
        }
    })
}

// The `{uuid}` in `/events/{uuid}/...` is not used: the dashboard always sends `all`, and the
// event query selects instances with `event_instance_ids`.
pub async fn get_event_buffer(
    State(state): State<AppState>,
    requester: AuthedUser,
    query: Query<EventQueryWrapper>,
) -> Result<Json<Vec<Event>>, Error> {
    let query = parse_event_query(&query)?;
    Ok(Json(
        state
            .events_buffer
            .lock()
            .await
            .iter()
            .filter(|event| {
                requester.can_view_event(*event) && query.filter(ClientEvent::from(*event))
            })
            .cloned()
            .collect(),
    ))
}

pub async fn get_event_search(
    State(state): State<AppState>,
    requester: AuthedUser,
    query: Query<EventQueryWrapper>,
) -> Result<Json<Vec<ClientEvent>>, Error> {
    let query = parse_event_query(&query)?;
    let events = search_events(&state.sqlite_pool, query).await?;
    Ok(Json(
        events
            .into_iter()
            .filter(|event| requester.can_view_event_parts(&event.event_inner, &event.caused_by))
            .collect(),
    ))
}

pub async fn get_console_buffer(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<Vec<Event>>, Error> {
    Ok(Json(
        state
            .console_out_buffer
            .lock()
            .await
            .get(&uuid)
            .map(|buffer| {
                buffer
                    .iter()
                    .filter(|event| match &event.event_inner {
                        EventInner::InstanceEvent(instance_event) => {
                            instance_event.instance_uuid == uuid && requester.can_view_event(event)
                        }
                        _ => false,
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
    ))
}

#[derive(Deserialize)]
pub struct WebsocketQuery {
    token: String,
}

pub async fn event_stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    query: Query<EventQueryWrapper>,
) -> Result<Response, Error> {
    let query = parse_event_query(&query)?;
    let token = query.bearer_token.clone().ok_or(Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Missing token"),
    })?;

    let user = authenticate(&state, &token).await?;
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(ws.on_upgrade(move |socket| {
        event_stream_ws(socket, event_receiver, query, user, state.users_manager)
    }))
}

/// What a stream should do after a user event.
enum UserEventAction {
    Continue,
    /// The user's permissions may have changed: read the user again.
    Refresh,
    /// The user logged out (or changed password) or was deleted: close the stream.
    Close,
}

fn user_event_action(event: &Event, uid: &UserId) -> UserEventAction {
    match &event.event_inner {
        EventInner::UserEvent(user_event) if user_event.user_id == *uid => {
            match user_event.user_event_inner {
                UserEventInner::UserLoggedOut | UserEventInner::UserDeleted => {
                    UserEventAction::Close
                }
                _ => UserEventAction::Refresh,
            }
        }
        _ => UserEventAction::Continue,
    }
}

/// Applies a user event to the stream's cached copy of the user. Returns `false` when the
/// stream must close.
async fn update_cached_user(
    event: &Event,
    user: &mut User,
    users_manager: &RwLock<UsersManager>,
) -> bool {
    match user_event_action(event, &user.uid) {
        UserEventAction::Continue => true,
        UserEventAction::Close => false,
        UserEventAction::Refresh => match users_manager.read().await.get_user(&user.uid) {
            Some(new_user) => {
                *user = new_user;
                true
            }
            None => false,
        },
    }
}

/// Handles a frame from the client. Returns `false` when the client is gone.
///
/// Pings are answered by the WebSocket library; other frames are ignored.
fn client_still_connected(msg: Option<Result<Message, axum::Error>>) -> bool {
    match msg {
        Some(Ok(Message::Close(_))) | None => {
            debug!("Websocket disconnected");
            false
        }
        Some(Ok(_)) => true,
        Some(Err(e)) => {
            debug!("Websocket error: {}", e);
            false
        }
    }
}

async fn send_event(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    event: &Event,
) -> bool {
    let text = match serde_json::to_string(event) {
        Ok(text) => text,
        Err(e) => {
            error!("Failed to serialize event: {}", e);
            return true;
        }
    };
    if let Err(e) = sender.send(Message::Text(text.into())).await {
        debug!("Error sending event to websocket: {}", e);
        return false;
    }
    true
}

async fn event_stream_ws(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    query: EventQuery,
    mut user: User,
    users_manager: Arc<RwLock<UsersManager>>,
) {
    let (mut sender, mut receiver) = stream.split();
    loop {
        tokio::select! {
            event = event_receiver.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(RecvError::Lagged(skipped)) => {
                        warn!("Event stream lagged, skipped {} events", skipped);
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };
                if !update_cached_user(&event, &mut user, &users_manager).await {
                    break;
                }
                if event.is_event_console_message() {
                    continue;
                }
                if user.can_view_event(&event)
                    && query.filter(ClientEvent::from(&event))
                    && !send_event(&mut sender, &event).await
                {
                    break;
                }
            }
            msg = receiver.next() => {
                if !client_still_connected(msg) {
                    break;
                }
            }
        }
    }
}

pub async fn console_stream(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    query: Query<WebsocketQuery>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Response, Error> {
    let user = authenticate_query_token(&state, &query.token).await?;
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(ws.on_upgrade(move |socket| {
        console_stream_ws(socket, event_receiver, user, uuid, state.users_manager)
    }))
}

async fn console_stream_ws(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    mut user: User,
    uuid: InstanceUuid,
    users_manager: Arc<RwLock<UsersManager>>,
) {
    let (mut sender, mut receiver) = stream.split();
    loop {
        tokio::select! {
            event = event_receiver.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(RecvError::Lagged(skipped)) => {
                        warn!("Console stream lagged, skipped {} events", skipped);
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                };
                if !update_cached_user(&event, &mut user, &users_manager).await {
                    break;
                }
                let EventInner::InstanceEvent(instance_event) = &event.event_inner else {
                    continue;
                };
                if event.is_event_console_message()
                    && instance_event.instance_uuid == uuid
                    && user.can_view_event(&event)
                    && !send_event(&mut sender, &event).await
                {
                    break;
                }
            }
            msg = receiver.next() => {
                if !client_still_connected(msg) {
                    break;
                }
            }
        }
    }
}

pub fn get_events_routes(state: AppState) -> Router {
    Router::new()
        .route("/events/{uuid}/stream", get(event_stream))
        .route("/events/{uuid}/buffer", get(get_event_buffer))
        .route("/events/search", get(get_event_search))
        .route("/instance/{uuid}/console/stream", get(console_stream))
        .route("/instance/{uuid}/console/buffer", get(get_console_buffer))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::UserEvent;
    use crate::{events::CausedBy, types::Snowflake};

    fn user_event(uid: &UserId, inner: UserEventInner) -> Event {
        Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid.clone(),
                user_event_inner: inner,
            }),
            details: String::new(),
            snowflake: Snowflake::default(),
            caused_by: CausedBy::System,
        }
    }

    #[test]
    fn streams_close_on_logout_and_delete() {
        let uid = UserId::default();
        let other = UserId::default();
        assert!(matches!(
            user_event_action(&user_event(&uid, UserEventInner::UserLoggedOut), &uid),
            UserEventAction::Close
        ));
        assert!(matches!(
            user_event_action(&user_event(&uid, UserEventInner::UserDeleted), &uid),
            UserEventAction::Close
        ));
        assert!(matches!(
            user_event_action(&user_event(&other, UserEventInner::UserLoggedOut), &uid),
            UserEventAction::Continue
        ));
        assert!(matches!(
            user_event_action(
                &user_event(
                    &uid,
                    UserEventInner::UsernameChanged {
                        new_username: "x".to_string()
                    }
                ),
                &uid
            ),
            UserEventAction::Refresh
        ));
    }
}
