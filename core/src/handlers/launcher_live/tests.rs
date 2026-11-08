//! Launcher keys and the console: the opt-ins chosen at sign-in, always within the
//! user's own permissions, against a core on a loopback port. The instances are
//! containers on the Docker watch list, which exist without setting up a server.

use futures::StreamExt;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{api_key::ApiKeyScope, permission::UserPermission, user::User},
    events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner},
    global_settings::GlobalSettingsData,
    test_util::{serve_api, test_app_state},
    types::Snowflake,
};

const MINE: &str = "DOCKER-launcher-live-mine";
const OTHER: &str = "DOCKER-launcher-live-other";

fn mine() -> InstanceUuid {
    InstanceUuid::from(MINE.to_string())
}

fn other() -> InstanceUuid {
    InstanceUuid::from(OTHER.to_string())
}

struct Core {
    _dir: tempfile::TempDir,
    state: AppState,
    base: String,
    client: reqwest::Client,
    owner_jwt: String,
}

async fn core() -> Core {
    crate::test_util::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("docker_bridge.json"),
        serde_json::to_string(&[MINE, OTHER]).unwrap(),
    )
    .unwrap();
    let state = test_app_state(
        dir.path(),
        GlobalSettingsData {
            code_execution_enabled: false,
            ..Default::default()
        },
    )
    .await;
    let owner = add_user(&state, "owner", true, Default::default()).await;
    let owner_jwt = owner.create_jwt().unwrap().to_string();
    let base = serve_api(state.clone()).await;
    // a line in each console's history
    {
        let mut buffers = state.console_out_buffer.lock().await;
        for uuid in [mine(), other()] {
            let mut buffer = ringbuffer::AllocRingBuffer::new(16);
            ringbuffer::RingBuffer::enqueue(
                &mut buffer,
                Event::new_instance_output(uuid.clone(), "test".into(), format!("line of {uuid}")),
            );
            buffers.insert(uuid, buffer);
        }
    }
    Core {
        _dir: dir,
        state,
        base,
        client: reqwest::Client::new(),
        owner_jwt,
    }
}

async fn add_user(state: &AppState, name: &str, owner: bool, perms: UserPermission) -> User {
    let user = User::new(name.to_string(), "pw", owner, false, perms);
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    user
}

fn launcher(read: bool, write: bool) -> Value {
    let mut scope = json!({ "type": "launcher" });
    if read {
        scope["console_read"] = json!(true);
    }
    if write {
        scope["console_write"] = json!(true);
    }
    scope
}

impl Core {
    async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    async fn create_key(&self, session: &str, scope: Value) -> (String, Value) {
        let (status, body) = self
            .call(
                Method::POST,
                "/user/api_keys",
                session,
                Some(json!({ "name": "Lodestone Launcher on Test Mac", "expires_at": null, "scope": scope })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        (
            body["key"].as_str().unwrap().to_string(),
            body["info"].clone(),
        )
    }

    /// The console history `token` gets for `uuid`.
    async fn buffer(&self, token: &str, uuid: &str) -> usize {
        let (status, body) = self
            .call(
                Method::GET,
                &format!("/instance/{uuid}/console/buffer"),
                token,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body.as_array().unwrap().len()
    }

    /// The status of sending a command: 403 when refused; anything else got past the
    /// permission check (the watched containers have no console, so 404 or similar).
    async fn send_command(&self, token: &str, uuid: &str) -> StatusCode {
        self.call(
            Method::POST,
            &format!("/instance/{uuid}/console"),
            token,
            Some(json!("say hi")),
        )
        .await
        .0
    }

    async fn access(&self, token: &str, uuid: &str) -> (StatusCode, Value) {
        self.call(
            Method::GET,
            &format!("/launcher/instances/{uuid}/access"),
            token,
            None,
        )
        .await
    }

    fn ws_url(&self, path_and_query: &str) -> String {
        format!(
            "{}{path_and_query}",
            self.base.replacen("http://", "ws://", 1)
        )
    }
}

fn instance_event(uuid: InstanceUuid, inner: InstanceEventInner) -> Event {
    Event {
        event_inner: EventInner::InstanceEvent(InstanceEvent {
            instance_uuid: uuid,
            instance_name: "test".into(),
            instance_event_inner: inner,
        }),
        details: String::new(),
        snowflake: Snowflake::default(),
        caused_by: CausedBy::System,
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_text(ws: &mut Ws) -> String {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(30), ws.next())
        .await
        .expect("timed out waiting for a WebSocket frame")
        .expect("the stream ended")
        .expect("WebSocket error");
    frame.to_text().unwrap().to_string()
}

#[test]
fn keys_from_before_the_opt_ins_load_without_console() {
    let old: ApiKeyScope = serde_json::from_str(r#"{"type":"launcher"}"#).unwrap();
    assert_eq!(
        old,
        ApiKeyScope::Launcher {
            console_read: false,
            console_write: false
        }
    );
    // saved and sent as before
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        json!({ "type": "launcher" })
    );
    let both: ApiKeyScope = serde_json::from_value(
        json!({ "type": "launcher", "console_read": true, "console_write": true }),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(&both).unwrap()["console_write"], true);
}

#[tokio::test]
async fn a_launcher_key_without_opt_ins_gets_no_console() {
    let core = core().await;
    // M8a's launcher sends exactly this
    let (key, info) = core
        .create_key(&core.owner_jwt, json!({ "type": "launcher" }))
        .await;
    assert_eq!(info["scope"], launcher(false, false));

    assert_eq!(core.buffer(&key, MINE).await, 0);
    assert_eq!(core.send_command(&key, MINE).await, StatusCode::FORBIDDEN);
    let (status, access) = core.access(&key, MINE).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        access,
        json!({ "view": true, "start": true, "stop": true, "console_read": false, "console_write": false })
    );
    // the owner's own session still reads and writes
    assert_eq!(core.buffer(&core.owner_jwt, MINE).await, 1);
    assert_ne!(
        core.send_command(&core.owner_jwt, MINE).await,
        StatusCode::FORBIDDEN
    );

    // the console stream sends nothing; the event stream leaves out what was typed but
    // still has state changes
    let filter = json!({ "bearer_token": key }).to_string();
    let filter: String = url::form_urlencoded::byte_serialize(filter.as_bytes()).collect();
    let (mut events, _) = tokio_tungstenite::connect_async(
        core.ws_url(&format!("/events/all/stream?filter={filter}")),
    )
    .await
    .unwrap();
    core.state.event_broadcaster.send(instance_event(
        mine(),
        InstanceEventInner::InstanceInput {
            message: "op someone".into(),
        },
    ));
    core.state.event_broadcaster.send(instance_event(
        mine(),
        InstanceEventInner::StateTransition {
            to: crate::traits::t_server::State::Running,
        },
    ));
    let first: Value = serde_json::from_str(&next_text(&mut events).await).unwrap();
    assert_eq!(
        first["event_inner"]["instance_event_inner"]["type"],
        "StateTransition"
    );
}

#[tokio::test]
async fn the_opt_ins_are_separate() {
    let core = core().await;
    let (read_only, _) = core
        .create_key(&core.owner_jwt, launcher(true, false))
        .await;
    let (write_only, _) = core
        .create_key(&core.owner_jwt, launcher(false, true))
        .await;
    let (both, info) = core.create_key(&core.owner_jwt, launcher(true, true)).await;
    assert_eq!(info["scope"], launcher(true, true));

    assert_eq!(core.buffer(&read_only, MINE).await, 1);
    assert_eq!(
        core.send_command(&read_only, MINE).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(core.buffer(&write_only, MINE).await, 0);
    assert_ne!(
        core.send_command(&write_only, MINE).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(core.buffer(&both, MINE).await, 1);
    assert_ne!(core.send_command(&both, MINE).await, StatusCode::FORBIDDEN);
    assert_eq!(
        core.access(&read_only, MINE).await.1,
        json!({ "view": true, "start": true, "stop": true, "console_read": true, "console_write": false })
    );

    // still nothing beyond the launcher scope
    for path in ["/instance/DOCKER-launcher-live-mine/settings", "/user/list"] {
        let (status, body) = core.call(Method::GET, path, &both, None).await;
        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::NOT_FOUND,
            "{path}: {status} {body}"
        );
    }
    let user = core
        .state
        .users_manager
        .read()
        .await
        .try_auth(&both)
        .unwrap();
    assert!(!user.is_owner && !user.is_admin);
    assert!(!user.can_perform_action(&UserAction::AccessSetting(mine())));
    assert!(!user.can_perform_action(&UserAction::ReadInstanceFile(mine())));

    // the console stream with the read opt-in
    let token: String =
        url::form_urlencoded::byte_serialize(format!("Bearer {read_only}").as_bytes()).collect();
    let (mut console, _) = tokio_tungstenite::connect_async(
        core.ws_url(&format!("/instance/{MINE}/console/stream?token={token}")),
    )
    .await
    .unwrap();
    core.state
        .event_broadcaster
        .send(Event::new_instance_output(
            mine(),
            "test".into(),
            "live line".into(),
        ));
    assert!(next_text(&mut console).await.contains("live line"));
}

#[tokio::test]
async fn opt_ins_never_go_beyond_the_users_permissions() {
    let core = core().await;
    let viewer = add_user(
        &core.state,
        "viewer",
        false,
        UserPermission {
            can_view_instance: [mine(), other()].into(),
            can_access_instance_console: [mine()].into(),
            ..Default::default()
        },
    )
    .await;
    let session = viewer.create_jwt().unwrap().to_string();
    let (key, _) = core.create_key(&session, launcher(true, true)).await;

    // the console of `mine` only
    assert_eq!(core.buffer(&key, MINE).await, 1);
    assert_ne!(core.send_command(&key, MINE).await, StatusCode::FORBIDDEN);
    assert_eq!(core.buffer(&key, OTHER).await, 0);
    assert_eq!(core.send_command(&key, OTHER).await, StatusCode::FORBIDDEN);
    assert_eq!(core.access(&key, OTHER).await.1["console_read"], false);
    // never start or stop: the viewer can't
    assert_eq!(core.access(&key, MINE).await.1["start"], false);
    // the viewer's session keeps the core's rule that viewers read the console
    assert_eq!(core.buffer(&session, OTHER).await, 1);

    // losing the permission takes effect on the key at once
    core.state
        .users_manager
        .write()
        .await
        .update_permissions(
            &viewer.uid,
            UserPermission {
                can_view_instance: [mine()].into(),
                ..Default::default()
            },
            CausedBy::System,
        )
        .await
        .unwrap();
    assert_eq!(core.buffer(&key, MINE).await, 0);
    assert_eq!(core.send_command(&key, MINE).await, StatusCode::FORBIDDEN);
    let (status, _) = core.access(&key, OTHER).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no longer viewable");
}
