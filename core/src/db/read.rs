use crate::{
    error::Error, output_types::ClientEvent,
    prelude::LODESTONE_EPOCH_MIL, events::EventQuery,
};

use color_eyre::eyre::Context;
use sqlx::sqlite::SqlitePool;
use tracing::error;

// TODO clean up all unwraps

pub async fn search_events(
    pool: &SqlitePool,
    event_query: EventQuery,
) -> Result<Vec<ClientEvent>, Error> {
    // TODO do not return sqlx::Error
    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire connection to db")?;
    let parsed_client_events = if let Some(time_range) = &event_query.time_range {
        let start = (time_range.start - LODESTONE_EPOCH_MIL.with(|p| *p)) << 22;
        let end = (time_range.end + 1 - LODESTONE_EPOCH_MIL.with(|p| *p)) << 22;
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
        .await
        .context("Failed to fetch events")?;
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
        .await
        .context("Failed to fetch events")?;
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
        .filter(|client_event| event_query.filter(client_event))
        .collect();
    Ok(filtered)
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use sqlx::{sqlite::SqliteConnectOptions, Pool, Sqlite};

    use crate::{
        db::write::init_client_events_table,
        events::{CausedBy, EventInner, EventLevel, FSEvent, FSOperation, FSTarget},
        types::Snowflake,
    };

    use super::*;

    #[tokio::test]
    async fn test_search() {
        let pool: Pool<Sqlite> = Pool::connect_with(
            SqliteConnectOptions::from_str("sqlite://test.db")
                .unwrap()
                .create_if_missing(true),
        )
        .await
        .unwrap();
        let drop_result = sqlx::query!(r#"DROP TABLE IF EXISTS ClientEvents"#)
            .execute(&pool)
            .await;
        assert!(drop_result.is_ok());
        let init_result = init_client_events_table(&pool).await;
        assert!(init_result.is_ok());

        let snowflake = Snowflake::new();
        let _dummy_event_1 = ClientEvent {
            event_inner: EventInner::FSEvent(FSEvent {
                operation: FSOperation::Read,
                target: FSTarget::File(PathBuf::from("/test")),
            }),
            details: "Dummy detail 1".to_string(),
            snowflake,
            level: EventLevel::Info,
            caused_by: CausedBy::System,
        };

        // let row_1_result = sqlx::query!(
        //     r#"
        //     INSERT INTO ClientEvents (event_value, details, snowflake, level)
        //     VALUES ($1, $2, $3, $4);
        //     "#,
        //     serde_json::to_string(&dummy_event_1).unwrap(),
        //     "Dummy detail 1".to_string(),
        //     snowflake.clone().to_string(),
        //     "Info"
        // )
        // .execute(&pool)
        // .await;

        // let row_1 = row_1_result.unwrap();
    }

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
