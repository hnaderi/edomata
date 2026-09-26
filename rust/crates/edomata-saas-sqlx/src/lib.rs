//! # Edomata SaaS sqlx driver
//!
//! A tenant-aware PostgreSQL CQRS driver, the port of `saas-skunk`. It is a
//! drop-in replacement for `edomata_sqlx::SqlxCqrsDriver` whose `states` and
//! `outbox` tables carry `tenant_id` / `owner_id` columns (the DDL of
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

#![forbid(unsafe_code)]

mod codec;
mod driver;
mod queries;

pub use codec::SaaSCodec;
pub use driver::{SaaSSqlxCqrsDriver, TenantStateLister};
pub use edomata_sqlx::SqlxHandler;
