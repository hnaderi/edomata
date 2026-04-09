---
sidebar_position: 3
title: "Java API"
---

# Java API

The `edomata-java-api` module provides a Java-friendly facade over Edomata's Scala types. It wraps the core abstractions into classes that are usable from pure Java code — no Scala knowledge required.

## Install

### Maven

```xml
<dependencies>
  <dependency>
    <groupId>io.github.beyond-scale-group</groupId>
    <artifactId>edomata-java-api_3</artifactId>
    <version>@VERSION@</version>
  </dependency>
</dependencies>
```

### Gradle

```groovy
dependencies {
    implementation 'io.github.beyond-scale-group:edomata-java-api_3:@VERSION@'
}
```

### SBT (mixed Scala/Java projects)

```scala
libraryDependencies += "io.github.beyond-scale-group" %% "edomata-java-api" % "@VERSION@"
```

The module depends on the Doobie backend. You do **not** need to add separate Doobie or Circe dependencies.

## Quick Start

Here is a minimal end-to-end example showing how to use Edomata from Java:

```java
import edomata.java.*;
import java.time.Instant;
import java.util.List;
import javax.sql.DataSource;

// 1. Define your domain model
JDomainModel<Integer, String, String> counterModel = JDomainModel.create(
    0, // initial state
    (event, state) -> {
        switch (event) {
            case "increment": return JEither.right(state + 1);
            case "decrement":
                if (state > 0) return JEither.right(state - 1);
                else return JEither.left(List.of("cannot go below zero"));
            default:
                return JEither.left(List.of("unknown event: " + event));
        }
    }
);

// 2. Define a command handler
JCommandHandler<String, Integer, String, String, Void> handler =
    JCommandHandler.create(ctx -> {
        String command = ctx.command();
        Integer state = ctx.state();
        switch (command) {
            case "inc": return JAppResult.accept("increment");
            case "dec": return JAppResult.accept("decrement");
            default:    return JAppResult.reject("unknown: " + command);
        }
    });

// 3. Build the backend
DataSource dataSource = /* your JDBC DataSource */;

try (EdomataRuntime runtime = EdomataRuntime.create();
     JBackend<Integer, String, String, Void> backend =
         JBackendBuilder.<Integer, String, String, Void>forDoobie(counterModel)
             .namespace("counter")
             .dataSource(dataSource)
             .eventCodec(JCodec.of(e -> e, s -> s))        // plain string codec
             .notificationCodec(JCodec.of(e -> "", s -> null))
             .build(runtime)) {

    // 4. Handle a command
    JCommandMessage<String> cmd =
        JCommandMessage.of("cmd-1", Instant.now(), "counter-1", "inc");

    JEither<List<String>, ?> result = backend.handle(handler, cmd).join();

    if (result.isRight()) {
        System.out.println("Command accepted!");
    } else {
        System.out.println("Rejected: " + result.getLeft());
    }

    // 5. Read the journal
    List<JEventMessage<String>> events =
        backend.journal().readStream("counter-1").join();
    events.forEach(e ->
        System.out.println("Event: " + e.payload() + " v" + e.version()));
}
```

## Core Types

### JDecision

Represents the outcome of a domain decision: accepted (with events), rejected (with reasons), or indecisive (pure value).

```java
// Accept with events
JDecision<String, String, ?> d = JDecision.accept("event1", "event2");

// Accept with a return value
JDecision<String, String, Integer> d = JDecision.acceptReturn(42, "event1");

// Reject
JDecision<String, String, ?> d = JDecision.reject("reason1", "reason2");

// Pure value (no events, no rejection)
JDecision<String, String, String> d = JDecision.pure("hello");

// Compose decisions
JDecision<String, String, ?> composed = d1.flatMap(result -> d2);

// Check outcome
d.isAccepted();
d.isRejected();
d.isIndecisive();
d.toEither(); // JEither<List<R>, A>
```

### JEither

A simple `Left`/`Right` union type (Java has no built-in `Either`).

```java
JEither<String, Integer> right = JEither.right(42);
JEither<String, Integer> left = JEither.left("error");

right.isRight();  // true
right.getRight(); // 42
left.getLeft();   // "error"

// Transform
String result = right.fold(
    l -> "Error: " + l,
    r -> "Value: " + r
);
```

### JDomainModel

Defines the domain model: initial state + transition function. There are two ways to create one:

```java
// Using the factory method
JDomainModel<MyState, MyEvent, MyRejection> model = JDomainModel.create(
    new MyState(), // initial state
    (event, state) -> {
        // return JEither.right(newState) or JEither.left(List.of(rejection))
    }
);

// Using a subclass (for more complex models)
class AccountModel extends JDomainModel<Account, AccountEvent, AccountError> {
    @Override
    public Account initial() { return Account.empty(); }

    @Override
    public JEither<List<AccountError>, Account> transition(
            AccountEvent event, Account state) {
        // ...
    }
}
```

### JCommandHandler

A function from request context (command + current state) to a result (decision + optional notifications).

```java
JCommandHandler<Command, State, Event, Rejection, Notification> handler =
    JCommandHandler.create(ctx -> {
        Command cmd = ctx.command();
        State state = ctx.state();
        String aggregateId = ctx.address();
        String messageId = ctx.messageId();

        // Return accept, reject, or publish notifications
        return JAppResult.accept(new SomeEvent());
    });
```

### JAppResult

The return type of command handlers. Combines a decision with optional notifications.

```java
// Accept events
JAppResult.accept("event1", "event2");

// Reject
JAppResult.reject("reason");

// Accept + publish notifications
JAppResult.decideAndPublish(
    JDecision.accept("event1"),
    List.of("notification1")
);

// Publish notifications only (no state change)
JAppResult.publish(List.of("notification1"));
```

## Backend

### JBackendBuilder

Fluent builder for the Doobie-backed event sourcing backend.

```java
JBackend<State, Event, Rejection, Notification> backend =
    JBackendBuilder.<State, Event, Rejection, Notification>forDoobie(model)
        .namespace("myaggregate")           // PostgreSQL table prefix
        .dataSource(dataSource)              // javax.sql.DataSource
        .eventCodec(eventCodec)              // JCodec<Event>
        .notificationCodec(notifCodec)       // JCodec<Notification>
        .maxRetry(5)                         // retries on version conflict
        .inMemSnapshotSize(1000)             // snapshot cache size
        .skipSetup(false)                    // set true for Flyway
        .build(runtime);
```

#### Naming strategies

```java
// Prefixed (default): tables named myapp_journal, myapp_outbox, etc.
builder.namespace("myapp")

// Schema: tables in a dedicated PostgreSQL schema
builder.schemaNamespace("myapp") // creates "myapp".journal, etc.
```

### JBackend

The main facade returned by the builder. Implements `AutoCloseable`.

```java
// Handle a command
CompletableFuture<JEither<List<R>, Unit>> result =
    backend.handle(handler, command);

// Read the journal
JJournalReader<E> journal = backend.journal();
CompletableFuture<List<JEventMessage<E>>> events =
    journal.readStream("my-aggregate-id");

// Read the outbox
JOutboxReader<N> outbox = backend.outbox();
CompletableFuture<List<JOutboxItem<N>>> items =
    outbox.read();

// Release resources
backend.close();
```

### EdomataRuntime

Manages the Cats Effect runtime. Use `create()` for the global shared runtime.

```java
// Recommended: shared global runtime (no shutdown needed)
EdomataRuntime runtime = EdomataRuntime.create();

// AutoCloseable for try-with-resources
try (EdomataRuntime rt = EdomataRuntime.create()) {
    // use rt
}
```

## Codecs

The `JCodec<T>` interface bridges Java serialization to Edomata's internal `BackendCodec.JsonB`. All payloads are stored as PostgreSQL `jsonb`.

```java
// Using Gson
JCodec<MyEvent> codec = JCodec.of(
    event -> gson.toJson(event),
    json -> gson.fromJson(json, MyEvent.class)
);

// Using Jackson
JCodec<MyEvent> codec = JCodec.of(
    event -> objectMapper.writeValueAsString(event),
    json -> objectMapper.readValue(json, MyEvent.class)
);
```

## DDL for Migrations

Use `JPGSchema` to generate SQL DDL for Flyway, Liquibase, or manual migrations.

```java
// Event sourcing tables (journal, outbox, commands, snapshots, migrations)
List<String> ddl = JPGSchema.eventsourcing("myapp");
ddl.forEach(System.out::println);

// CQRS tables (states, outbox, commands)
List<String> ddl = JPGSchema.cqrs("myapp");

// Custom payload types
List<String> ddl = JPGSchema.eventsourcing("myapp", "json", "jsonb", "bytea");

// Schema-based naming (CREATE SCHEMA + schema-qualified tables)
List<String> ddl = JPGSchema.eventsourcingWithSchema("myapp");
```

### Flyway workflow

1. Generate DDL:
   ```java
   JPGSchema.eventsourcing("myapp").forEach(System.out::println);
   ```
2. Paste into `src/main/resources/db/migration/V1__create_myapp_tables.sql`
3. Build with `skipSetup(true)`:
   ```java
   JBackendBuilder.forDoobie(model)
       .namespace("myapp")
       .skipSetup(true)  // tables already exist from Flyway
       .build(runtime);
   ```

## Reading Events and Outbox

### JJournalReader

```java
JJournalReader<Event> journal = backend.journal();

// All events for one aggregate
journal.readStream("aggregate-id").thenAccept(events -> {
    for (JEventMessage<Event> e : events) {
        System.out.printf("v%d: %s at %s%n",
            e.version(), e.payload(), e.time());
    }
});

// Events after a version
journal.readStreamAfter("aggregate-id", 5L);

// All events across all aggregates
journal.readAll();

// All events after a global sequence number
journal.readAllAfter(1000L);
```

### JOutboxReader

```java
JOutboxReader<Notification> outbox = backend.outbox();

outbox.read().thenAccept(items -> {
    for (JOutboxItem<Notification> item : items) {
        System.out.printf("stream=%s data=%s%n",
            item.streamId(), item.data());
    }
});
```

## Event and Command Messages

### JCommandMessage

```java
JCommandMessage<String> cmd = JCommandMessage.of(
    "unique-id",           // command ID (for idempotency)
    Instant.now(),         // timestamp
    "aggregate-123",       // target aggregate ID (stream)
    "my-command-payload"   // command payload
);

cmd.id();       // "unique-id"
cmd.time();     // the Instant
cmd.address();  // "aggregate-123"
cmd.payload();  // "my-command-payload"
```

### JEventMessage

Returned by the journal reader. Contains event metadata + payload.

```java
JEventMessage<Event> event = ...;

event.id();       // UUID
event.time();     // OffsetDateTime
event.seqNr();    // global sequence number
event.version();  // per-stream version
event.stream();   // aggregate/stream ID
event.payload();  // the deserialized event
```

## Thread Safety

- `EdomataRuntime` should be a **singleton** per application.
- `JBackend` is thread-safe — `handle()` can be called concurrently.
- `JCommandHandler` is thread-safe if the handler function is pure.
- All `CompletableFuture` results can be composed with standard Java async APIs.

## Error Handling

Backend operations return `CompletableFuture`. Failures surface as `CompletionException` wrapping:

- **Version conflict**: retried automatically up to `maxRetry` times
- **Codec errors**: if `JCodec.decode()` throws, the future completes exceptionally
- **Database errors**: connection/SQL failures propagate as-is

```java
backend.handle(handler, cmd)
    .thenAccept(result -> {
        if (result.isRight()) { /* success */ }
        else { /* domain rejection: result.getLeft() */ }
    })
    .exceptionally(ex -> {
        // infrastructure error (DB, codec, etc.)
        ex.printStackTrace();
        return null;
    });
```
