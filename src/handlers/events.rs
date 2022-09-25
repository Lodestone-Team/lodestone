use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, Path, Query, WebSocketUpgrade},
    response::Response,
    Extension, Json,
};
use axum_auth::AuthBearer;

use futures::{SinkExt, StreamExt};
use log::error;
use ringbuffer::RingBufferExt;

use serde::Deserialize;
use tokio::sync::{broadcast::Receiver, Mutex};

use crate::{
    events::{Event, EventInner},
    json_store::user::User,
    stateful::Stateful,
    traits::{Error, ErrorInner},
    AppState,
};

use super::util::{can_user_view_event, parse_bearer_token, try_auth};

pub async fn get_event_buffer(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Path(uuid): Path<String>,
) -> Result<Json<Vec<Event>>, Error> {
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "Token error".to_string(),
    })?;
    Ok(Json(
        state
            .events_buffer
            .lock()
            .await
            .get_ref()
            .iter()
            .rev()
            .filter(|event| {
                (event.instance_uuid == uuid || uuid == "all")
                    && can_user_view_event(event, &requester)
            })
            .cloned()
            .collect(),
    ))
}

pub async fn get_console_out_buffer(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Path(uuid): Path<String>,
) -> Result<Json<Vec<Event>>, Error> {
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "Token error".to_string(),
    })?;
    Ok(Json(
        state
            .console_out_buffer
            .lock()
            .await
            .get_ref()
            .iter()
            .rev()
            .filter(|event| {
                (event.instance_uuid == uuid || uuid == "all")
                    && can_user_view_event(event, &requester)
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
    query: Query<WebsocketQuery>,
    Path(uuid): Path<String>,
) -> Result<Response, Error> {
    let users = state.users.lock().await;

    let user = parse_bearer_token(query.token.as_str())
        .and_then(|token| try_auth(&token, users.get_ref()))
        .ok_or_else(|| Error {
            inner: ErrorInner::PermissionDenied,
            detail: "".to_string(),
        })?;
    drop(users);
    let users = state.users.clone();
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(ws.on_upgrade(move |socket| {
        event_stream_ws(socket, event_receiver, uuid.clone(), user.uid, users)
    }))
}

async fn event_stream_ws(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    uuid: String,
    uid: String,
    users: Arc<Mutex<Stateful<HashMap<String, User>>>>,
) {
    let (mut sender, mut _receiver) = stream.split();

    while let Ok(event) = event_receiver.recv().await {
        if event.instance_uuid != uuid && uuid != "all" {
            continue;
        }
        if can_user_view_event(
            &event,
            match users.lock().await.get_ref().get(&uid) {
                Some(user) => user,
                None => break,
            },
        ) {
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
            inner: ErrorInner::PermissionDenied,
            detail: "".to_string(),
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
    users: Arc<Mutex<Stateful<HashMap<String, User>>>>,
) {
    let (mut sender, mut _receiver) = stream.split();
    while let Ok(event) = event_receiver.recv().await {
        if event.instance_uuid != uuid && uuid != "all" {
            continue;
        }
        match event.event_inner {
            EventInner::InstanceOutput(_) => {
                if can_user_view_event(
                    &event,
                    match users.lock().await.get_ref().get(&uid) {
                        Some(user) => user,
                        None => break,
                    },
                ) {
                    if let Err(e) = sender
                        .send(axum::extract::ws::Message::Text(
                            serde_json::to_string(&event).unwrap(),
                        ))
                        .await
                    {
                        error!("Failed to send console out: {}", e);
                        break;
                    }
                }
            }
            _ => continue,
        }
    }
}
