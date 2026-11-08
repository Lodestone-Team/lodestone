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
    auth::user::{User, UserAction},
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
    Ok(ws.on_upgrade(move |stream| monitor_ws(stream, history, instance, state, requester, uuid)))
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

/// Whether `user` (as the stream authenticated) may still view the instance: the user still
/// exists and has the permission, and an API key is still valid and allows it.
async fn still_allowed(state: &AppState, user: &User, uuid: &InstanceUuid) -> bool {
    let Some(user) = state.users_manager.read().await.refresh_user(user) else {
        return false;
    };
    user.try_action(&UserAction::ViewInstance(uuid.clone()), state.safe_mode())
        .is_ok()
}

async fn monitor_ws(
    stream: WebSocket,
    history: Vec<MonitorReport>,
    instance: GameInstance,
    state: AppState,
    user: User,
    uuid: InstanceUuid,
) {
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
                if !still_allowed(&state, &user, &uuid).await {
                    debug!("Closing a monitor stream whose user may no longer view the instance");
                    break;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::{api_key::ApiKeyScope, permission::UserPermission},
        events::CausedBy,
        global_settings::GlobalSettingsData,
    };

    #[tokio::test]
    async fn monitor_streams_stop_when_the_key_or_permission_goes() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(
            dir.path(),
            GlobalSettingsData {
                safe_mode: false,
                code_execution_enabled: false,
                ..Default::default()
            },
        )
        .await;
        let uuid = InstanceUuid::from("DOCKER-monitored".to_string());
        let member = User::new(
            "member".to_string(),
            "pw",
            false,
            false,
            UserPermission {
                can_view_instance: [uuid.clone()].into(),
                ..Default::default()
            },
        );
        let mut users = state.users_manager.write().await;
        users
            .add_user(member.clone(), CausedBy::System)
            .await
            .unwrap();
        let key = users
            .create_api_key(
                &member.uid,
                "ci".to_string(),
                None,
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await
            .unwrap();
        let key_user = users.try_auth(&key.key).unwrap();
        drop(users);
        assert!(still_allowed(&state, &key_user, &uuid).await);
        assert!(still_allowed(&state, &member, &uuid).await);

        // the permission goes: both the session and the key stream stop
        state
            .users_manager
            .write()
            .await
            .modify_permissions(
                &member.uid,
                |p| p.can_view_instance.clear(),
                CausedBy::System,
            )
            .await
            .unwrap();
        assert!(!still_allowed(&state, &key_user, &uuid).await);
        assert!(!still_allowed(&state, &member, &uuid).await);

        // the permission is back, then the key is revoked: only the key stream stops
        state
            .users_manager
            .write()
            .await
            .modify_permissions(
                &member.uid,
                |p| {
                    p.can_view_instance.insert(uuid.clone());
                },
                CausedBy::System,
            )
            .await
            .unwrap();
        assert!(still_allowed(&state, &key_user, &uuid).await);
        state
            .users_manager
            .write()
            .await
            .revoke_api_key(&member.uid, &key.info.id, CausedBy::System)
            .await
            .unwrap();
        assert!(!still_allowed(&state, &key_user, &uuid).await);
        assert!(still_allowed(&state, &member, &uuid).await);
    }
}
