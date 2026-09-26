# edomata-rabbitmq

RabbitMQ publisher for `edomata-broker` relays, over `lapin`: publisher confirms awaited per message, persistent messages, exchange and routing key per message, reconnection. Integration tests use testcontainers (Docker required).

- **Scala module(s)**: *(new)*
- **Book chapter**: [Distributing events with Kafka / RabbitMQ](../../book/src/backends/brokers.md)
- **API documentation**: `cargo doc -p edomata-rabbitmq --no-deps --open`

## Main items

- `RabbitMqPublisher`, `default_exchange`, `default_routing_key`

Part of the [Edomata Rust port](../../README.md); see [`PORTING.md`](../../PORTING.md) for the mapping to the Scala library and [`docs/adr/`](../../docs/adr/) for the design decisions.
