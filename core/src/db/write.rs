use crate::{
    error::Error,
    events::{Event, EventInner, ProgressionEventInner},
    output_types::ClientEvent,
};

use color_eyre::eyre::Context;
use sqlx::sqlite::SqlitePool;
use tokio::sync::broadcast::{error::RecvError, Receiver};
use tracing::{error, warn};

use super::types::ClientEventRow;

// TODO clean up all unwraps

pub async fn write_event_to_db_task(mut event_receiver: Receiver<Event>, sqlite_pool: SqlitePool) {
    let init_result = init_client_events_table(&sqlite_pool).await;
    if let Err(error) = init_result.as_ref() {
        warn!("Failed to initialize client events table: {}", error);
        return;
    }

    loop {
        let result = event_receiver.recv().await;
        if let Err(error) = result.as_ref() {
            match error {
                RecvError::Lagged(_) => {
                    warn!("Event buffer lagged");
                    continue;
                }
                RecvError::Closed => {
                    warn!("Event buffer closed");
                    break;
                }
            }
        }

        let client_event: ClientEvent = result.unwrap().into();
        if let EventInner::ProgressionEvent(pe) = &client_event.event_inner {
            if let ProgressionEventInner::ProgressionUpdate { .. } = pe.progression_event_inner() {
                continue;
            }
        }
        let insertion_result = write_client_event(&sqlite_pool, client_event).await;
        if let Err(e) = insertion_result.as_ref() {
            error!("Error inserting into database: {}", e);
            break;
        }
    }
}

async fn write_client_event(pool: &SqlitePool, client_event: ClientEvent) -> Result<i64, Error> {
    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire db connection")?;

    let row = ClientEventRow::from(&client_event);
    let id = sqlx::query!(
        r#"
INSERT INTO ClientEvents
(event_value, details, snowflake, level, caused_by_user_id, instance_id)
VALUES
(?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        row.event_value,
        row.details,
        row.snowflake,
        row.level,
        row.caused_by_user_id,
        row.instance_id,
    )
    .execute(&mut connection)
    .await
    .context("Failed to write to DB")?
    .last_insert_rowid();
    Ok(id)
}

pub async fn init_client_events_table(pool: &SqlitePool) -> Result<(), Error> {
    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire db connection")?;

    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS ClientEvents (
            id                  INTEGER     PRIMARY KEY     AUTOINCREMENT,
            event_value         TEXT        NOT NULL,
            details             TEXT        NOT NULL,
            snowflake           BIGINT      NOT NULL,
            level               VARCHAR(20) NOT NULL,
            caused_by_user_id   TEXT,
            instance_id         TEXT
        );
        "#
    )
    .execute(&mut connection)
    .await
    .context("Failed to create table")?;

    Ok(())
}
