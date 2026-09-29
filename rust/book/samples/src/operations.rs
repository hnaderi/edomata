//! Samples of the "Operations" chapter: schema management, start-up order,
//! running relays in production and multi-tenant deployments. They need
//! PostgreSQL (and a broker), so they are compiled but not run, except the
//! pure DDL test.

use std::sync::Arc;
use std::time::Duration;

use crate::eventsourcing::{Account, AccountModel, Command, Event, Notification, Rejection};
use edomata_backend::BackendError;
use edomata_backend::eventsourcing::Backend;
use edomata_broker::postgres::{LeaderLock, PgCheckpointStore, listen};
use edomata_broker::{
    CancellationToken, JournalRelay, MessageEncoder, OutboxRelay, Publisher, RelayConfig,
    RelayError, RetryPolicy,
};
use edomata_core::DomainModel;
use edomata_sqlx::{PGNaming, PGSchema, PgPool, SqlxCodec, SqlxDriver, SqlxMigrations};

type AccountBackend = Backend<Account, Event, Rejection, Notification>;

// ANCHOR: flyway_file
/// The content of a Flyway migration (`V1__create_accounts_tables.sql`):
/// every statement of `PGSchema`, each ending with a semicolon.
pub fn flyway_migration(naming: &PGNaming) -> String {
    PGSchema::eventsourcing(naming)
        .iter()
        .map(|statement| format!("{}\n", statement.trim_end()))
        .collect()
}
// ANCHOR_END: flyway_file

// ANCHOR: startup
/// Start-up order of a writer process whose tables come from Flyway.
pub async fn start_writer(pool: PgPool) -> Result<AccountBackend, BackendError> {
    let naming = PGNaming::prefixed_str("accounts").map_err(BackendError::persistence)?;
    // 1. The driver executes no DDL: Flyway created the tables.
    let driver = SqlxDriver::new_with(naming.clone(), pool.clone(), true)
        .await?
        // Wake relays running in other processes (see below).
        .with_outbox_notify_channel("accounts_outbox");
    // 2. Pending event migrations, on the existing journal, before any
    //    command is handled.
    SqlxMigrations::run(&naming, &pool, &crate::migrations::all_migrations()).await?;
    // 3. The backend.
    Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .persisted_snapshot(SqlxCodec::<Account>::jsonb())
        .build_default()
        .await
}
// ANCHOR_END: startup

// ANCHOR: relay_process
/// A dedicated relay process: every replica runs this function, one of them
/// wins the advisory lock and publishes, the others stand by.
pub async fn run_outbox_relay(
    pool: PgPool,
    publisher: Arc<dyn Publisher>,
    cancel: CancellationToken,
) -> Result<(), RelayError> {
    let naming = PGNaming::prefixed_str("accounts").map_err(BackendError::persistence)?;
    // A reader-only backend: `skip_setup`, since the writer owns the schema.
    let driver = SqlxDriver::new_with(naming, pool.clone(), true).await?;
    let backend: AccountBackend =
        Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
            .driver(driver)
            .build_default()
            .await?;

    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        publisher,
        MessageEncoder::<Notification>::serde(),
        RelayConfig::new("accounts")
            .with_batch_size(200)
            .with_poll_interval(Duration::from_secs(10))
            .with_retry(RetryPolicy {
                initial_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(30),
                max_retries: Some(20), // then stop with an error, and restart
            }),
    )
    // Woken by the writers' `NOTIFY accounts_outbox`; polling is the fallback.
    .wake_on(listen(pool.clone(), "accounts_outbox"));

    // Export the counters; they are also emitted as `tracing` events.
    let metrics = relay.metrics();
    let reporter = cancel.clone();
    tokio::spawn(async move {
        while !reporter.is_cancelled() {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let m = metrics.snapshot();
            tracing::info!(
                published = m.published,
                retried = m.retried,
                failed = m.failed,
                lag = m.lag,
                "outbox relay"
            );
        }
    });

    // Use the relay source as the lock key, so replicas share one lock.
    relay
        .run_as_leader(LeaderLock::new(pool, "accounts"), cancel)
        .await
}
// ANCHOR_END: relay_process

// ANCHOR: journal_relay
/// Streaming the journal from a checkpoint stored in PostgreSQL.
pub async fn run_journal_relay(
    backend: &AccountBackend,
    pool: PgPool,
    publisher: Arc<dyn Publisher>,
    cancel: CancellationToken,
) -> Result<(), RelayError> {
    let naming = PGNaming::prefixed_str("accounts").map_err(BackendError::persistence)?;
    // `relay_checkpoints` table; or `PGSchema::relay_checkpoints` in Flyway.
    let checkpoints = PgCheckpointStore::new(pool.clone(), naming);
    checkpoints.setup().await?;
    let relay = JournalRelay::new(
        Arc::clone(backend.journal()),
        Arc::new(checkpoints),
        publisher,
        MessageEncoder::<Event>::serde(),
        RelayConfig::new("accounts"),
    )
    .wake_on(backend.updates().journal());
    relay
        .run_as_leader(LeaderLock::with_key(pool, "accounts:journal"), cancel)
        .await
}
// ANCHOR_END: journal_relay

// ANCHOR: dedup
/// Consumer side: at-least-once delivery means duplicates. Record the
/// message id (the `edomata-id` header, or `message_id` with RabbitMQ) in
/// the same transaction as the effect, and skip ids already seen.
pub async fn handle_once(
    pool: &PgPool,
    message_id: &str,
    payload: &[u8],
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let inserted =
        sqlx::query("INSERT INTO processed_messages (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(message_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if inserted == 0 {
        return Ok(false); // a redelivery: already handled
    }
    sqlx::query("INSERT INTO account_feed (message_id, payload) VALUES ($1, $2)")
        .bind(message_id)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
// ANCHOR_END: dedup

// ANCHOR: rls
/// Multi-tenant tables with Row-Level Security, for Flyway.
pub fn saas_ddl() -> Result<Vec<String>, edomata_postgres::PGNamespaceError> {
    use edomata_saas::{RlsConfig, SaaSPGSchema};
    let naming = PGNaming::prefixed_str("todos")?;
    // Enables RLS on the states and outbox tables, adds a policy comparing
    // `tenant_id` with the `app.tenant_id` setting, and grants the tables
    // to the `app_reader` role.
    Ok(SaaSPGSchema::cqrs_with(
        &naming,
        "jsonb",
        "jsonb",
        Some(&RlsConfig::new("app_reader", "app.tenant_id")),
    ))
}

/// A read through the RLS-restricted role: the policy only lets the rows
/// of the tenant set in this transaction through.
pub async fn count_tenant_states(reader_pool: &PgPool, tenant: &str) -> Result<i64, sqlx::Error> {
    let mut tx = reader_pool.begin().await?;
    // `true`: the setting is local to the transaction.
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant)
        .execute(&mut *tx)
        .await?;
    let count = sqlx::query_scalar("SELECT count(*) FROM todos_states")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(count)
}
// ANCHOR_END: rls

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flyway_file_holds_every_statement() {
        let naming = PGNaming::prefixed_str("accounts").unwrap();
        let sql = flyway_migration(&naming);
        assert_eq!(
            sql.matches("CREATE TABLE IF NOT EXISTS").count(),
            5,
            "journal, outbox, commands, snapshots, migrations"
        );
        assert!(!sql.contains("CREATE SCHEMA"), "prefixed mode");
        assert!(sql.lines().filter(|l| l.ends_with(';')).count() >= 5);
    }

    #[test]
    fn rls_ddl_enables_row_level_security() {
        let ddl = saas_ddl().unwrap();
        assert!(ddl.iter().any(|s| s.contains("ENABLE ROW LEVEL SECURITY")));
        assert!(
            ddl.iter()
                .any(|s| s.contains("current_setting('app.tenant_id')"))
        );
    }
}
