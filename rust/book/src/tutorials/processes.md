# Processes

The code of this chapter is compiled as part of the workspace (`rust/book/samples/src/processes.rs`).

## What are processes?

So far we have built individual aggregates that respond to commands. Real applications need more:

- **publishing notifications** to other systems (email, chat, external APIs);
- **building read models** (search indexes, reports, dashboards);
- **orchestrating workflows** across aggregates (order fulfilment, payments).

These are **processes**: background jobs that react to what happens in the system.

> **Real-world analogy**: in a restaurant kitchen, aggregates are the cooks preparing dishes; processes are the expeditor coordinating everything.

```text
 Account · Order · Delivery aggregates
            │
            ▼
   outbox / journal (notifications and events)
            │
   ┌────────┼─────────────┬──────────────────┐
   ▼        ▼             ▼                  ▼
 email    search index   payment workflow   Kafka / RabbitMQ relay
```

## Reading

Having a backend from the previous chapter (`Backend<Account, Event, Rejection, Notification>`), the readers are asynchronous **streams** (`futures::Stream`): items are processed one at a time, so millions of events can be read without loading them all in memory.

### Outbox

> **The outbox pattern**: you want to update your database *and* send a message. Done separately, one may fail while the other succeeds. Instead, the message is written to an outbox table in the same transaction as the state change; a separate process reads the outbox, sends the messages and marks them as processed.

```text
command → aggregate → BEGIN; update state; insert into outbox; COMMIT
                                    ...later...
process → read outbox → send email / call API → mark as sent
```

Use `backend.outbox()` directly, or the provided `OutboxConsumer`:

```rust,ignore
{{#include ../../samples/src/processes.rs:outbox}}
```

`OutboxConsumer` runs the handler on every unpublished item, then marks the batch as sent. It does not handle or recover handler failures, to keep error handling on the application side.

> The outbox is meant for atomically publishing external messages. It is not a queue for processing messages, and the best practice is exactly one consumer per outbox. To publish the outbox to a **message broker**, use the relays of [`edomata-broker`](../backends/brokers.md).

### Journal

> **What is the journal?** The complete, ordered history of all events. Unlike the outbox (notifications for external systems), the journal stores the domain events used for event sourcing.

```rust,ignore
{{#include ../../samples/src/processes.rs:journal}}
```

> **Use case: building a search index**. Read `read_all_after(last_checkpoint)`, update the index for each event and save the checkpoint. If the process crashes, it resumes from the last checkpoint. `edomata-broker`'s `JournalRelay` does exactly this towards a broker.

### Repository

> **What is the repository?** A higher-level reader that folds the journal (starting from the latest snapshot) into aggregate states.

```rust,ignore
{{#include ../../samples/src/processes.rs:repository}}
```

> **Tip**: the repository yields `AggregateState`: a valid state with its version, or the last valid state together with the conflicting event. Your journal never becomes corrupt through conflicting decisions, as they are rejected before being written; but if you change the *meaning* of events you change the meaning of history, and may face a conflicting stream. Invest in compatibility testing when migrating.

### Wake-ups

Instead of polling, processes can wait for the backend's in-process signals:

```rust,ignore
{{#include ../../samples/src/processes.rs:wakeups}}
```

Across processes, the sqlx drivers can raise `NOTIFY` when they write (`with_outbox_notify_channel`), see [Distributing events](../backends/brokers.md).

## Integrating

Streams and readers compose into any complex process or workflow.

### Process managers

> A process manager coordinates work across aggregates: it listens to notifications and sends commands to other aggregates.

An order-fulfilment process is an outbox consumer that reserves inventory, charges the payment and schedules delivery on `OrderPlaced`, and releases the inventory on `PaymentFailed`.

### Sagas

> A saga manages a long-running process across services, where each step has a compensation that undoes it if a later step fails.

```text
forward:      1. reserve inventory → 2. charge payment → 3. schedule delivery
compensation: release inventory   ← refund payment    ← cancel delivery
```

A custom read-side projection is another stream over journal events (`read_all_after(checkpoint)`), updating a report and saving its checkpoint.

### Relaying to a broker

```rust,ignore
{{#include ../../samples/src/processes.rs:relay}}
```

> **Key insight**: processes are streams and futures. They compose (`select`, `merge`), they are testable (feed test data), resilient (retry policies) and observable (`tracing`).
