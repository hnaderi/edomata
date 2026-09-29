# Testing

Domain logic is pure, so most tests are plain assertions on values. This chapter goes from the fastest tests to the most complete ones: decisions, programs with `edomata-testkit`, whole services on the in-memory driver, and integration tests against PostgreSQL. The code is in `rust/book/samples/src/testing.rs`; all of it runs with `cargo test`.

## Decisions and models

Decisions and folds are functions: call them and compare the results. No runtime is needed:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:model_tests}}
```

## Programs with `edomata-testkit`

```toml
[dev-dependencies]
edomata-testkit = { git = "https://github.com/beyond-scale-group/edomata" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

`EdomatonAssertions` adds assertion methods to every `Edomaton` over a `RequestContext`. Each one builds a command message (id `"1"`, time `DateTime::<Utc>::MIN_UTC`, address `"sut"`), runs the program on the given state, folds the decision into the model like the backends do, and panics with a readable message when the outcome differs. They are `async`, so they work with `#[tokio::test]`, `futures::executor::block_on` or any other executor:

```rust,ignore
{{#include ../../samples/src/testing.rs:testkit}}
```

| Method | Asserts |
|--------|---------|
| `expect(model, command, state, new_state, notifications)` | accepted, exactly this state and these notifications, in order |
| `expect_all(...)` | the same, with the notifications in any order |
| `expect_that(model, command, state, notifications, check)` | accepted, these notifications, then runs `check` (a closure with your own assertions) on the new state |
| `expect_rejection(model, command, state)` | rejected; returns the notifications and the reasons |
| `expect_rejection_with(model, command, state, reasons)` | rejected with exactly these reasons, and no notification |
| `expect_rejection_and_notify(model, command, state, reasons, notifications)` | rejected with these reasons and these notifications |
| `expect_rejection_notify(model, command, state, notifications)` | rejected, publishing exactly these notifications |
| `run_with` / `run_with_command` | nothing: returns the `EdomatonResult` |

A `TestCommand` sets another message id, time or address, for programs that read them:

```rust,ignore
{{#include ../../samples/src/testing.rs:testkit_custom}}
```

`StomatonAssertions` does the same for CQRS programs (`expect`, `expect_rejection_with`, `run_with`); there is no model to fold, so they take the command and the state only:

```rust,ignore
{{#include ../../samples/src/testing.rs:testkit_cqrs}}
```

## Services on the in-memory driver

`edomata_backend::inmemory::InMemoryDriver` is a complete storage driver kept in the process. It reproduces the PostgreSQL drivers' rules (versions, command idempotency, outbox, snapshots) and needs no codec, so `build_default()` builds the backend. Use it to test what happens around the domain: command routing, idempotency, what reaches the journal and the outbox:

```rust,ignore
{{#include ../../samples/src/testing.rs:in_memory}}
```

To start from an existing history, create an `InMemoryEventStore`, seed it with `seed_journal`, `seed_outbox`, `seed_commands` or `seed_snapshots`, and pass it to `InMemoryDriver::with_event_store`; `journal_rows()` then shows what the backend wrote. For CQRS, seed an `InMemoryStateStore` with `seed_states`, `seed_outbox` or `seed_commands` and pass it to `InMemoryDriver::with_state_store`.

## Integration tests against PostgreSQL

The in-memory driver does not check your codecs, your SQL or your DDL. Integration tests run the same service on `edomata-sqlx`:

```rust,ignore
{{#include ../../samples/src/testing.rs:postgres}}
```

- Start PostgreSQL with `docker-compose up -d` at the root of the repository (PostgreSQL 14, user and password `postgres`), or point `DATABASE_URL` at another server. If a local PostgreSQL also listens on `localhost:5432`, set `DATABASE_URL` to the container through the machine's LAN address.
- Give each test its own namespace (a prefixed name, or a schema) and drop its tables first: tests of one binary run in parallel, and a rerun must start clean.
- In CI, run PostgreSQL as a service container and export `DATABASE_URL`, like the workspace's own `.github/workflows/rust.yml`.
- The shared storage suites of the workspace (`edomata-backend-tests`) check any driver against the same expectations; a custom driver can reuse them.

## What to test where

| Test | Covers | Needs |
|------|--------|-------|
| decisions and models | business rules, folds, conflicts | nothing |
| `edomata-testkit` | programs: routing, notifications, rejections | an executor |
| in-memory driver | services: idempotency, outbox, journal, retries | Tokio |
| PostgreSQL | codecs, DDL, naming, migrations, concurrency | a database |
