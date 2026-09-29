//! # Edomata PostgreSQL naming and DDL
//!
//! Pure helpers shared by the PostgreSQL storage drivers (no I/O):
//!
//! - [`PGNamespace`]: a validated PostgreSQL identifier (Scala's opaque type)
//! - [`PGNaming`]: the table naming strategy, `Schema` or `Prefixed`
//! - [`PGSchema`]: DDL generation for migration tools (Flyway, Liquibase...),
//!   byte-for-byte identical to the Scala output
//! - [`EventMigration`] / [`MigrationResult`]: event payload migrations
//!
//! ```
//! use edomata_postgres::{PGNaming, PGNamespace, PGSchema};
//!
//! let naming = PGNaming::prefixed(PGNamespace::try_from("accounts").unwrap());
//! let ddl = PGSchema::eventsourcing(&naming);
//! assert!(ddl[0].starts_with("CREATE TABLE IF NOT EXISTS accounts_journal"));
//! assert_eq!(naming.table("journal"), "accounts_journal");
//! assert_eq!(naming.constraint("journal_pk"), "accounts_journal_pk");
//! ```
//!
//! ## Where it fits
//!
//! `edomata-postgres` has no Edomata dependency and does no I/O. It is the
//! shared SQL vocabulary of the PostgreSQL crates: `edomata-sqlx` (the
//! storage drivers and `SqlxMigrations`), `edomata-saas` (`SaaSPGSchema`),
//! `edomata-saas-sqlx`, `edomata-broker` (the relay checkpoints table) and
//! `edomata-simple` (`SimplePGSchema`). It is the port of the
//! Scala `postgres` module.
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

mod migration;
mod namespace;
mod naming;
mod schema;

pub use migration::{EventMigration, MigrationResult};
pub use namespace::{MAX_LEN, PGNamespace, PGNamespaceError};
pub use naming::PGNaming;
pub use schema::{PGSchema, ddl};
