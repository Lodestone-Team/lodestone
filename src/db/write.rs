use std::str::FromStr;

use crate::{
    events::{Event, EventInner, ProgressionEventInner},
    output_types::ClientEvent,
    traits::ErrorInner,
    Error,
};

use futures::Future;
use log::{info, warn};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool},
    Pool,
};
use tokio::sync::broadcast::{error::RecvError, Receiver};

use super::types::ClientEventRow;

// TODO clean up all unwraps

pub fn write_event_to_db_task(
    mut event_receiver: Receiver<Event>,
    sqlite_pool: SqlitePool,
) -> impl Future<Output = ()> {
    async move {
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
                match &pe.progression_event_inner {
                    ProgressionEventInner::ProgressionUpdate {
                        progress_message, ..
                    } => {
                        info!("Update event: {}", progress_message);
                        info!("Skipped storage...");
                        continue;
                    }
                    _ => {}
                }
            }
            let insertion_result = write_client_event(&sqlite_pool, client_event).await;
            if insertion_result.is_err() {
                warn!(
                    "Error inserting into database: {}",
                    insertion_result.err().unwrap()
                );
                break;
            }
        }
    }
}

async fn write_client_event(
    pool: &SqlitePool,
    client_event: ClientEvent,
) -> Result<i64, Error> {
    let mut connection = pool.acquire().await.map_err(|err| Error {
        inner: ErrorInner::DBPoolError,
        detail: format!("Failed to acquire connection: {}", err),
    })?;

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
    .await.map_err(|err| Error {
        inner: ErrorInner::DBWriteError,
        detail: format!("Failed to write to DB: {}", err),
    })?
    .last_insert_rowid();
    Ok(id)
}

async fn init_client_events_table(pool: &SqlitePool) -> Result<(), Error> {
    let mut connection = pool.acquire().await.map_err(|err| Error {
        inner: ErrorInner::DBPoolError,
        detail: format!("Failed to acquire connection: {}", err),
    })?;

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
    .map_err(|err| Error {
        inner: ErrorInner::DBInitError,
        detail: format!("Failed to init table: {}", err),
    })?;
    Ok(())
}
