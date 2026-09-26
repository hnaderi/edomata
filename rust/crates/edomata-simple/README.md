# edomata-simple

A closure-based facade for applications that do not want to touch the generic core types: plain-data decisions, closure models and handlers, a backend builder over `edomata-sqlx`, blocking and asynchronous entry points, and a string-based DDL helper.

- **Scala module(s)**: `java-api`
- **Book chapter**: [Simple API](../../book/src/backends/simple-api.md)
- **API documentation**: `cargo doc -p edomata-simple --no-deps --open`

## Main items

- `SimpleDecision`, `AppResult`, `SimpleDomainModel`, `ClosureModel`, `CommandHandler`, `Context`
- `SimpleCodec`, `ClosureCodec`, `serde_codec`
- `SimpleBackend::builder`, `SimpleBackend`, `SimpleJournal`, `SimpleOutbox`, `SimpleRuntime`, `BlockingBackend`
- `SimplePGSchema`, `SimpleError`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
