use axum::{
    extract::{ws::Message, ws::WebSocket, Path, Query, State, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use ringbuffer::RingBuffer;
use serde::Deserialize;
use tracing::{debug, error};

use crate::{
    auth::user::UserAction,
    error::Error,
    prelude::GameInstance,
    traits::{t_server::MonitorReport, t_server::TServer},
    AppState,
};

use super::util::{authenticate_query_token, get_instance, require};

use crate::types::InstanceUuid;

#[derive(Deserialize)]
pub struct MonitorQuery {
    /// `Bearer <jwt>`, like the console WebSocket: browsers cannot set headers on WebSockets.
    token: String,
}

pub async fn monitor(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    query: Query<MonitorQuery>,
) -> Result<Response, Error> {
    let requester = authenticate_query_token(&state, &query.token).await?;
    require(&state, &requester, UserAction::ViewInstance(uuid.clone())).await?;
    let instance = get_instance(&state, &uuid)?;
    // copy the buffered reports now, so the lock is not held while sending them
    let history: Vec<MonitorReport> = state
        .monitor_buffer
        .lock()
        .await
        .get(&uuid)
        .map(|buffer| buffer.iter().cloned().collect())
        .unwrap_or_default();
    Ok(ws.on_upgrade(move |stream| monitor_ws(stream, history, instance)))
}

async fn send_report(
    tx: &mut futures::stream::SplitSink<WebSocket, Message>,
    report: &MonitorReport,
) -> bool {
    let text = match serde_json::to_string(report) {
        Ok(text) => text,
        Err(e) => {
            error!("Failed to serialize monitor report: {}", e);
            return true;
        }
    };
    if let Err(e) = tx.send(Message::Text(text.into())).await {
        debug!("Error sending monitor report: {}", e);
        return false;
    }
    true
}

async fn monitor_ws(stream: WebSocket, history: Vec<MonitorReport>, instance: GameInstance) {
    let (mut tx, mut rx) = stream.split();
    for report in &history {
        if !send_report(&mut tx, report).await {
            return;
        }
    }
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let report = instance.monitor().await;
                if !send_report(&mut tx, &report).await {
                    break;
                }
            }
            msg = rx.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

pub fn get_monitor_routes(state: AppState) -> Router {
    Router::new()
        .route("/monitor/{uuid}", get(monitor))
        .with_state(state)
}
