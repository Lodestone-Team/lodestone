use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures::{SinkExt, StreamExt};
use log::error;
use tokio::sync::broadcast::Receiver;

use crate::{events::Event, AppState};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
) -> impl IntoResponse {
    let event_receiver = state.event_broadcaster.subscribe();
    ws.on_upgrade(|socket| websocket(socket, event_receiver))
}

async fn websocket(stream: WebSocket, mut event_receiver: Receiver<Event>) {
    let (mut sender, mut _receiver) = stream.split();

    while let Ok(event) = event_receiver.recv().await {
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
