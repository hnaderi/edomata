# Running

The code of this chapter is compiled as part of the workspace (`rust/book/samples/src/running.rs`); it needs a PostgreSQL instance to run.

## What is a backend?

So far we have written pure domain logic and service definitions. They are *descriptions* of what should happen, like a recipe that has not been cooked yet. A **backend** executes them, connecting them to a database and real-world infrastructure.

```text
 Your code (pure)                          Backend (side effects)
 Domain model → Service logic → Edomaton  ──compile──▶  storage · caching · message delivery
```

## The program / interpreter pattern

A common functional pattern: programs define *what*, interpreters define *how*. An `Edomaton` is a definition of an action; it does not affect its environment even if it runs several times (programs that use side effects must follow this rule), it just computes a decision. A backend interprets it.

> **Why separate what from how?** Testability (pure code needs no database), flexibility (swap backends without touching business logic), clarity (business rules are not tangled with infrastructure).

## Backends

| Backend | Crate | Notes |
|---------|-------|-------|
| **PostgreSQL** | `edomata-sqlx` | Asynchronous, built on `sqlx`; the production backend. Shares its tables and payloads with the Scala Skunk and Doobie backends. |
| **In memory** | `edomata-backend` (`inmemory` module) | Reproduces the PostgreSQL constraints in memory; for tests and prototypes. |

Creating new ones is straightforward: a backend is an implementation of the `StorageDriver` trait (event sourcing) or `cqrs::StorageDriver` (CQRS).

### PostgreSQL

The PostgreSQL backend is designed to be standard and a great default in production for service-based, event-driven architectures.

#### Persistence

Each aggregate type has its own namespace containing all the required tables. By default a namespace is a dedicated PostgreSQL schema (`"auth".journal`); in **prefixed mode** every table lives in the current schema with a prefixed name (`auth_journal`), see [PostgreSQL (sqlx)](../backends/postgres.md). The tables:

| Table | Purpose | Used by |
|-------|---------|---------|
| `journal` | all events, in order | event-sourced apps (`Edomaton`) |
| `outbox` | notifications waiting to be published | both |
| `commands` | processed command ids (idempotency) | both |
| `snapshots` | cached state (performance) | event-sourced apps |
| `states` | current state | CQRS apps (`Stomaton`) |
| `migrations` | applied event migrations | event-sourced apps |

> **What is an outbox?** A pattern for reliable message publishing. Instead of sending notifications directly (which might fail), they are stored in the database atomically with the state change; a separate process publishes them. Nothing is lost; delivery is at-least-once, so consumers must tolerate duplicates.

> **What is idempotency here?** The `commands` table records processed command ids: the same command sent twice (network retries, ...) is handled once.

```text
journal (id uuid PK, time, seqnr bigserial, version int8, stream text, payload jsonb)
outbox  (seqnr bigserial PK, stream, correlation, causation, payload jsonb, created, published)
commands(id text PK, time, address)
snapshots(id text PK, version int8, state jsonb)
states  (id text PK, version int8, state jsonb)
migrations(version text PK, description, applied_at)
```

The DDL is byte-for-byte identical to the Scala library's (checked by golden tests), so both implementations can share the tables.

#### Memory image

The backend has built-in caches (commands, states, snapshots) that, when sized correctly, avoid most database work and run the application like a [memory image](https://martinfowler.com/bliki/MemoryImage.html).

#### Serialization

Payload columns can be `jsonb` (the default, indexable and queryable), `json` or `bytea`. `edomata-serde` provides a `serde`-based codec for the three formats; you can also implement the `Codec` trait yourself.

| Format | PostgreSQL type | Pros | Cons |
|--------|-----------------|------|------|
| JSON (binary) | `jsonb` | indexable, fast queries | slightly less readable in dumps |
| JSON (text) | `json` | human readable | larger, slower queries |
| Binary | `bytea` | compact | not human readable |

> **Recommendation**: start with `jsonb`. It is also what `build_default()` uses.

## Minimal example

```rust,ignore
{{#include ../../samples/src/running.rs:minimal}}
```

> **What's happening here?**
> 1. Create a connection pool to PostgreSQL.
> 2. Create a driver for the `account` namespace (tables are created automatically unless `skip_setup` is used).
> 3. Build a backend with your domain model; `build_default` picks serde `jsonb` codecs for events and notifications, and the snapshot codec is given to `persisted_snapshot`.
> 4. Compile the pure `Edomaton` into a service that talks to the database.
> 5. Send commands and get results: `Ok(Ok(()))` when accepted (or already handled), `Ok(Err(reasons))` when rejected, `Err(BackendError)` on storage failures.

### Builder options

```rust,ignore
{{#include ../../samples/src/running.rs:builder_options}}
```

### CQRS backends

CQRS programs compile the same way with `edomata_backend::cqrs::Backend` and `SqlxCqrsDriver`; an optional handler runs inside the transaction that saves the state, which is how projections stay consistent with the aggregate:

```rust,ignore
{{#include ../../samples/src/running.rs:cqrs_backend}}
```

## Usage

For naming strategies, Flyway integration and codecs, see [PostgreSQL (sqlx)](../backends/postgres.md).
