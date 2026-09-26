# Crates

| Crate | Scala module(s) | Purpose | Platforms |
|-------|-----------------|---------|-----------|
| `edomata-core` | `core` | `Decision`, `Edomaton`, `Stomaton`, DSLs, `DomainModel`, `CqrsModel` | native, `wasm32-unknown-unknown` |
| `edomata-backend` | `backend` | backend abstractions, in-memory driver, command handling, caches, snapshots, outbox | native (Tokio) |
| `edomata-backend-tests` | `backend-tests` | shared storage test suites (test-only) | native |
| `edomata-postgres` | `postgres` | `PGNaming`, `PGNamespace`, `PGSchema` (golden-tested DDL), `EventMigration` | native |
| `edomata-serde` | `skunk-circe`, `skunk-jsoniter`, `skunk-upickle`, `doobie-circe`, `doobie-jsoniter`, `doobie-upickle` | `SerdeCodec` (`jsonb` / `json` / `bytea`), sqlx wire types | native |
| `edomata-sqlx` | `skunk`, `doobie` | PostgreSQL event-sourcing and CQRS drivers, readers, snapshots, migrations | native |
| `edomata-testkit` | `munit` | `TestCommand`, `EdomatonAssertions`, `StomatonAssertions` | native |
| `edomata-saas` | `saas` | tenancy, `AuthPolicy`, guards, guarded DSLs, `SaaSPGSchema` (RLS) | native |
| `edomata-saas-sqlx` | `saas-skunk` | tenant-aware CQRS driver, `SaaSCodec`, `TenantStateLister` | native |
| `edomata-simple` | `java-api` | closure-based facade over the PostgreSQL backend | native |
| `edomata-e2e` | `e2e` | end-to-end suite and the Scala/Rust cross-language test (test-only) | native |
| `edomata-broker` | *(new)* | `Publisher`, `OutboxRelay`, `JournalRelay`, leader election, `LISTEN/NOTIFY` | native |
| `edomata-kafka` | *(new)* | Kafka publisher (rdkafka) | native |
| `edomata-rabbitmq` | *(new)* | RabbitMQ publisher (lapin) | native |
| `edomata-examples` | `examples/` | one binary per Scala example, plus the broker relays | native |
| `edomata-book-samples` | `docs/` | the compiled samples of this book | native |

Every crate is `#![forbid(unsafe_code)]` and supports Rust 1.88 or later (edition 2024); every library crate documents every public item (the samples crate is exempt). Only `edomata-core` is built for `wasm32-unknown-unknown` in CI. None of them depends on a JVM, Scala or Java artifact.

The complete mapping of Scala modules and test suites to Rust is in the [porting map](porting.md).
