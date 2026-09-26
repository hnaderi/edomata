# ADR 0008: The sqlx driver

- Status: accepted
- Date: 2026-09-25
- Milestone: 5 (sqlx driver)

## Context

Scala ships two PostgreSQL drivers, Skunk (async, cross-platform) and Doobie
(JDBC), with duplicated `Queries.scala` files, repositories, readers,
snapshot persistence and migration runners. The plan replaces both with one
`sqlx`-based driver that covers every capability and keeps the wire format
so that Rust and Scala services can share a database.

## Decisions

1. **One driver, two storage-driver impls.** `SqlxDriver` implements
   `eventsourcing::StorageDriver` and `SqlxCqrsDriver` implements
   `cqrs::StorageDriver`, both over a `sqlx::PgPool` (the "transactor").
   Constructors mirror Scala's: `for_namespace("ns", pool)` (schema mode
   from a string), `new(naming, pool)` and `new_with(naming, pool,
   skip_setup)`. They are `async` because, like Scala's `from`, they create
   the schema eagerly; `skip_setup = true` disables every DDL statement.

2. **SQL is organised as query catalogues.** `queries.rs` holds one struct
   per table (`JournalQueries`, `OutboxQueries`, `SnapshotQueries`,
   `CommandQueries`, `StateQueries`, `MigrationQueries`) with the SQL
   strings pre-rendered for a naming strategy, so statements are built once
   and readers can return streams that borrow `&self`. The statements are
   the ones of the Skunk/Doobie `Queries.scala` (same columns, ordering,
   `on conflict` upserts); setup DDL comes from `edomata_postgres::ddl`,
   which is what `PGSchema` generates for migration tools, so automatic
   setup and Flyway scripts cannot drift apart. The one intentional
   difference: Skunk wraps the journal DDL in a `DO $$ ... $$` block, which
   sqlx executes as separate statements instead.

3. **Codecs are values.** `StorageDriver::Codec<T> = SqlxCodec<T>`, a
   cheap handle over any `Arc<dyn Codec<T>>`. Payloads are bound as
   `edomata_serde::pg::PgPayload` (native `jsonb` / `json` / `bytea` wire
   format) and decoded from the raw column value, never through a `String`
   or `serde_json::Value`. `SqlxCodec::default()` is `SerdeCodec::jsonb()`
   for serde types, so `BackendBuilder::build_default()` gives the
   `jsonb`-by-default behaviour the plan requires.

4. **Transactions and error mapping follow Scala.** `append` / `save` run
   in one transaction (events or state, outbox rows, command record) and a
   PostgreSQL unique violation (SQLSTATE `23505`, from the `(stream,
   version)` constraint or the command primary key) becomes
   `BackendError::VersionConflict`; the CQRS upsert reports `0` affected
   rows as `VersionConflict` too. Other database errors are wrapped in
   `BackendError::UnknownError`. Row counts are asserted like Scala's
   `assertInserted`. Update signals (`Notifications`) fire after commit.

5. **DDL runs under an advisory lock.** Concurrent
   `CREATE TABLE IF NOT EXISTS` for the same table can fail in PostgreSQL
   with `23505` on `pg_type` or `42P07`; the shared test suites reproduced
   it immediately. Every DDL batch therefore runs in a transaction that
   first takes `pg_advisory_xact_lock(hashtext('edomata-ddl'))`, which also
   makes several service replicas starting at once safe. This goes beyond
   the Scala drivers.

6. **The CQRS notification hook is a connection-scoped closure.**
   `SqlxHandler<N>` receives the notifications and the transaction's
   `PgConnection`, replacing Scala's `SkunkHandler` / `DoobieHandler`; a
   failing handler rolls the whole save back.

7. **Migrations.** `SqlxMigrations::run` reproduces the Skunk/Doobie runner:
   create the tracking table (even with `skip_setup`, as in Scala), skip
   applied versions, and per pending migration rewrite `payload::text` →
   `$1::jsonb` in one transaction, record the version and truncate the
   snapshots.

## Tests

The shared suites run against PostgreSQL for schema, keyword (`order`) and
prefixed naming, for the three `compatibility_*` schemas of `testdata.sql`
(json, jsonb and hex-encoded bytea codecs, as in Scala) and for CQRS; the
`persistence` checks of one namespace are serialised with a lock because
MUnit ran them sequentially on a shared fixture. Additional tests pin
`skip_setup`, the Flyway workflow, prefixed catalog names, the transactional
handler, migrations and the duplicate-command race. CI loads `testdata.sql`
into the service database for the compatibility suites.
