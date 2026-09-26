//! Samples of the "Running" and "PostgreSQL backend" chapters. They need a
//! database, so they are compiled but not run by the tests.

use crate::eventsourcing::{
    Account, AccountModel, Command, Event, Notification, Rejection, account_service,
};
use edomata_backend::eventsourcing::Backend;
use edomata_backend::{BackendError, RetryConfig};
use edomata_core::*;
use edomata_sqlx::{PGNaming, PGSchema, PgPool, SqlxCodec, SqlxDriver};

// ANCHOR: minimal
/// Wires the account service of the previous chapters to PostgreSQL.
pub async fn main_example(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create the connection pool.
    let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    // 2. Create the driver: `account` becomes a PostgreSQL schema.
    let driver = SqlxDriver::for_namespace("account", pool).await?;

    // 3. Build the backend. `build_default` uses serde `jsonb` codecs for
    //    events and notifications; the state codec of persisted snapshots
    //    is given explicitly.
    let backend = Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .persisted_snapshot(SqlxCodec::<Account>::jsonb())
        .build_default()
        .await?;

    // 4. Compile the pure `Edomaton` into a service talking to the database.
    let service = backend.compile(account_service());

    // 5. Use it!
    let result = service(CommandMessage::new(
        "cmd-1",
        chrono::Utc::now(),
        "account-123",
        Command::Open,
    ))
    .await?;
    println!("Result: {result:?}");

    backend.close().await?;
    Ok(())
}
// ANCHOR_END: minimal

// ANCHOR: builder_options
pub async fn builder_options(
    pool: PgPool,
) -> Result<Backend<Account, Event, Rejection, Notification>, BackendError> {
    let driver = SqlxDriver::for_namespace("account", pool).await?;
    Backend::builder(AccountModel, AccountModel.dsl::<Command, Notification>())
        .driver(driver)
        .in_mem_snapshot(200) // or `.persisted_snapshot(codec)`
        .with_retry_config(RetryConfig {
            initial_delay: std::time::Duration::from_secs(2),
            ..RetryConfig::default()
        })
        .with_command_cache_size(1000)
        .build(
            SqlxCodec::<Event>::jsonb(),
            SqlxCodec::<Notification>::jsonb(),
        ) // explicit codecs
        .await
}
// ANCHOR_END: builder_options

// ANCHOR: prefixed
pub async fn prefixed_naming(pool: PgPool) -> Result<SqlxDriver, BackendError> {
    // Tables `account_journal`, `account_outbox`, ... in the current schema,
    // and no `CREATE SCHEMA`.
    let naming =
        PGNaming::prefixed_str("account").map_err(|e| BackendError::persistence(e.to_string()))?;
    SqlxDriver::new(naming, pool).await
}
// ANCHOR_END: prefixed

// ANCHOR: flyway
pub fn print_ddl() -> Result<(), Box<dyn std::error::Error>> {
    let naming = PGNaming::prefixed_str("accounts")?;
    // Event sourcing tables (journal, outbox, commands, snapshots, migrations), jsonb payloads.
    for statement in PGSchema::eventsourcing(&naming) {
        println!("{statement}");
    }
    // Explicit payload types, and the CQRS tables (states, outbox, commands).
    let _ = PGSchema::eventsourcing_with(&naming, "jsonb", "jsonb", "bytea");
    let _ = PGSchema::cqrs(&naming);
    Ok(())
}

pub async fn skip_setup(pool: PgPool) -> Result<SqlxDriver, BackendError> {
    let naming =
        PGNaming::prefixed_str("accounts").map_err(|e| BackendError::persistence(e.to_string()))?;
    // `skip_setup = true`: the driver never executes DDL; Flyway owns the schema.
    SqlxDriver::new_with(naming, pool, true).await
}
// ANCHOR_END: flyway

// ANCHOR: cqrs_backend
pub async fn cqrs_backend(pool: PgPool) -> Result<(), BackendError> {
    use crate::cqrs::{Notification, OrderModel, order_service};
    use edomata_backend::cqrs::Backend;
    use edomata_sqlx::{SqlxCqrsDriver, SqlxHandler};

    // A handler runs inside the transaction that saves the state: a
    // projection can be updated atomically with the aggregate.
    let handler: SqlxHandler<Notification> = SqlxHandler::new(|notifications, _connection| {
        Box::pin(async move {
            for n in notifications.iter() {
                println!("saved with: {n:?}");
            }
            Ok(())
        })
    });
    let driver = SqlxCqrsDriver::for_namespace("orders", pool).await?;
    let backend = Backend::builder(
        OrderModel,
        OrderModel.dsl::<crate::cqrs::Command, Notification>(),
    )
    .driver(driver)
    .with_event_handler(handler)
    .build_default()
    .await?;
    let _service = backend.compile(order_service());
    Ok(())
}
// ANCHOR_END: cqrs_backend
