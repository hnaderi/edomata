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

#![forbid(unsafe_code)]

mod migration;
mod namespace;
mod naming;
mod schema;

pub use migration::{EventMigration, MigrationResult};
pub use namespace::{MAX_LEN, PGNamespace, PGNamespaceError};
pub use naming::PGNaming;
pub use schema::{PGSchema, ddl};
