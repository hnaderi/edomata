# edomata-postgres

PostgreSQL naming and DDL, shared by every driver: schema or prefixed naming, validated namespaces, DDL extraction for migration tools (byte-for-byte identical to the Scala output, golden-tested), and event migrations.

- **Scala module(s)**: `postgres`
- **Book chapter**: [PostgreSQL (sqlx)](../../book/src/backends/postgres.md)
- **API documentation**: `cargo doc -p edomata-postgres --no-deps --open`

## Main items

- `PGNaming`, `PGNamespace`, `PGSchema` (`eventsourcing`, `cqrs`, `relay_checkpoints`), `ddl::*`
- `EventMigration`, `MigrationResult`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
