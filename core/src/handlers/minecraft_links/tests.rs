//! Linking Minecraft accounts, against a core on a loopback port whose Minecraft
//! services are a mock on another loopback port. Nothing here reaches the real service.

use std::sync::{Arc, Mutex};

use axum::{http::HeaderMap, response::IntoResponse, routing::get};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::GlobalSettingsData,
    minecraft_links::MinecraftLinks,
    test_util::{serve_api, test_app_state},
};

const STEVE_UUID: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
const ALEX_UUID: &str = "853c80ef-3c37-49fd-aa49-938b674adae6";
// made up; long enough to be found in a file if it were ever written
const STEVE_TOKEN: &str = "eyJmock.steve-minecraft-access-token.do-not-store-1f9e";
const ALEX_TOKEN: &str = "eyJmock.alex-minecraft-access-token.do-not-store-77c2";
const NO_GAME_TOKEN: &str = "eyJmock.account-without-minecraft";

/// The mock of `GET /minecraft/profile`. Records the `Authorization` headers it gets.
async fn mock_minecraft_services(seen: Arc<Mutex<Vec<String>>>) -> String {
    let app = axum::Router::new().route(
        "/minecraft/profile",
        get(move |headers: HeaderMap| {
            let seen = seen.clone();
            async move {
                let auth = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                seen.lock().unwrap().push(auth.clone());
                let profile = |id: &str, name: &str| {
                    axum::Json(json!({ "id": id, "name": name, "skins": [], "capes": [] }))
                        .into_response()
                };
                match auth.strip_prefix("Bearer ") {
                    Some(STEVE_TOKEN) => profile("069a79f444e94726a5befca90e38aaf5", "Steve"),
                    Some(ALEX_TOKEN) => profile("853c80ef3c3749fdaa49938b674adae6", "Alex"),
                    Some(NO_GAME_TOKEN) => (
                        StatusCode::NOT_FOUND,
                        axum::Json(json!({ "error": "NOT_FOUND" })),
                    )
                        .into_response(),
                    _ => StatusCode::UNAUTHORIZED.into_response(),
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    format!("http://{addr}/minecraft/profile")
}

struct Core {
    dir: tempfile::TempDir,
    state: AppState,
    base: String,
    client: reqwest::Client,
    owner_jwt: String,
    seen: Arc<Mutex<Vec<String>>>,
}

async fn core() -> Core {
    crate::test_util::install_crypto_provider();
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_app_state(dir.path(), GlobalSettingsData::default()).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let profile_url = mock_minecraft_services(seen.clone()).await;
    state.minecraft_links =
        Arc::new(MinecraftLinks::load(dir.path().join("minecraft_links.json"), profile_url).await);
    let owner = add_user(&state, "owner", true).await;
    let owner_jwt = owner.create_jwt().unwrap().to_string();
    let base = serve_api(state.clone()).await;
    Core {
        dir,
        state,
        base,
        client: reqwest::Client::new(),
        owner_jwt,
        seen,
    }
}

async fn add_user(state: &AppState, name: &str, owner: bool) -> User {
    let user = User::new(
        name.to_string(),
        "pw",
        owner,
        false,
        UserPermission::default(),
    );
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    user
}

fn link_body(uuid: &str, name: &str, token: &str) -> Value {
    json!({ "minecraft_uuid": uuid, "minecraft_name": name, "access_token": token })
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

    async fn launcher_key(&self, session: &str) -> String {
        let (status, body) = self
            .call(
                Method::POST,
                "/user/api_keys",
                session,
                Some(
                    json!({ "name": "Lodestone Launcher on Test Mac", "expires_at": null,
                             "scope": { "type": "launcher" } }),
                ),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["key"].as_str().unwrap().to_string()
    }

    async fn own_links(&self, token: &str) -> Value {
        let (status, body) = self
            .call(Method::GET, "/user/minecraft_links", token, None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// Every byte the core wrote to its folder.
    fn files_contain(&self, needle: &str) -> bool {
        walkdir::WalkDir::new(self.dir.path())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .any(|e| {
                std::fs::read(e.path())
                    .is_ok_and(|data| data.windows(needle.len()).any(|w| w == needle.as_bytes()))
            })
    }
}

fn message(body: &Value) -> String {
    body["causes"][0].as_str().unwrap_or_default().to_string()
}

#[tokio::test]
async fn a_launcher_key_links_a_verified_account_and_the_token_is_not_kept() {
    let core = core().await;
    let player = add_user(&core.state, "player", false).await;
    let session = player.create_jwt().unwrap().to_string();
    let key = core.launcher_key(&session).await;

    // the name sent is only checked for shape; the stored one is Minecraft's
    let (status, link) = core
        .call(
            Method::POST,
            "/user/minecraft_links",
            &key,
            Some(link_body(
                "069A79F444E94726A5BEFCA90E38AAF5",
                "steve_old",
                STEVE_TOKEN,
            )),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{link}");
    assert_eq!(link["minecraft_uuid"], STEVE_UUID);
    assert_eq!(link["minecraft_name"], "Steve");
    assert_eq!(link["verified"], true);
    assert_eq!(link["uid"], json!(player.uid));
    assert!(!link.to_string().contains(STEVE_TOKEN));

    // the token went to the profile service once, as the bearer token, and nowhere else
    // (compared without printing the token if it fails)
    assert!(*core.seen.lock().unwrap() == vec![format!("Bearer {STEVE_TOKEN}")]);
    // never persisted: not in the links file, nor in any other file of the core
    let file = std::fs::read_to_string(core.dir.path().join("minecraft_links.json")).unwrap();
    assert!(file.contains(STEVE_UUID));
    assert!(!core.files_contain(STEVE_TOKEN));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(core.dir.path().join("minecraft_links.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    // listed for the user, through the key and the session alike
    assert_eq!(core.own_links(&key).await, json!([link]));
    assert_eq!(core.own_links(&session).await, json!([link]));
    // and it survives a restart
    let reloaded = MinecraftLinks::load(
        core.dir.path().join("minecraft_links.json"),
        "http://127.0.0.1:9/unused",
    )
    .await;
    assert_eq!(reloaded.for_user(&player.uid).await.len(), 1);
}

#[tokio::test]
async fn links_need_a_token_of_the_selected_account() {
    let core = core().await;
    let link = |uuid: &str, token: &str| {
        core.call(
            Method::POST,
            "/user/minecraft_links",
            &core.owner_jwt,
            Some(link_body(uuid, "Steve", token)),
        )
    };
    // a token of another account than the one selected
    let (status, body) = link(STEVE_UUID, ALEX_TOKEN).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("different Minecraft account"));
    // a refused token is a bad request, never a 401 that a client would read as "signed out"
    let (status, body) = link(STEVE_UUID, "eyJmock.expired").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("refused"));
    let (status, body) = link(STEVE_UUID, NO_GAME_TOKEN).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(message(&body).contains("doesn't own Minecraft"));
    // malformed requests never reach the service
    let before = core.seen.lock().unwrap().len();
    for body in [
        link_body("not-a-uuid", "Steve", STEVE_TOKEN),
        link_body(STEVE_UUID, "has space", STEVE_TOKEN),
        link_body(STEVE_UUID, "Steve", ""),
        link_body(STEVE_UUID, "Steve", "two words"),
    ] {
        let (status, reply) = core
            .call(
                Method::POST,
                "/user/minecraft_links",
                &core.owner_jwt,
                Some(body),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{reply}");
        assert!(!reply.to_string().contains(STEVE_TOKEN));
    }
    assert_eq!(core.seen.lock().unwrap().len(), before);
    // no request signed in, no link and no call
    let (status, _) = core
        .call(
            Method::POST,
            "/user/minecraft_links",
            "lsk_nope",
            Some(link_body(STEVE_UUID, "Steve", STEVE_TOKEN)),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(core.seen.lock().unwrap().len(), before);
    assert_eq!(core.own_links(&core.owner_jwt).await, json!([]));
}

#[tokio::test]
async fn users_remove_their_own_links_and_the_owner_any() {
    let core = core().await;
    let alice = add_user(&core.state, "alice", false).await;
    let bob = add_user(&core.state, "bob", false).await;
    let alice_jwt = alice.create_jwt().unwrap().to_string();
    let bob_jwt = bob.create_jwt().unwrap().to_string();
    for (jwt, uuid, token) in [
        (&alice_jwt, STEVE_UUID, STEVE_TOKEN),
        (&bob_jwt, ALEX_UUID, ALEX_TOKEN),
    ] {
        let (status, body) = core
            .call(
                Method::POST,
                "/user/minecraft_links",
                jwt,
                Some(link_body(uuid, "x", token)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    // each sees only their own
    assert_eq!(
        core.own_links(&alice_jwt).await.as_array().unwrap().len(),
        1
    );
    assert_eq!(
        core.own_links(&bob_jwt).await[0]["minecraft_uuid"],
        ALEX_UUID
    );
    // only the owner sees and removes everyone's
    for (method, path) in [
        (Method::GET, "/minecraft_links".to_string()),
        (Method::DELETE, format!("/minecraft_links/{ALEX_UUID}")),
    ] {
        let (status, _) = core.call(method, &path, &alice_jwt, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    // a launcher key of the owner isn't the owner either
    let owner_key = core.launcher_key(&core.owner_jwt).await;
    let (status, _) = core
        .call(Method::GET, "/minecraft_links", &owner_key, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, all) = core
        .call(Method::GET, "/minecraft_links", &core.owner_jwt, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let mut names: Vec<(String, String)> = all
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["username"].as_str().unwrap().to_string(),
                l["minecraft_name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            ("alice".to_string(), "Steve".to_string()),
            ("bob".to_string(), "Alex".to_string())
        ]
    );

    // alice can't remove bob's link through her own route
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/user/minecraft_links/{ALEX_UUID}"),
            &alice_jwt,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(core.own_links(&bob_jwt).await.as_array().unwrap().len(), 1);
    // her own, without dashes too
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/user/minecraft_links/{}", STEVE_UUID.replace('-', "")),
            &alice_jwt,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.own_links(&alice_jwt).await, json!([]));
    // the owner removes bob's
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/minecraft_links/{ALEX_UUID}"),
            &core.owner_jwt,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.own_links(&bob_jwt).await, json!([]));
}

#[tokio::test]
async fn proving_an_account_again_moves_it_and_deleting_a_user_removes_theirs() {
    let core = core().await;
    let alice = add_user(&core.state, "alice", false).await;
    let bob = add_user(&core.state, "bob", false).await;
    let alice_jwt = alice.create_jwt().unwrap().to_string();
    let bob_jwt = bob.create_jwt().unwrap().to_string();
    for jwt in [&alice_jwt, &bob_jwt] {
        let (status, body) = core
            .call(
                Method::POST,
                "/user/minecraft_links",
                jwt,
                Some(link_body(STEVE_UUID, "Steve", STEVE_TOKEN)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    // an account belongs to one user: the one who proved it last
    assert_eq!(core.own_links(&alice_jwt).await, json!([]));
    assert_eq!(core.own_links(&bob_jwt).await.as_array().unwrap().len(), 1);

    let (status, body) = core
        .call(
            Method::DELETE,
            &format!("/user/{}", bob.uid),
            &core.owner_jwt,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(core.state.minecraft_links.all().await.is_empty());
}
