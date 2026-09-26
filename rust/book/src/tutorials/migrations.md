# Event migrations

The code of this chapter is compiled and tested as part of the workspace (`rust/book/samples/src/migrations.rs`).

## Why schema evolution matters

In event sourcing, events are immutable: once written to the journal, they stay forever. But your domain evolves: you add fields, rename concepts, split events. Without a migration strategy you end up with a journal full of outdated formats that your current code cannot deserialize.

> **The fundamental tension**: events must be immutable for correctness, but the domain model must be free to evolve. Edomata resolves it by letting you **rewrite the journal** in a controlled, tracked way.

## The problem

You stored `PriceUpdated { price }` and now need a `currency` field.

- **Option A: an `Option<String>` field** works, but pollutes the model with optionality that only exists for historical reasons.
- **Option B: keep old and new event types** makes the fold grow with every change: `PriceUpdatedV1`, `V2`, `V3`...
- **Option C: migrate the journal** transforms old events to the new format in the database; your code only sees the latest format. ✓

Edomata provides option C with automatic tracking and Flyway-like idempotency.

## Defining migrations

A migration transforms event payloads from one format to another. `EventMigration::typed` takes a decoder, a transformation and an encoder; the transformation is a `match` over the old type, so forgetting a variant is a compile error:

```rust,ignore
{{#include ../../samples/src/migrations.rs:versions}}
```

```rust,ignore
{{#include ../../samples/src/migrations.rs:migration}}
```

## Chaining migrations

Define several migrations and pass them as an ordered list; each is applied independently (if `"001"` was applied, only `"002"` runs). `EventMigration::new` works on the JSON text directly, without compile-time exhaustivity; `and_then` composes two migrations into one:

```rust,ignore
{{#include ../../samples/src/migrations.rs:chaining}}
```

Migrations are plain values, so they are easy to test:

```rust,ignore
{{#include ../../samples/src/migrations.rs:migration_tests}}
```

## Running migrations

Call the runner **before** building your backend. It is idempotent and safe to call on every startup:

```rust,ignore
{{#include ../../samples/src/migrations.rs:running}}
```

## How it works internally

```text
Application startup
  ├── CREATE TABLE IF NOT EXISTS migrations (version, description, applied_at)
  ├── SELECT applied versions
  ├── for each pending migration:
  │     ├── BEGIN
  │     ├── SELECT id, payload FROM journal
  │     ├── apply the transformation to each payload
  │     ├── UPDATE journal SET payload = new payload (in batches)
  │     ├── INSERT INTO migrations (version, description)
  │     ├── TRUNCATE snapshots (cached state is invalid now)
  │     └── COMMIT
  └── MigrationResult { applied, skipped }
```

Each migration runs in its own transaction: if `"002"` fails, `"001"` stays applied and will not re-run. The `migrations` table tracks what was applied, like Flyway's history table. Snapshots are truncated because they hold state derived from the old format; the backend rebuilds them lazily. The snapshots table must therefore exist: use persisted snapshots, or create it with `PGSchema`.

## Compile-time safety

The biggest risk of event migration is forgetting an event variant. With `EventMigration::typed`, the transformation is an exhaustive `match` over the old enum: the compiler refuses a migration that does not handle every case. Keep old event types in a dedicated module until the migration has run in every environment, then delete them.

## Best practices

1. **Sequential version strings** (`"001"`, `"002"`): they are identifiers; order comes from the list position.
2. **One migration per schema change.**
3. **Test migrations locally first**, against a copy of production data.
4. **Back up your database** before running migrations in production: they rewrite payloads in place.
5. **Keep old event types until fully deployed.**
6. **Use the same serialization**: events stored by serde are decoded by serde.

## Full example

`rust/examples/src/bin/migration.rs` seeds a V1 journal, runs the V1 → V2 → V3 migrations and reads the events back with the V3 codec.
