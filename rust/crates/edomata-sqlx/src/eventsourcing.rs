//! Event-sourcing driver: journal, outbox, commands and snapshots.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use edomata_backend::eventsourcing::{
    CommandState, JournalReader, JournalRepositoryReader, Notifications, NotificationsPublisher,
    Repository, RepositoryReader, SnapshotItem, SnapshotPersistence, SnapshotStore, Storage,
    StorageDriver, ValidState, as_reader, dedup,
};
use edomata_backend::{
    BackendError, BoxStream, CommandRef, EventMessage, EventMetadata, EventVersion, OutboxItem,
    OutboxReader, Payload, SeqNr, SharedModel,
};
use edomata_core::{MessageMetadata, NonEmpty};
use edomata_postgres::{PGNamespace, PGNamespaceError, PGNaming};
use futures::{StreamExt, TryStreamExt};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::Query;
use sqlx::{PgPool, Postgres, Row};
use uuid::Uuid;

use crate::codec::SqlxCodec;
use crate::error::{assert_inserted, map_sqlx, map_write};
use crate::queries::{
    CommandQueries, JournalQueries, OutboxQueries, SnapshotQueries, setup_schema,
};

/// PostgreSQL storage driver for event-sourced aggregates.
///
/// Constructing the driver creates the schema in `Schema` naming mode unless
/// `skip_setup` is set; building a storage creates the tables unless
/// `skip_setup` is set (`CREATE ... IF NOT EXISTS`, so both are idempotent).
#[derive(Clone, Debug)]
pub struct SqlxDriver {
    naming: PGNaming,
    pool: PgPool,
    auto_setup: bool,
}

impl SqlxDriver {
    /// A driver that sets the schema and tables up automatically. Mirrors
    /// Scala's `SkunkDriver.from(naming, pool)` / `DoobieDriver.from`.
    pub async fn new(naming: PGNaming, pool: PgPool) -> Result<Self, BackendError> {
        Self::new_with(naming, pool, false).await
    }

    /// A driver for a schema-mode namespace given as a string. Mirrors
    /// Scala's `SkunkDriver("namespace", pool)`.
    pub async fn for_namespace(namespace: &str, pool: PgPool) -> Result<Self, BackendError> {
        let ns = PGNamespace::from_string(namespace).map_err(invalid_namespace)?;
        Self::new(PGNaming::schema(ns), pool).await
    }

    /// A driver with `skip_setup` as in Scala: when `true`, no `CREATE
    /// SCHEMA` and no `CREATE TABLE` / `CREATE INDEX` is ever executed and
    /// the tables are assumed to exist (created by Flyway or manually).
    pub async fn new_with(
        naming: PGNaming,
        pool: PgPool,
        skip_setup: bool,
    ) -> Result<Self, BackendError> {
        if !skip_setup {
            execute_all(&pool, &setup_schema(&naming)).await?;
        }
        Ok(Self {
            naming,
            pool,
            auto_setup: !skip_setup,
        })
    }

    /// The naming strategy.
    pub fn naming(&self) -> &PGNaming {
        &self.naming
    }

    /// The connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Whether tables are created automatically.
    pub fn auto_setup(&self) -> bool {
        self.auto_setup
    }
}

/// Maps a namespace validation error to a backend error.
pub fn invalid_namespace(e: PGNamespaceError) -> BackendError {
    BackendError::persistence(e.to_string())
}

/// Advisory lock key serialising Edomata DDL. Concurrent
/// `CREATE TABLE IF NOT EXISTS` for the same table can still fail in
/// PostgreSQL (`23505` on `pg_type` or `42P07`), so several replicas
/// starting at once must not set the same tables up concurrently.
const DDL_LOCK_KEY: &str = "edomata-ddl";

/// Runs DDL statements in one transaction, under a transaction-scoped
/// advisory lock shared by every Edomata driver.
pub async fn execute_all(pool: &PgPool, statements: &[String]) -> Result<(), BackendError> {
    if statements.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await.map_err(map_sqlx)?;
    sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
        .bind(DDL_LOCK_KEY)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    for statement in statements {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
    }
    tx.commit().await.map_err(map_sqlx)
}

/// The current time, as written in `time` / `created` columns.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

impl StorageDriver for SqlxDriver {
    type Codec<T: 'static> = SqlxCodec<T>;

    async fn build<S, E, R, N>(
        &self,
        model: SharedModel<S, E, R>,
        snapshot: Arc<dyn SnapshotStore<S>>,
        event_codec: SqlxCodec<E>,
        notification_codec: SqlxCodec<N>,
    ) -> Result<Storage<S, E, R, N>, BackendError>
    where
        S: Payload,
        E: Payload,
        R: Payload,
        N: Payload,
    {
        let journal_q = Arc::new(JournalQueries::new(&self.naming, event_codec.sql_type()));
        let outbox_q = Arc::new(OutboxQueries::new(
            &self.naming,
            notification_codec.sql_type(),
        ));
        let commands_q = Arc::new(CommandQueries::new(&self.naming));
        if self.auto_setup {
            execute_all(&self.pool, &journal_q.setup).await?;
            execute_all(&self.pool, &outbox_q.setup).await?;
            execute_all(&self.pool, &commands_q.setup).await?;
        }
        let updates = Notifications::new();
        let journal: Arc<dyn JournalReader<E>> = Arc::new(SqlxJournalReader {
            pool: self.pool.clone(),
            q: Arc::clone(&journal_q),
            codec: event_codec.clone(),
        });
        let outbox: Arc<dyn OutboxReader<N>> = Arc::new(SqlxOutboxReader {
            pool: self.pool.clone(),
            q: Arc::clone(&outbox_q),
            codec: notification_codec.clone(),
        });
        let reader: Arc<dyn RepositoryReader<S, E, R>> = Arc::new(JournalRepositoryReader::new(
            Arc::clone(&journal),
            as_reader(snapshot),
            model,
        ));
        let repository: Arc<dyn Repository<S, E, R, N>> = Arc::new(SqlxRepository {
            pool: self.pool.clone(),
            journal: journal_q,
            outbox: outbox_q,
            commands: commands_q,
            event_codec,
            notification_codec,
            reader: Arc::clone(&reader),
            updates: updates.clone(),
        });
        Ok(Storage {
            repository,
            reader,
            journal,
            outbox,
            updates: Arc::new(updates),
        })
    }

    async fn snapshot<S>(
        &self,
        state_codec: SqlxCodec<S>,
    ) -> Result<Arc<dyn SnapshotPersistence<S>>, BackendError>
    where
        S: Payload,
    {
        let q = SnapshotQueries::new(&self.naming, state_codec.sql_type());
        if self.auto_setup {
            execute_all(&self.pool, &q.setup).await?;
        }
        Ok(Arc::new(SqlxSnapshotPersistence {
            pool: self.pool.clone(),
            q,
            codec: state_codec,
        }))
    }
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

struct SqlxRepository<S, E, R, N> {
    pool: PgPool,
    journal: Arc<JournalQueries>,
    outbox: Arc<OutboxQueries>,
    commands: Arc<CommandQueries>,
    event_codec: SqlxCodec<E>,
    notification_codec: SqlxCodec<N>,
    reader: Arc<dyn RepositoryReader<S, E, R>>,
    updates: Notifications,
}

/// Inserts outbox rows inside `tx`.
pub async fn insert_outbox<N>(
    tx: &mut sqlx::PgConnection,
    q: &OutboxQueries,
    codec: &SqlxCodec<N>,
    stream: &str,
    time: DateTime<Utc>,
    metadata: &MessageMetadata,
    notifications: &[N],
) -> Result<(), BackendError> {
    for n in notifications {
        let affected = sqlx::query(&q.insert)
            .bind(codec.encode(n)?)
            .bind(stream)
            .bind(time)
            .bind(metadata.correlation.as_deref())
            .bind(metadata.causation.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(map_write)?
            .rows_affected();
        assert_inserted(affected, 1)?;
    }
    Ok(())
}

/// Records a handled command inside `tx`.
pub async fn insert_command(
    tx: &mut sqlx::PgConnection,
    q: &CommandQueries,
    cmd: CommandRef<'_>,
) -> Result<(), BackendError> {
    let affected = sqlx::query(&q.insert)
        .bind(cmd.id)
        .bind(cmd.address)
        .bind(cmd.time)
        .execute(&mut *tx)
        .await
        .map_err(map_write)?
        .rows_affected();
    assert_inserted(affected, 1)
}

/// Whether a command id was already recorded.
pub async fn command_exists(
    executor: impl sqlx::PgExecutor<'_>,
    q: &CommandQueries,
    id: &str,
) -> Result<bool, BackendError> {
    let count: i64 = sqlx::query_scalar(&q.count)
        .bind(id)
        .fetch_one(executor)
        .await
        .map_err(map_sqlx)?;
    Ok(count != 0)
}

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N>
    for SqlxRepository<S, E, R, N>
{
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        if command_exists(&self.pool, &self.commands, cmd.id).await? {
            return Ok(CommandState::Redundant);
        }
        Ok(CommandState::Aggregate(self.reader.get(cmd.address).await?))
    }

    async fn append(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        _new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        let now = now();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        for (i, event) in events.iter().enumerate() {
            let affected = sqlx::query(&self.journal.insert)
                .bind(Uuid::new_v4())
                .bind(cmd.address)
                .bind(now)
                .bind(version + i as SeqNr)
                .bind(self.event_codec.encode(event)?)
                .execute(&mut *tx)
                .await
                .map_err(map_write)?
                .rows_affected();
            assert_inserted(affected, 1)?;
        }
        insert_outbox(
            &mut tx,
            &self.outbox,
            &self.notification_codec,
            cmd.address,
            now,
            cmd.metadata,
            &notifications,
        )
        .await?;
        insert_command(&mut tx, &self.commands, cmd).await?;
        tx.commit().await.map_err(map_write)?;
        self.updates.notify_journal();
        self.updates.notify_outbox();
        Ok(())
    }

    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError> {
        let now = now();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        insert_outbox(
            &mut tx,
            &self.outbox,
            &self.notification_codec,
            cmd.address,
            now,
            cmd.metadata,
            &notifications,
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        self.updates.notify_outbox();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Journal reader
// ---------------------------------------------------------------------------

struct SqlxJournalReader<E> {
    pool: PgPool,
    q: Arc<JournalQueries>,
    codec: SqlxCodec<E>,
}

impl<E: Payload> SqlxJournalReader<E> {
    fn row_to_event(&self, row: &PgRow) -> Result<EventMessage<E>, BackendError> {
        let metadata = EventMetadata {
            id: row.try_get("id").map_err(map_sqlx)?,
            time: row.try_get("time").map_err(map_sqlx)?,
            seq_nr: row.try_get("seqnr").map_err(map_sqlx)?,
            version: row.try_get("version").map_err(map_sqlx)?,
            stream: row.try_get("stream").map_err(map_sqlx)?,
        };
        let payload = self.codec.decode_row(row, "payload")?;
        Ok(EventMessage { metadata, payload })
    }

    fn stream<'a>(
        &'a self,
        query: Query<'a, Postgres, PgArguments>,
    ) -> BoxStream<'a, Result<EventMessage<E>, BackendError>> {
        query
            .fetch(&self.pool)
            .map_err(map_sqlx)
            .and_then(move |row| std::future::ready(self.row_to_event(&row)))
            .boxed()
    }
}

impl<E: Payload> JournalReader<E> for SqlxJournalReader<E> {
    fn read_stream(&self, stream_id: &str) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(sqlx::query(&self.q.read_stream).bind(stream_id.to_owned()))
    }

    fn read_stream_after(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(
            sqlx::query(&self.q.read_stream_after)
                .bind(stream_id.to_owned())
                .bind(version),
        )
    }

    fn read_stream_before(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(
            sqlx::query(&self.q.read_stream_before)
                .bind(stream_id.to_owned())
                .bind(version),
        )
    }

    fn read_all(&self) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(sqlx::query(&self.q.read_all))
    }

    fn read_all_after(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(sqlx::query(&self.q.read_all_after).bind(seq_nr))
    }

    fn read_all_before(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.stream(sqlx::query(&self.q.read_all_before).bind(seq_nr))
    }
}

// ---------------------------------------------------------------------------
// Outbox reader
// ---------------------------------------------------------------------------

/// Outbox reader over a pool and an outbox query catalogue; shared with
/// derived drivers whose outbox table has extra columns.
pub struct SqlxOutboxReader<N> {
    /// Connection pool.
    pub pool: PgPool,
    /// Outbox statements (only `read` and `mark_published` are used).
    pub q: Arc<OutboxQueries>,
    /// Notification codec.
    pub codec: SqlxCodec<N>,
}

impl<N: Payload> SqlxOutboxReader<N> {
    fn row_to_item(&self, row: &PgRow) -> Result<OutboxItem<N>, BackendError> {
        Ok(OutboxItem {
            seq_nr: row.try_get("seqnr").map_err(map_sqlx)?,
            stream_id: row.try_get("stream").map_err(map_sqlx)?,
            time: row.try_get("created").map_err(map_sqlx)?,
            data: self.codec.decode_row(row, "payload")?,
            metadata: MessageMetadata {
                correlation: row.try_get("correlation").map_err(map_sqlx)?,
                causation: row.try_get("causation").map_err(map_sqlx)?,
            },
        })
    }
}

#[async_trait]
impl<N: Payload> OutboxReader<N> for SqlxOutboxReader<N> {
    fn read(&self) -> BoxStream<'_, Result<OutboxItem<N>, BackendError>> {
        sqlx::query(&self.q.read)
            .fetch(&self.pool)
            .map_err(map_sqlx)
            .and_then(move |row| std::future::ready(self.row_to_item(&row)))
            .boxed()
    }

    async fn mark_all_as_sent(&self, items: &NonEmpty<OutboxItem<N>>) -> Result<(), BackendError> {
        let ids: Vec<SeqNr> = items.iter().map(|i| i.seq_nr).collect();
        sqlx::query(&self.q.mark_published)
            .bind(now())
            .bind(&ids)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Snapshot persistence
// ---------------------------------------------------------------------------

struct SqlxSnapshotPersistence<S> {
    pool: PgPool,
    q: SnapshotQueries,
    codec: SqlxCodec<S>,
}

#[async_trait]
impl<S: Payload> SnapshotPersistence<S> for SqlxSnapshotPersistence<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        let row = sqlx::query(&self.q.get)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?;
        match row {
            None => Ok(None),
            Some(row) => Ok(Some(ValidState {
                state: self.codec.decode_row(&row, "state")?,
                version: row.try_get("version").map_err(map_sqlx)?,
            })),
        }
    }

    async fn put(&self, items: Vec<SnapshotItem<S>>) -> Result<(), BackendError> {
        let items = dedup(items);
        if items.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        for (id, state) in &items {
            sqlx::query(&self.q.put)
                .bind(id)
                .bind(self.codec.encode(&state.state)?)
                .bind(state.version)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        }
        tx.commit().await.map_err(map_sqlx)
    }
}
