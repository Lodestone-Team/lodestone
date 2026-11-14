//! `/user/api_keys`: creating, listing and revoking API keys. Every route here needs a
//! signed-in session: an API key cannot manage keys, not even a full one.

use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Json, Router,
};
use color_eyre::eyre::eyre;

use crate::{
    auth::{
        api_key::{self, ApiKeyInfo, ApiKeyScope, CreateApiKeyRequest, CreatedApiKey},
        user::User,
        user_id::UserId,
    },
    error::{Error, ErrorKind},
    AppState,
};

use super::util::AuthedUser;

/// Fails unless every instance a custom scope names exists.
async fn check_scope_instances(state: &AppState, scope: &ApiKeyScope) -> Result<(), Error> {
    for uuid in api_key::scope_instances(scope) {
        if !state.instances.contains_key(uuid) && !state.docker_bridge.is_watched(uuid).await {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Server {uuid} does not exist"),
            });
        }
    }
    Ok(())
}

/// The requester's own keys, newest first.
async fn list_own_keys(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<Vec<ApiKeyInfo>>, Error> {
    requester.require_session()?;
    Ok(Json(
        state
            .users_manager
            .read()
            .await
            .list_api_keys(&requester.uid),
    ))
}

async fn create_key(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<CreatedApiKey>, Error> {
    requester.require_session()?;
    let name = api_key::validate_name(&request.name)?;
    if let Some(expires_at) = request.expires_at {
        if expires_at <= api_key::now() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("The expiry date must be in the future"),
            });
        }
    }
    let mut scope = request.scope;
    if let ApiKeyScope::Custom { permissions } = &mut scope {
        // they grant nothing since macros and extensions became owner only
        permissions.clear_owner_only_legacy_fields();
    }
    check_scope_instances(&state, &scope).await?;
    let created = state
        .users_manager
        .write()
        .await
        .create_api_key(
            &requester.uid,
            name,
            request.expires_at,
            scope,
            requester.caused_by(),
        )
        .await?;
    Ok(Json(created))
}

async fn revoke_own_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester.require_session()?;
    state
        .users_manager
        .write()
        .await
        .revoke_api_key(&requester.uid, &id, requester.caused_by())
        .await?;
    Ok(Json(()))
}

/// For incident response, the owner and admins may see and revoke other users' keys.
/// Admins may not touch the owner's or other admins' keys, the same rule as for managing
/// permissions.
fn require_manage_keys_of(requester: &User, target: &User) -> Result<(), Error> {
    let allowed = requester.is_owner
        || (requester.is_admin
            && (requester.uid == target.uid || !(target.is_owner || target.is_admin)));
    if allowed {
        Ok(())
    } else {
        Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You don't have permission to manage this user's API keys"),
        })
    }
}

fn target_user(state_users: &crate::auth::user::UsersManager, uid: &UserId) -> Result<User, Error> {
    state_users.get_user(uid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("User not found"),
    })
}

async fn list_user_keys(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
) -> Result<Json<Vec<ApiKeyInfo>>, Error> {
    requester.require_session()?;
    let users_manager = state.users_manager.read().await;
    let target = target_user(&users_manager, &uid)?;
    require_manage_keys_of(&requester, &target)?;
    Ok(Json(users_manager.list_api_keys(&uid)))
}

async fn revoke_user_key(
    State(state): State<AppState>,
    Path((uid, id)): Path<(UserId, String)>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester.require_session()?;
    let mut users_manager = state.users_manager.write().await;
    let target = target_user(&users_manager, &uid)?;
    require_manage_keys_of(&requester, &target)?;
    users_manager
        .revoke_api_key(&uid, &id, requester.caused_by())
        .await?;
    Ok(Json(()))
}

pub fn get_api_key_routes(state: AppState) -> Router {
    Router::new()
        .route("/user/api_keys", get(list_own_keys).post(create_key))
        .route("/user/api_keys/{id}", delete(revoke_own_key))
        .route("/user/{uid}/api_keys", get(list_user_keys))
        .route("/user/{uid}/api_keys/{id}", delete(revoke_user_key))
        .with_state(state)
}

/// The routes, over HTTP and WebSocket, against a core on a loopback port. Covers every
/// way a request authenticates: the `AuthedUser` extractor, handlers that call `try_auth`
/// themselves, WebSocket query tokens, the event stream's `bearer_token`, and download
/// links.
#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use reqwest::{Method, StatusCode};
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite;

    use super::*;
    use crate::{
        auth::permission::UserPermission,
        events::{CausedBy, Event, EventInner, UserEventInner},
        global_settings::GlobalSettingsData,
        test_util::{serve_api, test_app_state},
        types::InstanceUuid,
    };

    /// A container on the Docker watch list: the only kind of instance that exists without
    /// setting up a real server.
    const INSTANCE: &str = "DOCKER-api-key-test";

    struct Core {
        _dir: tempfile::TempDir,
        state: AppState,
        base: String,
        client: reqwest::Client,
        owner: User,
        owner_jwt: String,
    }

    fn instance() -> InstanceUuid {
        InstanceUuid::from(INSTANCE.to_string())
    }

    fn settings(safe_mode: bool) -> GlobalSettingsData {
        GlobalSettingsData {
            safe_mode,
            // nothing here may run code, and the atom test relies on the switch being off
            code_execution_enabled: false,
            ..Default::default()
        }
    }

    async fn core_with(settings: GlobalSettingsData) -> Core {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("docker_bridge.json"),
            serde_json::to_string(&[INSTANCE]).unwrap(),
        )
        .unwrap();
        let state = test_app_state(dir.path(), settings).await;
        let owner = add_user(&state, "owner", true, false, Default::default()).await;
        let owner_jwt = owner.create_jwt().unwrap().to_string();
        let base = serve_api(state.clone()).await;
        Core {
            _dir: dir,
            state,
            base,
            client: reqwest::Client::new(),
            owner,
            owner_jwt,
        }
    }

    async fn core() -> Core {
        core_with(settings(false)).await
    }

    async fn add_user(
        state: &AppState,
        name: &str,
        is_owner: bool,
        is_admin: bool,
        permissions: UserPermission,
    ) -> User {
        let user = User::new(name.to_string(), "pw", is_owner, is_admin, permissions);
        state
            .users_manager
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user
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

        /// Creates a key through the API, with `token` (a session).
        async fn create_key(&self, token: &str, name: &str, scope: Value) -> (String, Value) {
            let (status, body) = self
                .call(
                    Method::POST,
                    "/user/api_keys",
                    token,
                    Some(json!({ "name": name, "expires_at": null, "scope": scope })),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            (
                body["key"].as_str().unwrap().to_string(),
                body["info"].clone(),
            )
        }

        fn ws_url(&self, path_and_query: &str) -> String {
            format!(
                "{}{path_and_query}",
                self.base.replacen("http://", "ws://", 1)
            )
        }
    }

    fn custom(permissions: UserPermission) -> Value {
        json!({ "type": "custom", "permissions": permissions })
    }

    fn message(body: &Value) -> String {
        body["causes"][0].as_str().unwrap_or_default().to_string()
    }

    async fn ws_status(url: &str) -> Result<(), StatusCode> {
        match tokio_tungstenite::connect_async(url).await {
            Ok(_) => Ok(()),
            Err(tungstenite::Error::Http(response)) => Err(response.status()),
            Err(e) => panic!("unexpected WebSocket error: {e}"),
        }
    }

    type Ws = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >;

    /// The next frame, or `None` once the stream ends. Fails the test instead of hanging
    /// if nothing arrives in time.
    async fn next_frame(ws: &mut Ws) -> Option<Result<tungstenite::Message, tungstenite::Error>> {
        tokio::time::timeout(std::time::Duration::from_secs(30), ws.next())
            .await
            .expect("timed out waiting for a WebSocket frame")
    }

    fn query_token(token: &str) -> String {
        url::form_urlencoded::byte_serialize(format!("Bearer {token}").as_bytes()).collect()
    }

    fn view_only() -> UserPermission {
        UserPermission {
            can_view_instance: [instance()].into(),
            ..Default::default()
        }
    }

    async fn login_with_key(core: &Core, key: &str) -> (StatusCode, Value) {
        let response = core
            .client
            .post(format!("{}/user/login_api_key", core.base))
            .json(&json!({ "key": key }))
            .send()
            .await
            .unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    #[tokio::test]
    async fn signing_in_with_a_key_keeps_its_scope() {
        let core = core().await;
        let (full, _) = core
            .create_key(&core.owner_jwt, "dashboard", json!({ "type": "full" }))
            .await;
        let (status, body) = login_with_key(&core, &format!("  {full}\n")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["user"]["username"], "owner");
        assert_eq!(body["user"]["is_owner"], true);
        // the key is then the session's token
        let (status, me) = core.call(Method::GET, "/user/info", &full, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["is_owner"], true);

        // a custom key signs in as the owner without owner powers, with only its permissions
        let (narrow, info) = core
            .create_key(&core.owner_jwt, "viewer", custom(view_only()))
            .await;
        let (status, body) = login_with_key(&core, &narrow).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"]["is_owner"], false);
        assert_eq!(body["user"]["is_admin"], false);
        assert_eq!(
            body["user"]["permissions"]["can_view_instance"],
            json!([INSTANCE])
        );
        assert_eq!(body["user"]["permissions"]["can_create_instance"], false);

        // revoking ends it at once
        let id = info["id"].as_str().unwrap().to_string();
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/api_keys/{id}"),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            login_with_key(&core, &narrow).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            core.call(Method::GET, "/user/info", &narrow, None).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn failed_key_sign_ins_are_limited() {
        let core = core().await;
        let (key, _) = core
            .create_key(&core.owner_jwt, "dashboard", json!({ "type": "full" }))
            .await;
        // the right id with wrong secrets
        let wrong = format!("{}{}", &key[..key.len() - 4], "AAAA");
        for _ in 0..crate::auth::login_limiter::MAX_FAILURES {
            assert_eq!(
                login_with_key(&core, &wrong).await.0,
                StatusCode::UNAUTHORIZED
            );
        }
        assert_eq!(
            login_with_key(&core, &wrong).await.0,
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            login_with_key(&core, "not a key").await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn key_sign_in_refuses_session_tokens() {
        let core = core().await;
        // a valid session JWT is not an API key
        let (status, _) = login_with_key(&core, &core.owner_jwt).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_list_and_revoke_with_a_session() {
        let core = core().await;
        let (key, info) = core
            .create_key(&core.owner_jwt, "  laptop  ", json!({ "type": "full" }))
            .await;
        assert!(key.starts_with("lsk_"));
        let id = info["id"].as_str().unwrap().to_string();
        assert_eq!(info["name"], "laptop");
        assert_eq!(info["display"], format!("lsk_{id}_…"));
        assert_eq!(info["uid"], json!(core.owner.uid));
        assert_eq!(info["scope"], json!({ "type": "full" }));
        assert_eq!(info["expires_at"], Value::Null);
        assert_eq!(info["last_used_at"], Value::Null);
        assert!(info["created_at"].is_i64());
        assert_eq!(info.as_object().unwrap().len(), 8, "{info}");

        let (_, second) = core
            .create_key(&core.owner_jwt, "ci", custom(view_only()))
            .await;

        // the key works, and is recorded as used
        let (status, me) = core.call(Method::GET, "/user/info", &key, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(me["username"], "owner");

        let (status, list) = core
            .call(Method::GET, "/user/api_keys", &core.owner_jwt, None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let list = list.as_array().unwrap();
        assert_eq!(list.len(), 2);
        // newest first: equal timestamps fall back to the id, so only check membership
        // when they tie
        if second["created_at"] != info["created_at"] {
            assert_eq!(list[0]["id"], second["id"]);
        }
        let listed = list.iter().find(|k| k["id"] == id.as_str()).unwrap();
        assert!(listed["last_used_at"].is_i64());
        let text = serde_json::to_string(&list).unwrap();
        assert!(!text.contains(&key[16..]), "the secret is never listed");
        assert!(!text.contains("secret_sha256"));
        assert!(!text.contains("\"secret\""));

        // revoking works at once
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/api_keys/{id}"),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = core.call(Method::GET, "/user/info", &key, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // and the session is still logged in
        let (status, _) = core
            .call(Method::GET, "/user/info", &core.owner_jwt, None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/api_keys/{id}"),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(core._dir.path().join("api_keys.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[tokio::test]
    async fn creation_is_validated() {
        let core = core().await;
        let post =
            |body: Value| core.call(Method::POST, "/user/api_keys", &core.owner_jwt, Some(body));
        for (body, expected) in [
            (
                json!({ "name": " ", "expires_at": null, "scope": { "type": "full" } }),
                "name",
            ),
            (
                json!({ "name": "a".repeat(65), "expires_at": null, "scope": { "type": "full" } }),
                "name",
            ),
            (
                json!({ "name": "x", "expires_at": api_key::now() - 10, "scope": { "type": "full" } }),
                "future",
            ),
            (
                json!({ "name": "x", "expires_at": null, "scope": custom(UserPermission {
                    can_view_instance: [InstanceUuid::from("nope".to_string())].into(),
                    ..Default::default()
                }) }),
                "does not exist",
            ),
        ] {
            let (status, body) = post(body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert!(message(&body).contains(expected), "{body}");
        }
        let (status, body) = post(json!({
            "name": "x",
            "expires_at": api_key::now() + 3600,
            "scope": custom(view_only()),
        }))
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    #[tokio::test]
    async fn session_only_routes_refuse_every_key() {
        let core = core().await;
        let (full, info) = core
            .create_key(&core.owner_jwt, "full", json!({ "type": "full" }))
            .await;
        let id = info["id"].as_str().unwrap();
        let uid = core.owner.uid.to_string();
        let routes = [
            (Method::GET, "/user/api_keys".to_string(), None),
            (
                Method::POST,
                "/user/api_keys".to_string(),
                Some(json!({ "name": "x", "expires_at": null, "scope": { "type": "full" } })),
            ),
            (Method::DELETE, format!("/user/api_keys/{id}"), None),
            (Method::GET, format!("/user/{uid}/api_keys"), None),
            (Method::DELETE, format!("/user/{uid}/api_keys/{id}"), None),
            (
                Method::PUT,
                format!("/user/{uid}/password"),
                Some(json!({ "uid": uid, "old_password": "pw", "new_password": "new" })),
            ),
            (Method::POST, format!("/user/logout/{uid}"), None),
        ];
        for (method, path, body) in routes {
            let (status, reply) = core.call(method.clone(), &path, &full, body).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {reply}");
            assert_eq!(message(&reply), api_key::SESSION_REQUIRED_MESSAGE);
        }
        // nothing happened: the key and the session both still work
        assert_eq!(
            core.call(Method::GET, "/user/info", &full, None).await.0,
            StatusCode::OK
        );
        assert_eq!(
            core.call(Method::GET, "/user/info", &core.owner_jwt, None)
                .await
                .0,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn only_sessions_and_full_keys_rename_their_user() {
        let core = core().await;
        let (empty, _) = core
            .create_key(&core.owner_jwt, "empty", custom(UserPermission::default()))
            .await;
        let (launcher, _) = core
            .create_key(&core.owner_jwt, "launcher", json!({ "type": "launcher" }))
            .await;
        let (full, _) = core
            .create_key(&core.owner_jwt, "full", json!({ "type": "full" }))
            .await;
        let path = format!("/user/{}/rename", core.owner.uid);
        let username = || async {
            let (_, me) = core
                .call(Method::GET, "/user/info", &core.owner_jwt, None)
                .await;
            me["username"].as_str().unwrap().to_string()
        };

        // scoped keys can't rename their own user, the owner here
        for key in [&empty, &launcher] {
            for name in ["", "x"] {
                let (status, reply) = core.call(Method::PUT, &path, key, Some(json!(name))).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{reply}");
                assert_eq!(message(&reply), api_key::FULL_KEY_REQUIRED_MESSAGE);
            }
        }
        assert_eq!(username().await, "owner");

        // nor can anyone give a blank name or one with control characters
        for token in [&core.owner_jwt, &full] {
            for name in ["", "   ", "a\nb", &"x".repeat(65)] {
                let (status, reply) = core
                    .call(Method::PUT, &path, token, Some(json!(name)))
                    .await;
                assert_eq!(status, StatusCode::BAD_REQUEST, "{name:?}: {reply}");
            }
        }
        assert_eq!(username().await, "owner");

        // a full key and a session can, and the name is trimmed
        let (status, _) = core
            .call(Method::PUT, &path, &full, Some(json!(" boss ")))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(username().await, "boss");
        let (status, _) = core
            .call(Method::PUT, &path, &core.owner_jwt, Some(json!("owner")))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(username().await, "owner");
    }

    #[tokio::test]
    async fn custom_keys_never_get_owner_powers() {
        let core = core().await;
        let (narrow, _) = core
            .create_key(&core.owner_jwt, "narrow", custom(view_only()))
            .await;
        let (full, _) = core
            .create_key(&core.owner_jwt, "full", json!({ "type": "full" }))
            .await;

        // /user/info shows what the key can do
        let (_, me) = core.call(Method::GET, "/user/info", &narrow, None).await;
        assert_eq!(me["is_owner"], false);
        assert_eq!(me["is_admin"], false);
        assert_eq!(me["permissions"]["can_view_instance"], json!([INSTANCE]));
        assert_eq!(me["permissions"]["can_create_instance"], false);

        // managing users
        let (status, _) = core.call(Method::GET, "/user/list", &narrow, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = core.call(Method::GET, "/user/list", &full, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn handlers_that_re_read_the_user_keep_the_key_scope() {
        let core = core().await;
        let member = add_user(&core.state, "member", false, false, Default::default()).await;
        // the owner may manage permissions; a custom key that lists the permission may
        // still not, because it is not the owner
        let (key, _) = core
            .create_key(
                &core.owner_jwt,
                "perms",
                custom(UserPermission {
                    can_manage_permission: true,
                    ..Default::default()
                }),
            )
            .await;
        let path = format!("/user/{}/update_perm", member.uid);
        let new = json!(UserPermission {
            can_create_instance: true,
            ..Default::default()
        });
        let (status, body) = core.call(Method::PUT, &path, &key, Some(new.clone())).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let stored = core
            .state
            .users_manager
            .read()
            .await
            .get_user(&member.uid)
            .unwrap();
        assert!(!stored.permissions.can_create_instance);
        // the owner's session can
        let (status, _) = core
            .call(Method::PUT, &path, &core.owner_jwt, Some(new))
            .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn global_fs_and_instance_routes_check_the_key() {
        let core = core().await;
        let (narrow, _) = core
            .create_key(&core.owner_jwt, "narrow", custom(view_only()))
            .await;
        let path: String = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            core._dir.path().to_string_lossy().as_bytes(),
        );
        // a handler that calls try_auth itself: the key authenticates, the scope refuses
        let ls = format!("/fs/{path}/ls");
        let (status, body) = core.call(Method::GET, &ls, &narrow, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        let (status, _) = core.call(Method::GET, &ls, "lsk_garbage", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // try_auth_or_err: settings need AccessSetting, which the key does not list
        let settings = format!("/instance/{INSTANCE}/settings");
        let (status, _) = core.call(Method::GET, &settings, &narrow, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn websockets_authenticate_keys_and_respect_the_scope() {
        let core = core().await;
        let (narrow, _) = core
            .create_key(&core.owner_jwt, "narrow", custom(view_only()))
            .await;
        let (nothing, _) = core
            .create_key(
                &core.owner_jwt,
                "nothing",
                custom(UserPermission::default()),
            )
            .await;

        // monitor: a query token, checked against ViewInstance before the instance is
        // looked up (it is a watched container with no monitor, hence 404 once allowed)
        let monitor =
            |token: &str| core.ws_url(&format!("/monitor/{INSTANCE}?token={}", query_token(token)));
        assert_eq!(
            ws_status(&monitor(&nothing)).await,
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            ws_status(&monitor(&narrow)).await,
            Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            ws_status(&monitor("lsk_nope")).await,
            Err(StatusCode::UNAUTHORIZED)
        );

        // console: a query token. Reading it needs the console permission; a key that can
        // only view the instance gets no console output.
        let (with_console, _) = core
            .create_key(
                &core.owner_jwt,
                "console",
                custom(UserPermission {
                    can_view_instance: [instance()].into(),
                    can_access_instance_console: [instance()].into(),
                    ..Default::default()
                }),
            )
            .await;
        let console_url = |token: &str| {
            core.ws_url(&format!(
                "/instance/{INSTANCE}/console/stream?token={}",
                query_token(token)
            ))
        };
        let (mut console, _) = tokio_tungstenite::connect_async(&console_url(&with_console))
            .await
            .unwrap();
        let (mut view_only_console, _) = tokio_tungstenite::connect_async(&console_url(&narrow))
            .await
            .unwrap();
        core.state
            .event_broadcaster
            .send(Event::new_instance_output(
                instance(),
                "test".to_string(),
                "hello from the console".to_string(),
            ));
        let frame = next_frame(&mut console).await.unwrap().unwrap();
        assert!(frame.to_text().unwrap().contains("hello from the console"));
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(500),
                view_only_console.next()
            )
            .await
            .is_err(),
            "a view-only key must not receive console output"
        );

        // events: the token is in the filter
        let events = |token: &str| {
            let filter = json!({ "bearer_token": token }).to_string();
            let filter: String = url::form_urlencoded::byte_serialize(filter.as_bytes()).collect();
            core.ws_url(&format!("/events/all/stream?filter={filter}"))
        };
        assert_eq!(
            ws_status(&events("lsk_nope")).await,
            Err(StatusCode::UNAUTHORIZED)
        );
        let (mut narrow_stream, _) = tokio_tungstenite::connect_async(&events(&narrow))
            .await
            .unwrap();
        let (mut nothing_stream, _) = tokio_tungstenite::connect_async(&events(&nothing))
            .await
            .unwrap();
        let other = InstanceUuid::from("DOCKER-other".to_string());
        // not visible to either key, then visible to `narrow` only
        core.state
            .event_broadcaster
            .send(Event::new_instance_state_transition(
                other,
                "other".to_string(),
                crate::traits::t_server::State::Running,
            ));
        core.state
            .event_broadcaster
            .send(Event::new_instance_state_transition(
                instance(),
                "mine".to_string(),
                crate::traits::t_server::State::Running,
            ));
        let frame = next_frame(&mut narrow_stream).await.unwrap().unwrap();
        let frame: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
        assert_eq!(frame["event_inner"]["instance_uuid"], INSTANCE);

        // revoking `nothing` closes its stream before it ever saw an event
        let nothing_id = api_key::parse_key(&nothing).unwrap().id.to_string();
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/api_keys/{nothing_id}"),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        match next_frame(&mut nothing_stream).await {
            None | Some(Ok(tungstenite::Message::Close(_))) | Some(Err(_)) => {}
            Some(Ok(other)) => panic!("a revoked key's stream got {other:?}"),
        }
        // the revocation is a user event, which `narrow` may not see (it needs ManageUser);
        // its stream stays open and keeps getting what it may see, and console lines are
        // not sent on the event stream
        core.state
            .event_broadcaster
            .send(Event::new_instance_output(
                instance(),
                "mine".to_string(),
                "console lines are not on the event stream".to_string(),
            ));
        core.state
            .event_broadcaster
            .send(Event::new_instance_state_transition(
                instance(),
                "mine".to_string(),
                crate::traits::t_server::State::Stopped,
            ));
        let frame = next_frame(&mut narrow_stream).await.unwrap().unwrap();
        assert!(frame.to_text().unwrap().contains("Stopped"));
    }

    #[tokio::test]
    async fn download_links_follow_the_key_that_made_them() {
        let core = core().await;
        let file = core._dir.path().join("report.txt");
        std::fs::write(&file, "contents").unwrap();
        let (key, info) = core
            .create_key(
                &core.owner_jwt,
                "reader",
                custom(UserPermission {
                    can_read_global_file: true,
                    ..Default::default()
                }),
            )
            .await;
        let user = core
            .state
            .users_manager
            .read()
            .await
            .try_auth(&key)
            .unwrap();
        let link = super::super::global_fs::register_download(
            &core.state,
            file.clone(),
            None,
            user,
            super::super::global_fs::DownloadScope::GlobalFile,
        )
        .await;
        let get = || async {
            core.client
                .get(format!("{}/file/{link}", core.base))
                .send()
                .await
                .unwrap()
                .status()
        };
        assert_eq!(get().await, StatusCode::OK);
        // safe mode applies to the key (it is not the owner), so the link stops working
        core.state
            .safe_mode
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(get().await, StatusCode::FORBIDDEN);
        core.state
            .safe_mode
            .store(false, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(get().await, StatusCode::OK);
        // and so does revoking the key
        let id = info["id"].as_str().unwrap();
        core.call(
            Method::DELETE,
            &format!("/user/api_keys/{id}"),
            &core.owner_jwt,
            None,
        )
        .await;
        assert_eq!(get().await, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn actions_name_the_key() {
        let core = core().await;
        let mut events = core.state.event_broadcaster.subscribe();
        let (key, _) = core
            .create_key(&core.owner_jwt, "deploy", json!({ "type": "full" }))
            .await;
        let path = format!("/user/{}/rename", core.owner.uid);
        let (status, _) = core
            .call(Method::PUT, &path, &key, Some(json!("boss")))
            .await;
        assert_eq!(status, StatusCode::OK);
        let mut created_by = None;
        let mut renamed_by = None;
        while let Ok(event) = events.try_recv() {
            if let EventInner::UserEvent(user_event) = &event.event_inner {
                match &user_event.user_event_inner {
                    UserEventInner::ApiKeyCreated { name, .. } => {
                        assert_eq!(name, "deploy");
                        assert_eq!(event.details, "API key `deploy` created for owner");
                        created_by = Some(event.caused_by.clone());
                    }
                    UserEventInner::UsernameChanged { .. } => {
                        renamed_by = Some(event.caused_by.clone())
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            created_by,
            Some(CausedBy::User {
                user_id: core.owner.uid.clone(),
                user_name: "owner".to_string()
            })
        );
        assert_eq!(
            renamed_by,
            Some(CausedBy::User {
                user_id: core.owner.uid.clone(),
                user_name: "owner (API key: deploy)".to_string()
            })
        );
    }

    #[tokio::test]
    async fn owner_and_admins_manage_other_users_keys() {
        let core = core().await;
        let admin = add_user(&core.state, "admin", false, true, Default::default()).await;
        let admin_jwt = admin.create_jwt().unwrap().to_string();
        let other_admin = add_user(&core.state, "admin2", false, true, Default::default()).await;
        let member = add_user(&core.state, "member", false, false, Default::default()).await;
        let member_jwt = member.create_jwt().unwrap().to_string();
        let (member_key, info) = core
            .create_key(&member_jwt, "member key", json!({ "type": "full" }))
            .await;
        let id = info["id"].as_str().unwrap();
        let (owner_key, _) = core
            .create_key(&core.owner_jwt, "owner key", json!({ "type": "full" }))
            .await;
        let list = |uid: &UserId| format!("/user/{uid}/api_keys");

        // a member may not look at others' keys, not even through this route for itself
        let (status, _) = core
            .call(Method::GET, &list(&core.owner.uid), &member_jwt, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // an admin may for members, not for the owner or other admins
        let (status, body) = core
            .call(Method::GET, &list(&member.uid), &admin_jwt, None)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body[0]["name"], "member key");
        for uid in [&core.owner.uid, &other_admin.uid] {
            let (status, _) = core.call(Method::GET, &list(uid), &admin_jwt, None).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
        }
        let (status, _) = core
            .call(
                Method::GET,
                &list(&UserId::default()),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // revoking: someone else's key id under this user is not found
        let owner_id = api_key::parse_key(&owner_key).unwrap().id.to_string();
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/{}/api_keys/{owner_id}", member.uid),
                &admin_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            core.call(Method::GET, "/user/info", &owner_key, None)
                .await
                .0,
            StatusCode::OK
        );
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/{}/api_keys/{id}", member.uid),
                &admin_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            core.call(Method::GET, "/user/info", &member_key, None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn deleting_a_user_deletes_their_keys() {
        let core = core().await;
        let member = add_user(&core.state, "member", false, false, Default::default()).await;
        let member_jwt = member.create_jwt().unwrap().to_string();
        let (key, _) = core
            .create_key(&member_jwt, "mine", json!({ "type": "full" }))
            .await;
        assert_eq!(
            core.call(Method::GET, "/user/info", &key, None).await.0,
            StatusCode::OK
        );
        let (status, _) = core
            .call(
                Method::DELETE,
                &format!("/user/{}", member.uid),
                &core.owner_jwt,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            core.call(Method::GET, "/user/info", &key, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let file = std::fs::read_to_string(core._dir.path().join("api_keys.json")).unwrap();
        assert_eq!(file, "[]");
    }

    #[tokio::test]
    async fn safe_mode_limits_custom_keys_over_http() {
        let core = core_with(settings(true)).await;
        let (key, _) = core
            .create_key(
                &core.owner_jwt,
                "files",
                custom(UserPermission {
                    can_read_global_file: true,
                    ..Default::default()
                }),
            )
            .await;
        let path: String = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            core._dir.path().to_string_lossy().as_bytes(),
        );
        let (status, body) = core
            .call(Method::GET, &format!("/fs/{path}/ls"), &key, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(message(&body).contains("safe mode"), "{body}");
    }
}
