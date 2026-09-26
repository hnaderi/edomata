# Porting map: Scala modules and suites → Rust

This table is the contract of the Rust port (see `docs/plans/rust-port.md` at the repository root).
Every Scala module and every Scala test suite is mapped to its Rust
counterpart. Rows marked *planned* name the milestone that ports them; the
port is complete when no row is left *planned*.

## Modules

| # | Scala module | Rust crate | Status |
|---|--------------|------------|--------|
| 1 | `core` | `edomata-core` | ported (milestone 1) |
| 2 | `backend` | `edomata-backend` | ported (milestone 2) |
| 3 | `postgres` | `edomata-postgres` | ported (milestone 3) |
| 4 | `skunk` | `edomata-sqlx` | ported (milestone 5) |
| 5 | `doobie` | `edomata-sqlx` | ported (milestone 5) |
| 6 | `skunk-circe` | `edomata-serde` | ported (milestone 4) |
| 7 | `skunk-jsoniter` | `edomata-serde` | ported (milestone 4) |
| 8 | `skunk-upickle` | `edomata-serde` | ported (milestone 4; `msgpack` payloads are a documented limitation) |
| 9 | `doobie-circe` | `edomata-serde` | ported (milestone 4) |
| 10 | `doobie-jsoniter` | `edomata-serde` | ported (milestone 4) |
| 11 | `doobie-upickle` | `edomata-serde` | ported (milestone 4; `msgpack` payloads are a documented limitation) |
| 12 | `backend-tests` | `edomata-backend-tests` | ported (milestone 2, in-memory; milestone 5, PostgreSQL through `edomata-sqlx`) |
| 13 | `e2e` | `edomata-e2e` | ported (milestone 8, test-only) |
| 14 | `munit` | `edomata-testkit` | ported (milestone 6) |
| 15 | `saas` | `edomata-saas` | ported (milestone 6) |
| 16 | `saas-skunk` | `edomata-saas-sqlx` | ported (milestone 6) |
| 17 | `java-api` | `edomata-simple` | ported (milestone 7) |
| — | `examples/` | `rust/examples/` (`edomata-examples`, one binary per example) | ported (milestone 8); Kafka and RabbitMQ examples added (milestone 9) |
| — | *(new)* | `edomata-broker`, `edomata-kafka`, `edomata-rabbitmq` | added (milestone 9; no Scala equivalent) |
| — | `docs/`, `website/` | `rust/book/` (mdBook) with `rust/book/samples` (`edomata-book-samples`) | ported (milestone 10) |

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

## Type mapping (codecs)

| Scala | Rust |
|-------|------|
| `BackendCodec[T]` (`Json`, `JsonB`, `Binary`; skunk and doobie variants) | `edomata_backend::Codec<T>` + `PayloadFormat` (`Json`, `Jsonb`, `Bytea`) |
| `CirceCodec.json` / `.jsonb` | `SerdeCodec::<T>::json()` / `::jsonb()` (default) |
| `JsoniterCodec.json` / `.jsonb` / `.msgpack` | `SerdeCodec::<T>::json()` / `::jsonb()` / `::bytea()` (jsoniter's `msgpack` writes JSON bytes) |
| `UpickleCodec.json` / `.jsonb` | `SerdeCodec::<T>::json()` / `::jsonb()` with `#[serde(tag = "$type")]` and `compat::upickle_option` |
| `UpickleCodec.msgpack` | not portable: real MessagePack, unreadable with `serde_json` (documented limitation) |
| skunk `Codec[T]` / doobie `Meta[T]` wire encoding | `edomata_serde::pg::{JsonbPayload, JsonPayload, ByteaPayload, PgPayload}` (`sqlx` `Encode` / `Decode` / `Type`) |

## Type mapping (PostgreSQL driver)

| Scala (Skunk / Doobie) | Rust (`edomata-sqlx`) |
|------------------------|-----------------------|
| `SkunkDriver` / `DoobieDriver` (`apply(ns, pool)`, `from(namespace, pool)`, `from(naming, pool, skipSetup)`) | `SqlxDriver::for_namespace(ns, pool)`, `SqlxDriver::new(naming, pool)`, `SqlxDriver::new_with(naming, pool, skip_setup)` |
| `SkunkCQRSDriver` / `DoobieCQRSDriver` | `SqlxCqrsDriver` (same constructors) |
| `Resource[F, Session[F]]` / `Transactor[F]` | `sqlx::PgPool` |
| `BackendCodec[T]` (driver codec typeclass) | `SqlxCodec<T>` (`StorageDriver::Codec<T>`; `Default` = `SerdeCodec::jsonb()`) |
| `SkunkHandler[F][N]` / `DoobieHandler[N]` | `SqlxHandler<N>` (`Fn(&NonEmpty<N>, &mut PgConnection) -> BoxFuture<Result<(), BackendError>>`) |
| `SkunkRepository`, `SkunkCQRSRepository`, `SkunkJournalReader`, `SkunkOutboxReader`, `SkunkSnapshotPersistence` (and Doobie twins) | private `SqlxRepository`, `SqlxCqrsRepository`, `SqlxJournalReader`, `SqlxOutboxReader`, `SqlxSnapshotPersistence` behind the `edomata-backend` traits |
| `Queries.scala` (both drivers) | `queries.rs` (same SQL; setup DDL from `edomata_postgres::ddl`) |
| `SkunkMigrations.run` / `DoobieMigrations.run(naming, pool, migrations, batchSize)` | `SqlxMigrations::run(&naming, &pool, &migrations)` / `run_with_batch_size` |
| `SqlState.UniqueViolation` → `VersionConflict` | SQLSTATE `23505` → `BackendError::VersionConflict` |

## Type mapping (test kit)

| Scala (`munit` module) | Rust (`edomata-testkit`) |
|------------------------|--------------------------|
| `trait DomainSuite(msgId, address)` (MUnit base trait with an `extension [F[_], C, S, E, R, N]` on `Edomaton[F, RequestContext[C, S], R, E, N, Unit]`) | `EdomatonAssertions` extension trait on `Edomaton<RequestContext<C, S>, R, E, N, T>` (works with any test runner) |
| `DomainSuite` constructor defaults (`msgId = "1"`, `address = "sut"`; commands timestamped `Instant.MIN`) | `TestCommand` (same defaults, `DateTime::<Utc>::MIN_UTC`; `TestCommand::new(id, address)`, `message(payload)`) |
| `app.runWith(command, state)` | `app.run_with(&model, command, state)`, `app.run_with_command(&model, &TestCommand, command, state)` |
| `expect`, `expectAll`, `expectRejection`, `expectRejectionWith(command, state)(err1, errs*)`, `expectRejectionWith(command, state)(expectedErrors, expectedNotifications)`, `expectRejectionNotify`, `expectThat` | `expect`, `expect_all`, `expect_rejection`, `expect_rejection_with`, `expect_rejection_and_notify`, `expect_rejection_notify`, `expect_that` |
| *(none)* | `StomatonAssertions` (`run_with`, `expect`, `expect_rejection_with`) for CQRS programs |

## Type mapping (SaaS)

| Scala (`saas`, `saas-skunk`) | Rust (`edomata-saas`, `edomata-saas-sqlx`) |
|------------------------------|--------------------------------------------|
| `TenantId`, `UserId` (opaque strings) | `TenantId`, `UserId` newtypes (`new`, `value`, `into_string`, `From<&str>` / `From<String>`) |
| `CrudAction` | `CrudAction` (+ `CrudAction::ALL`) |
| `CrudState[+A]` (`NonExistent`, `Active`, `Deleted`) | `CrudState<A>` (same variants; `active`, `deleted`, `tenant_id`, `owner_id`, `data`, `is_active`, `map`) |
| `SaaSCommand[Auth, +C]` | `SaaSCommand<Auth, C>` |
| `AuthPolicy[Auth]` (typeclass, `given`) | `AuthPolicy<Auth>` trait, passed as a value (ADR 0009) |
| `CallerIdentity` + its default `given AuthPolicy` | `CallerIdentity` + `PermissivePolicy` |
| `RoleBasedPolicy(rolesFor)` | `RoleBasedPolicy::new(roles_for)`, `RoleBasedPolicy::none()` |
| `SaaSGuard.checkTenant`, `checkAuthorization` | `SaaSGuard::check_tenant`, `check_authorization`, `check` |
| `SaaSDomainDSL[Auth, C, A, E, R, N](mkRejection)` (`App[F, T]`) | `SaaSDomainDsl<Auth, C, A, E, R, N>::new(policy, mk_rejection)` (`SaaSEsApp<..., T>`) |
| `SaaSCQRSDomainDSL[Auth, C, A, R, N](mkRejection)` | `SaaSCqrsDsl<Auth, C, A, R, N>::new(policy, mk_rejection)` (`SaaSCqrsApp<..., T>`) |
| `guardedRouter`, `unsafeUnguardedRouter`, `guarded`, `unsafeUnguarded`, `auth`, `command`, `entityState`, `set`, `modifyS`, `decideS`, ... | `guarded_router`, `unsafe_unguarded_router`, `guarded`, `unsafe_unguarded`, `auth`, `command`, `entity_state`, `set`, `modify_s`, `decide_s`, ... (+ `guard(action)` and `policy()`) |
| `SaaSEventSourcedService`, `SaaSCQRSService` (`SaaS`, `domain`) | `SaaSEventSourcedService`, `SaaSCqrsService` (`saas()`, `domain()`) |
| `TenantAwareReader[F, Auth, A]`, `TenantScopedQuery[F, Auth, A, Q]`, `UnsafeCrossTenantQuery[F, A, Q]` | same names (`async_trait`); `TenantScopedQuery.apply` → `ScopedQueryFn::new(policy, run)`, `UnsafeCrossTenantQuery.apply` → `CrossTenantQueryFn::new(run)` |
| `TenantExtractor[S]` (typeclass) | `TenantExtractor` trait implemented by `CrudState<A>` (`tenant_and_owner`) |
| `SaaSPGSchema.cqrs(naming, stateType, notificationType, rls)`, `SaaSPGSchema.RLSConfig` | `SaaSPGSchema::cqrs(&naming)` / `cqrs_with(&naming, state_type, notification_type, rls)`, `RlsConfig`; statements in `edomata_saas::ddl` |
| `edomata.saas` package re-exports (`CommandMessage`, `MessageMetadata`, `Decision`, `Backend`) | `edomata_saas` re-exports `CommandMessage`, `MessageMetadata`, `Decision`, `NonEmpty`, `PGNaming`, `PGNamespace` (no `Backend`: the SaaS crate does not depend on `edomata-backend`) |
| `SaaSSkunkCQRSDriver` (`apply`, `from`, `from(naming, pool, skipSetup)`) | `SaaSSqlxCqrsDriver::for_namespace`, `new`, `new_with(naming, pool, skip_setup)` |
| `BackendCodec[S]` + `TenantExtractor[S]` (driver requirements) | `SaaSCodec<T>` (`state`, `with_extractor`, `notification`, `jsonb_state`, `jsonb_notification`) |
| `SaaSQueries` (`listByTenant`, tenant-aware `put` / outbox insert) | private `SaaSStateQueries` / `SaaSOutboxQueries`; `TenantStateLister::list_by_tenant` |
| `SaaSSkunkCQRSRepository`, `SaaSSkunkOutboxReader` | private `SaaSRepository`; `edomata_sqlx::shared::SqlxOutboxReader` reused |

## Broker distribution (new, no Scala equivalent)

| Concept | Rust |
|---------|------|
| broker-neutral message | `edomata_broker::BrokerMessage` (`outbox_id` / `journal_id`, `headers()`), `MessageKind`, `headers::*` |
| publisher | `edomata_broker::Publisher` (`publish(&NonEmpty<BrokerMessage>)`), `PublishError` (`Transient` / `Permanent`), `RecordingPublisher` for tests |
| outbox relay | `edomata_broker::OutboxRelay` (`relay_once`, `run`, `run_as_leader`, `wake_on`, `metrics`) |
| journal relay | `edomata_broker::JournalRelay`, `CheckpointStore`, `InMemoryCheckpointStore`, `postgres::PgCheckpointStore`, `PGSchema::relay_checkpoints` (opt-in DDL) |
| payload encoding | `MessageEncoder::serde` / `from_codec` |
| configuration and counters | `RelayConfig`, `RetryPolicy`, `RelayMetrics` / `MetricsSnapshot`, `RelayError`, `CancellationToken` |
| leader election, cross-process wake-ups | `postgres::LeaderLock` / `LeaderGuard`, `postgres::listen`; `SqlxDriver::with_outbox_notify_channel` / `with_journal_notify_channel`, `SqlxCqrsDriver::with_outbox_notify_channel` |
| Kafka | `edomata_kafka::KafkaPublisher` (`builder`, `with_topic`, `with_fixed_topic`, `with_config`), `default_topic` |
| RabbitMQ | `edomata_rabbitmq::RabbitMqPublisher` (`connect`, `with_exchange`, `with_routing_key`, `with_fixed_exchange`, `declare_exchange`) |

Tests: `crates/edomata-broker/tests/{outbox_relay,journal_relay,postgres}.rs`,
`crates/edomata-kafka/tests/kafka.rs` and `crates/edomata-rabbitmq/tests/rabbitmq.rs`
(testcontainers; Docker required).

## Type mapping (simple facade)

| Scala (`java-api`) | Rust (`edomata-simple`) |
|--------------------|-------------------------|
| `JDomainModel` (`initial`, `transition`, `create`, `toModelTC`) | `SimpleDomainModel` trait (`initial`, `transition` → `Result<S, Vec<R>>`, `into_model`), `ClosureModel::new` (`create`), `ModelAdapter` |
| `JDecision` (`Accepted`, `Rejected`, `Indecisive`; `accept`, `acceptReturn`, `reject`, `pure`, `unit`, `map`, `flatMap`, `toEither`) | `SimpleDecision` enum (same variants; `accept`, `accept_return`, `reject`, `pure`, `unit`, `map`, `and_then` / `flat_map`, `to_result`, `events`, `reasons`) |
| `Converters.decisionToJava` / `decisionToScala` | `SimpleDecision::from(Decision)` / `SimpleDecision::into_decision` (`EmptyRejection` error) |
| `Converters.toJavaList` / `toChain` / `toNonEmptyChain` | `NonEmpty::into_vec`, `Vec`, `NonEmpty::from_vec` |
| `JAppResult` (`decide`, `decideAndPublish`, `accept`, `reject`, `publish`) | `AppResult` (same constructors + `and_publish`) |
| `JCommandHandler.create`, `JRequestContext` (`command`, `commandMessage`, `state`, `address`, `messageId`) | `CommandHandler::new` / `new_async`, `Context` (`command`, `message`, `state`, `address()`, `message_id()`, `time()`) |
| `JCodec` (`encode`, `decode`, `of`, `toBackendCodec`) | `SimpleCodec` trait (`encode`, `decode` → `Result<T, String>`, `into_codec`), `ClosureCodec::new` (`of`), `CodecAdapter`, `serde_codec` for serde types |
| `JCommandMessage.of(id, time, address, payload)` | `CommandMessage::new(id, time, address, payload)` (re-exported) |
| `JEventMessage`, `JOutboxItem` | `EventMessage`, `OutboxItem` (re-exported) |
| `JEither` (`left`, `right`, `isLeft`, `isRight`, `getLeft`, `getRight`, `fold`, `map`) | `Result<R, L>` (`Err`, `Ok`, `is_err`, `is_ok`, `unwrap_err`, `unwrap`, `map_or_else`, `map`) |
| `JBackendBuilder.forDoobie(model)` (`namespace`, `schemaNamespace`, `dataSource`, `eventCodec`, `notificationCodec`, `maxRetry`, `inMemSnapshotSize`, `skipSetup`, `build(runtime)`) | `SimpleBackend::builder(model)` (`namespace`, `schema_namespace`, `naming`, `pool` / `database_url`, `event_codec` / `simple_event_codec`, `notification_codec` / `simple_notification_codec`, `serde_codecs`, `max_retry`, `in_mem_snapshot_size`, `skip_setup`, `build().await`, `build_blocking(runtime)`) |
| `JBackend` (`handle`, `journal`, `outbox`, `close`) | `SimpleBackend` (`handle`, `compile`, `journal`, `outbox`, `close`, `inner`); `BlockingBackend` for blocking calls |
| `JJournalReader` (`readStream`, `readStreamAfter`, `readAll`, `readAllAfter`) | `SimpleJournal` (`read_stream`, `read_stream_after`, `read_all`, `read_all_after`, returning `Vec`) |
| `JOutboxReader.read` | `SimpleOutbox::read` (+ `mark_as_sent`, `mark_all_as_sent`); blocking twins on `BlockingBackend` |
| `EdomataRuntime` (`create`, `global`, `fromExisting`, `close`) | `SimpleRuntime` (`create` owns a Tokio runtime since there is no global one, `from_handle`, `block_on`, `close`) |
| `JPGSchema` (`eventsourcing`, `cqrs`, `eventsourcingWithSchema`, `cqrsWithSchema`) | `SimplePGSchema` (`eventsourcing` / `eventsourcing_with`, `cqrs` / `cqrs_with`, `eventsourcing_with_schema`, `cqrs_with_schema`, returning `Result<Vec<String>, SimpleError>`) |
| `IllegalArgumentException` / `IllegalStateException` | `SimpleError::InvalidNamespace` / `MissingConfig`; `SimpleError::Connection`, `SimpleError::Backend` |

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
against the in-memory driver and `crates/edomata-sqlx/tests/shared_suites.rs` runs them against PostgreSQL.

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

### Codec modules (`skunk-circe`, `skunk-jsoniter`, `skunk-upickle`, `doobie-circe`, `doobie-jsoniter`, `doobie-upickle`)

These modules have no Scala test suites. The Rust crate adds:

| Rust test | Purpose |
|-----------|---------|
| `crates/edomata-serde/tests/round_trip.rs` | round trips for every format, wire-format bytes of the sqlx wrappers |
| `crates/edomata-serde/tests/golden_payloads.rs` + `rust/tests/golden/payloads/` | payloads written by Circe, jsoniter (JSON and `msgpack`) and uPickle (JSON) are readable and re-encoded byte-for-byte; uPickle `msgpack` is asserted unreadable |
| `crates/edomata-serde/tests/sql.rs` | stored `jsonb` / `json` / `bytea` payloads queried with `->>` and `@>` on the docker-compose PostgreSQL |
| `examples/src/test/scala/GoldenPayloads.scala` (new generator, test scope) | produces the golden payload files |

### `modules/skunk` and `modules/doobie` tests

Both Scala suites wire the shared `backend-tests` suites to their driver; the
single `edomata-sqlx` driver runs them once, under
`crates/edomata-sqlx/tests/`.

| Scala suite (classes) | Rust test | Notes |
|-----------------------|-----------|-------|
| `SkunkCompatibilitySuite` / `DoobieCompatibilitySuite` (`SkunkCompatibilitySuite` and `DoobieJsonCompatibilitySuite` for json, `*JsonbCompatibilitySuite`, `*BinaryCompatibilitySuite`) | `shared_suites.rs` (`compatibility_json`, `compatibility_jsonb`, `compatibility_binary`) | same `compatibility_*` schemas from `testdata.sql`; `IntCodec` in `common/mod.rs` reproduces the Scala test codecs (hex bytes for `bytea`) |
| `*PersistenceSuite`, `*PersistenceKeywordNamespaceSuite`, `*PrefixedPersistenceSuite` | `shared_suites.rs` (`persistence`, `persistence_keyword_namespace`, `persistence_prefixed`) | checks of one namespace are serialised with a lock (MUnit runs a suite sequentially) |
| `*SnapshotPersistenceSuite` | `shared_suites.rs` (`snapshot_*`) | |
| `*CQRSSuite`, `*PrefixedCQRSSuite` | `shared_suites.rs` (`cqrs_schema`, `cqrs_prefixed`) | |
| *(none in Scala)* | `migrations.rs` | pins `SkunkMigrations` / `DoobieMigrations` behaviour (apply, skip, rewrite, truncate snapshots, atomic failure) |
| *(none in Scala)* | `driver.rs` | `skip_setup`, Flyway workflow with `PGSchema`, prefixed catalog names, namespace validation, transactional CQRS handler, duplicate-command race |

### `modules/saas/src/test`

All under `crates/edomata-saas/tests/`.

| Scala suite | Rust test | Notes |
|-------------|-----------|-------|
| `SaaSGuardSuite.scala` | `guard.rs` | includes the custom `AuthPolicy` (API key) checks |
| `SaaSDomainDSLSuite.scala` | `domain_dsl.rs` | |
| `SaaSCQRSDSLSuite.scala` | `cqrs_dsl.rs` | |
| `SaaSServiceSuite.scala` | `service.rs` | services, `CrudState`, `types.rs`, re-exports; the covariance test has no Rust equivalent (invariant generics), the `serde` shape is pinned instead |
| `SaaSPGSchemaSuite.scala` (`TenantExtractorSuite`, `SaaSPGSchemaSuite`) | `schema.rs` | |
| `GoldenSaaSDDL.scala` (new generator, test scope) | `schema.rs` (`saas_ddl_matches_scala_golden_files`) + `rust/tests/golden/saas_cqrs_*.sql` | 24 files: schema/prefixed/quoted namespace × `jsonb`/`json`/`bytea`/mixed × with/without RLS |
| `TenantAwareReaderSuite.scala` | `reader.rs` | |
| `ProductCatalogSuite.scala` | `product_catalog.rs` | |

### `modules/saas-skunk`

The Scala module has no test suite. `crates/edomata-saas-sqlx/tests/postgres.rs`
adds PostgreSQL tests for: `tenant_id` / `owner_id` population on save,
empty tenant on `notify`, catalog objects created by setup, `skip_setup` with
RLS DDL from `SaaSPGSchema` enforced as a non-superuser role, the
transactional handler, namespace validation, `TenantStateLister` and
`SaaSCodec`.

### `modules/java-api/src/test`

All under `crates/edomata-simple/tests/`.

| Scala suite | Rust test | Notes |
|-------------|-----------|-------|
| `ConvertersSuite.scala` | `converters.rs` | list conversions become `NonEmpty::from_vec` / `into_vec` |
| `JAppResultSuite.scala` | `app_result.rs` | |
| `JCodecSuite.scala` | `codec.rs` | |
| `JCommandMessageSuite.scala` | `command_message.rs` | on the re-exported `CommandMessage` |
| `JDecisionSuite.scala` | `decision.rs` | |
| `JDomainModelSuite.scala` | `domain_model.rs` | |
| `JEitherSuite.scala` | `either.rs` | pins the `JEither` → `Result` mapping |
| `JPGSchemaSuite.scala` | `pg_schema.rs` | |
| `JavaApiIntegrationSuite.scala` (runs `src/test/java/.../JavaApiTest.java`) | `integration.rs` | the 17 Java checks |
| *(none in Scala)* | `backend.rs` | `SimpleBackend` on PostgreSQL: builder validation, commands, journal, outbox, `skip_setup`, blocking backend |

### `modules/e2e`

| Scala | Rust | Notes |
|-------|------|-------|
| `main/accounts/Domain.scala`, `main/accounts/Service.scala` | `crates/edomata-e2e/src/lib.rs` | `BigDecimal` → `rust_decimal::Decimal`; JSON shapes match Circe (ADR 0011) |
| `main/e2e.scala` + `SkunkE2ETestSuites.scala` + `DoobieE2ETestSuites.scala` | `crates/edomata-e2e/tests/e2e.rs` | one sqlx variant replaces the Skunk and Doobie variants |
| `CrossLanguage.scala` (new, test scope: the Scala side of the cross-language test) | `crates/edomata-e2e/tests/cross_language.rs` | Scala writes, Rust reads and appends, Scala verifies; needs `sbt` (see `rust/README.md`) |

### `modules/munit`

| Scala | Rust |
|-------|------|
| `DomainSuite.scala` (no test suite of its own) | `edomata-testkit`; `crates/edomata-testkit/tests/assertions.rs` pins every helper |

### `examples/`

All under `rust/examples/src/bin/`; run with `cargo run -p edomata-examples --bin <name>`.

| Scala example | Rust example | Notes |
|---------------|--------------|-------|
| `Example1.scala` | `counter.rs` | the counter's transition really counts (ADR 0011) |
| `StomatonExample.scala` | `stomaton.rs` | runnable (the Scala one has `???` placeholders) |
| `MigrationExample.scala` | `migration.rs` | also seeds a V1 journal and reads it back as V3 |
| `SaaSExample.scala` | `saas_todo.rs` | tenant-scoped read queries run real SQL |
| `ProductCatalogExample.scala` | `product_catalog.rs` | the unimplemented `ProductQueries` read queries are replaced by `TenantStateLister::list_by_tenant` |
| *(new)* | `kafka_relay.rs`, `rabbitmq_relay.rs` (features `kafka` / `rabbitmq`) | outbox relays to Kafka and RabbitMQ (milestone 9) |

## Documentation

| Scala page (`docs/`) | Book chapter (`rust/book/src/`) | Compiled samples |
|----------------------|---------------------------------|------------------|
| `introduction.md` | `introduction.md` | |
| `about/design-goals.md`, `about/features.md` | `about/design-goals.md`, `about/features.md` | |
| `tutorials/getting-started.md` | `tutorials/getting-started.md` | |
| `tutorials/eventsourcing.md` | `tutorials/eventsourcing.md` | `book/samples/src/eventsourcing.rs`, `book/samples/src/testing.rs` |
| `tutorials/cqrs.md` | `tutorials/cqrs.md` | `book/samples/src/cqrs.rs` |
| `tutorials/backends.md` | `tutorials/backends.md` | `book/samples/src/running.rs` |
| `tutorials/processes.md` | `tutorials/processes.md` | `book/samples/src/processes.rs` |
| `tutorials/saas.md` | `tutorials/saas.md` | `book/samples/src/saas.rs` |
| `tutorials/migrations.md` | `tutorials/migrations.md` | `book/samples/src/migrations.rs` |
| `principles/index.md`, `principles/definitions.md` | `principles/index.md`, `principles/definitions.md` | |
| `backends/skunk.md`, `backends/doobie.md` | `backends/postgres.md` (one sqlx page) | `book/samples/src/running.rs` |
| `backends/java-api.md` | `backends/simple-api.md` | `book/samples/src/simple.rs` |
| *(none)* | `backends/brokers.md` | `book/samples/src/processes.rs`, `examples/src/bin/{kafka,rabbitmq}_relay.rs` |
| `other/modules.md`, `other/faq.md` | `other/modules.md`, `other/faq.md` | |
| *(none)* | `other/migration-guide.md`, `other/porting.md` (this file), `design/*` (the ADRs) | |
