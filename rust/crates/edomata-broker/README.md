# edomata-broker

Broker-agnostic distribution of outbox notifications and journal events: the `Publisher` trait, relays that mark items only after the broker acknowledged them (at-least-once, stable ids, per-stream ordering), PostgreSQL leader election and `LISTEN/NOTIFY` wake-ups. Pulls no broker client.

- **Scala module(s)**: *(new, no Scala equivalent)*
- **Book chapter**: [Distributing events with Kafka / RabbitMQ](../../book/src/backends/brokers.md)
- **API documentation**: `cargo doc -p edomata-broker --no-deps --open`

## Main items

- `BrokerMessage`, `MessageKind`, `headers`, `Publisher`, `PublishError`, `RecordingPublisher`
- `OutboxRelay`, `JournalRelay`, `CheckpointStore`, `MessageEncoder`, `RelayConfig`, `RetryPolicy`, `RelayMetrics`
- `postgres::{LeaderLock, listen, PgCheckpointStore}`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
