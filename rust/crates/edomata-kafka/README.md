# edomata-kafka

Kafka publisher for `edomata-broker` relays, over `rdkafka`: idempotent producer, stream id as partition key, message id and metadata as headers. Integration tests use testcontainers (Docker required).

- **Scala module(s)**: *(new)*
- **Book chapter**: [Distributing events with Kafka / RabbitMQ](../../book/src/backends/brokers.md)
- **API documentation**: `cargo doc -p edomata-kafka --no-deps --open`

## Main items

- `KafkaPublisher`, `KafkaPublisherBuilder`, `default_topic`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
