# Event sourcing

All the code of this chapter is compiled and tested as part of the workspace (`rust/book/samples/src/eventsourcing.rs`).

## What is event sourcing?

Before diving into code, let's understand what event sourcing is and why it matters.

> **Traditional approach**: store only the current state.
> ```text
> Account: { id: "123", balance: 500 }
> ```
> When the balance changes, you overwrite the old value.

> **Event sourcing approach**: store the sequence of events that led to the current state.
> ```text
> Events for Account "123":
>   1. AccountOpened
>   2. Deposited(1000)
>   3. Withdrawn(300)
>   4. Deposited(100)
>   5. Withdrawn(300)
> Current balance: 1000 - 300 + 100 - 300 = 500
> ```

**Why event sourcing?**
- **Complete audit trail**: you know exactly what happened and when.
- **Time travel**: rebuild the state at any point in history.
- **Debugging**: see the exact sequence of events that led to a bug.
- **Analytics**: derive new insights from historical events.
- **Flexibility**: build multiple views (projections) from the same events.

## Imports

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:imports}}
```

## Domain layer

### Decision

It all starts with decisions!

> **What is a `Decision`?** Think of a committee making a choice. Asked to approve something, it can:
> - **accept** it (and record what it decided, e.g. "approved the budget");
> - **reject** it (and say why, e.g. "insufficient funds");
> - **stay undecided** (need more information).

`Decision<R, E, A>` models programs that decide in an event-driven context. These programs are pure and can accept events (`E`), reject with reasons (`R`), or stay indecisive, always producing an output `A`:

```text
 InDecisive(output) ──event──▶ Accepted(events, output) ──accumulates──▶ Accepted
        │                             │
        └──────── reason(s) ──────────┴──────────▶ Rejected(reasons)
```

Examples:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:decisions}}
```

> **Reading the code above**:
> - `Decision::pure(1)` creates an indecisive decision that just returns `1`;
> - `Decision::accept(...)` accepts with an event (the string is the event here);
> - `Decision::reject(...)` rejects with a reason.

With the `edomata_core::syntax` extension methods, the same reads as:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:decision_syntax}}
```

Decisions are composable: you can chain them together.

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:decision_compose}}
```

> **What does `then` do?** It is the sequencing operator (`>>` in Scala): do the left side, then the right side. If either rejects, the whole thing rejects. Accepted events accumulate in order.

Scala's for-comprehension becomes a chain of `and_then` (`flat_map` is an alias) and `map`:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:decision_chain}}
```

If any step rejects, the chain stops and returns the rejection. `Decision` also offers `validate` / `validate_with`, `assert_with`, `to_result`, `to_option`, `handle_error_with` and `tail_rec`; see the API documentation.

### Modelling

Let's use what we've learned so far to create an overly simplified model of a bank account. Assume the following business requirements:

- we must be able to open an account, if that account name was not used before;
- we can deposit or withdraw some amount from an open account;
- we must be able to close an account if it is settled (its balance is zero).

We start by modelling the domain events that came out of event storming or other design practices:

> **What is event storming?** A workshop technique where you use sticky notes to discover domain events by asking "what happens in the system?".

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:events}}
```

> **Why the past tense?** Events represent facts that already happened: "Deposited", not "Deposit".

We continue with the rejection scenarios:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:rejections}}
```

> **What are rejections?** The reasons a command may fail. They are not exceptions; they are expected business outcomes. "You can't withdraw $500 when you only have $100" is a valid rejection, not an error.

Now we model the aggregate root, and use `Decision` to write its logic:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:account}}
```

> **Why three states?** An account has a lifecycle: `New` (the initial state every aggregate starts in), `Open` with a balance, `Close`. This is a **state machine**: the account only transitions between states in specific ways.

> **Reading the type** `Decision<Rejection, Event, i64>`: can reject with a `Rejection`, can accept `Event`s, and yields the balance of the open account on success.

1. `perform` (on the model, see below) runs a decision on the current state and folds the accepted events into the new state, so you reuse the transition instead of repeating yourself.
2. `validate_with` ensures that after applying the events we end up in an `Open` state, and returns its balance instead of the whole `Account`.
3. A `Result<T, NonEmpty<Rejection>>` (Scala's `ValidatedNec`) is turned into a decision with `map_or_else(Decision::Rejected, ...)`, or with `to_decision_nec()` from the syntax module.
4. Since everything is a value, common validations like `must_be_open` are extracted and reused.

But you might say: domain logic is not just deciding, you must perform what you decided. Right, so let's go to that part.

### DomainModel

To complete the model we define the transitions (the famous event-sourcing fold) and the starting point with the `DomainModel` trait:

> **What is a fold?** Rebuilding state from events is like folding a list: `initial + event1 → state1`, `state1 + event2 → state2`, ... Each event transforms the previous state into the next.

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:model}}
```

1. `initial` is the initial state of your domain model, the first point in the timeline; think of it as `None` in `Option<T>`. Defining it explicitly keeps you working with your model (not `Option<Model>`) and lets you add defaults later, which reduces the need for migrations.
2. `transition` is the fold: given an event and a state, the next state, or a **conflict** (`Err`) when the event cannot be applied. Conflicts come from programming errors, storage manipulation, or transitions that changed meaning against an existing history.
3. Reusing `must_be_open` keeps the fold honest: a `Withdrawn` on a `New` account is a conflict, not a silent mistake.

> **Thinking further**: not all timelines are valid, and you do not control what you read from a journal. With states modelled as enums, the compiler makes those impossible cases explicit, which lets you find logical problems very easily.

> **Info**: if a programming error in your fold would cause a conflict, Edomata has your back: a decision whose events conflict with the model is never persisted (see the [processes](processes.md) chapter for `AggregateState::Conflicted`).

As simple as that!

### Testing the domain model

Everything is pure and every piece of logic is a value, so tests are plain assertions:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:model_tests}}
```

> **Why is testing so easy?** No database to mock, no external service to stub: call the function and check the result. That is the biggest benefit of separating domain logic from side effects.

## Service layer

Domain models are pure state machines in isolation. To build a complete service you need a way to store and load models, interact with them, and possibly perform side effects.

Edomata's tools for building services are also event-driven state machines. Like [actors](../principles/index.md#actor-model), they respond to incoming messages (domain commands), may change state by emitting events, may emit other events (notifications) for integration with other services, and may perform idempotent side effects, as they may run several times in case of failure.

### Edomaton

An `Edomaton` (plural: Edomata) is an event-driven automaton that can:

- read the current state;
- ask what is requested (the command message);
- perform side effects;
- decide (as described above);
- notify (emit notifications, integration events, ...);
- output a value.

Edomata are composable: you assemble them, transform them, and treat them like normal data.

> An `Edomaton` is like an employee; a `Decision` is like a business rule. **Employee**: "I received a request to withdraw money. Let me check the account, apply our rules, record the withdrawal and notify accounting." **Business rule**: "You can only withdraw if balance >= amount and amount > 0."

We need two more types: commands and notifications.

> **Commands vs events**: a command is a request to do something ("Deposit $100"), an event a fact that happened ("Deposited $100"). Commands can be rejected; events cannot.

> **What are notifications?** Messages sent to other systems after something happened, also called integration events. Unlike domain events (which rebuild state), notifications tell the outside world what happened.

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:commands}}
```

And our first service:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:service}}
```

> **Reading the service code**:
> - `AccountModel.dsl::<Command, Notification>()` fixes every type parameter once; the DSL marker is `Copy`, so closures capture it freely;
> - `dsl.router(...)` matches on the incoming command;
> - `dsl.state().and_then(|account| dsl.decide(account.open()))` reads the current state and runs the domain decision;
> - `dsl.aggregate_id()` is the id of the aggregate being handled;
> - `dsl.publish(...)` emits a notification for other systems;
> - the program runs on any `Future`-based runtime: there is no `F[_]` parameter to thread through.

That's it! We have written our first `Edomaton`.

### Testing an Edomaton

An `Edomaton` takes an environment and works in that context. Programs built with the DSL require a `RequestContext`: the command message (id for idempotency, time, address of the aggregate, payload) and the current state.

There are two ways of running it:

- `execute(&model, ctx)`, the recommended one, folds the decision into the model and returns an `EdomatonResult`, exactly what the real backends do;
- `run(ctx)` returns the raw `ResponseD` (decision and notifications), which you can interpret as you like.

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:scenario}}
```

Now we can assert our expectations with any test framework:

```rust,ignore
{{#include ../../samples/src/eventsourcing.rs:scenario_assert}}
```

The `edomata-testkit` crate makes such tests one-liners:

```rust,ignore
{{#include ../../samples/src/testing.rs:testkit}}
```

## What's next?

So far we have created program definitions. To run them as a real application, we compile them with a backend, the subject of the [next chapter](backends.md).
