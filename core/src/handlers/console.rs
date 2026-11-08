use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    db::read::{console_history, ConsoleHistoryPage},
    error::Error,
    types::{InstanceUuid, Snowflake},
    AppState,
};

use super::util::AuthedUser;

/// Lines in a page when the request does not say.
pub const DEFAULT_PAGE_SIZE: u32 = 200;
/// Most lines in a page, whatever the request asks for.
pub const MAX_PAGE_SIZE: u32 = 500;

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct ConsoleHistoryQuery {
    /// Only lines older than this one (its snowflake). Without it, the newest lines.
    before: Option<Snowflake>,
    /// At most this many lines, and never more than [`MAX_PAGE_SIZE`].
    limit: Option<u32>,
}

fn page_size(requested: Option<u32>) -> u32 {
    requested
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE)
}

/// Console lines of an instance that are older than the ones in its in-memory console
/// buffer. Seen by the same users as the live console stream and the buffer.
pub async fn get_console_history(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
    Query(query): Query<ConsoleHistoryQuery>,
) -> Result<Json<ConsoleHistoryPage>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    let mut page = console_history(
        &state.sqlite_pool,
        &uuid,
        query.before,
        page_size(query.limit),
    )
    .await?;
    page.events
        .retain(|event| requester.can_view_event_parts(&event.event_inner, &event.caused_by));
    Ok(Json(page))
}

pub fn get_console_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/{uuid}/console/history", get(get_console_history))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::Value;

    use super::*;
    use crate::{
        auth::{
            permission::UserPermission,
            user::{User, UsersManager},
        },
        db::write::{tests::temp_pool, write_client_events},
        events::{CausedBy, EventInner, EventLevel, InstanceEvent, InstanceEventInner},
        global_settings::GlobalSettingsData,
        output_types::ClientEvent,
        test_util::{serve_api, test_app_state},
    };

    struct Core {
        _dir: tempfile::TempDir,
        _db_dir: tempfile::TempDir,
        state: AppState,
        base: String,
        client: reqwest::Client,
    }

    async fn core() -> Core {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(dir.path(), GlobalSettingsData::default()).await;
        let (db_dir, pool) = temp_pool().await;
        state.sqlite_pool = pool;
        let base = serve_api(state.clone()).await;
        Core {
            _dir: dir,
            _db_dir: db_dir,
            state,
            base,
            client: reqwest::Client::new(),
        }
    }

    async fn add_user(
        users: &tokio::sync::RwLock<UsersManager>,
        name: &str,
        is_owner: bool,
        permissions: UserPermission,
    ) -> String {
        let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
        users
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user.create_jwt().unwrap().to_string()
    }

    fn can_view(instances: &[&InstanceUuid]) -> UserPermission {
        UserPermission {
            can_view_instance: instances.iter().map(|uuid| (*uuid).clone()).collect(),
            ..Default::default()
        }
    }

    fn line(uuid: &InstanceUuid, inner: InstanceEventInner) -> ClientEvent {
        ClientEvent {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: uuid.clone(),
                instance_name: "test".to_string(),
                instance_event_inner: inner,
            }),
            details: String::new(),
            snowflake: Snowflake::new(),
            level: EventLevel::Info,
            caused_by: CausedBy::System,
        }
    }

    fn output(uuid: &InstanceUuid, message: &str) -> ClientEvent {
        line(
            uuid,
            InstanceEventInner::InstanceOutput {
                message: message.to_string(),
            },
        )
    }

    impl Core {
        async fn history(
            &self,
            uuid: &InstanceUuid,
            query: &str,
            token: Option<&str>,
        ) -> (StatusCode, Value) {
            let mut request = self.client.get(format!(
                "{}/instance/{uuid}/console/history?{query}",
                self.base
            ));
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let response = request.send().await.unwrap();
            let status = response.status();
            let text = response.text().await.unwrap();
            (
                status,
                serde_json::from_str(&text).unwrap_or(Value::String(text)),
            )
        }
    }

    fn messages(page: &Value) -> Vec<String> {
        page["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|event| {
                event["event_inner"]["instance_event_inner"]["message"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn page_size_is_capped_and_never_empty() {
        assert_eq!(page_size(None), DEFAULT_PAGE_SIZE);
        assert_eq!(page_size(Some(10)), 10);
        assert_eq!(page_size(Some(0)), 1);
        assert_eq!(page_size(Some(MAX_PAGE_SIZE)), MAX_PAGE_SIZE);
        assert_eq!(page_size(Some(MAX_PAGE_SIZE + 1)), MAX_PAGE_SIZE);
        assert_eq!(page_size(Some(u32::MAX)), MAX_PAGE_SIZE);
    }

    #[tokio::test]
    async fn history_requires_authentication() {
        let core = core().await;
        let a = InstanceUuid::default();
        write_client_events(&core.state.sqlite_pool, &[output(&a, "secret")])
            .await
            .unwrap();

        let (status, body) = core.history(&a, "", None).await;
        assert!(status.is_client_error(), "{status}: {body}");
        assert!(!body.to_string().contains("secret"));
        let (status, body) = core.history(&a, "", Some("not-a-token")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert!(!body.to_string().contains("secret"));
    }

    #[tokio::test]
    async fn history_is_limited_to_instances_the_user_can_view() {
        let core = core().await;
        let a = InstanceUuid::default();
        let b = InstanceUuid::default();
        write_client_events(
            &core.state.sqlite_pool,
            &[output(&a, "from a"), output(&b, "from b")],
        )
        .await
        .unwrap();
        let users = &core.state.users_manager;
        let only_a = add_user(users, "only-a", false, can_view(&[&a])).await;
        let nothing = add_user(users, "nothing", false, Default::default()).await;
        let owner = add_user(users, "owner", true, Default::default()).await;

        let (status, page) = core.history(&a, "", Some(&only_a)).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(messages(&page), ["from a"]);

        // another instance's lines are refused, not just filtered out
        let (status, body) = core.history(&b, "", Some(&only_a)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert!(!body.to_string().contains("from b"));
        let (status, _) = core.history(&a, "", Some(&nothing)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, page) = core.history(&b, "", Some(&owner)).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(messages(&page), ["from b"]);
    }

    #[tokio::test]
    async fn pages_are_complete_when_instances_interleave() {
        let core = core().await;
        let a = InstanceUuid::default();
        let b = InstanceUuid::default();
        // three of b's lines for every one of a's, so a limit applied before the instance
        // filter would return short pages
        let mut events = Vec::new();
        for i in 0..40 {
            events.push(output(&a, &format!("a{i}")));
            for j in 0..3 {
                events.push(output(&b, &format!("b{i}-{j}")));
            }
        }
        // events that are not console lines are mixed in as well
        events.insert(
            10,
            line(
                &a,
                InstanceEventInner::InstanceInput {
                    message: "not a console line".to_string(),
                },
            ),
        );
        write_client_events(&core.state.sqlite_pool, &events)
            .await
            .unwrap();
        let owner = add_user(&core.state.users_manager, "owner", true, Default::default()).await;

        let mut collected: Vec<String> = Vec::new();
        let mut query = "limit=15".to_string();
        let mut pages = 0;
        loop {
            let (status, page) = core.history(&a, &query, Some(&owner)).await;
            assert_eq!(status, StatusCode::OK, "{page}");
            let mut lines = messages(&page);
            let has_more = page["has_more"].as_bool().unwrap();
            if has_more {
                assert_eq!(lines.len(), 15, "a page before the last one is full");
            }
            let oldest = page["events"][0]["snowflake"].as_str().map(str::to_string);
            lines.extend(collected);
            collected = lines;
            pages += 1;
            match (has_more, oldest) {
                (true, Some(oldest)) => query = format!("limit=15&before={oldest}"),
                _ => break,
            }
        }
        assert_eq!(pages, 3);
        let expected: Vec<String> = (0..40).map(|i| format!("a{i}")).collect();
        assert_eq!(collected, expected);
    }

    #[tokio::test]
    async fn page_size_cap_is_enforced() {
        let core = core().await;
        let a = InstanceUuid::default();
        let total = MAX_PAGE_SIZE as usize + 25;
        let events: Vec<ClientEvent> = (0..total)
            .map(|i| output(&a, &format!("line {i}")))
            .collect();
        write_client_events(&core.state.sqlite_pool, &events)
            .await
            .unwrap();
        let owner = add_user(&core.state.users_manager, "owner", true, Default::default()).await;

        let (_, page) = core.history(&a, "limit=1000000", Some(&owner)).await;
        assert_eq!(messages(&page).len(), MAX_PAGE_SIZE as usize);
        assert_eq!(page["has_more"], true);
        // the newest lines come first
        assert_eq!(
            messages(&page).last().unwrap(),
            &format!("line {}", total - 1)
        );
        let (_, page) = core.history(&a, "", Some(&owner)).await;
        assert_eq!(messages(&page).len(), DEFAULT_PAGE_SIZE as usize);
        let (_, page) = core.history(&a, "limit=0", Some(&owner)).await;
        assert_eq!(messages(&page).len(), 1);
    }

    #[tokio::test]
    async fn bad_cursor_is_rejected() {
        let core = core().await;
        let a = InstanceUuid::default();
        let owner = add_user(&core.state.users_manager, "owner", true, Default::default()).await;
        let (status, _) = core.history(&a, "before=abc", Some(&owner)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
