---
sidebar_position: 7
title: "Event Migrations"
---

# Event Migrations

## Why Schema Evolution Matters

In event sourcing, events are immutable — once written to the journal, they stay forever. But your domain evolves: you add new fields, rename concepts, split events into finer-grained variants. Without a migration strategy, you end up with a journal full of outdated formats that your current code can't deserialize.

> **The fundamental tension**: Events must be immutable for correctness, but your domain model must be free to evolve. Edomata resolves this by letting you **rewrite the journal** in a controlled, tracked, and compile-time-verified way.

## The Problem

Imagine you have an event `PriceUpdated(price: Long)` stored in your journal. You now need to add a `currency` field. What happens?

- **Option A: Add `Option[String]` default** — Works, but pollutes your domain model with optionality that only exists for historical reasons. Every handler must deal with `None`.
- **Option B: Keep both old and new event types** — Your transition function grows with every schema change. After a few iterations, you have `PriceUpdatedV1`, `PriceUpdatedV2`, `PriceUpdatedV3`...
- **Option C: Migrate the journal** — Transform old events to the new format in the database. Your code only ever sees the latest format. ✓

Edomata provides **Option C** with automatic tracking, compile-time safety, and Flyway-like idempotency.

## Defining Migrations

A migration transforms event payloads from one format to another. Use the typed factory to get compile-time exhaustivity checking:

```scala
import edomata.backend.EventMigration

// Old event type (kept as a sealed enum for compile-time safety)
enum EventV1:
  case Created(name: String)
  case PriceUpdated(price: Long)

// New event type
enum EventV2:
  case Created(name: String)
  case PriceUpdated(price: Long, currency: String)

val v1ToV2 = EventMigration[EventV1, EventV2](
  "001",
  "Add currency to PriceUpdated"
)(
  decode = io.circe.jawn.decode[EventV1](_).leftMap(_.getMessage),
  transform = {
    case EventV1.Created(name)      => EventV2.Created(name)
    case EventV1.PriceUpdated(price) => EventV2.PriceUpdated(price, "USD")
  },
  encode = _.asJson.noSpaces
)
```

## Chaining Migrations

Define multiple migrations and pass them as an ordered list. Each migration is applied independently — if `"001"` is already applied, only `"002"` runs:

```scala
val v2ToV3 = EventMigration[EventV2, EventV3](
  "002",
  "Add description to Created"
)(
  decode = io.circe.jawn.decode[EventV2](_).leftMap(_.getMessage),
  transform = {
    case EventV2.Created(name)              => EventV3.Created(name, description = "")
    case EventV2.PriceUpdated(price, currency) => EventV3.PriceUpdated(price, currency)
  },
  encode = _.asJson.noSpaces
)

val allMigrations = List(v1ToV2, v2ToV3)
```

You can also compose two migrations into one using `andThen`:

```scala
val v1ToV3 = v1ToV2.andThen(v2ToV3)
```

## Running Migrations

Call the migration runner **before** building your backend. It is idempotent and safe to call on every application startup:

### With Skunk

```scala
import edomata.skunk.SkunkMigrations

for
  result <- SkunkMigrations.run(naming, pool, allMigrations)
  _      <- IO.println(s"Applied: ${result.applied}, Skipped: ${result.skipped}")
  driver <- SkunkDriver.from(naming, pool)
  // Build backend with latest event codec only
yield driver
```

### With Doobie

```scala
import edomata.doobie.DoobieMigrations

for
  result <- DoobieMigrations.run(naming, transactor, allMigrations)
  _      <- IO.println(s"Applied: ${result.applied}, Skipped: ${result.skipped}")
  driver <- DoobieDriver.from(naming, transactor)
yield driver
```

## How It Works Internally

```
Application Startup
  │
  ├── CREATE TABLE IF NOT EXISTS migrations
  │     (version, description, applied_at)
  │
  ├── SELECT applied versions from migrations table
  │
  ├── For each pending migration (not yet applied):
  │     ├── BEGIN TRANSACTION
  │     ├── SELECT id, payload FROM journal
  │     ├── Apply transform to each payload
  │     ├── UPDATE journal SET payload = new_payload
  │     ├── INSERT INTO migrations (version, description)
  │     ├── TRUNCATE snapshots (invalidate cached state)
  │     └── COMMIT
  │
  └── Return MigrationResult(applied, skipped)
```

Each migration runs in its own transaction. If migration `"002"` fails, `"001"` is already committed and will not re-run. The `migrations` table tracks exactly which migrations have been applied, just like Flyway's `flyway_schema_history`.

Snapshots are truncated after each migration because they contain pre-computed state derived from the old event format. The backend will lazily rebuild snapshots from the migrated journal on the next read.

## Compile-Time Safety: Exhaustive Migrations

One of the biggest risks with event migration is **forgetting to handle an event variant**. If your old event type has 5 cases and your migration only transforms 4, the 5th will crash at runtime — potentially corrupting your journal or losing data.

Edomata prevents this at **compile time**. The typed migration factory takes a `transform: A => B` function — a **total function** over type `A`. When `A` is a sealed enum (as all well-designed event types should be), Scala 3's exhaustivity checker ensures every case is handled:

```scala
enum EventV1:
  case Created(name: String)
  case Updated(name: String)
  case Deleted                    // ← added later

val migration = EventMigration[EventV1, EventV2]("001", "...")(
  decode = ...,
  transform = {
    case EventV1.Created(n) => EventV2.Created(n, email = "")
    case EventV1.Updated(n) => EventV2.Updated(n, email = None)
    // COMPILE ERROR: match may not be exhaustive.
    // It would fail on pattern case: EventV1.Deleted
  },
  encode = ...
)
```

With `sbt-typelevel` (used by Edomata), this warning is promoted to a **fatal compilation error**. Your project will not compile until every event variant is handled.

### What this means in practice

- **Incomplete migrations are impossible to deploy** — the compiler catches them before they reach production
- **Old event types must remain as sealed enums** in your codebase until the migration has run in all environments (dev, staging, production)
- Once deployed everywhere, you can safely delete the old types

> **Tip:** Keep old event types in a dedicated `migrations` package to make it clear they exist only for migration purposes and can be cleaned up later.

### Without the typed factory

If you use the raw `EventMigration(version, description, run)` constructor directly (operating on JSON strings), you lose this compile-time guarantee. **Always prefer the typed factory** `EventMigration[A, B](version, description)(decode, transform, encode)` to benefit from exhaustivity checking.

## Best Practices

1. **Use sequential version strings** — `"001"`, `"002"`, `"003"`. They are identifiers, not sorted — order comes from list position.

2. **One migration per schema change** — Don't bundle unrelated changes. Each migration should have a clear, single purpose.

3. **Test migrations locally first** — Run against a copy of your production data before deploying.

4. **Back up your database** — Before running migrations in production, always take a backup. Migrations rewrite journal payloads in-place.

5. **Keep old event types until fully deployed** — The compile-time exhaustivity guarantee requires the old sealed enum to exist. Delete it only after all environments have run the migration.

6. **Use the same serialization library** — If your events are stored with Circe, decode with Circe. Don't mix serialization libraries in a single migration.

## Full Example

See `examples/src/main/scala/MigrationExample.scala` for a complete working example showing V1 → V2 → V3 event evolution with automatic migrations.
