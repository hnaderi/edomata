# edomata-saas-sqlx

The tenant-aware PostgreSQL CQRS driver: fills the `tenant_id` / `owner_id` columns so that Row-Level Security and tenant-scoped indexes work.

- **Scala module(s)**: `saas-skunk`
- **Book chapter**: [SaaS multi-tenant module](../../book/src/tutorials/saas.md)
- **API documentation**: `cargo doc -p edomata-saas-sqlx --no-deps --open`

## Main items

- `SaaSSqlxCqrsDriver`, `SaaSCodec<T>`, `TenantStateLister`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
