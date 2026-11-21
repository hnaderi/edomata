# edomata-sqlx

The PostgreSQL backend: event-sourcing and CQRS drivers over a `sqlx::PgPool`, journal and outbox readers, persisted snapshots, migrations runner, `skip_setup` for Flyway, optional `LISTEN/NOTIFY` wake-ups.

- **Scala module(s)**: `skunk`, `doobie`
- **Book chapter**: [PostgreSQL (sqlx)](../../book/src/backends/postgres.md)
- **API documentation**: [edomata_sqlx](https://beyond-scale-group.github.io/edomata/rust/api/edomata_sqlx/) (locally: `cargo doc -p edomata-sqlx --no-deps --open`)

## Main items

- `SqlxDriver`, `SqlxCqrsDriver` (`for_namespace`, `new`, `new_with`, `with_outbox_notify_channel`)
- `TxScope`, `SqlxDriver::with_tx_scope` / `with_snapshot_write_scope`: prepare every transaction of the event-sourcing driver (used for Row-Level Security by `edomata-saas-sqlx`; off by default, no change without it)
- `SqlxCodec<T>`, `SqlxHandler<N>`, `SqlxMigrations`
- `queries` and `shared` for derived drivers

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
