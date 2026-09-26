# edomata-e2e

End-to-end tests on PostgreSQL and the Scala/Rust cross-language compatibility test (Scala writes, Rust reads and appends, Scala verifies). Test-only; the cross-language test needs `sbt` and a JDK.

- **Scala module(s)**: `e2e`
- **Book chapter**: [Migration guide](../../book/src/other/migration-guide.md)
- **API documentation**: `cargo doc -p edomata-e2e --no-deps --open`

## Main items

- the bank-account domain with Circe-compatible JSON (`src/lib.rs`)
- `tests/e2e.rs`, `tests/cross_language.rs`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
