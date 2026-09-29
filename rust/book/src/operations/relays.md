# Running relays in production

An outbox or journal relay ([Distributing events](../backends/brokers.md)) is a long-running task that reads PostgreSQL and publishes to a broker. This chapter covers where to run it, how to run several replicas, what delivery guarantees consumers get, and what to monitor. The code is in `rust/book/samples/src/operations.rs`.

## Where to run it

A relay needs only the database and the broker, so it can run:

- **in the writer process**, woken by the backend's in-process signal (`backend.updates().outbox()`): the simplest setup, with the lowest latency;
- **in a dedicated process**, woken by PostgreSQL `LISTEN/NOTIFY`: the writers raise `NOTIFY` in their transactions (`with_outbox_notify_channel` on the driver, `with_journal_notify_channel` for journal relays), and the relay subscribes with `edomata_broker::postgres::listen(pool, channel)`, which reconnects after connection losses.

In both cases the relay also polls every `poll_interval` (5 seconds by default), so a lost signal only delays delivery.

## Leader election

Run the relay in every replica with `run_as_leader`. A `LeaderLock` is a session-level PostgreSQL advisory lock (`pg_try_advisory_lock(hashtext('edomata-relay:{source}'))`) held on a dedicated connection: one replica holds it and publishes, the others retry every `leader_retry_interval` (1 second by default). The leader checks its connection every 5 seconds (`with_health_interval`); when the connection drops, PostgreSQL releases the lock, the leader stops publishing and stands by, and another replica takes over.

```rust,ignore
{{#include ../../samples/src/operations.rs:relay_process}}
```

- Use one lock per relay: the default key derives from the source, so give an outbox relay and a journal relay of the same source different keys (`LeaderLock::with_key`), as below.
- The lock lives on its own connection, detached from the pool: count one extra connection per relay replica against PostgreSQL's `max_connections`.
- Stop the relay by cancelling its `CancellationToken` (on `SIGTERM`, for instance); a leader releases the lock when it stops.

## Streaming the journal

A `JournalRelay` publishes raw events from a checkpoint, stored in the `relay_checkpoints` table by `PgCheckpointStore` (create it with `setup()` or `PGSchema::relay_checkpoints` in Flyway). The checkpoint advances only after the broker acknowledged a batch; each relay has a name (`"{source}:journal"` by default, `with_name` for several independent relays on one journal):

```rust,ignore
{{#include ../../samples/src/operations.rs:journal_relay}}
```

## Delivery guarantees

- **At-least-once.** Items are marked as sent (or the checkpoint advanced) only after the publisher returned, which it does once the broker acknowledged the whole batch. A crash in between publishes the batch again after the restart or the failover.
- **Stable message ids.** Every message has a deterministic id, `"{source}:outbox:{seq_nr}"` or `"{source}:journal:{seq_nr}"`, in the `edomata-id` header (Kafka) or the `message_id` property (RabbitMQ). A redelivery has the same id.
- **Ordering.** A relay publishes in sequence-number order, one batch at a time; Kafka partitions by stream id and RabbitMQ routes by stream id and awaits each confirm, so the messages of one aggregate stay in order.
- **Duplicates on the producer side too.** Rejected or indecisive commands that are sent again write their notifications again (only accepted commands are recorded as handled), and those are new outbox rows with new ids.

So consumers must be idempotent. Record each message id in the same transaction as the effect, and skip the ids already recorded:

```rust,ignore
{{#include ../../samples/src/operations.rs:dedup}}
```

For effects that cannot join a transaction (an e-mail, an HTTP call), pass the message id as an idempotency key to the remote system when it supports one.

## Failures

- A **transient** publish error (`PublishError::Transient`: broker unavailable, timeout) is retried with exponential backoff (`RetryPolicy`: from 200 ms, doubling, capped at 30 s, forever by default). Set `max_retries` to stop the relay instead, and let your supervisor restart it.
- A **permanent** error (`PublishError::Permanent`), an exhausted retry budget, an encoding error or a database error stops the relay with a `RelayError`. Nothing was marked, so a restart publishes the same batch again: fix the cause first (a payload the encoder cannot handle stops the relay at the same item every time).
- `run_as_leader` returns when the relay stops; restart it (with a delay) from your supervisor or orchestrator rather than in a tight loop.

## Monitoring

`relay.metrics()` shares the relay's counters; `snapshot()` reads them:

| Counter | Meaning | Alert when |
|---------|---------|------------|
| `published` | messages acknowledged and marked (or checkpointed) | it stops growing while writes continue |
| `retried` | batches retried after a transient failure | it grows steadily |
| `failed` | batches given up on (permanent failure, exhausted budget) | it is not zero |
| `lag` | items pending when the last pass started | it keeps growing |
| `passes` | completed passes | it stops growing |

The relays also emit `tracing` events: `info` when a replica becomes the leader, `warn` on retries and when the leader lock is lost, `error` on permanent failures, `debug` for stand-by passes. Install a subscriber (`tracing-subscriber`, OpenTelemetry, ...) in the relay process to collect them.
