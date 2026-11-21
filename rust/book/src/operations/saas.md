# Multi-tenant deployment

The [SaaS tutorial](../tutorials/saas.md) shows how guarded programs enforce tenant isolation on writes. This chapter covers the database side: tenant-aware tables, Row-Level Security (RLS) for reads, and the roles to run with. The code is in `rust/book/samples/src/operations.rs`.

## Tables

`SaaSSqlxCqrsDriver` uses the CQRS tables with extra columns, filled from the state by the state codec's `TenantExtractor` (`CrudState` implements it):

| Table | Extra columns | Indexes |
|-------|---------------|---------|
| `states` | `tenant_id`, `owner_id` | `(tenant_id)`, `(tenant_id, owner_id)` |
| `outbox` | `tenant_id` | `(tenant_id)` |
| `commands` | none | none |

`SaaSPGSchema::cqrs` / `cqrs_with` generate this DDL (identical to the Scala `SaaSPGSchema`), and the driver runs the same DDL itself unless it is built with `new_with(naming, pool, true)`.

## Row-Level Security

With an `RlsConfig`, `SaaSPGSchema::cqrs_with` also enables RLS on `states` and `outbox`, creates a policy per table that only lets through rows whose `tenant_id` equals the session setting you name, and grants `SELECT, INSERT, UPDATE` on both tables to the role you name:

```rust,ignore
{{#include ../../samples/src/operations.rs:rls}}
```

The generated policy is `USING (tenant_id = current_setting('app.tenant_id'))`: a session of that role that never set `app.tenant_id` gets an error, and one whose setting is empty (after a transaction-local setting ended) matches no row; neither sees other tenants' rows.

## Roles

RLS restricts the roles it applies to; the owner of a table (and superusers) bypass it unless you add `FORCE ROW LEVEL SECURITY`. The driver itself does not set the tenant setting: one command-handling process writes the rows of every tenant, and isolation on writes comes from the guarded programs. So:

- run the **writer** (the backend with `SaaSSqlxCqrsDriver`) and the Flyway migrations as the owner of the tables;
- run **read-side queries** (the `TenantScopedQuery` implementations, admin excepted) as the RLS role named in `RlsConfig`, setting the tenant per transaction with `set_config(name, value, true)`, as in `count_tenant_states` above. `true` makes the setting local to the transaction, so a pooled connection never carries another tenant's setting into its next use;
- keep cross-tenant reads (`UnsafeCrossTenantQuery`, dashboards, relays) on a separate role or pool, so that they are visible in configuration and audits.

RLS is then a second line of defence for reads: a query that forgets its `WHERE tenant_id = ...` still sees one tenant only. The workspace tests this setup end to end (`rust/crates/edomata-saas-sqlx/tests/postgres.rs`).

## Event-sourced services: mandatory Row-Level Security

The section above covers the CQRS tables, where RLS is optional. For **event-sourced** SaaS services it is mandatory, and PostgreSQL itself refuses cross-tenant reads and writes whatever the application code does. The Scala library has no equivalent (its SaaS module only checks tenants in code), so this is specific to the Rust port. The code is in `edomata-saas` (`RlsAccessPolicy`, `SaaSPGSchema::eventsourcing_rls`) and `edomata-saas-sqlx` (`RlsGateway`); the tests are `rust/crates/edomata-saas-sqlx/tests/rls.rs` and `rust/crates/edomata-saas/tests/rls.rs`.

### The contract, at compile time

`SaaSEventSourcedService` has no `new`: it is built with a typestate builder whose `build` only exists once the authorization policy, the rejection mapping **and** an `RlsAccessPolicy` are set. Leaving the access policy out does not compile (`compile_fail` doctests on the type). Only the tenant id type is left to you: `RlsAccessPolicy<T>` where `T: TenantKey` (implemented for `TenantId`, `String` and `&str`).

```rust,ignore
let policy: RlsAccessPolicy<TenantId> = RlsAccessPolicy::with_prefix("shop")?; // shop_tenant, shop_system, shop_inspection, shop.tenant_id
let service = SaaSEventSourcedService::builder()
    .auth_policy(RoleBasedPolicy::none())
    .rejection(|reason| reason)
    .access_policy(policy)
    .build();
```

### Three access modes, three PostgreSQL roles

| Mode | Role | Rows | Privileges |
|------|------|------|------------|
| tenant | `{prefix}_tenant` | the rows of the tenant set in the transaction (`USING` and `WITH CHECK`) | `SELECT, INSERT` (`UPDATE` on `snapshots`); no `DELETE` |
| system (relays, data migrations) | `{prefix}_system` | all (`USING (true) WITH CHECK (true)`) | `SELECT, INSERT, UPDATE, DELETE` |
| inspection (read-only agent) | `{prefix}_inspection` | all, `FOR SELECT` | `SELECT` only |

The roles are `NOLOGIN`, own nothing, and have no `BYPASSRLS`. A connection reaches one with `SET LOCAL ROLE`, which PostgreSQL only allows for a role the connection's login is a **member** of. That membership is the boundary in front of the system and inspection modes: no policy trusts a session variable that the application can `SET`. Give each login only the modes it needs (`SaaSPGSchema::rls_memberships`): the web tier's login the tenant mode, a relay's login the system mode, an inspection agent's login the inspection mode. Create logins `NOINHERIT`: without the explicit `SET LOCAL ROLE` of the entry point they have no access at all.

### The single entry point

`RlsGateway::begin(&access)` is the only place that switches role and sets the tenant. Inside a fresh transaction it runs `SET LOCAL ROLE` and `select set_config('shop.tenant_id', tenant, true)`: the setting is **transaction-local**, so the next user of a pooled connection never inherits a tenant. The policies read it with `NULLIF(current_setting(name, true), '')`: a missing or empty tenant is NULL and matches nothing. `RlsGateway::run(&access, |conn| ...)` runs a closure in that transaction and commits on `Ok`.

An `Access` can only be produced by the policy (`tenant_access`, `system_access`, `inspection_access`), and `tenant_access` validates the tenant. For a backend, `gateway.driver(naming)` returns an `SqlxDriver` whose every transaction (reads included) goes through `begin` for the *ambient* access, and the web tier wraps each request:

```rust,ignore
let gateway = RlsGateway::new(pool, service.access_policy().clone());
let driver = gateway.driver(PGNaming::prefixed_str("shop")?).await?; // skip_setup: the app does not own the tables
let backend = Backend::builder(Model, service.domain()).driver(driver)/* ... */.build_default().await?;
let run = backend.compile(app);
let access = service.access_policy().tenant_access(&tenant)?;
gateway.with_access(&access, run(command)).await?; // the tenant comes from authentication, never from the request body
```

The ambient access is a task-local value. A driver used outside `with_access` fails (`no access context`) instead of running unscoped, and a task started with `tokio::spawn` does not inherit it.

### Schema: a generated `tenant_id`, derived from the tenant-prefixed id

A downstream harness already makes every aggregate id and command id `{tenant}/{id}` / `{tenant}/{key}`. Each of the four tables therefore gets a **generated** column, `tenant_id text GENERATED ALWAYS AS (CASE WHEN position('/' in <col>) > 1 THEN split_part(<col>, '/', 1) END) STORED NOT NULL`:

| Table | Derived from | Notes |
|-------|--------------|-------|
| `journal` | `stream` | index `(tenant_id, seqnr)` |
| `outbox` | `stream` | the stream is the command's address |
| `commands` | `id` | a `CHECK` also requires `address` to belong to the same tenant |
| `snapshots` | `id` | |

Why generated rather than an explicit column filled by the driver: the driver's `INSERT`s do not change (so `SqlxDriver` and Scala-compatible readers keep working), and the column **cannot disagree with the id**: no code path, bug or role can write a row whose `tenant_id` differs from its stream. An explicit column would need every writer (driver, migrations, system jobs) to fill it consistently. The price is the `{tenant}/{id}` invariant, enforced by the database: an id without a `/` (or with an empty tenant) derives NULL and the `NOT NULL` constraint refuses the row, for every role.

**A tenant id may not contain `/`.** Tenant `a/b` would be read by the database as tenant `a` (`a/b/x` is `a`'s id `b/x`). `RlsAccessPolicy::tenant_access` and `scoped_id` refuse empty keys, keys containing `/`, and NUL (`TenantKeyError`); the refusal is tested. Ids after the first `/` are free.

RLS is **enabled and forced** on all four tables (`FORCE`: a table owner does not bypass it), each with a `FOR ALL` tenant policy carrying `USING` and `WITH CHECK`, a system policy and an inspection policy. The application roles own nothing: the tables belong to the migration role.

### Migration order

`SaaSPGSchema::eventsourcing_rls(naming, event_type, notification_type, snapshot_type, &policy)` returns the table statements followed by the RLS statements, byte-stable and golden-tested (`rust/tests/golden/saas_es_rls_*.sql`; the golden files are generated by this crate, there is no Scala oracle). Apply, with your migration tool:

1. `SaaSPGSchema::rls_roles(&policy)`: creates the three roles if missing (idempotent; roles are cluster-wide, so a DBA may run it once).
2. `SaaSPGSchema::eventsourcing_rls(...)`, **as the owner of the tables** (not an application login).
3. `SaaSPGSchema::rls_memberships(&policy, "shop_web", &[AccessMode::Tenant])` for each login.

The table statements are idempotent (`IF NOT EXISTS`) but `CREATE POLICY` has no `IF NOT EXISTS`: applying step 2 twice fails, so let the migration tool run it once (Flyway and Liquibase do). To retrofit existing tables, add the column with `ALTER TABLE ... ADD COLUMN tenant_id text GENERATED ALWAYS AS (...) STORED NOT NULL` (the expression above); this rewrites the table and fails if an existing id has no tenant prefix. The `migrations` and `relay_checkpoints` tables hold no tenant data and get no RLS.

### What this does not cover

- RLS protects against **mistakes** (a missing guard, a wrong id, a query without `WHERE`) and against a compromised web tier that can only reach the tenant role through the entry point's login. Code that can run arbitrary SQL as the tenant role can still `set_config` another tenant: the tenant setting is a selector, not a secret. Keep the system and inspection memberships off that login.
- The backend's in-process caches (snapshot cache, command cache) are not tenant-scoped: an entry written by tenant A can be served to tenant B from memory without reaching the database. Writes are still refused by the database, and the application guard checks the tenant; to also make *reads* database-enforced, disable the caches (`disable_cache`, a snapshot store that keeps nothing, as the end-to-end tests do).
- A persisted snapshot store writes evicted snapshots from a background task that has no caller. Give it an explicit scope, which needs a login that is a member of the system role: `driver.with_snapshot_write_scope(gateway.fixed_scope(policy.system_access()))`. Without it those writes fail closed.
- Relays use the system mode: build their driver on a gateway whose login holds the system role, and run `gateway.with_access(&policy.system_access(), ...)`.

## Operations

- **Relays**: the outbox of a SaaS aggregate is an ordinary outbox with a `tenant_id` column, so the [relays](relays.md) work unchanged; route or partition by tenant in the publisher (`with_topic` / `with_routing_key`) if consumers are per tenant.
- **Listing a tenant's states**: `TenantStateLister::list_by_tenant` reads the `states` table by `tenant_id`, using the tenant index.
- **Offboarding**: a tenant's data is the rows with its `tenant_id` in `states` and `outbox`; command ids are not per tenant. With the event-sourced RLS tables, it is the rows with its `tenant_id` in `journal`, `outbox`, `commands` and `snapshots` (command ids are tenant-prefixed there); delete them in the system mode.
