# edomata-testkit

Assertion helpers for domain tests, usable from any test runner: run a program on a command and a state, and expect a new state, notifications or rejections.

- **Scala module(s)**: `munit`
- **Book chapter**: [Event sourcing](../../book/src/tutorials/eventsourcing.md)
- **API documentation**: `cargo doc -p edomata-testkit --no-deps --open`

## Main items

- `TestCommand`
- `EdomatonAssertions` (`run_with`, `expect`, `expect_all`, `expect_rejection`, `expect_rejection_with`, `expect_that`, ...)
- `StomatonAssertions`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
