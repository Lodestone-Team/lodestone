use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, Path, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router,
};
use color_eyre::eyre::eyre;
use futures::{SinkExt, StreamExt};
use ringbuffer::{AllocRingBuffer, RingBufferExt};
use tokio::sync::Mutex;
use tracing::error;

use crate::{
    error::Error,
    prelude::GameInstance,
    traits::{t_server::MonitorReport, t_server::TServer},
    types::InstanceUuid,
    AppState,
};

pub async fn monitor(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Response, Error> {
    let instance = state
        .instances
        .get(&uuid)
        .ok_or_else(|| Error {
            kind: crate::error::ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .to_owned();
    Ok(ws.on_upgrade(move |stream| {
        monitor_ws(
            stream,
            state.monitor_buffer.clone(),
            instance.to_owned(),
            uuid,
        )
    }))
}

async fn monitor_ws(
    stream: WebSocket,
    monitor_buffer: Arc<Mutex<HashMap<InstanceUuid, AllocRingBuffer<MonitorReport>>>>,
    instance: GameInstance,
    uuid: InstanceUuid,
) {
    let (mut tx, mut rx) = stream.split();
    if let Some(buffer) = monitor_buffer.lock().await.get(&uuid) {
        for report in buffer.iter() {
            if let Err(e) = tx
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&report).unwrap(),
                ))
                .await
            {
                error!("1 Error sending monitor report: {}", e);
                break;
            }
        }
    }
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let monitor = instance.monitor().await;
                if let Err(e) = tx
                    .send(axum::extract::ws::Message::Text(
                        serde_json::to_string(&monitor).unwrap(),
                    ))
                    .await
                {
                    error!("2 Error sending monitor report: {}", e);
                    break;
                }
            }
            msg = rx.next() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }
}

pub fn get_monitor_routes(state: AppState) -> Router {
    Router::new()
        .route("/monitor/:uuid", get(monitor))
        .with_state(state)
}
