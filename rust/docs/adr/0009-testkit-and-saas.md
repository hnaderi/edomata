# ADR 0009: Test kit and multi-tenant SaaS crates

- Status: accepted
- Date: 2026-09-25
- Milestone: 6 (test kit and SaaS)

## Context

Scala's `munit` module (`DomainSuite`) gives domain tests a fixed
`CommandMessage` and `expect*` helpers. The `saas` module adds tenancy
types, an `AuthPolicy` typeclass, `SaaSGuard`, guarded DSLs for both
automata, tenant-scoped reader traits, a `TenantExtractor` typeclass and
`SaaSPGSchema` (tables with `tenant_id` / `owner_id` columns and optional
Row-Level Security). `saas-skunk` is a CQRS driver that fills those
columns. All three are ported in this milestone.

## Decisions

1. **The test kit is a pair of extension traits, not a test framework.**
   `edomata-testkit` has no framework of its own: `EdomatonAssertions`
   is implemented for every `Edomaton` whose environment is a
   `RequestContext` and `StomatonAssertions` for every unit-output
   `Stomaton` whose environment is a `CommandMessage`; both work with
   `#[test]`, `#[tokio::test]` or any runner. `TestCommand` reproduces
   `DomainSuite`'s defaults (`msgId = "1"`, `address = "sut"`, commands
   timestamped `Instant.MIN`, i.e. `DateTime::<Utc>::MIN_UTC`). The `expect*` helpers panic with
   the same wording as MUnit's `fail(...)` messages so that failures read
   alike in both languages, and `expect_rejection` returns the
   notifications and reasons like the Scala method.

2. **Policies are values, not implicits.** Scala resolves `AuthPolicy[Auth]`
   as a `given`. In Rust the policy is an explicit value held by the
   guarded DSL (`SaaSDomainDsl::new(policy, mk_rejection)`), stored as
   `Arc<dyn AuthPolicy<Auth>>` so DSLs stay cheap to clone into routers.
   `PermissivePolicy` is Scala's default `given AuthPolicy[CallerIdentity]`
   (every action authorised, tenant isolation only) and `RoleBasedPolicy`
   its role-based companion, with the same `"Missing roles: a, b"` message
   (roles sorted, as `BTreeSet` iteration is ordered).

3. **Guards short-circuit before the business logic runs.** `guard(action)`
   is itself a program; `guarded_router` and `guarded` sequence it before
   the logic with `then`, so a guard rejection publishes nothing, exactly
   as Scala's `for` comprehension does. Guard messages (`"Tenant
   mismatch"`, `"Entity not found"`, authorisation reasons) are turned
   into domain rejections by the user-supplied `mk_rejection`.

4. **Bypasses keep the `unsafe_` prefix.** `unsafe_unguarded_router` and
   `unsafe_unguarded` are ordinary safe Rust functions; the prefix is
   Scala's naming convention for code-review visibility and is kept on
   purpose. Rust's `unsafe` keyword is unrelated and remains forbidden.

5. **`TenantExtractor` is implemented on the state type.** Scala's
   `TenantExtractor[S]` typeclass becomes a trait implemented by
   `CrudState<A>`. The driver does not require the bound on every state
   type: `SaaSCodec<T>` pairs a `SqlxCodec<T>` with an optional extractor
   closure (`SaaSCodec::state(codec)` uses the trait, `with_extractor`
   takes any function, `notification(codec)` has none). This keeps
   `StorageDriver::Codec<T>` a single type for states and notifications,
   as the GAT requires, without a marker trait on notifications.

6. **The SaaS driver reuses the sqlx driver.** `SaaSSqlxCqrsDriver` mirrors
   `SaaSSkunkCQRSDriver`: same constructors as `SqlxCqrsDriver`, its own
   `SaaSStateQueries` / `SaaSOutboxQueries` (the `put` upsert writes
   `tenant_id` and `owner_id`, the outbox insert writes `tenant_id`), and
   the standard outbox reader for reading and marking rows, which do not
   involve the tenant column. `edomata-sqlx` exposes the shared helpers it
   needs (`edomata_sqlx::shared`, `edomata_sqlx::queries`) so derived
   drivers do not copy them. As in Scala, a state without a tenant writes
   empty strings and `notify` (a rejection that publishes) writes an empty
   tenant. `TenantStateLister` is the counterpart of Scala's
   `listByTenant` query.

7. **DDL is byte-identical and golden-tested.** `SaaSPGSchema::cqrs` /
   `cqrs_with` reuse `edomata_postgres::ddl` for the schema and commands
   statements and add the tenant-aware states and outbox tables, indexes
   and RLS statements. `GoldenSaaSDDL.scala` (test scope of the Scala
   `saas` module) writes 24 golden files (`saas_cqrs_*.sql`: schema and
   prefixed naming, a quoted namespace, four payload-type combinations,
   with and without RLS) that the Rust test compares byte for byte.

## Tests

Every Scala `saas` suite is ported one to one (guard, both DSLs, services
and types, readers, schema and extractor, the product catalogue scenario).
The driver has PostgreSQL tests that the Scala module lacks: column
population, catalog objects, RLS enforcement as a non-superuser role with
`set_config('app.tenant_id', ...)`, the transactional handler and
`TenantStateLister`. The test kit has its own suite pinning every helper.
