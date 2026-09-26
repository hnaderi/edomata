# Getting started

## What problem does Edomata solve?

Traditional applications store data by overwriting the current state. When you update a user's balance, you replace the old value with the new one. This approach has limitations:

- **Lost history**: you can't see how the balance changed over time.
- **Debugging difficulty**: when something goes wrong, you can't trace what happened.
- **Audit challenges**: compliance often requires knowing the full history of changes.
- **Concurrent conflicts**: two updates at the same time can cause data loss.

**Event-driven systems** solve these problems by storing *what happened* (events) rather than just the current state. Edomata helps you build these systems in a clean, composable way in Rust.

> **Real-world analogy**: think of a bank ledger vs. a bank balance. The balance tells you "you have $500"; the ledger tells you "you deposited $1000, withdrew $300, deposited $100, withdrew $300", from which you can always calculate the balance, plus understand how you got there.

## Add to your build

The crates live in the `rust/` workspace of the repository. Depend on them by path (or by git):

```toml
[dependencies]
edomata-core = { path = "rust/crates/edomata-core" }
# a PostgreSQL backend and serde codecs
edomata-backend = { path = "rust/crates/edomata-backend" }
edomata-sqlx = { path = "rust/crates/edomata-sqlx" }
edomata-serde = { path = "rust/crates/edomata-serde" }
```

`edomata-core` has no runtime dependency and builds for `wasm32-unknown-unknown`; the backends need Tokio and sqlx. The minimum supported Rust version is 1.88 (edition 2024).

## Layers of abstraction

Before jumping into code, let's see the big picture.

### Layers

Application logic is divided into two separate layers:

```text
┌──────────────────────── SERVICE LAYER ────────────────────────┐
│ Receives commands · loads the current state from storage      │
│ Calls domain logic to make decisions · persists events/state  │
│ Sends notifications to other systems · may involve effects    │
└───────────────────────────────┬───────────────────────────────┘
                                │ calls
┌───────────────────────────────▼───────────────────────────────┐
│                         DOMAIN LAYER                          │
│ Contains your business rules · pure functions: no I/O         │
│ Same input always gives the same output · easy to test        │
│ Models your aggregate root                                    │
└───────────────────────────────────────────────────────────────┘
```

> **What is a "pure function"?** A function with no side effects: it doesn't read from databases, send emails, or do anything that affects the outside world. Like a math formula, `f(x) = x + 1` always returns the same result for the same input.

> **What is a "side effect"?** Any action that affects something outside your function: writing to a database, sending an HTTP request, printing to the console, getting the current time. Side effects make code harder to test and reason about.

**Domain layer**:
* is pure and has no side effects at all;
* ensures aggregate invariants and implements all business rules and logic;
* does not change frequently in well-established businesses;
* models an aggregate root in DDD;
* aggregate roots hold just enough data to decide based on business rules and maintain invariants.

**Service layer**:
* may involve side effects;
* is very minimal and mostly gluing;
* must be idempotent (safe to run multiple times with the same result);
* models application services and command handlers in DDD.

> **What is "idempotent"?** An operation that produces the same result whether you run it once or several times, like pressing an elevator button. This matters because in distributed systems operations may be retried automatically.

### Aggregate space

> **What is an "aggregate"?** In domain-driven design, an aggregate is a cluster of related objects treated as a single unit for data changes. Think of a shopping cart: the cart and its items form an aggregate; you always go through the cart to keep it consistent.

> **What is an "aggregate root"?** The main entity that controls access to the aggregate. For a shopping cart, the Cart is the root.

* Each aggregate has a unique address (an id).
* All the possible aggregates together form the aggregate space, which is infinite (mathematically speaking).
* There is no such thing as a non-existing aggregate: aggregates that are not created yet exist in their initial state.

> **Real-world analogy**: phone numbers. The number 555-0199 "exists" even if no one has claimed it yet. Similarly, aggregate `order-12345` exists in its initial (empty) state before any event happens to it.

### Variants: choosing the right automaton

Two kinds of state machines are implemented in Edomata:

| Type | Name | Use when |
|------|------|----------|
| `Edomaton` | Event-driven automaton | You need full event sourcing: storing all events and rebuilding state from them |
| `Stomaton` | State-driven automaton | You only need the current state, but want clean command handling (CQRS style) |

> **What is event sourcing?** A persistence pattern where you store every change as an event, then rebuild the current state by replaying all events, like a git history.

> **What is CQRS?** Command Query Responsibility Segregation: separating the code that changes data (commands) from the code that reads data (queries), so each path can be optimised independently.

**Choose `Edomaton` when:**
- you need audit trails or compliance records;
- you want to be able to "replay" history;
- your domain naturally thinks in terms of "what happened";
- you need to build multiple views from the same events.

**Choose `Stomaton` when:**
- you only care about the current state;
- event history isn't valuable for your use case;
- you want simpler persistence (just save the state);
- you still want a clean separation of commands and business logic.

## Next

That's all we need to know for now. Let's jump into code!

For event-sourced applications, read [Event sourcing](eventsourcing.md); for CQRS applications (without event sourcing), read [CQRS style](cqrs.md).
