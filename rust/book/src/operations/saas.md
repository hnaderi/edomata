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

## Operations

- **Relays**: the outbox of a SaaS aggregate is an ordinary outbox with a `tenant_id` column, so the [relays](relays.md) work unchanged; route or partition by tenant in the publisher (`with_topic` / `with_routing_key`) if consumers are per tenant.
- **Listing a tenant's states**: `TenantStateLister::list_by_tenant` reads the `states` table by `tenant_id`, using the tenant index.
- **Offboarding**: a tenant's data is the rows with its `tenant_id` in `states` and `outbox`; command ids are not per tenant.
