# edomata-backend-tests

The shared storage test suites (compatibility, persistence, snapshots, CQRS) as a library crate, run against the in-memory driver here and against PostgreSQL by `edomata-sqlx`. Test-only.

- **Scala module(s)**: `backend-tests`
- **Book chapter**: [Running](../../book/src/tutorials/backends.md)
- **API documentation**: `cargo doc -p edomata-backend-tests --no-deps --open`

## Main items

- `eventsourcing::*` and `cqrs::*` suites, `TestDomain`, `TestCqrsModel`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
