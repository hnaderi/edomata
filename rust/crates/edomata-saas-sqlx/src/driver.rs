//! The tenant-aware CQRS driver.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_backend::cqrs::{
    AggregateState, CommandState, Notifications, NotificationsPublisher, Repository,
    RepositoryReader, SharedStateModel, Storage, StorageDriver,
};
use edomata_backend::{BackendError, CommandRef, OutboxReader, Payload, SeqNr};
use edomata_core::{MessageMetadata, NonEmpty};
use edomata_postgres::{PGNamespace, PGNaming, ddl};
use edomata_saas::TenantId;
use edomata_sqlx::queries::{CommandQueries, OutboxQueries};
use edomata_sqlx::shared::{
    SqlxOutboxReader, assert_inserted, command_exists, execute_all, insert_command,
    invalid_namespace, map_sqlx, map_write, now,
};
use edomata_sqlx::{SqlxCodec, SqlxHandler};
use sqlx::{PgConnection, PgPool, Row};

use crate::codec::SaaSCodec;
use crate::queries::{SaaSOutboxQueries, SaaSStateQueries};

/// Tenant-aware PostgreSQL CQRS driver. Mirrors Scala's
/// `SaaSSkunkCQRSDriver`; constructors follow `edomata_sqlx::SqlxCqrsDriver`.
#[derive(Clone, Debug)]
pub struct SaaSSqlxCqrsDriver {
    naming: PGNaming,
    pool: PgPool,
    auto_setup: bool,
}

impl SaaSSqlxCqrsDriver {
    /// A driver that sets the schema and tables up automatically.
    pub async fn new(naming: PGNaming, pool: PgPool) -> Result<Self, BackendError> {
        Self::new_with(naming, pool, false).await
    }

    /// A driver for a schema-mode namespace given as a string.
    pub async fn for_namespace(namespace: &str, pool: PgPool) -> Result<Self, BackendError> {
        let ns = PGNamespace::from_string(namespace).map_err(invalid_namespace)?;
        Self::new(PGNaming::schema(ns), pool).await
    }

    /// A driver with `skip_setup` as in Scala (no DDL is ever executed when
    /// `true`).
    pub async fn new_with(
        naming: PGNaming,
        pool: PgPool,
        skip_setup: bool,
    ) -> Result<Self, BackendError> {
        if !skip_setup {
            execute_all(&pool, &ddl::schema_statement(&naming)).await?;
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

impl StorageDriver for SaaSSqlxCqrsDriver {
    type Codec<T: 'static> = SaaSCodec<T>;
    type Handler<N: 'static> = SqlxHandler<N>;

    async fn build<S, N>(
        &self,
        model: SharedStateModel<S>,
        state_codec: SaaSCodec<S>,
        notification_codec: SaaSCodec<N>,
        handler: Option<SqlxHandler<N>>,
    ) -> Result<Storage<S, N>, BackendError>
    where
        S: Payload,
        N: Payload,
    {
        let states_q = Arc::new(SaaSStateQueries::new(&self.naming, state_codec.sql_type()));
        let outbox_q = Arc::new(SaaSOutboxQueries::new(
            &self.naming,
            notification_codec.sql_type(),
        ));
        // Reading and marking outbox rows does not involve the tenant column,
        // so the standard outbox reader is reused.
        let outbox_read_q = Arc::new(OutboxQueries::new(
            &self.naming,
            notification_codec.sql_type(),
        ));
        let commands_q = Arc::new(CommandQueries::new(&self.naming));
        if self.auto_setup {
            execute_all(&self.pool, &outbox_q.setup).await?;
            execute_all(&self.pool, &commands_q.setup).await?;
            execute_all(&self.pool, &states_q.setup).await?;
        }
        let updates = Notifications::new();
        let outbox: Arc<dyn OutboxReader<N>> = Arc::new(SqlxOutboxReader {
            pool: self.pool.clone(),
            q: outbox_read_q,
            codec: notification_codec.codec().clone(),
        });
        let repository: Arc<dyn Repository<S, N>> = Arc::new(SaaSRepository {
            pool: self.pool.clone(),
            states: states_q,
            outbox: outbox_q,
            commands: commands_q,
            state_codec,
            notification_codec: notification_codec.codec().clone(),
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

struct SaaSRepository<S, N> {
    pool: PgPool,
    states: Arc<SaaSStateQueries>,
    outbox: Arc<SaaSOutboxQueries>,
    commands: Arc<CommandQueries>,
    state_codec: SaaSCodec<S>,
    notification_codec: SqlxCodec<N>,
    model: SharedStateModel<S>,
    updates: Notifications,
    handler: Option<SqlxHandler<N>>,
}

impl<S: Payload, N: Payload> SaaSRepository<S, N> {
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

    async fn insert_outbox(
        &self,
        tx: &mut PgConnection,
        stream: &str,
        time: chrono::DateTime<chrono::Utc>,
        metadata: &MessageMetadata,
        tenant_id: &str,
        notifications: &[N],
    ) -> Result<(), BackendError> {
        for n in notifications {
            let affected = sqlx::query(&self.outbox.insert)
                .bind(self.notification_codec.encode(n)?)
                .bind(stream)
                .bind(time)
                .bind(metadata.correlation.as_deref())
                .bind(metadata.causation.as_deref())
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .map_err(map_write)?
                .rows_affected();
            assert_inserted(affected, 1)?;
        }
        Ok(())
    }
}

#[async_trait]
impl<S: Payload, N: Payload> RepositoryReader<S> for SaaSRepository<S, N> {
    async fn get(&self, id: &str) -> Result<AggregateState<S>, BackendError> {
        self.state_of(&self.pool, id).await
    }
}

#[async_trait]
impl<S: Payload, N: Payload> Repository<S, N> for SaaSRepository<S, N> {
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
        // Scala falls back to empty strings when the state has no tenant.
        let (tenant_id, owner_id) = self
            .state_codec
            .tenant_and_owner(&new_state)
            .map(|(t, o)| (t.into_string(), o.into_string()))
            .unwrap_or_else(|| (String::new(), String::new()));
        let now = now();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let affected = sqlx::query(&self.states.put)
            .bind(cmd.address)
            .bind(self.state_codec.encode(&new_state)?)
            .bind(&tenant_id)
            .bind(&owner_id)
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
            self.insert_outbox(&mut tx, cmd.address, now, cmd.metadata, &tenant_id, &ns)
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
        // Notifications without a state change carry no tenant context (as
        // in Scala).
        let now = now();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        self.insert_outbox(&mut tx, cmd.address, now, cmd.metadata, "", &notifications)
            .await?;
        tx.commit().await.map_err(map_sqlx)?;
        self.updates.notify_outbox();
        Ok(())
    }
}

/// Tenant-scoped listing of states, the counterpart of Scala's
/// `listByTenant` query.
pub struct TenantStateLister<S> {
    pool: PgPool,
    states: SaaSStateQueries,
    codec: SaaSCodec<S>,
}

impl<S: Payload> TenantStateLister<S> {
    /// Builds a lister for the driver's tables.
    pub fn new(driver: &SaaSSqlxCqrsDriver, codec: SaaSCodec<S>) -> Self {
        Self {
            pool: driver.pool.clone(),
            states: SaaSStateQueries::new(&driver.naming, codec.sql_type()),
            codec,
        }
    }

    /// All states of a tenant.
    pub async fn list_by_tenant(
        &self,
        tenant: &TenantId,
    ) -> Result<Vec<AggregateState<S>>, BackendError> {
        let rows = sqlx::query(&self.states.list_by_tenant)
            .bind(tenant.value())
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.iter()
            .map(|row| {
                Ok(AggregateState {
                    state: self.codec.decode_row(row, "state")?,
                    version: row.try_get("version").map_err(map_sqlx)?,
                })
            })
            .collect()
    }
}
