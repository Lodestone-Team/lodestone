use std::time::Duration;

use crate::{
    error::Error,
    events::{Event, EventInner, ProgressionEventInner},
    output_types::ClientEvent,
    prelude::LODESTONE_EPOCH_MIL,
};

use color_eyre::eyre::Context;
use sqlx::sqlite::SqlitePool;
use tokio::{
    sync::broadcast::{
        error::{RecvError, TryRecvError},
        Receiver,
    },
    time::Instant,
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
/// Most events one statement of [`apply_retention`] deletes. The first pass after an
/// upgrade can delete hundreds of thousands; in batches, writes and searches get the
/// database's one connection between them instead of waiting for all of it.
const RETENTION_BATCH_ROWS: i64 = 10_000;
/// How often, at most, the retention limits are applied.
const RETENTION_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// How long after the core starts the retention limits can first be applied. Short, so
/// that they are still applied on computers where the core never runs for long.
const FIRST_RETENTION_DELAY: Duration = Duration::from_secs(60);
/// How long the event database's connection stays open unused. Each open SQLite
/// connection has a thread of its own, and an idle core rarely writes, so the writer
/// closes the connection soon and the pool opens it again for the next write or search.
const IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

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
/// transaction per batch, and old events are pruned after a write at most once per
/// [`RETENTION_INTERVAL`] (see [`RETENTION_MAX_AGE`] and [`RETENTION_MAX_ROWS`]).
///
/// An idle core has no timers here: pruning waits for a write, which is the only thing
/// that adds events, and the only timer, which closes the connection
/// [`IDLE_CONNECTION_TIMEOUT`] after it was last used, is disarmed once it has. A search
/// can open the connection again; it then stays open until the writer next closes it.
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

    // Not at once: the core has better things to do while it starts.
    let mut next_retention = Instant::now() + FIRST_RETENTION_DELAY;
    // opening the database opened the connection
    let mut close_at = Some(Instant::now() + IDLE_CONNECTION_TIMEOUT);
    let mut batch: Vec<ClientEvent> = Vec::with_capacity(MAX_BATCH_SIZE);

    loop {
        let first = tokio::select! {
            result = event_receiver.recv() => result,
            _ = async {
                match close_at {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            } => {
                close_idle_connection(&sqlite_pool).await;
                close_at = None;
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
            if Instant::now() >= next_retention {
                if let Err(e) =
                    apply_retention(&sqlite_pool, chrono::Utc::now().timestamp_millis()).await
                {
                    error!("Failed to prune old events: {}", e);
                }
                next_retention = Instant::now() + RETENTION_INTERVAL;
            }
            close_at = Some(Instant::now() + IDLE_CONNECTION_TIMEOUT);
        }
        batch.clear();
    }
}

/// Closes the pool's connection if nothing is using it; the pool opens a new one when
/// it's next needed. A search holding it keeps it open.
async fn close_idle_connection(pool: &SqlitePool) {
    if let Some(connection) = pool.try_acquire() {
        if let Err(e) = connection.close().await {
            warn!("Couldn't close the event database's idle connection: {e}");
        }
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
/// [`RETENTION_MAX_ROWS`] events, [`RETENTION_BATCH_ROWS`] at a time. Returns the number
/// of deleted rows.
pub(crate) async fn apply_retention(pool: &SqlitePool, now_unix_millis: i64) -> Result<u64, Error> {
    apply_retention_with(
        pool,
        now_unix_millis,
        RETENTION_MAX_ROWS,
        RETENTION_BATCH_ROWS,
    )
    .await
}

async fn apply_retention_with(
    pool: &SqlitePool,
    now_unix_millis: i64,
    max_rows: i64,
    batch_rows: i64,
) -> Result<u64, Error> {
    let cutoff = snowflake_lower_bound(now_unix_millis - RETENTION_MAX_AGE.as_millis() as i64);
    let by_age = delete_in_batches(
        pool,
        "DELETE FROM ClientEvents WHERE id IN (SELECT id FROM ClientEvents WHERE snowflake < ?1 LIMIT ?2)",
        cutoff,
        batch_rows,
    )
    .await
    .context("Failed to delete old events")?;
    // ids only grow, so the newest rows have the largest ids: everything up to the
    // newest row past the limit goes
    let last_excess: Option<i64> =
        sqlx::query_scalar("SELECT id FROM ClientEvents ORDER BY id DESC LIMIT 1 OFFSET ?1")
            .bind(max_rows)
            .fetch_optional(pool)
            .await
            .context("Failed to count events")?;
    let by_count = match last_excess {
        Some(last_excess) => delete_in_batches(
            pool,
            "DELETE FROM ClientEvents WHERE id IN (SELECT id FROM ClientEvents WHERE id <= ?1 ORDER BY id LIMIT ?2)",
            last_excess,
            batch_rows,
        )
        .await
        .context("Failed to delete excess events")?,
        None => 0,
    };
    let deleted = by_age + by_count;
    if deleted > 0 {
        info!("Pruned {deleted} old events from the event database");
    }
    Ok(deleted)
}

/// Runs `delete`, which deletes at most `?2` rows given `?1`, until it deletes no more,
/// each statement on its own so that the connection is free between them. Returns the
/// number of deleted rows.
async fn delete_in_batches(
    pool: &SqlitePool,
    delete: &'static str,
    bound: i64,
    batch_rows: i64,
) -> Result<u64, sqlx::Error> {
    let mut deleted = 0;
    loop {
        let rows = sqlx::query(delete)
            .bind(bound)
            .bind(batch_rows)
            .execute(pool)
            .await?
            .rows_affected();
        deleted += rows;
        if rows < batch_rows as u64 {
            return Ok(deleted);
        }
        // let a waiting write or search have the connection
        tokio::task::yield_now().await;
    }
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
    async fn the_idle_connection_is_closed_and_reopened_when_needed() {
        // the core's pool, with its one connection
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::open_event_database(&dir.path().join("events.db"))
            .await
            .unwrap();
        init_client_events_table(&pool).await.unwrap();
        write_client_events(&pool, &[fs_event("before")])
            .await
            .unwrap();
        assert_eq!(pool.size(), 1);
        // a connection goes back to the pool in a task of its own
        tokio::time::timeout(Duration::from_secs(5), async {
            while pool.num_idle() == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        close_idle_connection(&pool).await;
        assert_eq!(pool.size(), 0);
        // a connection in use isn't closed
        let held = pool.acquire().await.unwrap();
        close_idle_connection(&pool).await;
        assert_eq!(pool.size(), 1);
        drop(held);
        write_client_events(&pool, &[fs_event("after")])
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ClientEvents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
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

    #[tokio::test]
    async fn retention_deletes_in_batches() {
        let (_dir, pool) = temp_pool().await;
        let now = chrono::Utc::now().timestamp_millis();
        let old = snowflake_lower_bound(now - RETENTION_MAX_AGE.as_millis() as i64 - 1000);
        let recent = snowflake_lower_bound(now);
        // 7 too old, then 12 recent ones, of which the newest 5 are kept
        let snowflakes: Vec<i64> = (0..7)
            .map(|n| old + n)
            .chain((0..12).map(|n| recent + n))
            .collect();
        for snowflake in &snowflakes {
            sqlx::query(
                "INSERT INTO ClientEvents (event_value, details, snowflake, level) VALUES ('{}', '', ?1, 'Info')",
            )
            .bind(snowflake)
            .execute(&pool)
            .await
            .unwrap();
        }
        // batches of 3: several per limit, the last one short
        assert_eq!(apply_retention_with(&pool, now, 5, 3).await.unwrap(), 14);
        let remaining: Vec<i64> =
            sqlx::query_scalar("SELECT snowflake FROM ClientEvents ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, snowflakes[14..]);
        // nothing more to delete
        assert_eq!(apply_retention_with(&pool, now, 5, 3).await.unwrap(), 0);
        // a batch exactly as large as what is left
        assert_eq!(apply_retention_with(&pool, now, 2, 3).await.unwrap(), 3);
    }
}
