# CQRS style

All the code of this chapter is compiled and tested as part of the workspace (`rust/book/samples/src/cqrs.rs`).

## What is CQRS?

CQRS stands for **Command Query Responsibility Segregation**: a pattern that separates reading data (queries) from writing data (commands).

> **Real-world analogy**: a library. The catalogue is optimised for finding books (queries); the checkout desk is optimised for borrowing and returning them (commands). They serve different purposes and could even be in different places.

```text
Traditional                     CQRS
┌──────────────────┐            ┌───────────────┐   ┌───────────────────┐
│ Create user      │            │ Command side  │   │ Query side        │
│ Update user      │            │ Create user   │   │ Get user by id    │
│ Get user by id   │    ──▶     │ Update user   │   │ Search users      │
│ Search users     │            │ Delete user   │   │ List all users    │
│ List all users   │            └───────────────┘   └───────────────────┘
└──────────────────┘
```

**When to use CQRS (without event sourcing)?**
- Your read and write patterns are very different.
- You want to scale reads and writes independently.
- Your business logic is complex enough to warrant separation.
- You don't need the full audit trail of event sourcing.

> **CQRS vs event sourcing**: you can have CQRS without event sourcing! This chapter uses Edomata's `Stomaton` for clean command handling while storing only the current state.

## Domain layer

### `Result<A, NonEmpty<R>>`

> **Why not `Decision`?** `Decision` accumulates events (event sourcing). Without events, success or failure is enough, and Rust already has a type for that: `Result`. The error side is a `NonEmpty<R>` so that a failure always carries at least one reason (Scala's `EitherNec`).

```rust,ignore
{{#include ../../samples/src/cqrs.rs:result_nec}}
```

### Modelling

Let's model an overly simplified food-delivery system with these requirements:

- a user can place an order;
- the line cook gets notified of new orders and allocates food to a cook;
- the user gets notified of the order status;
- the kitchen can report that food is ready;
- delivery gets notified of ready food and allocates it to a delivery unit;
- delivery can mark an order as delivered;
- the user can rate the experience.

The order lifecycle:

```text
[*] → Empty → New → Cooking → WaitingToPickUp → Delivering → Delivered → [*]
        place   allocate   ready        pick up       deliver
```

```rust,ignore
{{#include ../../samples/src/cqrs.rs:order}}
```

> **Why these three top-level states?** `Empty` (no order yet, the initial state), `Placed` (in progress, with a sub-status), `Delivered` (completed, tracking the rating).

> **Comparing with event sourcing**: the event-sourced `open` returns a `Decision` (events to store, then a new state); the CQRS `place` returns the new state directly.

> **Tip**: in real applications, make rejections specific (`OrderAlreadyCooking`, `CannotDeliverUnreadyFood`, ...) rather than one `InvalidRequest`.

### CqrsModel

A CQRS model only needs an initial state. Defining it explicitly keeps you working with your model (not `Option<Model>`) and lets you add defaults later.

```rust,ignore
{{#include ../../samples/src/cqrs.rs:model}}
```

> `CqrsModel` vs `DomainModel`: the event-sourced model requires `initial` **and** `transition`; the CQRS model only `initial`, as there are no events to fold.

### Testing the domain model

```rust,ignore
{{#include ../../samples/src/cqrs.rs:model_tests}}
```

## Service layer

### Stomaton

A `Stomaton` is a state-driven automaton that can:

- read or modify the current state;
- ask what is requested (the command message);
- perform side effects;
- decide (return a value or reject with one or more reasons);
- notify (emit notifications, integration events, ...);
- output a value.

| Feature | `Edomaton` | `Stomaton` |
|---------|------------|------------|
| Stores events | yes | no |
| Stores the current state | derived from events | yes, directly |
| Audit trail | complete history | current state only |
| Storage | grows over time | constant per aggregate |
| Use case | event sourcing | CQRS without ES |
| State update | `dsl.state().and_then(\|s\| dsl.decide(...))` | `dsl.modify_s(...)` |

Two more types: commands and notifications.

```rust,ignore
{{#include ../../samples/src/cqrs.rs:commands}}
```

> Commands are imperative ("place an order"); notifications are facts ("order received") sent to other systems after a command was processed.

And our first service:

```rust,ignore
{{#include ../../samples/src/cqrs.rs:service}}
```

> **Key difference from `Edomaton`**: `dsl.modify_s(|order| order.place(...))` directly replaces the state with the result of the domain function (or rejects); there is no event journal.

### Testing a Stomaton

A `Stomaton` runs on a command message and a state, and yields the new state (or the rejections) with the notifications:

```rust,ignore
{{#include ../../samples/src/cqrs.rs:scenario}}
```

> **Testing scenarios**: the happy path (`Order::Empty` + `Place`), errors (`Order::Placed` + `Place` must reject), and state transitions are all plain assertions. `edomata-testkit`'s `StomatonAssertions` (`expect`, `expect_rejection_with`) shortens them further.

## What's next?

To run these programs as a real application, we compile them with a backend, the subject of the [next chapter](backends.md).
