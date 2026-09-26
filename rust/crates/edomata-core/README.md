# edomata-core

Core abstractions of Edomata: decisions, event-driven automata (`Edomaton`), state-driven automata (`Stomaton`), domain models and their DSLs. No runtime dependency; builds for `wasm32-unknown-unknown`.

- **Scala module(s)**: `core`
- **Book chapter**: [Event sourcing](../../book/src/tutorials/eventsourcing.md)
- **API documentation**: `cargo doc -p edomata-core --no-deps --open`

## Main items

- `Decision<R, E, A>`, `DecisionT`, `ResponseD` / `ResponseE`, `Action`
- `Edomaton<Env, R, E, N, A>`, `Stomaton<Env, S, R, N, A>`
- `DomainModel`, `CqrsModel`, `DomainDsl`, `CqrsDomainDsl`, `DomainCompiler`, `EdomatonResult`
- `CommandMessage`, `MessageMetadata`, `RequestContext`, `NonEmpty`
- `edomata_core::syntax`: extension methods (`accept`, `reject`, `into_decision`, ...)

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
