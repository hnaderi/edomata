# Porting map: Scala modules and suites → Rust

This table is the contract of the Rust port (see [`../docs/plans/rust-port.md`](../docs/plans/rust-port.md)).
Every Scala module and every Scala test suite is mapped to its Rust
counterpart. Rows marked *planned* name the milestone that ports them; the
port is complete when no row is left *planned*.

## Modules

| # | Scala module | Rust crate | Status |
|---|--------------|------------|--------|
| 1 | `core` | `edomata-core` | ported (milestone 1) |
| 2 | `backend` | `edomata-backend` | ported (milestone 2) |
| 3 | `postgres` | `edomata-postgres` | ported (milestone 3) |
| 4 | `skunk` | `edomata-sqlx` | planned (milestone 5) |
| 5 | `doobie` | `edomata-sqlx` | planned (milestone 5) |
| 6 | `skunk-circe` | `edomata-serde` | planned (milestone 4) |
| 7 | `skunk-jsoniter` | `edomata-serde` | planned (milestone 4) |
| 8 | `skunk-upickle` | `edomata-serde` | planned (milestone 4) |
| 9 | `doobie-circe` | `edomata-serde` | planned (milestone 4) |
| 10 | `doobie-jsoniter` | `edomata-serde` | planned (milestone 4) |
| 11 | `doobie-upickle` | `edomata-serde` | planned (milestone 4) |
| 12 | `backend-tests` | `edomata-backend-tests` | ported (milestone 2, in-memory); run against `sqlx` in milestone 5 |
| 13 | `e2e` | `edomata-e2e` | planned (milestone 8) |
| 14 | `munit` | `edomata-testkit` | planned (milestone 6) |
| 15 | `saas` | `edomata-saas` | planned (milestone 6) |
| 16 | `saas-skunk` | `edomata-saas-sqlx` | planned (milestone 6) |
| 17 | `java-api` | `edomata-simple` | planned (milestone 7) |
| — | `examples/` | `rust/examples/` | planned (milestone 8) |
| — | *(new)* | `edomata-broker`, `edomata-kafka`, `edomata-rabbitmq` | planned (milestone 9) |

## Type mapping (core)

| Scala | Rust |
|-------|------|
| `NonEmptyChain[T]` | `NonEmpty<T>` (`nonempty![...]`) |
| `ValidatedNec[R, T]`, `EitherNec[R, T]` | `Result<T, NonEmpty<R>>` (`ResultNec<T, R>`) |
| `Decision[R, E, A]` | `Decision<R, E, A>` |
| `DecisionT[F, R, E, A]` | `DecisionT<R, E, A>` (a `Future`) |
| `ResponseT[RES, R, N, A]` | `ResponseT<Res, N>` with `Res: RaiseError` |
| `ResponseD[R, E, N, A]` | `ResponseD<R, E, N, A>` |
| `ResponseE[R, N, A]` | `ResponseE<R, N, A>` |
| `RaiseError[F, R]` | `RaiseError` trait (GAT `WithOutput<B>`) |
| `Action[F, R, E, N, A]` | `Action<R, E, N, A>` (a `Future`) |
| `Edomaton[F, Env, R, E, N, A]` | `Edomaton<Env, R, E, N, A>` |
| `Stomaton[F, Env, S, R, E, A]` | `Stomaton<Env, S, R, N, A>` |
| `RequestContext`, `CommandMessage`, `MessageMetadata` | same names |
| `DomainModel` + `ModelTC` | `DomainModel` trait |
| `CQRSModel` + `StateModelTC` | `CqrsModel` trait |
| `DomainDSL`, `Domain` | `DomainDsl` (`App<C, S, E, R, N, T>`) |
| `CQRSDomainDSL`, `CQRSDomain` | `CqrsDomainDsl` (`CqrsApp<C, S, R, N, T>`) |
| `DomainCompiler`, `EdomatonResult` | same names |
| `DomainService[F, C, R]` | `DomainService<'a, C, R>` |
| `edomata.syntax.all` | `edomata_core::syntax::*` |
| `Response` (deprecated alias) | not ported (deprecated in Scala) |

Method naming: Scala overloads become distinct names (`validate` /
`validate_one`, `assert_with` / `assert_one`, `acceptWhen` → `accept_when`,
`accept_some`, `accept_some_or`, `accept_ok`, `accept_ok_nec`); `flatMap` is
`and_then` (with `flat_map` as an alias); `>>` is `then`; `as` is `replace`;
`tailRecM` is `tail_rec` with `std::ops::ControlFlow`.

## Type mapping (backend)

| Scala | Rust |
|-------|------|
| `SeqNr`, `EventVersion`, `StreamId` | same names (`i64`, `i64`, `String`) |
| `EventMetadata`, `EventMessage[T]` | same names (`uuid::Uuid`, `chrono::DateTime<Utc>`) |
| `BackendError` (`VersionConflict`, `MaxRetryExceeded`, `PersistenceError`, `UnknownError`) | `BackendError` enum (`thiserror`) |
| `CommandState.Redundant` | `eventsourcing::CommandState::Redundant`, `cqrs::CommandState::Redundant` |
| `AggregateState.Valid` / `Conflicted` | `eventsourcing::ValidState`, `eventsourcing::AggregateState` |
| `cqrs.AggregateState` | `cqrs::AggregateState` |
| `Cache`, `LRUCache` | `Cache` trait, `LruCache` |
| `CommandStore`, `CommandStore.inMem` | `CommandStore` trait, `InMemoryCommandStore` |
| `Repository`, `RepositoryReader`, `CachedRepository` | same names in `eventsourcing` / `cqrs` |
| `JournalReader`, `OutboxReader`, `OutboxItem` | same names |
| `OutboxConsumer` | `OutboxConsumer` (batched, `run` / `consume_once`) |
| `SnapshotReader`, `SnapshotStore`, `SnapshotPersistence`, `SnapshotStore.inMem` / `persisted` | same traits, `InMemorySnapshotStore`, `PersistedSnapshotStore` (+ `PersistedSnapshotConfig`) |
| `Notifications`, `NotificationsConsumer`, `NotificationsPublisher` | same names (built on `Signal` / `tokio::sync::Notify`, ADR 0005) |
| `CommandHandler`, `CommandHandler.withRetry` | `CommandHandler::new` / `with_retry` + `RetryConfig` |
| `retry` | `retry`, `retry_with` |
| `Storage`, `StorageDriver[F, Codec[_]]` | `Storage`, `StorageDriver` with GAT `Codec<T>` (and `Handler<N>` for CQRS) |
| `Backend`, `BackendBuilder`, `PartialBackendBuilder` | same names; `Backend::builder(model, dsl).driver(d)...build(...)` |
| `DomainService[F, C, R]` | `DomainService<C, R>` (`Arc<dyn Fn(CommandMessage<C>) -> BoxFuture<CommandResult<R>>>`) |
| `CommandMessage[?]` (payload-erased) | `CommandRef<'_>` |
| `StateModelTC[S]` | `cqrs::StateModel<S>` |
| `SkunkHandler` (CQRS notification hook) | `StorageDriver::Handler<N>` (`InMemoryNotificationHandler<N>` in memory) |
| *(none)* | `inmemory::InMemoryDriver`, `InMemoryEventStore`, `InMemoryStateStore`, `InMemorySnapshotPersistence` |

## Type mapping (postgres)

| Scala | Rust |
|-------|------|
| `PGNamespace` (opaque type, inline macro `PGNamespace("x")`, `fromString`) | `PGNamespace` newtype, `TryFrom<&str>` / `FromStr` / `from_string` (runtime validation, same messages), `PGNamespaceError` |
| `PGNaming` (`Schema`, `Prefixed`, `schema`, `prefixed`, `table`, `constraint`, `index`, `needsSchemaSetup`) | `PGNaming` enum with the same methods (`needs_schema_setup`), plus `schema_str` / `prefixed_str` |
| `PGNamespace.prefixed("x")` | `PGNamespace::prefixed(self)` / `PGNaming::prefixed_str` |
| `PGSchema.eventsourcing(naming, eventType, notificationType, snapshotType)` | `PGSchema::eventsourcing(&naming)` (all `jsonb`) / `PGSchema::eventsourcing_with(&naming, ...)` |
| `PGSchema.cqrs(naming, stateType, notificationType)` | `PGSchema::cqrs(&naming)` / `PGSchema::cqrs_with(&naming, ...)` |
| private DDL helpers | `edomata_postgres::ddl::schema_statement` and `ddl::*_statements` (public, reused by drivers) |
| `EventMigration(version, description, run)`, `EventMigration[A, B](...)`, `andThen` | `EventMigration::new`, `EventMigration::typed`, `and_then`, `run` |
| `MigrationResult(applied, skipped)` | `MigrationResult { applied, skipped }` |

## Test suites

### `modules/core/src/test`

| Scala suite | Rust test | Notes |
|-------------|-----------|-------|
| `decision/DecisionSuite.scala` | `crates/edomata-core/tests/decision.rs` | MonadError, Traverse, Eq laws via `proptest`; accumulation and validation properties. `SerializableTests` is JVM-only (no equivalent). |
| `decision/DecisionSyntaxSuite.scala` | `crates/edomata-core/tests/decision_syntax.rs` | |
| `decision/Generators.scala`, `Helpers.scala` (at `modules/core/src/test/scala/`) | `crates/edomata-core/tests/common/mod.rs` | proptest strategies |
| `decisiont/DecisionTSuite.scala` | `crates/edomata-core/tests/decision_t.rs` | |
| `responset/ResponseTLaws.scala` | `crates/edomata-core/tests/response.rs` (`check_laws`) | generic over `Res: RaiseError`; the Cats `Traverse` law set is not ported (no `Traverse` in Rust, see ADR 0004) |
| `responset/ResponseDecisionSuite.scala` (`ResponseDecisionSuite`, `ResponseEitherNecSuite`) | `crates/edomata-core/tests/response.rs` | `response_decision_laws`, `response_result_nec_laws` |
| `action/ActionSuite.scala` | `crates/edomata-core/tests/action.rs` | |
| `edomaton/EdomatonSuite.scala` | `crates/edomata-core/tests/edomaton.rs` | laws checked on inputs `1, 2, 3` like `ExhaustiveCheck` |
| `stomaton/StomatonSuite.scala` | `crates/edomata-core/tests/stomaton.rs` | |
| `stomaton/StomatonConstructorSuite.scala` | `crates/edomata-core/tests/stomaton.rs` | |
| `ModelSyntaxSuite.scala` | `crates/edomata-core/tests/model_syntax.rs` | |
| *(none in Scala)* | `crates/edomata-core/tests/compiler.rs` | pins `DomainCompiler` / DSL behaviour |

### `modules/backend/src/test`

All under `crates/edomata-backend/tests/`.

| Scala suite | Rust test | Notes |
|-------------|-----------|-------|
| `LRUCacheSuite.scala` | `lru_cache.rs` | |
| `InMemoryCommandStoreSuite.scala` | `command_store.rs` | |
| `OutboxConsumerSuite.scala` | `outbox_consumer.rs` | "mark each chunk" uses the consumer's batch size instead of fs2 chunk boundaries |
| `FakeOutboxReader.scala`, `Doubles.scala` | `common/mod.rs` | test doubles |
| `eventsourcing/CachedRepositorySuite.scala` | `es_cached_repository.rs` | |
| `eventsourcing/CommandHandlerSuite.scala` | `es_command_handler.rs` | "raised errors" / "retry" use failing repositories (programs have no error channel, ADR 0005) |
| `eventsourcing/NotificationsSuite.scala` | `notifications.rs` | Tokio paused clock replaces `TestControl` |
| `eventsourcing/RepositoryReaderSuite.scala` | `es_repository_reader.rs` | |
| `eventsourcing/PersistedSnapshotStoreSuite.scala` | `es_snapshot_stores.rs` | Tokio paused clock replaces `TestControl` |
| `eventsourcing/InMemorySnapshotSuite.scala` | `es_snapshot_stores.rs` | |
| `eventsourcing/FakeRepository.scala`, `eventsourcing/Doubles.scala` | `common/mod.rs` | test doubles |
| `cqrs/CachedRepositorySuite.scala` | `cqrs_cached_repository.rs` | |
| `cqrs/CommandHandlerSuite.scala` | `cqrs_command_handler.rs` | same adaptation as the event-sourcing handler suite |
| `cqrs/NotificationsSuite.scala` | `notifications.rs` | |
| `cqrs/FakeRepository.scala` | `common/mod.rs` | test double |

### `modules/backend-tests`

The suites are a library crate, `crates/edomata-backend-tests`, so that the
same checks run against every storage. `tests/inmemory.rs` runs them
against the in-memory driver; milestone 5 adds the PostgreSQL runner.

| Scala suite | Rust |
|-------------|------|
| `shared/BackendCompatibilitySuite.scala` | `src/eventsourcing.rs` (`prepared_data` module) |
| `shared/CqrsSuite.scala` | `src/cqrs.rs` |
| `shared/PersistenceSuite.scala` | `src/eventsourcing.rs` |
| `shared/SnapshotPersistenceSuite.scala` | `src/eventsourcing.rs` (`snapshot_*` checks) |
| `shared/TestDomain.scala`, `shared/Utils.scala` | `src/lib.rs` (`TestDomain`, `TestCqrsModel`, `random_string`) |
| `{jvm,js,native}/StorageSuite.scala` | `tests/inmemory.rs` (one `#[tokio::test]` per check; a single Rust variant replaces the three platform variants) |

### `modules/postgres/src/test`

| Scala suite | Rust test | Notes |
|-------------|-----------|-------|
| `PGNamespaceSuite.scala` (`PGNamespaceSuite`) | `crates/edomata-postgres/tests/naming.rs` | the compile-time macro check becomes a runtime `TryFrom` check with the same message |
| `PGNamespaceSuite.scala` (`PGSchemaSuite`) | `crates/edomata-postgres/tests/naming.rs` | |
| `GoldenDDL.scala` (new generator, test scope) | `crates/edomata-postgres/tests/golden.rs` + `rust/tests/golden/*.sql` | byte-for-byte DDL comparison for `Schema`/`Prefixed` × `jsonb`/`json`/`bytea`/mixed (32 files) |
| *(none in Scala)* | `crates/edomata-postgres/tests/migration.rs` | pins `EventMigration` semantics |

### `modules/skunk` and `modules/doobie` tests — planned (milestone 5)

| Scala suite | Rust test |
|-------------|-----------|
| `skunk/SkunkCompatibilitySuite.scala` | planned |
| `doobie/DoobieCompatibilitySuite.scala` | planned |

### `modules/saas/src/test` — planned (milestone 6)

| Scala suite | Rust test |
|-------------|-----------|
| `SaaSGuardSuite.scala` | planned |
| `SaaSDomainDSLSuite.scala` | planned |
| `SaaSCQRSDSLSuite.scala` | planned |
| `SaaSServiceSuite.scala` | planned |
| `SaaSPGSchemaSuite.scala` | planned |
| `TenantAwareReaderSuite.scala` | planned |
| `ProductCatalogSuite.scala` | planned |

### `modules/java-api/src/test` — planned (milestone 7)

| Scala suite | Rust test |
|-------------|-----------|
| `ConvertersSuite.scala` | planned |
| `JAppResultSuite.scala` | planned |
| `JCodecSuite.scala` | planned |
| `JCommandMessageSuite.scala` | planned |
| `JDecisionSuite.scala` | planned |
| `JDomainModelSuite.scala` | planned |
| `JEitherSuite.scala` | planned |
| `JPGSchemaSuite.scala` | planned |
| `JavaApiIntegrationSuite.scala` | planned |

### `modules/e2e` — planned (milestone 8)

| Scala suite | Rust test |
|-------------|-----------|
| `SkunkE2ETestSuites.scala` | planned |
| `DoobieE2ETestSuites.scala` | planned |
| `main/accounts/*.scala`, `main/e2e.scala` | planned (e2e domain) |

### `modules/munit` — planned (milestone 6)

| Scala | Rust |
|-------|------|
| `DomainSuite.scala` | `edomata-testkit` (planned) |

### `examples/` — planned (milestone 8)

| Scala example | Rust example |
|---------------|--------------|
| `Example1.scala` | planned |
| `StomatonExample.scala` | planned |
| `MigrationExample.scala` | planned |
| `SaaSExample.scala` | planned |
| `ProductCatalogExample.scala` | planned |
| *(new)* | Kafka and RabbitMQ examples (planned, milestone 9) |
