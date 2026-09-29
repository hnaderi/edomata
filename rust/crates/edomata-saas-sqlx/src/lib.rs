//! # Edomata SaaS sqlx driver
//!
//! A tenant-aware PostgreSQL CQRS driver, the port of `saas-skunk`. It is a
//! drop-in replacement for `edomata_sqlx::SqlxCqrsDriver` whose `states`
//! table carries `tenant_id` / `owner_id` columns and whose `outbox` table
//! carries `tenant_id` (the DDL of
//! `edomata_saas::SaaSPGSchema`), populated from the state through
//! [`edomata_saas::TenantExtractor`], so that PostgreSQL Row-Level Security
//! and tenant-scoped indexes can be used.
//!
//! ```no_run
//! # use edomata_saas::*;
//! # use edomata_saas_sqlx::{SaaSCodec, SaaSSqlxCqrsDriver};
//! # use edomata_backend::cqrs::Backend;
//! # use edomata_core::CqrsModel;
//! # #[derive(Clone, serde::Serialize, serde::Deserialize)] struct Todo { title: String }
//! # struct Todos;
//! # impl CqrsModel for Todos { type State = CrudState<Todo>; type Rejection = String; fn initial(&self) -> CrudState<Todo> { CrudState::NonExistent } }
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let service = SaaSCqrsService::<CallerIdentity, String, Todo, String, String>::new(PermissivePolicy, |m| m);
//! let driver = SaaSSqlxCqrsDriver::new(PGNaming::prefixed_str("todos")?, pool).await?;
//! let backend = Backend::builder(Todos, service.domain())
//!     .driver(driver)
//!     .build(SaaSCodec::jsonb_state(), SaaSCodec::jsonb_notification())
//!     .await?;
//! # let _ = backend; Ok(()) }
//! ```
//!
//! ## Tables created by hand
//!
//! With `skip_setup` ([`SaaSSqlxCqrsDriver::new_with`]), generate the tables
//! (and, optionally, the RLS policies) with
//! [`SaaSPGSchema`](edomata_saas::SaaSPGSchema) and run them from a
//! migration tool:
//!
//! ```no_run
//! # use edomata_saas::{PGNaming, RlsConfig, SaaSPGSchema};
//! # use edomata_saas_sqlx::SaaSSqlxCqrsDriver;
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let naming = PGNaming::prefixed_str("todos")?;
//! let rls = RlsConfig::new("app_user", "app.tenant_id");
//! for statement in SaaSPGSchema::cqrs_with(&naming, "jsonb", "jsonb", Some(&rls)) {
//!     println!("{statement}"); // into V1__todos.sql
//! }
//! let driver = SaaSSqlxCqrsDriver::new_with(naming, pool, true).await?;
//! # let _ = driver; Ok(()) }
//! ```
//!
//! ## Where it fits
//!
//! It depends on `edomata-saas` (tenancy types, [`TenantExtractor`](edomata_saas::TenantExtractor)
//! and the tenant-aware DDL), `edomata-backend` (the
//! [`cqrs::StorageDriver`](edomata_backend::cqrs::StorageDriver) trait it
//! implements), `edomata-sqlx` (shared query helpers, the outbox reader and
//! [`SqlxHandler`]), `edomata-postgres` and `edomata-serde`. No library crate
//! builds on it (the examples and the book samples use it); applications use it in place of `edomata-sqlx`'s
//! CQRS driver.
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
mod driver;
mod queries;

pub use codec::SaaSCodec;
pub use driver::{SaaSSqlxCqrsDriver, TenantStateLister};
pub use edomata_sqlx::SqlxHandler;
