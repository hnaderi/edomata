//! # Edomata SaaS
//!
//! Multi-tenant abstractions for Edomata, the port of `modules/saas`:
//!
//! - tenancy types: [`TenantId`], [`UserId`], [`CallerIdentity`],
//!   [`SaaSCommand`], [`CrudAction`], [`CrudState`]
//! - authorization: the [`AuthPolicy`] trait with [`PermissivePolicy`] and
//!   [`RoleBasedPolicy`], and the [`SaaSGuard`] checks
//! - guarded DSLs: [`SaaSDomainDsl`] (event sourcing) and [`SaaSCqrsDsl`]
//!   (CQRS), plus the [`SaaSEventSourcedService`] / [`SaaSCqrsService`]
//!   bundles
//! - tenant-scoped reads: [`TenantAwareReader`], [`TenantScopedQuery`],
//!   [`UnsafeCrossTenantQuery`]; [`TenantExtractor`] for tenant-aware
//!   storage
//! - [`SaaSPGSchema`]: DDL with `tenant_id` / `owner_id` columns and
//!   optional Row-Level Security
//!
//! Application code should build programs through the guarded DSLs: every
//! command routed with `guarded_router` is checked for tenant isolation and
//! authorization before the business logic runs; anything named `unsafe_*`
//! bypasses the checks on purpose and is meant for administrative use.
//!
//! ```
//! use edomata_saas::*;
//! use edomata_core::RequestContext;
//!
//! type Dsl = SaaSDomainDsl<CallerIdentity, String, String, String, String, String>;
//!
//! // Updates need the `write` role; every other action only tenant isolation.
//! let policy = RoleBasedPolicy::new(|action| match action {
//!     CrudAction::Update => ["write".to_string()].into(),
//!     _ => Default::default(),
//! });
//! let dsl: Dsl = SaaSDomainDsl::new(policy, |reason| reason);
//! let d = dsl.clone();
//! let app = dsl.guarded_router(move |title: String| (CrudAction::Update, d.accept(title)));
//!
//! let state = CrudState::active("tenant-a", "alice", "old title".to_string());
//! let run = |caller: CallerIdentity| {
//!     let cmd = CommandMessage::new("cmd-1", chrono::DateTime::UNIX_EPOCH, "todo-1", SaaSCommand::new(caller, "new title".to_string()));
//!     futures::executor::block_on(app.run(RequestContext::new(cmd, state.clone()))).result
//! };
//! assert!(run(CallerIdentity::new("tenant-a", "alice", ["write"])).is_accepted());
//! assert_eq!(
//!     run(CallerIdentity::new("tenant-b", "mallory", ["write"])).rejections().map(|r| r.to_vec()),
//!     Some(vec!["Tenant mismatch".to_string()]),
//! );
//! ```
//!
//! ## Where it fits
//!
//! Built on `edomata-core` (the guarded DSLs produce ordinary
//! [`Edomaton`](edomata_core::Edomaton) / [`Stomaton`](edomata_core::Stomaton)
//! programs) and `edomata-postgres` (naming and DDL helpers). It has no
//! storage of its own: run the programs with any `edomata-backend` driver,
//! or with `edomata-saas-sqlx`, whose CQRS driver fills the `tenant_id` /
//! `owner_id` columns of [`SaaSPGSchema`] through [`TenantExtractor`].
//!
//! ## Feature flags
//!
//! - `serde` (enabled by default): `Serialize` / `Deserialize` for
//!   [`TenantId`], [`UserId`], [`CallerIdentity`], [`CrudAction`],
//!   [`CrudState`] and [`SaaSCommand`], with the JSON shape of the Scala
//!   module (ids are plain strings). Also enables `edomata-core/serde`.
//!   Needed by `edomata-saas-sqlx`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod dsl;
mod extractor;
mod guard;
mod reader;
mod schema;
mod service;
mod types;

pub use dsl::{SaaSCqrsApp, SaaSCqrsDsl, SaaSDomainDsl, SaaSEsApp};
pub use extractor::TenantExtractor;
pub use guard::SaaSGuard;
pub use reader::{
    CrossTenantQueryFn, ScopedQueryFn, TenantAwareReader, TenantScopedQuery, UnsafeCrossTenantQuery,
};
pub use schema::{RlsConfig, SaaSPGSchema, ddl};
pub use service::{SaaSCqrsService, SaaSEventSourcedService};
pub use types::{
    AuthPolicy, CallerIdentity, CrudAction, CrudState, PermissivePolicy, RoleBasedPolicy,
    SaaSCommand, TenantId, UserId,
};

// Re-exports so that SaaS applications need not depend on `edomata-core`
// directly (Scala's `edomata.saas` package object).
pub use edomata_core::{CommandMessage, Decision, MessageMetadata, NonEmpty};
pub use edomata_postgres::{PGNamespace, PGNaming};
