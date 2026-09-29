# Troubleshooting

Common errors, what they mean and what to do about them. The code is in `rust/book/samples/src/troubleshooting.rs`.

## Reading a command result

A compiled service returns a `CommandResult<R>`, that is `Result<Result<(), NonEmpty<R>>, BackendError>`. The outer `Err` is a failure of the backend; the inner `Err` is a business rejection:

```rust,ignore
{{#include ../../samples/src/troubleshooting.rs:classify}}
```

`BackendError` has four variants:

| Variant | Message | Raised when |
|---------|---------|-------------|
| `VersionConflict` | "You can't proceed due to version conflict, read and decide again!" | a write lost a race on `(stream, version)` or on a command id |
| `MaxRetryExceeded` | "Maximum number of retries exceeded!" | every attempt of the retry policy conflicted |
| `PersistenceError(message)` | the message | a codec failed, a write affected an unexpected number of rows, a namespace was invalid, a migration failed |
| `UnknownError(source)` | "Unknown error!" | any other driver error, such as a `sqlx::Error` (connection, SQL, pool timeout); the source is kept |

`UnknownError` prints only "Unknown error!": log its source (`std::error::Error::source`, or downcast it as in `sqlx_cause` above), or format the whole chain with a crate such as `anyhow`.

## `MaxRetryExceeded` and `VersionConflict`

**Symptom**: commands fail with `MaxRetryExceeded`; you never see `VersionConflict` from a service.

A compiled service retries version conflicts itself ([Optimistic concurrency and retries](../guides/cookbook.md#optimistic-concurrency-and-retries)) and reports `MaxRetryExceeded` when the last attempt still conflicts; `VersionConflict` reaches you only from direct calls to a repository or storage. The command was not applied, so retrying it later with the same id is safe. If it happens often:

- several writers compete for **one aggregate**: that aggregate is a hot spot; route its commands through one queue or partition, or split it;
- the **retry policy** is too short for the contention: raise `max_retry` or `initial_delay` in `RetryConfig`;
- a **program is slow** between loading and writing (a slow `eval` effect): the longer a command runs, the more likely another writer wins; move the effect out, to an outbox consumer.

## A command is accepted but nothing changes

- **The command id was already handled.** A redundant command returns `Ok(Ok(()))` and writes nothing. Check that command ids are unique per intent (not reused across aggregates or commands) and, for tests, that each test uses fresh ids or a fresh namespace.
- **The program is indecisive.** A program that accepts no event returns `Ok(Ok(()))` too; only its notifications are written. Test it with `edomata-testkit` to see its `EdomatonResult`.

## Codec errors

**Symptom**: `PersistenceError("decoding failed: ...")` when loading an aggregate or reading the journal or the outbox, or `"encoding failed: ..."` when writing.

The stored JSON does not match your serde types:

```rust,ignore
{{#include ../../samples/src/troubleshooting.rs:codec_error}}
```

- **The type changed** since the payload was written (a renamed variant or field, a new required field). Make the change backward compatible (`#[serde(default)]`, `#[serde(alias = "...")]`) or rewrite the journal with an [event migration](../tutorials/migrations.md).
- **The payload was written by Scala** with another JSON shape: Circe, jsoniter and uPickle encode enums differently; pick the matching serde representation ([Matching the JSON shape](../other/migration-guide.md#matching-the-json-shape)). uPickle's `msgpack` payloads cannot be read at all.
- **The column type differs** from the codec's format (for instance a `bytea` column read with a `jsonb` codec): use the codec of the column, `SqlxCodec::<T>::json()` / `jsonb()` / `bytea()`, and the same payload types in `PGSchema::eventsourcing_with`.

A journal event that decodes but that the model refuses (`transition` returns `Err`) is not a codec error: the repository reads the aggregate as `AggregateState::Conflicted`, and commands on it are rejected with the model's reasons. Fix the model (or migrate the events) so that the whole history applies again.

## Connection and setup problems

| Symptom | Cause | Fix |
|---------|-------|-----|
| `UnknownError` whose source is `PoolTimedOut` | every connection of the pool is busy, or the server is unreachable | raise `max_connections`, check the server; remember that each relay leader holds one extra connection |
| `UnknownError` whose source is an I/O or TLS error when connecting | wrong `DATABASE_URL`, host or port | check the URL; with a local PostgreSQL also on `localhost:5432`, you may be talking to the wrong server |
| `relation "..." does not exist` | the tables were not created: `skip_setup = true` without the Flyway migration, a different namespace or naming strategy, or a query before the backend was built | run the DDL from `PGSchema`, check the namespace and `PGNaming::schema` vs `PGNaming::prefixed` |
| `permission denied for schema` / `for table` | the application role cannot run the setup DDL | create the tables with Flyway and use `skip_setup = true` |
| `PersistenceError` mentioning an invalid namespace | the namespace is not a PostgreSQL identifier | see below |
| `SqlxMigrations::run` fails on the `snapshots` table | the table does not exist (in-memory snapshots, no Flyway) | create it with `PGSchema` before running migrations |
| a relay never wakes up in another process | no `NOTIFY` on the channel it listens to | configure `with_outbox_notify_channel` on the writer's driver with the same channel; polling still delivers every `poll_interval` |

Namespaces are validated before any SQL is sent:

```rust,ignore
{{#include ../../samples/src/troubleshooting.rs:namespace_error}}
```

## Relays

- **A relay stops with `RelayError::Publish`**: a permanent broker error, or the `max_retries` budget of its `RetryPolicy` was exhausted. Nothing was marked, so restarting republishes the batch.
- **A relay stops with `RelayError::Encode`**: the encoder cannot encode an item; the relay stops at the same item on every restart until the encoder or the data is fixed.
- **Consumers see duplicates**: expected with at-least-once delivery; deduplicate on the message id ([Running relays](relays.md#delivery-guarantees)).
- **No replica publishes**: another process holds the advisory lock (for instance a relay that is stuck); look for the lock in `pg_locks` (`locktype = 'advisory'`) and the session that holds it.
