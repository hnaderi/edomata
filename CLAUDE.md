# CLAUDE.md

This file provides guidance to Claude Code when working with the Edomata codebase.

## Project Overview

Edomata is a lightweight, purely functional Scala 3 library for implementing event-driven automata. It provides abstractions for event sourcing and CQRS patterns, built on the Typelevel ecosystem (Cats, Cats Effect, FS2).

**Author**: Hossein Naderi
**License**: Apache 2.0
**Platforms**: JVM, Scala.js, Scala Native

## Build System

- **Build tool**: SBT 1.12.9
- **Scala version**: 3.3.7
- **Cross-compilation**: JVM, JS, Native platforms

### Common Commands

```bash
# Compile all modules
sbt compile

# Run all tests
sbt test

# Pre-commit checks (format, headers, compile, test)
sbt precommit

# Full release checklist (clean, format check, compile, test)
sbt commit

# Generate documentation
sbt docs/mdoc

# Format code
sbt scalafmtAll

# Start PostgreSQL for integration tests
docker-compose up -d
```

## Project Structure

```
modules/
├── core/              # Core abstractions (Decision, Edomaton, Stomaton)
├── backend/           # Event sourcing backend abstractions
├── postgres/          # PostgreSQL common components
├── skunk/             # Skunk-based PostgreSQL backend (cross-platform)
├── skunk-circe/       # Circe JSON codecs for Skunk
├── skunk-jsoniter/    # Jsoniter codecs for Skunk
├── skunk-upickle/     # uPickle codecs for Skunk
├── doobie/            # Doobie-based PostgreSQL backend (JVM only)
├── doobie-circe/      # Circe JSON codecs for Doobie
├── doobie-jsoniter/   # Jsoniter codecs for Doobie
├── doobie-upickle/    # uPickle codecs for Doobie
├── backend-tests/     # Integration tests
├── e2e/               # End-to-end tests
├── munit/             # MUnit test framework integration
├── saas/              # Multi-tenant SaaS abstractions
├── saas-skunk/        # Skunk-based SaaS backend
└── java-api/          # Java-friendly facade (JVM only, backed by Doobie)

examples/              # Example implementations
docs/                  # Documentation (Markdown)
website/               # Docusaurus documentation site
```

## Core Abstractions

| Type | Purpose |
|------|---------|
| `Decision[R, E, A]` | State machine with Accepted/Rejected/InDecisive outcomes |
| `Response[R, E, N, A]` | Decision combined with notification publishing (alias of `ResponseD`) |
| `Edomaton[F, Env, R, E, N, A]` | Event-driven automaton (full event sourcing) |
| `Stomaton[F, Env, S, R, E, A]` | State-only automaton (CQRS without event sourcing) |
| `DecisionT[F, R, E, A]` | Effectful decision transformer |
| `Action[F, R, E, N, A]` | Effect runner yielding responses |

## Key Dependencies

- **Cats** 2.10.0 - Functional programming abstractions
- **Cats Effect** 3.6.3 - IO and effect handling
- **FS2** 3.12.2 - Functional streams
- **Skunk** 0.6.5 - Async PostgreSQL client
- **Doobie** 1.0.0-RC12 - JDBC-based database layer
- **Circe** 0.14.15 - JSON serialization
- **jsoniter-scala** 2.30.1 - High-performance JSON serialization
- **uPickle** 3.2.0 - JSON / MessagePack serialization
- **MUnit** 1.0.0-M8 - Testing framework
- **ScalaCheck** 1.15.4 - Property-based testing

## Testing

Tests require a PostgreSQL database. Use Docker Compose to start one:

```bash
docker-compose up -d
```

This starts PostgreSQL 14 on port 5432 with:
- User: `postgres`
- Password: `postgres`
- Databases created by init scripts: `postgres` (default), `skunk_jvmplatform`, `skunk_jsplatform`, `skunk_nativeplatform`, `doobie` (unquoted in `testdbs.sql`, so PostgreSQL folds them to lowercase)
- Fixture schemas loaded into `postgres` by `testdata.sql`: `compatibility_json`, `compatibility_jsonb`, `compatibility_binary` (read by the Scala and Rust compatibility suites)

Run tests with:

```bash
sbt test                    # All tests
sbt coreJVM/test           # Core module only (JVM)
sbt skunkBackendJVM/test   # Skunk backend tests
sbt doobieBackendJVM/test  # Doobie backend tests
sbt e2eTestsJVM/test       # End-to-end tests
```

## Code Style

- **Formatter**: Scalafmt 3.10.7
- **Config**: `.scalafmt.conf`
- **Dialect**: Scala 3

Run formatter before committing:

```bash
sbt scalafmtAll
```

## Development Environment

The project uses Nix/Direnv for environment management. If you have Nix installed:

```bash
direnv allow
```

This provides JDK and SBT automatically.

## Module Dependencies

```
core
 ├── munit
 └── backend
      └── postgres
           ├── saas
           ├── skunk (cross-platform)
           │    ├── skunk-circe
           │    ├── skunk-jsoniter
           │    ├── skunk-upickle
           │    └── saas-skunk (+ saas)
           └── doobie (JVM only)
                ├── doobie-circe
                ├── doobie-jsoniter
                ├── doobie-upickle
                └── java-api (JVM only)

backend-tests and e2e are test-only modules built on the drivers.
```

## PostgreSQL Naming & DDL

### Table Naming Strategies (`PGNaming`)

Edomata backends use `PGNaming` (in `modules/postgres/`) to control how PostgreSQL tables are named. There are two strategies:

| Strategy | Factory | Table example | Schema creation | Use case |
|----------|---------|---------------|-----------------|----------|
| **Schema** (default) | `PGNaming.schema("auth")` | `"auth".journal` | Yes (`CREATE SCHEMA`) | Isolated namespaces per aggregate |
| **Prefixed** | `PGNaming.prefixed("auth")` | `auth_journal` | No | Flyway/single-schema projects |

Convenience: `PGNamespace.prefixed("auth")` is shorthand for `PGNaming.Prefixed(PGNamespace("auth"))`.

In prefixed mode, constraint and index names are also prefixed (e.g. `auth_journal_pk`) to avoid collisions between aggregates sharing the same schema.

**Key files:**
- `modules/postgres/src/main/scala/PGNaming.scala` — sealed trait with `Schema` and `Prefixed` case classes
- `modules/postgres/src/main/scala/PGNamespace.scala` — opaque type for validated PG identifiers
- `modules/postgres/src/main/scala/PGSchema.scala` — DDL extraction utility

### DDL Extraction (`PGSchema`)

`PGSchema` generates plain SQL DDL strings for migration tools (Flyway, Liquibase, etc.):

```scala
import edomata.backend.{PGNaming, PGSchema}

// Event sourcing tables: journal, outbox, commands, snapshots, migrations
PGSchema.eventsourcing(PGNaming.prefixed("accounts"), eventType = "jsonb")

// CQRS tables: states, outbox, commands
PGSchema.cqrs(PGNaming.prefixed("accounts"), stateType = "jsonb")
```

Returns `List[String]` — each element is a standalone SQL statement (`CREATE SCHEMA IF NOT EXISTS`
first in schema mode only, then CREATE TABLE, CREATE INDEX).
Payload type parameters are plain SQL type names spliced into the DDL without validation; the
intended values are `"json"`, `"jsonb"` (default), or `"bytea"`.

### Disabling Automatic Setup (`skipSetup`)

The `from(naming: PGNaming, ...)` overload of every driver accepts `skipSetup: Boolean = false`
(the `from(namespace: PGNamespace, ...)` overload and `apply` don't):

```scala
SkunkDriver.from(naming, pool, skipSetup = true)    // no DDL at all
DoobieDriver.from(naming, trx, skipSetup = true)
SkunkCQRSDriver.from(naming, pool, skipSetup = true)
DoobieCQRSDriver.from(naming, trx, skipSetup = true)
SaaSSkunkCQRSDriver.from(naming, pool, skipSetup = true)
```

When `skipSetup = true`:
- No `CREATE SCHEMA` is executed
- No `CREATE TABLE` / `CREATE INDEX` is executed by the driver
- The driver assumes tables already exist (created by Flyway or manually)
- `SkunkMigrations.run` / `DoobieMigrations.run` are unaffected: they always run
  `CREATE TABLE IF NOT EXISTS` for the `migrations` table

### Typical Flyway Workflow

1. Generate DDL: `PGSchema.eventsourcing(PGNaming.prefixed("myapp")).foreach(println)`
2. Paste output into `src/main/resources/db/migration/V1__create_myapp_tables.sql`
3. Wire driver with `skipSetup = true`: `SkunkDriver.from(PGNaming.prefixed("myapp"), pool, skipSetup = true)`

### Where Naming Flows Through

`PGNaming` is passed through:
1. **Driver factories** (`SkunkDriver.from`, `DoobieDriver.from`, etc.) — schema setup
2. **Driver instances** (private field) — passed to Queries and SnapshotPersistence
3. **Queries classes** (`modules/skunk/src/main/scala/Queries.scala`, `modules/doobie/src/main/scala/Queries.scala`) — table refs, constraint names, index names in SQL
4. **SnapshotPersistence** — snapshot table setup
5. **Migration runners** (`SkunkMigrations.run`, `DoobieMigrations.run`) — the `migrations` table DDL lives there, not in `Queries.scala`

When modifying SQL generation, update both skunk and doobie Queries files. The skunk variant uses `sql"""#$interpolation"""` for literal SQL fragments; the doobie variant uses `Fragment.const(...)`.

### Adding a New Table

If a new table is needed:
1. Add a `PGNaming` method call in the new `Queries` class (both skunk and doobie; migration-related tables live in the `*Migrations` files)
2. Add the DDL to `PGSchema` (private helper method + include in `eventsourcing`/`cqrs`)
3. Add tests in `PGNamespaceSuite.scala` (PGSchemaSuite section)
4. Prefix constraint/index names using `naming.constraint()` / `naming.index()`

## Rust Port (`rust/`)

A Cargo workspace under `rust/` ports the library to Rust, milestone by milestone, following
`docs/plans/rust-port.md` (the source of truth for scope and quality bar).

- **Crates**: `rust/crates/*` (`edomata-core` first; see `rust/README.md` for the full list)
- **Porting map**: `rust/PORTING.md` maps every Scala module and test suite to its Rust counterpart
- **ADRs**: `rust/docs/adr/` records design decisions (effects/futures, `chrono`, `Edomaton` shape, `RaiseError`, backend abstractions, PostgreSQL naming and golden DDL, codecs and the `jsonb` wire format, the sqlx driver, the test kit and SaaS crates, the simple facade, the e2e/examples/cross-language layout, broker distribution)
- **MSRV**: 1.88 (edition 2024); every crate has `#![forbid(unsafe_code)]`
- **CI**: `.github/workflows/rust.yml` (fmt, clippy, doc, tests with PostgreSQL plus a JDK and `sbt` for the cross-language test and Docker for the testcontainers broker tests, MSRV, wasm32 build of `edomata-core`, no-JVM-dependency and no-broker-client guards)

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build -p edomata-core --all-features --target wasm32-unknown-unknown
```

Do not modify the Scala modules for the port, except to add golden-DDL tooling and the Scala
side of the cross-language compatibility test.

- **Golden DDL**: `rust/tests/golden/*.sql` is generated from the Scala `PGSchema` by
  `modules/postgres/src/test/scala/GoldenDDL.scala` and asserted byte-for-byte by
  `rust/crates/edomata-postgres/tests/golden.rs`. Regenerate after changing `PGSchema.scala`:

```bash
sbt "postgresJVM/Test/runMain edomata.backend.GoldenDDL rust/tests/golden"
```

- **Golden SaaS DDL**: `rust/tests/golden/saas_cqrs_*.sql` is generated from the Scala `SaaSPGSchema` by
  `modules/saas/src/test/scala/edomata/saas/GoldenSaaSDDL.scala` and asserted by
  `rust/crates/edomata-saas/tests/schema.rs`. Regenerate after changing `SaaSPGSchema.scala`:

```bash
sbt "saasJVM/Test/runMain edomata.saas.GoldenSaaSDDL rust/tests/golden"
```

- **Golden payloads**: `rust/tests/golden/payloads/` holds the JSON/MessagePack bytes written by the
  Circe, jsoniter and uPickle codecs for the same sample events, generated by
  `examples/src/test/scala/GoldenPayloads.scala` and asserted by
  `rust/crates/edomata-serde/tests/golden_payloads.rs`:

```bash
sbt "examplesJVM/Test/runMain golden.GoldenPayloads rust/tests/golden/payloads"
```

- **Cross-language test**: `rust/crates/edomata-e2e/tests/cross_language.rs` runs the Scala side
  (`modules/e2e/src/test/scala/CrossLanguage.scala`, a test-scope main) through `sbt` (`EDOMATA_SBT`
  overrides the binary), so `cargo test --workspace` needs a JDK and `sbt`. Run the Scala phases by hand
  with:

```bash
sbt "e2eTestsJVM/Test/runMain crosslang.CrossLanguage write cross_language"
sbt "e2eTestsJVM/Test/runMain crosslang.CrossLanguage verify cross_language"
```

- **Examples**: `rust/examples/src/bin/*.rs` (`cargo run -p edomata-examples --bin <name>`), one per Scala
  example, plus `kafka_relay` / `rabbitmq_relay` behind the `kafka` / `rabbitmq` features.

- **Broker tests**: `edomata-kafka` and `edomata-rabbitmq` start Kafka and RabbitMQ with testcontainers, so
  `cargo test --workspace` needs Docker (their leader-election tests also use the docker-compose PostgreSQL).

- **Database for Rust tests**: the SQL tests read `DATABASE_URL` (default
  `postgres://postgres:postgres@localhost:5432/postgres`, the docker-compose instance). If a local
  PostgreSQL also listens on `localhost:5432`, set `DATABASE_URL` to the container through the
  machine's LAN address.

## CI/CD

GitHub Actions runs on:
- JVM versions: temurin@8, temurin@17
- Platforms: JVM, JS, Native
- Checks: format, compile, test

## Language

All contributions MUST be in English: commit messages, PR titles/descriptions, code comments, documentation, and branch names. This is an international open-source library.

## Contribution Workflow

When given a ticket (number or URL):

1. Detect the repository context:
   ```bash
   # Check if this is a fork
   gh repo view --json isFork,parent -q '{fork: .isFork, upstream: .parent.owner.login + "/" + .parent.name}'
   ```
2. Read the ticket content:
   - If on a **fork**: read the ticket from the upstream repo (`gh issue view --repo <upstream> <number>`)
   - If on the **root repo**: read the ticket directly (`gh issue view <number>`)
3. Implement the solution on a dedicated branch
4. **Run documentation audit** (mandatory before opening a PR — see below)
5. Open a PR:
   - If on a **fork**: target the **upstream** repo with `Fixes <upstream>#<number>`
   - If on the **root repo**: target the default branch with `Fixes #<number>`

Do not ask for confirmation — go straight to reading the ticket and implementing.

## Documentation Audit (Pre-PR)

**This audit MUST run before every PR creation.** Launch a background Explore agent with the prompt below. If the agent finds discrepancies, fix them in a dedicated `docs:` commit before opening the PR.

### Audit agent prompt

> Audit the documentation in this project against the actual source code. Check every item below and report ALL discrepancies with exact file paths and line numbers.
>
> **1. CLAUDE.md accuracy**
> - Verify SBT version matches `project/build.properties`
> - Verify every dependency version matches `project/Dependencies.scala`
> - Verify docker-compose credentials match `docker-compose.yml`
> - Verify the project structure listing matches actual `modules/` directories
> - Verify all code examples and API signatures in the PostgreSQL Naming & DDL section match the actual source in `modules/postgres/src/main/scala/`
> - Verify driver method signatures (`from`, `apply`, `skipSetup`) match skunk and doobie driver source files
>
> **2. docs/backends/skunk.md and docs/backends/doobie.md**
> - Verify all code examples use existing classes and methods (e.g. no `SkunkBackend`; note that `BackendBuilder.withSnapshot(Resource[F, SnapshotStore[F, S]])` does exist, but `.withSnapshot()` with no argument does not)
> - Verify import paths are correct
> - Verify `PGSchema` and `skipSetup` examples match actual method signatures
>
> **3. docs/tutorials/backends.md**
> - Verify the minimal example uses the actual Backend builder API
> - Verify connection parameters match docker-compose defaults
> - Verify table descriptions match actual SQL in Queries.scala
>
> **4. docs/about/features.md and docs/introduction.md**
> - Flag any claims about supported features that don't match the code
>
> **5. Cross-reference**
> - For every public method mentioned in any doc, grep the codebase to confirm it exists
> - For every file path mentioned in CLAUDE.md, verify the file exists
>
> **6. Rust workspace (`rust/`)**
> - Verify the "Rust Port" section of CLAUDE.md: every path exists and every command is valid for the workspace
> - `rust/README.md`: crate table statuses, commands and MSRV match `rust/Cargo.toml`, `rust/crates/*` and `.github/workflows/rust.yml`
> - `rust/PORTING.md`: every Rust path exists under `rust/crates/`, every Scala path exists under `modules/`, and every Rust type or function named in the type-mapping tables exists in the Rust sources
> - `rust/docs/adr/*.md`: types, functions and modules they mention exist in the Rust sources
> - Crate-level docs (`src/lib.rs` of each crate): every intra-doc link target exists and is exported
>
> Report findings as a list: `[file:line] issue description — expected X, found Y`
