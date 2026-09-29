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
//!
//! ## Tables created by hand (Flyway, Liquibase...)
//!
//! [`SqlxDriver::new_with`] and [`SqlxCqrsDriver::new_with`] take the
//! `skip_setup` flag of Scala's `from(naming, ..., skipSetup)`: with `true`,
//! the driver never runs DDL and expects the tables that
//! [`PGSchema`] describes to exist.
//!
//! ```no_run
//! # use edomata_sqlx::{PGNaming, PGSchema, SqlxDriver};
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let naming = PGNaming::prefixed_str("counters")?;
//! // Paste these statements into a migration script such as `V1__counters.sql`.
//! for statement in PGSchema::eventsourcing(&naming) {
//!     println!("{statement}");
//! }
//! let driver = SqlxDriver::new_with(naming, pool, true).await?; // no DDL
//! assert!(!driver.auto_setup());
//! # Ok(()) }
//! ```
//!
//! ## Where it fits
//!
//! It depends on `edomata-core` (programs), `edomata-backend` (the
//! [`StorageDriver`](edomata_backend::eventsourcing::StorageDriver) traits
//! it implements and the command handling built on them), `edomata-postgres`
//! (naming and DDL) and `edomata-serde` (payload wire formats). It is used
//! by `edomata-saas-sqlx` (through [`shared`]) and `edomata-simple`, is
//! the storage of `edomata-e2e`, and backs the tests and examples of
//! `edomata-broker` and the Kafka / RabbitMQ publishers.
//!
//! ## Feature flags
//!
//! This crate has no Cargo feature flags.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

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
