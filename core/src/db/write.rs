use std::time::Duration;

use crate::{
    error::Error,
    events::{Event, EventInner, ProgressionEventInner},
    output_types::ClientEvent,
    prelude::LODESTONE_EPOCH_MIL,
};

use color_eyre::eyre::Context;
use sqlx::sqlite::SqlitePool;
use tokio::sync::broadcast::{
    error::{RecvError, TryRecvError},
    Receiver,
};
use tracing::{error, info, warn};

use super::types::ClientEventRow;

/// Most events written in one transaction.
const MAX_BATCH_SIZE: usize = 512;
/// Attempts to write one batch before it is dropped.
const MAX_WRITE_ATTEMPTS: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Events older than this are deleted.
pub const RETENTION_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// At most this many events are kept; the oldest are deleted first.
pub const RETENTION_MAX_ROWS: i64 = 1_000_000;
/// How often the retention limits are applied.
const RETENTION_INTERVAL: Duration = Duration::from_secs(60 * 60);

fn should_store(event: &ClientEvent) -> bool {
    !matches!(
        &event.event_inner,
        EventInner::ProgressionEvent(pe)
            if matches!(pe.progression_event_inner(), ProgressionEventInner::ProgressionUpdate { .. })
    )
}

/// Stores events in the `ClientEvents` table until the event channel closes.
///
/// Database errors are logged and retried with backoff; they never end this task, so a
/// busy or full database cannot shut Lodestone down. Events are written in batches, one
/// transaction per batch, and old events are pruned periodically (see
/// [`RETENTION_MAX_AGE`] and [`RETENTION_MAX_ROWS`]).
pub async fn write_event_to_db_task(mut event_receiver: Receiver<Event>, sqlite_pool: SqlitePool) {
    let mut backoff = INITIAL_BACKOFF;
    while let Err(e) = init_client_events_table(&sqlite_pool).await {
        error!(
            "Failed to initialize client events table, retrying in {}s: {}",
            backoff.as_secs_f32(),
            e
        );
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }

    let mut retention_interval = tokio::time::interval(RETENTION_INTERVAL);
    retention_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut batch: Vec<ClientEvent> = Vec::with_capacity(MAX_BATCH_SIZE);

    loop {
        let first = tokio::select! {
            result = event_receiver.recv() => result,
            _ = retention_interval.tick() => {
                if let Err(e) = apply_retention(&sqlite_pool, chrono::Utc::now().timestamp_millis()).await {
                    error!("Failed to prune old events: {}", e);
                }
                continue;
            }
        };
        match first {
            Ok(event) => batch.push(event.into()),
            Err(RecvError::Lagged(n)) => {
                warn!("Event database writer lagged, {n} events were not stored");
                continue;
            }
            Err(RecvError::Closed) => {
                info!("Event channel closed, event database writer stopped");
                break;
            }
        }
        // take whatever else is already queued, up to the batch size
        while batch.len() < MAX_BATCH_SIZE {
            match event_receiver.try_recv() {
                Ok(event) => batch.push(event.into()),
                Err(TryRecvError::Lagged(n)) => {
                    warn!("Event database writer lagged, {n} events were not stored")
                }
                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
            }
        }
        batch.retain(should_store);
        if !batch.is_empty() {
            write_batch_with_retry(&sqlite_pool, &batch).await;
        }
        batch.clear();
    }
}

async fn write_batch_with_retry(pool: &SqlitePool, batch: &[ClientEvent]) {
    let mut backoff = INITIAL_BACKOFF;
    for attempt in 1..=MAX_WRITE_ATTEMPTS {
        match write_client_events(pool, batch).await {
            Ok(()) => return,
            Err(e) if attempt < MAX_WRITE_ATTEMPTS => {
                warn!(
                    "Failed to write {} events to the database (attempt {attempt}), retrying in {}s: {}",
                    batch.len(),
                    backoff.as_secs_f32(),
                    e
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
            Err(e) => error!(
                "Failed to write {} events to the database, dropping them: {}",
                batch.len(),
                e
            ),
        }
    }
}

/// Inserts all events in one transaction.
pub(crate) async fn write_client_events(
    pool: &SqlitePool,
    client_events: &[ClientEvent],
) -> Result<(), Error> {
    let mut transaction = pool.begin().await.context("Failed to begin transaction")?;
    for client_event in client_events {
        let row = ClientEventRow::from(client_event);
        sqlx::query(
            r#"
INSERT INTO ClientEvents
(event_value, details, snowflake, level, caused_by_user_id, instance_id)
VALUES
(?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        )
        .bind(row.event_value)
        .bind(row.details)
        .bind(row.snowflake)
        .bind(row.level)
        .bind(row.caused_by_user_id)
        .bind(row.instance_id)
        .execute(&mut *transaction)
        .await
        .context("Failed to write to DB")?;
    }
    transaction
        .commit()
        .await
        .context("Failed to commit events to DB")?;
    Ok(())
}

/// The smallest snowflake generated at or after `unix_millis`.
pub(crate) fn snowflake_lower_bound(unix_millis: i64) -> i64 {
    (unix_millis - LODESTONE_EPOCH_MIL).max(0) << 22
}

/// Deletes events older than [`RETENTION_MAX_AGE`] and all but the newest
/// [`RETENTION_MAX_ROWS`] events. Returns the number of deleted rows.
pub(crate) async fn apply_retention(pool: &SqlitePool, now_unix_millis: i64) -> Result<u64, Error> {
    let cutoff = snowflake_lower_bound(now_unix_millis - RETENTION_MAX_AGE.as_millis() as i64);
    let by_age = sqlx::query("DELETE FROM ClientEvents WHERE snowflake < ?1")
        .bind(cutoff)
        .execute(pool)
        .await
        .context("Failed to delete old events")?
        .rows_affected();
    // ids only grow, so the newest rows have the largest ids
    let by_count = sqlx::query(
        r#"
DELETE FROM ClientEvents WHERE id <= (
    SELECT id FROM ClientEvents ORDER BY id DESC LIMIT 1 OFFSET ?1
)"#,
    )
    .bind(RETENTION_MAX_ROWS)
    .execute(pool)
    .await
    .context("Failed to delete excess events")?
    .rows_affected();
    let deleted = by_age + by_count;
    if deleted > 0 {
        info!("Pruned {deleted} old events from the event database");
    }
    Ok(deleted)
}

pub async fn init_client_events_table(pool: &SqlitePool) -> Result<(), Error> {
    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire db connection")?;

    sqlx::query(
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
        "#,
    )
    .execute(&mut *connection)
    .await
    .context("Failed to create table")?;

    // Searches filter on time (snowflake) and on instance + time.
    sqlx::query("CREATE INDEX IF NOT EXISTS ClientEvents_snowflake ON ClientEvents (snowflake)")
        .execute(&mut *connection)
        .await
        .context("Failed to create snowflake index")?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS ClientEvents_instance_snowflake ON ClientEvents (instance_id, snowflake)",
    )
    .execute(&mut *connection)
    .await
    .context("Failed to create instance index")?;

    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use sqlx::{sqlite::SqliteConnectOptions, Pool, Row, Sqlite};

    use crate::{
        events::{CausedBy, EventLevel, FSEvent, FSOperation, FSTarget},
        types::Snowflake,
    };

    use super::*;

    /// A fresh database in a temporary folder (instead of the shared `test.db`).
    pub(crate) async fn temp_pool() -> (tempfile::TempDir, Pool<Sqlite>) {
        let dir = tempfile::tempdir().unwrap();
        let pool = Pool::connect_with(
            SqliteConnectOptions::new()
                .filename(dir.path().join("events.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
        init_client_events_table(&pool).await.unwrap();
        (dir, pool)
    }

    pub(crate) fn fs_event(details: &str) -> ClientEvent {
        ClientEvent {
            event_inner: EventInner::FSEvent(FSEvent {
                operation: FSOperation::Read,
                target: FSTarget::File(PathBuf::from("/test")),
            }),
            details: details.to_string(),
            snowflake: Snowflake::new(),
            level: EventLevel::Info,
            caused_by: CausedBy::System,
        }
    }

    #[tokio::test]
    async fn test_write() {
        let (_dir, pool) = temp_pool().await;
        let dummy_event = fs_event("Dummy value");
        write_client_events(&pool, std::slice::from_ref(&dummy_event))
            .await
            .unwrap();

        let row = sqlx::query("SELECT * FROM ClientEvents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            row.get::<String, _>("event_value"),
            serde_json::to_string(&dummy_event).unwrap()
        );
        assert_eq!(row.get::<String, _>("details"), dummy_event.details);
        assert_eq!(
            row.get::<i64, _>("snowflake").to_string(),
            dummy_event.snowflake.to_string()
        );
        assert_eq!(row.get::<String, _>("level"), "Info");
        assert_eq!(row.get::<Option<String>, _>("caused_by_user_id"), None);
        assert_eq!(row.get::<Option<String>, _>("instance_id"), None);
    }

    #[tokio::test]
    async fn test_batch_write() {
        let (_dir, pool) = temp_pool().await;
        let events: Vec<_> = (0..100).map(|i| fs_event(&i.to_string())).collect();
        write_client_events(&pool, &events).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ClientEvents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 100);
    }

    #[tokio::test]
    async fn write_error_does_not_end_the_task() {
        use crate::test_util::wait_until;
        let (_dir, pool) = temp_pool().await;
        // make inserts of one event fail, like a full disk would
        sqlx::query(
            "CREATE TRIGGER fail_insert BEFORE INSERT ON ClientEvents WHEN NEW.details = 'poison' \
             BEGIN SELECT RAISE(ABORT, 'disk full'); END",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (tx, rx) = tokio::sync::broadcast::channel(16);
        let task = tokio::spawn(write_event_to_db_task(rx, pool.clone()));
        let event = |details: &str| {
            let mut event = crate::events::new_fs_event(
                FSOperation::Read,
                FSTarget::File(PathBuf::from("/a")),
                CausedBy::System,
            );
            event.details = details.to_string();
            event
        };
        tx.send(event("poison")).unwrap();
        // once the task has taken the event, a later one goes into the next batch
        wait_until("the writer received the event", || {
            let queued = tx.len();
            async move { queued == 0 }
        })
        .await;
        tx.send(event("fine")).unwrap();

        // the failing batch is retried and dropped; the old code ended the task instead,
        // which shut down the whole core
        let stored = |details: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ClientEvents WHERE details = ?1")
                    .bind(details)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
            }
        };
        wait_until("the next event is stored", || async {
            stored("fine").await == 1
        })
        .await;
        assert_eq!(stored("poison").await, 0);
        assert!(!task.is_finished());
        task.abort();
    }

    #[tokio::test]
    async fn test_retention() {
        let (_dir, pool) = temp_pool().await;
        let now = chrono::Utc::now().timestamp_millis();
        let old = snowflake_lower_bound(now - RETENTION_MAX_AGE.as_millis() as i64 - 1000);
        for snowflake in [old, old + 1, snowflake_lower_bound(now)] {
            sqlx::query(
                "INSERT INTO ClientEvents (event_value, details, snowflake, level) VALUES ('{}', '', ?1, 'Info')",
            )
            .bind(snowflake)
            .execute(&pool)
            .await
            .unwrap();
        }
        assert_eq!(apply_retention(&pool, now).await.unwrap(), 2);
        let remaining: i64 = sqlx::query_scalar("SELECT snowflake FROM ClientEvents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, snowflake_lower_bound(now));
    }
}
