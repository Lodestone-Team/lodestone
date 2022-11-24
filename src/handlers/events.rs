use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, Path, Query, WebSocketUpgrade},
    response::Response,
    routing::get,
    Extension, Json, Router,
};
use axum_auth::AuthBearer;

use futures::{SinkExt, StreamExt};
use log::{debug, error};
use ringbuffer::{AllocRingBuffer, RingBufferExt};

use crate::events::InstanceEventKind;
use crate::events::UserEventKind;
use crate::{events::EventType, output_types::ClientEvent};

use crate::{
    auth::user::User,
    events::{Event, EventInner, EventLevel, UserEventInner},
    stateful::Stateful,
    traits::{Error, ErrorInner},
    AppState,
};
use serde::Deserialize;
use tokio::sync::{broadcast::Receiver, Mutex};
use ts_rs::TS;

use super::util::{can_user_view_event, parse_bearer_token, try_auth};
#[derive(Deserialize, Clone, Debug, TS)]
#[ts(export)]
struct EventQuery {
    pub event_levels: Option<Vec<EventLevel>>,
    pub event_types: Option<Vec<EventType>>,
    pub instance_event_types: Option<Vec<InstanceEventKind>>,
    pub user_event_types: Option<Vec<UserEventKind>>,
    pub event_instance_ids: Option<Vec<String>>,
    pub bearer_token: Option<String>,
}

impl EventQuery {
    fn filter(&self, event: impl AsRef<ClientEvent>) -> bool {
        let event = event.as_ref();
        if let Some(event_levels) = &self.event_levels {
            if !event_levels.contains(&event.level) {
                return false;
            }
        }
        if let Some(event_types) = &self.event_types {
            if !event_types.contains(&event.event_inner.as_ref().into()) {
                return false;
            }
        }
        if let Some(instance_event_types) = &self.instance_event_types {
            if let EventInner::InstanceEvent(instance_event) = &event.event_inner {
                if !instance_event_types
                    .contains(&instance_event.instance_event_inner.as_ref().into())
                {
                    return false;
                }
            } else {
                return false;
            }
        }
        if let Some(user_event_types) = &self.user_event_types {
            if let EventInner::UserEvent(user_event) = &event.event_inner {
                if !user_event_types.contains(&user_event.user_event_inner.as_ref().into()) {
                    return false;
                }
            } else {
                return false;
            }
        }
        if let Some(event_instance_ids) = &self.event_instance_ids {
            if let EventInner::InstanceEvent(instance_event) = &event.event_inner {
                if !event_instance_ids.contains(&instance_event.instance_uuid) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

#[derive(Deserialize, Clone, Debug, TS)]
pub struct EventQueryWrapper {
    filter: String,
}

pub async fn get_event_buffer(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    query: Query<EventQueryWrapper>,
) -> Result<Json<Vec<Event>>, Error> {
    // deserialize query
    let query: EventQuery = serde_json::from_str(&query.filter).map_err(|e| {
        error!("Error deserializing event query: {}", e);
        Error {
            inner: ErrorInner::MalformedRequest,
            detail: e.to_string(),
        }
    })?;
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    Ok(Json(
        state
            .events_buffer
            .lock()
            .await
            .iter()
            .filter(|event| {
                query.filter(ClientEvent::from(*event)) && can_user_view_event(*event, &requester)
            })
            .cloned()
            .collect(),
    ))
}

pub async fn get_console_buffer(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Path(uuid): Path<String>,
) -> Result<Json<Vec<Event>>, Error> {
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    Ok(Json(
        state
            .console_out_buffer
            .lock()
            .await
            .get(&uuid)
            .unwrap_or(&AllocRingBuffer::new())
            .iter()
            .filter(|event| match &event.event_inner {
                EventInner::InstanceEvent(instance_event) => {
                    (instance_event.instance_uuid == uuid || uuid == "all")
                        && can_user_view_event(*event, &requester)
                }
                _ => false,
            })
            .cloned()
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct WebsocketQuery {
    token: String,
}

pub async fn event_stream(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    query: Query<EventQueryWrapper>,
) -> Result<Response, Error> {
    let query: EventQuery = serde_json::from_str(query.filter.as_str()).map_err(|e| {
        error!("Error deserializing event query: {}", e);
        Error {
            inner: ErrorInner::MalformedRequest,
            detail: e.to_string(),
        }
    })?;
    let token = query.bearer_token.clone().ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "No token provided".to_string(),
    })?;
    let users = state.users.lock().await;

    let user = try_auth(&token, users.get_ref()).ok_or_else(|| Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    drop(users);
    let users = state.users.clone();
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(
        ws.on_upgrade(move |socket| {
            event_stream_ws(socket, event_receiver, query, user.uid, users)
        }),
    )
}

async fn event_stream_ws(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    query: EventQuery,
    uid: String,
    users: Arc<Mutex<Stateful<HashMap<String, User>, ()>>>,
) {
    let (mut sender, mut receiver) = stream.split();
    loop {
        tokio::select! {
            Ok(event) = event_receiver.recv() => {
                if event.is_event_console_message() {
                    continue;
                }
                if query.filter(ClientEvent::from(event.clone())) && can_user_view_event(&event, users.lock().await.get_ref().get(&uid).unwrap()) {
                    if let Err(e) = sender.send(axum::extract::ws::Message::Text(serde_json::to_string(&event).unwrap())).await {
                        error!("Error sending event to websocket: {}", e);
                        break;
                    }
                }
            }
            Some(Ok(ws_msg)) = receiver.next() => {
                match sender.send(ws_msg).await {
                    Ok(_) => debug!("Replied to ping"),
                    Err(_) => {debug!("breaking"); break},
                };
            }
        }
    }
}

pub async fn console_stream(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    query: Query<WebsocketQuery>,
    Path(uuid): Path<String>,
) -> Result<Response, Error> {
    let uuid = uuid.as_str().to_owned();
    let users = state.users.lock().await;

    let user = parse_bearer_token(query.token.as_str())
        .and_then(|token| try_auth(&token, users.get_ref()))
        .ok_or_else(|| Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    drop(users);
    let users = state.users.clone();
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(ws
        .on_upgrade(move |socket| console_stream_ws(socket, event_receiver, user.uid, uuid, users)))
}

async fn console_stream_ws(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    uid: String,
    uuid: String,
    users: Arc<Mutex<Stateful<HashMap<String, User>, ()>>>,
) {
    let (mut sender, mut receiver) = stream.split();
    loop {
        tokio::select! {
            Ok(event) = event_receiver.recv() => {
                match &event.event_inner {
                    EventInner::InstanceEvent(instance_event) => {
                        if event.is_event_console_message() && (instance_event.instance_uuid == uuid || uuid == "all")
                            && can_user_view_event(
                                &event,
                                match users.lock().await.get_ref().get(&uid) {
                                    Some(user) => user,
                                    None => break,
                                },
                            )
                        {
                            if let Err(e) = sender
                                .send(axum::extract::ws::Message::Text(
                                    serde_json::to_string(&event).unwrap(),
                                ))
                                .await
                            {
                                error!("Failed to send event: {}", e);
                                break;
                            }
                        }
                    }
                    EventInner::UserEvent(user_event) => {
                        match user_event.user_event_inner {
                            UserEventInner::UserLoggedOut | UserEventInner::UserDeleted => {
                                if user_event.user_id == uid {
                                    break;
                                }
                            },
                            _ => {}
                        }
                    },
                    EventInner::MacroEvent(_) => continue,
                    EventInner::ProgressionEvent(_) => continue,
                    EventInner::FSEvent(_) => continue,
                }
            }
            Some(Ok(ws_msg)) = receiver.next() => {
                match sender.send(ws_msg).await {
                    Ok(_) => debug!("Replied to ping"),
                    Err(_) => break,
                };
            }
        }
    }
}

pub fn get_events_routes() -> Router {
    Router::new()
        .route("/events/:uuid/stream", get(event_stream))
        .route("/events/:uuid/buffer", get(get_event_buffer))
        .route("/instance/:uuid/console/stream", get(console_stream))
        .route("/instance/:uuid/console/buffer", get(get_console_buffer))
}
