# Distributing events with Kafka / RabbitMQ / Pulsar

Broker distribution is a Rust addition without Scala equivalent, and it is **opt-in**: `edomata-broker` is broker-agnostic and pulls no broker client; `edomata-kafka` (rdkafka), `edomata-rabbitmq` (lapin) and `edomata-pulsar` (pulsar) are separate crates you depend on only when you need them.

## The transactional outbox is the only source

Commands never publish to a broker. They write events and outbox rows to PostgreSQL in one transaction, as always; a **relay** then reads what was committed, publishes it, and **only after the broker acknowledged** marks the items as sent. Nothing is lost, nothing is published unless it was committed.

```text
command ──▶ aggregate ──▶ PostgreSQL: journal + outbox (one transaction)
                                        │
                        OutboxRelay ◀───┘ (in-process signal, LISTEN/NOTIFY or polling)
                             │ publish batch, in sequence order
                             ▼
                      Publisher (Kafka / RabbitMQ / Pulsar) ──ack──▶ mark as sent
```

### Delivery semantics

- **At-least-once**: a crash between publishing and marking redelivers the batch. Every message has a stable, deterministic id, `"{source}:outbox:{seq_nr}"` (or `"{source}:journal:{seq_nr}"`), so consumers deduplicate on it (the `edomata-id` header / `message_id` property).
- **Per-stream ordering**: relays publish in sequence order, one batch at a time; Kafka uses the stream id as partition key, RabbitMQ as routing key and awaits each confirm, Pulsar uses it as message key and awaits every send receipt.
- **Resilience**: transient failures are retried with exponential backoff (`RetryPolicy`); permanent failures, and transient ones once the policy's `max_retries` budget is exhausted, stop the relay with an error (counted as `failed`). `RelayMetrics` exposes published / retried / failed / lag counters, and the relay emits `tracing` events.
- **Multiple replicas**: `run_as_leader` with a `LeaderLock` (a PostgreSQL advisory lock) ensures only one relay per source publishes at a time, with automatic failover.

## Relaying the outbox

```rust,ignore
{{#include ../../samples/src/processes.rs:relay}}
```

With a real broker, the publisher is a `KafkaPublisher`, a `RabbitMqPublisher` or a `PulsarPublisher`. The samples below are compiled with the `kafka` / `rabbitmq` / `pulsar` features of the samples crate (`rust/book/samples/src/brokers.rs`); the full examples (`rust/examples/src/bin/kafka_relay.rs`, `rabbitmq_relay.rs`) also wire a writer:

### Kafka

```rust,ignore
{{#include ../../samples/src/brokers.rs:kafka}}
```

`KafkaPublisher::builder(bootstrap)` configures an idempotent producer (`enable.idempotence=true`, `acks=all`); `with_topic` chooses the topic per message (default: one topic per source), `with_config` sets any librdkafka property. Ids and metadata travel as record headers (see `edomata_broker::headers`).

```sh
KAFKA_BOOTSTRAP=localhost:9092 cargo run -p edomata-examples --features kafka --bin kafka_relay
```

### RabbitMQ

```rust,ignore
{{#include ../../samples/src/brokers.rs:rabbitmq}}
```

`RabbitMqPublisher::connect(uri)` enables publisher confirms; `with_exchange` / `with_routing_key` choose the route per message (defaults: the source and the stream id); messages are persistent (`delivery_mode = 2`) with `message_id` set, and the publisher reconnects after a connection failure.

```sh
AMQP_URL=amqp://guest:guest@localhost:5672/%2f cargo run -p edomata-examples --features rabbitmq --bin rabbitmq_relay
```

### Pulsar

```rust,ignore
{{#include ../../samples/src/brokers.rs:pulsar}}
```

`PulsarPublisher::builder(service_url)` (`pulsar://` or `pulsar+ssl://`) takes `with_token`, `with_namespace(tenant, namespace)`, `with_topic` / `with_fixed_topic` and the TLS settings of the `pulsar` client; `PulsarPublisherBuilder::from_clever_env()` reads the variables of Clever Cloud's Pulsar add-on (`ADDON_PULSAR_BINARY_URL`, `ADDON_PULSAR_TOKEN`, `ADDON_PULSAR_TENANT`, `ADDON_PULSAR_NAMESPACE`).

- **Topics**: a short name (the default is the sanitized relay source) goes to `persistent://<tenant>/<namespace>/<name>`; a full `persistent://...` or `non-persistent://...` URL is used as it is. A topic URL that is malformed is a permanent failure.
- **Producers**: Pulsar producers are per topic. They are created lazily, cached, and dropped when they fail (the whole connection too when the failure is a connection one), so the next publish recreates them. The first publish connects: a wrong address is a transient failure, not a build error. Keep the number of topics small: Clever Cloud documents 100 topics per namespace and 100 producers per topic.
- **Keys and ordering**: the stream id is the message key and producers do not batch, so a stream stays ordered within a topic (on a partitioned topic the key picks the partition).
- **Errors**: invalid topic, authentication or authorization refused, a payload above `with_max_message_size` (default 5 MiB less 10 KiB, checked before sending) are permanent; connection loss, timeouts and everything unrecognised are transient.
- **Build requirement**: the `pulsar` client's protocol definitions are compiled at build time, so building `edomata-pulsar` needs the `protoc` protobuf compiler (`apt install protobuf-compiler`); TLS uses rustls, no system OpenSSL.

The integration tests (`crates/edomata-pulsar/tests/pulsar.rs`) start a standalone Pulsar with testcontainers. Token authentication, TLS and the Clever Cloud add-on are not covered by them.

## Wake-ups across processes

A relay passes over the outbox when started, whenever a wake-up stream yields, and at least every `poll_interval`. In the writing process, `backend.updates().outbox()` is the in-process signal. In another process, use PostgreSQL `LISTEN/NOTIFY`: configure the driver with `with_outbox_notify_channel("channel")` (it raises `NOTIFY` inside the writing transaction) and wake the relay with `edomata_broker::postgres::listen(pool, "channel")`.

## Streaming the journal

`JournalRelay` tails the journal (raw events, not just notifications) from a checkpoint stored by a `CheckpointStore` (`PgCheckpointStore` in the `relay_checkpoints` table, created by `PgCheckpointStore::setup` or the opt-in `PGSchema::relay_checkpoints` DDL; `InMemoryCheckpointStore` for tests). The checkpoint advances only after the broker acknowledged a batch; journal messages carry the event id and version as extra headers.

## Testing

`RecordingPublisher` records every message it receives and can fail before or after publishing on demand, which is how the crash-between-publish-and-mark scenario is tested without a broker. The `edomata-kafka`, `edomata-rabbitmq` and `edomata-pulsar` crates run their integration tests against real brokers started with testcontainers (Docker required): no message marked before acknowledgment, redelivery and deduplication, per-stream ordering, leader election with two relays.

Running relays with several replicas, deduplicating on the consumer side and monitoring are covered in [Running relays in production](../operations/relays.md). See [ADR 0012](../design/adr-0012.md) for the design decisions.
