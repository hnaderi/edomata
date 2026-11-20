# edomata-pulsar

Apache Pulsar publisher for `edomata-broker` relays, over the `pulsar` client: one producer per topic (cached, dropped when it fails), stream id as message key, message id and metadata as message properties, token authentication and TLS, `from_clever_env()` for the Clever Cloud add-on. Integration tests use testcontainers (Docker required).

- **Scala module(s)**: *(new, no Scala counterpart)*
- **Book chapter**: [Distributing events with Kafka / RabbitMQ / Pulsar](../../book/src/backends/brokers.md)
- **API documentation**: [edomata_pulsar](https://beyond-scale-group.github.io/edomata/rust/api/edomata_pulsar/) (locally: `cargo doc -p edomata-pulsar --no-deps --open`)

## Main items

- `PulsarPublisher`, `PulsarPublisherBuilder`, `BuildError`
- `default_topic`, `sanitize_topic`, `resolve_topic`, `InvalidTopic`

## Building

The `pulsar` client compiles its protocol definitions with `prost-build`, which needs the `protoc` protobuf compiler on the build machine (`apt install protobuf-compiler`, `brew install protobuf`). TLS uses rustls; no system OpenSSL is needed.

## Status

Unit tests (topic resolution, sanitizing, property mapping, error classification, builder, Clever environment) run without a broker. The integration tests in `tests/pulsar.rs` need Docker. Token authentication, TLS and the Clever Cloud add-on have not been exercised against a real Pulsar.

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
