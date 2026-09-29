---
sidebar_position: 1
title: "Rust port"
---

# Edomata for Rust

Edomata also exists as a **Rust library**: a port of the Scala library that lives in the
[`rust/`](https://github.com/beyond-scale-group/edomata/tree/main/rust) Cargo workspace of the
same repository. It implements the same event-driven automata, event-sourced aggregates
(`Edomaton`) and CQRS state machines (`Stomaton`), on the same PostgreSQL tables.

The port keeps the **semantics, invariants and on-disk formats** of the Scala library, so Rust
and Scala services can share a database. It does not emulate Cats: each abstraction is mapped to
its idiomatic Rust equivalent.

| Scala | Rust |
|-------|------|
| effects in `F[_]` (Cats Effect) | `async` functions and `Future`s, run on Tokio by the backends |
| `Decision`, `ResponseD`, `Edomaton`, `Stomaton` | the same types in `edomata-core` (`ResponseD` is an alias of `ResponseT`), with `and_then` / `map` instead of `flatMap` / `map` |
| `DomainModel` and its DSL | the `DomainModel` trait and `model.dsl::<Command, Notification>()` |
| Skunk and Doobie drivers | one sqlx driver, `edomata-sqlx` |
| Circe, jsoniter-scala and uPickle codecs | `serde` through `edomata-serde` |
| `java-api` | `edomata-simple`, a closure-based facade |

The Rust port also adds broker distribution: an outbox relay that publishes notifications to
Kafka or RabbitMQ (`edomata-broker`, `edomata-kafka`, `edomata-rabbitmq`).

## Status and requirements

- 12 library crates and 2 test-only crates. 11 of them port a Scala module, and 3 (the broker
  crates) are new (see [Crates](crates.md)).
- Minimum supported Rust version: **1.88** (edition 2024). Every crate is `#![forbid(unsafe_code)]`.
- The backends run on Tokio and PostgreSQL through sqlx. `edomata-core` has no runtime
  dependency and also builds for `wasm32-unknown-unknown`.
- No crate depends on a JVM, Scala or Java artifact.
- The crates are not published on crates.io yet: depend on them through git (see
  [Installation](installation.md)).

## In this section

- [Crates](crates.md): how the Rust crates map to the Scala modules.
- [Installation](installation.md): `Cargo.toml` snippets for the main crates and their features.
- [Quickstart](quickstart.md): a bank account aggregate, run on PostgreSQL and unit-tested.
- [Wire compatibility](compatibility.md): sharing a PostgreSQL database between Scala and Rust
  services.

## Further reading

- [The Edomata Rust book](https://beyond-scale-group.github.io/edomata/rust/book/): tutorials,
  principles, the PostgreSQL backend, the Simple API, broker distribution and the design
  decisions (ADRs).
- [API documentation](https://beyond-scale-group.github.io/edomata/rust/api/) (rustdoc), for
  example [`edomata_core`](https://beyond-scale-group.github.io/edomata/rust/api/edomata_core/).
- [Migration guide for Scala and Java users](https://beyond-scale-group.github.io/edomata/rust/book/other/migration-guide.html).
- [Porting map](https://beyond-scale-group.github.io/edomata/rust/book/other/porting.html):
  every Scala module and test suite with its Rust counterpart.
- [`rust/README.md`](https://github.com/beyond-scale-group/edomata/blob/main/rust/README.md) on
  GitHub, with the development commands.
