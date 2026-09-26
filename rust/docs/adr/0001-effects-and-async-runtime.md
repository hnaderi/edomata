# ADR 0001: Effects, futures and the async runtime

- Status: accepted
- Date: 2026-09-25
- Milestone: 1 (workspace + core)

## Context

The Scala library is polymorphic in its effect type `F[_]` (Cats Effect `IO`
in practice) and uses `cats.Id` for pure programs. Rust has no higher-kinded
types, and the plan (`docs/plans/rust-port.md`) maps `F[_]` to `async fn`
and `std::future::Future`, with Tokio as the default runtime for the
PostgreSQL backends.

`edomata-core` must stay pure, must not depend on any async runtime, and must
compile to `wasm32-unknown-unknown`.

## Decision

1. Every effectful value in core is a **boxed, `Send` future**:

   ```rust
   pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
   ```

   `Send` is required so that programs can be driven by a multi-threaded
   executor (Tokio's default). Only `std` is used: no `futures`, `tokio`
   or `async-trait` dependency in `edomata-core`.

2. Pure programs (Scala's `App[Id, T]`) are not a separate type. A pure step
   is wrapped in `std::future::ready`, so one `Edomaton` type serves both the
   pure and the effectful cases (`Edomaton::from_fn` vs `Edomaton::new`).

3. Decision logic (`Decision`, `ResponseT`, `DomainModel::transition`) stays
   synchronous. Only the program wrappers (`Edomaton`, `Stomaton`, `Action`,
   `DecisionT`) are asynchronous.

4. `Action` and `DecisionT` are one-shot futures and implement `Future`
   directly, so they can be `.await`ed. `Edomaton` and `Stomaton` are
   reusable functions and expose `run(...)` returning a `BoxFuture`.

5. Tests in core use `futures::executor::block_on` (a dev-dependency); the
   backends use `tokio`.

## Consequences

- Type parameters that flow into futures must be `Send + 'static`. This is
  the price of runtime-agnostic, boxed futures and is documented on each
  type.
- A small allocation happens per program step (one `Box::pin` per
  combinator). This is acceptable for command handling workloads and keeps
  the API free of generic executor plumbing.
- Cats' `mapK` (natural transformations between effects) has no counterpart:
  there is only one effect type.
