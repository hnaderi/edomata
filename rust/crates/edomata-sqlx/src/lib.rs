//! # Edomata sqlx driver
//!
//! The PostgreSQL storage driver of the Rust port, replacing both the Skunk
//! and the Doobie drivers. It implements
//! [`edomata_backend::eventsourcing::StorageDriver`] ([`SqlxDriver`]) and
//! [`edomata_backend::cqrs::StorageDriver`] ([`SqlxCqrsDriver`]) on top of a
//! [`sqlx::PgPool`], with the same tables, columns, constraints, index names
//! and transaction semantics as the Scala drivers, so Rust and Scala
//! services can share a database.
//!
//! - Payloads go through [`SqlxCodec`] (any [`edomata_backend::Codec`],
//!   `SerdeCodec::jsonb()` by default) and are written in the column's
//!   native wire format by `edomata_serde::pg`.
//! - Table naming follows [`PGNaming`] (`Schema` or `Prefixed`); automatic
//!   setup reuses the DDL of `edomata_postgres` and can be disabled with
//!   `skip_setup`.
//! - Optimistic concurrency and command idempotency rely on the
//!   `(stream, version)` and command id unique constraints: a violation is
//!   reported as `BackendError::VersionConflict`.
//! - [`SqlxMigrations`] runs [`EventMigration`]s, tracked in the
//!   `migrations` table.
//!
//! ```no_run
//! # use edomata_core::*;
//! # use edomata_backend::eventsourcing::Backend;
//! # use edomata_sqlx::{PGNaming, SqlxDriver};
//! # struct Counter;
//! # impl DomainModel for Counter {
//! #     type State = i32; type Event = i32; type Rejection = String;
//! #     fn initial(&self) -> i32 { 0 }
//! #     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! # }
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = sqlx::PgPool::connect("postgres://postgres:postgres@localhost/postgres").await?;
//! let driver = SqlxDriver::new(PGNaming::prefixed_str("counters")?, pool).await?;
//! let dsl = Counter.dsl::<i32, String>();
//! let backend = Backend::builder(Counter, dsl)
//!     .driver(driver)
//!     .persisted_snapshot(Default::default())
//!     .build_default() // jsonb serde codecs
//!     .await?;
//! let service = backend.compile(dsl.router(move |by| dsl.accept(by)));
//! # let _ = service;
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]

mod codec;
mod cqrs;
mod error;
mod eventsourcing;
mod migrations;
pub mod queries;

pub use codec::SqlxCodec;
pub use cqrs::{SqlxCqrsDriver, SqlxHandler};
pub use edomata_postgres::{EventMigration, MigrationResult, PGNamespace, PGNaming, PGSchema};
pub use eventsourcing::SqlxDriver;
pub use migrations::{DEFAULT_MIGRATION_BATCH_SIZE, SqlxMigrations};
pub use sqlx::PgPool;

/// Building blocks for derived drivers (used by `edomata-saas-sqlx`).
pub mod shared {
    pub use crate::error::{assert_inserted, is_unique_violation, map_sqlx, map_write};
    pub use crate::eventsourcing::{
        SqlxOutboxReader, command_exists, execute_all, insert_command, insert_outbox,
        invalid_namespace, notify, now,
    };
}
