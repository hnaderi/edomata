# ADR 0012: Broker distribution (Kafka, RabbitMQ)

- Status: accepted
- Date: 2026-09-25
- Milestone: 9 (broker distribution)

## Context

The plan adds a capability the Scala library does not have: distributing
outbox notifications (and, optionally, raw journal events) to a message
broker, fully opt-in, without ever publishing from inside a command. The
transactional outbox stays the only source of truth.

## Decisions

1. **Three crates, no cycle.** `edomata-broker` holds the broker-agnostic
   parts: `BrokerMessage`, the `Publisher` trait, `OutboxRelay`,
   `JournalRelay`, `CheckpointStore`, retry / metrics, and the PostgreSQL
   helpers (`LeaderLock`, `listen`, `PgCheckpointStore`). `edomata-kafka`
   (rdkafka) and `edomata-rabbitmq` (lapin) depend on it and implement
   `Publisher`. The plan's "`kafka` / `rabbitmq` features" cannot live on
   `edomata-broker` (optional dependencies on crates that depend on it
   would be a cycle), so the opt-in is at the application level: an
   application that depends only on `edomata-broker` pulls no broker
   client, and the examples package gates its broker binaries behind
   `kafka` / `rabbitmq` features. CI checks `cargo tree` for both.

2. **`Publisher::publish` is all-or-nothing per batch.** It returns only
   after the broker acknowledged every message; the relay then marks the
   batch as sent (outbox) or advances the checkpoint (journal). A failure
   after publishing but before marking therefore redelivers the whole
   batch with the same ids: at-least-once delivery, deduplicated by
   consumers on the stable id `"{source}:outbox:{seq_nr}"` /
   `"{source}:journal:{seq_nr}"`.

3. **`BrokerMessage` is broker-neutral.** It carries the payload as bytes
   (`MessageEncoder::serde` writes the same JSON as the `jsonb` columns),
   a content type, the stream id, the sequence number, a timestamp, the
   correlation / causation of the originating command and, for journal
   events, the event id and version. `BrokerMessage::headers()` gives one
   stable header list (`edomata-id`, `edomata-source`, `edomata-kind`,
   `edomata-stream`, `edomata-seqnr`, `edomata-time`, `content-type`,
   `correlation-id`, `causation-id`, `edomata-event-id`,
   `edomata-version`). Kafka attaches them all as record headers; RabbitMQ
   maps `edomata-id` and `content-type` to the `message_id` and
   `content_type` properties and attaches the rest as headers.

4. **Ordering.** Relays publish in sequence-number order, one batch at a
   time. Kafka uses the stream id as partition key with an idempotent
   producer (`enable.idempotence=true`, `acks=all`, which also bounds
   in-flight requests and preserves per-partition order); RabbitMQ
   publishes sequentially, awaiting each publisher confirm, with the
   stream id as routing key so a stream always follows one route.
   Persistent messages (`delivery_mode = 2`) and `message_id` are set.

5. **Wake-ups and shutdown.** A relay passes over its source once when
   started, then on every element of its wake-up streams (the backend's
   in-process `Notifications`, and `postgres::listen`, a `LISTEN` stream
   fed by the new `with_outbox_notify_channel` /
   `with_journal_notify_channel` options of the sqlx drivers, which raise
   `pg_notify` inside the writing transaction) and at least every
   `poll_interval`. It stops on a `tokio_util::sync::CancellationToken`.
   `LISTEN/NOTIFY` is off by default so the drivers stay identical to
   Scala unless asked.

6. **Retry and failure.** Transient publish failures are retried with
   exponential backoff (`RetryPolicy`: 200 ms doubling to 30 s, unlimited
   by default); permanent failures and an exhausted budget stop the relay
   with a `RelayError` for the supervisor to report. Nothing is ever
   marked on failure. `RelayMetrics` counts published messages, retried
   batches, batches given up on (permanent failure or exhausted budget),
   passes, and the backlog the last pass found (`lag`); the relay emits
   `tracing` events. RabbitMQ reports `NOT_FOUND` / `ACCESS_REFUSED` /
   `PRECONDITION_FAILED` as permanent, Kafka message-size and topic
   errors.

7. **Leader election.** `LeaderLock` takes
   `pg_try_advisory_lock(hashtext('edomata-relay:{source}'))` on a
   dedicated, detached connection kept for the guard's lifetime;
   `run_as_leader` keeps a stand-by replica retrying every
   `leader_retry_interval`, watches the connection's health while leading,
   and falls back to standing by when the lock is lost. Closing the
   connection releases the lock server-side, so a crashed leader is
   replaced automatically.

8. **Journal streaming is opt-in DDL.** `JournalRelay` stores its
   checkpoint in the namespace's `relay_checkpoints` table
   (`ns_relay_checkpoints` in prefixed mode, `"ns".relay_checkpoints` in
   schema mode; `PGSchema::relay_checkpoints`,
   `PgCheckpointStore::setup`), which is **not** part of
   `PGSchema::eventsourcing`, so the default DDL stays byte-identical to
   Scala's golden files.

## Tests

`edomata-broker`: in-memory tests of ids / headers / payloads, ordering,
batching, "nothing marked until acknowledged", retry and budget, crash
between publishing and marking (redelivery with identical ids), permanent
failures, wake-ups and polling, journal checkpoints; PostgreSQL tests of
the leader lock, `LISTEN/NOTIFY` wake-ups, the checkpoint table and two
relays with leader failover. `edomata-kafka` and `edomata-rabbitmq`:
testcontainers integration tests covering the plan's list against real
brokers: no message marked before the broker acknowledges (a producer
pointed at a closed port for Kafka, a missing exchange for RabbitMQ),
redelivery after a crash between publishing and marking, per-stream
ordering, deduplication by message id, leader election with two relays.
