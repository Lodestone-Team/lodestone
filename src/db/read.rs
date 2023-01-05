use crate::{
    handlers::events::EventQuery,
    output_types::ClientEvent,
    Error, traits::ErrorInner,
};

use log::error;
use sqlx::{
    sqlite::{SqlitePool},
};

// TODO clean up all unwraps

pub async fn search_events(
    pool: &SqlitePool,
    event_query: EventQuery,
) -> Result<Vec<ClientEvent>, Error> {
    // TODO do not return sqlx::Error
    let mut connection = pool.acquire().await.map_err(|err| Error {
        inner: ErrorInner::DBPoolError,
        detail: format!("Failed to acquire connection: {}", err),
    })?;
    let parsed_client_events = if let Some(time_range) = &event_query.time_range {
        let start = (time_range.start) << 22;
        let end = (time_range.end + 1) << 22;
        let rows = sqlx::query!(
            r#"
SELECT
event_value, details, snowflake, level, caused_by_user_id, instance_id
FROM ClientEvents
WHERE snowflake >= ($1) AND snowflake <= ($2)"#,
            start,
            end
        ) // TODO bit shift
        .fetch_all(&mut connection)
        .await.map_err(|err| Error {
            inner: ErrorInner::DBFetchError,
            detail: format!("Failed to fetch events: {}", err),
        })?;
        let mut parsed_client_events: Vec<ClientEvent> = Vec::new();
        for row in rows {
            if let Ok(client_event) = serde_json::from_str(&row.event_value) {
                parsed_client_events.push(client_event);
            } else {
                error!("Failed to parse client event: {}", row.event_value);
            }
        }
        parsed_client_events
    } else {
        let rows = sqlx::query!(
            r#"
SELECT
*
FROM ClientEvents"#
        )
        .fetch_all(&mut connection)
        .await.map_err(|err| Error {
            inner: ErrorInner::DBFetchError,
            detail: format!("Failed to fetch events: {}", err),
        })?;
        let mut parsed_client_events: Vec<ClientEvent> = Vec::new();
        for row in rows {
            if let Ok(client_event) = serde_json::from_str(&row.event_value) {
                parsed_client_events.push(client_event);
            } else {
                error!("Failed to parse client event: {}", row.event_value);
            }
        }
        parsed_client_events
    };
    let filtered = parsed_client_events
        .into_iter()
        .filter(|client_event| event_query.filter(&client_event))
        .collect();
    return Ok(filtered);
}

#[cfg(test)]
mod tests {
    use crate::events::EventLevel;

    use super::*;

    // TODO should properly implement tests, with dummy values
    // #[tokio::test]
    // async fn test_read() {
    //     let pool = SqlitePool::connect("sqlite://dev.db")
    //         .await
    //         .unwrap();
    //     let results = search_events(
    //         &pool,
    //         EventQuery {
    //             event_levels: Some(vec![EventLevel::Error]),
    //             event_types: None,
    //             instance_event_types: None,
    //             user_event_types: None,
    //             event_user_ids: None,
    //             event_instance_ids: None,
    //             bearer_token: None,
    //             time_range: None,
    //         },
    //     )
    //     .await
    //     .unwrap();
    //     assert!(results.is_empty());
    //     for result in results {
    //         println!("{:?}", result);
    //     }
    // }
}
