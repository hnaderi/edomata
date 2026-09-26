# Introduction

Edomata is a lightweight library for implementing event-driven automata: event-sourced aggregates and CQRS state machines, with production-ready PostgreSQL backends. This book documents its **Rust port**, a set of crates under `rust/` that keep the semantics, invariants and on-disk formats of the Scala library, so that Rust and Scala services can share a database.

It is lightweight in the sense of simplicity: you always deal with a few primitive abstractions (`Decision`, `Edomaton`, `Stomaton`) that are intuitive and composable. It is built on the standard asynchronous Rust ecosystem (`std::future`, Tokio, sqlx, serde) and does not force any structure on your application: every program is a value you can run, test and compose however you like.

## Goals

Provide a solution to implement event-driven systems, using domain-driven design, by focusing on domain logic and building an automaton to represent the language.

## Alternatives

There are many successful attempts in this area; each has caveats, and Edomata attacks the problem from a different angle.

### Actor frameworks and their persistence modules

Actor systems (Akka and its persistence module on the JVM, and their equivalents elsewhere) are strong and well established, but:

* they are low level and easy to use but not simple;
* they try to solve many problems at once, which can mix domain logic and implementation details if not used carefully;
* sourcing inputs does not make a system event sourced, and the tooling can mislead here;
* actors are hard to compose or reuse, and require framework-specific experience.

They have a large ecosystem and should still be your first consideration if none of these drawbacks matters to you.

### Aecor

A purely functional wrapper (with a brilliant design) around Akka and Akka persistence, which solves composability and purity but adds another level of indirection.

### Axon

Not a direct alternative, but it tries to do some of the things this library is meant for.

### Comparison

|             | Edomata (Rust)                         | Edomata (Scala)     | Axon                | Akka and friends              | Aecor                                |
|-------------|----------------------------------------|---------------------|---------------------|-------------------------------|--------------------------------------|
| Paradigm    | Functional, value-based programs       | Purely functional   | OOP                 | Imperative / message passing  | Purely functional                    |
| Style       | Idiomatic async Rust                   | Haskell-ish Scala   | Java                | Erlang-ish Java               | MTL                                  |
| Usage       | Library                                | Library             | Framework           | Framework / Platform          | Library + Akka                       |
| Persistence | sqlx PostgreSQL driver*                | Skunk / Doobie*     | Axon server         | Akka persistence backends     | like Akka                            |
| Testing     | Trivial (`edomata-testkit`)            | Trivial (`munit`)   | Test library        | Test library                  | Trivial + like Akka                  |
| Runtime     | native, wasm32 for `edomata-core`      | JVM / JS / Native   | JVM                 | JVM                           | JVM                                  |
| Focus       | Expressive event-driven state machines | same                | CQRS + ES           | Actor model / Erlang OTP      | Functional behaviours for Akka       |
| Ecosystem   | Tokio, sqlx, serde                     | Typelevel           | Java                | Lightbend                     | Typelevel over Akka                  |

\* The Rust and Scala backends read and write the same tables and payloads.

### Your home-grown toolbox

Event sourcing does not require any framework, and frameworks almost always get in the way, as Greg Young and other pioneers of ES/CQRS have said. Using one for a real project quickly leads to the same conclusion, if you care for simplicity. So it is almost always better to develop your own toolbox and utilities, right? Yes, actually. And what is the point of this library then? That seems like a paradox; the [rationale](#rationale) explains it.

## Rationale

Designing DDD systems requires a lot of experience, which is hard to convey in textbooks: the problems DDD solves appear when you dig into a specific domain and try to discover it, a hard and time-consuming process that no book can simulate. Most development efforts rely on opinionated frameworks, which leads to the problems above; worst of all, most of the literature is written with OOP in mind and does not transfer easily to functional or Rust-style programming. Misconceptions about what event sourcing is, and when to use it, are widespread.

This library is meant to help with that: it shows how those ideas map to values and pure functions, and it is a handy, production-ready toolbox acting as a seed to grow your own.

## Next

- [Getting started](tutorials/getting-started.md)
- [Principles](principles/index.md)
- [Migration guide for Scala and Java users](other/migration-guide.md)
- [FAQ](other/faq.md)
- [Design goals](about/design-goals.md)
- [Crates](other/modules.md)
