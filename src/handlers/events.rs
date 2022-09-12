use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, Query, WebSocketUpgrade},
    response::Response,
    Extension, Json,
};
use axum_auth::AuthBearer;

use futures::{SinkExt, StreamExt};
use headers::HeaderMap;
use log::error;
use ringbuffer::RingBufferExt;

use serde::Deserialize;
use tokio::sync::{broadcast::Receiver, Mutex};

use crate::{
    events::Event,
    json_store::user::User,
    stateful::Stateful,
    traits::{Error, ErrorInner},
    AppState,
};

use super::util::{can_user_view_event, parse_bearer_token, try_auth};

pub async fn get_event_buffer(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<Event>>, Error> {
    let mut ret = Vec::new();
    for event in state.events_buffer.lock().await.get_ref().iter().rev() {
        let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Token error".to_string(),
        })?;
        if can_user_view_event(event, &requester) {
            ret.push(event.clone());
        }
    }
    Ok(Json(ret))
}

#[derive(Deserialize)]
pub struct WebsocketQuery {
    token: String,
}

pub async fn event_stream(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    query: Query<WebsocketQuery>,
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

    Ok(ws.on_upgrade(move |socket| websocket(socket, event_receiver, user.uid, users)))
}

async fn websocket(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    uid: String,
    users: Arc<Mutex<Stateful<HashMap<String, User>>>>,
) {
    let (mut sender, mut _receiver) = stream.split();
    while let Ok(event) = event_receiver.recv().await {
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
