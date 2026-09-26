# edomata-saas

Multi-tenant CRUD abstractions: tenancy types, pluggable authorisation policies, guards, guarded DSLs and services for event sourcing and CQRS, tenant-scoped read abstractions, and tenant-aware DDL with optional Row-Level Security.

- **Scala module(s)**: `saas`
- **Book chapter**: [SaaS multi-tenant module](../../book/src/tutorials/saas.md)
- **API documentation**: `cargo doc -p edomata-saas --no-deps --open`

## Main items

- `TenantId`, `UserId`, `CallerIdentity`, `SaaSCommand`, `CrudAction`, `CrudState`
- `AuthPolicy`, `PermissivePolicy`, `RoleBasedPolicy`, `SaaSGuard`
- `SaaSDomainDsl`, `SaaSCqrsDsl`, `SaaSEventSourcedService`, `SaaSCqrsService`
- `TenantAwareReader`, `TenantScopedQuery`, `UnsafeCrossTenantQuery`, `TenantExtractor`
- `SaaSPGSchema`, `RlsConfig`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
