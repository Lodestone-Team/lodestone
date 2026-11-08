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
        user::{User, UserAction, UsersManager},
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
/// stream must close: also when the stream's API key has expired, or was revoked (which
/// sends a user event).
async fn update_cached_user(
    event: &Event,
    user: &mut User,
    users_manager: &RwLock<UsersManager>,
) -> bool {
    if user.api_key_expired(crate::auth::api_key::now()) {
        return false;
    }
    match user_event_action(event, &user.uid) {
        UserEventAction::Continue => true,
        UserEventAction::Close => false,
        // re-derived from how the stream authenticated, so a custom key keeps only its
        // scope and a revoked key closes the stream
        UserEventAction::Refresh => match users_manager.read().await.refresh_user(user) {
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

/// Drops the buffered console output of one instance, for every client: a client that opens
/// the console afterwards no longer gets the old lines. Needs the permission to use the
/// instance's console, like sending a command does.
pub async fn clear_console_buffer(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::AccessConsole(uuid.clone()))
        .await?;
    state.clear_console_buffer(&uuid).await;
    Ok(Json(()))
}

pub fn get_events_routes(state: AppState) -> Router {
    Router::new()
        .route("/events/{uuid}/stream", get(event_stream))
        .route("/events/{uuid}/buffer", get(get_event_buffer))
        .route("/events/search", get(get_event_search))
        .route("/instance/{uuid}/console/stream", get(console_stream))
        .route(
            "/instance/{uuid}/console/buffer",
            get(get_console_buffer).delete(clear_console_buffer),
        )
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

    #[tokio::test]
    async fn streams_close_once_their_api_key_expires() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = crate::event_broadcaster::EventBroadcaster::new(8);
        let users_manager = RwLock::new(UsersManager::new(
            tx,
            Default::default(),
            dir.path().join("users.json"),
        ));
        let unrelated = user_event(&UserId::default(), UserEventInner::UserCreated);
        let mut user = User::new("u".to_string(), "pw", false, false, Default::default());
        // a session never expires here
        assert!(update_cached_user(&unrelated, &mut user, &users_manager).await);
        let now = crate::auth::api_key::now();
        user.auth_source = crate::auth::user::AuthSource::ApiKey {
            id: "abcdefghij12".to_string(),
            name: "ci".to_string(),
            expires_at: Some(now + 3600),
        };
        assert!(update_cached_user(&unrelated, &mut user, &users_manager).await);
        user.auth_source = crate::auth::user::AuthSource::ApiKey {
            id: "abcdefghij12".to_string(),
            name: "ci".to_string(),
            expires_at: Some(now - 1),
        };
        // any event closes the stream, not only events about the user
        assert!(!update_cached_user(&unrelated, &mut user, &users_manager).await);
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

    mod clear_console_buffer {
        use reqwest::{Method, StatusCode};
        use ringbuffer::AllocRingBuffer;
        use serde_json::Value;

        use super::*;
        use crate::{
            auth::{api_key::ApiKeyScope, permission::UserPermission},
            global_settings::GlobalSettingsData,
            test_util::{serve_api, test_app_state},
        };

        fn instance(name: &str) -> InstanceUuid {
            InstanceUuid::from(format!("DOCKER-{name}"))
        }

        struct Core {
            _dir: tempfile::TempDir,
            state: AppState,
            base: String,
            owner_jwt: String,
        }

        async fn core(safe_mode: bool) -> Core {
            let dir = tempfile::tempdir().unwrap();
            let state = test_app_state(
                dir.path(),
                GlobalSettingsData {
                    safe_mode,
                    ..Default::default()
                },
            )
            .await;
            let owner = add_user(&state, "owner", true, Default::default()).await;
            let owner_jwt = owner.create_jwt().unwrap().to_string();
            let base = serve_api(state.clone()).await;
            for name in ["a", "b"] {
                for line in 0..3 {
                    push_line(&state, &instance(name), &format!("{name} line {line}")).await;
                }
            }
            Core {
                _dir: dir,
                state,
                base,
                owner_jwt,
            }
        }

        async fn push_line(state: &AppState, uuid: &InstanceUuid, line: &str) {
            let event = Event::new_instance_output(uuid.clone(), "i".to_string(), line.to_string());
            state
                .console_out_buffer
                .lock()
                .await
                .entry(uuid.clone())
                .or_insert_with(|| AllocRingBuffer::new(1024))
                .enqueue(event);
        }

        async fn add_user(
            state: &AppState,
            name: &str,
            is_owner: bool,
            permissions: UserPermission,
        ) -> User {
            let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
            state
                .users_manager
                .write()
                .await
                .add_user(user.clone(), CausedBy::System)
                .await
                .unwrap();
            user
        }

        fn console_of(names: &[&str]) -> UserPermission {
            UserPermission {
                can_access_instance_console: names.iter().map(|n| instance(n)).collect(),
                ..Default::default()
            }
        }

        impl Core {
            async fn request(&self, method: Method, uuid: &str, token: Option<&str>) -> StatusCode {
                let url = format!("{}/instance/{}/console/buffer", self.base, instance(uuid));
                let mut request = reqwest::Client::new().request(method, url);
                if let Some(token) = token {
                    request = request.bearer_auth(token);
                }
                request.send().await.unwrap().status()
            }

            /// The lines the owner sees in an instance's console buffer.
            async fn lines(&self, uuid: &str) -> Vec<String> {
                let url = format!("{}/instance/{}/console/buffer", self.base, instance(uuid));
                let events: Vec<Value> = reqwest::Client::new()
                    .get(url)
                    .bearer_auth(&self.owner_jwt)
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                events
                    .iter()
                    .map(|e| {
                        e["event_inner"]["instance_event_inner"]["message"]
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .collect()
            }

            async fn token_for(&self, name: &str, permissions: UserPermission) -> String {
                let user = add_user(&self.state, name, false, permissions).await;
                user.create_jwt().unwrap().to_string()
            }
        }

        #[tokio::test]
        async fn clears_only_the_target_instance() {
            let core = core(false).await;
            core.state
                .events_buffer
                .lock()
                .await
                .enqueue(Event::new_instance_output(
                    instance("a"),
                    "i".to_string(),
                    "not console".to_string(),
                ));
            assert_eq!(core.lines("a").await.len(), 3);
            assert_eq!(core.lines("b").await.len(), 3);

            let status = core
                .request(Method::DELETE, "a", Some(&core.owner_jwt))
                .await;
            assert_eq!(status, StatusCode::OK);

            assert!(core.lines("a").await.is_empty());
            assert_eq!(core.lines("b").await.len(), 3);
            assert_eq!(core.state.events_buffer.lock().await.len(), 1);
            // clearing again, or an instance with no buffer, is fine
            let status = core
                .request(Method::DELETE, "a", Some(&core.owner_jwt))
                .await;
            assert_eq!(status, StatusCode::OK);
            let status = core
                .request(Method::DELETE, "never-seen", Some(&core.owner_jwt))
                .await;
            assert_eq!(status, StatusCode::OK);
        }

        #[tokio::test]
        async fn new_output_is_buffered_again_after_a_clear() {
            let core = core(false).await;
            core.state.clear_console_buffer(&instance("a")).await;
            push_line(&core.state, &instance("a"), "after").await;
            assert_eq!(core.lines("a").await, ["after"]);
        }

        #[tokio::test]
        async fn reading_never_clears() {
            let core = core(false).await;
            for _ in 0..2 {
                let status = core.request(Method::GET, "a", Some(&core.owner_jwt)).await;
                assert_eq!(status, StatusCode::OK);
            }
            assert_eq!(core.lines("a").await.len(), 3);
            // the route the original proposal used does not exist
            let url = format!("{}/instance/{}/console/clear", core.base, instance("a"));
            for method in [Method::GET, Method::POST, Method::DELETE] {
                let status = reqwest::Client::new()
                    .request(method, &url)
                    .bearer_auth(&core.owner_jwt)
                    .send()
                    .await
                    .unwrap()
                    .status();
                assert_eq!(status, StatusCode::NOT_FOUND);
            }
            assert_eq!(core.lines("a").await.len(), 3);
        }

        #[tokio::test]
        async fn needs_the_console_permission_of_that_instance() {
            let core = core(false).await;

            // no token, a bad token
            assert_eq!(
                core.request(Method::DELETE, "a", None).await,
                StatusCode::BAD_REQUEST
            );
            assert_eq!(
                core.request(Method::DELETE, "a", Some("bogus")).await,
                StatusCode::UNAUTHORIZED
            );

            // other permissions on this instance, or the console of another instance
            let viewer = core
                .token_for(
                    "viewer",
                    UserPermission {
                        can_view_instance: [instance("a")].into(),
                        can_access_instance_setting: [instance("a")].into(),
                        can_start_instance: [instance("a")].into(),
                        ..console_of(&["b"])
                    },
                )
                .await;
            assert_eq!(
                core.request(Method::DELETE, "a", Some(&viewer)).await,
                StatusCode::FORBIDDEN
            );
            assert_eq!(core.lines("a").await.len(), 3);
            assert_eq!(core.lines("b").await.len(), 3);

            // the permission for exactly this instance is enough, and only clears it
            let console = core.token_for("console", console_of(&["a"])).await;
            assert_eq!(
                core.request(Method::DELETE, "b", Some(&console)).await,
                StatusCode::FORBIDDEN
            );
            assert_eq!(core.lines("b").await.len(), 3);
            assert_eq!(
                core.request(Method::DELETE, "a", Some(&console)).await,
                StatusCode::OK
            );
            assert!(core.lines("a").await.is_empty());
            assert_eq!(core.lines("b").await.len(), 3);
        }

        #[tokio::test]
        async fn works_in_safe_mode_like_the_console_does() {
            let core = core(true).await;
            let console = core.token_for("console", console_of(&["a"])).await;
            assert_eq!(
                core.request(Method::DELETE, "a", Some(&console)).await,
                StatusCode::OK
            );
            assert!(core.lines("a").await.is_empty());
        }

        #[tokio::test]
        async fn api_keys_are_limited_to_their_scope() {
            let core = core(false).await;
            let user = add_user(&core.state, "keyed", false, console_of(&["a", "b"])).await;
            let mut keys = Vec::new();
            for (name, scope) in [
                ("full", ApiKeyScope::Full),
                (
                    "view",
                    ApiKeyScope::Custom {
                        permissions: Box::new(UserPermission {
                            can_view_instance: [instance("a")].into(),
                            ..Default::default()
                        }),
                    },
                ),
                (
                    "console-b",
                    ApiKeyScope::Custom {
                        permissions: Box::new(console_of(&["b"])),
                    },
                ),
                (
                    "console-a",
                    ApiKeyScope::Custom {
                        permissions: Box::new(console_of(&["a"])),
                    },
                ),
            ] {
                let created = core
                    .state
                    .users_manager
                    .write()
                    .await
                    .create_api_key(&user.uid, name.to_string(), None, scope, CausedBy::System)
                    .await
                    .unwrap();
                keys.push(created.key);
            }
            let [full, view_only, console_b, console_a] = &keys[..] else {
                unreachable!()
            };

            // the user holds the permission, but these keys do not
            for key in [view_only, console_b] {
                assert_eq!(
                    core.request(Method::DELETE, "a", Some(key)).await,
                    StatusCode::FORBIDDEN
                );
            }
            assert_eq!(core.lines("a").await.len(), 3);

            assert_eq!(
                core.request(Method::DELETE, "a", Some(console_a)).await,
                StatusCode::OK
            );
            assert!(core.lines("a").await.is_empty());
            assert_eq!(core.lines("b").await.len(), 3);

            assert_eq!(
                core.request(Method::DELETE, "b", Some(full)).await,
                StatusCode::OK
            );
            assert!(core.lines("b").await.is_empty());
        }
    }
}
