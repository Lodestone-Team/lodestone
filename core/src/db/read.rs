use crate::{
    error::Error, events::EventQuery, output_types::ClientEvent, prelude::LODESTONE_EPOCH_MIL,
};

use color_eyre::eyre::Context;
use sqlx::{sqlite::SqlitePool, QueryBuilder, Row, Sqlite};
use tracing::error;

/// Most rows a search reads from the database. The newest matching rows are kept.
pub const MAX_SEARCH_ROWS: i64 = 10_000;

/// Builds the SQL for `event_query`. Time range, instance ids and levels are filtered in
/// SQL (using the snowflake and instance indexes); the remaining filters need the decoded
/// event and are applied by [`search_events`]. Returns `None` when the query cannot match
/// anything (an empty id or level list).
fn build_search_query(event_query: &EventQuery) -> Option<QueryBuilder<Sqlite>> {
    let mut builder: QueryBuilder<Sqlite> =
        QueryBuilder::new("SELECT event_value FROM ClientEvents WHERE 1 = 1");
    if let Some(time_range) = &event_query.time_range {
        let start = (time_range.start - LODESTONE_EPOCH_MIL) << 22;
        let end = (time_range.end + 1 - LODESTONE_EPOCH_MIL) << 22;
        builder.push(" AND snowflake >= ");
        builder.push_bind(start);
        builder.push(" AND snowflake <= ");
        builder.push_bind(end);
    }
    if let Some(instance_ids) = &event_query.event_instance_ids {
        if instance_ids.is_empty() {
            return None;
        }
        builder.push(" AND instance_id IN (");
        let mut separated = builder.separated(", ");
        for id in instance_ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");
    }
    if let Some(levels) = &event_query.event_levels {
        if levels.is_empty() {
            return None;
        }
        builder.push(" AND level IN (");
        let mut separated = builder.separated(", ");
        for level in levels {
            separated.push_bind(level.clone());
        }
        separated.push_unseparated(")");
    }
    builder.push(" ORDER BY id DESC LIMIT ");
    builder.push_bind(MAX_SEARCH_ROWS);
    Some(builder)
}

/// Returns the stored events that match `event_query`, oldest first. At most
/// [`MAX_SEARCH_ROWS`] rows are read (the newest ones).
pub async fn search_events(
    pool: &SqlitePool,
    event_query: EventQuery,
) -> Result<Vec<ClientEvent>, Error> {
    let Some(mut query) = build_search_query(&event_query) else {
        return Ok(Vec::new());
    };
    let rows = query
        .build()
        .fetch_all(pool)
        .await
        .context("Failed to fetch events")?;
    let mut parsed_client_events: Vec<ClientEvent> = Vec::with_capacity(rows.len());
    // rows are newest first; return them oldest first, as before
    for row in rows.iter().rev() {
        let event_value: String = row.try_get("event_value").context("Bad event row")?;
        match serde_json::from_str(&event_value) {
            Ok(client_event) => parsed_client_events.push(client_event),
            Err(_) => error!("Failed to parse client event: {}", event_value),
        }
    }
    Ok(parsed_client_events
        .into_iter()
        .filter(|client_event| event_query.filter(client_event))
        .collect())
}

#[cfg(test)]
mod tests {
    use crate::{
        db::write::{tests::temp_pool, write_client_events},
        events::{CausedBy, EventInner, EventLevel, InstanceEvent, InstanceEventInner},
        types::{InstanceUuid, Snowflake, TimeRange},
    };

    use super::*;

    fn query() -> EventQuery {
        EventQuery {
            event_levels: None,
            event_types: None,
            instance_event_types: None,
            user_event_types: None,
            event_user_ids: None,
            event_instance_ids: None,
            bearer_token: None,
            time_range: None,
        }
    }

    fn instance_event(uuid: &InstanceUuid, level: EventLevel) -> ClientEvent {
        ClientEvent {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: uuid.clone(),
                instance_name: "test".to_string(),
                instance_event_inner: InstanceEventInner::InstanceOutput {
                    message: "hello".to_string(),
                },
            }),
            details: "".to_string(),
            snowflake: Snowflake::new(),
            level,
            caused_by: CausedBy::System,
        }
    }

    #[tokio::test]
    async fn test_search() {
        let (_dir, pool) = temp_pool().await;
        let a = InstanceUuid::default();
        let b = InstanceUuid::default();
        let events = vec![
            instance_event(&a, EventLevel::Info),
            instance_event(&b, EventLevel::Error),
            instance_event(&a, EventLevel::Error),
        ];
        write_client_events(&pool, &events).await.unwrap();

        let all = search_events(&pool, query()).await.unwrap();
        assert_eq!(all.len(), 3);
        // oldest first
        assert_eq!(all[0].snowflake, events[0].snowflake);

        let only_a = search_events(
            &pool,
            EventQuery {
                event_instance_ids: Some(vec![a.clone()]),
                ..query()
            },
        )
        .await
        .unwrap();
        assert_eq!(only_a.len(), 2);

        let a_errors = search_events(
            &pool,
            EventQuery {
                event_instance_ids: Some(vec![a.clone()]),
                event_levels: Some(vec![EventLevel::Error]),
                ..query()
            },
        )
        .await
        .unwrap();
        assert_eq!(a_errors.len(), 1);
        assert_eq!(a_errors[0].snowflake, events[2].snowflake);

        let none = search_events(
            &pool,
            EventQuery {
                event_instance_ids: Some(vec![]),
                ..query()
            },
        )
        .await
        .unwrap();
        assert!(none.is_empty());

        let now = chrono::Utc::now().timestamp_millis();
        let in_range = search_events(
            &pool,
            EventQuery {
                time_range: Some(TimeRange {
                    start: now - 60_000,
                    end: now + 60_000,
                }),
                ..query()
            },
        )
        .await
        .unwrap();
        assert_eq!(in_range.len(), 3);
        let out_of_range = search_events(
            &pool,
            EventQuery {
                time_range: Some(TimeRange {
                    start: now - 120_000,
                    end: now - 60_000,
                }),
                ..query()
            },
        )
        .await
        .unwrap();
        assert!(out_of_range.is_empty());
    }
}
