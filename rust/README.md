# Edomata for Rust

A Rust port of [Edomata](../README.md), a lightweight library for building
event-driven automata: event-sourced aggregates (`Edomaton`) and CQRS state
machines (`Stomaton`) on PostgreSQL.

The port keeps the **semantics, invariants and on-disk formats** of the Scala
library, so Rust and Scala services can share a database. It does not emulate
Cats: each abstraction is mapped to its idiomatic Rust equivalent (see
[`PORTING.md`](PORTING.md) and the ADRs in [`docs/adr/`](docs/adr/)).

## Workspace

| Crate | Scala module(s) | Status |
|-------|-----------------|--------|
| [`edomata-core`](crates/edomata-core) | `core` | available |
| `edomata-backend` | `backend` | planned |
| `edomata-postgres` | `postgres` | planned |
| `edomata-serde` | `*-circe`, `*-jsoniter`, `*-upickle` | planned |
| `edomata-sqlx` | `skunk`, `doobie` | planned |
| `edomata-testkit` | `munit` | planned |
| `edomata-saas`, `edomata-saas-sqlx` | `saas`, `saas-skunk` | planned |
| `edomata-simple` | `java-api` | planned |
| `edomata-broker`, `edomata-kafka`, `edomata-rabbitmq` | *(new)* | planned |

The full roadmap is in [`docs/plans/rust-port.md`](../docs/plans/rust-port.md).

## Quick start

```toml
[dependencies]
edomata-core = { path = "rust/crates/edomata-core" }
```

```rust
use edomata_core::*;

struct Counter;

impl DomainModel for Counter {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 { 0 }

    fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<String>> {
        if *event > 0 { Ok(state + event) } else { Err(NonEmpty::new("negative".into())) }
    }
}

let dsl = Counter.dsl::<i32, String>();
let app = dsl.router(move |by| {
    if by == 0 { dsl.reject("zero".to_string()) } else { dsl.accept(by).publish(["changed".to_string()]) }
});
```

`app` is an `Edomaton`: a reusable, asynchronous program that reads a
`RequestContext`, accepts events or rejects, and publishes notifications.
`app.execute(&Counter, ctx).await` folds its decision into the model and
yields an `EdomatonResult`.

## Development

```bash
cd rust
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo build -p edomata-core --target wasm32-unknown-unknown
```

The minimum supported Rust version is **1.85** (edition 2024) and is checked
in CI. Every crate has `#![forbid(unsafe_code)]`.

Integration tests (from milestone 5 on) use the PostgreSQL instance started
by the repository's `docker-compose.yml`.
