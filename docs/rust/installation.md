---
sidebar_position: 3
title: "Installation"
---

# Installation

The Rust crates are not published on crates.io yet. Depend on them through git, pinned to a
release tag of the repository: every tag from `v0.12.34` on contains the complete port. Cargo
finds each crate by name inside the repository's `rust/` workspace.

You need Rust **1.88** or later (edition 2024).

## Domain logic only

`edomata-core` holds the domain abstractions (`Decision`, `Edomaton`, `Stomaton`, the DSLs). It
has no runtime dependency and builds for `wasm32-unknown-unknown`.

```toml
[dependencies]
edomata-core = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

Its only feature, `serde` (off by default), derives `Serialize` and `Deserialize` for the message
types (`CommandMessage`, `MessageMetadata`) and for `NonEmpty`:

```toml
edomata-core = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34", features = ["serde"] }
```

## Event sourcing or CQRS on PostgreSQL

`edomata-backend` compiles programs into services, and `edomata-sqlx` provides the PostgreSQL
drivers (it re-exports `PgPool`, `PGNaming`, `PGNamespace` and `PGSchema`). Payloads are encoded
with serde, as `jsonb` by default.

```toml
[dependencies]
edomata-core = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
edomata-backend = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
edomata-sqlx = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
chrono = { version = "0.4", features = ["clock"] }
```

Add `edomata-serde` when you choose a payload format other than the default (`json` or `bytea`)
or use its compatibility helpers. Its `sqlx` feature, on by default, provides the PostgreSQL wire
types; turn it off with `default-features = false` to use `SerdeCodec` without sqlx.

```toml
edomata-serde = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

## Testing

`edomata-testkit` provides `TestCommand` and assertion helpers for unit tests of edomatons and
stomatons, without a database:

```toml
[dev-dependencies]
edomata-testkit = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

## Multi-tenant SaaS

```toml
edomata-saas = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
edomata-saas-sqlx = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

`edomata-saas` has a `serde` feature, on by default, for `TenantId`, `UserId`, `CrudAction`,
`CrudState`, `SaaSCommand` and `CallerIdentity`.

## Simple API

`edomata-simple` is the counterpart of the Scala `java-api` module: a closure-based facade over
the PostgreSQL backend, with a blocking runtime for applications that are not `async`.

```toml
edomata-simple = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

## Broker distribution

Publishing the outbox to a broker is opt-in. `edomata-broker` provides the relays, and one
publisher crate per broker brings its client library:

```toml
edomata-broker = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
# Kafka, through rdkafka (builds librdkafka, so it needs a C toolchain)
edomata-kafka = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
# RabbitMQ, through lapin
edomata-rabbitmq = { git = "https://github.com/beyond-scale-group/edomata", tag = "v0.12.34" }
```

## Next

- [Quickstart](quickstart.md)
- [Getting started in the Rust book](https://beyond-scale-group.github.io/edomata/rust/book/tutorials/getting-started.html)
