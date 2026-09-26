# Architecture decision records

The non-obvious decisions of the port are recorded as ADRs under `rust/docs/adr/` and reproduced here.

| ADR | Decision |
|-----|----------|
| [0001](adr-0001.md) | Effects and the async runtime: `std::future`, Tokio, no `F[_]` |
| [0002](adr-0002.md) | Timestamps with `chrono` |
| [0003](adr-0003.md) | How `Edomaton` is represented |
| [0004](adr-0004.md) | `ResponseT` and the `RaiseError` trait |
| [0005](adr-0005.md) | Backend abstractions, `Signal` over `Notify`, no error channel in programs |
| [0006](adr-0006.md) | PostgreSQL naming and golden DDL |
| [0007](adr-0007.md) | Codecs and the `jsonb` wire format |
| [0008](adr-0008.md) | The sqlx driver (query catalogues, advisory lock on DDL) |
| [0009](adr-0009.md) | Test kit and SaaS crates (policies as values) |
| [0010](adr-0010.md) | The simple facade |
| [0011](adr-0011.md) | E2E tests, examples and the cross-language test |
| [0012](adr-0012.md) | Broker distribution |
| [0013](adr-0013.md) | Documentation: this book and its compiled samples |
