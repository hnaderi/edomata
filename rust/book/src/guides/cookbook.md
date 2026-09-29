# Cookbook

Short recipes for common tasks. The code is compiled and tested with the workspace (`rust/book/samples/src/cookbook.rs`): every recipe runs on the in-memory driver, which follows the PostgreSQL drivers' concurrency and idempotency rules, so the same code works on `edomata-sqlx`.

The recipes share one small domain: the stock of an item, which is received and shipped.

## Define a domain

A domain is four types and a model: the events (what happened, past tense), the rejections (why a command is refused), the state, and a `DomainModel` giving the initial state and the fold (`transition`):

```rust,ignore
{{#include ../../samples/src/cookbook.rs:domain}}
```

- `initial` is the state of every aggregate that has no event yet; there is no "missing" aggregate.
- `transition` returns `Err` (a *conflict*) for an event that cannot apply to the state. A decision whose events conflict is never persisted; a journal that conflicts (because the meaning of an event changed) is read as `AggregateState::Conflicted`.
- Derive `Serialize` / `Deserialize` on the types that are stored (events, notifications, and the state when snapshots are persisted).

For a CQRS aggregate (no journal), implement `CqrsModel` instead: only `initial`, see [CQRS style](../tutorials/cqrs.md).

## Validate and reject

Business rules are pure functions returning a `Decision`: accept one or more events, or reject with one or more reasons. Reusable checks are `Result<T, NonEmpty<R>>` values, turned into decisions with `map_or_else(Decision::Rejected, ...)`:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:validate}}
```

A rejection is an expected business outcome, not an error: the service returns it as `Ok(Err(reasons))`, and nothing is written to the journal. Other helpers on `Decision`: `validate_with` (check the output), `assert_with`, `Decision::reject_when(predicate, reason)`, and `to_decision()` / `to_accepted_or(reason)` from `edomata_core::syntax`.

## Publish notifications

Notifications are messages for other systems, written to the outbox in the same transaction as the events. Publish them with `dsl.publish`; to publish from the state *after* the command, fold the decision with `perform`, which outputs the new state:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:commands}}
```

```rust,ignore
{{#include ../../samples/src/cookbook.rs:publish}}
```

- Notifications accumulate in order along a chain of programs.
- When a later step rejects, only the notifications of that step are kept, and `publish_on_rejection` (or `publish_on_rejection_with(|reasons| ...)`) adds notifications for the rejection case.
- The backend writes the notifications of a **rejected** or **indecisive** command to the outbox too: a rejection can be announced ("shipment refused") without changing the aggregate. Such commands are not recorded as handled, so a redelivery publishes them again (see [idempotency](#keep-commands-idempotent)).

## Read state

Inside a program, `dsl.state()` is the state before the command, and `dsl.aggregate_id()`, `dsl.command()`, `dsl.message_id()` and `dsl.metadata()` read the rest of the request. A program that accepts no event is *indecisive*: the journal is unchanged, and its notifications are still published:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:read_state}}
```

Outside a program, the backend's repository folds the journal (from the latest snapshot) into the current state:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:read_state_outside}}
```

`repository().history(id)` streams every state the aggregate went through. Edomata does not handle other reads: build read models from the outbox or the journal ([Processes](../tutorials/processes.md)), or with a CQRS handler in the save transaction ([Running](../tutorials/backends.md#cqrs-backends)).

## Compose programs

Programs are values: write small functions that return them, and route commands to them. `then` sequences two programs and `and_then` feeds the output of one to the next; events and notifications accumulate, and the first rejection stops the chain:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:compose}}
```

Every program of a chain reads the **same** state, the one of the request context, because a run does not apply events as it goes. When a step must see the events of a previous one, compose the decisions instead, folding each with `perform`:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:compose_decisions}}
```

Side effects go through `dsl.eval(|| async { ... })` and `Edomaton::eval_map` / `eval_tap`. A command may run several times (retries, redeliveries), so effects in a program must be idempotent; effects that must happen once belong in an outbox consumer.

## Optimistic concurrency and retries

Each command loads the aggregate at a version and appends its events at the next versions. When another writer appended first, the append fails with a unique violation, which the driver reports as `BackendError::VersionConflict`. The command handler then reloads the aggregate and runs the program again, with the delays of its `RetryConfig` (by default 5 attempts, starting at 2 seconds, doubling, plus up to 500 ms of jitter):

```rust,ignore
{{#include ../../samples/src/cookbook.rs:retries}}
```

When every attempt conflicts, the service returns `Err(BackendError::MaxRetryExceeded)`. Nothing was written, so the caller can retry later with the same command:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:handle_result}}
```

Code that writes through the storage itself can reuse the same policy with `edomata_backend::retry` (or `retry_with` and a `RetryConfig`):

```rust,ignore
{{#include ../../samples/src/cookbook.rs:retry_helper}}
```

Conflicts are expected under contention on one aggregate; if they are frequent, the aggregate is probably too large (split it) or a hot spot (queue its commands).

## Keep commands idempotent

The id of every **accepted** command is recorded (the `commands` table, in the same transaction as its events). A command whose id was already recorded is **redundant**: the service returns `Ok(Ok(()))` and writes nothing, so clients and consumers can retry freely. Rejected and indecisive commands are not recorded: sending one again runs the program again, which usually rejects again, and writes its notifications (if any) to the outbox again, so consumers of such notifications must deduplicate. For this to work, the id must identify the *intent*, not the attempt: derive it from the message that caused the command instead of generating a random one on each delivery:

```rust,ignore
{{#include ../../samples/src/cookbook.rs:idempotency}}
```

```rust,ignore
{{#include ../../samples/src/cookbook.rs:idempotency_test}}
```

Command ids are global to the namespace (the primary key of the `commands` table), not per aggregate: prefix them with what they do (`ship:...`) when several commands derive from one message. Two concurrent deliveries of the same command race on that key; the loser gets a version conflict, retries, and then finds the command redundant.

## Use snapshots and caching

Loading an aggregate means folding its journal. Two caches avoid most of that work:

- **Snapshots**: the last folded state of each aggregate, with its version. By default they are kept in memory only (1000 aggregates, an LRU cache). `persisted_snapshot(codec)` also writes evicted snapshots to the `snapshots` table, in batches, and every cached one on `close()`, so a restarted process does not refold every journal.
- **Command ids**: the ids of the last 1000 handled commands, so that redundant commands are detected without a query. The `commands` table remains the source of truth.

```rust,ignore
{{#include ../../samples/src/cookbook.rs:snapshots}}
```

- Call `backend.close().await` on shutdown to flush the persisted snapshots; dropping the backend without it loses the unflushed ones (they are rebuilt from the journal later).
- A snapshot is only a cache: an event migration truncates the `snapshots` table, and the states are rebuilt lazily.
- `disable_cache()` turns the command cache off. CQRS backends have a state cache instead of snapshots (`with_state_cache_size`), and `disable_cache()` turns both off.
