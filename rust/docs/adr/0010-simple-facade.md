# ADR 0010: The simple facade

- Status: accepted
- Date: 2026-09-25
- Milestone: 7 (simple facade)

## Context

Scala's `java-api` module exists so that Java code can use Edomata without
Scala knowledge or Cats types: `J*` wrappers with Java collections,
`CompletableFuture`, an `EdomataRuntime` for `IORuntime`, closure-based
`JDomainModel` / `JCommandHandler`, a string `JCodec` and a `JPGSchema`
taking namespaces as strings, all backed by Doobie. The plan asks for the
facade's *purpose* as idiomatic Rust: a simplified, closure-based API for
users who do not want to touch the generic core types, backed by
`edomata-sqlx`.

## Decisions

1. **Plain data instead of wrappers.** `SimpleDecision<R, E, A>` is an
   enum with `Vec` payloads (Scala's `JDecision` with Java lists), and
   `AppResult<R, E, N>` a struct of a `SimpleDecision<R, E, ()>` and a
   `Vec<N>`. Conversion to the core `Decision` follows `Converters`: an
   accepted decision without events becomes `InDecisive`; a rejected
   decision without reasons is the `EmptyRejection` error (Scala's
   `IllegalArgumentException`). `JEither` has no counterpart: it maps to
   `Result` (with `Result<R, L>` argument order), and `JCommandMessage`,
   `JEventMessage` and `JOutboxItem` map to the core `CommandMessage`,
   `EventMessage` and `OutboxItem`, which are already plain structs. They
   are re-exported so that an application needs only `edomata-simple`.

2. **Closure-based model, handler and codec.** `SimpleDomainModel` returns
   `Result<S, Vec<R>>` from `transition`; `ClosureModel::new(initial, f)`
   is `JDomainModel.create` and `ModelAdapter` the `toModelTC` adapter
   (an empty rejection list panics with Scala's message). `CommandHandler`
   wraps either a plain function (`CommandHandler::new`) or an
   asynchronous one (`new_async`) from a `Context` (Scala's
   `JRequestContext`: command, message, state) to an `AppResult`.
   `SimpleCodec` works on JSON strings and adapts to a `jsonb` storage
   codec through `into_codec`; `serde_codec::<T>()` (and the builder's
   `serde_codecs()`) is the shortcut Rust users will actually take.
   Constructors live on the concrete types (`ClosureModel::new`,
   `ClosureCodec::new`, the free `serde_codec`) rather than on the traits,
   because a trait function that never mentions `Self` cannot be called
   without naming an implementing type (E0790).

3. **The builder validates late.** `SimpleBackend::builder(model)` mirrors
   `JBackendBuilder`: `namespace` (prefixed naming) / `schema_namespace`,
   `pool` or `database_url` in place of the `DataSource`, the two codecs,
   `max_retry` (5), `in_mem_snapshot_size` (1000) and `skip_setup`.
   Builder methods cannot fail, so an invalid namespace and missing
   settings are reported by `build` as `SimpleError::InvalidNamespace` /
   `MissingConfig` (the `IllegalArgumentException` /
   `IllegalStateException` of Java). `build` opens the pool when only a
   URL was given, creates the driver with `SqlxDriver::new_with` and
   wires the core `Backend` with an in-memory snapshot cache and the retry
   configuration, exactly like the Doobie-backed Java builder.

4. **Two calling styles.** `SimpleBackend` is asynchronous: `handle` and
   `compile` return `HandleResult<R>` (`Ok(Ok(()))`, `Ok(Err(reasons))`
   or a storage error), `SimpleJournal` / `SimpleOutbox` collect the
   streams into vectors (`JJournalReader.readStream(...).compile.toList`).
   `SimpleRuntime` is `EdomataRuntime` over Tokio, with one deliberate
   difference: Scala's `create()` wraps the global `IORuntime` and owns
   nothing, but Tokio has no global runtime, so `create()` owns a
   multi-threaded runtime and `close` shuts it down; `from_handle` borrows
   one (Scala's `fromExisting`) and `close` leaves it alone.
   `BlockingBackend` (built with `build_blocking`) exposes command
   handling, journal and outbox reads, outbox marking and `close` as
   blocking calls for code outside an asynchronous context, which is the
   role `CompletableFuture.join()` played in Java (`compile` stays
   asynchronous-only, a compiled service being a future-returning
   function). `SimpleOutbox` also offers `mark_as_sent` /
   `mark_all_as_sent`, which the Java reader lacked and without which the
   outbox cannot be drained.

5. **`SimplePGSchema` takes strings.** As `JPGSchema`, the plain methods
   use prefixed naming and the `*_with_schema` ones the schema naming;
   invalid namespaces are `SimpleError::InvalidNamespace` instead of
   exceptions.

## Tests

Every `java-api` suite is ported (`JEitherSuite` as a test of the `Result`
mapping, `ConvertersSuite` as a test of the `Decision` conversions and of
`NonEmpty::from_vec` / `into_vec`); `JavaApiTest.java` becomes
`tests/integration.rs`. The facade is also exercised end to end on
PostgreSQL (builder validation, commands, journal, outbox, `skip_setup`
with DDL from `SimplePGSchema`, the blocking backend), which the Scala
module never was.
