
use axum::{
    extract::{ws::WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};
use futures::{SinkExt, StreamExt};
use log::error;

use crate::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))

}

async fn websocket(stream: WebSocket, state: AppState) {
    let (mut sender, mut _receiver) = stream.split();
    let mut event_receiver = state.event_broadcaster.subscribe();

    while let Ok(event) = event_receiver.recv().await {
        if let Err(e) = sender.send(axum::extract::ws::Message::Text(event)).await {
            error!("Failed to send event: {}", e);
            break;
        }
    }
}
