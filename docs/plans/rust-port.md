# Goal: Port Edomata to Rust

## Mission

Port the entire Edomata library (Scala 3, all 17 modules, ~11k lines of main code, ~7k lines
of tests, in `modules/`) to an idiomatic, production-grade Rust library, published as a Cargo
workspace under `rust/` in this repository. When you're done, a Rust developer can build
event-sourced (`Edomaton`) and CQRS (`Stomaton`) services with the same semantics and
guarantees as the Scala version. A Rust service must also be able to read and write the
same PostgreSQL tables as a Scala service, so both can share a database.

This is a translation of **semantics**, not syntax. Don't emulate Cats/HKT machinery. Map
each abstraction to its idiomatic Rust equivalent, and keep the behavior, the invariants
and the on-disk formats identical.

## Source of truth

Read these before writing any code, then keep checking them as you go:

- `modules/core`: `Decision`, `DecisionT`, `Response`/`ResponseT`/`ResponseD`/`ResponseE`,
  `Action`, `Edomaton`, `Stomaton`, `RequestContext`, `DomainModel`/`ModelTC`, `CQRSModel`,
  `DomainDSL`, `CQRSDomainDSL`, `DomainCompiler`, `RaiseError`
- `modules/backend`: `Backend` + `BackendBuilder`, `Storage`, `StorageDriver`, `Repository`,
  `RepositoryReader`, `CachedRepository`, `Cache`/`LRUCache`, `CommandHandler`,
  `CommandStore`, `CommandState`, `SnapshotReader`/`SnapshotStore`/`SnapshotPersistence`, `JournalReader`,
  `OutboxReader`/`OutboxItem`, `OutboxConsumer`, `Notifications`, `BackendError`, `EventMetadata`/`EventMessage`
- `modules/postgres`: `PGNaming` (Schema/Prefixed), `PGNamespace`, `PGSchema`, `EventMigration`
- `modules/skunk` and `modules/doobie`: `Queries.scala` (exact SQL), drivers, handlers,
  repositories, journal/outbox readers, snapshot persistence, migrations
- `modules/saas` and `modules/saas-skunk`: tenancy (`TenantId`, `UserId`, `CallerIdentity`),
  `SaaSGuard`, `RoleBasedPolicy`, `CrudState`, SaaS DSLs and services, RLS config, `SaaSPGSchema`
- `modules/munit`: `DomainSuite` test helpers
- `modules/backend-tests`, `modules/e2e`, `examples/`: behavioral specs to port as tests
- `docs/`: user-facing semantics (principles, tutorials, backends, migrations, saas)

## Scope: a full port, nothing left behind

This is a **complete** port: all 17 modules. Nothing is out of scope. Every JVM/Scala
connector and dependency is replaced by the standard, widely used Rust crate for the same
job. The Rust port must never require a JVM, JNI, or any Scala/Java artifact at build time
or at runtime. (The only exception is the cross-language compatibility *test*, which uses
the Scala build as an external oracle.)

## Connector and dependency replacement

Replace every Scala/Java library with its standard Rust equivalent:

| Scala / Java dependency | Used for | Rust replacement |
|---|---|---|
| Doobie + JDBC `Transactor` | `doobie` driver, `java-api` | `sqlx` (`postgres`, `runtime-tokio` features), with `PgPool` as the transactor |
| Circe, jsoniter-scala, uPickle | Payload codecs | `serde` + `serde_json`, the only serializer |
| Cats / Cats Effect `IO`, `F[_]` | Effects | `std::future::Future`, `async fn`, `tokio` as the default runtime |
| FS2 `Stream` | Journal/outbox streaming, notifications | `futures::Stream` + `async-stream` |
| `cats.effect.std.Queue` / `Semaphore` | Concurrency | `tokio::sync::{mpsc, Semaphore}` |
| `cats.effect.std.Random` / `SecureRandom` | Random data | `rand` / `getrandom` |
| `cats.effect.std.UUIDGen` / `java.util.UUID` | IDs | `uuid` (v4) |
| `java.time.Instant` / `OffsetDateTime` | Timestamps | `chrono` (`DateTime<Utc>`) or `time` (pick one, record it in an ADR) |
| `NonEmptyChain` / `NonEmptyList` | Non-empty collections | a small in-crate `NonEmpty<T>` type |
| Cats Effect `Queue.circularBuffer` | In-process `Notifications` (update signals) | `tokio::sync::broadcast` (same in-process semantics) |
| *(new)* | Cross-process wake-up of outbox/broker relays | `sqlx::postgres::PgListener` (`LISTEN/NOTIFY`), optional; the Scala drivers don't have this |
| MUnit + ScalaCheck | Tests | built-in `#[test]`, `#[tokio::test]`, `proptest`, `testcontainers` (or `docker-compose`) |
| Logging (if any) | Diagnostics | `tracing` |
| sbt-mdoc / Docusaurus site | Documentation | rustdoc + `mdBook` |

Use only stable, mainstream crates. Don't add a general-purpose FP crate to emulate Cats.

## Target workspace layout

| Scala module(s) | Rust crate | Notes |
|---|---|---|
| `core` | `edomata-core` | Pure, no async runtime dependency; must compile to `wasm32-unknown-unknown` (this replaces the Scala.js/Native cross-build) |
| `backend` | `edomata-backend` | Runtime-agnostic traits (`async fn` in traits), in-memory implementations |
| `postgres` | `edomata-postgres` | `PGNaming`, `PGNamespace`, `PGSchema`, `EventMigration`; DDL generation only, no I/O |
| `skunk` + `doobie` | `edomata-sqlx` | The single PostgreSQL driver (ES + CQRS). It must cover every capability of both Scala drivers |
| `*-circe`, `*-jsoniter`, `*-upickle` | `edomata-serde` | One codec crate built on `serde_json`. It serializes straight to JSON bytes (`serde_json::to_vec`) and stores them as **`jsonb` by default**, so payloads stay queryable in SQL (`->`, `@>`, GIN indexes). `json` and `bytea` are also supported, for tables that already use them |
| `munit` | `edomata-testkit` | `DomainSuite`-equivalent assertion helpers for Rust tests |
| `saas` | `edomata-saas` | Tenancy, guards, policies, CRUD state, SaaS DSLs and services |
| `saas-skunk` | `edomata-saas-sqlx` | SaaS CQRS driver, outbox reader, RLS |
| `java-api` | `edomata-simple` | The Java facade's purpose, as idiomatic Rust: a simplified, closure-based API (`SimpleDomainModel`, `SimpleDecision`, `SimpleBackend::builder()`, a blocking and an async `CommandHandler`, a `PGSchema` helper) for users who don't want to touch the generic core types. Mirror every `J*` class's capabilities, and back it with `edomata-sqlx` the way `java-api` is backed by Doobie. |
| `backend-tests` | shared test suite crate `edomata-backend-tests` | Generic suites run against in-memory and `sqlx` storages |
| `e2e` | `edomata-e2e` (test-only) | |
| *(new, no Scala equivalent)* | `edomata-broker` | Optional broker publishing: `Publisher` trait + outbox relay (see "Optional message broker distribution") |
| *(new)* | `edomata-kafka` | Kafka publisher, via `rdkafka` |
| *(new)* | `edomata-rabbitmq` | RabbitMQ publisher, via `lapin` |
| `examples/` | `rust/examples/` | Port every example, plus one Kafka and one RabbitMQ example |

Codecs are defined once as a `Codec<T>` trait in `edomata-backend`, bridged to sqlx through
`sqlx::Encode`/`Decode`/`Type`. `edomata-serde` implements it for any `T: Serialize + DeserializeOwned`.

## Translation rules

- **`F[_]` / effect polymorphism** → `async fn`, `impl Future`, and `Send + 'static` bounds
  where the backend needs them. Core decision logic stays synchronous and pure.
- **`Decision[R, E, A]`** → an enum `Decision<R, E, A> { Accepted { events: NonEmpty<E>, result: A }, Rejected(NonEmpty<R>), InDecisive(A) }`,
  with `map`, `and_then`/`flat_map`, `validate`, accumulation, `to_result`, and every other
  constructor/combinator the Scala API exposes. The Monad/Applicative semantics must be
  identical, including event accumulation order and short-circuiting on rejection.
- **`NonEmptyChain`** → a small non-empty vector type (your own, or the `nonempty` crate).
- **`Edomaton` / `Stomaton` / `Action` / `DecisionT` / `ResponseT`** → structs wrapping
  boxed closures or traits, whichever gives the best ergonomics without losing
  composability (`map`, `flat_map`, `modify`/`set` for Stomaton, reading `RequestContext`,
  publishing notifications). Composition must work without macros; declarative macros are
  fine as sugar only.
- **Typeclass instances (Cats)** → inherent methods plus standard traits (`From`,
  `IntoIterator`, `FromIterator` for traverse/sequence). Don't add a general-purpose FP crate.
- **Opaque types** (`PGNamespace`, `TenantId`, `UserId`) → validated newtypes with
  `TryFrom<&str>`, with the same validation rules.
- **`fs2.Stream`** → `futures::Stream`.
- **Errors** → `thiserror` enums that mirror `BackendError` (`VersionConflict`,
  `MaxRetryExceeded`, `PersistenceError`, `UnknownError`). `Redundant` belongs to
  `CommandState`, as in Scala. No panics in library code.
- **Concurrency and caching** → the same optimistic concurrency (version conflict + retry
  policy), the same command idempotency (`CommandStore`), and the same LRU and snapshot
  semantics as `CachedRepository`, `LRUCache`, and `SnapshotStore`.

## Optional message broker distribution (Kafka, RabbitMQ)

This is a new capability that goes beyond Scala parity. Events and notifications can
optionally be distributed to a message broker. It must be **fully opt-in**: without the
`kafka` / `rabbitmq` features, no broker crate appears in the dependency tree.

**Design: the transactional outbox is the only source.** Never publish to a broker from
inside a command. The command writes events and outbox items to PostgreSQL in one
transaction (as today). A separate relay then publishes them. This keeps the existing
guarantees: nothing is lost, nothing is published unless it was committed.

- **`Publisher` trait** (`edomata-broker`): `async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError>`.
  A `BrokerMessage` carries the payload as JSON bytes (from `edomata-serde`), a message ID, the
  stream ID, the sequence number, the event/notification type, a timestamp, and headers
  (correlation/causation IDs from the `RequestContext`/`MessageMetadata`).
- **`OutboxRelay`**: the Rust port of `OutboxConsumer` wired to a `Publisher`. It reads the
  outbox, is woken up by the in-process `Notifications` signal, optionally by `LISTEN/NOTIFY` so it can
  run in a separate process (a new capability), plus a polling fallback interval, publishes a batch,
  waits for the broker's acknowledgment, and **only then** calls `mark_all_as_sent`.
- **Optional journal streaming**: a `JournalRelay` that tails the journal (raw events, not
  just notifications) from a checkpoint stored in a `<naming>_relay_checkpoints` table. Add
  that table to `PGSchema` behind an opt-in flag, so the default DDL stays byte-identical to
  Scala.
- **Delivery semantics**: at-least-once. Every message has a stable, deterministic ID
  (derived from stream ID + sequence number, or from the outbox item ID), so consumers can
  deduplicate. Document this explicitly.
- **Ordering**: per-stream ordering is guaranteed. On Kafka, the partition key is the stream
  ID. On RabbitMQ, a stream always goes to the same routing key/queue, and the relay publishes
  sequentially per stream.
- **Kafka** (`edomata-kafka`, `rdkafka`): idempotent producer (`enable.idempotence=true`,
  `acks=all`), topic chosen by a user-provided function (default: one topic per aggregate
  namespace), message ID and metadata in Kafka headers.
- **RabbitMQ** (`edomata-rabbitmq`, `lapin`): publisher confirms enabled, persistent
  messages (`delivery_mode = 2`), exchange + routing key chosen by user-provided functions,
  `message_id` set to the message ID.
- **Resilience**: retry with exponential backoff on broker failures. Don't mark items as sent
  on failure. Stop cleanly on shutdown (a cancellation token), and expose `tracing` spans and
  counters (published, retried, failed, lag).
- **Multiple relay instances**: only one relay per outbox publishes at a time. Use a
  PostgreSQL advisory lock (`pg_try_advisory_lock`) for leader election, so running several
  service replicas is safe.
- **Tests**: integration tests with `testcontainers` (Kafka and RabbitMQ), covering: no
  message is marked as sent before the broker acknowledges it, redelivery after a crash between
  publishing and marking, per-stream ordering, deduplication by message ID, and leader
  election with two relays.

## Wire compatibility (hard requirement)

1. `PGSchema::eventsourcing` / `PGSchema::cqrs` / `SaaSPGSchema` must generate DDL that is
   **byte-for-byte identical** to the Scala output for both `Schema` and `Prefixed` naming
   and for every payload type. Capture golden files by running the Scala code, commit them
   under `rust/tests/golden/`, and assert against them. As in Scala, every payload column
   (events, notifications, snapshots, states) defaults to `jsonb`.
2. Every SQL statement in the Rust `Queries` must match the skunk/doobie `Queries.scala`
   semantics: same tables, columns, constraints, index names, and naming via `naming.constraint()`
   / `naming.index()`.
3. Add a cross-language integration test: Scala writes events, commands, outbox items and
   snapshots; Rust reads them back and appends more; then Scala reads the result. Use the
   existing `docker-compose.yml` PostgreSQL.
4. Known limitation: payloads written by uPickle's `msgpack` codec (real MessagePack in
   `bytea`) can't be read, because `serde_json` is the only serializer. Payloads written by the
   Circe, jsoniter (including jsoniter's `msgpack`, which actually writes JSON bytes) and
   uPickle JSON codecs must be readable. Document this in the migration guide.
5. `skip_setup: bool` must work on every driver constructor, with the same meaning as
   `skipSetup`.

## Quality bar

- Port **every** test suite in `modules/*/src/test` and `modules/backend-tests`. Add a
  `PORTING.md` table that maps each Scala suite to its Rust counterpart. No suite is left
  unmapped without a written justification.
- Replace ScalaCheck property tests with `proptest`. Include law tests for `Decision`
  (functor/monad laws, accumulation) equivalent to the Cats law checks.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo test --workspace --all-features` all pass. `cargo doc` has no warnings.
- `#![forbid(unsafe_code)]` in every crate. Declare an MSRV and test it in CI.
- Every public item has rustdoc, and runnable doc examples on the core types.
- Port `docs/` to an mdBook under `rust/book/`, covering the same tutorials (getting started,
  event sourcing, CQRS, backends, migrations, processes, SaaS), with compiled code samples.
- Add a GitHub Actions workflow for the Rust workspace (fmt, clippy, test with a
  PostgreSQL service, wasm32 build of `edomata-core`, MSRV).

## Milestones (one PR each, in order)

1. **Workspace + core**: `Decision`, `DecisionT`, `Response*`, `Action`, `RequestContext`,
   `Edomaton`, `Stomaton`, models, DSLs, `DomainCompiler`, with all core tests and law tests.
2. **Backend abstractions**: traits, in-memory storage, repository, caching, command
   store, snapshots, outbox, notifications, command handler with retry, and the backend test
   suites against in-memory storage.
3. **Postgres DDL**: `PGNaming`, `PGNamespace`, `PGSchema`, `EventMigration`, and the golden
   DDL tests.
4. **Codecs**: `edomata-serde`, with `jsonb` as the default. It writes JSON bytes directly in
   the PostgreSQL `jsonb` binary format (version byte + UTF-8 JSON, as `sqlx::types::Json`
   does) without going through an intermediate `String` or `serde_json::Value`. Include
   round-trip tests, golden-payload tests against the Scala JSON output, and a SQL test that
   queries a stored payload with `->>` / `@>`.
5. **sqlx driver** (replacing both Skunk and Doobie): event-sourcing and CQRS drivers,
   journal/outbox readers, snapshot persistence, migrations, in-process `Notifications`, `skip_setup`,
   and the full persistence/CQRS/snapshot suites against Docker PostgreSQL.
6. **Testkit + SaaS**: `edomata-testkit`, `edomata-saas`, and `edomata-saas-sqlx`
   (guards, policies, CRUD state, RLS, tenant-aware readers).
7. **Simple facade**: `edomata-simple` (the `java-api` equivalent), with every `J*` class's
   capability and its tests ported.
8. **E2E, examples, cross-language compatibility test**.
9. **Broker distribution (optional)**: `edomata-broker`, `edomata-kafka`, `edomata-rabbitmq`,
   `OutboxRelay`, the opt-in `JournalRelay`, and the testcontainers integration tests.
10. **Documentation**: mdBook (including a "Simple API" chapter in place of the Java API
    page and a "Distributing events with Kafka / RabbitMQ" chapter), READMEs, `PORTING.md`, a migration guide for Scala and Java users, and CI.

After each milestone, run the full Rust check suite and report the results truthfully,
including failures. Record non-obvious design decisions as ADRs under `rust/docs/adr/`
(for example: the async runtime, the timestamp crate, how `Edomaton` is represented, the
codec trait design, and how the sqlx queries are organized).

## Constraints

- All code, comments, commits, PR titles and descriptions, and docs are in English.
- Each milestone is developed in its own worktree and branch, never on `main`. Follow the
  repository's contribution workflow, including the pre-PR documentation audit.
- Don't modify the Scala modules, except to add the tooling that generates the golden DDL
  files and the Scala side of the cross-language test.
- When the Scala behavior is ambiguous, the Scala tests win. If there are none, write a
  Scala test that pins the behavior first, then port it.

## Definition of done

- Every row in `PORTING.md` is ported: all 17 Scala modules and every test suite. No
  out-of-scope rows.
- `cargo tree` shows no JVM, JNI, or Scala/Java artifact. Every connector uses the standard
  Rust crates listed above.
- The `sqlx` driver passes every shared backend test suite that the Skunk and Doobie drivers pass.
- All Rust checks are green in CI, including the PostgreSQL integration tests and the
  cross-language compatibility test.
- The golden DDL tests pass for every naming strategy and payload type.
- With the `kafka` / `rabbitmq` features on, the relays pass the broker integration tests (at-least-once
  delivery, per-stream ordering, leader election). With them off, `cargo tree` shows no broker crate.
- Every Scala example has a working Rust equivalent, and the mdBook builds.
