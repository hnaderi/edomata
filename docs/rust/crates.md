---
sidebar_position: 2
title: "Crates"
---

# Crates

Each Scala module has a Rust counterpart. Where Scala offers one module per database library or
JSON library, Rust has a single crate: sqlx replaces both Skunk and Doobie, and serde replaces
Circe, jsoniter-scala and uPickle.

| Scala module(s) | Rust crate | Purpose |
|-----------------|------------|---------|
| `core` | `edomata-core` | `Decision`, `ResponseT` / `ResponseD`, `Action`, `Edomaton`, `Stomaton`, the DSLs, `DomainModel`, `CqrsModel` |
| `backend` | `edomata-backend` | backend abstractions, in-memory driver, command handling, caches, snapshots, outbox |
| `postgres` | `edomata-postgres` | `PGNaming`, `PGNamespace`, `PGSchema` (DDL identical to Scala's), `EventMigration` |
| `skunk`, `doobie` | `edomata-sqlx` | PostgreSQL event-sourcing and CQRS drivers, journal and outbox readers, snapshots, migrations, `skip_setup` |
| `skunk-circe`, `skunk-jsoniter`, `skunk-upickle`, `doobie-circe`, `doobie-jsoniter`, `doobie-upickle` | `edomata-serde` | `SerdeCodec` for `jsonb` (default), `json` and `bytea` payloads, sqlx wire types |
| `munit` | `edomata-testkit` | `TestCommand`, `EdomatonAssertions`, `StomatonAssertions` |
| `saas` | `edomata-saas` | tenancy types, `AuthPolicy`, `SaaSGuard`, guarded DSLs, `SaaSPGSchema` with row-level security |
| `saas-skunk` | `edomata-saas-sqlx` | tenant-aware CQRS driver, `SaaSCodec`, `TenantStateLister` |
| `java-api` | `edomata-simple` | closure-based facade: `SimpleDomainModel`, `SimpleDecision`, `CommandHandler`, `SimpleBackend::builder`, a blocking runtime |
| `backend-tests` | `edomata-backend-tests` | shared storage test suites (test-only, not published) |
| `e2e` | `edomata-e2e` | end-to-end suite and the Scala/Rust cross-language test (test-only, not published) |
| *(none)* | `edomata-broker` | `Publisher`, `OutboxRelay`, `JournalRelay`, leader election with advisory locks, `LISTEN/NOTIFY` wake-ups |
| *(none)* | `edomata-kafka` | Kafka publisher, built on rdkafka |
| *(none)* | `edomata-rabbitmq` | RabbitMQ publisher, built on lapin |

The workspace also holds `edomata-examples` (one binary per Scala example, plus the Kafka and
RabbitMQ relays) and `edomata-book-samples` (the compiled code samples of the Rust book).

## Platforms

The Scala modules cross-build for the JVM, Scala.js and Scala Native. The Rust crates build for
native targets; `edomata-core` also builds for `wasm32-unknown-unknown`, which CI checks. The
backends need Tokio, and the PostgreSQL backends need a PostgreSQL server; `edomata-backend` also
has an in-memory driver for tests and prototypes.

## Dependencies between crates

| Crate | Depends on |
|-------|------------|
| `edomata-core` | none |
| `edomata-postgres` | none (SQL strings only, no database client) |
| `edomata-testkit` | `edomata-core` |
| `edomata-backend` | `edomata-core` |
| `edomata-serde` | `edomata-backend` |
| `edomata-sqlx` | `edomata-core`, `edomata-backend`, `edomata-postgres`, `edomata-serde` |
| `edomata-saas` | `edomata-core`, `edomata-postgres` |
| `edomata-saas-sqlx` | `edomata-saas`, `edomata-sqlx` and their dependencies |
| `edomata-simple` | `edomata-sqlx` and its dependencies |
| `edomata-broker` | `edomata-core`, `edomata-backend`, `edomata-postgres` |
| `edomata-kafka`, `edomata-rabbitmq` | `edomata-core`, `edomata-broker` |
| `edomata-backend-tests` (test-only) | `edomata-core`, `edomata-backend` |
| `edomata-e2e` (test-only) | `edomata-core`, `edomata-backend`, `edomata-serde`, `edomata-sqlx` |

Applications that do not depend on `edomata-kafka` or `edomata-rabbitmq` pull no broker client.

## See also

- [Porting map](https://beyond-scale-group.github.io/edomata/rust/book/other/porting.html): the
  suite-by-suite mapping and the Scala-to-Rust type mapping.
- [Crates chapter of the Rust book](https://beyond-scale-group.github.io/edomata/rust/book/other/modules.html).
- [Modules](../other/modules.md) of the Scala library.
