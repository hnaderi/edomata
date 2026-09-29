# SaaS multi-tenant module

The code of this chapter is compiled and tested as part of the workspace (`rust/book/samples/src/saas.rs`).

## Overview

The `edomata-saas` crate provides generic multi-tenant CRUD abstractions that automatically enforce **tenant isolation** and **authorisation** on writes, and make tenant filtering structural on reads. With the guarded DSLs, every command is checked before the business logic runs; no manual checks are needed.

Authorisation is **pluggable**: you define your own auth type (JWT claims, API key context, session token, ...) and implement the `AuthPolicy` trait. The crate ships `CallerIdentity` with `PermissivePolicy` and `RoleBasedPolicy` as convenient defaults.

## Getting started

```toml
[dependencies]
edomata-saas = { path = "rust/crates/edomata-saas" }
edomata-saas-sqlx = { path = "rust/crates/edomata-saas-sqlx" } # tenant-aware PostgreSQL driver
```

`edomata-saas` re-exports the core types its APIs take and return (`CommandMessage`, `Decision`, `MessageMetadata`, `NonEmpty`, `PGNaming`, `PGNamespace`), so `use edomata_saas::*;` covers the programs. An application still depends on `edomata-core` for the `CqrsModel` / `DomainModel` traits it implements, and on `edomata-backend` for `Backend::builder`:

```toml
[dependencies]
edomata-core = { git = "https://github.com/beyond-scale-group/edomata" }
edomata-backend = { git = "https://github.com/beyond-scale-group/edomata" }
```

## Core types

### `AuthPolicy`

The central abstraction for authentication and authorisation, implemented for your auth context type `Auth` (the trait requires `Send + Sync`). It has two methods:

- `tenant_id(&self, auth: &Auth) -> TenantId`: the tenant the caller acts for;
- `authorize(&self, auth: &Auth, action: CrudAction) -> Result<(), String>`: `Ok(())` if the caller may perform `action`, `Err(reason)` otherwise.

The [custom auth types](#custom-auth-types) section below implements it.

### `CallerIdentity` and `RoleBasedPolicy`

A built-in auth context (`tenant_id`, `user_id`, `roles`) and a role-based policy mapping each action to the roles it requires:

```rust,ignore
{{#include ../../samples/src/saas.rs:policy}}
```

### `SaaSCommand`, `CrudState`, `CrudAction`

- `SaaSCommand<Auth, C>` wraps the business command with the auth context; it is the `CommandMessage` payload.
- `CrudState<A>` wraps the entity state with tenant and owner: `NonExistent`, `Active { tenant_id, owner_id, data }`, `Deleted { tenant_id, owner_id }`.
- `CrudAction`: `Create`, `Read`, `Update`, `Delete`, used for authorisation.

## Defining a CQRS service

Build a `SaaSCqrsService` from a policy and a function turning guard messages into your rejection type. Its `saas()` DSL offers `guarded_router`, `auth`, `entity_state`, `set`, `modify_s`, `publish`, ...:

```rust,ignore
{{#include ../../samples/src/saas.rs:types}}
```

```rust,ignore
{{#include ../../samples/src/saas.rs:service}}
```

The same exists for event sourcing (`SaaSEventSourcedService`, `SaaSDomainDsl`). For a complete example with typed rejections, a custom auth type and a lifecycle state machine, see `rust/examples/src/bin/product_catalog.rs` and the `product_catalog.rs` test of `edomata-saas`.

## Custom auth types

Any type can be the auth context:

```rust,ignore
{{#include ../../samples/src/saas.rs:custom_auth}}
```

Other examples: an API key context (`key`, `tenant`, `is_admin`), OAuth2 token info, or an internal service identity that is always authorised.

## How guards work

`guarded_router` runs two checks before each command branch:

1. **Tenant check**: for non-`Create` actions, the entity's tenant must match `AuthPolicy::tenant_id(auth)`; a `NonExistent` entity yields `"Entity not found"`, a mismatch `"Tenant mismatch"`.
2. **Authorisation check**: `AuthPolicy::authorize(auth, action)`; a denial yields its reason.

The guard is a program sequenced before the logic (`guard(action).then(logic)`): when it rejects, the logic never runs and nothing is published.

```rust,ignore
{{#include ../../samples/src/saas.rs:guard_tests}}
```

## Bypassing guards (super-admin)

Administrative endpoints can bypass the guards with the `unsafe_` variants (`unsafe_unguarded_router`, `unsafe_unguarded`). The prefix is a naming convention borrowed from Scala that makes every bypass grep-able; it has nothing to do with Rust's `unsafe`, which remains forbidden in every crate.

```rust,ignore
{{#include ../../samples/src/saas.rs:admin}}
```

## Read-side queries

Edomata does not handle reads. The SaaS crate provides abstractions that make tenant filtering structurally required:

```rust,ignore
{{#include ../../samples/src/saas.rs:reads}}
```

`TenantScopedQuery` requires the auth context, so the tenant filter cannot be forgotten; `UnsafeCrossTenantQuery` is for admin dashboards. `TenantAwareReader` does the same for single-entity reads.

## Backend wiring

The tenant-aware driver `SaaSSqlxCqrsDriver` fills the `tenant_id` and `owner_id` columns of the states table and the `tenant_id` column of the outbox table, so that Row-Level Security and tenant-scoped indexes are possible. `SaaSPGSchema` generates the DDL (with optional RLS statements) for migration tools:

```rust,ignore
{{#include ../../samples/src/saas.rs:wiring}}
```

`TenantStateLister::list_by_tenant` lists the states of one tenant, and `SaaSCodec` wraps a payload codec; state codecs also carry the tenant extractor (`TenantExtractor`, implemented by `CrudState`) that fills those columns, notification codecs need none. Deploying with Row-Level Security is covered in [Multi-tenant deployment](../operations/saas.md).

## Build configuration

For maximum enforcement, build every program through the guarded DSLs (`service.saas()`), and use `service.domain()` only to pass the DSL marker to `Backend::builder`. The raw `DomainDsl` / `CqrsDomainDsl` stay reachable (`model.dsl()` and `domain()`), so this is a convention to keep in code review, for example by searching for `.dsl::<` and `unsafe_` in the SaaS code.
