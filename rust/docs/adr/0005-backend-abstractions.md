# ADR 0005: Backend abstractions

- Status: accepted
- Date: 2026-09-25
- Milestone: 2 (backend abstractions)

## Context

`modules/backend` defines the storage-facing traits (`Repository`,
`JournalReader`, `OutboxReader`, `SnapshotStore`, `CommandStore`, ...), the
command handler with optimistic concurrency and retry, caching, and the
`Backend` builder. Scala relies on `F[_]`, `fs2.Stream`, Cats Effect
`Resource`, `Queue.circularBuffer` and implicit typeclass instances
(`Codec[_]`, `ModelTC`). This ADR records how each was mapped.

## Decisions

### Object-safe traits with `async_trait`

All storage traits are `dyn`-compatible so that a `Backend<S, E, R, N>` can
hold `Arc<dyn Repository<S, E, R, N>>` and friends without leaking driver
types. `#[async_trait]` (boxed futures) is used for that; the source still
reads as `async fn` in traits. Payload types must satisfy the `Payload`
marker (`Clone + Send + Sync + 'static`).

Streams (`fs2.Stream`) are `futures::stream::BoxStream<'_, Result<T,
BackendError>>`.

### Codecs and drivers: `StorageDriver::Codec<T>` (GAT)

Scala's `StorageDriver[F, Codec[_]]` takes the codec typeclass as a type
constructor. The Rust trait has a generic associated type
`type Codec<T>: Send + Sync + 'static` naming the codec value a driver needs
for a payload of type `T`: `()` for the in-memory driver, a serde codec for
PostgreSQL. `BackendBuilder::build(event_codec, notification_codec)` takes
them explicitly; `build_default()` works when the codec type is `Default`.
The CQRS driver has a second GAT, `Handler<N>`, for the driver-specific
transactional notification hook (Scala's `Handler[_]`).

The shared `Codec<T>` trait (format + `encode`/`decode` to bytes) lives in
`edomata-backend`, as the plan requires; `edomata-serde` implements it.

### `DomainModel` is object-safe

`DomainModel::perform` takes a `Decision<R, E, ()>` and the generic helpers
(`handle`, `decide`, `accept`, `dsl`) carry `where Self: Sized`, so the
backend stores `Arc<dyn DomainModel<State = S, Event = E, Rejection = R>>`
(`SharedModel`). CQRS storages only need the initial state, exposed through
the object-safe `StateModel<S>` trait (blanket-implemented for `CqrsModel`).

### Payload-erased commands: `CommandRef<'_>`

Scala passes `CommandMessage[?]` / `RequestContext[?, ?]` to repositories.
Rust has no existential payload, so repositories receive `CommandRef<'_>`
(id, time, address, metadata borrowed from the message). Command handlers
copy the header before the program consumes the message.

### Command outcome: nested `Result`

Scala's `F[EitherNec[R, Unit]]` becomes
`Result<Result<(), NonEmpty<R>>, BackendError>` (`CommandResult<R>`): the
outer level is the backend failing, the inner level the domain rejecting.

### Programs have no error channel

An `Edomaton`'s effects are infallible futures; failures inside a program
must be expressed as rejections. Consequently the Scala tests that raise
exceptions from inside a program ("Must not change raised errors", "Must
retry on version conflict") are ported with repositories that fail; the
handler propagates repository errors untouched and retries version
conflicts up to `max_retry` attempts (with exponential backoff and up to
500 ms of jitter, exactly like Scala's `retry`).

### Notifications: `tokio::sync::Notify`, not `broadcast`

The plan suggested `tokio::sync::broadcast`. Scala's `Queue.circularBuffer(1)`
keeps one pending signal until it is consumed and coalesces bursts; the
ported `NotificationsSuite` notifies **before** subscribing and expects
exactly one signal. A broadcast channel drops messages sent before a
receiver subscribes, so it cannot satisfy that suite. `Notify::notify_one`
has exactly the circular-buffer semantics (one stored permit, coalescing),
so `Signal` wraps a `Notify`. Cross-process wake-ups (for the outbox relay)
will use PostgreSQL `LISTEN/NOTIFY`, as planned.

### Resources and lifetimes

Cats Effect `Resource`s become owned values plus an explicit
`async fn close()`. `PersistedSnapshotStore` starts its persister task in
`new` (a Tokio runtime is required) and flushes on `close()`; `Drop` only
aborts the task. `Backend::close()` forwards to the snapshot store.

### Outbox consumer batching

`OutboxConsumer` processes items in batches (default 100, like Skunk's
`stream(_, 100)` fetch size) and marks a batch as sent only after every
handler succeeded, so delivery is at-least-once. Scala relied on the
underlying stream's chunk boundaries for the same behaviour.

### In-memory driver

`InMemoryDriver` implements both `StorageDriver` traits (Scala has none; its
in-memory pieces are test doubles). It reproduces the PostgreSQL constraints
exactly: a duplicate `(stream, version)` or command id is a
`VersionConflict`, and the CQRS upsert inserts a new row at version 1
regardless of the expected version. Stores can be seeded for compatibility
tests. This is what lets `edomata-backend-tests` run the shared suites
without a database.
