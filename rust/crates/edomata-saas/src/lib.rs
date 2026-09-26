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

#![forbid(unsafe_code)]

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
