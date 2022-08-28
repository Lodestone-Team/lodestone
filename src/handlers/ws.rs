use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::{Response},
    Extension,
};
use futures::{SinkExt, StreamExt};
use headers::HeaderMap;
use log::error;
use tokio::sync::{broadcast::Receiver, Mutex};

use crate::{
    events::{Event, EventInner},
    json_store::{permission::Permission, user::User},
    stateful::Stateful,
    traits::{Error, ErrorInner},
    AppState,
};

use super::util::{is_authorized, parse_bearer_token, try_auth};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let users = state.users.lock().await;
    let user = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(parse_bearer_token)
        .and_then(|token| try_auth(&token, users.get_ref()));
    drop(users);
    if user.is_none() {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "".to_string(),
        });
    }
    let users = state.users.clone();
    let event_receiver = state.event_broadcaster.subscribe();

    Ok(ws.on_upgrade(move |socket| {
        websocket(socket, event_receiver, user.unwrap().uid, users)
    }))
}

async fn websocket(
    stream: WebSocket,
    mut event_receiver: Receiver<Event>,
    uid: String,
    users: Arc<Mutex<Stateful<HashMap<String, User>>>>,
) {
    let (mut sender, mut _receiver) = stream.split();
    while let Ok(event) = event_receiver.recv().await {
        let do_send = match event.event_inner {
            EventInner::Downloading(_) | EventInner::Setup(_) => true,
            _ => {
                let users = users.lock().await;
                is_authorized(
                    users.get_ref().get(&uid).unwrap(),
                    &event.instance_uuid,
                    Permission::CanViewInstance,
                )
            }
        };
        if do_send {
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
