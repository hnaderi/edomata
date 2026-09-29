# edomata-serde

Serde-based payload codecs for `jsonb` (default), `json` and `bytea` columns, with the sqlx wire types that write PostgreSQL's native `jsonb` format directly. Reads the payloads written by the Scala Circe, jsoniter and uPickle JSON codecs.

- **Scala module(s)**: `skunk-circe`, `skunk-jsoniter`, `skunk-upickle`, `doobie-circe`, `doobie-jsoniter`, `doobie-upickle`
- **Book chapter**: [PostgreSQL (sqlx)](../../book/src/backends/postgres.md)
- **API documentation**: [edomata_serde](https://beyond-scale-group.github.io/edomata/rust/api/edomata_serde/) (locally: `cargo doc -p edomata-serde --no-deps --open`)

## Main items

- `SerdeCodec<T>` (`jsonb()`, `json()`, `bytea()`)
- `pg::{JsonbPayload, JsonPayload, ByteaPayload, PgPayload}`
- `compat::upickle_option`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
