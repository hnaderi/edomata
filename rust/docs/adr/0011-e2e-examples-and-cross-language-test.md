# ADR 0011: End-to-end tests, examples and the cross-language test

- Status: accepted
- Date: 2026-09-25
- Milestone: 8 (E2E, examples, cross-language compatibility test)

## Context

The Scala `e2e` module runs a bank-account service against two backends
sharing one PostgreSQL namespace (Skunk and Doobie variants); `examples/`
holds five runnable programs; and the plan's wire-compatibility section
requires a cross-language test where Scala writes, Rust reads and appends,
and Scala reads the result, using the Scala build as an external oracle.

## Decisions

1. **`edomata-e2e` is a test-only crate with a shared domain.** The
   account domain (`Account`, `Event`, `Rejection`, `Command`,
   `Notification`, `AccountModel`, `account_service`) lives in `src/lib.rs`
   so that both the e2e suite and the cross-language test use it. Amounts
   are `rust_decimal::Decimal` (Scala's `BigDecimal`), serialized as JSON
   numbers with `rust_decimal::serde::float` so that Circe reads them and
   Circe's integers (`100`) are read back. Parameterless enum cases are
   struct variants (`Opened {}`) because Circe encodes Scala 3 singleton
   cases as `{"Opened":{}}`; field names follow Circe's camelCase
   (`accountId`). `tests/e2e.rs` pins these shapes.

2. **One e2e suite instead of two.** Scala runs the suite once per driver;
   the single `edomata-sqlx` driver runs it once, in the `sqlx_e2e`
   schema. The "distributed workload doesn't work with default cache" case
   asserts the `MaxRetryExceeded` conflict explicitly where Scala only
   `attempt`ed it.

3. **The cross-language test is a Cargo test that drives `sbt`.** The
   Scala side is a test-scope main in the `e2e` module
   (`modules/e2e/src/test/scala/CrossLanguage.scala`) with two phases:
   `write` (open, deposit 100, deposit 50; the snapshot is flushed on
   shutdown) and `verify` (state, journal, outbox, commands and the
   snapshot written by Rust). The Rust test drops the `cross_language`
   schema, runs the Scala `write` phase through `sbt -batch`
   (`EDOMATA_SBT` overrides the binary), reads the snapshot, state,
   journal and outbox Scala wrote, checks that Scala's command ids are
   redundant, appends a deposit and a withdrawal, closes the backend to
   flush its snapshot, and runs the Scala `verify` phase. The connection
   of `DATABASE_URL` is passed to Scala as the libpq environment variables
   (`PGHOST`, ...), so both sides always hit the same server. Requiring
   `sbt` and a JDK for `cargo test --workspace` is the price of a real
   oracle; the CI test job installs both. This is the only place the port
   touches the JVM, and only at test time.

4. **Examples are binaries of one package.** `rust/examples` is a
   workspace member (`edomata-examples`, not published) with one binary per
   Scala example under `src/bin/`, so `cargo build --all-targets` and
   clippy check them and `cargo run -p edomata-examples --bin <name>` runs
   them against the docker-compose PostgreSQL. Four deliberate differences:
   the counter example's transition really counts (the Scala one leaves the
   state unchanged); the migration example seeds a V1 journal and reads it
   back as V3 after migrating, where the Scala one stops after running the
   migrations (it uses persisted snapshots because the migration runner
   truncates the snapshots table, in Scala as in Rust, and a fresh
   namespace per run since migrations apply once per namespace); the SaaS todo
   example runs its tenant-scoped read queries for real (Scala's raise
   `NotImplementedError`); and the product catalogue example replaces the
   unimplemented `ProductQueries` with `TenantStateLister::list_by_tenant`
   on the tenant-aware driver. The Kafka and RabbitMQ examples were added
   by milestone 9 (ADR 0012).

## Tests

`tests/e2e.rs` ports the three e2e cases plus checks of the domain
decisions and of the Circe-compatible JSON shapes; `tests/cross_language.rs`
is the compatibility test (and a unit test of the URL → libpq mapping).
Every example was run against PostgreSQL.
