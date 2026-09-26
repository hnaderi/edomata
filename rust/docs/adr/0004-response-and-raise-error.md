# ADR 0004: `ResponseT`, `RaiseError` and the non-empty type

- Status: accepted
- Date: 2026-09-25
- Milestone: 1 (workspace + core)

## Context

Scala's `ResponseT[RES[+_], R, N, A]` pairs any "result-like" higher-kinded
type with notifications, and `ResponseD` / `ResponseE` specialise it to
`Decision` and `EitherNec`. The `RaiseError[F, R]` typeclass abstracts the
result type. Rust has no higher-kinded types, but it has generic associated
types (GATs, stable since 1.65).

Cats' `NonEmptyChain` is used for events and rejections.

## Decision

1. **`NonEmpty<T>`** is a small in-crate type wrapping a `Vec<T>` with a
   private field, dereferencing to `[T]`. It is `From<T>` so that APIs such
   as `Decision::accept(events: impl Into<NonEmpty<E>>)` accept a single
   event or several (`nonempty![a, b]`), which replaces Scala's `(ev, evs*)`
   varargs. Serde support is behind the `serde` feature and rejects empty
   input on deserialisation.

2. **`RaiseError`** is a trait with a GAT:

   ```rust
   pub trait RaiseError: Sized {
       type Rejection;
       type Output;
       type WithOutput<B>: RaiseError<Rejection = Self::Rejection, Output = B>;
       fn raise(errs: NonEmpty<Self::Rejection>) -> Self;
       fn pure(value: Self::Output) -> Self;
       fn as_result(&self) -> Result<&Self::Output, &NonEmpty<Self::Rejection>>;
       fn into_result(self) -> Result<Self::Output, NonEmpty<Self::Rejection>>;
       fn map<B, F>(self, f: F) -> Self::WithOutput<B>;
       fn and_then<B, F>(self, f: F) -> Self::WithOutput<B>;
       fn split<B>(value: Self::WithOutput<B>) -> Result<(Self::WithOutput<()>, B), ...>;
       fn join<B>(carrier: Self::WithOutput<()>, next: Self::WithOutput<B>) -> Self::WithOutput<B>;
   }
   ```

   `WithOutput<B>` plays the role of `RES[B]`. It is implemented for
   `Decision<R, E, A>` and `Result<A, NonEmpty<R>>`. `split` / `join` exist
   so that `tail_rec` can carry accumulated events across iterations without
   any panicking branch (the library forbids panics).

3. **`ResponseT<Res, N>`** has public fields `result` and `notifications:
   Vec<N>`. `ResponseD<R, E, N, A>` and `ResponseE<R, N, A>` are type aliases.
   Constructors specific to decisions (`accept`, `accept_return`) live in an
   inherent impl on the alias.

4. `and_then` implements Scala's semantics exactly: notifications
   concatenate on success, the next step's notifications replace the
   accumulated ones when it is rejected, and a rejected response never calls
   the continuation.

## Consequences

- Generic code over `Res` occasionally needs a turbofish (`then::<B>`)
  because the compiler cannot invert a GAT projection. Concrete uses
  (`ResponseD`, `ResponseE`) infer without annotations.
- Cats' `Traverse` instance for `ResponseT` is not ported; `Decision`
  provides `FromIterator` (sequence) and `transpose` for `Option`/`Result`.
