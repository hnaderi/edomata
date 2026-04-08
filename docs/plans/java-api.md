# Plan: `edomata-java-api` Module

## Overview

A JVM-only bridge module (`modules/java-api`) that wraps Edomata's Scala 3 types into Java-idiomatic classes. Internally uses `cats.effect.IO` and converts everything to `CompletableFuture` + Java collections at the API boundary.

## Module Coverage Map

Full inventory of Edomata modules and their Java API coverage plan:

| Module | Phase | Java Bridge Strategy |
|--------|-------|----------------------|
| **core** | 1 | `JDecision`, `JDomainModel`, `JCommandHandler`, `JAppResult` — wraps `Decision`, `Response`, `Edomaton`. `Stomaton` deferred to Phase 2. |
| **backend** | 1 | `JBackend`, `JBackendBuilder` — wraps `Backend` builder for event sourcing. CQRS builder in Phase 2. |
| **postgres** | 1 | `JPGSchema` — exposes DDL extraction (`PGSchema.eventsourcing`, `PGSchema.cqrs`) as `List<String>`. `PGNaming`/`PGNamespace` hidden behind builder, accept plain `String`. |
| **doobie** | 1 | Primary backend. `JBackendBuilder.forDoobie()` wraps `DoobieDriver`. |
| **doobie-circe** | 1 | Default codec path. Used internally when user provides `JCodec` with JSON strings. |
| **doobie-jsoniter** | 2 | Alternative codec — expose via `JBackendBuilder.jsoniterCodec()`. |
| **doobie-upickle** | 2 | Alternative codec — expose via `JBackendBuilder.upickleCodec()`. |
| **skunk** | 2 | `JBackendBuilder.forSkunk()` wrapping `SkunkDriver`. Less natural for Java (async/non-JDBC) but still useful. |
| **skunk-circe** | 2 | Default codec path for Skunk backend. |
| **skunk-jsoniter** | 3 | Alternative codec for Skunk. |
| **skunk-upickle** | 3 | Alternative codec for Skunk. |
| **saas** | 2 | `JTenantBackend` / `JTenantBackendBuilder` — wraps multi-tenant abstractions. |
| **saas-skunk** | 2 | Multi-tenant Skunk backend, exposed through `JTenantBackendBuilder.forSkunk()`. |
| **munit** | 2 | `JTestBackend` — JUnit 5 integration providing test fixtures (in-memory backend, assertion helpers). |
| **backend-tests** | — | Internal. No Java bridge needed. |
| **e2e** | — | Internal. No Java bridge needed. |

### No Extra Dependencies Needed

Research confirmed that all bridge utilities are already available:

| Need | Solution | Source |
|------|----------|--------|
| `IO` → `CompletableFuture` | `IO.unsafeToCompletableFuture()` | cats-effect (existing dep) |
| `CompletableFuture` → `IO` | `IO.fromCompletableFuture()` | cats-effect (existing dep) |
| Run `IO` from Java context | `cats.effect.std.Dispatcher` | cats-effect (existing dep) |
| FS2 Stream → `Flow.Publisher` | `fs2.interop.flow` | fs2-core (existing dep) |
| Scala ↔ Java collections | `scala.jdk.CollectionConverters` | Scala 3 stdlib |
| `Option` ↔ `Optional` | `scala.jdk.OptionConverters` | Scala 3 stdlib |
| `Future` ↔ `CompletionStage` | `scala.jdk.FutureConverters` | Scala 3 stdlib |
| Scala 3 ADT → Java | Manual facade (no library exists) | Hand-written wrappers |

## Design Decisions

| Choice | Rationale |
|--------|-----------|
| Hardcode `IO` (no `F[_]`) | Java users cannot deal with higher-kinded types |
| `CompletableFuture` at all boundaries | Standard Java async primitive |
| Collect FS2 streams into `List` | FS2 `Stream` is unusable from Java |
| Simple `Function<Context, Result>` instead of `Edomaton` monad | Monadic composition does not translate to Java |
| Doobie backend only (Phase 1) | Already JVM-only, JDBC-based — natural Java fit |
| JSON-string codecs only | Maps to `BackendCodec.JsonB`, covers 90% of use cases |

## Types to Expose

| Java-facing type | Wraps | Purpose |
|------------------|-------|---------|
| `JDecision<R, E, A>` | `Decision[R, E, A]` | Accept / Reject / Indecisive result |
| `JDomainModel<S, E, R>` | `ModelTC[S, E, R]` | Abstract class for domain definition |
| `JCommandHandler<C, S, E, R, N>` | `Edomaton` | Simple function from context to result |
| `JBackend` + `JBackendBuilder` | `Backend` + `DoobieDriver` | Builder pattern for wiring |
| `JCodec<T>` | `BackendCodec` | `encode(T) -> String`, `decode(String) -> T` |
| `JJournalReader<E>` | Journal `Stream` | `readStream(id) -> CompletableFuture<List<...>>` |
| `JOutboxReader<N>` | Outbox `Stream` | `readAll() -> CompletableFuture<List<...>>` |
| `EdomataRuntime` | `IORuntime` | Lifecycle management (`AutoCloseable`) |
| `JEither<L, R>` | — | Simple Either since Java has none |
| `JCommandMessage<C>` | `CommandMessage[C]` | Java-visible command envelope |
| `JEventMessage<E>` | `EventMessage[E]` | Java-visible event metadata + payload |
| `JPGSchema` | `PGSchema` | DDL extraction returning `List<String>` for Flyway/Liquibase |
| `JCQRSBackend` + `JCQRSBackendBuilder` | CQRS `Backend` | Phase 2 — CQRS backend facade (Stomaton-based) |
| `JStateHandler<C, S, R, N>` | `Stomaton` | Phase 2 — state-only command handler (no event sourcing) |
| `JTenantBackend` + `JTenantBackendBuilder` | SaaS `Backend` | Phase 2 — multi-tenant backend facade |
| `JTestBackend` | — | Phase 2 — JUnit 5 test fixtures and in-memory backend |

## File Listing

All source files under `modules/java-api/src/main/scala/edomata/java/`:

| File | Purpose |
|------|---------|
| `Converters.scala` | Internal Scala-Java type converters (private) |
| `JEither.scala` | Simple Either type for Java |
| `JDecision.scala` | Java-friendly Decision ADT with static factories |
| `JCodec.scala` | Java-friendly codec interface |
| `JCommandMessage.scala` | Java-friendly CommandMessage wrapper |
| `JEventMessage.scala` | Java-visible event metadata + payload wrapper |
| `JDomainModel.scala` | Abstract class for defining domain models |
| `JAppResult.scala` | Result of command handler logic (decision + notifications) |
| `JCommandHandler.scala` | Java-friendly command handler function type |
| `EdomataRuntime.scala` | IORuntime lifecycle management |
| `JJournalReader.scala` | Java-friendly journal reader |
| `JOutboxReader.scala` | Java-friendly outbox reader |
| `JBackendBuilder.scala` | Builder for constructing the backend |
| `JBackend.scala` | Main backend facade |
| `JPGSchema.scala` | DDL extraction for Flyway/Liquibase migrations |

Test files under `modules/java-api/src/test/`:

| File | Purpose |
|------|---------|
| `scala/edomata/java/JDecisionSuite.scala` | Unit tests for Decision bridge |
| `scala/edomata/java/JBackendSuite.scala` | Integration test with Docker PG |
| `java/edomata/java/JavaUsageTest.java` | Actual Java file proving the API compiles and runs |

## Implementation Sequence

### Step 1 — Internal converters

Create `Converters.scala` with private utilities to convert between Scala and Java collections, `Decision` types, and `Either`/`ValidatedNec`.

### Step 2 — Core value types

Create `JEither.scala` and `JDecision.scala`.

`JDecision` is a sealed hierarchy with:
- `Accepted<R, E, A>` — holds `java.util.List<E>` and result `A`
- `Rejected<R, E, A>` — holds `java.util.List<R>`
- `Indecisive<R, E, A>` — holds result `A`

Static factory methods:
- `JDecision.accept(event, events...)`
- `JDecision.acceptReturn(value, event, events...)`
- `JDecision.reject(reason, reasons...)`
- `JDecision.pure(value)`

### Step 3 — Supporting types

Create `JCodec.scala`, `JCommandMessage.scala`, `JEventMessage.scala`.

`JCodec<T>` interface:
```java
String encode(T value);
T decode(String json) throws CodecException;
```

### Step 4 — Domain model bridge

Create `JDomainModel.scala` — an abstract class Java users extend:

```java
public class Counter extends JDomainModel<CounterState, CounterEvent, CounterRejection> {
    public CounterState initial() { return CounterState.EMPTY; }
    public JEither<List<CounterRejection>, CounterState> transition(CounterEvent event, CounterState state) { ... }
}
```

Internally creates the Scala `ModelTC[S, E, R]` by adapting the Java `transition` function.

### Step 5 — Command handling bridge

Create `JAppResult.scala` and `JCommandHandler.scala`.

`JAppResult<R, E, N>` holds a `JDecision` + a `List<N>` of notifications. Static factories:
- `JAppResult.decide(decision)`
- `JAppResult.accept(event)`
- `JAppResult.reject(reason)`
- `JAppResult.publish(notification)`

`JCommandHandler` wraps a `Function<JRequestContext<C, S>, JAppResult<R, E, N>>`.

### Step 6 — Runtime management

Create `EdomataRuntime.scala`:
```java
EdomataRuntime runtime = EdomataRuntime.create();       // default
EdomataRuntime runtime = EdomataRuntime.create(threads); // custom
runtime.close();                                         // AutoCloseable
```

Manages the Cats Effect `IORuntime` lifecycle.

### Step 7 — Read-side wrappers

Create `JJournalReader.scala` and `JOutboxReader.scala`.

```java
CompletableFuture<List<JEventMessage<E>>> readStream(String streamId);
CompletableFuture<List<JEventMessage<E>>> readAll();
```

FS2 streams are collected into lists via `.compile.toList` before converting to `CompletableFuture`.

### Step 8 — Backend builder and facade

Create `JBackendBuilder.scala` and `JBackend.scala`.

Builder accepts:
- `DataSource` (javax.sql)
- namespace string
- `JCodec` instances for events, state, notifications
- optional snapshot config
- optional retry config

`JBackend` exposes:
- `handle(handler, command) -> CompletableFuture<JEither<List<R>, Void>>`
- `journal() -> JJournalReader<E>`
- `outbox() -> JOutboxReader<N>`
- `close()` — releases the Cats Effect `Resource`

Implements `AutoCloseable`.

### Step 9 — build.sbt changes

Add the module definition:

```scala
lazy val javaApi = project
  .in(file("modules/java-api"))
  .dependsOn(doobieBackend.jvm, doobieCirceCodecs.jvm)
  .settings(
    name := "module-java-api",
    moduleName := "edomata-java-api",
    description := "Java-friendly API for edomata with Doobie backend",
    Compile / javacOptions ++= Seq("-source", "11", "-target", "11")
  )
```

### Step 10 — Tests

- Scala unit tests for `JDecision` conversion round-trips
- Integration test with Docker PostgreSQL for full backend lifecycle
- **Actual `.java` test file** proving the API compiles and runs from Java code

## Target Java Usage

```java
// 1. Define domain model
public class Counter extends JDomainModel<CounterState, CounterEvent, CounterRejection> {
    public CounterState initial() { return new CounterState(0); }
    public JEither<List<CounterRejection>, CounterState> transition(
            CounterEvent event, CounterState state) {
        return JEither.right(new CounterState(state.count() + event.amount()));
    }
}

// 2. Define command handler
JCommandHandler<String, CounterState, CounterEvent, CounterRejection, Void> handler =
    JCommandHandler.create(counterModel, ctx -> {
        String command = ctx.command();
        return JAppResult.accept(new CounterEvent(Integer.parseInt(command)));
    });

// 3. Build backend
try (EdomataRuntime runtime = EdomataRuntime.create();
     JBackend<CounterState, CounterEvent, CounterRejection, Void> backend =
         JBackendBuilder.forDoobie(counterModel)
             .dataSource(dataSource)
             .namespace("counter")
             .jsonCodec(CounterEvent.class, gson::toJson,
                 json -> gson.fromJson(json, CounterEvent.class))
             .build(runtime)) {

    // 4. Handle commands
    JCommandMessage<String> cmd = JCommandMessage.of("cmd-1", Instant.now(), "counter-1", "5");
    CompletableFuture<JEither<List<CounterRejection>, Void>> result =
        backend.handle(handler, cmd);

    // 5. Read journal
    CompletableFuture<List<JEventMessage<CounterEvent>>> events =
        backend.journal().readStream("counter-1");
}
```

## Challenges and Mitigations

| Challenge | Mitigation |
|-----------|------------|
| `PGNamespace` is an opaque type | Construct on Scala side; accept plain `String` in builder |
| Resource lifecycle | `JBackend` implements `AutoCloseable`; document try-with-resources |
| Exception mapping | `VersionConflict` and `MaxRetryExceeded` surface as `CompletionException`; document in Javadoc |
| Generics erasure | Codec bridge requires explicit `Class<T>` parameters |
| Thread safety | Document `EdomataRuntime` as application singleton |

## Phased Rollout

### Phase 1 — Core + Doobie Event Sourcing (this plan)

- Core types: `JDecision`, `JEither`, `JDomainModel`, `JCommandHandler`, `JAppResult`
- Supporting types: `JCodec`, `JCommandMessage`, `JEventMessage`
- Backend: `JBackend`, `JBackendBuilder` (Doobie event sourcing + Circe codecs)
- DDL: `JPGSchema` for migration tool integration
- Runtime: `EdomataRuntime`
- Tests: Scala unit tests + Java compilation test

### Phase 2 — CQRS, Skunk, SaaS, Testing

- CQRS backend: `JCQRSBackend`, `JCQRSBackendBuilder`, `JStateHandler` (wrapping `Stomaton`)
- Skunk backend: `JBackendBuilder.forSkunk()`, `JCQRSBackendBuilder.forSkunk()`
- Skunk Circe codecs
- Alternative Doobie codecs: jsoniter, upickle via `JBackendBuilder.jsoniterCodec()` / `.upickleCodec()`
- Multi-tenant SaaS: `JTenantBackend`, `JTenantBackendBuilder` (wrapping `saas` + `saas-skunk`)
- Test support: `JTestBackend` with JUnit 5 integration, in-memory backend, assertion helpers
- Callback-based outbox consumer: `JOutboxConsumer.start(backend, item -> { ... })`

### Phase 3 — Streaming, Alternative Codecs, Reactor

- FS2 streams exposed as `Flow.Publisher` (optional streaming API alongside `List` collection)
- Reactor / RxJava adapters (bridge through `Flow.Publisher`)
- Skunk alternative codecs: jsoniter, upickle
- Binary codec support (`bytea` payloads)
