pub mod read;
pub mod types;
pub mod write;

use std::{future::Future, path::Path, time::Duration};

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqliteLockingMode, SqlitePoolOptions,
    SqliteSynchronous,
};
use sqlx::SqlitePool;
use tracing::warn;

/// How long a write or a search waits for the connection, which the other may hold, before
/// it fails. They hold it briefly: writes are batches of events, and old events are
/// deleted a batch at a time (see `write::apply_retention`). The writer retries; a search
/// fails with an error rather than hang.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(15);

/// Opens the event database at `path`, creating it if it is missing.
///
/// One connection is enough: the event writer and the event search are the only users,
/// and SQLite writes one transaction at a time anyway. Nothing takes a second connection
/// while it holds one, which would wait forever with this pool.
///
/// Write-ahead logging with `synchronous = NORMAL` syncs to disk at checkpoints rather
/// than at every commit; a power cut can lose the last events, never corrupt the
/// database. The connection locks the database for itself (`locking_mode = EXCLUSIVE`,
/// set before WAL is turned on), so SQLite keeps the WAL index in memory and needs no
/// shared-memory `-shm` file, which network drives and some Docker bind mounts don't
/// support. Should WAL still fail there, the database uses the rollback journal it
/// always used before.
pub async fn open_event_database(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    open_with_fallback(|journal_mode| open(path, journal_mode)).await
}

/// Opens with WAL, or with the rollback journal (`journal_mode = DELETE`) if that fails.
async fn open_with_fallback<F, Fut>(open: F) -> Result<SqlitePool, sqlx::Error>
where
    F: Fn(SqliteJournalMode) -> Fut,
    Fut: Future<Output = Result<SqlitePool, sqlx::Error>>,
{
    match open(SqliteJournalMode::Wal).await {
        Ok(pool) => Ok(pool),
        Err(e) => {
            warn!("Couldn't open the event database with write-ahead logging, using the rollback journal: {e}");
            open(SqliteJournalMode::Delete).await
        }
    }
}

async fn open(path: &Path, journal_mode: SqliteJournalMode) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(1)
        // No idle timeout or lifetime: with either, sqlx wakes a maintenance task that
        // often for as long as the pool lives, connection or not. The event writer closes
        // the connection once it's unused instead (`write::IDLE_CONNECTION_TIMEOUT`).
        .idle_timeout(None)
        .max_lifetime(None)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                // applied before journal_mode (sqlx keeps SQLite's pragmas in order)
                .locking_mode(SqliteLockingMode::Exclusive)
                .journal_mode(journal_mode)
                .synchronous(SqliteSynchronous::Normal),
        )
        .await
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::*;
    use crate::db::{
        read::{search_events, tests::query},
        write::{init_client_events_table, tests::fs_event, write_client_events},
    };

    async fn journal_mode(pool: &SqlitePool) -> String {
        sqlx::query("PRAGMA journal_mode")
            .fetch_one(pool)
            .await
            .unwrap()
            .get(0)
    }

    /// Writes and searches at the same time, on the one connection.
    async fn write_and_search(pool: &SqlitePool) {
        init_client_events_table(pool).await.unwrap();
        let events = vec![fs_event("a"), fs_event("b")];
        let (written, found) = tokio::time::timeout(Duration::from_secs(30), async {
            tokio::join!(
                write_client_events(pool, &events),
                search_events(pool, query()),
            )
        })
        .await
        .expect("the writer and a search deadlocked");
        written.unwrap();
        found.unwrap();
        assert_eq!(search_events(pool, query()).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn the_event_database_uses_one_connection_and_wal_without_shared_memory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.db");
        let pool = open_event_database(&path).await.unwrap();
        assert_eq!(pool.options().get_max_connections(), 1);
        assert_eq!(journal_mode(&pool).await, "wal");
        let locking_mode: String = sqlx::query("PRAGMA locking_mode")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(locking_mode, "exclusive");

        // the writer and searches share the one connection without waiting on each other
        write_and_search(&pool).await;
        assert!(dir.path().join("data.db-wal").exists(), "WAL is in use");
        assert!(
            !dir.path().join("data.db-shm").exists(),
            "the WAL index is kept in memory"
        );
    }

    #[tokio::test]
    async fn the_event_database_uses_the_rollback_journal_where_wal_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.db");
        let tried = std::sync::Mutex::new(Vec::new());
        let pool = open_with_fallback(|journal_mode| {
            tried.lock().unwrap().push(format!("{journal_mode:?}"));
            let path = path.clone();
            async move {
                match journal_mode {
                    // as on a drive where WAL can't work
                    SqliteJournalMode::Wal => Err(sqlx::Error::Protocol("no WAL here".into())),
                    _ => open(&path, journal_mode).await,
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(*tried.lock().unwrap(), ["Wal", "Delete"]);
        assert_eq!(journal_mode(&pool).await, "delete");
        write_and_search(&pool).await;
    }
}
