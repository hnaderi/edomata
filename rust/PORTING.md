# Porting map: Scala modules and suites → Rust

This table is the contract of the Rust port (see [`../docs/plans/rust-port.md`](../docs/plans/rust-port.md)).
Every Scala module and every Scala test suite is mapped to its Rust
counterpart. Rows marked *planned* name the milestone that ports them; the
port is complete when no row is left *planned*.

## Modules

| # | Scala module | Rust crate | Status |
|---|--------------|------------|--------|
| 1 | `core` | `edomata-core` | ported (milestone 1) |
| 2 | `backend` | `edomata-backend` | planned (milestone 2) |
| 3 | `postgres` | `edomata-postgres` | planned (milestone 3) |
| 4 | `skunk` | `edomata-sqlx` | planned (milestone 5) |
| 5 | `doobie` | `edomata-sqlx` | planned (milestone 5) |
| 6 | `skunk-circe` | `edomata-serde` | planned (milestone 4) |
| 7 | `skunk-jsoniter` | `edomata-serde` | planned (milestone 4) |
| 8 | `skunk-upickle` | `edomata-serde` | planned (milestone 4) |
| 9 | `doobie-circe` | `edomata-serde` | planned (milestone 4) |
| 10 | `doobie-jsoniter` | `edomata-serde` | planned (milestone 4) |
| 11 | `doobie-upickle` | `edomata-serde` | planned (milestone 4) |
| 12 | `backend-tests` | `edomata-backend-tests` | planned (milestone 2, run against `sqlx` in milestone 5) |
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

### `modules/backend/src/test` — planned (milestone 2)

| Scala suite | Rust test |
|-------------|-----------|
| `LRUCacheSuite.scala` | planned |
| `InMemoryCommandStoreSuite.scala` | planned |
| `OutboxConsumerSuite.scala` | planned |
| `FakeOutboxReader.scala`, `Doubles.scala` | planned (test doubles) |
| `eventsourcing/CachedRepositorySuite.scala` | planned |
| `eventsourcing/CommandHandlerSuite.scala` | planned |
| `eventsourcing/NotificationsSuite.scala` | planned |
| `eventsourcing/RepositoryReaderSuite.scala` | planned |
| `eventsourcing/PersistedSnapshotStoreSuite.scala` | planned |
| `eventsourcing/InMemorySnapshotSuite.scala` | planned |
| `eventsourcing/FakeRepository.scala`, `eventsourcing/Doubles.scala` | planned (test doubles) |
| `cqrs/CachedRepositorySuite.scala` | planned |
| `cqrs/CommandHandlerSuite.scala` | planned |
| `cqrs/NotificationsSuite.scala` | planned |
| `cqrs/FakeRepository.scala` | planned (test double) |

### `modules/backend-tests` — planned (milestone 2, PostgreSQL in milestone 5)

| Scala suite | Rust test |
|-------------|-----------|
| `shared/BackendCompatibilitySuite.scala` | planned |
| `shared/CqrsSuite.scala` | planned |
| `shared/PersistenceSuite.scala` | planned |
| `shared/SnapshotPersistenceSuite.scala` | planned |
| `shared/TestDomain.scala`, `shared/Utils.scala` | planned (shared fixtures) |
| `{jvm,js,native}/StorageSuite.scala` | planned (single Rust variant) |

### `modules/postgres/src/test` — planned (milestone 3)

| Scala suite | Rust test |
|-------------|-----------|
| `PGNamespaceSuite.scala` | planned (+ golden DDL tests under `rust/tests/golden/`) |

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
