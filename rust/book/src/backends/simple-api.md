# Simple API

`edomata-simple` is a closure-based facade for applications that do not want to touch the generic core types (`Edomaton`, `ResponseD`, `NonEmpty`, ...). It plays the role the `java-api` module plays for the Scala library: the same capabilities, plain data, and PostgreSQL through `edomata-sqlx`. The code of this page is compiled and tested as part of the workspace (`rust/book/samples/src/simple.rs`).

## Install

```toml
[dependencies]
edomata-simple = { path = "rust/crates/edomata-simple" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

`edomata-simple` re-exports everything an application needs (`CommandMessage`, `EventMessage`, `OutboxItem`, `PgPool`, `SqlxCodec`, ...), so `use edomata_simple::*;` is enough.

## Quick start

### 1. Define your domain model

A `SimpleDomainModel` has an initial state and a `transition` returning a plain `Result<State, Vec<Rejection>>`; `ClosureModel::new` builds one from closures:

```rust,ignore
{{#include ../../samples/src/simple.rs:model}}
```

### 2. Define a command handler

A `CommandHandler` is a function from a `Context` (the command, its message and the current state) to an `AppResult`: a `SimpleDecision` (accept events, reject, or do nothing) plus notifications. `CommandHandler::new_async` takes an asynchronous closure.

```rust,ignore
{{#include ../../samples/src/simple.rs:handler}}
```

### 3. Build the backend and handle commands

```rust,ignore
{{#include ../../samples/src/simple.rs:backend}}
```

`handle` returns `Ok(Ok(()))` when the command was accepted (or already handled), `Ok(Err(reasons))` when it was rejected, and `Err(SimpleError)` on storage failures. `compile(&handler)` gives a reusable service.

## Core types

| Type | Purpose |
|------|---------|
| `SimpleDecision<R, E, A>` | `Accepted { events, result }`, `Rejected { reasons }`, `Indecisive { result }`; `accept`, `accept_return`, `reject`, `pure`, `unit`, `map`, `and_then`, `to_result` |
| `AppResult<R, E, N>` | the decision of a handler plus its notifications; `accept`, `reject`, `publish`, `decide_and_publish`, `and_publish` |
| `SimpleDomainModel`, `ClosureModel`, `ModelAdapter` | models with `Result<S, Vec<R>>` transitions, adapted to the core `DomainModel` |
| `CommandHandler`, `Context` | closure-based handlers |
| `SimpleCodec`, `ClosureCodec`, `serde_codec` | JSON-string codecs adapted to `jsonb` storage codecs; `serde_codec::<T>()` for serde types |
| `SimpleError` | `InvalidNamespace`, `MissingConfig`, `Connection`, `Backend` |

Tests are plain assertions:

```rust,ignore
{{#include ../../samples/src/simple.rs:simple_tests}}
```

## Backend builder

| Method | Meaning |
|--------|---------|
| `namespace(ns)` | prefixed naming (`ns_journal`, ...) |
| `schema_namespace(ns)` | schema naming (`"ns".journal`) |
| `pool(pool)` / `database_url(url)` | the connection (a pool is opened by `build` when only a URL is given) |
| `event_codec` / `notification_codec` / `serde_codecs` | the codecs |
| `max_retry(n)` | retries on version conflicts (default 5) |
| `in_mem_snapshot_size(n)` | in-memory snapshot cache (default 1000) |
| `skip_setup(true)` | no DDL; the tables come from Flyway |
| `build().await` / `build_blocking(runtime)` | the asynchronous / blocking backend |

Missing settings and invalid namespaces are reported by `build` (`SimpleError::MissingConfig`, `SimpleError::InvalidNamespace`).

## Blocking use

`SimpleRuntime` owns (or borrows) a Tokio runtime, and `BlockingBackend` exposes command handling and reads as blocking calls, for code outside an asynchronous context:

```rust,ignore
{{#include ../../samples/src/simple.rs:blocking}}
```

## Reading events and the outbox

`SimpleJournal` (`read_stream`, `read_stream_after`, `read_all`, `read_all_after`) and `SimpleOutbox` (`read`, `mark_as_sent`, `mark_all_as_sent`) return vectors of `EventMessage` / `OutboxItem` instead of streams.

## DDL for migrations

```rust,ignore
{{#include ../../samples/src/simple.rs:ddl}}
```

`SimplePGSchema::eventsourcing(ns)` / `cqrs(ns)` use prefixed naming, `eventsourcing_with_schema(ns)` / `cqrs_with_schema(ns)` the schema naming; `*_with` variants take explicit payload types.
