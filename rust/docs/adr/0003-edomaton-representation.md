# ADR 0003: Representation of `Edomaton`, `Stomaton` and the DSLs

- Status: accepted
- Date: 2026-09-25
- Milestone: 1 (workspace + core)

## Context

In Scala, `Edomaton[F, Env, R, E, N, A]` is a value class wrapping
`Env => F[ResponseD[R, E, N, A]]` and `Stomaton[F, Env, S, R, E, A]` wraps
`(Env, S) => F[ResponseE[R, E, (S, A)]]`. Programs are values that are
composed with `flatMap` and run many times by the backend's command handler.
The plan requires composition without macros.

## Decision

1. `Edomaton<Env, R, E, N, A>` wraps
   `Arc<dyn Fn(Env) -> BoxFuture<'static, ResponseD<R, E, N, A>> + Send + Sync>`.
   `Stomaton<Env, S, R, N, A>` wraps the same shape over `(Env, S)`.
   `Arc` makes programs cheap to clone and share between threads; `dyn Fn`
   keeps them reusable. `Clone` is implemented by hand (it must not require
   the type parameters to be `Clone`).

2. `Env` is taken **by value** on every run. Constructors that only read part
   of the input (`state()`, `command()`, ...) therefore never clone. Only
   `and_then` (which runs two programs on the same input) requires
   `Env: Clone`. Closures passed to combinators are `Fn + Send + Sync +
   'static` and are wrapped in `Arc` internally.

3. Closure-based constructors mirror Scala's: `new` (async function of the
   input), `from_fn` (pure function), `eval` / `run_with` (effects),
   `lift` / `decide` / `reject` / `pure` (constants, which must be `Clone`
   since a program can be run many times).

4. Scala's zero-cost `DomainDSL[C, S, E, R, N]` becomes `DomainDsl<C, S, E,
   R, N>`, a `Copy` zero-sized type whose methods return `Edomaton<
   RequestContext<C, S>, R, E, N, T>`. Its role is the same: pin the type
   parameters so that inference works inside `router` closures. `DomainModel`
   and `CqrsModel` expose `dsl::<C, N>()`.

5. `Stomaton`'s type parameters are ordered `<Env, S, R, N, A>` and the
   notification parameter is called `N` (Scala calls it `E`), because there
   are no domain events in a CQRS program.

## Alternatives considered

- **Generic closures (`Edomaton<F: Fn(...)>`)**: zero allocation but every
  combinator produces a new nominal type, so programs cannot be stored in
  fields or matched on without boxing anyway, and error messages become
  unreadable.
- **`async_trait`-style trait objects**: equivalent runtime cost, more
  boilerplate for users.
- **Macros for composition**: rejected by the plan.

## Consequences

- One heap allocation per step (boxed future) and one `Arc` per combinator;
  acceptable for command handling.
- Type parameters in programs are `Send + 'static`. Borrowed inputs are not
  supported; the backend owns the `RequestContext` it builds per command.
