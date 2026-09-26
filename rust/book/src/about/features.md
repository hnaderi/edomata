# Features

## Flexible data models

The `edomata-core` crate provides several data models that are flexible enough for other scenarios too:

- `Decision<R, E, A>`: a state machine representing decisions (`Accepted` with events, `Rejected` with reasons, `InDecisive`), with the usual combinators (`map`, `and_then`, `then`, `validate_with`, ...) and law-tested accumulation semantics.
- `DecisionT<R, E, A>`: a future that yields a `Decision`.
- `ResponseD` / `ResponseE`: a decision (or a `Result`) combined with publishing capability, like a writer monad.
- `Action<R, E, N, A>`: a future that yields a response.
- `Edomaton<Env, R, E, N, A>`: a program that reads an environment, runs effects and yields a response (event sourcing).
- `Stomaton<Env, S, R, N, A>`: a program that reads an environment and the current state, runs effects and yields a new state (CQRS without event sourcing).

## Convenient DSLs and friendly type inference

Working with a type that has five or six type parameters would be tedious if every call had to spell them out. The `DomainDsl` / `CqrsDomainDsl` markers fix all the parameters once (`model.dsl::<Command, Notification>()`), and their methods (`state`, `decide`, `publish`, `router`, ...) build programs whose types are inferred. As a rule of thumb you name each of your domain types once.

The `edomata_core::syntax` module adds extension methods such as `"event".accept()`, `reason.reject()`, `value.into_decision()`, `option.to_accepted_or(reason)` and `result.to_decision()`.

## Starter kits

Some opinionated types and models help while learning or testing. As explained in the [rationale](../introduction.md#rationale), this library is not a framework: its most important goal is to be a working, production-ready example of what an event-sourced system looks like. Among these types:

- `CommandMessage`: a standard way of modelling command messages.
- `MessageMetadata`: a standard way of modelling metadata (correlation and causation) in a message-passing system.
- `RequestContext`: a standard representation of a request in an event-sourced system.
- `edomata-testkit`: `expect` / `expect_rejection` helpers for domain tests, usable from any test runner.
- `edomata-simple`: a closure-based facade (`SimpleDomainModel`, `SimpleDecision`, `SimpleBackend::builder`) for applications that do not want to touch the generic core types.

You can create your own models and use them with or without the other data models Edomata provides.

## Production backends and integrations

- `edomata-sqlx`: event-sourcing and CQRS drivers on PostgreSQL (journal, outbox, commands, snapshots, states, migrations), schema or prefixed naming, DDL extraction for Flyway, `skip_setup`.
- `edomata-serde`: `jsonb` (default), `json` and `bytea` payloads through serde, compatible with the payloads written by the Scala Circe, jsoniter and uPickle codecs.
- `edomata-saas` / `edomata-saas-sqlx`: multi-tenant CRUD with automatic tenant isolation and authorisation, tenant-aware tables and optional Row-Level Security.
- `edomata-broker` with `edomata-kafka` and `edomata-rabbitmq`: opt-in, at-least-once distribution of outbox notifications and journal events with leader election and `LISTEN/NOTIFY` wake-ups (a Rust addition without Scala equivalent).
