//! CQRS driver: states, outbox and commands.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_backend::cqrs::{
    AggregateState, CommandState, Notifications, NotificationsPublisher, Repository,
    RepositoryReader, SharedStateModel, Storage, StorageDriver,
};
use edomata_backend::{BackendError, CommandRef, OutboxReader, Payload, SeqNr};
use edomata_core::BoxFuture;
use edomata_core::NonEmpty;
use edomata_postgres::{PGNamespace, PGNaming};
use sqlx::{PgConnection, PgPool, Row};

use crate::codec::SqlxCodec;
use crate::error::{map_sqlx, map_write};
use crate::eventsourcing::{
    SqlxOutboxReader, command_exists, execute_all, insert_command, insert_outbox,
    invalid_namespace, now,
};
use crate::queries::{CommandQueries, OutboxQueries, StateQueries, setup_schema};

/// A hook run inside the `save` transaction for every non-empty batch of
/// notifications, with the transaction's connection. Use it to maintain
/// projections atomically with the state. Mirrors Scala's `SkunkHandler` /
/// `DoobieHandler`.
///
/// ```
/// use edomata_sqlx::SqlxHandler;
///
/// let handler: SqlxHandler<String> = SqlxHandler::new(|notifications, conn| {
///     Box::pin(async move {
///         for n in notifications.iter() {
///             sqlx::query("insert into audit(message) values ($1)")
///                 .bind(n)
///                 .execute(&mut *conn)
///                 .await
///                 .map_err(edomata_backend::BackendError::unknown)?;
///         }
///         Ok(())
///     })
/// });
/// # let _ = handler;
/// ```
pub struct SqlxHandler<N> {
    run: Arc<HandlerFn<N>>,
}

/// The function behind a [`SqlxHandler`].
type HandlerFn<N> = dyn for<'a> Fn(&'a NonEmpty<N>, &'a mut PgConnection) -> BoxFuture<'a, Result<(), BackendError>>
    + Send
    + Sync;

impl<N> Clone for SqlxHandler<N> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
        }
    }
}

impl<N> std::fmt::Debug for SqlxHandler<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SqlxHandler(<fn>)")
    }
}

impl<N> SqlxHandler<N> {
    /// Wraps a handler function.
    pub fn new<F>(f: F) -> Self
    where
        F: for<'a> Fn(
                &'a NonEmpty<N>,
                &'a mut PgConnection,
            ) -> BoxFuture<'a, Result<(), BackendError>>
            + Send
            + Sync
            + 'static,
    {
        Self { run: Arc::new(f) }
    }

    /// Runs the handler on `conn`, the connection of the enclosing
    /// transaction.
    ///
    /// # Errors
    ///
    /// Whatever the handler returns; an error rolls the save back.
    pub async fn call(
        &self,
        notifications: &NonEmpty<N>,
        conn: &mut PgConnection,
    ) -> Result<(), BackendError> {
        (self.run)(notifications, conn).await
    }
}

/// PostgreSQL storage driver for CQRS aggregates: the `states`, `outbox`
/// and `commands` tables.
///
/// States are saved with optimistic concurrency on their `version` column;
/// a concurrent update is reported as `BackendError::VersionConflict` and
/// retried by the command handler. A [`SqlxHandler`] registered with
/// `with_event_handler` runs in the same transaction as the save.
///
/// ```no_run
/// # use edomata_core::*;
/// # use edomata_backend::cqrs::Backend;
/// # use edomata_sqlx::{PGNaming, SqlxCqrsDriver};
/// # struct Counter;
/// # impl CqrsModel for Counter { type State = i32; type Rejection = String; fn initial(&self) -> i32 { 0 } }
/// # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// let driver = SqlxCqrsDriver::new(PGNaming::prefixed_str("counters")?, pool).await?;
/// let backend = Backend::builder(Counter, Counter.dsl::<i32, String>())
///     .driver(driver)
///     .build_default()
///     .await?;
/// # let _ = backend; Ok(()) }
/// ```
#[derive(Clone, Debug)]
pub struct SqlxCqrsDriver {
    naming: PGNaming,
    pool: PgPool,
    auto_setup: bool,
    outbox_notify_channel: Option<String>,
}

impl SqlxCqrsDriver {
    /// A driver that sets the schema and tables up automatically. Mirrors
    /// Scala's `SkunkCQRSDriver.from(naming, pool)`. Same as
    /// [`new_with`](Self::new_with) with `skip_setup = false`.
    ///
    /// # Errors
    ///
    /// [`BackendError::UnknownError`] if the `CREATE SCHEMA` statement (schema
    /// mode only) fails.
    pub async fn new(naming: PGNaming, pool: PgPool) -> Result<Self, BackendError> {
        Self::new_with(naming, pool, false).await
    }

    /// A driver for a schema-mode namespace given as a string.
    ///
    /// # Errors
    ///
    /// [`BackendError::PersistenceError`] if `namespace` is not a valid
    /// [`PGNamespace`], and the errors of [`new`](Self::new).
    pub async fn for_namespace(namespace: &str, pool: PgPool) -> Result<Self, BackendError> {
        let ns = PGNamespace::from_string(namespace).map_err(invalid_namespace)?;
        Self::new(PGNaming::schema(ns), pool).await
    }

    /// A driver with `skip_setup` as in Scala's
    /// `SkunkCQRSDriver.from(naming, pool, skipSetup)`: when `true`, no DDL
    /// is ever executed and the tables of
    /// [`PGSchema::cqrs`](edomata_postgres::PGSchema::cqrs) are assumed to
    /// exist.
    ///
    /// ```no_run
    /// # use edomata_sqlx::{PGNaming, SqlxCqrsDriver};
    /// # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
    /// let driver = SqlxCqrsDriver::new_with(PGNaming::prefixed_str("carts")?, pool, true).await?;
    /// assert!(!driver.auto_setup());
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// With `skip_setup = false`, [`BackendError::UnknownError`] if the setup
    /// DDL fails; never with `true`.
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
            outbox_notify_channel: None,
        })
    }

    /// Raises `NOTIFY channel` in the transaction that inserts outbox rows
    /// (see [`SqlxDriver::with_outbox_notify_channel`](crate::SqlxDriver::with_outbox_notify_channel)).
    pub fn with_outbox_notify_channel(mut self, channel: impl Into<String>) -> Self {
        self.outbox_notify_channel = Some(channel.into());
        self
    }

    /// The naming strategy.
    pub fn naming(&self) -> &PGNaming {
        &self.naming
    }

    /// The connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Whether tables are created automatically (`!skip_setup`).
    pub fn auto_setup(&self) -> bool {
        self.auto_setup
    }
}

impl StorageDriver for SqlxCqrsDriver {
    type Codec<T: 'static> = SqlxCodec<T>;
    type Handler<N: 'static> = SqlxHandler<N>;

    async fn build<S, N>(
        &self,
        model: SharedStateModel<S>,
        state_codec: SqlxCodec<S>,
        notification_codec: SqlxCodec<N>,
        handler: Option<SqlxHandler<N>>,
    ) -> Result<Storage<S, N>, BackendError>
    where
        S: Payload,
        N: Payload,
    {
        let outbox_q = Arc::new(OutboxQueries::with_notify(
            &self.naming,
            notification_codec.sql_type(),
            self.outbox_notify_channel.as_deref(),
        ));
        let commands_q = Arc::new(CommandQueries::new(&self.naming));
        let state_q = Arc::new(StateQueries::new(&self.naming, state_codec.sql_type()));
        if self.auto_setup {
            execute_all(&self.pool, &outbox_q.setup).await?;
            execute_all(&self.pool, &commands_q.setup).await?;
            execute_all(&self.pool, &state_q.setup).await?;
        }
        let updates = Notifications::new();
        let outbox: Arc<dyn OutboxReader<N>> = Arc::new(SqlxOutboxReader {
            pool: self.pool.clone(),
            q: Arc::clone(&outbox_q),
            codec: notification_codec.clone(),
        });
        let repository: Arc<dyn Repository<S, N>> = Arc::new(SqlxCqrsRepository {
            pool: self.pool.clone(),
            states: state_q,
            outbox: outbox_q,
            commands: commands_q,
            state_codec,
            notification_codec,
            model,
            updates: updates.clone(),
            handler,
        });
        Ok(Storage {
            repository,
            outbox,
            updates: Arc::new(updates),
        })
    }
}

struct SqlxCqrsRepository<S, N> {
    pool: PgPool,
    states: Arc<StateQueries>,
    outbox: Arc<OutboxQueries>,
    commands: Arc<CommandQueries>,
    state_codec: SqlxCodec<S>,
    notification_codec: SqlxCodec<N>,
    model: SharedStateModel<S>,
    updates: Notifications,
    handler: Option<SqlxHandler<N>>,
}

impl<S: Payload, N: Payload> SqlxCqrsRepository<S, N> {
    async fn state_of(
        &self,
        executor: impl sqlx::PgExecutor<'_>,
        id: &str,
    ) -> Result<AggregateState<S>, BackendError> {
        let row = sqlx::query(&self.states.get)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(map_sqlx)?;
        match row {
            None => Ok(AggregateState::new(self.model.initial(), 0)),
            Some(row) => Ok(AggregateState {
                state: self.state_codec.decode_row(&row, "state")?,
                version: row.try_get("version").map_err(map_sqlx)?,
            }),
        }
    }
}

#[async_trait]
impl<S: Payload, N: Payload> RepositoryReader<S> for SqlxCqrsRepository<S, N> {
    async fn get(&self, id: &str) -> Result<AggregateState<S>, BackendError> {
        self.state_of(&self.pool, id).await
    }
}

#[async_trait]
impl<S: Payload, N: Payload> Repository<S, N> for SqlxCqrsRepository<S, N> {
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S>, BackendError> {
        let mut conn = self.pool.acquire().await.map_err(map_sqlx)?;
        if command_exists(&mut *conn, &self.commands, cmd.id).await? {
            return Ok(CommandState::Redundant);
        }
        Ok(CommandState::Aggregate(
            self.state_of(&mut *conn, cmd.address).await?,
        ))
    }

    async fn save(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        let now = now();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let affected = sqlx::query(&self.states.put)
            .bind(cmd.address)
            .bind(self.state_codec.encode(&new_state)?)
            .bind(version)
            .execute(&mut *tx)
            .await
            .map_err(map_write)?
            .rows_affected();
        match affected {
            1 => {}
            0 => return Err(BackendError::VersionConflict),
            other => {
                return Err(BackendError::persistence(format!(
                    "expected to upsert state, but got invalid response from database! affected rows: {other}"
                )));
            }
        }
        if let Some(ns) = NonEmpty::from_vec(notifications) {
            insert_outbox(
                &mut tx,
                &self.outbox,
                &self.notification_codec,
                cmd.address,
                now,
                cmd.metadata,
                &ns,
            )
            .await?;
            if let Some(handler) = &self.handler {
                handler.call(&ns, &mut tx).await?;
            }
        }
        insert_command(&mut tx, &self.commands, cmd).await?;
        tx.commit().await.map_err(map_write)?;
        self.updates.notify_state();
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
